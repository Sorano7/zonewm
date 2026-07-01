pub mod flash;
pub mod focus;

use std::cell::RefCell;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, FrameRect,
    InvalidateRect, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, RegisterClassExW,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WNDCLASSEXW,
    CS_HREDRAW, CS_VREDRAW, LWA_ALPHA, LWA_COLORKEY, SW_SHOW, WM_PAINT,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    SWP_NOACTIVATE, SWP_NOZORDER,
};
use windows::core::{w, PCWSTR};

use crate::models::monitor::Rect;

const CLASS:         PCWSTR   = w!("ZoneWM_Overlay");
const FOCUS_CLASS:   PCWSTR   = w!("ZoneWM_FocusBorder");
const KEY:           COLORREF = COLORREF(0x00010101);
const BORDER:        COLORREF = COLORREF(0x00FFFFFF);

pub const FOCUS_BORDER_WIDTH: i32 = 2;

fn accent_colorref() -> COLORREF {
    (|| -> windows::core::Result<COLORREF> {
        let settings = UISettings::new()?;
        let c = settings.GetColorValue(UIColorType::Accent)?;
        // UISettings returns Windows.UI.Color { A, R, G, B }; COLORREF is 0x00BBGGRR
        Ok(COLORREF((c.B as u32) << 16 | (c.G as u32) << 8 | c.R as u32))
    })()
    .unwrap_or(COLORREF(0x00000000))
}

struct DrawData {
    origin: (i32, i32),
    zones: Vec<Rect>,
    highlighted: Option<usize>,
    fill: COLORREF,
    fill_all: bool,
}
impl Default for DrawData {
    fn default() -> Self {
        Self { origin: (0, 0), zones: Vec::new(), highlighted: None, fill: COLORREF(0), fill_all: false }
    }
}

thread_local! {
    static DRAW: RefCell<DrawData> = RefCell::new(DrawData::default());
}

struct FocusDrawData { color: COLORREF }

thread_local! {
    static FOCUS_DRAW: RefCell<FocusDrawData> = RefCell::new(FocusDrawData { color: COLORREF(0) });
}

fn hmod() -> HINSTANCE {
    unsafe {
        let m = GetModuleHandleW(PCWSTR(std::ptr::null())).unwrap_or_default();
        HINSTANCE(m.0)
    }
}

pub fn register_class() {
    unsafe {
        let base = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            hInstance: hmod(),
            ..Default::default()
        };
        RegisterClassExW(&WNDCLASSEXW {
            lpfnWndProc: Some(wnd_proc), lpszClassName: CLASS, ..base
        });
        RegisterClassExW(&WNDCLASSEXW {
            lpfnWndProc: Some(focus_wnd_proc), lpszClassName: FOCUS_CLASS, ..base
        });
    }
}

fn make_window(class: PCWSTR, rect: Rect, alpha: u8) -> HWND {
    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            class,
            PCWSTR(std::ptr::null()),
            WS_POPUP,
            rect.left,
            rect.top,
            rect.width(),
            rect.height(),
            HWND(std::ptr::null_mut()),
            None,
            hmod(),
            None,
        ).unwrap_or_default();
        let _ = SetLayeredWindowAttributes(hwnd, KEY, alpha, LWA_COLORKEY | LWA_ALPHA);
        let _ = ShowWindow(hwnd, SW_SHOW);
        hwnd
    }
}

pub fn create(work_area: Rect, zones: Vec<Rect>, highlighted: Option<usize>) -> HWND {
    DRAW.with(|d| {
        let mut d = d.borrow_mut();
        d.origin = (work_area.left, work_area.top);
        d.zones = zones;
        d.highlighted = highlighted;
        d.fill = accent_colorref();
        d.fill_all = false;
    });
    make_window(CLASS, work_area, 180)
}

pub fn create_flash(work_area: Rect, zones: Vec<Rect>) -> HWND {
    DRAW.with(|d| {
        let mut d = d.borrow_mut();
        d.origin = (work_area.left, work_area.top);
        d.zones = zones;
        d.highlighted = None;
        d.fill = accent_colorref();
        d.fill_all = true;
    });
    make_window(CLASS, work_area, 90)
}

pub fn set_highlighted(hwnd: HWND, highlighted: Option<usize>) {
    DRAW.with(|d| d.borrow_mut().highlighted = highlighted);
    unsafe { let _ = InvalidateRect(hwnd, None, true); }
}

pub fn close(hwnd: HWND) {
    unsafe { let _ = DestroyWindow(hwnd); }
}

pub fn create_focus_border(rect: Rect) -> HWND {
    FOCUS_DRAW.with(|d| d.borrow_mut().color = COLORREF(0x00FFA269));
    make_window(FOCUS_CLASS, rect, 255)
}

pub fn update_focus_border(border: HWND, rect: Rect) {
    unsafe {
        let _ = SetWindowPos(
            border,
            HWND(std::ptr::null_mut()),
            rect.left, rect.top, rect.width(), rect.height(),
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM,
) -> LRESULT {
    if msg == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);

        let mut cr = RECT::default();
        let _ = GetClientRect(hwnd, &mut cr);

        let fill_color = DRAW.with(|d| d.borrow().fill);
        let key_br    = CreateSolidBrush(KEY);
        let border_br = CreateSolidBrush(BORDER);
        let fill_br   = CreateSolidBrush(fill_color);

        FillRect(hdc, &cr, key_br);

        DRAW.with(|d| {
            let d = d.borrow();
            let (ox, oy) = d.origin;
            for (i, zone) in d.zones.iter().enumerate() {
                let r = RECT {
                    left:   zone.left   - ox,
                    top:    zone.top    - oy,
                    right:  zone.right  - ox,
                    bottom: zone.bottom - oy,
                };
                if d.fill_all || d.highlighted == Some(i) {
                    FillRect(hdc, &r, fill_br);
                }
                FrameRect(hdc, &r, border_br);
            }
        });

        let _ = DeleteObject(key_br);
        let _ = DeleteObject(border_br);
        let _ = DeleteObject(fill_br);

        let _ = EndPaint(hwnd, &ps);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

unsafe extern "system" fn focus_wnd_proc(
    hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM,
) -> LRESULT {
    if msg == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);

        let mut cr = RECT::default();
        let _ = GetClientRect(hwnd, &mut cr);

        let color  = FOCUS_DRAW.with(|d| d.borrow().color);
        let acc_br = CreateSolidBrush(color);
        let key_br = CreateSolidBrush(KEY);

        // Fill the entire frame with accent color, then punch out the center
        FillRect(hdc, &cr, acc_br);
        let inner = RECT {
            left:   FOCUS_BORDER_WIDTH,
            top:    FOCUS_BORDER_WIDTH,
            right:  cr.right  - FOCUS_BORDER_WIDTH,
            bottom: cr.bottom - FOCUS_BORDER_WIDTH,
        };
        if inner.right > inner.left && inner.bottom > inner.top {
            FillRect(hdc, &inner, key_br);
        }

        let _ = DeleteObject(acc_br);
        let _ = DeleteObject(key_br);
        let _ = EndPaint(hwnd, &ps);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

