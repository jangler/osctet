use fundsp::math::{amp_db, db_amp};

use crate::{config::{self, Config}, fx::{FXSettings, GlobalFX, SpatialFx}, module::Module, pitch::Tuning};

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
    ui.header("SPATIAL FX");
    let mut commit = false;

    if let Some(i) = ui.combo_box("spatial_type", "Type", settings.spatial.variant_name(),
        || SpatialFx::DEFAULT_VARIANTS.map(|v| v.variant_name().to_owned()).to_vec()) {
        settings.spatial = SpatialFx::DEFAULT_VARIANTS[i].clone();
        commit = true;
    }

    match &mut settings.spatial {
        SpatialFx::None => (),
        SpatialFx::Reverb { level, predelay, room_size, decay_time } => {
            if ui.slider("reverb_level", "Level", level, 0.0..=1.0, None, 2, true) {
                commit = true;
            }
            if ui.slider("predelay", "Predelay", predelay, 0.0..=0.1, Some("s"), 2, true) {
                commit = true;
            }
            if ui.slider("room_size", "Room size", room_size,
                10.0..=30.0, Some("m"), 2, true) {
                commit = true;
            }
            if ui.slider("decay_time", "Decay time", decay_time,
                0.0..=5.0, Some("s"), 2, true) {
                commit = true;
            }
        },
        SpatialFx::Delay { level, time, feedback } => {
            if ui.slider("delay_level", "Level", level, 0.01..=1.0, Some("s"), 2, true) {
                commit = true;
            }
            if ui.slider("delay_time", "Time", time, 0.01..=1.0, Some("s"), 2, true) {
                commit = true;
            }
            if ui.slider("feedback", "Feedback", feedback, 0.0..=1.0, None, 1, true) {
                commit = true;
            }
        }
    }

    if commit {
        fx.commit_spatial(&settings.spatial);
    }

    ui.space(2.0);
    ui.header("COMPRESSION");

    let comp = &mut settings.comp;
    let mut commit = false;

    if ui.formatted_slider("gain", "Gain", &mut comp.gain,
        0.0..=2.0, 2, true, |x| format!("{:+.1} dB", amp_db(x)), db_amp) {
        commit = true;
    }
    if ui.formatted_slider("threshold", "Threshold", &mut comp.threshold,
        0.0..=1.0, 1, true, |x| format!("{:.1} dB", amp_db(x)), db_amp) {
        commit = true;
    }
    if ui.formatted_slider("ratio", "Ratio", &mut comp.slope,
        0.0..=1.0, 1, true, |x| format!("{:.1}:1", if x == 1.0 {
            f32::INFINITY
        } else {
            1.0 / (1.0 - x)
        }), |f| if f == f32::INFINITY {
            1.0
        } else {
            (f - 1.0) / f
        }) {
        commit = true;
    }
    if ui.slider("comp_attack", "Attack", &mut comp.attack,
        0.0..=1.0, Some("s"), 2, true) {
        commit = true;
    }
    if ui.slider("comp_release", "Release", &mut comp.release,
        0.0..=1.0, Some("s"), 2, true) {
        commit = true;
    }

    if commit {
        fx.commit_comp(comp);
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
    if ui.button("Load scale", true) {
        if let Some(path) = super::new_file_dialog()
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