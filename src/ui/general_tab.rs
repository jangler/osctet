use fundsp::math::{amp_db, db_amp};
use info::Info;

use crate::{config::{self, Config}, fx::{FXSettings, GlobalFX, SpatialFx}, module::Module, pitch::Tuning};

use super::*;

/// Interval table cache.
pub struct TableCache {
    tuning: Tuning,
    table: Vec<Vec<String>>,
}

pub fn draw(ui: &mut UI, module: &mut Module, fx: &mut GlobalFX, cfg: &mut Config,
    player: &mut Player, scroll: &mut f32, table_cache: &mut Option<TableCache>,
) {
    ui.layout = Layout::Horizontal;
    let old_y = ui.cursor_y;
    ui.cursor_y -= *scroll;
    ui.cursor_z -= 1;
    ui.start_group();

    ui.header("METADATA", Info::None);
    if let Some(s) = ui.edit_box("Title", 40, module.title.clone(), Info::None) {
        module.title = s;
    }
    if let Some(s) = ui.edit_box("Author", 40, module.author.clone(), Info::None) {
        module.author = s;
    }
    fx_controls(ui, &mut module.fx, fx);
    tuning_controls(ui, &mut module.tuning, cfg, player, table_cache);

    let scroll_h = ui.end_group().unwrap().h + ui.style.margin;
    ui.cursor_z += 1;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(scroll, scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
}

fn fx_controls(ui: &mut UI, settings: &mut FXSettings, fx: &mut GlobalFX) {
    ui.space(2.0);
    ui.header("SPATIAL FX", Info::None);
    let mut commit = false;

    if let Some(i) = ui.combo_box("spatial_type", "Type", settings.spatial.variant_name(),
        Info::SpatialFxType,
        || SpatialFx::DEFAULT_VARIANTS.map(|v| v.variant_name().to_owned()).to_vec()) {
        settings.spatial = SpatialFx::DEFAULT_VARIANTS[i].clone();
        commit = true;
    }

    match &mut settings.spatial {
        SpatialFx::None => (),
        SpatialFx::Reverb { level, room_size, decay_time } => {
            if ui.slider("reverb_level", "Level", level,
                0.0..=1.0, None, 2, true, Info::None) {
                commit = true;
            }
            if ui.slider("room_size", "Room size", room_size,
                10.0..=30.0, Some("m"), 1, true, Info::None) {
                commit = true;
            }
            if ui.slider("decay_time", "Decay time", decay_time,
                0.0..=5.0, Some("s"), 2, true, Info::None) {
                commit = true;
            }
        },
        SpatialFx::Delay { level, time, feedback } => {
            if ui.slider("delay_level", "Level", level,
                0.01..=1.0, None, 2, true, Info::None) {
                commit = true;
            }
            if ui.slider("delay_time", "Time", time,
                0.01..=1.0, Some("s"), 2, true, Info::DelayTime) {
                commit = true;
            }
            if ui.slider("feedback", "Feedback", feedback,
                0.0..=1.0, None, 1, true, Info::DelayFeedback) {
                commit = true;
            }
        }
    }

    if commit {
        fx.commit_spatial(&settings.spatial);
    }

    ui.space(2.0);
    ui.header("COMPRESSION", Info::Compression);

    let comp = &mut settings.comp;
    let mut commit = false;

    if ui.formatted_slider("gain", "Gain", &mut comp.gain,
        0.0..=2.0, 2, true, Info::CompGain,
        |x| format!("{:+.1} dB", amp_db(x)), db_amp) {
        commit = true;
    }
    if ui.formatted_slider("threshold", "Threshold", &mut comp.threshold,
        0.0..=1.0, 1, true, Info::CompThreshold,
        |x| format!("{:.1} dB", amp_db(x)), db_amp) {
        commit = true;
    }
    if ui.formatted_slider("ratio", "Ratio", &mut comp.slope,
        0.0..=1.0, 1, true, Info::CompRatio, |x| format!("{:.1}:1", if x == 1.0 {
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
        0.0..=1.0, Some("s"), 2, true, Info::CompAttack) {
        commit = true;
    }
    if ui.slider("comp_release", "Release", &mut comp.release,
        0.0..=1.0, Some("s"), 2, true, Info::CompRelease) {
        commit = true;
    }

    if commit {
        fx.commit_comp(comp);
    }
}

fn tuning_controls(ui: &mut UI, tuning: &mut Tuning, cfg: &mut Config,
    player: &mut Player, table_cache: &mut Option<TableCache>
) {
    ui.space(2.0);
    ui.header("TUNING", Info::Tuning);
    if let Some(s) = ui.edit_box("Octave ratio", 8, tuning.equave().to_string(),
        Info::OctaveRatio
    ) {
        match s.parse() {
            Ok(ratio) => match Tuning::divide(ratio, tuning.size(), tuning.arrow_steps) {
                Ok(t) => {
                    *tuning = t;
                    *table_cache = None;
                }
                Err(e) => ui.report(e),
            }
            Err(e) => ui.report(e),
        }
    }
    if let Some(s) = ui.edit_box("Steps to octave", 3, tuning.scale.len().to_string(),
        Info::OctaveSteps
    ) {
        match s.parse() {
            Ok(steps) => match Tuning::divide(tuning.equave(), steps, tuning.arrow_steps) {
                Ok(t) => {
                    *tuning = t;
                    *table_cache = None;
                }
                Err(e) => ui.report(e),
            },
            Err(e) => ui.report(e),
        }
    }
    if let Some(s) = ui.edit_box("Steps to arrow", 3, tuning.arrow_steps.to_string(),
        Info::ArrowSteps
    ) {
        match s.parse() {
            Ok(steps) => {
                tuning.arrow_steps = steps;
                *table_cache = None;
            }
            Err(e) => ui.report(e),
        }
    }

    ui.start_group();
    if ui.button("Load scale", true, Info::LoadScale) {
        if let Some(path) = super::new_file_dialog(player)
            .add_filter("Scala scale file", &["scl"])
            .set_directory(cfg.scale_folder.clone().unwrap_or(String::from(".")))
            .pick_file() {
            cfg.scale_folder = config::dir_as_string(&path);
            match Tuning::load(path, tuning.root) {
                Ok(t) => {
                    *tuning = t;
                    *table_cache = None;
                }
                Err(e) => ui.report(e),
            }
        }
    }
    if ui.note_input("root", &mut tuning.root, Info::TuningRoot).is_some() {
        *table_cache = None;
    }
    ui.offset_label("Scale root", Info::TuningRoot);
    ui.end_group();

    ui.space(2.0);
    ui.start_group();
    if table_cache.as_ref().is_none_or(|tc| tc.tuning != *tuning) {
        *table_cache = Some(TableCache {
            tuning: tuning.clone(),
            table: make_table(tuning),
        });
    }
    if let Some(tc) = table_cache {
        draw_table(ui, &["Steps", "Notation", "Cents"], &tc.table);
    }
    ui.end_group();
}

fn make_table(t: &Tuning) -> Vec<Vec<String>> {
    let data = t.interval_table(&Note::new(0, crate::pitch::Nominal::C, 0, 4));
    let mut columns = Vec::new();

    columns.push((0..data.len()).map(|i| i.to_string()).collect());
    columns.push(data.iter().map(|(notation, _)| {
        notation.iter()
            .filter(|n| n.arrows.abs() <= 2 && n.sharps.abs() <= 2)
            .map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
    }).collect());
    columns.push(data.iter().map(|(_, cents)| format!("{:.1}", cents)).collect());

    columns
}

fn draw_table(ui: &mut UI, labels: &[&str], table: &Vec<Vec<String>>) {
    for (label, column) in labels.iter().zip(table) {
        ui.start_group();
        ui.label(label);
        for row in column {
            ui.label(row);
        }
        ui.end_group();
    }
}