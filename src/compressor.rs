use std::marker::PhantomData;

use fundsp::{hacker::{AFollow, An, AudioNode, U2}, math::{abs, amp_db, db_amp, max}, signal::SignalFrame, Frame, Size, DEFAULT_SR};

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
    pub fn new(sample_rate: f64, threshold: f32, slope: f32, attack: f32, release: f32
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