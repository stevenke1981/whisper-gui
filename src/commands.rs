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
            let correction_enabled = config.borrow().correction_enabled;
            let lt_enabled = state.get_settings().languagetool_enabled;
            let gain_enabled = state.get_settings().gain_enabled;
            let gain_level = state.get_gain_level();
            let opencc_mode = state.get_opencc_mode().to_string();
            let is_zh = state.get_is_zh();
            let t_params = whisper::TranscribeParams {
                language: language.clone(),
                n_threads: None,
                temperature: state.get_whisper_temperature(),
                beam_size: state.get_whisper_beam_size().round() as i32,
                best_of: state.get_whisper_best_of().round() as i32,
                no_context: state.get_whisper_no_context(),
                single_segment: state.get_whisper_single_segment(),
                word_timestamps: state.get_whisper_word_timestamps(),
                initial_prompt: state.get_whisper_initial_prompt().to_string(),
                max_len: state.get_whisper_max_len().round() as i32,
            };
            let ui_weak2 = ui_weak.clone();
            let engine2 = engine.clone();

            // Apply recording gain before handing audio to Whisper
            let audio_data = if gain_enabled && gain_level > 1.0 {
                crate::audio::apply_gain(audio_data, gain_level)
            } else {
                audio_data
            };

            // Save WAV to output directory (non-blocking, best-effort)
            match crate::audio::save_wav(&audio_data, &output_dir) {
                Ok(wav_path) => log_append(&state, &format!("音訊已儲存：{}", wav_path)),
                Err(e) => log_append(&state, &format!("音訊儲存失敗：{}", e)),
            }

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
                let result = guard.transcribe_pcm(&audio_data, &t_params);
                let infer_ms = t_infer.elapsed().as_millis();
                let total_ms = t_total.elapsed().as_millis();
                drop(guard);

                // Auto-correct in background thread (before UI update) if enabled
                let (result, correction_log) = match result {
                    Ok((text, elapsed)) if correction_enabled => {
                        let cr = crate::correction::correct_text(&text, &language, lt_enabled);
                        let log = cr.log_line(is_zh);
                        (Ok((cr.text, elapsed)), Some(log))
                    }
                    other => (other, None),
                };

                // OpenCC conversion (s2twp / t2sp)
                let result = match result {
                    Ok((text, elapsed)) if !opencc_mode.is_empty() => {
                        Ok((crate::opencc::apply_mode(&text, &opencc_mode), elapsed))
                    }
                    other => other,
                };

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
                                if let Some(ref log) = correction_log {
                                    log_append(&state, log);
                                }
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

            // Skip if model file already exists
            if whisper::model_exists(&model_name_str, "models") {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.global::<crate::AppState>().set_status(SharedString::from(format!(
                        "✓ {} 模型已存在，直接載入", model_name_str
                    )));
                    ui.global::<crate::AppState>().invoke_load_model();
                }
                return;
            }

            if let Some(ui) = ui_weak.upgrade() {
                let state = ui.global::<crate::AppState>();
                state.set_is_processing(true);
                state.set_status(SharedString::from(format!(
                    "⬇ 下載 {} 模型中（請稍候）…", model_name_str
                )));
            }

            std::thread::spawn(move || {
                match whisper::download_model(&model_name_str, "models") {
                    Ok(path) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak2.upgrade() {
                                let state = ui.global::<crate::AppState>();
                                state.set_status(SharedString::from(format!(
                                    "✓ 下載完成：{}", path
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
                                    "下載失敗: {}", msg
                                )));
                                state.set_is_processing(false);
                            }
                        });
                    }
                }
            });
        }
    });

    ui.global::<crate::AppState>().on_check_model_exists({
        let ui_weak = ui_weak.clone();
        move |model_name: SharedString| {
            let exists = whisper::model_exists(&model_name.to_string(), "models");
            if let Some(ui) = ui_weak.upgrade() {
                ui.global::<crate::AppState>().set_download_model_exists(exists);
            }
        }
    });

    ui.global::<crate::AppState>().on_check_dict_exists({
        let ui_weak = ui_weak.clone();
        move || {
            let exists = whisper::dict_exists("en_frequency.txt", "dicts");
            if let Some(ui) = ui_weak.upgrade() {
                ui.global::<crate::AppState>().set_dict_en_exists(exists);
            }
        }
    });

    ui.global::<crate::AppState>().on_download_dict({
        let ui_weak = ui_weak.clone();
        move |dict_id: SharedString| {
            let id = dict_id.to_string();
            let ui_weak2 = ui_weak.clone();

            if let Some(ui) = ui_weak.upgrade() {
                let state = ui.global::<crate::AppState>();
                state.set_dict_en_downloading(true);
                state.set_status(SharedString::from(format!(
                    "⬇ 下載 {} 字典檔中，請稍候…", id
                )));
            }

            std::thread::spawn(move || {
                let result = whisper::download_dict(&id, "dicts");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak2.upgrade() {
                        let state = ui.global::<crate::AppState>();
                        state.set_dict_en_downloading(false);
                        match result {
                            Ok(path) => {
                                state.set_dict_en_exists(true);
                                state.set_status(SharedString::from(format!(
                                    "✓ 字典檔下載完成：{}", path
                                )));
                            }
                            Err(e) => {
                                state.set_status(SharedString::from(format!(
                                    "字典下載失敗：{}", e
                                )));
                            }
                        }
                    }
                });
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
                cfg.correction_enabled = settings.correction_enabled;
                cfg.languagetool_enabled = settings.languagetool_enabled;
                cfg.gain_enabled = settings.gain_enabled;
                cfg.gain_level = ui.global::<crate::AppState>().get_gain_level();
                cfg.opencc_mode = ui.global::<crate::AppState>().get_opencc_mode().to_string();
                let s = ui.global::<crate::AppState>();
                cfg.temperature = s.get_whisper_temperature();
                cfg.beam_size = s.get_whisper_beam_size().round() as i32;
                cfg.best_of = s.get_whisper_best_of().round() as i32;
                cfg.no_context = s.get_whisper_no_context();
                cfg.single_segment = s.get_whisper_single_segment();
                cfg.word_timestamps = s.get_whisper_word_timestamps();
                cfg.initial_prompt = s.get_whisper_initial_prompt().to_string();
                cfg.max_len = s.get_whisper_max_len().round() as i32;
                if let Err(e) = cfg.save() {
                    eprintln!("Failed to save config: {}", e);
                }
            }
        }
    });

    ui.global::<crate::AppState>().on_correct_text({
        let ui_weak = ui_weak.clone();

        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let state = ui.global::<crate::AppState>();

            let text = state.get_transcribed_text().to_string();
            if text.trim().is_empty() {
                state.set_status(SharedString::from("沒有文字可校正"));
                return;
            }

            let language = state.get_settings().language.to_string();
            let is_zh = state.get_is_zh();
            state.set_is_processing(true);
            state.set_status(SharedString::from(if is_zh { "校正中..." } else { "Correcting..." }));

            let ui_weak2 = ui_weak.clone();

            let lt_enabled_corr = state.get_settings().languagetool_enabled;
            std::thread::spawn(move || {
                let result = crate::correction::correct_text(&text, &language, lt_enabled_corr);
                let corrected = result.text.clone();
                let summary = result.log_line(is_zh);

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak2.upgrade() {
                        let state = ui.global::<crate::AppState>();
                        state.set_transcribed_text(SharedString::from(corrected));
                        log_append(&state, &summary);
                        state.set_status(SharedString::from(summary));
                        state.set_is_processing(false);
                    }
                });
            });
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
                    settings.correction_enabled = cfg.correction_enabled;
                    settings.languagetool_enabled = cfg.languagetool_enabled;
                    settings.gain_enabled = cfg.gain_enabled;
                    state.set_settings(settings);
                    state.set_gain_level(cfg.gain_level);
                    state.set_opencc_mode(cfg.opencc_mode.clone().into());
                    state.set_whisper_temperature(cfg.temperature);
                    state.set_whisper_beam_size(cfg.beam_size as f32);
                    state.set_whisper_best_of(cfg.best_of as f32);
                    state.set_whisper_no_context(cfg.no_context);
                    state.set_whisper_single_segment(cfg.single_segment);
                    state.set_whisper_word_timestamps(cfg.word_timestamps);
                    state.set_whisper_initial_prompt(cfg.initial_prompt.clone().into());
                    state.set_whisper_max_len(cfg.max_len as f32);
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
