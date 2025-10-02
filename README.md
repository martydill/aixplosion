# AI Agent

A fully-featured CLI coding agent powered by Anthropic's Claude AI with built-in file management tools.

## Features

- **Interactive Chat Mode**: Have conversations with Claude AI in your terminal
- **Tool Calling**: Claude can use built-in file system tools
- **File Management**: Built-in tools for reading, writing, editing, and deleting files
- **Configuration**: Persistent configuration with TOML files
- **Single Message Mode**: Send one-off messages and get responses
- **Non-interactive Mode**: Pipe input into the agent

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd ai-agent
```

2. Build the project:
```bash
cargo build --release
```

3. The binary will be available at `target/release/ai-agent`

## Configuration

The agent looks for configuration in the following order:
1. Command line `--config` argument
2. `~/.config/ai-agent/config.toml`
3. Environment variables
4. Default values

### Setting up API Key

Set your Anthropic API key using one of these methods:

1. **Environment variable** (recommended):
```bash
export ANTHROPIC_API_KEY="your-api-key-here"
```

2. **Configuration file**:
Create `~/.config/ai-agent/config.toml`:
```toml
api_key = "your-api-key-here"
default_model = "claude-3-sonnet-20240229"
max_tokens = 4096
temperature = 0.7
```

3. **Command line**:
```bash
ai-agent --api-key "your-api-key-here"
```

## Usage

### Interactive Mode (default)

```bash
ai-agent
```

Start chatting with the AI agent. Type `exit` or `quit` to leave.

### Single Message Mode

```bash
ai-agent --message "Write a hello world function in Rust"
```

### Non-interactive Mode

```bash
echo "Explain this code" | ai-agent --non-interactive
```

### Pipe File Content

```bash
cat main.rs | ai-agent --non-interactive
```

## Built-in Tools

The AI agent has access to the following file system tools:

- **list_directory**: List contents of a directory
- **read_file**: Read the contents of a file
- **write_file**: Write content to a file (creates if doesn't exist)
- **edit_file**: Replace specific text in a file with new text
- **delete_file**: Delete a file or directory
- **create_directory**: Create a directory (and parent directories if needed)

### Example Usage

```bash
# Start interactive mode
ai-agent

# In the chat, you can ask things like:
# "List the files in the current directory"
# "Read the contents of main.rs"
# "Create a new file called hello.rs with a hello world function"
# "Edit the hello.rs file to change the function name"
# "Delete the temp directory"
```

## Command Line Options

```
USAGE:
    ai-agent [OPTIONS]

OPTIONS:
    -m, --message <MESSAGE>         The message to send to the agent
    -k, --api-key <API_KEY>         Set the API key (overrides config file)
    -M, --model <MODEL>             Specify the model to use [default: claude-3-sonnet-20240229]
    -c, --config <CONFIG>           Configuration file path
    -n, --non-interactive          Run in non-interactive mode
    -h, --help                     Print help
    -V, --version                  Print version
```

## Development

1. Install Rust: https://rustup.rs/
2. Clone the repository
3. Run tests: `cargo test`
4. Run in development: `cargo run`

## Requirements

- Rust 1.70 or higher
- Anthropic API key
- Internet connection for API calls

## License

This project is licensed under the MIT License.