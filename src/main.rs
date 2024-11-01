#![windows_subsystem = "windows"]

use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig};
use macroquad::prelude::*;

#[macroquad::main("Synth Tracker")]
async fn main() {
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
    let sample_rate = config.sample_rate.0 as f32;
    let mut phase = 0;
    let stream = device.build_output_stream(
        &config,
        move |data: &mut[f32], _: &cpal::OutputCallbackInfo| {
            for sample in data.iter_mut() {
                *sample = (((phase as f32) / sample_rate * 440.0) % 2.0 - 1.0) * 0.1;
                phase += 1;
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

        draw_text("IT WORKS!", 20.0, 20.0, 30.0, DARKGRAY);

        next_frame().await
    }
}