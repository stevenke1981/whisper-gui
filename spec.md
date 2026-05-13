# Whisper-Slint GUI 專案規格書 (Project Specification)

## 1. 專案概述
- **專案名稱**：WhisperSlint-GUI
- **版本**：v0.4.0
- **目標**：使用 Rust + Slint 開發跨平台桌面 GUI 應用，整合 whisper.cpp 進行離線語音轉錄，並支援自訂轉錄文字發送機制。
- **核心價值**：輕量、高性能、完全離線、隱私優先、可擴展自訂發送。
- **GPU 支援**：CUDA 加速（RTX 3060 Ti with CUDA 12.6），feature flag `cuda` 啟用。

## 2. 功能需求

### 2.1 核心功能
- 麥克風錄音轉錄（cpal，16kHz mono PCM）
- Press-to-Talk 按鈕（按住說話，放開自動轉錄）
- 全域快捷鍵（Ctrl+Shift+R 切換錄音；Ctrl+Shift+Space 按住說話）
- 模型管理（tiny/base/small/medium/large-v3，支援量化 Q4/Q5/Q8）
- 多語言支援（auto / zh / en / ...）
- 轉錄在背景執行緒，UI 全程響應

### 2.2 自訂發送機制（Custom Send）
- **模式選擇**：
  - Clipboard（預設）
  - Save to File（.txt）
  - HTTP POST（reqwest）
- 每段轉錄完成後自動觸發發送

### 2.3 GUI 介面（Slint 宣告式）
- **Tab 介面**：
  - 主頁 Tab：模型選擇、語言選擇、錄音/停止/PTT 按鈕、轉錄文字、process-log（12 行，newest-first）
  - 設定 Tab：send mode 選擇、HTTP URL、GPU 啟用/禁用、輸出目錄、per-hotkey 啟用/禁用、Save/Load 按鈕
- 狀態列：模型載入狀態、記憶體使用、進度條
- 繁中/英文切換（預設繁中）
- Process Log：step-by-step 計時顯示，時間戳，容量 12 行

### 2.4 非功能需求
- **平台**：Windows（主要）/ macOS / Linux
- **依賴**：Slint + whisper-rs + cpal + arboard + reqwest + global-hotkey

## 3. 技術架構
- **UI**：Slint (.slint) + Rust 後端
- **ASR**：whisper-rs（Arc<Mutex<>>, 背景執行緒轉錄）
- **音訊**：cpal（錄音）
- **發送**：arboard（剪貼簿）、reqwest（HTTP）
- **熱鍵**：global-hotkey（Slint Timer 輪詢）
- **建置**：Cargo + slint-build

## 4. 專案結構
```
whisper-gui/
├── ui/appwindow.slint   # 宣告式 UI
├── src/
│   ├── main.rs          # 入口，Arc<Mutex<engine>>, 熱鍵初始化
│   ├── whisper.rs       # WhisperEngine（unsafe impl Send）
│   ├── audio.rs         # AudioRecorder（cpal）
│   ├── commands.rs      # UI callback 橋接
│   ├── hotkeys.rs       # 全域熱鍵（global-hotkey）
│   ├── send.rs          # 發送模式
│   └── config.rs        # 設定持久化
├── models/              # 模型檔案
├── plan.md, spec.md, blueprint.md, knowledge_graph.md
└── ITERATIVE_DEV_CORE.md
```

## 5. 里程碑
1. [x] Slint 基礎視窗 + 麥克風轉錄
2. [x] 自訂發送（Clipboard + File + HTTP）
3. [x] 背景轉錄 + PTT + 熱鍵
4. [x] 設定面板（Tab 介面）+ per-hotkey + process-log
5. [ ] GPU 加速與 VAD（CUDA feature 開發中）
6. [ ] 檔案轉錄（symphonia）

## 變更歷史

| 版本 | 日期 | 內容 |
|------|------|------|
| v0.4.0 | 2026-05-12 | 設定 Tab、per-hotkey 自訂、輸出目錄、process-log、GPU CUDA 支援 |
| v0.3.0 | 2026-05-12 | PTT、熱鍵、背景轉錄、繁中預設 |
| v0.1.0 | 2026-05-11 | 初始規格 |
