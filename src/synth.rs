//! Subtractive/FM synth engine.

use core::f64;
use std::{collections::{HashMap, VecDeque}, error::Error, fmt::Display, fs, path::Path, u64};

use rand::prelude::*;
use fundsp::hacker32::*;
use serde::{Deserialize, Serialize};

use crate::adsr::adsr_scalable;

const KEY_TRACKING_REF_FREQ: f32 = 261.6; // C4
const SEMITONE_RATIO: f32 = 1.059463; // 12-ET
const VOICE_GAIN: f32 = 0.5; // -6 dB
const VOICES_PER_CHANNEL: usize = 3;

const MAX_ENV_SPEED: f32 = 4.0;

pub const MIN_LFO_RATE: f32 = 0.1;
pub const MAX_LFO_RATE: f32 = 20.0;

pub const MIN_FILTER_CUTOFF: f32 = 20.0;
pub const MAX_FILTER_CUTOFF: f32 = 20_000.0;

pub const MIN_FILTER_RESONANCE: f32 = 0.1;

const PITCH_FLOOR: f32 = 20.0;
const PITCH_CEILING: f32 = 5000.0;

const SMOOTH_TIME: f32 = 0.01;

const FM_DEPTH_MULTIPLIER: f32 = 20.0;

const LFO_DELAY_CURVE: f32 = 3.0; // cubic

pub const MAX_CLIP_GAIN: f32 = 8.0;

// wrap this type so we can serialize it
#[derive(Clone, Serialize, Deserialize)]
#[serde(from = "f32", into = "f32")]
pub struct Parameter(pub Shared);

impl From<f32> for Parameter {
    fn from(value: f32) -> Self {
        Parameter(shared(value))
    }
}

impl Into<f32> for Parameter {
    fn into(self) -> f32 {
        self.0.value()
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
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

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
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

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum Waveform {
    Sawtooth,
    Pulse,
    Triangle,
    Sine,
    Hold,
    Noise,
}

impl Waveform {
    pub const VARIANTS: [Waveform; 6] = [Self::Sawtooth, Self::Pulse, Self::Triangle, Self::Sine, Self::Hold, Self::Noise];

    pub fn name(&self) -> &str {
        match self {
            Self::Sawtooth => "Sawtooth",
            Self::Pulse => "Pulse",
            Self::Triangle => "Triangle",
            Self::Sine => "Sine",
            Self::Hold => "S&H",
            Self::Noise => "Noise",
        }
    }

    fn make_osc_net(&self, settings: &Patch, vars: &VoiceVars, osc: &Oscillator, index: usize, fm_oscs: Net) -> Net {
        let prev_freq = vars.prev_freq.unwrap_or(vars.freq.value());
        let glide_env = envelope2(move |t, x| if t == 0.0 { prev_freq } else { x });
        let base = (var(&vars.freq) >> glide_env >> follow(settings.glide_time * 0.5))
            * var(&osc.freq_ratio.0)
            * var_fn(&osc.fine_pitch.0, |x| pow(SEMITONE_RATIO, x))
            * ((settings.dsp_component(vars, ModTarget::OscPitch(index), &[])
                + settings.dsp_component(vars, ModTarget::Pitch, &[])
                >> shape_fn(|x| pow(16.0, x)))) // 4 octaves
            * ((settings.dsp_component(vars, ModTarget::OscFinePitch(index), &[])
                + settings.dsp_component(vars, ModTarget::FinePitch, &[])
                >> shape_fn(|x| pow(SEMITONE_RATIO, x/2.0))))
            * (1.0 + fm_oscs * FM_DEPTH_MULTIPLIER);
        let tone = var(&osc.tone.0) >> follow(SMOOTH_TIME)
            + settings.dsp_component(vars, ModTarget::Tone(index), &[])
            >> shape_fn(|x| clamp01(x));

        // have to compensate for different volumes. the sine is so loud!
        let au: Box<dyn AudioUnit> = match self {
            Self::Sawtooth => Box::new(base >> saw().phase(random())),
            Self::Pulse => Box::new((base | tone * 0.5 + 0.5) >> pulse().phase(random())),
            Self::Triangle => Box::new(base >> triangle().phase(random())),
            Self::Sine => Box::new(base >> sine().phase(random())),
            Self::Hold => Box::new((noise().seed(random()) | base) >> hold(0.0)),
            Self::Noise => Box::new(noise().seed(random())
                >> (pinkpass() * (1.0 - tone.clone()) ^ pass() * tone)
                >> join::<U2>()),
        };
        Net::wrap(au)
    }

    fn make_lfo_net(&self, settings: &Patch, vars: &VoiceVars, index: usize, path: &[ModSource]) -> Net {
        let lfo = &settings.lfos[index];
        let f = var(&lfo.freq.0)
            * (settings.dsp_component(vars, ModTarget::LFORate(index), path)
            >> shape_fn(|x| pow(MAX_LFO_RATE/MIN_LFO_RATE, x)))
            >> shape_fn(|x| clamp(MIN_LFO_RATE, MAX_LFO_RATE, x));
        let dt = lfo.delay;
        let d = envelope(move |t| clamp01(pow(t / dt, LFO_DELAY_CURVE)));
        let p = vars.lfo_phases[index];
        let au: Box<dyn AudioUnit> = match self {
            Self::Sawtooth => Box::new(f >> saw().phase(p) * d >> follow(0.01)),
            Self::Pulse => Box::new(f >> square().phase(p) * d >> follow(0.01)),
            Self::Triangle => Box::new(f >> triangle().phase(p) * d),
            Self::Sine => Box::new(f >> sine().phase(p) * d),
            Self::Hold => Box::new((noise().seed((p * u64::MAX as f32) as u64) | f) >> hold(0.0) * d >> follow(0.01)),
            Self::Noise => Box::new(pink().seed((p * u64::MAX as f32) as u64) * d),
        };
        Net::wrap(au)
    }

    fn has_tone_control(&self) -> bool {
        match *self {
            Waveform::Pulse | Waveform::Noise => true,
            _ => false,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum FilterType {
    Ladder,
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
}

impl FilterType {
    pub const VARIANTS: [FilterType; 5] =
        [Self::Ladder, Self::Lowpass, Self::Highpass, Self::Bandpass, Self::Notch];
    
    pub fn name(&self) -> &str {
        match self {
            Self::Ladder => "Ladder",
            Self::Lowpass => "Lowpass",
            Self::Highpass => "Highpass",
            Self::Bandpass => "Bandpass",
            Self::Notch => "Notch",
        }
    }
}

const DEFAULT_PRESSURE: f32 = 2.0/3.0; // equivalent to a 6 in pattern

/// A Synth orchestrates the playing of patches.
pub struct Synth {
    active_voices: HashMap<Key, Voice>,
    released_voices: Vec<VecDeque<Voice>>, // per channel
    bend_memory: Vec<f32>,
    mod_memory: Vec<f32>,
    pressure_memory: Vec<f32>,
    prev_freq: Option<f32>,
}

impl Synth {
    pub fn new() -> Self {
        Self {
            active_voices: HashMap::new(),
            released_voices: vec![VecDeque::new()],
            bend_memory: vec![0.0],
            mod_memory: vec![0.0],
            pressure_memory: vec![DEFAULT_PRESSURE],
            prev_freq: None,
        }
    }

    pub fn reset_memory(&mut self) {
        self.bend_memory.fill(0.0);
        self.mod_memory.fill(0.0);
        self.pressure_memory.fill(DEFAULT_PRESSURE);
        self.prev_freq = None;
    }

    fn expand_memory(&mut self, index: usize) {
        while self.bend_memory.len() <= index {
            self.bend_memory.push(0.0);
        }
        while self.mod_memory.len() <= index {
            self.mod_memory.push(0.0);
        }
        while self.pressure_memory.len() <= index {
            self.pressure_memory.push(DEFAULT_PRESSURE);
        }
        while self.released_voices.len() <= index {
            self.released_voices.push(VecDeque::new());
        }
    }

    /// If pressure is None, use memory.
    pub fn note_on(&mut self, key: Key, pitch: f32, pressure: Option<f32>,
        patch: &Patch, seq: &mut Sequencer
    ) {
        // turn off prev note(s) in channel
        if key.origin == KeyOrigin::Pattern {
            let removed_keys: Vec<Key> = self.active_voices.keys()
                .filter(|k| k.origin == key.origin && k.channel == key.channel)
                .cloned()
                .collect();
            for key in removed_keys {
                if let Some(voice) = self.active_voices.remove(&key) {
                    voice.off(seq);
                    self.released_voices[key.channel as usize].push_back(voice);
                }
            }
        }

        let bend = if key.origin == KeyOrigin::Midi {
            self.expand_memory(key.channel as usize);
            self.bend_memory[key.channel as usize]
        } else {
            0.0
        };
        let insert_voice = match patch.play_mode {
            PlayMode::Poly => true,
            PlayMode::Mono => {
                for (key, voice) in self.active_voices.drain() {
                    voice.off(seq);
                    self.released_voices[key.channel as usize].push_back(voice);
                }
                true
            },
            PlayMode::SingleTrigger => {
                if self.active_voices.is_empty() {
                    true
                } else {
                    let voice = self.active_voices.drain().map(|(_, v)| v).next()
                        .expect("voices confirmed non-empty");
                    voice.vars.freq.set(midi_hz(pitch));
                    self.active_voices.insert(key.clone(), voice);
                    false
                }
            },
        };
        if insert_voice {
            let channel = key.channel as usize;
            self.expand_memory(channel);
            let pressure = if let Some(p) = pressure {
                self.pressure_memory[channel] = p;
                p
            } else {
                self.pressure_memory[channel]
            };
            self.active_voices.insert(key, Voice::new(pitch, bend, pressure,
                self.mod_memory[channel], self.prev_freq, &patch, seq));
            self.check_truncate_voices(channel, seq);
            self.prev_freq = Some(midi_hz(pitch));
        }
    }

    /// Cut the oldest released voice if max_voices is exceeded.
    fn check_truncate_voices(&mut self, channel: usize, seq: &mut Sequencer) {
        if self.released_voices[channel].len() >= VOICES_PER_CHANNEL {
            let voice = self.released_voices[channel].pop_front().unwrap();
            voice.cut(seq);
        }
    }

    pub fn note_off(&mut self, key: Key, seq: &mut Sequencer) {
        if let Some(voice) = self.active_voices.remove(&key) {
            voice.off(seq);
            self.released_voices[key.channel as usize].push_back(voice);
        }
    }

    /// Turns off all notes from a specific origin.
    pub fn clear_notes_with_origin(&mut self, seq: &mut Sequencer, origin: KeyOrigin) {
        let remove_keys: Vec<_> = self.active_voices.keys()
            .filter(|k| k.origin == origin)
            .cloned()
            .collect();
        for k in remove_keys {
            let voice = self.active_voices.remove(&k).unwrap();
            voice.off(seq);
            self.released_voices[k.channel as usize].push_back(voice);
        }
    }

    /// Turns off all notes.
    pub fn clear_all_notes(&mut self, seq: &mut Sequencer) {
        for (k, voice) in self.active_voices.drain() {
            voice.off(seq);
            self.released_voices[k.channel as usize].push_back(voice);
        }
    }

    pub fn pitch_bend(&mut self, channel: u8, bend: f32) {
        self.expand_memory(channel as usize);
        self.bend_memory[channel as usize] = bend;
        for (key, voice) in self.active_voices.iter_mut() {
            if key.origin == KeyOrigin::Midi && key.channel == channel {
                voice.vars.freq.set(midi_hz(voice.base_pitch + bend));
            }
        }
    }

    pub fn poly_pressure(&mut self, key: Key, pressure: f32) {
        self.active_voices.get(&key).inspect(|v| v.vars.pressure.set(pressure));
    }

    pub fn channel_pressure(&mut self, channel: u8, pressure: f32) {
        self.expand_memory(channel as usize);
        self.pressure_memory[channel as usize] = pressure;
        for (key, voice) in self.active_voices.iter_mut() {
            if key.channel == channel {
                voice.vars.pressure.set(pressure);
            }
        }
    }

    pub fn modulate(&mut self, channel: u8, depth: f32) {
        self.expand_memory(channel as usize);
        self.mod_memory[channel as usize] = depth;
        for (key, voice) in self.active_voices.iter_mut() {
            if key.channel == channel {
                voice.vars.modulation.set(depth);
            }
        }
    }
}

/// A Patch is a configuration of synthesis parameters.
#[derive(Serialize, Deserialize)]
pub struct Patch {
    pub name: String,
    pub gain: Parameter,
    pub pan: Parameter, // range -1..1
    pub glide_time: f32,
    pub play_mode: PlayMode,
    pub filters: Vec<Filter>,
    pub oscs: Vec<Oscillator>,
    pub envs: Vec<ADSR>,
    pub lfos: Vec<LFO>,
    pub mod_matrix: Vec<Modulation>,
    #[serde(default = "one_parameter")]
    pub reverb_send: Parameter,
    #[serde(default = "one_parameter")]
    pub clip_gain: Parameter,
}

fn one_parameter() -> Parameter {
    Parameter(shared(1.0))
}

impl Patch {
    pub fn new() -> Self {
        Self {
            name: String::from("init"),
            gain: Parameter(shared(0.5)),
            reverb_send: Parameter(shared(1.0)),
            clip_gain: Parameter(shared(1.0)),
            oscs: vec![Oscillator::new()],
            envs: vec![ADSR::new()],
            filters: Vec::new(),
            lfos: Vec::new(),
            play_mode: PlayMode::Poly,
            glide_time: 0.0,
            pan: Parameter(shared(0.0)),
            mod_matrix: vec![Modulation {
                source: ModSource::Envelope(0),
                target: ModTarget::Gain,
                depth: Parameter(shared(1.0)),
            }, Modulation {
                source: ModSource::Pressure,
                target: ModTarget::Gain,
                depth: Parameter(shared(1.0)),
            }],
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let input = fs::read(path)?;
        Ok(rmp_serde::from_slice::<Self>(&input)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let contents = rmp_serde::to_vec(self)?;
        Ok(fs::write(path, contents)?)
    }

    pub fn duplicate(&self) -> Result<Self, Box<dyn Error>> {
        let data = rmp_serde::to_vec(self)?;
        let mut patch = rmp_serde::from_slice::<Self>(&data)?;
        patch.name = format!("Copy of {}", patch.name);
        Ok(patch)
    }

    fn dsp_component(&self, vars: &VoiceVars, target: ModTarget, path: &[ModSource]) -> Net {
        let mut net = Net::wrap(Box::new(constant(if target.is_additive() { 0.0 } else { 1.0 })));
        for (i, m) in self.mod_matrix.iter().enumerate() {
            if m.target == target && !path.contains(&m.source) {
                if target.is_additive() {
                    net = net + m.dsp_component(&self, &vars, i, path);
                } else {
                    net = net * m.dsp_component(&self, &vars, i, path);
                }
            }
        }
        net
    }

    pub fn mod_sources(&self) -> Vec<ModSource> {
        let mut v = vec![ModSource::Pitch, ModSource::Pressure, ModSource::Modulation, ModSource::Random];
        for i in 0..self.envs.len() {
            v.push(ModSource::Envelope(i));
        }
        for i in 0..self.lfos.len() {
            v.push(ModSource::LFO(i));
        }
        v
    }

    pub fn mod_targets(&self) -> Vec<ModTarget> {
        let mut v = vec![ModTarget::Gain, ModTarget::Pan, ModTarget::Pitch, ModTarget::FinePitch];
        for (i, osc) in self.oscs.iter().enumerate() {
            v.push(ModTarget::Level(i));
            v.push(ModTarget::OscPitch(i));
            v.push(ModTarget::OscFinePitch(i));
            if osc.waveform.has_tone_control() {
                v.push(ModTarget::Tone(i));
            }
            v.push(ModTarget::ClipGain);
        }
        for i in 0..self.filters.len() {
            v.push(ModTarget::FilterCutoff(i));
            v.push(ModTarget::FilterQ(i));
        }
        for i in 0..self.envs.len() {
            v.push(ModTarget::EnvSpeed(i));
        }
        for i in 0..self.lfos.len() {
            v.push(ModTarget::LFORate(i));
        }
        for i in 0..self.mod_matrix.len() {
            v.push(ModTarget::ModDepth(i));
        }
        v
    }

    pub fn remove_osc(&mut self, i: usize) {
        if i < self.oscs.len() {
            self.oscs.remove(i);

            // update outputs for new osc indices
            for (j, osc) in self.oscs.iter_mut().enumerate() {
                if j == 0 {
                    // first osc always has normal output
                    osc.output = OscOutput::Mix(0);
                } else {
                    match osc.output {
                        OscOutput::Mix(n) | OscOutput::AM(n) | OscOutput::FM(n) if n == i =>
                            osc.output = OscOutput::Mix(0),
                        OscOutput::Mix(n) if n > i => osc.output = OscOutput::Mix(n - 1),
                        OscOutput::AM(n) if n > i => osc.output = OscOutput::AM(n - 1),
                        OscOutput::FM(n) if n > i => osc.output = OscOutput::FM(n - 1),
                        _ => (),
                    }
                }
            }

            // update mod matrix for new osc indices
            self.mod_matrix.retain(|m| m.target.osc() != Some(i));
            for m in self.mod_matrix.iter_mut() {
                if let Some(n) = m.target.osc() {
                    if n > i {
                        m.target = ModTarget::Tone(n - 1);
                    }
                }
            }
        }
    }
    
    pub fn remove_filter(&mut self, i: usize) {
        if i < self.filters.len() {
            self.filters.remove(i);

            // update mod matrix for new indices
            self.mod_matrix.retain(|m| m.target != ModTarget::FilterCutoff(i)
                && m.target != ModTarget::FilterQ(i));
            for m in self.mod_matrix.iter_mut() {
                if let ModTarget::FilterCutoff(n) = m.target {
                    if n > i {
                        m.target = ModTarget::FilterCutoff(n - 1);
                    }
                } else if let ModTarget::FilterQ(n) = m.target {
                    if n > i {
                        m.target = ModTarget::FilterQ(n - 1);
                    }
                }
            }
        }
    }

    pub fn remove_env(&mut self, i: usize) {
        if i < self.envs.len() {
            self.envs.remove(i);

            // update mod matrix for new indices
            self.mod_matrix.retain(|m| m.source != ModSource::Envelope(i));
            for m in self.mod_matrix.iter_mut() {
                if let ModSource::Envelope(n) = m.source {
                    if n > i {
                        m.source = ModSource::Envelope(n - 1);
                    }
                }
            }
        }
    }

    pub fn remove_lfo(&mut self, i: usize) {
        if i < self.lfos.len() {
            self.lfos.remove(i);

            // update mod matrix for new indices
            self.mod_matrix.retain(|m|
                m.source != ModSource::LFO(i) && m.target != ModTarget::LFORate(i));
            for m in self.mod_matrix.iter_mut() {
                if let ModSource::LFO(n) = m.source {
                    if n > i {
                        m.source = ModSource::LFO(n - 1);
                    }
                }
                if let ModTarget::LFORate(n) = m.target {
                    if n > i {
                        m.target = ModTarget::LFORate(n - 1)
                    }
                }
            }
        }
    }

    pub fn remove_mod(&mut self, i: usize) {
        if i < self.mod_matrix.len() {
            self.mod_matrix.remove(i);

            // update mod matrix for new indices
            self.mod_matrix.retain(|m| m.target != ModTarget::ModDepth(i));
            for m in self.mod_matrix.iter_mut() {
                if let ModTarget::ModDepth(n) = m.target {
                    if n > i {
                        m.target = ModTarget::ModDepth(n - 1);
                    }
                }
            }
        }
    }

    fn make_osc(&self, i: usize, vars: &VoiceVars) -> Net {
        let mut mixed_oscs = Net::new(0, 1);
        let mut am_oscs = Net::wrap(Box::new(constant(1.0)));
        let mut fm_oscs = Net::new(0, 1);
        for (j, osc) in self.oscs.iter().enumerate() {
            if j > i {
                if osc.output == OscOutput::Mix(i) {
                    mixed_oscs = mixed_oscs + self.make_osc(j, vars);
                } else if osc.output == OscOutput::AM(i) {
                    am_oscs = am_oscs * self.make_osc(j, vars);
                } else if osc.output == OscOutput::FM(i) {
                    fm_oscs = fm_oscs + self.make_osc(j, vars);
                }
            }
        }

        let level = (var(&self.oscs[i].level.0) >> follow(0.01))
            * self.dsp_component(&vars, ModTarget::Level(i), &[]);

        (self.oscs[i].waveform.make_osc_net(self, &vars, &self.oscs[i], i, fm_oscs))
            * level
            * am_oscs
            + mixed_oscs
    }

    fn make_filter_net(&self, vars: &VoiceVars) -> Net {
        let mut net = Net::wrap(Box::new(pass()));
        for (i, filter) in self.filters.iter().enumerate() {
            net = net >> filter.make_net(self, vars, i);
        }
        net
    }

    /// Returns true unless gain is modulated by an envelope with zero sustain.
    pub fn sustains(&self) -> bool {
        for m in &self.mod_matrix {
            if m.target == ModTarget::Gain {
                if let ModSource::Envelope(i) = m.source {
                    if let Some(env) = self.envs.get(i) {
                        if env.sustain == 0.0 {
                            return false
                        }
                    }
                }
            }
        }
        
        true
    }
}

#[derive(Serialize, Deserialize)]
pub struct Oscillator {
    pub level: Parameter,
    pub tone: Parameter,
    pub freq_ratio: Parameter,
    pub fine_pitch: Parameter,
    pub waveform: Waveform,
    pub output: OscOutput,
}

impl Oscillator {
    pub fn new() -> Self {
        Self {
            level: Parameter(shared(1.0)),
            tone: Parameter(shared(0.0)),
            freq_ratio: Parameter(shared(1.0)),
            fine_pitch: Parameter(shared(0.0)),
            waveform: Waveform::Sine,
            output: OscOutput::Mix(0),
        }
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum OscOutput {
    Mix(usize),
    AM(usize),
    FM(usize),
}

impl OscOutput {
    pub fn choices(index: usize) -> Vec<OscOutput> {
        if index == 0 {
            vec![OscOutput::Mix(0)]
        } else {
            (0..index).flat_map(|i| if i + 1 == index {
                // only allow modulating the oscillator directly to the left
                vec![OscOutput::Mix(i), OscOutput::AM(i), OscOutput::FM(i)]
            } else {
                vec![OscOutput::Mix(i)]
            }).collect()
        }
    }
}

impl Display for OscOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Mix(i) if *i == 0 => "Mix",
            Self::Mix(i) => &format!("Mix to osc {}", i + 1),
            Self::AM(i) => &format!("RM to osc {}", i + 1),
            Self::FM(i) => &format!("FM to osc {}", i + 1),
        };
        f.write_str(s)
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct Filter {
    pub filter_type: FilterType,
    pub cutoff: Parameter,
    pub resonance: Parameter,
    pub key_tracking: KeyTracking,
}

impl Filter {
    pub fn new() -> Self {
        Self {
            cutoff: Parameter(shared(MAX_FILTER_CUTOFF)),
            resonance: Parameter(shared(MIN_FILTER_RESONANCE)),
            key_tracking: KeyTracking::None,
            filter_type: FilterType::Ladder,
        }
    }

    fn make_net(&self, settings: &Patch, vars: &VoiceVars, index: usize) -> Net {
        let kt = match self.key_tracking {
            KeyTracking::None => Net::wrap(Box::new(constant(1.0))),
            KeyTracking::Partial => Net::wrap(Box::new(var_fn(&vars.freq, |x| pow(x * 1.0/KEY_TRACKING_REF_FREQ, 0.5)))),
            KeyTracking::Full => Net::wrap(Box::new(var(&vars.freq) * (1.0/KEY_TRACKING_REF_FREQ))),
        };
        let cutoff_mod = settings.dsp_component(vars, ModTarget::FilterCutoff(index), &[])
            >> shape_fn(|x| pow(1000.0, x));
        let reso_mod = settings.dsp_component(vars, ModTarget::FilterQ(index), &[]);
        let f = match self.filter_type {
            FilterType::Ladder => Net::wrap(Box::new(moog())),
            FilterType::Lowpass => Net::wrap(Box::new(lowpass())),
            FilterType::Highpass => Net::wrap(Box::new(highpass())),
            FilterType::Bandpass => Net::wrap(Box::new(bandpass())),
            FilterType::Notch => Net::wrap(Box::new(notch())),
        };
        (pass()
            | var(&self.cutoff.0) * kt * cutoff_mod
                >> shape_fn(|x| clamp(MIN_FILTER_CUTOFF, MAX_FILTER_CUTOFF, x))
            | var(&self.resonance.0) + reso_mod) >> f
    }
}

#[derive(Serialize, Deserialize)]
pub struct ADSR {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub power: f32,
}

impl ADSR {
    pub fn new() -> Self {
        Self {
            attack: 0.01,
            decay: 1.0,
            sustain: 1.0,
            release: 0.01,
            power: 2.0,
        }
    }

    fn make_node(&self, settings: &Patch, vars: &VoiceVars, index: usize, path: &[ModSource]) -> Net {
        let attack = self.attack;
        let power = self.power;
        let scale = settings.dsp_component(vars, ModTarget::EnvSpeed(index), path)
            >> shape_fn(|x| pow(MAX_ENV_SPEED, -x));
        Net::wrap(Box::new(
            (var(&vars.gate) | scale) >> adsr_scalable(self.attack, self.decay, self.sustain, self.release)
                >> envelope2(move |t, x| if t < attack {
                    pow(x, 1.0/power)
                } else {
                    pow(x, power)
                })))
    }

    pub fn curve_name(&self) -> &'static str {
        match self.power {
            1.0 => "Linear",
            2.0 => "Quadratic",
            3.0 => "Cubic",
            _ => "Unknown",
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LFO {
    pub waveform: Waveform,
    pub freq: Parameter,
    pub delay: f32,
}

impl LFO {
    pub fn new() -> Self {
        Self {
            waveform: Waveform::Triangle,
            freq: Parameter(shared(1.0)),
            delay: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Modulation {
    pub source: ModSource,
    pub target: ModTarget,
    pub depth: Parameter,
}

impl Modulation {
    pub fn default() -> Self {
        Self {
            source: ModSource::Modulation,
            target: ModTarget::Gain,
            depth: Parameter(shared(0.0)),
        }
    }

    fn dsp_component(&self, settings: &Patch, vars: &VoiceVars, index: usize, path: &[ModSource]) -> Net {
        let mut path = path.to_vec();
        path.push(self.source);
        let net = match self.source {
            ModSource::Pitch => Net::wrap(Box::new(var_fn(&vars.freq,|f| dexerp(PITCH_FLOOR, PITCH_CEILING, f)))),
            ModSource::Pressure => Net::wrap(Box::new(var(&vars.pressure) >> follow(SMOOTH_TIME))),
            ModSource::Modulation => Net::wrap(Box::new(var(&vars.modulation) >> follow(SMOOTH_TIME))),
            ModSource::Random => Net::wrap(Box::new(constant(vars.random_values[index]))),
            ModSource::Envelope(i) => match settings.envs.get(i) {
                Some(env) => Net::wrap(Box::new(env.make_node(settings, vars, i, &path))),
                None => Net::wrap(Box::new(zero())),
            },
            ModSource::LFO(i) => match settings.lfos.get(i) {
                Some(osc) => Net::wrap(Box::new(osc.waveform.make_lfo_net(settings, vars, i, &path))),
                None => Net::wrap(Box::new(zero())),
            }
        };
        let d = var(&self.depth.0) >> follow(SMOOTH_TIME) + settings.dsp_component(vars, ModTarget::ModDepth(index), &path);
        if self.target.is_additive() {
            net * d
        } else if self.source.is_bipolar() {
            1.0 - d * (1.0 - 0.5 * (net + 1.0))
        } else {
            1.0 - d * (1.0 - net)
        }
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum ModSource {
    Pitch,
    Pressure,
    Modulation,
    Random,
    Envelope(usize),
    LFO(usize),
}

impl Display for ModSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pitch => "Pitch",
            Self::Pressure => "Pressure",
            Self::Modulation => "Mod wheel",
            Self::Random => "Random",
            Self::Envelope(i) => &format!("Envelope {}", i + 1),
            Self::LFO(i) => &format!("LFO {}", i + 1),
        };
        f.write_str(s)
    }
}

impl ModSource {
    fn is_bipolar(&self) -> bool {
        match *self {
            ModSource::LFO(_) => true,
            _ => false,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum ModTarget {
    Gain,
    Pan,
    Pitch,
    FinePitch,
    Level(usize),
    OscPitch(usize),
    OscFinePitch(usize),
    Tone(usize),
    FilterCutoff(usize),
    FilterQ(usize),
    EnvSpeed(usize),
    LFORate(usize),
    ModDepth(usize),
    ClipGain,
}

impl ModTarget {
    pub fn is_additive(&self) -> bool {
        match *self  {
            ModTarget::Gain | ModTarget::Level(_) => false,
            _ => true,
        }
    }

    fn osc(&self) -> Option<usize> {
        match *self {
            ModTarget::Level(n) | ModTarget::OscPitch(n) |
                ModTarget::OscFinePitch(n) | ModTarget::Tone(n) => Some(n),
            _ => None,
        }
    }
}

impl Display for ModTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Gain => "Gain",
            Self::Pan => "Pan",
            Self::Pitch => "Pitch",
            Self::FinePitch => "Fine pitch",
            Self::Level(n) => &format!("Osc {} level", n + 1),
            Self::OscPitch(n) => &format!("Osc {} pitch", n + 1),
            Self::OscFinePitch(n) => &format!("Osc {} fine pitch", n + 1),
            Self::Tone(n) => &format!("Osc {} tone", n + 1),
            Self::FilterCutoff(n) => &format!("Filter {} freq", n + 1),
            Self::FilterQ(n) => &format!("Filter {} reso", n + 1),
            Self::EnvSpeed(n) => &format!("Env {} speed", n + 1),
            Self::LFORate(n) => &format!("LFO {} freq", n + 1),
            Self::ModDepth(n) => &format!("Mod {} depth", n + 1),
            Self::ClipGain => "Distortion",
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
    fn new(pitch: f32, bend: f32, pressure: f32, modulation: f32, prev_freq: Option<f32>,
        settings: &Patch, seq: &mut Sequencer
    ) -> Self {
        let gate = shared(1.0);
        let vars = VoiceVars {
            freq: shared(midi_hz(pitch + bend)),
            gate,
            pressure: shared(pressure),
            modulation: shared(modulation),
            random_values: settings.mod_matrix.iter().map(|_| random()).collect(),
            lfo_phases: settings.lfos.iter().map(|_| random()).collect(),
            prev_freq,
        };
        let gain = (var(&settings.gain.0) >> follow(SMOOTH_TIME))
            * settings.dsp_component(&vars, ModTarget::Gain, &[])
            * VOICE_GAIN;
        let filter_net = settings.make_filter_net(&vars);

        // use dry signal if clip gain is set to 1.0
        let clip = pass()
            * (var(&settings.clip_gain.0)
                + settings.dsp_component(&vars, ModTarget::ClipGain, &[]) * MAX_CLIP_GAIN
                >> shape_fn(|x| if x == 1.0 { 1.0 } else { 0.0 }))
            & pass()
            * (var(&settings.clip_gain.0)
                + settings.dsp_component(&vars, ModTarget::ClipGain, &[]) * MAX_CLIP_GAIN
                >> shape_fn(|x| x * if x == 1.0 { 0.0 } else { 1.0 }))
            >> shape(Clip(1.0));

        let net = ((settings.make_osc(0, &vars) >> filter_net >> clip) * gain
            | var(&settings.pan.0) >> follow(SMOOTH_TIME)
                + settings.dsp_component(&vars, ModTarget::Pan, &[]) >> shape_fn(|x| clamp11(x)))
            >> panner() >> multisplit::<U2, U2>()
            >> (multipass::<U2>()
                | multipass::<U2>() * (var(&settings.reverb_send.0) >> split::<U2>()));
        Self {
            vars,
            base_pitch: pitch,
            release_time: settings.envs.iter()
                .map(|env| (env.attack + env.release) * MAX_ENV_SPEED)
                .fold(0.0, f32::max),
            event_id: seq.push_relative(0.0, f64::INFINITY, Fade::Smooth, 0.0, 0.0, Box::new(net)),
        }
    }

    fn off(&self, seq: &mut Sequencer) {
        self.vars.gate.set(0.0);
        seq.edit_relative(self.event_id, self.release_time as f64, SMOOTH_TIME as f64);
    }

    fn cut(&self, seq: &mut Sequencer) {
        seq.edit_relative(self.event_id, 0.0, SMOOTH_TIME as f64);
    }
}

struct VoiceVars {
    freq: Shared,
    pressure: Shared,
    modulation: Shared,
    gate: Shared,
    random_values: Vec<f32>,
    lfo_phases: Vec<f32>,
    prev_freq: Option<f32>,
}