use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use log::{debug, error};
use regex::Regex;
use serde_json::Value;

use crate::config::Config;
use crate::anthropic::{AnthropicClient, Message, ContentBlock, Usage};
use crate::tools::{Tool, ToolResult, get_builtin_tools};
use crate::tool_display::{ToolCallDisplay, SimpleToolDisplay, should_use_pretty_output, ToolDisplay};
use colored::*;

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
    system_prompt: Option<String>,
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
            system_prompt: None,
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

        let mut final_response = String::new();
        let max_iterations = 500;
        let mut iteration = 0;

        while iteration < max_iterations {
            iteration += 1;

            // Get available tools
            let available_tools: Vec<Tool> = self.tools.values().cloned().collect();

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
                // Note: Don't print here in streaming mode - it's handled by the stream callback
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
                    
                    if let Some(tool) = self.tools.get(&call.name) {
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
                                format!("{}...", &text[..100])
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
                                format!("{}...", &input_str[..80])
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
                                format!("{}...", &content[..80])
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