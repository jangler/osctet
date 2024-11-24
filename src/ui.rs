//! Basic immediate-mode GUI library implemented on top of macroquad.
//!
//! Not polished for general reuse. Macroquad also has its own built-in UI
//! library, but the demos don't give me much faith in it.

use std::{collections::HashMap, ops::RangeInclusive};

use fundsp::shared::Shared;
use macroquad::prelude::*;

const MARGIN: f32 = 5.0;
const LINE_THICKNESS: f32 = 1.0;

/// Draws text with the top-left corner at (x, y), plus margins.
/// Returns the bounds of the text, plus margins.
fn draw_text_topleft(params: TextParams, label: &str, x: f32, y: f32) -> Rect {
    let dim = draw_text_ex(label,
        (x + MARGIN).round(),
        (y + MARGIN + cap_height(&params)).round(),
        params);
    Rect { x, y, w: dim.width + MARGIN * 2.0, h: dim.height + MARGIN * 2.0 }
}

/// Returns the height of a capital letter.
fn cap_height(params: &TextParams) -> f32 {
    measure_text("X", params.font, params.font_size, params.font_scale).offset_y
}

/// Returns the width of rendered text.
fn text_width(text: &str, params: &TextParams) -> f32 {
    measure_text(text, params.font, params.font_size, params.font_scale).width
}

/// Returns mouse position as a `Vec2`.
fn mouse_position_vec2() -> Vec2 {
    let (x, y) = mouse_position();
    Vec2 { x, y }
}

/// Draw a rectangle with fill and stroke colors.
fn draw_filled_rect(r: Rect, fill: Color, stroke: Color) {
    draw_rectangle(r.x, r.y, r.w, r.h, fill);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, LINE_THICKNESS * 2.0, stroke);
}

/// Color theme.
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

/// UI style, including font and color theme.
pub struct Style {
    pub font: Font,
    pub theme: Theme,
}

impl Style {
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
pub enum Layout {
    Vertical,
    Horizontal,
}

struct ComboBoxState {
    id: String,
    options: Vec<String>,
    button_rect: Rect,
    list_rect: Rect,
}

enum Graphic {
    Rect(Rect, Color, Option<Color>),
    Line(f32, f32, f32, f32, Color),
    Text(f32, f32, String, Color),
}

impl Graphic {
    fn draw(&self, params: &TextParams) {
        match self {
            Self::Rect(rect, fill, stroke) => {
                if let Some(stroke) = stroke {
                    draw_filled_rect(*rect, *fill, *stroke);
                } else {
                    draw_rectangle(rect.x, rect.y, rect.w, rect.h, *fill);
                }
            },
            Self::Line(x1, y1, x2, y2, color) => {
                draw_line(*x1, *y1, *x2, *y2, LINE_THICKNESS, *color);
            },
            Self::Text(x, y, text, color) => {
                let params = TextParams {
                    color: *color,
                    ..*params
                };
                draw_text_topleft(params, text, *x, *y);
            }
        }
    }
}

struct DrawOp {
    z: i8,
    graphic: Graphic,
}

/// Draws widgets and tracks UI state.
pub struct UI {
    pub style: Style,
    open_combo_box: Option<ComboBoxState>,
    tabs: HashMap<String, usize>,
    grabbed_slider: Option<String>,
    bounds: Rect,
    cursor_x: f32,
    cursor_y: f32,
    cursor_z: i8,
    draw_queue: Vec<DrawOp>,
    pub layout: Layout,
}

impl UI {
    pub fn new() -> Self {
        Self {
            style: Style {
                font: load_ttf_font_from_bytes(include_bytes!("font/ProggyClean.ttf"))
                    .expect("included font should be loadable"),
                theme: LIGHT_THEME,
            },
            open_combo_box: None,
            tabs: HashMap::new(),
            grabbed_slider: None,
            bounds: Default::default(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_z: 0,
            layout: Layout::Vertical,
            draw_queue: Vec::new(),
        }
    }

    pub fn start_frame(&mut self) {
        self.bounds = Rect {
            x: 0.0,
            y: 0.0,
            w: screen_width(),
            h: screen_height(),
        };

        self.cursor_x = MARGIN;
        self.cursor_y = MARGIN;
        self.cursor_z = 0;

        if self.grabbed_slider.is_some() && is_mouse_button_released(MouseButton::Left) {
            self.grabbed_slider = None;
        }
        
        clear_background(self.style.theme.bg);
    }

    pub fn end_frame(&mut self) {
        let params = self.style.text_params();
        self.draw_queue.sort_by_key(|x| x.z);
        for op in &self.draw_queue {
            op.graphic.draw(&params);
        }
        self.draw_queue.clear();
    }

    fn push_graphic(&mut self, graphic: Graphic) {
        self.draw_queue.push(DrawOp {
            z: self.cursor_z,
            graphic,
        })
    }

    fn push_graphics(&mut self, gfx: Vec<Graphic>) {   
        for gfx in gfx {
            self.push_graphic(gfx);
        }
    }

    fn push_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color) {
        self.push_graphic(Graphic::Line(x1, y1, x2, y2, color));
    }

    fn push_rect(&mut self, rect: Rect, fill: Color, stroke: Option<Color>) {
        self.push_graphic(Graphic::Rect(rect, fill, stroke));
    }

    fn push_text(&mut self, x: f32, y: f32, text: String, color: Color) -> Rect {
        let params = self.style.text_params();
        let rect = Rect {
            x,
            y,
            w: text_width(&text, &params) + MARGIN * 2.0,
            h: cap_height(&params) + MARGIN * 2.0,
        };
        self.push_graphic(Graphic::Text(x, y, text, color));
        rect
    }

    fn bottom_panel_height(&self) -> f32 {
        cap_height(&self.style.text_params()) + MARGIN * 4.0
    }

    pub fn start_bottom_panel(&mut self) {
        let h = self.bottom_panel_height();
        self.cursor_z = 1;
        self.push_rect(Rect {
            y: self.bounds.h - h,
            h, 
            ..self.bounds
        }, self.style.theme.bg, None);
        self.push_line(self.bounds.x, self.bounds.h - h + 0.5,
            self.bounds.x + self.bounds.w, self.bounds.h - h + 0.5,
            self.style.theme.fg);
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
        self.cursor_z = 0;
    }

    fn update_cursor(&mut self, w: f32, h: f32) {
        match self.layout {
            Layout::Horizontal => self.cursor_x += w,
            Layout::Vertical => self.cursor_y += h,
        }
    }

    /// Check whether the mouse is within the rect and unoccluded.
    fn mouse_hits(&self, rect: Rect) -> bool {
        let pt = mouse_position_vec2();
        // TODO: account for dialogs
        if let Some(state) = &self.open_combo_box {
            if state.list_rect.contains(pt) {
                return false
            }
        }
        if self.cursor_z < 1 && screen_height() - self.bottom_panel_height() < pt.y {
            return false
        }
        rect.contains(pt)
    }
    
    pub fn label(&mut self, label: &str) {
        let rect = self.push_text(self.cursor_x, self.cursor_y,
            label.to_owned(), self.style.theme.fg);
        self.update_cursor(rect.w, rect.h);
    }

    fn text_rect(&mut self, label: &str, x: f32, y: f32) -> (Rect, MouseEvent) {
        let params = self.style.text_params();
        let rect = Rect {
            x,
            y,
            w: text_width(label, &params) + MARGIN * 2.0,
            h: cap_height(&params) + MARGIN * 2.0,
        };
        let mouse_hit = self.mouse_hits(rect);
    
        // draw fill based on mouse state
        let fill = if mouse_hit {
            if is_mouse_button_down(MouseButton::Left) {
                self.style.theme.click
            } else {
                self.style.theme.hover
            }
        } else {
            self.style.theme.bg
        };

        self.push_rect(rect, fill, Some(self.style.theme.fg));
        self.push_text(x, y, label.to_owned(), self.style.theme.fg);
    
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

    /// Draws a checkbox and returns true if it was changed this frame.
    pub fn checkbox(&mut self, label: &str, value: &mut bool) -> bool {
        let button_text = if *value { "X" } else { " " };
        let clicked = self.button(button_text);
        self.cursor_x -= MARGIN;
        let label_rect = self.push_text(self.cursor_x, self.cursor_y + MARGIN,
            label.to_owned(), self.style.theme.fg);
        self.update_cursor(label_rect.w, label_rect.h);
        if clicked {
            *value = !*value;
        }
        clicked
    }
    
    /// Draws a combo box. If a value was selected this frame, returns the value's index.
    pub fn combo_box(&mut self, id: &str, label: &str, button_text: &str,
        get_options: impl Fn() -> Vec<String>
    ) -> Option<usize> {
        // draw button and label
        let (button_rect, event) = self.text_rect(&button_text, self.cursor_x + MARGIN, self.cursor_y + MARGIN);
        let label_dim = self.push_text(self.cursor_x + button_rect.w + MARGIN,
            self.cursor_y + MARGIN, label.to_owned(), self.style.theme.fg);
        let params = self.style.text_params();

        // check to open list
        let open = self.open_combo_box.as_ref().is_some_and(|x| x.id == id);
        if event == MouseEvent::Pressed && !open {
            let options = get_options();
            let list_rect = combo_box_list_rect(&params, button_rect, &options);
            self.open_combo_box = Some(ComboBoxState {
                id: id.to_owned(),
                options,
                button_rect,
                list_rect,
            });
        }

        let return_val = if open {
            if let Some(state) = &mut self.open_combo_box {
                state.button_rect = button_rect;
                state.list_rect = combo_box_list_rect(&params, button_rect, &state.options);
            }
            self.combo_box_list(open)
        } else {
            None
        };

        // check to close. other close conditions are in combo_box_list()
        if open && (is_key_pressed(KeyCode::Escape) ||
            (is_mouse_button_pressed(MouseButton::Left)
                && button_rect.contains(mouse_position_vec2()))
        ) {
            self.open_combo_box = None;
        }

        self.update_cursor(button_rect.w + label_dim.w + MARGIN, button_rect.h + MARGIN);
    
        return_val
    }

    /// Draw the list of the active combo box.
    fn combo_box_list(&mut self, already_open: bool) -> Option<usize> {
        let old_z = self.cursor_z;
        self.cursor_z = 2;
        let state = self.open_combo_box.as_ref().unwrap();
        let mut gfx = vec![
            Graphic::Rect(state.list_rect, self.style.theme.bg, Some(self.style.theme.fg))
        ];

        // draw options
        let mut hit_rect = Rect {
            x: state.list_rect.x + 1.0,
            y: state.list_rect.y + 1.0,
            w: state.list_rect.w - 2.0,
            h: state.button_rect.h,
        };
        let mouse_pos = mouse_position_vec2();
        let mut return_val = None;
        let lmb = is_mouse_button_released(MouseButton::Left);
        for (i, option) in state.options.iter().enumerate() {
            if hit_rect.contains(mouse_pos) {
                gfx.push(Graphic::Rect(hit_rect, self.style.theme.hover, None));
                if lmb {
                    return_val = Some(i)
                }
            }
            gfx.push(Graphic::Text(hit_rect.x - 1.0, hit_rect.y - 1.0,
                option.to_owned(), self.style.theme.fg));
            hit_rect.y += hit_rect.h;
        }

        // check to close. other close conditions are in combo_box()
        if return_val.is_some() || (already_open
            && is_mouse_button_pressed(MouseButton::Left)
            && !state.list_rect.contains(mouse_position_vec2())) {
            self.open_combo_box = None;
        }

        self.push_graphics(gfx);
        self.cursor_z = old_z;

        return_val
    }

    /// Draws a tab menu. Returns the index of the selected tab.
    pub fn tab_menu(&mut self, id: &str, labels: &[&str]) -> usize {
        let params = self.style.text_params();
        let mut selected_index = self.tabs.get(id).cloned().unwrap_or_default();
        let mut x = self.cursor_x;
        let h = cap_height(&params) + MARGIN * 2.0;
        let mut gfx = vec![
            Graphic::Line(self.bounds.x, self.cursor_y + h + LINE_THICKNESS * 0.5,
                self.bounds.w, self.cursor_y + h + LINE_THICKNESS * 0.5,
                self.style.theme.fg)
        ];
        for (i, label) in labels.iter().enumerate() {
            let r = Rect {
                x,
                y: self.cursor_y,
                w: text_width(label, &params) + MARGIN * 2.0,
                h,
            };
            if i == selected_index {
                // erase line segment
                gfx.push(Graphic::Line(r.x, r.y + r.h + LINE_THICKNESS * 0.5,
                    r.x + r.w - LINE_THICKNESS, r.y + r.h + LINE_THICKNESS * 0.5,
                    self.style.theme.bg));
            } else {
                // fill background
                let color = if self.mouse_hits(r) {
                    if is_mouse_button_pressed(MouseButton::Left) {
                        self.tabs.insert(id.to_owned(), i);
                        selected_index = i;
                    }
                    self.style.theme.click
                } else {
                    self.style.theme.hover
                };
                gfx.push(Graphic::Rect(Rect {w: r.w - LINE_THICKNESS, ..r }, color, None));
            }
            gfx.push(Graphic::Text(x, self.cursor_y,
                label.to_string(), self.style.theme.fg));
            x += r.w;
            gfx.push(Graphic::Line(x - LINE_THICKNESS * 0.5, self.cursor_y,
                x - LINE_THICKNESS *0.5, self.cursor_y + r.h,
                self.style.theme.fg));
        }
        self.cursor_y += cap_height(&params) + MARGIN * 2.0;
        self.push_graphics(gfx);
        selected_index
    }

    pub fn set_tab(&mut self, id: &str, index: usize) {
        self.tabs.insert(id.to_owned(), index);
    }

    /// Draws a slider and returns true if the value was changed.
    pub fn slider(&mut self, id: &str, label: &str, val: &mut f32,
        range: RangeInclusive<f32>
    ) -> bool {
        let h = cap_height(&self.style.text_params());

        // draw groove
        let groove_w = 100.0;
        let groove_x = self.cursor_x + MARGIN * 2.0;
        let groove_y = (self.cursor_y + MARGIN * 2.0 + h * 0.5).round() + 0.5;
        draw_line(groove_x, groove_y,
            groove_x + groove_w, groove_y,
            LINE_THICKNESS, self.style.theme.fg);

        // get/set grabbed state
        let hit_rect = Rect {
            x: self.cursor_x + MARGIN,
            w: groove_w + MARGIN * 2.0,
            y: self.cursor_y + MARGIN,
            h: h + MARGIN * 2.0,
        };
        let mouse_pos = mouse_position_vec2();
        if is_mouse_button_pressed(MouseButton::Left) && self.mouse_hits(hit_rect) {
            self.grabbed_slider = Some(id.to_string());
        }
        let grabbed = if let Some(s) = &self.grabbed_slider {
            s == id
        } else {
            false
        };

        // update position, get handle color
        let (fill, changed) = if grabbed {
            let new_val = interpolate((mouse_pos.x - groove_x) / groove_w, &range)
                .max(*range.start())
                .min(*range.end());
            let changed = new_val != *val;
            *val = new_val;
            (self.style.theme.click, changed)
        } else if self.mouse_hits(hit_rect) {
            (self.style.theme.hover, false)
        } else {
            (self.style.theme.bg, false)
        };
        
        // draw handle
        let f = deinterpolate(*val, &range);
        let handle_rect = Rect {
            x: self.cursor_x + MARGIN + (f * groove_w).round(),
            w: MARGIN * 2.0,
            ..hit_rect
        };
        draw_filled_rect(handle_rect, fill, self.style.theme.fg);

        // draw label
        let text_rect = self.push_text(self.cursor_x + MARGIN * 3.0 + groove_w,
            self.cursor_y + MARGIN, label.to_owned(), self.style.theme.fg);

        self.update_cursor(groove_w + text_rect.w + MARGIN, h + MARGIN * 3.0);
        changed
    }

    pub fn shared_slider(&mut self, id: &str, label: &str, param: &Shared,
        range: RangeInclusive<f32>
    ) {
        let mut val = param.value();
        if self.slider(id, label, &mut val, range) {
            param.set(val);
        }
    }
}

fn interpolate(x: f32, range: &RangeInclusive<f32>) -> f32 {
    range.start() + x * (range.end() - range.start())
}

fn deinterpolate(x: f32, range: &RangeInclusive<f32>) -> f32 {
    (x - range.start()) / (range.end() - range.start())
}

pub fn alert_dialog(style: &Style, message: &str) -> bool {
    let params = style.text_params();
    let r = center(fit_strings(params.clone(), &[message.to_owned()]));
    draw_filled_rect(r, style.theme.bg, style.theme.fg);
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

fn combo_box_list_rect(params: &TextParams, button_rect: Rect, options: &[String]) -> Rect {
    // should options be drawn above or below the button?
    let y_direction = if button_rect.y > screen_height() / 2.0 {
        -1.0
    } else {
        1.0
    };

    let h = button_rect.h * options.len() as f32 + 2.0;
    Rect {
        x: button_rect.x,
        y: if y_direction < 0.0 {
            button_rect.y - h + 1.0
        } else {
            button_rect.y + button_rect.h - 1.0
        },
        w: options.iter().fold(0.0_f32, |w, s| w.max(text_width(s, &params))) + MARGIN * 2.0,
        h,
    }
}

fn screen_rect() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        w: screen_width(),
        h: screen_height(),
    }
}