#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CellContent {
    Empty,
    Square,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Number(f32),
    Direction(crate::ball::Direction),
    Boolean(bool),
    String(String),
    Coordinate(f32, f32), // Add coordinate support
}

#[derive(Clone, PartialEq, Debug)]
pub enum Expression {
    Literal(Value),
    Variable(String),
    GlobalVariable(String),
    BinaryOp { left: Box<Expression>, op: BinaryOperator, right: Box<Expression> },
    BallProperty(BallProperty),
    Random { min: f32, max: f32 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BinaryOperator {
    Add, Sub, Mul, Div, Mod,
    Equal, NotEqual, Less, Greater, LessEqual, GreaterEqual,
    And, Or,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BallProperty {
    Speed,
    Direction,
    X,
    Y,
    HitCount,
    Pitch,
    Volume,
}

#[derive(Clone, PartialEq, Debug)]
pub enum DestroyTarget {
    Coordinates { x: Expression, y: Expression },
    BallReference(String), // "self", "last.c_red.self", etc.
}

#[derive(Clone, PartialEq, Debug)]
pub enum Instruction {
    // Ball manipulation
    SetSpeed(Expression),
    SetDirection(Expression),
    SetPitch(Expression),
    SetVolume(Expression),
    SetColor(Expression),
    Bounce,
    Stop,
    
    // Variables
    SetVariable { name: String, value: Expression },
    SetGlobalVariable { name: String, value: Expression },
    
    // Control flow
    If { condition: Expression, then_block: Vec<Instruction>, else_block: Option<Vec<Instruction>> },
    Loop { count: Expression, body: Vec<Instruction> },
    RepeatAnd { count: Expression, body: Vec<Instruction> }, // Repeat instructions N times with 'and N'
    RepeatThen { count: Expression, body: Vec<Instruction> }, // Repeat instructions N times with 'then N'
    ExecuteProgram(Program),
    ExecuteLibraryFunction { library_function: String },
    ContinueToNext, // Continue to next function in sequence
    Return(Option<String>), // None = simple return, Some(name) = call function and return
    End, // Natural end of block
    
    // Audio
    PlaySample(Expression),
    SetReverse { ball_reference: String, speed: Expression },
    SetSliceArray { markers: Vec<u32> }, // Set slice array for sequential marker playback
    
    // Grid interaction
    SpawnBall { x: Expression, y: Expression, speed: Expression, direction: Expression },
    CreateBall { x: Expression, y: Expression, speed: Expression, direction: Expression },
    CreateSquare { x: Expression, y: Expression },
    CreateSquareWithProgram { x: Expression, y: Expression, program: Program },
    CreateBallFromSample { x: Expression, y: Expression, library_name: String, sample_name: String },
    CreateSquareFromSample { x: Expression, y: Expression, library_name: String, sample_name: String },
    CreateBallWithLibrary { x: Expression, y: Expression, library_function: String, audio_file: Option<String> },
    CreateSquareWithLibrary { x: Expression, y: Expression, library_function: String, audio_file: Option<String> },
    DestroyBall { target: DestroyTarget },
    DestroySquare { target: DestroyTarget },
    
    // Debugging
    Print(Expression),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SquareEffect {
    None,               // No effect, ball passes through
    Bounce,             // Reverse ball direction (default)
    SpeedBoost(f32),    // Multiply ball speed by factor
    DirectionChange(crate::ball::Direction), // Change ball to specific direction
    Stop,               // Stop the ball completely
    Sample(usize),      // Play a specific sample (index into sample array)
    Program(usize),     // Execute program at index
}

use std::collections::{HashMap, VecDeque};
use crate::ball::Ball;

#[derive(Clone, PartialEq, Debug)]
pub struct Program {
    pub instructions: Vec<Instruction>,
    pub name: String,
    pub source_text: Option<Vec<String>>, // Preserve original source text for editing
}

// Library system for reusable components
#[derive(Clone, PartialEq, Debug)]
pub struct FunctionLibrary {
    pub name: String,
    pub functions: HashMap<String, Program>,
    pub description: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SampleLibrary {
    pub name: String,
    pub samples: HashMap<String, SampleTemplate>,
    pub description: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SampleTemplate {
    pub name: String,
    pub default_speed: f32,
    pub default_direction: crate::ball::Direction,
    pub color: String,
    pub behavior_program: Option<String>, // Reference to function in library
}

#[derive(Clone, PartialEq, Debug)]
pub struct LibraryManager {
    pub function_libraries: HashMap<String, FunctionLibrary>,
    pub sample_libraries: HashMap<String, SampleLibrary>,
}

impl Default for LibraryManager {
    fn default() -> Self {
        Self {
            function_libraries: HashMap::new(),
            sample_libraries: HashMap::new(),
        }
    }
}

impl LibraryManager {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_function_library(&mut self, library: FunctionLibrary) {
        self.function_libraries.insert(library.name.clone(), library);
    }
    
    pub fn add_sample_library(&mut self, library: SampleLibrary) {
        self.sample_libraries.insert(library.name.clone(), library);
    }
    
    pub fn get_function(&self, library_name: &str, function_name: &str) -> Option<&Program> {
        self.function_libraries.get(library_name)?
            .functions.get(function_name)
    }
    
    pub fn get_sample_template(&self, library_name: &str, sample_name: &str) -> Option<&SampleTemplate> {
        self.sample_libraries.get(library_name)?
            .samples.get(sample_name)
    }
    
    pub fn get_ball_sample(&self, library_name: &str, sample_name: &str) -> Option<&SampleTemplate> {
        self.get_sample_template(library_name, sample_name)
    }
    
    pub fn get_square_sample(&self, library_name: &str, sample_name: &str) -> Option<&SampleTemplate> {
        self.get_sample_template(library_name, sample_name)
    }
    
    pub fn load_library_from_file(&mut self, file_path: &str) -> Result<(), String> {
        use std::fs;
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read library file {}: {}", file_path, e))?;
        
        self.parse_library_file(&content, file_path)
    }
    
    pub fn parse_library_file(&mut self, content: &str, file_path: &str) -> Result<(), String> {
        let lines: Vec<&str> = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty() && !l.starts_with("//")).collect();
        
        let mut i = 0;
        let mut current_library = FunctionLibrary {
            name: "default".to_string(),
            functions: HashMap::new(),
            description: format!("Library loaded from {}", file_path),
        };
        
        while i < lines.len() {
            let line = lines[i];
            
            if line.starts_with("library ") {
                // Save previous library if it has functions
                if !current_library.functions.is_empty() {
                    self.add_function_library(current_library.clone());
                }
                
                // Start new library
                let library_name = line[8..].trim().to_string();
                current_library = FunctionLibrary {
                    name: library_name,
                    functions: HashMap::new(),
                    description: format!("Library loaded from {}", file_path),
                };
                i += 1;
            } else if line.starts_with("def ") {
                // Parse function definition
                let (program, next_i) = self.parse_function_from_lines(&lines, i)?;
                current_library.functions.insert(program.name.clone(), program);
                i = next_i;
            } else {
                i += 1;
            }
        }
        
        // Add the last library
        if !current_library.functions.is_empty() {
            self.add_function_library(current_library);
        }
        
        Ok(())
    }
    
    fn parse_function_from_lines(&self, lines: &[&str], start_index: usize) -> Result<(Program, usize), String> {
        let line = lines[start_index];
        if !line.starts_with("def ") {
            return Err("Expected function definition".to_string());
        }
        
        let function_name = line[4..].trim().to_string();
        let (instructions, next_i) = self.parse_block_from_lines(lines, start_index + 1)?;
        
        Ok((Program {
            name: function_name,
            instructions,
            source_text: None,
        }, next_i))
    }
    
    fn parse_block_from_lines(&self, lines: &[&str], start_index: usize) -> Result<(Vec<Instruction>, usize), String> {
        let mut instructions = Vec::new();
        let mut i = start_index;
        
        while i < lines.len() {
            let line = lines[i];
            
            if line == "return" || line == "end" {
                i += 1;
                break;
            }
            
            // For now, use basic instruction parsing
            // This would need to be expanded to handle all instruction types
            if line.starts_with("create ball") {
                instructions.push(self.parse_create_ball_instruction(line)?);
            } else if line.starts_with("create square") {
                instructions.push(self.parse_create_square_instruction(line)?);
            } else if line == "bounce" {
                instructions.push(Instruction::Bounce);
            } else if line.starts_with("set speed") {
                instructions.push(self.parse_set_speed_instruction(line)?);
            }
            // Add more instruction parsing as needed
            
            i += 1;
        }
        
        Ok((instructions, i))
    }
    
    fn parse_create_ball_instruction(&self, line: &str) -> Result<Instruction, String> {
        // Parse "create ball(x, y)"
        if let Some(start) = line.find('(') {
            if let Some(end) = line.find(')') {
                let coords_str = &line[start + 1..end];
                let coords: Vec<&str> = coords_str.split(',').map(|s| s.trim()).collect();
                if coords.len() == 2 {
                    if let (Ok(x), Ok(y)) = (coords[0].parse::<f32>(), coords[1].parse::<f32>()) {
                        return Ok(Instruction::CreateBall {
                            x: Expression::Literal(Value::Number(x)),
                            y: Expression::Literal(Value::Number(y)),
                            speed: Expression::Literal(Value::Number(2.0)), // Default speed
                            direction: Expression::Literal(Value::Direction(crate::ball::Direction::Right)), // Default direction
                        });
                    }
                }
            }
        }
        Err(format!("Invalid create ball syntax: {}", line))
    }
    
    fn parse_create_square_instruction(&self, line: &str) -> Result<Instruction, String> {
        // Parse "create square(x, y)"
        if let Some(start) = line.find('(') {
            if let Some(end) = line.find(')') {
                let coords_str = &line[start + 1..end];
                let coords: Vec<&str> = coords_str.split(',').map(|s| s.trim()).collect();
                if coords.len() == 2 {
                    if let (Ok(x), Ok(y)) = (coords[0].parse::<f32>(), coords[1].parse::<f32>()) {
                        return Ok(Instruction::CreateSquare {
                            x: Expression::Literal(Value::Number(x)),
                            y: Expression::Literal(Value::Number(y)),
                        });
                    }
                }
            }
        }
        Err(format!("Invalid create square syntax: {}", line))
    }
    
    fn parse_set_speed_instruction(&self, line: &str) -> Result<Instruction, String> {
        // Parse "set speed X"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[0] == "set" && parts[1] == "speed" {
            if let Ok(speed) = parts[2].parse::<f32>() {
                return Ok(Instruction::SetSpeed(Expression::Literal(Value::Number(speed))));
            }
        }
        Err(format!("Invalid set speed syntax: {}", line))
    }
    
    pub fn load_libraries_from_directory(&mut self, dir_path: &str) -> Result<(), String> {
        use std::fs;
        
        let dir = fs::read_dir(dir_path)
            .map_err(|e| format!("Failed to read library directory {}: {}", dir_path, e))?;
        
        for entry in dir {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "lib") {
                if let Some(path_str) = path.to_str() {
                    if let Err(e) = self.load_library_from_file(path_str) {
                        eprintln!("Warning: Failed to load library {}: {}", path_str, e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    pub fn create_default_libraries(&mut self) {
        // Use programmatic library creation instead of file loading
        use crate::library::LibraryManagerExt;
        self.create_default_programmatic_libraries();
    }
    
    fn create_hardcoded_defaults(&mut self) {
        // Create default function library
        let mut default_functions = FunctionLibrary {
            name: "lib".to_string(),
            functions: HashMap::new(),
            description: "Default function library with common behaviors".to_string(),
        };
        
        // Add common function templates
        default_functions.functions.insert("ballcreator".to_string(), Program {
            name: "ballcreator".to_string(),
            instructions: vec![
                Instruction::CreateBall {
                    x: Expression::Literal(Value::Number(5.0)),
                    y: Expression::Literal(Value::Number(5.0)),
                    speed: Expression::Literal(Value::Number(2.0)),
                    direction: Expression::Literal(Value::Direction(crate::ball::Direction::Right)),
                }
            ],
            source_text: None,
        });
        
        default_functions.functions.insert("bounce".to_string(), Program {
            name: "bounce".to_string(),
            instructions: vec![Instruction::Bounce],
            source_text: None,
        });
        
        default_functions.functions.insert("speed_boost".to_string(), Program {
            name: "speed_boost".to_string(),
            instructions: vec![
                Instruction::SetSpeed(Expression::BinaryOp {
                    left: Box::new(Expression::BallProperty(BallProperty::Speed)),
                    op: BinaryOperator::Mul,
                    right: Box::new(Expression::Literal(Value::Number(1.5))),
                }),
            ],
            source_text: None,
        });
        
        default_functions.functions.insert("direction_cycle".to_string(), Program {
            name: "direction_cycle".to_string(),
            instructions: vec![
                Instruction::If {
                    condition: Expression::BinaryOp {
                        left: Box::new(Expression::BallProperty(BallProperty::Direction)),
                        op: BinaryOperator::Equal,
                        right: Box::new(Expression::Literal(Value::Direction(crate::ball::Direction::Up))),
                    },
                    then_block: vec![Instruction::SetDirection(Expression::Literal(Value::Direction(crate::ball::Direction::Right)))],
                    else_block: Some(vec![
                        Instruction::If {
                            condition: Expression::BinaryOp {
                                left: Box::new(Expression::BallProperty(BallProperty::Direction)),
                                op: BinaryOperator::Equal,
                                right: Box::new(Expression::Literal(Value::Direction(crate::ball::Direction::Right))),
                            },
                            then_block: vec![Instruction::SetDirection(Expression::Literal(Value::Direction(crate::ball::Direction::Down)))],
                            else_block: Some(vec![
                                Instruction::If {
                                    condition: Expression::BinaryOp {
                                        left: Box::new(Expression::BallProperty(BallProperty::Direction)),
                                        op: BinaryOperator::Equal,
                                        right: Box::new(Expression::Literal(Value::Direction(crate::ball::Direction::Down))),
                                    },
                                    then_block: vec![Instruction::SetDirection(Expression::Literal(Value::Direction(crate::ball::Direction::Left)))],
                                    else_block: Some(vec![Instruction::SetDirection(Expression::Literal(Value::Direction(crate::ball::Direction::Up)))]),
                                },
                            ]),
                        },
                    ]),
                },
            ],
            source_text: None,
        });
        
        self.add_function_library(default_functions);
        
        // Create default sample library
        let mut default_samples = SampleLibrary {
            name: "default".to_string(),
            samples: HashMap::new(),
            description: "Default sample library with common ball types".to_string(),
        };
        
        default_samples.samples.insert("red_bouncer".to_string(), SampleTemplate {
            name: "red_bouncer".to_string(),
            default_speed: 2.0,
            default_direction: crate::ball::Direction::Right,
            color: "Red".to_string(),
            behavior_program: Some("bounce".to_string()),
        });
        
        default_samples.samples.insert("blue_speedster".to_string(), SampleTemplate {
            name: "blue_speedster".to_string(),
            default_speed: 3.0,
            default_direction: crate::ball::Direction::Up,
            color: "Blue".to_string(),
            behavior_program: Some("speed_boost".to_string()),
        });
        
        default_samples.samples.insert("green_cycler".to_string(), SampleTemplate {
            name: "green_cycler".to_string(),
            default_speed: 1.5,
            default_direction: crate::ball::Direction::Left,
            color: "Green".to_string(),
            behavior_program: Some("direction_cycle".to_string()),
        });
        
        self.add_sample_library(default_samples);
    }
}

#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub variables: HashMap<String, Value>,
    pub ball_hit_count: u32,
    pub square_hit_count: u32,
    pub ball_x: f32,
    pub ball_y: f32,
    pub ball_speed: f32,
    pub ball_direction: crate::ball::Direction,
    pub ball_pitch: f32,
    pub ball_volume: f32,
    pub square_x: usize,
    pub square_y: usize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ProgramStep {
    pub trigger_hits: u32,     // Number of hits required to trigger this step
    pub effect: SquareEffect,  // Effect to apply when triggered
}

#[derive(Clone, Debug)]
pub struct SquareProgram {
    pub steps: Vec<ProgramStep>, // Legacy: Sequence of programmed effects
    pub programs: Vec<Program>,  // New: Full programs with instructions
    pub hit_count: u32,          // Track how many times this square has been hit
    pub sample_path: Option<usize>, // Index into sample array
    pub active_program: Option<usize>, // Index of currently active program
}

impl Default for SquareProgram {
    fn default() -> Self {
        Self {
            steps: vec![ProgramStep { trigger_hits: 1, effect: SquareEffect::Bounce }],
            programs: vec![
                Program {
                    name: "Default".to_string(),
                    instructions: vec![Instruction::Bounce],
                    source_text: None,
                }
            ],
            hit_count: 0,
            sample_path: None,
            active_program: Some(0),
        }
    }
}

impl SquareProgram {
    pub fn new() -> Self {
        Self::default()
    }
    
    // Legacy methods for backward compatibility
    pub fn add_step(&mut self, trigger_hits: u32, effect: SquareEffect) {
        self.steps.push(ProgramStep { trigger_hits, effect });
        self.steps.sort_by_key(|step| step.trigger_hits);
    }
    
    pub fn clear_steps(&mut self) {
        self.steps.clear();
    }
    
    pub fn delete_step(&mut self, step_index: usize) {
        if step_index < self.steps.len() {
            self.steps.remove(step_index);
        }
    }
    
    pub fn get_effects_for_hit_count(&self, hit_count: u32) -> Vec<SquareEffect> {
        self.steps.iter()
            .filter(|step| step.trigger_hits == hit_count)
            .map(|step| step.effect)
            .collect()
    }
    
    pub fn reset_hits(&mut self) {
        self.hit_count = 0;
    }
    
    // New programming system methods
    pub fn add_program(&mut self, program: Program) {
        self.programs.push(program);
    }
    
    pub fn update_program(&mut self, index: usize, program: Program) {
        if index < self.programs.len() {
            self.programs[index] = program;
        }
    }
    
    pub fn get_program(&self, index: usize) -> Option<&Program> {
        self.programs.get(index)
    }
    
    pub fn get_program_mut(&mut self, index: usize) -> Option<&mut Program> {
        self.programs.get_mut(index)
    }
    
    pub fn set_active_program(&mut self, index: Option<usize>) {
        self.active_program = index;
    }
    
    pub fn replace_or_add_program(&mut self, program: Program) -> usize {
        // If there's an active program and it's the default, replace it
        if let Some(active_index) = self.active_program {
            if let Some(existing_program) = self.programs.get(active_index) {
                let is_default_program = existing_program.name == "Default" && 
                    existing_program.instructions.len() == 1 &&
                    matches!(existing_program.instructions[0], crate::square::Instruction::Bounce);
                
                if is_default_program {
                    self.programs[active_index] = program;
                    return active_index;
                }
            }
        }
        
        // Otherwise, add as new program
        self.programs.push(program);
        self.programs.len() - 1
    }
    
    pub fn execute_program(&self, context: &mut ExecutionContext) -> Vec<ProgramAction> {
        if let Some(program_index) = self.active_program {
            if let Some(program) = self.programs.get(program_index) {
                return self.execute_instructions(&program.instructions, context);
            }
        }
        
        // Fallback to legacy system
        let effects = self.get_effects_for_hit_count(context.square_hit_count);
        effects.into_iter().map(|effect| match effect {
            SquareEffect::Bounce => ProgramAction::Bounce,
            SquareEffect::Stop => ProgramAction::Stop,
            SquareEffect::SpeedBoost(multiplier) => ProgramAction::SetSpeed(context.ball_speed * multiplier),
            SquareEffect::DirectionChange(dir) => ProgramAction::SetDirection(dir),
            SquareEffect::Sample(index) => ProgramAction::PlaySample(index),
            SquareEffect::None => ProgramAction::None,
            SquareEffect::Program(index) => {
                if let Some(program) = self.programs.get(index) {
                    return ProgramAction::ExecuteProgram(program.clone());
                }
                ProgramAction::None
            }
        }).collect()
    }
    
    pub fn execute_instructions(&self, instructions: &[Instruction], context: &mut ExecutionContext) -> Vec<ProgramAction> {
        let mut actions = Vec::new();
        
        for instruction in instructions {
            match instruction {
                Instruction::SetSpeed(expr) => {
                    if let Value::Number(speed) = self.evaluate_expression(expr, context) {
                        actions.push(ProgramAction::SetSpeed(speed));
                    }
                }
                Instruction::SetDirection(expr) => {
                    if let Value::Direction(dir) = self.evaluate_expression(expr, context) {
                        actions.push(ProgramAction::SetDirection(dir));
                    }
                }
                Instruction::SetPitch(expr) => {
                    if let Value::Number(pitch) = self.evaluate_expression(expr, context) {
                        actions.push(ProgramAction::SetPitch(pitch));
                    }
                }
                Instruction::SetVolume(expr) => {
                    if let Value::Number(volume) = self.evaluate_expression(expr, context) {
                        actions.push(ProgramAction::SetVolume(volume));
                    }
                }
                Instruction::SetColor(expr) => {
                    if let Value::String(color) = self.evaluate_expression(expr, context) {
                        actions.push(ProgramAction::SetColor(color));
                    }
                }
                Instruction::Bounce => {
                    actions.push(ProgramAction::Bounce);
                }
                Instruction::Stop => {
                    actions.push(ProgramAction::Stop);
                }
                Instruction::SetVariable { name, value } => {
                    let val = self.evaluate_expression(value, context);
                    context.variables.insert(name.clone(), val);
                }
                Instruction::SetGlobalVariable { name, value } => {
                    let val = self.evaluate_expression(value, context);
                    actions.push(ProgramAction::SetGlobalVariable { name: name.clone(), value: val });
                }
                Instruction::SetSliceArray { markers } => {
                    // Convert to ProgramAction with current square coordinates
                    actions.push(ProgramAction::SetSliceArray {
                        x: context.square_x,
                        y: context.square_y,
                        markers: markers.clone(),
                    });
                }
                Instruction::If { condition, then_block, else_block } => {
                    if let Value::Boolean(true) = self.evaluate_expression(condition, context) {
                        actions.extend(self.execute_instructions(then_block, context));
                    } else if let Some(else_instructions) = else_block {
                        actions.extend(self.execute_instructions(else_instructions, context));
                    }
                }
                Instruction::Loop { count, body } => {
                    if let Value::Number(n) = self.evaluate_expression(count, context) {
                        for _ in 0..(n as i32).max(0) {
                            actions.extend(self.execute_instructions(body, context));
                        }
                    }
                }
                Instruction::RepeatAnd { count, body } => {
                    if let Value::Number(n) = self.evaluate_expression(count, context) {
                        for _ in 0..(n as i32).max(0) {
                            actions.extend(self.execute_instructions(body, context));
                        }
                    }
                }
                Instruction::RepeatThen { count, body } => {
                    if let Value::Number(n) = self.evaluate_expression(count, context) {
                        for _ in 0..(n as i32).max(0) {
                            actions.extend(self.execute_instructions(body, context));
                        }
                    }
                }
                Instruction::PlaySample(expr) => {
                    if let Value::Number(index) = self.evaluate_expression(expr, context) {
                        actions.push(ProgramAction::PlaySample(index as usize));
                    }
                }
                Instruction::SetReverse { ball_reference, speed } => {
                    if let Value::Number(speed_val) = self.evaluate_expression(speed, context) {
                        actions.push(ProgramAction::SetReverse { 
                            ball_reference: ball_reference.clone(), 
                            speed: speed_val 
                        });
                    }
                }
                Instruction::SpawnBall { x, y, speed, direction } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    let speed_val = self.evaluate_expression(speed, context);
                    let dir_val = self.evaluate_expression(direction, context);
                    
                    if let (Value::Number(x), Value::Number(y), Value::Number(s), Value::Direction(d)) = 
                        (x_val, y_val, speed_val, dir_val) {
                        actions.push(ProgramAction::SpawnBall { x, y, speed: s, direction: d });
                    }
                }
                Instruction::CreateBall { x, y, speed, direction } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    let speed_val = self.evaluate_expression(speed, context);
                    let dir_val = self.evaluate_expression(direction, context);
                    
                    if let (Value::Number(x), Value::Number(y), Value::Number(s), Value::Direction(d)) = 
                        (x_val, y_val, speed_val, dir_val) {
                        actions.push(ProgramAction::CreateBall { x, y, speed: s, direction: d });
                    }
                }
                Instruction::CreateSquare { x, y } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    
                    if let (Value::Number(x), Value::Number(y)) = (x_val, y_val) {
                        actions.push(ProgramAction::CreateSquare { x: x as i32, y: y as i32 });
                    }
                }
                Instruction::CreateSquareWithProgram { x, y, program } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    
                    if let (Value::Number(x), Value::Number(y)) = (x_val, y_val) {
                        actions.push(ProgramAction::CreateSquareWithProgram { x: x as i32, y: y as i32, program: program.clone() });
                    }
                }
                Instruction::CreateBallWithLibrary { x, y, library_function, audio_file } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    
                    if let (Value::Number(x), Value::Number(y)) = (x_val, y_val) {
                        actions.push(ProgramAction::CreateBallWithLibrary { 
                            x, 
                            y, 
                            library_function: library_function.clone(), 
                            audio_file: audio_file.clone() 
                        });
                    }
                }
                Instruction::CreateSquareWithLibrary { x, y, library_function, audio_file } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    
                    if let (Value::Number(x), Value::Number(y)) = (x_val, y_val) {
                        actions.push(ProgramAction::CreateSquareWithLibrary { 
                            x, 
                            y, 
                            library_function: library_function.clone(), 
                            audio_file: audio_file.clone() 
                        });
                    }
                }
                Instruction::CreateBallFromSample { x, y, library_name, sample_name } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    
                    if let (Value::Number(x), Value::Number(y)) = (x_val, y_val) {
                        actions.push(ProgramAction::CreateBallFromSample { 
                            x: x as i32, 
                            y: y as i32, 
                            library_name: library_name.clone(), 
                            sample_name: sample_name.clone() 
                        });
                    }
                }
                Instruction::CreateSquareFromSample { x, y, library_name, sample_name } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    
                    if let (Value::Number(x), Value::Number(y)) = (x_val, y_val) {
                        actions.push(ProgramAction::CreateSquareFromSample { 
                            x: x as i32, 
                            y: y as i32, 
                            library_name: library_name.clone(), 
                            sample_name: sample_name.clone() 
                        });
                    }
                }
                Instruction::DestroyBall { target } => {
                    match target {
                        DestroyTarget::Coordinates { x, y } => {
                            let x_val = self.evaluate_expression(x, context);
                            let y_val = self.evaluate_expression(y, context);
                            let x_f32 = match x_val { Value::Number(n) => n, _ => 0.0 };
                            let y_f32 = match y_val { Value::Number(n) => n, _ => 0.0 };
                            actions.push(ProgramAction::DestroyBall { x: x_f32, y: y_f32, ball_reference: None });
                        }
                        DestroyTarget::BallReference(ball_ref) => {
                            actions.push(ProgramAction::DestroyBall { x: 0.0, y: 0.0, ball_reference: Some(ball_ref.clone()) });
                        }
                    }
                }
                Instruction::DestroySquare { target } => {
                    match target {
                        DestroyTarget::Coordinates { x, y } => {
                            let x_val = self.evaluate_expression(x, context);
                            let y_val = self.evaluate_expression(y, context);
                            let x_f32 = match x_val { Value::Number(n) => n, _ => 0.0 };
                            let y_f32 = match y_val { Value::Number(n) => n, _ => 0.0 };
                            actions.push(ProgramAction::DestroySquare { x: x_f32, y: y_f32, ball_reference: None });
                        }
                        DestroyTarget::BallReference(ball_ref) => {
                            actions.push(ProgramAction::DestroySquare { x: 0.0, y: 0.0, ball_reference: Some(ball_ref.clone()) });
                        }
                    }
                }
                Instruction::Print(expr) => {
                    println!("DEBUG SQUARE: Print instruction with expression: {:?}", expr);
                    let val = self.evaluate_expression(expr, context);
                    println!("DEBUG SQUARE: Evaluated expression to value: {:?}", val);
                    let display_text = match val {
                        Value::Number(n) => n.to_string(),
                        Value::Boolean(b) => b.to_string(),
                        Value::Direction(d) => format!("{:?}", d),
                        Value::String(s) => s,
                        Value::Coordinate(x, y) => format!("({}, {})", x, y),
                    };
                    println!("DEBUG SQUARE: Final display text: {}", display_text);
                    actions.push(ProgramAction::Print(display_text));
                }
                Instruction::ExecuteProgram(program) => {
                    actions.push(ProgramAction::ExecuteProgram(program.clone()));
                }
                Instruction::ContinueToNext => {
                    actions.push(ProgramAction::ContinueToNext);
                }
                Instruction::ExecuteLibraryFunction { library_function } => {
                    actions.push(ProgramAction::ExecuteLibraryFunction { 
                        library_function: library_function.clone() 
                    });
                }
                Instruction::Return(function_name) => {
                    actions.push(ProgramAction::Return(function_name.clone()));
                    break; // Exit the instruction loop immediately
                }
                Instruction::End => {
                    actions.push(ProgramAction::End);
                    break; // Exit the instruction loop immediately
                }
            }
        }
        
        actions
    }
    
    fn evaluate_expression(&self, expr: &Expression, context: &ExecutionContext) -> Value {
        match expr {
            Expression::Literal(value) => value.clone(),
            Expression::Variable(name) => {
                context.variables.get(name).cloned().unwrap_or(Value::Number(0.0))
            }
            Expression::GlobalVariable(name) => {
                // For now, global variables are not accessible in square.rs context
                // This would need to be passed through the context or handled differently
                // Defaulting to 0.0 for now
                Value::Number(0.0)
            }
            Expression::BinaryOp { left, op, right } => {
                let left_val = self.evaluate_expression(left, context);
                let right_val = self.evaluate_expression(right, context);
                self.apply_binary_op(&left_val, *op, &right_val)
            }
            Expression::BallProperty(prop) => {
                match prop {
                    BallProperty::Speed => Value::Number(context.ball_speed),
                    BallProperty::Direction => Value::Direction(context.ball_direction),
                    BallProperty::X => Value::Number(context.ball_x),
                    BallProperty::Y => Value::Number(context.ball_y),
                    BallProperty::HitCount => Value::Number(context.ball_hit_count as f32),
                    BallProperty::Pitch => Value::Number(context.ball_pitch),
                    BallProperty::Volume => Value::Number(context.ball_volume),
                }
            }
            Expression::Random { min, max } => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                Value::Number(rng.gen_range(*min..*max))
            }
        }
    }
    
    fn apply_binary_op(&self, left: &Value, op: BinaryOperator, right: &Value) -> Value {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => {
                match op {
                    BinaryOperator::Add => Value::Number(a + b),
                    BinaryOperator::Sub => Value::Number(a - b),
                    BinaryOperator::Mul => Value::Number(a * b),
                    BinaryOperator::Div => Value::Number(if *b != 0.0 { a / b } else { 0.0 }),
                    BinaryOperator::Mod => Value::Number(a % b),
                    BinaryOperator::Equal => Value::Boolean((a - b).abs() < f32::EPSILON),
                    BinaryOperator::NotEqual => Value::Boolean((a - b).abs() >= f32::EPSILON),
                    BinaryOperator::Less => Value::Boolean(a < b),
                    BinaryOperator::Greater => Value::Boolean(a > b),
                    BinaryOperator::LessEqual => Value::Boolean(a <= b),
                    BinaryOperator::GreaterEqual => Value::Boolean(a >= b),
                    _ => Value::Boolean(false),
                }
            }
            (Value::Boolean(a), Value::Boolean(b)) => {
                match op {
                    BinaryOperator::And => Value::Boolean(*a && *b),
                    BinaryOperator::Or => Value::Boolean(*a || *b),
                    BinaryOperator::Equal => Value::Boolean(a == b),
                    BinaryOperator::NotEqual => Value::Boolean(a != b),
                    _ => Value::Boolean(false),
                }
            }
            _ => Value::Boolean(false),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum ProgramAction {
    SetSpeed(f32),
    SetDirection(crate::ball::Direction),
    SetDirectionToCoordinate { target_x: f32, target_y: f32 },
    SetPitch(f32),
    SetVolume(f32),
    SetColor(String),
    Bounce,
    Stop,
    PlaySample(usize),
    SetReverse { ball_reference: String, speed: f32 },
    SetSliceArray { x: usize, y: usize, markers: Vec<u32> },
    PlaySliceMarker { x: usize, y: usize, marker_index: u32 },
    SpawnBall { x: f32, y: f32, speed: f32, direction: crate::ball::Direction },
    CreateBall { x: f32, y: f32, speed: f32, direction: crate::ball::Direction },
    CreateSquare { x: i32, y: i32 },
    CreateSquareWithProgram { x: i32, y: i32, program: Program },
    CreateBallFromSample { x: i32, y: i32, library_name: String, sample_name: String },
    CreateSquareFromSample { x: i32, y: i32, library_name: String, sample_name: String },
    CreateBallWithLibrary { x: f32, y: f32, library_function: String, audio_file: Option<String> },
    CreateSquareWithLibrary { x: f32, y: f32, library_function: String, audio_file: Option<String> },
    DestroyBall { x: f32, y: f32, ball_reference: Option<String> },
    DestroySquare { x: f32, y: f32, ball_reference: Option<String> },
    Print(String),
    ExecuteProgram(Program),
    ExecuteLibraryFunction { library_function: String },
    SetGlobalVariable { name: String, value: Value },
    ContinueToNext,
    Return(Option<String>), // None = simple return, Some(name) = call function and return
    End, // Natural end of block
    None,
}



#[derive(Clone, Debug)]
pub struct Cell {
    pub content: CellContent,
    pub color: [u8; 3], // RGB color
    pub program: SquareProgram, // Programming for square effects
    pub display_text: Option<String>, // Text to display on the square
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            content: CellContent::Empty,
            color: [100, 100, 100], // Default gray color
            program: SquareProgram::default(),
            display_text: None,
        }
    }
}

impl Cell {
    pub fn new_square(color: [u8; 3]) -> Self {
        Self {
            content: CellContent::Square,
            color,
            program: SquareProgram::default(),
            display_text: None,
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
        self.program = SquareProgram::default();
        self.display_text = None;
    }
    
    pub fn place_square(&mut self, color: Option<[u8; 3]>) {
        self.content = CellContent::Square;
        if let Some(c) = color {
            self.color = c;
        } else {
            self.color = [255, 255, 255]; // Default white square
        }
        self.program = SquareProgram::default();
    }
    
    pub fn set_program(&mut self, program: SquareProgram) {
        self.program = program;
    }
    
    pub fn get_program(&self) -> &SquareProgram {
        &self.program
    }
}