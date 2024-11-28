use crate::{module::*, playback::Player, synth::Patch};

use super::*;

/// Visual height of a pattern beat.
const BEAT_HEIGHT: f32 = 100.0;

fn is_shift_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

pub fn draw(ui: &mut UI, module: &mut Module, player: &mut Player) {
    if !ui.accepting_keyboard_input() {
        for key in get_keys_pressed() {
            handle_key(key, ui, module, player);
        }
    }

    let cursor = ui.edit_start;
    if !ui.accepting_note_input() && cursor.column == NOTE_COLUMN {
        while let Some(data) = ui.note_queue.pop() {
            let chan = &mut module.tracks[cursor.track].channels[cursor.channel];
            let evt = Event {
                tick: cursor.tick,
                data,
            };
            insert_event(chan, evt);
        }
    }

    ui.start_group();
    let left_x = ui.cursor_x;
    let track_xs = draw_track_headers(ui, module, player);
    ui.layout = Layout::Vertical;
    ui.end_group();
    ui.cursor_x = track_xs[0];

    if mouse_position_vec2().y >= ui.cursor_y {
        if is_mouse_button_pressed(MouseButton::Left) {
            ui.edit_start = position_from_mouse(ui, &track_xs, &module.tracks);
            ui.edit_end = ui.edit_start;
        } else if is_mouse_button_down(MouseButton::Left) {
            ui.edit_end = position_from_mouse(ui, &track_xs, &module.tracks);
            fix_cursors(ui, &module.tracks);
        }
    }

    draw_playhead(ui, player.tick, left_x);
    draw_beats(ui, left_x);
    draw_cursor(ui, &track_xs);

    // draw channel data
    let char_width = text_width("x", &ui.style.text_params());
    for (track_i, track) in module.tracks.iter().enumerate() {
        let chan_width = channel_width(track_i, char_width);
        for (channel_i, channel) in track.channels.iter().enumerate() {
            ui.cursor_x = track_xs[track_i] + chan_width * channel_i as f32;
            draw_channel(ui, channel);
        }
    }
}

fn draw_beats(ui: &mut UI, x: f32) {
    let mut beat = 1;
    let mut y = ui.cursor_y;

    while y < ui.bounds.y + ui.bounds.h {
        ui.push_text(x, y, beat.to_string(), ui.style.theme.fg);
        beat += 1;
        y += BEAT_HEIGHT;
    }
}

/// Returns x positions of each track, plus one extra position.
fn draw_track_headers(ui: &mut UI, module: &mut Module, player: &mut Player) -> Vec<f32> {
    let mut removed_track = None;
    let mut removed_channel_track = None;

    // offset for beat width
    ui.cursor_x += text_width("x", &ui.style.text_params()) * 3.0 + MARGIN * 2.0;

    let mut xs = vec![ui.cursor_x];
    xs.extend(module.tracks.iter_mut().enumerate().map(|(i, track)| {
        ui.start_group();
        ui.layout = Layout::Vertical;

        // track name & delete button
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

        // chanel add/remove buttons
        ui.start_group();
        ui.layout = Layout::Horizontal;
        if ui.button("-") && track.channels.len() > 1 {
            removed_channel_track = Some(i);
        }
        if ui.button("+") {
            track.channels.push(Vec::new());
        }
        ui.layout = Layout::Vertical;
        ui.end_group();

        // column labels
        ui.start_group();
        ui.layout = Layout::Horizontal;
        for _ in 0..track.channels.len() {
            if i == 0 {
                ui.label("XXX")
            } else {
                ui.label("NNN");
                ui.cursor_x -= MARGIN;
                ui.label("V");
                ui.cursor_x -= MARGIN;
                ui.label("M");
            }
        }
        ui.layout = Layout::Vertical;
        ui.end_group();

        ui.layout = Layout::Horizontal;
        ui.end_group();

        ui.cursor_x
    }));

    if let Some(i) = removed_channel_track {
        module.tracks[i].channels.pop();
        fix_cursors(ui, &module.tracks);
    }

    if let Some(i) = removed_track {
        module.tracks.remove(i);
        player.track_removed(i);
        fix_cursors(ui, &module.tracks);
    }

    if !module.patches.is_empty() && ui.button("+") {
        module.tracks.push(Track::new(TrackTarget::Patch(0)));
        player.track_added();
    }

    xs
}

fn handle_key(key: KeyCode, ui: &mut UI, module: &mut Module, player: &mut Player) {
    match key {
        KeyCode::Up => {
            translate_cursor(ui, (TICKS_PER_BEAT / ui.beat_division) as i64 * -1);
        },
        KeyCode::Down => translate_cursor(ui, (TICKS_PER_BEAT / ui.beat_division) as i64),
        KeyCode::Left => shift_column_left(ui, &module.tracks),
        KeyCode::Right => shift_column_right(ui, &module.tracks),
        KeyCode::Tab => if is_shift_down() {
            shift_channel_left(ui);
        } else {
            shift_channel_right(ui, &module.tracks);
        },
        KeyCode::Delete => {
            let (start, end) = ui.selection_corners();
            module.delete_events(start, end);
        },
        KeyCode::Key0 => input_digit(ui, module, 0),
        KeyCode::Key1 => input_digit(ui, module, 1),
        KeyCode::Key2 => input_digit(ui, module, 2),
        KeyCode::Key3 => input_digit(ui, module, 3),
        KeyCode::Key4 => input_digit(ui, module, 4),
        KeyCode::Key5 => input_digit(ui, module, 5),
        KeyCode::Key6 => input_digit(ui, module, 6),
        KeyCode::Key7 => input_digit(ui, module, 7),
        KeyCode::Key8 => input_digit(ui, module, 8),
        KeyCode::Key9 => input_digit(ui, module, 9),
        KeyCode::F5 => {
            player.tick = 0;
            player.playing = true;
        },
        KeyCode::F7 => player.playing = true,
        KeyCode::F8 => player.stop(),
        _ => (),
    }
}

fn input_digit(ui: &UI, module: &mut Module, value: u8) {
    let cursor = ui.edit_start;
    let channel = &mut module.tracks[cursor.track].channels[cursor.channel];
    match cursor.column {
        VEL_COLUMN => insert_event(channel, Event {
            tick: cursor.tick,
            data: EventData::Pressure(value),
        }),
        MOD_COLUMN => insert_event(channel, Event {
            tick: cursor.tick,
            data: EventData::Modulation(value),
        }),
        _ => (),
    }
}

fn insert_event(chan: &mut Vec<Event>, evt: Event) {
    chan.retain(|x| x.tick != evt.tick || x.data.column() != evt.data.column());
    chan.push(evt);
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

fn draw_playhead(ui: &mut UI, tick: u32, x: f32) {
    let rect = Rect {
        x,
        y: ui.cursor_y + tick as f32 / TICKS_PER_BEAT as f32 * BEAT_HEIGHT,
        w: ui.bounds.w,
        h: cap_height(&ui.style.text_params()) + MARGIN * 2.0,
    };
    ui.push_rect(rect, ui.style.theme.hover, None);
}

fn draw_cursor(ui: &mut UI, track_xs: &[f32]) {
    let params = &ui.style.text_params();
    let (tl, br) = ui.selection_corners();
    let start = position_coords(tl, &params, track_xs, false);
    let end = position_coords(br, &params, track_xs, true);

    let selection_rect = Rect {
        x: MARGIN + start.x,
        y: ui.cursor_y + start.y,
        w: end.x - start.x,
        h: end.y - start.y,
    };
    ui.push_rect(selection_rect, ui.style.theme.click, None);
}

fn draw_channel(ui: &mut UI, channel: &Vec<Event>) {
    let char_width = text_width("x", &ui.style.text_params());
    for event in channel {
        draw_event(ui, event, char_width);
    }
}

fn draw_event(ui: &mut UI, evt: &Event, char_width: f32) {
    let x = ui.cursor_x + column_x(evt.data.column(), char_width);
    let y = ui.cursor_y + evt.tick as f32 / TICKS_PER_BEAT as f32 * BEAT_HEIGHT;
    let text = match evt.data {
        EventData::Pitch(note) => note.to_string(),
        EventData::Pressure(v) => v.to_string(),
        EventData::Modulation(v) => v.to_string(),
        _ => String::from("unknown"),
    };
    ui.push_text(x, y, text, ui.style.theme.fg);
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

fn shift_column_left(ui: &mut UI, tracks: &[Track]) {
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
    ui.edit_end = ui.edit_start; // TODO
}

fn shift_column_right(ui: &mut UI, tracks: &[Track]) {
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
    ui.edit_end = ui.edit_start; // TODO
}

fn shift_channel_left(ui: &mut UI) {
    let channel = ui.edit_start.channel as isize - 1;
    if channel >= 0 {
        ui.edit_start.channel = channel as usize;
    } else if ui.edit_start.track > 0 {
        ui.edit_start.track -= 1;
        if ui.edit_start.track == 0 {
            ui.edit_start.column = 0;
        }
    }
    ui.edit_end = ui.edit_start; // TODO
}

fn shift_channel_right(ui: &mut UI, tracks: &[Track]) {
    let channel = ui.edit_start.channel + 1;
    if channel < tracks[ui.edit_start.track].channels.len() {
        ui.edit_start.channel = channel;
    } else if ui.edit_start.track + 1 < tracks.len() {
        ui.edit_start.channel = 0;
        ui.edit_start.track += 1;
    }
    ui.edit_end = ui.edit_start; // TODO
}

/// Reposition the pattern cursors if in an invalid position.
fn fix_cursors(ui: &mut UI, tracks: &[Track]) {
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

/// Returns the visual coordinates of a Position. Uses the top-left corner of
/// the cell by default.
fn position_coords(pos: Position, params: &TextParams, track_xs: &[f32],
    bottom_left: bool
) -> Vec2 {
    let char_width = text_width("x", &params);
    let x = track_xs[pos.track] + channel_width(pos.track, char_width) * pos.channel as f32
        + if bottom_left {
            column_x(pos.column + 1, char_width) - MARGIN
        } else {
            column_x(pos.column, char_width)
        };
    let y = pos.beat() * BEAT_HEIGHT + if bottom_left {
        cap_height(&params) + MARGIN * 2.0
    } else {
        0.0
    };
    Vec2 { x, y }
}

fn channel_width(track_index: usize, char_width: f32) -> f32 {
    if track_index == 0 {
        char_width * 3.0 + MARGIN * 2.0
    } else {
        char_width * 5.0 + MARGIN * 4.0
    }
}

fn column_x(column: u8, char_width: f32) -> f32 {
    match column {
        NOTE_COLUMN => 0.0,
        VEL_COLUMN => char_width * 3.0 + MARGIN,
        MOD_COLUMN => char_width * 4.0 + MARGIN * 2.0,
        // allow this to make some calculations easier
        3 => char_width * 5.0 + MARGIN * 3.0,
        _ => panic!("invalid cursor column"),
    }
}

/// Convert mouse coordinates to a Position.
fn position_from_mouse(ui: &UI, track_xs: &[f32], tracks: &[Track]) -> Position {
    let (x, y) = mouse_position();
    let y = y - ui.cursor_y;
    let params = ui.style.text_params();
    let cell_height = cap_height(&params) + MARGIN * 2.0;
    let char_width = text_width("x", &params);
    let mut pos = Position {
        tick: ((y - cell_height * 0.5) as f32
            / BEAT_HEIGHT * ui.beat_division as f32).round() as u32
            * TICKS_PER_BEAT / ui.beat_division,
        track: 0,
        channel: 0,
        column: 0,
    };

    for (i, tx) in track_xs.split_last().unwrap().1.iter().enumerate() {
        if x >= *tx {
            let chan_width = channel_width(i, char_width);
            pos.track = i;
            pos.channel = (tracks[i].channels.len() - 1)
                .min(((x - tx) / chan_width) as usize);
            pos.column = if i == 0 {
                0
            } else {
                let x = x - tx - pos.channel as f32 * chan_width;
                if column_x(2, char_width) < x {
                    2
                } else if column_x(1, char_width) < x {
                    1
                } else {
                    0
                }
            };
        }
    }

    pos
}