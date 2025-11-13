use clap::Parser;
use colored::*;
use anyhow::Result;
use std::io::{self, Read, Write};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal,
    cursor,
    ExecutableCommand, QueueableCommand,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::Path;

use log::{debug, info, warn, error};
use env_logger::Builder;
use indicatif::{ProgressBar, ProgressStyle};

mod config;
mod anthropic;
mod tools;
mod agent;
mod formatter;
mod tool_display;
mod mcp;
mod security;
mod logo;
mod autocomplete;

#[cfg(test)]
mod formatter_tests;

use config::Config;
use agent::Agent;
use formatter::create_code_formatter;
use mcp::McpManager;

/// Input history management
struct InputHistory {
    entries: Vec<String>,
    index: Option<usize>,
    temp_input: String,
}

impl InputHistory {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: None,
            temp_input: String::new(),
        }
    }
    
    fn add_entry(&mut self, entry: String) {
        // Don't add empty entries or duplicates of the last entry
        if entry.trim().is_empty() {
            return;
        }
        
        if self.entries.is_empty() || self.entries.last() != Some(&entry) {
            self.entries.push(entry);
            // Limit history size to prevent memory issues
            if self.entries.len() > 1000 {
                self.entries.remove(0);
            }
        }
        
        // Reset navigation state
        self.index = None;
        self.temp_input.clear();
    }
    
    fn navigate_up(&mut self, current_input: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        
        match self.index {
            None => {
                // First time pressing up - save current input and go to last entry
                self.temp_input = current_input.to_string();
                self.index = Some(self.entries.len() - 1);
                Some(self.entries[self.entries.len() - 1].clone())
            }
            Some(index) => {
                if index > 0 {
                    // Move to previous entry in history
                    self.index = Some(index - 1);
                    Some(self.entries[index - 1].clone())
                } else {
                    // Already at the oldest entry
                    Some(self.entries[0].clone())
                }
            }
        }
    }
    
    fn navigate_down(&mut self) -> Option<String> {
        match self.index {
            None => None,
            Some(index) => {
                if index < self.entries.len() - 1 {
                    // Move to next entry in history
                    self.index = Some(index + 1);
                    Some(self.entries[index + 1].clone())
                } else {
                    // At the end of history - restore current input
                    self.index = None;
                    Some(self.temp_input.clone())
                }
            }
        }
    }
    
    fn reset_navigation(&mut self) {
        self.index = None;
        self.temp_input.clear();
    }
}

/// Read input with autocompletion support and file highlighting
fn read_input_with_completion_and_highlighting(
    formatter: Option<&formatter::CodeFormatter>, 
    history: &mut InputHistory
) -> Result<String> {
    // Enable raw mode for keyboard input
    terminal::enable_raw_mode()?;

    let mut input = String::new();
    let mut cursor_pos = 0;

    // Clear any previous input and display fresh prompt
    redraw_input_line_with_highlighting(&input, cursor_pos, formatter)?;

    loop {
        match event::read()? {
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            }) => {
                // Check if this might be the start of multiline input BEFORE disabling raw mode
                let trimmed_input = input.trim();
                if should_start_multiline(trimmed_input) {
                    // Disable raw mode first
                    terminal::disable_raw_mode()?;

                    // Clear the current line completely and move to start
                    io::stdout()
                        .execute(terminal::Clear(terminal::ClearType::CurrentLine))?
                        .execute(cursor::MoveToColumn(0))?
                        .flush()?;

                      // Show the prompt and what we've typed so far with file highlighting
                    if let Some(fmt) = formatter {
                        let highlighted_input = fmt.format_input_with_file_highlighting(trimmed_input);
                        println!("> {}", highlighted_input);
                    } else {
                        println!("> {}", trimmed_input);
                    }

                    // Start multiline input mode with the current input
                    let multiline_result = read_multiline_input(trimmed_input, None); // Don't double-highlight
                    return multiline_result;
                } else {
                    // Normal single line input - add to history
                    let trimmed_input = input.trim().to_string();
                    history.add_entry(trimmed_input.clone());
                    println!();
                    terminal::disable_raw_mode()?;
                    return Ok(trimmed_input);
                }
            }
            
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                kind: KeyEventKind::Press,
                ..
            }) => {
                // Handle tab completion
                if let Some(completion) = autocomplete::handle_tab_completion(&input) {
                    input = completion;
                    cursor_pos = input.len();
                    
                    // Redraw the line with highlighting after completion
                    redraw_input_line_with_highlighting(&input, cursor_pos, formatter)?;
                }
            }
            
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if !input.is_empty() && cursor_pos > 0 {
                    // Reset history navigation when user edits input
                    history.reset_navigation();

                    input.remove(cursor_pos - 1);
                    cursor_pos -= 1;

                    // Use fast redraw unless @ symbol is present
                    if input.contains('@') {
                        redraw_input_line_with_highlighting(&input, cursor_pos, formatter)?;
                    } else {
                        redraw_input_line_fast(&input, cursor_pos)?;
                    }
                }
            }
            
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if cursor_pos > 0 {
                    cursor_pos -= 1;
                    redraw_input_line_fast(&input, cursor_pos)?;
                }
            }

            Event::Key(KeyEvent {
                code: KeyCode::Right,
                kind: KeyEventKind::Press,
                ..
            }) => {
                if cursor_pos < input.len() {
                    cursor_pos += 1;
                    redraw_input_line_fast(&input, cursor_pos)?;
                }
            }

            Event::Key(KeyEvent {
                code: KeyCode::Up,
                kind: KeyEventKind::Press,
                ..
            }) => {
                // Handle up arrow - navigate to previous history entry
                if let Some(new_input) = history.navigate_up(&input) {
                    input = new_input;
                    cursor_pos = input.len();
                    // Use fast redraw for history navigation to avoid regex overhead
                    redraw_input_line_fast(&input, cursor_pos)?;
                }
            }

            Event::Key(KeyEvent {
                code: KeyCode::Down,
                kind: KeyEventKind::Press,
                ..
            }) => {
                // Handle down arrow - navigate to next history entry
                if let Some(new_input) = history.navigate_down() {
                    input = new_input;
                    cursor_pos = input.len();
                    // Use fast redraw for history navigation to avoid regex overhead
                    redraw_input_line_fast(&input, cursor_pos)?;
                }
            }
            
            Event::Key(KeyEvent { 
                code: KeyCode::Char(c), 
                kind: KeyEventKind::Press,
                modifiers: KeyModifiers::CONTROL,
                ..
            }) if c == 'c' => {
                // Handle Ctrl+C
                println!();
                terminal::disable_raw_mode()?;
                std::process::exit(0);
            }
            
            Event::Key(KeyEvent { 
                code: KeyCode::Esc, 
                kind: KeyEventKind::Press,
                ..
            }) => {
                // Handle ESC key - return cancellation signal
                println!();
                terminal::disable_raw_mode()?;
                return Err(anyhow::anyhow!("CANCELLED"));
            }
            
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                kind: KeyEventKind::Press,
                ..
            }) => {
                // Handle all character input, including spaces
                // Reset history navigation when user starts typing
                history.reset_navigation();

                input.insert(cursor_pos, c);
                cursor_pos += 1;

                // Use fast redraw for most typing, only use highlighting when @ symbol is present
                if input.contains('@') {
                    redraw_input_line_with_highlighting(&input, cursor_pos, formatter)?;
                } else {
                    redraw_input_line_fast(&input, cursor_pos)?;
                }
            }
            
            _ => {}
        }
    }
}

/// Determine if input should start multiline mode
fn should_start_multiline(input: &str) -> bool {
    let trimmed = input.trim();

    // Only start multiline for clear, intentional cases:
    // 1. Input starts with explicit code block marker (```language) but doesn't end with ```
    // 2. Input contains actual newlines
    // 3. Input is an incomplete quoted string with reasonable length (to avoid accidental triggers)

    // Explicit code block start - most reliable multiline indicator
    (trimmed.starts_with("```") && !trimmed.ends_with("```") && trimmed.len() > 3) ||
    // Already contains newlines (shouldn't happen in single line input, but just in case)
    (trimmed.contains('\n')) ||
    // Incomplete quoted strings, but only if they're reasonably long and look intentional
    ((trimmed.starts_with('"') || trimmed.starts_with('\'')) &&
     !trimmed.ends_with('"') && !trimmed.ends_with('\'') &&
     trimmed.len() > 10 && // Only if substantial content
     (trimmed.contains(',') || trimmed.contains('{') || trimmed.contains('('))) // Looks like code/data
}

/// Read multiline input in normal mode
fn read_multiline_input(initial_line: &str, formatter: Option<&formatter::CodeFormatter>) -> Result<String> {
    // Start with the first line that was already entered
    let mut lines = vec![initial_line.to_string()];

    // Check if we're in a code block
    let is_code_block = initial_line.trim().starts_with("```");
    let is_quoted = initial_line.trim().starts_with('"') || initial_line.trim().starts_with('\'');

    // If the initial line is complete (not starting a multiline structure), return it immediately
    if !is_code_block && !is_quoted && !initial_line.contains('\n') {
        return Ok(initial_line.to_string());
    }

    loop {
        print!("... ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => {
                // EOF - end of input
                break;
            }
            Ok(_) => {
                let line = line.trim_end().to_string();

                // For code blocks, check for ending marker
                if is_code_block && line.trim().ends_with("```") {
                    lines.push(line);
                    break;
                }

                // For quoted strings, check for closing quote
                if is_quoted && line.trim().ends_with('"') {
                    lines.push(line);
                    break;
                }

                // Empty line ends multiline input (unless we're in a code block)
                if line.is_empty() && !is_code_block {
                    break;
                }

                // Add the line and continue
                lines.push(line);
            }
            Err(_) => {
                // Handle EOF or input error gracefully
                println!("\n{} End of input", "👋".blue());
                break;
            }
        }
    }

    let final_input = lines.join("\n");
    
    // Display the complete multiline input with file highlighting if formatter is available
    if let Some(fmt) = formatter {
        let highlighted_input = fmt.format_input_with_file_highlighting(&final_input);
        // Display the multiline input with proper formatting
        let input_lines: Vec<&str> = highlighted_input.lines().collect();
        if input_lines.len() > 1 {
            for (i, line) in input_lines.iter().enumerate() {
                if i == 0 {
                    println!("> {}", line);
                } else {
                    println!("... {}", line);
                }
            }
        }
    }

    Ok(final_input)
}

/// Redraw the input line with file highlighting and proper cursor positioning
fn redraw_input_line_with_highlighting(input: &str, cursor_pos: usize, formatter: Option<&formatter::CodeFormatter>) -> Result<()> {
    use crossterm::{
        cursor::MoveToColumn,
        style::{Print, ResetColor},
        terminal::Clear,
    };

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Clear the current line and move to start
    stdout
        .queue(Clear(crossterm::terminal::ClearType::CurrentLine))?
        .queue(MoveToColumn(0))?;

    // Display prompt
    stdout.queue(Print("> "))?;

    // Store the current cursor position relative to the start of input text
    let prompt_length = 2; // "> " length

    // Only apply highlighting if formatter is available AND input contains @file references
    // This is a huge optimization - we avoid regex for most input
    if let Some(fmt) = formatter {
        if input.contains('@') {
            let highlighted_text = fmt.format_input_with_file_highlighting(input);
            stdout.queue(Print(highlighted_text))?;
        } else {
            stdout.queue(Print(input))?;
        }
    } else {
        stdout.queue(Print(input))?;
    }

    // Move cursor to the correct position
    // We need to account for the fact that ANSI escape codes don't move the cursor
    // So we calculate the visual position based on the actual characters before cursor
    let chars_before_cursor = input.chars().take(cursor_pos).count();

    // Move to the position after the prompt + characters before cursor
    stdout
        .queue(MoveToColumn(prompt_length + chars_before_cursor as u16))?
        .queue(ResetColor)?
        .flush()?;

    Ok(())
}

/// Fast redraw without highlighting for cursor movements (up/down arrows, etc.)
fn redraw_input_line_fast(input: &str, cursor_pos: usize) -> Result<()> {
    use crossterm::{
        cursor::MoveToColumn,
        style::{Print, ResetColor},
        terminal::Clear,
    };

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Clear the current line and move to start
    stdout
        .queue(Clear(crossterm::terminal::ClearType::CurrentLine))?
        .queue(MoveToColumn(0))?;

    // Display prompt
    stdout.queue(Print("> "))?;

    // Print input without highlighting (much faster)
    stdout.queue(Print(input))?;

    // Store the current cursor position relative to the start of input text
    let prompt_length = 2; // "> " length

    // Move cursor to the correct position
    let chars_before_cursor = input.chars().take(cursor_pos).count();

    // Move to the position after the prompt + characters before cursor
    stdout
        .queue(MoveToColumn(prompt_length + chars_before_cursor as u16))?
        .queue(ResetColor)?
        .flush()?;

    Ok(())
}


/// Process input and handle streaming/non-streaming response
async fn process_input(input: &str, agent: &mut Agent, formatter: &formatter::CodeFormatter, stream: bool, cancellation_flag: Arc<AtomicBool>) {
    // Show spinner while processing (only for non-streaming)
    if stream {
        let result = agent.process_message_with_stream(&input, Some(|content| {
            print!("{}", content);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
        }), cancellation_flag.clone()).await;
        
        match result {
            Ok(_response) => {
                println!();
            }
            Err(e) => {
                if e.to_string().contains("CANCELLED") {
                    // Cancellation handled silently
                } else {
                    eprintln!("{}: {}", "Error".red(), e);
                }
                println!();
            }
        }
    } else {
        let spinner = create_spinner();
        let result = agent.process_message(&input, cancellation_flag.clone()).await;
        spinner.finish_and_clear();
        
        match result {
            Ok(response) => {
                // Only print response if it's not empty (i.e., not just @file references)
                if !response.is_empty() {
                    if let Err(e) = formatter.print_formatted(&response) {
                        eprintln!("{} formatting response: {}", "Error".red(), e);
                    }
                }
                println!();
            }
            Err(e) => {
                if e.to_string().contains("CANCELLED") {
                    // Cancellation handled silently
                } else {
                    eprintln!("{}: {}", "Error".red(), e);
                }
                println!();
            }
        }
    }
}

/// Check for and add context files
async fn add_context_files(agent: &mut Agent, context_files: &[String]) -> Result<()> {
    // Always add AGENTS.md from ~/.aixplosion/ if it exists (priority)
    let home_agents_md = get_home_agents_md_path();
    if home_agents_md.exists() {
        debug!("Auto-adding AGENTS.md from ~/.aixplosion/ as context");
        match agent.add_context_file(home_agents_md.to_str().unwrap()).await {
            Ok(_) => println!("{} Added context file: {}", "✓".green(), home_agents_md.display()),
            Err(e) => eprintln!("{} Failed to add context file '{}': {}", "✗".red(), home_agents_md.display(), e),
        }
    }
    
    // Also add AGENTS.md from current directory if it exists (in addition to home directory version)
    if Path::new("AGENTS.md").exists() {
        debug!("Auto-adding AGENTS.md from current directory as context");
        match agent.add_context_file("AGENTS.md").await {
            Ok(_) => println!("{} Added context file: {}", "✓".green(), "AGENTS.md"),
            Err(e) => eprintln!("{} Failed to add context file 'AGENTS.md': {}", "✗".red(), e),
        }
    }
    
    // Add any additional context files specified by the user
    for file_path in context_files {
        debug!("Adding context file: {}", file_path);
        match agent.add_context_file(file_path).await {
            Ok(_) => println!("{} Added context file: {}", "✓".green(), file_path),
            Err(e) => eprintln!("{} Failed to add context file '{}': {}", "✗".red(), file_path, e),
        }
    }

    Ok(())
}

/// Get the path to AGENTS.md in the user's home .aixplosion directory
fn get_home_agents_md_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".aixplosion")
        .join("AGENTS.md")
}

async fn handle_shell_command(command: &str, _agent: &mut Agent) -> Result<()> {
    // Extract the shell command by removing the '!' prefix
    let shell_command = command.trim_start_matches('!').trim();
    
    if shell_command.is_empty() {
        println!("{} Usage: !<command> - Execute a shell command", "⚠️".yellow());
        println!("{} Examples: !dir, !ls -la, !git status", "💡".blue());
        return Ok(());
    }

    println!("{} Executing: {}", "🔧".blue(), shell_command);
    
    // Create a tool call for the bash command
    let tool_call = tools::ToolCall {
        id: "shell_command".to_string(),
        name: "bash".to_string(),
        arguments: serde_json::json!({
            "command": shell_command
        }),
    };

    // Execute the bash command directly without permission checks
    // This bypasses the security manager for ! commands
    execute_bash_command_directly(&tool_call).await.map(|result| {
        if result.is_error {
            println!("{} Command failed:", "❌".red());
            println!("{}", result.content.red());
        } else {
            println!("{}", result.content);
        }
    }).map_err(|e| {
        eprintln!("{} Error executing shell command: {}", "✗".red(), e);
        e
    })?;

    Ok(())
}

/// Execute a bash command directly without security checks (for ! commands)
async fn execute_bash_command_directly(tool_call: &tools::ToolCall) -> Result<tools::ToolResult> {
    let command = tool_call.arguments.get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?
        .to_string();

    debug!("Direct shell command execution: {}", command);

    let tool_use_id = tool_call.id.clone();

    // Execute the command using tokio::task to spawn blocking operation
    let command_clone = command.clone();
    match tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", &command_clone])
                .output()
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("bash")
                .args(["-c", &command_clone])
                .output()
        }
    }).await
    {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            let content = if !stderr.is_empty() {
                format!("Exit code: {}\nStdout:\n{}\nStderr:\n{}", 
                    output.status.code().unwrap_or(-1), stdout, stderr)
            } else {
                format!("Exit code: {}\nOutput:\n{}", 
                    output.status.code().unwrap_or(-1), stdout)
            };

            Ok(tools::ToolResult {
                tool_use_id,
                content,
                is_error: !output.status.success(),
            })
        }
        Ok(Err(e)) => Ok(tools::ToolResult {
            tool_use_id,
            content: format!("Error executing command '{}': {}", command, e),
            is_error: true,
        }),
        Err(e) => Ok(tools::ToolResult {
            tool_use_id,
            content: format!("Task join error: {}", e),
            is_error: true,
        })
    }
}

async fn handle_slash_command(command: &str, agent: &mut Agent, mcp_manager: &McpManager) -> Result<bool> {
    let parts: Vec<&str> = command.trim().split(' ').collect();
    let cmd = parts[0];
    
    match cmd {
        "/help" => {
            print_help();
            Ok(true) // Command was handled
        }
        "/stats" | "/usage" => {
            print_usage_stats(agent);
            Ok(true) // Command was handled
        }
        "/context" => {
            agent.display_context();
            Ok(true) // Command was handled
        }
        "/clear" => {
            match agent.clear_conversation_keep_agents_md().await {
                Ok(_) => {
                    println!("{}", "🧹 Conversation context cleared! (AGENTS.md preserved if it existed)".green());
                }
                Err(e) => {
                    eprintln!("{} Failed to clear context: {}", "✗".red(), e);
                }
            }
            Ok(true) // Command was handled
        }
        "/reset-stats" => {
            agent.reset_token_usage();
            println!("{}", "📊 Token usage statistics reset!".green());
            Ok(true) // Command was handled
        }
        "/mcp" => {
            handle_mcp_command(&parts[1..], mcp_manager).await?;
            // Force refresh MCP tools after any MCP command
            if let Err(e) = agent.force_refresh_mcp_tools().await {
                warn!("Failed to refresh MCP tools: {}", e);
            }
            Ok(true) // Command was handled
        }
        "/permissions" => {
            handle_permissions_command(&parts[1..], agent).await?;
            Ok(true) // Command was handled
        }
        "/file-permissions" => {
            handle_file_permissions_command(&parts[1..], agent).await?;
            Ok(true) // Command was handled
        }
        "/exit" | "/quit" => {
            // Print final stats before exiting
            print_usage_stats(agent);
            println!("{}", "Goodbye! 👋".green());
            std::process::exit(0);
        }
        _ => {
            println!("{} Unknown command: {}. Type /help for available commands.", "⚠️".yellow(), cmd);
            Ok(true) // Command was handled (as unknown)
        }
    }
}

/// Handle MCP commands
async fn handle_mcp_command(args: &[&str], mcp_manager: &McpManager) -> Result<()> {
    
    if args.is_empty() {
        print_mcp_help();
        return Ok(());
    }

    match args[0] {
        "list" => {
            match mcp_manager.list_servers().await {
                Ok(servers) => {
                    println!("{}", "🔌 MCP Servers".cyan().bold());
                    println!();
                    if servers.is_empty() {
                        println!("{}", "No MCP servers configured.".yellow());
                        return Ok(());
                    }
                    
                    for (name, config, connected) in servers {
                        let status = if connected { 
                            "✅ Connected".green().to_string() 
                        } else if config.enabled { 
                            "❌ Disconnected".red().to_string() 
                        } else { 
                            "⏸️ Disabled".yellow().to_string() 
                        };
                        
                        println!("{} {} ({})", 
                            "Server:".bold(), 
                            name.cyan(), 
                            status
                        );
                        
                        if let Some(command) = &config.command {
                            println!("  Command: {}", command);
                        }
                        if let Some(args) = &config.args {
                            println!("  Args: {}", args.join(" "));
                        }
                        if let Some(url) = &config.url {
                            println!("  URL: {}", url);
                        }
                        
                        if connected {
                            if let Ok(tools) = mcp_manager.get_all_tools().await {
                                let server_tools: Vec<_> = tools.iter()
                                    .filter(|(server_name, _)| server_name == &name)
                                    .collect();
                                println!("  Tools: {} available", server_tools.len());
                            }
                        }
                        println!();
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to list MCP servers: {}", "✗".red(), e);
                }
            }
        }
        "connect" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp connect <server_name>", "⚠️".yellow());
                return Ok(());
            }
            
            println!("{} Connecting to MCP server: {}", "🔌".blue(), args[1].cyan());
            
            match mcp_manager.connect_server(args[1]).await {
                Ok(_) => {
                    println!("{} Successfully connected to MCP server: {}", "✅".green(), args[1].cyan());
                    
                    // Try to list available tools
                    match mcp_manager.get_all_tools().await {
                        Ok(tools) => {
                            let server_tools: Vec<_> = tools.iter()
                                .filter(|(server_name, _)| server_name == args[1])
                                .collect();
                            if !server_tools.is_empty() {
                                println!("{} Available tools: {}", "🛠️".blue(), server_tools.len());
                                for (_, tool) in server_tools {
                                    println!("  - {} {}", tool.name.bold(), 
                                        tool.description.as_ref().unwrap_or(&"".to_string()).dimmed());
                                }
                            }
                        }
                        Err(_) => {
                            println!("{} Connected but failed to list tools", "⚠️".yellow());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to connect to MCP server '{}': {}", "✗".red(), args[1], e);
                    println!("{} Troubleshooting:", "💡".yellow());
                    println!("  1. Check if the server is properly configured: /mcp list");
                    println!("  2. Verify the command/URL is correct");
                    println!("  3. Ensure all dependencies are installed");
                    println!("  4. Check network connectivity for WebSocket servers");
                    println!("  5. Try reconnecting: /mcp reconnect {}", args[1]);
                }
            }
        }
        "disconnect" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp disconnect <server_name>", "⚠️".yellow());
                return Ok(());
            }
            
            match mcp_manager.disconnect_server(args[1]).await {
                Ok(_) => {
                    println!("{} Disconnected from MCP server: {}", "🔌".yellow(), args[1].cyan());
                }
                Err(e) => {
                    eprintln!("{} Failed to disconnect from MCP server '{}': {}", "✗".red(), args[1], e);
                }
            }
        }
        "reconnect" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp reconnect <server_name>", "⚠️".yellow());
                return Ok(());
            }
            
            match mcp_manager.reconnect_server(args[1]).await {
                Ok(_) => {
                    println!("{} Reconnected to MCP server: {}", "🔄".blue(), args[1].cyan());
                }
                Err(e) => {
                    eprintln!("{} Failed to reconnect to MCP server '{}': {}", "✗".red(), args[1], e);
                }
            }
        }
        "tools" => {
            match mcp_manager.get_all_tools().await {
                Ok(tools) => {
                    println!("{}", "🛠️  MCP Tools".cyan().bold());
                    println!();
                    
                    if tools.is_empty() {
                        println!("{}", "No MCP tools available. Connect to a server first.".yellow());
                        return Ok(());
                    }
                    
                    let mut by_server = std::collections::HashMap::new();
                    for (server_name, tool) in tools {
                        by_server.entry(server_name).or_insert_with(Vec::new).push(tool);
                    }
                    
                    for (server_name, server_tools) in by_server {
                        println!("{} {}:", "Server:".bold(), server_name.cyan());
                        for tool in server_tools {
                            println!("  🛠️  {}", tool.name.bold());
                            if let Some(description) = &tool.description {
                                println!("     {}", description.dimmed());
                            }
                        }
                        println!();
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to list MCP tools: {}", "✗".red(), e);
                }
            }
        }
        "add" => {
            if args.len() < 4 {
                println!("{} Usage: /mcp add <name> stdio <command> [args...]", "⚠️".yellow());
                println!("{} Usage: /mcp add <name> ws <url>", "⚠️".yellow());
                println!();
                println!("{}", "Examples:".green().bold());
                println!("  /mcp add myserver stdio npx -y @modelcontextprotocol/server-filesystem");
                println!("  /mcp add websocket ws://localhost:8080");
                return Ok(());
            }
            
            let name = args[1];
            let connection_type = args[2];
            
            if connection_type == "stdio" {
                let command = args[3];
                let server_args: Vec<String> = args[4..].iter().map(|s| s.to_string()).collect();
                
                // Validate that we have a proper command
                if command.is_empty() {
                    println!("{} Command cannot be empty", "⚠️".yellow());
                    return Ok(());
                }
                
                let server_config = mcp::McpServerConfig {
                    name: name.to_string(),
                    command: Some(command.to_string()),
                    args: if server_args.is_empty() { None } else { Some(server_args) },
                    url: None,
                    env: None,
                    enabled: true,
                };
                
                println!("{} Adding MCP server: {}", "🔧".blue(), name.cyan());
                println!("  Command: {}", command);
                if !args[4..].is_empty() {
                    println!("  Args: {}", args[4..].join(" "));
                }
                
                match mcp_manager.add_server(name, server_config).await {
                    Ok(_) => {
                        println!("{} Successfully added MCP server: {}", "✅".green(), name.cyan());
                        println!("{} Use '/mcp connect {}' to connect to this server", "💡".blue(), name);
                    }
                    Err(e) => {
                        eprintln!("{} Failed to add MCP server '{}': {}", "✗".red(), name, e);
                        println!("{} Common issues:", "💡".yellow());
                        println!("  - Command '{}' not found or not executable", command);
                        println!("  - Missing dependencies (e.g., Node.js, npm, npx)");
                        println!("  - Network connectivity issues");
                        println!("  - Insufficient permissions");
                    }
                }
            } else if connection_type == "ws" || connection_type == "websocket" {
                let url = args[3];
                
                // Basic URL validation
                if !url.starts_with("ws://") && !url.starts_with("wss://") {
                    println!("{} URL must start with ws:// or wss://", "⚠️".yellow());
                    return Ok(());
                }
                
                let server_config = mcp::McpServerConfig {
                    name: name.to_string(),
                    command: None,
                    args: None,
                    url: Some(url.to_string()),
                    env: None,
                    enabled: true,
                };
                
                println!("{} Adding MCP server: {}", "🔧".blue(), name.cyan());
                println!("  URL: {}", url);
                
                match mcp_manager.add_server(name, server_config).await {
                    Ok(_) => {
                        println!("{} Successfully added MCP server: {}", "✅".green(), name.cyan());
                        println!("{} Use '/mcp connect {}' to connect to this server", "💡".blue(), name);
                    }
                    Err(e) => {
                        eprintln!("{} Failed to add MCP server '{}': {}", "✗".red(), name, e);
                    }
                }
            } else {
                println!("{} Connection type must be 'stdio' or 'ws'", "⚠️".yellow());
                println!("{} Available types:", "💡".blue());
                println!("  - stdio: For command-line based MCP servers");
                println!("  - ws: For WebSocket-based MCP servers");
            }
        }
        "remove" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp remove <server_name>", "⚠️".yellow());
                return Ok(());
            }
            
            match mcp_manager.remove_server(args[1]).await {
                Ok(_) => {
                    println!("{} Removed MCP server: {}", "🗑️".red(), args[1].cyan());
                }
                Err(e) => {
                    eprintln!("{} Failed to remove MCP server '{}': {}", "✗".red(), args[1], e);
                }
            }
        }
        "connect-all" => {
            match mcp_manager.connect_all_enabled().await {
                Ok(_) => {
                    println!("{} Attempted to connect to all enabled MCP servers", "🔄".blue());
                }
                Err(e) => {
                    eprintln!("{} Failed to connect to MCP servers: {}", "✗".red(), e);
                }
            }
        }
      "test" => {
            if args.len() < 2 {
                println!("{} Usage: /mcp test <command>", "⚠️".yellow());
                println!("{} Test if a command is available and executable", "💡".blue());
                return Ok(());
            }
            
            let command = args[1];
            println!("{} Testing command: {}", "🧪".blue(), command.cyan());
            
            // Try to run the command with --version or --help to test if it exists
            let test_args = if command == "npx" {
                vec!["--version".to_string()]
            } else {
                vec!["--version".to_string()]
            };
            
            match tokio::process::Command::new(command)
                .args(&test_args)
                .output()
                .await 
            {
                Ok(output) => {
                    if output.status.success() {
                        println!("{} Command '{}' is available and executable", "✅".green(), command);
                        if !output.stdout.is_empty() {
                            let version = String::from_utf8_lossy(&output.stdout);
                            println!("  Version: {}", version.trim());
                        }
                    } else {
                        println!("{} Command '{}' exists but failed to execute", "⚠️".yellow(), command);
                        if !output.stderr.is_empty() {
                            let error = String::from_utf8_lossy(&output.stderr);
                            println!("  Error: {}", error.trim());
                        }
                    }
                }
                Err(e) => {
                    println!("{} Command '{}' not found or not executable", "✗".red(), command);
                    println!("  Error: {}", e);
                    println!("{} Suggestions:", "💡".blue());
                    println!("  - Install the command/tool if missing");
                    println!("  - Check if the command is in your PATH");
                    println!("  - Use the full path to the command");
                }
            }
        }
        "disconnect-all" => {
            match mcp_manager.disconnect_all().await {
                Ok(_) => {
                    println!("{} Disconnected from all MCP servers", "🔌".yellow());
                }
                Err(e) => {
                    eprintln!("{} Failed to disconnect from MCP servers: {}", "✗".red(), e);
                }
            }
        }
        _ => {
            println!("{} Unknown MCP command: {}", "⚠️".yellow(), args[0]);
            print_mcp_help();
        }
    }
    
    Ok(())
}

/// Print MCP help information
fn print_mcp_help() {
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

/// Print usage statistics
fn print_usage_stats(agent: &Agent) {
    let usage = agent.get_token_usage();
    println!("{}", "📊 Token Usage Statistics".cyan().bold());
    println!();
    println!("{}", "Request Summary:".green().bold());
    println!("  Requests made: {}", usage.request_count);
    println!();
    println!("{}", "Token Usage:".green().bold());
    println!("  Input tokens:  {}", usage.total_input_tokens);
    println!("  Output tokens: {}", usage.total_output_tokens);
    println!("  Total tokens: {}", usage.total_tokens());
    println!();
    
    if usage.request_count > 0 {
        let avg_input = usage.total_input_tokens as f64 / usage.request_count as f64;
        let avg_output = usage.total_output_tokens as f64 / usage.request_count as f64;
        let avg_total = usage.total_tokens() as f64 / usage.request_count as f64;
        
        println!("{}", "Average per request:".green().bold());
        println!("  Input tokens:  {:.1}", avg_input);
        println!("  Output tokens: {:.1}", avg_output);
        println!("  Total tokens: {:.1}", avg_total);
        println!();
    }
}

/// Create a progress spinner for API calls
fn create_spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.set_message("Thinking...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner
}

/// Handle permissions commands
async fn handle_permissions_command(args: &[&str], agent: &mut Agent) -> Result<()> {
    use crate::security::PermissionResult;
    
    if args.is_empty() {
        // Display current permissions with full details
        let security_manager_ref = agent.get_bash_security_manager().clone();
        let security_manager = security_manager_ref.read().await;
        security_manager.display_permissions();
        return Ok(());
    }

    match args[0] {
        "show" | "list" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let security_manager = security_manager_ref.read().await;
            security_manager.display_permissions();
        }
        "test" => {
            if args.len() < 2 {
                println!("{} Usage: /permissions test <command>", "⚠️".yellow());
                return Ok(());
            }
            
            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let security_manager = security_manager_ref.read().await;
            
            match security_manager.check_command_permission(&command) {
                PermissionResult::Allowed => {
                    println!("{} Command '{}' is ALLOWED", "✅".green(), command);
                }
                PermissionResult::Denied => {
                    println!("{} Command '{}' is DENIED", "❌".red(), command);
                }
                PermissionResult::RequiresPermission => {
                    println!("{} Command '{}' requires permission", "❓".yellow(), command);
                }
            }
        }
        "allow" => {
            if args.len() < 2 {
                println!("{} Usage: /permissions allow <command_pattern>", "⚠️".yellow());
                println!("{} Examples:", "💡".blue());
                println!("  /permissions allow 'git *'");
                println!("  /permissions allow 'cargo test'");
                println!("  /permissions allow 'ls -la'");
                return Ok(());
            }
            
            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            
            security_manager.add_to_allowlist(command.clone());
            println!("{} Added '{}' to allowlist", "✅".green(), command);
            
            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "deny" => {
            if args.len() < 2 {
                println!("{} Usage: /permissions deny <command_pattern>", "⚠️".yellow());
                println!("{} Examples:", "💡".blue());
                println!("  /permissions deny 'rm *'");
                println!("  /permissions deny 'sudo *'");
                println!("  /permissions deny 'format'");
                return Ok(());
            }
            
            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            
            security_manager.add_to_denylist(command.clone());
            println!("{} Added '{}' to denylist", "❌".red(), command);
            
            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "remove-allow" => {
            if args.len() < 2 {
                println!("{} Usage: /permissions remove-allow <command_pattern>", "⚠️".yellow());
                return Ok(());
            }
            
            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            
            if security_manager.remove_from_allowlist(&command) {
                println!("{} Removed '{}' from allowlist", "🗑️".yellow(), command);
                
                // Save to config
                if let Err(e) = save_permissions_to_config(&agent).await {
                    println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
                }
            } else {
                println!("{} Command '{}' not found in allowlist", "⚠️".yellow(), command);
            }
        }
        "remove-deny" => {
            if args.len() < 2 {
                println!("{} Usage: /permissions remove-deny <command_pattern>", "⚠️".yellow());
                return Ok(());
            }
            
            let command = args[1..].join(" ");
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            
            if security_manager.remove_from_denylist(&command) {
                println!("{} Removed '{}' from denylist", "🗑️".yellow(), command);
                
                // Save to config
                if let Err(e) = save_permissions_to_config(&agent).await {
                    println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
                }
            } else {
                println!("{} Command '{}' not found in denylist", "⚠️".yellow(), command);
            }
        }
        "enable" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            let mut security = security_manager.get_security().clone();
            security.enabled = true;
            security_manager.update_security(security);
            println!("{} Bash security enabled", "✅".green());
            
            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "disable" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            let mut security = security_manager.get_security().clone();
            security.enabled = false;
            security_manager.update_security(security);
            println!("{} Bash security disabled", "⚠️".yellow());
            println!("{} Warning: This allows any bash command to be executed!", "⚠️".red().bold());
            
            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "ask-on" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            let mut security = security_manager.get_security().clone();
            security.ask_for_permission = true;
            security_manager.update_security(security);
            println!("{} Ask for permission enabled", "✅".green());
            
            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "ask-off" => {
            let security_manager_ref = agent.get_bash_security_manager().clone();
            let mut security_manager = security_manager_ref.write().await;
            let mut security = security_manager.get_security().clone();
            security.ask_for_permission = false;
            security_manager.update_security(security);
            println!("{} Ask for permission disabled", "⚠️".yellow());
            println!("{} Unknown commands will be denied by default", "⚠️".red());
            
            // Save to config
            if let Err(e) = save_permissions_to_config(&agent).await {
                println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
            }
        }
        "help" => {
            print_permissions_help();
        }
        _ => {
            println!("{} Unknown permissions command: {}", "⚠️".yellow(), args[0]);
            println!("{} Available commands:", "💡".yellow());
            println!("  /permissions                - Show current permissions");
            println!("  /permissions help          - Show permissions help");
            println!("  /permissions test <cmd>    - Test if a command is allowed");
            println!("  /permissions allow <cmd>   - Add command to allowlist");
            println!("  /permissions deny <cmd>    - Add command to denylist");
            println!("  /permissions remove-allow <cmd> - Remove from allowlist");
            println!("  /permissions remove-deny <cmd> - Remove from denylist");
            println!("  /permissions enable        - Enable bash security");
            println!("  /permissions disable       - Disable bash security");
            println!("  /permissions ask-on        - Enable asking for permission");
            println!("  /permissions ask-off       - Disable asking for permission");
        }
    }
    
    Ok(())
}

/// Save current permissions to unified config file
async fn save_permissions_to_config(agent: &Agent) -> Result<()> {
    use crate::config::Config;
    
    // Load existing config to preserve other settings
    let mut existing_config = Config::load(None).await?;
    
    // Get current security settings from agent
    let updated_config = agent.get_config_for_save().await;
    
    // Update only the bash_security settings
    existing_config.bash_security = updated_config.bash_security;
    
    // Save the updated config
    match existing_config.save(None).await {
        Ok(_) => {
            println!("{} Permissions saved to unified config", "💾".blue());
        }
        Err(e) => {
            println!("{} Failed to save permissions: {}", "⚠️".yellow(), e);
        }
    }
    
    Ok(())
}

/// Handle file permissions commands
async fn handle_file_permissions_command(args: &[&str], agent: &mut Agent) -> Result<()> {
    use crate::security::FilePermissionResult;
    
    if args.is_empty() {
        // Display current file permissions with full details
        let file_security_manager_ref = agent.get_file_security_manager().clone();
        let file_security_manager = file_security_manager_ref.read().await;
        file_security_manager.display_file_permissions();
        return Ok(());
    }

    match args[0] {
        "show" | "list" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let file_security_manager = file_security_manager_ref.read().await;
            file_security_manager.display_file_permissions();
        }
        "test" => {
            if args.len() < 3 {
                println!("{} Usage: /file-permissions test <operation> <path>", "⚠️".yellow());
                println!("{} Operations: write_file, edit_file, delete_file, create_directory", "💡".blue());
                return Ok(());
            }
            
            let operation = args[1];
            let path = args[2..].join(" ");
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            
            match file_security_manager.check_file_permission(operation, &path) {
                FilePermissionResult::Allowed => {
                    println!("{} File operation '{}' on '{}' is ALLOWED", "✅".green(), operation, path);
                }
                FilePermissionResult::Denied => {
                    println!("{} File operation '{}' on '{}' is DENIED", "❌".red(), operation, path);
                }
                FilePermissionResult::RequiresPermission => {
                    println!("{} File operation '{}' on '{}' requires permission", "❓".yellow(), operation, path);
                }
            }
        }
        "enable" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            let mut security = file_security_manager.get_file_security().clone();
            security.enabled = true;
            file_security_manager.update_file_security(security);
            println!("{} File security enabled", "✅".green());
            
            // Save to config
            if let Err(e) = save_file_permissions_to_config(&agent).await {
                println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
            }
        }
        "disable" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            let mut security = file_security_manager.get_file_security().clone();
            security.enabled = false;
            file_security_manager.update_file_security(security);
            println!("{} File security disabled", "⚠️".yellow());
            println!("{} Warning: This allows any file operation to be executed!", "⚠️".red().bold());
            
            // Save to config
            if let Err(e) = save_file_permissions_to_config(&agent).await {
                println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
            }
        }
        "ask-on" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            let mut security = file_security_manager.get_file_security().clone();
            security.ask_for_permission = true;
            file_security_manager.update_file_security(security);
            println!("{} Ask for file permission enabled", "✅".green());
            
            // Save to config
            if let Err(e) = save_file_permissions_to_config(&agent).await {
                println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
            }
        }
        "ask-off" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            let mut security = file_security_manager.get_file_security().clone();
            security.ask_for_permission = false;
            file_security_manager.update_file_security(security);
            println!("{} Ask for file permission disabled", "⚠️".yellow());
            println!("{} All file operations will be allowed by default", "⚠️".red());
            
            // Save to config
            if let Err(e) = save_file_permissions_to_config(&agent).await {
                println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
            }
        }
        "reset-session" => {
            let file_security_manager_ref = agent.get_file_security_manager().clone();
            let mut file_security_manager = file_security_manager_ref.write().await;
            file_security_manager.reset_session_permissions();
            println!("{} Session file permissions reset", "🔄".blue());
            println!("{} File operations will require permission again", "💡".blue());
        }
        "help" => {
            print_file_permissions_help();
        }
        _ => {
            println!("{} Unknown file permissions command: {}", "⚠️".yellow(), args[0]);
            println!("{} Available commands:", "💡".yellow());
            println!("  /file-permissions                - Show current file permissions");
            println!("  /file-permissions help          - Show file permissions help");
            println!("  /file-permissions test <op> <path> - Test if file operation is allowed");
            println!("  /file-permissions enable        - Enable file security");
            println!("  /file-permissions disable       - Disable file security");
            println!("  /file-permissions ask-on        - Enable asking for permission");
            println!("  /file-permissions ask-off       - Disable asking for permission");
            println!("  /file-permissions reset-session - Reset session permissions");
        }
    }
    
    Ok(())
}

/// Save current file permissions to unified config file
async fn save_file_permissions_to_config(agent: &Agent) -> Result<()> {
    use crate::config::Config;
    
    // Load existing config to preserve other settings
    let mut existing_config = Config::load(None).await?;
    
    // Get current file security settings from agent
    let file_security_manager_ref = agent.get_file_security_manager().clone();
    let file_security_manager = file_security_manager_ref.read().await;
    let updated_file_security = file_security_manager.get_file_security().clone();
    
    // Update only the file_security settings
    existing_config.file_security = updated_file_security;
    
    // Save the updated config
    match existing_config.save(None).await {
        Ok(_) => {
            println!("{} File permissions saved to unified config", "💾".blue());
        }
        Err(e) => {
            println!("{} Failed to save file permissions: {}", "⚠️".yellow(), e);
        }
    }
    
    Ok(())
}

/// Print file permissions help information
fn print_file_permissions_help() {
    println!("{}", "🔒 File Permissions Commands".cyan().bold());
    println!();
    println!("{}", "View File Permissions:".green().bold());
    println!("  /file-permissions                - Show current file permissions and security settings");
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
fn print_permissions_help() {
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

/// Display a large red warning for yolo mode
fn display_yolo_warning() {
    println!();
    println!("{}", "⚠️  WARNING: YOLO MODE ENABLED  ⚠️".red().bold().blink());
    println!("{}", " ALL SECURITY PERMISSIONS ARE BYPASSED - USE WITH EXTREME CAUTION ".red().bold());
    println!();
    println!("{}", " • File operations (read/write/delete) will execute WITHOUT prompts ".red());
    println!("{}", " • Bash commands will execute WITHOUT permission checks ".red());
    println!("{}", " • MCP tools will execute WITHOUT security validation ".red());
    println!("{}", " • No allowlist/denylist filtering will be applied ".red());
    println!("{}", " • All tool calls are automatically approved ".red());
    println!();
    println!("{}", " 🚨 This mode can cause irreversible damage to your system! ".red().bold());
    println!();
    println!("{}", " Press Ctrl+C NOW to cancel if this was not intended! ".red().bold());
    println!();
    
    // Add a dramatic pause for effect
    std::thread::sleep(std::time::Duration::from_millis(2000));
    println!("{}", "🔥 Proceeding in YOLO mode... You have been warned! 🔥".red().bold());
    println!();
}

/// Display YOLO mode warning after MCP configuration is complete
fn display_mcp_yolo_warning() {
    println!();
    println!("{}", "🔌 MCP Configuration Complete - YOLO Mode Active 🔌".red().bold());
    println!();
    println!("{}", " ⚠️  MCP TOOLS WILL EXECUTE WITHOUT SECURITY VALIDATION ⚠️ ".red().bold());
    println!();
    println!("{}", " • MCP server tools are now available and will execute WITHOUT prompts ".red());
    println!("{}", " • No permission checks will be applied to MCP tool calls ".red());
    println!("{}", " • All MCP operations (file access, commands, etc.) are auto-approved ".red());
    println!("{}", " • External MCP server connections have unrestricted access ".red());
    println!();
    println!("{}", " 🚨 MCP tools can potentially access and modify your system! ".red().bold());
    println!();
    println!("{}", " 🔥 All MCP servers and their tools are operating in YOLO mode! 🔥".red().bold());
    println!();
}
fn print_help() {
    println!("{}", "🤖 AIxplosion - Slash Commands".cyan().bold());
    println!();
    println!("{}", "Available commands:".green().bold());
    println!("  /help         - Show this help message");
    println!("  /stats        - Show token usage statistics");
    println!("  /usage        - Show token usage statistics (alias for /stats)");
    println!("  /context      - Show current conversation context");
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
    println!("  ESC           - Cancel current AI conversation (during processing)");
    println!("  Ctrl+C        - Exit the program immediately");
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
    println!("{}", "Examples:".green().bold());
    println!("  aixplosion -f config.toml \"Explain this configuration\"");
    println!("  aixplosion \"What does @Cargo.toml contain?\"");
    println!("  aixplosion \"Compare @file1.rs and @file2.rs\"");
    println!("  aixplosion \"@file1.txt @file2.txt\"  # Only adds context, no API call");
    println!("  aixplosion -s \"You are a Rust expert\" \"Help me with this code\"");
    println!("  aixplosion -s \"Act as a code reviewer\" -f main.rs \"Review this code\"");
    println!("  aixplosion --stream \"Tell me a story\"  # Stream the response");
    println!("  !dir                    # List directory contents");
    println!("  !git status             # Check git status");
    println!("  !cargo build            # Build the project");
    println!("  ESC                     # Cancel AI conversation during processing");
    println!();
    println!("{}", "History Navigation:".green().bold());
    println!("  • Press UP arrow to cycle through previous commands");
    println!("  • Press DOWN arrow to cycle through more recent commands");
    println!("  • Start typing to exit history navigation mode");
    println!("  • History is preserved across the entire session");
    println!("  • Duplicate and empty commands are not stored");
    println!();
    println!("{}", "Any other input will be sent to the AIxplosion for processing.".dimmed());
    println!();
}

#[derive(Parser)]
#[command(name = "aixplosion")]
#[command(about = "A CLI coding agent powered by Anthropic AI")]
#[command(version)]
struct Cli {
    /// The message to send to the agent
    #[arg(short = 'm', long)]
    message: Option<String>,

    /// Set the API key (overrides config file)
    #[arg(short = 'k', long)]
    api_key: Option<String>,

    /// Specify the model to use
    #[arg(long, default_value = "glm-4.6")]
    model: String,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,

    /// Run in non-interactive mode
    #[arg(short, long)]
    non_interactive: bool,

    /// Files to include as context
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    context_files: Vec<String>,

    /// System prompt to use for the conversation
    #[arg(short = 's', long = "system", value_name = "PROMPT")]
    system_prompt: Option<String>,

    /// Enable streaming responses
    #[arg(long)]
    stream: bool,

    /// Enable 'yolo' mode - bypass all permission checks for file and tool operations
    #[arg(long)]
    yolo: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Cli::parse();
    debug!("Starting AIxplosion with model: {}", cli.model);

    // Display large red warning if yolo mode is enabled
    if cli.yolo {
        display_yolo_warning();
    }

    // Load configuration
    let mut config = Config::load(cli.config.as_deref()).await?;

    // Override API key if provided via command line (highest priority)
    if let Some(api_key) = cli.api_key {
        config.api_key = api_key;
    } else if config.api_key.is_empty() {
        // If no API key from config, try environment variable
        config.api_key = std::env::var("ANTHROPIC_AUTH_TOKEN").unwrap_or_default();
    }

    println!("Using configuration:");
    println!("  Base URL: {}", config.base_url);
    println!("  Model: {}", cli.model);
    
    // Show yolo mode status
    if cli.yolo {
        println!("  {} YOLO MODE ENABLED - All permission checks bypassed!", "🔥".red().bold());
    }
    
    // Validate API key without exposing it
    if config.api_key.is_empty() {
        eprintln!("{}", "Error: API key is required. Set it via environment variable ANTHROPIC_AUTH_TOKEN or use --api-key".red());
        eprintln!("Create a config file at {} or set ANTHROPIC_AUTH_TOKEN environment variable",
                 Config::default_config_path().display());
        std::process::exit(1);
    } else {
        println!("  API Key: {}", if config.api_key.len() > 10 { 
            format!("{}... ({} chars)", &config.api_key[..8], config.api_key.len())
        } else { 
            "configured".to_string() 
        });
    }

    // Create code formatter
    let formatter = create_code_formatter()?;

    // Create and run agent
    let mut agent = Agent::new(config.clone(), cli.model, cli.yolo);
    
    // Initialize MCP manager
    let mcp_manager = Arc::new(McpManager::new());
    
    // Initialize MCP manager with config from unified config
    mcp_manager.initialize(config.mcp.clone()).await?;
    
    // Set MCP manager in agent
    agent = agent.with_mcp_manager(mcp_manager.clone());
    
    // Connect to all enabled MCP servers
    info!("Connecting to MCP servers...");
    let mcp_connect_result = tokio::time::timeout(
        std::time::Duration::from_secs(30), // 30 second timeout for MCP connections
        mcp_manager.connect_all_enabled()
    ).await;
    
    match mcp_connect_result {
        Ok(Ok(_)) => {
            info!("MCP servers connected successfully");
        }
        Ok(Err(e)) => {
            warn!("Failed to connect to MCP servers: {}", e);
            error!("MCP Server Connection Issues:");
            error!("  - Check that MCP servers are configured correctly: /mcp list");
            error!("  - Verify server commands/URLs are valid");
            error!("  - Ensure all dependencies are installed");
            error!("  - Use '/mcp test <command>' to verify command availability");
            error!("  - Tool calls to unavailable MCP servers will fail");
        }
        Err(_) => {
            warn!("MCP server connection timed out after 30 seconds");
            error!("MCP Server Connection Timeout:");
            error!("  - MCP servers are taking too long to respond");
            error!("  - Check if servers are running and accessible");
            error!("  - Use '/mcp reconnect <server>' to try connecting manually");
        }
    }

    // Force initial refresh of MCP tools after connecting
    info!("Refreshing MCP tools...");
    let mcp_refresh_result = tokio::time::timeout(
        std::time::Duration::from_secs(15), // 15 second timeout for MCP tools refresh
        agent.force_refresh_mcp_tools()
    ).await;
    
    match mcp_refresh_result {
        Ok(Ok(_)) => {
            info!("MCP tools loaded successfully");
        }
        Ok(Err(e)) => {
            warn!("Failed to refresh MCP tools on startup: {}", e);
            error!("MCP Tools Loading Failed:");
            error!("  - Connected MCP servers may not be responding properly");
            error!("  - Tools may have invalid schemas or descriptions");
            error!("  - Use '/mcp tools' to check available tools");
            error!("  - Use '/mcp reconnect <server>' to fix connection issues");
        }
        Err(_) => {
            warn!("MCP tools refresh timed out after 15 seconds");
            error!("MCP Tools Refresh Timeout:");
            error!("  - MCP servers are taking too long to provide tools");
            error!("  - Some tools may not be available initially");
            error!("  - Tools will be refreshed on demand during use");
        }
    }

    // Display YOLO mode warning after MCP configuration is complete
    if cli.yolo {
        display_mcp_yolo_warning();
    }

    // Set system prompt - use command line prompt if provided, otherwise use config default
    match &cli.system_prompt {
        Some(system_prompt) => {
            agent.set_system_prompt(system_prompt.clone());
            println!("{} Using custom system prompt: {}", "✓".green(), system_prompt);
        }
        None => {
            // Use config's default system prompt if available
            if let Some(default_prompt) = &config.default_system_prompt {
                agent.set_system_prompt(default_prompt.clone());
                println!("{} Using default system prompt from config", "✓".green());
            }
        }
    }

    // Add context files
    add_context_files(&mut agent, &cli.context_files).await?;

    let is_interactive = cli.message.is_none() && !cli.non_interactive;

    if let Some(message) = cli.message {
        // Display the message with file highlighting
        let highlighted_message = formatter.format_input_with_file_highlighting(&message);
        println!("> {}", highlighted_message);
        
        // Single message mode
        if cli.stream {
            let cancellation_flag = Arc::new(AtomicBool::new(false));
            let _response = agent.process_message_with_stream(&message, Some(|content| {
                print!("{}", content);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            }), cancellation_flag).await?;
            print_usage_stats(&agent);
        } else {
            let cancellation_flag = Arc::new(AtomicBool::new(false));
            let spinner = create_spinner();
            let response = agent.process_message(&message, cancellation_flag).await?;
            spinner.finish_and_clear();
            formatter.print_formatted(&response)?;
            print_usage_stats(&agent);
        }
    } else if cli.non_interactive {
        // Read from stdin
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let trimmed_input = input.trim();
        
        // Display the input with file highlighting
        let highlighted_input = formatter.format_input_with_file_highlighting(trimmed_input);
        println!("> {}", highlighted_input);
        
        let cancellation_flag = Arc::new(AtomicBool::new(false));
        if cli.stream {
            let _response = agent.process_message_with_stream(trimmed_input, Some(|content| {
                print!("{}", content);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            }), cancellation_flag).await?;
            print_usage_stats(&agent);
        } else {
            let spinner = create_spinner();
            let response = agent.process_message(trimmed_input, cancellation_flag).await?;
            spinner.finish_and_clear();
            formatter.print_formatted(&response)?;
            print_usage_stats(&agent);
        }
    } else {
        // Interactive mode
        // Display the cool logo on startup
        logo::display_logo();
        println!("{}", "🤖 AIxplosion - Interactive Mode".green().bold());
        println!("{}", "Type 'exit', 'quit', or '/exit' to quit. Type '/help' for available commands.".dimmed());
        println!();

        // Initialize shared history for the interactive session
        let mut input_history = InputHistory::new();

        loop {
            let input = match read_input_with_completion_and_highlighting(Some(&formatter), &mut input_history) {
                Ok(input) => input,
                Err(e) => {
                    if e.to_string().contains("CANCELLED") {
                        // User pressed ESC during input, just continue to next prompt
                        continue;
                    }
                    eprintln!("{} Error reading input: {}", "✗".red(), e);
                    continue;
                }
            };
            
            // If input is empty, continue to next iteration
            if input.is_empty() {
                continue;
            }
            
            // Check for commands first (they can't be multi-line)
            if input.starts_with('/') || input.starts_with('!') || 
               input == "exit" || input == "quit" {
                
                // Check for slash commands first
                if input.starts_with('/') {
                    match handle_slash_command(&input, &mut agent, &mcp_manager).await {
                        Ok(_) => {}, // Command handled successfully
                        Err(e) => {
                            eprintln!("{} Error handling command: {}", "✗".red(), e);
                        }
                    }
                    continue;
                }

                // Check for shell commands (!)
                if input.starts_with('!') {
                    match handle_shell_command(&input, &mut agent).await {
                        Ok(_) => {}, // Command handled successfully
                        Err(e) => {
                            eprintln!("{} Error executing shell command: {}", "✗".red(), e);
                        }
                    }
                    continue;
                }

                // Check for traditional exit commands
                if input == "exit" || input == "quit" {
                    // Print final stats before exiting
                    print_usage_stats(&agent);
                    println!("{}", "Goodbye! 👋".green());
                    break;
                }
            } else {
                // For regular messages, spawn ESC listener only for AI processing
                let cancellation_flag_for_processing = Arc::new(AtomicBool::new(false));
                let cancellation_flag_listener = cancellation_flag_for_processing.clone();

                // Start ESC key listener only during actual AI processing (not for commands)
                let esc_handle = tokio::spawn(async move {
                    use crossterm::event;
                    loop {
                        // Even longer polling interval since this is only during AI processing
                        if event::poll(std::time::Duration::from_millis(1000)).unwrap_or(false) {
                            if let Ok(event::Event::Key(key_event)) = event::read() {
                                if key_event.code == KeyCode::Esc && key_event.kind == KeyEventKind::Press {
                                    cancellation_flag_listener.store(true, Ordering::SeqCst);
                                    println!("\n{} Cancelling AI conversation...", "🛑".yellow());
                                    break;
                                }
                            }
                        }
                        // Longer sleep during AI processing
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    }
                });

                // Process the input (highlighting already shown during typing)
                process_input(&input, &mut agent, &formatter, cli.stream, cancellation_flag_for_processing.clone()).await;

                // Clean up the ESC listener task
                esc_handle.abort();
            }
        }
    }

    // Print final usage stats before exiting (only for interactive mode)
    if is_interactive {
        print_usage_stats(&agent);
    }

    // Disconnect from all MCP servers
    if let Err(e) = mcp_manager.disconnect_all().await {
        warn!("Failed to disconnect from MCP servers: {}", e);
    }

    Ok(())
}