//! Tuning and notation utilities.

use std::error::Error;
use std::{fmt, fs};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ui::text;

/// Fixed reference point regardless of tuning.
const REFERENCE_MIDI_PITCH: f32 = 69.0;

/// Default root note for unequal scales.
const DEFAULT_ROOT: Note = Note {
    arrows: 0,
    nominal: Nominal::C,
    sharps: 0,
    equave: 4,
};

/// Converts a freq ratio to cents.
fn cents(ratio: f32) -> f32 {
    1200.0 * ratio.log2() / 2.0_f32.log2()
}

/// Converts cents to a freq ratio.
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

    /// Returns the (period, generator) mapping of this nominal, relative to
    /// the reference MIDI pitch.
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

    /// Returns the character used for the nominal.
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
    /// Cents values of scale notes. The last value is also the scale period.
    pub scale: Vec<f32>,
    pub arrow_steps: u8,
}

impl Tuning {
    /// Generate a tuning by dividing a ratio into equal steps.
    pub fn divide(ratio: f32, steps: u16, arrow_steps: u8) -> Result<Tuning, &'static str> {
        if ratio <= 1.0 {
            return Err("ratio must be greater than 1");
        } else if steps < 1 {
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
            parse_interval(s).ok_or(format!("invalid interval: {s}"))
        }).collect();

        Ok(Tuning {
            root,
            scale: scale?,
            arrow_steps: 1,
        })
    }

    /// Translates notation to a concrete pitch.
    pub fn midi_pitch(&self, note: &Note) -> f32 {
        let root_steps = self.raw_steps(&self.root);
        let steps = self.raw_steps(note) - root_steps;
        let root_pitch = self.pitch_from_steps(
            root_steps, self.root.equave, REFERENCE_MIDI_PITCH, 4);
        self.pitch_from_steps(steps, note.equave, root_pitch, self.root.equave)
    }

    /// Returns a raw step count for a note.
    fn raw_steps(&self, note: &Note) -> i32 {
        let generator_steps = (self.scale.len() as f32 * 0.585).round() as i32;
        let sharp_steps = generator_steps * 7 - self.scale.len() as i32 * 4;
        let nominal_steps = {
            let (octaves, fifths) = note.nominal.vector();
            octaves * self.scale.len() as i32 + fifths * generator_steps
        };

        nominal_steps
            + sharp_steps * note.sharps as i32
            + self.arrow_steps as i32 * note.arrows as i32
    }

    /// Converts raw step counts into a MIDI pitch.
    fn pitch_from_steps(&self, steps: i32, equave: i8, ref_pitch: f32, ref_equave: i8
    ) -> f32 {
        let equave_interval = self.scale.last().expect("scale cannot be empty") / 100.0;
        let len = self.scale.len() as i32;
        let scale_index = (steps - 1).rem_euclid(len) as usize;
        let step_equaves = (steps - 1).div_euclid(len);
        ref_pitch +
            equave_interval * (equave as i32 - ref_equave as i32 + step_equaves) as f32
            + self.scale[scale_index] / 100.0
    }

    /// Returns the ratio of this scale's period.
    pub fn equave(&self) -> f32 {
        find_ratio(*self.scale.last().expect("scale cannot be empty"))
    }

    /// Returns the number of steps in the period.
    pub fn size(&self) -> u16 {
        self.scale.len() as u16
    }

    /// Returns the scale index and equave of a note in this tuning.
    pub fn scale_index(&self, note: &Note) -> (usize, i8) {
        let steps = self.raw_steps(note) - self.raw_steps(&self.root);
        let n = self.size() as i32;

        (
            steps.rem_euclid(n) as usize,
            note.equave + steps.div_euclid(n) as i8
        )
    }

    /// Returns the shortest notation for a given scale index. May return
    /// an empty vector.
    pub fn notation(&self, index: usize, equave: i8) -> Vec<Note> {
        let mut old_notes;
        let mut new_notes = Nominal::VARIANTS
            .map(|nominal| Note::new(0, nominal, 0, equave))
            .to_vec();

        // notation with more than 9 accidentals isn't exactly useful
        for _ in 0..9 {
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
                    old_notes.iter().flat_map(move |note|
                        // avoid backtracking and duplicates
                        if (note.sharps > 0 && *sharps < 0)
                            || (note.sharps < 0 && *sharps > 0)
                            || (note.arrows > 0 && *arrows < 0)
                            || (note.arrows < 0 && *arrows > 0)
                            || (note.arrows != 0 && *sharps != 0) {
                            None
                        } else {
                            Some(Note {
                                arrows: note.arrows + arrows,
                                sharps: note.sharps + sharps,
                                ..*note
                            })
                        })
                ).collect();
        }

        Vec::new()
    }

    /// Returns 1 if the note crosses the octave line B->C, -1 if it crosses
    /// the octave line B<-C, and 0 otherwise.
    fn octave_offet(&self, note: &Note) -> i8 {
        let (_, equave) = self.scale_index(note);
        equave - note.equave
    }

    /// Returns a table of (notation, cents) pairs, starting on `root`.
    pub fn interval_table(&self, root: &Note) -> Vec<(Vec<Note>, f32)> {
        let base = self.midi_pitch(root);
        let mut v = Vec::with_capacity(self.scale.len() + 1);

        for i in 0..=self.scale.len() {
            let notes = root.step_shift_all(i as isize, self);
            let cents = if let Some(note) = notes.first() {
                (self.midi_pitch(note) - base) * 100.0
            } else {
                // TODO: not necessarily true for unequal scales
                self.scale[(i as i32 - 1).rem_euclid(self.scale.len() as i32) as usize]
            };
            v.push((notes, cents))
        }

        v
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

/// Abstract notational representation of pitch.
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

    /// Returns the character code used for this note's arrows.
    pub fn arrow_char(&self) -> char {
        char::from_u32(match self.arrows {
            ..=-3 => text::SUB_DOWN,
            -2 => text::DOUBLE_DOWN,
            -1 => text::DOWN,
            0 => b' '.into(),
            1 => text::UP,
            2 => text::DOUBLE_UP,
            3.. => text::SUB_UP,
        }).expect("code points constants should be valid")
    }

    /// Returns the character code used for this note's sharps/flats.
    pub fn accidental_char(&self) -> char {
        char::from_u32(match self.sharps {
            ..=-3 => text::SUB_FLAT,
            -2 => text::DOUBLE_FLAT,
            -1 => text::FLAT,
            0 => b'-'.into(),
            1 => text::SHARP,
            2 => text::DOUBLE_SHARP,
            3.. => text::SUB_SHARP,
        }).expect("code points constants should be valid")
    }

    /// Returns the simplest notation for the next/previous note of the tuning.
    /// Prefers notes with the same nominal.
    pub fn step_shift(&self, steps: isize, tuning: &Tuning) -> Note {
        let notes = self.step_shift_all(steps, tuning);

        if let Some(note) = notes.iter().find(|n| n.nominal == self.nominal) {
            return *note
        }

        *notes.first().unwrap_or(self)
    }

    /// Returns all notation for the next/previous note of the tuning.
    /// May return an empty vector.
    fn step_shift_all(&self, steps: isize, tuning: &Tuning) -> Vec<Note> {
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

        tuning.notation(index as usize, equave + tuning.octave_offet(self))
    }

    /// Returns the next note in the set of simplest equivalent notations.
    pub fn cycle_notation(&self, tuning: &Tuning) -> Note {
        let (index, equave) = tuning.scale_index(self);
        let options = tuning.notation(index, equave);

        if let Some(i) = options.iter().position(|x| x == self) {
            options[(i + 1) % options.len()]
        } else {
            *options.first().unwrap_or(self)
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

        let t = Tuning::divide(2.0, 15, 0).unwrap();
        assert_eq!(t.notation(1, 4), Vec::new()); // no notation for desired note
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

        let t = Tuning::divide(2.0, 15, 0).unwrap();
        assert_eq!(A4.step_shift(1, &t), A4); // no notation for desired note
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