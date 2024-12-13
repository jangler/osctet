use palette::Lchuv;

use crate::config::Config;

use super::{text_width, theme::Theme, Layout, MARGIN, UI};

pub fn draw(ui: &mut UI, cfg: &mut Config, scroll: &mut f32) {
    ui.layout = Layout::Horizontal;
    let old_y = ui.cursor_y;
    ui.cursor_y -= *scroll;
    ui.cursor_z -= 1;
    ui.start_group();

    if ui.button("Save settings") {
        cfg.theme = Some(ui.style.theme.clone());
        match cfg.save() {
            Ok(()) => ui.notify(String::from("Saved settings.")),
            Err(e) => ui.report(e),
        }
    }

    ui.space(2.0);
    ui.header("COLOR THEME");

    // TODO: currently accent 2 isn't used on this page, so there's no way to
    //       see the effects of adjusting it
    ui.start_group();
    color_controls(ui, "Foreground", false, |t| &mut t.fg);
    color_controls(ui, "Background", false, |t| &mut t.bg);
    color_controls(ui, "Accent 1", true, |t| &mut t.accent1);
    color_controls(ui, "Accent 2", true, |t| &mut t.accent2);
    ui.end_group();
    
    ui.start_group();
    if ui.button("Reset (light)") {
        ui.style.theme = Theme::light();
    }
    if ui.button("Reset (dark)") {
        ui.style.theme = Theme::dark();
    }
    ui.end_group();

    ui.space(2.0);
    hotkey_controls(ui, cfg);

    // TODO: duplication with instruments tab scroll code
    let scroll_h = ui.end_group().unwrap().h + MARGIN;
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
        if ui.slider(&format!("{}_l", label), "Lightness", &mut l, 0.0..=100.0, None, 1) {
            f(&mut ui.style.theme).l = l;
        }
    }
    if ui.slider(&format!("{}_chroma", label), "Chroma", &mut chroma, 0.0..=180.0, None, 1) {
        f(&mut ui.style.theme).chroma = chroma;
    }
    if ui.slider(&format!("{}_hue", label), "Hue", &mut hue, -180.0..=180.0,
        Some("degrees"), 1) {
        f(&mut ui.style.theme).hue = hue.into();
    }

    ui.end_group();
}

fn hotkey_controls(ui: &mut UI, cfg: &mut Config) {
    ui.header("KEYS");
    ui.start_group();
    
    let mut id = 0;
    let mut changed = false;
    let mut keymap: Vec<&mut _> = cfg.iter_keymap().collect();

    // column heuristric
    let max_action_length = keymap.iter().map(|(_, a)| a.name().len()).max().unwrap();
    let char_width = text_width("x", &ui.style.text_params());
    let cols = (ui.bounds.w / (max_action_length as f32 * char_width * 2.0)) as usize;
    let entries_per_col = (keymap.len() as f32 / cols as f32).ceil() as usize;

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
}