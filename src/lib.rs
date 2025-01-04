//! Microtonal tracker with built-in subtractive/FM synth.

use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};

use config::Config;
use cpal::SampleRate;
use fx::{FXSettings, GlobalFX};
use midir::{InitError, MidiInput, MidiInputConnection, MidiInputPort};
use fundsp::hacker32::*;
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig};
use module::{EventData, Module, TrackTarget, TICKS_PER_BEAT};
use playback::{Player, RenderUpdate};
use synth::{Key, KeyOrigin};
use macroquad::prelude::*;

pub mod pitch;
mod input;
mod config;
pub mod synth;
pub mod fx;
pub mod ui;
pub mod module;
pub mod playback;
mod dsp;

use input::{Action, Hotkey, MidiEvent, Modifiers};
use ui::general_tab::TableCache;
use ui::info::Info;
use ui::instruments_tab::fix_patch_index;
use ui::pattern_tab::PatternEditor;

/// Application name, for window title, etc.
pub const APP_NAME: &str = "Osctet";
const MODULE_FILETYPE_NAME: &str = "Osctet module";
const MODULE_EXT: &str = "osctet";

const TABS: [&str; 4] = ["General", "Pattern", "Instruments", "Settings"];

struct Midi {
    // Keep one input around for listing ports. If we need to connect, we'll
    // create a new input just for that (see Boddlnagg/midir#90).
    input: Option<MidiInput>,
    port_name: Option<String>,
    port_selection: Option<String>,
    conn: Option<MidiInputConnection<Sender<Vec<u8>>>>,
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

    fn new_input(&mut self) -> Result<MidiInput, InitError> {
        self.input_id += 1;
        MidiInput::new(&format!("{} input #{}", APP_NAME, self.input_id))
    }

    fn selected_port(&self) -> Result<MidiInputPort, &'static str> {
        if let Some(selection) = &self.port_selection {
            if let Some(input) = &self.input {
                for port in input.ports() {
                    if let Ok(name) = input.port_name(&port) {
                        if name == *selection {
                            return Ok(port)
                        }
                    }
                }
                Err("Selected MIDI device not found")
            } else {
                Err("Could not open MIDI")
            }
        } else {
            Err("No MIDI device selected")
        }
    }
}

const MAIN_TAB_ID: &str = "main";
const TAB_GENERAL: usize = 0;
const TAB_PATTERN: usize = 1;
const TAB_INSTRUMENTS: usize = 2;
const TAB_SETTINGS: usize = 3;

struct App {
    octave: i8,
    midi: Midi,
    config: Config,
    fx: GlobalFX,
    patch_index: Option<usize>, // if None, kit is selected
    ui: ui::UI,
    pattern_editor: PatternEditor,
    general_scroll: f32,
    instruments_scroll: f32,
    settings_scroll: f32,
    save_path: Option<PathBuf>,
    render_channel: Option<Receiver<RenderUpdate>>,
    sample_rate: u32,
    table_cache: Option<TableCache>,
    version: String,
}

impl App {
    fn new(global_fx: GlobalFX, config: Config, sample_rate: u32) -> Self {
        let mut midi = Midi::new();
        midi.port_selection = config.default_midi_input.clone();
        App {
            octave: 3,
            midi,
            ui: ui::UI::new(config.theme.clone(), config.font_size),
            config,
            fx: global_fx,
            patch_index: Some(0),
            pattern_editor: PatternEditor::new(),
            general_scroll: 0.0,
            instruments_scroll: 0.0,
            settings_scroll: 0.0,
            save_path: None,
            render_channel: None,
            sample_rate,
            table_cache: None,
            version: format!("v{}-pre1", env::var("CARGO_PKG_VERSION").unwrap()),
        }
    }

    // TODO: switching tracks while keyjazzing could result in stuck notes
    // TODO: entering note input mode while keyjazzing could result in stuck notes
    // TODO: can keyjazzing mess up synth memory in a way that matters?
    fn keyjazz_track(&self) -> usize {
        if self.ui.get_tab(MAIN_TAB_ID) == Some(TAB_PATTERN) {
            self.pattern_editor.cursor_track()
        } else {
            0
        }
    }

    fn keyjazz_patch_index(&self, module: &Module) -> Option<usize> {
        match module.tracks[self.keyjazz_track()].target {
            TrackTarget::Global | TrackTarget::None => self.patch_index,
            TrackTarget::Kit => None,
            TrackTarget::Patch(i) => Some(i),
        }
    }

    // TODO: use most current vel/mod setting when keyjazzing in pattern
    fn handle_keys(&mut self, module: &mut Module, player: &mut Player) {
        let (pressed, released) = (get_keys_pressed(), get_keys_released());
        let mods = Modifiers::current();

        for key in released {
            let hk = Hotkey::new(mods, key);
            if let Some(_) = input::note_from_key(
                hk, &module.tuning, self.octave, &self.config) {
                let key = Key {
                    origin: KeyOrigin::Keyboard,
                    channel: 0,
                    key: input::u8_from_key(key),
                };
                self.ui.note_queue.push((key.clone(), EventData::NoteOff));
                player.note_off(self.keyjazz_track(), key);
            }
        }

        for key in pressed {
            let hk = Hotkey::new(mods, key);
            if let Some(action) = self.config.hotkey_action(&hk) {
                match action {
                    Action::IncrementDivision => self.pattern_editor.inc_division(),
                    Action::DecrementDivision => self.pattern_editor.dec_division(),
                    Action::DoubleDivision => self.pattern_editor.double_division(),
                    Action::HalveDivision => self.pattern_editor.halve_division(),
                    Action::IncrementOctave => self.octave += 1,
                    Action::DecrementOctave => self.octave -= 1,
                    Action::PlayFromStart => if player.is_playing() {
                        player.stop()
                    } else {
                        player.play_from(0, &module)
                    }
                    Action::PlayFromLoop => if player.is_playing() {
                        player.stop()
                    } else {
                        let tick = module.nearest_loop(self.pattern_editor.cursor_tick())
                            .unwrap_or_default();
                        player.play_from(tick, &module)
                    }
                    Action::PlayFromCursor => if player.is_playing() {
                        player.stop()
                    } else {
                        player.play_from(self.pattern_editor.cursor_tick(), &module)
                    }
                    Action::StopPlayback => player.stop(),
                    // TODO: prompt if unsaved
                    Action::NewSong => self.new_module(module, player),
                    // TODO: prompt if unsaved
                    Action::OpenSong=> self.open_module(module, player),
                    Action::SaveSong => self.save_module(module, player),
                    Action::SaveSongAs => self.save_module_as(module, player),
                    Action::RenderSong => self.render_and_save(&module, player, false),
                    Action::RenderTracks => self.render_and_save(&module, player, true),
                    // TODO: undo/redo are silent right now, which could be confusing when
                    //       things are being undone/redone offscreen. could either provide
                    //       messages describing what's being done, or move view to location
                    //       of changes
                    Action::Undo => if module.undo() {
                        player.update_synths(module.drain_track_history());
                        fix_patch_index(&mut self.patch_index, module.patches.len());
                    } else {
                        self.ui.report("Nothing to undo");
                    },
                    Action::Redo => if module.redo() {
                        player.update_synths(module.drain_track_history());
                        fix_patch_index(&mut self.patch_index, module.patches.len());
                    } else {
                        self.ui.report("Nothing to redo");
                    },
                    Action::NextTab => self.ui.next_tab(MAIN_TAB_ID, TABS.len()),
                    Action::PrevTab => self.ui.prev_tab(MAIN_TAB_ID, TABS.len()),
                    Action::Panic => player.panic(),
                    _ => if self.ui.get_tab(MAIN_TAB_ID) == Some(TAB_PATTERN) {
                        self.pattern_editor.action(*action, module, &self.config, player);
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
                            self.pattern_editor
                                .action(*action, module, &self.config, player),
                    _ => (),
                }
            }

            if let Some(note) = input::note_from_key(
                hk, &module.tuning, self.octave, &self.config) {
                let key = Key {
                    origin: KeyOrigin::Keyboard,
                    channel: 0,
                    key: input::u8_from_key(key),
                };
                self.ui.note_queue.push((key.clone(), EventData::Pitch(note)));
                if !self.ui.accepting_note_input()
                    && !self.pattern_editor.in_digit_column(&self.ui)
                    && !self.pattern_editor.in_global_track(&self.ui) {
                    if let Some((patch, note)) =
                        module.map_input(self.keyjazz_patch_index(&module), note) {
                        let pitch = module.tuning.midi_pitch(&note);
                        player.note_on(self.keyjazz_track(), key, pitch, None, patch);
                    }
                }
            }
        }
    }

    fn midi_connect(&mut self) -> Result<MidiInputConnection<Sender<Vec<u8>>>, Box<dyn Error>> {
        match self.midi.selected_port() {
            Ok(port) => {
                match self.midi.new_input() {
                    Ok(mut input) => {
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
                    },
                    Err(e) => Err(Box::new(e)),
                }
            },
            Err(e) => Err(e.into()),
        }
    }

    fn handle_midi(&mut self, module: &Module, player: &mut Player) {
        if let Some(rx) = &self.midi.rx {
            while let Ok(v) = rx.try_recv() {
                if let Some(evt) = MidiEvent::parse(&v) {
                    match evt {
                        MidiEvent::NoteOff { channel, key, .. } => {
                            let key = Key {
                                origin: KeyOrigin::Midi,
                                channel,
                                key,
                            };
                            player.note_off(self.keyjazz_track(), key.clone());
                            self.ui.note_queue.push((key, EventData::NoteOff));
                        },
                        MidiEvent::NoteOn { channel, key, velocity } => {
                            let key = Key {
                                origin: KeyOrigin::Midi,
                                channel,
                                key,
                            };
                            if velocity != 0 {
                                let note = input::note_from_midi(
                                    key.key, &module.tuning, &self.config);
                                let index = self.keyjazz_patch_index(module);
                                if let Some((patch, note)) = module.map_input(index, note) {
                                    let pitch = module.tuning.midi_pitch(&note);
                                    let pressure = velocity as f32 / 127.0;
                                    if !self.ui.accepting_note_input() {
                                        player.note_on(self.keyjazz_track(),
                                            key.clone(), pitch, Some(pressure), patch);
                                    }
                                    self.ui.note_queue.push((key.clone(),
                                        EventData::Pitch(note)));
                                    let v = (velocity as f32 * EventData::DIGIT_MAX as f32
                                        / 127.0).round() as u8;
                                    self.ui.note_queue.push((key, EventData::Pressure(v)));
                                }
                            } else {
                                player.note_off(self.keyjazz_track(), key.clone());
                                self.ui.note_queue.push((key, EventData::NoteOff));
                            }
                        },
                        MidiEvent::PolyPressure { channel, key, pressure } => {
                            if self.config.midi_send_pressure == Some(true) {
                                let key = Key {
                                    origin: KeyOrigin::Midi,
                                    channel,
                                    key,
                                };
                                player.poly_pressure(self.keyjazz_track(), key.clone(),
                                    pressure as f32 / 127.0);
                                let v = (pressure as f32 * EventData::DIGIT_MAX as f32
                                    / 127.0).round() as u8;
                                self.ui.note_queue.push((key, EventData::Pressure(v)));
                            }
                        },
                        MidiEvent::Controller { channel, controller, value } => {
                            match controller {
                                input::CC_MODULATION | input::CC_MACRO_MIN..=input::CC_MACRO_MAX => {
                                    player.modulate(self.keyjazz_track(),
                                        channel, value as f32 / 127.0);
                                },
                                input::CC_RPN_MSB => self.midi.rpn.0 = value,
                                input::CC_RPN_LSB => self.midi.rpn.1 = value,
                                input::CC_DATA_ENTRY_MSB => if self.midi.rpn == input::RPN_PITCH_BEND_SENSITIVITY {
                                    // set semitones
                                    self.midi.bend_range = self.midi.bend_range % 1.0 + value as f32;
                                },
                                input:: CC_DATA_ENTRY_LSB => if self.midi.rpn == input::RPN_PITCH_BEND_SENSITIVITY {
                                    // set cents
                                    self.midi.bend_range = self.midi.bend_range.round() + value as f32 / 100.0;
                                },
                                _ => (),
                            }
                        },
                        MidiEvent::ChannelPressure { channel, pressure } => {
                            if self.config.midi_send_pressure == Some(true) {
                                player.channel_pressure(self.keyjazz_track(),
                                    channel, pressure as f32 / 127.0);
                                let key = Key {
                                    origin: KeyOrigin::Midi,
                                    channel,
                                    key: 0,
                                };
                                let v = (pressure as f32 * EventData::DIGIT_MAX as f32
                                    / 127.0).round() as u8;
                                self.ui.note_queue.push((key, EventData::Pressure(v)));
                            }
                        },
                        // TODO: send event on note queue
                        MidiEvent::Pitch { channel, bend } => {
                            player.pitch_bend(self.keyjazz_track(),
                                channel, bend * self.midi.bend_range);
                        },
                    }
                }
            }
        }
    }

    fn check_midi_reconnect(&mut self) {
        if self.midi.port_selection.is_some() && self.midi.port_selection != self.midi.port_name {
            match self.midi_connect() {
                Ok(conn) => {
                    let old_conn = std::mem::replace(&mut self.midi.conn, Some(conn));
                    if let Some(c) = old_conn {
                        c.close();
                    }
                    self.midi.port_name = self.midi.port_selection.clone();
                    self.config.default_midi_input = self.midi.port_name.clone();
                },
                Err(e) => {
                    self.midi.port_selection = None;
                    self.ui.report(format!("MIDI connection failed: {e}"));
                },
            }
        } else if self.midi.port_selection.is_none() && self.midi.port_name.is_some() {
            if let Some(c) = self.midi.conn.take() {
                c.close();
            }
            self.midi.port_name = None;
        }
    }

    /// Returns false if it's quitting time.
    fn frame(&mut self, module: &Arc<Mutex<Module>>, player: &Arc<Mutex<Player>>) -> bool {
        if is_quit_requested() {
            if let Err(e) = self.config.save(self.ui.style.theme.clone()) {
                eprintln!("error saving config: {}", e);
            }
            return false
        }

        {
            let mut module = module.lock().unwrap();
            let mut player = player.lock().unwrap();

            if self.ui.accepting_keyboard_input() {
                player.clear_notes_with_origin(KeyOrigin::Keyboard);
            } else {
                self.handle_keys(&mut module, &mut player);
            }
            if self.ui.accepting_note_input() {
                player.clear_notes_with_origin(KeyOrigin::Midi);
            }

            if player.is_playing() {
                let end_tick = module.last_event_tick().unwrap_or_default()
                    + TICKS_PER_BEAT;
                if player.get_tick() > end_tick {
                    player.stop()
                }
            }

            self.handle_midi(&module, &mut player);
        }

        self.handle_render_updates();
        self.check_midi_reconnect();
        self.process_ui(module, player);

        true
    }

    fn handle_render_updates(&mut self) {
        if let Some(rx) = &self.render_channel {
            while let Ok(update) = rx.try_recv() {
                match update {
                    RenderUpdate::Progress(f) =>
                        self.ui.notify(format!("Rendering: {}%", (f * 100.0).round())),
                    RenderUpdate::Done(wav, path) => match wav.save_wav16(path) {
                        Ok(_) => self.ui.notify(String::from("Wrote WAV.")),
                        Err(e) => self.ui.report(format!("Writing WAV failed: {e}")),
                    }
                }
            }
        }
    }

    fn process_ui(&mut self, module: &Arc<Mutex<Module>>, player: &Arc<Mutex<Player>>) {
        self.ui.start_frame(&self.config);

        {
            let mut module = module.lock().unwrap();
            let mut player = player.lock().unwrap();

            self.bottom_panel(&mut player);

            match self.ui.tab_menu(MAIN_TAB_ID, &TABS, &self.version) {
                TAB_GENERAL => ui::general_tab::draw(&mut self.ui, &mut module,
                    &mut self.fx, &mut self.config, &mut player, &mut self.general_scroll,
                    &mut self.table_cache),
                TAB_PATTERN => ui::pattern_tab::draw(&mut self.ui, &mut module,
                    &mut player, &mut self.pattern_editor, &self.config),
                TAB_INSTRUMENTS => ui::instruments_tab::draw(&mut self.ui, &mut module,
                    &mut self.patch_index, &mut self.instruments_scroll, &mut self.config,
                    &mut player), // so, basically everything
                TAB_SETTINGS => ui::settings_tab::draw(&mut self.ui, &mut self.config,
                    &mut self.settings_scroll, self.sample_rate),
                _ => panic!("bad tab value"),
            }
        }

        self.ui.end_frame();
    }

    fn bottom_panel(&mut self, player: &mut Player) {
        self.ui.start_bottom_panel();

        if self.midi.input.is_some() {
            let s = if let Some(name) = &self.midi.port_name {
                &name
            } else {
                "(none)"
            };
            if let Some(i) = self.ui.combo_box("midi_input", "MIDI input", s,
                Info::MidiInput, || input_names(self.midi.input.as_ref().unwrap())) {
                self.midi.port_selection = if i == 0 {
                    None
                } else {
                    input_names(self.midi.input.as_ref().unwrap()).get(i).cloned()
                };
            }

            let mut v = self.config.midi_send_pressure.unwrap_or(true);
            if self.ui.checkbox("Use aftertouch", &mut v, self.midi.port_name.is_some(),
                Info::Aftertouch) {
                self.config.midi_send_pressure = Some(v);
            }
        } else {
            self.ui.label("No MIDI device");
        }

        if let Some(n) = self.ui.edit_box("Division", 3,
            self.pattern_editor.beat_division.to_string(), Info::Division
        ) {
            match n.parse::<u8>() {
                Ok(n) => self.pattern_editor.set_division(n),
                Err(e) => self.ui.report(e),
            }
        }

        if let Some(n) = self.ui.edit_box("Octave", 2, self.octave.to_string(),
            Info::Octave
        ) {
            match n.parse::<i8>() {
                Ok(n) => self.octave = n,
                Err(e) => self.ui.report(e),
            }
        }

        self.ui.shared_slider("stereo_width", "Stereo width",
            &mut player.stereo_width, -1.0..=1.0, None, 1, true, Info::StereoWidth);

        self.ui.end_bottom_panel();
    }

    fn render_and_save(&mut self, module: &Module, player: &mut Player, tracks: bool) {
        if module.ends() {
            if let Some(mut path) = ui::new_file_dialog(player)
                .add_filter("WAV file", &["wav"])
                .set_directory(self.config.render_folder.clone()
                    .unwrap_or(String::from(".")))
                .set_file_name(module.title.clone())
                .save_file() {
                path.set_extension("wav");
                self.config.render_folder = config::dir_as_string(&path);
                let module = Arc::new(module.clone());
                self.render_channel = Some(if tracks {
                    playback::render_tracks(module, path)
                } else {
                    playback::render(module, path, None)
                });
            }
        } else {
            self.ui.report("Module must have END event to export")
        }
    }

    fn new_module(&mut self, module: &mut Module, player: &mut Player) {
        self.load_module(module, Module::new(Default::default()), player);
        self.save_path = None;
    }

    fn save_module(&mut self, module: &mut Module, player: &mut Player) {
        if let Some(path) = &self.save_path {
            if let Err(e) = module.save(self.pattern_editor.beat_division, path) {
                self.ui.report(format!("Error saving module: {e}"));
            } else {
                self.ui.notify(String::from("Saved module."));
            }
        } else {
            self.save_module_as(module, player);
        }
    }

    fn save_module_as(&mut self, module: &mut Module, player: &mut Player) {
        if let Some(mut path) = ui::new_file_dialog(player)
            .add_filter(MODULE_FILETYPE_NAME, &[MODULE_EXT])
            .set_directory(self.config.module_folder.clone()
                .unwrap_or(String::from(".")))
            .set_file_name(module.title.clone())
            .save_file() {
            path.set_extension(MODULE_EXT);
            self.config.module_folder = config::dir_as_string(&path);
            if let Err(e) = module.save(self.pattern_editor.beat_division, &path) {
                self.ui.report(format!("Error saving module: {e}"));
            } else {
                self.save_path = Some(path);
                self.ui.notify(String::from("Saved module."));
            }
        }
    }

    fn open_module(&mut self, module: &mut Module, player: &mut Player) {
        if let Some(path) = ui::new_file_dialog(player)
            .add_filter(MODULE_FILETYPE_NAME, &[MODULE_EXT])
            .set_directory(self.config.module_folder.clone()
                .unwrap_or(String::from(".")))
            .pick_file() {
            self.config.module_folder = config::dir_as_string(&path);
            match Module::load(&path) {
                Ok(new_module) => {
                    self.load_module(module, new_module, player);
                    self.save_path = Some(path);
                },
                Err(e) => self.ui.report(format!("Error loading module: {e}")),
            }
        }
    }

    fn load_module(&mut self,
        old_module: &mut Module, module: Module, player: &mut Player) {
        *old_module = module;
        self.pattern_editor = PatternEditor::new();
        self.pattern_editor.beat_division = old_module.division;
        self.patch_index = None;
        player.reinit(old_module.tracks.len());
        self.fx.reinit(&old_module.fx);
    }
}

fn input_names(input: &MidiInput) -> Vec<String> {
    let mut v = vec![String::from("(none)")];
    v.extend(input.ports().into_iter()
        .map(|p| input.port_name(&p).unwrap_or(String::from("(unknown)"))));
    v
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

fn preferred_config(device: &cpal::Device, desired_sr: SampleRate
) -> Result<StreamConfig, Box<dyn Error>> {
    device.supported_output_configs()?
        .filter(|conf| conf.channels() == 2)
        .max_by_key(|conf| (
            conf.sample_format().sample_size() > 1,
            conf.max_sample_rate() >= desired_sr
        )).map(|conf| {
            let sr = desired_sr
                .clamp(conf.min_sample_rate(), conf.max_sample_rate());
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

    let mut seq = Sequencer::new(false, 4);
    seq.set_sample_rate(sample_rate as f64);

    let fx_settings: FXSettings = Default::default();
    let mut global_fx = GlobalFX::new(seq.backend(), &fx_settings);
    global_fx.net.set_sample_rate(sample_rate as f64);
    let mut backend = BlockRateAdapter::new(Box::new(global_fx.net.backend()));

    let module = Module::new(fx_settings);
    let player = Player::new(seq, module.tracks.len(), sample_rate as f32);
    let module = Arc::new(Mutex::new(module));
    let player = Arc::new(Mutex::new(player));

    const UPDATE_FRAMES: u32 = 64;
    let update_interval: f64 = UPDATE_FRAMES as f64 / sample_rate as f64;
    let mut frames_until_update = UPDATE_FRAMES;

    let stream_module = module.clone();
    let stream_player = player.clone();

    let stream = audio_conf.and_then(|config| {
        Ok(device.unwrap().build_output_stream(
            &config, move |data: &mut[f32], _: &cpal::OutputCallbackInfo| {
                // there's probably a better way to do this
                let mut i = 0;
                let len = data.len();
                while i < len {
                    if frames_until_update == 0 {
                        let module = stream_module.lock().unwrap();
                        let mut player = stream_player.lock().unwrap();
                        player.frame(&module, update_interval);
                        frames_until_update = UPDATE_FRAMES;
                    }
                    let (l, r) = backend.get_stereo();
                    data[i] = l;
                    data[i+1] = r;
                    i += 2;
                    frames_until_update -= 1;
                }
            },
            move |err| {
                eprintln!("stream error: {}", err);
            },
            None
        )?)
    });

    let mut app = App::new(global_fx, conf, sample_rate);

    // ugly duplication, but error typing makes a nice solution difficult
    match &stream {
        Ok(stream) => if let Err(e) = stream.play() {
            app.ui.report(format!("Could not initialize audio: {}", e));
        }
        Err(e) => app.ui.report(format!("Could not initialize audio: {}", e))
    };

    if let Some(arg) = arg {
        match Module::load(&arg.into()) {
            Ok(m) => app.load_module(
                &mut module.lock().unwrap(), m, &mut player.lock().unwrap()),
            Err(e) => app.ui.report(format!("Error loading module: {e}")),
        }
    }

    while app.frame(&module, &player) {
        next_frame().await
    }

    Ok(())
}