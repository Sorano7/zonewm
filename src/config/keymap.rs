use serde::Deserialize;
use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_DOWN, VK_ESCAPE, VK_F1, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB, VK_UP};

use crate::{actions::{Action, keymap::{Keymap, KeymapError, KeymapRegistry}}, config::{Config, ConfigError, default::DEFAULT_CONFIG}};

#[derive(Deserialize)]
pub struct KeymapEntry {
    combo: String,
    action: String,
}

fn mod_bit(tok: &str) -> Option<u32> {
    match tok {
        "ctrl" | "control" => Some(MOD_CONTROL.0),
        "alt"              => Some(MOD_ALT.0),
        "shift"            => Some(MOD_SHIFT.0),
        "win" | "super"    => Some(MOD_WIN.0),
        _ => None,
    }
}

fn vk_from_name(tok: &str) -> Option<u32> {
    if tok.len() == 1 {
        let c = tok.chars().next().unwrap();
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_uppercase() as u32);
        }
    }
    if let Some(n) = tok.strip_prefix('f').and_then(|s| s.parse::<u32>().ok()) {
        if (1..=24).contains(&n) {
            return Some(VK_F1.0 as u32 + n - 1);
        }
    }
    match tok {
        "left"             => Some(VK_LEFT.0 as u32),
        "right"            => Some(VK_RIGHT.0 as u32),
        "up"               => Some(VK_UP.0 as u32),
        "down"             => Some(VK_DOWN.0 as u32),
        "space"            => Some(VK_SPACE.0 as u32),
        "enter" | "return" => Some(VK_RETURN.0 as u32),
        "esc" | "escape"   => Some(VK_ESCAPE.0 as u32),
        "tab"              => Some(VK_TAB.0 as u32),
        _                  => None,
    }
}

fn parse_keymap(s: &str) -> Result<Keymap, KeymapError> {
    let toks: Vec<&str> = s.split("+").collect();
    let mut mods = 0u32;
    let mut vk = 0u32;

    for (i, tok) in toks.iter().enumerate() {
        if i < toks.len() - 1 {
            match mod_bit(tok) {
                Some(m) => mods |= m,
                None => return Err(KeymapError::Invalid),
            };
        } else {
            match vk_from_name(tok) {
                Some(v) => vk = v,
                None => return Err(KeymapError::Invalid),
            }
        }
    }
    Ok(Keymap { mods: mods, vk: vk })
}

pub fn to_keymaps(cfg: &Config) -> Result<KeymapRegistry, ConfigError> {
    let Some(keymap) = &cfg.keymap else {
        let default: Config = toml::from_str(DEFAULT_CONFIG)
            .expect("DEFAULT_CONFIG must always parse");
        return to_keymaps(&default);
    };

    let mut reg = KeymapRegistry::new();
    for entry in keymap {
        let Some(action) = Action::from_string(&entry.action) else {
            return Err(ConfigError::Invalid(format!("unknown action '{}'", &entry.action).into()));
        };

        let keymap = match parse_keymap(&entry.combo) {
            Ok(k) => k,
            Err(e) => {
                let msg = format!("{} for action '{}'", e.to_string(), action.to_string());
                return Err(ConfigError::Invalid(msg));
            },
        };

        if let Err(e) = reg.add_keymap(keymap, action) {
            return Err(ConfigError::Invalid(e.to_string()));
        }
    }
    Ok(reg)
}
