use winit::event::VirtualKeyCode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContextMenuState {
    None,
    BallMenu { ball_index: usize, selected_option: usize },
    BallDirection { ball_index: usize, selected_option: usize },
    BallSpeed { ball_index: usize, speed: f32 }, // speed in grid units per second
    BallSample { ball_index: usize, selected_option: usize },
}

pub struct ContextMenu {
    pub state: ContextMenuState,
}

const BALL_MENU_OPTIONS: &[&str] = &["Direction", "Speed", "Sample"];
const DIRECTION_OPTIONS: &[&str] = &["Up", "Down", "Left", "Right", "Up-Left", "Up-Right", "Down-Left", "Down-Right"];
const MIN_SPEED: f32 = 0.5;
const MAX_SPEED: f32 = 10.0;
const SPEED_STEP: f32 = 0.1;
const SAMPLE_OPTIONS: &[&str] = &["Kick", "Snare", "Hi-hat", "Load File..."];

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
                    match selected_option {
                        0 => self.state = ContextMenuState::BallDirection { ball_index, selected_option: 0 },
                        1 => {
                            // Initialize with current ball speed
                            let current_speed = balls.get(ball_index).map(|b| b.speed).unwrap_or(2.0);
                            self.state = ContextMenuState::BallSpeed { ball_index, speed: current_speed };
                        },
                        2 => self.state = ContextMenuState::BallSample { ball_index, selected_option: 0 },
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
            ContextMenuState::BallSample { ball_index, selected_option } => {
                if input.key_pressed(VirtualKeyCode::Escape) {
                    self.state = ContextMenuState::BallMenu { ball_index, selected_option: 2 };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Up) {
                    let new_option = if selected_option == 0 { SAMPLE_OPTIONS.len() - 1 } else { selected_option - 1 };
                    self.state = ContextMenuState::BallSample { ball_index, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Down) {
                    let new_option = (selected_option + 1) % SAMPLE_OPTIONS.len();
                    self.state = ContextMenuState::BallSample { ball_index, selected_option: new_option };
                    return None;
                }
                if input.key_pressed(VirtualKeyCode::Space) {
                    match selected_option {
                        0 => {
                            let sample = "kick.wav".to_string();
                            self.state = ContextMenuState::BallMenu { ball_index, selected_option: 2 };
                            return Some(ContextMenuAction::SetSample { ball_index, sample });
                        }
                        1 => {
                            let sample = "snare.wav".to_string();
                            self.state = ContextMenuState::BallMenu { ball_index, selected_option: 2 };
                            return Some(ContextMenuAction::SetSample { ball_index, sample });
                        }
                        2 => {
                            let sample = "hihat.wav".to_string();
                            self.state = ContextMenuState::BallMenu { ball_index, selected_option: 2 };
                            return Some(ContextMenuAction::SetSample { ball_index, sample });
                        }
                        3 => {
                            // Load File... option
                            self.state = ContextMenuState::BallMenu { ball_index, selected_option: 2 };
                            return Some(ContextMenuAction::OpenFileDialog { ball_index });
                        }
                        _ => {
                            let sample = "kick.wav".to_string();
                            self.state = ContextMenuState::BallMenu { ball_index, selected_option: 2 };
                            return Some(ContextMenuAction::SetSample { ball_index, sample });
                        }
                    }
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
                    draw_ball_menu(frame, ball_x, ball_y, selected_option);
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
            ContextMenuState::BallSample { ball_index, selected_option } => {
                if let Some(ball) = balls.get(ball_index) {
                    let (ball_x, ball_y) = ball.get_grid_position();
                    draw_sample_menu(frame, ball_x, ball_y, selected_option);
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
    OpenFileDialog { ball_index: usize },
}

// Import types from modules
use crate::ball::{Ball, Direction};

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
    let bg_color = if selected { [100, 100, 255] } else { [40, 40, 40] };
    let text_color = if selected { [255, 255, 255] } else { color };
    
    // Draw background for text area
    let text_width = text.len() * 8; // Approximate character width
    let text_height = 12; // Approximate character height
    
    for dy in 0..text_height {
        for dx in 0..text_width {
            let px = x + dx;
            let py = y + dy;
            if px < WINDOW_WIDTH && py < WINDOW_HEIGHT {
                let idx = (py * WINDOW_WIDTH + px) * 4;
                frame[idx] = bg_color[0];     // R
                frame[idx + 1] = bg_color[1]; // G
                frame[idx + 2] = bg_color[2]; // B
                frame[idx + 3] = 255;         // A
            }
        }
    }
    
    // Simple text rendering - draw pixels for each character
    for (i, ch) in text.chars().enumerate() {
        let char_x = x + i * 8;
        draw_simple_char(frame, ch, char_x, y + 2, text_color);
    }
}

fn draw_simple_char(frame: &mut [u8], ch: char, x: usize, y: usize, color: [u8; 3]) {
    // Improved character rendering with more complete patterns
    let patterns = match ch {
        'A' | 'a' => vec![
            [0,1,1,0],
            [1,0,0,1],
            [1,1,1,1],
            [1,0,0,1],
        ],
        'B' | 'b' => vec![
            [1,1,1,0],
            [1,0,0,1],
            [1,1,1,0],
            [1,1,1,1],
        ],
        'C' | 'c' => vec![
            [0,1,1,1],
            [1,0,0,0],
            [1,0,0,0],
            [0,1,1,1],
        ],
        'D' | 'd' => vec![
            [1,1,1,0],
            [1,0,0,1],
            [1,0,0,1],
            [1,1,1,0],
        ],
        'E' | 'e' => vec![
            [1,1,1,1],
            [1,0,0,0],
            [1,1,1,0],
            [1,1,1,1],
        ],
        'F' | 'f' => vec![
            [1,1,1,1],
            [1,0,0,0],
            [1,1,1,0],
            [1,0,0,0],
        ],
        'G' | 'g' => vec![
            [0,1,1,1],
            [1,0,0,0],
            [1,0,1,1],
            [0,1,1,1],
        ],
        'H' | 'h' => vec![
            [1,0,0,1],
            [1,0,0,1],
            [1,1,1,1],
            [1,0,0,1],
        ],
        'I' | 'i' => vec![
            [1,1,1,1],
            [0,1,1,0],
            [0,1,1,0],
            [1,1,1,1],
        ],
        'K' | 'k' => vec![
            [1,0,0,1],
            [1,0,1,0],
            [1,1,0,0],
            [1,0,1,1],
        ],
        'L' | 'l' => vec![
            [1,0,0,0],
            [1,0,0,0],
            [1,0,0,0],
            [1,1,1,1],
        ],
        'M' | 'm' => vec![
            [1,0,0,1],
            [1,1,1,1],
            [1,0,0,1],
            [1,0,0,1],
        ],
        'N' | 'n' => vec![
            [1,0,0,1],
            [1,1,0,1],
            [1,0,1,1],
            [1,0,0,1],
        ],
        'O' | 'o' => vec![
            [0,1,1,0],
            [1,0,0,1],
            [1,0,0,1],
            [0,1,1,0],
        ],
        'P' | 'p' => vec![
            [1,1,1,0],
            [1,0,0,1],
            [1,1,1,0],
            [1,0,0,0],
        ],
        'R' | 'r' => vec![
            [1,1,1,0],
            [1,0,0,1],
            [1,1,1,0],
            [1,0,0,1],
        ],
        'S' | 's' => vec![
            [0,1,1,1],
            [1,0,0,0],
            [0,1,1,0],
            [1,1,1,0],
        ],
        'T' | 't' => vec![
            [1,1,1,1],
            [0,1,1,0],
            [0,1,1,0],
            [0,1,1,0],
        ],
        'U' | 'u' => vec![
            [1,0,0,1],
            [1,0,0,1],
            [1,0,0,1],
            [0,1,1,0],
        ],
        'W' | 'w' => vec![
            [1,0,0,1],
            [1,0,0,1],
            [1,1,1,1],
            [1,0,0,1],
        ],
        ' ' => vec![
            [0,0,0,0],
            [0,0,0,0],
            [0,0,0,0],
            [0,0,0,0],
        ],
        '-' => vec![
            [0,0,0,0],
            [0,0,0,0],
            [1,1,1,1],
            [0,0,0,0],
        ],
        '(' => vec![
            [0,1,0,0],
            [1,0,0,0],
            [1,0,0,0],
            [0,1,0,0],
        ],
        ')' => vec![
            [0,0,1,0],
            [0,0,0,1],
            [0,0,0,1],
            [0,0,1,0],
        ],
        '0' => vec![
            [0,1,1,0],
            [1,0,0,1],
            [1,0,0,1],
            [0,1,1,0],
        ],
        '1' => vec![
            [0,1,0,0],
            [1,1,0,0],
            [0,1,0,0],
            [1,1,1,0],
        ],
        '2' => vec![
            [1,1,1,0],
            [0,0,0,1],
            [0,1,1,0],
            [1,1,1,1],
        ],
        '3' => vec![
            [1,1,1,0],
            [0,0,0,1],
            [0,1,1,0],
            [1,1,1,0],
        ],
        '5' => vec![
            [1,1,1,1],
            [1,0,0,0],
            [1,1,1,0],
            [1,1,1,0],
        ],
        _ => vec![
            [1,1,1,1],
            [1,0,0,1],
            [1,0,0,1],
            [1,1,1,1],
        ],
    };
    
    for (row, pattern) in patterns.iter().enumerate() {
        for (col, &pixel) in pattern.iter().enumerate() {
            if pixel == 1 {
                let px = x + col;
                let py = y + row;
                if px < WINDOW_WIDTH && py < WINDOW_HEIGHT {
                    let idx = (py * WINDOW_WIDTH + px) * 4;
                    frame[idx] = color[0];     // R
                    frame[idx + 1] = color[1]; // G
                    frame[idx + 2] = color[2]; // B
                    frame[idx + 3] = 255;      // A
                }
            }
        }
    }
}

fn draw_ball_menu(frame: &mut [u8], ball_x: usize, ball_y: usize, selected_option: usize) {
    let menu_width = CELL_SIZE * 4;
    let menu_height = CELL_SIZE * 3;
    
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
    
    // Draw menu options
    for (i, option) in BALL_MENU_OPTIONS.iter().enumerate() {
        let text_x = menu_x + 5;
        let text_y = menu_y + 5 + i * 20;
        let is_selected = i == selected_option;
        draw_text(frame, option, text_x, text_y, [200, 200, 200], is_selected);
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

fn draw_sample_menu(frame: &mut [u8], ball_x: usize, ball_y: usize, selected_option: usize) {
    let menu_width = CELL_SIZE * 4;
    let menu_height = CELL_SIZE * 3;
    
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
    
    // Draw sample options
    for (i, option) in SAMPLE_OPTIONS.iter().enumerate() {
        let text_x = menu_x + 5;
        let text_y = menu_y + 5 + i * 20;
        let is_selected = i == selected_option;
        draw_text(frame, option, text_x, text_y, [200, 200, 200], is_selected);
    }
}