use std::cell::{Cell, RefCell};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, KillTimer, PostThreadMessageW, SetTimer,
    SetWindowsHookExW, UnhookWindowsHookEx,
    EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZESTART,
    HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    WM_HOTKEY, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::models::{
    monitor::Rect,
    window,
    system::{Win32System, WindowSystem}
};
use crate::overlay;
use crate::state::{StateMap, window_state::WindowState};

const EVENT_SYSTEM_FOREGROUND:   u32 = 0x0003;
const EVENT_SYSTEM_MINIMIZEEND:  u32 = 0x0017;

/// WM_HOTKEY id bases for layout / workspace / window-to-workspace switching.
pub const LAYOUT_HOT_BASE:       i32 = 1;
pub const WORKSPACE_HOT_BASE:    i32 = 11;
pub const MOVE_HOT_BASE:         i32 = 21;
/// Hotkey IDs for window and focus navigation actions.
pub const FLOAT_HOT_ID:          i32 = 31;
pub const MONITOR_LOCK_HOT_ID:   i32 = 32;
pub const FOCUS_HOT_BASE:        i32 = 33; // +0..3 → Left/Down/Up/Right (hjkl)
pub const WIN_MOVE_HOT_BASE:     i32 = 37; // +0..3
pub const WIN_SWAP_HOT_BASE:     i32 = 41; // +0..3
pub const WIN_CYCLE_NEXT_HOT_ID: i32 = 45;
pub const WIN_CYCLE_PREV_HOT_ID: i32 = 46;

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
    static STATE_PTR:     Cell<*mut StateMap>        = const { Cell::new(std::ptr::null_mut()) };
    static DRAG:          RefCell<Option<DragState>> = const { RefCell::new(None) };
    static MAIN_TID:      Cell<u32>                  = const { Cell::new(0) };
    static PENDING_FOCUS: Cell<Option<HWND>>         = const { Cell::new(None) };
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

pub fn take_pending_focus() -> Option<HWND> {
    PENDING_FOCUS.with(|c| c.take())
}

pub fn is_drag_active() -> bool {
    DRAG.with(|d| d.borrow().is_some())
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

fn hjkl_dir(vk: u32) -> Option<usize> {
    match vk {
        0x48 => Some(0), // H → Left
        0x4A => Some(1), // J → Down
        0x4B => Some(2), // K → Up
        0x4C => Some(3), // L → Right
        _ => None,
    }
}

fn post_hotkey(id: i32) {
    let tid = MAIN_TID.with(|t| t.get());
    unsafe { let _ = PostThreadMessageW(tid, WM_HOTKEY, WPARAM(id as usize), LPARAM(0)); }
}

unsafe extern "system" fn kbd_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && (wp.0 == WM_KEYDOWN as usize || wp.0 == WM_SYSKEYDOWN as usize) {
        let info = &*(lp.0 as *const KBDLLHOOKSTRUCT);
        let vk    = info.vkCode;
        let ctrl  = GetKeyState(VK_CONTROL.0 as i32) < 0;
        let shift = GetKeyState(VK_SHIFT.0 as i32)   < 0;
        let alt   = GetKeyState(VK_MENU.0 as i32)    < 0;

        // layout, workspace, move-to-ws
        if (0x31..=0x39).contains(&vk) {
            let digit = (vk - 0x30) as i32;
            let id = if ctrl && !shift && alt {
                Some(LAYOUT_HOT_BASE    + (digit - 1))
            } else if alt && !ctrl && !shift {
                Some(WORKSPACE_HOT_BASE + (digit - 1))
            } else if !ctrl && alt && shift {
                Some(MOVE_HOT_BASE      + (digit - 1))
            } else {
                None
            };
            if let Some(id) = id {
                post_hotkey(id);
                return LRESULT(1);
            }
        }

        // float the focused zoned window
        if vk == 0x46 && alt && shift && !ctrl {
            post_hotkey(FLOAT_HOT_ID);
            return LRESULT(1);
        }

        // toggle monitor lock
        if vk == 0x47 && alt && !shift && !ctrl {
            post_hotkey(MONITOR_LOCK_HOT_ID);
            return LRESULT(1);
        }

        // HJKL navigation family
        if let Some(dir) = hjkl_dir(vk) {
            let id = if alt && !ctrl && !shift {
                Some(FOCUS_HOT_BASE    + dir as i32)
            } else if alt && shift && !ctrl {
                Some(WIN_MOVE_HOT_BASE + dir as i32)
            } else if ctrl && alt && !shift {
                Some(WIN_SWAP_HOT_BASE + dir as i32)
            } else {
                None
            };
            if let Some(id) = id {
                post_hotkey(id);
                return LRESULT(1);
            }
        }

        // cycle within zone
        if alt && !ctrl && !shift {
            if vk == 0x4E { post_hotkey(WIN_CYCLE_NEXT_HOT_ID); return LRESULT(1); }
            if vk == 0x50 { post_hotkey(WIN_CYCLE_PREV_HOT_ID); return LRESULT(1); }
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
    _id_object: i32,
    _id_child: i32,
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
