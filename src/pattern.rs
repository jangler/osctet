use crate::pitch::Note;

const TICKS_PER_BEAT: u32 = 120;

pub struct Position {
    pub tick: u32,
    pub channel: u8,
    pub column: u8,
    pub char: u8,
}

#[derive(Clone, Copy)]
pub enum TrackTarget {
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