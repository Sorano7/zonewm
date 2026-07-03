use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, KillTimer, PostThreadMessageW, SetTimer,
    SetWindowsHookExW, UnhookWindowsHookEx,
    EVENT_OBJECT_DESTROY, EVENT_OBJECT_SHOW, EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZESTART,
    HHOOK, KBDLLHOOKSTRUCT, OBJID_WINDOW, WH_KEYBOARD_LL,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    WM_HOTKEY, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::actions::keymap::{Keymap, KeymapRegistry};
use crate::models::{
    monitor::Rect,
    window,
    system::{Win32System, WindowSystem}
};
use crate::overlay;
use crate::state::{StateMap, window_state::WindowState};

const EVENT_SYSTEM_FOREGROUND:   u32 = 0x0003;
const EVENT_SYSTEM_MINIMIZEEND:  u32 = 0x0017;

const TIMER_ID: usize = 1;

struct DragState {
    dragged: HWND,
    pre_drag_rect: Rect,
    overlay_hwnd: Option<HWND>,
    zones: Vec<Rect>,
    monitor_key: isize,
    highlighted: Option<usize>,
}

thread_local! {
    static STATE_PTR:      Cell<*mut StateMap>        = const { Cell::new(std::ptr::null_mut()) };
    static DRAG:           RefCell<Option<DragState>> = const { RefCell::new(None) };
    static MAIN_TID:       Cell<u32>                  = const { Cell::new(0) };
    static PENDING_FOCUS:  Cell<Option<HWND>>         = const { Cell::new(None) };
    pub static KEYMAP_REG: OnceLock<KeymapRegistry>   = const { OnceLock::new() };
}

pub struct StateGuard;

impl StateGuard {
    pub fn new(state: &mut StateMap) -> Self {
        STATE_PTR.with(|p| p.set(state as *mut _));
        Self
    }
}

impl Drop for StateGuard {
    fn drop(&mut self) {
        STATE_PTR.with(|p| p.set(std::ptr::null_mut()));
    }
}

pub fn install() -> HWINEVENTHOOK {
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_MOVESIZESTART,
            EVENT_SYSTEM_MOVESIZEEND,
            None,
            Some(win_event_proc),
            0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    }
}

pub fn uninstall(hook: HWINEVENTHOOK) {
    if !hook.0.is_null() {
        unsafe { let _ = UnhookWinEvent(hook); }
    }
}

pub fn install_focus() -> HWINEVENTHOOK {
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    }
}

pub fn install_minimize() -> HWINEVENTHOOK {
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_MINIMIZEEND,
            EVENT_SYSTEM_MINIMIZEEND,
            None,
            Some(win_event_proc),
            0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    }
}

pub fn install_destroy() -> HWINEVENTHOOK {
    unsafe {
        SetWinEventHook(
            EVENT_OBJECT_DESTROY,
            EVENT_OBJECT_DESTROY,
            None,
            Some(win_event_proc),
            0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    }
}

pub fn install_show() -> HWINEVENTHOOK {
    unsafe {
        SetWinEventHook(
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_SHOW,
            None,
            Some(win_event_proc),
            0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    }
}

pub fn install_kbd() -> HHOOK {
    unsafe {
        MAIN_TID.with(|t| t.set(GetCurrentThreadId()));
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(kbd_proc), None, 0)
            .unwrap_or_default()
    }
}

pub fn uninstall_kbd(hook: HHOOK) {
    if !hook.0.is_null() {
        unsafe { let _ = UnhookWindowsHookEx(hook); }
    }
}

fn post_hotkey(id: i32) {
    let tid = MAIN_TID.with(|t| t.get());
    unsafe { let _ = PostThreadMessageW(tid, WM_HOTKEY, WPARAM(id as usize), LPARAM(0)); }
}

unsafe extern "system" fn kbd_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && (wp.0 == WM_KEYDOWN as usize || wp.0 == WM_SYSKEYDOWN as usize) {
        let ctrl  = GetKeyState(VK_CONTROL.0 as i32) < 0;
        let shift = GetKeyState(VK_SHIFT.0 as i32)   < 0;
        let alt   = GetKeyState(VK_MENU.0 as i32)    < 0;
        let win   = GetKeyState(VK_LWIN.0 as i32)    < 0;

        let mut mods = 0u32;
        if ctrl  { mods |= MOD_CONTROL.0; }
        if alt   { mods |= MOD_ALT.0; }
        if shift { mods |= MOD_SHIFT.0; }
        if win   { mods |= MOD_WIN.0; }

        let info = &*(lp.0 as *const KBDLLHOOKSTRUCT);

        if let Some(id) = KEYMAP_REG.with(|r| {
            r.get()?.get_id_from_keymap(Keymap { mods: mods, vk: info.vkCode })
        }) {
            post_hotkey(id);
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wp, lp)
}

pub fn tick() {
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };

    DRAG.with(|d| {
        let mut opt = d.borrow_mut();
        let ds = match opt.as_mut() {
            Some(ds) => ds,
            None => return,
        };

        if shift {
            unsafe {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
                let key = hmon.0 as isize;

                // Migrate overlay when cursor crosses to a different monitor.
                if ds.overlay_hwnd.is_some() && ds.monitor_key != key {
                    if let Some(ov) = ds.overlay_hwnd.take() {
                        overlay::close(ov);
                        ds.highlighted = None;
                    }
                }

                if ds.overlay_hwnd.is_none() {
                    let ptr = STATE_PTR.with(|p| p.get());
                    if !ptr.is_null() {
                        if let Some(ms) = (*ptr).get(&key) {
                            if let Some(layout) = ms.active_layout() {
                                let zones: Vec<Rect> = layout.zones.iter()
                                    .map(|z| z.to_rect(ms.monitor.work_area))
                                    .collect();
                                let ov = overlay::create(ms.monitor.work_area, zones.clone(), None);
                                ds.overlay_hwnd = Some(ov);
                                ds.zones = zones;
                                ds.monitor_key = key;
                            }
                        }
                    }
                }

                if let Some(ov) = ds.overlay_hwnd {
                    let hl = ds.zones.iter().position(|r| {
                        pt.x >= r.left && pt.x < r.right
                            && pt.y >= r.top && pt.y < r.bottom
                    });
                    if hl != ds.highlighted {
                        ds.highlighted = hl;
                        overlay::set_highlighted(ov, hl);
                    }
                }
            }
        } else if let Some(ov) = ds.overlay_hwnd.take() {
            overlay::close(ov);
            ds.highlighted = None;
        }
    });
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    match event {
        EVENT_SYSTEM_FOREGROUND => {
            PENDING_FOCUS.with(|c| c.set(Some(hwnd)));

            // If the focused window belongs to a non-active workspace, switch to it.
            let ptr = STATE_PTR.with(|p| p.get());
            if ptr.is_null() { return; }
            let states = &mut *ptr;
            for ms in states.values_mut() {
                if let Some(ws_idx) = ms.find_workspace(hwnd) {
                    if ws_idx != ms.active_ws {
                        ms.switch_workspace(ws_idx, &Win32System);
                    }
                    return;
                }
            }
        }

        EVENT_SYSTEM_MINIMIZEEND => {
            let ptr = STATE_PTR.with(|p| p.get());
            if ptr.is_null() { return; }
            for ms in (*ptr).values_mut() {
                ms.on_window_restored(hwnd, &Win32System);
            }
        }

        EVENT_OBJECT_SHOW => {
            if id_object != OBJID_WINDOW.0 || id_child != 0 { return; }

            let ptr = STATE_PTR.with(|p| p.get());
            if ptr.is_null() { return; }
            let mon_key = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST).0 as isize;
            if let Some(ms) = (*ptr).get_mut(&mon_key) {
                ms.capture_all_windows(&Win32System);
            }
        }

        EVENT_OBJECT_DESTROY => {
            if id_object != OBJID_WINDOW.0 || id_child != 0 { return; }

            let ptr = STATE_PTR.with(|p| p.get());
            if ptr.is_null() { return; }
            for ms in (*ptr).values_mut() {
                let Some(ws_idx) = ms.find_workspace(hwnd) else { continue };
                let was_zoned = matches!(ms.window_state(hwnd), WindowState::Zoned(_));
                ms.detach_window(hwnd);
                if was_zoned && ws_idx == ms.active_ws {
                    ms.reflow(&Win32System);
                }
                break;
            }
        }

        EVENT_SYSTEM_MOVESIZESTART => {
            let pre_drag_rect = window::window_rect(hwnd).unwrap_or_default();
            DRAG.with(|d| {
                *d.borrow_mut() = Some(DragState {
                    dragged: hwnd,
                    pre_drag_rect,
                    overlay_hwnd: None,
                    zones: Vec::new(),
                    monitor_key: 0,
                    highlighted: None,
                });
            });
            SetTimer(HWND(std::ptr::null_mut()), TIMER_ID, 16, None);
        }

        EVENT_SYSTEM_MOVESIZEEND => {
            let _ = KillTimer(HWND(std::ptr::null_mut()), TIMER_ID);
            let Some(ds) = DRAG.with(|d| d.borrow_mut().take()) else { return };

            if let Some(ov) = ds.overlay_hwnd {
                overlay::close(ov);
            }

            let ptr = STATE_PTR.with(|p| p.get());
            if ptr.is_null() { return; }

            if let Some(idx) = ds.highlighted {
                Win32System.snap_window(ds.dragged, &ds.zones[idx]);
                let states = &mut *ptr;
                // Detach from the source monitor before assigning to the destination,
                // so cross-monitor drags don't leave a ghost entry on the source.
                for (&k, ms) in states.iter_mut() {
                    if k != ds.monitor_key && ms.find_workspace(ds.dragged).is_some() {
                        ms.detach_window(ds.dragged);
                        break;
                    }
                }
                if let Some(ms) = states.get_mut(&ds.monitor_key) {
                    ms.assign_to_zone(idx, ds.dragged, ds.pre_drag_rect);
                }
            } else {
                let states = &mut *ptr;
                let ms_opt = states.values_mut()
                    .find(|ms| matches!(ms.window_state(ds.dragged), WindowState::Zoned(_)));
                if let Some(ms) = ms_opt {
                    let cur_rect = window::window_rect(ds.dragged).unwrap_or_default();
                    let resized = cur_rect.width()  != ds.pre_drag_rect.width()
                               || cur_rect.height() != ds.pre_drag_rect.height();
                    if resized {
                        ms.set_floating_in_place(ds.dragged);
                    } else {
                        ms.set_floating(ds.dragged, &Win32System);
                    }
                }
            }
        }

        _ => {}
    }
}
