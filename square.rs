#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CellContent {
    Empty,
    Square,
}

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub content: CellContent,
    pub color: [u8; 3], // RGB color
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            content: CellContent::Empty,
            color: [100, 100, 100], // Default gray color
        }
    }
}

impl Cell {
    pub fn new_square(color: [u8; 3]) -> Self {
        Self {
            content: CellContent::Square,
            color,
        }
    }
    
    pub fn new_empty() -> Self {
        Self::default()
    }
    
    pub fn is_square(&self) -> bool {
        self.content == CellContent::Square
    }
    
    pub fn is_empty(&self) -> bool {
        self.content == CellContent::Empty
    }
    
    pub fn set_color(&mut self, color: [u8; 3]) {
        self.color = color;
    }
    
    pub fn clear(&mut self) {
        self.content = CellContent::Empty;
        self.color = [100, 100, 100];
    }
    
    pub fn place_square(&mut self, color: Option<[u8; 3]>) {
        self.content = CellContent::Square;
        if let Some(c) = color {
            self.color = c;
        } else {
            self.color = [255, 255, 255]; // Default white square
        }
    }
}