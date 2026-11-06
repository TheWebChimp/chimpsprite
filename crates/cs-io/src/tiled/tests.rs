use std::collections::HashMap;
use std::sync::Arc;

use cs_core::*;

use super::tmx::{Element, parse_xml};
use super::*;

fn rgb(r: u8, g: u8, b: u8) -> PixelValue {
    PixelValue::Rgba(Rgba::rgb(r, g, b))
}

/// Tile de `size` con un color propio por tile y un píxel marcado en (0, 0) (para ver flips).
fn tile_image(size: u32, seed: u8) -> Arc<Image> {
    let mut img = Image::new(size, size, ColorMode::Rgba);
    for y in 0..size {
        for x in 0..size {
            img.set(x, y, rgb(seed, x as u8 * 10, y as u8 * 10))
                .unwrap();
        }
    }
    img.set(0, 0, rgb(255, 255, 255)).unwrap();
    Arc::new(img)
}

fn tileset(name: &str, size: u32, tiles: u8) -> Tileset {
    let mut ts = Tileset::new(name, (size, size), ColorMode::Rgba);
    for i in 0..tiles {
        ts.tiles.push(tile_image(size, 40 + i * 40));
    }
    ts
}

/// Agrega una capa tilemap con `grid` en `position`.
fn add_tilemap(
    s: &mut Sprite,
    name: &str,
    ts: u32,
    grid: TileGrid,
    position: (i32, i32),
) -> LayerId {
    let id = s.next_layer_id();
    let mut layer = Layer::new_image(id, name);
    layer.kind = LayerKind::Tilemap;
    layer.tileset = Some(ts);
    s.layers.push(layer);
    let image = s.next_image_id();
    s.tilemaps.insert(image, Arc::new(grid));
    s.cels.insert(
        (id, 0),
        Cel {
            position,
            opacity: 255,
            image,
        },
    );
    id
}

/// 32×16 con tiles de 8: tileset "ground" (3 tiles) y "deco" (2 tiles). La capa "Ground" tiene
/// el tile 1 con los 8 flips en la primera fila y el 3 en (0, 1); "Deco" el tile 2 en (3, 1).
fn sprite() -> Sprite {
    let mut s = Sprite::new(32, 16, ColorMode::Rgba);
    s.layers.clear();
    s.tilesets.push(tileset("ground", 8, 3));
    s.tilesets.push(tileset("deco", 8, 2));
    let mut g = TileGrid::new(4, 2);
    for i in 0..4u32 {
        g.set(i, 0, Tile::new(1, i & 1 != 0, i & 2 != 0, false));
    }
    g.set(0, 1, Tile::new(3, false, false, true));
    g.set(1, 1, Tile::new(1, true, true, true));
    add_tilemap(&mut s, "Ground", 0, g, (0, 0));
    let mut d = TileGrid::new(4, 2);
    d.set(3, 1, Tile::new(2, true, false, false));
    add_tilemap(&mut s, "Deco", 1, d, (0, 0));
    s
}

fn export_of(s: &Sprite, options: TiledOptions) -> TiledExport {
    let doc = Document::with_sprite("hero", s.clone());
    export(&doc, &doc.artboards[0].sprite, "hero", &options).unwrap()
}

fn file<'a>(out: &'a TiledExport, name: &str) -> &'a [u8] {
    &out.files
        .iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("{name} not in {:?}", names(out)))
        .1
}

fn names(out: &TiledExport) -> Vec<&str> {
    out.files.iter().map(|(n, _)| n.as_str()).collect()
}

fn text(out: &TiledExport, name: &str) -> String {
    String::from_utf8(file(out, name).to_vec()).unwrap()
}

fn children<'a>(el: &'a Element, name: &str) -> Vec<&'a Element> {
    el.children.iter().filter(|c| c.name == name).collect()
}

fn tiles_of(layer: &MapLayer) -> &[u32] {
    match &layer.content {
        LayerContent::Tiles { gids, .. } => gids,
        other => panic!("not tiles: {other:?}"),
    }
}

const H: u32 = 0x8000_0000;
const V: u32 = 0x4000_0000;
const D: u32 = 0x2000_0000;

#[test]
fn tile_flags_are_tileds_gid_flags() {
    assert_eq!(Tile::FLIP_X, H);
    assert_eq!(Tile::FLIP_Y, V);
    assert_eq!(Tile::FLIP_D, D);
    assert_eq!(FLIP_FLAGS, H | V | D);
}

#[test]
fn tmx_has_gids_with_flips_and_firstgid_per_tileset() {
    let out = export_of(&sprite(), TiledOptions::default());
    assert_eq!(
        names(&out),
        [
            "hero.tmx",
            "ground.png",
            "deco.png",
            "ground.tsx",
            "deco.tsx"
        ]
    );
    // El XML tal cual (atributos del formato).
    let root = parse_xml(&text(&out, "hero.tmx")).unwrap();
    assert_eq!(root.name, "map");
    for (k, v) in [
        ("version", "1.10"),
        ("orientation", "orthogonal"),
        ("renderorder", "right-down"),
        ("width", "4"),
        ("height", "2"),
        ("tilewidth", "8"),
        ("tileheight", "8"),
        ("infinite", "0"),
    ] {
        assert_eq!(root.attr(k), Some(v), "{k}");
    }
    let tilesets = children(&root, "tileset");
    assert_eq!(tilesets[0].attr("firstgid"), Some("1"));
    assert_eq!(tilesets[0].attr("source"), Some("ground.tsx"));
    assert_eq!(tilesets[1].attr("firstgid"), Some("4"), "after 3 tiles");
    let layers = children(&root, "layer");
    assert_eq!(layers[0].attr("name"), Some("Ground"));
    assert_eq!(layers[0].attr("width"), Some("4"));
    let data = layers[0].child("data").unwrap();
    assert_eq!(data.attr("encoding"), Some("csv"));
    let csv: Vec<u32> = data
        .text
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    assert_eq!(
        csv,
        [1, 1 | H, 1 | V, 1 | H | V, 3 | D, 1 | H | V | D, 0, 0]
    );
    let deco = layers[1].child("data").unwrap();
    assert!(
        deco.text.trim().ends_with(&(5 | H).to_string()),
        "tile 2 of deco"
    );
    // Y por el lector.
    let map = read_tmx(&text(&out, "hero.tmx")).unwrap();
    assert_eq!(tiles_of(&map.layers[1])[7], 5 | H);
    // Las propiedades del mapa.
    assert_eq!(
        map.properties[0],
        Property::new("frame", PropertyValue::Int(1))
    );
    assert_eq!(
        map.properties[1],
        Property::new("duration_ms", PropertyValue::Int(100))
    );
}

#[test]
fn tsx_and_tileset_image_are_a_grid() {
    let out = export_of(&sprite(), TiledOptions::default());
    let ts = parse_xml(&text(&out, "ground.tsx")).unwrap();
    assert_eq!(ts.name, "tileset");
    for (k, v) in [
        ("name", "ground"),
        ("tilewidth", "8"),
        ("tileheight", "8"),
        ("tilecount", "3"),
        ("columns", "2"),
    ] {
        assert_eq!(ts.attr(k), Some(v), "{k}");
    }
    assert_eq!(ts.attr("margin"), None, "0 by default");
    let image = ts.child("image").unwrap();
    assert_eq!(image.attr("source"), Some("ground.png"));
    assert_eq!(
        (image.attr("width"), image.attr("height")),
        (Some("16"), Some("16"))
    );
    let png = crate::import::decode_png(file(&out, "ground.png")).unwrap();
    assert_eq!((png.width, png.height), (16, 16));
    // El tile 2 (id local 1) está a la derecha del 1; el 3 abajo; el hueco transparente.
    assert_eq!(png.pixels[9], Rgba::rgb(80, 10, 0));
    assert_eq!(png.pixels[8 * 16 + 1], Rgba::rgb(120, 10, 0));
    assert_eq!(png.pixels[8 * 16 + 9], Rgba::TRANSPARENT);
    assert_eq!(read_tsx(&text(&out, "ground.tsx")).unwrap().tile_count, 3);
}

#[test]
fn indexed_tilesets_resolve_the_palette_and_flips_become_transformations() {
    let mode = ColorMode::Indexed {
        transparent_index: 0,
    };
    let mut s = Sprite::new(8, 8, mode);
    s.layers.clear();
    let mut ts = Tileset::new("pal", (4, 4), mode);
    let mut img = Image::new(4, 4, mode);
    img.set(1, 0, PixelValue::Index(3)).unwrap();
    ts.tiles.push(Arc::new(img));
    ts.allowed_flips = AllowedFlips {
        x: true,
        y: false,
        d: true,
    };
    s.tilesets.push(ts);
    let mut g = TileGrid::new(1, 1);
    g.set(0, 0, Tile(1));
    add_tilemap(&mut s, "L", 0, g, (0, 0));
    let out = export_of(&s, TiledOptions::default());
    let png = crate::import::decode_png(file(&out, "pal.png")).unwrap();
    assert_eq!(png.pixels[1], s.palette.get(3).unwrap());
    assert_eq!(png.pixels[0], Rgba::TRANSPARENT);
    let tsx = parse_xml(&text(&out, "pal.tsx")).unwrap();
    let t = tsx.child("transformations").unwrap();
    assert_eq!(
        (t.attr("hflip"), t.attr("vflip"), t.attr("rotate")),
        (Some("1"), Some("0"), Some("1"))
    );
}

#[test]
fn every_encoding_round_trips_in_tmx_and_tmj() {
    let s = sprite();
    let expected = read_tmx(&text(&export_of(&s, TiledOptions::default()), "hero.tmx")).unwrap();
    for encoding in Encoding::ALL {
        for format in [MapFormat::Tmx, MapFormat::Tmj] {
            let options = TiledOptions {
                encoding,
                format,
                ..Default::default()
            };
            let out = export_of(&s, options);
            let name = format!("hero.{}", format.map_extension());
            let map = match format {
                MapFormat::Tmx => read_tmx(&text(&out, &name)).unwrap(),
                MapFormat::Tmj => read_tmj(&text(&out, &name)).unwrap(),
            };
            for (a, b) in map.layers.iter().zip(&expected.layers) {
                assert_eq!(tiles_of(a), tiles_of(b), "{encoding:?} {format:?}");
            }
        }
    }
}

#[test]
fn base64_data_is_little_endian_u32_and_compresses_with_zlib_or_gzip() {
    let s = sprite();
    let gids = [1, 1 | H, 1 | V, 1 | H | V, 3 | D, 1 | H | V | D, 0, 0];
    let raw: Vec<u8> = gids.iter().flat_map(|g: &u32| g.to_le_bytes()).collect();
    for (encoding, compression) in [
        (Encoding::Base64, None),
        (Encoding::Base64Zlib, Some("zlib")),
        (Encoding::Base64Gzip, Some("gzip")),
    ] {
        let options = TiledOptions {
            encoding,
            ..Default::default()
        };
        let root = parse_xml(&text(&export_of(&s, options), "hero.tmx")).unwrap();
        let data = children(&root, "layer")[0].child("data").unwrap();
        assert_eq!(data.attr("encoding"), Some("base64"));
        assert_eq!(data.attr("compression"), compression);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.text.trim())
            .unwrap();
        let mut plain = Vec::new();
        match compression {
            Some("zlib") => {
                flate2::read::ZlibDecoder::new(&bytes[..])
                    .read_to_end(&mut plain)
                    .unwrap();
            }
            Some(_) => {
                flate2::read::GzDecoder::new(&bytes[..])
                    .read_to_end(&mut plain)
                    .unwrap();
            }
            None => plain = bytes,
        }
        assert_eq!(plain, raw, "{encoding:?}");
    }
}

#[test]
fn tmj_has_the_json_fields() {
    let options = TiledOptions {
        format: MapFormat::Tmj,
        ..Default::default()
    };
    let out = export_of(&sprite(), options);
    assert_eq!(
        names(&out),
        [
            "hero.tmj",
            "ground.png",
            "deco.png",
            "ground.tsj",
            "deco.tsj"
        ]
    );
    let v: serde_json::Value = serde_json::from_slice(file(&out, "hero.tmj")).unwrap();
    assert_eq!(v["type"], "map");
    assert_eq!(v["version"], "1.10");
    assert_eq!(
        (v["width"].as_u64(), v["tilewidth"].as_u64()),
        (Some(4), Some(8))
    );
    assert_eq!(v["infinite"], false);
    assert_eq!(v["tilesets"][1]["firstgid"], 4);
    assert_eq!(v["tilesets"][1]["source"], "deco.tsj");
    let layer = &v["layers"][0];
    assert_eq!(layer["type"], "tilelayer");
    assert_eq!(layer["encoding"], "csv");
    assert_eq!(layer["data"][1].as_u64(), Some((1 | H) as u64));
    assert_eq!(layer["data"][5].as_u64(), Some((1 | H | V | D) as u64));
    let ts: serde_json::Value = serde_json::from_slice(file(&out, "ground.tsj")).unwrap();
    assert_eq!(ts["type"], "tileset");
    assert_eq!(
        (ts["tilecount"].as_u64(), ts["columns"].as_u64()),
        (Some(3), Some(2))
    );
    assert_eq!(ts["image"], "ground.png");
    assert_eq!(ts["imagewidth"], 16);
}

#[test]
fn embedded_tilesets_go_inside_the_map() {
    for format in [MapFormat::Tmx, MapFormat::Tmj] {
        let options = TiledOptions {
            format,
            external_tilesets: false,
            ..Default::default()
        };
        let out = export_of(&sprite(), options);
        assert_eq!(out.files.len(), 3, "map + two PNGs: {:?}", names(&out));
        let name = format!("hero.{}", format.map_extension());
        let map = match format {
            MapFormat::Tmx => {
                let root = parse_xml(&text(&out, &name)).unwrap();
                let ts = children(&root, "tileset")[1];
                assert_eq!(ts.attr("source"), None);
                assert_eq!(ts.attr("name"), Some("deco"));
                assert_eq!(ts.child("image").unwrap().attr("source"), Some("deco.png"));
                read_tmx(&text(&out, &name)).unwrap()
            }
            MapFormat::Tmj => read_tmj(&text(&out, &name)).unwrap(),
        };
        assert_eq!(map.tilesets[1].source, None);
        assert_eq!(map.tilesets[1].first_gid, 4);
        assert_eq!(map.tilesets[1].tileset.tile_count, 2);
        assert_eq!(map.tilesets[1].tileset.image.as_ref().unwrap().width, 16);
    }
}

#[test]
fn map_size_rounds_up_and_unaligned_cels_use_the_offset() {
    let mut s = Sprite::new(20, 10, ColorMode::Rgba);
    s.layers.clear();
    s.tilesets.push(tileset("t", 8, 1));
    let mut g = TileGrid::new(2, 1);
    g.set(0, 0, Tile(1));
    g.set(1, 0, Tile(1));
    // En (12, 4): celda (1, 0) y 4 px de resto en cada eje; el segundo tile cae en la columna 2.
    add_tilemap(&mut s, "L", 0, g.clone(), (12, 4));
    // Uno fuera del lienzo: se descarta con aviso.
    add_tilemap(&mut s, "Out", 0, g, (-16, 0));
    let out = export_of(&s, TiledOptions::default());
    let map = read_tmx(&text(&out, "hero.tmx")).unwrap();
    assert_eq!((map.width, map.height), (3, 2));
    assert_eq!(map.layers[0].offset, (4, 4));
    assert_eq!(tiles_of(&map.layers[0]), [0, 1, 1, 0, 0, 0]);
    assert_eq!(map.layers[1].offset, (0, 0));
    assert_eq!(tiles_of(&map.layers[1]), [0; 6]);
    assert!(out.warnings[0].contains("'Out' has tiles outside the canvas"));
}

#[test]
fn groups_image_layers_and_layer_attributes() {
    let mut s = sprite();
    // Un grupo arriba con una capa de imagen oculta, a media opacidad y multiply.
    let group = s.next_layer_id();
    let mut g = Layer::new_image(group, "Props & <stuff>");
    g.kind = LayerKind::Group;
    s.layers.push(g);
    let image_layer = s.next_layer_id();
    let mut l = Layer::new_image(image_layer, "Sky");
    l.parent = Some(group);
    l.visible = false;
    l.opacity = 128;
    l.blend = BlendMode::Multiply;
    s.layers.push(l);
    let mut img = Image::new(6, 3, ColorMode::Rgba);
    img.set(0, 0, rgb(1, 2, 3)).unwrap();
    let id = s.next_image_id();
    s.images.insert(id, Arc::new(img));
    s.cels.insert(
        (image_layer, 0),
        Cel {
            position: (30, 2),
            opacity: 255,
            image: id,
        },
    );
    // Una de referencia, que no sale.
    let reference = s.next_layer_id();
    let mut r = Layer::new_image(reference, "Ref");
    r.reference = true;
    s.layers.push(r);
    s.layers[0].opacity = 51;
    let out = export_of(&s, TiledOptions::default());
    let root = parse_xml(&text(&out, "hero.tmx")).unwrap();
    assert_eq!(children(&root, "layer")[0].attr("opacity"), Some("0.2"));
    let groups = children(&root, "group");
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].attr("name"),
        Some("Props & <stuff>"),
        "escaped and back"
    );
    assert_eq!(groups[0].attr("id"), Some(group.0.to_string().as_str()));
    let il = groups[0].child("imagelayer").unwrap();
    assert_eq!(il.attr("name"), Some("Sky"));
    assert_eq!(il.attr("visible"), Some("0"));
    assert_eq!(il.attr("opacity"), Some("0.502"));
    assert_eq!(il.attr("mode"), Some("multiply"));
    // Recortada al lienzo: de x 30 a 32.
    assert_eq!(
        (il.attr("offsetx"), il.attr("offsety")),
        (Some("30"), Some("2"))
    );
    let image = il.child("image").unwrap();
    let source = format!("hero_{}_Sky.png", image_layer.0);
    assert_eq!(image.attr("source"), Some(source.as_str()));
    let png = crate::import::decode_png(file(&out, &source)).unwrap();
    assert_eq!((png.width, png.height), (2, 3));
    assert_eq!(png.pixels[0], Rgba::rgb(1, 2, 3));
    assert!(
        !text(&out, "hero.tmx").contains("Ref"),
        "reference layers stay out"
    );
    let map = read_tmx(&text(&out, "hero.tmx")).unwrap();
    assert_eq!(map.next_layer_id, reference.0 + 1);
}

#[test]
fn slices_become_rectangle_objects() {
    let mut s = sprite();
    s.slices.push(Slice {
        name: "door".into(),
        color: Rgba::rgb(255, 0, 0),
        text: "exit".into(),
        keys: vec![SliceKey {
            frame: 0,
            bounds: (8, 0, 8, 16),
            center: Some((1, 1, 6, 14)),
            pivot: Some((4, 16)),
        }],
    });
    for format in [MapFormat::Tmx, MapFormat::Tmj] {
        let options = TiledOptions {
            format,
            ..Default::default()
        };
        let out = export_of(&s, options);
        let name = format!("hero.{}", format.map_extension());
        let map = match format {
            MapFormat::Tmx => read_tmx(&text(&out, &name)).unwrap(),
            MapFormat::Tmj => read_tmj(&text(&out, &name)).unwrap(),
        };
        let top = map.layers.last().unwrap();
        assert_eq!(top.name, "Slices");
        let LayerContent::Objects(objects) = &top.content else {
            panic!("objects");
        };
        let o = &objects[0];
        assert_eq!(
            (o.name.as_str(), o.x, o.y, o.width, o.height),
            ("door", 8.0, 0.0, 8.0, 16.0)
        );
        assert!(o.rectangle);
        let prop = |n: &str| {
            o.properties
                .iter()
                .find(|p| p.name == n)
                .unwrap()
                .value
                .clone()
        };
        assert_eq!(prop("color"), PropertyValue::Color(Rgba::rgb(255, 0, 0)));
        assert_eq!(prop("text"), PropertyValue::Str("exit".into()));
        assert_eq!(prop("pivot_y"), PropertyValue::Int(16));
        assert_eq!(prop("center_w"), PropertyValue::Int(6));
        assert_eq!(map.next_object_id, 2);
    }
}

#[test]
fn all_frames_make_one_map_each_with_shared_tilesets() {
    let mut s = sprite();
    s.frames.push(Frame { duration_ms: 250 });
    // El frame 2 de Ground: solo el tile 2.
    let mut g = TileGrid::new(1, 1);
    g.set(0, 0, Tile(2));
    let image = s.next_image_id();
    s.tilemaps.insert(image, Arc::new(g));
    s.cels.insert(
        (LayerId(1), 1),
        Cel {
            position: (8, 8),
            opacity: 255,
            image,
        },
    );
    let options = TiledOptions {
        frames: TiledFrames::All,
        ..Default::default()
    };
    let out = export_of(&s, options);
    assert_eq!(
        names(&out),
        [
            "hero_001.tmx",
            "hero_002.tmx",
            "ground.png",
            "deco.png",
            "ground.tsx",
            "deco.tsx"
        ]
    );
    let second = read_tmx(&text(&out, "hero_002.tmx")).unwrap();
    assert_eq!(tiles_of(&second.layers[0]), [0, 0, 0, 0, 0, 2, 0, 0]);
    assert_eq!(tiles_of(&second.layers[1]), [0; 8], "no cel in frame 2");
    assert_eq!(second.properties[1].value, PropertyValue::Int(250));
    // `Current` elige el frame.
    let options = TiledOptions {
        frames: TiledFrames::Current(1),
        ..Default::default()
    };
    let one = read_tmx(&text(&export_of(&s, options), "hero.tmx")).unwrap();
    assert_eq!(tiles_of(&one.layers[0])[5], 2);
}

#[test]
fn layers_with_other_tile_sizes_become_images_and_missing_tiles_are_empty() {
    let mut s = sprite();
    s.tilesets.push(tileset("big", 16, 1));
    let mut g = TileGrid::new(1, 1);
    g.set(0, 0, Tile(1));
    let big = add_tilemap(&mut s, "Big", 2, g, (0, 0));
    // Un tile que el tileset no tiene (un tileset externo de Aseprite sin cargar).
    let mut grid = (*s.tilemaps[&ImageId(1)]).clone();
    grid.set(3, 1, Tile(9));
    s.tilemaps.insert(ImageId(1), Arc::new(grid));
    let out = export_of(&s, TiledOptions::default());
    let map = read_tmx(&text(&out, "hero.tmx")).unwrap();
    assert_eq!(map.tilesets.len(), 2, "the 16×16 tileset is not in the map");
    let LayerContent::Image(Some(image)) = &map.layers[2].content else {
        panic!("image layer: {:?}", map.layers[2]);
    };
    assert_eq!(image.source, format!("hero_{}_Big.png", big.0));
    assert_eq!((image.width, image.height), (16, 16));
    assert_eq!(tiles_of(&map.layers[0])[7], 0);
    assert_eq!(out.warnings.len(), 2, "{:?}", out.warnings);
}

#[test]
fn without_tilemaps_there_is_nothing_to_export() {
    let s = Sprite::new(8, 8, ColorMode::Rgba);
    assert!(!has_tilemaps(&s));
    assert!(has_tilemaps(&sprite()));
    let doc = Document::with_sprite("x", s);
    let err = export(
        &doc,
        &doc.artboards[0].sprite,
        "x",
        &TiledOptions::default(),
    );
    assert!(err.unwrap_err().to_string().contains("no tilemap layers"));
}

#[test]
fn zip_holds_every_file() {
    let out = export_of(&sprite(), TiledOptions::default());
    let zip = out.zip().unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    assert_eq!(archive.len(), out.files.len());
    let mut tmx = String::new();
    archive
        .by_name("hero.tmx")
        .unwrap()
        .read_to_string(&mut tmx)
        .unwrap();
    assert_eq!(tmx, text(&out, "hero.tmx"));
}

/// Importa los archivos exportados (el cargador lee de `out.files`).
fn import_export(out: &TiledExport, map: &str) -> (Sprite, Vec<String>) {
    let files: HashMap<String, Vec<u8>> = out.files.iter().cloned().collect();
    let mut load = |path: &str| files.get(path).cloned();
    import(map, &files[map], &mut load).unwrap()
}

#[test]
fn exported_maps_import_back_as_tilemaps() {
    let mut s = sprite();
    let group = s.next_layer_id();
    let mut g = Layer::new_image(group, "Group");
    g.kind = LayerKind::Group;
    g.visible = false;
    s.layers.push(g);
    let mut grid = TileGrid::new(1, 1);
    grid.set(0, 0, Tile::new(2, false, true, false));
    let inner = add_tilemap(&mut s, "Inner", 0, grid, (8, 0));
    s.layer_mut(inner).unwrap().parent = Some(group);
    s.slices.push(Slice {
        name: "hit".into(),
        color: Rgba::rgb(0, 255, 0),
        text: String::new(),
        keys: vec![SliceKey {
            frame: 0,
            bounds: (1, 2, 3, 4),
            center: None,
            pivot: Some((1, 1)),
        }],
    });
    for format in [MapFormat::Tmx, MapFormat::Tmj] {
        for external_tilesets in [true, false] {
            let options = TiledOptions {
                format,
                external_tilesets,
                encoding: Encoding::Base64Zlib,
                ..Default::default()
            };
            let out = export_of(&s, options);
            let (back, warnings) = import_export(&out, &format!("hero.{}", format.map_extension()));
            assert!(warnings.is_empty(), "{warnings:?}");
            assert_eq!((back.width, back.height), (32, 16));
            assert_eq!(back.grid.size, (8, 8));
            assert_eq!(back.tilesets.len(), 2);
            assert_eq!(back.tilesets[0].name, "ground");
            assert_eq!(back.tilesets[0].tiles.len(), 4, "empty + 3");
            assert_eq!(back.tilesets[0].tiles[2], s.tilesets[0].tiles[2], "pixels");
            let names: Vec<&str> = back.layers.iter().map(|l| l.name.as_str()).collect();
            assert_eq!(names, ["Ground", "Deco", "Group", "Inner"]);
            let ground = &back.layers[0];
            assert_eq!((ground.kind, ground.tileset), (LayerKind::Tilemap, Some(0)));
            let cel = back.cel(ground.id, 0).unwrap();
            assert_eq!(**back.tilemap(cel.image).unwrap(), *s.tilemaps[&ImageId(1)]);
            assert_eq!(back.layers[1].tileset, Some(1));
            let deco = back
                .tilemap(back.cel(back.layers[1].id, 0).unwrap().image)
                .unwrap();
            assert_eq!(deco.get(3, 1), Some(Tile::new(2, true, false, false)));
            assert_eq!(back.layers[2].kind, LayerKind::Group);
            assert!(!back.layers[2].visible);
            assert_eq!(back.layers[3].parent, Some(back.layers[2].id));
            let inner = back
                .tilemap(back.cel(back.layers[3].id, 0).unwrap().image)
                .unwrap();
            assert_eq!(inner.get(1, 0), Some(Tile::new(2, false, true, false)));
            assert_eq!(back.slices[0].name, "hit");
            assert_eq!(back.slices[0].keys[0].bounds, (1, 2, 3, 4));
            assert_eq!(back.slices[0].keys[0].pivot, Some((1, 1)));
            assert_eq!(back.slices[0].color, Rgba::rgb(0, 255, 0));
            assert!(back.is_valid_tree());
            // Se ve igual que el original.
            let doc = Document::with_sprite("a", s.clone());
            let a = cs_render::render_frame(
                &cs_render::materialize(&doc, &doc.artboards[0].sprite, 0..1),
                0,
            );
            let doc = Document::with_sprite("b", back.clone());
            let b = cs_render::render_frame(
                &cs_render::materialize(&doc, &doc.artboards[0].sprite, 0..1),
                0,
            );
            assert_eq!(a, b, "{format:?} external {external_tilesets}");
        }
    }
}

/// PNG de `w`×`h` con `paint(x, y)`.
fn png(w: u32, h: u32, paint: impl Fn(u32, u32) -> Rgba) -> Vec<u8> {
    let mut b = crate::export::Bitmap::new(w, h);
    for y in 0..h {
        for x in 0..w {
            b.pixels[(y * w + x) as usize] = paint(x, y);
        }
    }
    b.to_png().unwrap()
}

#[test]
fn import_handles_margin_spacing_trans_and_multi_tileset_layers() {
    // Tiles de 2×2 con margin 1 y spacing 1, 2 columnas: el tile k tiene color (k*50, 0, 0);
    // el fondo es magenta (`trans`).
    let sheet = png(7, 4, |x, y| {
        let (cx, cy) = (x.wrapping_sub(1) % 3, y.wrapping_sub(1) % 3);
        if x == 0 || y == 0 || cx == 2 || cy == 2 || (x, y) == (2, 2) {
            return Rgba::rgb(255, 0, 255);
        }
        let k = (x - 1) / 3;
        Rgba::rgb(50 + k as u8 * 50, 0, 0)
    });
    let tmx = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<map version="1.10" orientation="orthogonal" renderorder="right-down" width="3" height="1" tilewidth="2" tileheight="2" infinite="0" nextlayerid="3" nextobjectid="1">
 <tileset firstgid="1" name="a" tilewidth="2" tileheight="2" spacing="1" margin="1" tilecount="2" columns="2">
  <image source="art/sheet.png" trans="ff00ff" width="7" height="4"/>
 </tileset>
 <tileset firstgid="3" source="ext/b.tsx"/>
 <layer id="1" name="Mixed" width="3" height="1" opacity="0.5">
  <data encoding="csv">2,{},0</data>
 </layer>
 <layer id="2" name="Old" width="3" height="1">
  <data><tile gid="1"/><tile/><tile gid="99"/></data>
 </layer>
 <objectgroup id="3" name="Things"><object id="1" x="1" y="1"><point/></object></objectgroup>
</map>"#,
        3 | H
    );
    let tsx = r#"<tileset name="b" tilewidth="2" tileheight="2" tilecount="1" columns="1"><image source="../art/b.png" width="2" height="2"/></tileset>"#;
    let b_png = png(2, 2, |_, _| Rgba::rgb(0, 0, 200));
    let mut load = |path: &str| -> Option<Vec<u8>> {
        match path {
            "maps/art/sheet.png" | "art/sheet.png" => Some(sheet.clone()),
            "ext/b.tsx" => Some(tsx.as_bytes().to_vec()),
            "art/b.png" => Some(b_png.clone()),
            _ => None,
        }
    };
    let (s, warnings) = import("level.tmx", tmx.as_bytes(), &mut load).unwrap();
    assert_eq!((s.width, s.height), (6, 2));
    assert_eq!(s.tilesets.len(), 2);
    let a = &s.tilesets[0];
    assert_eq!(a.tiles.len(), 3);
    assert_eq!(a.tiles[1].get(0, 0), Some(rgb(50, 0, 0)));
    assert_eq!(a.tiles[2].get(1, 1), Some(rgb(100, 0, 0)));
    assert_eq!(s.tilesets[1].tiles[1].get(0, 0), Some(rgb(0, 0, 200)));
    // "Mixed" usa los dos tilesets: dos capas.
    let names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Mixed (a)", "Mixed (b)", "Old"]);
    assert_eq!(s.layers[0].opacity, 128);
    let grid = |i: usize| {
        s.tilemap(s.cel(s.layers[i].id, 0).unwrap().image)
            .unwrap()
            .clone()
    };
    assert_eq!(grid(0).tiles, [Tile(2), Tile(0), Tile(0)]);
    assert_eq!(grid(1).tiles, [Tile(0), Tile(1 | H), Tile(0)]);
    assert_eq!(grid(2).tiles, [Tile(1), Tile(0), Tile(0)]);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("'Old' uses tiles that no tileset has"));
    assert!(warnings[1].contains("only rectangles"));
    // El magenta dentro del tile queda transparente.
    assert_eq!(
        a.tiles[1].get(1, 1),
        Some(PixelValue::Rgba(Rgba::TRANSPARENT))
    );
}

#[test]
fn import_rejects_what_it_cannot_read() {
    let mut none = |_: &str| None;
    let infinite = r#"<map width="2" height="2" tilewidth="8" tileheight="8" infinite="1"></map>"#;
    let err = import("a.tmx", infinite.as_bytes(), &mut none).unwrap_err();
    assert!(err.to_string().contains("infinite"), "{err}");
    let iso =
        r#"<map orientation="isometric" width="2" height="2" tilewidth="8" tileheight="8"></map>"#;
    assert!(import("a.tmx", iso.as_bytes(), &mut none).is_err());
    let external = r#"<map width="1" height="1" tilewidth="8" tileheight="8"><tileset firstgid="1" source="x.tsx"/></map>"#;
    let err = import("a.tmx", external.as_bytes(), &mut none).unwrap_err();
    assert!(err.to_string().contains("'x.tsx' not found"), "{err}");
    let zstd = r#"{"width":1,"height":1,"tilewidth":8,"tileheight":8,"layers":[{"type":"tilelayer","width":1,"height":1,"encoding":"base64","compression":"zstd","data":"AAAA"}]}"#;
    assert!(import("a.tmj", zstd.as_bytes(), &mut none).is_err());
    assert!(import("a.tmx", b"<map", &mut none).is_err());
    // Un mapa sin capas da un sprite con una capa vacía.
    let empty = r#"{"width":2,"height":1,"tilewidth":4,"tileheight":4}"#;
    let (s, _) = import("a.tmj", empty.as_bytes(), &mut none).unwrap();
    assert_eq!((s.width, s.height, s.layers.len()), (8, 4, 1));
    assert!(is_tiled_map("Level.TMX") && is_tiled_map("x.tmj") && !is_tiled_map("x.tsx"));
}

#[test]
fn paths_resolve_from_the_referencing_file() {
    assert_eq!(resolve_path("level.tmx", "a.png"), "a.png");
    assert_eq!(resolve_path("maps/level.tmx", "../art/a.png"), "art/a.png");
    assert_eq!(resolve_path("ext/b.tsx", "./img\\b.png"), "ext/img/b.png");
    assert_eq!(resolve_path("level.tmx", "../a.png"), "../a.png");
    assert_eq!(resolve_path("level.tmx", "C:/x/a.png"), "C:/x/a.png");
}

#[test]
fn colors_and_properties_parse() {
    assert_eq!(parse_color("#ff00ff"), Some(Rgba::rgb(255, 0, 255)));
    assert_eq!(parse_color("80102030"), Some(Rgba::new(16, 32, 48, 128)));
    assert_eq!(color_text(Rgba::new(16, 32, 48, 128)), "#80102030");
    assert_eq!(parse_color("nope"), None);
    let mut map = read_tmx(&text(
        &export_of(&sprite(), TiledOptions::default()),
        "hero.tmx",
    ))
    .unwrap();
    map.properties = vec![
        Property::new("s", PropertyValue::Str("a \"b\"\nc".into())),
        Property::new("f", PropertyValue::Float(1.5)),
        Property::new("b", PropertyValue::Bool(true)),
        Property::new("c", PropertyValue::Color(Rgba::new(1, 2, 3, 4))),
    ];
    assert_eq!(
        read_tmx(&write_tmx(&map)).unwrap().properties,
        map.properties
    );
    assert_eq!(
        read_tmj(&write_tmj(&map)).unwrap().properties,
        map.properties
    );
}
