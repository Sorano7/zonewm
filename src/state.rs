use std::collections::HashMap;
use std::path::Path;

use crate::{
    config,
    models::{monitor::Monitor, system::WindowSystem, zone::Layout},
    state::monitor_state::MonitorState,
};

pub mod window_state;
pub mod workspace;
pub mod monitor_state;

pub type StateMap = HashMap<isize, MonitorState>;

pub fn build(monitors: Vec<Monitor>, layouts: Vec<Option<Layout>>) -> StateMap {
    monitors
        .into_iter()
        .map(|m| (m.handle.0 as isize, MonitorState::new(m, layouts.clone())))
        .collect()
}

pub fn reconcile(
    states: &mut StateMap,
    monitors: Vec<Monitor>,
    cfg_path: &Path,
    saved: &config::SavedState,
    sys: &impl WindowSystem,
) {
    let unchanged = monitors.iter().all(|m| {
        states.values().any(|ms| {
            ms.monitor.device_id == m.device_id
                && ms.monitor.handle == m.handle
                && ms.monitor.work_area == m.work_area
        })
    });
    if unchanged { return; }

    for m in monitors {
        let existing_key = states.iter()
            .find(|(_, ms)| ms.monitor.device_id == m.device_id)
            .map(|(&k, _)| k);

        if let Some(old_key) = existing_key {
            let mut ms = states.remove(&old_key).unwrap();
            ms.monitor = m;
            ms.reflow(sys);
            states.insert(ms.monitor.handle.0 as isize, ms);
        } else {
            let layouts = states.values().next()
                .map(|ms| ms.layouts.clone())
                .unwrap_or_else(|| config::to_layouts(&config::load(cfg_path)));
            let mut ms = MonitorState::new(m, layouts);
            if let Some(&idx) = saved.monitor_layouts.get(&ms.monitor_key()) {
                ms.switch_layout(idx);
            }
            states.insert(ms.monitor.handle.0 as isize, ms);
        }
    }
}

