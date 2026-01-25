// ============================================================================
// AGENT CHANNELS TYPES
// Shared types for agent channels feature
// ============================================================================

/// Transcript segment with speaker information
export interface TranscriptSegment {
  speaker_id: string;
  speaker_label: string;
  start_time: number;
  end_time: number;
  text: string;
}

/// Speaker information
export interface SpeakerInfo {
  id: string;
  label: string;
  segments_count: number;
  total_duration: number;
}

/// Complete transcript
export interface Transcript {
  audio_file: string;
  duration_seconds: number;
  created_at: number;
  speakers: SpeakerInfo[];
  segments: TranscriptSegment[];
}

/// Transcript attachment info for chat display
export interface TranscriptAttachmentInfo {
  filename: string;
  audio_file: string;
  duration_seconds: number;
  speaker_count: number;
  segment_count: number;
  plain_text: string;
}
