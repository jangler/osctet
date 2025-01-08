//! Color themes.

use std::{error::Error, path::Path};

use macroquad::color::Color;
use palette::{FromColor, Lchuv, Srgb};
use serde::{Deserialize, Serialize};

const DEFAULT_ACCENT1_HUE: f32 = 180.0;
const DEFAULT_ACCENT2_HUE: f32 = -90.0;
const DEFAULT_ACCENT_CHROMA: f32 = 45.0;

const PANEL_L_OFFSET: f32 = 2.0;
const CONTROL_L_OFFSET: f32 = PANEL_L_OFFSET * 2.0;
const HOVER_L_OFFSET: f32 = 4.0;
const CLICK_L_OFFSET: f32 = HOVER_L_OFFSET * 2.0;
const ACCENT_L_OFFSET: f32 = 15.0;

const ACCENT_BG_CHROMA_MULTIPLIER: f32 = 1.0/3.0;

/// Color theme using four seed colors. Seed colors use the CIE L*C*uv h°uv
/// color space, which is cylindrical and perceptually uniform. (Although in
/// practice, we gamma correct.)
#[derive(Serialize, Deserialize, Clone)]
pub struct Theme {
    pub fg: Lchuv,
    pub bg: Lchuv,
    pub accent1: Lchuv,
    pub accent2: Lchuv, // TODO: use this for focused controls?
    pub gamma: f32,
}

impl Theme {
    /// Returns the default light theme.
    pub fn light(gamma: f32) -> Theme {
        Theme {
            fg: Lchuv::new(10.0, 0.0, 0.0),
            bg: Lchuv::new(95.0, 0.0, 0.0),
            accent1: Lchuv::new(50.0, DEFAULT_ACCENT_CHROMA, DEFAULT_ACCENT1_HUE),
            accent2: Lchuv::new(50.0, DEFAULT_ACCENT_CHROMA, DEFAULT_ACCENT2_HUE),
            gamma,
        }
    }

    /// Returns the default dark theme.
    pub fn dark(gamma: f32) -> Theme {
        Theme {
            fg: Lchuv::new(90.0, 0.0, 0.0),
            bg: Lchuv::new(5.0, 0.0, 0.0),
            accent1: Lchuv::new(50.0, DEFAULT_ACCENT_CHROMA, DEFAULT_ACCENT1_HUE),
            accent2: Lchuv::new(50.0, DEFAULT_ACCENT_CHROMA, DEFAULT_ACCENT2_HUE),
            gamma,
        }
    }

    /// Load theme from a file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let s = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&s)?)
    }

    /// Save theme to a file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let s = toml::to_string_pretty(self)?;
        Ok(std::fs::write(path, s)?)
    }

    fn is_light(&self) -> bool {
        self.bg.l >= 50.0
    }

    pub fn fg(&self) -> Color {
        self.color_from_lchuv(self.fg)
    }

    pub fn accent1_bg(&self) -> Color {
        let sign = if self.is_light() { -1.0 } else { 1.0 };
        let c = Lchuv::new(self.bg.l + sign * ACCENT_L_OFFSET,
            self.accent1.chroma * ACCENT_BG_CHROMA_MULTIPLIER, self.accent1.hue);
        self.color_from_lchuv(c)
    }

    pub fn accent1_fg(&self) -> Color {
        let sign = if self.is_light() { -1.0 } else { 1.0 };
        let c = Lchuv::new(self.fg.l - sign * ACCENT_L_OFFSET,
            self.accent1.chroma, self.accent1.hue);
        self.color_from_lchuv(c)
    }

    pub fn accent2_bg(&self) -> Color {
        let sign = if self.is_light() { -1.0 } else { 1.0 };
        let c = Lchuv::new(self.bg.l + sign * ACCENT_L_OFFSET,
            self.accent2.chroma * ACCENT_BG_CHROMA_MULTIPLIER, self.accent2.hue);
        self.color_from_lchuv(c)
    }

    pub fn accent2_fg(&self) -> Color {
        let sign = if self.is_light() { -1.0 } else { 1.0 };
        let c = Lchuv::new(self.fg.l - sign * ACCENT_L_OFFSET,
            self.accent2.chroma, self.accent2.hue);
        self.color_from_lchuv(c)
    }

    fn bg_plus(&self, offset: f32) -> Color {
        let sign = if self.is_light() { -1.0 } else { 1.0 };
        let bg = Lchuv::new(self.bg.l + sign * offset, self.bg.chroma, self.bg.hue);
        self.color_from_lchuv(bg)
    }

    pub fn content_bg(&self) -> Color {
        self.color_from_lchuv(self.bg)
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
        self.color_from_lchuv(c)
    }

    pub fn border_focused(&self) -> Color {
        self.fg()
    }

    pub fn border_disabled(&self) -> Color {
        self.control_bg_click()
    }

    fn color_from_lchuv(&self, lchuv: Lchuv) -> Color {
        let lchuv = Lchuv {
            l: (lchuv.l * 0.01).powf(1.0/self.gamma) * 100.0,
            ..lchuv
        };
        let rgb = Srgb::from_color(lchuv);
        Color::new(rgb.red, rgb.green, rgb.blue, 1.0)
    }

    pub fn gamma_table(&self) -> impl Iterator<Item = Color> + use<'_> {
        (0..=10).map(|i| self.color_from_lchuv(Lchuv::new(i as f32 * 10.0, 0.0, 0.0)))
    }

    pub fn color_table(&self) -> Vec<Color> {
        vec![
            self.fg(),
            self.border_unfocused(),
            self.control_bg_click(),
            self.content_bg(),
            self.accent1_fg(),
            self.accent1_bg(),
            self.accent2_fg(),
            self.accent2_bg(),
        ]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light(1.8)
    }
}