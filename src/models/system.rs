use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::UI::WindowsAndMessaging::IsIconic;

use crate::models::monitor::Rect;
use crate::models::window;
use crate::commands::cloak;

pub trait WindowSystem {
    fn snap_window(&self, hwnd: HWND, rect: &Rect);
    fn restore_window_size(&self, hwnd: HWND, rect: &Rect);
    fn set_cloak(&self, hwnd: HWND, cloaked: bool);
    fn forget_cloak_view(&self, hwnd: HWND);
    fn enumerate_on_monitor(&self, hmon: HMONITOR) -> Vec<HWND>;
    fn is_minimized(&self, hwnd: HWND) -> bool;
    fn window_rect(&self, hwnd: HWND) -> Option<Rect>;
    fn bring_to_front(&self, hwnd: HWND);
}

pub struct Win32System;

impl WindowSystem for Win32System {
    fn snap_window(&self, hwnd: HWND, rect: &Rect) {
        window::snap_to_rect(hwnd, rect);
    }

    fn restore_window_size(&self, hwnd: HWND, rect: &Rect) {
        window::restore_size_from_rect(hwnd, rect);
    }

    fn set_cloak(&self, hwnd: HWND, cloaked: bool) {
        cloak::set_cloak(hwnd, cloaked);
    }

    fn forget_cloak_view(&self, hwnd: HWND) {
        cloak::forget(hwnd);
    }

    fn enumerate_on_monitor(&self, hmon: HMONITOR) -> Vec<HWND> {
        window::enumerate_windows_on_monitor(hmon)
    }

    fn is_minimized(&self, hwnd: HWND) -> bool {
        unsafe { IsIconic(hwnd).as_bool() }
    }

    fn window_rect(&self, hwnd: HWND) -> Option<Rect> {
        window::window_rect(hwnd)
    }

    fn bring_to_front(&self, hwnd: HWND) {
        window::bring_to_top_no_activate(hwnd);
    }
}
