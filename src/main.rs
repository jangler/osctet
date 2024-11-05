// disable console in windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::mpsc::{channel, Sender, Receiver};
use std::error::Error;

use midir::{MidiInput, MidiInputConnection, MidiInputPort};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig};
use fundsp::hacker::*;
use eframe::egui;

pub mod pitch;
pub mod input;

const APP_NAME: &str = "Synth Tracker";

struct App {
    tuning: pitch::Tuning,
    messages: Vec<String>,
    f: Shared,
    gate: Shared,
    octave: i8,
    midi_in: Option<MidiInput>,
    midi_port: Option<MidiInputPort>,
    midi_conn: Option<MidiInputConnection<Sender<Vec<u8>>>>,
    midi_rx: Option<Receiver<Vec<u8>>>,
    midi_input_id: u16,
}

impl App {
    fn new(f: Shared, gate: Shared) -> Self {
        let mut app = App {
            tuning: pitch::Tuning::divide(2.0, 12, 1).unwrap(),
            messages: vec![],
            f,
            gate,
            octave: 4,
            midi_in: None,
            midi_port: None,
            midi_conn: None,
            midi_rx: None,
            midi_input_id: 0,
        };
        app.open_midi_input();
        app
    }

    fn open_midi_input(&mut self) {
        self.midi_input_id += 1;
        match MidiInput::new(&format!("{} input #{}", APP_NAME, self.midi_input_id)) {
            Ok(input) => self.midi_in = Some(input),
            Err(e) => self.report(&e),
        }
    }

    fn report(&mut self, e: &impl std::fmt::Display) {
        self.messages.push(format!("{}", e))
    }

    fn handle_ui_event(&mut self, evt: &egui::Event) {
        match evt {
            egui::Event::Key { physical_key, pressed, .. } => {
                if let Some(key) = physical_key {
                    if let Some(note) = input::note_from_key(key, &self.tuning, self.octave) {
                        if *pressed {
                            self.messages.push(format!("{}", &note));
                            self.f.set(midi_hz(self.tuning.midi_pitch(&note)));
                            self.gate.set(1.0);
                        } else {
                            self.gate.set(0.0);
                        }
                    }
                }
            },

            _ => (),
        }
    }

    fn midi_connect(&mut self, ctx: egui::Context) -> Result<MidiInputConnection<Sender<Vec<u8>>>, Box<dyn Error>> {
        self.midi_input_id += 1;
        let midi_in = MidiInput::new(&format!("{} input #{}", APP_NAME, self.midi_input_id))?;

        let (tx, rx) = channel();
        self.midi_rx = Some(rx);
        Ok(midi_in.connect(
            &self.midi_port.as_ref().unwrap(),
            APP_NAME,
            move |_, message, tx| {
                tx.send(message.to_vec());
                ctx.request_repaint();
            },
            tx,
        )?)
    }

    fn handle_midi(&self, message: &[u8]) {
        match message[0] & 0xf0 {
            0b10000000 => {
                // note off
                self.gate.set(0.0);
            },
            0b10010000 => {
                // note on
                let note = input::note_from_midi(message[1] as i8, &self.tuning);
                self.f.set(midi_hz(self.tuning.midi_pitch(&note)));
                self.gate.set(1.0);
            },
            _ => (),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.midi_conn.is_none() {
                if let Some(midi_in) = &self.midi_in {
                    egui::ComboBox::from_label("Select MIDI input port")
                        .selected_text(match &self.midi_port {
                            Some(p) => midi_in.port_name(&p).unwrap_or(String::from("")),
                            None => String::from(""),
                        })
                        .show_ui(ui, |ui| {
                            for p in midi_in.ports() {
                                let name = midi_in.port_name(&p).unwrap_or(String::from(""));
                                ui.selectable_value(&mut self.midi_port, Some(p), name);
                            }
                        });
                    if self.midi_port.is_some() {
                        match self.midi_connect(ctx.clone()) {
                            Ok(conn) => {
                                self.midi_conn = Some(conn);
                                self.report(&"Connected to MIDI input port");
                            },
                            Err(e) => self.report(&e),
                        }
                    }
                }
            }
            for line in self.messages.iter() {
                ui.label(line);
            }
            ctx.input(|input| {
                for evt in input.events.iter() {
                    self.handle_ui_event(evt);
                }
            });
            if let Some(rx) = &self.midi_rx {
                loop {
                    match rx.try_recv() {
                        Ok(v) => {
                            self.handle_midi(&v);
                            self.messages.push(format!("Received MIDI message: {:?}", &v));
                        },
                        Err(_) => break,
                    }
                }
            }
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
    let f = shared(440.0);
    let env_input = shared(0.0);
    let mut osc = (var(&f) >> follow(0.01) >> saw() * 0.2) >>
        moog_hz(1_000.0, 0.0) * (var(&env_input) >> adsr_live(0.1, 0.5, 0.5, 0.5));
    osc.set_sample_rate(config.sample_rate.0 as f64);
    let stream = device.build_output_stream(
        &config,
        move |data: &mut[f32], _: &cpal::OutputCallbackInfo| {
            // there's probably a better way to do this
            let mut i = 0;
            let len = data.len();
            while i < len {
                let (l, r) = osc.get_stereo();
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
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|_cc| {
            // This gives us image support:
            Ok(Box::new(App::new(f, env_input)))
        }),
    )
}