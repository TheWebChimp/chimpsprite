use std::sync::Arc;

use super::*;
use crate::{Cel, FontMode, GlyphInfo, Image, ONE_BIT_INK, font_sprite};

/// Una fuente 8×10 (baseline 8) con los glifos de `glyphs` dibujados (columna, fila).
fn font_with(glyphs: &[(char, &[(u32, u32)])]) -> (FontInfo, Sprite) {
    let mut font = FontInfo::new("Mine", 10, FontMode::OneBit);
    font.metrics.baseline = 8;
    for &(ch, _) in glyphs {
        if let Err(i) = font.insert_index(ch) {
            font.glyphs.insert(i, GlyphInfo::new(ch));
        }
    }
    let mut s = font_sprite((8, 10), FontMode::OneBit, font.glyphs.len());
    let layer = s.layers[0].id;
    for &(ch, points) in glyphs {
        let mut image = Image::new(8, 10, s.color_mode);
        for &(x, y) in points {
            image.set(x, y, ONE_BIT_INK).unwrap();
        }
        let id = s.next_image_id();
        s.images.insert(id, Arc::new(image));
        let cel = Cel {
            position: (0, 0),
            opacity: 255,
            image: id,
        };
        s.cels.insert((layer, font.index_of(ch).unwrap()), cel);
    }
    (font, s)
}

/// Un rectángulo lleno de columnas `x0..x1` y filas `y0..y1`.
fn block(x0: u32, x1: u32, y0: u32, y1: u32) -> Vec<(u32, u32)> {
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .collect()
}

fn mark_of(plan: &AccentPlan) -> GlyphComponent {
    plan.components[1]
}

#[test]
fn the_table_is_sorted_complete_and_right() {
    assert_eq!(DECOMPOSITIONS.len(), 165);
    assert!(DECOMPOSITIONS.windows(2).all(|w| w[0].0 < w[1].0));
    assert_eq!(decomposition('á'), Some(('a', '\u{0301}')));
    assert_eq!(decomposition('Ç'), Some(('C', '\u{0327}')));
    assert_eq!(decomposition('ő'), Some(('o', '\u{030B}')));
    assert_eq!(decomposition('ș'), Some(('s', '\u{0326}')));
    assert_eq!(decomposition('ß'), None, "no canonical decomposition");
    assert_eq!(decomposition('Ł'), None);
    assert!(accent_chars().all(|c| decomposition(c).is_some()));
    // Toda marca de la tabla tiene de dónde salir.
    for &(ch, _, mark) in DECOMPOSITIONS {
        assert!(mark_sources(ch, mark).len() >= 2, "{ch}");
    }
}

#[test]
fn capitals_prefer_the_combining_mark_and_lowercase_the_spacing_one() {
    assert_eq!(mark_sources('á', '\u{0301}'), vec!['´', '\u{0301}']);
    assert_eq!(mark_sources('Á', '\u{0301}'), vec!['\u{0301}', '´']);
    assert_eq!(mark_sources('â', '\u{0302}'), vec!['ˆ', '^', '\u{0302}']);
}

#[test]
fn a_mark_above_is_centered_on_the_ink_one_pixel_over_it() {
    let a = block(0, 4, 4, 8);
    let (font, s) = font_with(&[('a', &a), ('´', &[(2, 0), (1, 1)])]);
    let plan = font.plan_accent(&s, 'á').unwrap();
    assert_eq!(
        plan.components[0],
        GlyphComponent {
            ch: 'a',
            offset: (0, 0)
        }
    );
    // La marca (columnas 1..3, filas 0..2) va a las columnas 1..3 y las filas 1..3: la 3 queda
    // libre sobre la `a`, que empieza en la 4.
    assert_eq!(
        mark_of(&plan),
        GlyphComponent {
            ch: '´',
            offset: (0, 1)
        }
    );
    assert!(!plan.cramped);
    let mut font = font;
    let i = font.index_of('á');
    assert!(i.is_none());
    font.glyphs.push(GlyphInfo {
        components: plan.components,
        ..GlyphInfo::new('á')
    });
    let mut s = s;
    s.frames.push(crate::Frame { duration_ms: 100 });
    let b = font.ink_bounds(&s, font.glyphs.len() - 1).unwrap();
    assert_eq!((b.y0, b.y1), (1, 8));
}

#[test]
fn a_mark_that_does_not_fit_goes_as_high_as_it_can_and_capitals_use_the_cap_mark() {
    let cap_a = block(0, 5, 2, 8);
    let acute: &[(u32, u32)] = &[(2, 0), (1, 1)];
    let (font, s) = font_with(&[('A', &cap_a), ('´', acute)]);
    let plan = font.plan_accent(&s, 'Á').unwrap();
    assert_eq!(mark_of(&plan).ch, '´');
    assert_eq!(mark_of(&plan).offset.1, 0, "row 0: no room for its air");
    assert!(plan.cramped);
    // Con la versión cap (la combinante, de una fila), cabe.
    let (font, s) = font_with(&[('A', &cap_a), ('´', acute), ('\u{0301}', &[(1, 5), (2, 5)])]);
    let plan = font.plan_accent(&s, 'Á').unwrap();
    assert_eq!(
        mark_of(&plan),
        GlyphComponent {
            ch: '\u{0301}',
            offset: (0, -5)
        }
    );
    assert!(!plan.cramped);
    // Las minúsculas siguen con la espaciadora.
    let (font, s) = font_with(&[
        ('a', &block(0, 4, 4, 8)),
        ('´', acute),
        ('\u{0301}', &[(1, 5)]),
    ]);
    assert_eq!(mark_of(&font.plan_accent(&s, 'á').unwrap()).ch, '´');
}

#[test]
fn cedillas_hang_below_and_ogoneks_to_the_right() {
    let c = block(0, 4, 4, 8);
    let (font, s) = font_with(&[
        ('c', &c),
        ('¸', &[(1, 0), (1, 1)]),
        ('˛', &[(0, 0), (1, 1)]),
    ]);
    let plan = font.plan_accent(&s, 'ç').unwrap();
    // La cedilla (columna 1, filas 0..2): centrada en 0..4 (columna 1) y pegada en la fila 8.
    assert_eq!(mark_of(&plan).offset, (0, 8));
    assert!(!plan.cramped);
    let (font, s) = font_with(&[('a', &c), ('˛', &[(0, 0), (1, 1)])]);
    let plan = font.plan_accent(&s, 'ą').unwrap();
    assert_eq!(mark_of(&plan).offset, (2, 8), "right edge on the a's");
}

#[test]
fn a_cedilla_on_a_descender_goes_above() {
    let g = block(0, 4, 4, 10);
    let (font, s) = font_with(&[('g', &g), ('¸', &[(1, 0), (1, 1)])]);
    let plan = font.plan_accent(&s, 'ģ').unwrap();
    assert_eq!(mark_of(&plan).offset, (0, 1), "rows 1..3, air at 3");
}

#[test]
fn i_uses_the_dotless_i_and_d_caron_the_apostrophe() {
    let i: &[(u32, u32)] = &[(0, 2), (0, 4), (0, 5), (0, 6), (0, 7)];
    let dotless: &[(u32, u32)] = &[(0, 4), (0, 5), (0, 6), (0, 7)];
    let acute: &[(u32, u32)] = &[(0, 0)];
    let (font, s) = font_with(&[('i', i), ('ı', dotless), ('´', acute)]);
    let plan = font.plan_accent(&s, 'í').unwrap();
    assert_eq!(plan.components[0].ch, 'ı');
    assert_eq!(mark_of(&plan).offset, (0, 2));
    let (font, s) = font_with(&[('i', i), ('´', acute)]);
    assert_eq!(font.plan_accent(&s, 'í').unwrap().components[0].ch, 'i');
    let d = block(0, 4, 1, 8);
    let (font, s) = font_with(&[('d', &d), ('\'', &[(0, 0), (0, 1)]), ('ˇ', acute)]);
    let plan = font.plan_accent(&s, 'ď').unwrap();
    assert_eq!(
        mark_of(&plan),
        GlyphComponent {
            ch: '\'',
            offset: (5, 1)
        }
    );
    let (font, s) = font_with(&[('d', &d), ('ˇ', acute)]);
    assert_eq!(mark_of(&font.plan_accent(&s, 'ď').unwrap()).ch, 'ˇ');
}

#[test]
fn what_cannot_be_built_says_why() {
    let (font, s) = font_with(&[('a', &block(0, 4, 4, 8))]);
    assert_eq!(font.plan_accent(&s, 'ß'), Err(AccentSkip::Unknown));
    assert_eq!(font.plan_accent(&s, 'é'), Err(AccentSkip::NoBase));
    assert_eq!(font.plan_accent(&s, 'á'), Err(AccentSkip::NoMark));
}
