use core::f64;
use std::collections::HashMap;

use fundsp::hacker::*;

#[derive(PartialEq, Eq, Hash)]
pub enum KeyOrigin {
    Keyboard,
    Midi,
    Pattern,
}

#[derive(PartialEq, Eq, Hash)]
pub struct Key {
    pub origin: KeyOrigin,
    pub channel: u8,
    pub key: u8,
}

pub struct Synth {
    pub oscs: [Oscillator; 1],
    voices: HashMap<Key, Voice>,
}

impl Synth {
    pub fn new() -> Self {
        Self {
            oscs: [Oscillator::new()],
            voices: HashMap::new(),
        }
    }

    pub fn note_on(&mut self, key: Key, pitch: f32, seq: &mut Sequencer) {
        self.voices.insert(key, Voice::new(pitch, &self.oscs, seq));
    }

    pub fn note_off(&mut self, key: Key, seq: &mut Sequencer) {
        if let Some(voice) = self.voices.remove(&key) {
            voice.off(seq);
        }
    }
}

pub struct Oscillator {
    pub gain: Shared,
    pub env: ADSR,
}

impl Oscillator {
    fn new() -> Self {
        let gain = shared(0.2);
        let env = ADSR::new();
        Self {
            gain,
            env,
        }
    }
}

pub struct ADSR {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl ADSR {
    fn new() -> Self {
        Self {
            attack: 0.5,
            decay: 0.5,
            sustain: 0.5,
            release: 0.5,
        }
    }
}

struct Voice {
    freq: Shared,
    gate: Shared,
    release_time: f32,
    event_id: EventId,
}

impl Voice {
    fn new(pitch: f32, oscs: &[Oscillator], seq: &mut Sequencer) -> Self {
        let freq = shared(midi_hz(pitch));
        let gate = shared(1.0);
        let f = |i: usize| {
            (var(&freq) >> saw()) * var(&oscs[i].gain) *
                (var(&gate) >> adsr_live(oscs[i].env.attack, oscs[i].env.decay, oscs[i].env.sustain, oscs[i].env.release))
        };
        let unit = f(0);
        Self {
            freq,
            gate,
            release_time: oscs[0].env.release,
            event_id: seq.push_relative(0.0, f64::INFINITY, Fade::Smooth, 0.0, 0.0, Box::new(unit)),
        }
    }

    fn off(&self, seq: &mut Sequencer) {
        self.gate.set(0.0);
        seq.edit_relative(self.event_id, self.release_time as f64, 0.0);
    }
}