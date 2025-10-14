use clap::Parser;
use colored::*;
use anyhow::Result;
use std::io::{self, Read};
use dialoguer::Input;
use log::{debug, info, warn, error};
use env_logger::Builder;
use std::path::Path;
use indicatif::{ProgressBar, ProgressStyle};

mod config;
mod anthropic;
mod tools;
mod agent;
mod formatter;
mod tool_display;
mod mcp;

use config::Config;
use agent::Agent;
use formatter::create_code_formatter;
use mcp::McpManager;
use std::sync::Arc;

/// Check for and add context files
async fn add_context_files(agent: &mut Agent, context_files: &[String]) -> Result<()> {
    if context_files.is_empty() {
        // Automatically add AGENTS.md if it exists
        if Path::new("AGENTS.md").exists() {
            debug!("Auto-adding AGENTS.md as context");
            agent.add_context_file("AGENTS.md").await?;
        }
        return Ok(());
    }

    for file_path in context_files {
        debug!("Adding context file: {}", file_path);
        match agent.add_context_file(file_path).await {
            Ok(_) => println!("{} Added context file: {}", "✓".green(), file_path),
            Err(e) => eprintln!("{} Failed to add context file '{}': {}", "✗".red(), file_path, e),
        }
    }

    Ok(())
}

async fn handle_slash_command(command: &str, agent: &mut Agent, mcp_manager: &McpManager) -> Result<bool> {
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
        "/clear" => {
            match agent.clear_conversation_keep_agents_md().await {
                Ok(_) => {
                    println!("{}", "🧹 Conversation context cleared! (AGENTS.md preserved if it existed)".green());
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
        "/exit" | "/quit" => {
            // Print final stats before exiting
            print_usage_stats(agent);
            println!("{}", "Goodbye! 👋".green());
            std::process::exit(0);
        }
        _ => {
            println!("{} Unknown command: {}. Type /help for available commands.", "⚠️".yellow(), cmd);
            Ok(true) // Command was handled (as unknown)
        }
    }
}

/// Handle MCP commands
async fn handle_mcp_command(args: &[&str], mcp_manager: &McpManager) -> Result<()> {
    use log::{warn, error};
    
    if args.is_empty() {
        print_mcp_help();
        return Ok(());
    }

    match args[0] {
        "list" => {
            match mcp_manager.list_servers().await {
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
                        
                        println!("{} {} ({})", 
                            "Server:".bold(), 
                            name.cyan(), 
                            status
                        );
                        
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
                                let server_tools: Vec<_> = tools.iter()
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
            }
        }
        "connect" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp connect <server_name>", "⚠️".yellow());
                return Ok(());
            }
            
            println!("{} Connecting to MCP server: {}", "🔌".blue(), args[1].cyan());
            
            match mcp_manager.connect_server(args[1]).await {
                Ok(_) => {
                    println!("{} Successfully connected to MCP server: {}", "✅".green(), args[1].cyan());
                    
                    // Try to list available tools
                    match mcp_manager.get_all_tools().await {
                        Ok(tools) => {
                            let server_tools: Vec<_> = tools.iter()
                                .filter(|(server_name, _)| server_name == args[1])
                                .collect();
                            if !server_tools.is_empty() {
                                println!("{} Available tools: {}", "🛠️".blue(), server_tools.len());
                                for (_, tool) in server_tools {
                                    println!("  - {} {}", tool.name.bold(), 
                                        tool.description.as_ref().unwrap_or(&"".to_string()).dimmed());
                                }
                            }
                        }
                        Err(_) => {
                            println!("{} Connected but failed to list tools", "⚠️".yellow());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to connect to MCP server '{}': {}", "✗".red(), args[1], e);
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
                    println!("{} Disconnected from MCP server: {}", "🔌".yellow(), args[1].cyan());
                }
                Err(e) => {
                    eprintln!("{} Failed to disconnect from MCP server '{}': {}", "✗".red(), args[1], e);
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
                    println!("{} Reconnected to MCP server: {}", "🔄".blue(), args[1].cyan());
                }
                Err(e) => {
                    eprintln!("{} Failed to reconnect to MCP server '{}': {}", "✗".red(), args[1], e);
                }
            }
        }
        "tools" => {
            match mcp_manager.get_all_tools().await {
                Ok(tools) => {
                    println!("{}", "🛠️  MCP Tools".cyan().bold());
                    println!();
                    
                    if tools.is_empty() {
                        println!("{}", "No MCP tools available. Connect to a server first.".yellow());
                        return Ok(());
                    }
                    
                    let mut by_server = std::collections::HashMap::new();
                    for (server_name, tool) in tools {
                        by_server.entry(server_name).or_insert_with(Vec::new).push(tool);
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
            }
        }
        "add" => {
            if args.len() < 4 {
                println!("{} Usage: /mcp add <name> stdio <command> [args...]", "⚠️".yellow());
                println!("{} Usage: /mcp add <name> ws <url>", "⚠️".yellow());
                println!();
                println!("{}", "Examples:".green().bold());
                println!("  /mcp add myserver stdio npx -y @modelcontextprotocol/server-filesystem");
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
                    args: if server_args.is_empty() { None } else { Some(server_args) },
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
                        println!("{} Successfully added MCP server: {}", "✅".green(), name.cyan());
                        println!("{} Use '/mcp connect {}' to connect to this server", "💡".blue(), name);
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
                        println!("{} Successfully added MCP server: {}", "✅".green(), name.cyan());
                        println!("{} Use '/mcp connect {}' to connect to this server", "💡".blue(), name);
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
                    eprintln!("{} Failed to remove MCP server '{}': {}", "✗".red(), args[1], e);
                }
            }
        }
        "connect-all" => {
            match mcp_manager.connect_all_enabled().await {
                Ok(_) => {
                    println!("{} Attempted to connect to all enabled MCP servers", "🔄".blue());
                }
                Err(e) => {
                    eprintln!("{} Failed to connect to MCP servers: {}", "✗".red(), e);
                }
            }
        }
      "test" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp test <command>", "⚠️".yellow());
                println!("{} Test if a command is available and executable", "💡".blue());
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
                        println!("{} Command '{}' is available and executable", "✅".green(), command);
                        if !output.stdout.is_empty() {
                            let version = String::from_utf8_lossy(&output.stdout);
                            println!("  Version: {}", version.trim());
                        }
                    } else {
                        println!("{} Command '{}' exists but failed to execute", "⚠️".yellow(), command);
                        if !output.stderr.is_empty() {
                            let error = String::from_utf8_lossy(&output.stderr);
                            println!("  Error: {}", error.trim());
                        }
                    }
                }
                Err(e) => {
                    println!("{} Command '{}' not found or not executable", "✗".red(), command);
                    println!("  Error: {}", e);
                    println!("{} Suggestions:", "💡".blue());
                    println!("  - Install the command/tool if missing");
                    println!("  - Check if the command is in your PATH");
                    println!("  - Use the full path to the command");
                }
            }
        }
        "disconnect-all" => {
            match mcp_manager.disconnect_all().await {
                Ok(_) => {
                    println!("{} Disconnected from all MCP servers", "🔌".yellow());
                }
                Err(e) => {
                    eprintln!("{} Failed to disconnect from MCP servers: {}", "✗".red(), e);
                }
            }
        }
        _ => {
            println!("{} Unknown MCP command: {}", "⚠️".yellow(), args[0]);
            print_mcp_help();
        }
    }
    
    Ok(())
}

/// Print MCP help information
fn print_mcp_help() {
    println!("{}", "🔌 MCP Commands".cyan().bold());
    println!();
    println!("{}", "Server Management:".green().bold());
    println!("  /mcp list                    - List all MCP servers and their status");
    println!("  /mcp add <name> stdio <cmd>  - Add a stdio MCP server");
    println!("  /mcp add <name> ws <url>     - Add a WebSocket MCP server");
    println!("  /mcp remove <name>           - Remove an MCP server");
    println!("  /mcp connect <name>          - Connect to a specific server");
    println!("  /mcp disconnect <name>       - Disconnect from a specific server");
    println!("  /mcp reconnect <name>        - Reconnect to a specific server");
    println!("  /mcp connect-all             - Connect to all enabled servers");
    println!("  /mcp disconnect-all          - Disconnect from all servers");
    println!();
    println!("{}", "Testing & Debugging:".green().bold());
    println!("  /mcp test <command>          - Test if a command is available");
    println!("  /mcp tools                   - List all available MCP tools");
    println!();
    println!("{}", "Examples:".green().bold());
    println!("  /mcp test npx                - Test if npx is available");
    println!("  /mcp add myserver stdio npx -y @modelcontextprotocol/server-filesystem");
    println!("  /mcp add websocket ws://localhost:8080");
    println!("  /mcp connect myserver");
    println!("  /mcp tools");
    println!();
}

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
            .unwrap()
    );
    spinner.set_message("Thinking...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner
}

/// Print help information
fn print_help() {
    println!("{}", "🤖 AI Agent - Slash Commands".cyan().bold());
    println!();
    println!("{}", "Available commands:".green().bold());
    println!("  /help         - Show this help message");
    println!("  /stats        - Show token usage statistics");
    println!("  /usage        - Show token usage statistics (alias for /stats)");
    println!("  /context      - Show current conversation context");
    println!("  /clear        - Clear all conversation context (keeps AGENTS.md if it exists)");
    println!("  /reset-stats  - Reset token usage statistics");
    println!("  /mcp          - Manage MCP (Model Context Protocol) servers");
    println!("  /exit         - Exit the program");
    println!("  /quit         - Exit the program");
    println!();
    println!("{}", "MCP Commands:".green().bold());
    println!("  /mcp list                    - List MCP servers");
    println!("  /mcp add <name> stdio <cmd>  - Add stdio server");
    println!("  /mcp add <name> ws <url>     - Add WebSocket server");
    println!("  /mcp test <command>          - Test command availability");
    println!("  /mcp connect <name>          - Connect to server");
    println!("  /mcp tools                   - List available tools");
    println!("  /mcp help                    - Show MCP help");
    println!();
    println!("{}", "Context Files:".green().bold());
    println!("  Use -f or --file to include files as context");
    println!("  Use @path/to/file syntax in messages to auto-include files");
    println!("  AGENTS.md is automatically included if it exists");
    println!("  Messages with only @file references will NOT make API calls");
    println!();
    println!("{}", "System Prompts:".green().bold());
    println!("  Use -s or --system to set a custom system prompt");
    println!("  System prompts set the behavior and personality of the AI");
    println!();
    println!("{}", "Streaming:".green().bold());
    println!("  Use --stream flag to enable streaming responses");
    println!("  Streaming shows responses as they're generated (no spinner)");
    println!("  Non-streaming shows a spinner and formats the complete response");
    println!();
    println!("{}", "Examples:".green().bold());
    println!("  ai-agent -f config.toml \"Explain this configuration\"");
    println!("  ai-agent \"What does @Cargo.toml contain?\"");
    println!("  ai-agent \"Compare @file1.rs and @file2.rs\"");
    println!("  ai-agent \"@file1.txt @file2.txt\"  # Only adds context, no API call");
    println!("  ai-agent -s \"You are a Rust expert\" \"Help me with this code\"");
    println!("  ai-agent -s \"Act as a code reviewer\" -f main.rs \"Review this code\"");
    println!("  ai-agent --stream \"Tell me a story\"  # Stream the response");
    println!();
    println!("{}", "Any other input will be sent to the AI agent for processing.".dimmed());
    println!();
}

#[derive(Parser)]
#[command(name = "ai-agent")]
#[command(about = "A CLI coding agent powered by Anthropic AI")]
#[command(version)]
struct Cli {
    /// The message to send to the agent
    #[arg(short = 'm', long)]
    message: Option<String>,

    /// Set the API key (overrides config file)
    #[arg(short = 'k', long)]
    api_key: Option<String>,

    /// Specify the model to use
    #[arg(long, default_value = "glm-4.6")]
    model: String,

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
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Cli::parse();
    debug!("Starting AI Agent with model: {}", cli.model);

    // Load configuration
    let mut config = Config::load(cli.config.as_deref()).await?;

    // Override API key if provided via command line
    if let Some(api_key) = cli.api_key {
        config.api_key = api_key;
    }

    println!("Using configuration:");
    println!("  Base URL: {}", config.base_url);
    println!("  Model: {}", cli.model);
    println!("  API Key (first 10 chars): {}...", &config.api_key[..config.api_key.len().min(10)]);

    // Validate API key
    if config.api_key.is_empty() {
        eprintln!("{}", "Error: API key is required. Set it in config or use --api-key".red());
        eprintln!("Create a config file at {} or set ANTHROPIC_AUTH_TOKEN environment variable",
                 Config::default_config_path().display());
        std::process::exit(1);
    }

    // Create code formatter
    let formatter = create_code_formatter()?;

    // Create and run agent
    let mut agent = Agent::new(config.clone(), cli.model);
    
    // Initialize MCP manager
    let mcp_manager = Arc::new(McpManager::new());
    
    // Set MCP manager in agent
    agent = agent.with_mcp_manager(mcp_manager.clone());
    
    // Connect to all enabled MCP servers
    if let Err(e) = mcp_manager.connect_all_enabled().await {
        warn!("Failed to connect to MCP servers: {}", e);
        error!("MCP Server Connection Issues:");
        error!("  - Check that MCP servers are configured correctly: /mcp list");
        error!("  - Verify server commands/URLs are valid");
        error!("  - Ensure all dependencies are installed");
        error!("  - Use '/mcp test <command>' to verify command availability");
        error!("  - Tool calls to unavailable MCP servers will fail");
    }

    // Force initial refresh of MCP tools after connecting
    if let Err(e) = agent.force_refresh_mcp_tools().await {
        warn!("Failed to refresh MCP tools on startup: {}", e);
        error!("MCP Tools Loading Failed:");
        error!("  - Connected MCP servers may not be responding properly");
        error!("  - Tools may have invalid schemas or descriptions");
        error!("  - Use '/mcp tools' to check available tools");
        error!("  - Use '/mcp reconnect <server>' to fix connection issues");
    } else {
        info!("MCP tools loaded successfully");
    }

    // Set system prompt - use command line prompt if provided, otherwise use config default
    match &cli.system_prompt {
        Some(system_prompt) => {
            agent.set_system_prompt(system_prompt.clone());
            println!("{} Using custom system prompt: {}", "✓".green(), system_prompt);
        }
        None => {
            // Use config's default system prompt if available
            if let Some(default_prompt) = &config.default_system_prompt {
                agent.set_system_prompt(default_prompt.clone());
                println!("{} Using default system prompt from config", "✓".green());
            }
        }
    }

    // Add context files
    add_context_files(&mut agent, &cli.context_files).await?;

    let is_interactive = cli.message.is_none() && !cli.non_interactive;

    if let Some(message) = cli.message {
        // Single message mode
        if cli.stream {
            let _response = agent.process_message_with_stream(&message, Some(|content| {
                print!("{}", content);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            })).await?;
            print_usage_stats(&agent);
        } else {
            let spinner = create_spinner();
            let response = agent.process_message(&message).await?;
            spinner.finish_and_clear();
            formatter.print_formatted(&response)?;
            print_usage_stats(&agent);
        }
    } else if cli.non_interactive {
        // Read from stdin
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        
        if cli.stream {
            let _response = agent.process_message_with_stream(&input.trim(), Some(|content| {
                print!("{}", content);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            })).await?;
            print_usage_stats(&agent);
        } else {
            let spinner = create_spinner();
            let response = agent.process_message(&input.trim()).await?;
            spinner.finish_and_clear();
            formatter.print_formatted(&response)?;
            print_usage_stats(&agent);
        }
    } else {
        // Interactive mode
        println!("{}", "🤖 AI Agent - Interactive Mode".green().bold());
        println!("{}", "Type 'exit', 'quit', or '/exit' to quit. Type '/help' for available commands.".dimmed());
        println!();

        loop {
            let input: String = match Input::new()
                .with_prompt("> ")
                .allow_empty(false)
                .interact_text() {
                Ok(input) => input,
                Err(_) => {
                    // Handle EOF or input error gracefully
                    println!("\n{} End of input. Exiting...", "👋".blue());
                    break;
                }
            };

            // Check for slash commands first
            if input.starts_with('/') {
                match handle_slash_command(&input, &mut agent, &mcp_manager).await {
                    Ok(_) => {}, // Command handled successfully
                    Err(e) => {
                        eprintln!("{} Error handling command: {}", "✗".red(), e);
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

            // Show spinner while processing (only for non-streaming)
            if cli.stream {
                let result = agent.process_message_with_stream(&input, Some(|content| {
                    print!("{}", content);
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                })).await;
                
                match result {
                    Ok(_response) => {
                        println!();
                    }
                    Err(e) => {
                        eprintln!("{}: {}", "Error".red(), e);
                        println!();
                    }
                }
            } else {
                let spinner = create_spinner();
                let result = agent.process_message(&input).await;
                spinner.finish_and_clear();
                
                match result {
                    Ok(response) => {
                        // Only print response if it's not empty (i.e., not just @file references)
                        if !response.is_empty() {
                            formatter.print_formatted(&response)?;
                        }
                        println!();
                    }
                    Err(e) => {
                        eprintln!("{}: {}", "Error".red(), e);
                        println!();
                    }
                }
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

    Ok(())
}