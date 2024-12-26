pub enum Info {
    None,
    Title,
    Author,
    OctaveRatio,
    OctaveSteps,
    ArrowSteps,
    Division,
    Octave,
}

impl Info {
    pub fn text(&self) -> &'static str {
        // keep max line width around 50 chars
        match self {
            Self::None => "",
            Self::Title => "Song title field.",
            Self::Author => "Song author field.",
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
        }
    }
}