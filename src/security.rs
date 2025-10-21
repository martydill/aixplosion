use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::collections::HashSet;
use glob::Pattern;
use dialoguer::Select;
use log::{debug, info, warn, error};
use colored::Colorize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashSecurity {
    /// List of allowed command patterns (supports wildcards)
    pub allowed_commands: HashSet<String>,
    /// List of explicitly denied command patterns (supports wildcards)
    pub denied_commands: HashSet<String>,
    /// Whether to ask for permission for unknown commands
    pub ask_for_permission: bool,
    /// Whether to enable security mode at all
    pub enabled: bool,
}

impl Default for BashSecurity {
    fn default() -> Self {
        Self {
            // Default safe commands
            allowed_commands: HashSet::from([
                "ls".to_string(),
                "pwd".to_string(),
                "cd".to_string(),
                "cat".to_string(),
                "head".to_string(),
                "tail".to_string(),
                "grep".to_string(),
                "find".to_string(),
                "which".to_string(),
                "whereis".to_string(),
                "echo".to_string(),
                "date".to_string(),
                "whoami".to_string(),
                "id".to_string(),
                "uname".to_string(),
                "df".to_string(),
                "du".to_string(),
                "wc".to_string(),
                "sort".to_string(),
                "uniq".to_string(),
                "cut".to_string(),
                "awk".to_string(),
                "sed".to_string(),
                "git status".to_string(),
                "git log".to_string(),
                "git diff".to_string(),
                "git show".to_string(),
                "git branch".to_string(),
                "git tag".to_string(),
                "cargo check".to_string(),
                "cargo test".to_string(),
                "cargo build".to_string(),
                "cargo clippy".to_string(),
                "rustc --version".to_string(),
                "node --version".to_string(),
                "npm --version".to_string(),
                "python --version".to_string(),
                "python3 --version".to_string(),
                "pip --version".to_string(),
                "pip3 --version".to_string(),
            ]),
            denied_commands: HashSet::from([
                "rm *".to_string(),
                "sudo rm *".to_string(),
                "format".to_string(),
                "fdisk".to_string(),
                "mkfs".to_string(),
                "dd".to_string(),
                "shutdown".to_string(),
                "reboot".to_string(),
                "halt".to_string(),
                "poweroff".to_string(),
                "passwd".to_string(),
                "su".to_string(),
                "sudo su".to_string(),
                "chmod 777 *".to_string(),
                "chown *".to_string(),
                "mv *".to_string(),
                "cp *".to_string(),
            ]),
            ask_for_permission: true,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionResult {
    Allowed,
    Denied,
    RequiresPermission,
}

pub struct BashSecurityManager {
    security: BashSecurity,
}

impl BashSecurityManager {
    pub fn new(security: BashSecurity) -> Self {
        Self { security }
    }

    /// Check if a command is allowed to execute
    pub fn check_command_permission(&self, command: &str) -> PermissionResult {
        if !self.security.enabled {
            debug!("Security is disabled, allowing command: {}", command);
            return PermissionResult::Allowed;
        }

        // Extract the base command (first word) and full command for checking
        let base_command = command.split_whitespace().next().unwrap_or("").trim();
        
        debug!("Checking permission for command: {}", command);
        debug!("Base command: {}", base_command);

        // Check denied patterns first (more restrictive)
        for denied_pattern in &self.security.denied_commands {
            if self.matches_pattern(command, denied_pattern) || self.matches_pattern(base_command, denied_pattern) {
                warn!("Command '{}' matches denied pattern: {}", command, denied_pattern);
                return PermissionResult::Denied;
            }
        }

        // Check allowed patterns
        for allowed_pattern in &self.security.allowed_commands {
            if self.matches_pattern(command, allowed_pattern) || self.matches_pattern(base_command, allowed_pattern) {
                debug!("Command '{}' matches allowed pattern: {}", command, allowed_pattern);
                return PermissionResult::Allowed;
            }
        }

        // If not explicitly allowed or denied, decide based on ask_for_permission setting
        if self.security.ask_for_permission {
            info!("Command '{}' requires user permission", command);
            PermissionResult::RequiresPermission
        } else {
            warn!("Command '{}' not in allowlist and ask_for_permission is false", command);
            PermissionResult::Denied
        }
    }

    /// Ask user for permission to execute a command
    pub async fn ask_permission(&mut self, command: &str) -> Result<Option<bool>> {
        if !self.security.ask_for_permission {
            return Ok(None);
        }

        println!();
        println!("{}", "🔒 Security Check".yellow().bold());
        println!("The following command is not in the allowlist:");
        println!("  {}", command.cyan());
        println!();
        println!("Choose an action:");
        
        let options = vec![
            "Allow this time only (don't add to allowlist)".to_string(),
            "Allow and add to allowlist".to_string(),
            "Deny this command".to_string(),
        ];
        
        // Use tokio::task::spawn_blocking with timeout to prevent hanging
        let options_clone = options.clone();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30), // 30 second timeout
            tokio::task::spawn_blocking(move || {
                Select::new()
                    .with_prompt("Select an option")
                    .items(&options_clone)
                    .default(2) // Default to "Deny this command" for safety
                    .interact()
            })
        ).await;
        
        match result {
            Ok(Ok(Ok(0))) => {
                println!("{} Command allowed for this time only", "✅".green());
                Ok(Some(false)) // Allow but don't add to allowlist
            }
            Ok(Ok(Ok(1))) => {
                println!("{} Command allowed and added to allowlist", "✅".green());
                self.add_to_allowlist(command.to_string());
                Ok(Some(true)) // Allow and add to allowlist
            }
            Ok(Ok(Ok(2))) => {
                println!("{} Command denied", "❌".red());
                Ok(None) // Deny
            }
            Ok(Ok(Ok(_))) => {
                println!("{} Invalid selection, denying command for safety", "⚠️".yellow());
                Ok(None) // Deny for safety
            }
            Ok(Ok(Err(e))) => {
                error!("Failed to get user input: {}", e);
                println!("{} Failed to get user input, denying command for safety", "⚠️".yellow());
                Ok(None) // Deny for safety
            }
            Ok(Err(e)) => {
                error!("Task join error: {}", e);
                println!("{} Failed to get user input, denying command for safety", "⚠️".yellow());
                Ok(None) // Deny for safety
            }
            Err(_) => {
                error!("Permission dialog timed out after 30 seconds");
                println!("{} Permission dialog timed out, denying command for safety", "⚠️".yellow());
                Ok(None) // Deny for safety
            }
        }
    }

    /// Add a command to the allowlist
    pub fn add_to_allowlist(&mut self, command: String) {
        self.security.allowed_commands.insert(command);
    }

    /// Add a command to the denylist
    pub fn add_to_denylist(&mut self, command: String) {
        self.security.denied_commands.insert(command);
    }

    /// Remove a command from the allowlist
    pub fn remove_from_allowlist(&mut self, command: &str) -> bool {
        self.security.allowed_commands.remove(command)
    }

    /// Remove a command from the denylist
    pub fn remove_from_denylist(&mut self, command: &str) -> bool {
        self.security.denied_commands.remove(command)
    }

    /// Get current security settings
    pub fn get_security(&self) -> &BashSecurity {
        &self.security
    }

    /// Update security settings
    pub fn update_security(&mut self, security: BashSecurity) {
        self.security = security;
    }

    /// Check if a command matches a pattern (supports wildcards)
    fn matches_pattern(&self, command: &str, pattern: &str) -> bool {
        // Handle exact match
        if command == pattern {
            return true;
        }

        // Handle wildcard patterns
        if pattern.contains('*') || pattern.contains('?') {
            match Pattern::new(pattern) {
                Ok(glob_pattern) => {
                    if glob_pattern.matches(command) {
                        return true;
                    }
                }
                Err(e) => {
                    debug!("Invalid glob pattern '{}': {}", pattern, e);
                }
            }
        }

        // Handle prefix match (e.g., "git" matches "git status")
        if command.starts_with(&format!("{} ", pattern)) || command == pattern {
            return true;
        }

        false
    }

    /// Display current permissions
    pub fn display_permissions(&self) {
        println!();
        println!("{}", "🔒 Bash Security Settings".cyan().bold());
        println!();
        
        println!("{}", "Security Status:".green().bold());
        let status = if self.security.enabled { 
            "✅ Enabled".green().to_string() 
        } else { 
            "❌ Disabled".red().to_string() 
        };
        println!("  Security: {}", status);
        
        let ask_status = if self.security.ask_for_permission { 
            "✅ Enabled".green().to_string() 
        } else { 
            "❌ Disabled".red().to_string() 
        };
        println!("  Ask for permission: {}", ask_status);
        println!();
        
        println!("{} Allowed Commands ({}):", "Allowed Commands".green().bold(), self.security.allowed_commands.len());
        if self.security.allowed_commands.is_empty() {
            println!("  {}", "<No commands allowed>".dimmed());
        } else {
            let mut sorted_commands: Vec<_> = self.security.allowed_commands.iter().collect();
            sorted_commands.sort();
            for command in sorted_commands {
                println!("  ✅ {}", command.green());
            }
        }
        println!();
        
        println!("{} Denied Commands ({}):", "Denied Commands".red().bold(), self.security.denied_commands.len());
        if self.security.denied_commands.is_empty() {
            println!("  {}", "<No commands denied>".dimmed());
        } else {
            let mut sorted_commands: Vec<_> = self.security.denied_commands.iter().collect();
            sorted_commands.sort();
            for command in sorted_commands {
                println!("  ❌ {}", command.red());
            }
        }
        println!();
        
        println!("{}", "Security Tips:".yellow().bold());
        println!("  • Use wildcards: 'git *' allows all git commands");
        println!("  • Be specific: 'cargo test' is safer than 'cargo *'");
        println!("  • Review denied commands regularly");
        println!("  • Enable 'ask for permission' for unknown commands");
        println!("  • Choose 'Allow this time only' for one-off commands");
        println!("  • Choose 'Allow and add to allowlist' for trusted commands");
        println!();
    }

    /// Consume the security manager and return the updated security settings
    pub fn into_security(self) -> BashSecurity {
        self.security
    }
}