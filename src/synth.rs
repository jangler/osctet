use core::f64;
use std::collections::HashMap;

use fundsp::hacker::*;

const KEY_TRACKING_REF_FREQ: f32 = 261.6;
const PITCH_BEND_RANGE: f32 = 2.0;

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum KeyOrigin {
    Keyboard,
    Midi,
    Pattern,
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Key {
    pub origin: KeyOrigin,
    pub channel: u8,
    pub key: u8,
}

#[derive(PartialEq, Clone, Copy)]
pub enum PlayMode {
    Poly,
    Mono,
    SingleTrigger,
}

impl PlayMode {
    pub const VARIANTS: [PlayMode; 3] = [Self::Poly, Self::Mono, Self::SingleTrigger];

    pub fn name(&self) -> &str {
        match self {
            Self::Poly => "Poly",
            Self::Mono => "Mono",
            Self::SingleTrigger => "Single trigger",
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum Waveform {
    Sawtooth,
    Pulse,
    Triangle,
    Sine,
}

impl Waveform {
    pub const VARIANTS: [Waveform; 4] = [Self::Sawtooth, Self::Pulse, Self::Triangle, Self::Sine];

    pub fn name(&self) -> &str {
        match self {
            Self::Sawtooth => "Sawtooth",
            Self::Pulse => "Pulse",
            Self::Triangle => "Triangle",
            Self::Sine => "Sine",
        }
    }

    fn make_net(&self, freq: &Shared, glide_time: f32, duty: &Shared) -> Net {
        // have to compensate for different volumes. the sine is so loud!
        match self {
            Self::Sawtooth => Net::wrap(Box::new(var(freq) >> follow(glide_time) >> saw())),
            Self::Pulse => Net::wrap(Box::new((var(freq) >> follow(glide_time) | var(duty)) >> pulse())),
            Self::Triangle => Net::wrap(Box::new(var(freq) >> follow(glide_time) >> triangle() * 2.0)),
            Self::Sine => Net::wrap(Box::new(var(freq) >> follow(glide_time) >> sine() * 0.5)),
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum FilterType {
    Lowpass,
    Highpass,
    Bandpass,
}

impl FilterType {
    pub const VARIANTS: [FilterType; 3] = [Self::Lowpass, Self::Highpass, Self::Bandpass];
    
    pub fn name(&self) -> &str {
        match self {
            Self::Lowpass => "Lowpass",
            Self::Highpass => "Highpass",
            Self::Bandpass => "Bandpass",
        }
    }
}

pub struct Synth {
    pub oscs: [Oscillator; 4],
    pub filter: Filter,
    pub play_mode: PlayMode,
    pub glide_time: f32,
    voices: HashMap<Key, Voice>,
    bend_memory: [f32; 16],
}

impl Synth {
    pub fn new() -> Self {
        Self {
            oscs: [Oscillator::new(0.5), Oscillator::new(0.0), Oscillator::new(0.0), Oscillator::new(0.0)],
            filter: Filter::new(),
            play_mode: PlayMode::Poly,
            glide_time: 0.05,
            voices: HashMap::new(),
            bend_memory: [0.0; 16],
        }
    }

    pub fn note_on(&mut self, key: Key, pitch: f32, pressure: f32, seq: &mut Sequencer) {
        let bend = if key.origin == KeyOrigin::Midi {
            self.bend_memory[key.channel as usize]
        } else {
            0.0
        };
        let insert_voice = match self.play_mode {
            PlayMode::Poly => true,
            PlayMode::Mono => {
                for voice in self.voices.values_mut() {
                    voice.off(seq);
                }
                true
            },
            PlayMode::SingleTrigger => {
                if self.voices.is_empty() {
                    true
                } else {
                    let voice = self.voices.drain().map(|(_, v)| v).next().unwrap();
                    voice.freq.set(midi_hz(pitch));
                    self.voices.insert(key.clone(), voice);
                    false
                }
            },
        };
        if insert_voice {
           self.voices.insert(key, Voice::new(pitch, bend, pressure, self.glide_time, &self.oscs, &self.filter, seq));
        }
    }

    pub fn note_off(&mut self, key: Key, seq: &mut Sequencer) {
        if let Some(voice) = self.voices.remove(&key) {
            voice.off(seq);
        }
    }

    pub fn pitch_bend(&mut self, channel: u8, bend: f32) {
        self.bend_memory[channel as usize] = bend;
        for (key, voice) in self.voices.iter_mut() {
            if key.origin == KeyOrigin::Midi && key.channel == channel {
                voice.freq.set(midi_hz(voice.base_pitch + bend * PITCH_BEND_RANGE));
            }
        }
    }

    pub fn poly_pressure(&mut self, key: Key, pressure: f32) {
        self.voices.get(&key).inspect(|v| v.pressure.set(pressure));
    }

    pub fn channel_pressure(&mut self, channel: u8, pressure: f32) {
        for (key, voice) in self.voices.iter_mut() {
            if key.origin == KeyOrigin::Midi && key.channel == channel {
                voice.pressure.set(pressure);
            }
        }
    }
}

pub struct Oscillator {
    pub level: Shared,
    pub duty: Shared,
    pub env: ADSR,
    pub waveform: Waveform,
}

impl Oscillator {
    fn new(level: f32) -> Self {
        Self {
            level: shared(level),
            duty: shared(0.5),
            env: ADSR::new(),
            waveform: Waveform::Sawtooth,
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum KeyTracking {
    None,
    Partial,
    Full,
}

impl KeyTracking {
    pub const VARIANTS: [KeyTracking; 3] = [Self::None, Self::Partial, Self::Full];

    pub fn name(&self) -> &str {
        match self {
            Self::None => "None",
            Self::Partial => "Partial",
            Self::Full => "Full",
        }
    }
}

pub struct Filter {
    pub filter_type: FilterType,
    pub cutoff: Shared,
    pub resonance: Shared,
    pub key_tracking: KeyTracking,
    pub env: ADSR,
    pub env_level: Shared,
}

impl Filter {
    fn new() -> Self {
        Self {
            cutoff: shared(20_000.0),
            resonance: shared(0.1),
            key_tracking: KeyTracking::None,
            filter_type: FilterType::Lowpass,
            env: ADSR::new(),
            env_level: shared(0.0),
        }
    }

    fn make_net(&self, note_freq: &Shared, gate: &Shared) -> Net {
        // FIXME: partial key tracking uses linear math, when it should be logarithmic
        let kt = match self.key_tracking {
            KeyTracking::None => Net::wrap(Box::new(constant(1.0))),
            KeyTracking::Partial => Net::wrap(Box::new((var(note_freq) + KEY_TRACKING_REF_FREQ) * 0.5 * (1.0/KEY_TRACKING_REF_FREQ))),
            KeyTracking::Full => Net::wrap(Box::new(var(note_freq) * (1.0/KEY_TRACKING_REF_FREQ))),
        };
        let f = match self.filter_type {
            FilterType::Lowpass => Net::wrap(Box::new(flowpass(Tanh(1.0)))),
            FilterType::Highpass => Net::wrap(Box::new(fhighpass(Tanh(1.0)))),
            FilterType::Bandpass => Net::wrap(Box::new(fresonator(Tanh(1.0)))),
        };
        (pass() |
            var(&self.cutoff) * kt *
            (var(gate) >> adsr_live(self.env.attack, self.env.decay, self.env.sustain, self.env.release) * var(&self.env_level) + 1.0) |
            var(&self.resonance)
        ) >> f
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
            attack: 0.01,
            decay: 1.0,
            sustain: 1.0,
            release: 0.01,
        }
    }
}

struct Voice {
    base_pitch: f32,
    freq: Shared,
    pressure: Shared,
    gate: Shared,
    release_time: f32,
    event_id: EventId,
}

impl Voice {
    fn new(pitch: f32, bend: f32, pressure: f32, glide_time: f32, oscs: &[Oscillator], filter: &Filter, seq: &mut Sequencer) -> Self {
        let freq = shared(midi_hz(pitch + bend * PITCH_BEND_RANGE));
        let gate = shared(1.0);
        let pressure = shared(pressure);
        let f = |i: usize| {
            (oscs[i].waveform.make_net(&freq, glide_time, &oscs[i].duty)) * (var(&oscs[i].level) >> follow(0.01)) *
                (var(&gate) >> adsr_live(oscs[i].env.attack, oscs[i].env.decay, oscs[i].env.sustain, oscs[i].env.release) *
                var(&pressure)) >>
                filter.make_net(&freq, &gate)
        };
        let unit = f(0) + f(1) + f(2) + f(3);
        Self {
            base_pitch: pitch,
            freq,
            pressure,
            gate,
            release_time: oscs[0].env.release,
            event_id: seq.push_relative(0.0, f64::INFINITY, Fade::Smooth, 0.0, 0.0, Box::new(unit)),
        }
    }

    fn off(&self, seq: &mut Sequencer) {
        self.gate.set(0.0);
        seq.edit_relative(self.event_id, self.release_time as f64, 0.01);
    }
}