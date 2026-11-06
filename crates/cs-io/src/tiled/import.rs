//! Mapa de Tiled → sprite con capas tilemap (un frame, RGBA).

use std::sync::Arc;

use cs_core::{
    BlendMode, Cel, ColorMode, Image, Layer, LayerId, LayerKind, MAX_TILE_PIXELS,
    MAX_TILEMAP_PIXELS, MAX_TILESET_TILES, PixelValue, Rgba, Slice, SliceKey, Sprite, Tile,
    TileGrid, Tileset,
};

use super::{
    FLIP_FLAGS, LayerContent, Map, MapLayer, MapObject, PropertyValue, TiledImage, TiledTileset,
    invalid, read_tmj, read_tmx, read_tsj, read_tsx, resolve_path,
};
use crate::Result;
use crate::export::Bitmap;
use crate::import::decode_png;

/// ¿El nombre es de un mapa de Tiled (`.tmx` o `.tmj`)?
pub fn is_tiled_map(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".tmx") || lower.ends_with(".tmj")
}

/// Cargador de los archivos vecinos (tilesets externos e imágenes); recibe rutas relativas a la
/// carpeta del mapa, con `/`.
pub type Loader<'a> = dyn FnMut(&str) -> Option<Vec<u8>> + 'a;

/// Lee un mapa (`name` decide el formato por la extensión). Devuelve el sprite y los avisos de lo
/// que no se pudo traer.
pub fn import(name: &str, bytes: &[u8], load: &mut Loader) -> Result<(Sprite, Vec<String>)> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| invalid("the map is not a UTF-8 text file"))?;
    let map = if name.to_ascii_lowercase().ends_with(".tmj") {
        read_tmj(text)?
    } else {
        read_tmx(text)?
    };
    let file_name = name
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .to_string();
    Importer::new(&file_name, load).run(map)
}

struct Importer<'a, 'b> {
    map_file: String,
    load: &'a mut Loader<'b>,
    warnings: Vec<String>,
    sprite: Sprite,
    /// `(firstgid, cantidad de tiles)` de cada tileset, en el orden de `sprite.tilesets`.
    ranges: Vec<(u32, u32)>,
}

fn blend(mode: Option<&str>) -> BlendMode {
    match mode {
        Some("multiply") => BlendMode::Multiply,
        Some("screen") => BlendMode::Screen,
        Some("overlay") => BlendMode::Overlay,
        Some("darken") => BlendMode::Darken,
        Some("lighten") => BlendMode::Lighten,
        Some("add") => BlendMode::Addition,
        Some("difference") => BlendMode::Difference,
        _ => BlendMode::Normal,
    }
}

impl<'a, 'b> Importer<'a, 'b> {
    fn new(map_file: &str, load: &'a mut Loader<'b>) -> Self {
        let mut sprite = Sprite::new(1, 1, ColorMode::Rgba);
        sprite.layers.clear();
        Self {
            map_file: map_file.into(),
            load,
            warnings: Vec::new(),
            sprite,
            ranges: Vec::new(),
        }
    }

    fn warn(&mut self, message: String) {
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }

    fn run(mut self, map: Map) -> Result<(Sprite, Vec<String>)> {
        if map.orientation != "orthogonal" {
            return Err(invalid(format!(
                "{} maps are not supported, only orthogonal ones",
                map.orientation
            )));
        }
        if map.infinite {
            return Err(invalid("infinite maps are not supported yet"));
        }
        let (w, h) = (
            map.width as u64 * map.tile_width as u64,
            map.height as u64 * map.tile_height as u64,
        );
        if w == 0 || h == 0 || w * h > MAX_TILEMAP_PIXELS {
            return Err(invalid(format!("the map is too big or empty ({w}×{h} px)")));
        }
        self.sprite.width = w as u32;
        self.sprite.height = h as u32;
        self.sprite.grid.size = (map.tile_width, map.tile_height);
        for ts in &map.tilesets {
            let (tiled, base) = match &ts.source {
                Some(source) => {
                    let path = resolve_path(&self.map_file, source);
                    let bytes = (self.load)(&path).ok_or_else(|| {
                        invalid(format!("tileset file '{path}' not found next to the map"))
                    })?;
                    let text = String::from_utf8_lossy(&bytes);
                    let tiled = if path.to_ascii_lowercase().ends_with(".tsx") {
                        read_tsx(&text)?
                    } else {
                        read_tsj(&text)?
                    };
                    (tiled, path)
                }
                None => (ts.tileset.clone(), self.map_file.clone()),
            };
            let tileset = self.tileset(&tiled, &base)?;
            self.ranges
                .push((ts.first_gid, tileset.tiles.len() as u32 - 1));
            self.sprite.tilesets.push(tileset);
        }
        let mut next = 1;
        self.layers(&map.layers, None, (0, 0), &mut next);
        if self.sprite.layers.is_empty() {
            self.sprite
                .layers
                .push(Layer::new_image(LayerId(1), "Layer 1"));
        }
        Ok((self.sprite, self.warnings))
    }

    /// Imagen de `image` (ruta relativa a `base`), con su color transparente aplicado.
    fn bitmap(&mut self, image: &TiledImage, base: &str) -> Option<Bitmap> {
        let path = resolve_path(base, &image.source);
        let Some(bytes) = (self.load)(&path) else {
            self.warn(format!("image '{path}' not found next to the map"));
            return None;
        };
        match decode_png(&bytes) {
            Ok(mut bitmap) => {
                if let Some(trans) = image.trans {
                    for c in &mut bitmap.pixels {
                        if (c.r, c.g, c.b) == (trans.r, trans.g, trans.b) {
                            *c = Rgba::TRANSPARENT;
                        }
                    }
                }
                Some(bitmap)
            }
            Err(e) => {
                self.warn(format!("image '{path}' could not be read: {e}"));
                None
            }
        }
    }

    fn tileset(&mut self, tiled: &TiledTileset, base: &str) -> Result<Tileset> {
        let (tw, th) = (tiled.tile_width, tiled.tile_height);
        if tw == 0 || th == 0 || tw as u64 * th as u64 > MAX_TILE_PIXELS {
            return Err(invalid(format!(
                "tileset '{}' has an unsupported tile size ({tw}×{th})",
                tiled.name
            )));
        }
        let mut tileset = Tileset::new(&tiled.name, (tw, th), ColorMode::Rgba);
        tileset.allowed_flips = tiled.transformations.unwrap_or_default();
        let bitmap = match &tiled.image {
            Some(image) => self.bitmap(image, base),
            None => {
                self.warn(format!(
                    "tileset '{}' is an image collection, which is not supported: its tiles are \
                     empty",
                    tiled.name
                ));
                None
            }
        };
        let (margin, spacing) = (tiled.margin, tiled.spacing);
        let fits = |size: u32, tile: u32| {
            (size.saturating_sub(2 * margin) + spacing) / (tile + spacing).max(1)
        };
        let columns = match (tiled.columns, &bitmap) {
            (0, Some(b)) => fits(b.width, tw).max(1),
            (c, _) => c.max(1),
        };
        let count = match (tiled.tile_count, &bitmap) {
            (0, Some(b)) => columns * fits(b.height, th),
            (c, _) => c,
        };
        if count >= MAX_TILESET_TILES {
            return Err(invalid(format!(
                "tileset '{}' has too many tiles ({count})",
                tiled.name
            )));
        }
        for id in 0..count {
            let mut tile = Image::new(tw, th, ColorMode::Rgba);
            if let Some(b) = &bitmap {
                let x0 = margin + (id % columns) * (tw + spacing);
                let y0 = margin + (id / columns) * (th + spacing);
                for y in 0..th {
                    for x in 0..tw {
                        let (sx, sy) = (x0 + x, y0 + y);
                        if sx < b.width && sy < b.height {
                            let c = b.pixels[(sy * b.width + sx) as usize];
                            tile.set(x, y, PixelValue::Rgba(c)).expect("in bounds");
                        }
                    }
                }
            }
            tileset.tiles.push(Arc::new(tile));
        }
        Ok(tileset)
    }

    fn new_layer(
        &mut self,
        next: &mut u32,
        name: &str,
        kind: LayerKind,
        source: &MapLayer,
        parent: Option<LayerId>,
    ) -> LayerId {
        let id = LayerId(*next);
        *next += 1;
        let mut layer = Layer::new_image(id, name);
        layer.kind = kind;
        layer.parent = parent;
        layer.visible = source.visible;
        layer.opacity = (source.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        layer.blend = blend(source.mode.as_deref());
        self.sprite.layers.push(layer);
        id
    }

    fn add_cel(&mut self, layer: LayerId, position: (i32, i32), image: CelImage) {
        let id = self.sprite.next_image_id();
        match image {
            CelImage::Pixels(image) => {
                self.sprite.images.insert(id, Arc::new(image));
            }
            CelImage::Tiles(grid) => {
                self.sprite.tilemaps.insert(id, Arc::new(grid));
            }
        }
        self.sprite.cels.insert(
            (layer, 0),
            Cel {
                position,
                opacity: 255,
                image: id,
            },
        );
    }

    /// Tileset (índice nuestro) e id local de un GID sin flips.
    fn locate(&self, gid: u32) -> Option<(usize, u32)> {
        let (index, (first, count)) = self
            .ranges
            .iter()
            .enumerate()
            .filter(|(_, (first, _))| *first <= gid)
            .max_by_key(|(_, (first, _))| *first)?;
        let local = gid - first;
        (local < *count).then_some((index, local))
    }

    fn layers(
        &mut self,
        layers: &[MapLayer],
        parent: Option<LayerId>,
        origin: (i32, i32),
        next: &mut u32,
    ) {
        for layer in layers {
            let offset = (origin.0 + layer.offset.0, origin.1 + layer.offset.1);
            match &layer.content {
                LayerContent::Group(children) => {
                    let id = self.new_layer(next, &layer.name, LayerKind::Group, layer, parent);
                    self.layers(children, Some(id), offset, next);
                }
                LayerContent::Image(image) => {
                    let id = self.new_layer(next, &layer.name, LayerKind::Image, layer, parent);
                    let map_file = self.map_file.clone();
                    if let Some(bitmap) = image.as_ref().and_then(|i| self.bitmap(i, &map_file)) {
                        let mut pixels = Image::new(bitmap.width, bitmap.height, ColorMode::Rgba);
                        for (i, c) in bitmap.pixels.iter().enumerate() {
                            let (x, y) = (i as u32 % bitmap.width, i as u32 / bitmap.width);
                            pixels.set(x, y, PixelValue::Rgba(*c)).expect("in bounds");
                        }
                        self.add_cel(id, offset, CelImage::Pixels(pixels));
                    }
                }
                LayerContent::Tiles {
                    width,
                    height,
                    gids,
                    ..
                } => self.tile_layer(layer, (*width, *height), gids, parent, offset, next),
                LayerContent::Objects(objects) => self.slices(layer, objects, offset),
            }
        }
    }

    /// Una capa tilemap por tileset que use la capa de Tiled.
    fn tile_layer(
        &mut self,
        layer: &MapLayer,
        (width, height): (u32, u32),
        gids: &[u32],
        parent: Option<LayerId>,
        offset: (i32, i32),
        next: &mut u32,
    ) {
        let mut used: Vec<usize> = Vec::new();
        let mut unknown = false;
        let cells: Vec<Option<(usize, Tile)>> = gids
            .iter()
            .map(|&gid| {
                let index = gid & !FLIP_FLAGS;
                if index == 0 {
                    return None;
                }
                let Some((ts, local)) = self.locate(index) else {
                    unknown = true;
                    return None;
                };
                if !used.contains(&ts) {
                    used.push(ts);
                }
                Some((ts, Tile((local + 1) | (gid & FLIP_FLAGS))))
            })
            .collect();
        if unknown {
            self.warn(format!(
                "Layer '{}' uses tiles that no tileset has: they are left empty",
                layer.name
            ));
        }
        used.sort_unstable();
        if used.is_empty() {
            // Una capa vacía: tilemap del primer tileset (si hay alguno).
            if self.sprite.tilesets.is_empty() {
                self.new_layer(next, &layer.name, LayerKind::Image, layer, parent);
                return;
            }
            used.push(0);
        }
        for &ts in &used {
            let name = if used.len() > 1 {
                format!("{} ({})", layer.name, self.sprite.tilesets[ts].name)
            } else {
                layer.name.clone()
            };
            let id = self.new_layer(next, &name, LayerKind::Tilemap, layer, parent);
            self.sprite.layer_mut(id).expect("just added").tileset = Some(ts as u32);
            let mut grid = TileGrid::new(width, height);
            for (i, cell) in cells.iter().enumerate() {
                if let Some((t, tile)) = cell
                    && *t == ts
                {
                    grid.set(i as u32 % width.max(1), i as u32 / width.max(1), *tile);
                }
            }
            self.add_cel(id, offset, CelImage::Tiles(grid));
        }
    }

    /// Los rectángulos de una capa de objetos pasan a ser slices (en el frame 0).
    fn slices(&mut self, layer: &MapLayer, objects: &[MapObject], offset: (i32, i32)) {
        let mut skipped = false;
        for o in objects {
            if !o.rectangle {
                skipped = true;
                continue;
            }
            let int = |name: &str| {
                o.properties
                    .iter()
                    .find(|p| p.name == name)
                    .and_then(|p| p.value.as_int())
            };
            let pivot = int("pivot_x").zip(int("pivot_y"));
            let center = match (
                int("center_x"),
                int("center_y"),
                int("center_w"),
                int("center_h"),
            ) {
                (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 => {
                    Some((x as i32, y as i32, w as u32, h as u32))
                }
                _ => None,
            };
            let color = o
                .properties
                .iter()
                .find_map(|p| match (&p.name[..], &p.value) {
                    ("color", PropertyValue::Color(c)) => Some(*c),
                    _ => None,
                })
                .unwrap_or(Rgba::rgb(0, 0, 255));
            let text = o
                .properties
                .iter()
                .find_map(|p| match (&p.name[..], &p.value) {
                    ("text", PropertyValue::Str(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            self.sprite.slices.push(Slice {
                name: if o.name.is_empty() {
                    format!("Slice {}", o.id)
                } else {
                    o.name.clone()
                },
                color,
                text,
                keys: vec![SliceKey {
                    frame: 0,
                    bounds: (
                        o.x.round() as i32 + offset.0,
                        o.y.round() as i32 + offset.1,
                        o.width.round().max(1.0) as u32,
                        o.height.round().max(1.0) as u32,
                    ),
                    center,
                    pivot: pivot.map(|(x, y)| (x as i32, y as i32)),
                }],
            });
        }
        if skipped {
            self.warn(format!(
                "Object layer '{}': only rectangles are imported (as slices)",
                layer.name
            ));
        }
    }
}

enum CelImage {
    Pixels(Image),
    Tiles(TileGrid),
}
