use winit::event::VirtualKeyCode;
use crate::square::{Cell, Program};
use crate::program_editor::{ProgramEditor, ProgramEditorAction};
use std::time::{Duration, Instant};
use crate::font;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SquareMenuState {
    None,
    SquareMenu { square_x: usize, square_y: usize, selected_option: usize },
    ProgramEditor { square_x: usize, square_y: usize, cursor_line: usize, cursor_col: usize },
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

const SQUARE_MENU_OPTIONS: &[&str] = &["Edit Program", "Clear Programs"];

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
        font::draw_text(frame, "Square Programming", menu_x + 10, menu_y + 5, [255, 255, 255], false, 640);

        // Draw menu options
        for (i, option) in SQUARE_MENU_OPTIONS.iter().enumerate() {
            let y_pos = menu_y + 25 + i * 20;
            let selected = i == selected_option;
            font::draw_text(frame, option, menu_x + 10, y_pos, [255, 255, 255], selected, 640);
        }
    }




}

#[derive(Debug)]
pub enum SquareMenuAction {
    SaveProgram { square_x: usize, square_y: usize, program: Program, program_index: Option<usize> },
    SaveMultiplePrograms { square_x: usize, square_y: usize, programs: Vec<Program>, program_index: Option<usize> },
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