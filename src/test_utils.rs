#![cfg(test)]

use std::{cell::RefCell, collections::{HashMap, HashSet}};

use windows::Win32::{Foundation::HWND, Graphics::Gdi::HMONITOR};

use crate::{models::{monitor::{Monitor, Rect}, system::WindowSystem, zone::{Axis, Layout, Zone, ZoneNode}}, state::{monitor_state::MonitorState, workspace::WORKSPACE_COUNT}};

pub fn h(n: usize) -> HWND {
    HWND(n as *mut core::ffi::c_void)
}

pub fn hmon(n: usize) -> HMONITOR {
    HMONITOR(n as *mut core::ffi::c_void)
}

#[derive(Default)]
pub struct MockSystem {
    pub minimized: HashSet<isize>,
    pub rects: HashMap<isize, Rect>,
    pub on_monitor: Vec<HWND>,
    pub snapped: RefCell<Vec<(isize, Rect)>>,
    pub cloaked: RefCell<HashMap<isize, bool>>,
    pub brought_to_front: RefCell<Vec<isize>>,
}

impl MockSystem {
    pub fn with_minimized(mut self, hwnd: HWND) -> Self {
        self.minimized.insert(hwnd.0 as isize);
        self
    }
    pub fn with_rect(mut self, hwnd: HWND, rect: Rect) -> Self {
        self.rects.insert(hwnd.0 as isize, rect);
        self
    }
    pub fn is_cloaked(&self, hwnd: HWND) -> bool {
        *self.cloaked.borrow().get(&(hwnd.0 as isize)).unwrap_or(&false)
    }
}

impl WindowSystem for MockSystem {
    fn snap_window(&self, hwnd: HWND, rect: &Rect) {
        self.snapped.borrow_mut().push((hwnd.0 as isize, *rect));
    }
    fn restore_window_size(&self, _hwnd: HWND, _rect: &Rect) {}
    fn set_cloak(&self, hwnd: HWND, cloaked: bool) {
        self.cloaked.borrow_mut().insert(hwnd.0 as isize, cloaked);
    }
    fn forget_cloak_view(&self, _hwnd: HWND) {}
    fn enumerate_on_monitor(&self, _hmon: HMONITOR) -> Vec<HWND> {
        self.on_monitor.clone()
    }
    fn is_minimized(&self, hwnd: HWND) -> bool {
        self.minimized.contains(&(hwnd.0 as isize))
    }
    fn window_rect(&self, hwnd: HWND) -> Option<Rect> {
        self.rects.get(&(hwnd.0 as isize)).copied()
    }
    fn bring_to_front(&self, hwnd: HWND) {
        self.brought_to_front.borrow_mut().push(hwnd.0 as isize);
    }
}

pub fn work_area() -> Rect {
    Rect { left: 0, top: 0, right: 1920, bottom: 1080 }
}

pub fn make_monitor() -> Monitor {
    Monitor {
        handle: hmon(1),
        work_area: work_area(),
        device_name: r"\\.\DISPLAY1".into(),
        device_id: "test-monitor-1".into(),
    }
}

pub fn make_layouts(layouts: Vec<impl Fn() -> Layout>) -> Vec<Option<Layout>> {
    let mut slots = vec![None; WORKSPACE_COUNT];
    assert!(layouts.len() <= WORKSPACE_COUNT, "Too many layouts");
    for (i, f) in layouts.iter().enumerate() {
        slots[i] = Some(f());
    }
    slots
}

pub fn one_col_layout() -> Layout {
    Layout {
        name: "1-col".into(),
        zones: vec![Zone { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }],
        tree: ZoneNode::Leaf(0),
    }
}

pub fn two_col_layout() -> Layout {
    Layout {
        name: "2-col".into(),
        zones: vec![
            Zone { x: 0.0, y: 0.0, w: 0.5, h: 1.0 },
            Zone { x: 0.5, y: 0.0, w: 0.5, h: 1.0 },
        ],
        tree: ZoneNode::Split {
            axis: Axis::Horizontal,
            children: vec![ZoneNode::Leaf(0), ZoneNode::Leaf(1)],
        },
    }
}

pub fn two_row_layout() -> Layout {
    Layout {
        name: "2-row".into(),
        zones: vec![
            Zone { x: 0.0, y: 0.0, w: 1.0, h: 0.5 },
            Zone { x: 0.0, y: 0.5, w: 1.0, h: 0.5 },
        ],
        tree: ZoneNode::Split {
            axis: Axis::Vertical,
            children: vec![ZoneNode::Leaf(0), ZoneNode::Leaf(1)],
        },
    }
}

pub fn two_by_two_layout() -> Layout {
    Layout {
        name: "2x2".into(),
        zones: vec![
            Zone { x: 0.0, y: 0.0, w: 0.5, h: 0.5 },
            Zone { x: 0.5, y: 0.0, w: 0.5, h: 0.5 },
            Zone { x: 0.0, y: 0.5, w: 0.5, h: 0.5 },
            Zone { x: 0.5, y: 0.5, w: 0.5, h: 0.5 },
        ],
        tree: ZoneNode::Split {
            axis: Axis::Vertical,
            children: vec![
                ZoneNode::Split { axis: Axis::Horizontal, children: vec![ZoneNode::Leaf(0), ZoneNode::Leaf(1)] },
                ZoneNode::Split { axis: Axis::Horizontal, children: vec![ZoneNode::Leaf(2), ZoneNode::Leaf(3)] },
            ],
        },
    }
}

/// Create a MonitorState with two_col_layout.
pub fn make_state() -> MonitorState {
    let layouts = make_layouts(vec![two_col_layout]);
    MonitorState::new(make_monitor(), layouts)
}
