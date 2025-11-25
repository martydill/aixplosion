use colored::*;
use serde_json::Value;
use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Common trait for tool display implementations
pub trait ToolDisplay {
    /// Show the tool call details (optional for simple display)
    fn show_call_details(&self, arguments: &Value) {
        let _ = arguments;
        // Default implementation does nothing for simple display
    }

    /// Complete the tool call with success
    fn complete_success(&mut self, result: &str);

    /// Complete the tool call with error
    fn complete_error(&mut self, error: &str);
}

/// A pretty display for tool calls with simple status indicators
pub struct ToolCallDisplay {
    tool_name: String,
    start_time: std::time::Instant,
}

impl ToolCallDisplay {
    /// Create a new tool call display for a specific tool
    pub fn new(tool_name: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            start_time: std::time::Instant::now(),
        }
    }

    /// Get current time as formatted string
    fn get_current_time(&self) -> String {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                let hours = (duration.as_secs() % 86400) / 3600;
                let minutes = (duration.as_secs() % 3600) / 60;
                let seconds = duration.as_secs() % 60;
                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            }
            Err(_) => "00:00:00".to_string(),
        }
    }

    /// Get an appropriate icon for each tool type
    fn get_tool_icon(tool_name: &str) -> &'static str {
        match tool_name {
            "list_directory" => "📁",
            "read_file" => "📖",
            "write_file" => "✏️",
            "edit_file" => "🔄",
            "delete_file" => "🗑️",
            "create_directory" => "📁",
            "bash" => "💻",
            _ => "🔧",
        }
    }

    /// Display the tool call details
    pub fn show_call_details(&self, arguments: &Value) {
        let icon = Self::get_tool_icon(&self.tool_name);

        println!(
            "{}",
            "┌─────────────────────────────────────────────────".dimmed()
        );
        println!(
            "{} {} {} {}",
            "│".dimmed(),
            icon,
            format!("Tool Call: {}", self.tool_name.cyan().bold()),
            format!("[{}]", self.get_current_time()).dimmed()
        );

        // Display relevant arguments based on tool type
        match self.tool_name.as_str() {
            "list_directory" => {
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    println!(
                        "{} {} {} {}",
                        "│".dimmed(),
                        "📍".yellow(),
                        "Path:".bold(),
                        path.green()
                    );
                }
            }
            "read_file" | "write_file" | "edit_file" | "delete_file" | "create_directory" => {
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    println!(
                        "{} {} {} {}",
                        "│".dimmed(),
                        "📄".yellow(),
                        "File:".bold(),
                        path.green()
                    );
                }

                // Additional info for specific file operations
                match self.tool_name.as_str() {
                    "write_file" => {
                        if let Some(content) = arguments.get("content").and_then(|v| v.as_str()) {
                            println!(
                                "{} {} {} {} bytes",
                                "│".dimmed(),
                                "📝".yellow(),
                                "Size:".bold(),
                                content.len().to_string().green()
                            );
                        }
                    }
                    "edit_file" => {
                        if let Some(old_text) = arguments.get("old_text").and_then(|v| v.as_str()) {
                            println!(
                                "{} {} {} {} bytes",
                                "│".dimmed(),
                                "📝".yellow(),
                                "Old:".bold(),
                                old_text.len().to_string().yellow()
                            );
                        }
                        if let Some(new_text) = arguments.get("new_text").and_then(|v| v.as_str()) {
                            println!(
                                "{} {} {} {} bytes",
                                "│".dimmed(),
                                "📝".yellow(),
                                "New:".bold(),
                                new_text.len().to_string().green()
                            );
                        }
                    }
                    _ => {}
                }
            }
            "bash" => {
                if let Some(command) = arguments.get("command").and_then(|v| v.as_str()) {
                    println!(
                        "{} {} {} {}",
                        "│".dimmed(),
                        "💻".yellow(),
                        "Command:".bold(),
                        command.green()
                    );
                }
            }
            _ => {}
        }

        println!(
            "{}",
            "└─────────────────────────────────────────────────".dimmed()
        );

        // Flush to ensure immediate display
        io::stdout().flush().unwrap();
    }

    /// Complete the tool call with success
    pub fn complete_success(self, result: &str) {
        let duration = self.start_time.elapsed();
        self.show_result(result, false, duration);
    }

    /// Complete the tool call with error
    pub fn complete_error(self, error: &str) {
        let duration = self.start_time.elapsed();
        self.show_result(error, true, duration);
    }

    /// Show the tool result with appropriate formatting
    fn show_result(&self, content: &str, is_error: bool, duration: Duration) {
        let icon = if is_error { "❌" } else { "✅" };
        let status = if is_error {
            "FAILED".red().bold()
        } else {
            "SUCCESS".green().bold()
        };

        println!(
            "{}",
            "┌─────────────────────────────────────────────────".dimmed()
        );
        println!(
            "{} {} {} {} ({})",
            "│".dimmed(),
            icon,
            format!("Result: {}", self.tool_name.cyan().bold()),
            status,
            format!("{:.2}s", duration.as_secs_f64()).dimmed()
        );

        // Limit output to max 5 lines
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let max_display_lines = 5;

        if total_lines == 0 {
            if is_error {
                println!("{} {}", "│".dimmed(), "Error:".red().bold());
                println!("{}   {}", "│".dimmed(), "[No error details]".dimmed());
            } else {
                println!("{} {}", "│".dimmed(), "Output:".green().bold());
                println!("{}   {}", "│".dimmed(), "[No output]".dimmed());
            }
        } else {
            // Display limited lines
            let display_lines = if total_lines <= max_display_lines {
                total_lines
            } else {
                max_display_lines
            };

            if is_error {
                println!("{} {}", "│".dimmed(), "Error:".red().bold());
                for line in lines.iter().take(display_lines) {
                    println!("{}   {}", "│".dimmed(), line.red());
                }
            } else {
                println!("{} {}", "│".dimmed(), "Output:".green().bold());
                for line in lines.iter().take(display_lines) {
                    println!("{}   {}", "│".dimmed(), line);
                }
            }

            // Show truncation indicator if content was limited
            if total_lines > max_display_lines {
                let remaining = total_lines - max_display_lines;
                println!(
                    "{}   {}",
                    "│".dimmed(),
                    format!("[... {} more lines omitted]", remaining).dimmed()
                );
            }
        }

        println!(
            "{}",
            "└─────────────────────────────────────────────────".dimmed()
        );

        // Flush to ensure immediate display
        io::stdout().flush().unwrap();
    }
}

impl ToolDisplay for ToolCallDisplay {
    fn show_call_details(&self, arguments: &Value) {
        self.show_call_details(arguments)
    }

    fn complete_success(&mut self, result: &str) {
        let duration = self.start_time.elapsed();
        self.show_result(result, false, duration);
    }

    fn complete_error(&mut self, error: &str) {
        let duration = self.start_time.elapsed();
        self.show_result(error, true, duration);
    }
}

/// Check if output should be pretty (terminal) or plain (redirected)
pub fn should_use_pretty_output() -> bool {
    // Always return true for now since we're not using atty check anymore
    true
}

/// A simple text-only display for non-interactive environments
pub struct SimpleToolDisplay {
    tool_name: String,
    start_time: std::time::Instant,
}

impl SimpleToolDisplay {
    pub fn new(tool_name: &str) -> Self {
        println!("▶ {} {}...", Self::get_tool_icon(tool_name), tool_name);

        Self {
            tool_name: tool_name.to_string(),
            start_time: std::time::Instant::now(),
        }
    }

    fn get_tool_icon(tool_name: &str) -> &'static str {
        match tool_name {
            "list_directory" => "📁",
            "read_file" => "📖",
            "write_file" => "✏️",
            "edit_file" => "🔄",
            "delete_file" => "🗑️",
            "create_directory" => "📁",
            "bash" => "💻",
            _ => "🔧",
        }
    }

    pub fn complete_success(self, result: &str) {
        let duration = self.start_time.elapsed();
        println!("✅ {} completed in {:?}", self.tool_name, duration);

        // Show limited result (max 5 lines)
        if !result.is_empty() {
            let lines: Vec<&str> = result.lines().collect();
            let total_lines = lines.len();
            let max_display_lines = 5;

            if total_lines <= max_display_lines {
                // Show all lines if within limit
                for line in lines {
                    println!("   {}", line);
                }
            } else {
                // Show first 5 lines and indicate truncation
                for line in lines.iter().take(max_display_lines) {
                    println!("   {}", line);
                }
                let remaining = total_lines - max_display_lines;
                println!(
                    "   [... {} more lines omitted] [{} bytes total]",
                    remaining,
                    result.len()
                );
            }
        }
    }

    pub fn complete_error(self, error: &str) {
        let duration = self.start_time.elapsed();
        println!("❌ {} failed in {:?}", self.tool_name, duration);

        // Show limited error (max 5 lines)
        let lines: Vec<&str> = error.lines().collect();
        let total_lines = lines.len();
        let max_display_lines = 5;

        println!("   Error:");
        if total_lines <= max_display_lines {
            // Show all lines if within limit
            for line in lines {
                println!("   {}", line);
            }
        } else {
            // Show first 5 lines and indicate truncation
            for line in lines.iter().take(max_display_lines) {
                println!("   {}", line);
            }
            let remaining = total_lines - max_display_lines;
            println!(
                "   [... {} more lines omitted] [{} bytes total]",
                remaining,
                error.len()
            );
        }
    }
}

impl ToolDisplay for SimpleToolDisplay {
    fn complete_success(&mut self, result: &str) {
        let duration = self.start_time.elapsed();
        println!("✅ {} completed in {:?}", self.tool_name, duration);

        // Show limited result (max 5 lines)
        if !result.is_empty() {
            let lines: Vec<&str> = result.lines().collect();
            let total_lines = lines.len();
            let max_display_lines = 5;

            if total_lines <= max_display_lines {
                // Show all lines if within limit
                for line in lines {
                    println!("   {}", line);
                }
            } else {
                // Show first 5 lines and indicate truncation
                for line in lines.iter().take(max_display_lines) {
                    println!("   {}", line);
                }
                let remaining = total_lines - max_display_lines;
                println!(
                    "   [... {} more lines omitted] [{} bytes total]",
                    remaining,
                    result.len()
                );
            }
        }
    }

    fn complete_error(&mut self, error: &str) {
        let duration = self.start_time.elapsed();
        println!("❌ {} failed in {:?}", self.tool_name, duration);

        // Show limited error (max 5 lines)
        let lines: Vec<&str> = error.lines().collect();
        let total_lines = lines.len();
        let max_display_lines = 5;

        println!("   Error:");
        if total_lines <= max_display_lines {
            // Show all lines if within limit
            for line in lines {
                println!("   {}", line);
            }
        } else {
            // Show first 5 lines and indicate truncation
            for line in lines.iter().take(max_display_lines) {
                println!("   {}", line);
            }
            let remaining = total_lines - max_display_lines;
            println!(
                "   [... {} more lines omitted] [{} bytes total]",
                remaining,
                error.len()
            );
        }
    }
}
