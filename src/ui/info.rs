pub enum Info {
    None,
    OctaveRatio,
    OctaveSteps,
    ArrowSteps,
    Division,
    Octave,
    Predelay,
    DelayTime,
    DelayFeedback,
    CompGain,
    CompThreshold,
    CompRatio,
    CompAttack,
    CompRelease,
    StereoWidth,
    Gamma,
    Chroma,
    GlideTime,
    Distortion,
    FxSend,
    LoopPoint,
    Tone,
    FreqRatio,
    FilterCutoff,
    FilterResonance,
    Attack,
    Decay,
    Sustain,
    Release,
    LfoDelay,
    ModDepth,
}

impl Info {
    pub fn text(&self) -> &'static str {
        // keep max line width around 50 chars
        match self {
            Self::None => "",
            Self::OctaveRatio =>
"Size of the octave, as a frequency multiplier.
Can be used to slightly stretch the octave, or to
use a different interval as the scale period.",
            Self::OctaveSteps => "Number of steps to divide the octave into.",
            Self::ArrowSteps =>
"Number of steps notated by an up/down accidental.
By default an arrow means one step, but in large
tunings it may be useful to notate multiple steps
with one arrow.",
            Self::Division => "Current number of rows per beat.",
            Self::Octave => "Current octave for note input.",
            Self::Predelay =>
"Delay before the dry signal reaches the reverb.
Can be used to avoid mudding transients, or to
create a sense of spatial proximity.",
            Self::DelayTime => "Time between echoes.",
            Self::DelayFeedback =>
"Amount of self-feedback. Larger values create more
persistent echoes.",
            Self::CompGain => "Pre-compression gain.",
            Self::CompThreshold => "Amplitude threshold where compression starts.",
            Self::CompRatio =>
"Ratio of input dB above threshold to output dB
above threshold.",
            Self::CompAttack =>
"Approximate time the compressor takes to engage
when the input level rises.",
            Self::CompRelease =>
"Approximate time the compressor takes to disengage
when the input level falls.",
            Self::StereoWidth =>
"Multiplier to instrument pan values. Can be used
to check the mono mix, or to reverse panning. Does
not affect render output.",
            Self::Gamma =>
"Gamma correction. Applies a brightness curve to
make value differences look approximately uniform.",
            Self::Chroma =>
"Colorfulness. Different hues reach full saturation
at different points in the 130-180 range.",
            Self::GlideTime =>
"Approximate time the patch takes to glide to new
pitches.",
            Self::Distortion => "Portion of the signal to be hard clipped.",
            Self::FxSend => "Amount of signal to send to the spatial FX bus.",
            Self::LoopPoint =>
"Position where loop begins. Snaps to values with
smaller discontinuities. Loop end point is always
the end of the sample.",
            Self::Tone =>
"For pulse waves, sets the duty cycle. For noise,
mixes between pink and white noise.",
            Self::FreqRatio =>
"Frequency ratio of this generator compared to the
base frequency of the note. Integer values give
harmonic results when mixing or modulating multiple
generators.",
            Self::FilterCutoff =>
"Approximate frequency where the filter starts
attenuating input. Also the resonant.",
            Self::FilterResonance =>
"How much to emphasize frequencies near the cutoff
frequency.",
            Self::Attack => "Time to reach initial peak level.",
            Self::Decay => "Time to transition between peak and sustain levels.",
            Self::Sustain => "Minimum level to hold while note is on.",
            Self::Release =>
"Time to transition between sustain level and zero
after note is released.",
            Self::LfoDelay => "Time LFO takes to reach full amplitude.",
            Self::ModDepth =>
"Amount of modulation. Scale varies depending on
the destination.",
        }
    }
}