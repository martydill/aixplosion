use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use log::{debug, info, error};

use crate::config::Config;
use crate::anthropic::{AnthropicClient, Message, ContentBlock, Usage};
use crate::tools::{Tool, ToolResult, get_builtin_tools};

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
    tools: HashMap<String, Tool>,
    conversation: Vec<Message>,
    token_usage: TokenUsage,
}

impl Agent {
    pub fn new(config: Config, model: String) -> Self {
        let client = AnthropicClient::new(config.api_key, config.base_url);
        let tools = get_builtin_tools()
            .into_iter()
            .map(|tool| (tool.name.clone(), tool))
            .collect();

        Self {
            client,
            model,
            tools,
            conversation: Vec::new(),
            token_usage: TokenUsage::new(),
        }
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
                
                info!("Added context file: {}", absolute_path.display());
                Ok(())
            }
            Err(e) => {
                anyhow::bail!("Failed to read file '{}': {}", absolute_path.display(), e);
            }
        }
    }

    pub async fn process_message(&mut self, message: &str) -> Result<String> {
        // Log incoming user message
        info!("Processing user message: {}", message);
        info!("Current conversation length: {}", self.conversation.len());

        // Add user message to conversation
        self.conversation.push(Message {
            role: "user".to_string(),
            content: vec![ContentBlock::text(message.to_string())],
        });

        let mut final_response = String::new();
        let max_iterations = 500;
        let mut iteration = 0;

        while iteration < max_iterations {
            iteration += 1;

            // Get available tools
            let available_tools: Vec<Tool> = self.tools.values().cloned().collect();

            // Call Anthropic API
            let response = self.client.create_message(
                &self.model,
                self.conversation.clone(),
                &available_tools,
                4096,
                0.7,
            ).await?;

            // Track token usage
            if let Some(usage) = &response.usage {
                self.token_usage.add_usage(usage);
                info!("Updated token usage - Total: {} (Input: {}, Output: {})", 
                      self.token_usage.total_tokens(), 
                      self.token_usage.total_input_tokens, 
                      self.token_usage.total_output_tokens);
            }

            // Check for tool calls
            let tool_calls = self.client.convert_tool_calls(&response.content);

            if tool_calls.is_empty() {
                // No tool calls, return the text response
                final_response = self.client.create_response_content(&response.content);
                if final_response.is_empty() {
                    final_response = "(No response received from assistant)".to_string();
                }
                break;
            }

            // Execute tool calls
            info!("Executing {} tool calls", tool_calls.len());
            let tool_results: Vec<ToolResult> = {
                let mut results = Vec::new();
                for call in &tool_calls {
                    info!("Executing tool: {} with ID: {}", call.name, call.id);
                    if let Some(tool) = self.tools.get(&call.name) {
                        match (tool.handler)(call).await {
                            Ok(result) => {
                                info!("Tool '{}' executed successfully", call.name);
                                results.push(result);
                            },
                            Err(e) => {
                                error!("Error executing tool '{}': {}", call.name, e);
                                results.push(ToolResult {
                                    tool_use_id: call.id.clone(),
                                    content: format!("Error executing tool '{}': {}", call.name, e),
                                    is_error: true,
                                });
                            }
                        }
                    } else {
                        error!("Unknown tool: {}", call.name);
                        results.push(ToolResult {
                            tool_use_id: call.id.clone(),
                            content: format!("Unknown tool: {}", call.name),
                            is_error: true,
                        });
                    }
                }
                results
            };

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
        info!("Final response generated ({} chars)", final_response.len());
        println!("Final response: {}", final_response);
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
}