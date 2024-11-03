// disable console in windows release builds
#![cfg_attr(
    all(
        target_os = "windows",
        not(debug_assertions),
    ),
    windows_subsystem = "windows"
)]

use midir::{MidiInput};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig};
use fundsp::hacker::*;
use macroquad::prelude::*;

#[macroquad::main("Synth Tracker")]
async fn main() {
    // init midi
    let midi_in = MidiInput::new("midir test input").unwrap();
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
    let _conn_in;
    if ports.len() > 0 {
        let env_input = env_input.clone();
        let f = f.clone();
        _conn_in = midi_in.connect(
            port,
            "synth tracker",
            move |_, message, _| {
                match message[0] & 0xf0 {
                    0b10000000 => {
                        // note off
                        env_input.set(0.0);
                    },
                    0b10010000 => {
                        // note on
                        f.set(midi_hz(message[1] as f32));
                        env_input.set(1.0);
                    },
                    _ => (),
                }
            },
            (),
        ).unwrap();
        println!("listening on {}", ports[0]);
    }

    // main loop
    loop {
        // handle input
        for key in get_keys_pressed().iter() {
            match key {
                KeyCode::Key0 => f.set(27.5),
                KeyCode::Key1 => f.set(55.0),
                KeyCode::Key2 => f.set(110.0),
                KeyCode::Key3 => f.set(220.0),
                KeyCode::Key4 => f.set(440.0),
                KeyCode::Space => env_input.set(1.0),
                _ => (),
            }
        }
        for key in get_keys_released().iter() {
            match key {
                KeyCode::Space => env_input.set(0.0),
                _ => (),
            }
        }

        // draw
        clear_background(RED);

        draw_line(40.0, 40.0, 100.0, 200.0, 15.0, BLUE);
        draw_rectangle(screen_width() / 2.0 - 60.0, 100.0, 120.0, 60.0, GREEN);
        draw_circle(screen_width() - 30.0, screen_height() - 30.0, 15.0, YELLOW);

        for (i, port) in ports.iter().enumerate() {
            draw_text(port, 20.0, 20.0 * (i + 1) as f32, 30.0, DARKGRAY);
        }

        next_frame().await
    }
}