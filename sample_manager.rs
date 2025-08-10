use std::fs;
use std::path::{Path, PathBuf};
use std::io;

/// Manages local copying and caching of audio samples
pub struct SampleManager {
    samples_dir: PathBuf,
}

impl SampleManager {
    pub fn new() -> io::Result<Self> {
        let samples_dir = PathBuf::from("samples");
        
        // Create samples directory if it doesn't exist
        if !samples_dir.exists() {
            fs::create_dir_all(&samples_dir)?;
            println!("Created samples directory: {:?}", samples_dir);
        }
        
        Ok(Self { samples_dir })
    }
    
    /// Copy an audio file to the local samples folder and return the local path
    pub fn import_sample(&self, source_path: &str) -> io::Result<String> {
        let source = Path::new(source_path);
        
        // Get the filename from the source path
        let filename = source.file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file path"))?
            .to_string_lossy();
        
        // Create destination path in samples folder
        let dest_path = self.samples_dir.join(&*filename);
        
        // Copy the file if it doesn't already exist or if source is newer
        if !dest_path.exists() || self.should_update_file(source, &dest_path)? {
            fs::copy(source, &dest_path)?;
            println!("Copied sample {} to local samples folder", filename);
        } else {
            println!("Sample {} already exists in local samples folder", filename);
        }
        
        // Return the local path as a string
        Ok(dest_path.to_string_lossy().to_string())
    }
    
    /// Check if source file is newer than destination
    fn should_update_file(&self, source: &Path, dest: &Path) -> io::Result<bool> {
        let source_modified = source.metadata()?.modified()?;
        let dest_modified = dest.metadata()?.modified()?;
        Ok(source_modified > dest_modified)
    }
    
    /// Get the local path for a sample filename
    pub fn get_local_path(&self, filename: &str) -> String {
        self.samples_dir.join(filename).to_string_lossy().to_string()
    }
    
    /// Check if a sample exists in the local samples folder
    pub fn sample_exists(&self, filename: &str) -> bool {
        self.samples_dir.join(filename).exists()
    }
    
    /// List all samples in the local samples folder
    pub fn list_samples(&self) -> io::Result<Vec<String>> {
        let mut samples = Vec::new();
        
        if self.samples_dir.exists() {
            for entry in fs::read_dir(&self.samples_dir)? {
                let entry = entry?;
                if let Some(filename) = entry.file_name().to_str() {
                    // Only include audio files
                    if filename.ends_with(".wav") || filename.ends_with(".mp3") {
                        samples.push(filename.to_string());
                    }
                }
            }
        }
        
        samples.sort();
        Ok(samples)
    }
    
    /// Clean up unused samples (optional maintenance function)
    pub fn cleanup_unused_samples(&self, used_samples: &[String]) -> io::Result<usize> {
        let mut removed_count = 0;
        
        if self.samples_dir.exists() {
            for entry in fs::read_dir(&self.samples_dir)? {
                let entry = entry?;
                if let Some(filename) = entry.file_name().to_str() {
                    if !used_samples.contains(&filename.to_string()) {
                        fs::remove_file(entry.path())?;
                        println!("Removed unused sample: {}", filename);
                        removed_count += 1;
                    }
                }
            }
        }
        
        Ok(removed_count)
    }
}

impl Default for SampleManager {
    fn default() -> Self {
        Self::new().expect("Failed to create SampleManager")
    }
}