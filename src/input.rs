use eframe::egui::Key;

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