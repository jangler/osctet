use crate::{module::Module, pattern::{Event, Track, TrackTarget}, synth::Patch};

use super::*;

pub fn draw(ui: &mut UI, module: &mut Module) {
    let mut removed_index = None;

    for (i, track) in module.pattern.iter_mut().enumerate() {
        ui.start_group();
        ui.layout = Layout::Vertical;
        let name = track_name(track.target, &module.patches);
        if let TrackTarget::Patch(_) | TrackTarget::None = track.target {
            ui.start_group();
            ui.layout = Layout::Horizontal;
            if let Some(i) = ui.combo_box(&format!("track_{}", i), "", name,
                || track_targets(&module.patches)) {
                track.target = match i {
                    0 => TrackTarget::None,
                    i => TrackTarget::Patch(i - 1),
                }
            }
            if ui.button("X") {
                removed_index = Some(i);
            }
            ui.layout = Layout::Vertical;
            ui.end_group();
        } else {
            ui.offset_label(name);
            ui.space(1.0);
        }
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

    if !module.patches.is_empty() && ui.button("+") {
        module.pattern.push(Track::new(TrackTarget::Patch(0)));
    }

    if let Some(i) = removed_index {
        module.pattern.remove(i);
    }
}

fn track_name(target: TrackTarget, patches: &[Patch]) -> &str {
    match target {
        TrackTarget::None => "(none)",
        TrackTarget::Global => "Global",
        TrackTarget::Kit => "Kit",
        TrackTarget::Patch(i) => patches.get(i)
            .map(|x| x.name.as_ref())
            .unwrap_or("(unknown)"),
    }
}

fn track_targets(patches: &[Patch]) -> Vec<String> {
    let mut v = vec![track_name(TrackTarget::None, patches).to_owned()];
    v.extend(patches.iter().map(|x| x.name.to_owned()));
    v
}

fn draw_channel(ui: &mut UI, channel: &Vec<Event>) {
    ui.layout = Layout::Vertical;
    ui.label("Channel");
}