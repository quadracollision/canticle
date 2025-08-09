use std::collections::HashMap;
use crate::ball::{Ball, Direction};
use crate::square::{Value, Expression, Instruction, BinaryOperator, BallProperty, Program, ExecutionContext, ProgramAction};
// Grid dimensions are available from the sequencer module if needed

#[derive(Clone, Debug)]
pub struct ProgrammerState {
    pub variables: HashMap<String, Value>,
    pub ball_hit_counts: HashMap<String, u32>, // Track hits per ball color
    pub square_hit_counts: HashMap<(usize, usize), u32>, // Track hits per square position
}

impl Default for ProgrammerState {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            ball_hit_counts: HashMap::new(),
            square_hit_counts: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SimpleProgramParser;

impl SimpleProgramParser {
    pub fn new() -> Self {
        Self
    }
    
    /// Parse a simple program like:
    /// def speed_increase
    /// if c_red hits self 10 times
    /// set speed relative +0.1
    /// return
    pub fn parse_program(&self, source: &str) -> Result<Program, String> {
        let lines: Vec<&str> = source.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        
        if lines.is_empty() {
            return Err("Empty program".to_string());
        }
        
        // First line should be "def function_name"
        let first_line = lines[0];
        if !first_line.starts_with("def ") {
            return Err("Program must start with 'def function_name'".to_string());
        }
        
        let function_name = first_line[4..].trim().to_string();
        let mut instructions = Vec::new();
        
        let mut i = 1;
        while i < lines.len() {
            let line = lines[i];
            
            if line == "return" {
                break;
            }
            
            if let Ok(instruction) = self.parse_line(line) {
                instructions.push(instruction);
            } else {
                return Err(format!("Failed to parse line: {}", line));
            }
            
            i += 1;
        }
        
        Ok(Program {
            name: function_name,
            instructions,
        })
    }
    
    fn parse_line(&self, line: &str) -> Result<Instruction, String> {
        let line = line.trim();
        
        // Handle "if" statements
        if line.starts_with("if ") {
            return self.parse_if_statement(line);
        }
        
        // Handle "set" statements
        if line.starts_with("set ") {
            return self.parse_set_statement(line);
        }
        
        Err(format!("Unknown instruction: {}", line))
    }
    
    fn parse_if_statement(&self, line: &str) -> Result<Instruction, String> {
        // Parse "if c_red hits self 10 times"
        let condition_part = &line[3..].trim(); // Remove "if "
        
        // Simple parsing for "color hits target count times"
        let parts: Vec<&str> = condition_part.split_whitespace().collect();
        if parts.len() >= 5 && parts[1] == "hits" && parts[4] == "times" {
            let color = parts[0];
            let target = parts[2]; // "self" or "sq(x,y)"
            let count_str = parts[3];
            
            if let Ok(count) = count_str.parse::<u32>() {
                // Create a condition that checks hit count
                let condition = self.create_hit_condition(color, target, count)?;
                
                // For now, we'll assume the next instruction is the "then" block
                // In a real parser, we'd need to handle multi-line blocks
                let then_block = vec![Instruction::Bounce]; // Placeholder
                
                return Ok(Instruction::If {
                    condition,
                    then_block,
                    else_block: None,
                });
            }
        }
        
        Err("Invalid if statement format".to_string())
    }
    
    fn create_hit_condition(&self, _color: &str, _target: &str, count: u32) -> Result<Expression, String> {
        // For now, create a simple condition that checks ball hit count
        // This would need to be expanded to handle different ball colors and targets
        Ok(Expression::BinaryOp {
            left: Box::new(Expression::BallProperty(BallProperty::HitCount)),
            op: BinaryOperator::GreaterEqual,
            right: Box::new(Expression::Literal(Value::Number(count as f32))),
        })
    }
    
    fn parse_set_statement(&self, line: &str) -> Result<Instruction, String> {
        // Parse "set speed relative +0.1"
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() >= 3 && parts[0] == "set" {
            let property = parts[1];
            
            match property {
                "speed" => {
                    if parts.len() >= 4 && parts[2] == "relative" {
                        // Relative speed change
                        let change_str = parts[3];
                        if let Ok(change) = change_str.parse::<f32>() {
                            return Ok(Instruction::SetSpeed(Expression::BinaryOp {
                                left: Box::new(Expression::BallProperty(BallProperty::Speed)),
                                op: BinaryOperator::Add,
                                right: Box::new(Expression::Literal(Value::Number(change))),
                            }));
                        }
                    } else if parts.len() >= 3 {
                        // Absolute speed change
                        if let Ok(speed) = parts[2].parse::<f32>() {
                            return Ok(Instruction::SetSpeed(Expression::Literal(Value::Number(speed))));
                        }
                    }
                }
                "direction" => {
                    if parts.len() >= 3 {
                        let direction = match parts[2] {
                            "up" => Direction::Up,
                            "down" => Direction::Down,
                            "left" => Direction::Left,
                            "right" => Direction::Right,
                            "up-left" => Direction::UpLeft,
                            "up-right" => Direction::UpRight,
                            "down-left" => Direction::DownLeft,
                            "down-right" => Direction::DownRight,
                            _ => return Err(format!("Unknown direction: {}", parts[2])),
                        };
                        return Ok(Instruction::SetDirection(Expression::Literal(Value::Direction(direction))));
                    }
                }
                _ => return Err(format!("Unknown property: {}", property)),
            }
        }
        
        Err("Invalid set statement format".to_string())
    }
}

#[derive(Clone, Debug)]
pub struct ProgramExecutor {
    pub state: ProgrammerState,
}

impl ProgramExecutor {
    pub fn new() -> Self {
        Self {
            state: ProgrammerState::default(),
        }
    }
    
    pub fn execute_on_collision(
        &mut self,
        program: &Program,
        ball: &Ball,
        square_x: usize,
        square_y: usize,
    ) -> Vec<ProgramAction> {
        // Update hit counts
        let ball_color = self.get_ball_color(ball);
        *self.state.ball_hit_counts.entry(ball_color).or_insert(0) += 1;
        *self.state.square_hit_counts.entry((square_x, square_y)).or_insert(0) += 1;
        
        // Create execution context
        let mut context = ExecutionContext {
            variables: self.state.variables.clone(),
            ball_hit_count: *self.state.ball_hit_counts.get(&self.get_ball_color(ball)).unwrap_or(&0),
            square_hit_count: *self.state.square_hit_counts.get(&(square_x, square_y)).unwrap_or(&0),
            ball_x: ball.x,
            ball_y: ball.y,
            ball_speed: ball.speed,
            ball_direction: ball.direction,
        };
        
        // Execute the program
        let actions = self.execute_instructions(&program.instructions, &mut context);
        
        // Update state with any variable changes
        self.state.variables = context.variables;
        
        actions
    }
    
    fn get_ball_color(&self, _ball: &Ball) -> String {
        // For now, we'll use a simple color mapping based on ball properties
        // In the future, this could be expanded to support actual ball colors
        "c_red".to_string() // Default color
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
                Instruction::If { condition, then_block, else_block } => {
                    if let Value::Boolean(true) = self.evaluate_expression(condition, context) {
                        actions.extend(self.execute_instructions(then_block, context));
                    } else if let Some(else_instructions) = else_block {
                        actions.extend(self.execute_instructions(else_instructions, context));
                    }
                }
                Instruction::SetVariable { name, value } => {
                    let val = self.evaluate_expression(value, context);
                    context.variables.insert(name.clone(), val);
                }
                _ => {} // Handle other instructions as needed
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