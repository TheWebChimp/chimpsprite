//! Fuentes pixel: un artboard con `font` es una fuente. Su sprite tiene un frame por glifo, del
//! tamaño de la celda, en el orden de los códigos Unicode de `FontInfo::glyphs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Artboard, ColorMode, Frame, Palette, PixelValue, Rgba, Sprite};

/// 1 bit (prendido o apagado, como BitFontMaker2) o color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontMode {
    OneBit,
    Color,
}

/// Las medidas, en filas desde arriba de la celda.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontMetrics {
    /// La línea de escritura: las filas `0..baseline` quedan arriba de ella.
    pub baseline: u32,
    /// Filas de las mayúsculas sobre la baseline.
    pub cap_height: u32,
    /// Filas de las minúsculas sin ascendentes sobre la baseline.
    pub x_height: u32,
    /// Filas debajo de la baseline.
    pub descent: u32,
}

impl FontMetrics {
    /// Las medidas iniciales de una celda de `height`: baseline en 3/4, mayúsculas en 5/8,
    /// x-height en 3/8 y los descendentes hasta abajo.
    pub fn for_cell(height: u32) -> Self {
        let baseline = (height * 3 / 4).max(1);
        FontMetrics {
            baseline,
            cap_height: (height * 5 / 8).max(1),
            x_height: (height * 3 / 8).max(1),
            descent: height - baseline,
        }
    }
}

/// La letra fantasma del editor: prendida y con qué fuente de egui (Ubuntu o Hack).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GhostSettings {
    pub enabled: bool,
    pub monospace: bool,
}

impl Default for GhostSettings {
    fn default() -> Self {
        GhostSettings {
            enabled: true,
            monospace: false,
        }
    }
}

/// Un glifo: su carácter y su ancho (`None`: automático).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GlyphInfo {
    pub ch: char,
    #[serde(default)]
    pub advance: Option<u32>,
}

/// La tinta y el papel de una fuente de 1 bit (índices de su paleta).
pub const ONE_BIT_INK: PixelValue = PixelValue::Index(1);
pub const ONE_BIT_PAPER: PixelValue = PixelValue::Index(0);
/// El color con que se ve la tinta de 1 bit (no se exporta).
const ONE_BIT_INK_COLOR: Rgba = Rgba {
    r: 5,
    g: 4,
    b: 3,
    a: 255,
};

/// Lo de fuente de un artboard. `glyphs` va alineada con los frames del sprite.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontInfo {
    pub name: String,
    pub mode: FontMode,
    pub metrics: FontMetrics,
    pub letter_spacing: u32,
    pub space_width: u32,
    pub monospace: bool,
    #[serde(default)]
    pub ghost: GhostSettings,
    pub glyphs: Vec<GlyphInfo>,
}

impl FontInfo {
    /// Una fuente nueva con los caracteres ASCII imprimibles (`!` a `~`), vacíos.
    pub fn new(name: impl Into<String>, cell_height: u32, mode: FontMode) -> FontInfo {
        FontInfo {
            name: name.into(),
            mode,
            metrics: FontMetrics::for_cell(cell_height),
            letter_spacing: 1,
            space_width: 3,
            monospace: false,
            ghost: GhostSettings::default(),
            glyphs: ('!'..='~')
                .map(|ch| GlyphInfo { ch, advance: None })
                .collect(),
        }
    }

    /// El índice (el frame) del glifo de `ch`.
    pub fn index_of(&self, ch: char) -> Option<usize> {
        self.insert_index(ch).ok()
    }

    /// `Ok(i)` si `ch` está en `i`; `Err(i)` donde iría.
    pub fn insert_index(&self, ch: char) -> Result<usize, usize> {
        self.glyphs.binary_search_by_key(&ch, |g| g.ch)
    }

    /// El ancho del glifo `index`: el de la celda con Monospace; el manual; o el automático
    /// (lo dibujado + `letter_spacing`; vacío, la mitad de la celda).
    pub fn advance(&self, sprite: &Sprite, index: usize) -> u32 {
        if self.monospace {
            return sprite.width;
        }
        if let Some(advance) = self.glyphs.get(index).and_then(|g| g.advance) {
            return advance;
        }
        match ink_width(sprite, index) {
            0 => sprite.width / 2,
            ink => ink + self.letter_spacing,
        }
    }

    /// Lo que avanza un carácter que la fuente no tiene (el cuadrito tachado).
    pub fn missing_width(&self) -> u32 {
        (self.metrics.cap_height / 2).max(3) + self.letter_spacing
    }

    /// El ancho de una línea de `text` (el espacio avanza `space_width`).
    pub fn text_width(&self, sprite: &Sprite, text: &str) -> u32 {
        text.chars()
            .map(|ch| match ch {
                ' ' => self.space_width,
                _ => self
                    .index_of(ch)
                    .map_or(self.missing_width(), |i| self.advance(sprite, i)),
            })
            .sum()
    }
}

/// El sprite de una fuente: `frames` glifos vacíos de `cell`. 1 bit: indexado con papel
/// (transparente) y tinta.
pub fn font_sprite(cell: (u32, u32), mode: FontMode, frames: usize) -> Sprite {
    let mut sprite = match mode {
        FontMode::OneBit => {
            let mut s = Sprite::new(
                cell.0,
                cell.1,
                ColorMode::Indexed {
                    transparent_index: 0,
                },
            );
            s.palette = Palette::new(vec![Rgba::TRANSPARENT, ONE_BIT_INK_COLOR]);
            s
        }
        FontMode::Color => Sprite::new(cell.0, cell.1, ColorMode::Rgba),
    };
    sprite.frames = vec![Frame { duration_ms: 100 }; frames.max(1)];
    sprite
}

/// Si `value` es un pixel pintado en `sprite`.
fn painted(sprite: &Sprite, value: PixelValue) -> bool {
    match value {
        PixelValue::Index(i) => sprite.color_mode.transparent() != PixelValue::Index(i),
        PixelValue::Rgba(c) => c.a > 0,
    }
}

/// Las columnas de lo dibujado en el glifo `frame`: la última con pixeles + 1 (0 si está vacío).
pub fn ink_width(sprite: &Sprite, frame: usize) -> u32 {
    let Some(layer) = sprite.layers.first().map(|l| l.id) else {
        return 0;
    };
    (0..sprite.width)
        .rev()
        .find(|&x| {
            (0..sprite.height)
                .any(|y| painted(sprite, sprite.pixel(layer, frame, x as i32, y as i32)))
        })
        .map_or(0, |x| x + 1)
}

/// Los pixeles de un glifo, en la celda: `None` es la tinta de 1 bit; en color, su color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphPixels {
    pub pixels: Vec<(u32, u32, Option<Rgba>)>,
    pub advance: u32,
}

/// Una fuente lista para escribir (la herramienta Texto, el Test Drive).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelFontView {
    pub cell: (u32, u32),
    pub metrics: FontMetrics,
    pub space_width: u32,
    pub missing_width: u32,
    pub glyphs: BTreeMap<char, GlyphPixels>,
}

impl Artboard {
    /// La fuente de este artboard lista para escribir; `None` si no es una fuente.
    pub fn font_view(&self) -> Option<PixelFontView> {
        let font = self.font.as_ref()?;
        let sprite = &self.sprite;
        let layer = sprite.layers.first()?.id;
        let glyphs = font
            .glyphs
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let mut pixels = Vec::new();
                for y in 0..sprite.height {
                    for x in 0..sprite.width {
                        let v = sprite.pixel(layer, i, x as i32, y as i32);
                        if !painted(sprite, v) {
                            continue;
                        }
                        let color = match (font.mode, v) {
                            (FontMode::Color, PixelValue::Rgba(c)) => Some(c),
                            (FontMode::Color, PixelValue::Index(k)) => sprite.palette.get(k),
                            (FontMode::OneBit, _) => None,
                        };
                        pixels.push((x, y, color));
                    }
                }
                let advance = font.advance(sprite, i);
                (g.ch, GlyphPixels { pixels, advance })
            })
            .collect();
        Some(PixelFontView {
            cell: (sprite.width, sprite.height),
            metrics: font.metrics,
            space_width: font.space_width,
            missing_width: font.missing_width(),
            glyphs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cel, Image};
    use std::sync::Arc;

    /// Pinta `points` con la tinta en el glifo `frame` de un sprite de 1 bit.
    fn ink(sprite: &mut Sprite, frame: usize, points: &[(u32, u32)]) {
        let layer = sprite.layers[0].id;
        let mut image = Image::new(sprite.width, sprite.height, sprite.color_mode);
        for &(x, y) in points {
            image.set(x, y, ONE_BIT_INK).unwrap();
        }
        let id = sprite.next_image_id();
        sprite.images.insert(id, Arc::new(image));
        sprite.cels.insert(
            (layer, frame),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
    }

    #[test]
    fn a_new_font_has_the_printable_ascii_and_metrics_from_the_cell() {
        let font = FontInfo::new("Mine", 16, FontMode::OneBit);
        assert_eq!(font.glyphs.len(), 94);
        assert_eq!(font.glyphs[0].ch, '!');
        assert_eq!(font.glyphs[93].ch, '~');
        assert!(font.glyphs.iter().all(|g| g.advance.is_none()));
        assert_eq!(
            font.metrics,
            FontMetrics {
                baseline: 12,
                cap_height: 10,
                x_height: 6,
                descent: 4
            }
        );
        assert_eq!((font.letter_spacing, font.space_width), (1, 3));
        assert!(!font.monospace && font.ghost.enabled);
    }

    #[test]
    fn glyphs_are_found_and_inserted_in_unicode_order() {
        let font = FontInfo::new("Mine", 16, FontMode::OneBit);
        assert_eq!(font.index_of('A'), Some(('A' as usize) - ('!' as usize)));
        assert_eq!(font.index_of('á'), None);
        assert_eq!(font.insert_index('á'), Err(94));
        assert_eq!(font.insert_index('!'), Ok(0));
    }

    #[test]
    fn a_one_bit_font_sprite_is_indexed_with_paper_and_ink() {
        let s = font_sprite((16, 16), FontMode::OneBit, 3);
        assert_eq!((s.width, s.height, s.frames.len()), (16, 16, 3));
        assert_eq!(
            s.color_mode,
            ColorMode::Indexed {
                transparent_index: 0
            }
        );
        assert_eq!(s.palette.len(), 2);
        let c = font_sprite((8, 10), FontMode::Color, 1);
        assert_eq!(c.color_mode, ColorMode::Rgba);
    }

    #[test]
    fn the_automatic_advance_is_the_ink_plus_the_spacing() {
        let mut font = FontInfo::new("Mine", 16, FontMode::OneBit);
        let mut s = font_sprite((16, 16), FontMode::OneBit, 94);
        ink(&mut s, 0, &[(0, 3), (4, 3)]);
        assert_eq!(ink_width(&s, 0), 5);
        assert_eq!(font.advance(&s, 0), 6, "5 columns + 1 of spacing");
        assert_eq!(font.advance(&s, 1), 8, "empty: half the cell");
        font.glyphs[0].advance = Some(9);
        assert_eq!(font.advance(&s, 0), 9, "manual");
        font.monospace = true;
        assert_eq!(font.advance(&s, 0), 16, "monospace: the cell");
    }

    #[test]
    fn the_width_of_a_text_adds_advances_spaces_and_missing_chars() {
        let font = FontInfo::new("Mine", 16, FontMode::OneBit);
        let mut s = font_sprite((16, 16), FontMode::OneBit, 94);
        let a = font.index_of('A').unwrap();
        ink(&mut s, a, &[(0, 5), (3, 5)]);
        // A (4 + 1) + espacio (3) + A (5) + "á" que falta (missing_width).
        assert_eq!(
            font.text_width(&s, "A Aá"),
            5 + 3 + 5 + font.missing_width()
        );
        assert_eq!(font.missing_width(), 5 + 1);
    }

    #[test]
    fn the_view_of_a_font_artboard_has_its_glyph_pixels() {
        let mut s = font_sprite((16, 16), FontMode::OneBit, 94);
        let a = 'A' as usize - '!' as usize;
        ink(&mut s, a, &[(1, 2)]);
        let artboard = crate::Artboard {
            id: crate::ArtboardId(1),
            name: "Mine".into(),
            position: (0, 0),
            sprite: s,
            guides: Vec::new(),
            symmetry: Default::default(),
            in_library: false,
            font: Some(FontInfo::new("Mine", 16, FontMode::OneBit)),
        };
        let view = artboard.font_view().unwrap();
        assert_eq!(view.cell, (16, 16));
        assert_eq!(view.glyphs[&'A'].pixels, vec![(1, 2, None)]);
        assert_eq!(view.glyphs[&'A'].advance, 2 + 1);
        assert!(view.glyphs[&'B'].pixels.is_empty());
        let plain = crate::Artboard {
            font: None,
            ..artboard
        };
        assert!(plain.font_view().is_none());
    }

    #[test]
    fn font_info_survives_json() {
        let font = FontInfo::new("Mine", 12, FontMode::Color);
        let json = serde_json::to_string(&font).unwrap();
        assert_eq!(serde_json::from_str::<FontInfo>(&json).unwrap(), font);
    }
}
