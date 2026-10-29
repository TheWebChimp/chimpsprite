//! Lo común a los dos lenguajes: la sesión del agente con el documento, los valores por defecto
//! de cada llamada, las funciones de apoyo (`pixels`, `selection`, `primary`…) y lo que imprime
//! el script.

use cs_agent::Session;
use cs_core::{ArtboardId, Document, LayerId, PixelValue, Rgba, Sprite};
use serde_json::{Value, json};

use crate::{Outcome, ScriptError};

/// Lo que está elegido en el editor: va solo en cada llamada que no lo dice, y lo leen
/// `primary()`, `secondary()` y `selection()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub artboard: u32,
    pub layer: u32,
    pub frame: usize,
    /// Los colores primario y secundario.
    pub primary: Rgba,
    pub secondary: Rgba,
    /// Los píxeles seleccionados (`None`: sin selección).
    pub selection: Option<Vec<(i32, i32)>>,
    /// Para `random()`: cambia en cada corrida.
    pub seed: u64,
    /// Para `export_folder()`: la carpeta del documento (o una de la app).
    pub export_folder: Option<String>,
}

impl Context {
    /// Sin colores, selección ni semilla (para los tests y el agente).
    pub fn new(artboard: u32, layer: u32, frame: usize) -> Self {
        Self {
            artboard,
            layer,
            frame,
            primary: Rgba::rgb(0, 0, 0),
            secondary: Rgba::rgb(255, 255, 255),
            selection: None,
            seed: 0x9e37_79b9_7f4a_7c15,
            export_folder: None,
        }
    }
}

/// Las funciones de apoyo, además de las operaciones: se llaman igual en los dos lenguajes.
pub(crate) const HELPERS: &[&str] = &[
    // `export` es palabra reservada en Rhai: el mismo `export` con otro nombre.
    "export_sprite",
    "pixels",
    "selection",
    "primary",
    "secondary",
    "width",
    "height",
    "export_folder",
];

pub(crate) struct Host {
    session: Session,
    doc: String,
    pub context: Context,
    pub output: Vec<String>,
    rng: u64,
}

/// `#rrggbbaa`, o `"transparent"`: lo que aceptan las operaciones como color.
pub(crate) fn hex(c: Rgba) -> String {
    if c.a == 0 {
        "transparent".into()
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    }
}

fn resolve(sprite: &Sprite, v: PixelValue) -> Rgba {
    match v {
        PixelValue::Rgba(c) => c,
        PixelValue::Index(i) => match sprite.color_mode {
            cs_core::ColorMode::Indexed { transparent_index } if i == transparent_index => {
                Rgba::TRANSPARENT
            }
            _ => sprite.palette.get(i).unwrap_or(Rgba::TRANSPARENT),
        },
    }
}

impl Host {
    pub fn new(document: Document, context: Context) -> Self {
        let mut session = Session::new(None);
        let doc = session.insert_document(document);
        let rng = context.seed | 1;
        Self {
            session,
            doc,
            context,
            output: Vec::new(),
            rng,
        }
    }

    /// Corre la operación `name`. A un objeto le agrega lo que no diga: el documento, el
    /// artboard, la capa y el frame actuales. Devuelve la respuesta como JSON (o su texto).
    pub fn call(&mut self, name: &str, params: Value) -> Result<Value, String> {
        if name == "export_sprite" {
            return self.call("export", params);
        }
        if HELPERS.contains(&name) {
            return self.helper(name, params);
        }
        let mut params = match params {
            Value::Null => json!({}),
            other => other,
        };
        if let Value::Object(map) = &mut params {
            let c = &self.context;
            map.entry("doc").or_insert_with(|| json!(self.doc));
            map.entry("artboard").or_insert_with(|| json!(c.artboard));
            map.entry("layer").or_insert_with(|| json!(c.layer));
            map.entry("frame").or_insert_with(|| json!(c.frame));
        }
        let reply = self.session.call(name, params).map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(&reply.text).unwrap_or(Value::String(reply.text)))
    }

    /// El sprite del artboard actual, como está ahora (con lo que ya hizo el script).
    fn sprite(&self) -> Result<&Sprite, String> {
        self.session
            .document(&self.doc)
            .and_then(|d| d.artboard(ArtboardId(self.context.artboard)))
            .map(|a| &a.sprite)
            .ok_or_else(|| "the artboard is gone".to_string())
    }

    fn helper(&mut self, name: &str, params: Value) -> Result<Value, String> {
        let number = |key: &str| params.get(key).and_then(Value::as_u64);
        match name {
            "primary" => Ok(json!(hex(self.context.primary))),
            "secondary" => Ok(json!(hex(self.context.secondary))),
            "selection" => Ok(match &self.context.selection {
                Some(points) => json!(points.iter().map(|&(x, y)| [x, y]).collect::<Vec<_>>()),
                None => Value::Null,
            }),
            "export_folder" => self
                .context
                .export_folder
                .clone()
                .map(Value::String)
                .ok_or_else(|| "export_folder: there is no folder to export to here".to_string()),
            "width" => Ok(json!(self.sprite()?.width)),
            "height" => Ok(json!(self.sprite()?.height)),
            "pixels" => {
                // La capa (por defecto la actual) en el frame (por defecto el actual), por filas.
                let layer = number("layer").map_or(self.context.layer, |l| l as u32);
                let frame = number("frame").map_or(self.context.frame, |f| f as usize);
                let sprite = self.sprite()?;
                if sprite.layer(LayerId(layer)).is_none() {
                    return Err(format!("pixels: no layer {layer}"));
                }
                let rows: Vec<Vec<String>> = (0..sprite.height as i32)
                    .map(|y| {
                        (0..sprite.width as i32)
                            .map(|x| {
                                hex(resolve(sprite, sprite.pixel(LayerId(layer), frame, x, y)))
                            })
                            .collect()
                    })
                    .collect();
                Ok(json!(rows))
            }
            _ => Err(format!("unknown helper '{name}'")),
        }
    }

    /// Un número al azar entre 0 y 1 (xorshift; la semilla cambia en cada corrida).
    pub fn random(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Un entero al azar entre `a` y `b` (los dos incluidos).
    pub fn random_between(&mut self, a: i64, b: i64) -> i64 {
        let (lo, hi) = (a.min(b), a.max(b));
        lo + (self.random() * (hi - lo + 1) as f64).floor() as i64
    }

    pub fn print(&mut self, line: String) {
        self.output.push(line);
    }

    pub fn finish(mut self) -> Result<Outcome, ScriptError> {
        let output = self.output.join("\n");
        match self.session.take_document(&self.doc) {
            Some(document) => Ok(Outcome { document, output }),
            None => Err(ScriptError {
                message: "the script closed the document".into(),
                output,
            }),
        }
    }

    pub fn fail(self, message: String) -> ScriptError {
        ScriptError {
            message,
            output: self.output.join("\n"),
        }
    }
}
