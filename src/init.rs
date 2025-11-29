use crate::agent::Agent;
use crate::config::Config;
use crate::database::get_database_path;
use crate::security::FilePermissionResult;
use anyhow::{bail, Context, Result};
use chrono::Local;
use path_absolutize::Absolutize;
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};
use tokio::fs;

struct ProjectMetadata {
    name: String,
    version: String,
}

/// Analyze the current project and write AGENTS.md with a project brief.
pub async fn run_init(agent: &Agent) -> Result<PathBuf> {
    let metadata = read_cargo_metadata()
        .await
        .context("Failed to read Cargo.toml for project metadata")?;

    let summary = read_readme_summary()
        .await
        .unwrap_or_else(|| "Rust-based CLI coding agent with interactive, single-message, and non-interactive modes plus built-in tooling for working in your repo.".to_string());

    let features = extract_readme_features()
        .await
        .unwrap_or_else(default_feature_list);

    let architecture = collect_architecture_overview();
    let build_and_run = build_and_run_commands();
    let config_notes = configuration_notes();
    let data_notes = data_and_context_notes()?;

    let generated_at = Local::now();
    let content = render_agents_md(
        &metadata,
        &summary,
        &features,
        &architecture,
        &build_and_run,
        &config_notes,
        &data_notes,
        generated_at,
    );

    let output_path = PathBuf::from("AGENTS.md");
    write_agents_file(agent, &output_path, &content).await?;

    Ok(output_path)
}

async fn read_cargo_metadata() -> Result<ProjectMetadata> {
    let content = fs::read_to_string("Cargo.toml")
        .await
        .context("Unable to read Cargo.toml")?;

    let parsed: toml::Value = toml::from_str(&content).context("Invalid Cargo.toml")?;
    let package = parsed
        .get("package")
        .and_then(|p| p.as_table())
        .context("Missing [package] section in Cargo.toml")?;

    let name = package
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let version = package
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    Ok(ProjectMetadata { name, version })
}

async fn read_readme_summary() -> Option<String> {
    let content = fs::read_to_string("README.md").await.ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        return Some(trimmed.to_string());
    }
    None
}

async fn extract_readme_features() -> Option<Vec<String>> {
    let content = fs::read_to_string("README.md").await.ok()?;
    let mut in_features = false;
    let mut features = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") && in_features {
            break;
        }
        if trimmed.eq_ignore_ascii_case("## features") {
            in_features = true;
            continue;
        }
        if in_features && (trimmed.starts_with('-') || trimmed.starts_with('*')) {
            let item = trimmed
                .trim_start_matches(['-', '*'])
                .trim()
                .to_string();
            if !item.is_empty() {
                features.push(item);
            }
        }
    }

    if features.is_empty() {
        None
    } else {
        Some(features)
    }
}

fn default_feature_list() -> Vec<String> {
    vec![
        "Interactive, single-message, and non-interactive modes with streaming or spinner-based output."
            .to_string(),
        "Built-in bash, file editing, directory listing, search, and syntax-highlighted formatter."
            .to_string(),
        "Context management with AGENTS.md auto-loading and @file or -f/--file inclusion."
            .to_string(),
        "MCP (Model Context Protocol) tooling support with connect/list/reconnect flows."
            .to_string(),
        "Security layers for shell and file operations with allow/deny patterns plus YOLO bypass."
            .to_string(),
        "SQLite-backed per-project conversation history with search/resume support and usage tracking."
            .to_string(),
    ]
}

fn collect_architecture_overview() -> Vec<String> {
    let module_descriptions = vec![
        ("src/main.rs", "CLI entrypoint using clap; handles interactive loop, streaming/non-streaming processing, slash commands, and ESC cancellation."),
        ("src/agent.rs", "Orchestrates Anthropic calls, tool execution, MCP integration, token accounting, and security-gated file/bash operations."),
        ("src/anthropic.rs", "HTTP client wrapper for Anthropic Messages API with streaming support and tool definition wiring."),
        ("src/tools.rs", "Built-in tools for listing directories, reading/searching files, writing/editing/deleting files, creating directories, and bridging MCP tools."),
        ("src/conversation.rs", "Conversation manager for context, AGENTS.md retention, and database-backed message storage."),
        ("src/database.rs", "SQLite layer storing conversations, messages, and usage stats in a per-project database."),
        ("src/input.rs", "Interactive input reader with syntax highlighting, history, cancellation, and multi-line support."),
        ("src/autocomplete.rs", "Tab completion for slash commands and @file paths."),
        ("src/formatter.rs", "Output formatter with syntax highlighting, streaming renderer, and safe truncation."),
        ("src/tool_display.rs", "Pretty/pragmatic display helpers for tool call progress and results."),
        ("src/security.rs", "Security policy for bash/file operations with ask/allow/deny and session-level overrides."),
        ("src/mcp.rs", "Model Context Protocol client for managing configured servers and surfacing their tools."),
        ("src/config.rs", "Config loader/saver with env overrides for API keys, defaults, and security settings."),
        ("src/logo.rs", "Startup branding used in interactive mode."),
    ];

    module_descriptions
        .into_iter()
        .filter(|(path, _)| Path::new(path).exists())
        .map(|(_, desc)| desc.to_string())
        .collect()
}

fn build_and_run_commands() -> Vec<String> {
    vec![
        "cargo build".to_string(),
        "cargo test".to_string(),
        "cargo run -- --help  # Inspect CLI options, including streaming and non-interactive flags"
            .to_string(),
        "cargo run -- --stream  # Interactive mode with streaming responses".to_string(),
        "cargo run -- -m \"Hello\"  # Single-message mode".to_string(),
        "cargo run -- --non-interactive --stream < input.txt  # Read from stdin without prompts"
            .to_string(),
    ]
}

fn configuration_notes() -> Vec<String> {
    vec![
        format!(
            "Configuration file: {} (API keys excluded; env vars override).",
            Config::default_config_path().display()
        ),
        "Environment: set ANTHROPIC_AUTH_TOKEN (required) and optionally ANTHROPIC_BASE_URL."
            .to_string(),
        "System prompt can be provided via --system or defaults to config; AGENTS.md files are auto-included as context."
            .to_string(),
        "Use /permissions and /file-permissions to manage security; pass --yolo to bypass checks (not recommended)."
            .to_string(),
        "Manage MCP servers with /mcp add/list/connect/tools; definitions come from the config file's [mcp] section."
            .to_string(),
    ]
}

fn data_and_context_notes() -> Result<Vec<String>> {
    let db_path = get_database_path()?;
    Ok(vec![
        format!(
            "Per-project SQLite database stored at {} for conversations, messages, and usage stats.",
            db_path.display()
        ),
        "Context automatically loads AGENTS.md from ~/.aixplosion/ (priority), then repo-root AGENTS.md when present."
            .to_string(),
        "Add more context with -f/--file flags or @path references in messages; messages containing only @files do not call the API."
            .to_string(),
        "Conversation history can be searched with /search and resumed with /resume; clearing keeps AGENTS.md context available."
            .to_string(),
    ])
}

fn render_agents_md(
    metadata: &ProjectMetadata,
    summary: &str,
    features: &[String],
    architecture: &[String],
    build_and_run: &[String],
    config_notes: &[String],
    data_notes: &[String],
    generated_at: chrono::DateTime<Local>,
) -> String {
    let mut output = String::new();

    writeln!(output, "# AIxplosion Project Guide").unwrap();
    writeln!(output, "").unwrap();
    writeln!(
        output,
        "- Generated: {}",
        generated_at.format("%Y-%m-%d %H:%M:%S")
    )
    .unwrap();
    writeln!(output, "").unwrap();
    writeln!(output, "## Overview").unwrap();
    writeln!(
        output,
        "- Name: {} v{}",
        metadata.name, metadata.version
    )
    .unwrap();
    writeln!(output, "- Summary: {}", summary).unwrap();
    writeln!(output, "").unwrap();

    writeln!(output, "## Key Features").unwrap();
    if features.is_empty() {
        writeln!(output, "- Not available").unwrap();
    } else {
        for item in features {
            writeln!(output, "- {}", item).unwrap();
        }
    }
    writeln!(output, "").unwrap();

    writeln!(output, "## Architecture").unwrap();
    if architecture.is_empty() {
        writeln!(output, "- Source layout not detected.").unwrap();
    } else {
        for item in architecture {
            writeln!(output, "- {}", item).unwrap();
        }
    }
    writeln!(output, "").unwrap();

    writeln!(output, "## Build & Run").unwrap();
    for cmd in build_and_run {
        writeln!(output, "- {}", cmd).unwrap();
    }
    writeln!(output, "").unwrap();

    writeln!(output, "## Configuration & Security").unwrap();
    for note in config_notes {
        writeln!(output, "- {}", note).unwrap();
    }
    writeln!(output, "").unwrap();

    writeln!(output, "## Data & Context").unwrap();
    for note in data_notes {
        writeln!(output, "- {}", note).unwrap();
    }
    writeln!(output, "").unwrap();

    output
}

async fn write_agents_file(agent: &Agent, output_path: &Path, content: &str) -> Result<()> {
    let absolute_path = output_path.absolutize()?.to_path_buf();

    if !agent.is_yolo_mode() {
        let mut manager = agent.get_file_security_manager().write().await;
        match manager.check_file_permission("write_file", &absolute_path.to_string_lossy()) {
            FilePermissionResult::Allowed => {}
            FilePermissionResult::Denied => {
                bail!(
                    "Security policy denied writing to {}",
                    absolute_path.display()
                );
            }
            FilePermissionResult::RequiresPermission => {
                let granted = manager
                    .ask_file_permission("write_file", &absolute_path.to_string_lossy())
                    .await?;
                if granted.unwrap_or(false) {
                    // Permission granted, proceed
                } else {
                    bail!(
                        "Permission denied for writing to {}",
                        absolute_path.display()
                    );
                }
            }
        }
    }

    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent)
            .await
            .context("Failed to create parent directory for AGENTS.md")?;
    }

    fs::write(&absolute_path, content)
        .await
        .with_context(|| format!("Failed to write {}", absolute_path.display()))?;

    Ok(())
}
