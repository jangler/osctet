//! Code for drawing text using bitmap fonts.

use std::{collections::HashMap, io::BufReader};

use bdf_reader::{Bitmap, Font};
use macroquad::{color::Color, math::Rect, texture::{build_textures_atlas, draw_texture, Texture2D}};

// character codes -- these are invalid as character literals,
// so we use u32 and convert.

pub const SHARP: u32 = 0x81;
pub const DOUBLE_SHARP: u32 = 0x82;
pub const SUB_SHARP: u32 = 0x83;
pub const FLAT: u32 = 0x84;
pub const DOUBLE_FLAT: u32 = 0x85;
pub const SUB_FLAT: u32 = 0x86;
pub const UP: u32 = 0x87;
pub const DOUBLE_UP: u32 = 0x88;
pub const SUB_UP: u32 = 0x8a;
pub const DOWN: u32 = 0x8b;
pub const DOUBLE_DOWN: u32 = 0x8c;
pub const SUB_DOWN: u32 = 0x8e;
pub const SUP_3: u32 = 0x8f;
pub const SUP_4: u32 = 0x90;
pub const SUP_5: u32 = 0x91;
pub const SUP_6: u32 = 0x92;
pub const SUP_7: u32 = 0x93;
pub const SUP_8: u32 = 0x94;
pub const SUP_9: u32 = 0x95;
pub const SUP_QUESTION: u32 = 0x96;

/// Bytes of included font files.
pub const FONT_BYTES: [&[u8]; 4] = [
    include_bytes!("../../font/DinaMedium-8.bdf"),
    include_bytes!("../../font/DinaMedium-10.bdf"),
    include_bytes!("../../font/DinaMedium-12.bdf"),
    include_bytes!("../../font/DinaMedium-13.bdf"),
];

/// Returns the character code for a superscript digit.
pub fn digit_superscript(digit: u8) -> char {
    char::from_u32(match digit {
        3 => SUP_3,
        4 => SUP_4,
        5 => SUP_5,
        6 => SUP_6,
        7 => SUP_7,
        8 => SUP_8,
        9 => SUP_9,
        _ => SUP_QUESTION,
    }).expect("code point constants should be valid")
}

/// Maps characters to GPU textures.
pub struct GlyphAtlas {
    map: HashMap<char, Texture2D>,
    width: f32,
    height: f32,
    cap_height: f32,
    offset_y: f32,
    font: Font,
}

impl GlyphAtlas {
    /// Creates a new atlas from the bytes of a BDF font.
    pub fn from_bdf_bytes(bytes: &[u8]) -> Result<Self, bdf_reader::Error> {
        let reader = BufReader::new(bytes);
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

        // takes time, but makes drawing much faster
        build_textures_atlas();

        let (cap_height, offset_y) = if let Some(glyph) = font.glyph('X') {
            (count_bitmap_rows(glyph.bitmap()) as f32,
                -(first_bitmap_row(glyph.bitmap()) as f32))
        } else {
            (height, 0.0)
        };

        Self { map, width, height, cap_height, offset_y, font }
    }

    /// Draws `text` horizontally without wrapping. Returns the drawn area.
    pub fn draw_text(&self, x: f32, y: f32, text: &str, color: Color) -> Rect {
        // round coordinates; bitmap fonts should be pixel-aligned
        let initial_x = x.round();
        let y = y.round() + self.offset_y;

        let mut x = initial_x;

        for char in text.chars() {
            if let Some(texture) = self.map.get(&char) {
                if let Some(glyph) = self.font.glyph(char) {
                    let bbox = glyph.bounding_box();
                    draw_texture(texture, x + bbox.offset_x as f32,
                        y - bbox.offset_y as f32 + self.cap_height - bbox.height as f32,
                        color);
                    x += self.width;
                }
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

/// Returns the index of the first non-blank row in a bitmap.
fn first_bitmap_row(bitmap: Bitmap) -> usize {
    (0..bitmap.height())
        .position(|y| (0..bitmap.width()).any(|x| bitmap.get(x, y).is_ok_and(|v| v)))
        .unwrap_or_default()
}