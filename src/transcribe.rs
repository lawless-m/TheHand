use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use crate::config::WhisperConfig;

// Track whether we've checked the server availability
static SERVER_AVAILABLE: Mutex<Option<bool>> = Mutex::new(None);

/// Check if whisper server is available
fn check_server(server_url: &str) -> bool {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
        .unwrap();

    // Try to GET the server root - whisper-server responds to this
    client.get(server_url).send().is_ok()
}

/// Transcribe audio file using whisper (auto-detect server or fallback to CLI)
pub fn transcribe(
    config: &WhisperConfig,
    audio_file: &Path,
) -> Result<String> {
    // Check if server is available (cache the result)
    let use_server = {
        let mut cached = SERVER_AVAILABLE.lock().unwrap();
        match *cached {
            Some(available) => available,
            None => {
                let available = check_server(&config.server_url);
                *cached = Some(available);
                available
            }
        }
    };

    if use_server {
        // Try server first
        match transcribe_server(&config.server_url, audio_file) {
            Ok(text) => Ok(text),
            Err(e) => {
                // Server failed, mark as unavailable and fall back to CLI
                *SERVER_AVAILABLE.lock().unwrap() = Some(false);
                eprintln!("Warning: Server failed ({}), falling back to CLI mode", e);
                transcribe_cli(&config.binary_path, &config.model_path, audio_file)
            }
        }
    } else {
        // Server not available, use CLI
        transcribe_cli(&config.binary_path, &config.model_path, audio_file)
    }
}

/// Transcribe using whisper-server (HTTP mode)
fn transcribe_server(server_url: &str, audio_file: &Path) -> Result<String> {
    let client = reqwest::blocking::Client::new();

    let form = reqwest::blocking::multipart::Form::new()
        .file("file", audio_file)
        .context("Failed to read audio file for upload")?;

    let response = client
        .post(format!("{}/inference", server_url))
        .multipart(form)
        .send()
        .context("Failed to send request to whisper server")?;

    if !response.status().is_success() {
        anyhow::bail!("Whisper server returned error: {}", response.status());
    }

    let json: serde_json::Value = response.json()
        .context("Failed to parse whisper server response")?;

    let transcription = json["text"]
        .as_str()
        .context("Whisper server response missing 'text' field")?
        .trim()
        .to_string();

    if transcription.is_empty() {
        anyhow::bail!("Whisper server returned empty transcription");
    }

    Ok(transcription)
}

/// Transcribe using whisper CLI binary
fn transcribe_cli(
    whisper_binary: &str,
    model_path: &str,
    audio_file: &Path,
) -> Result<String> {
    let output = Command::new(whisper_binary)
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(audio_file)
        .arg("--no-timestamps")
        .arg("--output-txt")
        .arg("--output-file")
        .arg("-") // Output to stdout
        .output()
        .context(format!("Failed to execute whisper binary at {}", whisper_binary))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Whisper.cpp failed: {}", stderr);
    }

    let transcription = String::from_utf8(output.stdout)
        .context("Failed to parse whisper output as UTF-8")?;

    // Clean up the transcription
    let cleaned = transcription
        .trim()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.is_empty() {
        anyhow::bail!("Whisper returned empty transcription");
    }

    Ok(cleaned)
}

/// Clean up temporary audio file
pub fn cleanup_audio_file(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)
            .context(format!("Failed to remove temporary file {:?}", path))?;
    }
    Ok(())
}
