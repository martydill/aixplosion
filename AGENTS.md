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

### Slash Commands

In interactive mode, you can use these commands:

- `/help` - Show help information
- `/exit` or `/quit` - Exit the program

### Error Handling

The agent includes comprehensive error handling for:
- API authentication failures
- Network connectivity issues
- File operation errors
- Tool execution failures

All errors are displayed with clear, actionable messages to help troubleshoot issues.