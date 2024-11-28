use fundsp::hacker::Sequencer;

use crate::{module::{EventData, Module, TICKS_PER_BEAT}, synth::{Key, KeyOrigin, Patch, Synth}};

const INITIAL_TEMPO: f32 = 120.0;

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

    pub fn stop(&mut self) {
        self.playing = false;
        self.clear_notes_with_origin(KeyOrigin::Pattern);
    }

    pub fn track_removed(&mut self, index: usize) {
        self.synths.remove(index + 1);
    }

    pub fn track_added(&mut self) {
        self.synths.push(Synth::new());
    }

    pub fn note_on(&mut self, track: Option<usize>, key: Key,
        pitch: f32, pressure: Option<f32>, patch: &Patch
    ) {
        if let Some(synth) = self.synths.get_mut(track_index(track)) {
            synth.note_on(key, pitch, pressure.unwrap_or(1.0), patch, &mut self.seq);
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

    pub fn clear_notes_with_origin(&mut self, origin: KeyOrigin) {
        for synth in self.synths.iter_mut() {
            synth.clear_notes_with_origin(&mut self.seq, origin);
        }
    }

    pub fn frame(&mut self, module: &Module, dt: f32) {
        if !self.playing {
            return
        }

        let next_tick = self.tick + interval_ticks(dt, INITIAL_TEMPO);

        for (track_i, track) in module.tracks.iter().enumerate() {
            for (channel_i, channel) in track.channels.iter().enumerate() {
                for event in channel {
                    if event.tick >= self.tick && event.tick < next_tick {
                        self.handle_event(&event.data, module, track_i, channel_i);
                    }
                }
            }
        }

        self.tick = next_tick;
    }

    fn handle_event(&mut self, data: &EventData, module: &Module,
        track: usize, channel: usize
    ) {
        match *data {
            EventData::Pitch(note) => {
                if let Some((patch, note)) = module.map_note(note, track) {
                    let key = Key {
                        origin: KeyOrigin::Pattern,
                        channel: channel as u8,
                        key: 0, // TODO
                    };
                    let pitch = module.tuning.midi_pitch(&note);
                    self.note_on(Some(track), key, pitch, None, patch);
                }
            },
            _ => (), // TODO
        }
    }
}

fn track_index(track: Option<usize>) -> usize {
    match track {
        Some(i) => i + 1,
        None => 0,
    }
}

fn interval_ticks(dt: f32, tempo: f32) -> u32 {
    (dt * tempo / 60.0 * TICKS_PER_BEAT as f32).round() as u32
}