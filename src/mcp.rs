use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command as TokioCommand};
use tokio::sync::{Mutex, RwLock};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::SinkExt;
use log::{debug, info, warn, error};
use std::env;

fn default_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "required": []
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub servers: HashMap<String, McpServerConfig>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<String>,
    #[serde(flatten)]
    pub method: McpMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum McpMethod {
    #[serde(rename = "initialize")]
    Initialize { 
        protocol_version: String,
        capabilities: McpClientCapabilities,
        client_info: McpClientInfo,
    },
    #[serde(rename = "tools/list")]
    ListTools,
    #[serde(rename = "tools/call")]
    CallTool { 
        name: String,
        arguments: Option<Value>,
    },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "notifications/initialized")]
    Initialized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientCapabilities {
    pub tools: Option<McpToolsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolsCapability {
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
}

#[derive(Debug)]
pub struct McpConnection {
    pub name: String,
    pub process: Option<Child>,
    pub websocket: Option<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>>,
    pub reader: Option<BufReader<tokio::process::ChildStdout>>,
    pub writer: Option<tokio::process::ChildStdin>,
    pub request_id: u64,
    pub pending_requests: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<McpResponse>>>>,
    pub tools: Arc<RwLock<Vec<McpTool>>>,
    pub tools_version: Arc<RwLock<u64>>,
}

impl McpConnection {
    pub fn new(name: String) -> Self {
        Self {
            name,
            process: None,
            websocket: None,
            reader: None,
            writer: None,
            request_id: 1,
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            tools: Arc::new(RwLock::new(Vec::new())),
            tools_version: Arc::new(RwLock::new(0)),
        }
    }

      pub async fn connect_stdio(&mut self, command: &str, args: &[String], env: &HashMap<String, String>) -> Result<()> {
        info!("Starting MCP server: {} {}", command, args.join(" "));
        debug!("MCP server details:");
        debug!("  Command: {}", command);
        debug!("  Args: {:?}", args);
        debug!("  Environment variables: {}", env.len());
        
        // Handle Windows-specific command resolution
        let (cmd, cmd_args) = if cfg!(target_os = "windows") {
            // On Windows, try to resolve the command properly
            if command == "npx" {
                // Try to find npx in common locations
                let npx_path = self.find_npx_on_windows().await?;
                (npx_path, args.to_vec())
            } else {
                // For other commands, try to find them in PATH
                match which::which(command) {
                    Ok(path) => (path.to_string_lossy().to_string(), args.to_vec()),
                    Err(_) => (command.to_string(), args.to_vec()),
                }
            }
        } else {
            (command.to_string(), args.to_vec())
        };
        
        let mut cmd_process = TokioCommand::new(&cmd);
        cmd_process.args(&cmd_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env {
            cmd_process.env(key, value);
        }

        // Add more detailed error logging for debugging
        debug!("Executing command: {} with args: {:?}", cmd, cmd_args);
        
        let mut child = cmd_process.spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn MCP server process '{}': {}\nPlease ensure:\n1. The command exists and is executable\n2. All required dependencies are installed\n3. The command is in your PATH\n4. On Windows: Node.js and npm are properly installed", cmd, e))?;
        
        let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to get stdin from child process"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to get stdout from child process"))?;

        self.reader = Some(BufReader::new(stdout));
        self.writer = Some(stdin);
        self.process = Some(child);

        // Start message handling loop
        let pending_requests = self.pending_requests.clone();
        let tools = self.tools.clone();
        let tools_version = self.tools_version.clone();
        let name = self.name.clone();
        let mut reader = self.reader.take().unwrap();

        tokio::spawn(async move {
            let mut buffer = String::new();
            loop {
                match reader.read_line(&mut buffer).await {
                    Ok(0) => {
                        debug!("MCP server {} closed connection", name);
                        break;
                    }
                    Ok(_) => {
                        if buffer.trim().is_empty() {
                            buffer.clear();
                            continue;
                        }

                        debug!("Received from MCP server {}: {}", name, buffer.trim());
                        
                        match serde_json::from_str::<McpResponse>(&buffer.trim()) {
                            Ok(response) => {
                                if let Some(id) = &response.id {
                                    let mut pending = pending_requests.lock().await;
                                    if let Some(sender) = pending.remove(id) {
                                        let _ = sender.send(response);
                                    }
                                } else if let Some(result) = &response.result {
                                    // Handle notifications
                                    if let Some(tools_list) = result.get("tools") {
                                        debug!("Received tools via notification from {}: {}", name, serde_json::to_string_pretty(tools_list).unwrap_or_else(|_| "Invalid JSON".to_string()));
                                        
                                        // Try to parse tools with better error handling
                                        match serde_json::from_value::<Vec<Value>>(tools_list.clone()) {
                                            Ok(raw_tools) => {
                                                let mut parsed_tools = Vec::new();
                                                for (i, raw_tool) in raw_tools.into_iter().enumerate() {
                                                    match serde_json::from_value::<McpTool>(raw_tool.clone()) {
                                                        Ok(tool) => {
                                                            debug!("Successfully parsed tool: {} from {}", tool.name, name);
                                                            parsed_tools.push(tool);
                                                        }
                                                        Err(e) => {
                                                            warn!("Failed to parse tool {} from server '{}' (index: {}): {}. Tool data: {}", 
                                                                  i, name, i, e, serde_json::to_string_pretty(&raw_tool).unwrap_or_else(|_| "Invalid JSON".to_string()));
                                                            
                                                            // Try to create a minimal tool with the available data
                                                            if let Some(tool_name) = raw_tool.get("name").and_then(|v| v.as_str()) {
                                                                let fallback_tool = McpTool {
                                                                    name: tool_name.to_string(),
                                                                    description: raw_tool.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                                                    input_schema: default_input_schema(),
                                                                };
                                                                info!("Created fallback tool '{}' with default schema", tool_name);
                                                                parsed_tools.push(fallback_tool);
                                                            }
                                                        }
                                                    }
                                                }
                                                *tools.write().await = parsed_tools;
                                                // Increment version for this connection
                                                let mut version = tools_version.write().await;
                                                *version += 1;
                                                info!("Updated {} tools from MCP server {} via notification", tools.read().await.len(), name);
                                            }
                                            Err(e) => {
                                                warn!("Failed to parse tools array from notification {}: {}. Raw response: {}", name, e, serde_json::to_string_pretty(tools_list).unwrap_or_else(|_| "Invalid JSON".to_string()));
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse MCP response from {}: {}. Response: {}", name, e, buffer.trim());
                                error!("MCP server '{}' sent invalid JSON data. This may indicate:", name);
                                error!("1. Server is not following MCP protocol correctly");
                                error!("2. Server process is crashing or outputting error messages");
                                error!("3. Version mismatch between client and server");
                                debug!("Raw response that failed to parse: {}", buffer.trim());
                            }
                        }
                        buffer.clear();
                    }
                    Err(e) => {
                        error!("Error reading from MCP server {}: {}", name, e);
                        error!("MCP server {} connection broken - tools may be unavailable", name);
                        break;
                    }
                }
            }
        });

        // Initialize connection
        match self.initialize().await {
            Ok(_) => {
                info!("MCP server '{}' initialization completed successfully", self.name);

                // Verify the process is still running after initialization
                if let Some(ref mut process) = self.process {
                    match process.try_wait() {
                        Ok(Some(status)) => {
                            error!("MCP server '{}' process exited unexpectedly with status: {}", self.name, status);
                            return Err(anyhow::anyhow!("MCP server '{}' process exited during initialization", self.name));
                        }
                        Ok(None) => {
                            debug!("MCP server '{}' process is running normally", self.name);
                        }
                        Err(e) => {
                            warn!("Failed to check MCP server '{}' status: {}", self.name, e);
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                error!("MCP server '{}' initialization failed: {}", self.name, e);

                // Check if the process is still running
                if let Some(ref mut process) = self.process {
                    match process.try_wait() {
                        Ok(Some(status)) => {
                            error!("MCP server '{}' process exited with status: {}", self.name, status);
                        }
                        Ok(None) => {
                            debug!("MCP server '{}' process is still running but initialization failed", self.name);
                        }
                        Err(_) => {}
                    }
                }

                Err(e)
            }
        }
    }

    /// Get the current tools version
    pub async fn get_tools_version(&self) -> u64 {
        *self.tools_version.read().await
    }

    /// Find npx executable on Windows
    async fn find_npx_on_windows(&self) -> Result<String> {
        // Try common Node.js installation paths on Windows
        let common_paths = vec![
            r"C:\Program Files\nodejs\npx.cmd",
            r"C:\Program Files (x86)\nodejs\npx.cmd",
            r"%APPDATA%\npm\npx.cmd",
        ];

        // First try to find npx in PATH
        if let Ok(npx_path) = which::which("npx") {
            return Ok(npx_path.to_string_lossy().to_string());
        }

        // Try common installation paths
        for path in &common_paths {
            let expanded_path = env::var("APPDATA").unwrap_or_default();
            let full_path = path.replace("%APPDATA%", &expanded_path);
            
            if Path::new(&full_path).exists() {
                info!("Found npx at: {}", full_path);
                return Ok(full_path);
            }
        }

        // Try to find Node.js and use npx from there
        if let Ok(node_path) = which::which("node") {
            if let Some(parent) = Path::new(&node_path).parent() {
                let npx_path = parent.join("npx.cmd");
                if npx_path.exists() {
                    info!("Found npx at: {}", npx_path.display());
                    return Ok(npx_path.to_string_lossy().to_string());
                }
            }
        }

        Err(anyhow::anyhow!("npx not found. Please install Node.js and npm from https://nodejs.org/"))
    }

    pub async fn connect_websocket(&mut self, url: &str) -> Result<()> {
        info!("Connecting to MCP server via WebSocket: {}", url);
        
        let (ws_stream, _) = connect_async(url).await?;
        self.websocket = Some(ws_stream);

        // Initialize connection
        self.initialize().await?;
        Ok(())
    }

    async fn initialize(&mut self) -> Result<()> {
        let init_request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(self.next_id()),
            method: McpMethod::Initialize {
                protocol_version: "2024-11-05".to_string(),
                capabilities: McpClientCapabilities {
                    tools: Some(McpToolsCapability {
                        list_changed: Some(true),
                    }),
                },
                client_info: McpClientInfo {
                    name: "ai-agent".to_string(),
                    version: "0.1.0".to_string(),
                },
            },
        };

        let response = self.send_request(init_request).await?;
        
        if response.error.is_some() {
            return Err(anyhow::anyhow!("MCP initialization failed: {:?}", response.error));
        }

        // Send initialized notification
        let initialized = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: McpMethod::Initialized,
        };

        self.send_notification(initialized).await?;

        // Load tools
        self.load_tools().await?;
        
        info!("MCP server {} initialized successfully", self.name);
        Ok(())
    }

    async fn load_tools(&mut self) -> Result<()> {
        let tools_request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(self.next_id()),
            method: McpMethod::ListTools,
        };

        let response = self.send_request(tools_request).await?;
        
        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("Failed to list tools: {:?}", error));
        }

        if let Some(result) = response.result {
            if let Some(tools_value) = result.get("tools") {
                debug!("Raw tools response from {}: {}", self.name, serde_json::to_string_pretty(tools_value)?);
                
                // Try to parse tools with better error handling
                match serde_json::from_value::<Vec<Value>>(tools_value.clone()) {
                    Ok(raw_tools) => {
                        let mut parsed_tools = Vec::new();
                        for (i, raw_tool) in raw_tools.into_iter().enumerate() {
                            match serde_json::from_value::<McpTool>(raw_tool.clone()) {
                                Ok(tool) => {
                                    debug!("Successfully parsed tool: {} from {}", tool.name, self.name);
                                    parsed_tools.push(tool);
                                }
                                Err(e) => {
                                    warn!("Failed to parse tool {} from server '{}' (index: {}): {}. Tool data: {}", 
                                          i, self.name, i, e, serde_json::to_string_pretty(&raw_tool).unwrap_or_else(|_| "Invalid JSON".to_string()));
                                    
                                    // Try to create a minimal tool with the available data
                                    if let Some(name) = raw_tool.get("name").and_then(|v| v.as_str()) {
                                        let fallback_tool = McpTool {
                                            name: name.to_string(),
                                            description: raw_tool.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                            input_schema: default_input_schema(),
                                        };
                                        info!("Created fallback tool '{}' with default schema", name);
                                        parsed_tools.push(fallback_tool);
                                    }
                                }
                            }
                        }
                        *self.tools.write().await = parsed_tools;
                        // Increment version for this connection
                        let mut version = self.tools_version.write().await;
                        *version += 1;
                        info!("Loaded {} tools from MCP server {}", self.tools.read().await.len(), self.name);
                    }
                    Err(e) => {
                        warn!("Failed to parse tools array from {}: {}. Raw response: {}", self.name, e, serde_json::to_string_pretty(tools_value)?);
                        return Err(anyhow::anyhow!("Invalid tools response format from MCP server '{}': {}", self.name, e));
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Option<Value>) -> Result<Value> {
        // Check if connection is still alive
        if let Some(ref mut process) = self.process {
            match process.try_wait() {
                Ok(Some(_)) => {
                    return Err(anyhow::anyhow!("MCP server '{}' process has terminated", self.name));
                }
                Ok(None) => {
                    // Process is still running, good
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to check MCP server '{}' status: {}", self.name, e));
                }
            }
        }

        debug!("Calling MCP tool '{}' on server '{}'", name, self.name);

        let tool_request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(self.next_id()),
            method: McpMethod::CallTool {
                name: name.to_string(),
                arguments,
            },
        };

        let response = self.send_request(tool_request).await?;

        if let Some(error) = response.error {
            error!("MCP tool '{}' failed on server '{}': {:?}", name, self.name, error);
            return Err(anyhow::anyhow!("Tool call failed: {:?}", error));
        }

        debug!("MCP tool '{}' completed successfully on server '{}'", name, self.name);
        Ok(response.result.unwrap_or(json!({})))
    }

    async fn send_request(&mut self, request: McpRequest) -> Result<McpResponse> {
        let id = request.id.clone().unwrap();
        let request_json = serde_json::to_string(&request)?;
        
        debug!("Sending MCP request to {}: {}", self.name, request_json);

        // Create response channel
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_requests.lock().await.insert(id.clone(), tx);

        // Send request
        if let Some(writer) = &mut self.writer {
            writer.write_all(request_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        } else if let Some(websocket) = &mut self.websocket {
            websocket.send(Message::Text(request_json)).await?;
        } else {
            return Err(anyhow::anyhow!("No connection available"));
        }

        // Wait for response with timeout
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(30), // 30 second timeout
            rx
        ).await
            .map_err(|_| anyhow::anyhow!("MCP server '{}' timed out after 30 seconds", self.name))?
            .map_err(|_| anyhow::anyhow!("MCP server '{}' response channel was dropped", self.name))?;

        Ok(response)
    }

    async fn send_notification(&mut self, notification: McpRequest) -> Result<()> {
        let notification_json = serde_json::to_string(&notification)?;
        
        debug!("Sending MCP notification to {}: {}", self.name, notification_json);

        if let Some(writer) = &mut self.writer {
            writer.write_all(notification_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        } else if let Some(websocket) = &mut self.websocket {
            websocket.send(Message::Text(notification_json)).await?;
        } else {
            return Err(anyhow::anyhow!("No connection available"));
        }

        Ok(())
    }

    pub async fn get_tools(&self) -> Vec<McpTool> {
        self.tools.read().await.clone()
    }

    fn next_id(&mut self) -> String {
        let id = self.request_id.to_string();
        self.request_id += 1;
        id
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
        }
        
        if let Some(mut websocket) = self.websocket.take() {
            let _ = websocket.close(None).await;
        }

        info!("Disconnected from MCP server {}", self.name);
        Ok(())
    }
}

#[derive(Debug)]
pub struct McpManager {
    connections: Arc<RwLock<HashMap<String, McpConnection>>>,
    config_path: String,
}

impl McpManager {
    pub fn new() -> Self {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| Path::new(".").to_path_buf())
            .join("ai-agent");
        
        let config_path = config_dir.join("mcp.toml").to_string_lossy().to_string();
        
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            config_path,
        }
    }

    pub async fn load_config(&self) -> Result<McpConfig> {
        if Path::new(&self.config_path).exists() {
            let content = tokio::fs::read_to_string(&self.config_path).await?;
            let config: McpConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(McpConfig::default())
        }
    }

    pub async fn save_config(&self, config: &McpConfig) -> Result<()> {
        if let Some(parent) = Path::new(&self.config_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let content = toml::to_string_pretty(config)?;
        tokio::fs::write(&self.config_path, content).await?;
        Ok(())
    }

    pub async fn add_server(&self, name: &str, server_config: McpServerConfig) -> Result<()> {
        let mut config = self.load_config().await?;
        config.servers.insert(name.to_string(), server_config);
        self.save_config(&config).await?;
        Ok(())
    }

    pub async fn remove_server(&self, name: &str) -> Result<()> {
        let mut config = self.load_config().await?;
        config.servers.remove(name);
        self.save_config(&config).await?;
        
        // Disconnect if connected
        let mut connections = self.connections.write().await;
        if let Some(mut connection) = connections.remove(name) {
            let _ = connection.disconnect().await;
        }
        
        Ok(())
    }

    pub async fn connect_server(&self, name: &str) -> Result<()> {
        let config = self.load_config().await?;
        let server_config = config.servers.get(name)
            .ok_or_else(|| anyhow::anyhow!("Server '{}' not found in configuration", name))?;

        if !server_config.enabled {
            return Err(anyhow::anyhow!("Server '{}' is disabled", name));
        }

        let mut connection = McpConnection::new(name.to_string());

        if let Some(url) = &server_config.url {
            // Connect via WebSocket
            connection.connect_websocket(url).await?;
        } else if let Some(command) = &server_config.command {
            // Connect via stdio
            let args = server_config.args.as_deref().unwrap_or(&[]);
            let env_vars = server_config.env.as_ref().cloned().unwrap_or_default();
            connection.connect_stdio(command, args, &env_vars).await?;
        } else {
            return Err(anyhow::anyhow!("Server '{}' has no command or URL configured", name));
        }

        self.connections.write().await.insert(name.to_string(), connection);
        info!("Connected to MCP server: {}", name);
        Ok(())
    }

    pub async fn disconnect_server(&self, name: &str) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(mut connection) = connections.remove(name) {
            connection.disconnect().await?;
            info!("Disconnected from MCP server: {}", name);
        }
        Ok(())
    }

    pub async fn reconnect_server(&self, name: &str) -> Result<()> {
        self.disconnect_server(name).await?;
        self.connect_server(name).await?;
        Ok(())
    }

    pub async fn list_servers(&self) -> Result<Vec<(String, McpServerConfig, bool)>> {
        let config = self.load_config().await?;
        let connections = self.connections.read().await;
        
        let mut servers = Vec::new();
        for (name, server_config) in config.servers {
            let connected = connections.contains_key(&name);
            servers.push((name, server_config, connected));
        }
        
        Ok(servers)
    }

    pub async fn get_all_tools(&self) -> Result<Vec<(String, McpTool)>> {
        let connections = self.connections.read().await;
        let mut all_tools = Vec::new();
        
        for (name, connection) in connections.iter() {
            let tools = connection.get_tools().await;
            for tool in tools {
                all_tools.push((name.clone(), tool));
            }
        }
        
        Ok(all_tools)
    }

    /// Get the global tools version (sum of all connection versions)
    pub async fn get_tools_version(&self) -> u64 {
        let connections = self.connections.read().await;
        let mut total_version = 0u64;
        
        for connection in connections.values() {
            total_version = total_version.wrapping_add(connection.get_tools_version().await);
        }
        
        total_version
    }

    /// Check if tools have changed since the last check
    pub async fn have_tools_changed(&self, last_version: u64) -> bool {
        self.get_tools_version().await != last_version
    }

    pub async fn call_tool(&self, server_name: &str, tool_name: &str, arguments: Option<Value>) -> Result<Value> {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(server_name) {
            connection.call_tool(tool_name, arguments).await
        } else {
            Err(anyhow::anyhow!("Server '{}' is not connected", server_name))
        }
    }

    pub async fn connect_all_enabled(&self) -> Result<()> {
        let config = self.load_config().await?;
        let mut connected_count = 0;
        
        for (name, server_config) in config.servers {
            if server_config.enabled {
                match self.connect_server(&name).await {
                    Ok(_) => {
                        connected_count += 1;
                        info!("Connected to MCP server: {}", name);
                    }
                    Err(e) => {
                        warn!("Failed to connect to MCP server '{}': {}", name, e);
                    }
                }
            }
        }
        
        if connected_count > 0 {
            info!("Connected to {} MCP server(s)", connected_count);
        }
        
        Ok(())
    }

    pub async fn disconnect_all(&self) -> Result<()> {
        let connections = self.connections.read().await;
        let server_names: Vec<String> = connections.keys().cloned().collect();
        drop(connections);
        
        for name in server_names {
            let _ = self.disconnect_server(&name).await;
        }
        
        Ok(())
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}