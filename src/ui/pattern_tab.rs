use crate::{module::Module, pattern::{Event, Track, TrackTarget, GLOBAL_COLUMN, MOD_COLUMN, TICKS_PER_BEAT}, synth::Patch};

use super::*;

/// Visual height of a pattern beat.
const BEAT_HEIGHT: f32 = 100.0;

fn is_shift_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

pub fn draw(ui: &mut UI, module: &mut Module) {
    let mut removed_track = None;
    let mut removed_channel_track = None;

    if !ui.accepting_keyboard_input() {
        if is_key_pressed(KeyCode::Up) {
            translate_cursor(ui, (TICKS_PER_BEAT / ui.beat_division) as i64 * -1);
        }
        if is_key_pressed(KeyCode::Down) {
            translate_cursor(ui, (TICKS_PER_BEAT / ui.beat_division) as i64);
        }
        if is_key_pressed(KeyCode::Left) {
            shift_column_left(ui, &module.tracks);
        }
        if is_key_pressed(KeyCode::Right) {
            shift_column_right(ui, &module.tracks);
        }
        if is_key_pressed(KeyCode::Tab) {
            if is_shift_down() {
                shift_channel_left(ui);
            } else {
                shift_channel_right(ui, &module.tracks);
            }
        }
    }

    for (i, track) in module.tracks.iter_mut().enumerate() {
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
                removed_track = Some(i);
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
            removed_channel_track = Some(i);
        }
        if ui.button("+") {
            track.channels.push(Vec::new());
        }
        ui.layout = Layout::Vertical;
        ui.end_group();
        for (j, channel) in track.channels.iter().enumerate() {
            ui.start_group();
            draw_channel(ui, channel, i, j);
            ui.layout = Layout::Horizontal;
            ui.end_group();
        }
        ui.end_group();
    }

    if let Some(i) = removed_channel_track {
        module.tracks[i].channels.pop();
        check_cursor(ui, &module.tracks);
    }

    if !module.patches.is_empty() && ui.button("+") {
        module.tracks.push(Track::new(TrackTarget::Patch(0)));
    }

    if let Some(i) = removed_track {
        module.tracks.remove(i);
        check_cursor(ui, &module.tracks);
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

fn draw_channel(ui: &mut UI, channel: &Vec<Event>, track_index: usize, channel_index: usize) {
    ui.layout = Layout::Vertical;
    ui.label("Channel");
    if ui.edit_start.track == track_index && ui.edit_start.channel == channel_index {
        ui.push_text(ui.cursor_x, ui.cursor_y + ui.edit_start.beat() * BEAT_HEIGHT,
            format!("{}", ui.edit_start.column), ui.style.theme.fg);
    }
}

fn translate_cursor(ui: &mut UI, offset: i64) {
    let dist = ui.edit_end.tick - ui.edit_start.tick;
    if -offset > ui.edit_start.tick as i64 {
        ui.edit_start.tick = 0;
    } else {
        ui.edit_start.tick = (ui.edit_start.tick as i64 + offset) as u32;
    }
    ui.edit_end.tick = ui.edit_start.tick + dist;
}

fn shift_column_left(ui: &mut UI, tracks: &Vec<Track>) {
    // TODO: edit_end
    let column = ui.edit_start.column as i8 - 1;
    if column >= 0 {
        ui.edit_start.column = column as u8;
    } else {
        if ui.edit_start.channel > 0 {
            ui.edit_start.channel -= 1;
        } else if ui.edit_start.track > 0 {
            ui.edit_start.track -= 1;
            ui.edit_start.channel = tracks[ui.edit_start.track].channels.len() - 1;
        }

        if ui.edit_start.track == 0 {
            ui.edit_start.column = GLOBAL_COLUMN;
        } else {
            ui.edit_start.column = MOD_COLUMN;
        }
    }
}

fn shift_column_right(ui: &mut UI, tracks: &Vec<Track>) {
    // TODO: edit_end
    let column = ui.edit_start.column + 1;
    let n_columns = if ui.edit_start.track == 0 { 1 } else { 3 };
    if column < n_columns {
        ui.edit_start.column = column;
    } else {
        if ui.edit_start.channel + 1 < tracks[ui.edit_start.track].channels.len() {
            ui.edit_start.channel += 1;
            ui.edit_start.column = 0;
        } else if ui.edit_start.track + 1 < tracks.len() {
            ui.edit_start.track += 1;
            ui.edit_start.channel = 0;
            ui.edit_start.column = 0;
        }
    }
}

fn shift_channel_left(ui: &mut UI) {
    // TODO: edit_end
    let channel = ui.edit_start.channel as isize - 1;
    if channel >= 0 {
        ui.edit_start.channel = channel as usize;
    } else if ui.edit_start.track > 0 {
        ui.edit_start.track -= 1;
        if ui.edit_start.track == 0 {
            ui.edit_start.column = 0;
        }
    }
}

fn shift_channel_right(ui: &mut UI, tracks: &Vec<Track>) {
    // TODO: edit_end
    let channel = ui.edit_start.channel + 1;
    if channel < tracks[ui.edit_start.track].channels.len() {
        ui.edit_start.channel = channel;
    } else if ui.edit_start.track + 1 < tracks.len() {
        ui.edit_start.channel = 0;
        ui.edit_start.track += 1;
    }
}

/// Reposition the pattern cursor if it's in an invalid position.
fn check_cursor(ui: &mut UI, tracks: &Vec<Track>) {
    for cursor in [&mut ui.edit_start, &mut ui.edit_end] {
        if cursor.track >= tracks.len() {
            cursor.track -= 1;
            cursor.channel = tracks[cursor.track].channels.len() - 1;
            if cursor.track == 0 {
                cursor.column = 0;
            }
        } else if cursor.channel >= tracks[cursor.track].channels.len() {
            cursor.channel -= 1;
        }
    }
}