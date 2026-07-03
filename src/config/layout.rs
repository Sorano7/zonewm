use serde::Deserialize;

use crate::config::Config;
use crate::config::default::DEFAULT_CONFIG;
use crate::models::zone::{Axis, Layout, Zone, ZoneNode};
use crate::state::workspace::WORKSPACE_COUNT;

/// A node in the zone tree.
/// - Empty `{}` → single zone filling the allocated rect.
/// - `columns` → split horizontally; fractions must sum to ~1.0.
/// - `rows`    → split vertically; fractions must sum to ~1.0.
/// - `children` → one child per column/row; omitted children become leaf zones.
#[derive(Deserialize, Default)]
struct ZoneTree {
    #[serde(default)]
    columns: Vec<f32>,
    #[serde(default)]
    rows: Vec<f32>,
    #[serde(default)]
    children: Vec<ZoneTree>,
}

#[derive(Deserialize)]
pub struct LayoutEntry {
    name: String,
    /// 1-based key number (1..9). Omit to fill the next available slot in order.
    #[serde(default)]
    index: Option<usize>,
    zones: ZoneTree,
}

/// Flattens a config zone tree into `zones` (leaf rects, in reading order)
/// and returns the matching `ZoneNode` tree, whose leaves index into `zones`.
fn flatten(node: &ZoneTree, x: f32, y: f32, w: f32, h: f32, zones: &mut Vec<Zone>) -> ZoneNode {
    if !node.columns.is_empty() {
        let mut children = Vec::new();
        let mut cx = x;
        for (i, &frac) in node.columns.iter().enumerate() {
            let cw = frac * w;
            children.push(match node.children.get(i) {
                Some(child) => flatten(child, cx, y, cw, h, zones),
                None => {
                    let idx = zones.len();
                    zones.push(Zone { x: cx, y, w: cw, h });
                    ZoneNode::Leaf(idx)
                }
            });
            cx += cw;
        }
        ZoneNode::Split { axis: Axis::Horizontal, children }
    } else if !node.rows.is_empty() {
        let mut children = Vec::new();
        let mut cy = y;
        for (i, &frac) in node.rows.iter().enumerate() {
            let rh = frac * h;
            children.push(match node.children.get(i) {
                Some(child) => flatten(child, x, cy, w, rh, zones),
                None => {
                    let idx = zones.len();
                    zones.push(Zone { x, y: cy, w, h: rh });
                    ZoneNode::Leaf(idx)
                }
            });
            cy += rh;
        }
        ZoneNode::Split { axis: Axis::Vertical, children }
    } else {
        let idx = zones.len();
        zones.push(Zone { x, y, w, h });
        ZoneNode::Leaf(idx)
    }
}

pub fn to_layouts(cfg: &Config) -> Vec<Option<Layout>> {
    let Some(layout) = &cfg.layout else {
        let default: Config = toml::from_str(DEFAULT_CONFIG)
            .expect("DEFAULT_CONFIG must always parse");
        return to_layouts(&default);
    };

    let mut slots: Vec<Option<Layout>> = vec![None; WORKSPACE_COUNT];
    let mut unindexed: Vec<Layout> = Vec::new();
    for entry in layout {
        let mut zones = Vec::new();
        let tree = flatten(&entry.zones, 0.0, 0.0, 1.0, 1.0, &mut zones);
        let layout = Layout { name: entry.name.clone(), zones, tree };
        match entry.index {
            Some(i) if (1..=WORKSPACE_COUNT).contains(&i) => {
                let slot = i - 1;
                if slots[slot].is_none() {
                    slots[slot] = Some(layout);
                }
            }
            _ => unindexed.push(layout),
        }
    }

    // Fill remaining empty slots with unindexed layouts, in order.
    let mut it = unindexed.into_iter();
    for slot in &mut slots {
        if slot.is_none() {
            if let Some(l) = it.next() { *slot = Some(l); }
        }
    }

    slots
}
