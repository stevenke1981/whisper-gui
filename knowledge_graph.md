# Knowledge Graph — Whisper GUI

初始建立：2026-05-12

## 節點

| 節點 | 類型 | 說明 |
|------|------|------|
| AppWindow | UI | Slint 主視窗，TabWidget（主頁 + 設定） |
| AppState | UI Global | 共用狀態（status, transcribed-text, process-log, progress, is-recording, is-processing, current-model, current-language, memory-usage, is-zh, settings） |
| Settings | UI Struct | send_mode, http_url, gpu_enabled, hotkey, output_dir, hotkey_toggle_enabled, hotkey_ptt_enabled |
| WhisperEngine | Core | whisper-rs 封裝，Arc<Mutex<>>, CUDA feature enabled |
| AudioRecorder | Core | cpal 錄音，Rc<RefCell<>> |
| AppConfig | Config | JSON 設定持久化（%APPDATA%\whisper-gui\config.json） |
| process-log | UI Log | 12-line 容量，newest-first，step-by-step 計時顯示 |
| commands | Glue | UI callback → Rust 邏輯橋接 |
| hotkeys | Feature | global-hotkey 快捷鍵管理（Ctrl+Shift+R / Ctrl+Shift+Space） |
| send | Output | Clipboard/File/HTTP 傳送 |
| download_model | Network | reqwest blocking 下載 HuggingFace 模型 |

## 關係

```
AppWindow → uses → AppState (global)
AppState → embeds → Settings (UI-level struct)
commands → controls → AppState
commands → drives → WhisperEngine
commands → drives → AudioRecorder
commands → reads/writes → AppConfig
commands → updates → process-log (via log_append helper)
hotkeys → invokes → AppState.start_recording / stop_recording
AudioRecorder → feeds → WhisperEngine (audio Vec<f32>)
WhisperEngine → outputs_to → send (transcribed text)
WhisperEngine → outputs_to → process-log (timing, steps)
send → targets → Clipboard | File | HTTP (respecting AppState.settings.send_mode)
AppConfig → persists → model_path, language, send_mode, http_url, gpu_enabled, hotkey, output_dir, hotkey_toggle_enabled, hotkey_ptt_enabled
AppConfig → saves_to → %APPDATA%\whisper-gui\config.json
```

## 變更歷史

| 版本 | 日期 | 內容 | 影響範圍 |
|------|------|------|----------|
| v0.4.0 | 2026-05-12 | 新增 AppState global、Settings 結構、process-log 節點、AppConfig JSON 持久化 | AppWindow, AppState, Settings, AppConfig, process-log, commands, send |
| v0.3.0 | 2026-05-12 | 新增 hotkeys 節點、背景執行緒關係 | hotkeys, commands, WhisperEngine |
| v0.1.0 | 2026-05-11 | 初始建立 | 全部 |
