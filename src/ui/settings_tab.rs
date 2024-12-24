use palette::Lchuv;

use crate::config::Config;

use super::{text::{self, GlyphAtlas}, theme::Theme, Layout, UI};

pub fn draw(ui: &mut UI, cfg: &mut Config, scroll: &mut f32) {
    ui.layout = Layout::Horizontal;
    let old_y = ui.cursor_y;
    ui.cursor_y -= *scroll;
    ui.cursor_z -= 1;
    ui.start_group();

    if ui.button("Reset to defaults", true) {
        *cfg = Default::default();
    }

    ui.space(2.0);
    ui.header("APPEARANCE");

    // TODO: currently accent 2 isn't used on this page, so there's no way to
    //       see the effects of adjusting it
    ui.start_group();
    color_controls(ui, "Foreground", false, |t| &mut t.fg);
    color_controls(ui, "Background", false, |t| &mut t.bg);
    color_controls(ui, "Accent 1", true, |t| &mut t.accent1);
    color_controls(ui, "Accent 2", true, |t| &mut t.accent2);
    ui.end_group();

    ui.start_group();
    if ui.button("Reset (light)", true) {
        ui.style.theme = Theme::light();
    }
    if ui.button("Reset (dark)", true) {
        ui.style.theme = Theme::dark();
    }
    ui.end_group();

    ui.start_group();
    ui.offset_label("Font size");
    if ui.button("-", cfg.font_size > 0) {
        set_font(cfg, ui, cfg.font_size - 1);
    }
    if ui.button("+", cfg.font_size < text::FONT_BYTES.len() - 1) {
        set_font(cfg, ui, cfg.font_size + 1);
    }
    ui.end_group();

    ui.space(2.0);
    let id = hotkey_controls(ui, cfg);

    ui.space(2.0);
    note_key_controls(ui, cfg, id);

    // TODO: duplication with instruments tab scroll code
    let scroll_h = ui.end_group().unwrap().h + ui.style.margin;
    ui.cursor_z += 1;
    ui.cursor_y = old_y;
    ui.vertical_scrollbar(scroll, scroll_h, ui.bounds.y + ui.bounds.h - ui.cursor_y, true);
}

fn color_controls(ui: &mut UI, label: &str, accent: bool,
    f: impl Fn(&mut Theme) -> &mut Lchuv) {
    ui.start_group();
    ui.label(label);

    let lchuv = f(&mut ui.style.theme);
    let (mut l, mut chroma, _) = lchuv.into_components();
    let mut hue = lchuv.hue.into_degrees();

    if !accent {
        if ui.slider(&format!("{}_l", label), "Lightness", &mut l,
            0.0..=100.0, None, 1, true) {
            f(&mut ui.style.theme).l = l;
        }
    }
    if ui.slider(&format!("{}_chroma", label), "Chroma",
        &mut chroma, 0.0..=180.0, None, 1, true) {
        f(&mut ui.style.theme).chroma = chroma;
    }
    if ui.slider(&format!("{}_hue", label), "Hue", &mut hue,
        -180.0..=180.0, Some("degrees"), 1, true) {
        f(&mut ui.style.theme).hue = hue.into();
    }

    ui.end_group();
}

fn hotkey_controls(ui: &mut UI, cfg: &mut Config) -> usize {
    ui.header("KEY COMMANDS");
    ui.start_group();

    let mut id = 0;
    let mut changed = false;
    let mut keymap: Vec<&mut _> = cfg.iter_keymap().collect();

    // column heuristric
    let max_action_length = keymap.iter().map(|(_, a)| a.name().len()).max().unwrap();
    let entries_per_col = entries_per_col(ui, max_action_length * 2, keymap.len());

    for chunk in keymap.chunks_mut(entries_per_col) {
        ui.start_group();
        for (_, action) in chunk.iter() {
            ui.offset_label(action.name());
        }
        ui.align_right(chunk.len());
        ui.end_group();

        ui.start_group();
        for (hotkey, _) in chunk.iter_mut() {
            if ui.hotkey_input(id, hotkey) {
                changed = true;
            }
            id += 1;
        }
        ui.end_group();
    }

    if changed {
        cfg.update_hotkeys();
    }

    ui.end_group();
    id
}

fn note_key_controls(ui: &mut UI, cfg: &mut Config, hotkey_input_id: usize) {
    ui.header("NOTE LAYOUT");
    ui.start_group();

    let mut hotkey_input_id = hotkey_input_id;
    let max_chars = cfg.note_keys.iter().map(|(k, _)| k.to_string().len()).max().unwrap();
    let entries_per_col = entries_per_col(ui, max_chars * 2, cfg.note_keys.len());

    for chunk in cfg.note_keys.chunks_mut(entries_per_col) {
        // TODO: duplication with hotkey_controls
        ui.start_group();
        for (_, note) in chunk.iter() {
            ui.offset_label(&note.to_string());
        }
        ui.align_right(chunk.len());
        ui.end_group();

        ui.start_group();
        for (hotkey, _) in chunk.iter_mut() {
            ui.hotkey_input(hotkey_input_id, hotkey);
            hotkey_input_id += 1;
        }
        ui.end_group();
    }

    ui.end_group();
}

fn entries_per_col(ui: &UI, max_chars: usize, len: usize) -> usize {
    let char_width = ui.style.atlas.char_width();
    let cols = (ui.bounds.w / (max_chars as f32 * char_width)) as usize;
    (len as f32 / cols as f32).ceil() as usize
}

fn set_font(cfg: &mut Config, ui: &mut UI, size: usize) {
    if let Some(bytes) = text::FONT_BYTES.get(size) {
        let atlas = GlyphAtlas::from_bdf_bytes(bytes).unwrap();
        ui.style.margin = atlas.max_height() - atlas.cap_height();
        ui.style.atlas = atlas;
        cfg.font_size = size;
    }
}