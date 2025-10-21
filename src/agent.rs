use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{debug, info, warn, error};
use crate::mcp::McpManager;
use crate::security::BashSecurityManager;
use regex::Regex;
use serde_json::{json, Value};
use colored::*;

use crate::config::Config;
use crate::anthropic::{AnthropicClient, Message, ContentBlock, Usage};
use crate::tools::{Tool, ToolResult, get_builtin_tools, bash, ToolCall};
use crate::tool_display::{ToolCallDisplay, SimpleToolDisplay, should_use_pretty_output, ToolDisplay};

#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub request_count: u32,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
}

impl TokenUsage {
    pub fn new() -> Self {
        Self {
            request_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    pub fn add_usage(&mut self, usage: &Usage) {
        self.request_count += 1;
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
    }

    pub fn total_tokens(&self) -> u32 {
        self.total_input_tokens + self.total_output_tokens
    }

    pub fn reset(&mut self) {
        self.request_count = 0;
        self.total_input_tokens = 0;
        self.total_output_tokens = 0;
    }
}

pub struct Agent {
    client: AnthropicClient,
    model: String,
    tools: Arc<RwLock<HashMap<String, Tool>>>,
    conversation: Vec<Message>,
    token_usage: TokenUsage,
    system_prompt: Option<String>,
    mcp_manager: Option<Arc<McpManager>>,
    last_mcp_tools_version: u64,
    bash_security_manager: Arc<RwLock<BashSecurityManager>>,
}

impl Agent {
    pub fn new(config: Config, model: String) -> Self {
        let client = AnthropicClient::new(config.api_key, config.base_url);
        let mut tools: std::collections::HashMap<String, Tool> = get_builtin_tools()
            .into_iter()
            .map(|tool| (tool.name.clone(), tool))
            .collect();

        // Create bash security manager
        let bash_security_manager = Arc::new(RwLock::new(
            BashSecurityManager::new(config.bash_security.clone())
        ));

        // Add bash tool with security to the initial tools
        let security_manager = bash_security_manager.clone();
        tools.insert("bash".to_string(), Tool {
            name: "bash".to_string(),
            description: "Execute shell commands and return the output (with security)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    }
                },
                "required": ["command"]
            }),
            handler: Box::new(move |call: ToolCall| {
                let security_manager = security_manager.clone();
                Box::pin(async move {
                    // Create a wrapper function that handles the mutable reference
                    async fn bash_wrapper(
                        call: ToolCall,
                        security_manager: Arc<RwLock<BashSecurityManager>>,
                    ) -> Result<ToolResult> {
                        let mut manager = security_manager.write().await;
                        bash(&call, &mut *manager).await
                    }
                    
                    bash_wrapper(call, security_manager).await
                })
            }),
        });

        Self {
            client,
            model,
            tools: Arc::new(RwLock::new(tools)),
            conversation: Vec::new(),
            token_usage: TokenUsage::new(),
            system_prompt: None,
            mcp_manager: None,
            last_mcp_tools_version: 0,
            bash_security_manager,
        }
    }

    pub fn with_mcp_manager(mut self, mcp_manager: Arc<McpManager>) -> Self {
        self.mcp_manager = Some(mcp_manager);
        self
    }

    /// Refresh MCP tools from connected servers (only if they have changed)
    pub async fn refresh_mcp_tools(&mut self) -> Result<()> {
        if let Some(mcp_manager) = &self.mcp_manager {
            // Check if tools have changed since last refresh
            let current_version = mcp_manager.get_tools_version().await;
            
            if current_version == self.last_mcp_tools_version {
                debug!("MCP tools unchanged, skipping refresh");
                return Ok(());
            }
            
            debug!("MCP tools changed (version {} -> {}), refreshing", self.last_mcp_tools_version, current_version);
            
            // Clear existing MCP tools
            let mut tools = self.tools.write().await;
            tools.retain(|name, _| !name.starts_with("mcp_"));
            
            // Add bash tool with security
            let security_manager = self.bash_security_manager.clone();
            tools.insert("bash".to_string(), Tool {
                name: "bash".to_string(),
                description: "Execute shell commands and return the output (with security)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute"
                        }
                    },
                    "required": ["command"]
                }),
                handler: Box::new(move |call: ToolCall| {
                    let security_manager = security_manager.clone();
                    Box::pin(async move {
                        // Create a wrapper function that handles the mutable reference
                        async fn bash_wrapper(
                            call: ToolCall,
                            security_manager: Arc<RwLock<BashSecurityManager>>,
                        ) -> Result<ToolResult> {
                            let mut manager = security_manager.write().await;
                            bash(&call, &mut *manager).await
                        }
                        
                        bash_wrapper(call, security_manager).await
                    })
                }),
            });
            
            // Get all MCP tools
            match mcp_manager.get_all_tools().await {
                Ok(mcp_tools) => {
                    for (server_name, mcp_tool) in mcp_tools {
                        let tool = crate::tools::create_mcp_tool(&server_name, mcp_tool, mcp_manager.clone());
                        tools.insert(tool.name.clone(), tool);
                    }
                    self.last_mcp_tools_version = current_version;
                    info!("Refreshed {} MCP tools", tools.iter().filter(|(name, _)| name.starts_with("mcp_")).count());
                }
                Err(e) => {
                    warn!("Failed to refresh MCP tools: {}", e);
                }
            }
        }
        Ok(())
    }

    /// Force refresh MCP tools regardless of version
    pub async fn force_refresh_mcp_tools(&mut self) -> Result<()> {
        if let Some(_mcp_manager) = &self.mcp_manager {
            // Reset version to force refresh
            self.last_mcp_tools_version = 0;
            self.refresh_mcp_tools().await
        } else {
            // Even without MCP manager, ensure bash tool is available
            let mut tools = self.tools.write().await;
            if !tools.contains_key("bash") {
                let security_manager = self.bash_security_manager.clone();
                tools.insert("bash".to_string(), Tool {
                    name: "bash".to_string(),
                    description: "Execute shell commands and return the output (with security)".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "Shell command to execute"
                            }
                        },
                        "required": ["command"]
                    }),
                    handler: Box::new(move |call: ToolCall| {
                        let security_manager = security_manager.clone();
                        Box::pin(async move {
                            // Create a wrapper function that handles the mutable reference
                            async fn bash_wrapper(
                                call: ToolCall,
                                security_manager: Arc<RwLock<BashSecurityManager>>,
                            ) -> Result<ToolResult> {
                                let mut manager = security_manager.write().await;
                                bash(&call, &mut *manager).await
                            }
                            
                            bash_wrapper(call, security_manager).await
                        })
                    }),
                });
            }
            Ok(())
        }
    }

    /// Set the system prompt for the conversation
    pub fn set_system_prompt(&mut self, system_prompt: String) {
        self.system_prompt = Some(system_prompt);
    }

    /// Get the current system prompt
    pub fn get_system_prompt(&self) -> Option<&String> {
        self.system_prompt.as_ref()
    }

    /// Add a file as context to the conversation
    pub async fn add_context_file(&mut self, file_path: &str) -> Result<()> {
        use tokio::fs;
        use path_absolutize::*;
        use shellexpand;

        let expanded_path = shellexpand::tilde(file_path);
        let absolute_path = Path::new(&*expanded_path).absolutize()?;

        match fs::read_to_string(&absolute_path).await {
            Ok(content) => {
                let context_message = format!(
                    "Context from file '{}':\n\n```\n{}\n```",
                    absolute_path.display(),
                    content
                );
                
                self.conversation.push(Message {
                    role: "user".to_string(),
                    content: vec![ContentBlock::text(context_message)],
                });
                
                debug!("Added context file: {}", absolute_path.display());
                Ok(())
            }
            Err(e) => {
                anyhow::bail!("Failed to read file '{}': {}", absolute_path.display(), e);
            }
        }
    }

    /// Extract file paths from message using @path syntax
    pub fn extract_context_files(&self, message: &str) -> Vec<String> {
        let re = Regex::new(r"@([^\s@]+)").unwrap();
        re.captures_iter(message)
            .map(|cap| cap[1].to_string())
            .collect()
    }

    /// Remove @file syntax from message and return cleaned message
    pub fn clean_message(&self, message: &str) -> String {
        let re = Regex::new(r"@[^\s@]+").unwrap();
        re.replace_all(message, "").trim().to_string()
    }

    pub async fn process_message(&mut self, message: &str) -> Result<String> {
        self.process_message_with_stream(message, None::<fn(String)>).await
    }

    pub async fn process_message_with_stream<F>(
        &mut self,
        message: &str,
        on_stream_content: Option<F>
    ) -> Result<String>
    where
        F: Fn(String) + Send + Sync + 'static + Clone,
    {
        // Log incoming user message
        debug!("Processing user message: {}", message);
        debug!("Current conversation length: {}", self.conversation.len());

        // Extract and add context files from @ syntax
        let context_files = self.extract_context_files(message);
        for file_path in &context_files {
            debug!("Auto-adding context file from @ syntax: {}", file_path);
            match self.add_context_file(file_path).await {
                Ok(_) => println!("{} Added context file: {}", "✓".green(), file_path),
                Err(e) => eprintln!("{} Failed to add context file '{}': {}", "✗".red(), file_path, e),
            }
        }

        // Clean message by removing @file syntax
        let cleaned_message = self.clean_message(message);

        // If message is empty after cleaning (only contained @file references), 
        // return early without making an API call
        if cleaned_message.trim().is_empty() {
            debug!("Message only contained @file references, not making API call");
            return Ok("".to_string());
        }

        // Add cleaned user message to conversation
        self.conversation.push(Message {
            role: "user".to_string(),
            content: vec![ContentBlock::text(cleaned_message)],
        });

        // Refresh MCP tools before processing the message (only if they have changed)
        if let Err(e) = self.refresh_mcp_tools().await {
            warn!("Failed to refresh MCP tools: {}", e);
            error!("MCP tools may not be available - some tool calls might fail");
        }
        let mut final_response = String::new();
        let max_iterations = 500;
        let mut iteration = 0;

        while iteration < max_iterations {
            iteration += 1;

            // Get available tools
            
            let available_tools: Vec<Tool> = {
                let tools = self.tools.read().await;
                tools.values().cloned().collect()
            };

            // Call Anthropic API with streaming if callback provided
            let response = if let Some(ref on_content) = on_stream_content {
                self.client.create_message_stream(
                    &self.model,
                    self.conversation.clone(),
                    &available_tools,
                    4096,
                    0.7,
                    self.system_prompt.as_ref(),
                    on_content.clone(),
                ).await?
            } else {
                self.client.create_message(
                    &self.model,
                    self.conversation.clone(),
                    &available_tools,
                    4096,
                    0.7,
                    self.system_prompt.as_ref(),
                ).await?
            };

            // Track token usage
            if let Some(usage) = &response.usage {
                self.token_usage.add_usage(usage);
                debug!("Updated token usage - Total: {} (Input: {}, Output: {})",
                      self.token_usage.total_tokens(),
                      self.token_usage.total_input_tokens,
                      self.token_usage.total_output_tokens);
            }

            // Extract and output the text response from this API call
            let response_content = self.client.create_response_content(&response.content);
            if !response_content.is_empty() {
                if on_stream_content.is_none() {
                    // Only print if not streaming (streaming handles its own output)
                    println!("{}", response_content);
                }
                // Always accumulate response content, even in streaming mode
                final_response = response_content.clone();
            }

            // Check for tool calls
            let tool_calls = self.client.convert_tool_calls(&response.content);

            if tool_calls.is_empty() {
                // No tool calls, return the accumulated text response
                // final_response was already set above during response processing
                if final_response.is_empty() {
                    final_response = "(No response received from assistant)".to_string();
                }
                  break;
            }

  
            // Execute tool calls with pretty output
            let tool_results: Vec<ToolResult> = {
                let mut results = Vec::new();
                for call in &tool_calls {
                    debug!("Executing tool: {} with ID: {}", call.name, call.id);
                    
                    // Create pretty display for this tool call
                    let mut display: Box<dyn ToolDisplay> = if should_use_pretty_output() {
                        Box::new(ToolCallDisplay::new(&call.name))
                    } else {
                        Box::new(SimpleToolDisplay::new(&call.name))
                    };
                    
                    // Show tool call details
                    if should_use_pretty_output() {
                        display.show_call_details(&call.arguments);
                    }
                    
                    // Special handling for MCP tools
                    if call.name.starts_with("mcp_") {
                        if let Some(mcp_manager) = &self.mcp_manager {
                            // Extract server name and tool name from the call
                            let parts: Vec<&str> = call.name.splitn(3, '_').collect();
                            if parts.len() >= 3 {
                                let server_name = parts[1];
                                let tool_name = parts[2..].join("_");
                                
                                match mcp_manager.call_tool(server_name, &tool_name, Some(call.arguments.clone())).await {
                                    Ok(result) => {
                                        debug!("MCP tool '{}' executed successfully", call.name);
                                        let result_content = serde_json::to_string_pretty(&result)
                                            .unwrap_or_else(|_| "Invalid JSON result".to_string());
                                        
                                        if should_use_pretty_output() {
                                            display.complete_success(&result_content);
                                        } else {
                                            display.complete_success(&result_content);
                                        }
                                        
                                        results.push(ToolResult {
                                            tool_use_id: call.id.clone(),
                                            content: result_content,
                                            is_error: false,
                                        });
                                    }
                                    Err(e) => {
                                        error!("Error executing MCP tool '{}': {}", call.name, e);
                                        error!("MCP server '{}' may have encountered an error or is unavailable", server_name);

                                        // Provide detailed error information
                                        let error_content = format!(
                                            "MCP tool call failed: {}. \n\
                                            Tool: {}\n\
                                            Server: {}\n\
                                            Arguments: {}\n\
                                            Please check:\n\
                                            1. MCP server '{}' is running\n\
                                            2. Server is responsive\n\
                                            3. Tool arguments are correct\n\
                                            4. Server has proper permissions",
                                            e, call.name, server_name,
                                            serde_json::to_string_pretty(&call.arguments).unwrap_or_else(|_| "Invalid JSON".to_string()),
                                            server_name
                                        );

                                        if should_use_pretty_output() {
                                            display.complete_error(&error_content);
                                        } else {
                                            display.complete_error(&error_content);
                                        }

                                        results.push(ToolResult {
                                            tool_use_id: call.id.clone(),
                                            content: error_content,
                                            is_error: true,
                                        });
                                    }
                                }
                            } else {
                                let error_content = format!("Invalid MCP tool name format: {}", call.name);
                                if should_use_pretty_output() {
                                    display.complete_error(&error_content);
                                } else {
                                    display.complete_error(&error_content);
                                }
                                
                                results.push(ToolResult {
                                    tool_use_id: call.id.clone(),
                                    content: error_content,
                                    is_error: true,
                                });
                            }
                        } else {
                            let error_content = "MCP manager not available. MCP tools cannot be executed without proper initialization.";
                            if should_use_pretty_output() {
                                display.complete_error(error_content);
                            } else {
                                display.complete_error(error_content);
                            }
                            
                            results.push(ToolResult {
                                tool_use_id: call.id.clone(),
                                content: error_content.to_string(),
                                is_error: true,
                            });
                        }
                    } else if call.name == "bash" {
                        // Handle bash tool with security
                        let security_manager = self.bash_security_manager.clone();
                        let call_clone = call.clone();
                        
                        // We need to get a mutable reference to the security manager
                        let mut manager = security_manager.write().await;
                        match bash(&call_clone, &mut *manager).await {
                            Ok(result) => {
                                debug!("Bash tool executed successfully");
                                
                                // Check if permissions were updated and save to config
                                if result.content.contains("💾 Note: This command has been added to your allowlist") {
                                    info!("Permissions updated, scheduling save to config file");
                                    // Get the current security settings to save in background
                                    let security_manager_clone = self.bash_security_manager.clone();
                                    tokio::spawn(async move {
                                        // Load existing config to preserve other settings
                                        match crate::config::Config::load(None).await {
                                            Ok(mut existing_config) => {
                                                // Get current security settings
                                                let current_security = security_manager_clone.read().await;
                                                let updated_security = current_security.get_security().clone();
                                                drop(current_security);
                                                
                                                // Update only the bash_security settings
                                                existing_config.bash_security = updated_security;
                                                
                                                // Save the updated config
                                                match existing_config.save(None).await {
                                                    Ok(_) => {
                                                        info!("Updated bash security settings saved to config (background)");
                                                    }
                                                    Err(e) => {
                                                        warn!("Failed to save bash security settings (background): {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                warn!("Failed to load config for saving permissions (background): {}", e);
                                            }
                                        }
                                    });
                                }
                                
                                if result.is_error {
                                    if should_use_pretty_output() {
                                        display.complete_error(&result.content);
                                    } else {
                                        display.complete_error(&result.content);
                                    }
                                } else {
                                    if should_use_pretty_output() {
                                        display.complete_success(&result.content);
                                    } else {
                                        display.complete_success(&result.content);
                                    }
                                }
                                results.push(result);
                            },
                            Err(e) => {
                                error!("Error executing bash tool: {}", e);
                                let error_content = format!("Error executing bash tool: {}", e);
                                if should_use_pretty_output() {
                                    display.complete_error(&error_content);
                                } else {
                                    display.complete_error(&error_content);
                                }
                                results.push(ToolResult {
                                    tool_use_id: call.id.clone(),
                                    content: error_content,
                                    is_error: true,
                                });
                            }
                        };
                        drop(manager); // Explicitly drop the lock guard
                    } else if let Some(tool) = {
                        let tools = self.tools.read().await;
                        tools.get(&call.name).cloned()
                    } {
                        match (tool.handler)(call.clone()).await {
                            Ok(result) => {
                                debug!("Tool '{}' executed successfully", call.name);
                                if result.is_error {
                                    if should_use_pretty_output() {
                                        display.complete_error(&result.content);
                                    } else {
                                        display.complete_error(&result.content);
                                    }
                                } else {
                                    if should_use_pretty_output() {
                                        display.complete_success(&result.content);
                                    } else {
                                        display.complete_success(&result.content);
                                    }
                                }
                                results.push(result);
                            },
                            Err(e) => {
                                error!("Error executing tool '{}': {}", call.name, e);
                                let error_content = format!("Error executing tool '{}': {}", call.name, e);
                                if should_use_pretty_output() {
                                    display.complete_error(&error_content);
                                } else {
                                    display.complete_error(&error_content);
                                }
                                results.push(ToolResult {
                                    tool_use_id: call.id.clone(),
                                    content: error_content,
                                    is_error: true,
                                });
                            }
                        }
                    } else {
                        error!("Unknown tool: {}", call.name);
                        let error_content = format!("Unknown tool: {}", call.name);
                        if should_use_pretty_output() {
                            display.complete_error(&error_content);
                        } else {
                            display.complete_error(&error_content);
                        }
                        results.push(ToolResult {
                            tool_use_id: call.id.clone(),
                            content: error_content,
                            is_error: true,
                        });
                    }
                }
                results
            };
            let _tool_results_count = tool_results.len();

            // Add assistant's tool use message to conversation
            let assistant_content: Vec<ContentBlock> = response.content
                .into_iter()
                .collect();

            self.conversation.push(Message {
                role: "assistant".to_string(),
                content: assistant_content,
            });

            // Add tool results to conversation
            for result in tool_results {
                self.conversation.push(Message {
                    role: "user".to_string(),
                    content: vec![ContentBlock::tool_result(
                        result.tool_use_id,
                        result.content,
                        Some(result.is_error),
                    )],
                });
            }
        }

        if iteration >= max_iterations {
            final_response.push_str("\n\n(Note: Maximum tool iterations reached)");
        }

        // Add final assistant response to conversation if it exists
        if !final_response.is_empty() {
            self.conversation.push(Message {
                role: "assistant".to_string(),
                content: vec![ContentBlock::text(final_response.clone())],
            });
        }
        debug!("Final response generated ({} chars)", final_response.len());
        Ok(final_response)
    }

    pub fn clear_conversation(&mut self) {
        self.conversation.clear();
    }

    pub fn get_conversation_length(&self) -> usize {
        self.conversation.len()
    }

    pub fn get_token_usage(&self) -> &TokenUsage {
        &self.token_usage
    }

    pub fn reset_token_usage(&mut self) {
        self.token_usage.reset();
    }

    /// Display the current conversation context
    pub fn display_context(&self) {
        println!("{}", "📝 Current Conversation Context".cyan().bold());
        println!("{}", "─".repeat(50).dimmed());
        println!();

        // Display system prompt if set
        if let Some(system_prompt) = &self.system_prompt {
            println!("{}", "System Prompt:".green().bold());
            println!("  {}", system_prompt);
            println!();
        }

        if self.conversation.is_empty() {
            println!("{}", "No context yet. Start a conversation to see context here.".dimmed());
            println!();
            return;
        }

        for (i, message) in self.conversation.iter().enumerate() {
            let role_color = match message.role.as_str() {
                "user" => "blue",
                "assistant" => "green",
                _ => "yellow",
            };

            println!("{} {}: {}",
                     format!("[{}]", i + 1).dimmed(),
                     format!("{}", message.role.to_uppercase()).color(role_color),
                     format!("({} content blocks)", message.content.len()).dimmed()
            );
            
            for (j, block) in message.content.iter().enumerate() {
                match block.block_type.as_str() {
                    "text" => {
                        if let Some(ref text) = block.text {
                            // Show first 100 characters of text content
                            let preview = if text.len() > 100 {
                                // Use safe character boundary slicing
                                let safe_end = text.char_indices().nth(100).map(|(idx, _)| idx).unwrap_or(text.len());
                                format!("{}...", &text[..safe_end])
                            } else {
                                text.clone()
                            };
                            println!("  {} {}: {}",
                                     format!("└─ Block {}", j + 1).dimmed(),
                                     "Text".green(),
                                     preview.replace('\n', " ")
                            );
                        }
                    },
                    "tool_use" => {
                        if let (Some(ref id), Some(ref name), Some(ref input)) = (&block.id, &block.name, &block.input) {
                            println!("  {} {}: {} ({})",
                                     format!("└─ Block {}", j + 1).dimmed(),
                                     "Tool Use".yellow(),
                                     name,
                                     id
                            );
                            // Safely handle the input as a string
                            let input_str = match input {
                                Value::String(s) => s.clone(),
                                _ => serde_json::to_string_pretty(input)
                                    .unwrap_or_else(|_| "Invalid JSON".to_string()),
                            };
                            let preview = if input_str.len() > 80 {
                                // Use safe character boundary slicing
                            let safe_end = input_str.char_indices().nth(80).map(|(idx, _)| idx).unwrap_or(input_str.len());
                            format!("{}...", &input_str[..safe_end])
                            } else {
                                input_str
                            };
                            println!("    {} {}", "Input:".dimmed(), preview);
                        }
                    },
                    "tool_result" => {
                        if let (Some(ref tool_use_id), Some(ref content), ref is_error) = (&block.tool_use_id, &block.content, &block.is_error) {
                            let result_type = if is_error.unwrap_or(false) { "Error".red() } else { "Result".green() };
                            println!("  {} {}: {} ({})",
                                     format!("└─ Block {}", j + 1).dimmed(),
                                     result_type,
                                     tool_use_id,
                                     format!("{} chars", content.len()).dimmed()
                            );
                            let preview = if content.len() > 80 {
                                // Use safe character boundary slicing
                                let safe_end = content.char_indices().nth(80).map(|(idx, _)| idx).unwrap_or(content.len());
                                format!("{}...", &content[..safe_end])
                            } else {
                                content.clone()
                            };
                            println!("    {} {}", "Content:".dimmed(), preview.replace('\n', " "));
                        }
                    },
                    _ => {
                        println!("  {} {}",
                                 format!("└─ Block {}", j + 1).dimmed(),
                                 "Unknown".red()
                        );
                    }
                }
            }
            println!();
        }
        
        println!("{}", "─".repeat(50).dimmed());
        println!("{}: {} messages, {} total content blocks", 
                 "Summary".bold(),
                 self.conversation.len(),
                 self.conversation.iter().map(|m| m.content.len()).sum::<usize>()
        );
        println!();
    }

    /// Get the bash security manager
    pub fn get_bash_security_manager(&self) -> Arc<RwLock<BashSecurityManager>> {
        self.bash_security_manager.clone()
    }

    /// Get current configuration (for saving permissions)
    pub async fn get_config_for_save(&self) -> crate::config::Config {
        use crate::config::Config;
        
        // Get current security settings from agent
        let security_manager = self.bash_security_manager.read().await;
        let current_security = security_manager.get_security().clone();
        
        // Create a basic config with the current bash security settings
        Config {
            api_key: "".to_string(), // Don't save API key from this method
            base_url: "".to_string(),
            default_model: self.model.clone(),
            max_tokens: 4096,
            temperature: 0.7,
            default_system_prompt: self.system_prompt.clone(),
            bash_security: current_security,
            mcp: crate::config::McpConfig::default(),
        }
    }

    /// Save updated bash security settings to config file
    async fn save_bash_security_to_config(&self) -> Result<()> {
        use crate::config::Config;
        
        // Load existing config to preserve other settings
        let mut existing_config = Config::load(None).await?;
        
        // Get current security settings from agent
        let security_manager = self.bash_security_manager.read().await;
        let current_security = security_manager.get_security().clone();
        drop(security_manager);
        
        // Update only the bash_security settings
        existing_config.bash_security = current_security;
        
        // Save the updated config
        match existing_config.save(None).await {
            Ok(_) => {
                info!("Updated bash security settings saved to config");
                Ok(())
            }
            Err(e) => {
                error!("Failed to save bash security settings: {}", e);
                Err(e)
            }
        }
    }

    /// Clear conversation but keep AGENTS.md if it exists in context
    pub async fn clear_conversation_keep_agents_md(&mut self) -> Result<()> {
        use std::path::Path;
        
        // Check if AGENTS.md exists in the current directory
        let agents_md_path = Path::new("AGENTS.md");
        let has_agents_md = agents_md_path.exists();
        
        if has_agents_md {
            debug!("Clearing conversation but keeping AGENTS.md context");
            // Clear the conversation
            self.conversation.clear();
            // Re-add AGENTS.md
            self.add_context_file("AGENTS.md").await?;
        } else {
            debug!("Clearing conversation (no AGENTS.md found)");
            self.conversation.clear();
        }
        
        Ok(())
    }
}