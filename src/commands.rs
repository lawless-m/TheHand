use anyhow::{Context, Result};
use enigo::{Enigo, Key, Keyboard, Settings};
use std::process::{Command, Stdio};

use crate::commands_config::VoiceCommand;

/// Execute a voice command
pub fn execute_command(command: &VoiceCommand) -> Result<()> {
    match command.action_type.as_str() {
        "keybind" => execute_keybind(&command.value),
        "shell" => execute_shell(&command.value),
        "undo" => Ok(()), // Undo is handled specially in main loop
        _ => anyhow::bail!("Unknown command type: {}", command.action_type),
    }
}

/// Send backspaces to undo typed text
pub fn execute_undo(count: usize) -> Result<()> {
    if count == 0 {
        return Ok(());
    }

    let mut enigo = Enigo::new(&Settings::default())
        .context("Failed to initialize enigo")?;

    // Send backspace key N times
    for _ in 0..count {
        enigo.key(Key::Backspace, enigo::Direction::Click)
            .context("Failed to press backspace")?;
    }

    Ok(())
}

/// Execute a keybind (e.g., "Super+e", "Ctrl+t", "Ctrl-u")
fn execute_keybind(keybind: &str) -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())
        .context("Failed to initialize enigo")?;

    // Parse the keybind string (e.g., "Super+e" -> [Super, e])
    // Accept both '+' and '-' as separators
    let parts: Vec<&str> = keybind.split(&['+', '-']).collect();

    if parts.is_empty() {
        anyhow::bail!("Empty keybind");
    }

    // Convert string parts to enigo keys
    let mut keys = Vec::new();
    for part in parts {
        let key = parse_key(part)?;
        keys.push(key);
    }

    // Press all modifier keys
    for key in &keys[..keys.len()-1] {
        enigo.key(*key, enigo::Direction::Press)
            .context("Failed to press modifier key")?;
    }

    // Press and release the final key
    if let Some(last_key) = keys.last() {
        enigo.key(*last_key, enigo::Direction::Click)
            .context("Failed to press key")?;
    }

    // Release all modifier keys in reverse order
    for key in keys[..keys.len()-1].iter().rev() {
        enigo.key(*key, enigo::Direction::Release)
            .context("Failed to release modifier key")?;
    }

    Ok(())
}

/// Parse a key string to an enigo Key
fn parse_key(s: &str) -> Result<Key> {
    let key = match s.to_lowercase().as_str() {
        // Modifiers
        "ctrl" | "control" => Key::Control,
        "alt" => Key::Alt,
        "shift" => Key::Shift,
        "super" | "meta" | "win" => Key::Meta,

        // Special keys
        "return" | "enter" => Key::Return,
        "escape" | "esc" => Key::Escape,
        "tab" => Key::Tab,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,

        // Arrow keys
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,

        // Function keys
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,

        // Single character keys
        c if c.len() == 1 => {
            let ch = c.chars().next().unwrap();
            Key::Unicode(ch)
        }

        _ => anyhow::bail!("Unknown key: {}", s),
    };

    Ok(key)
}

/// Execute a shell command
fn execute_shell(cmd: &str) -> Result<()> {
    // Split command into program and args
    let parts: Vec<&str> = cmd.split_whitespace().collect();

    if parts.is_empty() {
        anyhow::bail!("Empty shell command");
    }

    let program = parts[0];
    let args = &parts[1..];

    // Execute command in background (don't wait for it to finish)
    // Redirect stdout/stderr to prevent output in TUI
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context(format!("Failed to execute command: {}", cmd))?;

    Ok(())
}

/// Check if transcription matches a voice command pattern
/// Returns the matched command if found
pub fn match_command<'a>(
    transcription: &str,
    wake_word: &str,
    commands: &'a [VoiceCommand],
) -> Option<&'a VoiceCommand> {
    // Wake word disabled
    if wake_word.is_empty() {
        return None;
    }

    // Check if transcription starts with wake word (case-insensitive)
    let transcription_lower = transcription.trim().to_lowercase();
    let wake_word_lower = wake_word.trim().to_lowercase();

    if !transcription_lower.starts_with(&wake_word_lower) {
        return None;
    }

    // Extract the command part after wake word
    let mut command_part = transcription_lower[wake_word_lower.len()..].to_string();

    // Strip leading punctuation (comma, period, exclamation, etc.) and whitespace
    command_part = command_part.trim_start_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace()).to_string();

    // Strip trailing punctuation (Whisper often adds periods)
    command_part = command_part.trim_end_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace()).to_string();

    // Match against configured commands (exact match, case-insensitive)
    // Each command can have multiple phrase variants
    for cmd in commands {
        for phrase in &cmd.phrase {
            // Normalize configured phrase: lowercase and strip punctuation
            let mut phrase_normalized = phrase.trim().to_lowercase();
            phrase_normalized = phrase_normalized.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace()).to_string();

            if phrase_normalized == command_part {
                return Some(cmd);
            }
        }
    }

    None
}
