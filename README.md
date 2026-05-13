# Jarvis - Fullscreen AI Desktop Application

A beautiful, fullscreen Rust application providing access to multiple AI providers with intelligent local LLM detection.

## Features

- **Fullscreen by Default**: Starts in fullscreen mode with F11 to toggle
- **Multiple AI Providers**: Support for Anthropic, OpenAI, xAI, Ollama, and LM Studio
- **Intelligent Local LLM Detection**: Automatically detects and notifies when local LLMs (Ollama/LM Studio) become available
- **Settings Panel**: Easy configuration for API keys and preferences
- **Admin Mode**: Password-protected admin capabilities
- **Beautiful UI**: Cyberpunk-inspired interface with animations
- **Auto-Switching**: Automatically prefers local LLMs when available

## Prerequisites

### System Requirements

- Linux/macOS/Windows with GTK3 development libraries (Linux)
- Rust 1.65+
- Node.js 16+ (for frontend development)

### Linux Dependencies

```bash
sudo apt-get install -y \
  libgtk-3-dev \
  libssl-dev \
  libsoup2.4-dev \
  libwebkit2gtk-4.1-dev \
  libjavascriptcoregtk-4.1-dev
```

### macOS

Install Xcode Command Line Tools:
```bash
xcode-select --install
```

### Windows

No additional setup required beyond Rust installation.

## Installation

### Clone and Build

```bash
git clone <repository>
cd Jarvis-Draft
cargo build --release
```

### Development Mode

```bash
npm install
cargo tauri dev
```

## Configuration

### API Keys

1. Click the settings button (gear icon) in the dashboard
2. Enter your API keys for desired providers:
   - **Anthropic**: `sk-ant-...`
   - **OpenAI**: `sk-...`
   - **xAI**: `xai-...`

3. Use "Test Connection" to verify your keys

### Local LLMs

#### Ollama Setup

1. [Install Ollama](https://ollama.ai)
2. Run Ollama:
   ```bash
   ollama serve
   ```
3. Pull a model:
   ```bash
   ollama pull qwen2
   ```

#### LM Studio Setup

1. [Download LM Studio](https://lmstudio.ai)
2. Launch LM Studio
3. Load a model and start the server on port 1234

### Settings Options

- **Auto-switch to Local LLM**: Automatically use local LLMs when available (default: enabled)
- **Notifications**: Get notified when local LLMs become available (default: enabled)
- **Admin Mode**: Set a password to protect admin features

## Usage

### Starting the App

```bash
cargo tauri dev        # Development
cargo tauri build      # Build release binary
```

### Fullscreen Controls

- **F11**: Toggle fullscreen mode
- **Click Blob**: Start interaction

### Asking Questions

1. Click the blob or wait for activation sequence
2. Type your question in the input field
3. Press Enter to submit
4. App automatically selects best available LLM

### Settings Access

1. Click the settings button (⚙️) in the bottom dashboard
2. Configure API keys and preferences
3. Click "Save Settings" to persist changes

## Architecture

### Rust Backend

- **main.rs**: Tauri app initialization and fullscreen setup
- **commands.rs**: Tauri commands for frontend-to-backend communication
- **settings.rs**: Settings management and persistence
- **llm_detector.rs**: Automatic detection of local LLMs
- **api_client.rs**: Integration with all AI providers
- **models.rs**: Shared data structures

### Frontend

- **index.html**: Complete UI with animations and settings panel
- Pure JavaScript (no framework)
- Tauri API integration for backend communication

## LLM Detection

The app automatically checks for local LLMs every 30 seconds:

- **Ollama**: Checks `http://localhost:11434/api/tags`
- **LM Studio**: Checks `http://localhost:1234/api/models`

When detected, users are notified and can opt to use them instead of cloud APIs.

## Supported Models

### Anthropic
- `claude-3-5-sonnet-20241022` (default)

### OpenAI
- `gpt-4o-mini` (default)

### xAI
- `grok-2` (default)

### Ollama
- Any model installed locally (e.g., qwen2, llama2, mistral)

### LM Studio
- Any model loaded in the app

## Building for Release

```bash
# Build for your platform
cargo tauri build

# Built binary location:
# Linux: src-tauri/target/release/jarvis
# macOS: src-tauri/target/release/jarvis.app
# Windows: src-tauri/target/release/jarvis.exe
```

## Troubleshooting

### "GTK3 not found"

Install development headers:
```bash
sudo apt-get install libgtk-3-dev
```

### "Local LLM not detected"

1. Verify Ollama/LM Studio is running
2. Check correct ports: Ollama (11434), LM Studio (1234)
3. Check firewall settings
4. Restart the app

### "API key not working"

1. Verify key is correct in settings
2. Use "Test Connection" button
3. Check API provider status and rate limits
4. Ensure key has proper permissions

### Build Fails on Linux

Make sure all required packages are installed:
```bash
sudo apt-get install -y \
  build-essential \
  libssl-dev \
  libgtk-3-dev \
  libsoup2.4-dev \
  libwebkit2gtk-4.1-dev \
  libjavascriptcoregtk-4.1-dev \
  pkg-config
```

## Performance

- Lightweight async runtime using Tokio
- Efficient LLM detection (30-second check interval)
- Optimized particle/grid animations with canvas rendering
- Settings cached in memory with file persistence

## Security

- API keys stored in local config file (encrypt for production)
- Admin password hashing recommended for multi-user systems
- No data sent to external services except API providers
- All API calls use HTTPS

## Development

### Project Structure

```
Jarvis-Draft/
├── src/
│   ├── main.rs           # App entry point
│   ├── commands.rs       # Tauri commands
│   ├── settings.rs       # Settings management
│   ├── llm_detector.rs   # LLM detection
│   ├── api_client.rs     # API integrations
│   └── models.rs         # Data structures
├── index.html            # Frontend UI
├── Cargo.toml           # Rust dependencies
├── tauri.conf.json      # Tauri config
└── package.json         # Node dependencies
```

### Adding a New AI Provider

1. Add provider to `models.rs` enum
2. Implement query function in `api_client.rs`
3. Add settings field in `settings.rs`
4. Create frontend UI in `index.html`
5. Add command in `commands.rs`

## Future Enhancements

- Voice input/output integration
- Custom model selection per query
- Chat history management
- Plugin system for custom LLMs
- Cross-platform native notifications
- Settings encryption
- Multi-window support

## License

MIT License

## Contributing

Contributions welcome! Please ensure:
- Code compiles without warnings
- Settings are properly persisted
- LLM detection works reliably
- UI remains responsive

## Support

For issues or feature requests, please open an issue on the repository.
