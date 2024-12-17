use std::{collections::{HashMap, HashSet}, env, error::Error, path::PathBuf};

use macroquad::input::KeyCode;
use serde::{Deserialize, Serialize};

use crate::{input::{self, Action, Hotkey, Modifiers}, pitch::Note, ui::theme::Theme};

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
    pub sample_folder: Option<String>,
    #[serde(default = "default_keys")]
    keys: Vec<(Hotkey, Action)>, // for serialization
    #[serde(skip)]
    key_map: HashMap<Hotkey, Action>, // for use
    #[serde(default = "input::default_note_keys")]
    pub note_keys: Vec<(Hotkey, Note)>,
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
            sample_folder: None,
            keys: default_keys(),
            key_map: HashMap::new(),
            note_keys: input::default_note_keys(),
        }
    }

    pub fn load() -> Result<Self, Box<dyn Error>> {
        let s = std::fs::read_to_string(config_path()?)?;
        let mut c: Self = toml::from_str(&s)?;
        let actions: HashSet<Action> = c.keys.iter().map(|x| x.1).collect();
        for (k, a) in default_keys() {
            if !actions.contains(&a) {
                c.keys.push((k, a));
            }
        }
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

    pub fn action_is_down(&self, action: Action) -> bool {
        for (k, a) in &self.keys {
            if *a == action && k.is_down() {
                return true
            }
        }
        false
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
        (Hotkey::new(Modifiers::None, KeyCode::KpMultiply), Action::IncrementOctave),
        (Hotkey::new(Modifiers::None, KeyCode::KpDivide), Action::DecrementOctave),
        (Hotkey::new(Modifiers::None, KeyCode::F5), Action::PlayFromStart),
        (Hotkey::new(Modifiers::None, KeyCode::F6), Action::PlayFromScreen),
        (Hotkey::new(Modifiers::None, KeyCode::F7), Action::PlayFromCursor),
        (Hotkey::new(Modifiers::None, KeyCode::F8), Action::StopPlayback),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::N), Action::NewSong),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::O), Action::OpenSong),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::S), Action::SaveSong),
        (Hotkey::new(Modifiers::CtrlAlt, KeyCode::S), Action::SaveSongAs),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::E), Action::RenderSong),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Z), Action::Undo),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Y), Action::Redo),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::X), Action::Cut),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::C), Action::Copy),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::V), Action::Paste),
        (Hotkey::new(Modifiers::CtrlShift, KeyCode::V), Action::MixPaste),
        (Hotkey::new(Modifiers::None, KeyCode::Down), Action::NextRow),
        (Hotkey::new(Modifiers::None, KeyCode::Up), Action::PrevRow),
        (Hotkey::new(Modifiers::None, KeyCode::Right), Action::NextColumn),
        (Hotkey::new(Modifiers::None, KeyCode::Left), Action::PrevColumn),
        (Hotkey::new(Modifiers::None, KeyCode::Tab), Action::NextChannel),
        (Hotkey::new(Modifiers::Shift, KeyCode::Tab), Action::PrevChannel),
        (Hotkey::new(Modifiers::None, KeyCode::Delete), Action::Delete),
        (Hotkey::new(Modifiers::None, KeyCode::GraveAccent), Action::NoteOff),
        (Hotkey::new(Modifiers::None, KeyCode::E), Action::End),
        (Hotkey::new(Modifiers::None, KeyCode::L), Action::Loop),
        (Hotkey::new(Modifiers::None, KeyCode::T), Action::TapTempo),
        (Hotkey::new(Modifiers::None, KeyCode::R), Action::RationalTempo),
        (Hotkey::new(Modifiers::None, KeyCode::Insert), Action::InsertRows),
        (Hotkey::new(Modifiers::None, KeyCode::Backspace), Action::DeleteRows),
        (Hotkey::new(Modifiers::None, KeyCode::F2), Action::NudgeArrowUp),
        (Hotkey::new(Modifiers::None, KeyCode::F1), Action::NudgeArrowDown),
        (Hotkey::new(Modifiers::None, KeyCode::Backslash), Action::NudgeSharp),
        (Hotkey::new(Modifiers::None, KeyCode::Minus), Action::NudgeFlat),
        (Hotkey::new(Modifiers::None, KeyCode::F4), Action::NudgeOctaveUp),
        (Hotkey::new(Modifiers::None, KeyCode::F3), Action::NudgeOctaveDown),
        (Hotkey::new(Modifiers::None, KeyCode::Apostrophe), Action::NudgeEnharmonic),
        (Hotkey::new(Modifiers::None, KeyCode::ScrollLock), Action::ToggleFollow),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Tab), Action::NextTab),
        (Hotkey::new(Modifiers::CtrlShift, KeyCode::Tab), Action::PrevTab),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::A), Action::SelectAllChannels),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::P), Action::PlaceEvenly),
        (Hotkey::new(Modifiers::None, KeyCode::PageUp), Action::PrevBeat),
        (Hotkey::new(Modifiers::None, KeyCode::PageDown), Action::NextBeat),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Up), Action::PrevEvent),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Down), Action::NextEvent),
        (Hotkey::new(Modifiers::None, KeyCode::Home), Action::PatternStart),
        (Hotkey::new(Modifiers::None, KeyCode::End), Action::PatternEnd),
        (Hotkey::new(Modifiers::Shift, KeyCode::F2), Action::IncrementValues),
        (Hotkey::new(Modifiers::Shift, KeyCode::F1), Action::DecrementValues),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::I), Action::Interpolate),
        // (Hotkey::new(Modifiers::Ctrl, KeyCode::R), Action::ToggleRecord),
    ]
}