// ============================================================================
// AUDIO RECORDER
// Cross-platform audio recording using cpal
// ============================================================================

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SupportedStreamConfig};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Audio recorder state
pub struct AudioRecorder {
    is_recording: Arc<TokioMutex<bool>>,
    data_buffer: Arc<TokioMutex<Vec<u8>>>,
    sample_rate: Arc<TokioMutex<u32>>,
    channels: Arc<TokioMutex<u16>>,
}

// Implement Clone for AudioRecorder
impl Clone for AudioRecorder {
    fn clone(&self) -> Self {
        Self {
            is_recording: self.is_recording.clone(),
            data_buffer: self.data_buffer.clone(),
            sample_rate: self.sample_rate.clone(),
            channels: self.channels.clone(),
        }
    }
}

impl AudioRecorder {
    /// Create a new audio recorder
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(TokioMutex::new(false)),
            data_buffer: Arc::new(TokioMutex::new(Vec::new())),
            sample_rate: Arc::new(TokioMutex::new(48000)),
            channels: Arc::new(TokioMutex::new(1)),
        }
    }

    /// Get list of available audio input devices
    pub fn get_input_devices() -> Result<Vec<String>, String> {
        let host = cpal::default_host();
        let mut devices = Vec::new();

        if let Ok(input_devices) = host.input_devices() {
            for device in input_devices {
                if let Ok(name) = device.name() {
                    devices.push(name);
                }
            }
        }

        if devices.is_empty() {
            return Err("No audio input devices found".to_string());
        }

        Ok(devices)
    }

    /// Get the default input device
    pub fn get_default_input_device() -> Result<Device, String> {
        let host = cpal::default_host();
        host.default_input_device()
            .ok_or_else(|| "Failed to get default input device".to_string())
    }

    /// Get current sample rate
    pub async fn get_sample_rate(&self) -> u32 {
        *self.sample_rate.lock().await
    }

    /// Get current channels
    pub async fn get_channels(&self) -> u16 {
        *self.channels.lock().await
    }

    /// Start recording
    pub async fn start_recording(&self) -> Result<cpal::StreamConfig, String> {
        let mut is_recording = self.is_recording.lock().await;

        if *is_recording {
            return Err("Already recording".to_string());
        }

        *is_recording = true;
        drop(is_recording);

        // Clear previous data
        if let Ok(mut buffer) = self.data_buffer.try_lock() {
            buffer.clear();
        }

        // Get default input device
        let device = Self::get_default_input_device()?;

        // Get supported config
        let supported = device.supported_input_configs()
            .map_err(|e| format!("Failed to get supported configs: {}", e))?
            .find(|c| {
                // Prefer 16-bit or 32-bit format, single channel or stereo
                matches!(c.sample_format(), SampleFormat::I16 | SampleFormat::I32 | SampleFormat::F32)
                    && (c.channels() == 1 || c.channels() == 2)
                    && c.min_sample_rate() <= cpal::SampleRate(48000)
                    && c.max_sample_rate() >= cpal::SampleRate(48000)
            })
            .ok_or_else(|| "No suitable audio config found".to_string())?;

        // Use 48000 Hz as sample rate, or the max if 48000 is not available
        let sample_rate = if supported.max_sample_rate() >= cpal::SampleRate(48000) {
            cpal::SampleRate(48000)
        } else {
            supported.max_sample_rate()
        };

        // Convert to StreamConfig
        let config: cpal::StreamConfig = supported.with_sample_rate(sample_rate).into();

        // Store sample rate and channels
        if let Ok(mut sr) = self.sample_rate.try_lock() {
            *sr = config.sample_rate.0;
        }
        if let Ok(mut ch) = self.channels.try_lock() {
            *ch = config.channels;
        }

        let sample_format = supported.sample_format();
        let data_buffer = self.data_buffer.clone();
        let is_recording_clone = self.is_recording.clone();
        let config_clone = config.clone();

        // Spawn recording thread
        tokio::task::spawn_blocking(move || {
            let device = Self::get_default_input_device()?;

            match sample_format {
                SampleFormat::I16 => {
                    let err_fn = move |err: cpal::StreamError| {
                        tracing::error!("Audio stream error: {:?}", err);
                    };

                    let data_callback = move |data: &[i16], _info: &cpal::InputCallbackInfo| {
                        if let Ok(mut buffer) = data_buffer.try_lock() {
                            // Convert i16 samples to bytes (little-endian WAV format)
                            for &sample in data {
                                buffer.extend_from_slice(&sample.to_le_bytes());
                            }
                        }
                    };

                    // Build stream with proper cpal v0.15 API
                    let stream = device.build_input_stream(
                        &config_clone,
                        data_callback,
                        err_fn,
                        None,
                    ).map_err(|e| format!("Failed to build i16 stream: {}", e))?;

                    stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;

                    // Keep recording while flag is set
                    while *is_recording_clone.blocking_lock() {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    drop(stream);
                    Ok::<(), String>(())
                }
                SampleFormat::I32 => {
                    let err_fn = move |err: cpal::StreamError| {
                        tracing::error!("Audio stream error: {:?}", err);
                    };

                    let data_callback = move |data: &[i32], _info: &cpal::InputCallbackInfo| {
                        if let Ok(mut buffer) = data_buffer.try_lock() {
                            // Convert i32 samples to i16 (downscale)
                            for &sample in data {
                                let scaled = (sample / 65536) as i16;
                                buffer.extend_from_slice(&scaled.to_le_bytes());
                            }
                        }
                    };

                    let stream = device.build_input_stream(
                        &config_clone,
                        data_callback,
                        err_fn,
                        None,
                    ).map_err(|e| format!("Failed to build i32 stream: {}", e))?;

                    stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;

                    while *is_recording_clone.blocking_lock() {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    drop(stream);
                    Ok(())
                }
                SampleFormat::F32 => {
                    let err_fn = move |err: cpal::StreamError| {
                        tracing::error!("Audio stream error: {:?}", err);
                    };

                    let data_callback = move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                        if let Ok(mut buffer) = data_buffer.try_lock() {
                            // Convert f32 samples to i16
                            for &sample in data {
                                let scaled = (sample * 32768.0) as i16;
                                buffer.extend_from_slice(&scaled.to_le_bytes());
                            }
                        }
                    };

                    let stream = device.build_input_stream(
                        &config_clone,
                        data_callback,
                        err_fn,
                        None,
                    ).map_err(|e| format!("Failed to build f32 stream: {}", e))?;

                    stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;

                    while *is_recording_clone.blocking_lock() {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    drop(stream);
                    Ok(())
                }
                _ => Err("Unsupported sample format".to_string()),
            }
        });

        // Wait a bit for the stream to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(config)
    }

    /// Stop recording and return the audio data as WAV bytes
    pub async fn stop_recording(&self) -> Result<Vec<u8>, String> {
        let mut is_recording = self.is_recording.lock().await;

        if !*is_recording {
            return Err("Not recording".to_string());
        }

        *is_recording = false;
        drop(is_recording);

        // Give time for the stream to finish
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Get the recorded data
        let data = self.data_buffer.lock().await;

        if data.is_empty() {
            return Err("No audio data recorded".to_string());
        }

        let sample_rate = *self.sample_rate.lock().await;
        let channels = *self.channels.lock().await;

        // Convert to WAV format
        Ok(self.write_wav(&data, sample_rate, channels))
    }

    /// Check if currently recording
    pub async fn is_recording(&self) -> Result<bool, String> {
        Ok(*self.is_recording.lock().await)
    }

    /// Convert raw PCM data to WAV format
    fn write_wav(&self, data: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
        let bytes_per_sample = 2u16; // i16
        let data_size = data.len() as u32;
        let file_size = 36 + data_size;
        let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
        let block_align = channels * bytes_per_sample;

        let mut wav = Vec::with_capacity(data.len() + 44);

        // RIFF header
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());

        // WAVE header
        wav.extend_from_slice(b"WAVE");

        // fmt chunk
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        wav.extend_from_slice(&1u16.to_le_bytes());  // audio format (PCM)
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

        // data chunk
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.extend_from_slice(data);

        wav
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}
