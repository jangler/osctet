use std::fmt;
use std::cmp::max;

use anyhow::{bail, Result};

const REFERENCE_MIDI_PITCH: f32 = 69.0; // A4

fn cents(ratio: f32) -> f32 {
    1200.0 * ratio.log2() / 2.0_f32.log2()
}

#[derive(Clone, Copy)]
pub enum Nominal {
    A, B, C, D, E, F, G
}

impl Nominal {
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

    fn char(&self) -> char {
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
}

#[derive(Debug, PartialEq)]
pub struct Tuning {
    scale: Vec<f32>,
    arrow_steps: u8,
}

impl Tuning {
    pub fn divide(ratio: f32, steps: u16, arrow_steps: u8) -> Result<Tuning> {
        if ratio <= 1.0 {
            bail!("ratio must be greater than 1");
        }
        if steps < 1 {
            bail!("step count cannot be zero");
        }
        let step = cents(ratio) / steps as f32;
        Ok(Tuning {
            scale: (1..=steps).map(|i| i as f32 * step).collect(),
            arrow_steps,
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

    pub fn midi_pitch(&self, note: &Note) -> f32 {
        let equave = self.scale.last().unwrap() / 100.0;
        let steps = self.nominal_steps(note.nominal) +
            self.sharp_steps() * note.demisharps as i32 / 2 +
            self.arrow_steps as i32 * note.arrows as i32;
        let len = self.scale.len() as i32;
        let scale_index = (steps - 1).rem_euclid(len) as usize;
        let step_equaves = (steps - 1).div_euclid(len);
        REFERENCE_MIDI_PITCH +
            equave * (note.equave as i32 - 4 + step_equaves) as f32
            + self.scale[scale_index] / 100.0
    }
}

pub struct Note {
    pub arrows: i8,
    pub nominal: Nominal,
    pub demisharps: i8,
    pub equave: i8,
}

impl Note {
    pub fn new(arrows: i8, nominal: Nominal, demisharps: i8, equave: i8) -> Note {
        Note { arrows, nominal, demisharps, equave }
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let arrow_str = "^".repeat(max(0, self.arrows) as usize) +
            &"v".repeat(max(0, -self.arrows) as usize);
        let sharp_str = match self.demisharps {
            -4 => "bb",
            -3 => "db",
            -2 => "b",
            -1 => "d",
            0 => "",
            1 => "t",
            2 => "#",
            3 => "t#",
            4 => "x",
            _ => "?",
        };
        write!(f, "{}{}{}{}", arrow_str, self.nominal.char(), sharp_str, self.equave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    const A4: Note = Note {
        arrows: 0,
        nominal: Nominal::A,
        equave: 4,
        demisharps: 0,
    };

    #[test]
    fn test_cents() {
        assert_eq!(cents(2.0), 1200.0);
        assert_eq!(cents(1.0), 0.0);
    }

    #[test]
    fn test_tuning_divide() {
        assert_eq!(Tuning::divide(2.0, 5, 1).unwrap(), Tuning {
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
        let t = Tuning::divide(2.0, 12, 1).unwrap();
        assert_eq!(t.midi_pitch(&A4), 69.0);
        assert_eq!(t.midi_pitch(&Note { arrows: 1, ..A4 }), 70.0);
        assert_eq!(t.midi_pitch(&Note { nominal: Nominal::B, ..A4 }), 71.0);
        assert_eq!(t.midi_pitch(&Note { nominal: Nominal::C, ..A4 }), 60.0);
        assert_eq!(t.midi_pitch(&Note { equave: 5, ..A4 }), 81.0);
        assert_eq!(t.midi_pitch(&Note { demisharps: 1, ..A4 }), 69.0);
        assert_eq!(t.midi_pitch(&Note { demisharps: -1, ..A4 }), 69.0);
        assert_eq!(t.midi_pitch(&Note { demisharps: 2, ..A4 }), 70.0);
    }

    #[test]
    fn test_note_display() {
        assert_eq!(format!("{}", A4), "A4");
        assert_eq!(format!("{}", Note { demisharps: 2, ..A4 }), "A#4");
        assert_eq!(format!("{}", Note { arrows: -1, demisharps: 2, ..A4 }), "vA#4");
    }
}