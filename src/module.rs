use std::error::Error;

use crate::{fx::GlobalFX, pattern::{Track, TrackTarget}, pitch::{Note, Tuning}, synth::Patch};

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
}

#[derive(Default)]
pub struct KitEntry {
    pub input_note: Note,
    pub patch_index: usize,
    pub patch_note: Note,
}