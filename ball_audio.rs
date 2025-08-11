//! Centralized ball audio playback system
//! This module handles all ball audio properties and playback logic in one place,
//! making it easier to add new audio features without modifying multiple locations.

use crate::audio_engine::AudioEngine;
use crate::ball::Ball;
use std::collections::HashMap;

/// Centralized ball audio playback system
pub struct BallAudioSystem {
    /// Cache for collision-specific pitch calculations
    collision_pitch_cache: HashMap<String, f32>,
}

impl BallAudioSystem {
    pub fn new() -> Self {
        Self {
            collision_pitch_cache: HashMap::new(),
        }
    }



    /// Play ball audio for PlaySample action with specific channel
    pub fn play_sample_action(
        &self,
        audio_engine: &AudioEngine,
        ball: &Ball,
        collision_pitch: f32,
        sample_index: u32,
        log_messages: &mut Vec<String>,
    ) -> Result<(), String> {
        log_messages.push(format!(
            "  → PlaySample: {} with collision pitch {:.2} and volume {:.2}",
            sample_index, collision_pitch, ball.volume
        ));

        if let Some(sample_path) = ball.sample_path.as_ref() {
            let current_active = audio_engine.get_active_sample_count();
            if current_active < 12 { // Conservative limit
                if let Err(e) = audio_engine.play_on_channel_with_pitch_and_volume(sample_index, sample_path, collision_pitch, ball.volume) {
                    return Err(format!("Failed to play sample: {}", e));
                }
            } else {
                log_messages.push(format!("  → Skipped sample (audio load: {})", current_active));
            }
        }
        
        Ok(())
    }

    /// Play ball audio on collision (uses channel 0)
    pub fn play_collision_audio(
        &self,
        audio_engine: &AudioEngine,
        ball: &Ball,
        collision_pitch: f32,
        log_messages: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Some(ref sample_path) = ball.sample_path {
            let current_active = audio_engine.get_active_sample_count();
            if current_active < 12 { // Conservative limit
                if let Err(e) = audio_engine.play_on_channel_with_pitch_and_volume(0, sample_path, collision_pitch, ball.volume) {
                    return Err(format!("Failed to play ball audio on collision: {}", e));
                } else {
                    log_messages.push(format!(
                        "♪ Ball audio played with collision pitch {} and volume {}: {}", 
                        collision_pitch, 
                        ball.volume, 
                        sample_path.split('/').last().unwrap_or(sample_path).split('\\').last().unwrap_or(sample_path)
                    ));
                }
            } else {
                log_messages.push(format!("Ball audio skipped (audio load: {})", current_active));
            }
        }
        
        Ok(())
    }



    /// Clear any cached data (useful for performance)
    pub fn clear_cache(&mut self) {
        self.collision_pitch_cache.clear();
    }
}

impl Default for BallAudioSystem {
    fn default() -> Self {
        Self::new()
    }
}