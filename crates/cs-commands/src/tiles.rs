//! Píxeles sobre capas tilemap con los modos de tileset de Aseprite (Manual, Auto y Stack) y
//! reordenar los tiles de un tileset. El comportamiento de referencia lo fijan los escenarios de
//! Aseprite en `tests/fixtures/tile_pixels.*` (ver `tests/tile_pixels_aseprite.rs`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use cs_core::{
    AllowedFlips, Cel, ColorMode, Image, ImageId, LayerId, LayerKind, MAX_TILESET_TILES,
    PixelBuffer, PixelValue, Sprite, Tile, TileGrid, orient_tile, unorient_tile,
};
use serde::{Deserialize, Serialize};

use crate::Command;
use crate::ops::{MAX_TILEMAP_SIDE, tilemap_fits};

/// Qué pasa con el tileset al pintar píxeles sobre una capa tilemap (como en Aseprite).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TilesetMode {
    /// Se modifica el tile de cada celda (todas sus instancias cambian); no se crean tiles.
    Manual,
    /// Se crean tiles si hace falta, se reutilizan los iguales y se borran los que quedan sin uso.
    #[default]
    Auto,
    /// Cada celda modificada pasa a un tile nuevo al final (o a uno igual que ya exista).
    Stack,
}

impl TilesetMode {
    pub const ALL: [TilesetMode; 3] = [TilesetMode::Manual, TilesetMode::Auto, TilesetMode::Stack];
}

/// Por qué `draw_tilemap_pixels` no devolvió un comando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TilePaintError {
    /// La capa no es tilemap o su tileset no existe.
    NotTilemap,
    /// Nada cambia (ningún píxel cae en el lienzo o todos pintan lo que ya había).
    Nothing,
    /// La grilla pasaría `MAX_TILEMAP_SIDE` / `tilemap_fits` o el tileset `MAX_TILESET_TILES`.
    TooLarge,
}

/// Orden en que se prueban los flips al buscar un tile igual: sin flip, X, Y, X+Y, D, D+X, D+Y,
/// D+X+Y (con un tile simétrico gana el primero, como en Aseprite).
const FLIP_ORDER: [u32; 8] = [
    0,
    Tile::FLIP_X,
    Tile::FLIP_Y,
    Tile::FLIP_X | Tile::FLIP_Y,
    Tile::FLIP_D,
    Tile::FLIP_D | Tile::FLIP_X,
    Tile::FLIP_D | Tile::FLIP_Y,
    Tile::FLIP_D | Tile::FLIP_X | Tile::FLIP_Y,
];

/// Un píxel de un trazo dentro de su celda: posición en el tile (como se ve) y valor.
type CellPixel = (u32, u32, PixelValue);

/// Un píxel que no se ve: alfa 0 en RGBA, el índice transparente en indexado.
fn is_clear(value: PixelValue, mode: ColorMode) -> bool {
    match value {
        PixelValue::Rgba(c) => c.a == 0,
        v => v == mode.transparent(),
    }
}

/// Lleva los píxeles invisibles al transparente del modo (como `preprocess_transparent_pixels`
/// de Aseprite): así dos tiles que se ven igual son iguales.
fn normalize(image: &mut Image, mode: ColorMode) {
    let clear = mode.transparent();
    for y in 0..image.height() {
        for x in 0..image.width() {
            let v = image.get(x, y).expect("in bounds");
            if v != clear && is_clear(v, mode) {
                image.set(x, y, clear).expect("in bounds and same mode");
            }
        }
    }
}

/// Clave para comparar tiles por cómo se ven (los invisibles cuentan como transparentes).
fn key(image: &Image) -> Vec<u8> {
    match image.pixels() {
        PixelBuffer::Rgba(px) => px
            .iter()
            .flat_map(|c| {
                if c.a == 0 {
                    [0; 4]
                } else {
                    [c.r, c.g, c.b, c.a]
                }
            })
            .collect(),
        PixelBuffer::Indexed(px) => px.clone(),
    }
}

fn is_transparent(image: &Image, mode: ColorMode) -> bool {
    (0..image.height())
        .all(|y| (0..image.width()).all(|x| image.get(x, y).is_none_or(|v| is_clear(v, mode))))
}

/// Busca tiles iguales a una imagen (exactos o con los flips permitidos). Guarda, por clave de
/// píxeles, el menor índice con esa imagen; el vacío (0) no entra.
struct Finder {
    map: HashMap<Vec<u8>, u32>,
    flips: AllowedFlips,
}

impl Finder {
    fn new(tiles: &[Arc<Image>], flips: AllowedFlips) -> Finder {
        let mut finder = Finder {
            map: HashMap::new(),
            flips,
        };
        for (i, tile) in tiles.iter().enumerate().skip(1) {
            finder.add(i as u32, tile);
        }
        finder
    }

    fn add(&mut self, index: u32, image: &Image) {
        let e = self.map.entry(key(image)).or_insert(index);
        *e = (*e).min(index);
    }

    /// El tile `index` pasó de `old` a `new` (`tiles` ya tiene la imagen nueva).
    fn replace(&mut self, index: u32, old: &Image, tiles: &[Arc<Image>]) {
        let old_key = key(old);
        if self.map.get(&old_key) == Some(&index) {
            self.map.remove(&old_key);
            // Si otro tile tenía la misma imagen, pasa a ser el de esa clave.
            if let Some(j) = (1..tiles.len())
                .filter(|&j| j != index as usize)
                .find(|&j| key(&tiles[j]) == old_key)
            {
                self.map.insert(old_key, j as u32);
            }
        }
        self.add(index, &tiles[index as usize]);
    }

    /// Un tile que, dibujado con sus flips, se ve como `seen`.
    fn find(&self, seen: &Image) -> Option<Tile> {
        let square = seen.width() == seen.height();
        FLIP_ORDER
            .iter()
            .filter(|&&f| {
                (f & Tile::FLIP_X == 0 || self.flips.x)
                    && (f & Tile::FLIP_Y == 0 || self.flips.y)
                    && (f & Tile::FLIP_D == 0 || (self.flips.d && square))
            })
            .find_map(|&f| {
                let source = if f == 0 {
                    seen.clone()
                } else {
                    unorient_tile(seen, Tile(f))
                };
                self.map.get(&key(&source)).map(|&i| Tile(i | f))
            })
    }
}

/// Ids de las grillas que usan las capas tilemap con el tileset `tileset` (los cels vinculados
/// comparten una).
fn tileset_grids(sprite: &Sprite, tileset: usize) -> BTreeSet<ImageId> {
    let layers: BTreeSet<LayerId> = sprite
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Tilemap && l.tileset == Some(tileset as u32))
        .map(|l| l.id)
        .collect();
    sprite
        .cels
        .iter()
        .filter(|((layer, _), _)| layers.contains(layer))
        .map(|(_, c)| c.image)
        .filter(|id| sprite.tilemap(*id).is_some())
        .collect()
}

/// La grilla con cada tile no vacío pasado por `f` (los flips se conservan); `None` si no cambia.
fn remap_grid(grid: &TileGrid, f: &impl Fn(u32) -> u32) -> Option<TileGrid> {
    let mut out = grid.clone();
    for t in &mut out.tiles {
        if t.index() != 0 {
            *t = t.with_index(f(t.index()));
        }
    }
    (out != *grid).then_some(out)
}

/// Lo que dibuja `tile` (con sus flips) si se ve: tile no vacío, en rango, del tamaño del tileset
/// y sin flip D en tiles no cuadrados.
fn shown(tiles: &[Arc<Image>], tile: Tile, size: (u32, u32)) -> Option<Image> {
    let index = tile.index() as usize;
    let image = tiles.get(index).filter(|_| index != 0)?;
    if (image.width(), image.height()) != size || (tile.flip_d() && size.0 != size.1) {
        return None;
    }
    Some(orient_tile(image, tile))
}

/// La grilla con `cells` puestas (celdas del lienzo; la grilla empieza en la celda `origin`),
/// agrandada para que entren, y la celda donde queda su esquina. Sin grilla, la mínima que abarca
/// las celdas.
fn place_cells(
    grid: Option<&TileGrid>,
    origin: (i32, i32),
    cells: &[(i32, i32, Tile)],
    tile_size: (u32, u32),
) -> Result<(TileGrid, (i32, i32)), TilePaintError> {
    let (mut x0, mut y0, mut x1, mut y1) = match grid {
        Some(g) => (
            origin.0 as i64,
            origin.1 as i64,
            origin.0 as i64 + g.width as i64,
            origin.1 as i64 + g.height as i64,
        ),
        None => (i64::MAX, i64::MAX, i64::MIN, i64::MIN),
    };
    for &(x, y, _) in cells {
        x0 = x0.min(x as i64);
        y0 = y0.min(y as i64);
        x1 = x1.max(x as i64 + 1);
        y1 = y1.max(y as i64 + 1);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    if w <= 0
        || h <= 0
        || w > MAX_TILEMAP_SIDE as i64
        || h > MAX_TILEMAP_SIDE as i64
        || !tilemap_fits(w as u64, h as u64, tile_size)
    {
        return Err(TilePaintError::TooLarge);
    }
    let mut out = TileGrid::new(w as u32, h as u32);
    if let Some(g) = grid {
        let (dx, dy) = ((origin.0 as i64 - x0) as u32, (origin.1 as i64 - y0) as u32);
        for y in 0..g.height {
            for x in 0..g.width {
                out.set(x + dx, y + dy, g.get(x, y).unwrap_or(Tile::EMPTY));
            }
        }
    }
    for &(x, y, tile) in cells {
        out.set((x as i64 - x0) as u32, (y as i64 - y0) as u32, tile);
    }
    Ok((out, (x0 as i32, y0 as i32)))
}

/// Pinta `pixels` (píxeles del lienzo, en el modo de color del sprite) sobre la capa tilemap
/// `layer` en `frame`, actualizando el tileset según `mode` como Aseprite (ver el spec
/// `2026-11-06-tilemaps-pixels-design.md`, sección 1). Las celdas se procesan fila por fila con
/// el tileset tal como va quedando. Un solo `Batch` de primitivas: `SetTileset`, los
/// `SetTilemap` de las grillas que cambian (la del cel y, si Auto borró tiles, las remapeadas) y
/// los `SetCel` si el cel se crea o se corre (los vinculados se corren juntos: comparten la
/// grilla, que se edita en el lugar). Se ignoran los píxeles fuera del lienzo o de otro modo.
pub fn draw_tilemap_pixels(
    sprite: &Sprite,
    layer: LayerId,
    frame: usize,
    pixels: &[(i32, i32, PixelValue)],
    mode: TilesetMode,
) -> Result<Command, TilePaintError> {
    let l = sprite
        .layer(layer)
        .filter(|l| l.kind == LayerKind::Tilemap)
        .ok_or(TilePaintError::NotTilemap)?;
    let ts_index = l.tileset.ok_or(TilePaintError::NotTilemap)? as usize;
    let ts = sprite
        .tilesets
        .get(ts_index)
        .ok_or(TilePaintError::NotTilemap)?;
    let (tw, th) = ts.tile_size;
    if tw == 0 || th == 0 {
        return Err(TilePaintError::NotTilemap);
    }
    if frame >= sprite.frames.len() {
        return Err(TilePaintError::Nothing);
    }
    let color = sprite.color_mode;
    let cel = sprite.cel(layer, frame).copied();
    // Un cel que apunta a una grilla que no existe es un documento roto: no se toca.
    let grid = match cel {
        Some(c) => Some(&**sprite.tilemap(c.image).ok_or(TilePaintError::Nothing)?),
        None => None,
    };
    let (tw_i, th_i) = (tw as i32, th as i32);
    let position = cel.map_or((0, 0), |c| c.position);
    let offset = (position.0.rem_euclid(tw_i), position.1.rem_euclid(th_i));
    // Celda del lienzo donde empieza la grilla (como en `ops::set_tiles`).
    let origin = (position.0.div_euclid(tw_i), position.1.div_euclid(th_i));

    // Los píxeles por celda, en orden de filas (y, x).
    let mut by_cell: BTreeMap<(i32, i32), Vec<CellPixel>> = BTreeMap::new();
    let same_mode = |v: PixelValue| {
        matches!(
            (v, color),
            (PixelValue::Rgba(_), ColorMode::Rgba)
                | (PixelValue::Index(_), ColorMode::Indexed { .. })
        )
    };
    for &(x, y, v) in pixels {
        if x < 0 || y < 0 || x as u32 >= sprite.width || y as u32 >= sprite.height || !same_mode(v)
        {
            continue;
        }
        let (cx, cy) = (
            (x - offset.0).div_euclid(tw_i),
            (y - offset.1).div_euclid(th_i),
        );
        let (lx, ly) = (x - cx * tw_i - offset.0, y - cy * th_i - offset.1);
        by_cell
            .entry((cy, cx))
            .or_default()
            .push((lx as u32, ly as u32, v));
    }
    if by_cell.is_empty() {
        return Err(TilePaintError::Nothing);
    }
    // Sin cel, Manual hace lo mismo que Stack (Aseprite crea el cel y los tiles).
    let mode = match (mode, cel) {
        (TilesetMode::Manual, None) => TilesetMode::Stack,
        (mode, _) => mode,
    };
    let current = |cx: i32, cy: i32| -> Tile {
        let found = grid.and_then(|g| {
            let gx = u32::try_from(cx - origin.0).ok()?;
            let gy = u32::try_from(cy - origin.1).ok()?;
            g.get(gx, gy)
        });
        found.unwrap_or(Tile::EMPTY)
    };

    let grid_ids = tileset_grids(sprite, ts_index);
    let mut tiles = ts.tiles.clone();
    let mut finder = Finder::new(&tiles, ts.allowed_flips);
    // Usos de cada tile en todas las grillas del tileset (solo los necesita Auto).
    let mut usage = vec![0u32; tiles.len()];
    if mode == TilesetMode::Auto {
        for id in &grid_ids {
            for t in &sprite.tilemap(*id).expect("listed").tiles {
                if let Some(n) = usage.get_mut(t.index() as usize) {
                    *n += 1;
                }
            }
        }
    }
    let mut touched = BTreeSet::new();
    let mut changes: Vec<(i32, i32, Tile)> = Vec::new();
    for ((cy, cx), px) in by_cell {
        let t = current(cx, cy);
        let before = shown(&tiles, t, (tw, th));
        let old = before.clone().unwrap_or_else(|| Image::new(tw, th, color));
        let mut new = old.clone();
        for (lx, ly, v) in px {
            new.set(lx, ly, v).expect("in the tile and same mode");
        }
        normalize(&mut new, color);
        if key(&new) == key(&old) {
            continue;
        }
        if mode == TilesetMode::Manual {
            // Solo celdas con un tile que se ve; se escribe a través de sus flips.
            if before.is_none() {
                continue;
            }
            let i = t.index() as usize;
            let previous = std::mem::replace(&mut tiles[i], Arc::new(unorient_tile(&new, t)));
            finder.replace(i as u32, &previous, &tiles);
            continue;
        }
        let next = if is_transparent(&new, color) {
            Tile::EMPTY
        } else if let Some(found) = finder.find(&new) {
            found
        } else if mode == TilesetMode::Auto
            && before.is_some()
            && usage.get(t.index() as usize) == Some(&1)
        {
            // Solo lo usa esta celda: se modifica en el lugar, como se ve (sin flips).
            let i = t.index() as usize;
            let previous = std::mem::replace(&mut tiles[i], Arc::new(new));
            finder.replace(i as u32, &previous, &tiles);
            Tile(i as u32)
        } else {
            if tiles.len() as u32 >= MAX_TILESET_TILES {
                return Err(TilePaintError::TooLarge);
            }
            let i = tiles.len() as u32;
            tiles.push(Arc::new(new));
            usage.push(0);
            finder.add(i, &tiles[i as usize]);
            Tile(i)
        };
        if mode == TilesetMode::Auto {
            if let Some(n) = usage.get_mut(t.index() as usize).filter(|_| t.index() != 0) {
                *n = n.saturating_sub(1);
            }
            if next.index() != 0 {
                usage[next.index() as usize] += 1;
            }
            touched.insert(t.index());
        }
        if next != t {
            changes.push((cx, cy, next));
        }
    }

    // Auto borra los tiles viejos que quedaron sin uso (no otros que ya estuvieran sin usar).
    let removed: Vec<u32> = touched
        .into_iter()
        .filter(|&i| i != 0 && (i as usize) < ts.tiles.len() && usage[i as usize] == 0)
        .collect();
    let remap = |i: u32| i - removed.iter().filter(|&&r| r < i).count() as u32;
    let mut new_ts = ts.clone();
    new_ts.tiles = tiles
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !removed.contains(&(*i as u32)))
        .map(|(_, t)| t)
        .collect();
    for &r in removed.iter().rev() {
        if (r as usize) < new_ts.tile_user_data.len() {
            new_ts.tile_user_data.remove(r as usize);
        }
    }

    let mut cmds = Vec::new();
    if new_ts != *ts {
        cmds.push(Command::SetTileset {
            index: ts_index,
            tileset: Some(new_ts),
        });
    }
    let mut cels = Vec::new();
    // La grilla del cel: con las celdas nuevas (agrandada si hace falta) y remapeada.
    let target = cel.map(|c| c.image);
    if !changes.is_empty() || (!removed.is_empty() && target.is_some()) {
        let (mut placed, corner) = if changes.is_empty() {
            (grid.expect("cel has a grid").clone(), origin)
        } else {
            place_cells(grid, origin, &changes, ts.tile_size)?
        };
        if !removed.is_empty()
            && let Some(g) = remap_grid(&placed, &remap)
        {
            placed = g;
        }
        let new_position = match cel {
            Some(_) => (
                position.0 + (corner.0 - origin.0) * tw_i,
                position.1 + (corner.1 - origin.1) * th_i,
            ),
            None => (corner.0 * tw_i, corner.1 * th_i),
        };
        let id = target.unwrap_or_else(|| sprite.next_image_id());
        if grid != Some(&placed) {
            cmds.push(Command::SetTilemap {
                id,
                grid: Some(Arc::new(placed)),
            });
        }
        match cel {
            None => cels.push(Command::SetCel {
                layer,
                frame,
                cel: Some(Cel {
                    position: new_position,
                    opacity: 255,
                    image: id,
                }),
            }),
            Some(c) if new_position != c.position => {
                // Los cels vinculados comparten la grilla: se corren juntos.
                for (&(l, f), linked) in sprite.cels.iter().filter(|(_, o)| o.image == id) {
                    cels.push(Command::SetCel {
                        layer: l,
                        frame: f,
                        cel: Some(Cel {
                            position: new_position,
                            ..*linked
                        }),
                    });
                }
            }
            Some(_) => {}
        }
    }
    // Las demás grillas del tileset, remapeadas si se borraron tiles.
    if !removed.is_empty() {
        for id in grid_ids.iter().filter(|id| Some(**id) != target) {
            let g = sprite.tilemap(*id).expect("listed");
            if let Some(g) = remap_grid(g, &remap) {
                cmds.push(Command::SetTilemap {
                    id: *id,
                    grid: Some(Arc::new(g)),
                });
            }
        }
    }
    cmds.extend(cels);
    if cmds.is_empty() {
        return Err(TilePaintError::Nothing);
    }
    Ok(Command::Batch(cmds))
}

/// Mueve el tile `from` del tileset `tileset` a la posición `to` (los del medio se corren) y
/// remapea las grillas de todas las capas que usan el tileset (los flips se conservan); el user
/// data de cada tile va con él. Un solo `Batch`. `None` si el tileset no existe, alguno es el
/// tile 0 o está fuera de rango, o `from == to`.
pub fn move_tile(sprite: &Sprite, tileset: usize, from: u32, to: u32) -> Option<Command> {
    let ts = sprite.tilesets.get(tileset)?;
    let n = ts.tiles.len() as u32;
    if from == 0 || to == 0 || from >= n || to >= n || from == to {
        return None;
    }
    // `order[nuevo] = viejo`.
    let mut order: Vec<u32> = (0..n).collect();
    let moved = order.remove(from as usize);
    order.insert(to as usize, moved);
    let mut new_index = vec![0u32; n as usize];
    for (new, &old) in order.iter().enumerate() {
        new_index[old as usize] = new as u32;
    }
    let mut new_ts = ts.clone();
    new_ts.tiles = order
        .iter()
        .map(|&i| ts.tiles[i as usize].clone())
        .collect();
    if !ts.tile_user_data.is_empty() {
        let mut data = ts.tile_user_data.clone();
        data.resize(n as usize, None);
        let mut moved: Vec<_> = order.iter().map(|&i| data[i as usize].clone()).collect();
        // Sin los vacíos que se agregaron al final (el largo original basta para el archivo).
        while moved.len() > ts.tile_user_data.len() && moved.last() == Some(&None) {
            moved.pop();
        }
        new_ts.tile_user_data = moved;
    }
    let mut cmds = vec![Command::SetTileset {
        index: tileset,
        tileset: Some(new_ts),
    }];
    let remap = |i: u32| new_index.get(i as usize).copied().unwrap_or(i);
    for id in tileset_grids(sprite, tileset) {
        let g = sprite.tilemap(id).expect("listed");
        if let Some(g) = remap_grid(g, &remap) {
            cmds.push(Command::SetTilemap {
                id,
                grid: Some(Arc::new(g)),
            });
        }
    }
    Some(Command::Batch(cmds))
}

#[cfg(test)]
mod tests;
