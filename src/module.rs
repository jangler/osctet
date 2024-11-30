//! Definitions for all stored module data except patches.

use std::error::Error;

use crate::{fx::GlobalFX, pitch::{Note, Tuning}, synth::Patch};

pub const TICKS_PER_BEAT: u32 = 120;

pub const GLOBAL_COLUMN: u8 = 0;
pub const NOTE_COLUMN: u8 = 0;
pub const VEL_COLUMN: u8 = 1;
pub const MOD_COLUMN: u8 = 2;

pub struct Module {
    pub title: String,
    pub author: String,
    pub tuning: Tuning,
    pub fx: GlobalFX,
    pub kit: Vec<KitEntry>,
    pub patches: Vec<Patch>,
    pub tracks: Vec<Track>,

    // TODO: cap size of undo stack.
    //       could use https://crates.io/crates/deepsize?
    undo_stack: Vec<Edit>,
    redo_stack: Vec<Edit>,
    track_history: Vec<TrackEdit>,
}

impl Module {
    pub fn new(fx: GlobalFX) -> Module {
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
        }
    }

    pub fn load() -> Result<Module, Box<dyn Error>> {
        todo!()
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        todo!()
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

    /// Delete pattern events between two positions.
    pub fn delete_events(&mut self, start: Position, end: Position) {
        let tick_range = start.tick..=end.tick;
        let (start_tuple, end_tuple) = (start.x_tuple(), end.x_tuple());
        let mut remove = Vec::new();

        for (track_i, track) in self.tracks.iter_mut().enumerate() {
            for (channel_i, channel) in track.channels.iter_mut().enumerate() {
                for evt in channel {
                    let tuple = (track_i, channel_i, evt.data.column());
                    if tick_range.contains(&evt.tick)
                        && tuple >= start_tuple && tuple <= end_tuple {
                        remove.push(Position {
                            tick: evt.tick,
                            track: track_i,
                            channel: channel_i,
                            column: evt.data.column(),
                        });
                    }
                }
            }
        }

        self.push_edit(Edit::PatternData {
            remove,
            add: Vec::new(),
        });
    }

    fn delete_event(&mut self, pos: Position) -> Option<Event> {
        let channel = &mut self.tracks[pos.track].channels[pos.channel];
        channel.iter()
            .position(|e| e.tick == pos.tick && e.data.column() == pos.column)
            .map(|i| channel.remove(i))
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

    /// Returns the tick of the last event in the module.
    /// TODO: Having an End event type would be better, probably.
    pub fn last_event_tick(&self) -> u32 {
        self.tracks.iter()
            .flat_map(|track| track.channels.iter())
            .flat_map(|channel| channel.iter().map(|x| x.tick))
            .max()
            .unwrap_or(0)
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
                column: event.data.column()
            }],
            add: vec![LocatedEvent { track, channel, event }]
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
                let flip_remove = add.into_iter().map(|e| {
                    let pos = Position {
                        track: e.track,
                        channel: e.channel,
                        tick: e.event.tick,
                        column: e.event.data.column(),
                    };
                    self.tracks[e.track].channels[e.channel].push(e.event);
                    pos
                }).collect();
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
}

#[derive(Default)]
pub struct KitEntry {
    pub input_note: Note,
    pub patch_index: usize,
    pub patch_note: Note,
}

pub struct Track {
    pub target: TrackTarget,
    pub channels: Vec<Vec<Event>>,
}

impl Track {
    pub fn new(target: TrackTarget) -> Self {
        Self {
            target,
            channels: vec![Vec::new()],
        }
    }
}

#[derive(Clone, Copy)]
pub enum TrackTarget {
    None,
    Global,
    Kit,
    Patch(usize),
}

pub struct Event {
    pub tick: u32,
    pub data: EventData,
}

pub enum EventData {
    Pitch(Note),
    NoteOff,
    Pressure(u8),
    Modulation(u8),
    Tempo(f32),
    LoopStart,
    LoopEnd,
}

impl EventData {
    pub fn column(&self) -> u8 {
        match *self {
            Self::Pressure(_) => VEL_COLUMN,
            Self::Modulation(_) => MOD_COLUMN,
            _ => NOTE_COLUMN,
        }
    }
}

#[derive(Clone, Copy, Debug)]
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
}

/// An operation that changes Module data.
pub enum Edit {
    InsertTrack(usize, Track),
    RemoveTrack(usize),
    RemapTrack(usize, TrackTarget),
    AddChannel(usize, Vec<Event>),
    RemoveChannel(usize),
    // TODO: insertion doesn't overwrite existing data
    PatternData {
        remove: Vec<Position>,
        add: Vec<LocatedEvent>,
    },
    // TODO: redoing patch removal doesn't revert track/kit mappings
    InsertPatch(usize, Patch),
    RemovePatch(usize),
}

/// Used to track added/removed Tracks for synchronizing Player with Module.
pub enum TrackEdit {
    Insert(usize),
    Remove(usize),
}

/// Event with global location data, for the undo stack.
pub struct LocatedEvent {
    track: usize,
    channel: usize,
    event: Event,
}