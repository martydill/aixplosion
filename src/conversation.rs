use crate::anthropic::ContentBlock;
use crate::database::{DatabaseManager, Message as StoredMessage};
use anyhow::Result;
use colored::Colorize;
use log::{debug, info};
use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// Manages conversation state and database operations
pub struct ConversationManager {
    pub conversation: Vec<crate::anthropic::Message>,
    pub system_prompt: Option<String>,
    pub current_conversation_id: Option<String>,
    pub database_manager: Option<Arc<DatabaseManager>>,
    pub model: String,
}

impl ConversationManager {
    pub fn new(
        system_prompt: Option<String>,
        database_manager: Option<Arc<DatabaseManager>>,
        model: String,
    ) -> Self {
        Self {
            conversation: Vec::new(),
            system_prompt,
            current_conversation_id: None,
            database_manager,
            model,
        }
    }

    /// Start a new conversation
    pub async fn start_new_conversation(&mut self) -> Result<String> {
        if let Some(database_manager) = &self.database_manager {
            // Create new conversation in database
            let conversation_id = database_manager
                .create_conversation(self.system_prompt.clone(), &self.model)
                .await?;

            // Update current conversation tracking
            self.current_conversation_id = Some(conversation_id.clone());

            info!("Started new conversation: {}", conversation_id);
            Ok(conversation_id)
        } else {
            // Fallback: just generate a conversation ID without database
            let conversation_id = Uuid::new_v4().to_string();
            self.current_conversation_id = Some(conversation_id.clone());
            Ok(conversation_id)
        }
    }

    /// Save a message to the current conversation in the database
    pub async fn save_message_to_conversation(
        &mut self,
        role: &str,
        content: &str,
        tokens: i32,
    ) -> Result<()> {
        if let (Some(database_manager), Some(conversation_id)) =
            (&self.database_manager, &self.current_conversation_id)
        {
            database_manager
                .add_message(conversation_id, role, content, tokens)
                .await?;
        }
        Ok(())
    }

    /// Update usage statistics in the database
    pub async fn update_database_usage_stats(
        &mut self,
        input_tokens: i32,
        output_tokens: i32,
    ) -> Result<()> {
        if let Some(database_manager) = &self.database_manager {
            database_manager
                .update_usage_stats(input_tokens, output_tokens)
                .await?;
        }
        Ok(())
    }

    /// Clear conversation but keep AGENTS.md files if they exist in context
    /// Create a new conversation in the database and start tracking it
    pub async fn clear_conversation_keep_agents_md(&mut self) -> Result<String> {
        use std::path::Path;

        // Check for AGENTS.md in ~/.aixplosion/ (priority)
        let home_agents_md = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".aixplosion")
            .join("AGENTS.md");

        // Check for AGENTS.md in current directory
        let local_agents_md = Path::new("AGENTS.md");

        // Store the AGENTS.md content before clearing
        let mut home_agents_content = None;
        let mut local_agents_content = None;

        if home_agents_md.exists() {
            debug!("Reading AGENTS.md from ~/.aixplosion/ before clearing conversation");
            match std::fs::read_to_string(&home_agents_md) {
                Ok(content) => home_agents_content = Some(content),
                Err(e) => {
                    log::warn!("Failed to read AGENTS.md from ~/.aixplosion/: {}", e);
                }
            }
        }

        if local_agents_md.exists() {
            debug!("Reading AGENTS.md from current directory before clearing conversation");
            match std::fs::read_to_string(local_agents_md) {
                Ok(content) => local_agents_content = Some(content),
                Err(e) => {
                    log::warn!("Failed to read AGENTS.md from current directory: {}", e);
                }
            }
        }

        // Clear the conversation first
        self.conversation.clear();

        // Start a new conversation in the database
        let new_conversation_id = self.start_new_conversation().await?;
        debug!(
            "Started new conversation {} after clearing",
            new_conversation_id
        );

        // Re-add AGENTS.md content if it was captured
        if let Some(content) = home_agents_content {
            let context_message = format!(
                "Context from file '{}':\n\n```\n{}\n```",
                home_agents_md.display(),
                content
            );

            self.conversation.push(crate::anthropic::Message {
                role: "user".to_string(),
                content: vec![crate::anthropic::ContentBlock::text(context_message)],
            });

            println!(
                "{} Re-added context file: {}",
                "\u{2713}", // Using unicode character instead of colored()
                home_agents_md.display()
            );
        }

        if let Some(content) = local_agents_content {
            let context_message =
                format!("Context from file 'AGENTS.md':\n\n```\n{}\n```", content);

            self.conversation.push(crate::anthropic::Message {
                role: "user".to_string(),
                content: vec![crate::anthropic::ContentBlock::text(context_message)],
            });

            println!("{} Re-added context file: AGENTS.md", "\u{2713}");
        }

        if !home_agents_md.exists() && !local_agents_md.exists() {
            debug!("Clearing conversation (no AGENTS.md files found)");
        }

        Ok(new_conversation_id)
    }

    /// Add a file as context to the conversation
    pub async fn add_context_file(&mut self, file_path: &str) -> Result<()> {
        use path_absolutize::*;
        use shellexpand;
        use tokio::fs;

        let expanded_path = shellexpand::tilde(file_path);
        let absolute_path = Path::new(&*expanded_path).absolutize()?;

        match fs::read_to_string(&absolute_path).await {
            Ok(content) => {
                let context_message = format!(
                    "Context from file '{}':\n\n```\n{}\n```",
                    absolute_path.display(),
                    content
                );

                self.conversation.push(crate::anthropic::Message {
                    role: "user".to_string(),
                    content: vec![crate::anthropic::ContentBlock::text(context_message)],
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
        let re = regex::Regex::new(r"@([^\s@]+)").unwrap();
        re.captures_iter(message)
            .map(|cap| cap[1].to_string())
            .collect()
    }

    /// Remove @file syntax from message and return cleaned message
    pub fn clean_message(&self, message: &str) -> String {
        let re = Regex::new(r"@[^\s@]+").unwrap();
        re.replace_all(message, "").trim().to_string()
    }

    /// Replace the current in-memory conversation with records loaded from storage
    pub fn set_conversation_from_records(
        &mut self,
        conversation_id: String,
        system_prompt: Option<String>,
        model: String,
        messages: &[StoredMessage],
    ) {
        self.conversation.clear();

        for message in messages {
            self.conversation.push(crate::anthropic::Message {
                role: message.role.clone(),
                content: vec![ContentBlock::text(message.content.clone())],
            });
        }

        self.current_conversation_id = Some(conversation_id);
        self.system_prompt = system_prompt;
        self.model = model;
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
            println!(
                "{}",
                "No context yet. Start a conversation to see context here.".dimmed()
            );
            println!();
            return;
        }

        for (i, message) in self.conversation.iter().enumerate() {
            let role_color = match message.role.as_str() {
                "user" => "blue",
                "assistant" => "green",
                _ => "yellow",
            };

            println!(
                "{} {}: {}",
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
                                let safe_end = text
                                    .char_indices()
                                    .nth(100)
                                    .map(|(idx, _)| idx)
                                    .unwrap_or(text.len());
                                format!("{}...", &text[..safe_end])
                            } else {
                                text.clone()
                            };
                            println!(
                                "  {} {}: {}",
                                format!("└─ Block {}", j + 1).dimmed(),
                                "Text".green(),
                                preview.replace('\n', " ")
                            );
                        }
                    }
                    "tool_use" => {
                        if let (Some(ref id), Some(ref name), Some(ref input)) =
                            (&block.id, &block.name, &block.input)
                        {
                            println!(
                                "  {} {}: {} ({})",
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
                                let safe_end = input_str
                                    .char_indices()
                                    .nth(80)
                                    .map(|(idx, _)| idx)
                                    .unwrap_or(input_str.len());
                                format!("{}...", &input_str[..safe_end])
                            } else {
                                input_str
                            };
                            println!("    {} {}", "Input:".dimmed(), preview);
                        }
                    }
                    "tool_result" => {
                        if let (Some(ref tool_use_id), Some(ref content), ref is_error) =
                            (&block.tool_use_id, &block.content, &block.is_error)
                        {
                            let result_type = if is_error.unwrap_or(false) {
                                "Error".red()
                            } else {
                                "Result".green()
                            };
                            println!(
                                "  {} {}: {} ({})",
                                format!("└─ Block {}", j + 1).dimmed(),
                                result_type,
                                tool_use_id,
                                format!("{} chars", content.len()).dimmed()
                            );
                            let preview = if content.len() > 80 {
                                // Use safe character boundary slicing
                                let safe_end = content
                                    .char_indices()
                                    .nth(80)
                                    .map(|(idx, _)| idx)
                                    .unwrap_or(content.len());
                                format!("{}...", &content[..safe_end])
                            } else {
                                content.clone()
                            };
                            println!("    {} {}", "Content:".dimmed(), preview.replace('\n', " "));
                        }
                    }
                    _ => {
                        println!(
                            "  {} {}",
                            format!("└─ Block {}", j + 1).dimmed(),
                            "Unknown".red()
                        );
                    }
                }
            }
            println!();
        }

        println!("{}", "─".repeat(50).dimmed());
        println!(
            "{}: {} messages, {} total content blocks",
            "Summary".bold(),
            self.conversation.len(),
            self.conversation
                .iter()
                .map(|m| m.content.len())
                .sum::<usize>()
        );
        println!();
    }
}
