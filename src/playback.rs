use fundsp::hacker::Sequencer;

use crate::synth::{Key, Patch, Synth};

pub struct Player {
    seq: Sequencer,
    synths: Vec<Synth>, // one for keyjazz plus one per track
    pub playing: bool,
    pub tick: u32,
}

impl Player {
    pub fn new(seq: Sequencer, num_tracks: usize) -> Self {
        let mut synths = vec![Synth::new()];
        for _ in 0..=num_tracks {
            synths.push(Synth::new());
        }

        Self {
            seq,
            synths,
            playing: false,
            tick: 0,
        }
    }

    pub fn note_on(&mut self, track: Option<usize>, key: Key,
        pitch: f32, pressure: f32, patch: &Patch
    ) {
        if let Some(synth) = self.synths.get_mut(track_index(track)) {
            synth.note_on(key, pitch, pressure, patch, &mut self.seq);
        }
    }

    pub fn note_off(&mut self, track: Option<usize>, key: Key) {
        if let Some(synth) = self.synths.get_mut(track_index(track)) {
            synth.note_off(key, &mut self.seq);
        }
    }

    pub fn poly_pressure(&mut self, track: Option<usize>, key: Key, pressure: f32) {
        if let Some(synth) = self.synths.get_mut(track_index(track)) {
            synth.poly_pressure(key, pressure);
        }
    }

    pub fn modulate(&mut self, track: Option<usize>, depth: f32) {
        // TODO: shouldn't this take a channel argument?
        if let Some(synth) = self.synths.get_mut(track_index(track)) {
            synth.modulate(depth);
        }
    }

    pub fn channel_pressure(&mut self, track: Option<usize>, channel: u8, pressure: f32) {
        if let Some(synth) = self.synths.get_mut(track_index(track)) {
            synth.channel_pressure(channel, pressure);
        }
    }

    pub fn pitch_bend(&mut self, track: Option<usize>, channel: u8, bend: f32) {
        if let Some(synth) = self.synths.get_mut(track_index(track)) {
            synth.pitch_bend(channel, bend);
        }
    }

    pub fn clear_keyboard_notes(&mut self) {
        for synth in self.synths.iter_mut() {
            synth.clear_keyboard_notes(&mut self.seq);
        }
    }

    pub fn clear_midi_notes(&mut self) {
        for synth in self.synths.iter_mut() {
            synth.clear_midi_notes(&mut self.seq);
        }
    }
}

fn track_index(track: Option<usize>) -> usize {
    match track {
        Some(i) => i + 1,
        None => 0,
    }
}