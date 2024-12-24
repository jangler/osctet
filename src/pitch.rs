//! Tuning and notation utilities.

use std::error::Error;
use std::{fmt, fs};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ui::text;

const REFERENCE_MIDI_PITCH: f32 = 69.0; // A4
const DEFAULT_ROOT: Note = Note {
    arrows: 0,
    nominal: Nominal::C,
    sharps: 0,
    equave: 4,
};

fn cents(ratio: f32) -> f32 {
    1200.0 * ratio.log2() / 2.0_f32.log2()
}

fn find_ratio(cents: f32) -> f32 {
    2.0_f32.powf(2.0_f32.log2() * cents / 1200.0)
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Nominal {
    A, B, C, D, E, F, G
}

impl Nominal {
    const VARIANTS: [Nominal; 7] =
        [Self::A, Self::B, Self::C, Self::D, Self::E, Self::F, Self::G];

    // (period, generator)
    fn vector(&self) -> (i32, i32) {
        match self {
            Nominal::A => (0, 0),
            Nominal::B => (-1, 2),
            Nominal::C => (1, -3),
            Nominal::D => (0, -1),
            Nominal::E => (-1, 1),
            Nominal::F => (2, -4),
            Nominal::G => (1, -2),
        }
    }

    pub fn char(&self) -> char {
        match self {
            Nominal::A => 'A',
            Nominal::B => 'B',
            Nominal::C => 'C',
            Nominal::D => 'D',
            Nominal::E => 'E',
            Nominal::F => 'F',
            Nominal::G => 'G',
        }
    }

    /// Returns the next nominal in the scale, along with octave offset.
    pub fn next(&self) -> (Nominal, i8) {
        match self {
            Nominal::A => (Nominal::B, 0),
            Nominal::B => (Nominal::C, 1),
            Nominal::C => (Nominal::D, 0),
            Nominal::D => (Nominal::E, 0),
            Nominal::E => (Nominal::F, 0),
            Nominal::F => (Nominal::G, 0),
            Nominal::G => (Nominal::A, 0),
        }
    }

    /// Returns the previous nominal in the scale, along with octave offset.
    pub fn prev(&self) -> (Nominal, i8) {
        match self {
            Nominal::A => (Nominal::G, 0),
            Nominal::B => (Nominal::A, 0),
            Nominal::C => (Nominal::B, -1),
            Nominal::D => (Nominal::C, 0),
            Nominal::E => (Nominal::D, 0),
            Nominal::F => (Nominal::E, 0),
            Nominal::G => (Nominal::F, 0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    pub root: Note,
    pub scale: Vec<f32>,
    pub arrow_steps: u8,
}

impl Tuning {
    pub fn divide(ratio: f32, steps: u16, arrow_steps: u8) -> Result<Tuning, &'static str> {
        if ratio <= 1.0 {
            return Err("ratio must be greater than 1");
        }
        if steps < 1 {
            return Err("step count cannot be zero");
        }
        let step = cents(ratio) / steps as f32;
        Ok(Tuning {
            root: DEFAULT_ROOT,
            scale: (1..=steps).map(|i| i as f32 * step).collect(),
            arrow_steps,
        })
    }

    /// Load a tuning from a Scala scale file.
    pub fn load(path: PathBuf, root: Note) -> Result<Tuning, Box<dyn Error>> {
        let s = fs::read_to_string(path)?;
        let mut lines = s.lines()
            .filter(|s| !s.starts_with("!")) // ignore comments
            .skip(1); // skip description

        let note_count: usize = if let Some(s) = lines.next() {
            s.parse()?
        } else {
            return Err("invalid scale file".into())
        };

        let scale: Result<Vec<_>, _> = lines.take(note_count).map(|s| {
            parse_interval(s).ok_or(format!("invalid interval: {}", s))
        }).collect();

        Ok(Tuning {
            root,
            scale: scale?,
            arrow_steps: 1,
        })
    }

    fn generator_steps(&self) -> i32 {
        (self.scale.len() as f32 * 0.585).round() as i32
    }

    fn sharp_steps(&self) -> i32 {
        self.generator_steps() * 7 - self.scale.len() as i32 * 4
    }

    fn nominal_steps(&self, nominal: Nominal) -> i32 {
        let (octaves, fifths) = nominal.vector();
        octaves * self.scale.len() as i32 + fifths * self.generator_steps()
    }

    // TODO: this and root_pitch might have off-by-one errors?
    pub fn midi_pitch(&self, note: &Note) -> f32 {
        let equave = self.scale.last().expect("scale cannot be empty") / 100.0;
        let root_steps = self.nominal_steps(self.root.nominal)
            + self.sharp_steps() * self.root.sharps as i32
            + self.arrow_steps as i32 * self.root.arrows as i32;
        let steps = -root_steps
            + self.nominal_steps(note.nominal)
            + self.sharp_steps() * note.sharps as i32
            + self.arrow_steps as i32 * note.arrows as i32;
        let len = self.scale.len() as i32;
        let scale_index = (steps - 1).rem_euclid(len) as usize;
        let step_equaves = (steps - 1).div_euclid(len);
        self.root_pitch() +
            equave * (note.equave as i32 - self.root.equave as i32 + step_equaves) as f32
            + self.scale[scale_index] / 100.0
    }

    // TODO: fix duplication of code with midi_pitch
    fn root_pitch(&self) -> f32 {
        let equave = self.scale.last().expect("scale cannot be empty") / 100.0;
        let steps = self.nominal_steps(self.root.nominal)
            + self.sharp_steps() * self.root.sharps as i32
            + self.arrow_steps as i32 * self.root.arrows as i32;
        let len = self.scale.len() as i32;
        let scale_index = (steps - 1).rem_euclid(len) as usize;
        let step_equaves = (steps - 1).div_euclid(len);
        REFERENCE_MIDI_PITCH +
            equave * (self.root.equave as i32 - 4 + step_equaves) as f32
            + self.scale[scale_index] / 100.0
    }

    pub fn equave(&self) -> f32 {
        find_ratio(*self.scale.last().unwrap())
    }

    pub fn size(&self) -> u16 {
        self.scale.len() as u16
    }

    /// Returns the scale index and equave of a note in this tuning.
    pub fn scale_index(&self, note: &Note) -> (usize, i8) {
        let root_steps = self.nominal_steps(self.root.nominal)
            + self.sharp_steps() * self.root.sharps as i32
            + self.arrow_steps as i32 * self.root.arrows as i32;
        let steps = -root_steps
            + self.nominal_steps(note.nominal)
            + self.sharp_steps() * note.sharps as i32
            + self.arrow_steps as i32 * note.arrows as i32;
        let n = self.size() as i32;

        (
            steps.rem_euclid(n) as usize,
            note.equave + steps.div_euclid(n) as i8
        )
    }

    /// Returns the shortest notation for a given scale index.
    pub fn notation(&self, index: usize, equave: i8) -> Vec<Note> {
        let mut old_notes;
        let mut new_notes = Nominal::VARIANTS
            .map(|nominal| Note::new(0, nominal, 0, equave))
            .to_vec();

        loop {
            let matches: Vec<_> = new_notes.iter()
                .filter(|note| self.scale_index(note).0 == index)
                .collect();

            if !matches.is_empty() {
                return matches.into_iter().map(|note| Note {
                    equave: note.equave - self.octave_offet(note),
                    ..*note
                }).collect()
            }

            old_notes = new_notes;
            new_notes = [(0, 1), (0, -1), (1, 0), (-1, 0)]
                .iter().flat_map(|(arrows, sharps)|
                    old_notes.iter().map(move |note| Note {
                        arrows: note.arrows + arrows,
                        sharps: note.sharps + sharps,
                        ..*note
                    })
                ).collect();
        }
    }

    /// Returns 1 if the note crosses the octave line B->C, -1 if it crosses
    /// the octave line B<-C, and 0 otherwise.
    fn octave_offet(&self, note: &Note) -> i8 {
        let (_, equave) = self.scale_index(note);
        equave - note.equave
    }
}

/// Parses a Scala file interval into cents.
fn parse_interval(s: &str) -> Option<f32> {
    let s = s.trim();

    if let Ok(n) = s.parse::<u32>() {
        Some(cents(n as f32))
    } else if let Ok(n) = s.parse::<f32>() {
        Some(n)
    } else if let Some((n, d)) = s.split_once("/") {
        let n = n.parse::<u32>().ok()?;
        let d = d.parse::<u32>().ok()?;
        Some(cents(n as f32 / d as f32))
    } else {
        None
    }
}

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Note {
    pub arrows: i8,
    pub nominal: Nominal,
    pub sharps: i8,
    pub equave: i8,
}

impl Note {
    pub fn new(arrows: i8, nominal: Nominal, sharps: i8, equave: i8) -> Note {
        Note { arrows, nominal, sharps, equave }
    }

    pub fn arrow_char(&self) -> char {
        char::from_u32(match self.arrows {
            ..=-3 => text::SUB_DOWN,
            -2 => text::DOUBLE_DOWN,
            -1 => text::DOWN,
            0 => b' '.into(),
            1 => text::UP,
            2 => text::DOUBLE_UP,
            3.. => text::SUB_UP,
        }).unwrap()
    }

    pub fn accidental_char(&self) -> char {
        char::from_u32(match self.sharps {
            ..=-3 => text::SUB_FLAT,
            -2 => text::DOUBLE_FLAT,
            -1 => text::FLAT,
            0 => b'-'.into(),
            1 => text::SHARP,
            2 => text::DOUBLE_SHARP,
            3.. => text::SUB_SHARP,
        }).unwrap()
    }

    /// Returns the simplest notation for the next/previous note of the tuning.
    /// Prefers notes with the same nominal.
    pub fn step_shift(&self, steps: isize, tuning: &Tuning) -> Note {
        let mut index = tuning.scale_index(self).0 as isize + steps;
        let mut equave = self.equave;
        let n = tuning.size() as isize;

        while index >= n {
            index -= n;
            equave += 1;
        }
        while index < 0 {
            index += n;
            equave -= 1;
        }

        let notes = tuning.notation(index as usize, equave + tuning.octave_offet(self));

        if let Some(note) = notes.iter().filter(|n| n.nominal == self.nominal).next() {
            return *note
        }

        notes[0]
    }

    /// Returns the next note in the set of simplest equivalent notations.
    pub fn cycle_notation(&self, tuning: &Tuning) -> Note {
        let (index, equave) = tuning.scale_index(self);
        let options = tuning.notation(index, equave);

        if let Some(i) = options.iter().position(|x| x == self) {
            options[(i + 1) % options.len()]
        } else {
            *options.get(0).unwrap_or(self)
        }
    }
}

impl Default for Note {
    fn default() -> Self {
        Self {
            arrows: 0,
            nominal: Nominal::C,
            sharps: 0,
            equave: 4,
        }
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let arrow_char = match self.arrow_char() {
            ' ' => "",
            c => &c.to_string(),
        };
        let accidental_char = match self.accidental_char() {
            '-' => "",
            c => &c.to_string(),
        };
        write!(f, "{}{}{}{}", arrow_char, self.nominal.char(),
            accidental_char, self.equave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: Note = Note {
        arrows: 0,
        nominal: Nominal::A,
        equave: 4,
        sharps: 0,
    };

    #[test]
    fn test_cents() {
        assert_eq!(cents(2.0), 1200.0);
        assert_eq!(cents(1.0), 0.0);
    }

    #[test]
    fn test_tuning_divide() {
        assert_eq!(Tuning::divide(2.0, 5, 1).unwrap(), Tuning {
            root: DEFAULT_ROOT,
            scale: vec![240.0, 480.0, 720.0, 960.0, 1200.0],
            arrow_steps: 1,
        });
        Tuning::divide(1.0, 5, 1).unwrap_err();
        Tuning::divide(0.5, 5, 1).unwrap_err();
        Tuning::divide(2.0, 0, 1).unwrap_err();
    }

    #[test]
    fn test_tuning_steps() {
        let t = Tuning::divide(2.0, 7, 1).unwrap();
        assert_eq!(t.generator_steps(), 4);
        assert_eq!(t.sharp_steps(), 0);
        let t = Tuning::divide(2.0, 12, 1).unwrap();
        assert_eq!(t.generator_steps(), 7);
        assert_eq!(t.sharp_steps(), 1);
        let t = Tuning::divide(2.0, 17, 1).unwrap();
        assert_eq!(t.generator_steps(), 10);
        assert_eq!(t.sharp_steps(), 2);
        let t = Tuning::divide(3.0, 13, 1).unwrap();
        assert_eq!(t.generator_steps(), 8);
        assert_eq!(t.sharp_steps(), 4); // very sharp generator!
    }

    #[test]
    fn test_tuning_nominal_steps() {
        let t = Tuning::divide(2.0, 12, 1).unwrap();
        assert_eq!(t.nominal_steps(Nominal::A), 0);
        assert_eq!(t.nominal_steps(Nominal::B), 2);
        assert_eq!(t.nominal_steps(Nominal::C), -9);
        assert_eq!(t.nominal_steps(Nominal::D), -7);
        assert_eq!(t.nominal_steps(Nominal::E), -5);
        assert_eq!(t.nominal_steps(Nominal::F), -4);
        assert_eq!(t.nominal_steps(Nominal::G), -2);
    }

    #[test]
    fn test_tuning_midi_pitch() {
        let mut t = Tuning::divide(2.0, 12, 1).unwrap();
        assert_eq!(t.midi_pitch(&A4), 69.0);
        assert_eq!(t.midi_pitch(&Note { arrows: 1, ..A4 }), 70.0);
        assert_eq!(t.midi_pitch(&Note { nominal: Nominal::B, ..A4 }), 71.0);
        assert_eq!(t.midi_pitch(&Note { nominal: Nominal::C, ..A4 }), 60.0);
        assert_eq!(t.midi_pitch(&Note { equave: 5, ..A4 }), 81.0);
        assert_eq!(t.midi_pitch(&Note { sharps: 1, ..A4 }), 70.0);
        t.root = Note::new(0, Nominal::D, 0, 0);
        assert_eq!(t.midi_pitch(&A4), 69.0);
    }

    #[test]
    fn test_parse_interval() {
        assert_eq!(parse_interval("2"), Some(1200.0));
        assert_eq!(parse_interval("4/1"), Some(2400.0));
        assert_eq!(parse_interval("386.6"), Some(386.6));
        assert_eq!(parse_interval("4/"), None);
    }

    #[test]
    fn test_tuning_scale_index() {
        let t = Tuning::divide(2.0, 12, 1).unwrap();
        assert_eq!(t.scale_index(&A4), (9, 4));
        assert_eq!(t.scale_index(&Note::new(0, Nominal::C, -1, 4)), (11, 3));
        assert_eq!(t.scale_index(&Note::new(1, Nominal::B, 0, 4)), (0, 5));
    }

    #[test]
    fn test_notation() {
        let t = Tuning::divide(2.0, 12, 1).unwrap();
        assert_eq!(t.notation(0, 4), vec![Note::new(0, Nominal::C, 0, 4)]);
        assert_eq!(t.notation(1, 3), vec![
            Note::new(0, Nominal::C, 1, 3),
            Note::new(0, Nominal::D, -1, 3),
            Note::new(1, Nominal::C, 0, 3),
            Note::new(-1, Nominal::D, 0, 3),
        ]);
        let t = Tuning::divide(2.0, 17, 1).unwrap();
        assert_eq!(t.notation(15, 4), vec![
            Note::new(0, Nominal::A, 1, 4),
            Note::new(0, Nominal::C, -1, 5),
            Note::new(-1, Nominal::B, 0, 4),
        ]);
    }

    #[test]
    fn test_step_shift() {
        let t = Tuning::divide(2.0, 12, 1).unwrap();
        assert_eq!(A4.step_shift(1, &t), Note {
            sharps: 1,
            ..A4
        });
        assert_eq!(Note::new(0, Nominal::B, 0, 4).step_shift(-1, &t), Note {
            nominal: Nominal::B,
            sharps: -1,
            ..A4
        });
        assert_eq!(Note::new(0, Nominal::B, 0, 4).step_shift(1, &t), Note {
            nominal: Nominal::C,
            equave: 5,
            ..A4
        });

        let t = Tuning::divide(2.0, 41, 1).unwrap();
        assert_eq!(Note::new(0, Nominal::E, 0, 4).step_shift(-1, &t), Note {
            nominal: Nominal::E,
            arrows: -1,
            ..A4
        });
    }

    #[test]
    fn test_octave_offset() {
        let t = Tuning::divide(2.0, 12, 1).unwrap();
        assert_eq!(t.octave_offet(&A4), 0);
        assert_eq!(t.octave_offet(&Note::new(0, Nominal::B, 1, 4)), 1);
        assert_eq!(t.octave_offet(&Note::new(0, Nominal::C, -1, 4)), -1);
        assert_eq!(t.octave_offet(&Note::new(0, Nominal::A, 5, 4)), 1);
        assert_eq!(t.octave_offet(&Note::new(-1, Nominal::B, 0, 4)), 0);
    }
}