use std::collections::HashMap;
use thiserror::Error;

use crate::actions::Action;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct Keymap {
    pub mods: u32,
    pub vk:   u32,
}

#[derive(Error, Debug)]
pub enum KeymapError {
    #[error("Invalid key combo")]
    Invalid,
    #[error("Duplicate keymap found for action '{0}' and '{1}'")]
    Duplicate(String, String),
}

pub struct KeymapRegistry {
    keymap_to_id: HashMap<Keymap, i32>,
    id_to_action: Vec<Action>,
}

impl KeymapRegistry {
    pub fn new() -> Self {
        KeymapRegistry {
            keymap_to_id: HashMap::new(),
            id_to_action: Vec::new(),
        }
    }

    pub fn add_keymap(&mut self, keymap: Keymap, action: Action) -> Result<(), KeymapError> {
        self.id_to_action.push(action);
        let id = (self.id_to_action.len() - 1) as i32;
        if let Some(prev) = self.keymap_to_id.insert(keymap, id) {
            let prev_action = self.id_to_action.get(prev as usize).map_or("none".to_string(), |a| a.to_string());
            Err(KeymapError::Duplicate(prev_action, action.to_string()))
        } else {
            Ok(())
        }
    }

    pub fn get_id_from_keymap(&self, keymap: Keymap) -> Option<i32> {
        self.keymap_to_id.get(&keymap).map(|&i| i)
    }

    pub fn get_action_from_id(&self, id: i32) -> Option<Action> {
        self.id_to_action.get(id as usize).map(|&a| a)
    }
}
