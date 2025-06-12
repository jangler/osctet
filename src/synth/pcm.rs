//! PCM loading and manipulation.

use std::{error::Error, fs, ops::RangeInclusive, path::{Path, PathBuf}, sync::Arc};

use fundsp::{math::db_amp, wave::Wave};
use memmem::{Searcher, TwoWaySearcher};
use ordered_float::OrderedFloat;
use pitch_detector::pitch::{HannedFftDetector, PitchDetector};
use serde::{Deserialize, Serialize};

/// Stores data for PCM waveforms.
#[derive(Clone, Serialize, Deserialize)]
pub struct PcmData {
    data: Vec<u8>, // for serialization
    #[serde(skip)]
    #[serde(default = "empty_wave")]
    pub wave: Arc<Wave>,
    pub loop_point: Option<usize>,
    #[serde(skip)]
    pub path: Option<PathBuf>,
    #[serde(skip)]
    pub midi_pitch: Option<f32>,
    #[serde(default)]
    pub filename: String,
}

/// Default for serde.
fn empty_wave() -> Arc<Wave> {
    Arc::new(Wave::new(1, 44100.0))
}

impl PcmData {
    /// Supported file extensions for loading.
    pub const FILE_EXTENSIONS: [&str; 11] =
        ["aac", "aiff", "caf", "flac", "m4a", "mkv", "mp3", "mp4", "ogg", "wav", "webm"];

    /// Check whether a path has a loadable file extension.
    fn can_load_path(path: &Path) -> bool {
        path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| {
            let ext = ext.to_ascii_lowercase();
            Self::FILE_EXTENSIONS.iter().any(|x| x.to_ascii_lowercase() == ext)
        })
    }

    /// Load PCM from an audio file.
    pub fn load(path: impl AsRef<Path>, trim: bool) -> Result<Self, Box<dyn Error>> {
        let data = fs::read(&path)?;
        // TODO: it'd be great not to have to clone the whole wave
        let mut wave = Wave::load_slice(data.clone())?;
        wave.normalize();

        let trim_offset = if trim {
            trim_wave(&mut wave)
        } else {
            0
        };

        let smpl = SmplData::from_wave(&data);
        let loop_point = smpl.as_ref().and_then(|smpl|
            smpl.sample_loops.first().map(|lp|
                (*lp.start() - trim_offset)
                .min(wave.len().saturating_sub(1))));
        let midi_pitch = smpl.as_ref().map(|smpl| smpl.midi_pitch);
        let filename = path.as_ref().file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();

        Ok(Self {
            wave: Arc::new(wave),
            data,
            loop_point,
            path: Some(path.as_ref().to_path_buf()),
            midi_pitch,
            filename,
        })
    }

    /// Loads the audio file with position offset by `offset` in the file's
    /// directory.
    pub fn load_offset(path: &PathBuf, offset: isize, trim: bool) -> Result<Self, Box<dyn Error>> {
        let path = path.parent().and_then(|p| {
            fs::read_dir(p).ok().and_then(|entries| {
                let mut entries: Vec<_> = entries.flatten()
                    .filter(|e| Self::can_load_path(&e.path()))
                    .collect();
                // on some platforms, entries are unsorted by default
                entries.sort_by_key(|e| e.file_name());
                entries.iter().position(|e| e.path() == *path).map(|i| {
                    let i = (i as isize + offset)
                        .rem_euclid(entries.len() as isize) as usize;
                    entries[i].path()
                })
            })
        });

        match path {
            Some(path) => Self::load(path, trim),
            None => Err("could not find path".into()),
        }
    }

    /// Initialize deserialized PcmData before use.
    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {
        let mut wave = Wave::load_slice(self.data.clone())?;
        // the stored data is the raw file, so we have to normalize on init
        wave.normalize();
        self.wave = Arc::new(wave);
        Ok(())
    }

    /// Adjust loop point to be smoother.
    pub fn fix_loop_point(&mut self) {
        // look for a sample that's after a similar sample to the last sample
        // in the file, in terms of sample value and slope. this algorithm is
        // a bit weird and "legacy" (there used to be a wrong calculation that
        // was unknowingly compensated for) but it often gets the "right"
        // answer so i don't want to mess with it right now.

        if let Some(pt) = &mut self.loop_point {
            // don't mess with the loop point if it's zero -- it might be a
            // single-cycle wave
            if *pt == 0 || self.wave.len() < 3 {
                return
            }

            // don't move the point by more than 2 ms
            let max_distance = (self.wave.sample_rate() as f32 * 0.002) as usize;
            let window_start = pt.saturating_sub(max_distance);
            let window_end = (*pt + max_distance).min(self.wave.len() - 3);

            let last_sample = self.wave.at(0, self.wave.len() - 1);
            let second_last_sample = self.wave.at(0, self.wave.len() - 2);
            let delta = last_sample - second_last_sample;
            let mut matches = Vec::new();

            for i in window_start..window_end {
                let s1 = self.wave.at(0, i);
                let s2 = self.wave.at(0, i + 1);
                let test_delta = s2 - s1;

                if test_delta.signum() == delta.signum() {
                    matches.push((i + 2, s2));
                }
            }

            if let Some((i, _)) = matches.into_iter()
                .min_by_key(|(_, s)| OrderedFloat((last_sample - s).abs())) {
                *pt = i;
            }
        }
    }

    /// Attempts to detect the fundamental frequency of the sample.
    pub fn detect_pitch(&self) -> Option<f64> {
        let signal: Vec<_> = (0..self.wave.len())
            .map(|i| self.wave.at(0, i) as f64)
            .collect();
        let rate = self.wave.sample_rate();

        HannedFftDetector::default().detect_pitch(&signal, rate)
    }
}

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
            let offset = i * 24;
            let start = read_u32(data, 0x34 + offset)?;
            let end = read_u32(data, 0x38 + offset)?;
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

/// Trim leading and trailing silence from the wave.
/// Returns the total count of samples trimmed.
fn trim_wave(wave: &mut Wave) -> usize {
    // 80 dB is the difference between a loudish listening volume and the limit
    // of perception, so we can consider anything below -80 dB to be silence
    let threshold = db_amp(-80.0);
    let mut start = 0;
    let mut end = wave.len();
    let len = end;

    while start < end && wave.at(0, start) < threshold {
        start += 1;
    }

    while end > start && wave.at(0, end - 1) < threshold {
        end -= 1;
    }

    wave.retain(start as isize, end - start);

    start + len - end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_load_path() {
        let wav_lower = Path::new("./files/a.wav");
        let wav_upper = Path::new("./files/B.WAV");
        let png = Path::new("./files/c.png");

        assert_eq!(PcmData::can_load_path(wav_lower), true);
        assert_eq!(PcmData::can_load_path(wav_upper), true);
        assert_eq!(PcmData::can_load_path(png), false);
    }
}