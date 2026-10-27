/// Lo que puede tardar en rechazar un archivo armado para trabar la lectura: lo normal es
/// mucho menos, pero con todas las pruebas corriendo a la vez la máquina va más lenta (con 1 s
/// fallaba a veces sin motivo). Lo que se cuida es no tardar minutos.
const FAST_ENOUGH: std::time::Duration = std::time::Duration::from_secs(5);

use cs_commands::Command;
use cs_commands::ops::{self, LayerDrop};
use cs_core::*;

use super::*;

/// Igualdad semántica: mismos metadatos, mismo contenido y mismos vínculos (ignorando ids).
pub(crate) fn assert_equivalent(a: &Sprite, b: &Sprite) {
    assert_eq!(
        (a.width, a.height, a.color_mode),
        (b.width, b.height, b.color_mode)
    );
    assert_eq!(a.palette, b.palette);
    assert_eq!(a.frames, b.frames);
    assert_eq!(a.tags, b.tags);
    assert_eq!(a.extra_chunks, b.extra_chunks);
    assert_eq!(a.slices, b.slices);
    assert_eq!(a.grid, b.grid);
    assert_eq!(a.tilesets, b.tilesets);
    assert_eq!(a.layers.len(), b.layers.len());
    for (la, lb) in a.layers.iter().zip(&b.layers) {
        // Un grupo con opacidad 0 se lee como 255: así guarda Aseprite todos los grupos.
        let opacity = match (la.kind, la.opacity) {
            (LayerKind::Group, 0) => 255,
            (_, o) => o,
        };
        assert_eq!(
            (
                &la.name, la.kind, la.visible, la.locked, opacity, la.blend, la.tileset
            ),
            (
                &lb.name, lb.kind, lb.visible, lb.locked, lb.opacity, lb.blend, lb.tileset
            )
        );
        let parent_index = |s: &Sprite, l: &Layer| l.parent.and_then(|p| s.layer_index(p));
        assert_eq!(parent_index(a, la), parent_index(b, lb));
        for f in 0..a.frames.len() {
            let (ca, cb) = (a.cel(la.id, f), b.cel(lb.id, f));
            assert_eq!(ca.is_some(), cb.is_some(), "cel {} frame {f}", la.name);
            if let (Some(ca), Some(cb)) = (ca, cb) {
                assert_eq!((ca.position, ca.opacity), (cb.position, cb.opacity));
                assert_eq!(a.image(ca.image), b.image(cb.image));
                assert_eq!(a.tilemap(ca.image), b.tilemap(cb.image));
            }
        }
    }
    // Al leer, un grupo con opacidad 0 vale 255 (ver arriba): se compara lo que se ve así.
    let mut a = a.clone();
    for l in a.layers.iter_mut() {
        if l.kind == LayerKind::Group && l.opacity == 0 {
            l.opacity = 255;
        }
    }
    let a = &a;
    let (ma, mb) = (
        cs_render::materialize_tilemaps(a, 0..a.frames.len()),
        cs_render::materialize_tilemaps(b, 0..b.frames.len()),
    );
    for f in 0..a.frames.len() {
        assert_eq!(
            cs_render::render_frame(&ma, f),
            cs_render::render_frame(&mb, f)
        );
    }
    assert_eq!(linked_pairs(a), linked_pairs(b));
}

fn roundtrip(s: &Sprite) -> Sprite {
    let (back, report) = read(&write(s)).expect("read back");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    back
}

/// Sprite con casi todo: grupos, capas ocultas/bloqueadas, blend, cels vinculados,
/// posiciones negativas, varios frames con duraciones y tags.
pub(crate) fn rich_sprite(mode: ColorMode) -> Sprite {
    let mut s = Sprite::new(12, 8, mode);
    let value = |i: u8| match mode {
        ColorMode::Rgba | ColorMode::Grayscale => {
            PixelValue::Rgba(Rgba::new(i * 20, 255 - i * 10, i, 200 + i))
        }
        ColorMode::Indexed { .. } => PixelValue::Index(i + 1),
    };
    let draw = |s: &mut Sprite, layer: u32, frame: usize, pts: &[(i32, i32)], v: PixelValue| {
        Command::DrawPixels {
            layer: LayerId(layer),
            frame,
            pixels: pts.iter().map(|&(x, y)| (x, y, v)).collect(),
        }
        .apply(s)
        .unwrap();
    };
    draw(&mut s, 1, 0, &[(0, 0), (11, 7), (5, 3)], value(1));
    ops::new_layer(&s.clone(), Some(LayerId(1)), LayerKind::Group)
        .apply(&mut s)
        .unwrap(); // 2
    ops::new_layer(&s.clone(), Some(LayerId(2)), LayerKind::Image)
        .apply(&mut s)
        .unwrap(); // 3
    ops::move_layer(&s.clone(), LayerId(3), LayerDrop::Into(LayerId(2)))
        .unwrap()
        .apply(&mut s)
        .unwrap();
    draw(&mut s, 3, 0, &[(2, 2), (3, 2)], value(2));
    s.layer_mut(LayerId(3)).unwrap().blend = BlendMode::Multiply;
    s.layer_mut(LayerId(3)).unwrap().opacity = 128;
    s.layer_mut(LayerId(2)).unwrap().visible = false;
    s.layer_mut(LayerId(1)).unwrap().locked = true;
    s.layer_mut(LayerId(1)).unwrap().blend = BlendMode::Other(7);
    s.layer_mut(LayerId(1)).unwrap().name = "Fondo ñ ✓".into();
    ops::duplicate_frame(&s.clone(), 0, true)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    ops::duplicate_frame(&s.clone(), 1, false)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    // Cel con posición negativa.
    let cel = *s.cel(LayerId(3), 2).unwrap();
    s.cels.insert(
        (LayerId(3), 2),
        Cel {
            position: (-3, -1),
            opacity: 90,
            ..cel
        },
    );
    s.frames[1].duration_ms = 33;
    s.tags = vec![
        Tag {
            name: "idle".into(),
            from: 0,
            to: 1,
            direction: AnimDirection::PingPong,
            color: Rgba::new(10, 20, 30, 255),
            repeat: 3,
        },
        Tag {
            name: "hit".into(),
            from: 2,
            to: 2,
            direction: AnimDirection::PingPongReverse,
            color: Rgba::new(200, 0, 0, 255),
            repeat: 0,
        },
    ];
    s.grid = GridSettings {
        size: (8, 6),
        offset: (2, -1),
    };
    s.slices = vec![
        Slice {
            name: "hitbox".into(),
            color: Rgba::rgb(255, 0, 0),
            text: "damage=3".into(),
            keys: vec![
                SliceKey {
                    frame: 0,
                    bounds: (1, 2, 3, 4),
                    center: None,
                    pivot: Some((1, 4)),
                },
                SliceKey {
                    frame: 2,
                    bounds: (-2, 0, 5, 5),
                    center: None,
                    pivot: Some((2, 5)),
                },
            ],
        },
        Slice {
            name: "panel".into(),
            color: SLICE_COLOR,
            text: String::new(),
            keys: vec![SliceKey {
                frame: 0,
                bounds: (0, 0, 8, 8),
                center: Some((2, 2, 4, 4)),
                pivot: None,
            }],
        },
    ];
    s
}

#[test]
fn rgba_roundtrip() {
    let s = rich_sprite(ColorMode::Rgba);
    let back = roundtrip(&s);
    assert_equivalent(&s, &back);
    assert!(!linked_pairs(&back).is_empty(), "keeps linked cels");
}

#[test]
fn indexed_roundtrip() {
    let s = rich_sprite(ColorMode::Indexed {
        transparent_index: 0,
    });
    assert_equivalent(&s, &roundtrip(&s));
}

#[test]
fn write_is_a_fixpoint_after_one_roundtrip() {
    let once = write(&roundtrip(&rich_sprite(ColorMode::Rgba)));
    let (again, _) = read(&once).unwrap();
    assert_eq!(write(&again), once);
}

#[test]
fn bad_magic_and_truncation_never_panic() {
    let bytes = write(&rich_sprite(ColorMode::Rgba));
    let mut bad = bytes.clone();
    bad[4] = 0;
    assert!(matches!(read(&bad), Err(IoError::BadMagic)));
    for len in 0..bytes.len() {
        assert!(
            read(&bytes[..len]).is_err(),
            "prefix of {len} bytes should fail"
        );
    }
}

#[test]
fn corrupted_bytes_never_panic() {
    let bytes = write(&rich_sprite(ColorMode::Rgba));
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0xA5;
        let _ = read(&b);
    }
}

#[test]
fn reads_old_palette_chunk_and_grayscale() {
    // Archivo mínimo armado a mano: 2×1, grayscale, una capa y paleta vieja 0x0004.
    let mut out = Vec::new();
    out.w_u32(0);
    out.w_u16(MAGIC);
    out.w_u16(1);
    out.w_u16(2);
    out.w_u16(1);
    out.w_u16(16);
    out.w_u32(1);
    out.w_u16(50);
    out.w_zeros(8);
    out.w_u8(0);
    out.w_zeros(3);
    out.w_u16(2);
    out.w_zeros(2 + 8 + 84);
    assert_eq!(out.len(), 128);
    let mut old_palette = Vec::new();
    old_palette.w_u16(1);
    old_palette.extend_from_slice(&[0, 2, 255, 0, 0, 0, 0, 255]);
    let mut layer = Vec::new();
    layer.w_u16(3);
    layer.w_u16(0);
    layer.w_u16(0);
    layer.w_zeros(4);
    layer.w_u16(0);
    layer.w_u8(255);
    layer.w_zeros(3);
    layer.w_string("gray");
    let mut cel = Vec::new();
    cel.w_u16(0);
    cel.w_i16(0);
    cel.w_i16(0);
    cel.w_u8(255);
    cel.w_u16(CEL_RAW);
    cel.w_i16(0);
    cel.w_zeros(5);
    cel.w_u16(2);
    cel.w_u16(1);
    cel.extend_from_slice(&[100, 255, 200, 128]);
    write_frame(
        &mut out,
        0,
        &[
            (CHUNK_OLD_PALETTE, old_palette),
            (CHUNK_LAYER, layer),
            (CHUNK_CEL, cel),
        ],
    );
    let (s, report) = read(&out).unwrap();
    assert_eq!(
        s.palette.colors(),
        &[Rgba::rgb(255, 0, 0), Rgba::rgb(0, 0, 255)]
    );
    assert_eq!(s.frames[0].duration_ms, 50, "uses header speed");
    assert_eq!(
        s.pixel(LayerId(1), 0, 1, 0),
        PixelValue::Rgba(Rgba::new(200, 200, 200, 128))
    );
    assert_eq!(s.color_mode, ColorMode::Grayscale, "stays in grayscale");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

#[test]
fn a_grayscale_sprite_roundtrips_as_grayscale() {
    let mut s = Sprite::new(4, 4, ColorMode::Grayscale);
    Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, PixelValue::Rgba(Rgba::new(255, 0, 0, 200)))],
    }
    .apply(&mut s)
    .unwrap();
    let painted = s.pixel(LayerId(1), 0, 1, 1);
    assert_eq!(
        painted,
        PixelValue::Rgba(Rgba::new(76, 76, 76, 200)),
        "its gray"
    );
    let back = roundtrip(&s);
    assert_eq!(back.color_mode, ColorMode::Grayscale);
    assert_eq!(back.pixel(LayerId(1), 0, 1, 1), painted);
}

/// Sprite con una capa de imagen y una tilemap (id 2) con un tileset de 4×4 con user data;
/// 2 frames con el cel tilemap vinculado, flips y la grilla corrida hacia la izquierda.
fn tilemap_sprite(mode: ColorMode) -> Sprite {
    let mut s = Sprite::new(16, 12, mode);
    let value = |i: u8| match mode {
        ColorMode::Rgba | ColorMode::Grayscale => {
            PixelValue::Rgba(Rgba::new(i * 30, 200 - i * 20, i * 7, 255))
        }
        ColorMode::Indexed { .. } => PixelValue::Index(i + 1),
    };
    let mut ts = Tileset::new("terrain ✓", (4, 4), mode);
    for i in 1..=3u8 {
        let mut tile = Image::new(4, 4, mode);
        tile.set(0, 0, value(i)).unwrap();
        tile.set(3, 1, value(i + 1)).unwrap();
        ts.tiles.push(Arc::new(tile));
    }
    ts.base_index = 0;
    ts.allowed_flips = AllowedFlips {
        x: true,
        y: false,
        d: true,
    };
    // Texto "grass" y color (flags 1 | 2).
    let mut ud = Vec::new();
    ud.w_u32(3);
    ud.w_string("grass");
    ud.extend_from_slice(&[1, 2, 3, 255]);
    ts.user_data = Some(RawChunk {
        kind: CHUNK_USER_DATA,
        data: ud,
    });
    let mut tile_ud = Vec::new();
    tile_ud.w_u32(1);
    tile_ud.w_string("solid");
    ts.tile_user_data = vec![
        None,
        None,
        Some(RawChunk {
            kind: CHUNK_USER_DATA,
            data: tile_ud,
        }),
    ];
    Command::SetTileset {
        index: 0,
        tileset: Some(ts),
    }
    .apply(&mut s)
    .unwrap();
    ops::new_tilemap_layer(&s.clone(), Some(LayerId(1)), 0)
        .apply(&mut s)
        .unwrap();
    Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, value(4))],
    }
    .apply(&mut s)
    .unwrap();
    let cells = [
        (0, 0, Tile::new(1, false, false, false)),
        (1, 0, Tile::new(2, true, false, false)),
        (2, 1, Tile::new(3, false, true, true)),
        (3, 2, Tile::new(1, true, true, false)),
        (-1, 0, Tile::new(2, false, false, false)),
    ];
    ops::set_tiles(&s, LayerId(2), 0, &cells)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    ops::duplicate_frame(&s.clone(), 0, true)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    s
}

#[test]
fn tilemap_layers_without_a_tileset_are_written_as_empty_image_layers() {
    for broken in [None, Some(7)] {
        let mut s = tilemap_sprite(ColorMode::Rgba);
        s.layer_mut(LayerId(2)).unwrap().tileset = broken;
        let (back, report) = read(&write(&s)).expect("the file stays readable");
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let layer = back.layer(LayerId(2)).unwrap();
        assert_eq!(
            (layer.kind, layer.tileset),
            (LayerKind::Image, None),
            "{broken:?}"
        );
        assert!(back.cel(LayerId(2), 0).is_none() && back.cel(LayerId(2), 1).is_none());
        assert!(back.cel(LayerId(1), 0).is_some(), "other cels are kept");
        assert_eq!(back.tilesets, s.tilesets);
    }
}

#[test]
fn tilemaps_roundtrip_in_both_modes() {
    for mode in [
        ColorMode::Rgba,
        ColorMode::Indexed {
            transparent_index: 0,
        },
    ] {
        let s = tilemap_sprite(mode);
        assert_eq!(s.cel(LayerId(2), 0).unwrap().position, (-4, 0));
        let back = roundtrip(&s);
        assert_equivalent(&s, &back);
        assert_eq!(back.layers[1].kind, LayerKind::Tilemap);
        assert_eq!(back.layers[1].tileset, Some(0));
        assert!(!linked_pairs(&back).is_empty(), "linked tilemap cels");
        let rendered = cs_render::render_frame(&cs_render::materialize_tilemaps(&back, 0..1), 0);
        assert!(rendered.iter().any(|c| c.a > 0));
    }
}

#[test]
fn written_tilemap_cels_declare_aseprites_standard_masks() {
    let bytes = write(&tilemap_sprite(ColorMode::Rgba));
    let mut masks = Vec::new();
    for m in [0x1fff_ffffu32, 0x8000_0000, 0x4000_0000, 0x2000_0000] {
        masks.extend_from_slice(&m.to_le_bytes());
    }
    assert!(
        bytes.windows(masks.len()).any(|w| w == masks.as_slice()),
        "index/X/Y/D masks 0x1fffffff/0x80000000/0x40000000/0x20000000"
    );
    assert_eq!(
        (Tile::FLIP_X, Tile::FLIP_Y, Tile::FLIP_D),
        (0x8000_0000, 0x4000_0000, 0x2000_0000)
    );
}

#[test]
fn tilemap_write_is_a_fixpoint() {
    let once = write(&roundtrip(&tilemap_sprite(ColorMode::Rgba)));
    let (again, _) = read(&once).unwrap();
    assert_eq!(write(&again), once);
}

#[test]
fn corrupted_tilemap_files_never_panic() {
    let bytes = write(&tilemap_sprite(ColorMode::Rgba));
    for len in 0..bytes.len() {
        let _ = read(&bytes[..len]);
    }
    for i in 0..bytes.len() {
        let mut b = bytes.clone();
        b[i] ^= 0xA5;
        let _ = read(&b);
    }
}

/// `.aseprite` de 8×8 y un frame con los chunks dados.
fn file_with(chunks: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, &Sprite::new(8, 8, ColorMode::Rgba));
    write_frame(&mut out, 100, chunks);
    out
}

/// Capa tilemap que pide el tileset con id 0 del archivo.
fn tilemap_layer_chunk() -> Vec<u8> {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    // El escritor solo guarda como tilemap una capa cuyo tileset existe.
    s.tilesets.push(Tileset::new("t", (1, 1), ColorMode::Rgba));
    s.layers[0].kind = LayerKind::Tilemap;
    s.layers[0].tileset = Some(0);
    layer_chunk(&s, &s.layers[0])
}

fn tileset_header(flags: u32, count: u32, size: u16, name: &str) -> Vec<u8> {
    let mut d = Vec::new();
    d.w_u32(0);
    d.w_u32(flags);
    d.w_u32(count);
    d.w_u16(size);
    d.w_u16(size);
    d.w_i16(1);
    d.w_zeros(14);
    d.w_string(name);
    d
}

#[test]
fn external_tilesets_load_empty_with_a_warning() {
    let mut ts = tileset_header(TILESET_EXTERNAL | TILESET_EMPTY_ZERO, 3, 4, "outside");
    ts.w_u32(1);
    ts.w_u32(0);
    let (s, report) = read(&file_with(&[
        (CHUNK_TILESET, ts),
        (CHUNK_LAYER, tilemap_layer_chunk()),
    ]))
    .unwrap();
    assert_eq!(s.tilesets.len(), 1);
    assert_eq!(s.tilesets[0].tiles.len(), 1, "only the empty tile");
    assert_eq!(s.layers[0].kind, LayerKind::Tilemap);
    assert_eq!(s.layers[0].tileset, Some(0));
    assert!(
        report.warnings.iter().any(|w| w.contains("external")),
        "{:?}",
        report.warnings
    );
}

#[test]
fn huge_tiles_are_rejected_before_allocating() {
    // Tileset externo de tiles de 65535×65535: el archivo pesa unos bytes, pero el tile 0
    // ocuparía 16 GiB.
    let mut ts = tileset_header(TILESET_EXTERNAL | TILESET_EMPTY_ZERO, 1, u16::MAX, "huge");
    ts.w_u32(1);
    ts.w_u32(0);
    let file = file_with(&[(CHUNK_TILESET, ts), (CHUNK_LAYER, tilemap_layer_chunk())]);
    assert!(file.len() < 600, "{} bytes", file.len());
    let start = std::time::Instant::now();
    let result = read(&file);
    assert!(
        matches!(&result, Err(IoError::Corrupt(m)) if m.contains("65535")),
        "{:?}",
        result.map(|(_, r)| r.warnings)
    );
    assert!(start.elapsed() < FAST_ENOUGH, "{:?}", start.elapsed());
    // El límite es por área: 2048×2048 todavía se acepta, 2049×2049 no.
    for (size, ok) in [(2048u16, true), (2049, false)] {
        let mut ts = tileset_header(TILESET_EXTERNAL | TILESET_EMPTY_ZERO, 1, size, "t");
        ts.w_u32(1);
        ts.w_u32(0);
        assert_eq!(
            read(&file_with(&[(CHUNK_TILESET, ts)])).is_ok(),
            ok,
            "{size}"
        );
    }
}

fn external_tileset(id: u32, w: u16, h: u16) -> Vec<u8> {
    let mut d = Vec::new();
    d.w_u32(id);
    d.w_u32(TILESET_EXTERNAL | TILESET_EMPTY_ZERO);
    d.w_u32(1);
    d.w_u16(w);
    d.w_u16(h);
    d.w_i16(1);
    d.w_zeros(14);
    d.w_string("x");
    d.w_u32(1);
    d.w_u32(0);
    d
}

#[test]
fn many_large_tilesets_in_a_small_file_are_bounded() {
    // 200 tilesets externos de tiles de 2048×2048: el tile 0 (vacío) de todos es el mismo.
    let chunks: Vec<_> = (0..200)
        .map(|i| (CHUNK_TILESET, external_tileset(i, 2048, 2048)))
        .collect();
    let file = file_with(&chunks);
    assert!(file.len() < 12_000, "{} bytes", file.len());
    let start = std::time::Instant::now();
    let (s, _) = read(&file).unwrap();
    assert!(start.elapsed() < FAST_ENOUGH, "{:?}", start.elapsed());
    assert_eq!(s.tilesets.len(), 200);
    assert!(
        s.tilesets
            .iter()
            .all(|t| Arc::ptr_eq(&t.tiles[0], &s.tilesets[0].tiles[0])),
        "one shared empty tile"
    );
    // Con tamaños distintos no se pueden compartir: pasan el presupuesto total y se rechazan.
    let chunks: Vec<_> = (0..200)
        .map(|i| (CHUNK_TILESET, external_tileset(i, 2048, 2048 - i as u16)))
        .collect();
    let start = std::time::Instant::now();
    let result = read(&file_with(&chunks));
    assert!(
        matches!(&result, Err(IoError::Corrupt(m)) if m.contains("in total")),
        "{:?}",
        result.map(|(_, r)| r.warnings)
    );
    assert!(start.elapsed() < FAST_ENOUGH, "{:?}", start.elapsed());
    // Un tileset embebido que solo por su cuenta pasa el presupuesto se rechaza antes de
    // descomprimir (los datos ni siquiera son válidos).
    let mut ts = tileset_header(TILESET_EMBEDDED | TILESET_EMPTY_ZERO, 17, 2048, "big");
    ts.w_u32(4);
    ts.w_u32(0);
    let result = read(&file_with(&[(CHUNK_TILESET, ts)]));
    assert!(
        matches!(&result, Err(IoError::Corrupt(m)) if m.contains("in total")),
        "{:?}",
        result.map(|(_, r)| r.warnings)
    );
}

/// Cel tilemap de `w`×`h` tiles vacíos en la capa 0.
fn empty_tilemap_cel(w: u16, h: u16) -> Vec<u8> {
    let mut cel = Vec::new();
    cel.w_u16(0);
    cel.w_i16(0);
    cel.w_i16(0);
    cel.w_u8(255);
    cel.w_u16(CEL_TILEMAP);
    cel.w_i16(0);
    cel.w_zeros(5);
    cel.w_u16(w);
    cel.w_u16(h);
    cel.w_u16(32);
    cel.w_u32(Tile::INDEX_MASK);
    cel.w_u32(Tile::FLIP_X);
    cel.w_u32(Tile::FLIP_Y);
    cel.w_u32(Tile::FLIP_D);
    cel.w_zeros(10);
    cel.extend_from_slice(&deflate(&vec![0; w as usize * h as usize * 4]));
    cel
}

#[test]
fn missing_tilesets_and_huge_tilemap_cels_are_dropped_with_warnings() {
    // Capa que pide el tileset 0 sin ningún tileset en el archivo.
    let (s, report) = read(&file_with(&[
        (CHUNK_LAYER, tilemap_layer_chunk()),
        (CHUNK_CEL, empty_tilemap_cel(1, 1)),
    ]))
    .unwrap();
    assert_eq!(s.layers[0].tileset, None);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("missing tileset")),
        "{:?}",
        report.warnings
    );
    // Tiles de 2048×2048 en una grilla de 5×5: 10240² píxeles, más que MAX_TILEMAP_PIXELS. La
    // tira del tileset (solo el tile 0) ocupa 16 MiB sin comprimir y unos KiB comprimida.
    let mut ts = tileset_header(TILESET_EMBEDDED | TILESET_EMPTY_ZERO, 1, 2048, "big");
    let pixels = deflate(&vec![0; 2048 * 2048 * 4]);
    ts.w_u32(pixels.len() as u32);
    ts.extend_from_slice(&pixels);
    let (s, report) = read(&file_with(&[
        (CHUNK_TILESET, ts),
        (CHUNK_LAYER, tilemap_layer_chunk()),
        (CHUNK_CEL, empty_tilemap_cel(5, 5)),
    ]))
    .unwrap();
    assert!(s.cels.is_empty() && s.tilemaps.is_empty());
    assert!(
        report.warnings.iter().any(|w| w.contains("too large")),
        "{:?}",
        report.warnings
    );
}

#[test]
fn tilesets_with_too_many_tiles_are_rejected_before_allocating() {
    // 1×1 tiles: la cuenta, no el tamaño de la tira, es lo que dispararía las asignaciones.
    let mut ts = tileset_header(
        TILESET_EMBEDDED | TILESET_EMPTY_ZERO,
        100_000_000,
        1,
        "huge",
    );
    let pixels = deflate(&[0; 16]);
    ts.w_u32(pixels.len() as u32);
    ts.extend_from_slice(&pixels);
    let start = std::time::Instant::now();
    let result = read(&file_with(&[(CHUNK_TILESET, ts)]));
    assert!(
        matches!(&result, Err(IoError::Corrupt(m)) if m.contains("tiles")),
        "{:?}",
        result.map(|(_, r)| r.warnings)
    );
    assert!(start.elapsed() < FAST_ENOUGH, "{:?}", start.elapsed());
}

#[test]
fn tilesets_without_tiles_or_link_load_empty_with_a_warning() {
    let ts = tileset_header(TILESET_EMPTY_ZERO, 3, 4, "nothing");
    let (s, report) = read(&file_with(&[
        (CHUNK_TILESET, ts),
        (CHUNK_LAYER, tilemap_layer_chunk()),
    ]))
    .unwrap();
    assert_eq!(s.tilesets[0].tiles.len(), 1, "only the empty tile");
    assert!(
        report.warnings.iter().any(|w| w.contains("no tiles")),
        "{:?}",
        report.warnings
    );
}

#[test]
fn tilemap_cels_wider_than_the_format_are_not_written() {
    let mut s = tilemap_sprite(ColorMode::Rgba);
    let cel = *s.cel(LayerId(2), 0).unwrap();
    // Una grilla de 65536 tiles de ancho no entra en el WORD del cel.
    s.tilemaps
        .insert(cel.image, Arc::new(TileGrid::new(u16::MAX as u32 + 1, 1)));
    let (back, _) = read(&write(&s)).expect("the file stays readable");
    assert!(back.cel(LayerId(2), 0).is_none() && back.cel(LayerId(2), 1).is_none());
    assert_eq!(back.layers.len(), 2);
    assert!(back.cel(LayerId(1), 0).is_some(), "other cels are kept");
}

#[test]
fn tilemap_cels_that_are_not_32_bit_are_skipped_with_a_warning() {
    let mut ts = tileset_header(TILESET_EMBEDDED | TILESET_EMPTY_ZERO, 1, 2, "t");
    let pixels = deflate(&[0; 16]);
    ts.w_u32(pixels.len() as u32);
    ts.extend_from_slice(&pixels);
    let mut cel = Vec::new();
    cel.w_u16(0);
    cel.w_i16(0);
    cel.w_i16(0);
    cel.w_u8(255);
    cel.w_u16(CEL_TILEMAP);
    cel.w_i16(0);
    cel.w_zeros(5);
    cel.w_u16(1);
    cel.w_u16(1);
    cel.w_u16(16);
    cel.w_u32(0x1fff);
    cel.w_u32(0x2000);
    cel.w_u32(0x4000);
    cel.w_u32(0x8000);
    cel.w_zeros(10);
    cel.extend_from_slice(&deflate(&[1, 0]));
    let (s, report) = read(&file_with(&[
        (CHUNK_TILESET, ts),
        (CHUNK_LAYER, tilemap_layer_chunk()),
        (CHUNK_CEL, cel),
    ]))
    .unwrap();
    assert!(s.cels.is_empty());
    assert!(
        report.warnings.iter().any(|w| w.contains("16 bits")),
        "{:?}",
        report.warnings
    );
}

mod random {
    use proptest::prelude::*;

    use super::*;

    #[derive(Debug, Clone)]
    enum Op {
        Draw(i32, i32, u8, usize, usize),
        Layer(bool, usize),
        Move(usize, usize, u8),
        Frame(usize, bool),
        Opacity(usize, u8),
        Hide(usize),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            4 => (-3..12i32, -3..10i32, 1..30u8, 0..6usize, 0..6usize)
                .prop_map(|(x, y, c, l, f)| Op::Draw(x, y, c, l, f)),
            1 => (any::<bool>(), 0..6usize).prop_map(|(g, l)| Op::Layer(g, l)),
            1 => (0..6usize, 0..6usize, 0..3u8).prop_map(|(a, b, h)| Op::Move(a, b, h)),
            1 => (0..6usize, any::<bool>()).prop_map(|(f, l)| Op::Frame(f, l)),
            1 => (0..6usize, any::<u8>()).prop_map(|(l, o)| Op::Opacity(l, o)),
            1 => (0..6usize).prop_map(Op::Hide),
        ]
    }

    fn build(mode: ColorMode, ops_list: &[Op]) -> Sprite {
        let mut s = Sprite::new(10, 8, mode);
        for o in ops_list {
            let layer = |i: usize| s.layers[i % s.layers.len()].id;
            let frame = |i: usize| i % s.frames.len();
            let cmd = match *o {
                Op::Draw(x, y, c, l, f) => Some(Command::DrawPixels {
                    layer: layer(l),
                    frame: frame(f),
                    pixels: vec![(
                        x,
                        y,
                        match mode {
                            ColorMode::Rgba | ColorMode::Grayscale => {
                                PixelValue::Rgba(Rgba::new(c, c * 3, 255 - c, c * 8))
                            }
                            ColorMode::Indexed { .. } => PixelValue::Index(c),
                        },
                    )],
                }),
                Op::Layer(group, l) => Some(ops::new_layer(
                    &s,
                    Some(layer(l)),
                    if group {
                        LayerKind::Group
                    } else {
                        LayerKind::Image
                    },
                )),
                Op::Move(a, b, how) => {
                    let t = layer(b);
                    let drop = match how {
                        0 => LayerDrop::Above(t),
                        1 => LayerDrop::Below(t),
                        _ => LayerDrop::Into(t),
                    };
                    ops::move_layer(&s, layer(a), drop)
                }
                Op::Frame(f, linked) => ops::duplicate_frame(&s, frame(f), linked),
                Op::Opacity(l, o) => Some(Command::SetLayerProps {
                    id: layer(l),
                    props: cs_commands::LayerProps {
                        opacity: Some(o),
                        ..Default::default()
                    },
                }),
                Op::Hide(l) => Some(Command::SetLayerProps {
                    id: layer(l),
                    props: cs_commands::LayerProps {
                        visible: Some(false),
                        ..Default::default()
                    },
                }),
            };
            if let Some(cmd) = cmd {
                let _ = cmd.apply(&mut s);
            }
        }
        s
    }

    proptest! {
        #[test]
        fn random_sprites_roundtrip(ops_list in proptest::collection::vec(op(), 0..30), indexed in any::<bool>()) {
            let mode = if indexed {
                ColorMode::Indexed { transparent_index: 0 }
            } else {
                ColorMode::Rgba
            };
            let s = build(mode, &ops_list);
            assert_equivalent(&s, &roundtrip(&s));
        }
    }
}
