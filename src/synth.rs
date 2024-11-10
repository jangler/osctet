use core::f64;
use std::{collections::HashMap, fmt::Display};

use fundsp::hacker::*;

const KEY_TRACKING_REF_FREQ: f32 = 261.6;
const SEMITONE_RATIO: f32 = 1.059463;

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

    fn make_net(&self, settings: &Settings, vars: &VoiceVars, osc: &Oscillator, index: usize) -> Net {
        let base = var(&vars.freq) * (var(&osc.fine_pitch) >> shape_fn(|x| pow(SEMITONE_RATIO, x))) >> follow(settings.glide_time);

        // have to compensate for different volumes. the sine is so loud!
        match self {
            Self::Sawtooth => Net::wrap(Box::new(base >> saw())),
            Self::Pulse => {
                let duty_mod = settings.dsp_component(vars, ModTarget::Duty(index));
                Net::wrap(Box::new((base | var(&osc.duty) + duty_mod >> follow(0.01)) >> pulse()))
            },
            Self::Triangle => Net::wrap(Box::new(base >> triangle() * 2.0)),
            Self::Sine => Net::wrap(Box::new(base >> sine() * 0.5)),
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
    pub settings: Settings,
    voices: HashMap<Key, Voice>,
    bend_memory: [f32; 16],
    mod_memory: f32,
}

impl Synth {
    pub fn new() -> Self {
        Self {
            settings: Settings {
                oscs: [Oscillator::new(0.5), Oscillator::new(0.0), Oscillator::new(0.0), Oscillator::new(0.0)],
                envs: vec![ADSR::new()],
                filter: Filter::new(),
                play_mode: PlayMode::Poly,
                glide_time: 0.05,
                mod_matrix: vec![Modulation {
                    source: ModSource::Envelope(0),
                    target: ModTarget::Gain,
                    depth: shared(1.0),
                }],
            },
            voices: HashMap::new(),
            bend_memory: [0.0; 16],
            mod_memory: 0.0,
        }
    }

    pub fn note_on(&mut self, key: Key, pitch: f32, pressure: f32, seq: &mut Sequencer) {
        let bend = if key.origin == KeyOrigin::Midi {
            self.bend_memory[key.channel as usize]
        } else {
            0.0
        };
        let insert_voice = match self.settings.play_mode {
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
                    let voice = self.voices.drain().map(|(_, v)| v).next()
                        .expect("voices confirmed non-empty");
                    voice.vars.freq.set(midi_hz(pitch));
                    self.voices.insert(key.clone(), voice);
                    false
                }
            },
        };
        if insert_voice {
           self.voices.insert(key, Voice::new(pitch, bend, pressure, self.mod_memory, &self.settings, seq));
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
                voice.vars.freq.set(midi_hz(voice.base_pitch + bend));
            }
        }
    }

    pub fn poly_pressure(&mut self, key: Key, pressure: f32) {
        self.voices.get(&key).inspect(|v| v.vars.pressure.set(pressure));
    }

    pub fn channel_pressure(&mut self, channel: u8, pressure: f32) {
        for (key, voice) in self.voices.iter_mut() {
            if key.origin == KeyOrigin::Midi && key.channel == channel {
                voice.vars.pressure.set(pressure);
            }
        }
    }

    pub fn modulate(&mut self, depth: f32) {
        self.mod_memory = depth;
        for (key, voice) in self.voices.iter_mut() {
            if key.origin == KeyOrigin::Midi {
                voice.vars.modulation.set(depth);
            }
        }
    }
}

pub struct Settings {
    pub oscs: [Oscillator; 4],
    pub envs: Vec<ADSR>,
    pub filter: Filter,
    pub play_mode: PlayMode,
    pub glide_time: f32,
    pub mod_matrix: Vec<Modulation>,
}

impl Settings {
    fn dsp_component(&self, vars: &VoiceVars, target: ModTarget) -> Net {
        let mut net = Net::wrap(Box::new(constant(if target.is_additive() { 0.0 } else { 1.0 })));
        for m in &self.mod_matrix {
            if m.target == target {
                if target.is_additive() {
                    net = net + m.dsp_component(&self, &vars);
                } else {
                    net = net * m.dsp_component(&self, &vars);
                }
            }
        }
        net
    }
}

pub struct Oscillator {
    pub level: Shared,
    pub duty: Shared,
    pub fine_pitch: Shared,
    pub waveform: Waveform,
}

impl Oscillator {
    fn new(level: f32) -> Self {
        Self {
            level: shared(level),
            duty: shared(0.5),
            fine_pitch: shared(0.0),
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
}

impl Filter {
    fn new() -> Self {
        Self {
            cutoff: shared(20_000.0),
            resonance: shared(0.1),
            key_tracking: KeyTracking::None,
            filter_type: FilterType::Lowpass,
        }
    }

    fn make_net(&self, settings: &Settings, vars: &VoiceVars) -> Net {
        // FIXME: partial key tracking uses linear math, when it should be logarithmic
        let kt = match self.key_tracking {
            KeyTracking::None => Net::wrap(Box::new(constant(1.0))),
            KeyTracking::Partial => Net::wrap(Box::new((var(&vars.freq) + KEY_TRACKING_REF_FREQ) * 0.5 * (1.0/KEY_TRACKING_REF_FREQ))),
            KeyTracking::Full => Net::wrap(Box::new(var(&vars.freq) * (1.0/KEY_TRACKING_REF_FREQ))),
        };
        let cutoff_mod = settings.dsp_component(vars, ModTarget::FilterCutoff) >> shape_fn(|x| pow(4.0, x));
        let f = match self.filter_type {
            FilterType::Lowpass => Net::wrap(Box::new(flowpass(Tanh(1.0)))),
            FilterType::Highpass => Net::wrap(Box::new(fhighpass(Tanh(1.0)))),
            FilterType::Bandpass => Net::wrap(Box::new(fresonator(Tanh(1.0)))),
        };
        (pass() | var(&self.cutoff) * kt * cutoff_mod | var(&self.resonance)) >> f
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

    fn make_node(&self, gate: &Shared) -> An<Pipe<Var, EnvelopeIn<f32, impl FnMut(f32, &numeric_array::NumericArray<f32, typenum::UInt<typenum::UTerm, typenum::B1>>) -> f32 + Clone, typenum::UInt<typenum::UTerm, typenum::B1>, f32>>> {
        var(gate) >> adsr_live(self.attack, self.decay, self.sustain, self.release)
    }
}

pub struct Modulation {
    pub source: ModSource,
    pub target: ModTarget,
    pub depth: Shared,
}

impl Modulation {
    pub fn default() -> Self {
        Self {
            source: ModSource::Modulation,
            target: ModTarget::FilterCutoff,
            depth: shared(0.0),
        }
    }

    fn dsp_component(&self, settings: &Settings, vars: &VoiceVars) -> Net {
        let net = match self.source {
            ModSource::Envelope(i) => match settings.envs.get(i) {
                Some(env) => Net::wrap(Box::new(env.make_node(&vars.gate))),
                None => Net::wrap(Box::new(zero())),
            },
            ModSource::Pressure => Net::wrap(Box::new(var(&vars.pressure))),
            ModSource::Modulation => Net::wrap(Box::new(var(&vars.modulation))),
        };
        if self.target.is_additive() {
            net * var(&self.depth)
        } else {
            net
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum ModSource {
    Pressure,
    Modulation,
    Envelope(usize),
}

impl ModSource {
    pub const VARIANTS: [ModSource; 3] = [Self::Pressure, Self::Modulation, Self::Envelope(0)];
}

impl Display for ModSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pressure => "Pressure",
            Self::Modulation => "Mod wheel",
            Self::Envelope(i) => &format!("Envelope {}", i + 1),
        };
        f.write_str(s)
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum ModTarget {
    Gain,
    Duty(usize),
    FilterCutoff,
}

impl ModTarget {
    pub const VARIANTS: [ModTarget; 3] = [Self::Gain, Self::Duty(0), Self::FilterCutoff];

    pub fn is_additive(&self) -> bool {
        *self != ModTarget::Gain
    }
}

impl Display for ModTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Gain => "Gain",
            Self::Duty(n) => &format!("Osc {} duty", n + 1),
            Self::FilterCutoff => "Filter cutoff",
        };
        f.write_str(s)
    }
}

struct Voice {
    vars: VoiceVars,
    base_pitch: f32,
    release_time: f32,
    event_id: EventId,
}

impl Voice {
    fn new(pitch: f32, bend: f32, pressure: f32, modulation: f32, settings: &Settings, seq: &mut Sequencer) -> Self {
        let gate = shared(1.0);
        let vars = VoiceVars {
            freq: shared(midi_hz(pitch + bend)),
            gate,
            pressure: shared(pressure),
            modulation: shared(modulation),
        };
        let f = |i: usize| {
            let oscs = &settings.oscs;
            (oscs[i].waveform.make_net(&settings, &vars, &oscs[i], i)) * (var(&oscs[i].level) >> follow(0.01)) *
                settings.dsp_component(&vars, ModTarget::Gain) >>
                settings.filter.make_net(&settings, &vars)
        };
        let unit = f(0) + f(1) + f(2) + f(3);
        Self {
            vars,
            base_pitch: pitch,
            release_time: settings.envs.iter().map(|env| env.release).fold(0.0, f32::max),
            event_id: seq.push_relative(0.0, f64::INFINITY, Fade::Smooth, 0.0, 0.0, Box::new(unit)),
        }
    }

    fn off(&self, seq: &mut Sequencer) {
        self.vars.gate.set(0.0);
        seq.edit_relative(self.event_id, self.release_time as f64, 0.01);
    }
}

struct VoiceVars {
    freq: Shared,
    pressure: Shared,
    modulation: Shared,
    gate: Shared,
}