use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;
use crate::audio_engine::{AudioEngine, DecodedSample};
use crate::font;
use std::time::{Duration, Instant};
use std::path::Path;
use std::fs;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum AudioPlayerState {
    Hidden,
    Visible {
        sample_path: String,
        sample_name: String,
        waveform_data: Vec<f32>,
        playback_position: f32, // 0.0 to 1.0 - actual audio playback position
        cursor_position: f32, // 0.0 to 1.0 - user cursor position for navigation
        is_playing: bool,
        sample_rate: u32,
        duration_ms: u32,
        markers: Vec<AudioMarker>,
        zoom_level: f32, // 1.0 = full view, higher = zoomed in
        scroll_offset: f32, // 0.0 to 1.0
        selection_start: Option<f32>,
        selection_end: Option<f32>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioMarker {
    pub position: f32, // 0.0 to 1.0
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum AudioPlayerAction {
    Close,
    SaveSlice { start: f32, end: f32, name: String },
    ExportMarkers,
}

const PLAYER_WIDTH: usize = 800;
const PLAYER_HEIGHT: usize = 400;
const WAVEFORM_HEIGHT: usize = 200;
const CONTROLS_HEIGHT: usize = 60;
const MARKERS_HEIGHT: usize = 40;
const WAVEFORM_Y_OFFSET: usize = 80;

pub struct AudioPlayer {
    pub state: AudioPlayerState,
    last_update: Instant,
    playback_start_time: Option<Instant>,
    audio_channel_id: Option<u32>,
    // Arrow key acceleration
    left_arrow_held_time: f32,
    right_arrow_held_time: f32,
    // Segment playback tracking
    current_segment_end: Option<f32>,
    // Persistent marker storage by sample path
    saved_markers: HashMap<String, Vec<AudioMarker>>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            state: AudioPlayerState::Hidden,
            last_update: Instant::now(),
            playback_start_time: None,
            audio_channel_id: None,
            left_arrow_held_time: 0.0,
            right_arrow_held_time: 0.0,
            current_segment_end: None,
            saved_markers: HashMap::new(),
        }
    }

    pub fn open_sample(&mut self, sample_path: String, audio_engine: &mut AudioEngine) -> Result<(), Box<dyn std::error::Error>> {
        // Load and decode the audio file
        let decoded_sample = audio_engine.load_sample(&sample_path)?;
        
        // Generate waveform data (downsample for visualization)
        let waveform_data = self.generate_waveform_data(&decoded_sample);
        
        // Extract filename for display
        let sample_name = Path::new(&sample_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Create audio channel for playback
        let channel_id = audio_engine.create_channel(format!("AudioPlayer_{}", sample_name));
        self.audio_channel_id = Some(channel_id);

        // Load previously saved markers for this sample path
        let saved_markers = self.saved_markers.get(&sample_path).cloned().unwrap_or_default();

        self.state = AudioPlayerState::Visible {
            sample_path,
            sample_name,
            waveform_data,
            playback_position: 0.0,
            cursor_position: 0.0,
            is_playing: false,
            sample_rate: decoded_sample.sample_rate,
            duration_ms: decoded_sample.duration_ms,
            markers: saved_markers,
            zoom_level: 1.0,
            scroll_offset: 0.0,
            selection_start: None,
            selection_end: None,
        };

        Ok(())
    }

    pub fn close(&mut self) {
        self.state = AudioPlayerState::Hidden;
        self.playback_start_time = None;
        self.audio_channel_id = None;
    }

    pub fn is_visible(&self) -> bool {
        matches!(self.state, AudioPlayerState::Visible { .. })
    }
    
    pub fn update(&mut self, delta_time: f32, audio_engine: &AudioEngine) {
        let should_update = if let AudioPlayerState::Visible { 
            duration_ms, 
            is_playing, 
            .. 
        } = &self.state {
            *is_playing && duration_ms > &0
        } else {
            false
        };
        
        if should_update {
            if let AudioPlayerState::Visible { duration_ms, .. } = &self.state {
                let duration = *duration_ms;
                self.update_playback_position(duration, audio_engine);
            }
        }
        
        self.last_update = Instant::now();
    }

    pub fn handle_input(&mut self, input: &WinitInputHelper, audio_engine: &mut AudioEngine) -> Option<AudioPlayerAction> {
        // Extract values first to avoid borrowing conflicts
        let (sample_path, current_cursor_pos, current_playing, duration_value) = if let AudioPlayerState::Visible { 
            ref sample_path,
            cursor_position,
            is_playing,
            duration_ms,
            ..
        } = &self.state {
            (sample_path.clone(), *cursor_position, *is_playing, *duration_ms)
        } else {
            return None;
        };
        
        // ESC to close - save markers and preserve them in memory for other modules to access
        if input.key_pressed(VirtualKeyCode::Escape) {
            // Save current markers to persistent storage
            if let AudioPlayerState::Visible { ref sample_path, ref markers, .. } = &self.state {
                self.saved_markers.insert(sample_path.clone(), markers.clone());
            }
            self.stop_playback(audio_engine);
            return Some(AudioPlayerAction::Close);
        }

        // Space to play/pause - play from current cursor position
        if input.key_pressed(VirtualKeyCode::Space) && !input.held_shift() {
            if current_playing {
                self.stop_playback(audio_engine);
            } else {
                // Check if cursor is at or near a marker for segment playback
                let mut segment_end = None;
                if let AudioPlayerState::Visible { ref markers, .. } = &self.state {
                    if !markers.is_empty() {
                        let mut sorted_markers: Vec<_> = markers.iter().collect();
                        sorted_markers.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                        
                        // Check if cursor is exactly at a marker or very close (for snapped positions)
                        const MARKER_TOLERANCE: f32 = 0.001; // Reduced tolerance for more precise snapping
                        let current_marker = sorted_markers.iter()
                            .find(|m| (m.position - current_cursor_pos).abs() < MARKER_TOLERANCE);
                        
                        if let Some(marker) = current_marker {
                            // Find the next marker for segment end
                            let marker_index = sorted_markers.iter()
                                .position(|m| m.position == marker.position)
                                .unwrap();
                            
                            if marker_index + 1 < sorted_markers.len() {
                                segment_end = Some(sorted_markers[marker_index + 1].position);
                            } else {
                                segment_end = Some(1.0); // Play to end if no next marker
                            }
                            
                            // Start playback from the exact marker position
                            self.current_segment_end = segment_end;
                            self.start_playback(audio_engine, &sample_path, marker.position);
                            return None;
                        }
                    }
                }
                
                // If not at a marker, play normally from cursor position
                self.current_segment_end = None;
                self.start_playback(audio_engine, &sample_path, current_cursor_pos);
            }
        }
        
        // Handle navigation and other state changes
        let mut need_restart = false;
        let mut new_cursor_pos = current_cursor_pos;
        
        if let AudioPlayerState::Visible { 
            ref mut playback_position,
            ref mut cursor_position,
            ref mut is_playing,
            ref mut markers,
            ref mut zoom_level,
            ref mut scroll_offset,
            ref mut selection_start,
            ref mut selection_end,
            duration_ms,
            ..
        } = &mut self.state {

            // Shift+Space to add marker
            if input.key_pressed(VirtualKeyCode::Space) && input.held_shift() {
                // Add new marker at cursor position
                markers.push(AudioMarker {
                    position: *cursor_position,
                    name: String::new(), // Temporary name, will be set below
                });
                
                // Sort all markers by position
                markers.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                
                // Renumber all markers sequentially
                for (index, marker) in markers.iter_mut().enumerate() {
                    marker.name = format!("{}", index + 1);
                }
            }

            // Arrow keys for navigation with smooth acceleration
            let delta_time = self.last_update.elapsed().as_secs_f32();
            
            // Handle Shift+Left for marker snapping (key_pressed for single action)
            if input.key_pressed(VirtualKeyCode::Left) && input.held_shift() {
                if !markers.is_empty() {
                    let mut sorted_markers: Vec<_> = markers.iter().collect();
                    sorted_markers.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                    
                    // Find the closest marker to the left of current cursor position
                    let target_marker = sorted_markers.iter()
                        .rposition(|m| m.position < *cursor_position)
                        .unwrap_or(sorted_markers.len() - 1); // Wrap to last marker if none found
                    
                    let new_pos = sorted_markers[target_marker].position;
                    *cursor_position = new_pos;
                    *playback_position = new_pos; // Snap playback cursor to marker too
                    
                    // Reset acceleration timer to prevent continued movement
                    self.left_arrow_held_time = 0.0;
                }
            }
            // Handle regular left arrow with smooth acceleration (key_held for continuous action)
            else if input.key_held(VirtualKeyCode::Left) && !input.held_shift() {
                // Smooth acceleration for left arrow
                self.left_arrow_held_time += delta_time;
                let acceleration = (self.left_arrow_held_time * 2.0).min(5.0); // Max 5x speed
                let move_amount = 0.005 * acceleration; // Base speed 0.005, accelerates up to 0.025
                new_cursor_pos = (*cursor_position - move_amount).max(0.0);
                *cursor_position = new_cursor_pos;
                if *is_playing {
                    *playback_position = new_cursor_pos;
                    need_restart = true;
                }
            } else {
                self.left_arrow_held_time = 0.0;
            }

            // Handle Shift+Right for marker snapping (key_pressed for single action)
            if input.key_pressed(VirtualKeyCode::Right) && input.held_shift() {
                if !markers.is_empty() {
                    let mut sorted_markers: Vec<_> = markers.iter().collect();
                    sorted_markers.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                    
                    // Find the closest marker to the right of current cursor position
                    let target_marker = sorted_markers.iter()
                        .position(|m| m.position > *cursor_position)
                        .unwrap_or(0); // Wrap to first marker if none found
                    
                    let new_pos = sorted_markers[target_marker].position;
                    *cursor_position = new_pos;
                    *playback_position = new_pos; // Snap playback cursor to marker too
                    
                    // Reset acceleration timer to prevent continued movement
                    self.right_arrow_held_time = 0.0;
                }
            }
            // Handle regular right arrow with smooth acceleration (key_held for continuous action)
            else if input.key_held(VirtualKeyCode::Right) && !input.held_shift() {
                // Smooth acceleration for right arrow
                self.right_arrow_held_time += delta_time;
                let acceleration = (self.right_arrow_held_time * 2.0).min(5.0); // Max 5x speed
                let move_amount = 0.005 * acceleration; // Base speed 0.005, accelerates up to 0.025
                new_cursor_pos = (*cursor_position + move_amount).min(1.0);
                *cursor_position = new_cursor_pos;
                if *is_playing {
                    *playback_position = new_cursor_pos;
                    need_restart = true;
                }
            } else {
                self.right_arrow_held_time = 0.0;
            }

            // Zoom controls
            if input.key_pressed(VirtualKeyCode::Plus) || input.key_pressed(VirtualKeyCode::Equals) {
                *zoom_level = (*zoom_level * 1.5).min(10.0);
            }

            if input.key_pressed(VirtualKeyCode::Minus) {
                *zoom_level = (*zoom_level / 1.5).max(1.0);
            }

            // Scroll when zoomed
            if *zoom_level > 1.0 {
                if input.key_pressed(VirtualKeyCode::A) {
                    *scroll_offset = (*scroll_offset - 0.1).max(0.0);
                }
                if input.key_pressed(VirtualKeyCode::D) {
                    let max_scroll = 1.0 - (1.0 / *zoom_level);
                    *scroll_offset = (*scroll_offset + 0.1).min(max_scroll);
                }
            }

            // Selection with mouse (simplified - using keys for now)
            if input.key_pressed(VirtualKeyCode::S) && input.held_shift() {
                if selection_start.is_none() {
                    *selection_start = Some(*cursor_position);
                } else if selection_end.is_none() {
                    *selection_end = Some(*cursor_position);
                } else {
                    // Reset selection
                    *selection_start = Some(*cursor_position);
                    *selection_end = None;
                }
            }

            // Export selection as new sample
            if input.key_pressed(VirtualKeyCode::E) && selection_start.is_some() && selection_end.is_some() {
                let start = selection_start.unwrap();
                let end = selection_end.unwrap();
                let slice_name = format!("slice_{}_{}", (start * 100.0) as u32, (end * 100.0) as u32);
                return Some(AudioPlayerAction::SaveSlice { 
                    start: start.min(end), 
                    end: start.max(end), 
                    name: slice_name 
                });
            }

            // Delete markers with backspace (within range of 10 units)
            if input.key_pressed(VirtualKeyCode::Back) {
                let cursor_pos = *cursor_position;
                let duration_ms_f32 = *duration_ms as f32;
                
                // Convert 10 unit range to normalized position (assuming 1000ms = 1.0)
                let range_normalized = 10.0 / duration_ms_f32;
                
                // Find markers within range and remove them
                markers.retain(|marker| {
                    let distance = (marker.position - cursor_pos).abs();
                    distance > range_normalized
                });
            }

            // Update playback position if playing
            let should_update = *is_playing;
            
            if should_update {
                self.update_playback_position(duration_value, audio_engine);
            }
        }
        
        // Handle restart outside the borrow
        if need_restart {
            self.stop_playback(audio_engine);
            self.start_playback(audio_engine, &sample_path, new_cursor_pos);
        }

        None
    }

    fn start_playback(&mut self, audio_engine: &mut AudioEngine, sample_path: &str, start_position: f32) {
        if let Some(channel_id) = self.audio_channel_id {
            // Stop any existing playback first
            audio_engine.stop_channel(channel_id).ok();
            
            // Try to start playback and handle errors properly
            match audio_engine.play_on_channel_with_position(channel_id, sample_path, 1.0, 1.0, start_position) {
                Ok(()) => {
                    if let AudioPlayerState::Visible { is_playing, playback_position, .. } = &mut self.state {
                        *is_playing = true;
                        *playback_position = start_position;
                        println!("Started playback from position: {}", start_position);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to start playback: {}", e);
                    // Make sure we don't set is_playing to true if playback failed
                    if let AudioPlayerState::Visible { is_playing, .. } = &mut self.state {
                        *is_playing = false;
                    }
                }
            }
        } else {
            eprintln!("No audio channel available for playback");
        }
    }
    


    fn stop_playback(&mut self, audio_engine: &mut AudioEngine) {
        if let Some(channel_id) = self.audio_channel_id {
            audio_engine.stop_channel(channel_id).ok();
        }
        
        if let AudioPlayerState::Visible { is_playing, .. } = &mut self.state {
            *is_playing = false;
            self.playback_start_time = None;
            self.current_segment_end = None;
        }
    }

    fn update_playback_position(&mut self, duration_ms: u32, audio_engine: &AudioEngine) {
        if let (Some(channel_id), AudioPlayerState::Visible { playback_position, is_playing, .. }) = 
            (self.audio_channel_id, &mut self.state) {
            
            if let Some(real_position) = audio_engine.get_channel_playback_position(channel_id) {
                // Always update the playback position for visual feedback
                *playback_position = real_position;
                
                // Check if we've reached the end of a segment (next marker)
                if let Some(segment_end) = self.current_segment_end {
                    if real_position >= segment_end {
                        *playback_position = segment_end;
                        *is_playing = false;
                        self.current_segment_end = None;
                        // Stop the audio engine
                        if let Err(e) = audio_engine.stop_channel(channel_id) {
                            eprintln!("Error stopping audio at segment end: {}", e);
                        }
                        return;
                    }
                }
                
                // Check if we've reached the end of the file
                if real_position >= 1.0 {
                    *is_playing = false;
                    self.current_segment_end = None;
                }
            }
        }
    }

    fn generate_waveform_data(&self, sample: &DecodedSample) -> Vec<f32> {
        let target_samples = 1600; // Target number of waveform points
        let chunk_size = sample.data.len() / target_samples;
        
        if chunk_size == 0 {
            return sample.data.clone();
        }

        let mut waveform = Vec::with_capacity(target_samples);
        
        for chunk_start in (0..sample.data.len()).step_by(chunk_size) {
            let chunk_end = (chunk_start + chunk_size).min(sample.data.len());
            let chunk = &sample.data[chunk_start..chunk_end];
            
            // Calculate RMS for this chunk
            let rms = if chunk.is_empty() {
                0.0
            } else {
                let sum_squares: f32 = chunk.iter().map(|&x| x * x).sum();
                (sum_squares / chunk.len() as f32).sqrt()
            };
            
            waveform.push(rms);
        }
        
        waveform
    }

    pub fn render(&self, frame: &mut [u8], window_width: usize, window_height: usize) {
        if let AudioPlayerState::Visible { 
            ref sample_name,
            ref waveform_data,
            playback_position,
            cursor_position,
            is_playing,
            duration_ms,
            ref markers,
            zoom_level,
            scroll_offset,
            selection_start,
            selection_end,
            ..
        } = &self.state {
            
            // Calculate position (center of screen) and constrain to window bounds
            let actual_player_width = PLAYER_WIDTH.min(window_width);
            let actual_player_height = PLAYER_HEIGHT.min(window_height);
            
            let player_x = if window_width > actual_player_width {
                (window_width - actual_player_width) / 2
            } else {
                0
            };
            let player_y = if window_height > actual_player_height {
                (window_height - actual_player_height) / 2
            } else {
                0
            };

            // Draw background
            self.draw_background(frame, player_x, player_y, actual_player_width, actual_player_height, window_width);

            // Draw title
            let title = format!("Audio Player - {}", sample_name);
            font::draw_text(frame, &title, player_x + 10, player_y + 10, [255, 255, 255], false, window_width);

            // Draw duration info
            let duration_text = format!("Duration: {:.2}s", *duration_ms as f32 / 1000.0);
            font::draw_text(frame, &duration_text, player_x + 10, player_y + 30, [200, 200, 200], false, window_width);

            // Draw playback status
            let status_text = if *is_playing { "Playing" } else { "Stopped" };
            let status_color = if *is_playing { [0, 255, 0] } else { [255, 0, 0] };
            font::draw_text(frame, status_text, player_x + 200, player_y + 30, status_color, false, window_width);

            // Draw waveform
            self.draw_waveform(frame, player_x, player_y + WAVEFORM_Y_OFFSET, waveform_data, 
                             *playback_position, *cursor_position, *zoom_level, *scroll_offset, actual_player_width, window_width);

            // Draw selection
            if let (Some(start), Some(end)) = (*selection_start, *selection_end) {
                self.draw_selection(frame, player_x, player_y + WAVEFORM_Y_OFFSET, start, end, 
                                  *zoom_level, *scroll_offset, actual_player_width, window_width);
            }

            // Draw markers
            self.draw_markers(frame, player_x, player_y + WAVEFORM_Y_OFFSET + WAVEFORM_HEIGHT + 10, 
                            markers, *zoom_level, *scroll_offset, actual_player_width, window_width);

            // Draw controls help
            self.draw_controls_help(frame, player_x, player_y + actual_player_height - 60, window_width);
        }
    }

    fn draw_background(&self, frame: &mut [u8], x: usize, y: usize, width: usize, height: usize, window_width: usize) {
        // Draw semi-transparent background
        for dy in 0..height {
            for dx in 0..width {
                let px = x + dx;
                let py = y + dy;
                if px < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + px) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = 40;     // R
                        frame[idx + 1] = 40; // G
                        frame[idx + 2] = 40; // B
                        frame[idx + 3] = 255; // A
                    }
                }
            }
        }

        // Draw border
        for dx in 0..width {
            // Top border
            if x + dx < window_width {
                let idx_top = (y * window_width + (x + dx)) * 4;
                if idx_top + 3 < frame.len() {
                    frame[idx_top] = 100;
                    frame[idx_top + 1] = 100;
                    frame[idx_top + 2] = 100;
                }
            }
            
            // Bottom border
            if x + dx < window_width && y + height > 0 {
                let idx_bottom = ((y + height - 1) * window_width + (x + dx)) * 4;
                if idx_bottom + 3 < frame.len() {
                    frame[idx_bottom] = 100;
                    frame[idx_bottom + 1] = 100;
                    frame[idx_bottom + 2] = 100;
                }
            }
        }

        for dy in 0..height {
            // Left border
            if x < window_width {
                let idx_left = ((y + dy) * window_width + x) * 4;
                if idx_left + 3 < frame.len() {
                    frame[idx_left] = 100;
                    frame[idx_left + 1] = 100;
                    frame[idx_left + 2] = 100;
                }
            }
            
            // Right border
            if x + width > 0 && x + width - 1 < window_width {
                let idx_right = ((y + dy) * window_width + (x + width - 1)) * 4;
                if idx_right + 3 < frame.len() {
                    frame[idx_right] = 100;
                    frame[idx_right + 1] = 100;
                    frame[idx_right + 2] = 100;
                }
            }
        }
    }

    fn draw_waveform(&self, frame: &mut [u8], x: usize, y: usize, waveform_data: &[f32], 
                    playback_position: f32, cursor_position: f32, zoom_level: f32, scroll_offset: f32, player_width: usize, window_width: usize) {
        let waveform_width = player_width.saturating_sub(20);
        let waveform_height = WAVEFORM_HEIGHT;
        let center_y = y + waveform_height / 2;

        // Calculate visible range based on zoom and scroll
        let visible_start = scroll_offset;
        let visible_end = (scroll_offset + (1.0 / zoom_level)).min(1.0);
        
        let start_sample = (visible_start * waveform_data.len() as f32) as usize;
        let end_sample = (visible_end * waveform_data.len() as f32) as usize;
        let visible_samples = &waveform_data[start_sample..end_sample.min(waveform_data.len())];

        // Draw waveform
        for (i, &amplitude) in visible_samples.iter().enumerate() {
            let sample_x = x + 10 + (i * waveform_width) / visible_samples.len();
            let wave_height = (amplitude * (waveform_height as f32 / 2.0)) as usize;
            
            // Draw positive part
            for dy in 0..wave_height {
                let py = center_y - dy;
                if sample_x < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + sample_x) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = 0;     // R
                        frame[idx + 1] = 150; // G
                        frame[idx + 2] = 255; // B
                    }
                }
            }
            
            // Draw negative part
            for dy in 0..wave_height {
                let py = center_y + dy;
                if sample_x < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + sample_x) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = 0;     // R
                        frame[idx + 1] = 150; // G
                        frame[idx + 2] = 255; // B
                    }
                }
            }
        }

        // Draw center line
        for dx in 0..waveform_width {
            let px = x + 10 + dx;
            if px < window_width && center_y < frame.len() / (window_width * 4) {
                let idx = (center_y * window_width + px) * 4;
                if idx + 3 < frame.len() {
                    frame[idx] = 80;
                    frame[idx + 1] = 80;
                    frame[idx + 2] = 80;
                }
            }
        }

        // Draw navigation cursor (cursor_position) - white/light gray
        let nav_cursor_x = x + 10 + ((cursor_position - visible_start) / (visible_end - visible_start) * waveform_width as f32) as usize;
        
        let clamped_nav_cursor_x = if cursor_position < visible_start {
            x + 10 // Show at left edge
        } else if cursor_position > visible_end {
            x + 10 + waveform_width - 1 // Show at right edge
        } else {
            nav_cursor_x // Show at actual position
        };
        
        // Draw the navigation cursor line (white)
        if clamped_nav_cursor_x >= x + 10 && clamped_nav_cursor_x < x + 10 + waveform_width {
            for dy in 0..waveform_height {
                let py = y + dy;
                if clamped_nav_cursor_x < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + clamped_nav_cursor_x) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = 255;   // R
                        frame[idx + 1] = 255; // G
                        frame[idx + 2] = 255; // B (white)
                    }
                }
            }
        }

        // Draw playback cursor (playback_position) - yellow/orange
        let playback_cursor_x = x + 10 + ((playback_position - visible_start) / (visible_end - visible_start) * waveform_width as f32) as usize;
        
        let clamped_playback_cursor_x = if playback_position < visible_start {
            x + 10 // Show at left edge
        } else if playback_position > visible_end {
            x + 10 + waveform_width - 1 // Show at right edge
        } else {
            playback_cursor_x // Show at actual position
        };
        
        // Draw the playback cursor line (yellow/orange)
        if clamped_playback_cursor_x >= x + 10 && clamped_playback_cursor_x < x + 10 + waveform_width {
            for dy in 0..waveform_height {
                let py = y + dy;
                if clamped_playback_cursor_x < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + clamped_playback_cursor_x) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = 255;   // R
                        frame[idx + 1] = 165; // G (orange)
                        frame[idx + 2] = 0;   // B
                    }
                }
            }
        }
    }

    fn draw_selection(&self, frame: &mut [u8], x: usize, y: usize, start: f32, end: f32, 
                     zoom_level: f32, scroll_offset: f32, player_width: usize, window_width: usize) {
        let waveform_width = player_width.saturating_sub(20);
        let visible_start = scroll_offset;
        let visible_end = (scroll_offset + (1.0 / zoom_level)).min(1.0);
        
        let selection_start_x = x + 10 + ((start - visible_start) / (visible_end - visible_start) * waveform_width as f32) as usize;
        let selection_end_x = x + 10 + ((end - visible_start) / (visible_end - visible_start) * waveform_width as f32) as usize;
        
        // Draw selection overlay
        for dx in selection_start_x..selection_end_x {
            for dy in 0..WAVEFORM_HEIGHT {
                let px = dx;
                let py = y + dy;
                if px < window_width && py < frame.len() / (window_width * 4) {
                    let idx = (py * window_width + px) * 4;
                    if idx + 3 < frame.len() {
                        frame[idx] = frame[idx].saturating_add(30);
                        frame[idx + 1] = frame[idx + 1].saturating_add(30);
                        frame[idx + 2] = frame[idx + 2].saturating_add(0);
                    }
                }
            }
        }
    }

    fn draw_markers(&self, frame: &mut [u8], x: usize, y: usize, markers: &[AudioMarker], 
                   zoom_level: f32, scroll_offset: f32, player_width: usize, window_width: usize) {
        let waveform_width = player_width.saturating_sub(20);
        let visible_start = scroll_offset;
        let visible_end = (scroll_offset + (1.0 / zoom_level)).min(1.0);
        
        for marker in markers {
            if marker.position >= visible_start && marker.position <= visible_end {
                let marker_x = x + 10 + ((marker.position - visible_start) / (visible_end - visible_start) * waveform_width as f32) as usize;
                
                // Draw marker line
                for dy in 0..MARKERS_HEIGHT {
                    let py = y + dy;
                    if marker_x < window_width && py < frame.len() / (window_width * 4) {
                        let idx = (py * window_width + marker_x) * 4;
                        if idx + 3 < frame.len() {
                            frame[idx] = 255;   // R
                            frame[idx + 1] = 0;   // G
                            frame[idx + 2] = 255; // B
                        }
                    }
                }
                
                // Draw marker name
                font::draw_text(frame, &marker.name, marker_x + 2, y + 5, [255, 0, 255], false, window_width);
            }
        }
    }

    fn draw_controls_help(&self, frame: &mut [u8], x: usize, y: usize, window_width: usize) {
        let help_lines = [
            "Controls: Space=Play/Pause, Shift+Space=Add Marker, Left/Right=Seek",
            "Zoom: +/- keys, Scroll: A/D keys, Selection: Shift+S, Export: E",
            "ESC=Close",
        ];
        
        for (i, line) in help_lines.iter().enumerate() {
            font::draw_text(frame, line, x + 10, y + i * 15, [180, 180, 180], false, window_width);
        }
    }

    pub fn save_slice(&self, start: f32, end: f32, name: &str, audio_engine: &AudioEngine) -> Result<String, Box<dyn std::error::Error>> {
        if let AudioPlayerState::Visible { ref sample_path, duration_ms, .. } = &self.state {
            // Load the original sample
            let decoded_sample = audio_engine.load_sample(sample_path)?;
            
            // Calculate sample indices
            let total_samples = decoded_sample.data.len();
            let start_sample = (start * total_samples as f32) as usize;
            let end_sample = (end * total_samples as f32) as usize;
            
            // Extract the slice
            let slice_data = &decoded_sample.data[start_sample..end_sample.min(total_samples)];
            
            // Create output filename
            let output_path = format!("samples/{}.wav", name);
            
            // Save as WAV file (simplified - would need proper WAV encoding)
            // For now, just return the path where it would be saved
            Ok(output_path)
        } else {
            Err("No sample loaded".into())
        }
    }
    
    /// Get the current markers for access by other modules like programmer.rs
    pub fn get_markers(&self) -> Option<&Vec<AudioMarker>> {
        if let AudioPlayerState::Visible { ref markers, .. } = &self.state {
            Some(markers)
        } else {
            None
        }
    }
    
    /// Get the current sample information along with markers
    pub fn get_sample_info(&self) -> Option<(&str, &str, &Vec<AudioMarker>, u32)> {
        if let AudioPlayerState::Visible { ref sample_path, ref sample_name, ref markers, duration_ms, .. } = &self.state {
            Some((sample_path, sample_name, markers, *duration_ms))
        } else {
            None
        }
    }
    
    /// Get saved markers for a specific sample path (for use by programmer.rs and other modules)
    pub fn get_saved_markers(&self, sample_path: &str) -> Option<&Vec<AudioMarker>> {
        self.saved_markers.get(sample_path)
    }
    
    /// Get all saved markers (for debugging or export purposes)
    pub fn get_all_saved_markers(&self) -> &HashMap<String, Vec<AudioMarker>> {
        &self.saved_markers
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}