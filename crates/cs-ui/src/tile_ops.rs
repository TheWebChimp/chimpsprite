//! Modo Tiles: las herramientas trabajan sobre un proxy donde cada píxel es una celda del tilemap.

use std::sync::Arc;

use cs_commands::Command;
use cs_core::Tile;
use cs_core::{Cel, ColorMode, Image, ImageId, LayerId, LayerKind, Mask, PixelValue, Rgba, Sprite};
use cs_tools::Point;

/// Única capa del proxy (frame 0).
pub const PROXY_LAYER: LayerId = LayerId(1);
const PROXY_IMAGE: ImageId = ImageId(1);

/// Un tile como píxel del proxy: los 4 bytes del valor (little endian). El tile 0 es transparente.
pub fn encode(tile: Tile) -> Rgba {
    let [r, g, b, a] = tile.0.to_le_bytes();
    Rgba::new(r, g, b, a)
}

pub fn decode(c: Rgba) -> Tile {
    Tile(u32::from_le_bytes([c.r, c.g, c.b, c.a]))
}

/// Número que ve el usuario: el vacío es 0; el tile `index` es `index + base_index - 1`.
pub fn tile_number(index: u32, base_index: i16) -> i64 {
    if index == 0 {
        0
    } else {
        index as i64 + base_index as i64 - 1
    }
}

/// Geometría de las celdas de una capa tilemap en el lienzo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileSpace {
    pub tile_size: (u32, u32),
    /// Resto de la posición del cel respecto del tamaño de tile (0 si está alineado o no hay cel):
    /// la celda `(cx, cy)` empieza en el píxel `(cx·tw + offset.0, cy·th + offset.1)`, como en
    /// `ops::set_tiles`.
    pub offset: (i32, i32),
}

impl TileSpace {
    /// `None` si la capa no es tilemap o su tileset no existe.
    pub fn of(sprite: &Sprite, layer: LayerId, frame: usize) -> Option<TileSpace> {
        let l = sprite.layer(layer)?;
        if l.kind != LayerKind::Tilemap {
            return None;
        }
        let tileset = sprite.tilesets.get(l.tileset? as usize)?;
        let (tw, th) = tileset.tile_size;
        if tw == 0 || th == 0 {
            return None;
        }
        let offset = sprite.cel(layer, frame).map_or((0, 0), |c| {
            (
                c.position.0.rem_euclid(tw as i32),
                c.position.1.rem_euclid(th as i32),
            )
        });
        Some(TileSpace {
            tile_size: (tw, th),
            offset,
        })
    }

    /// Celda que contiene el píxel `p` del lienzo.
    pub fn cell_at(&self, p: Point) -> Point {
        let (tw, th) = (self.tile_size.0 as i32, self.tile_size.1 as i32);
        (
            (p.0 - self.offset.0).div_euclid(tw),
            (p.1 - self.offset.1).div_euclid(th),
        )
    }

    /// Píxel del lienzo donde empieza la celda `c`.
    pub fn cell_origin(&self, c: Point) -> Point {
        let (tw, th) = (self.tile_size.0 as i32, self.tile_size.1 as i32);
        (c.0 * tw + self.offset.0, c.1 * th + self.offset.1)
    }

    /// Celdas que cubren un lienzo de `canvas` píxeles: `ceil(ancho / tw) × ceil(alto / th)`.
    pub fn proxy_size(&self, canvas: (u32, u32)) -> (u32, u32) {
        (
            canvas.0.div_ceil(self.tile_size.0),
            canvas.1.div_ceil(self.tile_size.1),
        )
    }
}

/// Sprite donde cada píxel es una celda de la capa tilemap: lo que ven las herramientas en modo
/// Tiles.
pub struct TileProxy {
    /// Una capa de imagen (`PROXY_LAYER`) y un frame; cada píxel es `encode(tile)`.
    pub sprite: Sprite,
    /// Selección del editor en celdas: una celda está seleccionada si su píxel de origen lo está.
    pub selection: Option<Mask>,
    pub space: TileSpace,
    /// Capa y frame reales.
    pub layer: LayerId,
    pub frame: usize,
}

impl TileProxy {
    /// Proxy de la capa tilemap `layer` en `frame`, con `selection` (en píxeles) pasada a celdas.
    /// `None` si la capa no es tilemap o su tileset no existe.
    pub fn build(
        sprite: &Sprite,
        layer: LayerId,
        frame: usize,
        selection: Option<&Mask>,
    ) -> Option<TileProxy> {
        let space = TileSpace::of(sprite, layer, frame)?;
        let (pw, ph) = space.proxy_size((sprite.width, sprite.height));
        let mut image = Image::new(pw, ph, ColorMode::Rgba);
        let found = sprite
            .cel(layer, frame)
            .and_then(|c| Some((c.position, sprite.tilemap(c.image)?)));
        if let Some((position, grid)) = found {
            let (tw, th) = (space.tile_size.0 as i32, space.tile_size.1 as i32);
            // Celda del lienzo donde empieza la grilla (la misma corrección que `ops::set_tiles`).
            let origin = (position.0.div_euclid(tw), position.1.div_euclid(th));
            for cy in 0..ph as i32 {
                for cx in 0..pw as i32 {
                    let tile = u32::try_from(cx - origin.0)
                        .ok()
                        .zip(u32::try_from(cy - origin.1).ok())
                        .and_then(|(gx, gy)| grid.get(gx, gy))
                        .unwrap_or(Tile::EMPTY);
                    image
                        .set(cx as u32, cy as u32, PixelValue::Rgba(encode(tile)))
                        .expect("in bounds and RGBA");
                }
            }
        }
        let mut proxy = Sprite::new(pw, ph, ColorMode::Rgba);
        proxy.images.insert(PROXY_IMAGE, Arc::new(image));
        proxy.cels.insert(
            (PROXY_LAYER, 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: PROXY_IMAGE,
            },
        );
        Some(TileProxy {
            selection: selection.map(|m| pixels_to_cells(m, &space, (pw, ph))),
            sprite: proxy,
            space,
            layer,
            frame,
        })
    }

    /// Tile de la celda `c` (vacío fuera del proxy).
    pub fn tile_at(&self, c: Point) -> Tile {
        match self.sprite.pixel(PROXY_LAYER, 0, c.0, c.1) {
            PixelValue::Rgba(v) => decode(v),
            PixelValue::Index(_) => Tile::EMPTY,
        }
    }

    /// Celdas que pide un `DrawPixels` del proxy, con el tile decodificado: solo las que caen
    /// dentro del proxy y de la selección. `None` si el comando no es un `DrawPixels`.
    pub fn cells(&self, cmd: &Command) -> Option<Vec<(i32, i32, Tile)>> {
        let Command::DrawPixels { pixels, .. } = cmd else {
            return None;
        };
        let (w, h) = (self.sprite.width as i32, self.sprite.height as i32);
        Some(
            pixels
                .iter()
                .filter(|&&(x, y, _)| {
                    x >= 0
                        && y >= 0
                        && x < w
                        && y < h
                        && self.selection.as_ref().is_none_or(|m| m.contains(x, y))
                })
                .map(|&(x, y, v)| {
                    let tile = match v {
                        PixelValue::Rgba(c) => decode(c),
                        PixelValue::Index(_) => Tile::EMPTY,
                    };
                    (x, y, tile)
                })
                .collect(),
        )
    }

    /// `true` si poner `cells` no cambiaría ningún tile (incluido no tener celdas).
    pub fn unchanged(&self, cells: &[(i32, i32, Tile)]) -> bool {
        cells.iter().all(|&(x, y, t)| self.tile_at((x, y)) == t)
    }
}

/// Selección en píxeles → celdas (`size` = tamaño del proxy): una celda está seleccionada si su
/// píxel de origen lo está.
pub fn pixels_to_cells(pixels: &Mask, space: &TileSpace, size: (u32, u32)) -> Mask {
    let cells = (0..size.1 as i32).flat_map(|cy| (0..size.0 as i32).map(move |cx| (cx, cy)));
    Mask::from_points(
        size.0,
        size.1,
        cells.filter(|&c| {
            let (x, y) = space.cell_origin(c);
            pixels.contains(x, y)
        }),
    )
}

/// Selección en celdas → píxeles del lienzo: cada celda cubre su rectángulo de `tw × th`.
pub fn cells_to_pixels(cells: &Mask, space: &TileSpace, canvas: (u32, u32)) -> Mask {
    let space = *space;
    let (tw, th) = (space.tile_size.0 as i32, space.tile_size.1 as i32);
    let points = cells.points().flat_map(move |c| {
        let (x0, y0) = space.cell_origin(c);
        (0..th).flat_map(move |dy| (0..tw).map(move |dx| (x0 + dx, y0 + dy)))
    });
    Mask::from_points(canvas.0, canvas.1, points)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cs_core::{Cel, ColorMode, ImageId, Layer, LayerId, LayerKind, Mask, PixelValue, Rgba};
    use cs_core::{Sprite, Tile, TileGrid, Tileset};

    use super::*;

    #[test]
    fn encode_and_decode_round_trip_with_flips() {
        for tile in [
            Tile::EMPTY,
            Tile::new(1, false, false, false),
            Tile::new(7, true, false, true),
            Tile::new(Tile::INDEX_MASK, true, true, true),
        ] {
            assert_eq!(decode(encode(tile)), tile);
        }
        assert_eq!(
            encode(Tile::EMPTY),
            Rgba::TRANSPARENT,
            "the empty tile is transparent"
        );
        assert_eq!(
            encode(Tile::new(5, false, false, false)),
            Rgba::new(5, 0, 0, 0)
        );
        assert_eq!(tile_number(0, 1), 0, "the empty tile is always 0");
        assert_eq!(tile_number(3, 1), 3);
        assert_eq!(tile_number(3, 0), 2);
    }

    /// Sprite `w`×`h` con un tileset de 4×4 y la capa 2 tilemap; su cel en `position` tiene una
    /// grilla de 2×1 (tile 1 y tile 2 volteado en X).
    fn tiled(w: u32, h: u32, position: (i32, i32)) -> Sprite {
        let mut s = Sprite::new(w, h, ColorMode::Rgba);
        s.tilesets.push(Tileset::new("t", (4, 4), ColorMode::Rgba));
        let mut layer = Layer::new_image(LayerId(2), "map");
        layer.kind = LayerKind::Tilemap;
        layer.tileset = Some(0);
        s.layers.push(layer);
        let mut grid = TileGrid::new(2, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        grid.set(1, 0, Tile::new(2, true, false, false));
        s.tilemaps.insert(ImageId(1), Arc::new(grid));
        s.cels.insert(
            (LayerId(2), 0),
            Cel {
                position,
                opacity: 255,
                image: ImageId(1),
            },
        );
        s
    }

    #[test]
    fn proxy_covers_the_canvas_even_when_it_is_not_a_multiple_of_the_tile() {
        let s = tiled(10, 9, (0, 0));
        let proxy = TileProxy::build(&s, LayerId(2), 0, None).unwrap();
        assert_eq!((proxy.sprite.width, proxy.sprite.height), (3, 3));
        assert_eq!(proxy.tile_at((0, 0)), Tile::new(1, false, false, false));
        assert_eq!(proxy.tile_at((1, 0)), Tile::new(2, true, false, false));
        assert_eq!(proxy.tile_at((2, 2)), Tile::EMPTY);
        assert_eq!(proxy.space.offset, (0, 0));
        assert!(
            TileProxy::build(&s, LayerId(1), 0, None).is_none(),
            "image layers have no proxy"
        );
    }

    #[test]
    fn proxy_of_an_unaligned_cel_uses_the_set_tiles_correction() {
        // Cel en (6, 3): la grilla empieza en la celda (1, 0), corrida (2, 3) píxeles.
        let s = tiled(16, 8, (6, 3));
        let proxy = TileProxy::build(&s, LayerId(2), 0, None).unwrap();
        assert_eq!((proxy.sprite.width, proxy.sprite.height), (4, 2));
        assert_eq!(proxy.space.offset, (2, 3));
        assert_eq!(proxy.tile_at((0, 0)), Tile::EMPTY);
        assert_eq!(proxy.tile_at((1, 0)), Tile::new(1, false, false, false));
        assert_eq!(proxy.tile_at((2, 0)), Tile::new(2, true, false, false));
        // El puntero sobre un tile que se ve cae en la celda de ese tile.
        assert_eq!(proxy.space.cell_at((6, 3)), (1, 0));
        assert_eq!(proxy.space.cell_at((13, 6)), (2, 0));
        assert_eq!(proxy.space.cell_at((5, 3)), (0, 0));
        assert_eq!(proxy.space.cell_at((1, 1)), (-1, -1));
        assert_eq!(proxy.space.cell_origin((1, 0)), (6, 3));
        // Lo que se escribe en la celda (1, 0) del proxy es la celda (0, 0) de la grilla.
        let cmd = cs_commands::ops::set_tiles(
            &s,
            LayerId(2),
            0,
            &[(1, 0, Tile::new(2, false, false, false))],
        )
        .unwrap();
        let mut after = s.clone();
        cmd.apply(&mut after).unwrap();
        let cel = after.cel(LayerId(2), 0).unwrap();
        assert_eq!(
            after.tilemap(cel.image).unwrap().get(0, 0),
            Some(Tile::new(2, false, false, false))
        );
    }

    #[test]
    fn selections_convert_between_pixels_and_cells() {
        let s = tiled(16, 8, (0, 0));
        // El píxel de origen de la celda (1, 0) y un píxel suelto dentro de la celda (2, 1).
        let mut sel = Mask::new(16, 8);
        sel.set(4, 0, true);
        sel.set(9, 5, true);
        let proxy = TileProxy::build(&s, LayerId(2), 0, Some(&sel)).unwrap();
        let cells = proxy.selection.clone().unwrap();
        assert_eq!(cells.points().collect::<Vec<_>>(), vec![(1, 0)]);
        let back = cells_to_pixels(&cells, &proxy.space, (16, 8));
        assert_eq!(back.bounds(), Some(((4, 0), (7, 3))));
        assert_eq!(back.points().count(), 16);
    }

    #[test]
    fn proxy_strokes_become_cells_inside_the_proxy_and_the_selection() {
        let s = tiled(16, 8, (0, 0));
        // Selección de las celdas (0, 0) y (1, 0).
        let mut sel = Mask::new(16, 8);
        for y in 0..4 {
            for x in 0..8 {
                sel.set(x, y, true);
            }
        }
        let proxy = TileProxy::build(&s, LayerId(2), 0, Some(&sel)).unwrap();
        let flipped = Tile::new(2, false, true, false);
        let brush = PixelValue::Rgba(encode(flipped));
        let cmd = Command::DrawPixels {
            layer: PROXY_LAYER,
            frame: 0,
            pixels: vec![
                (0, 0, brush),
                (1, 0, brush),
                (2, 0, brush),
                (-1, 0, brush),
                (1, 0, PixelValue::Rgba(Rgba::TRANSPARENT)),
            ],
        };
        assert_eq!(
            proxy.cells(&cmd).unwrap(),
            vec![(0, 0, flipped), (1, 0, flipped), (1, 0, Tile::EMPTY)]
        );
        assert!(proxy.cells(&Command::SetGrid(Default::default())).is_none());
        assert!(proxy.unchanged(&[(0, 0, Tile::new(1, false, false, false))]));
        assert!(!proxy.unchanged(&[(0, 0, Tile::EMPTY)]));
    }
}
