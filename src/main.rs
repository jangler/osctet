// disable console in windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{error::Error, fs, panic};

use macroquad::prelude::Conf;

use osctet::{APP_NAME, run};

const PANIC_FILE: &str = "error.txt";

fn window_conf() -> Conf {
    Conf {
        window_title: APP_NAME.to_owned(),
        window_width: 1280,
        window_height: 720,
        // TODO: icon
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() -> Result<(), Box<dyn Error>> {
    // panic::set_hook(Box::new(|info| {
    //     let message = if let Some(location) = info.location() {
    //         &format!("panic at {}:{}:{}\n",
    //             location.file(), location.line(), location.column())
    //     } else {
    //         "panic at unknown location"
    //     };
    //     let _ = fs::write(PANIC_FILE, message);
    //     eprint!("{}", message);
    // }));
    run().await
}