use eframe::{egui::Key, glow::PRIMITIVE_RESTART_FOR_PATCHES_SUPPORTED};

use crate::pitch::{Nominal, Note, Tuning};

// sharps aren't much use for keyboard mapping if they're equal to unison
// or the whole tone
fn use_sharps(t: &Tuning) -> bool {
    let d4 = Note::new(0, Nominal::D, 0, 4);
    let ds4 = Note { demisharps: 2, ..d4 };
    t.midi_pitch(&d4) != t.midi_pitch(&ds4) &&
        t.midi_pitch(&ds4) != t.midi_pitch(&Note { nominal: Nominal::E, ..d4 })
}

pub fn note_from_key(k: &Key, t: &Tuning, equave: i8) -> Option<Note> {
    let f = |nominal, accidentals, offset| {
        Some(Note {
            arrows: if use_sharps(t) { 0 } else { accidentals },
            nominal,
            demisharps: if use_sharps(t) { accidentals * 2} else { 0 },
            equave: equave + offset,
        })
    };
    match k {
        Key::Z => f(Nominal::C, 0, -1),
        Key::S => f(Nominal::C, 1, -1),
        Key::X => f(Nominal::D, 0, -1),
        Key::D => f(Nominal::D, 1, -1),
        Key::C => f(Nominal::E, 0, -1),
        Key::V => f(Nominal::F, 0, -1),
        Key::G => f(Nominal::F, 1, -1),
        Key::B => f(Nominal::G, 0, -1),
        Key::H => f(Nominal::G, 1, -1),
        Key::N => f(Nominal::A, 0, -1),
        Key::J => f(Nominal::A, 1, -1),
        Key::M => f(Nominal::B, 0, -1),
        Key::Q => f(Nominal::C, 0, 0),
        Key::Num2 => f(Nominal::C, 1, 0),
        Key::W => f(Nominal::D, 0, 0),
        Key::Num3 => f(Nominal::D, 1, 0),
        Key::E => f(Nominal::E, 0, 0),
        Key::R => f(Nominal::F, 0, 0),
        Key::Num5 => f(Nominal::F, 1, 0),
        Key::T => f(Nominal::G, 0, 0),
        Key::Num6 => f(Nominal::G, 1, 0),
        Key::Y => f(Nominal::A, 0, 0),
        Key::Num7 => f(Nominal::A, 1, 0),
        Key::U => f(Nominal::B, 0, 0),
        Key::I => f(Nominal::C, 0, 1),
        Key::Num9 => f(Nominal::C, 1, 1),
        Key::O => f(Nominal::D, 0, 1),
        Key::Num0 => f(Nominal::D, 1, 1),
        Key::P => f(Nominal::E, 0, 1),
        _ => None
    }
}

pub fn note_from_midi(n: i8, t: &Tuning) -> Note {
    let f = |nominal, accidentals| {
        Note {
            arrows: if use_sharps(t) { 0 } else { accidentals },
            nominal,
            demisharps: if use_sharps(t) { accidentals * 2} else { 0 },
            equave: n / 12 - 1,
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
        velocity: u8,
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
            0x80 => Some(Self::NoteOff { channel, key: data[1], velocity: *data.get(2)? }),
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