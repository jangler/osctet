//! Definitions for all stored module data except patches.

use std::{collections::HashSet, error::Error, fs, path::PathBuf};

use fundsp::math::{delerp, lerp};
use serde::{Deserialize, Serialize};

use crate::{fx::FXSettings, pitch::{Note, Tuning}, playback::DEFAULT_TEMPO, synth::{Patch, DEFAULT_PRESSURE}};

pub const TICKS_PER_BEAT: u32 = 120;

pub const GLOBAL_COLUMN: u8 = 0;
pub const NOTE_COLUMN: u8 = 0;
pub const VEL_COLUMN: u8 = 1;
pub const MOD_COLUMN: u8 = 2;

#[derive(Serialize, Deserialize)]
pub struct Module {
    pub title: String,
    pub author: String,
    pub tuning: Tuning,
    pub fx: FXSettings,
    pub kit: Vec<KitEntry>,
    pub patches: Vec<Patch>,
    pub tracks: Vec<Track>,

    // TODO: cap size of undo stack.
    //       could use https://crates.io/crates/deepsize?
    #[serde(skip)]
    undo_stack: Vec<Edit>,
    #[serde(skip)]
    redo_stack: Vec<Edit>,
    #[serde(skip)]
    track_history: Vec<TrackEdit>,
    
    #[serde(skip)]
    pub has_unsaved_changes: bool,
}

impl Module {
    pub fn new(fx: FXSettings) -> Module {
        Self {
            title: "".to_owned(),
            author: "".to_owned(),
            tuning: Tuning::divide(2.0, 12, 1).unwrap(),
            fx,
            kit: Vec::new(),
            patches: vec![Patch::new()],
            tracks: vec![
                Track::new(TrackTarget::Global),
                Track::new(TrackTarget::Kit),
                Track::new(TrackTarget::Patch(0)),
            ],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            track_history: Vec::new(),
            has_unsaved_changes: false,
        }
    }

    pub fn load(path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let input = fs::read(path)?;
        let mut module = rmp_serde::from_slice::<Self>(&input)?;
        module.init_pcm();
        Ok(module)
    }

    fn init_pcm(&mut self) {
        for patch in &mut self.patches {
            patch.init_pcm();
        }
    }

    pub fn save(&mut self, path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let contents = rmp_serde::to_vec(self)?;
        fs::write(path, contents)?; 
        self.has_unsaved_changes = false;
        Ok(())
    }

    pub fn map_input(&self,
        patch_index: Option<usize>, note: Note
    ) -> Option<(&Patch, Note)> {
        if let Some(index) = patch_index {
            self.patches.get(index).map(|x| (x, note))
        } else {
            self.get_kit_patch(note)
        }
    }

    fn get_kit_patch(&self, note: Note) -> Option<(&Patch, Note)> {
        self.kit.iter()
            .find(|x| x.input_note == note)
            .map(|x| (self.patches.get(x.patch_index).unwrap(), x.patch_note))
    }

    fn remove_patch(&mut self, index: usize) -> Patch {
        let patch = self.patches.remove(index);
        self.kit.retain(|x| x.patch_index != index);

        for entry in self.kit.iter_mut() {
            if entry.patch_index > index {
                entry.patch_index -= 1;
            }
        }

        for track in self.tracks.iter_mut() {
            match track.target {
                TrackTarget::Patch(i) if i == index =>
                    track.target = TrackTarget::None,
                TrackTarget::Patch(i) if i > index =>
                    track.target = TrackTarget::Patch(i - 1),
                _ => (),
            }
        }

        patch
    }

    /// Return copies of pattern events between two positions.
    pub fn scan_events(&self, start: Position, end: Position) -> Vec<LocatedEvent> {
        let tick_range = start.tick..=end.tick;
        let (start_tuple, end_tuple) = (start.x_tuple(), end.x_tuple());
        let mut events = Vec::new();

        for (track_i, track) in self.tracks.iter().enumerate() {
            for (channel_i, channel) in track.channels.iter().enumerate() {
                for evt in &channel.events {
                    let tuple = (track_i, channel_i, evt.data.spatial_column());
                    if tick_range.contains(&evt.tick)
                        && tuple >= start_tuple && tuple <= end_tuple {
                        events.push(LocatedEvent {
                            track: track_i,
                            channel: channel_i,
                            event: evt.clone(),
                        });
                    }
                }
            }
        }

        events
    }

    /// Return references to pattern events between two locations.
    pub fn modify_events(&mut self, start: Position, end: Position) -> Vec<&mut Event> {
        let tick_range = start.tick..=end.tick;
        let (start_tuple, end_tuple) = (start.x_tuple(), end.x_tuple());
        let mut events = Vec::new();

        for (track_i, track) in self.tracks.iter_mut().enumerate() {
            for (channel_i, channel) in track.channels.iter_mut().enumerate() {
                for evt in &mut channel.events {
                    let tuple = (track_i, channel_i, evt.data.spatial_column());
                    if tick_range.contains(&evt.tick)
                        && tuple >= start_tuple && tuple <= end_tuple {
                        events.push(evt);
                    }
                }
            }
        }

        events
    }

    pub fn modify_channels(&mut self, start: Position, end: Position) -> Vec<&mut Channel> {
        let (start_tuple, end_tuple) = (start.x_tuple(), end.x_tuple());
        let start_tuple = (start_tuple.0, start_tuple.1);
        let end_tuple = (end_tuple.0, end_tuple.1);
        let mut channels = Vec::new();
        
        for (track_i, track) in self.tracks.iter_mut().enumerate() {
            for (channel_i, channel) in track.channels.iter_mut().enumerate() {
                let tuple = (track_i, channel_i);
                if tuple >= start_tuple && tuple <= end_tuple {
                    channels.push(channel);
                }
            }
        }

        channels
    }

    pub fn event_at(&mut self, pos: Position) -> Option<&mut Event> {
        if let Some(track) = self.tracks.get_mut(pos.track) {
            if let Some(channel) = track.channels.get_mut(pos.channel) {
                return channel.events.iter_mut().find(|evt|
                    evt.tick == pos.tick && evt.data.logical_column() == pos.column)
            }
        }
        None
    }

    /// Delete pattern events between two positions.
    pub fn delete_events(&mut self, start: Position, end: Position) {
        let remove: Vec<_> = self.scan_events(start, end).iter()
            .map(|x| x.position())
            .collect();
        if !remove.is_empty() {
            self.push_edit(Edit::PatternData {
                remove,
                add: Vec::new(),
            });
        }
    }

    fn delete_event(&mut self, pos: Position) -> Option<Event> {
        let channel = &mut self.tracks[pos.track].channels[pos.channel];
        channel.events.iter()
            .position(|e| e.tick == pos.tick && e.data.logical_column() == pos.column)
            .map(|i| channel.events.remove(i))
    }

    pub fn map_note(&self, note: Note, track: usize) -> Option<(&Patch, Note)> {
        self.tracks.get(track).map(|track| {
            match track.target {
                TrackTarget::None | TrackTarget::Global => None,
                TrackTarget::Kit => self.get_kit_patch(note),
                TrackTarget::Patch(i) => self.patches.get(i).map(|x| (x, note)),
            }
        }).flatten()
    }

    pub fn add_track(&mut self) {
        let index = self.tracks.len();
        let track = Track::new(TrackTarget::Patch(0));
        self.push_edit(Edit::InsertTrack(index, track));
    }

    pub fn insert_event(&mut self, track: usize, channel: usize, event: Event) {
        self.push_edit(Edit::PatternData {
            remove: vec![Position {
                track,
                channel,
                tick: event.tick,
                column: event.data.logical_column()
            }],
            add: vec![LocatedEvent { track, channel, event }]
        });
    }

    pub fn shift_channel_events(&mut self, start: Position, end: Position, distance: i32) {
        let (x_start, x_end) = ((start.track, start.channel), (end.track, end.channel));
        let mut channels = Vec::new();
        for (track_i, track) in self.tracks.iter().enumerate() {
            for channel_i in 0..track.channels.len() {
                if (track_i, channel_i) >= x_start && (track_i, channel_i) <= x_end {
                    channels.push(ChannelCoords {
                        track: track_i as u8,
                        channel: channel_i as u8
                    });
                }
            }
        }

        self.push_edit(Edit::ShiftEvents {
            channels,
            start: start.tick,
            distance,
            insert: Vec::new(),
        });
    }
    
    /// Performs an edit operation and handles undo/redo stacks.
    pub fn push_edit(&mut self, edit: Edit) {
        // TODO: merge consecutive pattern data operations
        let edit = self.flip_edit(edit);
        self.undo_stack.push(edit);
        self.redo_stack.clear();
    }

    /// Performs an edit operation and returns its inverse.
    fn flip_edit(&mut self, edit: Edit) -> Edit {
        self.has_unsaved_changes = true;
        match edit {
            Edit::InsertTrack(index, track) => {
                self.tracks.insert(index, track);
                self.track_history.push(TrackEdit::Insert(index));
                Edit::RemoveTrack(self.tracks.len() - 1)
            }
            Edit::RemoveTrack(index) => {
                let track = self.tracks.remove(index);
                self.track_history.push(TrackEdit::Remove(index));
                Edit::InsertTrack(index, track)
            }
            Edit::RemapTrack(index, target) => {
                let target = std::mem::replace(&mut self.tracks[index].target, target);
                Edit::RemapTrack(index, target)
            }
            Edit::AddChannel(index, channel) => {
                let track = &mut self.tracks[index];
                track.channels.push(channel);
                Edit::RemoveChannel(index)
            }
            Edit::RemoveChannel(index) => {
                let track = &mut self.tracks[index];
                let channel = track.channels.pop().unwrap();
                Edit::AddChannel(index, channel)
            }
            Edit::PatternData { remove, add } => {
                let flip_add = remove.into_iter().flat_map(|p| {
                    self.delete_event(p).map(|event| LocatedEvent {
                        track: p.track,
                        channel: p.channel,
                        event,
                    })
                }).collect();
                let mut modified_channels = HashSet::new();
                let flip_remove = add.into_iter().map(|e| {
                    modified_channels.insert((e.track, e.channel));
                    let pos = e.position();
                    self.tracks[e.track].channels[e.channel].events.push(e.event);
                    pos
                }).collect();
                for (track, channel) in modified_channels {
                    self.tracks[track].channels[channel].sort_events();
                }
                Edit::PatternData { remove: flip_remove, add: flip_add }
            }
            Edit::InsertPatch(index, patch) => {
                self.patches.insert(index, patch);
                Edit::RemovePatch(index)
            }
            Edit::RemovePatch(index) => {
                let patch = self.remove_patch(index);
                Edit::InsertPatch(index, patch)
            }
            Edit::ShiftEvents { channels, start, distance, insert } => {
                // shift/delete events starting at selection
                let mut deleted = Vec::new();
                for coords in &channels {
                    deleted.extend(self.tracks[coords.track as usize]
                        .channels[coords.channel as usize]
                        .shift_events(start, distance)
                        .into_iter()
                        .map(|event| LocatedEvent {
                            track: coords.track as usize,
                            channel: coords.channel as usize,
                            event,
                        }));
                }

                // re-insert previously deleted events
                let mut modified_channels = HashSet::new();
                for e in insert {
                    modified_channels.insert((e.track, e.channel));
                    self.tracks[e.track].channels[e.channel].events.push(e.event);
                }
                for (track, channel) in modified_channels {
                    self.tracks[track].channels[channel].sort_events();
                }

                Edit::ShiftEvents {
                    channels,
                    start,
                    distance: -distance,
                    insert: deleted,
                }
            },
            Edit::ReplaceEvents(events) => {
                Edit::ReplaceEvents(events.into_iter().map(|event| {
                    self.replace_event(event)
                }).collect())
            },
        }
    }

    /// Replace an event in-place, returning the old value.
    pub fn replace_event(&mut self, new_evt: LocatedEvent) -> LocatedEvent {
        if let Some(old_evt) = self.event_at(new_evt.position()) {
            let ret = LocatedEvent {
                event: old_evt.clone(),
                ..new_evt
            };
            old_evt.data = new_evt.event.data;
            ret
        } else {
            new_evt.clone()
        }
    }

    /// Returns true if there was something to undo.
    pub fn undo(&mut self) -> bool {
        if let Some(edit) = self.undo_stack.pop() {
            let edit = self.flip_edit(edit);
            self.redo_stack.push(edit);
            true
        } else {
            false
        }
    }

    /// Returns true if there was something to redo.
    pub fn redo(&mut self) -> bool {
        if let Some(edit) = self.redo_stack.pop() {
            let edit = self.flip_edit(edit);
            self.undo_stack.push(edit);
            true
        } else {
            false
        }
    }

    pub fn drain_track_history(&mut self) -> Vec<TrackEdit> {
        self.track_history.drain(..).collect()
    }

    pub fn find_loop_start(&self, before_tick: u32) -> Option<u32> {
        self.tracks[0].channels.iter().flat_map(|c| {
            c.events.iter()
                .filter(|e| e.data == EventData::Loop && e.tick < before_tick)
                .map(|e| e.tick)
        }).max()
    }

    pub fn ends(&self) -> bool {
        for track in &self.tracks {
            for channel in &track.channels {
                for event in &channel.events {
                    if event.data == EventData::End {
                        return true
                    }
                }
            }
        }
        false
    }

    /// Return the number of channels between two positions.
    pub fn channels_between(&self, start: Position, end: Position) -> usize {
        let mut n = 0;
        let mut t = start.track;
        let mut c = start.channel;
        while t < end.track || c < end.channel {
            n += 1;
            c += 1;
            if c >= self.tracks[t].channels.len() {
                t += 1;
                c = 0;
            }
        }
        n
    }

    /// Return the tick value of the last event in the pattern.
    pub fn last_event_tick(&self) -> Option<u32> {
        self.tracks.iter().flat_map(|t| {
            t.channels.iter().flat_map(|c| {
                c.events.iter().map(|e| e.tick)
            })
        }).max()
    }

    /// Return the tempo at a given tick.
    pub fn tempo_at(&self, tick: u32) -> f32 {
        let mut events: Vec<_> = self.tracks[0].channels.iter()
            .flat_map(|c| c.events.iter().filter(|e| e.tick < tick))
            .collect();
        events.sort_by_key(|e| e.tick);

        let mut result = DEFAULT_TEMPO;
        for evt in events {
            match evt.data {
                EventData::Tempo(t) => result = t,
                EventData::RationalTempo(n, d) => result *= n as f32 / d as f32,
                _ => (),
            }
        }
        result
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct KitEntry {
    pub input_note: Note,
    pub patch_index: usize,
    pub patch_note: Note,
}

#[derive(Serialize, Deserialize)]
pub struct Track {
    pub target: TrackTarget,
    pub channels: Vec<Channel>,
    pub share_pressure: bool, // TODO
    pub share_modulation: bool, // TODO
}

impl Track {
    pub fn new(target: TrackTarget) -> Self {
        Self {
            target,
            channels: vec![Channel::new()],
            share_pressure: false,
            share_modulation: false,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum TrackTarget {
    None,
    Global,
    Kit,
    Patch(usize),
}

#[derive(Serialize, Deserialize)]
pub struct Channel {
    pub events: Vec<Event>,
}

impl Channel {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    /// Shifts events after `start` by `distance` ticks, returning deleted events.
    pub fn shift_events(&mut self, start: u32, distance: i32) -> Vec<Event> {
        let mut deleted = Vec::new();

        if distance < 0 {
            let (keep, pass) = std::mem::take(&mut self.events).into_iter()
                .partition(|e| e.tick < start
                    || e.tick >= (start as i32 - distance) as u32);
            self.events = keep;
            deleted = pass;
        }

        for event in self.events.iter_mut() {
            if event.tick >= start {
                event.tick = (event.tick as i32 + distance).max(0) as u32;
            }
        }

        self.sort_events();

        deleted
    }

    pub fn sort_events(&mut self) {
        self.events.sort_by_key(|e| (e.tick, e.data.spatial_column()));
    }

    pub fn interp_by_col(&self, col: u8) -> impl Iterator<Item = u32> + use<'_> {
        self.events.iter().filter_map(move |e| match e.data {
            EventData::ToggleInterpolation(i) if i == col => Some(e.tick),
            _ => None,
        })
    }

    pub fn is_interpolated(&self, col: u8, tick: u32) -> bool {
        let interp = self.interp_by_col(col);
        interp.filter(|x| *x < tick).count() % 2 == 1
    }

    /// Returns an interpolated value from the given parameters.
    fn interp_values(&self, col: u8, tick: u32, default_value: Option<f32>,
        filter_fn: impl Fn(&&Event) -> bool, extract_fn: impl Fn(&EventData) -> Option<f32>,
    ) -> Option<f32> {
        let interp: Vec<_> = self.interp_by_col(col).collect();
        if let Some(i) = interp.iter().position(|t| *t >= tick) {
            if i % 2 == 1 {
                let start = interp[i - 1];
                let end = interp[i];
                let events: Vec<_> = self.events.iter().filter(filter_fn).collect();
                if let Some(i) = events.iter().position(|e| e.tick >= tick) {
                    let next_event = events[i];
                    if next_event.tick <= end {
                        let end = end.min(next_event.tick);
                        let (prev, start) = if i > 0 {
                            let prev_event = events[i - 1];
                            let start = start.max(prev_event.tick);
                            (extract_fn(&prev_event.data), start)
                        } else {
                            (default_value, start)
                        };
                        if let Some(prev) = prev {
                            if let Some(next) = extract_fn(&next_event.data) {
                                let t = delerp(start as f32, end as f32, tick as f32);
                                return Some(lerp(prev, next, t))
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Returns the interpolated MIDI pitch at a given tick.
    pub fn interpolate_pitch(&self, tick: u32, tuning: &Tuning) -> Option<f32> {
        self.interp_values(NOTE_COLUMN, tick, None, |e| match e.data {
            EventData::Pitch(_) => true,
            _ => false,
        }, |data| match data {
            EventData::Pitch(note) => Some(tuning.midi_pitch(&note)),
            _ => None,
        })
    }

    /// Returns the interpolated pressure at a given tick.
    pub fn interpolate_pressure(&self, tick: u32) -> Option<f32> {
        self.interp_values(VEL_COLUMN, tick, Some(DEFAULT_PRESSURE), |e| match e.data {
            EventData::Pressure(_) => true,
            _ => false,
        }, |data| match data {
            EventData::Pressure(v) => Some(*v as f32 / EventData::DIGIT_MAX as f32),
            _ => None,
        })
    }

    /// Returns the interpolated modulation at a given tick.
    pub fn interpolate_modulation(&self, tick: u32) -> Option<f32> {
        self.interp_values(MOD_COLUMN, tick, Some(0.0), |e| match e.data {
            EventData::Modulation(_) => true,
            _ => false,
        }, |data| match data {
            EventData::Modulation(v) => Some(*v as f32 / EventData::DIGIT_MAX as f32),
            _ => None,
        })
    }

    /// Returns the interpolated tempo at a given tick.
    pub fn interpolate_tempo(&self, tick: u32) -> Option<f32> {
        self.interp_values(NOTE_COLUMN, tick, Some(DEFAULT_TEMPO), |e| match e.data {
            EventData::Tempo(_) => true,
            _ => false,
        }, |data| match data {
            EventData::Tempo(v) => Some(*v),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub tick: u32,
    pub data: EventData,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum EventData {
    Pitch(Note),
    NoteOff,
    Pressure(u8),
    Modulation(u8),
    Tempo(f32),
    RationalTempo(u8, u8),
    End,
    Loop,
    InterpolatedPitch(f32),
    InterpolatedPressure(f32),
    InterpolatedModulation(f32),
    ToggleInterpolation(u8), // column
}

impl EventData {
    pub const DIGIT_MAX: u8 = 0xf;
    pub const INTERP_COL_FLAG: u8 = 0x80;

    pub fn spatial_column(&self) -> u8 {
        self.logical_column() & !Self::INTERP_COL_FLAG
    }
    
    pub fn logical_column(&self) -> u8 {
        match *self {
            Self::Pressure(_) => VEL_COLUMN,
            Self::Modulation(_) => MOD_COLUMN,
            Self::ToggleInterpolation(col) => col | Self::INTERP_COL_FLAG,
            _ => NOTE_COLUMN,
        }
    }

    /// Returns true if the data goes in the control/global track.
    pub fn is_ctrl(&self) -> bool {
        // TODO: how to handle tempo interpolation?
        match *self {
            Self::Tempo(_) | Self::RationalTempo(_, _)
                | Self::End | Self::Loop => true,
            _ => false,
        }
    }
}

/// Defines a linked copy region.
#[derive(Serialize, Deserialize)]
pub struct Link {
    pub src_tick: u32,
    pub dst_tick: u32,
    pub duration: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub tick: u32,
    pub track: usize,
    pub channel: usize,
    pub column: u8,
}

impl Position {
    pub fn beat(&self) -> f32 {
        self.tick as f32 / TICKS_PER_BEAT as f32
    }

    /// Returns a tuple of horizontal indices for comparison.
    pub fn x_tuple(&self) -> (usize, usize, u8) {
        (self.track, self.channel, self.column)
    }

    /// Returns None if the position is out of range.
    pub fn add_channels(&self, channels: usize, tracks: &Vec<Track>) -> Option<Self> {
        let mut track = self.track;
        let mut channel = self.channel;
        for _ in 0..channels {
            channel += 1;
            if channel >= tracks[track].channels.len() {
                track += 1;
                channel = 0;
            }
            if track >= tracks.len() {
                return None
            }
        }
        Some(Self {
            track,
            channel,
            ..*self
        })
    }
}

/// An operation that changes Module data.
pub enum Edit {
    InsertTrack(usize, Track),
    RemoveTrack(usize),
    RemapTrack(usize, TrackTarget),
    AddChannel(usize, Channel),
    RemoveChannel(usize),
    PatternData {
        remove: Vec<Position>,
        add: Vec<LocatedEvent>,
    },
    // TODO: redoing patch removal doesn't revert track/kit mappings
    InsertPatch(usize, Patch),
    RemovePatch(usize),
    ShiftEvents {
        channels: Vec<ChannelCoords>,
        start: u32,
        distance: i32,
        insert: Vec<LocatedEvent>,
    },
    ReplaceEvents(Vec<LocatedEvent>),
}

pub struct ChannelCoords {
    track: u8,
    channel: u8,
}

/// Used to track added/removed Tracks for synchronizing Player with Module.
pub enum TrackEdit {
    Insert(usize),
    Remove(usize),
}

/// Event with global location data, for the undo stack.
#[derive(Clone, Debug)]
pub struct LocatedEvent {
    pub track: usize,
    pub channel: usize,
    pub event: Event,
}

impl LocatedEvent {
    pub fn from_position(pos: Position, data: EventData) -> Self {
        Self {
            track: pos.track,
            channel: pos.channel,
            event: Event {
                tick: pos.tick,
                data,
            }
        }
    }

    /// Returns the position of the event.
    pub fn position(&self) -> Position {
        Position {
            tick: self.event.tick,
            track: self.track,
            channel: self.channel,
            column: self.event.data.logical_column(),
        }
    }
}