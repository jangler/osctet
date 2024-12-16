//! Basic immediate-mode GUI library implemented on top of macroquad.
//!
//! Not polished for general reuse. Macroquad also has its own built-in UI
//! library, but the demos don't give me much faith in it.

use std::{collections::HashMap, fmt::Display, ops::RangeInclusive};

use fundsp::shared::Shared;
use macroquad::prelude::*;
use rfd::FileDialog;
use textedit::TextEditState;
use theme::Theme;

use crate::{input::{Hotkey, Modifiers}, module::EventData, pitch::Note, synth::Key, MAIN_TAB_ID, TAB_PATTERN};

pub mod general_tab;
pub mod pattern_tab;
pub mod instruments_tab;
pub mod settings_tab;
pub mod theme;
mod textedit;

const MARGIN: f32 = 5.0;
const LINE_THICKNESS: f32 = 1.0;
const SLIDER_WIDTH: f32 = 100.0;
const MOUSE_WHEEL_INCREMENT: f32 = 120.0;

const PANEL_Z_OFFSET: i8 = 10;
const COMBO_Z_OFFSET: i8 = 20;
const TOOLTIP_Z_OFFSET: i8 = 30;

/// Return a new file dialog. Use this instead of using `rfd` directly.
pub fn new_file_dialog() -> FileDialog {
    // macroquad currently doesn't handle focus lost events, which means that
    // whatever keys were pressed to open the file dialog will be considered
    // to be down until they're released *when the macroquad window has focus*.
    // the workaround here is just to clear the input state when opening a
    // dialog.
    reset_input_state();

    rfd::FileDialog::new()
}

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
            color: self.theme.fg(),
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

#[derive(PartialEq, Debug)]
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

    fn align_right(&mut self, right_edge: f32, params: &TextParams) {
        match self {
            Self::Rect(rect, _, _) => {
                rect.x = right_edge - rect.w;
            },
            Self::Line(_, _, _, _, _) => todo!(),
            Self::Text(x, _, text, _) => {
                *x = right_edge - text_width(text, params);
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
    focused_hotkey: Option<usize>,
    bounds: Rect,
    cursor_x: f32,
    cursor_y: f32,
    cursor_z: i8,
    draw_queue: Vec<DrawOp>,
    pub layout: Layout,
    dialog: Option<Dialog>,
    group_rects: Vec<Rect>,
    focused_note: Option<String>,
    pub note_queue: Vec<(Key, EventData)>,
    instrument_edit_index: Option<usize>,
    mouse_consumed: bool,
    scrollbar_grabbed: bool,
    notification: Option<Notification>,
    text_clipboard: Option<String>,
    group_ignores_geometry: bool,
    widget_on_stack: bool,
}

impl UI {
    pub fn new(theme: Option<Theme>) -> Self {
        Self {
            style: Style {
                font: load_ttf_font_from_bytes(include_bytes!("../font/ProggyClean.ttf"))
                    .expect("included font should be loadable"),
                theme: theme.unwrap_or(Theme::light()),
            },
            open_combo_box: None,
            tabs: HashMap::new(),
            focused_slider: None,
            focused_text: None,
            focused_hotkey: None,
            bounds: Default::default(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_z: 0,
            layout: Layout::Vertical,
            draw_queue: Vec::new(),
            dialog: None,
            group_rects: Vec::new(),
            focused_note: None,
            note_queue: Vec::new(),
            instrument_edit_index: None,
            mouse_consumed: false,
            scrollbar_grabbed: false,
            notification: None,
            text_clipboard: None,
            group_ignores_geometry: false,
            widget_on_stack: false,
        }
    }

    /// Aligns the last `n` graphics elements to the right of the current group.
    /// Panics if no group.
    pub fn align_right(&mut self, n: usize) {
        let start = self.draw_queue.len() - n;
        let params = self.style.text_params();
        let rect = self.group_rects.last().unwrap();
        let edge = rect.x + rect.w - MARGIN;

        for op in self.draw_queue[start..].iter_mut() {
            op.graphic.align_right(edge, &params);
        }
    }

    pub fn grabbed(&self) -> bool {
        self.scrollbar_grabbed || self.focused_slider.is_some()
    }

    pub fn get_tab(&self, key: &str) -> Option<usize> {
        self.tabs.get(key).copied()
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

        if !is_mouse_button_down(MouseButton::Left)
            && !is_mouse_button_released(MouseButton::Left) {
            self.mouse_consumed = false;
        }

        if self.focused_slider.is_some() && is_mouse_button_released(MouseButton::Left) {
            self.focused_slider = None;
        }
        
        clear_background(self.style.theme.panel_bg());

        self.info_box();
    }

    fn flip_layout(&mut self) {
        self.layout = match self.layout {
            Layout::Horizontal => Layout::Vertical,
            Layout::Vertical => Layout::Horizontal,
        };
    }

    /// Starting a group changes the layout axis and starts tracking the total
    /// area of pushed graphics.
    pub fn start_group(&mut self) {
        self.flip_layout();
        self.start_widget();
    }

    /// A widget is just like a group, but doesn't change the layout axis.
    fn start_widget(&mut self) {
        self.widget_on_stack = true;
        self.group_rects.push(Rect {
            x: self.cursor_x,
            y: self.cursor_y,
            w: 0.0,
            h: 0.0,
        });
    }

    /// Ending a group changes the layout axis and offsets the cursor along the
    /// new axis by the width or height of the graphics in the group.
    pub fn end_group(&mut self) -> Option<Rect> {
        if !self.group_rects.is_empty() {
            self.flip_layout();
        }
        self.end_widget()
    }

    /// A widget is just like a group, but doesn't change the layout axis.
    pub fn end_widget(&mut self) -> Option<Rect> {
        self.widget_on_stack = false;
        let rect = self.group_rects.pop();
        if let Some(rect) = rect {
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
        rect
    }

    pub fn end_frame(&mut self) {
        let params = self.style.text_params();
        self.draw_queue.sort_by_key(|x| x.z);
        for op in &self.draw_queue {
            op.graphic.draw(&params);
        }
        self.draw_queue.clear();

        // dialog
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

    pub fn space(&mut self, scale: f32) {
        match self.layout {
            Layout::Horizontal => self.cursor_x += MARGIN * scale,
            Layout::Vertical => self.cursor_y += MARGIN * scale,
        }
    }

    fn push_graphic(&mut self, graphic: Graphic) {
        let (x, y) = match &graphic {
            Graphic::Line(x1, y1, x2, y2, _) => (x1.max(*x2), y1.max(*y2)),
            Graphic::Rect(rect, _, _) => (rect.x + rect.w, rect.y + rect.h),
            Graphic::Text(x, y, text, _) => {
                let params = self.style.text_params();
                (x + text_width(&text, &params) + MARGIN * 2.0,
                    y + cap_height(&params) + MARGIN * 2.0)
            }
        };
        self.expand_groups(x, y);
        self.draw_queue.push(DrawOp {
            z: self.cursor_z,
            graphic,
        });
    }

    fn push_graphics(&mut self, gfx: Vec<Graphic>) {   
        for gfx in gfx {
            self.push_graphic(gfx);
        }
    }

    fn expand_groups(&mut self, x: f32, y: f32) {
        if self.cursor_z < 15 {
            let n = self.group_rects.len();
            for (i, rect) in self.group_rects.iter_mut().enumerate() {
                if !self.group_ignores_geometry || (self.widget_on_stack && i == n - 1) {
                    rect.w = rect.w.max(x - rect.x);
                    rect.h = rect.h.max(y - rect.y);
                }
            }
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
        self.cursor_z += PANEL_Z_OFFSET;
        self.push_rect(Rect {
            y: self.bounds.h - h,
            h, 
            ..self.bounds
        }, self.style.theme.panel_bg(), None);
        self.push_line(self.bounds.x, self.bounds.h - h + 0.5,
            self.bounds.x + self.bounds.w, self.bounds.h - h + 0.5,
            self.style.theme.border_unfocused());
        self.layout = Layout::Horizontal;
        self.cursor_x = self.bounds.x;
        self.cursor_y = self.bounds.h - h;
    }

    pub fn end_bottom_panel(&mut self) {
        self.bounds.h -= self.bottom_panel_height();
        self.cursor_x = self.bounds.x;
        self.cursor_y = self.bounds.y;
        self.cursor_z -= PANEL_Z_OFFSET;
    }

    /// Draws a scrollbar on the right edge of the current bounds.
    pub fn vertical_scrollbar(&mut self,
        current_y: &mut f32, max_y: f32, viewport_h: f32, keys: bool
    ) {
        let (_, y_scroll) = mouse_wheel();
        let actual_increment = MARGIN * 6.0 + cap_height(&self.style.text_params()) * 3.0;
        let dy = -y_scroll / MOUSE_WHEEL_INCREMENT * actual_increment;
        *current_y += dy;

        if keys && !self.accepting_keyboard_input() {
            if is_key_pressed(KeyCode::Home) {
                *current_y = 0.0;
            } else if is_key_pressed(KeyCode::End) {
                *current_y = max_y - viewport_h;
            } else if is_key_pressed(KeyCode::PageUp) {
                *current_y -= viewport_h;
            } else if is_key_pressed(KeyCode::PageDown) {
                *current_y += viewport_h;
            }
        }

        *current_y = (*current_y).min(max_y - viewport_h).max(0.0);

        if viewport_h >= max_y {
            return // no need to draw scrollbar
        }

        let w = MARGIN * 2.0;
        let trough = Rect {
            x: self.bounds.x + self.bounds.w - w,
            y: self.cursor_y,
            w,
            h: viewport_h,
        };
        self.push_rect(trough, self.style.theme.control_bg(), None);

        let h = clamp(viewport_h / max_y, 0.0, 1.0) * trough.h;
        let handle = Rect {
            y: trough.y + (trough.h - h) * *current_y / (max_y - viewport_h),
            h,
            ..trough
        };
        let hit = self.mouse_hits(trough);
        self.push_rect(handle, self.style.theme.control_bg_click(), None);

        if is_mouse_button_pressed(MouseButton::Left) && hit {
            self.scrollbar_grabbed = true;
        }

        if is_mouse_button_down(MouseButton::Left) && (self.scrollbar_grabbed || hit) {
            let (_, y) = mouse_position();
            let offset = ((y - trough.y - handle.h / 2.0) / (trough.h - handle.h))
                .min(1.0).max(0.0);
            *current_y = ((max_y - viewport_h) * offset).round();
        } else {
            self.scrollbar_grabbed = false;
        }

        self.bounds.w -= w;
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

        // occlusion by bottom panel, tab menu, etc.
        if self.cursor_z < 1 && !self.bounds.contains(pt) {
            return false
        }

        rect.contains(pt)
    }
    
    /// A label is non-interactive text.
    pub fn label(&mut self, label: &str) {
        self.start_widget();
        self.push_text(self.cursor_x, self.cursor_y,
            label.to_owned(), self.style.theme.fg());
        self.end_widget();
    }

    /// An offset label is a label offset in the y direction to align with
    /// control labels.
    pub fn offset_label(&mut self, label: &str) {
        self.start_widget();
        self.push_text(self.cursor_x, self.cursor_y + MARGIN,
            label.to_owned(), self.style.theme.fg());
        self.end_widget();
    }

    pub fn header(&mut self, label: &str) {
        let rect = Rect {
            x: self.cursor_x,
            y: self.cursor_y,
            w: self.bounds.w + self.bounds.x - self.cursor_x,
            h: cap_height(&self.style.text_params()) + MARGIN * 2.0,
        };
        self.start_widget();
        self.push_rect(rect, self.style.theme.accent1_bg(), None);
        self.push_text(self.cursor_x, self.cursor_y,
            label.to_owned(), self.style.theme.fg());
        self.end_widget();
    }

    fn text_rect(&mut self, label: &str, x: f32, y: f32,
        bg: &Color, bg_hover: &Color, bg_click: &Color,
    ) -> (Rect, MouseEvent) {
        let params = self.style.text_params();
        let rect = Rect {
            x,
            y,
            w: text_width(label, &params) + MARGIN * 2.0,
            h: cap_height(&params) + MARGIN * 2.0,
        };
        let mouse_hit = self.mouse_hits(rect);
    
        // draw fill based on mouse state
        let (fill, stroke) = if mouse_hit {
            (if is_mouse_button_down(MouseButton::Left) {
                bg_click
            } else {
                bg_hover
            }, self.style.theme.border_focused())
        } else {
            (bg, self.style.theme.border_unfocused())
        };

        self.push_rect(rect, *fill, Some(stroke));
        self.push_text(x, y, label.to_owned(), self.style.theme.fg());
    
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
        self.start_widget();
        let (_, event) = self.text_rect(label,
            self.cursor_x + MARGIN, self.cursor_y + MARGIN,
            &self.style.theme.control_bg(),
            &self.style.theme.control_bg_hover(),
            &self.style.theme.control_bg_click());
        self.end_widget();
        event == MouseEvent::Released
    }

    /// Draws a checkbox and returns true if it was changed this frame.
    pub fn checkbox(&mut self, label: &str, value: &mut bool) -> bool {
        let button_text = if *value { "X" } else { " " };
        self.start_widget();
        let (rect, event) = self.text_rect(button_text,
            self.cursor_x + MARGIN, self.cursor_y + MARGIN,
            &self.style.theme.content_bg(),
            &self.style.theme.content_bg(),
            &self.style.theme.content_bg());
        let clicked = event == MouseEvent::Released;
        self.push_text(self.cursor_x + rect.w + MARGIN, self.cursor_y + MARGIN,
            label.to_owned(), self.style.theme.fg());
        if clicked {
            *value = !*value;
        }
        self.end_widget();
        clicked
    }
    
    /// Draws a combo box. If a value was selected this frame, returns the value's index.
    pub fn combo_box(&mut self, id: &str, label: &str, button_text: &str,
        get_options: impl Fn() -> Vec<String>
    ) -> Option<usize> {
        self.start_widget();

        // draw button and label
        let (button_rect, event) = self.text_rect(&button_text,
            self.cursor_x + MARGIN, self.cursor_y + MARGIN,
            &self.style.theme.control_bg(),
            &self.style.theme.control_bg_hover(),
            &self.style.theme.control_bg_click());
        if !label.is_empty() {
            self.push_text(self.cursor_x + button_rect.w + MARGIN,
                self.cursor_y + MARGIN, label.to_owned(), self.style.theme.fg());
        }
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

        self.end_widget();
        return_val
    }

    /// Draw the list of the active combo box.
    fn combo_box_list(&mut self, already_open: bool) -> Option<usize> {
        self.cursor_z += COMBO_Z_OFFSET;
        let state = self.open_combo_box.as_ref().unwrap();
        let mut gfx = vec![
            Graphic::Rect(state.list_rect, self.style.theme.panel_bg(),
                Some(self.style.theme.border_unfocused()))
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
                gfx.push(Graphic::Rect(hit_rect, self.style.theme.panel_bg_hover(), None));
                if lmb {
                    return_val = Some(i);
                    self.mouse_consumed = true;
                }
            }
            gfx.push(Graphic::Text(hit_rect.x - 1.0, hit_rect.y - 1.0,
                option.to_owned(), self.style.theme.fg()));
            hit_rect.y += hit_rect.h;
        }

        // check to close. other close conditions are in combo_box()
        if return_val.is_some() || (already_open
            && is_mouse_button_pressed(MouseButton::Left)
            && !state.list_rect.contains(mouse_position_vec2())) {
            self.open_combo_box = None;
        }

        self.push_graphics(gfx);
        self.cursor_z -= COMBO_Z_OFFSET;

        return_val
    }

    /// Draws a tab menu. Returns the index of the selected tab.
    pub fn tab_menu(&mut self, id: &str, labels: &[&str]) -> usize {
        if !self.tabs.contains_key(id) {
            self.tabs.insert(id.to_owned(), 0);
        }

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
            }, self.style.theme.panel_bg_hover(), None),
            Graphic::Line(self.bounds.x, self.cursor_y + h + LINE_THICKNESS * 0.5,
                self.bounds.w, self.cursor_y + h + LINE_THICKNESS * 0.5,
                self.style.theme.border_unfocused())
        ];
        for (i, label) in labels.iter().enumerate() {
            let r = Rect {
                x,
                y: self.cursor_y,
                w: text_width(label, &params) + MARGIN * 2.0,
                h,
            };
            // fill background
            let color = if i == selected_index {
                self.style.theme.panel_bg()
            } else if self.mouse_hits(r) {
                if is_mouse_button_pressed(MouseButton::Left) {
                    self.tabs.insert(id.to_owned(), i);
                    selected_index = i;
                }
                self.style.theme.panel_bg_click()
            } else {
                self.style.theme.panel_bg_hover()
            };
            gfx.push(Graphic::Rect(Rect {w: r.w, ..r }, color, None));
            gfx.push(Graphic::Text(x, self.cursor_y,
                label.to_string(), self.style.theme.fg()));
            x += r.w;
            gfx.push(Graphic::Line(x - LINE_THICKNESS * 0.5, self.cursor_y,
                x - LINE_THICKNESS *0.5, self.cursor_y + r.h,
                self.style.theme.border_unfocused()));
            gfx.push(Graphic::Line(r.x, r.y + LINE_THICKNESS * 0.5,
                r.x + r.w - LINE_THICKNESS, r.y + LINE_THICKNESS * 0.5,
                self.style.theme.border_unfocused()));
            if i == selected_index {
                // erase line segment
                gfx.push(Graphic::Line(r.x, r.y + r.h + LINE_THICKNESS * 0.5,
                    r.x + r.w - LINE_THICKNESS, r.y + r.h + LINE_THICKNESS * 0.5,
                    self.style.theme.panel_bg()));
            }
        }
        let h = cap_height(&params) + MARGIN * 2.0 + LINE_THICKNESS;
        self.cursor_y += h;
        self.bounds.y += h;
        self.bounds.h -= h;
        self.push_graphics(gfx);
        selected_index
    }

    pub fn set_tab(&mut self, id: &str, index: usize) {
        self.tabs.insert(id.to_owned(), index);
    }

    pub fn next_tab(&mut self, id: &str, n: usize) {
        if let Some(i) = self.tabs.get_mut(id) {
            *i = (*i + 1) % n;
        }
    }
    
    pub fn prev_tab(&mut self, id: &str, n: usize) {
        if let Some(i) = self.tabs.get_mut(id) {
            *i = (*i as isize - 1).rem_euclid(n as isize) as usize;
        }
    }

    /// Draws a slider and returns true if the value was changed.
    pub fn slider(&mut self, id: &str, label: &str, val: &mut f32,
        range: RangeInclusive<f32>, unit: Option<&str>, power: i32,
    ) -> bool {
        // are we in text entry mode?
        if self.focused_text.as_ref().is_some_and(|x| x.id == id) {
            return self.slider_text_entry(id, label, val, range);
        }

        self.start_widget();
        let h = cap_height(&self.style.text_params());

        // draw groove
        let groove_w = SLIDER_WIDTH;
        let groove_x = self.cursor_x + MARGIN * 2.0;
        let groove_y = (self.cursor_y + MARGIN * 2.0 + h * 0.5).round() + 0.5;

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
                self.mouse_consumed = true;
            }
            if is_mouse_button_pressed(MouseButton::Right) {
                let text = val.to_string();
                self.focused_text = Some(TextEditState::new(id.to_owned(), text));
            }
        }
        let grabbed = if let Some(s) = &self.focused_slider {
            s == id
        } else {
            false
        };

        // update position, get handle color
        let (fill, stroke, changed) = if grabbed {
            let f = ((mouse_pos.x - groove_x) / groove_w).max(0.0).powi(power);
            let new_val = interpolate(f, &range)
                .max(*range.start())
                .min(*range.end());
            let changed = new_val != *val;
            *val = new_val;
            (self.style.theme.control_bg_click(), self.style.theme.border_focused(),
                changed)
        } else if self.mouse_hits(hit_rect) {
            (self.style.theme.control_bg_hover(), self.style.theme.border_focused(), false)
        } else {
            (self.style.theme.control_bg(), self.style.theme.border_unfocused(), false)
        };
        
        // draw groove & handle
        self.push_line(groove_x, groove_y, groove_x + groove_w, groove_y, stroke);
        let f = deinterpolate(*val, &range).powf(1.0/power as f32);
        let handle_rect = Rect {
            x: self.cursor_x + MARGIN + (f * groove_w).round(),
            w: MARGIN * 2.0,
            ..hit_rect
        };
        self.push_rect(handle_rect, fill, Some(stroke));

        // draw label
        let x = self.cursor_x + MARGIN * 3.0 + groove_w;
        let y = self.cursor_y + MARGIN;
        if !label.is_empty() {
            self.push_text(x, y, label.to_owned(), self.style.theme.fg());
        } else {
            // push an invisible rect to reserve space for the handle
            let r = Rect { x, y, w: 0.0, h: 0.0 };
            self.push_rect(r, Color { a: 0.0, ..Default::default() }, None);
        };
        
        if grabbed {
            let text = if let Some(unit) = unit {
                &format!("{:.3} {}", val, unit)
            } else {
                &format!("{:.3}", val)
            };
            self.tooltip(text, handle_rect.x, self.cursor_y - (h + MARGIN * 2.0));
        }

        self.end_widget();
        changed
    }

    fn slider_text_entry(&mut self, id: &str, label: &str, val: &mut f32,
        range: RangeInclusive<f32>
    ) -> bool {
        // another silly little dance for the borrow checker
        let mut text = self.focused_text.as_ref().unwrap().text.clone();
        let mut changed = false;
        if self.text_box(id, label, SLIDER_WIDTH + MARGIN * 2.0, &mut text) {
            match text.parse::<f32>() {
                Ok(f) => {
                    *val = f.max(*range.start()).min(*range.end());
                    changed = true;
                },
                Err(e) => self.report(e),
            }
            self.focused_text = None;
        }
        changed
    }

    /// Widget for editing a value as text.
    pub fn edit_box(&mut self, label: &str, chars_wide: usize,
        text: String
    ) -> Option<String> {
        let w = chars_wide as f32 * text_width("x", &self.style.text_params())
            + MARGIN * 2.0;

        if self.text_box(label, label, w, &text) {
            let s = self.focused_text.as_ref().map(|x| x.text.clone());
            self.focused_text = None;
            s
        } else {
            None
        }
    }

    /// Returns true if the text was submitted (i.e. Enter was pressed).
    fn text_box(&mut self, id: &str, label: &str, width: f32, text: &str) -> bool {
        let box_rect = Rect {
            x: self.cursor_x + MARGIN,
            y: self.cursor_y + MARGIN,
            w: width,
            h: cap_height(&self.style.text_params()) + MARGIN * 2.0,
        };

        let focused = self.focused_text.as_ref().is_some_and(|x| x.id == id);
        let hit = self.mouse_hits(box_rect);
        
        // focus/unfocus
        if !focused && hit && is_mouse_button_pressed(MouseButton::Left) {
            self.focused_text = Some(TextEditState::new(id.to_owned(), text.to_owned()));
        } else if is_key_pressed(KeyCode::Escape) {
            self.focused_text = None;
        }
        
        self.start_widget();

        // draw box
        let stroke = if focused || hit {
            self.style.theme.border_focused()
        } else {
            self.style.theme.border_unfocused()
        };
        self.push_rect(box_rect, self.style.theme.content_bg(), Some(stroke));
        
        // draw text
        let submit = if focused {
            self.editable_text(box_rect)
        } else {
            self.push_text(box_rect.x, box_rect.y, text.to_string(),
                self.style.theme.fg());
            false
        };

        // draw label
        if !label.is_empty() {
            self.push_text(box_rect.x + box_rect.w, self.cursor_y + MARGIN,
                label.to_owned(), self.style.theme.fg());
        }
        
        self.end_widget();
        submit
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

        self.start_widget();
        self.push_rect(list_rect, self.style.theme.content_bg(),
            Some(self.style.theme.border_unfocused()));

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
                self.push_rect(hit_rect, self.style.theme.content_bg_click(), None);
            } else if self.mouse_hits(hit_rect) {
                self.push_rect(hit_rect, self.style.theme.content_bg_hover(), None);
                if lmb {
                    *index = i;
                }
            }

            // check for unfocus
            if Some(i) == self.instrument_edit_index && is_key_pressed(KeyCode::Escape) {
                self.focused_text = None;
                self.instrument_edit_index = None;
            }

            if Some(i) == self.instrument_edit_index {
                if self.editable_text(hit_rect) {
                    if let Some(state) = self.focused_text.take() {
                        return_val = Some(state.text);
                        self.instrument_edit_index = None;
                    }
                }
            } else {
                if self.mouse_hits(hit_rect) && is_mouse_button_pressed(MouseButton::Right) && i > 0 {
                    let text = option.clone();
                    self.focused_text = Some(TextEditState::new(id.to_string(), text));
                    self.instrument_edit_index = Some(i);
                    *index = i;
                }
                self.push_text(hit_rect.x, hit_rect.y,
                    option.to_owned(), self.style.theme.fg());
            }
            hit_rect.y += hit_rect.h;
        }

        self.end_widget();
        return_val
    }

    /// Primitive that draws the currently focused text and handles edit input.
    fn editable_text(&mut self, rect: Rect) -> bool {
        let hit = self.mouse_hits(rect);

        if let Some(state) = self.focused_text.as_mut() {
            let params = self.style.text_params();
            let char_w = text_width("x", &params);
            let mouse_i = if hit {
                Some(((mouse_position_vec2().x - rect.x - MARGIN) / char_w)
                    .max(0.0).round() as usize)
            } else {
                None
            };
            state.handle_input(mouse_i, &mut self.text_clipboard);

            let text_h = cap_height(&params);
            let f = |i| rect.x + char_w * i as f32 + MARGIN + LINE_THICKNESS * 0.5;
            let cursor_x = f(state.cursor);
            let y1 = rect.y + MARGIN - 1.0;
            let y2 = rect.y + MARGIN + text_h + 1.0;
            let text = state.text.clone();
            
            if state.cursor != state.anchor {
                let anchor_x = f(state.anchor);
                let start = cursor_x.min(anchor_x);
                let end = cursor_x.max(anchor_x);
                let r = Rect {
                    x: start,
                    y: y1,
                    w: end - start,
                    h: y2 - y1,
                };
                let c = Color {
                    a: 0.1,
                    ..self.style.theme.fg()
                };
                self.push_rect(r, c, None);
            }
            
            self.push_line(cursor_x, y1, cursor_x, y2, self.style.theme.fg());
            self.push_text(rect.x, rect.y, text, self.style.theme.fg());
        }

        is_key_pressed(KeyCode::Enter) ||
            (!hit && is_mouse_button_pressed(MouseButton::Left))
    }

    pub fn shared_slider(&mut self, id: &str, label: &str, param: &Shared,
        range: RangeInclusive<f32>, unit: Option<&str>, power: i32
    ) {
        let mut val = param.value();
        if self.slider(id, label, &mut val, range, unit, power) {
            param.set(val);
        }
    }

    fn open_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
    }

    /// Report an error in an alert dialog.
    pub fn report(&mut self, e: impl Display) {
        self.open_dialog(Dialog::Alert(e.to_string()));
    }

    /// Temporarily use the info box to display a message.
    pub fn notify(&mut self, message: String) {
        self.notification = Some(Notification {
            time_remaining: 1.0 + message.chars().count() as f32 * 0.1,
            message,
        });
    }

    pub fn accepting_keyboard_input(&self) -> bool {
        self.focused_text.is_some() || self.focused_hotkey.is_some()
    }

    pub fn accepting_note_input(&self) -> bool {
        self.focused_note.is_some()
    }

    pub fn tooltip(&mut self, text: &str, x: f32, y: f32) {
        self.cursor_z += TOOLTIP_Z_OFFSET;
        self.text_rect(text, x, y,
            &self.style.theme.panel_bg(),
            &self.style.theme.panel_bg(),
            &self.style.theme.panel_bg());
        self.cursor_z -= TOOLTIP_Z_OFFSET;
    }

    /// Returns the key that set the new note value.
    pub fn note_input(&mut self, id: &str, note: &mut Note) -> Option<Key> {
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
        let (fill, stroke) = if focused {
            (self.style.theme.control_bg_click(), self.style.theme.border_focused())
        } else if mouse_hit {
            (self.style.theme.control_bg_hover(), self.style.theme.border_focused())
        } else {
            (self.style.theme.control_bg(), self.style.theme.border_unfocused())
        };

        let mut key = None;
        if focused {
            for evt in self.note_queue.iter() {
                if let (k, EventData::Pitch(input_note)) = evt {
                    *note = *input_note;
                    self.focused_note = None;
                    key = Some(k.clone());
                }
            }
        }

        self.start_widget();
        self.push_rect(rect, fill, Some(stroke));
        self.push_text(rect.x, rect.y, label, self.style.theme.fg());
        self.end_widget();

        key
    }

    // TODO: code duplication with note_input
    pub fn hotkey_input(&mut self, id: usize, hotkey: &mut Hotkey) -> bool {
        let params = self.style.text_params();
        let label = hotkey.to_string();

        let rect = Rect {
            x: self.cursor_x + MARGIN,
            y: self.cursor_y + MARGIN,
            w: text_width(&label, &params) + MARGIN * 2.0,
            h: cap_height(&params) + MARGIN * 2.0,
        };
        let mouse_hit = self.mouse_hits(rect);

        if mouse_hit && is_mouse_button_pressed(MouseButton::Left) {
            self.focused_hotkey = Some(id.to_owned());
        }
        let focused = self.focused_hotkey.as_ref().is_some_and(|x| *x == id);
    
        // draw fill based on mouse state
        let (fill, stroke) = if focused {
            (self.style.theme.control_bg_click(), self.style.theme.border_focused())
        } else if mouse_hit {
            (self.style.theme.control_bg_hover(), self.style.theme.border_focused())
        } else {
            (self.style.theme.control_bg(), self.style.theme.border_unfocused())
        };

        let mut changed = false;
        if focused {
            let key = get_keys_pressed().into_iter().filter(|x| !is_mod(*x)).next();
            if let Some(key) = key {
                *hotkey = Hotkey::new(Modifiers::current(), key);
                self.focused_hotkey = None;
                changed = true;
            }
        }

        self.start_widget();
        self.push_rect(rect, fill, Some(stroke));
        self.push_text(rect.x, rect.y, label, self.style.theme.fg());
        self.end_widget();

        changed
    }

    fn info_box(&mut self) {
        // notification
        let mut note_expired = false;
        let text = if let Some(note) = &mut self.notification {
            note.time_remaining -= get_frame_time().min(0.1);
            note_expired = note.time_remaining <= 0.0;
            Some(note.message.clone())
        } else {
            None
        };
        if note_expired {
            self.notification = None;
        }

        if let Some(text) = text {
            let params = self.style.text_params();
            let w = text_width(&text, &params);
            let h = cap_height(&params);
            self.cursor_z += TOOLTIP_Z_OFFSET;
            let (_, evt) = self.text_rect(&text,
                self.bounds.x + self.bounds.w - w - MARGIN * 3.0,
                self.bounds.y + self.bounds.h - h - MARGIN * 3.0,
                &self.style.theme.panel_bg(),
                &self.style.theme.panel_bg(),
                &self.style.theme.panel_bg());
            self.cursor_z -= TOOLTIP_Z_OFFSET;

            if evt == MouseEvent::Pressed {
                self.notification = None;
                self.mouse_consumed = true;
            }
        }
    }
}

fn interpolate(x: f32, range: &RangeInclusive<f32>) -> f32 {
    range.start() + x * (range.end() - range.start())
}

fn deinterpolate(x: f32, range: &RangeInclusive<f32>) -> f32 {
    (x - range.start()) / (range.end() - range.start())
}

// TODO: characters with descenders give this too large a bottom margin. make
//       the rect size independent of the particular characters
fn alert_dialog(style: &Style, message: &str) {
    let params = style.text_params();
    let mut r = center(fit_strings(params.clone(), &[message.to_owned()]));
    r.h += MARGIN;
    draw_filled_rect(r, style.theme.panel_bg(), style.theme.border_unfocused());
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

struct Notification {
    message: String,
    time_remaining: f32,
}

fn is_mod(key: KeyCode) -> bool {
    match key {
        KeyCode::LeftAlt | KeyCode::RightAlt
            | KeyCode::LeftControl | KeyCode::RightControl
            | KeyCode::LeftShift | KeyCode::RightShift
            | KeyCode::LeftSuper | KeyCode::RightSuper => true,
        _ => false,
    }
}