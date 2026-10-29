//! Exportación: PNG, secuencia PNG, GIF animado, spritesheet y atlas con JSON estilo Aseprite.

use std::borrow::Cow;
use std::collections::HashMap;

use cs_core::{AnimDirection, BlendMode, Document, Rgba, Sprite, Tag};
use cs_render::render_frame;
use serde_json::{Map, Value, json};

use crate::{IoError, Result};

const APP: &str = "https://github.com/TheWebChimp/chimpsprite";

/// Imagen RGBA en memoria.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Rgba>,
}

impl Bitmap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![Rgba::TRANSPARENT; (width * height) as usize],
        }
    }

    /// Frame compuesto, escalado por un factor entero (vecino más cercano). Sin las capas de
    /// referencia.
    pub fn from_frame(sprite: &Sprite, frame: usize, scale: u32) -> Self {
        let scale = scale.max(1);
        let src = render_frame(&without_references(sprite), frame);
        let (w, h) = (sprite.width * scale, sprite.height * scale);
        let pixels = (0..h)
            .flat_map(|y| {
                let src = &src;
                (0..w).map(move |x| src[((y / scale) * sprite.width + x / scale) as usize])
            })
            .collect();
        Self {
            width: w,
            height: h,
            pixels,
        }
    }

    /// Normal map del frame compuesto, escalado (vecino más cercano).
    pub fn normal_from_frame(
        sprite: &Sprite,
        frame: usize,
        scale: u32,
        settings: &cs_render::NormalSettings,
    ) -> Self {
        let base = Self {
            width: sprite.width,
            height: sprite.height,
            pixels: cs_render::normal_map(
                &render_frame(&without_references(sprite), frame),
                sprite.width,
                sprite.height,
                settings,
            ),
        };
        base.scaled(scale)
    }

    /// Copia ampliada por un factor entero.
    pub fn scaled(&self, scale: u32) -> Self {
        let scale = scale.max(1);
        let (w, h) = (self.width * scale, self.height * scale);
        let pixels = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| self.pixels[((y / scale) * self.width + x / scale) as usize])
            .collect();
        Self {
            width: w,
            height: h,
            pixels,
        }
    }

    /// Copia `other` en la posición `(x, y)`.
    pub fn blit(&mut self, other: &Bitmap, x: u32, y: u32) {
        for row in 0..other.height {
            let dst = ((y + row) * self.width + x) as usize;
            let src = (row * other.width) as usize;
            self.pixels[dst..dst + other.width as usize]
                .copy_from_slice(&other.pixels[src..src + other.width as usize]);
        }
    }

    pub fn to_png(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let bytes: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|c| [c.r, c.g, c.b, c.a])
            .collect();
        encoder
            .write_header()
            .and_then(|mut w| w.write_image_data(&bytes))
            .map_err(|e| IoError::Png(e.to_string()))?;
        Ok(out)
    }
}

/// El sprite sin las capas de referencia (ocultas), que no salen al exportar.
fn without_references(sprite: &Sprite) -> std::borrow::Cow<'_, Sprite> {
    if !sprite.layers.iter().any(|l| l.reference) {
        return std::borrow::Cow::Borrowed(sprite);
    }
    let mut out = sprite.clone();
    for layer in out.layers.iter_mut().filter(|l| l.reference) {
        layer.visible = false;
    }
    std::borrow::Cow::Owned(out)
}

pub fn export_png(sprite: &Sprite, frame: usize, scale: u32) -> Result<Vec<u8>> {
    Bitmap::from_frame(sprite, frame, scale).to_png()
}

/// Un PNG por frame, en orden.
pub fn export_png_sequence(sprite: &Sprite, scale: u32) -> Result<Vec<Vec<u8>>> {
    (0..sprite.frames.len())
        .map(|f| export_png(sprite, f, scale))
        .collect()
}

/// GIF animado de los frames `from..=to`, en bucle infinito.
pub fn export_gif(sprite: &Sprite, from: usize, to: usize, scale: u32) -> Result<Vec<u8>> {
    let gif_error = |e: gif::EncodingError| IoError::Gif(e.to_string());
    let last = sprite.frames.len() - 1;
    let (from, to) = (from.min(last), to.min(last).max(from.min(last)));
    let frames: Vec<Bitmap> = (from..=to)
        .map(|f| Bitmap::from_frame(sprite, f, scale))
        .collect();
    let (w, h) = (frames[0].width, frames[0].height);
    if w > u16::MAX as u32 || h > u16::MAX as u32 {
        return Err(IoError::Unsupported(
            "GIF images are limited to 65535 px".into(),
        ));
    }

    // Paleta global si todos los colores opacos caben (deja un índice para la transparencia).
    let mut colors: HashMap<[u8; 3], u8> = HashMap::new();
    let mut fits = true;
    'scan: for frame in &frames {
        for c in &frame.pixels {
            if c.a >= 128 && !colors.contains_key(&[c.r, c.g, c.b]) {
                if colors.len() == 255 {
                    fits = false;
                    break 'scan;
                }
                let index = colors.len() as u8 + 1;
                colors.insert([c.r, c.g, c.b], index);
            }
        }
    }

    let mut out = Vec::new();
    {
        let mut palette = vec![0u8; 3];
        if fits {
            let mut ordered: Vec<_> = colors.iter().collect();
            ordered.sort_by_key(|(_, i)| **i);
            palette.extend(ordered.iter().flat_map(|(rgb, _)| rgb.iter().copied()));
        }
        let global: &[u8] = if fits { &palette } else { &[] };
        let mut encoder =
            gif::Encoder::new(&mut out, w as u16, h as u16, global).map_err(gif_error)?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(gif_error)?;
        for (i, bitmap) in frames.iter().enumerate() {
            let ms = sprite.frames[from + i].duration_ms as u32;
            let mut frame = if fits {
                let indices: Vec<u8> = bitmap
                    .pixels
                    .iter()
                    .map(|c| {
                        if c.a < 128 {
                            0
                        } else {
                            colors[&[c.r, c.g, c.b]]
                        }
                    })
                    .collect();
                gif::Frame {
                    width: w as u16,
                    height: h as u16,
                    buffer: Cow::Owned(indices),
                    transparent: Some(0),
                    ..Default::default()
                }
            } else {
                let mut rgba: Vec<u8> = bitmap
                    .pixels
                    .iter()
                    .flat_map(|c| [c.r, c.g, c.b, if c.a < 128 { 0 } else { 255 }])
                    .collect();
                gif::Frame::from_rgba_speed(w as u16, h as u16, &mut rgba, 10)
            };
            frame.delay = ms.div_ceil(10).clamp(1, u16::MAX as u32) as u16;
            frame.dispose = gif::DisposalMethod::Background;
            encoder.write_frame(&frame).map_err(gif_error)?;
        }
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetLayout {
    Horizontal,
    Vertical,
    Grid { columns: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonFormat {
    /// `"frames": { "nombre 0.aseprite": {...} }`
    Hash,
    /// `"frames": [ { "filename": "nombre 0.aseprite", ... } ]`
    Array,
}

/// Imagen de la hoja + JSON de metadatos.
pub struct Sheet {
    pub png: Vec<u8>,
    pub json: String,
}

fn direction_name(direction: AnimDirection) -> &'static str {
    match direction {
        AnimDirection::Forward => "forward",
        AnimDirection::Reverse => "reverse",
        AnimDirection::PingPong => "pingpong",
        AnimDirection::PingPongReverse => "pingpong_reverse",
    }
}

fn tag_json(tag: &Tag, name: &str, offset: usize) -> Value {
    let c = tag.color;
    json!({
        "name": name,
        "from": tag.from + offset,
        "to": tag.to + offset,
        "direction": direction_name(tag.direction),
        "color": format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a),
    })
}

fn blend_name(mode: BlendMode) -> String {
    match mode {
        BlendMode::Normal => "normal".into(),
        BlendMode::Multiply => "multiply".into(),
        BlendMode::Screen => "screen".into(),
        BlendMode::Overlay => "overlay".into(),
        BlendMode::Darken => "darken".into(),
        BlendMode::Lighten => "lighten".into(),
        BlendMode::Addition => "addition".into(),
        BlendMode::Subtract => "subtract".into(),
        BlendMode::Difference => "difference".into(),
        BlendMode::Other(n) => format!("other_{n}"),
    }
}

/// Un frame colocado en la hoja.
struct Placed {
    name: String,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    duration: u16,
}

fn frames_json(placed: &[Placed], format: JsonFormat) -> Value {
    let entry = |p: &Placed| {
        json!({
            "frame": { "x": p.x, "y": p.y, "w": p.w, "h": p.h },
            "rotated": false,
            "trimmed": false,
            "spriteSourceSize": { "x": 0, "y": 0, "w": p.w, "h": p.h },
            "sourceSize": { "w": p.w, "h": p.h },
            "duration": p.duration,
        })
    };
    match format {
        JsonFormat::Hash => {
            let mut map = Map::new();
            for p in placed {
                map.insert(p.name.clone(), entry(p));
            }
            Value::Object(map)
        }
        JsonFormat::Array => Value::Array(
            placed
                .iter()
                .map(|p| {
                    let mut e = entry(p);
                    let mut obj = Map::new();
                    obj.insert("filename".into(), json!(p.name));
                    obj.extend(e.as_object_mut().expect("object").clone());
                    Value::Object(obj)
                })
                .collect(),
        ),
    }
}

/// Slices en el formato JSON de Aseprite (escalados), con prefijo de nombre y corrimiento de frames.
fn slices_json(sprite: &Sprite, prefix: &str, frame_offset: usize, scale: u32) -> Vec<Value> {
    let s = scale as i64;
    sprite
        .slices
        .iter()
        .map(|slice| {
            let keys: Vec<Value> = slice
                .keys
                .iter()
                .map(|k| {
                    let (x, y, w, h) = k.bounds;
                    let mut key = json!({
                        "frame": k.frame + frame_offset,
                        "bounds": { "x": x as i64 * s, "y": y as i64 * s, "w": w as i64 * s, "h": h as i64 * s },
                    });
                    if let Some((cx, cy, cw, ch)) = k.center {
                        key["center"] = json!({ "x": cx as i64 * s, "y": cy as i64 * s, "w": cw as i64 * s, "h": ch as i64 * s });
                    }
                    if let Some((px, py)) = k.pivot {
                        key["pivot"] = json!({ "x": px as i64 * s, "y": py as i64 * s });
                    }
                    key
                })
                .collect();
            let c = slice.color;
            let mut v = json!({
                "name": format!("{prefix}{}", slice.name),
                "color": format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a),
                "keys": keys,
            });
            if !slice.text.is_empty() {
                v["data"] = json!(slice.text);
            }
            v
        })
        .collect()
}

fn meta(
    image_name: &str,
    w: u32,
    h: u32,
    scale: u32,
    tags: Vec<Value>,
    layers: Value,
    slices: Vec<Value>,
) -> Value {
    json!({
        "app": APP,
        "version": env!("CARGO_PKG_VERSION"),
        "image": image_name,
        "format": "RGBA8888",
        "size": { "w": w, "h": h },
        "scale": scale.to_string(),
        "frameTags": tags,
        "layers": layers,
        "slices": slices,
    })
}

/// Normal map de un frame como PNG.
pub fn export_normal_png(
    sprite: &Sprite,
    frame: usize,
    scale: u32,
    settings: &cs_render::NormalSettings,
) -> Result<Vec<u8>> {
    Bitmap::normal_from_frame(sprite, frame, scale, settings).to_png()
}

/// Hoja de normal maps con la misma distribución que `export_sheet` (cada frame por separado,
/// para que no se mezclen los bordes).
pub fn export_normal_sheet(
    sprite: &Sprite,
    layout: SheetLayout,
    scale: u32,
    settings: &cs_render::NormalSettings,
) -> Result<Vec<u8>> {
    let rects = crate::engines::frame_rects(sprite, layout, scale);
    let (w, h) = rects
        .iter()
        .fold((1, 1), |(w, h), r| (w.max(r.0 + r.2), h.max(r.1 + r.3)));
    let mut sheet = Bitmap::new(w, h);
    for (f, &(x, y, _, _)) in rects.iter().enumerate() {
        sheet.blit(&Bitmap::normal_from_frame(sprite, f, scale, settings), x, y);
    }
    sheet.to_png()
}

/// Spritesheet de todos los frames del sprite.
pub fn export_sheet(
    sprite: &Sprite,
    name: &str,
    image_name: &str,
    layout: SheetLayout,
    scale: u32,
    format: JsonFormat,
) -> Result<Sheet> {
    let scale = scale.max(1);
    let n = sprite.frames.len() as u32;
    let (fw, fh) = (sprite.width * scale, sprite.height * scale);
    let columns = match layout {
        SheetLayout::Horizontal => n,
        SheetLayout::Vertical => 1,
        SheetLayout::Grid { columns } => columns.clamp(1, n),
    };
    let rows = n.div_ceil(columns);
    let mut sheet = Bitmap::new(fw * columns, fh * rows);
    let mut placed = Vec::new();
    for f in 0..n {
        let (x, y) = ((f % columns) * fw, (f / columns) * fh);
        sheet.blit(&Bitmap::from_frame(sprite, f as usize, scale), x, y);
        placed.push(Placed {
            name: format!("{name} {f}.aseprite"),
            x,
            y,
            w: fw,
            h: fh,
            duration: sprite.frames[f as usize].duration_ms,
        });
    }
    let tags = sprite
        .tags
        .iter()
        .map(|t| tag_json(t, &t.name, 0))
        .collect();
    let layers = Value::Array(
        sprite
            .layers
            .iter()
            .map(|l| json!({ "name": l.name, "opacity": l.opacity, "blendMode": blend_name(l.blend) }))
            .collect(),
    );
    let json = json!({
        "frames": frames_json(&placed, format),
        "meta": meta(
            image_name,
            sheet.width,
            sheet.height,
            scale,
            tags,
            layers,
            slices_json(sprite, "", 0, scale),
        ),
    });
    Ok(Sheet {
        png: sheet.to_png()?,
        json: serde_json::to_string_pretty(&json)?,
    })
}

/// Atlas de todos los artboards: una fila por artboard con sus frames en orden.
/// Los tags se renombran como `artboard/tag` y apuntan a los índices globales de frames.
pub fn export_atlas(
    doc: &Document,
    image_name: &str,
    scale: u32,
    format: JsonFormat,
) -> Result<Sheet> {
    let scale = scale.max(1);
    let width = doc
        .artboards
        .iter()
        .map(|a| a.sprite.width * scale * a.sprite.frames.len() as u32)
        .max()
        .unwrap_or(1);
    let height: u32 = doc.artboards.iter().map(|a| a.sprite.height * scale).sum();
    let mut atlas = Bitmap::new(width.max(1), height.max(1));
    let mut placed = Vec::new();
    let mut tags = Vec::new();
    let mut slices = Vec::new();
    let mut y = 0;
    for artboard in &doc.artboards {
        let baked = cs_render::bake_layers(doc, &artboard.sprite, cs_render::Bake::Everything);
        let s = baked.as_ref().unwrap_or(&artboard.sprite);
        let offset = placed.len();
        for f in 0..s.frames.len() {
            let x = f as u32 * s.width * scale;
            atlas.blit(&Bitmap::from_frame(s, f, scale), x, y);
            placed.push(Placed {
                name: format!("{} {f}.aseprite", artboard.name),
                x,
                y,
                w: s.width * scale,
                h: s.height * scale,
                duration: s.frames[f].duration_ms,
            });
        }
        tags.extend(
            s.tags
                .iter()
                .map(|t| tag_json(t, &format!("{}/{}", artboard.name, t.name), offset)),
        );
        slices.extend(slices_json(
            s,
            &format!("{}/", artboard.name),
            offset,
            scale,
        ));
        y += s.height * scale;
    }
    let artboards = Value::Array(
        doc.artboards
            .iter()
            .map(|a| json!({ "name": a.name, "position": [a.position.0, a.position.1], "frames": a.sprite.frames.len() }))
            .collect(),
    );
    let mut meta = meta(
        image_name,
        atlas.width,
        atlas.height,
        scale,
        tags,
        json!([]),
        slices,
    );
    meta["artboards"] = artboards;
    let json = json!({ "frames": frames_json(&placed, format), "meta": meta });
    Ok(Sheet {
        png: atlas.to_png()?,
        json: serde_json::to_string_pretty(&json)?,
    })
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_commands::ops;
    use cs_core::*;

    use super::*;

    fn decode_png(bytes: &[u8]) -> Bitmap {
        crate::import::decode_png(bytes).unwrap()
    }

    /// 2×1, tres frames distintos, un tag.
    fn sprite() -> Sprite {
        let mut s = Sprite::new(2, 1, ColorMode::Rgba);
        let colors = [
            Rgba::rgb(255, 0, 0),
            Rgba::rgb(0, 255, 0),
            Rgba::rgb(0, 0, 255),
        ];
        for (f, c) in colors.iter().enumerate() {
            if f > 0 {
                ops::duplicate_frame(&s.clone(), f - 1, false)
                    .unwrap()
                    .apply(&mut s)
                    .unwrap();
            }
            Command::DrawPixels {
                layer: LayerId(1),
                frame: f,
                pixels: vec![(0, 0, PixelValue::Rgba(*c))],
            }
            .apply(&mut s)
            .unwrap();
            s.frames[f].duration_ms = 100 + f as u16 * 50;
        }
        s.tags.push(Tag {
            name: "run".into(),
            from: 1,
            to: 2,
            direction: AnimDirection::PingPong,
            color: Rgba::rgb(1, 2, 3),
            repeat: 0,
        });
        s
    }

    #[test]
    fn png_with_scale() {
        let b = decode_png(&export_png(&sprite(), 1, 3).unwrap());
        assert_eq!((b.width, b.height), (6, 3));
        assert_eq!(b.pixels[0], Rgba::rgb(0, 255, 0));
        assert_eq!(b.pixels[2], Rgba::rgb(0, 255, 0));
        assert_eq!(b.pixels[3], Rgba::TRANSPARENT);
        assert_eq!(export_png_sequence(&sprite(), 1).unwrap().len(), 3);
    }

    #[test]
    fn gif_has_frames_delays_and_transparency() {
        let bytes = export_gif(&sprite(), 0, 2, 2).unwrap();
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options.read_info(std::io::Cursor::new(bytes)).unwrap();
        let mut delays = Vec::new();
        let mut first = None;
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            delays.push(frame.delay);
            first.get_or_insert_with(|| frame.buffer.to_vec());
        }
        assert_eq!(delays, vec![10, 15, 20]);
        let first = first.unwrap();
        assert_eq!(&first[0..4], &[255, 0, 0, 255]);
        assert_eq!(first[4 * 2 + 3], 0, "transparent pixel");
    }

    #[test]
    fn sheet_layouts_and_json() {
        let s = sprite();
        let sheet = export_sheet(
            &s,
            "hero",
            "hero.png",
            SheetLayout::Grid { columns: 2 },
            1,
            JsonFormat::Hash,
        )
        .unwrap();
        let png = decode_png(&sheet.png);
        assert_eq!((png.width, png.height), (4, 2));
        assert_eq!(
            png.pixels[4],
            Rgba::rgb(0, 0, 255),
            "third frame starts the second row"
        );
        let json: Value = serde_json::from_str(&sheet.json).unwrap();
        let keys: Vec<_> = json["frames"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            keys,
            vec!["hero 0.aseprite", "hero 1.aseprite", "hero 2.aseprite"]
        );
        assert_eq!(
            json["frames"]["hero 2.aseprite"]["frame"],
            json!({"x":0,"y":1,"w":2,"h":1})
        );
        assert_eq!(json["frames"]["hero 1.aseprite"]["duration"], 150);
        assert_eq!(json["meta"]["frameTags"][0]["direction"], "pingpong");
        assert_eq!(json["meta"]["size"], json!({"w":4,"h":2}));

        let horizontal = export_sheet(
            &s,
            "hero",
            "x.png",
            SheetLayout::Horizontal,
            2,
            JsonFormat::Array,
        )
        .unwrap();
        assert_eq!(decode_png(&horizontal.png).width, 12);
        let json: Value = serde_json::from_str(&horizontal.json).unwrap();
        assert_eq!(json["frames"][1]["filename"], "hero 1.aseprite");
        assert_eq!(json["frames"][1]["frame"]["x"], 4);
        let vertical = export_sheet(
            &s,
            "hero",
            "x.png",
            SheetLayout::Vertical,
            1,
            JsonFormat::Hash,
        )
        .unwrap();
        assert_eq!(decode_png(&vertical.png).height, 3);
    }

    #[test]
    fn atlas_stacks_artboards_and_offsets_tags() {
        let mut doc = Document::with_sprite("hero", sprite());
        doc.artboards.push(Artboard {
            guides: Vec::new(),
            symmetry: Default::default(),
            id: ArtboardId(2),
            name: "coin".into(),
            position: (10, 0),
            sprite: sprite(),
        });
        let atlas = export_atlas(&doc, "atlas.png", 1, JsonFormat::Hash).unwrap();
        let png = decode_png(&atlas.png);
        assert_eq!((png.width, png.height), (6, 2));
        let json: Value = serde_json::from_str(&atlas.json).unwrap();
        assert_eq!(json["frames"]["coin 0.aseprite"]["frame"]["y"], 1);
        assert_eq!(json["meta"]["frameTags"][1]["name"], "coin/run");
        assert_eq!(json["meta"]["frameTags"][1]["from"], 4);
        assert_eq!(json["meta"]["artboards"][1]["name"], "coin");
    }
}
