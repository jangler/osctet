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
    let neg1 = -1.0;
    let zero = 0.0;
    let a = shared(zero);
    let b = shared(neg1);
    let c = shared(zero);
    let d = shared(zero);
    let attack_start = var(&a);
    let release_start = var(&b);
    let prev_time = var(&c);
    let scaled_time = var(&d);
    envelope3(move |time, control, speed| {
        scaled_time.set_value(scaled_time.value() + speed * (time - prev_time.value()));
        prev_time.set_value(time);
        let time = scaled_time.value();

        if release_start.value() >= zero && control > zero {
            attack_start.set_value(time);
            release_start.set_value(neg1);
        } else if release_start.value() < zero && control <= zero {
            release_start.set_value(time);
        }
        let ads_value =
            ads(attack, decay, sustain, time - attack_start.value(), sqrt_attack);
        if release_start.value() < zero {
            ads_value
        } else {
            ads_value
                * clamp01(delerp(
                    release_start.value() + release,
                    release_start.value(),
                    time,
                ))
        }
    })
}

fn ads<F: Real>(attack: F, decay: F, sustain: F, time: F, sqrt_attack: bool) -> F {
    if time < attack {
        let level = lerp(F::from_f64(0.0), F::from_f64(1.0), time / attack);
        if sqrt_attack {
            level.sqrt()
        } else {
            level
        }
    } else {
        let decay_time = time - attack;
        if decay_time < decay {
            lerp(F::from_f64(1.0), sustain, decay_time / decay)
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

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        for i in 0..full_simd_items(size) {
            output.set(0, i, F32x::new(core::array::from_fn(|i| {
                self.shape(input.at(0, i).as_array_ref()[i])
            })))
        }
        self.process_remainder(size, input, output);
    }

    fn route(&mut self, input: &SignalFrame, _frequency: f64) -> SignalFrame {
        let mut output = SignalFrame::new(self.outputs());
        output.set(0, input.at(0).distort(0.0));
        output
    }
}