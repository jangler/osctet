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

    pub fn remove_patch(&mut self, index: usize) {
        self.patches.remove(index);
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
    }

    /// Delete pattern events between two positions.
    pub fn delete_events(&mut self, start: Position, end: Position) {
        let tick_range = start.tick..=end.tick;
        let (start_tuple, end_tuple) = (start.x_tuple(), end.x_tuple());
        for (track_i, track) in self.tracks.iter_mut().enumerate() {
            for (channel_i, channel) in track.channels.iter_mut().enumerate() {
                channel.retain(|e| {
                    let tuple = (track_i, channel_i, e.data.column());
                    !(tick_range.contains(&e.tick)
                        && tuple >= start_tuple && tuple <= end_tuple)
                });
            }
        }
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