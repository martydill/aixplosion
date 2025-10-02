use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use anyhow::Result;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;
use path_absolutize::*;
use shellexpand;

pub type AsyncToolHandler = Box<dyn Fn(&ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send + '_>> + Send + Sync>;

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

    let tool_use_id = call.id.clone();

    let expanded_path = shellexpand::tilde(path);
    let absolute_path = Path::new(&*expanded_path).absolutize()?;

    // Read existing file
    match fs::read_to_string(&absolute_path).await {
        Ok(mut content) => {
            if !content.contains(old_text) {
                return Ok(ToolResult {
                    tool_use_id,
                    content: format!("Text not found in file '{}': {}", absolute_path.display(), old_text),
                    is_error: true,
                });
            }

            content = content.replace(old_text, new_text);

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

// Wrapper functions to convert async functions to the expected handler signature
fn list_directory_sync(call: &ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send + '_>> {
    Box::pin(list_directory(call))
}

fn read_file_sync(call: &ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send + '_>> {
    Box::pin(read_file(call))
}

fn write_file_sync(call: &ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send + '_>> {
    Box::pin(write_file(call))
}

fn edit_file_sync(call: &ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send + '_>> {
    Box::pin(edit_file(call))
}

fn delete_file_sync(call: &ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send + '_>> {
    Box::pin(delete_file(call))
}

fn create_directory_sync(call: &ToolCall) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolResult>> + Send + '_>> {
    Box::pin(create_directory(call))
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
    ]
}