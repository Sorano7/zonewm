use std::collections::HashMap;

use crate::{models::{monitor::Monitor, zone::Layout}, state::monitor_state::MonitorState};

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

