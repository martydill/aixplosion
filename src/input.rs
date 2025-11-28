use anyhow::Result;
use colored::*;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal, ExecutableCommand, QueueableCommand,
};
use std::cell::Cell;
use std::io::{self, Write};
use std::thread_local;

use crate::autocomplete;
use crate::formatter;

thread_local! {
    static LAST_RENDERED_LINES: Cell<usize> = Cell::new(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_history() {
        let mut history = InputHistory::new();

        // Test adding entries
        history.add_entry("test1".to_string());
        history.add_entry("test2".to_string());

        assert_eq!(history.entries.len(), 2);
        assert_eq!(history.entries[0], "test1");
        assert_eq!(history.entries[1], "test2");

        // Test navigation
        let current_input = "current";
        let prev = history.navigate_up(current_input);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap(), "test2");

        let next = history.navigate_down();
        assert!(next.is_some());
        assert_eq!(next.unwrap(), current_input);

        // Test that empty entries are not added
        history.add_entry("".to_string());
        assert_eq!(history.entries.len(), 2); // Should not increase

        // Test that duplicates are not added
        history.add_entry("test2".to_string());
        assert_eq!(history.entries.len(), 2); // Should not increase
    }
}

/// Input history management
pub struct InputHistory {
    entries: Vec<String>,
    index: Option<usize>,
    temp_input: String,
}

impl InputHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: None,
            temp_input: String::new(),
        }
    }

    pub fn add_entry(&mut self, entry: String) {
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

    pub fn navigate_up(&mut self, current_input: &str) -> Option<String> {
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

    pub fn navigate_down(&mut self) -> Option<String> {
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

    pub fn reset_navigation(&mut self) {
        self.index = None;
        self.temp_input.clear();
    }
}

/// Read input with autocompletion support and file highlighting
pub fn read_input_with_completion_and_highlighting(
    formatter: Option<&formatter::CodeFormatter>,
    history: &mut InputHistory,
) -> Result<String> {
    // Enable raw mode for keyboard input
    terminal::enable_raw_mode()?;

    let mut input = String::new();
    let mut cursor_pos = 0;

    // Reset render tracking when starting a new prompt
    LAST_RENDERED_LINES.with(|cell| cell.set(1));

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
                        let highlighted_input =
                            fmt.format_input_with_file_highlighting(trimmed_input);
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
                // Handle tab completion with cursor position
                if let Some(completion) = autocomplete::handle_tab_completion(&input, cursor_pos) {
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
pub fn read_multiline_input(
    initial_line: &str,
    formatter: Option<&formatter::CodeFormatter>,
) -> Result<String> {
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

fn calculate_line_usage(content_len: usize, prompt_len: usize, terminal_width: usize) -> usize {
    let width = terminal_width.max(1);
    ((prompt_len + content_len).saturating_sub(1) / width) + 1
}

fn clear_previous_render(stdout: &mut impl Write, lines_rendered: usize) -> Result<()> {
    use crossterm::{cursor::MoveToColumn, cursor::MoveUp, terminal::Clear, terminal::ClearType};

    if lines_rendered > 1 {
        let lines_to_clear = lines_rendered.saturating_sub(1).min(u16::MAX as usize) as u16;
        if lines_to_clear > 0 {
            stdout.queue(MoveUp(lines_to_clear))?;
        }
    }
    stdout
        .queue(MoveToColumn(0))?
        .queue(Clear(ClearType::FromCursorDown))?;

    Ok(())
}

fn redraw_input_line(
    input: &str,
    cursor_pos: usize,
    formatter: Option<&formatter::CodeFormatter>,
    use_highlighting: bool,
) -> Result<()> {
    use crossterm::{
        cursor::{MoveToColumn, MoveUp},
        style::{Print, ResetColor},
        terminal,
    };

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let terminal_width = terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
        .max(1);
    let prompt_length = 2; // "> " length
    let visible_len = input.chars().count();
    let cursor_visible = input.chars().take(cursor_pos).count();

    let total_lines = calculate_line_usage(visible_len, prompt_length, terminal_width);
    let cursor_line = (prompt_length + cursor_visible) / terminal_width;
    let cursor_column = (prompt_length + cursor_visible) % terminal_width;

    let previous_lines = LAST_RENDERED_LINES.with(|cell| {
        let prev = cell.get();
        cell.set(total_lines);
        prev.max(1)
    });

    clear_previous_render(&mut stdout, previous_lines)?;

    // Display prompt
    stdout.queue(Print("> "))?;

    if use_highlighting {
        // Only apply highlighting if formatter is available AND input contains @file references
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
    } else {
        stdout.queue(Print(input))?;
    }

    // After printing the full input, we're on the last rendered line.
    // Move up to the correct line and column for the cursor.
    let lines_to_move_up = total_lines
        .saturating_sub(cursor_line + 1)
        .min(u16::MAX as usize);
    if lines_to_move_up > 0 {
        stdout.queue(MoveUp(lines_to_move_up as u16))?;
    }

    stdout
        .queue(MoveToColumn(cursor_column as u16))?
        .queue(ResetColor)?
        .flush()?;

    Ok(())
}

/// Redraw the input line with file highlighting and proper cursor positioning
fn redraw_input_line_with_highlighting(
    input: &str,
    cursor_pos: usize,
    formatter: Option<&formatter::CodeFormatter>,
) -> Result<()> {
    redraw_input_line(input, cursor_pos, formatter, true)
}

/// Fast redraw without highlighting for cursor movements (up/down arrows, etc.)
fn redraw_input_line_fast(input: &str, cursor_pos: usize) -> Result<()> {
    redraw_input_line(input, cursor_pos, None, false)
}
