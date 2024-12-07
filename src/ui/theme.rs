//! Color themes.

use macroquad::color::Color;
use palette::{FromColor, Lchuv, Srgb};
use serde::{Deserialize, Serialize};

const DEFAULT_ACCENT1_HUE: f32 = 180.0;
const DEFAULT_ACCENT2_HUE: f32 = -90.0;
const DEFAULT_ACCENT_CHROMA: f32 = 60.0;

// turns out Lchuv is not actually perceptually uniform --
// these need to be scaled up in dark themes
const PANEL_L_OFFSET: f32 = 2.0;
const CONTROL_L_OFFSET: f32 = 4.0;
const HOVER_L_OFFSET: f32 = 3.0;
const CLICK_L_OFFSET: f32 = 6.0;
const ACCENT_L_OFFSET: f32 = 12.0;

const DARK_CONTRAST_SCALE: f32 = 2.25;

/// Color theme using four seed colors. Seed colors use the CIE L*C*uv h°uv
/// color space, which is cylindrical and perceptually uniform.
#[derive(Serialize, Deserialize, Clone)]
pub struct Theme {
    pub fg: Lchuv,
    pub bg: Lchuv,
    pub accent1: Lchuv,
    pub accent2: Lchuv, // TODO: use this for focused controls?
}

impl Theme {
    /// Returns the default light theme.
    pub fn light() -> Theme {
        Theme {
            fg: Lchuv::new(20.0, 0.0, 0.0),
            bg: Lchuv::new(100.0, 0.0, 0.0),
            accent1: Lchuv::new(50.0, DEFAULT_ACCENT_CHROMA, DEFAULT_ACCENT1_HUE),
            accent2: Lchuv::new(50.0, DEFAULT_ACCENT_CHROMA, DEFAULT_ACCENT2_HUE),
        }
    }

    /// Returns the default dark theme.
    pub fn dark() -> Theme {
        Theme {
            fg: Lchuv::new(90.0, 0.0, 0.0),
            bg: Lchuv::new(10.0, 0.0, 0.0),
            accent1: Lchuv::new(50.0, DEFAULT_ACCENT_CHROMA, DEFAULT_ACCENT1_HUE),
            accent2: Lchuv::new(50.0, DEFAULT_ACCENT_CHROMA, DEFAULT_ACCENT2_HUE),
        }
    }

    fn is_light(&self) -> bool {
        self.bg.l >= 50.0
    }

    pub fn fg(&self) -> Color {
        color_from_lchuv(self.fg)
    }

    pub fn accent1_bg(&self) -> Color {
        let sign = if self.is_light() { -1.0 } else { DARK_CONTRAST_SCALE };
        let c = Lchuv::new(self.bg.l + sign * ACCENT_L_OFFSET,
            self.accent1.chroma * 0.25, self.accent1.hue);
        color_from_lchuv(c)
    }

    pub fn accent1_fg(&self) -> Color {
        let sign = if self.is_light() { -DARK_CONTRAST_SCALE } else { 1.0 };
        let c = Lchuv::new(self.fg.l - sign * ACCENT_L_OFFSET,
            self.accent1.chroma, self.accent1.hue);
        color_from_lchuv(c)
    }

    pub fn accent2_bg(&self) -> Color {
        let sign = if self.is_light() { -1.0 } else { DARK_CONTRAST_SCALE };
        let c = Lchuv::new(self.bg.l + sign * ACCENT_L_OFFSET,
            self.accent2.chroma * 0.25, self.accent2.hue);
        color_from_lchuv(c)
    }

    pub fn accent2_fg(&self) -> Color {
        let sign = if self.is_light() { -DARK_CONTRAST_SCALE } else { 1.0 };
        let c = Lchuv::new(self.fg.l - sign * ACCENT_L_OFFSET,
            self.accent2.chroma, self.accent2.hue);
        color_from_lchuv(c)
    }

    fn bg_plus(&self, offset: f32) -> Color {
        let sign = if self.is_light() { -1.0 } else { DARK_CONTRAST_SCALE };
        let bg = Lchuv::new(self.bg.l + sign * offset, self.bg.chroma, self.bg.hue);
        color_from_lchuv(bg)
    }

    pub fn content_bg(&self) -> Color {
        color_from_lchuv(self.bg)
    }

    pub fn content_bg_hover(&self) -> Color {
        self.bg_plus(HOVER_L_OFFSET)
    }

    pub fn content_bg_click(&self) -> Color {
        self.bg_plus(CLICK_L_OFFSET)
    }

    pub fn panel_bg(&self) -> Color {
        self.bg_plus(PANEL_L_OFFSET)
    }

    pub fn panel_bg_hover(&self) -> Color {
        self.bg_plus(PANEL_L_OFFSET + HOVER_L_OFFSET)
    }

    pub fn panel_bg_click(&self) -> Color {
        self.bg_plus(PANEL_L_OFFSET + CLICK_L_OFFSET)
    }

    pub fn control_bg(&self) -> Color {
        self.bg_plus(CONTROL_L_OFFSET)
    }
    
    pub fn control_bg_hover(&self) -> Color {
        self.bg_plus(CONTROL_L_OFFSET + HOVER_L_OFFSET)
    }

    pub fn control_bg_click(&self) -> Color {
        self.bg_plus(CONTROL_L_OFFSET + CLICK_L_OFFSET)
    }

    pub fn border_unfocused(&self) -> Color {
        let c = Lchuv::new(
            (self.bg.l + self.fg.l) * 0.5,
            (self.bg.chroma + self.fg.chroma) * 0.5,
            self.bg.hue);
        color_from_lchuv(c)
    }

    pub fn border_focused(&self) -> Color {
        color_from_lchuv(self.fg)
    }
}

fn color_from_lchuv(lchuv: Lchuv) -> Color {
    let rgb = Srgb::from_color(lchuv);
    Color::new(rgb.red, rgb.green, rgb.blue, 1.0)
}