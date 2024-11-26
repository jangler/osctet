use std::error::Error;

use crate::{fx::GlobalFX, pattern::Pattern, pitch::{Note, Tuning}, synth::Patch};

pub struct Module {
    pub title: String,
    pub author: String,
    pub tuning: Tuning,
    pub fx: GlobalFX,
    pub kit: Vec<KitEntry>,
    pub patches: Vec<Patch>,
    pub pattern: Pattern,
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
            pattern: Pattern::new(),
        }
    }

    pub fn load() -> Result<Module, Box<dyn Error>> {
        todo!()
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        todo!()
    }
}

pub struct KitEntry {
    pub input_note: Note,
    pub patch_index: usize,
    pub patch_note: Note,
}