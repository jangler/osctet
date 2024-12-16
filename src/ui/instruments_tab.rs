use fundsp::math::midi_hz;

use crate::{config::{self, Config}, module::{Edit, Module}, playback::Player, synth::*};

use super::{Layout, MARGIN, UI};

// for file dialogs
const PATCH_FILTER_NAME: &str = "Instrument";
const PATCH_FILTER_EXT: &str = "oscins";

pub fn draw(ui: &mut UI, module: &mut Module, patch_index: &mut Option<usize>,
    scroll: &mut f32, cfg: &mut Config, player: &mut Player
) {
    ui.layout = Layout::Horizontal;
    ui.start_group();
    patch_list(ui, module, patch_index, cfg);
    ui.end_group();
    let old_y = ui.cursor_y;

    ui.cursor_y -= *scroll;
    ui.cursor_z -= 1;
    ui.space(1.0);
    ui.start_group();
    if let Some(index) = patch_index {
        if let Some(patch) = module.patches.get_mut(*index) {
            patch_controls(ui, patch, cfg);
        }
    } else {
        kit_controls(ui, module, player);
    }
    ui.cursor_z += 1;
    ui.cursor_y += *scroll;
    let scroll_h = ui.end_group().unwrap().h + MARGIN;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(scroll, scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
}

fn patch_list(ui: &mut UI, module: &mut Module, patch_index: &mut Option<usize>,
    cfg: &mut Config
) {
    let mut edits = Vec::new();
    let patches = &mut module.patches;

    let mut names = vec![String::from("Kit")];
    names.extend(patches.iter().map(|x| x.name.clone()));

    let mut list_index = match patch_index {
        Some(i) => *i + 1,
        None => 0,
    };
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
    if ui.button("Add") {
        edits.push(Edit::InsertPatch(patches.len(), Patch::new()));
        *patch_index = Some(patches.len());
    }

    if ui.button("Remove") {
        if let Some(index) = patch_index {
            edits.push(Edit::RemovePatch(*index));
        }
    }
    ui.end_group();

    ui.start_group();
    let patches = &mut module.patches;
    if ui.button("Save") {
        if let Some(patch) = patch_index.map(|i| patches.get(i)).flatten() {
            if let Some(path) = super::new_file_dialog()
                .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
                .set_directory(cfg.patch_folder.clone().unwrap_or(String::from(".")))
                .set_file_name(patch.name.clone())
                .save_file() {
                cfg.patch_folder = config::dir_as_string(&path);
                let _ = cfg.save();
                if let Err(e) = patch.save(&path) {
                    ui.report(e);
                }
            }
        }
    }
    if ui.button("Load") {
        if let Some(paths) = super::new_file_dialog()
            .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
            .set_directory(cfg.patch_folder.clone().unwrap_or(String::from(".")))
            .pick_files() {
            for (i, path) in paths.iter().enumerate() {
                cfg.patch_folder = config::dir_as_string(&path);
                match Patch::load(&path) {
                    Ok(mut p) => {
                        if let Some(s) = path.file_stem() {
                            if let Some(s) = s.to_str() {
                                p.name = s.to_owned();
                            }
                        }
                        edits.push(Edit::InsertPatch(patches.len() + i, p));
                        *patch_index = Some(patches.len() + i);
                    },
                    Err(e) => ui.report(e),
                }
            }
            let _ = cfg.save();
        }
    }
    ui.end_group();
    
    if ui.button("Duplicate") {
        if let Some(index) = patch_index {
            if let Some(p) = patches.get(*index).map(|p| p.duplicate().ok()).flatten() {
                edits.push(Edit::InsertPatch(patches.len(), p));
                *patch_index = Some(patches.len());
            }
        }
    }

    for edit in edits {
        module.push_edit(edit);
        fix_patch_index(patch_index, module.patches.len());
    }
}

pub fn fix_patch_index(index: &mut Option<usize>, len: usize) {
    if len == 0 {
        *index = None;
    } else if let Some(index) = index {
        if *index >= len {
            *index = len - 1;
        }
    }
}

fn kit_controls(ui: &mut UI, module: &mut Module, player: &mut Player) {
    if !module.kit.is_empty() {
        ui.start_group();
        let mut removed_index = None;
    
        labeled_group(ui, "Note in", |ui| {
            for (i, entry) in module.kit.iter_mut().enumerate() {
                let label = format!("kit_{}_input", i);
                ui.note_input(&label, &mut entry.input_note);
            }
        });
        
        labeled_group(ui, "Patch", |ui| {
            for (i, entry) in module.kit.iter_mut().enumerate() {
                let name = module.patches.get(entry.patch_index)
                    .map(|x| x.name.as_ref())
                    .unwrap_or("");
                if let Some(j) = ui.combo_box(&format!("kit_{}_patch", i), "", name,
                    || module.patches.iter().map(|x| x.name.clone()).collect()) {
                    entry.patch_index = j;
                }
            }
        });
        
        labeled_group(ui, "Note out", |ui| {
            for (i, entry) in module.kit.iter_mut().enumerate() {
                let label = format!("kit_{}_output", i);
                if let Some(key) = ui.note_input(&label, &mut entry.patch_note) {
                    if let Some(patch) = module.patches.get(entry.patch_index) {
                        let pitch = module.tuning.midi_pitch(&entry.patch_note);
                        player.note_on(0, key, pitch, None, patch);
                    }
                }
            }
        });
        
        labeled_group(ui, "", |ui| {
            for i in 0..module.kit.len() {
                if ui.button("X") {
                    removed_index = Some(i);
                }
            }
        });
    
        if let Some(i) = removed_index {
            module.kit.remove(i);
        }
        ui.end_group();
    }

    if !module.patches.is_empty() && ui.button("+") {
        module.kit.push(Default::default());
    }
}

fn patch_controls(ui: &mut UI, patch: &mut Patch, cfg: &mut Config) {
    ui.header("GENERAL");
    ui.shared_slider("gain", "Gain", &patch.gain.0, 0.0..=1.0, None, 2);
    ui.shared_slider("pan", "Pan", &patch.pan.0, -1.0..=1.0, None, 1);
    ui.slider("glide_time", "Glide time", &mut patch.glide_time, 0.0..=0.5, Some("s"), 2);
    if let Some(i) = ui.combo_box("play_mode",
        "Play mode", patch.play_mode.name(),
        || PlayMode::VARIANTS.map(|v| v.name().to_owned()).to_vec()
    ) {
        patch.play_mode = PlayMode::VARIANTS[i];
    }
    ui.shared_slider("distortion", "Distortion",
        &patch.clip_gain.0, 1.0..=MAX_CLIP_GAIN, None, 1);
    ui.shared_slider("reverb_send", "Reverb send",
        &patch.reverb_send.0, 0.0..=1.0, None, 1);

    ui.space(2.0);
    oscillator_controls(ui, patch, cfg);
    ui.space(2.0);
    filter_controls(ui, patch);
    ui.space(2.0);
    envelope_controls(ui, patch);
    ui.space(2.0);
    lfo_controls(ui, patch);
    ui.space(2.0);
    modulation_controls(ui, patch);
    ui.space(2.0);
}

fn oscillator_controls(ui: &mut UI, patch: &mut Patch, cfg: &mut Config) {
    ui.header("GENERATORS");

    ui.start_group();
    let mut removed_osc = None;

    // the code for these controls is a little hairier because the PCM
    // controls use an extra line.

    labeled_group(ui, "#", |ui| {
        for (i, osc) in patch.oscs.iter().enumerate() {
            ui.offset_label(&(i + 1).to_string());

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("");
            }
        }
    });

    labeled_group(ui, "Level", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_level", i),
                "", &osc.level.0, 0.0..=1.0, None, 2);

            if let Waveform::Pcm(data) = &mut osc.waveform {
                if ui.button("Load sample") {
                    load_pcm(data, ui, cfg);
                }
            }
        }
    });
    
    labeled_group(ui, "Tone", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_tone", i),
                "", &osc.tone.0, 0.0..=1.0, None, 1);

            if let Waveform::Pcm(data) = &mut osc.waveform {
                if let Some(data) = data {
                    let mut on = data.loop_point.is_some();
                    if ui.checkbox("Loop", &mut on) {
                        data.loop_point = if on {
                            Some(0)
                        } else {
                            None
                        };
                    }
                } else {
                    ui.offset_label("");
                }
            }
        }
    });
    
    labeled_group(ui, "Freq. ratio", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_ratio", i),
                "", &osc.freq_ratio.0, MIN_FREQ_RATIO..=MAX_FREQ_RATIO, None, 2);

            if let Waveform::Pcm(data) = &mut osc.waveform {
                if let Some(PcmData { wave, loop_point: Some(pt), .. }) = data {
                    let sr = wave.sample_rate() as f32;
                    let mut pt2 = *pt as f32 / sr;
                    if ui.slider(&format!("osc_{}_loop", i), "",
                        &mut pt2, 0.0..=wave.duration() as f32, Some("s"), 1) {
                        *pt = (pt2 * sr).round() as usize;
                        data.as_mut().unwrap().fix_loop_point();
                    }
                } else {
                    ui.offset_label("");
                }
            }
        }
    });
    
    labeled_group(ui, "Finetune", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_tune", i),
                "", &osc.fine_pitch.0, -0.5..=0.5, Some("semitones"), 1);

            if let Waveform::Pcm(data) = &osc.waveform {
                if let Some(data) = data {
                    if ui.button("Detect pitch") {
                        match data.detect_pitch() {
                            Some(freq) => {
                                osc.freq_ratio.0.set(midi_hz(60.0) / freq as f32);
                                osc.fine_pitch.0.set(0.0);
                            },
                            None => ui.report("Could not detect pitch"),
                        }
                    }    
                } else {
                    ui.offset_label("");
                }
            }
        }
    });
    
    labeled_group(ui, "Waveform", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            if let Some(i) = ui.combo_box(&format!("osc_{}_wave", i),
                "", osc.waveform.name(),
                || Waveform::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                osc.waveform = Waveform::VARIANTS[i].clone();
            }

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("");
            }
        }
    });
    
    labeled_group(ui, "Output", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            let outputs = OscOutput::choices(i);
            if let Some(i) = ui.combo_box(&format!("osc_{}_output", i),
                "", &osc.output.to_string(),
                || outputs.iter().map(|x| x.to_string()).collect()) {
                osc.output = outputs[i];
            }

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("");
            }
        }
    });

    labeled_group(ui, "", |ui| {
        for (i, osc) in patch.oscs.iter().enumerate() {
            if i == 0 {
                ui.offset_label(""); // can't delete first osc
            } else if ui.button("X") {
                removed_osc = Some(i);
            }

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("");
            }
        }
    });

    if let Some(i) = removed_osc {
        patch.remove_osc(i);
    }
    ui.end_group();

    if ui.button("+") {
        patch.oscs.push(Oscillator::new());
    }
}

fn load_pcm(data: &mut Option<PcmData>, ui: &mut UI, cfg: &mut Config) {
    if let Some(path) = super::new_file_dialog()
        .add_filter("Audio file",&["aac", "aiff", "caf", "flac", "m4a", "mkv", "mp3", "mp4",
            "ogg", "wav", "webm"])
        .set_directory(cfg.sample_folder.clone()
            .unwrap_or(String::from(".")))
        .pick_file() {
        cfg.sample_folder = config::dir_as_string(&path);
        let _ = cfg.save();
        match PcmData::load(path) {
            Ok(result) => *data = Some(result),
            Err(e) => ui.report(e),
        }
    }
}

fn filter_controls(ui: &mut UI, patch: &mut Patch) {
    ui.header("FILTERS");

    if !patch.filters.is_empty() {
        ui.start_group();
        let mut removed_filter = None;

        index_group(ui, patch.filters.len());

        labeled_group(ui, "Type", |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("filter_{}_type", i),
                    "", filter.filter_type.name(),
                    || FilterType::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    filter.filter_type = FilterType::VARIANTS[i];
                }
            }
        });
        
        labeled_group(ui, "Cutoff", |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                ui.shared_slider(&format!("filter_{}_cutoff", i), "",
                    &filter.cutoff.0, MIN_FILTER_CUTOFF..=MAX_FILTER_CUTOFF, Some("Hz"), 2);
            }
        });

        labeled_group(ui, "Resonance", |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                ui.shared_slider(&format!("filter_{}_q", i), "",
                    &filter.resonance.0, MIN_FILTER_RESONANCE..=1.0, None, 1);
            }
        });

        labeled_group(ui, "Keytrack", |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("filter_{}_keytrack", i),
                    "", filter.key_tracking.name(),
                    || KeyTracking::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    filter.key_tracking = KeyTracking::VARIANTS[i];
                }
            }
        });

        labeled_group(ui, "", |ui| {
            for i in 0..patch.filters.len() {
                if ui.button("X") {
                    removed_filter = Some(i);
                }
            }
        });
        
        if let Some(i) = removed_filter {
            patch.remove_filter(i);
        }
        ui.end_group();
    }

    if ui.button("+") {
        patch.filters.push(Filter::new());
    }
}

fn envelope_controls(ui: &mut UI, patch: &mut Patch) {
    ui.header("ENVELOPES");
    
    if !patch.envs.is_empty() {
        ui.start_group();
        let mut removed_env = None;

        index_group(ui, patch.envs.len());
    
        labeled_group(ui, "Attack", |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                ui.slider(&format!("env_{}_A", i), "", &mut env.attack, 0.0..=10.0,
                    Some("s"), 2);
            }
        });
    
        labeled_group(ui, "Decay", |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                ui.slider(&format!("env_{}_D", i), "", &mut env.decay, 0.01..=10.0,
                    Some("s"), 2);
            }
        });
    
        labeled_group(ui, "Sustain", |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                ui.slider(&format!("env_{}_S", i), "", &mut env.sustain, 0.0..=1.0,
                    None, 1);
            }
        });
    
        labeled_group(ui, "Release", |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                ui.slider(&format!("env_{}_R", i), "", &mut env.release, 0.01..=10.0,
                    Some("s"), 2);
            }
        });
    
        labeled_group(ui, "", |ui| {
            for i in 0..patch.envs.len() {
                if ui.button("X") {
                    removed_env = Some(i);
                }
            }
        });

        if let Some(i) = removed_env {
            patch.remove_env(i);
        }
        ui.end_group();
    }

    if ui.button("+") {
        patch.envs.push(ADSR::new());
    }
}

fn lfo_controls(ui: &mut UI, patch: &mut Patch) {
    ui.header("LFOS");

    if !patch.lfos.is_empty() {
        let mut removed_lfo = None;
        ui.start_group();
    
        index_group(ui, patch.lfos.len());
        
        labeled_group(ui, "Waveform", |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("lfo_{}_wave", i),
                    "", lfo.waveform.name(),
                    || Waveform::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    lfo.waveform = Waveform::VARIANTS[i].clone();
                }
            }
        });
        
        labeled_group(ui, "Rate", |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                ui.shared_slider(&format!("lfo_{}_rate", i), "", &lfo.freq.0,
                    MIN_LFO_RATE..=MAX_LFO_RATE, Some("Hz"), 2);
            }
        });
    
        labeled_group(ui, "Delay", |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                ui.slider(&format!("lfo_{}_delay", i), "", &mut lfo.delay,
                    0.0..=10.0, Some("s"), 2);
            }
        });
    
        labeled_group(ui, "", |ui| {
            for i in 0..patch.lfos.len() {
                if ui.button("X") {
                    removed_lfo = Some(i);
                }
            }
        });
    
        if let Some(i) = removed_lfo {
            patch.remove_lfo(i);
        }
        ui.end_group();
    }
    
    if ui.button("+") {
        patch.lfos.push(LFO::new());
    }
}

fn modulation_controls(ui: &mut UI, patch: &mut Patch) {
    ui.header("MOD MATRIX");

    if !patch.mod_matrix.is_empty() {
        let mut removed_mod = None;
        let sources = patch.mod_sources();
        let targets = patch.mod_targets();
    
        ui.start_group();
    
        index_group(ui, patch.mod_matrix.len());
    
        labeled_group(ui, "Source", |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("mod_{}_source", i),
                    "", &m.source.to_string(),
                    || sources.iter().map(|x| x.to_string()).collect()) {
                    m.source = sources[i];
                }
            }
        });
    
        labeled_group(ui, "Target", |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("mod_{}_target", i),
                    "", &m.target.to_string(),
                    || targets.iter().map(|x| x.to_string()).collect()) {
                    m.target = targets[i];
                }
            }
        });
    
        labeled_group(ui, "Depth", |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                ui.shared_slider(&format!("mod_{}_depth", i), "", &m.depth.0, -1.0..=1.0,
                    None, 1);
            }
        });
    
        labeled_group(ui, "", |ui| {
            for i in 0..patch.mod_matrix.len() {
                if ui.button("X") {
                    removed_mod = Some(i);
                }
            }
        });
    
        ui.end_group();
    
        if let Some(i) = removed_mod {
            patch.remove_mod(i);
        }
    }

    if ui.button("+") {
        patch.mod_matrix.push(Modulation::default());
    }
}

fn index_group(ui: &mut UI, len: usize) {
    ui.start_group();
    ui.label("#");
    for i in 0..len {
        ui.offset_label(&(i + 1).to_string());
    }
    ui.end_group();
}

fn labeled_group(ui: &mut UI, label: &str, f: impl FnOnce(&mut UI) -> ()) {
    ui.start_group();
    ui.label(label);
    f(ui);
    ui.end_group();
}