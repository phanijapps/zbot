#!/usr/bin/env python3
"""
Transcribe and diarize audio files using Hugging Face models

Requirements (add to venv/requirements.txt):
    pyannote.audio>=3.1.0
    transformers>=4.35.0
    torch>=2.0.0
    accelerate>=0.24.0

Usage:
    python transcribe.py <audio_path> <output_dir> [<num_speakers>]
"""

import sys
import json
from pathlib import Path

def transcribe_and_diarize(audio_path: str, output_dir: str, num_speakers: int = None) -> dict:
    """
    Process audio file and return transcript with speaker segments

    Args:
        audio_path: Path to WAV audio file
        output_dir: Directory to save transcript files
        num_speakers: Optional number of speakers (auto-detect if None)

    Returns:
        Dictionary with transcript data
    """
    from pyannote.audio import Pipeline
    from transformers import pipeline as hf_pipeline
    import torch

    audio_file = Path(audio_path)
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    # Determine device
    device = "cuda" if torch.cuda.is_available() else "cpu"

    # Step 1: Run diarization
    diarization_pipeline = Pipeline.from_pretrained(
        "pyannote/speaker-diarization-3.1"
    )
    diarization_pipeline.to(device)
    diarization = diarization_pipeline(str(audio_file), num_speakers=num_speakers)

    # Step 2: Transcribe using Hugging Face Whisper pipeline
    # Use the automatic-speech-recognition pipeline which handles timestamps
    whisper = hf_pipeline(
        "automatic-speech-recognition",
        model="openai/whisper-base",
        chunk_length_s=30,
        device=device
    )

    # Transcribe with timestamps
    transcription = whisper(
        str(audio_file),
        batch_size=8,
        return_timestamps=True,
        generate_kwargs={"language": "english"}
    )

    # Extract segments with timestamps
    raw_segments = []
    if "chunks" in transcription:
        for chunk in transcription["chunks"]:
            if isinstance(chunk, dict) and "text" in chunk:
                text = chunk["text"].strip()
                if "timestamp" in chunk:
                    start, end = chunk["timestamp"]
                    raw_segments.append({
                        "text": text,
                        "start": start if start is not None else 0.0,
                        "end": end if end is not None else start + 1.0 if start is not None else 1.0
                    })
    elif "text" in transcription:
        # Fallback if no chunks returned
        raw_segments.append({
            "text": transcription["text"].strip(),
            "start": 0.0,
            "end": 30.0  # Default duration
        })

    # Step 3: Merge diarization with transcription
    results = []
    speaker_map = {}
    speaker_idx = 0

    for turn, _, speaker in diarization.itertracks(yield_label=True):
        if speaker not in speaker_map:
            speaker_map[speaker] = f"Speaker {speaker_idx + 1}"
            speaker_idx += 1

        speaker_label = speaker_map[speaker]

        # Find transcription segments within this speaker turn
        for segment in raw_segments:
            # Check if segment overlaps with this speaker turn
            segment_center = (segment["start"] + segment["end"]) / 2
            if turn.start <= segment_center <= turn.end:
                results.append({
                    "speaker_id": speaker,
                    "speaker_label": speaker_label,
                    "start_time": round(segment["start"], 2),
                    "end_time": round(segment["end"], 2),
                    "text": segment["text"]
                })

    # Build output structure
    speakers_list = []
    for sp_id, sp_label in speaker_map.items():
        sp_segments = [r for r in results if r["speaker_id"] == sp_id]
        total_dur = sum(r["end_time"] - r["start_time"] for r in sp_segments)
        speakers_list.append({
            "id": sp_id,
            "label": sp_label,
            "segments_count": len(sp_segments),
            "total_duration": round(total_dur, 2)
        })

    # Calculate total duration from audio file
    import wave
    try:
        with wave.open(str(audio_file), 'rb') as wf:
            frames = wf.getnframes()
            rate = wf.getframerate()
            duration = frames / float(rate)
    except Exception:
        # Fallback if wave reading fails
        duration = sum(s["total_duration"] for s in speakers_list)

    output = {
        "audio_file": audio_file.name,
        "duration_seconds": round(duration, 2),
        "created_at": audio_file.stat().st_mtime,
        "speakers": speakers_list,
        "segments": results
    }

    # Save JSON metadata
    json_path = output_path / f"{audio_file.stem}.json"
    with open(json_path, 'w') as f:
        json.dump(output, f, indent=2)

    # Save plain text transcript
    txt_path = output_path / f"{audio_file.stem}_segments.txt"
    with open(txt_path, 'w') as f:
        for seg in results:
            text = seg["text"] if seg["text"].strip() else "Cannot be transcribed"
            f.write(f"{seg['speaker_label']}: {text}\n")

    return output

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python transcribe.py <audio_path> <output_dir> [num_speakers]", file=sys.stderr)
        sys.exit(1)

    audio_path = sys.argv[1]
    output_dir = sys.argv[2]
    num_speakers = int(sys.argv[3]) if len(sys.argv) > 3 else None

    try:
        result = transcribe_and_diarize(audio_path, output_dir, num_speakers)
        print(json.dumps(result))
    except Exception as e:
        import traceback
        error_output = {
            "error": str(e),
            "type": type(e).__name__,
            "traceback": traceback.format_exc()
        }
        print(json.dumps(error_output), file=sys.stderr)
        sys.exit(1)
