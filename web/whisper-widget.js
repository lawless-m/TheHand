/**
 * WhisperWidget - A modular audio transcription component
 *
 * Usage:
 *   const widget = new WhisperWidget('#container', {
 *     serverUrl: '/whisper/inference',
 *     onTranscription: (text) => console.log(text)
 *   });
 *
 * Events emitted on the container element:
 *   - whisper:ready        - Widget initialized
 *   - whisper:recording    - Recording started
 *   - whisper:stopped      - Recording stopped
 *   - whisper:processing   - Sending to server
 *   - whisper:transcription - Transcription received (detail: { text })
 *   - whisper:error        - Error occurred (detail: { message })
 */

class WhisperWidget {
    static defaultOptions = {
        serverUrl: '/whisper/inference',
        showVisualizer: true,
        showTimer: true,
        showHistory: true,
        historyLimit: 20,
        onTranscription: null,
        onError: null,
        onRecordingStart: null,
        onRecordingStop: null
    };

    constructor(container, options = {}) {
        this.container = typeof container === 'string'
            ? document.querySelector(container)
            : container;

        if (!this.container) {
            throw new Error('WhisperWidget: Container element not found');
        }

        this.options = { ...WhisperWidget.defaultOptions, ...options };
        this.state = {
            isRecording: false,
            isProcessing: false,
            mediaRecorder: null,
            audioChunks: [],
            pcmData: [],  // Store raw PCM samples
            audioContext: null,
            analyser: null,
            scriptProcessor: null,
            animationId: null,
            timerInterval: null,
            startTime: null,
            history: []
        };

        this.elements = {};
        this._init();
    }

    _init() {
        this._render();
        this._bindEvents();
        this._initCanvas();
        this._emit('ready');
    }

    _render() {
        this.container.classList.add('whisper-widget');

        this.container.innerHTML = `
            <div class="ww-device-selector">
                <label for="ww-mic-select">Microphone:</label>
                <select class="ww-mic-select">
                    <option value="">Loading...</option>
                </select>
            </div>
            <div class="ww-device-selector">
                <label for="ww-server-select">Server:</label>
                <select class="ww-server-select">
                    <option value="http://rivsprod01:5555/inference">rivsprod01:5555 (Network)</option>
                    <option value="http://localhost:5555/inference">localhost:5555 (Local)</option>
                </select>
            </div>
            <div class="ww-visualizer-section">
                ${this.options.showVisualizer ? '<canvas class="ww-visualizer"></canvas>' : ''}
                <div class="ww-controls">
                    ${this.options.showTimer ? '<span class="ww-timer">0:00</span>' : ''}
                    <button class="ww-record-btn" title="Click to record (Space)">
                        <span class="ww-record-icon"></span>
                    </button>
                    <div class="ww-status">
                        <span class="ww-status-dot"></span>
                        <span class="ww-status-text">Ready</span>
                    </div>
                </div>
            </div>
            <div class="ww-output-section">
                <div class="ww-output-header">
                    <span class="ww-output-label">Transcription</span>
                    <button class="ww-copy-btn">Copy</button>
                </div>
                <div class="ww-output" data-empty="true">Click record to start...</div>
            </div>
            ${this.options.showHistory ? `
                <div class="ww-history-section" style="display: none;">
                    <div class="ww-history-header">
                        <span class="ww-history-label">History</span>
                        <button class="ww-clear-btn">Clear</button>
                    </div>
                    <div class="ww-history-list"></div>
                </div>
            ` : ''}
        `;

        // Cache element references
        this.elements = {
            micSelect: this.container.querySelector('.ww-mic-select'),
            serverSelect: this.container.querySelector('.ww-server-select'),
            canvas: this.container.querySelector('.ww-visualizer'),
            timer: this.container.querySelector('.ww-timer'),
            recordBtn: this.container.querySelector('.ww-record-btn'),
            statusDot: this.container.querySelector('.ww-status-dot'),
            statusText: this.container.querySelector('.ww-status-text'),
            output: this.container.querySelector('.ww-output'),
            copyBtn: this.container.querySelector('.ww-copy-btn'),
            historySection: this.container.querySelector('.ww-history-section'),
            historyList: this.container.querySelector('.ww-history-list'),
            clearBtn: this.container.querySelector('.ww-clear-btn')
        };

        // Populate microphone list
        this._populateMicrophoneList();

        // Set server selector to current server URL
        if (this.elements.serverSelect) {
            this.elements.serverSelect.value = this.options.serverUrl;
        }

        if (this.elements.canvas) {
            this.canvasCtx = this.elements.canvas.getContext('2d');
        }
    }

    _bindEvents() {
        this.elements.recordBtn.addEventListener('click', () => this.toggle());
        this.elements.copyBtn.addEventListener('click', () => this.copy());

        // Server selector change handler
        if (this.elements.serverSelect) {
            this.elements.serverSelect.addEventListener('change', (e) => {
                this.setServerUrl(e.target.value);
                console.log('Server changed to:', this.options.serverUrl);
            });
        }

        if (this.elements.clearBtn) {
            this.elements.clearBtn.addEventListener('click', () => this.clearHistory());
        }

        // Keyboard shortcuts within widget
        this.container.addEventListener('keydown', (e) => {
            if (e.code === 'Space' && e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
                e.preventDefault();
                this.toggle();
            }
            if (e.code === 'Escape' && this.state.isRecording) {
                this.stop();
            }
        });

        // Make container focusable for keyboard events
        if (!this.container.hasAttribute('tabindex')) {
            this.container.setAttribute('tabindex', '0');
        }
    }

    _initCanvas() {
        if (!this.elements.canvas) return;

        const resize = () => {
            const dpr = window.devicePixelRatio || 1;
            const rect = this.elements.canvas.getBoundingClientRect();
            this.elements.canvas.width = rect.width * dpr;
            this.elements.canvas.height = rect.height * dpr;
            this.canvasCtx.scale(dpr, dpr);
            this._drawIdle();
        };

        resize();
        window.addEventListener('resize', resize);
        this._resizeHandler = resize;
    }

    _drawIdle() {
        if (!this.elements.canvas) return;

        const width = this.elements.canvas.getBoundingClientRect().width;
        const height = this.elements.canvas.getBoundingClientRect().height;

        this.canvasCtx.fillStyle = 'var(--ww-bg-visualizer, #1a1a2e)';
        this.canvasCtx.fillRect(0, 0, width, height);

        this.canvasCtx.beginPath();
        this.canvasCtx.strokeStyle = 'var(--ww-accent, #e94560)';
        this.canvasCtx.lineWidth = 2;
        this.canvasCtx.moveTo(0, height / 2);
        this.canvasCtx.lineTo(width, height / 2);
        this.canvasCtx.stroke();
    }

    _drawVisualizer() {
        if (!this.state.analyser || !this.elements.canvas) return;

        const width = this.elements.canvas.getBoundingClientRect().width;
        const height = this.elements.canvas.getBoundingClientRect().height;
        const bufferLength = this.state.analyser.frequencyBinCount;
        const dataArray = new Uint8Array(bufferLength);

        const draw = () => {
            this.state.animationId = requestAnimationFrame(draw);
            this.state.analyser.getByteTimeDomainData(dataArray);

            this.canvasCtx.fillStyle = 'var(--ww-bg-visualizer, #1a1a2e)';
            this.canvasCtx.fillRect(0, 0, width, height);

            this.canvasCtx.lineWidth = 2;
            this.canvasCtx.strokeStyle = this.state.isRecording
                ? 'var(--ww-recording, #ef4444)'
                : 'var(--ww-accent, #e94560)';
            this.canvasCtx.beginPath();

            const sliceWidth = width / bufferLength;
            let x = 0;

            for (let i = 0; i < bufferLength; i++) {
                const v = dataArray[i] / 128.0;
                const y = (v * height) / 2;

                if (i === 0) {
                    this.canvasCtx.moveTo(x, y);
                } else {
                    this.canvasCtx.lineTo(x, y);
                }
                x += sliceWidth;
            }

            this.canvasCtx.lineTo(width, height / 2);
            this.canvasCtx.stroke();
        };

        draw();
    }

    _updateTimer() {
        if (!this.state.startTime || !this.elements.timer) return;
        const elapsed = Math.floor((Date.now() - this.state.startTime) / 1000);
        const minutes = Math.floor(elapsed / 60);
        const seconds = elapsed % 60;
        this.elements.timer.textContent = `${minutes}:${seconds.toString().padStart(2, '0')}`;
    }

    _setStatus(status, text) {
        this.elements.statusDot.className = 'ww-status-dot ww-status-' + status;
        this.elements.statusText.textContent = text;
    }

    _emit(eventName, detail = {}) {
        const event = new CustomEvent(`whisper:${eventName}`, {
            detail,
            bubbles: true
        });
        this.container.dispatchEvent(event);
    }

    // Public API

    async _populateMicrophoneList() {
        try {
            // Request initial permissions to get device labels
            await navigator.mediaDevices.getUserMedia({ audio: true })
                .then(stream => stream.getTracks().forEach(track => track.stop()));

            const devices = await navigator.mediaDevices.enumerateDevices();
            const audioInputs = devices.filter(device => device.kind === 'audioinput');

            this.elements.micSelect.innerHTML = audioInputs.map((device, idx) =>
                `<option value="${device.deviceId}"${idx === 0 ? ' selected' : ''}>
                    ${device.label || `Microphone ${idx + 1}`}
                </option>`
            ).join('');
        } catch (err) {
            console.error('Failed to enumerate devices:', err);
            this.elements.micSelect.innerHTML = '<option value="">No microphones found</option>';
        }
    }

    async toggle() {
        if (this.state.isRecording) {
            this.stop();
        } else {
            await this.start();
        }
    }

    async start() {
        if (this.state.isRecording || this.state.isProcessing) return;

        try {
            const selectedDeviceId = this.elements.micSelect.value;

            const constraints = {
                audio: {
                    deviceId: selectedDeviceId ? { exact: selectedDeviceId } : undefined,
                    channelCount: 1,
                    echoCancellation: false,
                    noiseSuppression: false,
                    autoGainControl: false
                }
            };

            const stream = await navigator.mediaDevices.getUserMedia(constraints);

            // Log which device is actually being used
            const audioTrack = stream.getAudioTracks()[0];
            console.log('Using microphone:', audioTrack.label, audioTrack.getSettings());

            // Set up audio context for visualization and recording (use device's native sample rate)
            this.state.audioContext = new (window.AudioContext || window.webkitAudioContext)();
            console.log('AudioContext sample rate:', this.state.audioContext.sampleRate);

            this.state.analyser = this.state.audioContext.createAnalyser();
            this.state.analyser.fftSize = 2048;
            const source = this.state.audioContext.createMediaStreamSource(stream);
            source.connect(this.state.analyser);

            // Capture raw PCM data using ScriptProcessorNode
            this.state.scriptProcessor = this.state.audioContext.createScriptProcessor(4096, 1, 1);
            this.state.pcmData = [];  // Reset for new recording
            this.state.nativeSampleRate = this.state.audioContext.sampleRate;
            console.log('Recording will capture at', this.state.nativeSampleRate, 'Hz');

            this.state.scriptProcessor.onaudioprocess = (e) => {
                if (this.state.isRecording) {
                    const inputData = e.inputBuffer.getChannelData(0);
                    // Copy the data
                    const copy = new Float32Array(inputData);
                    this.state.pcmData.push(copy);

                    // Debug: Check if we're actually getting audio
                    if (this.state.pcmData.length === 1) {
                        const rms = Math.sqrt(copy.reduce((sum, val) => sum + val * val, 0) / copy.length);
                        console.log('First audio chunk RMS:', rms, 'length:', copy.length);
                    }
                }
            };

            source.connect(this.state.scriptProcessor);
            this.state.scriptProcessor.connect(this.state.audioContext.destination);
            this.state.stream = stream;
            this.state.isRecording = true;
            this.elements.recordBtn.classList.add('ww-recording');
            this._setStatus('recording', 'Recording...');

            // Start timer
            this.state.startTime = Date.now();
            this.state.timerInterval = setInterval(() => this._updateTimer(), 100);

            // Start visualization
            this._drawVisualizer();

            this._emit('recording');
            if (this.options.onRecordingStart) {
                this.options.onRecordingStart();
            }

        } catch (err) {
            console.error('WhisperWidget: Error accessing microphone:', err);
            this._showError('Could not access microphone. Please grant permission.');
        }
    }

    stop() {
        if (!this.state.isRecording) return;

        this.state.isRecording = false;
        this.elements.recordBtn.classList.remove('ww-recording');
        this._setStatus('processing', 'Processing...');

        clearInterval(this.state.timerInterval);
        this.state.timerInterval = null;

        if (this.state.animationId) {
            cancelAnimationFrame(this.state.animationId);
            this.state.animationId = null;
        }

        this._emit('stopped');
        if (this.options.onRecordingStop) {
            this.options.onRecordingStop();
        }

        // Process the recorded audio
        this._onRecordingStopped();
    }

    async _onRecordingStopped() {
        this.state.isProcessing = true;

        console.log('Recorded', this.state.pcmData.length, 'chunks at', this.state.nativeSampleRate, 'Hz');

        // Clean up stream resources
        if (this.state.stream) {
            this.state.stream.getTracks().forEach(track => track.stop());
        }
        if (this.state.scriptProcessor) {
            this.state.scriptProcessor.disconnect();
            this.state.scriptProcessor = null;
        }
        if (this.state.audioContext) {
            this.state.audioContext.close();
            this.state.audioContext = null;
        }

        // Check if we have any audio data
        if (this.state.pcmData.length === 0) {
            this._showError('Recording too short - no audio captured');
            this.state.isProcessing = false;
            if (this.elements.timer) {
                this.elements.timer.textContent = '0:00';
            }
            this.state.startTime = null;
            this._drawIdle();
            return;
        }

        // Convert PCM data to WAV (resample to 16kHz)
        const wavBlob = await this._pcmDataToWav(this.state.pcmData, this.state.nativeSampleRate);
        await this._transcribe(wavBlob);

        this.state.isProcessing = false;
        if (this.elements.timer) {
            this.elements.timer.textContent = '0:00';
        }
        this.state.startTime = null;
        this._drawIdle();
    }

    async _transcribe(wavBlob) {
        this._emit('processing');

        try {
            const formData = new FormData();
            formData.append('file', wavBlob, 'recording.wav');

            const response = await fetch(this.options.serverUrl, {
                method: 'POST',
                body: formData
            });

            if (!response.ok) {
                throw new Error(`Server returned ${response.status}: ${response.statusText}`);
            }

            const result = await response.json();

            if (result.text) {
                const text = result.text.trim();
                this._displayOutput(text);
                this._addToHistory(text);
                this._setStatus('ready', 'Ready');

                this._emit('transcription', { text });
                if (this.options.onTranscription) {
                    this.options.onTranscription(text);
                }
            } else {
                throw new Error('No transcription text in response');
            }

        } catch (err) {
            console.error('WhisperWidget: Transcription error:', err);
            this._showError(`Transcription failed: ${err.message}`);
        }
    }

    async _resample(pcmData, fromSampleRate, toSampleRate) {
        // Create an AudioBuffer from the PCM data
        const audioContext = new OfflineAudioContext(1, pcmData.length, fromSampleRate);
        const audioBuffer = audioContext.createBuffer(1, pcmData.length, fromSampleRate);
        audioBuffer.copyToChannel(pcmData, 0);

        // Create an offline context for resampling
        const duration = pcmData.length / fromSampleRate;
        const outputLength = Math.ceil(duration * toSampleRate);
        const offlineContext = new OfflineAudioContext(1, outputLength, toSampleRate);

        const source = offlineContext.createBufferSource();
        source.buffer = audioBuffer;
        source.connect(offlineContext.destination);
        source.start();

        const resampled = await offlineContext.startRendering();
        return resampled.getChannelData(0);
    }

    async _pcmDataToWav(pcmChunks, nativeSampleRate) {
        // Flatten all PCM chunks into a single array
        let totalLength = 0;
        for (const chunk of pcmChunks) {
            totalLength += chunk.length;
        }

        let pcmData = new Float32Array(totalLength);
        let offset = 0;
        for (const chunk of pcmChunks) {
            pcmData.set(chunk, offset);
            offset += chunk.length;
        }

        // Resample to 16kHz if needed
        const targetSampleRate = 16000;
        if (nativeSampleRate !== targetSampleRate) {
            pcmData = await this._resample(pcmData, nativeSampleRate, targetSampleRate);
        }

        // Convert to 16-bit PCM WAV
        const numberOfChannels = 1;
        const bitDepth = 16;
        const bytesPerSample = bitDepth / 8;
        const blockAlign = numberOfChannels * bytesPerSample;
        const dataLength = pcmData.length * bytesPerSample;
        const sampleRate = targetSampleRate;

        const buffer = new ArrayBuffer(44 + dataLength);
        const view = new DataView(buffer);

        // WAV header
        const writeString = (offset, string) => {
            for (let i = 0; i < string.length; i++) {
                view.setUint8(offset + i, string.charCodeAt(i));
            }
        };

        writeString(0, 'RIFF');
        view.setUint32(4, 36 + dataLength, true);
        writeString(8, 'WAVE');
        writeString(12, 'fmt ');
        view.setUint32(16, 16, true);
        view.setUint16(20, 1, true); // PCM
        view.setUint16(22, numberOfChannels, true);
        view.setUint32(24, sampleRate, true);
        view.setUint32(28, sampleRate * blockAlign, true);
        view.setUint16(32, blockAlign, true);
        view.setUint16(34, bitDepth, true);
        writeString(36, 'data');
        view.setUint32(40, dataLength, true);

        // Convert float32 PCM to int16
        let writeOffset = 44;
        for (let i = 0; i < pcmData.length; i++) {
            const sample = Math.max(-1, Math.min(1, pcmData[i]));
            view.setInt16(writeOffset, sample < 0 ? sample * 0x8000 : sample * 0x7FFF, true);
            writeOffset += 2;
        }

        return new Blob([buffer], { type: 'audio/wav' });
    }

    _displayOutput(text) {
        this.elements.output.textContent = text;
        this.elements.output.removeAttribute('data-empty');
    }

    _showError(message) {
        this._setStatus('error', 'Error');
        this._emit('error', { message });
        if (this.options.onError) {
            this.options.onError(message);
        }
    }

    _addToHistory(text) {
        if (!this.options.showHistory) return;

        const timestamp = new Date().toLocaleTimeString();
        this.state.history.unshift({ text, timestamp });

        if (this.state.history.length > this.options.historyLimit) {
            this.state.history.pop();
        }

        this._renderHistory();
    }

    _renderHistory() {
        if (!this.elements.historySection) return;

        if (this.state.history.length === 0) {
            this.elements.historySection.style.display = 'none';
            return;
        }

        this.elements.historySection.style.display = 'block';
        this.elements.historyList.innerHTML = this.state.history.map(item => `
            <div class="ww-history-item">
                <span class="ww-history-text">${this._escapeHtml(item.text)}</span>
                <span class="ww-history-time">${item.timestamp}</span>
            </div>
        `).join('');
    }

    _escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    copy() {
        const text = this.elements.output.textContent;
        if (text && !this.elements.output.hasAttribute('data-empty')) {
            navigator.clipboard.writeText(text).then(() => {
                const btn = this.elements.copyBtn;
                const original = btn.textContent;
                btn.textContent = 'Copied!';
                btn.classList.add('ww-copied');
                setTimeout(() => {
                    btn.textContent = original;
                    btn.classList.remove('ww-copied');
                }, 2000);
            });
        }
    }

    clearHistory() {
        this.state.history = [];
        this._renderHistory();
    }

    getTranscription() {
        if (this.elements.output.hasAttribute('data-empty')) {
            return null;
        }
        return this.elements.output.textContent;
    }

    getHistory() {
        return [...this.state.history];
    }

    setServerUrl(url) {
        this.options.serverUrl = url;
    }

    destroy() {
        // Clean up
        if (this.state.isRecording) {
            this.stop();
        }
        if (this._resizeHandler) {
            window.removeEventListener('resize', this._resizeHandler);
        }
        this.container.innerHTML = '';
        this.container.classList.remove('whisper-widget');
    }
}

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = WhisperWidget;
}
