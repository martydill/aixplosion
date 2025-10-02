use clap::Parser;
use colored::*;
use anyhow::Result;
use std::io::{self, Read};
use dialoguer::Input;
use log::info;
use env_logger::Builder;
use std::path::Path;

mod config;
mod anthropic;
mod tools;
mod agent;
mod formatter;

use config::Config;
use agent::Agent;
use formatter::create_code_formatter;

/// Check for and add context files
    async fn add_context_files(agent: &mut Agent, context_files: &[String]) -> Result<()> {
        if context_files.is_empty() {
            // Automatically add AGENTS.md if it exists
            if Path::new("AGENTS.md").exists() {
                info!("Auto-adding AGENTS.md as context");
                agent.add_context_file("AGENTS.md").await?;
            }
            return Ok(());
        }

        for file_path in context_files {
            info!("Adding context file: {}", file_path);
            match agent.add_context_file(file_path).await {
                Ok(_) => println!("{} Added context file: {}", "✓".green(), file_path),
                Err(e) => eprintln!("{} Failed to add context file '{}': {}", "✗".red(), file_path, e),
            }
        }

        Ok(())
    }
fn handle_slash_command(command: &str, agent: &mut Agent) -> Result<bool> {
    let parts: Vec<&str> = command.trim().splitn(2, ' ').collect();
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
        "/reset-stats" => {
            agent.reset_token_usage();
            println!("{}", "📊 Token usage statistics reset!".green());
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

/// Print help information
fn print_help() {
    println!("{}", "🤖 AI Agent - Slash Commands".cyan().bold());
    println!();
    println!("{}", "Available commands:".green().bold());
    println!("  /help         - Show this help message");
    println!("  /stats        - Show token usage statistics");
    println!("  /usage        - Show token usage statistics (alias for /stats)");
    println!("  /context      - Show current conversation context");
    println!("  /reset-stats  - Reset token usage statistics");
    println!("  /exit         - Exit the program");
    println!("  /quit         - Exit the program");
    println!();
    println!("{}", "Context Files:".green().bold());
    println!("  Use -f or --file to include files as context");
    println!("  AGENTS.md is automatically included if it exists");
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

  }

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Cli::parse();
    info!("Starting AI Agent with model: {}", cli.model);

    // Load configuration
    let mut config = Config::load(cli.config.as_deref()).await?;

    // Override API key if provided via command line
    if let Some(api_key) = cli.api_key {
        config.api_key = api_key;
    }

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
    let mut agent = Agent::new(config, cli.model);

    // Add context files
    add_context_files(&mut agent, &cli.context_files).await?;

    let is_interactive = cli.message.is_none() && !cli.non_interactive;

    if let Some(message) = cli.message {
        // Single message mode
        let response = agent.process_message(&message).await?;
        formatter.print_formatted(&response)?;

        // Print usage stats for single message mode
        print_usage_stats(&agent);
    } else if cli.non_interactive {
        // Read from stdin
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let response = agent.process_message(&input.trim()).await?;
        formatter.print_formatted(&response)?;

        // Print usage stats for non-interactive mode
        print_usage_stats(&agent);
    } else {
        // Interactive mode
        println!("{}", "🤖 AI Agent - Interactive Mode".green().bold());
        println!("{}", "Type 'exit', 'quit', or '/exit' to quit. Type '/help' for available commands.".dimmed());
        println!();

        loop {
            let input: String = Input::new()
                .with_prompt("> ")
                .allow_empty(false)
                .interact_text()?;

            // Check for slash commands first
            if input.starts_with('/') {
                let _ = handle_slash_command(&input, &mut agent);
                continue;
            }

            // Check for traditional exit commands
            if input == "exit" || input == "quit" {
                // Print final stats before exiting
                print_usage_stats(&agent);
                println!("{}", "Goodbye! 👋".green());
                break;
            }

            match agent.process_message(&input).await {
                Ok(response) => {
                    formatter.print_formatted(&response)?;
                    println!();
                }
                Err(e) => {
                    eprintln!("{}: {}", "Error".red(), e);
                    println!();
                }
            }
        }
    }

  // Print final usage stats before exiting (only for interactive mode)
    if is_interactive {
        print_usage_stats(&agent);
    }

    Ok(())
}