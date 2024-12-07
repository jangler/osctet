use std::num::ParseIntError;

use macroquad::color::Color;

use super::{Layout, Theme, DARK_THEME, LIGHT_THEME, UI};

pub fn draw(ui: &mut UI) {
    ui.layout = Layout::Vertical;
    ui.header("COLOR THEME");
    color_box(ui, "Foreground", |t| &mut t.fg);
    color_box(ui, "Background (panel)", |t| &mut t.panel_bg);
    color_box(ui, "Background (content)", |t| &mut t.content_bg);
    color_box(ui, "Background (control)", |t| &mut t.control_bg);
    color_box(ui, "Border (unfocused)", |t| &mut t.border_unfocused);
    color_box(ui, "Border (focused)", |t| &mut t.border_focused);
    color_box(ui, "Note column", |t| &mut t.column[0]);
    color_box(ui, "Pressure column", |t| &mut t.column[1]);
    color_box(ui, "Modulation column", |t| &mut t.column[2]);
    
    if ui.button("Use light theme") {
        ui.style.theme = LIGHT_THEME;
    }
    if ui.button("Use dark theme") {
        ui.style.theme = DARK_THEME;
    }
}

fn color_box(ui: &mut UI, label: &str, f: impl Fn(&mut Theme) -> &mut Color) {
    let s = string_from_color(*f(&mut ui.style.theme));
    if let Some(s) = ui.edit_box(label, 8, s) {
        match color_from_string(&s) {
            Ok(c) => *f(&mut ui.style.theme) = c,
            Err(e) => ui.report(e),
        }
    }
}

fn string_from_color(c: Color) -> String {
    format!("{:02x}{:02x}{:02x}",
        (c.r*255.0) as u8,
        (c.g*255.0) as u8,
        (c.b*255.0) as u8)
}

fn color_from_string(s: &str) -> Result<Color, ParseIntError> {
    u32::from_str_radix(s, 16).map(|x| Color::from_hex(x))
}