use std::f32::consts::PI;

use fundsp::hacker32::*;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};

use super::{pow_shape, smooth, ModSource, ModTarget, Parameter, Patch, VoiceVars, Waveform};

// (Hz)
pub const MIN_LFO_RATE: f32 = 0.1;
pub const MAX_LFO_RATE: f32 = 20.0;
pub const AR_RATE_MULTIPLIER: f32 = MAX_LFO_RATE/MIN_LFO_RATE;

/// Use a cubic attack envelope for LFO delay.
const LFO_DELAY_CURVE: f32 = 3.0;

#[derive(Clone, Serialize, Deserialize)]
pub struct LFO {
    pub waveform: Waveform,
    pub freq: Parameter,
    pub delay: f32,
    #[serde(default)]
    pub audio_rate: bool,
}

impl Default for LFO {
    fn default() -> Self {
        Self {
            waveform: Waveform::Triangle,
            freq: Parameter(shared(1.0)),
            delay: 0.0,
            audio_rate: false,
        }
    }
}

impl LFO {
    /// Make an LFO DSP net.
    pub(super) fn make_net(&self,
        settings: &Patch, vars: &VoiceVars, index: usize, path: &[ModSource]
    ) -> Net {
        let f = {
            let f_mod = settings.mod_net(vars, ModTarget::LFORate(index), path)
                >> pow_shape(MAX_LFO_RATE/MIN_LFO_RATE);
            let f = var(&self.freq.0) * f_mod
                >> shape_fn(|x| clamp(MIN_LFO_RATE, MAX_LFO_RATE, x));
            if self.audio_rate {
                f * AR_RATE_MULTIPLIER
            } else {
                f
            }
        };
        let d = {
            let dt = self.delay;
            envelope(move |t| clamp01(pow(t / dt, LFO_DELAY_CURVE)))
        };
        let p = vars.lfo_phases[index];

        match &self.waveform {
            Waveform::Sawtooth => if self.audio_rate {
                f >> saw().phase(p) * d
            } else {
                f >> saw_lfo(p) * d >> smooth()
            },
            Waveform::Pulse => if self.audio_rate {
                f >> square().phase(p) * d
            } else {
                f >> sqr_lfo(p) * d >> smooth()
            },
            Waveform::Triangle => if self.audio_rate {
                f >> triangle().phase(p) * d
            } else {
                f >> tri_lfo(p) * d
            },
            Waveform::Sine => if self.audio_rate {
                f >> sine().phase(p) * d
            } else {
                f >> sin_lfo(p) * d
            },
            Waveform::Hold => if self.audio_rate {
                (noise().seed((p * u64::MAX as f32) as u64) | f) >> hold(0.0) * d
            } else {
                f >> hold_lfo(p) * d >> smooth()
            },
            Waveform::Noise => Net::wrap(Box::new(
                brown().seed((p * u64::MAX as f32) as u64) * d)),
            Waveform::Pcm(data) => Net::wrap(if let Some(data) = data {
                Box::new(wavech(&data.wave, 0, data.loop_point))
            } else {
                Box::new(zero())
            }),
        }
    }
    
    pub(crate) fn shared_clone(&self) -> Self {
        Self {
            waveform: self.waveform.clone(),
            freq: self.freq.shared_clone(),
            delay: self.delay,
            audio_rate: self.audio_rate,
        }
    }
}

// not sure how to de-duplicate these, since closures aren't Clone

/// Ramp LFO in -1..1. Takes frequency as an input.
fn saw_lfo(phase: f32) -> An<EnvelopeIn<f32, impl FnMut(f32, &Frame<f32, U1>) -> f32 + Clone, U1, f32>> {
    let phase = var(&shared(phase));
    let prev_time = var(&shared(0.0));
    lfo_in(move |t, i| {
        let dt = t - prev_time.value();
        prev_time.set_value(t);
        let p = (phase.value() + dt * i[0]) % 1.0;
        phase.set_value(p);
        p * 2.0 - 1.0
    })
}

/// Square LFO in -1..1. Takes frequency as an input.
fn sqr_lfo(phase: f32) -> An<EnvelopeIn<f32, impl FnMut(f32, &Frame<f32, U1>) -> f32 + Clone, U1, f32>> {
    let phase = var(&shared(phase));
    let prev_time = var(&shared(0.0));
    lfo_in(move |t, i| {
        let dt = t - prev_time.value();
        prev_time.set_value(t);
        let p = (phase.value() + dt * i[0]) % 1.0;
        phase.set_value(p);
        if p < 0.5 {
            1.0
        } else {
            -1.0
        }
    })
}

/// Triangle LFO in -1..1. Takes frequency as an input.
fn tri_lfo(phase: f32) -> An<EnvelopeIn<f32, impl FnMut(f32, &Frame<f32, U1>) -> f32 + Clone, U1, f32>> {
    let phase = var(&shared(phase));
    let prev_time = var(&shared(0.0));
    lfo_in(move |t, i| {
        let dt = t - prev_time.value();
        prev_time.set_value(t);
        let p = (phase.value() + dt * i[0]) % 1.0;
        phase.set_value(p);
        if p < 0.25 {
            p * 4.0
        } else if p < 0.75 {
            1.0 - (p - 0.25) * 4.0
        } else {
            (p - 0.75) * 4.0 - 1.0
        }
    })
}

/// Sine LFO in -1..1. Takes frequency as an input.
fn sin_lfo(phase: f32) -> An<EnvelopeIn<f32, impl FnMut(f32, &Frame<f32, U1>) -> f32 + Clone, U1, f32>> {
    let phase = var(&shared(phase));
    let prev_time = var(&shared(0.0));
    lfo_in(move |t, i| {
        let dt = t - prev_time.value();
        prev_time.set_value(t);
        let p = (phase.value() + dt * i[0]) % 1.0;
        phase.set_value(p);
        (p * PI * 2.0).sin()
    })
}

/// Sample & hold LFO in -1..1. Takes frequency as an input.
fn hold_lfo(phase: f32) -> An<EnvelopeIn<f32, impl FnMut(f32, &Frame<f32, U1>) -> f32 + Clone, U1, f32>> {
    let mut source = StdRng::seed_from_u64((phase as f64 * u64::MAX as f64) as u64);
    let phase = var(&shared(phase));
    let prev_time = var(&shared(0.0));
    let state = var(&shared(
        ((source.next_u32() as f64 / u32::MAX as f64) * 2.0 - 1.0) as f32
    ));
    lfo_in(move |t, i| {
        let dt = t - prev_time.value();
        prev_time.set_value(t);
        let p = (phase.value() + dt * i[0]) % 1.0;
        if p < phase.value() {
            state.set_value(
                ((source.next_u32() as f64 / u32::MAX as f64) * 2.0 - 1.0) as f32
            );
        }
        phase.set_value(p);
        state.value()
    })
}