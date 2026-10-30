use std::sync::Arc;

use cs_core::*;

use crate::{CmdError, Command, LayerProps};

fn dot(layer: u32) -> Command {
    Command::DrawPixels {
        layer: LayerId(layer),
        frame: 0,
        pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 2, 3)))],
    }
}

fn add_tileset(s: &mut Sprite, index: usize, name: &str) -> Command {
    Command::SetTileset {
        index,
        tileset: Some(Tileset::new(name, (4, 4), ColorMode::Rgba)),
    }
    .apply(s)
    .unwrap()
}

#[test]
fn set_tileset_adds_replaces_and_removes_with_undo() {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    let empty = s.clone();
    let inv = add_tileset(&mut s, 0, "a");
    assert_eq!(s.tilesets.len(), 1);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, empty);
    add_tileset(&mut s, 0, "a");
    add_tileset(&mut s, 1, "b");
    let two = s.clone();
    let inv = add_tileset(&mut s, 0, "c");
    assert_eq!(s.tilesets[0].name, "c");
    inv.apply(&mut s).unwrap();
    assert_eq!(s, two);
    assert_eq!(
        Command::SetTileset {
            index: 5,
            tileset: Some(Tileset::new("x", (4, 4), ColorMode::Rgba)),
        }
        .apply(&mut s),
        Err(CmdError::IndexOutOfRange(5))
    );
    // Quitar: solo el último y solo si ninguna capa lo usa.
    assert_eq!(
        Command::SetTileset {
            index: 0,
            tileset: None
        }
        .apply(&mut s),
        Err(CmdError::IndexOutOfRange(0))
    );
    s.layers[0].kind = LayerKind::Tilemap;
    s.layers[0].tileset = Some(1);
    assert_eq!(
        Command::SetTileset {
            index: 1,
            tileset: None
        }
        .apply(&mut s),
        Err(CmdError::TilesetInUse(1))
    );
    s.layers[0].tileset = Some(0);
    let before = s.clone();
    let inv = Command::SetTileset {
        index: 1,
        tileset: None,
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(s.tilesets.len(), 1);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn set_tilemap_roundtrip() {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    let before = s.clone();
    let grid = Arc::new(TileGrid::new(2, 2));
    let inv = Command::SetTilemap {
        id: ImageId(3),
        grid: Some(grid.clone()),
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(s.tilemap(ImageId(3)), Some(&grid));
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn removing_a_layer_frees_unused_images_and_grids() {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    dot(1).apply(&mut s).unwrap();
    let mut layer = Layer::new_image(LayerId(2), "map");
    layer.kind = LayerKind::Tilemap;
    layer.tileset = Some(0);
    s.layers.push(layer);
    s.tilemaps.insert(ImageId(5), Arc::new(TileGrid::new(1, 1)));
    s.cels.insert(
        (LayerId(2), 0),
        Cel {
            position: (0, 0),
            opacity: 255,
            image: ImageId(5),
        },
    );
    let before = s.clone();
    let inv = Command::RemoveLayer { id: LayerId(2) }
        .apply(&mut s)
        .unwrap();
    assert!(s.tilemaps.is_empty());
    assert_eq!(s.images.len(), 1, "the other layer's image stays");
    let inv1 = Command::RemoveLayer { id: LayerId(1) }
        .apply(&mut s)
        .unwrap();
    assert!(s.images.is_empty());
    inv1.apply(&mut s).unwrap();
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn resize_moves_tilemap_cels_and_trim_ignores_them() {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    s.layers[0].kind = LayerKind::Tilemap;
    s.tilemaps.insert(ImageId(1), Arc::new(TileGrid::new(4, 4)));
    s.cels.insert(
        (LayerId(1), 0),
        Cel {
            position: (0, 0),
            opacity: 255,
            image: ImageId(1),
        },
    );
    Command::ResizeCanvas {
        width: 4,
        height: 4,
        offset: (-2, -2),
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(s.cel(LayerId(1), 0).unwrap().position, (-2, -2));
    let before = s.clone();
    Command::TrimToCanvas.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn add_and_remove_layer_roundtrip() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    let id = s.next_layer_id();
    let inv = Command::AddLayer {
        index: 1,
        layer: Layer::new_image(id, "Layer 2"),
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(s.layers[1].id, id);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn remove_layer_restores_cels_on_undo() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    dot(1).apply(&mut s).unwrap();
    let before = s.clone();
    let inv = Command::RemoveLayer { id: LayerId(1) }
        .apply(&mut s)
        .unwrap();
    assert!(s.layers.is_empty() && s.cels.is_empty());
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn frames_shift_cels() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    dot(1).apply(&mut s).unwrap();
    let before = s.clone();
    let inv = Command::AddFrame {
        index: 0,
        frame: Frame { duration_ms: 50 },
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(s.frames.len(), 2);
    assert!(s.cel(LayerId(1), 0).is_none() && s.cel(LayerId(1), 1).is_some());
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert_eq!(
        Command::RemoveFrame { index: 0 }.apply(&mut s),
        Err(CmdError::LastFrame)
    );
}

#[test]
fn set_layer_props_roundtrip() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    let inv = Command::SetLayerProps {
        id: LayerId(1),
        props: LayerProps {
            visible: Some(false),
            opacity: Some(10),
            ..Default::default()
        },
    }
    .apply(&mut s)
    .unwrap();
    assert!(!s.layers[0].visible);
    assert_eq!(s.layers[0].opacity, 10);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn slices_follow_inserted_and_removed_frames() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    for i in 1..3 {
        Command::AddFrame {
            index: i,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
    }
    let key = |frame| cs_core::SliceKey {
        frame,
        bounds: (0, 0, 2, 2),
        center: None,
        pivot: Some((1, 1)),
    };
    let slices = vec![Slice {
        name: "hit".into(),
        color: Rgba::rgb(0, 0, 255),
        text: String::new(),
        keys: vec![key(0), key(2)],
    }];
    Command::SetSlices(slices).apply(&mut s).unwrap();
    let before = s.clone();
    let inv = Command::AddFrame {
        index: 1,
        frame: Frame { duration_ms: 100 },
    }
    .apply(&mut s)
    .unwrap();
    let frames = |s: &Sprite| s.slices[0].keys.iter().map(|k| k.frame).collect::<Vec<_>>();
    assert_eq!(frames(&s), [0, 3]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    let inv = Command::RemoveFrame { index: 1 }.apply(&mut s).unwrap();
    assert_eq!(frames(&s), [0, 1]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn masks_set_draw_resize_roundtrip() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    assert_eq!(
        Command::DrawMask {
            layer: LayerId(1),
            pixels: vec![(0, 0, 0)],
        }
        .apply(&mut s),
        Err(CmdError::NoMask(LayerId(1)))
    );
    let inv_set = Command::SetLayerMask {
        id: LayerId(1),
        mask: Some(LayerMask::filled(4, 4, 255)),
    }
    .apply(&mut s)
    .unwrap();
    let with_mask = s.clone();
    let inv_draw = Command::DrawMask {
        layer: LayerId(1),
        pixels: vec![(1, 1, 0), (6, 1, 40)],
    }
    .apply(&mut s)
    .unwrap();
    let mask = s.layers[0].mask.as_ref().unwrap();
    assert_eq!(
        (mask.get(1, 1), mask.get(6, 1), mask.get(5, 1)),
        (0, 40, 255)
    );
    let drawn = s.clone();
    let inv_resize = Command::ResizeCanvas {
        width: 6,
        height: 6,
        offset: (2, 1),
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(s.layers[0].mask.as_ref().unwrap().get(3, 2), 0);
    inv_resize.apply(&mut s).unwrap();
    assert_eq!(s, drawn);
    inv_draw.apply(&mut s).unwrap();
    assert_eq!(s, with_mask);
    inv_set.apply(&mut s).unwrap();
    assert_eq!(s, before);
    let inv_clip = Command::SetLayerProps {
        id: LayerId(1),
        props: LayerProps {
            clip: Some(true),
            ..Default::default()
        },
    }
    .apply(&mut s)
    .unwrap();
    assert!(s.layers[0].clip);
    inv_clip.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn failing_batch_rolls_back() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    let r = Command::Batch(vec![
        Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        },
        Command::RemoveLayer { id: LayerId(99) },
    ])
    .apply(&mut s);
    assert_eq!(r, Err(CmdError::LayerNotFound(LayerId(99))));
    assert_eq!(s, before);
}

#[test]
fn resize_canvas_shifts_cels_and_undoes() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    dot(1).apply(&mut s).unwrap();
    let before = s.clone();
    let inv = Command::ResizeCanvas {
        width: 6,
        height: 5,
        offset: (2, 1),
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!((s.width, s.height), (6, 5));
    assert_eq!(s.cel(LayerId(1), 0).unwrap().position, (2, 1));
    assert_eq!(
        s.pixel(LayerId(1), 0, 2, 1),
        PixelValue::Rgba(Rgba::rgb(1, 2, 3))
    );
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert_eq!(
        Command::ResizeCanvas {
            width: 0,
            height: 3,
            offset: (0, 0)
        }
        .apply(&mut s),
        Err(CmdError::InvalidSize(0, 3))
    );
}

#[test]
fn trim_crops_partial_cels_and_drops_outside_ones() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let c = PixelValue::Rgba(Rgba::rgb(5, 5, 5));
    Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(0, 0, c), (3, 3, c)],
    }
    .apply(&mut s)
    .unwrap();
    // Achica el lienzo a 2×2 dejando el cel de 4×4 asomando.
    Command::ResizeCanvas {
        width: 2,
        height: 2,
        offset: (-1, -1),
    }
    .apply(&mut s)
    .unwrap();
    let before = s.clone();
    let inv = Command::TrimToCanvas.apply(&mut s).unwrap();
    let cel = *s.cel(LayerId(1), 0).unwrap();
    assert_eq!(cel.position, (0, 0));
    let img = s.image(cel.image).unwrap();
    assert_eq!((img.width(), img.height()), (2, 2));
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);

    // Un cel completamente afuera desaparece.
    Command::ResizeCanvas {
        width: 2,
        height: 2,
        offset: (-10, 0),
    }
    .apply(&mut s)
    .unwrap();
    let before = s.clone();
    let inv = Command::TrimToCanvas.apply(&mut s).unwrap();
    assert!(s.cels.is_empty() && s.images.is_empty());
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn set_grid_roundtrip_and_rejects_zero() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    let grid = GridSettings {
        size: (8, 4),
        offset: (1, -2),
    };
    let inv = Command::SetGrid(grid).apply(&mut s).unwrap();
    assert_eq!(s.grid, grid);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert!(
        Command::SetGrid(GridSettings {
            size: (0, 4),
            offset: (0, 0)
        })
        .apply(&mut s)
        .is_err()
    );
}

#[test]
fn commands_serialize() {
    let cmds = [
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Index(2))],
        },
        Command::SetTileset {
            index: 0,
            tileset: Some(Tileset::new("t", (8, 8), ColorMode::Rgba)),
        },
        Command::SetTilemap {
            id: ImageId(4),
            grid: Some(Arc::new(TileGrid::new(2, 1))),
        },
        Command::SetObjects {
            layer: LayerId(2),
            objects: vec![object(1, vec![InstanceKey::new(0, ArtboardId(2), (1, 2))])],
        },
    ];
    for c in cmds {
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), c);
    }
}

fn object(id: u32, keys: Vec<InstanceKey>) -> SceneObject {
    SceneObject {
        id: ObjectId(id),
        name: format!("o{id}"),
        track: InstanceTrack { keys },
    }
}

fn with_instance() -> Sprite {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    for i in 1..3 {
        Command::AddFrame {
            index: i,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
    }
    let mut layer = Layer::new_image(LayerId(2), "inst");
    layer.kind = LayerKind::Objects;
    layer.objects = vec![object(
        1,
        vec![
            InstanceKey::new(0, ArtboardId(2), (0, 0)),
            InstanceKey::new(2, ArtboardId(2), (4, 4)),
        ],
    )];
    s.layers.push(layer);
    s
}

fn key_frames(s: &Sprite) -> Vec<usize> {
    let track = &s.layer(LayerId(2)).unwrap().objects[0].track;
    track.keys.iter().map(|k| k.frame).collect()
}

#[test]
fn set_objects_roundtrip_and_rejects_image_layers() {
    let mut s = with_instance();
    let before = s.clone();
    let objects = vec![
        object(1, vec![InstanceKey::new(1, ArtboardId(3), (1, 1))]),
        object(5, vec![InstanceKey::new(0, ArtboardId(2), (2, 2))]),
    ];
    let inv = Command::SetObjects {
        layer: LayerId(2),
        objects: objects.clone(),
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(s.layer(LayerId(2)).unwrap().objects, objects);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert_eq!(
        Command::SetObjects {
            layer: LayerId(1),
            objects: Vec::new(),
        }
        .apply(&mut s),
        Err(CmdError::NotObjectLayer(LayerId(1)))
    );
}

#[test]
fn set_objects_rejects_repeated_ids() {
    let mut s = with_instance();
    let before = s.clone();
    let keys = vec![InstanceKey::new(0, ArtboardId(2), (0, 0))];
    assert_eq!(
        Command::SetObjects {
            layer: LayerId(2),
            objects: vec![object(3, keys.clone()), object(3, keys)],
        }
        .apply(&mut s),
        Err(CmdError::DuplicateObject(ObjectId(3)))
    );
    assert_eq!(s, before);
}

#[test]
fn frame_changes_and_resize_move_the_keys_of_every_object() {
    let mut s = with_instance();
    s.layer_mut(LayerId(2))
        .unwrap()
        .objects
        .push(object(2, vec![InstanceKey::new(1, ArtboardId(3), (1, 1))]));
    let before = s.clone();
    let frames = |s: &Sprite| -> Vec<Vec<usize>> {
        s.layer(LayerId(2))
            .unwrap()
            .objects
            .iter()
            .map(|o| o.track.keys.iter().map(|k| k.frame).collect())
            .collect()
    };
    let inv = Command::AddFrame {
        index: 1,
        frame: Frame { duration_ms: 100 },
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(frames(&s), vec![vec![0, 3], vec![2]]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    let inv = Command::ResizeCanvas {
        width: 10,
        height: 6,
        offset: (1, 2),
    }
    .apply(&mut s)
    .unwrap();
    let second = &s.layer(LayerId(2)).unwrap().objects[1].track;
    assert_eq!(second.keys[0].position, (2, 3));
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn set_objects_normalizes_keys() {
    let mut s = with_instance();
    let before = s.clone();
    let inv = Command::SetObjects {
        layer: LayerId(2),
        objects: vec![object(
            1,
            vec![
                InstanceKey::new(2, ArtboardId(2), (0, 0)),
                InstanceKey::new(0, ArtboardId(2), (1, 1)),
                InstanceKey::new(2, ArtboardId(2), (5, 5)),
            ],
        )],
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(key_frames(&s), vec![0, 2]);
    let track = &s.layer(LayerId(2)).unwrap().objects[0].track;
    assert_eq!(track.keys[1].position, (5, 5), "last wins");
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

fn set_keys(s: &mut Sprite, keys: Vec<InstanceKey>) {
    s.layer_mut(LayerId(2)).unwrap().objects = vec![object(1, keys)];
}

#[test]
fn removing_the_last_frame_moves_its_key_to_the_new_last() {
    let mut s = with_instance();
    set_keys(&mut s, vec![InstanceKey::new(2, ArtboardId(2), (4, 4))]);
    let before = s.clone();
    let inv = Command::RemoveFrame { index: 2 }.apply(&mut s).unwrap();
    assert_eq!(key_frames(&s), vec![1]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn removing_the_last_frame_keeps_an_existing_key_on_the_new_last() {
    let mut s = with_instance();
    set_keys(
        &mut s,
        vec![
            InstanceKey::new(1, ArtboardId(2), (1, 1)),
            InstanceKey::new(2, ArtboardId(2), (2, 2)),
        ],
    );
    let before = s.clone();
    let inv = Command::RemoveFrame { index: 2 }.apply(&mut s).unwrap();
    let track = &s.layer(LayerId(2)).unwrap().objects[0].track;
    let got: Vec<_> = track.keys.iter().map(|k| (k.frame, k.position)).collect();
    assert_eq!(got, vec![(1, (1, 1))], "frame 1 keeps its own key");
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn removing_a_frame_between_two_keys_keeps_the_later_one() {
    let mut s = with_instance();
    set_keys(
        &mut s,
        vec![
            InstanceKey::new(1, ArtboardId(2), (1, 1)),
            InstanceKey::new(2, ArtboardId(2), (2, 2)),
        ],
    );
    let before = s.clone();
    let inv = Command::RemoveFrame { index: 1 }.apply(&mut s).unwrap();
    let track = &s.layer(LayerId(2)).unwrap().objects[0].track;
    let got: Vec<_> = track.keys.iter().map(|k| (k.frame, k.position)).collect();
    assert_eq!(got, vec![(1, (2, 2))]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn inserting_frame_zero_keeps_the_frame_zero_key() {
    let mut s = with_instance();
    let before = s.clone();
    let inv = Command::AddFrame {
        index: 0,
        frame: Frame { duration_ms: 100 },
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(key_frames(&s), vec![0, 3]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn instance_keys_follow_inserted_and_removed_frames() {
    let mut s = with_instance();
    let before = s.clone();
    let inv = Command::AddFrame {
        index: 1,
        frame: Frame { duration_ms: 100 },
    }
    .apply(&mut s)
    .unwrap();
    assert_eq!(key_frames(&s), vec![0, 3]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    let inv = Command::RemoveFrame { index: 1 }.apply(&mut s).unwrap();
    assert_eq!(key_frames(&s), vec![0, 1]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn resize_canvas_moves_instance_keys_and_undoes() {
    let mut s = with_instance();
    let before = s.clone();
    let inv = Command::ResizeCanvas {
        width: 10,
        height: 6,
        offset: (3, -2),
    }
    .apply(&mut s)
    .unwrap();
    let track = &s.layer(LayerId(2)).unwrap().objects[0].track;
    let positions: Vec<_> = track.keys.iter().map(|k| k.position).collect();
    assert_eq!(positions, vec![(3, -2), (7, 2)]);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}
