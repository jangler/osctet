use std::{env, error::Error, path::PathBuf};

use serde::{Serialize, Deserialize};

use crate::ui::theme::Theme;

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
        }
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let s = std::fs::read_to_string(config_path()?)?;
        let c = toml::from_str(&s)?;
        Ok(c)
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let s = toml::to_string(self)?;
        std::fs::write(config_path()?, s)?;
        Ok(())
    }
}

pub fn dir_as_string(p: &PathBuf) -> Option<String> {
    p.parent().map(|p| p.to_str().map(|s| s.to_owned())).flatten()
}