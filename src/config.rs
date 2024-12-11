use std::{collections::HashMap, env, error::Error, path::PathBuf};

use macroquad::input::KeyCode;
use serde::{Deserialize, Serialize};

use crate::{input::{Action, Hotkey, Modifiers}, ui::theme::Theme};

const CONFIG_FILENAME: &str = "config.toml";

fn config_path() -> Result<PathBuf, std::io::Error> {
    let mut path = env::current_exe()?;
    path.pop();
    path.push(CONFIG_FILENAME);
    Ok(path)
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub default_midi_input: Option<String>,
    pub midi_send_pressure: Option<bool>,
    pub theme: Option<Theme>,
    pub module_folder: Option<String>,
    pub patch_folder: Option<String>,
    pub render_folder: Option<String>,
    pub scale_folder: Option<String>,
    #[serde(default = "default_keys")]
    keys: Vec<(Hotkey, Action)>, // for serialization
    #[serde(skip)]
    key_map: HashMap<Hotkey, Action>, // for use
}

impl Config {
    pub fn default() -> Self {
        Self {
            default_midi_input: None,
            midi_send_pressure: Some(true),
            theme: None,
            module_folder: None,
            patch_folder: None,
            render_folder: None,
            scale_folder: None,
            keys: default_keys(),
            key_map: HashMap::new(),
        }
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let s = std::fs::read_to_string(config_path()?)?;
        let mut c: Self = toml::from_str(&s)?;
        c.key_map = c.keys.iter().cloned().collect();
        Ok(c)
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let s = toml::to_string_pretty(self)?;
        std::fs::write(config_path()?, s)?;
        Ok(())
    }

    pub fn iter_keymap(&mut self) -> impl Iterator<Item = &mut (Hotkey, Action)> {
        self.keys.iter_mut()
    }

    pub fn hotkey_action(&self, hotkey: &Hotkey) -> Option<&Action> {
        self.key_map.get(hotkey)
    }

    pub fn update_hotkeys(&mut self) {
        self.key_map = self.keys.iter().cloned().collect();
    }
}

pub fn dir_as_string(p: &PathBuf) -> Option<String> {
    p.parent().map(|p| p.to_str().map(|s| s.to_owned())).flatten()
}

fn default_keys() -> Vec<(Hotkey, Action)> {
    vec![
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Equal), Action::IncrementDivision),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Minus), Action::DecrementDivision),
        (Hotkey::new(Modifiers::Alt, KeyCode::Equal), Action::DoubleDivision),
        (Hotkey::new(Modifiers::Alt, KeyCode::Minus), Action::HalveDivision),
    ]
}