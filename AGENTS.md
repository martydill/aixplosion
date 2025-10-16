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
8. **System Prompts**: Set custom system prompts to control AI behavior and personality
9. **Streaming Support**: Real-time response streaming for immediate feedback

### Available Tools

- **read_file**: Read the contents of a file
- **write_file**: Write content to a file (creates if doesn't exist)
- **edit_file**: Replace specific text in a file with new text
- **list_directory**: List contents of a directory
- **create_directory**: Create a directory (and parent directories if needed)
- **delete_file**: Delete a file or directory
- **bash**: Execute shell commands and return the output

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

# With system prompts (NEW!)
ai-agent -s "You are a Rust expert" "Help me with this code"
ai-agent -s "Act as a code reviewer" -f main.rs "Review this code"
ai-agent -s "You are a helpful assistant" "Explain this concept"

# With streaming support (NEW!)
ai-agent --stream -m "Tell me a story"
ai-agent --stream --non-interactive < input.txt
ai-agent --stream  # Interactive mode with streaming
```

### System Prompts

System prompts allow you to control the AI's behavior, personality, and response style. They are set at the beginning of the conversation and influence all subsequent responses.

#### System Prompt Examples

```bash
# Set the AI to act as a specific expert
ai-agent -s "You are a senior Rust developer with 10 years of experience" "Review this code"

# Set a specific response style
ai-agent -s "Respond in a concise, technical manner" "Explain distributed systems"

# Set a specific context or role
ai-agent -s "You are a code reviewer. Focus on security, performance, and maintainability" -f app.rs "Review this file"

# Multiple instructions
ai-agent -s "You are a helpful coding assistant. Always provide code examples and explain your reasoning" "How do I implement a binary tree in Rust?"
```

#### When to Use System Prompts

- **Code Review**: Set the AI to act as a senior developer reviewing code
- **Learning**: Set the AI to act as a teacher explaining concepts
- **Specific Domains**: Set the AI as an expert in a particular field
- **Response Style**: Control how detailed, technical, or casual the responses should be
- **Context Setting**: Provide background information that should influence all responses

### Shell Command Execution

The agent can execute shell commands directly, allowing you to:

- List directory contents
- Check git status
- Run tests and build processes
- Execute any shell command
- Get system information

#### Command Examples
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

#### Platform Support

The shell command tool automatically detects the operating system and uses the appropriate shell:
- **Windows**: Uses `cmd.exe /C` for command execution
- **Unix/Linux/macOS**: Uses `bash -c` for command execution

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

### Streaming Support

The agent now supports streaming responses for real-time feedback as the AI generates its response:

- **Real-time Output**: See responses as they're being generated
- **Reduced Perceived Latency**: No waiting for complete response
- **Visual Feedback**: Immediate indication that the system is working
- **Backward Compatible**: Existing functionality unchanged
- **Optional**: Can be enabled via `--stream` flag

#### Streaming Examples
```bash
# Enable streaming for single message
ai-agent --stream -m "Tell me a story"

# Enable streaming for stdin
echo "Explain quantum computing" | ai-agent --stream --non-interactive

# Enable streaming in interactive mode
ai-agent --stream

# Compare streaming vs non-streaming
ai-agent -m "What's the weather like?"  # Shows spinner, then formatted response
ai-agent --stream -m "What's the weather like?"  # Shows real-time response
```

#### When to Use Streaming
- **Long Responses**: Better experience for detailed explanations
- **Interactive Sessions**: More natural conversation flow
- **Real-time Needs**: When you need immediate feedback
- **Scripting**: Better for pipelines where you want immediate output

#### When to Use Non-Streaming
- **Short Responses**: Spinner provides better UX for quick responses
- **Formatted Output**: Non-streaming mode applies syntax highlighting
- **Debugging**: Easier to capture complete response for troubleshooting

### Slash Commands

In interactive mode, you can use these commands:

- `/help` - Show help information
- `/stats` - Show token usage statistics
- `/usage` - Show token usage statistics (alias for /stats)
- `/context` - Show current conversation context (including system prompt)
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
- Streaming connection failures (graceful fallback to non-streaming)

All errors are displayed with clear, actionable messages to help troubleshoot issues.

### Configuration for Streaming

Streaming can be enabled via:
1. **Command Line Flag**: Use `--stream` flag
2. **Default Behavior**: Non-streaming remains the default for backward compatibility
3. **Mode Support**: Available in all modes (single message, non-interactive, interactive)

#### Streaming Configuration Examples
```bash
# Per-request streaming
ai-agent --stream -m "Your message"

# Interactive mode with streaming
ai-agent --stream

# Non-interactive with streaming
cat input.txt | ai-agent --stream --non-interactive

# Combine with other options
ai-agent --stream -s "You are an expert" -f context.txt "Analyze this"
```