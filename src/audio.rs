use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use hound::{WavSpec, WavWriter};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::time::Instant;

/// Audio events sent from the capture thread
#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// Audio level update (RMS value 0.0-1.0)
    Level(f32),
    /// Voice activity detected
    VoiceDetected,
    /// Recording started
    RecordingStarted,
    /// Recording stopped, file path provided
    RecordingStopped(PathBuf),
    /// Silence detected
    SilenceDetected,
    /// Error occurred
    Error(String),
}

/// Audio capture and VAD state
struct CaptureState {
    /// Whether we're currently recording
    recording: bool,
    /// Buffer for recorded samples
    buffer: Vec<f32>,
    /// Time when silence was first detected
    silence_start: Option<Instant>,
    /// Time when recording started
    recording_start: Option<Instant>,
    /// Voice threshold
    voice_threshold: f32,
    /// Silence threshold
    silence_threshold: f32,
    /// Silence duration before stopping (seconds)
    silence_duration: f32,
    /// Minimum speech duration (seconds)
    min_speech_duration: f32,
    /// Sample rate
    sample_rate: u32,
    /// Event sender
    event_tx: Sender<AudioEvent>,
}

impl CaptureState {
    fn new(
        voice_threshold: f32,
        silence_threshold: f32,
        silence_duration: f32,
        min_speech_duration: f32,
        sample_rate: u32,
        event_tx: Sender<AudioEvent>,
    ) -> Self {
        Self {
            recording: false,
            buffer: Vec::new(),
            silence_start: None,
            recording_start: None,
            voice_threshold,
            silence_threshold,
            silence_duration,
            min_speech_duration,
            sample_rate,
            event_tx,
        }
    }

    fn process_samples(&mut self, samples: &[f32]) {
        // Calculate RMS
        let rms = calculate_rms(samples);

        // Send level update
        let _ = self.event_tx.send(AudioEvent::Level(rms));

        // State machine logic
        if !self.recording {
            // Not recording - check for voice activity
            if rms > self.voice_threshold {
                // Voice detected!
                self.recording = true;
                self.recording_start = Some(Instant::now());
                self.silence_start = None;
                self.buffer.clear();
                self.buffer.extend_from_slice(samples);
                let _ = self.event_tx.send(AudioEvent::VoiceDetected);
                let _ = self.event_tx.send(AudioEvent::RecordingStarted);
            }
        } else {
            // Recording - add to buffer and check for silence
            self.buffer.extend_from_slice(samples);

            if rms < self.silence_threshold {
                // Silence detected
                if self.silence_start.is_none() {
                    self.silence_start = Some(Instant::now());
                    let _ = self.event_tx.send(AudioEvent::SilenceDetected);
                } else if let Some(silence_start) = self.silence_start {
                    // Check if silence duration exceeded
                    let silence_elapsed = silence_start.elapsed().as_secs_f32();
                    if silence_elapsed >= self.silence_duration {
                        // Check minimum speech duration
                        if let Some(recording_start) = self.recording_start {
                            let recording_elapsed = recording_start.elapsed().as_secs_f32();
                            if recording_elapsed >= self.min_speech_duration {
                                // Stop recording and save
                                self.stop_recording();
                            } else {
                                // Too short, cancel recording
                                self.cancel_recording();
                            }
                        }
                    }
                }
            } else {
                // Voice still active, reset silence timer
                self.silence_start = None;
            }
        }
    }

    fn stop_recording(&mut self) {
        if !self.recording {
            return;
        }

        // Save to temporary WAV file
        match self.save_wav() {
            Ok(path) => {
                let _ = self.event_tx.send(AudioEvent::RecordingStopped(path));
            }
            Err(e) => {
                let _ = self.event_tx.send(AudioEvent::Error(format!("Failed to save audio: {}", e)));
            }
        }

        self.recording = false;
        self.buffer.clear();
        self.silence_start = None;
        self.recording_start = None;
    }

    fn cancel_recording(&mut self) {
        self.recording = false;
        self.buffer.clear();
        self.silence_start = None;
        self.recording_start = None;
    }

    fn save_wav(&self) -> Result<PathBuf> {
        // Create temporary file
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let path = temp_dir.join(format!("thehand_{}.wav", timestamp));

        let spec = WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(&path, spec)
            .context("Failed to create WAV writer")?;

        // Convert f32 samples to i16
        for &sample in &self.buffer {
            let sample_i16 = (sample * i16::MAX as f32) as i16;
            writer.write_sample(sample_i16)
                .context("Failed to write sample")?;
        }

        writer.finalize()
            .context("Failed to finalize WAV file")?;

        Ok(path)
    }
}

/// Calculate RMS (Root Mean Square) of audio samples
fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum: f32 = samples.iter().map(|&s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

/// Audio capture manager
pub struct AudioCapture {
    #[allow(dead_code)]
    stream: Stream,
    event_rx: Receiver<AudioEvent>,
    state: Arc<Mutex<CaptureState>>,
}

impl AudioCapture {
    /// Create a new audio capture instance
    pub fn new(
        voice_threshold: f32,
        silence_threshold: f32,
        silence_duration: f32,
        min_speech_duration: f32,
        sample_rate: u32,
    ) -> Result<Self> {
        let (event_tx, event_rx) = channel();

        let host = cpal::default_host();
        let device = host.default_input_device()
            .context("No input device available")?;

        let config = Self::get_config(&device, sample_rate)?;

        let state = Arc::new(Mutex::new(CaptureState::new(
            voice_threshold,
            silence_threshold,
            silence_duration,
            min_speech_duration,
            sample_rate,
            event_tx.clone(),
        )));

        let stream = Self::build_stream(&device, &config, state.clone())?;
        stream.play().context("Failed to start audio stream")?;

        Ok(Self {
            stream,
            event_rx,
            state,
        })
    }

    /// Get audio stream configuration
    fn get_config(device: &Device, sample_rate: u32) -> Result<StreamConfig> {
        let supported_config = device.default_input_config()
            .context("Failed to get default input config")?;

        Ok(StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        })
    }

    /// Build audio input stream
    fn build_stream(
        device: &Device,
        config: &StreamConfig,
        state: Arc<Mutex<CaptureState>>,
    ) -> Result<Stream> {
        let err_fn = |err| {
            eprintln!("Audio stream error: {}", err);
        };

        let stream = device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut state) = state.lock() {
                    state.process_samples(data);
                }
            },
            err_fn,
            None,
        ).context("Failed to build input stream")?;

        Ok(stream)
    }

    /// Get next audio event (non-blocking)
    pub fn poll_event(&self) -> Option<AudioEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Cancel current recording
    pub fn cancel_recording(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.cancel_recording();
        }
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.state.lock().map(|s| s.recording).unwrap_or(false)
    }
}
