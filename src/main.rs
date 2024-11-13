// disable console in windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::ops::RangeInclusive;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::VecDeque;

use config::Config;
use midir::{InitError, MidiInput, MidiInputConnection, MidiInputPort};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig};
use fundsp::hacker::*;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Ui};
use rfd::FileDialog;
use synth::{FilterType, Key, KeyOrigin, KeyTracking, ModTarget, PlayMode, Synth, Waveform};

mod pitch;
mod input;
mod config;
mod synth;
mod song;
mod adsr;

use input::MidiEvent;

const APP_NAME: &str = "Synth Tracker";

// for file dialogs
const PATCH_FILTER_NAME: &str = "Instrument";
const PATCH_FILTER_EXT: &str = "inst";

struct MessageBuffer {
    capacity: usize,
    messages: VecDeque<String>,
}

impl MessageBuffer {
    fn new(capacity: usize) -> Self {
        MessageBuffer {
            capacity,
            messages: VecDeque::new(),
        }
    }

    fn push(&mut self, msg: String) {
        self.messages.push_front(msg);
        self.messages.truncate(self.capacity);
    }

    fn report(&mut self, e: &impl std::fmt::Display) {
        self.push(format!("{}", e));
    }

    fn iter(&self) -> impl Iterator<Item = &'_ String> {
        self.messages.iter().rev()
    }
}

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

    fn selected_port(&self) -> Option<MidiInputPort> {
        self.port_selection.as_ref().map(|selection| {
            self.input.as_ref().map(|input| {
                for port in input.ports() {
                    if let Ok(name) = input.port_name(&port) {
                        if name == *selection {
                            return Some(port)
                        }
                    }
                }
                None
            })?
        })?
    }
}

struct App {
    tuning: pitch::Tuning,
    messages: MessageBuffer,
    synth: Synth,
    seq: Sequencer,
    octave: i8,
    midi: Midi,
    config: Config,
    selected_osc: usize,
    selected_env: usize,
    selected_lfo: usize,
    global_fx: GlobalFX,
    song_editor: song::Editor,
}

impl App {
    fn new(synth: Synth, seq: Sequencer, global_fx: GlobalFX, init_messages: Vec<String>) -> Self {
        let mut messages = MessageBuffer::new(100);
        for msg in init_messages {
            messages.push(msg);
        }
        let config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                messages.push(format!("Could not load config: {}", &e));
                Config::default()
            },
        };
        let mut midi = Midi::new();
        midi.port_selection = config.default_midi_input.clone();
        App {
            tuning: pitch::Tuning::divide(2.0, 12, 1).unwrap(),
            messages,
            synth,
            seq,
            octave: 4,
            midi,
            config,
            selected_osc: 0,
            selected_env: 0,
            selected_lfo: 0,
            global_fx,
            song_editor: song::Editor::new(song::Song::new()),
        }
    }

    fn handle_ui_event(&mut self, evt: &egui::Event) {
        match evt {
            egui::Event::Key { physical_key, pressed, repeat, .. } => {
                if let Some(key) = physical_key {
                    if let Some(note) = input::note_from_key(key, &self.tuning, self.octave) {
                        if *pressed && !*repeat {
                            self.synth.note_on(Key {
                                origin: KeyOrigin::Keyboard,
                                channel: 0,
                                key: key.name().bytes().next().unwrap_or(0),
                            }, self.tuning.midi_pitch(&note), 100.0 / 127.0, &mut self.seq);
                        } else if !*pressed {
                            self.synth.note_off(Key {
                                origin: KeyOrigin::Keyboard,
                                channel: 0,
                                key: key.name().bytes().next().unwrap_or(0),
                            }, &mut self.seq);
                        }
                    }
                }
            },
            _ => (),
        }
    }

    fn midi_connect(&mut self, ctx: egui::Context) -> Result<MidiInputConnection<Sender<Vec<u8>>>, Box<dyn Error>> {
        match self.midi.selected_port() {
            Some(port) => {
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
                                // ignore the error here, it probably just means that the user
                                // changed ports
                                let _ = tx.send(message.to_vec());
                                ctx.request_repaint();
                            },
                            tx,
                        )?)
                    },
                    Err(e) => Err(Box::new(e)),
                }
            },
            None => Err("no MIDI port selected".into()),
        }
    }

    fn handle_midi(&mut self) {
        if let Some(rx) = &self.midi.rx {
            while let Ok(v) = rx.try_recv() {
                match MidiEvent::parse(&v) {
                    Some(MidiEvent::NoteOff { channel, key, .. }) => {
                        self.synth.note_off(Key{
                            origin: KeyOrigin::Midi,
                            channel: channel,
                            key: key,
                        }, &mut self.seq);
                    },
                    Some(MidiEvent::NoteOn { channel, key, velocity }) => {
                        if velocity != 0 {
                            let note = input::note_from_midi(v[1] as i8, &self.tuning);
                            self.synth.note_on(Key {
                                origin: KeyOrigin::Midi,
                                channel: channel,
                                key: key,
                            }, self.tuning.midi_pitch(&note), velocity as f32 / 127.0, &mut self.seq);
                        } else {
                            self.synth.note_off(Key {
                                origin: KeyOrigin::Midi,
                                channel: channel,
                                key: key,
                            }, &mut self.seq);
                        }
                    },
                    Some(MidiEvent::PolyPressure { channel, key, pressure }) => {
                        if self.config.midi_send_pressure == Some(true) {
                            self.synth.poly_pressure(Key {
                                origin: KeyOrigin::Midi,
                                channel: channel,
                                key: key,
                            }, pressure as f32 / 127.0);
                        }
                    },
                    Some(MidiEvent::Controller { controller, value, .. }) => {
                        match controller {
                            input::CC_MODULATION | input::CC_MACRO_MIN..=input::CC_MACRO_MAX => {
                                self.synth.modulate(value as f32 / 127.0);
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
                    Some(MidiEvent::ChannelPressure { channel, pressure }) => {
                        if self.config.midi_send_pressure == Some(true) {
                            self.synth.channel_pressure(channel, pressure as f32 / 127.0);
                        }
                    },
                    Some(MidiEvent::Pitch { channel, bend }) => {
                        self.synth.pitch_bend(channel, bend * self.midi.bend_range);
                    },
                    None => (),
                }
            }
        }
    }
}

fn shared_slider(ui: &mut Ui, var: &Shared, range: RangeInclusive<f32>, text: &str, log: bool) {
    let mut val = var.value();
    ui.add(egui::Slider::new(&mut val, range).text(text).logarithmic(log));
    if val != var.value() {
        var.set_value(val);
    }
}

fn make_reverb(r: f32, t: f32, d: f32, m: f32, damp: f32) -> Box<dyn AudioUnit> {
    Box::new(reverb2_stereo(r, t, d, m, highshelf_hz(5000.0, 1.0, db_amp(-damp))))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // process UI input
        ctx.input(|input| {
            for evt in input.events.iter() {
                self.handle_ui_event(evt);
            }
        });

        // process MIDI input
        self.handle_midi();

        // bottom panel
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            if self.midi.input.is_some() {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("MIDI input port")
                        .selected_text(self.midi.port_name.clone().unwrap_or("(none)".to_string()))
                        .show_ui(ui, |ui| {
                            let input = self.midi.input.as_ref()
                                .expect("MIDI input was just checked");
                            for p in input.ports() {
                                let name = input.port_name(&p).unwrap_or(String::from("(unknown)"));
                                ui.selectable_value(&mut self.midi.port_selection, Some(name.clone()), name);
                            }
                        });
                    if self.midi.port_selection.is_some() && self.midi.port_selection != self.midi.port_name {
                        match self.midi_connect(ctx.clone()) {
                            Ok(conn) => {
                                let old_conn = std::mem::replace(&mut self.midi.conn, Some(conn));
                                if let Some(c) = old_conn {
                                    c.close();
                                }
                                self.midi.port_name = self.midi.port_selection.clone();
                                self.midi.port_name.as_ref().inspect(|name| {
                                    self.messages.push(format!("Connected to {} for MIDI input", name));
                                });
                                self.config.default_midi_input = self.midi.port_name.clone();
                                if let Err(e) = self.config.save() {
                                    self.messages.push(format!("Error saving config: {}", e));
                                };
                            },
                            Err(e) => {
                                self.midi.port_selection = None;
                                self.messages.report(&e);
                            },
                        }
                    }
                    let mut v = self.config.midi_send_pressure.unwrap_or(true);
                    ui.checkbox(&mut v, "Send pressure");
                    if Some(v) != self.config.midi_send_pressure {
                        self.config.midi_send_pressure = Some(v);
                        if let Err(e) = self.config.save() {
                            self.messages.report(&e);
                        }
                    }
                });
            }
        });

        // instrument panel
        egui::SidePanel::left("left_panel").resizable(false).show(ctx, |ui| {
            // global controls
            {
                let mut commit = false;
                let fx = &mut self.global_fx;
                shared_slider(ui, &fx.reverb_amount, 0.0..=1.0, "Reverb level", false);
                let (predelay_time, room_size, time, diffusion, mod_speed, damping) =
                    (fx.predelay_time, fx.reverb_room_size, fx.reverb_time,
                        fx.reverb_diffusion, fx.reverb_mod_speed, fx.reverb_damping);
                ui.add(egui::Slider::new(&mut fx.predelay_time, 0.0..=0.1).text("Predelay"));
                ui.add(egui::Slider::new(&mut fx.reverb_room_size, 5.0..=100.0).text("Room size").logarithmic(true));
                ui.add(egui::Slider::new(&mut fx.reverb_time, 0.1..=10.0).text("Time").logarithmic(true));
                ui.add(egui::Slider::new(&mut fx.reverb_diffusion, 0.0..=1.0).text("Diffusion"));
                ui.add(egui::Slider::new(&mut fx.reverb_mod_speed, 0.0..=1.0).text("Mod speed"));
                ui.add(egui::Slider::new(&mut fx.reverb_damping, 0.0..=6.0).text("HF damping"));
                if predelay_time != fx.predelay_time {
                    fx.net.crossfade(fx.predelay_id, Fade::Smooth, 0.1,
                        Box::new(delay(predelay_time) | delay(predelay_time)));
                    commit = true;
                }
                if room_size != fx.reverb_room_size || time != fx.reverb_time ||
                    diffusion != fx.reverb_diffusion || mod_speed != fx.reverb_mod_speed ||
                    damping != fx.reverb_damping {
                    fx.net.crossfade(fx.reverb_id, Fade::Smooth, 0.1,
                        make_reverb(fx.reverb_room_size, fx.reverb_time, fx.reverb_diffusion,
                            fx.reverb_mod_speed, fx.reverb_damping));
                    commit = true;
                }
                if commit {
                    fx.net.commit();
                }
                ui.separator();
            }

            // play mode control
            let settings = &mut self.synth.settings;
            egui::ComboBox::from_label("Play mode")
                .selected_text(settings.play_mode.name())
                .show_ui(ui, |ui| {
                    for variant in PlayMode::VARIANTS {
                        ui.selectable_value(&mut settings.play_mode, variant, variant.name());
                    }
                });

            // glide time slider
            // FIXME: this doesn't update voices that are already playing
            ui.add(egui::Slider::new(&mut settings.glide_time, 0.0..=0.5).text("Glide"));
            shared_slider(ui, &settings.pan.0, -1.0..=1.0, "Pan", false);

            // oscillator controls
            ui.separator();
            ui.horizontal(|ui| {
                for i in 0..settings.oscs.len() {
                    ui.selectable_value(&mut self.selected_osc, i, format!("Osc {}", i + 1));
                }
                if settings.oscs.len() < synth::MAX_OSCS && ui.button("+").clicked() {
                    settings.oscs.push(synth::Oscillator::new());
                    self.selected_osc = settings.oscs.len() - 1;
                }
                if settings.oscs.len() > 1 && ui.button("-").clicked() {
                    settings.remove_osc(self.selected_osc);
                    if self.selected_osc >= settings.oscs.len() {
                        self.selected_osc -= 1;
                    }
                }
            });
            let outputs = settings.outputs(self.selected_osc);
            if let Some(osc) = settings.oscs.get_mut(self.selected_osc) {
                shared_slider(ui, &osc.level.0, 0.0..=1.0, "Level", false);
                shared_slider(ui, &osc.freq_ratio.0, 0.25..=16.0, "Freq. ratio", true);
                shared_slider(ui, &osc.fine_pitch.0, -0.5..=0.5, "Fine pitch", false);
                ui.add_enabled_ui(osc.waveform == Waveform::Pulse || osc.waveform == Waveform::Noise, |ui| {
                    shared_slider(ui, &osc.tone.0, 0.0..=1.0, "Tone", false);
                });
                egui::ComboBox::new("osc_waveform", "Waveform")
                    .selected_text(osc.waveform.name())
                    .show_ui(ui, |ui| {
                        for variant in Waveform::VARIANTS {
                            ui.selectable_value(&mut osc.waveform, variant, variant.name());
                        }
                    });
                ui.add_enabled_ui(self.selected_osc != 0, |ui| {
                    egui::ComboBox::from_label("Output")
                        .selected_text(osc.output.to_string())
                        .show_ui(ui, |ui| {
                            for variant in &outputs {
                                ui.selectable_value(&mut osc.output, *variant, variant.to_string());
                            }
                        });
                });
            }
            ui.separator();

            // filter controls
            ui.label("Filter");
            let filter = &mut settings.filter;
            egui::ComboBox::from_label("Type")
                .selected_text(filter.filter_type.name())
                .show_ui(ui, |ui| {
                    for variant in FilterType::VARIANTS {
                        ui.selectable_value(&mut filter.filter_type, variant, variant.name());
                    }
                });
            ui.add_enabled_ui(filter.filter_type != FilterType::Off, |ui| {
                shared_slider(ui, &filter.cutoff.0, 20.0..=20_000.0, "Cutoff", true);
                shared_slider(ui, &filter.resonance.0, 0.1..=1.0, "Resonance", false);
                egui::ComboBox::from_label("Key tracking")
                    .selected_text(filter.key_tracking.name())
                    .show_ui(ui, |ui| {
                        for variant in KeyTracking::VARIANTS {
                            ui.selectable_value(&mut filter.key_tracking, variant, variant.name());
                        }
                    });
            });

            // envelopes
            ui.separator();
            ui.horizontal(|ui| {
                if settings.envs.is_empty() {
                    ui.label("Envelopes");
                }
                for i in 0..settings.envs.len() {
                    ui.selectable_value(&mut self.selected_env, i, format!("Env {}", i + 1));
                }
                if settings.envs.len() < synth::MAX_ENVS && ui.button("+").clicked() {
                    settings.envs.push(synth::ADSR::new());
                    if !settings.envs.is_empty() {
                        self.selected_env = settings.envs.len() - 1;
                    }
                }
                if ui.button("-").clicked() {
                    settings.remove_env(self.selected_env);
                    if self.selected_env > 0 && self.selected_env >= settings.envs.len() {
                        self.selected_env -= 1;
                    }
                }
            });
            if let Some(env) = settings.envs.get_mut(self.selected_env) {
                ui.add(egui::Slider::new(&mut env.attack, 0.0..=10.0).text("Attack").logarithmic(true));
                ui.add(egui::Slider::new(&mut env.decay, 0.01..=10.0).text("Decay").logarithmic(true));
                ui.add(egui::Slider::new(&mut env.sustain, 0.0..=1.0).text("Sustain"));
                ui.add(egui::Slider::new(&mut env.release, 0.01..=10.0).text("Release").logarithmic(true));
                egui::ComboBox::from_label("Curve")
                    .selected_text(env.curve_name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut env.power, 1.0, "Linear");
                        ui.selectable_value(&mut env.power, 2.0, "Quadratic");
                        ui.selectable_value(&mut env.power, 3.0, "Cubic");
                    });
            }

            // LFOs
            ui.separator();
            ui.horizontal(|ui| {
                if settings.lfos.is_empty() {
                    ui.label("LFOs");
                }
                for i in 0..settings.lfos.len() {
                    ui.selectable_value(&mut self.selected_lfo, i, format!("LFO {}", i + 1));
                }
                if settings.lfos.len() < synth::MAX_LFOS && ui.button("+").clicked() {
                    settings.lfos.push(synth::LFO::new());
                    if !settings.lfos.is_empty() {
                        self.selected_lfo = settings.lfos.len() - 1;
                    }
                }
                if ui.button("-").clicked() {
                    settings.remove_lfo(self.selected_lfo);
                    if self.selected_lfo > 0 && self.selected_lfo >= settings.lfos.len() {
                        self.selected_lfo -= 1;
                    }
                }
            });
            if let Some(lfo) = settings.lfos.get_mut(self.selected_lfo) {
                egui::ComboBox::new("lfo_waveform", "Waveform")
                    .selected_text(lfo.waveform.name())
                    .show_ui(ui, |ui| {
                        for variant in Waveform::VARIANTS {
                            ui.selectable_value(&mut lfo.waveform, variant, variant.name());
                        }
                    });
                shared_slider(ui, &lfo.freq.0, 0.1..=20.0, "Frequency", true);
                ui.add(egui::Slider::new(&mut lfo.delay, 0.0..=10.0).text("Delay"));
            }

            // mod matrix
            ui.separator();
            ui.add(egui::Label::new("Modulation"));
            egui::Grid::new("mod_matrix")
                .num_columns(3)
                .show(ui, |ui| {
                    // header
                    ui.label("Source");
                    ui.label("Target");
                    ui.label("Depth");
                    ui.label("");
                    ui.end_row();

                    let mut removal_index: Option<usize> = None;
                    let sources = settings.mod_sources();
                    let targets = settings.mod_targets();
                    for (i, m) in settings.mod_matrix.iter_mut().enumerate() {
                        egui::ComboBox::from_id_salt(format!("mod_source_{}", i))
                            .selected_text(m.source.to_string())
                            .show_ui(ui, |ui| {
                                for variant in &sources {
                                    ui.selectable_value(&mut m.source, *variant, variant.to_string());
                                }
                            });
                        egui::ComboBox::from_id_salt(format!("mod_target_{}", i))
                            .selected_text(m.target.to_string())
                            .show_ui(ui, |ui| {
                                for variant in &targets {
                                    if let ModTarget::ModDepth(n) = *variant {
                                        if n >= i {
                                            // prevent infinite loops
                                            continue
                                        }
                                    }
                                    ui.selectable_value(&mut m.target, *variant, variant.to_string());
                                }
                            });
                        shared_slider(ui, &m.depth.0, -1.0..=1.0, "", false);
                        if ui.button("x").clicked() {
                            removal_index = Some(i);
                        }
                        ui.end_row();
                    }
                    if let Some(i) = removal_index {
                        settings.remove_mod(i);
                    }
                });
            if ui.button("+").clicked() {
                settings.mod_matrix.push(synth::Modulation::default());
            }

            // file ops
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save patch").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
                        .save_file() {
                        match self.synth.settings.save(&path) {
                            Ok(_) => self.messages.push(format!("Patch saved to {}", path.display())),
                            Err(e) => self.messages.report(&e),
                        }
                    }
                }
                if ui.button("Load patch").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
                        .pick_file() {
                        match synth::Settings::load(&path) {
                            Ok(patch) => {
                                self.synth.settings = patch;
                                self.messages.push(format!("Patch loaded from {}", path.display()));
                            },
                            Err(e) => self.messages.report(&e),
                        }
                    }
                }
            });

            // message area
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in self.messages.iter() {
                    ui.label(line);
                }
            });
        });

        // song panel
        egui::CentralPanel::default().show(ctx, |ui| {
            let line_height = 12.0;
            let font = FontId::monospace(line_height);
            let fg_color = Color32::BLACK;
            let cursor_color = Color32::GRAY;
            let char_width = 8.0;
            let channel_width = char_width * 10.0;
            let column_offsets = [0.0, char_width * 3.0, char_width * 5.0, char_width * 7.0];

            let (response, painter) = ui.allocate_painter(ui.available_size_before_wrap(), Sense::click());

            let cursor = &self.song_editor.cursor;
            let cursor_x = response.rect.min.x + 
                channel_width * cursor.channel as f32 +
                column_offsets[cursor.column as usize] +
                char_width * cursor.char as f32;
            let cursor_y = response.rect.min.y + line_height * 2.0 + cursor.tick as f32; // TODO
            let cursor_rect = Rect {
                min: Pos2 { x: cursor_x , y: cursor_y },
                max: Pos2 { x: cursor_x + char_width, y: cursor_y + line_height, },
            };
            painter.rect_filled(cursor_rect, 0.0, cursor_color);

            for (i, _channel) in self.song_editor.song.channels.iter().enumerate() {
                let pos = Pos2 {
                    x: channel_width * i as f32,
                    y: 0.0
                };
                painter.text(response.rect.min + pos.to_vec2(),
                    Align2::LEFT_TOP, format!("Channel {}", i + 1), font.clone(), fg_color);
            }

            if let Some(_pointer_pos) = response.interact_pointer_pos() {
                // TODO: update cursor position
            }
        });
    }
}

struct GlobalFX {
    predelay_id: NodeId,
    predelay_time: f32,
    reverb_id: NodeId,
    reverb_room_size: f32,
    reverb_time: f32,
    reverb_diffusion: f32,
    reverb_mod_speed: f32,
    reverb_damping: f32,
    reverb_amount: Shared,
    net: Net,
}

fn main() -> eframe::Result {
    // init audio
    let host = cpal::default_host();
    let device = host.default_output_device()
        .expect("no output device available");
    let mut configs = device.supported_output_configs()
        .expect("error querying output configs");
    let config: StreamConfig = configs.next()
        .expect("no supported output config")
        .with_max_sample_rate()
        .into();
    let synth = Synth::new();
    let mut seq = Sequencer::new(false, 2);
    seq.set_sample_rate(config.sample_rate.0 as f64);

    let predelay_time = 0.01;
    let reverb_room_size = 20.0;
    let reverb_time = 0.2;
    let reverb_diffusion = 0.5;
    let reverb_mod_speed = 0.5;
    let reverb_damping = 3.0;
    let (reverb, reverb_id) = Net::wrap_id(
        make_reverb(reverb_room_size, reverb_time, reverb_diffusion, reverb_mod_speed, reverb_damping));
    let (predelay, predelay_id) = Net::wrap_id(Box::new(delay(predelay_time) | delay(predelay_time)));
    let reverb_amount = shared(0.1);
    let mut net = Net::wrap(Box::new(seq.backend()))
        >> (highpass_hz(1.0, 0.1) | highpass_hz(1.0, 0.1))
        >> (shape(Tanh(1.0)) | shape(Tanh(1.0)))
        >> (multipass::<U2>() & (var(&reverb_amount) >> split::<U2>()) * (predelay >> reverb));
    let mut backend = BlockRateAdapter::new(Box::new(net.backend()));
    let global_fx = GlobalFX {
        predelay_time,
        predelay_id,
        reverb_id,
        reverb_room_size,
        reverb_time,
        reverb_diffusion,
        reverb_mod_speed,
        reverb_damping,
        reverb_amount,
        net,
    };

    // TODO: handle these errors correctly (without dropping the stream!)
    let errs = vec![];
    let stream = device.build_output_stream(
        &config,
        move |data: &mut[f32], _: &cpal::OutputCallbackInfo| {
            // there's probably a better way to do this
            let mut i = 0;
            let len = data.len();
            while i < len {
                let (l, r) = backend.get_stereo();
                data[i] = l;
                data[i+1] = r;
                i += 2;
            }
        },
        move |err| {
            eprintln!("stream error: {}", err);
        },
        None
    ).unwrap();
    stream.play().unwrap();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|_cc| {
            Ok(Box::new(App::new(synth, seq, global_fx, errs)))
        }),
    )
}