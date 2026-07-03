use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

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
struct LayoutEntry {
    name: String,
    /// 1-based key number (1..9). Omit to fill the next available slot in order.
    #[serde(default)]
    index: Option<usize>,
    zones: ZoneTree,
}

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    layout: Vec<LayoutEntry>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct SavedState {
    #[serde(default)]
    pub monitor_layouts: HashMap<String, usize>,
}

fn zonewm_dir() -> PathBuf {
    let user_profile = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    user_profile.join(".config").join("zonewm")
}

pub fn config_path() -> PathBuf {
    zonewm_dir().join("config.toml")
}

pub fn state_path() -> PathBuf {
    zonewm_dir().join("state.toml")
}

const DEFAULT_CONFIG: &str = r#"# ZoneWM layout definitions.
# Layouts are mapped to ctrl+alt+1 .. ctrl+alt+9.
# 'index' pins a layout to a specific key (1..9); layouts without index fill
# remaining slots in order. Fractions are relative to the parent slot.

[[layout]]
name = "2-Column"
zones = { columns = [0.5, 0.5] }

[[layout]]
name = "3-Column"
zones = { columns = [0.333, 0.334, 0.333] }

[[layout]]
name = "2x2 Grid"
zones = { rows = [0.5, 0.5], children = [
    { columns = [0.5, 0.5] },
    { columns = [0.5, 0.5] },
]}
"#;

pub fn load(path: &Path) -> Config {
    match fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(path, DEFAULT_CONFIG);
            toml::from_str(DEFAULT_CONFIG).unwrap_or_default()
        }
    }
}

pub fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok().and_then(|m| m.modified().ok())
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
    if cfg.layout.is_empty() {
        let default: Config = toml::from_str(DEFAULT_CONFIG)
            .expect("DEFAULT_CONFIG must always parse");
        return to_layouts(&default);
    }

    let mut slots: Vec<Option<Layout>> = vec![None; WORKSPACE_COUNT];
    let mut unindexed: Vec<Layout> = Vec::new();
    for entry in &cfg.layout {
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

pub fn load_state(path: &Path) -> SavedState {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_state(path: &Path, state: &SavedState) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(s) = toml::to_string(state) {
        let _ = fs::write(path, s);
    }
}

#[cfg(test)]
mod test {
    use crate::{config::{self, to_layouts}, state::workspace::WORKSPACE_COUNT};

    #[test]
    fn default_config_parses_to_some_layouts() {
        let cfg = config::load(std::path::Path::new("/nonexistent/path"));
        let layouts = to_layouts(&cfg);
        assert_eq!(layouts.len(), WORKSPACE_COUNT);
        assert!(layouts.iter().any(|l| l.is_some()), "at least one layout must be populated");
    }

    #[test]
    fn two_column_toml_produces_two_zones() {
        let cfg: crate::config::Config = toml::from_str(r#"
            [[layout]]
            name = "2-col"
            zones = { columns = [0.5, 0.5] }
        "#).unwrap();
        let layouts = config::to_layouts(&cfg);
        let zone_count = layouts[0].as_ref().unwrap().zones.len();
        assert_eq!(zone_count, 2);
    }

    #[test]
    fn explicit_index_places_layout_in_correct_slot() {
        let cfg: crate::config::Config = toml::from_str(r#"
            [[layout]]
            name = "pinned"
            index = 5
            zones = { columns = [1.0] }
        "#).unwrap();
        let layouts = config::to_layouts(&cfg);
        assert!(layouts[4].is_some(), "slot 5 (index 4) should be occupied");
        assert!(layouts[0].is_none(), "slot 1 should be empty");
    }

    #[test]
    fn unindexed_layouts_fill_slots_in_order() {
        let cfg: crate::config::Config = toml::from_str(r#"
            [[layout]]
            name = "A"
            zones = { columns = [1.0] }

            [[layout]]
            name = "B"
            zones = { columns = [0.5, 0.5] }
        "#).unwrap();
        let layouts = config::to_layouts(&cfg);
        assert_eq!(layouts[0].as_ref().unwrap().name, "A");
        assert_eq!(layouts[1].as_ref().unwrap().name, "B");
    }
}
