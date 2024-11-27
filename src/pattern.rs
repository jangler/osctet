use crate::pitch::Note;

pub const TICKS_PER_BEAT: u32 = 120;

pub const GLOBAL_COLUMN: u8 = 0;
pub const NOTE_COLUMN: u8 = 0;
pub const VEL_COLUMN: u8 = 1;
pub const MOD_COLUMN: u8 = 2;

#[derive(Clone, Copy)]
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
}

#[derive(Clone, Copy)]
pub enum TrackTarget {
    None,
    Global,
    Kit,
    Patch(usize),
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

pub struct Event {
    pub tick: u32,
    pub data: EventData,
}

pub enum EventData {
    Pitch(Note),
    Instrument(u8),
    Pressure(u8),
    Modulation(u8),
    Tempo(f32),
    LoopStart,
    LoopEnd,
}