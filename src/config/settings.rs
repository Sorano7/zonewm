use std::sync::RwLock;

use serde::Deserialize;

use crate::config::Config;

#[derive(Deserialize, Default)]
pub struct SnappingEntry {
    #[serde(default)]
    pub gap: Option<i32>,
}

pub struct Settings {
    pub snap_gap: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { snap_gap: crate::models::window::SNAP_GAP }
    }
}

static SETTINGS: RwLock<Settings> = RwLock::new(Settings { snap_gap: crate::models::window::SNAP_GAP });

pub fn update(cfg: &Config) {
    let snap_gap = cfg.snapping.as_ref()
        .and_then(|s| s.gap)
        .unwrap_or(crate::models::window::SNAP_GAP);
    *SETTINGS.write().unwrap() = Settings { snap_gap };
}

pub fn snap_gap() -> i32 {
    SETTINGS.read().unwrap().snap_gap
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn missing_snapping_section_falls_back_to_default_gap() {
        let cfg: Config = toml::from_str("").unwrap();
        update(&cfg);
        assert_eq!(snap_gap(), crate::models::window::SNAP_GAP);
    }

    #[test]
    fn snapping_gap_is_read_from_config() {
        let cfg: Config = toml::from_str(r#"
            [snapping]
            gap = 7
        "#).unwrap();
        update(&cfg);
        assert_eq!(snap_gap(), 7);
    }
}
