use palette::Lchuv;

use crate::{config::{self, Config, RenderFormat}, playback::PlayerShell, Midi};

use super::{info::Info, text::{self, GlyphAtlas}, theme::Theme, Layout, Ui};

/// State for the settings tab UI.
pub struct SettingsState {
    scroll: f32,
    sample_rate: u32,
}

impl SettingsState {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            scroll: 0.0,
            sample_rate,
        }
    }
}

pub fn draw(ui: &mut Ui, cfg: &mut Config, state: &mut SettingsState,
    player: &mut PlayerShell, midi: &mut Midi
) {
    ui.layout = Layout::Horizontal;
    let old_y = ui.cursor_y;
    ui.cursor_y -= state.scroll;
    ui.cursor_z -= 1;
    ui.start_group();

    general_controls(ui, cfg);
    ui.vertical_space();
    io_controls(ui, cfg, state.sample_rate, midi, player);
    ui.vertical_space();
    appearance_controls(ui, cfg, player);
    ui.vertical_space();
    let id = hotkey_controls(ui, cfg);
    ui.vertical_space();
    note_key_controls(ui, cfg, id);

    // TODO: duplication with instruments tab scroll code
    let scroll_h = ui.end_group().unwrap().h + ui.style.margin;
    ui.cursor_z += 1;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(&mut state.scroll,
        scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
}

fn general_controls(ui: &mut Ui, cfg: &mut Config) {
    ui.header("GENERAL", Info::None);

    if ui.button("Reset to defaults", true, Info::ResetSettings) {
        cfg.reset();
        ui.style.theme = Default::default();
    }
    ui.checkbox("Smooth playhead", &mut cfg.smooth_playhead, true, Info::SmoothPlayhead);
    ui.checkbox("Display info text", &mut cfg.display_info, true, Info::DisplayInfo);
}

fn io_controls(ui: &mut Ui, cfg: &mut Config, sample_rate: u32, midi: &mut Midi,
    player: &mut PlayerShell
) {
    ui.header("I/O", Info::None);

    if let Some(s) = ui.edit_box("Desired sample rate", 6,
        cfg.desired_sample_rate.to_string(), Info::DesiredSampleRate
    ) {
        match s.parse::<u32>() {
            Ok(n) => cfg.desired_sample_rate = n,
            Err(e) => ui.report(e),
        }
    }
    if sample_rate != cfg.desired_sample_rate {
        ui.label(&format!("Actual sample rate: {} Hz", sample_rate), Info::None);
    }

    if midi.input.is_some() {
        ui.start_group();

        // midi input selection
        let s = if let Some(name) = &midi.port_name {
            name
        } else {
            "(none)"
        };
        if let Some(i) = ui.combo_box("midi_input", "MIDI input", s,
            Info::MidiInput, || input_names(midi.input.as_ref().unwrap())) {
            midi.port_selection = if i == 0 {
                None
            } else {
                input_names(midi.input.as_ref().unwrap()).get(i).cloned()
            };
        }

        let mut v = cfg.midi_send_pressure.unwrap_or(true);
        if ui.checkbox("Use aftertouch", &mut v, midi.port_name.is_some(),
            Info::UseAftertouch) {
            cfg.midi_send_pressure = Some(v);
        }

        if ui.checkbox("Use velocity", &mut cfg.midi_send_velocity, midi.port_name.is_some(),
            Info::UseVelocity) {
            player.reset_memory();
        }

        ui.end_group();
    } else {
        ui.label("No MIDI device", Info::None);
    }

    if let Some(i) = ui.combo_box("render_format", "Render format",
        &cfg.render_format.to_string(), Info::RenderFormat,
        || RenderFormat::VARIANTS.map(|x| x.to_string()).to_vec()
    ) {
        cfg.render_format = RenderFormat::VARIANTS[i]
    }
}

fn appearance_controls(ui: &mut Ui, cfg: &mut Config, player: &mut PlayerShell) {
    ui.header("APPEARANCE", Info::None);

    ui.start_group();
    color_controls(ui, "Foreground", false, |t| &mut t.fg);
    color_controls(ui, "Background", false, |t| &mut t.bg);
    color_controls(ui, "Accent 1", true, |t| &mut t.accent1);
    color_controls(ui, "Accent 2", true, |t| &mut t.accent2);
    {
        ui.start_group();
        let mut g = ui.style.theme.gamma;
        if ui.slider("gamma", "Gamma", &mut g, 1.5..=2.5, None, 1, true, Info::Gamma) {
            ui.style.theme.gamma = g;
        }
        ui.color_table(ui.style.theme.color_table());
        ui.end_group();
    }
    ui.end_group();

    ui.start_group();
    if ui.button("Reset (light)", true, Info::ResetTheme("light")) {
        ui.style.theme = Theme::light(ui.style.theme.gamma);
    }
    if ui.button("Reset (dark)", true, Info::ResetTheme("dark")) {
        ui.style.theme = Theme::dark(ui.style.theme.gamma);
    }
    if ui.button("Save", true, Info::SaveTheme) {
        save_theme(ui, cfg, player);
    }
    if ui.button("Load", true, Info::LoadTheme) {
        load_theme(ui, cfg, player);
    }
    ui.end_group();

    ui.start_group();
    ui.offset_label("Font size", Info::Font);
    if ui.button("-", cfg.font_size > 0, Info::FontSize("Increase")) {
        set_font(cfg, ui, cfg.font_size - 1);
    }
    if ui.button("+", cfg.font_size < text::FONT_BYTES.len() - 1,
        Info::FontSize("Decrease")) {
        set_font(cfg, ui, cfg.font_size + 1);
    }
    ui.end_group();
}

fn color_controls(ui: &mut Ui, label: &str, accent: bool,
    get_lchuv: impl Fn(&mut Theme) -> &mut Lchuv) {
    ui.start_group();
    ui.label(label, Info::None);

    let lchuv = get_lchuv(&mut ui.style.theme);
    let (mut l, mut chroma, _) = lchuv.into_components();
    let mut hue = lchuv.hue.into_degrees();

    if !accent {
        if ui.formatted_slider(&format!("{}_l", label), "Lightness", &mut l,
            0.0..=100.0, 1, true, Info::None, |f| format!("{f:.1}"), |f| f) {
            get_lchuv(&mut ui.style.theme).l = l;
        }
    }
    if ui.formatted_slider(&format!("{}_chroma", label), "Chroma",
        &mut chroma, 0.0..=180.0, 1, true, Info::Chroma, |f| format!("{f:.1}"), |f| f) {
        get_lchuv(&mut ui.style.theme).chroma = chroma;
    }
    if ui.formatted_slider(&format!("{}_hue", label), "Hue", &mut hue,
        -180.0..=180.0, 1, true, Info::None, |f| format!("{f:.1} degrees"), |f| f) {
        get_lchuv(&mut ui.style.theme).hue = hue.into();
    }

    ui.end_group();
}

fn hotkey_controls(ui: &mut Ui, cfg: &mut Config) -> usize {
    ui.header("KEY COMMANDS", Info::None);
    ui.start_group();

    let mut id = 0;
    let mut keymap: Vec<&mut _> = cfg.iter_keymap().collect();

    // column heuristric
    let max_action_length = keymap.iter().map(|(_, a)| a.name().len()).max().unwrap();
    let entries_per_col = entries_per_col(ui, max_action_length * 2, keymap.len());

    for chunk in keymap.chunks_mut(entries_per_col) {
        ui.start_group();
        for (hotkey, action) in chunk.iter_mut() {
            ui.start_group();
            ui.hotkey_input(id, hotkey, Info::Action(*action));
            id += 1;
            ui.offset_label(action.name(), Info::Action(*action));
            ui.end_group();
        }
        ui.end_group();
    }

    ui.end_group();
    id
}

fn note_key_controls(ui: &mut Ui, cfg: &mut Config, hotkey_input_id: usize) {
    ui.header("NOTE LAYOUT", Info::NoteLayout);

    let mut hotkey_input_id = hotkey_input_id;

    for range in [17..cfg.note_keys.len(), 0..17] {
        ui.start_group();
        for (hotkey, note) in &mut cfg.note_keys[range] {
            ui.hotkey_input(hotkey_input_id, hotkey, Info::None);
            hotkey_input_id += 1;
            ui.offset_label(&note.to_string(), Info::None);
        }
        ui.end_group();

    }
}

/// Return the number of entries to use in each column, given the maximum width
/// of a column in characters.
fn entries_per_col(ui: &Ui, max_chars: usize, len: usize) -> usize {
    let char_width = ui.style.atlas.char_width();
    let cols = ui.bounds.w / (max_chars as f32 * char_width);
    (len as f32 / cols).ceil() as usize
}

/// Change the current font size.
fn set_font(cfg: &mut Config, ui: &mut Ui, size: usize) {
    if let Some(bytes) = text::FONT_BYTES.get(size) {
        let atlas = GlyphAtlas::from_bdf_bytes(bytes).unwrap();
        ui.style.margin = atlas.max_height() - atlas.cap_height();
        ui.style.atlas = atlas;
        cfg.font_size = size;
    }
}

const THEME_FILTER_NAME: &str = "Osctet theme";
const THEME_FILTER_EXT: &str = "oscthm";

/// Browse and save a theme to disk.
fn save_theme(ui: &mut Ui, cfg: &mut Config, player: &mut PlayerShell) {
    if let Some(mut path) = super::new_file_dialog(player)
        .add_filter(THEME_FILTER_NAME, &[THEME_FILTER_EXT])
        .set_directory(cfg.theme_folder.clone().unwrap_or(String::from(".")))
        .save_file() {
        path.set_extension(THEME_FILTER_EXT);
        cfg.theme_folder = config::dir_as_string(&path);
        if let Err(e) = ui.style.theme.save(path) {
            ui.report(format!("Error saving theme: {e}"));
        }
    }
}

/// Browse and load a theme from disk.
fn load_theme(ui: &mut Ui, cfg: &mut Config, player: &mut PlayerShell) {
    if let Some(path) = super::new_file_dialog(player)
        .add_filter(THEME_FILTER_NAME, &[THEME_FILTER_EXT])
        .set_directory(cfg.theme_folder.clone().unwrap_or(String::from(".")))
        .pick_file() {
        cfg.theme_folder = config::dir_as_string(&path);
        match Theme::load(path) {
            Ok(t) => ui.style.theme = Theme {
                gamma: ui.style.theme.gamma,
                ..t
            },
            Err(e) => ui.report(format!("Error loading theme: {e}")),
        }
    }
}

/// Return the names of MIDI input options.
fn input_names(input: &midir::MidiInput) -> Vec<String> {
    let mut v = vec![String::from("(none)")];
    v.extend(input.ports().into_iter()
        .map(|p| input.port_name(&p).unwrap_or(String::from("(unknown)"))));
    v
}