// disable console in windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{backtrace::Backtrace, env, error::Error, fs, panic};

use macroquad::{input::prevent_quit, miniquad::conf::Icon, prelude::Conf, texture::Image};

use osctet::{exe_relative_path, run, APP_NAME};

/// Filename to write panic messages to.
const PANIC_FILE: &str = "error.txt";

/// Returns initial WM settings.
fn window_conf() -> Conf {
    Conf {
        window_title: APP_NAME.to_owned(),
        window_width: 1280,
        window_height: 720,
        icon: Some(Icon {
            small: decode_icon(include_bytes!("../icon/icon_16.png"))
                .try_into().unwrap(),
            medium: decode_icon(include_bytes!("../icon/icon_32.png"))
                .try_into().unwrap(),
            big: decode_icon(include_bytes!("../icon/icon_64.png"))
                .try_into().unwrap(),
        }),
        ..Default::default()
    }
}

/// Decode icon image data into raw bitmap bytes.
fn decode_icon(bytes: &[u8]) -> Vec<u8> {
    Image::from_file_with_format(bytes, None)
        .unwrap().get_image_data().as_flattened().into()
}

#[macroquad::main(window_conf)]
async fn main() -> Result<(), Box<dyn Error>> {
    // intercept quit so we can run actions before closing
    prevent_quit();

    // in release mode, write panics to a test file as well as stderr.
    // since the error is typically discovered as a poisoned mutex, the actual
    // panic info isn't very helpful, but we can print a backtrace.
    if cfg!(not(debug_assertions)) {
        panic::set_hook(Box::new(|_| {
            let backtrace = Backtrace::force_capture();
            let message = format!("Backtrace:\n\n{}", backtrace);
            let _ = fs::write(exe_relative_path(PANIC_FILE), &message);

            // no printing works for me here (at least on windows)
            // but it can't hurt to try. it might be because of the
            // windows_subsystem thing
            eprintln!("panic; backtrace written to {PANIC_FILE}");
        }));
    }

    // pass the first arg, hopefully a module path
    run(env::args().nth(1)).await
}