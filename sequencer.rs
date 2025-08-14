use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use rfd::FileDialog;

use crate::ball::{Ball, Direction};
use crate::square::{Cell, CellContent, ProgramAction, DestroyTarget, LibraryManager};
use crate::context_menu::{ContextMenu, ContextMenuAction};
use crate::square_menu::{SquareContextMenu, SquareMenuAction};
use crate::programmer::ProgramExecutor;
use crate::audio_engine::AudioEngine;
use crate::library_gui::{LibraryGui, LibraryGuiAction};
use crate::sample_manager::SampleManager;
use crate::ball_audio::BallAudioSystem;
use crate::audio_player::{AudioPlayer, AudioPlayerAction};
use crate::font;
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct CollisionEvent {
    pub ball_index: usize,
    pub ball_color: String,
    pub square_x: usize,
    pub square_y: usize,
    pub timestamp: std::time::Instant,
}

#[derive(Clone, Debug)]
pub struct CollisionCooldown {
    pub ball_index: usize,
    pub square_x: usize,
    pub square_y: usize,
    pub last_collision: std::time::Instant,
}


pub const GRID_WIDTH: usize = 16;
pub const GRID_HEIGHT: usize = 12;
const CELL_SIZE: usize = 40;
const CONSOLE_HEIGHT: usize = 150;
const WINDOW_WIDTH: usize = GRID_WIDTH * CELL_SIZE;
const WINDOW_HEIGHT: usize = GRID_HEIGHT * CELL_SIZE + CONSOLE_HEIGHT;
const GRID_AREA_HEIGHT: usize = GRID_HEIGHT * CELL_SIZE;

pub struct Cursor {
    pub x: usize,
    pub y: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { x: 7, y: 5 }
    }
    
    pub fn move_up(&mut self) {
        if self.y > 0 {
            self.y -= 1;
        }
    }
    
    pub fn move_down(&mut self) {
        if self.y < GRID_HEIGHT - 1 {
            self.y += 1;
        }
    }
    
    pub fn move_left(&mut self) {
        if self.x > 0 {
            self.x -= 1;
        }
    }
    
    pub fn move_right(&mut self) {
        if self.x < GRID_WIDTH - 1 {
            self.x += 1;
        }
    }
}

pub struct SequencerGrid {
    pub cells: [[Cell; GRID_WIDTH]; GRID_HEIGHT],
    pub cursor: Cursor,
    pub balls: Vec<Ball>,
    pub context_menu: ContextMenu,
    pub square_menu: SquareContextMenu,
    pub program_executor: ProgramExecutor,
    pub selected_ball: Option<usize>,
    pub collision_history: VecDeque<CollisionEvent>,
    pub audio_engine: AudioEngine,
    pub ball_audio_system: BallAudioSystem,
    pub console_messages: VecDeque<String>,
    pub collision_cooldowns: Vec<CollisionCooldown>,
    pub library_manager: LibraryManager,
    pub library_gui: LibraryGui,
    pub sample_manager: SampleManager,
    pub audio_player: AudioPlayer,
    // State tracking for reset functionality
    pub original_cells: [[Cell; GRID_WIDTH]; GRID_HEIGHT],
    pub original_balls: Vec<Ball>,
    pub ball_counter: u32,
}

impl SequencerGrid {
    pub fn new(audio_engine: AudioEngine) -> Self {
        let initial_cells = std::array::from_fn(|_| std::array::from_fn(|_| Cell::default()));
        let sample_manager = SampleManager::new().expect("Failed to create SampleManager");
        Self {
            cells: initial_cells.clone(),
            cursor: Cursor::new(),
            balls: Vec::new(),
            context_menu: ContextMenu::new(),
            square_menu: SquareContextMenu::new(),
            program_executor: ProgramExecutor::new(),
            selected_ball: None,
            collision_history: VecDeque::new(),
            audio_engine,
            ball_audio_system: BallAudioSystem::new(),
            console_messages: VecDeque::new(),
            collision_cooldowns: Vec::new(),
            library_manager: LibraryManager::new(),
            library_gui: LibraryGui::new(),
            sample_manager,
            audio_player: AudioPlayer::new(),
            // Initialize original state
            original_cells: initial_cells,
            original_balls: Vec::new(),
            ball_counter: 0,
        }
    }
    
    pub fn log_to_console(&mut self, message: String) {
        // Add timestamp to message
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let formatted_message = format!("[{}] {}", timestamp, message);
        
        // Add to console (keep only last 10 messages)
        self.console_messages.push_back(formatted_message.clone());
        if self.console_messages.len() > 10 {
            self.console_messages.pop_front();
        }
        
        // Write to file
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("parser_log.txt") {
            let _ = writeln!(file, "{}", formatted_message);
        }
    }
    

    
    pub fn place_square(&mut self, x: usize, y: usize) {
        if x < GRID_WIDTH && y < GRID_HEIGHT {
            self.cells[y][x].place_square(Some([255, 100, 100])); // Red square
        }
    }
    
    pub fn place_ball(&mut self, x: usize, y: usize) {
        if x < GRID_WIDTH && y < GRID_HEIGHT {
            // Create a ball at this position but don't start it moving
            self.ball_counter += 1;
            let ball_id = format!("ball{}", self.ball_counter);
            let ball = Ball::new(x, y, ball_id);
            self.balls.push(ball);
        }
    }
    
    pub fn clear_cell(&mut self, x: usize, y: usize) {
        if x < GRID_WIDTH && y < GRID_HEIGHT {
            self.cells[y][x].clear();
            
            // Remove any ball at this position (check both original and current positions)
            self.balls.retain(|ball| {
                let (current_x, current_y) = ball.get_grid_position();
                let original_x = (ball.original_x - 0.5) as usize;
                let original_y = (ball.original_y - 0.5) as usize;
                !(current_x == x && current_y == y) && !(original_x == x && original_y == y)
            });
        }
    }
    
    pub fn get_ball_at(&self, x: usize, y: usize) -> Option<usize> {
        self.balls.iter().position(|ball| {
            let (ball_x, ball_y) = ball.get_grid_position();
            ball_x == x && ball_y == y
        })
    }
    
    pub fn open_context_menu(&mut self, x: usize, y: usize) {
        if let Some(ball_index) = self.get_ball_at(x, y) {
            self.context_menu.open_ball_menu(ball_index);
            self.selected_ball = Some(ball_index);
        } else if x < GRID_WIDTH && y < GRID_HEIGHT && self.cells[y][x].is_square() {
            // Open square programming menu
            self.square_menu.open_square_menu(x, y);
        }
    }
    
    pub fn close_context_menu(&mut self) {
        self.context_menu.close();
        self.square_menu.close();
        self.selected_ball = None;
    }
    
    pub fn set_ball_direction(&mut self, ball_index: usize, direction: Direction) {
        if ball_index < self.balls.len() {
            self.balls[ball_index].set_direction(direction);
        }
    }
    
    pub fn set_ball_speed(&mut self, ball_index: usize, speed: f32) {
        if ball_index < self.balls.len() {
            self.balls[ball_index].set_speed(speed);
        }
    }
    
    pub fn set_ball_sample(&mut self, ball_index: usize, sample_path: String) {
        if ball_index < self.balls.len() {
            // Import sample to local folder and get local path
            let local_path = match self.sample_manager.import_sample(&sample_path) {
                Ok(path) => {
                    self.log_to_console(format!("Imported sample to local folder: {}", path));
                    path
                },
                Err(e) => {
                    self.log_to_console(format!("Failed to import sample {}: {}", sample_path, e));
                    sample_path.clone() // Fallback to original path
                }
            };
            
            // Set the local path for the ball
            self.balls[ball_index].set_sample(local_path.clone());
            
            // Preload the sample for better performance using local path
            if let Err(e) = self.audio_engine.preload_sample(&local_path) {
                self.log_to_console(format!("Warning: Failed to preload sample {}: {}", local_path, e));
            } else {
                self.log_to_console(format!("Preloaded sample: {}", local_path));
            }
            
            // Automatically add sample to library using original path
            self.auto_add_sample_to_library(&sample_path, "ball");
        }
    }
    
    pub fn set_ball_color(&mut self, ball_index: usize, color: String) {
        if ball_index < self.balls.len() {
            self.balls[ball_index].set_color(color);
        }
    }
    
    pub fn reset_balls_to_origin(&mut self) {
        for ball in &mut self.balls {
            ball.reset_to_original();
        }
    }
    
    pub fn reset_balls(&mut self) {
        for ball in &mut self.balls {
            ball.reset_to_original();
        }
    }
    
    pub fn toggle_all_balls(&mut self) {
        let any_active = self.balls.iter().any(|ball| ball.active);
        
        if any_active {
            // If any balls are active, reset to original state
            self.reset_to_original_state();
        } else {
            // If no balls are active, save current state as original and start balls
            self.save_current_state_as_original();
            for ball in &mut self.balls {
                ball.activate();
            }
        }
        
        // Reset all hit counts and variables when toggling ball states
        self.program_executor.reset_all_state();
    }
    
    pub fn save_current_state_as_original(&mut self) {
        // Save current grid state as the original state
        self.original_cells = self.cells.clone();
        self.original_balls = self.balls.clone();
        self.log_to_console("Current state saved as original".to_string());
    }
    
    pub fn reset_to_original_state(&mut self) {
        // Restore grid to original state
        self.cells = self.original_cells.clone();
        
        // Preserve sample paths when restoring balls
        let current_sample_paths: Vec<Option<String>> = self.balls.iter()
            .map(|ball| ball.sample_path.clone())
            .collect();
        
        self.balls = self.original_balls.clone();
        
        // Restore the sample paths
        for (i, sample_path) in current_sample_paths.into_iter().enumerate() {
            if i < self.balls.len() {
                self.balls[i].sample_path = sample_path;
            }
        }
        
        // Reset all balls to their original positions and stop them
        for ball in &mut self.balls {
            ball.reset_to_original();
        }
        
        // Clear collision history and cooldowns
        self.collision_history.clear();
        self.collision_cooldowns.clear();
        
        self.log_to_console("Grid reset to original state".to_string());
    }
    
    pub fn find_last_ball_collision(&self, ball_color: &str, square_x: usize, square_y: usize) -> Option<usize> {
        // Find the most recent collision of a ball with the specified color hitting the specified square
        self.collision_history
            .iter()
            .rev() // Start from most recent
            .find(|event| {
                event.ball_color == ball_color && 
                event.square_x == square_x && 
                event.square_y == square_y
            })
            .map(|event| event.ball_index)
    }
    
    // Automatically add sample template to library when used in creation
    pub fn auto_add_sample_template_to_library(&mut self, sample_template: &crate::square::SampleTemplate, sample_type: &str) {
        use crate::square::SampleLibrary;
        
        // Check if sample already exists in auto library
        if self.library_manager.get_sample_template("auto", &sample_template.name).is_some() {
            return; // Already exists
        }
        
        // Get or create auto library
        if !self.library_manager.sample_libraries.contains_key("auto") {
            let auto_library = SampleLibrary {
                name: "auto".to_string(),
                samples: std::collections::HashMap::new(),
                description: "Automatically generated samples from loaded files".to_string(),
            };
            self.library_manager.add_sample_library(auto_library);
        }
        
        // Add sample to auto library
        if let Some(auto_lib) = self.library_manager.sample_libraries.get_mut("auto") {
            auto_lib.samples.insert(sample_template.name.clone(), sample_template.clone());
            self.log_to_console(format!("Auto-added sample template '{}' to library for {}", sample_template.name, sample_type));
        }
    }
    
    // Automatically add sample to library when loaded into ball or square
    pub fn auto_add_sample_to_library(&mut self, sample_path: &str, sample_type: &str) {
        self.add_sample_to_library(sample_path, sample_type, "auto");
    }
    
    // Add sample to specified library
    pub fn add_sample_to_library(&mut self, sample_path: &str, sample_type: &str, library_name: &str) {
        use crate::square::{SampleTemplate, SampleLibrary};
        use crate::ball::Direction;
        use std::path::Path;
        
        // Extract full filename as sample name
        let sample_name = Path::new(sample_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Check if sample already exists in the specified library
        if self.library_manager.get_sample_template(library_name, &sample_name).is_some() {
            self.log_to_console(format!("Sample '{}' already exists in library '{}'", sample_name, library_name));
            return; // Already exists
        }
        
        // Copy the sample to local samples folder
        let local_path = match self.sample_manager.import_sample(sample_path) {
            Ok(path) => {
                self.log_to_console(format!("Imported sample {} to local samples folder", sample_name));
                path
            },
            Err(e) => {
                self.log_to_console(format!("Failed to import sample {}: {}", sample_name, e));
                sample_path.to_string() // Fallback to original path
            }
        };
        
        // Create sample template with defaults
        let sample_template = SampleTemplate {
            name: sample_name.clone(),
            default_speed: 2.0,
            default_direction: Direction::Up,
            color: if sample_type == "ball" { "white".to_string() } else { "gray".to_string() },
            behavior_program: None,
        };
        
        // Get or create the specified library
        if !self.library_manager.sample_libraries.contains_key(library_name) {
            let new_library = SampleLibrary {
                name: library_name.to_string(),
                samples: std::collections::HashMap::new(),
                description: format!("Sample library: {}", library_name),
            };
            self.library_manager.add_sample_library(new_library);
        }
        
        // Add sample to the specified library
        if let Some(lib) = self.library_manager.sample_libraries.get_mut(library_name) {
            lib.samples.insert(sample_name.clone(), sample_template);
            self.log_to_console(format!("Added sample '{}' to library '{}' from {}", sample_name, library_name, local_path));
        }
    }
    
    // Automatically add program to library when created in square
    pub fn auto_add_program_to_library(&mut self, program: &crate::square::Program) {
        use crate::square::FunctionLibrary;
        
        // Check if program already exists in auto library
        if self.library_manager.get_function("auto", &program.name).is_some() {
            return; // Already exists
        }
        
        // Get or create auto library
        if !self.library_manager.function_libraries.contains_key("auto") {
            let auto_library = FunctionLibrary {
                name: "auto".to_string(),
                functions: std::collections::HashMap::new(),
                description: "Automatically generated functions from square programs".to_string(),
            };
            self.library_manager.add_function_library(auto_library);
        }
        
        // Add program to auto library
        if let Some(auto_lib) = self.library_manager.function_libraries.get_mut("auto") {
            auto_lib.functions.insert(program.name.clone(), program.clone());
            self.log_to_console(format!("Auto-added program '{}' to library", program.name));
        }
    }
    
    // Handle console commands for library access
    pub fn handle_console_command(&mut self, command: &str) {
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        if parts.is_empty() {
            return;
        }
        
        match parts[0].to_lowercase().as_str() {
            "lib" | "library" => {
                if parts.len() == 1 {
                    self.show_library_help();
                } else {
                    match parts[1] {
                        "list" => self.list_libraries(),
                        "functions" => {
                            if parts.len() > 2 {
                                self.list_functions_in_library(parts[2]);
                            } else {
                                self.list_all_functions();
                            }
                        },
                        "samples" => {
                            if parts.len() > 2 {
                                self.list_samples_in_library(parts[2]);
                            } else {
                                self.list_all_samples();
                            }
                        },
                        "clear" => {
                            if parts.len() > 2 && parts[2] == "auto" {
                                self.clear_auto_library();
                            } else {
                                self.log_to_console("Usage: lib clear auto".to_string());
                            }
                        },
                        _ => self.show_library_help(),
                    }
                }
            },
            _ => {}
        }
    }
    
    fn show_library_help(&mut self) {
        self.log_to_console("Library Commands:".to_string());
        self.log_to_console("  lib list - List all libraries".to_string());
        self.log_to_console("  lib functions [library] - List functions".to_string());
        self.log_to_console("  lib samples [library] - List samples".to_string());
        self.log_to_console("  lib clear auto - Clear auto-generated library".to_string());
    }
    
    fn list_libraries(&mut self) {
        let mut messages = Vec::new();
        messages.push("Function Libraries:".to_string());
        for (name, lib) in &self.library_manager.function_libraries {
            messages.push(format!("  {} - {} ({} functions)", name, lib.description, lib.functions.len()));
        }
        messages.push("Sample Libraries:".to_string());
        for (name, lib) in &self.library_manager.sample_libraries {
            messages.push(format!("  {} - {} ({} samples)", name, lib.description, lib.samples.len()));
        }
        for message in messages {
            self.log_to_console(message);
        }
    }
    
    fn list_functions_in_library(&mut self, library_name: &str) {
        if let Some(lib) = self.library_manager.function_libraries.get(library_name) {
            let mut messages = Vec::new();
            messages.push(format!("Functions in '{}' library:", library_name));
            for (name, program) in &lib.functions {
                messages.push(format!("  {} - {} instructions", name, program.instructions.len()));
            }
            for message in messages {
                self.log_to_console(message);
            }
        } else {
            self.log_to_console(format!("Function library '{}' not found", library_name));
        }
    }
    
    fn list_all_functions(&mut self) {
        let mut messages = Vec::new();
        for (lib_name, lib) in &self.library_manager.function_libraries {
            messages.push(format!("Functions in '{}' library:", lib_name));
            for (name, program) in &lib.functions {
                messages.push(format!("  {}.{} - {} instructions", lib_name, name, program.instructions.len()));
            }
        }
        for message in messages {
            self.log_to_console(message);
        }
    }
    
    fn list_samples_in_library(&mut self, library_name: &str) {
        if let Some(lib) = self.library_manager.sample_libraries.get(library_name) {
            let mut messages = Vec::new();
            messages.push(format!("Samples in '{}' library:", library_name));
            for (name, sample) in &lib.samples {
                messages.push(format!("  {} - speed:{}, dir:{:?}, color:{}", 
                    name, sample.default_speed, sample.default_direction, sample.color));
            }
            for message in messages {
                self.log_to_console(message);
            }
        } else {
            self.log_to_console(format!("Sample library '{}' not found", library_name));
        }
    }
    
    fn list_all_samples(&mut self) {
        let mut messages = Vec::new();
        for (lib_name, lib) in &self.library_manager.sample_libraries {
            messages.push(format!("Samples in '{}' library:", lib_name));
            for (name, sample) in &lib.samples {
                messages.push(format!("  {}.{} - speed:{}, dir:{:?}, color:{}", 
                    lib_name, name, sample.default_speed, sample.default_direction, sample.color));
            }
        }
        for message in messages {
            self.log_to_console(message);
        }
    }
    
    fn clear_auto_library(&mut self) {
        self.library_manager.function_libraries.remove("auto");
        self.library_manager.sample_libraries.remove("auto");
        self.log_to_console("Cleared auto-generated library".to_string());
    }
    
    /// Add an error comment to the program's source text to help users identify issues
    fn add_error_comment_to_program(&mut self, grid_x: usize, grid_y: usize, error_msg: &str) {
        if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
            let square_program = &mut self.cells[grid_y][grid_x].program;
            if let Some(active_index) = square_program.active_program {
                if let Some(program) = square_program.programs.get_mut(active_index) {
                    if let Some(ref mut source_text) = program.source_text {
                        // Check if this error comment already exists to avoid duplicates
                        let error_comment = format!("// RUNTIME ERROR: {}", error_msg);
                        if !source_text.iter().any(|line| line.contains(&error_comment)) {
                            // Find the line with the problematic function call and add comment after it
                            let mut found_error_line = false;
                            for (i, line) in source_text.iter().enumerate() {
                                if line.contains("return ") && error_msg.contains("Unknown function") {
                                    // Extract function name from error message
                                    if let Some(func_start) = error_msg.find("Unknown function: ") {
                                        let func_name = &error_msg[func_start + 17..];
                                        if line.contains(func_name) {
                                            source_text.insert(i + 1, error_comment.clone());
                                            found_error_line = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            // If we couldn't find the specific line, add at the top
                            if !found_error_line {
                                source_text.insert(0, error_comment);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn resolve_ball_reference(&self, ball_reference: &str, current_square_x: usize, current_square_y: usize) -> Option<usize> {
        // Parse ball reference syntax: "last.c_red.self(-10)"
        // Format: last.<color>.self(<speed>)
        if ball_reference.starts_with("last.") {
            let parts: Vec<&str> = ball_reference.split('.').collect();
            if parts.len() >= 3 && parts[0] == "last" && parts[2].starts_with("self") {
                let ball_color = parts[1];
                // For "self", we look for collisions with the current square
                return self.find_last_ball_collision(ball_color, current_square_x, current_square_y);
            }
        }
        None
    }
    
    pub fn update_balls(&mut self, delta_time: f32) -> Vec<(usize, usize, usize)> { // Returns (x, y, ball_index) where samples should be triggered
        let mut triggered_positions = Vec::new();
        
        // Clean up finished audio samples for better performance
        self.audio_engine.cleanup_finished_samples();
        
        // Collect reverse sample actions to process after the mutable iteration
        let mut reverse_sample_actions = Vec::new();
        
        // Collect all log messages to avoid borrowing conflicts
        let mut all_log_messages = Vec::new();
        
        // Collect create/destroy actions to process after ball iteration
        let mut create_ball_actions = Vec::new();
        let mut create_ball_with_library_actions = Vec::new();
        let mut destroy_ball_actions = Vec::new();
        let mut create_square_actions = Vec::new();
        let mut create_square_with_program_actions = Vec::new();
        let mut create_ball_from_sample_actions = Vec::new();
        let mut create_square_from_sample_actions = Vec::new();
        let mut destroy_square_actions = Vec::new();
        
        // Performance monitoring
        let active_samples = self.audio_engine.get_active_sample_count();
        if active_samples > 15 {
            // Skip audio processing if too many samples are playing to prevent audio engine overload
            self.log_to_console(format!("Audio engine overloaded ({} samples), skipping new triggers", active_samples));
            return triggered_positions;
        }
        
        // Collect ball information for reference resolution before mutable iteration
        let ball_positions: Vec<(f32, f32)> = self.balls.iter().map(|b| (b.x, b.y)).collect();
        let collision_history = self.collision_history.clone();
        
        // Helper function to resolve ball references without borrowing self
        let resolve_ball_ref = |ball_reference: &str, current_square_x: usize, current_square_y: usize| -> Option<usize> {
            if ball_reference.starts_with("last.") {
                let parts: Vec<&str> = ball_reference.split('.').collect();
                if parts.len() >= 3 && parts[0] == "last" && parts[2].starts_with("self") {
                    let ball_color = parts[1];
                    // Find the most recent collision of a ball with the specified color hitting the specified square
                    return collision_history
                        .iter()
                        .rev() // Start from most recent
                        .find(|event| {
                            event.ball_color == ball_color && 
                            event.square_x == current_square_x && 
                            event.square_y == current_square_y
                        })
                        .map(|event| event.ball_index);
                }
            }
            None
        };
        
        // Collect error comments to add after ball iteration (to avoid borrowing conflicts)
        let mut error_comments: Vec<(usize, usize, String)> = Vec::new();
        
        for (ball_index, ball) in self.balls.iter_mut().enumerate() {
            if !ball.active {
                continue;
            }
            
            // Store old position for collision detection
            let old_x = ball.x;
            let old_y = ball.y;
            
            // Update ball position and get newly entered grid cells
            let entered_cells = ball.update_position(delta_time);
            
            // Check for collisions with squares in newly entered cells
            for (grid_x, grid_y) in entered_cells {
                if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                    if self.cells[grid_y][grid_x].is_square() {
                        // Record collision event
                        let collision_event = CollisionEvent {
                            ball_index,
                            ball_color: ball.color.clone(),
                            square_x: grid_x,
                            square_y: grid_y,
                            timestamp: std::time::Instant::now(),
                        };
                        self.collision_history.push_back(collision_event);
                        
                        // Keep only recent collisions (last 100)
                        if self.collision_history.len() > 100 {
                            self.collision_history.pop_front();
                        }
                        
                        // Audio will be played after program actions are processed
                        
                        // Check cooldown before executing program
                        let can_execute = {
                            const COOLDOWN_MS: u128 = 100; // 100ms cooldown between executions
                            let now = std::time::Instant::now();
                            
                            // Check if there's an existing cooldown for this combination
                            if let Some(cooldown) = self.collision_cooldowns.iter().find(|c| 
                                c.ball_index == ball_index && c.square_x == grid_x && c.square_y == grid_y
                            ) {
                                now.duration_since(cooldown.last_collision).as_millis() >= COOLDOWN_MS
                            } else {
                                true // No existing cooldown
                            }
                        };
                        
                        if can_execute {
                            // Increment the square's own hit count
                            self.cells[grid_y][grid_x].program.hit_count += 1;
                            let new_hit_count = self.cells[grid_y][grid_x].program.hit_count;
                            all_log_messages.push(format!("Square ({},{}) hit count incremented to: {}", grid_x, grid_y, new_hit_count));
                            
                            let square_program = &self.cells[grid_y][grid_x].program;
                            // Debug: Show what programs are available on this square
                            all_log_messages.push(format!("Square ({},{}) has {} programs, active: {:?}", 
                                grid_x, grid_y, square_program.programs.len(), square_program.active_program));
                            for (i, prog) in square_program.programs.iter().enumerate() {
                                all_log_messages.push(format!("  Program {}: '{}' with {} instructions", i, prog.name, prog.instructions.len()));
                            }
                            
                            if !square_program.programs.is_empty() {
                                if let Some(active_program_index) = square_program.active_program {
                                    if let Some(program) = square_program.programs.get(active_program_index) {
                                        let actions = self.program_executor.execute_on_collision(
                                            program, ball, grid_x, grid_y
                                        );
                                        
                                        // Collect log messages to avoid borrowing conflicts
                                        if !actions.is_empty() {
                                            all_log_messages.push(format!(
                                                "Executing program at ({},{}) for {} ball: {} actions",
                                                grid_x, grid_y, ball.color, actions.len()
                                            ));
                                        }
                                        
                                        // Check if any action requires ball position reset
                        let mut should_reset_position = false;
                        let mut should_snap_to_grid_center = false;
                        let mut explicit_bounce = false;
                        let mut collision_pitch = ball.pitch; // Start with ball's base pitch
                        
                        // Apply program actions to the ball
                        for action in actions {
                                            match action {
                                                ProgramAction::SetSpeed(speed) => {
                                                    all_log_messages.push(format!("  → SetSpeed: {}", speed));
                                                    ball.speed = speed.max(0.1); // Ensure minimum speed
                                                    should_reset_position = true;
                                                }
                                                ProgramAction::SetPitch(pitch) => {
                                                    all_log_messages.push(format!("  → SetPitch: {} (collision-specific)", pitch));
                                                    collision_pitch = pitch; // Apply pitch only for this collision
                                                }
                                                ProgramAction::SetVolume(volume) => {
                                                    all_log_messages.push(format!("  → SetVolume: {}", volume));
                                                    ball.set_volume(volume);
                                                }
                                                ProgramAction::Return(function_name) => {
                                                    if let Some(ref func_name) = function_name {
                                                        all_log_messages.push(format!("  → Return: calling function '{}'", func_name));
                                                        
                                                        // Look for the named function in the current square's programs
                                                        let square_program = &self.cells[grid_y][grid_x].program;
                                                        let mut found_function = None;
                                                        
                                                        for program in &square_program.programs {
                                                            if program.name == *func_name {
                                                                found_function = Some(program.clone());
                                                                break;
                                                            }
                                                        }
                                                        
                                                        if let Some(target_program) = found_function {
                                                            all_log_messages.push(format!("    Executing function: {}", func_name));
                                                            
                                                            // Execute the target function's instructions
                                                            let mut context = crate::square::ExecutionContext {
                                                                variables: std::collections::HashMap::new(),
                                                                ball_hit_count: 0,
                                                                square_hit_count: 0,
                                                                ball_x: ball.x,
                                                                ball_y: ball.y,
                                                                ball_speed: ball.speed,
                                                                ball_direction: ball.direction,
                                                                ball_pitch: ball.pitch,
                                                                ball_volume: ball.volume,
                                                                square_x: grid_x,
                                                                square_y: grid_y,
                                                            };
                                                            
                                                            // Create a temporary SquareProgram to execute the function
                                                            let mut temp_square_program = crate::square::SquareProgram::new();
                                                            let function_actions = temp_square_program.execute_instructions(&target_program.instructions, &mut context);
                                                            
                                                            // Apply the actions from the function
                                                            for function_action in function_actions {
                                                                match function_action {
                                                                    ProgramAction::CreateBall { x, y, speed, direction } => {
                                                                        all_log_messages.push(format!("    Function creating ball at ({}, {})", x, y));
                                                                        create_ball_actions.push((x, y, speed, direction));
                                                                    }
                                                                    ProgramAction::CreateSquare { x, y } => {
                                                                        all_log_messages.push(format!("    Function creating square at ({}, {})", x, y));
                                                                        create_square_actions.push((x, y));
                                                                    }
                                                                    ProgramAction::SetSpeed(speed) => {
                                                                        all_log_messages.push(format!("    Function setting speed: {}", speed));
                                                                        ball.speed = speed.max(0.1);
                                                                        should_reset_position = true;
                                                                    }
                                                                    ProgramAction::SetPitch(pitch) => {
                                                                        all_log_messages.push(format!("    Function setting pitch: {}", pitch));
                                                                        ball.set_pitch(pitch);
                                                                    }
                                                                    ProgramAction::SetVolume(volume) => {
                                                                        all_log_messages.push(format!("    Function setting volume: {}", volume));
                                                                        ball.set_volume(volume);
                                                                    }
                                                                    ProgramAction::SetDirection(direction) => {
                                                                        all_log_messages.push(format!("    Function setting direction: {:?}", direction));
                                                                        ball.direction = direction;
                                                                        should_snap_to_grid_center = true;
                                                                    }
                                                                    ProgramAction::Bounce => {
                                                                        all_log_messages.push("    Function bouncing".to_string());
                                                                        ball.reverse_direction();
                                                                        should_reset_position = true;
                                                                        explicit_bounce = true;
                                                                    }
                                                                    // Handle other actions as needed
                                                                    _ => {
                                                                        all_log_messages.push(format!("    Function action: {:?}", function_action));
                                                                    }
                                                                }
                                                            }
                                                        } else {
                                                            all_log_messages.push(format!("    Unknown function: {}", func_name));
                                                            // Collect error info to add comment later (after ball iteration)
                                                            error_comments.push((grid_x, grid_y, format!("Unknown function: {}", func_name)));
                                                        }
                                                    } else {
                                                        all_log_messages.push("  → Return: simple return".to_string());
                                                    }
                                                }
                                                ProgramAction::End => {
                                                    all_log_messages.push("  → End: natural block termination".to_string());
                                                }
                                                ProgramAction::SetDirection(direction) => {
                                                    all_log_messages.push(format!("  → SetDirection: {:?}", direction));
                                                    ball.direction = direction;
                                                    should_snap_to_grid_center = true;
                                                }
                                                ProgramAction::Bounce => {
                                                    all_log_messages.push("  → Bounce".to_string());
                                                    ball.reverse_direction();
                                                    should_reset_position = true;
                                                    explicit_bounce = true;
                                                }
                                                ProgramAction::Stop => {
                                                    all_log_messages.push("  → Stop".to_string());
                                                    ball.active = false;
                                                    should_reset_position = true;
                                                }
                                                ProgramAction::PlaySample(sample_index) => {
                                                    // Use centralized audio system for PlaySample action
                                                    if let Err(e) = self.ball_audio_system.play_sample_action(
                                                        &self.audio_engine,
                                                        ball,
                                                        collision_pitch,
                                                        sample_index as u32,
                                                        &mut all_log_messages,
                                                    ) {
                                                        all_log_messages.push(format!("PlaySample audio error: {}", e));
                                                    }
                                                    // PlaySample doesn't affect ball movement, so don't reset position
                                                }
                                                ProgramAction::SetReverse { ball_reference, speed } => {
                                                    all_log_messages.push(format!("  → SetReverse: {} at speed {}", ball_reference, speed));
                                                    // Collect for later processing to avoid borrowing conflicts
                                                    reverse_sample_actions.push((ball_reference, speed, grid_x, grid_y));
                                                    // SetReverse doesn't affect ball movement, so don't reset position
                                                }
                                                ProgramAction::CreateBall { x, y, speed, direction } => {
                                                    all_log_messages.push(format!("  → CreateBall at ({}, {}) with speed {} and direction {:?}", x, y, speed, direction));
                                                    create_ball_actions.push((x, y, speed, direction));
                                                }
                                                ProgramAction::CreateSquare { x, y } => {
                                                    all_log_messages.push(format!("  → CreateSquare at ({}, {})", x, y));
                                                    create_square_actions.push((x, y));
                                                }
                                                ProgramAction::CreateSquareWithProgram { x, y, program } => {
                                                    all_log_messages.push(format!("  → CreateSquareWithProgram at ({}, {})", x, y));
                                                    create_square_with_program_actions.push((x, y, program));
                                                }
                                                ProgramAction::CreateBallFromSample { x, y, library_name, sample_name } => {
                                                    all_log_messages.push(format!("  → CreateBallFromSample at ({}, {}) from {}.{}", x, y, library_name, sample_name));
                                                    create_ball_from_sample_actions.push((x, y, library_name, sample_name));
                                                }
                                                ProgramAction::CreateSquareFromSample { x, y, library_name, sample_name } => {
                                                    all_log_messages.push(format!("  → CreateSquareFromSample at ({}, {}) from {}.{}", x, y, library_name, sample_name));
                                                    create_square_from_sample_actions.push((x, y, library_name, sample_name));
                                                }
                                                ProgramAction::CreateBallWithLibrary { x, y, library_function, audio_file } => {
                                                    all_log_messages.push(format!("  → CreateBallWithLibrary at ({}, {}) with lib.{}", x, y, library_function));
                                                    if let Some(ref audio) = audio_file {
                                                        all_log_messages.push(format!("    and lib.{}", audio));
                                                    }
                                                    
                                                    // Collect ball creation with library for processing after iteration
                                                    create_ball_with_library_actions.push((x, y, library_function.clone(), audio_file.clone()));
                                                    all_log_messages.push(format!("    Ball with library queued for creation at ({}, {})", x, y));
                                                }
                                                ProgramAction::CreateSquareWithLibrary { x, y, library_function, audio_file } => {
                                                    all_log_messages.push(format!("  → CreateSquareWithLibrary at ({}, {}) with lib.{}", x, y, library_function));
                                                    if let Some(audio) = audio_file {
                                                        all_log_messages.push(format!("    and lib.{}", audio));
                                                    }
                                                    
                                                    // Create square with library function loaded
                                                    let grid_x = x as usize;
                                                    let grid_y = y as usize;
                                                    if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                                                        // Get the library function program
                                                        if let Some(library_program) = self.library_manager.get_function("lib", &library_function) {
                                                            self.cells[grid_y][grid_x].place_square(None);
                                                            self.cells[grid_y][grid_x].program.add_program(library_program.clone());
                                                            let program_count = self.cells[grid_y][grid_x].program.programs.len();
                                                            self.cells[grid_y][grid_x].program.set_active_program(Some(program_count - 1));
                                                            
                                                            all_log_messages.push(format!("    Square created at ({}, {}) with lib.{} loaded", grid_x, grid_y, library_function));
                                                        } else {
                                                            all_log_messages.push(format!("    Failed to load library function: lib.{}", library_function));
                                                        }
                                                    }
                                                }
                                                ProgramAction::DestroyBall { x, y, ball_reference } => {
                                                    if let Some(ball_ref) = ball_reference {
                                                        if ball_ref == "self" {
                                                            // Destroy the current ball
                                                            all_log_messages.push(format!("  → DestroyBall self (ball {})", ball_index));
                                                            destroy_ball_actions.push((ball.x, ball.y));
                                                        } else if let Some(target_ball_index) = resolve_ball_ref(&ball_ref, grid_x, grid_y) {
                                                             if target_ball_index < ball_positions.len() {
                                                                 let (target_x, target_y) = ball_positions[target_ball_index];
                                                                 all_log_messages.push(format!("  → DestroyBall {} (ball {})", ball_ref, target_ball_index));
                                                                 destroy_ball_actions.push((target_x, target_y));
                                                            }
                                                        }
                                                    } else {
                                                        // Coordinate-based destruction
                                                        all_log_messages.push(format!("  → DestroyBall at ({}, {})", x, y));
                                                        destroy_ball_actions.push((x, y));
                                                    }
                                                }
                                                ProgramAction::DestroySquare { x, y, ball_reference } => {
                                                    if let Some(ball_ref) = ball_reference {
                                                        if ball_ref == "self" {
                                                            // Destroy square at current ball position
                                                            all_log_messages.push(format!("  → DestroySquare self at ({}, {})", grid_x, grid_y));
                                                            destroy_square_actions.push((grid_x as f32, grid_y as f32));
                                                        } else if let Some(target_ball_index) = resolve_ball_ref(&ball_ref, grid_x, grid_y) {
                                                             if target_ball_index < ball_positions.len() {
                                                                 let (target_x, target_y) = ball_positions[target_ball_index];
                                                                 let target_grid_x = target_x.round() as usize;
                                                                 let target_grid_y = target_y.round() as usize;
                                                                 all_log_messages.push(format!("  → DestroySquare {} at ({}, {})", ball_ref, target_grid_x, target_grid_y));
                                                                 destroy_square_actions.push((target_grid_x as f32, target_grid_y as f32));
                                                            }
                                                        }
                                                    } else {
                                                        // Coordinate-based destruction
                                                        all_log_messages.push(format!("  → DestroySquare at ({}, {})", x, y));
                                                        destroy_square_actions.push((x, y));
                                                    }
                                                }
                                                ProgramAction::Print(text) => {
                                                    all_log_messages.push(format!("  → Print: {}", text));
                                                    
                                                    // Store the printed text on the current square for visual display
                                                    if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                                                        if self.cells[grid_y][grid_x].content == CellContent::Square {
                                                            // Truncate text to fit in square (max ~10 characters per line)
                                                            let truncated_text = if text.len() > 10 {
                                                                format!("{}...", &text[..7])
                                                            } else {
                                                                text.clone()
                                                            };
                                                            
                                                            // Get existing text and split into lines
                                                            let existing_text = self.cells[grid_y][grid_x].display_text
                                                                .as_ref()
                                                                .cloned()
                                                                .unwrap_or_default();
                                                            
                                                            let mut lines: Vec<String> = existing_text.split('\n')
                                                                .map(|s| s.to_string())
                                                                .collect();
                                                            
                                                            // Ensure we have at least 3 lines
                                                            while lines.len() < 3 {
                                                                lines.push(String::new());
                                                            }
                                                            
                                                            // Set the third line to the print output
                                                            lines[2] = truncated_text;
                                                            
                                                            // Join back into display text
                                                            let formatted_text = lines.join("\n");
                                                            self.cells[grid_y][grid_x].display_text = Some(formatted_text);
                                                        }
                                                    }
                                                }
                                                ProgramAction::ExecuteLibraryFunction { library_function } => {
                                                    all_log_messages.push(format!("  → ExecuteLibraryFunction: {}", library_function));
                                                    
                                                    // Parse the library function call (e.g., "lib.function_name" or "auto.test")
                                                    if let Some(dot_pos) = library_function.find('.') {
                                                        let library_name = &library_function[..dot_pos];
                                                        let function_name = &library_function[dot_pos + 1..];
                                                        
                                                        // Get the library function program and execute it
                                                        if let Some(library_program) = self.library_manager.get_function(library_name, function_name) {
                                                            all_log_messages.push(format!("    Executing library function: {}", function_name));
                                                            
                                                            // Execute the library function's instructions
                                                            let mut context = crate::square::ExecutionContext {
                                                                variables: std::collections::HashMap::new(),
                                                                ball_hit_count: 0,
                                                                square_hit_count: 0,
                                                                ball_x: ball.x,
                                                                ball_y: ball.y,
                                                                ball_speed: ball.speed,
                                                                ball_direction: ball.direction,
                                                                ball_pitch: ball.pitch,
                                                                ball_volume: ball.volume,
                                                                square_x: grid_x,
                                                                square_y: grid_y,
                                                            };
                                                            // Create a temporary SquareProgram to execute the library function
                                                            let mut temp_square_program = crate::square::SquareProgram::new();
                                                            let library_actions = temp_square_program.execute_instructions(&library_program.instructions, &mut context);
                                                            
                                                            // Apply the actions from the library function
                                            for library_action in library_actions {
                                                match library_action {
                                                    ProgramAction::CreateBall { x, y, speed, direction } => {
                                                        all_log_messages.push(format!("    Library function creating ball at ({}, {})", x, y));
                                                        create_ball_actions.push((x, y, speed, direction));
                                                    }
                                                    ProgramAction::CreateSquare { x, y } => {
                                                        all_log_messages.push(format!("    Library function creating square at ({}, {})", x, y));
                                                        create_square_actions.push((x, y));
                                                    }
                                                    ProgramAction::Return(function_name) => {
                                                        if let Some(ref func_name) = function_name {
                                                            all_log_messages.push(format!("    Library function return: calling function '{}'", func_name));
                                                            
                                                            // Look for the named function in the current square's programs
                                                            let square_program = &self.cells[grid_y][grid_x].program;
                                                            let mut found_function = None;
                                                            
                                                            for program in &square_program.programs {
                                                                if program.name == *func_name {
                                                                    found_function = Some(program.clone());
                                                                    break;
                                                                }
                                                            }
                                                            
                                                            if let Some(target_program) = found_function {
                                                                all_log_messages.push(format!("      Executing function: {}", func_name));
                                                                
                                                                // Execute the target function's instructions
                                                                let mut context = crate::square::ExecutionContext {
                                                                    variables: std::collections::HashMap::new(),
                                                                    ball_hit_count: 0,
                                                                    square_hit_count: 0,
                                                                    ball_x: ball.x,
                                                                    ball_y: ball.y,
                                                                    ball_speed: ball.speed,
                                                                    ball_direction: ball.direction,
                                                                    ball_pitch: ball.pitch,
                                                                    ball_volume: ball.volume,
                                                                    square_x: grid_x,
                                                                    square_y: grid_y,
                                                                };
                                                                
                                                                // Create a temporary SquareProgram to execute the function
                                                                let mut temp_square_program = crate::square::SquareProgram::new();
                                                                let function_actions = temp_square_program.execute_instructions(&target_program.instructions, &mut context);
                                                                
                                                                // Apply the actions from the function
                                                                for function_action in function_actions {
                                                                    match function_action {
                                                                        ProgramAction::CreateBall { x, y, speed, direction } => {
                                                                            all_log_messages.push(format!("      Function creating ball at ({}, {})", x, y));
                                                                            create_ball_actions.push((x, y, speed, direction));
                                                                        }
                                                                        ProgramAction::CreateSquare { x, y } => {
                                                                            all_log_messages.push(format!("      Function creating square at ({}, {})", x, y));
                                                                            create_square_actions.push((x, y));
                                                                        }
                                                                        ProgramAction::SetSpeed(speed) => {
                                                                            all_log_messages.push(format!("      Function setting speed: {}", speed));
                                                                            ball.speed = speed.max(0.1);
                                                                            should_reset_position = true;
                                                                        }
                                                                        ProgramAction::SetPitch(pitch) => {
                                                                            all_log_messages.push(format!("      Function setting pitch: {}", pitch));
                                                                            ball.set_pitch(pitch);
                                                                        }
                                                                        ProgramAction::SetVolume(volume) => {
                                                                            all_log_messages.push(format!("      Function setting volume: {}", volume));
                                                                            ball.set_volume(volume);
                                                                        }
                                                                        ProgramAction::SetDirection(direction) => {
                                                                            all_log_messages.push(format!("      Function setting direction: {:?}", direction));
                                                                            ball.direction = direction;
                                                                            should_snap_to_grid_center = true;
                                                                        }
                                                                        ProgramAction::Bounce => {
                                                                            all_log_messages.push("      Function bouncing".to_string());
                                                                            ball.reverse_direction();
                                                                            should_reset_position = true;
                                                                            explicit_bounce = true;
                                                                        }
                                                                        // Handle other actions as needed
                                                                        _ => {
                                                                            all_log_messages.push(format!("      Function action: {:?}", function_action));
                                                                        }
                                                                    }
                                                                }
                                                            } else {
                                                                all_log_messages.push(format!("      Unknown function: {}", func_name));
                                                                // Collect error info to add comment later (after ball iteration)
                                                                error_comments.push((grid_x, grid_y, format!("Unknown function: {}", func_name)));
                                                            }
                                                        } else {
                                                            all_log_messages.push("    Library function return: simple return".to_string());
                                                        }
                                                    }
                                                    ProgramAction::End => {
                                                        all_log_messages.push("    Library function end: natural block termination".to_string());
                                                    }
                                                    // Handle other actions as needed
                                                    _ => {
                                                        all_log_messages.push(format!("    Library function action: {:?}", library_action));
                                                    }
                                                }
                                            }
                                                        } else {
                                                            all_log_messages.push(format!("    Failed to find library function: {}.{}", library_name, function_name));
                                                        }
                                                    } else {
                                                        all_log_messages.push(format!("    Invalid library function format: {} (expected library.function)", library_function));
                                                    }
                                                }
                                                ProgramAction::SetSliceArray { x, y, markers } => {
                                                    all_log_messages.push(format!("  → SetSliceArray at ({}, {}) with {} markers", x, y, markers.len()));
                                                    
                                                    // Only set up the slice array if it doesn't already exist
                                                    if !self.program_executor.state.slice_arrays.contains_key(&(x, y)) {
                                                        // Store the slice array in the program executor state
                                                        self.program_executor.state.slice_arrays.insert((x, y), markers.clone());
                                                        // Initialize the hit index to 0 for first time setup
                                                        self.program_executor.state.slice_hit_indices.insert((x, y), 0);
                                                        all_log_messages.push("    Slice array initialized".to_string());
                                                    } else {
                                                        all_log_messages.push("    Slice array already exists, skipping setup".to_string());
                                                    }
                                                }
                                                ProgramAction::PlaySliceMarker { x, y, marker_index } => {
                                                    all_log_messages.push(format!("  → PlaySliceMarker at ({}, {}) marker {}", x, y, marker_index));
                                                    // Get the current slice array for this square
                                                    if let Some(slice_array) = self.program_executor.state.slice_arrays.get(&(x, y)) {
                                                        let current_index = self.program_executor.state.slice_hit_indices.get(&(x, y)).unwrap_or(&0);
                                                        if *current_index < slice_array.len() {
                                                            let marker_to_play = slice_array[*current_index];
                                                            all_log_messages.push(format!("    Playing marker {} from slice array (index {})", marker_to_play, current_index));
                                                            
                                                            // Try to get markers from audio player first, then from saved markers
                                                            let mut marker_found = false;
                                                            if let Some(markers) = self.audio_player.get_markers() {
                                                                // Look for marker by extracting number from "Marker_X" format or by position index
                                                                let marker = markers.iter().find(|m| {
                                                                    // Try to extract number from "Marker_X" format
                                                                    if m.name.starts_with("Marker_") {
                                                                        if let Ok(marker_num) = m.name[7..].parse::<u32>() {
                                                                            return marker_num == marker_to_play;
                                                                        }
                                                                    }
                                                                    // Fallback: try parsing the entire name as a number
                                                                    m.name.parse::<u32>().unwrap_or(0) == marker_to_play
                                                                });
                                                                
                                                                if let Some(marker) = marker {
                                                                    // Play the marker using the audio engine
                                                                    if let Some(sample_path) = self.audio_player.get_sample_info().map(|(path, _, _, _)| path) {
                                                                        if let Err(e) = self.audio_engine.play_on_channel_with_position(0, sample_path, 1.0, 1.0, marker.position) {
                                                                            all_log_messages.push(format!("    Error playing marker: {}", e));
                                                                        }
                                                                        marker_found = true;
                                                                    }
                                                                }
                                                            }
                                                            
                                                            // If not found in current markers, search saved markers
                                                            if !marker_found {
                                                                for (sample_path, saved_markers) in self.audio_player.get_all_saved_markers() {
                                                                    // Look for marker by extracting number from "Marker_X" format or by position index
                                                                    let marker = saved_markers.iter().find(|m| {
                                                                        // Try to extract number from "Marker_X" format
                                                                        if m.name.starts_with("Marker_") {
                                                                            if let Ok(marker_num) = m.name[7..].parse::<u32>() {
                                                                                return marker_num == marker_to_play;
                                                                            }
                                                                        }
                                                                        // Fallback: try parsing the entire name as a number
                                                                        m.name.parse::<u32>().unwrap_or(0) == marker_to_play
                                                                    });
                                                                    
                                                                    if let Some(marker) = marker {
                                                                        if let Err(e) = self.audio_engine.play_on_channel_with_position(0, &sample_path, 1.0, 1.0, marker.position) {
                                                                            all_log_messages.push(format!("    Error playing saved marker: {}", e));
                                                                        }
                                                                        marker_found = true;
                                                                        break;
                                                                    }
                                                                }
                                                            }
                                                            
                                                            if !marker_found {
                                                                all_log_messages.push(format!("    Marker {} not found in current or saved markers", marker_to_play));
                                                            }
                                                            
                                                            // Increment the hit index for next time
                                            let next_index = (*current_index + 1) % slice_array.len();
                                            self.program_executor.state.slice_hit_indices.insert((x, y), next_index);
                                                        }
                                                    }
                                                }
                                                _ => {
                                                    all_log_messages.push("  → Unknown action".to_string());
                                                } // Handle other actions as needed
                                            }
                                        }
                                        
                                        // Only play ball's audio if there's no slice array active for this square
                        let has_slice_array = self.program_executor.state.slice_arrays.contains_key(&(grid_x, grid_y));
                        if !has_slice_array {
                            if let Err(e) = self.ball_audio_system.play_collision_audio(
                                &self.audio_engine,
                                ball,
                                collision_pitch,
                                &mut all_log_messages,
                            ) {
                                all_log_messages.push(format!("Ball audio system error: {}", e));
                            }
                        } else {
                            all_log_messages.push("Skipping regular ball audio - slice array active".to_string());
                        }
                                        
                                        // Always bounce off squares unless an explicit bounce was already performed
                                        if !explicit_bounce {
                                            ball.reverse_direction();
                                            should_reset_position = true;
                                        }
                                        
                                        // Reset position based on action type
                                        if should_snap_to_grid_center {
                                            // Snap ball to center of current grid cell for SetDirection
                                            ball.x = grid_x as f32 + 0.5;
                                            ball.y = grid_y as f32 + 0.5;
                                            ball.last_grid_x = grid_x;
                                            ball.last_grid_y = grid_y;
                                        } else if should_reset_position {
                                            // Move ball back to previous position for other actions
                                            ball.x = old_x;
                                            ball.y = old_y;
                                            ball.last_grid_x = old_x.floor() as usize;
                                            ball.last_grid_y = old_y.floor() as usize;
                                        }
                                        
                                        // Check for slice arrays and play next marker in sequence
                                        if let Some(slice_array) = self.program_executor.state.slice_arrays.get(&(grid_x, grid_y)) {
                                            let current_index = self.program_executor.state.slice_hit_indices.get(&(grid_x, grid_y)).unwrap_or(&0);
                                            if *current_index < slice_array.len() {
                                                let marker_to_play = slice_array[*current_index];
                                                all_log_messages.push(format!("  → Slice Array: Playing marker {} (index {} of {})", marker_to_play, current_index, slice_array.len()));
                                                
                                                // Use the ball's sample path for slice array playback
                                                if let Some(ball_sample_path) = &ball.sample_path {
                                                    all_log_messages.push(format!("    Looking for markers for ball sample: {}", ball_sample_path));
                                                    
                                                    // First check if the ball's sample has markers in the audio player (if it's currently open)
                                                    let markers_and_path = if let Some(markers) = self.audio_player.get_markers() {
                                                        if let Some(current_sample_path) = self.audio_player.get_sample_info().map(|(path, _, _, _)| path) {
                                                            all_log_messages.push(format!("    Audio player has sample: {} (looking for: {})", current_sample_path, ball_sample_path));
                                                            if current_sample_path == ball_sample_path {
                                                                all_log_messages.push(format!("    Found {} markers in audio player", markers.len()));
                                                                Some((markers, ball_sample_path.clone()))
                                                            } else {
                                                                all_log_messages.push("    Audio player sample doesn't match ball sample".to_string());
                                                                None
                                                            }
                                                        } else {
                                                            all_log_messages.push("    No sample info in audio player".to_string());
                                                            None
                                                        }
                                                    } else {
                                                        all_log_messages.push("    No markers in audio player".to_string());
                                                        None
                                                    };
                                                    
                                                    // If not found in current audio player, check saved markers for the ball's sample
                                                    let markers_and_path = markers_and_path.or_else(|| {
                                                        if let Some(saved_markers) = self.audio_player.get_saved_markers(ball_sample_path) {
                                                            all_log_messages.push(format!("    Found {} saved markers for ball sample", saved_markers.len()));
                                                            Some((saved_markers, ball_sample_path.clone()))
                                                        } else {
                                                            all_log_messages.push("    No saved markers for ball sample".to_string());
                                                            None
                                                        }
                                                    });
                                                    
                                                    if let Some((markers, sample_path)) = markers_and_path {
                                                        // Look for marker by extracting number from "Marker_X" format or by position index
                                                        let marker = markers.iter().find(|m| {
                                                            // Try to extract number from "Marker_X" format
                                                            if m.name.starts_with("Marker_") {
                                                                if let Ok(marker_num) = m.name[7..].parse::<u32>() {
                                                                    return marker_num == marker_to_play;
                                                                }
                                                            }
                                                            // Fallback: try parsing the entire name as a number
                                                            m.name.parse::<u32>().unwrap_or(0) == marker_to_play
                                                        });
                                                        
                                                        if let Some(marker) = marker {
                                            all_log_messages.push(format!("    Found marker '{}' at position {}", marker.name, marker.position));
                                            
                                            // Find the next marker chronologically for end position
                            let end_position = {
                                // Find the next marker chronologically after the current marker
                                let mut sorted_markers: Vec<_> = markers.iter().collect();
                                sorted_markers.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                                
                                let next_chronological = sorted_markers.iter()
                                    .find(|m| m.position > marker.position)
                                    .map(|m| m.position);
                                    
                                next_chronological.unwrap_or(1.0) // Play to end if no next marker
                            };
                                            
                                            all_log_messages.push(format!("    Playing segment from {} to {:?}", marker.position, end_position));
                            
                            // Create a dedicated channel for this segment to avoid conflicts
                            let segment_channel = self.audio_engine.create_channel(format!("Segment_{}_{}", grid_x, grid_y));
                            
                            // Play the marker segment using the dedicated channel
                            if let Err(e) = self.audio_engine.play_on_channel_with_segment(segment_channel, &sample_path, 1.0, 1.0, marker.position, Some(end_position)) {
                                all_log_messages.push(format!("    Error playing slice marker: {}", e));
                            } else {
                                all_log_messages.push("    Successfully started segment playback".to_string());
                            }
                                            
                                            // Increment the hit index for next time
                                            let next_index = (*current_index + 1) % slice_array.len();
                                            self.program_executor.state.slice_hit_indices.insert((grid_x, grid_y), next_index);
                                        } else {
                                            all_log_messages.push(format!("    Marker {} not found in ball sample markers", marker_to_play));
                                        }
                                                    } else {
                                                        all_log_messages.push(format!("    No markers available for ball sample: {}", ball_sample_path));
                                                    }
                                                } else {
                                                    all_log_messages.push("    Ball has no sample path for slice array playback".to_string());
                                                }
                                            }
                                        }
                                        
                                        // Update cooldown tracking
                                        let now = std::time::Instant::now();
                                        if let Some(cooldown) = self.collision_cooldowns.iter_mut().find(|c| 
                                            c.ball_index == ball_index && c.square_x == grid_x && c.square_y == grid_y
                                        ) {
                                            cooldown.last_collision = now;
                                        } else {
                                            self.collision_cooldowns.push(CollisionCooldown {
                                                ball_index,
                                                square_x: grid_x,
                                                square_y: grid_y,
                                                last_collision: now,
                                            });
                                            
                                            // Clean up old cooldowns (keep only last 50)
                                            if self.collision_cooldowns.len() > 50 {
                                                self.collision_cooldowns.remove(0);
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Default behavior: reverse direction
                                ball.reverse_direction();
                                // Move ball back to previous position to prevent overlap
                                ball.x = old_x;
                                ball.y = old_y;
                                ball.last_grid_x = old_x.floor() as usize;
                                ball.last_grid_y = old_y.floor() as usize;
                            }
                        } else {
                            // Cooldown active, just reverse direction without executing program
                            ball.reverse_direction();
                            // Move ball back to previous position to prevent overlap
                            ball.x = old_x;
                            ball.y = old_y;
                            ball.last_grid_x = old_x.floor() as usize;
                            ball.last_grid_y = old_y.floor() as usize;
                        }
                        
                        triggered_positions.push((grid_x, grid_y, ball_index));
                        break; // Only trigger once per update
                    }
                }
            }
        }
        
        // Process reverse sample actions after the mutable iteration
        for (ball_reference, speed, grid_x, grid_y) in reverse_sample_actions {
            if let Some(referenced_ball_index) = self.resolve_ball_reference(&ball_reference, grid_x, grid_y) {
                if let Some(referenced_ball) = self.balls.get(referenced_ball_index) {
                    if let Some(sample_path) = referenced_ball.sample_path.as_ref() {
                        if let Err(e) = self.audio_engine.play_reverse_on_channel(0, sample_path, speed) {
                            eprintln!("Failed to play reverse sample: {}", e);
                        }
                    } else {
                        eprintln!("Referenced ball has no sample loaded");
                    }
                } else {
                    eprintln!("Referenced ball index {} not found", referenced_ball_index);
                }
            } else {
                eprintln!("Could not resolve ball reference: {}", ball_reference);
            }
        }
        
        // Process collected error comments after ball iteration
        for (grid_x, grid_y, error_msg) in error_comments {
            self.add_error_comment_to_program(grid_x, grid_y, &error_msg);
        }
        
        // Process create/destroy actions after the mutable iteration
        for (x, y, speed, direction) in create_ball_actions {
            let grid_x = x.round() as usize;
            let grid_y = y.round() as usize;
            if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                self.ball_counter += 1;
                let ball_id = format!("ball{}", self.ball_counter);
                let mut new_ball = Ball::new(grid_x, grid_y, ball_id.clone());
                new_ball.speed = speed;
                new_ball.direction = direction;
                new_ball.activate(); // Activate the newly created ball
                let is_active = new_ball.active;
                self.balls.push(new_ball);
                self.log_to_console(format!("Ball {} created at ({}, {}) - Total balls: {}, Active: {}", 
                    ball_id, grid_x, grid_y, self.balls.len(), is_active));
            } else {
                self.log_to_console(format!("Ball creation failed - coordinates ({}, {}) out of bounds", grid_x, grid_y));
            }
        }
        
        for (x, y) in create_square_actions {
            let grid_x = x as usize;
            let grid_y = y as usize;
            if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                self.cells[grid_y][grid_x].place_square(Some([255, 100, 100])); // Red square
            }
        }
        
        for (x, y, program) in create_square_with_program_actions {
            let grid_x = x as usize;
            let grid_y = y as usize;
            if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                self.cells[grid_y][grid_x].place_square(Some([255, 100, 100])); // Red square
                self.cells[grid_y][grid_x].program.add_program(program.clone());
                // Set the newly added program as active
                let program_count = self.cells[grid_y][grid_x].program.programs.len();
                self.cells[grid_y][grid_x].program.set_active_program(Some(program_count - 1));
                // Automatically add program to library
                self.auto_add_program_to_library(&program);
                self.log_to_console(format!("Square with program created at ({}, {})", grid_x, grid_y));
            }
        }
        
        // Process sample-based creation actions
        for (x, y, library_name, sample_name) in create_ball_from_sample_actions {
            let grid_x = x as usize;
            let grid_y = y as usize;
            if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                if let Some(sample_template) = self.library_manager.get_ball_sample(&library_name, &sample_name) {
                    let template_clone = sample_template.clone();
                    self.ball_counter += 1;
                    let ball_id = format!("ball{}", self.ball_counter);
                    let mut new_ball = Ball::new(grid_x, grid_y, ball_id.clone());
                    new_ball.speed = template_clone.default_speed;
                    new_ball.direction = template_clone.default_direction;
                    new_ball.color = template_clone.color.clone();
                    
                    // Set sample path based on sample name (assuming .wav extension)
                    let sample_path = format!("{}.wav", sample_name);
                    new_ball.set_sample(sample_path.clone());
                    
                    // Automatically add sample to library
                    self.auto_add_sample_to_library(&sample_path, "ball");
                    
                    new_ball.activate();
                    self.balls.push(new_ball);
                    self.log_to_console(format!("Ball {} created from sample {}.{} at ({}, {}) with sample path {}", ball_id, library_name, sample_name, grid_x, grid_y, sample_path));
                } else {
                    self.log_to_console(format!("Failed to create ball: sample {}.{} not found", library_name, sample_name));
                }
            } else {
                self.log_to_console(format!("Ball creation failed - coordinates ({}, {}) out of bounds", grid_x, grid_y));
            }
        }
        
        for (x, y, library_name, sample_name) in create_square_from_sample_actions {
            let grid_x = x as usize;
            let grid_y = y as usize;
            if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                if let Some(sample_template) = self.library_manager.get_square_sample(&library_name, &sample_name) {
                    // Parse color string to RGB array
                    let color_rgb = if sample_template.color == "red" {
                        [255, 100, 100]
                    } else if sample_template.color == "blue" {
                        [100, 100, 255]
                    } else if sample_template.color == "green" {
                        [100, 255, 100]
                    } else {
                        [200, 200, 200] // Default gray
                    };
                    self.cells[grid_y][grid_x].place_square(Some(color_rgb));
                    if let Some(program_name) = &sample_template.behavior_program {
                        // Look up the actual program from the library
                        if let Some(library_program) = self.library_manager.get_function("lib", program_name) {
                            let program_clone = library_program.clone();
                            self.cells[grid_y][grid_x].program.add_program(program_clone.clone());
                            let program_count = self.cells[grid_y][grid_x].program.programs.len();
                            self.cells[grid_y][grid_x].program.set_active_program(Some(program_count - 1));
                            // Automatically add program to library
                            self.auto_add_program_to_library(&program_clone);
                        }
                    }
                    self.log_to_console(format!("Square created from sample {}.{} at ({}, {})", library_name, sample_name, grid_x, grid_y));
                } else {
                    self.log_to_console(format!("Failed to create square: sample {}.{} not found", library_name, sample_name));
                }
            } else {
                self.log_to_console(format!("Square creation failed - coordinates ({}, {}) out of bounds", grid_x, grid_y));
            }
        }
        
        // Process balls created with library audio files
        for (x, y, library_function, audio_file) in create_ball_with_library_actions {
            let grid_x = x.round() as usize;
            let grid_y = y.round() as usize;
            if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                self.ball_counter += 1;
                let ball_id = format!("ball{}", self.ball_counter);
                let mut new_ball = Ball::new(grid_x, grid_y, ball_id.clone());
                
                // Load audio file if specified
                if let Some(ref audio_name) = audio_file {
                    // For library references, first check if the sample exists locally
                    let sample_path = if self.sample_manager.sample_exists(audio_name) {
                        // Sample already exists locally, get its path
                        self.sample_manager.get_local_path(audio_name)
                    } else {
                        // Try to import the sample (this will handle full paths)
                        match self.sample_manager.import_sample(audio_name) {
                            Ok(path) => {
                                self.log_to_console(format!("Imported sample {} to local folder", audio_name));
                                path
                            },
                            Err(e) => {
                                self.log_to_console(format!("Failed to import sample {}: {}", audio_name, e));
                                // For library references, try to find the file in samples directory
                                let samples_path = format!("samples/{}", audio_name);
                                if std::path::Path::new(&samples_path).exists() {
                                    samples_path
                                } else {
                                    audio_name.clone() // Final fallback
                                }
                            }
                        }
                    };
                    
                    new_ball.set_sample(sample_path.clone());
                    
                    // Preload sample to cache to avoid repeated loading using sample path
                    if let Err(e) = self.audio_engine.preload_sample(&sample_path) {
                        self.log_to_console(format!("Failed to preload sample {}: {}", sample_path, e));
                    } else {
                        self.log_to_console(format!("Audio {} loaded into ball at ({}, {})", sample_path, grid_x, grid_y));
                    }
                }
                
                new_ball.activate();
                // Use default speed and direction for library-created balls
                new_ball.speed = 1.0;
                new_ball.direction = crate::ball::Direction::Right;
                self.balls.push(new_ball);
                self.log_to_console(format!("Ball {} created with library at ({}, {}) - Total balls: {}", ball_id, grid_x, grid_y, self.balls.len()));
            } else {
                self.log_to_console(format!("Ball creation failed - coordinates ({}, {}) out of bounds", grid_x, grid_y));
            }
        }

        for (x, y) in destroy_ball_actions {
            let grid_x = x.round() as usize;
            let grid_y = y.round() as usize;
            // Remove balls at the specified position
            self.balls.retain(|ball| {
                let ball_grid_x = ball.x.round() as usize;
                let ball_grid_y = ball.y.round() as usize;
                !(ball_grid_x == grid_x && ball_grid_y == grid_y)
            });
        }
        
        for (x, y) in destroy_square_actions {
            let grid_x = x.round() as usize;
            let grid_y = y.round() as usize;
            if grid_x < GRID_WIDTH && grid_y < GRID_HEIGHT {
                self.cells[grid_y][grid_x].clear();
            }
        }
        
        // Log all collected messages after ball processing is complete
        for message in all_log_messages {
            self.log_to_console(message);
        }
        
        // Periodic performance logging (every 100 updates)
        static mut UPDATE_COUNTER: u32 = 0;
        unsafe {
            UPDATE_COUNTER += 1;
            if UPDATE_COUNTER % 100 == 0 {
                let active = self.audio_engine.get_active_sample_count();
                let cache_size = self.audio_engine.get_cache_size();
                // self.log_to_console(format!("Audio: {} active samples, {} cached", active, cache_size));
            }
        }
        
        triggered_positions
    }
    
    pub fn update(&mut self, delta_time: f32) {
        // Update audio player
        self.audio_player.update(delta_time, &self.audio_engine);
    }
}

pub struct SequencerUI {
    grid: SequencerGrid,
    pixels: Pixels,
    input: WinitInputHelper,
    last_update: std::time::Instant,
    audio_engine: AudioEngine,
    // Label editing state
    label_editing_mode: bool,
    label_editing_x: usize,
    label_editing_y: usize,
    current_label: String,
    label_editing_line: usize, // 0 for first line, 1 for second line
    // Track last cursor position for console logging
    last_cursor_x: usize,
    last_cursor_y: usize,
}

impl SequencerUI {
    pub fn new(window: &winit::window::Window, audio_engine: AudioEngine) -> Result<Self, Error> {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window);
        let pixels = Pixels::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32, surface_texture)?;
        
        let mut grid = SequencerGrid::new(audio_engine);
        
        // Add startup message to console
        grid.log_to_console("Quadracollision Canticle v0.001".to_string());
        
        Ok(Self {
            grid,
            pixels,
            input: WinitInputHelper::new(),
            last_update: std::time::Instant::now(),
            audio_engine: AudioEngine::new().map_err(|e| Error::UserDefined(Box::new(e)))?,
            label_editing_mode: false,
            label_editing_x: 0,
            label_editing_y: 0,
            current_label: String::new(),
            label_editing_line: 0,
            last_cursor_x: 0,
            last_cursor_y: 0,
        })
    }
    
    fn log_cursor_position_if_changed(&mut self) {
        let current_x = self.grid.cursor.x;
        let current_y = self.grid.cursor.y;
        
        // Only log if position actually changed
        if current_x != self.last_cursor_x || current_y != self.last_cursor_y {
            self.last_cursor_x = current_x;
            self.last_cursor_y = current_y;
            
            // Get ball information at cursor position
            let ball_info = if let Some(ball_index) = self.grid.get_ball_at(current_x, current_y) {
                if let Some(ball) = self.grid.balls.get(ball_index) {
                    let sample_name = ball.sample_path.as_ref()
                        .and_then(|path| std::path::Path::new(path).file_stem())
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("no_sample");
                    format!("Ball: {} ({})", ball.id, sample_name)
                } else {
                    "Ball: unknown".to_string()
                }
            } else {
                "Ball: none".to_string()
            };
            
            // Get square information at cursor position
            let square_info = if self.grid.cells[current_y][current_x].is_square() {
                let cell = &self.grid.cells[current_y][current_x];
                if let Some(ref display_text) = cell.display_text {
                    format!("Square: {}", display_text)
                } else {
                    "Square: unlabeled".to_string()
                }
            } else {
                "Square: none".to_string()
            };
            
            // Log to in-game console
            self.grid.log_to_console(format!("Cursor: ({}, {}) | {} | {}", current_x, current_y, ball_info, square_info));
        }
    }
    
    pub fn handle_input(&mut self, event: &Event<()>) {
        if self.input.update(event) {
            // Handle label editing mode first
            if self.label_editing_mode {
                self.handle_label_editing_input();
                return;
            }
            
            // Handle context menu input first
            if let Some(action) = self.grid.context_menu.handle_input(&self.input, &self.grid.balls) {
                 match action {
                     ContextMenuAction::SetDirection { ball_index, direction } => {
                         self.grid.set_ball_direction(ball_index, direction);
                     }
                     ContextMenuAction::SetSpeed { ball_index, speed } => {
                         self.grid.set_ball_speed(ball_index, speed);
                     }
                     ContextMenuAction::SetSample { ball_index, sample } => {
                         self.grid.set_ball_sample(ball_index, sample);
                     }
                     ContextMenuAction::SetColor { ball_index, color } => {
                         self.grid.set_ball_color(ball_index, color);
                     }
                     ContextMenuAction::OpenFileDialog { ball_index } => {
                         self.open_file_dialog_for_ball(ball_index);
                     }
                     ContextMenuAction::AddSampleToLibrary { ball_index } => {
                         self.add_sample_to_library_for_ball(ball_index);
                     }
                     ContextMenuAction::OpenAudioPlayer { ball_index } => {
                         if let Some(ball) = self.grid.balls.get(ball_index) {
                             if let Some(ref sample_path) = ball.sample_path {
                                 // Open the audio player with the ball's sample
                                 if let Err(e) = self.grid.audio_player.open_sample(sample_path.clone(), &mut self.grid.audio_engine) {
                                     self.grid.log_to_console(format!("Failed to open audio player: {}", e));
                                 } else {
                                     self.grid.log_to_console(format!("Opened audio player for ball sample: {}", sample_path));
                                 }
                             }
                         }
                     }
                 }
                 return;
             }
            
            if self.grid.context_menu.is_open() {
                return;
            }

            // Handle square menu input
            if self.grid.square_menu.is_open() {
                if let Some(action) = self.grid.square_menu.handle_input(&self.input, &self.grid.cells) {
                    match action {
                        SquareMenuAction::SaveProgram { square_x, square_y, program, program_index } => {
                            if square_x < GRID_WIDTH && square_y < GRID_HEIGHT {
                                let square_program = &mut self.grid.cells[square_y][square_x].program;
                                
                                if let Some(index) = program_index {
                                    // Update existing program
                                    square_program.update_program(index, program.clone());
                                    square_program.set_active_program(Some(index));
                                } else {
                                    // Add new program
                                    square_program.add_program(program.clone());
                                    let program_count = square_program.programs.len();
                                    square_program.set_active_program(Some(program_count - 1));
                                }
                                
                                // Automatically add program to library
                                self.grid.auto_add_program_to_library(&program);
                            }
                        }
                        SquareMenuAction::SaveMultiplePrograms { square_x, square_y, programs, program_index } => {
                            if square_x < GRID_WIDTH && square_y < GRID_HEIGHT {
                                // First, handle the square program operations
                                {
                                    let square_program = &mut self.grid.cells[square_y][square_x].program;
                                    
                                    if let Some(index) = program_index {
                                        // Replace existing program with the first one, then add the rest
                                        if !programs.is_empty() {
                                            square_program.update_program(index, programs[0].clone());
                                            square_program.set_active_program(Some(index));
                                            
                                            // Add remaining programs as new programs
                                            for program in programs.iter().skip(1) {
                                                square_program.add_program(program.clone());
                                            }
                                        }
                                    } else {
                                        // Add all programs as new programs
                                        for (i, program) in programs.iter().enumerate() {
                                            square_program.add_program(program.clone());
                                            
                                            // Set the first program as active
                                            if i == 0 {
                                                let program_count = square_program.programs.len();
                                                square_program.set_active_program(Some(program_count - 1));
                                            }
                                        }
                                    }
                                }
                                
                                // Then, add all programs to library (after square_program borrow is released)
                                for program in &programs {
                                    self.grid.auto_add_program_to_library(program);
                                }
                            }
                        }

                        SquareMenuAction::ClearPrograms { square_x, square_y } => {
                            if square_x < GRID_WIDTH && square_y < GRID_HEIGHT {
                                self.grid.cells[square_y][square_x].program.programs.clear();
                                self.grid.cells[square_y][square_x].program.set_active_program(None);
                            }
                        }
                        SquareMenuAction::SaveProgramToFile => {
                            let program_name = self.grid.square_menu.program_editor.get_program_name();
                            let program_text = self.grid.square_menu.program_editor.get_program_text();
                            self.save_program_to_file_with_data(program_name, program_text);
                        }
                        SquareMenuAction::LoadProgramFromFile => {
                            self.load_program_from_file();
                        }
                        SquareMenuAction::OpenLibrary { square_x, square_y } => {
                        // Open library GUI with Programs column selected for the specific square
                        self.grid.library_gui.open_for_program_selection(square_x, square_y);
                    }
                    }
                }
                return; // Don't process other input while square menu is open
            }

            // Library GUI open (G key) - only opens, never closes
            if self.input.key_pressed(VirtualKeyCode::G) {
                // Only open if hidden
                if !self.grid.library_gui.is_visible() {
                    self.grid.library_gui.toggle();
                }
            }
            
            // Handle library GUI input if visible and audio player is not open
            if self.grid.library_gui.is_visible() && !self.grid.audio_player.is_visible() {
                if let Some(action) = self.grid.library_gui.handle_input(&self.input, &self.grid.library_manager, &self.grid.cells) {
                    match action {
                        LibraryGuiAction::RenameItem { library_name, old_name, new_name, is_sample } => {
                            // TODO: Implement rename functionality
                            self.grid.log_to_console(format!("Rename {} from {} to {} in library {}", 
                                if is_sample { "sample" } else { "program" }, old_name, new_name, library_name));
                        }
                        LibraryGuiAction::DeleteItem { library_name, item_name, is_sample } => {
                            // TODO: Implement delete functionality
                            self.grid.log_to_console(format!("Delete {} {} from library {}", 
                                if is_sample { "sample" } else { "program" }, item_name, library_name));
                        }
                        LibraryGuiAction::CreateProgram { library_name, name, program } => {
                            // Add program to the specified library
                            if let Some(lib) = self.grid.library_manager.function_libraries.get_mut(&library_name) {
                                lib.functions.insert(name.clone(), program);
                                self.grid.log_to_console(format!("Created program '{}' in library '{}'", name, library_name));
                            } else {
                                // Create library if it doesn't exist
                                let mut new_lib = crate::square::FunctionLibrary {
                                    name: library_name.clone(),
                                    functions: std::collections::HashMap::new(),
                                    description: format!("User created library: {}", library_name),
                                };
                                new_lib.functions.insert(name.clone(), program);
                                self.grid.library_manager.function_libraries.insert(library_name.clone(), new_lib);
                                self.grid.log_to_console(format!("Created library '{}' and program '{}'", library_name, name));
                            }
                        }
                        LibraryGuiAction::EditProgram { source, name, program, raw_text } => {
                            // Create a program with the raw text preserved
                            let mut updated_program = program;
                            updated_program.source_text = Some(raw_text);
                            
                            match source {
                                crate::library_gui::ProgramSource::Library { library_name } => {
                                    // Update program in library
                                    if let Some(lib) = self.grid.library_manager.function_libraries.get_mut(&library_name) {
                                        lib.functions.insert(name.clone(), updated_program);
                                        self.grid.log_to_console(format!("Updated program '{}' in library '{}'", name, library_name));
                                    }
                                },
                                crate::library_gui::ProgramSource::Square { x, y, program_index } => {
                                    // Update program in square
                                    if x < crate::sequencer::GRID_WIDTH && y < crate::sequencer::GRID_HEIGHT {
                                        if let Some(square_program) = self.grid.cells[y][x].program.programs.get_mut(program_index) {
                                            *square_program = updated_program;
                                            self.grid.log_to_console(format!("Updated program '{}' in square ({}, {})", name, x, y));
                                        }
                                    }
                                },
                            }
                        }
                        LibraryGuiAction::OpenSquareScript { x, y, program_index } => {
                            // Open the square menu in program editor mode for the specific square
                            if x < crate::sequencer::GRID_WIDTH && y < crate::sequencer::GRID_HEIGHT {
                                let cell = &self.grid.cells[y][x];
                                
                                // Get the program at the specified index
                                if let Some(program) = cell.program.get_program(program_index) {
                                    // Check if this is the default program (name "Default" with only bounce instruction)
                                    let is_default_program = program.name == "Default" && 
                                        program.instructions.len() == 1 &&
                                        matches!(program.instructions[0], crate::square::Instruction::Bounce);
                                    
                                    if is_default_program {
                                        // Default program, start with empty editor
                                        self.grid.square_menu.program_editor = crate::program_editor::ProgramEditor::new_empty();
                                        self.grid.square_menu.editing_program_index = Some(program_index);
                                    } else {
                                        // Use preserved source text if available, otherwise start with empty editor
                                        if let Some(ref source_text) = program.source_text {
                                            self.grid.square_menu.program_editor = crate::program_editor::ProgramEditor::new_with_text(source_text.clone());
                                            self.grid.square_menu.editing_program_index = Some(program_index);
                                        } else {
                                            // No source text available, start with truly empty editor to avoid corrupting the script
                                            self.grid.square_menu.program_editor = crate::program_editor::ProgramEditor::new_truly_empty();
                                            self.grid.square_menu.editing_program_index = Some(program_index);
                                        }
                                    }
                                } else {
                                    // No existing program, start with truly empty editor
                                    self.grid.square_menu.program_editor = crate::program_editor::ProgramEditor::new_truly_empty();
                                    self.grid.square_menu.editing_program_index = None;
                                }
                                
                                // Set the square menu state to program editor mode
                                self.grid.square_menu.state = crate::square_menu::SquareMenuState::ProgramEditor {
                                    square_x: x,
                                    square_y: y,
                                    cursor_line: 0,
                                    cursor_col: 0,
                                };
                                
                                self.grid.library_gui.toggle(); // Close the library GUI
                                self.grid.log_to_console(format!("Opened script editor for square ({}, {}) program {}", x, y, program_index));
                            }
                        }
                        LibraryGuiAction::LoadSample { library_name } => {
                            if let Some(file_path) = FileDialog::new()
                                .add_filter("Audio Files", &["wav", "mp3"])
                                .set_title("Select Audio Sample to Add to Library")
                                .pick_file()
                            {
                                if let Some(path_str) = file_path.to_str() {
                                    // Add sample to the selected library
                                    self.grid.add_sample_to_library(path_str, "library", &library_name);
                                }
                            }
                        }
                        LibraryGuiAction::LoadAutoSample => {
                            if let Some(file_path) = FileDialog::new()
                                .add_filter("Audio Files", &["wav", "mp3"])
                                .set_title("Select Audio Sample to Load Directly into Balls")
                                .pick_file()
                            {
                                if let Some(path_str) = file_path.to_str() {
                                    // Add sample to the auto library for direct ball usage
                                    self.grid.auto_add_sample_to_library(path_str, "ball");
                                }
                            }
                        }
                        LibraryGuiAction::SaveProgramToFile { editor } => {
                            let program_name = editor.get_program_name();
                            let program_text = editor.get_program_text();
                            self.save_program_to_file_with_data(program_name, program_text);
                        }
                        LibraryGuiAction::LoadProgramFromFile => {
                            self.load_program_from_file();
                        }
                        LibraryGuiAction::OpenAudioPlayer { library_name, sample_name } => {
                            // Get the sample from the library
                            if let Some(lib) = self.grid.library_manager.sample_libraries.get(&library_name) {
                                if let Some(_sample) = lib.samples.get(&sample_name) {
                                    // Construct the file path using the sample manager
                                    let sample_path = self.grid.sample_manager.get_local_path(&sample_name);
                                    
                                    // Open the audio player with the sample
                                    if let Err(e) = self.grid.audio_player.open_sample(sample_path.clone(), &mut self.grid.audio_engine) {
                                        self.grid.log_to_console(format!("Failed to open audio player: {}", e));
                                    } else {
                                        self.grid.log_to_console(format!("Opened audio player for {}", sample_name));
                                    }
                                }
                            }
                        }
                        LibraryGuiAction::LoadProgramToSquare { program, square_x, square_y } => {
                            // Load the selected program into the target square
                            if square_x < GRID_WIDTH && square_y < GRID_HEIGHT {
                                self.grid.cells[square_y][square_x].program.add_program(program);
                                let program_count = self.grid.cells[square_y][square_x].program.programs.len();
                                self.grid.cells[square_y][square_x].program.set_active_program(Some(program_count - 1));
                                self.grid.log_to_console(format!("Loaded program into square at ({}, {})", square_x, square_y));
                            }
                        }
                    }}
                return; // Don't process other input while library GUI is open
            }
            
            // Handle audio player input if visible
            if self.grid.audio_player.is_visible() {
                if let Some(action) = self.grid.audio_player.handle_input(&self.input, &mut self.grid.audio_engine) {
                    match action {
                        AudioPlayerAction::Close => {
                            self.grid.audio_player.close();
                            self.grid.log_to_console("Audio player closed".to_string());
                        }
                        AudioPlayerAction::SaveSlice { start, end, name } => {
                            self.grid.log_to_console(format!("Saved audio slice from {:.2} to {:.2} as {}", start, end, name));
                        }
                        AudioPlayerAction::ExportMarkers => {
                            self.grid.log_to_console("Exported audio markers".to_string());
                        }
                    }
                }
                return; // Block all other input while audio player is open
            }
            
            // Normal grid navigation (only when library GUI, audio player, and square menu are not open)
            if !self.grid.square_menu.is_open() {
                if self.input.key_pressed(VirtualKeyCode::Up) {
                    self.grid.cursor.move_up();
                    self.log_cursor_position_if_changed();
                }
                if self.input.key_pressed(VirtualKeyCode::Down) {
                    self.grid.cursor.move_down();
                    self.log_cursor_position_if_changed();
                }
                if self.input.key_pressed(VirtualKeyCode::Left) {
                    self.grid.cursor.move_left();
                    self.log_cursor_position_if_changed();
                }
                if self.input.key_pressed(VirtualKeyCode::Right) {
                    self.grid.cursor.move_right();
                    self.log_cursor_position_if_changed();
                }
            }
            
            // Shape placement / Label editing
            if self.input.key_pressed(VirtualKeyCode::S) {
                let cursor_x = self.grid.cursor.x;
                let cursor_y = self.grid.cursor.y;
                
                // Check if there's already a square at cursor position
                if cursor_x < GRID_WIDTH && cursor_y < GRID_HEIGHT && 
                   self.grid.cells[cursor_y][cursor_x].content == CellContent::Square {
                    // Enter label editing mode
                    self.label_editing_mode = true;
                    self.label_editing_x = cursor_x;
                    self.label_editing_y = cursor_y;
                    self.current_label = self.grid.cells[cursor_y][cursor_x].display_text.clone().unwrap_or_default();
                } else {
                    // Place a new square
                    self.grid.place_square(cursor_x, cursor_y);
                }
            }
            if self.input.key_pressed(VirtualKeyCode::C) {
                 self.grid.place_ball(self.grid.cursor.x, self.grid.cursor.y);
             }
            
            // Stop all sounds (P key)
            if self.input.key_pressed(VirtualKeyCode::P) {
                self.audio_engine.stop_all();
                self.grid.log_to_console("All sounds stopped".to_string());
            }
            
            // Cell clearing
            if self.input.key_pressed(VirtualKeyCode::Delete) || self.input.key_pressed(VirtualKeyCode::Back) {
                self.grid.clear_cell(self.grid.cursor.x, self.grid.cursor.y);
            }
            
            // Context menu for balls or library for empty tiles
            if self.input.key_pressed(VirtualKeyCode::Space) {
                let cursor_x = self.grid.cursor.x;
                let cursor_y = self.grid.cursor.y;
                
                // Check if there's a ball at cursor position
                let has_ball = self.grid.get_ball_at(cursor_x, cursor_y).is_some();
                
                // Check if there's a square at cursor position
                let has_square = cursor_x < GRID_WIDTH && cursor_y < GRID_HEIGHT && 
                                self.grid.cells[cursor_y][cursor_x].content == CellContent::Square;
                
                if has_ball || has_square {
                    // Open context menu for balls or squares
                    self.grid.open_context_menu(cursor_x, cursor_y);
                } else {
                    // Open library for empty tiles
                    self.grid.library_gui.toggle();
                }
            }
            
            // Square programming menu (R key)
            if self.input.key_pressed(VirtualKeyCode::R) {
                // Check if there's a square at the cursor position
                if self.grid.cells[self.grid.cursor.y][self.grid.cursor.x].content == CellContent::Square {
                    self.grid.square_menu.open_square_menu(self.grid.cursor.x, self.grid.cursor.y);
                }
            }

            
            // Console commands (L key for Library)
            if self.input.key_pressed(VirtualKeyCode::L) {
                self.grid.handle_console_command("lib list");
            }
            
            // Quick library commands
            if self.input.key_pressed(VirtualKeyCode::F1) {
                self.grid.handle_console_command("lib functions");
            }
            if self.input.key_pressed(VirtualKeyCode::F2) {
                self.grid.handle_console_command("lib samples");
            }
            if self.input.key_pressed(VirtualKeyCode::F3) {
                self.grid.handle_console_command("lib clear auto");
            }
        }
    }
    
    fn handle_label_editing_input(&mut self) {
        // Handle Return key - move to next line or save and exit
        if self.input.key_pressed(VirtualKeyCode::Return) {
            if self.label_editing_line == 0 {
                // Move to second line
                self.label_editing_line = 1;
                if self.current_label.len() < 4 {
                    // Pad first line to 4 characters with spaces
                    while self.current_label.len() < 4 {
                        self.current_label.push(' ');
                    }
                }
                self.current_label.push('\n'); // Add newline separator
            } else {
                // Save and exit from second line
                if self.label_editing_x < GRID_WIDTH && self.label_editing_y < GRID_HEIGHT {
                    let label = if self.current_label.trim().is_empty() {
                        None
                    } else {
                        Some(self.current_label.clone())
                    };
                    self.grid.cells[self.label_editing_y][self.label_editing_x].display_text = label;
                }
                self.exit_label_editing_mode();
            }
            return;
        }
        
        // Handle Escape key - cancel and exit
        if self.input.key_pressed(VirtualKeyCode::Escape) {
            self.exit_label_editing_mode();
            return;
        }
        
        // Handle Backspace - remove last character
        if self.input.key_pressed(VirtualKeyCode::Back) {
            self.current_label.pop();
            return;
        }
        
        // Handle character input
        let current_line_length = if self.label_editing_line == 0 {
            self.current_label.len()
        } else {
            // Count characters after the newline
            self.current_label.split('\n').last().unwrap_or("").len()
        };
        
        if current_line_length < 4 {
            // Check for letter keys
            for (keycode, ch_lower, ch_upper) in [
                (VirtualKeyCode::A, 'a', 'A'), (VirtualKeyCode::B, 'b', 'B'), (VirtualKeyCode::C, 'c', 'C'),
                (VirtualKeyCode::D, 'd', 'D'), (VirtualKeyCode::E, 'e', 'E'), (VirtualKeyCode::F, 'f', 'F'),
                (VirtualKeyCode::G, 'g', 'G'), (VirtualKeyCode::H, 'h', 'H'), (VirtualKeyCode::I, 'i', 'I'),
                (VirtualKeyCode::J, 'j', 'J'), (VirtualKeyCode::K, 'k', 'K'), (VirtualKeyCode::L, 'l', 'L'),
                (VirtualKeyCode::M, 'm', 'M'), (VirtualKeyCode::N, 'n', 'N'), (VirtualKeyCode::O, 'o', 'O'),
                (VirtualKeyCode::P, 'p', 'P'), (VirtualKeyCode::Q, 'q', 'Q'), (VirtualKeyCode::R, 'r', 'R'),
                (VirtualKeyCode::S, 's', 'S'), (VirtualKeyCode::T, 't', 'T'), (VirtualKeyCode::U, 'u', 'U'),
                (VirtualKeyCode::V, 'v', 'V'), (VirtualKeyCode::W, 'w', 'W'), (VirtualKeyCode::X, 'x', 'X'),
                (VirtualKeyCode::Y, 'y', 'Y'), (VirtualKeyCode::Z, 'z', 'Z'),
            ] {
                if self.input.key_pressed(keycode) {
                    let ch = if self.input.held_shift() { ch_upper } else { ch_lower };
                    self.current_label.push(ch);
                    return;
                }
            }
            
            // Check for number keys
            for (keycode, ch_normal, ch_shift) in [
                (VirtualKeyCode::Key0, '0', ')'), (VirtualKeyCode::Key1, '1', '!'), (VirtualKeyCode::Key2, '2', '@'),
                (VirtualKeyCode::Key3, '3', '#'), (VirtualKeyCode::Key4, '4', '$'), (VirtualKeyCode::Key5, '5', '%'),
                (VirtualKeyCode::Key6, '6', '^'), (VirtualKeyCode::Key7, '7', '&'), (VirtualKeyCode::Key8, '8', '*'),
                (VirtualKeyCode::Key9, '9', '('),
            ] {
                if self.input.key_pressed(keycode) {
                    let ch = if self.input.held_shift() { ch_shift } else { ch_normal };
                    self.current_label.push(ch);
                    return;
                }
            }
            
            // Check for space and common symbols
            if self.input.key_pressed(VirtualKeyCode::Space) {
                self.current_label.push(' ');
            } else if self.input.key_pressed(VirtualKeyCode::Minus) {
                let ch = if self.input.held_shift() { '_' } else { '-' };
                self.current_label.push(ch);
            } else if self.input.key_pressed(VirtualKeyCode::Equals) {
                let ch = if self.input.held_shift() { '+' } else { '=' };
                self.current_label.push(ch);
            }
        }
    }
    
    fn exit_label_editing_mode(&mut self) {
        self.label_editing_mode = false;
        self.current_label.clear();
        self.label_editing_line = 0;
    }
    

    pub fn render(&mut self) -> Result<(), Error> {
        // Calculate delta time for smooth movement
        let now = std::time::Instant::now();
        let delta_time = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;
        
        // Update context menu timing
        self.grid.context_menu.update(delta_time);
        
        // Update balls with delta time
        let triggered_positions = self.grid.update_balls(delta_time);
        
        // Update grid (including audio player)
        self.grid.update(delta_time);
        
        // Play audio samples for triggered positions
        for (_x, _y, ball_index) in triggered_positions {
            if let Some(ball) = self.grid.balls.get(ball_index) {
                if let Some(sample_path) = &ball.sample_path {
                    // Use the first channel (channel 0) for ball samples
                    if let Err(e) = self.audio_engine.play_on_channel(0, sample_path) {
                        log::warn!("Failed to play sample {}: {}", sample_path, e);
                    }
                }
            }
        }
        
        let frame = self.pixels.frame_mut();
        
        // Clear the frame
        for pixel in frame.chunks_exact_mut(4) {
            pixel[0] = 20;  // R
            pixel[1] = 20;  // G
            pixel[2] = 20;  // B
            pixel[3] = 255; // A
        }
        
        // Draw grid lines
        Self::draw_grid_lines_static(frame);
        
        // Draw cells
        for y in 0..GRID_HEIGHT {
            for x in 0..GRID_WIDTH {
                let cell = &self.grid.cells[y][x];
                match cell.content {
                    CellContent::Square => {
                        // Check if this square is being edited
                        let display_text = if self.label_editing_mode && 
                                             self.label_editing_x == x && 
                                             self.label_editing_y == y {
                            // Show current label being typed with cursor on appropriate line
                            if self.label_editing_line == 0 {
                                // Cursor on first line
                                Some(format!("{}_", self.current_label))
                            } else {
                                // Cursor on second line - add cursor after the newline content
                                let parts: Vec<&str> = self.current_label.split('\n').collect();
                                if parts.len() >= 2 {
                                    Some(format!("{}\n{}_", parts[0], parts[1]))
                                } else {
                                    Some(format!("{}_", self.current_label))
                                }
                            }
                        } else {
                            cell.display_text.clone()
                        };
                        Self::draw_square_static(frame, x, y, cell.color, &display_text);
                    }
                    CellContent::Empty => {}
                }
            }
        }
        
        // Draw balls
        for ball in &self.grid.balls {
            let ball_color = Self::get_color_rgb(&ball.color);
            Self::draw_ball_static(frame, ball.x, ball.y, ball_color);
        }
        
        // Draw context menu if open
        self.grid.context_menu.render(frame, &self.grid.balls);
        
        // Draw square menu if open
        self.grid.square_menu.render(frame, &self.grid.cells);
        
        // Draw library GUI if visible
        self.grid.library_gui.render(frame, &self.grid.library_manager, &self.grid.cells, WINDOW_WIDTH, WINDOW_HEIGHT);
        
        // Draw audio player if visible
        self.grid.audio_player.render(frame, WINDOW_WIDTH, WINDOW_HEIGHT);
        
        // Draw cursor only when library GUI, audio player, and square menu are not visible
        if !self.grid.library_gui.is_visible() && !self.grid.audio_player.is_visible() && !self.grid.square_menu.is_open() {
            Self::draw_cursor_static(frame, self.grid.cursor.x, self.grid.cursor.y);
        }
        
        // Draw console area
        Self::draw_console_static(frame, &self.grid.console_messages);
        
        self.pixels.render()
    }
    
    fn draw_console_static(frame: &mut [u8], console_messages: &VecDeque<String>) {
        // Draw console background
        let console_y_start = GRID_AREA_HEIGHT;
        for y in console_y_start..WINDOW_HEIGHT {
            for x in 0..WINDOW_WIDTH {
                let idx = (y * WINDOW_WIDTH + x) * 4;
                if idx + 3 < frame.len() {
                    frame[idx] = 30;     // R - darker background
                    frame[idx + 1] = 30; // G
                    frame[idx + 2] = 30; // B
                    frame[idx + 3] = 255; // A
                }
            }
        }
        
        // Draw console border
        for x in 0..WINDOW_WIDTH {
            let idx = (console_y_start * WINDOW_WIDTH + x) * 4;
            if idx + 3 < frame.len() {
                frame[idx] = 100;     // R - border color
                frame[idx + 1] = 100; // G
                frame[idx + 2] = 100; // B
                frame[idx + 3] = 255; // A
            }
        }
        
        // Draw console messages
        for (i, message) in console_messages.iter().enumerate() {
            let text_y = console_y_start + 10 + i * 14;
            if text_y + 12 < WINDOW_HEIGHT {
                Self::draw_menu_text(frame, message, 5, text_y, [200, 200, 200], false);
            }
        }
    }
    
    fn draw_cursor_coordinates(frame: &mut [u8], cursor_x: usize, cursor_y: usize) {
        let coord_text = format!("({}, {})", cursor_x, cursor_y);
        // Position coordinates in the black area above grid (0,0)
        // Grid (0,0) starts at pixel (0,0), so we position the text just above it
        Self::draw_menu_text(frame, &coord_text, 5, 25, [255, 255, 255], false); // White text above grid (0,0)
    }
    
    fn draw_menu_text(frame: &mut [u8], text: &str, x: usize, y: usize, color: [u8; 3], selected: bool) {
        font::draw_text(frame, text, x, y, color, selected, WINDOW_WIDTH);
    }
    


    fn draw_grid_lines_static(frame: &mut [u8]) {
        let grid_color = [60, 60, 60];
        
        // Vertical lines
        for x in 0..=GRID_WIDTH {
            let pixel_x = x * CELL_SIZE;
            if pixel_x < WINDOW_WIDTH as usize {
                for y in 0..WINDOW_HEIGHT as usize {
                    let index = (y * WINDOW_WIDTH as usize + pixel_x) * 4;
                    if index + 2 < frame.len() {
                        frame[index] = grid_color[0];
                        frame[index + 1] = grid_color[1];
                        frame[index + 2] = grid_color[2];
                    }
                }
            }
        }
        
        // Horizontal lines
        for y in 0..=GRID_HEIGHT {
            let pixel_y = y * CELL_SIZE;
            if pixel_y < WINDOW_HEIGHT as usize {
                for x in 0..WINDOW_WIDTH as usize {
                    let index = (pixel_y * WINDOW_WIDTH as usize + x) * 4;
                    if index + 2 < frame.len() {
                        frame[index] = grid_color[0];
                        frame[index + 1] = grid_color[1];
                        frame[index + 2] = grid_color[2];
                    }
                }
            }
        }
    }
    
    fn draw_square_static(frame: &mut [u8], grid_x: usize, grid_y: usize, color: [u8; 3], display_text: &Option<String>) {
        let start_x = grid_x * CELL_SIZE + 2;
        let start_y = grid_y * CELL_SIZE + 2;
        let end_x = (grid_x + 1) * CELL_SIZE - 2;
        let end_y = (grid_y + 1) * CELL_SIZE - 2;
        
        for y in start_y..end_y {
            for x in start_x..end_x {
                if x < WINDOW_WIDTH as usize && y < WINDOW_HEIGHT as usize {
                    let index = (y * WINDOW_WIDTH as usize + x) * 4;
                    if index + 2 < frame.len() {
                        frame[index] = color[0];
                        frame[index + 1] = color[1];
                        frame[index + 2] = color[2];
                    }
                }
            }
        }
        
        // Draw display text if present
        if let Some(text) = display_text {
            let text_x = start_x + 4;
            let text_y = start_y + 4;
            
            // Handle multi-line text by splitting on newlines
            let lines: Vec<&str> = text.split('\n').collect();
            for (line_index, line) in lines.iter().enumerate() {
                let line_y = text_y + (line_index * 12); // 12 pixels per line (font height)
                // Only draw if the line fits within the cell
                if line_y + 12 <= end_y {
                    font::draw_text(frame, line, text_x, line_y, [255, 255, 255], false, WINDOW_WIDTH);
                }
            }
        }
    }
    
    fn draw_circle_static(frame: &mut [u8], grid_x: usize, grid_y: usize, color: [u8; 3]) {
        let center_x = grid_x * CELL_SIZE + CELL_SIZE / 2;
        let center_y = grid_y * CELL_SIZE + CELL_SIZE / 2;
        let radius = (CELL_SIZE / 2 - 2) as f32;
        
        let start_x = grid_x * CELL_SIZE + 2;
        let start_y = grid_y * CELL_SIZE + 2;
        let end_x = (grid_x + 1) * CELL_SIZE - 2;
        let end_y = (grid_y + 1) * CELL_SIZE - 2;
        
        for y in start_y..end_y {
            for x in start_x..end_x {
                let dx = x as f32 - center_x as f32;
                let dy = y as f32 - center_y as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                
                if distance <= radius && x < WINDOW_WIDTH as usize && y < WINDOW_HEIGHT as usize {
                    let index = (y * WINDOW_WIDTH as usize + x) * 4;
                    if index + 2 < frame.len() {
                        frame[index] = color[0];
                        frame[index + 1] = color[1];
                        frame[index + 2] = color[2];
                    }
                }
            }
        }
    }
    
    fn draw_cursor_static(frame: &mut [u8], cursor_x: usize, cursor_y: usize) {
        let cursor_color = [255, 255, 0]; // Yellow cursor
        let x = cursor_x * CELL_SIZE;
        let y = cursor_y * CELL_SIZE;
        
        // Draw cursor border
        for i in 0..CELL_SIZE {
            // Top border
            if x + i < WINDOW_WIDTH as usize && y < WINDOW_HEIGHT as usize {
                let index = (y * WINDOW_WIDTH as usize + x + i) * 4;
                if index + 2 < frame.len() {
                    frame[index] = cursor_color[0];
                    frame[index + 1] = cursor_color[1];
                    frame[index + 2] = cursor_color[2];
                }
            }
            
            // Bottom border
            if x + i < WINDOW_WIDTH as usize && y + CELL_SIZE - 1 < WINDOW_HEIGHT as usize {
                let index = ((y + CELL_SIZE - 1) * WINDOW_WIDTH as usize + x + i) * 4;
                if index + 2 < frame.len() {
                    frame[index] = cursor_color[0];
                    frame[index + 1] = cursor_color[1];
                    frame[index + 2] = cursor_color[2];
                }
            }
            
            // Left border
            if x < WINDOW_WIDTH as usize && y + i < WINDOW_HEIGHT as usize {
                let index = ((y + i) * WINDOW_WIDTH as usize + x) * 4;
                if index + 2 < frame.len() {
                    frame[index] = cursor_color[0];
                    frame[index + 1] = cursor_color[1];
                    frame[index + 2] = cursor_color[2];
                }
            }
            
            // Right border
            if x + CELL_SIZE - 1 < WINDOW_WIDTH as usize && y + i < WINDOW_HEIGHT as usize {
                let index = ((y + i) * WINDOW_WIDTH as usize + x + CELL_SIZE - 1) * 4;
                if index + 2 < frame.len() {
                    frame[index] = cursor_color[0];
                    frame[index + 1] = cursor_color[1];
                    frame[index + 2] = cursor_color[2];
                }
            }
        }
    }
    
    fn get_color_rgb(color_name: &str) -> [u8; 3] {
        match color_name {
            "Red" => [255, 0, 0],
            "Green" => [0, 255, 0],
            "Blue" => [0, 0, 255],
            "Yellow" => [255, 255, 0],
            "Cyan" => [0, 255, 255],
            "Magenta" => [255, 0, 255],
            "White" => [255, 255, 255],
            "Orange" => [255, 165, 0],
            _ => [255, 255, 255], // Default to white
        }
    }
    
    fn draw_ball_static(frame: &mut [u8], ball_x: f32, ball_y: f32, color: [u8; 3]) {
        let pixel_x = ball_x * CELL_SIZE as f32;
        let pixel_y = ball_y * CELL_SIZE as f32;
        let center_x = pixel_x;
        let center_y = pixel_y;
        let radius = CELL_SIZE as f32 / 4.0;
        
        let start_x = (pixel_x as usize).saturating_sub(CELL_SIZE / 2);
        let start_y = (pixel_y as usize).saturating_sub(CELL_SIZE / 2);
        let end_x = ((pixel_x + CELL_SIZE as f32) as usize).min(WINDOW_WIDTH);
        let end_y = ((pixel_y + CELL_SIZE as f32) as usize).min(WINDOW_HEIGHT);
        
        // Draw ball with specified color
        for y in start_y..end_y {
            for x in start_x..end_x {
                if x < WINDOW_WIDTH && y < WINDOW_HEIGHT {
                    let dx = x as f32 - center_x;
                    let dy = y as f32 - center_y;
                    if dx * dx + dy * dy <= radius * radius {
                        let index = (y * WINDOW_WIDTH + x) * 4;
                        if index + 3 < frame.len() {
                            frame[index] = color[0];     // R
                            frame[index + 1] = color[1]; // G
                            frame[index + 2] = color[2]; // B
                            frame[index + 3] = 0xff;     // A
                        }
                    }
                }
            }
        }
    }
    

    

    

    

    
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if let Err(err) = self.pixels.resize_surface(new_size.width, new_size.height) {
            log::error!("Failed to resize surface: {}", err);
        }
    }
    
    fn open_file_dialog_for_ball(&mut self, ball_index: usize) {
        if let Some(file_path) = FileDialog::new()
            .add_filter("Audio Files", &["wav", "mp3"])
            .set_title("Select Audio Sample")
            .pick_file()
        {
            if let Some(path_str) = file_path.to_str() {
                self.grid.set_ball_sample(ball_index, path_str.to_string());
                println!("Selected audio file: {}", path_str);
            }
        }
    }
    
    fn add_sample_to_library_for_ball(&mut self, ball_index: usize) {
        if let Some(file_path) = FileDialog::new()
            .add_filter("Audio Files", &["wav", "mp3"])
            .set_title("Select Audio Sample to Add to Library")
            .pick_file()
        {
            if let Some(path_str) = file_path.to_str() {
                // Add sample to library without setting it to the ball
                self.grid.auto_add_sample_to_library(path_str, "ball");
                println!("Added audio file to library: {}", path_str);
            }
        }
    }

    fn save_program_to_file_with_data(&mut self, program_name: String, program_text: Vec<String>) {
        let default_filename = format!("{}.cant", program_name);
        
        if let Some(file_path) = FileDialog::new()
            .add_filter("Canticle Programs", &["cant"])
            .set_title("Save Program")
            .set_file_name(&default_filename)
            .save_file()
        {
            if let Some(path_str) = file_path.to_str() {
                let program_text_str = program_text.join("\n");
                match std::fs::write(&file_path, program_text_str) {
                    Ok(_) => {
                        self.grid.log_to_console(format!("Program saved to: {}", path_str));
                    }
                    Err(e) => {
                        self.grid.log_to_console(format!("Failed to save program: {}", e));
                    }
                }
            }
        }
    }

    fn load_program_from_file(&mut self) {
        if let Some(file_path) = FileDialog::new()
            .add_filter("Canticle Programs", &["cant"])
            .set_title("Load Program")
            .pick_file()
        {
            if let Some(path_str) = file_path.to_str() {
                match std::fs::read_to_string(&file_path) {
                    Ok(content) => {
                        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                        
                        // Update the appropriate editor based on current context
                        if self.grid.library_gui.is_visible() {
                            // Update library GUI editor
                            if let Some(editor) = self.grid.library_gui.get_current_editor_mut() {
                                *editor = crate::program_editor::ProgramEditor::new_with_text(lines);
                            }
                        } else if self.grid.square_menu.is_open() {
                            // Update square menu editor
                            self.grid.square_menu.program_editor = crate::program_editor::ProgramEditor::new_with_text(lines);
                        }
                        
                        self.grid.log_to_console(format!("Program loaded from: {}", path_str));
                    }
                    Err(e) => {
                        self.grid.log_to_console(format!("Failed to load program: {}", e));
                    }
                }
            }
        }
    }
}

pub async fn run_sequencer(audio_engine: AudioEngine) -> Result<(), Error> {
    
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Canticle")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };
    
    let mut sequencer_ui = SequencerUI::new(&window, audio_engine)?;
    
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::RedrawRequested(_) => {
                if let Err(err) = sequencer_ui.render() {
                    log::error!("Render error: {}", err);
                    *control_flow = ControlFlow::Exit;
                }
            }
            
            Event::WindowEvent { ref event, .. } => {
                match event {
                    winit::event::WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    winit::event::WindowEvent::Resized(new_size) => {
                        sequencer_ui.resize(*new_size);
                    }
                    winit::event::WindowEvent::ReceivedCharacter(ch) => {
                        // Filter out control characters (backspace, delete, etc.)
                        if ch.is_control() {
                            return;
                        }
                        
                        // Pass character directly to program editor
                        if sequencer_ui.grid.square_menu.is_open() {
                            if let crate::square_menu::SquareMenuState::ProgramEditor { .. } = sequencer_ui.grid.square_menu.state {
                                sequencer_ui.grid.square_menu.program_editor.insert_character(*ch);
                            }
                        } else if sequencer_ui.grid.library_gui.is_visible() {
                            if let Some(editor) = sequencer_ui.grid.library_gui.get_current_editor_mut() {
                                editor.insert_character(*ch);
                            }
                        }
                    }
                    _ => {}
                }
            }
            
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            
            _ => {}
        }
        
        sequencer_ui.handle_input(&event);
    });
}