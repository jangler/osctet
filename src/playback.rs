use std::{path::PathBuf, sync::mpsc::{self, Receiver}, thread};

use fundsp::hacker32::*;

use crate::{fx::GlobalFX, module::{Event, EventData, LocatedEvent, Module, TrackEdit, MOD_COLUMN, NOTE_COLUMN, TICKS_PER_BEAT, VEL_COLUMN}, synth::{Key, KeyOrigin, Patch, Synth, DEFAULT_PRESSURE}};

pub const DEFAULT_TEMPO: f32 = 120.0;
const LOOP_FADEOUT_TIME: f64 = 10.0;

/// Handles module playback. In methods that take a `track` argument, 0 can
/// safely be used for keyjazz events (since track 0 will never sequence).
pub struct Player {
    pub seq: Sequencer,
    synths: Vec<Synth>, // one per track
    playing: bool,
    tick: u32,
    playtime: f64,
    tempo: f32,
    looped: bool,
    metronome: bool,
    sample_rate: f32,
    pub stereo_width: Shared,
}

impl Player {
    pub fn new(seq: Sequencer, num_tracks: usize, sample_rate: f32) -> Self {
        Self {
            seq,
            synths: (0..num_tracks).map(|_| Synth::new(sample_rate)).collect(),
            playing: false,
            tick: 0,
            playtime: 0.0, // not total playtime!
            tempo: DEFAULT_TEMPO,
            looped: false,
            metronome: false,
            sample_rate,
            stereo_width: shared(1.0),
        }
    }

    pub fn reinit(&mut self, num_tracks: usize) {
        for synth in &mut self.synths {
            synth.clear_all_notes(&mut self.seq);
        }
        self.synths = (0..num_tracks).map(|_| Synth::new(self.sample_rate)).collect();
        self.playing = false;
        self.tick = 0;
        self.playtime = 0.0;
        self.tempo = DEFAULT_TEMPO;
        self.looped = false;
    }

    pub fn get_tick(&self) -> u32 {
        self.tick
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.metronome = false;
        self.clear_notes_with_origin(KeyOrigin::Pattern);
    }

    pub fn play(&mut self) {
        self.playing = true;
        self.playtime = 0.0;
        self.looped = false;
    }

    pub fn play_from(&mut self, tick: u32, module: &Module) {
        self.simulate_events(tick, module);
        self.tick = tick;
        self.play();
    }

    pub fn record_from(&mut self, tick: u32, module: &Module) {
        self.metronome = true;
        self.play_from(tick, module);
    }

    pub fn update_synths(&mut self, edits: Vec<TrackEdit>) {
        for edit in edits {
            match edit {
                TrackEdit::Insert(i) => {
                    self.synths.insert(i, Synth::new(self.sample_rate));
                }
                TrackEdit::Remove(i) => {
                    self.synths.remove(i);
                }
            }
        }
    }

    pub fn note_on(&mut self, track: usize, key: Key,
        pitch: f32, pressure: Option<f32>, patch: &Patch
    ) {
        if let Some(synth) = self.synths.get_mut(track) {
            synth.note_on(key, pitch, pressure, patch, &mut self.seq, &self.stereo_width);
        }
    }

    pub fn note_off(&mut self, track: usize, key: Key) {
        if let Some(synth) = self.synths.get_mut(track) {
            synth.note_off(key, &mut self.seq);
        }
    }

    pub fn poly_pressure(&mut self, track: usize, key: Key, pressure: f32) {
        if let Some(synth) = self.synths.get_mut(track) {
            synth.poly_pressure(key, pressure);
        }
    }

    pub fn modulate(&mut self, track: usize, channel: u8, depth: f32) {
        if let Some(synth) = self.synths.get_mut(track) {
            synth.modulate(channel, depth);
        }
    }

    pub fn channel_pressure(&mut self, track: usize, channel: u8, pressure: f32) {
        if let Some(synth) = self.synths.get_mut(track) {
            synth.channel_pressure(channel, pressure);
        }
    }

    /// MIDI-style pitch bend.
    pub fn pitch_bend(&mut self, track: usize, channel: u8, bend: f32) {
        if let Some(synth) = self.synths.get_mut(track) {
            synth.pitch_bend(channel, bend);
        }
    }

    /// Interpolation pitch bend.
    pub fn bend_to(&mut self, track: usize, key: Key, pitch: f32) {
        if let Some(synth) = self.synths.get_mut(track) {
            synth.bend_to(key, pitch);
        }
    }

    pub fn clear_notes_with_origin(&mut self, origin: KeyOrigin) {
        for synth in self.synths.iter_mut() {
            synth.clear_notes_with_origin(&mut self.seq, origin);
        }
    }

    /// Turns off all notes and stops playback.
    pub fn panic(&mut self) {
        self.stop();
        for synth in self.synths.iter_mut() {
            synth.panic(&mut self.seq);
        }
    }

    pub fn frame(&mut self, module: &Module, dt: f64) {
        if !self.playing {
            return
        }

        self.playtime += dt;
        let prev_tick = self.tick;
        self.tick += interval_ticks(self.playtime, self.tempo);
        self.playtime -= tick_interval(self.tick - prev_tick, self.tempo);

        let mut events = Vec::new();

        for (track_i, track) in module.tracks.iter().enumerate() {
            for (channel_i, channel) in track.channels.iter().enumerate() {
                let mut prev_data = [None, None, None];
                let mut next_event = [None, None, None];
                let mut start_tick = [0, 0, 0];
                let mut glide = [false, false, false];

                for event in &channel.events {
                    let col = event.data.logical_column();

                    if event.tick < self.tick {
                        if event.tick >= prev_tick {
                            events.push(LocatedEvent {
                                event: event.clone(),
                                track: track_i,
                                channel: channel_i,
                            });
                        }

                        match event.data {
                            EventData::StartGlide(i) => if glide[i as usize] {
                                continue
                            } else {
                                glide[i as usize] = true;
                            }
                            EventData::EndGlide(i) => glide[i as usize] = false,
                            _ => (),
                        }

                        if let Some(v) = prev_data.get_mut(col as usize) {
                            *v = Some(&event.data);
                        }

                        start_tick[event.data.spatial_column() as usize] = event.tick;
                    } else {
                        if let Some(v) = next_event.get_mut(col as usize) {
                            if v.is_none() {
                                *v = Some(event);
                            }
                        }
                    }
                }

                for i in 0..prev_data.len() {
                    if glide[i] {
                        if let Some(data) = interpolate_events(
                            prev_data[i], next_event[i], start_tick[i], self.tick, &module
                        ) {
                            events.push(LocatedEvent {
                                track: track_i,
                                channel: channel_i,
                                event: Event {
                                    tick: self.tick,
                                    data,
                                },
                            });
                        }
                    }
                }
            }
        }

        events.sort_by_key(|e| (e.event.tick, e.event.data.spatial_column()));

        // set pressure/modulation memory so that new notes will use new values
        for event in &events {
            match event.event.data {
                EventData::Pressure(v) => self.synths[event.track].set_vel_memory(
                    event.channel as u8, v as f32 / EventData::DIGIT_MAX as f32),
                EventData::Modulation(v) => self.synths[event.track].set_mod_memory(
                    event.channel as u8, v as f32 / EventData::DIGIT_MAX as f32),
                _ => (),
            }
        }

        for event in events {
            self.handle_event(&event.event, module, event.track, event.channel);
        }

        if self.metronome && (self.tick as f32 / TICKS_PER_BEAT as f32).ceil()
            != (prev_tick as f32 / TICKS_PER_BEAT as f32).ceil() {
            self.seq.push_relative(0.0, 0.01, Fade::Smooth, 0.01, 0.01,
                Box::new(square_hz(440.0 * 8.0) >> split::<U4>()));
        }
    }

    /// Update state as if the module had been played up to a given tick.
    fn simulate_events(&mut self, tick: u32, module: &Module) {
        self.tempo = DEFAULT_TEMPO;

        for track in 0..module.tracks.len() {
            self.simulate_track_events(tick, module, track);
        }
    }

    fn simulate_track_events(&mut self, tick: u32, module: &Module, track_i: usize) {
        self.synths[track_i].reset_memory();

        for (channel_i, channel) in module.tracks[track_i].channels.iter().enumerate() {
            let mut events: Vec<_> = channel.events.iter()
                .filter(|e| e.tick < tick)
                .collect();
            events.sort_by_key(|e| (e.tick, e.data.spatial_column()));

            let mut active_note = None;

            for evt in events {
                match evt.data {
                    EventData::Pitch(note) => {
                        if let Some((patch, note)) = module.map_note(note, track_i) {
                            if patch.sustains() {
                                active_note = Some((patch, note));
                            }
                        }
                    }
                    EventData::Pressure(v) => {
                        self.channel_pressure(track_i, channel_i as u8,
                            v as f32 / EventData::DIGIT_MAX as f32);
                    }
                    EventData::Modulation(v) => {
                        self.modulate(track_i, channel_i as u8,
                            v as f32 / EventData::DIGIT_MAX as f32);
                    }
                    EventData::NoteOff => active_note = None,
                    EventData::Tempo(t) => self.tempo = t,
                    EventData::RationalTempo(n, d) => self.tempo *= n as f32 / d as f32,
                    EventData::End | EventData::Loop | EventData::StartGlide(_)
                        | EventData::EndGlide(_) | EventData::TickGlide(_) => (),
                    EventData::InterpolatedPitch(_)
                        | EventData::InterpolatedPressure(_)
                        | EventData::InterpolatedModulation(_)
                        => panic!("interpolated event in pattern"),
                }
            }

            if let Some((patch, note)) = active_note {
                let key = Key {
                    origin: KeyOrigin::Pattern,
                    channel: channel_i as u8,
                    key: 0,
                };
                let pitch = module.tuning.midi_pitch(&note);
                self.note_on(track_i, key, pitch, None, patch);
            }
        }
    }

    fn reinit_memory(&mut self, tick: u32, module: &Module) {
        for track in 0..module.tracks.len() {
            self.reinit_track_memory(tick, module, track);
        }
    }

    fn reinit_track_memory(&mut self, tick: u32, module: &Module, track_i: usize) {
        self.synths[track_i].reset_memory();

        for (channel_i, channel) in module.tracks[track_i].channels.iter().enumerate() {
            let mut events: Vec<_> = channel.events.iter()
                .filter(|e| e.tick < tick
                    && (VEL_COLUMN..=MOD_COLUMN).contains(&e.data.logical_column()))
                .collect();
            events.sort_by_key(|e| e.tick);

            for evt in events {
                match evt.data {
                    EventData::Pressure(v) => {
                        self.synths[track_i].set_vel_memory(
                            channel_i as u8, v as f32 / EventData::DIGIT_MAX as f32);
                    }
                    EventData::Modulation(v) => {
                        self.synths[track_i].set_mod_memory(
                            channel_i as u8, v as f32 / EventData::DIGIT_MAX as f32);
                    }
                    _ => ()
                }
            }
        }
    }

    pub fn toggle_mute(&mut self, module: &Module, track_i: usize) {
        if track_i == 0 {
            return // never mute keyjazz track
        }

        let synth = &mut self.synths[track_i];
        synth.muted = !synth.muted;

        if synth.muted {
            synth.clear_all_notes(&mut self.seq);
        } else if self.playing {
            self.simulate_track_events(self.tick, module, track_i);
        }
    }

    pub fn toggle_solo(&mut self, module: &Module, track_i: usize) {
        let soloed = self.synths.iter().enumerate()
            .all(|(i, x)| i == 0 || x.muted == (i != track_i));

        let toggle_indices: Vec<_> = self.synths.iter().enumerate()
            .filter(|(i, x)| (*i == track_i && x.muted)
                || (*i != track_i && x.muted == soloed))
            .map(|(i, _)| i)
            .collect();

        for i in toggle_indices {
            self.toggle_mute(module, i);
        }
    }

    pub fn unmute_all(&mut self, module: &Module) {
        let toggle_indices: Vec<_> = self.synths.iter().enumerate()
            .filter(|(_, x)| x.muted)
            .map(|(i, _)| i)
            .collect();

        for i in toggle_indices {
            self.toggle_mute(module, i);
        }
    }

    pub fn track_muted(&self, i: usize) -> bool {
        self.synths[i].muted
    }

    fn handle_event(&mut self, event: &Event, module: &Module,
        track: usize, channel: usize
    ) {
        let key = Key {
            origin: KeyOrigin::Pattern,
            channel: channel as u8,
            key: 0,
        };

        match event.data {
            EventData::Pitch(note) => {
                if let Some((patch, note)) = module.map_note(note, track) {
                    let pitch = module.tuning.midi_pitch(&note);
                    let channel = &module.tracks[track].channels[channel];
                    if channel.is_interpolated(NOTE_COLUMN, event.tick) {
                        self.bend_to(track, key, pitch);
                    } else {
                        self.note_on(track, key, pitch, None, patch);
                    }
                }
            }
            EventData::Pressure(v) => {
                self.channel_pressure(track, channel as u8,
                    v as f32 / EventData::DIGIT_MAX as f32);
            }
            EventData::Modulation(v) => {
                self.modulate(track, channel as u8,
                    v as f32 / EventData::DIGIT_MAX as f32);
            }
            EventData::NoteOff => {
                self.note_off(track, key);
            }
            EventData::Tempo(t) => self.tempo = t,
            EventData::RationalTempo(n, d) => self.tempo *= n as f32 / d as f32,
            EventData::End => if let Some(tick) = module.find_loop_start(self.tick) {
                self.tick = tick;
                self.reinit_memory(tick, module);
                self.looped = true;
            } else {
                self.stop();
            },
            EventData::Loop | EventData::StartGlide(_) | EventData::EndGlide(_)
                | EventData::TickGlide(_) => (),
            EventData::InterpolatedPitch(pitch) => self.bend_to(track, key, pitch),
            EventData::InterpolatedPressure(v) =>
                self.channel_pressure(track, channel as u8, v),
            EventData::InterpolatedModulation(v) =>
                self.modulate(track, channel as u8, v),
        }
    }
}

fn interval_ticks(dt: f64, tempo: f32) -> u32 {
    (dt * tempo as f64 / 60.0 * TICKS_PER_BEAT as f64).round() as u32
}

pub fn tick_interval(dtick: u32, tempo: f32) -> f64 {
    dtick as f64 / tempo as f64 * 60.0 / TICKS_PER_BEAT as f64
}

pub enum RenderUpdate {
    Progress(f64),
    Done(Wave, PathBuf),
}

/// Renders module to PCM. Loops forever if module is missing END!
pub fn render(module: Module, path: PathBuf) -> Receiver<RenderUpdate> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {let sample_rate = 44100;
        let mut wave = Wave::new(2, sample_rate as f64);
        let mut seq = Sequencer::new(false, 4);
        seq.set_sample_rate(sample_rate as f64);
        let mut fx = GlobalFX::new(seq.backend(), &module.fx);
        let fadeout_gain = shared(1.0);
        fx.net = fx.net * (var(&fadeout_gain) | var(&fadeout_gain));
        fx.net.set_sample_rate(sample_rate as f64);
        let mut player = Player::new(seq, module.tracks.len(), sample_rate as f32);
        let mut backend = BlockRateAdapter::new(Box::new(fx.net.backend()));
        let block_size = 64;
        let dt = block_size as f64 / sample_rate as f64;
        let mut playtime = 0.0;
        let mut time_since_loop = 0.0;
        let render_time = if module.loops() {
            module.playtime() + LOOP_FADEOUT_TIME as f64
        } else {
            module.playtime()
        };

        // TODO: render would probably be faster if we called player.frame() only
        //       when there's a new event. benchmark this
        player.play();
        while player.playing && time_since_loop < LOOP_FADEOUT_TIME {
            player.frame(&module, dt);
            playtime += dt;
            for _ in 0..block_size {
                wave.push(backend.get_stereo());
            }
            if player.looped {
                fadeout_gain.set(1.0 - (time_since_loop / LOOP_FADEOUT_TIME) as f32);
                time_since_loop += dt;
            }
            if let Err(e) = tx.send(RenderUpdate::Progress(playtime / render_time)) {
                eprintln!("{}", e);
            }
        }

        if let Err(e) = tx.send(RenderUpdate::Done(wave, path)) {
            eprintln!("{}", e);
        }
    });

    rx
}

fn interpolate_events(prev: Option<&EventData>, next: Option<&Event>,
    start: u32, tick: u32, module: &Module
) -> Option<EventData> {
    if let Some(next) = next {
        let t = (tick - start) as f32 / (next.tick - start) as f32;

        match next.data {
            EventData::Pitch(b) => {
                if let Some(EventData::Pitch(a)) = prev {
                    let a = module.tuning.midi_pitch(a);
                    let b = module.tuning.midi_pitch(&b);
                    Some(EventData::InterpolatedPitch(lerp(a, b, t)))
                } else {
                    None
                }
            }
            EventData::Tempo(b) => {
                let a = match prev {
                    Some(EventData::Tempo(a)) => *a,
                    Some(EventData::RationalTempo(..)) => module.tempo_at(start),
                    _ => DEFAULT_TEMPO,
                };
                Some(EventData::Tempo(lerp(a, b, t)))
            }
            EventData::Pressure(b) => {
                let a = if let Some(EventData::Pressure(a)) = prev {
                    *a as f32 / EventData::DIGIT_MAX as f32
                } else {
                    DEFAULT_PRESSURE
                };
                let b = b as f32 / EventData::DIGIT_MAX as f32;
                Some(EventData::InterpolatedPressure(lerp(a, b, t)))
            }
            EventData::Modulation(b) => {
                let a = if let Some(EventData::Modulation(a)) = prev {
                    *a as f32 / EventData::DIGIT_MAX as f32
                } else {
                    0.0
                };
                let b = b as f32 / EventData::DIGIT_MAX as f32;
                Some(EventData::InterpolatedModulation(lerp(a, b, t)))
            }
            _ => None,
        }
    } else {
        None
    }
}