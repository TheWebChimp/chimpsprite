//! Formato `.aseprite` / `.ase`, según la especificación pública
//! (<https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>).

use std::io::{Read, Write};
use std::sync::Arc;

use cs_core::{
    AnimDirection, BlendMode, Cel, ColorMode, Frame, Image, Layer, LayerId, LayerKind, Palette,
    PixelBuffer, Rgba, Slice, SliceKey, Sprite, Tag,
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
                    UserDataTarget::None => {
                        report.warn("user data (text, colors, properties) is not kept yet");
                    }
                },
                CHUNK_SLICE => {
                    sprite.slices.push(read_slice(&mut c)?);
                    user_data = UserDataTarget::Slice(sprite.slices.len() - 1);
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
    let kind = match kind_code {
        1 => LayerKind::Group,
        0 => LayerKind::Image,
        2 => {
            report.warn("tilemap layers are not supported yet; they were loaded as empty layers");
            c.u32()?;
            LayerKind::Image
        }
        other => {
            report.warn(format!(
                "unknown layer type {other}; loaded as an image layer"
            ));
            LayerKind::Image
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
        tileset: None,
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
    let cel = match cel_type {
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
            report.warn("tilemap cels are not supported yet and were skipped");
            return Ok(());
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
            for layer in &sprite.layers {
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
    d.w_u16(if layer.kind == LayerKind::Group { 1 } else { 0 });
    d.w_u16(sprite.depth(layer.id) as u16);
    d.w_u16(0);
    d.w_u16(0);
    d.w_u16(blend_code(layer.blend));
    d.w_u8(layer.opacity);
    d.w_zeros(3);
    d.w_string(&layer.name);
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

fn cel_chunk(sprite: &Sprite, layer_index: usize, layer: LayerId, frame: usize) -> Option<Vec<u8>> {
    let cel = sprite.cel(layer, frame)?;
    let image = sprite.image(cel.image)?;
    let mut d = Vec::new();
    d.w_u16(layer_index as u16);
    d.w_i16(cel.position.0.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    d.w_i16(cel.position.1.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    d.w_u8(cel.opacity);
    // Vinculado si un frame anterior de la misma capa tiene el mismo cel.
    let link = (0..frame).find(|&f| sprite.cel(layer, f) == Some(cel));
    match link {
        Some(source) => {
            d.w_u16(CEL_LINKED);
            d.w_i16(0);
            d.w_zeros(5);
            d.w_u16(source as u16);
        }
        None => {
            d.w_u16(CEL_COMPRESSED);
            d.w_i16(0);
            d.w_zeros(5);
            d.w_u16(image.width() as u16);
            d.w_u16(image.height() as u16);
            d.extend_from_slice(&deflate(&raw_pixels(image)));
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
        assert_eq!(a.layers.len(), b.layers.len());
        for (la, lb) in a.layers.iter().zip(&b.layers) {
            // Un grupo con opacidad 0 se lee como 255: así guarda Aseprite todos los grupos.
            let opacity = match (la.kind, la.opacity) {
                (LayerKind::Group, 0) => 255,
                (_, o) => o,
            };
            assert_eq!(
                (&la.name, la.kind, la.visible, la.locked, opacity, la.blend),
                (
                    &lb.name, lb.kind, lb.visible, lb.locked, lb.opacity, lb.blend
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
                }
            }
        }
        for f in 0..a.frames.len() {
            assert_eq!(cs_render::render_frame(a, f), cs_render::render_frame(b, f));
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
