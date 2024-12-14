use rfd::FileDialog;

use crate::{config::{self, Config}, fx::{FXSettings, GlobalFX}, module::Module, pitch::Tuning};

use super::*;

pub fn draw(ui: &mut UI, module: &mut Module, fx: &mut GlobalFX, cfg: &mut Config) {
    ui.layout = Layout::Vertical;
    ui.header("METADATA");
    if let Some(s) = ui.edit_box("Title", 20, module.title.clone()) {
        module.title = s;
    }
    if let Some(s) = ui.edit_box("Author", 20, module.author.clone()) {
        module.author = s;
    }
    fx_controls(ui, &mut module.fx, fx);
    tuning_controls(ui, &mut module.tuning, cfg);
}

fn fx_controls(ui: &mut UI, settings: &mut FXSettings, fx: &mut GlobalFX) {
    ui.space(2.0);
    ui.header("FX");
    ui.shared_slider("gain",
        "Global volume", &settings.gain.0, 0.0..=2.0, None, 2);
    ui.shared_slider("reverb_level",
        "Reverb level", &settings.reverb_amount.0, 0.0..=1.0, None, 2);

    if ui.slider("predelay",
        "Predelay time", &mut settings.predelay_time, 0.0..=0.1, Some("s"), 2) {
        fx.commit_predelay(settings);
    }
    if ui.slider("room_size",
        "Room size", &mut settings.reverb_room_size, 5.0..=100.0, Some("m"), 2) {
        fx.commit_reverb(settings);
    }
    if ui.slider("decay_time",
        "Decay time", &mut settings.reverb_time, 0.0..=5.0, Some("s"), 2) {
        fx.commit_reverb(settings);
    }
}

fn tuning_controls(ui: &mut UI, tuning: &mut Tuning, cfg: &mut Config) {
    ui.space(2.0);
    ui.header("TUNING");
    if let Some(s) = ui.edit_box("Equave", 8, tuning.equave().to_string()) {
        match s.parse() {
            Ok(ratio) => match Tuning::divide(ratio, tuning.size(), tuning.arrow_steps) {
                Ok(t) => *tuning = t,
                Err(e) => ui.report(e),
            }
            Err(e) => ui.report(e),
        }
    }
    if let Some(s) = ui.edit_box("Steps to equave", 4, tuning.scale.len().to_string()) {
        match s.parse() {
            Ok(steps) => match Tuning::divide(tuning.equave(), steps, tuning.arrow_steps) {
                Ok(t) => *tuning = t,
                Err(e) => ui.report(e),
            },
            Err(e) => ui.report(e),
        }
    }
    if let Some(s) = ui.edit_box("Steps to arrow", 4, tuning.arrow_steps.to_string()) {
        match s.parse() {
            Ok(steps) => tuning.arrow_steps = steps,
            Err(e) => ui.report(e),
        }
    }
    ui.layout = Layout::Horizontal;
    if ui.button("Load scale") {
        if let Some(path) = FileDialog::new()
            .add_filter("Scala scale file", &["scl"])
            .set_directory(cfg.scale_folder.clone().unwrap_or(String::from(".")))
            .pick_file() {
            cfg.scale_folder = config::dir_as_string(&path);
            let _ = cfg.save();
            match Tuning::load(path, tuning.root) {
                Ok(t) => *tuning = t,
                Err(e) => ui.report(e),
            }
        }
    }
    ui.note_input("root", &mut tuning.root);
    ui.offset_label("Scale root");
}