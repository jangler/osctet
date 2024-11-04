use eframe::egui::Key;

use crate::pitch::{Nominal, Note};

pub fn note_from_key(key: &Key) -> Option<Note> {
    match key {
        Key::Z => Some(Note::new(0, Nominal::C, 0, -1)),
        Key::S => Some(Note::new(0, Nominal::C, 2, -1)),
        Key::X => Some(Note::new(0, Nominal::D, 0, -1)),
        Key::D => Some(Note::new(0, Nominal::D, 2, -1)),
        Key::C => Some(Note::new(0, Nominal::E, 0, -1)),
        Key::V => Some(Note::new(0, Nominal::F, 0, -1)),
        Key::G => Some(Note::new(0, Nominal::F, 2, -1)),
        Key::B => Some(Note::new(0, Nominal::G, 0, -1)),
        Key::H => Some(Note::new(0, Nominal::G, 2, -1)),
        Key::N => Some(Note::new(0, Nominal::A, 0, -1)),
        Key::J => Some(Note::new(0, Nominal::A, 2, -1)),
        Key::M => Some(Note::new(0, Nominal::B, 0, -1)),
        Key::Q => Some(Note::new(0, Nominal::C, 0, 0)),
        Key::Num2 => Some(Note::new(0, Nominal::C, 2, 0)),
        Key::W => Some(Note::new(0, Nominal::D, 0, 0)),
        Key::Num3 => Some(Note::new(0, Nominal::D, 2, 0)),
        Key::E => Some(Note::new(0, Nominal::E, 0, 0)),
        Key::R => Some(Note::new(0, Nominal::F, 0, 0)),
        Key::Num5 => Some(Note::new(0, Nominal::F, 2, 0)),
        Key::T => Some(Note::new(0, Nominal::G, 0, 0)),
        Key::Num6 => Some(Note::new(0, Nominal::G, 2, 0)),
        Key::Y => Some(Note::new(0, Nominal::A, 0, 0)),
        Key::Num7 => Some(Note::new(0, Nominal::A, 2, 0)),
        Key::U => Some(Note::new(0, Nominal::B, 0, 0)),
        Key::I => Some(Note::new(0, Nominal::C, 0, 1)),
        Key::Num9 => Some(Note::new(0, Nominal::C, 2, 1)),
        Key::O => Some(Note::new(0, Nominal::D, 0, 1)),
        Key::Num0 => Some(Note::new(0, Nominal::D, 2, 1)),
        Key::P => Some(Note::new(0, Nominal::E, 0, 1)),
        _ => None
    }
}