use chrono::{DateTime, Local};
use std::collections::VecDeque;

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
}

impl AppState {
    pub fn display_text(&self) -> &'static str {
        match self {
            AppState::Idle => "Listening...",
            AppState::Recording => "Recording... ●",
            AppState::Transcribing => "Transcribing...",
            AppState::Typing => "Sent ✓",
            AppState::Muted => "MUTED",
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
    pub current_text: String,
    pub audio_level: f32,
    pub error_message: Option<String>,
    pub should_quit: bool,
    pub history_limit: usize,
}

impl AppStateContainer {
    pub fn new(history_limit: usize) -> Self {
        Self {
            state: AppState::Idle,
            history: VecDeque::new(),
            current_text: String::new(),
            audio_level: 0.0,
            error_message: None,
            should_quit: false,
            history_limit,
        }
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

    /// Set current text being processed
    pub fn set_current_text(&mut self, text: String) {
        self.current_text = text;
    }

    /// Clear current text
    pub fn clear_current_text(&mut self) {
        self.current_text.clear();
    }
}
