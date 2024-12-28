use std::collections::HashSet;

use gcd::Gcd;

use crate::{config::Config, input::{self, Action}, module::*, playback::Player, synth::Patch};

use super::*;

const PATTERN_MARGIN: f32 = 2.0;

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
    beat_scroll: f32, // measured in beats
    tap_tempo_intervals: Vec<f32>,
    pending_interval: Option<f32>,
    clipboard: Option<PatternClip>,
    follow: bool,
    record: bool,
    screen_tick: u32,
    screen_tick_max: u32,
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
            beat_scroll: 0.0,
            tap_tempo_intervals: Vec::new(),
            pending_interval: None,
            clipboard: None,
            follow: false,
            record: false,
            screen_tick: 0,
            screen_tick_max: 0,
        }
    }

    pub fn inc_division(&mut self) {
        self.set_division((self.beat_division + 1).min(TICKS_PER_BEAT as u8));
    }

    pub fn dec_division(&mut self) {
        self.set_division((self.beat_division - 1).max(1));
    }

    pub fn double_division(&mut self) {
        self.set_division((self.beat_division * 2).min(TICKS_PER_BEAT as u8));
    }

    pub fn halve_division(&mut self) {
        self.set_division((self.beat_division / 2).max(1));
    }

    pub fn set_division(&mut self, division: u8) {
        let division = division.max(1).min(TICKS_PER_BEAT as u8);
        self.screen_tick_max = self.screen_tick
            + (self.screen_tick_max - self.screen_tick)
            * self.beat_division as u32 / division as u32;
        self.beat_division = division;
        self.cursor_to_division();
        self.scroll_to(self.cursor_tick());
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

    fn beat_height(&self, ui: &UI) -> f32 {
        line_height(&ui.style.atlas) * self.beat_division as f32
    }

    /// Convert mouse coordinates to a Position.
    fn position_from_mouse(&self, ui: &UI, track_xs: &[f32], tracks: &[Track]) -> Position {
        let (x, y) = mouse_position();
        let mut pos = Position {
            tick: self.round_tick(self.y_tick(y, ui)),
            track: 0,
            channel: 0,
            column: 0,
        };

        for (i, tx) in track_xs.split_last().unwrap().1.iter().enumerate() {
            if x >= *tx {
                let chan_width = channel_width(i, &ui.style);
                pos.track = i;
                pos.channel = (tracks[i].channels.len() - 1)
                    .min(((x - tx) / chan_width) as usize);
                pos.column = if i == 0 {
                    0
                } else {
                    let x = x - tx - pos.channel as f32 * chan_width;
                    if column_x(2, &ui.style) < x {
                        2
                    } else if column_x(1, &ui.style) < x {
                        1
                    } else {
                        0
                    }
                };
            }
        }

        pos
    }

    fn y_tick(&self, y: f32, ui: &UI) -> u32 {
        let beat_height = self.beat_height(ui);
        ((y - ui.cursor_y - line_height(&ui.style.atlas) * 0.5)
            / beat_height * self.beat_division as f32
            * TICKS_PER_BEAT as f32 / self.beat_division as f32).round() as u32
    }

    /// Returns the tick of the first beat on-screen.
    pub fn screen_beat_tick(&self) -> u32 {
        (self.screen_tick as f32 / TICKS_PER_BEAT as f32).ceil() as u32 * TICKS_PER_BEAT
    }

    fn set_screen_ticks(&mut self, viewport: Rect, ui: &UI) {
        self.screen_tick = self.y_tick(viewport.y, ui);
        self.screen_tick_max = self.y_tick(viewport.y + viewport.h, ui);
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
        let (tl, br) = self.selection_corners();
        let beat_height = self.beat_height(ui);
        let start = position_coords(tl, &ui.style, track_xs, false, beat_height);
        let end = position_coords(br, &ui.style, track_xs, true, beat_height);

        let selection_rect = Rect {
            x: ui.style.margin + start.x,
            y: ui.cursor_y + start.y,
            w: end.x - start.x,
            h: end.y - start.y,
        };
        let color = Color { a: 0.1, ..ui.style.theme.fg() };
        ui.push_rect(selection_rect, color, None);
    }

    pub fn action(&mut self, action: Action, module: &mut Module, cfg: &Config,
        player: &mut Player
    ) {
        match action {
            Action::Cut => self.cut(module),
            Action::Copy => self.copy(module),
            Action::Paste => self.paste(module, false),
            Action::MixPaste => self.paste(module, true),
            Action::InsertPaste => {
                self.selection_to_clip(&module);
                self.push_rows(module);
                self.paste(module, false);
            },
            Action::PrevRow => self.translate_cursor(
                (TICKS_PER_BEAT / self.beat_division as u32) as i64 * -1),
            Action::NextRow => self.translate_cursor(
                (TICKS_PER_BEAT / self.beat_division as u32) as i64),
            Action::PrevColumn => shift_column_left(self, &module.tracks),
            Action::NextColumn => shift_column_right(self, &module.tracks),
            Action::NextChannel => shift_channel_right(self, &module.tracks),
            Action::PrevChannel => shift_channel_left(self),
            Action::Delete => {
                let (start, end) = self.selection_corners();
                if (start.track, start.channel, start.column)
                    == (end.track, end.channel, end.column)
                    && is_shift_down() {
                    self.multi_channel_delete(module);
                } else {
                    module.delete_events(start, end);
                }
            },
            Action::NoteOff => self.input_note_off(module),
            Action::End =>
                insert_event_at_cursor(module, &self.edit_start, EventData::End),
            Action::Loop =>
                insert_event_at_cursor(module, &self.edit_start, EventData::Loop),
            Action::RationalTempo => self.rational_tempo(module),
            Action::TapTempo => self.tap_tempo(module),
            Action::InsertRows => self.push_rows(module),
            Action::DeleteRows => self.pull_rows(module),
            Action::NudgeArrowUp | Action::NudgeArrowDown
                | Action::NudgeSharp | Action::NudgeFlat
                | Action::NudgeOctaveUp | Action::NudgeOctaveDown
                | Action::NudgeEnharmonic =>
                    nudge_notes(module, self.selection_corners(), cfg),
            Action::ToggleFollow => self.follow = !self.follow,
            Action::ToggleRecord => if self.record {
                player.stop();
                self.record = false;
            } else {
                player.record_from(self.cursor_tick(), module);
                self.record = true;
            },
            Action::SelectAllChannels => self.select_all_channels(module),
            Action::PlaceEvenly => self.place_events_evenly(module),
            Action::NextBeat => self.translate_cursor(TICKS_PER_BEAT as i64),
            Action::PrevBeat => self.translate_cursor(-(TICKS_PER_BEAT as i64)),
            Action::NextEvent => self.next_event(module),
            Action::PrevEvent => self.prev_event(module),
            Action::PatternStart => self.translate_cursor(-(self.cursor_tick() as i64)),
            Action::PatternEnd => if let Some(tick) = module.last_event_tick() {
                self.translate_cursor(tick as i64 - self.cursor_tick() as i64);
            }
            Action::IncrementValues => self.shift_values(1, module),
            Action::DecrementValues => self.shift_values(-1, module),
            Action::Interpolate => self.interpolate(module),
            Action::MuteTrack => player.toggle_mute(module, self.cursor_track()),
            Action::SoloTrack => player.toggle_solo(module, self.cursor_track()),
            Action::UnmuteAllTracks => player.unmute_all(module),
            Action::CycleNotation => self.cycle_notation(module),
            _ => (),
        }
    }

    /// Expands the selection to the bounds of what would be pasted.
    fn selection_to_clip(&mut self, module: &Module) {
        if let Some(clip) = &self.clipboard {
            let channel_offset = module.channels_between(clip.start, clip.end);
            self.edit_end = Position {
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
        }
    }

    fn interpolate(&self, module: &mut Module) {
        let (start, mut end) = self.selection_corners();

        if start.tick == end.tick &&
            (start.column > 0 || module.event_at(self.edit_start).is_none()) {
            // interpolate to next event in column
            let evt = module.tracks[self.edit_start.track]
                .channels[self.edit_start.channel]
                .events.iter()
                .find(|e| e.tick > self.edit_start.tick
                    && e.data.logical_column() == self.edit_start.column);

            if let Some(evt) = evt {
                end.tick = evt.tick;
            } else {
                return
            }
        }

        let interp_event_at_start = module.event_at(Position {
            column: start.column | EventData::INTERP_COL_FLAG,
            ..start
        }).map(|e| e.data.clone());
        let interp_event_at_end = module.event_at(Position {
            column: end.column | EventData::INTERP_COL_FLAG,
            ..end
        }).map(|e| e.data.clone());

        let add = if start.tick == end.tick {
            vec![LocatedEvent::from_position(start, EventData::TickGlide(start.column))]
        } else {
            vec![
                LocatedEvent::from_position(start, EventData::StartGlide(start.column)),
                LocatedEvent::from_position(end, EventData::EndGlide(start.column)),
            ]
        };

        let edit = match (interp_event_at_start, interp_event_at_end) {
            // if we're over an existing span, just delete it
            (Some(EventData::StartGlide(_)), Some(EventData::EndGlide(_)))
                | (Some(EventData::EndGlide(_)), Some(EventData::StartGlide(_)))
                | (Some(EventData::TickGlide(_)), Some(EventData::TickGlide(_)))=>
                Edit::PatternData {
                    remove: add.iter().map(|e| e.position()).collect(),
                    add: Vec::new(),
                },
            // otherwise, insert as normal
            _ => Edit::PatternData {
                remove: add.iter().map(|e| e.position()).collect(),
                add,
            },
            // TODO: other cases
        };

        module.push_edit(edit);
    }

    fn multi_channel_delete(&self, module: &mut Module) {
        let (mut start, mut end) = self.selection_corners();
        let n = module.tracks[self.edit_start.track].channels.len();
        let mut remove = Vec::new();

        for i in 0..n {
            start.channel = i;
            end.channel = i;
            for event in module.scan_events(start, end, true) {
                remove.push(event.position());
            }
        }

        module.push_edit(Edit::PatternData {
            remove,
            add: Vec::new()
        });
    }

    fn shift_values(&self, offset: i8, module: &mut Module) {
        let (start, end) = self.selection_corners();

        let replacements = module.scan_events(start, end, false).iter().filter_map(|evt| {
            match evt.event.data {
                EventData::Pitch(note) => Some(LocatedEvent {
                    event: Event {
                        data: EventData::Pitch(
                            note.step_shift(offset as isize, &module.tuning)),
                        ..evt.event
                    },
                    ..evt.clone()
                }),
                EventData::Pressure(v) => Some(LocatedEvent {
                    event: Event {
                        data: EventData::Pressure(
                            v.saturating_add_signed(offset).min(EventData::DIGIT_MAX)),
                        ..evt.event
                    },
                    ..evt.clone()
                }),
                EventData::Modulation(v) => Some(LocatedEvent {
                    event: Event {
                        data: EventData::Modulation(
                            v.saturating_add_signed(offset).min(EventData::DIGIT_MAX)),
                        ..evt.event
                    },
                    ..evt.clone()
                }),
                _ => None,
            }
        }).collect();

        module.push_edit(Edit::ReplaceEvents(replacements));
    }

    fn cycle_notation(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();

        let replacements = module.scan_events(start, end, false).iter().filter_map(|evt| {
            match evt.event.data {
                EventData::Pitch(note) => Some(LocatedEvent {
                    event: Event {
                        data: EventData::Pitch(note.cycle_notation(&module.tuning)),
                        ..evt.event
                    },
                    ..evt.clone()
                }),
                _ => None,
            }
        }).collect();

        module.push_edit(Edit::ReplaceEvents(replacements));
    }

    fn next_event(&mut self, module: &Module) {
        let tick = self.edit_start.tick;
        self.snap_to_event(module, |t| *t > tick);
    }

    fn prev_event(&mut self, module: &Module) {
        let tick = self.edit_start.tick;
        self.snap_to_event(module, |t| *t < tick);
    }

    fn snap_to_event(&mut self, module: &Module, filter_fn: impl Fn(&u32) -> bool) {
        let cursor = &mut self.edit_start;
        let tick = module.tracks[cursor.track].channels[cursor.channel].events.iter()
            .map(|e| e.tick)
            .filter(filter_fn)
            .min_by_key(|t| (*t as i32 - cursor.tick as i32).abs());

        if let Some(tick) = tick {
            if !is_shift_down() {
                self.edit_start.tick = tick;
            }
            self.edit_end.tick = tick;
            self.division_to_cursor();
            self.scroll_to_cursor();
        }
    }

    /// If the cursor tick is off-divison, set the division to the smallest
    /// division that contains the cursor tick.
    fn division_to_cursor(&mut self) {
        let tick = self.cursor_tick();
        if tick % self.ticks_per_row() != 0 {
            self.beat_division = 2;
            while tick % self.ticks_per_row() != 0 {
                self.beat_division += 1;
            }
        }
    }

    /// If the cursor tick is off-divison, set it to the nearest on-divison
    /// value.
    pub fn cursor_to_division(&mut self) {
        self.edit_start.tick = self.round_tick(self.edit_start.tick);
        self.edit_end.tick = self.round_tick(self.edit_end.tick);
    }

    pub fn round_tick(&self, tick: u32) -> u32 {
        let tpr = TICKS_PER_BEAT as f32 / self.beat_division as f32;
        let div = ((tick % TICKS_PER_BEAT) as f32 / tpr).round();
        (tick / TICKS_PER_BEAT) * TICKS_PER_BEAT + (div * tpr).round() as u32
    }

    fn off_division(&self, tick: u32) -> bool {
        tick != self.round_tick(tick)
    }

    fn ticks_per_row(&self) -> u32 {
        TICKS_PER_BEAT / self.beat_division as u32
    }

    fn select_all_channels(&mut self, module: &Module) {
        self.edit_start.track = 0;
        self.edit_start.channel = 0;
        self.edit_end.track = module.tracks.len() - 1;
        self.edit_end.channel = module.tracks[self.edit_end.track].channels.len() - 1;
    }

    fn place_events_evenly(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();
        let tick_delta = end.tick - start.tick + self.ticks_per_row();
        let events = module.scan_events(start, end, false);
        let channels: HashSet<_> = events.iter().map(|e| (e.track, e.channel)).collect();

        module.push_edit(Edit::PatternData {
            remove: events.iter().map(|e| e.position()).collect(),
            add: channels.into_iter().flat_map(|(track, channel)| {
                let events: Vec<_> = events.iter()
                    .filter(|e| e.track == track && e.channel == channel)
                    .collect();
                let n = events.len();

                events.into_iter().enumerate().map(move |(i, e)| LocatedEvent {
                    track,
                    channel,
                    event: Event {
                        tick: start.tick + tick_delta / n as u32 * i as u32,
                        data: e.event.data.clone()
                    }
                })
            }).collect(),
        })
    }

    fn handle_key(&mut self, key: KeyCode, module: &mut Module) {
        if !is_ctrl_down() {
            match key {
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
                KeyCode::A => input_digit(module, &self.edit_start, 0xa),
                KeyCode::B => input_digit(module, &self.edit_start, 0xb),
                KeyCode::C => input_digit(module, &self.edit_start, 0xc),
                KeyCode::D => input_digit(module, &self.edit_start, 0xd),
                KeyCode::E => input_digit(module, &self.edit_start, 0xe),
                KeyCode::F => input_digit(module, &self.edit_start, 0xf),
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
        let events = module.scan_events(start, end, true).iter().map(|x| ClipEvent {
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
            let event_positions: Vec<_> = module.scan_events(self.edit_start, end, true)
                .iter().map(|x| x.position()).collect();

            let add: Vec<_> = clip.events.iter().filter_map(|x| {
                self.edit_start.add_channels(x.channel_offset, &module.tracks)
                    .map(|pos| {
                        if x.event.data.is_ctrl() == (pos.track == 0)
                            && (!mix || !event_positions.contains(&Position {
                                tick: (x.event.tick as i32 + tick_offset) as u32,
                                ..pos
                            })) {
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
                event_positions
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

    fn draw_channel(&self, ui: &mut UI, channel: &Channel, muted: bool) {
        let beat_height = self.beat_height(ui);
        self.draw_channel_line(ui);
        self.draw_interpolation(ui, channel);

        // draw events
        for event in &channel.events {
            self.draw_event(ui, event, beat_height, muted);
        }
    }

    fn draw_channel_line(&self, ui: &mut UI) {
        let scroll = self.scroll(ui);
        ui.cursor_z -= 1;
        ui.push_line(ui.cursor_x + LINE_THICKNESS * 0.5, ui.cursor_y + scroll,
            ui.cursor_x + LINE_THICKNESS * 0.5, ui.cursor_y + scroll + ui.bounds.h,
            ui.style.theme.control_bg());
        ui.cursor_z += 1;
    }

    fn draw_interpolation(&self, ui: &mut UI, channel: &Channel) {
        ui.cursor_z -= 1;
        let beat_height = self.beat_height(ui);
        let tpr = self.ticks_per_row();
        let colors = [
            with_alpha(0.5, ui.style.theme.fg()),
            with_alpha(0.5, ui.style.theme.accent1_fg()),
            with_alpha(0.5, ui.style.theme.accent2_fg()),
        ];

        // TODO: would be better to only collect events once
        for col in 0..3 {
            let interp: Vec<_> = channel.interp_by_col(col).collect();
            let x = ui.cursor_x + ui.style.margin - 1.0 - LINE_THICKNESS * 0.5
                + column_x(col, &ui.style);
            let mut depth = 0;
            let mut start_tick = 0;

            let mut draw_line = |start: u32, end: u32| {
                let y1 = ui.cursor_y + (start + tpr / 4) as f32
                    / TICKS_PER_BEAT as f32 * beat_height;
                let y2 = ui.cursor_y + (end + tpr * 3 / 4) as f32
                    / TICKS_PER_BEAT as f32 * beat_height;
                ui.push_line(x, y1, x, y2, colors[col as usize]);
            };

            for event in interp {
                match event.data {
                    EventData::StartGlide(_) => {
                        if depth == 0 {
                            start_tick = event.tick;
                        }
                        depth += 1;
                    }
                    EventData::EndGlide(_) => {
                        depth -= 1;
                        if depth == 0 {
                            draw_line(start_tick, event.tick);
                        }
                    }
                    EventData::TickGlide(_) => if depth == 0 {
                        draw_line(event.tick, event.tick);
                    }
                    _ => panic!("expected glide event"),
                }
            }
        }

        ui.cursor_z += 1;
    }

    /// Returns scroll in pixels instead of in beats.
    fn scroll(&self, ui: &UI) -> f32 {
        self.beat_scroll * self.beat_height(ui) as f32
    }

    fn set_scroll(&mut self, scroll: f32, ui: &UI) {
        self.beat_scroll = scroll / self.beat_height(ui);
    }

    /// Scroll to a position that centers the given tick.
    fn scroll_to(&mut self, tick: u32) {
        // TODO: this should also be offset by half of the line height, or something
        let offset = (self.screen_tick_max - self.screen_tick) / 2;
        self.beat_scroll = tick.saturating_sub(offset) as f32 / TICKS_PER_BEAT as f32;
    }

    /// Inserts rows into the pattern, shifting events.
    fn push_rows(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();
        let ticks = end.tick - start.tick + TICKS_PER_BEAT / self.beat_division as u32;
        module.shift_channel_events(start, end, ticks as i32);
    }

    /// Deletes rows from the pattern, shifting events.
    fn pull_rows(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();
        let ticks = start.tick as i32 - end.tick as i32
            - TICKS_PER_BEAT as i32 / self.beat_division as i32;
        module.shift_channel_events(start, end, ticks);
    }

    fn input_note_off(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();
        let (start_tuple, end_tuple) = (start.x_tuple(), end.x_tuple());
        let mut add = Vec::new();

        for (track_i, track) in module.tracks.iter().enumerate() {
            for channel_i in 0..track.channels.len() {
                let tuple = (track_i, channel_i, NOTE_COLUMN);
                if track_i > 0 && tuple >= start_tuple && tuple <= end_tuple {
                    add.push(LocatedEvent {
                        track: track_i,
                        channel: channel_i,
                        event: Event {
                            tick: self.edit_start.tick,
                            data: EventData::NoteOff
                        }
                    });
                }
            }
        }

        module.push_edit(Edit::PatternData {
            remove: add.iter().map(|e| e.position()).collect(),
            add,
        });
    }

    fn record_event(&mut self, _key: Key, data: EventData, module: &mut Module) {
        let cursor = self.edit_start;
        if data.is_ctrl() != (cursor.track == 0) {
            return
        }

        // skip to next open row
        let mut pos = Position {
            track: cursor.track,
            tick: cursor.tick,
            channel: cursor.channel,
            column: data.logical_column(),
        };
        if module.event_at(pos).is_some_and(|e| e.data != EventData::NoteOff) {
            pos.tick += TICKS_PER_BEAT / self.beat_division as u32;
        }

        module.insert_event(cursor.track, cursor.channel, Event {
            tick: pos.tick,
            data,
        });
    }

    fn translate_cursor(&mut self, offset: i64) {
        if -offset > self.edit_end.tick as i64 {
            self.edit_end.tick = 0;
        } else {
            self.edit_end.tick =
                self.round_tick((self.edit_end.tick as i64 + offset) as u32);
        }

        if !is_shift_down() {
            self.edit_start.tick = self.edit_end.tick;
        }

        self.scroll_to_cursor();
    }

    /// If cursor is off-screen, scroll to center the cursor.
    fn scroll_to_cursor(&mut self) {
        let tick = self.cursor_tick();
        if !self.tick_visible(tick) {
            self.scroll_to(tick);
        }
    }

    fn tick_visible(&self, tick: u32) -> bool {
        tick >= self.screen_tick && tick <= self.screen_tick_max
    }

    fn draw_event(&self, ui: &mut UI, evt: &Event, beat_height: f32, muted: bool) {
        let y = ui.cursor_y + evt.tick as f32 / TICKS_PER_BEAT as f32 * beat_height;
        if y < 0.0 || y > ui.bounds.y + ui.bounds.h {
            return
        }
        let col = evt.data.spatial_column();
        let x = ui.cursor_x + column_x(col, &ui.style);
        if x < 0.0 || x > ui.bounds.x + ui.bounds.w {
            return
        }

        let mut color = match evt.data {
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
        if muted || self.off_division(evt.tick) {
            color = with_alpha(0.25, color);
        }

        let y = y - ui.style.margin + PATTERN_MARGIN;
        let text = match evt.data {
            EventData::Pitch(note) => {
                ui.push_note_text(x, y, &note, color);
                return
            },
            EventData::NoteOff => String::from(" ---"),
            EventData::Pressure(v) => format!("{:X}", v),
            EventData::Modulation(v) => format!("{:X}", v),
            EventData::End => String::from("End"),
            EventData::Loop => String::from("Loop"),
            EventData::Tempo(t) => t.round().to_string(),
            EventData::RationalTempo(n, d) => format!("{}:{}", n, d),
            EventData::InterpolatedPitch(_)
                | EventData::InterpolatedPressure(_)
                | EventData::InterpolatedModulation(_)
                => panic!("interpolated event in pattern"),
            EventData::StartGlide(_)
                | EventData::EndGlide(_)
                | EventData::TickGlide(_) => return,
        };
        ui.push_text(x, y, text, color);
    }
}

pub fn draw(ui: &mut UI, module: &mut Module, player: &mut Player, pe: &mut PatternEditor) {
    if let Some(interval) = pe.pending_interval.as_mut() {
        *interval += get_frame_time();
    }
    if pe.record && !player.is_playing() {
        pe.record = false;
    }

    if !ui.accepting_keyboard_input() {
        for key in get_keys_pressed() {
            pe.handle_key(key, module);
        }
    }

    let cursor = pe.edit_start;
    if pe.record {
        while let Some((key, data)) = ui.note_queue.pop() {
            pe.record_event(key, data, module);
        }
    } else {
        if !ui.accepting_note_input() && cursor.column == NOTE_COLUMN {
            while let Some((_, data)) = ui.note_queue.pop() {
                match data {
                    EventData::NoteOff => (),
                    _ => insert_event_at_cursor(module, &cursor, data),
                }
            }
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

    let beat_height = pe.beat_height(ui);
    let end_y = ui.bounds.h - ui.cursor_y
        + (module.last_event_tick().unwrap_or(0)
            .max(pe.edit_start.tick).max(pe.edit_end.tick) as f32)
        * beat_height / TICKS_PER_BEAT as f32;
    let viewport_h = ui.bounds.h + ui.bounds.y - ui.cursor_y;
    ui.push_line(ui.bounds.x, ui.cursor_y - LINE_THICKNESS * 0.5,
        ui.bounds.x + ui.bounds.w, ui.cursor_y - LINE_THICKNESS * 0.5,
        ui.style.theme.border_unfocused());
    if (pe.follow || pe.record) && player.is_playing() {
        pe.scroll_to(player.get_tick());
    }
    if pe.record {
        let ticks_per_row = pe.ticks_per_row();
        let tick = (player.get_tick() as f32 / ticks_per_row as f32)
            .round() as u32 * ticks_per_row;
        pe.edit_start.tick = tick;
        pe.edit_end.tick = tick;
    }
    let mut scroll = pe.scroll(ui);
    if !(pe.follow || pe.record) || !player.is_playing() {
        ui.vertical_scrollbar(&mut scroll, end_y, viewport_h, false);
        pe.set_scroll(scroll, ui);
    }
    let viewport = Rect {
        x: ui.bounds.x,
        y: ui.cursor_y,
        w: ui.bounds.w,
        h: viewport_h,
    };
    ui.cursor_z -= 1;
    ui.cursor_y -= scroll;

    pe.set_screen_ticks(viewport, ui);

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
    draw_beats(ui, left_x, beat_height);
    ui.cursor_z += 1;
    if player.is_playing() {
        draw_playhead(ui, player.get_tick(), left_x, beat_height);
    }
    pe.draw_cursor(ui, &track_xs);

    // draw channel data
    for (track_i, track) in module.tracks.iter().enumerate() {
        let chan_width = channel_width(track_i, &ui.style);
        for (channel_i, channel) in track.channels.iter().enumerate() {
            ui.cursor_x = track_xs[track_i] + chan_width * channel_i as f32;
            pe.draw_channel(ui, channel, player.track_muted(track_i));
        }
    }
    ui.cursor_x += channel_width(1, &ui.style);
    pe.draw_channel_line(ui);
}

/// Draws beat numbers and lines.
fn draw_beats(ui: &mut UI, x: f32, beat_height: f32) {
    let mut beat = 1;
    let mut y = ui.cursor_y;
    let line_height = line_height(&ui.style.atlas);
    while y < ui.bounds.y + ui.bounds.h {
        if y >= 0.0 {
            ui.push_rect(Rect {
                x: ui.bounds.x,
                y,
                w: ui.bounds.w,
                h: line_height,
            }, ui.style.theme.panel_bg(), None);
            ui.push_text(x, y - ui.style.margin + PATTERN_MARGIN, beat.to_string(),
                ui.style.theme.fg());
        }
        beat += 1;
        y += beat_height;
    }
}

/// Returns x positions of each track, plus one extra position.
fn draw_track_headers(ui: &mut UI, module: &mut Module, player: &mut Player) -> Vec<f32> {
    let mut edit = None;
    ui.layout = Layout::Horizontal;

    // offset for beat width
    ui.cursor_x += ui.style.atlas.char_width() * 3.0 + ui.style.margin * 2.0;

    let mut xs = vec![ui.cursor_x];
    xs.extend(module.tracks.iter_mut().enumerate().map(|(i, track)| {
        ui.start_group();

        // track name & delete button
        let name = track_name(track.target, &module.patches);
        match track.target {
            TrackTarget::Patch(_) | TrackTarget::None => {
                ui.start_group();
                if let Some(j) = ui.combo_box(&format!("track_{}", i), "", name,
                    || track_targets(&module.patches)) {
                    edit = Some(Edit::RemapTrack(i, match j {
                        0 => TrackTarget::None,
                        j => TrackTarget::Patch(j - 1),
                    }));
                }
                if ui.button("X", true, Info::Remove("this track")) {
                    edit = Some(Edit::RemoveTrack(i));
                }
                ui.end_group();
            }
            TrackTarget::Global => ui.offset_label(name, Info::GlobalTrack),
            TrackTarget::Kit => ui.offset_label(name, Info::KitTrack),
        }

        // chanel add/remove buttons
        ui.start_group();
        if ui.button("-", track.channels.len() > 1, Info::Remove("the last channel")) {
            edit = Some(Edit::RemoveChannel(i));
        }
        if ui.button("+", true, Info::Add("a new channel")) {
            edit = Some(Edit::AddChannel(i, Channel::new()));
        }
        ui.end_group();

        // column labels
        ui.start_group();
        for _ in 0..track.channels.len() {
            let color = ui.style.theme.border_unfocused();
            if i == 0 {
                ui.colored_label("Ctrl", color)
            } else {
                ui.colored_label("Note", color);
                ui.cursor_x -= ui.style.margin;
                ui.colored_label("P", color);
                ui.cursor_x -= ui.style.margin;
                ui.colored_label("M", color);
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

    if ui.button("+", !module.patches.is_empty(), Info::Add("a new track")) {
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

fn nudge_notes(module: &mut Module, (start, end): (Position, Position), cfg: &Config) {
    let replacements = module.scan_events(start, end, false).iter().filter_map(|evt| {
        if let EventData::Pitch(note) = evt.event.data {
            Some(LocatedEvent {
                event: Event {
                    data: EventData::Pitch(input::adjust_note_for_modifier_keys(note, cfg)),
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

    let n = module.tracks[cursor.track].channels.len();
    if is_shift_down() && n > 1 {
        // hold shift to insert events into all track channels
        let add: Vec<_> = (0..n).map(|i| LocatedEvent {
            track: cursor.track,
            channel: i,
            event: Event {
                tick: cursor.tick,
                data: data.clone(),
            },
        }).collect();
        module.push_edit(Edit::PatternData {
            remove: add.iter().map(|e| e.position()).collect(),
            add,
        });
    } else {
        module.insert_event(cursor.track, cursor.channel, Event {
            tick: cursor.tick,
            data,
        });
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

fn draw_playhead(ui: &mut UI, tick: u32, x: f32, beat_height: f32) {
    let rect = Rect {
        x,
        y: ui.cursor_y + tick as f32 / TICKS_PER_BEAT as f32 * beat_height,
        w: ui.bounds.w,
        h: line_height(&ui.style.atlas),
    };
    let color = Color { a: 0.1, ..ui.style.theme.fg() };
    ui.push_rect(rect, color, None);
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
fn position_coords(pos: Position, style: &Style, track_xs: &[f32],
    bottom_left: bool, beat_height: f32
) -> Vec2 {
    let x = track_xs[pos.track] + channel_width(pos.track, style) * pos.channel as f32
        + if bottom_left {
            column_x(pos.column + 1, style) - style.margin
        } else {
            column_x(pos.column, style)
        };
    let y = pos.beat() * beat_height + if bottom_left {
        line_height(&style.atlas)
    } else {
        0.0
    };
    Vec2 { x, y }
}

fn channel_width(track_index: usize, style: &Style) -> f32 {
    if track_index == 0 {
        column_x(1, style) + style.margin
    } else {
        column_x(3, style) + style.margin
    }
}

fn column_x(column: u8, style: &Style) -> f32 {
    let char_width = style.atlas.char_width();
    let margin = style.margin;

    match column {
        NOTE_COLUMN => 0.0,
        VEL_COLUMN => char_width * 4.0 + margin,
        MOD_COLUMN => char_width * 5.0 + margin * 2.0,
        // allow this to make some calculations easier
        3 => char_width * 6.0 + margin * 3.0,
        _ => panic!("invalid cursor column"),
    }
}

fn with_alpha(a: f32, color: Color) -> Color {
    Color {
        a: color.a * a,
        ..color
    }
}

fn line_height(atlas: &GlyphAtlas) -> f32 {
    atlas.cap_height() + PATTERN_MARGIN * 2.0
}