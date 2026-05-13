use crate::config::SendMode;
use anyhow::Context;
use std::fs;
use std::path::PathBuf;

pub fn send_text(
    text: &str,
    mode: &SendMode,
    http_url: &str,
    output_dir: &str,
    target_hwnd: isize,
) -> anyhow::Result<String> {
    match mode {
        SendMode::Clipboard => send_to_clipboard(text),
        SendMode::File => save_to_file(text, output_dir),
        SendMode::HttpPost => send_http(text, http_url),
        SendMode::ActiveWindow => send_to_active_window(text, target_hwnd),
    }
}

fn send_to_clipboard(text: &str) -> anyhow::Result<String> {
    let mut clipboard = arboard::Clipboard::new().context("Failed to open clipboard")?;
    clipboard.set_text(text).context("Failed to set clipboard")?;
    Ok("已複製到剪貼簿".to_string())
}

fn save_to_file(text: &str, output_dir: &str) -> anyhow::Result<String> {
    let dir = PathBuf::from(output_dir);
    fs::create_dir_all(&dir).context("Failed to create output directory")?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("transcript_{}.txt", timestamp);
    let filepath = dir.join(&filename);

    fs::write(&filepath, text).context("Failed to write file")?;
    Ok(format!("已儲存至 {}", filepath.display()))
}

fn send_http(text: &str, url: &str) -> anyhow::Result<String> {
    if url.is_empty() {
        anyhow::bail!("HTTP URL is empty");
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "text": text,
            "timestamp": chrono::Local::now().to_rfc3339(),
        });

        let resp = client
            .post(url)
            .json(&payload)
            .send()
            .await
            .context("HTTP request failed")?;

        if resp.status().is_success() {
            Ok(format!("已送出至 {} (HTTP {})", url, resp.status()))
        } else {
            anyhow::bail!("HTTP error: {}", resp.status());
        }
    })
}

fn send_to_active_window(text: &str, target_hwnd: isize) -> anyhow::Result<String> {
    // Always copy to clipboard first so there is a fallback
    send_to_clipboard(text)?;

    if target_hwnd == 0 {
        return Ok("已複製到剪貼簿（無記錄目標視窗，請用快捷鍵觸發錄音）".to_string());
    }

    #[cfg(windows)]
    {
        use winapi::shared::windef::HWND;
        use winapi::um::winuser::{IsWindow, SetForegroundWindow};
        let hwnd = target_hwnd as HWND;
        unsafe {
            if IsWindow(hwnd) == 0 {
                return Ok("已複製到剪貼簿（目標視窗已關閉）".to_string());
            }
            SetForegroundWindow(hwnd);
        }
        // Give the target window time to receive focus before we send keys
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).context("Failed to create Enigo")?;
    enigo.key(Key::Control, Direction::Press).ok();
    enigo.key(Key::Unicode('v'), Direction::Click).ok();
    enigo.key(Key::Control, Direction::Release).ok();

    Ok("已貼到目標視窗".to_string())
}
