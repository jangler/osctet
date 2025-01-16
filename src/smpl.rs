//! Read the "smpl" chunk of a Wave file.

use std::ops::RangeInclusive;

use memmem::{Searcher, TwoWaySearcher};

/// Relevant data from a "smpl" chunk.
#[derive(Debug)]
pub struct SmplData {
    pub midi_pitch: f32,
    pub sample_loops: Vec<RangeInclusive<usize>>,
}

impl SmplData {
    /// Search for and read a sample chunk in the bytes of a wave file.
    pub fn from_wave(data: &[u8]) -> Option<Self> {
        TwoWaySearcher::new("smpl".as_bytes())
            .search_in(data)
            .and_then(|index| Self::from_chunk(&data[index..]))
    }

    /// Read a sample chunk.
    fn from_chunk(data: &[u8]) -> Option<Self> {
        let unity_note = read_u32(data, 0x14)?;
        let pitch_fraction = read_u32(data, 0x18)?;

        let num_loops = read_u32(data, 0x24)?;
        let sample_loops = (0..num_loops as usize).flat_map(|i| {
            let start = read_u32(data, 0x34 + i * 4)?;
            let end = read_u32(data, 0x38)?;
            Some((start as usize)..=(end as usize))
        }).collect();

        Some(Self {
            midi_pitch: unity_note as f32 + pitch_fraction as f32 / 256.0,
            sample_loops,
        })
    }
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..(offset + 4))?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}