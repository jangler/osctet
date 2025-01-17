//! Subtractive/FM synth engine.

pub(crate) mod pcm;

use core::f64;
use std::{collections::{HashMap, VecDeque}, error::Error, fmt::Display, fs, path::Path};

use pcm::PcmData;
use rand::prelude::*;
use fundsp::hacker32::*;
use serde::{Deserialize, Serialize};

use crate::{dsp::*, ui::MAX_PATCH_NAME_CHARS};

/// The MIDI pitch of the default note (C4). Used to adjust frequency controls
/// of loaded samples.
pub const REF_PITCH: f64 = 60.0;

/// The frequency of the middle note (C4). This seems not to cause any problems
/// despite the actual frequency of C4 changing with each tuning.
pub const REF_FREQ: f32 = 261.6256;

/// Frequency ratio of one semitone in 12-ET.
const SEMITONE_RATIO: f32 = 1.059463;

/// Maximum voices that can be playing at one time in a channel, including
/// voices in the release phase.
const VOICES_PER_CHANNEL: usize = 3;

/// Maximum scale when modulating envelopes. The minimum is just the inverse.
pub const MAX_ENV_SCALE: f32 = 16.0;

pub const MIN_FREQ_RATIO: f32 = 0.25;
pub const MAX_FREQ_RATIO: f32 = 16.0;

// (Hz)
pub const MIN_LFO_RATE: f32 = 0.1;
pub const MAX_LFO_RATE: f32 = 20.0;

// (Hz)
pub const MIN_FILTER_CUTOFF: f32 = 20.0;
pub const MAX_FILTER_CUTOFF: f32 = 22_000.0;

pub const FILTER_CUTOFF_MOD_BASE: f32 = MAX_FILTER_CUTOFF / MIN_FILTER_CUTOFF;

/// FunDSP's filters drop a lot in level near zero resonance, so limit the
/// minimum resonance.
pub const MIN_FILTER_RESONANCE: f32 = 0.1;

/// Minimum Hz value for pitch-based modulation (E1).
const PITCH_FLOOR: f32 = 41.25;

/// Maximum Hz value for pitch-based modulation (E7).
const PITCH_CEILING: f32 = 2640.0;

/// Maximum pitch modulation multiplier. The minimum is just the inverse.
pub const MAX_PITCH_MOD: f32 = 16.0;

/// Smoothing time for transitions, in seconds.
pub const SMOOTH_TIME: f32 = 0.01;

/// Arbitrary constant for scaling FM depth.
const FM_DEPTH_MULTIPLIER: f32 = 20.0;

/// Use a cubic attack envelope for LFO delay.
const LFO_DELAY_CURVE: f32 = 3.0;

/// Wraps a Shared value for serialization.
/// Cloning creates a new Shared value.
#[derive(Serialize, Deserialize)]
#[serde(from = "f32", into = "f32")]
pub struct Parameter(pub Shared);

impl Clone for Parameter {
    fn clone(&self) -> Self {
        Self(shared(self.0.value()))
    }
}

impl From<f32> for Parameter {
    fn from(value: f32) -> Self {
        Parameter(shared(value))
    }
}

impl From<Parameter> for f32 {
    fn from(value: Parameter) -> Self {
        value.0.value()
    }
}

impl Default for Parameter {
    fn default() -> Self {
        Self(shared(1.0))
    }
}

/// Source type for note keys.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum KeyOrigin {
    Keyboard,
    Midi,
    Pattern,
}

/// Source for note keys, to track on/offs.
#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Key {
    pub origin: KeyOrigin,
    pub channel: u8,
    pub key: u8,
}

/// How to behave when a note starts before the last has ended.
#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum PlayMode {
    Poly,
    Mono,
    SingleTrigger,
}

impl PlayMode {
    pub const VARIANTS: [PlayMode; 3] = [Self::Poly, Self::Mono, Self::SingleTrigger];

    /// Returns the UI string for this play mode.
    pub fn name(&self) -> &str {
        match self {
            Self::Poly => "Poly",
            Self::Mono => "Mono",
            Self::SingleTrigger => "Single trigger",
        }
    }
}

/// Generator/LFO wave source.
#[derive(Clone, Serialize, Deserialize)]
pub enum Waveform {
    Sawtooth,
    Pulse,
    Triangle,
    Sine,
    Hold,
    Noise,
    Pcm(Option<PcmData>),
}

impl Waveform {
    /// Variants that generators can be set to.
    pub const VARIANTS: [Waveform; 7] = [
        Self::Sawtooth,
        Self::Pulse,
        Self::Triangle,
        Self::Sine,
        Self::Hold,
        Self::Noise,
        Self::Pcm(None),
    ];

    /// Variants that LFOs can be set to.
    pub const LFO_VARIANTS: [Waveform; 6] = [
        Self::Sawtooth,
        Self::Pulse,
        Self::Triangle,
        Self::Sine,
        Self::Hold,
        Self::Noise,
    ];

    /// Returns UI string.
    pub fn name(&self) -> &str {
        match self {
            Self::Sawtooth => "Sawtooth",
            Self::Pulse => "Pulse",
            Self::Triangle => "Triangle",
            Self::Sine => "Sine",
            Self::Hold => "S&H",
            Self::Noise => "Noise",
            Self::Pcm(_) => "PCM",
        }
    }

    /// Returns true if this waveform makes use of the `tone` control.
    pub fn uses_tone(&self) -> bool {
        matches!(self, Self::Pulse | Self::Noise)
    }

    /// Returns true if this waveform makes uses of frequency controls.
    pub fn uses_freq(&self) -> bool {
        !matches!(self, Self::Noise)
    }

    /// Check whether this waveform is affected by the "tone" control.
    fn has_tone_control(&self) -> bool {
        matches!(*self, Waveform::Pulse | Waveform::Noise)
    }

    /// Check whether this waveform can use oversampling.
    pub fn uses_oversampling(&self) -> bool {
        !matches!(*self, Waveform::Hold | Waveform::Noise | Waveform::Pcm(_))
    }
}

/// Default pressure at song start. Equivalent to 0xA/0xF.
pub const DEFAULT_PRESSURE: f32 = 2.0/3.0;

/// A Synth orchestrates the playing of voices.
pub struct Synth {
    /// Voices that are "on".
    active_voices: HashMap<Key, Voice>,
    /// Voices that are "off" (releasing), but not yet deallocated.
    released_voices: Vec<VecDeque<Voice>>,
    /// Per-channel pitch bend memory.
    bend_memory: Vec<f32>,
    /// Per-channel modulation level memory.
    mod_memory: Vec<f32>,
    /// Per-channel pressure level memory.
    pressure_memory: Vec<f32>,
    /// Previous frequency played by any note.
    prev_freq: Option<f32>,
    /// Sample rate to pass when creating DSP.
    sample_rate: f32,
    /// If true, note-ons are ignored.
    pub muted: bool,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            active_voices: HashMap::new(),
            released_voices: vec![VecDeque::new()],
            bend_memory: vec![0.0],
            mod_memory: vec![0.0],
            pressure_memory: vec![DEFAULT_PRESSURE],
            prev_freq: None,
            sample_rate,
            muted: false,
        }
    }

    /// Reset channel-state-type memory.
    pub fn reset_memory(&mut self) {
        self.bend_memory.fill(0.0);
        self.mod_memory.fill(0.0);
        self.pressure_memory.fill(DEFAULT_PRESSURE);
        self.prev_freq = None;
    }

    /// Add channel memory slots until `index` is in bounds.
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

    /// Start a note. If pressure is None, use memory.
    pub fn note_on(&mut self, key: Key, pitch: f32, pressure: Option<f32>,
        patch: &Patch, seq: &mut Sequencer, pan_polarity: &Shared,
    ) {
        if self.muted {
            return
        }

        // turn off prev note(s) in channel
        if key.origin == KeyOrigin::Pattern {
            let removed_keys: Vec<Key> = self.active_voices.keys()
                .filter(|k| k.origin == key.origin && k.channel == key.channel)
                .cloned().collect();
            for key in removed_keys {
                if let Some(voice) = self.active_voices.remove(&key) {
                    voice.off(seq);
                    self.released_voices[key.channel as usize].push_back(voice);
                }
            }
        }

        // calculate pitch bend
        let bend = if key.origin == KeyOrigin::Midi {
            self.expand_memory(key.channel as usize);
            self.bend_memory[key.channel as usize]
        } else {
            0.0
        };

        // handle play mode behavior & determine whether to insert a new voice
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
            let voice = Voice::new(pitch, bend, pressure, self.mod_memory[channel],
                self.prev_freq, patch, seq, self.sample_rate, pan_polarity);

            self.active_voices.insert(key, voice);
            self.check_truncate_voices(channel, seq);
            self.prev_freq = Some(midi_hz(pitch));
        }
    }

    /// Cut the oldest released voice if max_voices is exceeded.
    fn check_truncate_voices(&mut self, channel: usize, seq: &mut Sequencer) {
        if self.released_voices[channel].len() >= VOICES_PER_CHANNEL {
            let voice = self.released_voices[channel].pop_front()
                .expect("released voice count confirmed to be nonzero");
            voice.cut(seq);
        }
    }

    /// Handle a note off event.
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
            .cloned().collect();

        for k in remove_keys {
            let voice = self.active_voices.remove(&k)
                .expect("key taken from map should be valid");
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

    /// Cuts all notes.
    pub fn panic(&mut self, seq: &mut Sequencer) {
        for (_, voice) in self.active_voices.drain() {
            voice.cut(seq);
        }
        for channel in &mut self.released_voices {
            for voice in channel.drain(..) {
                voice.cut(seq);
            }
        }
    }

    /// Handle a MIDI-style pitch bend.
    pub fn pitch_bend(&mut self, channel: u8, bend: f32) {
        self.expand_memory(channel as usize);
        self.bend_memory[channel as usize] = bend;
        for (key, voice) in self.active_voices.iter_mut() {
            if key.origin == KeyOrigin::Midi && key.channel == channel {
                voice.vars.freq.set(midi_hz(voice.base_pitch + bend));
            }
        }
    }

    /// Set `key` note's MIDI pitch.
    pub fn bend_to(&mut self, key: Key, pitch: f32) {
        if let Some(voice) = self.active_voices.get_mut(&key) {
            self.prev_freq = Some(midi_hz(pitch));
            voice.base_pitch = pitch;
            voice.vars.freq.set(midi_hz(pitch));
        }
    }

    /// Handle polyphonic aftertouch.
    pub fn poly_pressure(&mut self, key: Key, pressure: f32) {
        if let Some(v) = self.active_voices.get(&key) {
            v.vars.pressure.set(pressure);
        }
    }

    /// Handle MIDI-channel-based aftertouch.
    pub fn channel_pressure(&mut self, channel: u8, pressure: f32) {
        self.set_vel_memory(channel, pressure);
        for (key, voice) in self.active_voices.iter_mut() {
            if key.channel == channel {
                voice.vars.pressure.set(pressure);
            }
        }
    }

    /// Set modulation level.
    pub fn modulate(&mut self, channel: u8, depth: f32) {
        self.set_mod_memory(channel, depth);
        for (key, voice) in self.active_voices.iter_mut() {
            if key.channel == channel {
                voice.vars.modulation.set(depth);
            }
        }
    }

    /// Set pressure that new notes will use.
    pub fn set_vel_memory(&mut self, channel: u8, pressure: f32) {
        self.expand_memory(channel as usize);
        self.pressure_memory[channel as usize] = pressure;
    }

    /// Set modulation level that new notes will use.
    pub fn set_mod_memory(&mut self, channel: u8, depth: f32) {
        self.expand_memory(channel as usize);
        self.mod_memory[channel as usize] = depth;
    }
}

/// A Patch is a configuration of synthesis parameters.
#[derive(Clone, Serialize, Deserialize)]
pub struct Patch {
    pub name: String,
    pub gain: Parameter,
    pub pan: Parameter,
    pub glide_time: f32,
    pub play_mode: PlayMode,
    pub filters: Vec<Filter>,
    pub oscs: Vec<Oscillator>,
    pub envs: Vec<ADSR>,
    pub lfos: Vec<LFO>,
    pub mod_matrix: Vec<Modulation>,
    pub fx_send: Parameter,
    pub distortion: Parameter,
    #[serde(default)]
    pub version: u8,
}

impl Patch {
    /// Current save version.
    const VERSION: u8 = 1;

    pub fn new(name: String) -> Self {
        Self {
            name,
            gain: Parameter(shared(0.5)),
            fx_send: Parameter(shared(1.0)),
            distortion: Parameter(shared(0.0)),
            oscs: vec![Oscillator::default()],
            envs: vec![ADSR::default()],
            filters: Vec::new(),
            lfos: Vec::new(),
            play_mode: PlayMode::Poly,
            glide_time: 0.0,
            pan: Parameter(shared(0.0)),
            mod_matrix: vec![
                Modulation {
                    source: ModSource::Envelope(0),
                    target: ModTarget::Gain,
                    depth: Parameter(shared(1.0)),
                },
                Modulation {
                    source: ModSource::Pressure,
                    target: ModTarget::Gain,
                    depth: Parameter(shared(1.0)),
                },
            ],
            version: Self::VERSION,
        }
    }

    /// Initialize a loaded patch.
    pub fn init(&mut self) {
        // initialize PCM generators
        for osc in self.oscs.iter_mut() {
            if let Waveform::Pcm(Some(data)) = &mut osc.waveform {
                if let Err(e) = data.init() {
                    eprintln!("{}", e);
                }
            }
        }

        if self.version < 1 {
            // convert generator levels
            for osc in self.oscs.iter_mut() {
                osc.level.0.set(osc.level.0.value().powi(2));
            }
        }

        self.version = Self::VERSION;
    }

    /// Load a patch from disk.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let input = fs::read(path)?;
        let mut patch = rmp_serde::from_slice::<Self>(&input)?;
        patch.init();
        Ok(patch)
    }

    /// Save the patch to disk.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let contents = rmp_serde::to_vec(self)?;
        Ok(fs::write(path, contents)?)
    }

    /// Create a copy of the patch. Copies share access to wave data.
    pub fn duplicate(&self) -> Self {
        let mut patch = self.clone();

        if !patch.name.starts_with("Copy of") {
            patch.name = format!("Copy of {}", patch.name);
            patch.name.truncate(MAX_PATCH_NAME_CHARS);
        }

        patch
    }

    /// Returns the DSP net for a modulation, given voice parameters.
    fn mod_net(&self, vars: &VoiceVars, target: ModTarget, path: &[ModSource]) -> Net {
        let mut net = Net::wrap(Box::new(
            constant(if target.is_additive() { 0.0 } else { 1.0 })));

        for (i, m) in self.mod_matrix.iter().enumerate() {
            if m.target == target && !path.contains(&m.source) {
                if target.is_additive() {
                    net = net + m.make_net(self, vars, i, path);
                } else {
                    net = net * m.make_net(self, vars, i, path);
                }
            }
        }

        net
    }

    /// Returns valid modulation sources for the patch.
    pub fn mod_sources(&self) -> Vec<ModSource> {
        let mut v = vec![
            ModSource::Pitch,
            ModSource::Pressure,
            ModSource::Modulation,
            ModSource::Random
        ];

        v.extend((0..self.envs.len()).map(|i| ModSource::Envelope(i)));
        v.extend((0..self.lfos.len()).map(|i| ModSource::LFO(i)));

        v
    }

    /// Returns valid modulation destinations for the patch.
    pub fn mod_targets(&self) -> Vec<ModTarget> {
        let mut v = vec![
            ModTarget::Gain,
            ModTarget::Pan,
            ModTarget::Pitch,
            ModTarget::FinePitch,
            ModTarget::ClipGain,
        ];

        for (i, osc) in self.oscs.iter().enumerate() {
            v.push(ModTarget::Level(i));
            v.push(ModTarget::OscPitch(i));
            v.push(ModTarget::OscFinePitch(i));
            if osc.waveform.has_tone_control() {
                v.push(ModTarget::Tone(i));
            }
        }

        for i in 0..self.filters.len() {
            v.push(ModTarget::FilterCutoff(i));
            v.push(ModTarget::FilterQ(i));
        }

        for i in 0..self.envs.len() {
            v.push(ModTarget::EnvScale(i));
        }

        for (i, lfo) in self.lfos.iter().enumerate() {
            if lfo.waveform.uses_freq() {
                v.push(ModTarget::LFORate(i));
            }
        }

        for i in 0..self.mod_matrix.len() {
            v.push(ModTarget::ModDepth(i));
        }

        v
    }

    /// Remove a generator, updating other settings as needed.
    pub fn remove_osc(&mut self, i: usize) {
        if i >= self.oscs.len() {
            return
        }

        self.oscs.remove(i);

        // update outputs
        for (j, osc) in self.oscs.iter_mut().enumerate() {
            if j == 0 {
                // first osc always has normal output
                osc.output = OscOutput::Mix(0);
            } else {
                match &mut osc.output {
                    OscOutput::Mix(n) | OscOutput::AM(n)
                        | OscOutput::RM(n) | OscOutput::FM(n) if *n == i =>
                        osc.output = OscOutput::Mix(0),
                    OscOutput::Mix(n) | OscOutput::AM(n)
                        | OscOutput::RM(n) | OscOutput::FM(n) if *n > i => *n -= 1,
                    _ => (),
                }
            }
        }

        // update mod matrix

        self.mod_matrix.retain(|m| m.target.osc() != Some(i));

        for m in self.mod_matrix.iter_mut() {
            if let Some(n) = m.target.osc_mut() {
                if *n > i {
                    *n -= 1;
                }
            }
        }
    }

    /// Remove a filter, updating other settings as needed.
    pub fn remove_filter(&mut self, i: usize) {
        if i >= self.filters.len() {
            return
        }

        self.filters.remove(i);
        self.mod_matrix.retain(|m| m.target.filter() != Some(i));

        for m in self.mod_matrix.iter_mut() {
            if let Some(n) = m.target.filter_mut() {
                if *n > i {
                    *n -= 1;
                }
            }
        }
    }

    /// Remove an envelope, updating other settings as needed.
    pub fn remove_env(&mut self, i: usize) {
        if i < self.envs.len() {
            self.envs.remove(i);
            self.mod_matrix.retain(|m| m.source != ModSource::Envelope(i));

            for m in self.mod_matrix.iter_mut() {
                if let ModSource::Envelope(n) = &mut m.source {
                    if *n > i {
                        *n -= 1;
                    }
                }
            }
        }
    }

    /// Remove an LFO, updating other settings as needed.
    pub fn remove_lfo(&mut self, i: usize) {
        if i < self.lfos.len() {
            self.lfos.remove(i);
            self.mod_matrix.retain(|m| m.source != ModSource::LFO(i)
                && m.target != ModTarget::LFORate(i));

            for m in self.mod_matrix.iter_mut() {
                if let ModSource::LFO(n) = &mut m.source {
                    if *n > i {
                        *n -= 1;
                    }
                }
                if let ModTarget::LFORate(n) = &mut m.target {
                    if *n > i {
                        *n -= 1;
                    }
                }
            }
        }
    }

    /// Remove a mod matrix entry, updating other settings as needed.
    pub fn remove_mod(&mut self, i: usize) {
        if i < self.mod_matrix.len() {
            self.mod_matrix.remove(i);
            self.mod_matrix.retain(|m| m.target != ModTarget::ModDepth(i));

            for m in self.mod_matrix.iter_mut() {
                if let ModTarget::ModDepth(n) = &mut m.target {
                    if *n > i {
                        *n -= 1;
                    }
                }
            }
        }
    }

    /// Construct a DSP net for generator `i`.
    fn make_osc(&self, i: usize, vars: &VoiceVars) -> Net {
        let mut freq_mod = Net::new(0, 1);

        for (j, osc) in self.oscs.iter().enumerate() {
            if j > i && osc.output == OscOutput::FM(i) {
                freq_mod = freq_mod + self.make_osc(j, vars);
            }
        }

        let level = {
            let modu = self.mod_net(vars, ModTarget::Level(i), &[]);
            (var(&self.oscs[i].level.0) >> smooth()) * (modu >> shape_fn(|x| x*x))
        };
        let mut net = self.oscs[i].make_net(self, vars, i, freq_mod) * level;

        // need to iterate multiple times because order of operations matters

        for (j, osc) in self.oscs.iter().enumerate() {
            if j > i {
                if osc.output == OscOutput::AM(i) {
                    net = net * (1.0 + self.make_osc(j, vars));
                } else if osc.output == OscOutput::RM(i) {
                    net = net * self.make_osc(j, vars);
                }
            }
        }

        for (j, osc) in self.oscs.iter().enumerate() {
            if j > i && osc.output == OscOutput::Mix(i) {
                net = net + self.make_osc(j, vars);
            }
        }

        net
    }

    /// Filter a net through the patch filter chain.
    fn filter(&self, vars: &VoiceVars, net: Net) -> Net {
        let mut net = net;
        for (i, filter) in self.filters.iter().enumerate() {
            net = filter.filter(self, vars, i, net);
        }
        net
    }

    /// Returns true unless gain is modulated by an envelope with zero sustain,
    /// or all mixed generators are one-shot PCM.
    pub fn sustains(&self) -> bool {
        for m in &self.mod_matrix {
            if m.target == ModTarget::Gain {
                if let ModSource::Envelope(i) = m.source {
                    if self.envs.get(i).is_some_and(|env| env.sustain == 0.0) {
                        return false
                    }
                }
            }
        }

        !self.oscs.iter()
            .filter(|g| g.output == OscOutput::Mix(0))
            .all(|g| match &g.waveform {
                Waveform::Pcm(data) => data.as_ref()
                    .is_none_or(|data| data.loop_point.is_none()),
                _ => false,
            })
    }

    /// Returns the maximum amount of time that it could take for this patch
    /// to release.
    fn release_time(&self) -> f32 {
        self.envs.iter().enumerate()
            .map(|(i, env)| env.release * self.env_scale_factor(i))
            .fold(0.0, f32::max)
    }

    /// Returns a longest-case estimate of envelope scale factor.
    fn env_scale_factor(&self, i: usize) -> f32 {
        // TODO: this doesn't account for depth modulation
        pow(MAX_ENV_SCALE, self.mod_matrix.iter()
            .filter(|m| m.target == ModTarget::EnvScale(i))
            .map(|m| m.depth.0.value().max(0.0))
            .sum())
    }
}

/// Tone generator.
#[derive(Clone, Serialize, Deserialize)]
pub struct Oscillator {
    pub level: Parameter,
    pub tone: Parameter,
    pub freq_ratio: Parameter,
    pub fine_pitch: Parameter,
    pub waveform: Waveform,
    pub output: OscOutput,
    #[serde(default)]
    pub oversample: bool,
}

impl Default for Oscillator {
    fn default() -> Self {
        Self {
            level: Parameter(shared(1.0)),
            tone: Parameter(shared(0.5)),
            freq_ratio: Parameter(shared(1.0)),
            fine_pitch: Parameter(shared(0.0)),
            waveform: Waveform::Sine,
            output: OscOutput::Mix(0),
            oversample: false,
        }
    }
}

impl Oscillator {
    /// Make a generator DSP net.
    fn make_net(&self, settings: &Patch, vars: &VoiceVars, index: usize, freq_mod: Net
    ) -> Net {
        // TODO: glide can be skipped if glide time is zero
        let glide = {
            let prev_freq = vars.prev_freq.unwrap_or(vars.freq.value());
            let env = envelope2(move |t, x| if t == 0.0 { prev_freq } else { x });
            env >> follow(settings.glide_time * 0.5)
        };
        let base_freq = (var(&vars.freq) >> glide)
            * var(&self.freq_ratio.0)
            * (settings.mod_net(vars, ModTarget::OscPitch(index), &[])
                + settings.mod_net(vars, ModTarget::Pitch, &[])
                >> pow_shape(MAX_PITCH_MOD))
            * ((settings.mod_net(vars, ModTarget::OscFinePitch(index), &[])
                + settings.mod_net(vars, ModTarget::FinePitch, &[]))
                * 0.5 + var(&self.fine_pitch.0) >> pow_shape(SEMITONE_RATIO))
            * (1.0 + freq_mod * FM_DEPTH_MULTIPLIER);
        let tone = var(&self.tone.0)
            + settings.mod_net(vars, ModTarget::Tone(index), &[])
            >> shape_fn(clamp01);

        match &self.waveform {
            Waveform::Sawtooth => if self.oversample {
                base_freq >> oversample(saw().phase(0.0))
            } else {
                base_freq >> saw().phase(0.0)
            },
            Waveform::Pulse => if self.oversample {
                (base_freq | tone) >> oversample(pulse().phase(0.0))
            } else {
                (base_freq | tone) >> pulse().phase(0.0)
            },
            Waveform::Triangle => if self.oversample {
                base_freq >> oversample(triangle().phase(0.0))
            } else {
                base_freq >> triangle().phase(0.0)
            },
            Waveform::Sine => if self.oversample {
                base_freq >> oversample(sine().phase(0.0))
            } else {
                base_freq >> sine().phase(0.0)
            },
            Waveform::Hold => (noise().seed(random()) | base_freq) >> hold(0.0),
            Waveform::Noise => (noise().seed(random()) | tone)
                >> (pinkpass() * (1.0 - pass()) & pass() * pass()),
            Waveform::Pcm(data) => if let Some(data) = data {
                let f = data.wave.sample_rate() as f32 / vars.sample_rate / REF_FREQ;
                base_freq * f >>
                    resample(wavech(&data.wave, 0, data.loop_point))
            } else {
                Net::new(0, 1)
            },
        }
    }
}

/// Destination for generator signals.
#[derive(PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum OscOutput {
    Mix(usize),
    AM(usize),
    RM(usize),
    FM(usize),
}

impl OscOutput {
    /// Returns valid choices for a generator at `index`.
    pub fn choices(index: usize) -> Vec<OscOutput> {
        if index == 0 {
            vec![OscOutput::Mix(0)]
        } else {
            (0..index).flat_map(|i| if i + 1 == index {
                // only allow modulating the previous oscillator in the list
                vec![OscOutput::Mix(i), OscOutput::AM(i),
                    OscOutput::RM(i), OscOutput::FM(i)]
            } else {
                vec![OscOutput::Mix(i)]
            }).collect()
        }
    }
}

impl Display for OscOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Mix(0) => "Mix",
            Self::Mix(i) => &format!("Mix to gen {}", i + 1),
            Self::AM(i) => &format!("AM to gen {}", i + 1),
            Self::RM(i) => &format!("RM to gen {}", i + 1),
            Self::FM(i) => &format!("FM to gen {}", i + 1),
        };
        f.write_str(s)
    }
}

/// Key tracking options for filter cutoff.
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

#[derive(Clone, Serialize, Deserialize)]
pub struct Filter {
    pub filter_type: FilterType,
    pub cutoff: Parameter,
    pub resonance: Parameter,
    pub key_tracking: KeyTracking,
}

impl Filter {
    /// Filter DSP net.
    fn filter(&self, settings: &Patch, vars: &VoiceVars, index: usize, net: Net) -> Net {
        let cutoff = {
            let kt_freq = Net::wrap(match self.key_tracking {
                KeyTracking::None => Box::new(var(&self.cutoff.0)),
                KeyTracking::Partial => Box::new(var(&self.cutoff.0)
                    * var_fn(&vars.freq, |x| pow(x/REF_FREQ, 0.5))),
                KeyTracking::Full => Box::new(var(&self.cutoff.0)
                    * var_fn(&vars.freq, |x| x/REF_FREQ)),
            });
            let modu = settings.mod_net(vars, ModTarget::FilterCutoff(index), &[])
                >> pow_shape(FILTER_CUTOFF_MOD_BASE);
            kt_freq * modu
                >> shape_fn(|x| clamp(MIN_FILTER_CUTOFF, MAX_FILTER_CUTOFF, x))
        };
        let reso = var(&self.resonance.0)
            + settings.mod_net(vars, ModTarget::FilterQ(index), &[]);
        let filter = Net::wrap(match self.filter_type {
            FilterType::Ladder => Box::new(moog()),
            FilterType::Lowpass => Box::new(lowpass()),
            FilterType::Highpass => Box::new(highpass()),
            FilterType::Bandpass => Box::new(bandpass()),
            FilterType::Notch => Box::new(notch()),
        });
        (net | cutoff | reso) >> filter
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            cutoff: Parameter(shared(MAX_FILTER_CUTOFF)),
            resonance: Parameter(shared(MIN_FILTER_RESONANCE)),
            key_tracking: KeyTracking::None,
            filter_type: FilterType::Ladder,
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

    /// Returns the UI string for the filter type.
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

/// ADSR envelope.
#[derive(Clone, Serialize, Deserialize)]
pub struct ADSR {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,

    #[serde(rename = "power")]
    _power: f32 // legacy
}

impl ADSR {
    fn make_net(&self, settings: &Patch, vars: &VoiceVars, index: usize,
        path: &[ModSource], sqrt_attack: bool,
    ) -> Net {
        let scale = settings.mod_net(vars, ModTarget::EnvScale(index), path)
            >> pow_shape(1.0/MAX_ENV_SCALE);
        let adsr = adsr_scalable(self.attack, self.decay, self.sustain, self.release,
            sqrt_attack);

        (var(&vars.gate) | scale) >> adsr
    }
}

impl Default for ADSR {
    fn default() -> Self {
        Self {
            attack: 0.0,
            decay: 1.0,
            sustain: 1.0,
            release: 0.01,
            _power: 0.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LFO {
    pub waveform: Waveform,
    pub freq: Parameter,
    pub delay: f32,
}

impl Default for LFO {
    fn default() -> Self {
        Self {
            waveform: Waveform::Triangle,
            freq: Parameter(shared(1.0)),
            delay: 0.0,
        }
    }
}

impl LFO {
    /// Make an LFO DSP net.
    fn make_net(&self,
        settings: &Patch, vars: &VoiceVars, index: usize, path: &[ModSource]
    ) -> Net {
        let f = {
            let f_mod = settings.mod_net(vars, ModTarget::LFORate(index), path)
                >> pow_shape(MAX_LFO_RATE/MIN_LFO_RATE);
            var(&self.freq.0) * f_mod
                >> shape_fn(|x| clamp(MIN_LFO_RATE, MAX_LFO_RATE, x))
        };
        let d = {
            let dt = self.delay;
            envelope(move |t| clamp01(pow(t / dt, LFO_DELAY_CURVE)))
        };
        let p = vars.lfo_phases[index];

        match &self.waveform {
            Waveform::Sawtooth => f >> saw_lfo(p) * d >> smooth(),
            Waveform::Pulse => f >> sqr_lfo(p) * d >> smooth(),
            Waveform::Triangle => f >> tri_lfo(p) * d,
            Waveform::Sine => f >> sin_lfo(p) * d,
            Waveform::Hold => f >> hold_lfo(p) * d >> smooth(),
            Waveform::Noise => Net::wrap(Box::new(
                brown().seed((p * u64::MAX as f32) as u64) * d)),
            Waveform::Pcm(data) => Net::wrap(if let Some(data) = data {
                Box::new(wavech(&data.wave, 0, data.loop_point))
            } else {
                Box::new(zero())
            }),
        }
    }
}

/// Mod matrix entry.
#[derive(Clone, Serialize, Deserialize)]
pub struct Modulation {
    pub source: ModSource,
    pub target: ModTarget,
    pub depth: Parameter,
}

impl Default for Modulation {
    fn default() -> Self {
        Self {
            source: ModSource::Modulation,
            target: ModTarget::Gain,
            depth: Parameter(shared(0.0)),
        }
    }
}

impl Modulation {
    fn make_net(&self, settings: &Patch, vars: &VoiceVars, index: usize,
        path: &[ModSource]
    ) -> Net {
        let mut path = path.to_vec();
        path.push(self.source);

        let net = match self.source {
            ModSource::Pitch => Net::wrap(Box::new(
                var_fn(&vars.freq,|f| dexerp(PITCH_FLOOR, PITCH_CEILING, f)))),
            ModSource::Pressure => Net::wrap(Box::new(var(&vars.pressure) >> smooth())),
            ModSource::Modulation =>
                Net::wrap(Box::new(var(&vars.modulation) >> smooth())),
            ModSource::Random => Net::wrap(Box::new(constant(vars.random_values[index]))),
            ModSource::Envelope(i) => match settings.envs.get(i) {
                Some(env) => env.make_net(
                    settings, vars, i, &path, self.target.uses_sqrt_attack()),
                None => Net::new(0, 1),
            },
            ModSource::LFO(i) => match settings.lfos.get(i) {
                Some(lfo) => lfo.make_net(settings, vars, i, &path),
                None => Net::new(0, 1),
            }
        };
        let depth = var(&self.depth.0) >> smooth()
            + settings.mod_net(vars, ModTarget::ModDepth(index), &path);

        if self.target.is_additive() {
            // zero depth = +0 for additive targets
            net * depth
        } else if self.source.is_bipolar() {
            // a bipolar source oscillates in [-1, 1] -- map that onto [0, 1]
            1.0 - (depth * (1.0 - 0.5 * (net + 1.0)) >> shape_fn(abs))
        } else if self.depth.0.value() >= 0.0 {
            1.0 - depth * (1.0 - net)
        } else {
            1.0 + depth * net
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
            Self::Modulation => "Modulation",
            Self::Random => "Random",
            Self::Envelope(i) => &format!("Envelope {}", i + 1),
            Self::LFO(i) => &format!("LFO {}", i + 1),
        };
        f.write_str(s)
    }
}

impl ModSource {
    /// Returns true if the source oscillates in -1..1 rather than 0..1.
    fn is_bipolar(&self) -> bool {
        matches!(*self, ModSource::LFO(_))
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
    EnvScale(usize),
    LFORate(usize),
    ModDepth(usize),
    /// Distortion. Inaccurate name for legacy reasons.
    ClipGain,
}

impl ModTarget {
    /// Returns true if modulations should be summed rather than multiplied.
    pub fn is_additive(&self) -> bool {
        !matches!(*self, Self::Gain | Self::Level(_))
    }

    /// Returns the generator index, if any.
    fn osc(&self) -> Option<usize> {
        match *self {
            Self::Level(n) | Self::OscPitch(n) |
                Self::OscFinePitch(n) | Self::Tone(n) => Some(n),
            _ => None,
        }
    }

    /// Returns the generator index, if any.
    fn osc_mut(&mut self) -> Option<&mut usize> {
        match self {
            Self::Level(n) | Self::OscPitch(n) |
                Self::OscFinePitch(n) | Self::Tone(n) => Some(n),
            _ => None,
        }
    }

    /// Returns the filter index, if any.
    fn filter(&self) -> Option<usize> {
        match *self {
            Self::FilterCutoff(i) | Self::FilterQ(i) => Some(i),
            _ => None,
        }
    }

    /// Returns the filter index, if any.
    fn filter_mut(&mut self) -> Option<&mut usize> {
        match self {
            Self::FilterCutoff(i) | Self::FilterQ(i) => Some(i),
            _ => None,
        }
    }

    /// Returns true if the attack level should be sqrt'ed. Since gain values
    /// are squared, this compensates and gives a linear attack.
    fn uses_sqrt_attack(&self) -> bool {
        matches!(*self, ModTarget::Gain | ModTarget::Level(_))
    }
}

impl Display for ModTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Gain => "Level",
            Self::Pan => "Pan",
            Self::Pitch => "Pitch",
            Self::FinePitch => "Finetune",
            Self::Level(n) => &format!("Gen {} level", n + 1),
            Self::OscPitch(n) => &format!("Gen {} pitch", n + 1),
            Self::OscFinePitch(n) => &format!("Gen {} finetune", n + 1),
            Self::Tone(n) => &format!("Gen {} tone", n + 1),
            Self::FilterCutoff(n) => &format!("Filter {} freq", n + 1),
            Self::FilterQ(n) => &format!("Filter {} reso", n + 1),
            Self::EnvScale(n) => &format!("Env {} scale", n + 1),
            Self::LFORate(n) => &format!("LFO {} freq", n + 1),
            Self::ModDepth(n) => &format!("Mod {} depth", n + 1),
            Self::ClipGain => "Distortion",
        };
        f.write_str(s)
    }
}

struct Voice {
    vars: VoiceVars,
    /// MIDI pitch before MIDI pitch bend.
    base_pitch: f32,
    /// Estimated length of release before deallocation.
    release_time: f32,
    event_id: EventId,
}

impl Voice {
    /// Create and play a new voice.
    fn new(pitch: f32, bend: f32, pressure: f32, modulation: f32, prev_freq: Option<f32>,
        settings: &Patch, seq: &mut Sequencer, rate: f32, pan_polarity: &Shared,
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
            sample_rate: rate,
        };
        let gain = (var(&settings.gain.0) >> smooth())
            * (settings.mod_net(&vars, ModTarget::Gain, &[]) >> shape_fn(|x| x*x));

        // use dry signal when distortion is zero
        let clip = (
            var(&settings.distortion.0)
                + settings.mod_net(&vars, ModTarget::ClipGain, &[])
            | pass()
        ) >> map(|i: &Frame<f32, U2>| if i[0] == 0.0 {
            i[1]
        } else {
            clamp11(i[1] * (1.0 - clamp01(i[0])).recip())
        });

        let signal = (settings.filter(&vars, settings.make_osc(0, &vars)) >> clip) * gain;
        let pan = (var(&settings.pan.0) >> smooth()
            + settings.mod_net(&vars, ModTarget::Pan, &[]) * 2.0)
            * var(pan_polarity) >> shape_fn(clamp11);

        let net = (signal | pan) >> panner()
            >> multisplit::<U2, U2>()
            >> (multipass::<U2>()
                | multipass::<U2>() * (var(&settings.fx_send.0) >> split::<U2>()));

        Self {
            vars,
            base_pitch: pitch,
            release_time: settings.release_time(),
            event_id: seq.push_relative(
                0.0, f64::INFINITY, Fade::Smooth, 0.0, 0.0, Box::new(net)),
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

/// State of a playing voice.
struct VoiceVars {
    freq: Shared,
    pressure: Shared,
    modulation: Shared,
    /// Triggers envelope release when zero.
    gate: Shared,
    /// Used by the "Random" modulation source.
    random_values: Vec<f32>,
    /// Used to synchronize multiple DSP instances of the same logical LFO.
    lfo_phases: Vec<f32>,
    /// Initial frequency to glide from.
    prev_freq: Option<f32>,
    sample_rate: f32,
}