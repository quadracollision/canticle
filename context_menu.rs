use winit::event::VirtualKeyCode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContextMenuState {
    None,
    BallMenu { ball_index: usize, selected_option: usize },
    BallDirection { ball_index: usize, selected_option: usize },
    BallSpeed { ball_index: usize, speed: f32 }, // speed in grid units per second
    BallRelativeSpeed { ball_index: usize, selected_ball: usize, speed_ratio: f32 },
    BallColor { ball_index: usize, selected_option: usize },
}

pub struct ContextMenu {
    pub state: ContextMenuState,
}

const BALL_MENU_OPTIONS: &[&str] = &["Direction", "Speed", "Relative Speed", "Sample", "Color"];
const DIRECTION_OPTIONS: &[&str] = &["Up", "Down", "Left", "Right", "Up-Left", "Up-Right", "Down-Left", "Down-Right"];
const MIN_SPEED: f32 = 0.5;
const MAX_SPEED: f32 = 10.0;
const SPEED_STEP: f32 = 0.1;

const COLOR_OPTIONS: &[&str] = &["Red", "Green", "Blue", "Yellow", "Cyan", "Magenta", "White", "Orange"];
const RELATIVE_SPEED_RATIOS: &[f32] = &[1.0/16.0, 1.0/8.0, 1.0/4.0, 1.0/2.0, 1.0, 2.0, 4.0, 8.0, 16.0];
const RELATIVE_SPEED_LABELS: &[&str] = &["1/16x", "1/8x", "1/4x", "1/2x", "1x", "2x", "4x", "8x", "16x"];

impl ContextMenu {
    pub fn new() -> Self {
        ContextMenu {
            state: ContextMenuState::None,
        }
    }

    pub fn open_ball_menu(&mut self, ball_index: usize) {
        self.state = ContextMenuState::BallMenu { ball_index, selected_option: 0 };
    }
    
    pub fn open_speed_menu(&mut self, ball_index: usize, current_speed: f32) {
        self.state = ContextMenuState::BallSpeed { ball_index, speed: current_speed };
    }

    pub fn close(&mut self) {
        self.state = ContextMenuState::None;
    }

    pub fn is_open(&self) -> bool {
        !matches!(self.state, ContextMenuState::None)
    }

    pub fn handle_input(&mut self, input: &winit_input_helper::WinitInputHelper, balls: &[Ball]) -> Option<ContextMenuAction> {
        match self.state {
            ContextMenuState::BallMenu { ball_index, selected_option } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    self.close();
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Up) {
                    let new_option = if selected_option == 0 { BALL_MENU_OPTIONS.len() - 1 } else { selected_option - 1 };
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Down) {
                    let new_option = (selected_option + 1) % BALL_MENU_OPTIONS.len();
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Space) {
                    // Check for Shift+Space on Sample option to add to library
                    if selected_option == 3 && (input.held_shift()) {
                        self.close();
                        return Some(ContextMenuAction::AddSampleToLibrary { ball_index });
                    }
                    
                    match selected_option {
                        0 => self.state = ContextMenuState::BallDirection { ball_index, selected_option: 0 },
                        1 => {
                            // Initialize with current ball speed
                            let current_speed = balls.get(ball_index).map(|b| b.speed).unwrap_or(2.0);
                            self.state = ContextMenuState::BallSpeed { ball_index, speed: current_speed };
                        },
                        2 => {
                            // Relative Speed - find first other ball or default to ball 0
                            let selected_ball = if ball_index == 0 && balls.len() > 1 { 1 } else { 0 };
                            self.state = ContextMenuState::BallRelativeSpeed { ball_index, selected_ball, speed_ratio: 1.0 };
                        },
                        3 => {
                            // Sample - directly open file dialog
                            self.close();
                            return Some(ContextMenuAction::OpenFileDialog { ball_index });
                        },
                        4 => self.state = ContextMenuState::BallColor { ball_index, selected_option: 0 },
                        _ => {}
                    }
                    return None;
                }
                None
            }
            ContextMenuState::BallDirection { ball_index, selected_option } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 0 };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Up) {
                    let new_option = if selected_option == 0 { DIRECTION_OPTIONS.len() - 1 } else { selected_option - 1 };
                    self.state = ContextMenuState::BallDirection { ball_index, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Down) {
                    let new_option = (selected_option + 1) % DIRECTION_OPTIONS.len();
                    self.state = ContextMenuState::BallDirection { ball_index, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Space) {
                    let direction = match selected_option {
                        0 => Direction::Up,
                        1 => Direction::Down,
                        2 => Direction::Left,
                        3 => Direction::Right,
                        4 => Direction::UpLeft,
                        5 => Direction::UpRight,
                        6 => Direction::DownLeft,
                        7 => Direction::DownRight,
                        _ => Direction::Up,
                    };
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 0 };
                    return Some(ContextMenuAction::SetDirection { ball_index, direction });
                }
                None
            }
            ContextMenuState::BallSpeed { ball_index, speed } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 1 };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Left) {
                    let new_speed = (speed - SPEED_STEP).max(MIN_SPEED);
                    self.state = ContextMenuState::BallSpeed { ball_index, speed: new_speed };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Right) {
                    let new_speed = (speed + SPEED_STEP).min(MAX_SPEED);
                    self.state = ContextMenuState::BallSpeed { ball_index, speed: new_speed };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Space) {
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 1 };
                    return Some(ContextMenuAction::SetSpeed { ball_index, speed });
                }
                None
            }
            ContextMenuState::BallRelativeSpeed { ball_index, selected_ball, speed_ratio } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 2 };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Up) {
                    // Cycle through available balls
                    let new_selected = if selected_ball == 0 { balls.len().saturating_sub(1) } else { selected_ball - 1 };
                    self.state = ContextMenuState::BallRelativeSpeed { ball_index, selected_ball: new_selected, speed_ratio };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Down) {
                    // Cycle through available balls
                    let new_selected = (selected_ball + 1) % balls.len().max(1);
                    self.state = ContextMenuState::BallRelativeSpeed { ball_index, selected_ball: new_selected, speed_ratio };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Left) {
                    // Decrease speed ratio
                    let current_index = RELATIVE_SPEED_RATIOS.iter().position(|&r| (r - speed_ratio).abs() < 0.001).unwrap_or(4);
                    let new_index = if current_index == 0 { RELATIVE_SPEED_RATIOS.len() - 1 } else { current_index - 1 };
                    self.state = ContextMenuState::BallRelativeSpeed { ball_index, selected_ball, speed_ratio: RELATIVE_SPEED_RATIOS[new_index] };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Right) {
                    // Increase speed ratio
                    let current_index = RELATIVE_SPEED_RATIOS.iter().position(|&r| (r - speed_ratio).abs() < 0.001).unwrap_or(4);
                    let new_index = (current_index + 1) % RELATIVE_SPEED_RATIOS.len();
                    self.state = ContextMenuState::BallRelativeSpeed { ball_index, selected_ball, speed_ratio: RELATIVE_SPEED_RATIOS[new_index] };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Space) {
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 2 };
                    if let Some(reference_ball) = balls.get(selected_ball) {
                        let new_speed = reference_ball.speed * speed_ratio;
                        return Some(ContextMenuAction::SetSpeed { ball_index, speed: new_speed.clamp(MIN_SPEED, MAX_SPEED) });
                    }
                }
                None
            }

            ContextMenuState::BallColor { ball_index, selected_option } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 4 };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Up) {
                    let new_option = if selected_option == 0 { COLOR_OPTIONS.len() - 1 } else { selected_option - 1 };
                    self.state = ContextMenuState::BallColor { ball_index, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Down) {
                    let new_option = (selected_option + 1) % COLOR_OPTIONS.len();
                    self.state = ContextMenuState::BallColor { ball_index, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Space) {
                    let color = COLOR_OPTIONS[selected_option].to_string();
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 4 };
                    return Some(ContextMenuAction::SetColor { ball_index, color });
                }
                None
            }
            ContextMenuState::None => None,
        }
    }

    pub fn render(&self, frame: &mut [u8], balls: &[Ball]) {
        match self.state {
            ContextMenuState::BallMenu { ball_index, selected_option } => {
                if let Some(ball) = balls.get(ball_index) {
                    let (ball_x, ball_y) = ball.get_grid_position();
                    draw_ball_menu(frame, ball_x, ball_y, selected_option, ball, ball_index);
                }
            }
            ContextMenuState::BallDirection { ball_index, selected_option } => {
                if let Some(ball) = balls.get(ball_index) {
                    let (ball_x, ball_y) = ball.get_grid_position();
                    draw_direction_menu(frame, ball_x, ball_y, selected_option);
                }
            }
            ContextMenuState::BallSpeed { ball_index, speed } => {
                if let Some(ball) = balls.get(ball_index) {
                    let (ball_x, ball_y) = ball.get_grid_position();
                    draw_speed_slider(frame, ball_x, ball_y, speed);
                }
            }
            ContextMenuState::BallRelativeSpeed { ball_index, selected_ball, speed_ratio } => {
                if let Some(ball) = balls.get(ball_index) {
                    let (ball_x, ball_y) = ball.get_grid_position();
                    draw_relative_speed_menu(frame, ball_x, ball_y, selected_ball, speed_ratio, balls);
                }
            }

            ContextMenuState::BallColor { ball_index, selected_option } => {
                if let Some(ball) = balls.get(ball_index) {
                    let (ball_x, ball_y) = ball.get_grid_position();
                    draw_color_menu(frame, ball_x, ball_y, selected_option);
                }
            }
            ContextMenuState::None => {}
        }
    }
}

#[derive(Debug)]
pub enum ContextMenuAction {
    SetDirection { ball_index: usize, direction: Direction },
    SetSpeed { ball_index: usize, speed: f32 },
    SetSample { ball_index: usize, sample: String },
    SetColor { ball_index: usize, color: String },
    OpenFileDialog { ball_index: usize },
    AddSampleToLibrary { ball_index: usize },
}

// Import types from modules
use crate::ball::{Ball, Direction};
use crate::font;

// Constants for drawing
const CELL_SIZE: usize = 40;
const WINDOW_WIDTH: usize = 640;
const WINDOW_HEIGHT: usize = 480;

fn draw_menu_background(frame: &mut [u8], x: usize, y: usize, width: usize, height: usize) {
    for dy in 0..height {
        for dx in 0..width {
            let px = x + dx;
            let py = y + dy;
            if px < WINDOW_WIDTH && py < WINDOW_HEIGHT {
                let idx = (py * WINDOW_WIDTH + px) * 4;
                frame[idx] = 40;     // R
                frame[idx + 1] = 40; // G
                frame[idx + 2] = 40; // B
                frame[idx + 3] = 255; // A
            }
        }
    }
}

fn draw_menu_border(frame: &mut [u8], x: usize, y: usize, width: usize, height: usize) {
    for dy in 0..height {
        for dx in 0..width {
            let px = x + dx;
            let py = y + dy;
            if px < WINDOW_WIDTH && py < WINDOW_HEIGHT {
                if dx == 0 || dx == width - 1 || dy == 0 || dy == height - 1 {
                    let idx = (py * WINDOW_WIDTH + px) * 4;
                    frame[idx] = 255;     // R
                    frame[idx + 1] = 255; // G
                    frame[idx + 2] = 255; // B
                    frame[idx + 3] = 255; // A
                }
            }
        }
    }
}

fn draw_text(frame: &mut [u8], text: &str, x: usize, y: usize, color: [u8; 3], selected: bool) {
    font::draw_text(frame, text, x, y, color, selected, WINDOW_WIDTH);
}



fn draw_ball_menu(frame: &mut [u8], ball_x: usize, ball_y: usize, selected_option: usize, ball: &Ball, ball_index: usize) {
    let menu_width = CELL_SIZE * 6; // Increased width to accommodate sample names
    let menu_height = CELL_SIZE * 4; // Increased height to accommodate ball info
    
    // Position menu to the right of the ball, but keep it on screen
    let mut menu_x = ball_x * CELL_SIZE + CELL_SIZE;
    let mut menu_y = ball_y * CELL_SIZE;
    
    // Adjust if menu would go off screen
    if menu_x + menu_width > WINDOW_WIDTH {
        menu_x = ball_x * CELL_SIZE - menu_width;
    }
    if menu_y + menu_height > WINDOW_HEIGHT {
        menu_y = WINDOW_HEIGHT - menu_height;
    }
    
    draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
    draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);
    
    // Draw ball name and color at the top
    let ball_name = format!("Ball {}", ball_index);
    let ball_info = format!("{} ({})", ball_name, ball.color);
    draw_text(frame, &ball_info, menu_x + 5, menu_y + 5, [255, 255, 255], false);
    
    // Draw separator line
    let separator_y = menu_y + 25;
    for x in (menu_x + 5)..(menu_x + menu_width - 5) {
        if x < WINDOW_WIDTH && separator_y < WINDOW_HEIGHT {
            let idx = (separator_y * WINDOW_WIDTH + x) * 4;
            if idx + 3 < frame.len() {
                frame[idx] = 150;     // R
                frame[idx + 1] = 150; // G
                frame[idx + 2] = 150; // B
                frame[idx + 3] = 255; // A
            }
        }
    }
    
    // Draw menu options (offset down to make room for ball info)
    for (i, option) in BALL_MENU_OPTIONS.iter().enumerate() {
        let text_x = menu_x + 5;
        let text_y = menu_y + 35 + i * 20; // Offset by 35 instead of 5
        let is_selected = i == selected_option;
        
        // Special handling for Sample option to show current sample
        if i == 3 && option == &"Sample" { // Sample is at index 3
            let display_text = if let Some(ref sample_path) = ball.sample_path {
                // Extract filename from path
                let filename = std::path::Path::new(sample_path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                format!("Sample ({})", filename)
            } else {
                "Sample".to_string()
            };
            draw_text(frame, &display_text, text_x, text_y, [200, 200, 200], is_selected);
        } else {
            draw_text(frame, option, text_x, text_y, [200, 200, 200], is_selected);
        }
    }
}

fn draw_direction_menu(frame: &mut [u8], ball_x: usize, ball_y: usize, selected_option: usize) {
    let menu_width = CELL_SIZE * 5;
    let menu_height = CELL_SIZE * 8;
    
    // Position menu to the right of the ball, but keep it on screen
    let mut menu_x = ball_x * CELL_SIZE + CELL_SIZE;
    let mut menu_y = ball_y * CELL_SIZE;
    
    // Adjust if menu would go off screen
    if menu_x + menu_width > WINDOW_WIDTH {
        menu_x = ball_x * CELL_SIZE - menu_width;
    }
    if menu_y + menu_height > WINDOW_HEIGHT {
        menu_y = WINDOW_HEIGHT - menu_height;
    }
    
    draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
    draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);
    
    // Draw direction options
    for (i, option) in DIRECTION_OPTIONS.iter().enumerate() {
        let text_x = menu_x + 5;
        let text_y = menu_y + 5 + i * 18;
        let is_selected = i == selected_option;
        draw_text(frame, option, text_x, text_y, [200, 200, 200], is_selected);
    }
}

fn draw_speed_slider(frame: &mut [u8], ball_x: usize, ball_y: usize, speed: f32) {
    let menu_width = 200;
    let menu_height = 80;
    
    // Position menu to the right of the ball, but keep it on screen
    let mut menu_x = ball_x * CELL_SIZE + CELL_SIZE;
    let mut menu_y = ball_y * CELL_SIZE;
    
    // Adjust if menu would go off screen
    if menu_x + menu_width > WINDOW_WIDTH {
        menu_x = ball_x * CELL_SIZE - menu_width;
    }
    if menu_y + menu_height > WINDOW_HEIGHT {
        menu_y = WINDOW_HEIGHT - menu_height;
    }
    
    draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
    draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);
    
    // Title
    draw_text(frame, "Speed:", menu_x + 10, menu_y + 10, [255, 255, 255], false);
    
    // Speed value display
    let speed_text = format!("{:.1} units/sec", speed);
    draw_text(frame, &speed_text, menu_x + 10, menu_y + 30, [255, 255, 0], false);
    
    // Slider track
    let slider_x = menu_x + 10;
    let slider_y = menu_y + 50;
    let slider_width = 150;
    let slider_height = 4;
    
    // Draw slider track (dark gray)
    for y in slider_y..slider_y + slider_height {
        for x in slider_x..slider_x + slider_width {
            if x < WINDOW_WIDTH && y < WINDOW_HEIGHT {
                let index = (y * WINDOW_WIDTH + x) * 4;
                if index + 2 < frame.len() {
                    frame[index] = 80;     // R
                    frame[index + 1] = 80; // G
                    frame[index + 2] = 80; // B
                }
            }
        }
    }
    
    // Calculate slider position
    let normalized_speed = (speed - MIN_SPEED) / (MAX_SPEED - MIN_SPEED);
    let slider_pos = slider_x + (normalized_speed * slider_width as f32) as usize;
    
    // Draw slider handle (white circle)
    let handle_radius = 6;
    let handle_center_x = slider_pos;
    let handle_center_y = slider_y + slider_height / 2;
    
    for y in handle_center_y.saturating_sub(handle_radius)..handle_center_y + handle_radius {
        for x in handle_center_x.saturating_sub(handle_radius)..handle_center_x + handle_radius {
            if x < WINDOW_WIDTH && y < WINDOW_HEIGHT {
                let dx = x as i32 - handle_center_x as i32;
                let dy = y as i32 - handle_center_y as i32;
                if dx * dx + dy * dy <= (handle_radius as i32) * (handle_radius as i32) {
                    let index = (y * WINDOW_WIDTH + x) * 4;
                    if index + 2 < frame.len() {
                        frame[index] = 255;     // R
                        frame[index + 1] = 255; // G
                        frame[index + 2] = 255; // B
                    }
                }
            }
        }
    }
    
    // Instructions
    draw_text(frame, "<- -> to adjust, Space to confirm", menu_x + 10, menu_y + 65, [180, 180, 180], false);
}



fn draw_color_menu(frame: &mut [u8], ball_x: usize, ball_y: usize, selected_option: usize) {
    let menu_width = CELL_SIZE * 4;
    let menu_height = CELL_SIZE * 6;
    
    // Position menu to the right of the ball, but keep it on screen
    let mut menu_x = ball_x * CELL_SIZE + CELL_SIZE;
    let mut menu_y = ball_y * CELL_SIZE;
    
    // Adjust if menu would go off screen
    if menu_x + menu_width > WINDOW_WIDTH {
        menu_x = ball_x * CELL_SIZE - menu_width;
    }
    if menu_y + menu_height > WINDOW_HEIGHT {
        menu_y = WINDOW_HEIGHT - menu_height;
    }
    
    draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
    draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);
    
    // Draw color options with color preview
    for (i, option) in COLOR_OPTIONS.iter().enumerate() {
        let text_x = menu_x + 25;
        let text_y = menu_y + 5 + i * 18;
        let is_selected = i == selected_option;
        
        // Draw color preview square
        let color_preview = get_color_rgb(option);
        let preview_x = menu_x + 5;
        let preview_y = text_y + 2;
        let preview_size = 12;
        
        for dy in 0..preview_size {
            for dx in 0..preview_size {
                let px = preview_x + dx;
                let py = preview_y + dy;
                if px < WINDOW_WIDTH && py < WINDOW_HEIGHT {
                    let idx = (py * WINDOW_WIDTH + px) * 4;
                    frame[idx] = color_preview[0];     // R
                    frame[idx + 1] = color_preview[1]; // G
                    frame[idx + 2] = color_preview[2]; // B
                    frame[idx + 3] = 255;              // A
                }
            }
        }
        
        draw_text(frame, option, text_x, text_y, [200, 200, 200], is_selected);
    }
}

fn draw_relative_speed_menu(frame: &mut [u8], ball_x: usize, ball_y: usize, selected_ball: usize, speed_ratio: f32, balls: &[Ball]) {
    let menu_width = 280;
    let menu_height = 180;
    
    // Position menu to the right of the ball, but keep it on screen
    let mut menu_x = ball_x * CELL_SIZE + CELL_SIZE;
    let mut menu_y = ball_y * CELL_SIZE;
    
    // Adjust if menu would go off screen
    if menu_x + menu_width > WINDOW_WIDTH {
        menu_x = ball_x * CELL_SIZE - menu_width;
    }
    if menu_y + menu_height > WINDOW_HEIGHT {
        menu_y = WINDOW_HEIGHT - menu_height;
    }
    
    draw_menu_background(frame, menu_x, menu_y, menu_width, menu_height);
    draw_menu_border(frame, menu_x, menu_y, menu_width, menu_height);
    
    // Title with icon
    draw_text(frame, "⚡ Relative Speed", menu_x + 10, menu_y + 10, [255, 255, 255], false);
    
    // Reference ball section with visual indicator
    let ref_y = menu_y + 35;
    draw_text(frame, "Reference Ball:", menu_x + 10, ref_y, [200, 200, 200], false);
    
    if let Some(ref_ball) = balls.get(selected_ball) {
        // Draw reference ball indicator
        let ball_indicator_x = menu_x + 10;
        let ball_indicator_y = ref_y + 20;
        let ball_color = get_color_rgb(&ref_ball.color);
        
        // Draw small ball representation
        for dy in 0..8 {
            for dx in 0..8 {
                let px = ball_indicator_x + dx;
                let py = ball_indicator_y + dy;
                if px < WINDOW_WIDTH && py < WINDOW_HEIGHT {
                    let distance_sq = (dx as f32 - 4.0).powi(2) + (dy as f32 - 4.0).powi(2);
                    if distance_sq <= 16.0 {
                        let idx = (py * WINDOW_WIDTH + px) * 4;
                        if idx + 3 < frame.len() {
                            frame[idx] = ball_color[0];
                            frame[idx + 1] = ball_color[1];
                            frame[idx + 2] = ball_color[2];
                            frame[idx + 3] = 255;
                        }
                    }
                }
            }
        }
        
        let ref_text = format!("Ball {} - {:.1} u/s", selected_ball, ref_ball.speed);
        draw_text(frame, &ref_text, ball_indicator_x + 20, ball_indicator_y, [255, 255, 255], false);
    } else {
        draw_text(frame, "No ball selected", menu_x + 10, ref_y + 20, [255, 100, 100], false);
    }
    
    // Speed ratio section with visual slider
    let ratio_y = menu_y + 80;
    draw_text(frame, "Speed Multiplier:", menu_x + 10, ratio_y, [200, 200, 200], false);
    
    let ratio_index = RELATIVE_SPEED_RATIOS.iter().position(|&r| (r - speed_ratio).abs() < 0.001).unwrap_or(4);
    let ratio_label = RELATIVE_SPEED_LABELS.get(ratio_index).unwrap_or(&"1x");
    
    // Draw ratio slider background
    let slider_x = menu_x + 10;
    let slider_y = ratio_y + 20;
    let slider_width = 200;
    let slider_height = 6;
    
    // Draw slider track
    for y in slider_y..slider_y + slider_height {
        for x in slider_x..slider_x + slider_width {
            if x < WINDOW_WIDTH && y < WINDOW_HEIGHT {
                let idx = (y * WINDOW_WIDTH + x) * 4;
                if idx + 3 < frame.len() {
                    frame[idx] = 60;
                    frame[idx + 1] = 60;
                    frame[idx + 2] = 60;
                    frame[idx + 3] = 255;
                }
            }
        }
    }
    
    // Draw ratio markers
    for (i, &ratio) in RELATIVE_SPEED_RATIOS.iter().enumerate() {
        let marker_x = slider_x + (i * slider_width / (RELATIVE_SPEED_RATIOS.len() - 1));
        let marker_color = if i == ratio_index { [255, 255, 0] } else { [150, 150, 150] };
        let marker_size = if i == ratio_index { 8 } else { 4 };
        
        // Draw marker
        for dy in 0..marker_size {
            for dx in 0..marker_size {
                let px = marker_x + dx - marker_size / 2;
                let py = slider_y + dy - 2;
                if px < WINDOW_WIDTH && py < WINDOW_HEIGHT {
                    let idx = (py * WINDOW_WIDTH + px) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = marker_color[0];
                        frame[idx + 1] = marker_color[1];
                        frame[idx + 2] = marker_color[2];
                        frame[idx + 3] = 255;
                    }
                }
            }
        }
    }
    
    // Current ratio display
    let ratio_text = format!("× {}", ratio_label);
    draw_text(frame, &ratio_text, menu_x + 220, ratio_y + 15, [255, 255, 0], false);
    
    // Result section with visual feedback
    let result_y = menu_y + 115;
    draw_text(frame, "Result Speed:", menu_x + 10, result_y, [200, 200, 200], false);
    
    if let Some(ref_ball) = balls.get(selected_ball) {
        let calculated_speed = ref_ball.speed * speed_ratio;
        let clamped_speed = calculated_speed.clamp(MIN_SPEED, MAX_SPEED);
        let speed_text = format!("{:.1} u/s", clamped_speed);
        
        // Color coding for result
        let result_color = if calculated_speed != clamped_speed {
            [255, 100, 100] // Red if clamped
        } else if speed_ratio > 1.0 {
            [100, 255, 100] // Green if faster
        } else if speed_ratio < 1.0 {
            [100, 150, 255] // Blue if slower
        } else {
            [255, 255, 255] // White if same
        };
        
        draw_text(frame, &speed_text, menu_x + 120, result_y, result_color, false);
        
        // Warning if clamped
        if calculated_speed != clamped_speed {
            draw_text(frame, "(clamped)", menu_x + 180, result_y, [255, 100, 100], false);
        }
    }
    
    // Instructions with better formatting
    let instr_y = menu_y + 145;
    draw_text(frame, "↑↓ Reference Ball  ←→ Multiplier", menu_x + 10, instr_y, [180, 180, 180], false);
    draw_text(frame, "Space: Apply  Esc: Back", menu_x + 10, instr_y + 15, [180, 180, 180], false);
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