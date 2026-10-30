use std::sync::Arc;

use cs_core::*;

use super::*;

/// Sprite de 16×16 con un tileset de 4×4 (dos tiles además del vacío) y una capa tilemap
/// (id 2) encima de la capa 1.
fn with_tilemap_layer() -> Sprite {
    let mut s = Sprite::new(16, 16, ColorMode::Rgba);
    let mut ts = Tileset::new("t", (4, 4), ColorMode::Rgba);
    for _ in 0..2 {
        ts.tiles.push(Arc::new(Image::new(4, 4, ColorMode::Rgba)));
    }
    crate::Command::SetTileset {
        index: 0,
        tileset: Some(ts),
    }
    .apply(&mut s)
    .unwrap();
    new_tilemap_layer(&s.clone(), Some(LayerId(1)), 0)
        .apply(&mut s)
        .unwrap();
    s
}

fn grid_of(s: &Sprite, layer: u32, frame: usize) -> (Cel, TileGrid) {
    let cel = *s.cel(LayerId(layer), frame).unwrap();
    (cel, TileGrid::clone(s.tilemap(cel.image).unwrap()))
}

#[test]
fn new_tilemap_layer_uses_the_tileset() {
    let s = with_tilemap_layer();
    let layer = s.layer(LayerId(2)).unwrap();
    assert_eq!(
        (layer.kind, layer.tileset, layer.name.as_str()),
        (LayerKind::Tilemap, Some(0), "Tilemap 2")
    );
}

#[test]
fn set_tiles_creates_grows_and_unlinks() {
    let mut s = with_tilemap_layer();
    let t = |i| Tile::new(i, false, false, false);
    // Sin cel: se crea en (0, 0) con la grilla mínima que llega a la celda (2, 1).
    set_tiles(&s, LayerId(2), 0, &[(2, 1, t(1))])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    let (cel, grid) = grid_of(&s, 2, 0);
    assert_eq!((cel.position, grid.width, grid.height), ((0, 0), 3, 2));
    assert_eq!(grid.get(2, 1), Some(t(1)));
    // Hacia la izquierda y hacia arriba: la grilla crece y el cel se corre.
    let before = s.clone();
    let inv = set_tiles(&s, LayerId(2), 0, &[(-1, -2, t(2))])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    let (cel, grid) = grid_of(&s, 2, 0);
    assert_eq!((cel.position, grid.width, grid.height), ((-4, -8), 4, 4));
    assert_eq!(grid.get(0, 0), Some(t(2)));
    assert_eq!(
        grid.get(3, 3),
        Some(t(1)),
        "the old tile moved with the grid"
    );
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    // Si nada cambia, no hay comando.
    assert!(set_tiles(&s, LayerId(2), 0, &[(2, 1, t(1))]).is_none());
    // Vinculado: solo el cel editado recibe una grilla nueva.
    crate::Command::AddFrame {
        index: 1,
        frame: Frame { duration_ms: 100 },
    }
    .apply(&mut s)
    .unwrap();
    link_cel(&s, LayerId(2), 1, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    set_tiles(&s, LayerId(2), 1, &[(0, 0, t(2))])
        .unwrap()
        .apply(&mut s)
        .unwrap();
    let ((c0, g0), (c1, g1)) = (grid_of(&s, 2, 0), grid_of(&s, 2, 1));
    assert_ne!(c0.image, c1.image);
    assert_eq!(
        (g0.get(0, 0), g1.get(0, 0)),
        (Some(Tile::EMPTY), Some(t(2)))
    );
    // En una capa de imagen, o sin celdas, no hace nada.
    assert!(set_tiles(&s, LayerId(1), 0, &[(0, 0, t(1))]).is_none());
    assert!(set_tiles(&s, LayerId(2), 0, &[]).is_none());
}

#[test]
fn set_tiles_rejects_huge_bounds() {
    let s = with_tilemap_layer();
    let t = |i| Tile::new(i, false, false, false);
    // Una celda muy lejana (100_000, 100_000) haría una grilla gigantesca: rechazada.
    assert!(set_tiles(&s, LayerId(2), 0, &[(100_000, 100_000, t(1))]).is_none());
    // i32::MAX en una coordenada causa overflow en lógica de bounds: rechazada.
    assert!(set_tiles(&s, LayerId(2), 0, &[(i32::MAX, 0, t(1))]).is_none());
    assert!(set_tiles(&s, LayerId(2), 0, &[(0, i32::MAX, t(1))]).is_none());
    // Una llamada normal sigue funcionando.
    assert!(set_tiles(&s, LayerId(2), 0, &[(1, 1, t(1))]).is_some());
}

#[test]
fn set_tiles_caps_the_grid_pixel_area() {
    let s = with_tilemap_layer();
    let t = |i| Tile::new(i, false, false, false);
    // Tiles de 4×4: 4096×4096 celdas entran en el lado máximo pero son 16384² píxeles, más
    // que MAX_TILEMAP_PIXELS (el lector descartaría ese cel).
    assert!(set_tiles(&s, LayerId(2), 0, &[(0, 0, t(1)), (4095, 4095, t(1))]).is_none());
    // 2048×2048 celdas = 8192² = justo el límite.
    assert_eq!(8192u64 * 8192, MAX_TILEMAP_PIXELS);
    assert!(set_tiles(&s, LayerId(2), 0, &[(0, 0, t(1)), (2047, 2047, t(1))]).is_some());
    assert!(set_tiles(&s, LayerId(2), 0, &[(0, 0, t(1)), (2047, 2048, t(1))]).is_none());
}

#[test]
fn set_tile_image_adds_and_replaces_but_not_tile_zero() {
    let mut s = with_tilemap_layer();
    let mut img = Image::new(4, 4, ColorMode::Rgba);
    img.set(1, 1, PixelValue::Rgba(Rgba::rgb(9, 9, 9))).unwrap();
    let before = s.clone();
    let inv = set_tile_image(&s, 0, 1, img.clone())
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(*s.tilesets[0].tiles[1], img);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    set_tile_image(&s, 0, 3, img.clone())
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(s.tilesets[0].tiles.len(), 4, "index == len adds a tile");
    assert!(
        set_tile_image(&s, 0, 0, img.clone()).is_none(),
        "tile 0 is the empty tile"
    );
    assert!(
        set_tile_image(&s, 0, 9, img.clone()).is_none(),
        "out of range"
    );
    assert!(
        set_tile_image(&s, 1, 1, img.clone()).is_none(),
        "no tileset 1"
    );
    assert!(
        set_tile_image(&s, 0, 3, img).is_none(),
        "same image: nothing changes"
    );
    let small = Image::new(2, 2, ColorMode::Rgba);
    assert!(set_tile_image(&s, 0, 1, small).is_none(), "wrong size");
}

#[test]
fn duplicating_and_unlinking_tilemap_cels_copies_the_grid() {
    let mut s = with_tilemap_layer();
    set_tiles(
        &s,
        LayerId(2),
        0,
        &[(0, 0, Tile::new(1, false, false, false))],
    )
    .unwrap()
    .apply(&mut s)
    .unwrap();
    duplicate_frame(&s.clone(), 0, false)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    let ((c0, g0), (c1, g1)) = (grid_of(&s, 2, 0), grid_of(&s, 2, 1));
    assert_ne!(c0.image, c1.image);
    assert_eq!(g0, g1);
    link_cel(&s, LayerId(2), 1, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    unlink_cel(&s, LayerId(2), 1)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    let cel = *s.cel(LayerId(2), 1).unwrap();
    assert_ne!(cel.image, c0.image);
    assert!(s.tilemap(cel.image).is_some());
}

fn dot(s: &mut Sprite, layer: u32, frame: usize) {
    Command::DrawPixels {
        layer: LayerId(layer),
        frame,
        pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 2, 3)))],
    }
    .apply(s)
    .unwrap();
}

/// Abajo → arriba: L1, G2 { L3 }, L4.
fn tree() -> Sprite {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    new_layer(&s.clone(), Some(LayerId(1)), LayerKind::Group)
        .apply(&mut s)
        .unwrap();
    let mut child = Layer::new_image(LayerId(3), "c");
    child.parent = Some(LayerId(2));
    Command::AddLayer {
        index: 2,
        layer: child,
    }
    .apply(&mut s)
    .unwrap();
    new_layer(&s.clone(), None, LayerKind::Image)
        .apply(&mut s)
        .unwrap();
    s
}

fn order(s: &Sprite) -> Vec<(u32, Option<u32>)> {
    s.layers
        .iter()
        .map(|l| (l.id.0, l.parent.map(|p| p.0)))
        .collect()
}

#[test]
fn object_layer_keys_update_and_remove() {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    for i in 1..4 {
        crate::Command::AddFrame {
            index: i,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
    }
    let cmd = new_object_layer(&s, Some(LayerId(1)), 1, ArtboardId(2), "Hero", (3, 4));
    cmd.apply(&mut s).unwrap();
    let layer = s.layers.last().unwrap().clone();
    assert_eq!(
        (layer.kind, layer.name.as_str()),
        (LayerKind::Objects, "Hero")
    );
    assert_eq!(
        (
            layer.objects.len(),
            layer.objects[0].id,
            layer.objects[0].name.as_str()
        ),
        (1, ObjectId(1), "Hero")
    );
    let id = layer.id;
    let o = ObjectId(1);
    let track = |s: &Sprite| s.layer(id).unwrap().objects[0].track.clone();
    assert_eq!(
        track(&s).keys,
        vec![InstanceKey::new(1, ArtboardId(2), (3, 4))]
    );
    // Editar en un frame sin key crea una copia de la vigente ahí.
    update_object_key(&s, id, o, 3, |k| k.flip_h = true)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    let t = track(&s);
    assert_eq!(t.keys.len(), 2);
    assert!(!t.key_at(2).unwrap().flip_h && t.key_at(3).unwrap().flip_h);
    assert_eq!(t.key_at(3).unwrap().position, (3, 4));
    // Sin cambios, no hay comando; antes de la primera key, tampoco.
    assert!(update_object_key(&s, id, o, 3, |_| {}).is_none());
    assert!(update_object_key(&s, id, o, 0, |k| k.opacity = 1).is_none());
    // Borrar una key; borrar la única quita la capa.
    remove_object_key(&s, id, o, 3)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(track(&s).keys.len(), 1);
    assert!(remove_object_key(&s, id, o, 2).is_none(), "no key there");
    remove_object_key(&s, id, o, 1)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(s.layer(id).is_none());
}

#[test]
fn removing_the_only_key_of_one_of_several_objects_removes_just_that_object() {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    new_object_layer(&s, None, 0, ArtboardId(2), "Hero", (0, 0))
        .apply(&mut s)
        .unwrap();
    let id = s.layers.last().unwrap().id;
    let mut second = s.layer(id).unwrap().objects[0].clone();
    second.id = ObjectId(2);
    s.layer_mut(id).unwrap().objects.push(second);
    remove_object_key(&s, id, ObjectId(1), 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    let ids: Vec<_> = s.layer(id).unwrap().objects.iter().map(|o| o.id).collect();
    assert_eq!(ids, vec![ObjectId(2)]);
    assert!(update_object_key(&s, id, ObjectId(1), 0, |k| k.flip_h = true).is_none());
    // Se edita la key del objeto pedido.
    update_object_key(&s, id, ObjectId(2), 0, |k| k.flip_h = true)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(s.layer(id).unwrap().objects[0].track.keys[0].flip_h);
}

#[test]
fn new_layer_goes_above_target_in_same_group() {
    let mut s = tree();
    new_layer(&s.clone(), Some(LayerId(3)), LayerKind::Image)
        .apply(&mut s)
        .unwrap();
    assert_eq!(
        order(&s),
        vec![(1, None), (2, None), (3, Some(2)), (5, Some(2)), (4, None)]
    );
    assert!(s.is_valid_tree());
}

#[test]
fn moving_group_moves_children_and_undo_restores() {
    let mut s = tree();
    let before = s.clone();
    let inv = move_layer(&s, LayerId(2), LayerDrop::Above(LayerId(4)))
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(
        order(&s),
        vec![(1, None), (4, None), (2, None), (3, Some(2))]
    );
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn move_into_and_out_of_group() {
    let mut s = tree();
    move_layer(&s, LayerId(4), LayerDrop::Into(LayerId(2)))
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(
        order(&s),
        vec![(1, None), (2, None), (3, Some(2)), (4, Some(2))]
    );
    move_layer(&s, LayerId(3), LayerDrop::Below(LayerId(1)))
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(
        order(&s),
        vec![(3, None), (1, None), (2, None), (4, Some(2))]
    );
    assert!(s.is_valid_tree());
}

#[test]
fn invalid_moves_are_rejected() {
    let s = tree();
    // Un grupo no puede entrar en sí mismo ni en sus hijos; una capa normal no es destino "Into".
    assert_eq!(
        move_layer(&s, LayerId(2), LayerDrop::Into(LayerId(2))),
        None
    );
    assert_eq!(
        move_layer(&s, LayerId(2), LayerDrop::Above(LayerId(3))),
        None
    );
    assert_eq!(
        move_layer(&s, LayerId(4), LayerDrop::Into(LayerId(1))),
        None
    );
    let mut s2 = s.clone();
    assert_eq!(
        Command::MoveLayer {
            id: LayerId(4),
            index: 2,
            parent: None
        }
        .apply(&mut s2),
        Err(crate::CmdError::InvalidLayerTree)
    );
    assert_eq!(s2, s);
}

#[test]
fn remove_group_removes_children() {
    let mut s = tree();
    dot(&mut s, 3, 0);
    let before = s.clone();
    let inv = remove_layer_tree(&s, LayerId(2))
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(order(&s), vec![(1, None), (4, None)]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn duplicate_frame_linked_and_unlinked() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    dot(&mut s, 1, 0);
    let mut linked = s.clone();
    duplicate_frame(&linked.clone(), 0, true)
        .unwrap()
        .apply(&mut linked)
        .unwrap();
    assert_eq!(linked.frames.len(), 2);
    assert!(is_linked(&linked, LayerId(1), 1));
    let before = s.clone();
    let inv = duplicate_frame(&s.clone(), 0, false)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(!is_linked(&s, LayerId(1), 1));
    assert_eq!(
        s.pixel(LayerId(1), 1, 0, 0),
        PixelValue::Rgba(Rgba::rgb(1, 2, 3))
    );
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn link_and_unlink() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    Command::AddFrame {
        index: 1,
        frame: Frame { duration_ms: 100 },
    }
    .apply(&mut s)
    .unwrap();
    dot(&mut s, 1, 0);
    link_cel(&s, LayerId(1), 1, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(is_linked(&s, LayerId(1), 0) && is_linked(&s, LayerId(1), 1));
    // Pintar en uno se ve en el otro.
    Command::DrawPixels {
        layer: LayerId(1),
        frame: 1,
        pixels: vec![(2, 2, PixelValue::Rgba(Rgba::rgb(9, 9, 9)))],
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(
        s.pixel(LayerId(1), 0, 2, 2),
        PixelValue::Rgba(Rgba::rgb(9, 9, 9))
    );
    unlink_cel(&s, LayerId(1), 1)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(!is_linked(&s, LayerId(1), 0));
    assert_eq!(unlink_cel(&s, LayerId(1), 1), None);
}

#[test]
fn new_apply_and_select_masks() {
    let mut s = Sprite::new(4, 1, ColorMode::Rgba);
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: (0..4).map(|x| (x, 0, red)).collect(),
    }
    .apply(&mut s)
    .unwrap();
    duplicate_frame(&s, 0, true).unwrap().apply(&mut s).unwrap();
    let sel = Mask::from_rect(4, 1, (1, 0), (2, 0));
    let mask = new_mask(&s, false, Some(&sel));
    assert_eq!(
        (mask.get(0, 0), mask.get(1, 0), mask.get(9, 9)),
        (0, 255, 0)
    );
    assert_eq!(new_mask(&s, true, None).get(0, 0), 0);
    Command::SetLayerMask {
        id: LayerId(1),
        mask: Some(mask),
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(mask_to_selection(&s, LayerId(1)).unwrap(), sel);
    let before = s.clone();
    let inv = apply_mask(&s, LayerId(1)).unwrap().apply(&mut s).unwrap();
    assert!(s.layers[0].mask.is_none());
    assert_eq!(
        s.pixel(LayerId(1), 1, 0, 0),
        PixelValue::Rgba(Rgba::new(255, 0, 0, 0))
    );
    assert_eq!(s.pixel(LayerId(1), 0, 1, 0), red);
    assert!(is_linked(&s, LayerId(1), 1), "links survive");
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert!(apply_mask(&s, LayerId(9)).is_none());
}

#[test]
fn link_and_unlink_ranges() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    for i in 1..4 {
        Command::AddFrame {
            index: i,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
    }
    dot(&mut s, 1, 1); // el primer cel del rango 0..=3 está en el frame 1
    let before = s.clone();
    let inv = link_range(&s, LayerId(1), 3, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    for f in 0..4 {
        assert!(is_linked(&s, LayerId(1), f), "frame {f}");
    }
    assert_eq!(link_range(&s, LayerId(1), 0, 3), None, "already linked");
    let linked = s.clone();
    let inv2 = unlink_range(&s, LayerId(1), 0, 3)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    for f in 0..4 {
        assert!(!is_linked(&s, LayerId(1), f), "frame {f}");
    }
    inv2.apply(&mut s).unwrap();
    assert_eq!(s, linked);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    let empty = Sprite::new(2, 2, ColorMode::Rgba);
    assert_eq!(link_range(&empty, LayerId(1), 0, 0), None);
}

#[test]
fn set_palette_roundtrip() {
    let mut s = Sprite::new(2, 2, ColorMode::Rgba);
    let before = s.clone();
    let mut colors = s.palette.colors().to_vec();
    colors.push(Rgba::rgb(1, 2, 3));
    let inv = Command::SetPalette(Palette::new(colors))
        .apply(&mut s)
        .unwrap();
    assert_eq!(s.palette.len(), 33);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn frames_adjust_tags_and_undo_restores_them() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    for i in 1..4 {
        Command::AddFrame {
            index: i,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
    }
    let tag = |name: &str, from, to| Tag {
        name: name.into(),
        from,
        to,
        direction: AnimDirection::Forward,
        color: Rgba::rgb(1, 1, 1),
        repeat: 0,
    };
    Command::SetTags(vec![tag("walk", 1, 2), tag("solo", 3, 3)])
        .apply(&mut s)
        .unwrap();
    let before = s.clone();
    let inv = Command::AddFrame {
        index: 2,
        frame: Frame { duration_ms: 50 },
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!((s.tags[0].from, s.tags[0].to), (1, 3));
    assert_eq!((s.tags[1].from, s.tags[1].to), (4, 4));
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    let inv = Command::RemoveFrame { index: 3 }.apply(&mut s).unwrap();
    assert_eq!(s.tags.len(), 1, "el tag de un solo frame desaparece");
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn move_frame_and_duration_roundtrip() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    for i in 1..3 {
        Command::AddFrame {
            index: i,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
    }
    dot(&mut s, 1, 0);
    let before = s.clone();
    let inv = Command::MoveFrame { from: 0, to: 2 }.apply(&mut s).unwrap();
    assert!(s.cel(LayerId(1), 2).is_some() && s.cel(LayerId(1), 0).is_none());
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    let inv = Command::SetFrameDuration {
        index: 1,
        duration_ms: 33,
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(s.frames[1].duration_ms, 33);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

fn red() -> PixelValue {
    PixelValue::Rgba(Rgba::rgb(255, 0, 0))
}

/// Lo que se ve del frame (con los tilemaps dibujados).
fn shown(s: &Sprite, frame: usize) -> Vec<Rgba> {
    cs_render::render_frame(&cs_render::materialize_tilemaps(s, frame..frame + 1), frame)
}

/// 16×8 con la capa 1: un píxel rojo en la esquina de las celdas de 4×4 (0, 0) y (2, 0).
fn two_equal_cells() -> Sprite {
    let mut s = Sprite::new(16, 8, ColorMode::Rgba);
    Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(0, 0, red()), (8, 0, red())],
    }
    .apply(&mut s)
    .unwrap();
    s
}

#[test]
fn convert_to_tilemap_dedupes_cells_keeps_the_layer_and_undoes() {
    let mut s = two_equal_cells();
    crate::Command::SetLayerProps {
        id: LayerId(1),
        props: crate::LayerProps {
            name: Some("ground".into()),
            opacity: Some(128),
            blend: Some(BlendMode::Multiply),
            locked: Some(true),
            ..Default::default()
        },
    }
    .apply(&mut s)
    .unwrap();
    let before = s.clone();
    let shown_before = shown(&s, 0);
    let cmd = convert_to_tilemap(&s, LayerId(1), (4, 4)).unwrap();
    let inv = cmd.apply(&mut s).unwrap();
    let (layer, old) = (
        s.layer(LayerId(1)).unwrap(),
        before.layer(LayerId(1)).unwrap(),
    );
    assert_eq!((layer.kind, layer.tileset), (LayerKind::Tilemap, Some(0)));
    assert_eq!(
        (
            &layer.name,
            layer.opacity,
            layer.blend,
            layer.locked,
            layer.visible
        ),
        (&old.name, old.opacity, old.blend, old.locked, old.visible)
    );
    assert_eq!((layer.parent, layer.clip), (old.parent, old.clip));
    assert_eq!(s.layer_index(LayerId(1)), before.layer_index(LayerId(1)));
    let ts = &s.tilesets[0];
    assert_eq!(
        (ts.name.as_str(), ts.tile_size, ts.tiles.len()),
        ("ground", (4, 4), 2),
        "one tile plus the empty tile"
    );
    let (cel, grid) = grid_of(&s, 1, 0);
    assert_eq!(
        (cel.position, cel.opacity, grid.width, grid.height),
        ((0, 0), 255, 4, 2)
    );
    assert_eq!(grid.get(0, 0), Some(Tile::new(1, false, false, false)));
    assert_eq!(grid.get(2, 0), grid.get(0, 0));
    assert_eq!(grid.get(1, 0), Some(Tile::EMPTY));
    assert!(s.images.is_empty(), "the old image left the pool");
    assert_eq!(shown(&s, 0), shown_before);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn convert_to_tilemap_keeps_links_and_handles_unaligned_cels() {
    let mut s = two_equal_cells();
    Command::AddFrame {
        index: 1,
        frame: Frame { duration_ms: 100 },
    }
    .apply(&mut s)
    .unwrap();
    let cel = *s.cel(LayerId(1), 0).unwrap();
    for frame in 0..2 {
        Command::SetCel {
            layer: LayerId(1),
            frame,
            cel: Some(Cel {
                position: (3, 2),
                ..cel
            }),
        }
        .apply(&mut s)
        .unwrap();
    }
    let before = (shown(&s, 0), shown(&s, 1));
    let cmd = convert_to_tilemap(&s, LayerId(1), (4, 4)).unwrap();
    cmd.apply(&mut s).unwrap();
    let (c0, c1) = (
        *s.cel(LayerId(1), 0).unwrap(),
        *s.cel(LayerId(1), 1).unwrap(),
    );
    assert_eq!(c0.image, c1.image, "linked cels stay linked");
    assert!(is_linked(&s, LayerId(1), 1));
    assert_eq!(
        c0.position,
        (0, 0),
        "the grid starts at the corner of its first cell"
    );
    assert_eq!(s.tilesets[0].tiles.len(), 2);
    assert_eq!((shown(&s, 0), shown(&s, 1)), before);
}

#[test]
fn convert_to_tilemap_rejects_bad_sizes_other_layers_and_too_many_tiles() {
    let s = two_equal_cells();
    assert!(convert_to_tilemap(&s, LayerId(1), (0, 4)).is_none());
    assert!(convert_to_tilemap(&s, LayerId(1), (MAX_TILE_SIDE + 1, 4)).is_none());
    assert!(convert_to_tilemap(&s, LayerId(9), (4, 4)).is_none());
    let t = with_tilemap_layer();
    assert!(
        convert_to_tilemap(&t, LayerId(2), (4, 4)).is_none(),
        "already a tilemap"
    );
    // 257×256 píxeles distintos con tiles de 1×1: más de MAX_TILESET_TILES tiles.
    let mut big = Sprite::new(257, 256, ColorMode::Rgba);
    let pixels = (0..256i32)
        .flat_map(|y| {
            (0..257i32).map(move |x| {
                let c = Rgba::rgb(x as u8, y as u8, (x >> 8) as u8);
                (x, y, PixelValue::Rgba(c))
            })
        })
        .collect();
    Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels,
    }
    .apply(&mut big)
    .unwrap();
    assert!(convert_to_tilemap(&big, LayerId(1), (1, 1)).is_none());
    assert!(convert_to_tilemap(&big, LayerId(1), (2, 2)).is_some());
}

#[test]
fn convert_to_tilemap_dedupes_tiles_that_differ_only_in_invisible_pixels() {
    let mut s = Sprite::new(16, 8, ColorMode::Rgba);
    let ghost = |r| PixelValue::Rgba(Rgba::new(r, 0, 0, 0));
    Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![
            (0, 0, red()),
            (1, 0, ghost(7)),
            (8, 0, red()),
            (9, 0, ghost(200)),
        ],
    }
    .apply(&mut s)
    .unwrap();
    let mut c = s.clone();
    convert_to_tilemap(&s, LayerId(1), (8, 8))
        .unwrap()
        .apply(&mut c)
        .unwrap();
    assert_eq!(c.tilesets[0].tiles.len(), 2, "empty + one shared tile");
}

#[test]
fn convert_to_tilemap_refuses_a_cel_with_a_missing_image() {
    let mut s = two_equal_cells();
    let cel = *s.cel(LayerId(1), 0).unwrap();
    s.images.remove(&cel.image);
    assert!(convert_to_tilemap(&s, LayerId(1), (4, 4)).is_none());
}

#[test]
fn convert_to_image_refuses_when_a_cel_would_be_lost() {
    // Grilla de 33×32 tiles de 256×256: más de MAX_TILEMAP_PIXELS al dibujarla.
    let mut s = Sprite::new(16, 16, ColorMode::Rgba);
    Command::SetTileset {
        index: 0,
        tileset: Some(Tileset::new("big", (256, 256), ColorMode::Rgba)),
    }
    .apply(&mut s)
    .unwrap();
    new_tilemap_layer(&s.clone(), Some(LayerId(1)), 0)
        .apply(&mut s)
        .unwrap();
    let id = s.next_image_id();
    Command::SetTilemap {
        id,
        grid: Some(Arc::new(TileGrid::new(33, 32))),
    }
    .apply(&mut s)
    .unwrap();
    let cel = Cel {
        image: id,
        position: (0, 0),
        opacity: 255,
    };
    Command::SetCel {
        layer: LayerId(2),
        frame: 0,
        cel: Some(cel),
    }
    .apply(&mut s)
    .unwrap();
    assert!(convert_to_image(&s, LayerId(2)).is_none(), "over the limit");
    // Una grilla que cabe sí se convierte; si falta del pool, se rechaza.
    let mut ok = s.clone();
    Command::SetTilemap {
        id,
        grid: Some(Arc::new(TileGrid::new(1, 1))),
    }
    .apply(&mut ok)
    .unwrap();
    assert!(convert_to_image(&ok, LayerId(2)).is_some());
    s.tilemaps.remove(&id);
    assert!(convert_to_image(&s, LayerId(2)).is_none(), "missing grid");
}

#[test]
fn convert_to_image_renders_the_same_keeps_off_canvas_tiles_and_undoes() {
    let mut s = with_tilemap_layer();
    let mut img = Image::new(4, 4, ColorMode::Rgba);
    img.set(1, 1, red()).unwrap();
    let cmd = set_tile_image(&s, 0, 1, img).unwrap();
    cmd.apply(&mut s).unwrap();
    let t1 = Tile::new(1, true, false, false);
    let cmd = set_tiles(&s, LayerId(2), 0, &[(-1, 0, t1), (3, 3, t1)]).unwrap();
    cmd.apply(&mut s).unwrap();
    let before = s.clone();
    let shown_before = shown(&s, 0);
    let cmd = convert_to_image(&s, LayerId(2)).unwrap();
    let inv = cmd.apply(&mut s).unwrap();
    let layer = s.layer(LayerId(2)).unwrap();
    assert_eq!((layer.kind, layer.tileset), (LayerKind::Image, None));
    assert_eq!(s.tilesets.len(), 1, "the tileset stays");
    assert!(s.tilemaps.is_empty(), "the grid left the pool");
    let cel = *s.cel(LayerId(2), 0).unwrap();
    let image = s.image(cel.image).unwrap();
    assert_eq!(cel.position, (-4, 0));
    assert_eq!(
        (image.width(), image.height()),
        (20, 16),
        "not cropped to the canvas"
    );
    assert_eq!(
        image.get(2, 1),
        Some(red()),
        "the off-canvas tile is kept, flipped"
    );
    assert_eq!(shown(&s, 0), shown_before);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert!(
        convert_to_image(&s, LayerId(1)).is_none(),
        "image layers stay"
    );
}

/// Capa de objetos con A, B, C, D (ids 1 a 4) en el frame 0.
fn four_objects() -> (Sprite, LayerId) {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    new_object_layer(&s, None, 0, ArtboardId(2), "A", (0, 0))
        .apply(&mut s)
        .unwrap();
    let id = s.layers.last().unwrap().id;
    for name in ["B", "C", "D"] {
        let (cmd, _) = add_object(&s, id, 0, ArtboardId(2), name, (0, 0)).unwrap();
        cmd.apply(&mut s).unwrap();
    }
    (s, id)
}

fn order_of(s: &Sprite, id: LayerId) -> Vec<u32> {
    s.layer(id)
        .unwrap()
        .objects
        .iter()
        .map(|o| o.id.0)
        .collect()
}

fn ids(v: &[u32]) -> Vec<ObjectId> {
    v.iter().map(|&i| ObjectId(i)).collect()
}

#[test]
fn add_object_goes_on_top_and_undoes() {
    let (mut s, id) = four_objects();
    assert_eq!(order_of(&s, id), vec![1, 2, 3, 4]);
    let before = s.clone();
    let (cmd, new) = add_object(&s, id, 0, ArtboardId(3), "E", (1, 2)).unwrap();
    assert_eq!(new, ObjectId(5));
    let undo = cmd.apply(&mut s).unwrap();
    assert_eq!(s.layer(id).unwrap().objects[4].name, "E");
    undo.apply(&mut s).unwrap();
    assert_eq!(s, before);
    // No es capa de objetos.
    assert!(add_object(&s, LayerId(1), 0, ArtboardId(3), "E", (0, 0)).is_none());
}

#[test]
fn remove_objects_can_empty_the_layer() {
    let (mut s, id) = four_objects();
    let before = s.clone();
    let undo = remove_objects(&s, id, &ids(&[2, 3]))
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(order_of(&s, id), vec![1, 4]);
    undo.apply(&mut s).unwrap();
    assert_eq!(s, before);
    remove_objects(&s, id, &ids(&[1, 2, 3, 4]))
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(s.layer(id).unwrap().objects.is_empty());
    assert!(remove_objects(&s, id, &ids(&[1])).is_none());
}

#[test]
fn move_objects_creates_key_and_keeps_the_old_one() {
    let (mut s, id) = four_objects();
    for i in 1..3 {
        crate::Command::AddFrame {
            index: i,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
    }
    let before = s.clone();
    let undo = move_objects(&s, id, &ids(&[1, 3]), 2, 5, -1)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    let l = s.layer(id).unwrap();
    for o in [&l.objects[0], &l.objects[2]] {
        assert_eq!(o.track.keys.len(), 2);
        assert_eq!(o.track.keys[0].position, (0, 0));
        assert_eq!(o.track.keys[1].frame, 2);
        assert_eq!(o.track.keys[1].position, (5, -1));
    }
    assert_eq!(l.objects[1].track.keys.len(), 1);
    undo.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert!(move_objects(&s, id, &ids(&[1]), 2, 0, 0).is_none());
    assert!(move_objects(&s, id, &ids(&[9]), 2, 1, 1).is_none());
}

#[test]
fn reorder_objects_modes_with_non_contiguous_selection() {
    let (s, id) = four_objects();
    let run = |sel: &[u32], to| {
        let mut t = s.clone();
        let cmd = reorder_objects(&t, id, &ids(sel), to)?;
        let undo = cmd.apply(&mut t).unwrap();
        let res = order_of(&t, id);
        undo.apply(&mut t).unwrap();
        assert_eq!(t, s);
        Some(res)
    };
    assert_eq!(run(&[1, 3], Reorder::Forward), Some(vec![2, 1, 4, 3]));
    assert_eq!(run(&[2, 4], Reorder::Forward), Some(vec![1, 3, 2, 4]));
    assert_eq!(run(&[1, 2], Reorder::Forward), Some(vec![3, 1, 2, 4]));
    assert_eq!(run(&[2, 4], Reorder::Backward), Some(vec![2, 1, 4, 3]));
    assert_eq!(run(&[3, 4], Reorder::Backward), Some(vec![1, 3, 4, 2]));
    assert_eq!(run(&[1, 3], Reorder::Front), Some(vec![2, 4, 1, 3]));
    assert_eq!(run(&[2, 4], Reorder::Back), Some(vec![2, 4, 1, 3]));
    // Sin cambios o sin ids válidos.
    assert_eq!(run(&[3, 4], Reorder::Front), None);
    assert_eq!(run(&[1, 2], Reorder::Back), None);
    assert_eq!(run(&[4], Reorder::Forward), None);
    assert_eq!(run(&[9], Reorder::Front), None);
}

#[test]
fn duplicate_objects_get_new_ids_and_offset() {
    let (mut s, id) = four_objects();
    let before = s.clone();
    let (cmd, new) = duplicate_objects(&s, id, &ids(&[3, 1]), (4, 6)).unwrap();
    assert_eq!(new, ids(&[5, 6]));
    let undo = cmd.apply(&mut s).unwrap();
    let l = s.layer(id).unwrap();
    assert_eq!(l.objects.len(), 6);
    assert_eq!(l.objects[4].name, "A copy");
    assert_eq!(l.objects[5].name, "C copy");
    assert_eq!(l.objects[4].track.keys[0].position, (4, 6));
    assert_eq!(l.objects[0].track.keys[0].position, (0, 0));
    undo.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert!(duplicate_objects(&s, id, &ids(&[9]), (1, 1)).is_none());
}
