# TheHand 🎤✋

**"Talk to the hand"** - Voice-activated transcription that types directly into your focused window.

TheHand is a Linux voice-activated transcription tool that continuously monitors audio for speech, auto-records when you start talking, transcribes using whisper.cpp when you stop talking, and types the transcription directly into whatever window has focus.

## Features

- 🎙️ **Voice-activated recording** - Automatically starts recording when you speak
- 🔇 **Silence detection** - Stops recording automatically after you stop talking
- 🤖 **Local transcription** - Uses whisper.cpp for offline, private transcription
- 🚀 **GPU-accelerated server mode** - Optional whisper server for 10x faster transcription
- ⌨️ **Direct typing** - Types transcription into any focused window
- 🎮 **Voice commands** - Execute keybinds, shell commands, and more via voice
- 📊 **Real-time VU meter** - Visual feedback of audio levels
- 📝 **Transcription history** - See your recent transcriptions
- 🔕 **Mute mode** - Disable voice activation when needed
- 🎨 **Color-coded status** - Clear visual indication of current state
- 🔧 **i3blocks integration** - Status bar indicator with click-to-mute
- ⚙️ **Configurable** - Adjust thresholds, delays, and paths

## Use Cases

- Send voice prompts to Claude Code
- Dictate Teams messages or Slack messages
- Write email snippets
- Execute keyboard shortcuts and shell commands by voice
- Quick text expansion (email addresses, common phrases)
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
│ [F1] Mute  [F2] Cancel  [F12] Quit                 │
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

TheHand supports two modes:
- **Server mode** (recommended): 10x faster with GPU acceleration, keeps model in memory
- **CLI mode** (fallback): Slower, loads model for each transcription

##### Option 1: Whisper Server (Recommended)

For GPU-accelerated, high-performance transcription, set up a whisper.cpp server. See the detailed guide:
- **[WHISPER_SERVER_SETUP.md](WHISPER_SERVER_SETUP.md)** - Complete server installation guide

Quick summary:
```bash
# Clone and build with CUDA support
git clone https://github.com/ggerganov/whisper.cpp.git
cd whisper.cpp
mkdir build && cd build
cmake .. -DWHISPER_CUDA=ON
cmake --build . --config Release

# Download model
cd ~/whisper.cpp/models
bash download-ggml-model.sh large-v3

# Start server
~/whisper.cpp/build/bin/whisper-server \
  -m ~/whisper.cpp/models/ggml-large-v3.bin \
  --port 8080
```

##### Option 2: CLI Mode (Fallback)

For basic usage without GPU:
```bash
git clone https://github.com/ggerganov/whisper.cpp.git
cd whisper.cpp
make

# Copy binary to system path
sudo cp main /usr/local/bin/whisper

# Download base model (recommended for CPU)
mkdir -p ~/.local/share/thehand/models
bash ./models/download-ggml-model.sh base
cp models/ggml-base.bin ~/.local/share/thehand/models/
```

Available models (larger = more accurate but slower):
- `tiny` - Fastest, least accurate (~75MB)
- `base` - Good balance for CPU (~150MB)
- `small` - Better accuracy (~500MB)
- `medium` - Even better accuracy (~1.5GB)
- `large-v3` - Best accuracy, requires GPU (~3GB)

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
cp .config/thehand/prefs.toml.example ~/.config/thehand/prefs.toml
```

3. Edit the config file:
```bash
nano ~/.config/thehand/prefs.toml
```

Key settings:
- `whisper.server_url` - Whisper server URL (default: `http://localhost:8080`)
  - TheHand will auto-detect and use if server is running
  - Falls back to CLI mode if server unavailable
- `whisper.binary_path` - Path to whisper.cpp binary for CLI fallback (default: `/usr/local/bin/whisper`)
- `whisper.model_path` - Path to GGML model file for CLI fallback (default: `~/.local/share/thehand/models/ggml-base.bin`)

## Usage

### Starting TheHand

Simply run:
```bash
thehand
```

Or specify a custom whisper server:
```bash
thehand --server http://SERVER_IP:8080
```

The application will start in listening mode, monitoring for speech.

### Controls

TheHand supports both function keys and Ctrl shortcuts:

- **F1** or **Ctrl+M** - Toggle mute (disable/enable voice activation)
- **F2** or **Ctrl+C** - Cancel current recording
- **F12**, **Esc**, or **Ctrl+Q** - Quit application

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

### Voice Commands

TheHand can execute custom voice commands beyond just typing text. Configure commands in `~/.config/thehand/commands.toml`.

#### Command Types

- **keybind** - Press keyboard shortcuts (e.g., `Super+e`, `Ctrl+t`)
- **shell** - Execute shell commands (e.g., open apps, run scripts)
- **text** - Type specific text strings
- **mouse** - Perform mouse actions
- **undo** - Delete the last transcription

#### Example Configuration

Create `~/.config/thehand/commands.toml`:

```toml
# Optional wake word to enter command mode
wake_word = ""

# Voice commands
[[actions]]
phrase = ["mute", "voice mute"]
type = "keybind"
value = "F1"

[[actions]]
phrase = ["open terminal", "launch terminal"]
type = "shell"
value = "gnome-terminal"

[[actions]]
phrase = ["undo that", "delete that", "scratch that"]
type = "undo"
value = ""

[[actions]]
phrase = "my email"
type = "text"
value = "user@example.com"
```

#### How It Works

1. Speak normally - TheHand transcribes and types the text
2. If transcription matches a command phrase, the action executes instead
3. Commands are case-insensitive and support multiple phrase variations

For example, saying "open terminal" will launch your terminal instead of typing those words.

### i3blocks Integration

TheHand includes a status bar indicator for i3blocks that shows mute state and allows click-to-toggle.

See **[contrib/i3blocks/README.md](contrib/i3blocks/README.md)** for setup instructions.

Features:
- **MIC✓** (green) - TheHand is active and listening
- **MIC✗** (red) - TheHand is muted
- **MIC-** (gray) - TheHand is not running
- Click the indicator to toggle mute on/off

## Configuration Reference

All settings are in `~/.config/thehand/prefs.toml`.

### Whisper Settings

```toml
[whisper]
# Whisper server URL (preferred for speed)
server_url = "http://localhost:8080"

# Binary path for CLI fallback mode
binary_path = "/usr/local/bin/whisper"

# Model path for CLI fallback mode
model_path = "~/.local/share/thehand/models/ggml-base.bin"
```

**Note:** TheHand auto-detects if the server is available. If server is running, it uses server mode (much faster). If not, it falls back to CLI mode.

You can override the server URL via command line:
```bash
thehand --server http://REMOTE_IP:8080
```

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

### Transcription is slow

**Using CLI mode?** Switch to server mode for 10x faster transcription:
1. Set up a whisper server (see [WHISPER_SERVER_SETUP.md](WHISPER_SERVER_SETUP.md))
2. Start the server: `~/whisper.cpp/build/bin/whisper-server -m MODEL.bin --port 8080`
3. TheHand will auto-detect and use it

**Using server mode but still slow?**
- Check server is using GPU: `nvidia-smi` should show whisper-server process
- Try a smaller model (`medium` instead of `large-v3`)
- Check network latency if server is remote

### Can't connect to whisper server

```bash
# Test if server is running
curl http://localhost:8080/

# Check server logs
journalctl -u whisper-server -f

# Try CLI fallback
thehand  # Should auto-fallback if server unavailable
```

## Tested Environments

- Debian with i3wm
- Other Linux distros with X11 should work

**Note:** Currently only X11 is supported. Wayland support may require additional configuration.

## Development

### Project Structure

```
thehand/
├── src/
│   ├── main.rs              # Entry point and main loop
│   ├── config.rs            # Configuration loading (prefs.toml)
│   ├── commands_config.rs   # Voice commands config (commands.toml)
│   ├── commands.rs          # Voice command execution
│   ├── audio.rs             # Audio capture and VAD
│   ├── transcribe.rs        # whisper.cpp integration (server + CLI)
│   ├── typing.rs            # Keyboard simulation
│   ├── ui.rs                # TUI rendering
│   └── state.rs             # State machine
├── contrib/
│   └── i3blocks/
│       ├── thehand-status   # Status bar script
│       └── README.md        # i3blocks setup guide
├── Cargo.toml
├── README.md
├── WHISPER_SERVER_SETUP.md  # Server installation guide
└── .config/
    └── thehand/
        └── prefs.toml.example
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
A: No! Everything runs locally using whisper.cpp. Even in server mode, the server is your own local/private whisper.cpp server - no cloud services involved. Your audio never leaves your control.

**Q: What's the difference between server mode and CLI mode?**
A:
- **Server mode**: Keeps the Whisper model loaded in GPU memory. Transcription is ~10x faster (near real-time). Requires setting up whisper-server.
- **CLI mode**: Loads the model for each transcription. Slower but simpler setup, no GPU required.

TheHand automatically uses server mode if available, otherwise falls back to CLI mode.

**Q: Can I run the whisper server on a different machine?**
A: Yes! Use `thehand --server http://REMOTE_IP:8080` to connect to a remote server. Your audio will be sent to that machine for transcription. Use SSH tunneling for security.

**Q: Can I use this on Wayland?**
A: X11 is currently required for keyboard simulation. Wayland support may be possible with additional work.

**Q: Can I use a different transcription engine?**
A: Currently only whisper.cpp is supported, but the architecture is modular enough to add alternatives.

**Q: Why is it called TheHand?**
A: "Talk to the hand" - it's a voice tool! 🎤✋

## Support

For issues, questions, or feature requests, please open an issue on GitHub.
