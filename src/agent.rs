use anyhow::Result;
use std::collections::HashMap;

use crate::config::Config;
use crate::anthropic::{AnthropicClient, Message, ContentBlock};
use crate::tools::{Tool, ToolResult, get_builtin_tools};

pub struct Agent {
    client: AnthropicClient,
    model: String,
    tools: HashMap<String, Tool>,
    conversation: Vec<Message>,
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
        }
    }

    pub async fn process_message(&mut self, message: &str) -> Result<String> {
        // Add user message to conversation
        self.conversation.push(Message {
            role: "user".to_string(),
            content: vec![ContentBlock::text(message.to_string())],
        });

        let mut final_response = String::new();
        let max_iterations = 10;
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
            let tool_results: Vec<ToolResult> = {
                let mut results = Vec::new();
                for call in &tool_calls {
                    if let Some(tool) = self.tools.get(&call.name) {
                        match (tool.handler)(call).await {
                            Ok(result) => results.push(result),
                            Err(e) => results.push(ToolResult {
                                tool_use_id: call.id.clone(),
                                content: format!("Error executing tool '{}': {}", call.name, e),
                                is_error: true,
                            }),
                        }
                    } else {
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

        Ok(final_response)
    }

    pub fn clear_conversation(&mut self) {
        self.conversation.clear();
    }

    pub fn get_conversation_length(&self) -> usize {
        self.conversation.len()
    }
}