use std::collections::HashMap;

use macroquad::prelude::*;

use crate::MessageBuffer;

const MARGIN: f32 = 5.0;
const LINE_THICKNESS: f32 = 3.0;

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

#[derive(PartialEq, Debug)]
enum MouseEvent {
    None,
    Pressed,
    Released,
}

#[derive(PartialEq)]
enum Layout {
    Vertical,
    Horizontal,
}

struct ComboBoxState {
    open: bool,
    index: usize,
}

impl Default for ComboBoxState {
    fn default() -> Self {
        Self { open: false, index: 0, }
    }
}

pub struct UI {
    pub style: UIStyle,
    combo_boxes: HashMap<String, ComboBoxState>,
    bounds: Rect,
    cursor_x: f32,
    cursor_y: f32,
    layout: Layout,
}

impl UI {
    pub fn new() -> Self {
        Self {
            style: UIStyle {
                variable_font: load_ttf_font_from_bytes(include_bytes!("font/Roboto-Regular.ttf"))
                    .expect("included font should be loadable"),
                monospace_font: load_ttf_font_from_bytes(include_bytes!("font/RobotoMono-Regular.ttf"))
                    .expect("included font should be loadable"),
                theme: LIGHT_THEME,
            },
            combo_boxes: HashMap::new(),
            bounds: Default::default(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            layout: Layout::Vertical,
        }
    }

    pub fn new_frame(&mut self) {
        self.bounds = Rect {
            x: 0.0,
            y: 0.0,
            w: screen_width(),
            h: screen_height(),
        };
        self.cursor_x = MARGIN;
        self.cursor_y = MARGIN;
        
        clear_background(self.style.theme.bg);
    }

    pub fn start_bottom_panel(&mut self) {
        let params = self.style.text_params(FontStyle::Variable);
        let h = params.font_size as f32 + MARGIN * 2.0;
        draw_line(self.bounds.x, self.bounds.h - h,
            self.bounds.x + self.bounds.w, self.bounds.h - h,
            LINE_THICKNESS, self.style.theme.fg);
        self.layout = Layout::Horizontal;
        self.cursor_x = self.bounds.x;
        self.cursor_y = self.bounds.h - h;
    }

    pub fn end_bottom_panel(&mut self) {
        let params = self.style.text_params(FontStyle::Variable);
        let h = params.font_size as f32 + MARGIN * 2.0;
        self.bounds.h -= h;
        self.cursor_x = self.bounds.x;
        self.cursor_y = self.bounds.y;
    }

    fn update_cursor(&mut self, w: f32, h: f32) {
        match self.layout {
            Layout::Horizontal => self.cursor_x += w,
            Layout::Vertical => self.cursor_y += h,
        }
    }

    fn draw_text(&self, label: &str, x: f32, y: f32) -> Rect {
        let params = self.style.text_params(FontStyle::Variable);
        let dim = measure_text(label, params.font, params.font_size, params.font_scale);
        draw_text_ex(label, x + MARGIN, y + MARGIN + dim.offset_y, params);
        Rect { x, y, w: dim.width + MARGIN, h: dim.height + MARGIN }
    }
    
    pub fn label(&mut self, label: &str) {
        let rect = self.draw_text(label, self.cursor_x, self.cursor_y);
        self.update_cursor(rect.w, rect.h);
    }

    fn text_rect(&self, label: &str, x: f32, y: f32) -> (Rect, MouseEvent) {
        let params = self.style.text_params(FontStyle::Variable);
        let (mouse_x, mouse_y) = mouse_position();
        let dim = measure_text(label, params.font, params.font_size, params.font_scale);
        let rect = Rect { x, y, w: dim.width + MARGIN * 2.0, h: dim.height + MARGIN * 2.0 };
        let mouse_hit = rect.contains(Vec2 { x: mouse_x, y: mouse_y });
    
        // draw fill based on mouse state
        if mouse_hit {
            let color = if is_mouse_button_down(MouseButton::Left) {
                self.style.theme.click
            } else {
                self.style.theme.mouseover
            };
            draw_rectangle(x, y, rect.w, rect.h, color);
        }
    
        draw_text_ex(label, x + MARGIN, y + MARGIN + dim.offset_y, params);
        draw_rectangle_lines(x, y, rect.w, rect.h, LINE_THICKNESS, self.style.theme.fg);
    
        (rect, if mouse_hit && is_mouse_button_pressed(MouseButton::Left) {
            MouseEvent::Pressed
        } else if mouse_hit && is_mouse_button_released(MouseButton::Left) {
            MouseEvent::Released
        } else {
            MouseEvent::None
        })
    }

    /// Draws a button and returns true if it was clicked this frame.
    pub fn button(&mut self, label: &str) -> bool {
        let (rect, event) = self.text_rect(label, self.cursor_x + MARGIN, self.cursor_y + MARGIN);
        self.update_cursor(rect.w + MARGIN * 2.0, rect.h + MARGIN * 2.0);
        event == MouseEvent::Released
    }
    
    /// Draws a combo box. If a value was selected this frame, returns the value's index.
    pub fn combo_box(&mut self, label: &str, options: &Vec<String>) -> Option<usize> {
        let k = label.to_owned();
        let state = self.combo_boxes.entry(k.clone()).or_insert(Default::default());
        let button_text = format!("{} v", options.get(state.index).unwrap_or(&"".to_owned())); // nasty!
        let (rect, event) = self.text_rect(&button_text, self.cursor_x, self.cursor_y);
        let label_dim = draw_text_ex(label,
            self.cursor_x + rect.w + MARGIN,
            self.cursor_y,
            self.style.text_params(FontStyle::Variable));

        if event == MouseEvent::Pressed {
            self.combo_boxes.entry(k.clone()).and_modify(|x| x.open = !x.open);
        }


        if self.combo_boxes.get(&k).is_some_and(|x| x.open) {
            // draw text
            for (i, option) in options.iter().enumerate() {
                self.text_rect(option, self.cursor_x, self.cursor_y + (i as f32 * self.style.text_params(FontStyle::Variable).font_size as f32));
            }
        }

        self.update_cursor(rect.w.max(label_dim.width) + MARGIN, rect.h + MARGIN);
    
        None
    }

    pub fn message_buffer(&mut self, messages: &MessageBuffer, width: f32) {
        let init_y = self.cursor_y;
        for msg in messages.iter() {
            let rect = self.draw_text(msg, self.cursor_x, self.cursor_y);
            self.update_cursor(0.0, rect.h);
        }
        self.update_cursor(0.0, MARGIN);
        draw_rectangle_lines(
            self.cursor_x, init_y,
            width, self.cursor_y - init_y,
            LINE_THICKNESS, self.style.theme.fg);
    }
}