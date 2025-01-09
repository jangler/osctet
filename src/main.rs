// disable console in windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, error::Error, fs, panic};

use macroquad::{input::prevent_quit, miniquad::conf::Icon, prelude::Conf, texture::Image};

use osctet::{APP_NAME, run};

/// Filename to write panic messages to.
const PANIC_FILE: &str = "error.txt";

/// Returns initial WM settings.
fn window_conf() -> Conf {
    Conf {
        window_title: APP_NAME.to_owned(),
        window_width: 1280,
        window_height: 720,
        icon: Some(Icon {
            small: Image::from_file_with_format(
                include_bytes!("../icon/icon_16.png"), None)
                .unwrap().get_image_data().as_flattened().try_into().unwrap(),
            medium: Image::from_file_with_format(
                include_bytes!("../icon/icon_32.png"), None)
                .unwrap().get_image_data().as_flattened().try_into().unwrap(),
            big: Image::from_file_with_format(
                include_bytes!("../icon/icon_64.png"), None)
                .unwrap().get_image_data().as_flattened().try_into().unwrap(),
        }),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() -> Result<(), Box<dyn Error>> {
    // intercept quit so we can run actions before closing
    prevent_quit();

    // in release mode, write panics to a test file as well as stderr
    if cfg!(not(debug_assertions)) {
        panic::set_hook(Box::new(|info| {
            let message = if let Some(location) = info.location() {
                &format!("panic at {}:{}:{}\n",
                    location.file(), location.line(), location.column())
            } else {
                "panic at unknown location"
            };
            let _ = fs::write(PANIC_FILE, message);
            eprintln!("{}", message);
        }));
    }

    // pass the first arg, hopefully a module path
    run(env::args().nth(1)).await
}