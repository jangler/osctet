use crate::{module::Module, pattern::TrackTarget, synth::Patch};

use super::*;

pub fn draw(ui: &mut UI, module: &mut Module) {
    for track in module.pattern.iter() {
        ui.start_group();
        ui.layout = Layout::Vertical;
        ui.label(track_name(track.target, &module.patches));
        ui.layout = Layout::Horizontal;
        ui.end_group();
    }
}

fn track_name(target: TrackTarget, patches: &[Patch]) -> &str {
    match target {
        TrackTarget::Global => "Global",
        TrackTarget::Kit => "Kit",
        TrackTarget::Patch(i) => patches.get(i)
            .map(|x| x.name.as_ref())
            .unwrap_or("(unknown)"),
    }
}