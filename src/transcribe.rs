use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Transcribe audio file using whisper.cpp
pub fn transcribe(
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
