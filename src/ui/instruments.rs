use lfo::{AR_RATE_MULTIPLIER, LFO, MAX_LFO_RATE, MIN_LFO_RATE};
use macroquad::input::{KeyCode, is_key_pressed};
use pcm::PcmData;

use crate::{config::{self, Config}, module::{Edit, Module, ModuleCommand, ModuleSync}, playback::PlayerShell, synth::*};

use super::{info::Info, Layout, Ui};

// for file dialogs
const PATCH_FILTER_NAME: &str = "Instrument";
const PATCH_FILTER_EXT: &str = "oscins";

/// State for the instruments tab UI.
pub struct InstrumentsState {
    scroll: f32,
    /// If None, kit is selected.
    pub patch_index: Option<usize>,
}

impl InstrumentsState {
    pub fn new(patch_index: Option<usize>) -> Self {
        Self {
            scroll: 0.0,
            patch_index,
        }
    }
}

pub fn draw(ui: &mut Ui, module: &mut Module, state: &mut InstrumentsState,
    cfg: &mut Config, player: &mut PlayerShell, module_sync: &mut ModuleSync,
) {
    if is_key_pressed(KeyCode::Up) {
        shift_patch_index(-1, &mut state.patch_index, module.patches.len());
    } else if is_key_pressed(KeyCode::Down) {
        shift_patch_index(1, &mut state.patch_index, module.patches.len());
    }

    ui.layout = Layout::Horizontal;
    let old_y = ui.cursor_y;
    ui.cursor_y -= state.scroll;
    ui.cursor_z -= 1;

    patch_list(ui, module, &mut state.patch_index, cfg, player);
    ui.space(1.0);
    ui.start_group();
    if let Some(index) = &state.patch_index {
        if let Some(patch) = module.patches.get_mut(*index) {
            if patch_controls(ui, patch, cfg, player) {
                module_sync.push(ModuleCommand::Patch(*index, patch.shared_clone()));
            }
        }
    } else {
        if kit_controls(ui, module, player) {
            module_sync.push(ModuleCommand::Kit(module.kit.clone()))
        }
    }

    ui.cursor_z += 1;
    ui.cursor_y += state.scroll;
    let scroll_h = ui.end_group().unwrap().h + ui.style.margin;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(&mut state.scroll,
        scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
}

fn patch_list(ui: &mut Ui, module: &mut Module, patch_index: &mut Option<usize>,
    cfg: &mut Config, player: &mut PlayerShell
) {
    ui.start_group();

    let mut edits = Vec::new();
    let patches = &mut module.patches;

    let mut names = vec![String::from("Kit")];
    names.extend(patches.iter().map(|x| x.name.clone()));

    let mut list_index = patch_index.map(|i| i + 1).unwrap_or_default();
    if let Some(s) = ui.instrument_list(&names, &mut list_index, 10) {
        if list_index > 0 {
            if let Some(patch) = patches.get_mut(list_index - 1) {
                patch.name = s;
            }
        }
    }
    *patch_index = match list_index {
        0 => None,
        i => Some(i - 1),
    };

    ui.start_group();
    if ui.button("Add", true, Info::Add("a new patch with default settings")) {
        let mut name = String::from("Init");
        let mut i = 0;
        while names.contains(&name) {
            i += 1;
            name = format!("Init {}", i);
        }
        edits.push(Edit::InsertPatch(patches.len(), Patch::new(name)));
        *patch_index = Some(patches.len());
    }

    if ui.button("Remove", patch_index.is_some(), Info::Remove("the selected patch")) {
        if let Some(index) = patch_index {
            edits.push(Edit::RemovePatch(*index));
        }
    }
    ui.end_group();

    ui.start_group();
    let patches = &mut module.patches;
    if ui.button("Save", patch_index.is_some(), Info::SavePatch) {
        if let Some(patch) = patch_index.map(|i| patches.get(i)).flatten() {
            let dialog = super::new_file_dialog(player)
                .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
                .set_directory(cfg.patch_folder.clone().unwrap_or(String::from(".")))
                .set_file_name(patch.name.clone());

            if let Some(mut path) = dialog.save_file() {
                path.set_extension(PATCH_FILTER_EXT);
                cfg.patch_folder = config::dir_as_string(&path);
                if let Err(e) = patch.save(&path) {
                    ui.report(format!("Error saving patch: {e}"));
                }
            }
        }
    }
    if ui.button("Load", true, Info::LoadPatch) {
        let dialog = super::new_file_dialog(player)
            .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
            .add_filter("Sample", &PcmData::FILE_EXTENSIONS)
            .set_directory(cfg.patch_folder.clone().unwrap_or(String::from(".")));

        if let Some(paths) = dialog.pick_files() {
            for (i, path) in paths.iter().enumerate() {
                cfg.patch_folder = config::dir_as_string(path);
                let patch = if path.extension().and_then(|s| s.to_str())
                    .is_some_and(|s| s == PATCH_FILTER_EXT)
                {
                    Patch::load(path)
                } else {
                    Patch::load_sample(path, cfg.trim_samples)
                };
                match patch {
                    Ok(p) => {
                        edits.push(Edit::InsertPatch(patches.len() + i, p));
                        *patch_index = Some(patches.len() + i);
                    },
                    Err(e) => ui.report(format!("Error loading patch: {e}")),
                }
            }
        }
    }
    ui.end_group();

    if ui.button("Duplicate", patch_index.is_some(), Info::DuplicatePatch) {
        let index = patch_index.unwrap();
        if let Some(p) = patches.get(index).map(|p| p.duplicate()) {
            edits.push(Edit::InsertPatch(patches.len(), p));
            *patch_index = Some(patches.len());
        }
    }

    for edit in edits {
        module.push_edit(edit);
        fix_patch_index(patch_index, module.patches.len());
    }

    ui.end_group();
}

/// Correct the patch index if it's out of bounds.
pub fn fix_patch_index(index: &mut Option<usize>, len: usize) {
    if len == 0 {
        *index = None;
    } else if let Some(index) = index {
        *index = (*index).min(len - 1);
    }
}

fn kit_controls(ui: &mut Ui, module: &mut Module, player: &mut PlayerShell) -> bool {
    let mut changed = false;

    if !module.kit.is_empty() {
        ui.start_group();
        let mut removed_index = None;

        labeled_group(ui, "Note in", Info::KitNoteIn, |ui| {
            let mut notes = Vec::new();

            for (i, entry) in module.kit.iter_mut().enumerate() {
                ui.start_group();
                let label = format!("kit_{}_input", i);
                changed |= ui.note_input(&label, &mut entry.input_note, Info::KitNoteIn)
                    .is_some();

                if notes.contains(&entry.input_note) {
                    ui.offset_label("*", Info::DuplicateKitEntry)
                } else {
                    notes.push(entry.input_note);
                }
                ui.end_group();
            }
        });

        labeled_group(ui, "Patch", Info::KitPatch, |ui| {
            for (i, entry) in module.kit.iter_mut().enumerate() {
                let name = module.patches.get(entry.patch_index)
                    .map(|x| x.name.as_ref())
                    .unwrap_or_default();
                if let Some(j) = ui.combo_box(&format!("kit_{}_patch", i), "", name,
                    Info::KitPatch,
                    || module.patches.iter().map(|x| x.name.clone()).collect()) {
                    entry.patch_index = j;
                    changed = true;
                }
            }
        });

        labeled_group(ui, "Note out", Info::KitNoteOut, |ui| {
            for (i, entry) in module.kit.iter_mut().enumerate() {
                let label = format!("kit_{}_output", i);
                let key = ui.note_input(&label, &mut entry.patch_note, Info::KitNoteOut);
                if let Some(key) = key {
                    let pitch = module.tuning.midi_pitch(&entry.patch_note);
                    player.note_on(0, key, pitch, None, entry.patch_index);
                    changed = true;
                }
            }
        });

        labeled_group(ui, "", Info::None, |ui| {
            for i in 0..module.kit.len() {
                if ui.button("X", true, Info::Remove("this mapping")) {
                    removed_index = Some(i);
                    changed = true;
                }
            }
        });

        if let Some(i) = removed_index {
            module.kit.remove(i);
        }
        ui.end_group();
    }

    if ui.button("+", !module.patches.is_empty(), Info::Add("a new mapping")) {
        module.kit.push(Default::default());
        changed = true;
    }

    changed
}

fn patch_controls(ui: &mut Ui, patch: &mut Patch, cfg: &mut Config,
    player: &mut PlayerShell
) -> bool {
    let mut changed = false;

    ui.header("GENERAL", Info::None);
    ui.shared_slider("gain", "Level", &patch.gain.0,
        0.0..=2.0, None, 2, true, Info::None);
    ui.formatted_shared_slider("pan", "Pan", &patch.pan.0,
        -1.0..=1.0, 1, true, Info::None, |f| format!("{f:+.2}"), |f| f);
    changed |= ui.slider("glide_time", "Glide time", &mut patch.glide_time,
        0.0..=0.5, Some("s"), 2, true, Info::GlideTime);

    // TODO: re-enable this if & when recording is implemented
    // if let Some(i) = ui.combo_box("play_mode",
    //     "Play mode", patch.play_mode.name(), Info::PlayMode,
    //     || PlayMode::VARIANTS.map(|v| v.name().to_owned()).to_vec()
    // ) {
    //     patch.play_mode = PlayMode::VARIANTS[i];
    // }

    ui.formatted_shared_slider("distortion", "Distortion", &patch.distortion.0,
        0.0..=1.0, 1, true, Info::Distortion, |f| format!("{f:.2}"), |f| f);
    ui.shared_slider("fx_send", "FX send", &patch.fx_send.0,
        0.0..=1.0, None, 1, true, Info::FxSend);

    ui.vertical_space();
    changed |= generator_controls(ui, patch, cfg, player);
    ui.vertical_space();
    changed |= filter_controls(ui, patch);
    ui.vertical_space();
    changed |= envelope_controls(ui, patch);
    ui.vertical_space();
    changed |= lfo_controls(ui, patch);
    ui.vertical_space();
    changed |= modulation_controls(ui, patch);

    changed
}

fn generator_controls(ui: &mut Ui, patch: &mut Patch, cfg: &mut Config,
    player: &mut PlayerShell
) -> bool {
    ui.header("GENERATORS", Info::Generators);

    ui.start_group();
    let mut removed_osc = None;
    let mut changed = false;

    // the code for these controls is a little hairier because the PCM
    // controls use an extra line.

    labeled_group(ui, "#", Info::None, |ui| {
        for (i, osc) in patch.oscs.iter().enumerate() {
            ui.offset_label(&(i + 1).to_string(), Info::None);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "Level", Info::None, |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_level", i),
                "", &osc.level.0, 0.0..=1.0, None, 2, true, Info::None);

            if let Waveform::Pcm(data) = &mut osc.waveform {
                ui.start_group();
                let mut loaded_sample = false;

                if ui.button("Load sample", true, Info::LoadSample) {
                    loaded_sample |= load_pcm(data, ui, cfg, player);
                }

                ui.group_ignores_geometry = true;

                if let Some(data) = data {
                    if data.path.is_some() {
                        if ui.button("Prev", true, Info::PrevSample) {
                            loaded_sample |= load_pcm_offset(data, -1, ui, cfg.trim_samples);
                        }
                        if ui.button("Next", true, Info::NextSample) {
                            loaded_sample |= load_pcm_offset(data, 1, ui, cfg.trim_samples);
                        }
                    }

                    if ui.button("Detect pitch", true, Info::DetectPitch) {
                        match data.detect_pitch() {
                            Some(freq) => {
                                osc.freq_ratio.0
                                    .set(clamp_freq_ratio(REF_FREQ / freq as f32));
                                osc.fine_pitch.0.set(0.0);
                            },
                            None => ui.report("Could not detect pitch"),
                        }
                        changed = true;
                    }

                    let mut on = data.loop_point.is_some();
                    if ui.checkbox("Loop", &mut on, true, Info::None) {
                        data.loop_point = if on {
                            Some(0)
                        } else {
                            None
                        };
                        changed = true;
                    }

                    if let Some(pt) = &mut data.loop_point {
                        let sr = data.wave.sample_rate() as f32;
                        let mut pt2 = *pt as f32 / sr;
                        if ui.slider(&format!("osc_{}_loop", i), "Loop point", &mut pt2,
                            0.0..=data.wave.duration() as f32, Some("s"), 1, true,
                            Info::LoopPoint) {
                            *pt = (pt2 * sr).round() as usize;
                            data.fix_loop_point();
                            changed = true;
                        }
                    }

                    if !data.filename.is_empty() {
                        ui.offset_label(&format!("({})", &data.filename), Info::None);
                    }
                }

                if loaded_sample {
                    changed = true;
                    if let Some(pitch) = data.as_ref().and_then(|d| d.midi_pitch) {
                        osc.freq_ratio.0.set(clamp_freq_ratio(
                            2.0_f32.powf((REF_PITCH as f32 - pitch) / 12.0)));
                        osc.fine_pitch.0.set(0.0);
                    }
                }

                ui.group_ignores_geometry = false;
                ui.end_group();
            }
        }
    });

    labeled_group(ui, "Tone", Info::Tone, |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_tone", i), "", &osc.tone.0,
                0.0..=1.0, None, 1, osc.waveform.uses_tone(), Info::Tone);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "Freq. ratio", Info::FreqRatio, |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_ratio", i),
                "", &osc.freq_ratio.0, MIN_FREQ_RATIO..=MAX_FREQ_RATIO, None, 2,
                osc.waveform.uses_freq(), Info::FreqRatio);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("" , Info::None);
            }
        }
    });

    labeled_group(ui, "Finetune", Info::None, |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.formatted_shared_slider(&format!("osc_{}_tune", i),
                "", &osc.fine_pitch.0, -0.5..=0.5, 1, osc.waveform.uses_freq(), Info::None,
                |f| format!("{:+.1} cents", f * 100.0), |f| f * 0.01);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "Waveform", Info::Waveform, |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            if let Some(i) = ui.combo_box(&format!("osc_{}_wave", i),
                "", osc.waveform.name(), Info::Waveform,
                || Waveform::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                osc.waveform = Waveform::VARIANTS[i].clone();
                changed = true;
            }

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("" , Info::None);
            }
        }
    });

    labeled_group(ui, "Output", Info::GenOutput, |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            let outputs = OscOutput::choices(i);
            if let Some(i) = ui.combo_box(&format!("osc_{}_output", i),
                "", &osc.output.to_string(), Info::GenOutput,
                || outputs.iter().map(|x| x.to_string()).collect()) {
                osc.output = outputs[i];
                changed = true;
            }

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "2X", Info::Oversample, |ui| {
        for osc in patch.oscs.iter_mut() {
            changed |= ui.checkbox("", &mut osc.oversample,
                osc.waveform.uses_oversampling(), Info::Oversample);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "", Info::None, |ui| {
        for (i, osc) in patch.oscs.iter().enumerate() {
            if patch.oscs.len() < 2 {
                ui.offset_label("", Info::None); // can't delete the only osc
            } else if ui.button("X", true, Info::Remove("this generator")) {
                removed_osc = Some(i);
                changed = true;
            }

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    if let Some(i) = removed_osc {
        patch.remove_osc(i);
    }
    ui.end_group();

    if ui.button("+", true, Info::Add("a generator")) {
        patch.oscs.push(Oscillator::default());
        changed = true;
    }

    changed
}

/// Browse for and load an audio file into `data`. Returns true if successful.
fn load_pcm(data: &mut Option<PcmData>, ui: &mut Ui, cfg: &mut Config,
    player: &mut PlayerShell
) -> bool {
    let dialog = super::new_file_dialog(player)
        .add_filter("Audio file", &PcmData::FILE_EXTENSIONS)
        .set_directory(cfg.sample_folder.clone()
            .unwrap_or(String::from(".")));

    if let Some(path) = dialog.pick_file() {
        cfg.sample_folder = config::dir_as_string(&path);
        match PcmData::load(path, cfg.trim_samples) {
            Ok(result) => {
                *data = Some(result);
                return true
            }
            Err(e) => ui.report(format!("Error loading audio: {e}")),
        }
    }

    false
}

/// Load the previous/next audio file from `data`'s directory. Returns true if
/// successful.
fn load_pcm_offset(data: &mut PcmData, offset: isize, ui: &mut Ui, trim: bool) -> bool {
    if let Some(path) = &data.path {
        match PcmData::load_offset(path, offset, trim) {
            Ok(result) => {
                *data = result;
                if let Some(s) = data.path.as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str()) {
                    ui.notify(format!("Loaded {}", s))
                }
                return true
            }
            Err(e) => ui.report(format!("Error loading audio: {e}")),
        }
    }

    false
}

fn filter_controls(ui: &mut Ui, patch: &mut Patch) -> bool {
    let mut changed = false;

    ui.header("FILTERS", Info::Filters);

    if !patch.filters.is_empty() {
        ui.start_group();
        let mut removed_filter = None;

        index_group(ui, patch.filters.len());

        labeled_group(ui, "Type", Info::FilterType, |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("filter_{}_type", i),
                    "", filter.filter_type.name(), Info::FilterType,
                    || FilterType::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    filter.filter_type = FilterType::VARIANTS[i];
                    changed = true;
                }
            }
        });

        labeled_group(ui, "Cutoff", Info::FilterCutoff, |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                ui.formatted_shared_slider(&format!("filter_{}_cutoff", i), "",
                    &filter.cutoff.0, MIN_FILTER_CUTOFF..=MAX_FILTER_CUTOFF, 2, true,
                    Info::FilterCutoff, |f| format!("{f:.0} Hz"), |f| f);
            }
        });

        labeled_group(ui, "Resonance", Info::FilterResonance, |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                ui.formatted_shared_slider(&format!("filter_{}_q", i), "",
                    &filter.resonance.0, MIN_FILTER_RESONANCE..=1.0, 1, true,
                    Info::FilterResonance, |f| format!("{f:.2}"), |f| f);
            }
        });

        labeled_group(ui, "Keytrack", Info::FilterKeytrack, |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("filter_{}_keytrack", i),
                    "", filter.key_tracking.name(), Info::FilterKeytrack,
                    || KeyTracking::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    filter.key_tracking = KeyTracking::VARIANTS[i];
                    changed = true;
                }
            }
        });

        labeled_group(ui, "", Info::None, |ui| {
            for i in 0..patch.filters.len() {
                if ui.button("X", true, Info::Remove("this filter")) {
                    removed_filter = Some(i);
                    changed = true;
                }
            }
        });

        if let Some(i) = removed_filter {
            patch.remove_filter(i);
        }
        ui.end_group();
    }

    if ui.button("+", true, Info::Add("a filter")) {
        patch.filters.push(Filter::default());
        changed = true;
    }

    changed
}

fn envelope_controls(ui: &mut Ui, patch: &mut Patch) -> bool {
    let mut changed = false;
    ui.header("ENVELOPES", Info::Envelopes);

    if !patch.envs.is_empty() {
        ui.start_group();
        let mut removed_env = None;

        index_group(ui, patch.envs.len());

        labeled_group(ui, "Attack", Info::Attack, |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                changed |= ui.slider(&format!("env_{}_A", i), "", &mut env.attack,
                    0.0..=10.0, Some("s"), 2, true, Info::Attack);
            }
        });

        labeled_group(ui, "Decay", Info::Decay, |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                changed |= ui.slider(&format!("env_{}_D", i), "", &mut env.decay,
                    0.01..=10.0, Some("s"), 2, true, Info::Decay);
            }
        });

        labeled_group(ui, "Sustain", Info::Sustain, |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                changed |= ui.slider(&format!("env_{}_S", i), "", &mut env.sustain,
                    0.0..=1.0, None, 1, true, Info::Sustain);
            }
        });

        labeled_group(ui, "Release", Info::Release, |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                changed |= ui.slider(&format!("env_{}_R", i), "", &mut env.release,
                    0.01..=10.0, Some("s"), 2, true, Info::Release);
            }
        });

        labeled_group(ui, "", Info::None, |ui| {
            for i in 0..patch.envs.len() {
                if ui.button("X", true, Info::Remove("this envelope")) {
                    removed_env = Some(i);
                    changed = true;
                }
            }
        });

        if let Some(i) = removed_env {
            patch.remove_env(i);
        }
        ui.end_group();
    }

    if ui.button("+", true, Info::Add("an envelope")) {
        patch.envs.push(ADSR::default());
        changed = true;
    }

    changed
}

fn lfo_controls(ui: &mut Ui, patch: &mut Patch) -> bool {
    let mut changed = false;
    ui.header("LFOS", Info::Lfos);

    if !patch.lfos.is_empty() {
        let mut removed_lfo = None;
        ui.start_group();

        index_group(ui, patch.lfos.len());

        labeled_group(ui, "Waveform", Info::Waveform, |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("lfo_{}_wave", i),
                    "", lfo.waveform.name(), Info::Waveform,
                    || Waveform::LFO_VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    lfo.waveform = Waveform::LFO_VARIANTS[i].clone();
                    changed = true;
                }
            }
        });

        labeled_group(ui, "Rate", Info::None, |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                let scale = if lfo.audio_rate {
                    AR_RATE_MULTIPLIER
                } else {
                    1.0
                };
                ui.formatted_shared_slider(&format!("lfo_{}_rate", i), "",
                    &lfo.freq.0, MIN_LFO_RATE..=MAX_LFO_RATE, 2, lfo.waveform.uses_freq(),
                    Info::None, |f| format!("{:.2} Hz", f * scale),
                    |f| f / scale);
            }
        });

        labeled_group(ui, "Delay", Info::LfoDelay, |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                changed |= ui.formatted_slider(&format!("lfo_{}_delay", i), "",
                    &mut lfo.delay, 0.0..=10.0, 2, true, Info::LfoDelay,
                    |f| format!("{f:.2} s"), |f| f);
            }
        });

        labeled_group(ui, "AR", Info::LfoAudioRate, |ui| {
            for lfo in patch.lfos.iter_mut() {
                let enabled = lfo.waveform.uses_freq();
                changed |=
                    ui.checkbox("", &mut lfo.audio_rate, enabled, Info::LfoAudioRate);
            }
        });

        labeled_group(ui, "", Info::None, |ui| {
            for i in 0..patch.lfos.len() {
                if ui.button("X", true, Info::Remove("this LFO")) {
                    removed_lfo = Some(i);
                    changed = true;
                }
            }
        });

        if let Some(i) = removed_lfo {
            patch.remove_lfo(i);
        }
        ui.end_group();
    }

    if ui.button("+", true, Info::Add("an LFO")) {
        patch.lfos.push(LFO::default());
        changed = true;
    }

    changed
}

fn modulation_controls(ui: &mut Ui, patch: &mut Patch) -> bool {
    let mut changed = false;
    ui.header("MOD MATRIX", Info::ModMatrix);

    if !patch.mod_matrix.is_empty() {
        let mut removed_mod = None;
        let sources = patch.mod_sources();
        let targets = patch.mod_targets();

        ui.start_group();

        index_group(ui, patch.mod_matrix.len());

        labeled_group(ui, "Source", Info::ModSource, |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("mod_{}_source", i),
                    "", &m.source.to_string(), Info::ModSource,
                    || sources.iter().map(|x| x.to_string()).collect()) {
                    m.source = sources[i];
                    changed = true;
                }
            }
        });

        labeled_group(ui, "Target", Info::ModDest, |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("mod_{}_target", i),
                    "", &m.target.to_string(), Info::ModDest,
                    || targets.iter().map(|x| x.to_string()).collect()) {
                    m.target = targets[i];
                    changed = true;
                }
            }
        });

        labeled_group(ui, "Depth", Info::ModDepth, |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                ui.formatted_shared_slider(&format!("mod_{}_depth", i), "",
                    &m.depth.0, -1.0..=1.0, 1, true, Info::ModDepth,
                    display_mod(&m.target), convert_mod(&m.target));
            }
        });

        labeled_group(ui, "", Info::None, |ui| {
            for i in 0..patch.mod_matrix.len() {
                if ui.button("X", true, Info::Remove("this modulation")) {
                    removed_mod = Some(i);
                    changed = true;
                }
            }
        });

        ui.end_group();

        if let Some(i) = removed_mod {
            patch.remove_mod(i);
        }
    }

    if ui.button("+", true, Info::Add("a modulation")) {
        patch.mod_matrix.push(Modulation::default());
        changed = true;
    }

    changed
}

/// Draw a column of indices.
fn index_group(ui: &mut Ui, len: usize) {
    ui.start_group();
    ui.label("#", Info::None);
    for i in 0..len {
        ui.offset_label(&(i + 1).to_string(), Info::None);
    }
    ui.end_group();
}

/// Wrap a block of UI code in a labeled column.
fn labeled_group(ui: &mut Ui, label: &str, info: Info, f: impl FnOnce(&mut Ui)) {
    ui.start_group();
    ui.label(label, info);
    f(ui);
    ui.end_group();
}

/// Look up the display function for a modulation target.
fn display_mod(target: &ModTarget) -> Box<dyn Fn(f32) -> String> {
    // TODO: this would ideally be a recursive lookup with loop detection in the
    //       case of ModDepth
    match target {
        ModTarget::EnvScale(_) =>
            Box::new(|d| format!("x{:.2}", MAX_ENV_SCALE.powf(d))),
        ModTarget::FilterCutoff(_) =>
            Box::new(|d| format!("{:+.2} octaves", d * FILTER_CUTOFF_MOD_BASE.log2())),
        ModTarget::ClipGain | ModTarget::FilterQ(_) | ModTarget::Tone(_)
            | ModTarget::FxSend => Box::new(|d| format!("{:+.2}", d)),
        ModTarget::FinePitch | ModTarget::OscFinePitch(_) =>
            Box::new(|d| format!("{:+.1} cents", d * 50.0)),
        ModTarget::Gain | ModTarget::Level(_) =>
            Box::new(|d| format!("{:.2}", d * d * d.signum())),
        ModTarget::LFORate(_) =>
            Box::new(|d| format!("x{:.2}", (MAX_LFO_RATE/MIN_LFO_RATE).powf(d))),
        ModTarget::Pitch | ModTarget::OscPitch(_) =>
            Box::new(|d| format!("{:+.2} octaves", d * MAX_PITCH_MOD.log2())),
        ModTarget::Pan | ModTarget::ModDepth(_) =>
            Box::new(|d| format!("{:+.2}", d * 2.0)),
    }
}

/// Look up the conversion function for a modulation target.
fn convert_mod(target: &ModTarget) -> Box<dyn FnOnce(f32) -> f32> {
    match target {
        ModTarget::EnvScale(_) =>
            Box::new(|f| f.log(MAX_ENV_SCALE)),
        ModTarget::FilterCutoff(_) =>
            Box::new(|f| f / FILTER_CUTOFF_MOD_BASE.log2()),
        ModTarget::ClipGain | ModTarget::FilterQ(_) | ModTarget::Tone(_)
            | ModTarget::FxSend => Box::new(|f| f),
        ModTarget::FinePitch | ModTarget::OscFinePitch(_) =>
            Box::new(|f| f / 50.0),
        ModTarget::Gain | ModTarget::Level(_) =>
            Box::new(signed_sqrt),
        ModTarget::LFORate(_) =>
            Box::new(|f| f.log(MAX_LFO_RATE/MIN_LFO_RATE)),
        ModTarget::Pitch | ModTarget::OscPitch(_) =>
            Box::new(|f| f / MAX_PITCH_MOD.log2()),
        ModTarget::Pan | ModTarget::ModDepth(_) => Box::new(|f| f * 0.5),
    }
}

/// Shift the patch index while keeping it in bounds.
fn shift_patch_index(offset: isize, patch_index: &mut Option<usize>, n: usize) {
    if let Some(index) = patch_index {
        if let Some(i) = index.checked_add_signed(offset) {
            *index = i.min(n - 1);
        } else {
            *patch_index = None;
        }
    } else if offset > 0 && n > 0 {
        *patch_index = Some((offset as usize - 1).min(n - 1));
    }
}

fn signed_sqrt(f: f32) -> f32 {
    f.abs().sqrt() * f.signum()
}

/// Clamps `r` to the freq. ratio range that can be set in the UI,
/// by adding or removing octaves.
pub fn clamp_freq_ratio(mut r: f32) -> f32 {
    while r > MAX_FREQ_RATIO {
        r *= 0.5;
    }
    while r < MIN_FREQ_RATIO {
        r *= 2.0;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signed_sqrt() {
        assert_eq!(signed_sqrt(0.0), 0.0);
        assert_eq!(signed_sqrt(1.0), 1.0);
        assert_eq!(signed_sqrt(-1.0), -1.0);
        assert_eq!(signed_sqrt(-4.0), -2.0);
    }

    #[test]
    fn test_clamp_freq_ratio() {
        assert_eq!(clamp_freq_ratio(20.0), 10.0);
        assert_eq!(clamp_freq_ratio(40.0), 10.0);
        assert_eq!(clamp_freq_ratio(0.1), 0.4);
    }
}