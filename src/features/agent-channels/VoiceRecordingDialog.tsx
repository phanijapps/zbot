// ============================================================================
// VOICE RECORDING DIALOG
// Modal dialog for recording voice notes with animated timer
// Uses Rust-based audio recording (cpal) instead of browser MediaRecorder API
// ============================================================================

import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Square } from "lucide-react";

interface VoiceRecordingDialogProps {
  open: boolean;
  onClose: () => void;
  agentId: string;
  agentName: string;
  onTranscriptComplete?: (filename: string) => void;
}

export function VoiceRecordingDialog({
  open,
  onClose,
  agentId,
  agentName,
  onTranscriptComplete,
}: VoiceRecordingDialogProps) {
  const [isRecording, setIsRecording] = useState(false);
  const [elapsedTime, setElapsedTime] = useState(0);
  const [audioData, setAudioData] = useState<{ base64: string; sampleRate: number; channels: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const timerRef = useRef<number | null>(null);

  // Format time as MM:SS
  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  // Start recording using Rust command
  const startRecording = async () => {
    setError(null);

    try {
      console.log("Starting audio recording...");
      const result = await invoke<[number, number]>("start_audio_recording");
      const [sampleRate, channels] = result;

      console.log(`Recording started: ${sampleRate}Hz, ${channels} channels`);
      setIsRecording(true);
      setElapsedTime(0);

      // Start timer
      timerRef.current = window.setInterval(() => {
        setElapsedTime(prev => prev + 1);
      }, 1000);

    } catch (err) {
      console.error("Failed to start recording:", err);
      const errorMessage = err instanceof Error ? err.message : String(err);

      if (errorMessage.includes("Already recording")) {
        setError("Recording is already in progress.");
      } else if (errorMessage.includes("No audio input devices")) {
        setError("No microphone found. Please connect a microphone and try again.");
      } else {
        setError(`Failed to start recording: ${errorMessage}`);
      }
    }
  };

  // Stop recording and get audio data
  const stopRecording = async () => {
    if (!isRecording) return;

    try {
      console.log("Stopping audio recording...");
      const result = await invoke<[string, number, number]>("stop_audio_recording");
      const [base64, sampleRate, channels] = result;

      console.log(`Recording stopped: ${base64.length} bytes, ${sampleRate}Hz, ${channels} channels`);
      setAudioData({ base64, sampleRate, channels });
      setIsRecording(false);

      // Stop timer
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }

    } catch (err) {
      console.error("Failed to stop recording:", err);
      const errorMessage = err instanceof Error ? err.message : String(err);

      if (errorMessage.includes("Not recording")) {
        setError("No recording in progress.");
      } else if (errorMessage.includes("No audio data")) {
        setError("No audio data recorded. Please try again.");
      } else {
        setError(`Failed to stop recording: ${errorMessage}`);
      }

      // Reset recording state
      setIsRecording(false);
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    }
  };

  // Save recording when audioData is ready
  useEffect(() => {
    const saveRecording = async () => {
      if (!audioData) return;

      setIsSaving(true);
      setError(null);

      try {
        // Generate filename: recording-YYYYMMDD-HHMMSS.wav
        const now = new Date();
        const timestamp = now.toISOString();
        const filename = `recording-${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, '0')}${String(now.getDate()).padStart(2, '0')}-${String(now.getHours()).padStart(2, '0')}${String(now.getMinutes()).padStart(2, '0')}${String(now.getSeconds()).padStart(2, '0')}.wav`;

        // Save to file system via Rust command
        const filePath = await invoke<string>("save_audio_recording", {
          agentId,
          filename,
          audioBase64: audioData.base64,
        });

        console.log("Recording saved:", filePath);

        // Add to knowledge graph
        await invoke<string>("add_recording_to_kg", {
          agentId,
          filename,
          durationSeconds: elapsedTime,
          timestamp,
        });

        console.log("Added to knowledge graph");

        // Trigger transcription (optional, don't fail if it errors)
        try {
          await invoke<string>("transcribe_recording", {
            agentId,
            filename,
            numSpeakers: null, // Auto-detect
          });
          console.log("Transcription completed");

          // Notify parent that transcript is ready
          onTranscriptComplete?.(filename);
        } catch (transcribeErr) {
          console.error("Failed to start transcription (non-critical):", transcribeErr);
          // Don't fail - transcription is optional
        }

        // Close dialog after brief delay
        setTimeout(() => {
          onClose();
          setAudioData(null);
          setElapsedTime(0);
          setIsSaving(false);
        }, 500);

      } catch (err) {
        console.error("Failed to save recording:", err);
        setError("Failed to save recording. Please try again.");
        setIsSaving(false);
      }
    };

    saveRecording();
  }, [audioData, agentId, elapsedTime, onClose]);

  // Auto-start recording when dialog opens
  useEffect(() => {
    if (open && !isRecording && !audioData && !error) {
      startRecording();
    }
  }, [open]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, []);

  if (!open) return null;

  return (
    <div className="fixed inset-0 bg-black/90 z-50 flex items-center justify-center">
      <div className="flex flex-col items-center gap-8">
        {/* Recording indicator */}
        <div className="relative">
          {/* Pulse animation */}
          <div className={`absolute inset-0 rounded-full bg-red-500/30 ${
            isRecording ? "animate-ping" : ""
          }`} style={{ width: '220px', height: '220px' }} />

          {/* Outer circle */}
          <div className="relative w-56 h-56 rounded-full border-4 border-red-500/50 flex items-center justify-center bg-black/50 backdrop-blur-sm shadow-2xl">

            {/* Timer */}
            <div className="text-center">
              <div className="text-6xl font-mono text-white mb-2 tabular-nums">
                {formatTime(elapsedTime)}
              </div>
              <div className="text-sm text-gray-400">
                {isSaving ? "Saving..." : isRecording ? "Recording..." : "Processing..."}
              </div>
            </div>
          </div>
        </div>

        {/* Stop button */}
        {!isSaving && !audioData && (
          <button
            onClick={stopRecording}
            disabled={!isRecording}
            className="w-20 h-20 rounded-full bg-red-600 hover:bg-red-700 disabled:bg-gray-700 disabled:cursor-not-allowed flex items-center justify-center transition-colors shadow-lg hover:shadow-red-600/20 hover:scale-105 active:scale-95"
            aria-label="Stop recording"
          >
            <Square className="size-8 text-white" fill="white" />
          </button>
        )}

        {/* Saving indicator */}
        {isSaving && (
          <div className="flex items-center gap-3 text-gray-400">
            <div className="animate-spin rounded-full h-6 w-6 border-2 border-red-500 border-t-transparent" />
            <span>Saving recording...</span>
          </div>
        )}

        {/* Error display */}
        {error && (
          <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-4 max-w-md">
            <p className="text-sm text-red-200 text-center">{error}</p>
          </div>
        )}

        {/* Agent name */}
        {!error && (
          <div className="text-gray-400 text-sm">
            Recording for {agentName}
          </div>
        )}
      </div>
    </div>
  );
}
