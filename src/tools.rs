use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use anyhow::Result;
use std::path::Path;
use std::process::Command;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::task;
use path_absolutize::*;
use shellexpand;
use log::debug;
use std::sync::Arc;
use crate::mcp::{McpManager, McpTool};

pub type AsyncToolHandler = Box<dyn Fn(ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send>> + Send + Sync>;

pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub handler: AsyncToolHandler,
}

impl Clone for Tool {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            // Note: We can't clone the function pointer directly, so we recreate it
            handler: self.recreate_handler(),
        }
    }
}

impl Tool {
    fn recreate_handler(&self) -> AsyncToolHandler {
        match self.name.as_str() {
            "list_directory" => Box::new(list_directory_sync),
            "read_file" => Box::new(read_file_sync),
            "write_file" => Box::new(write_file_sync),
            "edit_file" => Box::new(edit_file_sync),
            "delete_file" => Box::new(delete_file_sync),
            "create_directory" => Box::new(create_directory_sync),
            "bash" => Box::new(bash_sync),
            _ if self.name.starts_with("mcp_") => {
                // This is an MCP tool - we need to handle this differently
                // The issue is that we can't recreate MCP handlers without the MCP manager
                // So we'll create a placeholder that indicates the issue
                Box::new(|call| Box::pin(async move {
                    Err(anyhow::anyhow!("MCP tool '{}' cannot be recreated without proper MCP manager context. This suggests there's an issue with how MCP tools are being cloned or moved.", call.name))
                }))
            }
            _ => panic!("Unknown tool: {}", self.name),
        }
    }
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("handler", &"<async_handler>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

// Built-in tools
pub async fn list_directory(call: &ToolCall) -> Result<ToolResult> {
    let path = call.arguments.get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    debug!("TOOL CALL: list_directory('{}')", path);

    let tool_use_id = call.id.clone();

    let expanded_path = shellexpand::tilde(path);
    let absolute_path = Path::new(&*expanded_path).absolutize()?;

    match fs::read_dir(&absolute_path).await {
        Ok(mut entries) => {
            let mut result = String::new();
            result.push_str(&format!("Contents of '{}':\n", absolute_path.display()));

            let mut items = Vec::new();
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");

                if path.is_dir() {
                    items.push(format!("📁 {}/", name));
                } else {
                    let size = if let Ok(metadata) = fs::metadata(&path).await {
                        metadata.len()
                    } else {
                        0
                    };
                    items.push(format!("📄 {} ({} bytes)", name, size));
                }
            }

            items.sort();
            result.push_str(&items.join("\n"));

            Ok(ToolResult {
                tool_use_id,
                content: result,
                is_error: false,
            })
        }
        Err(e) => Ok(ToolResult {
            tool_use_id,
            content: format!("Error reading directory '{}': {}", absolute_path.display(), e),
            is_error: true,
        })
    }
}

pub async fn read_file(call: &ToolCall) -> Result<ToolResult> {
    let path = call.arguments.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;

    debug!("TOOL CALL: read_file('{}')", path);

    let tool_use_id = call.id.clone();

    let expanded_path = shellexpand::tilde(path);
    let absolute_path = Path::new(&*expanded_path).absolutize()?;

    match fs::File::open(&absolute_path).await {
        Ok(mut file) => {
            let mut contents = Vec::new();
            match file.read_to_end(&mut contents).await {
                Ok(_) => {
                    let content = String::from_utf8_lossy(&contents);
                    Ok(ToolResult {
                        tool_use_id,
                        content: format!("File: {}\n\n{}", absolute_path.display(), content),
                        is_error: false,
                    })
                }
                Err(e) => Ok(ToolResult {
                    tool_use_id,
                    content: format!("Error reading file '{}': {}", absolute_path.display(), e),
                    is_error: true,
                })
            }
        }
        Err(e) => Ok(ToolResult {
            tool_use_id,
            content: format!("Error opening file '{}': {}", absolute_path.display(), e),
            is_error: true,
        })
    }
}

pub async fn write_file(call: &ToolCall) -> Result<ToolResult> {
    let path = call.arguments.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;

    let content = call.arguments.get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;

    debug!("TOOL CALL: write_file('{}', {} bytes)", path, content.len());

    let tool_use_id = call.id.clone();

    let expanded_path = shellexpand::tilde(path);
    let absolute_path = Path::new(&*expanded_path).absolutize()?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = absolute_path.parent() {
        if let Err(e) = fs::create_dir_all(parent).await {
            return Ok(ToolResult {
                tool_use_id,
                content: format!("Error creating parent directory: {}", e),
                is_error: true,
            });
        }
    }

    match fs::write(&absolute_path, content).await {
        Ok(_) => Ok(ToolResult {
            tool_use_id,
            content: format!("Successfully wrote to file: {}", absolute_path.display()),
            is_error: false,
        }),
        Err(e) => Ok(ToolResult {
            tool_use_id,
            content: format!("Error writing to file '{}': {}", absolute_path.display(), e),
            is_error: true,
        })
    }
}

// Detect the line ending type used in the content
fn detect_line_ending(content: &str) -> &str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

// Convert text to use the specified line ending type
fn normalize_line_endings(text: &str, line_ending: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', line_ending)
}

pub async fn edit_file(call: &ToolCall) -> Result<ToolResult> {
    let path = call.arguments.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;

    let old_text = call.arguments.get("old_text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'old_text' argument"))?;

    let new_text = call.arguments.get("new_text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'new_text' argument"))?;

    debug!("TOOL CALL: edit_file('{}', {} -> {} bytes)", path, old_text.len(), new_text.len());

    let tool_use_id = call.id.clone();

    let expanded_path = shellexpand::tilde(path);
    let absolute_path = Path::new(&*expanded_path).absolutize()?;

    // Read existing file
    match fs::read_to_string(&absolute_path).await {
        Ok(mut content) => {
            // Detect the line ending type used in the file
            let file_line_ending = detect_line_ending(&content);

            // Normalize old_text to use the file's line endings for matching
            let normalized_old_text = normalize_line_endings(old_text, file_line_ending);

            if !content.contains(&normalized_old_text) {
                return Ok(ToolResult {
                    tool_use_id,
                    content: format!("Text not found in file '{}': {}", absolute_path.display(), normalized_old_text),
                    is_error: true,
                });
            }

            // Normalize new_text to use the file's line endings
            let normalized_new_text = normalize_line_endings(new_text, file_line_ending);

            content = content.replace(&normalized_old_text, &normalized_new_text);

            match fs::write(&absolute_path, content).await {
                Ok(_) => Ok(ToolResult {
                    tool_use_id,
                    content: format!("Successfully edited file: {}", absolute_path.display()),
                    is_error: false,
                }),
                Err(e) => Ok(ToolResult {
                    tool_use_id,
                    content: format!("Error writing to file '{}': {}", absolute_path.display(), e),
                    is_error: true,
                })
            }
        }
        Err(e) => Ok(ToolResult {
            tool_use_id,
            content: format!("Error reading file '{}': {}", absolute_path.display(), e),
            is_error: true,
        })
    }
}

pub async fn delete_file(call: &ToolCall) -> Result<ToolResult> {
    let path = call.arguments.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;

    debug!("TOOL CALL: delete_file('{}')", path);

    let tool_use_id = call.id.clone();

    let expanded_path = shellexpand::tilde(path);
    let absolute_path = Path::new(&*expanded_path).absolutize()?;

    match fs::metadata(&absolute_path).await {
        Ok(metadata) => {
            if metadata.is_dir() {
                match fs::remove_dir_all(&absolute_path).await {
                    Ok(_) => Ok(ToolResult {
                        tool_use_id,
                        content: format!("Successfully deleted directory: {}", absolute_path.display()),
                        is_error: false,
                    }),
                    Err(e) => Ok(ToolResult {
                        tool_use_id,
                        content: format!("Error deleting directory '{}': {}", absolute_path.display(), e),
                        is_error: true,
                    })
                }
            } else {
                match fs::remove_file(&absolute_path).await {
                    Ok(_) => Ok(ToolResult {
                        tool_use_id,
                        content: format!("Successfully deleted file: {}", absolute_path.display()),
                        is_error: false,
                    }),
                    Err(e) => Ok(ToolResult {
                        tool_use_id,
                        content: format!("Error deleting file '{}': {}", absolute_path.display(), e),
                        is_error: true,
                    })
                }
            }
        }
        Err(e) => Ok(ToolResult {
            tool_use_id,
            content: format!("Error accessing path '{}': {}", absolute_path.display(), e),
            is_error: true,
        })
    }
}

pub async fn create_directory(call: &ToolCall) -> Result<ToolResult> {
    let path = call.arguments.get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;

    debug!("TOOL CALL: create_directory('{}')", path);

    let tool_use_id = call.id.clone();

    let expanded_path = shellexpand::tilde(path);
    let absolute_path = Path::new(&*expanded_path).absolutize()?;

    match fs::create_dir_all(&absolute_path).await {
        Ok(_) => Ok(ToolResult {
            tool_use_id,
            content: format!("Successfully created directory: {}", absolute_path.display()),
            is_error: false,
        }),
        Err(e) => Ok(ToolResult {
            tool_use_id,
            content: format!("Error creating directory '{}': {}", absolute_path.display(), e),
            is_error: true,
        })
    }
}

pub async fn bash(call: &ToolCall) -> Result<ToolResult> {
    let command = call.arguments.get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?
        .to_string();

    debug!("TOOL CALL: bash('{}')", command);

    let tool_use_id = call.id.clone();

    // Execute the command using tokio::task to spawn blocking operation
    let command_clone = command.clone();
    match task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/C", &command_clone])
                .output()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Command::new("bash")
                .args(["-c", &command_clone])
                .output()
        }
    }).await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            let content = if !stderr.is_empty() {
                format!("Exit code: {}\nStdout:\n{}\nStderr:\n{}", 
                    output.status.code().unwrap_or(-1), stdout, stderr)
            } else {
                format!("Exit code: {}\nOutput:\n{}", 
                    output.status.code().unwrap_or(-1), stdout)
            };

            Ok(ToolResult {
                tool_use_id,
                content,
                is_error: !output.status.success(),
            })
        }
        Ok(Err(e)) => Ok(ToolResult {
            tool_use_id,
            content: format!("Error executing command '{}': {}", command, e),
            is_error: true,
        }),
        Err(e) => Ok(ToolResult {
            tool_use_id,
            content: format!("Task join error: {}", e),
            is_error: true,
        })
    }
}

// Wrapper functions to convert async functions to the expected handler signature
fn list_directory_sync(call: ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send>> {
    Box::pin(async move {
        list_directory(&call).await
    })
}

fn read_file_sync(call: ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send>> {
    Box::pin(async move {
        read_file(&call).await
    })
}

fn write_file_sync(call: ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send>> {
    Box::pin(async move {
        write_file(&call).await
    })
}

fn edit_file_sync(call: ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send>> {
    Box::pin(async move {
        edit_file(&call).await
    })
}

fn delete_file_sync(call: ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send>> {
    Box::pin(async move {
        delete_file(&call).await
    })
}

fn create_directory_sync(call: ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send>> {
    Box::pin(async move {
        create_directory(&call).await
    })
}

fn bash_sync(call: ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send>> {
    Box::pin(async move {
        bash(&call).await
    })
}

pub fn get_builtin_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "list_directory".to_string(),
            description: "List contents of a directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the directory to list (default: current directory)"
                    }
                }
            }),
            handler: Box::new(list_directory_sync),
        },
        Tool {
            name: "read_file".to_string(),
            description: "Read the contents of a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    }
                },
                "required": ["path"]
            }),
            handler: Box::new(read_file_sync),
        },
        Tool {
            name: "write_file".to_string(),
            description: "Write content to a file (creates file if it doesn't exist)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
            handler: Box::new(write_file_sync),
        },
        Tool {
            name: "edit_file".to_string(),
            description: "Replace specific text in a file with new text".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to edit"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Text to replace"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "New text to replace with"
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }),
            handler: Box::new(edit_file_sync),
        },
        Tool {
            name: "delete_file".to_string(),
            description: "Delete a file or directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file or directory to delete"
                    }
                },
                "required": ["path"]
            }),
            handler: Box::new(delete_file_sync),
        },
        Tool {
            name: "create_directory".to_string(),
            description: "Create a directory (and parent directories if needed)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the directory to create"
                    }
                },
                "required": ["path"]
            }),
            handler: Box::new(create_directory_sync),
        },
        Tool {
            name: "bash".to_string(),
            description: "Execute shell commands and return the output".to_string(),
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
            handler: Box::new(bash_sync),
        },
    ]
}

pub fn create_mcp_tool(server_name: &str, mcp_tool: McpTool, mcp_manager: Arc<McpManager>) -> Tool {
    let tool_name = format!("mcp_{}_{}", server_name, mcp_tool.name);
    let description = mcp_tool.description.unwrap_or_else(|| format!("MCP tool from server: {}", server_name));
    let server_name_owned = server_name.to_string();
    let mcp_manager_clone = mcp_manager.clone();
    let tool_name_original = mcp_tool.name.clone();

    // Ensure input_schema is a valid JSON object (not null)
    let input_schema = if mcp_tool.input_schema.is_null() {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    } else {
        mcp_tool.input_schema
    };

    Tool {
        name: tool_name.clone(),
        description: format!("{} (MCP: {})", description, server_name),
        input_schema,
        handler: Box::new(move |call: ToolCall| {
            let server_name = server_name_owned.clone();
            let mcp_manager = mcp_manager_clone.clone();
            let tool_name_original = tool_name_original.clone();

            Box::pin(async move {
                // Extract the actual tool name from the mcp_ prefix
                let actual_tool_name = call.name.strip_prefix(&format!("mcp_{}_", server_name))
                    .unwrap_or(&tool_name_original);

                match mcp_manager.call_tool(&server_name, actual_tool_name, Some(call.arguments)).await {
                    Ok(result) => {
                        Ok(ToolResult {
                            tool_use_id: call.id,
                            content: serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Invalid JSON result".to_string()),
                            is_error: false,
                        })
                    }
                    Err(e) => {
                        Ok(ToolResult {
                            tool_use_id: call.id,
                            content: format!("MCP tool call failed: {}", e),
                            is_error: true,
                        })
                    }
                }
            })
        }),
    }
}