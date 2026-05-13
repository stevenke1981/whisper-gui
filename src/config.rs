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
    #[serde(default = "default_true")]
    pub correction_enabled: bool,
    #[serde(default)]
    pub gain_enabled: bool,
    #[serde(default = "default_gain_level")]
    pub gain_level: f32,
    #[serde(default)]
    pub opencc_mode: String,  // "" | "s2twp" (簡→繁) | "t2sp" (繁→簡)
    #[serde(default = "default_true")]
    pub languagetool_enabled: bool,
    // ── Whisper engine params ────────────────────────────────────────────────
    #[serde(default)]
    pub temperature: f32,           // 0.0 = most deterministic
    #[serde(default = "default_beam_size")]
    pub beam_size: i32,             // ≥2 → BeamSearch; 1 → Greedy
    #[serde(default = "default_best_of")]
    pub best_of: i32,               // samples for greedy fallback
    #[serde(default)]
    pub no_context: bool,           // ignore previous segment context
    #[serde(default)]
    pub single_segment: bool,       // force single output segment
    #[serde(default)]
    pub word_timestamps: bool,      // token-level timestamps
    #[serde(default)]
    pub initial_prompt: String,     // domain hint for first segment
    #[serde(default)]
    pub max_len: i32,               // max tokens per segment; 0 = unlimited
}

fn default_true() -> bool { true }
fn default_gain_level() -> f32 { 2.0 }
fn default_beam_size() -> i32 { 5 }
fn default_best_of() -> i32 { 5 }

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
            model_path: String::from("models/ggml-medium.bin"),
            language: String::from("auto"),
            send_mode: SendMode::Clipboard,
            http_url: String::new(),
            gpu_enabled: true,
            hotkey: String::new(),
            output_dir: String::from("output"),
            hotkey_toggle_enabled: true,
            hotkey_ptt_enabled: true,
            correction_enabled: true,
            gain_enabled: false,
            gain_level: 2.0,
            opencc_mode: String::new(),
            languagetool_enabled: true,
            temperature: 0.0,
            beam_size: 5,
            best_of: 5,
            no_context: false,
            single_segment: false,
            word_timestamps: false,
            initial_prompt: String::new(),
            max_len: 0,
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