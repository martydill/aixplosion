use std::time::{Duration, SystemTime, UNIX_EPOCH};
use indicatif::{ProgressBar, ProgressStyle};
use colored::*;
use std::io::{self, Write};
use serde_json::Value;

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

/// A pretty display for tool calls with progress indicators
pub struct ToolCallDisplay {
    progress_bar: Option<ProgressBar>,
    tool_name: String,
    start_time: std::time::Instant,
}

impl ToolCallDisplay {
    /// Create a new tool call display for a specific tool
    pub fn new(tool_name: &str) -> Self {
        let progress_bar = Self::create_progress_bar(tool_name);
        
        Self {
            progress_bar,
            tool_name: tool_name.to_string(),
            start_time: std::time::Instant::now(),
        }
    }

    /// Create a styled progress bar for the tool
    fn create_progress_bar(tool_name: &str) -> Option<ProgressBar> {
        // Only show progress bar if we're in a terminal
        if !atty::is(atty::Stream::Stdout) {
            return None;
        }

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈ ")
                .template("{spinner:.green} {msg}")
                .unwrap()
        );
        
        let icon = Self::get_tool_icon(tool_name);
        let action = Self::get_tool_action(tool_name);
        pb.set_message(format!("{} {} {}...", icon, tool_name.cyan().bold(), action));
        
        Some(pb)
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

    /// Get a descriptive action for each tool
    fn get_tool_action(tool_name: &str) -> &'static str {
        match tool_name {
            "list_directory" => "listing directory",
            "read_file" => "reading file",
            "write_file" => "writing to file",
            "edit_file" => "editing file",
            "delete_file" => "deleting",
            "create_directory" => "creating directory",
            "bash" => "executing command",
            _ => "processing",
        }
    }

    /// Display the tool call details
    pub fn show_call_details(&self, arguments: &Value) {
        let icon = Self::get_tool_icon(&self.tool_name);
        
        println!();
        println!("{}", "┌─────────────────────────────────────────────────".dimmed());
        println!("{} {} {} {}",
                 "│".dimmed(),
                 icon,
                 format!("Tool Call: {}", self.tool_name.cyan().bold()),
                 format!("[{}]", self.get_current_time()).dimmed()
        );
        
        // Display relevant arguments based on tool type
        match self.tool_name.as_str() {
            "list_directory" => {
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    println!("{} {} {} {}", "│".dimmed(), "📍".yellow(), "Path:".bold(), path.green());
                }
            }
            "read_file" | "write_file" | "edit_file" | "delete_file" | "create_directory" => {
                if let Some(path) = arguments.get("path").and_then(|v| v.as_str()) {
                    println!("{} {} {} {}", "│".dimmed(), "📄".yellow(), "File:".bold(), path.green());
                }

                // Additional info for specific file operations
                match self.tool_name.as_str() {
                    "write_file" => {
                        if let Some(content) = arguments.get("content").and_then(|v| v.as_str()) {
                            println!("{} {} {} {} bytes", "│".dimmed(), "📝".yellow(), "Size:".bold(), content.len().to_string().green());
                        }
                    }
                    "edit_file" => {
                        if let Some(old_text) = arguments.get("old_text").and_then(|v| v.as_str()) {
                            println!("{} {} {} {} bytes", "│".dimmed(), "📝".yellow(), "Old:".bold(), old_text.len().to_string().yellow());
                        }
                        if let Some(new_text) = arguments.get("new_text").and_then(|v| v.as_str()) {
                            println!("{} {} {} {} bytes", "│".dimmed(), "📝".yellow(), "New:".bold(), new_text.len().to_string().green());
                        }
                    }
                    _ => {}
                }
            }
            "bash" => {
                if let Some(command) = arguments.get("command").and_then(|v| v.as_str()) {
                    println!("{} {} {} {}", "│".dimmed(), "💻".yellow(), "Command:".bold(), command.green());
                }
            }
            _ => {}
        }
        
        println!("{}", "└─────────────────────────────────────────────────".dimmed());
        
        // Flush to ensure immediate display
        io::stdout().flush().unwrap();
    }

    /// Complete the tool call with success
    pub fn complete_success(mut self, result: &str) {
        let duration = self.start_time.elapsed();
        
        if let Some(pb) = self.progress_bar.take() {
            pb.finish_with_message(format!("✅ {} completed in {:?}", 
                                         self.tool_name.cyan().bold(), 
                                         duration));
        }

        self.show_result(result, false, duration);
    }

    /// Complete the tool call with error
    pub fn complete_error(mut self, error: &str) {
        let duration = self.start_time.elapsed();
        
        if let Some(pb) = self.progress_bar.take() {
            pb.finish_with_message(format!("❌ {} failed in {:?}", 
                                         self.tool_name.cyan().bold(), 
                                         duration));
        }

        self.show_result(error, true, duration);
    }

    /// Show the tool result with appropriate formatting
    fn show_result(&self, content: &str, is_error: bool, duration: Duration) {
        let icon = if is_error { "❌" } else { "✅" };
        let status = if is_error { "FAILED".red().bold() } else { "SUCCESS".green().bold() };
        
        println!();
        println!("{}", "┌─────────────────────────────────────────────────".dimmed());
        println!("{} {} {} {} ({})", 
                 "│".dimmed(),
                 icon,
                 format!("Result: {}", self.tool_name.cyan().bold()),
                 status,
                 format!("{:.2}s", duration.as_secs_f64()).dimmed()
        );
        
        // Truncate very long content for display
        let display_content = if content.len() > 500 {
            format!("{}...\n[{} bytes total, truncated for display]", 
                   &content[..500], 
                   content.len())
        } else {
            content.to_string()
        };
        
        // Display content with appropriate formatting
        if is_error {
            println!("{} {}", "│".dimmed(), "Error:".red().bold());
            for line in display_content.lines().take(10) {
                println!("{}   {}", "│".dimmed(), line.red());
            }
            if display_content.lines().count() > 10 {
                println!("{}   {}", "│".dimmed(), "[...]".dimmed());
            }
        } else {
            println!("{} {}", "│".dimmed(), "Output:".green().bold());
            for line in display_content.lines().take(10) {
                println!("{}   {}", "│".dimmed(), line);
            }
            if display_content.lines().count() > 10 {
                println!("{}   {}", "│".dimmed(), "[...]".dimmed());
            }
        }
        
        println!("{}", "└─────────────────────────────────────────────────".dimmed());
        println!();
        
        // Flush to ensure immediate display
        io::stdout().flush().unwrap();
    }
}

impl ToolDisplay for ToolCallDisplay {
    fn show_call_details(&self, arguments: &Value) {
        self.show_call_details(arguments)
    }

    fn complete_success(&mut self, result: &str) {
        // Take ownership of the progress bar and complete it
        if let Some(pb) = self.progress_bar.take() {
            let duration = self.start_time.elapsed();
            pb.finish_with_message(format!("✅ {} completed in {:?}",
                                         self.tool_name.cyan().bold(),
                                         duration));
        }

        self.show_result(result, false, self.start_time.elapsed());
    }

    fn complete_error(&mut self, error: &str) {
        // Take ownership of the progress bar and complete it
        if let Some(pb) = self.progress_bar.take() {
            let duration = self.start_time.elapsed();
            pb.finish_with_message(format!("❌ {} failed in {:?}",
                                         self.tool_name.cyan().bold(),
                                         duration));
        }

        self.show_result(error, true, self.start_time.elapsed());
    }
}

impl Drop for ToolCallDisplay {
    fn drop(&mut self) {
        // Ensure progress bar is finished
        if let Some(pb) = self.progress_bar.take() {
            pb.finish();
        }
    }
}

/// Check if output should be pretty (terminal) or plain (redirected)
pub fn should_use_pretty_output() -> bool {
    atty::is(atty::Stream::Stdout)
}

/// A simple text-only display for non-interactive environments
pub struct SimpleToolDisplay {
    tool_name: String,
    start_time: std::time::Instant,
}

impl SimpleToolDisplay {
    pub fn new(tool_name: &str) -> Self {
        println!("▶ {} {}...", 
                Self::get_tool_icon(tool_name),
                tool_name);
        
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
        
        // Show abbreviated result
        if !result.is_empty() {
            let preview = if result.len() > 200 {
                format!("{}... [{} bytes]", &result[..200], result.len())
            } else {
                result.to_string()
            };
            println!("   {}", preview);
        }
    }

    pub fn complete_error(self, error: &str) {
        let duration = self.start_time.elapsed();
        println!("❌ {} failed in {:?}", self.tool_name, duration);
        println!("   Error: {}", error);
    }
}

impl ToolDisplay for SimpleToolDisplay {
    fn complete_success(&mut self, result: &str) {
        let duration = self.start_time.elapsed();
        println!("✅ {} completed in {:?}", self.tool_name, duration);

        // Show abbreviated result
        if !result.is_empty() {
            let preview = if result.len() > 200 {
                format!("{}... [{} bytes]", &result[..200], result.len())
            } else {
                result.to_string()
            };
            println!("   {}", preview);
        }
    }

    fn complete_error(&mut self, error: &str) {
        let duration = self.start_time.elapsed();
        println!("❌ {} failed in {:?}", self.tool_name, duration);
        println!("   Error: {}", error);
    }
}