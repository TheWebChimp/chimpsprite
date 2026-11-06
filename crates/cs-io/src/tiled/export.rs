//! Sprite → mapa de Tiled: tilesets en grilla, capas tilemap como capas de tiles, capas de imagen
//! como `imagelayer`, grupos y slices.

use std::collections::HashMap;
use std::io::{Cursor, Write as _};

use cs_core::{BlendMode, Document, Image, LayerId, LayerKind, Rgba, Sprite, Tile, Tileset};
use cs_render::resolve;

use super::{
    Encoding, LayerContent, Map, MapFormat, MapLayer, MapObject, MapTileset, Property,
    PropertyValue, TiledImage, TiledTileset, invalid, write_tmj, write_tmx, write_tsj, write_tsx,
};
use crate::Result;
use crate::export::Bitmap;

/// Qué frames se exportan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TiledFrames {
    /// Un solo mapa, `<nombre>.tmx`.
    Current(usize),
    /// Un mapa por frame, `<nombre>_001.tmx`…
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TiledOptions {
    pub format: MapFormat,
    /// Tilesets en archivos aparte (`.tsx`/`.tsj`) o dentro del mapa.
    pub external_tilesets: bool,
    pub encoding: Encoding,
    pub frames: TiledFrames,
}

impl Default for TiledOptions {
    fn default() -> Self {
        Self {
            format: MapFormat::Tmx,
            external_tilesets: true,
            encoding: Encoding::Csv,
            frames: TiledFrames::Current(0),
        }
    }
}

/// Archivos a escribir (nombres relativos a la carpeta de salida; el mapa, o los mapas, primero)
/// y lo que no se pudo representar en Tiled.
#[derive(Clone, Debug, Default)]
pub struct TiledExport {
    pub files: Vec<(String, Vec<u8>)>,
    pub warnings: Vec<String>,
}

impl TiledExport {
    fn warn(&mut self, message: String) {
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }

    /// Todos los archivos en un `.zip` (para la descarga en web).
    pub fn zip(&self) -> Result<Vec<u8>> {
        let zip_error = |e: zip::result::ZipError| crate::IoError::Zip(e.to_string());
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in &self.files {
            zip.start_file(name.as_str(), options).map_err(zip_error)?;
            zip.write_all(bytes)?;
        }
        Ok(zip.finish().map_err(zip_error)?.into_inner())
    }
}

/// La primera capa tilemap exportable (de abajo hacia arriba): su tileset da el tamaño de tile
/// del mapa.
fn first_tilemap(sprite: &Sprite) -> Option<&Tileset> {
    sprite
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Tilemap && !l.reference)
        .find_map(|l| sprite.tilesets.get(l.tileset? as usize))
        .filter(|ts| ts.tile_size.0 > 0 && ts.tile_size.1 > 0)
}

/// ¿Hay algo que exportar a Tiled? (al menos una capa tilemap con tileset).
pub fn has_tilemaps(sprite: &Sprite) -> bool {
    first_tilemap(sprite).is_some()
}

/// Nombre apto para archivo: letras, dígitos, `-` y `_`.
fn file_safe(name: &str, fallback: &str) -> String {
    let safe: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = safe.trim_matches('_');
    if safe.is_empty() {
        fallback.into()
    } else {
        safe.into()
    }
}

/// Blend de Tiled para el nuestro (`None` = normal o sin equivalente).
fn tiled_mode(blend: BlendMode) -> Option<&'static str> {
    match blend {
        BlendMode::Multiply => Some("multiply"),
        BlendMode::Screen => Some("screen"),
        BlendMode::Overlay => Some("overlay"),
        BlendMode::Darken => Some("darken"),
        BlendMode::Lighten => Some("lighten"),
        BlendMode::Addition => Some("add"),
        BlendMode::Difference => Some("difference"),
        BlendMode::Normal | BlendMode::Subtract | BlendMode::Other(_) => None,
    }
}

/// Un tileset del sprite ya listo para Tiled.
struct Exported {
    first_gid: u32,
    /// Tiles del sprite (incluido el vacío): índices ≥ esto no existen.
    tiles: u32,
    tiled: TiledTileset,
    /// Nombre del `.tsx`/`.tsj` (sin usar si va embebido).
    file: String,
}

/// Imagen del tileset: los tiles 1.. en una grilla de `columns` columnas.
fn tileset_bitmap(sprite: &Sprite, ts: &Tileset, columns: u32) -> Bitmap {
    let (tw, th) = ts.tile_size;
    let count = ts.tiles.len() as u32 - 1;
    let rows = count.div_ceil(columns);
    let mut bitmap = Bitmap::new(columns * tw, rows * th);
    for (i, image) in ts.tiles.iter().enumerate().skip(1) {
        if (image.width(), image.height()) != (tw, th) {
            continue;
        }
        let k = i as u32 - 1;
        let (ox, oy) = ((k % columns) * tw, (k / columns) * th);
        for y in 0..th {
            for x in 0..tw {
                let c = resolve(sprite, image.get(x, y).expect("in bounds"));
                bitmap.pixels[((oy + y) * bitmap.width + ox + x) as usize] = c;
            }
        }
    }
    bitmap
}

/// Bitmap de `image` puesta en `position`, recortada al lienzo y con `opacity` aplicada al alfa.
/// `None` si no se ve nada.
fn layer_bitmap(
    sprite: &Sprite,
    image: &Image,
    position: (i32, i32),
    opacity: u8,
) -> Option<(Bitmap, (i32, i32))> {
    let x0 = position.0.max(0);
    let y0 = position.1.max(0);
    let x1 = (position.0 + image.width() as i32).min(sprite.width as i32);
    let y1 = (position.1 + image.height() as i32).min(sprite.height as i32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut bitmap = Bitmap::new((x1 - x0) as u32, (y1 - y0) as u32);
    let mut any = false;
    for y in y0..y1 {
        for x in x0..x1 {
            let v = image
                .get((x - position.0) as u32, (y - position.1) as u32)
                .expect("in bounds");
            let mut c = resolve(sprite, v);
            c.a = (c.a as u32 * opacity as u32 / 255) as u8;
            if c.a == 0 {
                c = Rgba::TRANSPARENT;
            }
            any |= c.a > 0;
            bitmap.pixels[((y - y0) as u32 * bitmap.width + (x - x0) as u32) as usize] = c;
        }
    }
    any.then_some((bitmap, (x0, y0)))
}

/// Lo que comparten todos los frames.
struct Context<'a> {
    sprite: &'a Sprite,
    options: &'a TiledOptions,
    map_size: (u32, u32),
    tile_size: (u32, u32),
    /// Por índice de tileset del sprite.
    tilesets: HashMap<u32, Exported>,
}

pub fn export(
    doc: &Document,
    sprite: &Sprite,
    stem: &str,
    options: &TiledOptions,
) -> Result<TiledExport> {
    // Instancias a píxeles (los tilemaps quedan), máscaras y recortes aplicados.
    let baked = cs_render::bake_layers(doc, sprite, cs_render::Bake::KeepTilemaps);
    let sprite = baked.as_ref().unwrap_or(sprite);
    let masked = cs_render::bake_masks(sprite);
    let sprite = masked.as_ref().unwrap_or(sprite);

    let mut out = TiledExport::default();
    let base = first_tilemap(sprite)
        .ok_or_else(|| invalid("the sprite has no tilemap layers (Layer → Convert to Tilemap)"))?;
    let tile_size = base.tile_size;
    let map_size = (
        sprite.width.div_ceil(tile_size.0),
        sprite.height.div_ceil(tile_size.1),
    );
    let stem = file_safe(stem, "map");

    // Tilesets que usa alguna capa de tiles del mapa (con tiles y del tamaño del mapa).
    let mut tilesets = HashMap::new();
    let mut next_gid = 1;
    let mut used_files: Vec<String> = Vec::new();
    for (index, ts) in sprite.tilesets.iter().enumerate() {
        let used = sprite.layers.iter().any(|l| {
            l.kind == LayerKind::Tilemap && !l.reference && l.tileset == Some(index as u32)
        });
        let count = ts.tiles.len().saturating_sub(1) as u32;
        if !used || count == 0 || ts.tile_size != tile_size {
            continue;
        }
        let columns = (count as f64).sqrt().ceil().max(1.0) as u32;
        let mut name = file_safe(&ts.name, &format!("tileset{}", index + 1));
        let mut n = 2;
        while used_files.contains(&name) {
            name = format!("{}_{n}", file_safe(&ts.name, "tileset"));
            n += 1;
        }
        used_files.push(name.clone());
        let bitmap = tileset_bitmap(sprite, ts, columns);
        let png = format!("{name}.png");
        out.files.push((png.clone(), bitmap.to_png()?));
        tilesets.insert(
            index as u32,
            Exported {
                first_gid: next_gid,
                tiles: ts.tiles.len() as u32,
                tiled: TiledTileset {
                    name: ts.name.clone(),
                    tile_width: tile_size.0,
                    tile_height: tile_size.1,
                    tile_count: count,
                    columns,
                    margin: 0,
                    spacing: 0,
                    image: Some(TiledImage {
                        source: png,
                        width: bitmap.width,
                        height: bitmap.height,
                        trans: None,
                    }),
                    transformations: Some(ts.allowed_flips).filter(|f| f.x || f.y || f.d),
                },
                file: format!("{name}.{}", options.format.tileset_extension()),
            },
        );
        next_gid += count;
    }
    if options.external_tilesets {
        let mut ordered: Vec<&Exported> = tilesets.values().collect();
        ordered.sort_by_key(|e| e.first_gid);
        for e in ordered {
            let text = match options.format {
                MapFormat::Tmx => write_tsx(&e.tiled),
                MapFormat::Tmj => write_tsj(&e.tiled),
            };
            out.files.push((e.file.clone(), text.into_bytes()));
        }
    }

    let ctx = Context {
        sprite,
        options,
        map_size,
        tile_size,
        tilesets,
    };
    let frames: Vec<(usize, String)> = match options.frames {
        TiledFrames::Current(f) => vec![(f.min(sprite.frames.len() - 1), stem.clone())],
        TiledFrames::All => (0..sprite.frames.len())
            .map(|f| (f, format!("{stem}_{:03}", f + 1)))
            .collect(),
    };
    // Los mapas van primero (el principal es el primero de la lista).
    let mut maps = Vec::new();
    let mut images = Vec::new();
    for (frame, map_stem) in frames {
        let map = ctx.map(frame, &map_stem, &mut images, &mut out);
        let text = match options.format {
            MapFormat::Tmx => write_tmx(&map),
            MapFormat::Tmj => write_tmj(&map),
        };
        maps.push((
            format!("{map_stem}.{}", options.format.map_extension()),
            text.into_bytes(),
        ));
    }
    let rest = std::mem::take(&mut out.files);
    out.files = maps;
    out.files.extend(rest);
    out.files.extend(images);
    Ok(out)
}

impl Context<'_> {
    fn map(
        &self,
        frame: usize,
        map_stem: &str,
        images: &mut Vec<(String, Vec<u8>)>,
        out: &mut TiledExport,
    ) -> Map {
        let sprite = self.sprite;
        let mut layers = self.layers(None, frame, map_stem, images, out);
        let max_id = sprite.layers.iter().map(|l| l.id.0).max().unwrap_or(0);
        let mut next_layer_id = max_id + 1;
        let mut next_object_id = 1;
        let objects = self.slice_objects(frame, &mut next_object_id);
        if !objects.is_empty() {
            layers.push(MapLayer::new(
                next_layer_id,
                "Slices",
                LayerContent::Objects(objects),
            ));
            next_layer_id += 1;
        }
        let mut tilesets: Vec<MapTileset> = self
            .tilesets
            .values()
            .map(|e| MapTileset {
                first_gid: e.first_gid,
                source: self.options.external_tilesets.then(|| e.file.clone()),
                tileset: e.tiled.clone(),
            })
            .collect();
        tilesets.sort_by_key(|t| t.first_gid);
        Map {
            width: self.map_size.0,
            height: self.map_size.1,
            tile_width: self.tile_size.0,
            tile_height: self.tile_size.1,
            infinite: false,
            orientation: "orthogonal".into(),
            tilesets,
            layers,
            next_layer_id,
            next_object_id,
            properties: vec![
                Property::new("frame", PropertyValue::Int(frame as i64 + 1)),
                Property::new(
                    "duration_ms",
                    PropertyValue::Int(sprite.frames[frame].duration_ms as i64),
                ),
            ],
        }
    }

    /// Las capas hijas de `parent` (de abajo hacia arriba), sin las de referencia.
    fn layers(
        &self,
        parent: Option<LayerId>,
        frame: usize,
        map_stem: &str,
        images: &mut Vec<(String, Vec<u8>)>,
        out: &mut TiledExport,
    ) -> Vec<MapLayer> {
        let sprite = self.sprite;
        let mut result = Vec::new();
        for layer in sprite
            .layers
            .iter()
            .filter(|l| l.parent == parent && !l.reference)
        {
            let cel = sprite.cel(layer.id, frame);
            let mut cel_opacity = 255;
            let (content, offset) = match layer.kind {
                LayerKind::Group => (
                    LayerContent::Group(self.layers(Some(layer.id), frame, map_stem, images, out)),
                    (0, 0),
                ),
                LayerKind::Tilemap => match layer.tileset.and_then(|i| self.tilesets.get(&i)) {
                    Some(exported) => {
                        cel_opacity = cel.map_or(255, |c| c.opacity);
                        self.tile_content(layer, frame, exported, out)
                    }
                    None => {
                        let ts = layer.tileset.and_then(|i| sprite.tilesets.get(i as usize));
                        if ts.is_some_and(|ts| ts.tile_size != self.tile_size) {
                            out.warn(format!(
                                "Layer '{}' uses tiles of another size than the map: exported as \
                                 an image layer",
                                layer.name
                            ));
                        }
                        let drawn = cel.and_then(|c| {
                            let grid = sprite.tilemap(c.image)?;
                            let image = cs_core::draw_tile_grid(sprite.color_mode, ts?, grid)?;
                            Some((image, c.position, c.opacity))
                        });
                        self.image_content(layer, drawn, map_stem, images)
                    }
                },
                LayerKind::Image => {
                    let image = cel.and_then(|c| {
                        Some((Image::clone(sprite.image(c.image)?), c.position, c.opacity))
                    });
                    self.image_content(layer, image, map_stem, images)
                }
                // Ya horneadas por `bake_layers`; por si acaso, no salen.
                LayerKind::Objects => continue,
            };
            if layer.kind != LayerKind::Group
                && matches!(layer.blend, BlendMode::Subtract | BlendMode::Other(_))
            {
                out.warn(format!(
                    "Tiled has no blend mode like the one of layer '{}': exported as normal",
                    layer.name
                ));
            }
            result.push(MapLayer {
                id: layer.id.0,
                name: layer.name.clone(),
                visible: layer.visible,
                opacity: layer.opacity as f32 * cel_opacity as f32 / (255.0 * 255.0),
                offset,
                mode: (layer.kind != LayerKind::Group)
                    .then(|| tiled_mode(layer.blend))
                    .flatten()
                    .map(str::to_string),
                properties: Vec::new(),
                content,
            });
        }
        result
    }

    /// Los GIDs de una capa tilemap ubicados en la grilla del mapa, y el resto de la posición del
    /// cel (lo que no cae justo en una celda) como offset.
    fn tile_content(
        &self,
        layer: &cs_core::Layer,
        frame: usize,
        exported: &Exported,
        out: &mut TiledExport,
    ) -> (LayerContent, (i32, i32)) {
        let (w, h) = self.map_size;
        let (tw, th) = (self.tile_size.0 as i32, self.tile_size.1 as i32);
        let mut gids = vec![0u32; w as usize * h as usize];
        let mut offset = (0, 0);
        let cel = self.sprite.cel(layer.id, frame);
        if let Some((cel, grid)) = cel.and_then(|c| Some((c, self.sprite.tilemap(c.image)?))) {
            let (cx, cy) = (cel.position.0.div_euclid(tw), cel.position.1.div_euclid(th));
            offset = (cel.position.0.rem_euclid(tw), cel.position.1.rem_euclid(th));
            let (mut dropped, mut missing) = (false, false);
            for gy in 0..grid.height {
                for gx in 0..grid.width {
                    let tile = grid.get(gx, gy).unwrap_or(Tile::EMPTY);
                    if tile.index() == 0 {
                        continue;
                    }
                    let (mx, my) = (cx + gx as i32, cy + gy as i32);
                    if mx < 0 || my < 0 || mx >= w as i32 || my >= h as i32 {
                        dropped = true;
                        continue;
                    }
                    if tile.index() >= exported.tiles {
                        missing = true;
                        continue;
                    }
                    gids[my as usize * w as usize + mx as usize] =
                        (exported.first_gid + tile.index() - 1) | tile.flags();
                }
            }
            if dropped {
                out.warn(format!(
                    "Layer '{}' has tiles outside the canvas: they are not in the map",
                    layer.name
                ));
            }
            if missing {
                out.warn(format!(
                    "Layer '{}' uses tiles its tileset doesn't have: they are left empty",
                    layer.name
                ));
            }
        }
        (
            LayerContent::Tiles {
                width: w,
                height: h,
                gids,
                encoding: self.options.encoding,
            },
            offset,
        )
    }

    /// Una `imagelayer` con su PNG (si se ve algo).
    fn image_content(
        &self,
        layer: &cs_core::Layer,
        image: Option<(Image, (i32, i32), u8)>,
        map_stem: &str,
        images: &mut Vec<(String, Vec<u8>)>,
    ) -> (LayerContent, (i32, i32)) {
        let Some((bitmap, offset)) = image.and_then(|(image, position, opacity)| {
            layer_bitmap(self.sprite, &image, position, opacity)
        }) else {
            return (LayerContent::Image(None), (0, 0));
        };
        let name = format!(
            "{map_stem}_{}_{}.png",
            layer.id.0,
            file_safe(&layer.name, "layer")
        );
        let Ok(png) = bitmap.to_png() else {
            return (LayerContent::Image(None), (0, 0));
        };
        images.push((name.clone(), png));
        (
            LayerContent::Image(Some(TiledImage {
                source: name,
                width: bitmap.width,
                height: bitmap.height,
                trans: None,
            })),
            offset,
        )
    }

    /// Un rectángulo por slice vigente en `frame`, con pivote, 9-slice, color y texto como
    /// propiedades.
    fn slice_objects(&self, frame: usize, next_id: &mut u32) -> Vec<MapObject> {
        let mut objects = Vec::new();
        for slice in &self.sprite.slices {
            let Some(key) = slice.key_at(frame) else {
                continue;
            };
            let (x, y, w, h) = key.bounds;
            let mut properties = vec![Property::new("color", PropertyValue::Color(slice.color))];
            if !slice.text.is_empty() {
                properties.push(Property::new(
                    "text",
                    PropertyValue::Str(slice.text.clone()),
                ));
            }
            if let Some((px, py)) = key.pivot {
                properties.push(Property::new("pivot_x", PropertyValue::Int(px as i64)));
                properties.push(Property::new("pivot_y", PropertyValue::Int(py as i64)));
            }
            if let Some((cx, cy, cw, ch)) = key.center {
                for (name, v) in [
                    ("center_x", cx as i64),
                    ("center_y", cy as i64),
                    ("center_w", cw as i64),
                    ("center_h", ch as i64),
                ] {
                    properties.push(Property::new(name, PropertyValue::Int(v)));
                }
            }
            objects.push(MapObject {
                id: *next_id,
                name: slice.name.clone(),
                x: x as f64,
                y: y as f64,
                width: w as f64,
                height: h as f64,
                rectangle: true,
                properties,
            });
            *next_id += 1;
        }
        objects
    }
}
