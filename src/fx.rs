// global FX

use fundsp::hacker32::*;
use realseq::SequencerBackend;
use serde::{Deserialize, Serialize};

use crate::dsp::compressor;

// serializable global FX settings
#[derive(Clone, Serialize, Deserialize)]
pub struct FXSettings {
    pub spatial: SpatialFx,
    pub comp: Compression,
}

impl Default for FXSettings {
    fn default() -> Self {
        Self {
            comp: Default::default(),
            spatial: Default::default(),
        }
    }
}

// controls updates of global FX
pub struct GlobalFX {
    pub net: Net,
    spatial_id: NodeId,
    comp_id: NodeId,
}

impl GlobalFX {
    pub fn new(backend: SequencerBackend, settings: &FXSettings) -> Self {
        let (spatial, spatial_id) = Net::wrap_id(settings.spatial.make_node());
        let (comp, comp_id) = Net::wrap_id(settings.comp.make_node());

        Self {
            net: Net::wrap(Box::new(backend))
                >> (multipass::<U2>()
                    + (multipass::<U2>() >> spatial))
                >> (dcblock() | dcblock())
                >> comp,
            spatial_id,
            comp_id,
        }
    }

    /// Constructs a new instance with a dummy sequencer backend.
    pub fn new_dummy(settings: &FXSettings) -> Self {
        Self::new(Sequencer::new(false, 4).backend(), settings)
    }

    pub fn reinit(&mut self, settings: &FXSettings) {
        self.net.crossfade(self.spatial_id, Fade::Smooth, 0.1,
            settings.spatial.make_node());
        self.net.crossfade(self.comp_id, Fade::Smooth, 0.1,
            settings.comp.make_node());
        self.net.commit();
    }

    pub fn commit_spatial(&mut self, spatial: &SpatialFx) {
        self.crossfade(self.spatial_id, spatial.make_node());
    }

    pub fn commit_comp(&mut self, comp: &Compression) {
        self.crossfade(self.comp_id, comp.make_node());
    }

    fn crossfade(&mut self, id: NodeId, unit: Box<dyn AudioUnit>) {
        self.net.crossfade(id, Fade::Smooth, 0.1, unit);
        self.net.commit();
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Compression {
    pub gain: f32,
    pub threshold: f32,
    pub slope: f32,
    pub attack: f32,
    pub release: f32,
}

impl Compression {
    fn make_node(&self) -> Box<dyn AudioUnit> {
        if self.threshold < 1.0 && self.slope > 0.0 {
            Box::new((mul(self.gain) | mul(self.gain))
                >> compressor(self.threshold, self.slope, self.attack, self.release))
        } else {
            Box::new(pass() | pass())
        }
    }
}

impl Default for Compression {
    fn default() -> Self {
        Self {
            gain: 0.5,
            threshold: db_amp(-3.0),
            slope: 0.75,
            attack: 0.001,
            release: 0.05,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SpatialFx {
    None,
    Reverb {
        level: f32,
        room_size: f32,
        decay_time: f32,
    },
    Delay {
        level: f32,
        time: f32,
        feedback: f32,
    }
}

impl SpatialFx {
    pub const DEFAULT_VARIANTS: [Self; 3] = [
        Self::None,
        Self::Reverb { level: 0.1, room_size: 20.0, decay_time: 0.2 },
        Self::Delay { level: 0.1, time: 0.5, feedback: 0.5 },
    ];

    fn make_node(&self) -> Box<dyn AudioUnit> {
        match self {
            Self::None => Box::new(mul(0.0) | mul(0.0)),
            Self::Reverb { level, room_size, decay_time } => {
                Box::new((pass() | pass())
                    >> *level * reverb2_stereo(*room_size, *decay_time, 0.5, 0.5,
                        lowpole_hz(5000.0) >> highpole_hz(80.0)))
            }
            Self::Delay { level, time, feedback } => {
                Box::new(*level * hacker32::feedback(
                    (delay(*time) | delay(*time)) * *feedback))
            }
        }
    }

    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Reverb { .. } => "Reverb",
            Self::Delay { .. } => "Delay",
        }
    }
}

impl Default for SpatialFx {
    fn default() -> Self {
        Self::None
    }
}