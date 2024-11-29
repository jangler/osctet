use fundsp::{hacker::{AudioUnit, BlockRateAdapter, Sequencer}, wave::Wave};

use crate::{fx::GlobalFX, module::{EventData, Module, TICKS_PER_BEAT}, synth::{Key, KeyOrigin, Patch, Synth}};

const INITIAL_TEMPO: f32 = 120.0;

pub struct Player {
    seq: Sequencer,
    synths: Vec<Synth>, // one for keyjazz plus one per track
    playing: bool,
    tick: u32,
    start_tick: u32,
    playtime: f32,
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
            start_tick: 0,
            playtime: 0.0,
        }
    }

    pub fn get_tick(&self) -> u32 {
        self.tick
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.clear_notes_with_origin(KeyOrigin::Pattern);
    }

    pub fn play(&mut self) {
        self.playing = true;
        self.start_tick = self.tick;
        self.playtime = 0.0;
    }

    pub fn play_from(&mut self, tick: u32) {
        self.tick = tick;
        self.play();
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

        self.playtime += dt;
        let next_tick = self.start_tick + interval_ticks(self.playtime, INITIAL_TEMPO);

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

pub fn render(module: &Module) -> Wave {
    let sample_rate = 44100;
    let mut wave = Wave::new(2, sample_rate as f64);
    let mut seq = Sequencer::new(false, 2);
    seq.set_sample_rate(sample_rate as f64);
    let mut fx = GlobalFX::new_from_settings(seq.backend(), module.fx.settings.clone());
    fx.net.set_sample_rate(sample_rate as f64);
    let mut player = Player::new(seq, module.tracks.len());
    let mut backend = BlockRateAdapter::new(Box::new(fx.net.backend()));
    let block_size = 64;
    let dt = block_size as f32 / sample_rate as f32;
    let last_event_tick = module.last_event_tick();

    player.play();
    while player.tick <= last_event_tick {
        player.frame(module, dt);
        for _ in 0..block_size {
            wave.push(backend.get_stereo());
        }
    }

    wave
}