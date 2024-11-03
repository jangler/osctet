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
    let mut osc = sine_hz(440.0);
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

    // draw
    loop {
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