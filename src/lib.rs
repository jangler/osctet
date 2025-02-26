use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::Arc;

use config::Config;
use cpal::SampleRate;
use fx::{FXSettings, GlobalFX};
use midir::{InitError, MidiInput, MidiInputConnection, MidiInputPort};
use fundsp::hacker32::*;
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig};
use module::{Edit, EventData, Module, ModuleCommand, ModuleSync, TrackTarget};
use playback::{Player, PlayerShell, RenderUpdate};
use rfd::FileDialog;
use rtrb::RingBuffer;
use synth::{Key, KeyOrigin};
use macroquad::prelude::*;

mod pitch;
mod input;
mod config;
mod synth;
mod fx;
mod ui;
pub mod module;
pub mod playback;
mod dsp;
mod timespan;

use input::{Action, Hotkey, MidiEvent, Modifiers};
use timespan::Timespan;
use triple_buffer::triple_buffer;
use ui::developer::DevState;
use ui::general::GeneralState;
use ui::info::Info;
use ui::instruments::{fix_patch_index, InstrumentsState};
use ui::settings::SettingsState;
use ui::{is_alt_down, is_ctrl_down};
use ui::pattern::PatternEditor;

/// Application name, for window title, etc.
pub const APP_NAME: &str = "Osctet";
const MODULE_FILETYPE_NAME: &str = "Osctet module";
const MODULE_EXT: &str = "osctet";
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a path in the same directory as the executable. If no executable
/// path is available, returns the plain filename as a path.
pub fn exe_relative_path(filename: &str) -> PathBuf {
    match env::current_exe() {
        Ok(mut path) => {
            path.pop();
            path.push(filename);
            path
        }
        Err(e) => {
            eprintln!("Error finding executable path: {e}");
            filename.into()
        }
    }
}

type MidiConn = MidiInputConnection<Sender<Vec<u8>>>;

/// Handles MIDI connection and state.
pub struct Midi {
    // Keep one input around for listing ports. If we need to connect, we'll
    // create a new input just for that (see Boddlnagg/midir#90).
    input: Option<MidiInput>,
    port_name: Option<String>,
    port_selection: Option<String>,
    conn: Option<MidiConn>,
    rx: Option<Receiver<Vec<u8>>>,
    input_id: u16,
    rpn: (u8, u8),
    bend_range: f32,
}

impl Midi {
    fn new() -> Self {
        let mut m = Self {
            input: None,
            port_name: None,
            port_selection: None,
            conn: None,
            rx: None,
            input_id: 0,
            rpn: (0, 0),
            bend_range: 2.0,
        };
        m.input = m.new_input().ok();
        m
    }

    /// Create a new MIDI input for the application.
    fn new_input(&mut self) -> Result<MidiInput, InitError> {
        self.input_id += 1;
        MidiInput::new(&format!("{} input #{}", APP_NAME, self.input_id))
    }

    /// Returns the currently selected input port.
    fn selected_port(&self) -> Result<MidiInputPort, &'static str> {
        let selection = self.port_selection.as_ref().ok_or("No MIDI device selected")?;
        let input = self.input.as_ref().ok_or("Could not open MIDI")?;
        input.ports().into_iter()
            .find(|p| input.port_name(p).is_ok_and(|s| s == *selection))
            .ok_or("Selected MIDI device not found")
    }
}

const MAIN_TAB_ID: &str = "main";
const TAB_GENERAL: usize = 0;
const TAB_PATTERN: usize = 1;
const TAB_INSTRUMENTS: usize = 2;
const TAB_SETTINGS: usize = 3;
const TAB_DEVELOPER: usize = 4;

#[cfg(not(debug_assertions))]
const TABS: [&str; 4] = ["General", "Pattern", "Instruments", "Settings"];

#[cfg(debug_assertions)]
const TABS: [&str; 5] = ["General", "Pattern", "Instruments", "Settings", "Developer"];

/// Top-level store of application state.
struct App {
    octave: i8,
    midi: Midi,
    config: Config,
    fx: GlobalFX,
    ui: ui::Ui,
    general_state: GeneralState,
    pattern_editor: PatternEditor,
    instruments_state: InstrumentsState,
    settings_state: SettingsState,
    dev_state: DevState,
    save_path: Option<PathBuf>,
    render_channel: Option<Receiver<RenderUpdate>>,
    version: String,
    player: PlayerShell,
    stereo_width: Shared,
    module: Module,
    module_sync: ModuleSync,
    keyjazz_modulation: f32,
}

impl App {
    fn new(global_fx: GlobalFX, config: Config, sample_rate: u32,
        audio_conf: Option<StreamConfig>, player: PlayerShell, stereo_width: Shared,
        module: Module, module_sync: ModuleSync
    ) -> Self {
        let mut midi = Midi::new();
        midi.port_selection = config.default_midi_input.clone();
        let mut module = module;
        module.sync = true;
        App {
            octave: 3,
            midi,
            ui: ui::Ui::new(config.theme.clone(), config.font_size),
            config,
            fx: global_fx,
            pattern_editor: PatternEditor::default(),
            general_state: Default::default(),
            instruments_state: InstrumentsState::new(Some(0)),
            settings_state: SettingsState::new(sample_rate),
            dev_state: DevState::new(audio_conf),
            save_path: None,
            render_channel: None,
            version: format!("v{PKG_VERSION}"),
            player,
            stereo_width,
            module,
            module_sync,
            keyjazz_modulation: 0.0,
        }
    }

    // TODO: use most current vel/mod setting when keyjazzing in pattern

    /// Returns the index of the current track to use for keyjazzing.
    fn keyjazz_track(&self) -> usize {
        // TODO: switching tracks while keyjazzing could result in stuck notes
        // TODO: entering note input mode while keyjazzing could result in stuck notes
        // TODO: switching octave while keyjazzing can result in stuck notes?
        if self.ui.get_tab(MAIN_TAB_ID) == Some(TAB_PATTERN) {
            self.pattern_editor.cursor_track()
        } else {
            0
        }
    }

    /// Returns the current patch index to use for keyjazzing.
    fn keyjazz_patch_index(&self) -> Option<usize> {
        match self.module.tracks[self.keyjazz_track()].target {
            TrackTarget::Global | TrackTarget::None => self.instruments_state.patch_index,
            TrackTarget::Kit => None,
            TrackTarget::Patch(i) => Some(i),
        }
    }

    /// Handle keyboard input.
    fn handle_keys(&mut self) {
        let (pressed, released) = (get_keys_pressed(), get_keys_released());
        let mods = Modifiers::current();

        // translate released keys into note-offs
        for key in released {
            let hk = Hotkey::new(mods, key);
            let note = input::note_from_key(
                hk, &self.module.tuning, self.octave, &self.config);
            if note.is_some() {
                let key = Key::new_from_keyboard(input::u8_from_key(key));
                self.ui.note_queue.push((key.clone(), EventData::NoteOff));
                self.player.note_off(self.keyjazz_track(), key);
            }
        }

        // translate pressed keys into key commands
        for key in pressed {
            let hk = Hotkey::new(mods, key);
            if let Some(action) = self.config.hotkey_action(&hk) {
                match action {
                    Action::IncrementDivision => self.pattern_editor.inc_division(),
                    Action::DecrementDivision => self.pattern_editor.dec_division(),
                    Action::DoubleDivision => self.pattern_editor.double_division(),
                    Action::HalveDivision => self.pattern_editor.halve_division(),
                    Action::FocusDivision => self.ui.focus("Division"),
                    Action::IncrementOctave =>
                        self.octave = self.octave.saturating_add(1),
                    Action::DecrementOctave =>
                        self.octave = self.octave.saturating_sub(1),
                    Action::PlayFromStart => self.player.toggle_play_from(Timespan::ZERO),
                    Action::PlayFromScreen => {
                        let tick = self.pattern_editor.screen_beat_tick();
                        self.player.toggle_play_from(tick)
                    }
                    Action::PlayFromCursor =>
                        self.player.toggle_play_from(self.pattern_editor.cursor_tick()),
                    Action::StopPlayback => self.player.stop(),
                    Action::NewSong => if self.module.has_unsaved_changes {
                        self.ui.confirm("Discard unsaved changes?", Action::NewSong);
                    } else {
                        self.new_module()
                    },
                    Action::OpenSong=> if self.module.has_unsaved_changes {
                        self.ui.confirm("Discard unsaved changes?", Action::OpenSong);
                    } else {
                        self.open_module()
                    },
                    Action::SaveSong => self.save_module(),
                    Action::SaveSongAs => self.save_module_as(),
                    Action::RenderSong => self.render_and_save(false),
                    Action::RenderTracks => self.render_and_save(true),
                    Action::Undo => if self.module.undo() {
                        self.player.update_synths(self.module.drain_track_history());
                        fix_patch_index(&mut self.instruments_state.patch_index,
                            self.module.patches.len());
                    } else {
                        self.ui.report("Nothing to undo");
                    },
                    Action::Redo => if self.module.redo() {
                        self.player.update_synths(self.module.drain_track_history());
                        fix_patch_index(&mut self.instruments_state.patch_index,
                            self.module.patches.len());
                    } else {
                        self.ui.report("Nothing to redo");
                    },
                    Action::NextTab => self.ui.next_tab(MAIN_TAB_ID, TABS.len()),
                    Action::PrevTab => self.ui.prev_tab(MAIN_TAB_ID, TABS.len()),
                    Action::Panic => self.player.panic(),
                    _ => if self.ui.get_tab(MAIN_TAB_ID) == Some(TAB_PATTERN) {
                        self.pattern_editor.action(*action, &mut self.module, &self.config,
                            &mut self.player);
                    },
                }
            } else if let Some(action) = self.config.hotkey_action(&hk.without_shift()) {
                // these actions have some special behavior when used with shift
                match action {
                    Action::NextRow | Action::PrevRow
                        | Action::NextColumn | Action::PrevColumn
                        | Action::NextBeat | Action::PrevBeat
                        | Action::NextEvent | Action::PrevEvent
                        | Action::PatternStart | Action::PatternEnd
                        | Action::Delete | Action::NoteOff =>
                            self.pattern_editor.action(
                                *action, &mut self.module, &self.config, &mut self.player),
                    _ => (),
                }
            }

            // translate pressed keys into note-ons
            let note = input::note_from_key(
                hk, &self.module.tuning, self.octave, &self.config);
            if let Some(note) = note {
                let key = Key::new_from_keyboard(input::u8_from_key(key));
                self.ui.note_queue.push((key.clone(), EventData::Pitch(note)));
                if !(self.ui.accepting_note_input()
                    || self.pattern_editor.in_digit_column(&self.ui)
                    || self.pattern_editor.in_global_track(&self.ui)
                ) {
                    if let Some((patch, note)) =
                        self.module.map_input(self.keyjazz_patch_index(), note) {
                        let pitch = self.module.tuning.midi_pitch(&note);
                        self.player.note_on(self.keyjazz_track(), key, pitch, None, patch);
                    }
                }
            }
        }
    }

    /// Attempt to connect to the selected MIDI port.
    fn midi_connect(&mut self) -> Result<MidiConn, Box<dyn Error>> {
        let port = self.midi.selected_port()?;
        let mut input = self.midi.new_input()?;

        // ignore SysEx, time, and active sensing
        input.ignore(midir::Ignore::All);

        let (tx, rx) = channel();
        self.midi.rx = Some(rx);
        Ok(input.connect(
            &port,
            APP_NAME,
            move |_, message, tx| {
                // ignore the error here, it probably just means that the
                // user changed ports
                let _ = tx.send(message.to_vec());
            },
            tx,
        )?)
    }

    /// Handle incoming MIDI messages.
    fn handle_midi(&mut self) {
        for evt in self.get_midi_events() {
            self.handle_midi_event(evt);
        }
    }

    /// Collect incoming MIDI events.
    fn get_midi_events(&mut self) -> Vec<MidiEvent> {
        let mut v = Vec::new();

        if let Some(rx) = &self.midi.rx {
            while let Ok(chunk) = rx.try_recv() {
                if let Some(evt) = MidiEvent::parse(&chunk) {
                    v.push(evt);
                }
            }
        }

        v
    }

    /// Handle an incoming MIDI message.
    fn handle_midi_event(&mut self, evt: MidiEvent) {
        match evt {
            MidiEvent::NoteOff { channel, key, .. } => {
                let key = Key::new_from_midi(channel, key);
                self.player.note_off(self.keyjazz_track(), key.clone());
                self.ui.note_queue.push((key, EventData::NoteOff));
            },
            MidiEvent::NoteOn { channel, key, velocity } => {
                let key = Key::new_from_midi(channel, key);
                if velocity != 0 {
                    let note = input::note_from_midi(
                        key.key, &self.module.tuning, &self.config);
                    self.ui.note_queue.push((key.clone(), EventData::Pitch(note)));
                    if self.config.midi_send_velocity {
                        let v = EventData::digit_from_midi(velocity);
                        self.ui.note_queue.push((key.clone(), EventData::Pressure(v)));
                    }

                    let index = self.keyjazz_patch_index();
                    if let Some((patch, mapped_note)) = self.module.map_input(index, note) {
                        if !self.ui.accepting_note_input() {
                            let pitch = self.module.tuning.midi_pitch(&mapped_note);
                            let pressure = if self.config.midi_send_velocity {
                                Some(velocity as f32 / 127.0)
                            } else {
                                None
                            };
                            self.player.note_on(self.keyjazz_track(),
                                key.clone(), pitch, pressure, patch);
                        }
                    }
                } else {
                    self.player.note_off(self.keyjazz_track(), key.clone());
                    self.ui.note_queue.push((key, EventData::NoteOff));
                }
            },
            MidiEvent::PolyPressure { channel, key, pressure } => {
                if self.config.midi_send_pressure == Some(true) {
                    let key = Key::new_from_midi(channel, key);
                    self.player.poly_pressure(self.keyjazz_track(), key.clone(),
                        pressure as f32 / 127.0);
                    let v = EventData::digit_from_midi(pressure);
                    self.ui.note_queue.push((key, EventData::Pressure(v)));
                }
            },
            MidiEvent::Controller { channel, controller, value } => {
                let norm_value = value as f32 / 127.0;
                match controller {
                    input::CC_MODULATION | input::CC_MACRO_MIN..=input::CC_MACRO_MAX => {
                        self.player.modulate(self.keyjazz_track(), channel, norm_value);
                    },
                    input::CC_RPN_MSB => self.midi.rpn.0 = value,
                    input::CC_RPN_LSB => self.midi.rpn.1 = value,
                    input::CC_DATA_ENTRY_MSB =>
                        if self.midi.rpn == input::RPN_PITCH_BEND_SENSITIVITY {
                            // set semitones
                            self.midi.bend_range =
                                self.midi.bend_range % 1.0 + norm_value as f32;
                        },
                    input:: CC_DATA_ENTRY_LSB =>
                        if self.midi.rpn == input::RPN_PITCH_BEND_SENSITIVITY {
                            // set cents
                            self.midi.bend_range =
                                self.midi.bend_range.floor() + norm_value as f32 / 100.0;
                        },
                    _ => (),
                }
            },
            MidiEvent::ChannelPressure { channel, pressure } => {
                if self.config.midi_send_pressure == Some(true) {
                    self.player.channel_pressure(self.keyjazz_track(),
                        channel, pressure as f32 / 127.0);
                    let key = Key::new_from_midi(channel, 0);
                    let v = EventData::digit_from_midi(pressure);
                    self.ui.note_queue.push((key, EventData::Pressure(v)));
                }
            },
            MidiEvent::Pitch { channel, bend } => {
                let semitones = bend * self.midi.bend_range;
                self.player.pitch_bend(self.keyjazz_track(), channel, semitones);
                let key = Key::new_from_midi(channel, 0);
                let data = EventData::Bend((semitones * 100.0).round() as i16);
                self.ui.note_queue.push((key, data));
            },
        }
    }

    /// Reconnect if MIDI connection settings have changed.
    fn check_midi_reconnect(&mut self) {
        if self.midi.port_selection.is_some()
            && self.midi.port_selection != self.midi.port_name {
            match self.midi_connect() {
                Ok(conn) => {
                    if let Some(c) = self.midi.conn.replace(conn) {
                        c.close();
                    }
                    self.midi.port_name = self.midi.port_selection.clone();
                    self.config.default_midi_input = self.midi.port_name.clone();
                },
                Err(e) => {
                    self.midi.port_selection = None;
                    self.config.default_midi_input = None;
                    self.ui.report(format!("MIDI connection failed: {e}"));
                },
            }
        } else if self.midi.port_selection.is_none() && self.midi.port_name.is_some() {
            if let Some(c) = self.midi.conn.take() {
                c.close();
            }
            self.midi.port_name = None;
            self.config.default_midi_input = None;
        }
    }

    /// Do 1 frame. Returns false if it's quitting time.
    fn frame(&mut self) -> bool {
        if self.dev_state.only_draw_on_input && !mouse_kb_input() {
            return true
        }

        self.player.update();

        if is_quit_requested() {
            if self.module.has_unsaved_changes {
                self.ui.confirm("Discard unsaved changes?", Action::Quit);
            } else {
                self.save_config();
                return false
            }
        }

        if self.ui.accepting_keyboard_input() {
            self.player.clear_notes_with_origin(KeyOrigin::Keyboard);
        } else {
            self.handle_keys();
        }

        if self.ui.accepting_note_input() {
            self.player.clear_notes_with_origin(KeyOrigin::Midi);
        }

        // ctrl+scroll. this is here instead of in pattern code because
        // division can always be changed
        if is_ctrl_down() && mouse_wheel().1 != 0.0 {
            let pe = &mut self.pattern_editor;
            let d = mouse_wheel().1.signum() as i8;
            pe.set_division(if !is_alt_down() {
                pe.beat_division.saturating_add_signed(d)
            } else if d > 0 {
                pe.beat_division.saturating_mul(2)
            } else {
                pe.beat_division / 2
            });
        }

        if self.player.is_playing() {
            let end_tick = self.module.last_event_tick().unwrap_or_default()
                + Timespan::new(1, 1);
            if self.player.get_tick() > end_tick {
                self.player.stop();
            }
        }

        self.handle_midi();

        self.handle_render_updates();
        self.check_midi_reconnect();
        let quit = self.process_ui();
        self.sync_edits();
        quit
    }

    fn sync_edits(&mut self) {
        for edit in self.module.sync_edits() {
            let patch_index = if let Edit::InsertPatch(i, _) = edit {
                Some(i)
            } else {
                None
            };
            self.module_sync.push(ModuleCommand::Edit(edit));
            if let Some(i) = patch_index {
                self.module_sync.push(
                    ModuleCommand::Patch(i, self.module.patches[i].shared_clone()));
            }
        }
    }

    /// Save config to disk, logging errors.
    fn save_config(&mut self) {
        if let Err(e) = self.config.save(self.ui.style.theme.clone()) {
            eprintln!("error saving config: {}", e);
        }
    }

    /// Handle incoming render status updates.
    fn handle_render_updates(&mut self) {
        if let Some(rx) = &self.render_channel {
            while let Ok(update) = rx.try_recv() {
                match update {
                    RenderUpdate::Progress(f) =>
                        self.ui.notify(format!("Rendering: {}%", (f * 100.0).round())),
                    RenderUpdate::Done(wav, path) => {
                        let write_result = if self.config.render_bit_depth == Some(32) {
                            wav.save_wav32(path)
                        } else {
                            wav.save_wav16(path)
                        };

                        match write_result {
                            Ok(_) => self.ui.notify(String::from("Wrote WAV.")),
                            Err(e) => self.ui.report(format!("Writing WAV failed: {e}")),
                        }
                    }
                }
            }
        }
    }

    /// Process the UI for 1 frame. Returns false if it's quitting time.
    fn process_ui(&mut self) -> bool {
        // process actions confirmed via dialog
        if let Some(action) = self.ui.start_frame(&self.config) {
            match action {
                Action::NewSong => self.new_module(),
                Action::OpenSong => self.open_module(),
                Action::Quit => {
                    self.save_config();
                    return false
                }
                _ => panic!("unhandled dialog action: {:?}", action),
            }
        }

        self.bottom_panel();

        match self.ui.tab_menu(MAIN_TAB_ID, &TABS, &self.version) {
            TAB_GENERAL => {
                let (fx_changed, tuning_changed) = ui::general::draw(
                    &mut self.ui, &mut self.module, &mut self.fx, &mut self.config,
                    &mut self.player, &mut self.general_state);
                if fx_changed {
                    self.module_sync.push(
                        ModuleCommand::FX(self.module.fx.clone()));
                }
                if tuning_changed {
                    self.module_sync.push(
                        ModuleCommand::Tuning(self.module.tuning.clone()));
                }
            }
            TAB_PATTERN => ui::pattern::draw(&mut self.ui, &mut self.module,
                &mut self.player, &mut self.pattern_editor, &self.config),
            TAB_INSTRUMENTS => ui::instruments::draw(&mut self.ui, &mut self.module,
                &mut self.instruments_state, &mut self.config, &mut self.player,
                &mut self.module_sync),
            TAB_SETTINGS => ui::settings::draw(&mut self.ui, &mut self.config,
                &mut self.settings_state, &mut self.player, &mut self.midi),
            TAB_DEVELOPER => ui::developer::draw(&mut self.ui, &mut self.dev_state,
                &self.player),
            _ => panic!("bad tab value"),
        }

        let tab_nav = self.ui.get_tab(MAIN_TAB_ID).is_none_or(|i| i != TAB_PATTERN);
        self.ui.end_frame(tab_nav);
        true
    }

    /// Draw the status panel at the bottom of the screen.
    fn bottom_panel(&mut self) {
        self.ui.start_bottom_panel();

        if let Some(n) = self.ui.edit_box("Octave", 2, self.octave.to_string(),
            Info::Octave
        ) {
            match n.parse::<i8>() {
                Ok(n) => self.octave = n,
                Err(e) => self.ui.report(e),
            }
        }

        self.ui.shared_slider("stereo_width", "Stereo width",
            &self.stereo_width, -1.0..=1.0, None, 1, true, Info::StereoWidth);

        match self.ui.get_tab(MAIN_TAB_ID) {
            Some(TAB_PATTERN) => {
                if let Some(n) = self.ui.edit_box("Division", 3,
                    self.pattern_editor.beat_division.to_string(), Info::Division
                ) {
                    match n.parse::<u8>() {
                        Ok(n) => self.pattern_editor.set_division(n),
                        Err(e) => self.ui.report(e),
                    }
                }

                self.ui.checkbox("Follow", &mut self.pattern_editor.follow, true,
                    Info::FollowCheckbox);
            }
            _ => {
                const MAX: f32 = EventData::DIGIT_MAX as f32;
                if self.ui.formatted_slider("modulation", "Modulation",
                    &mut self.keyjazz_modulation, 0.0..=MAX, 1, true,
                    Info::KeyjazzModulation, |x| format!("{:X}", x.round() as u8),
                    |x| x.round()
                ) {
                    self.player.modulate(self.keyjazz_track(), 0,
                        self.keyjazz_modulation / MAX);
                }
            }
        }

        self.ui.end_bottom_panel();
    }

    /// Browse for and start rendering a WAV file.
    fn render_and_save(&mut self, tracks: bool) {
        if self.module.ends() {
            let dialog = ui::new_file_dialog(&mut self.player)
                .add_filter("WAV file", &["wav"])
                .set_directory(self.config.render_folder.clone()
                    .unwrap_or(String::from(".")))
                .set_file_name(self.module.title.clone());

            if let Some(mut path) = dialog.save_file() {
                path.set_extension("wav");
                self.config.render_folder = config::dir_as_string(&path);
                let module = Arc::new(self.module.clone());
                self.render_channel = Some(if tracks {
                    playback::render_tracks(module, path)
                } else {
                    playback::render(module, path, None)
                });
            }
        } else {
            self.ui.report("Module must have End event to export")
        }
    }

    /// Handle the "new song" key command.
    fn new_module(&mut self) {
        self.load_module(Module::new(Default::default()));
        self.save_path = None;
    }

    /// Handle the "save song" key command.
    fn save_module(&mut self) {
        if let Some(path) = &self.save_path {
            if let Err(e) = self.module.save(self.pattern_editor.beat_division, path) {
                self.ui.report(format!("Error saving module: {e}"));
            } else {
                self.ui.notify(String::from("Saved module."));
            }
        } else {
            self.save_module_as();
        }
    }

    /// Handle the "save song as" key command.
    fn save_module_as(&mut self) {
        let dialog = self.module_dialog().set_file_name(self.module.title.clone());

        if let Some(mut path) = dialog.save_file() {
            path.set_extension(MODULE_EXT);
            self.config.module_folder = config::dir_as_string(&path);
            if let Err(e) = self.module.save(self.pattern_editor.beat_division, &path) {
                self.ui.report(format!("Error saving module: {e}"));
            } else {
                self.save_path = Some(path);
                self.ui.notify(String::from("Saved module."));
            }
        }
    }

    /// Handle the "open song" key command.
    fn open_module(&mut self) {
        if let Some(path) = self.module_dialog().pick_file() {
            self.config.module_folder = config::dir_as_string(&path);
            match Module::load(&path) {
                Ok(new_module) => {
                    self.load_module(new_module);
                    self.save_path = Some(path);
                },
                Err(e) => self.ui.report(format!("Error loading module: {e}")),
            }
        }
    }

    fn module_dialog(&mut self) -> FileDialog {
        let dir = self.config.module_folder.clone().unwrap_or(String::from("."));
        ui::new_file_dialog(&mut self.player)
            .add_filter(MODULE_FILETYPE_NAME, &[MODULE_EXT])
            .set_directory(dir)
    }

    /// Replace the current module with `module`, reinitializing state as
    /// needed.
    fn load_module(&mut self, new_mod: Module) {
        self.module_sync.push(ModuleCommand::Load(new_mod.shared_clone()));
        self.module = new_mod;
        self.module.sync = true;
        let follow = self.pattern_editor.follow;
        self.pattern_editor = PatternEditor::default();
        self.pattern_editor.beat_division = self.module.division;
        self.pattern_editor.follow = follow;
        self.instruments_state.patch_index = if self.module.patches.is_empty() {
            None
        } else {
            Some(0)
        };
        self.player.reinit();
        self.fx.reinit(&self.module.fx);
    }
}

/// Returns JACK if available, otherwise ALSA.
#[cfg(target_os = "linux")]
fn get_audio_device() -> Option<cpal::Device> {
    cpal::host_from_id(cpal::HostId::Jack).ok()
        .and_then(|host| host.default_output_device())
        .or_else(|| cpal::default_host().default_output_device())
}

/// Returns the default device.
#[cfg(not(target_os = "linux"))]
fn get_audio_device() -> Option<cpal::Device> {
    cpal::default_host().default_output_device()
}

/// Returns the best available audio output stream config.
fn preferred_config(device: &cpal::Device, desired_sr: SampleRate
) -> Result<StreamConfig, Box<dyn Error>> {
    device.supported_output_configs()?
        .filter(|conf| conf.channels() == 2)
        .max_by_key(|conf| (
            conf.sample_format().sample_size() > 1,
            conf.max_sample_rate() >= desired_sr,
            conf.min_sample_rate() <= desired_sr,
            conf.sample_format() == cpal::SampleFormat::F32
        )).map(|conf| {
            let sr = desired_sr.clamp(conf.min_sample_rate(), conf.max_sample_rate());
            conf.with_sample_rate(sr).into()
        }).ok_or("no supported audio config".into())
}

/// Application entry point.
pub async fn run(arg: Option<String>) -> Result<(), Box<dyn Error>> {
    let conf = Config::load().unwrap_or_default();
    let device = get_audio_device();

    let audio_conf: Result<StreamConfig, Box<dyn Error>> = device.as_ref()
        .ok_or("no audio output device".into())
        .and_then(|device| preferred_config(device, SampleRate(conf.desired_sample_rate)));
    let sample_rate = audio_conf.as_ref()
        .map(|config| config.sample_rate.0)
        .unwrap_or(44100);
    let cloned_conf = audio_conf.as_ref().cloned().ok();

    let mut seq = Sequencer::new(false, 4);
    seq.set_sample_rate(sample_rate as f64);

    // the sequencer backend is probably not necessary anymore due to mutexing,
    // but it's still convenient for ownership reasons.
    let fx_settings: FXSettings = Default::default();
    let mut global_fx = GlobalFX::new(seq.backend(), &fx_settings);
    global_fx.net.set_sample_rate(sample_rate as f64);
    let mut backend = BlockRateAdapter::new(Box::new(global_fx.net.backend()));

    let module = Module::new(fx_settings);
    let mut player = Player::new(seq, module.tracks.len(), sample_rate as f32);
    let (player_cmd_producer, mut player_cmd_consumer) = RingBuffer::new(10);
    let (module_cmd_producer, mut module_cmd_consumer) = RingBuffer::new(10);
    let (mut player_state_input, player_state_output) = triple_buffer(&player.state());

    const UPDATE_FRAMES: u32 = 64;
    let update_interval: f64 = UPDATE_FRAMES as f64 / sample_rate as f64;
    let mut frames_until_update = UPDATE_FRAMES;

    let mut stream_module = module.shared_clone();
    let stereo_width = player.stereo_width.clone();

    // audio callback
    let stream = audio_conf.and_then(|config| {
        Ok(device.expect("device should be present if config is").build_output_stream(
            &config, move |data: &mut[f32], _: &cpal::OutputCallbackInfo| {
                let mut i = 0;
                let len = data.len();
                while i < len {
                    if frames_until_update == 0 {
                        while let Ok(cmd) = module_cmd_consumer.pop() {
                            stream_module.handle_command(cmd);
                        }
                        while let Ok(cmd) = player_cmd_consumer.pop() {
                            player.handle_command(cmd, &stream_module);
                        }
                        player.buffer_size = data.len() / 2;
                        player.frame(&stream_module, update_interval);
                        frames_until_update = UPDATE_FRAMES;
                        player_state_input.write(player.state());
                    }
                    let (l, r) = backend.get_stereo();
                    data[i] = l;
                    data[i+1] = r;
                    i += 2;
                    frames_until_update -= 1;
                }
            },
            |err| eprintln!("stream error: {err}"),
            None
        )?)
    });

    let ps = PlayerShell::new(player_state_output, player_cmd_producer);
    let mut app = App::new(global_fx, conf, sample_rate, cloned_conf, ps, stereo_width,
        module, ModuleSync::new(module_cmd_producer));

    // ugly duplication, but error typing makes a nice solution difficult
    match &stream {
        Ok(stream) => if let Err(e) = stream.play() {
            app.ui.report(format!("Could not initialize audio: {e}"));
        }
        Err(e) => app.ui.report(format!("Could not initialize audio: {e}"))
    };

    if let Some(arg) = arg {
        match Module::load(&arg.into()) {
            Ok(m) => app.load_module(m),
            Err(e) => app.ui.report(format!("Error loading module: {e}")),
        }
    }

    while app.frame() {
        next_frame().await
    }

    Ok(())
}

/// Returns true if there was mouse or keyboard input.
fn mouse_kb_input() -> bool {
    !(get_keys_down().is_empty()
        && !is_mouse_button_pressed(MouseButton::Left)
        && !is_mouse_button_released(MouseButton::Left)
        && !is_mouse_button_down(MouseButton::Left)
        && !is_mouse_button_pressed(MouseButton::Right)
        && !is_mouse_button_released(MouseButton::Right)
        && !is_mouse_button_down(MouseButton::Right)
        && mouse_wheel() == (0.0, 0.0)
        && mouse_delta_position() == Vec2::ZERO
        && !is_quit_requested())
}