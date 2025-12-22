use anyhow::anyhow;
use anyhow::Result;
use chrono::Local;
use clap::Parser;
use colored::*;
use crossterm::event::{KeyCode, KeyEventKind};
use dialoguer::Select;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

use env_logger::Builder;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, warn};

mod agent;
mod anthropic;
mod autocomplete;
mod config;
mod conversation;
mod database;
mod gemini;
mod formatter;
mod help;
mod input;
mod logo;
mod mcp;
mod security;
mod subagent;
mod web;

mod llm;
mod tools;

#[cfg(test)]
mod formatter_tests;

use agent::Agent;
use config::{Config, Provider};
use database::{
    get_database_path, Conversation as StoredConversation, DatabaseManager,
    Message as StoredMessage,
};
use formatter::create_code_formatter;
use help::{
    display_mcp_yolo_warning, display_yolo_warning, print_agent_help, print_file_permissions_help,
    print_help, print_mcp_help, print_permissions_help,
};
use input::InputHistory;
use mcp::McpManager;

/// Create a streaming renderer
fn create_streaming_renderer(
    formatter: &formatter::CodeFormatter,
) -> (
    Arc<Mutex<formatter::StreamingResponseFormatter>>,
    Arc<dyn Fn(String) + Send + Sync>,
) {
    let state = Arc::new(Mutex::new(formatter::StreamingResponseFormatter::new(
        formatter.clone(),
    )));
    let callback_state = Arc::clone(&state);
    let callback: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |content: String| {
        if content.is_empty() {
            return;
        }
        if let Ok(mut renderer) = callback_state.lock() {
            if let Err(e) = renderer.handle_chunk(&content) {
                eprintln!("{} Streaming formatter error: {}", "Error".red(), e);
            }
        }
    });
    (state, callback)
}

/// Process input and handle streaming/non-streaming response
async fn process_input(
    input: &str,
    agent: &mut Agent,
    formatter: &formatter::CodeFormatter,
    stream: bool,
    cancellation_flag: Arc<AtomicBool>,
) {
    // Show spinner while processing (only for non-streaming)
    if stream {
        let (streaming_state, stream_callback) = create_streaming_renderer(formatter);
        let result = agent
            .process_message_with_stream(
                &input,
                Some(Arc::clone(&stream_callback)),
                None,
                cancellation_flag.clone(),
            )
            .await;

        if let Ok(mut renderer) = streaming_state.lock() {
            if let Err(e) = renderer.finish() {
                eprintln!("{} Streaming formatter error: {}", "Error".red(), e);
            }
        }

        match result {
            Ok(_response) => {
                println!();
            }
            Err(e) => {
                if e.to_string().contains("CANCELLED") {
                    // Cancellation handled silently
                } else {
                    eprintln!("{}: {}", "Error".red(), e);
                }
                println!();
            }
        }
    } else {
        let spinner = create_spinner();
        let result = agent
            .process_message(&input, cancellation_flag.clone())
            .await;
        spinner.finish_and_clear();

        match result {
            Ok(response) => {
                // Only print response if it's not empty (i.e., not just @file references)
                if !response.is_empty() {
                    if let Err(e) = formatter.print_formatted(&response) {
                        eprintln!("{} formatting response: {}", "Error".red(), e);
                    }
                }
                println!();
            }
            Err(e) => {
                if e.to_string().contains("CANCELLED") {
                    // Cancellation handled silently
                } else {
                    eprintln!("{}: {}", "Error".red(), e);
                }
                println!();
            }
        }
    }
}

/// Check for and add context files
async fn add_context_files(agent: &mut Agent, context_files: &[String]) -> Result<()> {
    // Always add AGENTS.md from ~/.aixplosion/ if it exists (priority)
    let home_agents_md = get_home_agents_md_path();
    if home_agents_md.exists() {
        debug!("Auto-adding AGENTS.md from ~/.aixplosion/ as context");
        match agent
            .add_context_file(home_agents_md.to_str().unwrap())
            .await
        {
            Ok(_) => println!(
                "{} Added context file: {}",
                "✓".green(),
                home_agents_md.display()
            ),
            Err(e) => eprintln!(
                "{} Failed to add context file '{}': {}",
                "✗".red(),
                home_agents_md.display(),
                e
            ),
        }
    }

    // Also add AGENTS.md from current directory if it exists (in addition to home directory version)
    if Path::new("AGENTS.md").exists() {
        debug!("Auto-adding AGENTS.md from current directory as context");
        match agent.add_context_file("AGENTS.md").await {
            Ok(_) => println!("{} Added context file: {}", "✓".green(), "AGENTS.md"),
            Err(e) => eprintln!(
                "{} Failed to add context file 'AGENTS.md': {}",
                "✗".red(),
                e
            ),
        }
    }

    // Add any additional context files specified by the user
    for file_path in context_files {
        debug!("Adding context file: {}", file_path);
        match agent.add_context_file(file_path).await {
            Ok(_) => println!("{} Added context file: {}", "✓".green(), file_path),
            Err(e) => eprintln!(
                "{} Failed to add context file '{}': {}",
                "✗".red(),
                file_path,
                e
            ),
        }
    }

    Ok(())
}

/// Get the path to AGENTS.md in the user's home .aixplosion directory
fn get_home_agents_md_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".aixplosion")
        .join("AGENTS.md")
}

async fn handle_agent_command(
    args: &[&str],
    agent: &mut Agent,
    formatter: &formatter::CodeFormatter,
    stream: bool,
) -> Result<()> {
    let mut subagent_manager = subagent::SubagentManager::new()?;
    subagent_manager.load_all_subagents().await?;

    if args.is_empty() {
        // Show current subagent status
        if agent.is_subagent_mode() {
            println!("{}", "🤖 Current Subagent".cyan().bold());
            println!("  You are currently in a subagent session");
            println!("  Use '/agent exit' to return to default mode");
        } else {
            println!("{}", "🤖 Subagent Management".cyan().bold());
            println!("  No subagent currently active");
        }
        println!();
        print_agent_help();
        return Ok(());
    }

    match args[0] {
        "list" => {
            let subagents = subagent_manager.list_subagents();
            if subagents.is_empty() {
                println!(
                    "{}",
                    "No subagents configured. Use '/agent create' to create one.".yellow()
                );
                return Ok(());
            }

            println!("{}", "🤖 Available Subagents".cyan().bold());
            println!();
            for subagent in subagents {
                let status = if agent.is_subagent_mode()
                    && agent
                        .get_system_prompt()
                        .map_or(false, |p| p.contains(&subagent.system_prompt))
                {
                    "✅ Active".green().to_string()
                } else {
                    "⏸️ Inactive".yellow().to_string()
                };

                println!(
                    "  {} {} ({})",
                    "Agent:".bold(),
                    subagent.name.cyan(),
                    status
                );
                if !subagent.allowed_tools.is_empty() {
                    let allowed_tools: Vec<&str> =
                        subagent.allowed_tools.iter().map(|s| s.as_str()).collect();
                    println!("  Allowed tools: {}", allowed_tools.join(", "));
                }
                if !subagent.denied_tools.is_empty() {
                    let denied_tools: Vec<&str> =
                        subagent.denied_tools.iter().map(|s| s.as_str()).collect();
                    println!("  Denied tools: {}", denied_tools.join(", "));
                }
                println!();
            }
        }
        "create" => {
            if args.len() < 3 {
                println!(
                    "{} Usage: /agent create <name> <system_prompt>",
                    "⚠️".yellow()
                );
                println!(
                    "{} Example: /agent create rust-expert \"You are a Rust expert...\"",
                    "💡".blue()
                );
                return Ok(());
            }

            let name = args[1];
            let system_prompt = args[2..].join(" ");

            // Default tool set for new subagent - use readonly tools from metadata
            let registry = agent.tool_registry.read().await;
            let allowed_tools: Vec<String> = registry
                .get_all_tools()
                .filter(|metadata| metadata.readonly)
                .map(|metadata| metadata.name.clone())
                .collect();

            match subagent_manager
                .create_subagent(name, &system_prompt, allowed_tools, vec![])
                .await
            {
                Ok(config) => {
                    println!("{} Created subagent: {}", "✅".green(), name.cyan());
                    println!("  Config file: ~/.aixplosion/agents/{}.md", name);
                }
                Err(e) => {
                    eprintln!("{} Failed to create subagent: {}", "✗".red(), e);
                }
            }
        }
        "use" | "switch" => {
            if args.len() < 2 {
                println!("{} Usage: /agent use <name>", "⚠️".yellow());
                return Ok(());
            }

            let name = args[1];
            if let Some(config) = subagent_manager.get_subagent(name) {
                match agent.switch_to_subagent(config).await {
                    Ok(_) => {
                        // Clear conversation context when switching to subagent
                        match agent.clear_conversation_keep_agents_md().await {
                            Ok(_) => {
                                println!("{} Switched to subagent: {}", "✅".green(), name.cyan());
                                println!("{} Conversation context cleared", "🗑️".blue());
                            }
                            Err(e) => {
                                println!("{} Switched to subagent: {}", "✅".green(), name.cyan());
                                eprintln!(
                                    "{} Failed to clear conversation context: {}",
                                    "⚠️".yellow(),
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{} Failed to switch to subagent: {}", "✗".red(), e);
                    }
                }
            } else {
                eprintln!("{} Subagent '{}' not found", "✗".red(), name);
                println!(
                    "{} Available subagents: {}",
                    "💡".blue(),
                    subagent_manager
                        .list_subagents()
                        .iter()
                        .map(|s| s.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        "exit" => match agent.exit_subagent().await {
            Ok(_) => {
                println!("{} Exited subagent mode", "✅".green());
                println!("{} Previous conversation context restored", "🔄".blue());
            }
            Err(e) => {
                eprintln!("{} Failed to exit subagent mode: {}", "✗".red(), e);
            }
        },
        "delete" => {
            if args.len() < 2 {
                println!("{} Usage: /agent delete <name>", "⚠️".yellow());
                return Ok(());
            }

            let name = args[1];
            println!(
                "{} Are you sure you want to delete subagent '{}'?",
                "⚠️".yellow(),
                name
            );
            println!("  This action cannot be undone.");
            println!("  Use '/agent delete {} --confirm' to proceed", name);

            if args.len() > 2 && args[2] == "--confirm" {
                match subagent_manager.delete_subagent(name).await {
                    Ok(_) => {
                        println!("{} Deleted subagent: {}", "✅".green(), name);
                    }
                    Err(e) => {
                        eprintln!("{} Failed to delete subagent: {}", "✗".red(), e);
                    }
                }
            }
        }
        "edit" => {
            if args.len() < 2 {
                println!("{} Usage: /agent edit <name>", "⚠️".yellow());
                return Ok(());
            }

            let name = args[1];
            let file_path = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".aixplosion")
                .join("agents")
                .join(format!("{}.md", name));

            if file_path.exists() {
                println!(
                    "{} Opening subagent config for editing: {}",
                    "📝".blue(),
                    file_path.display()
                );

                // Try to open in default editor
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("notepad")
                    .arg(&file_path)
                    .status();

                #[cfg(not(target_os = "windows"))]
                {
                    if let Ok(editor) = std::env::var("EDITOR") {
                        let _ = std::process::Command::new(editor).arg(&file_path).status();
                    } else {
                        let _ = std::process::Command::new("nano").arg(&file_path).status();
                    }
                }

                println!(
                    "{} After editing, use '/agent reload {}' to apply changes",
                    "💡".blue(),
                    name
                );
            } else {
                eprintln!("{} Subagent '{}' not found", "✗".red(), name);
            }
        }
        "reload" => {
            subagent_manager.load_all_subagents().await?;
            println!("{} Reloaded subagent configurations", "✅".green());
        }
        "help" => {
            print_agent_help();
        }
        _ => {
            println!("{} Unknown agent command: {}", "⚠️".yellow(), args[0]);
            print_agent_help();
        }
    }

    Ok(())
}

async fn handle_shell_command(command: &str, _agent: &mut Agent) -> Result<()> {
    // Extract the shell command by removing the '!' prefix
    let shell_command = command.trim_start_matches('!').trim();

    if shell_command.is_empty() {
        println!(
            "{} Usage: !<command> - Execute a shell command",
            "⚠️".yellow()
        );
        println!("{} Examples: !dir, !ls -la, !git status", "💡".blue());
        return Ok(());
    }

    println!("{} Executing: {}", "🔧".blue(), shell_command);

    // Create a tool call for the bash command
    let tool_call = tools::ToolCall {
        id: "shell_command".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": shell_command
        }),
    };

    // Execute the bash command directly without permission checks
    // This bypasses the security manager for ! commands
    execute_bash_command_directly(&tool_call)
        .await
        .map(|result| {
            if result.is_error {
                println!("{} Command failed:", "❌".red());
                println!("{}", result.content.red());
            } else {
                println!("{}", result.content);
            }
        })
        .map_err(|e| {
            eprintln!("{} Error executing shell command: {}", "✗".red(), e);
            e
        })?;

    Ok(())
}

/// Execute a bash command directly without security checks (for ! commands)
async fn execute_bash_command_directly(tool_call: &tools::ToolCall) -> Result<tools::ToolResult> {
    let command = tool_call
        .arguments
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?
        .to_string();

    debug!("Direct shell command execution: {}", command);

    let tool_use_id = tool_call.id.clone();

    // Execute the command using tokio::task to spawn blocking operation
    let command_clone = command.clone();
    match tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", &command_clone])
                .output()
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("bash")
                .args(["-c", &command_clone])
                .output()
        }
    })
    .await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let content = if !stderr.is_empty() {
                format!(
                    "Exit code: {}\nStdout:\n{}\nStderr:\n{}",
                    output.status.code().unwrap_or(-1),
                    stdout,
                    stderr
                )
            } else {
                format!(
                    "Exit code: {}\nOutput:\n{}",
                    output.status.code().unwrap_or(-1),
                    stdout
                )
            };

            Ok(tools::ToolResult {
                tool_use_id,
                content,
                is_error: !output.status.success(),
            })
        }
        Ok(Err(e)) => Ok(tools::ToolResult {
            tool_use_id,
            content: format!("Error executing command '{}': {}", command, e),
            is_error: true,
        }),
        Err(e) => Ok(tools::ToolResult {
            tool_use_id,
            content: format!("Task join error: {}", e),
            is_error: true,
        }),
    }
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    let truncated: String = line.chars().take(max_chars).collect();
    if line.chars().count() > max_chars {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

fn build_message_preview(messages: &[StoredMessage]) -> String {
    if messages.is_empty() {
        return "(no messages)".to_string();
    }

    // Use the first message only; keep it single line and short
    let first_message = messages
        .iter()
        .find(|m| !m.content.trim().is_empty())
        .unwrap_or(&messages[0]);

    let first_line = first_message.content.lines().next().unwrap_or("").trim();
    let single_line = first_line.split_whitespace().collect::<Vec<_>>().join(" ");

    truncate_line(&single_line, 50)
}

fn format_resume_option(conversation: &StoredConversation, preview: &str) -> String {
    let updated_local = conversation
        .updated_at
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string();

    let short_id: String = conversation.id.chars().take(8).collect();
    let short_id = if conversation.id.len() > 8 {
        format!("{}…", short_id)
    } else {
        short_id
    };

    let meta = format!(
        "{} | Updated {} | Model {} | Requests {} | Tokens {}",
        short_id,
        updated_local,
        conversation.model,
        conversation.request_count,
        conversation.total_tokens
    );

    // Put preview on its own line and add a trailing newline to create spacing between items
    format!("{}\n  Preview: {}\n", meta, preview)
}

async fn build_conversation_previews(
    agent: &Agent,
    conversations: &[StoredConversation],
) -> Result<Vec<(StoredConversation, String)>> {
    let database_manager = agent
        .database_manager()
        .ok_or_else(|| anyhow!("Database is not configured"))?;

    let mut conversations_with_previews: Vec<(StoredConversation, String)> = Vec::new();

    for conversation in conversations {
        let messages = database_manager
            .get_conversation_messages(&conversation.id)
            .await?;
        if messages.is_empty() {
            continue; // Skip conversations with no messages
        }
        let preview = build_message_preview(&messages);
        conversations_with_previews.push((conversation.clone(), preview));
    }

    Ok(conversations_with_previews)
}

async fn select_conversation_index(
    prompt: &str,
    options: Vec<String>,
    cancel_message: &str,
) -> Option<usize> {
    let options_clone = options.clone();
    let prompt_text = prompt.to_string();
    let selection = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::task::spawn_blocking(move || {
            Select::new()
                .with_prompt(prompt_text)
                .items(&options_clone)
                .default(0) // Set first option as default
                .interact_opt()
        }),
    )
    .await;

    match selection {
        Ok(Ok(Ok(Some(index)))) => Some(index),
        Ok(Ok(Ok(None))) => {
            println!("{}", cancel_message.yellow());
            None
        }
        Ok(Ok(Err(e))) => {
            eprintln!("{} Failed to select conversation: {}", "?".red(), e);
            None
        }
        Ok(Err(e)) => {
            eprintln!("{} Failed to read selection: {}", "?".red(), e);
            None
        }
        Err(_) => {
            eprintln!(
                "{} Conversation selection timed out after 30 seconds.",
                "?".red()
            );
            None
        }
    }
}

async fn handle_resume_command(agent: &mut Agent) -> Result<()> {
    if agent.database_manager().is_none() {
        println!(
            "{} Database is not configured; cannot resume conversations.",
            "??".yellow()
        );
        return Ok(());
    }

    let current_id = agent.current_conversation_id();

    // Fetch more than 5 in case the current conversation is among the most recent
    let recent = agent.list_recent_conversations(15, None).await?;
    let available: Vec<StoredConversation> = recent
        .into_iter()
        .filter(|conv| Some(conv.id.as_str()) != current_id.as_deref())
        .take(5)
        .collect();

    if available.is_empty() {
        println!(
            "{} No other recent conversations found to resume.",
            "??".yellow()
        );
        return Ok(());
    }

    let conversations_with_previews = build_conversation_previews(agent, &available).await?;

    if conversations_with_previews.is_empty() {
        println!(
            "{} No recent conversations with messages found to resume.",
            "??".yellow()
        );
        return Ok(());
    }

    let options: Vec<String> = conversations_with_previews
        .iter()
        .map(|(conversation, preview)| format_resume_option(conversation, preview))
        .collect();

    let selected_index = select_conversation_index(
        "Select a conversation to resume",
        options,
        "Resume cancelled.",
    )
    .await;

    if let Some(index) = selected_index {
        if let Some((conversation, _)) = conversations_with_previews.get(index) {
            agent.resume_conversation(&conversation.id).await?;
            println!(
                "{} Resumed conversation {} ({} messages loaded).",
                "û".green(),
                conversation.id,
                agent.conversation_len()
            );
        }
    }

    Ok(())
}

async fn handle_search_command(agent: &mut Agent, query: &str) -> Result<()> {
    if agent.database_manager().is_none() {
        println!(
            "{} Database is not configured; cannot search conversations.",
            "??".yellow()
        );
        return Ok(());
    }

    let search_term = query.trim();
    if search_term.is_empty() {
        println!("{} Usage: /search <text>", "??".yellow());
        return Ok(());
    }

    let current_id = agent.current_conversation_id();
    let recent = agent
        .list_recent_conversations(30, Some(search_term))
        .await?;
    let available: Vec<StoredConversation> = recent
        .into_iter()
        .filter(|conv| Some(conv.id.as_str()) != current_id.as_deref())
        .collect();

    if available.is_empty() {
        println!(
            "{} No conversations matched '{}'.",
            "??".yellow(),
            search_term
        );
        return Ok(());
    }

    let conversations_with_previews = build_conversation_previews(agent, &available).await?;

    if conversations_with_previews.is_empty() {
        println!(
            "{} No matching conversations with messages found.",
            "??".yellow()
        );
        return Ok(());
    }

    let options: Vec<String> = conversations_with_previews
        .iter()
        .map(|(conversation, preview)| format_resume_option(conversation, preview))
        .collect();

    let prompt = format!("Select a conversation matching \"{}\"", search_term);
    let selected_index = select_conversation_index(&prompt, options, "Search cancelled.").await;

    if let Some(index) = selected_index {
        if let Some((conversation, _)) = conversations_with_previews.get(index) {
            agent.resume_conversation(&conversation.id).await?;
            println!(
                "{} Resumed conversation {} ({} messages loaded).",
                "–".green(),
                conversation.id,
                agent.conversation_len()
            );
        }
    }

    Ok(())
}

async fn handle_slash_command(
    command: &str,
    agent: &mut Agent,
    mcp_manager: &McpManager,
    formatter: &formatter::CodeFormatter,
    stream: bool,
) -> Result<bool> {
    let parts: Vec<&str> = command.trim().split(' ').collect();
    let cmd = parts[0];

    match cmd {
        "/help" => {
            print_help();
            Ok(true) // Command was handled
        }
        "/stats" | "/usage" => {
            print_usage_stats(agent);
            Ok(true) // Command was handled
        }
        "/context" => {
            agent.display_context();
            Ok(true) // Command was handled
        }
        "/provider" => {
            agent.display_provider();
            Ok(true)
        }
        "/model" => {
            let provider = agent.provider();
            let available = config::provider_models(provider);
            if parts.len() == 1 {
                println!("{}", "LLM Model".cyan().bold());
                println!("  Provider: {}", provider);
                println!("  Current: {}", agent.model());
                if !available.is_empty() {
                    println!("  Available:");
                    for model in available {
                        println!("    - {}", model);
                    }
                }
                println!("  Usage: /model <name> | /model list | /model pick");
                return Ok(true);
            }

            match parts[1] {
                "list" => {
                    println!("{}", "Available Models".cyan().bold());
                    println!("  Provider: {}", provider);
                    if available.is_empty() {
                        println!("  (no default models configured)");
                    } else {
                        for model in available {
                            println!("  - {}", model);
                        }
                    }
                }
                "pick" => {
                    if available.is_empty() {
                        println!("{} No default models configured for {}", "??".yellow(), provider);
                        return Ok(true);
                    }
                    let selected = Select::new()
                        .with_prompt("Select a model")
                        .items(available)
                        .default(0)
                        .interact_opt()?;
                    if let Some(index) = selected {
                        let new_model = available[index].to_string();
                        agent.set_model(new_model.clone()).await?;
                        println!("{} Active model set to {}", "??".green(), new_model);
                    }
                }
                _ => {
                    let new_model = parts[1..].join(" ");
                    if new_model.is_empty() {
                        println!("{} Usage: /model <name>", "??".yellow());
                        return Ok(true);
                    }
                    agent.set_model(new_model.clone()).await?;
                    println!("{} Active model set to {}", "??".green(), new_model);
                }
            }
            Ok(true)
        }
        "/search" => {
            let search_text = command.trim_start_matches("/search").trim();
            handle_search_command(agent, search_text).await?;
            Ok(true)
        }
        "/resume" => {
            handle_resume_command(agent).await?;
            Ok(true)
        }
        "/clear" => {
            match agent.clear_conversation_keep_agents_md().await {
                Ok(_) => {
                    println!(
                        "{}",
                        "🧹 Conversation context cleared! (AGENTS.md preserved if it existed)"
                            .green()
                    );
                }
                Err(e) => {
                    eprintln!("{} Failed to clear context: {}", "✗".red(), e);
                }
            }
            Ok(true) // Command was handled
        }
        "/reset-stats" => {
            agent.reset_token_usage();
            println!("{}", "📊 Token usage statistics reset!".green());
            Ok(true) // Command was handled
        }
        "/mcp" => {
            handle_mcp_command(&parts[1..], mcp_manager).await?;
            // Force refresh MCP tools after any MCP command
            if let Err(e) = agent.force_refresh_mcp_tools().await {
                warn!("Failed to refresh MCP tools: {}", e);
            }
            Ok(true) // Command was handled
        }
        "/permissions" => {
            handle_permissions_command(&parts[1..], agent).await?;
            Ok(true) // Command was handled
        }
        "/file-permissions" => {
            handle_file_permissions_command(&parts[1..], agent).await?;
            Ok(true) // Command was handled
        }
        "/plan" => {
            // Parse subcommand with splitn to preserve plan IDs containing whitespace
            let mut plan_parts = command.splitn(3, ' ');
            let _ = plan_parts.next(); // "/plan"
            let sub = plan_parts.next().unwrap_or("").trim();
            match sub {
                "on" => {
                    agent.set_plan_mode(true).await?;
                    println!(
                        "{} Plan mode enabled: generating read-only plans and saving them to the database.",
                        "✓".green()
                    );
                }
                "off" => {
                    agent.set_plan_mode(false).await?;
                    println!(
                        "{} Plan mode disabled: execution tools restored.",
                        "✓".green()
                    );
                }
                "run" => {
                    let plan_id_raw = plan_parts.next().unwrap_or("").trim();
                    if plan_id_raw.is_empty() {
                        println!("{} Usage: /plan run <plan_id>", "ℹ".yellow());
                        return Ok(true);
                    }
                    let plan_id = plan_id_raw;
                    println!("{} Loading plan {}...", "…".cyan(), plan_id);
                    let message = agent.load_plan_for_execution(plan_id).await?;
                    println!(
                        "{} Running saved plan {} (plan mode disabled for execution).",
                        "→".green(),
                        plan_id
                    );
                    let cancellation_flag = Arc::new(AtomicBool::new(false));
                    if stream {
                        let (streaming_state, stream_callback) =
                            create_streaming_renderer(formatter);
                        let response = agent
                            .process_message_with_stream(
                                &message,
                                Some(Arc::clone(&stream_callback)),
                                None,
                                cancellation_flag,
                            )
                            .await;
                        if let Ok(mut renderer) = streaming_state.lock() {
                            if let Err(e) = renderer.finish() {
                                eprintln!("{} Streaming formatter error: {}", "Error".red(), e);
                            }
                        }
                        response?;
                    } else {
                        let spinner = create_spinner();
                        let response = agent.process_message(&message, cancellation_flag).await?;
                        spinner.finish_and_clear();
                        formatter.print_formatted(&response)?;
                    }
                }
                _ => {
                    println!(
                        "{} Unknown /plan command. Use '/plan on', '/plan off', or '/plan run <id>'.",
                        "ℹ".yellow()
                    );
                }
            }
            Ok(true)
        }
        "/agent" => {
            handle_agent_command(&parts[1..], agent, formatter, stream).await?;
            Ok(true)
        }
        "/exit" | "/quit" => {
            // Print final stats before exiting
            print_usage_stats(agent);
            println!("{}", "Goodbye! 👋".green());
            std::process::exit(0);
        }
        _ => {
            println!(
                "{} Unknown command: {}. Type /help for available commands.",
                "⚠️".yellow(),
                cmd
            );
            Ok(true) // Command was handled (as unknown)
        }
    }
}

/// Handle MCP commands
async fn handle_mcp_command(args: &[&str], mcp_manager: &McpManager) -> Result<()> {
    if args.is_empty() {
        print_mcp_help();
        return Ok(());
    }

    match args[0] {
        "list" => match mcp_manager.list_servers().await {
            Ok(servers) => {
                println!("{}", "🔌 MCP Servers".cyan().bold());
                println!();
                if servers.is_empty() {
                    println!("{}", "No MCP servers configured.".yellow());
                    return Ok(());
                }

                for (name, config, connected) in servers {
                    let status = if connected {
                        "✅ Connected".green().to_string()
                    } else if config.enabled {
                        "❌ Disconnected".red().to_string()
                    } else {
                        "⏸️ Disabled".yellow().to_string()
                    };

                    println!("{} {} ({})", "Server:".bold(), name.cyan(), status);

                    if let Some(command) = &config.command {
                        println!("  Command: {}", command);
                    }
                    if let Some(args) = &config.args {
                        println!("  Args: {}", args.join(" "));
                    }
                    if let Some(url) = &config.url {
                        println!("  URL: {}", url);
                    }

                    if connected {
                        if let Ok(tools) = mcp_manager.get_all_tools().await {
                            let server_tools: Vec<_> = tools
                                .iter()
                                .filter(|(server_name, _)| server_name == &name)
                                .collect();
                            println!("  Tools: {} available", server_tools.len());
                        }
                    }
                    println!();
                }
            }
            Err(e) => {
                eprintln!("{} Failed to list MCP servers: {}", "✗".red(), e);
            }
        },
        "connect" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp connect <server_name>", "⚠️".yellow());
                return Ok(());
            }

            println!(
                "{} Connecting to MCP server: {}",
                "🔌".blue(),
                args[1].cyan()
            );

            match mcp_manager.connect_server(args[1]).await {
                Ok(_) => {
                    println!(
                        "{} Successfully connected to MCP server: {}",
                        "✅".green(),
                        args[1].cyan()
                    );

                    // Try to list available tools
                    match mcp_manager.get_all_tools().await {
                        Ok(tools) => {
                            let server_tools: Vec<_> = tools
                                .iter()
                                .filter(|(server_name, _)| server_name == args[1])
                                .collect();
                            if !server_tools.is_empty() {
                                println!("{} Available tools: {}", "🛠️".blue(), server_tools.len());
                                for (_, tool) in server_tools {
                                    println!(
                                        "  - {} {}",
                                        tool.name.bold(),
                                        tool.description
                                            .as_ref()
                                            .unwrap_or(&"".to_string())
                                            .dimmed()
                                    );
                                }
                            }
                        }
                        Err(_) => {
                            println!("{} Connected but failed to list tools", "⚠️".yellow());
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "{} Failed to connect to MCP server '{}': {}",
                        "✗".red(),
                        args[1],
                        e
                    );
                    println!("{} Troubleshooting:", "💡".yellow());
                    println!("  1. Check if the server is properly configured: /mcp list");
                    println!("  2. Verify the command/URL is correct");
                    println!("  3. Ensure all dependencies are installed");
                    println!("  4. Check network connectivity for WebSocket servers");
                    println!("  5. Try reconnecting: /mcp reconnect {}", args[1]);
                }
            }
        }
        "disconnect" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp disconnect <server_name>", "⚠️".yellow());
                return Ok(());
            }

            match mcp_manager.disconnect_server(args[1]).await {
                Ok(_) => {
                    println!(
                        "{} Disconnected from MCP server: {}",
                        "🔌".yellow(),
                        args[1].cyan()
                    );
                }
                Err(e) => {
                    eprintln!(
                        "{} Failed to disconnect from MCP server '{}': {}",
                        "✗".red(),
                        args[1],
                        e
                    );
                }
            }
        }
        "reconnect" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp reconnect <server_name>", "⚠️".yellow());
                return Ok(());
            }

            match mcp_manager.reconnect_server(args[1]).await {
                Ok(_) => {
                    println!(
                        "{} Reconnected to MCP server: {}",
                        "🔄".blue(),
                        args[1].cyan()
                    );
                }
                Err(e) => {
                    eprintln!(
                        "{} Failed to reconnect to MCP server '{}': {}",
                        "✗".red(),
                        args[1],
                        e
                    );
                }
            }
        }
        "tools" => match mcp_manager.get_all_tools().await {
            Ok(tools) => {
                println!("{}", "🛠️  MCP Tools".cyan().bold());
                println!();

                if tools.is_empty() {
                    println!(
                        "{}",
                        "No MCP tools available. Connect to a server first.".yellow()
                    );
                    return Ok(());
                }

                let mut by_server = std::collections::HashMap::new();
                for (server_name, tool) in tools {
                    by_server
                        .entry(server_name)
                        .or_insert_with(Vec::new)
                        .push(tool);
                }

                for (server_name, server_tools) in by_server {
                    println!("{} {}:", "Server:".bold(), server_name.cyan());
                    for tool in server_tools {
                        println!("  🛠️  {}", tool.name.bold());
                        if let Some(description) = &tool.description {
                            println!("     {}", description.dimmed());
                        }
                    }
                    println!();
                }
            }
            Err(e) => {
                eprintln!("{} Failed to list MCP tools: {}", "✗".red(), e);
            }
        },
        "add" => {
            if args.len() < 4 {
                println!(
                    "{} Usage: /mcp add <name> stdio <command> [args...]",
                    "⚠️".yellow()
                );
                println!("{} Usage: /mcp add <name> ws <url>", "⚠️".yellow());
                println!();
                println!("{}", "Examples:".green().bold());
                println!(
                    "  /mcp add myserver stdio npx -y @modelcontextprotocol/server-filesystem"
                );
                println!("  /mcp add websocket ws://localhost:8080");
                return Ok(());
            }

            let name = args[1];
            let connection_type = args[2];

            if connection_type == "stdio" {
                let command = args[3];
                let server_args: Vec<String> = args[4..].iter().map(|s| s.to_string()).collect();

                // Validate that we have a proper command
                if command.is_empty() {
                    println!("{} Command cannot be empty", "⚠️".yellow());
                    return Ok(());
                }

                let server_config = mcp::McpServerConfig {
                    name: name.to_string(),
                    command: Some(command.to_string()),
                    args: if server_args.is_empty() {
                        None
                    } else {
                        Some(server_args)
                    },
                    url: None,
                    env: None,
                    enabled: true,
                };

                println!("{} Adding MCP server: {}", "🔧".blue(), name.cyan());
                println!("  Command: {}", command);
                if !args[4..].is_empty() {
                    println!("  Args: {}", args[4..].join(" "));
                }

                match mcp_manager.add_server(name, server_config).await {
                    Ok(_) => {
                        println!(
                            "{} Successfully added MCP server: {}",
                            "✅".green(),
                            name.cyan()
                        );
                        println!(
                            "{} Use '/mcp connect {}' to connect to this server",
                            "💡".blue(),
                            name
                        );
                    }
                    Err(e) => {
                        eprintln!("{} Failed to add MCP server '{}': {}", "✗".red(), name, e);
                        println!("{} Common issues:", "💡".yellow());
                        println!("  - Command '{}' not found or not executable", command);
                        println!("  - Missing dependencies (e.g., Node.js, npm, npx)");
                        println!("  - Network connectivity issues");
                        println!("  - Insufficient permissions");
                    }
                }
            } else if connection_type == "ws" || connection_type == "websocket" {
                let url = args[3];

                // Basic URL validation
                if !url.starts_with("ws://") && !url.starts_with("wss://") {
                    println!("{} URL must start with ws:// or wss://", "⚠️".yellow());
                    return Ok(());
                }

                let server_config = mcp::McpServerConfig {
                    name: name.to_string(),
                    command: None,
                    args: None,
                    url: Some(url.to_string()),
                    env: None,
                    enabled: true,
                };

                println!("{} Adding MCP server: {}", "🔧".blue(), name.cyan());
                println!("  URL: {}", url);

                match mcp_manager.add_server(name, server_config).await {
                    Ok(_) => {
                        println!(
                            "{} Successfully added MCP server: {}",
                            "✅".green(),
                            name.cyan()
                        );
                        println!(
                            "{} Use '/mcp connect {}' to connect to this server",
                            "💡".blue(),
                            name
                        );
                    }
                    Err(e) => {
                        eprintln!("{} Failed to add MCP server '{}': {}", "✗".red(), name, e);
                    }
                }
            } else {
                println!("{} Connection type must be 'stdio' or 'ws'", "⚠️".yellow());
                println!("{} Available types:", "💡".blue());
                println!("  - stdio: For command-line based MCP servers");
                println!("  - ws: For WebSocket-based MCP servers");
            }
        }
        "remove" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp remove <server_name>", "⚠️".yellow());
                return Ok(());
            }

            match mcp_manager.remove_server(args[1]).await {
                Ok(_) => {
                    println!("{} Removed MCP server: {}", "🗑️".red(), args[1].cyan());
                }
                Err(e) => {
                    eprintln!(
                        "{} Failed to remove MCP server '{}': {}",
                        "✗".red(),
                        args[1],
                        e
                    );
                }
            }
        }
        "connect-all" => match mcp_manager.connect_all_enabled().await {
            Ok(_) => {
                println!(
                    "{} Attempted to connect to all enabled MCP servers",
                    "🔄".blue()
                );
            }
            Err(e) => {
                eprintln!("{} Failed to connect to MCP servers: {}", "✗".red(), e);
            }
        },
        "test" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp test <command>", "⚠️".yellow());
                println!(
                    "{} Test if a command is available and executable",
                    "💡".blue()
                );
                return Ok(());
            }

            let command = args[1];
            println!("{} Testing command: {}", "🧪".blue(), command.cyan());

            // Try to run the command with --version or --help to test if it exists
            let test_args = if command == "npx" {
                vec!["--version".to_string()]
            } else {
                vec!["--version".to_string()]
            };

            match tokio::process::Command::new(command)
                .args(&test_args)
                .output()
                .await
            {
                Ok(output) => {
                    if output.status.success() {
                        println!(
                            "{} Command '{}' is available and executable",
                            "✅".green(),
                            command
                        );
                        if !output.stdout.is_empty() {
                            let version = String::from_utf8_lossy(&output.stdout);
                            println!("  Version: {}", version.trim());
                        }
                    } else {
                        println!(
                            "{} Command '{}' exists but failed to execute",
                            "⚠️".yellow(),
                            command
                        );
                        if !output.stderr.is_empty() {
                            let error = String::from_utf8_lossy(&output.stderr);
                            println!("  Error: {}", error.trim());
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "{} Command '{}' not found or not executable",
                        "✗".red(),
                        command
                    );
                    println!("  Error: {}", e);
                    println!("{} Suggestions:", "💡".blue());
                    println!("  - Install the command/tool if missing");
                    println!("  - Check if the command is in your PATH");
                    println!("  - Use the full path to the command");
                }
            }
        }
        "disconnect-all" => match mcp_manager.disconnect_all().await {
            Ok(_) => {
                println!("{} Disconnected from all MCP servers", "🔌".yellow());
            }
            Err(e) => {
                eprintln!("{} Failed to disconnect from MCP servers: {}", "✗".red(), e);
            }
        },
        _ => {
            println!("{} Unknown MCP command: {}", "⚠️".yellow(), args[0]);
            print_mcp_help();
        }
    }

    Ok(())
}

/// Print MCP help information

/// Print usage statistics
fn print_usage_stats(agent: &Agent) {
    let usage = agent.get_token_usage();
    println!("{}", "📊 Token Usage Statistics".cyan().bold());
    println!();
    println!("{}", "Request Summary:".green().bold());
    println!("  Requests made: {}", usage.request_count);
    println!();
    println!("{}", "Token Usage:".green().bold());
    println!("  Input tokens:  {}", usage.total_input_tokens);
    println!("  Output tokens: {}", usage.total_output_tokens);
    println!("  Total tokens: {}", usage.total_tokens());
    println!();

    if usage.request_count > 0 {
        let avg_input = usage.total_input_tokens as f64 / usage.request_count as f64;
        let avg_output = usage.total_output_tokens as f64 / usage.request_count as f64;
        let avg_total = usage.total_tokens() as f64 / usage.request_count as f64;

        println!("{}", "Average per request:".green().bold());
        println!("  Input tokens:  {:.1}", avg_input);
        println!("  Output tokens: {:.1}", avg_output);
        println!("  Total tokens: {:.1}", avg_total);
        println!();
    }
}

/// Create a progress spinner for API calls
fn create_spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    spinner.set_message("Thinking...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner
}

/// Handle permissions commands
async fn handle_permissions_command(args: &[&str], agent: &mut Agent) -> Result<()> {
    use crate::security::PermissionResult;

    if args.is_empty() {
        // Display current permissions with full details
        let security_manager_ref = agent.get_bash_security_manager().clone();
        let security_manager = security_manager_ref.read().await;
        security_manager.display_permissions();
        return Ok(());
    }

    match args[0] {
        "show" | "list" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let security_manager = security_manager_ref.read().await;
            security_manager.display_permissions();
        }
        "test" => {
            if args.len() < 2 {
                println!("{} Usage: /permissions test <command>", "⚠️".yellow());
                return Ok(());
            }

            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let security_manager = security_manager_ref.read().await;

            match security_manager.check_command_permission(&command) {
                PermissionResult::Allowed => {
                    println!("{} Command '{}' is ALLOWED", "✅".green(), command);
                }
                PermissionResult::Denied => {
                    println!("{} Command '{}' is DENIED", "❌".red(), command);
                }
                PermissionResult::RequiresPermission => {
                    println!(
                        "{} Command '{}' requires permission",
                        "❓".yellow(),
                        command
                    );
                }
            }
        }
        "allow" => {
            if args.len() < 2 {
                println!(
                    "{} Usage: /permissions allow <command_pattern>",
                    "⚠️".yellow()
                );
                println!("{} Examples:", "💡".blue());
                println!("  /permissions allow 'git *'");
                println!("  /permissions allow 'cargo test'");
                println!("  /permissions allow 'ls -la'");
                return Ok(());
            }

            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;

            security_manager.add_to_allowlist(command.clone());
            println!("{} Added '{}' to allowlist", "✅".green(), command);

            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "deny" => {
            if args.len() < 2 {
                println!(
                    "{} Usage: /permissions deny <command_pattern>",
                    "⚠️".yellow()
                );
                println!("{} Examples:", "💡".blue());
                println!("  /permissions deny 'rm *'");
                println!("  /permissions deny 'sudo *'");
                println!("  /permissions deny 'format'");
                return Ok(());
            }

            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;

            security_manager.add_to_denylist(command.clone());
            println!("{} Added '{}' to denylist", "❌".red(), command);

            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "remove-allow" => {
            if args.len() < 2 {
                println!(
                    "{} Usage: /permissions remove-allow <command_pattern>",
                    "⚠️".yellow()
                );
                return Ok(());
            }

            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;

            if security_manager.remove_from_allowlist(&command) {
                println!("{} Removed '{}' from allowlist", "🗑️".yellow(), command);

                // Save to config
                if let Err(e) = save_permissions_to_config(&agent).await {
                    println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
                }
            } else {
                println!(
                    "{} Command '{}' not found in allowlist",
                    "⚠️".yellow(),
                    command
                );
            }
        }
        "remove-deny" => {
            if args.len() < 2 {
                println!(
                    "{} Usage: /permissions remove-deny <command_pattern>",
                    "⚠️".yellow()
                );
                return Ok(());
            }

            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;

            if security_manager.remove_from_denylist(&command) {
                println!("{} Removed '{}' from denylist", "🗑️".yellow(), command);

                // Save to config
                if let Err(e) = save_permissions_to_config(&agent).await {
                    println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
                }
            } else {
                println!(
                    "{} Command '{}' not found in denylist",
                    "⚠️".yellow(),
                    command
                );
            }
        }
        "enable" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            let mut security = security_manager.get_security().clone();
            security.enabled = true;
            security_manager.update_security(security);
            println!("{} Bash security enabled", "✅".green());

            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "disable" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            let mut security = security_manager.get_security().clone();
            security.enabled = false;
            security_manager.update_security(security);
            println!("{} Bash security disabled", "⚠️".yellow());
            println!(
                "{} Warning: This allows any bash command to be executed!",
                "⚠️".red().bold()
            );

            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "ask-on" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            let mut security = security_manager.get_security().clone();
            security.ask_for_permission = true;
            security_manager.update_security(security);
            println!("{} Ask for permission enabled", "✅".green());

            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "ask-off" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            let mut security = security_manager.get_security().clone();
            security.ask_for_permission = false;
            security_manager.update_security(security);
            println!("{} Ask for permission disabled", "⚠️".yellow());
            println!("{} Unknown commands will be denied by default", "⚠️".red());

            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "help" => {
            print_permissions_help();
        }
        _ => {
            println!("{} Unknown permissions command: {}", "⚠️".yellow(), args[0]);
            println!("{} Available commands:", "💡".yellow());
            println!("  /permissions                - Show current permissions");
            println!("  /permissions help          - Show permissions help");
            println!("  /permissions test <cmd>    - Test if a command is allowed");
            println!("  /permissions allow <cmd>   - Add command to allowlist");
            println!("  /permissions deny <cmd>    - Add command to denylist");
            println!("  /permissions remove-allow <cmd> - Remove from allowlist");
            println!("  /permissions remove-deny <cmd> - Remove from denylist");
            println!("  /permissions enable        - Enable bash security");
            println!("  /permissions disable       - Disable bash security");
            println!("  /permissions ask-on        - Enable asking for permission");
            println!("  /permissions ask-off       - Disable asking for permission");
        }
    }

    Ok(())
}

/// Save current permissions to unified config file
async fn save_permissions_to_config(agent: &Agent) -> Result<()> {
    use crate::config::Config;

    // Load existing config to preserve other settings
    let mut existing_config = Config::load(None).await?;

    // Get current security settings from agent
    let updated_config = agent.get_config_for_save().await;

    // Update only the bash_security settings
    existing_config.bash_security = updated_config.bash_security;

    // Save the updated config
    match existing_config.save(None).await {
        Ok(_) => {
            println!("{} Permissions saved to unified config", "💾".blue());
        }
        Err(e) => {
            println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
        }
    }

    Ok(())
}

/// Handle file permissions commands
async fn handle_file_permissions_command(args: &[&str], agent: &mut Agent) -> Result<()> {
    use crate::security::FilePermissionResult;

    if args.is_empty() {
        // Display current file permissions with full details
        let file_security_manager_ref = agent.get_file_security_manager().clone();
        let file_security_manager = file_security_manager_ref.read().await;
        file_security_manager.display_file_permissions();
        return Ok(());
    }

    match args[0] {
        "show" | "list" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let file_security_manager = file_security_manager_ref.read().await;
            file_security_manager.display_file_permissions();
        }
        "test" => {
            if args.len() < 3 {
                println!(
                    "{} Usage: /file-permissions test <operation> <path>",
                    "⚠️".yellow()
                );
                println!(
                    "{} Operations: write_file, edit_file, delete_file, create_directory",
                    "💡".blue()
                );
                return Ok(());
            }

            let operation = args[1];
            let path = args[2..].join(" ");
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;

            match file_security_manager.check_file_permission(operation, &path) {
                FilePermissionResult::Allowed => {
                    println!(
                        "{} File operation '{}' on '{}' is ALLOWED",
                        "✅".green(),
                        operation,
                        path
                    );
                }
                FilePermissionResult::Denied => {
                    println!(
                        "{} File operation '{}' on '{}' is DENIED",
                        "❌".red(),
                        operation,
                        path
                    );
                }
                FilePermissionResult::RequiresPermission => {
                    println!(
                        "{} File operation '{}' on '{}' requires permission",
                        "❓".yellow(),
                        operation,
                        path
                    );
                }
            }
        }
        "enable" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            let mut security = file_security_manager.get_file_security().clone();
            security.enabled = true;
            file_security_manager.update_file_security(security);
            println!("{} File security enabled", "✅".green());

            // Save to config
            if let Err(e) = save_file_permissions_to_config(&agent).await {
                println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
            }
        }
        "disable" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            let mut security = file_security_manager.get_file_security().clone();
            security.enabled = false;
            file_security_manager.update_file_security(security);
            println!("{} File security disabled", "⚠️".yellow());
            println!(
                "{} Warning: This allows any file operation to be executed!",
                "⚠️".red().bold()
            );

            // Save to config
            if let Err(e) = save_file_permissions_to_config(&agent).await {
                println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
            }
        }
        "ask-on" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            let mut security = file_security_manager.get_file_security().clone();
            security.ask_for_permission = true;
            file_security_manager.update_file_security(security);
            println!("{} Ask for file permission enabled", "✅".green());

            // Save to config
            if let Err(e) = save_file_permissions_to_config(&agent).await {
                println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
            }
        }
        "ask-off" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            let mut security = file_security_manager.get_file_security().clone();
            security.ask_for_permission = false;
            file_security_manager.update_file_security(security);
            println!("{} Ask for file permission disabled", "⚠️".yellow());
            println!(
                "{} All file operations will be allowed by default",
                "⚠️".red()
            );

            // Save to config
            if let Err(e) = save_file_permissions_to_config(&agent).await {
                println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
            }
        }
        "reset-session" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            file_security_manager.reset_session_permissions();
            println!("{} Session file permissions reset", "🔄".blue());
            println!(
                "{} File operations will require permission again",
                "💡".blue()
            );
        }
        "help" => {
            print_file_permissions_help();
        }
        _ => {
            println!(
                "{} Unknown file permissions command: {}",
                "⚠️".yellow(),
                args[0]
            );
            println!("{} Available commands:", "💡".yellow());
            println!("  /file-permissions                - Show current file permissions");
            println!("  /file-permissions help          - Show file permissions help");
            println!("  /file-permissions test <op> <path> - Test if file operation is allowed");
            println!("  /file-permissions enable        - Enable file security");
            println!("  /file-permissions disable       - Disable file security");
            println!("  /file-permissions ask-on        - Enable asking for permission");
            println!("  /file-permissions ask-off       - Disable asking for permission");
            println!("  /file-permissions reset-session - Reset session permissions");
        }
    }

    Ok(())
}

/// Save current file permissions to unified config file
async fn save_file_permissions_to_config(agent: &Agent) -> Result<()> {
    use crate::config::Config;

    // Load existing config to preserve other settings
    let mut existing_config = Config::load(None).await?;

    // Get current file security settings from agent
    let file_security_manager_ref = agent.get_file_security_manager().clone();
    let file_security_manager = file_security_manager_ref.read().await;
    let updated_file_security = file_security_manager.get_file_security().clone();

    // Update only the file_security settings
    existing_config.file_security = updated_file_security;

    // Save the updated config
    match existing_config.save(None).await {
        Ok(_) => {
            println!("{} File permissions saved to unified config", "💾".blue());
        }
        Err(e) => {
            println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
        }
    }

    Ok(())
}

#[derive(Parser)]
#[command(name = "aixplosion")]
#[command(about = "A CLI coding agent with pluggable LLM providers")]
#[command(version)]
struct Cli {
    /// The message to send to the agent
    #[arg(short = 'm', long)]
    message: Option<String>,

    /// Set the API key (overrides config file)
    #[arg(short = 'k', long)]
    api_key: Option<String>,

    /// LLM provider to use (anthropic, gemini, or z.ai)
    #[arg(long)]
    provider: Option<config::Provider>,

    /// Specify the model to use
    #[arg(long)]
    model: Option<String>,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,

    /// Run in non-interactive mode
    #[arg(short, long)]
    non_interactive: bool,

    /// Files to include as context
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    context_files: Vec<String>,

    /// System prompt to use for the conversation
    #[arg(short = 's', long = "system", value_name = "PROMPT")]
    system_prompt: Option<String>,

    /// Enable streaming responses
    #[arg(long)]
    stream: bool,

    /// Enable 'yolo' mode - bypass all permission checks for file and tool operations
    #[arg(long)]
    yolo: bool,

    /// Enable plan-only mode (generate a plan in Markdown without making changes)
    #[arg(long = "plan-mode")]
    plan_mode: bool,

    /// Enable the optional web UI
    #[arg(long)]
    web: bool,

    /// Port for the web UI
    #[arg(long, default_value = "3000")]
    web_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Cli::parse();
    debug!("Starting AIxplosion");

    // Display large red warning if yolo mode is enabled
    if cli.yolo {
        display_yolo_warning();
    }

    // Load configuration
    let mut config = Config::load(cli.config.as_deref()).await?;
    if let Some(provider) = cli.provider {
        if provider != config.provider {
            config.set_provider(provider);
        }
    }

    // Initialize database
    info!("Initializing database...");
    let db_path = get_database_path()?;
    let database_manager = DatabaseManager::new(db_path).await?;
    info!(
        "Database initialized at: {}",
        database_manager.path().display()
    );

    // Override API key if provided via command line (highest priority)
    if let Some(api_key) = cli.api_key {
        config.api_key = api_key;
    } else if config.api_key.is_empty() {
        // If no API key from config, try environment variable for the selected provider
        config.api_key = config::provider_default_api_key(config.provider);
    }

    let model = cli
        .model
        .clone()
        .unwrap_or_else(|| config.default_model.clone());

    println!("Using configuration:");
    println!("  Provider: {}", config.provider);
    println!("  Base URL: {}", config.base_url);
    println!("  Model: {}", model);

    // Show yolo mode status
    if cli.yolo {
        println!(
            "  {} YOLO MODE ENABLED - All permission checks bypassed!",
            "🔥".red().bold()
        );
    }

    // Validate API key without exposing it
    if config.api_key.is_empty() {
        let env_hint = match config.provider {
            Provider::Anthropic => "ANTHROPIC_AUTH_TOKEN",
            Provider::Gemini => "GEMINI_API_KEY or GOOGLE_API_KEY",
            Provider::Zai => "ZAI_API_KEY",
        };
        eprintln!(
            "{}",
            format!(
                "Error: API key is required for {}. Set {} or use --api-key",
                config.provider, env_hint
            )
            .red()
        );
        eprintln!(
            "Create a config file at {} or set {}",
            Config::default_config_path().display(),
            env_hint
        );
        std::process::exit(1);
    } else {
        println!(
            "  API Key: {}",
            if config.api_key.len() > 10 {
                format!(
                    "{}... ({} chars)",
                    &config.api_key[..8],
                    config.api_key.len()
                )
            } else {
                "configured".to_string()
            }
        );
    }

    // Create code formatter
    let formatter = create_code_formatter()?;

    // Create and run agent using the new async constructor
    let mut agent =
        Agent::new_with_plan_mode(config.clone(), model.clone(), cli.yolo, cli.plan_mode).await;

    // Initialize MCP manager
    let mcp_manager = Arc::new(McpManager::new());

    // Initialize MCP manager with config from unified config
    mcp_manager.initialize(config.mcp.clone()).await?;

    // Set MCP manager in agent
    agent = agent.with_mcp_manager(mcp_manager.clone());

    // Set database manager in agent
    let database_manager = Arc::new(database_manager);
    agent = agent.with_database_manager(database_manager.clone());

    // Connect to all enabled MCP servers
    info!("Connecting to MCP servers...");
    let mcp_connect_result = tokio::time::timeout(
        std::time::Duration::from_secs(30), // 30 second timeout for MCP connections
        mcp_manager.connect_all_enabled(),
    )
    .await;

    match mcp_connect_result {
        Ok(Ok(_)) => {
            info!("MCP servers connected successfully");
        }
        Ok(Err(e)) => {
            warn!("Failed to connect to MCP servers: {}", e);
            error!("MCP Server Connection Issues:");
            error!("  - Check that MCP servers are configured correctly: /mcp list");
            error!("  - Verify server commands/URLs are valid");
            error!("  - Ensure all dependencies are installed");
            error!("  - Use '/mcp test <command>' to verify command availability");
            error!("  - Tool calls to unavailable MCP servers will fail");
        }
        Err(_) => {
            warn!("MCP server connection timed out after 30 seconds");
            error!("MCP Server Connection Timeout:");
            error!("  - MCP servers are taking too long to respond");
            error!("  - Check if servers are running and accessible");
            error!("  - Use '/mcp reconnect <server>' to try connecting manually");
        }
    }

    // Force initial refresh of MCP tools after connecting
    info!("Refreshing MCP tools...");
    let mcp_refresh_result = tokio::time::timeout(
        std::time::Duration::from_secs(15), // 15 second timeout for MCP tools refresh
        agent.force_refresh_mcp_tools(),
    )
    .await;

    match mcp_refresh_result {
        Ok(Ok(_)) => {
            info!("MCP tools loaded successfully");
        }
        Ok(Err(e)) => {
            warn!("Failed to refresh MCP tools on startup: {}", e);
            error!("MCP Tools Loading Failed:");
            error!("  - Connected MCP servers may not be responding properly");
            error!("  - Tools may have invalid schemas or descriptions");
            error!("  - Use '/mcp tools' to check available tools");
            error!("  - Use '/mcp reconnect <server>' to fix connection issues");
        }
        Err(_) => {
            warn!("MCP tools refresh timed out after 15 seconds");
            error!("MCP Tools Refresh Timeout:");
            error!("  - MCP servers are taking too long to provide tools");
            error!("  - Some tools may not be available initially");
            error!("  - Tools will be refreshed on demand during use");
        }
    }

    // Display YOLO mode warning after MCP configuration is complete
    if cli.yolo {
        display_mcp_yolo_warning();
    }

    // Set system prompt - use command line prompt if provided, otherwise use config default
    match &cli.system_prompt {
        Some(system_prompt) => {
            agent.set_system_prompt(system_prompt.clone());
            println!(
                "{} Using custom system prompt: {}",
                "✓".green(),
                system_prompt
            );
        }
        None => {
            // Use config's default system prompt if available
            if let Some(default_prompt) = &config.default_system_prompt {
                agent.set_system_prompt(default_prompt.clone());
                println!("{} Using default system prompt from config", "✓".green());
            }
        }
    }

    if cli.plan_mode {
        agent.apply_plan_mode_prompt();
        println!(
            "{} Plan mode enabled: generating read-only plans and saving them to the database.",
            "û".green()
        );
    }

    // Add context files
    add_context_files(&mut agent, &cli.context_files).await?;

    // Create initial conversation in database
    match agent.start_new_conversation().await {
        Ok(conversation_id) => {
            info!("Started initial conversation: {}", conversation_id);
        }
        Err(e) => {
            warn!("Failed to create initial conversation: {}", e);
        }
    }

    if cli.web {
        if cli.message.is_some() || cli.non_interactive {
            println!(
                "{} Ignoring -m/--message and --non-interactive flags because --web was supplied.",
                "?".yellow()
            );
        }

        let shared_agent = Arc::new(AsyncMutex::new(agent));
        let subagent_manager = Arc::new(AsyncMutex::new(subagent::SubagentManager::new()?));
        {
            let mut manager = subagent_manager.lock().await;
            manager.load_all_subagents().await?;
        }

        let state = web::WebState {
            agent: shared_agent,
            database: database_manager.clone(),
            mcp_manager: mcp_manager.clone(),
            subagent_manager,
        };

        web::launch_web_ui(state, cli.web_port).await?;
        return Ok(());
    }

    let is_interactive = cli.message.is_none() && !cli.non_interactive;

    if let Some(message) = cli.message {
        // Display the message with file highlighting
        let highlighted_message = formatter.format_input_with_file_highlighting(&message);
        println!("> {}", highlighted_message);

        // Single message mode
        if cli.stream {
            let cancellation_flag = Arc::new(AtomicBool::new(false));
            let (streaming_state, stream_callback) = create_streaming_renderer(&formatter);
            let response = agent
                .process_message_with_stream(
                    &message,
                    Some(Arc::clone(&stream_callback)),
                    None,
                    cancellation_flag,
                )
                .await;
            if let Ok(mut renderer) = streaming_state.lock() {
                if let Err(e) = renderer.finish() {
                    eprintln!("{} Streaming formatter error: {}", "Error".red(), e);
                }
            }
            response?;
            print_usage_stats(&agent);
        } else {
            let cancellation_flag = Arc::new(AtomicBool::new(false));
            let spinner = create_spinner();
            let response = agent.process_message(&message, cancellation_flag).await?;
            spinner.finish_and_clear();
            formatter.print_formatted(&response)?;
            print_usage_stats(&agent);
        }
    } else if cli.non_interactive {
        // Read from stdin
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let trimmed_input = input.trim();

        // Display the input with file highlighting
        let highlighted_input = formatter.format_input_with_file_highlighting(trimmed_input);
        println!("> {}", highlighted_input);

        let cancellation_flag = Arc::new(AtomicBool::new(false));
        if cli.stream {
            let (streaming_state, stream_callback) = create_streaming_renderer(&formatter);
            let response = agent
                .process_message_with_stream(
                    trimmed_input,
                    Some(Arc::clone(&stream_callback)),
                    None,
                    cancellation_flag,
                )
                .await;
            if let Ok(mut renderer) = streaming_state.lock() {
                if let Err(e) = renderer.finish() {
                    eprintln!("{} Streaming formatter error: {}", "Error".red(), e);
                }
            }
            response?;
            print_usage_stats(&agent);
        } else {
            let spinner = create_spinner();
            let response = agent
                .process_message(trimmed_input, cancellation_flag)
                .await?;
            spinner.finish_and_clear();
            formatter.print_formatted(&response)?;
            print_usage_stats(&agent);
        }
    } else {
        // Interactive mode
        // Display the cool logo on startup
        logo::display_logo();
        println!("{}", "🤖 AIxplosion - Interactive Mode".green().bold());
        if cli.plan_mode {
            println!(
                "{}",
                "Plan mode enabled: generating read-only plans and saving them to the database."
                    .yellow()
                    .bold()
            );
        }
        println!(
            "{}",
            "Type 'exit', 'quit', or '/exit' to quit. Type '/help' for available commands."
                .dimmed()
        );
        println!();

        // Initialize shared history for the interactive session
        let mut input_history = InputHistory::new();

        loop {
            let input = match input::read_input_with_completion_and_highlighting(
                Some(&formatter),
                &mut input_history,
            ) {
                Ok(input) => input,
                Err(e) => {
                    if e.to_string().contains("CANCELLED") {
                        // User pressed ESC during input, just continue to next prompt
                        continue;
                    }
                    eprintln!("{} Error reading input: {}", "✗".red(), e);
                    continue;
                }
            };

            // If input is empty, continue to next iteration
            if input.is_empty() {
                continue;
            }

            // Check for commands first (they can't be multi-line)
            if input.starts_with('/')
                || input.starts_with('!')
                || input == "exit"
                || input == "quit"
            {
                // Check for slash commands first
                if input.starts_with('/') {
                    match handle_slash_command(
                        &input,
                        &mut agent,
                        &mcp_manager,
                        &formatter,
                        cli.stream,
                    )
                    .await
                    {
                        Ok(_) => {} // Command handled successfully
                        Err(e) => {
                            eprintln!("{} Error handling command: {}", "✗".red(), e);
                        }
                    }
                    continue;
                }

                // Check for shell commands (!)
                if input.starts_with('!') {
                    match handle_shell_command(&input, &mut agent).await {
                        Ok(_) => {} // Command handled successfully
                        Err(e) => {
                            eprintln!("{} Error executing shell command: {}", "✗".red(), e);
                        }
                    }
                    continue;
                }

                // Check for traditional exit commands
                if input == "exit" || input == "quit" {
                    // Print final stats before exiting
                    print_usage_stats(&agent);
                    println!("{}", "Goodbye! 👋".green());
                    break;
                }
            } else {
                // For regular messages, spawn ESC listener only for AI processing
                let cancellation_flag_for_processing = Arc::new(AtomicBool::new(false));
                let cancellation_flag_listener = cancellation_flag_for_processing.clone();

                // Start ESC key listener only during actual AI processing (not for commands)
                let esc_handle = tokio::spawn(async move {
                    use crossterm::event;
                    loop {
                        // Even longer polling interval since this is only during AI processing
                        if event::poll(std::time::Duration::from_millis(1000)).unwrap_or(false) {
                            if let Ok(event::Event::Key(key_event)) = event::read() {
                                if key_event.code == KeyCode::Esc
                                    && key_event.kind == KeyEventKind::Press
                                {
                                    cancellation_flag_listener.store(true, Ordering::SeqCst);
                                    println!("\n{} Cancelling AI conversation...", "🛑".yellow());
                                    break;
                                }
                            }
                        }
                        // Longer sleep during AI processing
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    }
                });

                // Process the input (highlighting already shown during typing)
                process_input(
                    &input,
                    &mut agent,
                    &formatter,
                    cli.stream,
                    cancellation_flag_for_processing.clone(),
                )
                .await;

                // Clean up the ESC listener task
                esc_handle.abort();
            }
        }
    }

    // Print final usage stats before exiting (only for interactive mode)
    if is_interactive {
        print_usage_stats(&agent);
    }

    // Disconnect from all MCP servers
    if let Err(e) = mcp_manager.disconnect_all().await {
        warn!("Failed to disconnect from MCP servers: {}", e);
    }

    // Close database connection
    database_manager.close().await;

    Ok(())
}
