use macroquad::input::KeyCode;

use crate::pitch::{Nominal, Note, Tuning};

pub const CC_MODULATION: u8 = 1;
pub const CC_MACRO_MIN: u8 = 41;
pub const CC_MACRO_MAX: u8 = 48;
pub const CC_RPN_MSB: u8 = 101;
pub const CC_RPN_LSB: u8 = 100;
pub const CC_DATA_ENTRY_MSB: u8 = 6;
pub const CC_DATA_ENTRY_LSB: u8 = 38;
pub const RPN_PITCH_BEND_SENSITIVITY: (u8, u8) = (0, 0);

pub fn u8_from_key(k: KeyCode) -> u8 {
    format!("{:?}", k).bytes().last().unwrap_or_default()
}

// sharps aren't much use for keyboard mapping if they're equal to unison
// or the whole tone
fn use_sharps(t: &Tuning) -> bool {
    let d4 = Note::new(0, Nominal::D, 0, 4);
    let ds4 = Note { demisharps: 2, ..d4 };
    t.midi_pitch(&d4) != t.midi_pitch(&ds4) &&
        t.midi_pitch(&ds4) != t.midi_pitch(&Note { nominal: Nominal::E, ..d4 })
}

pub fn note_from_key(k: KeyCode, t: &Tuning, equave: i8) -> Option<Note> {
    let f = |nominal, accidentals, offset| {
        Some(Note {
            arrows: if use_sharps(t) { 0 } else { accidentals },
            nominal,
            demisharps: if use_sharps(t) { accidentals * 2} else { 0 },
            equave: equave + offset,
        })
    };
    match k {
        KeyCode::Z => f(Nominal::C, 0, -1),
        KeyCode::S => f(Nominal::C, 1, -1),
        KeyCode::X => f(Nominal::D, 0, -1),
        KeyCode::D => f(Nominal::D, 1, -1),
        KeyCode::C => f(Nominal::E, 0, -1),
        KeyCode::V => f(Nominal::F, 0, -1),
        KeyCode::G => f(Nominal::F, 1, -1),
        KeyCode::B => f(Nominal::G, 0, -1),
        KeyCode::H => f(Nominal::G, 1, -1),
        KeyCode::N => f(Nominal::A, 0, -1),
        KeyCode::J => f(Nominal::A, 1, -1),
        KeyCode::M => f(Nominal::B, 0, -1),
        KeyCode::Q => f(Nominal::C, 0, 0),
        KeyCode::Key2 => f(Nominal::C, 1, 0),
        KeyCode::W => f(Nominal::D, 0, 0),
        KeyCode::Key3 => f(Nominal::D, 1, 0),
        KeyCode::E => f(Nominal::E, 0, 0),
        KeyCode::R => f(Nominal::F, 0, 0),
        KeyCode::Key5 => f(Nominal::F, 1, 0),
        KeyCode::T => f(Nominal::G, 0, 0),
        KeyCode::Key6 => f(Nominal::G, 1, 0),
        KeyCode::Y => f(Nominal::A, 0, 0),
        KeyCode::Key7 => f(Nominal::A, 1, 0),
        KeyCode::U => f(Nominal::B, 0, 0),
        KeyCode::I => f(Nominal::C, 0, 1),
        KeyCode::Key9 => f(Nominal::C, 1, 1),
        KeyCode::O => f(Nominal::D, 0, 1),
        KeyCode::Key0 => f(Nominal::D, 1, 1),
        KeyCode::P => f(Nominal::E, 0, 1),
        _ => None
    }
}

pub fn note_from_midi(n: u8, t: &Tuning) -> Note {
    let f = |nominal, accidentals| {
        Note {
            arrows: if use_sharps(t) { 0 } else { accidentals },
            nominal,
            demisharps: if use_sharps(t) { accidentals * 2} else { 0 },
            equave: (n as i8) / 12 - 1,
        }
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