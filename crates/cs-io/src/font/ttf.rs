//! Exportar una fuente pixel a TrueType: cada pixel es un cuadrado de [`UNITS_PER_PIXEL`]
//! unidades, los vecinos unidos en contornos ([`super::outline`]), y la celda entera es la línea
//! (ascender + descender = alto). Las de color llevan además COLR v0 + CPAL: el glifo base es la
//! silueta (para los programas sin color) y cada color, una capa.
//!
//! Escrito a mano (sin crates): `head`, `hhea`, `maxp`, `OS/2`, `hmtx`, `cmap` (4 y 12),
//! `loca`, `glyf`, `name`, `post`, `COLR`/`CPAL` y, con kerning, `kern` y `GPOS`. El estilo de
//! la fuente (Bold, Italic, Outline…) va en `name`, `OS/2`, `head.macStyle`, `post` y `hhea`.

use cs_core::{Artboard, FontMode, FontStyle, Rgba, VariantKind};

use super::outline::{self, Point};
use crate::{IoError, Result};

/// Unidades por pixel (la cuadrícula de BitFontMaker2), si la celda no es enorme.
pub const UNITS_PER_PIXEL: u32 = 64;
/// El `unitsPerEm` más grande que permite TrueType.
const MAX_EM: u32 = 16384;
/// La fecha de creación y modificación (fija, para que el archivo no cambie solo): el 1 de enero
/// de 2026 en segundos desde 1904.
const DATE: i64 = 3_850_070_400;
/// Los pares de kerning que caben (una subtabla `kern` formato 0 mide a lo más 64 KB).
pub const MAX_KERNING_PAIRS: usize = 10_000;

/// Las unidades por pixel de una celda de `height` filas: 64, o menos si el em pasaría de 16384.
pub fn units_per_pixel(height: u32) -> u32 {
    UNITS_PER_PIXEL.min(MAX_EM / height.max(1)).max(1)
}

/// Lo que se elige al exportar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TtfOptions {
    /// El nombre de la familia (el de la fuente si está vacío).
    pub family: String,
    /// Con COLR + CPAL (solo para fuentes de color).
    pub color: bool,
}

/// Una capa de color de un glifo: sus pixeles y el índice de su color en la paleta.
type Layer = (Vec<(u32, u32)>, usize);

/// Un glifo del archivo: su avance y sus contornos (en unidades).
struct OutGlyph {
    advance: u32,
    contours: Vec<Vec<Point>>,
}

impl OutGlyph {
    /// La caja de sus puntos (`None` sin contornos).
    fn bbox(&self) -> Option<(i32, i32, i32, i32)> {
        let mut points = self.contours.iter().flatten();
        let first = points.next()?;
        Some(points.fold((first.0, first.1, first.0, first.1), |b, p| {
            (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
        }))
    }
}

/// El TTF de la fuente del artboard `artboard`.
pub fn export_ttf(artboard: &Artboard, options: &TtfOptions) -> Result<Vec<u8>> {
    let font = artboard
        .font
        .as_ref()
        .ok_or_else(|| IoError::Unsupported("the artboard is not a font".into()))?;
    let view = artboard
        .font_view()
        .ok_or_else(|| IoError::Unsupported("the artboard is not a font".into()))?;
    let (cell_w, cell_h) = view.cell;
    let upp = units_per_pixel(cell_h) as i32;
    let baseline = view.metrics.baseline.min(cell_h);
    let color = options.color && font.mode == FontMode::Color;
    let scaled = |contours: Vec<Vec<Point>>| -> Vec<Vec<Point>> {
        contours
            .into_iter()
            .map(|c| c.into_iter().map(|(x, y)| (x * upp, y * upp)).collect())
            .collect()
    };
    // .notdef: un cuadrito hueco del alto de las mayúsculas, con el ancho de la celda (así quien
    // lo lea sabe el ancho de la celda); luego el espacio y los glifos.
    let (bw, bh) = (
        (view.metrics.cap_height / 2).max(3) as i32,
        view.metrics.cap_height.max(3) as i32,
    );
    let mut glyphs = vec![
        OutGlyph {
            advance: cell_w,
            contours: scaled(vec![
                vec![(0, 0), (0, bh), (bw, bh), (bw, 0)],
                vec![(1, 1), (bw - 1, 1), (bw - 1, bh - 1), (1, bh - 1)],
            ]),
        },
        OutGlyph {
            advance: view.space_width,
            contours: Vec::new(),
        },
    ];
    let mut cmap: Vec<(u32, u16)> = vec![(' ' as u32, 1)];
    // Las capas de color: (glifo base, [(glifo capa, índice de la paleta)]).
    let mut palette: Vec<Rgba> = Vec::new();
    let mut layered: Vec<(usize, Vec<Layer>)> = Vec::new();
    for (&ch, glyph) in &view.glyphs {
        let gid = glyphs.len();
        let all: Vec<(u32, u32)> = glyph.pixels.iter().map(|&(x, y, _)| (x, y)).collect();
        glyphs.push(OutGlyph {
            advance: glyph.advance,
            contours: scaled(outline::contours(&all, baseline)),
        });
        if ch != ' ' {
            cmap.push((ch as u32, gid as u16));
        }
        if color {
            // Un grupo por color, en el orden de su primer pixel.
            let mut groups: Vec<(Rgba, Vec<(u32, u32)>)> = Vec::new();
            for &(x, y, c) in &glyph.pixels {
                let c = c.unwrap_or(Rgba::new(0, 0, 0, 255));
                match groups.iter_mut().find(|(g, _)| *g == c) {
                    Some((_, pixels)) => pixels.push((x, y)),
                    None => groups.push((c, vec![(x, y)])),
                }
            }
            if !groups.is_empty() {
                let layers = groups
                    .into_iter()
                    .map(|(c, pixels)| {
                        let index = palette.iter().position(|p| *p == c).unwrap_or_else(|| {
                            palette.push(c);
                            palette.len() - 1
                        });
                        (pixels, index)
                    })
                    .collect();
                layered.push((gid, layers));
            }
        }
    }
    // Los glifos capa van al final, sin código.
    let mut base_records = Vec::new();
    let mut layer_records = Vec::new();
    for (gid, layers) in layered {
        let advance = glyphs[gid].advance;
        base_records.push((gid, layer_records.len(), layers.len()));
        for (pixels, index) in layers {
            layer_records.push((glyphs.len(), index));
            glyphs.push(OutGlyph {
                advance,
                contours: scaled(outline::contours(&pixels, baseline)),
            });
        }
    }
    if glyphs.len() > u16::MAX as usize || layer_records.len() > u16::MAX as usize {
        return Err(IoError::Unsupported(
            "too many glyphs for a TrueType font".into(),
        ));
    }
    cmap.sort_unstable();
    let family = match options.family.trim() {
        "" => font.name.trim(),
        name => name,
    };
    let family = if family.is_empty() {
        "Pixel Font"
    } else {
        family
    };
    // El kerning aplanado, de glifo a glifo, en unidades.
    let gid_of = |ch: char| {
        cmap.binary_search_by_key(&(ch as u32), |c| c.0)
            .ok()
            .map(|i| cmap[i].1)
    };
    let mut kerning: Vec<(u16, u16, i32)> = view
        .kerning
        .iter()
        .filter_map(|(&(a, b), &v)| Some((gid_of(a)?, gid_of(b)?, v * upp)))
        .collect();
    kerning.sort_unstable();
    if kerning.len() > MAX_KERNING_PAIRS {
        return Err(IoError::Unsupported(format!(
            "too many kerning pairs ({}; at most {MAX_KERNING_PAIRS})",
            kerning.len()
        )));
    }
    let style = &font.style;
    let kind = style.source.as_ref().map(|s| s.recipe.kind);
    let metrics = Metrics {
        em: (cell_h as i32 * upp) as u16,
        upp,
        ascender: baseline as i32 * upp,
        descender: -((cell_h - baseline) as i32 * upp),
        cap_height: view.metrics.cap_height as i32 * upp,
        x_height: view.metrics.x_height as i32 * upp,
        cell_h,
        monospace: font.monospace,
        weight: style.weight.clamp(1, 1000),
        bold: style.is_bold(),
        italic: style.italic,
        outline: kind == Some(VariantKind::Outline),
        shadow: kind == Some(VariantKind::Shadow),
        slant: style.slant,
        italic_angle: style.italic_angle(),
    };
    let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    let (loca, glyf) = glyf_loca(&glyphs);
    tables.push((*b"OS/2", os2(&glyphs, &cmap, &metrics)));
    tables.push((*b"cmap", cmap_table(&cmap)));
    tables.push((*b"glyf", glyf));
    tables.push((*b"head", head(&glyphs, &metrics)));
    tables.push((*b"hhea", hhea(&glyphs, &metrics)));
    tables.push((*b"hmtx", hmtx(&glyphs, metrics.upp)));
    tables.push((*b"loca", loca));
    tables.push((*b"maxp", maxp(&glyphs)));
    tables.push((*b"name", name_table(family, style)));
    tables.push((*b"post", post(&metrics)));
    if color && !base_records.is_empty() {
        tables.push((*b"COLR", colr(&base_records, &layer_records)));
        tables.push((*b"CPAL", cpal(&palette)));
    }
    if !kerning.is_empty() {
        tables.push((*b"kern", kern(&kerning)));
        tables.push((*b"GPOS", gpos(&kerning)));
    }
    tables.sort_by_key(|(tag, _)| *tag);
    Ok(sfnt(tables))
}

/// Las medidas del archivo, en unidades.
struct Metrics {
    em: u16,
    upp: i32,
    ascender: i32,
    descender: i32,
    cap_height: i32,
    x_height: i32,
    cell_h: u32,
    monospace: bool,
    /// El estilo: el peso (usWeightClass), las banderas y la itálica.
    weight: u16,
    bold: bool,
    italic: bool,
    outline: bool,
    shadow: bool,
    slant: u32,
    italic_angle: f64,
}

impl Metrics {
    /// `head.macStyle`: bold, italic, outline y shadow.
    fn mac_style(&self) -> u16 {
        (self.bold as u16)
            | ((self.italic as u16) << 1)
            | ((self.outline as u16) << 3)
            | ((self.shadow as u16) << 4)
    }
}

/// Escritura big-endian.
#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn i16(&mut self, v: i32) -> &mut Self {
        self.u16(v.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16)
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn i64(&mut self, v: i64) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.0.extend_from_slice(v);
        self
    }
}

/// La caja de todos los glifos (ceros si no hay ninguno con contornos).
fn font_bbox(glyphs: &[OutGlyph]) -> (i32, i32, i32, i32) {
    glyphs
        .iter()
        .filter_map(OutGlyph::bbox)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .unwrap_or((0, 0, 0, 0))
}

fn head(glyphs: &[OutGlyph], m: &Metrics) -> Vec<u8> {
    let (x0, y0, x1, y1) = font_bbox(glyphs);
    let mut w = W::default();
    w.u16(1).u16(0).u32(0x0001_0000).u32(0).u32(0x5F0F_3CF5);
    // Baseline en y = 0, lsb en x = 0, ppem entero.
    w.u16(0b1011).u16(m.em).i64(DATE).i64(DATE);
    w.i16(x0).i16(y0).i16(x1).i16(y1);
    w.u16(m.mac_style())
        .u16(m.cell_h.min(u16::MAX as u32) as u16)
        .i16(2)
        .i16(1)
        .i16(0);
    w.0
}

fn hhea(glyphs: &[OutGlyph], m: &Metrics) -> Vec<u8> {
    let upp = m.upp;
    let advance_max = glyphs.iter().map(|g| g.advance as i32 * upp).max();
    let boxed: Vec<(i32, (i32, i32, i32, i32))> = glyphs
        .iter()
        .filter_map(|g| Some((g.advance as i32 * upp, g.bbox()?)))
        .collect();
    let min_lsb = boxed.iter().map(|(_, b)| b.0).min().unwrap_or(0);
    let min_rsb = boxed.iter().map(|(a, b)| a - b.2).min().unwrap_or(0);
    let extent = boxed.iter().map(|(_, b)| b.2).max().unwrap_or(0);
    let mut w = W::default();
    w.u16(1).u16(0).i16(m.ascender).i16(m.descender).i16(0);
    w.u16(advance_max.unwrap_or(0).clamp(0, u16::MAX as i32) as u16);
    w.i16(min_lsb).i16(min_rsb).i16(extent);
    // La pendiente del cursor: derecha, o la de la itálica (`slant` filas por columna).
    let (rise, run) = if m.slant > 0 {
        (m.slant as i32, 1)
    } else {
        (1, 0)
    };
    w.i16(rise)
        .i16(run)
        .i16(0)
        .i16(0)
        .i16(0)
        .i16(0)
        .i16(0)
        .i16(0);
    w.u16(glyphs.len() as u16);
    w.0
}

fn maxp(glyphs: &[OutGlyph]) -> Vec<u8> {
    let points = glyphs
        .iter()
        .map(|g| g.contours.iter().map(Vec::len).sum::<usize>())
        .max()
        .unwrap_or(0);
    let contours = glyphs.iter().map(|g| g.contours.len()).max().unwrap_or(0);
    let mut w = W::default();
    w.u32(0x0001_0000).u16(glyphs.len() as u16);
    w.u16(points.min(u16::MAX as usize) as u16);
    w.u16(contours.min(u16::MAX as usize) as u16);
    // Sin compuestos ni instrucciones; dos zonas.
    w.u16(0).u16(0).u16(2);
    for _ in 0..8 {
        w.u16(0);
    }
    w.0
}

fn hmtx(glyphs: &[OutGlyph], upp: i32) -> Vec<u8> {
    let mut w = W::default();
    for g in glyphs {
        let advance = (g.advance as i32 * upp).clamp(0, u16::MAX as i32) as u16;
        w.u16(advance).i16(g.bbox().map_or(0, |b| b.0));
    }
    w.0
}

/// `glyf` y `loca` (larga): cada glifo simple con solo puntos sobre la curva; cada coordenada,
/// con las banderas cortas (las de 0 no ocupan nada).
fn glyf_loca(glyphs: &[OutGlyph]) -> (Vec<u8>, Vec<u8>) {
    let mut glyf = W::default();
    let mut loca = W::default();
    for g in glyphs {
        loca.u32(glyf.0.len() as u32);
        let Some((x0, y0, x1, y1)) = g.bbox() else {
            continue;
        };
        glyf.i16(g.contours.len() as i32)
            .i16(x0)
            .i16(y0)
            .i16(x1)
            .i16(y1);
        let mut end = 0usize;
        for c in &g.contours {
            end += c.len();
            glyf.u16((end - 1) as u16);
        }
        glyf.u16(0); // sin instrucciones
        let (mut flags, mut xs, mut ys) = (Vec::new(), W::default(), W::default());
        let mut last = (0, 0);
        for &(x, y) in g.contours.iter().flatten() {
            let (dx, dy) = (x - last.0, y - last.1);
            last = (x, y);
            let mut flag = 0x01u8; // sobre la curva
            flag |= coord(dx, 0x02, 0x10, &mut xs);
            flag |= coord(dy, 0x04, 0x20, &mut ys);
            flags.push(flag);
        }
        glyf.bytes(&flags).bytes(&xs.0).bytes(&ys.0);
        while glyf.0.len() % 4 != 0 {
            glyf.u8(0);
        }
    }
    loca.u32(glyf.0.len() as u32);
    (loca.0, glyf.0)
}

/// Escribe un delta de coordenada y devuelve sus banderas (`short`, `same`): 0 no ocupa nada;
/// hasta 255, un byte con el signo en `same`; si no, 16 bits.
fn coord(delta: i32, short: u8, same: u8, out: &mut W) -> u8 {
    match delta {
        0 => same,
        d if d.abs() <= 255 => {
            out.u8(d.unsigned_abs() as u8);
            if d > 0 { short | same } else { short }
        }
        d => {
            out.i16(d);
            0
        }
    }
}

/// `cmap` con el formato 4 (el BMP) y, si hay caracteres fuera de él, el 12.
fn cmap_table(cmap: &[(u32, u16)]) -> Vec<u8> {
    let format4 = cmap_format4(cmap);
    let wide = cmap.iter().any(|&(c, _)| c > 0xFFFF);
    let format12 = wide.then(|| cmap_format12(cmap));
    let mut records: Vec<(u16, u16, bool)> = vec![(0, 3, false), (3, 1, false)];
    if wide {
        records = vec![(0, 3, false), (0, 4, true), (3, 1, false), (3, 10, true)];
    }
    let header = 4 + 8 * records.len() as u32;
    let mut w = W::default();
    w.u16(0).u16(records.len() as u16);
    for (platform, encoding, is12) in &records {
        let offset = if *is12 {
            header + format4.len() as u32
        } else {
            header
        };
        w.u16(*platform).u16(*encoding).u32(offset);
    }
    w.bytes(&format4);
    if let Some(f) = format12 {
        w.bytes(&f);
    }
    w.0
}

/// Los tramos de códigos y glifos seguidos: (primer código, último, primer glifo).
fn runs(cmap: &[(u32, u16)]) -> Vec<(u32, u32, u16)> {
    let mut out: Vec<(u32, u32, u16)> = Vec::new();
    for &(c, g) in cmap {
        match out.last_mut() {
            Some(run) if run.1 + 1 == c && run.2 as u32 + (c - run.0) == g as u32 => run.1 = c,
            _ => out.push((c, c, g)),
        }
    }
    out
}

fn cmap_format4(cmap: &[(u32, u16)]) -> Vec<u8> {
    let bmp: Vec<(u32, u16)> = cmap.iter().copied().filter(|&(c, _)| c < 0xFFFF).collect();
    let mut segments: Vec<(u16, u16, u16)> = runs(&bmp)
        .into_iter()
        .map(|(a, b, g)| (a as u16, b as u16, (g as u32).wrapping_sub(a) as u16))
        .collect();
    segments.push((0xFFFF, 0xFFFF, 1));
    let n = segments.len() as u16;
    let power = largest_power_of_two(n);
    let mut w = W::default();
    w.u16(4).u16(16 + 8 * n).u16(0);
    w.u16(n * 2)
        .u16(power * 2)
        .u16(power.trailing_zeros() as u16);
    w.u16(n * 2 - power * 2);
    for s in &segments {
        w.u16(s.1);
    }
    w.u16(0);
    for s in &segments {
        w.u16(s.0);
    }
    for s in &segments {
        w.u16(s.2);
    }
    for _ in &segments {
        w.u16(0);
    }
    w.0
}

fn cmap_format12(cmap: &[(u32, u16)]) -> Vec<u8> {
    let groups = runs(cmap);
    let mut w = W::default();
    w.u16(12).u16(0).u32(16 + 12 * groups.len() as u32).u32(0);
    w.u32(groups.len() as u32);
    for (a, b, g) in groups {
        w.u32(a).u32(b).u32(g as u32);
    }
    w.0
}

/// La potencia de 2 más grande que no pasa de `n` (para las búsquedas binarias de TrueType).
fn largest_power_of_two(n: u16) -> u16 {
    let mut power = 1u16;
    while power <= n / 2 {
        power *= 2;
    }
    power
}

/// El nombre PostScript: lo imprimible de ASCII sin espacios ni `[](){}<>/%`, hasta 63.
fn postscript_name(family: &str) -> String {
    let name: String = family
        .chars()
        .filter(|c| c.is_ascii_graphic() && !"[](){}<>/%".contains(*c))
        .take(63)
        .collect();
    if name.is_empty() {
        "PixelFont".into()
    } else {
        name
    }
}

/// El nombre completo de `family` con `style` ("Mine Bold"; el Regular, "Mine").
fn full_name(family: &str, style: &FontStyle) -> String {
    let name = style.name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("Regular") {
        family.to_owned()
    } else {
        format!("{family} {name}")
    }
}

/// `name`: la familia y la subfamilia legacy (RIBBI), el ID único, el nombre completo, la versión,
/// el PostScript ("Mine-Bold"; el Regular, "Mine") y, si el estilo no es RIBBI, la familia y la
/// subfamilia tipográficas.
fn name_table(family: &str, style: &FontStyle) -> Vec<u8> {
    let (legacy, sub) = style.legacy_names(family);
    let full = full_name(family, style);
    let ps = if full == family {
        postscript_name(family)
    } else {
        format!("{}-{}", postscript_name(family), style.compact_name())
    };
    let mut records: Vec<(u16, String)> = vec![
        (1, legacy),
        (2, sub),
        (3, format!("ChimpSprite: {full}")),
        (4, full),
        (5, "Version 1.000".into()),
        (6, postscript_name(&ps)),
    ];
    if !style.is_ribbi() && !style.name.trim().is_empty() {
        records.push((16, family.to_owned()));
        records.push((17, style.name.trim().to_owned()));
    }
    let mut strings = W::default();
    let mut w = W::default();
    w.u16(0)
        .u16(records.len() as u16)
        .u16(6 + 12 * records.len() as u16);
    for (id, text) in &records {
        let start = strings.0.len();
        for unit in text.encode_utf16() {
            strings.u16(unit);
        }
        let len = strings.0.len() - start;
        w.u16(3).u16(1).u16(0x409).u16(*id);
        w.u16(len as u16).u16(start as u16);
    }
    w.bytes(&strings.0);
    w.0
}

fn post(m: &Metrics) -> Vec<u8> {
    let mut w = W::default();
    // italicAngle en 16.16.
    let angle = (m.italic_angle * 65536.0).round() as i32;
    w.u32(0x0003_0000).u32(angle as u32).i16(-m.upp).i16(m.upp);
    w.u32(m.monospace as u32).u32(0).u32(0).u32(0).u32(0);
    w.0
}

fn os2(glyphs: &[OutGlyph], cmap: &[(u32, u16)], m: &Metrics) -> Vec<u8> {
    let em = m.em as i32;
    let advances: Vec<i32> = glyphs
        .iter()
        .filter(|g| g.advance > 0)
        .map(|g| g.advance as i32 * m.upp)
        .collect();
    let avg = advances.iter().sum::<i32>() / (advances.len().max(1) as i32);
    let mut ranges = 0u32;
    for &(c, _) in cmap {
        ranges |= match c {
            0..=0x7F => 1 << 0,
            0x80..=0xFF => 1 << 1,
            0x100..=0x17F => 1 << 2,
            0x180..=0x24F => 1 << 3,
            _ => 0,
        };
    }
    let first = cmap.first().map_or(0x20, |c| c.0.min(0xFFFF));
    let last = cmap.last().map_or(0x20, |c| c.0.min(0xFFFF));
    let mut w = W::default();
    w.u16(4).i16(avg).u16(m.weight).u16(5).u16(0);
    // Subíndices y superíndices.
    w.i16(em * 65 / 100)
        .i16(em * 60 / 100)
        .i16(0)
        .i16(em * 75 / 1000);
    w.i16(em * 65 / 100)
        .i16(em * 60 / 100)
        .i16(0)
        .i16(em * 35 / 100);
    // Tachado: un pixel a media x-height.
    w.i16(m.upp).i16(m.x_height / 2);
    w.i16(0).bytes(&[0; 10]);
    w.u32(ranges).u32(0).u32(0).u32(0).bytes(b"CHMP");
    // ITALIC, OUTLINED, BOLD o REGULAR, y USE_TYPO_METRICS.
    let mut selection = 0x80u16;
    if m.italic {
        selection |= 0x01;
    }
    if m.outline {
        selection |= 0x08;
    }
    if m.bold {
        selection |= 0x20;
    }
    if !m.italic && !m.bold {
        selection |= 0x40;
    }
    w.u16(selection).u16(first as u16).u16(last as u16);
    w.i16(m.ascender).i16(m.descender).i16(0);
    w.u16(m.ascender.max(0) as u16)
        .u16((-m.descender).max(0) as u16);
    w.u32(1).u32(0);
    w.i16(m.x_height).i16(m.cap_height).u16(0).u16(0x20).u16(0);
    w.0
}

/// `kern` (versión 0 de Microsoft): una subtabla formato 0, horizontal, con los pares
/// `(izquierdo, derecho, valor)` ordenados por glifos.
fn kern(pairs: &[(u16, u16, i32)]) -> Vec<u8> {
    let n = pairs.len() as u16;
    let power = largest_power_of_two(n.max(1));
    let mut w = W::default();
    w.u16(0).u16(1);
    w.u16(0).u16(14 + 6 * n).u16(0x0001);
    w.u16(n).u16(power * 6).u16(power.trailing_zeros() as u16);
    w.u16((n - power.min(n)) * 6);
    for &(a, b, v) in pairs {
        w.u16(a).u16(b).i16(v);
    }
    w.0
}

/// `GPOS` 1.0: los scripts `DFLT` y `latn` con la feature `kern`, que usa un lookup de tipo 2
/// (PairPos formato 1: un PairSet por glifo de la izquierda, con el `XAdvance` del primero).
fn gpos(pairs: &[(u16, u16, i32)]) -> Vec<u8> {
    // ScriptList: dos scripts, cada uno con su LangSys por defecto que usa la feature 0.
    let tags: [&[u8; 4]; 2] = [b"DFLT", b"latn"];
    let mut scripts = W::default();
    scripts.u16(tags.len() as u16);
    // Cada Script: su encabezado (4) y su LangSys (6 + un índice).
    let script_size = 4 + 6 + 2;
    for (i, tag) in tags.iter().enumerate() {
        let at = 2 + 6 * tags.len() + i * script_size;
        scripts.bytes(*tag).u16(at as u16);
    }
    for _ in tags {
        scripts.u16(4).u16(0);
        scripts.u16(0).u16(0xFFFF).u16(1).u16(0);
    }
    // FeatureList: `kern` con el lookup 0.
    let mut features = W::default();
    features.u16(1).bytes(b"kern").u16(8);
    features.u16(0).u16(1).u16(0);
    // El subtable PairPos formato 1: el encabezado, los PairSet y el Coverage.
    let mut firsts: Vec<u16> = pairs.iter().map(|p| p.0).collect();
    firsts.dedup();
    let header = 10 + 2 * firsts.len();
    let mut sets = W::default();
    let mut offsets = Vec::new();
    for &first in &firsts {
        offsets.push(header + sets.0.len());
        let row: Vec<&(u16, u16, i32)> = pairs.iter().filter(|p| p.0 == first).collect();
        sets.u16(row.len() as u16);
        for &&(_, second, v) in &row {
            sets.u16(second).i16(v);
        }
    }
    let coverage_at = header + sets.0.len();
    let mut sub = W::default();
    sub.u16(1).u16(coverage_at as u16).u16(0x0004).u16(0);
    sub.u16(firsts.len() as u16);
    for o in offsets {
        sub.u16(o as u16);
    }
    sub.bytes(&sets.0);
    sub.u16(1).u16(firsts.len() as u16);
    for &g in &firsts {
        sub.u16(g);
    }
    // LookupList: un lookup de tipo 2 con ese subtable.
    let mut lookups = W::default();
    lookups.u16(1).u16(4);
    lookups.u16(2).u16(0).u16(1).u16(8);
    lookups.bytes(&sub.0);
    let script_at = 10;
    let feature_at = script_at + scripts.0.len();
    let lookup_at = feature_at + features.0.len();
    let mut w = W::default();
    w.u16(1).u16(0);
    w.u16(script_at as u16)
        .u16(feature_at as u16)
        .u16(lookup_at as u16);
    w.bytes(&scripts.0).bytes(&features.0).bytes(&lookups.0);
    w.0
}

fn colr(base: &[(usize, usize, usize)], layers: &[(usize, usize)]) -> Vec<u8> {
    let layers_at = 14 + 6 * base.len() as u32;
    let mut w = W::default();
    w.u16(0).u16(base.len() as u16).u32(14).u32(layers_at);
    w.u16(layers.len() as u16);
    for &(gid, first, count) in base {
        w.u16(gid as u16).u16(first as u16).u16(count as u16);
    }
    for &(gid, index) in layers {
        w.u16(gid as u16).u16(index as u16);
    }
    w.0
}

fn cpal(palette: &[Rgba]) -> Vec<u8> {
    let mut w = W::default();
    let n = palette.len() as u16;
    w.u16(0).u16(n).u16(1).u16(n).u32(14).u16(0);
    for c in palette {
        w.u8(c.b).u8(c.g).u8(c.r).u8(c.a);
    }
    w.0
}

/// La suma de un bloque como `u32` big-endian (con ceros al final hasta múltiplo de 4).
fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

/// El archivo: el directorio de tablas (ordenadas por etiqueta), cada tabla alineada a 4 y el
/// `checkSumAdjustment` de `head`.
fn sfnt(tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    let n = tables.len() as u16;
    let power = largest_power_of_two(n);
    let mut w = W::default();
    w.u32(0x0001_0000).u16(n).u16(power * 16);
    w.u16(power.trailing_zeros() as u16)
        .u16(n * 16 - power * 16);
    let mut offset = 12 + 16 * n as u32;
    let mut head_at = None;
    for (tag, data) in &tables {
        if tag == b"head" {
            head_at = Some(offset as usize);
        }
        w.bytes(tag).u32(checksum(data)).u32(offset);
        w.u32(data.len() as u32);
        offset += (data.len() as u32).div_ceil(4) * 4;
    }
    for (_, data) in &tables {
        w.bytes(data);
        while w.0.len() % 4 != 0 {
            w.u8(0);
        }
    }
    if let Some(at) = head_at {
        let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&w.0));
        w.0[at + 8..at + 12].copy_from_slice(&adjust.to_be_bytes());
    }
    w.0
}

#[cfg(test)]
#[path = "ttf_tests.rs"]
mod tests;
