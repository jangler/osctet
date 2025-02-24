use std::{path::PathBuf, sync::{mpsc::{self, Receiver}, Arc, Mutex}, thread};

use fundsp::hacker32::*;
use rtrb::Producer;
use triple_buffer::Output;

use crate::{fx::GlobalFX, module::{Event, EventData, LocatedEvent, Module, TrackEdit, GLOBAL_COLUMN, MOD_COLUMN, NOTE_COLUMN, VEL_COLUMN}, synth::{Key, KeyOrigin, Patch, Synth, DEFAULT_PRESSURE}, timespan::Timespan};

pub const DEFAULT_TEMPO: f32 = 120.0;

/// For rendering.
const LOOP_FADEOUT_TIME: f64 = 10.0;

/// Information for the UI thread sent from the audio thread.
#[derive(Clone)]
pub struct PlayerState {
    pub playing: bool,
    pub beat: f64,
    pub buffer_size: usize,
    pub tracks_muted: Vec<bool>,
}

impl PlayerState {
    pub fn get_tick(&self) -> Timespan {
        Timespan::approximate(self.beat)
    }
}

/// Information for the audio thread sent from the UI thread.
pub enum PlayerCommand {
    PlayFrom(Timespan),
    Stop,
    Reinitialize,
    Panic,
    ClearNotesWithOrigin(KeyOrigin),
    NoteOff {
        track: usize,
        key: Key,
    },
    UpdateSynths(Vec<TrackEdit>),
    ToggleMute(usize),
    ToggleSolo(usize),
    UnmuteAll,
    NoteOn {
        track: usize,
        key: Key,
        pitch: f32,
        pressure: Option<f32>,
        patch: usize,
    },
    ResetMemory,
    PolyPressure {
        track: usize,
        key: Key,
        pressure: f32,
    },
    Modulate {
        track: usize,
        channel: u8,
        value: f32,
    },
    ChannelPressure {
        track: usize,
        channel: u8,
        pressure: f32,
    },
    PitchBend {
        track: usize,
        channel: u8,
        semitones: f32,
    }
}

/// Imitation of the Player API for the UI thread.
pub struct PlayerShell {
    state_output: Output<PlayerState>,
    cmd_producer: Producer<PlayerCommand>,
    state: PlayerState,
}

impl PlayerShell {
    pub fn new(state_output: Output<PlayerState>, cmd_producer: Producer<PlayerCommand>
    ) -> Self {
        let mut state_output = state_output;
        Self {
            state: state_output.read().clone(),
            state_output,
            cmd_producer,
        }
    }

    /// Update cached state.
    pub fn update(&mut self) {
        self.state = self.state_output.read().clone();
    }

    fn cmd(&mut self, cmd: PlayerCommand) {
        if let Err(e) = self.cmd_producer.push(cmd) {
            eprintln!("error pushing player command: {e}");
        }
    }

    pub fn clear_notes_with_origin(&mut self, origin: KeyOrigin) {
        self.cmd(PlayerCommand::ClearNotesWithOrigin(origin))
    }

    pub fn is_playing(&self) -> bool {
        self.state.playing
    }

    pub fn get_tick(&self) -> Timespan {
        Timespan::approximate(self.state.beat)
    }

    pub fn stop(&mut self) {
        self.cmd(PlayerCommand::Stop)
    }

    pub fn reinit(&mut self) {
        // state override here fixes issue when loading module while song is
        // playing and scroll lock is on
        self.state.playing = false;
        self.cmd(PlayerCommand::Reinitialize)
    }

    pub fn note_off(&mut self, track: usize, key: Key) {
        self.cmd(PlayerCommand::NoteOff { track, key })
    }

    pub fn toggle_play_from(&mut self, tick: Timespan) {
        self.cmd(PlayerCommand::PlayFrom(tick))
    }

    pub fn update_synths(&mut self, edits: Vec<TrackEdit>) {
        self.cmd(PlayerCommand::UpdateSynths(edits))
    }

    pub fn panic(&mut self) {
        self.cmd(PlayerCommand::Panic)
    }

    pub fn toggle_mute(&mut self, track: usize) {
        self.cmd(PlayerCommand::ToggleMute(track))
    }

    pub fn toggle_solo(&mut self, track: usize) {
        self.cmd(PlayerCommand::ToggleSolo(track))
    }

    pub fn unmute_all(&mut self) {
        self.cmd(PlayerCommand::UnmuteAll)
    }

    pub fn track_muted(&mut self, track: usize) -> bool {
        self.state.tracks_muted.get(track).cloned().unwrap_or_default()
    }

    pub fn note_on(&mut self, track: usize, key: Key, pitch: f32, pressure: Option<f32>,
        patch: usize
    ) {
        self.cmd(PlayerCommand::NoteOn { track, key, pitch, pressure, patch })
    }

    pub fn reset_memory(&mut self) {
        self.cmd(PlayerCommand::ResetMemory)
    }

    pub fn buffer_size(&self) -> usize {
        self.state.buffer_size
    }

    pub fn poly_pressure(&mut self, track: usize, key: Key, pressure: f32) {
        self.cmd(PlayerCommand::PolyPressure { track, key, pressure })
    }

    pub fn modulate(&mut self, track: usize, channel: u8, value: f32) {
        self.cmd(PlayerCommand::Modulate { track, channel, value })
    }

    pub fn channel_pressure(&mut self, track: usize, channel: u8, pressure: f32) {
        self.cmd(PlayerCommand::ChannelPressure { track, channel, pressure })
    }

    pub fn pitch_bend(&mut self, track: usize, channel: u8, semitones: f32) {
        self.cmd(PlayerCommand::PitchBend { track, channel, semitones })
    }
}

/// Handles module playback. In methods that take a `track` argument, 0 can
/// safely be used for keyjazz events (since track 0 will never sequence).
pub struct Player {
    seq: Sequencer,
    synths: Vec<Synth>, // one per track
    playing: bool,
    beat: f64,
    tempo: f32,
    looped: bool,
    metronome: bool,
    sample_rate: f32,
    pub stereo_width: Shared,
    pub buffer_size: usize,
}

impl Player {
    pub fn new(seq: Sequencer, num_tracks: usize, sample_rate: f32) -> Self {
        Self {
            seq,
            synths: (0..num_tracks).map(|_| Synth::new(sample_rate)).collect(),
            playing: false,
            beat: 0.0,
            tempo: DEFAULT_TEMPO,
            looped: false,
            metronome: false,
            sample_rate,
            stereo_width: shared(1.0),
            buffer_size: 0,
        }
    }

    pub fn state(&self) -> PlayerState {
        PlayerState {
            playing: self.playing,
            beat: self.beat,
            buffer_size: self.buffer_size,
            tracks_muted: self.synths.iter().map(|x| x.muted).collect(),
        }
    }

    pub fn handle_command(&mut self, cmd: PlayerCommand, module: &Module) {
        match cmd {
            PlayerCommand::PlayFrom(beat) => if self.playing {
                self.stop();
            } else {
                self.play_from(beat, module);
            },
            PlayerCommand::Stop => self.stop(),
            PlayerCommand::Reinitialize => self.reinit(module.tracks.len()),
            PlayerCommand::Panic => self.panic(),
            PlayerCommand::ClearNotesWithOrigin(origin) =>
                self.clear_notes_with_origin(origin),
            PlayerCommand::NoteOff { track, key } => self.note_off(track, key),
            PlayerCommand::UpdateSynths(edits) => self.update_synths(edits),
            PlayerCommand::ToggleMute(track) => self.toggle_mute(module, track),
            PlayerCommand::ToggleSolo(track) => self.toggle_solo(module, track),
            PlayerCommand::UnmuteAll => self.unmute_all(module),
            PlayerCommand::NoteOn { track, key, pitch, pressure, patch } =>
                match module.patches.get(patch) {
                    Some(patch) => self.note_on(track, key, pitch, pressure, patch),
                    None => eprintln!("patch index out of bounds"),
                },
            PlayerCommand::ResetMemory => self.reset_memory(),
            PlayerCommand::ChannelPressure { track, channel, pressure } =>
                self.channel_pressure(track, channel, pressure),
            PlayerCommand::Modulate { track, channel, value } =>
                self.modulate(track, channel, value),
            PlayerCommand::PitchBend { track, channel, semitones } =>
                self.pitch_bend(track, channel, semitones),
            PlayerCommand::PolyPressure { track, key, pressure } =>
                self.poly_pressure(track, key, pressure),
        }
    }

    /// Reinitialize state.
    pub fn reinit(&mut self, num_tracks: usize) {
        for synth in &mut self.synths {
            synth.clear_all_notes(&mut self.seq);
        }
        self.synths = (0..num_tracks).map(|_| Synth::new(self.sample_rate)).collect();
        self.playing = false;
        self.beat = 0.0;
        self.tempo = DEFAULT_TEMPO;
        self.looped = false;
        self.metronome = false;
    }

    /// Return the closest `Timespan` to the playhead.
    pub fn get_tick(&self) -> Timespan {
        Timespan::approximate(self.beat)
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
        self.looped = false;
    }

    pub fn play_from(&mut self, tick: Timespan, module: &Module) {
        self.simulate_events(tick, module);
        self.beat = tick.as_f64();
        self.play();
    }

    pub fn toggle_play_from(&mut self, tick: Timespan, module: &Module) {
        if self.playing {
            self.stop()
        } else {
            self.play_from(tick, module)
        }
    }

    /// Start playing at `tick` in record mode.
    pub fn record_from(&mut self, tick: Timespan, module: &Module) {
        self.metronome = true;
        self.play_from(tick, module);
    }

    /// Update synths for track edits.
    pub fn update_synths(&mut self, edits: Vec<TrackEdit>) {
        for edit in edits {
            match edit {
                TrackEdit::Insert(i) =>
                    self.synths.insert(i, Synth::new(self.sample_rate)),
                TrackEdit::Remove(i) => { self.synths.remove(i); }
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

    /// Handle a modulation event.
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

    /// Release all notes from a given source.
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

    /// Handle a frame of length `dt`.
    pub fn frame(&mut self, module: &Module, dt: f64) {
        if !self.playing {
            return
        }

        let prev_time = self.beat;
        self.beat += interval_beats(dt, self.tempo);
        let current_timespan = Timespan::approximate(self.beat);

        let mut events = Vec::new();

        for (track_i, track) in module.tracks.iter().enumerate() {
            for (channel_i, channel) in track.channels.iter().enumerate() {
                let mut prev_data = [None, None, None];
                let mut next_event = [None, None, None];
                let mut start_tick = [Timespan::ZERO, Timespan::ZERO, Timespan::ZERO];
                let mut glide = [false, false, false];

                for event in &channel.events {
                    let col = event.data.logical_column();
                    let t = event.tick.as_f64();

                    if t < self.beat {
                        if t >= prev_time {
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
                    } else if let Some(v) = next_event.get_mut(col as usize) {
                        if v.is_none() {
                            *v = Some(event);
                        }
                    }
                }

                for i in 0..prev_data.len() {
                    if glide[i] {
                        if let Some(data) = interpolate_events(
                            prev_data[i], next_event[i], start_tick[i],
                            self.beat as f32, module
                        ) {
                            events.push(LocatedEvent {
                                track: track_i,
                                channel: channel_i,
                                event: Event {
                                    tick: current_timespan,
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
            if let EventData::End = event.event.data {
                break
            }
        }

        if self.metronome && self.beat.ceil() != prev_time.ceil() {
            self.seq.push_relative(0.0, 0.01, Fade::Smooth, 0.01, 0.01,
                Box::new(square_hz(440.0 * 8.0) >> split::<U4>()));
        }
    }

    /// Update state as if the module had been played up to a given tick.
    fn simulate_events(&mut self, tick: Timespan, module: &Module) {
        self.tempo = DEFAULT_TEMPO;

        for track in 0..module.tracks.len() {
            self.simulate_track_events(tick, module, track);
        }
    }

    /// Update one track's state as if the module had been played up to `tick`.
    fn simulate_track_events(&mut self, tick: Timespan, module: &Module, track_i: usize) {
        self.synths[track_i].reset_memory();

        for (channel_i, channel) in module.tracks[track_i].channels.iter().enumerate() {
            let mut events: Vec<_> = channel.events.iter()
                .filter(|e| e.tick < tick)
                .collect();
            events.sort_by_key(|e| (e.tick, e.data.spatial_column()));

            let mut active_note = None;
            let mut bend_offset = 0;

            for evt in events {
                match evt.data {
                    EventData::Pitch(note) => {
                        if let Some((patch, note)) = module.map_note(note, track_i) {
                            if module.patches[patch].sustains() {
                                active_note = Some((patch, note));
                                bend_offset = 0;
                            }
                        }
                    }
                    EventData::Pressure(v) =>
                        self.channel_pressure(track_i, channel_i as u8,
                            v as f32 / EventData::DIGIT_MAX as f32),
                    EventData::Modulation(v) =>
                        self.modulate(track_i, channel_i as u8,
                            v as f32 / EventData::DIGIT_MAX as f32),
                    EventData::NoteOff => active_note = None,
                    EventData::Tempo(t) => self.tempo = t,
                    EventData::RationalTempo(n, d) => self.tempo *= n as f32 / d as f32,
                    EventData::End | EventData::Loop | EventData::StartGlide(_)
                        | EventData::EndGlide(_) | EventData::TickGlide(_)
                        | EventData::Section => (),
                    EventData::InterpolatedPitch(_)
                        | EventData::InterpolatedPressure(_)
                        | EventData::InterpolatedModulation(_)
                        => panic!("interpolated event in pattern"),
                    EventData::Bend(c) => bend_offset = c,
                }
            }

            if channel.events.iter()
                .any(|e| e.tick == tick && e.data == EventData::NoteOff) {
                active_note = None;
            }

            if let Some((patch, note)) = active_note {
                let key = Key {
                    origin: KeyOrigin::Pattern,
                    channel: channel_i as u8,
                    key: 0,
                };
                let pitch = module.tuning.midi_pitch(&note);
                self.note_on(track_i, key, pitch, None, &module.patches[patch]);
                self.pitch_bend(track_i, channel_i as u8, bend_offset as f32 / 100.0);
            }
        }
    }

    /// Reset vel/mod memory.
    pub fn reset_memory(&mut self) {
        for synth in &mut self.synths {
            synth.reset_memory();
        }
    }

    /// Reinitialize vel/mod memory (for looping).
    fn reinit_memory(&mut self, tick: Timespan, module: &Module) {
        for track in 0..module.tracks.len() {
            self.reinit_track_memory(tick, module, track);
        }
    }

    /// Reinitialize a track's vel/mod memory.
    fn reinit_track_memory(&mut self, tick: Timespan, module: &Module, track_i: usize) {
        self.synths[track_i].reset_memory();

        for (channel_i, channel) in module.tracks[track_i].channels.iter().enumerate() {
            let mut events: Vec<_> = channel.events.iter()
                .filter(|e| e.tick < tick
                    && (VEL_COLUMN..=MOD_COLUMN).contains(&e.data.logical_column()))
                .collect();
            events.sort_by_key(|e| e.tick);

            for evt in events {
                match evt.data {
                    EventData::Pressure(v) =>
                        self.synths[track_i].set_vel_memory(
                            channel_i as u8, v as f32 / EventData::DIGIT_MAX as f32),
                    EventData::Modulation(v) =>
                        self.synths[track_i].set_mod_memory(
                            channel_i as u8, v as f32 / EventData::DIGIT_MAX as f32),
                    _ => ()
                }
            }
        }
    }

    /// Mute/unmute a track.
    pub fn toggle_mute(&mut self, module: &Module, track_i: usize) {
        if track_i == 0 {
            return // never mute keyjazz track
        }

        let synth = &mut self.synths[track_i];
        synth.muted = !synth.muted;

        if synth.muted {
            synth.clear_all_notes(&mut self.seq);
        } else if self.playing {
            self.simulate_track_events(Timespan::approximate(self.beat), module, track_i);
        }
    }

    /// Solo/unsolo a track.
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

    /// Unmute all tracks.
    pub fn unmute_all(&mut self, module: &Module) {
        let toggle_indices: Vec<_> = self.synths.iter().enumerate()
            .filter(|(_, x)| x.muted)
            .map(|(i, _)| i)
            .collect();

        for i in toggle_indices {
            self.toggle_mute(module, i);
        }
    }

    /// Check whether a track is muted.
    pub fn track_muted(&self, i: usize) -> bool {
        self.synths[i].muted
    }

    /// Process a pattern event.
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
                        self.note_on(track, key, pitch, None, &module.patches[patch]);
                    }
                }
            }
            EventData::Pressure(v) =>
                self.channel_pressure(track, channel as u8,
                    v as f32 / EventData::DIGIT_MAX as f32),
            EventData::Modulation(v) =>
                self.modulate(track, channel as u8,
                    v as f32 / EventData::DIGIT_MAX as f32),
            EventData::NoteOff => self.note_off(track, key),
            EventData::Tempo(t) => self.tempo = t,
            EventData::RationalTempo(n, d) => {
                let channel = &module.tracks[track].channels[channel];
                if !channel.is_interpolated(GLOBAL_COLUMN, event.tick) {
                    self.tempo *= n as f32 / d as f32;
                }
            }
            EventData::End => if let Some(tick) = module.find_loop_start(self.beat) {
                self.beat = tick.as_f64();
                self.reinit_memory(tick, module);
                self.looped = true;
            } else {
                self.stop();
            },
            EventData::Loop | EventData::StartGlide(_) | EventData::EndGlide(_)
                | EventData::TickGlide(_) | EventData::Section => (),
            EventData::InterpolatedPitch(pitch) => self.bend_to(track, key, pitch),
            EventData::InterpolatedPressure(v) =>
                self.channel_pressure(track, channel as u8, v),
            EventData::InterpolatedModulation(v) =>
                self.modulate(track, channel as u8, v),
            EventData::Bend(c) => self.pitch_bend(track, channel as u8, c as f32 / 100.0),
        }
    }
}

/// Convert a time interval to beat-space.
fn interval_beats(dt: f64, tempo: f32) -> f64 {
    dt * tempo as f64 / 60.0
}

/// Convert a `Timespan` to a wall clock interval.
pub fn tick_interval(dtick: Timespan, tempo: f32) -> f64 {
    dtick.as_f64() / tempo as f64 * 60.0
}

/// Used to communicate between the render thread and main thread.
pub enum RenderUpdate {
    Progress(f64),
    Done(Wave, PathBuf),
}

/// Renders module to PCM. Loops forever if module is missing End!
/// If `track` is some, solo that track for rendering.
pub fn render(module: Arc<Module>, path: PathBuf, track: Option<usize>
) -> Receiver<RenderUpdate> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        const SAMPLE_RATE: f64 = 44100.0;
        const BLOCK_SIZE: i32 = 64;

        let mut wave = Wave::new(2, SAMPLE_RATE);
        let mut seq = Sequencer::new(false, 4);
        seq.set_sample_rate(SAMPLE_RATE);
        let mut fx = GlobalFX::new(seq.backend(), &module.fx);
        let fadeout_gain = shared(1.0);
        fx.net = fx.net * (var(&fadeout_gain) | var(&fadeout_gain));
        fx.net.set_sample_rate(SAMPLE_RATE);
        let mut player = Player::new(seq, module.tracks.len(), SAMPLE_RATE as f32);
        if let Some(track) = track {
            player.toggle_solo(&module, track);
        }
        let mut backend = BlockRateAdapter::new(Box::new(fx.net.backend()));
        let dt = BLOCK_SIZE as f64 / SAMPLE_RATE;
        let mut playtime = 0.0;
        let mut time_since_loop = 0.0;
        let render_time = if module.loops() {
            module.playtime() + LOOP_FADEOUT_TIME
        } else {
            module.playtime()
        };
        let mut prev_progress = 0.0;

        player.play();
        while player.playing && time_since_loop < LOOP_FADEOUT_TIME {
            player.frame(&module, dt);
            playtime += dt;
            for _ in 0..BLOCK_SIZE {
                wave.push(backend.get_stereo());
            }
            if player.looped {
                fadeout_gain.set(1.0 - (time_since_loop / LOOP_FADEOUT_TIME) as f32);
                time_since_loop += dt;
            }

            let progress = playtime / render_time;
            if progress - prev_progress >= 0.01 {
                prev_progress = progress;
                if let Err(e) = tx.send(RenderUpdate::Progress(progress)) {
                    eprintln!("{e}");
                }
            }
        }

        if let Err(e) = tx.send(RenderUpdate::Done(wave, path)) {
            eprintln!("{e}");
        }
    });

    rx
}

/// Renders each track to its own WAV file.
pub fn render_tracks(module: Arc<Module>, path: PathBuf) -> Receiver<RenderUpdate> {
    let (tx, rx) = mpsc::channel();
    let track_range = 1..module.tracks.len();
    let progress = Arc::new(Mutex::new(
        track_range.clone().map(|_| 0.0).collect::<Vec<_>>()
    ));

    for i in track_range {
        let path = path
            .with_file_name(format!("{}_{}",
                path.file_stem().and_then(|s| s.to_str()).unwrap_or_default(), i))
            .with_extension("wav");
        let track_rx = render(module.clone(), path, Some(i));
        let tx = tx.clone();
        let progress = progress.clone();

        thread::spawn(move || {
            for msg in track_rx {
                match msg {
                    RenderUpdate::Progress(f) => {
                        let mut progress = progress.lock().unwrap();
                        progress[i - 1] = f;
                        let total_progress = progress.iter().sum::<f64>()
                            / progress.len() as f64;
                        if let Err(e) = tx.send(RenderUpdate::Progress(total_progress)) {
                            eprintln!("{e}")
                        }
                    }
                    RenderUpdate::Done(..) => if let Err(e) = tx.send(msg) {
                        eprintln!("{e}")
                    }
                }
            }
        });
    }

    rx
}

/// Calculates interpolated event data.
fn interpolate_events(prev: Option<&EventData>, next: Option<&Event>,
    start: Timespan, time: f32, module: &Module
) -> Option<EventData> {
    if let Some(next) = next {
        let t = (time - start.as_f32()) / (next.tick.as_f32() - start.as_f32());

        match next.data {
            EventData::Pitch(b) => if let Some(EventData::Pitch(a)) = prev {
                let a = module.tuning.midi_pitch(a);
                let b = module.tuning.midi_pitch(&b);
                Some(EventData::InterpolatedPitch(lerp(a, b, t)))
            } else {
                None
            }
            EventData::Tempo(b) => {
                let a = match prev {
                    Some(EventData::Tempo(a)) => *a,
                    Some(EventData::RationalTempo(..)) => module.tempo_at(start),
                    _ => DEFAULT_TEMPO,
                };
                Some(EventData::Tempo(lerp(a, b, t)))
            }
            EventData::RationalTempo(n, d) => {
                let a = match prev {
                    Some(EventData::Tempo(a)) => *a,
                    Some(EventData::RationalTempo(..)) => module.tempo_at(start),
                    _ => DEFAULT_TEMPO,
                };
                let b = a * n as f32 / d as f32;
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