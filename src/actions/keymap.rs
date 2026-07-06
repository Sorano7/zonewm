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
        if self.keymap_to_id.contains_key(&keymap) {
            let prev = self.keymap_to_id.get(&keymap)
                .and_then(|&i| self.id_to_action.get(i as usize))
                .map_or("none".to_string(), |a| a.to_string());
            Err(KeymapError::Duplicate(prev, action.to_string()))
        } else {
            self.id_to_action.push(action);
            self.keymap_to_id.insert(keymap, (self.id_to_action.len() - 1) as i32);
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

#[cfg(test)]
mod test {
    use crate::actions::{Action, keymap::{Keymap, KeymapRegistry}};

    #[test]
    fn add_keymap_errors_on_duplicate_keymap() {
        let mut r = KeymapRegistry::new();
        let k = Keymap { mods: 0, vk: 0 };
        let a1 = Action::SetLayout(1);
        let a2 = Action::SetLayout(2);

        r.add_keymap(k, a1).unwrap();
        assert!(r.add_keymap(k, a2).is_err());
        assert!(r.get_id_from_keymap(k).unwrap() == 0);

        assert!(r.keymap_to_id.len() == 1);
        assert!(r.id_to_action.len() == 1);
    }

    #[test]
    fn add_keymap_allow_duplicate_action() {
        let mut r = KeymapRegistry::new();
        let k1 = Keymap { mods: 0, vk: 0 };
        let k2 = Keymap { mods: 1, vk: 1 };
        let a = Action::SetLayout(1);

        r.add_keymap(k1, a).unwrap();
        r.add_keymap(k2, a).unwrap();

        let id1 = r.get_id_from_keymap(k1).unwrap();
        let id2 = r.get_id_from_keymap(k2).unwrap();
        
        let a1 = r.get_action_from_id(id1).unwrap();
        let a2 = r.get_action_from_id(id2).unwrap();

        assert!(a1 == a2);
    }
}
