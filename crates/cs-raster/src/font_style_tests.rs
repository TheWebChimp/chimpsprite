use cs_core::{FontBuild, FontMetrics, GlyphBuild, GlyphComponent, KernSide, Rgba, build_font};

use super::*;

/// Una fuente de celda 8×8 (baseline 6) con un glifo por `(carácter, filas)`: `X` es tinta (en
/// color, roja), desde la fila 0.
fn font(mode: FontMode, glyphs: &[(char, &[&str])]) -> (FontInfo, Sprite) {
    let red = Some(Rgba::new(200, 0, 0, 255));
    let (mut font, sprite) = build_font(FontBuild {
        name: "Mine".into(),
        mode,
        cell: (8, 8),
        metrics: FontMetrics {
            baseline: 6,
            cap_height: 5,
            x_height: 3,
            descent: 2,
        },
        letter_spacing: 1,
        space_width: 3,
        monospace: false,
        glyphs: glyphs
            .iter()
            .map(|(ch, rows)| GlyphBuild {
                ch: *ch,
                advance: None,
                pixels: rows
                    .iter()
                    .enumerate()
                    .flat_map(|(y, row)| {
                        row.chars()
                            .enumerate()
                            .filter(|(_, c)| *c == 'X')
                            .map(move |(x, _)| (x as u32, y as u32, red))
                    })
                    .collect(),
            })
            .collect(),
        kerning: Vec::new(),
        style: Default::default(),
    });
    font.kerning
        .set(KernSide::Char('a'), KernSide::Char('b'), -1);
    (font, sprite)
}

/// Las filas del glifo `i` del sprite (`X` pintado), de su celda entera.
fn rows(sprite: &Sprite, i: usize) -> Vec<String> {
    let layer = sprite.layers[0].id;
    (0..sprite.height as i32)
        .map(|y| {
            (0..sprite.width as i32)
                .map(|x| {
                    if painted(sprite.color_mode, sprite.pixel(layer, i, x, y)) {
                        'X'
                    } else {
                        '.'
                    }
                })
                .collect::<String>()
                .trim_end_matches('.')
                .to_owned()
        })
        .collect()
}

fn recipe(kind: VariantKind) -> VariantRecipe {
    VariantRecipe::new(kind)
}

#[test]
fn bold_paints_each_pixel_one_to_the_right_too() {
    let (f, s) = font(FontMode::OneBit, &[('a', &["", "X.X", "X"])]);
    let (bold, out) = generate_variant(&f, &s, &recipe(VariantKind::Bold));
    assert_eq!(&rows(&out, 0)[..3], &["", "XXXX", "XX"]);
    assert_eq!((out.width, out.height), (8, 8), "it still fits");
    assert_eq!(bold.kerning.value('a', 'b'), -1, "the kerning goes along");
    assert_eq!(bold.name, "Mine");
    assert_eq!(bold.metrics, f.metrics);
    // En color, el pixel nuevo es del color del de su izquierda.
    let (f, s) = font(FontMode::Color, &[('a', &["X"])]);
    let (_, out) = generate_variant(&f, &s, &recipe(VariantKind::Bold));
    let layer = out.layers[0].id;
    assert_eq!(
        out.pixel(layer, 0, 1, 0),
        PixelValue::Rgba(Rgba::new(200, 0, 0, 255))
    );
}

#[test]
fn italic_slants_the_rows_above_the_baseline_and_keeps_the_descenders() {
    // Una barra de la fila 0 a la 7 (baseline 6: filas 6 y 7 son descendentes).
    let bar: &[&str] = &["X", "X", "X", "X", "X", "X", "X", "X"];
    let (f, s) = font(FontMode::OneBit, &[('l', bar)]);
    let (_, out) = generate_variant(&f, &s, &recipe(VariantKind::Italic));
    // Filas desde la de arriba de la baseline (la 5): 5,4,3 → 0; 2,1,0 → 1 (cada 3).
    assert_eq!(
        rows(&out, 0),
        vec![".X", ".X", ".X", "X", "X", "X", "X", "X"]
    );
    let steep = VariantRecipe {
        slant: 1,
        ..recipe(VariantKind::Italic)
    };
    let (_, out) = generate_variant(&f, &s, &steep);
    assert_eq!(rows(&out, 0)[0], ".....X", "five rows up, five columns");
    assert_eq!(italic_shift(6, 6, 3), 0);
    assert_eq!(italic_shift(5, 6, 3), 0);
    assert_eq!(italic_shift(2, 6, 3), 1);
}

#[test]
fn bold_italic_is_bold_then_italic() {
    let (f, s) = font(FontMode::OneBit, &[('l', &["X", "X", "X", "X"])]);
    let (_, out) = generate_variant(&f, &s, &recipe(VariantKind::BoldItalic));
    // Filas 0..4 sobre la baseline 6: la 0 sube 5 filas → 1; las demás, 0 a 1.
    assert_eq!(
        rows(&out, 0),
        vec![".XX", ".XX", ".XX", "XX", "", "", "", ""]
    );
}

#[test]
fn a_one_bit_outline_is_hollow_and_moves_everything_to_fit() {
    let (f, s) = font(FontMode::OneBit, &[('o', &["", "X"])]);
    let (info, out) = generate_variant(&f, &s, &recipe(VariantKind::Outline));
    // El contorno de un pixel en (0, 1) con 4 vecinos se sale por la izquierda: todo se corre
    // 1 a la derecha (la celda crece 1); arriba llega a la fila 0, sin salirse.
    assert_eq!(&rows(&out, 0)[..4], &[".X", "X.X", ".X", ""]);
    assert_eq!((out.width, out.height), (9, 8));
    assert_eq!(info.metrics.baseline, 6, "nothing went above");
    let square = VariantRecipe {
        square: true,
        ..recipe(VariantKind::Outline)
    };
    let (_, out) = generate_variant(&f, &s, &square);
    assert_eq!(&rows(&out, 0)[..4], &["XXX", "X.X", "XXX", ""]);
}

#[test]
fn a_color_outline_keeps_the_letter_and_adds_the_ring_in_its_color() {
    let (f, s) = font(FontMode::Color, &[('o', &["", "X"])]);
    let blue = Rgba::new(0, 0, 200, 255);
    let r = VariantRecipe {
        color: blue,
        ..recipe(VariantKind::Outline)
    };
    let (_, out) = generate_variant(&f, &s, &r);
    let layer = out.layers[0].id;
    assert_eq!(
        out.pixel(layer, 0, 1, 1),
        PixelValue::Rgba(Rgba::new(200, 0, 0, 255))
    );
    assert_eq!(out.pixel(layer, 0, 0, 1), PixelValue::Rgba(blue));
    assert_eq!(out.pixel(layer, 0, 1, 0), PixelValue::Rgba(blue));
}

#[test]
fn things_that_go_above_the_cell_move_the_baseline_down() {
    let (f, s) = font(FontMode::OneBit, &[('T', &["X"])]);
    let (info, out) = generate_variant(&f, &s, &recipe(VariantKind::Outline));
    assert_eq!((out.width, out.height), (9, 9));
    assert_eq!(info.metrics.baseline, 7);
    assert_eq!(info.metrics.descent, 2);
    assert_eq!(&rows(&out, 0)[..3], &[".X", "X.X", ".X"]);
}

#[test]
fn shadows_in_color_and_in_one_bit() {
    let (f, s) = font(FontMode::Color, &[('i', &["X", "X"])]);
    let gray = Rgba::new(90, 90, 90, 255);
    let r = VariantRecipe {
        color: gray,
        ..recipe(VariantKind::Shadow)
    };
    let (_, out) = generate_variant(&f, &s, &r);
    let layer = out.layers[0].id;
    assert_eq!(out.pixel(layer, 0, 1, 1), PixelValue::Rgba(gray));
    assert_eq!(out.pixel(layer, 0, 1, 2), PixelValue::Rgba(gray));
    assert_eq!(
        out.pixel(layer, 0, 0, 1),
        PixelValue::Rgba(Rgba::new(200, 0, 0, 255)),
        "the letter on top"
    );
    // 1 bit: hueca, con el borde de abajo y la derecha de 2.
    let (f, s) = font(FontMode::OneBit, &[('i', &["", "X"])]);
    let (_, out) = generate_variant(&f, &s, &recipe(VariantKind::Shadow));
    assert_eq!(&rows(&out, 0)[..4], &[".X", "X.X", ".XXX", "..X"]);
}

#[test]
fn manual_advances_grow_with_the_letter_and_copy_changes_nothing() {
    let (mut f, s) = font(FontMode::OneBit, &[('a', &["", "X"]), ('b', &["XX"])]);
    f.glyphs[0].advance = Some(4);
    let (bold, _) = generate_variant(&f, &s, &recipe(VariantKind::Bold));
    assert_eq!(bold.glyphs[0].advance, Some(5));
    assert_eq!(bold.glyphs[1].advance, None, "automatic stays automatic");
    let (outline, _) = generate_variant(&f, &s, &recipe(VariantKind::Outline));
    assert_eq!(outline.glyphs[0].advance, Some(6), "1 at each side");
    let (italic, _) = generate_variant(&f, &s, &recipe(VariantKind::Italic));
    assert_eq!(italic.glyphs[0].advance, Some(4));
    let (copy, out) = generate_variant(&f, &s, &recipe(VariantKind::Copy));
    assert_eq!(copy, f);
    assert_eq!(rows(&out, 1), rows(&s, 1));
}

#[test]
fn composites_stay_composites_and_italics_move_their_marks() {
    let (mut f, s) = font(
        FontMode::OneBit,
        &[
            ('a', &["", "", "", "X", "X", "X"]),
            ('´', &["X"]),
            ('á', &[]),
        ],
    );
    let accented = f.index_of('á').unwrap();
    f.glyphs[accented].components = vec![
        GlyphComponent {
            ch: 'a',
            offset: (0, 0),
        },
        GlyphComponent {
            ch: '´',
            offset: (0, 1),
        },
    ];
    let (bold, _) = generate_variant(&f, &s, &recipe(VariantKind::Bold));
    assert_eq!(
        bold.glyphs[accented].components,
        f.glyphs[accented].components
    );
    let steep = VariantRecipe {
        slant: 1,
        ..recipe(VariantKind::Italic)
    };
    let (italic, out) = generate_variant(&f, &s, &steep);
    // La `´` se dibuja en la fila 0 (corrida 5) y va en la 1 (debe correrse 4): −1.
    assert_eq!(italic.glyphs[accented].components[1].offset, (-1, 1));
    let composed = italic.composed(&out, accented);
    let mark = composed.keys().find(|(y, _)| *y == 1).copied();
    assert_eq!(mark, Some((1, 4)), "four columns at row 1");
    assert!(
        italic
            .kerning
            .index_of(&KernSide::Char('a'), &KernSide::Char('b'))
            .is_some()
    );
}
