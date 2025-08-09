mod audio_engine;
mod sequencer;
mod context_menu;
mod ball;
mod square;
mod programmer;
mod square_menu;

use audio_engine::AudioEngine;
use sequencer::run_sequencer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("Starting Canticlec Churn Music Sequencer...");
    
    // Initialize the audio engine
    let mut audio_engine = AudioEngine::new()?;
    println!("Audio engine initialized successfully!");
    
    // Create some default channels
    let _drum_channel = audio_engine.create_channel("Drums".to_string());
    let _bass_channel = audio_engine.create_channel("Bass".to_string());
    let _melody_channel = audio_engine.create_channel("Melody".to_string());
    
    println!("Created {} audio channels", audio_engine.get_channel_count());
    println!("Controls:");
    println!("  Arrow keys: Move cursor");
    println!("  S: Place/remove square");
    println!("  C: Place ball (starts inactive)");
    println!("  P: Toggle ball movement (start all balls / reset to original positions)");
    println!("  Space: Open ball context menu (when cursor is on a ball)");
    println!("  R: Open square programming menu (when cursor is on a square)");
    println!("  ESC: Close/go back in context menu");
    println!();
    println!("Ball Physics:");
    println!("  - Balls start inactive when placed");
    println!("  - Press P to start/stop all balls");
    println!("  - Default direction: Up, speed: 200ms");
    println!("  - Balls reverse direction when hitting squares");
    println!();
    println!("Ball Context Menu:");
    println!("  Up/Down: Navigate menu options");
    println!("  Space: Select option");
    println!("  ESC: Go back to previous menu");
    
    // Run the sequencer UI
    if let Err(err) = run_sequencer(audio_engine).await {
        eprintln!("Sequencer error: {}", err);
    }
    
    Ok(())
}