use windows::Win32::Foundation::HWND;

use crate::{commands, models::{system::Win32System}, overlay::flash::FlashOverlay, state::{StateMap, window_state::{Direction}}};

pub mod hooks;
pub mod keymap;

pub struct ActionCtx<'a> {
    pub states: &'a mut StateMap,
    pub mon_key: isize,
    pub focused: HWND,
    pub flash: &'a mut FlashOverlay,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Action {
    SetLayout(usize),
    SetWorkspace(usize),
    WinMoveWS(usize),
    ToggleFloat,
    ToggleMonLock,
    WinCycle(bool),
    WinFocus(Direction),
    WinMove(Direction),
    WinSwap(Direction),
    WinStretch(Direction),
    WinShrink(Direction),
    WinFullscreen,
    WinMinimize,
}

impl Action {
    pub fn to_string(&self) -> String {
        match self {
            Action::SetLayout(idx)    => format!("set_layout_{}", idx),
            Action::SetWorkspace(idx) => format!("set_workspace_{}", idx),
            Action::WinMoveWS(idx)    => format!("move_to_workspace_{}", idx),
            Action::ToggleFloat       => "toggle_float".into(),
            Action::ToggleMonLock     => "toggle_monitor_lock".into(),
            Action::WinCycle(fw)      => format!("cycle_window_{}", if *fw {"next"} else {"prev"}),
            Action::WinFocus(dir)     => format!("move_focus_{}", dir.to_string()),
            Action::WinMove(dir)      => format!("move_window_{}", dir.to_string()),
            Action::WinSwap(dir)      => format!("swap_window_{}", dir.to_string()),
            Action::WinStretch(dir)   => format!("stretch_window_{}", dir.to_string()),
            Action::WinShrink(dir)    => format!("shrink_window_{}", dir.to_string()),
            Action::WinFullscreen     => "set_fullscreen".into(),
            Action::WinMinimize       => "set_minimized".into(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        if s == "toggle_float" {
            Some(Action::ToggleFloat)
        } else if s == "toggle_monitor_lock" {
            Some(Action::ToggleMonLock)
        } else if s == "set_fullscreen" {
            Some(Action::WinFullscreen)
        } else if s == "set_minimized" {
            Some(Action::WinMinimize)
        } else if let Some(rest) = s.strip_prefix("set_layout_") {
            let idx = rest.parse::<usize>().ok()? - 1;
            Some(Action::SetLayout(idx))
        } else if let Some(rest) = s.strip_prefix("set_workspace_") {
            let idx = rest.parse::<usize>().ok()? - 1;
            Some(Action::SetWorkspace(idx))
        } else if let Some(rest) = s.strip_prefix("move_to_workspace_") {
            let idx = rest.parse::<usize>().ok()? - 1;
            Some(Action::WinMoveWS(idx))
        } else if let Some(rest) = s.strip_prefix("cycle_window_") {
            match rest {
                "next" => Some(Action::WinCycle(true)),
                "prev" => Some(Action::WinCycle(false)),
                _ => None,
            }
        } else if let Some(rest) = s.strip_prefix("move_focus_") {
            let dir = Direction::from_string(rest)?;
            Some(Action::WinFocus(dir))
        } else if let Some(rest) = s.strip_prefix("move_window_") {
            let dir = Direction::from_string(rest)?;
            Some(Action::WinMove(dir))
        } else if let Some(rest) = s.strip_prefix("swap_window_") {
            let dir = Direction::from_string(rest)?;
            Some(Action::WinSwap(dir))
        } else if let Some(rest) = s.strip_prefix("stretch_window_") {
            let dir = Direction::from_string(rest)?;
            Some(Action::WinStretch(dir))
        } else if let Some(rest) = s.strip_prefix("shrink_window_") {
            let dir = Direction::from_string(rest)?;
            Some(Action::WinShrink(dir))
        } else {
            None
        }
    }

    pub fn execute(&self, ctx: &mut ActionCtx) {
        match self {
            Action::SetLayout(idx)    => self.set_layout(*idx, ctx),
            Action::SetWorkspace(idx) => self.set_workspace(*idx, ctx), 
            Action::WinMoveWS(idx)    => self.move_window_ws(*idx, ctx), 
            Action::ToggleFloat       => self.toggle_float(ctx), 
            Action::ToggleMonLock     => self.toggle_monitor_lock(ctx), 
            Action::WinCycle(fw)      => self.cycle_window(*fw, ctx),
            Action::WinFocus(dir)     => self.focus_window(*dir, ctx),
            Action::WinMove(dir)      => self.move_window(*dir, ctx),
            Action::WinSwap(dir)      => self.swap_window(*dir, ctx),
            Action::WinStretch(dir)   => self.stretch_window(*dir, ctx),
            Action::WinShrink(dir)    => self.shrink_window(*dir, ctx),
            Action::WinFullscreen     => self.set_fullscreen(ctx),
            Action::WinMinimize       => self.set_minimize(ctx),
        }
    }

    fn set_layout(&self, idx: usize, ctx: &mut ActionCtx) {
        let Some(ms) = ctx.states.get_mut(&ctx.mon_key) else {
            return;
        };

        ms.switch_layout(idx);
        ms.reflow(&Win32System);
        if let Some(layout) = ms.active_layout() {
            let zones = layout.zones.iter()
                .map(|z| z.to_rect(ms.monitor.work_area))
                .collect();
            ctx.flash.show(ms.monitor.work_area, zones);
        } else {
            ctx.flash.cancel();
        }
    }

    fn set_workspace(&self, idx: usize, ctx: &mut ActionCtx) {
        let Some(ms) = ctx.states.get_mut(&ctx.mon_key) else {
            return;
        };

        ms.switch_workspace(idx, &Win32System);
        if let Some(hwnd) = ms.get_last_focused_window(&Win32System) {
            commands::window::set_foreground_window(hwnd);
        }
    }

    fn toggle_float(&self, ctx: &mut ActionCtx) {
        let Some(ms) = ctx.states.get_mut(&ctx.mon_key) else {
            return;
        };

        if ms.is_floating(ctx.focused) {
            ms.snap_overlapping(ctx.focused, &Win32System);
        } else {
            ms.set_floating(ctx.focused, &Win32System);
        }
    }

    fn move_window_ws(&self, idx: usize, ctx: &mut ActionCtx) {
        let Some(ms) = ctx.states.get_mut(&ctx.mon_key) else {
            return;
        };

        ms.move_window_to_workspace(ctx.focused, idx, &Win32System);
        ms.switch_workspace(idx, &Win32System);
    }

    fn toggle_monitor_lock(&self, ctx: &mut ActionCtx) {
        for (_, s) in ctx.states.iter_mut() {
            s.monitor_locked = !s.monitor_locked;
        }
    }

    fn cycle_window(&self, fw: bool, ctx: &mut ActionCtx) {
        commands::window::handle_cycle(ctx.focused, fw);
    }

    fn focus_window(&self, dir: Direction, ctx: &mut ActionCtx) {
        commands::window::handle_focus_move(ctx.focused, ctx.mon_key, dir, ctx.states);
    }

    fn move_window(&self, dir: Direction, ctx: &mut ActionCtx) {
        commands::window::handle_window_move(ctx.focused, ctx.mon_key, dir, ctx.states);
    }

    fn swap_window(&self, dir: Direction, ctx: &mut ActionCtx) {
        commands::window::handle_window_swap(ctx.focused, ctx.mon_key, dir, ctx.states);
    }

    fn stretch_window(&self, dir: Direction, ctx: &mut ActionCtx) {
        if let Some(ms) = ctx.states.get_mut(&ctx.mon_key) {
            ms.stretch_window(ctx.focused, dir, &Win32System);
        }
    }

    fn shrink_window(&self, dir: Direction, ctx: &mut ActionCtx) {
        if let Some(ms) = ctx.states.get_mut(&ctx.mon_key) {
            ms.shrink_window(ctx.focused, dir, &Win32System);
        }
    }

    fn set_fullscreen(&self, ctx: &mut ActionCtx) {
        if let Some(ms) = ctx.states.get_mut(&ctx.mon_key) {
            ms.set_fullscreen(ctx.focused, &Win32System);
        }
    }

    fn set_minimize(&self, ctx: &mut ActionCtx) {
        commands::window::set_window_minimize(ctx.focused);
        if let Some(prev) = ctx.states.get_mut(&ctx.mon_key)
            .and_then(|ms| ms.get_last_focused_window(&Win32System)) {
                commands::window::set_foreground_window(prev);
        }
    }
}
