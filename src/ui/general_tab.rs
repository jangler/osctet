use crate::{fx::GlobalFX, module::Module, pitch::Tuning};

use super::*;

pub fn draw(ui: &mut UI, module: &mut Module) {
    ui.layout = Layout::Vertical;
    fx_controls(ui, &mut module.fx);
    ui.space(2.0);
    tuning_controls(ui, &mut module.tuning);
}

fn fx_controls(ui: &mut UI, fx: &mut GlobalFX) {
    ui.shared_slider("gain",
        "Global volume", &fx.settings.gain.0, 0.0..=1.0, None);

    ui.label("REVERB");

    ui.shared_slider("reverb_level",
        "Level", &fx.settings.reverb_amount.0, 0.0..=1.0, None);

    if ui.slider("predelay",
        "Predelay time", &mut fx.settings.predelay_time, 0.0..=0.1, Some("s")) {
        fx.commit_predelay();
    }
    if ui.slider("room_size",
        "Room size", &mut fx.settings.reverb_room_size, 5.0..=100.0, Some("m")) {
        fx.commit_reverb();
    }
    if ui.slider("decay_time",
        "Decay time", &mut fx.settings.reverb_time, 0.0..=5.0, Some("s")) {
        fx.commit_reverb();
    }
}

fn tuning_controls(ui: &mut UI, tuning: &mut Tuning) {
    ui.label("TUNING");
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
            Ok(steps) => match Tuning::divide(tuning.equave(), tuning.size(), steps) {
                Ok(t) => *tuning = t,
                Err(e) => ui.report(e),
            },
            Err(e) => ui.report(e),
        }
    }
}