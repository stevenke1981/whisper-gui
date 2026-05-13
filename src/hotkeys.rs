use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use slint::ComponentHandle as _;
use std::sync::{Arc, Mutex};

/// Registers global hotkeys and returns the manager + polling timer (both must stay alive).
///
/// Hotkeys (no conflict with standard Windows shortcuts):
///   Ctrl+Shift+R       — toggle start/stop recording
///   Ctrl+Shift+Space   — press-to-talk (press = start, release = stop+transcribe)
pub fn setup(
    ui_weak: slint::Weak<crate::AppWindow>,
    target_hwnd: Arc<Mutex<isize>>,
) -> (GlobalHotKeyManager, slint::Timer) {
    let manager = GlobalHotKeyManager::new().expect("Failed to create GlobalHotKeyManager");

    let toggle = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyR);
    let ptt = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);

    let toggle_id = toggle.id();
    let ptt_id = ptt.id();

    manager.register(toggle).ok();
    manager.register(ptt).ok();

    let receiver = GlobalHotKeyEvent::receiver();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(30),
        move || {
            while let Ok(event) = receiver.try_recv() {
                let Some(ui) = ui_weak.upgrade() else { continue };
                let state = ui.global::<crate::AppState>();
                let settings = state.get_settings();

                if event.id == toggle_id && event.state == HotKeyState::Pressed {
                    if !settings.hotkey_toggle_enabled {
                        continue;
                    }
                    if state.get_is_recording() {
                        state.invoke_stop_recording();
                    } else if !state.get_is_processing() {
                        *target_hwnd.lock().unwrap() = foreground_hwnd();
                        state.invoke_start_recording();
                    }
                } else if event.id == ptt_id {
                    if !settings.hotkey_ptt_enabled {
                        continue;
                    }
                    match event.state {
                        HotKeyState::Pressed
                            if !state.get_is_recording() && !state.get_is_processing() =>
                        {
                            *target_hwnd.lock().unwrap() = foreground_hwnd();
                            state.invoke_start_recording();
                        }
                        HotKeyState::Released if state.get_is_recording() => {
                            state.invoke_stop_recording();
                        }
                        _ => {}
                    }
                }
            }
        },
    );

    (manager, timer)
}

#[cfg(windows)]
fn foreground_hwnd() -> isize {
    unsafe { winapi::um::winuser::GetForegroundWindow() as isize }
}

#[cfg(not(windows))]
fn foreground_hwnd() -> isize {
    0
}
