use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;
use crate::square::{LibraryManager, Program, Cell};
use crate::program_editor::{ProgramEditor, ProgramEditorAction};
use crate::font;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum ProgramSource {
    Library { library_name: String },
    Square { x: usize, y: usize, program_index: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SampleSource {
    Auto,
    Library { library_name: String },
}

#[derive(Debug, Clone)]
pub struct SampleEntry {
    pub name: String,
    pub source: SampleSource,
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
        target_square: Option<(usize, usize)>, // Add this field to track target square
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
    EditProgram { source: ProgramSource, name: String, program: Program, raw_text: Vec<String> },
    OpenSquareScript { x: usize, y: usize, program_index: usize },
    LoadSample { library_name: String },
    LoadAutoSample,
    SaveProgramToFile { editor: ProgramEditor },
    LoadProgramFromFile,
    OpenAudioPlayer { library_name: String, sample_name: String },
    LoadProgramToSquare { program: Program, square_x: usize, square_y: usize },
}

const LIBRARY_GUI_WIDTH: usize = 580;
const LIBRARY_GUI_HEIGHT: usize = 420;
const COLUMN_WIDTH: usize = 280;
const ITEM_HEIGHT: usize = 22;
const HEADER_HEIGHT: usize = 40;
const MAX_VISIBLE_ITEMS: usize = 16;

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
                target_square: None, // No target square when opened normally
            },
            LibraryGuiState::Visible { .. } => LibraryGuiState::Hidden,
        };
    }

    // Add this new method for opening with Programs column selected
    pub fn open_for_program_selection(&mut self, square_x: usize, square_y: usize) {
        self.state = LibraryGuiState::Visible {
            selected_column: LibraryColumn::Programs,
            selected_library: "lib".to_string(),
            selected_item: 0,
            scroll_offset: 0,
            editing_mode: None, // Ensure we're not in editing mode
            target_square: Some((square_x, square_y)), // Store the target square
        };
        // Add debug logging
        println!("Library GUI opened for program selection at square ({}, {})", square_x, square_y);
    }

    pub fn is_visible(&self) -> bool {
        matches!(self.state, LibraryGuiState::Visible { .. })
    }

    pub fn is_editing(&self) -> bool {
        if let LibraryGuiState::Visible { editing_mode, .. } = &self.state {
            editing_mode.is_some()
        } else {
            false
        }
    }

    pub fn get_current_editor_mut(&mut self) -> Option<&mut ProgramEditor> {
        if let LibraryGuiState::Visible { editing_mode: Some(ref mut edit_mode), .. } = &mut self.state {
            match edit_mode {
                EditingMode::CreateProgram { editor, .. } => Some(editor),
                EditingMode::EditProgram { editor, .. } => Some(editor),
                EditingMode::RenameItem { .. } => None,
            }
        } else {
            None
        }
    }

    pub fn handle_input(&mut self, input: &WinitInputHelper, library_manager: &LibraryManager, grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) -> Option<LibraryGuiAction> {
        // Extract state to avoid borrowing conflicts
        let (mut selected_column, mut selected_library, mut selected_item, mut scroll_offset, mut editing_mode, mut target_square) = 
            if let LibraryGuiState::Visible { 
                selected_column, 
                selected_library, 
                selected_item, 
                scroll_offset,
                editing_mode,
                target_square
            } = &self.state {
                (selected_column.clone(), selected_library.clone(), *selected_item, *scroll_offset, editing_mode.clone(), *target_square)
            } else {
                return None;
            };
            
        // If opened from square menu (target_square is Some), don't allow editing mode
        if target_square.is_some() && editing_mode.is_some() {
            editing_mode = None;
        }
            
        // Handle editing mode input ONLY if we're actually in editing mode
        if let Some(ref mut edit_mode) = editing_mode {
            let result = self.handle_editing_input(input, edit_mode, &selected_library);
            // Update state - but don't overwrite editing_mode if it was set to None by handle_editing_input
            if let LibraryGuiState::Visible { editing_mode: ref current_editing_mode, .. } = &self.state {
                if current_editing_mode.is_none() {
                    // handle_editing_input already updated the state to None, don't overwrite it
                    return result;
                }
            }
            self.state = LibraryGuiState::Visible {
                selected_column,
                selected_library,
                selected_item,
                scroll_offset,
                editing_mode,
                target_square,
            };
            return result;
        }

        // Add debug output for navigation
        if input.key_pressed(VirtualKeyCode::Up) || input.key_pressed(VirtualKeyCode::Down) {
            println!("Navigation key pressed, current item: {}, target_square: {:?}", selected_item, target_square);
        }

        // Handle escape key to close library when not in editing mode
        if input.key_pressed(VirtualKeyCode::Escape) {
            self.state = LibraryGuiState::Hidden;
            return None;
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

        if input.held_shift() && input.key_pressed(VirtualKeyCode::Space) { // Create new program or load sample
            // Only allow creating new programs when NOT opened from square menu
            if target_square.is_none() {
                match selected_column {
                    LibraryColumn::Programs => {
                        let initial_text = vec!["def new_program".to_string(), "".to_string()];
                        editing_mode = Some(EditingMode::CreateProgram {
                            name: "new_program".to_string(),
                            editor: ProgramEditor::new_with_text(initial_text),
                        });
                    }
                    LibraryColumn::Samples => {
                        // Check if the selected sample is auto or library
                        let all_samples = self.collect_all_samples(library_manager, &selected_library);
                        if let Some(sample_entry) = all_samples.get(selected_item) {
                            match &sample_entry.source {
                                SampleSource::Auto => {
                                    result = Some(LibraryGuiAction::LoadAutoSample);
                                },
                                SampleSource::Library { library_name } => {
                                    result = Some(LibraryGuiAction::LoadSample {
                                        library_name: library_name.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        if input.key_pressed(VirtualKeyCode::Return) { // Open program or audio player
            match selected_column {
                LibraryColumn::Programs => {
                    let all_programs = self.collect_all_programs(library_manager, grid);
                    if let Some(program_entry) = all_programs.get(selected_item) {
                        // For both square and library programs, use the editing mode
                        let script = self.program_to_source_code(&program_entry.program);
                        editing_mode = Some(EditingMode::EditProgram {
                            name: program_entry.name.clone(),
                            source: program_entry.source.clone(),
                            editor: ProgramEditor::new_with_text(script),
                        });
                    }
                }
                LibraryColumn::Samples => {
                    // Open audio player for .wav files
                    let all_samples = self.collect_all_samples(library_manager, &selected_library);
                    if let Some(sample_entry) = all_samples.get(selected_item) {
                        // Check if the original name (without suffix) ends with .wav
                        let original_name = if sample_entry.name.contains(" (") {
                            sample_entry.name.split(" (").next().unwrap_or(&sample_entry.name)
                        } else {
                            &sample_entry.name
                        };
                        
                        if original_name.ends_with(".wav") {
                            // Use the correct library name based on the sample source
                            let library_name = match &sample_entry.source {
                                SampleSource::Auto => "auto".to_string(),
                                SampleSource::Library { library_name } => library_name.clone(),
                            };
                            
                            result = Some(LibraryGuiAction::OpenAudioPlayer {
                                library_name,
                                sample_name: original_name.to_string(),
                            });
                        }
                    }
                }
            }
        }

        if input.key_pressed(VirtualKeyCode::Space) && !input.held_shift() {
            match selected_column {
                LibraryColumn::Programs => {
                    let all_programs = self.collect_all_programs(library_manager, grid);
                    if let Some(program_entry) = all_programs.get(selected_item) {
                        // Check if we have a target square (opened from square menu)
                        println!("Space pressed on program: {}, target_square: {:?}", program_entry.name, target_square);
                        if let Some((square_x, square_y)) = target_square {
                            // Load program into the target square and close library
                            println!("Loading program '{}' into square ({}, {})", program_entry.name, square_x, square_y);
                            result = Some(LibraryGuiAction::LoadProgramToSquare {
                                program: program_entry.program.clone(),
                                square_x,
                                square_y,
                            });
                            // Close the library GUI
                            self.state = LibraryGuiState::Hidden;
                        } else {
                            // Normal behavior - open for editing
                            println!("Opening program '{}' for editing", program_entry.name);
                            let script = self.program_to_source_code(&program_entry.program);
                            editing_mode = Some(EditingMode::EditProgram {
                                name: program_entry.name.clone(),
                                source: program_entry.source.clone(),
                                editor: ProgramEditor::new_with_text(script),
                            });
                        }
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
            target_square,
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
                match editor.handle_input_with_context(input, true) {
                    ProgramEditorAction::SaveAndCompile => {
                        let program = editor.get_program();
                        let action = Some(LibraryGuiAction::CreateProgram {
                            library_name: "lib".to_string(), // Always save user-created programs to lib
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
                            library_name: "lib".to_string(), // Always save user-created programs to lib
                            name: program.name.clone(),
                            program,
                        });
                        if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                            *editing_mode = None;
                        }
                        return action;
                    },
                    ProgramEditorAction::SaveToFile => {
                        return Some(LibraryGuiAction::SaveProgramToFile { editor: editor.clone() });
                    },
                    ProgramEditorAction::LoadFromFile => {
                        return Some(LibraryGuiAction::LoadProgramFromFile);
                    },
                    ProgramEditorAction::OpenLibrary => {
                        // This shouldn't happen in library GUI context, just ignore
                    },
                    ProgramEditorAction::None => {
                        // Do nothing
                    }
                }
            },
            EditingMode::EditProgram { name, source, editor } => {
                match editor.handle_input_with_context(input, true) {
                    ProgramEditorAction::SaveAndCompile => {
                        let program = editor.get_program();
                        let raw_text = editor.get_program_text();
                        let action = Some(LibraryGuiAction::EditProgram {
                            source: source.clone(),
                            name: name.clone(),
                            program,
                            raw_text,
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
                        let raw_text = editor.get_program_text();
                        let action = Some(LibraryGuiAction::EditProgram {
                            source: source.clone(),
                            name: name.clone(),
                            program,
                            raw_text,
                        });
                        if let LibraryGuiState::Visible { editing_mode, .. } = &mut self.state {
                            *editing_mode = None;
                        }
                        return action;
                    },
                    ProgramEditorAction::SaveToFile => {
                        return Some(LibraryGuiAction::SaveProgramToFile { editor: editor.clone() });
                    },
                    ProgramEditorAction::LoadFromFile => {
                        return Some(LibraryGuiAction::LoadProgramFromFile);
                    },
                    ProgramEditorAction::OpenLibrary => {
                        // This shouldn't happen in library GUI context, just ignore
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
                self.collect_all_samples(library_manager, library_name).len()
            },
            LibraryColumn::Programs => {
                self.collect_all_programs(library_manager, grid).len()
            },
        }
    }

    fn get_selected_item_name(&self, library_manager: &LibraryManager, column: &LibraryColumn, library_name: &str, index: usize, grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) -> Option<String> {
        match column {
            LibraryColumn::Samples => {
                let all_samples = self.collect_all_samples(library_manager, library_name);
                all_samples.get(index).map(|entry| entry.name.clone())
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

    fn collect_all_samples(&self, library_manager: &LibraryManager, selected_library: &str) -> Vec<SampleEntry> {
        let mut all_samples = Vec::new();
        
        // Add auto samples first
        if let Some(auto_library) = library_manager.sample_libraries.get("auto") {
            for (name, _sample) in &auto_library.samples {
                all_samples.push(SampleEntry {
                    name: format!("{} (auto)", name),
                    source: SampleSource::Auto,
                });
            }
        }
        
        // Add library samples if it's not the auto library
        if selected_library != "auto" {
            if let Some(library) = library_manager.sample_libraries.get(selected_library) {
                for (name, _sample) in &library.samples {
                    all_samples.push(SampleEntry {
                        name: format!("{} ({})", name, selected_library),
                        source: SampleSource::Library { library_name: selected_library.to_string() },
                    });
                }
            }
        }
        
        all_samples
    }

    fn collect_all_programs(&self, library_manager: &LibraryManager, grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT]) -> Vec<ProgramEntry> {
        let mut all_programs = Vec::new();
        let mut seen_names = std::collections::HashSet::new();
        
        // First, collect programs from squares (prioritize original square programs)
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
                        
                        // Only add if we haven't seen this program name before
                        if seen_names.insert(program.name.clone()) {
                            all_programs.push(ProgramEntry {
                                name: program.name.clone(),
                                program: program.clone(),
                                source: ProgramSource::Square { x, y, program_index: prog_index },
                            });
                        }
                    }
                }
            }
        }
        
        // Then, include library programs that don't conflict with square programs
        for (lib_name, lib) in &library_manager.function_libraries {
            for (prog_name, program) in &lib.functions {
                // Skip predefined functions and auto-generated copies of square programs
                if lib_name == "lib" && self.is_predefined_function(prog_name) {
                    continue;
                }
                if lib_name == "auto" {
                    continue; // Skip auto library entirely as these are copies of square programs
                }
                
                // Only add if we haven't seen this program name before
                if seen_names.insert(prog_name.clone()) {
                    all_programs.push(ProgramEntry {
                        name: prog_name.clone(),
                        program: program.clone(),
                        source: ProgramSource::Library { library_name: lib_name.clone() },
                    });
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
        // If the program has preserved source text, use it directly
        if let Some(ref source_text) = program.source_text {
            return source_text.clone();
        }
        
        // Otherwise, reconstruct from instructions (fallback for library functions)
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
                // Skip unknown instructions instead of adding comments
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
            editing_mode,
            target_square
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
        // Draw gradient background similar to program editor
        for dy in 0..LIBRARY_GUI_HEIGHT {
            for dx in 0..LIBRARY_GUI_WIDTH {
                let px = x + dx;
                let py = y + dy;
                if px < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + px) * 4;
                    if idx + 3 < frame.len() {
                        // Create subtle gradient from top to bottom
                        let gradient_factor = dy as f32 / LIBRARY_GUI_HEIGHT as f32;
                        let base_color = 45.0 + gradient_factor * 10.0;
                        
                        frame[idx] = base_color as u8;     // R
                        frame[idx + 1] = base_color as u8; // G
                        frame[idx + 2] = (base_color + 5.0) as u8; // B - slightly more blue
                        frame[idx + 3] = 255; // A
                    }
                }
            }
        }

        // Draw enhanced border
        self.draw_border(frame, x, y, LIBRARY_GUI_WIDTH, LIBRARY_GUI_HEIGHT, window_width);
    }

    fn draw_border(&self, frame: &mut [u8], x: usize, y: usize, width: usize, height: usize, window_width: usize) {
        // Draw multi-layered border for depth
        let outer_border = [120, 120, 130];
        let inner_border = [80, 80, 90];
        
        // Outer border (2px thick)
        for thickness in 0..2 {
            // Top and bottom borders
            for dx in 0..width {
                let px = x + dx;
                if px < window_width {
                    // Top border
                    let top_y = y + thickness;
                    if top_y < frame.len() / (window_width * 4) {
                        let idx = (top_y * window_width + px) * 4;
                        if idx + 3 < frame.len() {
                            let color = if thickness == 0 { outer_border } else { inner_border };
                            frame[idx] = color[0];
                            frame[idx + 1] = color[1];
                            frame[idx + 2] = color[2];
                        }
                    }
                    // Bottom border
                    let bottom_y = y + height - 1 - thickness;
                    if bottom_y < frame.len() / (window_width * 4) {
                        let idx = (bottom_y * window_width + px) * 4;
                        if idx + 3 < frame.len() {
                            let color = if thickness == 0 { outer_border } else { inner_border };
                            frame[idx] = color[0];
                            frame[idx + 1] = color[1];
                            frame[idx + 2] = color[2];
                        }
                    }
                }
            }

            // Left and right borders
            for dy in 0..height {
                let py = y + dy;
                if py < frame.len() / (window_width * 4) {
                    // Left border
                    let left_x = x + thickness;
                    if left_x < window_width {
                        let idx = (py * window_width + left_x) * 4;
                        if idx + 3 < frame.len() {
                            let color = if thickness == 0 { outer_border } else { inner_border };
                            frame[idx] = color[0];
                            frame[idx + 1] = color[1];
                            frame[idx + 2] = color[2];
                        }
                    }
                    // Right border
                    let right_x = x + width - 1 - thickness;
                    if right_x < window_width {
                        let idx = (py * window_width + right_x) * 4;
                        if idx + 3 < frame.len() {
                            let color = if thickness == 0 { outer_border } else { inner_border };
                            frame[idx] = color[0];
                            frame[idx + 1] = color[1];
                            frame[idx + 2] = color[2];
                        }
                    }
                }
            }
        }
    }

    fn draw_headers(&self, frame: &mut [u8], x: usize, y: usize, selected_column: &LibraryColumn, window_width: usize) {
        // Draw header background with subtle gradient
        for dy in 0..HEADER_HEIGHT {
            for dx in 0..LIBRARY_GUI_WIDTH {
                let px = x + dx;
                let py = y + dy;
                if px < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + px) * 4;
                    if idx + 3 < frame.len() {
                        let gradient_factor = dy as f32 / HEADER_HEIGHT as f32;
                        let base_color = 35.0 + gradient_factor * 15.0;
                        
                        frame[idx] = base_color as u8;
                        frame[idx + 1] = base_color as u8;
                        frame[idx + 2] = (base_color + 8.0) as u8;
                        frame[idx + 3] = 255;
                    }
                }
            }
        }

        // Draw title
        font::draw_text(frame, "Library Manager", x + 15, y + 8, [200, 200, 255], false, window_width);
        
        // Sample header
        let sample_selected = matches!(selected_column, LibraryColumn::Samples);
        self.draw_text(frame, "SAMPLES", x + 15, y + 25, 
                      if sample_selected { [255, 255, 100] } else { [180, 180, 180] }, 
                      sample_selected, window_width);

        // Program header
        let program_selected = matches!(selected_column, LibraryColumn::Programs);
        self.draw_text(frame, "PROGRAMS", x + COLUMN_WIDTH + 25, y + 25, 
                      if program_selected { [255, 255, 100] } else { [180, 180, 180] }, 
                      program_selected, window_width);

        // Draw column separator with enhanced styling
        let separator_x = x + COLUMN_WIDTH + 10;
        for dy in HEADER_HEIGHT..LIBRARY_GUI_HEIGHT {
            let py = y + dy;
            if separator_x < window_width && py < frame.len() / (window_width * 4) {
                let idx = (py * window_width + separator_x) * 4;
                if idx + 3 < frame.len() {
                    // Create a subtle gradient separator
                    let gradient = (dy - HEADER_HEIGHT) as f32 / (LIBRARY_GUI_HEIGHT - HEADER_HEIGHT) as f32;
                    let color_val = (100.0 + gradient * 20.0) as u8;
                    frame[idx] = color_val;
                    frame[idx + 1] = color_val;
                    frame[idx + 2] = color_val + 10;
                }
            }
        }
    }

    fn draw_sample_column(&self, frame: &mut [u8], x: usize, y: usize, library_manager: &LibraryManager, 
                         selected_library: &str, selected_column: &LibraryColumn, 
                         selected_item: usize, scroll_offset: usize, window_width: usize) {
        let all_samples = self.collect_all_samples(library_manager, selected_library);
        let start_y = y + HEADER_HEIGHT + 5;
        let is_column_selected = matches!(selected_column, LibraryColumn::Samples);
        
        for (i, sample_entry) in all_samples.iter().enumerate().skip(scroll_offset).take(MAX_VISIBLE_ITEMS) {
            let item_y = start_y + (i - scroll_offset) * ITEM_HEIGHT;
            let is_selected = is_column_selected && (i) == selected_item;
            
            // Draw selection background
            if is_selected {
                for dx in 0..(COLUMN_WIDTH - 10) {
                    for dy in 0..(ITEM_HEIGHT - 2) {
                        let px = x + 8 + dx;
                        let py = item_y - 2 + dy;
                        if px < window_width && py < frame.len() / (window_width * 4) {
                            let idx = (py * window_width + px) * 4;
                            if idx + 3 < frame.len() {
                                frame[idx] = frame[idx].saturating_add(25);
                                frame[idx + 1] = frame[idx + 1].saturating_add(25);
                                frame[idx + 2] = frame[idx + 2].saturating_add(35);
                            }
                        }
                    }
                }
            }
            
            // Draw sample icon with different colors for auto vs library samples
            let icon_color = match &sample_entry.source {
                SampleSource::Auto => if is_selected { [255, 150, 100] } else { [180, 120, 80] },
                SampleSource::Library { .. } => if is_selected { [100, 200, 255] } else { [120, 120, 150] },
            };
            
            font::draw_text(frame, "♪", x + 15, item_y, icon_color, false, window_width);
            
            self.draw_text(frame, &sample_entry.name, x + 30, item_y, 
                          if is_selected { [255, 255, 100] } else { [220, 220, 220] }, 
                          is_selected, window_width);
        }
    }

    fn draw_program_column(&self, frame: &mut [u8], x: usize, y: usize, library_manager: &LibraryManager, 
                          grid: &[[Cell; crate::sequencer::GRID_WIDTH]; crate::sequencer::GRID_HEIGHT],
                          selected_library: &str, selected_column: &LibraryColumn, 
                          selected_item: usize, scroll_offset: usize, window_width: usize) {
        let start_y = y + HEADER_HEIGHT + 5;
        let is_column_selected = matches!(selected_column, LibraryColumn::Programs);
        
        let all_programs = self.collect_all_programs(library_manager, grid);
        for (i, entry) in all_programs.iter().enumerate().skip(scroll_offset).take(MAX_VISIBLE_ITEMS) {
            let item_y = start_y + i * ITEM_HEIGHT;
            let is_selected = is_column_selected && (i + scroll_offset) == selected_item;
            
            // Draw selection background
            if is_selected {
                for dx in 0..(COLUMN_WIDTH - 20) {
                    for dy in 0..(ITEM_HEIGHT - 2) {
                        let px = x + 8 + dx;
                        let py = item_y - 2 + dy;
                        if px < window_width && py < frame.len() / (window_width * 4) {
                            let idx = (py * window_width + px) * 4;
                            if idx + 3 < frame.len() {
                                frame[idx] = frame[idx].saturating_add(25);
                                frame[idx + 1] = frame[idx + 1].saturating_add(25);
                                frame[idx + 2] = frame[idx + 2].saturating_add(35);
                            }
                        }
                    }
                }
            }
            
            // Draw program type icon
            let (icon, icon_color) = match &entry.source {
                ProgramSource::Library { .. } => ("📚", if is_selected { [100, 255, 100] } else { [100, 150, 100] }),
                ProgramSource::Square { .. } => ("⚡", if is_selected { [255, 200, 100] } else { [150, 120, 80] }),
            };
            
            font::draw_text(frame, icon, x + 15, item_y, icon_color, false, window_width);
            
            let display_text = entry.name.clone();
            self.draw_text(frame, &display_text, x + 30, item_y, 
                          if is_selected { [255, 255, 100] } else { [220, 220, 220] }, 
                          is_selected, window_width);
        }
    }

    fn draw_editing_overlay(&self, frame: &mut [u8], x: usize, y: usize, edit_mode: &EditingMode, window_width: usize) {
        match edit_mode {
            EditingMode::RenameItem { original_name: _, new_name } => {
                let overlay_width = 350;
                let overlay_height = 150;
                let overlay_x = x + (LIBRARY_GUI_WIDTH - overlay_width) / 2;
                let overlay_y = y + (LIBRARY_GUI_HEIGHT - overlay_height) / 2;

                // Draw overlay background with gradient
                for dy in 0..overlay_height {
                    for dx in 0..overlay_width {
                        let px = overlay_x + dx;
                        let py = overlay_y + dy;
                        if px < window_width && py < frame.len() / (window_width * 4) {
                            let idx = (py * window_width + px) * 4;
                            if idx + 3 < frame.len() {
                                let gradient_factor = dy as f32 / overlay_height as f32;
                                let base_color = 60.0 + gradient_factor * 15.0;
                                
                                frame[idx] = base_color as u8;
                                frame[idx + 1] = base_color as u8;
                                frame[idx + 2] = (base_color + 10.0) as u8;
                                frame[idx + 3] = 255;
                            }
                        }
                    }
                }

                self.draw_border(frame, overlay_x, overlay_y, overlay_width, overlay_height, window_width);
                
                // Draw title with icon
                font::draw_text(frame, "✏️", overlay_x + 15, overlay_y + 15, [255, 200, 100], false, window_width);
                self.draw_text(frame, "Rename Item", overlay_x + 35, overlay_y + 15, [200, 200, 255], false, window_width);
                
                // Draw input field background
                for dx in 0..(overlay_width - 40) {
                    for dy in 0..25 {
                        let px = overlay_x + 20 + dx;
                        let py = overlay_y + 45 + dy;
                        if px < window_width && py < frame.len() / (window_width * 4) {
                            let idx = (py * window_width + px) * 4;
                            if idx + 3 < frame.len() {
                                frame[idx] = 40;
                                frame[idx + 1] = 40;
                                frame[idx + 2] = 50;
                                frame[idx + 3] = 255;
                            }
                        }
                    }
                }
                
                self.draw_text(frame, new_name, overlay_x + 25, overlay_y + 50, [255, 255, 100], true, window_width);
                self.draw_text(frame, "Enter: Confirm  •  Esc: Cancel", overlay_x + 20, overlay_y + 85, [180, 180, 180], false, window_width);
            },
            EditingMode::CreateProgram { name, editor } => {
                editor.draw_program_editor(frame, "Create Program", "Arrow Keys: Navigate | Ctrl+Space: Load | Shift+Space: Save | ESC: Save & Exit");
            },
            EditingMode::EditProgram { name, source: _, editor } => {
                editor.draw_program_editor(frame, &format!("Edit Program: {}", name), "Arrow Keys: Navigate | Ctrl+Space: Load | Shift+Space: Save | ESC: Save & Exit");
            },
        }
    }



    fn draw_text(&self, frame: &mut [u8], text: &str, x: usize, y: usize, color: [u8; 3], selected: bool, window_width: usize) {
        font::draw_text(frame, text, x, y, color, selected, window_width);
    }



    fn draw_syntax_highlighted_text(&self, frame: &mut [u8], text: &str, x: usize, y: usize, window_width: usize) {
        font::draw_syntax_highlighted_text(frame, text, x, y, window_width);
    }
}