// disable console in windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ops::RangeInclusive;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::VecDeque;

use config::Config;
use midir::{InitError, MidiInput, MidiInputConnection, MidiInputPort};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig};
use fundsp::hacker::*;
use eframe::egui::{self, Ui};
use anyhow::{bail, Result};
use synth::{Key, KeyOrigin, PlayMode, Synth, Waveform};

mod pitch;
mod input;
mod config;
mod synth;

const APP_NAME: &str = "Synth Tracker";

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
}

impl App {
    fn new(synth: Synth, seq: Sequencer) -> Self {
        let mut messages = MessageBuffer::new(100);
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
                            }, self.tuning.midi_pitch(&note), &mut self.seq);
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

    fn midi_connect(&mut self, ctx: egui::Context) -> Result<MidiInputConnection<Sender<Vec<u8>>>> {
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
                    Err(e) => bail!(e),
                }
            },
            None => bail!("no MIDI port selected")
        }
    }

    fn handle_midi(&mut self) {
        if let Some(rx) = &self.midi.rx {
            loop {
                match rx.try_recv() {
                    Ok(v) => {
                        // FIXME: invalid MIDI could crash this
                        // FIXME: this shouldn't all be inlined here
                        match v[0] & 0xf0 {
                            0b10000000 => {
                                // note off
                                self.synth.note_off(Key{
                                    origin: KeyOrigin::Midi,
                                    channel: v[0] & 0xf,
                                    key: v[1],
                                }, &mut self.seq);
                            },
                            0b10010000 => {
                                // note on, unless velocity is zero
                                if v[2] != 0 {
                                    let note = input::note_from_midi(v[1] as i8, &self.tuning);
                                    self.synth.note_on(Key {
                                        origin: KeyOrigin::Midi,
                                        channel: v[0] & 0xf,
                                        key: v[1],
                                    }, self.tuning.midi_pitch(&note), &mut self.seq);
                                } else {
                                    self.synth.note_off(Key {
                                        origin: KeyOrigin::Midi,
                                        channel: v[0] & 0xf,
                                        key: v[1],
                                    }, &mut self.seq);
                                }
                            },
                            _ => (),
                        }
                    },
                    Err(_) => break,
                }
            }
        }
    }
}

fn shared_slider(ui: &mut Ui, var: &Shared, range: RangeInclusive<f32>, text: &str) {
    let mut val = var.value();
    ui.add(egui::Slider::new(&mut val, range).text(text));
    if val != var.value() {
        var.set_value(val);
    }
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
                egui::ComboBox::from_label("MIDI input port")
                    .selected_text(self.midi.port_name.clone().unwrap_or("(none)".to_string()))
                    .show_ui(ui, |ui| {
                        let input = self.midi.input.as_ref().unwrap();
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
            }
        });

        // message panel
        egui::CentralPanel::default().show(ctx, |ui| {
            // oscillator controlls
            shared_slider(ui, &self.synth.oscs[0].level, 0.0..=1.0, "Level");
            shared_slider(ui, &self.synth.oscs[0].duty, 0.0..=1.0, "Duty");
            egui::ComboBox::from_label("Waveform")
                .selected_text(self.synth.oscs[0].waveform.name())
                .show_ui(ui, |ui| {
                    for variant in Waveform::VARIANTS {
                        ui.selectable_value(&mut self.synth.oscs[0].waveform, variant, variant.name());
                    }
                });
            ui.add(egui::Slider::new(&mut self.synth.oscs[0].env.attack, 0.0..=10.0).text("Attack").logarithmic(true));
            ui.add(egui::Slider::new(&mut self.synth.oscs[0].env.decay, 0.0..=10.0).text("Decay").logarithmic(true));
            ui.add(egui::Slider::new(&mut self.synth.oscs[0].env.sustain, 0.0..=1.0).text("Sustain"));
            ui.add(egui::Slider::new(&mut self.synth.oscs[0].env.release, 0.0..=10.0).text("Release").logarithmic(true));

            // glide time slider
            // FIXME: this doesn't update voices that are already playing
            ui.add(egui::Slider::new(&mut self.synth.glide_time, 0.0..=0.5).text("Glide"));

            // play mode control
            egui::ComboBox::from_label("Play mode")
                .selected_text(self.synth.play_mode.name())
                .show_ui(ui, |ui| {
                    for variant in PlayMode::VARIANTS {
                        ui.selectable_value(&mut self.synth.play_mode, variant, variant.name());
                    }
                });

            // message area
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in self.messages.iter() {
                    ui.label(line);
                }
            });
        });
    }
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
    let mut seq = Sequencer::new(false, 1);
    seq.set_sample_rate(config.sample_rate.0 as f64);
    let mut backend = BlockRateAdapter::new(Box::new(seq.backend()));
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
            Ok(Box::new(App::new(synth, seq)))
        }),
    )
}