use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use rfd::FileDialog;

use crate::ball::{Ball, Direction};
use crate::square::{Cell, CellContent, ProgramAction};
use crate::context_menu::{ContextMenu, ContextMenuAction};
use crate::square_menu::{SquareContextMenu, SquareMenuAction};
use crate::programmer::ProgramExecutor;
use crate::audio_engine::AudioEngine;


pub const GRID_WIDTH: usize = 16;
pub const GRID_HEIGHT: usize = 12;
const CELL_SIZE: usize = 40;
const WINDOW_WIDTH: usize = GRID_WIDTH * CELL_SIZE;
const WINDOW_HEIGHT: usize = GRID_HEIGHT * CELL_SIZE;

pub struct Cursor {
    pub x: usize,
    pub y: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { x: 0, y: 0 }
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
}

impl SequencerGrid {
    pub fn new() -> Self {
        Self {
            cells: std::array::from_fn(|_| std::array::from_fn(|_| Cell::default())),
            cursor: Cursor::new(),
            balls: Vec::new(),
            context_menu: ContextMenu::new(),
            square_menu: SquareContextMenu::new(),
            program_executor: ProgramExecutor::new(),
            selected_ball: None,
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
            let ball = Ball::new(x, y);
            self.balls.push(ball);
        }
    }
    
    pub fn clear_cell(&mut self, x: usize, y: usize) {
        if x < GRID_WIDTH && y < GRID_HEIGHT {
            self.cells[y][x].clear();
            
            // Remove any ball at this position
            self.balls.retain(|ball| !(ball.original_x == x as f32 && ball.original_y == y as f32));
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
            self.balls[ball_index].set_sample(sample_path);
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
            // If any balls are active, reset all to original positions and stop them
            self.reset_balls_to_origin();
        } else {
            // If no balls are active, start all balls moving
            for ball in &mut self.balls {
                ball.activate();
            }
        }
    }
    
    pub fn update_balls(&mut self, delta_time: f32) -> Vec<(usize, usize, usize)> { // Returns (x, y, ball_index) where samples should be triggered
        let mut triggered_positions = Vec::new();
        
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
                        // Execute square program if it has one
                        let square_program = &self.cells[grid_y][grid_x].program;
                        if !square_program.programs.is_empty() {
                            if let Some(active_program_index) = square_program.active_program {
                                if let Some(program) = square_program.programs.get(active_program_index) {
                                    let actions = self.program_executor.execute_on_collision(
                                        program, ball, grid_x, grid_y
                                    );
                                    
                                    // Apply program actions to the ball
                                    for action in actions {
                                        match action {
                                            ProgramAction::SetSpeed(speed) => {
                                                ball.speed = speed.max(0.1); // Ensure minimum speed
                                            }
                                            ProgramAction::SetDirection(direction) => {
                                                ball.direction = direction;
                                            }
                                            ProgramAction::Bounce => {
                                                ball.reverse_direction();
                                            }
                                            ProgramAction::Stop => {
                                                ball.active = false;
                                            }
                                            _ => {} // Handle other actions as needed
                                        }
                                    }
                                }
                            }
                        } else {
                            // Default behavior: reverse direction
                            ball.reverse_direction();
                        }
                        
                        // Move ball back to previous position to prevent overlap
                        ball.x = old_x;
                        ball.y = old_y;
                        ball.last_grid_x = old_x.floor() as usize;
                        ball.last_grid_y = old_y.floor() as usize;
                        triggered_positions.push((grid_x, grid_y, ball_index));
                        break; // Only trigger once per update
                    }
                }
            }
        }
        
        triggered_positions
    }
}

pub struct SequencerUI {
    grid: SequencerGrid,
    pixels: Pixels,
    input: WinitInputHelper,
    last_update: std::time::Instant,
    audio_engine: AudioEngine,
}

impl SequencerUI {
    pub fn new(window: &winit::window::Window, audio_engine: AudioEngine) -> Result<Self, Error> {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window);
        let pixels = Pixels::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32, surface_texture)?;
        
        Ok(Self {
            grid: SequencerGrid::new(),
            pixels,
            input: WinitInputHelper::new(),
            last_update: std::time::Instant::now(),
            audio_engine,
        })
    }
    
    pub fn handle_input(&mut self, event: &Event<()>) {
        if self.input.update(event) {
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
                 }
                 return;
             }
            
            if self.grid.context_menu.is_open() {
                return;
            }

            // Handle square menu input
            if self.grid.square_menu.is_open() {
                if let Some(action) = self.grid.square_menu.handle_input(&self.input) {
                    match action {
                        SquareMenuAction::SaveProgram { square_x, square_y, program } => {
                            if square_x < GRID_WIDTH && square_y < GRID_HEIGHT {
                                self.grid.cells[square_y][square_x].program.add_program(program);
                                // Set the newly added program as active
                                let program_count = self.grid.cells[square_y][square_x].program.programs.len();
                                self.grid.cells[square_y][square_x].program.set_active_program(Some(program_count - 1));
                            }
                        }
                        SquareMenuAction::TestProgram { square_x, square_y } => {
                            // For testing, we could simulate a ball collision
                            println!("Testing program for square at ({}, {})", square_x, square_y);
                        }
                        SquareMenuAction::ClearPrograms { square_x, square_y } => {
                            if square_x < GRID_WIDTH && square_y < GRID_HEIGHT {
                                self.grid.cells[square_y][square_x].program.programs.clear();
                                self.grid.cells[square_y][square_x].program.set_active_program(None);
                            }
                        }
                    }
                }
                return; // Don't process other input while square menu is open
            }

            
            // Normal grid navigation
            if self.input.key_pressed(VirtualKeyCode::Up) {
                self.grid.cursor.move_up();
            }
            if self.input.key_pressed(VirtualKeyCode::Down) {
                self.grid.cursor.move_down();
            }
            if self.input.key_pressed(VirtualKeyCode::Left) {
                self.grid.cursor.move_left();
            }
            if self.input.key_pressed(VirtualKeyCode::Right) {
                self.grid.cursor.move_right();
            }
            
            // Shape placement
            if self.input.key_pressed(VirtualKeyCode::S) {
                self.grid.place_square(self.grid.cursor.x, self.grid.cursor.y);
            }
            if self.input.key_pressed(VirtualKeyCode::C) {
                 self.grid.place_ball(self.grid.cursor.x, self.grid.cursor.y);
             }
            
            // Toggle ball movement (P key)
            if self.input.key_pressed(VirtualKeyCode::P) {
                self.grid.toggle_all_balls();
            }
            
            // Cell clearing
            if self.input.key_pressed(VirtualKeyCode::Delete) || self.input.key_pressed(VirtualKeyCode::Back) {
                self.grid.clear_cell(self.grid.cursor.x, self.grid.cursor.y);
            }
            
            // Context menu for balls
            if self.input.key_pressed(VirtualKeyCode::Space) {
                self.grid.open_context_menu(self.grid.cursor.x, self.grid.cursor.y);
            }
            
            // Square programming menu (R key)
            if self.input.key_pressed(VirtualKeyCode::R) {
                // Check if there's a square at the cursor position
                if self.grid.cells[self.grid.cursor.y][self.grid.cursor.x].content == CellContent::Square {
                    self.grid.square_menu.open_square_menu(self.grid.cursor.x, self.grid.cursor.y);
                }
            }
        }
    }
    
    pub fn render(&mut self) -> Result<(), Error> {
        // Calculate delta time for smooth movement
        let now = std::time::Instant::now();
        let delta_time = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;
        
        // Update balls with delta time
        let triggered_positions = self.grid.update_balls(delta_time);
        
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
                    CellContent::Square => Self::draw_square_static(frame, x, y, cell.color),

                    CellContent::Empty => {}
                }
            }
        }
        
        // Draw balls
        for ball in &self.grid.balls {
            let ball_color = Self::get_color_rgb(&ball.color);
            Self::draw_ball_static(frame, ball.x, ball.y, ball_color);
        }
        
        // Draw cursor
        Self::draw_cursor_static(frame, self.grid.cursor.x, self.grid.cursor.y);
        
        // Draw context menu if open
        self.grid.context_menu.render(frame, &self.grid.balls);
        
        // Draw square menu if open
        self.grid.square_menu.render(frame, &self.grid.cells);
        
        self.pixels.render()
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
    
    fn draw_square_static(frame: &mut [u8], grid_x: usize, grid_y: usize, color: [u8; 3]) {
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
}

pub async fn run_sequencer(audio_engine: AudioEngine) -> Result<(), Error> {
    
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Canticlec Churn - Music Sequencer")
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