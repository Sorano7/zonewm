use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::default::DEFAULT_CONFIG;
use crate::config::keymap::KeymapEntry;
use crate::config::layout::LayoutEntry;

pub mod layout;
pub mod keymap;
pub mod default;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid config: {0}")]
    Invalid(String)
}

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    layout: Option<Vec<LayoutEntry>>,
    keymap: Option<Vec<KeymapEntry>>,
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
    use crate::{config::{self, layout::to_layouts}, state::workspace::WORKSPACE_COUNT};

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
        let layouts = to_layouts(&cfg);
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
        let layouts = to_layouts(&cfg);
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
        let layouts = to_layouts(&cfg);
        assert_eq!(layouts[0].as_ref().unwrap().name, "A");
        assert_eq!(layouts[1].as_ref().unwrap().name, "B");
    }
}
