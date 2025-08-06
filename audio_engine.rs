use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
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

#[derive(Clone)]
pub struct AudioChannel {
    pub id: u32,
    pub name: String,
    pub volume: f32,
    pub muted: bool,
    sink: Arc<Sink>,
}

impl AudioChannel {
    pub fn new(id: u32, name: String, stream_handle: &OutputStreamHandle) -> Self {
        let sink = Arc::new(Sink::try_new(stream_handle).unwrap());
        Self {
            id,
            name,
            volume: 1.0,
            muted: false,
            sink,
        }
    }
    
    pub fn play_file(&self, file_path: &str) -> Result<()> {
        if self.muted {
            return Ok(());
        }
        
        let file = File::open(file_path)?;
        let source = Decoder::new(BufReader::new(file))?;
        let amplified_source = source.amplify(self.volume);
        
        // Stop any currently playing sound and play the new one immediately
        self.sink.stop();
        self.sink.append(amplified_source);
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
        let channels = self.channels.lock().unwrap();
        let channel = channels.get(&channel_id)
            .ok_or(AudioError::ChannelNotFound(channel_id))?;
        
        channel.play_file(file_path)?;
        log::info!("Playing {} on channel {}", file_path, channel_id);
        Ok(())
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
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop_all();
    }
}