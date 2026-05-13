use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub model_path: String,
    pub language: String,
    pub send_mode: SendMode,
    pub http_url: String,
    pub gpu_enabled: bool,
    pub hotkey: String,
    pub output_dir: String,
    #[serde(default = "default_true")]
    pub hotkey_toggle_enabled: bool,
    #[serde(default = "default_true")]
    pub hotkey_ptt_enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SendMode {
    Clipboard,
    File,
    HttpPost,
    ActiveWindow,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            model_path: String::from("models/ggml-tiny.bin"),
            language: String::from("auto"),
            send_mode: SendMode::Clipboard,
            http_url: String::new(),
            gpu_enabled: true,
            hotkey: String::new(),
            output_dir: String::from("output"),
            hotkey_toggle_enabled: true,
            hotkey_ptt_enabled: true,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("whisper-gui");
        data_dir.join("config.json")
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn send_mode_str(&self) -> &str {
        match self.send_mode {
            SendMode::Clipboard => "clipboard",
            SendMode::File => "file",
            SendMode::HttpPost => "http-post",
            SendMode::ActiveWindow => "active-window",
        }
    }

    pub fn set_send_mode(&mut self, mode: &str) {
        self.send_mode = match mode {
            "file" => SendMode::File,
            "http-post" => SendMode::HttpPost,
            "active-window" => SendMode::ActiveWindow,
            _ => SendMode::Clipboard,
        };
    }
}