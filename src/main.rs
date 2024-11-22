// disable console in windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use macroquad::prelude::Conf;

use osctet::{APP_NAME, run};

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
    run().await
}