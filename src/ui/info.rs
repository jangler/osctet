use crate::{config::Config, input::Action};

/// Info text types for specific controls.
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
    Aftertouch,
    TuningRoot,
    KitNoteIn,
    KitNoteOut,
}

/// Info text types for widget categories.
pub enum ControlInfo {
    None,
    Slider,
    Note,
    Hotkey,
}

pub fn text(info: &Info, ctrl: &ControlInfo, conf: &Config) -> String {
    let mut text = String::new();
    let mut actions = Vec::new();

    // keep max line width around 50 chars
    match info {
        Info::None => (),
        Info::OctaveRatio => text =
"Size of the octave, as a frequency multiplier.
Can be used to slightly stretch the octave, or to
use a different interval as the scale period.".to_string(),
        Info::OctaveSteps => text =
            "Number of steps to divide the octave into.".to_string(),
        Info::ArrowSteps => text =
"Number of steps notated by an up/down accidental.
By default an arrow means one step, but in large
tunings it may be useful to notate multiple steps
with one arrow.".to_string(),
        Info::Division => {
            text = "Current number of rows per beat.".to_string();
            actions = vec![Action::IncrementDivision, Action::DecrementDivision,
                Action::HalveDivision, Action::DoubleDivision];
        },
        Info::Octave => {
            text = "Current octave for note input.".to_string();
            actions = vec![Action::IncrementOctave, Action::DecrementOctave];
        },
        Info::Predelay => text =
"Delay before the dry signal reaches the reverb.
Can be used to avoid muddying transients, or to
create a sense of spatial proximity.".to_string(),
        Info::DelayTime => text = "Time between echoes.".to_string(),
        Info::DelayFeedback => text =
"Amount of self-feedback. Larger values create more
persistent echoes.".to_string(),
        Info::CompGain => text = "Pre-compression gain.".to_string(),
        Info::CompThreshold => text =
            "Amplitude threshold where compression starts.".to_string(),
        Info::CompRatio => text =
"Ratio of input dB above threshold to output dB
above threshold.".to_string(),
        Info::CompAttack => text =
"Approximate time the compressor takes to engage
when the input level rises.".to_string(),
        Info::CompRelease => text =
"Approximate time the compressor takes to disengage
when the input level falls.".to_string(),
        Info::StereoWidth => text =
"Multiplier to instrument pan values. Can be used
to check the mono mix, or to reverse panning. Does
not affect render output.".to_string(),
        Info::Gamma => text =
"Gamma correction. Applies a brightness curve to
make value differences look approximately uniform.".to_string(),
        Info::Chroma => text =
"Colorfulness. Different hues reach full saturation
at different points in the 130-180 range.".to_string(),
        Info::GlideTime => text =
"Approximate time the patch takes to glide to new
pitches.".to_string(),
        Info::Distortion =>
            text = "Portion of the signal to be hard clipped.".to_string(),
        Info::FxSend =>
            text = "Amount of signal to send to the spatial FX bus.".to_string(),
        Info::LoopPoint => text =
"Position where loop begins. Snaps to values with
smaller discontinuities. Loop end point is always
the end of the sample.".to_string(),
        Info::Tone => text =
"For pulse waves, sets the duty cycle. For noise,
mixes between pink and white noise.".to_string(),
        Info::FreqRatio => text =
"Frequency ratio of this generator compared to the
base frequency of the note. Integer values give
harmonic results when mixing or modulating multiple
generators.".to_string(),
        Info::FilterCutoff => text =
"Approximate frequency where the filter starts
attenuating input. Also the resonant.".to_string(),
        Info::FilterResonance => text =
"How much to emphasize frequencies near the cutoff
frequency.".to_string(),
        Info::Attack => text = "Time to reach initial peak level.".to_string(),
        Info::Decay =>
            text = "Time to transition between peak and sustain levels.".to_string(),
        Info::Sustain => text = "Minimum level to hold while note is on.".to_string(),
        Info::Release => text =
"Time to transition between sustain level and zero
after note is released.".to_string(),
        Info::LfoDelay => text = "Time LFO takes to reach full amplitude.".to_string(),
        Info::ModDepth => text =
"Amount of modulation. Scale varies depending on
the destination.".to_string(),
        Info::LoadScale => text =
"Load a tuning from a Scala .scl file. The tuning
will be notated the same as an equal temperament
with the same number of notes.".to_string(),
        Info::SavePatch => text = "Write the selected patch to disk.".to_string(),
        Info::LoadPatch => text = "Load patches from disk.".to_string(),
        Info::DuplicatePatch =>
            text = "Create a copy of the selected patch.".to_string(),
        Info::LoadSample => text =
"Load an audio file from disk. For multichannel
audio, only the first channel will be used. Most
common audio formats are supported. Audio is
normalized when loading. Compressed formats will
use less space in a save file.".to_string(),
        Info::PrevSample =>
            text = "Load the previous sample in the directory.".to_string(),
        Info::NextSample =>
            text = "Load the next sample in the directory.".to_string(),
        Info::DetectPitch => text =
"Attempt to automatically set the sample pitch to
match the default oscillator pitch. Works best with
harmonic spectra and strong fundamentals.".to_string(),
        Info::Add(s) => text = format!("Add {}.", s),
        Info::Remove(s) => text = format!("Remove {}.", s),
        Info::ResetTheme(variant) => text =
            format!("Reset colors to the default {} theme.", variant),
        Info::FontSize(op) => text = format!("{} font size.", op),
        Info::ResetSettings => text = "Reset all settings to defaults.".to_string(),
        Info::Aftertouch => text =
"If enabled, convert channel pressure and key pressure
messages to pressure values. If disabled, only
velocity is converted.".to_string(),
        Info::TuningRoot => text =
"Determines which note is mapped to the start of
the loaded scale. For equal-step scales, this has
no effect.".to_string(),
        Info::KitNoteIn =>
            text = "The note that activates this kit mapping.".to_string(),
        Info::KitNoteOut =>
            text = "The pitch that this kit mapping plays at.".to_string(),
    };

    if !actions.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        for action in actions {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&conf.hotkey_string(action));
        }
    }

    let mut push_if_nonempty = |s| if !text.is_empty() {
        text.push_str(s);
    };

    match ctrl {
        ControlInfo::None => (),
        ControlInfo::Slider => {
            push_if_nonempty("\n\n");
            text.push_str("Right-click slider to edit value as text.")
        }
        ControlInfo::Note => {
            push_if_nonempty("\n\n");
            text.push_str(
"Note input. Click to focus, then enter a note
using the keyboard.");
        }
        ControlInfo::Hotkey => {
            push_if_nonempty("\n\n");
            text.push_str(
"Hotkey input. Click to focus, then press a key
combination to set value.")
        }
    }

    text
}