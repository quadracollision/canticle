use winit::event::VirtualKeyCode;
use crate::square::{Cell, Program};
use crate::programmer::SimpleProgramParser;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SquareMenuState {
    None,
    SquareMenu { square_x: usize, square_y: usize, selected_option: usize },
    ProgramEditor { square_x: usize, square_y: usize, cursor_line: usize, cursor_col: usize },
    ProgramList { square_x: usize, square_y: usize, selected_program: usize },
}

pub struct SquareContextMenu {
    pub state: SquareMenuState,
    pub program_text: Vec<String>, // Lines of program text being edited
    pub parser: SimpleProgramParser,
}

const SQUARE_MENU_OPTIONS: &[&str] = &["Edit Program", "View Programs", "Test Program", "Clear Programs"];

impl SquareContextMenu {
    pub fn new() -> Self {
        SquareContextMenu {
            state: SquareMenuState::None,
            program_text: vec![
                "def my_program".to_string(),
                "".to_string(),
            ],
            parser: SimpleProgramParser::new(),
        }
    }

    pub fn open_square_menu(&mut self, square_x: usize, square_y: usize) {
        self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: 0 };
    }

    pub fn close(&mut self) {
        self.state = SquareMenuState::None;
    }

    pub fn is_open(&self) -> bool {
        !matches!(self.state, SquareMenuState::None)
    }

    pub fn handle_input(&mut self, input: &winit_input_helper::WinitInputHelper) -> Option<SquareMenuAction> {
        match self.state {
            SquareMenuState::SquareMenu { square_x, square_y, selected_option } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    self.close();
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Up) {
                    let new_option = if selected_option == 0 { SQUARE_MENU_OPTIONS.len() - 1 } else { selected_option - 1 };
                    self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Down) {
                    let new_option = (selected_option + 1) % SQUARE_MENU_OPTIONS.len();
                    self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Space) {
                    match selected_option {
                        0 => {
                            // Edit Program
                            self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: 0, cursor_col: 0 };
                        },
                        1 => {
                            // View Programs
                            self.state = SquareMenuState::ProgramList { square_x, square_y, selected_program: 0 };
                        },
                        2 => {
                            // Test Program
                            return Some(SquareMenuAction::TestProgram { square_x, square_y });
                        },
                        3 => {
                            // Clear Programs
                            return Some(SquareMenuAction::ClearPrograms { square_x, square_y });
                        },
                        _ => {}
                    }
                    return None;
                }
                None
            }
            SquareMenuState::ProgramEditor { square_x, square_y, cursor_line, cursor_col } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    // Try to compile and save the program
                    let program_source = self.program_text.join("\n");
                    match self.parser.parse_program(&program_source) {
                        Ok(program) => {
                            self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: 0 };
                            return Some(SquareMenuAction::SaveProgram { square_x, square_y, program });
                        }
                        Err(error) => {
                            // Show error (for now, just go back to menu)
                            println!("Parse error: {}", error);
                            self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: 0 };
                        }
                    }
                    return None;
                }
                
                // Cursor movement
                if input.key_pressed(VirtualKeyCode::Up) {
                    if cursor_line > 0 {
                        let new_line = cursor_line - 1;
                        let new_col = cursor_col.min(self.program_text[new_line].len());
                        self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: new_line, cursor_col: new_col };
                    }
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Down) {
                    if cursor_line < self.program_text.len().saturating_sub(1) {
                        let new_line = cursor_line + 1;
                        let new_col = cursor_col.min(self.program_text[new_line].len());
                        self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: new_line, cursor_col: new_col };
                    }
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Left) {
                    if cursor_col > 0 {
                        self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line, cursor_col: cursor_col - 1 };
                    } else if cursor_line > 0 {
                        let new_line = cursor_line - 1;
                        let new_col = self.program_text[new_line].len();
                        self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: new_line, cursor_col: new_col };
                    }
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Right) {
                    if cursor_col < self.program_text[cursor_line].len() {
                        self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line, cursor_col: cursor_col + 1 };
                    } else if cursor_line < self.program_text.len() - 1 {
                        self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: cursor_line + 1, cursor_col: 0 };
                    }
                    return None;
                }
                
                // Text editing
                if input.key_pressed(VirtualKeyCode::Return) {
                    // Split current line at cursor position
                    let current_line = self.program_text[cursor_line].clone();
                    let (left, right) = current_line.split_at(cursor_col);
                    self.program_text[cursor_line] = left.to_string();
                    self.program_text.insert(cursor_line + 1, right.to_string());
                    self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: cursor_line + 1, cursor_col: 0 };
                    return None;
                }
                
                if input.key_pressed(VirtualKeyCode::Back) {
                    if cursor_col > 0 {
                        // Remove character before cursor
                        self.program_text[cursor_line].remove(cursor_col - 1);
                        self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line, cursor_col: cursor_col - 1 };
                    } else if cursor_line > 0 {
                        // Join with previous line
                        let current_line = self.program_text.remove(cursor_line);
                        let new_col = self.program_text[cursor_line - 1].len();
                        self.program_text[cursor_line - 1].push_str(&current_line);
                        self.state = SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: cursor_line - 1, cursor_col: new_col };
                    }
                    return None;
                }
                
                if input.key_pressed(VirtualKeyCode::Delete) {
                    if cursor_col < self.program_text[cursor_line].len() {
                        // Remove character at cursor
                        self.program_text[cursor_line].remove(cursor_col);
                    } else if cursor_line < self.program_text.len() - 1 {
                        // Join with next line
                        let next_line = self.program_text.remove(cursor_line + 1);
                        self.program_text[cursor_line].push_str(&next_line);
                    }
                    return None;
                }
                
                // Character input - handle basic alphanumeric and common symbols
                self.handle_character_input(input, square_x, square_y, cursor_line, cursor_col);
                None
            }
            SquareMenuState::ProgramList { square_x, square_y, selected_program: _ } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: 0 };
                    return None;
                }
                // Handle program list navigation here
                None
            }
            SquareMenuState::None => None,
        }
    }

    fn handle_character_input(&mut self, input: &winit_input_helper::WinitInputHelper, square_x: usize, square_y: usize, cursor_line: usize, cursor_col: usize) {
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
                self.insert_character(ch, square_x, square_y, cursor_line, cursor_col);
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
                self.insert_character(ch, square_x, square_y, cursor_line, cursor_col);
                return;
            }
        }
        
        // Handle special characters
        if input.key_pressed(VirtualKeyCode::Space) {
            self.insert_character(' ', square_x, square_y, cursor_line, cursor_col);
        } else if input.key_pressed(VirtualKeyCode::Minus) {
            let ch = if shift_pressed { '_' } else { '-' };
            self.insert_character(ch, square_x, square_y, cursor_line, cursor_col);
        } else if input.key_pressed(VirtualKeyCode::Equals) {
            let ch = if shift_pressed { '+' } else { '=' };
            self.insert_character(ch, square_x, square_y, cursor_line, cursor_col);
        } else if input.key_pressed(VirtualKeyCode::Period) {
            let ch = if shift_pressed { '>' } else { '.' };
            self.insert_character(ch, square_x, square_y, cursor_line, cursor_col);
        } else if input.key_pressed(VirtualKeyCode::Comma) {
            let ch = if shift_pressed { '<' } else { ',' };
            self.insert_character(ch, square_x, square_y, cursor_line, cursor_col);
        }
    }
    
    fn insert_character(&mut self, ch: char, square_x: usize, square_y: usize, cursor_line: usize, cursor_col: usize) {
        self.program_text[cursor_line].insert(cursor_col, ch);
        self.state = SquareMenuState::ProgramEditor { 
            square_x, 
            square_y, 
            cursor_line, 
            cursor_col: cursor_col + 1 
        };
    }

    pub fn render(&self, frame: &mut [u8], cells: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) {
        match self.state {
            SquareMenuState::SquareMenu { square_x, square_y, selected_option } => {
                self.draw_square_menu(frame, square_x, square_y, selected_option);
            }
            SquareMenuState::ProgramEditor { square_x, square_y, cursor_line, cursor_col } => {
                self.draw_program_editor(frame, square_x, square_y, cursor_line, cursor_col);
            }
            SquareMenuState::ProgramList { square_x, square_y, selected_program } => {
                self.draw_program_list(frame, square_x, square_y, selected_program, cells);
            }
            SquareMenuState::None => {}
        }
    }

    fn draw_square_menu(&self, frame: &mut [u8], square_x: usize, square_y: usize, selected_option: usize) {
        let menu_x = (square_x * 40 + 50).min(600);
        let menu_y = (square_y * 40 + 50).min(400);
        let menu_width = 200;
        let menu_height = SQUARE_MENU_OPTIONS.len() * 20 + 20;

        // Draw menu background
        draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
        draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);

        // Draw title
        draw_text(frame, "Square Programming", menu_x + 10, menu_y + 5, [255, 255, 255], false);

        // Draw menu options
        for (i, option) in SQUARE_MENU_OPTIONS.iter().enumerate() {
            let y_pos = menu_y + 25 + i * 20;
            let selected = i == selected_option;
            draw_text(frame, option, menu_x + 10, y_pos, [255, 255, 255], selected);
        }
    }

    fn draw_program_editor(&self, frame: &mut [u8], square_x: usize, square_y: usize, cursor_line: usize, cursor_col: usize) {
        let menu_x = 50;
        let menu_y = 50;
        let menu_width = 500;
        let menu_height = 400;

        // Draw editor background
        draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
        draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);

        // Draw title
        draw_text(frame, &format!("Programming Square ({}, {})", square_x, square_y), menu_x + 10, menu_y + 5, [255, 255, 255], false);
        draw_text(frame, "Type to program, ESC to save", menu_x + 10, menu_y + 25, [200, 200, 200], false);

        // Draw program text with cursor
        for (i, line) in self.program_text.iter().enumerate() {
            let y_pos = menu_y + 50 + i * 20;
            let is_cursor_line = i == cursor_line;
            
            if is_cursor_line {
                // Draw line with cursor
                let (before_cursor, after_cursor) = if cursor_col <= line.len() {
                    line.split_at(cursor_col)
                } else {
                    (line.as_str(), "")
                };
                
                // Draw text before cursor
                draw_text(frame, before_cursor, menu_x + 10, y_pos, [255, 255, 255], false);
                
                // Calculate cursor position
                let cursor_x = menu_x + 10 + before_cursor.len() * 8; // Approximate character width
                
                // Draw cursor (vertical line)
                for dy in 0..16 {
                    if cursor_x < 640 && y_pos + dy < 480 {
                        let pixel_index = ((y_pos + dy) * 640 + cursor_x) * 4;
                        if pixel_index + 3 < frame.len() {
                            frame[pixel_index] = 255;     // R
                            frame[pixel_index + 1] = 255; // G
                            frame[pixel_index + 2] = 255; // B
                            frame[pixel_index + 3] = 255; // A
                        }
                    }
                }
                
                // Draw text after cursor
                if !after_cursor.is_empty() {
                    let after_cursor_x = cursor_x + 2;
                    draw_text(frame, after_cursor, after_cursor_x, y_pos, [255, 255, 255], false);
                }
            } else {
                // Draw normal line
                draw_text(frame, line, menu_x + 10, y_pos, [255, 255, 255], false);
            }
        }

        // Draw help text
        let help_y = menu_y + 50 + self.program_text.len() * 20 + 30;
        draw_text(frame, "Programming Language:", menu_x + 10, help_y, [150, 150, 150], false);
        draw_text(frame, "def function_name", menu_x + 10, help_y + 20, [150, 150, 150], false);
        draw_text(frame, "if c_red hits self N times", menu_x + 10, help_y + 40, [150, 150, 150], false);
        draw_text(frame, "set speed/direction value", menu_x + 10, help_y + 60, [150, 150, 150], false);
        draw_text(frame, "return", menu_x + 10, help_y + 80, [150, 150, 150], false);
    }

    fn draw_program_list(&self, frame: &mut [u8], square_x: usize, square_y: usize, selected_program: usize, cells: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) {
        let menu_x = 100;
        let menu_y = 100;
        let menu_width = 400;
        let menu_height = 200;

        // Draw list background
        draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
        draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);

        // Draw title
        draw_text(frame, &format!("Programs for Square ({}, {})", square_x, square_y), menu_x + 10, menu_y + 5, [255, 255, 255], false);

        // Get the square's programs
        if square_x < crate::sequencer::GRID_WIDTH && square_y < crate::sequencer::GRID_HEIGHT {
            let cell = &cells[square_y][square_x];
            let programs = &cell.program.programs;

            if programs.is_empty() {
                draw_text(frame, "No programs defined", menu_x + 10, menu_y + 30, [200, 200, 200], false);
            } else {
                for (i, program) in programs.iter().enumerate() {
                    let y_pos = menu_y + 30 + i * 20;
                    let selected = i == selected_program;
                    draw_text(frame, &program.name, menu_x + 10, y_pos, [255, 255, 255], selected);
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum SquareMenuAction {
    SaveProgram { square_x: usize, square_y: usize, program: Program },
    TestProgram { square_x: usize, square_y: usize },
    ClearPrograms { square_x: usize, square_y: usize },
}

// Helper functions for drawing (similar to context_menu.rs)
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
    let border_color = [100, 100, 100];
    
    // Top and bottom borders
    for px in x..x + width {
        if px < window_width {
            // Top border
            if y < window_height {
                let index = (y * window_width + px) * 4;
                if index + 2 < frame.len() {
                    frame[index] = border_color[0];
                    frame[index + 1] = border_color[1];
                    frame[index + 2] = border_color[2];
                }
            }
            // Bottom border
            let bottom_y = y + height - 1;
            if bottom_y < window_height {
                let index = (bottom_y * window_width + px) * 4;
                if index + 2 < frame.len() {
                    frame[index] = border_color[0];
                    frame[index + 1] = border_color[1];
                    frame[index + 2] = border_color[2];
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
                if index + 2 < frame.len() {
                    frame[index] = border_color[0];
                    frame[index + 1] = border_color[1];
                    frame[index + 2] = border_color[2];
                }
            }
            // Right border
            let right_x = x + width - 1;
            if right_x < window_width {
                let index = (py * window_width + right_x) * 4;
                if index + 2 < frame.len() {
                    frame[index] = border_color[0];
                    frame[index + 1] = border_color[1];
                    frame[index + 2] = border_color[2];
                }
            }
        }
    }
}

fn draw_text(frame: &mut [u8], text: &str, x: usize, y: usize, color: [u8; 3], selected: bool) {
    let bg_color = if selected { [80, 80, 120] } else { [0, 0, 0] };
    
    // Draw background if selected
    if selected {
        let text_width = text.len() * 8;
        let text_height = 12;
        for py in y..y + text_height {
            for px in x..x + text_width {
                if px < 640 && py < 480 {
                    let index = (py * 640 + px) * 4;
                    if index + 3 < frame.len() {
                        frame[index] = bg_color[0];
                        frame[index + 1] = bg_color[1];
                        frame[index + 2] = bg_color[2];
                    }
                }
            }
        }
    }
    
    // Draw text characters
    for (i, ch) in text.chars().enumerate() {
        draw_simple_char(frame, ch, x + i * 8, y, color);
    }
}

fn draw_simple_char(frame: &mut [u8], ch: char, x: usize, y: usize, color: [u8; 3]) {
    let window_width = 640;
    let window_height = 480;
    
    // Simple 8x12 bitmap font patterns
    let pattern = match ch {
        'A' | 'a' => [
            0b01110000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b11111000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'B' | 'b' => [
            0b11110000,
            0b10001000,
            0b10001000,
            0b11110000,
            0b11110000,
            0b10001000,
            0b10001000,
            0b11110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'C' | 'c' => [
            0b01110000,
            0b10001000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b10001000,
            0b01110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'D' | 'd' => [
            0b11110000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b11110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'E' | 'e' => [
            0b11111000,
            0b10000000,
            0b10000000,
            0b11110000,
            0b11110000,
            0b10000000,
            0b10000000,
            0b11111000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'F' | 'f' => [
            0b11111000,
            0b10000000,
            0b10000000,
            0b11110000,
            0b11110000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'G' | 'g' => [
            0b01110000,
            0b10001000,
            0b10000000,
            0b10000000,
            0b10111000,
            0b10001000,
            0b10001000,
            0b01110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'H' | 'h' => [
            0b10001000,
            0b10001000,
            0b10001000,
            0b11111000,
            0b11111000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'I' | 'i' => [
            0b01110000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b01110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'L' | 'l' => [
            0b10000000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b11111000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'M' | 'm' => [
            0b10001000,
            0b11011000,
            0b10101000,
            0b10101000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'N' | 'n' => [
            0b10001000,
            0b11001000,
            0b10101000,
            0b10101000,
            0b10011000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'O' | 'o' => [
            0b01110000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b01110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'P' | 'p' => [
            0b11110000,
            0b10001000,
            0b10001000,
            0b11110000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b10000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'R' | 'r' => [
            0b11110000,
            0b10001000,
            0b10001000,
            0b11110000,
            0b10100000,
            0b10010000,
            0b10001000,
            0b10001000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'S' | 's' => [
            0b01111000,
            0b10000000,
            0b10000000,
            0b01110000,
            0b00001000,
            0b00001000,
            0b00001000,
            0b11110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'T' | 't' => [
            0b11111000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'U' | 'u' => [
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b01110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'V' | 'v' => [
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b01010000,
            0b01010000,
            0b00100000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        'W' | 'w' => [
            0b10001000,
            0b10001000,
            0b10001000,
            0b10001000,
            0b10101000,
            0b10101000,
            0b11011000,
            0b10001000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        '0' => [
            0b01110000,
            0b10001000,
            0b10011000,
            0b10101000,
            0b11001000,
            0b10001000,
            0b10001000,
            0b01110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        '1' => [
            0b00100000,
            0b01100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b00100000,
            0b01110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        '2' => [
            0b01110000,
            0b10001000,
            0b00001000,
            0b00010000,
            0b00100000,
            0b01000000,
            0b10000000,
            0b11111000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        '3' => [
            0b01110000,
            0b10001000,
            0b00001000,
            0b00110000,
            0b00001000,
            0b00001000,
            0b10001000,
            0b01110000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        ' ' => [0; 12],
        ':' => [
            0b00000000,
            0b00000000,
            0b01100000,
            0b01100000,
            0b00000000,
            0b01100000,
            0b01100000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        '(' => [
            0b00010000,
            0b00100000,
            0b01000000,
            0b01000000,
            0b01000000,
            0b01000000,
            0b00100000,
            0b00010000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        ')' => [
            0b01000000,
            0b00100000,
            0b00010000,
            0b00010000,
            0b00010000,
            0b00010000,
            0b00100000,
            0b01000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        ',' => [
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b01100000,
            0b01100000,
            0b00100000,
            0b01000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        '-' => [
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b11111000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        '+' => [
            0b00000000,
            0b00000000,
            0b00100000,
            0b00100000,
            0b11111000,
            0b00100000,
            0b00100000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        '.' => [
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b01100000,
            0b01100000,
            0b00000000,
            0b00000000,
            0b00000000,
            0b00000000,
        ],
        _ => [0; 12], // Default to empty for unknown characters
    };
    
    for (row, &byte) in pattern.iter().enumerate() {
        for col in 0..8 {
            if byte & (0x80 >> col) != 0 {
                let px = x + col;
                let py = y + row;
                if px < window_width && py < window_height {
                    let index = (py * window_width + px) * 4;
                    if index + 3 < frame.len() {
                        frame[index] = color[0];     // R
                        frame[index + 1] = color[1]; // G
                        frame[index + 2] = color[2]; // B
                        frame[index + 3] = 255;      // A
                    }
                }
            }
        }
    }
}