// ============================================================================
// TRANSCRIPTION MODULE
// Speaker diarization and transcription using Python bridge
// ============================================================================

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command as AsyncCommand;
extern crate dirs;

/// Transcript segment with speaker information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub speaker_id: String,
    pub speaker_label: String,
    pub start_time: f32,
    pub end_time: f32,
    pub text: String,
}

/// Speaker information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerInfo {
    pub id: String,
    pub label: String,
    pub segments_count: usize,
    pub total_duration: f32,
}

/// Complete transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub audio_file: String,
    pub duration_seconds: f32,
    pub created_at: f64,
    pub speakers: Vec<SpeakerInfo>,
    pub segments: Vec<TranscriptSegment>,
}

/// Transcription error
#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("Python not found")]
    PythonNotFound,

    #[error("Script not found: {0}")]
    ScriptNotFound(String),

    #[error("Audio file not found: {0}")]
    AudioFileNotFound(String),

    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),

    #[error("Invalid output: {0}")]
    InvalidOutput(String),
}

/// Result type for transcription operations
pub type Result<T> = std::result::Result<T, TranscriptionError>;

/// Transcription service
pub struct TranscriptionService {
    python_path: PathBuf,
    utils_dir: PathBuf,
}

impl TranscriptionService {
    /// Create new transcription service using shared venv at ~/.config/zeroagent/venv
    pub fn new() -> Result<Self> {
        use crate::settings::AppDirs;

        // Get AppDirs for utils_dir (still vault-specific for scripts)
        let app_dirs = AppDirs::get()
            .map_err(|_e| TranscriptionError::PythonNotFound)?;

        // Get Python from shared venv at ~/.config/zeroagent/venv
        let config_dir = dirs::config_dir()
            .ok_or_else(|| TranscriptionError::PythonNotFound)?
            .join("zeroagent");
        let venv_path = config_dir.join("venv");

        #[cfg(target_os = "windows")]
        let python_path = venv_path.join("Scripts").join("python.exe");

        #[cfg(not(target_os = "windows"))]
        let python_path = venv_path.join("bin").join("python");

        if !python_path.exists() {
            return Err(TranscriptionError::PythonNotFound);
        }

        // Script is stored in utils directory (user-accessible)
        let utils_dir = app_dirs.utils_dir;

        Ok(Self {
            python_path,
            utils_dir,
        })
    }

    /// Get the transcribe.py script path
    pub fn script_path(&self) -> PathBuf {
        self.utils_dir.join("transcribe.py")
    }

    /// Check if transcription dependencies are installed
    pub fn check_dependencies(&self) -> Result<bool> {
        use std::process::Command;

        let output = Command::new(&self.python_path)
            .args(["-m", "pip", "list", "--format=json"])
            .output()
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check for required packages (Hugging Face)
        let has_pyannote = stdout.contains("pyannote");
        let has_transformers = stdout.contains("transformers");
        let has_torch = stdout.contains("torch");

        Ok(has_pyannote && has_transformers && has_torch)
    }

    /// Transcribe audio file
    pub async fn transcribe(
        &self,
        audio_path: &Path,
        output_dir: &Path,
        num_speakers: Option<u32>,
    ) -> Result<Transcript> {
        if !audio_path.exists() {
            return Err(TranscriptionError::AudioFileNotFound(audio_path.to_string_lossy().to_string()));
        }

        let script = self.script_path();
        if !script.exists() {
            return Err(TranscriptionError::ScriptNotFound(script.to_string_lossy().to_string()));
        }

        // Build command using venv Python
        let mut cmd = AsyncCommand::new(&self.python_path);
        cmd.arg(&script)
            .arg(audio_path)
            .arg(output_dir);

        if let Some(n) = num_speakers {
            cmd.arg(n.to_string());
        }

        // Run and capture output
        let output = cmd.output().await
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TranscriptionError::TranscriptionFailed(stderr.to_string()));
        }

        // Parse JSON output
        let json_str = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&json_str)
            .map_err(|e| TranscriptionError::InvalidOutput(e.to_string()))
    }

    /// Get transcript for existing recording
    pub fn get_transcript(&self, transcript_path: &Path) -> Result<Transcript> {
        if !transcript_path.exists() {
            return Err(TranscriptionError::AudioFileNotFound(transcript_path.to_string_lossy().to_string()));
        }

        let json_str = std::fs::read_to_string(transcript_path)
            .map_err(|e| TranscriptionError::InvalidOutput(e.to_string()))?;

        serde_json::from_str(&json_str)
            .map_err(|e| TranscriptionError::InvalidOutput(e.to_string()))
    }
}

impl Default for TranscriptionService {
    fn default() -> Self {
        Self::new().expect("Failed to create transcription service")
    }
}
