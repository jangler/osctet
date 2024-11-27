use crate::{module::Module, pattern::{Event, TrackTarget}, synth::Patch};

use super::*;

pub fn draw(ui: &mut UI, module: &mut Module) {
    for track in module.pattern.iter_mut() {
        ui.start_group();
        ui.layout = Layout::Vertical;
        ui.label(track_name(track.target, &module.patches));
        ui.layout = Layout::Horizontal;
        ui.start_group();
        if ui.button("-") && track.channels.len() > 1 {
            track.channels.pop();
        }
        if ui.button("+") {
            track.channels.push(Vec::new());
        }
        ui.layout = Layout::Vertical;
        ui.end_group();
        for channel in &track.channels {
            ui.start_group();
            draw_channel(ui, channel);
            ui.layout = Layout::Horizontal;
            ui.end_group();
        }
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

fn draw_channel(ui: &mut UI, channel: &Vec<Event>) {
    ui.layout = Layout::Vertical;
    ui.label("Channel");
}