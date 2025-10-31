# AIxplosion

I used AI to build an AI coding agent and then used the AI coding agent to keep building the AI coding agent.

<img width="844" height="514" alt="image" src="https://github.com/user-attachments/assets/de024e06-4fe5-4bc8-95bc-63ff25c9a21e" />

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
cd aixplosion
```

2. Build the project:
```bash
cargo build --release
```

3. The binary will be available at `target/release/aixplosion`

## Configuration

The agent looks for configuration in the following order:
1. Command line `--config` argument
2. `~/.config/aixplosion/config.toml`
3. Environment variables
4. Default values

### Setting up API Key

Set your Anthropic API key using one of these methods:

1. **Environment variable** (recommended):
```bash
export ANTHROPIC_API_KEY="your-api-key-here"
```

2. **Configuration file**:
Create `~/.config/aixplosion/config.toml`:
```toml
api_key = "your-api-key-here"
default_model = "claude-3-sonnet-20240229"
max_tokens = 4096
temperature = 0.7
```

3. **Command line**:
```bash
aixplosion --api-key "your-api-key-here"
```

## Usage

### Interactive Mode (default)

```bash
aixplosion
```

Start chatting with the AIxplosion. Type `exit` or `quit` to leave.

### Single Message Mode

```bash
aixplosion --message "Write a hello world function in Rust"
```

### Non-interactive Mode

```bash
echo "Explain this code" | aixplosion --non-interactive
```

### Pipe File Content

```bash
cat main.rs | aixplosion --non-interactive
```

## Built-in Tools

The AIxplosion has access to the following file system tools:

- **list_directory**: List contents of a directory
- **read_file**: Read the contents of a file
- **write_file**: Write content to a file (creates if doesn't exist)
- **edit_file**: Replace specific text in a file with new text
- **delete_file**: Delete a file or directory
- **create_directory**: Create a directory (and parent directories if needed)

### Example Usage

```bash
# Start interactive mode
aixplosion

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
    aixplosion [OPTIONS]

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
