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
}

#[derive(Clone, PartialEq, Debug)]
pub enum Expression {
    Literal(Value),
    Variable(String),
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
}

#[derive(Clone, PartialEq, Debug)]
pub enum Instruction {
    // Ball manipulation
    SetSpeed(Expression),
    SetDirection(Expression),
    Bounce,
    Stop,
    
    // Variables
    SetVariable { name: String, value: Expression },
    
    // Control flow
    If { condition: Expression, then_block: Vec<Instruction>, else_block: Option<Vec<Instruction>> },
    Loop { count: Expression, body: Vec<Instruction> },
    
    // Audio
    PlaySample(Expression),
    
    // Grid interaction
    SpawnBall { x: Expression, y: Expression, speed: Expression, direction: Expression },
    
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

use std::collections::HashMap;

#[derive(Clone, PartialEq, Debug)]
pub struct Program {
    pub instructions: Vec<Instruction>,
    pub name: String,
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
    
    pub fn get_program(&self, index: usize) -> Option<&Program> {
        self.programs.get(index)
    }
    
    pub fn get_program_mut(&mut self, index: usize) -> Option<&mut Program> {
        self.programs.get_mut(index)
    }
    
    pub fn set_active_program(&mut self, index: Option<usize>) {
        self.active_program = index;
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
    
    fn execute_instructions(&self, instructions: &[Instruction], context: &mut ExecutionContext) -> Vec<ProgramAction> {
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
                Instruction::PlaySample(expr) => {
                    if let Value::Number(index) = self.evaluate_expression(expr, context) {
                        actions.push(ProgramAction::PlaySample(index as usize));
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
                Instruction::Print(expr) => {
                    let val = self.evaluate_expression(expr, context);
                    actions.push(ProgramAction::Print(format!("{:?}", val)));
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
    Bounce,
    Stop,
    PlaySample(usize),
    SpawnBall { x: f32, y: f32, speed: f32, direction: crate::ball::Direction },
    Print(String),
    ExecuteProgram(Program),
    None,
}



#[derive(Clone, Debug)]
pub struct Cell {
    pub content: CellContent,
    pub color: [u8; 3], // RGB color
    pub program: SquareProgram, // Programming for square effects
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            content: CellContent::Empty,
            color: [100, 100, 100], // Default gray color
            program: SquareProgram::default(),
        }
    }
}

impl Cell {
    pub fn new_square(color: [u8; 3]) -> Self {
        Self {
            content: CellContent::Square,
            color,
            program: SquareProgram::default(),
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