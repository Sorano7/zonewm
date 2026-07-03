mod config;
mod actions;
mod models;
mod state;
mod commands;
mod overlay;
#[cfg(debug_assertions)]
mod debug;
mod tray;
mod test_utils;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use models::system::{Win32System};

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetForegroundWindow, GetMessageW, KillTimer,
    SetTimer, TranslateMessage, MSG, WM_HOTKEY, WM_TIMER,
};

use crate::models::monitor;

use crate::overlay::{
    flash::{FlashOverlay, NO_HWND, HOT_RELOAD_MS, STYLE_POLL_MS, DISPLAY_MS, MONITOR_POLL_MS},
};
use crate::state::{StateMap, set_all_window_styles};
use crate::state::window_state::Direction;
use crate::tray::SystemTray;
use crate::actions::{Action, ActionCtx, hooks};

fn hot_reload(states: &mut StateMap, cfg_path: &Path, cfg_mtime: &mut Option<SystemTime>) {
    let new_mtime = config::mtime(cfg_path);
    if new_mtime != *cfg_mtime {
        *cfg_mtime = new_mtime;
        let layouts = config::to_layouts(&config::load(cfg_path));
        for ms in states.values_mut() {
            ms.reload_layouts(layouts.clone(), &Win32System);
        }
    }
}

fn poll_monitors(states: &mut StateMap, cfg_path: &Path, saved: &config::SavedState) {
    let monitors = monitor::enumerate_monitors();
    state::reconcile(states, monitors, cfg_path, saved, &Win32System);
}

fn on_hotkey(
    id: i32,
    states: &mut StateMap,
    flash: &mut FlashOverlay
) {
    let focused = unsafe { GetForegroundWindow() };
    let mon_key = unsafe { MonitorFromWindow(focused, MONITOR_DEFAULTTONEAREST) }.0 as isize;
    let hot_count = state::workspace::WORKSPACE_COUNT as i32;

    let mut ctx = ActionCtx {
        states: states, mon_key: mon_key, focused: focused, flash: flash,
    };

    if (hooks::LAYOUT_HOT_BASE..hooks::LAYOUT_HOT_BASE + hot_count).contains(&id) {
        let layout_idx = (id - hooks::LAYOUT_HOT_BASE) as usize;
        Action::SetLayout(layout_idx).execute(&mut ctx);

    } else if (hooks::WORKSPACE_HOT_BASE..hooks::WORKSPACE_HOT_BASE + hot_count).contains(&id) {
        let ws_idx = (id - hooks::WORKSPACE_HOT_BASE) as usize;
        Action::SetWorkspace(ws_idx).execute(&mut ctx);

    } else if (hooks::MOVE_HOT_BASE..hooks::MOVE_HOT_BASE + hot_count).contains(&id) {
        let ws_idx = (id - hooks::MOVE_HOT_BASE) as usize;
        Action::WinMoveWS(ws_idx).execute(&mut ctx);

    } else if id == hooks::FLOAT_HOT_ID {
        Action::SetFloat.execute(&mut ctx);

    } else if id == hooks::MONITOR_LOCK_HOT_ID {
        Action::ToggleMonLock.execute(&mut ctx);

    } else if (hooks::FOCUS_HOT_BASE..hooks::FOCUS_HOT_BASE + 4).contains(&id) {
        if let Some(dir) = Direction::from_idx((id - hooks::FOCUS_HOT_BASE) as usize) {
            Action::WinFocus(dir).execute(&mut ctx);
        }

    } else if (hooks::WIN_MOVE_HOT_BASE..hooks::WIN_MOVE_HOT_BASE + 4).contains(&id) {
        if let Some(dir) = Direction::from_idx((id - hooks::WIN_MOVE_HOT_BASE) as usize) {
            Action::WinMove(dir).execute(&mut ctx);
        }

    } else if (hooks::WIN_SWAP_HOT_BASE..hooks::WIN_SWAP_HOT_BASE + 4).contains(&id) {
        if let Some(dir) = Direction::from_idx((id - hooks::WIN_SWAP_HOT_BASE) as usize) {
            Action::WinSwap(dir).execute(&mut ctx);
        }

    } else if (hooks::WIN_STRETCH_HOT_BASE..hooks::WIN_STRETCH_HOT_BASE + 4).contains(&id) {
        if let Some(dir) = Direction::from_idx((id - hooks::WIN_STRETCH_HOT_BASE) as usize) {
            Action::WinStretch(dir).execute(&mut ctx);
        }

    } else if (hooks::WIN_SHRINK_HOT_BASE..hooks::WIN_SHRINK_HOT_BASE + 4).contains(&id) {
        if let Some(dir) = Direction::from_idx((id - hooks::WIN_SHRINK_HOT_BASE) as usize) {
            Action::WinShrink(dir).execute(&mut ctx);
        }

    } else if id == hooks::WIN_CYCLE_NEXT_HOT_ID {
        Action::WinCycle(true).execute(&mut ctx);

    } else if id == hooks::WIN_CYCLE_PREV_HOT_ID {
        Action::WinCycle(false).execute(&mut ctx);
    }
}

fn run(
    states: &mut StateMap,
    cfg_path: &Path,
    running: &AtomicBool,
    tray: &SystemTray,
    saved: &config::SavedState,
) {
    let hot_reload_timer   = unsafe { SetTimer(NO_HWND, 0, HOT_RELOAD_MS,   None) };
    let style_timer        = unsafe { SetTimer(NO_HWND, 0, STYLE_POLL_MS,   None) };
    let display_timer      = unsafe { SetTimer(NO_HWND, 0, DISPLAY_MS,      None) };
    let monitor_poll_timer = unsafe { SetTimer(NO_HWND, 0, MONITOR_POLL_MS, None) };

    let mut cfg_mtime = config::mtime(cfg_path);
    let mut flash = FlashOverlay::new();
    let mut msg = MSG::default();
    let mut prev_focused = HWND::default();

    while running.load(Ordering::SeqCst) {
        let ret = unsafe { GetMessageW(&mut msg, NO_HWND, 0, 0) };
        if ret.0 <= 0 { break; }

        match msg.message {
            WM_TIMER => {
                hooks::tick();
                flash.try_expire(msg.wParam.0);

                let focused = unsafe { GetForegroundWindow() };

                if tray.quit_requested() {
                    running.store(false, Ordering::SeqCst);
                }
                if msg.wParam.0 == hot_reload_timer {
                    hot_reload(states, cfg_path, &mut cfg_mtime);
                }
                if msg.wParam.0 == monitor_poll_timer {
                    poll_monitors(states, cfg_path, saved);
                }
                #[cfg(debug_assertions)]
                if msg.wParam.0 == display_timer {
                    debug::print_status(states, focused);
                }
                if msg.wParam.0 == style_timer {
                    if let Some(new_focused) = set_all_window_styles(states, prev_focused) {
                        prev_focused = new_focused;
                    }

                }
            }
            WM_HOTKEY => on_hotkey(msg.wParam.0 as i32, states, &mut flash),
            _ => {}
        }

        unsafe {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }

    unsafe {
        let _ = KillTimer(NO_HWND, hot_reload_timer);
        let _ = KillTimer(NO_HWND, style_timer);
        let _ = KillTimer(NO_HWND, display_timer);
        let _ = KillTimer(NO_HWND, monitor_poll_timer);
    }
}

pub fn run_wm() {
    #[cfg(debug_assertions)]
    debug::enable_ansi_console();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        #[cfg(debug_assertions)]
        println!("Exit");
        r.store(false, Ordering::SeqCst);
    }).expect("Error handling Ctrl-C");

    unsafe { let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }

    let cfg_path = config::config_path();
    let layouts = config::to_layouts(&config::load(&cfg_path));

    let saved = config::load_state(&config::state_path());
    let monitors = monitor::enumerate_monitors();
    let mut states = state::build(monitors, layouts);
    for ms in states.values_mut() {
        if let Some(&idx) = saved.monitor_layouts.get(&ms.monitor_key()) {
            ms.switch_layout(idx);
        }
        ms.capture_all_windows(&Win32System);
        ms.clear_all_window_style();
    }

    overlay::register_class();
    let _state_guard   = hooks::StateGuard::new(&mut states);
    let hook           = hooks::install();
    assert!(!hook.0.is_null(),        "zonewm: failed to install WinEvent move/size hook");
    let focus_hook     = hooks::install_focus();
    assert!(!focus_hook.0.is_null(),  "zonewm: failed to install WinEvent focus hook");
    let minimize_hook  = hooks::install_minimize();
    assert!(!minimize_hook.0.is_null(), "zonewm: failed to install WinEvent minimize hook");
    let destroy_hook   = hooks::install_destroy();
    assert!(!destroy_hook.0.is_null(), "zonewm: failed to install WinEvent destroy hook");
    let show_hook      = hooks::install_show();
    assert!(!show_hook.0.is_null(),   "zonewm: failed to install WinEvent show hook");
    let kbd_hook       = hooks::install_kbd();
    assert!(!kbd_hook.0.is_null(),    "zonewm: failed to install keyboard hook");

    let tray = SystemTray::new();

    run(&mut states, &cfg_path, &running, &tray, &saved);

    let mut persist = config::SavedState::default();
    for ms in states.values() {
        persist.monitor_layouts.insert(ms.monitor_key(), ms.workspace1_layout_idx());
        ms.clear_all_window_style();
    }
    config::save_state(&config::state_path(), &persist);

    for ms in states.values() { ms.uncloak_all(&Win32System); }
    hooks::uninstall(hook);
    hooks::uninstall(focus_hook);
    hooks::uninstall(minimize_hook);
    hooks::uninstall(destroy_hook);
    hooks::uninstall(show_hook);
    hooks::uninstall_kbd(kbd_hook);
}
