use rfd::FileDialog;

use crate::synth::*;

use super::{UI, Layout};

// for file dialogs
const PATCH_FILTER_NAME: &str = "Instrument";
const PATCH_FILTER_EXT: &str = "oscins";

const NUM_COLUMN_WIDTH: f32 = 20.0;
const SLIDER_COLUMN_WIDTH: f32 = 120.0;
const X_COLUMN_WIDTH: f32 = 20.0;
const WAVE_COLUMN_WIDTH: f32 = 80.0;

const OSC_COLUMN_NAMES: [&str; 8] =
    ["#", "Level", "Tone", "Freq. ratio", "Finetune", "Waveform", "Output", ""];
const OSC_COLUMN_WIDTHS: [f32; 8] = [
    NUM_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH,
    SLIDER_COLUMN_WIDTH, WAVE_COLUMN_WIDTH, 110.0, X_COLUMN_WIDTH,
];

const ENV_COLUMN_NAMES: [&str; 7] =
    ["#", "Attack", "Decay", "Sustain", "Release", "Curve", ""];
const ENV_COLUMN_WIDTHS: [f32; 7] = [
    NUM_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH,
    SLIDER_COLUMN_WIDTH, 90.0, X_COLUMN_WIDTH,
];
const CURVES: [&str; 3] = ["Linear", "Quadratic", "Cubic"];

const FILTER_COLUMN_NAMES: [&str; 6] = ["#", "Type", "Cutoff", "Resonance", "Keytrack", ""];
const FILTER_COLUMN_WIDTHS: [f32; 6] = [
    NUM_COLUMN_WIDTH, 80.0, SLIDER_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH, 70.0,
    X_COLUMN_WIDTH,
];

const LFO_COLUMN_NAMES: [&str; 5] = ["#", "Waveform", "Rate", "Delay", ""];
const LFO_COLUMN_WIDTHS: [f32; 5] = [
    NUM_COLUMN_WIDTH, WAVE_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH, SLIDER_COLUMN_WIDTH,
    X_COLUMN_WIDTH,
];

const MOD_COLUMN_NAMES: [&str; 5] = ["#", "Source", "Target", "Depth", ""];
const MOD_COLUMN_WIDTHS: [f32; 5] = [
    NUM_COLUMN_WIDTH, 100.0, 100.0, SLIDER_COLUMN_WIDTH, X_COLUMN_WIDTH,
];

pub fn draw(ui: &mut UI, settings: &mut Patch) {
    ui.layout = Layout::Vertical;

    ui.label("GENERAL");
    ui.shared_slider("gain", "Gain", &settings.gain.0, 0.0..=1.0);
    ui.shared_slider("pan", "Pan", &settings.pan.0, -1.0..=1.0);
    ui.slider("glide_time", "Glide time", &mut settings.glide_time, 0.0..=0.5);
    if let Some(i) = ui.combo_box("play_mode",
        "Play mode", settings.play_mode.name(),
        || PlayMode::VARIANTS.map(|v| v.name().to_owned()).to_vec()
    ) {
        settings.play_mode = PlayMode::VARIANTS[i];
    }

    ui.space();
    oscillator_controls(ui, settings);
    ui.space();
    filter_controls(ui, settings);
    ui.space();
    envelope_controls(ui, settings);
    ui.space();
    lfo_controls(ui, settings);
    ui.space();
    modulation_controls(ui, settings);
    ui.space();
    file_ops(ui, settings);
}

fn oscillator_controls(ui: &mut UI, settings: &mut Patch) {
    let mut removed_osc = None;
    ui.label("OSCILLATORS");
    ui.start_grid(&OSC_COLUMN_WIDTHS, &OSC_COLUMN_NAMES);
    for (i, osc) in settings.oscs.iter_mut().enumerate() {
        ui.offset_label(&(i + 1).to_string());
        ui.next_cell();
        ui.shared_slider(&format!("osc_{}_level", i),
            "", &osc.level.0, 0.0..=1.0);
        ui.next_cell();
        ui.shared_slider(&format!("osc_{}_tone", i),
            "", &osc.tone.0, 0.0..=1.0);
        ui.next_cell();
        ui.shared_slider(&format!("osc_{}_ratio", i),
            "", &osc.freq_ratio.0, 0.5..=16.0);
        ui.next_cell();
        ui.shared_slider(&format!("osc_{}_tune", i),
            "", &osc.fine_pitch.0, -0.5..=0.5);
        ui.next_cell();
        if let Some(i) = ui.combo_box(&format!("osc_{}_wave", i),
            "", osc.waveform.name(),
            || Waveform::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
            osc.waveform = Waveform::VARIANTS[i];
        }
        ui.next_cell();
        let outputs = OscOutput::choices(i);
        if let Some(i) = ui.combo_box(&format!("osc_{}_output", i),
            "", &osc.output.to_string(),
            || outputs.iter().map(|x| x.to_string()).collect()) {
            osc.output = outputs[i];
        }
        ui.next_cell();
        if i > 0 && ui.button("X") {
            removed_osc = Some(i);
        }
        ui.next_cell();
    }
    if let Some(i) = removed_osc {
        settings.remove_osc(i);
    }
    ui.end_grid();
    if ui.button("+") {
        settings.oscs.push(Oscillator::new());
    }
}

fn filter_controls(ui: &mut UI, patch: &mut Patch) {
    ui.label("FILTERS");
    ui.start_grid(&FILTER_COLUMN_WIDTHS, &FILTER_COLUMN_NAMES);
    let mut removed_filter = None;
    for (i, filter) in patch.filters.iter_mut().enumerate() {
        ui.offset_label(&(i + 1).to_string());
        ui.next_cell();
        if let Some(i) = ui.combo_box(&format!("filter_{}_type", i),
            "", filter.filter_type.name(),
            || FilterType::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
            filter.filter_type = FilterType::VARIANTS[i];
        }
        ui.next_cell();
        ui.shared_slider(&format!("filter_{}_cutoff", i), "",
            &filter.cutoff.0, 20.0..=20_000.0);
        ui.next_cell();
        ui.shared_slider(&format!("filter_{}_q", i), "",
            &filter.resonance.0, 0.0..=1.0);
        ui.next_cell();
        if let Some(i) = ui.combo_box(&format!("filter_{}_keytrack", i),
            "", filter.key_tracking.name(),
            || KeyTracking::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
            filter.key_tracking = KeyTracking::VARIANTS[i];
        }
        ui.next_cell();
        if ui.button("X") {
            removed_filter = Some(i);
        }
        ui.next_cell();
    }
    if let Some(i) = removed_filter {
        patch.remove_filter(i);
    }
    ui.end_grid();
    if ui.button("+") {
        patch.filters.push(Filter::new());
    }
}

fn envelope_controls(ui: &mut UI, patch: &mut Patch) {
    ui.label("ENVELOPES");
    ui.start_grid(&ENV_COLUMN_WIDTHS, &ENV_COLUMN_NAMES);
    let mut removed_env = None;
    for (i, env) in patch.envs.iter_mut().enumerate() {
        ui.offset_label(&(i + 1).to_string());
        ui.next_cell();
        ui.slider(&format!("env_{}_A", i), "", &mut env.attack, 0.0..=10.0);
        ui.next_cell();
        ui.slider(&format!("env_{}_D", i), "", &mut env.decay, 0.01..=10.0);
        ui.next_cell();
        ui.slider(&format!("env_{}_S", i), "", &mut env.sustain, 0.0..=1.0);
        ui.next_cell();
        ui.slider(&format!("env_{}_R", i), "", &mut env.release, 0.01..=10.0);
        ui.next_cell();
        if let Some(i) = ui.combo_box(&format!("env_{}_curve", i), "",
            &env.curve_name(), || CURVES.map(|x| x.to_string()).to_vec()) {
            env.power = (i + 1) as f32;
        }
        ui.next_cell();
        if ui.button("X") {
            removed_env = Some(i);
        }
        ui.next_cell();
    }
    if let Some(i) = removed_env {
        patch.remove_env(i);
    }
    ui.end_grid();
    if ui.button("+") {
        patch.envs.push(ADSR::new());
    }
}

fn lfo_controls(ui: &mut UI, patch: &mut Patch) {
    ui.label("LFOS");
    ui.start_grid(&LFO_COLUMN_WIDTHS, &LFO_COLUMN_NAMES);
    let mut removed_lfo = None;
    for (i, lfo) in patch.lfos.iter_mut().enumerate() {
        ui.offset_label(&(i + 1).to_string());
        ui.next_cell();
        if let Some(i) = ui.combo_box(&format!("lfo_{}_wave", i),
            "", lfo.waveform.name(),
            || Waveform::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
            lfo.waveform = Waveform::VARIANTS[i];
        }
        ui.next_cell();
        ui.shared_slider(&format!("lfo_{}_rate", i), "", &lfo.freq.0, 0.1..=20.0);
        ui.next_cell();
        ui.slider(&format!("lfo_{}_delay", i), "", &mut lfo.delay, 0.0..=10.0);
        ui.next_cell();
        if ui.button("X") {
            removed_lfo = Some(i);
        }
        ui.next_cell();
    }
    if let Some(i) = removed_lfo {
        patch.remove_lfo(i);
    }
    ui.end_grid();
    if ui.button("+") {
        patch.lfos.push(LFO::new());
    }
}

fn modulation_controls(ui: &mut UI, patch: &mut Patch) {
    ui.label("MOD MATRIX");
    ui.start_grid(&MOD_COLUMN_WIDTHS, &MOD_COLUMN_NAMES);
    let mut removed_mod = None;
    let sources = patch.mod_sources();
    let targets = patch.mod_targets();
    for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
        ui.offset_label(&(i + 1).to_string());
        ui.next_cell();
        if let Some(i) = ui.combo_box(&format!("mod_{}_source", i),
            "", &m.source.to_string(),
            || sources.iter().map(|x| x.to_string()).collect()) {
            m.source = sources[i];
        }
        ui.next_cell();
        if let Some(i) = ui.combo_box(&format!("mod_{}_target", i),
            "", &m.target.to_string(),
            || targets.iter().map(|x| x.to_string()).collect()) {
            m.target = targets[i];
        }
        ui.next_cell();
        ui.shared_slider(&format!("mod_{}_depth", i), "", &m.depth.0, -1.0..=1.0);
        ui.next_cell();
        if ui.button("X") {
            removed_mod = Some(i);
        }
        ui.next_cell();
    }
    if let Some(i) = removed_mod {
        patch.remove_mod(i);
    }
    ui.end_grid();
    if ui.button("+") {
        patch.mod_matrix.push(Modulation::default());
    }
}

fn file_ops(ui: &mut UI, patch: &mut Patch) {
    ui.layout = Layout::Horizontal;
    if ui.button("Save patch") {
        if let Some(path) = FileDialog::new()
            .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
            .save_file() {
            match patch.save(&path) {
                Ok(_) => (),
                Err(e) => (), // TODO: alert
            }
        }
    }
    if ui.button("Load patch") {
        if let Some(path) = FileDialog::new()
            .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
            .pick_file() {
            match Patch::load(&path) {
                Ok(p) => {
                    *patch = p;
                },
                Err(e) => (), // TODO: alert
            }
        }
    }
}