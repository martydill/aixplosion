
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

/// Display the logo with colors using crossterm for smooth gradients
pub fn display_logo() {
    use crossterm::{
        style::{Color, Print, ResetColor, SetForegroundColor},
        queue,
    };
    use std::io::{stdout, Write};
    
    let logo = get_logo_for_terminal();
    let mut stdout = stdout();
    
    // Display the logo with smooth gradient effect
    for (line_idx, line) in logo.lines().enumerate() {
        if line.trim().is_empty() {
            queue!(stdout, Print(line), Print("\n")).ok();
        } else {
            // Create a smooth horizontal gradient for each line
            let chars: Vec<char> = line.chars().collect();
            
            // Add vertical gradient variation for more interesting effect
            let vertical_offset = line_idx as f32 / logo.lines().count() as f32;
            
            for (i, ch) in chars.iter().enumerate() {
                if *ch == ' ' {
                    queue!(stdout, Print(' ')).ok();
                    continue;
                }
                
                let horizontal_progress = i as f32 / chars.len() as f32;
                
                // Create smooth RGB gradient
                // From deep red -> orange -> yellow (fire gradient)
                let combined_progress = (horizontal_progress + vertical_offset * 0.3) % 1.0;
                
                let (r, g, b) = if combined_progress < 0.33 {
                    // Deep red to red
                    let t = combined_progress / 0.33;
                    (
                        (139.0 * (1.0 - t) + 255.0 * t) * 255.0 / 255.0,
                        (0.0 * (1.0 - t) + 0.0 * t) * 255.0 / 255.0,
                        (0.0 * (1.0 - t) + 0.0 * t) * 255.0 / 255.0,
                    )
                } else if combined_progress < 0.67 {
                    // Red to orange
                    let t = (combined_progress - 0.33) / 0.34;
                    (
                        (255.0 * (1.0 - t) + 255.0 * t) * 255.0 / 255.0,
                        (0.0 * (1.0 - t) + 165.0 * t) * 255.0 / 255.0,
                        (0.0 * (1.0 - t) + 0.0 * t) * 255.0 / 255.0,
                    )
                } else {
                    // Orange to yellow
                    let t = (combined_progress - 0.67) / 0.33;
                    (
                        (255.0 * (1.0 - t) + 255.0 * t) * 255.0 / 255.0,
                        (165.0 * (1.0 - t) + 255.0 * t) * 255.0 / 255.0,
                        (0.0 * (1.0 - t) + 0.0 * t) * 255.0 / 255.0,
                    )
                };
                
                // Add some brightness variation for more dynamic effect
                let brightness_factor = 0.7 + 0.3 * (combined_progress * std::f32::consts::PI * 2.0).sin();
                let r = (r as f32 * brightness_factor).min(255.0) as u8;
                let g = (g as f32 * brightness_factor).min(255.0) as u8;
                let b = (b as f32 * brightness_factor).min(255.0) as u8;
                
                queue!(
                    stdout,
                    SetForegroundColor(Color::Rgb { r, g, b }),
                    Print(ch)
                ).ok();
            }
            
            queue!(stdout, ResetColor, Print("\n")).ok();
        }
    }
    
    // Add a subtitle with fire gradient effect
    let subtitle = "🔥 Your Supercharged AI Coding Agent";
    for (i, ch) in subtitle.chars().enumerate() {
        let progress = i as f32 / subtitle.len() as f32;
        let (r, g, b) = if progress < 0.5 {
            // Red to orange
            let t = progress / 0.5;
            (
                255,
                (0.0 * (1.0 - t) + 165.0 * t) as u8,
                0,
            )
        } else {
            // Orange to yellow
            let t = (progress - 0.5) / 0.5;
            (
                255,
                (165.0 * (1.0 - t) + 255.0 * t) as u8,
                0,
            )
        };
        
        queue!(
            stdout,
            SetForegroundColor(Color::Rgb { r, g, b }),
            Print(ch)
        ).ok();
    }
    
    queue!(stdout, ResetColor, Print("\n\n")).ok();
    stdout.flush().ok();
}
