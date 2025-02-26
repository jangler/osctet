//! Definitions for most stored module data.

use std::{collections::HashSet, error::Error, fs::File, io::{BufReader, Read, Write}, path::PathBuf};

use flate2::{bufread::GzDecoder, write::GzEncoder};
use rmp_serde::{config::BytesMode, Serializer};
use rtrb::Producer;
use serde::{Deserialize, Serialize};

use crate::{fx::FXSettings, pitch::{Note, Tuning}, playback::{tick_interval, DEFAULT_TEMPO}, synth::Patch, timespan::Timespan};

pub const GLOBAL_COLUMN: u8 = 0;
pub const NOTE_COLUMN: u8 = 0;
pub const VEL_COLUMN: u8 = 1;
pub const MOD_COLUMN: u8 = 2;

/// Stores all saved song data and undo state.
#[derive(Clone, Serialize, Deserialize)]
pub struct Module {
    pub title: String,
    pub author: String,
    pub tuning: Tuning,
    pub fx: FXSettings,
    pub kit: Vec<KitEntry>,
    pub patches: Vec<Patch>,
    pub tracks: Vec<Track>,
    /// This field is just for save/load. See `PatternEditor` for actual usage.
    #[serde(default = "default_division")]
    pub division: u8,

    #[serde(skip)]
    undo_stack: Vec<Edit>,
    #[serde(skip)]
    redo_stack: Vec<Edit>,
    #[serde(skip)]
    track_history: Vec<TrackEdit>,
    #[serde(skip)]
    pub has_unsaved_changes: bool,
    #[serde(skip)]
    sync_stack: Vec<Edit>,
    #[serde(skip)]
    pub sync: bool,
}

/// Default beat division for serde.
fn default_division() -> u8 { 4 }

impl Module {
    pub fn new(fx: FXSettings) -> Module {
        Self {
            title: "".to_owned(),
            author: "".to_owned(),
            tuning: Tuning::divide(2.0, 12, 1)
                .expect("12-ET should be a valid tuning"),
            fx,
            kit: Vec::new(),
            patches: vec![Patch::new(String::from("Init"))],
            tracks: vec![
                Track::new(TrackTarget::Global),
                Track::new(TrackTarget::Kit),
                Track::new(TrackTarget::Patch(0)),
            ],
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            track_history: Vec::new(),
            has_unsaved_changes: false,
            division: default_division(),
            sync_stack: Vec::new(),
            sync: false,
        }
    }

    /// Load a module from `path`.
    pub fn load(path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let file = File::open(path)?;
        let mut input = Vec::new();
        GzDecoder::new(BufReader::new(file)).read_to_end(&mut input)?;
        let mut module = rmp_serde::from_slice::<Self>(&input)?;
        module.init_patches();
        Ok(module)
    }

    /// Initialize deserialized patches.
    fn init_patches(&mut self) {
        for patch in &mut self.patches {
            patch.init();
        }
    }

    /// Save the module to `path`. `division` is passed because the pattern
    /// editor stores the working beat division, not the module.
    pub fn save(&mut self, division: u8, path: &PathBuf) -> Result<(), Box<dyn Error>> {
        self.division = division;
        let mut contents = Vec::new();
        let mut ser = Serializer::new(&mut contents)
            .with_bytes(BytesMode::ForceIterables);
        self.serialize(&mut ser)?;
        let file = File::create(path)?;
        GzEncoder::new(file, Default::default()).write_all(&contents)?;
        self.has_unsaved_changes = false;
        Ok(())
    }

    /// Map a patch index and note to a patch and note, accounting for kit
    /// mappings.
    pub fn map_input(&self,
        patch_index: Option<usize>, note: Note
    ) -> Option<(usize, Note)> {
        if let Some(index) = patch_index {
            Some((index, note))
        } else {
            self.get_kit_patch(note)
        }
    }

    /// Returns the kit patch that `note` maps to, if any.
    fn get_kit_patch(&self, note: Note) -> Option<(usize, Note)> {
        self.kit.iter()
            .find(|x| x.input_note == note)
            .map(|x| (x.patch_index, x.patch_note))
    }

    /// Remove the patch at `index`.
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
    /// The end tick is exclusive unless start and end ticks are equal.
    pub fn scan_events(&self, start: Position, end: Position) -> Vec<LocatedEvent> {
        let tick_range = start.tick..end.tick;
        let (start_tuple, end_tuple) = (start.x_tuple(), end.x_tuple());
        let mut events = Vec::new();

        for (track_i, track) in self.tracks.iter().enumerate() {
            for (channel_i, channel) in track.channels.iter().enumerate() {
                for evt in &channel.events {
                    let tuple = (track_i, channel_i, evt.data.spatial_column());
                    if (tick_range.contains(&evt.tick) || evt.tick == start.tick)
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

    /// Returns the pattern event at `pos`.
    pub fn event_at(&mut self, pos: &Position) -> Option<&mut Event> {
        if let Some(track) = self.tracks.get_mut(pos.track) {
            if let Some(channel) = track.channels.get_mut(pos.channel) {
                return channel.events.iter_mut().find(|evt|
                    evt.tick == pos.tick && evt.data.logical_column() == pos.column)
            }
        }
        None
    }

    /// Push an edit that deletes pattern events between two positions.
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

    /// Delete the single pattern event at `pos`.
    fn delete_event(&mut self, pos: Position) -> Option<Event> {
        let channel = &mut self.tracks[pos.track].channels[pos.channel];
        channel.events.iter()
            .position(|e| e.tick == pos.tick && e.data.logical_column() == pos.column)
            .map(|i| channel.events.remove(i))
    }

    /// Maps a note based on track index.
    pub fn map_note(&self, note: Note, track: usize) -> Option<(usize, Note)> {
        self.tracks.get(track).and_then(|track| {
            match track.target {
                TrackTarget::None | TrackTarget::Global => None,
                TrackTarget::Kit => self.get_kit_patch(note),
                TrackTarget::Patch(i) => Some((i, note)),
            }
        })
    }

    /// Push an edit appending a new track.
    pub fn add_track(&mut self) {
        let index = self.tracks.len();
        let track = Track::new(TrackTarget::Patch(0));
        self.push_edit(Edit::InsertTrack(index, track));
    }

    /// Push an edit inserting an event.
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

    /// Push an edit shifting events forward or backward.
    pub fn shift_channel_events(&mut self,
        start: Position, end: Position, distance: Timespan
    ) {
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

    /// Performs an edit operation and updates undo/redo stacks.
    pub fn push_edit(&mut self, edit: Edit) {
        let edit = self.flip_edit(edit);
        self.undo_stack.push(edit);
        self.redo_stack.clear();
    }

    /// Performs an edit operation and returns its inverse.
    fn flip_edit(&mut self, edit: Edit) -> Edit {
        if self.sync {
            self.sync_stack.push(edit.clone());
        }
        self.has_unsaved_changes = true;
        match edit {
            Edit::InsertTrack(index, track) => {
                self.tracks.insert(index, track);
                self.track_history.push(TrackEdit::Insert(index));
                Edit::RemoveTrack(index)
            }
            Edit::RemoveTrack(index) => {
                let track = self.tracks.remove(index);
                self.track_history.push(TrackEdit::Remove(index));
                Edit::InsertTrack(index, track)
            }
            Edit::ShiftTrack(index, offset) => {
                // this could be implemented with insert + remove, but that
                // means multiple undo items and more memory usage
                let dst = index.saturating_add_signed(offset);
                let track = self.tracks.remove(index);
                self.tracks.insert(dst, track);
                self.track_history.push(TrackEdit::Remove(index));
                self.track_history.push(TrackEdit::Insert(dst));
                Edit::ShiftTrack(dst, -offset)
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
                let channel = track.channels.pop()
                    .expect("removed channel index should be valid");
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
        if let Some(old_evt) = self.event_at(&new_evt.position()) {
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

    /// Returns track insertions & removals made since the last call.
    pub fn drain_track_history(&mut self) -> Vec<TrackEdit> {
        self.track_history.drain(..).collect()
    }

    /// Returns the last loop event before beat count `before_time`.
    pub fn find_loop_start(&self, before_time: f64) -> Option<Timespan> {
        self.tracks[0].channels.iter().flat_map(|c| {
            c.events.iter()
                .filter(|e| e.data == EventData::Loop && e.tick.as_f64() < before_time)
                .map(|e| e.tick)
        }).max()
    }

    /// Returns true if the module has an End event.
    pub fn ends(&self) -> bool {
        self.tracks[0].channels.iter().any(|c|
            c.events.iter().any(|e| e.data == EventData::End)
        )
    }

    /// Return all events in the global channel, in sorted order.
    fn ctrl_events(&self) -> Vec<&Event> {
        let mut events: Vec<_> = self.tracks[0].channels.iter()
            .flat_map(|c| c.events.iter())
            .collect();
        events.sort_by_key(|e| e.tick);
        events
    }

    /// Returns true if the module loops.
    pub fn loops(&self) -> bool {
        for event in self.ctrl_events() {
            match event.data {
                EventData::End => return false,
                EventData::Loop => return true,
                _ => (),
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
    pub fn last_event_tick(&self) -> Option<Timespan> {
        self.tracks.iter().flat_map(|t| {
            t.channels.iter().flat_map(|c| {
                c.events.iter().map(|e| e.tick)
            })
        }).max()
    }

    /// Return the tempo at a given tick.
    pub fn tempo_at(&self, tick: Timespan) -> f32 {
        let mut result = DEFAULT_TEMPO;

        for evt in self.ctrl_events().iter().take_while(|e| e.tick <= tick) {
            match evt.data {
                EventData::Tempo(t) => result = t,
                EventData::RationalTempo(n, d) => result *= n as f32 / d as f32,
                _ => (),
            }
        }

        result
    }

    /// Returns the total playtime of the module in seconds.
    pub fn playtime(&self) -> f64 {
        let mut tick = Timespan::ZERO;
        let mut time = 0.0;
        let mut tempo = DEFAULT_TEMPO;

        for evt in self.ctrl_events() {
            match evt.data {
                EventData::Tempo(t) => {
                    time += tick_interval(evt.tick - tick, tempo);
                    tick = evt.tick;
                    tempo = t;
                }
                EventData::RationalTempo(n, d) => {
                    time += tick_interval(evt.tick - tick, tempo);
                    tick = evt.tick;
                    tempo *= n as f32 / d as f32;
                }
                EventData::End => {
                    return time + tick_interval(evt.tick - tick, tempo)
                }
                _ => (),
            }
        }

        if let Some(last_tick) = self.last_event_tick() {
            time += tick_interval(last_tick - tick, tempo)
        }

        time
    }

    pub fn handle_command(&mut self, cmd: ModuleCommand) {
        match cmd {
            ModuleCommand::FX(fx) => self.fx = fx,
            ModuleCommand::Kit(kit) => self.kit = kit,
            ModuleCommand::Load(module) => *self = module,
            ModuleCommand::Tuning(tuning) => self.tuning = tuning,
            ModuleCommand::Edit(edit) => { self.flip_edit(edit); }
            ModuleCommand::Patch(index, patch) => self.patches[index] = patch,
        }
    }

    /// Returns edits that have been made since the last call.
    pub fn sync_edits(&mut self) -> Vec<Edit> {
        std::mem::take(&mut self.sync_stack)
    }

    pub fn shared_clone(&self) -> Self {
        let mut m = self.clone();
        m.patches = self.patches.iter().map(|x| x.shared_clone()).collect();
        m
    }
}

/// Kit mapping.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct KitEntry {
    pub input_note: Note,
    pub patch_index: usize,
    pub patch_note: Note,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Track {
    pub target: TrackTarget,
    pub channels: Vec<Channel>,
}

impl Track {
    pub fn new(target: TrackTarget) -> Self {
        Self {
            target,
            channels: vec![Channel::default()],
        }
    }
}

/// Track "output" mapping.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum TrackTarget {
    None,
    Global,
    Kit,
    Patch(usize),
}

/// Contains an event sequence. Is a struct for legacy reasons.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Channel {
    pub events: Vec<Event>,
}

impl Channel {
    /// Shifts events after `start` by `distance` ticks, returning deleted events.
    pub fn shift_events(&mut self, start: Timespan, distance: Timespan) -> Vec<Event> {
        let deleted = if distance < Timespan::ZERO {
            let (keep, pass) = std::mem::take(&mut self.events).into_iter()
                .partition(|e| e.tick < start || e.tick >= start - distance);
            self.events = keep;
            pass
        } else {
            Vec::new()
        };

        for event in self.events.iter_mut() {
            if event.tick >= start {
                event.tick = (event.tick + distance).max(Timespan::ZERO);
            }
        }

        deleted
    }

    /// Sort events in the channel by their position. Some functions expect
    /// channel data to be sorted.
    pub fn sort_events(&mut self) {
        self.events.sort_by_key(|e| (e.tick, e.data.spatial_column()));
    }

    /// Return interpolation events in a (spatial) column.
    pub fn interp_by_col(&self, col: u8) -> impl Iterator<Item = &Event> + use<'_> {
        self.events.iter().filter(move |e| matches!(e.data,
            EventData::StartGlide(i)
            | EventData::EndGlide(i)
            | EventData::TickGlide(i) if i == col))
    }

    /// Returns true if the (spatial) column is interpolated at `tick`.
    pub fn is_interpolated(&self, col: u8, tick: Timespan) -> bool {
        let mut glide = false;

        for event in self.interp_by_col(col).take_while(|e| e.tick <= tick) {
            match event.data {
                EventData::StartGlide(_) => if event.tick < tick {
                    glide = true
                }
                EventData::EndGlide(_) => if event.tick < tick {
                    glide = false
                }
                EventData::TickGlide(_) => if event.tick == tick {
                    return true
                }
                _ => panic!("expected glide event"),
            }
        }

        glide
    }

    /// Returns the last event before `tick` in `column`.
    pub fn prev_event(&self, column: u8, tick: Timespan) -> Option<&Event> {
        self.events.iter()
            .filter(|e| e.tick < tick && e.data.logical_column() == column)
            .last()
    }
}

/// Channel event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub tick: Timespan,
    pub data: EventData,
}

/// Types of pattern event data.
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
    StartGlide(u8),
    EndGlide(u8),
    TickGlide(u8),
    /// MIDI-style pitch bend. Data is cent offset from starting note.
    Bend(i16),
    /// Section marker. No effect on playback.
    Section,
}

impl EventData {
    /// Maximum value in a digit column.
    pub const DIGIT_MAX: u8 = 0xf;

    /// Binary or'ed with "spatial column" value.
    pub const INTERP_COL_FLAG: u8 = 0x80;

    /// Convert a 7-bit MIDI value to a digit value.
    pub fn digit_from_midi(midi_value: u8) -> u8 {
        (midi_value as f32 * Self::DIGIT_MAX as f32 / 127.0).round() as u8
    }

    /// Returns the column where the event should be drawn.
    pub fn spatial_column(&self) -> u8 {
        self.logical_column() & !Self::INTERP_COL_FLAG
    }

    /// Returns a logical column value. Used to distinguish
    /// interpolation from "normal" events.
    pub fn logical_column(&self) -> u8 {
        match *self {
            Self::Pressure(_) => VEL_COLUMN,
            Self::Modulation(_) => MOD_COLUMN,
            Self::StartGlide(col) | Self::EndGlide(col) | Self::TickGlide(col)
                => col | Self::INTERP_COL_FLAG,
            _ => NOTE_COLUMN,
        }
    }

    /// Returns true if the data belongs in the given track index.
    pub fn goes_in_track(&self, track: usize) -> bool {
        match self {
            Self::Bend(_) | Self::Pressure(_) | Self::Modulation(_)
                | Self::NoteOff | Self::Pitch(_) => track != 0,
            Self::Tempo(_) | Self::RationalTempo(_, _)
                | Self::End | Self::Loop | Self::Section => track == 0,
            Self::StartGlide(col) | Self::EndGlide(col) | Self::TickGlide(col)
                => track != 0 || *col == GLOBAL_COLUMN,
            Self::InterpolatedModulation(_) | Self::InterpolatedPitch(_)
                | Self::InterpolatedPressure(_) => false, // never in pattern
        }
    }
}

/// Pattern position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub tick: Timespan,
    pub track: usize,
    pub channel: usize,
    /// Logical column, not spatial column.
    pub column: u8,
}

impl Position {
    pub fn new(tick: Timespan, track: usize, channel: usize, column: u8) -> Self {
        Self { tick, track, channel, column }
    }

    /// Returns the position's beat as a zero-indexed float.
    pub fn beat(&self) -> f32 {
        self.tick.as_f32()
    }

    /// Returns a tuple of horizontal indices for comparison.
    pub fn x_tuple(&self) -> (usize, usize, u8) {
        (self.track, self.channel, self.column)
    }

    /// Recalculate the position, given an offset in channels.
    /// Returns None if the position is out of range.
    pub fn add_channels(&self, channels: usize, tracks: &[Track]) -> Option<Self> {
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

/// An operation that changes `Module` data. Used for undo/redo.
#[derive(Clone)]
pub enum Edit {
    InsertTrack(usize, Track),
    RemoveTrack(usize),
    ShiftTrack(usize, isize),
    RemapTrack(usize, TrackTarget),
    AddChannel(usize, Channel),
    RemoveChannel(usize),
    PatternData {
        remove: Vec<Position>,
        add: Vec<LocatedEvent>,
    },
    InsertPatch(usize, Patch),
    RemovePatch(usize),
    ShiftEvents {
        channels: Vec<ChannelCoords>,
        start: Timespan,
        distance: Timespan,
        insert: Vec<LocatedEvent>,
    },
    ReplaceEvents(Vec<LocatedEvent>),
}

/// Position of a channel.
#[derive(Clone)]
pub struct ChannelCoords {
    track: u8,
    channel: u8,
}

/// Used to track added/removed Tracks for synchronizing Player with Module.
#[derive(Clone)]
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

/// Module sync messages sent from UI thread to audio thread.
pub enum ModuleCommand {
    Load(Module),
    Tuning(Tuning),
    FX(FXSettings),
    Kit(Vec<KitEntry>),
    Edit(Edit),
    Patch(usize, Patch),
}

/// Wrapper for module sync handling.
pub struct ModuleSync {
    producer: Producer<ModuleCommand>,
}

impl ModuleSync {
    pub fn new(producer: Producer<ModuleCommand>) -> Self {
        Self { producer }
    }

    pub fn push(&mut self, cmd: ModuleCommand) {
        if let Err(e) = self.producer.push(cmd) {
            eprintln!("error pushing module command: {e}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digit_from_midi() {
        assert_eq!(EventData::digit_from_midi(0x00), 0x0);
        assert_eq!(EventData::digit_from_midi(0x7f), 0xF);
        assert_eq!(EventData::digit_from_midi(0x3f), 0x7);
        assert_eq!(EventData::digit_from_midi(0x40), 0x8);
    }
}