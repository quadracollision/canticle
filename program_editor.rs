use winit::event::VirtualKeyCode;
use crate::square::Program;
use crate::programmer::SimpleProgramParser;
use std::time::{Duration, Instant};
use clipboard::{ClipboardProvider, ClipboardContext};
use crate::font;

#[derive(Clone, Debug)]
pub struct ProgramEditor {
    pub program_text: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub parser: SimpleProgramParser,
    pub scroll_offset: usize,
    // Key repeat timing
    last_key_repeat: Option<Instant>,
    key_repeat_delay: Duration,
    key_repeat_rate: Duration,
}

#[derive(Debug)]
pub enum ProgramEditorAction {
    SaveProgram(Program),
    SaveAndCompile,
    CloseWithoutSaving,
    SaveToFile,
    LoadFromFile,
    Continue,
    None,
}

impl ProgramEditor {
    pub fn new() -> Self {
        Self {
            program_text: vec![
                "def my_program".to_string(),
                "".to_string(),
            ],
            cursor_line: 0,
            cursor_col: "def my_program".len(), // Position cursor at end of first line
            parser: SimpleProgramParser::new(),
            scroll_offset: 0,
            last_key_repeat: None,
            key_repeat_delay: Duration::from_millis(500),
            key_repeat_rate: Duration::from_millis(100), // Slower to prevent double deletions
        }
    }

    pub fn new_with_text(text: Vec<String>) -> Self {
        let mut editor = Self::new();
        editor.program_text = text.clone();
        // Position cursor at end of first line if it exists
        if !text.is_empty() {
            editor.cursor_col = text[0].len();
        }
        editor
    }

    pub fn new_empty() -> Self {
        Self {
            program_text: vec![
                "def my_program".to_string(),
                "".to_string(),
            ],
            cursor_line: 0,
            cursor_col: "def my_program".len(), // Position cursor at end of first line
            parser: SimpleProgramParser::new(),
            scroll_offset: 0,
            last_key_repeat: None,
            key_repeat_delay: Duration::from_millis(500),
            key_repeat_rate: Duration::from_millis(100),
        }
    }

    pub fn new_truly_empty() -> Self {
        Self {
            program_text: vec!["".to_string()],
            cursor_line: 0,
            cursor_col: 0,
            parser: SimpleProgramParser::new(),
            scroll_offset: 0,
            last_key_repeat: None,
            key_repeat_delay: Duration::from_millis(500),
            key_repeat_rate: Duration::from_millis(100),
        }
    }

    pub fn get_program(&self) -> Program {
        let program_source = self.program_text.join("\n");
        match self.parser.parse_program(&program_source) {
            Ok(mut program) => {
                program.source_text = Some(self.program_text.clone());
                program
            },
            Err(_) => {
                // Return a program with the raw text preserved, even if parsing fails
                // This allows users to save work-in-progress code
                let program_name = if let Some(first_line) = self.program_text.first() {
                    if first_line.starts_with("def ") {
                        first_line.strip_prefix("def ").unwrap_or("my_program").trim().to_string()
                    } else {
                        "my_program".to_string()
                    }
                } else {
                    "my_program".to_string()
                };
                
                Program {
                    name: program_name,
                    instructions: vec![], // Empty instructions but name is preserved
                    source_text: Some(self.program_text.clone()), // Preserve source text
                }
            }
        }
    }
    
    /// Get all programs defined in the editor text
    pub fn get_all_programs(&self) -> Vec<Program> {
        let program_source = self.program_text.join("\n");
        match self.parser.parse_multiple_programs(&program_source) {
            Ok(mut programs) => {
                // Add source text to all programs
                for program in &mut programs {
                    program.source_text = Some(self.program_text.clone());
                }
                programs
            },
            Err(error_msg) => {
                // Instead of falling back, preserve the user's code with error comments
                let mut commented_text = self.program_text.clone();
                
                // Add error comment at the top
                commented_text.insert(0, format!("// SYNTAX ERROR: {}", error_msg));
                commented_text.insert(1, "// Fix the error above to make this code functional".to_string());
                commented_text.insert(2, "".to_string()); // Empty line for readability
                
                // Extract program name from the first def line if possible
                let program_name = if let Some(def_line) = self.program_text.iter().find(|line| line.starts_with("def ")) {
                    def_line.strip_prefix("def ").unwrap_or("my_program").trim().to_string()
                } else {
                    "my_program".to_string()
                };
                
                // Return a program with preserved source text but empty instructions
                vec![Program {
                    name: program_name,
                    instructions: vec![], // Empty instructions due to syntax error
                    source_text: Some(commented_text), // Preserve source with error comments
                }]
            }
        }
    }
    
    pub fn get_program_text(&self) -> Vec<String> {
        self.program_text.clone()
    }

    /// Extract the program name from the first "def" line for use in file dialogs
    pub fn get_program_name(&self) -> String {
        for line in &self.program_text {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") {
                let name = trimmed.strip_prefix("def ").unwrap_or("my_program").trim();
                // Remove any invalid filename characters
                return name.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .collect::<String>()
                    .trim_matches('_')
                    .to_string();
            }
        }
        "my_program".to_string()
    }

    pub fn handle_input(&mut self, input: &winit_input_helper::WinitInputHelper) -> ProgramEditorAction {
        self.handle_input_with_context(input, false)
    }

    pub fn handle_input_with_context(&mut self, input: &winit_input_helper::WinitInputHelper, from_library: bool) -> ProgramEditorAction {
        // Handle Escape key - save and compile
        if input.key_pressed(VirtualKeyCode::Escape) {
            return ProgramEditorAction::SaveAndCompile;
        }

        // Handle Ctrl+Q - close without saving
        if input.held_control() && input.key_pressed(VirtualKeyCode::Q) {
            return ProgramEditorAction::CloseWithoutSaving;
        }

        // Handle file operations
        if input.held_shift() && input.key_pressed(VirtualKeyCode::Space) {
            return ProgramEditorAction::SaveToFile;
        }
        if input.held_control() && input.key_pressed(VirtualKeyCode::Space) {
            return ProgramEditorAction::LoadFromFile;
        }

        // Handle clipboard operations
        if input.held_control() {
            if input.key_pressed(VirtualKeyCode::A) {
                // Select all - move cursor to end
                self.cursor_line = self.program_text.len().saturating_sub(1);
                self.cursor_col = self.program_text[self.cursor_line].len();
            } else if input.key_pressed(VirtualKeyCode::C) {
                // Copy all text to clipboard
                if let Ok(mut ctx) = ClipboardContext::new() {
                    let text = self.program_text.join("\n");
                    let _ = ctx.set_contents(text);
                }
            } else if input.key_pressed(VirtualKeyCode::V) {
                // Paste from clipboard
                if let Ok(mut ctx) = ClipboardContext::new() {
                    if let Ok(clipboard_text) = ctx.get_contents() {
                        // Clear current content and replace with clipboard content
                        self.program_text = clipboard_text.lines().map(|s| s.to_string()).collect();
                        if self.program_text.is_empty() {
                            self.program_text.push(String::new());
                        }
                        // Move cursor to end
                        self.cursor_line = self.program_text.len().saturating_sub(1);
                        self.cursor_col = self.program_text[self.cursor_line].len();
                    }
                }
            }
        }

        // Cursor movement with key repeat support
        if self.should_handle_key_repeat(input, VirtualKeyCode::Up) {
            if self.cursor_line > 0 {
                self.cursor_line -= 1;
                self.cursor_col = self.cursor_col.min(self.program_text[self.cursor_line].len());
            }
        }
        if self.should_handle_key_repeat(input, VirtualKeyCode::Down) {
            if self.cursor_line < self.program_text.len().saturating_sub(1) {
                self.cursor_line += 1;
                self.cursor_col = self.cursor_col.min(self.program_text[self.cursor_line].len());
            }
        }
        if self.should_handle_key_repeat(input, VirtualKeyCode::Left) {
            if self.cursor_col > 0 {
                self.cursor_col -= 1;
            } else if self.cursor_line > 0 {
                self.cursor_line -= 1;
                self.cursor_col = self.program_text[self.cursor_line].len();
            }
        }
        if self.should_handle_key_repeat(input, VirtualKeyCode::Right) {
            if self.cursor_col < self.program_text[self.cursor_line].len() {
                self.cursor_col += 1;
            } else if self.cursor_line < self.program_text.len() - 1 {
                self.cursor_line += 1;
                self.cursor_col = 0;
            }
        }

        // Update scroll offset to keep cursor visible
        self.update_scroll_offset();

        // Text editing
        if input.key_pressed(VirtualKeyCode::Return) {
            // Split current line at cursor position
            let current_line = self.program_text[self.cursor_line].clone();
            let (left, right) = current_line.split_at(self.cursor_col);
            self.program_text[self.cursor_line] = left.to_string();
            self.program_text.insert(self.cursor_line + 1, right.to_string());
            self.cursor_line += 1;
            self.cursor_col = 0;
            self.update_scroll_offset();
        }

        // Handle backspace with proper key repeat
        if self.should_handle_key_repeat(input, VirtualKeyCode::Back) {
            if input.held_shift() {
                // Shift+Backspace: Delete entire line
                if self.program_text.len() > 1 {
                    self.program_text.remove(self.cursor_line);
                    // Adjust cursor position after line deletion
                    if self.cursor_line >= self.program_text.len() {
                        self.cursor_line = self.program_text.len().saturating_sub(1);
                    }
                    self.cursor_col = self.cursor_col.min(self.program_text[self.cursor_line].len());
                } else {
                    // If only one line left, just clear it
                    self.program_text[0].clear();
                    self.cursor_col = 0;
                }
            } else {
                // Normal backspace behavior
                if self.cursor_col > 0 {
                    // Remove character before cursor
                    self.program_text[self.cursor_line].remove(self.cursor_col - 1);
                    self.cursor_col -= 1;
                } else if self.cursor_line > 0 {
                    // Join with previous line
                    let current_line = self.program_text.remove(self.cursor_line);
                    self.cursor_col = self.program_text[self.cursor_line - 1].len();
                    self.program_text[self.cursor_line - 1].push_str(&current_line);
                    self.cursor_line -= 1;
                }
            }
        }

        if self.should_handle_key_repeat(input, VirtualKeyCode::Delete) {
            if self.cursor_col < self.program_text[self.cursor_line].len() {
                // Remove character at cursor
                self.program_text[self.cursor_line].remove(self.cursor_col);
            } else if self.cursor_line < self.program_text.len() - 1 {
                // Join with next line
                let next_line = self.program_text.remove(self.cursor_line + 1);
                self.program_text[self.cursor_line].push_str(&next_line);
            }
        }

        // Character input
        self.handle_character_input(input, from_library);

        ProgramEditorAction::Continue
    }

    fn handle_character_input(&mut self, input: &winit_input_helper::WinitInputHelper, from_library: bool) {
        // Skip character input handling when using ReceivedCharacter events
        // This prevents duplicate character insertion
        // Control keys (backspace, delete, arrows, etc.) are still handled in handle_input_with_context
        return;
        
        let shift_pressed = input.held_shift();
        
        // Handle letter keys
        for key_code in [
            VirtualKeyCode::A, VirtualKeyCode::B, VirtualKeyCode::C, VirtualKeyCode::D, VirtualKeyCode::E,
            VirtualKeyCode::F, VirtualKeyCode::G, VirtualKeyCode::H, VirtualKeyCode::I, VirtualKeyCode::J,
            VirtualKeyCode::K, VirtualKeyCode::L, VirtualKeyCode::M, VirtualKeyCode::N, VirtualKeyCode::O,
            VirtualKeyCode::P, VirtualKeyCode::Q, VirtualKeyCode::R, VirtualKeyCode::S, VirtualKeyCode::T,
            VirtualKeyCode::U, VirtualKeyCode::V, VirtualKeyCode::W, VirtualKeyCode::X, VirtualKeyCode::Y,
            VirtualKeyCode::Z,
        ] {
            if input.key_pressed(key_code) {
                let ch = match key_code {
                    VirtualKeyCode::A => if shift_pressed { 'A' } else { 'a' },
                    VirtualKeyCode::B => if shift_pressed { 'B' } else { 'b' },
                    VirtualKeyCode::C => if shift_pressed { 'C' } else { 'c' },
                    VirtualKeyCode::D => if shift_pressed { 'D' } else { 'd' },
                    VirtualKeyCode::E => if shift_pressed { 'E' } else { 'e' },
                    VirtualKeyCode::F => if shift_pressed { 'F' } else { 'f' },
                    VirtualKeyCode::G => if shift_pressed { 'G' } else { 'g' },
                    VirtualKeyCode::H => if shift_pressed { 'H' } else { 'h' },
                    VirtualKeyCode::I => if shift_pressed { 'I' } else { 'i' },
                    VirtualKeyCode::J => if shift_pressed { 'J' } else { 'j' },
                    VirtualKeyCode::K => if shift_pressed { 'K' } else { 'k' },
                    VirtualKeyCode::L => if shift_pressed { 'L' } else { 'l' },
                    VirtualKeyCode::M => if shift_pressed { 'M' } else { 'm' },
                    VirtualKeyCode::N => if shift_pressed { 'N' } else { 'n' },
                    VirtualKeyCode::O => if shift_pressed { 'O' } else { 'o' },
                    VirtualKeyCode::P => if shift_pressed { 'P' } else { 'p' },
                    VirtualKeyCode::Q => if shift_pressed { 'Q' } else { 'q' },
                    VirtualKeyCode::R => if shift_pressed { 'R' } else { 'r' },
                    VirtualKeyCode::S => if shift_pressed { 'S' } else { 's' },
                    VirtualKeyCode::T => if shift_pressed { 'T' } else { 't' },
                    VirtualKeyCode::U => if shift_pressed { 'U' } else { 'u' },
                    VirtualKeyCode::V => if shift_pressed { 'V' } else { 'v' },
                    VirtualKeyCode::W => if shift_pressed { 'W' } else { 'w' },
                    VirtualKeyCode::X => if shift_pressed { 'X' } else { 'x' },
                    VirtualKeyCode::Y => if shift_pressed { 'Y' } else { 'y' },
                    VirtualKeyCode::Z => if shift_pressed { 'Z' } else { 'z' },
                    _ => continue,
                };
                self.insert_character(ch);
                return;
            }
        }
        
        // Handle number keys
        for (key_code, normal_char, shift_char) in [
            (VirtualKeyCode::Key0, '0', ')'),
            (VirtualKeyCode::Key1, '1', '!'),
            (VirtualKeyCode::Key2, '2', '@'),
            (VirtualKeyCode::Key3, '3', '#'),
            (VirtualKeyCode::Key4, '4', '$'),
            (VirtualKeyCode::Key5, '5', '%'),
            (VirtualKeyCode::Key6, '6', '^'),
            (VirtualKeyCode::Key7, '7', '&'),
            (VirtualKeyCode::Key8, '8', '*'),
            (VirtualKeyCode::Key9, '9', '('),
        ] {
            if input.key_pressed(key_code) {
                let ch = if shift_pressed { shift_char } else { normal_char };
                self.insert_character(ch);
                return;
            }
        }
        
        // Handle special characters
        if input.key_pressed(VirtualKeyCode::Space) {
            self.insert_character(' ');
        } else if input.key_pressed(VirtualKeyCode::Minus) {
            let ch = if shift_pressed { '_' } else { '-' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::Equals) {
            let ch = if shift_pressed { '+' } else { '=' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::LBracket) {
            let ch = if shift_pressed { '{' } else { '[' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::RBracket) {
            let ch = if shift_pressed { '}' } else { ']' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::Backslash) {
            let ch = if shift_pressed { '|' } else { '\\' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::Semicolon) {
            let ch = if shift_pressed { ':' } else { ';' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::Apostrophe) {
            let ch = if shift_pressed { '"' } else { '\'' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::Grave) {
            let ch = if shift_pressed { '~' } else { '`' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::Comma) {
            let ch = if shift_pressed { '<' } else { ',' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::Period) {
            let ch = if shift_pressed { '>' } else { '.' };
            self.insert_character(ch);
        } else if input.key_pressed(VirtualKeyCode::Slash) {
            let ch = if shift_pressed { '?' } else { '/' };
            self.insert_character(ch);
        }
    }
    
    pub fn insert_character(&mut self, ch: char) {
        self.program_text[self.cursor_line].insert(self.cursor_col, ch);
        self.cursor_col += 1;
    }

    fn should_handle_key_repeat(&mut self, input: &winit_input_helper::WinitInputHelper, key: VirtualKeyCode) -> bool {
        // Simple approach: just use winit's built-in key repeat
        input.key_pressed(key)
    }

    fn update_scroll_offset(&mut self) {
        const VISIBLE_LINES: usize = 19; // Maximum visible lines before scrolling
        
        // Scroll down if cursor is below visible area
        if self.cursor_line >= self.scroll_offset + VISIBLE_LINES {
            self.scroll_offset = self.cursor_line - VISIBLE_LINES + 1;
        }
        
        // Scroll up if cursor is above visible area
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        }
    }

    pub fn draw_syntax_highlighted_text(&self, frame: &mut [u8], text: &str, x: usize, y: usize) {
        font::draw_syntax_highlighted_text(frame, text, x, y, 640);
    }

    pub fn draw_program_editor(&self, frame: &mut [u8], title: &str, instructions: &str) {
        let menu_x = 30;
        let menu_y = 30;
        let menu_width = 580;
        let menu_height = 420;

        // Draw editor background
        draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
        draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);

        // Draw title bar
        font::draw_text(frame, title, menu_x + 10, menu_y + 5, [255, 255, 255], false, 640);
        font::draw_text(frame, instructions, menu_x + 10, menu_y + 25, [180, 180, 180], false, 640);

        // Draw line number background
        let line_num_width = 40;
        for y in (menu_y + 45)..(menu_y + menu_height - 10) {
            for x in (menu_x + 5)..(menu_x + line_num_width) {
                if x < 640 && y < 480 {
                    let pixel_index = (y * 640 + x) * 4;
                    if pixel_index + 3 < frame.len() {
                        frame[pixel_index] = 40;     // R
                        frame[pixel_index + 1] = 40; // G
                        frame[pixel_index + 2] = 50; // B
                        frame[pixel_index + 3] = 255; // A
                    }
                }
            }
        }

        // Draw program text with line numbers and cursor
        let text_start_x = menu_x + line_num_width + 10;
        const VISIBLE_LINES: usize = 19;
        
        for display_line in 0..VISIBLE_LINES {
            let actual_line = self.scroll_offset + display_line;
            if actual_line >= self.program_text.len() {
                break;
            }
            
            let line = &self.program_text[actual_line];
            let y_pos = menu_y + 50 + display_line * 18;
            let is_cursor_line = actual_line == self.cursor_line;
            
            // Draw line number
            let line_num = format!("{:2}", actual_line + 1);
            let line_num_color = if is_cursor_line { [255, 255, 100] } else { [120, 120, 120] };
            font::draw_text(frame, &line_num, menu_x + 8, y_pos, line_num_color, false, 640);
            
            // Highlight current line background
            if is_cursor_line {
                for x in text_start_x..(menu_x + menu_width - 10) {
                    for dy in 0..16 {
                        if x < 640 && y_pos + dy < 480 {
                            let pixel_index = ((y_pos + dy) * 640 + x) * 4;
                            if pixel_index + 3 < frame.len() {
                                frame[pixel_index] = frame[pixel_index].saturating_add(15);     // R
                                frame[pixel_index + 1] = frame[pixel_index + 1].saturating_add(15); // G
                                frame[pixel_index + 2] = frame[pixel_index + 2].saturating_add(25); // B
                            }
                        }
                    }
                }
            }
            
            if is_cursor_line {
                // Draw line with cursor
                let (before_cursor, after_cursor) = if self.cursor_col <= line.len() {
                    line.split_at(self.cursor_col)
                } else {
                    (line.as_str(), "")
                };
                
                // Draw text before cursor
                self.draw_syntax_highlighted_text(frame, before_cursor, text_start_x, y_pos);
                
                // Calculate cursor position
                let cursor_x = text_start_x + before_cursor.len() * 8;
                
                // Draw cursor
                for dx in 0..2 {
                    for dy in 0..16 {
                        if cursor_x + dx < 640 && y_pos + dy < 480 {
                            let pixel_index = ((y_pos + dy) * 640 + cursor_x + dx) * 4;
                            if pixel_index + 3 < frame.len() {
                                frame[pixel_index] = 255;     // R
                                frame[pixel_index + 1] = 255; // G
                                frame[pixel_index + 2] = 255; // B
                                frame[pixel_index + 3] = 255; // A
                            }
                        }
                    }
                }
                
                // Draw text after cursor
                if !after_cursor.is_empty() {
                    let after_cursor_x = cursor_x + 3;
                    self.draw_syntax_highlighted_text(frame, after_cursor, after_cursor_x, y_pos);
                }
            } else {
                // Draw normal line with syntax highlighting
                self.draw_syntax_highlighted_text(frame, line, text_start_x, y_pos);
            }
        }

        // Only draw status info at the bottom
         
         // Status info
         let status_text = format!("Line: {} | Column: {} | Lines: {}", self.cursor_line + 1, self.cursor_col + 1, self.program_text.len());
         font::draw_text(frame, &status_text, menu_x + 10, menu_y + menu_height - 20, [180, 180, 180], false, 640);
    }
}

// Helper functions for drawing
fn draw_menu_background(frame: &mut [u8], x: usize, y: usize, width: usize, height: usize) {
    let window_width = 640;
    let window_height = 480;
    
    for py in y..y + height {
        for px in x..x + width {
            if px < window_width && py < window_height {
                let index = (py * window_width + px) * 4;
                if index + 3 < frame.len() {
                    frame[index] = 40;      // R
                    frame[index + 1] = 40;  // G
                    frame[index + 2] = 40;  // B
                    frame[index + 3] = 220; // A
                }
            }
        }
    }
}

fn draw_menu_border(frame: &mut [u8], x: usize, y: usize, width: usize, height: usize) {
    let window_width = 640;
    let window_height = 480;
    
    // Top and bottom borders
    for px in x..x + width {
        if px < window_width {
            // Top border
            if y < window_height {
                let index = (y * window_width + px) * 4;
                if index + 3 < frame.len() {
                    frame[index] = 100;     // R
                    frame[index + 1] = 100; // G
                    frame[index + 2] = 100; // B
                    frame[index + 3] = 255; // A
                }
            }
            // Bottom border
            if y + height - 1 < window_height {
                let index = ((y + height - 1) * window_width + px) * 4;
                if index + 3 < frame.len() {
                    frame[index] = 100;     // R
                    frame[index + 1] = 100; // G
                    frame[index + 2] = 100; // B
                    frame[index + 3] = 255; // A
                }
            }
        }
    }
    
    // Left and right borders
    for py in y..y + height {
        if py < window_height {
            // Left border
            if x < window_width {
                let index = (py * window_width + x) * 4;
                if index + 3 < frame.len() {
                    frame[index] = 100;     // R
                    frame[index + 1] = 100; // G
                    frame[index + 2] = 100; // B
                    frame[index + 3] = 255; // A
                }
            }
            // Right border
            if x + width - 1 < window_width {
                let index = (py * window_width + x + width - 1) * 4;
                if index + 3 < frame.len() {
                    frame[index] = 100;     // R
                    frame[index + 1] = 100; // G
                    frame[index + 2] = 100; // B
                    frame[index + 3] = 255; // A
                }
            }
        }
    }
}