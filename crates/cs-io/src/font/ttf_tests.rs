use std::collections::BTreeSet;

use cs_core::{Artboard, FontMode, Rgba};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use super::*;
use crate::font::sample_font;

fn ttf(mode: FontMode, color: bool) -> Vec<u8> {
    let options = TtfOptions {
        family: String::new(),
        color,
    };
    export_ttf(&sample_font(mode), &options).unwrap()
}

/// Cuenta los contornos y los puntos de un glifo.
#[derive(Default)]
struct Count {
    contours: usize,
    points: usize,
}

impl OutlineBuilder for Count {
    fn move_to(&mut self, _: f32, _: f32) {
        self.contours += 1;
        self.points += 1;
    }
    fn line_to(&mut self, _: f32, _: f32) {
        self.points += 1;
    }
    fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
        panic!("only straight lines");
    }
    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
        panic!("only straight lines");
    }
    fn close(&mut self) {}
}

#[test]
fn the_ttf_reads_back_with_its_metrics_and_name() {
    let bytes = ttf(FontMode::OneBit, false);
    let face = Face::parse(&bytes, 0).unwrap();
    // .notdef, el espacio y los seis glifos.
    assert_eq!(face.number_of_glyphs(), 8);
    assert_eq!(face.units_per_em(), 10 * 64);
    assert_eq!((face.ascender(), face.descender()), (8 * 64, -2 * 64));
    assert_eq!(face.line_gap(), 0);
    assert_eq!(face.capital_height(), Some(7 * 64));
    assert_eq!(face.x_height(), Some(5 * 64));
    let family = face
        .names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
        .and_then(|n| n.to_string());
    assert_eq!(family.as_deref(), Some("Sample"));
    assert!(!face.is_monospaced());
}

#[test]
fn the_cmap_and_the_advances_are_those_of_the_font() {
    let artboard = sample_font(FontMode::OneBit);
    let font = artboard.font.as_ref().unwrap();
    let bytes = ttf(FontMode::OneBit, false);
    let face = Face::parse(&bytes, 0).unwrap();
    let space = face.glyph_index(' ').unwrap();
    assert_eq!(face.glyph_hor_advance(space), Some(3 * 64));
    for (i, g) in font.glyphs.iter().enumerate() {
        let id = face.glyph_index(g.ch).unwrap_or_else(|| panic!("{}", g.ch));
        let advance = font.advance(&artboard.sprite, i) as u16 * 64;
        assert_eq!(face.glyph_hor_advance(id), Some(advance), "{}", g.ch);
    }
    // A: 5 de tinta + 1; i: manual 3; z vacía: media celda.
    let adv = |c| {
        face.glyph_hor_advance(face.glyph_index(c).unwrap())
            .unwrap()
            / 64
    };
    assert_eq!((adv('A'), adv('i'), adv('z')), (6, 3, 4));
    assert!(face.glyph_index('\u{1F600}').is_some(), "format 12");
    assert!(face.glyph_index('B').is_none());
    // El .notdef guarda el ancho de la celda.
    assert_eq!(face.glyph_hor_advance(GlyphId(0)), Some(8 * 64));
}

#[test]
fn the_glyphs_are_merged_pixel_outlines() {
    let bytes = ttf(FontMode::OneBit, false);
    let face = Face::parse(&bytes, 0).unwrap();
    let a = face.glyph_index('A').unwrap();
    let mut count = Count::default();
    let bbox = face.outline_glyph(a, &mut count).unwrap();
    // La A: dos contornos, el techo (toca el cuerpo solo por las esquinas) y el cuerpo.
    assert_eq!(count.contours, 2);
    assert_eq!(
        (bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max),
        (0, 0, 5 * 64, 7 * 64)
    );
    let g = face.glyph_index('g').unwrap();
    let bbox = face.glyph_bounding_box(g).unwrap();
    assert_eq!(bbox.y_min, -64, "the descender goes below the baseline");
    let z = face.glyph_index('z').unwrap();
    let mut empty = Count::default();
    face.outline_glyph(z, &mut empty);
    assert_eq!(empty.points, 0);
}

/// Los pixeles prendidos (columna, fila desde arriba de la celda) del glifo `ch` como los pinta
/// `fontdue` (la herramienta Texto) a `px`.
fn fontdue_pixels(bytes: &[u8], ch: char, px: f32, baseline: i32) -> BTreeSet<(i32, i32)> {
    let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).unwrap();
    let (m, coverage) = font.rasterize(ch, px);
    let mut out = BTreeSet::new();
    for by in 0..m.height {
        for bx in 0..m.width {
            if coverage[by * m.width + bx] >= 128 {
                let top = m.ymin + m.height as i32;
                out.insert((m.xmin + bx as i32, baseline - top + by as i32));
            }
        }
    }
    out
}

#[test]
fn fontdue_draws_the_same_pixels_at_the_cell_height() {
    let artboard = sample_font(FontMode::OneBit);
    let view = artboard.font_view().unwrap();
    let bytes = ttf(FontMode::OneBit, false);
    for (ch, glyph) in &view.glyphs {
        let expected: BTreeSet<(i32, i32)> = glyph
            .pixels
            .iter()
            .map(|&(x, y, _)| (x as i32, y as i32))
            .collect();
        assert_eq!(fontdue_pixels(&bytes, *ch, 10.0, 8), expected, "{ch}");
    }
}

/// Junta las capas que pinta `paint_color_glyph`: el glifo de cada una y su color.
#[derive(Default)]
struct Layers(Vec<(GlyphId, Rgba)>, Option<GlyphId>);

impl<'a> ttf_parser::colr::Painter<'a> for Layers {
    fn outline_glyph(&mut self, glyph_id: GlyphId) {
        self.1 = Some(glyph_id);
    }
    fn paint(&mut self, paint: ttf_parser::colr::Paint<'a>) {
        if let ttf_parser::colr::Paint::Solid(c) = paint {
            let color = Rgba::new(c.red, c.green, c.blue, c.alpha);
            self.0.push((self.1.unwrap(), color));
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

#[test]
fn a_color_font_has_colr_layers_and_the_silhouette() {
    let bytes = ttf(FontMode::Color, true);
    let face = Face::parse(&bytes, 0).unwrap();
    let a = face.glyph_index('A').unwrap();
    assert!(face.is_color_glyph(a));
    let mut layers = Layers::default();
    let black = ttf_parser::RgbaColor::new(0, 0, 0, 255);
    face.paint_color_glyph(a, 0, black, &mut layers).unwrap();
    let colors: Vec<Rgba> = layers.0.iter().map(|(_, c)| *c).collect();
    assert_eq!(
        colors,
        vec![Rgba::new(220, 30, 40, 255), Rgba::new(20, 40, 200, 255)],
        "red first (its first pixel is higher), then blue"
    );
    // La capa azul es solo la barra; la silueta (el glifo base) es la A entera.
    let blue = layers.0[1].0;
    let bar = face.glyph_bounding_box(blue).unwrap();
    assert_eq!((bar.y_min, bar.y_max), (3 * 64, 4 * 64));
    let base = face.glyph_bounding_box(a).unwrap();
    assert_eq!((base.y_min, base.y_max), (0, 7 * 64));
    // Los glifos capa no tienen código; el vacío no es de color.
    assert!(!face.is_color_glyph(face.glyph_index('z').unwrap()));
    assert_eq!(face.glyph_hor_advance(blue), face.glyph_hor_advance(a));
}

#[test]
fn without_color_or_in_one_bit_there_is_no_colr() {
    for bytes in [ttf(FontMode::Color, false), ttf(FontMode::OneBit, true)] {
        let face = Face::parse(&bytes, 0).unwrap();
        assert!(face.tables().colr.is_none());
        assert_eq!(face.number_of_glyphs(), 8);
    }
}

#[test]
fn the_checksums_add_up() {
    let bytes = ttf(FontMode::Color, true);
    assert_eq!(checksum(&bytes), 0xB1B0_AFBA);
    assert_eq!(bytes.len() % 4, 0);
}

#[test]
fn monospace_and_the_family_option_reach_the_file() {
    let mut artboard = sample_font(FontMode::OneBit);
    artboard.font.as_mut().unwrap().monospace = true;
    let options = TtfOptions {
        family: "  My Pixels ".into(),
        color: false,
    };
    let bytes = export_ttf(&artboard, &options).unwrap();
    let face = Face::parse(&bytes, 0).unwrap();
    assert!(face.is_monospaced());
    let name = |id| {
        face.names()
            .into_iter()
            .find(|n| n.name_id == id)
            .and_then(|n| n.to_string())
    };
    assert_eq!(
        name(ttf_parser::name_id::FAMILY).as_deref(),
        Some("My Pixels")
    );
    assert_eq!(
        name(ttf_parser::name_id::POST_SCRIPT_NAME).as_deref(),
        Some("MyPixels")
    );
    let a = face.glyph_index('A').unwrap();
    assert_eq!(face.glyph_hor_advance(a), Some(8 * 64));
}

#[test]
fn a_tall_cell_uses_fewer_units_per_pixel() {
    assert_eq!(units_per_pixel(16), 64);
    assert_eq!(units_per_pixel(256), 64);
    assert_eq!(units_per_pixel(400), 40);
}

#[test]
fn a_plain_artboard_is_not_a_font() {
    let mut artboard = sample_font(FontMode::OneBit);
    artboard.font = None;
    let options = TtfOptions {
        family: String::new(),
        color: false,
    };
    assert!(export_ttf(&artboard, &options).is_err());
}

/// Con `CHIMPSPRITE_FONT_DUMP=<carpeta>`, escribe las fuentes de prueba para revisarlas con
/// otras herramientas (fontTools, FontForge…).
#[test]
fn dump_for_external_tools() {
    let Some(dir) = std::env::var_os("CHIMPSPRITE_FONT_DUMP") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::write(dir.join("sample-1bit.ttf"), ttf(FontMode::OneBit, false)).unwrap();
    std::fs::write(dir.join("sample-color.ttf"), ttf(FontMode::Color, true)).unwrap();
    let options = TtfOptions {
        family: String::new(),
        color: false,
    };
    let bold = export_ttf(&styled(bold()), &options).unwrap();
    std::fs::write(dir.join("sample-bold-kern.ttf"), bold).unwrap();
}

/// La fuente de prueba con un estilo y unos pares de kerning (A·g −1, g·i +2, A·€ −3).
fn styled(style: cs_core::FontStyle) -> Artboard {
    use cs_core::KernSide::Char;
    let mut artboard = sample_font(FontMode::OneBit);
    let font = artboard.font.as_mut().unwrap();
    font.style = style;
    font.kerning.set(Char('A'), Char('g'), -1);
    font.kerning.set(Char('g'), Char('i'), 2);
    font.kerning.set(Char('A'), Char('€'), -3);
    font.kerning.set(Char('A'), Char('q'), -5); // la fuente no tiene `q`
    artboard
}

fn name_of(face: &Face, id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .find(|n| n.name_id == id && n.is_unicode())
        .and_then(|n| n.to_string())
}

fn bold() -> cs_core::FontStyle {
    cs_core::FontStyle {
        name: "Bold".into(),
        weight: 700,
        ..Default::default()
    }
}

#[test]
fn the_style_goes_to_name_os2_head_and_post() {
    let options = TtfOptions {
        family: String::new(),
        color: false,
    };
    let bytes = export_ttf(&styled(bold()), &options).unwrap();
    let face = Face::parse(&bytes, 0).unwrap();
    assert_eq!(name_of(&face, 1).as_deref(), Some("Sample"));
    assert_eq!(name_of(&face, 2).as_deref(), Some("Bold"));
    assert_eq!(name_of(&face, 4).as_deref(), Some("Sample Bold"));
    assert_eq!(name_of(&face, 6).as_deref(), Some("Sample-Bold"));
    assert_eq!(name_of(&face, 16), None, "RIBBI: no typographic family");
    assert!(face.is_bold() && !face.is_italic());
    assert_eq!(face.weight(), ttf_parser::Weight::Bold);
    let os2 = bytes_of(&face, b"OS/2");
    assert_eq!(
        u16::from_be_bytes([os2[62], os2[63]]),
        0x80 | 0x20,
        "BOLD, not REGULAR"
    );
    // Italic, 1 columna cada 3 filas.
    let italic = cs_core::FontStyle {
        name: "Italic".into(),
        italic: true,
        slant: 3,
        ..Default::default()
    };
    let bytes = export_ttf(&styled(italic), &options).unwrap();
    let face = Face::parse(&bytes, 0).unwrap();
    assert!(face.is_italic() && !face.is_bold());
    let angle = face.italic_angle();
    assert!((angle + 18.43).abs() < 0.01, "{angle}");
    let mac_style =
        u16::from_be_bytes([bytes_of(&face, b"head")[44], bytes_of(&face, b"head")[45]]);
    assert_eq!(mac_style, 0b10);
    // Un Outline generado: no es RIBBI, lleva la familia tipográfica y OUTLINED.
    let outline = cs_core::FontStyle::for_variant(
        cs_core::ArtboardId(1),
        cs_core::VariantRecipe::new(cs_core::VariantKind::Outline),
        "Outline",
    );
    let bytes = export_ttf(&styled(outline), &options).unwrap();
    let face = Face::parse(&bytes, 0).unwrap();
    assert_eq!(name_of(&face, 1).as_deref(), Some("Sample Outline"));
    assert_eq!(name_of(&face, 2).as_deref(), Some("Regular"));
    assert_eq!(name_of(&face, 16).as_deref(), Some("Sample"));
    assert_eq!(name_of(&face, 17).as_deref(), Some("Outline"));
    assert!(face.is_regular());
    assert!(face.style() == ttf_parser::Style::Normal);
    let os2 = bytes_of(&face, b"OS/2");
    let selection = u16::from_be_bytes([os2[62], os2[63]]);
    assert_eq!(selection, 0x80 | 0x40 | 0x08);
}

/// Los bytes de la tabla `tag`.
fn bytes_of<'a>(face: &Face<'a>, tag: &[u8; 4]) -> &'a [u8] {
    face.raw_face()
        .table(ttf_parser::Tag::from_bytes(tag))
        .unwrap()
}

#[test]
fn kerning_goes_to_the_kern_table_and_fontdue_reads_it() {
    let options = TtfOptions {
        family: String::new(),
        color: false,
    };
    let bytes = export_ttf(&styled(Default::default()), &options).unwrap();
    let font =
        fontdue::Font::from_bytes(bytes.as_slice(), fontdue::FontSettings::default()).unwrap();
    // A 10 px (el em), 1 pixel de la fuente = 1 px.
    assert_eq!(font.horizontal_kern('A', 'g', 10.0), Some(-1.0));
    assert_eq!(font.horizontal_kern('g', 'i', 10.0), Some(2.0));
    assert_eq!(font.horizontal_kern('A', '€', 10.0), Some(-3.0));
    assert_eq!(font.horizontal_kern('g', 'A', 10.0), None);
    let face = Face::parse(&bytes, 0).unwrap();
    let kern = face.tables().kern.unwrap();
    let sub = kern.subtables.into_iter().next().unwrap();
    assert!(sub.horizontal);
    let (a, g) = (
        face.glyph_index('A').unwrap(),
        face.glyph_index('g').unwrap(),
    );
    assert_eq!(sub.glyphs_kerning(a, g), Some(-64));
}

#[test]
fn kerning_goes_to_gpos_with_the_kern_feature() {
    use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
    let options = TtfOptions {
        family: String::new(),
        color: false,
    };
    let bytes = export_ttf(&styled(Default::default()), &options).unwrap();
    let face = Face::parse(&bytes, 0).unwrap();
    let gpos = face.tables().gpos.expect("a GPOS table");
    let scripts: Vec<String> = gpos
        .scripts
        .into_iter()
        .map(|s| s.tag.to_string())
        .collect();
    assert_eq!(scripts, vec!["DFLT", "latn"]);
    let feature = gpos
        .features
        .find(ttf_parser::Tag::from_bytes(b"kern"))
        .expect("the kern feature");
    let lookup = gpos
        .lookups
        .get(feature.lookup_indices.get(0).unwrap())
        .unwrap();
    let sub: PositioningSubtable = lookup.subtables.get(0).unwrap();
    let PositioningSubtable::Pair(PairAdjustment::Format1 { coverage, sets }) = sub else {
        panic!("PairPos format 1");
    };
    let id = |c| face.glyph_index(c).unwrap();
    let value = |a, b| {
        let set = sets.get(coverage.get(id(a))?)?;
        set.get(id(b)).map(|(v, _)| v.x_advance)
    };
    assert_eq!(value('A', 'g'), Some(-64));
    assert_eq!(value('A', '€'), Some(-3 * 64));
    assert_eq!(value('g', 'i'), Some(2 * 64));
    assert_eq!(value('g', 'A'), None);
    assert_eq!(value('i', 'g'), None);
}

#[test]
fn without_kerning_there_are_no_kern_tables_and_too_many_pairs_is_an_error() {
    let bytes = ttf(FontMode::OneBit, false);
    let face = Face::parse(&bytes, 0).unwrap();
    assert!(face.tables().kern.is_none() && face.tables().gpos.is_none());
    // 101 × 100 pares de caracteres de uso privado.
    let mut artboard = sample_font(FontMode::OneBit);
    let font = artboard.font.as_mut().unwrap();
    let chars: Vec<char> = (0xE000..0xE000 + 101).filter_map(char::from_u32).collect();
    font.glyphs
        .extend(chars.iter().map(|&c| cs_core::GlyphInfo::new(c)));
    font.glyphs.sort_by_key(|g| g.ch);
    artboard.sprite.frames = vec![cs_core::Frame { duration_ms: 100 }; font.glyphs.len()];
    let font = artboard.font.as_mut().unwrap();
    for &a in &chars {
        for &b in &chars[..100] {
            font.kerning
                .set(cs_core::KernSide::Char(a), cs_core::KernSide::Char(b), 1);
        }
    }
    let options = TtfOptions {
        family: String::new(),
        color: false,
    };
    let err = export_ttf(&artboard, &options).unwrap_err().to_string();
    assert!(err.contains("too many kerning pairs (10100"), "{err}");
}
