# Audio Setup for TheHand with AB13X

This document describes the audio configuration needed to run TheHand with the AB13X USB Audio device on Linux with PulseAudio.

## Problem

TheHand needs direct ALSA access to the AB13X microphone, but PulseAudio auto-detects USB audio devices and can block ALSA access. Additionally, browser audio (YouTube, etc.) needs to work through the AB13X output.

## Solution Overview

- **TheHand**: Uses direct ALSA access to AB13X microphone (`hw:CARD=Audio,DEV=0`)
- **Browser/YouTube**: Uses PulseAudio for AB13X audio output
- **PulseAudio**: Configured to only use AB13X for OUTPUT, leaving microphone free for TheHand

## Configuration Steps

### 1. Create PulseAudio Configuration

Create `~/.config/pulse/default.pa`:

```bash
#!/usr/bin/pulseaudio -nF

# Load system-wide default configuration
.include /etc/pulse/default.pa

# Unload suspend-on-idle module to prevent audio clipping
.nofail
unload-module module-suspend-on-idle

# Load AB13X USB Audio device as sink ONLY (output)
# TheHand uses direct ALSA access for the microphone input
load-module module-alsa-sink device=hw:0,0

# Set AB13X as default sink
set-default-sink alsa_output.hw_0_0
```

**Why disable suspend-on-idle?**
Without this, PulseAudio suspends the audio device when idle. When TheHand plays back transcriptions, the device takes time to wake up, clipping the first word.

### 2. Create udev Rule to Block PulseAudio from Microphone

Create `/etc/udev/rules.d/90-pulseaudio-ab13x.rules`:

```bash
# Ignore AB13X input device for PulseAudio
# TheHand uses direct ALSA access to the microphone
SUBSYSTEM=="sound", ATTRS{idVendor}=="001f", ATTRS{idProduct}=="0b21", ENV{PULSE_IGNORE}="1"
```

**Note:** The vendor/product IDs are specific to the AB13X. Find your device IDs with:
```bash
lsusb | grep -i AB13X
```

Reload udev rules:
```bash
sudo udevadm control --reload-rules
```

### 3. Restart PulseAudio

```bash
pulseaudio --kill
pulseaudio --start
```

### 4. Verify Setup

Check that AB13X output is loaded but input is NOT:
```bash
# Should show alsa_output.hw_0_0
pactl list sinks short

# Should NOT show any AB13X input source
pactl list sources short | grep -i AB13X

# AB13X microphone should be free for TheHand
fuser /dev/snd/pcmC0D0c  # Should return nothing or only TheHand PID
```

## TheHand Configuration

TheHand is configured in `~/.config/thehand/prefs.toml` to use the AB13X directly:

```toml
[audio]
sample_rate = 48000         # AB13X native rate
voice_threshold = 0.010
silence_threshold = 0.005
silence_duration = 0.6
min_speech_duration = 0.5
```

The code in `src/main.rs` hardcodes the device:
```rust
let jabra_device_name = "hw:CARD=Audio,DEV=0".to_string();
```

## Game Wrapper (bin/oni)

If you have a game wrapper that previously managed PulseAudio start/stop, simplify it to just launch the game. PulseAudio now runs permanently with the correct configuration.

Example simplified wrapper:
```bash
#!/bin/bash
# Game launcher - PulseAudio is now permanently configured

export __GL_SYNC_TO_VBLANK=1

echo "Launching game..."
steam steam://rungameid/457140
```

## Troubleshooting

### TheHand can't access microphone
- Check if PulseAudio grabbed it: `pactl list sources short | grep AB13X`
- If it shows up, unload the module: `pactl unload-module <module-id>`
- Verify udev rule is in place: `cat /etc/udev/rules.d/90-pulseaudio-ab13x.rules`

### YouTube has no audio
- Verify AB13X output is loaded: `pactl list sinks short`
- Check default sink: `pactl get-default-sink`
- Should be `alsa_output.hw_0_0`
- Restart browser completely

### First word clipped in transcriptions
- Check if suspend-on-idle is loaded: `pactl list modules short | grep suspend`
- Should return nothing
- Verify sink is IDLE not SUSPENDED: `pactl list sinks short`

### After reboot, configuration lost
- Verify `~/.config/pulse/default.pa` exists
- Verify `/etc/udev/rules.d/90-pulseaudio-ab13x.rules` exists
- Check PulseAudio isn't masked: `systemctl --user status pulseaudio.service`

## Alternative: Using Different USB Audio Device

If using a different USB audio device:

1. Find the device name:
```bash
cat /proc/asound/cards
aplay -l
```

2. Update TheHand code (`src/main.rs`) with your device path:
```rust
let jabra_device_name = "hw:CARD=YourCard,DEV=0".to_string();
```

3. Update PulseAudio config with your card number:
```bash
load-module module-alsa-sink device=hw:X,0  # Replace X with your card number
```

4. Update udev rule with your device's vendor/product IDs from `lsusb`

## Summary

This setup allows:
- ✅ TheHand to access AB13X microphone via ALSA (no blocking)
- ✅ Browser/YouTube audio through AB13X via PulseAudio
- ✅ No audio clipping on transcription playback
- ✅ Persistent configuration across reboots
