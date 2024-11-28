//! Basic immediate-mode GUI library implemented on top of macroquad.
//!
//! Not polished for general reuse. Macroquad also has its own built-in UI
//! library, but the demos don't give me much faith in it.

use std::{collections::HashMap, fmt::Display, ops::RangeInclusive};

use fundsp::shared::Shared;
use macroquad::prelude::*;

use crate::{module::{EventData, Position}, pitch::Note};

pub mod general_tab;
pub mod pattern_tab;
pub mod instruments_tab;

const MARGIN: f32 = 5.0;
const LINE_THICKNESS: f32 = 1.0;
const SLIDER_WIDTH: f32 = 100.0;

enum Dialog {
    Alert(String),
}

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

struct TextEditState {
    id: String,
    text: String,
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
    focused_slider: Option<String>,
    focused_text: Option<TextEditState>,
    bounds: Rect,
    cursor_x: f32,
    cursor_y: f32,
    cursor_z: i8,
    grid_x: f32,
    column_widths: Vec<f32>,
    column_index: usize,
    draw_queue: Vec<DrawOp>,
    pub layout: Layout,
    dialog: Option<Dialog>,
    group_rects: Vec<Rect>,
    focused_note: Option<String>,
    pub note_queue: Vec<EventData>,
    instrument_edit_index: Option<usize>,
    mouse_consumed: bool,
    edit_start: Position,
    edit_end: Position,
    beat_division: u32,
}

impl UI {
    pub fn new() -> Self {
        let edit_cursor = Position {
            tick: 0,
            track: 0,
            channel: 0,
            column: 0,
        };
        Self {
            style: Style {
                font: load_ttf_font_from_bytes(include_bytes!("font/ProggyClean.ttf"))
                    .expect("included font should be loadable"),
                theme: LIGHT_THEME,
            },
            open_combo_box: None,
            tabs: HashMap::new(),
            focused_slider: None,
            focused_text: None,
            bounds: Default::default(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_z: 0,
            grid_x: 0.0,
            column_widths: Vec::new(),
            column_index: 0,
            layout: Layout::Vertical,
            draw_queue: Vec::new(),
            dialog: None,
            group_rects: Vec::new(),
            focused_note: None,
            note_queue: Vec::new(),
            instrument_edit_index: None,
            mouse_consumed: false,
            edit_start: edit_cursor,
            edit_end: edit_cursor,
            beat_division: 4,
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
        self.mouse_consumed = false;

        if self.focused_slider.is_some() && is_mouse_button_released(MouseButton::Left) {
            self.focused_slider = None;
        }
        
        clear_background(self.style.theme.bg);
    }

    pub fn start_group(&mut self) {
        self.group_rects.push(Rect {
            x: self.cursor_x,
            y: self.cursor_y,
            w: 0.0,
            h: 0.0,
        });
    }

    pub fn end_group(&mut self) {
        if let Some(rect) = self.group_rects.pop() {
            match self.layout {
                Layout::Horizontal => {
                    self.cursor_x = rect.x + rect.w;
                    self.cursor_y = rect.y;
                },
                Layout::Vertical => {
                    self.cursor_x = rect.x;
                    self.cursor_y = rect.y + rect.h;
                },
            }
        }
    }

    pub fn end_frame(&mut self) {
        let params = self.style.text_params();
        self.draw_queue.sort_by_key(|x| x.z);
        for op in &self.draw_queue {
            op.graphic.draw(&params);
        }
        self.draw_queue.clear();

        let close = if let Some(dialog) = &self.dialog {
            match dialog {
                Dialog::Alert(s) => alert_dialog(&self.style, s),
            };
            is_key_pressed(KeyCode::Escape) || is_mouse_button_pressed(MouseButton::Left)
        } else {
            false
        };
        if close {
            self.dialog = None;
        }

        // drain input queues
        while let Some(_) = get_char_pressed() {}
        self.note_queue.clear();
    }

    pub fn start_grid(&mut self, column_widths: &[f32], column_names: &[&str]) {
        assert_eq!(column_widths.len(), column_names.len());
        self.layout = Layout::Horizontal;
        self.grid_x = self.cursor_x;
        self.column_widths = column_widths.to_vec();
        self.column_index = 0;
        for name in column_names {
            self.offset_label(name);
            self.next_cell();
        }
    }

    pub fn end_grid(&mut self) {
        self.layout = Layout::Vertical;
    }

    pub fn next_cell(&mut self) {
        self.column_index += 1;
        if self.column_index >= self.column_widths.len() {
            self.column_index = 0;
            self.cursor_y += cap_height(&self.style.text_params()) + MARGIN * 3.0;
        }
        self.cursor_x = self.grid_x
            + self.column_widths[..self.column_index].iter().sum::<f32>();
    }

    pub fn space(&mut self, scale: f32) {
        self.update_cursor(MARGIN * scale, MARGIN * scale);
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

    fn expand_groups(&mut self, x: f32, y: f32) {
        if self.cursor_z == 0 {
            for rect in self.group_rects.iter_mut() {
                rect.w = rect.w.max(x - rect.x);
                rect.h = rect.h.max(y - rect.y);
            }
        }
    }

    fn push_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color) {
        self.push_graphic(Graphic::Line(x1, y1, x2, y2, color));
        self.expand_groups(x1.max(x2), y1.max(y2));
    }

    fn push_rect(&mut self, rect: Rect, fill: Color, stroke: Option<Color>) {
        self.push_graphic(Graphic::Rect(rect, fill, stroke));
        self.expand_groups(rect.x + rect.w, rect.y + rect.h);
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
        self.expand_groups(rect.x + rect.w, rect.y + rect.h);
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
        if self.mouse_consumed || self.dialog.is_some() {
            return false
        }

        let pt = mouse_position_vec2();

        // occlusion by combo box
        if let Some(state) = &self.open_combo_box {
            if state.list_rect.contains(pt) {
                return false
            }
        }

        // occlusion by bottom panel
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

    pub fn offset_label(&mut self, label: &str) {
        let rect = self.push_text(self.cursor_x, self.cursor_y + MARGIN,
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
        self.update_cursor(rect.w + MARGIN, rect.h + MARGIN * 2.0);
        event == MouseEvent::Released
    }

    /// Draws a checkbox and returns true if it was changed this frame.
    pub fn checkbox(&mut self, label: &str, value: &mut bool) -> bool {
        let button_text = if *value { "X" } else { " " };
        let clicked = self.button(button_text);
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
        let label_dim = if !label.is_empty() {
            self.push_text(self.cursor_x + button_rect.w + MARGIN,
                self.cursor_y + MARGIN, label.to_owned(), self.style.theme.fg)
        } else {
            Rect::default()
        };
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
                    return_val = Some(i);
                    self.mouse_consumed = true;
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
            Graphic::Rect(Rect {
                x: self.bounds.x,
                y: self.cursor_y,
                w: self.bounds.w,
                h: h
            }, self.style.theme.hover, None),
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
            }
            // fill background
            let color = if i == selected_index {
                self.style.theme.bg
            } else if self.mouse_hits(r) {
                if is_mouse_button_pressed(MouseButton::Left) {
                    self.tabs.insert(id.to_owned(), i);
                    selected_index = i;
                }
                self.style.theme.click
            } else {
                self.style.theme.hover
            };
            gfx.push(Graphic::Rect(Rect {w: r.w - LINE_THICKNESS, ..r }, color, None));
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
        range: RangeInclusive<f32>, unit: Option<&str>
    ) -> bool {
        // are we in text entry mode?
        if self.focused_text.as_ref().is_some_and(|x| x.id == id) {
            if is_key_pressed(KeyCode::Escape)
                || is_mouse_button_released(MouseButton::Left) {
                self.focused_text = None;
            } else {
                return self.slider_text_entry(label, val, range);
            }
        }

        let h = cap_height(&self.style.text_params());

        // draw groove
        let groove_w = SLIDER_WIDTH;
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
        if self.mouse_hits(hit_rect) {
            if is_mouse_button_pressed(MouseButton::Left) {
                self.focused_slider = Some(id.to_string());
            }
            if is_mouse_button_pressed(MouseButton::Right) {
                self.focused_text = Some(TextEditState {
                    id: id.to_string(),
                    text: val.to_string(),
                });
            }
        }
        let grabbed = if let Some(s) = &self.focused_slider {
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
        
        if grabbed {
            let text = if let Some(unit) = unit {
                &format!("{:.2} {}", val, unit)
            } else {
                &format!("{:.2}", val)
            };
            self.tooltip(text, text_rect.x + text_rect.w, text_rect.y);
        }

        self.update_cursor(groove_w + text_rect.w + MARGIN, h + MARGIN * 3.0);

        changed
    }

    fn slider_text_entry(&mut self, label: &str, val: &mut f32,
        range: RangeInclusive<f32>
    ) -> bool {
        // another silly little dance for the borrow checker
        let mut text = self.focused_text.as_ref().unwrap().text.clone();
        let mut changed = false;
        if self.text_box(label, label, MARGIN, SLIDER_WIDTH, &mut text) {
            match text.parse::<f32>() {
                Ok(f) => {
                    *val = f.max(*range.start()).min(*range.end());
                    changed = true;
                },
                Err(e) => self.report(e),
            }
            self.focused_text = None;
        }
        if let Some(focus) = self.focused_text.as_mut() {
            focus.text = text;
        }
        changed
    }

    pub fn edit_box(&mut self, label: &str, chars_wide: usize,
        text: String
    ) -> Option<String> {
        // check whether to unfocus
        if self.focused_text.as_ref().is_some_and(|x| x.id == label)
            && (is_key_pressed(KeyCode::Escape)
                || is_mouse_button_pressed(MouseButton::Left)) {
            self.focused_text = None;
        }

        let w = chars_wide as f32 * text_width("x", &self.style.text_params())
            + MARGIN * 2.0;

        if self.text_box(label, label, 0.0, w, &text) {
            let s = self.focused_text.as_ref().map(|x| x.text.clone());
            self.focused_text = None;
            s
        } else {
            None
        }
    }

    /// Returns true if the text was submitted (i.e. Enter was pressed).
    fn text_box(&mut self, id: &str, label: &str, offset_x: f32, width: f32,
        text: &str
    ) -> bool {
        let box_rect = Rect {
            x: self.cursor_x + MARGIN + offset_x,
            y: self.cursor_y + MARGIN,
            w: width,
            h: cap_height(&self.style.text_params()) + MARGIN * 2.0,
        };
        self.push_rect(box_rect, self.style.theme.bg, Some(self.style.theme.fg));

        let focused = self.focused_text.as_ref().is_some_and(|x| x.id == id);
        let text = if focused {
            let text = &mut self.focused_text.as_mut().unwrap().text;
            // handle text editing
            while let Some(c) = get_char_pressed() {
                if !c.is_ascii_control() {
                    text.push(c);
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                text.pop();
            }
            text.clone()
        } else {
            if is_mouse_button_pressed(MouseButton::Left) && self.mouse_hits(box_rect) {
                self.focused_text = Some(TextEditState {
                    id: id.to_owned(),
                    text: text.to_string(),
                });
            }
            text.to_string()
        };

        let text_rect = self.push_text(box_rect.x, box_rect.y, text, self.style.theme.fg);
        
        if focused {
            let line_x = text_rect.x + text_rect.w - MARGIN + 0.5;
            self.push_line(line_x, text_rect.y + MARGIN - 1.0,
                line_x, text_rect.y + text_rect.h - MARGIN + 1.0,
                self.style.theme.fg);
        }

        let label_rect = self.push_text(box_rect.x + box_rect.w + MARGIN,
            self.cursor_y + MARGIN, label.to_owned(), self.style.theme.fg);
        
        self.update_cursor(width + label_rect.w + MARGIN, box_rect.h + MARGIN);
        
        focused && is_key_pressed(KeyCode::Enter)
    }

    /// List box with editable values. Returns a string when an edit is submitted.
    pub fn instrument_list(&mut self, options: &[String], index: &mut usize,
        min_chars: usize,
    ) -> Option<String> {
        let params = self.style.text_params();
        let line_height = cap_height(&params) + MARGIN * 2.0;
        let list_rect = Rect {
            x: self.cursor_x + MARGIN,
            y: self.cursor_y + MARGIN,
            w: options.iter().fold(0.0_f32, |w, s| w.max(text_width(s, &params)))
                .max(text_width("x", &params) * min_chars as f32)
                + MARGIN * 2.0,
            h: line_height * options.len() as f32 + 2.0,
        };

        self.push_graphic(Graphic::Rect(
            list_rect, self.style.theme.bg, Some(self.style.theme.fg)));

        // draw options
        let mut hit_rect = Rect {
            x: list_rect.x + 1.0,
            y: list_rect.y + 1.0,
            w: list_rect.w - 2.0,
            h: line_height,
        };
        let lmb = is_mouse_button_released(MouseButton::Left);
        let mut return_val = None;
        let id = "patch name";
        for (i, option) in options.iter().enumerate() {
            if i == *index {
                self.push_graphic(Graphic::Rect(hit_rect, self.style.theme.click, None));
            } else if self.mouse_hits(hit_rect) {
                self.push_graphic(Graphic::Rect(hit_rect, self.style.theme.hover, None));
                if lmb {
                    *index = i;
                }
            }
            if Some(i) == self.instrument_edit_index {
                if self.editable_text(hit_rect.x - 1.0, hit_rect.y) {
                    if let Some(state) = self.focused_text.take() {
                        return_val = Some(state.text);
                        self.instrument_edit_index = None;
                    }
                }
            } else {
                if self.mouse_hits(hit_rect) && is_mouse_button_pressed(MouseButton::Right) && i > 0 {
                    self.focused_text = Some(TextEditState {
                        id: id.to_owned(),
                        text: option.clone(),
                    });
                    self.instrument_edit_index = Some(i);
                    *index = i;
                }
                self.push_graphic(Graphic::Text(hit_rect.x - 1.0, hit_rect.y,
                    option.to_owned(), self.style.theme.fg));
            }
            hit_rect.y += hit_rect.h;
        }

        self.update_cursor(list_rect.w + MARGIN * 2.0, list_rect.h + MARGIN);

        return_val
    }

    /// Primitive that draws the currently focused text and handles edit input.
    fn editable_text(&mut self, x: f32, y: f32) -> bool {
        let mut gfx = Vec::new();
        let params = self.style.text_params();

        if let Some(state) = self.focused_text.as_mut() {
            let text_width = text_width(&state.text, &params);
            let text_height = cap_height(&params);
            gfx.push(Graphic::Text(x, y, state.text.clone(), self.style.theme.fg));
            let line_x = x + text_width + MARGIN + 0.5;
            gfx.push(Graphic::Line(line_x, y + MARGIN - 2.0,
                line_x, y + MARGIN + text_height + 2.0,
                self.style.theme.fg));
            
            while let Some(c) = get_char_pressed() {
                if c.is_ascii_graphic() || c == ' ' {
                    state.text.push(c);
                }
            }

            if is_key_pressed(KeyCode::Backspace) {
                state.text.pop();
            }
        }

        self.push_graphics(gfx);

        is_key_pressed(KeyCode::Enter)
    }

    pub fn shared_slider(&mut self, id: &str, label: &str, param: &Shared,
        range: RangeInclusive<f32>, unit: Option<&str>
    ) {
        let mut val = param.value();
        if self.slider(id, label, &mut val, range, unit) {
            param.set(val);
        }
    }

    fn open_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
    }

    pub fn report(&mut self, e: impl Display) {
        self.open_dialog(Dialog::Alert(e.to_string()));
    }

    pub fn accepting_keyboard_input(&self) -> bool {
        self.focused_text.is_some()
    }

    pub fn accepting_note_input(&self) -> bool {
        self.focused_note.is_some()
    }

    pub fn tooltip(&mut self, text: &str, x: f32, y: f32) {
        let old_z = self.cursor_z;
        self.cursor_z = 3;
        self.text_rect(text, x, y);
        self.cursor_z = old_z;
    }

    pub fn note_input(&mut self, id: &str, note: &mut Note) {
        let params = self.style.text_params();
        let label = note.to_string();

        let rect = Rect {
            x: self.cursor_x + MARGIN,
            y: self.cursor_y + MARGIN,
            w: text_width(&label, &params) + MARGIN * 2.0,
            h: cap_height(&params) + MARGIN * 2.0,
        };
        let mouse_hit = self.mouse_hits(rect);

        if mouse_hit && is_mouse_button_pressed(MouseButton::Left) {
            self.focused_note = Some(id.to_owned());
        }
        let focused = self.focused_note.as_ref().is_some_and(|x| x == id);
    
        // draw fill based on mouse state
        let fill = if focused {
            self.style.theme.click
        } else if mouse_hit {
            self.style.theme.hover
        } else {
            self.style.theme.bg
        };

        if focused {
            for evt in self.note_queue.iter() {
                if let EventData::Pitch(input_note) = evt {
                    *note = *input_note;
                    self.focused_note = None;
                }
            }
        }

        self.push_rect(rect, fill, Some(self.style.theme.fg));
        self.push_text(rect.x, rect.y, label, self.style.theme.fg);

        self.update_cursor(rect.w + MARGIN, rect.h + MARGIN);
    }
}

fn interpolate(x: f32, range: &RangeInclusive<f32>) -> f32 {
    range.start() + x * (range.end() - range.start())
}

fn deinterpolate(x: f32, range: &RangeInclusive<f32>) -> f32 {
    (x - range.start()) / (range.end() - range.start())
}

fn alert_dialog(style: &Style, message: &str) {
    let params = style.text_params();
    let mut r = center(fit_strings(params.clone(), &[message.to_owned()]));
    r.h += MARGIN;
    draw_filled_rect(r, style.theme.bg, style.theme.fg);
    draw_text_topleft(params.clone(), message, r.x, r.y);
}

fn fit_strings(params: TextParams, v: &[String]) -> Rect {
    let mut rect: Rect = Default::default();
    for s in v {
        let dim = measure_text(s, params.font, params.font_size, params.font_scale);
        rect.w = rect.w.max(dim.width + MARGIN * 2.0);
        rect.h += dim.height + MARGIN;
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