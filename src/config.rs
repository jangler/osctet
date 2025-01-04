use std::{collections::{HashMap, HashSet}, env, error::Error, path::PathBuf};

use macroquad::input::KeyCode;
use serde::{Deserialize, Serialize};

use crate::{input::{self, Action, Hotkey, Modifiers}, pitch::Note, ui::theme::Theme};

const CONFIG_FILENAME: &str = "config.toml";

// this is a function instead of a constant to make serde happy
fn default_font_size() -> usize { 1 }

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
    #[serde(default = "default_font_size")]
    pub font_size: usize,
    pub smooth_playhead: bool,
    pub display_info: bool,
    pub desired_sample_rate: u32,
}

impl Config {
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

    /// Reset all settings except paths to defaults.
    pub fn reset(&mut self) {
        *self = Self {
            module_folder: self.module_folder.clone(),
            patch_folder: self.patch_folder.clone(),
            render_folder: self.render_folder.clone(),
            scale_folder: self.scale_folder.clone(),
            sample_folder: self.sample_folder.clone(),
            ..Default::default()
        };
    }

    pub fn save(&mut self, theme: Theme) -> Result<(), Box<dyn Error>> {
        self.theme = Some(theme);
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

    pub fn hotkey_string(&self, action: Action) -> String {
        let key_string = self.keys.iter()
            .find(|(_, a)| *a == action)
            .map(|(k, _)| k.to_string())
            .unwrap_or(String::from("(no hotkey)"));
        format!("{} - {}", key_string, action.name())
    }
}

impl Default for Config {
    fn default() -> Self {
        let keys = default_keys();
        Self {
            default_midi_input: None,
            midi_send_pressure: Some(true),
            theme: None,
            module_folder: None,
            patch_folder: None,
            render_folder: None,
            scale_folder: None,
            sample_folder: None,
            key_map: keys.iter().cloned().collect(),
            keys,
            note_keys: input::default_note_keys(),
            font_size: default_font_size(),
            smooth_playhead: false,
            display_info: true,
            desired_sample_rate: 48000,
        }
    }
}

pub fn dir_as_string(p: &PathBuf) -> Option<String> {
    p.parent().map(|p| p.to_str().map(|s| s.to_owned())).flatten()
}

fn default_keys() -> Vec<(Hotkey, Action)> {
    vec![
        // global
        (Hotkey::new(Modifiers::Ctrl, KeyCode::N), Action::NewSong),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::O), Action::OpenSong),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::S), Action::SaveSong),
        (Hotkey::new(Modifiers::CtrlShift, KeyCode::S), Action::SaveSongAs),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::E), Action::RenderSong),
        (Hotkey::new(Modifiers::CtrlShift, KeyCode::E), Action::RenderTracks),
        (Hotkey::new(Modifiers::CtrlShift, KeyCode::Tab), Action::PrevTab),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Tab), Action::NextTab),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Z), Action::Undo),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Y), Action::Redo),

        // status
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Minus), Action::DecrementDivision),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Equal), Action::IncrementDivision),
        (Hotkey::new(Modifiers::Alt, KeyCode::Minus), Action::HalveDivision),
        (Hotkey::new(Modifiers::Alt, KeyCode::Equal), Action::DoubleDivision),
        (Hotkey::new(Modifiers::Shift, KeyCode::Key9), Action::DecrementOctave),
        (Hotkey::new(Modifiers::Shift, KeyCode::Key0), Action::IncrementOctave),

        // pattern nav
        (Hotkey::new(Modifiers::None, KeyCode::Up), Action::PrevRow),
        (Hotkey::new(Modifiers::None, KeyCode::Down), Action::NextRow),
        (Hotkey::new(Modifiers::None, KeyCode::Left), Action::PrevColumn),
        (Hotkey::new(Modifiers::None, KeyCode::Right), Action::NextColumn),
        (Hotkey::new(Modifiers::Shift, KeyCode::Tab), Action::PrevChannel),
        (Hotkey::new(Modifiers::None, KeyCode::Tab), Action::NextChannel),
        (Hotkey::new(Modifiers::None, KeyCode::PageUp), Action::PrevBeat),
        (Hotkey::new(Modifiers::None, KeyCode::PageDown), Action::NextBeat),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Up), Action::PrevEvent),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Down), Action::NextEvent),
        (Hotkey::new(Modifiers::None, KeyCode::Home), Action::PatternStart),
        (Hotkey::new(Modifiers::None, KeyCode::End), Action::PatternEnd),

        // events
        (Hotkey::new(Modifiers::None, KeyCode::Space), Action::UseLastNote),
        (Hotkey::new(Modifiers::None, KeyCode::GraveAccent), Action::NoteOff),
        (Hotkey::new(Modifiers::None, KeyCode::T), Action::TapTempo),
        (Hotkey::new(Modifiers::None, KeyCode::R), Action::RationalTempo),
        (Hotkey::new(Modifiers::None, KeyCode::L), Action::Loop),
        (Hotkey::new(Modifiers::None, KeyCode::E), Action::End),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::GraveAccent), Action::Interpolate),

        // pitch & notation
        (Hotkey::new(Modifiers::None, KeyCode::F1), Action::DecrementValues),
        (Hotkey::new(Modifiers::None, KeyCode::F2), Action::IncrementValues),
        (Hotkey::new(Modifiers::None, KeyCode::F3), Action::NudgeOctaveDown),
        (Hotkey::new(Modifiers::None, KeyCode::F4), Action::NudgeOctaveUp),
        (Hotkey::new(Modifiers::None, KeyCode::LeftBracket), Action::NudgeArrowDown),
        (Hotkey::new(Modifiers::None, KeyCode::RightBracket), Action::NudgeArrowUp),
        (Hotkey::new(Modifiers::None, KeyCode::Minus), Action::NudgeFlat),
        (Hotkey::new(Modifiers::None, KeyCode::Equal), Action::NudgeSharp),
        (Hotkey::new(Modifiers::None, KeyCode::Apostrophe), Action::NudgeEnharmonic),
        (Hotkey::new(Modifiers::None, KeyCode::Backslash), Action::CycleNotation),

        // clipboard
        (Hotkey::new(Modifiers::Ctrl, KeyCode::X), Action::Cut),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::C), Action::Copy),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::V), Action::Paste),
        (Hotkey::new(Modifiers::CtrlShift, KeyCode::V), Action::MixPaste),
        (Hotkey::new(Modifiers::CtrlAlt, KeyCode::V), Action::InsertPaste),

        // playback
        (Hotkey::new(Modifiers::Ctrl, KeyCode::Enter), Action::PlayFromStart),
        (Hotkey::new(Modifiers::Shift, KeyCode::Enter), Action::PlayFromLoop),
        (Hotkey::new(Modifiers::None, KeyCode::Enter), Action::PlayFromCursor),
        (Hotkey::new(Modifiers::None, KeyCode::ScrollLock), Action::ToggleFollow),
        (Hotkey::new(Modifiers::None, KeyCode::F9), Action::MuteTrack),
        (Hotkey::new(Modifiers::None, KeyCode::F10), Action::SoloTrack),
        (Hotkey::new(Modifiers::None, KeyCode::F11), Action::UnmuteAllTracks),
        (Hotkey::new(Modifiers::None, KeyCode::F12), Action::Panic),

        // misc. pattern
        (Hotkey::new(Modifiers::None, KeyCode::Delete), Action::Delete),
        (Hotkey::new(Modifiers::None, KeyCode::Insert), Action::InsertRows),
        (Hotkey::new(Modifiers::None, KeyCode::Backspace), Action::DeleteRows),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::A), Action::SelectAllChannels),
        (Hotkey::new(Modifiers::Ctrl, KeyCode::P), Action::PlaceEvenly),
    ]
}