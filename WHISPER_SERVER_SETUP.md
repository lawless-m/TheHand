# Whisper Server Setup Guide

This guide covers setting up a whisper.cpp server for use with TheHand voice transcription.

## Overview

TheHand connects to a Whisper server via HTTP to transcribe recorded audio. The server runs whisper.cpp with GPU acceleration using the large-v3 model for high-quality transcription.

## Requirements

### Hardware
- **GPU**: NVIDIA GPU with CUDA support (large-v3 model requires GPU)
- **VRAM**: At least 6GB recommended for large-v3
- **CPU**: Any modern x86_64 processor
- **RAM**: 8GB minimum, 16GB recommended
- **Storage**: ~3GB for the large-v3 model

### Software
- Linux (tested on Debian/Ubuntu)
- CUDA toolkit (for GPU acceleration)
- NVIDIA drivers
- Build tools (gcc, g++, make, cmake)
- Git

## Installation Steps

### 1. Install Prerequisites

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install build tools
sudo apt install -y build-essential cmake git wget

# Install CUDA (if not already installed)
# Check NVIDIA driver version first
nvidia-smi

# For CUDA 12.x on Debian/Ubuntu:
wget https://developer.download.nvidia.com/compute/cuda/repos/debian11/x86_64/cuda-keyring_1.0-1_all.deb
sudo dpkg -i cuda-keyring_1.0-1_all.deb
sudo apt update
sudo apt install -y cuda
```

### 2. Clone and Build whisper.cpp

```bash
# Clone the repository
cd ~
git clone https://github.com/ggerganov/whisper.cpp.git
cd whisper.cpp

# Build with CUDA support
mkdir build
cd build
cmake .. -DWHISPER_CUDA=ON
cmake --build . --config Release

# Verify the build
ls -la bin/
# Should see: whisper-server, whisper-cli, etc.
```

### 3. Download the Whisper Model

```bash
# Download large-v3 model (recommended for quality)
cd ~/whisper.cpp/models
bash download-ggml-model.sh large-v3

# Verify the download
ls -lh ggml-large-v3.bin
# Should show ~3GB file
```

**Alternative models** (if GPU memory is limited):
- `medium` - ~1.5GB, good quality, less VRAM
- `small` - ~500MB, fast but lower quality
- `base` - ~150MB, very fast but basic quality

### 4. Test the Server

```bash
# Start the server manually
cd ~/whisper.cpp/build
./bin/whisper-server \
  -m ../models/ggml-large-v3.bin \
  -l en \
  --port 5555 \
  --host 0.0.0.0

# Test from another terminal
curl -X POST http://localhost:5555/inference \
  -F "file=@test.wav"
```

The server should:
- Start on port 5555
- Accept POST requests to `/inference`
- Return JSON with transcribed text

### 5. Create a Systemd Service

Create `/etc/systemd/system/whisper-server.service`:

```ini
[Unit]
Description=Whisper.cpp Transcription Server
After=network.target

[Service]
Type=simple
User=YOUR_USERNAME
WorkingDirectory=/home/YOUR_USERNAME/whisper.cpp/build
ExecStart=/home/YOUR_USERNAME/whisper.cpp/build/bin/whisper-server \
  -m /home/YOUR_USERNAME/whisper.cpp/models/ggml-large-v3.bin \
  -l en \
  --port 5555 \
  --host 0.0.0.0 \
  --threads 4
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

**Important:** Replace `YOUR_USERNAME` with your actual username.

Enable and start the service:

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable whisper-server

# Start the service
sudo systemctl start whisper-server

# Check status
sudo systemctl status whisper-server

# View logs
sudo journalctl -u whisper-server -f
```

### 6. Configure Firewall (if needed)

If running the server on a separate machine:

```bash
# Allow port 5555 through firewall
sudo ufw allow 5555/tcp

# Or for iptables:
sudo iptables -A INPUT -p tcp --dport 5555 -j ACCEPT
sudo iptables-save | sudo tee /etc/iptables/rules.v4
```

## Server Configuration Options

Common whisper-server options:

```bash
--model, -m PATH       # Model file path (required)
--language, -l LANG    # Language code (en, es, fr, etc.)
--port PORT            # Server port (default: 8080)
--host HOST            # Bind address (0.0.0.0 for all interfaces)
--threads N            # CPU threads to use
--processors N         # Number of processors for parallel inference
--convert              # Convert audio to 16kHz automatically
```

### Performance Tuning

For better performance:

```bash
# Use multiple processors for parallel requests
--processors 2

# Adjust threads based on CPU cores
--threads 8

# Enable automatic audio conversion
--convert
```

## Testing the Server

### From the Same Machine

```bash
# Record a test audio file
arecord -d 3 -f S16_LE -r 16000 -c 1 test.wav

# Send to server
curl -X POST http://localhost:5555/inference \
  -F "file=@test.wav"
```

### From TheHand Client

Update TheHand config to point to your server:

```toml
# ~/.config/thehand/prefs.toml
[whisper]
server_url = "http://SERVER_IP:5555"
```

Or use command line:
```bash
thehand --server http://SERVER_IP:5555
```

## Monitoring and Maintenance

### Check Server Status

```bash
# Check if running
systemctl status whisper-server

# Check logs
journalctl -u whisper-server -n 50

# Check GPU usage
nvidia-smi

# Monitor in real-time
watch -n 1 nvidia-smi
```

### Performance Metrics

Typical performance for large-v3 on mid-range GPU:
- **Transcription speed**: 2-4x real-time (1 second of audio in 0.25-0.5 seconds)
- **GPU memory**: 4-6GB VRAM
- **Accuracy**: Excellent for English speech

### Common Issues

**Server won't start:**
```bash
# Check CUDA is available
nvidia-smi

# Check model file exists
ls -lh ~/whisper.cpp/models/ggml-large-v3.bin

# Check port isn't in use
sudo lsof -i :5555
```

**Slow transcription:**
- Verify GPU is being used: `nvidia-smi` should show whisper-server process
- Try reducing model size to `medium` or `small`
- Increase `--processors` count

**Out of memory:**
- Use smaller model (`medium` instead of `large-v3`)
- Reduce `--processors` count
- Check other GPU processes: `nvidia-smi`

## Network Setup

### Local Machine Setup
- Server: `http://localhost:5555`
- TheHand connects to localhost
- No firewall configuration needed

### Separate Server Setup
- Server: `http://SERVER_IP:5555` (bind to 0.0.0.0)
- TheHand connects to server IP
- Open firewall port 5555
- Consider using SSH tunnel for security:
  ```bash
  ssh -L 5555:localhost:5555 user@server
  # Then connect to http://localhost:5555
  ```

## Security Considerations

The whisper-server has no authentication by default:

1. **Firewall**: Only expose to trusted networks
2. **SSH Tunnel**: Use for remote access
3. **VPN**: Run on VPN-only interface
4. **Reverse Proxy**: Add nginx with authentication if needed

Example nginx config with basic auth:

```nginx
server {
    listen 80;
    server_name whisper.example.com;

    auth_basic "Whisper Server";
    auth_basic_user_file /etc/nginx/.htpasswd;

    location / {
        proxy_pass http://localhost:5555;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Upgrading

To upgrade whisper.cpp:

```bash
cd ~/whisper.cpp
git pull
cd build
cmake --build . --config Release

# Restart service
sudo systemctl restart whisper-server
```

To update the model:

```bash
cd ~/whisper.cpp/models
mv ggml-large-v3.bin ggml-large-v3.bin.old
bash download-ggml-model.sh large-v3

# Restart service
sudo systemctl restart whisper-server
```

## Alternative: CPU-Only Setup

If you don't have a GPU, you can run on CPU (much slower):

```bash
# Build without CUDA
cd ~/whisper.cpp/build
cmake .. -DWHISPER_CUDA=OFF
cmake --build . --config Release

# Use smaller model for acceptable speed
cd ~/whisper.cpp/models
bash download-ggml-model.sh base

# Start server with more threads
./bin/whisper-server \
  -m ../models/ggml-base.bin \
  -l en \
  --port 5555 \
  --threads 8
```

**Note**: CPU-only transcription with large models is very slow. Use `base` or `small` models for reasonable performance.

## Integration with TheHand

Once the server is running:

1. Configure TheHand to use the server:
   ```toml
   # ~/.config/thehand/prefs.toml
   [whisper]
   server_url = "http://SERVER_IP:5555"
   ```

2. Or specify via command line:
   ```bash
   thehand --server http://SERVER_IP:5555
   ```

3. TheHand will automatically send recorded audio to the server for transcription

## Troubleshooting Network Issues

Test connectivity:

```bash
# From TheHand machine
curl http://SERVER_IP:5555/

# Check if server is listening
# On server:
sudo netstat -tlnp | grep 5555

# Test with sample audio
curl -X POST http://SERVER_IP:5555/inference \
  -F "file=@test.wav" \
  -v
```

## Summary

- **whisper.cpp** provides the transcription server
- **large-v3 model** requires GPU but gives best quality
- **Port 5555** is the default (configurable)
- **Systemd service** keeps it running automatically
- **TheHand** connects via HTTP to `/inference` endpoint

For production use:
1. Run as systemd service
2. Monitor with journalctl
3. Use SSH tunnel or VPN for remote access
4. Keep whisper.cpp updated via git pull
