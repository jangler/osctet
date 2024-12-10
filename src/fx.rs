// global FX

use fundsp::hacker32::*;
use realseq::SequencerBackend;
use serde::{Deserialize, Serialize};

use crate::synth::Parameter;

// serializable global FX settings
#[derive(Clone, Serialize, Deserialize)]
pub struct FXSettings {
    pub gain: Parameter,
    pub reverb_amount: Parameter,
    pub predelay_time: f32,
    pub reverb_room_size: f32,
    pub reverb_time: f32,
    pub reverb_diffusion: f32,
    pub reverb_mod_speed: f32,
    pub reverb_damping: f32,
}

impl FXSettings {
    pub fn make_gain(&self) -> Box<dyn AudioUnit> {
        Box::new(var(&self.gain.0) >> split::<U4>())
    }

    pub fn make_amount(&self) -> Box<dyn AudioUnit> {
        Box::new(var(&self.reverb_amount.0) >> split::<U2>())
    }

    pub fn make_predelay(&self) -> Box<dyn AudioUnit> {
        Box::new(delay(self.predelay_time) | delay(self.predelay_time))
    }

    pub fn make_reverb(&self) -> Box<dyn AudioUnit> {
        Box::new(reverb2_stereo(
            self.reverb_room_size,
            self.reverb_time,
            self.reverb_diffusion,
            self.reverb_mod_speed,
            highshelf_hz(5000.0, 1.0, db_amp(-self.reverb_damping))
                >> lowshelf_hz(80.0, 1.0, db_amp(-self.reverb_damping))))
    }
}

impl Default for FXSettings {
    fn default() -> Self {
        Self {
            gain: Parameter(shared(1.0)),
            reverb_amount: Parameter(shared(0.1)),
            predelay_time: 0.01,
            reverb_room_size: 20.0,
            reverb_time: 0.2,
            reverb_diffusion: 0.5,
            reverb_mod_speed: 0.5,
            reverb_damping: 3.0,
        } 
    }
}

// controls updates of global FX
pub struct GlobalFX {
    pub net: Net,
    gain_id: NodeId,
    amount_id: NodeId,
    predelay_id: NodeId,
    reverb_id: NodeId,
}

impl GlobalFX {
    pub fn new(backend: SequencerBackend, settings: &FXSettings) -> Self {
        let (predelay, predelay_id) = Net::wrap_id(settings.make_predelay());
        let (reverb, reverb_id) = Net::wrap_id(settings.make_reverb());
        let (gain, gain_id) = Net::wrap_id(settings.make_gain());
        let (amount, amount_id) = Net::wrap_id(settings.make_amount());

        Self {
            net: Net::wrap(Box::new(backend))
                * gain
                >> (highpass_hz(1.0, 0.1)
                    | highpass_hz(1.0, 0.1)
                    | highpass_hz(1.0, 0.1)
                    | highpass_hz(1.0, 0.1))
                >> (shape(Tanh(1.0))
                    | shape(Tanh(1.0))
                    | shape(Tanh(1.0))
                    | shape(Tanh(1.0)))
                >> (multipass::<U2>()
                    | (multipass::<U2>() >> amount * (predelay >> reverb)))
                >> multijoin::<U2, U2>(),
            gain_id,
            amount_id,
            predelay_id,
            reverb_id,
        }
    }

    /// Constructs a new instance with a dummy sequencer backend.
    pub fn new_dummy(settings: &FXSettings) -> Self {
        Self::new(Sequencer::new(false, 4).backend(), settings)
    }

    pub fn reinit(&mut self, settings: &FXSettings) {
        self.net.crossfade(self.gain_id, Fade::Smooth, 0.1, settings.make_gain());
        self.net.crossfade(self.amount_id, Fade::Smooth, 0.1, settings.make_amount());
        self.net.crossfade(self.predelay_id, Fade::Smooth, 0.1, settings.make_predelay());
        self.net.crossfade(self.reverb_id, Fade::Smooth, 0.1, settings.make_reverb());
        self.net.commit();
    }

    pub fn commit_predelay(&mut self, settings: &FXSettings) {
        self.crossfade(self.predelay_id, settings.make_predelay());
    }

    pub fn commit_reverb(&mut self, settings: &FXSettings) {
        self.crossfade(self.reverb_id, settings.make_reverb());
    }

    fn crossfade(&mut self, id: NodeId, unit: Box<dyn AudioUnit>) {
        self.net.crossfade(id, Fade::Smooth, 0.1, unit);
        self.net.commit();
    }
}