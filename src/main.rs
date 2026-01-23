mod audio;
mod commands;
mod commands_config;
mod config;
mod filter;
mod state;
mod transcribe;
mod typing;
mod ui;

use anyhow::Result;
use audio::{AudioCapture, AudioEvent};
use clap::Parser;
use config::Config;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use signal_hook::consts::SIGUSR1;
use signal_hook::iterator::Signals;
use state::{AppState, AppStateContainer};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;

/// Voice-activated transcription that types directly into your focused window
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Whisper server URL (overrides prefs.toml)
    #[arg(short, long)]
    server: Option<String>,
}

fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Check if we're running in a terminal, if not, launch rxvt
    if !is_terminal() && std::env::var("THEHAND_IN_TERMINAL").is_err() {
        launch_in_terminal()?;
        return Ok(());
    }
    // Load configuration first so errors can be displayed
    let mut config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            eprintln!("\nPlease configure TheHand before running.");
            eprintln!("Edit ~/.config/thehand/prefs.toml and set:");
            eprintln!("  - whisper.binary_path (path to whisper.cpp binary)");
            eprintln!("  - whisper.model_path (path to GGML model file)");
            eprintln!("\nNote: You may need to rename config.toml to prefs.toml");
            std::process::exit(1);
        }
    };

    // Override server URL if provided via command line
    if let Some(server_url) = cli.server {
        config.whisper.server_url = server_url;
    }

    // Run the application
    if let Err(e) = run_app(config) {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

fn run_app(config: Config) -> Result<()> {
    // Suppress stderr BEFORE terminal setup to prevent ALSA messages in TUI
    unsafe {
        let devnull = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .expect("Failed to open /dev/null");
        use std::os::unix::io::AsRawFd;
        libc::dup2(devnull.as_raw_fd(), libc::STDERR_FILENO);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = AppStateContainer::new(config.ui.history_limit, config.whisper.server_url.clone());

    // Load voice commands
    match commands_config::CommandsFile::load() {
        Ok(commands_file) => {
            app.wake_word = commands_file.wake_word;
            app.voice_commands = commands_file.actions;
        }
        Err(e) => {
            app.set_error(format!("Failed to load commands: {}", e));
        }
    }

    // Load filtered phrases
    match filter::load_filtered_phrases() {
        Ok(phrases) => {
            app.filtered_phrases = phrases;
        }
        Err(e) => {
            app.set_error(format!("Failed to load filters: {}", e));
        }
    }

    // Use default audio input device (pipewire-alsa handles routing)
    app.current_device_name = "Default".to_string();
    app.current_raw_device_name = None;

    // Initialize audio capture with default device
    let mut audio = AudioCapture::new(
        config.audio.voice_threshold,
        config.audio.silence_threshold,
        config.audio.silence_duration,
        config.audio.min_speech_duration,
        config.audio.sample_rate,
        config.audio.pre_buffer_ms,
        None,
        None,
    )?;

    // Setup signal handler for SIGUSR1 (toggle mute)
    let toggle_mute_flag = Arc::new(AtomicBool::new(false));
    let toggle_mute_flag_clone = toggle_mute_flag.clone();

    std::thread::spawn(move || {
        let mut signals = Signals::new(&[SIGUSR1]).expect("Failed to create signal handler");
        for _ in signals.forever() {
            toggle_mute_flag_clone.store(true, Ordering::Relaxed);
        }
    });

    // Main loop
    let result = main_loop(&mut terminal, &mut app, &mut audio, &config, toggle_mute_flag);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn main_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppStateContainer,
    audio: &mut AudioCapture,
    config: &Config,
    toggle_mute_flag: Arc<AtomicBool>,
) -> Result<()> {
    let mut pending_transcription: Option<PathBuf> = None;

    loop {
        // Check if signal handler wants to toggle mute
        if toggle_mute_flag.load(Ordering::Relaxed) {
            toggle_mute_flag.store(false, Ordering::Relaxed);
            app.toggle_mute();
        }


        // Draw UI
        terminal.draw(|f| ui::render(f, app))?;

        // Handle keyboard events (non-blocking)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Quit on Esc, Ctrl+Q, or F12
                if matches!(key.code, KeyCode::Esc | KeyCode::F(12))
                    || (matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    app.should_quit = true;
                    break;
                }

                // Toggle mute on Ctrl+M or F1
                if matches!(key.code, KeyCode::F(1))
                    || (matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M'))
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    app.toggle_mute();
                }

                // Cancel recording on Ctrl+C or F2
                if matches!(key.code, KeyCode::F(2))
                    || (matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    if audio.is_recording() {
                        audio.cancel_recording();
                        app.set_state(AppState::Idle);
                    }
                }

                // Add last transcription to filter on F3
                if matches!(key.code, KeyCode::F(3)) {
                    if let Some(last_entry) = app.history.front() {
                        // Normalize the text (lowercase, remove trailing punctuation)
                        let normalized = last_entry.text
                            .trim()
                            .trim_end_matches(&['.', '!', '?', ','][..])
                            .to_lowercase();

                        match filter::add_filtered_phrase(&normalized) {
                            Ok(()) => {
                                // Reload filters
                                match filter::load_filtered_phrases() {
                                    Ok(phrases) => {
                                        app.filtered_phrases = phrases;
                                        app.add_to_history(format!("[System] Added to filter: {}", normalized));
                                    }
                                    Err(e) => {
                                        app.set_error(format!("Failed to reload filters: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                app.set_error(format!("Failed to add filter: {}", e));
                            }
                        }
                    } else {
                        app.set_error("No transcription to filter".to_string());
                    }
                }

                // Reload voice commands and filters on F4
                if matches!(key.code, KeyCode::F(4)) {
                    let mut reload_messages = Vec::new();

                    match commands_config::CommandsFile::load() {
                        Ok(commands_file) => {
                            app.wake_word = commands_file.wake_word;
                            app.voice_commands = commands_file.actions;
                            reload_messages.push("commands");
                        }
                        Err(e) => {
                            app.set_error(format!("Failed to reload commands: {}", e));
                        }
                    }

                    match filter::load_filtered_phrases() {
                        Ok(phrases) => {
                            app.filtered_phrases = phrases;
                            reload_messages.push("filters");
                        }
                        Err(e) => {
                            app.set_error(format!("Failed to reload filters: {}", e));
                        }
                    }

                    if !reload_messages.is_empty() {
                        app.add_to_history(format!("[System] Reloaded: {}", reload_messages.join(", ")));
                    }
                }
            }
        }

        // Handle audio events
        while let Some(event) = audio.poll_event() {
            match event {
                AudioEvent::Level(level) => {
                    if app.state != AppState::Muted {
                        app.update_audio_level(level);
                    }
                }
                AudioEvent::VoiceDetected => {
                    if app.state != AppState::Muted {
                        app.clear_error();
                    }
                }
                AudioEvent::RecordingStarted => {
                    if app.state != AppState::Muted {
                        app.set_state(AppState::Recording);
                    }
                }
                AudioEvent::RecordingStopped(path) => {
                    if app.state != AppState::Muted {
                        app.set_state(AppState::Transcribing);
                        pending_transcription = Some(path);
                    }
                }
                AudioEvent::SilenceDetected => {
                    // Just for informational purposes
                }
                AudioEvent::Error(msg) => {
                    app.set_error(msg);
                    app.set_state(AppState::Idle);
                }
            }
        }

        // Handle transcription if pending
        if let Some(audio_path) = pending_transcription.take() {
            match transcribe::transcribe(
                &config.whisper,
                &audio_path,
            ) {
                Ok(text) => {
                    // Filter out common Whisper hallucinations
                    // Split by newlines and check if all non-empty lines are filtered
                    let lines: Vec<String> = text
                        .lines()
                        .map(|line| {
                            // Trim whitespace and trailing punctuation, convert to lowercase
                            line.trim()
                                .trim_end_matches(&['.', '!', '?', ','][..])
                                .to_lowercase()
                        })
                        .filter(|line| !line.is_empty())
                        .collect();

                    // Normalize filter phrases to lowercase without trailing punctuation
                    let normalized_filters: Vec<String> = app.filtered_phrases
                        .iter()
                        .map(|phrase| {
                            phrase.trim()
                                .trim_end_matches(&['.', '!', '?', ','][..])
                                .to_lowercase()
                        })
                        .collect();

                    let is_spurious = !lines.is_empty() && lines
                        .iter()
                        .all(|line| normalized_filters.contains(line));

                    if is_spurious {
                        // Ignore spurious hallucinations, just return to idle
                        app.set_state(AppState::Idle);
                    } else {
                        // Check if this is a voice command
                        let matched_command = commands::match_command(
                            &text,
                            &app.wake_word,
                            &app.voice_commands,
                        ).cloned();

                        if let Some(command) = matched_command {
                            // Execute the voice command (but not for mute)
                            if command.action_type != "mute" {
                                app.set_state(AppState::Typing);
                            }

                            // Handle undo command specially
                            if command.action_type == "undo" {
                                if let Err(e) = commands::execute_undo(app.last_typed_length) {
                                    app.set_error(format!("Undo failed: {}", e));
                                } else {
                                    app.add_to_history(format!("[UNDO] {} chars", app.last_typed_length));
                                    app.set_last_command(format!("UNDO"));
                                    app.last_typed_length = 0; // Reset after undo
                                }
                            } else if command.action_type == "mute" {
                                // Handle mute command specially
                                // Check current state BEFORE toggling
                                let was_muted = app.state == AppState::Muted;
                                app.toggle_mute();
                                let status = if was_muted { "UNMUTED" } else { "MUTED" };
                                app.add_to_history(format!("[{}] {}", status, text));
                                app.set_last_command(status.to_string());

                                // Log to file if enabled
                                if config.ui.log_to_file {
                                    let _ = log_transcription(&config.ui.log_path, &format!("[{}] {}", status, text));
                                }
                            } else {
                                // Regular command
                                if let Err(e) = commands::execute_command(&command) {
                                    app.set_error(format!("Command failed: {}", e));
                                } else {
                                    // Add to history
                                    app.add_to_history(format!("[CMD] {}", text));
                                    app.set_last_command(text.clone());

                                    // Log to file if enabled
                                    if config.ui.log_to_file {
                                        let _ = log_transcription(&config.ui.log_path, &format!("[CMD] {}", text));
                                    }
                                }
                            }

                            // Don't override state for mute command (it handles its own state)
                            if command.action_type != "mute" {
                                app.set_state(AppState::Idle);
                            }
                        } else {
                            // Not a command, type the text normally
                            app.set_state(AppState::Typing);

                            // Type the text
                            if let Err(e) = typing::type_text(&text, config.typing.keystroke_delay) {
                                app.set_error(format!("Failed to type text: {}", e));
                            } else {
                                // Save length for undo (+1 for trailing space)
                                app.last_typed_length = text.len() + 1;

                                // Add to history
                                app.add_to_history(text.clone());

                                // Track for i3blocks display
                                app.set_last_command(text.clone());

                                // Log to file if enabled
                                if config.ui.log_to_file {
                                    let _ = log_transcription(&config.ui.log_path, &text);
                                }
                            }

                            app.set_state(AppState::Idle);
                        }
                    }
                }
                Err(e) => {
                    app.set_error(format!("Transcription failed: {}", e));
                    app.set_state(AppState::Idle);
                }
            }

            // Clean up audio file
            let _ = transcribe::cleanup_audio_file(&audio_path);
        }
    }

    Ok(())
}

fn log_transcription(log_path: &str, text: &str) -> Result<()> {
    let path = shellexpand::tilde(log_path).to_string();

    // Create parent directory if needed
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "[{}] {}", timestamp, text)?;

    Ok(())
}

fn is_terminal() -> bool {
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}

fn launch_in_terminal() -> Result<()> {
    let exe_path = std::env::current_exe()?;

    std::process::Command::new("rxvt")
        .env("THEHAND_IN_TERMINAL", "1")
        .arg("-e")
        .arg(&exe_path)
        .spawn()?;

    Ok(())
}
