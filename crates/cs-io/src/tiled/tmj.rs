//! `.tmj` y `.tsj`: el formato JSON de Tiled.

use cs_core::AllowedFlips;
use serde_json::{Map as JsonMap, Value, json};

use super::{
    Encoding, FORMAT_VERSION, LayerContent, Map, MapLayer, MapObject, MapTileset, Property,
    PropertyValue, TiledImage, TiledTileset, color_text, decode_base64, encode_base64, invalid,
    parse_color, tmx::empty_tileset,
};
use crate::Result;

fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("serializable");
    text.push('\n');
    text
}

pub fn write_tmj(map: &Map) -> String {
    let tilesets: Vec<Value> = map
        .tilesets
        .iter()
        .map(|ts| match &ts.source {
            Some(source) => json!({"firstgid": ts.first_gid, "source": source}),
            None => {
                let mut v = tileset_json(&ts.tileset);
                v.insert("firstgid".into(), json!(ts.first_gid));
                Value::Object(v)
            }
        })
        .collect();
    let mut v = json!({
        "type": "map",
        "version": FORMAT_VERSION,
        "orientation": map.orientation,
        "renderorder": "right-down",
        "width": map.width,
        "height": map.height,
        "tilewidth": map.tile_width,
        "tileheight": map.tile_height,
        "infinite": map.infinite,
        "nextlayerid": map.next_layer_id,
        "nextobjectid": map.next_object_id,
        "tilesets": tilesets,
        "layers": map.layers.iter().map(layer_json).collect::<Vec<_>>(),
    });
    if !map.properties.is_empty() {
        v["properties"] = properties_json(&map.properties);
    }
    pretty(&v)
}

pub fn write_tsj(tileset: &TiledTileset) -> String {
    let mut v = tileset_json(tileset);
    v.insert("type".into(), json!("tileset"));
    v.insert("version".into(), json!(FORMAT_VERSION));
    pretty(&Value::Object(v))
}

fn tileset_json(ts: &TiledTileset) -> JsonMap<String, Value> {
    let mut v = JsonMap::new();
    v.insert("name".into(), json!(ts.name));
    v.insert("tilewidth".into(), json!(ts.tile_width));
    v.insert("tileheight".into(), json!(ts.tile_height));
    v.insert("tilecount".into(), json!(ts.tile_count));
    v.insert("columns".into(), json!(ts.columns));
    v.insert("margin".into(), json!(ts.margin));
    v.insert("spacing".into(), json!(ts.spacing));
    if let Some(image) = &ts.image {
        v.insert("image".into(), json!(image.source));
        v.insert("imagewidth".into(), json!(image.width));
        v.insert("imageheight".into(), json!(image.height));
        if let Some(trans) = image.trans {
            v.insert(
                "transparentcolor".into(),
                json!(format!("#{}", &color_text(trans)[3..])),
            );
        }
    }
    if let Some(f) = ts.transformations {
        v.insert(
            "transformations".into(),
            json!({"hflip": f.x, "vflip": f.y, "rotate": f.d, "preferuntransformed": true}),
        );
    }
    v
}

fn properties_json(properties: &[Property]) -> Value {
    Value::Array(
        properties
            .iter()
            .map(|p| {
                let value = match &p.value {
                    PropertyValue::Str(s) => json!(s),
                    PropertyValue::Int(i) => json!(i),
                    PropertyValue::Float(f) => json!(f),
                    PropertyValue::Bool(b) => json!(b),
                    PropertyValue::Color(c) => json!(color_text(*c)),
                };
                json!({"name": p.name, "type": p.value.type_name(), "value": value})
            })
            .collect(),
    )
}

fn layer_json(layer: &MapLayer) -> Value {
    let mut v = JsonMap::new();
    v.insert("id".into(), json!(layer.id));
    v.insert("name".into(), json!(layer.name));
    let kind = match &layer.content {
        LayerContent::Tiles { .. } => "tilelayer",
        LayerContent::Image(_) => "imagelayer",
        LayerContent::Objects(_) => "objectgroup",
        LayerContent::Group(_) => "group",
    };
    v.insert("type".into(), json!(kind));
    v.insert("visible".into(), json!(layer.visible));
    v.insert(
        "opacity".into(),
        json!((layer.opacity.clamp(0.0, 1.0) as f64 * 1000.0).round() / 1000.0),
    );
    v.insert("x".into(), json!(0));
    v.insert("y".into(), json!(0));
    if layer.offset != (0, 0) {
        v.insert("offsetx".into(), json!(layer.offset.0));
        v.insert("offsety".into(), json!(layer.offset.1));
    }
    if let Some(mode) = &layer.mode {
        v.insert("mode".into(), json!(mode));
    }
    if !layer.properties.is_empty() {
        v.insert("properties".into(), properties_json(&layer.properties));
    }
    match &layer.content {
        LayerContent::Tiles {
            width,
            height,
            gids,
            encoding,
        } => {
            v.insert("width".into(), json!(width));
            v.insert("height".into(), json!(height));
            let (enc, compression) = encoding.attributes();
            v.insert("encoding".into(), json!(enc));
            if let Some(c) = compression {
                v.insert("compression".into(), json!(c));
            }
            let data = match encoding {
                Encoding::Csv => json!(gids),
                other => json!(encode_base64(gids, *other)),
            };
            v.insert("data".into(), data);
        }
        LayerContent::Image(image) => {
            let (source, w, h) = image.as_ref().map_or((String::new(), 0, 0), |i| {
                (i.source.clone(), i.width, i.height)
            });
            v.insert("image".into(), json!(source));
            v.insert("imagewidth".into(), json!(w));
            v.insert("imageheight".into(), json!(h));
        }
        LayerContent::Objects(objects) => {
            v.insert("draworder".into(), json!("topdown"));
            let objects: Vec<Value> = objects
                .iter()
                .map(|o| {
                    let mut ov = json!({
                        "id": o.id,
                        "name": o.name,
                        "type": "",
                        "x": o.x,
                        "y": o.y,
                        "width": o.width,
                        "height": o.height,
                        "rotation": 0,
                        "visible": true,
                    });
                    if !o.properties.is_empty() {
                        ov["properties"] = properties_json(&o.properties);
                    }
                    ov
                })
                .collect();
            v.insert("objects".into(), json!(objects));
        }
        LayerContent::Group(children) => {
            v.insert(
                "layers".into(),
                json!(children.iter().map(layer_json).collect::<Vec<_>>()),
            );
        }
    }
    Value::Object(v)
}

// --- Lectura ---

fn parse(text: &str) -> Result<Value> {
    serde_json::from_str(text).map_err(|e| invalid(format!("not valid JSON: {e}")))
}

fn uint(v: &Value, key: &str) -> Result<Option<u32>> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(n) => n
            .as_f64()
            .filter(|f| *f >= 0.0 && *f <= u32::MAX as f64)
            .map(|f| Some(f as u32))
            .ok_or_else(|| invalid(format!("'{key}' must be a number"))),
    }
}

fn required(v: &Value, key: &str) -> Result<u32> {
    uint(v, key)?.ok_or_else(|| invalid(format!("missing '{key}'")))
}

fn float(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn string<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

pub fn read_tmj(text: &str) -> Result<Map> {
    let v = parse(text)?;
    let infinite = v.get("infinite").and_then(Value::as_bool).unwrap_or(false);
    let mut tilesets = Vec::new();
    for ts in v
        .get("tilesets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let first_gid = required(ts, "firstgid")?;
        tilesets.push(match string(ts, "source") {
            Some(source) => MapTileset {
                first_gid,
                source: Some(source.into()),
                tileset: empty_tileset(),
            },
            None => MapTileset {
                first_gid,
                source: None,
                tileset: read_tileset(ts)?,
            },
        });
    }
    Ok(Map {
        width: required(&v, "width")?,
        height: required(&v, "height")?,
        tile_width: required(&v, "tilewidth")?,
        tile_height: required(&v, "tileheight")?,
        infinite,
        orientation: string(&v, "orientation").unwrap_or("orthogonal").into(),
        tilesets,
        layers: read_layers(&v, infinite)?,
        next_layer_id: uint(&v, "nextlayerid")?.unwrap_or(1),
        next_object_id: uint(&v, "nextobjectid")?.unwrap_or(1),
        properties: read_properties(&v),
    })
}

pub fn read_tsj(text: &str) -> Result<TiledTileset> {
    read_tileset(&parse(text)?)
}

fn read_tileset(v: &Value) -> Result<TiledTileset> {
    let image = match string(v, "image") {
        Some(source) if !source.is_empty() => Some(TiledImage {
            source: source.into(),
            width: uint(v, "imagewidth")?.unwrap_or(0),
            height: uint(v, "imageheight")?.unwrap_or(0),
            trans: string(v, "transparentcolor").and_then(parse_color),
        }),
        _ => None,
    };
    let flag = |t: &Value, k: &str| t.get(k).and_then(Value::as_bool).unwrap_or(false);
    Ok(TiledTileset {
        name: string(v, "name").unwrap_or("").into(),
        tile_width: required(v, "tilewidth")?,
        tile_height: required(v, "tileheight")?,
        tile_count: uint(v, "tilecount")?.unwrap_or(0),
        columns: uint(v, "columns")?.unwrap_or(0),
        margin: uint(v, "margin")?.unwrap_or(0),
        spacing: uint(v, "spacing")?.unwrap_or(0),
        image,
        transformations: v.get("transformations").map(|t| AllowedFlips {
            x: flag(t, "hflip"),
            y: flag(t, "vflip"),
            d: flag(t, "rotate"),
        }),
    })
}

fn read_properties(v: &Value) -> Vec<Property> {
    v.get("properties")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|p| {
            let kind = string(p, "type").unwrap_or("string");
            let value = match p.get("value") {
                Some(Value::String(s)) => PropertyValue::parse(kind, s),
                Some(Value::Bool(b)) => PropertyValue::Bool(*b),
                Some(Value::Number(n)) if kind == "float" => {
                    PropertyValue::Float(n.as_f64().unwrap_or(0.0))
                }
                Some(Value::Number(n)) => n.as_i64().map_or(
                    PropertyValue::Float(n.as_f64().unwrap_or(0.0)),
                    PropertyValue::Int,
                ),
                Some(other) => PropertyValue::Str(other.to_string()),
                None => PropertyValue::Str(String::new()),
            };
            Property::new(string(p, "name").unwrap_or(""), value)
        })
        .collect()
}

fn read_layers(parent: &Value, infinite: bool) -> Result<Vec<MapLayer>> {
    let mut layers = Vec::new();
    for v in parent
        .get("layers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let content = match string(v, "type").unwrap_or("") {
            "tilelayer" => read_tile_data(v, infinite)?,
            "imagelayer" => LayerContent::Image(match string(v, "image") {
                Some(source) if !source.is_empty() => Some(TiledImage {
                    source: source.into(),
                    width: uint(v, "imagewidth")?.unwrap_or(0),
                    height: uint(v, "imageheight")?.unwrap_or(0),
                    trans: string(v, "transparentcolor").and_then(parse_color),
                }),
                _ => None,
            }),
            "objectgroup" => LayerContent::Objects(read_objects(v)),
            "group" => LayerContent::Group(read_layers(v, infinite)?),
            _ => continue,
        };
        layers.push(MapLayer {
            id: uint(v, "id")?.unwrap_or(0),
            name: string(v, "name").unwrap_or("").into(),
            visible: v.get("visible").and_then(Value::as_bool).unwrap_or(true),
            opacity: float(v, "opacity").unwrap_or(1.0) as f32,
            offset: (
                float(v, "offsetx").unwrap_or(0.0).round() as i32,
                float(v, "offsety").unwrap_or(0.0).round() as i32,
            ),
            mode: string(v, "mode")
                .filter(|m| *m != "normal")
                .map(str::to_string),
            properties: read_properties(v),
            content,
        });
    }
    Ok(layers)
}

fn read_tile_data(v: &Value, infinite: bool) -> Result<LayerContent> {
    if infinite || v.get("chunks").is_some() {
        return Err(invalid("infinite maps are not supported yet"));
    }
    let width = required(v, "width")?;
    let height = required(v, "height")?;
    let count = width as usize * height as usize;
    let encoding = Encoding::from_attributes(
        Some(string(v, "encoding").unwrap_or("csv")),
        string(v, "compression"),
    )?;
    let gids = match (v.get("data"), encoding) {
        (Some(Value::Array(cells)), _) => {
            let gids: Vec<u32> = cells
                .iter()
                .map(|c| {
                    c.as_u64()
                        .filter(|g| *g <= u32::MAX as u64)
                        .map(|g| g as u32)
                        .ok_or_else(|| invalid(format!("'{c}' is not a tile id")))
                })
                .collect::<Result<_>>()?;
            if gids.len() != count {
                return Err(invalid(format!(
                    "tile layer data has {} cells, expected {count}",
                    gids.len()
                )));
            }
            gids
        }
        (Some(Value::String(text)), e) if e != Encoding::Csv => decode_base64(text, e, count)?,
        _ => return Err(invalid("tile layer without data")),
    };
    Ok(LayerContent::Tiles {
        width,
        height,
        gids,
        encoding,
    })
}

fn read_objects(v: &Value) -> Vec<MapObject> {
    v.get("objects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|o| {
            let shaped = ["ellipse", "point", "polygon", "polyline", "text", "gid"]
                .iter()
                .any(|k| {
                    o.get(k)
                        .is_some_and(|x| !x.is_null() && x != &Value::Bool(false))
                });
            let width = float(o, "width").unwrap_or(0.0);
            let height = float(o, "height").unwrap_or(0.0);
            MapObject {
                id: uint(o, "id").ok().flatten().unwrap_or(0),
                name: string(o, "name").unwrap_or("").into(),
                x: float(o, "x").unwrap_or(0.0),
                y: float(o, "y").unwrap_or(0.0),
                width,
                height,
                rectangle: !shaped && width > 0.0 && height > 0.0,
                properties: read_properties(o),
            }
        })
        .collect()
}
