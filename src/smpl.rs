//! Read the "smpl" chunk of a Wave file.

use std::ops::RangeInclusive;

use memmem::{Searcher, TwoWaySearcher};

#[derive(Debug)]
pub struct SmplData {
    pub midi_pitch: f32,
    pub sample_loops: Vec<RangeInclusive<usize>>,
}

impl SmplData {
    pub fn from_wave(data: &[u8]) -> Option<Self> {
        TwoWaySearcher::new("smpl".as_bytes())
            .search_in(data)
            .and_then(|index| Self::from_chunk(&data[index..]))
    }

    pub fn from_chunk(data: &[u8]) -> Option<Self> {
        let unity_note = u32::from_le_bytes(
            data.get(0x14..0x18)?.try_into().ok()?);
        let pitch_fraction = u32::from_le_bytes(
            data.get(0x18..0x1c)?.try_into().ok()?);

        let num_loops = u32::from_le_bytes(
            data.get(0x24..0x28)?.try_into().ok()?);
        let sample_loops = (0..num_loops as usize).flat_map(|i| {
            let start = u32::from_le_bytes(
                data.get((0x34 + i * 4)..(0x38 + i * 4))?.try_into().ok()?);
            let end = u32::from_le_bytes(
                data.get((0x38 + i * 4)..(0x3c + i * 4))?.try_into().ok()?);
            Some((start as usize)..=(end as usize))
        }).collect();

        Some(Self {
            midi_pitch: unity_note as f32 + pitch_fraction as f32 / 127.0,
            sample_loops,
        })
    }
}