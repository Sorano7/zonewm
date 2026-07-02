use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_COLOR_DEFAULT, DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD,
};
use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

use crate::models::system::{Win32System, WindowSystem};
use crate::models::{
    window,
    monitor::Rect,
};
use crate::state::StateMap;
use crate::state::window_state::{Direction, WindowState, nearest_in_dir};

fn set_dwm_attr<T>(hwnd: HWND, attr: DWMWINDOWATTRIBUTE, value: T) {
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd, 
            attr, 
            std::ptr::from_ref(&value).cast(), 
            std::mem::size_of::<u32>() as u32,
            );
    }
}

pub fn set_window_border(hwnd: HWND, bgr: COLORREF) {
    set_dwm_attr(hwnd, DWMWA_BORDER_COLOR, bgr);
}

pub fn clear_window_border(hwnd: HWND) {
    set_dwm_attr(hwnd, DWMWA_BORDER_COLOR, COLORREF(DWMWA_COLOR_DEFAULT));
}

pub fn set_foreground_window(hwnd: HWND) {
    unsafe {
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: std::mem::zeroed() },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        let _ = SetForegroundWindow(hwnd);
    }
}

pub fn handle_focus_move(
    focused: HWND,
    mon_key: isize,
    dir: Direction,
    states: &StateMap,
) {
    let focused_rect = match window::visible_rect(focused) {
        Some(r) => r,
        None => return,
    };

    let candidates: Vec<(HWND, Rect)> = states.iter()
        .filter(|(&k, ms)| k == mon_key || !ms.monitor_locked)
        .flat_map(|(_, ms)| {
            ms.zoned_focus_candidates(&Win32System).into_iter()
                .chain(ms.floating_focus_candidates(&Win32System))
        })
        .filter(|&(h, _)| h != focused)
        .collect();

    if let Some(target) = nearest_in_dir(&candidates, focused_rect, dir) {
        set_foreground_window(target);
        clear_window_border(focused);
    }
}

pub fn handle_window_move(
    focused: HWND,
    mon_key: isize,
    dir: Direction,
    states: &mut StateMap,
) {
    let focused_rect = match window::visible_rect(focused) {
        Some(r) => r,
        None => return,
    };

    let zone_entries: Vec<((isize, usize), Rect)> = states.iter()
        .filter(|(&k, ms)| k == mon_key || !ms.monitor_locked)
        .flat_map(|(&k, ms)| {
            ms.active_zone_rects().into_iter().enumerate().map(move |(zi, r)| ((k, zi), r))
        })
        .collect();

    let Some((dst_key, dst_zone)) = nearest_in_dir(&zone_entries, focused_rect, dir) else { return };

    if dst_key == mon_key {
        if let Some(ms) = states.get_mut(&mon_key) {
            ms.move_window_to_zone_idx(focused, dst_zone, &Win32System);
        }
    } else {
        // Cross-monitor: detach from source, insert into destination.
        if let Some(src) = states.get_mut(&mon_key) {
            src.detach_window(focused);
        }
        if let Some(dst) = states.get_mut(&dst_key) {
            dst.move_window_to_zone_idx(focused, dst_zone, &Win32System);
        }
    }
}

pub fn handle_window_swap(
    focused: HWND,
    mon_key: isize,
    dir: Direction,
    states: &mut StateMap,
) {
    let src_state = states.get(&mon_key)
        .map(|ms| ms.window_state(focused))
        .unwrap_or(WindowState::Ignored);
    let WindowState::Zoned(src_zone) = src_state else { return };

    let focused_rect = match window::visible_rect(focused) {
        Some(r) => r,
        None => return,
    };

    let zone_entries: Vec<((isize, usize), Rect)> = states.iter()
        .filter(|(&k, ms)| k == mon_key || !ms.monitor_locked)
        .flat_map(|(&k, ms)| {
            ms.active_zone_rects().into_iter().enumerate().map(move |(zi, r)| ((k, zi), r))
        })
        .collect();

    let Some((dst_key, dst_zone)) = nearest_in_dir(&zone_entries, focused_rect, dir) else { return };

    let swap_partner: Option<HWND> = states.get(&dst_key)
        .and_then(|ms| ms.topmost_in_zone(dst_zone, &Win32System));

    if dst_key == mon_key {
        if let Some(ms) = states.get_mut(&mon_key) {
            if let Some(partner) = swap_partner {
                ms.move_window_to_zone_idx(partner, src_zone, &Win32System);
                Win32System.bring_to_front(partner);
            }
            ms.move_window_to_zone_idx(focused, dst_zone, &Win32System);
        }
    } else {
        // Cross-monitor swap: two sequential passes to avoid aliasing.
        if let Some(partner) = swap_partner {
            if let Some(dst_ms) = states.get_mut(&dst_key) {
                dst_ms.detach_window(partner);
            }
            if let Some(src_ms) = states.get_mut(&mon_key) {
                src_ms.move_window_to_zone_idx(partner, src_zone, &Win32System);
                Win32System.bring_to_front(partner);
            }
        }
        if let Some(src_ms) = states.get_mut(&mon_key) {
            src_ms.detach_window(focused);
        }
        if let Some(dst_ms) = states.get_mut(&dst_key) {
            dst_ms.move_window_to_zone_idx(focused, dst_zone, &Win32System);
        }
    }
}

pub fn handle_cycle(
    focused: HWND,
    mon_key: isize,
    forward: bool,
    states: &mut StateMap,
) {
    if let Some(ms) = states.get_mut(&mon_key) {
        if let Some(target) = ms.cycle_window_in_zone(focused, forward, &Win32System) {
            set_foreground_window(target);
            clear_window_border(focused);
        }
    }
}
