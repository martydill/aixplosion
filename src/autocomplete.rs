use crossterm::{cursor, style::Print, terminal, ExecutableCommand, QueueableCommand};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Get completion suggestions based on current input
pub fn get_completion(input: &str) -> Option<String> {
    let input = input.trim_start();

    // Command completions
    let commands = vec![
        "/help",
        "/stats",
        "/usage",
        "/context",
        "/clear",
        "/reset-stats",
        "/permissions",
        "/file-permissions",
        "/mcp",
        "/exit",
        "/quit",
    ];

    // File completion for @ syntax
    if input.starts_with('@') {
        let path_part = &input[1..];
        if let Some(completion) = complete_file_path(path_part) {
            return Some(format!("@{}", completion));
        }
    }

    // Command completion
    for cmd in commands {
        if cmd.starts_with(input) && cmd != input {
            return Some(cmd.to_string());
        }
    }

    // MCP command completions
    if input.starts_with("/mcp ") {
        let mcp_part = &input[5..];
        let mcp_commands = vec![
            "list",
            "add",
            "remove",
            "connect",
            "disconnect",
            "reconnect",
            "tools",
            "connect-all",
            "disconnect-all",
            "test",
            "help",
        ];

        for cmd in mcp_commands {
            if cmd.starts_with(mcp_part) && cmd != mcp_part {
                return Some(format!("/mcp {}", cmd));
            }
        }
    }

    // Permission command completions
    if input.starts_with("/permissions ") {
        let perm_part = &input[13..];
        let perm_commands = vec![
            "show",
            "list",
            "test",
            "allow",
            "deny",
            "remove-allow",
            "remove-deny",
            "enable",
            "disable",
            "ask-on",
            "ask-off",
            "help",
        ];

        for cmd in perm_commands {
            if cmd.starts_with(perm_part) && cmd != perm_part {
                return Some(format!("/permissions {}", cmd));
            }
        }
    }

    if input.starts_with("/file-permissions ") {
        let perm_part = &input[18..];
        let perm_commands = vec![
            "show",
            "list",
            "test",
            "enable",
            "disable",
            "ask-on",
            "ask-off",
            "reset-session",
            "help",
        ];

        for cmd in perm_commands {
            if cmd.starts_with(perm_part) && cmd != perm_part {
                return Some(format!("/file-permissions {}", cmd));
            }
        }
    }

    None
}

/// Complete file paths for @ syntax
fn complete_file_path(path_part: &str) -> Option<String> {
    let (dir_part, file_prefix) = if let Some(last_slash) = path_part.rfind('/') {
        (&path_part[..last_slash], &path_part[last_slash + 1..])
    } else if let Some(last_slash) = path_part.rfind('\\') {
        (&path_part[..last_slash], &path_part[last_slash + 1..])
    } else {
        ("", path_part)
    };

    let search_dir = if dir_part.is_empty() {
        Path::new(".")
    } else {
        Path::new(dir_part)
    };

    if let Ok(entries) = fs::read_dir(search_dir) {
        let mut matches: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                file_name_str.starts_with(file_prefix)
            })
            .map(|entry| {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() {
                    format!("{}/", file_name)
                } else {
                    file_name
                }
            })
            .collect();

        matches.sort();

        if let Some(first_match) = matches.first() {
            if matches.len() == 1 {
                // Single match - return it
                let full_path = if dir_part.is_empty() {
                    first_match.clone()
                } else {
                    format!("{}/{}", dir_part, first_match)
                };
                Some(full_path)
            } else {
                // Multiple matches - find common prefix
                let common_prefix = find_common_prefix(&matches);
                let full_path = if dir_part.is_empty() {
                    common_prefix
                } else {
                    format!("{}/{}", dir_part, common_prefix)
                };
                Some(full_path)
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Find common prefix among multiple strings
fn find_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }

    let first = &strings[0];
    let mut end = first.len();

    for s in strings.iter().skip(1) {
        end = end.min(s.len());
        while !first[..end].starts_with(&s[..end]) {
            end -= 1;
        }
    }

    first[..end].to_string()
}

/// Handle tab completion in raw mode
pub fn handle_tab_completion(input: &str) -> Option<String> {
    if let Some(completion) = get_completion(input) {
        // Clear current line and show completion
        std::io::stdout()
            .execute(terminal::Clear(terminal::ClearType::CurrentLine))
            .unwrap()
            .execute(cursor::MoveToColumn(0))
            .unwrap()
            .queue(Print("> "))
            .unwrap()
            .queue(Print(&completion))
            .unwrap()
            .flush()
            .unwrap();

        Some(completion)
    } else {
        None
    }
}
