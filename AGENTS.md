# AI Agents Documentation

This file contains documentation about the available AI agents in this project.

## Agent Configuration

### Default Agent
- **Model**: glm-4.6
- **Temperature**: 0.7
- **Max Tokens**: 4096

### Agent Capabilities

The AI agent supports the following features:

1. **Interactive Mode**: Chat with the AI in a terminal interface
2. **Single Message Mode**: Send a single message and get a response
3. **Non-interactive Mode**: Read from stdin for scripting
4. **Tool Support**: Execute various tools for file operations, code analysis, etc.
5. **Context Management**: Maintains conversation history
6. **@file Syntax**: Auto-include files using @path-to-file syntax

### Available Tools

- **read_file**: Read the contents of a file
- **write_file**: Write content to a file (creates if doesn't exist)
- **edit_file**: Replace specific text in a file with new text
- **list_directory**: List contents of a directory
- **create_directory**: Create a directory (and parent directories if needed)
- **delete_file**: Delete a file or directory

### Usage Examples

```bash
# Interactive mode
ai-agent

# Single message
ai-agent -m "Hello, how are you?"

# Read from stdin
echo "Help me understand this code" | ai-agent --non-interactive

# With custom API key
ai-agent -k "your-api-key" -m "Your message here"

# With context files
ai-agent -f config.toml -f Cargo.toml "Explain this project"

# Using @file syntax (NEW!)
ai-agent "What does @Cargo.toml contain?"
ai-agent "Compare @src/main.rs and @src/lib.rs"
ai-agent "@file1.txt @file2.txt"
```

### Configuration

The agent can be configured via:

1. **Environment Variables**:
   - `ANTHROPIC_AUTH_TOKEN`: Your API key
   - `ANTHROPIC_BASE_URL`: Custom base URL (default: https://api.anthropic.com/v1)

2. **Config File**: Located at `~/.config/ai-agent/config.toml`

Example config file:
```toml
api_key = "your-api-key"
base_url = "https://api.anthropic.com/v1"
default_model = "glm-4.6"
max_tokens = 4096
temperature = 0.7
```

### Context Files

The agent supports multiple ways to include files as context:

1. **Command Line Flag**: Use `-f` or `--file` to specify files
2. **@file Syntax**: Use `@path-to-file` directly in messages
3. **Auto-inclusion**: AGENTS.md is automatically included if it exists

#### @file Syntax Examples
```bash
# Single file
ai-agent "What does @config.toml contain?"

# Multiple files
ai-agent "Compare @file1.rs and @file2.rs"

# File with question
ai-agent "Explain the Rust code in @src/main.rs"

# Only file references
ai-agent "@file1.txt @file2.txt"
```

### Slash Commands

In interactive mode, you can use these commands:

- `/help` - Show help information
- `/stats` - Show token usage statistics
- `/usage` - Show token usage statistics (alias for /stats)
- `/context` - Show current conversation context
- `/clear` - Clear all conversation context (keeps AGENTS.md if it exists)
- `/reset-stats` - Reset token usage statistics
- `/exit` or `/quit` - Exit the program

### Error Handling

The agent includes comprehensive error handling for:
- API authentication failures
- Network connectivity issues
- File operation errors
- Tool execution failures
- Invalid file references in @file syntax

All errors are displayed with clear, actionable messages to help troubleshoot issues.