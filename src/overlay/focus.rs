use windows::{Win32::{Foundation::HWND, UI::WindowsAndMessaging::{SW_HIDE, SW_SHOW, ShowWindow}}};

use crate::{hooks, models::{monitor::Rect, window}};

pub struct FocusBorder {
    tracked: Option<HWND>,
    border: Option<HWND>,
}

impl FocusBorder {
    pub fn new() -> Self { Self { tracked: None, border: None } }

    /// Begin tracking `hwnd`. Destroys any existing border window first.
    pub fn update(&mut self, hwnd: HWND) {
        self.clear();
        if let Some(rect) = border_rect(hwnd) {
            self.tracked = Some(hwnd);
            self.border = Some(super::create_focus_border(rect));
        }
    }

    /// Reposition the border to follow the tracked window's current rect.
    /// Hides the border while a drag is in progress and shows it again after.
    pub fn reposition(&self) {
        let (Some(tracked), Some(border)) = (self.tracked, self.border) else { return };
        if hooks::is_drag_active() {
            unsafe { let _ = ShowWindow(border, SW_HIDE); }
            return;
        }
        if let Some(rect) = border_rect(tracked) {
            unsafe { let _ = ShowWindow(border, SW_SHOW); }
            super::update_focus_border(border, rect);
        }
    }

    pub fn clear(&mut self) {
        if let Some(b) = self.border.take() { super::close(b); }
        self.tracked = None;
    }
}

/// Returns the visible rect of `hwnd` expanded by `FOCUS_BORDER_WIDTH` on each
/// side. Uses DWM extended frame bounds so the border sits just outside the
/// visible content area, not inside the invisible DWM shadow region.
fn border_rect(hwnd: HWND) -> Option<Rect> {
    let r = window::visible_rect(hwnd)?;
    let p = super::FOCUS_BORDER_WIDTH;
    Some(Rect { left: r.left - p, top: r.top - p, right: r.right + p, bottom: r.bottom + p })
}
