// ============================================================================
// MEDIA COMMANDS
// Audio recording storage and knowledge graph integration
// ============================================================================

use std::fs;
use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use chrono::Utc;
use knowledge_graph::types::{Entity, EntityType, ExtractedKnowledge};
use serde::{Deserialize, Serialize};
use serde_json::json;
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Global audio recorder instance
static AUDIO_RECORDER: StdMutex<Option<crate::audio_recorder::AudioRecorder>> = StdMutex::new(None);

/// Get or create the global audio recorder
fn get_recorder() -> Result<crate::audio_recorder::AudioRecorder, String> {
    let mut recorder_guard = AUDIO_RECORDER.lock()
        .map_err(|e| format!("Failed to lock recorder: {}", e))?;

    if recorder_guard.is_none() {
        *recorder_guard = Some(crate::audio_recorder::AudioRecorder::new());
    }

    recorder_guard.as_ref()
        .cloned()
        .ok_or_else(|| "Failed to create recorder".to_string())
}

/// Get available audio input devices
#[tauri::command]
pub async fn get_audio_input_devices() -> Result<Vec<String>, String> {
    crate::audio_recorder::AudioRecorder::get_input_devices()
}

/// Start audio recording
///
/// Returns the sample rate and channels of the recording
#[tauri::command]
pub async fn start_audio_recording() -> Result<(u32, u16), String> {
    let recorder = get_recorder()?;

    // Check if already recording
    if recorder.is_recording().await? {
        return Err("Already recording".to_string());
    }

    let config = recorder.start_recording().await?;
    tracing::info!("Started audio recording: {}Hz, {} channels", config.sample_rate.0, config.channels);

    Ok((config.sample_rate.0, config.channels))
}

/// Stop audio recording and get the WAV data
///
/// Returns the WAV file data as base64 (for easy transport) along with metadata
#[tauri::command]
pub async fn stop_audio_recording() -> Result<(String, u32, u16), String> {
    let recorder = get_recorder()?;

    // Check if recording
    if !recorder.is_recording().await? {
        return Err("Not recording".to_string());
    }

    let wav_data = recorder.stop_recording().await?;

    tracing::info!("Stopped audio recording: {} bytes", wav_data.len());

    // Encode as base64 for transport to frontend
    let base64_data = STANDARD.encode(&wav_data);

    // Get recording info
    let sample_rate = recorder.get_sample_rate().await;
    let channels = recorder.get_channels().await;

    Ok((base64_data, sample_rate, channels))
}

/// Check if currently recording
#[tauri::command]
pub async fn is_recording_audio() -> Result<bool, String> {
    let recorder = get_recorder()?;
    recorder.is_recording().await
}

/// Save an audio recording to the vault's media directory
///
/// # Arguments
/// * `agent_id` - The ID of the agent
/// * `filename` - The name of the audio file (e.g., "recording-20250123-143022.wav")
/// * `audio_base64` - The audio file data as base64 string
///
/// # Returns
/// The full file path where the recording was saved
#[tauri::command]
pub async fn save_audio_recording(
    agent_id: String,
    filename: String,
    audio_base64: String,
) -> Result<String, String> {
    // Decode base64
    let audio_data = STANDARD.decode(&audio_base64)
        .map_err(|e| format!("Failed to decode audio data: {}", e))?;

    // Get vault directories
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    // Create media directory: <vault>/agents_data/{agent-id}/media/YYYY-MM/
    let year_month = Utc::now().format("%Y-%m").to_string();
    let media_dir = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media")
        .join(&year_month);

    fs::create_dir_all(&media_dir)
        .map_err(|e| format!("Failed to create media directory: {}", e))?;

    // Save audio file
    let file_path = media_dir.join(&filename);
    fs::write(&file_path, audio_data)
        .map_err(|e| format!("Failed to write audio file: {}", e))?;

    tracing::info!("Saved audio recording: {:?}", file_path);

    Ok(file_path.to_string_lossy().to_string())
}

/// Add a recording to the knowledge graph
///
/// # Arguments
/// * `agent_id` - The ID of the agent
/// * `filename` - The name of the audio file
/// * `duration_seconds` - The duration of the recording in seconds
/// * `timestamp` - ISO 8601 timestamp of when the recording was made
///
/// # Returns
/// The ID of the created knowledge graph entity
#[tauri::command]
pub async fn add_recording_to_kg(
    agent_id: String,
    filename: String,
    duration_seconds: i64,
    timestamp: String,
) -> Result<String, String> {
    use knowledge_graph::GraphStorage;

    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    let db_path = dirs.db_dir.join("knowledge-graph.db");
    let storage = GraphStorage::new(db_path)
        .map_err(|e| format!("Failed to open knowledge graph: {}", e))?;

    // Create entity name from filename (remove extension for cleaner display)
    let entity_name = filename.replace(".mp3", "").replace(".wav", "").replace(".webm", "");

    // Determine format from extension
    let format = if filename.ends_with(".mp3") {
        "mp3"
    } else if filename.ends_with(".wav") {
        "wav"
    } else if filename.ends_with(".webm") {
        "webm"
    } else {
        "mp3"
    };

    // Create recording entity
    let mut entity = Entity::new(
        agent_id.clone(),
        EntityType::Custom("audio_recording".to_string()),
        entity_name,
    );

    // Add properties to the entity
    entity.properties = HashMap::from([
        ("filename".to_string(), json!(filename)),
        ("duration".to_string(), json!(duration_seconds)),
        ("timestamp".to_string(), json!(timestamp)),
        ("format".to_string(), json!(format)),
    ]);

    // Store in knowledge graph
    let knowledge = ExtractedKnowledge {
        entities: vec![entity],
        relationships: vec![],
    };

    storage.store_knowledge(&agent_id, knowledge).await
        .map_err(|e| format!("Failed to store entity: {}", e))?;

    tracing::info!("Added recording to knowledge graph: agent={}", agent_id);

    Ok(filename)
}

// ============================================================================
// TRANSCRIPTION COMMANDS
// ============================================================================

/// Install the transcription script to the utils directory
#[tauri::command]
pub async fn install_transcription_script() -> Result<String, String> {
    use crate::transcription::TranscriptionService;

    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    let script_path = dirs.utils_dir.join("transcribe.py");

    // Read the embedded script
    let script_content = include_str!("../../scripts/transcribe.py");

    // Create utils directory if it doesn't exist
    fs::create_dir_all(&dirs.utils_dir)
        .map_err(|e| format!("Failed to create utils directory: {}", e))?;

    // Write the script
    fs::write(&script_path, script_content)
        .map_err(|e| format!("Failed to write script: {}", e))?;

    tracing::info!("Installed transcription script: {:?}", script_path);

    Ok(format!("Transcription script installed to: {:?}", script_path))
}

/// Check if transcription dependencies are installed
#[tauri::command]
pub async fn check_transcription_dependencies() -> Result<bool, String> {
    use crate::transcription::TranscriptionService;

    let service = TranscriptionService::new()
        .map_err(|e| e.to_string())?;

    service.check_dependencies()
        .map_err(|e| e.to_string())
}

/// Transcribe a recording
#[tauri::command]
pub async fn transcribe_recording(
    agent_id: String,
    filename: String,
    num_speakers: Option<u32>,
) -> Result<String, String> {
    use crate::transcription::TranscriptionService;

    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    // Find audio file
    // Filename format: recording-YYYYMMDD-HHMMSS.wav
    // Extract YYYY-MM from filename (date has no hyphen, need to insert it)
    let year = &filename[10..14];   // "2026"
    let month = &filename[14..16]; // "01"
    let year_month = format!("{}-{}", year, month); // "2026-01"
    let audio_path = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media")
        .join(&year_month)
        .join(&filename);

    if !audio_path.exists() {
        return Err(format!("Audio file not found: {:?}", audio_path));
    }

    // Output is same directory as audio
    let output_dir = audio_path.parent().unwrap();

    // Transcribe
    let service = TranscriptionService::new()
        .map_err(|e| format!("Failed to create transcription service: {}", e))?;

    let transcript = service.transcribe(&audio_path, output_dir, num_speakers).await
        .map_err(|e| format!("Transcription failed: {}", e))?;

    // Update knowledge graph with transcript
    add_transcript_to_kg(agent_id, filename.clone(), transcript).await?;

    Ok(filename)
}

/// Get transcript for a recording
#[tauri::command]
pub async fn get_recording_transcript(
    agent_id: String,
    filename: String,
) -> Result<crate::transcription::Transcript, String> {
    use crate::transcription::TranscriptionService;

    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    // Find transcript JSON file
    // Filename format: recording-YYYYMMDD-HHMMSS.wav
    // Extract YYYY-MM from filename (date has no hyphen, need to insert it)
    let year = &filename[10..14];   // "2026"
    let month = &filename[14..16]; // "01"
    let year_month = format!("{}-{}", year, month); // "2026-01"
    let audio_name = &filename[..filename.len()-4]; // Remove .wav
    let transcript_path = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media")
        .join(&year_month)
        .join(format!("{}.json", audio_name));

    let service = TranscriptionService::new()
        .map_err(|e| e.to_string())?;

    service.get_transcript(&transcript_path)
        .map_err(|e| e.to_string())
}

/// Check if transcript exists
#[tauri::command]
pub async fn has_transcript(
    agent_id: String,
    filename: String,
) -> Result<bool, String> {
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    // Filename format: recording-YYYYMMDD-HHMMSS.wav
    // Extract YYYY-MM from filename (date has no hyphen, need to insert it)
    let year = &filename[10..14];   // "2026"
    let month = &filename[14..16]; // "01"
    let year_month = format!("{}-{}", year, month); // "2026-01"
    let audio_name = &filename[..filename.len()-4]; // Remove .wav
    let transcript_path = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media")
        .join(&year_month)
        .join(format!("{}.json", audio_name));

    Ok(transcript_path.exists())
}

/// Get transcript attachment info for chat display
///
/// Returns metadata about a transcript for displaying it as a file attachment
#[tauri::command]
pub async fn get_transcript_attachment_info(
    agent_id: String,
    filename: String,
) -> Result<TranscriptAttachmentInfo, String> {
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| e.to_string())?;

    // Filename format: recording-YYYYMMDD-HHMMSS.wav
    let year = &filename[10..14];
    let month = &filename[14..16];
    let year_month = format!("{}-{}", year, month);
    let audio_name = &filename[..filename.len()-4];

    // Get transcript
    let transcript_path = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media")
        .join(&year_month)
        .join(format!("{}.json", audio_name));

    let service = crate::transcription::TranscriptionService::new()
        .map_err(|e| e.to_string())?;

    let transcript = service.get_transcript(&transcript_path)
        .map_err(|e| e.to_string())?;

    // Build the plain text file path for agent to use with grep/rg
    let segments_path = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media")
        .join(&year_month)
        .join(format!("{}_segments.txt", audio_name));

    Ok(TranscriptAttachmentInfo {
        filename: filename.clone(),
        file_path: segments_path.to_string_lossy().to_string(),
        audio_file: transcript.audio_file.clone(),
        duration_seconds: transcript.duration_seconds,
        speaker_count: transcript.speakers.len(),
        segment_count: transcript.segments.len(),
        plain_text: format_transcript_as_text(&transcript),
    })
}

/// Transcript attachment info for chat display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptAttachmentInfo {
    pub filename: String,
    pub file_path: String,  // Full path to plain text file for grep/rg
    pub audio_file: String,
    pub duration_seconds: f32,
    pub speaker_count: usize,
    pub segment_count: usize,
    pub plain_text: String,
}

/// Format transcript as plain text for attachment preview
fn format_transcript_as_text(transcript: &crate::transcription::Transcript) -> String {
    transcript.segments.iter()
        .map(|s| format!("{}: {}", s.speaker_label, s.text))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Add transcript to knowledge graph
async fn add_transcript_to_kg(
    agent_id: String,
    filename: String,
    transcript: crate::transcription::Transcript,
) -> Result<(), String> {
    use knowledge_graph::GraphStorage;

    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;

    let db_path = dirs.db_dir.join("knowledge-graph.db");
    let storage = GraphStorage::new(db_path)
        .map_err(|e| format!("Failed to open knowledge graph: {}", e))?;

    // Create entity name from filename (remove extension for cleaner display)
    let entity_name = filename.replace(".wav", "").replace(".mp3", "").replace(".webm", "");

    // Create transcript entity
    let mut entity = Entity::new(
        agent_id.clone(),
        EntityType::Custom("transcript".to_string()),
        entity_name,
    );

    // Add properties to the entity
    let mut props = HashMap::new();
    props.insert("audio_file".to_string(), json!(filename));
    props.insert("duration".to_string(), json!(transcript.duration_seconds));
    props.insert("speaker_count".to_string(), json!(transcript.speakers.len()));
    props.insert("segments".to_string(), json!(transcript.segments));
    entity.properties = props;

    // Store in knowledge graph
    let knowledge = ExtractedKnowledge {
        entities: vec![entity],
        relationships: vec![],
    };

    storage.store_knowledge(&agent_id, knowledge).await
        .map_err(|e| format!("Failed to store entity: {}", e))?;

    tracing::info!("Added transcript to knowledge graph: agent={}", agent_id);

    Ok(())
}

// ============================================================================
// ATTACHMENT COMMANDS
// ============================================================================

/// Attachment metadata for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub id: String,
    pub r#type: String,  // "transcript" or "file"
    pub filename: String,
    pub file_path: String,
    pub created_at: i64,
    pub duration_seconds: Option<f32>,
    pub speaker_count: Option<usize>,
}

/// List all attachments for an agent
#[tauri::command]
pub async fn list_attachments(
    agent_id: String,
) -> Result<Vec<AttachmentInfo>, String> {
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| e.to_string())?;

    let media_base_dir = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media");

    if !media_base_dir.exists() {
        return Ok(vec![]);
    }

    let mut attachments = Vec::new();

    // Scan all YYYY-MM directories
    let entries = fs::read_dir(&media_base_dir)
        .map_err(|e| format!("Failed to read media directory: {}", e))?;

    for entry in entries.flatten() {
        let year_month_dir = entry.path();
        if !year_month_dir.is_dir() {
            continue;
        }

        // Scan for .json transcript files
        let transcript_entries = fs::read_dir(&year_month_dir)
            .map_err(|e| format!("Failed to read year-month directory: {}", e))?;

        for transcript_entry in transcript_entries.flatten() {
            let path = transcript_entry.path();
            let filename = transcript_entry.file_name();

            // Only process .json transcript files
            if filename.to_string_lossy().ends_with(".json") {
                // Get metadata without loading full transcript
                let metadata = fs::metadata(&path)
                    .map_err(|e| format!("Failed to get file metadata: {}", e))?;
                let created_at = metadata.created()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                // Get transcript info
                let service = crate::transcription::TranscriptionService::new()
                    .map_err(|e| e.to_string())?;

                if let Ok(transcript) = service.get_transcript(&path) {
                    // Build plain text file path
                    let filename_str = filename.to_string_lossy();
                    let audio_name = filename_str
                        .trim_end_matches(".json");
                    let file_path = year_month_dir.join(format!("{}_segments.txt", audio_name));

                    attachments.push(AttachmentInfo {
                        id: format!("transcript-{}", filename.to_string_lossy()),
                        r#type: "transcript".to_string(),
                        filename: filename.to_string_lossy().to_string(),
                        file_path: file_path.to_string_lossy().to_string(),
                        created_at,
                        duration_seconds: Some(transcript.duration_seconds),
                        speaker_count: Some(transcript.speakers.len()),
                    });
                }
            }
        }
    }

    // Sort by creation date (newest first)
    attachments.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(attachments)
}

/// Get attachment details with full transcript
#[tauri::command]
pub async fn get_attachment(
    agent_id: String,
    attachment_id: String,
) -> Result<crate::transcription::Transcript, String> {
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| e.to_string())?;

    // Extract filename from attachment_id (format: transcript-<filename>.json)
    let filename = attachment_id.trim_start_matches("transcript-");

    // Parse the filename to get year_month
    // Format: recording-YYYYMMDD-HHMMSS.json
    let year = &filename.get(10..14).unwrap_or("2024");
    let month = &filename.get(14..16).unwrap_or("01");
    let year_month = format!("{}-{}", year, month);

    let transcript_path = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media")
        .join(&year_month)
        .join(filename);

    if !transcript_path.exists() {
        return Err(format!("Transcript not found: {}", filename));
    }

    let service = crate::transcription::TranscriptionService::new()
        .map_err(|e| e.to_string())?;

    service.get_transcript(&transcript_path)
        .map_err(|e| e.to_string())
}

/// Delete an attachment (audio, transcript JSON, and plain text files)
#[tauri::command]
pub async fn delete_attachment(
    agent_id: String,
    attachment_id: String,
) -> Result<(), String> {
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| e.to_string())?;

    // Extract filename from attachment_id
    let filename = attachment_id.trim_start_matches("transcript-");
    let audio_name = filename.trim_end_matches(".json");

    // Parse year_month from filename
    let year = filename.get(10..14).unwrap_or("2024");
    let month = filename.get(14..16).unwrap_or("01");
    let year_month = format!("{}-{}", year, month);

    let media_dir = dirs.config_dir
        .join("agents_data")
        .join(&agent_id)
        .join("media")
        .join(&year_month);

    // Delete audio file (could be .wav, .mp3, or .webm)
    for ext in &["wav", "mp3", "webm"] {
        let audio_path = media_dir.join(format!("{}.{}", audio_name, ext));
        if audio_path.exists() {
            fs::remove_file(&audio_path)
                .map_err(|e| format!("Failed to delete audio file: {}", e))?;
            tracing::info!("Deleted audio file: {:?}", audio_path);
        }
    }

    // Delete transcript JSON
    let transcript_path = media_dir.join(filename);
    if transcript_path.exists() {
        fs::remove_file(&transcript_path)
            .map_err(|e| format!("Failed to delete transcript: {}", e))?;
        tracing::info!("Deleted transcript: {:?}", transcript_path);
    }

    // Delete plain text segments file
    let segments_path = media_dir.join(format!("{}_segments.txt", audio_name));
    if segments_path.exists() {
        fs::remove_file(&segments_path)
            .map_err(|e| format!("Failed to delete segments file: {}", e))?;
        tracing::info!("Deleted segments file: {:?}", segments_path);
    }

    Ok(())
}
