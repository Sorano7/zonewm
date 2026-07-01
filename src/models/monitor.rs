use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
}

impl From<RECT> for Rect {
    fn from(r: RECT) -> Self {
        Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
    }
}

#[derive(Debug)]
pub struct Monitor {
    pub handle: HMONITOR,
    pub work_area: Rect,
}

pub fn enumerate_monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();
    let ptr = &mut monitors as *mut Vec<Monitor> as isize;
    unsafe {
        let _ = EnumDisplayMonitors(HDC(std::ptr::null_mut()), None, Some(monitor_enum_proc), LPARAM(ptr));
    }
    monitors
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<Monitor>);
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
        monitors.push(Monitor { handle: hmonitor, work_area: info.rcWork.into() });
    }
    BOOL(1)
}
