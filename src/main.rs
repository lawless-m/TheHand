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
use config::Config;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use state::{AppState, AppStateContainer};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<()> {
    // Check if we're running in a terminal, if not, launch rxvt
    if !is_terminal() && std::env::var("THEHAND_IN_TERMINAL").is_err() {
        launch_in_terminal()?;
        return Ok(());
    }
    // Load configuration first so errors can be displayed
    let config = match Config::load() {
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
    let mut app = AppStateContainer::new(config.ui.history_limit);

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

    // Skip device enumeration on startup to avoid headphone noise
    // Just open the Jabra directly
    let jabra_device_name = "plughw:CARD=LINK,DEV=0".to_string();
    app.current_device_name = "Jabra EVOLVE LINK".to_string();
    app.current_raw_device_name = Some(jabra_device_name.clone());

    // Initialize audio capture directly with Jabra
    let mut audio = AudioCapture::new(
        config.audio.voice_threshold,
        config.audio.silence_threshold,
        config.audio.silence_duration,
        config.audio.min_speech_duration,
        config.audio.sample_rate,
        None,
        Some(jabra_device_name),
    )?;

    // Main loop
    let result = main_loop(&mut terminal, &mut app, &mut audio, &config);

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
) -> Result<()> {
    let mut pending_transcription: Option<PathBuf> = None;
    let mut device_preview: Option<audio::DevicePreview> = None;
    let mut last_preview_device: Option<usize> = None;

    loop {
        // Update device preview level if in device selection mode
        if app.state == AppState::DeviceSelection {
            let current_device = app.available_devices.get(app.selected_device_index).cloned();

            // Create or recreate preview if device changed
            let current_device_index = current_device.as_ref().map(|d| d.index);
            if current_device_index != last_preview_device {
                device_preview = current_device.and_then(|device| {
                    let idx = if device.index == usize::MAX { None } else { Some(device.index) };
                    audio::DevicePreview::new(idx, device.device_name, config.audio.sample_rate).ok()
                });
                last_preview_device = current_device_index;
            }

            // Update preview level
            if let Some(ref preview) = device_preview {
                app.update_preview_level(preview.get_level());
            }
        } else {
            // Clean up preview when not in device selection mode
            if device_preview.is_some() {
                device_preview = None;
                last_preview_device = None;
            }
        }

        // Draw UI
        terminal.draw(|f| ui::render(f, app))?;

        // Handle keyboard events (non-blocking)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Handle device selection mode
                if app.state == AppState::DeviceSelection {
                    match key.code {
                        KeyCode::Up => {
                            app.select_previous_device();
                        }
                        KeyCode::Down => {
                            app.select_next_device();
                        }
                        KeyCode::Enter => {
                            // Switch to selected device
                            if let Some(selected_device) = app.available_devices.get(app.selected_device_index).cloned() {
                                match AudioCapture::new(
                                    config.audio.voice_threshold,
                                    config.audio.silence_threshold,
                                    config.audio.silence_duration,
                                    config.audio.min_speech_duration,
                                    config.audio.sample_rate,
                                    None, // Always use device_name, not index
                                    selected_device.device_name.clone(),
                                ) {
                                    Ok(new_audio) => {
                                        *audio = new_audio;
                                        app.current_device_index = Some(selected_device.index);
                                        app.current_device_name = selected_device.name.clone();
                                        app.current_raw_device_name = selected_device.device_name.clone();
                                        app.set_state(AppState::Idle);
                                    }
                                    Err(e) => {
                                        app.set_error(format!("Failed to switch device: {}", e));
                                        app.set_state(AppState::Idle);
                                    }
                                }
                            }
                        }
                        KeyCode::Esc | KeyCode::F(3) => {
                            // Cancel device selection
                            app.set_state(AppState::Idle);
                        }
                        KeyCode::F(12) => {
                            // Allow quit even in device selection mode
                            app.should_quit = true;
                            break;
                        }
                        _ => {}
                    }
                } else {
                    // Normal mode keybindings
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

                    // Toggle device selection on F3
                    if matches!(key.code, KeyCode::F(3)) {
                        match audio::list_input_devices() {
                            Ok(devices) => {
                                app.set_available_devices(devices);
                                app.set_state(AppState::DeviceSelection);
                            }
                            Err(e) => {
                                app.set_error(format!("Failed to list devices: {}", e));
                            }
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
        }

        // Handle audio events (skip if in device selection mode)
        if app.state != AppState::DeviceSelection {
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
        }

        // Handle transcription if pending
        if let Some(audio_path) = pending_transcription.take() {
            match transcribe::transcribe(
                &config.whisper,
                &audio_path,
            ) {
                Ok(text) => {
                    // Filter out common Whisper hallucinations
                    let trimmed = text.trim();
                    let is_spurious = app.filtered_phrases
                        .iter()
                        .any(|phrase| phrase == trimmed);

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
                            // Execute the voice command
                            app.set_state(AppState::Typing);

                            // Handle undo command specially
                            if command.action_type == "undo" {
                                if let Err(e) = commands::execute_undo(app.last_typed_length) {
                                    app.set_error(format!("Undo failed: {}", e));
                                } else {
                                    app.add_to_history(format!("[UNDO] {} chars", app.last_typed_length));
                                    app.last_typed_length = 0; // Reset after undo
                                }
                            } else {
                                // Regular command
                                if let Err(e) = commands::execute_command(&command) {
                                    app.set_error(format!("Command failed: {}", e));
                                } else {
                                    // Add to history
                                    app.add_to_history(format!("[CMD] {}", text));

                                    // Log to file if enabled
                                    if config.ui.log_to_file {
                                        let _ = log_transcription(&config.ui.log_path, &format!("[CMD] {}", text));
                                    }
                                }
                            }

                            app.set_state(AppState::Idle);
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
