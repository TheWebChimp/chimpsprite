//! Compatibilidad con archivos hechos por Aseprite 1.3 (generados con `fixtures/make.lua`):
//! capas, grupo, modos de mezcla, opacidades, cels vinculados y movidos, tags y slices. Cada frame
//! que compone ChimpSprite debe coincidir con el PNG RGBA que exportó Aseprite (`frames.lua`), y lo
//! que guarda ChimpSprite debe volver a leerse igual.

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
    for frame in 0..sprite.frames.len() {
        let png = std::fs::read(format!("{FIXTURES}/{name}_{frame}.png")).unwrap();
        let reference = import::decode_png(&png).unwrap();
        let ours = cs_render::render_frame(sprite, frame);
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
