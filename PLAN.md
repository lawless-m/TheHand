# TheHand - Project Plan

**Tagline:** "Talk to the hand" - Voice-activated transcription that types directly into your focused window.

## Project Overview

TheHand is a voice-activated transcription tool for Linux that:
1. Continuously monitors audio for speech
2. Auto-records when you start talking
3. Transcribes using whisper.cpp when you stop talking
4. Types the transcription directly into whatever window has focus
5. Returns to monitoring for the next utterance

**Primary Use Case:** Sending voice prompts to Claude Code, Teams messages, email snippets, etc. without touching the keyboard.

## Technology Stack

**Language:** Rust

**Core Dependencies:**
- `ratatui` - Terminal UI framework
- `crossterm` - Terminal handling and keyboard events
- `cpal` - Audio capture from microphone
- `hound` - WAV file writing
- `enigo` or X11 bindings - Keyboard simulation for typing output
- External: `whisper.cpp` binary for transcription

## User Flow

### Normal Operation
1. **Listening** - App monitors audio levels continuously
2. **Voice detected** - Automatically starts recording
3. **Silence detected** - Stops recording, transcribes via whisper.cpp
4. **Types output** - Sends transcribed text to focused window
5. **Returns to listening** - Ready for next utterance

### Mute Mode
- User presses **M** to mute (e.g., to talk to dogs without triggering)
- All voice detection disabled
- Press **M** again to resume

### Manual Controls
- **M** - Toggle mute (enable/disable voice activation)
- **C** - Cancel current recording in progress
- **Q** - Quit application

## Console UI Design

```
┌─ TheHand ─────────────────────────────────────────┐
│ Status: Listening...                    [██████░░] │ <- VU meter
│                                                     │
│ History:                                            │
│ [12:34] How do I configure the database             │
│ [12:35] Can you review this code?                   │
│ [12:36] Thanks, that worked!                        │
│                                                     │
│ Current: _                                          │
│                                                     │
│ [M]ute  [C]ancel  [Q]uit                           │
└─────────────────────────────────────────────────────┘
```

### UI States & Colors

| State         | Color  | Description                           |
|---------------|--------|---------------------------------------|
| Listening...  | Green  | Ready, monitoring for voice           |
| Recording... ● | Red    | Voice detected, actively recording    |
| Transcribing... | Yellow | Processing with whisper.cpp          |
| Sent ✓        | Green  | Successfully typed to window          |
| MUTED         | Gray   | Voice activation disabled             |

### VU Meter
- Real-time audio level visualization
- Shows current input volume as bar graph
- Helps user confirm microphone is working
- Updates continuously even when not recording

## Configuration

**Location:** `~/.config/thehand/config.toml`

### Config Structure

```toml
[whisper]
# Path to whisper.cpp binary
binary_path = "/usr/local/bin/whisper"
# Path to GGML model file (user must specify which model)
model_path = "~/.local/share/thehand/models/ggml-base.bin"

[audio]
# Sample rate for recording (16kHz is whisper standard)
sample_rate = 16000
# RMS threshold for voice detection (0.0-1.0)
voice_threshold = 0.02
# RMS threshold for silence detection (0.0-1.0)
silence_threshold = 0.01
# Duration of silence before stopping recording (seconds)
silence_duration = 2.0
# Minimum speech duration to avoid false triggers (seconds)
min_speech_duration = 0.5

[ui]
# Number of transcriptions to keep in history
history_limit = 50
# Log transcriptions to file for debugging
log_to_file = true
# Log file location
log_path = "~/.local/share/thehand/transcriptions.log"

[typing]
# Delay between keystrokes when typing output (milliseconds)
keystroke_delay = 10
```

### Config Behavior
- Create default config on first run if missing
- Display helpful error if whisper.cpp binary not found
- Display helpful error if model file doesn't exist
- Expand `~` in paths to user's home directory

## Audio Processing Pipeline

### 1. Continuous Monitoring
- Read audio in small chunks (e.g., 100ms)
- Calculate RMS (Root Mean Square) of each chunk
- Check if RMS exceeds `voice_threshold`

### 2. Voice Activity Detection (VAD)
```
State: IDLE
  ↓ (RMS > voice_threshold)
State: RECORDING
  ↓ (RMS < silence_threshold for silence_duration)
State: PROCESSING
  ↓ (transcription complete)
State: TYPING
  ↓ (output sent)
State: IDLE
```

### 3. Recording
- Buffer audio chunks in memory
- Continue until silence detected
- Save to temporary WAV file (16kHz mono)

### 4. Transcription
- Call whisper.cpp as subprocess
- Pass temporary WAV file
- Capture stdout for transcribed text
- Clean up temporary file

### 5. Output
- Type text character-by-character into focused window
- Use X11 automation (xdotool or enigo)
- Brief delay between keystrokes for reliability

## Technical Implementation Details

### Audio Capture (cpal)
```rust
// Pseudo-code structure
let stream = device.build_input_stream(
    config,
    move |data: &[f32], _: &_| {
        // Calculate RMS
        // Check voice threshold
        // Buffer if recording
        // Check silence threshold
    },
    err_fn,
)?;
```

### State Machine
```rust
enum AppState {
    Idle,           // Monitoring for voice
    Recording,      // Capturing audio
    Transcribing,   // Processing with whisper.cpp
    Typing,         // Sending output
    Muted,          // Voice detection disabled
}
```

### Whisper.cpp Integration
```rust
let output = Command::new(&config.whisper.binary_path)
    .arg("-m").arg(&config.whisper.model_path)
    .arg("-f").arg(&temp_wav_path)
    .arg("--no-timestamps")
    .output()?;

let transcription = String::from_utf8(output.stdout)?;
```

### Keyboard Simulation
- Use `enigo` crate for cross-platform typing
- Or direct X11 via `x11rb` or `xdotool` wrapper
- Type character-by-character with configurable delay
- Handle special characters properly

## Error Handling

### Critical Errors (should exit gracefully)
- Audio device not available
- whisper.cpp binary not found
- Model file not found
- Cannot access config directory

### Recoverable Errors (show in UI, continue)
- Transcription failed (empty result)
- Whisper.cpp crashed
- Cannot type to window (no focus)
- Audio glitch/dropout

### Error Display
- Show errors in UI status line
- Log errors to file if `log_to_file = true`
- Provide actionable error messages

## File Structure

```
thehand/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs           # Entry point, main loop
│   ├── config.rs         # Config loading/parsing
│   ├── audio.rs          # Audio capture and VAD
│   ├── transcribe.rs     # whisper.cpp integration
│   ├── typing.rs         # Keyboard simulation
│   ├── ui.rs             # TUI rendering
│   └── state.rs          # State machine
└── .config/
    └── thehand/
        └── config.toml.example
```

## Build & Installation

### Dependencies
```bash
# Ubuntu/Debian
sudo apt install libasound2-dev libx11-dev libxtst-dev

# Fedora
sudo dnf install alsa-lib-devel libX11-devel libXtst-devel

# Arch
sudo pacman -S alsa-lib libx11 libxtst
```

### Build
```bash
cargo build --release
```

### Installation
```bash
# Copy binary
sudo cp target/release/thehand /usr/local/bin/

# Create config directory
mkdir -p ~/.config/thehand

# Copy example config
cp .config/thehand/config.toml.example ~/.config/thehand/config.toml

# Edit config to set model path
$EDITOR ~/.config/thehand/config.toml
```

### whisper.cpp Setup
User must:
1. Clone and compile whisper.cpp
2. Download GGML model file
3. Update config with paths

Provide clear instructions in README.

## Testing Plan

### Manual Testing
1. **Voice activation** - Verify it starts recording when speaking
2. **Silence detection** - Verify it stops after silence
3. **Mute function** - Verify M key disables/enables monitoring
4. **Cancel function** - Verify C key stops recording
5. **Transcription** - Test with various phrases
6. **Typing output** - Test in different applications (terminal, browser, RDP)
7. **VU meter** - Verify audio levels display correctly
8. **Color coding** - Verify status colors change appropriately
9. **History** - Verify transcriptions appear in history
10. **Config** - Test with different settings

### Edge Cases
- Very short utterances (< 0.5s)
- Very long utterances (> 30s)
- Background noise
- Multiple speakers
- Rapid successive utterances
- Muting while recording
- No audio device
- Missing model file
- Invalid config

## Future Enhancements (Not in MVP)

- [ ] Configurable keybindings
- [ ] Multiple language support
- [ ] Hotkey for manual trigger (alongside voice activation)
- [ ] Edit transcription before sending
- [ ] Clipboard mode (copy instead of type)
- [ ] System tray icon
- [ ] Different models for different contexts
- [ ] Noise suppression/filtering
- [ ] Recording to file option
- [ ] Statistics (words typed, time saved, etc.)

## Success Criteria

The MVP is complete when:
1. ✅ Voice activation reliably detects speech
2. ✅ Silence detection reliably stops recording
3. ✅ Transcription produces accurate text via whisper.cpp
4. ✅ Text types correctly into focused window
5. ✅ Mute function works as expected
6. ✅ TUI displays status with colors and VU meter
7. ✅ History shows recent transcriptions
8. ✅ Config file is loaded and respected
9. ✅ Errors are handled gracefully
10. ✅ Works on Pop!OS (GNOME) and i3wm

## Notes for Claude Code

- User is on Pop!OS (GNOME-based) currently, planning to move to i3wm
- User already uses whisper.cpp in another project
- User sends audio over RDP currently but it's flaky - this replaces that
- Use case: batch text (prompts, Teams messages, email snippets)
- "Talk to the hand" - hence the name
- User has dogs and needs mute to avoid triggering on dog conversations 🐕

## Questions/Decisions for Implementation

1. **Audio library** - `cpal` is standard, any alternatives?
2. **Typing library** - `enigo` vs direct X11? (need Linux/X11 support)
3. **VAD algorithm** - Simple RMS threshold or more sophisticated?
4. **Buffer management** - How much audio to buffer before starting transcription?
5. **Temp file cleanup** - Delete immediately or keep for debugging?
6. **Thread model** - Audio capture on separate thread from UI?
7. **Cancellation** - Should C key work during transcription or only during recording?

Good luck! 🎤✋
