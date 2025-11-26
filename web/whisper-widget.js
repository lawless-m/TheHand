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
            audioContext: null,
            analyser: null,
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

        if (this.elements.canvas) {
            this.canvasCtx = this.elements.canvas.getContext('2d');
        }
    }

    _bindEvents() {
        this.elements.recordBtn.addEventListener('click', () => this.toggle());
        this.elements.copyBtn.addEventListener('click', () => this.copy());

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
            const stream = await navigator.mediaDevices.getUserMedia({
                audio: {
                    channelCount: 1,
                    sampleRate: 16000,
                    echoCancellation: true,
                    noiseSuppression: true
                }
            });

            // Set up audio context for visualization
            this.state.audioContext = new (window.AudioContext || window.webkitAudioContext)();
            this.state.analyser = this.state.audioContext.createAnalyser();
            this.state.analyser.fftSize = 2048;
            const source = this.state.audioContext.createMediaStreamSource(stream);
            source.connect(this.state.analyser);

            // Determine best supported format
            let mimeType = 'audio/webm';
            if (MediaRecorder.isTypeSupported('audio/webm;codecs=opus')) {
                mimeType = 'audio/webm;codecs=opus';
            } else if (MediaRecorder.isTypeSupported('audio/ogg;codecs=opus')) {
                mimeType = 'audio/ogg;codecs=opus';
            } else if (MediaRecorder.isTypeSupported('audio/mp4')) {
                mimeType = 'audio/mp4';
            }

            this.state.mimeType = mimeType;
            this.state.mediaRecorder = new MediaRecorder(stream, { mimeType });
            this.state.audioChunks = [];
            this.state.stream = stream;

            this.state.mediaRecorder.ondataavailable = (event) => {
                if (event.data.size > 0) {
                    this.state.audioChunks.push(event.data);
                }
            };

            this.state.mediaRecorder.onstop = () => this._onRecordingStopped();

            this.state.mediaRecorder.start(100);
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

        if (this.state.mediaRecorder && this.state.mediaRecorder.state !== 'inactive') {
            this.state.mediaRecorder.stop();
        }

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
    }

    async _onRecordingStopped() {
        this.state.isProcessing = true;

        const audioBlob = new Blob(this.state.audioChunks, { type: this.state.mimeType });

        // Clean up stream
        if (this.state.stream) {
            this.state.stream.getTracks().forEach(track => track.stop());
        }
        if (this.state.audioContext) {
            this.state.audioContext.close();
            this.state.audioContext = null;
        }

        await this._transcribe(audioBlob);

        this.state.isProcessing = false;
        if (this.elements.timer) {
            this.elements.timer.textContent = '0:00';
        }
        this.state.startTime = null;
        this._drawIdle();
    }

    async _transcribe(audioBlob) {
        this._emit('processing');

        try {
            const formData = new FormData();

            let extension = 'webm';
            if (audioBlob.type.includes('ogg')) extension = 'ogg';
            else if (audioBlob.type.includes('mp4')) extension = 'mp4';
            else if (audioBlob.type.includes('wav')) extension = 'wav';

            formData.append('file', audioBlob, `recording.${extension}`);

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
