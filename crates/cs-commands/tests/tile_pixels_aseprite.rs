//! Píxeles sobre tilemaps contra Aseprite: corre los escenarios de `fixtures/tile_pixels.json` con
//! `tiles::draw_tilemap_pixels` y compara el resultado con lo que dejó Aseprite 1.3.18 en
//! `fixtures/tile_pixels.expected` (generado con `fixtures/tile_pixels.lua`, ver su cabecera).

use std::sync::Arc;

use cs_commands::tiles::{TilePaintError, TilesetMode, draw_tilemap_pixels};
use cs_core::{
    Cel, ColorMode, Frame, Image, ImageId, Layer, LayerId, LayerKind, PixelValue, Rgba, Sprite,
    Tile, TileGrid, Tileset,
};
use serde_json::Value;

fn color(hex: &str) -> Rgba {
    let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
    Rgba::rgb(c(0), c(2), c(4))
}

fn tile_value(v: &Value) -> Tile {
    match v {
        Value::Number(n) => Tile(n.as_u64().unwrap() as u32),
        Value::String(s) => {
            let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
            let rest = &s[digits.len()..];
            Tile::new(
                digits.parse().unwrap(),
                rest.contains('x'),
                rest.contains('y'),
                rest.contains('d'),
            )
        }
        _ => panic!("tile value {v}"),
    }
}

/// El sprite de un escenario: 16×8 RGBA, sin la capa de imagen, con las capas tilemap del
/// escenario (ids 2, 3…) y un tileset de 4×4.
fn setup(sc: &Value) -> Sprite {
    let mut s = Sprite::new(16, 8, ColorMode::Rgba);
    s.layers.clear();
    s.cels.clear();
    for _ in 1..sc["frames"].as_u64().unwrap() {
        s.frames.push(Frame { duration_ms: 100 });
    }
    let mut ts = Tileset::new("t", (4, 4), ColorMode::Rgba);
    for pixels in sc["tiles"].as_array().unwrap() {
        let mut img = Image::new(4, 4, ColorMode::Rgba);
        for p in pixels.as_array().unwrap() {
            let (x, y) = (p[0].as_u64().unwrap() as u32, p[1].as_u64().unwrap() as u32);
            img.set(x, y, PixelValue::Rgba(color(p[2].as_str().unwrap())))
                .unwrap();
        }
        ts.tiles.push(Arc::new(img));
    }
    if let Some(flips) = sc["flips"].as_str() {
        ts.allowed_flips.x = flips.contains('x');
        ts.allowed_flips.y = flips.contains('y');
        ts.allowed_flips.d = flips.contains('d');
    }
    s.tilesets.push(ts);
    let mut next = 1;
    for (li, frames) in sc["layers"].as_array().unwrap().iter().enumerate() {
        let id = LayerId(li as u32 + 2);
        let mut layer = Layer::new_image(id, format!("Map{}", li + 1));
        layer.kind = LayerKind::Tilemap;
        layer.tileset = Some(0);
        s.layers.push(layer);
        for (fi, rows) in frames.as_array().unwrap().iter().enumerate() {
            let rows = rows.as_array().unwrap();
            let (w, h) = (rows[0].as_array().unwrap().len(), rows.len());
            let mut grid = TileGrid::new(w as u32, h as u32);
            for (y, row) in rows.iter().enumerate() {
                for (x, v) in row.as_array().unwrap().iter().enumerate() {
                    grid.set(x as u32, y as u32, tile_value(v));
                }
            }
            let linked = li == 0 && fi == 1 && sc["linked"].as_bool() == Some(true);
            let image = if linked {
                s.cel(id, 0).unwrap().image
            } else {
                let image = ImageId(next);
                next += 1;
                s.tilemaps.insert(image, Arc::new(grid));
                image
            };
            let position = match &sc["cel"] {
                Value::Array(p) if li == 0 && fi == 0 => {
                    (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32)
                }
                _ => (0, 0),
            };
            s.cels.insert(
                (id, fi),
                Cel {
                    position,
                    opacity: 255,
                    image,
                },
            );
        }
    }
    s
}

/// Los píxeles de un trazo de 1 px: los puntos unidos con rectas horizontales o verticales.
fn stroke_pixels(stroke: &Value) -> Vec<(i32, i32, PixelValue)> {
    let value = match stroke["tool"].as_str() {
        Some("eraser") => PixelValue::Rgba(Rgba::TRANSPARENT),
        _ => PixelValue::Rgba(color(stroke["color"].as_str().unwrap())),
    };
    let points: Vec<(i32, i32)> = stroke["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32))
        .collect();
    let mut out = vec![(points[0].0, points[0].1, value)];
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        assert!(x0 == x1 || y0 == y1, "only straight strokes");
        let steps = (x1 - x0).abs().max((y1 - y0).abs());
        for i in 1..=steps {
            out.push((
                x0 + (x1 - x0).signum() * i,
                y0 + (y1 - y0).signum() * i,
                value,
            ));
        }
    }
    out
}

fn mode(name: &str) -> TilesetMode {
    match name {
        "manual" => TilesetMode::Manual,
        "auto" => TilesetMode::Auto,
        "stack" => TilesetMode::Stack,
        _ => panic!("mode {name}"),
    }
}

/// Lo mismo que escribe `dump` en `tile_pixels.lua`.
fn dump(name: &str, s: &Sprite, out: &mut Vec<String>) {
    out.push(format!("== {name}"));
    for (i, tile) in s.tilesets[0].tiles.iter().enumerate().skip(1) {
        let mut px = Vec::new();
        for y in 0..tile.height() {
            for x in 0..tile.width() {
                if let Some(PixelValue::Rgba(c)) = tile.get(x, y)
                    && c.a != 0
                {
                    px.push(format!(
                        "{x},{y}={:02x}{:02x}{:02x}{:02x}",
                        c.r, c.g, c.b, c.a
                    ));
                }
            }
        }
        let sep = if px.is_empty() { "" } else { " " };
        out.push(format!("tile {i}:{sep}{}", px.join(" ")));
    }
    for (li, layer) in s.layers.iter().enumerate() {
        for f in 0..s.frames.len() {
            let Some(cel) = s.cel(layer.id, f) else {
                continue;
            };
            let grid = s.tilemap(cel.image).unwrap();
            let mut cells = Vec::new();
            for y in 0..grid.height {
                if y > 0 {
                    cells.push("|".to_string());
                }
                for x in 0..grid.width {
                    let t = grid.get(x, y).unwrap();
                    let mut v = t.index().to_string();
                    for (on, c) in [(t.flip_x(), 'x'), (t.flip_y(), 'y'), (t.flip_d(), 'd')] {
                        if on {
                            v.push(c);
                        }
                    }
                    cells.push(v);
                }
            }
            out.push(format!(
                "L{} F{} ({},{}) {}",
                li + 1,
                f + 1,
                cel.position.0,
                cel.position.1,
                cells.join(" ")
            ));
        }
    }
}

#[test]
fn tile_pixels_behave_like_aseprite() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    let spec: Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{dir}/tile_pixels.json")).unwrap())
            .unwrap();
    let expected = std::fs::read_to_string(format!("{dir}/tile_pixels.expected")).unwrap();
    let mut out = Vec::new();
    for sc in spec["scenarios"].as_array().unwrap() {
        let mut s = setup(sc);
        for stroke in sc["strokes"].as_array().unwrap() {
            let pixels = stroke_pixels(stroke);
            let layer = LayerId(stroke["layer"].as_u64().unwrap_or(1) as u32 + 1);
            let frame = stroke["frame"].as_u64().unwrap_or(1) as usize - 1;
            match draw_tilemap_pixels(
                &s,
                layer,
                frame,
                &pixels,
                mode(stroke["mode"].as_str().unwrap()),
            ) {
                Ok(cmd) => {
                    cmd.apply(&mut s).unwrap();
                }
                Err(TilePaintError::Nothing) => {}
                Err(e) => panic!("{}: {e:?}", sc["name"]),
            }
        }
        dump(sc["name"].as_str().unwrap(), &s, &mut out);
    }
    let got = out.join("\n") + "\n";
    // Escenario por escenario, para que el fallo diga cuál.
    let split = |text: &str| -> Vec<String> {
        text.replace("\r\n", "\n")
            .split("== ")
            .filter(|b| !b.is_empty())
            .map(str::to_string)
            .collect()
    };
    let (got, expected) = (split(&got), split(&expected));
    for (g, e) in got.iter().zip(&expected) {
        assert_eq!(g, e, "scenario differs from Aseprite");
    }
    assert_eq!(got.len(), expected.len());
}
