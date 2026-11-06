//! `.tmx` y `.tsx`: el formato XML de Tiled.

use std::fmt::Write as _;

use cs_core::AllowedFlips;
use quick_xml::events::Event;

use super::{
    Encoding, FORMAT_VERSION, LayerContent, Map, MapLayer, MapObject, MapTileset, Property,
    PropertyValue, TiledImage, TiledTileset, color_text, decode_base64, decode_csv, encode_base64,
    encode_csv, invalid, opacity_text, parse_color,
};
use crate::Result;

const HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";

/// Texto apto para un atributo XML.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
    out
}

/// El `.tmx` del mapa. Los tilesets con `source` se escriben como referencia (su `.tsx` va
/// aparte, con `write_tsx`).
pub fn write_tmx(map: &Map) -> String {
    let mut out = String::from(HEADER);
    let _ = writeln!(
        out,
        "<map version=\"{FORMAT_VERSION}\" orientation=\"{}\" renderorder=\"right-down\" width=\"{}\" height=\"{}\" tilewidth=\"{}\" tileheight=\"{}\" infinite=\"{}\" nextlayerid=\"{}\" nextobjectid=\"{}\">",
        esc(&map.orientation),
        map.width,
        map.height,
        map.tile_width,
        map.tile_height,
        u8::from(map.infinite),
        map.next_layer_id,
        map.next_object_id
    );
    write_properties(&mut out, &map.properties, 1);
    for ts in &map.tilesets {
        match &ts.source {
            Some(source) => {
                let _ = writeln!(
                    out,
                    " <tileset firstgid=\"{}\" source=\"{}\"/>",
                    ts.first_gid,
                    esc(source)
                );
            }
            None => write_tileset(&mut out, &ts.tileset, Some(ts.first_gid), 1),
        }
    }
    for layer in &map.layers {
        write_layer(&mut out, layer, 1);
    }
    out.push_str("</map>\n");
    out
}

/// Un `.tsx` (tileset externo).
pub fn write_tsx(tileset: &TiledTileset) -> String {
    let mut out = String::from(HEADER);
    write_tileset(&mut out, tileset, None, 0);
    out
}

fn indent(depth: usize) -> String {
    " ".repeat(depth)
}

fn write_tileset(out: &mut String, ts: &TiledTileset, first_gid: Option<u32>, depth: usize) {
    let pad = indent(depth);
    let lead = match first_gid {
        Some(gid) => format!("firstgid=\"{gid}\""),
        None => format!("version=\"{FORMAT_VERSION}\""),
    };
    let _ = write!(
        out,
        "{pad}<tileset {lead} name=\"{}\" tilewidth=\"{}\" tileheight=\"{}\"",
        esc(&ts.name),
        ts.tile_width,
        ts.tile_height
    );
    if ts.spacing != 0 {
        let _ = write!(out, " spacing=\"{}\"", ts.spacing);
    }
    if ts.margin != 0 {
        let _ = write!(out, " margin=\"{}\"", ts.margin);
    }
    let _ = writeln!(
        out,
        " tilecount=\"{}\" columns=\"{}\">",
        ts.tile_count, ts.columns
    );
    if let Some(f) = ts.transformations {
        let _ = writeln!(
            out,
            "{pad} <transformations hflip=\"{}\" vflip=\"{}\" rotate=\"{}\" preferuntransformed=\"1\"/>",
            u8::from(f.x),
            u8::from(f.y),
            u8::from(f.d)
        );
    }
    if let Some(image) = &ts.image {
        write_image(out, image, depth + 1);
    }
    let _ = writeln!(out, "{pad}</tileset>");
}

fn write_image(out: &mut String, image: &TiledImage, depth: usize) {
    let _ = write!(
        out,
        "{}<image source=\"{}\"",
        indent(depth),
        esc(&image.source)
    );
    if let Some(trans) = image.trans {
        // Sin `#` ni alfa, como lo escribe Tiled.
        let _ = write!(out, " trans=\"{}\"", &color_text(trans)[3..]);
    }
    let _ = writeln!(
        out,
        " width=\"{}\" height=\"{}\"/>",
        image.width, image.height
    );
}

fn write_properties(out: &mut String, properties: &[Property], depth: usize) {
    if properties.is_empty() {
        return;
    }
    let pad = indent(depth);
    let _ = writeln!(out, "{pad}<properties>");
    for p in properties {
        let kind = match p.value {
            PropertyValue::Str(_) => String::new(),
            ref v => format!(" type=\"{}\"", v.type_name()),
        };
        let _ = writeln!(
            out,
            "{pad} <property name=\"{}\"{kind} value=\"{}\"/>",
            esc(&p.name),
            esc(&p.value.text())
        );
    }
    let _ = writeln!(out, "{pad}</properties>");
}

/// Atributos comunes a todas las capas (después de `id` y `name`).
fn common_attributes(layer: &MapLayer) -> String {
    let mut out = String::new();
    if !layer.visible {
        out.push_str(" visible=\"0\"");
    }
    if layer.opacity < 1.0 {
        let _ = write!(out, " opacity=\"{}\"", opacity_text(layer.opacity));
    }
    if layer.offset.0 != 0 {
        let _ = write!(out, " offsetx=\"{}\"", layer.offset.0);
    }
    if layer.offset.1 != 0 {
        let _ = write!(out, " offsety=\"{}\"", layer.offset.1);
    }
    if let Some(mode) = &layer.mode {
        let _ = write!(out, " mode=\"{}\"", esc(mode));
    }
    out
}

fn write_layer(out: &mut String, layer: &MapLayer, depth: usize) {
    let pad = indent(depth);
    let head = format!(
        "id=\"{}\" name=\"{}\"{}",
        layer.id,
        esc(&layer.name),
        common_attributes(layer)
    );
    match &layer.content {
        LayerContent::Tiles {
            width,
            height,
            gids,
            encoding,
        } => {
            let _ = writeln!(
                out,
                "{pad}<layer {head} width=\"{width}\" height=\"{height}\">"
            );
            write_properties(out, &layer.properties, depth + 1);
            let (enc, compression) = encoding.attributes();
            let compression = compression
                .map(|c| format!(" compression=\"{c}\""))
                .unwrap_or_default();
            let data = match encoding {
                Encoding::Csv => format!("\n{}\n{pad} ", encode_csv(gids, *width)),
                other => format!("\n{pad}  {}\n{pad} ", encode_base64(gids, *other)),
            };
            let _ = writeln!(
                out,
                "{pad} <data encoding=\"{enc}\"{compression}>{data}</data>"
            );
            let _ = writeln!(out, "{pad}</layer>");
        }
        LayerContent::Image(image) => {
            let _ = writeln!(out, "{pad}<imagelayer {head}>");
            write_properties(out, &layer.properties, depth + 1);
            if let Some(image) = image {
                write_image(out, image, depth + 1);
            }
            let _ = writeln!(out, "{pad}</imagelayer>");
        }
        LayerContent::Objects(objects) => {
            let _ = writeln!(out, "{pad}<objectgroup {head}>");
            write_properties(out, &layer.properties, depth + 1);
            for o in objects {
                let _ = write!(
                    out,
                    "{pad} <object id=\"{}\" name=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                    o.id,
                    esc(&o.name),
                    o.x,
                    o.y,
                    o.width,
                    o.height
                );
                if o.properties.is_empty() {
                    out.push_str("/>\n");
                } else {
                    out.push_str(">\n");
                    write_properties(out, &o.properties, depth + 2);
                    let _ = writeln!(out, "{pad} </object>");
                }
            }
            let _ = writeln!(out, "{pad}</objectgroup>");
        }
        LayerContent::Group(children) => {
            let _ = writeln!(out, "{pad}<group {head}>");
            write_properties(out, &layer.properties, depth + 1);
            for child in children {
                write_layer(out, child, depth + 1);
            }
            let _ = writeln!(out, "{pad}</group>");
        }
    }
}

// --- Lectura ---

/// Un elemento XML ya leído (un árbol chico: los archivos de Tiled no son enormes).
#[derive(Debug, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    pub text: String,
}

impl Element {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    fn num<T: std::str::FromStr>(&self, key: &str) -> Result<Option<T>> {
        match self.attr(key) {
            None => Ok(None),
            Some(v) => v
                .trim()
                .parse()
                .map(Some)
                .map_err(|_| invalid(format!("<{}> has an invalid {key}: '{v}'", self.name))),
        }
    }

    fn required<T: std::str::FromStr>(&self, key: &str) -> Result<T> {
        self.num(key)?
            .ok_or_else(|| invalid(format!("<{}> is missing {key}", self.name)))
    }

    fn flag(&self, key: &str, default: bool) -> bool {
        self.attr(key)
            .map_or(default, |v| matches!(v.trim(), "1" | "true"))
    }
}

/// Lee un documento XML entero como árbol.
pub(crate) fn parse_xml(text: &str) -> Result<Element> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<Element> = vec![Element::default()];
    let open = |e: &quick_xml::events::BytesStart| -> Result<Element> {
        let mut el = Element {
            name: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
            ..Default::default()
        };
        for attr in e.attributes() {
            let attr = attr.map_err(|e| invalid(format!("bad XML attribute: {e}")))?;
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|e| invalid(format!("bad XML attribute: {e}")))?;
            el.attrs.push((
                String::from_utf8_lossy(attr.key.as_ref()).into_owned(),
                value.into_owned(),
            ));
        }
        Ok(el)
    };
    // Los elementos no se cierran solos: `stack` nunca queda vacío salvo con XML roto.
    let parent = |stack: &mut Vec<Element>| -> Result<Element> {
        stack.pop().ok_or_else(|| invalid("unbalanced XML"))
    };
    loop {
        match reader
            .read_event()
            .map_err(|e| invalid(format!("not valid XML: {e}")))?
        {
            Event::Start(e) => stack.push(open(&e)?),
            Event::Empty(e) => {
                let el = open(&e)?;
                stack.last_mut().expect("root").children.push(el);
            }
            Event::End(_) => {
                let mut el = parent(&mut stack)?;
                el.text = quick_xml::escape::unescape(&el.text)
                    .map_err(|e| invalid(format!("bad XML text: {e}")))?
                    .into_owned();
                stack
                    .last_mut()
                    .ok_or_else(|| invalid("unbalanced XML"))?
                    .children
                    .push(el);
            }
            Event::Text(t) => {
                let text = t
                    .decode()
                    .map_err(|e| invalid(format!("bad XML text: {e}")))?;
                if let Some(el) = stack.last_mut() {
                    el.text.push_str(&text);
                }
            }
            Event::GeneralRef(r) => {
                let name = r
                    .decode()
                    .map_err(|e| invalid(format!("bad XML text: {e}")))?;
                if let Some(el) = stack.last_mut() {
                    let _ = write!(el.text, "&{name};");
                }
            }
            Event::CData(t) => {
                let text = t
                    .decode()
                    .map_err(|e| invalid(format!("bad XML text: {e}")))?;
                if let Some(el) = stack.last_mut() {
                    // Ya sin escapar: se escapa para que el `unescape` del cierre lo deje igual.
                    el.text.push_str(&quick_xml::escape::escape(text.as_ref()));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut root = match stack.len() {
        1 => stack.pop().expect("root"),
        _ => return Err(invalid("unbalanced XML")),
    };
    match root.children.len() {
        1 => Ok(root.children.pop().expect("one")),
        _ => Err(invalid("the XML has no single root element")),
    }
}

pub fn read_tmx(text: &str) -> Result<Map> {
    let root = parse_xml(text)?;
    if root.name != "map" {
        return Err(invalid(format!("expected <map>, found <{}>", root.name)));
    }
    let mut map = Map {
        width: root.required("width")?,
        height: root.required("height")?,
        tile_width: root.required("tilewidth")?,
        tile_height: root.required("tileheight")?,
        infinite: root.flag("infinite", false),
        orientation: root.attr("orientation").unwrap_or("orthogonal").into(),
        tilesets: Vec::new(),
        layers: Vec::new(),
        next_layer_id: root.num("nextlayerid")?.unwrap_or(1),
        next_object_id: root.num("nextobjectid")?.unwrap_or(1),
        properties: read_properties(&root),
    };
    for child in &root.children {
        if child.name == "tileset" {
            let first_gid = child.required("firstgid")?;
            map.tilesets.push(match child.attr("source") {
                Some(source) => MapTileset {
                    first_gid,
                    source: Some(source.into()),
                    tileset: empty_tileset(),
                },
                None => MapTileset {
                    first_gid,
                    source: None,
                    tileset: read_tileset(child)?,
                },
            });
        }
    }
    map.layers = read_layers(&root, map.infinite)?;
    Ok(map)
}

pub fn read_tsx(text: &str) -> Result<TiledTileset> {
    let root = parse_xml(text)?;
    if root.name != "tileset" {
        return Err(invalid(format!(
            "expected <tileset>, found <{}>",
            root.name
        )));
    }
    read_tileset(&root)
}

/// Tileset vacío (lugar de uno externo hasta cargarlo).
pub(crate) fn empty_tileset() -> TiledTileset {
    TiledTileset {
        name: String::new(),
        tile_width: 0,
        tile_height: 0,
        tile_count: 0,
        columns: 0,
        margin: 0,
        spacing: 0,
        image: None,
        transformations: None,
    }
}

fn read_tileset(el: &Element) -> Result<TiledTileset> {
    Ok(TiledTileset {
        name: el.attr("name").unwrap_or("").into(),
        tile_width: el.required("tilewidth")?,
        tile_height: el.required("tileheight")?,
        tile_count: el.num("tilecount")?.unwrap_or(0),
        columns: el.num("columns")?.unwrap_or(0),
        margin: el.num("margin")?.unwrap_or(0),
        spacing: el.num("spacing")?.unwrap_or(0),
        image: el.child("image").map(read_image).transpose()?,
        transformations: el.child("transformations").map(|t| AllowedFlips {
            x: t.flag("hflip", false),
            y: t.flag("vflip", false),
            d: t.flag("rotate", false),
        }),
    })
}

fn read_image(el: &Element) -> Result<TiledImage> {
    Ok(TiledImage {
        source: el
            .attr("source")
            .ok_or_else(|| invalid("<image> without source"))?
            .into(),
        width: el.num("width")?.unwrap_or(0),
        height: el.num("height")?.unwrap_or(0),
        trans: el.attr("trans").and_then(parse_color),
    })
}

fn read_properties(el: &Element) -> Vec<Property> {
    let Some(props) = el.child("properties") else {
        return Vec::new();
    };
    props
        .children
        .iter()
        .filter(|p| p.name == "property")
        .map(|p| {
            let text = p
                .attr("value")
                .map_or_else(|| p.text.clone(), str::to_string);
            Property::new(
                p.attr("name").unwrap_or(""),
                PropertyValue::parse(p.attr("type").unwrap_or("string"), &text),
            )
        })
        .collect()
}

fn read_layers(parent: &Element, infinite: bool) -> Result<Vec<MapLayer>> {
    let mut layers = Vec::new();
    for el in &parent.children {
        let content = match el.name.as_str() {
            "layer" => read_tile_data(el, infinite)?,
            "imagelayer" => LayerContent::Image(el.child("image").map(read_image).transpose()?),
            "objectgroup" => LayerContent::Objects(read_objects(el)?),
            "group" => LayerContent::Group(read_layers(el, infinite)?),
            _ => continue,
        };
        let offset =
            |key: &str| -> Result<i32> { Ok(el.num::<f64>(key)?.unwrap_or(0.0).round() as i32) };
        layers.push(MapLayer {
            id: el.num("id")?.unwrap_or(0),
            name: el.attr("name").unwrap_or("").into(),
            visible: el.flag("visible", true),
            opacity: el.num("opacity")?.unwrap_or(1.0),
            offset: (offset("offsetx")?, offset("offsety")?),
            mode: el
                .attr("mode")
                .filter(|m| *m != "normal")
                .map(str::to_string),
            properties: read_properties(el),
            content,
        });
    }
    Ok(layers)
}

fn read_tile_data(el: &Element, infinite: bool) -> Result<LayerContent> {
    if infinite {
        return Err(invalid("infinite maps are not supported yet"));
    }
    let width: u32 = el.required("width")?;
    let height: u32 = el.required("height")?;
    let count = width as usize * height as usize;
    let data = el
        .child("data")
        .ok_or_else(|| invalid("<layer> without <data>"))?;
    let encoding = Encoding::from_attributes(data.attr("encoding"), data.attr("compression"));
    let (gids, encoding) = match (data.attr("encoding"), encoding) {
        // Sin `encoding`: un `<tile gid>` por celda (el formato viejo).
        (None, _) => {
            let gids: Vec<u32> = data
                .children
                .iter()
                .filter(|t| t.name == "tile")
                .map(|t| t.num("gid").map(|g| g.unwrap_or(0)))
                .collect::<Result<_>>()?;
            if gids.len() != count {
                return Err(invalid(format!(
                    "tile layer data has {} cells, expected {count}",
                    gids.len()
                )));
            }
            (gids, Encoding::Csv)
        }
        (_, Ok(Encoding::Csv)) => (decode_csv(&data.text, count)?, Encoding::Csv),
        (_, Ok(e)) => (decode_base64(&data.text, e, count)?, e),
        (_, Err(e)) => return Err(e),
    };
    Ok(LayerContent::Tiles {
        width,
        height,
        gids,
        encoding,
    })
}

fn read_objects(el: &Element) -> Result<Vec<MapObject>> {
    el.children
        .iter()
        .filter(|o| o.name == "object")
        .map(|o| {
            let shaped = ["ellipse", "point", "polygon", "polyline", "text"]
                .iter()
                .any(|s| o.child(s).is_some());
            let width: f64 = o.num("width")?.unwrap_or(0.0);
            let height: f64 = o.num("height")?.unwrap_or(0.0);
            Ok(MapObject {
                id: o.num("id")?.unwrap_or(0),
                name: o.attr("name").unwrap_or("").into(),
                x: o.num("x")?.unwrap_or(0.0),
                y: o.num("y")?.unwrap_or(0.0),
                width,
                height,
                rectangle: !shaped && o.attr("gid").is_none() && width > 0.0 && height > 0.0,
                properties: read_properties(o),
            })
        })
        .collect()
}
