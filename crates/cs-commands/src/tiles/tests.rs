use std::sync::Arc;

use cs_core::*;

use super::*;

const RED: Rgba = Rgba::rgb(255, 0, 0);
const BLUE: Rgba = Rgba::rgb(0, 0, 255);

fn px(x: i32, y: i32, c: Rgba) -> (i32, i32, PixelValue) {
    (x, y, PixelValue::Rgba(c))
}

/// Sprite de 16×8 con un tileset de 4×4 (tile 1 rojo en (0,0), tile 2 azul en (0,0)) y dos capas
/// tilemap: la 2 con `[1, 1, 2]` en el frame 0 y la 3 con `[2]`.
fn sprite() -> Sprite {
    let mut s = Sprite::new(16, 8, ColorMode::Rgba);
    let mut ts = Tileset::new("t", (4, 4), ColorMode::Rgba);
    for c in [RED, BLUE] {
        let mut img = Image::new(4, 4, ColorMode::Rgba);
        img.set(0, 0, PixelValue::Rgba(c)).unwrap();
        ts.tiles.push(Arc::new(img));
    }
    s.tilesets.push(ts);
    for (id, cells) in [(2u32, vec![1, 1, 2]), (3, vec![2])] {
        let mut layer = Layer::new_image(LayerId(id), "map");
        layer.kind = LayerKind::Tilemap;
        layer.tileset = Some(0);
        s.layers.push(layer);
        let mut grid = TileGrid::new(cells.len() as u32, 1);
        for (x, i) in cells.into_iter().enumerate() {
            grid.set(x as u32, 0, Tile(i));
        }
        s.tilemaps.insert(ImageId(id), Arc::new(grid));
        s.cels.insert(
            (LayerId(id), 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: ImageId(id),
            },
        );
    }
    s
}

fn grid(s: &Sprite, layer: u32, frame: usize) -> Vec<Tile> {
    let cel = s.cel(LayerId(layer), frame).unwrap();
    s.tilemap(cel.image).unwrap().tiles.clone()
}

/// Aplica el comando y comprueba que su inverso deja el sprite exacto.
fn apply_and_check_undo(s: &mut Sprite, cmd: Command) {
    let before = s.clone();
    let inverse = cmd.apply(s).unwrap();
    let mut undone = s.clone();
    inverse.apply(&mut undone).unwrap();
    assert_eq!(undone, before, "the inverse restores the sprite");
}

#[test]
fn auto_removal_remaps_every_layer_and_undoes_exactly() {
    let mut s = sprite();
    // La celda 2 (único uso del tile 2 en la capa 2, pero la capa 3 también lo usa): pasa a ser
    // igual al tile 1, así que el tile 2 sigue en uso y no se borra.
    let cmd = draw_tilemap_pixels(&s, LayerId(2), 0, &[px(8, 0, RED)], TilesetMode::Auto).unwrap();
    apply_and_check_undo(&mut s, cmd);
    assert_eq!(grid(&s, 2, 0), vec![Tile(1); 3]);
    assert_eq!(
        s.tilesets[0].tiles.len(),
        3,
        "tile 2 is still used by layer 3"
    );
    // Ahora la capa 3 deja el tile 2: se borra… y nada más lo usaba.
    let cmd = draw_tilemap_pixels(&s, LayerId(3), 0, &[px(0, 0, RED)], TilesetMode::Auto).unwrap();
    apply_and_check_undo(&mut s, cmd);
    assert_eq!(s.tilesets[0].tiles.len(), 2);
    assert_eq!(grid(&s, 3, 0), vec![Tile(1)]);
}

#[test]
fn auto_removal_remaps_the_other_layers_with_their_flips() {
    let mut s = sprite();
    // Tile 3 verde, usado volteado por la capa 3.
    let mut img = Image::new(4, 4, ColorMode::Rgba);
    img.set(1, 0, PixelValue::Rgba(Rgba::rgb(0, 255, 0)))
        .unwrap();
    s.tilesets[0].tiles.push(Arc::new(img));
    s.tilemaps.insert(
        ImageId(3),
        Arc::new(TileGrid {
            width: 1,
            height: 1,
            tiles: vec![Tile::new(3, true, false, false)],
        }),
    );
    // La celda 2 de la capa 2 (tile 2) pasa a ser igual al 1: el 2 queda sin uso y se borra.
    let cmd = draw_tilemap_pixels(&s, LayerId(2), 0, &[px(8, 0, RED)], TilesetMode::Auto).unwrap();
    apply_and_check_undo(&mut s, cmd);
    assert_eq!(s.tilesets[0].tiles.len(), 3);
    assert_eq!(grid(&s, 3, 0), vec![Tile::new(2, true, false, false)]);
}

#[test]
fn removed_tiles_take_their_user_data_with_them() {
    let mut s = sprite();
    s.layers.retain(|l| l.id != LayerId(3));
    s.cels.remove(&(LayerId(3), 0));
    let data = |b: u8| {
        Some(RawChunk {
            kind: 0x2020,
            data: vec![b],
        })
    };
    s.tilesets[0].tile_user_data = vec![None, data(1), data(2)];
    let cmd = draw_tilemap_pixels(&s, LayerId(2), 0, &[px(8, 0, RED)], TilesetMode::Auto).unwrap();
    cmd.apply(&mut s).unwrap();
    assert_eq!(s.tilesets[0].tiles.len(), 2);
    assert_eq!(s.tilesets[0].tile_user_data, vec![None, data(1)]);
}

#[test]
fn indexed_sprites_use_the_transparent_index() {
    let mode = ColorMode::Indexed {
        transparent_index: 0,
    };
    let mut s = Sprite::new(8, 4, mode);
    let mut ts = Tileset::new("t", (4, 4), mode);
    let mut img = Image::new(4, 4, mode);
    img.set(0, 0, PixelValue::Index(3)).unwrap();
    ts.tiles.push(Arc::new(img));
    s.tilesets.push(ts);
    let add = crate::ops::new_tilemap_layer(&s, None, 0);
    add.apply(&mut s).unwrap();
    let layer = s.layers.last().unwrap().id;
    // Una celda nueva con el índice 3 en (0,0) es el tile 1.
    let cmd = draw_tilemap_pixels(
        &s,
        layer,
        0,
        &[(4, 0, PixelValue::Index(3))],
        TilesetMode::Stack,
    )
    .unwrap();
    cmd.apply(&mut s).unwrap();
    let cel = *s.cel(layer, 0).unwrap();
    assert_eq!(cel.position, (4, 0), "the minimal grid");
    assert_eq!(s.tilemap(cel.image).unwrap().tiles, vec![Tile(1)]);
    assert_eq!(s.tilesets[0].tiles.len(), 2);
    // Borrarla con el índice transparente la deja vacía.
    let cmd = draw_tilemap_pixels(
        &s,
        layer,
        0,
        &[(4, 0, PixelValue::Index(0))],
        TilesetMode::Auto,
    )
    .unwrap();
    cmd.apply(&mut s).unwrap();
    assert_eq!(s.tilemap(cel.image).unwrap().tiles, vec![Tile::EMPTY]);
    // Un valor RGBA en un sprite indexado se ignora.
    assert_eq!(
        draw_tilemap_pixels(&s, layer, 0, &[px(0, 0, RED)], TilesetMode::Auto),
        Err(TilePaintError::Nothing)
    );
}

#[test]
fn invisible_pixels_count_as_transparent() {
    let s = sprite();
    // Alfa 0 con RGB: no se ve, así que la celda vacía sigue vacía.
    let ghost = Rgba::new(10, 20, 30, 0);
    for mode in TilesetMode::ALL {
        assert_eq!(
            draw_tilemap_pixels(&s, LayerId(2), 0, &[px(13, 1, ghost)], mode),
            Err(TilePaintError::Nothing),
            "{mode:?}"
        );
    }
}

#[test]
fn errors_for_other_layers_outside_pixels_and_limits() {
    let s = sprite();
    assert_eq!(
        draw_tilemap_pixels(&s, LayerId(1), 0, &[px(0, 0, BLUE)], TilesetMode::Auto),
        Err(TilePaintError::NotTilemap)
    );
    assert_eq!(
        draw_tilemap_pixels(&s, LayerId(9), 0, &[px(0, 0, BLUE)], TilesetMode::Auto),
        Err(TilePaintError::NotTilemap)
    );
    assert_eq!(
        draw_tilemap_pixels(
            &s,
            LayerId(2),
            0,
            &[px(-1, 0, BLUE), px(16, 0, BLUE)],
            TilesetMode::Auto
        ),
        Err(TilePaintError::Nothing),
        "pixels outside the canvas are ignored"
    );
    assert_eq!(
        draw_tilemap_pixels(&s, LayerId(2), 0, &[px(0, 0, RED)], TilesetMode::Stack),
        Err(TilePaintError::Nothing),
        "painting what is there changes nothing"
    );
    assert_eq!(
        draw_tilemap_pixels(&s, LayerId(2), 5, &[px(0, 0, BLUE)], TilesetMode::Auto),
        Err(TilePaintError::Nothing),
        "no such frame"
    );
    // Un tileset lleno no recibe tiles nuevos.
    let mut full = Sprite::new(4, 1, ColorMode::Rgba);
    let mut ts = Tileset::new("t", (1, 1), ColorMode::Rgba);
    let one = Arc::new(Image::new(1, 1, ColorMode::Rgba));
    ts.tiles.resize(MAX_TILESET_TILES as usize, one);
    full.tilesets.push(ts);
    crate::ops::new_tilemap_layer(&full, None, 0)
        .apply(&mut full)
        .unwrap();
    let layer = full.layers.last().unwrap().id;
    assert_eq!(
        draw_tilemap_pixels(&full, layer, 0, &[px(0, 0, BLUE)], TilesetMode::Stack),
        Err(TilePaintError::TooLarge)
    );
}

#[test]
fn growing_left_moves_linked_cels_together_without_unlinking() {
    let mut s = sprite();
    s.frames.push(Frame { duration_ms: 100 });
    // La capa 3 en (8, 0), vinculada en los frames 0 y 1.
    let cel = Cel {
        position: (8, 0),
        opacity: 200,
        image: ImageId(3),
    };
    s.cels.insert((LayerId(3), 0), cel);
    s.cels.insert((LayerId(3), 1), cel);
    let cmd = draw_tilemap_pixels(&s, LayerId(3), 1, &[px(1, 1, BLUE)], TilesetMode::Auto).unwrap();
    apply_and_check_undo(&mut s, cmd);
    for f in 0..2 {
        let c = *s.cel(LayerId(3), f).unwrap();
        assert_eq!((c.position, c.opacity, c.image), ((0, 0), 200, ImageId(3)));
    }
    assert_eq!(grid(&s, 3, 1), vec![Tile(3), Tile::EMPTY, Tile(2)]);
}

#[test]
fn manual_writes_through_a_diagonal_flip() {
    let mut s = sprite();
    s.tilemaps.insert(
        ImageId(3),
        Arc::new(TileGrid {
            width: 1,
            height: 1,
            tiles: vec![Tile::new(1, false, false, true)],
        }),
    );
    // La celda transpuesta: pintar (3, 1) escribe el píxel (1, 3) del tile.
    let cmd =
        draw_tilemap_pixels(&s, LayerId(3), 0, &[px(3, 1, BLUE)], TilesetMode::Manual).unwrap();
    cmd.apply(&mut s).unwrap();
    assert_eq!(
        s.tilesets[0].tiles[1].get(1, 3),
        Some(PixelValue::Rgba(BLUE))
    );
    assert_eq!(grid(&s, 3, 0), vec![Tile::new(1, false, false, true)]);
}

#[test]
fn move_tile_remaps_every_grid_with_flips_and_user_data() {
    let mut s = sprite();
    let mut img = Image::new(4, 4, ColorMode::Rgba);
    img.set(2, 2, PixelValue::Rgba(RED)).unwrap();
    s.tilesets[0].tiles.push(Arc::new(img));
    let data = Some(RawChunk {
        kind: 0x2020,
        data: vec![7],
    });
    s.tilesets[0].tile_user_data = vec![None, data.clone()];
    s.tilemaps.insert(
        ImageId(3),
        Arc::new(TileGrid {
            width: 2,
            height: 1,
            tiles: vec![Tile::new(1, true, false, false), Tile(3)],
        }),
    );
    let tiles = s.tilesets[0].tiles.clone();
    // El 1 pasa al final: 2 → 1, 3 → 2, 1 → 3.
    let cmd = move_tile(&s, 0, 1, 3).unwrap();
    apply_and_check_undo(&mut s, cmd);
    let ts = &s.tilesets[0];
    assert!(Arc::ptr_eq(&ts.tiles[3], &tiles[1]) && Arc::ptr_eq(&ts.tiles[1], &tiles[2]));
    assert_eq!(ts.tile_user_data, vec![None, None, None, data]);
    assert_eq!(grid(&s, 2, 0), vec![Tile(3), Tile(3), Tile(1)]);
    assert_eq!(
        grid(&s, 3, 0),
        vec![Tile::new(3, true, false, false), Tile(2)]
    );
    // Y de vuelta.
    let cmd = move_tile(&s, 0, 3, 1).unwrap();
    cmd.apply(&mut s).unwrap();
    assert_eq!(grid(&s, 2, 0), vec![Tile(1), Tile(1), Tile(2)]);
    assert!(Arc::ptr_eq(&s.tilesets[0].tiles[1], &tiles[1]));
}

#[test]
fn move_tile_rejects_the_empty_tile_and_bad_indices() {
    let s = sprite();
    assert!(move_tile(&s, 0, 0, 1).is_none());
    assert!(move_tile(&s, 0, 1, 0).is_none());
    assert!(move_tile(&s, 0, 1, 1).is_none());
    assert!(move_tile(&s, 0, 1, 3).is_none());
    assert!(move_tile(&s, 1, 1, 2).is_none());
    assert!(move_tile(&s, 0, 2, 1).is_some());
}

#[test]
fn tileset_modes_serialize_in_lowercase() {
    assert_eq!(
        serde_json::to_string(&TilesetMode::Stack).unwrap(),
        "\"stack\""
    );
    assert_eq!(TilesetMode::default(), TilesetMode::Auto);
}
