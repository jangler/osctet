use fundsp::hacker::*;

#[derive(PartialEq)]
pub enum KeyOrigin {
    Keyboard,
    Midi,
    Pattern,
}

#[derive(PartialEq)]
pub struct Key {
    pub origin: KeyOrigin,
    pub channel: u8,
    pub key: u8,
}

pub struct Synth {
    pub oscs: [Oscillator; 1],
    gate: Shared,
    held_key: Option<Key>,
}

impl Synth {
    pub fn new() -> Self {
        let gate = shared(0.0);
        let oscs = [Oscillator::new(&gate)];
        Self {
            gate,
            oscs,
            held_key: None,
        }
    }

    pub fn note_on(&mut self, key: Key, pitch: f32) {
        for osc in self.oscs.iter() {
            osc.freq.set(midi_hz(pitch));
        }
        self.gate.set(1.0);
        self.held_key = Some(key);
    }

    pub fn note_off(&mut self, key: Key) {
        if let Some(held_key) = &mut self.held_key {
            if key == *held_key {
                self.gate.set(0.0);
                self.held_key = None;
            }
        }
    }
}

pub struct Oscillator {
    pub freq: Shared,
    pub gain: Shared,
    pub env: ADSR,
    pub unit: Box<dyn AudioUnit>,
}

impl Oscillator {
    fn new(gate: &Shared) -> Self {
        let freq = shared(440.0);
        let gain = shared(0.2);
        let env = ADSR::new();
        let unit = (var(&freq) >> saw()) * var(&gain) *
            (var(gate) >> adsr_live(env.attack, env.decay, env.sustain, env.release));
        Self {
            freq,
            gain,
            env,
            unit: Box::new(unit),
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
            attack: 0.1,
            decay: 0.1,
            sustain: 0.5,
            release: 0.1,
        }
    }
}