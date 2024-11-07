use crate::pitch::Note;

const TICKS_PER_BEAT: u32 = 120;

pub struct Editor {
    pub song: Song,
    pub cursor: Position,
}

impl Editor {
    pub fn new(song: Song) -> Self {
        Self {
            song,
            cursor: Position { tick: 0, channel: 0, column: 0, char: 0 },
        }
    }
}

pub struct Position {
    pub tick: u32,
    pub channel: u8,
    pub column: u8,
    pub char: u8,
}

pub struct Song {
    pub channels: Vec<Channel>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            channels: vec![Channel::new(), Channel::new(), Channel::new(), Channel::new()],
        }
    }
}

pub struct Channel {
    pub events: Vec<Event>,
}

impl Channel {
    fn new() -> Self {
        Self {
            events: vec![],
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
}