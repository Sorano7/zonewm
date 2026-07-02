use std::collections::HashMap;
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::GetForegroundWindow};

use crate::{commands::window::clear_window_border, models::{monitor::{Monitor, Rect}, system::WindowSystem, zone::{Layout, Zone}}, state::{window_state::WindowState, workspace::WORKSPACE_COUNT}};
#[cfg(debug_assertions)]
use crate::state::window_state::WindowRecord;
use super::workspace::Workspace;
use crate::models::zone::{MAX_POS_DELTA, MAX_SIZE_DELTA, AUTO_SNAP_THRESHOLD};

pub struct MonitorState {
    pub monitor: Monitor,
    pub layouts: Vec<Option<Layout>>,
    workspaces: Vec<Workspace>,
    pub active_ws: usize,
    /// When true, focus/move/swap operations stay on this monitor.
    pub monitor_locked: bool,
    /// Reverse lookup: hwnd (as isize) → workspace index.
    hwnd_ws: HashMap<isize, usize>,
    /// Last rect each hwnd was snapped to.
    snap_cache: HashMap<isize, Rect>,
    /// Raw window rect captured before a window was first snapped to a zone.
    pre_snap_rects: HashMap<isize, Rect>,
}

impl MonitorState {
    pub fn new(monitor: Monitor, layouts: Vec<Option<Layout>>) -> Self {
        let initial_idx = layouts.iter().position(|l| l.is_some()).unwrap_or(0);
        let zone_count = layouts.get(initial_idx).and_then(|l| l.as_ref())
            .map(|l| l.zones.len()).unwrap_or(0);
        let mut workspaces: Vec<Workspace> =
            (0..WORKSPACE_COUNT).map(|_| Workspace::new(zone_count)).collect();
        for ws in &mut workspaces { ws.layout_idx = initial_idx; }
        Self {
            monitor, layouts, workspaces, active_ws: 0, monitor_locked: true,
            hwnd_ws: HashMap::new(),
            snap_cache: HashMap::new(),
            pre_snap_rects: HashMap::new(),
        }
    }

    pub fn monitor_key(&self) -> String {
        self.monitor.device_id.clone()
    }

    pub fn workspace1_layout_idx(&self) -> usize {
        self.workspaces[0].layout_idx
    }

    pub fn reload_layouts(&mut self, layouts: Vec<Option<Layout>>, sys: &impl WindowSystem) {
        self.layouts = layouts;
        let fallback = self.layouts.iter().position(|l| l.is_some()).unwrap_or(0);
        for i in 0..WORKSPACE_COUNT {
            let idx = self.workspaces[i].layout_idx;
            let effective = if self.layouts.get(idx).and_then(|l| l.as_ref()).is_some() {
                idx
            } else {
                fallback
            };
            self.workspaces[i].layout_idx = effective;
            let zone_count = self.layouts.get(effective).and_then(|l| l.as_ref())
                .map(|l| l.zones.len()).unwrap_or(0);
            self.workspaces[i].zoned.resize_with(zone_count, Vec::new);
        }
        self.reflow(sys);
    }

    pub fn active_layout(&self) -> Option<&Layout> {
        self.layouts.get(self.workspaces[self.active_ws].layout_idx)?.as_ref()
    }

    /// Returns the workspace index that contains `hwnd`, or `None` if untracked.
    pub fn find_workspace(&self, hwnd: HWND) -> Option<usize> {
        self.hwnd_ws.get(&(hwnd.0 as isize)).copied()
    }

    /// All tracked windows across every workspace, with placement metadata for display.
    #[cfg(debug_assertions)]
    pub fn all_window_records(&self) -> Vec<WindowRecord> {
        let mut records = Vec::new();
        for (ws_idx, ws) in self.workspaces.iter().enumerate() {
            for (zone_idx, zone_windows) in ws.zoned.iter().enumerate() {
                for (z_order, &hwnd) in zone_windows.iter().enumerate() {
                    records.push(WindowRecord { hwnd, ws_idx, state: WindowState::Zoned(zone_idx), z_order });
                }
            }
            for (z_order, &hwnd) in ws.floating.iter().enumerate() {
                records.push(WindowRecord { hwnd, ws_idx, state: WindowState::Floating, z_order });
            }
        }
        records
    }

    /// Returns the management state of `hwnd` on this monitor.
    pub fn window_state(&self, hwnd: HWND) -> WindowState {
        let key = hwnd.0 as isize;
        match self.hwnd_ws.get(&key).copied() {
            None => WindowState::Ignored,
            Some(ws_idx) => {
                let ws = &self.workspaces[ws_idx];
                match ws.zoned.iter().position(|z| z.contains(&hwnd)) {
                    Some(zone_idx) => WindowState::Zoned(zone_idx),
                    None => WindowState::Floating,
                }
            }
        }
    }

    /// Absolute pixel rects for all zones in the active workspace's layout.
    pub fn active_zone_rects(&self) -> Vec<Rect> {
        let ws = &self.workspaces[self.active_ws];
        let Some(layout) = self.layouts.get(ws.layout_idx).and_then(|l| l.as_ref()) else {
            return vec![];
        };
        layout.zones.iter().map(|z| z.to_rect(self.monitor.work_area)).collect()
    }

    pub fn zoned_focus_candidates(&self, sys: &impl WindowSystem) -> Vec<(HWND, Rect)> {
        let ws = &self.workspaces[self.active_ws];
        let Some(layout) = self.layouts.get(ws.layout_idx).and_then(|l| l.as_ref()) else {
            return vec![];
        };
        layout.zones.iter().enumerate()
            .filter_map(|(idx, zone)| {
                let hwnd = self.topmost_in_zone(idx, sys)?;
                Some((hwnd, zone.to_rect(self.monitor.work_area)))
            })
            .collect()
    }

    pub fn floating_focus_candidates(&self, sys: &impl WindowSystem) -> Vec<(HWND, Rect)> {
        let ws = &self.workspaces[self.active_ws];

        let mut result: Vec<(HWND, Rect)> = ws.floating.iter().copied()
            .filter(|&h| !sys.is_minimized(h))
            .filter_map(|h| sys.window_rect(h).map(|r| (h, r)))
            .collect();

        // Untracked windows currently visible on this monitor.
        let tracked_keys: std::collections::HashSet<isize> =
            ws.all_windows().into_iter().map(|h| h.0 as isize).collect();
        for h in sys.enumerate_on_monitor(self.monitor.handle) {
            if !tracked_keys.contains(&(h.0 as isize)) {
                if let Some(rect) = sys.window_rect(h) {
                    result.push((h, rect));
                }
            }
        }

        result
    }

    pub fn assign_to_zone_ws(&mut self, zone_idx: usize, ws_idx: usize, hwnd: HWND, pre_snap_rect: Rect) {
        let key = hwnd.0 as isize;
        if zone_idx >= self.workspaces[ws_idx].zoned.len() { return; }

        if !self.workspaces[ws_idx].zoned.iter().any(|z| z.contains(&hwnd)) {
            self.pre_snap_rects.insert(key, pre_snap_rect);
        }

        {
            let ws = &mut self.workspaces[ws_idx];
            for zone in &mut ws.zoned {
                zone.retain(|&h| h != hwnd);
            }
            ws.floating.retain(|&h| h != hwnd);
            if let Some(zone) = ws.zoned.get_mut(zone_idx) {
                zone.push(hwnd);
            }
        }
        self.hwnd_ws.insert(key, ws_idx);
        self.snap_cache.remove(&key);
    }

    pub fn assign_to_zone(&mut self, zone_idx: usize, hwnd: HWND, pre_drag_rect: Rect) {
        self.assign_to_zone_ws(zone_idx, self.active_ws, hwnd, pre_drag_rect);
    }

    /// Move a zoned window to floating and restore its pre-snap rect.
    pub fn set_floating(&mut self, hwnd: HWND, sys: &impl WindowSystem) {
        let key = hwnd.0 as isize;
        if self.detach_from_zone(hwnd) {
            if let Some(rect) = self.pre_snap_rects.get(&key).copied() {
                if rect.width() > 0 && rect.height() > 0 {
                    sys.restore_window_size(hwnd, &rect);
                }
            }
        }
    }

    /// Move a zoned window to floating, keeping its current rect.
    pub fn set_floating_in_place(&mut self, hwnd: HWND) {
        self.detach_from_zone(hwnd);
    }

    fn detach_from_zone(&mut self, hwnd: HWND) -> bool {
        let key = hwnd.0 as isize;
        let ws_idx = match self.hwnd_ws.get(&key).copied() {
            Some(i) => i,
            None => return false,
        };
        let ws = &mut self.workspaces[ws_idx];
        if !ws.zoned.iter().any(|z| z.contains(&hwnd)) {
            return false;
        }
        for zone in &mut ws.zoned {
            zone.retain(|&h| h != hwnd);
        }
        ws.floating.push(hwnd);
        self.snap_cache.remove(&key);
        true
    }

    pub fn move_window_to_zone_idx(&mut self, hwnd: HWND, dst_zone: usize, sys: &impl WindowSystem) {
        let key = hwnd.0 as isize;
        let ws_idx = self.active_ws;

        // Capture pre_snap_rect when first transitioning from non-zoned.
        if !self.workspaces[ws_idx].zoned.iter().any(|z| z.contains(&hwnd)) {
            if let Some(rect) = sys.window_rect(hwnd) {
                self.pre_snap_rects.insert(key, rect);
            }
        }

        {
            let ws = &mut self.workspaces[ws_idx];
            for zone in &mut ws.zoned {
                zone.retain(|&h| h != hwnd);
            }
            ws.floating.retain(|&h| h != hwnd);
            if let Some(zone) = ws.zoned.get_mut(dst_zone) {
                zone.push(hwnd);
            }
        }
        self.hwnd_ws.insert(key, ws_idx);

        let layout_idx = self.workspaces[ws_idx].layout_idx;
        if let Some(zone) = self.layouts.get(layout_idx).and_then(|l| l.as_ref())
            .and_then(|l| l.zones.get(dst_zone))
        {
            let rect = zone.to_rect(self.monitor.work_area);
            sys.snap_window(hwnd, &rect);
            self.snap_cache.insert(key, rect);
        } else {
            self.snap_cache.remove(&key);
        }
    }

    /// Completely remove `hwnd` from this monitor's tracking.
    pub fn detach_window(&mut self, hwnd: HWND) {
        let key = hwnd.0 as isize;
        if let Some(ws_idx) = self.hwnd_ws.remove(&key) {
            self.workspaces[ws_idx].remove(hwnd);
        }
        self.snap_cache.remove(&key);
        self.pre_snap_rects.remove(&key);
    }

    pub fn topmost_in_zone(&self, zone_idx: usize, sys: &impl WindowSystem) -> Option<HWND> {
        self.workspaces[self.active_ws].zoned.get(zone_idx)?
            .iter().rev()
            .find(|&&h| !sys.is_minimized(h))
            .copied()
    }

    pub fn cycle_window_in_zone(&mut self, hwnd: HWND, forward: bool, sys: &impl WindowSystem) -> Option<HWND> {
        let ws_idx = *self.hwnd_ws.get(&(hwnd.0 as isize))?;
        for zone_windows in &mut self.workspaces[ws_idx].zoned {
            let pos = match zone_windows.iter().position(|&h| h == hwnd) {
                Some(p) => p,
                None => continue,
            };
            let n = zone_windows.len();
            // Walk through the other entries (wrapping), skipping minimized.
            for step in 1..n {
                let idx = if forward {
                    (pos + step) % n
                } else {
                    (pos + n - step) % n
                };
                let candidate = zone_windows[idx];
                if !sys.is_minimized(candidate) {
                    zone_windows.remove(idx);
                    zone_windows.push(candidate);
                    return Some(candidate);
                }
            }
            return None; // Only minimized windows remain
        }
        None // Not in any zone (floating)
    }

    /// Called when a zoned window is restored from minimized.
    pub fn on_window_restored(&mut self, hwnd: HWND, sys: &impl WindowSystem) {
        let key = hwnd.0 as isize;
        if let Some(&ws_idx) = self.hwnd_ws.get(&key) {
            if ws_idx == self.active_ws
                && matches!(self.window_state(hwnd), WindowState::Zoned(_))
            {
                self.snap_cache.remove(&key);
                self.reflow(sys);
            }
        }
    }

    pub fn update_last_focused_window(&mut self) {
        let ws = &mut self.workspaces[self.active_ws];
        let focused = unsafe { GetForegroundWindow() };
        ws.last_focused.window = Some(focused);
        ws.last_focused.zone = ws.get_zone_index(focused);
    }
    
    fn first_visible_in_vec(&self, vec: &Vec<HWND>, sys: &impl WindowSystem) -> Option<HWND> {
        vec.iter().rev().find(|&&h| !sys.is_minimized(h)).copied()
    }

    pub fn get_last_focused_window(&self, sys: &impl WindowSystem) -> Option<HWND> {
        let ws = &self.workspaces[self.active_ws];

        let last_focused = ws.last_focused.window.filter(|&h| !sys.is_minimized(h));

        // first visible window in either last focused zone or first zone.
        let visible_last_zone = ws.last_focused.zone
            .and_then(|idx| ws.zoned.get(idx))
            .or_else(|| ws.zoned.first())
            .and_then(|z| self.first_visible_in_vec(z, sys));

        let visible_floating = self.first_visible_in_vec(&ws.floating, sys);

        last_focused.or(visible_last_zone).or(visible_floating)
    }

    pub fn layout_len(&self, layout_idx: usize) -> Option<usize> {
        self.layouts.get(layout_idx)
            .and_then(|l| l.as_ref())
            .map(|l| l.zones.len())
    }

    pub fn switch_layout(&mut self, layout_idx: usize) {
        let new_len = match self.layout_len(layout_idx) {
            Some(l) => l,
            None => return,
        };
        let ws = &mut self.workspaces[self.active_ws];
        ws.layout_idx = layout_idx;
        ws.zoned.resize_with(new_len, Vec::new);
    }

    pub fn reflow(&mut self, sys: &impl WindowSystem) {
        let ws = &self.workspaces[self.active_ws];
        let Some(layout) = self.layouts.get(ws.layout_idx).and_then(|l| l.as_ref()) else { return; };
        let work_area = self.monitor.work_area;
        for (i, zone) in layout.zones.iter().enumerate() {
            if let Some(hwnds) = ws.zoned.get(i) {
                let rect = zone.to_rect(work_area);
                for &hwnd in hwnds {
                    let key = hwnd.0 as isize;
                    if self.snap_cache.get(&key) != Some(&rect) {
                        sys.snap_window(hwnd, &rect);
                        self.snap_cache.insert(key, rect);
                    }
                }
            }
        }
    }

    fn auto_snap_score(&self, zone: &Zone, work_area: Rect, r: Rect) -> Option<i32> {
        let zr = zone.to_rect(work_area);

        let dx = (r.left - zr.left).abs();
        let dy = (r.top - zr.top).abs();
        let dw = (r.width() - zr.width()).abs();
        let dh = (r.height() - zr.height()).abs();
        let score = dx + dy + dw + dh;

        let within_tolerance = dx <= MAX_POS_DELTA
            && dy <= MAX_POS_DELTA
            && dw <= MAX_SIZE_DELTA
            && dh <= MAX_SIZE_DELTA
            && score <= AUTO_SNAP_THRESHOLD;

        within_tolerance.then_some(score)
    }

    fn try_auto_snap(&mut self, hwnd: HWND, sys: &impl WindowSystem) -> bool {
        let Some(r) = sys.window_rect(hwnd) else {
            return false;
        };
        let layout_idx = self.workspaces[self.active_ws].layout_idx;
        let Some(layout) = &self.layouts[layout_idx] else {
            return false;
        };

        let best_zone = layout
            .zones
            .iter()
            .enumerate()
            .filter_map(|(idx, zone)| 
                self.auto_snap_score(zone, self.monitor.work_area, r)
                    .map(|score| (idx, score))
            )
            .min_by_key(|&(_, score)| score);

        let Some((zone_idx, _)) = best_zone else {
            return false;
        };

        self.assign_to_zone(zone_idx, hwnd, r);
        true
    }

    pub fn capture_all_windows(&mut self, sys: &impl WindowSystem) {
        let on_monitor = sys.enumerate_on_monitor(self.monitor.handle);
        let new_floating: Vec<HWND> = on_monitor
            .into_iter()
            .filter(|&h| !self.hwnd_ws.contains_key(&(h.0 as isize)))
            .collect();
        for &h in &new_floating {
            let key = h.0 as isize;
            self.hwnd_ws.insert(key, self.active_ws);
            self.snap_cache.remove(&key);
            if !self.try_auto_snap(h, sys) {
                self.workspaces[self.active_ws].floating.push(h);
            }
        }

        self.reflow(sys);
    }

    pub fn switch_workspace(&mut self, new_idx: usize, sys: &impl WindowSystem) {
        if new_idx >= WORKSPACE_COUNT || new_idx == self.active_ws { return; }

        self.update_last_focused_window();

        for hwnd in self.workspaces[self.active_ws].all_windows() {
            sys.set_cloak(hwnd, true);
        }

        self.active_ws = new_idx;

        for hwnd in self.workspaces[self.active_ws].all_windows() {
            sys.set_cloak(hwnd, false);
        }

        self.reflow(sys);
    }

    pub fn window_is_in_bound(&self, hwnd: HWND, layout_idx: usize) -> bool {
        self.workspaces[self.active_ws].get_zone_index(hwnd).is_some_and(|idx| {
            let new_len = self.layout_len(layout_idx).unwrap_or(0);
            idx < new_len
        })
    }

    pub fn move_window_to_workspace(&mut self, hwnd: HWND, target_ws: usize, sys: &impl WindowSystem) {
        if target_ws >= WORKSPACE_COUNT || target_ws == self.active_ws { return; }
        let key = hwnd.0 as isize;
        let layout_idx = self.workspaces[target_ws].layout_idx;
        let rect = match self.pre_snap_rects.get(&key).copied() {
            Some(r) => r,
            None => sys.window_rect(hwnd).expect("Failed to get window rect"),
        };

        if self.window_is_in_bound(hwnd, layout_idx) {
            let zone_idx = self.workspaces[self.active_ws].get_zone_index(hwnd)
                .expect("Window removed before zone index is retrieved");
            self.assign_to_zone_ws(zone_idx, target_ws, hwnd, rect);
        } else {
            self.workspaces[target_ws].floating.push(hwnd);
            if rect.width() > 0 && rect.height() > 0 {
                sys.restore_window_size(hwnd, &rect);
            }
        }

        self.workspaces[self.active_ws].remove(hwnd);
        self.hwnd_ws.insert(key, target_ws);
        self.snap_cache.remove(&key);
        sys.set_cloak(hwnd, true);
    }

    pub fn uncloak_all(&self, sys: &impl WindowSystem) {
        for ws in &self.workspaces {
            for hwnd in ws.all_windows() {
                sys.set_cloak(hwnd, false);
            }
        }
    }

    pub fn clear_all_window_borders(&self) {
        for ws in &self.workspaces {
            for hwnd in ws.all_windows() {
                clear_window_border(hwnd);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::{models::monitor::Rect, state::window_state::WindowState, test_utils::{MockSystem, h, make_layouts, make_state, two_by_two_layout, two_col_layout}};

    #[test]
    fn assign_places_window_in_zone() {
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        assert_eq!(ms.window_state(h(1)), WindowState::Zoned(0));
    }

    #[test]
    fn window_state_is_ignored_when_untracked() {
        let ms = make_state();
        assert_eq!(ms.window_state(h(99)), WindowState::Ignored);
    }

    #[test]
    fn reassign_moves_window_to_new_zone() {
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(1, h(1), Rect::default());
        assert_eq!(ms.window_state(h(1)), WindowState::Zoned(1));
    }

    #[test]
    fn window_state_is_floating_after_set_floating() {
        let mut ms = make_state();
        let sys = MockSystem::default();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.set_floating(h(1), &sys);
        assert_eq!(ms.window_state(h(1)), WindowState::Floating);
    }

    #[test]
    fn detach_window_removes_from_tracking() {
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.detach_window(h(1));
        assert_eq!(ms.window_state(h(1)), WindowState::Ignored);
    }

    #[test]
    fn capture_all_windows_tracks_untracked_windows_as_floating() {
        let mut ms = make_state();
        let sys = MockSystem { on_monitor: vec![h(1)], ..Default::default() };
        ms.capture_all_windows(&sys);
        assert_eq!(ms.window_state(h(1)), WindowState::Floating);
    }

    #[test]
    fn capture_all_windows_does_not_redetect_window_on_inactive_workspace() {
        let mut ms = make_state();
        let move_sys = MockSystem::default().with_rect(h(1), Rect::default());
        ms.move_window_to_workspace(h(1), 1, &move_sys);
        let sys = MockSystem { on_monitor: vec![h(1)], ..Default::default() };
        ms.capture_all_windows(&sys);
        assert_eq!(ms.find_workspace(h(1)), Some(1));
    }

    #[test]
    fn switch_workspace_cloaks_windows_on_old_workspace() {
        let sys = MockSystem::default();
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.switch_workspace(1, &sys);
        assert!(sys.is_cloaked(h(1)));
    }

    #[test]
    fn switch_workspace_uncloaks_windows_on_new_workspace() {
        let sys = MockSystem::default().with_rect(h(2), Rect::default());
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());

        ms.move_window_to_workspace(h(2), 1, &sys);
        ms.switch_workspace(1, &sys);
        assert!(!sys.is_cloaked(h(2)));
    }

    #[test]
    fn move_to_workspace_arrives_as_zoned_if_index_in_bound() {
        let sys = MockSystem::default().with_rect(h(1), Rect::default());
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.move_window_to_workspace(h(1), 1, &sys);
        ms.switch_workspace(1, &sys);
        assert_eq!(ms.window_state(h(1)), WindowState::Zoned(0));
    }

    #[test]
    fn move_to_workspace_arrives_as_floating_if_index_out_of_bounds() {
        let sys = MockSystem::default().with_rect(h(1), Rect::default());
        let mut ms = make_state();
        ms.layouts = make_layouts(vec![two_by_two_layout, two_col_layout]);

        ms.assign_to_zone(3, h(1), Rect::default());
        ms.move_window_to_workspace(h(1), 1, &sys);
        ms.switch_workspace(1, &sys);
        assert_eq!(ms.window_state(h(1)), WindowState::Floating);
    }

    #[test]
    fn topmost_in_zone_returns_last_added() {
        let sys = MockSystem::default();
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(0, h(2), Rect::default());
        assert_eq!(ms.topmost_in_zone(0, &sys), Some(h(2)));
    }

    #[test]
    fn topmost_in_zone_skips_minimized() {
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(0, h(2), Rect::default());
        let sys = MockSystem::default().with_minimized(h(2));
        assert_eq!(ms.topmost_in_zone(0, &sys), Some(h(1)));
    }

    #[test]
    fn topmost_in_zone_returns_none_when_all_minimized() {
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        let sys = MockSystem::default().with_minimized(h(1));
        assert_eq!(ms.topmost_in_zone(0, &sys), None);
    }

    #[test]
    fn topmost_in_zone_tracks_window_focused_via_cycle() {
        let sys = MockSystem::default();
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default()); // A
        ms.assign_to_zone(0, h(2), Rect::default()); // B, inserted last

        // Cycle backward from B: A becomes the window actually in front.
        assert_eq!(ms.cycle_window_in_zone(h(2), false, &sys), Some(h(1)));
        assert_eq!(ms.topmost_in_zone(0, &sys), Some(h(1)));
    }

    /// 2-column layout, A and B in the left zone (B inserted last), C alone in
    /// the right zone. After cycling zone 0's front back to A, a directional
    /// focus move into zone 0 must land on A — the window actually in front —
    /// not on B.
    #[test]
    fn zoned_focus_candidates_tracks_window_focused_via_cycle_in_left_zone() {
        let sys = MockSystem::default();
        let mut ms = make_state(); // two_col_layout: zone 0 left, zone 1 right
        ms.assign_to_zone(0, h(1), Rect::default()); // A
        ms.assign_to_zone(0, h(2), Rect::default()); // B, inserted last
        ms.assign_to_zone(1, h(3), Rect::default()); // C

        // Cycle backward from B: A becomes the window actually in front of zone 0.
        ms.cycle_window_in_zone(h(2), false, &sys);

        let candidates = ms.zoned_focus_candidates(&sys);
        let zone0_rect = Rect { left: 0, top: 0, right: 960, bottom: 1080 };
        let zone0_candidate = candidates.iter().find(|&&(_, r)| r == zone0_rect).map(|&(h, _)| h);

        assert_eq!(zone0_candidate, Some(h(1)));
    }

    /// Repeated forward cycling through a 3-window zone should visit each window
    /// exactly once and return to the starting front window, confirming the
    /// front/back reordering behaves as a clean round-robin rather than getting
    /// stuck or skipping entries.
    #[test]
    fn cycle_forward_through_full_zone_is_a_clean_round_robin() {
        let sys = MockSystem::default();
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default()); // A
        ms.assign_to_zone(0, h(2), Rect::default()); // B
        ms.assign_to_zone(0, h(3), Rect::default()); // C, inserted last (front)

        let first  = ms.cycle_window_in_zone(h(3), true, &sys).unwrap();
        let second = ms.cycle_window_in_zone(first, true, &sys).unwrap();
        let third  = ms.cycle_window_in_zone(second, true, &sys).unwrap();

        let mut visited = [first, second, third];
        visited.sort_by_key(|h| h.0 as isize);
        assert_eq!(visited, [h(1), h(2), h(3)]);
        assert_eq!(third, h(3));
    }

    #[test]
    fn cycle_forward_moves_to_next_window() {
        let sys = MockSystem::default();
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(0, h(2), Rect::default());
        ms.assign_to_zone(0, h(3), Rect::default());
        assert_eq!(ms.cycle_window_in_zone(h(1), true, &sys), Some(h(2)));
    }

    #[test]
    fn cycle_forward_wraps_around() {
        let sys = MockSystem::default();
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(0, h(2), Rect::default());
        assert_eq!(ms.cycle_window_in_zone(h(2), true, &sys), Some(h(1)));
    }

    #[test]
    fn cycle_skips_minimized() {
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(0, h(2), Rect::default());
        ms.assign_to_zone(0, h(3), Rect::default());
        let sys = MockSystem::default().with_minimized(h(2));
        assert_eq!(ms.cycle_window_in_zone(h(1), true, &sys), Some(h(3)));
    }

    #[test]
    fn first_visible_returns_topmost_zoned_window() {
        let sys = MockSystem::default();
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(0, h(2), Rect::default());
        assert_eq!(ms.get_last_focused_window(&sys), Some(h(2)));
    }

    #[test]
    fn first_visible_skips_minimized() {
        let mut ms = make_state();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(0, h(2), Rect::default());
        let sys = MockSystem::default().with_minimized(h(2));
        assert_eq!(ms.get_last_focused_window(&sys), Some(h(1)));
    }

    #[test]
    fn first_visible_falls_back_to_floating() {
        let sys = MockSystem::default();
        let mut ms = make_state();
        let sys_m = MockSystem::default();
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.set_floating(h(1), &sys_m);
        assert_eq!(ms.get_last_focused_window(&sys), Some(h(1)));
    }

    #[test]
    fn zoned_focus_candidates_one_per_zone_using_topmost() {
        let sys = MockSystem::default();
        let mut ms = make_state(); // two_col_layout
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.assign_to_zone(0, h(2), Rect::default()); // topmost in zone 0
        ms.assign_to_zone(1, h(3), Rect::default());

        let mut candidates = ms.zoned_focus_candidates(&sys);
        candidates.sort_by_key(|&(h, _)| h.0 as isize);
        assert_eq!(candidates, vec![
            (h(2), Rect { left: 0, top: 0, right: 960, bottom: 1080 }),
            (h(3), Rect { left: 960, top: 0, right: 1920, bottom: 1080 }),
        ]);
    }

    #[test]
    fn zoned_focus_candidates_skips_empty_and_all_minimized_zones() {
        let mut ms = make_state(); // zone 0 stays empty
        ms.assign_to_zone(1, h(1), Rect::default());
        let sys = MockSystem::default().with_minimized(h(1));
        assert_eq!(ms.zoned_focus_candidates(&sys), vec![]);
    }

    #[test]
    fn floating_focus_candidates_uses_actual_window_rect() {
        let mut ms = make_state();
        let sys = MockSystem::default()
            .with_rect(h(1), Rect { left: 100, top: 100, right: 300, bottom: 300 });
        ms.assign_to_zone(0, h(1), Rect::default());
        ms.set_floating_in_place(h(1));
        assert_eq!(
            ms.floating_focus_candidates(&sys),
            vec![(h(1), Rect { left: 100, top: 100, right: 300, bottom: 300 })],
        );
    }

    #[test]
    fn floating_focus_candidates_includes_untracked_windows_on_monitor() {
        let sys = MockSystem {
            on_monitor: vec![h(9)],
            rects: HashMap::from([(9, Rect { left: 50, top: 50, right: 150, bottom: 150 })]),
            ..Default::default()
        };
        let ms = make_state();
        assert_eq!(
            ms.floating_focus_candidates(&sys),
            vec![(h(9), Rect { left: 50, top: 50, right: 150, bottom: 150 })],
        );
    }
}
