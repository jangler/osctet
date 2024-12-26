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
    LoadScale,
    SavePatch,
    LoadPatch,
    DuplicatePatch,
    LoadSample,
    PrevSample,
    NextSample,
    DetectPitch,
    Add(&'static str),
    Remove(&'static str),
    ResetTheme(&'static str),
    FontSize(&'static str),
    ResetSettings,
}

impl Info {
    pub fn text(&self) -> String {
        // keep max line width around 50 chars
        match self {
            Self::None => String::new(),
            Self::OctaveRatio =>
"Size of the octave, as a frequency multiplier.
Can be used to slightly stretch the octave, or to
use a different interval as the scale period.".to_string(),
            Self::OctaveSteps => "Number of steps to divide the octave into.".to_string(),
            Self::ArrowSteps =>
"Number of steps notated by an up/down accidental.
By default an arrow means one step, but in large
tunings it may be useful to notate multiple steps
with one arrow.".to_string(),
            Self::Division => "Current number of rows per beat.".to_string(),
            Self::Octave => "Current octave for note input.".to_string(),
            Self::Predelay =>
"Delay before the dry signal reaches the reverb.
Can be used to avoid muddying transients, or to
create a sense of spatial proximity.".to_string(),
            Self::DelayTime => "Time between echoes.".to_string(),
            Self::DelayFeedback =>
"Amount of self-feedback. Larger values create more
persistent echoes.".to_string(),
            Self::CompGain => "Pre-compression gain.".to_string(),
            Self::CompThreshold => "Amplitude threshold where compression starts."
                .to_string(),
            Self::CompRatio =>
"Ratio of input dB above threshold to output dB
above threshold.".to_string(),
            Self::CompAttack =>
"Approximate time the compressor takes to engage
when the input level rises.".to_string(),
            Self::CompRelease =>
"Approximate time the compressor takes to disengage
when the input level falls.".to_string(),
            Self::StereoWidth =>
"Multiplier to instrument pan values. Can be used
to check the mono mix, or to reverse panning. Does
not affect render output.".to_string(),
            Self::Gamma =>
"Gamma correction. Applies a brightness curve to
make value differences look approximately uniform.".to_string(),
            Self::Chroma =>
"Colorfulness. Different hues reach full saturation
at different points in the 130-180 range.".to_string(),
            Self::GlideTime =>
"Approximate time the patch takes to glide to new
pitches.".to_string(),
            Self::Distortion => "Portion of the signal to be hard clipped.".to_string(),
            Self::FxSend => "Amount of signal to send to the spatial FX bus.".to_string(),
            Self::LoopPoint =>
"Position where loop begins. Snaps to values with
smaller discontinuities. Loop end point is always
the end of the sample.".to_string(),
            Self::Tone =>
"For pulse waves, sets the duty cycle. For noise,
mixes between pink and white noise.".to_string(),
            Self::FreqRatio =>
"Frequency ratio of this generator compared to the
base frequency of the note. Integer values give
harmonic results when mixing or modulating multiple
generators.".to_string(),
            Self::FilterCutoff =>
"Approximate frequency where the filter starts
attenuating input. Also the resonant.".to_string(),
            Self::FilterResonance =>
"How much to emphasize frequencies near the cutoff
frequency.".to_string(),
            Self::Attack => "Time to reach initial peak level.".to_string(),
            Self::Decay => "Time to transition between peak and sustain levels."
                .to_string(),
            Self::Sustain => "Minimum level to hold while note is on.".to_string(),
            Self::Release =>
"Time to transition between sustain level and zero
after note is released.".to_string(),
            Self::LfoDelay => "Time LFO takes to reach full amplitude.".to_string(),
            Self::ModDepth =>
"Amount of modulation. Scale varies depending on
the destination.".to_string(),
            Self::LoadScale =>
"Load a tuning from a Scala .scl file. The tuning
will be notated the same as an equal temperament
with the same number of notes.".to_string(),
            Self::SavePatch => "Write the selected patch to disk.".to_string(),
            Self::LoadPatch => "Load patches from disk.".to_string(),
            Self::DuplicatePatch => "Create a copy of the selected patch.".to_string(),
            Self::LoadSample =>
"Load an audio file from disk. For multichannel
audio, only the first channel will be used. Most
common audio formats are supported. Audio is
normalized when loading. Compressed formats will
use less space in a save file.".to_string(),
            Self::PrevSample => "Load the previous sample in the directory.".to_string(),
            Self::NextSample => "Load the next sample in the directory.".to_string(),
            Self::DetectPitch =>
"Attempt to automatically set the sample pitch to
match the default oscillator pitch. Works best with
harmonic spectra and strong fundamentals.".to_string(),
            Self::Add(s) => format!("Add {}.", s),
            Self::Remove(s) => format!("Remove {}.", s),
            Self::ResetTheme(variant) =>
                format!("Reset colors to the default {} theme.", variant),
            Self::FontSize(op) => format!("{} font size.", op),
            Self::ResetSettings => "Reset all settings to defaults.".to_string(),
        }
    }
}