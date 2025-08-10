use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Failed to initialize audio output: {0}")]
    OutputError(#[from] rodio::StreamError),
    #[error("Failed to play audio: {0}")]
    PlayError(#[from] rodio::PlayError),
    #[error("Failed to decode audio file: {0}")]
    DecodeError(#[from] rodio::decoder::DecoderError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Channel {0} not found")]
    ChannelNotFound(u32),
}

pub type Result<T> = std::result::Result<T, AudioError>;

// Cached audio sample data
#[derive(Clone)]
pub struct CachedSample {
    pub data: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone)]
pub struct AudioChannel {
    pub id: u32,
    pub name: String,
    pub volume: f32,
    pub muted: bool,
    sink: Arc<Sink>,
    // Pool of additional sinks for simultaneous playback
    sink_pool: Vec<Arc<Sink>>,
    pool_index: usize,
}

impl AudioChannel {
    pub fn new(id: u32, name: String, stream_handle: &OutputStreamHandle) -> Self {
        let sink = Arc::new(Sink::try_new(stream_handle).unwrap());
        
        // Create a pool of sinks for simultaneous playback
        let mut sink_pool = Vec::new();
        for _ in 0..8 { // Pool of 8 sinks per channel
            if let Ok(pool_sink) = Sink::try_new(stream_handle) {
                sink_pool.push(Arc::new(pool_sink));
            }
        }
        
        Self {
            id,
            name,
            volume: 1.0,
            muted: false,
            sink,
            sink_pool,
            pool_index: 0,
        }
    }
    
    pub fn play_file(&self, file_path: &str) -> Result<()> {
        if self.muted {
            return Ok(());
        }
        
        // Try to resolve the file path - check if it's absolute or relative
        let resolved_path = if std::path::Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            // For relative paths, try multiple locations
            let current_dir = std::env::current_dir().unwrap_or_default();
            let libraries_path = current_dir.join("libraries").join(file_path);
            let direct_path = current_dir.join(file_path);
            
            if libraries_path.exists() {
                libraries_path.to_string_lossy().to_string()
            } else if direct_path.exists() {
                direct_path.to_string_lossy().to_string()
            } else {
                // If file doesn't exist in expected locations, try the original path anyway
                file_path.to_string()
            }
        };
        
        log::info!("Attempting to play audio file: {}", resolved_path);
        
        let file = File::open(&resolved_path).map_err(|e| {
            log::error!("Failed to open audio file '{}': {}", resolved_path, e);
            e
        })?;
        let source = Decoder::new(BufReader::new(file))?;
        let amplified_source = source.amplify(self.volume);
        
        // Try to find an available sink in the pool for simultaneous playback
        for sink in &self.sink_pool {
            if sink.empty() {
                sink.append(amplified_source);
                log::info!("Audio playing on pool sink: {}", resolved_path);
                return Ok(());
            }
        }
        
        // If no sink is available, use the main sink (this will interrupt current playback)
        self.sink.stop();
        self.sink.append(amplified_source);
        log::info!("Audio playing on main sink: {}", resolved_path);
        Ok(())
    }
    
    pub fn play_file_with_pitch(&self, file_path: &str, pitch: f32) -> Result<()> {
        if self.muted {
            return Ok(());
        }
        
        // Try to resolve the file path - check if it's absolute or relative
        let resolved_path = if std::path::Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            // For relative paths, try multiple locations
            let current_dir = std::env::current_dir().unwrap_or_default();
            let libraries_path = current_dir.join("libraries").join(file_path);
            let direct_path = current_dir.join(file_path);
            
            if libraries_path.exists() {
                libraries_path.to_string_lossy().to_string()
            } else if direct_path.exists() {
                direct_path.to_string_lossy().to_string()
            } else {
                // If file doesn't exist in expected locations, try the original path anyway
                file_path.to_string()
            }
        };
        
        // Validate and clamp pitch to safe range to prevent audio engine failures
        let safe_pitch = pitch.clamp(0.1, 10.0); // Clamp between 0.1x and 10x speed
        if safe_pitch != pitch {
            log::warn!("Pitch {} clamped to safe range: {}", pitch, safe_pitch);
        }
        
        log::info!("Attempting to play audio file with pitch {}: {}", safe_pitch, resolved_path);
        
        let file = File::open(&resolved_path).map_err(|e| {
            log::error!("Failed to open audio file '{}': {}", resolved_path, e);
            e
        })?;
        let source = Decoder::new(BufReader::new(file))?;
        let amplified_source = source.amplify(self.volume);
        // Apply pitch adjustment using speed transformation
        let pitched_source = amplified_source.speed(safe_pitch);
        
        // Try to find an available sink in the pool for simultaneous playback
        for sink in &self.sink_pool {
            if sink.empty() {
                sink.append(pitched_source);
                log::info!("Audio playing on pool sink with pitch {}: {}", safe_pitch, resolved_path);
                return Ok(());
            }
        }
        
        // If no sink is available, use the main sink (this will interrupt current playback)
        self.sink.stop();
        self.sink.append(pitched_source);
        log::info!("Audio playing on main sink with pitch {}: {}", safe_pitch, resolved_path);
        Ok(())
    }
    
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 2.0);
        self.sink.set_volume(self.volume);
    }
    
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if muted {
            self.sink.pause();
        } else {
            self.sink.play();
        }
    }
    
    pub fn stop(&self) {
        self.sink.stop();
    }
    
    pub fn pause(&self) {
        self.sink.pause();
    }
    
    pub fn resume(&self) {
        self.sink.play();
    }
    
    pub fn is_empty(&self) -> bool {
        self.sink.empty()
    }
}

pub struct AudioEngine {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    channels: Arc<Mutex<HashMap<u32, AudioChannel>>>,
    next_channel_id: u32,
    master_volume: f32,
    // Sample cache to avoid repeated file I/O
    sample_cache: Arc<Mutex<HashMap<String, CachedSample>>>,
    // Performance monitoring
    active_samples: Arc<Mutex<u32>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        
        Ok(Self {
            _stream: stream,
            stream_handle,
            channels: Arc::new(Mutex::new(HashMap::new())),
            next_channel_id: 0,
            master_volume: 1.0,
            sample_cache: Arc::new(Mutex::new(HashMap::new())),
            active_samples: Arc::new(Mutex::new(0)),
        })
    }
    
    pub fn create_channel(&mut self, name: String) -> u32 {
        let channel_id = self.next_channel_id;
        self.next_channel_id += 1;
        
        let channel = AudioChannel::new(channel_id, name, &self.stream_handle);
        
        let mut channels = self.channels.lock().unwrap();
        channels.insert(channel_id, channel);
        
        log::info!("Created audio channel {} with ID {}", channels.get(&channel_id).unwrap().name, channel_id);
        channel_id
    }
    
    pub fn play_on_channel(&self, channel_id: u32, file_path: &str) -> Result<()> {
        self.play_on_channel_with_pitch(channel_id, file_path, 1.0)
    }
    
    pub fn play_on_channel_with_pitch(&self, channel_id: u32, file_path: &str, pitch: f32) -> Result<()> {
        let channels = self.channels.lock().unwrap();
        let channel = channels.get(&channel_id)
            .ok_or(AudioError::ChannelNotFound(channel_id))?;
        
        channel.play_file_with_pitch(file_path, pitch)?;
        log::info!("Playing {} on channel {} with pitch {} (active samples: {})", 
                  file_path, channel_id, pitch, self.get_active_sample_count());
        Ok(())
    }
    
    pub fn play_reverse_on_channel(&self, channel_id: u32, file_path: &str, speed: f32) -> Result<()> {
        // For now, this is a placeholder that plays the sample normally
        // TODO: Implement actual reverse playback with speed control
        let channels = self.channels.lock().unwrap();
        if let Some(channel) = channels.get(&channel_id) {
            // Log the reverse playback request for debugging
            println!("Reverse sample playback requested: {} at speed {}", file_path, speed);
            channel.play_file(file_path)
        } else {
            Err(AudioError::ChannelNotFound(channel_id))
        }
    }
    
    pub fn set_channel_volume(&self, channel_id: u32, volume: f32) -> Result<()> {
        let mut channels = self.channels.lock().unwrap();
        let channel = channels.get_mut(&channel_id)
            .ok_or(AudioError::ChannelNotFound(channel_id))?;
        
        channel.set_volume(volume * self.master_volume);
        Ok(())
    }
    
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 2.0);
        
        // Update all channel volumes
        let mut channels = self.channels.lock().unwrap();
        for channel in channels.values_mut() {
            channel.set_volume(channel.volume * self.master_volume);
        }
    }
    
    pub fn mute_channel(&self, channel_id: u32, muted: bool) -> Result<()> {
        let mut channels = self.channels.lock().unwrap();
        let channel = channels.get_mut(&channel_id)
            .ok_or(AudioError::ChannelNotFound(channel_id))?;
        
        channel.set_muted(muted);
        Ok(())
    }
    
    pub fn stop_channel(&self, channel_id: u32) -> Result<()> {
        let channels = self.channels.lock().unwrap();
        let channel = channels.get(&channel_id)
            .ok_or(AudioError::ChannelNotFound(channel_id))?;
        
        channel.stop();
        Ok(())
    }
    
    pub fn pause_channel(&self, channel_id: u32) -> Result<()> {
        let channels = self.channels.lock().unwrap();
        let channel = channels.get(&channel_id)
            .ok_or(AudioError::ChannelNotFound(channel_id))?;
        
        channel.pause();
        Ok(())
    }
    
    pub fn resume_channel(&self, channel_id: u32) -> Result<()> {
        let channels = self.channels.lock().unwrap();
        let channel = channels.get(&channel_id)
            .ok_or(AudioError::ChannelNotFound(channel_id))?;
        
        channel.resume();
        Ok(())
    }
    
    pub fn stop_all(&self) {
        let channels = self.channels.lock().unwrap();
        for channel in channels.values() {
            channel.stop();
        }
    }
    
    pub fn get_channel_count(&self) -> usize {
        let channels = self.channels.lock().unwrap();
        channels.len()
    }
    
    pub fn list_channels(&self) -> Vec<(u32, String, bool)> {
        let channels = self.channels.lock().unwrap();
        channels.values()
            .map(|ch| (ch.id, ch.name.clone(), ch.is_empty()))
            .collect()
    }
    
    // Performance monitoring methods
    pub fn get_active_sample_count(&self) -> u32 {
        let active = self.active_samples.lock().unwrap();
        *active
    }
    
    pub fn cleanup_finished_samples(&self) {
        // Reset the active sample counter by counting only currently playing samples
        let channels = self.channels.lock().unwrap();
        let mut currently_playing = 0;
        
        for channel in channels.values() {
            // Count non-empty sinks in the pool
            for sink in &channel.sink_pool {
                if !sink.empty() {
                    currently_playing += 1;
                }
            }
            if !channel.sink.empty() {
                currently_playing += 1;
            }
        }
        
        // Update active sample counter to reflect actual playing samples
        let mut active = self.active_samples.lock().unwrap();
        *active = currently_playing;
    }
    
    // Cache management for better performance
    pub fn preload_sample(&self, file_path: &str) -> Result<()> {
        // Use the same path resolution logic as play_file
        let resolved_path = if std::path::Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            // For relative paths, try multiple locations
            let current_dir = std::env::current_dir().unwrap_or_default();
            let libraries_path = current_dir.join("libraries").join(file_path);
            let direct_path = current_dir.join(file_path);
            
            if libraries_path.exists() {
                libraries_path.to_string_lossy().to_string()
            } else if direct_path.exists() {
                direct_path.to_string_lossy().to_string()
            } else {
                // If file doesn't exist in expected locations, try the original path anyway
                file_path.to_string()
            }
        };
        
        let path_key = resolved_path.clone();
        
        // Check if already cached
        {
            let cache = self.sample_cache.lock().unwrap();
            if cache.contains_key(&path_key) {
                log::info!("Sample already cached: {}", resolved_path);
                return Ok(());
            }
        }
        
        log::info!("Attempting to preload sample: {}", resolved_path);
        
        // Load and cache the sample
        let file = File::open(&resolved_path).map_err(|e| {
            log::error!("Failed to preload sample '{}': {}", resolved_path, e);
            e
        })?;
        let mut reader = BufReader::new(file);
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        
        let cached_sample = CachedSample {
            data,
            sample_rate: 44100, // Default, could be extracted from file
            channels: 2,        // Default stereo
        };
        
        let mut cache = self.sample_cache.lock().unwrap();
        cache.insert(path_key, cached_sample);
        
        log::info!("Successfully preloaded sample: {}", resolved_path);
        Ok(())
    }
    
    pub fn clear_sample_cache(&self) {
        let mut cache = self.sample_cache.lock().unwrap();
        cache.clear();
        log::info!("Sample cache cleared");
    }
    
    pub fn get_cache_size(&self) -> usize {
        let cache = self.sample_cache.lock().unwrap();
        cache.len()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop_all();
    }
}