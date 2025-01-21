//! Basic immediate-mode GUI library implemented on top of macroquad.
//!
//! Not polished for general reuse. Macroquad also has its own built-in UI
//! library, but the demos don't give me much faith in it.

use std::{collections::HashMap, fmt::Display, mem, ops::RangeInclusive};

use fundsp::shared::Shared;
use info::{ControlInfo, Info};
use macroquad::prelude::*;
use rfd::FileDialog;
use text::GlyphAtlas;
use textedit::TextEditState;
use theme::Theme;

use crate::{config::Config, input::{Action, Hotkey, Modifiers}, module::EventData, pitch::Note, playback::Player, synth::Key, MAIN_TAB_ID, TAB_PATTERN};

pub mod general_tab;
pub mod pattern_tab;
pub mod instruments_tab;
pub mod settings_tab;
pub mod theme;
pub mod text;
mod textedit;
pub mod info;

const LINE_THICKNESS: f32 = 1.0;
const SLIDER_WIDTH: f32 = 100.0;

const PANEL_Z_OFFSET: i8 = 10;
const COMBO_Z_OFFSET: i8 = 20;
const TOOLTIP_Z_OFFSET: i8 = 30;

/// Seconds before info popup.
const INFO_DELAY: f32 = 0.1;

pub const MAX_PATCH_NAME_CHARS: usize = 20;

/// Return a new file dialog. Use this instead of using `rfd` directly.
pub fn new_file_dialog(player: &mut Player) -> FileDialog {
    // macroquad currently doesn't handle focus lost events, which means that
    // whatever keys were pressed to open the file dialog will be considered
    // to be down until they're released *when the macroquad window has focus*.
    // the workaround here is just to clear the input state when opening a
    // dialog.
    reset_input_state();

    player.stop(); // file dialog is sync, events will hang
    FileDialog::new()
}

enum Dialog {
    Alert(String),
    OkCancel(String, Action),
}

/// Draws text with the top-left corner at (x, y), plus margins.
/// Returns the bounds of the text, plus margins.
fn draw_text_topleft(style: &Style, color: Color, label: &str, x: f32, y: f32
) -> Rect {
    style.atlas.draw_text(x + style.margin, y + style.margin, label, color)
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
    pub atlas: GlyphAtlas,
    pub theme: Theme,
    pub margin: f32,
}

impl Style {
    pub fn line_height(&self) -> f32 {
        self.atlas.cap_height() + self.margin * 2.0
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
    fn draw(&self, style: &Style) {
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
                draw_text_topleft(style, *color, text, *x, *y);
            }
        }
    }

    fn overlaps(&self, style: &Style, rect: &Rect) -> bool {
        let this_rect = match self {
            Self::Rect(rect, _, _) => rect,
            Self::Line(x1, y1, x2, y2, _) =>
                &Rect::new(*x1, *y1, x2 - x1, y2 - y1),
            Self::Text(x, y, text, _) =>
                &Rect::new(*x, *y, style.atlas.text_width(text), style.line_height()),
        };
        this_rect.overlaps(rect)
    }
}

struct DrawOp {
    z: i8,
    graphic: Graphic,
}

enum Focus {
    None,
    ComboBox(ComboBoxState),
    Slider(String),
    Text(TextEditState),
    Hotkey(usize),
    Note(String),
}

impl Focus {
    fn is_slider(&self) -> bool {
        matches!(self, Self::Slider(_))
    }

    fn id(&self) -> Option<&str> {
        match self {
            Self::ComboBox(state) => Some(&state.id),
            Self::Slider(s) | Self::Note(s) => Some(s),
            Self::Text(state) => Some(&state.id),
            _ => None,
        }
    }
}

impl Default for Focus {
    fn default() -> Self {
        Self::None
    }
}

/// Draws widgets and tracks UI state.
pub struct Ui {
    pub style: Style,
    tabs: HashMap<String, usize>,
    bounds: Rect,
    cursor_x: f32,
    cursor_y: f32,
    cursor_z: i8,
    draw_queue: Vec<DrawOp>,
    pub layout: Layout,
    dialog: Option<Dialog>,
    group_rects: Vec<Rect>,
    pub note_queue: Vec<(Key, EventData)>,
    instrument_edit_index: Option<usize>,
    mouse_consumed: Option<String>,
    v_scrollbar_grabbed: bool,
    h_scrollbar_grabbed: bool,
    notification: Option<Notification>,
    text_clipboard: Option<String>,
    group_ignores_geometry: bool,
    widget_on_stack: bool,
    info: Info,
    ctrl_info: ControlInfo,
    saved_info: (Info, ControlInfo),
    info_delay: f32,
    bottom_right_corner: Vec2,
    focus: Focus,
    pending_focus: Option<String>,
    lost_focus: Focus,
    tab_nav_list: Vec<(Vec2, String)>,
}

impl Ui {
    pub fn new(theme: Option<Theme>, font_index: usize) -> Self {
        let atlas = GlyphAtlas::from_bdf_bytes(text::FONT_BYTES.get(font_index)
            .unwrap_or(&text::FONT_BYTES[0]))
            .expect("included font should be loadable");

        Self {
            style: Style {
                margin: atlas.max_height() - atlas.cap_height(),
                atlas,
                theme: theme.unwrap_or_default(),
            },
            tabs: HashMap::new(),
            bounds: Default::default(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_z: 0,
            layout: Layout::Vertical,
            draw_queue: Vec::new(),
            dialog: None,
            group_rects: Vec::new(),
            note_queue: Vec::new(),
            instrument_edit_index: None,
            mouse_consumed: None,
            v_scrollbar_grabbed: false,
            h_scrollbar_grabbed: false,
            notification: None,
            text_clipboard: None,
            group_ignores_geometry: false,
            widget_on_stack: false,
            info: Info::None,
            ctrl_info: ControlInfo::None,
            saved_info: (Info::None, ControlInfo::None),
            info_delay: INFO_DELAY,
            bottom_right_corner: Vec2::ZERO,
            focus: Focus::None,
            pending_focus: None,
            lost_focus: Focus::None,
            tab_nav_list: Vec::new(),
        }
    }

    pub fn grabbed(&self) -> bool {
        self.v_scrollbar_grabbed || self.focus.is_slider()
    }

    pub fn get_tab(&self, key: &str) -> Option<usize> {
        self.tabs.get(key).copied()
    }

    /// Start a new frame. Returns any action returned by a dialog.
    pub fn start_frame(&mut self, conf: &Config) -> Option<Action> {
        self.bounds = Rect {
            x: 0.0,
            y: 0.0,
            w: screen_width(),
            h: screen_height(),
        };

        self.cursor_x = self.style.margin;
        self.cursor_y = self.style.margin;
        self.cursor_z = 0;

        if !is_mouse_button_down(MouseButton::Left)
            && !is_mouse_button_released(MouseButton::Left) {
            self.mouse_consumed = None;
        }

        if self.focus.is_slider() && is_mouse_button_released(MouseButton::Left) {
            self.focus = Focus::None;
        }
        self.tab_nav_list.clear();

        clear_background(self.style.theme.panel_bg());

        self.info_box(conf);
        self.info = Info::None;
        self.ctrl_info = ControlInfo::None;
        self.handle_dialog()
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
        self.start_raw_group();
    }

    fn start_raw_group(&mut self) {
        self.group_rects.push(Rect {
            x: self.cursor_x,
            y: self.cursor_y,
            w: 0.0,
            h: 0.0,
        });
    }

    fn end_raw_group(&mut self) -> Option<Rect> {
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

    /// A widget is a group that doesn't change the layout axis, and may have
    /// info text.
    fn start_widget(&mut self) {
        self.widget_on_stack = true;
        self.start_raw_group();
    }

    /// Ending a group changes the layout axis and offsets the cursor along the
    /// new axis by the width or height of the graphics in the group.
    pub fn end_group(&mut self) -> Option<Rect> {
        if !self.group_rects.is_empty() {
            self.flip_layout();
        }
        self.end_raw_group()
    }

    /// End a widget group, returning the occupied rect.
    pub fn end_widget(&mut self, id: &str, info: Info, ctrl_info: ControlInfo
    ) -> Option<Rect> {
        self.widget_on_stack = false;
        let rect = self.end_raw_group();
        if let Some(rect) = rect {
            if self.mouse_hits(rect, id) {
                self.info = info;
                self.ctrl_info = ctrl_info;
            }
        }
        rect
    }

    pub fn end_frame(&mut self, tab_nav: bool) {
        self.draw_queue.sort_by_key(|x| x.z);
        let screen_rect = Rect::new(0.0, 0.0, screen_width(), screen_height());
        for op in &self.draw_queue {
            if op.graphic.overlaps(&self.style, &screen_rect) {
                op.graphic.draw(&self.style);
            }
        }
        self.draw_queue.clear();

        // drain input queues
        while get_char_pressed().is_some() {}
        self.note_queue.clear();

        self.bottom_right_corner = Vec2 {
            x: self.bounds.x + self.bounds.w,
            y: self.bounds.y + self.bounds.h,
        };

        if tab_nav && is_key_pressed(KeyCode::Tab) && !is_alt_down() && !is_ctrl_down() {
            if is_shift_down() {
                self.tab_focus(-1);
            } else {
                self.tab_focus(1);
            }
        }
    }

    fn tab_focus(&mut self, offset: isize) {
        if self.tab_nav_list.is_empty() {
            return
        }

        // the "next field" should be determined by position, not order of addition
        self.tab_nav_list.sort_by_key(|(v, _)| (v.y as i32, v.x as i32));

        let index = self.focus.id()
            .and_then(|id| self.tab_nav_list.iter().position(|(_, s)| s == id));

        if let Some(index) = index {
            let index = (index as isize + offset)
                .rem_euclid(self.tab_nav_list.len() as isize);
            self.pending_focus = Some(self.tab_nav_list[index as usize].1.clone());
        } else {
            self.pending_focus = self.tab_nav_list.first().cloned().map(|x| x.1);
        }
    }

    fn space(&mut self, scale: f32) {
        match self.layout {
            Layout::Horizontal => self.cursor_x += self.style.margin * scale,
            Layout::Vertical => self.cursor_y += self.style.margin * scale,
        }
    }

    pub fn vertical_space(&mut self) {
        self.space(2.0);
    }

    fn push_graphic(&mut self, graphic: Graphic) {
        let (x, y) = match &graphic {
            Graphic::Line(x1, y1, x2, y2, _) => (x1.max(*x2), y1.max(*y2)),
            Graphic::Rect(rect, _, stroke) => if stroke.is_some() {
                (rect.x + rect.w, rect.y + rect.h)
            } else {
                (rect.x, rect.y)
            },
            Graphic::Text(x, y, text, _) => (
                x + self.style.atlas.text_width(text) + self.style.margin * 2.0,
                y + self.style.line_height()
            ),
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
        let rect = Rect {
            x,
            y,
            w: self.style.atlas.text_width(&text) + self.style.margin * 2.0,
            h: self.style.line_height(),
        };
        self.push_graphic(Graphic::Text(x, y, text, color));
        rect
    }

    fn bottom_panel_height(&self) -> f32 {
        self.style.line_height() + self.style.margin * 2.0
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
        if !is_shift_down() && !is_ctrl_down() {
            let (_, y_scroll) = mouse_wheel();
            if y_scroll != 0.0 {
                let increment = if is_alt_down() {
                    viewport_h / 2.0
                } else {
                    self.style.line_height() * 3.0
                };
                *current_y += -y_scroll.signum() * increment;
            }
        }

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

        let w = self.style.margin * 2.0;
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
        let hit = self.mouse_hits(trough, "vertical_scrollbar");
        self.push_rect(handle, self.style.theme.control_bg_click(), None);

        if hit {
            self.info = Info::VerticalScrollbar;
            if is_mouse_button_pressed(MouseButton::Left) {
                self.v_scrollbar_grabbed = true;
            }
        }

        if is_mouse_button_down(MouseButton::Left) && (self.v_scrollbar_grabbed || hit) {
            let (_, y) = mouse_position();
            let offset = ((y - trough.y - handle.h / 2.0) / (trough.h - handle.h))
                .clamp(0.0, 1.0);
            *current_y = ((max_y - viewport_h) * offset).round();
        } else {
            self.v_scrollbar_grabbed = false;
        }

        self.bounds.w -= w;
    }

    pub fn horizontal_scroll(&mut self,
         current_x: &mut f32, max_x: f32, viewport_w: f32
    ) {
        if is_shift_down() && !is_ctrl_down() {
            let (_, y_scroll) = mouse_wheel();
            if y_scroll != 0.0 {
                let increment = self.style.line_height() * 3.0;
                let dx = -y_scroll.signum() * increment;
                *current_x += dx;
            }
        }

        *current_x = (*current_x).min(max_x - viewport_w).max(0.0);

        if viewport_w >= max_x {
            return // no need to draw scrollbar
        }

        let h = self.style.margin * 2.0;
        let trough = Rect {
            x: self.bounds.x,
            y: self.bounds.y + self.bounds.h - h,
            w: viewport_w,
            h,
        };
        self.push_rect(trough, self.style.theme.control_bg(), None);

        let w = clamp(viewport_w / max_x, 0.0, 1.0) * trough.w;
        let handle = Rect {
            x: trough.x + (trough.w - w) * *current_x / (max_x - viewport_w),
            w,
            ..trough
        };
        let hit = self.mouse_hits(trough, "horizontal_scrollbar");
        self.push_rect(handle, self.style.theme.control_bg_click(), None);

        if hit {
            self.info = Info::HorizontalScrollbar;
            if is_mouse_button_pressed(MouseButton::Left) {
                self.h_scrollbar_grabbed = true;
            }
        }

        if is_mouse_button_down(MouseButton::Left) && (self.h_scrollbar_grabbed || hit) {
            let (x, _) = mouse_position();
            let offset = ((x - trough.x - handle.w / 2.0) / (trough.w - handle.w))
                .clamp(0.0, 1.0);
            *current_x = ((max_x - viewport_w) * offset).round();
        } else {
            self.h_scrollbar_grabbed = false;
        }

        self.bounds.h -= h;
    }

    /// Check whether the mouse is within the rect and unoccluded.
    fn mouse_hits(&self, rect: Rect, id: &str) -> bool {
        if self.mouse_consumed.as_ref().is_some_and(|s| s != id) {
            return false
        }

        let pt = mouse_position_vec2();

        // occlusion by combo box
        if let Focus::ComboBox(state) = &self.focus {
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
    pub fn label(&mut self, label: &str, info: Info) {
        self.colored_label(label, info, self.style.theme.fg());
    }

    pub fn colored_label(&mut self, label: &str, info: Info, color: Color) {
        self.start_widget();
        self.push_text(self.cursor_x, self.cursor_y, label.to_owned(), color);
        self.end_widget("label", info, ControlInfo::None);
    }

    /// An offset label is a label offset in the y direction to align with
    /// control labels.
    pub fn offset_label(&mut self, label: &str, info: Info) {
        self.start_widget();
        self.push_text(self.cursor_x, self.cursor_y + self.style.margin,
            label.to_owned(), self.style.theme.fg());
        self.end_widget("label", info, ControlInfo::None);
    }

    pub fn header(&mut self, label: &str, info: Info) {
        let rect = Rect {
            x: self.cursor_x,
            y: self.cursor_y,
            w: self.bounds.w + self.bounds.x - self.cursor_x,
            h: self.style.line_height(),
        };
        self.start_widget();
        self.push_rect(rect, self.style.theme.accent1_bg(), None);
        self.push_text(self.cursor_x, self.cursor_y,
            label.to_owned(), self.style.theme.fg());
        self.end_widget("header", info, ControlInfo::None);
    }

    fn text_rect(&mut self, label: &str, enabled: bool, x: f32, y: f32,
        bg: &Color, bg_hover: &Color, bg_click: &Color,
    ) -> (Rect, MouseEvent) {
        let id = "text_rect_".to_string() + label;

        let rect = Rect {
            x,
            y,
            w: self.style.atlas.text_width(label) + self.style.margin * 2.0,
            h: self.style.line_height(),
        };
        let mouse_hit = self.mouse_hits(rect, &id) && enabled;

        // draw fill based on mouse state
        let (fill, stroke) = if mouse_hit {
            (if is_mouse_button_down(MouseButton::Left) {
                bg_click
            } else {
                bg_hover
            }, self.style.theme.border_focused())
        } else if enabled {
            (bg, self.style.theme.border_unfocused())
        } else {
            (&self.style.theme.panel_bg(), self.style.theme.border_disabled())
        };

        self.push_rect(rect, *fill, Some(stroke));
        self.push_text(x, y, label.to_owned(), if enabled {
            self.style.theme.fg()
        } else {
            self.style.theme.border_disabled()
        });

        (rect, if mouse_hit && is_mouse_button_pressed(MouseButton::Left) {
            self.mouse_consumed = Some(id);
            MouseEvent::Pressed
        } else if mouse_hit && is_mouse_button_released(MouseButton::Left) {
            self.mouse_consumed = Some(id);
            MouseEvent::Released
        } else {
            MouseEvent::None
        })
    }

    /// Draws a button and returns true if it was clicked this frame.
    pub fn button(&mut self, label: &str, enabled: bool, info: Info) -> bool {
        self.start_widget();

        let (_, event) = self.text_rect(label, enabled,
            self.cursor_x + self.style.margin, self.cursor_y + self.style.margin,
            &self.style.theme.control_bg(),
            &self.style.theme.control_bg_hover(),
            &self.style.theme.control_bg_click());

        self.end_widget("button", info, ControlInfo::None);
        event == MouseEvent::Released
    }

    /// Draws a checkbox and returns true if it was changed this frame.
    pub fn checkbox(&mut self, label: &str, value: &mut bool, enabled: bool, info: Info
    ) -> bool {
        let button_text = if *value { "X" } else { " " };
        self.start_widget();
        let (rect, event) = self.text_rect(button_text, enabled,
            self.cursor_x + self.style.margin, self.cursor_y + self.style.margin,
            &self.style.theme.content_bg(),
            &self.style.theme.content_bg(),
            &self.style.theme.content_bg());
        let clicked = event == MouseEvent::Released;
        self.push_text(self.cursor_x + rect.w + self.style.margin,
            self.cursor_y + self.style.margin,
            label.to_owned(), if enabled {
                self.style.theme.fg()
            } else {
                self.style.theme.border_disabled()
            });
        if clicked {
            *value = !*value;
        }
        self.end_widget("checkbox", info, ControlInfo::None);
        clicked
    }

    /// Draws a combo box. If a value was selected this frame, returns the value's index.
    pub fn combo_box(&mut self, id: &str, label: &str, button_text: &str,
        info: Info, get_options: impl Fn() -> Vec<String>
    ) -> Option<usize> {
        self.start_widget();
        let margin = self.style.margin;

        // draw button and label
        let (button_rect, event) = self.text_rect(button_text, true,
            self.cursor_x + margin, self.cursor_y + margin,
            &self.style.theme.control_bg(),
            &self.style.theme.control_bg_hover(),
            &self.style.theme.control_bg_click());
        if !label.is_empty() {
            self.push_text(self.cursor_x + button_rect.w + margin,
                self.cursor_y + margin, label.to_owned(), self.style.theme.fg());
        }

        // check to open list
        let open = match &self.focus {
            Focus::ComboBox(state) => state.id == id,
            _ => false,
        };
        if event == MouseEvent::Pressed && !open {
            let options = get_options();
            let list_rect = combo_box_list_rect(&self.style, button_rect, &options);
            self.set_focus(Focus::ComboBox(ComboBoxState {
                id: id.to_owned(),
                options,
                button_rect,
                list_rect,
            }));
        }

        let return_val = if open {
            if let Focus::ComboBox(state) = &mut self.focus {
                state.button_rect = button_rect;
                state.list_rect =
                    combo_box_list_rect(&self.style, button_rect, &state.options);
            }
            self.combo_box_list(open, info.clone())
        } else {
            None
        };

        // check to close. other close conditions are in combo_box_list()
        if open && (is_key_pressed(KeyCode::Escape) ||
            (is_mouse_button_pressed(MouseButton::Left)
                && button_rect.contains(mouse_position_vec2()))
        ) {
            self.focus = Focus::None;
        }

        self.end_widget(id, info, ControlInfo::None);
        return_val
    }

    /// Draw the list of the active combo box.
    fn combo_box_list(&mut self, already_open: bool, info: Info) -> Option<usize> {
        self.cursor_z += COMBO_Z_OFFSET;
        if let Focus::ComboBox(state) = &self.focus {
            let mut gfx = vec![
                Graphic::Rect(state.list_rect, self.style.theme.panel_bg(),
                    Some(self.style.theme.border_unfocused()))
            ];
            if state.list_rect.contains(mouse_position_vec2()) {
                self.info = info;
            }

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
                    gfx.push(Graphic::Rect(
                        hit_rect, self.style.theme.panel_bg_hover(), None));
                    if lmb {
                        return_val = Some(i);
                        self.mouse_consumed = Some(state.id.clone());
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
                self.focus = Focus::None;
            }

            self.push_graphics(gfx);
            self.cursor_z -= COMBO_Z_OFFSET;

            return_val
        } else {
            None
        }
    }

    /// Draws a tab menu. Returns the index of the selected tab.
    pub fn tab_menu(&mut self, id: &str, labels: &[&str], version: &str) -> usize {
        if !self.tabs.contains_key(id) {
            self.tabs.insert(id.to_owned(), 0);
        }

        let mut selected_index = self.tabs.get(id).cloned().unwrap_or_default();
        let mut x = self.cursor_x + 1.0;
        let h = self.style.line_height();
        let mut gfx = vec![
            Graphic::Rect(Rect {
                x: self.bounds.x,
                y: self.cursor_y,
                w: self.bounds.w,
                h,
            }, self.style.theme.panel_bg_hover(), None),
            Graphic::Line(self.bounds.x, self.cursor_y + h + LINE_THICKNESS * 0.5,
                self.bounds.w, self.cursor_y + h + LINE_THICKNESS * 0.5,
                self.style.theme.border_unfocused())
        ];
        for (i, label) in labels.iter().enumerate() {
            let r = Rect {
                x,
                y: self.cursor_y,
                w: self.style.atlas.text_width(label) + self.style.margin * 2.0,
                h,
            };
            // fill background
            let color = if i == selected_index {
                self.style.theme.panel_bg()
            } else if self.mouse_hits(r, "tab_menu") {
                if is_mouse_button_pressed(MouseButton::Left) {
                    self.tabs.insert(id.to_owned(), i);
                    self.unfocus();
                    selected_index = i;
                }
                self.style.theme.panel_bg_click()
            } else {
                self.style.theme.panel_bg_hover()
            };
            gfx.push(Graphic::Rect(Rect {w: r.w, ..r }, color, None));
            gfx.push(Graphic::Text(x, self.cursor_y,
                label.to_string(), self.style.theme.fg()));
            if i == 0 {
                gfx.push(Graphic::Line(x - LINE_THICKNESS * 0.5, self.cursor_y,
                    x - LINE_THICKNESS *0.5, self.cursor_y + r.h,
                    self.style.theme.border_unfocused()));
            }
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
        self.push_graphics(gfx);
        {
            let w = self.style.atlas.text_width(version) + self.style.margin * 2.0;
            let x = self.bounds.x + self.bounds.w - w;
            self.push_text(x, self.cursor_y, version.to_owned(),
                self.style.theme.border_unfocused());
        }
        let h = self.style.line_height() + LINE_THICKNESS;
        self.cursor_y += h;
        self.bounds.y += h;
        self.bounds.h -= h;
        selected_index
    }

    /// Unfocus all controls.
    fn unfocus(&mut self) {
        self.focus = Focus::None;
        self.instrument_edit_index = None;
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
        range: RangeInclusive<f32>, unit: Option<&'static str>, power: i32, enabled: bool,
        info: Info
    ) -> bool {
        self.formatted_slider(id, label, val, range, power, enabled, info,
            display_unit(unit), |x| x)
    }

    pub fn formatted_slider(&mut self, id: &str, label: &str, val: &mut f32,
        range: RangeInclusive<f32>, power: i32, enabled: bool, info: Info,
        display: impl Fn(f32) -> String, convert: impl FnOnce(f32) -> f32,
    ) -> bool {
        // are we in text entry mode?
        if let Focus::Text(state) = &self.focus {
            if state.id == id {
                return self.slider_text_entry(id, label, val, range, convert);
            }
        }

        self.start_widget();
        let h = self.style.atlas.cap_height();

        // draw groove
        let groove_w = SLIDER_WIDTH;
        let groove_x = self.cursor_x + self.style.margin * 2.0;
        let groove_y = (self.cursor_y + self.style.margin * 2.0 + h * 0.5).round() + 0.5;

        // get/set grabbed state
        let hit_rect = Rect {
            x: self.cursor_x + self.style.margin,
            w: groove_w + self.style.margin * 2.0,
            y: self.cursor_y + self.style.margin,
            h: h + self.style.margin * 2.0,
        };
        let mouse_pos = mouse_position_vec2();
        let hit = enabled && self.mouse_hits(hit_rect, id);
        if hit {
            if is_mouse_button_pressed(MouseButton::Left) {
                self.set_focus(Focus::Slider(id.to_string()));
                self.mouse_consumed = Some(id.to_string());
            }
            if is_mouse_button_pressed(MouseButton::Right) {
                let text = display(*val).trim_start_matches('x')
                    .split([' ', ':']).next()
                    .expect("at least 1 token should be present")
                    .to_owned();
                self.set_focus(Focus::Text(TextEditState::new(id.to_owned(), text)));
            }
        }
        let grabbed = if let Focus::Slider(s) = &self.focus {
            s == id
        } else {
            false
        };

        // update position, get handle color
        let (fill, stroke, mut changed) = if grabbed {
            let f = ((mouse_pos.x - groove_x) / groove_w).max(0.0).powi(power);
            let new_val = interpolate(f, &range)
                .max(*range.start())
                .min(*range.end());
            let changed = new_val != *val;
            *val = new_val;
            (self.style.theme.control_bg_click(), self.style.theme.border_focused(),
                changed)
        } else if hit {
            (self.style.theme.control_bg_hover(), self.style.theme.border_focused(), false)
        } else if enabled {
            (self.style.theme.control_bg(), self.style.theme.border_unfocused(), false)
        } else {
            (self.style.theme.panel_bg(), self.style.theme.border_disabled(), false)
        };

        // draw groove & handle
        self.push_line(groove_x, groove_y, groove_x + groove_w, groove_y, stroke);
        let f = deinterpolate(*val, &range).powf(1.0/power as f32);
        let handle_rect = Rect {
            x: self.cursor_x + self.style.margin + (f * groove_w).round(),
            w: self.style.margin * 2.0,
            ..hit_rect
        };
        self.push_rect(handle_rect, fill, Some(stroke));

        // draw label
        let x = self.cursor_x + self.style.margin * 3.0 + groove_w;
        let y = self.cursor_y + self.style.margin;
        if !label.is_empty() {
            self.push_text(x, y, label.to_owned(), self.style.theme.fg());
        } else {
            // push an invisible rect to reserve space for the handle
            let r = Rect { x, y, w: 0.0, h: 0.0 };
            self.push_rect(r, Color { a: 0.0, ..Default::default() }, None);
        };

        if hit || grabbed {
            let text = display(*val);
            self.tooltip(&text, handle_rect.x,
                self.cursor_y - (h + self.style.margin * 2.0));
        }

        // TODO: duplication with slider_text_entry
        match &self.lost_focus {
            Focus::Text(state) if state.id == id => {
                match state.text.parse::<f32>() {
                    Ok(f) => {
                        *val = convert(f).max(*range.start()).min(*range.end());
                        changed = true;
                    },
                    Err(e) => self.report(e),
                }
                self.focus = Focus::None;
            }
            _ => ()
        }

        self.end_widget(id, info, ControlInfo::Slider);
        changed
    }

    fn slider_text_entry(&mut self, id: &str, label: &str, val: &mut f32,
        range: RangeInclusive<f32>, convert: impl FnOnce(f32) -> f32,
    ) -> bool {
        let text = if let Focus::Text(state) = &mut self.focus {
            state.text.clone()
        } else {
            panic!("no focused text")
        };

        let mut changed = false;
        let w = SLIDER_WIDTH + self.style.margin * 2.0;
        if self.text_box(id, label, w, &text, 10, Info::None) {
            match text.parse::<f32>() {
                Ok(f) => {
                    *val = convert(f).max(*range.start()).min(*range.end());
                    changed = true;
                },
                Err(e) => self.report(e),
            }
            self.focus = Focus::None;
        }
        changed
    }

    pub fn color_table(&mut self, colors: Vec<Color>) {
        let dim = self.style.line_height();
        let margin = self.style.margin;
        self.start_widget();

        let rect = Rect {
            x: self.cursor_x + margin - 1.0,
            y: self.cursor_y + margin - 1.0,
            w: dim * colors.len() as f32 + 2.0,
            h: dim + 2.0,
        };
        self.push_rect(rect, WHITE, Some(self.style.theme.border_unfocused()));

        for (i, fill) in colors.into_iter().enumerate() {
            let rect = Rect {
                x: self.cursor_x + margin + i as f32 * dim,
                y: self.cursor_y + margin,
                w: dim,
                h: dim,
            };
            self.push_rect(rect, fill, None);
        }

        self.end_widget("color_table", Info::None, ControlInfo::None);
    }

    fn cursor_vec(&self) -> Vec2 {
        Vec2::new(self.cursor_x, self.cursor_y)
    }

    /// Widget for editing a value as text.
    pub fn edit_box(&mut self, label: &str, chars_wide: usize,
        mut text: String, info: Info
    ) -> Option<String> {
        self.tab_nav_list.push((self.cursor_vec(), label.to_string()));

        let w = chars_wide as f32 * self.style.atlas.char_width()
            + self.style.margin * 2.0;

        let mut result = match &self.lost_focus {
            Focus::Text(state) if state.id == label => {
                let s = state.text.clone();
                text = s.clone();
                self.lost_focus = Focus::None;
                Some(s)
            }
            _ => None,
        };

        if self.text_box(label, label, w, &text, chars_wide, info) {
            if let Focus::Text(state) = &self.focus {
                let s = state.text.clone();
                self.focus = Focus::None;
                result = Some(s)
            } else {
                panic!("no focused text");
            }
        }

        result
    }

    /// Returns true if the text was submitted (i.e. Enter was pressed).
    fn text_box(&mut self, id: &str, label: &str, width: f32, text: &str, max_width: usize,
        info: Info
    ) -> bool {
        match &self.pending_focus {
            Some(s) if s == id => {
                let f = Focus::Text(TextEditState::new(id.to_owned(), text.to_owned()));
                self.set_focus(f);
            }
            _ => (),
        }

        let box_rect = Rect {
            x: self.cursor_x + self.style.margin,
            y: self.cursor_y + self.style.margin,
            w: width,
            h: self.style.line_height(),
        };

        let focused = match &self.focus {
            Focus::Text(state) => state.id == id,
            _ => false,
        };
        let hit = self.mouse_hits(box_rect, id);

        // focus/unfocus
        if !focused && hit && is_mouse_button_pressed(MouseButton::Left) {
            let f = Focus::Text(TextEditState::new(id.to_owned(), text.to_owned()));
            self.set_focus(f);
            self.mouse_consumed = Some(id.to_string());
        } else if focused && is_key_pressed(KeyCode::Escape) {
            self.focus = Focus::None;
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
            self.editable_text(box_rect, max_width, max_width)
        } else {
            self.push_text(box_rect.x, box_rect.y, text.to_string(),
                self.style.theme.fg());
            false
        };

        // draw label
        if !label.is_empty() {
            self.push_text(box_rect.x + box_rect.w, self.cursor_y + self.style.margin,
                label.to_owned(), self.style.theme.fg());
        }

        self.end_widget(id, info, ControlInfo::None);
        submit
    }

    /// List box with editable values. Returns a string when an edit is submitted.
    pub fn instrument_list(&mut self, options: &[String], index: &mut usize,
        min_chars: usize,
    ) -> Option<String> {
        const TEXT_ID: &str = "instrument_list";
        let pointer = String::from(char::from_u32(0xbb).unwrap());

        let margin = self.style.margin;
        let atlas = &self.style.atlas;
        let line_height = self.style.line_height();
        let char_width = self.style.atlas.char_width();
        let list_rect = Rect {
            x: self.cursor_x + margin,
            y: self.cursor_y + margin,
            w: options.iter().fold(0.0_f32, |w, s| w.max(atlas.text_width(s)))
                .max(atlas.char_width() * min_chars as f32)
                .max(match &self.focus {
                    Focus::Text(state) if state.id == TEXT_ID =>
                        atlas.text_width(&state.text),
                    _ => 0.0,
                })
                + margin * 2.0 + char_width,
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
        for (i, option) in options.iter().enumerate() {
            if i == *index {
                self.push_rect(hit_rect, self.style.theme.content_bg_click(), None);
                self.push_text(list_rect.x, hit_rect.y,
                    pointer.clone(), self.style.theme.fg());
            } else if self.mouse_hits(hit_rect, "instrument_list") {
                self.push_rect(hit_rect, self.style.theme.content_bg_hover(), None);
                if lmb {
                    *index = i;
                }
            }

            // check for unfocus
            let mut option = option.clone();
            if Some(i) == self.instrument_edit_index {
                if is_key_pressed(KeyCode::Escape) {
                    self.focus = Focus::None;
                    self.instrument_edit_index = None;
                }
                match &self.focus {
                    Focus::Text(state) if state.id == TEXT_ID => (),
                    _ => {
                        self.instrument_edit_index = None;
                        match &self.lost_focus {
                            Focus::Text(state) if state.id == TEXT_ID => {
                                option = state.text.clone();
                                return_val = Some(option.clone());
                                self.lost_focus = Focus::None;
                            }
                            _ => (),
                        }
                    }
                }
            }

            if Some(i) == self.instrument_edit_index {
                let rect = Rect {
                    x: list_rect.x + char_width,
                    w: hit_rect.w - char_width,
                    ..hit_rect
                };
                if self.editable_text(rect, MAX_PATCH_NAME_CHARS, MAX_PATCH_NAME_CHARS) {
                    if let Focus::Text(state) = &mut self.focus {
                        return_val = Some(state.text.clone());
                        self.focus = Focus::None;
                        self.instrument_edit_index = None;
                    }
                }
            } else {
                if self.mouse_hits(hit_rect, "instrument_list")
                    && is_mouse_button_pressed(MouseButton::Right) && i > 0 {
                    let text = option.clone();
                    let f = Focus::Text(TextEditState::new(TEXT_ID.to_string(), text));
                    self.set_focus(f);
                    self.instrument_edit_index = Some(i);
                    *index = i;
                }
                self.push_text(list_rect.x + char_width, hit_rect.y,
                    option, self.style.theme.fg());
            }
            hit_rect.y += hit_rect.h;
        }

        self.end_widget("instrument_list", Info::InstrumentList, ControlInfo::None);
        return_val
    }

    /// Focus a new text field.
    fn focus_text(&mut self, id: String, text: String) {
        self.set_focus(Focus::Text(TextEditState::new(id, text)));
    }

    /// Transient text edit for use in pattern grid.
    fn pattern_edit_box(&mut self, id: &str, rect: Rect, max_width: usize, margin: f32,
        force_submit: bool,
    ) -> Option<String> {
        if is_key_pressed(KeyCode::Escape) {
            self.focus = Focus::None;
            return Some("".into())
        } else if self.focus.id() != Some(id) {
            return Some("".into())
        }

        self.push_rect(rect, self.style.theme.control_bg(), None);

        let rect = Rect {
            x: rect.x - self.style.margin,
            y: rect.y + margin - self.style.margin,
            ..rect
        };

        if self.editable_text(rect, max_width * 2, max_width) || force_submit {
            if let Focus::Text(te) = &self.focus {
                let s = te.text.clone();
                self.focus = Focus::None;
                return Some(s)
            }
        }

        None
    }

    /// Primitive that draws the currently focused text and handles edit input.
    fn editable_text(&mut self, rect: Rect, max_width: usize, display_width: usize
    ) -> bool {
        const ID: &str = "editable_text";

        let hit = self.mouse_hits(rect, ID);
        let margin = self.style.margin;

        if let Focus::Text(state) = &mut self.focus {
            let char_w = self.style.atlas.char_width();
            let mouse_i = if hit {
                Some(((mouse_position_vec2().x - rect.x - margin) / char_w)
                    .max(0.0).round() as usize)
            } else {
                None
            };
            state.handle_input(mouse_i, &mut self.text_clipboard, max_width);

            let text_h = self.style.atlas.cap_height();
            let f = |i: usize| rect.x + char_w * i.min(display_width) as f32
                + margin + LINE_THICKNESS * 0.5;
            let cursor_x = f(state.cursor);
            let y1 = rect.y + margin - 1.0;
            let y2 = rect.y + margin + text_h + 1.0;
            let text = state.text.clone();
            let len = state.len();

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
            let text = if len <= display_width {
                text
            } else {
                text.chars().skip(len - display_width).collect()
            };
            self.push_text(rect.x, rect.y, text, self.style.theme.fg());
        }

        let mouse_off = !hit && is_mouse_button_pressed(MouseButton::Left);
        is_key_pressed(KeyCode::Enter) || mouse_off
    }

    pub fn shared_slider(&mut self, id: &str, label: &str, param: &Shared,
        range: RangeInclusive<f32>, unit: Option<&'static str>, power: i32, enabled: bool,
        info: Info,
    ) {
        self.formatted_shared_slider(id, label, param, range, power, enabled, info,
            display_unit(unit), |x| x);
    }

    pub fn formatted_shared_slider(&mut self, id: &str, label: &str, param: &Shared,
        range: RangeInclusive<f32>, power: i32, enabled: bool, info: Info,
        display: impl Fn(f32) -> String, convert: impl FnOnce(f32) -> f32,
    ) {
        let mut val = param.value();
        if self.formatted_slider(id, label, &mut val, range, power, enabled, info,
            display, convert) {
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

    /// Prompt for confirmation before performing an action.
    pub fn confirm(&mut self, prompt: &str, action: Action) {
        self.dialog = Some(Dialog::OkCancel(prompt.to_owned(), action));
    }

    /// Temporarily use the info box to display a message.
    pub fn notify(&mut self, message: String) {
        self.notification = Some(Notification {
            time_remaining: 1.0 + message.chars().count() as f32 * 0.1,
            message,
        });
    }

    pub fn accepting_keyboard_input(&self) -> bool {
        matches!(self.focus, Focus::Text(_) | Focus::Hotkey(_))
    }

    pub fn accepting_note_input(&self) -> bool {
        matches!(self.focus, Focus::Note(_))
    }

    pub fn tooltip(&mut self, text: &str, x: f32, y: f32) {
        self.cursor_z += TOOLTIP_Z_OFFSET;
        self.text_rect(text, true, x, y,
            &self.style.theme.panel_bg(),
            &self.style.theme.panel_bg(),
            &self.style.theme.panel_bg());
        self.cursor_z -= TOOLTIP_Z_OFFSET;
    }

    /// Returns the key that set the new note value.
    pub fn note_input(&mut self, id: &str, note: &mut Note, info: Info) -> Option<Key> {
        let label = note.to_string();
        let margin = self.style.margin;

        let rect = Rect {
            x: self.cursor_x + margin,
            y: self.cursor_y + margin,
            w: self.style.atlas.text_width(&label) + margin * 2.0,
            h: self.style.line_height(),
        };
        let mouse_hit = self.mouse_hits(rect, id);

        if mouse_hit && is_mouse_button_pressed(MouseButton::Left) {
            self.set_focus(Focus::Note(id.to_owned()));
        }
        let focused = match &self.focus {
            Focus::Note(s) => s == id,
            _ => false,
        };

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
                    self.focus = Focus::None;
                    key = Some(k.clone());
                }
            }
        }

        self.start_widget();
        self.push_rect(rect, fill, Some(stroke));
        self.push_text(rect.x, rect.y, label, self.style.theme.fg());
        self.end_widget(id, info, ControlInfo::Note);

        key
    }

    // TODO: code duplication with note_input
    pub fn hotkey_input(&mut self, id: usize, hotkey: &mut Hotkey, info: Info) -> bool {
        let label = hotkey.to_string();
        let margin = self.style.margin;

        let rect = Rect {
            x: self.cursor_x + margin,
            y: self.cursor_y + margin,
            w: self.style.atlas.text_width(&label) + margin * 2.0,
            h: self.style.line_height(),
        };
        let mouse_hit = self.mouse_hits(rect, "hotkey_input");

        if mouse_hit && is_mouse_button_pressed(MouseButton::Left) {
            self.set_focus(Focus::Hotkey(id.to_owned()));
        }
        let focused = match &self.focus {
            Focus::Hotkey(s) => *s == id,
            _ => false,
        };

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
            let key = get_keys_pressed().into_iter().find(|x| !is_mod(*x));
            if let Some(key) = key {
                *hotkey = Hotkey::new(Modifiers::current(), key);
                self.focus = Focus::None;
                changed = true;
            }
        }

        self.start_widget();
        self.push_rect(rect, fill, Some(stroke));
        self.push_text(rect.x, rect.y, label, self.style.theme.fg());
        self.end_widget("hotkey_input", info, ControlInfo::Hotkey);

        changed
    }

    fn info_box(&mut self, conf: &Config) {
        // notification
        let mut note_expired = false;
        let text = if let Some(note) = &mut self.notification {
            note.time_remaining -= get_frame_time().min(0.1);
            note_expired = note.time_remaining <= 0.0;
            Some(note.message.clone())
        } else if self.info == self.saved_info.0 && self.ctrl_info == self.saved_info.1 {
            self.info_delay = (self.info_delay - get_frame_time()).max(0.0);
            if conf.display_info && self.info_delay == 0.0 {
                let s = info::text(&self.info, &self.ctrl_info, conf);
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            } else {
                None
            }
        } else {
            self.saved_info = (mem::take(&mut self.info), mem::take(&mut self.ctrl_info));
            self.info_delay = INFO_DELAY;
            None
        };
        if note_expired {
            self.notification = None;
        }

        if let Some(text) = text {
            let lines: Vec<_> = text.lines().collect();
            let w = self.style.atlas.char_width() * lines.iter()
                .map(|s| s.chars().count())
                .max()
                .unwrap_or_default() as f32 + self.style.margin * 2.0;
            let h = self.style.line_height() * lines.len() as f32;
            let mut rect = Rect {
                x: self.bottom_right_corner.x - w - self.style.margin,
                y: self.bottom_right_corner.y - h - self.style.margin,
                w,
                h,
            };

            // draw in left corner if mouse is over right corner
            if self.mouse_hits(rect, "info_box") {
                rect.x = self.bounds.x + self.style.margin;
            }

            self.cursor_z += TOOLTIP_Z_OFFSET;
            self.push_rect(rect, self.style.theme.panel_bg(),
                Some(self.style.theme.border_unfocused()));
            for (i, line) in lines.into_iter().enumerate() {
                self.push_text(
                    rect.x,
                    rect.y + self.style.line_height() * i as f32,
                    line.to_string(),
                    self.style.theme.fg());
            }
            self.cursor_z -= TOOLTIP_Z_OFFSET;
        }
    }

    /// Focus the control with the given ID.
    pub fn focus(&mut self, id: &str) {
        self.pending_focus = Some(id.to_owned());
    }

    fn set_focus(&mut self, focus: Focus) {
        self.lost_focus = mem::take(&mut self.focus);
        self.focus = focus;
        self.pending_focus = None;
    }

    /// Pushes a note to the draw list. The notation is drawn in the space of
    /// 4 characters.
    pub fn push_note_text(&mut self, x: f32, y: f32, note: &Note, color: Color) {
        let base = format!("{}{}{}{}", note.arrow_char(), note.nominal.char(),
            note.accidental_char(), note.equave);

        if (3..).contains(&note.arrows.abs()) {
            let s = text::digit_superscript(note.arrows.unsigned_abs()).to_string();
            self.push_text(x, y, s, color);
        }

        if (3..).contains(&note.sharps.abs()) {
            let s = text::digit_superscript(note.sharps.unsigned_abs()).to_string();
            self.push_text(x + self.style.atlas.char_width() * 2.0, y, s, color);
        }

        self.push_text(x, y, base, color);
    }

    fn handle_dialog(&mut self) -> Option<Action> {
        const ID: &str = "dialog";
        self.cursor_z += PANEL_Z_OFFSET;

        let mut close = false;
        let mut action = None;

        if let Some(dialog) = &self.dialog {
            match dialog {
                Dialog::Alert(s) => {
                    let s = s.clone();
                    let mut r = center(fit_strings(&self.style, &[s.clone()]));
                    r.h += self.style.margin;
                    self.push_rect(r, self.style.theme.panel_bg(),
                        Some(self.style.theme.border_unfocused()));
                    self.push_text(r.x, r.y, s, self.style.theme.fg());
                    close = is_key_pressed(KeyCode::Escape)
                        || (self.mouse_consumed.is_none()
                            && is_mouse_button_pressed(MouseButton::Left))
                }
                Dialog::OkCancel(s, a) => {
                    let a = *a;
                    if let Some(v) = self.ok_cancel_dialog(s.to_owned()) {
                        close = true;
                        if v {
                            action = Some(a);
                        }
                    }
                }
            };
        }

        if close {
            self.dialog = None;
            self.mouse_consumed = Some(ID.to_string());
        }

        self.cursor_z -= PANEL_Z_OFFSET;
        action
    }

    /// Returns Some(true) if OK, Some(false) if Cancel.
    fn ok_cancel_dialog(&mut self, prompt: String) -> Option<bool> {
        let margin = self.style.margin;
        let buttons_w = self.style.atlas.text_width("OKCancel") + margin * 5.0;
        let w = self.style.atlas.text_width(&prompt).max(buttons_w) + margin * 2.0;
        let h = self.style.line_height() * 2.0 + margin * 3.0;
        let rect = Rect {
            x: ((screen_width() - w) * 0.5).round(),
            y: ((screen_height() - h) * 0.5).round(),
            w, h
        };
        self.push_rect(rect, self.style.theme.panel_bg(),
            Some(self.style.theme.border_unfocused()));

        let old_cursor = (self.cursor_x, self.cursor_y);
        self.cursor_x = rect.x;
        self.cursor_y = rect.y;

        let mut result = None;

        self.layout = Layout::Vertical;
        self.offset_label(&prompt, Info::None);
        self.flip_layout();

        self.cursor_x = rect.x + rect.w - (buttons_w + margin * 2.0);
        if self.button("OK", true, Info::None) {
            result = Some(true);
        }
        if is_key_pressed(KeyCode::Enter) {
            result = Some(true);
        }

        if self.button("Cancel", true, Info::None) {
            result = Some(false);
        }
        if is_key_pressed(KeyCode::Escape) {
            result = Some(false);
        }

        (self.cursor_x, self.cursor_y) = old_cursor;

        result
    }
}

fn interpolate(x: f32, range: &RangeInclusive<f32>) -> f32 {
    range.start() + x * (range.end() - range.start())
}

fn deinterpolate(x: f32, range: &RangeInclusive<f32>) -> f32 {
    (x - range.start()) / (range.end() - range.start())
}

fn fit_strings(style: &Style, v: &[String]) -> Rect {
    let mut rect: Rect = Default::default();
    for s in v {
        rect.w = rect.w.max(style.atlas.text_width(s) + style.margin * 2.0);
        rect.h += style.atlas.cap_height() + style.margin;
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

fn combo_box_list_rect(style: &Style, button_rect: Rect, options: &[String]) -> Rect {
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
        w: options.iter().fold(0.0_f32,
            |w, s| w.max(style.atlas.text_width(s))) + style.margin * 2.0,
        h,
    }
}

struct Notification {
    message: String,
    time_remaining: f32,
}

fn is_mod(key: KeyCode) -> bool {
    matches!(key, KeyCode::LeftAlt | KeyCode::RightAlt
        | KeyCode::LeftControl | KeyCode::RightControl
        | KeyCode::LeftShift | KeyCode::RightShift
        | KeyCode::LeftSuper | KeyCode::RightSuper)
}

fn display_unit(unit: Option<&'static str>) -> Box<dyn Fn(f32) -> String> {
    if let Some(unit) = unit {
        let unit = unit.to_owned();
        Box::new(move |x| format!("{:.3} {}", x, unit))
    } else {
        Box::new(|x| format!("{:.3}", x))
    }
}

/// Returns true if either Shift key is down.
fn is_shift_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

/// Returns true if either Alt key is down.
fn is_alt_down() -> bool {
    is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt)
}

/// Returns true if either Ctrl key is down.
pub fn is_ctrl_down() -> bool {
    is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl)
}