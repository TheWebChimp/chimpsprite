//! Tiled Map Editor: mapas `.tmx` (XML) y `.tmj` (JSON) con tilesets `.tsx`/`.tsj`.
//!
//! El export y el import pasan por un modelo intermedio (`Map`) que se escribe y se lee en los dos
//! formatos. Formato: <https://doc.mapeditor.org/en/stable/reference/tmx-map-format/> y
//! <https://doc.mapeditor.org/en/stable/reference/json-map-format/>.
//!
//! Los GID de Tiled usan los mismos bits de flip que `cs_core::Tile` (H = bit 31, V = bit 30,
//! D = bit 29, con D aplicado primero): solo cambia el índice, `gid = firstgid + tile − 1`.

use std::io::{Read as _, Write as _};

use base64::Engine as _;
use cs_core::{AllowedFlips, Rgba};

use crate::{IoError, Result};

mod export;
mod import;
#[cfg(test)]
mod tests;
mod tmj;
mod tmx;

pub use export::{TiledExport, TiledFrames, TiledOptions, export, has_tilemaps};
pub use import::{import, is_tiled_map};
pub use tmj::{read_tmj, read_tsj, write_tmj, write_tsj};
pub use tmx::{read_tmx, read_tsx, write_tmx, write_tsx};

/// Versión del formato que escribimos (la de Tiled 1.10+).
pub const FORMAT_VERSION: &str = "1.10";

/// Bits de flip del GID (los mismos que `cs_core::Tile`).
pub const FLIP_FLAGS: u32 = 0xe000_0000;

/// TMX (XML) o TMJ (JSON).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapFormat {
    Tmx,
    Tmj,
}

impl MapFormat {
    pub fn map_extension(self) -> &'static str {
        match self {
            MapFormat::Tmx => "tmx",
            MapFormat::Tmj => "tmj",
        }
    }

    /// Extensión del tileset externo en el mismo formato.
    pub fn tileset_extension(self) -> &'static str {
        match self {
            MapFormat::Tmx => "tsx",
            MapFormat::Tmj => "tsj",
        }
    }
}

/// Cómo se guardan los GID de una capa de tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Csv,
    Base64,
    Base64Zlib,
    Base64Gzip,
}

impl Encoding {
    pub const ALL: [Encoding; 4] = [
        Encoding::Csv,
        Encoding::Base64,
        Encoding::Base64Zlib,
        Encoding::Base64Gzip,
    ];

    /// Valores de `encoding` y `compression` del formato.
    fn attributes(self) -> (&'static str, Option<&'static str>) {
        match self {
            Encoding::Csv => ("csv", None),
            Encoding::Base64 => ("base64", None),
            Encoding::Base64Zlib => ("base64", Some("zlib")),
            Encoding::Base64Gzip => ("base64", Some("gzip")),
        }
    }

    fn from_attributes(encoding: Option<&str>, compression: Option<&str>) -> Result<Encoding> {
        match (encoding.unwrap_or("xml"), compression.unwrap_or("")) {
            ("csv", _) => Ok(Encoding::Csv),
            ("base64", "") => Ok(Encoding::Base64),
            ("base64", "zlib") => Ok(Encoding::Base64Zlib),
            ("base64", "gzip") => Ok(Encoding::Base64Gzip),
            ("base64", other) => Err(invalid(format!(
                "tile layer compression '{other}' is not supported"
            ))),
            (other, _) => Err(invalid(format!(
                "tile layer encoding '{other}' is not supported"
            ))),
        }
    }
}

/// Error de un archivo de Tiled mal formado o con algo que no soportamos.
pub(crate) fn invalid(message: impl Into<String>) -> IoError {
    IoError::Tiled(message.into())
}

/// GIDs en base64 (u32 little-endian, comprimidos o no).
fn encode_base64(gids: &[u32], encoding: Encoding) -> String {
    let raw: Vec<u8> = gids.iter().flat_map(|g| g.to_le_bytes()).collect();
    let bytes = match encoding {
        Encoding::Base64Zlib => {
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(&raw).expect("in memory");
            e.finish().expect("in memory")
        }
        Encoding::Base64Gzip => {
            let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(&raw).expect("in memory");
            e.finish().expect("in memory")
        }
        Encoding::Csv | Encoding::Base64 => raw,
    };
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// El inverso de `encode_base64`; `count` es el número de celdas esperado.
fn decode_base64(text: &str, encoding: Encoding, count: usize) -> Result<Vec<u32>> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(compact)
        .map_err(|e| invalid(format!("tile layer data is not valid base64: {e}")))?;
    // Nunca más bytes que los de la grilla (defensa contra bombas de compresión).
    let limit = count as u64 * 4 + 1;
    let mut raw = Vec::new();
    let read = match encoding {
        Encoding::Base64Zlib => flate2::read::ZlibDecoder::new(&bytes[..])
            .take(limit)
            .read_to_end(&mut raw),
        Encoding::Base64Gzip => flate2::read::GzDecoder::new(&bytes[..])
            .take(limit)
            .read_to_end(&mut raw),
        Encoding::Csv | Encoding::Base64 => {
            raw = bytes;
            Ok(0)
        }
    };
    read.map_err(|e| invalid(format!("tile layer data does not decompress: {e}")))?;
    if raw.len() != count * 4 {
        return Err(invalid(format!(
            "tile layer data has {} cells, expected {count}",
            raw.len() / 4
        )));
    }
    Ok(raw
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect())
}

/// GIDs separados por comas, una fila por línea (como los escribe Tiled).
fn encode_csv(gids: &[u32], width: u32) -> String {
    let width = width.max(1) as usize;
    let rows: Vec<String> = gids
        .chunks(width)
        .map(|row| row.iter().map(u32::to_string).collect::<Vec<_>>().join(","))
        .collect();
    rows.join(",\n")
}

fn decode_csv(text: &str, count: usize) -> Result<Vec<u32>> {
    let gids = text
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<u32>()
                .map_err(|_| invalid(format!("'{s}' is not a tile id")))
        })
        .collect::<Result<Vec<u32>>>()?;
    if gids.len() != count {
        return Err(invalid(format!(
            "tile layer data has {} cells, expected {count}",
            gids.len()
        )));
    }
    Ok(gids)
}

/// Mapa ortogonal finito.
#[derive(Clone, Debug, PartialEq)]
pub struct Map {
    /// En tiles.
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub infinite: bool,
    pub orientation: String,
    pub tilesets: Vec<MapTileset>,
    /// De abajo hacia arriba, como en el archivo.
    pub layers: Vec<MapLayer>,
    pub next_layer_id: u32,
    pub next_object_id: u32,
    pub properties: Vec<Property>,
}

/// Tileset tal como aparece en el mapa: embebido (`source = None`) o externo.
#[derive(Clone, Debug, PartialEq)]
pub struct MapTileset {
    pub first_gid: u32,
    /// Archivo del tileset externo, relativo al mapa.
    pub source: Option<String>,
    /// Con `source` solo vale al leer (se carga del archivo) y al escribir el `.tsx` aparte.
    pub tileset: TiledTileset,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TiledTileset {
    pub name: String,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tile_count: u32,
    pub columns: u32,
    pub margin: u32,
    pub spacing: u32,
    /// `None` en las colecciones de imágenes (no las soportamos).
    pub image: Option<TiledImage>,
    /// Flips permitidos (`<transformations>`): `d` es `rotate`.
    pub transformations: Option<AllowedFlips>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TiledImage {
    /// Relativo al archivo que lo menciona.
    pub source: String,
    pub width: u32,
    pub height: u32,
    /// Color que se vuelve transparente.
    pub trans: Option<Rgba>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapLayer {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    /// 0–1.
    pub opacity: f32,
    /// En píxeles.
    pub offset: (i32, i32),
    /// Blend de Tiled (`multiply`, `add`…); `None` es normal.
    pub mode: Option<String>,
    pub properties: Vec<Property>,
    pub content: LayerContent,
}

impl MapLayer {
    pub fn new(id: u32, name: impl Into<String>, content: LayerContent) -> MapLayer {
        MapLayer {
            id,
            name: name.into(),
            visible: true,
            opacity: 1.0,
            offset: (0, 0),
            mode: None,
            properties: Vec::new(),
            content,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayerContent {
    /// GIDs fila por fila; `encoding` es cómo se escriben (al leer, cómo venían).
    Tiles {
        width: u32,
        height: u32,
        gids: Vec<u32>,
        encoding: Encoding,
    },
    Image(Option<TiledImage>),
    Objects(Vec<MapObject>),
    Group(Vec<MapLayer>),
}

/// Objeto de una capa de objetos; solo usamos rectángulos.
#[derive(Clone, Debug, PartialEq)]
pub struct MapObject {
    pub id: u32,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Elipses, polígonos, texto, tiles: los ignoramos al importar.
    pub rectangle: bool,
    pub properties: Vec<Property>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub name: String,
    pub value: PropertyValue,
}

impl Property {
    pub fn new(name: impl Into<String>, value: PropertyValue) -> Property {
        Property {
            name: name.into(),
            value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Color(Rgba),
}

impl PropertyValue {
    fn type_name(&self) -> &'static str {
        match self {
            PropertyValue::Str(_) => "string",
            PropertyValue::Int(_) => "int",
            PropertyValue::Float(_) => "float",
            PropertyValue::Bool(_) => "bool",
            PropertyValue::Color(_) => "color",
        }
    }

    /// El valor como lo escribe el XML (y el JSON para colores).
    fn text(&self) -> String {
        match self {
            PropertyValue::Str(s) => s.clone(),
            PropertyValue::Int(i) => i.to_string(),
            PropertyValue::Float(f) => f.to_string(),
            PropertyValue::Bool(b) => b.to_string(),
            PropertyValue::Color(c) => color_text(*c),
        }
    }

    fn parse(kind: &str, text: &str) -> PropertyValue {
        match kind {
            "int" | "object" => text
                .parse()
                .map_or_else(|_| PropertyValue::Str(text.into()), PropertyValue::Int),
            "float" => text
                .parse()
                .map_or_else(|_| PropertyValue::Str(text.into()), PropertyValue::Float),
            "bool" => PropertyValue::Bool(text == "true"),
            "color" => parse_color(text)
                .map_or_else(|| PropertyValue::Str(text.into()), PropertyValue::Color),
            _ => PropertyValue::Str(text.into()),
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            PropertyValue::Int(i) => Some(*i),
            PropertyValue::Float(f) => Some(f.round() as i64),
            _ => None,
        }
    }
}

/// `#AARRGGBB`, como guarda Tiled los colores.
pub fn color_text(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.a, c.r, c.g, c.b)
}

/// `#RRGGBB` o `#AARRGGBB` (con o sin `#`).
pub fn parse_color(text: &str) -> Option<Rgba> {
    let hex = text.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        6 => Some(Rgba::new(byte(0)?, byte(2)?, byte(4)?, 255)),
        8 => Some(Rgba::new(byte(2)?, byte(4)?, byte(6)?, byte(0)?)),
        _ => None,
    }
}

/// Opacidad con 3 decimales como mucho (y sin ceros de más).
fn opacity_text(opacity: f32) -> String {
    let rounded = (opacity.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;
    format!("{rounded}")
}

/// Ruta `relative` vista desde la carpeta del archivo `from` (rutas con `/`). Normaliza `.` y
/// `..`; las absolutas quedan igual.
pub fn resolve_path(from: &str, relative: &str) -> String {
    let relative = relative.replace('\\', "/");
    if relative.starts_with('/') || relative.contains(':') {
        return relative;
    }
    let from = from.replace('\\', "/");
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != ".." && !p.is_empty()) => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.retain(|p| !p.is_empty());
    parts.join("/")
}
