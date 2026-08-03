use std::sync::RwLock;

use serde::Deserialize;
use windows::Win32::Foundation::COLORREF;

use crate::config::Config;

#[derive(Deserialize, Default)]
pub struct SnappingEntry {
    #[serde(default)]
    pub gap: Option<i32>,
    #[serde(default)]
    pub auto_snap_strength: Option<f32>,
}

#[derive(Deserialize, Default)]
pub struct ColorsEntry {
    #[serde(default)]
    pub floating: Option<String>,
    #[serde(default)]
    pub stretched: Option<String>,
    #[serde(default)]
    pub zoned: Option<String>,
}

const DEFAULT_COLOR_FLOATING:  COLORREF = COLORREF(0x0067B051);
const DEFAULT_COLOR_STRETCHED: COLORREF = COLORREF(0x0058C5ED);
const DEFAULT_COLOR_ZONED:     COLORREF = COLORREF(0x00FFA269);

const DEFAULT_AUTO_SNAP_STRENGTH: f32 = 0.5;

pub struct Settings {
    pub snap_gap: i32,
    pub color_floating: COLORREF,
    pub color_stretched: COLORREF,
    pub color_zoned: COLORREF,
    pub max_pos_delta: i32,
    pub max_size_delta: i32,
    pub auto_snap_threshold: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            snap_gap: crate::models::window::SNAP_GAP,
            color_floating: DEFAULT_COLOR_FLOATING,
            color_stretched: DEFAULT_COLOR_STRETCHED,
            color_zoned: DEFAULT_COLOR_ZONED,
            max_pos_delta: crate::models::zone::MAX_POS_DELTA,
            max_size_delta: crate::models::zone::MAX_SIZE_DELTA,
            auto_snap_threshold: crate::models::zone::AUTO_SNAP_THRESHOLD,
        }
    }
}

static SETTINGS: RwLock<Settings> = RwLock::new(Settings {
    snap_gap: crate::models::window::SNAP_GAP,
    color_floating: DEFAULT_COLOR_FLOATING,
    color_stretched: DEFAULT_COLOR_STRETCHED,
    color_zoned: DEFAULT_COLOR_ZONED,
    max_pos_delta: crate::models::zone::MAX_POS_DELTA,
    max_size_delta: crate::models::zone::MAX_SIZE_DELTA,
    auto_snap_threshold: crate::models::zone::AUTO_SNAP_THRESHOLD,
});

/// Maps a 0-1 strength to a tolerance in the same unit as base.
fn strength_to_threshold(strength: f32, base: i32) -> i32 {
    let s = strength.clamp(0.0, 1.0);
    if s >= 1.0 {
        return i32::MAX;
    }
    let value = base as f32 * (s / (1.0 - s));
    if value >= i32::MAX as f32 { i32::MAX } else { value.round() as i32 }
}

fn parse_hex_color(s: &str) -> Option<COLORREF> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 { return None; }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(COLORREF((b as u32) << 16 | (g as u32) << 8 | r as u32))
}

pub fn update(cfg: &Config) {
    let snapping = cfg.snapping.as_ref();
    let snap_gap = snapping.and_then(|s| s.gap)
        .unwrap_or(crate::models::window::SNAP_GAP);
    let auto_snap_strength = snapping.and_then(|s| s.auto_snap_strength)
        .unwrap_or(DEFAULT_AUTO_SNAP_STRENGTH);

    let colors = cfg.colors.as_ref();
    let color_floating = colors.and_then(|c| c.floating.as_deref())
        .and_then(parse_hex_color).unwrap_or(DEFAULT_COLOR_FLOATING);
    let color_stretched = colors.and_then(|c| c.stretched.as_deref())
        .and_then(parse_hex_color).unwrap_or(DEFAULT_COLOR_STRETCHED);
    let color_zoned = colors.and_then(|c| c.zoned.as_deref())
        .and_then(parse_hex_color).unwrap_or(DEFAULT_COLOR_ZONED);

    let max_pos_delta = strength_to_threshold(auto_snap_strength, crate::models::zone::MAX_POS_DELTA);
    let max_size_delta = strength_to_threshold(auto_snap_strength, crate::models::zone::MAX_SIZE_DELTA);
    let auto_snap_threshold = strength_to_threshold(auto_snap_strength, crate::models::zone::AUTO_SNAP_THRESHOLD);

    *SETTINGS.write().unwrap() = Settings {
        snap_gap, color_floating, color_stretched, color_zoned,
        max_pos_delta, max_size_delta, auto_snap_threshold,
    };
}

pub fn snap_gap() -> i32 {
    SETTINGS.read().unwrap().snap_gap
}

pub fn color_floating() -> COLORREF {
    SETTINGS.read().unwrap().color_floating
}

pub fn color_stretched() -> COLORREF {
    SETTINGS.read().unwrap().color_stretched
}

pub fn color_zoned() -> COLORREF {
    SETTINGS.read().unwrap().color_zoned
}

pub fn max_pos_delta() -> i32 {
    SETTINGS.read().unwrap().max_pos_delta
}

pub fn max_size_delta() -> i32 {
    SETTINGS.read().unwrap().max_size_delta
}

pub fn auto_snap_threshold() -> i32 {
    SETTINGS.read().unwrap().auto_snap_threshold
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

    #[test]
    fn missing_colors_section_falls_back_to_defaults() {
        let cfg: Config = toml::from_str("").unwrap();
        update(&cfg);
        assert_eq!(color_floating().0, DEFAULT_COLOR_FLOATING.0);
        assert_eq!(color_stretched().0, DEFAULT_COLOR_STRETCHED.0);
        assert_eq!(color_zoned().0, DEFAULT_COLOR_ZONED.0);
    }

    #[test]
    fn colors_are_read_from_config_as_rgb_hex() {
        let cfg: Config = toml::from_str(r##"
            [colors]
            floating = "#112233"
            stretched = "#aabbcc"
            zoned = "445566"
        "##).unwrap();
        update(&cfg);
        assert_eq!(color_floating().0, 0x00332211);
        assert_eq!(color_stretched().0, 0x00ccbbaa);
        assert_eq!(color_zoned().0, 0x00665544);
    }

    #[test]
    fn invalid_hex_color_falls_back_to_default() {
        let cfg: Config = toml::from_str(r#"
            [colors]
            floating = "not-a-color"
        "#).unwrap();
        update(&cfg);
        assert_eq!(color_floating().0, DEFAULT_COLOR_FLOATING.0);
    }

    #[test]
    fn parse_hex_color_handles_leading_hash_and_case() {
        assert_eq!(parse_hex_color("#FFFFFF").map(|c| c.0), Some(0x00FFFFFF));
        assert_eq!(parse_hex_color("000000").map(|c| c.0), Some(0));
        assert_eq!(parse_hex_color("#zzzzzz"), None);
        assert_eq!(parse_hex_color("#fff"), None);
    }

    #[test]
    fn strength_zero_never_snaps() {
        assert_eq!(strength_to_threshold(0.0, 200), 0);
    }

    #[test]
    fn strength_half_reproduces_the_base_value() {
        assert_eq!(strength_to_threshold(0.5, 200), 200);
    }

    #[test]
    fn strength_one_always_snaps() {
        assert_eq!(strength_to_threshold(1.0, 200), i32::MAX);
    }

    #[test]
    fn strength_out_of_range_is_clamped() {
        assert_eq!(strength_to_threshold(-1.0, 200), 0);
        assert_eq!(strength_to_threshold(2.0, 200), i32::MAX);
    }

    #[test]
    fn missing_auto_snap_strength_falls_back_to_default() {
        let cfg: Config = toml::from_str("").unwrap();
        update(&cfg);
        assert_eq!(max_pos_delta(), crate::models::zone::MAX_POS_DELTA);
        assert_eq!(max_size_delta(), crate::models::zone::MAX_SIZE_DELTA);
        assert_eq!(auto_snap_threshold(), crate::models::zone::AUTO_SNAP_THRESHOLD);
    }

    #[test]
    fn auto_snap_strength_is_read_from_config() {
        let cfg: Config = toml::from_str(r#"
            [snapping]
            auto_snap_strength = 1.0
        "#).unwrap();
        update(&cfg);
        assert_eq!(max_pos_delta(), i32::MAX);
        assert_eq!(max_size_delta(), i32::MAX);
        assert_eq!(auto_snap_threshold(), i32::MAX);
    }
}
