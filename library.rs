use std::collections::HashMap;
use crate::square::{Program, Instruction, Expression, Value, FunctionLibrary, SampleLibrary, SampleTemplate, LibraryManager};
use crate::ball::Direction;

/// Library builder for creating function libraries programmatically
pub struct LibraryBuilder {
    name: String,
    description: String,
    functions: HashMap<String, Program>,
}

impl LibraryBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: format!("Library: {}", name),
            functions: HashMap::new(),
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Add a function with a simple instruction sequence
    pub fn add_function(mut self, name: &str, instructions: Vec<Instruction>) -> Self {
        let program = Program {
            name: name.to_string(),
            instructions,
        };
        self.functions.insert(name.to_string(), program);
        self
    }

    /// Add a bounce function
    pub fn add_bounce_function(self, name: &str) -> Self {
        self.add_function(name, vec![Instruction::Bounce])
    }

    /// Add a speed setting function
    pub fn add_speed_function(self, name: &str, speed: f32) -> Self {
        self.add_function(name, vec![
            Instruction::SetSpeed(Expression::Literal(Value::Number(speed)))
        ])
    }

    /// Add a direction setting function
    pub fn add_direction_function(self, name: &str, direction: Direction) -> Self {
        self.add_function(name, vec![
            Instruction::SetDirection(Expression::Literal(Value::Direction(direction)))
        ])
    }

    /// Add a ball creation function
    pub fn add_ball_creator(self, name: &str, x: f32, y: f32, speed: f32, direction: Direction) -> Self {
        self.add_function(name, vec![
            Instruction::CreateBall {
                x: Expression::Literal(Value::Number(x)),
                y: Expression::Literal(Value::Number(y)),
                speed: Expression::Literal(Value::Number(speed)),
                direction: Expression::Literal(Value::Direction(direction)),
            }
        ])
    }

    /// Add a square creation function
    pub fn add_square_creator(self, name: &str, x: f32, y: f32) -> Self {
        self.add_function(name, vec![
            Instruction::CreateSquare {
                x: Expression::Literal(Value::Number(x)),
                y: Expression::Literal(Value::Number(y)),
            }
        ])
    }

    /// Build the function library
    pub fn build(self) -> FunctionLibrary {
        FunctionLibrary {
            name: self.name,
            functions: self.functions,
            description: self.description,
        }
    }
}

/// Sample library builder for creating sample templates programmatically
pub struct SampleLibraryBuilder {
    name: String,
    description: String,
    samples: HashMap<String, SampleTemplate>,
}

impl SampleLibraryBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: format!("Sample Library: {}", name),
            samples: HashMap::new(),
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Add a sample template
    pub fn add_sample(
        mut self,
        name: &str,
        default_speed: f32,
        default_direction: Direction,
        color: &str,
        behavior_program: Option<&str>,
    ) -> Self {
        let sample = SampleTemplate {
            name: name.to_string(),
            default_speed,
            default_direction,
            color: color.to_string(),
            behavior_program: behavior_program.map(|s| s.to_string()),
        };
        self.samples.insert(name.to_string(), sample);
        self
    }

    /// Add a bouncing ball sample
    pub fn add_bouncing_ball(self, name: &str, speed: f32, direction: Direction, color: &str) -> Self {
        self.add_sample(name, speed, direction, color, Some("bounce"))
    }

    /// Add a speed boost ball sample
    pub fn add_speed_ball(self, name: &str, speed: f32, direction: Direction, color: &str) -> Self {
        self.add_sample(name, speed, direction, color, Some("speed_boost"))
    }

    /// Add a direction cycling ball sample
    pub fn add_cycling_ball(self, name: &str, speed: f32, direction: Direction, color: &str) -> Self {
        self.add_sample(name, speed, direction, color, Some("direction_cycle"))
    }

    /// Build the sample library
    pub fn build(self) -> SampleLibrary {
        SampleLibrary {
            name: self.name,
            samples: self.samples,
            description: self.description,
        }
    }
}

/// Main library manager extension for programmatic library creation
pub trait LibraryManagerExt {
    fn create_default_programmatic_libraries(&mut self);
    fn add_custom_function_library(&mut self, library: FunctionLibrary);
    fn add_custom_sample_library(&mut self, library: SampleLibrary);
}

impl LibraryManagerExt for LibraryManager {
    /// Create default libraries programmatically instead of from files
    fn create_default_programmatic_libraries(&mut self) {
        // Create default function library
        let function_lib = LibraryBuilder::new("lib")
            .with_description("Default function library with common behaviors")
            .add_ball_creator("ballcreator", 5.0, 5.0, 2.0, Direction::Right)
            .add_bounce_function("bounce")
            .add_speed_function("speed_boost", 3.0)
            .add_direction_function("direction_cycle", Direction::Up)
            .add_function("multi_creator", vec![
                Instruction::CreateBall {
                    x: Expression::Literal(Value::Number(2.0)),
                    y: Expression::Literal(Value::Number(2.0)),
                    speed: Expression::Literal(Value::Number(1.5)),
                    direction: Expression::Literal(Value::Direction(Direction::Right)),
                },
                Instruction::CreateSquare {
                    x: Expression::Literal(Value::Number(4.0)),
                    y: Expression::Literal(Value::Number(4.0)),
                },
            ])
            .build();

        self.add_function_library(function_lib);

        // Create default sample library
        let sample_lib = SampleLibraryBuilder::new("default")
            .with_description("Default sample library with common ball and square types")
            .add_bouncing_ball("red_bouncer", 2.0, Direction::Right, "Red")
            .add_speed_ball("blue_speedster", 3.0, Direction::Up, "Blue")
            .add_cycling_ball("green_cycler", 1.5, Direction::Left, "Green")
            .build();

        self.add_sample_library(sample_lib);
    }

    fn add_custom_function_library(&mut self, library: FunctionLibrary) {
        self.add_function_library(library);
    }

    fn add_custom_sample_library(&mut self, library: SampleLibrary) {
        self.add_sample_library(library);
    }
}

/// Example usage and helper functions
pub mod examples {
    use super::*;

    /// Create a custom music-focused function library
    pub fn create_music_library() -> FunctionLibrary {
        LibraryBuilder::new("music")
            .with_description("Music-specific functions for rhythm and melody")
            .add_function("kick_pattern", vec![
                Instruction::CreateBall {
                    x: Expression::Literal(Value::Number(1.0)),
                    y: Expression::Literal(Value::Number(1.0)),
                    speed: Expression::Literal(Value::Number(2.0)),
                    direction: Expression::Literal(Value::Direction(Direction::Down)),
                },
                Instruction::CreateBall {
                    x: Expression::Literal(Value::Number(5.0)),
                    y: Expression::Literal(Value::Number(1.0)),
                    speed: Expression::Literal(Value::Number(2.0)),
                    direction: Expression::Literal(Value::Direction(Direction::Down)),
                },
            ])
            .add_function("snare_hit", vec![
                Instruction::SetSpeed(Expression::Literal(Value::Number(4.0))),
                Instruction::Bounce,
            ])
            .add_speed_function("tempo_120", 2.0)
            .add_speed_function("tempo_140", 2.5)
            .build()
    }

    /// Create a custom sample library for different instrument sounds
    pub fn create_drum_samples() -> SampleLibrary {
        SampleLibraryBuilder::new("drums")
            .with_description("Drum kit samples for rhythm programming")
            .add_sample("kick", 2.0, Direction::Down, "Red", Some("kick_pattern"))
            .add_sample("snare", 3.0, Direction::Up, "White", Some("snare_hit"))
            .add_sample("hihat", 1.5, Direction::Right, "Yellow", Some("bounce"))
            .add_sample("crash", 4.0, Direction::Left, "Orange", Some("speed_boost"))
            .build()
    }
}