//! Codificar y decodificar documentos según la extensión del nombre de archivo (sin E/S).

use std::path::Path;

use crate::{IoError, LoadReport, aseprite, chimp, import};
use cs_core::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Chimp,
    Aseprite,
    Png,
}

pub fn kind_of(name: &str) -> Option<FileKind> {
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "chimp" => Some(FileKind::Chimp),
        "aseprite" | "ase" => Some(FileKind::Aseprite),
        "png" => Some(FileKind::Png),
        _ => None,
    }
}

/// ¿Guardar en este formato aplica (pierde como capas editables) máscaras o recortes?
pub fn bakes_masks(doc: &Document, file_name: &str) -> bool {
    kind_of(file_name) == Some(FileKind::Aseprite)
        && doc
            .artboards
            .iter()
            .any(|a| a.sprite.layers.iter().any(|l| l.mask.is_some() || l.clip))
}

/// Nombre sin extensión.
pub fn stem(name: &str) -> &str {
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
}

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("unknown file type: {0}")]
    UnknownType(String),
    #[error("this document has {0} artboards and .aseprite files hold only one; save it as .chimp")]
    MultipleArtboards(usize),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error("{0}")]
    Platform(String),
}

/// Abre los bytes de un `.chimp`, `.aseprite`/`.ase` o `.png` como documento.
pub fn decode(name: &str, bytes: &[u8]) -> Result<(Document, LoadReport), FileError> {
    let kind = kind_of(name).ok_or_else(|| FileError::UnknownType(name.to_string()))?;
    Ok(match kind {
        FileKind::Chimp => chimp::read(bytes)?,
        FileKind::Aseprite => {
            let (sprite, report) = aseprite::read(bytes)?;
            (Document::with_sprite(stem(name), sprite), report)
        }
        FileKind::Png => (
            Document::with_sprite(stem(name), import::import_png(bytes)?),
            LoadReport::default(),
        ),
    })
}

/// Bytes a guardar con el nombre `file_name`, según su extensión.
pub fn encode(doc: &Document, file_name: &str) -> Result<Vec<u8>, FileError> {
    match kind_of(file_name) {
        Some(FileKind::Chimp) => Ok(chimp::write(doc)?),
        Some(FileKind::Aseprite) => match doc.artboards.as_slice() {
            // `.aseprite` no tiene máscaras ni recortes: se aplican a los píxeles.
            [only] => Ok(aseprite::write(
                cs_render::bake_masks(&only.sprite)
                    .as_ref()
                    .unwrap_or(&only.sprite),
            )),
            many => Err(FileError::MultipleArtboards(many.len())),
        },
        _ => Err(FileError::UnknownType(file_name.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{Artboard, ArtboardId, ColorMode, Sprite};

    use super::*;

    #[test]
    fn kinds_by_extension() {
        assert_eq!(kind_of("a/b.CHIMP"), Some(FileKind::Chimp));
        assert_eq!(kind_of("x.ase"), Some(FileKind::Aseprite));
        assert_eq!(kind_of("x.png"), Some(FileKind::Png));
        assert_eq!(kind_of("x.txt"), None);
        assert_eq!(stem("dir/hero.aseprite"), "hero");
    }

    #[test]
    fn encode_and_decode_roundtrip() {
        let mut doc = Document::with_sprite("hero", Sprite::new(5, 4, ColorMode::Rgba));
        let bytes = encode(&doc, "hero.aseprite").unwrap();
        let (back, _) = decode("hero.aseprite", &bytes).unwrap();
        assert_eq!(back.artboards[0].name, "hero");
        assert_eq!(back.artboards[0].sprite.width, 5);

        doc.artboards.push(Artboard {
            guides: Vec::new(),
            symmetry: Default::default(),
            id: ArtboardId(2),
            name: "coin".into(),
            position: (20, 0),
            sprite: Sprite::new(2, 2, ColorMode::Rgba),
        });
        assert!(matches!(
            encode(&doc, "game.aseprite"),
            Err(FileError::MultipleArtboards(2))
        ));
        let (back, _) = decode("game.chimp", &encode(&doc, "game.chimp").unwrap()).unwrap();
        assert_eq!(back.artboards.len(), 2);
        assert!(matches!(
            decode("x.txt", b""),
            Err(FileError::UnknownType(_))
        ));
    }
}
