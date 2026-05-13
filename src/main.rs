#![cfg_attr(windows, windows_subsystem = "windows")]

mod commands;
mod config;
mod audio;
mod hotkeys;
mod send;
mod whisper;

slint::include_modules!();

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        eprintln!("PANIC: {}", msg);
        rfd::MessageDialog::new()
            .set_title("Whisper GUI - Fatal Crash")
            .set_description(&format!("應用程式崩潰：\n\n{}", msg))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }));

    if let Err(e) = run() {
        let msg = format!("{:?}", e);
        eprintln!("Startup error: {}", msg);
        rfd::MessageDialog::new()
            .set_title("Whisper GUI - 啟動錯誤")
            .set_description(&format!("啟動失敗：\n\n{}", msg))
            .set_level(rfd::MessageLevel::Error)
            .show();
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let config = Rc::new(RefCell::new(config::AppConfig::load().unwrap_or_default()));
    let engine = Arc::new(Mutex::new(whisper::WhisperEngine::new()));
    let recorder = Rc::new(RefCell::new(audio::AudioRecorder::new()));
    let target_hwnd = Arc::new(Mutex::new(0isize));

    let ui = AppWindow::new()?;

    sync_config_to_ui(&ui, &config.borrow());

    commands::setup_commands(&ui, engine, recorder, config, target_hwnd.clone());

    // Auto-load model 100 ms after event loop starts
    let ui_weak_init = ui.as_weak();
    let init_timer = slint::Timer::default();
    init_timer.start(
        slint::TimerMode::SingleShot,
        std::time::Duration::from_millis(100),
        move || {
            if let Some(ui) = ui_weak_init.upgrade() {
                ui.global::<AppState>().invoke_load_model();
            }
        },
    );

    // Global hotkeys (kept alive until window closes)
    let (_hk_manager, _hk_timer) = hotkeys::setup(ui.as_weak(), target_hwnd);

    ui.run()?;
    Ok(())
}

fn sync_config_to_ui(ui: &AppWindow, config: &config::AppConfig) {
    let state = ui.global::<AppState>();
    let mut settings = state.get_settings();
    settings.model_path = config.model_path.clone().into();
    settings.language = config.language.clone().into();
    settings.send_mode = config.send_mode_str().to_string().into();
    settings.http_url = config.http_url.clone().into();
    settings.gpu_enabled = true; // always force CUDA
    settings.hotkey = config.hotkey.clone().into();
    settings.output_dir = config.output_dir.clone().into();
    settings.hotkey_toggle_enabled = config.hotkey_toggle_enabled;
    settings.hotkey_ptt_enabled = config.hotkey_ptt_enabled;
    state.set_settings(settings);
    state.set_current_model(extract_model_name(&config.model_path).into());
    state.set_current_language(config.language.clone().into());
}

fn extract_model_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("tiny")
}
