use std::fmt;

use macroquad::input::{is_key_down, KeyCode};
use serde::{Deserialize, Serialize};

use crate::{config::Config, pitch::{Nominal, Note, Tuning}};

pub const CC_MODULATION: u8 = 1;
pub const CC_MACRO_MIN: u8 = 41;
pub const CC_MACRO_MAX: u8 = 48;
pub const CC_RPN_MSB: u8 = 101;
pub const CC_RPN_LSB: u8 = 100;
pub const CC_DATA_ENTRY_MSB: u8 = 6;
pub const CC_DATA_ENTRY_LSB: u8 = 38;
pub const RPN_PITCH_BEND_SENSITIVITY: (u8, u8) = (0, 0);

const DEFAULT_EQUAVE: i8 = 4;

pub fn u8_from_key(k: KeyCode) -> u8 {
    format!("{:?}", k).bytes().last().unwrap_or_default()
}

// sharps aren't much use for keyboard mapping if they're equal to unison
// or the whole tone
fn use_sharps(t: &Tuning) -> bool {
    let d4 = Note::new(0, Nominal::D, 0, 4);
    let ds4 = Note { sharps: 1, ..d4 };
    t.midi_pitch(&d4) != t.midi_pitch(&ds4) &&
        t.midi_pitch(&ds4) != t.midi_pitch(&Note { nominal: Nominal::E, ..d4 })
}

pub fn note_from_key(key: Hotkey, t: &Tuning, equave: i8, cfg: &Config) -> Option<Note> {
    cfg.note_keys.iter()
        .find(|(k, _)| *k == key)
        .map(|(_, n)| {
            let n = if use_sharps(t) { *n } else {
                Note {
                    sharps: 0,
                    arrows: n.sharps,
                    ..*n
                }
            };
            let n = adjust_note_for_modifier_keys(n, cfg);
            Note {
                equave: n.equave + equave - DEFAULT_EQUAVE,
                ..n
            }
        })
}

pub fn default_note_keys() -> Vec<(Hotkey, Note)> {
    let f1 = |key| Hotkey {
        key,
        mods: Modifiers::None,
    };
    let f2 = |nominal, accidentals: i8, offset: i8| Note {
        arrows: 0,
        nominal,
        sharps: accidentals,
        equave: DEFAULT_EQUAVE + offset,
    };
    vec![
        (f1(KeyCode::Z), f2(Nominal::C, 0, -1)),
        (f1(KeyCode::S), f2(Nominal::C, 1, -1)),
        (f1(KeyCode::X), f2(Nominal::D, 0, -1)),
        (f1(KeyCode::D), f2(Nominal::D, 1, -1)),
        (f1(KeyCode::C), f2(Nominal::E, 0, -1)),
        (f1(KeyCode::V), f2(Nominal::F, 0, -1)),
        (f1(KeyCode::G), f2(Nominal::F, 1, -1)),
        (f1(KeyCode::B), f2(Nominal::G, 0, -1)),
        (f1(KeyCode::H), f2(Nominal::G, 1, -1)),
        (f1(KeyCode::N), f2(Nominal::A, 0, -1)),
        (f1(KeyCode::J), f2(Nominal::A, 1, -1)),
        (f1(KeyCode::M), f2(Nominal::B, 0, -1)),
        (f1(KeyCode::Comma), f2(Nominal::C, 0, 0)),
        (f1(KeyCode::L), f2(Nominal::C, 1, 0)),
        (f1(KeyCode::Period), f2(Nominal::D, 0, 0)),
        (f1(KeyCode::Semicolon), f2(Nominal::D, 1, 0)),
        (f1(KeyCode::Slash), f2(Nominal::E, 0, 0)),
        (f1(KeyCode::Q), f2(Nominal::C, 0, 0)),
        (f1(KeyCode::Key2), f2(Nominal::C, 1, 0)),
        (f1(KeyCode::W), f2(Nominal::D, 0, 0)),
        (f1(KeyCode::Key3), f2(Nominal::D, 1, 0)),
        (f1(KeyCode::E), f2(Nominal::E, 0, 0)),
        (f1(KeyCode::R), f2(Nominal::F, 0, 0)),
        (f1(KeyCode::Key5), f2(Nominal::F, 1, 0)),
        (f1(KeyCode::T), f2(Nominal::G, 0, 0)),
        (f1(KeyCode::Key6), f2(Nominal::G, 1, 0)),
        (f1(KeyCode::Y), f2(Nominal::A, 0, 0)),
        (f1(KeyCode::Key7), f2(Nominal::A, 1, 0)),
        (f1(KeyCode::U), f2(Nominal::B, 0, 0)),
        (f1(KeyCode::I), f2(Nominal::C, 0, 1)),
        (f1(KeyCode::Key9), f2(Nominal::C, 1, 1)),
        (f1(KeyCode::O), f2(Nominal::D, 0, 1)),
        (f1(KeyCode::Key0), f2(Nominal::D, 1, 1)),
        (f1(KeyCode::P), f2(Nominal::E, 0, 1)),
        (f1(KeyCode::LeftBracket), f2(Nominal::F, 0, 1)),
        (f1(KeyCode::Equal), f2(Nominal::F, 1, 1)),
        (f1(KeyCode::RightBracket), f2(Nominal::G, 0, 1)),
    ]
}

pub fn note_from_midi(n: u8, t: &Tuning, cfg: &Config) -> Note {
    let f = |nominal, accidentals| {
        adjust_note_for_modifier_keys(Note {
            arrows: if use_sharps(t) { 0 } else { accidentals },
            nominal,
            sharps: if use_sharps(t) { accidentals } else { 0 },
            equave: (n as i8) / 12 - 1,
        }, cfg)
    };
    match n % 12 {
        0 => f(Nominal::C, 0),
        1 => f(Nominal::C, 1),
        2 => f(Nominal::D, 0),
        3 => f(Nominal::D, 1),
        4 => f(Nominal::E, 0),
        5 => f(Nominal::F, 0),
        6 => f(Nominal::F, 1),
        7 => f(Nominal::G, 0),
        8 => f(Nominal::G, 1),
        9 => f(Nominal::A, 0),
        10 => f(Nominal::A, 1),
        11 => f(Nominal::B, 0),
        _ => panic!("unreachable"),
    }
}

pub fn adjust_note_for_modifier_keys(note: Note, cfg: &Config) -> Note {
    let mut note = Note {
        arrows: note.arrows,
        sharps: note.sharps,
        equave: note.equave,
        ..note
    };

    if cfg.action_is_down(Action::NudgeArrowUp) {
        note.arrows += 1;
    }
    if cfg.action_is_down(Action::NudgeArrowDown) {
        note.arrows -= 1;
    }
    if cfg.action_is_down(Action::NudgeSharp) {
        note.sharps += 1;
    }
    if cfg.action_is_down(Action::NudgeFlat) {
        note.sharps -= 1;
    }
    if cfg.action_is_down(Action::NudgeOctaveUp) {
        note.equave += 1;
    }
    if cfg.action_is_down(Action::NudgeOctaveDown) {
        note.equave -= 1;
    }

    if cfg.action_is_down(Action::NudgeEnharmonic) {
        enharmonic_alternative(note)
    } else {
        note
    }
}

fn enharmonic_alternative(note: Note) -> Note {
    let bias = match note.nominal {
        Nominal::E | Nominal::B => 1,
        Nominal::C | Nominal::F => -1,
        _ => 0,
    };
    let ((nominal, equave_offset), sharp_offset) = if note.sharps * 2 + bias >= 0 {
        (note.nominal.next(), if bias > 0 { -1 } else { -2 })
    } else {
        (note.nominal.prev(), if bias < 0 { 1 } else { 2 })
    };
    Note {
        nominal,
        sharps: note.sharps + sharp_offset,
        equave: note.equave + equave_offset,
        ..note
    }
}

// program change is omitted; we have no use for it
pub enum MidiEvent {
    NoteOff {
        channel: u8,
        key: u8,
        // velocity is unused
    },
    NoteOn {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    PolyPressure {
        channel: u8,
        key: u8,
        pressure: u8,
    },
    Controller {
        channel: u8,
        controller: u8,
        value: u8,
    },
    ChannelPressure {
        channel: u8,
        pressure: u8,
    },
    Pitch {
        channel: u8,
        bend: f32,
    },
}

impl MidiEvent {
    const PITCH_CENTER: i16 = 0x2000; // center value of pitch message

    pub fn parse(data: &[u8]) -> Option<Self> {
        // all the messages we're interested in are at least 2 bytes
        if data.len() < 2 { return None }

        let channel = data[0] & 0xf;

        match data[0] & 0xf0 {
            0x80 => Some(Self::NoteOff { channel, key: data[1] }),
            0x90 => Some(Self::NoteOn { channel, key: data[1], velocity: *data.get(2)? }),
            0xa0 => Some(Self::PolyPressure { channel, key: data[1], pressure: *data.get(2)? }),
            0xb0 => Some(Self::Controller { channel, controller: data[1], value: *data.get(2)? }),
            0xd0 => Some(Self::ChannelPressure { channel, pressure: data[1] }),
            0xe0 => Some(Self::Pitch { channel, bend: {
                let raw_pitch = ((*data.get(2)? as i16) << 7) + data[1] as i16;
                (raw_pitch - Self::PITCH_CENTER) as f32 / Self::PITCH_CENTER as f32
            }}),
            _ => None,
        }
    }
}

/// Redefinition of macroquad's KeyCode for serde.
#[derive(Serialize, Deserialize)]
#[serde(remote = "KeyCode")]
enum KeyCodeDef {
    Space = 0x0020,
    Apostrophe = 0x0027,
    Comma = 0x002c,
    Minus = 0x002d,
    Period = 0x002e,
    Slash = 0x002f,
    Key0 = 0x0030,
    Key1 = 0x0031,
    Key2 = 0x0032,
    Key3 = 0x0033,
    Key4 = 0x0034,
    Key5 = 0x0035,
    Key6 = 0x0036,
    Key7 = 0x0037,
    Key8 = 0x0038,
    Key9 = 0x0039,
    Semicolon = 0x003b,
    Equal = 0x003d,
    A = 0x0041,
    B = 0x0042,
    C = 0x0043,
    D = 0x0044,
    E = 0x0045,
    F = 0x0046,
    G = 0x0047,
    H = 0x0048,
    I = 0x0049,
    J = 0x004a,
    K = 0x004b,
    L = 0x004c,
    M = 0x004d,
    N = 0x004e,
    O = 0x004f,
    P = 0x0050,
    Q = 0x0051,
    R = 0x0052,
    S = 0x0053,
    T = 0x0054,
    U = 0x0055,
    V = 0x0056,
    W = 0x0057,
    X = 0x0058,
    Y = 0x0059,
    Z = 0x005a,
    LeftBracket = 0x005b,
    Backslash = 0x005c,
    RightBracket = 0x005d,
    GraveAccent = 0x0060,
    World1 = 0x0100,
    World2 = 0x0101,
    Escape = 0xff1b,
    Enter = 0xff0d,
    Tab = 0xff09,
    Backspace = 0xff08,
    Insert = 0xff63,
    Delete = 0xffff,
    Right = 0xff53,
    Left = 0xff51,
    Down = 0xff54,
    Up = 0xff52,
    PageUp = 0xff55,
    PageDown = 0xff56,
    Home = 0xff50,
    End = 0xff57,
    CapsLock = 0xffe5,
    ScrollLock = 0xff14,
    NumLock = 0xff7f,
    PrintScreen = 0xfd1d,
    Pause = 0xff13,
    F1 = 0xffbe,
    F2 = 0xffbf,
    F3 = 0xffc0,
    F4 = 0xffc1,
    F5 = 0xffc2,
    F6 = 0xffc3,
    F7 = 0xffc4,
    F8 = 0xffc5,
    F9 = 0xffc6,
    F10 = 0xffc7,
    F11 = 0xffc8,
    F12 = 0xffc9,
    F13 = 0xffca,
    F14 = 0xffcb,
    F15 = 0xffcc,
    F16 = 0xffcd,
    F17 = 0xffce,
    F18 = 0xffcf,
    F19 = 0xffd0,
    F20 = 0xffd1,
    F21 = 0xffd2,
    F22 = 0xffd3,
    F23 = 0xffd4,
    F24 = 0xffd5,
    F25 = 0xffd6,
    Kp0 = 0xffb0,
    Kp1 = 0xffb1,
    Kp2 = 0xffb2,
    Kp3 = 0xffb3,
    Kp4 = 0xffb4,
    Kp5 = 0xffb5,
    Kp6 = 0xffb6,
    Kp7 = 0xffb7,
    Kp8 = 0xffb8,
    Kp9 = 0xffb9,
    KpDecimal = 0xffae,
    KpDivide = 0xffaf,
    KpMultiply = 0xffaa,
    KpSubtract = 0xffad,
    KpAdd = 0xffab,
    KpEnter = 0xff8d,
    KpEqual = 0xffbd,
    LeftShift = 0xffe1,
    LeftControl = 0xffe3,
    LeftAlt = 0xffe9,
    LeftSuper = 0xffeb,
    RightShift = 0xffe2,
    RightControl = 0xffe4,
    RightAlt = 0xffea,
    RightSuper = 0xffec,
    Menu = 0xff67,
    // Android back button
    Back = 0xff04,
    Unknown = 0x01ff,
}

/// Combination of modifier keys. This is kind of a silly way to store this
/// information, but it serializes to TOML a lot nicer than a struct of three
/// booleans.
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum Modifiers {
    None,
    Ctrl,
    Alt,
    Shift,
    CtrlAlt,
    CtrlShift,
    AltShift,
    CtrlAltShift,
}

impl Modifiers {
    pub fn current() -> Self {
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        match (ctrl, alt, shift) {
            (false, false, false) => Self::None,
            (true, false, false) => Self::Ctrl,
            (false, true, false) => Self::Alt,
            (false, false, true) => Self::Shift,
            (true, true, false) => Self::CtrlAlt,
            (true, false, true) => Self::CtrlShift,
            (false, true, true) => Self::AltShift,
            (true, true, true) => Self::CtrlAltShift,
        }
    }

    pub fn without_shift(&self) -> Self {
        match self {
            Self::Shift => Self::None,
            Self::CtrlShift => Self::Ctrl,
            Self::AltShift => Self::Alt,
            Self::CtrlAltShift => Self::CtrlAlt,
            _ => *self,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct Hotkey {
    pub mods: Modifiers,
    #[serde(with = "KeyCodeDef")]
    pub key: KeyCode,
}

impl Hotkey {
    pub fn new(mods: Modifiers, key: KeyCode) -> Self {
        Self { mods, key }
    }

    pub fn is_down(&self) -> bool {
        is_key_down(self.key) && self.mods == Modifiers::current()
    }

    pub fn without_shift(&self) -> Self {
        Self { mods: self.mods.without_shift(), key: self.key }
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.mods == Modifiers::None {
            write!(f, "{:?}", self.key)
        } else {
            write!(f, "{:?}+{:?}", self.mods, self.key)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    IncrementDivision,
    DecrementDivision,
    DoubleDivision,
    HalveDivision,
    IncrementOctave,
    DecrementOctave,
    PlayFromStart,
    PlayFromScreen,
    PlayFromCursor,
    StopPlayback,
    NewSong,
    OpenSong,
    SaveSong,
    SaveSongAs,
    RenderSong,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    MixPaste,
    NextRow,
    PrevRow,
    NextColumn,
    PrevColumn,
    NextChannel,
    PrevChannel,
    Delete,
    NoteOff,
    End,
    Loop,
    TapTempo,
    RationalTempo,
    InsertRows,
    DeleteRows,
    NudgeArrowUp,
    NudgeArrowDown,
    NudgeSharp,
    NudgeFlat,
    NudgeOctaveUp,
    NudgeOctaveDown,
    NudgeEnharmonic,
    ToggleFollow,
    ToggleRecord,
    NextTab,
    PrevTab,
    SelectAllChannels,
    PlaceEvenly,
    NextBeat,
    PrevBeat,
    NextEvent,
    PrevEvent,
    PatternStart,
    PatternEnd,
    IncrementValues,
    DecrementValues,
    Interpolate,
    MuteTrack,
    SoloTrack,
    UnmuteAllTracks,
}

impl Action {
    pub fn name(&self) -> &'static str {
        match self {
            Self::IncrementDivision => "Increment division",
            Self::DecrementDivision => "Decrement division",
            Self::DoubleDivision => "Double division",
            Self::HalveDivision => "Halve division",
            Self::IncrementOctave => "Increment octave",
            Self::DecrementOctave => "Decrement octave",
            Self::PlayFromStart => "Play from start",
            Self::PlayFromScreen => "Play from screen",
            Self::PlayFromCursor => "Play from cursor",
            Self::StopPlayback => "Stop playback",
            Self::NewSong => "New song",
            Self::OpenSong => "Open song...",
            Self::SaveSong => "Save song",
            Self::SaveSongAs => "Save song as...",
            Self::RenderSong => "Render song",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::MixPaste => "Mix paste",
            Self::NextRow => "Next row",
            Self::PrevRow => "Previous row",
            Self::NextColumn => "Next column",
            Self::PrevColumn => "Previous column",
            Self::NextChannel => "Next channel",
            Self::PrevChannel => "Previous channel",
            Self::Delete => "Delete",
            Self::NoteOff => "Insert note off event",
            Self::End => "Insert end event",
            Self::Loop => "Insert loop event",
            Self::TapTempo => "Insert tap tempo event",
            Self::RationalTempo => "Insert rational tempo event",
            Self::InsertRows => "Insert rows",
            Self::DeleteRows => "Delete rows",
            Self::NudgeArrowUp => "Arrow up",
            Self::NudgeArrowDown => "Arrow down",
            Self::NudgeSharp => "Sharp",
            Self::NudgeFlat => "Flat",
            Self::NudgeOctaveUp => "Octave up",
            Self::NudgeOctaveDown => "Octave down",
            Self::NudgeEnharmonic => "Enharmonic swap",
            Self::ToggleFollow => "Toggle pattern follow",
            Self::ToggleRecord => "Record",
            Self::NextTab => "Next tab",
            Self::PrevTab => "Previous tab",
            Self::SelectAllChannels => "Select all channels",
            Self::PlaceEvenly => "Place events evenly",
            Self::NextBeat => "Next beat",
            Self::PrevBeat => "Prev beat",
            Self::NextEvent => "Next event",
            Self::PrevEvent => "Prev event",
            Self::PatternStart => "Go to pattern start",
            Self::PatternEnd => "Go to pattern end",
            Self::IncrementValues => "Increment values",
            Self::DecrementValues => "Decrement values",
            Self::Interpolate => "Interpolate",
            Self::MuteTrack => "Mute track",
            Self::SoloTrack => "Solo track",
            Self::UnmuteAllTracks => "Unmute all tracks",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tuning_uses_sharps() {
        assert!(use_sharps(&Tuning::divide(2.0, 12, 1).unwrap()));
        assert!(use_sharps(&Tuning::divide(2.0, 13, 1).unwrap()));
        assert!(!use_sharps(&Tuning::divide(2.0, 10, 1).unwrap()));
        assert!(!use_sharps(&Tuning::divide(2.0, 14, 1).unwrap()));
    }
}