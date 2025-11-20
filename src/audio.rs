use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use hound::{WavSpec, WavWriter};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::time::Instant;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::io::Write;

/// Helper to write debug messages to a log file
fn debug_log(msg: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/thehand_debug.log")
    {
        let _ = writeln!(file, "{}", msg);
    }
}

/// Information about an audio input device
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub index: usize,
    pub device_name: Option<String>, // Raw device name for manually-added devices
}

/// Helper to suppress stderr during noisy ALSA operations
struct StderrSuppress {
    saved_stderr: i32,
}

impl StderrSuppress {
    fn new() -> Result<Self> {
        unsafe {
            // Duplicate stderr
            let saved_stderr = libc::dup(libc::STDERR_FILENO);
            if saved_stderr == -1 {
                anyhow::bail!("Failed to duplicate stderr");
            }

            // Open /dev/null
            let devnull = OpenOptions::new()
                .write(true)
                .open("/dev/null")
                .context("Failed to open /dev/null")?;

            // Redirect stderr to /dev/null
            libc::dup2(devnull.as_raw_fd(), libc::STDERR_FILENO);

            Ok(Self { saved_stderr })
        }
    }
}

impl Drop for StderrSuppress {
    fn drop(&mut self) {
        unsafe {
            // Restore original stderr
            libc::dup2(self.saved_stderr, libc::STDERR_FILENO);
            libc::close(self.saved_stderr);
        }
    }
}

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

/// Extract friendly name from ALSA device name
fn get_friendly_name(raw_name: &str) -> String {
    // Helper to extract card number from device name
    let extract_card_num = |name: &str| -> Option<String> {
        // Try "hw:CARD=Device,DEV=0" format first
        if let Some(card_part) = name.split("CARD=").nth(1) {
            if let Some(card_name) = card_part.split(',').next() {
                return Some(card_name.to_string());
            }
        }
        // Try "hw:X,Y" format
        if name.starts_with("hw:") {
            if let Some(num) = name.split(':').nth(1).and_then(|s| s.split(',').next()) {
                return Some(num.to_string());
            }
        }
        None
    };

    // Helper to get friendly name from /proc/asound/cards
    let lookup_card_name = |card_identifier: &str| -> Option<String> {
        let cards_content = std::fs::read_to_string("/proc/asound/cards").ok()?;

        for line in cards_content.lines() {
            let trimmed = line.trim();

            // Match by card number or card short name
            let matches = if card_identifier.chars().all(|c| c.is_ascii_digit()) {
                // Numeric card ID
                trimmed.starts_with(card_identifier)
            } else {
                // Short name in brackets
                line.contains(&format!("[{}]", card_identifier)) ||
                line.contains(&format!("[{} ", card_identifier))
            };

            if matches {
                // Extract long name from format: " 0 [NVidia]: HDA-Intel - HDA NVidia"
                // We want "HDA NVidia" (after the last dash)
                if let Some(dash_parts) = line.split(" - ").nth(1) {
                    return Some(dash_parts.trim().to_string());
                }
                // Fallback to bracket name if no dash found
                if let Some(bracket_content) = line.split('[').nth(1) {
                    if let Some(name) = bracket_content.split(']').next() {
                        return Some(name.trim().to_string());
                    }
                }
            }
        }
        None
    };

    // For "default", "pipewire", etc - try to resolve to a card
    if raw_name == "default" || raw_name == "pipewire" {
        if let Some(name) = lookup_card_name("0") {
            return format!("{} ({})", name, raw_name);
        }
        return raw_name.to_string();
    }

    // Try to extract and lookup card identifier
    if let Some(card_id) = extract_card_num(raw_name) {
        if let Some(friendly) = lookup_card_name(&card_id) {
            return friendly;
        }
    }

    // Fallback to original name
    raw_name.to_string()
}

/// List available audio input devices
pub fn list_input_devices() -> Result<Vec<DeviceInfo>> {
    let host = cpal::default_host();

    let devices: Vec<DeviceInfo> = {
        // Suppress ALSA stderr messages during entire device enumeration
        let _suppress = StderrSuppress::new().ok();

        host
            .input_devices()?
            .enumerate()
            .filter_map(|(index, device)| {
                // Try to get device name
                let raw_name = match device.name() {
                    Ok(name) => {
                        debug_log(&format!("DEBUG: Checking device [{}]: {}", index, name));
                        name
                    }
                    Err(e) => {
                        debug_log(&format!("DEBUG: Device [{}] - failed to get name: {}", index, e));
                        return None;
                    }
                };

                // Get device's supported format
                let supported_config = match device.default_input_config() {
                    Ok(cfg) => {
                        debug_log(&format!("DEBUG: Device [{}] {} - supports format: {:?}, rate: {}",
                                 index, raw_name, cfg.sample_format(), cfg.sample_rate().0));
                        cfg
                    }
                    Err(e) => {
                        debug_log(&format!("DEBUG: Device [{}] {} - failed to get config: {}", index, raw_name, e));
                        return None;
                    }
                };

                // Verify device can actually be opened for input using its exact config
                let config = supported_config.config();

                // Try to build a test stream with the device's native format
                let test_stream_result = match supported_config.sample_format() {
                    cpal::SampleFormat::F32 => {
                        device.build_input_stream(
                            &config,
                            |_data: &[f32], _: &cpal::InputCallbackInfo| {},
                            |_err| {},
                            None,
                        )
                    }
                    cpal::SampleFormat::I16 => {
                        device.build_input_stream(
                            &config,
                            |_data: &[i16], _: &cpal::InputCallbackInfo| {},
                            |_err| {},
                            None,
                        )
                    }
                    cpal::SampleFormat::U16 => {
                        device.build_input_stream(
                            &config,
                            |_data: &[u16], _: &cpal::InputCallbackInfo| {},
                            |_err| {},
                            None,
                        )
                    }
                    _ => {
                        debug_log(&format!("DEBUG: Device [{}] {} - unsupported format: {:?}",
                                 index, raw_name, supported_config.sample_format()));
                        return None;
                    }
                };

                let test_stream = match test_stream_result {
                    Ok(stream) => stream,
                    Err(e) => {
                        debug_log(&format!("DEBUG: Device [{}] {} - failed to build test stream: {:?}",
                                 index, raw_name, e));
                        return None;
                    }
                };

                // Clean up test stream
                drop(test_stream);

                let name = get_friendly_name(&raw_name);
                debug_log(&format!("DEBUG: Device [{}] {} - PASSED all tests, adding as '{}'",
                         index, raw_name, name));
                // Store the raw device name so we can open by name instead of index
                Some(DeviceInfo { name, index, device_name: Some(raw_name) })
            })
            .collect()
    };

    // Deduplicate by name, preferring plughw/sysdefault over raw hw devices
    // Process devices and store raw device names for scoring
    let mut devices_with_names: Vec<(DeviceInfo, String)> = Vec::new();
    for device_info in devices {
        if let Some(device) = host.input_devices()?.nth(device_info.index) {
            if let Ok(raw_name) = device.name() {
                devices_with_names.push((device_info, raw_name));
            }
        }
    }

    // Deduplicate, preferring plughw (which matches working arecord command)
    let mut seen_names: std::collections::HashMap<String, (DeviceInfo, String)> = std::collections::HashMap::new();
    for (device_info, raw_name) in devices_with_names {
        if let Some((_existing_info, existing_raw)) = seen_names.get(&device_info.name) {
            // Prefer plughw since that's what worked with arecord
            let current_is_plughw = raw_name.starts_with("plughw:");
            let existing_is_plughw = existing_raw.starts_with("plughw:");

            if current_is_plughw && !existing_is_plughw {
                debug_log(&format!("DEBUG: Replacing {} with {} (plughw preferred)", existing_raw, raw_name));
                seen_names.insert(device_info.name.clone(), (device_info, raw_name));
            } else {
                debug_log(&format!("DEBUG: Filtering out duplicate: {}", raw_name));
            }
        } else {
            seen_names.insert(device_info.name.clone(), (device_info, raw_name));
        }
    }
    // Extract just the names for checking missing devices
    let seen_device_names: std::collections::HashSet<String> = seen_names.keys().cloned().collect();

    let mut devices: Vec<DeviceInfo> = seen_names.into_values().map(|(info, _)| info).collect();

    // Sort by index to ensure consistent ordering
    devices.sort_by_key(|d| d.index);

    // Workaround: cpal sometimes misses devices, so manually check for missing cards
    // by trying to open hw:CARD=name,DEV=0 for each card in /proc/asound/cards
    debug_log("DEBUG: Checking for devices missed by cpal enumeration...");
    if let Ok(cards_content) = std::fs::read_to_string("/proc/asound/cards") {
        let _suppress = StderrSuppress::new().ok();
        for line in cards_content.lines() {
            let _trimmed = line.trim();
            // Parse lines like: " 4 [C200           ]: USB-Audio - Anker PowerConf C200"
            if let Some(bracket_start) = line.find('[') {
                if let Some(bracket_end) = line.find(']') {
                    let card_short_name = line[bracket_start + 1..bracket_end].trim();

                    // Check if we already have this card
                    let friendly_name = if let Some(dash_parts) = line.split(" - ").nth(1) {
                        dash_parts.trim().to_string()
                    } else {
                        card_short_name.to_string()
                    };

                    if seen_device_names.contains(&friendly_name) {
                        continue; // Already have this one
                    }

                    // Try multiple device name variants
                    let device_names = vec![
                        format!("hw:CARD={},DEV=0", card_short_name),
                        format!("plughw:CARD={},DEV=0", card_short_name),
                        format!("sysdefault:CARD={}", card_short_name),
                    ];

                    debug_log(&format!("DEBUG: Manually checking missed card: {}", card_short_name));

                    // Try each device name variant
                    let mut found = false;
                    for device_name in &device_names {
                        debug_log(&format!("DEBUG: Trying device name: {}", device_name));

                        // Try to find this device in cpal's enumeration by name
                        if let Ok(all_devices) = host.input_devices() {
                            for device in all_devices {
                                if let Ok(name) = device.name() {
                                    if name == *device_name {
                                        debug_log(&format!("DEBUG: Found matching device: {}", name));
                                        // Found it! Try to open it
                                        if let Ok(supported_config) = device.default_input_config() {
                                            let config = supported_config.config();

                                            // Try to build a test stream
                                            let test_result = match supported_config.sample_format() {
                                                cpal::SampleFormat::F32 => device.build_input_stream(
                                                    &config,
                                                    |_data: &[f32], _| {},
                                                    |_| {},
                                                    None,
                                                ),
                                                cpal::SampleFormat::I16 => device.build_input_stream(
                                                    &config,
                                                    |_data: &[i16], _| {},
                                                    |_| {},
                                                    None,
                                                ),
                                                cpal::SampleFormat::U16 => device.build_input_stream(
                                                    &config,
                                                    |_data: &[u16], _| {},
                                                    |_| {},
                                                    None,
                                                ),
                                                _ => break,
                                            };

                                            if let Ok(_stream) = test_result {
                                                debug_log(&format!("DEBUG: Successfully added missed device: {} as '{}'", device_name, friendly_name));
                                                devices.push(DeviceInfo {
                                                    name: friendly_name.clone(),
                                                    index: usize::MAX,
                                                    device_name: Some(device_name.clone()),
                                                });
                                                found = true;
                                            }
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                        if found {
                            break; // Stop trying variants once we found one that works
                        }
                    }
                }
            }
        }
    }

    debug_log(&format!("DEBUG: Final device list has {} devices", devices.len()));

    if devices.is_empty() {
        anyhow::bail!("No input devices found");
    }

    Ok(devices)
}

/// Simple audio level preview for device identification
pub struct DevicePreview {
    #[allow(dead_code)]
    stream: Stream,
    level: Arc<Mutex<f32>>,
}

impl DevicePreview {
    /// Create a new device preview for the given device index or name
    pub fn new(device_index: Option<usize>, device_name: Option<String>, _sample_rate: u32) -> Result<Self> {
        // Suppress ALSA stderr messages during preview stream creation
        let _suppress = StderrSuppress::new().ok();

        let host = cpal::default_host();
        let device = if let Some(name) = device_name {
            // Find device by name
            host.input_devices()?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .context(format!("Device not found: {}", name))?
        } else if let Some(index) = device_index {
            host.input_devices()?
                .nth(index)
                .context("Invalid device index")?
        } else {
            anyhow::bail!("Must provide either device_index or device_name");
        };

        // Check if device supports input configuration
        let supported_config = device.default_input_config()
            .context("Device does not support input")?;

        // Use device's exact default config
        let config = supported_config.config();

        let level = Arc::new(Mutex::new(0.0f32));
        let level_clone = level.clone();

        // Build stream based on device's sample format
        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => {
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let rms = calculate_rms(data);
                        if let Ok(mut level) = level_clone.lock() {
                            *level = rms;
                        }
                    },
                    |_err| {},
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let samples: Vec<f32> = data.iter()
                            .map(|&s| s as f32 / i16::MAX as f32)
                            .collect();
                        let rms = calculate_rms(&samples);
                        if let Ok(mut level) = level_clone.lock() {
                            *level = rms;
                        }
                    },
                    |_err| {},
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let samples: Vec<f32> = data.iter()
                            .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                            .collect();
                        let rms = calculate_rms(&samples);
                        if let Ok(mut level) = level_clone.lock() {
                            *level = rms;
                        }
                    },
                    |_err| {},
                    None,
                )?
            }
            format => {
                anyhow::bail!("Unsupported sample format: {:?}", format);
            }
        };

        stream.play()?;

        Ok(Self { stream, level })
    }

    /// Get the current audio level
    pub fn get_level(&self) -> f32 {
        self.level.lock().map(|l| *l).unwrap_or(0.0)
    }
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

        // Debug: Print audio levels
        if rms > 0.001 {
            debug_log(&format!("DEBUG: Audio RMS: {:.4}, threshold: {:.4}, recording: {}",
                     rms, self.voice_threshold, self.recording));
        }

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
        device_index: Option<usize>,
        device_name: Option<String>, // Raw ALSA device name for manually-added devices
    ) -> Result<Self> {
        let (event_tx, event_rx) = channel();

        let host = cpal::default_host();
        let device = if let Some(name) = device_name {
            // Find device by name (preferred method for consistency)
            debug_log(&format!("DEBUG: Looking for device: {}", name));
            host.input_devices()?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .context(format!("Device not found: {}", name))?
        } else if let Some(index) = device_index {
            // Use device index from cpal enumeration (fallback)
            host.input_devices()?
                .nth(index)
                .context("Invalid device index")?
        } else {
            // Use default device
            host.default_input_device()
                .context("No input device available")?
        };

        let config = Self::get_config(&device, sample_rate)?;

        // Use the actual sample rate from the config (device's native rate)
        let actual_sample_rate = config.sample_rate.0;

        debug_log(&format!("DEBUG: Opening device with config: sample_rate={}, channels={}",
                 actual_sample_rate, config.channels));

        let state = Arc::new(Mutex::new(CaptureState::new(
            voice_threshold,
            silence_threshold,
            silence_duration,
            min_speech_duration,
            actual_sample_rate,
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
    fn get_config(device: &Device, _requested_sample_rate: u32) -> Result<StreamConfig> {
        let supported_config = device.default_input_config()
            .context("Failed to get default input config")?;

        let mut config = supported_config.config();

        // Check if this is a USB device that might need mono
        // Force mono for hw:/plughw: devices (not pipewire/default)
        if let Ok(device_name) = device.name() {
            debug_log(&format!("DEBUG: Checking device name for mono: {}", device_name));
            if (device_name.starts_with("hw:") || device_name.starts_with("plughw:"))
               && device_name.contains("LINK") {
                config.channels = 1;
                debug_log(&format!("DEBUG: Forcing mono for USB device: {}", device_name));
            }
        }

        debug_log(&format!("DEBUG: Using config: sample_rate={}, channels={}",
                          config.sample_rate.0, config.channels));

        Ok(config)
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

        // Get the device's supported sample format
        let supported_config = device.default_input_config()
            .context("Failed to get default input config")?;

        // Build stream with appropriate sample format
        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => {
                device.build_input_stream(
                    config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if let Ok(mut state) = state.lock() {
                            state.process_samples(data);
                        }
                    },
                    err_fn,
                    None,
                ).context("Failed to build f32 input stream")?
            }
            cpal::SampleFormat::I16 => {
                device.build_input_stream(
                    config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if let Ok(mut state) = state.lock() {
                            // Convert i16 to f32
                            let samples: Vec<f32> = data.iter()
                                .map(|&s| s as f32 / i16::MAX as f32)
                                .collect();
                            state.process_samples(&samples);
                        }
                    },
                    err_fn,
                    None,
                ).context("Failed to build i16 input stream")?
            }
            cpal::SampleFormat::U16 => {
                device.build_input_stream(
                    config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if let Ok(mut state) = state.lock() {
                            // Convert u16 to f32
                            let samples: Vec<f32> = data.iter()
                                .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                                .collect();
                            state.process_samples(&samples);
                        }
                    },
                    err_fn,
                    None,
                ).context("Failed to build u16 input stream")?
            }
            format => {
                anyhow::bail!("Unsupported sample format: {:?}", format);
            }
        };

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
