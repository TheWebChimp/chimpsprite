//! Abrir y guardar documentos según la extensión del archivo.

use std::path::Path;

use cs_core::Document;
use cs_io::{IoError, LoadReport, aseprite, chimp, import};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Chimp,
    Aseprite,
    Png,
}

pub fn kind_of(path: &Path) -> Option<FileKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "chimp" => Some(FileKind::Chimp),
        "aseprite" | "ase" => Some(FileKind::Aseprite),
        "png" => Some(FileKind::Png),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("unknown file type: {0}")]
    UnknownType(String),
    #[error("this document has {0} artboards and .aseprite files hold only one; save it as .chimp")]
    MultipleArtboards(usize),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Fs(#[from] std::io::Error),
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Artboard 1")
        .to_string()
}

/// Abre un `.chimp`, `.aseprite`/`.ase` o `.png` como documento.
pub fn open(path: &Path) -> Result<(Document, LoadReport), FileError> {
    let kind = kind_of(path).ok_or_else(|| FileError::UnknownType(path.display().to_string()))?;
    let bytes = std::fs::read(path)?;
    Ok(match kind {
        FileKind::Chimp => chimp::read(&bytes)?,
        FileKind::Aseprite => {
            let (sprite, report) = aseprite::read(&bytes)?;
            (Document::with_sprite(stem(path), sprite), report)
        }
        FileKind::Png => (
            Document::with_sprite(stem(path), import::import_png(&bytes)?),
            LoadReport::default(),
        ),
    })
}

/// Bytes a guardar para `path` según su extensión.
pub fn encode(doc: &Document, path: &Path) -> Result<Vec<u8>, FileError> {
    match kind_of(path) {
        Some(FileKind::Chimp) => Ok(chimp::write(doc)?),
        Some(FileKind::Aseprite) => match doc.artboards.as_slice() {
            [only] => Ok(aseprite::write(&only.sprite)),
            many => Err(FileError::MultipleArtboards(many.len())),
        },
        _ => Err(FileError::UnknownType(path.display().to_string())),
    }
}

/// Guarda de forma atómica: escribe un temporal y lo renombra, así un fallo no deja
/// el archivo a medias.
pub fn save(doc: &Document, path: &Path) -> Result<(), FileError> {
    let bytes = encode(doc, path)?;
    write_atomic(path, &bytes)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), FileError> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use cs_core::{Artboard, ArtboardId, ColorMode, Sprite};

    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "chimpsprite-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn kinds_by_extension() {
        assert_eq!(kind_of(Path::new("a/b.CHIMP")), Some(FileKind::Chimp));
        assert_eq!(kind_of(Path::new("x.ase")), Some(FileKind::Aseprite));
        assert_eq!(kind_of(Path::new("x.png")), Some(FileKind::Png));
        assert_eq!(kind_of(Path::new("x.txt")), None);
    }

    #[test]
    fn save_and_open_roundtrip() {
        let dir = temp_dir();
        let mut doc = Document::with_sprite("hero", Sprite::new(5, 4, ColorMode::Rgba));
        let ase = dir.join("hero.aseprite");
        save(&doc, &ase).unwrap();
        let (back, _) = open(&ase).unwrap();
        assert_eq!(back.artboards[0].name, "hero");
        assert_eq!(back.artboards[0].sprite.width, 5);

        doc.artboards.push(Artboard {
            id: ArtboardId(2),
            name: "coin".into(),
            position: (20, 0),
            sprite: Sprite::new(2, 2, ColorMode::Rgba),
        });
        assert!(matches!(
            save(&doc, &ase),
            Err(FileError::MultipleArtboards(2))
        ));
        let chimp_path = dir.join("game.chimp");
        save(&doc, &chimp_path).unwrap();
        let (back, _) = open(&chimp_path).unwrap();
        assert_eq!(back.artboards.len(), 2);
        assert!(!dir.join("game.chimp.tmp").exists());
        std::fs::remove_dir_all(dir).ok();
    }
}
