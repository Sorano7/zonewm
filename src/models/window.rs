use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, HMONITOR, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongPtrW, GetWindowRect, GetWindowTextW,
    IsIconic, IsWindowVisible, SetWindowPos,
    GWL_EXSTYLE, WS_EX_TOOLWINDOW, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
};
use crate::models::monitor::Rect;
use crate::commands::cloak;

struct FrameOffsets {
    left:   i32,
    top:    i32,
    right:  i32,
    bottom: i32,
}

unsafe fn dwm_frame_offsets(hwnd: HWND) -> FrameOffsets {
    let mut frame = RECT::default();
    let mut total = RECT::default();
    let frame_ok = DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS,
        &mut frame as *mut RECT as *mut _,
        std::mem::size_of::<RECT>() as u32,
    );
    let total_ok = GetWindowRect(hwnd, &mut total);
    if frame_ok.is_err() || total_ok.is_err() {
        return FrameOffsets { left: 0, top: 0, right: 0, bottom: 0 };
    }
    FrameOffsets {
        left:   frame.left   - total.left,
        top:    frame.top    - total.top,
        right:  total.right  - frame.right,
        bottom: total.bottom - frame.bottom,
    }
}

pub const SNAP_GAP: i32 = 2;

pub fn snap_to_rect(hwnd: HWND, zone: &Rect) {
    unsafe {
        let off = dwm_frame_offsets(hwnd);
        let g = SNAP_GAP;
        let _ = SetWindowPos(
            hwnd,
            HWND(std::ptr::null_mut()),
            zone.left   + g - off.left,
            zone.top    + g - off.top,
            zone.width()  - 2 * g + off.left + off.right,
            zone.height() - 2 * g + off.top  + off.bottom,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

pub fn bring_to_top_no_activate(hwnd: HWND) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(debug_assertions)]
pub fn title(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len == 0 { return None; }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn window_rect(hwnd: HWND) -> Option<Rect> {
    unsafe {
        let mut r = RECT::default();
        GetWindowRect(hwnd, &mut r).ok()?;
        Some(r.into())
    }
}

pub fn visible_rect(hwnd: HWND) -> Option<Rect> {
    unsafe {
        let mut r = RECT::default();
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        ).ok()?;
        Some(r.into())
    }
}

pub fn restore_to_rect(hwnd: HWND, rect: &Rect) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND(std::ptr::null_mut()),
            rect.left, rect.top,
            rect.width(), rect.height(),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

struct MonitorFilter {
    target: HMONITOR,
    windows: Vec<HWND>,
}

pub fn enumerate_windows_on_monitor(hmon: HMONITOR) -> Vec<HWND> {
    let mut data = Box::new(MonitorFilter { target: hmon, windows: Vec::new() });
    let ptr = &mut *data as *mut MonitorFilter as isize;
    unsafe { let _ = EnumWindows(Some(monitor_filter_proc), LPARAM(ptr)); }
    data.windows
}

/// Minimum width and height a window must have to be managed.
const MIN_DIMENSION: i32 = 50;

pub fn is_manageable(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() { return false; }
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 { return false; }
        let mut buf = [0u16; 256];
        if GetWindowTextW(hwnd, &mut buf) == 0 { return false; }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() { return false; }
        rect.right - rect.left >= MIN_DIMENSION && rect.bottom - rect.top >= MIN_DIMENSION
    }
}

unsafe extern "system" fn monitor_filter_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if !is_manageable(hwnd) { return BOOL(1); }
    let data = &mut *(lparam.0 as *mut MonitorFilter);
    if MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) != data.target {
        return BOOL(1);
    }
    if cloak::is_cloaked(hwnd) {
        return BOOL(1);
    }
    data.windows.push(hwnd);
    BOOL(1)
}
