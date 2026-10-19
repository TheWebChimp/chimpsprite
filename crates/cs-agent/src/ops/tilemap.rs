//! Tilemaps: tilesets, tiles y grillas de las capas tilemap.

use cs_commands::{Command, ops};
use cs_core::{Image, LayerId, LayerKind, Sprite, Tile, Tileset};
use serde_json::{Value, json};

use crate::error::{Result, invalid};
use crate::params::{AddTilemapLayer, CreateTileset, GetTiles, Ref, SetTilePixels, SetTiles};
use crate::target;
use crate::{Reply, Session};

/// Lado máximo de un tile.
const MAX_TILE: u32 = 256;
/// Celdas máximas que devuelve una llamada a `get_tiles` (256×256).
const MAX_READ_CELLS: u64 = 256 * 256;
/// Coordenada máxima (en celdas, en valor absoluto) que acepta `set_tiles`.
const MAX_CELL: i32 = 4096;

/// Capa tilemap indicada, con el índice y el tileset que usa.
fn tilemap_layer<'a>(sprite: &'a Sprite, r: &Ref) -> Result<(LayerId, usize, &'a Tileset)> {
    let id = target::layer(sprite, r)?;
    let layer = sprite.layer(id).expect("resolved");
    if layer.kind != LayerKind::Tilemap {
        return Err(invalid(format!(
            "layer '{}' is not a tilemap layer; create one with add_tilemap_layer",
            layer.name
        )));
    }
    let index = layer.tileset.map_or(usize::MAX, |i| i as usize);
    let tileset = sprite.tilesets.get(index).ok_or_else(|| {
        invalid(format!(
            "tilemap layer '{}' has no valid tileset",
            layer.name
        ))
    })?;
    Ok((id, index, tileset))
}

fn tileset_at(sprite: &Sprite, index: usize) -> Result<&Tileset> {
    sprite.tilesets.get(index).ok_or_else(|| {
        invalid(format!(
            "tileset {index} not found; the artboard has {} tileset(s) (create one with create_tileset)",
            sprite.tilesets.len()
        ))
    })
}

impl Session {
    pub fn create_tileset(&mut self, p: CreateTileset) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (w, h) = (p.tile_width, p.tile_height);
        if !(1..=MAX_TILE).contains(&w) || !(1..=MAX_TILE).contains(&h) {
            return Err(invalid(format!(
                "tile size {w}×{h} must be 1-{MAX_TILE} on each side"
            )));
        }
        let mut tileset = Tileset::new(p.name.clone(), (w, h), sprite.color_mode);
        if let Some(base) = p.base_index {
            tileset.base_index = base;
        }
        let index = sprite.tilesets.len();
        let cmd = Command::SetTileset {
            index,
            tileset: Some(tileset),
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!(
            "Created tileset {index} '{}' with {w}×{h} tiles. Tile 0 is the empty tile; add tiles \
             with set_tile_pixels (tile 1 first).",
            p.name
        )))
    }

    pub fn set_tile_pixels(&mut self, p: SetTilePixels) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let ts = tileset_at(sprite, p.tileset)?;
        if p.tile == 0 {
            return Err(invalid("tile 0 is the empty tile and cannot be edited"));
        }
        let count = ts.tiles.len();
        if p.tile > count {
            return Err(invalid(format!(
                "tile {} out of range; the tileset has {count} tiles (use tile {count} to add one)",
                p.tile
            )));
        }
        let (w, h) = ts.tile_size;
        let mut image = ts
            .tiles
            .get(p.tile)
            .map_or_else(|| Image::new(w, h, sprite.color_mode), |t| Image::clone(t));
        for px in &p.pixels {
            if px.x < 0 || px.y < 0 || px.x as u32 >= w || px.y as u32 >= h {
                return Err(invalid(format!(
                    "pixel ({}, {}) is outside the {w}×{h} tile",
                    px.x, px.y
                )));
            }
            let value = target::color(sprite, &px.color)?;
            image
                .set(px.x as u32, px.y as u32, value)
                .map_err(|e| invalid(e.to_string()))?;
        }
        let added = p.tile == count;
        let Some(cmd) = ops::set_tile_image(sprite, p.tileset, p.tile, image) else {
            return Ok(Reply::text("Nothing changed."));
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(if added {
            format!("Added tile {} to tileset {}.", p.tile, p.tileset)
        } else {
            format!("Painted tile {} of tileset {}.", p.tile, p.tileset)
        }))
    }

    pub fn add_tilemap_layer(&mut self, p: AddTilemapLayer) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        tileset_at(sprite, p.tileset)?;
        // Arriba de todo, en el nivel raíz.
        let Command::AddLayer { index, mut layer } =
            ops::new_tilemap_layer(sprite, None, p.tileset as u32)
        else {
            unreachable!("new_tilemap_layer devuelve AddLayer");
        };
        if let Some(name) = &p.name {
            layer.name = name.clone();
        }
        let reply = format!(
            "Added tilemap layer '{}' using tileset {}.",
            layer.name, p.tileset
        );
        self.run_on(&p.doc, ab, vec![Command::AddLayer { index, layer }])?;
        Ok(Reply::text(reply))
    }

    pub fn set_tiles(&mut self, p: SetTiles) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (layer, _, tileset) = tilemap_layer(sprite, &p.layer)?;
        let frame = target::frame(sprite, p.frame)?;
        if p.cells.is_empty() {
            return Err(invalid("cells is empty"));
        }
        let count = tileset.tiles.len();
        let square = tileset.tile_size.0 == tileset.tile_size.1;
        let mut cells = Vec::with_capacity(p.cells.len());
        for c in &p.cells {
            if !(-MAX_CELL..=MAX_CELL).contains(&c.x) || !(-MAX_CELL..=MAX_CELL).contains(&c.y) {
                return Err(invalid(format!(
                    "cell ({}, {}) is too far; use -{MAX_CELL}..{MAX_CELL}",
                    c.x, c.y
                )));
            }
            if c.tile as usize >= count {
                return Err(invalid(format!(
                    "tile {} out of range; the tileset has {count} tiles (0 = empty)",
                    c.tile
                )));
            }
            if c.flip_d && !square {
                return Err(invalid("flip_d needs square tiles"));
            }
            cells.push((c.x, c.y, Tile::new(c.tile, c.flip_x, c.flip_y, c.flip_d)));
        }
        let Some(cmd) = ops::set_tiles(sprite, layer, frame, &cells) else {
            // `None` también significa "grilla > 4096 celdas de lado": se distingue replicando
            // la extensión que calcula `ops::set_tiles` (relativa al origen del cel, incluyendo
            // las celdas vacías, que también agrandan la grilla).
            let (tw, th) = tileset.tile_size;
            let found = sprite
                .cel(layer, frame)
                .and_then(|c| Some((c, sprite.tilemap(c.image)?)));
            let (origin, (ow, oh)) = found.map_or(((0i64, 0i64), (0i64, 0i64)), |(c, g)| {
                (
                    (
                        c.position.0.div_euclid(tw as i32) as i64,
                        c.position.1.div_euclid(th as i32) as i64,
                    ),
                    (g.width as i64, g.height as i64),
                )
            });
            let (mut x0, mut y0, mut x1, mut y1) = (0i64, 0i64, ow, oh);
            for &(x, y, _) in &cells {
                let (rx, ry) = (x as i64 - origin.0, y as i64 - origin.1);
                x0 = x0.min(rx);
                y0 = y0.min(ry);
                x1 = x1.max(rx + 1);
                y1 = y1.max(ry + 1);
            }
            let limit = ops::MAX_TILEMAP_SIDE as i64;
            let (gw, gh) = (x1 - x0, y1 - y0);
            if gw > limit || gh > limit {
                return Err(invalid(format!(
                    "the tilemap grid would exceed the {limit}-tile limit per side"
                )));
            }
            if !ops::tilemap_fits(gw as u64, gh as u64, (tw, th)) {
                return Err(invalid(format!(
                    "the tilemap grid would be {gw}×{gh} tiles of {tw}×{th} ({}×{} pixels), more \
                     than the {} pixels a tilemap cel can cover; use fewer or closer cells",
                    gw * tw as i64,
                    gh * th as i64,
                    cs_core::MAX_TILEMAP_PIXELS
                )));
            }
            return Ok(Reply::text("Nothing changed."));
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!(
            "Set {} cell(s) on frame {frame}.",
            cells.len()
        )))
    }

    pub fn get_tiles(&self, p: GetTiles) -> Result<Reply> {
        let (_, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (layer, index, tileset) = tilemap_layer(sprite, &p.layer)?;
        let frame = target::frame(sprite, p.frame)?;
        let (tw, th) = tileset.tile_size;
        let found = sprite
            .cel(layer, frame)
            .and_then(|c| Some((c, sprite.tilemap(c.image)?)));
        let Some((cel, grid)) = found else {
            return Ok(Reply::json(json!({
                "layer": layer.0,
                "frame": frame,
                "tileset": index,
                "tile_size": [tw, th],
                "cel": null,
                "rows": [],
            })));
        };
        let first_cell = [
            cel.position.0.div_euclid(tw as i32),
            cel.position.1.div_euclid(th as i32),
        ];
        // Celdas del lienzo a leer: el rectángulo pedido o la grilla entera.
        let rect = match (p.x, p.y, p.w, p.h) {
            (None, None, None, None) => {
                if grid.width as u64 * grid.height as u64 > MAX_READ_CELLS {
                    return Err(invalid(format!(
                        "the grid has {}×{} cells, more than {MAX_READ_CELLS} per call; read it in                          parts with x, y, w, h (a rect of canvas cells; it starts at cell ({}, {}))",
                        grid.width, grid.height, first_cell[0], first_cell[1]
                    )));
                }
                None
            }
            (Some(x), Some(y), Some(w), Some(h)) => {
                if w == 0 || h == 0 || w as u64 * h as u64 > MAX_READ_CELLS {
                    return Err(invalid(format!(
                        "the rect must be 1 or more cells per side and at most {MAX_READ_CELLS}                          cells in total (e.g. 256×256)"
                    )));
                }
                Some((x, y, w, h))
            }
            _ => return Err(invalid("give all of x, y, w, h for a rect, or none")),
        };
        let (x0, y0, w, h) =
            rect.unwrap_or((first_cell[0], first_cell[1], grid.width, grid.height));
        let rows: Vec<Value> = (0..h as i64)
            .map(|r| {
                let row: Vec<Value> = (0..w as i64)
                    .map(|c| {
                        // Celda del lienzo → celda de la grilla; fuera de la grilla, vacía.
                        let gx = x0 as i64 + c - first_cell[0] as i64;
                        let gy = y0 as i64 + r - first_cell[1] as i64;
                        let t = u32::try_from(gx)
                            .ok()
                            .zip(u32::try_from(gy).ok())
                            .and_then(|(gx, gy)| grid.get(gx, gy))
                            .unwrap_or(Tile::EMPTY);
                        let mut v = json!({ "tile": t.index() });
                        if t.flip_x() {
                            v["flip_x"] = json!(true);
                        }
                        if t.flip_y() {
                            v["flip_y"] = json!(true);
                        }
                        if t.flip_d() {
                            v["flip_d"] = json!(true);
                        }
                        v
                    })
                    .collect();
                Value::from(row)
            })
            .collect();
        let mut reply = json!({
            "layer": layer.0,
            "frame": frame,
            "tileset": index,
            "tile_size": [tw, th],
            "cel": {
                "x": cel.position.0,
                "y": cel.position.1,
                "opacity": cel.opacity,
                "first_cell": first_cell,
                "columns": grid.width,
                "rows": grid.height,
            },
            "rows": rows,
            "notes": "rows[r][c] is canvas cell (first_cell[0] + c, first_cell[1] + r); tile 0 is empty.",
        });
        if rect.is_some() {
            reply["rect"] = json!({ "x": x0, "y": y0, "w": w, "h": h });
            reply["notes"] = json!(
                "rows[r][c] is canvas cell (rect.x + c, rect.y + r); tile 0 is empty (also outside the grid)."
            );
        }
        Ok(Reply::json(reply))
    }
}

#[cfg(test)]
mod tests {
    use cs_core::Rgba;
    use serde_json::json;

    use crate::Session;
    use crate::ops::test_util::{json, p, session};

    const RED: Rgba = Rgba::rgb(255, 0, 0);
    const BLUE: Rgba = Rgba::rgb(0, 0, 255);

    fn px(s: &Session, x: usize, y: usize) -> Rgba {
        let (_, sprite) = s.sprite("d1", None).unwrap();
        let d = &s.doc("d1").unwrap().document;
        cs_render::render_frame(&cs_render::materialize(d, sprite, 0..1), 0)
            [y * sprite.width as usize + x]
    }

    #[test]
    fn tilemaps_create_paint_place_read_and_describe() {
        let mut s = session(16, 8);
        s.create_tileset(p(json!({"doc": "d1", "name": "terrain",
            "tile_width": 4, "tile_height": 4})))
            .unwrap();
        s.set_tile_pixels(p(json!({"doc": "d1", "tileset": 0, "tile": 1, "pixels": [
            {"x": 0, "y": 0, "color": "#ff0000"}, {"x": 3, "y": 0, "color": "#0000ff"}]})))
            .unwrap();
        assert!(
            s.set_tile_pixels(p(json!({"doc": "d1", "tileset": 0, "tile": 0,
                "pixels": [{"x": 0, "y": 0, "color": "#ff0000"}]})))
                .is_err(),
            "tile 0 cannot be edited"
        );
        assert!(
            s.set_tile_pixels(p(
                json!({"doc": "d1", "tileset": 0, "tile": 3, "pixels": []})
            ))
            .is_err(),
            "tile 3 would skip tile 2"
        );
        s.add_tilemap_layer(p(json!({"doc": "d1", "tileset": 0, "name": "map"})))
            .unwrap();
        s.set_tiles(p(json!({"doc": "d1", "layer": "map", "cells": [
            {"x": 1, "y": 1, "tile": 1}, {"x": 2, "y": 1, "tile": 1, "flip_x": true}]})))
            .unwrap();
        // Celda (1, 1) = píxeles 4..8 × 4..8; la (2, 1) está espejada.
        assert_eq!(px(&s, 4, 4), RED);
        assert_eq!(px(&s, 7, 4), BLUE);
        assert_eq!(px(&s, 11, 4), RED);
        assert_eq!(px(&s, 8, 4), BLUE);
        assert!(
            s.set_tiles(p(json!({"doc": "d1", "layer": "map",
                "cells": [{"x": 0, "y": 0, "tile": 5}]})))
                .is_err(),
            "tile out of range"
        );
        assert!(
            s.set_tiles(p(json!({"doc": "d1", "layer": 1,
                "cells": [{"x": 0, "y": 0, "tile": 1}]})))
                .is_err(),
            "not a tilemap layer"
        );

        let tiles = json(
            &s.get_tiles(p(json!({"doc": "d1", "layer": "map"})))
                .unwrap(),
        );
        assert_eq!(tiles["cel"]["columns"], 3);
        assert_eq!(tiles["cel"]["rows"], 2);
        assert_eq!(tiles["cel"]["first_cell"], json!([0, 0]));
        assert_eq!(tiles["rows"][1][2]["tile"], 1);
        assert_eq!(tiles["rows"][1][2]["flip_x"], true);
        assert!(tiles["rows"][1][1].get("flip_x").is_none());

        let described = json(&s.describe(p(json!({"doc": "d1"}))).unwrap());
        let artboard = &described["artboards"][0];
        assert_eq!(artboard["tilesets"][0]["name"], "terrain");
        assert_eq!(artboard["tilesets"][0]["tiles"], 2);
        assert_eq!(artboard["tilesets"][0]["tile_size"], json!([4, 4]));
        let layer = &artboard["layers_bottom_to_top"][1];
        assert_eq!(layer["kind"], "tilemap");
        assert_eq!(layer["tileset"], 0);
        let cel = artboard["cels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["layer"] == layer["id"])
            .unwrap();
        assert_eq!(cel["grid"], json!([3, 2]));
    }

    fn tile_session() -> Session {
        let mut s = session(16, 8);
        s.create_tileset(p(
            json!({"doc": "d1", "name": "t", "tile_width": 4, "tile_height": 4}),
        ))
        .unwrap();
        s.set_tile_pixels(p(json!({"doc": "d1", "tileset": 0, "tile": 1,
            "pixels": [{"x": 0, "y": 0, "color": "#ff0000"}]})))
            .unwrap();
        s.add_tilemap_layer(p(json!({"doc": "d1", "tileset": 0, "name": "map"})))
            .unwrap();
        s
    }

    fn set(s: &mut Session, cells: serde_json::Value) -> crate::error::Result<crate::Reply> {
        s.set_tiles(p(json!({"doc": "d1", "layer": "map", "cells": cells})))
    }

    #[test]
    fn set_tiles_rejects_extreme_coordinates_without_panicking() {
        let mut s = tile_session();
        for c in [
            json!([{"x": i32::MIN, "y": 0, "tile": 1}]),
            json!([{"x": 0, "y": i32::MIN, "tile": 1}]),
            json!([{"x": i32::MAX, "y": 0, "tile": 1}]),
        ] {
            assert!(set(&mut s, c).is_err());
        }
    }

    #[test]
    fn set_tiles_reports_the_grid_limit() {
        let mut s = tile_session();
        // (a) Sin cel previo: 0 y 4096 abarcan 4097 celdas.
        let e = set(
            &mut s,
            json!([{"x": 0, "y": 0, "tile": 1}, {"x": 4096, "y": 0, "tile": 1}]),
        )
        .unwrap_err();
        assert!(e.to_string().contains("4096"), "{e}");
        // (b) Con una grilla existente, una celda lejana la haría pasar el límite.
        set(&mut s, json!([{"x": 0, "y": 0, "tile": 1}])).unwrap();
        let e = set(&mut s, json!([{"x": 4096, "y": 0, "tile": 1}])).unwrap_err();
        assert!(e.to_string().contains("4096"), "{e}");
        // Una celda lejana que sí cabe agranda la grilla.
        set(&mut s, json!([{"x": 4095, "y": 0, "tile": 1}])).unwrap();
        // Borrar (tile 0) fuera de la grilla también la agranda, así que cuenta para el límite.
        let mut s2 = tile_session();
        set(&mut s2, json!([{"x": 0, "y": 0, "tile": 1}])).unwrap();
        let e = set(&mut s2, json!([{"x": -4096, "y": 0, "tile": 0}])).unwrap_err();
        assert!(e.to_string().contains("4096"), "{e}");
    }

    #[test]
    fn set_tiles_reports_the_pixel_area_limit() {
        // Tiles de 4×4: 4096×4096 celdas caben de lado, pero son 16384² píxeles.
        let mut s = tile_session();
        let e = set(
            &mut s,
            json!([{"x": 0, "y": 0, "tile": 1}, {"x": 4095, "y": 4095, "tile": 1}]),
        )
        .unwrap_err();
        assert!(e.to_string().contains("pixels"), "{e}");
        set(
            &mut s,
            json!([{"x": 0, "y": 0, "tile": 1}, {"x": 2047, "y": 2047, "tile": 1}]),
        )
        .unwrap();
    }

    #[test]
    fn get_tiles_pages_large_grids_with_a_rect() {
        let mut s = tile_session();
        // 300×300 celdas: más que 256×256, hay que pedirlas por partes.
        set(
            &mut s,
            json!([{"x": 0, "y": 0, "tile": 1}, {"x": 299, "y": 299, "tile": 1, "flip_y": true}]),
        )
        .unwrap();
        let get = |s: &Session, extra: serde_json::Value| {
            let mut v = json!({"doc": "d1", "layer": "map"});
            v.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            s.get_tiles(p(v))
        };
        let e = get(&s, json!({})).unwrap_err().to_string();
        assert!(e.contains("x, y, w, h"), "{e}");
        // Un rectángulo de celdas del lienzo; lo que cae fuera de la grilla es 0.
        let page = json(&get(&s, json!({"x": 298, "y": 298, "w": 3, "h": 2})).unwrap());
        assert_eq!(page["rect"], json!({"x": 298, "y": 298, "w": 3, "h": 2}));
        assert_eq!(page["rows"].as_array().unwrap().len(), 2);
        assert_eq!(page["rows"][1][1]["tile"], 1);
        assert_eq!(page["rows"][1][1]["flip_y"], true);
        assert_eq!(page["rows"][1][2]["tile"], 0, "outside the grid");
        assert_eq!(page["rows"][0][0]["tile"], 0);
        assert_eq!(page["cel"]["columns"], 300);
        // Rectángulos inválidos o demasiado grandes.
        for bad in [
            json!({"x": 0, "y": 0, "w": 3}),
            json!({"x": 0, "y": 0, "w": 0, "h": 3}),
            json!({"x": 0, "y": 0, "w": 257, "h": 257}),
        ] {
            assert!(get(&s, bad.clone()).is_err(), "{bad}");
        }
        // Una grilla chica sigue saliendo entera sin rectángulo.
        let small = json(&get(&tile_session_with_cell(), json!({})).unwrap());
        assert_eq!(small["rows"][0][0]["tile"], 1);
    }

    fn tile_session_with_cell() -> Session {
        let mut s = tile_session();
        set(&mut s, json!([{"x": 0, "y": 0, "tile": 1}])).unwrap();
        s
    }

    #[test]
    fn set_tiles_with_no_change_says_so() {
        let mut s = tile_session();
        set(&mut s, json!([{"x": 0, "y": 0, "tile": 1}])).unwrap();
        let r = set(&mut s, json!([{"x": 0, "y": 0, "tile": 1}])).unwrap();
        assert_eq!(r.text, "Nothing changed.");
    }
}
