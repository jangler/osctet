use fundsp::math::{amp_db, db_amp};
use info::Info;

use crate::{config::{self, Config}, fx::{Compression, GlobalFX, SpatialFx}, module::Module, pitch::Tuning};

use super::*;

/// State for the general tab UI.
#[derive(Default)]
pub struct GeneralState {
    scroll: f32,
    table_cache: Option<TableCache>,
}

/// Interval table cache.
struct TableCache {
    tuning: Tuning,
    table: Vec<Vec<String>>,
}

/// Return values are (fx_changed, tuning_changed).
pub fn draw(ui: &mut Ui, module: &mut Module, fx: &mut GlobalFX, cfg: &mut Config,
    player: &mut PlayerShell, state: &mut GeneralState,
) -> (bool, bool) {
    ui.layout = Layout::Horizontal;
    let old_y = ui.cursor_y;
    ui.cursor_y -= state.scroll;
    ui.cursor_z -= 1;
    ui.start_group();

    metadata_controls(ui, module);
    ui.vertical_space();
    let mut fx_changed = spatial_fx_controls(ui, &mut module.fx.spatial, fx);
    ui.vertical_space();
    fx_changed |= compression_controls(ui, &mut module.fx.comp, fx);
    ui.vertical_space();
    let tuning_changed =
        tuning_controls(ui, &mut module.tuning, cfg, player, &mut state.table_cache);
    ui.vertical_space();
    interval_table(ui, &mut module.tuning, &mut state.table_cache);

    let scroll_h = ui.end_group().unwrap().h + ui.style.margin;
    ui.cursor_z += 1;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(&mut state.scroll,
        scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
    
    (fx_changed, tuning_changed)
}

fn metadata_controls(ui: &mut Ui, module: &mut Module) {
    ui.header("METADATA", Info::None);
    if let Some(s) = ui.edit_box("Title", 40, module.title.clone(), Info::None) {
        module.title = s;
    }
    if let Some(s) = ui.edit_box("Author", 40, module.author.clone(), Info::None) {
        module.author = s;
    }
}

/// Returns true if changes were made.
fn spatial_fx_controls(ui: &mut Ui, spatial: &mut SpatialFx, fx: &mut GlobalFX) -> bool {
    ui.header("SPATIAL FX", Info::None);

    let mut commit = false;

    if let Some(i) = ui.combo_box("spatial_type", "Type", spatial.variant_name(),
        Info::SpatialFxType,
        || SpatialFx::DEFAULT_VARIANTS.map(|v| v.variant_name().to_owned()).to_vec()) {
        *spatial = SpatialFx::DEFAULT_VARIANTS[i].clone();
        commit = true;
    }

    match spatial {
        SpatialFx::None => (),
        SpatialFx::Reverb { level, room_size, decay_time } => {
            if ui.slider("reverb_level", "Level", level,
                0.0..=1.0, None, 2, true, Info::None) {
                commit = true;
            }
            if ui.formatted_slider("room_size", "Room size", room_size,
                10.0..=30.0, 1, true, Info::None, |f| format!("{f:.1} m"), |f| f) {
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
                0.0..=1.0, None, 2, true, Info::DelayFeedback) {
                commit = true;
            }
        }
    }

    if commit {
        fx.commit_spatial(&spatial);
    }
    commit
}

/// Returns true if changes were made.
fn compression_controls(ui: &mut Ui, comp: &mut Compression, fx: &mut GlobalFX) -> bool {
    ui.header("COMPRESSION", Info::Compression);

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
    commit
}

/// Returns true if changes were made.
fn tuning_controls(ui: &mut Ui, tuning: &mut Tuning, cfg: &mut Config,
    player: &mut PlayerShell, table_cache: &mut Option<TableCache>
) -> bool {
    const OCTAVE_CHARS: usize = 7;

    ui.header("TUNING", Info::Tuning);

    if let Some(s) = ui.edit_box("Octave ratio", OCTAVE_CHARS,
        tuning.equave().to_string().chars().take(OCTAVE_CHARS).collect(), Info::OctaveRatio
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

    // unequal scale controls
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
                Err(e) => ui.report(format!("Error loading scale: {e}")),
            }
        }
    }
    if ui.note_input("root", &mut tuning.root, Info::TuningRoot).is_some() {
        *table_cache = None;
    }
    ui.offset_label("Scale root", Info::TuningRoot);
    ui.end_group();

    table_cache.is_none()
}

fn interval_table(ui: &mut Ui, tuning: &mut Tuning, table_cache: &mut Option<TableCache>) {
    ui.header("INVERVAL TABLE", Info::None);
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

/// Construct an interval table (as column-major strings) from a tuning.
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

/// Draw a table of strings, stored in column-major order.
fn draw_table(ui: &mut Ui, labels: &[&str], table: &Vec<Vec<String>>) {
    for (label, column) in labels.iter().zip(table) {
        ui.start_group();
        ui.label(label, Info::None);
        for row in column {
            ui.label(row, Info::None);
        }
        ui.end_group();
    }
}