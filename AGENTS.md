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
4. **Tool Support**: Execute various tools for file operations, code analysis, bash commands, etc.
5. **Context Management**: Maintains conversation history
6. **@file Syntax**: Auto-include files using @path-to-file syntax
7. **Progress Spinner**: Visual feedback while waiting for LLM responses

### Available Tools

- **read_file**: Read the contents of a file
- **write_file**: Write content to a file (creates if doesn't exist)
- **edit_file**: Replace specific text in a file with new text
- **list_directory**: List contents of a directory
- **create_directory**: Create a directory (and parent directories if needed)
- **delete_file**: Delete a file or directory
- **bash**: Execute bash commands and return the output

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

### Bash Command Execution

The agent can execute bash commands directly, allowing you to:

- List directory contents
- Check git status
- Run tests and build processes
- Execute any shell command
- Get system information

#### Bash Examples
```bash
# List files in current directory
ai-agent "List the files in the current directory"

# Check git status
ai-agent "Check the git status"

# Run tests
ai-agent "Run tests and show me the results"

# Execute multiple commands
ai-agent "Check the current branch and run the build process"
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

### Progress Spinner

The agent now includes a visual progress spinner that appears while waiting for LLM responses. The spinner provides immediate feedback that the system is processing your request:

- **Spinner Characters**: Rotating Unicode characters (⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏)
- **Message**: Shows "Thinking..." while processing
- **Color**: Green spinner with clear visibility
- **Behavior**: Automatically clears when the response is ready

The spinner appears in all modes:
- Interactive mode (during conversation)
- Single message mode
- Non-interactive mode (stdin)

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
- Bash command execution failures
- Invalid file references in @file syntax

All errors are displayed with clear, actionable messages to help troubleshoot issues.