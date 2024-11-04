// disable console in windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use midir::MidiInput;
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig};
use fundsp::hacker::*;
use eframe::egui;
use pitch::Tuning;

pub mod pitch;
pub mod input;

const APP_NAME: &str = "Synth Tracker";

struct MyApp {
    tuning: pitch::Tuning,
    text: Vec<String>,
    f: Shared,
    env_input: Shared,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let octave = 4;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MIDI input ports:");
            for line in self.text.iter() {
                ui.label(line);
            }
            ctx.input(|input| {
                for evt in input.events.iter() {
                    match evt {
                        egui::Event::Key { physical_key, pressed, .. } => {
                            if let Some(key) = physical_key {
                                if let Some(note) = input::note_from_key(key, &self.tuning, octave) {
                                    if *pressed {
                                        self.text.push(format!("{}", &note));
                                        self.f.set(midi_hz(self.tuning.midi_pitch(&note)));
                                        self.env_input.set(1.0);
                                    } else {
                                        self.env_input.set(0.0);
                                    }
                                }
                            }
                        },
                        _ => (),
                    }
                }
            })
        });
    }
}

fn main() -> eframe::Result {
    // init midi
    let midi_in = MidiInput::new(&format!("{} input", APP_NAME)).unwrap();
    let ports: Vec<String> = midi_in.ports().iter().map(|p| midi_in.port_name(p).unwrap()).collect();
    let port = &midi_in.ports()[0];

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

    // handle MIDI input
    let tuning = Tuning::divide(2.0, 5, 1).unwrap();
    let _conn_in;
    if ports.len() > 0 {
        let env_input = env_input.clone();
        let f = f.clone();
        let tuning = tuning.clone();
        _conn_in = midi_in.connect(
            port,
            APP_NAME,
            move |_, message, _| {
                match message[0] & 0xf0 {
                    0b10000000 => {
                        // note off
                        env_input.set(0.0);
                    },
                    0b10010000 => {
                        // note on
                        let note = input::note_from_midi(message[1] as i8, &tuning);
                        f.set(midi_hz(tuning.midi_pitch(&note)));
                        env_input.set(1.0);
                    },
                    _ => (),
                }
            },
            (),
        ).unwrap();
        println!("listening on {}", ports[0]);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|_cc| {
            // This gives us image support:
            Ok(Box::new(MyApp {
                tuning,
                text: ports,
                f,
                env_input,
            }))
        }),
    )
}