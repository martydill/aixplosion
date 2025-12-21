use colored::*;
use std::thread;

/// Print agent subcommand help information
pub fn print_agent_help() {
    println!("{}", "🤖 Subagent Commands".cyan().bold());
    println!();
    println!("{}", "Management:".green().bold());
    println!("  /agent list                    - List all subagents");
    println!("  /agent create <name> <prompt> - Create new subagent");
    println!("  /agent delete <name> [--confirm] - Delete subagent");
    println!("  /agent edit <name>             - Edit subagent configuration");
    println!("  /agent reload                  - Reload configurations from disk");
    println!();
    println!("{}", "Usage:".green().bold());
    println!("  /agent use <name>              - Switch to subagent");
    println!("  /agent switch <name>           - Alias for use");
    println!("  /agent exit                    - Exit subagent mode");
    println!("  /agent                         - Show current status");
    println!();
    println!("{}", "Examples:".green().bold());
    println!("  /agent create rust-expert \"You are a Rust expert...\"");
    println!("  /agent use rust-expert");
    println!("  /agent list");
    println!("  /agent exit");
    println!();
}

/// Print MCP help information
pub fn print_mcp_help() {
    println!("{}", "🔌 MCP Commands".cyan().bold());
    println!();
    println!("{}", "Server Management:".green().bold());
    println!("  /mcp list                    - List all MCP servers and their status");
    println!("  /mcp add <name> stdio <cmd>  - Add a stdio MCP server");
    println!("  /mcp add <name> ws <url>     - Add a WebSocket MCP server");
    println!("  /mcp remove <name>           - Remove an MCP server");
    println!("  /mcp connect <name>          - Connect to a specific server");
    println!("  /mcp disconnect <name>       - Disconnect from a specific server");
    println!("  /mcp reconnect <name>        - Reconnect to a specific server");
    println!("  /mcp connect-all             - Connect to all enabled servers");
    println!("  /mcp disconnect-all          - Disconnect from all servers");
    println!();
    println!("{}", "Testing & Debugging:".green().bold());
    println!("  /mcp test <command>          - Test if a command is available");
    println!("  /mcp tools                   - List all available MCP tools");
    println!();
    println!("{}", "Examples:".green().bold());
    println!("  /mcp test npx                - Test if npx is available");
    println!("  /mcp add myserver stdio npx -y @modelcontextprotocol/server-filesystem");
    println!("  /mcp add websocket ws://localhost:8080");
    println!("  /mcp connect myserver");
    println!("  /mcp tools");
    println!();
}

/// Print file permissions help information
pub fn print_file_permissions_help() {
    println!("{}", "🔒 File Permissions Commands".cyan().bold());
    println!();
    println!("{}", "View File Permissions:".green().bold());
    println!(
        "  /file-permissions                - Show current file permissions and security settings"
    );
    println!("  /file-permissions show          - Alias for /file-permissions");
    println!("  /file-permissions list          - Alias for /file-permissions");
    println!("  /file-permissions help          - Show this help message");
    println!();
    println!("{}", "Testing:".green().bold());
    println!("  /file-permissions test <op> <path> - Test if file operation is allowed");
    println!("    Operations: write_file, edit_file, delete_file, create_directory");
    println!();
    println!("{}", "Security Settings:".green().bold());
    println!("  /file-permissions enable        - Enable file security");
    println!("  /file-permissions disable       - Disable file security");
    println!("  /file-permissions ask-on        - Enable asking for permission");
    println!("  /file-permissions ask-off       - Disable asking for permission");
    println!("  /file-permissions reset-session - Reset session permissions");
    println!();
    println!("{}", "Permission Options:".green().bold());
    println!("  When a file operation requires permission, you can choose:");
    println!("  • Allow this operation only - One-time permission");
    println!("  • Allow all file operations this session - Session-wide permission");
    println!("  • Deny this operation - Block the operation");
    println!();
    println!("{}", "Security Tips:".yellow().bold());
    println!("  • Enable 'ask for permission' for better security");
    println!("  • Use 'Allow this operation only' for one-off edits");
    println!("  • Use 'Allow all file operations this session' for trusted sessions");
    println!("  • File operations include: write_file, edit_file, create_directory, delete_file");
    println!("  • Read operations (read_file, list_directory) are always allowed");
    println!("  • Session permissions are reset when you restart the agent");
    println!();
    println!("{}", "Examples:".green().bold());
    println!("  /file-permissions test write_file /tmp/test.txt");
    println!("  /file-permissions enable");
    println!("  /file-permissions ask-on");
    println!("  /file-permissions reset-session");
    println!();
}

/// Print permissions help information
pub fn print_permissions_help() {
    println!("{}", "🔒 Permissions Commands".cyan().bold());
    println!();
    println!("{}", "View Permissions:".green().bold());
    println!("  /permissions                - Show current permissions and security settings");
    println!("  /permissions show          - Alias for /permissions");
    println!("  /permissions list          - Alias for /permissions");
    println!("  /permissions help          - Show this help message");
    println!();
    println!("{}", "Manage Allowlist:".green().bold());
    println!("  /permissions allow <cmd>    - Add command to allowlist");
    println!("  /permissions remove-allow <cmd> - Remove from allowlist");
    println!();
    println!("{}", "Manage Denylist:".green().bold());
    println!("  /permissions deny <cmd>     - Add command to denylist");
    println!("  /permissions remove-deny <cmd> - Remove from denylist");
    println!();
    println!("{}", "Security Settings:".green().bold());
    println!("  /permissions enable         - Enable bash security");
    println!("  /permissions disable        - Disable bash security");
    println!("  /permissions ask-on         - Enable asking for permission");
    println!("  /permissions ask-off        - Disable asking for permission");
    println!();
    println!("{}", "Testing:".green().bold());
    println!("  /permissions test <cmd>     - Test if a command is allowed");
    println!();
    println!("{}", "Pattern Matching:".green().bold());
    println!("  • Use wildcards: 'git *' allows all git commands");
    println!("  • Use exact match: 'cargo test' allows only that command");
    println!("  • Prefix matching: 'git' matches 'git status', 'git log', etc.");
    println!();
    println!("{}", "Examples:".green().bold());
    println!("  /permissions allow 'git *'  - Allow all git commands");
    println!("  /permissions deny 'rm *'    - Deny dangerous rm commands");
    println!("  /permissions test 'ls -la'  - Test if ls -la is allowed");
    println!("  /permissions enable         - Turn security on");
    println!("  /permissions ask-on         - Ask for unknown commands");
    println!();
    println!("{}", "Security Tips:".yellow().bold());
    println!("  • Be specific with allowlist entries for better security");
    println!("  • Use denylist for dangerous command patterns");
    println!("  • Enable 'ask for permission' for unknown commands");
    println!("  • Changes are automatically saved to config file");
    println!();
}

/// Print the main help message
pub fn print_help() {
    println!("{}", "🤖 AIxplosion - Slash Commands".cyan().bold());
    println!();
    println!("{}", "Available commands:".green().bold());
    println!("  /help         - Show this help message");
    println!("  /stats        - Show token usage statistics");
    println!("  /usage        - Show token usage statistics (alias for /stats)");
    println!("  /context      - Show current conversation context");
    println!("  /provider     - Show active LLM provider, model, and base URL");
    println!("  /search <q>   - Search previous conversations");
    println!("  /resume       - Resume a previous conversation");
    println!("  /clear        - Clear all conversation context (keeps AGENTS.md if it exists)");
    println!("  /reset-stats  - Reset token usage statistics");
    println!("  /permissions  - Manage bash command security permissions");
    println!("  /file-permissions  - Manage file operation security permissions");
    println!("  /mcp          - Manage MCP (Model Context Protocol) servers");
    println!("  /exit         - Exit the program");
    println!("  /quit         - Exit the program");
    println!();
    println!("{}", "Navigation:".green().bold());
    println!("  ↑ / ↓ Arrow   - Navigate through input history");
    println!("  ← / → Arrow   - Move cursor left/right in current input");
    println!("  Tab           - Auto-complete file paths and commands");
    println!("  Ctrl+R        - Start reverse history search (like readline)");
    println!("  ESC           - Cancel current AI conversation (during processing)");
    println!("  Ctrl+C        - Exit the program immediately");
    println!();
    println!("{}", "Reverse Search (Ctrl+R):".green().bold());
    println!("  Ctrl+R        - Start reverse search mode");
    println!("  Type text     - Search for matching history entries");
    println!("  Ctrl+R / r    - Find next match");
    println!("  ↑ / ↓ Arrow   - Navigate between matches");
    println!("  Enter         - Accept current match");
    println!("  ESC           - Cancel search and restore original input");
    println!("  Backspace     - Remove last character from search query");
    println!();
    println!("{}", "Shell Commands:".green().bold());
    println!("  !<command>    - Execute a shell command directly (bypasses all security)");
    println!("  Examples: !dir, !ls -la, !git status, !cargo test");
    println!("  Note: ! commands execute immediately without permission checks");
    println!();
    println!("{}", "Security Commands:".green().bold());
    println!("  /permissions              - Show current bash security settings");
    println!("  /file-permissions        - Show current file security settings");
    println!("  /permissions allow <cmd>  - Add command to allowlist");
    println!("  /permissions deny <cmd>   - Add command to denylist");
    println!("  /permissions test <cmd>  - Test if command is allowed");
    println!("  /file-permissions test <op> <path> - Test if file operation is allowed");
    println!("  /plan on|off             - Toggle plan mode at runtime");
    println!("  /plan run <id>           - Load and execute a saved plan by ID");
    println!();
    println!("{}", "MCP Commands:".green().bold());
    println!("  /mcp list                    - List MCP servers");
    println!("  /mcp add <name> stdio <cmd>  - Add stdio server");
    println!("  /mcp add <name> ws <url>     - Add WebSocket server");
    println!("  /mcp test <command>          - Test command availability");
    println!("  /mcp connect <name>          - Connect to server");
    println!("  /mcp tools                   - List available tools");
    println!("  /mcp help                    - Show MCP help");
    println!();
    println!("{}", "Context Files:".green().bold());
    println!("  Use -f or --file to include files as context");
    println!("  Use @path/to/file syntax in messages to auto-include files");
    println!("  AGENTS.md is automatically included from ~/.aixplosion/AGENTS.md (priority)");
    println!("  Falls back to ./AGENTS.md if home directory version doesn't exist");
    println!("  Messages with only @file references will NOT make API calls");
    println!();
    println!("{}", "System Prompts:".green().bold());
    println!("  Use -s or --system to set a custom system prompt");
    println!("  System prompts set the behavior and personality of the AI");
    println!();
    println!("{}", "Streaming:".green().bold());
    println!("  Use --stream flag to enable streaming responses");
    println!("  Streaming shows responses as they're generated (no spinner)");
    println!("  Non-streaming shows a spinner and formats the complete response");
    println!();
    println!("{}", "Plan Mode:".green().bold());
    println!("  Use --plan-mode to generate a read-only plan in Markdown");
    println!("  Plan mode disables mutating tools and saves the plan to the database");
    println!();
    println!("{}", "Examples:".green().bold());
    println!("  aixplosion -f config.toml \"Explain this configuration\"");
    println!("  aixplosion \"What does @Cargo.toml contain?\"");
    println!("  aixplosion \"Compare @file1.rs and @file2.rs\"");
    println!("  aixplosion \"@file1.txt @file2.txt\"  # Only adds context, no API call");
    println!("  aixplosion -s \"You are a Rust expert\" \"Help me with this code\"");
    println!("  aixplosion -s \"Act as a code reviewer\" -f main.rs \"Review this code\"");
    println!("  aixplosion --stream \"Tell me a story\"  # Stream the response");
    println!("  aixplosion --plan-mode \"Add Stripe billing\"  # Plan only, saves to DB");
    println!("  !dir                    # List directory contents");
    println!("  !git status             # Check git status");
    println!("  !cargo build            # Build the project");
    println!("  ESC                     # Cancel AI conversation during processing");
    println!();
    println!("{}", "History Navigation:".green().bold());
    println!("  • Press UP arrow to cycle through previous commands");
    println!("  • Press DOWN arrow to cycle through more recent commands");
    println!("  • Press Ctrl+R to start reverse history search");
    println!("  • Start typing to exit history navigation mode");
    println!("  • History is preserved across the entire session");
    println!("  • Duplicate and empty commands are not stored");
    println!();
    println!(
        "{}",
        "Any other input will be sent to the AIxplosion for processing.".dimmed()
    );
    println!();
}

/// Display a large red warning for yolo mode
pub fn display_yolo_warning() {
    println!();
    println!(
        "{}",
        "⚠️  WARNING: YOLO MODE ENABLED  ⚠️".red().bold().blink()
    );
    println!(
        "{}",
        " ALL SECURITY PERMISSIONS ARE BYPASSED - USE WITH EXTREME CAUTION "
            .red()
            .bold()
    );
    println!();
    println!(
        "{}",
        " • File operations (read/write/delete) will execute WITHOUT prompts ".red()
    );
    println!(
        "{}",
        " • Bash commands will execute WITHOUT permission checks ".red()
    );
    println!(
        "{}",
        " • MCP tools will execute WITHOUT security validation ".red()
    );
    println!(
        "{}",
        " • No allowlist/denylist filtering will be applied ".red()
    );
    println!("{}", " • All tool calls are automatically approved ".red());
    println!();
    println!(
        "{}",
        " 🚨 This mode can cause irreversible damage to your system! "
            .red()
            .bold()
    );
    println!();
    println!(
        "{}",
        " Press Ctrl+C NOW to cancel if this was not intended! "
            .red()
            .bold()
    );
    println!();

    // Add a dramatic pause for effect
    thread::sleep(std::time::Duration::from_millis(2000));
    println!(
        "{}",
        "🔥 Proceeding in YOLO mode... You have been warned! 🔥"
            .red()
            .bold()
    );
    println!();
}

/// Display YOLO mode warning after MCP configuration is complete
pub fn display_mcp_yolo_warning() {
    println!();
    println!(
        "{}",
        "🔌 MCP Configuration Complete - YOLO Mode Active 🔌"
            .red()
            .bold()
    );
    println!();
    println!(
        "{}",
        " ⚠️  MCP TOOLS WILL EXECUTE WITHOUT SECURITY VALIDATION ⚠️ "
            .red()
            .bold()
    );
    println!();
    println!(
        "{}",
        " • MCP server tools are now available and will execute WITHOUT prompts ".red()
    );
    println!(
        "{}",
        " • No permission checks will be applied to MCP tool calls ".red()
    );
    println!(
        "{}",
        " • All MCP operations (file access, commands, etc.) are auto-approved ".red()
    );
    println!(
        "{}",
        " • External MCP server connections have unrestricted access ".red()
    );
    println!();
    println!(
        "{}",
        " 🚨 MCP tools can potentially access and modify your system! "
            .red()
            .bold()
    );
    println!();
    println!(
        "{}",
        " 🔥 All MCP servers and their tools are operating in YOLO mode! 🔥"
            .red()
            .bold()
    );
    println!();
}
