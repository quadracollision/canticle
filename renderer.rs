use std::collections::VecDeque;
use crate::ball::Ball;
use crate::square::Cell;
use crate::font;

// Rendering constants moved from sequencer.rs
pub const GRID_WIDTH: usize = 16;
pub const GRID_HEIGHT: usize = 12;
pub const CELL_SIZE: usize = 40;
pub const CONSOLE_HEIGHT: usize = 150;
pub const WINDOW_WIDTH: usize = GRID_WIDTH * CELL_SIZE;
pub const WINDOW_HEIGHT: usize = GRID_HEIGHT * CELL_SIZE + CONSOLE_HEIGHT;
pub const GRID_AREA_HEIGHT: usize = GRID_HEIGHT * CELL_SIZE;

pub struct Renderer;

impl Renderer {
    pub fn get_color_rgb(color_name: &str) -> [u8; 3] {
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

    pub fn draw_grid_lines(frame: &mut [u8]) {
        let grid_color = [60, 60, 60];
        
        // Vertical lines
        for x in 0..=GRID_WIDTH {
            let pixel_x = x * CELL_SIZE;
            if pixel_x < WINDOW_WIDTH {
                for y in 0..WINDOW_HEIGHT {
                    let index = (y * WINDOW_WIDTH + pixel_x) * 4;
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
            if pixel_y < WINDOW_HEIGHT {
                for x in 0..WINDOW_WIDTH {
                    let index = (pixel_y * WINDOW_WIDTH + x) * 4;
                    if index + 2 < frame.len() {
                        frame[index] = grid_color[0];
                        frame[index + 1] = grid_color[1];
                        frame[index + 2] = grid_color[2];
                    }
                }
            }
        }
    }

    pub fn draw_square(frame: &mut [u8], grid_x: usize, grid_y: usize, color: [u8; 3], display_text: &Option<String>) {
        let start_x = grid_x * CELL_SIZE + 2;
        let start_y = grid_y * CELL_SIZE + 2;
        let end_x = (grid_x + 1) * CELL_SIZE - 2;
        let end_y = (grid_y + 1) * CELL_SIZE - 2;
        
        for y in start_y..end_y {
            for x in start_x..end_x {
                if x < WINDOW_WIDTH && y < WINDOW_HEIGHT {
                    let index = (y * WINDOW_WIDTH + x) * 4;
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

    pub fn draw_circle(frame: &mut [u8], grid_x: usize, grid_y: usize, color: [u8; 3]) {
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
                
                if distance <= radius && x < WINDOW_WIDTH && y < WINDOW_HEIGHT {
                    let index = (y * WINDOW_WIDTH + x) * 4;
                    if index + 2 < frame.len() {
                        frame[index] = color[0];
                        frame[index + 1] = color[1];
                        frame[index + 2] = color[2];
                    }
                }
            }
        }
    }

    pub fn draw_cursor(frame: &mut [u8], cursor_x: usize, cursor_y: usize) {
        let cursor_color = [255, 255, 0]; // Yellow cursor
        let x = cursor_x * CELL_SIZE;
        let y = cursor_y * CELL_SIZE;
        
        // Draw cursor border
        for i in 0..CELL_SIZE {
            // Top border
            if x + i < WINDOW_WIDTH && y < WINDOW_HEIGHT {
                let index = (y * WINDOW_WIDTH + x + i) * 4;
                if index + 2 < frame.len() {
                    frame[index] = cursor_color[0];
                    frame[index + 1] = cursor_color[1];
                    frame[index + 2] = cursor_color[2];
                }
            }
            
            // Bottom border
            if x + i < WINDOW_WIDTH && y + CELL_SIZE - 1 < WINDOW_HEIGHT {
                let index = ((y + CELL_SIZE - 1) * WINDOW_WIDTH + x + i) * 4;
                if index + 2 < frame.len() {
                    frame[index] = cursor_color[0];
                    frame[index + 1] = cursor_color[1];
                    frame[index + 2] = cursor_color[2];
                }
            }
            
            // Left border
            if x < WINDOW_WIDTH && y + i < WINDOW_HEIGHT {
                let index = ((y + i) * WINDOW_WIDTH + x) * 4;
                if index + 2 < frame.len() {
                    frame[index] = cursor_color[0];
                    frame[index + 1] = cursor_color[1];
                    frame[index + 2] = cursor_color[2];
                }
            }
            
            // Right border
            if x + CELL_SIZE - 1 < WINDOW_WIDTH && y + i < WINDOW_HEIGHT {
                let index = ((y + i) * WINDOW_WIDTH + x + CELL_SIZE - 1) * 4;
                if index + 2 < frame.len() {
                    frame[index] = cursor_color[0];
                    frame[index + 1] = cursor_color[1];
                    frame[index + 2] = cursor_color[2];
                }
            }
        }
    }

    pub fn draw_ball(frame: &mut [u8], ball_x: f32, ball_y: f32, color: [u8; 3]) {
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

    pub fn draw_console(frame: &mut [u8], console_messages: &VecDeque<String>) {
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

    pub fn draw_menu_text(frame: &mut [u8], text: &str, x: usize, y: usize, color: [u8; 3], selected: bool) {
        font::draw_text(frame, text, x, y, color, selected, WINDOW_WIDTH);
    }

    pub fn draw_cursor_coordinates(frame: &mut [u8], cursor_x: usize, cursor_y: usize) {
        let coord_text = format!("({}, {})", cursor_x, cursor_y);
        // Position coordinates in the black area above grid (0,0)
        // Grid (0,0) starts at pixel (0,0), so we position the text just above it
        Self::draw_menu_text(frame, &coord_text, 5, 25, [255, 255, 255], false); // White text above grid (0,0)
    }
}