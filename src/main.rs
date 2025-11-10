mod audio;
mod config;
mod state;
mod transcribe;
mod typing;
mod ui;

use anyhow::Result;
use audio::{AudioCapture, AudioEvent};
use config::Config;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
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
    // Load configuration
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            eprintln!("\nPlease configure TheHand before running.");
            eprintln!("Edit ~/.config/thehand/config.toml and set:");
            eprintln!("  - whisper.binary_path (path to whisper.cpp binary)");
            eprintln!("  - whisper.model_path (path to GGML model file)");
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
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = AppStateContainer::new(config.ui.history_limit);

    // Initialize audio capture
    let audio = AudioCapture::new(
        config.audio.voice_threshold,
        config.audio.silence_threshold,
        config.audio.silence_duration,
        config.audio.min_speech_duration,
        config.audio.sample_rate,
    )?;

    // Main loop
    let result = main_loop(&mut terminal, &mut app, &audio, &config);

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
    audio: &AudioCapture,
    config: &Config,
) -> Result<()> {
    let mut pending_transcription: Option<PathBuf> = None;

    loop {
        // Draw UI
        terminal.draw(|f| ui::render(f, app))?;

        // Handle keyboard events (non-blocking)
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        app.should_quit = true;
                        break;
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        app.toggle_mute();
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        if audio.is_recording() {
                            audio.cancel_recording();
                            app.set_state(AppState::Idle);
                            app.clear_current_text();
                        }
                    }
                    _ => {}
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
                        app.clear_current_text();
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
                &config.whisper.binary_path,
                &config.whisper.model_path,
                &audio_path,
            ) {
                Ok(text) => {
                    app.set_current_text(text.clone());
                    app.set_state(AppState::Typing);

                    // Type the text
                    if let Err(e) = typing::type_text(&text, config.typing.keystroke_delay) {
                        app.set_error(format!("Failed to type text: {}", e));
                    } else {
                        // Add to history
                        app.add_to_history(text.clone());

                        // Log to file if enabled
                        if config.ui.log_to_file {
                            let _ = log_transcription(&config.ui.log_path, &text);
                        }
                    }

                    app.set_state(AppState::Idle);
                    app.clear_current_text();
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
