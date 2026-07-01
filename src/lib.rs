mod config;
mod hooks;
mod models;
mod state;
mod commands;
mod overlay;
mod debug;
mod test_utils;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use models::system::{Win32System};

use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetForegroundWindow, GetMessageW, KillTimer,
    SetTimer, TranslateMessage, MSG, WM_HOTKEY, WM_TIMER,
};

use crate::models::{
    window,
    monitor,
};
use crate::overlay::{
    focus::FocusBorder,
    flash::{FlashOverlay, NO_HWND, HOT_RELOAD_MS, FOCUS_POLL_MS, DISPLAY_MS},
};
use crate::state::StateMap;
use crate::state::window_state::Direction;

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

fn on_hotkey(
    id: i32,
    states: &mut StateMap,
    flash: &mut FlashOverlay,
    focus_border: &mut FocusBorder,
) {
    let focused = unsafe { GetForegroundWindow() };
    let mon_key = unsafe { MonitorFromWindow(focused, MONITOR_DEFAULTTONEAREST) }.0 as isize;
    let hot_count = state::workspace::WORKSPACE_COUNT as i32;

    if (hooks::LAYOUT_HOT_BASE..hooks::LAYOUT_HOT_BASE + hot_count).contains(&id) {
        let layout_idx = (id - hooks::LAYOUT_HOT_BASE) as usize;
        if let Some(s) = states.get_mut(&mon_key) {
            s.switch_layout(layout_idx);
            s.reflow(&Win32System);
            if let Some(layout) = s.active_layout() {
                let zones = layout.zones.iter()
                    .map(|z| z.to_rect(s.monitor.work_area))
                    .collect();
                flash.show(s.monitor.work_area, zones);
            } else {
                flash.cancel();
            }
        }
    } else if (hooks::WORKSPACE_HOT_BASE..hooks::WORKSPACE_HOT_BASE + hot_count).contains(&id) {
        let ws_idx = (id - hooks::WORKSPACE_HOT_BASE) as usize;
        if let Some(s) = states.get_mut(&mon_key) {
            s.switch_workspace(ws_idx, &Win32System);
            if let Some(hwnd) = s.first_visible_window(&Win32System) {
                commands::window::set_foreground_window(hwnd);
                focus_border.update(hwnd);
            } else {
                focus_border.clear();
            }
        }
    } else if (hooks::MOVE_HOT_BASE..hooks::MOVE_HOT_BASE + hot_count).contains(&id) {
        let ws_idx = (id - hooks::MOVE_HOT_BASE) as usize;
        if let Some(s) = states.get_mut(&mon_key) {
            s.move_window_to_workspace(focused, ws_idx, &Win32System);
            s.switch_workspace(ws_idx, &Win32System);
        }
    } else if id == hooks::FLOAT_HOT_ID {
        if let Some(s) = states.get_mut(&mon_key) {
            s.set_floating(focused, &Win32System);
        }
    } else if id == hooks::MONITOR_LOCK_HOT_ID {
        if let Some(s) = states.get_mut(&mon_key) {
            s.monitor_locked = !s.monitor_locked;
        }
    } else if (hooks::FOCUS_HOT_BASE..hooks::FOCUS_HOT_BASE + 4).contains(&id) {
        if let Some(dir) = Direction::from_idx((id - hooks::FOCUS_HOT_BASE) as usize) {
            commands::window::handle_focus_move(focused, mon_key, dir, states, focus_border);
        }
    } else if (hooks::WIN_MOVE_HOT_BASE..hooks::WIN_MOVE_HOT_BASE + 4).contains(&id) {
        if let Some(dir) = Direction::from_idx((id - hooks::WIN_MOVE_HOT_BASE) as usize) {
            commands::window::handle_window_move(focused, mon_key, dir, states, focus_border);
        }
    } else if (hooks::WIN_SWAP_HOT_BASE..hooks::WIN_SWAP_HOT_BASE + 4).contains(&id) {
        if let Some(dir) = Direction::from_idx((id - hooks::WIN_SWAP_HOT_BASE) as usize) {
            commands::window::handle_window_swap(focused, mon_key, dir, states);
        }
    } else if id == hooks::WIN_CYCLE_NEXT_HOT_ID {
        commands::window::handle_cycle(focused, mon_key, true, states, focus_border);
    } else if id == hooks::WIN_CYCLE_PREV_HOT_ID {
       commands::window::handle_cycle(focused, mon_key, false, states, focus_border);
    }
}

fn run(states: &mut StateMap, cfg_path: &Path, running: &AtomicBool) {
    let hot_reload_timer = unsafe { SetTimer(NO_HWND, 0, HOT_RELOAD_MS, None) };
    let focus_timer      = unsafe { SetTimer(NO_HWND, 0, FOCUS_POLL_MS, None) };
    let display_timer    = unsafe { SetTimer(NO_HWND, 0, DISPLAY_MS, None) };
    let mut cfg_mtime = config::mtime(cfg_path);
    let mut flash = FlashOverlay::new();
    let mut focus_border = FocusBorder::new();
    let mut msg = MSG::default();

    while running.load(Ordering::SeqCst) {
        let ret = unsafe { GetMessageW(&mut msg, NO_HWND, 0, 0) };
        if ret.0 <= 0 { break; }

        match msg.message {
            WM_TIMER => {
                hooks::tick();
                flash.try_expire(msg.wParam.0);

                if msg.wParam.0 == hot_reload_timer {
                    hot_reload(states, cfg_path, &mut cfg_mtime);
                }
                if msg.wParam.0 == display_timer {
                    let focused = unsafe { GetForegroundWindow() };
                    debug::print_status(states, focused);
                }
                if msg.wParam.0 == focus_timer {
                    focus_border.reposition();
                    if let Some(hwnd) = hooks::take_pending_focus() {
                        if window::is_manageable(hwnd) {
                            focus_border.update(hwnd);
                        } else {
                            focus_border.clear();
                        }
                    }
                }
            }
            WM_HOTKEY => on_hotkey(msg.wParam.0 as i32, states, &mut flash, &mut focus_border),
            _ => {}
        }

        unsafe {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }

    unsafe {
        let _ = KillTimer(NO_HWND, hot_reload_timer);
        let _ = KillTimer(NO_HWND, focus_timer);
        let _ = KillTimer(NO_HWND, display_timer);
    }
}

pub fn run_wm() {
    debug::enable_ansi_console();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
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
    }

    overlay::register_class();
    let _state_guard   = hooks::StateGuard::new(&mut states);
    let hook           = hooks::install();
    assert!(!hook.0.is_null(),        "zonewm: failed to install WinEvent move/size hook");
    let focus_hook     = hooks::install_focus();
    assert!(!focus_hook.0.is_null(),  "zonewm: failed to install WinEvent focus hook");
    let minimize_hook  = hooks::install_minimize();
    assert!(!minimize_hook.0.is_null(), "zonewm: failed to install WinEvent minimize hook");
    let kbd_hook       = hooks::install_kbd();
    assert!(!kbd_hook.0.is_null(),    "zonewm: failed to install keyboard hook");

    run(&mut states, &cfg_path, &running);

    let mut persist = config::SavedState::default();
    for ms in states.values() {
        persist.monitor_layouts.insert(ms.monitor_key(), ms.workspace1_layout_idx());
    }
    config::save_state(&config::state_path(), &persist);

    for ms in states.values() { ms.uncloak_all(&Win32System); }
    hooks::uninstall(hook);
    hooks::uninstall(focus_hook);
    hooks::uninstall(minimize_hook);
    hooks::uninstall_kbd(kbd_hook);
}
