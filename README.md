# Whisper GUI - Offline Speech Recognition

A cross-platform desktop GUI for OpenAI Whisper using Rust + Slint.

## Features
- Offline speech recognition with Whisper models
- Microphone recording with real-time transcription
- Custom send modes: Clipboard, File, HTTP POST
- Multi-language support
- Model management

## Setup

### Prerequisites
- Rust (latest stable)
- LLVM/Clang (for whisper-rs on Windows)
  - Download from https://github.com/llvm/llvm-project/releases
  - Set `LIBCLANG_PATH` environment variable to LLVM bin directory

### Install LLVM on Windows
1. Download LLVM from https://github.com/llvm/llvm-project/releases
2. Extract and set `LIBCLANG_PATH=C:\path\to\llvm\bin`
3. Ensure `clang.dll` is in the PATH

### Build
```bash
cargo build --release
```

### Run
```bash
cargo run
```

## Configuration
- Config saved to `%APPDATA%\whisper-gui\config.json`
- Models directory: `models/`
- Output directory: `output/`

## Send Modes
- **clipboard**: Copy text to clipboard
- **file**: Save transcript to file
- **http-post**: Send via HTTP POST

## Dependencies
- slint: UI framework
- whisper-rs: Whisper bindings
- cpal: Audio recording
- arboard: Clipboard
- reqwest: HTTP client
- rfd: File dialogs
