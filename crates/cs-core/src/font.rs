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

/// Un glifo: su carácter, su ancho (`None`: automático) y, si es compuesto, sus componentes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GlyphInfo {
    pub ch: char,
    #[serde(default)]
    pub advance: Option<u32>,
    /// Los glifos que se dibujan debajo de los pixeles propios (como los *composites* de
    /// TrueType); vacío: un glifo normal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<GlyphComponent>,
}

impl GlyphInfo {
    /// Un glifo normal de ancho automático.
    pub fn new(ch: char) -> Self {
        GlyphInfo {
            ch,
            advance: None,
            components: Vec::new(),
        }
    }

    pub fn is_composite(&self) -> bool {
        !self.components.is_empty()
    }
}

/// Un componente de un glifo compuesto: lo que se ve del glifo de `ch`, recorrido `offset`
/// pixeles desde la esquina de la celda.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GlyphComponent {
    pub ch: char,
    pub offset: (i32, i32),
}

/// Hasta cuántos niveles se siguen los compuestos dentro de compuestos.
const MAX_COMPONENT_DEPTH: usize = 8;

/// Lo que se ve de un glifo: pixel pintado por (fila, columna) de la celda.
pub type ComposedPixels = BTreeMap<(u32, u32), PixelValue>;

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
            glyphs: ('!'..='~').map(GlyphInfo::new).collect(),
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
    ///
    /// Un compuesto automático avanza lo mayor entre su primer componente (la base, con su
    /// offset) y lo que se ve + `letter_spacing`: `á` avanza como `a`.
    pub fn advance(&self, sprite: &Sprite, index: usize) -> u32 {
        self.advance_at(sprite, index, &mut Vec::new())
    }

    fn advance_at(&self, sprite: &Sprite, index: usize, chain: &mut Vec<usize>) -> u32 {
        if self.monospace {
            return sprite.width;
        }
        let Some(glyph) = self.glyphs.get(index) else {
            return sprite.width / 2;
        };
        if let Some(advance) = glyph.advance {
            return advance;
        }
        if !glyph.is_composite() {
            return match ink_width(sprite, index) {
                0 => sprite.width / 2,
                ink => ink + self.letter_spacing,
            };
        }
        let ink = self
            .composed(sprite, index)
            .keys()
            .map(|&(_, x)| x + 1)
            .max()
            .map(|ink| ink + self.letter_spacing);
        chain.push(index);
        let base = glyph.components.first().and_then(|c| {
            let j = self.index_of(c.ch)?;
            if chain.contains(&j) || chain.len() > MAX_COMPONENT_DEPTH {
                return None;
            }
            let advance = self.advance_at(sprite, j, chain) as i32 + c.offset.0;
            Some(advance.max(0) as u32)
        });
        chain.pop();
        match (base, ink) {
            (None, None) => sprite.width / 2,
            (a, b) => a.unwrap_or(0).max(b.unwrap_or(0)),
        }
    }

    /// Lo que se ve del glifo `index`: sus componentes en orden (cada uno con lo que se ve de su
    /// glifo, recorrido) y encima sus pixeles propios, recortado a la celda. Un componente que
    /// la fuente no tiene no dibuja; un ciclo (de un archivo roto) se corta.
    pub fn composed(&self, sprite: &Sprite, index: usize) -> ComposedPixels {
        let mut out = BTreeMap::new();
        self.compose_into(sprite, index, (0, 0), &mut Vec::new(), &mut out);
        out
    }

    fn compose_into(
        &self,
        sprite: &Sprite,
        index: usize,
        offset: (i32, i32),
        chain: &mut Vec<usize>,
        out: &mut ComposedPixels,
    ) {
        let Some(glyph) = self.glyphs.get(index) else {
            return;
        };
        chain.push(index);
        for c in &glyph.components {
            let Some(j) = self.index_of(c.ch) else {
                continue;
            };
            if chain.contains(&j) || chain.len() > MAX_COMPONENT_DEPTH {
                continue;
            }
            let at = (offset.0 + c.offset.0, offset.1 + c.offset.1);
            self.compose_into(sprite, j, at, chain, out);
        }
        chain.pop();
        let Some(layer) = sprite.layers.first().map(|l| l.id) else {
            return;
        };
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        for y in 0..h {
            for x in 0..w {
                let v = sprite.pixel(layer, index, x, y);
                let (sx, sy) = (x + offset.0, y + offset.1);
                if painted(sprite, v) && (0..w).contains(&sx) && (0..h).contains(&sy) {
                    out.insert((sy as u32, sx as u32), v);
                }
            }
        }
    }

    /// La caja de lo que se ve del glifo `index` (columnas y filas, el final sin incluir).
    pub fn ink_bounds(&self, sprite: &Sprite, index: usize) -> Option<InkBounds> {
        bounds_of(&self.composed(sprite, index))
    }

    /// Si el glifo de `a` usa (a cualquier profundidad) el de `b`.
    pub fn depends_on(&self, a: char, b: char) -> bool {
        let mut seen = std::collections::BTreeSet::new();
        let mut todo = vec![a];
        while let Some(ch) = todo.pop() {
            if !seen.insert(ch) {
                continue;
            }
            let Some(g) = self.index_of(ch).map(|i| &self.glyphs[i]) else {
                continue;
            };
            for c in &g.components {
                if c.ch == b {
                    return true;
                }
                todo.push(c.ch);
            }
        }
        false
    }

    /// Si poner `components` en el glifo de `ch` haría un ciclo.
    pub fn would_cycle(&self, ch: char, components: &[GlyphComponent]) -> bool {
        components
            .iter()
            .any(|c| c.ch == ch || self.depends_on(c.ch, ch))
    }

    /// Los compuestos que usan directamente el glifo de `ch`.
    pub fn users_of(&self, ch: char) -> Vec<char> {
        self.glyphs
            .iter()
            .filter(|g| g.components.iter().any(|c| c.ch == ch))
            .map(|g| g.ch)
            .collect()
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
        .map(|f| font.glyphs.get(f).cloned().filter(|g| used.insert(g.ch)))
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
        *slot = Some(GlyphInfo::new(ch));
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

/// Un glifo para [`build_font`]: su carácter, su ancho (`None`: automático) y sus pixeles en la
/// celda (`None` es la tinta de 1 bit; en color, su color).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphBuild {
    pub ch: char,
    pub advance: Option<u32>,
    pub pixels: Vec<(u32, u32, Option<Rgba>)>,
}

/// Una fuente armada desde pixeles (lo que devuelven los importadores).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontBuild {
    pub name: String,
    pub mode: FontMode,
    pub cell: (u32, u32),
    pub metrics: FontMetrics,
    pub letter_spacing: u32,
    pub space_width: u32,
    pub monospace: bool,
    pub glyphs: Vec<GlyphBuild>,
}

/// La fuente y su sprite desde `build`: los glifos en orden y sin repetir (gana el primero), sin
/// el espacio (que usa `space_width`), con lo que cae fuera de la celda recortado y sin cel los
/// vacíos. En 1 bit todo pixel es tinta; en color, un pixel sin color es negro. Sin glifos queda
/// uno vacío (`!`), porque una fuente tiene al menos un frame.
pub fn build_font(build: FontBuild) -> (FontInfo, Sprite) {
    let cell = (build.cell.0.max(1), build.cell.1.max(1));
    let mut glyphs: Vec<GlyphBuild> = build.glyphs.into_iter().filter(|g| g.ch != ' ').collect();
    glyphs.sort_by_key(|g| g.ch); // estable: el primero de los repetidos queda primero
    glyphs.dedup_by_key(|g| g.ch);
    if glyphs.is_empty() {
        glyphs.push(GlyphBuild {
            ch: '!',
            advance: None,
            pixels: Vec::new(),
        });
    }
    let mut sprite = font_sprite(cell, build.mode, glyphs.len());
    let layer = sprite.layers[0].id;
    for (frame, glyph) in glyphs.iter().enumerate() {
        let inside = glyph
            .pixels
            .iter()
            .filter(|(x, y, _)| *x < cell.0 && *y < cell.1);
        let mut image = crate::Image::new(cell.0, cell.1, sprite.color_mode);
        let mut any = false;
        for &(x, y, color) in inside {
            let value = match build.mode {
                FontMode::OneBit => ONE_BIT_INK,
                FontMode::Color => PixelValue::Rgba(color.unwrap_or(Rgba::new(0, 0, 0, 255))),
            };
            if painted(&sprite, value) && image.set(x, y, value).is_ok() {
                any = true;
            }
        }
        if !any {
            continue;
        }
        let id = sprite.next_image_id();
        sprite.images.insert(id, std::sync::Arc::new(image));
        sprite.cels.insert(
            (layer, frame),
            crate::Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
    }
    let font = FontInfo {
        name: build.name,
        mode: build.mode,
        metrics: build.metrics.clamped(cell.1),
        letter_spacing: build.letter_spacing,
        space_width: build.space_width,
        monospace: build.monospace,
        ghost: GhostSettings::default(),
        glyphs: glyphs
            .iter()
            .map(|g| GlyphInfo {
                advance: g.advance,
                ..GlyphInfo::new(g.ch)
            })
            .collect(),
    };
    (font, sprite)
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

/// Una caja de pixeles: columnas `x0..x1` y filas `y0..y1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InkBounds {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl InkBounds {
    pub fn width(&self) -> i32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> i32 {
        self.y1 - self.y0
    }
}

/// La caja de `pixels` (`None` si no hay ninguno).
pub fn bounds_of(pixels: &ComposedPixels) -> Option<InkBounds> {
    let mut it = pixels.keys();
    let &(y, x) = it.next()?;
    let mut b = InkBounds {
        x0: x as i32,
        y0: y as i32,
        x1: x as i32 + 1,
        y1: y as i32 + 1,
    };
    for &(y, x) in it {
        let (x, y) = (x as i32, y as i32);
        b.x0 = b.x0.min(x);
        b.x1 = b.x1.max(x + 1);
        b.y0 = b.y0.min(y);
        b.y1 = b.y1.max(y + 1);
    }
    Some(b)
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
        sprite.layers.first()?;
        let glyphs = font
            .glyphs
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let pixels = font
                    .composed(sprite, i)
                    .into_iter()
                    .map(|((y, x), v)| {
                        let color = match (font.mode, v) {
                            (FontMode::Color, PixelValue::Rgba(c)) => Some(c),
                            (FontMode::Color, PixelValue::Index(k)) => sprite.palette.get(k),
                            (FontMode::OneBit, _) => None,
                        };
                        (x, y, color)
                    })
                    .collect();
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
                advance: Some(2),
                ..GlyphInfo::new('B')
            },
            GlyphInfo::new('A'),
            GlyphInfo::new('A'),
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

    fn glyph(ch: char, advance: Option<u32>, pixels: &[(u32, u32)]) -> GlyphBuild {
        GlyphBuild {
            ch,
            advance,
            pixels: pixels.iter().map(|&(x, y)| (x, y, None)).collect(),
        }
    }

    fn build(mode: FontMode, glyphs: Vec<GlyphBuild>) -> FontBuild {
        FontBuild {
            name: "Built".into(),
            mode,
            cell: (8, 10),
            metrics: FontMetrics::for_cell(10),
            letter_spacing: 1,
            space_width: 4,
            monospace: false,
            glyphs,
        }
    }

    #[test]
    fn building_a_font_sorts_dedups_and_drops_the_space() {
        let (font, sprite) = build_font(build(
            FontMode::OneBit,
            vec![
                glyph('b', Some(5), &[(1, 1)]),
                glyph(' ', None, &[(0, 0)]),
                glyph('a', None, &[(0, 2), (20, 2), (1, 30)]),
                glyph('b', None, &[(3, 3)]),
                glyph('c', None, &[]),
            ],
        ));
        let chars: Vec<char> = font.glyphs.iter().map(|g| g.ch).collect();
        assert_eq!(chars, vec!['a', 'b', 'c']);
        assert_eq!(font.glyphs[1].advance, Some(5), "the first 'b' wins");
        assert_eq!(font.check(&sprite), Ok(()));
        assert_eq!((sprite.width, sprite.height), (8, 10));
        assert_eq!((font.name.as_str(), font.space_width), ("Built", 4));
        let layer = sprite.layers[0].id;
        assert_eq!(sprite.pixel(layer, 0, 0, 2), ONE_BIT_INK);
        assert_eq!(sprite.pixel(layer, 1, 1, 1), ONE_BIT_INK);
        assert_eq!(sprite.pixel(layer, 1, 3, 3), ONE_BIT_PAPER);
        assert!(!sprite.cels.contains_key(&(layer, 2)), "empty: no cel");
        assert_eq!(ink_width(&sprite, 0), 1, "outside the cell is cut");
    }

    #[test]
    fn building_a_color_font_keeps_the_colors() {
        let red = Rgba::new(200, 10, 10, 255);
        let (font, sprite) = build_font(build(
            FontMode::Color,
            vec![GlyphBuild {
                ch: 'A',
                advance: None,
                pixels: vec![(0, 0, Some(red)), (1, 0, None)],
            }],
        ));
        assert_eq!(font.check(&sprite), Ok(()));
        let layer = sprite.layers[0].id;
        assert_eq!(sprite.pixel(layer, 0, 0, 0), PixelValue::Rgba(red));
        assert_eq!(
            sprite.pixel(layer, 0, 1, 0),
            PixelValue::Rgba(Rgba::new(0, 0, 0, 255))
        );
        let (font, sprite) = build_font(build(FontMode::Color, Vec::new()));
        assert_eq!(font.glyphs.len(), 1, "a font has at least one glyph");
        assert_eq!(font.check(&sprite), Ok(()));
    }

    #[test]
    fn font_info_survives_json() {
        let font = FontInfo::new("Mine", 12, FontMode::Color);
        let json = serde_json::to_string(&font).unwrap();
        assert_eq!(serde_json::from_str::<FontInfo>(&json).unwrap(), font);
    }

    /// Una fuente 8×8 con `a` (2 px en la fila 5), `´` (1 px en la fila 0) y `á` compuesta.
    fn accented() -> (FontInfo, Sprite) {
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        font.glyphs.push(GlyphInfo::new('´'));
        font.glyphs.push(GlyphInfo::new('á'));
        let mut s = font_sprite((8, 8), FontMode::OneBit, font.glyphs.len());
        let a = font.index_of('a').unwrap();
        ink(&mut s, a, &[(0, 5), (3, 5)]);
        ink(&mut s, 94, &[(0, 0)]);
        font.glyphs[95].components = vec![
            GlyphComponent {
                ch: 'a',
                offset: (0, 0),
            },
            GlyphComponent {
                ch: '´',
                offset: (2, 3),
            },
        ];
        (font, s)
    }

    #[test]
    fn a_composite_shows_its_components_moved_and_its_own_pixels_on_top() {
        let (font, mut s) = accented();
        let px = |font: &FontInfo, s: &Sprite| -> Vec<(u32, u32)> {
            font.composed(s, 95).keys().map(|&(y, x)| (x, y)).collect()
        };
        assert_eq!(px(&font, &s), vec![(2, 3), (0, 5), (3, 5)]);
        ink(&mut s, 95, &[(7, 7)]);
        assert_eq!(px(&font, &s), vec![(2, 3), (0, 5), (3, 5), (7, 7)]);
        assert!(font.glyphs[95].is_composite());
        assert!(font.depends_on('á', 'a') && !font.depends_on('a', 'á'));
        assert_eq!(font.users_of('´'), vec!['á']);
        let b = font.ink_bounds(&s, 95).unwrap();
        assert_eq!((b.x0, b.y0, b.x1, b.y1), (0, 3, 8, 8));
    }

    #[test]
    fn components_are_clipped_and_missing_or_cyclic_ones_draw_nothing() {
        let (mut font, s) = accented();
        font.glyphs[95].components[1].offset = (7, -1);
        let keys: Vec<_> = font.composed(&s, 95).into_keys().collect();
        assert_eq!(keys, vec![(5, 0), (5, 3)], "the accent fell outside");
        font.glyphs[95].components[1].ch = 'é';
        assert_eq!(font.composed(&s, 95).len(), 2, "missing: nothing");
        // Un ciclo (de un archivo roto): a usa á, á usa a.
        let a = font.index_of('a').unwrap();
        font.glyphs[a].components = vec![GlyphComponent {
            ch: 'á',
            offset: (1, 0),
        }];
        assert_eq!(font.composed(&s, 95).len(), 2, "cut, no loop");
        let back = GlyphComponent {
            ch: 'á',
            offset: (0, 0),
        };
        assert!(font.would_cycle('a', &[back]));
        assert!(font.would_cycle('á', &[back]));
        let _ = font.advance(&s, a);
    }

    #[test]
    fn composites_nest() {
        let (mut font, mut s) = accented();
        font.glyphs.push(GlyphInfo {
            components: vec![GlyphComponent {
                ch: 'á',
                offset: (1, 1),
            }],
            ..GlyphInfo::new('\u{E000}')
        });
        s.frames.push(Frame { duration_ms: 100 });
        let keys: Vec<_> = font.composed(&s, 96).into_keys().collect();
        assert_eq!(keys, vec![(4, 3), (6, 1), (6, 4)]);
    }

    #[test]
    fn the_automatic_advance_of_a_composite_follows_its_base() {
        let (mut font, s) = accented();
        assert_eq!(font.advance(&s, 95), 5, "like a: 4 columns + 1");
        let a = font.index_of('a').unwrap();
        font.glyphs[a].advance = Some(7);
        assert_eq!(font.advance(&s, 95), 7, "a's manual advance");
        font.glyphs[95].components[1].offset = (7, 3);
        font.glyphs[a].advance = None;
        assert_eq!(font.advance(&s, 95), 9, "the accent reaches further");
        let artboard = crate::Artboard {
            model: None,
            id: crate::ArtboardId(1),
            name: "Mine".into(),
            position: (0, 0),
            sprite: s,
            guides: Vec::new(),
            symmetry: Default::default(),
            in_library: false,
            font: Some(font),
        };
        let view = artboard.font_view().unwrap();
        assert_eq!(view.glyphs[&'á'].pixels.len(), 3);
        assert_eq!(view.glyphs[&'á'].advance, 9);
    }

    #[test]
    fn glyph_components_survive_json_and_old_glyphs_still_read() {
        let (font, _) = accented();
        let json = serde_json::to_string(&font).unwrap();
        assert_eq!(serde_json::from_str::<FontInfo>(&json).unwrap(), font);
        let old: GlyphInfo = serde_json::from_str(r#"{"ch":"a","advance":null}"#).unwrap();
        assert_eq!(old, GlyphInfo::new('a'));
        assert!(!serde_json::to_string(&old).unwrap().contains("components"));
    }
}
