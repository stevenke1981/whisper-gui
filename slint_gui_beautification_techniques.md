# Slint GUI 美化技巧大全（2026 最新版）

**宇宙主腦 整理** - 基於 Slint 1.16+ 官方文件與社群最佳實踐

## 1. 選擇正確的 Style（全局風格）

Slint 提供多種預設風格，Fluent 為 1.16+ 預設。

```slint
// 在 build.rs 或 Cargo.toml 設定
// 環境變數：SLINT_STYLE=fluent
// 或 native / cupertino / material 等
```

**推薦**：
- **Fluent**：現代、乾淨（Windows 友好）
- **Cupertino**：macOS 風格
- **Material**：Android / Google 風格

使用 `Platform.style-name` 動態判斷。

## 2. 顏色系統 - 使用 Palette

```slint
import { Palette } from "std-widgets.slint";

Rectangle {
    background: Palette.background;
    border-color: Palette.border;
    // 其他：accent, foreground, selection 等
}
```

自訂全域調色盤：
```slint
global Theme {
    in-out property <brush> primary: #2379F4;
    in-out property <brush> background: #1e1e1e;
}
```

## 3. 常用視覺屬性

### 邊框與圓角
```slint
border-radius: 8px;           // 圓角
border-width: 1px;
border-color: Palette.border;
```

### 陰影與深度
```slint
drop-shadow: 0px 4px 12px rgba(0,0,0,0.3);
```

### 漸層
```slint
background: @linear-gradient(90deg, #2379F4, #00c6ff);
```

## 4. 文字美化

```slint
Text {
    font-family: "Segoe UI", system-ui;
    font-size: 16px;
    font-weight: 500;
    letter-spacing: 0.5px;
    color: Palette.foreground;
}
```

## 5. 狀態與動畫

```slint
Button {
    background: root.pressed ? Palette.accent.darker(20%) : 
                root.has-hover ? Palette.accent : #456;
    
    animate background { duration: 150ms; easing: ease-out; }
}
```

## 6. 自訂元件範例 - Modern Card

```slint
component ModernCard inherits Rectangle {
    in property <string> title;
    in property <string> content;
    
    width: 300px;
    height: 180px;
    border-radius: 16px;
    background: Palette.background;
    drop-shadow: 0px 8px 24px rgba(0,0,0,0.25);
    
    VerticalLayout {
        padding: 20px;
        spacing: 12px;
        
        Text { text: title; font-size: 18px; font-weight: bold; }
        Text { text: content; color: Palette.foreground; }
    }
}
```

## 7. 進階美化技巧

- **玻璃態 (Glassmorphism)**：半透明 + 模糊
- **Neumorphism**：軟陰影 + 內外陰影組合
- **暗黑模式**：使用 `Palette.color-scheme`
- **響應式**：`min-width`、`max-width` + Layout
- **自訂 Widget Style**：進階可覆寫 std-widgets

## 8. 工具與最佳實踐

- 使用 **Slint Live Preview** 即時調整
- VS Code + Slint LSP
- 顏色工具：Coolors / Adobe Color
- 一致性：建立 Global Theme component

---

**下載提示**：複製以下完整內容儲存為 `slint_gui_beautification.md`
