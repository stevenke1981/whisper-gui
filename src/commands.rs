use crate::audio::AudioRecorder;
use crate::config::AppConfig;
use crate::send;
use crate::whisper;
use crate::whisper::WhisperEngine;
use slint::{ComponentHandle as _, SharedString};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn setup_commands(
    ui: &crate::AppWindow,
    engine: Arc<Mutex<WhisperEngine>>,
    recorder: Rc<RefCell<AudioRecorder>>,
    config: Rc<RefCell<AppConfig>>,
    target_hwnd: Arc<Mutex<isize>>,
) {
    let ui_weak = ui.as_weak();

    ui.global::<crate::AppState>().on_preset_model_selected({
        let ui_weak = ui_weak.clone();

        move |model_name: SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let state = ui.global::<crate::AppState>();
            let mut settings = state.get_settings();
            settings.model_path = SharedString::from(format!("models/ggml-{}.bin", model_name));
            state.set_settings(settings);
        }
    });

    ui.global::<crate::AppState>().on_load_model({
        let ui_weak = ui_weak.clone();
        let engine = engine.clone();

        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let state = ui.global::<crate::AppState>();
            let model_path = state.get_settings().model_path.to_string();
            let gpu_enabled = state.get_settings().gpu_enabled;

            state.set_is_processing(true);

            if std::path::Path::new(&model_path).exists() {
                state.set_status(SharedString::from(format!("載入模型中: {}…", model_path)));

                let engine2   = engine.clone();
                let ui_weak2  = ui_weak.clone();

                // Run model loading on a background thread to avoid UI freeze
                std::thread::spawn(move || {
                    let result = {
                        let mut eng = engine2.lock().unwrap();
                        eng.load_model(&model_path, gpu_enabled)
                    };

                    // Gather display info while holding lock briefly
                    let (name, memory, backend) = {
                        let eng = engine2.lock().unwrap();
                        (eng.model_name().to_string(), eng.estimate_memory(), eng.backend_label().to_string())
                    };

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak2.upgrade() {
                            let state = ui.global::<crate::AppState>();
                            match result {
                                Ok(()) => {
                                    state.set_status(SharedString::from(format!(
                                        "模型已載入: {} [{}]", name, backend
                                    )));
                                    state.set_memory_usage(SharedString::from(memory));
                                    state.set_gpu_usage(SharedString::from(backend));
                                }
                                Err(e) => {
                                    state.set_status(SharedString::from(format!("載入模型失敗: {}", e)));
                                }
                            }
                            state.set_is_processing(false);
                        }
                    });
                });
            } else {
                let model_name = extract_model_name_from_path(&model_path);
                let ui_weak2   = ui_weak.clone();

                state.set_status(SharedString::from(format!(
                    "⬇ 下載 {} 模型中（請稍候）…", model_name
                )));

                std::thread::spawn(move || {
                    match whisper::download_model(&model_name, "models") {
                        Ok(_) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak2.upgrade() {
                                    ui.global::<crate::AppState>().invoke_load_model();
                                }
                            });
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_weak2.upgrade() {
                                    let state = ui.global::<crate::AppState>();
                                    state.set_status(SharedString::from(format!(
                                        "下載失敗: {}", msg
                                    )));
                                    state.set_is_processing(false);
                                }
                            });
                        }
                    }
                });
            }
        }
    });

    ui.global::<crate::AppState>().on_start_recording({
        let ui_weak = ui_weak.clone();
        let recorder = recorder.clone();

        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let state = ui.global::<crate::AppState>();

            if recorder.borrow().is_recording() {
                return;
            }

            match recorder.borrow_mut().start_recording() {
                Ok(()) => {
                    state.set_is_recording(true);
                    state.set_status(SharedString::from("錄音中..."));
                    state.set_process_log(SharedString::from(""));
                }
                Err(e) => {
                    state.set_status(SharedString::from(format!("錄音錯誤: {}", e)));
                }
            }
        }
    });

    ui.global::<crate::AppState>().on_stop_recording({
        let ui_weak = ui_weak.clone();
        let recorder = recorder.clone();
        let engine = engine.clone();
        let config = config.clone();
        let target_hwnd = target_hwnd.clone();

        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let state = ui.global::<crate::AppState>();
            state.set_is_recording(false);
            state.set_is_processing(true);

            let audio_data = recorder.borrow_mut().stop_recording();

            if audio_data.is_empty() {
                state.set_status(SharedString::from("未錄製到音訊"));
                state.set_is_processing(false);
                return;
            }

            if !engine.lock().unwrap().is_loaded() {
                state.set_status(SharedString::from("尚未載入模型，請先載入"));
                state.set_is_processing(false);
                return;
            }

            let duration_secs = audio_data.len() as f64 / 16000.0;
            let samples = audio_data.len();
            log_append(&state, &format!("音訊 {:.2}s ({} 樣本)，等待引擎鎖...", duration_secs, samples));
            state.set_status(SharedString::from(format!("音訊 {:.1}s，等待 Whisper 引擎...", duration_secs)));

            let language = state.get_settings().language.to_string();
            let send_mode = config.borrow().send_mode.clone();
            let http_url = config.borrow().http_url.clone();
            let output_dir = config.borrow().output_dir.clone();
            let hwnd = *target_hwnd.lock().unwrap();
            let ui_weak2 = ui_weak.clone();
            let engine2 = engine.clone();

            std::thread::spawn(move || {
                let t_total = std::time::Instant::now();

                // Acquire engine lock — measure wait time
                let t_lock = std::time::Instant::now();
                let guard = engine2.lock().unwrap();
                let lock_ms = t_lock.elapsed().as_millis();

                // Notify UI: lock acquired, inference starting
                {
                    let ui_w = ui_weak2.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_w.upgrade() {
                            let state = ui.global::<crate::AppState>();
                            log_append(&state, &format!("引擎就緒（等鎖 {}ms），開始 Whisper 辨識...", lock_ms));
                            state.set_status(SharedString::from(
                                format!("Whisper 辨識中... (等鎖 {}ms)", lock_ms)
                            ));
                        }
                    });
                }

                let t_infer = std::time::Instant::now();
                let result = guard.transcribe_pcm(&audio_data, &language, None);
                let infer_ms = t_infer.elapsed().as_millis();
                let total_ms = t_total.elapsed().as_millis();
                drop(guard);

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak2.upgrade() {
                        let state = ui.global::<crate::AppState>();
                        match result {
                            Ok((ref text, elapsed)) => {
                                state.set_transcribed_text(SharedString::from(text.clone()));

                                let send_result = send::send_text(text, &send_mode, &http_url, &output_dir, hwnd);
                                let send_msg = match &send_result {
                                    Ok(s) => s.clone(),
                                    Err(e) => format!("傳送錯誤: {}", e),
                                };

                                log_append(&state, &format!(
                                    "完成：辨識 {:.2}s | 推論 {}ms | 總計 {}ms | 音訊 {:.2}s | {}",
                                    elapsed, infer_ms, total_ms, duration_secs, send_msg
                                ));
                                state.set_status(SharedString::from(format!(
                                    "完成 辨識:{:.1}s（音訊:{:.1}s  推論:{}ms）→ {}",
                                    elapsed, duration_secs, infer_ms, send_msg
                                )));
                            }
                            Err(ref e) => {
                                let msg = format!("辨識錯誤: {}", e);
                                log_append(&state, &msg);
                                state.set_status(SharedString::from(msg));
                            }
                        }
                        state.set_is_processing(false);
                    }
                });
            });
        }
    });

    ui.global::<crate::AppState>().on_transcribe_file({
        let ui_weak = ui_weak.clone();
        let engine = engine.clone();
        let config = config.clone();
        let target_hwnd = target_hwnd.clone();

        move |_path: SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let state = ui.global::<crate::AppState>();

            let path = rfd::FileDialog::new()
                .add_filter("Audio", &["wav", "mp3", "m4a", "flac", "ogg"])
                .pick_file();

            match path {
                Some(file_path) => {
                    if !engine.lock().unwrap().is_loaded() {
                        state.set_status(SharedString::from("尚未載入模型，請先載入"));
                        return;
                    }
                    let path_str = file_path.to_string_lossy().to_string();
                    state.set_is_processing(true);
                    state.set_status(SharedString::from("轉錄檔案中..."));

                    match transcribe_file(&engine.lock().unwrap(), &path_str, &state.get_settings().language) {
                        Ok(text) => {
                            state.set_transcribed_text(SharedString::from(text));
                            state.set_status(SharedString::from("File transcription complete"));

                            let cfg = config.borrow();
                            let hwnd = *target_hwnd.lock().unwrap();
                            let _ = send::send_text(
                                &state.get_transcribed_text(),
                                &cfg.send_mode,
                                &cfg.http_url,
                                &cfg.output_dir,
                                hwnd,
                            );
                        }
                        Err(e) => {
                            state.set_status(SharedString::from(format!("Error: {}", e)));
                        }
                    }
                    state.set_is_processing(false);
                }
                None => {
                    state.set_status(SharedString::from("No file selected"));
                }
            }
        }
    });

    ui.global::<crate::AppState>().on_select_model_file({
        let ui_weak = ui_weak.clone();

        move || {
            let path = rfd::FileDialog::new()
                .add_filter("Whisper Model", &["bin", "ggml"])
                .pick_file();

            if let Some(file_path) = path {
                let Some(ui) = ui_weak.upgrade() else { return };
                let state = ui.global::<crate::AppState>();
                let mut settings = state.get_settings();
                settings.model_path = file_path.to_string_lossy().to_string().into();
                state.set_settings(settings);
            }
        }
    });

    ui.global::<crate::AppState>().on_send_text({
        let ui_weak = ui_weak.clone();
        let config = config.clone();
        let target_hwnd = target_hwnd.clone();

        move |text: SharedString| {
            let cfg = config.borrow();
            let hwnd = *target_hwnd.lock().unwrap();
            match send::send_text(&text, &cfg.send_mode, &cfg.http_url, &cfg.output_dir, hwnd) {
                Ok(status) => {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.global::<crate::AppState>().set_status(SharedString::from(status));
                    }
                }
                Err(e) => {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.global::<crate::AppState>()
                            .set_status(SharedString::from(format!("Send error: {}", e)));
                    }
                }
            }
        }
    });

    ui.global::<crate::AppState>().on_download_model({
        let ui_weak = ui_weak.clone();

        move |model_name: SharedString| {
            let model_name_str = model_name.to_string();
            let ui_weak2 = ui_weak.clone();

            if let Some(ui) = ui_weak.upgrade() {
                let state = ui.global::<crate::AppState>();
                state.set_is_processing(true);
                state.set_status(SharedString::from(format!(
                    "⬇ Downloading {} model...",
                    model_name_str
                )));
            }

            std::thread::spawn(move || {
                match whisper::download_model(&model_name_str, "models") {
                    Ok(path) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak2.upgrade() {
                                let state = ui.global::<crate::AppState>();
                                state.set_status(SharedString::from(format!(
                                    "Downloaded: {}",
                                    path
                                )));
                                state.set_is_processing(false);
                                ui.global::<crate::AppState>().invoke_load_model();
                            }
                        });
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak2.upgrade() {
                                let state = ui.global::<crate::AppState>();
                                state.set_status(SharedString::from(format!(
                                    "Download failed: {}",
                                    msg
                                )));
                                state.set_is_processing(false);
                            }
                        });
                    }
                }
            });
        }
    });

    ui.global::<crate::AppState>().on_save_settings({
        let config = config.clone();
        let ui_weak = ui_weak.clone();

        move || {
            if let Some(ui) = ui_weak.upgrade() {
                let settings = ui.global::<crate::AppState>().get_settings();
                let mut cfg = config.borrow_mut();
                cfg.model_path = settings.model_path.to_string();
                cfg.language = settings.language.to_string();
                cfg.set_send_mode(&settings.send_mode);
                cfg.http_url = settings.http_url.to_string();
                cfg.gpu_enabled = settings.gpu_enabled;
                cfg.hotkey = settings.hotkey.to_string();
                cfg.output_dir = settings.output_dir.to_string();
                cfg.hotkey_toggle_enabled = settings.hotkey_toggle_enabled;
                cfg.hotkey_ptt_enabled = settings.hotkey_ptt_enabled;
                if let Err(e) = cfg.save() {
                    eprintln!("Failed to save config: {}", e);
                }
            }
        }
    });

    ui.global::<crate::AppState>().on_load_settings({
        let config = config.clone();
        let ui_weak = ui_weak.clone();

        move || {
            if let Ok(cfg) = AppConfig::load() {
                if let Some(ui) = ui_weak.upgrade() {
                    let state = ui.global::<crate::AppState>();
                    let mut settings = state.get_settings();
                    settings.model_path = cfg.model_path.clone().into();
                    settings.language = cfg.language.clone().into();
                    settings.send_mode = cfg.send_mode_str().to_string().into();
                    settings.http_url = cfg.http_url.clone().into();
                    settings.gpu_enabled = cfg.gpu_enabled;
                    settings.hotkey = cfg.hotkey.clone().into();
                    settings.output_dir = cfg.output_dir.clone().into();
                    settings.hotkey_toggle_enabled = cfg.hotkey_toggle_enabled;
                    settings.hotkey_ptt_enabled = cfg.hotkey_ptt_enabled;
                    state.set_settings(settings);
                }
                *config.borrow_mut() = cfg;
            }
        }
    });
}

fn log_append(state: &crate::AppState, msg: &str) {
    let now = chrono::Local::now().format("%H:%M:%S%.3f");
    let new_line = format!("[{}] {}", now, msg);
    let current = state.get_process_log().to_string();
    // Prepend so newest entry is always visible at top (no need to scroll)
    let combined = if current.is_empty() {
        new_line
    } else {
        format!("{}\n{}", new_line, current)
    };
    // Keep at most 12 lines to prevent unbounded growth
    let lines: Vec<&str> = combined.lines().collect();
    let kept = if lines.len() > 12 { lines[..12].join("\n") } else { combined };
    state.set_process_log(SharedString::from(kept));
}

fn extract_model_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("ggml-"))
        .unwrap_or("tiny")
        .to_string()
}

fn transcribe_file(
    _engine: &WhisperEngine,
    file_path: &str,
    _language: &str,
) -> anyhow::Result<String> {
    let path = std::path::Path::new(file_path);
    anyhow::ensure!(path.exists(), "File not found: {}", file_path);
    anyhow::bail!("File transcription requires audio decoding (ffmpeg/symphonia). Use microphone recording for now, or convert to 16kHz mono WAV externally.");
}
