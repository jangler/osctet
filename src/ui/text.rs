//! Code for drawing text using bitmap fonts.

use std::{collections::HashMap, error::Error, fs::File, io::BufReader, path::Path};

use bdf_reader::{Bitmap, Font};
use macroquad::{color::Color, math::Rect, texture::{draw_texture, Texture2D}};

/// Maps characters to GPU textures.
pub struct GlyphAtlas {
    map: HashMap<char, Texture2D>,
    width: f32,
    height: f32,
    cap_height: f32,
    offset_y: f32,
}

impl GlyphAtlas {
    /// Creates a new atlas from the bytes of a BDF font.
    pub fn from_bdf_bytes(bytes: &[u8]) -> Result<Self, bdf_reader::Error> {
        let reader = BufReader::new(bytes);
        let font = Font::read(reader)?;
        Ok(Self::from_bdf(font))
    }

    /// Creates a new atlas from a BDF font file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let font = Font::read(reader)?;
        Ok(Self::from_bdf(font))
    }

    /// Creates a new atlas from a BDF font.
    fn from_bdf(font: Font) -> Self {
        let mut map = HashMap::new();
        let mut width = 0.0_f32;
        let mut height = 0.0_f32;

        for glyph in font.glyphs() {
            let texture = texture_from_bitmap(glyph.bitmap());
            height = height.max(texture.height());
            width = width.max(texture.width());
            match char::from_u32(glyph.encoding()) {
                Some(c) => { map.insert(c, texture); }
                None => eprintln!("invalid char encoding: {}", glyph.encoding())
            }
        }

        let (cap_height, offset_y) = if let Some(glyph) = font.glyph('X') {
            (count_bitmap_rows(glyph.bitmap()) as f32,
                -(first_bitmap_row(glyph.bitmap()) as f32))
        } else {
            (height, 0.0)
        };

        Self { map, width, height, cap_height, offset_y }
    }

    /// Draws `text` horizontally without wrapping. Returns the drawn area.
    pub fn draw_text(&self, x: f32, y: f32, text: &str, color: Color) -> Rect {
        let initial_x = x.round();
        let y = y.round() + self.offset_y;
        let mut x = initial_x;

        for char in text.chars() {
            if let Some(texture) = self.map.get(&char) {
                draw_texture(texture, x, y, color);
                x += texture.width();
            }
        }

        Rect {
            x: initial_x,
            y,
            w: x - initial_x,
            h: self.height,
        }
    }

    /// Returns the width of a single character.
    pub fn char_width(&self) -> f32 {
        self.width
    }

    /// Return the maximum height of a character.
    pub fn max_height(&self) -> f32 {
        self.height
    }

    /// Return the visual height of a capital Latin letter.
    pub fn cap_height(&self) -> f32 {
        self.cap_height
    }

    /// Returns the width of a string.
    pub fn text_width(&self, text: &str) -> f32 {
        self.width * text.chars().count() as f32
    }
}

/// Converts a BDF bitmap to a GPU texture.
fn texture_from_bitmap(bitmap: Bitmap) -> Texture2D {
    let mut rgba = Vec::new();
    rgba.reserve_exact(bitmap.width() * bitmap.height() * 4);

    for y in 0..bitmap.height() {
        for x in 0..bitmap.width() {
            if let Ok(true) = bitmap.get(x, y) {
                rgba.extend_from_slice(&[255, 255, 255, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    Texture2D::from_rgba8(bitmap.width() as u16, bitmap.height() as u16, &rgba)
}

/// Returns the number of non-blank rows in a bitmap.
fn count_bitmap_rows(bitmap: Bitmap) -> usize {
    (0..bitmap.height())
        .filter(|y| (0..bitmap.width()).any(|x| bitmap.get(x, *y).is_ok_and(|v| v)))
        .count()
}

fn first_bitmap_row(bitmap: Bitmap) -> usize {
    (0..bitmap.height())
        .position(|y| (0..bitmap.width()).any(|x| bitmap.get(x, y).is_ok_and(|v| v)))
        .unwrap_or_default()
}