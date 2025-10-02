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
        println!("{}", "Type 'exit' or press Ctrl+C to quit".dimmed());
        println!();

        loop {
            let input: String = Input::new()
                .with_prompt("> ")
                .allow_empty(false)
                .interact_text()?;

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

