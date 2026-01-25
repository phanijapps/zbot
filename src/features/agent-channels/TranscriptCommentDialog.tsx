// ============================================================================
// TRANSCRIPT COMMENT DIALOG
// Modal dialog for adding comments to a transcript before sending to agent
// ============================================================================

import { useState } from "react";
import { FileText, X, Send } from "lucide-react";

export interface TranscriptAttachmentInfo {
  filename: string;
  file_path: string;
  audio_file: string;
  duration_seconds: number;
  speaker_count: number;
  segment_count: number;
  plain_text: string;
}

interface TranscriptCommentDialogProps {
  open: boolean;
  transcript: TranscriptAttachmentInfo;
  agentName: string;
  onSend: (comments: string) => void;
  onCancel: () => void;
  loading?: boolean;
}

export function TranscriptCommentDialog({
  open,
  transcript,
  agentName,
  onSend,
  onCancel,
  loading = false,
}: TranscriptCommentDialogProps) {
  const [comments, setComments] = useState("");
  const [previewExpanded, setPreviewExpanded] = useState(false);

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  // Get first few lines of transcript for preview
  const getPreviewLines = () => {
    const lines = transcript.plain_text.split('\n').slice(0, 3);
    const preview = lines.join('\n');
    const remaining = transcript.segment_count - 3;
    return {
      preview,
      remaining,
      hasMore: transcript.segment_count > 3,
    };
  };

  const { preview, remaining, hasMore } = getPreviewLines();

  const handleSend = () => {
    onSend(comments);
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/80 z-50 flex items-center justify-center p-4">
      <div className="bg-gray-900 border border-white/10 rounded-xl max-w-2xl w-full shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-white/10">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-violet-600/20 rounded-lg">
              <FileText className="size-5 text-violet-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-white">Transcript Ready</h2>
              <p className="text-sm text-gray-400">Send to {agentName}</p>
            </div>
          </div>
          <button
            onClick={onCancel}
            disabled={loading}
            className="p-1 text-gray-400 hover:text-white transition-colors disabled:opacity-50"
          >
            <X className="size-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-4 max-h-[60vh] overflow-y-auto">
          {/* Transcript info */}
          <div className="bg-gray-800/50 rounded-lg p-3 border border-white/10">
            <div className="flex items-center justify-between text-sm">
              <span className="text-gray-300 font-mono">{transcript.filename}</span>
              <span className="text-gray-400">
                {formatTime(transcript.duration_seconds)}  {transcript.speaker_count} speaker{transcript.speaker_count !== 1 ? 's' : ''}
              </span>
            </div>
          </div>

          {/* Transcript preview */}
          <div className="border border-white/10 rounded-lg overflow-hidden">
            <button
              onClick={() => setPreviewExpanded(!previewExpanded)}
              className="w-full flex items-center justify-between p-3 bg-gray-800/50 hover:bg-gray-800 transition-colors"
            >
              <span className="text-sm font-medium text-gray-300">Preview</span>
              <span className="text-xs text-gray-500">
                {previewExpanded ? 'Click to collapse' : `Click to expand (${transcript.segment_count} segments)`}
              </span>
            </button>

            {previewExpanded && (
              <div className="p-3 bg-gray-900/50 max-h-48 overflow-y-auto">
                <pre className="text-sm text-gray-300 whitespace-pre-wrap font-sans">
                  {transcript.plain_text}
                </pre>
              </div>
            )}

            {!previewExpanded && (
              <div className="p-3 bg-gray-900/50">
                <p className="text-sm text-gray-300 whitespace-pre-wrap">
                  {preview}
                  {hasMore && (
                    <span className="text-gray-500">
                      {'\n'}... and {remaining} more segment{remaining !== 1 ? 's' : ''}
                    </span>
                  )}
                </p>
              </div>
            )}
          </div>

          {/* Comments textarea */}
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              Add context for the agent (optional)
            </label>
            <textarea
              value={comments}
              onChange={(e) => setComments(e.target.value)}
              placeholder="E.g., This is our Q4 planning meeting. Focus on action items and decisions..."
              disabled={loading}
              rows={4}
              className="w-full bg-gray-800 border border-white/10 rounded-lg px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-violet-500 focus:border-transparent resize-none disabled:opacity-50"
            />
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 p-4 border-t border-white/10">
          <button
            onClick={onCancel}
            disabled={loading}
            className="px-4 py-2 text-gray-300 hover:text-white transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={handleSend}
            disabled={loading}
            className="px-4 py-2 bg-violet-600 hover:bg-violet-700 text-white rounded-lg flex items-center gap-2 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {loading ? (
              <>
                <div className="animate-spin rounded-full h-4 w-4 border-2 border-white border-t-transparent" />
                Sending...
              </>
            ) : (
              <>
                <Send className="size-4" />
                Send to Agent
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
