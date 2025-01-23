//! Custom FunDSP audio nodes.

use std::marker::PhantomData;

use fundsp::prelude::*;

/// Slightly different implementation of adsr_live. Inputs are 1) gate and 2) scale.
pub fn adsr_scalable(
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    sqrt_attack: bool,
) -> An<EnvelopeIn<f32, impl FnMut(f32, &Frame<f32, U2>) -> f32 + Clone, U2, f32>> {
    let attack_start = var(&shared(0.0));
    let release_start = var(&shared(-1.0));
    let prev_time = var(&shared(0.0));
    let scaled_time = var(&shared(0.0));

    envelope3(move |time, control, speed| {
        scaled_time.set_value(scaled_time.value() + speed * (time - prev_time.value()));
        prev_time.set_value(time);
        let time = scaled_time.value();

        if release_start.value() >= 0.0 && control > 0.0 {
            attack_start.set_value(time);
            release_start.set_value(-1.0);
        } else if release_start.value() < 0.0 && control <= 0.0 {
            release_start.set_value(time);
        }

        let ads_value =
            ads(attack, decay, sustain, time - attack_start.value(), sqrt_attack);
        if release_start.value() < 0.0 {
            ads_value
        } else {
            ads_value * clamp01(delerp(release, 0.0, time - release_start.value()))
        }
    })
}

/// ADS envelope. Helper for ADSR.
fn ads(attack: f32, decay: f32, sustain: f32, time: f32, sqrt_attack: bool) -> f32 {
    if time < attack {
        let level = lerp(0.0, 1.0, time / attack);
        if sqrt_attack {
            level.sqrt()
        } else {
            level
        }
    } else {
        let decay_time = time - attack;
        if decay_time < decay {
            lerp(1.0, sustain, decay_time / decay)
        } else {
            sustain
        }
    }
}

/// Stereo compressor. Slope is 0.0..=1.0, equivalent to (ratio - 1) / ratio.
pub fn compressor(threshold: f32, slope: f32, attack: f32, release: f32
) -> An<Compressor<U2>> {
    An(Compressor::new(DEFAULT_SR, threshold, slope, attack, release))
}

#[derive(Clone)]
pub struct Compressor<N>
where
    N: Size<f32>,
{
    _marker: PhantomData<N>,
    sample_rate: f64,
    threshold_db: f32,
    slope: f32,
    follower: AFollow<f32>,
}

impl<N> Compressor<N>
where
    N: Size<f32>,
{
    fn new(sample_rate: f64, threshold: f32, slope: f32, attack: f32, release: f32
    ) -> Self {
        // attack/release scaling copied from fundsp's limiter
        // follower tracks dB of gain reduction
        let mut follower = AFollow::new(attack * 0.4, release * 0.4);
        follower.set_sample_rate(sample_rate);
        follower.set_value(0.0);

        Self {
            _marker: PhantomData,
            sample_rate,
            threshold_db: amp_db(threshold),
            slope,
            follower,
        }
    }
}

impl<N> AudioNode for Compressor<N>
where
    N: Size<f32>,
{
    const ID: u64 = 200;
    type Inputs = N;
    type Outputs = N;

    fn reset(&mut self) {
        self.set_sample_rate(self.sample_rate);
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.sample_rate = sample_rate;
        self.follower.set_sample_rate(sample_rate);
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let amp = input.iter().fold(0.0, |amp, &x| max(amp, abs(x)));
        let resp = self.follower.filter_mono(
            (amp_db(amp) - self.threshold_db).max(0.0) * self.slope);
        input.clone() * Frame::splat(db_amp(-resp))
    }

    fn route(&mut self, input: &SignalFrame, _frequency: f64) -> SignalFrame {
        let mut output = SignalFrame::new(self.outputs());
        for i in 0..N::USIZE {
            output.set(i, input.at(i));
        }
        output
    }

    fn allocate(&mut self) {}
}

/// Optimized waveshaper. Output is `pow(base, input)`.
pub fn pow_shape(base: f32) -> An<PowShaper> {
    An(PowShaper::new(base))
}

#[derive(Clone)]
pub struct PowShaper {
    base: f32,
    cached_in: f32,
    cached_out: f32,
}

impl PowShaper {
    fn new(base: f32) -> Self {
        let mut shaper = Self { base, cached_in: 0.0, cached_out: 1.0 };
        shaper.set_sample_rate(DEFAULT_SR);
        shaper
    }

    fn shape(&mut self, input: f32) -> f32 {
        if input != self.cached_in {
            self.cached_in = input;
            self.cached_out = pow(self.base, input)
        }
        self.cached_out
    }
}

impl AudioNode for PowShaper {
    const ID: u64 = 202;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        self.cached_in = 0.0;
        self.cached_out = 1.0;
    }

    fn set_sample_rate(&mut self, _sample_rate: f64) {}

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        Frame::from([self.shape(input[0])])
    }

    fn route(&mut self, input: &SignalFrame, _frequency: f64) -> SignalFrame {
        let mut output = SignalFrame::new(self.outputs());
        output.set(0, input.at(0).distort(0.0));
        output
    }
}

/// Parameter smoother. Cheaper than `follow()`.
pub fn smooth() -> An<Smooth> {
    An(Smooth::new())
}

#[derive(Clone)]
pub struct Smooth {
    value: Option<f32>,
    prev_coeff: f32,
    next_coeff: f32,
}

impl Smooth {
    /// Halfway response time in seconds.
    const RESPONSE_TIME: f32 = 0.005;

    fn new() -> Self {
        let mut node = Self {
            value: None,
            prev_coeff: 0.0,
            next_coeff: 0.0,
        };
        node.reset();
        node.set_sample_rate(DEFAULT_SR);
        node
    }
}

impl AudioNode for Smooth {
    const ID: u64 = 201;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        self.value = None;
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let response_samples = Self::RESPONSE_TIME * sample_rate as f32;
        self.next_coeff = 0.6912 / response_samples;
        self.prev_coeff = 1.0 - self.next_coeff;
    }

    #[inline]
    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let v = match &mut self.value {
            Some(v) => {
                *v = *v * self.prev_coeff + input[0] * self.next_coeff;
                *v
            }
            None => {
                self.value = Some(input[0]);
                input[0]
            }
        };

        [v].into()
    }

    fn route(&mut self, input: &SignalFrame, _frequency: f64) -> SignalFrame {
        // pretend this doesn't affect response
        let mut output = SignalFrame::new(self.outputs());
        output.set(0, input.at(0));
        output
    }
}