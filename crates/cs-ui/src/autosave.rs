//! Autosave en el navegador (`localStorage`) y restauración de la última sesión.
//!
//! La codificación es pura (y se prueba en escritorio); el almacenamiento solo existe en web.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use cs_core::Document;
use serde::{Deserialize, Serialize};

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub const KEY: &str = "chimpsprite.autosave";
/// `localStorage` suele permitir ~5 MB por sitio.
pub const MAX_ENCODED: usize = 4 * 1024 * 1024;
pub const INTERVAL_SECS: f64 = 10.0;

#[derive(Serialize, Deserialize)]
struct Payload {
    name: String,
    chimp: String,
}

/// Documentos (nombre, contenido) listos para guardar; `Err` si no caben.
pub fn encode(docs: &[(&str, &Document)]) -> Result<String, String> {
    let payloads = docs
        .iter()
        .map(|(name, doc)| {
            let bytes = cs_io::chimp::write(doc).map_err(|e| e.to_string())?;
            Ok(Payload {
                name: name.to_string(),
                chimp: STANDARD.encode(bytes),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let text = serde_json::to_string(&payloads).map_err(|e| e.to_string())?;
    if text.len() > MAX_ENCODED {
        return Err(format!(
            "the document is too large for browser autosave ({} KB)",
            text.len() / 1024
        ));
    }
    Ok(text)
}

pub fn decode(text: &str) -> Result<Vec<Restore>, String> {
    let payloads: Vec<Payload> = serde_json::from_str(text).map_err(|e| e.to_string())?;
    payloads
        .into_iter()
        .map(|p| {
            let bytes = STANDARD.decode(p.chimp).map_err(|e| e.to_string())?;
            let (doc, _) = cs_io::chimp::read(&bytes).map_err(|e| e.to_string())?;
            Ok(Restore { name: p.name, doc })
        })
        .collect()
}

/// Sesión encontrada al iniciar, esperando a que el usuario decida.
pub struct Restore {
    pub name: String,
    pub doc: Document,
}

/// Cuándo guardar: cada `INTERVAL_SECS` mientras haya cambios nuevos.
#[derive(Default)]
pub struct Autosaver {
    saved_version: Option<u64>,
    last_time: f64,
    pub disabled_reason: Option<String>,
}

impl Autosaver {
    /// `true` si toca guardar ahora.
    pub fn due(&self, now: f64, doc_version: u64, dirty: bool) -> bool {
        dirty
            && self.disabled_reason.is_none()
            && self.saved_version != Some(doc_version)
            && now - self.last_time >= INTERVAL_SECS
    }

    pub fn mark_saved(&mut self, now: f64, doc_version: u64) {
        self.saved_version = Some(doc_version);
        self.last_time = now;
    }
}

#[cfg(target_arch = "wasm32")]
pub mod storage {
    fn local() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn load() -> Option<String> {
        local()?.get_item(super::KEY).ok()?
    }

    pub fn store(text: &str) -> Result<(), String> {
        local()
            .ok_or("browser storage is not available")?
            .set_item(super::KEY, text)
            .map_err(|_| "the browser refused to store the autosave (quota?)".to_string())
    }

    pub fn clear() {
        if let Some(s) = local() {
            let _ = s.remove_item(super::KEY);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod storage {
    pub fn load() -> Option<String> {
        None
    }

    pub fn store(_text: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn clear() {}
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};

    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let doc = Document::with_sprite("hero", Sprite::new(8, 8, ColorMode::Rgba));
        let other = Document::with_sprite("coin", Sprite::new(4, 4, ColorMode::Rgba));
        let text = encode(&[("game.chimp", &doc), ("Untitled 2", &other)]).unwrap();
        let back = decode(&text).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].name, "game.chimp");
        assert_eq!(back[0].doc.artboards[0].name, "hero");
        assert_eq!(back[1].doc.artboards[0].name, "coin");
        assert!(decode("{nope").is_err());
    }

    /// Autosave real de la versión anterior: "hero.chimp", un sprite de 4×4 con la simetría
    /// `{"mode": "Both", "center2": [3, 5]}` en su manifiesto.
    const OLD_AUTOSAVE: &str = r#"[{"name":"hero.chimp","chimp":"UEsDBBQAAAAIAAAAIQAnyrMt8wAAAJ8BAAAUAAAAYXJ0Ym9hcmRzLzEuYXNlcHJpdGW1jzFIQmEYRc+PD1ydgqZ+IqKt50MKSRCClmwQByMaVCzBNxhFBCI0RBC4tARRDrW05Sw0uDRGc5RDBbUGNVZw4xcice+MF77v3nNm4OnC4OFhMcAag1iMC4kR4z8YM/D5Hum3RoARIGr7lfzxAIxa+3sz/EQwbhOChamUoJKMCw7zc4LHzUnB/XlF8HZ9Kvj6mBGcvGYFYdcXzB7VBJnqtCCXmRAEqaQgnd4W+MU9wWrtWVBuSVC4fRHc9L4Fkkta7Y5gf/dAUA1DwUo+J9iZbwoug8BtWC4L7hpXbtvxoqC54cu5eNaZD8gQZalUX9+y8R9QSwMEFAAAAAgAAAAhACE8PA+jAAAAMgEAAA0AAABtYW5pZmVzdC5qc29uVY7BCoMwEETvfsWw59JqSy8e+xvFQ6orBhojm1AQ8d9LVk1pDpvZzczLLgVAvRdnItWgdrBuolMafliC9SPVqLQ3El/eSBeoxrMAgEUrQLY7XNqOxnGCDSxeWTqdfLBxA27xdMrjGSh31eREb9/KyT9fqrMJPImN/OOG2TmOMlOdFwLI+U6zDx+H7AWo5TGyXP+WAG4/B3DPutnVqneqTbF+AVBLAQIUABQAAAAIAAAAIQAnyrMt8wAAAJ8BAAAUAAAAAAAAAAAAAACkgQAAAABhcnRib2FyZHMvMS5hc2Vwcml0ZVBLAQIUABQAAAAIAAAAIQAhPDwPowAAADIBAAANAAAAAAAAAAAAAACkgSUBAABtYW5pZmVzdC5qc29uUEsFBgAAAAACAAIAfQAAAPMBAAAAAA=="}]"#;

    #[test]
    fn an_old_autosave_restores_the_symmetry_as_axes() {
        let back = decode(OLD_AUTOSAVE).unwrap();
        assert_eq!(back[0].name, "hero.chimp");
        let symmetry = back[0].doc.artboards[0].symmetry;
        assert_eq!(
            symmetry.axes,
            cs_core::SymmetryAxes {
                vertical: true,
                horizontal: true,
                ..cs_core::SymmetryAxes::NONE
            }
        );
        assert_eq!(symmetry.center2, Some((3, 5)));
    }

    #[test]
    fn too_large_documents_are_rejected() {
        // Una imagen con ruido no se comprime: 1200×1200 RGBA ≈ 5.7 MB.
        let mut sprite = Sprite::new(1200, 1200, ColorMode::Rgba);
        let pixels = (0..1200 * 1200u32)
            .map(|i| {
                let v = i.wrapping_mul(2654435761);
                (
                    (i % 1200) as i32,
                    (i / 1200) as i32,
                    cs_core::PixelValue::Rgba(cs_core::Rgba::new(
                        v as u8,
                        (v >> 8) as u8,
                        (v >> 16) as u8,
                        255,
                    )),
                )
            })
            .collect();
        cs_commands::Command::DrawPixels {
            layer: cs_core::LayerId(1),
            frame: 0,
            pixels,
        }
        .apply(&mut sprite)
        .unwrap();
        let doc = Document::with_sprite("noise", sprite);
        assert!(encode(&[("noise.chimp", &doc)]).is_err());
    }

    #[test]
    fn autosaver_timing() {
        let mut a = Autosaver::default();
        assert!(!a.due(0.0, 1, false), "nothing to save when clean");
        assert!(a.due(10.0, 1, true));
        a.mark_saved(10.0, 1);
        assert!(!a.due(30.0, 1, true), "same version");
        assert!(!a.due(15.0, 2, true), "too soon");
        assert!(a.due(20.0, 2, true));
    }
}
