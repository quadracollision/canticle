use crate::sequencer::{GRID_WIDTH, GRID_HEIGHT};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

#[derive(Clone, Debug)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub original_x: f32,
    pub original_y: f32,
    pub direction: Direction,
    pub sample_path: Option<String>,
    pub speed: f32, // Speed in grid units per second
    pub active: bool,
    pub last_grid_x: usize,
    pub last_grid_y: usize,
    pub color: String,
}

impl Ball {
    pub fn new(x: usize, y: usize) -> Self {
        Self {
            x: x as f32 + 0.5,
            y: y as f32 + 0.5,
            original_x: x as f32 + 0.5,
            original_y: y as f32 + 0.5,
            direction: Direction::Up,
            sample_path: None,
            speed: 2.0, // 2 grid units per second
            active: false, // Start inactive
            last_grid_x: x,
            last_grid_y: y,
            color: "White".to_string(), // Default color
        }
    }
    
    pub fn update_position(&mut self, delta_time: f32) -> Vec<(usize, usize)> {
        if !self.active {
            return Vec::new();
        }
        
        let mut triggered_positions = Vec::new();
        
        // Calculate movement delta
        let movement_speed = self.speed * delta_time;
        let (dx, dy) = self.get_direction_vector();
        
        // Store old position
        let old_x = self.x;
        let old_y = self.y;
        
        // Update position
        self.x += dx * movement_speed;
        self.y += dy * movement_speed;
        
        // Check boundaries and reverse if needed
        let mut _reversed = false;
        if self.x <= 0.0 || self.x >= GRID_WIDTH as f32 {
            self.x = old_x;
            self.direction = self.reverse_horizontal_direction();
            _reversed = true;
        }
        if self.y <= 0.0 || self.y >= GRID_HEIGHT as f32 {
              self.y = old_y;
              self.direction = self.reverse_vertical_direction();
              _reversed = true;
          }
        
        // Check if we've entered a new grid cell
        let current_grid_x = self.x.floor() as usize;
        let current_grid_y = self.y.floor() as usize;
        
        if current_grid_x != self.last_grid_x || current_grid_y != self.last_grid_y {
            if current_grid_x < GRID_WIDTH && current_grid_y < GRID_HEIGHT {
                triggered_positions.push((current_grid_x, current_grid_y));
            }
            self.last_grid_x = current_grid_x;
            self.last_grid_y = current_grid_y;
        }
        
        triggered_positions
    }
    
    fn get_direction_vector(&self) -> (f32, f32) {
        match self.direction {
            Direction::Up => (0.0, -1.0),
            Direction::Down => (0.0, 1.0),
            Direction::Left => (-1.0, 0.0),
            Direction::Right => (1.0, 0.0),
            Direction::UpLeft => (-0.707, -0.707),
            Direction::UpRight => (0.707, -0.707),
            Direction::DownLeft => (-0.707, 0.707),
            Direction::DownRight => (0.707, 0.707),
        }
    }
    
    fn reverse_horizontal_direction(&self) -> Direction {
        match self.direction {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::UpLeft => Direction::UpRight,
            Direction::UpRight => Direction::UpLeft,
            Direction::DownLeft => Direction::DownRight,
            Direction::DownRight => Direction::DownLeft,
            _ => self.direction,
        }
    }
    
    fn reverse_vertical_direction(&self) -> Direction {
        match self.direction {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::UpLeft => Direction::DownLeft,
            Direction::UpRight => Direction::DownRight,
            Direction::DownLeft => Direction::UpLeft,
            Direction::DownRight => Direction::UpRight,
            _ => self.direction,
        }
    }
    
    pub fn get_grid_position(&self) -> (usize, usize) {
        (self.x.floor() as usize, self.y.floor() as usize)
    }
    
    pub fn reverse_direction(&mut self) {
        self.direction = match self.direction {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::UpLeft => Direction::DownRight,
            Direction::UpRight => Direction::DownLeft,
            Direction::DownLeft => Direction::UpRight,
            Direction::DownRight => Direction::UpLeft,
        };
    }
    
    pub fn reset_to_original(&mut self) {
        self.x = self.original_x;
        self.y = self.original_y;
        self.last_grid_x = self.original_x.floor() as usize;
        self.last_grid_y = self.original_y.floor() as usize;
        self.active = false;
    }
    
    pub fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;
    }
    
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }
    
    pub fn set_sample(&mut self, sample_path: String) {
        self.sample_path = Some(sample_path);
    }
    
    pub fn set_color(&mut self, color: String) {
        self.color = color;
    }
    
    pub fn toggle_active(&mut self) {
        self.active = !self.active;
    }
    
    pub fn activate(&mut self) {
        self.active = true;
    }
    
    pub fn deactivate(&mut self) {
        self.active = false;
    }
}