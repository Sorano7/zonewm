use std::sync::RwLock;

use serde::Deserialize;
use windows::Win32::Foundation::COLORREF;

use crate::config::Config;

#[derive(Deserialize, Default)]
pub struct SnappingEntry {
    #[serde(default)]
    pub gap: Option<i32>,
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

pub struct Settings {
    pub snap_gap: i32,
    pub color_floating: COLORREF,
    pub color_stretched: COLORREF,
    pub color_zoned: COLORREF,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            snap_gap: crate::models::window::SNAP_GAP,
            color_floating: DEFAULT_COLOR_FLOATING,
            color_stretched: DEFAULT_COLOR_STRETCHED,
            color_zoned: DEFAULT_COLOR_ZONED,
        }
    }
}

static SETTINGS: RwLock<Settings> = RwLock::new(Settings {
    snap_gap: crate::models::window::SNAP_GAP,
    color_floating: DEFAULT_COLOR_FLOATING,
    color_stretched: DEFAULT_COLOR_STRETCHED,
    color_zoned: DEFAULT_COLOR_ZONED,
});

fn parse_hex_color(s: &str) -> Option<COLORREF> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 { return None; }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(COLORREF((b as u32) << 16 | (g as u32) << 8 | r as u32))
}

pub fn update(cfg: &Config) {
    let snap_gap = cfg.snapping.as_ref()
        .and_then(|s| s.gap)
        .unwrap_or(crate::models::window::SNAP_GAP);

    let colors = cfg.colors.as_ref();
    let color_floating = colors.and_then(|c| c.floating.as_deref())
        .and_then(parse_hex_color).unwrap_or(DEFAULT_COLOR_FLOATING);
    let color_stretched = colors.and_then(|c| c.stretched.as_deref())
        .and_then(parse_hex_color).unwrap_or(DEFAULT_COLOR_STRETCHED);
    let color_zoned = colors.and_then(|c| c.zoned.as_deref())
        .and_then(parse_hex_color).unwrap_or(DEFAULT_COLOR_ZONED);

    *SETTINGS.write().unwrap() = Settings { snap_gap, color_floating, color_stretched, color_zoned };
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
}
