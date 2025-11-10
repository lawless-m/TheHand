# TheHand 🎤✋

**"Talk to the hand"** - Voice-activated transcription that types directly into your focused window.

TheHand is a Linux voice-activated transcription tool that continuously monitors audio for speech, auto-records when you start talking, transcribes using whisper.cpp when you stop talking, and types the transcription directly into whatever window has focus.

## Features

- 🎙️ **Voice-activated recording** - Automatically starts recording when you speak
- 🔇 **Silence detection** - Stops recording automatically after you stop talking
- 🤖 **Local transcription** - Uses whisper.cpp for offline, private transcription
- ⌨️ **Direct typing** - Types transcription into any focused window
- 📊 **Real-time VU meter** - Visual feedback of audio levels
- 📝 **Transcription history** - See your recent transcriptions
- 🔕 **Mute mode** - Disable voice activation when needed
- 🎨 **Color-coded status** - Clear visual indication of current state
- ⚙️ **Configurable** - Adjust thresholds, delays, and paths

## Use Cases

- Send voice prompts to Claude Code
- Dictate Teams messages or Slack messages
- Write email snippets
- Any scenario where you want to speak instead of type

## Demo

```
┌─ TheHand ─────────────────────────────────────────┐
│ Status: Listening...                    [██████░░] │
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

## Installation

### Prerequisites

#### System Dependencies

**Ubuntu/Debian:**
```bash
sudo apt install libasound2-dev libx11-dev libxtst-dev build-essential
```

**Fedora:**
```bash
sudo dnf install alsa-lib-devel libX11-devel libXtst-devel gcc
```

**Arch Linux:**
```bash
sudo pacman -S alsa-lib libx11 libxtst base-devel
```

#### Rust

Install Rust if you haven't already:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### whisper.cpp

1. Clone and build whisper.cpp:
```bash
git clone https://github.com/ggerganov/whisper.cpp.git
cd whisper.cpp
make

# Copy binary to system path
sudo cp main /usr/local/bin/whisper
```

2. Download a GGML model file:
```bash
# Create model directory
mkdir -p ~/.local/share/thehand/models

# Download base model (recommended for good balance)
bash ./models/download-ggml-model.sh base

# Copy model to TheHand directory
cp models/ggml-base.bin ~/.local/share/thehand/models/
```

Available models (larger = more accurate but slower):
- `tiny` - Fastest, least accurate
- `base` - Good balance (recommended)
- `small` - Better accuracy
- `medium` - Even better accuracy
- `large` - Best accuracy, slowest

### Building TheHand

```bash
# Clone the repository
git clone https://github.com/yourusername/thehand.git
cd thehand

# Build release version
cargo build --release

# Copy binary to system path
sudo cp target/release/thehand /usr/local/bin/
```

### Configuration

1. Create config directory:
```bash
mkdir -p ~/.config/thehand
```

2. Copy example config:
```bash
cp .config/thehand/config.toml.example ~/.config/thehand/config.toml
```

3. Edit the config file:
```bash
nano ~/.config/thehand/config.toml
```

At minimum, verify/update these paths:
- `whisper.binary_path` - Path to whisper.cpp binary (default: `/usr/local/bin/whisper`)
- `whisper.model_path` - Path to GGML model file (default: `~/.local/share/thehand/models/ggml-base.bin`)

## Usage

### Starting TheHand

Simply run:
```bash
thehand
```

The application will start in listening mode, monitoring for speech.

### Controls

- **M** - Toggle mute (disable/enable voice activation)
- **C** - Cancel current recording
- **Q** - Quit application

### Workflow

1. **Listening** - TheHand monitors audio levels continuously
2. **Start speaking** - When voice is detected, recording starts automatically
3. **Stop speaking** - After 2 seconds of silence, recording stops
4. **Transcribing** - Audio is sent to whisper.cpp for transcription
5. **Typing** - Transcribed text is typed into the focused window
6. **Ready** - TheHand returns to listening mode

### Tips

- **Click into target window** before speaking (e.g., terminal, browser, chat app)
- **Use mute mode** when you need to talk without triggering (e.g., talking to pets)
- **Check VU meter** to confirm microphone is working
- **Adjust thresholds** in config if it's too sensitive or not sensitive enough

## Configuration Reference

### Audio Settings

```toml
[audio]
sample_rate = 16000           # 16kHz is whisper standard
voice_threshold = 0.02        # Increase if too sensitive
silence_threshold = 0.01      # Must be < voice_threshold
silence_duration = 2.0        # Seconds of silence before stopping
min_speech_duration = 0.5     # Minimum length to process
```

**Tuning Tips:**
- If it triggers on background noise: Increase `voice_threshold`
- If it doesn't detect your voice: Decrease `voice_threshold`
- If it cuts you off mid-sentence: Increase `silence_duration`
- If it waits too long after you stop: Decrease `silence_duration`

### Typing Settings

```toml
[typing]
keystroke_delay = 10          # Milliseconds between keystrokes
```

- Increase if characters are being dropped
- Decrease for faster typing

### UI Settings

```toml
[ui]
history_limit = 50            # Number of transcriptions to keep
log_to_file = true            # Save transcriptions to log file
log_path = "~/.local/share/thehand/transcriptions.log"
```

## Troubleshooting

### "Whisper binary not found"

Make sure whisper.cpp is installed and the path in config is correct:
```bash
which whisper
# Update whisper.binary_path in config
```

### "Model file not found"

Download a model file and update the path in config:
```bash
cd whisper.cpp
bash ./models/download-ggml-model.sh base
cp models/ggml-base.bin ~/.local/share/thehand/models/
```

### "No input device available"

Check your microphone:
```bash
arecord -l
```

Make sure your microphone is not muted in system settings.

### Recording triggers too easily

Increase `voice_threshold` in config (e.g., from 0.02 to 0.03).

### Recording doesn't trigger

- Check VU meter shows audio levels
- Decrease `voice_threshold` in config
- Make sure you're not in mute mode
- Check microphone volume in system settings

### Text types in wrong window

Make sure to click into the target window before speaking, so it has focus.

### Transcription is inaccurate

- Try a larger model (e.g., `small` or `medium` instead of `base`)
- Speak more clearly and avoid background noise
- Check if your accent/language is well-supported by Whisper

## Tested Environments

- Pop!_OS (GNOME)
- i3wm
- Other Linux distros with X11 should work

**Note:** Currently only X11 is supported. Wayland support may require additional configuration.

## Development

### Project Structure

```
thehand/
├── src/
│   ├── main.rs         # Entry point and main loop
│   ├── config.rs       # Configuration loading
│   ├── audio.rs        # Audio capture and VAD
│   ├── transcribe.rs   # whisper.cpp integration
│   ├── typing.rs       # Keyboard simulation
│   ├── ui.rs           # TUI rendering
│   └── state.rs        # State machine
├── Cargo.toml
├── README.md
└── .config/
    └── thehand/
        └── config.toml.example
```

### Building for Development

```bash
cargo build
cargo run
```

### Running Tests

```bash
cargo test
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) - Fast inference of OpenAI's Whisper model
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI library
- [cpal](https://github.com/RustAudio/cpal) - Cross-platform audio library

## FAQ

**Q: Does this send my audio to the cloud?**
A: No! Everything runs locally using whisper.cpp. Your audio never leaves your machine.

**Q: Can I use this on Wayland?**
A: X11 is currently required for keyboard simulation. Wayland support may be possible with additional work.

**Q: Can I use a different transcription engine?**
A: Currently only whisper.cpp is supported, but the architecture is modular enough to add alternatives.

**Q: Why is it called TheHand?**
A: "Talk to the hand" - it's a voice tool! 🎤✋

## Support

For issues, questions, or feature requests, please open an issue on GitHub.
