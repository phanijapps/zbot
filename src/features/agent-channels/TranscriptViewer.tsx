// ============================================================================
// TRANSCRIPT VIEWER
// Display transcript with speaker labels and timestamps
// ============================================================================

import { FileText } from "lucide-react";

export interface TranscriptSegment {
  speaker_id: string;
  speaker_label: string;
  start_time: number;
  end_time: number;
  text: string;
}

export interface SpeakerInfo {
  id: string;
  label: string;
  segments_count: number;
  total_duration: number;
}

export interface Transcript {
  audio_file: string;
  duration_seconds: number;
  created_at: number;
  speakers: SpeakerInfo[];
  segments: TranscriptSegment[];
}

interface TranscriptViewerProps {
  transcript: Transcript;
  onEdit?: (segmentIndex: number, newText: string) => void;
}

export function TranscriptViewer({ transcript }: TranscriptViewerProps) {
  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  // Handle inaudible/untranscribable segments
  const getDisplayText = (text: string) => {
    if (!text || text.trim().length === 0) {
      return "Cannot be transcribed";
    }
    return text;
  };

  return (
    <div className="flex flex-col">
      {/* Header */}
      <div className="flex items-center gap-3 p-4 border-b border-white/10">
        <FileText className="size-5 text-gray-400" />
        <div>
          <h3 className="text-white font-medium">Transcript</h3>
          <p className="text-sm text-gray-400">
            {transcript.speakers.length} speaker{transcript.speakers.length !== 1 ? 's' : ''} ·
            {formatTime(transcript.duration_seconds)}
          </p>
        </div>
      </div>

      {/* Segments */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {transcript.segments.map((segment, index) => (
          <div
            key={index}
            className="flex gap-3 group"
          >
            {/* Speaker label */}
            <div className="flex-shrink-0 w-28">
              <span className="text-sm font-medium text-gray-300">
                {segment.speaker_label}:
              </span>
            </div>

            {/* Segment text */}
            <div className="flex-1 min-w-0">
              <p className="text-gray-100 break-words">
                {getDisplayText(segment.text)}
              </p>
              {segment.start_time > 0 && (
                <span className="text-xs text-gray-500">
                  {formatTime(segment.start_time)}
                </span>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Speaker summary */}
      <div className="border-t border-white/10 p-4">
        <h4 className="text-sm font-medium text-gray-400 mb-2">Speakers</h4>
        <div className="flex gap-4">
          {transcript.speakers.map((speaker) => (
            <div key={speaker.id} className="text-sm">
              <span className="text-gray-300">{speaker.label}</span>
              <span className="text-gray-500 ml-2">
                ({Math.round(speaker.total_duration / 60)}:{(speaker.total_duration % 60).toFixed(0).padStart(2, '0')})
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
