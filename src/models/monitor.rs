use windows::core::HSTRING;
use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW,
    HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::EDD_GET_DEVICE_INTERFACE_NAME;

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

#[derive(Debug, Clone)]
pub struct Monitor {
    pub handle: HMONITOR,
    pub work_area: Rect,
    /// GDI adapter name (e.g. `\\.\DISPLAY1`). Reassigned by Windows across
    /// topology changes, so only useful for matching within one enumeration.
    pub device_name: String,
    /// EDID-derived hardware identity, stable across replugs/reboots. Falls
    /// back to `device_name` when no device interface is available (e.g.
    /// virtual/RDP displays).
    pub device_id: String,
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
    let mut info: MONITORINFOEXW = std::mem::zeroed();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut MONITORINFO).as_bool() {
        let device_name = wide_to_string(&info.szDevice);
        let device_id = device_id_for(&device_name).unwrap_or_else(|| device_name.clone());
        monitors.push(Monitor {
            handle: hmonitor,
            work_area: info.monitorInfo.rcWork.into(),
            device_name,
            device_id,
        });
    }
    BOOL(1)
}

fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}

/// Looks up the EDID-backed device interface path for the monitor attached
/// to `adapter_device_name` (e.g. `\\.\DISPLAY1`). This ID stays stable
/// across the monitor being unplugged/replugged into a different port.
fn device_id_for(adapter_device_name: &str) -> Option<String> {
    let mut dd = DISPLAY_DEVICEW { cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
    let ok = unsafe {
        EnumDisplayDevicesW(&HSTRING::from(adapter_device_name), 0, &mut dd, EDD_GET_DEVICE_INTERFACE_NAME)
    };
    if !ok.as_bool() {
        return None;
    }
    let id = wide_to_string(&dd.DeviceID);
    if id.is_empty() { None } else { Some(id) }
}
