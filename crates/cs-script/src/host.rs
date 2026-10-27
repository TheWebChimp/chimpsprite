//! Lo común a los dos lenguajes: la sesión del agente con el documento, los valores por defecto
//! de cada llamada y lo que imprime el script.

use cs_agent::Session;
use cs_core::Document;
use serde_json::{Value, json};

use crate::{Outcome, ScriptError};

/// Lo que está elegido en el editor: va solo en cada llamada que no lo dice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub artboard: u32,
    pub layer: u32,
    pub frame: usize,
}

pub(crate) struct Host {
    session: Session,
    doc: String,
    pub context: Context,
    pub output: Vec<String>,
}

impl Host {
    pub fn new(document: Document, context: Context) -> Self {
        let mut session = Session::new(None);
        let doc = session.insert_document(document);
        Self {
            session,
            doc,
            context,
            output: Vec::new(),
        }
    }

    /// Corre la operación `name`. A un objeto le agrega lo que no diga: el documento, el
    /// artboard, la capa y el frame actuales. Devuelve la respuesta como JSON (o su texto).
    pub fn call(&mut self, name: &str, params: Value) -> Result<Value, String> {
        let mut params = match params {
            Value::Null => json!({}),
            other => other,
        };
        if let Value::Object(map) = &mut params {
            let c = self.context;
            map.entry("doc").or_insert_with(|| json!(self.doc));
            map.entry("artboard").or_insert_with(|| json!(c.artboard));
            map.entry("layer").or_insert_with(|| json!(c.layer));
            map.entry("frame").or_insert_with(|| json!(c.frame));
        }
        let reply = self.session.call(name, params).map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(&reply.text).unwrap_or(Value::String(reply.text)))
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
