use clap::Parser;
use colored::*;
use anyhow::Result;
use std::io::{self, Read};
use dialoguer::Input;
use log::{info, error};
use env_logger::Builder;

mod config;
mod anthropic;
mod tools;
mod agent;
mod formatter;

use config::Config;
use agent::Agent;
use formatter::create_code_formatter;

/// Handle slash commands
fn handle_slash_command(command: &str, agent: &Agent) -> Result<bool> {
    let parts: Vec<&str> = command.trim().splitn(2, ' ').collect();
    let cmd = parts[0];
    
    match cmd {
        "/help" => {
            print_help();
            Ok(true) // Command was handled
        }
        "/exit" | "/quit" => {
            println!("{}", "Goodbye! 👋".green());
            std::process::exit(0);
        }
        _ => {
            println!("{} Unknown command: {}. Type /help for available commands.", "⚠️".yellow(), cmd);
            Ok(true) // Command was handled (as unknown)
        }
    }
}

/// Print help information
fn print_help() {
    println!("{}", "🤖 AI Agent - Slash Commands".cyan().bold());
    println!();
    println!("{}", "Available commands:".green().bold());
    println!("  /help     - Show this help message");
    println!("  /exit     - Exit the program");
    println!("  /quit     - Exit the program");
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

    if let Some(message) = cli.message {
        // Single message mode
        let response = agent.process_message(&message).await?;
        formatter.print_formatted(&response)?;
    } else if cli.non_interactive {
        // Read from stdin
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let response = agent.process_message(&input.trim()).await?;
        formatter.print_formatted(&response)?;
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
                let _ = handle_slash_command(&input, &agent);
                continue;
            }

            // Check for traditional exit commands
            if input == "exit" || input == "quit" {
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

    Ok(())
}