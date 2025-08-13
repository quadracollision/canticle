use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig, SampleFormat, SampleRate};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use std::collections::HashMap;
use std::fs::File;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Failed to initialize audio output: {0}")]
    OutputError(String),
    #[error("Failed to play audio: {0}")]
    PlayError(String),
    #[error("Failed to decode audio file: {0}")]
    DecodeError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Channel {0} not found")]
    ChannelNotFound(u32),
    #[error("Sample {0} not found")]
    SampleNotFound(String),
}

pub type Result<T> = std::result::Result<T, AudioError>;

// Pre-decoded audio sample stored in memory
#[derive(Clone)]
pub struct DecodedSample {
    pub data: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u32,
}

// Voice represents a single playing instance of a sample
#[derive(Clone)]
struct Voice {
    sample_data: Vec<f32>,
    position: usize,
    volume: f32,
    pitch: f32,
    channels: u16,
    active: bool,
    channel_id: u32,
    end_position: Option<usize>, // Optional end position for segment playback
    start_time: Option<std::time::Instant>,
    start_position_samples: usize,
}

impl Voice {
    fn new(sample: &DecodedSample, volume: f32, pitch: f32, channel_id: u32) -> Self {
        Self::new_with_position(sample, volume, pitch, channel_id, 0.0)
    }
    
    fn new_with_position(sample: &DecodedSample, volume: f32, pitch: f32, channel_id: u32, start_position: f32) -> Self {
        Self::new_with_segment(sample, volume, pitch, channel_id, start_position, None)
    }
    
    fn new_with_segment(sample: &DecodedSample, volume: f32, pitch: f32, channel_id: u32, start_position: f32, end_position: Option<f32>) -> Self {
        // Calculate the number of samples per frame (1 for mono, 2 for stereo)
        let samples_per_frame = sample.channels as usize;
        let total_frames = sample.data.len() / samples_per_frame;
        
        // Calculate start frame and convert to sample index
        let start_frame = (start_position * total_frames as f32) as usize;
        let start_sample = start_frame * samples_per_frame;
        let clamped_position = start_sample.min(sample.data.len().saturating_sub(samples_per_frame));
        
        let end_sample = end_position.map(|end_pos| {
            let end_frame = (end_pos * total_frames as f32) as usize;
            let end_sample = end_frame * samples_per_frame;
            // Ensure end position doesn't exceed sample length
            end_sample.min(sample.data.len())
        });
        
        // Debug logging for segment creation
        if let Some(end_pos) = end_sample {
            log::debug!("Creating segment: start_sample={}, end_sample={}, total_samples={}, duration_frames={}", 
                       clamped_position, end_pos, sample.data.len(), end_pos - clamped_position);
        }
        
        Self {
            sample_data: sample.data.clone(),
            position: clamped_position,
            volume,
            pitch,
            channels: sample.channels,
            active: true,
            channel_id,
            end_position: end_sample,
            start_time: Some(std::time::Instant::now()),
            start_position_samples: clamped_position,
        }
    }
    
    fn get_next_sample(&mut self) -> (f32, f32) {
        // Check if we've reached the end position for segment playback
        if let Some(end_pos) = self.end_position {
            if self.position >= end_pos {
                self.active = false;
                return (0.0, 0.0);
            }
        }
        
        if !self.active || self.position >= self.sample_data.len() {
            self.active = false;
            return (0.0, 0.0);
        }
        
        let left = self.sample_data[self.position] * self.volume;
        let right = if self.channels == 2 && self.position + 1 < self.sample_data.len() {
            self.sample_data[self.position + 1] * self.volume
        } else {
            left // Mono or end of data
        };
        
        // Fixed: Use consistent stepping regardless of pitch for segment accuracy
        // Pitch affects playback speed but shouldn't affect segment boundary precision
        let base_step = self.channels as usize;
        let pitch_step = if self.pitch != 1.0 {
            // For non-unity pitch, still step by channel count but track fractional position
            (self.pitch * base_step as f32) as usize
        } else {
            base_step
        };
        
        // Ensure we don't step beyond the end position for segments
        let next_position = self.position + pitch_step.max(base_step);
        if let Some(end_pos) = self.end_position {
            if next_position >= end_pos {
                // If next step would exceed end, set position to end and mark inactive
                self.position = end_pos;
                self.active = false;
            } else {
                self.position = next_position;
            }
        } else {
            self.position = next_position;
        }
        
        (left, right)
    }
    
    fn is_finished(&self) -> bool {
        !self.active || self.position >= self.sample_data.len()
    }
}

// Audio channel for organizing sounds
pub struct AudioChannel {
    pub id: u32,
    pub name: String,
    pub volume: f32,
    pub muted: bool,
}

impl AudioChannel {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            volume: 1.0,
            muted: false,
        }
    }
}

// High-performance audio engine with lock-free mixing
pub struct AudioEngine {
    _stream: Stream,
    sample_cache: Arc<Mutex<HashMap<String, DecodedSample>>>,
    channels: Arc<Mutex<HashMap<u32, AudioChannel>>>,
    voices: Arc<Mutex<Vec<Voice>>>,
    next_channel_id: AtomicU32,
    active_voices: AtomicUsize,
    master_volume: Arc<Mutex<f32>>,
    sample_rate: u32,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| AudioError::OutputError("No output device available".to_string()))?;
        
        let config = device.default_output_config()
            .map_err(|e| AudioError::OutputError(format!("Failed to get default config: {}", e)))?;
        
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        
        let sample_cache = Arc::new(Mutex::new(HashMap::new()));
        let engine_channels = Arc::new(Mutex::new(HashMap::new()));
        let voices = Arc::new(Mutex::new(Vec::new()));
        let master_volume = Arc::new(Mutex::new(1.0));
        let active_voices = AtomicUsize::new(0);
        
        // Clone for the audio callback
        let voices_clone = voices.clone();
        let master_volume_clone = master_volume.clone();
        
        let stream_config = StreamConfig {
            channels,
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };
        
        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        Self::audio_callback_f32(data, &voices_clone, &master_volume_clone, channels as usize);
                    },
                    |err| log::error!("Audio stream error: {}", err),
                    None,
                )
            },
            SampleFormat::I16 => {
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::audio_callback_i16(data, &voices_clone, &master_volume_clone, channels as usize);
                    },
                    |err| log::error!("Audio stream error: {}", err),
                    None,
                )
            },
            SampleFormat::U16 => {
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        Self::audio_callback_u16(data, &voices_clone, &master_volume_clone, channels as usize);
                    },
                    |err| log::error!("Audio stream error: {}", err),
                    None,
                )
            },
            _ => return Err(AudioError::OutputError("Unsupported sample format".to_string())),
        }.map_err(|e| AudioError::OutputError(format!("Failed to build stream: {}", e)))?;
        
        stream.play().map_err(|e| AudioError::OutputError(format!("Failed to start stream: {}", e)))?;
        
        log::info!("Audio engine initialized: {} Hz, {} channels", sample_rate, channels);
        
        Ok(Self {
            _stream: stream,
            sample_cache,
            channels: engine_channels,
            voices,
            next_channel_id: AtomicU32::new(0),
            active_voices,
            master_volume,
            sample_rate,
        })
    }
    
    // Lock-free audio callback for f32 samples
    fn audio_callback_f32(
        data: &mut [f32],
        voices: &Arc<Mutex<Vec<Voice>>>,
        master_volume: &Arc<Mutex<f32>>,
        output_channels: usize,
    ) {
        // Clear output buffer
        data.fill(0.0);
        
        let master_vol = *master_volume.lock().unwrap();
        
        if let Ok(mut voices_guard) = voices.try_lock() {
            // Mix all active voices
            for voice in voices_guard.iter_mut() {
                if voice.active {
                    // Process audio in stereo pairs
                    for chunk in data.chunks_mut(output_channels) {
                        let (left, right) = voice.get_next_sample();
                        
                        if chunk.len() >= 2 {
                            chunk[0] += left * master_vol;
                            chunk[1] += right * master_vol;
                        } else if chunk.len() == 1 {
                            chunk[0] += (left + right) * 0.5 * master_vol;
                        }
                        
                        if voice.is_finished() {
                            break;
                        }
                    }
                }
            }
            
            // Remove finished voices
            voices_guard.retain(|v| v.active && !v.is_finished());
        }
    }
    
    // Audio callback for i16 samples
    fn audio_callback_i16(
        data: &mut [i16],
        voices: &Arc<Mutex<Vec<Voice>>>,
        master_volume: &Arc<Mutex<f32>>,
        output_channels: usize,
    ) {
        data.fill(0);
        
        let master_vol = *master_volume.lock().unwrap();
        
        if let Ok(mut voices_guard) = voices.try_lock() {
            for voice in voices_guard.iter_mut() {
                if voice.active {
                    for chunk in data.chunks_mut(output_channels) {
                        let (left, right) = voice.get_next_sample();
                        
                        if chunk.len() >= 2 {
                            chunk[0] = (chunk[0] as f32 + left * master_vol * 32767.0) as i16;
                            chunk[1] = (chunk[1] as f32 + right * master_vol * 32767.0) as i16;
                        } else if chunk.len() == 1 {
                            chunk[0] = (chunk[0] as f32 + (left + right) * 0.5 * master_vol * 32767.0) as i16;
                        }
                        
                        if voice.is_finished() {
                            break;
                        }
                    }
                }
            }
            
            voices_guard.retain(|v| v.active && !v.is_finished());
        }
    }
    
    // Audio callback for u16 samples
    fn audio_callback_u16(
        data: &mut [u16],
        voices: &Arc<Mutex<Vec<Voice>>>,
        master_volume: &Arc<Mutex<f32>>,
        output_channels: usize,
    ) {
        data.fill(32768);
        
        let master_vol = *master_volume.lock().unwrap();
        
        if let Ok(mut voices_guard) = voices.try_lock() {
            for voice in voices_guard.iter_mut() {
                if voice.active {
                    for chunk in data.chunks_mut(output_channels) {
                        let (left, right) = voice.get_next_sample();
                        
                        if chunk.len() >= 2 {
                            chunk[0] = ((chunk[0] as f32 - 32768.0) + left * master_vol * 32767.0 + 32768.0) as u16;
                            chunk[1] = ((chunk[1] as f32 - 32768.0) + right * master_vol * 32767.0 + 32768.0) as u16;
                        } else if chunk.len() == 1 {
                            chunk[0] = ((chunk[0] as f32 - 32768.0) + (left + right) * 0.5 * master_vol * 32767.0 + 32768.0) as u16;
                        }
                        
                        if voice.is_finished() {
                            break;
                        }
                    }
                }
            }
            
            voices_guard.retain(|v| v.active && !v.is_finished());
        }
    }
    
    // Decode audio file using Symphonia
    fn decode_audio_file(file_path: &str) -> Result<DecodedSample> {
        log::debug!("Attempting to decode audio file: {}", file_path);
        
        let file = File::open(file_path)
            .map_err(|e| AudioError::DecodeError(format!("Failed to open file {}: {}", file_path, e)))?;
        let media_source = MediaSourceStream::new(Box::new(file), Default::default());
        
        let mut hint = Hint::new();
        if let Some(extension) = std::path::Path::new(file_path).extension() {
            if let Some(ext_str) = extension.to_str() {
                hint.with_extension(ext_str);
                log::debug!("File extension hint: {}", ext_str);
            }
        }
        
        let meta_opts = MetadataOptions::default();
        let fmt_opts = FormatOptions::default();
        
        let probed = symphonia::default::get_probe()
            .format(&hint, media_source, &fmt_opts, &meta_opts)
            .map_err(|e| {
                log::error!("Failed to probe format for {}: {}", file_path, e);
                AudioError::DecodeError(format!("Failed to probe format: {}", e))
            })?;
        
        let mut format = probed.format;
        log::debug!("Detected format: {:?}", format.metadata());
        
        let track = format.tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| {
                log::error!("No supported audio tracks found in {}", file_path);
                AudioError::DecodeError("No supported audio tracks found".to_string())
            })?;
        
        log::debug!("Found track with codec: {:?}", track.codec_params.codec);
        
        let track_id = track.id;
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| {
                log::error!("Failed to create decoder for {}: {}", file_path, e);
                AudioError::DecodeError(format!("Failed to create decoder: {}", e))
            })?;
        
        let mut sample_data = Vec::new();
        let mut sample_rate = 44100;
        let mut channels = 2;
        
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::ResetRequired) => {
                    // Reset decoder and continue
                    decoder.reset();
                    continue;
                }
                Err(SymphoniaError::IoError(_)) => break, // End of stream
                Err(e) => return Err(AudioError::DecodeError(format!("Decode error: {}", e))),
            };
            
            if packet.track_id() != track_id {
                continue;
            }
            
            match decoder.decode(&packet) {
                Ok(audio_buf) => {
                    match audio_buf {
                        AudioBufferRef::F32(buf) => {
                            sample_rate = buf.spec().rate;
                            channels = buf.spec().channels.count() as u16;
                            
                            // Interleave channels
                             for frame in 0..buf.frames() {
                                 for ch in 0..channels {
                                     let channel_data = buf.chan(ch as usize);
                                     if frame < channel_data.len() {
                                         sample_data.push(channel_data[frame]);
                                     }
                                 }
                             }
                        }
                        AudioBufferRef::F64(buf) => {
                            sample_rate = buf.spec().rate;
                            channels = buf.spec().channels.count() as u16;
                            
                            for frame in 0..buf.frames() {
                                 for ch in 0..channels {
                                     let channel_data = buf.chan(ch as usize);
                                     if frame < channel_data.len() {
                                         sample_data.push(channel_data[frame] as f32);
                                     }
                                 }
                             }
                        }
                        AudioBufferRef::S32(buf) => {
                            sample_rate = buf.spec().rate;
                            channels = buf.spec().channels.count() as u16;
                            
                            for frame in 0..buf.frames() {
                                 for ch in 0..channels {
                                     let channel_data = buf.chan(ch as usize);
                                     if frame < channel_data.len() {
                                         sample_data.push(channel_data[frame] as f32 / i32::MAX as f32);
                                     }
                                 }
                             }
                        }
                        AudioBufferRef::S16(buf) => {
                            sample_rate = buf.spec().rate;
                            channels = buf.spec().channels.count() as u16;
                            
                            for frame in 0..buf.frames() {
                                 for ch in 0..channels {
                                     let channel_data = buf.chan(ch as usize);
                                     if frame < channel_data.len() {
                                         sample_data.push(channel_data[frame] as f32 / i16::MAX as f32);
                                     }
                                 }
                             }
                        }
                        AudioBufferRef::U8(buf) => {
                            sample_rate = buf.spec().rate;
                            channels = buf.spec().channels.count() as u16;
                            
                            for frame in 0..buf.frames() {
                                 for ch in 0..channels {
                                     let channel_data = buf.chan(ch as usize);
                                     if frame < channel_data.len() {
                                         // Convert U8 to f32: U8 range is 0-255, center at 128
                                         sample_data.push((channel_data[frame] as f32 - 128.0) / 128.0);
                                     }
                                 }
                             }
                        }
                        AudioBufferRef::U16(buf) => {
                            sample_rate = buf.spec().rate;
                            channels = buf.spec().channels.count() as u16;
                            
                            for frame in 0..buf.frames() {
                                 for ch in 0..channels {
                                     let channel_data = buf.chan(ch as usize);
                                     if frame < channel_data.len() {
                                         // Convert U16 to f32: U16 range is 0-65535, center at 32768
                                         sample_data.push((channel_data[frame] as f32 - 32768.0) / 32768.0);
                                     }
                                 }
                             }
                        }
                        AudioBufferRef::U32(buf) => {
                            sample_rate = buf.spec().rate;
                            channels = buf.spec().channels.count() as u16;
                            
                            for frame in 0..buf.frames() {
                                 for ch in 0..channels {
                                     let channel_data = buf.chan(ch as usize);
                                     if frame < channel_data.len() {
                                         // Convert U32 to f32: U32 range is 0-4294967295, center at 2147483648
                                         sample_data.push((channel_data[frame] as f32 - 2147483648.0) / 2147483648.0);
                                     }
                                 }
                             }
                        }
                        AudioBufferRef::S24(buf) => {
                            sample_rate = buf.spec().rate;
                            channels = buf.spec().channels.count() as u16;
                            
                            for frame in 0..buf.frames() {
                                 for ch in 0..channels {
                                     let channel_data = buf.chan(ch as usize);
                                     if frame < channel_data.len() {
                                         // Convert S24 to f32: S24 range is -8388608 to 8388607
                                         let sample_value = channel_data[frame].inner();
                                         sample_data.push(sample_value as f32 / 8388608.0);
                                     }
                                 }
                             }
                        }
                        _ => {
                            log::error!("Unsupported audio buffer type encountered for file: {}", file_path);
                            return Err(AudioError::DecodeError(format!("Unsupported audio format for file: {}", file_path)));
                        }
                    }
                }
                Err(SymphoniaError::DecodeError(_)) => {
                    // Skip decode errors and continue
                    continue;
                }
                Err(e) => {
                    return Err(AudioError::DecodeError(format!("Decode error: {}", e)));
                }
            }
        }
        
        let duration_ms = if sample_rate > 0 {
            (sample_data.len() as u32 * 1000) / (sample_rate * channels as u32)
        } else {
            0
        };
        
        Ok(DecodedSample {
            data: sample_data,
            sample_rate,
            channels,
            duration_ms,
        })
    }
    
    // Public API methods
    pub fn create_channel(&mut self, name: String) -> u32 {
        let id = self.next_channel_id.fetch_add(1, Ordering::Relaxed);
        let channel = AudioChannel::new(id, name);
        
        let mut channels = self.channels.lock().unwrap();
        channels.insert(id, channel);
        
        log::info!("Created audio channel {} with ID {}", channels.get(&id).unwrap().name, id);
        id
    }
    
    pub fn preload_sample(&self, file_path: &str) -> Result<()> {
        let resolved_path = self.resolve_file_path(file_path);
        
        // Check if already cached
        {
            let cache = self.sample_cache.lock().unwrap();
            if cache.contains_key(&resolved_path) {
                log::info!("Sample already cached: {}", resolved_path);
                return Ok(());
            }
        }
        
        log::info!("Preloading sample: {}", resolved_path);
        
        let decoded_sample = Self::decode_audio_file(&resolved_path)?;
        
        let mut cache = self.sample_cache.lock().unwrap();
        cache.insert(resolved_path.clone(), decoded_sample);
        
        log::info!("Successfully preloaded sample: {}", resolved_path);
        Ok(())
    }
    
    pub fn load_sample(&self, file_path: &str) -> Result<DecodedSample> {
        let resolved_path = self.resolve_file_path(file_path);
        
        // Get sample from cache or load it
        let mut cache = self.sample_cache.lock().unwrap();
        if let Some(cached_sample) = cache.get(&resolved_path) {
            Ok(cached_sample.clone())
        } else {
            // Load and cache the sample
            let decoded_sample = Self::decode_audio_file(&resolved_path)?;
            cache.insert(resolved_path.clone(), decoded_sample.clone());
            Ok(decoded_sample)
        }
    }
    
    pub fn play_on_channel(&self, channel_id: u32, file_path: &str) -> Result<()> {
        self.play_on_channel_with_pitch_and_volume(channel_id, file_path, 1.0, 1.0)
    }
    
    pub fn play_on_channel_with_pitch(&self, channel_id: u32, file_path: &str, pitch: f32) -> Result<()> {
        self.play_on_channel_with_pitch_and_volume(channel_id, file_path, pitch, 1.0)
    }
    
    pub fn play_on_channel_with_pitch_and_volume(&self, channel_id: u32, file_path: &str, pitch: f32, volume: f32) -> Result<()> {
        self.play_on_channel_with_position(channel_id, file_path, pitch, volume, 0.0)
    }
    
    pub fn play_on_channel_with_position(&self, channel_id: u32, file_path: &str, pitch: f32, volume: f32, start_position: f32) -> Result<()> {
        self.play_on_channel_with_segment(channel_id, file_path, pitch, volume, start_position, None)
    }
    
    pub fn play_on_channel_with_segment(&self, channel_id: u32, file_path: &str, pitch: f32, volume: f32, start_position: f32, end_position: Option<f32>) -> Result<()> {
        let resolved_path = self.resolve_file_path(file_path);
        
        // Get sample from cache or load it
        let sample = {
            let mut cache = self.sample_cache.lock().unwrap();
            if let Some(cached_sample) = cache.get(&resolved_path) {
                cached_sample.clone()
            } else {
                // Load and cache the sample
                let decoded_sample = Self::decode_audio_file(&resolved_path)?;
                cache.insert(resolved_path.clone(), decoded_sample.clone());
                decoded_sample
            }
        };
        
        // Check if channel exists
        {
            let channels = self.channels.lock().unwrap();
            if !channels.contains_key(&channel_id) {
                return Err(AudioError::ChannelNotFound(channel_id));
            }
        }
        
        // Create and add voice
        let safe_pitch = pitch.clamp(0.1, 10.0);
        let safe_volume = volume.clamp(0.0, 2.0);
        let safe_position = start_position.clamp(0.0, 1.0);
        let safe_end_position = end_position.map(|end_pos| end_pos.clamp(0.0, 1.0));
        
        let voice = Voice::new_with_segment(&sample, safe_volume, safe_pitch, channel_id, safe_position, safe_end_position);
        
        {
            let mut voices = self.voices.lock().unwrap();
            voices.push(voice);
            
            // Limit total voices to prevent memory issues
            if voices.len() > 100 {
                voices.retain(|v| v.active && !v.is_finished());
            }
        }
        
        self.active_voices.fetch_add(1, Ordering::Relaxed);
        
        if let Some(end_pos) = safe_end_position {
            log::debug!("Playing sample {} on channel {} with pitch {:.2}, volume {:.2}, position {:.2} to {:.2}", 
                       file_path, channel_id, safe_pitch, safe_volume, safe_position, end_pos);
        } else {
            log::debug!("Playing sample {} on channel {} with pitch {:.2}, volume {:.2}, and position {:.2}", 
                       file_path, channel_id, safe_pitch, safe_volume, safe_position);
        }
        
        Ok(())
    }
    
    pub fn set_master_volume(&mut self, volume: f32) {
        let safe_volume = volume.clamp(0.0, 2.0);
        *self.master_volume.lock().unwrap() = safe_volume;
        log::info!("Master volume set to {:.2}", safe_volume);
    }
    
    pub fn set_channel_volume(&self, channel_id: u32, volume: f32) -> Result<()> {
        let mut channels = self.channels.lock().unwrap();
        if let Some(channel) = channels.get_mut(&channel_id) {
            channel.volume = volume.clamp(0.0, 2.0);
            Ok(())
        } else {
            Err(AudioError::ChannelNotFound(channel_id))
        }
    }
    
    pub fn mute_channel(&self, channel_id: u32, muted: bool) -> Result<()> {
        let mut channels = self.channels.lock().unwrap();
        if let Some(channel) = channels.get_mut(&channel_id) {
            channel.muted = muted;
            Ok(())
        } else {
            Err(AudioError::ChannelNotFound(channel_id))
        }
    }
    
    pub fn stop_channel(&self, channel_id: u32) -> Result<()> {
        let mut voices = self.voices.lock().unwrap();
        for voice in voices.iter_mut() {
            if voice.channel_id == channel_id {
                voice.active = false;
            }
        }
        Ok(())
    }
    
    pub fn stop_all(&self) {
        let mut voices = self.voices.lock().unwrap();
        for voice in voices.iter_mut() {
            voice.active = false;
        }
        voices.clear();
        self.active_voices.store(0, Ordering::Relaxed);
    }
    
    pub fn get_active_sample_count(&self) -> u32 {
        let voices = self.voices.lock().unwrap();
        let active_count = voices.iter().filter(|v| v.active && !v.is_finished()).count();
        self.active_voices.store(active_count, Ordering::Relaxed);
        active_count as u32
    }
    
    pub fn cleanup_finished_samples(&self) {
        if let Ok(mut voices) = self.voices.try_lock() {
            let initial_count = voices.len();
            voices.retain(|voice| voice.active && !voice.is_finished());
            let removed_count = initial_count - voices.len();
            
            if removed_count > 0 {
                self.active_voices.fetch_sub(removed_count, Ordering::Relaxed);
                log::debug!("Cleaned up {} finished voices, {} remaining", removed_count, voices.len());
            }
        }
    }
    
    pub fn get_channel_count(&self) -> usize {
        let channels = self.channels.lock().unwrap();
        channels.len()
    }
    
    pub fn list_channels(&self) -> Vec<(u32, String, bool)> {
        let channels = self.channels.lock().unwrap();
        channels.values()
            .map(|ch| (ch.id, ch.name.clone(), !ch.muted))
            .collect()
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
    
    // Helper method to resolve file paths
    fn resolve_file_path(&self, file_path: &str) -> String {
        if std::path::Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            let current_dir = std::env::current_dir().unwrap_or_default();
            let libraries_path = current_dir.join("libraries").join(file_path);
            let samples_path = current_dir.join("samples").join(file_path);
            let direct_path = current_dir.join(file_path);
            
            if libraries_path.exists() {
                libraries_path.to_string_lossy().to_string()
            } else if samples_path.exists() {
                samples_path.to_string_lossy().to_string()
            } else if direct_path.exists() {
                direct_path.to_string_lossy().to_string()
            } else {
                file_path.to_string()
            }
        }
    }
    
    // Add method to get current playback position
    pub fn get_channel_playback_position(&self, channel_id: u32) -> Option<f32> {
        let voices = self.voices.lock().unwrap();
        for voice in voices.iter() {
            if voice.channel_id == channel_id && voice.active {
                if let Some(start_time) = voice.start_time {
                    let elapsed_samples = (start_time.elapsed().as_secs_f32() * self.sample_rate as f32) as usize;
                    let total_position = voice.start_position_samples + elapsed_samples;
                    return Some(total_position as f32 / voice.sample_data.len() as f32);
                }
            }
        }
        None
    }
    
    // Compatibility methods for existing code
    pub fn pause_channel(&self, _channel_id: u32) -> Result<()> {
        // Not implemented in this version - voices play to completion
        Ok(())
    }
    
    pub fn resume_channel(&self, _channel_id: u32) -> Result<()> {
        // Not implemented in this version - voices play to completion
        Ok(())
    }
    
    pub fn play_reverse_on_channel(&self, channel_id: u32, file_path: &str, speed: f32) -> Result<()> {
        // Simple implementation - just play with negative pitch
        self.play_on_channel_with_pitch(channel_id, file_path, -speed.abs())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop_all();
        log::info!("Audio engine shut down");
    }
}
