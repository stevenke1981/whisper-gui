# Blueprint — Whisper GUI 系統架構

## 執行緒模型

```
主執行緒（Slint Event Loop）
├── UI 渲染 & 事件處理
├── Slint 回調（start_recording, stop_recording, load_model...）
├── 熱鍵輪詢 Timer（每 30ms，global-hotkey receiver）
└── invoke_from_event_loop 接收器

背景執行緒（std::thread::spawn）
├── 轉錄執行緒（每次錄音結束後生成）
│   ├── 輸入：Vec<f32> audio_data
│   ├── 執行：engine.lock().transcribe_pcm()
│   └── 輸出：invoke_from_event_loop 更新 UI
└── 下載執行緒（模型不存在時）
    ├── 執行：reqwest::blocking 下載
    └── 輸出：invoke_from_event_loop 觸發 load_model
```

## 資料流

```
麥克風 → cpal Stream → Arc<Mutex<Vec<f32>>> buffer
                                    ↓ stop_recording()
                               Vec<f32> audio_data
                                    ↓ 背景執行緒（std::thread::spawn）
                         WhisperEngine.transcribe_pcm()
                                    ↓ invoke_from_event_loop
              UI: transcribed-text + process-log (12-line cap, newest-first)
                                    ↓
                           send::send_text() → Clipboard/File/HTTP

AppConfig ← Rc<RefCell<AppConfig>> (JSON load/save)
                                    ↓ save_config()
                        %APPDATA%\whisper-gui\config.json
```

## 關鍵元件

| 元件 | 型別 | 說明 |
|------|------|------|
| engine | Arc<Mutex<WhisperEngine>> | 跨執行緒共用，Mutex 序列化存取 |
| recorder | Rc<RefCell<AudioRecorder>> | 僅主執行緒使用 |
| config | Rc<RefCell<AppConfig>> | 僅主執行緒使用，JSON 持久化 |
| hotkey manager | GlobalHotKeyManager | 保持存活至視窗關閉 |
| AppState (Global) | Slint Global | status, transcribed-text, process-log, progress, is-recording, is-processing, current-model, current-language, memory-usage, is-zh, settings |
| Settings (Slint) | Struct | model_path, language, send_mode, http_url, gpu_enabled, hotkey, output_dir, hotkey_toggle_enabled, hotkey_ptt_enabled |

## 快捷鍵設計

| 鍵位 | 功能 | 衝突分析 |
|------|------|---------|
| Ctrl+Shift+R | 切換錄音 | 安全（無 Windows 保留） |
| Ctrl+Shift+Space | 按住說話 | 需注意 IME 切換（部分系統） |

## 變更歷史

| 版本 | 日期 | 內容 |
|------|------|------|
| v0.4.0 | 2026-05-12 | 新增 AppState 全域狀態、Settings 結構、process-log (12-line) 資料流、AppConfig JSON 持久化 |
| v0.3.0 | 2026-05-12 | 新增背景執行緒轉錄、熱鍵架構 |
| v0.1.0 | 2026-05-11 | 初始架構文件 |
