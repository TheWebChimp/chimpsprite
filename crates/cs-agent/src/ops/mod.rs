//! Herramientas agrupadas por tema; cada una es un método de `Session`.

mod color;
mod document;
mod draw;
mod structure;
mod transform;
mod view;

#[cfg(test)]
pub(crate) mod test_util {
    use crate::params::NewDocument;
    use crate::{Reply, Session};

    /// Sesión con un documento RGBA vacío de `w`×`h` ("d1").
    pub fn session(w: u32, h: u32) -> Session {
        let mut s = Session::new(None);
        s.new_document(NewDocument {
            width: w,
            height: h,
            color_mode: Default::default(),
            name: None,
        })
        .unwrap();
        s
    }

    /// Parámetros desde JSON (como los manda el agente).
    pub fn p<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
        serde_json::from_value(v).expect("valid params")
    }

    pub fn json(r: &Reply) -> serde_json::Value {
        serde_json::from_str(&r.text).expect("json reply")
    }
}
