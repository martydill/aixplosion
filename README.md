# AIxplosion

I used AI to build an AI coding agent and then used the AI coding agent to keep building the AI coding agent.

<img width="898" height="571" alt="image" src="https://github.com/user-attachments/assets/324128e6-9bcd-4e78-bb5a-a0657ed23d64" />

## Features
 - Interactive and non-interactive mode
 - Built-in file editing, bash, and code search tools
 - Syntax highlighting for code snippets
 - Direct bash command execution with !
 - Adding context files with @path_to_file_name
 - <tab> autocomplete for file paths and commands
 - MCP support
 - Local and global AGENTS.md support
 - Bash command and file editing security model with easy adding of wildcard versions to your allow list and sensible defaults
 - Yolo mode for living dangerously
 - Customizable system prompt
 - Conversation history stored in a per-project Sqlite DB
 - Session resuming via /resume
 - Full text conversation search via /search
 - Plan mode and /plan command support for managing plans and toggling plan mode
 - Subagent support via /agent
 

## Todo 
 - Git worktrees
 - Token speedometer
 - Hooks
 - Web search tool
 - Compacting
 - Pasting or referencing images
 - Custom slash commands
 - Tui support
 - Memory editing
 - Sandboxing 


## Usage
```
export ANTHROPIC_AUTH_TOKEN="your-api-key"
export ANTHROPIC_BASE_URL="https://api.z.ai/api/anthropic" 

cargo run -- --stream
```


## License

This project is licensed under the MIT License.
