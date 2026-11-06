use super::*;
use cs_core::{ColorMode, Document};

/// Un documento con una fuente 8×10 de 1 bit (baseline 7) con `´` agregada, la `a` (columnas
/// 0..4, filas 3..7) y la `´` (columnas 1..3, filas 0..2) dibujadas.
fn doc_with_accent_parts() -> (Document, ArtboardId) {
    let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
    let (id, cmd) = new_font_artboard(&doc, "Mine", (8, 10), FontMode::OneBit, (40, 0));
    cmd.apply(&mut doc).unwrap();
    add_glyphs(doc.artboard(id).unwrap(), &['´'])
        .unwrap()
        .apply(&mut doc)
        .unwrap();
    let a: Vec<(i32, i32)> = (3..7).flat_map(|y| (0..4).map(move |x| (x, y))).collect();
    draw(&mut doc, id, 'a', &a);
    draw(&mut doc, id, '´', &[(2, 0), (1, 1)]);
    (doc, id)
}

fn draw(doc: &mut Document, id: ArtboardId, ch: char, points: &[(i32, i32)]) {
    let a = doc.artboard(id).unwrap();
    let frame = a.font.as_ref().unwrap().index_of(ch).unwrap();
    let layer = a.sprite.layers[0].id;
    DocCommand::OnArtboard {
        id,
        cmd: Command::DrawPixels {
            layer,
            frame,
            pixels: points.iter().map(|&(x, y)| (x, y, ONE_BIT_INK)).collect(),
        },
    }
    .apply(doc)
    .unwrap();
}

fn font_of(doc: &Document, id: ArtboardId) -> &FontInfo {
    doc.artboard(id).unwrap().font.as_ref().unwrap()
}

/// Lo que se ve del glifo de `ch`, como (columna, fila).
fn seen(doc: &Document, id: ArtboardId, ch: char) -> Vec<(u32, u32)> {
    let a = doc.artboard(id).unwrap();
    let font = a.font.as_ref().unwrap();
    font.composed(&a.sprite, font.index_of(ch).unwrap())
        .into_keys()
        .map(|(y, x)| (x, y))
        .collect()
}

#[test]
fn auto_build_accents_adds_the_composites_it_can_and_undoes() {
    let (mut doc, id) = doc_with_accent_parts();
    // La é ya está dibujada: no se toca (ni con base y marca no se podría: no hay `e`).
    add_glyphs(doc.artboard(id).unwrap(), &['é'])
        .unwrap()
        .apply(&mut doc)
        .unwrap();
    draw(&mut doc, id, 'é', &[(0, 0)]);
    let before = doc.clone();
    let (cmd, report) = auto_build_accents(doc.artboard(id).unwrap(), None, false);
    // Con a y ´: á, y nada más (las demás marcas y bases no están).
    assert_eq!(report.built, vec!['á']);
    assert_eq!(report.kept, 1, "é was drawn");
    assert!(report.missing.contains(&'à') && report.missing.contains(&'Á'));
    let undo = cmd.unwrap().apply(&mut doc).unwrap();
    let font = font_of(&doc, id);
    let a = doc.artboard(id).unwrap();
    assert_eq!(font.check(&a.sprite), Ok(()));
    let g = &font.glyphs[font.index_of('á').unwrap()];
    assert_eq!(g.components.len(), 2);
    assert_eq!(g.components[1].offset, (0, 0), "rows 0..2, air at row 2");
    assert!(seen(&doc, id, 'á').contains(&(2, 0)));
    // Otra vez: ya está.
    let (again, report) = auto_build_accents(doc.artboard(id).unwrap(), Some(&['á']), false);
    assert!(again.is_none());
    assert_eq!(report.kept, 1);
    // Con replace se rearma, y una é dibujada pierde sus pixeles (aquí no hay `e`: no se puede).
    let (again, report) = auto_build_accents(doc.artboard(id).unwrap(), Some(&['á', 'é']), true);
    assert_eq!(report.built, vec!['á']);
    assert_eq!(report.missing, vec!['é']);
    assert!(again.is_some());
    undo.apply(&mut doc).unwrap();
    assert_eq!(doc, before);
}

#[test]
fn a_composite_follows_its_parts_and_unlinks_to_its_own_pixels() {
    let (mut doc, id) = doc_with_accent_parts();
    let (cmd, _) = auto_build_accents(doc.artboard(id).unwrap(), Some(&['á']), false);
    cmd.unwrap().apply(&mut doc).unwrap();
    draw(&mut doc, id, 'a', &[(5, 6)]);
    assert!(seen(&doc, id, 'á').contains(&(5, 6)), "live");
    let looked = seen(&doc, id, 'á');
    let before = doc.clone();
    let undo = unlink_glyph(doc.artboard(id).unwrap(), 'á')
        .unwrap()
        .apply(&mut doc)
        .unwrap();
    let font = font_of(&doc, id);
    assert!(!font.glyphs[font.index_of('á').unwrap()].is_composite());
    assert_eq!(seen(&doc, id, 'á'), looked);
    assert!(unlink_glyph(doc.artboard(id).unwrap(), 'a').is_none());
    let unlinked = doc.clone();
    draw(&mut doc, id, 'a', &[(6, 6)]);
    assert!(!seen(&doc, id, 'á').contains(&(6, 6)), "not linked anymore");
    let mut doc = unlinked;
    undo.apply(&mut doc).unwrap();
    assert_eq!(doc, before);
}

#[test]
fn components_are_set_with_undo_and_cycles_refused() {
    let (mut doc, id) = doc_with_accent_parts();
    let before = doc.clone();
    let comps = vec![GlyphComponent {
        ch: 'a',
        offset: (1, 0),
    }];
    let undo = set_components(doc.artboard(id).unwrap(), 'b', comps.clone())
        .unwrap()
        .apply(&mut doc)
        .unwrap();
    assert!(set_components(doc.artboard(id).unwrap(), 'b', comps).is_none());
    let back = vec![GlyphComponent {
        ch: 'b',
        offset: (0, 0),
    }];
    assert!(set_components(doc.artboard(id).unwrap(), 'a', back).is_none());
    let own = vec![GlyphComponent {
        ch: 'c',
        offset: (0, 0),
    }];
    assert!(set_components(doc.artboard(id).unwrap(), 'c', own).is_none());
    undo.apply(&mut doc).unwrap();
    assert_eq!(doc, before);
}

#[test]
fn removing_a_component_glyph_unlinks_its_users_first() {
    let (mut doc, id) = doc_with_accent_parts();
    let (cmd, _) = auto_build_accents(doc.artboard(id).unwrap(), Some(&['á']), false);
    cmd.unwrap().apply(&mut doc).unwrap();
    let looked = seen(&doc, id, 'á');
    let before = doc.clone();
    let undo = remove_glyph(doc.artboard(id).unwrap(), 'a')
        .unwrap()
        .apply(&mut doc)
        .unwrap();
    let font = font_of(&doc, id);
    assert!(font.index_of('a').is_none());
    assert!(!font.glyphs[font.index_of('á').unwrap()].is_composite());
    assert_eq!(seen(&doc, id, 'á'), looked);
    assert_eq!(font.check(&doc.artboard(id).unwrap().sprite), Ok(()));
    undo.apply(&mut doc).unwrap();
    assert_eq!(doc, before);
}

fn build(ch: char, pixels: &[(u32, u32)]) -> GlyphBuild {
    GlyphBuild {
        ch,
        advance: None,
        pixels: pixels.iter().map(|&(x, y)| (x, y, None)).collect(),
    }
}

#[test]
fn filling_glyphs_adds_missing_and_empty_ones_and_replaces_only_if_asked() {
    let (mut doc, id) = doc_with_accent_parts();
    let before = doc.clone();
    let glyphs = vec![
        build('a', &[(7, 9)]),
        build('b', &[(1, 1), (20, 1)]),
        build('ñ', &[(2, 2)]),
        build(' ', &[(0, 0)]),
        build('z', &[(30, 30)]),
    ];
    let (cmd, count) = fill_glyphs(doc.artboard(id).unwrap(), &glyphs, false).unwrap();
    assert_eq!(count, 2, "b and ñ; a is drawn, z falls outside");
    let undo = cmd.apply(&mut doc).unwrap();
    assert_eq!(seen(&doc, id, 'b'), vec![(1, 1)]);
    assert_eq!(seen(&doc, id, 'ñ'), vec![(2, 2)]);
    assert!(!seen(&doc, id, 'a').contains(&(7, 9)));
    let font = font_of(&doc, id);
    assert_eq!(font.check(&doc.artboard(id).unwrap().sprite), Ok(()));
    undo.apply(&mut doc).unwrap();
    assert_eq!(doc, before);
    // Con replace, también la a (y un compuesto deja de serlo).
    let (cmd, _) = auto_build_accents(doc.artboard(id).unwrap(), Some(&['á']), false);
    cmd.unwrap().apply(&mut doc).unwrap();
    let glyphs = vec![build('a', &[(7, 9)]), build('á', &[(3, 3)])];
    let (cmd, count) = fill_glyphs(doc.artboard(id).unwrap(), &glyphs, true).unwrap();
    assert_eq!(count, 2);
    cmd.apply(&mut doc).unwrap();
    assert_eq!(seen(&doc, id, 'a'), vec![(7, 9)]);
    assert_eq!(seen(&doc, id, 'á'), vec![(3, 3)]);
    assert!(fill_glyphs(doc.artboard(id).unwrap(), &[build('q', &[])], true).is_none());
}
