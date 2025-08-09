use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;
use crate::square::{LibraryManager, Program, Cell};
use crate::program_editor::{ProgramEditor, ProgramEditorAction};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum ProgramSource {
    Library { library_name: String },
    Square { x: usize, y: usize, program_index: usize },
}

#[derive(Debug, Clone)]
pub struct ProgramEntry {
    pub name: String,
    pub program: Program,
    pub source: ProgramSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LibraryGuiState {
    Hidden,
    Visible {
        selected_column: LibraryColumn,
        selected_library: String,
        selected_item: usize,
        scroll_offset: usize,
        editing_mode: Option<EditingMode>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LibraryColumn {
    Samples,
    Programs,
}

#[derive(Debug, Clone)]
pub enum EditingMode {
    RenameItem { original_name: String, new_name: String },
    CreateProgram { name: String, editor: ProgramEditor },
    EditProgram { name: String, source: ProgramSource, editor: ProgramEditor },
}

// Manual PartialEq implementation since ProgramEditor doesn't derive PartialEq
impl PartialEq for EditingMode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (EditingMode::RenameItem { original_name: a1, new_name: b1 }, 
             EditingMode::RenameItem { original_name: a2, new_name: b2 }) => a1 == a2 && b1 == b2,
            (EditingMode::CreateProgram { name: n1, .. }, 
             EditingMode::CreateProgram { name: n2, .. }) => n1 == n2,
            (EditingMode::EditProgram { name: n1, source: s1, .. }, 
             EditingMode::EditProgram { name: n2, source: s2, .. }) => n1 == n2 && s1 == s2,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub enum LibraryGuiAction {
    RenameItem { library_name: String, old_name: String, new_name: String, is_sample: bool },
    DeleteItem { library_name: String, item_name: String, is_sample: bool },
    CreateProgram { library_name: String, name: String, program: Program },
    EditProgram { source: ProgramSource, name: String, program: Program },
    LoadSample { library_name: String },
}

const LIBRARY_GUI_WIDTH: usize = 400;
const LIBRARY_GUI_HEIGHT: usize = 480;
const COLUMN_WIDTH: usize = 190;
const ITEM_HEIGHT: usize = 20;
const HEADER_HEIGHT: usize = 30;
const MAX_VISIBLE_ITEMS: usize = 20;

pub struct LibraryGui {
    pub state: LibraryGuiState,
    last_key_repeat: Option<Instant>,
    key_repeat_delay: Duration,
    key_repeat_rate: Duration,
}

impl LibraryGui {
    pub fn new() -> Self {
        Self {
            state: LibraryGuiState::Hidden,
            last_key_repeat: None,
            key_repeat_delay: Duration::from_millis(500),
            key_repeat_rate: Duration::from_millis(100), // Slower repeat rate to prevent double deletion
        }
    }

    pub fn toggle(&mut self) {
        self.state = match self.state {
            LibraryGuiState::Hidden => LibraryGuiState::Visible {
                selected_column: LibraryColumn::Samples,
                selected_library: "lib".to_string(),
                selected_item: 0,
                scroll_offset: 0,
                editing_mode: None,
            },
            LibraryGuiState::Visible { .. } => LibraryGuiState::Hidden,
        };
    }

    pub fn is_visible(&self) -> bool {
        matches!(self.state, LibraryGuiState::Visible { .. })
    }

    pub fn handle_input(&mut self, input: &WinitInputHelper, library_manager: &LibraryManager, grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) -> Option<LibraryGuiAction> {
        // Extract state to avoid borrowing conflicts
        let (mut selected_column, mut selected_library, mut selected_item, mut scroll_offset, mut editing_mode) = 
            if let LibraryGuiState::Visible { 
                selected_column, 
                selected_library, 
                selected_item, 
                scroll_offset,
                editing_mode 
            } = &self.state {
                (selected_column.clone(), selected_library.clone(), *selected_item, *scroll_offset, editing_mode.clone())
            } else {
                return None;
            };
            
        // Handle editing mode input
        if let Some(ref mut edit_mode) = editing_mode {
            let result = self.handle_editing_input(input, edit_mode, &selected_library);
            // Update state
            self.state = LibraryGuiState::Visible {
                selected_column,
                selected_library,
                selected_item,
                scroll_offset,
                editing_mode,
            };
            return result;
        }

        // Navigation between columns
        if input.key_pressed(VirtualKeyCode::Tab) {
            selected_column = match selected_column {
                LibraryColumn::Samples => LibraryColumn::Programs,
                LibraryColumn::Programs => LibraryColumn::Samples,
            };
            selected_item = 0;
            scroll_offset = 0;
        }

        // Library switching with Left/Right arrows
        if input.key_pressed(VirtualKeyCode::Left) || input.key_pressed(VirtualKeyCode::Right) {
            let available_libraries: Vec<String> = library_manager.function_libraries.keys().cloned().collect();
            if !available_libraries.is_empty() {
                let current_index = available_libraries.iter().position(|lib| lib == &selected_library).unwrap_or(0);
                let new_index = if input.key_pressed(VirtualKeyCode::Left) {
                    if current_index == 0 { available_libraries.len() - 1 } else { current_index - 1 }
                } else {
                    (current_index + 1) % available_libraries.len()
                };
                selected_library = available_libraries[new_index].clone();
                selected_item = 0;
                scroll_offset = 0;
            }
        }

        // Navigation within column
        if input.key_pressed(VirtualKeyCode::Up) {
            if selected_item > 0 {
                selected_item -= 1;
                if selected_item < scroll_offset {
                    scroll_offset = selected_item;
                }
            }
        }

        if input.key_pressed(VirtualKeyCode::Down) {
            let max_items = self.get_item_count(library_manager, &selected_column, &selected_library, grid);
            if selected_item + 1 < max_items {
                selected_item += 1;
                if selected_item >= scroll_offset + MAX_VISIBLE_ITEMS {
                    scroll_offset = selected_item - MAX_VISIBLE_ITEMS + 1;
                }
            }
        }

        // Actions
        if input.key_pressed(VirtualKeyCode::F2) { // Rename
            if let Some(item_name) = self.get_selected_item_name(library_manager, &selected_column, &selected_library, selected_item, grid) {
                editing_mode = Some(EditingMode::RenameItem {
                    original_name: item_name.clone(),
                    new_name: item_name,
                });
            }
        }

        let mut result = None;
        if input.key_pressed(VirtualKeyCode::Delete) { // Delete
            if let Some(item_name) = self.get_selected_item_name(library_manager, &selected_column, &selected_library, selected_item, grid) {
                result = Some(LibraryGuiAction::DeleteItem {
                    library_name: selected_library.clone(),
                    item_name,
                    is_sample: matches!(selected_column, LibraryColumn::Samples),
                });
            }
        }

        if input.held_shift() && input.key_pressed(VirtualKeyCode::Space) { // Create new program
            if matches!(selected_column, LibraryColumn::Programs) {
                let initial_text = vec!["def new_program".to_string(), "".to_string()];
                editing_mode = Some(EditingMode::CreateProgram {
                    name: "new_program".to_string(),
                    editor: ProgramEditor::new_with_text(initial_text),
                });
            }
        }

        if input.key_pressed(VirtualKeyCode::Return) { // Edit program
            if matches!(selected_column, LibraryColumn::Programs) {
                let all_programs = self.collect_all_programs(library_manager, grid);
                 if let Some(program_entry) = all_programs.get(selected_item) {
                     let script = self.program_to_source_code(&program_entry.program);
                     editing_mode = Some(EditingMode::EditProgram {
                         name: program_entry.name.clone(),
                         source: program_entry.source.clone(),
                         editor: ProgramEditor::new_with_text(script),
                     });
                 }
            }
        }

        if input.key_pressed(VirtualKeyCode::Space) && !input.held_shift() {
            match selected_column {
                LibraryColumn::Programs => {
                    // Edit existing program
                    let all_programs = self.collect_all_programs(library_manager, grid);
                    if let Some(program_entry) = all_programs.get(selected_item) {
                        let script = self.program_to_source_code(&program_entry.program);
                        editing_mode = Some(EditingMode::EditProgram {
                            name: program_entry.name.clone(),
                            source: program_entry.source.clone(),
                            editor: ProgramEditor::new_with_text(script),
                        });
                    }
                }
                LibraryColumn::Samples => {
                    // Load sample
                    result = Some(LibraryGuiAction::LoadSample {
                        library_name: selected_library.clone(),
                    });
                }
            }
        }
        
        // Update state
        self.state = LibraryGuiState::Visible {
            selected_column,
            selected_library,
            selected_item,
            scroll_offset,
            editing_mode,
        };
        
        result
    }

    fn handle_editing_input(&mut self, input: &WinitInputHelper, edit_mode: &mut EditingMode, selected_library: &str) -> Option<LibraryGuiAction> {
        match edit_mode {
            EditingMode::RenameItem { original_name, new_name } => {
                // Handle text input for renaming
                if input.key_pressed(VirtualKeyCode::Return) {
                    let action = Some(LibraryGuiAction::RenameItem {
                        library_name: selected_library.to_string(),
                        old_name: original_name.clone(),
                        new_name: new_name.clone(),
                        is_sample: true, // This would need to be determined based on context
                    });
                    if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                        *editing_mode = None;
                    }
                    return action;
                }
                if input.key_pressed(VirtualKeyCode::Escape) {
                    if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                        *editing_mode = None;
                    }
                }
                // TODO: Handle character input for editing the name
            },
            EditingMode::CreateProgram { name, editor } => {
                match editor.handle_input(input) {
                    ProgramEditorAction::SaveAndCompile => {
                        let program = editor.get_program();
                        let action = Some(LibraryGuiAction::CreateProgram {
                            library_name: selected_library.to_string(),
                            name: program.name.clone(),
                            program,
                        });
                        if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                            *editing_mode = None;
                        }
                        return action;
                    },
                    ProgramEditorAction::CloseWithoutSaving => {
                        if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                            *editing_mode = None;
                        }
                        return None;
                    },
                    ProgramEditorAction::Continue => {
                        // Continue editing
                    },
                    ProgramEditorAction::SaveProgram(program) => {
                        let action = Some(LibraryGuiAction::CreateProgram {
                            library_name: selected_library.to_string(),
                            name: program.name.clone(),
                            program,
                        });
                        if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                            *editing_mode = None;
                        }
                        return action;
                    },
                    ProgramEditorAction::None => {
                        // Do nothing
                    }
                }
            },
            EditingMode::EditProgram { name, source, editor } => {
                match editor.handle_input(input) {
                    ProgramEditorAction::SaveAndCompile => {
                        let program = editor.get_program();
                        let action = Some(LibraryGuiAction::EditProgram {
                            source: source.clone(),
                            name: name.clone(),
                            program,
                        });
                        if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                            *editing_mode = None;
                        }
                        return action;
                    },
                    ProgramEditorAction::CloseWithoutSaving => {
                        if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                            *editing_mode = None;
                        }
                        return None;
                    },
                    ProgramEditorAction::Continue => {
                        // Continue editing
                    },
                    ProgramEditorAction::SaveProgram(program) => {
                        let action = Some(LibraryGuiAction::EditProgram {
                            source: source.clone(),
                            name: name.clone(),
                            program,
                        });
                        if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                            *editing_mode = None;
                        }
                        return action;
                    },
                    ProgramEditorAction::None => {
                        // Do nothing
                    }
                }
            },
        }
        None
    }
    

    
    fn should_handle_key_repeat(&mut self, input: &WinitInputHelper, key: VirtualKeyCode) -> bool {
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

    fn get_item_count(&self, library_manager: &LibraryManager, column: &LibraryColumn, library_name: &str, grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) -> usize {
        match column {
            LibraryColumn::Samples => {
                library_manager.sample_libraries
                    .get(library_name)
                    .map(|lib| lib.samples.len())
                    .unwrap_or(0)
            },
            LibraryColumn::Programs => {
                self.collect_all_programs(library_manager, grid).len()
            },
        }
    }

    fn get_selected_item_name(&self, library_manager: &LibraryManager, column: &LibraryColumn, library_name: &str, index: usize, grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) -> Option<String> {
        match column {
            LibraryColumn::Samples => {
                library_manager.sample_libraries
                    .get(library_name)?
                    .samples
                    .keys()
                    .nth(index)
                    .cloned()
            },
            LibraryColumn::Programs => {
                let all_programs = self.collect_all_programs(library_manager, grid);
                all_programs.get(index).map(|entry| {
                    match &entry.source {
                        ProgramSource::Library { library_name } => {
                            format!("{} ({})", entry.name, library_name)
                        },
                        ProgramSource::Square { x, y, program_index: _ } => {
                            format!("{} @({},{})", entry.name, x, y)
                        }
                    }
                })
            },
        }
    }

    fn collect_all_programs(&self, library_manager: &LibraryManager, grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) -> Vec<ProgramEntry> {
        let mut all_programs = Vec::new();
        
        // Include all libraries, including "lib" for user-created programs
        for (lib_name, lib) in &library_manager.function_libraries {
            for (prog_name, program) in &lib.functions {
                // Skip only the default predefined functions, not user-created ones
                if lib_name == "lib" && self.is_predefined_function(prog_name) {
                    continue;
                }
                all_programs.push(ProgramEntry {
                    name: prog_name.clone(),
                    program: program.clone(),
                    source: ProgramSource::Library { library_name: lib_name.clone() },
                });
            }
        }
        
        // Collect programs from squares, but skip the default "Default" program
        for (y, row) in grid.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if cell.is_square() {
                    for (prog_index, program) in cell.program.programs.iter().enumerate() {
                        // Skip the default "Default" program that contains only a bounce instruction
                        if program.name == "Default" && program.instructions.len() == 1 {
                            if let crate::square::Instruction::Bounce = program.instructions[0] {
                                continue;
                            }
                        }
                        all_programs.push(ProgramEntry {
                            name: program.name.clone(),
                            program: program.clone(),
                            source: ProgramSource::Square { x, y, program_index: prog_index },
                        });
                    }
                }
            }
        }
        
        all_programs
    }

    fn get_program<'a>(&self, library_manager: &'a LibraryManager, library_name: &str, program_name: &str) -> Option<&'a Program> {
        library_manager.function_libraries
            .get(library_name)?
            .functions
            .get(program_name)
    }

    fn is_predefined_function(&self, program_name: &str) -> bool {
        // List of predefined function names that should be hidden from the UI
        matches!(program_name, "ballcreator" | "bounce" | "speed_boost" | "direction_cycle" | "multi_creator")
    }

    fn program_to_source_code(&self, program: &Program) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("def {}", program.name));
        
        for instruction in &program.instructions {
            self.instruction_to_source_lines(instruction, &mut lines, 0);
        }
        
        lines.push("end".to_string());
        lines
    }

    fn instruction_to_source_lines(&self, instruction: &crate::square::Instruction, lines: &mut Vec<String>, _indent: usize) {
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
            Instruction::DestroySquare { target } => {
                match target {
                    crate::square::DestroyTarget::BallReference(ball_ref) => {
                        lines.push(format!("destroy square({})", ball_ref));
                    },
                    crate::square::DestroyTarget::Coordinates { x, y } => {
                        let x_val = self.expression_to_number(x).unwrap_or(0.0);
                        let y_val = self.expression_to_number(y).unwrap_or(0.0);
                        lines.push(format!("destroy square({}, {})", x_val as i32, y_val as i32));
                    }
                }
            },
            Instruction::If { condition, then_block, else_block: _ } => {
                if let Some(condition_line) = self.condition_to_source(condition) {
                    lines.push(format!("if {}", condition_line));
                    for then_instruction in then_block {
                        self.instruction_to_source_lines(then_instruction, lines, 0);
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

    fn condition_to_source(&self, condition: &crate::square::Expression) -> Option<String> {
        use crate::square::{Expression, BinaryOperator, Value};
        
        match condition {
            Expression::BinaryOp { left, op, right } => {
                match op {
                    BinaryOperator::GreaterEqual => {
                        // This is likely a hit count condition
                        if let Expression::Literal(Value::Number(count)) = right.as_ref() {
                            Some(format!("c_white hits self {} times", *count as i32))
                        } else {
                            Some("c_white hits self 1 times".to_string())
                        }
                    },
                    _ => Some("c_white hits self 1 times".to_string())
                }
            },
            _ => Some("c_white hits self 1 times".to_string())
        }
    }

    pub fn render(&self, frame: &mut [u8], library_manager: &LibraryManager, grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT], window_width: usize, window_height: usize) {
        if let LibraryGuiState::Visible { 
            selected_column, 
            selected_library, 
            selected_item, 
            scroll_offset,
            editing_mode 
        } = &self.state {
            
            // Calculate position (center of screen)
            let gui_x = (window_width - LIBRARY_GUI_WIDTH) / 2;
            let gui_y = (window_height - LIBRARY_GUI_HEIGHT) / 2;

            // Draw background
            self.draw_background(frame, gui_x, gui_y, window_width);

            // Draw headers
            self.draw_headers(frame, gui_x, gui_y, selected_column, window_width);

            // Draw sample column
            self.draw_sample_column(frame, gui_x, gui_y, library_manager, selected_library, 
                                  selected_column, *selected_item, *scroll_offset, window_width);

            // Draw program column
            self.draw_program_column(frame, gui_x + COLUMN_WIDTH + 10, gui_y, library_manager, grid,
                                   selected_library, selected_column, *selected_item, *scroll_offset, window_width);

            // Draw editing overlay if in editing mode
            if let Some(edit_mode) = editing_mode {
                self.draw_editing_overlay(frame, gui_x, gui_y, edit_mode, window_width);
            }
        }
    }

    fn draw_background(&self, frame: &mut [u8], x: usize, y: usize, window_width: usize) {
        for dy in 0..LIBRARY_GUI_HEIGHT {
            for dx in 0..LIBRARY_GUI_WIDTH {
                let px = x + dx;
                let py = y + dy;
                if px < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + px) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = 50;     // R
                        frame[idx + 1] = 50; // G
                        frame[idx + 2] = 50; // B
                        frame[idx + 3] = 255; // A
                    }
                }
            }
        }

        // Draw border
        self.draw_border(frame, x, y, LIBRARY_GUI_WIDTH, LIBRARY_GUI_HEIGHT, window_width);
    }

    fn draw_border(&self, frame: &mut [u8], x: usize, y: usize, width: usize, height: usize, window_width: usize) {
        let border_color = [100, 100, 100];
        
        // Top and bottom borders
        for dx in 0..width {
            let px = x + dx;
            if px < window_width {
                // Top border
                if y < frame.len() / (window_width * 4) {
                    let idx = (y * window_width + px) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = border_color[0];
                        frame[idx + 1] = border_color[1];
                        frame[idx + 2] = border_color[2];
                    }
                }
                // Bottom border
                let bottom_y = y + height - 1;
                if bottom_y < frame.len() / (window_width * 4) {
                    let idx = (bottom_y * window_width + px) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = border_color[0];
                        frame[idx + 1] = border_color[1];
                        frame[idx + 2] = border_color[2];
                    }
                }
            }
        }

        // Left and right borders
        for dy in 0..height {
            let py = y + dy;
            if py < frame.len() / (window_width * 4) {
                // Left border
                if x < window_width {
                    let idx = (py * window_width + x) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = border_color[0];
                        frame[idx + 1] = border_color[1];
                        frame[idx + 2] = border_color[2];
                    }
                }
                // Right border
                let right_x = x + width - 1;
                if right_x < window_width {
                    let idx = (py * window_width + right_x) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = border_color[0];
                        frame[idx + 1] = border_color[1];
                        frame[idx + 2] = border_color[2];
                    }
                }
            }
        }
    }

    fn draw_headers(&self, frame: &mut [u8], x: usize, y: usize, selected_column: &LibraryColumn, window_width: usize) {
        // Sample header
        let sample_selected = matches!(selected_column, LibraryColumn::Samples);
        self.draw_text(frame, "SAMPLES", x + 10, y + 10, 
                      if sample_selected { [255, 255, 255] } else { [150, 150, 150] }, 
                      sample_selected, window_width);

        // Program header
        let program_selected = matches!(selected_column, LibraryColumn::Programs);
        self.draw_text(frame, "PROGRAMS", x + COLUMN_WIDTH + 20, y + 10, 
                      if program_selected { [255, 255, 255] } else { [150, 150, 150] }, 
                      program_selected, window_width);

        // Draw column separator
        let separator_x = x + COLUMN_WIDTH + 5;
        for dy in 0..LIBRARY_GUI_HEIGHT {
            let py = y + dy;
            if separator_x < window_width && py < frame.len() / (window_width * 4) {
                let idx = (py * window_width + separator_x) * 4;
                if idx + 3 < frame.len() {
                    frame[idx] = 80;
                    frame[idx + 1] = 80;
                    frame[idx + 2] = 80;
                }
            }
        }
    }

    fn draw_sample_column(&self, frame: &mut [u8], x: usize, y: usize, library_manager: &LibraryManager, 
                         selected_library: &str, selected_column: &LibraryColumn, 
                         selected_item: usize, scroll_offset: usize, window_width: usize) {
        if let Some(library) = library_manager.sample_libraries.get(selected_library) {
            let start_y = y + HEADER_HEIGHT;
            let is_column_selected = matches!(selected_column, LibraryColumn::Samples);
            
            for (i, (name, _sample)) in library.samples.iter().enumerate().skip(scroll_offset).take(MAX_VISIBLE_ITEMS) {
                let item_y = start_y + i * ITEM_HEIGHT;
                let is_selected = is_column_selected && (i + scroll_offset) == selected_item;
                
                self.draw_text(frame, name, x + 10, item_y, 
                              if is_selected { [255, 255, 0] } else { [200, 200, 200] }, 
                              is_selected, window_width);
            }
        }
    }

    fn draw_program_column(&self, frame: &mut [u8], x: usize, y: usize, library_manager: &LibraryManager, 
                          grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT],
                          selected_library: &str, selected_column: &LibraryColumn, 
                          selected_item: usize, scroll_offset: usize, window_width: usize) {
        let start_y = y + HEADER_HEIGHT;
        let is_column_selected = matches!(selected_column, LibraryColumn::Programs);
        
        let all_programs = self.collect_all_programs(library_manager, grid);
        for (i, entry) in all_programs.iter().enumerate().skip(scroll_offset).take(MAX_VISIBLE_ITEMS) {
            let item_y = start_y + i * ITEM_HEIGHT;
            let is_selected = is_column_selected && (i + scroll_offset) == selected_item;
            
            let display_text = entry.name.clone();
            
            self.draw_text(frame, &display_text, x + 10, item_y, 
                          if is_selected { [255, 255, 0] } else { [200, 200, 200] }, 
                          is_selected, window_width);
        }
    }

    fn draw_editing_overlay(&self, frame: &mut [u8], x: usize, y: usize, edit_mode: &EditingMode, window_width: usize) {
        match edit_mode {
            EditingMode::RenameItem { original_name: _, new_name } => {
                let overlay_width = 300;
                let overlay_height = 200;
                let overlay_x = x + (LIBRARY_GUI_WIDTH - overlay_width) / 2;
                let overlay_y = y + (LIBRARY_GUI_HEIGHT - overlay_height) / 2;

                // Draw overlay background
                for dy in 0..overlay_height {
                    for dx in 0..overlay_width {
                        let px = overlay_x + dx;
                        let py = overlay_y + dy;
                        if px < window_width && py < frame.len() / (window_width * 4) {
                            let idx = (py * window_width + px) * 4;
                            if idx + 3 < frame.len() {
                                frame[idx] = 70;     // R
                                frame[idx + 1] = 70; // G
                                frame[idx + 2] = 70; // B
                                frame[idx + 3] = 255; // A
                            }
                        }
                    }
                }

                self.draw_border(frame, overlay_x, overlay_y, overlay_width, overlay_height, window_width);
                self.draw_text(frame, "Rename Item:", overlay_x + 10, overlay_y + 10, [255, 255, 255], false, window_width);
                self.draw_text(frame, new_name, overlay_x + 10, overlay_y + 40, [255, 255, 0], true, window_width);
                self.draw_text(frame, "Press Enter to confirm, Esc to cancel", overlay_x + 10, overlay_y + 70, [150, 150, 150], false, window_width);
            },
            EditingMode::CreateProgram { name, editor } => {
                editor.draw_program_editor(frame, "Create Program", "Arrow keys: Navigate | Backspace/Delete: Edit | ESC: Save & Exit");
            },
            EditingMode::EditProgram { name, source: _, editor } => {
                editor.draw_program_editor(frame, &format!("Edit Program: {}", name), "Arrow keys: Navigate | Backspace/Delete: Edit | ESC: Save & Exit");
            },
        }
    }



    fn draw_text(&self, frame: &mut [u8], text: &str, x: usize, y: usize, color: [u8; 3], selected: bool, window_width: usize) {
        let bg_color = if selected { [80, 80, 120] } else { [0, 0, 0] };
        
        // Draw background if selected
        if selected {
            let text_width = text.len() * 8;
            let text_height = 12;
            for py in y..y + text_height {
                for px in x..x + text_width {
                    if px < window_width && py < frame.len() / (window_width * 4) {
                        let index = (py * window_width + px) * 4;
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
            self.draw_char(frame, ch, x + i * 8, y, color, window_width);
        }
    }

    fn draw_char(&self, frame: &mut [u8], ch: char, x: usize, y: usize, color: [u8; 3], window_width: usize) {
        // Simple 8x12 bitmap font (reusing patterns from existing code)
        let pattern = match ch {
            'A' | 'a' => [
                0b01110000, 0b10001000, 0b10001000, 0b10001000,
                0b11111000, 0b10001000, 0b10001000, 0b10001000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'B' | 'b' => [
                0b11110000, 0b10001000, 0b10001000, 0b11110000,
                0b11110000, 0b10001000, 0b10001000, 0b11110000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'C' | 'c' => [
                0b01110000, 0b10001000, 0b10000000, 0b10000000,
                0b10000000, 0b10000000, 0b10001000, 0b01110000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'D' | 'd' => [
                0b11110000, 0b10001000, 0b10001000, 0b10001000,
                0b10001000, 0b10001000, 0b10001000, 0b11110000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'E' | 'e' => [
                0b11111000, 0b10000000, 0b10000000, 0b11110000,
                0b11110000, 0b10000000, 0b10000000, 0b11111000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'F' | 'f' => [
                0b11111000, 0b10000000, 0b10000000, 0b11110000,
                0b11110000, 0b10000000, 0b10000000, 0b10000000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'G' | 'g' => [
                0b01110000, 0b10001000, 0b10000000, 0b10000000,
                0b10111000, 0b10001000, 0b10001000, 0b01110000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'H' | 'h' => [
                0b10001000, 0b10001000, 0b10001000, 0b11111000,
                0b11111000, 0b10001000, 0b10001000, 0b10001000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'I' | 'i' => [
                0b01110000, 0b00100000, 0b00100000, 0b00100000,
                0b00100000, 0b00100000, 0b00100000, 0b01110000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'L' | 'l' => [
                0b10000000, 0b10000000, 0b10000000, 0b10000000,
                0b10000000, 0b10000000, 0b10000000, 0b11111000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'M' | 'm' => [
                0b10001000, 0b11011000, 0b10101000, 0b10101000,
                0b10001000, 0b10001000, 0b10001000, 0b10001000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'N' | 'n' => [
                0b10001000, 0b11001000, 0b10101000, 0b10101000,
                0b10011000, 0b10001000, 0b10001000, 0b10001000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'O' | 'o' => [
                0b01110000, 0b10001000, 0b10001000, 0b10001000,
                0b10001000, 0b10001000, 0b10001000, 0b01110000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'P' | 'p' => [
                0b11110000, 0b10001000, 0b10001000, 0b11110000,
                0b10000000, 0b10000000, 0b10000000, 0b10000000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'R' | 'r' => [
                0b11110000, 0b10001000, 0b10001000, 0b11110000,
                0b10100000, 0b10010000, 0b10001000, 0b10001000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'S' | 's' => [
                0b01111000, 0b10000000, 0b10000000, 0b01110000,
                0b00001000, 0b00001000, 0b00001000, 0b11110000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'T' | 't' => [
                0b11111000, 0b00100000, 0b00100000, 0b00100000,
                0b00100000, 0b00100000, 0b00100000, 0b00100000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'U' | 'u' => [
                0b10001000, 0b10001000, 0b10001000, 0b10001000,
                0b10001000, 0b10001000, 0b10001000, 0b01110000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'V' | 'v' => [
                0b10001000, 0b10001000, 0b10001000, 0b10001000,
                0b10001000, 0b01010000, 0b01010000, 0b00100000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            'W' | 'w' => [
                0b10001000, 0b10001000, 0b10001000, 0b10001000,
                0b10101000, 0b10101000, 0b11011000, 0b10001000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            ' ' => [0; 12],
            ':' => [
                0b00000000, 0b00000000, 0b01100000, 0b01100000,
                0b00000000, 0b01100000, 0b01100000, 0b00000000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            '/' => [
                0b00000000, 0b00001000, 0b00010000, 0b00100000,
                0b01000000, 0b10000000, 0b00000000, 0b00000000,
                0b00000000, 0b00000000, 0b00000000, 0b00000000,
            ],
            _ => [0; 12], // Default to empty for unknown characters
        };
        
        for (row, &byte) in pattern.iter().enumerate() {
            for bit in 0..8 {
                if (byte >> (7 - bit)) & 1 == 1 {
                    let px = x + bit;
                    let py = y + row;
                    if px < window_width && py < frame.len() / (window_width * 4) {
                        let idx = (py * window_width + px) * 4;
                        if idx + 3 < frame.len() {
                            frame[idx] = color[0];     // R
                            frame[idx + 1] = color[1]; // G
                            frame[idx + 2] = color[2]; // B
                            frame[idx + 3] = 255;      // A
                        }
                    }
                }
            }
        }
    }

    fn draw_syntax_highlighted_text(&self, frame: &mut [u8], text: &str, x: usize, y: usize, window_width: usize) {
        let keywords = ["def", "if", "set", "and", "then", "return", "end", "create", "with"];
        let colors = ["c_red", "c_green", "c_blue", "c_yellow", "c_cyan", "c_magenta"];
        let numbers = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
        
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut current_x = x;
        
        for (i, word) in words.iter().enumerate() {
            let color = if keywords.contains(word) {
                [100, 200, 255] // Blue for keywords
            } else if colors.iter().any(|&c| word.contains(c)) {
                [255, 150, 100] // Orange for color references
            } else if word.chars().any(|c| c.is_numeric()) {
                [150, 255, 150] // Green for numbers
            } else if *word == "self" || *word == "hits" || *word == "times" {
                [255, 200, 100] // Yellow for special words
            } else {
                [255, 255, 255] // White for regular text
            };
            
            self.draw_text(frame, word, current_x, y, color, false, window_width);
            current_x += word.len() * 8 + 8; // Move to next word position
            
            // Add space between words (except for last word)
            if i < words.len() - 1 {
                self.draw_text(frame, " ", current_x - 8, y, [255, 255, 255], false, window_width);
            }
        }
        
        // Handle case where text is empty or only whitespace
        if words.is_empty() {
            self.draw_text(frame, text, x, y, [255, 255, 255], false, window_width);
        }
    }
}