//! Tilemaps: tilesets (las imágenes de los tiles) y las grillas de tiles de los cels.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::{ColorMode, Image, PixelValue, RawChunk};

/// Píxeles máximos de un cel tilemap (ancho × alto de la grilla en píxeles, 64 Mpx). Una grilla
/// chica con tiles grandes ocupa poco en el archivo pero mucho al dibujarse: el lector descarta
/// los cels más grandes y los comandos no dejan crearlos.
pub const MAX_TILEMAP_PIXELS: u64 = 1 << 26;

/// Píxeles máximos de un tile (4 Mpx, p. ej. 2048×2048: 16 MiB en RGBA). El tamaño de tile viene
/// del archivo y cada tileset reserva al menos el tile 0, así que se valida antes de asignar.
/// Alcanza para cualquier uso real (Aseprite crea tiles de 16×16 por defecto) y deja que una
/// grilla de esos tiles todavía quepa en `MAX_TILEMAP_PIXELS`.
pub const MAX_TILE_PIXELS: u64 = 1 << 22;

/// Tiles máximos de un tileset (incluido el vacío). Cada tile es una imagen aparte: con tiles de
/// 1×1 la cuenta dispararía millones de asignaciones. Lo usan el lector de `.aseprite` y la
/// conversión de capas a tilemap.
pub const MAX_TILESET_TILES: u32 = 1 << 16;

/// Valor de una celda de tilemap, con el mismo layout que Aseprite:
/// bits 0–28 índice del tile, bit 31 flip X, bit 30 flip Y, bit 29 flip D (transponer).
/// Coincide con las máscaras estándar de Aseprite y con los flags del GID de Tiled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile(pub u32);

impl Tile {
    pub const EMPTY: Tile = Tile(0);
    pub const INDEX_MASK: u32 = 0x1fff_ffff;
    pub const FLIP_X: u32 = 1 << 31;
    pub const FLIP_Y: u32 = 1 << 30;
    pub const FLIP_D: u32 = 1 << 29;

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

    /// Imagen que dibuja `tile`, o `None` si la celda no dibuja nada: tile vacío, índice fuera de
    /// rango, imagen de otro tamaño o flip D en un tile no cuadrado.
    pub fn drawn_tile(&self, tile: Tile) -> Option<&Image> {
        let index = tile.index() as usize;
        if index == 0 {
            return None;
        }
        let image = self.tiles.get(index)?;
        let (tw, th) = self.tile_size;
        if (image.width(), image.height()) != (tw, th) {
            return None;
        }
        if tile.flip_d() && tw != th {
            if !WARNED_DIAGONAL.swap(true, Ordering::Relaxed) {
                log::warn!("diagonal flip on non-square tiles ({tw}×{th}) is not drawn");
            }
            return None;
        }
        Some(image)
    }
}

/// Avisado una sola vez: flip D en tiles no cuadrados.
static WARNED_DIAGONAL: AtomicBool = AtomicBool::new(false);

/// Píxel del tile original que se ve en `(x, y)` del tile volteado. Equivale a aplicar a la imagen
/// D (transponer), después X y después Y; el orden lo confirma `tilemap_file_renders_like_aseprite`
/// en `cs-io/tests/aseprite_interop.rs`.
fn tile_source(tile: Tile, w: u32, h: u32, x: u32, y: u32) -> (u32, u32) {
    let mut sx = if tile.flip_x() { w - 1 - x } else { x };
    let mut sy = if tile.flip_y() { h - 1 - y } else { y };
    if tile.flip_d() {
        std::mem::swap(&mut sx, &mut sy);
    }
    (sx, sy)
}

/// Valor que dibuja la grilla en `(lx, ly)`, relativo a su esquina: una sola búsqueda de tile.
/// `None` fuera de la grilla, en celdas que no dibujan (ver `Tileset::drawn_tile`) o si el tile es
/// de otro modo de color (datos corruptos).
pub fn tile_grid_pixel(
    mode: ColorMode,
    tileset: &Tileset,
    grid: &TileGrid,
    lx: i64,
    ly: i64,
) -> Option<PixelValue> {
    let (tw, th) = tileset.tile_size;
    if lx < 0 || ly < 0 || tw == 0 || th == 0 {
        return None;
    }
    let gx = u32::try_from(lx / tw as i64).ok()?;
    let gy = u32::try_from(ly / th as i64).ok()?;
    let tile = grid.get(gx, gy)?;
    let src = tileset.drawn_tile(tile)?;
    let (sx, sy) = tile_source(
        tile,
        tw,
        th,
        (lx % tw as i64) as u32,
        (ly % th as i64) as u32,
    );
    let value = src.get(sx, sy)?;
    let same_mode = matches!(
        (value, mode),
        (PixelValue::Rgba(_), ColorMode::Rgba) | (PixelValue::Index(_), ColorMode::Indexed { .. })
    );
    same_mode.then_some(value)
}

/// La grilla entera dibujada (sin recortar), con cada tile copiado como valores y sus flips.
/// `None` si la grilla no tiene celdas o pasaría `MAX_TILEMAP_PIXELS`.
pub fn draw_tile_grid(mode: ColorMode, tileset: &Tileset, grid: &TileGrid) -> Option<Image> {
    let (tw, th) = tileset.tile_size;
    let w = grid.width as u64 * tw as u64;
    let h = grid.height as u64 * th as u64;
    if w == 0 || h == 0 || w * h > MAX_TILEMAP_PIXELS {
        return None;
    }
    let mut image = Image::new(w as u32, h as u32, mode);
    for y in 0..h as u32 {
        for x in 0..w as u32 {
            if let Some(v) = tile_grid_pixel(mode, tileset, grid, x as i64, y as i64) {
                image.set(x, y, v).expect("in bounds and same mode");
            }
        }
    }
    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cel, ImageId, LayerId, LayerKind, PixelValue, Sprite};

    #[test]
    fn tiles_pack_index_and_flips_like_aseprite() {
        let t = Tile::new(5, true, false, true);
        assert_eq!(t.0, 5 | Tile::FLIP_X | Tile::FLIP_D);
        assert_eq!(t.0, 5 | 0x8000_0000 | 0x2000_0000);
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

    #[test]
    fn tile_grids_draw_with_flips_and_skip_cells_that_draw_nothing() {
        let mode = ColorMode::Rgba;
        let red = PixelValue::Rgba(crate::Rgba::rgb(255, 0, 0));
        let mut ts = Tileset::new("t", (2, 2), mode);
        let mut img = Image::new(2, 2, mode);
        img.set(0, 0, red).unwrap();
        ts.tiles.push(Arc::new(img));
        let mut grid = TileGrid::new(3, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        grid.set(1, 0, Tile::new(1, true, true, false));
        grid.set(2, 0, Tile::new(9, false, false, false));
        let out = draw_tile_grid(mode, &ts, &grid).unwrap();
        assert_eq!((out.width(), out.height()), (6, 2));
        assert_eq!(out.get(0, 0), Some(red));
        assert_eq!(
            out.get(3, 1),
            Some(red),
            "X+Y flips move it to the opposite corner"
        );
        assert_eq!(
            out.get(4, 0),
            Some(mode.transparent()),
            "tile 9 does not exist"
        );
        assert_eq!(tile_grid_pixel(mode, &ts, &grid, 4, 0), None);
        assert!(draw_tile_grid(mode, &ts, &TileGrid::new(0, 0)).is_none());
        // Flip D en tiles no cuadrados: no dibuja.
        let mut wide = Tileset::new("w", (4, 2), mode);
        wide.tiles.push(Arc::new(Image::new(4, 2, mode)));
        let mut g = TileGrid::new(1, 1);
        g.set(0, 0, Tile::new(1, false, false, true));
        assert!(wide.drawn_tile(Tile::new(1, false, false, false)).is_some());
        assert!(wide.drawn_tile(Tile::new(1, false, false, true)).is_none());
        assert_eq!(tile_grid_pixel(mode, &wide, &g, 0, 0), None);
        assert_eq!(MAX_TILESET_TILES, 1 << 16);
    }
}
