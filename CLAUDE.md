# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Building and Running
- `cargo build --release` - Build optimized release binary
- `cargo run` - Run in development mode
- `cargo test` - Run tests
- `cargo check` - Check code without building

### Binary Usage
- `target/release/ai-agent` - Run interactive chat mode (default)
- `ai-agent --message "your message"` - Single message mode
- `ai-agent --non-interactive` - Pipe input from stdin
- `ai-agent --api-key "key"` - Override API key
- `ai-agent --model "model-name"` - Specify model

## Project Architecture

This is a Rust-based CLI coding agent that provides an interactive chat interface with Anthropic's Claude AI and built-in file system tools.

### Core Components

- **main.rs**: CLI entry point using clap for argument parsing, handles three modes:
  - Interactive mode (default)
  - Single message mode (--message)
  - Non-interactive mode (--non-interactive for stdin piping)

- **agent.rs**: Core conversation logic that manages:
  - Message history with Claude API
  - Tool calling loop (max 10 iterations to prevent infinite loops)
  - Conversation state management

- **tools.rs**: File system tool implementations:
  - list_directory, read_file, write_file, edit_file, delete_file, create_directory
  - Uses async handlers with JSON schema definitions
  - Tools are cloned via recreation due to function pointer limitations

- **anthropic.rs**: (not examined but handles API communication)

- **config.rs**: Configuration management:
  - Loads from `~/.config/ai-agent/config.toml` or custom path
  - Environment variable fallback: `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`
  - Default model: "glm-4.6" (not the standard Claude model name)

- **formatter.rs**: (not examined but handles response formatting)

### Key Technical Details

- Uses tokio async runtime throughout
- Tool calling uses a custom Tool struct with async handlers
- Configuration supports TOML files with environment variable overrides
- Built-in tools use absolute paths with tilde expansion
- Error handling with anyhow::Result
- Default API base URL: https://api.anthropic.com/v1

### Configuration Setup

The agent requires an API key set via:
1. Environment variable: `ANTHROPIC_AUTH_TOKEN`
2. Config file at `~/.config/ai-agent/config.toml`
3. Command line `--api-key` argument

Config file format:
```toml
api_key = "your-api-key-here"
base_url = "https://api.anthropic.com/v1"
default_model = "glm-4.6"
max_tokens = 4096
temperature = 0.7
```