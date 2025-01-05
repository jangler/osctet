use macroquad::input::{KeyCode, is_key_pressed};

use crate::{config::{self, Config}, module::{Edit, Module}, playback::Player, synth::*};

use super::{info::Info, Layout, UI};

// for file dialogs
const PATCH_FILTER_NAME: &str = "Instrument";
const PATCH_FILTER_EXT: &str = "oscins";

pub fn draw(ui: &mut UI, module: &mut Module, patch_index: &mut Option<usize>,
    scroll: &mut f32, cfg: &mut Config, player: &mut Player
) {
    if is_key_pressed(KeyCode::Up) {
        shift_patch_index(-1, patch_index, module.patches.len());
    } else if is_key_pressed(KeyCode::Down) {
        shift_patch_index(1, patch_index, module.patches.len());
    }

    ui.layout = Layout::Horizontal;
    ui.start_group();
    patch_list(ui, module, patch_index, cfg, player);
    ui.end_group();
    let old_y = ui.cursor_y;

    ui.cursor_y -= *scroll;
    ui.cursor_z -= 1;
    ui.space(1.0);
    ui.start_group();
    if let Some(index) = patch_index {
        if let Some(patch) = module.patches.get_mut(*index) {
            patch_controls(ui, patch, cfg, player);
        }
    } else {
        kit_controls(ui, module, player);
    }
    ui.cursor_z += 1;
    ui.cursor_y += *scroll;
    let scroll_h = ui.end_group().unwrap().h + ui.style.margin;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(scroll, scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
}

fn patch_list(ui: &mut UI, module: &mut Module, patch_index: &mut Option<usize>,
    cfg: &mut Config, player: &mut Player
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
            if let Some(mut path) = super::new_file_dialog(player)
                .add_filter(PATCH_FILTER_NAME, &[PATCH_FILTER_EXT])
                .set_directory(cfg.patch_folder.clone().unwrap_or(String::from(".")))
                .set_file_name(patch.name.clone())
                .save_file() {
                path.set_extension(PATCH_FILTER_EXT);
                cfg.patch_folder = config::dir_as_string(&path);
                if let Err(e) = patch.save(&path) {
                    ui.report(format!("Error saving patch: {e}"));
                }
            }
        }
    }
    if ui.button("Load", true, Info::LoadPatch) {
        if let Some(paths) = super::new_file_dialog(player)
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
                    Err(e) => ui.report(format!("Error loading patch: {e}")),
                }
            }
        }
    }
    ui.end_group();

    if ui.button("Duplicate", patch_index.is_some(), Info::DuplicatePatch) {
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
                ui.note_input(&label, &mut entry.input_note, Info::KitNoteIn);
            }
        });

        labeled_group(ui, "Patch", |ui| {
            for (i, entry) in module.kit.iter_mut().enumerate() {
                let name = module.patches.get(entry.patch_index)
                    .map(|x| x.name.as_ref())
                    .unwrap_or("");
                if let Some(j) = ui.combo_box(&format!("kit_{}_patch", i), "", name,
                    Info::KitPatch,
                    || module.patches.iter().map(|x| x.name.clone()).collect()) {
                    entry.patch_index = j;
                }
            }
        });

        labeled_group(ui, "Note out", |ui| {
            for (i, entry) in module.kit.iter_mut().enumerate() {
                let label = format!("kit_{}_output", i);
                let key = ui.note_input(&label, &mut entry.patch_note, Info::KitNoteOut);
                if let Some(key) = key {
                    if let Some(patch) = module.patches.get(entry.patch_index) {
                        let pitch = module.tuning.midi_pitch(&entry.patch_note);
                        player.note_on(0, key, pitch, None, patch);
                    }
                }
            }
        });

        labeled_group(ui, "", |ui| {
            for i in 0..module.kit.len() {
                if ui.button("X", true, Info::Remove("this mapping")) {
                    removed_index = Some(i);
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
    }
}

fn patch_controls(ui: &mut UI, patch: &mut Patch, cfg: &mut Config, player: &mut Player) {
    ui.header("GENERAL", Info::None);
    ui.shared_slider("gain", "Gain", &patch.gain.0, 0.0..=1.0, None, 2, true, Info::None);
    ui.shared_slider("pan", "Pan", &patch.pan.0, -1.0..=1.0, None, 1, true, Info::None);
    ui.slider("glide_time", "Glide time", &mut patch.glide_time,
        0.0..=0.5, Some("s"), 2, true, Info::GlideTime);

    // TODO: re-enable this if & when recording is implemented
    // if let Some(i) = ui.combo_box("play_mode",
    //     "Play mode", patch.play_mode.name(), Info::PlayMode,
    //     || PlayMode::VARIANTS.map(|v| v.name().to_owned()).to_vec()
    // ) {
    //     patch.play_mode = PlayMode::VARIANTS[i];
    // }

    ui.shared_slider("distortion", "Distortion",
        &patch.distortion.0, 0.0..=1.0, None, 1, true, Info::Distortion);
    ui.shared_slider("fx_send", "FX send",
        &patch.fx_send.0, 0.0..=1.0, None, 1, true, Info::FxSend);

    ui.space(2.0);
    oscillator_controls(ui, patch, cfg, player);
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

fn oscillator_controls(ui: &mut UI, patch: &mut Patch, cfg: &mut Config,
    player: &mut Player
) {
    ui.header("GENERATORS", Info::Generators);

    ui.start_group();
    let mut removed_osc = None;

    // the code for these controls is a little hairier because the PCM
    // controls use an extra line.

    labeled_group(ui, "#", |ui| {
        for (i, osc) in patch.oscs.iter().enumerate() {
            ui.offset_label(&(i + 1).to_string(), Info::None);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "Level", |ui| {
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
                    // these are two separate if-lets for ownership reasons
                    if data.path.is_some() && ui.button("Prev", true, Info::PrevSample) {
                        loaded_sample |= load_pcm_offset(data, -1, ui);
                    }
                    if data.path.is_some() && ui.button("Next", true, Info::NextSample) {
                        loaded_sample |= load_pcm_offset(data, 1, ui);
                    }

                    if ui.button("Detect pitch", true, Info::DetectPitch) {
                        match data.detect_pitch() {
                            Some(freq) => {
                                osc.freq_ratio.0.set(REF_FREQ / freq as f32);
                                osc.fine_pitch.0.set(0.0);
                            },
                            None => ui.report("Could not detect pitch"),
                        }
                    }

                    let mut on = data.loop_point.is_some();
                    if ui.checkbox("Loop", &mut on, true, Info::None) {
                        data.loop_point = if on {
                            Some(0)
                        } else {
                            None
                        };
                    }

                    if let Some(pt) = &mut data.loop_point {
                        let sr = data.wave.sample_rate() as f32;
                        let mut pt2 = *pt as f32 / sr;
                        if ui.slider(&format!("osc_{}_loop", i), "Loop point", &mut pt2,
                            0.0..=data.wave.duration() as f32, Some("s"), 1, true,
                            Info::LoopPoint) {
                            *pt = (pt2 * sr).round() as usize;
                            data.fix_loop_point();
                        }
                    }
                }

                if loaded_sample {
                    if let Some(pitch) = data.as_ref().and_then(|d| d.midi_pitch) {
                        osc.freq_ratio.0.set(
                            2.0_f32.powf((REF_PITCH as f32 - pitch) / 12.0));
                        osc.fine_pitch.0.set(0.0);
                    }
                }

                ui.group_ignores_geometry = false;
                ui.end_group();
            }
        }
    });

    labeled_group(ui, "Tone", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_tone", i), "", &osc.tone.0,
                0.0..=1.0, None, 1, osc.waveform.uses_tone(), Info::Tone);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "Freq. ratio", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.shared_slider(&format!("osc_{}_ratio", i),
                "", &osc.freq_ratio.0, MIN_FREQ_RATIO..=MAX_FREQ_RATIO, None, 2,
                osc.waveform.uses_freq(), Info::FreqRatio);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("" , Info::None);
            }
        }
    });

    labeled_group(ui, "Finetune", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            ui.formatted_shared_slider(&format!("osc_{}_tune", i),
                "", &osc.fine_pitch.0, -0.5..=0.5, 1, osc.waveform.uses_freq(), Info::None,
                |f| format!("{:+.1} cents", f * 100.0), |f| f * 0.01);

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "Waveform", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            if let Some(i) = ui.combo_box(&format!("osc_{}_wave", i),
                "", osc.waveform.name(), Info::Waveform,
                || Waveform::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                osc.waveform = Waveform::VARIANTS[i].clone();
            }

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("" , Info::None);
            }
        }
    });

    labeled_group(ui, "Output", |ui| {
        for (i, osc) in patch.oscs.iter_mut().enumerate() {
            let outputs = OscOutput::choices(i);
            if let Some(i) = ui.combo_box(&format!("osc_{}_output", i),
                "", &osc.output.to_string(), Info::GenOutput,
                || outputs.iter().map(|x| x.to_string()).collect()) {
                osc.output = outputs[i];
            }

            if let Waveform::Pcm(_) = osc.waveform {
                ui.offset_label("", Info::None);
            }
        }
    });

    labeled_group(ui, "", |ui| {
        for (i, osc) in patch.oscs.iter().enumerate() {
            if patch.oscs.len() < 2 {
                ui.offset_label("", Info::None); // can't delete the only osc
            } else if ui.button("X", true, Info::Remove("this generator")) {
                removed_osc = Some(i);
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
        patch.oscs.push(Oscillator::new());
    }
}

fn load_pcm(data: &mut Option<PcmData>, ui: &mut UI, cfg: &mut Config,
    player: &mut Player
) -> bool {
    if let Some(path) = super::new_file_dialog(player)
        .add_filter("Audio file", &PcmData::FILE_EXTENSIONS)
        .set_directory(cfg.sample_folder.clone()
            .unwrap_or(String::from(".")))
        .pick_file() {
        cfg.sample_folder = config::dir_as_string(&path);
        match PcmData::load(path) {
            Ok(result) => {
                *data = Some(result);
                return true
            }
            Err(e) => ui.report(format!("Error loading audio: {e}")),
        }
    }

    false
}

fn load_pcm_offset(data: &mut PcmData, offset: isize, ui: &mut UI) -> bool {
    if let Some(path) = &data.path {
        match PcmData::load_offset(path, offset) {
            Ok(result) => {
                *data = result;
                data.path.as_ref()
                    .map(|p| p.file_name()).flatten()
                    .map(|s| s.to_str()).flatten()
                    .map(|s| ui.notify(format!("Loaded {}", s)));
                return true
            }
            Err(e) => ui.report(format!("Error loading audio: {e}")),
        }
    }

    false
}

fn filter_controls(ui: &mut UI, patch: &mut Patch) {
    ui.header("FILTERS", Info::Filters);

    if !patch.filters.is_empty() {
        ui.start_group();
        let mut removed_filter = None;

        index_group(ui, patch.filters.len());

        labeled_group(ui, "Type", |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("filter_{}_type", i),
                    "", filter.filter_type.name(), Info::FilterType,
                    || FilterType::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    filter.filter_type = FilterType::VARIANTS[i];
                }
            }
        });

        labeled_group(ui, "Cutoff", |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                ui.shared_slider(&format!("filter_{}_cutoff", i), "", &filter.cutoff.0,
                    MIN_FILTER_CUTOFF..=MAX_FILTER_CUTOFF, Some("Hz"), 2, true,
                    Info::FilterCutoff);
            }
        });

        labeled_group(ui, "Resonance", |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                ui.shared_slider(&format!("filter_{}_q", i), "",
                    &filter.resonance.0, MIN_FILTER_RESONANCE..=1.0, None, 1, true,
                    Info::FilterResonance);
            }
        });

        labeled_group(ui, "Keytrack", |ui| {
            for (i, filter) in patch.filters.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("filter_{}_keytrack", i),
                    "", filter.key_tracking.name(), Info::FilterKeytrack,
                    || KeyTracking::VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    filter.key_tracking = KeyTracking::VARIANTS[i];
                }
            }
        });

        labeled_group(ui, "", |ui| {
            for i in 0..patch.filters.len() {
                if ui.button("X", true, Info::Remove("this filter")) {
                    removed_filter = Some(i);
                }
            }
        });

        if let Some(i) = removed_filter {
            patch.remove_filter(i);
        }
        ui.end_group();
    }

    if ui.button("+", true, Info::Add("a filter")) {
        patch.filters.push(Filter::new());
    }
}

fn envelope_controls(ui: &mut UI, patch: &mut Patch) {
    ui.header("ENVELOPES", Info::Envelopes);

    if !patch.envs.is_empty() {
        ui.start_group();
        let mut removed_env = None;

        index_group(ui, patch.envs.len());

        labeled_group(ui, "Attack", |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                ui.slider(&format!("env_{}_A", i), "", &mut env.attack, 0.0..=10.0,
                    Some("s"), 2, true, Info::Attack);
            }
        });

        labeled_group(ui, "Decay", |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                ui.slider(&format!("env_{}_D", i), "", &mut env.decay, 0.01..=10.0,
                    Some("s"), 2, true, Info::Decay);
            }
        });

        labeled_group(ui, "Sustain", |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                ui.slider(&format!("env_{}_S", i), "", &mut env.sustain, 0.0..=1.0,
                    None, 1, true, Info::Sustain);
            }
        });

        labeled_group(ui, "Release", |ui| {
            for (i, env) in patch.envs.iter_mut().enumerate() {
                ui.slider(&format!("env_{}_R", i), "", &mut env.release, 0.01..=10.0,
                    Some("s"), 2, true, Info::Release);
            }
        });

        labeled_group(ui, "", |ui| {
            for i in 0..patch.envs.len() {
                if ui.button("X", true, Info::Remove("this envelope")) {
                    removed_env = Some(i);
                }
            }
        });

        if let Some(i) = removed_env {
            patch.remove_env(i);
        }
        ui.end_group();
    }

    if ui.button("+", true, Info::Add("an envelope")) {
        patch.envs.push(ADSR::new());
    }
}

fn lfo_controls(ui: &mut UI, patch: &mut Patch) {
    ui.header("LFOS", Info::Lfos);

    if !patch.lfos.is_empty() {
        let mut removed_lfo = None;
        ui.start_group();

        index_group(ui, patch.lfos.len());

        labeled_group(ui, "Waveform", |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("lfo_{}_wave", i),
                    "", lfo.waveform.name(), Info::Waveform,
                    || Waveform::LFO_VARIANTS.map(|x| x.name().to_owned()).to_vec()) {
                    lfo.waveform = Waveform::LFO_VARIANTS[i].clone();
                }
            }
        });

        labeled_group(ui, "Rate", |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                ui.shared_slider(&format!("lfo_{}_rate", i), "", &lfo.freq.0,
                    MIN_LFO_RATE..=MAX_LFO_RATE, Some("Hz"), 2, lfo.waveform.uses_freq(),
                    Info::None);
            }
        });

        labeled_group(ui, "Delay", |ui| {
            for (i, lfo) in patch.lfos.iter_mut().enumerate() {
                ui.slider(&format!("lfo_{}_delay", i), "", &mut lfo.delay,
                    0.0..=10.0, Some("s"), 2, true, Info::LfoDelay);
            }
        });

        labeled_group(ui, "", |ui| {
            for i in 0..patch.lfos.len() {
                if ui.button("X", true, Info::Remove("this LFO")) {
                    removed_lfo = Some(i);
                }
            }
        });

        if let Some(i) = removed_lfo {
            patch.remove_lfo(i);
        }
        ui.end_group();
    }

    if ui.button("+", true, Info::Add("an LFO")) {
        patch.lfos.push(LFO::new());
    }
}

fn modulation_controls(ui: &mut UI, patch: &mut Patch) {
    ui.header("MOD MATRIX", Info::ModMatrix);

    if !patch.mod_matrix.is_empty() {
        let mut removed_mod = None;
        let sources = patch.mod_sources();
        let targets = patch.mod_targets();

        ui.start_group();

        index_group(ui, patch.mod_matrix.len());

        labeled_group(ui, "Source", |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("mod_{}_source", i),
                    "", &m.source.to_string(), Info::ModSource,
                    || sources.iter().map(|x| x.to_string()).collect()) {
                    m.source = sources[i];
                }
            }
        });

        labeled_group(ui, "Target", |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                if let Some(i) = ui.combo_box(&format!("mod_{}_target", i),
                    "", &m.target.to_string(), Info::ModDest,
                    || targets.iter().map(|x| x.to_string()).collect()) {
                    m.target = targets[i];
                }
            }
        });

        labeled_group(ui, "Depth", |ui| {
            for (i, m) in patch.mod_matrix.iter_mut().enumerate() {
                ui.formatted_shared_slider(&format!("mod_{}_depth", i), "", &m.depth.0,
                    -1.0..=1.0, 1, true, Info::ModDepth,
                    display_mod(&m.target), convert_mod(&m.target));
            }
        });

        labeled_group(ui, "", |ui| {
            for i in 0..patch.mod_matrix.len() {
                if ui.button("X", true, Info::Remove("this modulation")) {
                    removed_mod = Some(i);
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
    }
}

fn index_group(ui: &mut UI, len: usize) {
    ui.start_group();
    ui.label("#");
    for i in 0..len {
        ui.offset_label(&(i + 1).to_string(), Info::None);
    }
    ui.end_group();
}

fn labeled_group(ui: &mut UI, label: &str, f: impl FnOnce(&mut UI) -> ()) {
    ui.start_group();
    ui.label(label);
    f(ui);
    ui.end_group();
}

// TODO: this would ideally be a recursive lookup with loop detection in the
//       case of ModDepth
fn display_mod(target: &ModTarget) -> Box<dyn Fn(f32) -> String> {
    match target {
        ModTarget::EnvScale(_) =>
            Box::new(|d| format!("x{:.2}", MAX_ENV_SCALE.powf(d))),
        ModTarget::FilterCutoff(_) =>
            Box::new(|d| format!("{:+.2} octaves", d * FILTER_CUTOFF_MOD_BASE.log2())),
        ModTarget::ClipGain | ModTarget::FilterQ(_) | ModTarget::Tone(_)
            | ModTarget::ModDepth(_) => Box::new(|d| format!("{:+.2}", d)),
        ModTarget::FinePitch | ModTarget::OscFinePitch(_) =>
            Box::new(|d| format!("{:+.1} cents", d * 50.0)),
        ModTarget::Gain | ModTarget::Level(_) =>
            Box::new(|d| format!("{:.2}", d * d)),
        ModTarget::LFORate(_) =>
            Box::new(|d| format!("x{:.2}", (MAX_LFO_RATE/MIN_LFO_RATE).powf(d))),
        ModTarget::Pitch | ModTarget::OscPitch(_) =>
            Box::new(|d| format!("{:+.2} octaves", d * PITCH_MOD_BASE.log2())),
        ModTarget::Pan => Box::new(|d| format!("{:+.2}", d * 2.0)),
    }
}

fn convert_mod(target: &ModTarget) -> Box<dyn FnOnce(f32) -> f32> {
    match target {
        ModTarget::EnvScale(_) =>
            Box::new(|f| f.log(MAX_ENV_SCALE)),
        ModTarget::FilterCutoff(_) =>
            Box::new(|f| f / FILTER_CUTOFF_MOD_BASE.log2()),
        ModTarget::ClipGain | ModTarget::FilterQ(_) | ModTarget::Tone(_)
            | ModTarget::ModDepth(_) => Box::new(|f| f),
        ModTarget::FinePitch | ModTarget::OscFinePitch(_) =>
            Box::new(|f| f / 50.0),
        ModTarget::Gain | ModTarget::Level(_) =>
            Box::new(|f| f.sqrt()),
        ModTarget::LFORate(_) =>
            Box::new(|f| f.log(MAX_LFO_RATE/MIN_LFO_RATE)),
        ModTarget::Pitch | ModTarget::OscPitch(_) =>
            Box::new(|f| f / PITCH_MOD_BASE.log2()),
        ModTarget::Pan => Box::new(|f| f * 0.5),
    }
}

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