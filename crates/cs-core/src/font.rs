//! Fuentes pixel: un artboard con `font` es una fuente. Su sprite tiene un frame por glifo, del
//! tamaño de la celda, en el orden de los códigos Unicode de `FontInfo::glyphs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Artboard, ColorMode, Frame, LayerKind, Palette, PixelValue, Rgba, Sprite};

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

    /// Las medidas dentro de una celda de `height`: la baseline en la celda, los descendentes
    /// debajo de ella hasta abajo y las mayúsculas y la x-height hasta la baseline.
    pub fn clamped(self, height: u32) -> Self {
        let baseline = self.baseline.min(height);
        FontMetrics {
            baseline,
            cap_height: self.cap_height.min(baseline),
            x_height: self.x_height.min(baseline),
            descent: self.descent.min(height - baseline),
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

    /// Si el sprite de la fuente está bien armado: glifos en orden y sin repetir, uno por frame,
    /// una sola capa de imagen y el modo de color del modo de la fuente.
    pub fn check(&self, sprite: &Sprite) -> Result<(), FontError> {
        if self.glyphs.windows(2).any(|w| w[0].ch >= w[1].ch) {
            return Err(FontError::Unsorted);
        }
        if self.glyphs.len() != sprite.frames.len() {
            return Err(FontError::GlyphCount {
                glyphs: self.glyphs.len(),
                frames: sprite.frames.len(),
            });
        }
        if sprite.layers.len() != 1 || sprite.layers[0].kind != LayerKind::Image {
            return Err(FontError::Layers);
        }
        let mode_ok = match self.mode {
            FontMode::OneBit => matches!(sprite.color_mode, ColorMode::Indexed { .. }),
            FontMode::Color => sprite.color_mode == ColorMode::Rgba,
        };
        if !mode_ok {
            return Err(FontError::ColorMode);
        }
        Ok(())
    }
}

/// Lo que está mal armado en una fuente (ver [`FontInfo::check`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FontError {
    #[error("the glyphs are not in order or are repeated")]
    Unsorted,
    #[error("{glyphs} glyphs for {frames} frames")]
    GlyphCount { glyphs: usize, frames: usize },
    #[error("a font has exactly one image layer")]
    Layers,
    #[error("the color mode does not match the font mode")]
    ColorMode,
}

/// Arregla la lista de glifos de una fuente cargada para que vaya con los frames: los ordena
/// (con sus frames) y quita los repetidos; los frames sin glifo reciben caracteres de uso
/// privado libres desde U+E000 y los glifos sin frame se descartan. Devuelve si cambió algo.
pub fn normalize_font(font: &mut FontInfo, sprite: &mut Sprite) -> bool {
    let frames = sprite.frames.len();
    if font.glyphs.len() == frames && font.glyphs.windows(2).all(|w| w[0].ch < w[1].ch) {
        return false;
    }
    // Cada frame con su glifo: el que tenía (si no se repite) o uno de uso privado.
    let mut used = std::collections::BTreeSet::new();
    let mut slots: Vec<Option<GlyphInfo>> = (0..frames)
        .map(|f| font.glyphs.get(f).copied().filter(|g| used.insert(g.ch)))
        .collect();
    let mut next = 0xE000u32;
    for slot in slots.iter_mut().filter(|s| s.is_none()) {
        let ch = loop {
            let c = char::from_u32(next).unwrap_or('\u{E000}');
            next += 1;
            if used.insert(c) {
                break c;
            }
        };
        *slot = Some(GlyphInfo { ch, advance: None });
    }
    let mut order: Vec<(GlyphInfo, usize)> = slots
        .into_iter()
        .enumerate()
        .map(|(f, g)| (g.expect("filled"), f))
        .collect();
    order.sort_by_key(|(g, _)| g.ch);
    // Los frames (y sus cels) en el orden nuevo.
    let mut new_index = vec![0; frames];
    for (new, (_, old)) in order.iter().enumerate() {
        new_index[*old] = new;
    }
    sprite.frames = order.iter().map(|(_, old)| sprite.frames[*old]).collect();
    sprite.cels = std::mem::take(&mut sprite.cels)
        .into_iter()
        .map(|((layer, f), cel)| ((layer, new_index.get(f).copied().unwrap_or(f)), cel))
        .collect();
    font.glyphs = order.into_iter().map(|(g, _)| g).collect();
    true
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
            model: None,
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
    fn clamped_metrics_fit_the_cell_and_the_baseline() {
        let m = FontMetrics {
            baseline: 12,
            cap_height: 14,
            x_height: 13,
            descent: 9,
        };
        assert_eq!(
            m.clamped(16),
            FontMetrics {
                baseline: 12,
                cap_height: 12,
                x_height: 12,
                descent: 4
            }
        );
        let ok = FontMetrics::for_cell(16);
        assert_eq!(ok.clamped(16), ok);
        assert_eq!(FontMetrics { baseline: 20, ..ok }.clamped(16).baseline, 16);
    }

    #[test]
    fn check_finds_what_is_wrong_with_a_font_sprite() {
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        let mut s = font_sprite((8, 8), FontMode::OneBit, 94);
        assert_eq!(font.check(&s), Ok(()));
        s.frames.push(Frame { duration_ms: 100 });
        assert_eq!(
            font.check(&s),
            Err(FontError::GlyphCount {
                glyphs: 94,
                frames: 95
            })
        );
        s.frames.pop();
        font.glyphs.swap(0, 1);
        assert_eq!(font.check(&s), Err(FontError::Unsorted));
        font.glyphs.swap(0, 1);
        s.color_mode = ColorMode::Rgba;
        assert_eq!(font.check(&s), Err(FontError::ColorMode));
        s.color_mode = font_sprite((8, 8), FontMode::OneBit, 1).color_mode;
        let mut extra = s.layers[0].clone();
        extra.id = crate::LayerId(99);
        s.layers.push(extra);
        assert_eq!(font.check(&s), Err(FontError::Layers));
    }

    #[test]
    fn normalizing_sorts_dedups_and_pads_the_glyphs_with_their_frames() {
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        font.glyphs = vec![
            GlyphInfo {
                ch: 'B',
                advance: Some(2),
            },
            GlyphInfo {
                ch: 'A',
                advance: None,
            },
            GlyphInfo {
                ch: 'A',
                advance: None,
            },
        ];
        // Cuatro frames: el de 'B' (0) pintado; el 2 repite 'A' y el 3 no tiene glifo.
        let mut s = font_sprite((8, 8), FontMode::OneBit, 4);
        ink(&mut s, 0, &[(1, 1)]);
        assert!(normalize_font(&mut font, &mut s));
        let chars: Vec<char> = font.glyphs.iter().map(|g| g.ch).collect();
        assert_eq!(chars, vec!['A', 'B', '\u{E000}', '\u{E001}']);
        assert_eq!(font.glyphs[1].advance, Some(2));
        assert_eq!(font.check(&s), Ok(()));
        let layer = s.layers[0].id;
        assert_eq!(
            s.pixel(layer, 1, 1, 1),
            ONE_BIT_INK,
            "B's frame went with it"
        );
        assert!(!normalize_font(&mut font, &mut s), "already fine");
        // Más glifos que frames: sobran los últimos.
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        let mut s = font_sprite((8, 8), FontMode::OneBit, 2);
        assert!(normalize_font(&mut font, &mut s));
        assert_eq!(font.glyphs.len(), 2);
        assert_eq!(font.check(&s), Ok(()));
    }

    #[test]
    fn font_info_survives_json() {
        let font = FontInfo::new("Mine", 12, FontMode::Color);
        let json = serde_json::to_string(&font).unwrap();
        assert_eq!(serde_json::from_str::<FontInfo>(&json).unwrap(), font);
    }
}
