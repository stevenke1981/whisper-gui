# ITERATIVE_DEV_CORE — Whisper GUI 迭代狀態總覽

current_version: v0.4.0
last_updated: 2026-05-12

## 功能清單

- [x] Slint GUI 基礎視窗（繁/英切換，預設繁中）
- [x] Tab 介面（主頁 Tab + 設定 Tab）
- [x] 模型載入（本地 + 自動下載 HuggingFace）
- [x] 麥克風錄音（cpal 16kHz mono）
- [x] 語音轉錄（whisper-rs，背景執行緒，不凍結 UI）
- [x] Press-to-Talk 按鈕（按住說話，放開轉錄）
- [x] 全域快捷鍵（Ctrl+Shift+R 切換，Ctrl+Shift+Space PTT）
- [x] 熱鍵自訂與啟用/禁用（per-hotkey checkboxes）
- [x] 傳送模式（Clipboard / File / HTTP POST）
- [x] 輸出目錄自訂（rfd folder picker）
- [x] 設定持久化（JSON，%APPDATA%\whisper-gui\config.json）
- [x] 錯誤可視化（rfd 對話框，不閃退）
- [x] Process Log 顯示（12-line 容量，newest-first，step-by-step 計時）
- [x] GPU 支援（CUDA feature 啟用，RTX 3060 Ti 已驗證）
- [ ] 檔案轉錄（需 ffmpeg/symphonia 解碼）
- [ ] VAD 靜音偵測（自動分段）

## 已知問題

- 無

## 變更歷史

| 版本 | 日期 | 內容 | 影響範圍 |
|------|------|------|----------|
| v0.4.0 | 2026-05-12 | Tab 介面、設定面板、per-hotkey 自訂、輸出目錄、process-log (12-line)、GPU CUDA 支援 | appwindow.slint, commands.rs, config.rs, main.rs, Cargo.toml |
| v0.3.0 | 2026-05-12 | PTT 按鈕、全域熱鍵、背景轉錄、繁中預設 | commands.rs, hotkeys.rs, appwindow.slint |
| v0.2.0 | 2026-05-12 | 修復閃退（RefCell match panic）、錯誤對話框 | main.rs, commands.rs |
| v0.1.0 | 2026-05-11 | 初始實作 | 全部 |
