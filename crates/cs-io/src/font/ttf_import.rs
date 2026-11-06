//! Importar un TTF/OTF como fuente pixel: se rasteriza cada glifo a la celda con 4×4 muestras por
//! pixel y la regla de devanado no cero de TrueType; un pixel se prende si su cobertura llega al
//! umbral. Las fuentes pixel en TTF (las de BitFontMaker2, las nuestras) se reconocen porque
//! todas sus coordenadas caen en una cuadrícula común: su tamaño sugerido es su celda exacta. En
//! color, las capas COLR (v0) se pintan una sobre otra con su color.

use std::collections::{BTreeMap, BTreeSet};

use cs_core::{FontBuild, FontMetrics, FontMode, GlyphBuild, Rgba};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::{IoError, Result};

/// Lo más que se importa con [`CharRange::All`].
pub const MAX_GLYPHS: usize = 4096;
/// El lado máximo de la celda importada.
pub const MAX_CELL: u32 = 256;
/// Los tamaños que se pueden pedir.
pub const PX_RANGE: std::ops::RangeInclusive<u32> = 4..=256;
/// Muestras por lado de cada pixel.
const SAMPLES: usize = 4;

/// Lo que se sabe de un archivo antes de importarlo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TtfInfo {
    pub family: String,
    pub units_per_em: u16,
    /// El tamaño en px que deja cada pixel de la fuente en un pixel (si es pixel), o 16.
    pub suggested_px: u32,
    /// Si sus coordenadas caen en una cuadrícula común (es una fuente pixel).
    pub pixel_grid: bool,
    /// Si tiene capas de color (COLR).
    pub color: bool,
    /// Cuántos caracteres tiene (sin el espacio ni los de control).
    pub chars: usize,
    /// Sus primeros caracteres (hasta 12), para mostrarla si no tiene letras latinas.
    pub sample: String,
}

/// Qué caracteres importar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CharRange {
    /// De `!` a `~`.
    Ascii,
    /// Hasta U+017F (ASCII, Latin-1 y Latin Ext-A).
    Latin,
    /// Todos (hasta [`MAX_GLYPHS`]).
    #[default]
    All,
}

impl CharRange {
    pub const ALL: [CharRange; 3] = [CharRange::Ascii, CharRange::Latin, CharRange::All];

    pub fn label(self) -> &'static str {
        match self {
            CharRange::Ascii => "ASCII",
            CharRange::Latin => "Latin (to U+017F)",
            CharRange::All => "All",
        }
    }

    fn contains(self, ch: char) -> bool {
        match self {
            CharRange::Ascii => ch <= '~',
            CharRange::Latin => ch <= '\u{17F}',
            CharRange::All => true,
        }
    }
}

/// Las opciones de importar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TtfImport {
    /// El nombre de la fuente (el de la familia si está vacío).
    pub name: String,
    /// El tamaño en px (el em).
    pub px: u32,
    /// La cobertura mínima de un pixel prendido, en % (1 a 100).
    pub threshold: u32,
    pub chars: CharRange,
    /// Con los colores de COLR (si los tiene); si no, 1 bit.
    pub color: bool,
}

fn parse(bytes: &[u8]) -> Result<Face<'_>> {
    Face::parse(bytes, 0)
        .map_err(|e| IoError::Unsupported(format!("not a TrueType or OpenType font ({e})")))
}

/// El nombre de la familia de `face` (el tipográfico si lo tiene).
fn family(face: &Face) -> String {
    let names = face.names();
    let find = |id| {
        names
            .into_iter()
            .filter(|n| n.name_id == id)
            .find_map(|n| n.to_string())
    };
    find(ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| find(ttf_parser::name_id::FAMILY))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Imported".into())
}

/// Los caracteres del cmap que se pueden importar (sin el espacio ni los de control), en orden.
fn chars_of(face: &Face) -> BTreeSet<char> {
    let mut out = BTreeSet::new();
    if let Some(cmap) = face.tables().cmap {
        for sub in cmap.subtables.into_iter().filter(|s| s.is_unicode()) {
            sub.codepoints(|cp| {
                if let Some(ch) = char::from_u32(cp)
                    && !ch.is_control()
                    && !ch.is_whitespace()
                    && sub.glyph_index(cp).is_some_and(|g| g.0 != 0)
                {
                    out.insert(ch);
                }
            });
        }
    }
    out
}

/// Lo que se sabe de `bytes` antes de importarlo.
pub fn ttf_info(bytes: &[u8]) -> Result<TtfInfo> {
    let face = parse(bytes)?;
    let chars = chars_of(&face);
    let em = face.units_per_em();
    // La cuadrícula común: el máximo común divisor de todo lo que mide.
    let mut values: Vec<f32> = vec![face.ascender() as f32, face.descender() as f32];
    for &ch in chars.iter().take(MAX_GLYPHS) {
        let Some(id) = face.glyph_index(ch) else {
            continue;
        };
        values.extend(face.glyph_hor_advance(id).map(|a| a as f32));
        let mut path = Path::default();
        face.outline_glyph(id, &mut path);
        values.extend(path.coords);
    }
    let grid = grid_of(&values);
    let px = grid
        .filter(|g| *g >= 2 && (em as u32).is_multiple_of(*g))
        .map(|g| em as u32 / g);
    let px = px.filter(|p| PX_RANGE.contains(p));
    Ok(TtfInfo {
        family: family(&face),
        units_per_em: em,
        suggested_px: px.unwrap_or(16),
        pixel_grid: px.is_some(),
        color: face.tables().colr.is_some(),
        chars: chars.len(),
        sample: chars.iter().take(12).collect(),
    })
}

/// El máximo común divisor de `values` si todos son enteros (`None` si alguno no lo es o si
/// todos son 0).
fn grid_of(values: &[f32]) -> Option<u32> {
    let mut g = 0u32;
    for &v in values {
        if (v - v.round()).abs() > 1e-3 {
            return None;
        }
        g = gcd(g, v.round().abs() as u32);
    }
    (g > 0).then_some(g)
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Un contorno aplanado en segmentos (en unidades de la fuente) y las coordenadas que se dieron.
#[derive(Default)]
struct Path {
    segments: Vec<((f32, f32), (f32, f32))>,
    start: (f32, f32),
    last: (f32, f32),
    coords: Vec<f32>,
}

impl Path {
    fn line(&mut self, to: (f32, f32)) {
        if to != self.last {
            self.segments.push((self.last, to));
        }
        self.last = to;
    }
}

impl OutlineBuilder for Path {
    fn move_to(&mut self, x: f32, y: f32) {
        self.coords.extend([x, y]);
        self.start = (x, y);
        self.last = (x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.coords.extend([x, y]);
        self.line((x, y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.coords.extend([x1, y1, x, y]);
        let p0 = self.last;
        const STEPS: usize = 8;
        for i in 1..=STEPS {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            self.line((
                u * u * p0.0 + 2.0 * u * t * x1 + t * t * x,
                u * u * p0.1 + 2.0 * u * t * y1 + t * t * y,
            ));
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.coords.extend([x1, y1, x2, y2, x, y]);
        let p0 = self.last;
        const STEPS: usize = 12;
        for i in 1..=STEPS {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.line((
                a * p0.0 + b * x1 + c * x2 + d * x,
                a * p0.1 + b * y1 + c * y2 + d * y,
            ));
        }
    }
    fn close(&mut self) {
        let start = self.start;
        self.line(start);
    }
}

/// Cómo pasar de unidades a pixeles: la escala y la fila de la baseline.
#[derive(Clone, Copy)]
struct Raster {
    scale: f32,
    baseline: f32,
    /// La cobertura mínima en muestras.
    min_samples: usize,
}

/// Los pixeles (columna, fila desde arriba de la celda; pueden ser negativos) que cubre el
/// contorno de `glyph`.
fn rasterize(face: &Face, glyph: GlyphId, r: Raster) -> Vec<(i32, i32)> {
    let mut path = Path::default();
    face.outline_glyph(glyph, &mut path);
    let segments: Vec<((f32, f32), (f32, f32))> = path
        .segments
        .iter()
        .map(|&(a, b)| {
            let to_px = |(x, y): (f32, f32)| (x * r.scale, r.baseline - y * r.scale);
            (to_px(a), to_px(b))
        })
        .collect();
    if segments.is_empty() {
        return Vec::new();
    }
    let ys = segments.iter().flat_map(|(a, b)| [a.1, b.1]);
    let xs = segments.iter().flat_map(|(a, b)| [a.0, b.0]);
    let (y0, y1) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let (x0, x1) = xs.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let (row0, row1) = (y0.floor() as i32, y1.ceil() as i32);
    let (col0, col1) = (x0.floor() as i32, x1.ceil() as i32);
    let width = (col1 - col0).max(0) as usize;
    let mut out = Vec::new();
    for row in row0..row1 {
        let mut coverage = vec![0usize; width];
        for s in 0..SAMPLES {
            let sy = row as f32 + (s as f32 + 0.5) / SAMPLES as f32;
            // Los cruces del renglón de muestras, con su sentido.
            let mut crossings: Vec<(f32, i32)> = segments
                .iter()
                .filter_map(|&(a, b)| {
                    let (lo, hi) = if a.1 < b.1 { (a, b) } else { (b, a) };
                    if a.1 == b.1 || sy < lo.1 || sy >= hi.1 {
                        return None;
                    }
                    let x = lo.0 + (sy - lo.1) * (hi.0 - lo.0) / (hi.1 - lo.1);
                    Some((x, if b.1 > a.1 { 1 } else { -1 }))
                })
                .collect();
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (mut k, mut winding) = (0, 0);
            for (c, cov) in coverage.iter_mut().enumerate() {
                for t in 0..SAMPLES {
                    let sx = (col0 + c as i32) as f32 + (t as f32 + 0.5) / SAMPLES as f32;
                    while k < crossings.len() && crossings[k].0 < sx {
                        winding += crossings[k].1;
                        k += 1;
                    }
                    if winding != 0 {
                        *cov += 1;
                    }
                }
            }
        }
        for (c, &cov) in coverage.iter().enumerate() {
            if cov >= r.min_samples {
                out.push((col0 + c as i32, row));
            }
        }
    }
    out
}

/// Junta las capas que pinta `paint_color_glyph` (COLR v0: glifo y color sólido).
#[derive(Default)]
struct Layers {
    outlined: Option<GlyphId>,
    layers: Vec<(GlyphId, Rgba)>,
}

impl<'a> ttf_parser::colr::Painter<'a> for Layers {
    fn outline_glyph(&mut self, glyph_id: GlyphId) {
        self.outlined = Some(glyph_id);
    }
    fn paint(&mut self, paint: ttf_parser::colr::Paint<'a>) {
        // Los degradados (COLR v1) no: solo los colores sólidos.
        if let (ttf_parser::colr::Paint::Solid(c), Some(id)) = (paint, self.outlined) {
            self.layers
                .push((id, Rgba::new(c.red, c.green, c.blue, c.alpha)));
        }
    }
    fn push_clip(&mut self) {}
    fn push_clip_box(&mut self, _: ttf_parser::colr::ClipBox) {}
    fn pop_clip(&mut self) {}
    fn push_layer(&mut self, _: ttf_parser::colr::CompositeMode) {}
    fn pop_layer(&mut self) {}
    fn push_transform(&mut self, _: ttf_parser::Transform) {}
    fn pop_transform(&mut self) {}
}

/// Los pixeles de un glifo antes de acomodar la celda (pueden ser negativos), con su color.
type RawPixels = BTreeMap<(i32, i32), Rgba>;

const BLACK: Rgba = Rgba::new(0, 0, 0, 255);

/// La fuente de `bytes` con `options`, lista para [`cs_core::build_font`].
pub fn import_ttf(bytes: &[u8], options: &TtfImport) -> Result<FontBuild> {
    let face = parse(bytes)?;
    let chars: Vec<char> = chars_of(&face)
        .into_iter()
        .filter(|&c| options.chars.contains(c))
        .take(MAX_GLYPHS)
        .collect();
    rasterize_font(&face, &chars, options)
}

/// Solo los caracteres `chars` de `bytes` (para la vista previa del diálogo).
pub fn import_chars(bytes: &[u8], chars: &[char], options: &TtfImport) -> Result<FontBuild> {
    let face = parse(bytes)?;
    let chars: Vec<char> = chars
        .iter()
        .copied()
        .filter(|&c| c != ' ' && face.glyph_index(c).is_some())
        .collect();
    rasterize_font(&face, &chars, options)
}

fn rasterize_font(face: &Face, chars: &[char], options: &TtfImport) -> Result<FontBuild> {
    let px = options.px.clamp(*PX_RANGE.start(), *PX_RANGE.end());
    let em = face.units_per_em().max(1) as f32;
    let scale = px as f32 / em;
    let (asc, desc) = (face.ascender() as f32, face.descender() as f32);
    let baseline = (asc * scale).round();
    let samples = SAMPLES * SAMPLES;
    let threshold = options.threshold.clamp(1, 100) as usize;
    let raster = Raster {
        scale,
        baseline,
        min_samples: (threshold * samples).div_ceil(100).max(1),
    };
    let color = options.color && face.tables().colr.is_some();
    let advance_px = |id: GlyphId| (face.glyph_hor_advance(id).unwrap_or(0) as f32 * scale).round();
    // Cada glifo: sus pixeles (con color) y su avance, antes de acomodar la celda.
    let mut raw: Vec<(char, RawPixels, i32)> = Vec::new();
    for &ch in chars {
        let Some(id) = face.glyph_index(ch) else {
            continue;
        };
        let mut pixels = BTreeMap::new();
        let mut layers = Layers::default();
        if color && face.is_color_glyph(id) {
            let fg = ttf_parser::RgbaColor::new(0, 0, 0, 255);
            face.paint_color_glyph(id, 0, fg, &mut layers);
        }
        if layers.layers.is_empty() {
            layers.layers.push((id, BLACK));
        }
        for (glyph, c) in layers.layers {
            for p in rasterize(face, glyph, raster) {
                pixels.insert(p, c);
            }
        }
        raw.push((ch, pixels, advance_px(id) as i32));
    }
    // La celda: la línea de la fuente, y más si algo se sale por arriba, abajo o la izquierda.
    let ink = raw.iter().flat_map(|(_, p, _)| p.keys().copied());
    let (min_x, min_y, max_y, max_x) = ink.fold((0, 0, 0, 0), |b, (x, y)| {
        (b.0.min(x), b.1.min(y), b.2.max(y + 1), b.3.max(x + 1))
    });
    let (shift_x, shift_y) = (-min_x, -min_y);
    let line = ((asc - desc) * scale).round().max(1.0) as i32;
    let height = (line.max(max_y) + shift_y).clamp(1, MAX_CELL as i32);
    let widest = raw.iter().map(|(_, _, a)| *a).max().unwrap_or(0);
    let notdef = advance_px(GlyphId(0)) as i32;
    let width = (widest.max(notdef).max(max_x) + shift_x).clamp(1, MAX_CELL as i32);
    let baseline = (baseline as i32 + shift_y).clamp(0, height) as u32;
    let mode = if color {
        FontMode::Color
    } else {
        FontMode::OneBit
    };
    let letter_spacing = 1;
    let mut effective = Vec::new();
    let mut glyphs: Vec<GlyphBuild> = raw
        .into_iter()
        .map(|(ch, pixels, advance)| {
            let pixels: Vec<(u32, u32, Option<Rgba>)> = pixels
                .into_iter()
                .map(|((x, y), c)| {
                    let c = (mode == FontMode::Color).then_some(c);
                    ((x + shift_x) as u32, (y + shift_y) as u32, c)
                })
                .collect();
            let advance = (advance + shift_x).max(0) as u32;
            effective.push(advance);
            // Automático si coincide con el automático (lo dibujado + el espaciado).
            let ink = pixels.iter().map(|p| p.0 + 1).max().unwrap_or(0);
            let auto = match ink {
                0 => width as u32 / 2,
                ink => ink + letter_spacing,
            };
            GlyphBuild {
                ch,
                advance: (advance != auto).then_some(advance),
                pixels,
            }
        })
        .collect();
    let monospace = !effective.is_empty() && effective.iter().all(|a| *a == width as u32);
    if monospace {
        for g in &mut glyphs {
            g.advance = None;
        }
    }
    // Las medidas: las de OS/2 o, si no, lo dibujado en H y x.
    let top_of = |ch: char| {
        glyphs
            .iter()
            .find(|g| g.ch == ch)
            .and_then(|g| g.pixels.iter().map(|p| p.1).min())
            .map(|top| baseline.saturating_sub(top))
    };
    let measure = |os2: Option<i16>, ch| {
        os2.filter(|v| *v > 0)
            .map(|v| (v as f32 * scale).round() as u32)
            .or_else(|| top_of(ch))
            .unwrap_or(0)
    };
    let cap_height = measure(face.capital_height(), 'H');
    let x_height = measure(face.x_height(), 'x');
    let space_width = face
        .glyph_index(' ')
        .map(|id| advance_px(id) as u32)
        .unwrap_or((px / 4).max(1));
    let name = match options.name.trim() {
        "" => family(face),
        name => name.to_owned(),
    };
    Ok(FontBuild {
        name,
        mode,
        cell: (width as u32, height as u32),
        metrics: FontMetrics {
            baseline,
            cap_height: cap_height.min(baseline),
            x_height: x_height.min(baseline),
            descent: height as u32 - baseline,
        },
        letter_spacing,
        space_width,
        monospace,
        glyphs,
    })
}

#[cfg(test)]
#[path = "ttf_import_tests.rs"]
mod tests;
