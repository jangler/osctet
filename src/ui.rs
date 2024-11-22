use std::collections::HashMap;

use macroquad::prelude::*;

use crate::MessageBuffer;

const MARGIN: f32 = 5.0;
const LINE_THICKNESS: f32 = 1.0;

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
        let h = params.font_size as f32 + MARGIN * 4.0;
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
        Rect { x, y, w: dim.width + MARGIN, h: dim.height + MARGIN }
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
                self.style.theme.mouseover
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
    pub fn combo_box(&mut self, label: &str, options: &Vec<String>) -> Option<usize> {
        let k = label.to_owned();
        let state = self.combo_boxes.entry(k.clone()).or_insert(Default::default());
        let button_text = format!("{} v", options.get(state.index).unwrap_or(&"".to_owned())); // nasty!
        let (rect, event) = self.text_rect(&button_text, self.cursor_x, self.cursor_y);
        let label_dim = draw_text_ex(label,
            self.cursor_x + rect.w + MARGIN,
            self.cursor_y,
            self.style.text_params());

        if event == MouseEvent::Pressed {
            self.combo_boxes.entry(k.clone()).and_modify(|x| x.open = !x.open);
        }


        if self.combo_boxes.get(&k).is_some_and(|x| x.open) {
            // draw text
            for (i, option) in options.iter().enumerate() {
                self.text_rect(option, self.cursor_x, self.cursor_y + (i as f32 * self.style.text_params().font_size as f32));
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
            LINE_THICKNESS * 2.0, self.style.theme.fg);
    }
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

// second return value is whether to close the dialog
pub fn choice_dialog(style: &UIStyle, title: &str, choices: &[String]) -> (Option<usize>, bool) {
    let params = style.text_params();
    let mut strings = vec![
        title.to_string(),
        "".to_string(), // empty line between title & choices
    ];
    strings.append(&mut choices.to_vec());
    let mut r = center(fit_strings(params.clone(), &strings));
    draw_rectangle(r.x, r.y, r.w, r.h, style.theme.bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, LINE_THICKNESS * 2.0, style.theme.fg);

    let click = is_mouse_button_released(MouseButton::Left);
    let mouse_pos = {
        let (x, y) = mouse_position();
        Vec2 { x, y }
    };
    if click && !r.contains(mouse_pos) {
        return (None, true)
    }

    draw_text_topleft(params.clone(), title, r.x, r.y);
    r.y += params.font_size as f32;
    for (i, choice) in choices.iter().enumerate() {
        r.y += params.font_size as f32;
        let dim = draw_text_topleft(params.clone(), choice, r.x, r.y);
        if dim.contains(mouse_pos) {
            if click {
                return (Some(i), true)
            }
        }
    } 
    (None, false)
}

fn fit_strings(params: TextParams, v: &[String]) -> Rect {
    let mut rect: Rect = Default::default();
    for (i, s) in v.iter().enumerate() {
        let dim = measure_text(s, params.font, params.font_size, params.font_scale);
        rect.w = rect.w.max(dim.width + MARGIN * 2.0);
        rect.h = rect.h.max(dim.height * (i + 1) as f32 + MARGIN * 2.0);
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