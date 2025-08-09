use winit::event::VirtualKeyCode;
use crate::square::{Cell, Program};
use crate::program_editor::{ProgramEditor, ProgramEditorAction};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SquareMenuState {
    None,
    SquareMenu { square_x: usize, square_y: usize, selected_option: usize },
    ProgramEditor { square_x: usize, square_y: usize, cursor_line: usize, cursor_col: usize },
    ProgramList { square_x: usize, square_y: usize, selected_program: usize },
}

pub struct SquareContextMenu {
    pub state: SquareMenuState,
    pub program_editor: ProgramEditor,
    pub editing_program_index: Option<usize>, // Track which program is being edited
    // Key repeat timing
    last_key_repeat: Option<Instant>,
    key_repeat_delay: Duration,
    key_repeat_rate: Duration,
}

const SQUARE_MENU_OPTIONS: &[&str] = &["Edit Program", "View Programs", "Test Program", "Clear Programs"];

impl SquareContextMenu {
    pub fn new() -> Self {
        SquareContextMenu {
            state: SquareMenuState::None,
            program_editor: ProgramEditor::new(),
            editing_program_index: None,
            last_key_repeat: None,
            key_repeat_delay: Duration::from_millis(500), // Initial delay before repeat
            key_repeat_rate: Duration::from_millis(50),   // Repeat rate
        }
    }

    pub fn open_square_menu(&mut self, square_x: usize, square_y: usize) {
        self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: 0 };
    }

    fn program_to_source_code(&self, program: &Program) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("def {}", program.name));
        
        for instruction in &program.instructions {
            self.instruction_to_source_lines(instruction, &mut lines);
        }
        
        lines.push("end".to_string());
        lines
    }

    fn instruction_to_source_lines(&self, instruction: &crate::square::Instruction, lines: &mut Vec<String>) {
        use crate::square::Instruction;
        
        match instruction {
            Instruction::SetSpeed(expr) => {
                if let Some(speed_line) = self.expression_to_set_speed(expr) {
                    lines.push(speed_line);
                }
            },
            Instruction::SetDirection(_) => {
                lines.push("set direction right".to_string()); // Simplified
            },
            Instruction::Bounce => {
                lines.push("bounce".to_string());
            },
            Instruction::Stop => {
                lines.push("stop".to_string());
            },
            Instruction::CreateBall { x, y, speed, direction: _ } => {
                let x_val = self.expression_to_number(x).unwrap_or(0.0);
                let y_val = self.expression_to_number(y).unwrap_or(0.0);
                let speed_val = self.expression_to_number(speed).unwrap_or(1.0);
                lines.push(format!("create ball({}, {}) speed {}", x_val as i32, y_val as i32, speed_val));
            },
            Instruction::CreateSquare { x, y } => {
                let x_val = self.expression_to_number(x).unwrap_or(0.0);
                let y_val = self.expression_to_number(y).unwrap_or(0.0);
                lines.push(format!("create square({}, {})", x_val as i32, y_val as i32));
            },
            Instruction::If { condition, then_block, else_block: _ } => {
                if let Some(condition_line) = self.condition_to_source(condition) {
                    lines.push(format!("if {}", condition_line));
                    for then_instruction in then_block {
                        self.instruction_to_source_lines(then_instruction, lines);
                    }
                }
            },
            _ => {
                // Fallback for unknown instructions
                lines.push("// Unknown instruction".to_string());
            }
        }
    }

    fn expression_to_set_speed(&self, expr: &crate::square::Expression) -> Option<String> {
        use crate::square::{Expression, Value};
        
        match expr {
            Expression::Literal(Value::Number(speed)) => {
                Some(format!("set speed {}", speed))
            },
            Expression::BinaryOp { left: _, op: _, right } => {
                if let Expression::Literal(Value::Number(change)) = right.as_ref() {
                    if *change >= 0.0 {
                        Some(format!("set speed +{}", change))
                    } else {
                        Some(format!("set speed {}", change))
                    }
                } else {
                    Some("set speed 1.0".to_string())
                }
            },
            _ => Some("set speed 1.0".to_string())
        }
    }

    fn expression_to_number(&self, expr: &crate::square::Expression) -> Option<f32> {
        use crate::square::{Expression, Value};
        
        match expr {
            Expression::Literal(Value::Number(n)) => Some(*n),
            _ => None
        }
    }

    fn condition_to_source(&self, _condition: &crate::square::Expression) -> Option<String> {
        // Simplified condition conversion
        Some("c_red hits self 1 times".to_string())
    }

    pub fn close(&mut self) {
        self.state = SquareMenuState::None;
    }

    pub fn is_open(&self) -> bool {
        !matches!(self.state, SquareMenuState::None)
    }

    pub fn handle_input(&mut self, input: &winit_input_helper::WinitInputHelper, cells: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) -> Option<SquareMenuAction> {
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
                            // Edit Program - Initialize with square's current program
                            if square_x < crate::sequencer::GRID_WIDTH && square_y < crate::sequencer::GRID_HEIGHT {
                                let cell = &cells[square_y][square_x];
                                
                                // Get the active program, or the first program if no active program is set
                                let active_index = cell.program.active_program.unwrap_or(0);
                                if let Some(program) = cell.program.get_program(active_index) {
                                    // Check if this is the default program (name "Default" with only bounce instruction)
                                    let is_default_program = program.name == "Default" && 
                                        program.instructions.len() == 1 &&
                                        matches!(program.instructions[0], crate::square::Instruction::Bounce);
                                    
                                    if is_default_program {
                                        // Default program, start with empty editor
                                        self.program_editor = ProgramEditor::new_empty();
                                        self.editing_program_index = Some(active_index); // Will replace default program
                                    } else {
                                        // Use preserved source text if available, otherwise convert from instructions
                                        let source_lines = if let Some(ref source_text) = program.source_text {
                                            source_text.clone()
                                        } else {
                                            self.program_to_source_code(program)
                                        };
                                        self.program_editor = ProgramEditor::new_with_text(source_lines);
                                        self.editing_program_index = Some(active_index); // Editing existing program at active index
                                    }
                                } else {
                                    // No existing program, start with empty editor
                                    self.program_editor = ProgramEditor::new_empty();
                                    self.editing_program_index = None; // Will add new program
                                }
                            } else {
                                self.program_editor = ProgramEditor::new_empty();
                                self.editing_program_index = None;
                            }
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
            SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: _, cursor_col: _ } => {
                match self.program_editor.handle_input(input) {
                    ProgramEditorAction::SaveProgram(program) => {
                        self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: 0 };
                        let program_index = self.editing_program_index;
                        self.editing_program_index = None; // Reset after saving
                        return Some(SquareMenuAction::SaveProgram { square_x, square_y, program, program_index });
                    }
                    ProgramEditorAction::SaveAndCompile => {
                        let programs = self.program_editor.get_all_programs();
                        self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: 0 };
                        let program_index = self.editing_program_index;
                        self.editing_program_index = None; // Reset after saving
                        return Some(SquareMenuAction::SaveMultiplePrograms { square_x, square_y, programs, program_index });
                    }
                    ProgramEditorAction::CloseWithoutSaving => {
                        self.state = SquareMenuState::SquareMenu { square_x, square_y, selected_option: 0 };
                    }
                    ProgramEditorAction::Continue => {
                        // Continue editing
                    }
                    ProgramEditorAction::None => {
                        // Do nothing
                    }
                }
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



    fn should_handle_key_repeat(&mut self, input: &winit_input_helper::WinitInputHelper, key: VirtualKeyCode) -> bool {
        let now = Instant::now();
        
        // Check if key is currently pressed
        if !input.key_held(key) {
            // Key not held, reset timing
            if input.key_pressed(key) {
                self.last_key_repeat = Some(now);
                return true; // Handle initial press
            }
            return false;
        }
        
        // Key is held, check timing
        match self.last_key_repeat {
            Some(last_time) => {
                let elapsed = now.duration_since(last_time);
                if elapsed >= self.key_repeat_rate {
                    self.last_key_repeat = Some(now);
                    return true;
                }
                false
            }
            None => {
                // First time holding, set initial delay
                self.last_key_repeat = Some(now);
                false
            }
        }
    }



    pub fn render(&self, frame: &mut [u8], cells: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) {
        match self.state {
            SquareMenuState::SquareMenu { square_x, square_y, selected_option } => {
                self.draw_square_menu(frame, square_x, square_y, selected_option);
            }
            SquareMenuState::ProgramEditor { square_x, square_y, cursor_line: _, cursor_col: _ } => {
                self.program_editor.draw_program_editor(frame, &format!("Programming Square ({}, {})", square_x, square_y), "Arrow keys: Navigate | Backspace/Delete: Edit | ESC: Save & Exit");
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
    SaveProgram { square_x: usize, square_y: usize, program: Program, program_index: Option<usize> },
    SaveMultiplePrograms { square_x: usize, square_y: usize, programs: Vec<Program>, program_index: Option<usize> },
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