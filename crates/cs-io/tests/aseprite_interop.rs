//! Compatibilidad con archivos hechos por Aseprite 1.3 (generados con `fixtures/make.lua`):
//! capas, grupo, modos de mezcla, opacidades, cels vinculados y movidos, tags y slices. Cada frame
//! que compone ChimpSprite debe coincidir con el PNG RGBA que exportó Aseprite (`frames.lua`), y lo
//! que guarda ChimpSprite debe volver a leerse igual. Tilemaps: `fixtures/tilemaps.lua`.

use cs_core::{AnimDirection, LayerKind, Rgba, Sprite};
use cs_io::{aseprite, import};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn load(name: &str) -> Sprite {
    let bytes = std::fs::read(format!("{FIXTURES}/{name}.aseprite")).unwrap();
    let (sprite, report) = aseprite::read(&bytes).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    sprite
}

/// Diferencia máxima por canal tolerada (redondeos de mezcla).
const TOLERANCE: u8 = 2;

fn assert_matches_aseprite(sprite: &Sprite, name: &str) {
    assert_matches_aseprite_within(sprite, name, TOLERANCE);
}

fn assert_matches_aseprite_within(sprite: &Sprite, name: &str, tolerance: u8) {
    let mut worst = (0u8, String::new());
    let mut off = 0;
    let composed = cs_render::materialize_tilemaps(sprite, 0..sprite.frames.len());
    for frame in 0..sprite.frames.len() {
        let png = std::fs::read(format!("{FIXTURES}/{name}_{frame}.png")).unwrap();
        let reference = import::decode_png(&png).unwrap();
        let ours = cs_render::render_frame(&composed, frame);
        assert_eq!(ours.len(), reference.pixels.len());
        for (i, (a, b)) in ours.iter().zip(&reference.pixels).enumerate() {
            if a.a == 0 && b.a == 0 {
                continue;
            }
            let d = [
                a.r.abs_diff(b.r),
                a.g.abs_diff(b.g),
                a.b.abs_diff(b.b),
                a.a.abs_diff(b.a),
            ]
            .into_iter()
            .max()
            .unwrap();
            if d > tolerance {
                off += 1;
            }
            if d > worst.0 {
                let (x, y) = (i as u32 % sprite.width, i as u32 / sprite.width);
                worst = (
                    d,
                    format!("frame {frame} ({x}, {y}): ours {a:?}, Aseprite {b:?}"),
                );
            }
        }
    }
    assert!(
        off == 0,
        "{name}: {off} pixel(s) differ by more than {tolerance}; worst {} at {}",
        worst.0,
        worst.1
    );
}

#[test]
fn rgb_file_renders_like_aseprite() {
    let s = load("ase_rgb");
    assert_eq!((s.width, s.height, s.frames.len()), (32, 24, 4));
    let durations: Vec<u16> = s.frames.iter().map(|f| f.duration_ms).collect();
    assert_eq!(durations, [100, 150, 200, 50]);
    let group = s.layers.iter().find(|l| l.name == "FX").unwrap();
    assert_eq!(group.kind, LayerKind::Group);
    assert_eq!(group.opacity, 255, "Aseprite stores 0 for groups");
    assert_eq!(s.tags[0].name, "walk");
    assert_eq!(s.tags[0].direction, AnimDirection::PingPong);
    assert_eq!(s.tags[0].color, Rgba::rgb(200, 50, 50));
    let hitbox = s.slices.iter().find(|s| s.name == "hitbox").unwrap();
    let panel = s.slices.iter().find(|s| s.name == "panel").unwrap();
    assert_eq!(hitbox.text, "damage=3");
    assert_eq!(hitbox.keys[0].bounds, (6, 5, 20, 14));
    assert_eq!(hitbox.keys[0].pivot, Some((10, 14)));
    assert_eq!(panel.keys[0].center, Some((3, 3, 6, 6)));
    let base = s.layers[0].id;
    assert_eq!(
        s.cel(base, 0).unwrap().image,
        s.cel(base, 2).unwrap().image,
        "linked cels"
    );
    assert_matches_aseprite(&s, "ase_rgb");
}

#[test]
fn indexed_file_renders_like_aseprite() {
    let s = load("ase_indexed");
    assert!(matches!(s.color_mode, cs_core::ColorMode::Indexed { .. }));
    assert_matches_aseprite(&s, "ase_indexed");
}

#[test]
fn every_blend_mode_matches_aseprite_over_partial_alpha() {
    let s = load("blends");
    assert_eq!(s.frames.len(), 9);
    // Con alfas muy bajos (~50) los redondeos enteros de Aseprite se notan un poco más.
    assert_matches_aseprite_within(&s, "blends", 4);
}

#[test]
fn saving_keeps_everything_aseprite_wrote() {
    for name in ["ase_rgb", "ase_indexed", "blends"] {
        let s = load(name);
        let (back, report) = aseprite::read(&aseprite::write(&s)).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(back.layers, s.layers);
        assert_eq!(back.tags, s.tags);
        assert_eq!(back.slices, s.slices);
        assert_eq!(back.frames, s.frames);
        for f in 0..s.frames.len() {
            assert_eq!(
                cs_render::render_frame(&back, f),
                cs_render::render_frame(&s, f)
            );
        }
    }
}

#[test]
fn tilemap_file_renders_like_aseprite() {
    let s = load("tilemap_rgba");
    let maps: Vec<_> = s
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Tilemap)
        .collect();
    assert_eq!(maps.len(), 2);
    assert!(maps.iter().all(|l| l.tileset == Some(0)), "shared tileset");
    assert_eq!(s.tilesets.len(), 1);
    let ts = &s.tilesets[0];
    assert_eq!(
        (ts.name.as_str(), ts.tile_size, ts.tiles.len()),
        ("Terrain", (8, 8), 4)
    );
    assert!(ts.user_data.is_some(), "tileset user data");
    assert!(
        ts.tile_user_data.get(2).is_some_and(Option::is_some),
        "tile 2 user data: {:?}",
        ts.tile_user_data
    );
    let ground = maps[0].id;
    let cel = s.cel(ground, 0).unwrap();
    assert_eq!(cel.image, s.cel(ground, 1).unwrap().image, "linked cels");
    let grid = s.tilemap(cel.image).unwrap();
    assert!(grid.width >= 4 && grid.height >= 2);
    // El generador Lua pone en la celda (x, y) de las 2 primeras filas combo = y * 4 + x, con
    // flip X = bit 0, Y = bit 1, D = bit 2 de combo, e índice 1 + combo % 3.
    for y in 0..2u32 {
        for x in 0..4u32 {
            let combo = y * 4 + x;
            let t = grid.get(x, y).unwrap();
            assert_eq!(
                (t.index(), t.flip_x(), t.flip_y(), t.flip_d()),
                (
                    1 + combo % 3,
                    combo & 1 != 0,
                    combo & 2 != 0,
                    combo & 4 != 0
                ),
                "cell ({x}, {y})"
            );
        }
    }
    assert_matches_aseprite(&s, "tilemap_rgba");
}

#[test]
fn indexed_tilemap_file_renders_like_aseprite() {
    let s = load("tilemap_indexed");
    assert!(matches!(s.color_mode, cs_core::ColorMode::Indexed { .. }));
    assert!(s.layers.iter().any(|l| l.kind == LayerKind::Tilemap));
    assert_matches_aseprite(&s, "tilemap_indexed");
}

#[test]
fn saving_tilemaps_keeps_everything_aseprite_wrote() {
    for name in ["tilemap_rgba", "tilemap_indexed"] {
        let s = load(name);
        let (back, report) = aseprite::read(&aseprite::write(&s)).unwrap();
        assert!(report.warnings.is_empty(), "{name}: {:?}", report.warnings);
        assert_eq!(back.layers, s.layers, "{name}");
        assert_eq!(back.tilesets, s.tilesets, "{name}");
        assert_eq!(back.frames, s.frames, "{name}");
        for layer in &s.layers {
            for f in 0..s.frames.len() {
                let (a, b) = (s.cel(layer.id, f), back.cel(layer.id, f));
                assert_eq!(
                    a.map(|c| (c.position, c.opacity)),
                    b.map(|c| (c.position, c.opacity)),
                    "{name} {} frame {f}",
                    layer.name
                );
                if let (Some(a), Some(b)) = (a, b) {
                    assert_eq!(s.tilemap(a.image), back.tilemap(b.image));
                    assert_eq!(s.image(a.image), back.image(b.image));
                }
            }
        }
        assert_matches_aseprite(&back, name);
    }
}

/// Lo que escribe ChimpSprite, abierto por el Aseprite real, se ve igual que el original. Necesita
/// Aseprite: `cargo test -p cs-io --test aseprite_interop -- --ignored` (ruta en `ASEPRITE`, o la
/// de Steam por defecto).
#[test]
#[ignore = "needs Aseprite; set ASEPRITE to its executable"]
fn aseprite_opens_the_tilemaps_we_write() {
    let exe = std::env::var("ASEPRITE").unwrap_or_else(|_| {
        r"C:\Program Files (x86)\Steam\steamapps\common\Aseprite\Aseprite.exe".into()
    });
    let dir = std::env::temp_dir().join("chimpsprite-tilemaps");
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["tilemap_rgba", "tilemap_indexed"] {
        let s = load(name);
        let file = dir.join(format!("{name}.aseprite"));
        std::fs::write(&file, aseprite::write(&s)).unwrap();
        let prefix = dir.join(format!("{name}_"));
        let status = std::process::Command::new(&exe)
            .arg("-b")
            .arg("--script-param")
            .arg(format!("file={}", file.display()))
            .arg("--script-param")
            .arg(format!("out={}", prefix.display()))
            .arg("--script")
            .arg(format!("{FIXTURES}/frames.lua"))
            .status()
            .unwrap();
        assert!(status.success(), "{name}: Aseprite failed");
        for f in 0..s.frames.len() {
            let read_png =
                |path: String| import::decode_png(&std::fs::read(path).unwrap()).unwrap();
            let ours = read_png(format!("{}{f}.png", prefix.display()));
            let theirs = read_png(format!("{FIXTURES}/{name}_{f}.png"));
            assert_eq!(ours.pixels, theirs.pixels, "{name} frame {f}");
        }
    }
}
