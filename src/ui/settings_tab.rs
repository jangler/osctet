use std::num::ParseIntError;

use macroquad::color::Color;

use super::{Layout, Theme, UI};

pub fn draw(ui: &mut UI) {
    ui.layout = Layout::Vertical;
    color_box(ui, "Panel background", |t| &mut t.bg);
    color_box(ui, "Panel foreground", |t| &mut t.fg);
    color_box(ui, "Content background", |t| &mut t.content_bg);
    color_box(ui, "Content foreground", |t| &mut t.content_fg);
    color_box(ui, "Control background", |t| &mut t.control_bg);
    color_box(ui, "Control foreground", |t| &mut t.control_fg);
    color_box(ui, "Note column", |t| &mut t.column[0]);
    color_box(ui, "Pressure column", |t| &mut t.column[1]);
    color_box(ui, "Modulation column", |t| &mut t.column[2]);
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