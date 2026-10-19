//! Tilemaps: tilesets (las imágenes de los tiles) y las grillas de tiles de los cels.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{ColorMode, Image, RawChunk};

/// Valor de una celda de tilemap, con el mismo layout que Aseprite:
/// bits 0–28 índice del tile, bit 29 flip X, bit 30 flip Y, bit 31 flip D (transponer).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile(pub u32);

impl Tile {
    pub const EMPTY: Tile = Tile(0);
    pub const INDEX_MASK: u32 = 0x1fff_ffff;
    pub const FLIP_X: u32 = 1 << 29;
    pub const FLIP_Y: u32 = 1 << 30;
    pub const FLIP_D: u32 = 1 << 31;

    pub fn new(index: u32, flip_x: bool, flip_y: bool, flip_d: bool) -> Tile {
        let flag = |on: bool, bit: u32| if on { bit } else { 0 };
        Tile(
            (index & Self::INDEX_MASK)
                | flag(flip_x, Self::FLIP_X)
                | flag(flip_y, Self::FLIP_Y)
                | flag(flip_d, Self::FLIP_D),
        )
    }

    pub fn index(self) -> u32 {
        self.0 & Self::INDEX_MASK
    }

    pub fn flip_x(self) -> bool {
        self.0 & Self::FLIP_X != 0
    }

    pub fn flip_y(self) -> bool {
        self.0 & Self::FLIP_Y != 0
    }

    pub fn flip_d(self) -> bool {
        self.0 & Self::FLIP_D != 0
    }
}

/// Grilla de tiles de un cel; medidas en tiles, fila por fila.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileGrid {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
}

impl TileGrid {
    /// Grilla con todas las celdas vacías.
    pub fn new(width: u32, height: u32) -> TileGrid {
        TileGrid {
            width,
            height,
            tiles: vec![Tile::EMPTY; width as usize * height as usize],
        }
    }

    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height).then(|| y as usize * self.width as usize + x as usize)
    }

    pub fn get(&self, x: u32, y: u32) -> Option<Tile> {
        self.tiles.get(self.offset(x, y)?).copied()
    }

    /// `false` si la celda cae fuera de la grilla.
    pub fn set(&mut self, x: u32, y: u32, tile: Tile) -> bool {
        match self.offset(x, y).and_then(|i| self.tiles.get_mut(i)) {
            Some(t) => {
                *t = tile;
                true
            }
            None => false,
        }
    }
}

/// Flips con los que un tile puede reconocerse como otro (los usa el modo Auto del subproyecto 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowedFlips {
    pub x: bool,
    pub y: bool,
    pub d: bool,
}

/// Imágenes de los tiles que usan las capas tilemap de un sprite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tileset {
    pub name: String,
    pub tile_size: (u32, u32),
    /// Número que ve el usuario para el tile 1 (Aseprite usa 1 por defecto).
    pub base_index: i16,
    #[serde(default)]
    pub allowed_flips: AllowedFlips,
    /// El 0 es el tile vacío: siempre existe y es transparente. Imágenes en el modo de color del
    /// sprite y del tamaño `tile_size`.
    pub tiles: Vec<Arc<Image>>,
    /// User data del tileset y de cada tile, sin interpretar (para el round-trip exacto).
    #[serde(default)]
    pub user_data: Option<RawChunk>,
    #[serde(default)]
    pub tile_user_data: Vec<Option<RawChunk>>,
}

impl Tileset {
    /// Tileset con solo el tile 0 (vacío).
    pub fn new(name: impl Into<String>, tile_size: (u32, u32), color_mode: ColorMode) -> Tileset {
        Tileset {
            name: name.into(),
            tile_size,
            base_index: 1,
            allowed_flips: AllowedFlips::default(),
            tiles: vec![Arc::new(Image::new(tile_size.0, tile_size.1, color_mode))],
            user_data: None,
            tile_user_data: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cel, ImageId, LayerId, LayerKind, PixelValue, Sprite};

    #[test]
    fn tiles_pack_index_and_flips_like_aseprite() {
        let t = Tile::new(5, true, false, true);
        assert_eq!(t.0, 5 | 0x2000_0000 | 0x8000_0000);
        assert_eq!(
            (t.index(), t.flip_x(), t.flip_y(), t.flip_d()),
            (5, true, false, true)
        );
        assert_eq!(Tile::new(0, false, true, false).0, 0x4000_0000);
        assert_eq!(
            Tile::new(0x3fff_ffff, false, false, false).index(),
            0x1fff_ffff,
            "the index is masked"
        );
        assert_eq!(Tile::EMPTY.index(), 0);
    }

    #[test]
    fn grid_get_and_set_respect_bounds() {
        let mut g = TileGrid::new(3, 2);
        assert_eq!(g.tiles.len(), 6);
        assert_eq!(g.get(2, 1), Some(Tile::EMPTY));
        assert!(g.set(2, 1, Tile(7)));
        assert_eq!(g.get(2, 1), Some(Tile(7)));
        assert_eq!(g.tiles[5], Tile(7), "row by row");
        assert!(!g.set(3, 0, Tile(1)));
        assert!(!g.set(0, 2, Tile(1)));
        assert_eq!(g.get(3, 0), None);
        // Una grilla corrupta (menos tiles que celdas) no entra en pánico.
        let broken = TileGrid {
            width: 2,
            height: 2,
            tiles: vec![Tile(1)],
        };
        assert_eq!(broken.get(1, 1), None);
    }

    #[test]
    fn new_tileset_has_the_empty_tile() {
        let ts = Tileset::new(
            "t",
            (4, 2),
            ColorMode::Indexed {
                transparent_index: 3,
            },
        );
        assert_eq!(ts.tiles.len(), 1);
        assert_eq!((ts.tiles[0].width(), ts.tiles[0].height()), (4, 2));
        assert_eq!(ts.tiles[0].get(0, 0), Some(PixelValue::Index(3)));
        assert_eq!(ts.base_index, 1);
        assert_eq!(ts.allowed_flips, AllowedFlips::default());
        assert!(ts.user_data.is_none() && ts.tile_user_data.is_empty());
    }

    #[test]
    fn image_ids_are_shared_with_tilemaps() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        s.tilemaps.insert(ImageId(9), Arc::new(TileGrid::new(1, 1)));
        assert_eq!(s.next_image_id(), ImageId(10));
        s.images
            .insert(ImageId(12), Arc::new(Image::new(1, 1, ColorMode::Rgba)));
        assert_eq!(s.next_image_id(), ImageId(13));
        assert!(s.tilemap(ImageId(9)).is_some());
        assert!(s.tilemap(ImageId(12)).is_none());
    }

    #[test]
    fn sprites_with_tilemaps_serialize_and_old_sprites_still_load() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        let mut ts = Tileset::new("ground", (4, 4), ColorMode::Rgba);
        ts.tiles.push(Arc::new(Image::new(4, 4, ColorMode::Rgba)));
        ts.user_data = Some(RawChunk {
            kind: 0x2020,
            data: vec![1, 0, 0, 0, 2, 0, b'h', b'i'],
        });
        ts.tile_user_data = vec![
            None,
            Some(RawChunk {
                kind: 0x2020,
                data: vec![0; 4],
            }),
        ];
        ts.allowed_flips.x = true;
        s.tilesets.push(ts);
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        let mut grid = TileGrid::new(2, 1);
        grid.set(1, 0, Tile::new(1, true, false, false));
        s.tilemaps.insert(ImageId(1), Arc::new(grid));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (4, 0),
                opacity: 255,
                image: ImageId(1),
            },
        );
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Sprite>(&json).unwrap(), s);

        // Un sprite guardado antes de los tilemaps (sin esos campos) sigue cargando.
        let mut old = serde_json::to_value(Sprite::new(2, 2, ColorMode::Rgba)).unwrap();
        let obj = old.as_object_mut().unwrap();
        obj.remove("tilesets");
        obj.remove("tilemaps");
        obj["layers"][0].as_object_mut().unwrap().remove("tileset");
        let back: Sprite = serde_json::from_value(old).unwrap();
        assert!(back.tilesets.is_empty() && back.tilemaps.is_empty());
        assert_eq!(back.layers[0].tileset, None);
    }
}
