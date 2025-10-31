# AIxplosions Documentation

This file contains documentation about the available AIxplosions in this project.

## Agent Configuration

### Default Agent
- **Model**: glm-4.6
- **Temperature**: 0.7
- **Max Tokens**: 4096

### Agent Capabilities

The AIxplosion supports the following features:

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
aixplosion

# Single message
aixplosion -m "Hello, how are you?"

# Read from stdin
echo "Help me understand this code" | aixplosion --non-interactive

# With API key via command line
aixplosion -k "your-api-key" -m "Your message here"

# With API key from environment variable (RECOMMENDED)
export ANTHROPIC_AUTH_TOKEN="your-api-key"
aixplosion -m "Your message here"

# With context files
aixplosion -f config.toml -f Cargo.toml "Explain this project"

# Using @file syntax (NEW!)
aixplosion "What does @Cargo.toml contain?"
aixplosion "Compare @src/main.rs and @src/lib.rs"
aixplosion "@file1.txt @file2.txt"

# With system prompts (NEW!)
aixplosion -s "You are a Rust expert" "Help me with this code"
aixplosion -s "Act as a code reviewer" -f main.rs "Review this code"
aixplosion -s "You are a helpful assistant" "Explain this concept"

# With streaming support (NEW!)
aixplosion --stream -m "Tell me a story"
aixplosion --stream --non-interactive < input.txt
aixplosion --stream  # Interactive mode with streaming

# Interactive mode examples
aixplosion
> !dir                    # List directory contents
> !git status             # Check git status
> !cargo build            # Build the project
> /help                   # Show available commands
> /permissions allow "git *"  # Allow git commands
```

### System Prompts

System prompts allow you to control the AI's behavior, personality, and response style. They are set at the beginning of the conversation and influence all subsequent responses.

#### System Prompt Examples

```bash
# Set the AI to act as a specific expert
aixplosion -s "You are a senior Rust developer with 10 years of experience" "Review this code"

# Set a specific response style
aixplosion -s "Respond in a concise, technical manner" "Explain distributed systems"

# Set a specific context or role
aixplosion -s "You are a code reviewer. Focus on security, performance, and maintainability" -f app.rs "Review this file"

# Multiple instructions
aixplosion -s "You are a helpful coding assistant. Always provide code examples and explain your reasoning" "How do I implement a binary tree in Rust?"
```

#### When to Use System Prompts

- **Code Review**: Set the AI to act as a senior developer reviewing code
- **Learning**: Set the AI to act as a teacher explaining concepts
- **Specific Domains**: Set the AI as an expert in a particular field
- **Response Style**: Control how detailed, technical, or casual the responses should be
- **Context Setting**: Provide background information that should influence all responses

### Shell Command Execution

The agent can execute shell commands directly using two different methods:

#### 1. AI-Executed Commands
The agent can automatically execute shell commands when you ask it to:

```bash
# List files in current directory
aixplosion "List the files in the current directory"

# Check git status
aixplosion "Check the git status"

# Run tests
aixplosion "Run tests and show me the results"

# Execute multiple commands
aixplosion "Check the current branch and run the build process"
```

#### 2. Direct Shell Commands (!)
In interactive mode, you can use `!` commands to execute shell commands directly:

```bash
# Start interactive mode
aixplosion

# Then use shell commands directly
> !dir
> !ls -la
> !git status
> !cargo build
> !cargo test
> !pwd
> !ps aux
```

#### Security for Shell Commands
**Important distinction** between AI-executed and direct shell commands:

- **AI-Executed Commands**: Subject to security permissions, allowlist/denylist checks
- **Direct Shell Commands (!)**: Execute immediately without permission checks for full user control

Use `/permissions` to manage security settings for AI-executed commands only. Direct `!` commands provide unrestricted shell access.

#### Platform Support

The shell command tool automatically detects the operating system and uses the appropriate shell:
- **Windows**: Uses `cmd.exe /C` for command execution
- **Unix/Linux/macOS**: Uses `bash -c` for command execution

### Configuration

The agent can be configured via:

1. **Environment Variables**:
   - `ANTHROPIC_AUTH_TOKEN`: Your API key (required)
   - `ANTHROPIC_BASE_URL`: Custom base URL (default: https://api.anthropic.com/v1)

2. **Command Line**:
   - `-k` or `--api-key`: Set API key via command line

3. **Config File**: Located at `~/.config/aixplosion/config.toml` (API keys are excluded for security)

⚠️ **Security Note**: API keys are **never** stored in config files for security reasons. Always use environment variables or command line flags.

Example config file (API key excluded):
```toml
base_url = "https://api.anthropic.com/v1"
default_model = "glm-4.6"
max_tokens = 4096
temperature = 0.7
```

#### API Key Security Best Practices
- **Use environment variables** for API keys (recommended)
- **Use command line flag `-k`** for temporary API keys
- **Never commit API keys** to version control
- **API keys are automatically excluded** from config files
- **Use `.env` files** for local development (add to .gitignore)

### Context Files

The agent supports multiple ways to include files as context:

1. **Command Line Flag**: Use `-f` or `--file` to specify files
2. **@file Syntax**: Use `@path-to-file` directly in messages
3. **Auto-inclusion**: AGENTS.md is automatically included if it exists

#### @file Syntax Examples
```bash
# Single file
aixplosion "What does @config.toml contain?"

# Multiple files
aixplosion "Compare @file1.rs and @file2.rs"

# File with question
aixplosion "Explain the Rust code in @src/main.rs"

# Only file references
aixplosion "@file1.txt @file2.txt"
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
aixplosion --stream -m "Tell me a story"

# Enable streaming for stdin
echo "Explain quantum computing" | aixplosion --stream --non-interactive

# Enable streaming in interactive mode
aixplosion --stream

# Compare streaming vs non-streaming
aixplosion -m "What's the weather like?"  # Shows spinner, then formatted response
aixplosion --stream -m "What's the weather like?"  # Shows real-time response
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

### Shell Commands (!)

In interactive mode, you can use `!` commands to execute shell commands directly:

- `!<command>` - Execute a shell command and display the output
- Examples: `!dir`, `!ls -la`, `!git status`, `!cargo test`
- **Note**: Shell commands with `!` bypass all security permissions for unrestricted access

#### Shell Command Examples
```bash
# List directory contents
!dir

# List files with details (Unix)
!ls -la

# Check git status
!git status

# Build project
!cargo build

# Run tests
!cargo test

# Show current directory
!pwd

# List processes
!ps aux

# Any command executes without permission checks
!sudo apt update
!rm -rf /tmp/*
!chmod +x script.sh
```

#### Security for Shell Commands
Direct shell commands (`!`) bypass all security restrictions:
- **No permission checks**: Commands execute immediately
- **No allowlist/denylist**: All commands are allowed
- **No interactive prompts**: No confirmation dialogs
- **User responsibility**: You have full control and responsibility

⚠️ **Warning**: `!` commands provide unrestricted shell access. Use with caution and only execute commands you trust.

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
aixplosion --stream -m "Your message"

# Interactive mode with streaming
aixplosion --stream

# Non-interactive with streaming
cat input.txt | aixplosion --stream --non-interactive

# Combine with other options
aixplosion --stream -s "You are an expert" -f context.txt "Analyze this"
```

#### Rules
 - Any time you create a doc, it must go in the docs folder. Any time you need to read a doc, look in the docs folder.