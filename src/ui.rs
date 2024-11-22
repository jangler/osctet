use std::collections::HashMap;

use macroquad::prelude::*;

use crate::MessageBuffer;

const MARGIN: f32 = 5.0;
const LINE_THICKNESS: f32 = 1.0;

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub hover: Color,
    pub click: Color,
}

pub const LIGHT_THEME: Theme = Theme {
    bg: Color { r: 0.99, g: 0.99, b: 0.99, a: 1.0 },
    fg: Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 },
    hover: Color { r: 0.95, g: 0.95, b: 0.95, a: 1.0 },
    click: Color { r: 0.9, g: 0.9, b: 0.9, a: 1.0 },
};

pub struct UIStyle {
    pub font: Font,
    pub theme: Theme,
}

impl UIStyle {
    pub fn text_params(&self) -> TextParams {
        TextParams {
            font: Some(&self.font),
            font_size: 16,
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
    options: Vec<String>,
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
                font: load_ttf_font_from_bytes(include_bytes!("font/ProggyClean.ttf"))
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
        let params = self.style.text_params();
        let h = params.font_size as f32 + MARGIN * 3.0;
        draw_line(self.bounds.x, self.bounds.h - h + 0.5,
            self.bounds.x + self.bounds.w, self.bounds.h - h + 0.5,
            LINE_THICKNESS, self.style.theme.fg);
        self.layout = Layout::Horizontal;
        self.cursor_x = self.bounds.x;
        self.cursor_y = self.bounds.h - h;
    }

    pub fn end_bottom_panel(&mut self) {
        let params = self.style.text_params();
        let h = params.font_size as f32 + MARGIN * 4.0;
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
        let params = self.style.text_params();
        let dim = measure_text(label, params.font, params.font_size, params.font_scale);
        draw_text_ex(label, x + MARGIN, y + MARGIN + dim.offset_y, params);
        Rect { x, y, w: dim.width + MARGIN * 2.0, h: dim.height + MARGIN * 2.0 }
    }
    
    pub fn label(&mut self, label: &str) {
        let rect = self.draw_text(label, self.cursor_x, self.cursor_y + MARGIN);
        self.update_cursor(rect.w, rect.h + MARGIN);
    }

    fn text_rect(&self, label: &str, x: f32, y: f32) -> (Rect, MouseEvent) {
        let params = self.style.text_params();
        let (mouse_x, mouse_y) = mouse_position();
        let dim = measure_text(label, params.font, params.font_size, params.font_scale);
        let rect = Rect { x, y, w: dim.width + MARGIN * 2.0, h: dim.height + MARGIN * 2.0 };
        let mouse_hit = rect.contains(Vec2 { x: mouse_x, y: mouse_y });
    
        // draw fill based on mouse state
        if mouse_hit {
            let color = if is_mouse_button_down(MouseButton::Left) {
                self.style.theme.click
            } else {
                self.style.theme.hover
            };
            draw_rectangle(x, y, rect.w, rect.h, color);
        }
    
        draw_text_ex(label, (x + MARGIN).round(), (y + MARGIN + dim.offset_y).round(), params);
        draw_rectangle_lines(x.round(), y.round(), rect.w.round(), rect.h.round(),
            LINE_THICKNESS * 2.0, self.style.theme.fg);
    
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
    pub fn combo_box(&mut self, label: &str, button_text: &str, get_options: impl Fn() -> Vec<String>) -> Option<usize> {
        // draw button and label
        let (button_rect, event) = self.text_rect(&button_text, self.cursor_x + MARGIN, self.cursor_y + MARGIN);
        let label_dim = self.draw_text(label, self.cursor_x + button_rect.w + MARGIN, self.cursor_y + MARGIN);

        // check to open list
        let open = self.combo_boxes.contains_key(label);
        if event == MouseEvent::Pressed && !open {
            self.combo_boxes.insert(label.to_owned(), ComboBoxState {
                options: get_options(),
            });
        }

        let lmb_press = is_mouse_button_pressed(MouseButton::Left);
        let mut return_val = None;

        if let Some(state) = self.combo_boxes.get(label) {
            // should options be drawn above or below the button?
            let y_direction = if self.cursor_y > screen_height() / 2.0 {
                -1.0
            } else {
                1.0
            };

            // draw bounding box
            let w = fit_strings(self.style.text_params(), &state.options).w;
            let h = button_rect.h * state.options.len() as f32 + 2.0;
            let bg_rect = Rect {
                x: button_rect.x,
                y: if y_direction < 0.0 {
                    button_rect.y - h + 1.0
                } else {
                    button_rect.y + button_rect.h - 1.0
                },
                w,
                h,
            };
            draw_rectangle(bg_rect.x, bg_rect.y, bg_rect.w, bg_rect.h, self.style.theme.bg);
            draw_rectangle_lines(bg_rect.x, bg_rect.y, bg_rect.w, bg_rect.h, LINE_THICKNESS * 2.0, self.style.theme.fg);

            // draw options
            let mut hit_rect = Rect {
                x: bg_rect.x + 1.0,
                y: bg_rect.y + 1.0,
                w: bg_rect.w - 2.0,
                h: button_rect.h,
            };
            let mouse_pos = mouse_position_vec2();
            for (i, option) in state.options.iter().enumerate() {
                if hit_rect.contains(mouse_pos) {
                    draw_rectangle(hit_rect.x, hit_rect.y, hit_rect.w, hit_rect.h, self.style.theme.hover);
                    if lmb_press {
                        return_val = Some(i)
                    }
                }
                self.draw_text(&option, hit_rect.x, hit_rect.y);
                hit_rect.y += hit_rect.h;
            }
        }

        // check to close list
        if open && (lmb_press || is_key_pressed(KeyCode::Escape)) {
            self.combo_boxes.remove(label);
        }

        self.update_cursor(button_rect.w + label_dim.w + MARGIN, button_rect.h + MARGIN);
    
        return_val
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
            LINE_THICKNESS * 2.0, self.style.theme.fg);
    }
}

fn mouse_position_vec2() -> Vec2 {
    let (x, y) = mouse_position();
    Vec2 { x, y }
}

pub fn alert_dialog(style: &UIStyle, message: &str) -> bool {
    let params = style.text_params();
    let r = center(fit_strings(params.clone(), &[message.to_owned()]));
    draw_rectangle(r.x, r.y, r.w, r.h, style.theme.bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, LINE_THICKNESS * 2.0, style.theme.fg);
    draw_text_topleft(params.clone(), message, r.x, r.y);

    let click = is_mouse_button_released(MouseButton::Left);
    let mouse_pos = {
        let (x, y) = mouse_position();
        Vec2 { x, y }
    };
    click && !r.contains(mouse_pos)
}

fn fit_strings(params: TextParams, v: &[String]) -> Rect {
    let mut rect: Rect = Default::default();
    rect.h += MARGIN;
    for s in v {
        let dim = measure_text(s, params.font, params.font_size, params.font_scale);
        rect.w = rect.w.max(dim.width + MARGIN * 2.0);
        rect.h += dim.height + MARGIN * 2.0;
    }
    rect
}

fn center(r: Rect) -> Rect {
    Rect {
        x: (screen_width() / 2.0 - r.w / 2.0).round(),
        y: (screen_height() / 2.0 - r.h / 2.0).round(),
        ..r
    }
}

fn draw_text_topleft(params: TextParams, label: &str, x: f32, y: f32) -> Rect {
    let dim = measure_text(label, params.font, params.font_size, params.font_scale);
    draw_text_ex(label, (x + MARGIN).round(), (y + MARGIN + dim.offset_y).round(), params);
    Rect { x, y, w: dim.width + MARGIN, h: dim.height + MARGIN }
}