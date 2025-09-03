use std::collections::HashSet;

use fundsp::math::delerp;

use crate::{config::Config, input::{self, Action}, module::*, synth::Patch, timespan::Timespan};

use super::*;

/// Narrower margin used in the pattern grid.
const PATTERN_MARGIN: f32 = 2.0;

const CTRL_COLUMN_TEXT_ID: &str = "ctrl_column";

/// These actions are valid ways to exit pattern text entry.
/// Defining what's on this list is a little hairy since there are pattern
/// navigation actions that are bound to useful text editing keys by default,
/// but they don't *have* to be. And any of these actions could be rebound to
/// conflict with text edit keys.
const TEXT_EXIT_ACTIONS: [Action; 8] = [
    Action::PrevRow,
    Action::NextRow,
    Action::PrevChannel,
    Action::NextChannel,
    Action::PrevBeat,
    Action::NextBeat,
    Action::PrevEvent,
    Action::NextEvent,
];

/// State specific to the pattern view.
pub struct PatternEditor {
    edit_start: Position,
    edit_end: Position,
    pub beat_division: u8,
    pub edit_step: u8,
    beat_scroll: Timespan,
    h_scroll: f32,
    tap_tempo_intervals: Vec<f32>,
    /// For tap tempo.
    pending_interval: Option<f32>,
    clipboard: Option<PatternClip>,
    pub follow: bool,
    record: bool,
    /// Highest visible tick. Lowest is `beat_scroll`.
    screen_tick_max: Timespan,
    text_position: Option<Position>,
}

/// Pattern data clipboard.
struct PatternClip {
    start: Position,
    end: Position,
    events: Vec<ClipEvent>,
    channels: usize,
}

/// Different behavior variants for the paste command.
#[derive(PartialEq)]
enum PasteMode {
    Normal,
    Mix,
    Stretch,
}

/// Event in the pattern data clipboard.
#[derive(Debug)]
struct ClipEvent {
    channel_offset: usize,
    event: Event,
}

impl Default for PatternEditor {
    fn default() -> Self {
        let edit_cursor = Position {
            tick: Timespan::ZERO,
            track: 0,
            channel: 0,
            column: 0,
        };
        Self {
            edit_start: edit_cursor,
            edit_end: edit_cursor,
            beat_division: 4,
            edit_step: 0,
            beat_scroll: Timespan::ZERO,
            h_scroll: 0.0,
            tap_tempo_intervals: Vec::new(),
            pending_interval: None,
            clipboard: None,
            follow: false,
            record: false,
            screen_tick_max: Timespan::ZERO,
            text_position: None,
        }
    }
}

impl PatternEditor {
    /// Increment division.
    pub fn inc_division(&mut self) {
        self.set_division(self.beat_division.saturating_add(1));
    }

    /// Decrement division.
    pub fn dec_division(&mut self) {
        self.set_division(self.beat_division - 1);
    }

    pub fn double_division(&mut self) {
        self.set_division(self.beat_division.saturating_mul(2));
    }

    pub fn halve_division(&mut self) {
        self.set_division(self.beat_division / 2);
    }

    /// Set division, adjusting other parameters as necessary.
    pub fn set_division(&mut self, division: u8) {
        let division = division.max(1);

        // the tricky part here is to preserve the visual position of either
        // the cursor or the center of the viewport
        let isotick = if self.tick_visible(self.edit_end.tick) {
            let n = (self.edit_end.tick.as_f64() * division as f64).round() as i32;
            Timespan::new(n, division)
        } else {
            (self.beat_scroll + self.screen_tick_max) * Timespan::new(1, 2)
        };
        let old_pos = delerp(self.beat_scroll.as_f32(), self.screen_tick_max.as_f32(),
            isotick.as_f32());

        self.screen_tick_max = self.beat_scroll
            + (self.screen_tick_max - self.beat_scroll)
            * Timespan::new(self.beat_division as i32, division);
        self.beat_division = division;
        self.cursor_to_division();

        let new_pos = delerp(self.beat_scroll.as_f32(), self.screen_tick_max.as_f32(),
            isotick.as_f32());
        let offset = (self.screen_tick_max - self.beat_scroll)
            * Timespan::approximate((new_pos - old_pos).into());
        self.beat_scroll = (self.beat_scroll + offset).max(Timespan::ZERO);
    }

    /// Returns the track the cursor is in.
    pub fn cursor_track(&self) -> usize {
        self.edit_start.track
    }

    /// Returns the tick the cursor is on.
    pub fn cursor_tick(&self) -> Timespan {
        self.edit_start.tick
    }

    /// Check whether the cursor is in the digit column.
    pub fn in_digit_column(&self, ui: &Ui) -> bool {
        ui.tabs.get(MAIN_TAB_ID) == Some(&TAB_PATTERN)
            && self.edit_start.column != NOTE_COLUMN
    }

    /// Check whether the cursor is in the global track.
    pub fn in_global_track(&self, ui: &Ui) -> bool {
        ui.tabs.get(MAIN_TAB_ID) == Some(&TAB_PATTERN)
            && self.edit_start.track == 0
    }

    /// Return the current height of a beat, in pixels.
    fn beat_height(&self, ui: &Ui) -> f32 {
        line_height(&ui.style.atlas) * self.beat_division as f32
    }

    /// Convert mouse coordinates to a Position.
    fn position_from_mouse(&self, ui: &Ui, track_xs: &[f32], tracks: &[Track])
    -> Position {
        let (x, y) = mouse_position();
        let mut pos = Position {
            tick: self.round_tick(self.y_tick(y, ui)),
            ..Default::default()
        };

        // skip last track_x since it's not the start of a track
        for (i, tx) in track_xs.split_last().unwrap().1.iter().enumerate() {
            if x >= *tx {
                let chan_width = channel_width(i, &ui.style);
                pos.track = i;
                pos.channel = (tracks[i].channels.len() - 1)
                    .min(((x - tx) / chan_width) as usize);
                pos.column = if i == 0 {
                    GLOBAL_COLUMN
                } else {
                    let x = x - tx - pos.channel as f32 * chan_width;
                    if column_x(2, &ui.style) < x {
                        MOD_COLUMN
                    } else if column_x(1, &ui.style) < x {
                        VEL_COLUMN
                    } else {
                        NOTE_COLUMN
                    }
                };
            }
        }

        pos
    }

    /// Returns the beat position of a vertical screen position.
    fn y_tick(&self, y: f32, ui: &Ui) -> Timespan {
        let beat_height = self.beat_height(ui);
        let f = (y - ui.cursor_y - line_height(&ui.style.atlas) * 0.5) / beat_height;
        Timespan::approximate(f.into())
    }

    /// Returns the tick of the first beat on-screen.
    pub fn screen_beat_tick(&self) -> Timespan {
        Timespan::new(self.beat_scroll.as_f64().ceil() as i32, 1)
    }

    /// Cache viewport data from this frame.
    fn set_metrics(&mut self, viewport: Rect, ui: &Ui) {
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

    /// As `selection_corners`, but the end position is offset by the row
    /// timespan if the start and end ticks are unequal.
    fn selection_corners_with_tail(&self) -> (Position, Position) {
        let (start, mut end) = self.selection_corners();
        if start.tick != end.tick {
            end.tick += self.row_timespan();
        }
        (start, end)
    }

    /// Draws the cursor/selection.
    fn draw_cursor(&self, ui: &mut Ui, track_xs: &[f32]) {
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

    /// Handles a pattern-editor-specific action.
    pub fn action(&mut self, action: Action, module: &mut Module, cfg: &Config,
        player: &mut PlayerShell, ui: &mut Ui
    ) {
        match action {
            Action::Cut => self.cut(module),
            Action::Copy => self.copy(module),
            Action::Paste => self.paste(module, PasteMode::Normal),
            Action::MixPaste => self.paste(module, PasteMode::Mix),
            Action::InsertPaste => {
                self.selection_to_clip(module);
                self.push_rows(module);
                self.paste(module, PasteMode::Normal);
            },
            Action::StretchPaste => self.paste(module, PasteMode::Stretch),
            Action::PrevRow => self.translate_cursor(
                -self.step_timespan().max(self.row_timespan())),
            Action::NextRow => self.translate_cursor(
                self.step_timespan().max(self.row_timespan())),
            Action::PrevColumn => shift_column_left(
                &mut self.edit_start, &mut self.edit_end, &module.tracks),
            Action::NextColumn => shift_column_right(
                &mut self.edit_start, &mut self.edit_end, &module.tracks),
            Action::NextChannel => shift_channel_right(
                &mut self.edit_start, &mut self.edit_end, &module.tracks),
            Action::PrevChannel => shift_channel_left(
                &mut self.edit_start, &mut self.edit_end, &module.tracks),
            Action::Delete => {
                let (start, end) = self.selection_corners_with_tail();
                if start.x_tuple() == end.x_tuple() && is_shift_down() {
                    self.multi_channel_delete(module);
                } else {
                    module.delete_events(start, end);
                }
            },
            Action::NoteOff => self.input_note_off(module, is_shift_down()),
            Action::End => self.input_and_step(module, EventData::End, false),
            Action::Loop => self.input_and_step(module, EventData::Loop, false),
            Action::TapTempo => self.tap_tempo(module),
            Action::InsertRows => self.push_rows(module),
            Action::DeleteRows => self.pull_rows(module),
            Action::NudgeArrowUp | Action::NudgeArrowDown
                | Action::NudgeSharp | Action::NudgeFlat
                | Action::NudgeOctaveUp | Action::NudgeOctaveDown
                | Action::NudgeEnharmonic =>
                    nudge_notes(module, self.selection_corners_with_tail(), cfg),
            Action::ToggleFollow => self.follow = !self.follow,
            // TODO: re-enable this if & when recording is implemented
            // Action::ToggleRecord => if self.record {
            //     player.stop();
            //     self.record = false;
            // } else {
            //     player.record_from(self.cursor_tick(), module);
            //     self.record = true;
            // },
            Action::SelectAllChannels => self.select_all_channels(module),
            Action::SelectAllRows => self.select_all_rows(module),
            Action::PlaceEvenly => self.place_events_evenly(module),
            Action::NextBeat => self.translate_cursor(Timespan::new(1, 1)),
            Action::PrevBeat => self.translate_cursor(Timespan::new(-1, 1)),
            Action::NextEvent => self.next_event(module),
            Action::PrevEvent => self.prev_event(module),
            Action::PatternStart => self.translate_cursor(-self.cursor_tick()),
            Action::PatternEnd => if let Some(tick) = module.last_event_tick() {
                self.translate_cursor(tick - self.cursor_tick());
            }
            Action::IncrementValues => self.shift_values(1, module),
            Action::DecrementValues => self.shift_values(-1, module),
            Action::Interpolate => self.interpolate(module),
            Action::MuteTrack => player.toggle_mute(self.cursor_track()),
            Action::SoloTrack => player.toggle_solo(self.cursor_track()),
            Action::UnmuteAllTracks => player.unmute_all(),
            Action::CycleNotation => self.cycle_notation(module),
            Action::UseLastNote => self.use_last_note(module),
            Action::ShiftTrackLeft => self.shift_track(-1, module, player),
            Action::ShiftTrackRight => self.shift_track(1, module, player),
            Action::FocusDivision => ui.focus("Division"),
            Action::FocusEditStep => ui.focus("Step"),
            _ => (),
        }

        if action != Action::TapTempo {
            self.clear_tap_tempo_state();
        }
    }

    fn input_and_step(&mut self, module: &mut Module, data: EventData, all_channels: bool) {
        if insert_event_at_cursor(module, &self.edit_start, data, all_channels) {
            self.translate_cursor(self.step_timespan());

            // in this case, autostep creates a selection we don't want
            if all_channels && self.edit_step > 0 {
                self.edit_start = self.edit_end;
            }
        }
    }

    fn step_timespan(&self) -> Timespan {
        self.row_timespan() * Timespan::new(self.edit_step.into(), 1)
    }

    fn shift_track(&mut self, offset: isize,
        module: &mut Module, player: &mut PlayerShell
    ) {
        let src_track = self.cursor_track();
        let dst_track = src_track.saturating_add_signed(offset);
        if src_track > 1 && dst_track > 1 && dst_track < module.tracks.len() {
            module.push_edit(Edit::ShiftTrack(src_track, offset));
            player.update_synths(module.drain_track_history());
            self.edit_start.track = self.edit_start.track.wrapping_add_signed(offset);
            self.edit_end.track = self.edit_end.track.wrapping_add_signed(offset);
            fix_cursors(&mut self.edit_start, &mut self.edit_end, &module.tracks);
        }
    }

    fn clear_tap_tempo_state(&mut self) {
        self.tap_tempo_intervals.clear();
        self.pending_interval = None;
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
                        track: module.tracks.len() - 1,
                        channel: module.tracks.last().unwrap().channels.len() - 1,
                        tick: Timespan::ZERO,
                        column: 0,
                    })
            };
            // compensate for selection tail
            if self.edit_end.tick >= self.edit_start.tick + self.row_timespan() {
                self.edit_end.tick += -self.row_timespan();
            }
        }
    }

    /// Handle the Interpolate key command.
    fn interpolate(&self, module: &mut Module) {
        let (mut start, end) = self.selection_corners();
        let mut remove = Vec::new();
        let mut add = Vec::new();

        // iterate over columns
        while start.x_tuple() <= end.x_tuple() {
            let mut end = Position {
                tick: end.tick,
                ..start
            };
            let mut skip = false;

            if start.tick == end.tick && (
                start.column > 0
                || start.track == 0
                || module.event_at(&start).is_none()
            ) {
                // interpolate to next event in column
                let evt = module.tracks[start.track]
                    .channels[start.channel]
                    .events.iter()
                    .find(|e| e.tick > start.tick
                        && e.data.logical_column() == start.column);

                if let Some(evt) = evt {
                    end.tick = evt.tick;
                } else {
                    skip = true;
                }
            }

            if !skip {
                let start_interp_pos = Position {
                    column: start.column | EventData::INTERP_COL_FLAG,
                    ..start
                };
                let interp_event_at_start =
                    module.event_at(&start_interp_pos).map(|e| e.data.clone());

                if start.tick == end.tick {
                    match interp_event_at_start {
                        None => add.push(LocatedEvent::from_position(start,
                            EventData::TickGlide(start.column))),
                        Some(EventData::TickGlide(_)) => remove.push(start_interp_pos),
                        _ => (),
                    }
                } else {
                    let end_interp_pos = Position {
                        column: start.column | EventData::INTERP_COL_FLAG,
                        ..end
                    };
                    let interp_event_at_end =
                        module.event_at(&end_interp_pos).map(|e| e.data.clone());

                    if matches!((&interp_event_at_start, &interp_event_at_end),
                        (Some(EventData::StartGlide(_)), Some(EventData::EndGlide(_)))
                        | (Some(EventData::EndGlide(_)), Some(EventData::StartGlide(_)))) {
                        remove.push(start_interp_pos);
                        remove.push(end_interp_pos);
                    } else {
                        match interp_event_at_start {
                            None => add.push(LocatedEvent::from_position(start,
                                EventData::StartGlide(start.column))),
                            Some(EventData::TickGlide(_)) => {
                                add.push(LocatedEvent::from_position(start,
                                    EventData::StartGlide(start.column)));
                                remove.push(start_interp_pos);
                            }
                            Some(EventData::EndGlide(_)) => remove.push(start_interp_pos),
                            _ => (),
                        }
                        match interp_event_at_end {
                            None => add.push(LocatedEvent::from_position(end,
                                EventData::EndGlide(start.column))),
                            Some(EventData::TickGlide(_)) => {
                                add.push(LocatedEvent::from_position(end,
                                    EventData::EndGlide(start.column)));
                                remove.push(end_interp_pos);
                            }
                            Some(EventData::StartGlide(_)) => remove.push(end_interp_pos),
                            _ => (),
                        }
                    }
                };
            }

            if let Some(pos) = start.add_channels(1, &module.tracks) {
                start = pos
            } else {
                break
            }
        }

        module.push_edit(Edit::PatternData { remove, add });
    }

    /// Delete in each channel of the current track.
    fn multi_channel_delete(&self, module: &mut Module) {
        let (mut start, mut end) = self.selection_corners_with_tail();
        let n = module.tracks[self.edit_start.track].channels.len();
        let mut remove = Vec::new();

        for i in 0..n {
            start.channel = i;
            end.channel = i;
            for event in module.scan_events(start, end) {
                remove.push(event.position());
            }
        }

        module.push_edit(Edit::PatternData {
            remove,
            add: Vec::new()
        });
    }

    /// Handle the "increment/decrement values" key commands.
    fn shift_values(&self, offset: i8, module: &mut Module) {
        let (start, end) = self.selection_corners_with_tail();

        let replacements = module.scan_events(start, end).iter().filter_map(|evt| {
            let mut evt = evt.clone();

            match &mut evt.event.data {
                EventData::Pitch(note) => {
                    *note = note.step_shift(offset as isize, &module.tuning);
                    Some(evt)
                }
                EventData::Pressure(v) => {
                    *v = v.saturating_add_signed(offset).min(EventData::DIGIT_MAX);
                    Some(evt)
                }
                EventData::Modulation(v) => {
                    *v = v.saturating_add_signed(offset).min(EventData::DIGIT_MAX);
                    Some(evt)
                }
                EventData::Tempo(t) => {
                    *t = (*t + offset as f32).max(1.0);
                    Some(evt)
                }
                EventData::RationalTempo(n, _) => {
                    *n = n.saturating_add_signed(offset).max(1);
                    Some(evt)
                }
                _ => None,
            }
        }).collect();

        module.push_edit(Edit::ReplaceEvents(replacements));
    }

    /// Handle the "cycle notation" key command.
    fn cycle_notation(&self, module: &mut Module) {
        let (start, end) = self.selection_corners_with_tail();

        let replacements = module.scan_events(start, end).into_iter()
            .filter_map(|mut evt| {
                match &mut evt.event.data {
                    EventData::Pitch(note) => {
                        *note = note.cycle_notation(&module.tuning);
                        Some(evt)
                    },
                    _ => None,
                }
            }).collect();

        module.push_edit(Edit::ReplaceEvents(replacements));
    }

    /// Handle the "next event" key command.
    fn next_event(&mut self, module: &Module) {
        let tick = self.edit_end.tick;
        self.snap_to_event(module, |t| *t > tick);
    }

    /// Handle the "previous event" key command.
    fn prev_event(&mut self, module: &Module) {
        let tick = self.edit_end.tick;
        self.snap_to_event(module, |t| *t < tick);
    }

    /// Snap cursor to the closest channel event whose position matches `filter_fn`.
    fn snap_to_event(&mut self, module: &Module, filter_fn: impl Fn(&Timespan) -> bool) {
        let cursor = &mut self.edit_end;
        let tick = module.tracks[cursor.track].channels[cursor.channel].events.iter()
            .map(|e| e.tick)
            .filter(filter_fn)
            .min_by_key(|t| (*t - cursor.tick).abs());

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
        let ticks = [self.edit_start.tick, self.edit_end.tick];

        if ticks.iter().any(|t| self.off_division(*t)) {
            let old_div = self.beat_division;
            self.beat_division = 2;

            while ticks.iter().any(|t| self.off_division(*t)) {
                self.beat_division += 1;
            }

            let div = self.beat_division;
            self.beat_division = old_div;
            self.set_division(div); // to set screen ticks
        }
    }

    /// If the cursor tick is off-divison, set it to the nearest on-divison
    /// value.
    pub fn cursor_to_division(&mut self) {
        self.edit_start.tick = self.round_tick(self.edit_start.tick);
        self.edit_end.tick = self.round_tick(self.edit_end.tick);
    }

    /// Round a tick to the nearest on-division value.
    pub fn round_tick(&self, tick: Timespan) -> Timespan {
        let n = (tick.as_f64() * self.beat_division as f64).round() as i32;
        Timespan::new(n, self.beat_division)
    }

    fn off_division(&self, tick: Timespan) -> bool {
        self.beat_division % tick.den() != 0
    }

    fn select_all_channels(&mut self, module: &Module) {
        self.edit_start.track = 0;
        self.edit_start.channel = 0;
        self.edit_start.column = GLOBAL_COLUMN;
        self.edit_end.track = module.tracks.len() - 1;
        self.edit_end.channel = module.tracks[self.edit_end.track].channels.len() - 1;
        self.edit_end.column = MOD_COLUMN;
    }

    fn select_all_rows(&mut self, module: &Module) {
        self.edit_start.tick = Timespan::ZERO;
        self.edit_end.tick = module.last_event_tick().unwrap_or_default();
    }

    /// Return the current timespan of a single row.
    fn row_timespan(&self) -> Timespan {
        Timespan::new(1, self.beat_division)
    }

    /// Handle the "place events evenly" key command.
    fn place_events_evenly(&self, module: &mut Module) {
        let (start, end) = self.selection_corners_with_tail();
        let tick_delta = end.tick - start.tick;
        let events = module.scan_events(start, end);
        let channels: HashSet<_> = events.iter().map(|e| (e.track, e.channel)).collect();

        module.push_edit(Edit::PatternData {
            remove: events.iter().map(|e| e.position()).collect(),
            add: channels.into_iter().flat_map(|(track, channel)| {
                let mut events: Vec<_> = events.iter()
                    .filter(|e| e.track == track && e.channel == channel)
                    .cloned()
                    .collect();
                let mut ticks: Vec<_> = events.iter().map(|e| e.event.tick).collect();
                ticks.dedup();
                let n = ticks.len();

                for evt in &mut events {
                    let i = ticks.iter().position(|t| *t == evt.event.tick)
                        .expect("event tick should have been collected");
                    evt.event.tick =
                        start.tick + tick_delta * Timespan::new(i as i32, n as u8);
                }

                events
            }).collect(),
        })
    }

    /// Handle raw keys for digit input.
    fn handle_key(&mut self, key: KeyCode, module: &mut Module, ui: &mut Ui) {
        if !(is_ctrl_down() || is_alt_down()) {
            let value = match key {
                KeyCode::Key0 => 0,
                KeyCode::Key1 => 1,
                KeyCode::Key2 => 2,
                KeyCode::Key3 => 3,
                KeyCode::Key4 => 4,
                KeyCode::Key5 => 5,
                KeyCode::Key6 => 6,
                KeyCode::Key7 => 7,
                KeyCode::Key8 => 8,
                KeyCode::Key9 => 9,
                KeyCode::A => 0xa,
                KeyCode::B => 0xb,
                KeyCode::C => 0xc,
                KeyCode::D => 0xd,
                KeyCode::E => 0xe,
                KeyCode::F => 0xf,
                _ => return,
            };

            match self.edit_start.column {
                VEL_COLUMN => self.input_and_step(
                    module, EventData::Pressure(value), is_shift_down()),
                MOD_COLUMN => self.input_and_step(
                    module, EventData::Modulation(value), is_shift_down()),
                GLOBAL_COLUMN => if self.edit_start.track == 0 && value < 10 {
                    self.text_position = Some(self.edit_start);
                    ui.focus_text(CTRL_COLUMN_TEXT_ID.into(), value.to_string());
                },
                _ => (),
            }
        }
    }

    /// Handle a tempo tap.
    fn tap_tempo(&mut self, module: &mut Module) {
        if let Some(interval) = self.pending_interval {
            self.tap_tempo_intervals.push(interval);
            let n = self.tap_tempo_intervals.len();
            let mean = self.tap_tempo_intervals.iter().sum::<f32>() / n as f32;
            let t = 60.0 / mean;
            insert_event_at_cursor(module, &self.edit_start, EventData::Tempo(t), false);
        }
        self.pending_interval = Some(0.0);
    }

    /// Cut selection to the clipboard.
    fn cut(&mut self, module: &mut Module) {
        self.copy(module);
        let (start, end) = self.selection_corners_with_tail();
        module.delete_events(start, end);
    }

    /// Copy selection to the clipboard.
    fn copy(&mut self, module: &Module) {
        let (start, end) = self.selection_corners_with_tail();
        let events = module.scan_events(start, end).iter().map(|x| ClipEvent {
            channel_offset: module.channels_between(start, x.position()),
            event: x.event.clone(),
        }).collect();
        self.clipboard = Some(PatternClip {
            start,
            end,
            events,
            channels: module.channels_between(start, end),
        });
    }

    /// Paste from the clipboard.
    fn paste(&self, module: &mut Module, mode: PasteMode) {
        if let Some(clip) = &self.clipboard {
            let (start, end) = self.selection_corners_with_tail();
            let start = Position {
                column: clip.start.column,
                ..start
            };
            let end = Position {
                tick: match mode {
                    PasteMode::Stretch => end.tick,
                    _ => start.tick + clip.end.tick - clip.start.tick,
                },
                column: clip.end.column,
                ..start.add_channels(clip.channels, &module.tracks)
                    .unwrap_or(Position {
                        track: module.tracks.len() - 1,
                        channel: module.tracks.last().unwrap().channels.len() - 1,
                        ..Default::default()
                    })
            };

            let event_positions: Vec<_> = module.scan_events(start, end)
                .iter().map(|x| x.position()).collect();
            let scale = if mode == PasteMode::Stretch && end.tick != start.tick {
                (end.tick - start.tick) / (clip.end.tick - clip.start.tick)
            } else {
                Timespan::new(1, 1)
            };

            let add: Vec<_> = clip.events.iter().filter_map(|x| {
                let start_offset = x.event.tick - clip.start.tick;
                let tick = start.tick + start_offset * scale;
                start.add_channels(x.channel_offset, &module.tracks)
                    .and_then(|pos| {
                        if x.event.data.goes_in_track(pos.track)
                            && (mode != PasteMode::Mix
                                || !event_positions.contains(&Position {
                                    tick,
                                    ..pos
                                })) {
                            Some(LocatedEvent {
                                track: pos.track,
                                channel: pos.channel,
                                event: Event {
                                    tick,
                                    data: x.event.data.clone(),
                                },
                            })
                        } else {
                            None
                        }
                    })
            }).collect();

            let remove = if mode == PasteMode::Mix {
                add.iter().map(|x| x.position()).collect()
            } else {
                event_positions
            };

            if !add.is_empty() || !remove.is_empty() {
                module.push_edit(Edit::PatternData {
                    remove,
                    add,
                });
            }
        }
    }

    fn draw_channel(&self, ui: &mut Ui, channel: &Channel, muted: bool, index: usize) {
        self.draw_channel_line(ui, index == 0);
        self.draw_interpolation(ui, channel);
        let beat_height = self.beat_height(ui);
        for event in &channel.events {
            self.draw_event(ui, event, beat_height, muted);
        }
    }

    /// Draw a vertical line to separate channels.
    fn draw_channel_line(&self, ui: &mut Ui, track_boundary: bool) {
        let scroll = self.scroll(ui);
        ui.cursor_z -= 1;
        let color = if track_boundary {
            ui.style.theme.panel_bg_hover()
        } else {
            ui.style.theme.control_bg()
        };
        ui.push_line(ui.cursor_x + LINE_THICKNESS * 0.5, ui.cursor_y + scroll,
            ui.cursor_x + LINE_THICKNESS * 0.5, ui.cursor_y + scroll + ui.bounds.h,
            color);
        ui.cursor_z += 1;
    }

    /// Draw all interpolation lines for a channel.
    fn draw_interpolation(&self, ui: &mut Ui, channel: &Channel) {
        const NUM_COLS: usize = 3;

        ui.cursor_z -= 1;
        let beat_height = self.beat_height(ui);
        let tpr = self.row_timespan();
        let colors = [
            Color { a: 0.5, ..ui.style.theme.fg() },
            Color { a: 0.5, ..ui.style.theme.accent1_fg() },
            Color { a: 0.5, ..ui.style.theme.accent2_fg() },
        ];

        let mut interp: Vec<_> = (0..NUM_COLS).map(|_| Vec::new()).collect();
        for evt in &channel.events {
            if let EventData::StartGlide(i)
                | EventData::EndGlide(i)
                | EventData::TickGlide(i) = evt.data {
                interp[i as usize].push(evt)
            }
        }

        for col in 0..NUM_COLS {
            let mut start_tick = None;
            let x = ui.cursor_x + ui.style.margin - 1.0 - LINE_THICKNESS * 0.5
                + column_x(col as u8, &ui.style);

            // normally it would make sense to have one graphics vector scoped
            // outside the loop, but the closures require this approach.
            let mut lines = Vec::new();
            let mut marks = Vec::new();

            let mut draw_line = |start: Timespan, end: Timespan| {
                let y1 = ui.cursor_y
                    + (start + tpr * Timespan::new(1, 4)).as_f32() * beat_height;
                let y2 = ui.cursor_y
                    + (end + tpr * Timespan::new(3, 4)).as_f32() * beat_height;
                lines.push(Graphic::Line(x, y1, x, y2, colors[col as usize]));
            };

            let mut draw_dup = |tick: Timespan| {
                let offset = ui.style.margin * 0.5;
                let (x1, x2) = (x - offset, x + offset);
                let y = (ui.cursor_y
                    + (tick + tpr * Timespan::new(1, 2)).as_f32() * beat_height).round()
                    + LINE_THICKNESS * 0.5;
                marks.push(Graphic::Line(x1, y, x2, y, colors[col as usize]));
            };

            for event in &interp[col] {
                match event.data {
                    EventData::StartGlide(_) => {
                        if start_tick.is_none() {
                            start_tick = Some(event.tick);
                        } else {
                            draw_dup(event.tick);
                        }
                    }
                    EventData::EndGlide(_) => {
                        if let Some(start_tick) = start_tick.take() {
                            draw_line(start_tick, event.tick);
                        } else {
                            draw_dup(event.tick);
                        }
                    }
                    EventData::TickGlide(_) => if start_tick.is_none() {
                        draw_line(event.tick, event.tick);
                    }
                    _ => panic!("expected glide event"),
                }
            }

            if let Some(start_tick) = start_tick {
                draw_line(start_tick, self.screen_tick_max);
            }

            ui.push_graphics(lines);
            ui.push_graphics(marks);
        }

        ui.cursor_z += 1;
    }

    /// Returns scroll in pixels instead of in beats.
    fn scroll(&self, ui: &Ui) -> f32 {
        self.beat_scroll.as_f32() * self.beat_height(ui)
    }

    /// Set scroll in pixels instead of in beats.
    fn set_scroll(&mut self, scroll: f32, ui: &Ui) {
        let beats = scroll / self.beat_height(ui);
        self.beat_scroll = Timespan::approximate(beats as f64);
    }

    /// Scroll to a position that centers the given tick.
    fn scroll_to(&mut self, tick: Timespan) {
        let offset = (self.screen_tick_max - self.beat_scroll - self.row_timespan())
            * Timespan::new(1, 2);
        self.beat_scroll = (tick - offset).max(Timespan::ZERO);
    }

    /// Inserts rows into the pattern, shifting events.
    fn push_rows(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();
        let ticks = end.tick - start.tick + self.row_timespan();
        module.shift_channel_events(start, end, ticks);
    }

    /// Deletes rows from the pattern, shifting events.
    fn pull_rows(&self, module: &mut Module) {
        let (start, end) = self.selection_corners();
        let ticks = start.tick - end.tick - self.row_timespan();
        module.shift_channel_events(start, end, ticks);
    }

    /// Handle the "note off" key command.
    fn input_note_off(&mut self, module: &mut Module, all_channels: bool) {
        let (start, end) = self.selection_corners();

        if start == end && start.column == NOTE_COLUMN {
            self.input_and_step(module, EventData::NoteOff, all_channels);
        } else {
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
            self.translate_cursor(self.step_timespan());
        }
    }

    /// Handle event input in record mode.
    fn record_event(&mut self, data: EventData, module: &mut Module) {
        let cursor = self.edit_start;
        if !data.goes_in_track(cursor.track) {
            return
        }

        // skip to next open row
        let mut pos = Position {
            track: cursor.track,
            tick: cursor.tick,
            channel: cursor.channel,
            column: data.logical_column(),
        };
        if module.event_at(&pos).is_some_and(|e| e.data != EventData::NoteOff) {
            pos.tick += self.row_timespan();
        }

        module.insert_event(cursor.track, cursor.channel, Event {
            tick: pos.tick,
            data,
        });
    }

    /// Move the cursor by `offset`.
    fn translate_cursor(&mut self, offset: Timespan) {
        self.edit_end.tick = self.round_tick(self.edit_end.tick + offset)
            .max(Timespan::ZERO);

        if !is_shift_down() {
            self.edit_start.tick = self.edit_end.tick;
        }

        self.scroll_to_cursor();
    }

    /// If cursor is off-screen, scroll to center the cursor.
    fn scroll_to_cursor(&mut self) {
        let tick = self.edit_end.tick;
        if !self.tick_visible(tick) {
            self.scroll_to(tick);
        }
    }

    /// Returns true if the current viewport contains `tick`.
    fn tick_visible(&self, tick: Timespan) -> bool {
        tick >= self.beat_scroll && tick <= self.screen_tick_max
    }

    /// Draw a single pattern event.
    fn draw_event(&self, ui: &mut Ui, evt: &Event, beat_height: f32, muted: bool) {
        let y = ui.cursor_y + evt.tick.as_f32() * beat_height;
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
                a: 0.5 + x as f32 / (EventData::DIGIT_MAX as f32 * 2.0),
                ..ui.style.theme.accent1_fg()
            },
            EventData::Modulation(x) => Color {
                a: 0.5 + x as f32 / (EventData::DIGIT_MAX as f32 * 2.0),
                ..ui.style.theme.accent2_fg()
            },
            _ => ui.style.theme.fg(),
        };
        if muted || self.off_division(evt.tick) {
            color = Color { a: 0.25, ..color };
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
            EventData::Section => String::from("Sect"),
            EventData::Tempo(t) => t.round().to_string(),
            EventData::RationalTempo(n, d) => format!("{}:{}", n, d),
            EventData::InterpolatedPitch(_)
                | EventData::InterpolatedPressure(_)
                | EventData::InterpolatedModulation(_)
                => panic!("interpolated event in pattern"),
            EventData::StartGlide(_)
                | EventData::EndGlide(_)
                | EventData::TickGlide(_) => return,
            EventData::Bend(c) => format!("{:+}", c),
        };
        ui.push_text(x, y, text, color);
    }

    /// Handle the "use last note" key command.
    fn use_last_note(&mut self, module: &mut Module) {
        let cursor = self.edit_start;

        if cursor.track == 0 || cursor.column != NOTE_COLUMN {
            return
        }

        let note = module.tracks[cursor.track].channels[cursor.channel]
            .prev_note(cursor.tick);

        if let Some(note) = note {
            module.insert_event(cursor.track, cursor.channel, Event {
                tick: cursor.tick,
                data: note.data.clone(),
            });
            self.translate_cursor(self.step_timespan());
        }
    }

    /// Handle entered control column text.
    fn enter_ctrl_text(&mut self, s: String, module: &mut Module, ui: &mut Ui) {
        if let Some(pos) = self.text_position.take() {
            if !s.is_empty() {
                match parse_ctrl_text(&s) {
                    Some(data) => {
                        let event = Event { tick: pos.tick, data };
                        module.insert_event(pos.track, pos.channel, event);
                    },
                    None => ui.report("Could not parse event text"),
                }
            }
        }
    }
}

/// Parse control column text into an event.
fn parse_ctrl_text(s: &str) -> Option<EventData> {
    if let Ok(f) = s.parse::<f32>() {
        if f > 0.0 {
            return Some(EventData::Tempo(f))
        }
    } else if let Some((n, d)) = s.split_once(['/', ':']) {
        let n = n.parse::<u8>().ok()?;
        let d = d.parse::<u8>().ok()?;
        if n > 0 && d > 0 {
            return Some(EventData::RationalTempo(n, d))
        }
    }

    None
}

pub fn draw(ui: &mut Ui, module: &mut Module, player: &mut PlayerShell,
    pe: &mut PatternEditor, conf: &Config
) {
    // update tap tempo timekeeping
    if let Some(interval) = pe.pending_interval.as_mut() {
        *interval += get_frame_time();
    }

    pe.record &= player.is_playing();

    // raw key input
    if !ui.accepting_keyboard_input() {
        for key in get_keys_pressed() {
            pe.handle_key(key, module, ui);
        }
    }

    // note input
    let cursor = pe.edit_start;
    if pe.record {
        while let Some((_, data)) = ui.note_queue.pop() {
            pe.record_event(data, module);
        }
    } else if !ui.accepting_note_input() && cursor.column == NOTE_COLUMN {
        while let Some((_, data)) = ui.note_queue.pop() {
            match data {
                EventData::NoteOff => (),
                _ => pe.input_and_step(module, data, false),
            }
        }
    }

    // draw track headers
    ui.start_group();
    ui.cursor_x -= pe.h_scroll;
    let left_x = ui.cursor_x;
    let track_xs = draw_track_headers(ui, module, player, pe);
    let rect = Rect {
        w: ui.bounds.w - left_x.min(0.0),
        ..ui.end_group().unwrap()
    };
    ui.cursor_z -= 1;
    ui.push_rect(rect, ui.style.theme.panel_bg(), None);

    // set up pattern viewport
    let beat_height = pe.beat_height(ui);
    let end_y = ui.bounds.h - ui.cursor_y
        + (module.last_event_tick().unwrap_or_default()
            .max(pe.edit_start.tick).max(pe.edit_end.tick).as_f32())
        * beat_height;
    ui.push_line(ui.bounds.x, ui.cursor_y - LINE_THICKNESS * 0.5,
        ui.bounds.x + ui.bounds.w, ui.cursor_y - LINE_THICKNESS * 0.5,
        ui.style.theme.border_unfocused());
    let playhead_tick = if conf.smooth_playhead {
        player.get_tick()
    } else {
        pe.round_tick(player.get_tick())
    };
    if (pe.follow || pe.record) && player.is_playing() {
        pe.scroll_to(playhead_tick);
    }
    if pe.record {
        let tick = pe.round_tick(player.get_tick());
        pe.edit_start.tick = tick;
        pe.edit_end.tick = tick;
    }
    let mut scroll = pe.scroll(ui);
    if !(pe.follow || pe.record) || !player.is_playing() {
        let viewport_h = ui.bounds.h + ui.bounds.y - ui.cursor_y;
        ui.vertical_scrollbar(&mut scroll, end_y, viewport_h, false);
        pe.set_scroll(scroll, ui);
    }
    {
        let max_x = track_xs.last().unwrap() - left_x
            + ui.style.margin * 4.0 + ui.style.atlas.char_width();
        ui.horizontal_scrollbar(&mut pe.h_scroll, max_x, ui.bounds.w);
    }
    ui.cursor_x = track_xs[0];
    let viewport = Rect {
        x: ui.bounds.x,
        y: ui.cursor_y,
        w: ui.bounds.w,
        h: ui.bounds.h + ui.bounds.y - ui.cursor_y,
    };
    ui.cursor_z -= 1;
    ui.cursor_y -= scroll;

    pe.set_metrics(viewport, ui);

    // handle mouse input
    if ui.mouse_hits(viewport, "pattern") {
        let pos = pe.position_from_mouse(ui, &track_xs, &module.tracks);
        if is_mouse_button_pressed(MouseButton::Left) {
            pe.edit_end = pos;
            if !is_shift_down() {
                pe.edit_start = pe.edit_end;
            }
            pe.clear_tap_tempo_state();
        } else if is_mouse_button_down(MouseButton::Left) && !ui.grabbed() {
            pe.edit_end = pos;
        }

        if (track_xs[0]..*track_xs.last().unwrap()).contains(&mouse_position().0) {
            ui.info = match (pos.track, pos.column) {
                (0, GLOBAL_COLUMN) => Info::ControlColumn,
                (_, NOTE_COLUMN) => Info::NoteColumn,
                (_, VEL_COLUMN) => Info::PressureColumn,
                (_, MOD_COLUMN) => Info::ModulationColumn,
                _ => panic!("invalid column"),
            };
        }
    }

    // draw background visuals
    ui.cursor_z -= 1;
    ui.push_rect(viewport, ui.style.theme.content_bg(), None);
    draw_beats(ui, left_x, beat_height);
    ui.cursor_z += 1;
    if player.is_playing() {
        draw_playhead(ui, playhead_tick, left_x + pe.h_scroll, beat_height);
    }
    pe.draw_cursor(ui, &track_xs);

    // draw channel data
    for (track_i, track) in module.tracks.iter().enumerate() {
        let chan_width = channel_width(track_i, &ui.style);
        for (channel_i, channel) in track.channels.iter().enumerate() {
            ui.cursor_x = track_xs[track_i] + chan_width * channel_i as f32;
            pe.draw_channel(ui, channel, player.track_muted(track_i), channel_i);
        }
    }

    // handle text entry
    if let Some(pos) = pe.text_position {
        let max_width = 4;
        let coords = position_coords(pos, &ui.style, &track_xs, false, beat_height);
        let rect = Rect {
            x: coords.x + ui.style.margin,
            y: coords.y + ui.cursor_y,
            w: ui.style.atlas.char_width() * max_width as f32,
            h: line_height(&ui.style.atlas),
        };
        let action = TEXT_EXIT_ACTIONS.iter().find(|a| conf.action_is_down(**a));
        if let Some(s) = ui.pattern_edit_box(
            CTRL_COLUMN_TEXT_ID, rect, max_width, PATTERN_MARGIN, action.is_some()
        ) {
            pe.enter_ctrl_text(s, module, ui);
        }
        if let Some(action) = action {
            pe.action(*action, module, conf, player, ui);
        }
    }

    ui.cursor_x += channel_width(1, &ui.style);
    pe.draw_channel_line(ui, true);
}

/// Draws beat numbers and lines.
fn draw_beats(ui: &mut Ui, x: f32, beat_height: f32) {
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

/// Returns x positions of each track, plus the position of the last track's
/// right edge.
fn draw_track_headers(ui: &mut Ui, module: &mut Module, player: &mut PlayerShell,
    pe: &mut PatternEditor
) -> Vec<f32> {
    let mut edit = None;
    ui.layout = Layout::Horizontal;

    // offset for beat width
    ui.cursor_x += ui.style.atlas.char_width() * 4.0 + ui.style.margin * 2.0;

    let mut xs = vec![ui.cursor_x];
    xs.extend(module.tracks.iter_mut().enumerate().map(|(i, track)| {
        ui.start_group();

        // track name & delete button
        let name = track_name(track.target, &module.patches);
        match track.target {
            TrackTarget::Patch(_) | TrackTarget::None => {
                ui.start_group();
                if let Some(j) = ui.combo_box(&format!("track_{}", i), "", name,
                    Info::TrackPatch, || track_targets(&module.patches)) {
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
            edit = Some(Edit::AddChannel(i, Channel::default()));
        }
        ui.end_group();

        // column labels
        ui.start_group();
        for _ in 0..track.channels.len() {
            let color = ui.style.theme.border_unfocused();
            if i == 0 {
                ui.colored_label("Ctrl", Info::ControlColumn, color)
            } else {
                ui.colored_label("Note", Info::NoteColumn, color);
                ui.cursor_x -= ui.style.margin;
                ui.colored_label("P", Info::PressureColumn, color);
                ui.cursor_x -= ui.style.margin;
                ui.colored_label("M", Info::ModulationColumn, color);
            }
        }
        ui.end_group();

        ui.end_group();
        ui.cursor_x
    }));

    if let Some(edit) = edit {
        module.push_edit(edit);
        player.update_synths(module.drain_track_history());
        fix_cursors(&mut pe.edit_start, &mut pe.edit_end, &module.tracks);
    }

    if ui.button("+", !module.patches.is_empty(), Info::Add("a new track")) {
        module.add_track();
        player.update_synths(module.drain_track_history());
    }

    xs
}

/// Adjust selected notes for transposition commands.
fn nudge_notes(module: &mut Module, (start, end): (Position, Position), cfg: &Config) {
    let replacements = module.scan_events(start, end).into_iter().filter_map(|mut evt| {
        if let EventData::Pitch(note) = &mut evt.event.data {
            *note = input::adjust_note_for_modifier_keys(*note, cfg, &module.tuning);
            Some(evt)
        } else {
            None
        }
    }).collect();
    module.push_edit(Edit::ReplaceEvents(replacements));
}

/// Returns true if an event was inserted.
fn insert_event_at_cursor(module: &mut Module, cursor: &Position, data: EventData,
    all_channels: bool
) -> bool {
    // only write control data in control columns
    if !data.goes_in_track(cursor.track) {
        return false
    }

    // midi pitch bend can only overwrite midi pitch bend
    if matches!(data, EventData::Bend(_))
        && !matches!(module.event_at(cursor).map(|e| &e.data),
            Some(EventData::Bend(_)) | None) {
        return false
    }

    let n = module.tracks[cursor.track].channels.len();
    if all_channels && n > 1 {
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

    return true
}

/// Returns the UI display string for a track.
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

/// Returns UI display strings for each patch.
fn track_targets(patches: &[Patch]) -> Vec<String> {
    let mut v = vec![track_name(TrackTarget::None, patches).to_owned()];
    v.extend(patches.iter().map(|x| x.name.to_owned()));
    v
}

fn draw_playhead(ui: &mut Ui, tick: Timespan, x: f32, beat_height: f32) {
    let rect = Rect {
        x,
        y: ui.cursor_y + tick.as_f32() * beat_height,
        w: ui.bounds.w,
        h: line_height(&ui.style.atlas),
    };
    let color = Color { a: 0.1, ..ui.style.theme.fg() };
    ui.push_rect(rect, color, None);
}

/// Handle the "previous column" key command.
fn shift_column_left(start: &mut Position, end: &mut Position, tracks: &[Track]) {
    let column = end.column as i8 - 1;
    if column >= 0 {
        end.column = column as u8;
    } else {
        if end.channel > 0 {
            end.channel -= 1;
        } else if end.track > 0 {
            end.track -= 1;
            end.channel = tracks[end.track].channels.len() - 1;
        }

        if end.track == 0 {
            end.column = GLOBAL_COLUMN;
        } else {
            end.column = MOD_COLUMN;
        }
    }
    if !is_shift_down() {
        start.track = end.track;
        start.channel = end.channel;
        start.column = end.column;
    }
}

/// Handle the "next column" key command.
fn shift_column_right(start: &mut Position, end: &mut Position, tracks: &[Track]) {
    *end = next_column(*end, tracks);

    if !is_shift_down() {
        start.track = end.track;
        start.channel = end.channel;
        start.column = end.column;
    }
}

fn next_column(pos: Position, tracks: &[Track]) -> Position {
    let column = pos.column + 1;
    let n_columns = if pos.track == 0 { 1 } else { 3 };
    let mut pos = pos;

    if column < n_columns {
        pos.column = column;
    } else if pos.channel + 1 < tracks[pos.track].channels.len() {
        pos.channel += 1;
        pos.column = 0;
    } else if pos.track + 1 < tracks.len() {
        pos.track += 1;
        pos.channel = 0;
        pos.column = 0;
    }

    pos
}

/// Handle the "previous channel" key command.
fn shift_channel_left(start: &mut Position, end: &mut Position, tracks: &[Track]) {
    let channel = end.channel as isize - 1;
    if channel >= 0 {
        end.channel = channel as usize;
    } else if end.track > 0 {
        end.track -= 1;
        end.channel = tracks[end.track].channels.len() - 1;
        if end.track == 0 {
            end.column = 0;
        }
    }
    start.track = end.track;
    start.channel = end.channel;
}

/// Handle the "next channel" key command.
fn shift_channel_right(start: &mut Position, end: &mut Position, tracks: &[Track]) {
    *end = next_channel(*end, tracks);
    start.track = end.track;
    start.channel = end.channel;
}

/// Shift a position one channel to the right.
fn next_channel(pos: Position, tracks: &[Track]) -> Position {
    pos.add_channels(1, tracks).unwrap_or(pos)
}

/// Reposition the pattern cursors if in an invalid position.
fn fix_cursors(start: &mut Position, end: &mut Position, tracks: &[Track]) {
    for cursor in [start, end] {
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

/// Returns the minimum visual width of a channel.
fn channel_width(track_index: usize, style: &Style) -> f32 {
    if track_index == 0 {
        column_x(1, style) + style.margin
    } else {
        column_x(3, style) + style.margin
    }
}

/// Returns the x offset for a pattern column.
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

/// Return the line height used in the pattern grid.
fn line_height(atlas: &GlyphAtlas) -> f32 {
    atlas.cap_height() + PATTERN_MARGIN * 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ctrl_text() {
        assert_eq!(parse_ctrl_text(""), None);
        assert_eq!(parse_ctrl_text("-100"), None);
        assert_eq!(parse_ctrl_text("1237:1273"), None);
        assert_eq!(parse_ctrl_text("1:0"), None);
        assert_eq!(parse_ctrl_text("100"), Some(EventData::Tempo(100.0)));
        assert_eq!(parse_ctrl_text("60.5"), Some(EventData::Tempo(60.5)));
        assert_eq!(parse_ctrl_text("1/2"), Some(EventData::RationalTempo(1, 2)));
        assert_eq!(parse_ctrl_text("4:3"), Some(EventData::RationalTempo(4, 3)));
    }
}
