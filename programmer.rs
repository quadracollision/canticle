use std::collections::HashMap;
use crate::ball::{Ball, Direction};
use crate::square::{Value, Expression, Instruction, BinaryOperator, BallProperty, Program, ExecutionContext, ProgramAction, DestroyTarget};
// Grid dimensions are available from the sequencer module if needed

#[derive(Clone, Debug)]
pub struct ProgrammerState {
    pub variables: HashMap<String, Value>,
    pub ball_hit_counts: HashMap<String, u32>, // Track hits per ball color (global)
    pub square_hit_counts: HashMap<(usize, usize), u32>, // Track hits per square position
    pub ball_color_square_hits: HashMap<(String, usize, usize), u32>, // Track hits per ball color per square
}

impl Default for ProgrammerState {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            ball_hit_counts: HashMap::new(),
            square_hit_counts: HashMap::new(),
            ball_color_square_hits: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SimpleProgramParser;

impl SimpleProgramParser {
    pub fn new() -> Self {
        Self
    }
    
    // Available colors that can be referenced in programs
    const VALID_COLORS: &'static [&'static str] = &["Red", "Green", "Blue", "Yellow", "Cyan", "Magenta", "White", "Orange"];
    
    fn color_to_prefix(&self, color: &str) -> String {
        // Convert color name to c_ prefix format (e.g., "Red" -> "c_red")
        if color.starts_with("c_") {
            // Already in c_ format, validate the base color
            let base_color = &color[2..];
            let capitalized = format!("{}{}", base_color.chars().next().unwrap().to_uppercase(), &base_color[1..].to_lowercase());
            if Self::VALID_COLORS.contains(&capitalized.as_str()) {
                color.to_string()
            } else {
                format!("c_{}", base_color.to_lowercase())
            }
        } else {
            format!("c_{}", color.to_lowercase())
        }
    }
    
    fn validate_color(&self, color: &str) -> Result<String, String> {
        // Handle both "Red" and "c_red" formats
        let normalized_color = if color.starts_with("c_") {
            let base_color = &color[2..];
            format!("{}{}", base_color.chars().next().unwrap().to_uppercase(), &base_color[1..].to_lowercase())
        } else {
            color.to_string()
        };
        
        if Self::VALID_COLORS.contains(&normalized_color.as_str()) {
            Ok(self.color_to_prefix(&normalized_color))
        } else {
            Err(format!("Invalid color '{}'. Valid colors are: {}", color, Self::VALID_COLORS.join(", ")))
        }
    }
    
    /// Parse a simple program like:
    /// def speed_increase
    /// if c_red hits self 1 times
    /// set speed relative +0.1
    /// and
    /// if c_red hits self 1 times
    /// set speed relative 10
    /// then
    /// def example
    /// return
    pub fn parse_program(&self, source: &str) -> Result<Program, String> {
        let programs = self.parse_multiple_programs(source)?;
        if programs.is_empty() {
            return Err("No programs found".to_string());
        }
        // Return the first program for backward compatibility
        Ok(programs[0].clone())
    }
    
    /// Parse multiple function definitions from the same source text
    pub fn parse_multiple_programs(&self, source: &str) -> Result<Vec<Program>, String> {
        let lines: Vec<&str> = source.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        
        if lines.is_empty() {
            return Err("Empty program".to_string());
        }
        
        let mut programs = Vec::new();
        let mut i = 0;
        
        while i < lines.len() {
            let line = lines[i];
            
            if line.starts_with("def ") {
                let function_name = line[4..].trim().to_string();
                let (instructions, next_i) = self.parse_block(&lines, i + 1)?;
                
                programs.push(Program {
                    name: function_name,
                    instructions,
                    source_text: None, // Parser doesn't preserve original text
                });
                
                i = next_i;
            } else {
                return Err(format!("Expected 'def function_name', found: {}", line));
            }
        }
        
        if programs.is_empty() {
            return Err("No function definitions found".to_string());
        }
        
        Ok(programs)
    }
    
    fn parse_block(&self, lines: &[&str], start_index: usize) -> Result<(Vec<Instruction>, usize), String> {
        let mut instructions = Vec::new();
        let mut i = start_index;
        
        while i < lines.len() {
            let line = lines[i];
            
            if line == "return" {
                instructions.push(Instruction::Return(None));
                i += 1;
                break;
            }
            
            if line.starts_with("return ") {
                let function_name = line[7..].trim().to_string();
                instructions.push(Instruction::Return(Some(function_name)));
                i += 1;
                break;
            }
            
            if line == "end" {
                instructions.push(Instruction::End);
                i += 1;
                break;
            }
            
            // Handle if statements with potential then blocks
            if line.starts_with("if ") {
                let (if_instruction, next_i) = self.parse_if_with_then(lines, i)?;
                instructions.push(if_instruction);
                i = next_i;
                continue;
            }
            
            // Handle nested function definitions - skip them as they should be parsed separately
            if line.starts_with("def ") {
                // This is a nested function definition, which should be handled at the top level
                // Skip to the end of this function block
                let mut depth = 1;
                i += 1;
                while i < lines.len() && depth > 0 {
                    let current_line = lines[i];
                    if current_line == "end" || current_line.starts_with("return") {
                        depth -= 1;
                    }
                    i += 1;
                }
                continue;
            }
            
            // Handle create square with embedded program
            if line.starts_with("create square(") && line.contains("with") {
                let (create_instruction, next_i) = self.parse_create_square_with_program(lines, i)?;
                instructions.push(create_instruction);
                i = next_i;
                continue;
            }
            
            if let Ok(instruction) = self.parse_line(line) {
                instructions.push(instruction);
            } else {
                return Err(format!("Failed to parse line: {}", line));
            }
            
            i += 1;
        }
        
        Ok((instructions, i))
    }
    
    fn parse_nested_function(&self, lines: &[&str], start_index: usize) -> Result<(Program, usize), String> {
        let line = lines[start_index];
        if !line.starts_with("def ") {
            return Err("Expected function definition".to_string());
        }
        
        let function_name = line[4..].trim().to_string();
        let (instructions, next_i) = self.parse_block(lines, start_index + 1)?;
        
        Ok((Program {
            name: function_name,
            instructions,
            source_text: None, // Parser doesn't preserve original text
        }, next_i))
    }
    
    fn parse_create_square_with_program(&self, lines: &[&str], start_index: usize) -> Result<(Instruction, usize), String> {
        let first_line = lines[start_index];
        
        // Parse "create square(3, 4) with def n"
        if let Some(with_pos) = first_line.find("with") {
            let create_part = &first_line[..with_pos].trim();
            let def_part = &first_line[with_pos + 4..].trim(); // Skip "with "
            
            // Parse coordinates from create_part
            let content = &create_part[7..].trim(); // Remove "create "
            if let Some(paren_pos) = content.find('(') {
                if let Some(close_paren) = content.find(')') {
                    let object_type = content[..paren_pos].trim();
                    let coords_str = &content[paren_pos + 1..close_paren].trim();
                    
                    if object_type == "square" {
                        let coords: Vec<&str> = coords_str.split(',').map(|s| s.trim()).collect();
                        if coords.len() == 2 {
                            let x_expr = self.parse_coordinate_expression(coords[0])?;
                            let y_expr = self.parse_coordinate_expression(coords[1])?;
                            
                            // Parse the embedded program starting from def_part
                            if !def_part.starts_with("def ") {
                                return Err("Expected 'def function_name' after 'with'".to_string());
                            }
                            
                            let function_name = def_part[4..].trim().to_string();
                            let (instructions, end_index) = self.parse_block(lines, start_index + 1)?;
                            
                            let embedded_program = Program {
                name: function_name,
                instructions,
                source_text: None, // Parser doesn't preserve original text
            };
                            
                            return Ok((Instruction::CreateSquareWithProgram {
                                x: x_expr,
                                y: y_expr,
                                program: embedded_program,
                            }, end_index));
                        }
                    }
                }
            }
        }
        
        Err("Invalid create square with program syntax. Expected: create square(x,y) with def function_name".to_string())
    }
    
    fn parse_if_with_then(&self, lines: &[&str], start_index: usize) -> Result<(Instruction, usize), String> {
        let line = lines[start_index];
        let condition = self.parse_if_condition(line)?;
        
        let mut i = start_index + 1;
        let mut then_block = Vec::new();
        
        // Look for immediate instructions, 'and' keywords, or 'then' keyword
        while i < lines.len() {
            let current_line = lines[i];
            
            if current_line == "then" {
                // 'then' means continue to next function in sequence
                then_block.push(Instruction::ContinueToNext);
                i += 1;
                break;
            } else if current_line.starts_with("then ") {
                // 'then N' means repeat the previous instructions N times
                let count_str = current_line[5..].trim();
                if let Ok(count) = count_str.parse::<f32>() {
                    if !then_block.is_empty() {
                        let repeat_body = then_block.clone();
                        then_block.clear();
                        then_block.push(Instruction::RepeatThen {
                            count: Expression::Literal(Value::Number(count)),
                            body: repeat_body,
                        });
                    }
                } else {
                    return Err(format!("Invalid number in 'then {}'", count_str));
                }
                i += 1;
                break;
            } else if current_line == "and" {
                // 'and' means continue to next instruction in the same block
                i += 1;
                continue;
            } else if current_line.starts_with("and ") {
                // 'and N' means repeat the previous instructions N times
                let count_str = current_line[4..].trim();
                if let Ok(count) = count_str.parse::<f32>() {
                    if !then_block.is_empty() {
                        let repeat_body = then_block.clone();
                        then_block.clear();
                        then_block.push(Instruction::RepeatAnd {
                            count: Expression::Literal(Value::Number(count)),
                            body: repeat_body,
                        });
                    }
                } else {
                    return Err(format!("Invalid number in 'and {}'", count_str));
                }
                i += 1;
                continue;
            } else if current_line == "return" || current_line.starts_with("if ") || current_line.starts_with("def ") {
                // End of if block without explicit then
                break;
            } else {
                // Parse instruction and continue looking for 'and' or 'then'
                if let Ok(instruction) = self.parse_line(current_line) {
                    then_block.push(instruction);
                    i += 1;
                    // Don't break here - continue to look for 'and' or 'then'
                } else {
                    return Err(format!("Failed to parse instruction in if block: {}", current_line));
                }
            }
        }
        
        Ok((Instruction::If {
            condition,
            then_block,
            else_block: None,
        }, i))
    }
    
    fn parse_if_condition(&self, line: &str) -> Result<Expression, String> {
        // Parse "if c_red hits self 10 times"
        let condition_part = &line[3..].trim(); // Remove "if "
        
        // Simple parsing for "color hits target count times"
        let parts: Vec<&str> = condition_part.split_whitespace().collect();
        if parts.len() >= 5 && parts[1] == "hits" && parts[4] == "times" {
            let color = parts[0];
            let target = parts[2]; // "self" or "sq(x,y)"
            let count_str = parts[3];
            
            if let Ok(count) = count_str.parse::<u32>() {
                return self.create_hit_condition(color, target, count);
            }
        }
        
        Err("Invalid if statement format".to_string())
    }
    
    fn parse_line(&self, line: &str) -> Result<Instruction, String> {
        let line = line.trim();
        
        // Handle "set" statements
        if line.starts_with("set ") {
            return self.parse_set_statement(line);
        }
        
        // Note: 'reverse sample of' syntax has been removed
        // Use 'set reverse ball_reference speed' instead
        
        // Handle "create" statements
        if line.starts_with("create ") {
            return self.parse_create_statement(line);
        }
        
        // Handle "destroy" statements
        if line.starts_with("destroy ") {
            return self.parse_destroy_statement(line);
        }
        
        // Handle library function calls
        if line.starts_with("lib.") {
            let library_function = line.to_string();
            return Ok(Instruction::ExecuteLibraryFunction { library_function });
        }
        
        Err(format!("Unknown instruction: {}", line))
    }
    
    fn create_hit_condition(&self, color: &str, _target: &str, count: u32) -> Result<Expression, String> {
        // Validate the color first
        let _validated_color = self.validate_color(color)?;
        
        // Create a condition that checks ball hit count for the specific color
        // Note: This still uses the general hit count for now, but the color validation ensures
        // only valid colors are accepted in the program syntax
        Ok(Expression::BinaryOp {
            left: Box::new(Expression::BallProperty(BallProperty::HitCount)),
            op: BinaryOperator::Equal,
            right: Box::new(Expression::Literal(Value::Number(count as f32))),
        })
    }
    
    fn parse_set_statement(&self, line: &str) -> Result<Instruction, String> {
        // Parse "set speed +0.1" or "set speed 2.0"
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() >= 3 && parts[0] == "set" {
            let property = parts[1];
            
            match property {
                "speed" => {
                    let speed_str = parts[2];
                    
                    // Check if it starts with + or - for relative change
                    if speed_str.starts_with('+') || speed_str.starts_with('-') {
                        // Relative speed change
                        if let Ok(change) = speed_str.parse::<f32>() {
                            return Ok(Instruction::SetSpeed(Expression::BinaryOp {
                                left: Box::new(Expression::BallProperty(BallProperty::Speed)),
                                op: BinaryOperator::Add,
                                right: Box::new(Expression::Literal(Value::Number(change))),
                            }));
                        }
                    } else {
                        // Absolute speed change
                        if let Ok(speed) = speed_str.parse::<f32>() {
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
                "reverse" => {
                    // Parse "set reverse ball_reference speed"
                    if parts.len() >= 4 {
                        let ball_reference = parts[2].to_string();
                        let speed_str = parts[3];
                        
                        if let Ok(speed) = speed_str.parse::<f32>() {
                            return Ok(Instruction::SetReverse {
                                ball_reference,
                                speed: Expression::Literal(Value::Number(speed)),
                            });
                        } else {
                            return Err(format!("Invalid speed value: {}", speed_str));
                        }
                    } else {
                        return Err("Invalid reverse statement format. Expected: set reverse ball_reference speed".to_string());
                    }
                }
                _ => return Err(format!("Unknown property: {}", property)),
            }
        }
        
        Err("Invalid set statement format".to_string())
    }
    
    // Note: parse_reverse_sample_statement has been removed
    // Use 'set reverse ball_reference speed' syntax instead
    
    fn parse_coordinate_expression(&self, coord_str: &str) -> Result<Expression, String> {
        let coord_str = coord_str.trim();
        
        // Check for ball properties
        if coord_str == "x" {
            return Ok(Expression::BallProperty(BallProperty::X));
        }
        if coord_str == "y" {
            return Ok(Expression::BallProperty(BallProperty::Y));
        }
        
        // Check for arithmetic expressions like "x+1", "y-2", etc.
        for op_char in ['+', '-', '*', '/', '%'] {
            if let Some(op_pos) = coord_str.find(op_char) {
                let left_str = coord_str[..op_pos].trim();
                let right_str = coord_str[op_pos + 1..].trim();
                
                let left_expr = self.parse_coordinate_expression(left_str)?;
                let right_expr = self.parse_coordinate_expression(right_str)?;
                
                let op = match op_char {
                    '+' => BinaryOperator::Add,
                    '-' => BinaryOperator::Sub,
                    '*' => BinaryOperator::Mul,
                    '/' => BinaryOperator::Div,
                    '%' => BinaryOperator::Mod,
                    _ => return Err(format!("Unsupported operator: {}", op_char)),
                };
                
                return Ok(Expression::BinaryOp {
                    left: Box::new(left_expr),
                    op,
                    right: Box::new(right_expr),
                });
            }
        }
        
        // Try to parse as a literal number
        if let Ok(num) = coord_str.parse::<f32>() {
            return Ok(Expression::Literal(Value::Number(num)));
        }
        
        // Try to parse as a variable
        Ok(Expression::Variable(coord_str.to_string()))
    }
    
    fn parse_create_statement(&self, line: &str) -> Result<Instruction, String> {
        // Parse "create ball(3,14)(self,self)", "create square(3, 17)", or "create ball from sample library.sample_name(3,4)"
        let content = &line[7..].trim(); // Remove "create "
        
        // Check for library-based creation syntax
        if content.contains(" from sample ") {
            return self.parse_create_from_sample(content);
        }
        
        if let Some(paren_pos) = content.find('(') {
            if let Some(close_paren) = content.find(')') {
                let object_type = content[..paren_pos].trim();
                let coords_str = &content[paren_pos + 1..close_paren].trim();
                
                // Parse coordinates
                let coords: Vec<&str> = coords_str.split(',').map(|s| s.trim()).collect();
                if coords.len() == 2 {
                    let x_expr = self.parse_coordinate_expression(coords[0])?;
                    let y_expr = self.parse_coordinate_expression(coords[1])?;
                        match object_type {
                            "ball" => {
                                // Check for speed and direction parameters or library references
                                let remaining = &content[close_paren + 1..];
                                if remaining.starts_with('(') {
                                    // Parse speed and direction: (speed,direction)
                                    if let Some(second_close) = remaining.find(')') {
                                        let params_str = &remaining[1..second_close].trim();
                                        let params: Vec<&str> = params_str.split(',').map(|s| s.trim()).collect();
                                        if params.len() == 2 {
                                            let speed_expr = self.parse_speed_expression(params[0])?;
                                            let direction_expr = self.parse_direction_expression(params[1])?;
                                            return Ok(Instruction::CreateBall {
                                                x: x_expr,
                                                y: y_expr,
                                                speed: speed_expr,
                                                direction: direction_expr,
                                            });
                                        }
                                    }
                                    return Err("Invalid ball parameters. Expected: create ball(x,y)(speed,direction)".to_string());
                                } else if remaining.trim().starts_with("with") {
                                    // Check if it's a library reference like "with lib.ballcreator and lib.kick4.wav"
                                    if remaining.contains("lib.") {
                                        // For library references, we need to evaluate expressions to get literal values
                                        // This is a limitation - library references currently expect literal coordinates
                                        return Err("Library references with dynamic coordinates not yet supported".to_string());
                                    }
                                    return Err("Invalid 'with' syntax for ball creation".to_string());
                                } else {
                                    // Default values for backward compatibility
                                    return Ok(Instruction::CreateBall {
                                        x: x_expr,
                                        y: y_expr,
                                        speed: Expression::Literal(Value::Number(1.0)),
                                        direction: Expression::Literal(Value::Direction(Direction::Right)),
                                    });
                                }
                            }
                            "square" => {
                                // Check if there's a "with" keyword for library reference or embedded program
                                let remaining = &content[close_paren + 1..].trim();
                                if remaining.starts_with("with") {
                                    // Check if it's a library reference like "with lib.ballcreator and lib.kick4.wav"
                                    if remaining.contains("lib.") {
                                        // For library references, we need to evaluate expressions to get literal values
                                        // This is a limitation - library references currently expect literal coordinates
                                        return Err("Library references with dynamic coordinates not yet supported".to_string());
                                    } else {
                                        // This indicates we have an embedded program, but we need to handle this
                                        // at a higher level since we need access to multiple lines
                                        return Err("Square with embedded program detected - needs multi-line parsing".to_string());
                                    }
                                } else {
                                    return Ok(Instruction::CreateSquare {
                                        x: x_expr,
                                        y: y_expr,
                                    });
                                }
                            }
                            _ => return Err(format!("Unknown object type: {}", object_type)),
                        }
                } else {
                    return Err("Invalid coordinate format. Expected: (x,y)".to_string());
                }
            }
        }
        
        Err("Invalid create statement format. Expected: create ball(x,y)(speed,direction) or create square(x,y)".to_string())
    }
    
    fn parse_create_from_sample(&self, content: &str) -> Result<Instruction, String> {
        // Parse "ball from sample library.sample_name(3,4)" or "square from sample library.sample_name(3,4)"
        let parts: Vec<&str> = content.split(" from sample ").collect();
        if parts.len() != 2 {
            return Err("Invalid sample creation format. Expected: create [ball|square] from sample library.sample_name(x,y)".to_string());
        }
        
        let object_type = parts[0].trim();
        let sample_and_coords = parts[1].trim();
        
        // Find the coordinates part
        if let Some(paren_pos) = sample_and_coords.find('(') {
            if let Some(close_paren) = sample_and_coords.find(')') {
                let sample_ref = sample_and_coords[..paren_pos].trim();
                let coords_str = &sample_and_coords[paren_pos + 1..close_paren].trim();
                
                // Parse library.sample_name
                let sample_parts: Vec<&str> = sample_ref.split('.').collect();
                if sample_parts.len() != 2 {
                    return Err("Invalid sample reference format. Expected: library.sample_name".to_string());
                }
                
                let library_name = sample_parts[0].trim().to_string();
                let sample_name = sample_parts[1].trim().to_string();
                
                // Parse coordinates
                let coords: Vec<&str> = coords_str.split(',').map(|s| s.trim()).collect();
                if coords.len() == 2 {
                    if let (Ok(x), Ok(y)) = (coords[0].parse::<i32>(), coords[1].parse::<i32>()) {
                        match object_type {
                            "ball" => {
                                return Ok(Instruction::CreateBallFromSample {
                                    x: Expression::Literal(Value::Number(x as f32)),
                                    y: Expression::Literal(Value::Number(y as f32)),
                                    library_name,
                                    sample_name,
                                });
                            }
                            "square" => {
                                return Ok(Instruction::CreateSquareFromSample {
                                    x: Expression::Literal(Value::Number(x as f32)),
                                    y: Expression::Literal(Value::Number(y as f32)),
                                    library_name,
                                    sample_name,
                                });
                            }
                            _ => return Err(format!("Unknown object type for sample creation: {}", object_type)),
                        }
                    }
                }
            }
        }
        
        Err("Invalid sample creation format. Expected: create [ball|square] from sample library.sample_name(x,y)".to_string())
    }
    
    fn parse_speed_expression(&self, speed_str: &str) -> Result<Expression, String> {
        match speed_str {
            "self" => Ok(Expression::BallProperty(BallProperty::Speed)),
            _ => {
                if let Ok(speed) = speed_str.parse::<f32>() {
                    Ok(Expression::Literal(Value::Number(speed)))
                } else {
                    Err(format!("Invalid speed value: {}", speed_str))
                }
            }
        }
    }
    
    fn parse_direction_expression(&self, direction_str: &str) -> Result<Expression, String> {
        match direction_str {
            "self" => Ok(Expression::BallProperty(BallProperty::Direction)),
            "up" => Ok(Expression::Literal(Value::Direction(Direction::Up))),
            "down" => Ok(Expression::Literal(Value::Direction(Direction::Down))),
            "left" => Ok(Expression::Literal(Value::Direction(Direction::Left))),
            "right" => Ok(Expression::Literal(Value::Direction(Direction::Right))),
            "+1" => Ok(Expression::Literal(Value::Number(1.0))), // Treat as speed modifier for now
            "-1" => Ok(Expression::Literal(Value::Number(-1.0))),
            _ => {
                if let Ok(num) = direction_str.parse::<f32>() {
                    // Convert number to direction based on value
                    let dir = match (num as i32) % 4 {
                        0 => Direction::Right,
                        1 => Direction::Down,
                        2 => Direction::Left,
                        3 => Direction::Up,
                        _ => Direction::Right,
                    };
                    Ok(Expression::Literal(Value::Direction(dir)))
                } else {
                    Err(format!("Invalid direction value: {}", direction_str))
                }
            }
        }
    }
    
    fn parse_create_with_library_reference(&self, object_type: &str, x: f32, y: f32, with_clause: &str) -> Result<Instruction, String> {
        // Parse "with lib.ballcreator and lib.kick4.wav"
        let with_content = &with_clause[4..].trim(); // Remove "with "
        
        let mut library_function = None;
        let mut audio_file = None;
        
        // Split by "and" to get function and audio references
        let parts: Vec<&str> = with_content.split(" and ").collect();
        
        for part in parts {
            let part = part.trim();
            if part.starts_with("lib.") {
                let reference = &part[4..]; // Remove "lib."
                if reference.ends_with(".wav") || reference.ends_with(".mp3") || reference.ends_with(".ogg") {
                    audio_file = Some(reference.to_string());
                } else {
                    library_function = Some(reference.to_string());
                }
            }
        }
        
        let lib_func = library_function.unwrap_or_else(|| "default".to_string());
        
        match object_type {
            "ball" => {
                Ok(Instruction::CreateBallWithLibrary {
                    x: Expression::Literal(Value::Number(x)),
                    y: Expression::Literal(Value::Number(y)),
                    library_function: lib_func,
                    audio_file,
                })
            }
            "square" => {
                Ok(Instruction::CreateSquareWithLibrary {
                    x: Expression::Literal(Value::Number(x)),
                    y: Expression::Literal(Value::Number(y)),
                    library_function: lib_func,
                    audio_file,
                })
            }
            _ => Err(format!("Unknown object type: {}", object_type))
        }
    }
    
    fn parse_destroy_statement(&self, line: &str) -> Result<Instruction, String> {
        // Parse "destroy ball(3,14)", "destroy ball(self)", or "destroy square(3, 17)"
        let content = &line[8..].trim(); // Remove "destroy "
        
        if let Some(paren_pos) = content.find('(') {
            if let Some(close_paren) = content.find(')') {
                let object_type = content[..paren_pos].trim();
                let target_str = &content[paren_pos + 1..close_paren].trim();
                
                // Check if it's a ball reference (contains no comma or is "self")
                if *target_str == "self" || (target_str.contains("last.") && !target_str.contains(",")) {
                    // Ball reference syntax
                    let target = DestroyTarget::BallReference(target_str.to_string());
                    match object_type {
                        "ball" => {
                            return Ok(Instruction::DestroyBall { target });
                        }
                        "square" => {
                            return Ok(Instruction::DestroySquare { target });
                        }
                        _ => return Err(format!("Unknown object type: {}", object_type)),
                    }
                } else {
                    // Coordinate syntax
                    let coords: Vec<&str> = target_str.split(',').map(|s| s.trim()).collect();
                    if coords.len() == 2 {
                        if let (Ok(x), Ok(y)) = (coords[0].parse::<f32>(), coords[1].parse::<f32>()) {
                            let target = DestroyTarget::Coordinates {
                                x: Expression::Literal(Value::Number(x)),
                                y: Expression::Literal(Value::Number(y)),
                            };
                            match object_type {
                                "ball" => {
                                    return Ok(Instruction::DestroyBall { target });
                                }
                                "square" => {
                                    return Ok(Instruction::DestroySquare { target });
                                }
                                _ => return Err(format!("Unknown object type: {}", object_type)),
                            }
                        }
                    }
                }
            }
        }
        
        Err("Invalid destroy statement format. Expected: destroy ball(x,y), destroy ball(self), or destroy square(x,y)".to_string())
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
    
    pub fn reset_all_hit_counts(&mut self) {
        self.state.ball_hit_counts.clear();
        self.state.square_hit_counts.clear();
        self.state.ball_color_square_hits.clear();
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
        *self.state.ball_hit_counts.entry(ball_color.clone()).or_insert(0) += 1;
        *self.state.square_hit_counts.entry((square_x, square_y)).or_insert(0) += 1;
        
        // Update ball color per square hit counts
        let ball_color_square_key = (ball_color.clone(), square_x, square_y);
        *self.state.ball_color_square_hits.entry(ball_color_square_key.clone()).or_insert(0) += 1;
        
        // Create execution context
        let mut context = ExecutionContext {
            variables: self.state.variables.clone(),
            ball_hit_count: *self.state.ball_color_square_hits.get(&ball_color_square_key).unwrap_or(&0),
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
    
    fn get_ball_color(&self, ball: &Ball) -> String {
        // Convert ball color to c_ prefix format for consistency with parser
        let color = &ball.color;
        if color.starts_with("c_") {
            color.clone()
        } else {
            format!("c_{}", color.to_lowercase())
        }
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
                Instruction::CreateBallFromSample { x, y, library_name, sample_name } => {
                    let x_val = self.evaluate_expression(x, context);
                    let y_val = self.evaluate_expression(y, context);
                    
                    if let (Value::Number(x), Value::Number(y)) = (x_val, y_val) {
                        actions.push(ProgramAction::CreateBallFromSample {
                            x: x as i32,
                            y: y as i32,
                            library_name: library_name.clone(),
                            sample_name: sample_name.clone(),
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
                            sample_name: sample_name.clone(),
                        });
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
                            audio_file: audio_file.clone(),
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
                            audio_file: audio_file.clone(),
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
                Instruction::ExecuteLibraryFunction { library_function } => {
                    actions.push(ProgramAction::ExecuteLibraryFunction {
                        library_function: library_function.clone(),
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