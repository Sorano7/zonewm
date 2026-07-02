use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::{KillTimer, SetTimer}};

use crate::models::monitor::Rect;

pub const FLASH_MS:       u32  = 400;
pub const HOT_RELOAD_MS:  u32  = 1000;
pub const FOCUS_POLL_MS:  u32  = 50;
pub const DISPLAY_MS:     u32  = 500;
pub const MONITOR_POLL_MS: u32 = 1000;
pub const NO_HWND:        HWND = HWND(std::ptr::null_mut());

pub struct FlashOverlay {
    hwnd: Option<HWND>,
    timer_id: Option<usize>,
}

impl FlashOverlay {
    pub fn new() -> Self { Self { hwnd: None, timer_id: None } }

    pub fn show(&mut self, work_area: Rect, zones: Vec<Rect>) {
        self.cancel();
        self.hwnd = Some(super::create_flash(work_area, zones));
        self.timer_id = Some(unsafe { SetTimer(NO_HWND, 0, FLASH_MS, None) });
    }

    pub fn cancel(&mut self) {
        if let Some(ov) = self.hwnd.take() { super::close(ov); }
        if let Some(tid) = self.timer_id.take() { unsafe { let _ = KillTimer(NO_HWND, tid); } }
    }

    pub fn try_expire(&mut self, fired_id: usize) {
        if self.timer_id == Some(fired_id) { self.cancel(); }
    }
}

