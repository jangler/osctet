use crate::{config::Config, input::Action};

/// Info text types for specific controls.
#[derive(PartialEq, Clone)]
pub enum Info {
    None,
    OctaveRatio,
    OctaveSteps,
    ArrowSteps,
    Division,
    Octave,
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
    Action(Action),
    GlobalTrack,
    KitTrack,
    MidiInput,
    SpatialFxType,
    KitPatch,
    Waveform,
    GenOutput,
    FilterType,
    FilterKeytrack,
    ModSource,
    ModDest,
    TrackPatch,
    SmoothPlayhead,
    ControlColumn,
    NoteColumn,
    PressureColumn,
    ModulationColumn,
    NoteLayout,
    Compression,
    Tuning,
    Generators,
    Filters,
    Envelopes,
    Lfos,
    ModMatrix,
    DisplayInfo,
    DesiredSampleRate,
    VerticalScrollbar,
    HorizontalScrollbar,
    SaveTheme,
    LoadTheme,
    InstrumentList,
    Font,
    Oversample,
}

impl Default for Info {
    fn default() -> Self {
        Self::None
    }
}

/// Info text types for widget categories.
#[derive(PartialEq)]
pub enum ControlInfo {
    None,
    Slider,
    Note,
    Hotkey,
}

impl Default for ControlInfo {
    fn default() -> Self {
        Self::None
    }
}

/// Returns help text.
pub fn text(info: &Info, ctrl: &ControlInfo, conf: &Config) -> String {
    let mut text = String::new();
    let mut actions = Vec::new();
    let mut custom_actions = false;

    // keep max line width around 50 chars
    match info {
        Info::None => (),
        Info::Oversample => text =
"Run the generator at twice the normal sample rate.
Mainly useful for avoiding inharmonic artifacts in
high-pitched modulators.".to_string(),
        Info::Font =>
            text = "Font is a modified version of Dina by Joergen Ibsen.".to_string(),
        Info::InstrumentList => text =
"Patch list. Right-click to edit names.

Up - Previous entry
Down - Next entry".to_string(),
        Info::LoadTheme => text = "Load color theme from disk.".to_string(),
        Info::SaveTheme => text = "Save color theme to disk.".to_string(),
        Info::VerticalScrollbar => text =
"Vertical scrollbar.

Wheel - Scroll
Alt+Wheel - Scroll faster".to_string(),
        Info::HorizontalScrollbar => text =
"Horizontal scrollbar.

Shift+Wheel - Scroll".to_string(),
        Info::DesiredSampleRate => text =
"Audio output sample rate to request, in Hz. Requires
program restart to take effect. Does not affect
sample rate of WAV export.".to_string(),
        Info::DisplayInfo =>
            text = "Display mouseover help text for UI elements.".to_string(),
        Info::Generators => text =
"Generators create the initial signal that other
patch parameters shape.".to_string(),
        Info::Filters => text =
"Filters attenuate certain parts of the frequency
spectrum to change the timbre of a sound.".to_string(),
        Info::Envelopes => text =
"Envelopes modulate parameters between different
levels over time. They have no effect unless
assigned in the mod matrix.".to_string(),
        Info::Lfos => text =
"Low-frequency oscillators modulate parameters in
a repeating pattern. They have no effect unless
assigned in the mod matrix.".to_string(),
        Info::ModMatrix => text =
"Assign modulation inputs and outputs. Modulation
must not contain loops.".to_string(),
        Info::Compression => text =
"Dynamic range compression. Reduces the output level
based on the input level. Can be used to clip peaks,
shape transients, regulate overall volume, etc.".to_string(),
        Info::Tuning => text =
"Song tuning. Notation is always diatonic, based
on the tuning's octave and best fifth.".to_string(),
        Info::NoteLayout => text =
"Keys used for note input. The octaves of these
notes represent an offset from the base octave
setting.".to_string(),
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
            text =
"Current number of rows per beat.

Ctrl+Scroll - Inc/dec division
Ctrl+Alt+Scroll - Double/halve division".to_string();
            custom_actions = true;
            actions = vec![Action::IncrementDivision, Action::DecrementDivision,
                Action::HalveDivision, Action::DoubleDivision];
        },
        Info::Octave => {
            text = "Current octave for note input.".to_string();
            actions = vec![Action::IncrementOctave, Action::DecrementOctave];
        },
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
attenuating input. Also the resonant peak of the
filter.".to_string(),
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
the destination. For gain/level controls, depth
indicates the amount of attenuation rather than
the amount of increase.".to_string(),
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
        Info::Add(s) => text = format!("Add {s}."),
        Info::Remove(s) => text = format!("Remove {s}."),
        Info::ResetTheme(variant) => text =
            format!("Reset colors to the default {variant} theme."),
        Info::FontSize(op) => text = format!("{op} font size."),
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
        Info::Action(action) => match action {
            Action::RenderTracks => text =
"Render each track to WAV. Compression will be
applied on a per-track basis.".to_string(),
            Action::CycleNotation =>
                text = "Cycle selected notes through alternative notations.".to_string(),
            Action::IncrementOctave =>
                text = "Increment the octave used for note input.".to_string(),
            Action::DecrementOctave =>
                text = "Decrement the octave used for note input.".to_string(),
            Action::PlayFromStart =>
                text = "Play/stop from the beginning of the song.".to_string(),
            Action::PlayFromScreen =>
                text = "Play/stop from the first beat on-screen.".to_string(),
            Action::PlayFromCursor =>
                text = "Play/stop from the pattern cursor.".to_string(),
            Action::RenderSong => text = "Render song to WAV.".to_string(),
            Action::Undo => text = "Undo last pattern action.".to_string(),
            Action::Redo => text = "Redo last undone pattern action.".to_string(),
            Action::MixPaste => text =
"Paste pattern data. Blank space in paste data
will not overwrite events.".to_string(),
            Action::NoteOff => text =
"Insert a note off event to trigger envelope release.
With Shift, insert into all track channels.".to_string(),
            Action::End => text =
"Mark the end point of the song. Can only be placed
in a Ctrl channel.".to_string(),
            Action::Loop => text =
"Mark the beginning loop point of the song. If this
marker is present, the song will return to it when
the End marker is reached. Can only be placed in a
Ctrl channel.".to_string(),
            Action::TapTempo => text =
"Insert a tempo change event. Tap in time to set
tempo. Can only be placed in a Ctrl channel.".to_string(),
            Action::RationalTempo => text =
"Insert a tempo change event. Tempo will change so
that the selected timespan will receive the same
time that 1 beat previously received. Can only be
placed in a Ctrl channel.".to_string(),
            Action::InsertRows =>
                text = "Push pattern events by inserting rows.".to_string(),
            Action::DeleteRows =>
                text = "Pull pattern events by deleting rows.".to_string(),
            Action::NudgeArrowUp => text =
"Tranpose the selected notes up by one arrow. Can
also be held to transpose note input.".to_string(),
            Action::NudgeArrowDown => text =
"Tranpose the selected notes down by one arrow. Can
also be held to transpose note input.".to_string(),
            Action::NudgeSharp => text =
"Transpose the selected notes up by one sharp. Can
also be held to transpose note input.".to_string(),
            Action::NudgeFlat => text =
"Transpose the selected notes down by one flat. Can
also be held to transpose note input.".to_string(),
            Action::NudgeOctaveUp => text =
"Transpose the selected notes up by one octave. Can
also be held to transpose note input.".to_string(),
            Action::NudgeOctaveDown => text =
"Transpose the selected notes down by one octave. Can
also be held to transpose note input.".to_string(),
            Action::NudgeEnharmonic => text =
"Replace the selected notes with enharmonic
alternatives. Can also be held to remap note input.
Enharmonic notes have unequal values in most tunings.".to_string(),
            Action::ToggleFollow => text =
"Toggle whether the pattern view tracks the playhead.".to_string(),
            Action::SelectAllChannels =>
                text = "Expand the pattern selection to all channels.".to_string(),
            Action::SelectAllRows =>
                text = "Expand the pattern selection to all rows in song.".to_string(),
            Action::PlaceEvenly => text =
"Place selected events evenly across the selected
timespan.".to_string(),
            Action::PrevBeat =>
                text = "Move the pattern cursor up by 1 beat.".to_string(),
            Action::NextBeat =>
                text = "Move the pattern cursor down by 1 beat.".to_string(),
            Action::PrevEvent => text =
"Move the pattern cursor to the previous event in
the channel.".to_string(),
            Action::NextEvent => text =
"Move the pattern cursor to the next event in the
channel.".to_string(),
            Action::PatternStart => text = "Move the cursor to beat 1.".to_string(),
            Action::PatternEnd =>
                text = "Move the cursor to the time of the final event.".to_string(),
            Action::IncrementValues =>
                text = "Increment selected pattern values by 1 step.".to_string(),
            Action::DecrementValues =>
                text = "Decrement selected pattern values by 1 step.".to_string(),
            Action::Interpolate => text =
"Smoothly transition between two pitches, pressure
levels, or modulation levels. If a timespan is
selected, interpolate over that timespan. Otherwise,
interpolate from the cursor position to the next
column event.".to_string(),
            Action::MuteTrack => text = "Toggle muting the current track.".to_string(),
            Action::SoloTrack => text =
"Toggle muting all tracks except for the current
track.".to_string(),
            Action::Panic => text = "Cut all notes and stop playback.".to_string(),
            Action::InsertPaste => text =
"Paste, shifting existing events by the size of the
clipboard.".to_string(),
            Action::UseLastNote =>
                text = "Insert a copy of the last note in the channel.".to_string(),
            Action::IncrementDivision => text = "Increase beat division by 1.".to_string(),
            Action::DecrementDivision => text = "Decrease beat division by 1.".to_string(),
            Action::DoubleDivision => text = "Double the beat division.".to_string(),
            Action::HalveDivision => text = "Halve the beat division.".to_string(),
            Action::FocusDivision => text = "Focus the division field.".to_string(),
            Action::StopPlayback => text = "Stop song playback.".to_string(),
            Action::NewSong =>
                text = "Close the open song and start a new one.".to_string(),
            Action::OpenSong => text = "Load a song from disk.".to_string(),
            Action::SaveSong => text =
"Save the open song, using the path it was last
saved to or loaded from.".to_string(),
            Action::SaveSongAs => text =
"Save the open song using a file dialog.".to_string(),
            Action::Cut =>
                text = "Delete and copy selection to the internal clipboard.".to_string(),
            Action::Copy =>
                text = "Copy selection to the internal clipboard.".to_string(),
            Action::Paste => text = "Paste data from the internal clipboard.".to_string(),
            Action::NextRow => text = "Move pattern cursor down 1 row.".to_string(),
            Action::PrevRow => text = "Move pattern cursor up 1 row.".to_string(),
            Action::NextColumn => text = "Move pattern cursor right 1 column.".to_string(),
            Action::PrevColumn => text = "Move pattern cursor left 1 column.".to_string(),
            Action::NextChannel =>
                text = "Move pattern cursor right 1 channel.".to_string(),
            Action::PrevChannel =>
                text = "Move pattern cursor left 1 channel.".to_string(),
            Action::Delete => text =
"Delete selected data. With Shift, delete from all
track channels.".to_string(),
            Action::NextTab => text = "View the next UI tab.".to_string(),
            Action::PrevTab => text = "View the previous UI tab.".to_string(),
            Action::UnmuteAllTracks => text = "Unmute all muted tracks.".to_string(),
            Action::Quit => text = "Close the program.".to_string(),
        }
        Info::GlobalTrack =>
            text = "Holds control events like tempo, loop, and end.".to_string(),
        Info::KitTrack => text =
"Uses the patch & note mappings from the Kit entry
in the Instruments tab.".to_string(),
        Info::MidiInput => text = "MIDI input to use for note input.".to_string(),
        Info::SpatialFxType => text =
"Type of global spatial FX to use. Individual send
levels can be set in patch settings.".to_string(),
        Info::KitPatch => text = "The patch that plays this kit mapping.".to_string(),
        Info::Waveform => text =
"Waveform used by the generator. S&H is periodically
sampled white noise. For generators, Noise is pink
to white noise based on the Tone control. For LFOs,
Noise is brown noise.".to_string(),
        Info::GenOutput => text =
"The destination for this generator's signal. The
signal can be mixed with the final outputs of other
generators, mixed with the previous generator, or
used to modulate the previous generator.

- AM (amplitude modulation) mixes the carrier with
  the combination tones of the carrier and modulator.
- RM (ring modulation) takes only the combination
  tones of the carrier and modulator.
- FM (frequency modulation) is similar to AM, but
  creates a series of tones for each tone that AM
  would create.".to_string(),
        Info::FilterType => text =
"Filter type. Ladder is 24 dB/oct and can self-
oscillate; other filters are 12 dB/oct.".to_string(),
        Info::FilterKeytrack => text =
"How much the filter cutoff follows the fundamental
of the note. The break-even point for key tracking
is C4 (~261 Hz).".to_string(),
        Info::ModSource => text =
"The source used for this modulation. Most sources
operate in the range 0..1, but LFOs oscillate in
the range -1..1.".to_string(),
        Info::ModDest => text = "The modulated parameter.".to_string(),
        Info::TrackPatch => text = "The patch controlled by this track.".to_string(),
        Info::SmoothPlayhead => text =
"If disabled, playhead visual and pattern follow
will be quantized to the nearest row.".to_string(),
        Info::PressureColumn => text =
"Pressure column.

0..F - Enter digit
Shift+0..F - Track enter digit".to_string(),
        Info::ModulationColumn => text =
"Modulation column.

0..F - Enter digit
Shift+0..F - Track enter digit".to_string(),
        Info::ControlColumn => {
            text =
"Control column. Type to enter BPM values (ex. 120)
or tempo ratios (ex. 3:2 or 3/2).".to_string();
            actions =
                vec![Action::TapTempo, Action::RationalTempo, Action::Loop, Action::End];
        },
        Info::NoteColumn => {
            text = "Note column.".to_string();
            actions = vec![Action::NoteOff, Action::CycleNotation, Action::UseLastNote];
        },
    };

    if !actions.is_empty() {
        if !text.is_empty() && !custom_actions {
            text.push('\n');
        }
        for action in actions {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&conf.hotkey_string(action));
        }
    }

    if !text.is_empty() && !matches!(ctrl, ControlInfo::None) {
        text.push_str("\n\n");
    };

    match ctrl {
        ControlInfo::None => (),
        ControlInfo::Slider => {
            text.push_str("Right-click slider to edit value as text.")
        }
        ControlInfo::Note => {
            text.push_str(
"Note input. Click to focus, then enter a note
using the keyboard.")
        }
        ControlInfo::Hotkey => {
            text.push_str(
"Hotkey input. Click to focus, then press a key
combination to set value.")
        }
    }

    text
}