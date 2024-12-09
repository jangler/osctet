use gcd::Gcd;

use crate::{input, module::*, playback::Player, synth::Patch};

use super::*;

/// Visual height of a pattern beat.
const BEAT_HEIGHT: f32 = 100.0;

fn is_shift_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

fn is_ctrl_down() -> bool {
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
}

/// Tracks state specific to the pattern editor.
pub struct PatternEditor {
    edit_start: Position,
    edit_end: Position,
    pub beat_division: u8,
    scroll: f32,
    tap_tempo_intervals: Vec<f32>,
    pending_interval: Option<f32>,
    clipboard: Option<PatternClip>,
}

struct PatternClip {
    start: Position,
    end: Position,
    events: Vec<ClipEvent>,
}

#[derive(Debug)]
struct ClipEvent {
    channel_offset: usize,
    event: Event,
}

impl PatternEditor {
    pub fn new() -> Self {
        let edit_cursor = Position {
            tick: 0,
            track: 0,
            channel: 0,
            column: 0,
        };
        Self {
            edit_start: edit_cursor,
            edit_end: edit_cursor,
            beat_division: 4,
            scroll: 0.0,
            tap_tempo_intervals: Vec::new(),
            pending_interval: None,
            clipboard: None,
        }
    }

    pub fn cursor_track(&self) -> usize {
        self.edit_start.track
    }
    
    pub fn cursor_tick(&self) -> u32 {
        self.edit_start.tick
    }

    pub fn in_digit_column(&self, ui: &UI) -> bool {
        ui.tabs.get(MAIN_TAB_ID) == Some(&TAB_PATTERN)
            && self.edit_start.column != NOTE_COLUMN
    }

    pub fn in_global_track(&self, ui: &UI) -> bool {
        ui.tabs.get(MAIN_TAB_ID) == Some(&TAB_PATTERN)
            && self.edit_start.track == 0
    }

    /// Convert mouse coordinates to a Position.
    fn position_from_mouse(&self, ui: &UI, track_xs: &[f32], tracks: &[Track]) -> Position {
        let (x, y) = mouse_position();
        let y = y - ui.cursor_y;
        let params = ui.style.text_params();
        let cell_height = cap_height(&params) + MARGIN * 2.0;
        let char_width = text_width("x", &params);
        let mut pos = Position {
            tick: ((y - cell_height * 0.5) as f32
                / BEAT_HEIGHT * self.beat_division as f32).round() as u32
                * TICKS_PER_BEAT / self.beat_division as u32,
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

    /// Returns the top-left and bottom-right corners of the pattern selection.
    fn selection_corners(&self) -> (Position, Position) {
        let mut start_x = self.edit_start.x_tuple();
        let mut end_x = self.edit_end.x_tuple();
        if start_x > end_x {
            (start_x, end_x) = (end_x, start_x)
        }
        let tl = Position {
            track: start_x.0,
            channel: start_x.1,
            column: start_x.2,
            tick: self.edit_start.tick.min(self.edit_end.tick),
        };
        let br = Position {
            track: end_x.0,
            channel: end_x.1,
            column: end_x.2,
            tick: self.edit_start.tick.max(self.edit_end.tick),
        };
        (tl, br)
    }
    
    fn draw_cursor(&self, ui: &mut UI, track_xs: &[f32]) {
        let params = &ui.style.text_params();
        let (tl, br) = self.selection_corners();
        let start = position_coords(tl, &params, track_xs, false);
        let end = position_coords(br, &params, track_xs, true);

        let selection_rect = Rect {
            x: MARGIN + start.x,
            y: ui.cursor_y + start.y,
            w: end.x - start.x,
            h: end.y - start.y,
        };
        let color = Color { a: 0.1, ..ui.style.theme.fg() };
        ui.push_rect(selection_rect, color, None);
    }
    
    fn handle_key(&mut self, key: KeyCode, module: &mut Module) {
        if is_ctrl_down() {
            match key {
                KeyCode::X => self.cut(module),
                KeyCode::C => self.copy(module),
                KeyCode::V => self.paste(module, is_shift_down()),
                _ => (),
            }
        } else {
            match key {
                KeyCode::Up => {
                    translate_cursor(self, (TICKS_PER_BEAT / self.beat_division as u32)
                        as i64 * -1);
                },
                KeyCode::Down =>
                    translate_cursor(self, (TICKS_PER_BEAT / self.beat_division as u32)
                        as i64),
                KeyCode::Left => shift_column_left(self, &module.tracks),
                KeyCode::Right => shift_column_right(self, &module.tracks),
                KeyCode::Tab => if is_shift_down() {
                    shift_channel_left(self);
                } else {
                    shift_channel_right(self, &module.tracks);
                },
                KeyCode::Delete => {
                    let (start, end) = self.selection_corners();
                    module.delete_events(start, end);
                },
                KeyCode::Key0 => input_digit(module, &self.edit_start, 0),
                KeyCode::Key1 => input_digit(module, &self.edit_start, 1),
                KeyCode::Key2 => input_digit(module, &self.edit_start, 2),
                KeyCode::Key3 => input_digit(module, &self.edit_start, 3),
                KeyCode::Key4 => input_digit(module, &self.edit_start, 4),
                KeyCode::Key5 => input_digit(module, &self.edit_start, 5),
                KeyCode::Key6 => input_digit(module, &self.edit_start, 6),
                KeyCode::Key7 => input_digit(module, &self.edit_start, 7),
                KeyCode::Key8 => input_digit(module, &self.edit_start, 8),
                KeyCode::Key9 => input_digit(module, &self.edit_start, 9),
                KeyCode::GraveAccent => input_note_off(&self.edit_start, module),
                KeyCode::E => insert_event_at_cursor(module, &self.edit_start, EventData::End),
                KeyCode::L => insert_event_at_cursor(module, &self.edit_start, EventData::Loop),
                KeyCode::R => self.rational_tempo(module),
                KeyCode::T => self.tap_tempo(module),
                KeyCode::Insert => self.push_rows(module),
                KeyCode::Backspace => self.pull_rows(module),
                input::ARROW_DOWN_KEY | input::ARROW_UP_KEY
                    | input::SHARP_KEY | input::FLAT_KEY
                    | input::OCTAVE_UP_KEY | input::OCTAVE_DOWN_KEY
                    | input::ENHARMONIC_ALT_KEY =>
                        nudge_notes(module, self.selection_corners()), // TODO: undo/redo
                _ => (),
            }
        }

        if key != KeyCode::T {
            self.tap_tempo_intervals.clear();
            self.pending_interval = None;
        }
    }

    fn tap_tempo(&mut self, module: &mut Module) {
        if let Some(interval) = self.pending_interval {
            self.tap_tempo_intervals.push(interval);
            let n = self.tap_tempo_intervals.len();
            let mean = self.tap_tempo_intervals.iter().sum::<f32>() / n as f32;
            let t = 60.0 / mean;
            insert_event_at_cursor(module, &self.edit_start, EventData::Tempo(t));
        }
        self.pending_interval = Some(0.0);
    }

    fn rational_tempo(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();
        let n = ((end.beat() - start.beat()) * self.beat_division as f32).round() as u8;
        let d = self.beat_division;
        if n > 0 && n != d {
            let lcm = n.gcd(d);
            insert_event_at_cursor(module, &self.edit_start,
                EventData::RationalTempo(n / lcm, d / lcm));
        }
    }

    fn cut(&mut self, module: &mut Module) {
        self.copy(module);
        let (start, end) = self.selection_corners();
        module.delete_events(start, end);
    }

    fn copy(&mut self, module: &Module) {
        let (start, end) = self.selection_corners();
        let events = module.scan_events(start, end).iter().map(|x| ClipEvent {
            channel_offset: module.channels_between(start, x.position()),
            event: x.event.clone(),
        }).collect();
        self.clipboard = Some(PatternClip {
            start,
            end,
            events,
        });
    }

    fn paste(&self, module: &mut Module, mix: bool) {
        if let Some(clip) = &self.clipboard {
            let tick_offset = self.edit_start.tick as i32 - clip.start.tick as i32;

            let add: Vec<_> = clip.events.iter().filter_map(|x| {
                self.edit_start.add_channels(x.channel_offset, &module.tracks)
                    .map(|pos| {
                        if x.event.data.is_ctrl() == (pos.track == 0) {
                            Some(LocatedEvent {
                                track: pos.track,
                                channel: pos.channel,
                                event: Event {
                                    tick: (x.event.tick as i32 + tick_offset) as u32,
                                    data: x.event.data.clone(),
                                },
                            })
                        } else {
                            None
                        }
                    }).flatten()
            }).collect();

            let remove = if mix {
                add.iter().map(|x| x.position()).collect()
            } else {
                let channel_offset = module.channels_between(clip.start, clip.end);
                let end = Position {
                    tick: self.edit_start.tick + clip.end.tick - clip.start.tick,
                    column: clip.end.column,
                    ..self.edit_start.add_channels(channel_offset, &module.tracks)
                        .unwrap_or(Position {
                            track: module.tracks.len(),
                            channel: module.tracks.last().unwrap().channels.len() - 1,
                            tick: 0,
                            column: 0,
                        })
                };
                module.scan_events(self.edit_start, end)
                    .iter().map(|x| x.position()).collect()
            };

            if !add.is_empty() {
                module.push_edit(Edit::PatternData {
                    remove,
                    add,
                });
            }
        }
        // TODO: report error?
    }

    fn draw_channel(&self, ui: &mut UI, channel: &Channel) {
        let char_width = text_width("x", &ui.style.text_params());
        self.draw_channel_line(ui);

        // draw events
        for event in &channel.events {
            draw_event(ui, event, char_width);
        }
    }

    fn draw_channel_line(&self, ui: &mut UI) {
        ui.cursor_z -= 1;
        ui.push_line(ui.cursor_x + LINE_THICKNESS * 0.5, ui.cursor_y + self.scroll,
            ui.cursor_x + LINE_THICKNESS * 0.5, ui.cursor_y + self.scroll + ui.bounds.h,
            ui.style.theme.control_bg());
        ui.cursor_z += 1;
    }

    /// Inserts rows into the pattern, shifting events.
    fn push_rows(&self, module: &mut Module) {
        // TODO: have a way to do this for all channels
        let (start, end) = self.selection_corners();
        let mut ticks = end.tick - start.tick;
        if ticks == 0 {
            ticks = TICKS_PER_BEAT / self.beat_division as u32;
        }
        module.shift_channel_events(start, end, ticks as i32);
    }

    /// Deletes rows from the pattern, shifting events.
    fn pull_rows(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();
        let mut ticks = start.tick as i32 - end.tick as i32;
        if ticks == 0 {
            ticks -= TICKS_PER_BEAT as i32 / self.beat_division as i32;
        }
        module.shift_channel_events(start, end, ticks);
    }
}

pub fn draw(ui: &mut UI, module: &mut Module, player: &mut Player, pe: &mut PatternEditor) {
    if let Some(interval) = pe.pending_interval.as_mut() {
        *interval += get_frame_time();
    }

    if !ui.accepting_keyboard_input() {
        for key in get_keys_pressed() {
            pe.handle_key(key, module);
        }
    }

    let cursor = pe.edit_start;
    if !ui.accepting_note_input() && cursor.column == NOTE_COLUMN {
        while let Some(data) = ui.note_queue.pop() {
            insert_event_at_cursor(module, &cursor, data);
        }
    }

    ui.start_group();
    let left_x = ui.cursor_x;
    let track_xs = draw_track_headers(ui, module, player);
    let rect = Rect {
        w: ui.bounds.w,
        ..ui.end_group().unwrap()
    };
    ui.cursor_z -= 1;
    ui.push_rect(rect, ui.style.theme.panel_bg(), None);
    ui.cursor_x = track_xs[0];

    let end_y = ui.bounds.h - ui.cursor_y
        + module.last_event_tick().unwrap_or(0) as f32
        * BEAT_HEIGHT / TICKS_PER_BEAT as f32;
    let viewport_h = ui.bounds.h + ui.bounds.y - ui.cursor_y;
    ui.push_line(ui.bounds.x, ui.cursor_y - LINE_THICKNESS * 0.5,
        ui.bounds.x + ui.bounds.w, ui.cursor_y - LINE_THICKNESS * 0.5,
        ui.style.theme.border_unfocused());
    ui.vertical_scrollbar(&mut pe.scroll, end_y, viewport_h);
    let viewport = Rect {
        x: ui.bounds.x,
        y: ui.cursor_y,
        w: ui.bounds.w,
        h: viewport_h,
    };
    ui.cursor_z -= 1;
    ui.cursor_y -= pe.scroll;

    if viewport.contains(mouse_position_vec2()) {
        if is_mouse_button_pressed(MouseButton::Left) {
            pe.edit_end = pe.position_from_mouse(ui, &track_xs, &module.tracks);
            if !is_shift_down() {
                pe.edit_start = pe.edit_end;
            }
        } else if is_mouse_button_down(MouseButton::Left) && !ui.grabbed() {
            pe.edit_end = pe.position_from_mouse(ui, &track_xs, &module.tracks);
            fix_cursors(pe, &module.tracks);
        }
    }

    ui.cursor_z -= 1;
    ui.push_rect(viewport, ui.style.theme.content_bg(), None);
    ui.cursor_z += 1;

    draw_playhead(ui, player.get_tick(), left_x);
    draw_beats(ui, left_x);
    pe.draw_cursor(ui, &track_xs);

    // draw channel data
    let char_width = text_width("x", &ui.style.text_params());
    for (track_i, track) in module.tracks.iter().enumerate() {
        let chan_width = channel_width(track_i, char_width);
        for (channel_i, channel) in track.channels.iter().enumerate() {
            ui.cursor_x = track_xs[track_i] + chan_width * channel_i as f32;
            pe.draw_channel(ui, channel);
        }
    }
    ui.cursor_x += channel_width(1, char_width);
    pe.draw_channel_line(ui);
}

fn draw_beats(ui: &mut UI, x: f32) {
    let mut beat = 1;
    let mut y = ui.cursor_y;
    while y < ui.bounds.y + ui.bounds.h {
        if y >= 0.0 {
            ui.push_text(x, y, beat.to_string(), ui.style.theme.fg());
        }
        beat += 1;
        y += BEAT_HEIGHT;
    }
}

/// Returns x positions of each track, plus one extra position.
fn draw_track_headers(ui: &mut UI, module: &mut Module, player: &mut Player) -> Vec<f32> {
    let mut edit = None;
    ui.layout = Layout::Horizontal;

    // offset for beat width
    ui.cursor_x += text_width("x", &ui.style.text_params()) * 3.0 + MARGIN * 2.0;

    let mut xs = vec![ui.cursor_x];
    xs.extend(module.tracks.iter_mut().enumerate().map(|(i, track)| {
        ui.start_group();

        // track name & delete button
        let name = track_name(track.target, &module.patches);
        if let TrackTarget::Patch(_) | TrackTarget::None = track.target {
            ui.start_group();
            if let Some(j) = ui.combo_box(&format!("track_{}", i), "", name,
                || track_targets(&module.patches)) {
                edit = Some(Edit::RemapTrack(i, match j {
                    0 => TrackTarget::None,
                    j => TrackTarget::Patch(j - 1),
                }));
            }
            if ui.button("X") {
                edit = Some(Edit::RemoveTrack(i));
            }
            ui.end_group();
        } else {
            ui.offset_label(name);
        }

        // chanel add/remove buttons
        ui.start_group();
        if ui.button("-") && track.channels.len() > 1 {
            edit = Some(Edit::RemoveChannel(i));
        }
        if ui.button("+") {
            edit = Some(Edit::AddChannel(i, Channel::new()));
        }
        ui.end_group();

        // column labels
        ui.start_group();
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
        ui.end_group();

        ui.end_group();
        ui.cursor_x
    }));

    if let Some(edit) = edit {
        module.push_edit(edit);
        player.update_synths(module.drain_track_history());
    }

    if !module.patches.is_empty() && ui.button("+") {
        module.add_track();
        player.update_synths(module.drain_track_history());
    }

    xs
}

fn input_digit(module: &mut Module, cursor: &Position, value: u8) {
    match cursor.column {
        VEL_COLUMN => insert_event_at_cursor(module, cursor, EventData::Pressure(value)),
        MOD_COLUMN => insert_event_at_cursor(module, cursor, EventData::Modulation(value)),
        _ => (),
    }
}

fn input_note_off(cursor: &Position, module: &mut Module) {
    insert_event_at_cursor(module, cursor, EventData::NoteOff);
}

fn nudge_notes(module: &mut Module, (start, end): (Position, Position)) {
    let replacements = module.scan_events(start, end).iter().filter_map(|evt| {
        if let EventData::Pitch(note) = evt.event.data {
            Some(LocatedEvent {
                event: Event {
                    data: EventData::Pitch(input::adjust_note_for_modifier_keys(note)),
                    ..evt.event
                },
                ..evt.clone()
            })
        } else {
            None
        }
    }).collect();
    module.push_edit(Edit::ReplaceEvents(replacements));
}

fn insert_event_at_cursor(module: &mut Module, cursor: &Position, data: EventData) {
    // TODO: insert events at all valid selected positions
    if data.is_ctrl() != (cursor.track == 0) {
        return
    }
    module.insert_event(cursor.track, cursor.channel, Event {
        tick: cursor.tick,
        data,
    });
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
    let color = Color { a: 0.1, ..ui.style.theme.fg() };
    ui.push_rect(rect, color, None);
}

fn draw_event(ui: &mut UI, evt: &Event, char_width: f32) {
    let y = ui.cursor_y + evt.tick as f32 / TICKS_PER_BEAT as f32 * BEAT_HEIGHT;
    if y < 0.0 || y > ui.bounds.y + ui.bounds.h {
        return
    }
    let col = evt.data.column();
    let x = ui.cursor_x + column_x(col, char_width);
    if x < 0.0 || x > ui.bounds.x + ui.bounds.w {
        return
    }
    let text = match evt.data {
        EventData::Pitch(note) => note.to_string(),
        EventData::NoteOff => String::from("---"),
        EventData::Pressure(v) => v.to_string(),
        EventData::Modulation(v) => v.to_string(),
        EventData::End => String::from("END"),
        EventData::Loop => String::from("LP"),
        EventData::Tempo(t) => t.round().to_string(),
        EventData::RationalTempo(n, d) => format!("{}:{}", n, d),
    };
    let color = match evt.data {
        EventData::Pressure(x) => Color {
            a: 0.5 + x as f32 / 18.0,
            ..ui.style.theme.accent1_fg()
        },
        EventData::Modulation(x) => Color {
            a: 0.5 + x as f32 / 18.0,
            ..ui.style.theme.accent2_fg()
        },
        _ => ui.style.theme.fg(),
    };
    ui.push_text(x, y, text, color);
}

fn translate_cursor(pe: &mut PatternEditor, offset: i64) {
    if -offset > pe.edit_end.tick as i64 {
        pe.edit_end.tick = 0;
    } else {
        pe.edit_end.tick = (pe.edit_end.tick as i64 + offset) as u32;
    }
    if !is_shift_down() {
        pe.edit_start.tick = pe.edit_end.tick;
    }
}

fn shift_column_left(pe: &mut PatternEditor, tracks: &[Track]) {
    let column = pe.edit_end.column as i8 - 1;
    if column >= 0 {
        pe.edit_end.column = column as u8;
    } else {
        if pe.edit_end.channel > 0 {
            pe.edit_end.channel -= 1;
        } else if pe.edit_end.track > 0 {
            pe.edit_end.track -= 1;
            pe.edit_end.channel = tracks[pe.edit_end.track].channels.len() - 1;
        }

        if pe.edit_end.track == 0 {
            pe.edit_end.column = GLOBAL_COLUMN;
        } else {
            pe.edit_end.column = MOD_COLUMN;
        }
    }
    if !is_shift_down() {
        pe.edit_start.track = pe.edit_end.track;
        pe.edit_start.channel = pe.edit_end.channel;
        pe.edit_start.column = pe.edit_end.column;
    }
}

fn shift_column_right(pe: &mut PatternEditor, tracks: &[Track]) {
    let column = pe.edit_end.column + 1;
    let n_columns = if pe.edit_end.track == 0 { 1 } else { 3 };
    if column < n_columns {
        pe.edit_end.column = column;
    } else {
        if pe.edit_end.channel + 1 < tracks[pe.edit_end.track].channels.len() {
            pe.edit_end.channel += 1;
            pe.edit_end.column = 0;
        } else if pe.edit_end.track + 1 < tracks.len() {
            pe.edit_end.track += 1;
            pe.edit_end.channel = 0;
            pe.edit_end.column = 0;
        }
    }
    if !is_shift_down() {
        pe.edit_start.track = pe.edit_end.track;
        pe.edit_start.channel = pe.edit_end.channel;
        pe.edit_start.column = pe.edit_end.column;
    }
}

fn shift_channel_left(pe: &mut PatternEditor) {
    let channel = pe.edit_end.channel as isize - 1;
    if channel >= 0 {
        pe.edit_end.channel = channel as usize;
    } else if pe.edit_end.track > 0 {
        pe.edit_end.track -= 1;
        if pe.edit_end.track == 0 {
            pe.edit_end.column = 0;
        }
    }
    pe.edit_start.track = pe.edit_end.track;
    pe.edit_start.channel = pe.edit_end.channel;
}

fn shift_channel_right(pe: &mut PatternEditor, tracks: &[Track]) {
    let channel = pe.edit_end.channel + 1;
    if channel < tracks[pe.edit_end.track].channels.len() {
        pe.edit_end.channel = channel;
    } else if pe.edit_end.track + 1 < tracks.len() {
        pe.edit_end.channel = 0;
        pe.edit_end.track += 1;
    }
    pe.edit_start.track = pe.edit_end.track;
    pe.edit_start.channel = pe.edit_end.channel;
}

/// Reposition the pattern cursors if in an invalid position.
fn fix_cursors(pe: &mut PatternEditor, tracks: &[Track]) {
    for cursor in [&mut pe.edit_start, &mut pe.edit_end] {
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