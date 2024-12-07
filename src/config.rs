use std::error::Error;

use serde::{Serialize, Deserialize};

use crate::ui::theme::Theme;

const CONFIG_PATH: &str = "config.toml";

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub default_midi_input: Option<String>,
    pub midi_send_pressure: Option<bool>,
    pub theme: Option<Theme>,
}

impl Config {
    pub fn default() -> Self {
        Self {
            default_midi_input: None,
            midi_send_pressure: Some(true),
            theme: None,
        }
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let s = std::fs::read_to_string(CONFIG_PATH)?;
        let c = toml::from_str(&s)?;
        Ok(c)
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let s = toml::to_string(self)?;
        std::fs::write(CONFIG_PATH, s)?;
        Ok(())
    }
}