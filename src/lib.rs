//! Microtonal tracker with built-in subtractive/FM synth.

use std::error::Error;
use std::sync::mpsc::{channel, Sender, Receiver};

use config::Config;
use fx::GlobalFX;
use midir::{InitError, MidiInput, MidiInputConnection, MidiInputPort};
use fundsp::hacker::*;
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, Stream, StreamConfig};
use module::Module;
use synth::{Key, KeyOrigin, Synth};
use macroquad::prelude::*;

pub mod pitch;
mod input;
mod config;
pub mod synth;
mod pattern;
mod adsr;
mod fx;
pub mod ui;
mod module;

use input::MidiEvent;

/// Application name, for window title, etc.
pub const APP_NAME: &str = "Osctet";

const TABS: [&str; 3] = ["General", "Pattern", "Instruments"];

fn init_audio(mut backend: BlockRateAdapter) -> Result<Stream, Box<dyn Error>> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("could not open audio output device")?;

    let config: StreamConfig = device.supported_output_configs()?
        .next()
        .ok_or("could not find audio output config")?
        .with_max_sample_rate()
        .into();

    backend.set_sample_rate(config.sample_rate.0 as f64);

    let stream = device.build_output_stream(
        &config,move |data: &mut[f32], _: &cpal::OutputCallbackInfo| {
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
    )?;
    
    stream.play()?;
    Ok(stream)
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

struct App {
    tuning: pitch::Tuning,
    synth: Synth,
    seq: Sequencer,
    octave: i8,
    midi: Midi,
    config: Config,
    module: Module,
    ui: ui::UI,
    fullscreen: bool,
}

impl App {
    fn new(seq: Sequencer, global_fx: GlobalFX) -> Self {
        let mut err: Option<Box<dyn Error>> = None;
        let config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                err = Some(e);
                Config::default()
            },
        };
        let mut midi = Midi::new();
        midi.port_selection = config.default_midi_input.clone();
        let mut app = App {
            tuning: pitch::Tuning::divide(2.0, 12, 1).unwrap(),
            synth: Synth::new(),
            seq,
            octave: 4,
            midi,
            config,
            module: Module::new(global_fx),
            ui: ui::UI::new(),
            fullscreen: false,
        };
        if let Some(err) = err {
            app.ui.report(err);
        }
        app
    }

    fn handle_keys(&mut self) {
        let (pressed, released) = (get_keys_pressed(), get_keys_released());

        for key in released {
            if let Some(_) = input::note_from_key(key, &self.tuning, self.octave) {
                self.synth.note_off(Key {
                    origin: KeyOrigin::Keyboard,
                    channel: 0,
                    key: input::u8_from_key(key),
                }, &mut self.seq);
            }
        }

        for key in pressed {
            if let Some(note) = input::note_from_key(key, &self.tuning, self.octave) {
                self.synth.note_on(Key {
                    origin: KeyOrigin::Keyboard,
                    channel: 0,
                    key: input::u8_from_key(key),
                }, self.tuning.midi_pitch(&note), 100.0 / 127.0, &mut self.seq);
            } else {
                match key {
                    KeyCode::F1 => self.ui.set_tab("main", 0),
                    KeyCode::F2 => self.ui.set_tab("main", 1),
                    KeyCode::F3 => self.ui.set_tab("main", 2),
                    KeyCode::F11 => {
                        self.fullscreen = !self.fullscreen;
                        set_fullscreen(self.fullscreen);
                    }
                    _ => (),
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
                                // ignore the error here, it probably just means that the user
                                // changed ports
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

    fn handle_midi(&mut self) {
        if let Some(rx) = &self.midi.rx {
            while let Ok(v) = rx.try_recv() {
                match MidiEvent::parse(&v) {
                    Some(MidiEvent::NoteOff { channel, key, .. }) => {
                        self.synth.note_off(Key{
                            origin: KeyOrigin::Midi,
                            channel,
                            key,
                        }, &mut self.seq);
                    },
                    Some(MidiEvent::NoteOn { channel, key, velocity }) => {
                        if velocity != 0 {
                            let note = input::note_from_midi(v[1] as i8, &self.tuning);
                            self.synth.note_on(Key {
                                origin: KeyOrigin::Midi,
                                channel,
                                key,
                            }, self.tuning.midi_pitch(&note), velocity as f32 / 127.0, &mut self.seq);
                        } else {
                            self.synth.note_off(Key {
                                origin: KeyOrigin::Midi,
                                channel,
                                key,
                            }, &mut self.seq);
                        }
                    },
                    Some(MidiEvent::PolyPressure { channel, key, pressure }) => {
                        if self.config.midi_send_pressure == Some(true) {
                            self.synth.poly_pressure(Key {
                                origin: KeyOrigin::Midi,
                                channel,
                                key,
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
                    if let Err(e) = self.config.save() {
                        self.ui.report(e);
                    };
                },
                Err(e) => {
                    self.midi.port_selection = None;
                    self.ui.report(e);
                },
            }
        }
    }

    fn frame(&mut self) {
        self.handle_keys();
        self.handle_midi();
        self.check_midi_reconnect();
        self.process_ui();
    }
    
    fn process_ui(&mut self) {
        self.ui.start_frame();

        self.bottom_panel();

        match self.ui.tab_menu("main", &TABS) {
            0 => ui::general_tab::draw(&mut self.ui, &mut self.module.fx),
            1 => ui::pattern_tab::draw(&mut self.ui, &mut self.module.pattern),
            2 => ui::instruments_tab::draw(&mut self.ui, &mut self.synth.settings),
            _ => panic!("bad tab value"),
        }

        self.ui.end_frame();
    }
    
    fn bottom_panel(&mut self) {
        self.ui.start_bottom_panel();

        if self.midi.input.is_some() {
            let s = if let Some(name) = &self.midi.port_name {
                &name
            } else {
                "(none)"
            };
            if let Some(i) = self.ui.combo_box("midi_input", "MIDI input", s,
                || input_names(self.midi.input.as_ref().unwrap())) {
                self.midi.port_selection = input_names(self.midi.input.as_ref().unwrap()).get(i).cloned();
            }

            let mut v = self.config.midi_send_pressure.unwrap_or(true);
            if self.ui.checkbox("Use aftertouch", &mut v) {
                self.config.midi_send_pressure = Some(v);
                if let Err(e) = self.config.save() {
                    self.ui.report(e);
                }
            }
        } else {
            self.ui.label("No MIDI device");
        }
        
        self.ui.end_bottom_panel();
    }
}

fn input_names(input: &MidiInput) -> Vec<String> {
    input.ports().into_iter()
        .map(|p| input.port_name(&p).unwrap_or(String::from("(unknown)")))
        .collect()
}

/// Application entry point.
pub async fn run() -> Result<(), Box<dyn Error>> {
    let mut seq = Sequencer::new(false, 2);

    let mut global_fx = GlobalFX::new(seq.backend());
    let backend = BlockRateAdapter::new(Box::new(global_fx.net.backend()));

    // grab the stream to keep it alive
    let _stream = init_audio(backend)?;

    let mut app = App::new(seq, global_fx);

    loop {
        app.frame();
        next_frame().await
    }
}