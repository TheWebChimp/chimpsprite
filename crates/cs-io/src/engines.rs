//! Exportación para motores de juego: Godot (SpriteFrames), Unity (sprites cortados en el .meta)
//! y Bevy (atlas en RON). Todos usan la misma hoja que `export_sheet`.

use std::fmt::Write as _;

use cs_core::{AnimDirection, Sprite};

use crate::export::SheetLayout;

/// Columnas de la hoja para un layout (igual que `export_sheet`).
pub fn sheet_columns(sprite: &Sprite, layout: SheetLayout) -> u32 {
    let n = sprite.frames.len() as u32;
    match layout {
        SheetLayout::Horizontal => n,
        SheetLayout::Vertical => 1,
        SheetLayout::Grid { columns } => columns.clamp(1, n),
    }
}

/// Rectángulo `(x, y, w, h)` de cada frame en la hoja.
pub fn frame_rects(sprite: &Sprite, layout: SheetLayout, scale: u32) -> Vec<(u32, u32, u32, u32)> {
    let scale = scale.max(1);
    let (fw, fh) = (sprite.width * scale, sprite.height * scale);
    let columns = sheet_columns(sprite, layout);
    (0..sprite.frames.len() as u32)
        .map(|f| ((f % columns) * fw, (f / columns) * fh, fw, fh))
        .collect()
}

/// Animación lista para un motor: frames en orden de reproducción (con la dirección del tag ya
/// aplicada) y si se repite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    pub name: String,
    pub frames: Vec<usize>,
    pub durations_ms: Vec<u16>,
    pub looping: bool,
}

/// Una animación por tag; sin tags, una "default" con todos los frames.
pub fn animations(sprite: &Sprite) -> Vec<Animation> {
    let ranges: Vec<(String, usize, usize, AnimDirection, u16)> = if sprite.tags.is_empty() {
        vec![(
            "default".into(),
            0,
            sprite.frames.len() - 1,
            AnimDirection::Forward,
            0,
        )]
    } else {
        sprite
            .tags
            .iter()
            .map(|t| (t.name.clone(), t.from, t.to, t.direction, t.repeat))
            .collect()
    };
    ranges
        .into_iter()
        .map(|(name, from, to, direction, repeat)| {
            let forward: Vec<usize> = (from..=to).collect();
            let back: Vec<usize> = forward.iter().rev().copied().collect();
            // Ping-pong sin repetir los extremos.
            let inner_back = || back[1..back.len().saturating_sub(1).max(1)].to_vec();
            let frames = match direction {
                AnimDirection::Forward => forward.clone(),
                AnimDirection::Reverse => back.clone(),
                AnimDirection::PingPong if forward.len() > 2 => {
                    [forward.clone(), inner_back()].concat()
                }
                AnimDirection::PingPongReverse if forward.len() > 2 => {
                    let inner_fwd = forward[1..forward.len() - 1].to_vec();
                    [back.clone(), inner_fwd].concat()
                }
                AnimDirection::PingPong => forward.clone(),
                AnimDirection::PingPongReverse => back.clone(),
            };
            let durations_ms = frames
                .iter()
                .map(|&f| sprite.frames[f].duration_ms)
                .collect();
            Animation {
                name,
                frames,
                durations_ms,
                looping: repeat == 0,
            }
        })
        .collect()
}

/// Recurso `SpriteFrames` de Godot 4 (`.tres`) que usa la hoja en `texture_path` (`res://…`).
pub fn godot_sprite_frames(
    sprite: &Sprite,
    texture_path: &str,
    layout: SheetLayout,
    scale: u32,
) -> String {
    let rects = frame_rects(sprite, layout, scale);
    let anims = animations(sprite);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "[gd_resource type=\"SpriteFrames\" load_steps={} format=3]\n",
        rects.len() + 2
    );
    let _ = writeln!(
        out,
        "[ext_resource type=\"Texture2D\" path=\"{texture_path}\" id=\"1_sheet\"]\n"
    );
    for (i, (x, y, w, h)) in rects.iter().enumerate() {
        let _ = writeln!(
            out,
            "[sub_resource type=\"AtlasTexture\" id=\"AtlasTexture_{i}\"]\natlas = ExtResource(\"1_sheet\")\nregion = Rect2({x}, {y}, {w}, {h})\n"
        );
    }
    out.push_str("[resource]\nanimations = [");
    for (a, anim) in anims.iter().enumerate() {
        // La velocidad es la del frame más corto; cada frame dura un múltiplo de ella.
        let base = anim
            .durations_ms
            .iter()
            .copied()
            .min()
            .unwrap_or(100)
            .max(1) as f32;
        out.push_str(if a == 0 { "{\n" } else { ", {\n" });
        out.push_str("\"frames\": [");
        for (i, (&f, &ms)) in anim.frames.iter().zip(&anim.durations_ms).enumerate() {
            let _ = write!(
                out,
                "{}{{\n\"duration\": {:?},\n\"texture\": SubResource(\"AtlasTexture_{f}\")\n}}",
                if i == 0 { "" } else { ", " },
                ms as f32 / base
            );
        }
        let _ = write!(
            out,
            "],\n\"loop\": {},\n\"name\": &\"{}\",\n\"speed\": {:?}\n}}",
            anim.looping,
            anim.name.replace('"', "'"),
            (1000.0 / base * 100.0).round() / 100.0
        );
    }
    out.push_str("]\n");
    out
}

/// Hash FNV-1a de 64 bits (para ids estables entre exportaciones).
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// 32 dígitos hex estables a partir de un texto (guid de Unity).
fn stable_guid(text: &str) -> String {
    format!("{:016x}{:016x}", fnv(text), fnv(&format!("{text}#")))
}

/// Pivote normalizado (0–1, origen abajo a la izquierda como en Unity) del frame, tomado del
/// primer slice con pivote; si no hay, el centro.
fn unity_pivot(sprite: &Sprite, frame: usize) -> (f32, f32) {
    sprite
        .slices
        .iter()
        .filter_map(|s| s.key_at(frame))
        .find_map(|k| {
            let (px, py) = k.pivot?;
            let (x, y) = (k.bounds.0 + px, k.bounds.1 + py);
            Some((
                x as f32 / sprite.width as f32,
                1.0 - y as f32 / sprite.height as f32,
            ))
        })
        .unwrap_or((0.5, 0.5))
}

/// Borde 9-slice de Unity del frame (izquierda, abajo, derecha, arriba; en píxeles escalados),
/// tomado del primer slice con centro; sin centro, sin borde.
fn unity_border(sprite: &Sprite, frame: usize, scale: u32) -> (u32, u32, u32, u32) {
    sprite
        .slices
        .iter()
        .filter_map(|s| s.key_at(frame))
        .find_map(|k| {
            let (cx, cy, cw, ch) = k.center?;
            let (x0, y0) = (k.bounds.0 + cx, k.bounds.1 + cy);
            let (x1, y1) = (x0 + cw as i32, y0 + ch as i32);
            let (w, h) = (sprite.width as i32, sprite.height as i32);
            let px = |v: i32, max: i32| v.clamp(0, max) as u32 * scale.max(1);
            Some((px(x0, w), px(h - y1, h), px(w - x1, w), px(y0, h)))
        })
        .unwrap_or_default()
}

/// `.meta` de Unity para la hoja: textura en modo sprite múltiple, filtro point, sin compresión,
/// y un sprite por frame (con el pivote de los slices). El guid es estable para no romper
/// referencias al reexportar.
pub fn unity_meta(sprite: &Sprite, image_name: &str, layout: SheetLayout, scale: u32) -> String {
    let rects = frame_rects(sprite, layout, scale);
    let sheet_h = rects.iter().map(|r| r.1 + r.3).max().unwrap_or(0);
    let stem = image_name.trim_end_matches(".png");
    let mut out = format!(
        "fileFormatVersion: 2\nguid: {}\nTextureImporter:\n  serializedVersion: 12\n  mipmaps:\n    enableMipMap: 0\n  textureSettings:\n    serializedVersion: 2\n    filterMode: 0\n    aniso: 1\n    wrapU: 1\n    wrapV: 1\n  textureType: 8\n  textureShape: 1\n  spriteMode: 2\n  spritePixelsToUnits: {}\n  alphaUsage: 1\n  alphaIsTransparency: 1\n  platformSettings:\n  - serializedVersion: 3\n    buildTarget: DefaultTexturePlatform\n    textureCompression: 0\n    filterMode: 0\n  spriteSheet:\n    serializedVersion: 2\n    sprites:\n",
        stable_guid(image_name),
        sprite.width.max(sprite.height) * scale.max(1)
    );
    for (f, (x, y, w, h)) in rects.iter().enumerate() {
        let name = format!("{stem}_{f}");
        let (px, py) = unity_pivot(sprite, f);
        let (bl, bb, br, bt) = unity_border(sprite, f, scale);
        let _ = write!(
            out,
            "    - serializedVersion: 2\n      name: {name}\n      rect:\n        serializedVersion: 2\n        x: {x}\n        y: {}\n        width: {w}\n        height: {h}\n      alignment: 9\n      pivot: {{x: {px}, y: {py}}}\n      border: {{x: {bl}, y: {bb}, z: {br}, w: {bt}}}\n      spriteID: {}\n      internalID: {}\n",
            sheet_h - y - h,
            stable_guid(&name),
            (fnv(&name) >> 1) as i64
        );
    }
    out.push_str("  spritePackingTag:\n  userData:\n  assetBundleName:\n  assetBundleVariant:\n");
    out
}

/// Atlas para Bevy en RON: tamaño de celda y grilla (para `TextureAtlasLayout::from_grid`),
/// animaciones con duraciones y los slices por frame.
pub fn bevy_atlas(sprite: &Sprite, image_name: &str, layout: SheetLayout, scale: u32) -> String {
    let scale = scale.max(1);
    let columns = sheet_columns(sprite, layout);
    let rows = (sprite.frames.len() as u32).div_ceil(columns);
    let mut out = format!(
        "(\n    image: \"{image_name}\",\n    tile_size: ({}, {}),\n    columns: {columns},\n    rows: {rows},\n    animations: {{\n",
        sprite.width * scale,
        sprite.height * scale
    );
    for anim in animations(sprite) {
        let _ = writeln!(
            out,
            "        \"{}\": (frames: {:?}, durations_ms: {:?}, looping: {}),",
            anim.name.replace('"', "'"),
            anim.frames,
            anim.durations_ms,
            anim.looping
        );
    }
    out.push_str("    },\n    slices: {\n");
    let s = scale as i32;
    for slice in &sprite.slices {
        let keys: Vec<String> = slice
            .keys
            .iter()
            .map(|k| {
                let (x, y, w, h) = k.bounds;
                let pivot = match k.pivot {
                    Some((px, py)) => format!("Some(({}, {}))", px * s, py * s),
                    None => "None".into(),
                };
                let center = match k.center {
                    Some((cx, cy, cw, ch)) => {
                        format!(
                            "Some(({}, {}, {}, {}))",
                            cx * s,
                            cy * s,
                            cw * scale,
                            ch * scale
                        )
                    }
                    None => "None".into(),
                };
                format!(
                    "(frame: {}, rect: ({}, {}, {}, {}), pivot: {pivot}, center: {center})",
                    k.frame,
                    x * s,
                    y * s,
                    w * scale,
                    h * scale
                )
            })
            .collect();
        let _ = writeln!(
            out,
            "        \"{}\": [{}],",
            slice.name.replace('"', "'"),
            keys.join(", ")
        );
    }
    out.push_str("    },\n)\n");
    out
}

#[cfg(test)]
mod tests {
    use cs_core::*;

    use super::*;

    fn sprite() -> Sprite {
        let mut s = Sprite::new(8, 4, ColorMode::Rgba);
        s.frames = vec![
            Frame { duration_ms: 100 },
            Frame { duration_ms: 200 },
            Frame { duration_ms: 100 },
            Frame { duration_ms: 100 },
        ];
        s.tags = vec![
            Tag {
                name: "walk".into(),
                from: 0,
                to: 3,
                direction: AnimDirection::PingPong,
                color: Rgba::rgb(0, 0, 0),
                repeat: 0,
            },
            Tag {
                name: "hit".into(),
                from: 2,
                to: 3,
                direction: AnimDirection::Reverse,
                color: Rgba::rgb(0, 0, 0),
                repeat: 1,
            },
        ];
        s.slices = vec![Slice {
            name: "feet".into(),
            color: Rgba::rgb(0, 0, 255),
            text: String::new(),
            keys: vec![SliceKey {
                frame: 0,
                bounds: (2, 0, 4, 4),
                center: None,
                pivot: Some((2, 4)),
            }],
        }];
        s
    }

    #[test]
    fn animations_expand_directions() {
        let a = animations(&sprite());
        assert_eq!(a[0].frames, [0, 1, 2, 3, 2, 1]);
        assert_eq!(a[0].durations_ms, [100, 200, 100, 100, 100, 200]);
        assert!(a[0].looping);
        assert_eq!(a[1].frames, [3, 2]);
        assert!(!a[1].looping);
        let mut plain = sprite();
        plain.tags.clear();
        assert_eq!(animations(&plain)[0].name, "default");
        assert_eq!(animations(&plain)[0].frames, [0, 1, 2, 3]);
    }

    #[test]
    fn grid_rects() {
        let rects = frame_rects(&sprite(), SheetLayout::Grid { columns: 3 }, 2);
        assert_eq!(rects[3], (0, 8, 16, 8));
        assert_eq!(
            frame_rects(&sprite(), SheetLayout::Vertical, 1)[2],
            (0, 8, 8, 4)
        );
    }

    #[test]
    fn godot_resource_shape() {
        let tres = godot_sprite_frames(&sprite(), "res://hero.png", SheetLayout::Horizontal, 1);
        assert!(tres.starts_with("[gd_resource type=\"SpriteFrames\" load_steps=6 format=3]"));
        assert!(tres.contains("region = Rect2(24, 0, 8, 4)"));
        assert!(tres.contains("\"name\": &\"walk\""));
        assert!(tres.contains("\"duration\": 2.0"));
        assert!(tres.contains("\"speed\": 10.0"));
        assert!(tres.contains("\"loop\": false"));
        assert_eq!(tres.matches("SubResource(\"AtlasTexture_").count(), 6 + 2);
    }

    #[test]
    fn unity_meta_has_flipped_rects_and_slice_pivots() {
        let meta = unity_meta(&sprite(), "hero.png", SheetLayout::Grid { columns: 2 }, 1);
        assert!(meta.contains("spriteMode: 2"));
        assert!(meta.contains("name: hero_3"));
        // Frame 0 está arriba a la izquierda: en Unity y = alto - 0 - 4 = 4.
        assert!(meta.contains("x: 0\n        y: 4\n"));
        assert!(meta.contains("pivot: {x: 0.5, y: 0}"));
        assert!(meta.contains("border: {x: 0, y: 0, z: 0, w: 0}"));
        let mut nine = sprite();
        nine.slices[0].keys[0].center = Some((1, 1, 2, 2));
        let meta = unity_meta(&nine, "hero.png", SheetLayout::Horizontal, 2);
        // Centro en x 3..5, y 1..3 de un frame de 8×4, a escala 2.
        assert!(meta.contains("border: {x: 6, y: 2, z: 6, w: 2}"));
        let ron = bevy_atlas(&nine, "hero.png", SheetLayout::Horizontal, 2);
        assert!(ron.contains("center: Some((2, 2, 4, 4))"));
        assert_eq!(
            unity_meta(&sprite(), "hero.png", SheetLayout::Horizontal, 1)
                .lines()
                .nth(1),
            meta.lines().nth(1),
            "stable guid"
        );
    }

    #[test]
    fn bevy_ron_lists_grid_animations_and_slices() {
        let ron = bevy_atlas(&sprite(), "hero.png", SheetLayout::Grid { columns: 2 }, 2);
        assert!(ron.contains("tile_size: (16, 8)"));
        assert!(ron.contains("columns: 2,\n    rows: 2,"));
        assert!(
            ron.contains("\"hit\": (frames: [3, 2], durations_ms: [100, 100], looping: false)")
        );
        assert!(ron.contains(
            "\"feet\": [(frame: 0, rect: (4, 0, 8, 8), pivot: Some((4, 8)), center: None)]"
        ));
    }
}
