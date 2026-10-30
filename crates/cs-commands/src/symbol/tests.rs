use cs_core::*;

use super::*;
use crate::CmdError;

/// Documento con un artboard de 8×8 en el lienzo (id 1).
fn doc() -> Document {
    Document::with_sprite("host", Sprite::new(8, 8, ColorMode::Rgba))
}

fn add(doc: &mut Document, id: u32, position: (i32, i32), in_library: bool) {
    doc.artboards.push(Artboard {
        id: ArtboardId(id),
        name: format!("a{id}"),
        position,
        sprite: Sprite::new(4, 4, ColorMode::Rgba),
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library,
    });
}

#[test]
fn move_to_library_and_back_with_undo() {
    let mut d = doc();
    add(&mut d, 2, (20, 0), false);
    let cmd = move_to_library(&d, ArtboardId(2)).unwrap();
    let undo = cmd.apply(&mut d).unwrap();
    assert!(d.artboard(ArtboardId(2)).unwrap().in_library);
    undo.apply(&mut d).unwrap();
    assert!(!d.artboard(ArtboardId(2)).unwrap().in_library);
}

#[test]
fn the_last_canvas_artboard_cannot_leave() {
    let mut d = doc();
    add(&mut d, 2, (20, 0), true);
    assert_eq!(move_to_library(&d, ArtboardId(1)), None);
    let forced = DocCommand::SetArtboardInLibrary {
        id: ArtboardId(1),
        in_library: true,
    };
    assert_eq!(forced.apply(&mut d), Err(CmdError::LastArtboard));
    let remove = DocCommand::RemoveArtboard { id: ArtboardId(1) };
    assert_eq!(remove.apply(&mut d), Err(CmdError::LastArtboard));
    // Un símbolo sí se puede borrar.
    DocCommand::RemoveArtboard { id: ArtboardId(2) }
        .apply(&mut d)
        .unwrap();
}

#[test]
fn place_on_canvas_keeps_its_spot_or_goes_to_the_right() {
    let mut d = doc();
    add(&mut d, 2, (40, 0), true);
    let cmd = place_on_canvas(&d, ArtboardId(2)).unwrap();
    cmd.apply(&mut d).unwrap();
    assert_eq!(d.artboard(ArtboardId(2)).unwrap().position, (40, 0));

    let mut d = doc();
    add(&mut d, 2, (2, 2), true); // choca con el host (0,0)-(8,8)
    place_on_canvas(&d, ArtboardId(2))
        .unwrap()
        .apply(&mut d)
        .unwrap();
    assert_eq!(d.artboard(ArtboardId(2)).unwrap().position, (8 + 16, 0));
    assert!(!d.artboard(ArtboardId(2)).unwrap().in_library);
}

const RED: PixelValue = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
const BLUE: PixelValue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));

/// Host de 8×8 con 3 frames de duraciones desiguales. Capa 1 (abajo): un píxel rojo que se
/// mueve (2,3) → (3,3) → (4,4). Capa 2 (arriba): un píxel azul fijo en (6,6).
fn animated() -> Document {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    s.frames[0].duration_ms = 100;
    for (index, ms) in [(1, 50), (2, 200)] {
        crate::Command::AddFrame {
            index,
            frame: Frame { duration_ms: ms },
        }
        .apply(&mut s)
        .unwrap();
    }
    crate::ops::new_layer(&s.clone(), Some(LayerId(1)), LayerKind::Image)
        .apply(&mut s)
        .unwrap();
    for (frame, (x, y)) in [(0, (2, 3)), (1, (3, 3)), (2, (4, 4))] {
        crate::Command::DrawPixels {
            layer: LayerId(1),
            frame,
            pixels: vec![(x, y, RED)],
        }
        .apply(&mut s)
        .unwrap();
    }
    for frame in 0..3 {
        crate::Command::DrawPixels {
            layer: LayerId(2),
            frame,
            pixels: vec![(6, 6, BLUE)],
        }
        .apply(&mut s)
        .unwrap();
    }
    Document::with_sprite("host", s)
}

/// Cada frame del host, materializado y renderizado.
fn rendered(doc: &Document, id: ArtboardId) -> Vec<Vec<Rgba>> {
    let s = &doc.artboard(id).unwrap().sprite;
    (0..s.frames.len())
        .map(|f| cs_render::render_frame(&cs_render::materialize(doc, s, f..f + 1), f))
        .collect()
}

/// Mete `ids` (en ese orden) en un grupo nuevo, el 3, arriba de todo.
fn group(d: &mut Document, ids: &[LayerId]) {
    let s = &mut d.artboards[0].sprite;
    crate::ops::new_layer(&s.clone(), Some(LayerId(2)), LayerKind::Group)
        .apply(s)
        .unwrap();
    for &id in ids {
        crate::ops::move_layer(&s.clone(), id, crate::ops::LayerDrop::Into(LayerId(3)))
            .unwrap()
            .apply(s)
            .unwrap();
    }
}

#[test]
fn converting_layers_looks_the_same_in_both_timings() {
    for timing in [SymbolTiming::Graphic, SymbolTiming::MovieClip] {
        let mut d = animated();
        let before = rendered(&d, ArtboardId(1));
        let source = SymbolSource::Layers(vec![LayerId(1)]);
        let c = convert_to_symbol(&d, ArtboardId(1), &source, "Dot", timing).unwrap();
        let undo = c.command.apply(&mut d).unwrap();
        assert_eq!(rendered(&d, ArtboardId(1)), before, "{timing:?}");

        let symbol = d.artboard(c.symbol).unwrap();
        assert!(symbol.in_library);
        assert_eq!(symbol.name, "Dot");
        // La unión de (2,3), (3,3) y (4,4): 3×2 desde (2,3).
        assert_eq!((symbol.sprite.width, symbol.sprite.height), (3, 2));
        assert_eq!(
            symbol.sprite.frames,
            d.artboard(ArtboardId(1)).unwrap().sprite.frames
        );

        let host = &d.artboard(ArtboardId(1)).unwrap().sprite;
        assert!(
            host.layer(LayerId(1)).is_none(),
            "the layer moved into the symbol"
        );
        let layer = host.layer(c.layer).unwrap();
        assert_eq!(
            (layer.kind, layer.name.as_str()),
            (LayerKind::Objects, "Dot")
        );
        let key = layer.objects[0].track.keys[0];
        assert_eq!(
            (key.frame, key.source, key.position),
            (0, Some(c.symbol), (2, 3))
        );
        assert_eq!(key.timing, timing.timing());
        // La instancia queda donde estaba la capa: debajo de la capa 2.
        assert!(host.layer_index(c.layer) < host.layer_index(LayerId(2)));

        undo.apply(&mut d).unwrap();
        assert_eq!(d, animated(), "one undo restores everything");
    }
}

#[test]
fn groups_keep_their_structure_and_empty_ones_go_away() {
    let mut d = animated();
    // Grupo 3 con la capa 1 adentro.
    group(&mut d, &[LayerId(1)]);
    let original = d.clone();
    let before = rendered(&d, ArtboardId(1));
    // Convertir solo la capa 1: el grupo 3 queda vacío en el host y se borra.
    let c = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![LayerId(1)]),
        "Dot",
        SymbolTiming::Graphic,
    )
    .unwrap();
    let undo = c.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);
    let symbol = &d.artboard(c.symbol).unwrap().sprite;
    assert_eq!(symbol.layer(LayerId(1)).unwrap().parent, Some(LayerId(3)));
    assert_eq!(symbol.layer(LayerId(3)).unwrap().kind, LayerKind::Group);
    // En el host, la instancia ocupa el lugar del grupo, que ya no existe.
    let host = &d.artboard(ArtboardId(1)).unwrap().sprite;
    assert!(host.layer(LayerId(3)).is_none());
    assert_eq!(host.layer(c.layer).unwrap().parent, None);
    assert!(host.is_valid_tree() && symbol.is_valid_tree());

    undo.apply(&mut d).unwrap();
    assert_eq!(d, original, "one undo restores everything");
}

#[test]
fn a_group_that_stays_applies_only_once() {
    let mut d = animated();
    // Grupo 3 a media opacidad con las capas 2 y 1 adentro.
    group(&mut d, &[LayerId(2), LayerId(1)]);
    d.artboards[0].sprite.layer_mut(LayerId(3)).unwrap().opacity = 128;
    let original = d.clone();
    let before = rendered(&d, ArtboardId(1));
    let c = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![LayerId(1)]),
        "Dot",
        SymbolTiming::Graphic,
    )
    .unwrap();
    let undo = c.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);
    // El grupo sigue en el host (le queda la capa 2) y la instancia va adentro; el símbolo no
    // lo copia (si no, su opacidad se aplicaría dos veces).
    let host = &d.artboard(ArtboardId(1)).unwrap().sprite;
    assert_eq!(host.layer(c.layer).unwrap().parent, Some(LayerId(3)));
    assert!(host.is_valid_tree());
    let symbol = &d.artboard(c.symbol).unwrap().sprite;
    assert!(symbol.layer(LayerId(3)).is_none());
    assert_eq!(symbol.layer(LayerId(1)).unwrap().parent, None);

    undo.apply(&mut d).unwrap();
    assert_eq!(d, original, "one undo restores everything");
}

#[test]
fn layer_masks_move_with_the_crop() {
    let mut d = animated();
    // Máscara que tapa la columna x = 3: el rojo del frame 1 no se ve.
    let mut mask = LayerMask::filled(8, 8, 255);
    std::sync::Arc::make_mut(&mut mask.data)[3 * 8 + 3] = 0;
    d.artboards[0].sprite.layer_mut(LayerId(1)).unwrap().mask = Some(mask);
    let before = rendered(&d, ArtboardId(1));
    let c = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![LayerId(1)]),
        "Dot",
        SymbolTiming::Graphic,
    )
    .unwrap();
    c.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);
}

#[test]
fn converting_an_instance_layer_nests_and_looks_the_same() {
    let mut d = animated();
    let first = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![LayerId(1)]),
        "Dot",
        SymbolTiming::Graphic,
    )
    .unwrap();
    first.command.apply(&mut d).unwrap();
    let before = rendered(&d, ArtboardId(1));
    let outer = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![first.layer]),
        "Outer",
        SymbolTiming::Graphic,
    )
    .unwrap();
    outer.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);
    assert_eq!(d.instance_sources(outer.symbol), vec![first.symbol]);
}

#[test]
fn converting_a_layer_with_several_objects_looks_the_same_and_undoes_exactly() {
    let mut d = animated();
    let first = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![LayerId(1)]),
        "Dot",
        SymbolTiming::Graphic,
    )
    .unwrap();
    first.command.apply(&mut d).unwrap();
    // Dos objetos más en la misma capa: uno desde el frame 0 y otro desde el 1, que en el 2 se
    // mueve y se da vuelta.
    let s = &mut d.artboards[0].sprite;
    let (cmd, _) = crate::ops::add_object(s, first.layer, 0, first.symbol, "B", (0, 5)).unwrap();
    cmd.apply(s).unwrap();
    let (cmd, c) = crate::ops::add_object(s, first.layer, 1, first.symbol, "C", (4, 0)).unwrap();
    cmd.apply(s).unwrap();
    crate::ops::update_object_key(s, first.layer, c, 2, |k| {
        k.position = (5, 1);
        k.flip_h = true;
    })
    .unwrap()
    .apply(s)
    .unwrap();
    assert_eq!(s.layer(first.layer).unwrap().objects.len(), 3);
    let snapshot = d.clone();
    let before = rendered(&d, ArtboardId(1));
    let outer = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![first.layer]),
        "Outer",
        SymbolTiming::Graphic,
    )
    .unwrap();
    let undo = outer.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);
    assert_eq!(d.instance_sources(outer.symbol), vec![first.symbol]);
    // La capa con sus tres objetos entra entera al símbolo.
    let inner = &d.artboard(outer.symbol).unwrap().sprite;
    let moved = inner
        .layers
        .iter()
        .find(|l| l.kind == LayerKind::Objects)
        .unwrap();
    assert_eq!(moved.objects.len(), 3);
    undo.apply(&mut d).unwrap();
    assert_eq!(d, snapshot, "one undo restores everything");
}

#[test]
fn convert_errors() {
    let d = animated();
    let layers = |ids: &[u32]| SymbolSource::Layers(ids.iter().map(|&i| LayerId(i)).collect());
    let run = |d: &Document, src: &SymbolSource| {
        convert_to_symbol(d, ArtboardId(1), src, "S", SymbolTiming::Graphic).err()
    };
    assert_eq!(run(&d, &layers(&[])), Some(ConvertError::NoLayers));
    assert_eq!(run(&d, &layers(&[99])), Some(ConvertError::NoLayers));

    let mut empty = d.clone();
    let s = &mut empty.artboards[0].sprite;
    crate::ops::new_layer(&s.clone(), None, LayerKind::Image)
        .apply(s)
        .unwrap();
    assert_eq!(run(&empty, &layers(&[3])), Some(ConvertError::Empty));

    let mut indexed = d.clone();
    indexed.artboards[0].sprite.color_mode = ColorMode::Indexed {
        transparent_index: 0,
    };
    assert_eq!(run(&indexed, &layers(&[1])), Some(ConvertError::NotRgba));

    let mut tiles = d.clone();
    tiles.artboards[0].sprite.layers[0].kind = LayerKind::Tilemap;
    assert_eq!(run(&tiles, &layers(&[1])), Some(ConvertError::Tilemap));
}

#[test]
fn converting_pixels_cuts_the_selection_in_every_frame() {
    let mut d = animated();
    let before = rendered(&d, ArtboardId(1));
    // La marquesina toma (2,3) y (3,3) pero no (4,4) ni el azul.
    let selection = Mask::from_rect(8, 8, (2, 3), (3, 3));
    let source = SymbolSource::Pixels {
        layers: vec![LayerId(1), LayerId(2)],
        selection,
    };
    let c = convert_to_symbol(&d, ArtboardId(1), &source, "Cut", SymbolTiming::Graphic).unwrap();
    let undo = c.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);

    let host = &d.artboard(ArtboardId(1)).unwrap().sprite;
    assert_eq!(
        host.pixel(LayerId(1), 0, 2, 3),
        PixelValue::Rgba(Rgba::TRANSPARENT)
    );
    assert_eq!(
        host.pixel(LayerId(1), 2, 4, 4),
        RED,
        "outside the marquee stays"
    );
    assert_eq!(host.pixel(LayerId(2), 0, 6, 6), BLUE);

    let symbol = &d.artboard(c.symbol).unwrap().sprite;
    assert_eq!((symbol.width, symbol.height), (2, 1));
    assert_eq!(symbol.pixel(LayerId(1), 0, 0, 0), RED);
    assert_eq!(symbol.pixel(LayerId(1), 1, 1, 0), RED);
    // La capa 2 viaja al símbolo, pero el azul quedó fuera de la marquesina.
    assert_eq!(
        symbol.pixel(LayerId(2), 0, 0, 0),
        PixelValue::Rgba(Rgba::TRANSPARENT)
    );

    undo.apply(&mut d).unwrap();
    assert_eq!(d, animated(), "one undo restores everything");
}

#[test]
fn an_empty_marquee_is_nothing_to_convert() {
    let d = animated();
    let selection = Mask::from_rect(8, 8, (0, 0), (1, 1));
    let source = SymbolSource::Pixels {
        layers: vec![LayerId(1)],
        selection,
    };
    assert_eq!(
        convert_to_symbol(&d, ArtboardId(1), &source, "S", SymbolTiming::Graphic).err(),
        Some(ConvertError::Empty)
    );
}

#[test]
fn cutting_linked_cels_keeps_them_linked() {
    let mut d = animated();
    // Frame 1 de la capa 1 vinculado al frame 0: el rojo en (2,3) en los dos.
    let s = &mut d.artboards[0].sprite;
    let cel = *s.cel(LayerId(1), 0).unwrap();
    crate::Command::SetCel {
        layer: LayerId(1),
        frame: 1,
        cel: Some(cel),
    }
    .apply(s)
    .unwrap();
    let original = d.clone();
    let before = rendered(&d, ArtboardId(1));
    let source = SymbolSource::Pixels {
        layers: vec![LayerId(1)],
        selection: Mask::from_rect(8, 8, (0, 0), (3, 3)),
    };
    let c = convert_to_symbol(&d, ArtboardId(1), &source, "Cut", SymbolTiming::MovieClip).unwrap();
    let undo = c.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);

    let host = &d.artboard(ArtboardId(1)).unwrap().sprite;
    let image = |s: &Sprite, f| s.cel(LayerId(1), f).unwrap().image;
    assert_eq!(image(host, 0), image(host, 1));
    let symbol = &d.artboard(c.symbol).unwrap().sprite;
    assert_eq!(image(symbol, 0), image(symbol, 1));
    assert_eq!(symbol.pixel(LayerId(1), 1, 0, 0), RED);

    undo.apply(&mut d).unwrap();
    assert_eq!(d, original);
}

/// Convierte `ids` (capas enteras o, con `pixels`, todo el lienzo) y devuelve el error.
fn convert_err(d: &Document, ids: &[u32], pixels: bool) -> Option<ConvertError> {
    let layers = ids.iter().map(|&i| LayerId(i)).collect();
    let source = if pixels {
        SymbolSource::Pixels {
            layers,
            selection: Mask::all(8, 8),
        }
    } else {
        SymbolSource::Layers(layers)
    };
    convert_to_symbol(d, ArtboardId(1), &source, "S", SymbolTiming::Graphic).err()
}

#[test]
fn locked_layers_are_rejected_in_pixel_mode() {
    let mut d = animated();
    d.artboards[0].sprite.layer_mut(LayerId(2)).unwrap().locked = true;
    assert_eq!(convert_err(&d, &[1, 2], true), Some(ConvertError::Locked));
    assert_eq!(convert_err(&d, &[2], true), Some(ConvertError::Locked));
    // Las capas enteras sí se pueden llevar.
    assert_eq!(convert_err(&d, &[1, 2], false), None);
}

#[test]
fn layers_from_different_groups_are_rejected() {
    let mut d = animated();
    // Grupo 3 con la capa 1 adentro; la 2 queda afuera.
    group(&mut d, &[LayerId(1)]);
    for pixels in [false, true] {
        assert_eq!(
            convert_err(&d, &[1, 2], pixels),
            Some(ConvertError::Scattered),
            "pixels: {pixels}"
        );
    }
    // El grupo entero y la capa 2 sí comparten padre.
    assert_eq!(convert_err(&d, &[3, 2], false), None);
}

#[test]
fn layers_with_others_in_between_are_rejected() {
    let mut d = animated();
    // Capa 3 arriba de todo, con algo en (5,5): la 2 queda entre la 1 y la 3.
    let s = &mut d.artboards[0].sprite;
    crate::ops::new_layer(&s.clone(), Some(LayerId(2)), LayerKind::Image)
        .apply(s)
        .unwrap();
    crate::Command::DrawPixels {
        layer: LayerId(3),
        frame: 0,
        pixels: vec![(5, 5, RED)],
    }
    .apply(s)
    .unwrap();
    for pixels in [false, true] {
        assert_eq!(
            convert_err(&d, &[1, 3], pixels),
            Some(ConvertError::Scattered),
            "pixels: {pixels}"
        );
        assert_eq!(convert_err(&d, &[2, 3], pixels), None, "pixels: {pixels}");
    }
}

#[test]
fn an_instance_between_cut_layers_is_rejected() {
    let mut d = animated();
    // Una capa de instancia (de un símbolo del 1) entre la 1 y la 2.
    let first = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![LayerId(1)]),
        "Dot",
        SymbolTiming::Graphic,
    )
    .unwrap();
    first.command.apply(&mut d).unwrap();
    let s = &mut d.artboards[0].sprite;
    crate::ops::new_layer(&s.clone(), None, LayerKind::Image)
        .apply(s)
        .unwrap();
    crate::ops::move_layer(
        &s.clone(),
        LayerId(4),
        crate::ops::LayerDrop::Below(first.layer),
    )
    .unwrap()
    .apply(s)
    .unwrap();
    crate::Command::DrawPixels {
        layer: LayerId(4),
        frame: 0,
        pixels: vec![(5, 5, RED)],
    }
    .apply(s)
    .unwrap();
    // Abajo a arriba: 4, instancia, 2. Con marquesina la instancia no se corta y quedaría tapada.
    assert_eq!(
        convert_err(&d, &[4, first.layer.0, 2], true),
        Some(ConvertError::Scattered)
    );
    assert_eq!(convert_err(&d, &[4, first.layer.0, 2], false), None);
}

#[test]
fn splitting_a_clipping_pair_is_rejected() {
    let mut d = animated();
    // La capa 2 recorta sobre la 1.
    d.artboards[0].sprite.layer_mut(LayerId(2)).unwrap().clip = true;
    for pixels in [false, true] {
        for ids in [&[1][..], &[2][..]] {
            assert_eq!(
                convert_err(&d, ids, pixels),
                Some(ConvertError::Clipping),
                "{ids:?}, pixels: {pixels}"
            );
        }
    }
    // Las dos juntas se ven igual.
    let before = rendered(&d, ArtboardId(1));
    let c = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![LayerId(1), LayerId(2)]),
        "Pair",
        SymbolTiming::Graphic,
    )
    .unwrap();
    c.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);
}

#[test]
fn layers_with_blend_modes_are_rejected() {
    let mut d = animated();
    d.artboards[0].sprite.layer_mut(LayerId(2)).unwrap().blend = BlendMode::Multiply;
    for pixels in [false, true] {
        for ids in [&[2][..], &[1, 2][..]] {
            assert_eq!(
                convert_err(&d, ids, pixels),
                Some(ConvertError::BlendMode),
                "{ids:?}, pixels: {pixels}"
            );
            let source = SymbolSource::Layers(ids.iter().map(|&i| LayerId(i)).collect());
            if !pixels {
                assert_eq!(
                    check_convert(&d, ArtboardId(1), &source),
                    Err(ConvertError::BlendMode)
                );
            }
        }
        // La capa 1 (Normal) sola sí.
        assert_eq!(convert_err(&d, &[1], pixels), None, "pixels: {pixels}");
    }
    // Dentro de un grupo convertido también cuenta; el modo del grupo no (no mezcla).
    group(&mut d, &[LayerId(1), LayerId(2)]);
    assert_eq!(convert_err(&d, &[3], false), Some(ConvertError::BlendMode));
    d.artboards[0].sprite.layer_mut(LayerId(2)).unwrap().blend = BlendMode::Normal;
    d.artboards[0].sprite.layer_mut(LayerId(3)).unwrap().blend = BlendMode::Screen;
    assert_eq!(convert_err(&d, &[3], false), None);
}

#[test]
fn check_convert_agrees_with_convert() {
    let d = animated();
    for ids in [&[][..], &[1], &[1, 2], &[99]] {
        for pixels in [false, true] {
            let layers: Vec<LayerId> = ids.iter().map(|&i| LayerId(i)).collect();
            let source = if pixels {
                SymbolSource::Pixels {
                    layers,
                    selection: Mask::from_rect(8, 8, (0, 0), (1, 1)),
                }
            } else {
                SymbolSource::Layers(layers)
            };
            let full = convert_to_symbol(&d, ArtboardId(1), &source, "S", SymbolTiming::Graphic);
            assert_eq!(
                check_convert(&d, ArtboardId(1), &source),
                full.map(|_| ()),
                "{ids:?}, pixels: {pixels}"
            );
        }
    }
}
