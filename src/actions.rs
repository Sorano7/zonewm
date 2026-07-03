use windows::Win32::Foundation::HWND;

use crate::{commands, models::system::Win32System, overlay::flash::FlashOverlay, state::{StateMap, window_state::Direction}};

pub mod hooks;

pub struct ActionCtx<'a> {
    pub states: &'a mut StateMap,
    pub mon_key: isize,
    pub focused: HWND,
    pub flash: &'a mut FlashOverlay,
}

pub enum Action {
    SetLayout(usize),
    SetWorkspace(usize),
    WinMoveWS(usize),
    SetFloat,
    ToggleMonLock,
    WinCycle(bool),
    WinFocus(Direction),
    WinMove(Direction),
    WinSwap(Direction),
    WinStretch(Direction),
    WinShrink(Direction),
}

impl Action {
    pub fn execute(&self, ctx: &mut ActionCtx) {
        match self {
            Action::SetLayout(idx)    => self.set_layout(*idx, ctx),
            Action::SetWorkspace(idx) => self.set_workspace(*idx, ctx), 
            Action::WinMoveWS(idx)    => self.move_window_ws(*idx, ctx), 
            Action::SetFloat          => self.set_float(ctx), 
            Action::ToggleMonLock     => self.toggle_monitor_lock(ctx), 
            Action::WinCycle(fw)      => self.cycle_window(*fw, ctx),
            Action::WinFocus(dir)     => self.focus_window(*dir, ctx),
            Action::WinMove(dir)      => self.move_window(*dir, ctx),
            Action::WinSwap(dir)      => self.swap_window(*dir, ctx),
            Action::WinStretch(dir)   => self.stretch_window(*dir, ctx),
            Action::WinShrink(dir)    => self.shrink_window(*dir, ctx),

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

    fn set_float(&self, ctx: &mut ActionCtx) {
        let Some(ms) = ctx.states.get_mut(&ctx.mon_key) else {
            return;
        };

        ms.set_floating(ctx.focused, &Win32System);

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
        commands::window::handle_cycle(ctx.focused, ctx.mon_key, fw, ctx.states);
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
}
