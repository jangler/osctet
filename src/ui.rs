use std::env::var;

use macroquad::prelude::*;

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub mouseover: Color,
    pub click: Color,
}

pub const LIGHT_THEME: Theme = Theme {
    bg: Color { r: 0.99, g: 0.99, b: 0.99, a: 1.0 },
    fg: Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 },
    mouseover: Color { r: 0.9, g: 0.9, b: 0.9, a: 1.0 },
    click: Color { r: 0.8, g: 0.8, b: 0.8, a: 1.0 },
};

pub struct UIStyle {
    pub variable_font: Font,
    pub monospace_font: Font,
    pub theme: Theme,
}

enum FontStyle {
    Variable,
    Monospace,
}

impl UIStyle {
    fn text_params(&self, font_style: FontStyle) -> TextParams {
        TextParams {
            font: Some(match font_style {
                FontStyle::Variable => &self.variable_font,
                FontStyle::Monospace => &self.monospace_font,
            }),
            font_size: 14,
            color: self.theme.fg,
            ..Default::default()
        }
    }
}

// returns true if a mouse click was released
pub fn button(label: &str, x: f32, y: f32, style: &UIStyle) -> bool {
    let margin = 5.0;
    let params = style.text_params(FontStyle::Variable);
    let (mouse_x, mouse_y) = mouse_position();
    let dim = measure_text(label, params.font, params.font_size, params.font_scale);
    let rect = Rect { x, y, w: dim.width + margin * 2.0, h: dim.height + margin * 2.0 };
    let mouse_hit = rect.contains(Vec2 { x: mouse_x, y: mouse_y });

    // draw fill based on mouse state
    if mouse_hit {
        let color = if is_mouse_button_down(MouseButton::Left) {
            style.theme.click
        } else {
            style.theme.mouseover
        };
        draw_rectangle(x, y, rect.w, rect.h, color);
    }

    draw_text_ex(label, x + margin, y + margin + dim.offset_y, params);
    draw_rectangle_lines(x, y, rect.w, rect.h, 3.0, style.theme.fg);

    mouse_hit && is_mouse_button_released(MouseButton::Left)
}