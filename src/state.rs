use chrono::{DateTime, Local};
use std::collections::VecDeque;
use crate::audio::DeviceInfo;
use crate::commands_config::VoiceCommand;

/// Application state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Monitoring for voice
    Idle,
    /// Capturing audio
    Recording,
    /// Processing with whisper.cpp
    Transcribing,
    /// Sending output to focused window
    Typing,
    /// Voice detection disabled
    Muted,
    /// Selecting audio input device
    DeviceSelection,
}

impl AppState {
    pub fn display_text(&self) -> &'static str {
        match self {
            AppState::Idle => "Listening...",
            AppState::Recording => "Recording... ●",
            AppState::Transcribing => "Transcribing...",
            AppState::Typing => "Sent ✓",
            AppState::Muted => "MUTED",
            AppState::DeviceSelection => "Select Audio Device",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            AppState::Idle => Color::Green,
            AppState::Recording => Color::Red,
            AppState::Transcribing => Color::Yellow,
            AppState::Typing => Color::Green,
            AppState::Muted => Color::DarkGray,
            AppState::DeviceSelection => Color::Cyan,
        }
    }
}

/// Transcription history entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Local>,
    pub text: String,
}

impl HistoryEntry {
    pub fn new(text: String) -> Self {
        Self {
            timestamp: Local::now(),
            text,
        }
    }

    pub fn format_time(&self) -> String {
        self.timestamp.format("%H:%M").to_string()
    }
}

/// Application state container
pub struct AppStateContainer {
    pub state: AppState,
    pub history: VecDeque<HistoryEntry>,
    pub audio_level: f32,
    pub error_message: Option<String>,
    pub should_quit: bool,
    pub history_limit: usize,
    pub available_devices: Vec<DeviceInfo>,
    pub selected_device_index: usize,
    pub current_device_index: Option<usize>,
    pub preview_level: f32,
    pub current_device_name: String,
    pub current_raw_device_name: Option<String>, // Raw ALSA device name for manually-added devices
    pub wake_word: String,
    pub voice_commands: Vec<VoiceCommand>,
    pub filtered_phrases: Vec<String>,
    pub last_typed_length: usize, // Length of last typed text for undo
    pub server_url: String, // Whisper server URL being used
}

impl AppStateContainer {
    pub fn new(history_limit: usize, server_url: String) -> Self {
        let mut app = Self {
            state: AppState::Idle,
            history: VecDeque::new(),
            audio_level: 0.0,
            error_message: None,
            should_quit: false,
            history_limit,
            available_devices: Vec::new(),
            selected_device_index: 0,
            current_device_index: None,
            preview_level: 0.0,
            current_device_name: String::from("Default"),
            current_raw_device_name: None,
            wake_word: String::new(),
            voice_commands: Vec::new(),
            filtered_phrases: Vec::new(),
            last_typed_length: 0,
            server_url,
        };
        // Write initial state file
        app.write_state_file();
        app
    }

    /// Add a transcription to history
    pub fn add_to_history(&mut self, text: String) {
        let entry = HistoryEntry::new(text);
        self.history.push_front(entry);

        // Limit history size
        while self.history.len() > self.history_limit {
            self.history.pop_back();
        }
    }

    /// Set the current state
    pub fn set_state(&mut self, state: AppState) {
        self.state = state;

        // Clear error message on state change
        if state != AppState::Idle {
            self.error_message = None;
        }
    }

    /// Toggle mute state
    pub fn toggle_mute(&mut self) {
        self.state = match self.state {
            AppState::Muted => AppState::Idle,
            _ => AppState::Muted,
        };
        self.write_state_file();
    }

    /// Write state to file for external monitoring (i3blocks, etc.)
    fn write_state_file(&self) {
        use std::io::Write;
        let state_str = match self.state {
            AppState::Muted => "Muted",
            _ => "Active",
        };
        if let Ok(mut file) = std::fs::File::create("/tmp/thehand_state") {
            let _ = writeln!(file, "{}", state_str);
        }
    }

    /// Set error message
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    /// Clear error message
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// Update audio level (0.0 - 1.0)
    pub fn update_audio_level(&mut self, level: f32) {
        self.audio_level = level.clamp(0.0, 1.0);
    }

    /// Set available audio devices
    pub fn set_available_devices(&mut self, devices: Vec<DeviceInfo>) {
        self.available_devices = devices;
        self.selected_device_index = 0;
    }

    /// Move selection up in device list
    pub fn select_previous_device(&mut self) {
        if !self.available_devices.is_empty() && self.selected_device_index > 0 {
            self.selected_device_index -= 1;
        }
    }

    /// Move selection down in device list
    pub fn select_next_device(&mut self) {
        if self.selected_device_index + 1 < self.available_devices.len() {
            self.selected_device_index += 1;
        }
    }

    /// Get the currently selected device index
    pub fn get_selected_device_index(&self) -> Option<usize> {
        self.available_devices
            .get(self.selected_device_index)
            .map(|d| d.index)
    }

    /// Update preview audio level
    pub fn update_preview_level(&mut self, level: f32) {
        self.preview_level = level.clamp(0.0, 1.0);
    }
}
