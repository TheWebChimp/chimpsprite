//! Formato `.aseprite` / `.ase`, según la especificación pública
//! (<https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>).

use std::io::{Read, Write};
use std::sync::Arc;

use cs_core::{
    AllowedFlips, AnimDirection, BlendMode, Cel, ColorMode, Frame, Image, Layer, LayerId,
    LayerKind, MAX_TILE_PIXELS, MAX_TILEMAP_PIXELS, Palette, PixelBuffer, RawChunk, Rgba, Slice,
    SliceKey, Sprite, Tag, Tile, TileGrid, Tileset,
};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use crate::bytes::{Reader, Writer};
use crate::{IoError, LoadReport, Result};

const MAGIC: u16 = 0xA5E0;
const FRAME_MAGIC: u16 = 0xF1FA;
const HEADER_FLAG_LAYER_OPACITY: u32 = 1;
const HEADER_FLAG_LAYER_UUID: u32 = 4;

const CHUNK_OLD_PALETTE: u16 = 0x0004;
const CHUNK_OLD_PALETTE_64: u16 = 0x0011;
const CHUNK_LAYER: u16 = 0x2004;
const CHUNK_CEL: u16 = 0x2005;
const CHUNK_COLOR_PROFILE: u16 = 0x2007;
const CHUNK_TAGS: u16 = 0x2018;
const CHUNK_PALETTE: u16 = 0x2019;
const CHUNK_USER_DATA: u16 = 0x2020;
const CHUNK_SLICE: u16 = 0x2022;
const CHUNK_TILESET: u16 = 0x2023;

const TILESET_EXTERNAL: u32 = 1;
const TILESET_EMBEDDED: u32 = 2;
const TILESET_EMPTY_ZERO: u32 = 4;
const TILESET_FLIP_X: u32 = 8;
const TILESET_FLIP_Y: u32 = 16;
const TILESET_FLIP_D: u32 = 32;

/// Tiles máximos de un tileset. Cada tile es una imagen aparte: con tiles de 1×1 la tira
/// comprimida es mínima pero la cuenta dispararía millones de asignaciones.
const MAX_TILESET_TILES: u32 = 1 << 16;

const LAYER_VISIBLE: u16 = 1;
const LAYER_EDITABLE: u16 = 2;

const CEL_RAW: u16 = 0;
const CEL_LINKED: u16 = 1;
const CEL_COMPRESSED: u16 = 2;
const CEL_TILEMAP: u16 = 3;

/// Profundidad de color en el archivo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Depth {
    Rgba,
    Grayscale,
    Indexed,
}

impl Depth {
    fn bytes_per_pixel(self) -> usize {
        match self {
            Depth::Rgba => 4,
            Depth::Grayscale => 2,
            Depth::Indexed => 1,
        }
    }
}

fn blend_from_code(code: u16) -> BlendMode {
    match code {
        0 => BlendMode::Normal,
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        10 => BlendMode::Difference,
        16 => BlendMode::Addition,
        17 => BlendMode::Subtract,
        other => BlendMode::Other(other),
    }
}

fn blend_code(mode: BlendMode) -> u16 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::Difference => 10,
        BlendMode::Addition => 16,
        BlendMode::Subtract => 17,
        BlendMode::Other(n) => n,
    }
}

fn direction_from_code(code: u8, report: &mut LoadReport) -> AnimDirection {
    match code {
        0 => AnimDirection::Forward,
        1 => AnimDirection::Reverse,
        2 => AnimDirection::PingPong,
        3 => AnimDirection::PingPongReverse,
        other => {
            report.warn(format!("unknown tag direction {other}; using forward"));
            AnimDirection::Forward
        }
    }
}

fn direction_code(direction: AnimDirection) -> u8 {
    match direction {
        AnimDirection::Forward => 0,
        AnimDirection::Reverse => 1,
        AnimDirection::PingPong => 2,
        AnimDirection::PingPongReverse => 3,
    }
}

/// A qué objeto se asocia el próximo chunk de user data.
enum UserDataTarget {
    None,
    /// Índice del próximo tag que recibe user data.
    Tags(usize),
    /// El slice con este índice recibe el user data (color y texto).
    Slice(usize),
    /// Tras un tileset (índice en la lista que se va leyendo): `next` 0 es el tileset; `i + 1`,
    /// su tile `i`.
    Tileset {
        index: usize,
        next: usize,
    },
}

// --- Lectura ---------------------------------------------------------------------------------

/// Lee un `.aseprite`. Lo que no se puede conservar queda en los avisos del `LoadReport`.
pub fn read(data: &[u8]) -> Result<(Sprite, LoadReport)> {
    let mut report = LoadReport::default();
    let mut r = Reader::new(data);
    r.u32()?; // tamaño del archivo
    if r.u16()? != MAGIC {
        return Err(IoError::BadMagic);
    }
    let frame_count = r.u16()? as usize;
    let width = r.u16()? as u32;
    let height = r.u16()? as u32;
    let depth = match r.u16()? {
        32 => Depth::Rgba,
        16 => Depth::Grayscale,
        8 => Depth::Indexed,
        other => return Err(IoError::Unsupported(format!("color depth {other}"))),
    };
    let header_flags = r.u32()?;
    let speed = r.u16()?;
    r.skip(8)?;
    let transparent_index = r.u8()?;
    r.skip(3)?;
    r.u16()?; // cantidad de colores (la paleta trae el dato real)
    r.skip(2)?; // proporción de píxel
    let grid_offset = (r.i16()? as i32, r.i16()? as i32);
    let grid_size = (r.u16()? as u32, r.u16()? as u32);
    r.skip(84)?;
    if width == 0 || height == 0 {
        return Err(IoError::Corrupt(format!("invalid size {width}×{height}")));
    }
    if frame_count == 0 {
        return Err(IoError::Corrupt("the file has no frames".into()));
    }
    let mode = match depth {
        Depth::Indexed => ColorMode::Indexed { transparent_index },
        Depth::Rgba => ColorMode::Rgba,
        Depth::Grayscale => {
            report.warn("grayscale sprite converted to RGBA");
            ColorMode::Rgba
        }
    };

    let mut sprite = Sprite::new(width, height, mode);
    if grid_size.0 > 0 && grid_size.1 > 0 {
        sprite.grid = cs_core::GridSettings {
            size: grid_size,
            offset: grid_offset,
        };
    }
    sprite.layers.clear();
    sprite.frames.clear();
    let mut palette: Vec<Rgba> = Vec::new();
    let mut has_new_palette = false;
    // Grupo más reciente en cada nivel de anidamiento (NOTE.1 de la spec).
    let mut groups_at_level: Vec<LayerId> = Vec::new();
    let mut user_data = UserDataTarget::None;
    // Tilesets con su id del archivo; al final se ordenan por id.
    let mut tilesets: Vec<(u32, Tileset)> = Vec::new();

    for frame in 0..frame_count {
        let frame_start = r.pos;
        let frame_bytes = r.u32()? as usize;
        if r.u16()? != FRAME_MAGIC {
            return Err(IoError::Corrupt(format!(
                "bad magic in frame {}",
                frame + 1
            )));
        }
        let old_chunks = r.u16()? as usize;
        let duration = r.u16()?;
        r.skip(2)?;
        let new_chunks = r.u32()? as usize;
        let chunk_count = if new_chunks == 0 {
            old_chunks
        } else {
            new_chunks
        };
        let duration_ms = match (duration, speed) {
            (0, 0) => 100,
            (0, s) => s,
            (d, _) => d,
        };
        sprite.frames.push(Frame { duration_ms });

        for _ in 0..chunk_count {
            let chunk_start = r.pos;
            let size = r.u32()? as usize;
            if size < 6 {
                return Err(IoError::Corrupt(format!("chunk too small ({size} bytes)")));
            }
            let kind = r.u16()?;
            let mut c = Reader::new(r.bytes(size - 6)?);
            match kind {
                CHUNK_PALETTE => {
                    has_new_palette = true;
                    read_palette(&mut c, &mut palette)?;
                }
                CHUNK_OLD_PALETTE | CHUNK_OLD_PALETTE_64 if !has_new_palette => {
                    read_old_palette(&mut c, &mut palette, kind == CHUNK_OLD_PALETTE_64)?;
                }
                CHUNK_OLD_PALETTE | CHUNK_OLD_PALETTE_64 => {}
                CHUNK_LAYER => {
                    read_layer(
                        &mut c,
                        &mut sprite,
                        header_flags,
                        &mut groups_at_level,
                        &mut report,
                    )?;
                    user_data = UserDataTarget::None;
                }
                CHUNK_CEL => {
                    read_cel(&mut c, &mut sprite, frame, depth, &mut report)?;
                    user_data = UserDataTarget::None;
                }
                CHUNK_TAGS => {
                    let first = sprite.tags.len();
                    read_tags(&mut c, &mut sprite.tags, &mut report)?;
                    user_data = UserDataTarget::Tags(first);
                }
                CHUNK_USER_DATA => match &mut user_data {
                    UserDataTarget::Tags(next) => {
                        if let Some(color) = read_user_data_color(&mut c)?
                            && let Some(tag) = sprite.tags.get_mut(*next)
                        {
                            tag.color = color;
                        }
                        *next += 1;
                    }
                    UserDataTarget::Slice(i) => {
                        let (text, color) = read_user_data(&mut c)?;
                        if let Some(slice) = sprite.slices.get_mut(*i) {
                            slice.text = text.unwrap_or_default();
                            if let Some(color) = color {
                                slice.color = color;
                            }
                        }
                        user_data = UserDataTarget::None;
                    }
                    UserDataTarget::Tileset { index, next } => {
                        let data = c.rest().to_vec();
                        // Un chunk vacío (flags 0) es lo mismo que no tener user data.
                        let chunk =
                            (data.get(..4) != Some(&[0, 0, 0, 0][..])).then_some(RawChunk {
                                kind: CHUNK_USER_DATA,
                                data,
                            });
                        let tileset = &mut tilesets[*index].1;
                        match *next {
                            0 => tileset.user_data = chunk,
                            n if n <= tileset.tiles.len() => {
                                if let Some(chunk) = chunk {
                                    let i = n - 1;
                                    if tileset.tile_user_data.len() <= i {
                                        tileset.tile_user_data.resize(i + 1, None);
                                    }
                                    tileset.tile_user_data[i] = Some(chunk);
                                }
                            }
                            _ => {}
                        }
                        *next += 1;
                    }
                    UserDataTarget::None => {
                        report.warn("user data (text, colors, properties) is not kept yet");
                    }
                },
                CHUNK_SLICE => {
                    sprite.slices.push(read_slice(&mut c)?);
                    user_data = UserDataTarget::Slice(sprite.slices.len() - 1);
                }
                CHUNK_TILESET => {
                    tilesets.push(read_tileset(&mut c, depth, mode, &mut report)?);
                    user_data = UserDataTarget::Tileset {
                        index: tilesets.len() - 1,
                        next: 0,
                    };
                }
                CHUNK_COLOR_PROFILE => {}
                other => {
                    report.warn(format!(
                        "chunk type 0x{other:04X} is not supported and was skipped"
                    ));
                    user_data = UserDataTarget::None;
                }
            }
            r.pos = chunk_start + size;
        }
        // Respeta el tamaño declarado del frame aunque haya bytes extra.
        if frame_bytes >= 16 && frame_start + frame_bytes <= data.len() {
            r.pos = frame_start + frame_bytes;
        }
    }

    if !palette.is_empty() {
        sprite.palette = Palette::new(palette);
    }
    if sprite.layers.is_empty() {
        report.warn("the file has no layers; an empty layer was added");
        sprite.layers.push(Layer::new_image(LayerId(1), "Layer 1"));
    }
    if !sprite.is_valid_tree() {
        report.warn("invalid layer hierarchy; groups were flattened");
        for layer in &mut sprite.layers {
            layer.parent = None;
        }
    }
    let last = sprite.frames.len() - 1;
    for tag in &mut sprite.tags {
        tag.from = tag.from.min(last);
        tag.to = tag.to.clamp(tag.from, last);
    }
    // Los tilesets quedan en el orden de su id; las capas pasan de id a índice.
    tilesets.sort_by_key(|(id, _)| *id);
    let ids: Vec<u32> = tilesets.iter().map(|(id, _)| *id).collect();
    for layer in &mut sprite.layers {
        let Some(id) = layer.tileset else {
            continue;
        };
        match ids.iter().position(|&t| t == id) {
            Some(i) => layer.tileset = Some(i as u32),
            None => {
                report.warn(format!(
                    "tilemap layer '{}' uses missing tileset {id}",
                    layer.name
                ));
                // Sin tileset la capa no dibuja nada; un id viejo podría apuntar a otro tileset.
                layer.tileset = None;
            }
        }
    }
    sprite.tilesets = tilesets.into_iter().map(|(_, t)| t).collect();
    drop_huge_tilemap_cels(&mut sprite, &mut report);
    Ok((sprite, report))
}

fn read_palette(c: &mut Reader, palette: &mut Vec<Rgba>) -> Result<()> {
    let size = c.u32()? as usize;
    let from = c.u32()? as usize;
    let to = c.u32()? as usize;
    c.skip(8)?;
    if size > 65536 || to >= size.max(1) || from > to {
        return Err(IoError::Corrupt(format!(
            "invalid palette range {from}..={to} of {size}"
        )));
    }
    palette.resize(size, Rgba::rgb(0, 0, 0));
    for entry in &mut palette[from..=to] {
        let flags = c.u16()?;
        let (r, g, b, a) = (c.u8()?, c.u8()?, c.u8()?, c.u8()?);
        if flags & 1 != 0 {
            c.string()?;
        }
        *entry = Rgba::new(r, g, b, a);
    }
    Ok(())
}

fn read_old_palette(c: &mut Reader, palette: &mut Vec<Rgba>, six_bit: bool) -> Result<()> {
    let packets = c.u16()?;
    let mut index = 0usize;
    for _ in 0..packets {
        index += c.u8()? as usize;
        let count = match c.u8()? {
            0 => 256,
            n => n as usize,
        };
        for _ in 0..count {
            let mut channel = || -> Result<u8> {
                let v = c.u8()?;
                Ok(if six_bit {
                    (v.min(63) as u32 * 255 / 63) as u8
                } else {
                    v
                })
            };
            let color = Rgba::rgb(channel()?, channel()?, channel()?);
            if index >= palette.len() {
                palette.resize(index + 1, Rgba::rgb(0, 0, 0));
            }
            palette[index] = color;
            index += 1;
        }
    }
    Ok(())
}

fn read_tileset(
    c: &mut Reader,
    depth: Depth,
    mode: ColorMode,
    report: &mut LoadReport,
) -> Result<(u32, Tileset)> {
    let id = c.u32()?;
    let flags = c.u32()?;
    let count = c.u32()?;
    let tw = c.u16()? as u32;
    let th = c.u16()? as u32;
    let base_index = c.i16()?;
    c.skip(14)?;
    let name = c.string()?;
    if tw == 0 || th == 0 {
        return Err(IoError::Corrupt(format!(
            "tileset '{name}' has {tw}×{th} tiles"
        )));
    }
    // El tamaño viene del archivo (hasta 65535²): se valida antes de reservar cualquier tile.
    if tw as u64 * th as u64 > MAX_TILE_PIXELS {
        return Err(IoError::Corrupt(format!(
            "tileset '{name}' has {tw}×{th} tiles (at most {MAX_TILE_PIXELS} pixels per tile are \
             supported)"
        )));
    }
    // Sin el tile 0 todavía: el caso embebido arma la lista desde la tira decodificada.
    let mut tileset = Tileset {
        name,
        tile_size: (tw, th),
        base_index,
        allowed_flips: AllowedFlips {
            x: flags & TILESET_FLIP_X != 0,
            y: flags & TILESET_FLIP_Y != 0,
            d: flags & TILESET_FLIP_D != 0,
        },
        tiles: Vec::new(),
        user_data: None,
        tile_user_data: Vec::new(),
    };
    if flags & TILESET_EMPTY_ZERO == 0 {
        report.warn(format!(
            "tileset '{}' uses an old empty-tile format; tile 0 is treated as empty",
            tileset.name
        ));
    }
    if flags & TILESET_EXTERNAL != 0 {
        c.skip(8)?;
    }
    if flags & TILESET_EMBEDDED != 0 {
        let len = c.u32()? as usize;
        // Error y no tileset recortado: los índices de las grillas y el user data de los tiles
        // dependen de la cuenta completa.
        if count > MAX_TILESET_TILES {
            return Err(IoError::Corrupt(format!(
                "tileset '{}' has {count} tiles (at most {MAX_TILESET_TILES} are supported)",
                tileset.name
            )));
        }
        let too_big = || IoError::Corrupt(format!("tileset '{}' is too large", tileset.name));
        let strip_h = th.checked_mul(count).ok_or_else(too_big)?;
        let expected = (tw as usize)
            .checked_mul(strip_h as usize)
            .and_then(|n| n.checked_mul(depth.bytes_per_pixel()))
            .ok_or_else(too_big)?;
        // `inflate` exige exactamente `expected` bytes: no reserva lo que el header promete.
        let raw = inflate(c.bytes(len)?, expected)?;
        if count > 0 {
            let strip = decode_pixels(tw, strip_h, &raw, depth)?;
            tileset.tiles = (0..count)
                .map(|i| Arc::new(strip.crop(0, i * th, tw, th)))
                .collect();
        }
    } else if flags & TILESET_EXTERNAL != 0 {
        report.warn(format!(
            "tileset '{}' is stored in an external file and was loaded without tiles",
            tileset.name
        ));
    } else {
        report.warn(format!(
            "tileset '{}' has no tiles in the file nor a link to one; it was loaded empty",
            tileset.name
        ));
    }
    // El tile 0 (vacío) siempre existe.
    if tileset.tiles.is_empty() {
        tileset.tiles.push(Arc::new(Image::new(tw, th, mode)));
    }
    Ok((id, tileset))
}

/// Descarta los cels tilemap que, dibujados, serían enormes (ver `MAX_TILEMAP_PIXELS`).
fn drop_huge_tilemap_cels(sprite: &mut Sprite, report: &mut LoadReport) {
    let mut dropped = false;
    let sizes: Vec<(LayerId, (u32, u32))> = sprite
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Tilemap)
        .filter_map(|l| {
            let ts = sprite.tilesets.get(l.tileset? as usize)?;
            Some((l.id, ts.tile_size))
        })
        .collect();
    for (layer, (tw, th)) in sizes {
        let tilemaps = &sprite.tilemaps;
        sprite.cels.retain(|&(l, _), cel| {
            let Some(grid) = (l == layer).then(|| tilemaps.get(&cel.image)).flatten() else {
                return true;
            };
            let w = grid.width as u64 * tw as u64;
            let h = grid.height as u64 * th as u64;
            let keep = w.saturating_mul(h) <= MAX_TILEMAP_PIXELS;
            dropped |= !keep;
            keep
        });
    }
    if dropped {
        report.warn("tilemap cels too large to draw were skipped");
        let used: std::collections::HashSet<_> = sprite.cels.values().map(|c| c.image).collect();
        sprite.tilemaps.retain(|id, _| used.contains(id));
    }
}

fn read_layer(
    c: &mut Reader,
    sprite: &mut Sprite,
    header_flags: u32,
    groups_at_level: &mut Vec<LayerId>,
    report: &mut LoadReport,
) -> Result<()> {
    let flags = c.u16()?;
    let kind_code = c.u16()?;
    let level = c.u16()? as usize;
    c.skip(4)?;
    let blend = blend_from_code(c.u16()?);
    let opacity = c.u8()?;
    c.skip(3)?;
    let name = c.string()?;
    let (kind, tileset) = match kind_code {
        0 => (LayerKind::Image, None),
        1 => (LayerKind::Group, None),
        // Id del tileset en el archivo; `read` lo pasa a índice al terminar.
        2 => (LayerKind::Tilemap, Some(c.u32()?)),
        other => {
            report.warn(format!(
                "unknown layer type {other}; loaded as an image layer"
            ));
            (LayerKind::Image, None)
        }
    };
    if header_flags & HEADER_FLAG_LAYER_UUID != 0 {
        c.skip(16)?;
    }
    let id = LayerId(sprite.layers.len() as u32 + 1);
    let parent = match level {
        0 => None,
        l => match groups_at_level.get(l - 1) {
            Some(&p) => Some(p),
            None => {
                report
                    .warn("a layer is nested deeper than its group; it was moved to the top level");
                None
            }
        },
    };
    groups_at_level.truncate(level);
    if kind == LayerKind::Group && groups_at_level.len() == level {
        groups_at_level.push(id);
    }
    sprite.layers.push(Layer {
        id,
        name,
        kind,
        parent,
        visible: flags & LAYER_VISIBLE != 0,
        locked: flags & LAYER_EDITABLE == 0,
        // Aseprite guarda los grupos con opacidad 0 (y la ignora al componer): eso es "opaco".
        opacity: match (header_flags & HEADER_FLAG_LAYER_OPACITY != 0, kind, opacity) {
            (false, _, _) | (true, LayerKind::Group, 0) => 255,
            (true, _, o) => o,
        },
        blend,
        mask: None,
        clip: false,
        instance: None,
        tileset,
    });
    Ok(())
}

fn read_cel(
    c: &mut Reader,
    sprite: &mut Sprite,
    frame: usize,
    depth: Depth,
    report: &mut LoadReport,
) -> Result<()> {
    let layer_index = c.u16()? as usize;
    let x = c.i16()? as i32;
    let y = c.i16()? as i32;
    let opacity = c.u8()?;
    let cel_type = c.u16()?;
    c.i16()?; // z-index: todavía no soportado
    c.skip(5)?;
    let Some(layer) = sprite.layers.get(layer_index).map(|l| l.id) else {
        return Err(IoError::Corrupt(format!(
            "cel references missing layer {layer_index}"
        )));
    };
    let in_tilemap = sprite.layers[layer_index].kind == LayerKind::Tilemap;
    let cel = match cel_type {
        // Una capa tilemap solo tiene grillas: un cel de píxeles ahí no se podría dibujar ni guardar.
        CEL_RAW | CEL_COMPRESSED if in_tilemap => {
            report.warn("an image cel in a tilemap layer was skipped");
            return Ok(());
        }
        CEL_RAW | CEL_COMPRESSED => {
            let w = c.u16()? as u32;
            let h = c.u16()? as u32;
            let expected = w as usize * h as usize * depth.bytes_per_pixel();
            let raw = if cel_type == CEL_RAW {
                c.bytes(expected)?.to_vec()
            } else {
                inflate(c.rest(), expected)?
            };
            let image = decode_pixels(w, h, &raw, depth)?;
            let id = sprite.next_image_id();
            sprite.images.insert(id, Arc::new(image));
            Cel {
                position: (x, y),
                opacity,
                image: id,
            }
        }
        CEL_LINKED => {
            let source = c.u16()? as usize;
            match sprite.cel(layer, source) {
                Some(cel) => *cel,
                None => {
                    report.warn("a linked cel points to a missing cel and was skipped");
                    return Ok(());
                }
            }
        }
        CEL_TILEMAP => {
            let w = c.u16()? as u32;
            let h = c.u16()? as u32;
            let bits = c.u16()?;
            let index_mask = c.u32()?;
            let (x_mask, y_mask, d_mask) = (c.u32()?, c.u32()?, c.u32()?);
            c.skip(10)?;
            if bits != 32 {
                report.warn(format!(
                    "a tilemap cel with {bits} bits per tile was skipped"
                ));
                return Ok(());
            }
            if !in_tilemap {
                report.warn("a tilemap cel in a non-tilemap layer was skipped");
                return Ok(());
            }
            let raw = inflate(c.rest(), w as usize * h as usize * 4)?;
            // Cada valor se normaliza a las máscaras estándar de `Tile`.
            let tiles = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| {
                    let v = u32::from_le_bytes(*b);
                    Tile::new(
                        v & index_mask,
                        v & x_mask != 0,
                        v & y_mask != 0,
                        v & d_mask != 0,
                    )
                })
                .collect();
            let id = sprite.next_image_id();
            sprite.tilemaps.insert(
                id,
                Arc::new(TileGrid {
                    width: w,
                    height: h,
                    tiles,
                }),
            );
            Cel {
                position: (x, y),
                opacity,
                image: id,
            }
        }
        other => {
            report.warn(format!("unknown cel type {other} was skipped"));
            return Ok(());
        }
    };
    sprite.cels.insert((layer, frame), cel);
    Ok(())
}

/// Descomprime zlib exigiendo exactamente `expected` bytes (evita "zip bombs").
fn inflate(data: &[u8], expected: usize) -> Result<Vec<u8>> {
    // zlib no comprime más de ~1032:1; no reservamos más que eso aunque el header diga otra cosa.
    let mut out = Vec::with_capacity(expected.min(data.len().saturating_mul(1032)));
    ZlibDecoder::new(data)
        .take(expected as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| IoError::Corrupt(format!("bad compressed image: {e}")))?;
    if out.len() != expected {
        return Err(IoError::Corrupt(format!(
            "compressed image has {} bytes, expected {expected}",
            out.len()
        )));
    }
    Ok(out)
}

fn decode_pixels(w: u32, h: u32, raw: &[u8], depth: Depth) -> Result<Image> {
    let pixels = match depth {
        Depth::Rgba => PixelBuffer::Rgba(
            raw.as_chunks::<4>()
                .0
                .iter()
                .map(|p| Rgba::new(p[0], p[1], p[2], p[3]))
                .collect(),
        ),
        Depth::Grayscale => PixelBuffer::Rgba(
            raw.as_chunks::<2>()
                .0
                .iter()
                .map(|p| Rgba::new(p[0], p[0], p[0], p[1]))
                .collect(),
        ),
        Depth::Indexed => PixelBuffer::Indexed(raw.to_vec()),
    };
    Image::from_buffer(w, h, pixels).ok_or_else(|| IoError::Corrupt("image size mismatch".into()))
}

fn read_tags(c: &mut Reader, tags: &mut Vec<Tag>, report: &mut LoadReport) -> Result<()> {
    let count = c.u16()?;
    c.skip(8)?;
    for _ in 0..count {
        let from = c.u16()? as usize;
        let to = c.u16()? as usize;
        let direction = direction_from_code(c.u8()?, report);
        let repeat = c.u16()?;
        c.skip(6)?;
        let (r, g, b) = (c.u8()?, c.u8()?, c.u8()?);
        c.skip(1)?;
        let name = c.string()?;
        tags.push(Tag {
            name,
            from: from.min(to),
            to: from.max(to),
            direction,
            color: Rgba::rgb(r, g, b),
            repeat,
        });
    }
    Ok(())
}

/// Color de los slices sin user data (el azul de Aseprite).
const SLICE_COLOR: Rgba = Rgba::rgb(0, 0, 255);

fn read_slice(c: &mut Reader) -> Result<Slice> {
    let count = c.u32()? as usize;
    let flags = c.u32()?;
    c.skip(4)?;
    let name = c.string()?;
    let mut keys = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        let frame = c.u32()? as usize;
        let (x, y) = (c.u32()? as i32, c.u32()? as i32);
        let (w, h) = (c.u32()?, c.u32()?);
        let center = if flags & 1 != 0 {
            Some((c.u32()? as i32, c.u32()? as i32, c.u32()?, c.u32()?))
        } else {
            None
        };
        let pivot = if flags & 2 != 0 {
            Some((c.u32()? as i32, c.u32()? as i32))
        } else {
            None
        };
        keys.push(SliceKey {
            frame,
            bounds: (x, y, w, h),
            center,
            pivot,
        });
    }
    keys.sort_by_key(|k| k.frame);
    Ok(Slice {
        name,
        color: SLICE_COLOR,
        text: String::new(),
        keys,
    })
}

/// Texto y color de un chunk de user data.
fn read_user_data(c: &mut Reader) -> Result<(Option<String>, Option<Rgba>)> {
    let flags = c.u32()?;
    let text = if flags & 1 != 0 {
        Some(c.string()?)
    } else {
        None
    };
    let color = if flags & 2 != 0 {
        let (r, g, b, a) = (c.u8()?, c.u8()?, c.u8()?, c.u8()?);
        Some(Rgba::new(r, g, b, a))
    } else {
        None
    };
    Ok((text, color))
}

fn read_user_data_color(c: &mut Reader) -> Result<Option<Rgba>> {
    let flags = c.u32()?;
    if flags & 1 != 0 {
        c.string()?;
    }
    if flags & 2 != 0 {
        let (r, g, b, a) = (c.u8()?, c.u8()?, c.u8()?, c.u8()?);
        return Ok(Some(Rgba::new(r, g, b, a)));
    }
    Ok(None)
}

// --- Escritura -------------------------------------------------------------------------------

/// Escribe el sprite como `.aseprite`.
pub fn write(sprite: &Sprite) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, sprite);
    for frame in 0..sprite.frames.len() {
        let mut chunks: Vec<(u16, Vec<u8>)> = Vec::new();
        if frame == 0 {
            chunks.push((CHUNK_COLOR_PROFILE, color_profile_srgb()));
            if !sprite.palette.is_empty() {
                chunks.push((CHUNK_PALETTE, palette_chunk(&sprite.palette)));
            }
            // Como Aseprite: los tilesets van antes de las capas, cada uno seguido de su user data.
            for (index, tileset) in sprite.tilesets.iter().enumerate() {
                chunks.push((CHUNK_TILESET, tileset_chunk(index, tileset)));
                let has_data = tileset.user_data.is_some()
                    || tileset.tile_user_data.iter().any(Option::is_some);
                if has_data {
                    chunks.push((CHUNK_USER_DATA, user_data_bytes(tileset.user_data.as_ref())));
                    for i in 0..tileset.tiles.len() {
                        let tile = tileset.tile_user_data.get(i).and_then(Option::as_ref);
                        chunks.push((CHUNK_USER_DATA, user_data_bytes(tile)));
                    }
                }
            }
            for layer in &sprite.layers {
                if layer.kind == LayerKind::Tilemap && !has_tileset(sprite, layer) {
                    log::warn!(
                        "tilemap layer '{}' has no tileset; saved as an empty image layer",
                        layer.name
                    );
                }
                chunks.push((CHUNK_LAYER, layer_chunk(sprite, layer)));
            }
            if !sprite.tags.is_empty() {
                chunks.push((CHUNK_TAGS, tags_chunk(&sprite.tags)));
                for tag in &sprite.tags {
                    chunks.push((CHUNK_USER_DATA, color_user_data(tag.color)));
                }
            }
            for chunk in &sprite.extra_chunks {
                chunks.push((chunk.kind, chunk.data.clone()));
            }
            for slice in &sprite.slices {
                chunks.push((CHUNK_SLICE, slice_chunk(slice)));
                chunks.push((CHUNK_USER_DATA, slice_user_data(slice)));
            }
        }
        for (index, layer) in sprite.layers.iter().enumerate() {
            if let Some(data) = cel_chunk(sprite, index, layer.id, frame) {
                chunks.push((CHUNK_CEL, data));
            }
        }
        write_frame(&mut out, sprite.frames[frame].duration_ms, &chunks);
    }
    let size = out.len() as u32;
    out[0..4].copy_from_slice(&size.to_le_bytes());
    out
}

fn write_header(out: &mut Vec<u8>, sprite: &Sprite) {
    let (depth, transparent) = match sprite.color_mode {
        ColorMode::Rgba => (32, 0),
        ColorMode::Indexed { transparent_index } => (8, transparent_index),
    };
    out.w_u32(0); // tamaño: se completa al final
    out.w_u16(MAGIC);
    out.w_u16(sprite.frames.len() as u16);
    out.w_u16(sprite.width as u16);
    out.w_u16(sprite.height as u16);
    out.w_u16(depth);
    out.w_u32(HEADER_FLAG_LAYER_OPACITY);
    out.w_u16(sprite.frames.first().map_or(100, |f| f.duration_ms));
    out.w_zeros(8);
    out.w_u8(transparent);
    out.w_zeros(3);
    out.w_u16(sprite.palette.len().min(u16::MAX as usize) as u16);
    out.w_u8(1); // proporción de píxel 1:1
    out.w_u8(1);
    let grid = sprite.grid;
    out.w_i16(grid.offset.0.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    out.w_i16(grid.offset.1.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    out.w_u16(grid.size.0.min(u16::MAX as u32) as u16);
    out.w_u16(grid.size.1.min(u16::MAX as u32) as u16);
    out.w_zeros(84);
}

fn write_frame(out: &mut Vec<u8>, duration_ms: u16, chunks: &[(u16, Vec<u8>)]) {
    let body: usize = chunks.iter().map(|(_, d)| d.len() + 6).sum();
    out.w_u32((16 + body) as u32);
    out.w_u16(FRAME_MAGIC);
    out.w_u16(chunks.len().min(0xFFFF) as u16);
    out.w_u16(duration_ms);
    out.w_zeros(2);
    out.w_u32(chunks.len() as u32);
    for (kind, data) in chunks {
        out.w_u32((data.len() + 6) as u32);
        out.w_u16(*kind);
        out.extend_from_slice(data);
    }
}

fn color_profile_srgb() -> Vec<u8> {
    let mut d = Vec::new();
    d.w_u16(1); // sRGB
    d.w_u16(0);
    d.w_u32(0);
    d.w_zeros(8);
    d
}

fn palette_chunk(palette: &Palette) -> Vec<u8> {
    let mut d = Vec::new();
    let n = palette.len() as u32;
    d.w_u32(n);
    d.w_u32(0);
    d.w_u32(n - 1);
    d.w_zeros(8);
    for c in palette.colors() {
        d.w_u16(0);
        d.extend_from_slice(&[c.r, c.g, c.b, c.a]);
    }
    d
}

/// ¿La capa tilemap apunta a un tileset que existe? Si no, se guarda como capa de imagen vacía:
/// escribir otro id (el 0) la ataría a un tileset ajeno.
fn has_tileset(sprite: &Sprite, layer: &Layer) -> bool {
    layer
        .tileset
        .is_some_and(|i| (i as usize) < sprite.tilesets.len())
}

fn layer_chunk(sprite: &Sprite, layer: &Layer) -> Vec<u8> {
    let mut flags = 0;
    if layer.visible {
        flags |= LAYER_VISIBLE;
    }
    if !layer.locked {
        flags |= LAYER_EDITABLE;
    }
    let mut d = Vec::new();
    d.w_u16(flags);
    let tileset = layer.tileset.filter(|_| has_tileset(sprite, layer));
    d.w_u16(match (layer.kind, tileset) {
        (LayerKind::Group, _) => 1,
        (LayerKind::Tilemap, Some(_)) => 2,
        (LayerKind::Tilemap, None) | (LayerKind::Image | LayerKind::Instance, _) => 0,
    });
    d.w_u16(sprite.depth(layer.id) as u16);
    d.w_u16(0);
    d.w_u16(0);
    d.w_u16(blend_code(layer.blend));
    d.w_u8(layer.opacity);
    d.w_zeros(3);
    d.w_string(&layer.name);
    if let (LayerKind::Tilemap, Some(index)) = (layer.kind, tileset) {
        d.w_u32(index);
    }
    d
}

fn tags_chunk(tags: &[Tag]) -> Vec<u8> {
    let mut d = Vec::new();
    d.w_u16(tags.len() as u16);
    d.w_zeros(8);
    for tag in tags {
        d.w_u16(tag.from as u16);
        d.w_u16(tag.to as u16);
        d.w_u8(direction_code(tag.direction));
        d.w_u16(tag.repeat);
        d.w_zeros(6);
        d.extend_from_slice(&[tag.color.r, tag.color.g, tag.color.b]);
        d.w_u8(0);
        d.w_string(&tag.name);
    }
    d
}

fn slice_chunk(slice: &Slice) -> Vec<u8> {
    let nine = slice.keys.iter().any(|k| k.center.is_some());
    let pivot = slice.keys.iter().any(|k| k.pivot.is_some());
    let mut d = Vec::new();
    d.w_u32(slice.keys.len() as u32);
    d.w_u32(nine as u32 | (pivot as u32) << 1);
    d.w_u32(0);
    d.w_string(&slice.name);
    for k in &slice.keys {
        let (x, y, w, h) = k.bounds;
        d.w_u32(k.frame as u32);
        d.w_u32(x as u32);
        d.w_u32(y as u32);
        d.w_u32(w);
        d.w_u32(h);
        if nine {
            let (cx, cy, cw, ch) = k.center.unwrap_or((0, 0, w, h));
            d.w_u32(cx as u32);
            d.w_u32(cy as u32);
            d.w_u32(cw);
            d.w_u32(ch);
        }
        if pivot {
            let (px, py) = k.pivot.unwrap_or((0, 0));
            d.w_u32(px as u32);
            d.w_u32(py as u32);
        }
    }
    d
}

fn slice_user_data(slice: &Slice) -> Vec<u8> {
    let c = slice.color;
    let mut d = Vec::new();
    if slice.text.is_empty() {
        d.w_u32(2);
    } else {
        d.w_u32(3);
        d.w_string(&slice.text);
    }
    d.extend_from_slice(&[c.r, c.g, c.b, c.a]);
    d
}

fn color_user_data(color: Rgba) -> Vec<u8> {
    let mut d = Vec::new();
    d.w_u32(2);
    d.extend_from_slice(&[color.r, color.g, color.b, color.a]);
    d
}

fn tileset_chunk(index: usize, tileset: &Tileset) -> Vec<u8> {
    let (tw, th) = tileset.tile_size;
    let f = tileset.allowed_flips;
    let flag = |on: bool, bit: u32| if on { bit } else { 0 };
    let flags = TILESET_EMBEDDED
        | TILESET_EMPTY_ZERO
        | flag(f.x, TILESET_FLIP_X)
        | flag(f.y, TILESET_FLIP_Y)
        | flag(f.d, TILESET_FLIP_D);
    let mut d = Vec::new();
    d.w_u32(index as u32);
    d.w_u32(flags);
    d.w_u32(tileset.tiles.len() as u32);
    d.w_u16(tw.min(u16::MAX as u32) as u16);
    d.w_u16(th.min(u16::MAX as u32) as u16);
    d.w_i16(tileset.base_index);
    d.w_zeros(14);
    d.w_string(&tileset.name);
    // Tira vertical de todos los tiles, incluido el 0.
    let raw: Vec<u8> = tileset.tiles.iter().flat_map(|t| raw_pixels(t)).collect();
    let compressed = deflate(&raw);
    d.w_u32(compressed.len() as u32);
    d.extend_from_slice(&compressed);
    d
}

/// Bytes de un chunk de user data; vacío (flags 0) si no hay.
fn user_data_bytes(chunk: Option<&RawChunk>) -> Vec<u8> {
    chunk.map_or_else(|| vec![0; 4], |c| c.data.clone())
}

/// Contenido de un cel según su capa.
enum CelBody<'a> {
    Pixels(&'a Image),
    Tiles(&'a TileGrid),
}

fn cel_chunk(sprite: &Sprite, layer_index: usize, layer: LayerId, frame: usize) -> Option<Vec<u8>> {
    let cel = sprite.cel(layer, frame)?;
    let l = sprite.layer(layer)?;
    let body = match l.kind {
        // Sin tileset la capa se guarda vacía (ver `has_tileset`).
        LayerKind::Tilemap if !has_tileset(sprite, l) => return None,
        LayerKind::Tilemap => CelBody::Tiles(sprite.tilemap(cel.image)?),
        _ => CelBody::Pixels(sprite.image(cel.image)?),
    };
    // El cel guarda las medidas de la grilla en un WORD: truncarlas dejaría un archivo ilegible.
    // Se omite (y con él los cels vinculados, que comparten la grilla).
    if let CelBody::Tiles(grid) = body
        && (grid.width > u16::MAX as u32 || grid.height > u16::MAX as u32)
    {
        log::warn!(
            "tilemap cel of {}×{} tiles does not fit in .aseprite and was not saved",
            grid.width,
            grid.height
        );
        return None;
    }
    let mut d = Vec::new();
    d.w_u16(layer_index as u16);
    d.w_i16(cel.position.0.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    d.w_i16(cel.position.1.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    d.w_u8(cel.opacity);
    // Vinculado si un frame anterior de la misma capa tiene el mismo cel.
    let link = (0..frame).find(|&f| sprite.cel(layer, f) == Some(cel));
    match (link, body) {
        (Some(source), _) => {
            d.w_u16(CEL_LINKED);
            d.w_i16(0);
            d.w_zeros(5);
            d.w_u16(source as u16);
        }
        (None, CelBody::Pixels(image)) => {
            d.w_u16(CEL_COMPRESSED);
            d.w_i16(0);
            d.w_zeros(5);
            d.w_u16(image.width() as u16);
            d.w_u16(image.height() as u16);
            d.extend_from_slice(&deflate(&raw_pixels(image)));
        }
        (None, CelBody::Tiles(grid)) => {
            d.w_u16(CEL_TILEMAP);
            d.w_i16(0);
            d.w_zeros(5);
            d.w_u16(grid.width as u16);
            d.w_u16(grid.height as u16);
            d.w_u16(32);
            d.w_u32(Tile::INDEX_MASK);
            d.w_u32(Tile::FLIP_X);
            d.w_u32(Tile::FLIP_Y);
            d.w_u32(Tile::FLIP_D);
            d.w_zeros(10);
            let raw: Vec<u8> = grid.tiles.iter().flat_map(|t| t.0.to_le_bytes()).collect();
            d.extend_from_slice(&deflate(&raw));
        }
    }
    Some(d)
}

fn raw_pixels(image: &Image) -> Vec<u8> {
    match image.pixels() {
        PixelBuffer::Rgba(p) => p.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect(),
        PixelBuffer::Indexed(p) => p.clone(),
    }
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .expect("writing to a Vec cannot fail");
    encoder.finish().expect("writing to a Vec cannot fail")
}

/// Para tests: imágenes que comparten id dentro de una capa.
#[cfg(test)]
pub(crate) fn linked_pairs(sprite: &Sprite) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    for (li, layer) in sprite.layers.iter().enumerate() {
        for a in 0..sprite.frames.len() {
            for b in a + 1..sprite.frames.len() {
                let image = |f| sprite.cel(layer.id, f).map(|c| c.image);
                if image(a).is_some() && image(a) == image(b) {
                    out.push((li, a, b));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_commands::ops::{self, LayerDrop};
    use cs_core::*;

    use super::*;

    /// Igualdad semántica: mismos metadatos, mismo contenido y mismos vínculos (ignorando ids).
    pub(crate) fn assert_equivalent(a: &Sprite, b: &Sprite) {
        assert_eq!(
            (a.width, a.height, a.color_mode),
            (b.width, b.height, b.color_mode)
        );
        assert_eq!(a.palette, b.palette);
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.tags, b.tags);
        assert_eq!(a.extra_chunks, b.extra_chunks);
        assert_eq!(a.slices, b.slices);
        assert_eq!(a.grid, b.grid);
        assert_eq!(a.tilesets, b.tilesets);
        assert_eq!(a.layers.len(), b.layers.len());
        for (la, lb) in a.layers.iter().zip(&b.layers) {
            // Un grupo con opacidad 0 se lee como 255: así guarda Aseprite todos los grupos.
            let opacity = match (la.kind, la.opacity) {
                (LayerKind::Group, 0) => 255,
                (_, o) => o,
            };
            assert_eq!(
                (
                    &la.name, la.kind, la.visible, la.locked, opacity, la.blend, la.tileset
                ),
                (
                    &lb.name, lb.kind, lb.visible, lb.locked, lb.opacity, lb.blend, lb.tileset
                )
            );
            let parent_index = |s: &Sprite, l: &Layer| l.parent.and_then(|p| s.layer_index(p));
            assert_eq!(parent_index(a, la), parent_index(b, lb));
            for f in 0..a.frames.len() {
                let (ca, cb) = (a.cel(la.id, f), b.cel(lb.id, f));
                assert_eq!(ca.is_some(), cb.is_some(), "cel {} frame {f}", la.name);
                if let (Some(ca), Some(cb)) = (ca, cb) {
                    assert_eq!((ca.position, ca.opacity), (cb.position, cb.opacity));
                    assert_eq!(a.image(ca.image), b.image(cb.image));
                    assert_eq!(a.tilemap(ca.image), b.tilemap(cb.image));
                }
            }
        }
        let (ma, mb) = (
            cs_render::materialize_tilemaps(a, 0..a.frames.len()),
            cs_render::materialize_tilemaps(b, 0..b.frames.len()),
        );
        for f in 0..a.frames.len() {
            assert_eq!(
                cs_render::render_frame(&ma, f),
                cs_render::render_frame(&mb, f)
            );
        }
        assert_eq!(linked_pairs(a), linked_pairs(b));
    }

    fn roundtrip(s: &Sprite) -> Sprite {
        let (back, report) = read(&write(s)).expect("read back");
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        back
    }

    /// Sprite con casi todo: grupos, capas ocultas/bloqueadas, blend, cels vinculados,
    /// posiciones negativas, varios frames con duraciones y tags.
    pub(crate) fn rich_sprite(mode: ColorMode) -> Sprite {
        let mut s = Sprite::new(12, 8, mode);
        let value = |i: u8| match mode {
            ColorMode::Rgba => PixelValue::Rgba(Rgba::new(i * 20, 255 - i * 10, i, 200 + i)),
            ColorMode::Indexed { .. } => PixelValue::Index(i + 1),
        };
        let draw = |s: &mut Sprite, layer: u32, frame: usize, pts: &[(i32, i32)], v: PixelValue| {
            Command::DrawPixels {
                layer: LayerId(layer),
                frame,
                pixels: pts.iter().map(|&(x, y)| (x, y, v)).collect(),
            }
            .apply(s)
            .unwrap();
        };
        draw(&mut s, 1, 0, &[(0, 0), (11, 7), (5, 3)], value(1));
        ops::new_layer(&s.clone(), Some(LayerId(1)), LayerKind::Group)
            .apply(&mut s)
            .unwrap(); // 2
        ops::new_layer(&s.clone(), Some(LayerId(2)), LayerKind::Image)
            .apply(&mut s)
            .unwrap(); // 3
        ops::move_layer(&s.clone(), LayerId(3), LayerDrop::Into(LayerId(2)))
            .unwrap()
            .apply(&mut s)
            .unwrap();
        draw(&mut s, 3, 0, &[(2, 2), (3, 2)], value(2));
        s.layer_mut(LayerId(3)).unwrap().blend = BlendMode::Multiply;
        s.layer_mut(LayerId(3)).unwrap().opacity = 128;
        s.layer_mut(LayerId(2)).unwrap().visible = false;
        s.layer_mut(LayerId(1)).unwrap().locked = true;
        s.layer_mut(LayerId(1)).unwrap().blend = BlendMode::Other(7);
        s.layer_mut(LayerId(1)).unwrap().name = "Fondo ñ ✓".into();
        ops::duplicate_frame(&s.clone(), 0, true)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        ops::duplicate_frame(&s.clone(), 1, false)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        // Cel con posición negativa.
        let cel = *s.cel(LayerId(3), 2).unwrap();
        s.cels.insert(
            (LayerId(3), 2),
            Cel {
                position: (-3, -1),
                opacity: 90,
                ..cel
            },
        );
        s.frames[1].duration_ms = 33;
        s.tags = vec![
            Tag {
                name: "idle".into(),
                from: 0,
                to: 1,
                direction: AnimDirection::PingPong,
                color: Rgba::new(10, 20, 30, 255),
                repeat: 3,
            },
            Tag {
                name: "hit".into(),
                from: 2,
                to: 2,
                direction: AnimDirection::PingPongReverse,
                color: Rgba::new(200, 0, 0, 255),
                repeat: 0,
            },
        ];
        s.grid = GridSettings {
            size: (8, 6),
            offset: (2, -1),
        };
        s.slices = vec![
            Slice {
                name: "hitbox".into(),
                color: Rgba::rgb(255, 0, 0),
                text: "damage=3".into(),
                keys: vec![
                    SliceKey {
                        frame: 0,
                        bounds: (1, 2, 3, 4),
                        center: None,
                        pivot: Some((1, 4)),
                    },
                    SliceKey {
                        frame: 2,
                        bounds: (-2, 0, 5, 5),
                        center: None,
                        pivot: Some((2, 5)),
                    },
                ],
            },
            Slice {
                name: "panel".into(),
                color: SLICE_COLOR,
                text: String::new(),
                keys: vec![SliceKey {
                    frame: 0,
                    bounds: (0, 0, 8, 8),
                    center: Some((2, 2, 4, 4)),
                    pivot: None,
                }],
            },
        ];
        s
    }

    #[test]
    fn rgba_roundtrip() {
        let s = rich_sprite(ColorMode::Rgba);
        let back = roundtrip(&s);
        assert_equivalent(&s, &back);
        assert!(!linked_pairs(&back).is_empty(), "keeps linked cels");
    }

    #[test]
    fn indexed_roundtrip() {
        let s = rich_sprite(ColorMode::Indexed {
            transparent_index: 0,
        });
        assert_equivalent(&s, &roundtrip(&s));
    }

    #[test]
    fn write_is_a_fixpoint_after_one_roundtrip() {
        let once = write(&roundtrip(&rich_sprite(ColorMode::Rgba)));
        let (again, _) = read(&once).unwrap();
        assert_eq!(write(&again), once);
    }

    #[test]
    fn bad_magic_and_truncation_never_panic() {
        let bytes = write(&rich_sprite(ColorMode::Rgba));
        let mut bad = bytes.clone();
        bad[4] = 0;
        assert!(matches!(read(&bad), Err(IoError::BadMagic)));
        for len in 0..bytes.len() {
            assert!(
                read(&bytes[..len]).is_err(),
                "prefix of {len} bytes should fail"
            );
        }
    }

    #[test]
    fn corrupted_bytes_never_panic() {
        let bytes = write(&rich_sprite(ColorMode::Rgba));
        for i in 0..bytes.len() {
            let mut b = bytes.clone();
            b[i] ^= 0xA5;
            let _ = read(&b);
        }
    }

    #[test]
    fn reads_old_palette_chunk_and_grayscale() {
        // Archivo mínimo armado a mano: 2×1, grayscale, una capa y paleta vieja 0x0004.
        let mut out = Vec::new();
        out.w_u32(0);
        out.w_u16(MAGIC);
        out.w_u16(1);
        out.w_u16(2);
        out.w_u16(1);
        out.w_u16(16);
        out.w_u32(1);
        out.w_u16(50);
        out.w_zeros(8);
        out.w_u8(0);
        out.w_zeros(3);
        out.w_u16(2);
        out.w_zeros(2 + 8 + 84);
        assert_eq!(out.len(), 128);
        let mut old_palette = Vec::new();
        old_palette.w_u16(1);
        old_palette.extend_from_slice(&[0, 2, 255, 0, 0, 0, 0, 255]);
        let mut layer = Vec::new();
        layer.w_u16(3);
        layer.w_u16(0);
        layer.w_u16(0);
        layer.w_zeros(4);
        layer.w_u16(0);
        layer.w_u8(255);
        layer.w_zeros(3);
        layer.w_string("gray");
        let mut cel = Vec::new();
        cel.w_u16(0);
        cel.w_i16(0);
        cel.w_i16(0);
        cel.w_u8(255);
        cel.w_u16(CEL_RAW);
        cel.w_i16(0);
        cel.w_zeros(5);
        cel.w_u16(2);
        cel.w_u16(1);
        cel.extend_from_slice(&[100, 255, 200, 128]);
        write_frame(
            &mut out,
            0,
            &[
                (CHUNK_OLD_PALETTE, old_palette),
                (CHUNK_LAYER, layer),
                (CHUNK_CEL, cel),
            ],
        );
        let (s, report) = read(&out).unwrap();
        assert_eq!(
            s.palette.colors(),
            &[Rgba::rgb(255, 0, 0), Rgba::rgb(0, 0, 255)]
        );
        assert_eq!(s.frames[0].duration_ms, 50, "uses header speed");
        assert_eq!(
            s.pixel(LayerId(1), 0, 1, 0),
            PixelValue::Rgba(Rgba::new(200, 200, 200, 128))
        );
        assert_eq!(
            report.warnings,
            vec!["grayscale sprite converted to RGBA".to_string()]
        );
    }

    /// Sprite con una capa de imagen y una tilemap (id 2) con un tileset de 4×4 con user data;
    /// 2 frames con el cel tilemap vinculado, flips y la grilla corrida hacia la izquierda.
    fn tilemap_sprite(mode: ColorMode) -> Sprite {
        let mut s = Sprite::new(16, 12, mode);
        let value = |i: u8| match mode {
            ColorMode::Rgba => PixelValue::Rgba(Rgba::new(i * 30, 200 - i * 20, i * 7, 255)),
            ColorMode::Indexed { .. } => PixelValue::Index(i + 1),
        };
        let mut ts = Tileset::new("terrain ✓", (4, 4), mode);
        for i in 1..=3u8 {
            let mut tile = Image::new(4, 4, mode);
            tile.set(0, 0, value(i)).unwrap();
            tile.set(3, 1, value(i + 1)).unwrap();
            ts.tiles.push(Arc::new(tile));
        }
        ts.base_index = 0;
        ts.allowed_flips = AllowedFlips {
            x: true,
            y: false,
            d: true,
        };
        // Texto "grass" y color (flags 1 | 2).
        let mut ud = Vec::new();
        ud.w_u32(3);
        ud.w_string("grass");
        ud.extend_from_slice(&[1, 2, 3, 255]);
        ts.user_data = Some(RawChunk {
            kind: CHUNK_USER_DATA,
            data: ud,
        });
        let mut tile_ud = Vec::new();
        tile_ud.w_u32(1);
        tile_ud.w_string("solid");
        ts.tile_user_data = vec![
            None,
            None,
            Some(RawChunk {
                kind: CHUNK_USER_DATA,
                data: tile_ud,
            }),
        ];
        Command::SetTileset {
            index: 0,
            tileset: Some(ts),
        }
        .apply(&mut s)
        .unwrap();
        ops::new_tilemap_layer(&s.clone(), Some(LayerId(1)), 0)
            .apply(&mut s)
            .unwrap();
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, value(4))],
        }
        .apply(&mut s)
        .unwrap();
        let cells = [
            (0, 0, Tile::new(1, false, false, false)),
            (1, 0, Tile::new(2, true, false, false)),
            (2, 1, Tile::new(3, false, true, true)),
            (3, 2, Tile::new(1, true, true, false)),
            (-1, 0, Tile::new(2, false, false, false)),
        ];
        ops::set_tiles(&s, LayerId(2), 0, &cells)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        ops::duplicate_frame(&s.clone(), 0, true)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        s
    }

    #[test]
    fn tilemap_layers_without_a_tileset_are_written_as_empty_image_layers() {
        for broken in [None, Some(7)] {
            let mut s = tilemap_sprite(ColorMode::Rgba);
            s.layer_mut(LayerId(2)).unwrap().tileset = broken;
            let (back, report) = read(&write(&s)).expect("the file stays readable");
            assert!(report.warnings.is_empty(), "{:?}", report.warnings);
            let layer = back.layer(LayerId(2)).unwrap();
            assert_eq!(
                (layer.kind, layer.tileset),
                (LayerKind::Image, None),
                "{broken:?}"
            );
            assert!(back.cel(LayerId(2), 0).is_none() && back.cel(LayerId(2), 1).is_none());
            assert!(back.cel(LayerId(1), 0).is_some(), "other cels are kept");
            assert_eq!(back.tilesets, s.tilesets);
        }
    }

    #[test]
    fn tilemaps_roundtrip_in_both_modes() {
        for mode in [
            ColorMode::Rgba,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ] {
            let s = tilemap_sprite(mode);
            assert_eq!(s.cel(LayerId(2), 0).unwrap().position, (-4, 0));
            let back = roundtrip(&s);
            assert_equivalent(&s, &back);
            assert_eq!(back.layers[1].kind, LayerKind::Tilemap);
            assert_eq!(back.layers[1].tileset, Some(0));
            assert!(!linked_pairs(&back).is_empty(), "linked tilemap cels");
            let rendered =
                cs_render::render_frame(&cs_render::materialize_tilemaps(&back, 0..1), 0);
            assert!(rendered.iter().any(|c| c.a > 0));
        }
    }

    #[test]
    fn written_tilemap_cels_declare_aseprites_standard_masks() {
        let bytes = write(&tilemap_sprite(ColorMode::Rgba));
        let mut masks = Vec::new();
        for m in [0x1fff_ffffu32, 0x8000_0000, 0x4000_0000, 0x2000_0000] {
            masks.extend_from_slice(&m.to_le_bytes());
        }
        assert!(
            bytes.windows(masks.len()).any(|w| w == masks.as_slice()),
            "index/X/Y/D masks 0x1fffffff/0x80000000/0x40000000/0x20000000"
        );
        assert_eq!(
            (Tile::FLIP_X, Tile::FLIP_Y, Tile::FLIP_D),
            (0x8000_0000, 0x4000_0000, 0x2000_0000)
        );
    }

    #[test]
    fn tilemap_write_is_a_fixpoint() {
        let once = write(&roundtrip(&tilemap_sprite(ColorMode::Rgba)));
        let (again, _) = read(&once).unwrap();
        assert_eq!(write(&again), once);
    }

    #[test]
    fn corrupted_tilemap_files_never_panic() {
        let bytes = write(&tilemap_sprite(ColorMode::Rgba));
        for len in 0..bytes.len() {
            let _ = read(&bytes[..len]);
        }
        for i in 0..bytes.len() {
            let mut b = bytes.clone();
            b[i] ^= 0xA5;
            let _ = read(&b);
        }
    }

    /// `.aseprite` de 8×8 y un frame con los chunks dados.
    fn file_with(chunks: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        write_header(&mut out, &Sprite::new(8, 8, ColorMode::Rgba));
        write_frame(&mut out, 100, chunks);
        out
    }

    /// Capa tilemap que pide el tileset con id 0 del archivo.
    fn tilemap_layer_chunk() -> Vec<u8> {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        // El escritor solo guarda como tilemap una capa cuyo tileset existe.
        s.tilesets.push(Tileset::new("t", (1, 1), ColorMode::Rgba));
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        layer_chunk(&s, &s.layers[0])
    }

    fn tileset_header(flags: u32, count: u32, size: u16, name: &str) -> Vec<u8> {
        let mut d = Vec::new();
        d.w_u32(0);
        d.w_u32(flags);
        d.w_u32(count);
        d.w_u16(size);
        d.w_u16(size);
        d.w_i16(1);
        d.w_zeros(14);
        d.w_string(name);
        d
    }

    #[test]
    fn external_tilesets_load_empty_with_a_warning() {
        let mut ts = tileset_header(TILESET_EXTERNAL | TILESET_EMPTY_ZERO, 3, 4, "outside");
        ts.w_u32(1);
        ts.w_u32(0);
        let (s, report) = read(&file_with(&[
            (CHUNK_TILESET, ts),
            (CHUNK_LAYER, tilemap_layer_chunk()),
        ]))
        .unwrap();
        assert_eq!(s.tilesets.len(), 1);
        assert_eq!(s.tilesets[0].tiles.len(), 1, "only the empty tile");
        assert_eq!(s.layers[0].kind, LayerKind::Tilemap);
        assert_eq!(s.layers[0].tileset, Some(0));
        assert!(
            report.warnings.iter().any(|w| w.contains("external")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn huge_tiles_are_rejected_before_allocating() {
        // Tileset externo de tiles de 65535×65535: el archivo pesa unos bytes, pero el tile 0
        // ocuparía 16 GiB.
        let mut ts = tileset_header(TILESET_EXTERNAL | TILESET_EMPTY_ZERO, 1, u16::MAX, "huge");
        ts.w_u32(1);
        ts.w_u32(0);
        let file = file_with(&[(CHUNK_TILESET, ts), (CHUNK_LAYER, tilemap_layer_chunk())]);
        assert!(file.len() < 600, "{} bytes", file.len());
        let start = std::time::Instant::now();
        let result = read(&file);
        assert!(
            matches!(&result, Err(IoError::Corrupt(m)) if m.contains("65535")),
            "{:?}",
            result.map(|(_, r)| r.warnings)
        );
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        // El límite es por área: 2048×2048 todavía se acepta, 2049×2049 no.
        for (size, ok) in [(2048u16, true), (2049, false)] {
            let mut ts = tileset_header(TILESET_EXTERNAL | TILESET_EMPTY_ZERO, 1, size, "t");
            ts.w_u32(1);
            ts.w_u32(0);
            assert_eq!(
                read(&file_with(&[(CHUNK_TILESET, ts)])).is_ok(),
                ok,
                "{size}"
            );
        }
    }

    /// Cel tilemap de `w`×`h` tiles vacíos en la capa 0.
    fn empty_tilemap_cel(w: u16, h: u16) -> Vec<u8> {
        let mut cel = Vec::new();
        cel.w_u16(0);
        cel.w_i16(0);
        cel.w_i16(0);
        cel.w_u8(255);
        cel.w_u16(CEL_TILEMAP);
        cel.w_i16(0);
        cel.w_zeros(5);
        cel.w_u16(w);
        cel.w_u16(h);
        cel.w_u16(32);
        cel.w_u32(Tile::INDEX_MASK);
        cel.w_u32(Tile::FLIP_X);
        cel.w_u32(Tile::FLIP_Y);
        cel.w_u32(Tile::FLIP_D);
        cel.w_zeros(10);
        cel.extend_from_slice(&deflate(&vec![0; w as usize * h as usize * 4]));
        cel
    }

    #[test]
    fn missing_tilesets_and_huge_tilemap_cels_are_dropped_with_warnings() {
        // Capa que pide el tileset 0 sin ningún tileset en el archivo.
        let (s, report) = read(&file_with(&[
            (CHUNK_LAYER, tilemap_layer_chunk()),
            (CHUNK_CEL, empty_tilemap_cel(1, 1)),
        ]))
        .unwrap();
        assert_eq!(s.layers[0].tileset, None);
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("missing tileset")),
            "{:?}",
            report.warnings
        );
        // Tiles de 2048×2048 en una grilla de 5×5: 10240² píxeles, más que MAX_TILEMAP_PIXELS. La
        // tira del tileset (solo el tile 0) ocupa 16 MiB sin comprimir y unos KiB comprimida.
        let mut ts = tileset_header(TILESET_EMBEDDED | TILESET_EMPTY_ZERO, 1, 2048, "big");
        let pixels = deflate(&vec![0; 2048 * 2048 * 4]);
        ts.w_u32(pixels.len() as u32);
        ts.extend_from_slice(&pixels);
        let (s, report) = read(&file_with(&[
            (CHUNK_TILESET, ts),
            (CHUNK_LAYER, tilemap_layer_chunk()),
            (CHUNK_CEL, empty_tilemap_cel(5, 5)),
        ]))
        .unwrap();
        assert!(s.cels.is_empty() && s.tilemaps.is_empty());
        assert!(
            report.warnings.iter().any(|w| w.contains("too large")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn tilesets_with_too_many_tiles_are_rejected_before_allocating() {
        // 1×1 tiles: la cuenta, no el tamaño de la tira, es lo que dispararía las asignaciones.
        let mut ts = tileset_header(
            TILESET_EMBEDDED | TILESET_EMPTY_ZERO,
            100_000_000,
            1,
            "huge",
        );
        let pixels = deflate(&[0; 16]);
        ts.w_u32(pixels.len() as u32);
        ts.extend_from_slice(&pixels);
        let start = std::time::Instant::now();
        let result = read(&file_with(&[(CHUNK_TILESET, ts)]));
        assert!(
            matches!(&result, Err(IoError::Corrupt(m)) if m.contains("tiles")),
            "{:?}",
            result.map(|(_, r)| r.warnings)
        );
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn tilesets_without_tiles_or_link_load_empty_with_a_warning() {
        let ts = tileset_header(TILESET_EMPTY_ZERO, 3, 4, "nothing");
        let (s, report) = read(&file_with(&[
            (CHUNK_TILESET, ts),
            (CHUNK_LAYER, tilemap_layer_chunk()),
        ]))
        .unwrap();
        assert_eq!(s.tilesets[0].tiles.len(), 1, "only the empty tile");
        assert!(
            report.warnings.iter().any(|w| w.contains("no tiles")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn tilemap_cels_wider_than_the_format_are_not_written() {
        let mut s = tilemap_sprite(ColorMode::Rgba);
        let cel = *s.cel(LayerId(2), 0).unwrap();
        // Una grilla de 65536 tiles de ancho no entra en el WORD del cel.
        s.tilemaps
            .insert(cel.image, Arc::new(TileGrid::new(u16::MAX as u32 + 1, 1)));
        let (back, _) = read(&write(&s)).expect("the file stays readable");
        assert!(back.cel(LayerId(2), 0).is_none() && back.cel(LayerId(2), 1).is_none());
        assert_eq!(back.layers.len(), 2);
        assert!(back.cel(LayerId(1), 0).is_some(), "other cels are kept");
    }

    #[test]
    fn tilemap_cels_that_are_not_32_bit_are_skipped_with_a_warning() {
        let mut ts = tileset_header(TILESET_EMBEDDED | TILESET_EMPTY_ZERO, 1, 2, "t");
        let pixels = deflate(&[0; 16]);
        ts.w_u32(pixels.len() as u32);
        ts.extend_from_slice(&pixels);
        let mut cel = Vec::new();
        cel.w_u16(0);
        cel.w_i16(0);
        cel.w_i16(0);
        cel.w_u8(255);
        cel.w_u16(CEL_TILEMAP);
        cel.w_i16(0);
        cel.w_zeros(5);
        cel.w_u16(1);
        cel.w_u16(1);
        cel.w_u16(16);
        cel.w_u32(0x1fff);
        cel.w_u32(0x2000);
        cel.w_u32(0x4000);
        cel.w_u32(0x8000);
        cel.w_zeros(10);
        cel.extend_from_slice(&deflate(&[1, 0]));
        let (s, report) = read(&file_with(&[
            (CHUNK_TILESET, ts),
            (CHUNK_LAYER, tilemap_layer_chunk()),
            (CHUNK_CEL, cel),
        ]))
        .unwrap();
        assert!(s.cels.is_empty());
        assert!(
            report.warnings.iter().any(|w| w.contains("16 bits")),
            "{:?}",
            report.warnings
        );
    }

    mod random {
        use proptest::prelude::*;

        use super::*;

        #[derive(Debug, Clone)]
        enum Op {
            Draw(i32, i32, u8, usize, usize),
            Layer(bool, usize),
            Move(usize, usize, u8),
            Frame(usize, bool),
            Opacity(usize, u8),
            Hide(usize),
        }

        fn op() -> impl Strategy<Value = Op> {
            prop_oneof![
                4 => (-3..12i32, -3..10i32, 1..30u8, 0..6usize, 0..6usize)
                    .prop_map(|(x, y, c, l, f)| Op::Draw(x, y, c, l, f)),
                1 => (any::<bool>(), 0..6usize).prop_map(|(g, l)| Op::Layer(g, l)),
                1 => (0..6usize, 0..6usize, 0..3u8).prop_map(|(a, b, h)| Op::Move(a, b, h)),
                1 => (0..6usize, any::<bool>()).prop_map(|(f, l)| Op::Frame(f, l)),
                1 => (0..6usize, any::<u8>()).prop_map(|(l, o)| Op::Opacity(l, o)),
                1 => (0..6usize).prop_map(Op::Hide),
            ]
        }

        fn build(mode: ColorMode, ops_list: &[Op]) -> Sprite {
            let mut s = Sprite::new(10, 8, mode);
            for o in ops_list {
                let layer = |i: usize| s.layers[i % s.layers.len()].id;
                let frame = |i: usize| i % s.frames.len();
                let cmd = match *o {
                    Op::Draw(x, y, c, l, f) => Some(Command::DrawPixels {
                        layer: layer(l),
                        frame: frame(f),
                        pixels: vec![(
                            x,
                            y,
                            match mode {
                                ColorMode::Rgba => {
                                    PixelValue::Rgba(Rgba::new(c, c * 3, 255 - c, c * 8))
                                }
                                ColorMode::Indexed { .. } => PixelValue::Index(c),
                            },
                        )],
                    }),
                    Op::Layer(group, l) => Some(ops::new_layer(
                        &s,
                        Some(layer(l)),
                        if group {
                            LayerKind::Group
                        } else {
                            LayerKind::Image
                        },
                    )),
                    Op::Move(a, b, how) => {
                        let t = layer(b);
                        let drop = match how {
                            0 => LayerDrop::Above(t),
                            1 => LayerDrop::Below(t),
                            _ => LayerDrop::Into(t),
                        };
                        ops::move_layer(&s, layer(a), drop)
                    }
                    Op::Frame(f, linked) => ops::duplicate_frame(&s, frame(f), linked),
                    Op::Opacity(l, o) => Some(Command::SetLayerProps {
                        id: layer(l),
                        props: cs_commands::LayerProps {
                            opacity: Some(o),
                            ..Default::default()
                        },
                    }),
                    Op::Hide(l) => Some(Command::SetLayerProps {
                        id: layer(l),
                        props: cs_commands::LayerProps {
                            visible: Some(false),
                            ..Default::default()
                        },
                    }),
                };
                if let Some(cmd) = cmd {
                    let _ = cmd.apply(&mut s);
                }
            }
            s
        }

        proptest! {
            #[test]
            fn random_sprites_roundtrip(ops_list in proptest::collection::vec(op(), 0..30), indexed in any::<bool>()) {
                let mode = if indexed {
                    ColorMode::Indexed { transparent_index: 0 }
                } else {
                    ColorMode::Rgba
                };
                let s = build(mode, &ops_list);
                assert_equivalent(&s, &roundtrip(&s));
            }
        }
    }
}
