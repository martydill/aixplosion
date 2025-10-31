
/// ASCII art logo for aixplosion
pub static AIXPLOSION_LOGO: &str = r#"
     █████╗ ██╗██╗  ██╗██████╗ ██╗      ██████╗ ███████╗██╗ ██████╗ ███╗   ██╗
    ██╔══██╗██║╚██╗██╔╝██╔══██╗██║     ██╔═══██╗██╔════╝██║██╔═══██╗████╗  ██║
    ███████║██║ ╚███╔╝ ██████╔╝██║     ██║   ██║███████╗██║██║   ██║██╔██╗ ██║
    ██╔══██║██║ ██╔██╗ ██╔═══╝ ██║     ██║   ██║╚════██║██║██║   ██║██║╚██╗██║
    ██║  ██║██║██╔╝ ██╗██║     ███████╗╚██████╔╝███████║██║╚██████╔╝██║ ╚████║
    ╚═╝  ╚═╝╚═╝╚═╝  ╚═╝╚═╝     ╚══════╝ ╚═════╝ ╚══════╝╚═╝ ╚═════╝ ╚═╝  ╚═══╝
"#;

/// Minimal logo for very small terminals
pub static AIXPLOSION_LOGO_MINIMAL: &str = r#"
    ▄▀█ █ ▀▄▀ █▀█ █   █▀█ █▀ █ █▀█ █▄ █
    █▀█ █ █ █ █▀▀ █▄▄ █▄█ ▄█ █ █▄█ █ ▀█
"#;

/// Function to get the appropriate logo based on terminal width
pub fn get_logo_for_terminal() -> &'static str {
    // Try to get terminal width
    if let Ok((width, _)) = crossterm::terminal::size() {
        if width >= 80 {
            AIXPLOSION_LOGO
        } else {
            AIXPLOSION_LOGO_MINIMAL
        }
    } else {
        // Fallback to compact if we can't detect terminal size
        AIXPLOSION_LOGO_MINIMAL 
    }
}

/// Display the logo with colors
pub fn display_logo() {
    use colored::*;
    
    let logo = get_logo_for_terminal();
    
    // Display the logo with gradient effect
    for line in logo.lines() {
        if line.trim().is_empty() {
            println!("{}", line);
        } else {
            // Create a gradient effect from red to orange to yellow
            let chars: Vec<char> = line.chars().collect();
            let mut colored_line = String::new();
            
            for (i, ch) in chars.iter().enumerate() {
                let progress = i as f32 / chars.len() as f32;
                let colored_char = if progress < 0.33 {
                    ch.to_string().red()
                } else if progress < 0.66 {
                    // Custom orange approximation using true color
                    ch.to_string().truecolor(255, 95, 31) // RGB for orange
                } else {
                    ch.to_string().bright_yellow()
                };
                colored_line.push_str(&colored_char.to_string());
            }
            
            println!("{}", colored_line);
        }
    }
    
    // Add a subtitle
    println!("{}", "🔥 Your Supercharged AI Coding Agent".bright_yellow().bold());
    println!();
}
