//! Texto rasterizado: una fuente pixel 5×7 integrada y fuentes vectoriales (TTF/OTF) con fontdue.

use std::sync::{Arc, OnceLock};

use cs_core::{ColorMode, LayerId, PixelValue, Sprite};

/// Fuente del texto.
#[derive(Clone)]
pub enum FontChoice {
    /// 5×7 integrada; el tamaño es una escala entera.
    Pixel,
    Hack,
    Ubuntu,
    /// Fuente cargada por el usuario.
    Custom {
        name: String,
        font: Arc<fontdue::Font>,
    },
}

impl std::fmt::Debug for FontChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}

impl PartialEq for FontChoice {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl FontChoice {
    pub fn name(&self) -> String {
        match self {
            FontChoice::Pixel => "Pixel 5×7".into(),
            FontChoice::Hack => "Hack".into(),
            FontChoice::Ubuntu => "Ubuntu Light".into(),
            FontChoice::Custom { name, .. } => name.clone(),
        }
    }

    /// Carga una fuente TTF/OTF.
    pub fn load(name: impl Into<String>, bytes: &[u8]) -> Result<Self, String> {
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|e| e.to_string())?;
        Ok(FontChoice::Custom {
            name: name.into(),
            font: Arc::new(font),
        })
    }

    /// Los caracteres que tiene la fuente, en orden (los visibles: sin los de control ni el
    /// espacio).
    pub fn chars(&self) -> Vec<char> {
        let mut chars: Vec<char> = match self.vector() {
            None => (33u8..=126).map(char::from).collect(),
            Some(font) => font.chars().keys().copied().collect(),
        };
        chars.retain(|c| !c.is_control() && !c.is_whitespace());
        chars.sort_unstable();
        chars
    }

    fn vector(&self) -> Option<&fontdue::Font> {
        static HACK: OnceLock<fontdue::Font> = OnceLock::new();
        static UBUNTU: OnceLock<fontdue::Font> = OnceLock::new();
        let builtin = |cell: &'static OnceLock<fontdue::Font>, bytes: &'static [u8]| {
            cell.get_or_init(|| {
                fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
                    .expect("bundled font parses")
            })
        };
        match self {
            FontChoice::Pixel => None,
            FontChoice::Hack => Some(builtin(&HACK, epaint_default_fonts::HACK_REGULAR)),
            FontChoice::Ubuntu => Some(builtin(&UBUNTU, epaint_default_fonts::UBUNTU_LIGHT)),
            FontChoice::Custom { font, .. } => Some(font),
        }
    }
}

/// Estilo del texto.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub font: FontChoice,
    /// Escala (fuente pixel) o tamaño en píxeles (vectoriales).
    pub size: u32,
    pub antialias: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: FontChoice::Pixel,
            size: 1,
            antialias: false,
        }
    }
}

/// Glifos 5×8 por columnas (bit 0 = fila de arriba) para ASCII 32–126.
const GLYPHS: [[u8; 5]; 95] = [
    [0x00, 0x00, 0x00, 0x00, 0x00], // ' '
    [0x00, 0x00, 0x5F, 0x00, 0x00], // !
    [0x00, 0x07, 0x00, 0x07, 0x00], // "
    [0x14, 0x7F, 0x14, 0x7F, 0x14], // #
    [0x24, 0x2A, 0x7F, 0x2A, 0x12], // $
    [0x23, 0x13, 0x08, 0x64, 0x62], // %
    [0x36, 0x49, 0x55, 0x22, 0x50], // &
    [0x00, 0x05, 0x03, 0x00, 0x00], // '
    [0x00, 0x1C, 0x22, 0x41, 0x00], // (
    [0x00, 0x41, 0x22, 0x1C, 0x00], // )
    [0x14, 0x08, 0x3E, 0x08, 0x14], // *
    [0x08, 0x08, 0x3E, 0x08, 0x08], // +
    [0x00, 0x80, 0x60, 0x00, 0x00], // ,
    [0x08, 0x08, 0x08, 0x08, 0x08], // -
    [0x00, 0x60, 0x60, 0x00, 0x00], // .
    [0x20, 0x10, 0x08, 0x04, 0x02], // /
    [0x3E, 0x51, 0x49, 0x45, 0x3E], // 0
    [0x00, 0x42, 0x7F, 0x40, 0x00], // 1
    [0x42, 0x61, 0x51, 0x49, 0x46], // 2
    [0x21, 0x41, 0x45, 0x4B, 0x31], // 3
    [0x18, 0x14, 0x12, 0x7F, 0x10], // 4
    [0x27, 0x45, 0x45, 0x45, 0x39], // 5
    [0x3C, 0x4A, 0x49, 0x49, 0x30], // 6
    [0x01, 0x71, 0x09, 0x05, 0x03], // 7
    [0x36, 0x49, 0x49, 0x49, 0x36], // 8
    [0x06, 0x49, 0x49, 0x29, 0x1E], // 9
    [0x00, 0x36, 0x36, 0x00, 0x00], // :
    [0x00, 0x56, 0x36, 0x00, 0x00], // ;
    [0x08, 0x14, 0x22, 0x41, 0x00], // <
    [0x14, 0x14, 0x14, 0x14, 0x14], // =
    [0x00, 0x41, 0x22, 0x14, 0x08], // >
    [0x02, 0x01, 0x51, 0x09, 0x06], // ?
    [0x32, 0x49, 0x79, 0x41, 0x3E], // @
    [0x7E, 0x11, 0x11, 0x11, 0x7E], // A
    [0x7F, 0x49, 0x49, 0x49, 0x36], // B
    [0x3E, 0x41, 0x41, 0x41, 0x22], // C
    [0x7F, 0x41, 0x41, 0x22, 0x1C], // D
    [0x7F, 0x49, 0x49, 0x49, 0x41], // E
    [0x7F, 0x09, 0x09, 0x09, 0x01], // F
    [0x3E, 0x41, 0x49, 0x49, 0x7A], // G
    [0x7F, 0x08, 0x08, 0x08, 0x7F], // H
    [0x00, 0x41, 0x7F, 0x41, 0x00], // I
    [0x20, 0x40, 0x41, 0x3F, 0x01], // J
    [0x7F, 0x08, 0x14, 0x22, 0x41], // K
    [0x7F, 0x40, 0x40, 0x40, 0x40], // L
    [0x7F, 0x02, 0x0C, 0x02, 0x7F], // M
    [0x7F, 0x04, 0x08, 0x10, 0x7F], // N
    [0x3E, 0x41, 0x41, 0x41, 0x3E], // O
    [0x7F, 0x09, 0x09, 0x09, 0x06], // P
    [0x3E, 0x41, 0x51, 0x21, 0x5E], // Q
    [0x7F, 0x09, 0x19, 0x29, 0x46], // R
    [0x46, 0x49, 0x49, 0x49, 0x31], // S
    [0x01, 0x01, 0x7F, 0x01, 0x01], // T
    [0x3F, 0x40, 0x40, 0x40, 0x3F], // U
    [0x1F, 0x20, 0x40, 0x20, 0x1F], // V
    [0x3F, 0x40, 0x38, 0x40, 0x3F], // W
    [0x63, 0x14, 0x08, 0x14, 0x63], // X
    [0x07, 0x08, 0x70, 0x08, 0x07], // Y
    [0x61, 0x51, 0x49, 0x45, 0x43], // Z
    [0x00, 0x7F, 0x41, 0x41, 0x00], // [
    [0x02, 0x04, 0x08, 0x10, 0x20], // \
    [0x00, 0x41, 0x41, 0x7F, 0x00], // ]
    [0x04, 0x02, 0x01, 0x02, 0x04], // ^
    [0x40, 0x40, 0x40, 0x40, 0x40], // _
    [0x00, 0x01, 0x02, 0x04, 0x00], // `
    [0x20, 0x54, 0x54, 0x54, 0x78], // a
    [0x7F, 0x48, 0x44, 0x44, 0x38], // b
    [0x38, 0x44, 0x44, 0x44, 0x20], // c
    [0x38, 0x44, 0x44, 0x48, 0x7F], // d
    [0x38, 0x54, 0x54, 0x54, 0x18], // e
    [0x08, 0x7E, 0x09, 0x01, 0x02], // f
    [0x18, 0xA4, 0xA4, 0xA4, 0x7C], // g
    [0x7F, 0x08, 0x04, 0x04, 0x78], // h
    [0x00, 0x44, 0x7D, 0x40, 0x00], // i
    [0x40, 0x80, 0x84, 0x7D, 0x00], // j
    [0x7F, 0x10, 0x28, 0x44, 0x00], // k
    [0x00, 0x41, 0x7F, 0x40, 0x00], // l
    [0x7C, 0x04, 0x18, 0x04, 0x78], // m
    [0x7C, 0x08, 0x04, 0x04, 0x78], // n
    [0x38, 0x44, 0x44, 0x44, 0x38], // o
    [0xFC, 0x24, 0x24, 0x24, 0x18], // p
    [0x18, 0x24, 0x24, 0x18, 0xFC], // q
    [0x7C, 0x08, 0x04, 0x04, 0x08], // r
    [0x48, 0x54, 0x54, 0x54, 0x20], // s
    [0x04, 0x3F, 0x44, 0x40, 0x20], // t
    [0x3C, 0x40, 0x40, 0x20, 0x7C], // u
    [0x1C, 0x20, 0x40, 0x20, 0x1C], // v
    [0x3C, 0x40, 0x30, 0x40, 0x3C], // w
    [0x44, 0x28, 0x10, 0x28, 0x44], // x
    [0x1C, 0xA0, 0xA0, 0xA0, 0x7C], // y
    [0x44, 0x64, 0x54, 0x4C, 0x44], // z
    [0x00, 0x08, 0x36, 0x41, 0x00], // {
    [0x00, 0x00, 0x7F, 0x00, 0x00], // |
    [0x00, 0x41, 0x36, 0x08, 0x00], // }
    [0x08, 0x04, 0x08, 0x10, 0x08], // ~
];

/// Cobertura (0–255) de cada píxel del texto, con el origen en la esquina superior izquierda.
pub fn rasterize(text: &str, style: &TextStyle) -> Vec<(i32, i32, u8)> {
    match style.font.vector() {
        None => pixel_text(text, style.size.clamp(1, 8) as i32),
        Some(font) => vector_text(font, text, style.size.clamp(4, 256) as f32),
    }
}

fn pixel_text(text: &str, scale: i32) -> Vec<(i32, i32, u8)> {
    let mut out = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let y0 = row as i32 * 9 * scale;
        for (col, ch) in line.chars().enumerate() {
            let code = ch as u32;
            let glyph = if (32..=126).contains(&code) {
                GLYPHS[(code - 32) as usize]
            } else {
                GLYPHS[('?' as u32 - 32) as usize]
            };
            let x0 = col as i32 * 6 * scale;
            for (gx, bits) in glyph.iter().enumerate() {
                for gy in 0..8 {
                    if bits & (1 << gy) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            out.push((x0 + gx as i32 * scale + sx, y0 + gy * scale + sy, 255));
                        }
                    }
                }
            }
        }
    }
    out
}

fn vector_text(font: &fontdue::Font, text: &str, px: f32) -> Vec<(i32, i32, u8)> {
    let metrics = font.horizontal_line_metrics(px);
    let ascent = metrics.map_or(px, |m| m.ascent).ceil() as i32;
    let line_height = metrics.map_or(px * 1.2, |m| m.new_line_size).ceil() as i32;
    let mut out = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let baseline = row as i32 * line_height + ascent;
        let mut pen = 0.0f32;
        for ch in line.chars() {
            let (m, bitmap) = font.rasterize(ch, px);
            let left = pen.round() as i32 + m.xmin;
            let top = baseline - m.ymin - m.height as i32;
            for (i, &coverage) in bitmap.iter().enumerate() {
                if coverage > 0 {
                    let (gx, gy) = ((i % m.width) as i32, (i / m.width) as i32);
                    out.push((left + gx, top + gy, coverage));
                }
            }
            pen += m.advance_width;
        }
    }
    out
}

/// Píxeles a pintar con `color` en `origin`. Sin antialias (o en indexado) cada píxel es todo o
/// nada; con antialias en RGBA se mezcla sobre lo que hay en la capa.
pub fn text_pixels(
    sprite: &Sprite,
    (layer, frame): (LayerId, usize),
    origin: (i32, i32),
    color: PixelValue,
    text: &str,
    style: &TextStyle,
) -> Vec<(i32, i32, PixelValue)> {
    let smooth = style.antialias && sprite.color_mode == ColorMode::Rgba;
    let src = cs_render::resolve(sprite, color);
    rasterize(text, style)
        .into_iter()
        .filter_map(|(x, y, coverage)| {
            let (x, y) = (origin.0 + x, origin.1 + y);
            if smooth {
                let under = cs_render::resolve(sprite, sprite.pixel(layer, frame, x, y));
                let alpha = (src.a as u32 * coverage as u32 / 255) as u8;
                Some((
                    x,
                    y,
                    PixelValue::Rgba(cs_render::blend_over(under, src, alpha)),
                ))
            } else {
                (coverage >= 128).then_some((x, y, color))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use cs_core::Rgba;

    use super::*;

    fn bounds(px: &[(i32, i32, u8)]) -> ((i32, i32), (i32, i32)) {
        px.iter().fold(
            ((i32::MAX, i32::MAX), (i32::MIN, i32::MIN)),
            |((x0, y0), (x1, y1)), p| ((x0.min(p.0), y0.min(p.1)), (x1.max(p.0), y1.max(p.1))),
        )
    }

    #[test]
    fn pixel_font_glyphs_scale_and_lines() {
        let style = TextStyle::default();
        let i = rasterize("I", &style);
        // 'I': barra central de 7 píxeles + remates arriba y abajo.
        assert_eq!(i.len(), 7 + 4);
        assert_eq!(bounds(&i), ((1, 0), (3, 6)));
        let two = rasterize("I\nI", &style);
        assert_eq!(bounds(&two).1.1, 9 + 6);
        let big = rasterize(
            "I",
            &TextStyle {
                size: 3,
                ..style.clone()
            },
        );
        assert_eq!(big.len(), 11 * 9);
        assert_eq!(
            rasterize("é", &style),
            rasterize("?", &style),
            "unknown → ?"
        );
        assert!(rasterize("g", &style).iter().any(|p| p.1 == 7), "descender");
    }

    #[test]
    fn vector_fonts_render_with_coverage() {
        for font in [FontChoice::Hack, FontChoice::Ubuntu] {
            let style = TextStyle {
                font,
                size: 16,
                antialias: true,
            };
            let px = rasterize("Hi", &style);
            assert!(px.len() > 20);
            assert!(px.iter().any(|p| p.2 < 255), "antialiased edges");
            let ((x0, y0), (_, y1)) = bounds(&px);
            assert!(x0 >= 0 && y0 >= 0 && y1 < 20);
        }
        assert!(FontChoice::load("bad", b"nope").is_err());
        let hack = FontChoice::load("Hack copy", epaint_default_fonts::HACK_REGULAR).unwrap();
        assert_eq!(hack.name(), "Hack copy");
    }

    #[test]
    fn antialias_blends_in_rgba_and_thresholds_otherwise() {
        let s = Sprite::new(32, 32, ColorMode::Rgba);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let at = (LayerId(1), 0);
        let style = TextStyle {
            font: FontChoice::Hack,
            size: 14,
            antialias: true,
        };
        let smooth = text_pixels(&s, at, (2, 2), red, "o", &style);
        assert!(
            smooth
                .iter()
                .any(|p| matches!(p.2, PixelValue::Rgba(c) if c.a > 0 && c.a < 255))
        );
        let hard = text_pixels(
            &s,
            at,
            (2, 2),
            red,
            "o",
            &TextStyle {
                antialias: false,
                ..style
            },
        );
        assert!(hard.iter().all(|p| p.2 == red));
        assert!(hard.iter().all(|p| p.0 >= 2 && p.1 >= 2));
    }
}
