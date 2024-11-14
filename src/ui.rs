use macroquad::prelude::*;

use crate::Theme;

pub struct UIStyle {
    pub font: Font,
    pub theme: Theme,
}

impl UIStyle {
    fn text_params(&self) -> TextParams {
        TextParams {
            font: Some(&self.font),
            font_size: 14,
            color: self.theme.fg,
            ..Default::default()
        }
    }
}

pub fn draw_button(label: &str, x: f32, y: f32, style: &UIStyle) {
    let margin = 5.0;
    let params = style.text_params();
    let dim = draw_text_ex(label,
        (x + margin).round(),
        (y + margin + params.font_size as f32 * 0.75).round(),
        params);
    draw_rectangle_lines(x, y,
        dim.width.round() + margin * 2.0,
        dim.height.round() + margin * 2.0,
        3.0, style.theme.fg);
}