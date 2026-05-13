# Plan — Whisper GUI

## 里程碑

### MVP（完成）
- [x] Slint 視窗 + 模型載入
- [x] 麥克風錄音 + 轉錄
- [x] 傳送模式（Clipboard/File/HTTP）
- [x] 設定持久化

### v0.3（完成）
- [x] 背景執行緒轉錄（修復 UI 凍結）
- [x] Press-to-Talk 按鈕
- [x] 全域熱鍵（Ctrl+Shift+R / Ctrl+Shift+Space）
- [x] 繁中介面預設

### v0.4（進行中）
- [x] 設定面板（Tab 介面：主頁 + 設定）
- [x] 熱鍵自訂與啟用/禁用 per-hotkey
- [x] 輸出目錄自訂
- [x] 進度條顯示（process-log，12 行容量，newest-first）
- [ ] 檔案轉錄（symphonia 解碼）
- [ ] VAD 靜音偵測（自動分段）

### v0.5（未來）
- [ ] GPU 加速（CUDA）
- [ ] 串流即時轉錄
- [ ] 自訂 Plugin 介面

## 依賴風險

- `global-hotkey`：Windows 保留鍵衝突需測試
- `whisper-rs`：大模型需 >4GB RAM
- 檔案轉錄：需增加 symphonia 依賴

## 變更歷史

| 版本 | 日期 | 內容 |
|------|------|------|
| v0.4.0 | 2026-05-12 | 設定面板、per-hotkey 自訂、輸出目錄、process-log 顯示 |
| v0.3.0 | 2026-05-12 | 新增 PTT 和熱鍵里程碑 |
| v0.1.0 | 2026-05-11 | 初始建立 |
