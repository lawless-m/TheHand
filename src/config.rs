use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub whisper: WhisperConfig,
    pub audio: AudioConfig,
    pub ui: UiConfig,
    pub typing: TypingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperConfig {
    /// Path to whisper.cpp binary
    pub binary_path: String,
    /// Path to GGML model file
    pub model_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Sample rate for recording (16kHz is whisper standard)
    pub sample_rate: u32,
    /// RMS threshold for voice detection (0.0-1.0)
    pub voice_threshold: f32,
    /// RMS threshold for silence detection (0.0-1.0)
    pub silence_threshold: f32,
    /// Duration of silence before stopping recording (seconds)
    pub silence_duration: f32,
    /// Minimum speech duration to avoid false triggers (seconds)
    pub min_speech_duration: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Number of transcriptions to keep in history
    pub history_limit: usize,
    /// Log transcriptions to file for debugging
    pub log_to_file: bool,
    /// Log file location
    pub log_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingConfig {
    /// Delay between keystrokes when typing output (milliseconds)
    pub keystroke_delay: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            whisper: WhisperConfig {
                binary_path: "/usr/local/bin/whisper".to_string(),
                model_path: "~/.local/share/thehand/models/ggml-base.bin".to_string(),
            },
            audio: AudioConfig {
                sample_rate: 16000,
                voice_threshold: 0.02,
                silence_threshold: 0.01,
                silence_duration: 2.0,
                min_speech_duration: 0.5,
            },
            ui: UiConfig {
                history_limit: 50,
                log_to_file: true,
                log_path: "~/.local/share/thehand/transcriptions.log".to_string(),
            },
            typing: TypingConfig {
                keystroke_delay: 10,
            },
        }
    }
}

impl Config {
    /// Load configuration from default path or create default config
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            eprintln!("Config file not found at {:?}", config_path);
            eprintln!("Creating default configuration...");
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&config_path)
            .context(format!("Failed to read config file at {:?}", config_path))?;

        let mut config: Config = toml::from_str(&content)
            .context("Failed to parse config file. Please check TOML syntax.")?;

        // Expand ~ in paths
        config.whisper.binary_path = Self::expand_path(&config.whisper.binary_path);
        config.whisper.model_path = Self::expand_path(&config.whisper.model_path);
        config.ui.log_path = Self::expand_path(&config.ui.log_path);

        config.validate()?;

        Ok(config)
    }

    /// Save configuration to default path
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .context(format!("Failed to create config directory at {:?}", parent))?;
        }

        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config to TOML")?;

        fs::write(&config_path, content)
            .context(format!("Failed to write config file to {:?}", config_path))?;

        println!("Configuration saved to {:?}", config_path);
        Ok(())
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Check if whisper binary exists
        let whisper_path = PathBuf::from(&self.whisper.binary_path);
        if !whisper_path.exists() {
            anyhow::bail!(
                "Whisper binary not found at {:?}\n\
                Please install whisper.cpp and update the binary_path in your config.",
                whisper_path
            );
        }

        // Check if model file exists
        let model_path = PathBuf::from(&self.whisper.model_path);
        if !model_path.exists() {
            anyhow::bail!(
                "Model file not found at {:?}\n\
                Please download a GGML model file and update the model_path in your config.",
                model_path
            );
        }

        // Validate thresholds
        if self.audio.voice_threshold <= 0.0 || self.audio.voice_threshold > 1.0 {
            anyhow::bail!("voice_threshold must be between 0.0 and 1.0");
        }
        if self.audio.silence_threshold <= 0.0 || self.audio.silence_threshold > 1.0 {
            anyhow::bail!("silence_threshold must be between 0.0 and 1.0");
        }
        if self.audio.silence_threshold >= self.audio.voice_threshold {
            anyhow::bail!("silence_threshold must be less than voice_threshold");
        }

        Ok(())
    }

    /// Get the default config path
    fn config_path() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join(".config/thehand/config.toml"))
    }

    /// Expand ~ to home directory
    fn expand_path(path: &str) -> String {
        shellexpand::tilde(path).to_string()
    }
}
