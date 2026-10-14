//! Formato nativo `.chimp`: zip con un `.aseprite` por artboard y un `manifest.json`.

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

use cs_core::{Artboard, ArtboardId, Document, Guide, Symmetry};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{IoError, LoadReport, Result, aseprite};

const FORMAT: &str = "chimp";
const VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";
/// Tope por entrada al descomprimir (protege contra zips maliciosos).
const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    artboards: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    id: u32,
    name: String,
    position: [i32; 2],
    file: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    guides: Vec<Guide>,
    #[serde(default)]
    symmetry: Symmetry,
}

fn zip_error(e: impl std::fmt::Display) -> IoError {
    IoError::Zip(e.to_string())
}

pub fn write(doc: &Document) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut entries = Vec::new();
    for artboard in &doc.artboards {
        let file = format!("artboards/{}.aseprite", artboard.id.0);
        zip.start_file(file.as_str(), options).map_err(zip_error)?;
        zip.write_all(&aseprite::write(&artboard.sprite))?;
        entries.push(Entry {
            id: artboard.id.0,
            name: artboard.name.clone(),
            position: [artboard.position.0, artboard.position.1],
            file,
            guides: artboard.guides.clone(),
            symmetry: artboard.symmetry,
        });
    }
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        artboards: entries,
    };
    zip.start_file(MANIFEST, options).map_err(zip_error)?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    Ok(zip.finish().map_err(zip_error)?.into_inner())
}

fn read_entry(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>> {
    let file = zip
        .by_name(name)
        .map_err(|e| IoError::Zip(format!("{name}: {e}")))?;
    let mut out = Vec::new();
    file.take(MAX_ENTRY_BYTES).read_to_end(&mut out)?;
    Ok(out)
}

pub fn read(data: &[u8]) -> Result<(Document, LoadReport)> {
    let mut zip = ZipArchive::new(Cursor::new(data)).map_err(zip_error)?;
    let manifest: Manifest = serde_json::from_slice(&read_entry(&mut zip, MANIFEST)?)?;
    if manifest.format != FORMAT {
        return Err(IoError::Unsupported(format!(
            "unknown format \"{}\"",
            manifest.format
        )));
    }
    if manifest.version > VERSION {
        return Err(IoError::Unsupported(format!(
            "file version {} is newer than this ChimpSprite (supports {VERSION})",
            manifest.version
        )));
    }
    if manifest.artboards.is_empty() {
        return Err(IoError::Corrupt("the document has no artboards".into()));
    }
    let mut report = LoadReport::default();
    let mut seen = HashSet::new();
    let mut artboards = Vec::new();
    for entry in manifest.artboards {
        if !seen.insert(entry.id) {
            return Err(IoError::Corrupt(format!(
                "duplicate artboard id {}",
                entry.id
            )));
        }
        let (sprite, sub) = aseprite::read(&read_entry(&mut zip, &entry.file)?)?;
        for w in sub.warnings {
            report.warn(format!("{}: {w}", entry.name));
        }
        artboards.push(Artboard {
            guides: entry.guides,
            symmetry: entry.symmetry,
            id: ArtboardId(entry.id),
            name: entry.name,
            position: (entry.position[0], entry.position[1]),
            sprite,
        });
    }
    Ok((Document { artboards }, report))
}

#[cfg(test)]
mod tests {
    use cs_core::*;

    use super::*;

    fn doc() -> Document {
        let mut chimp = Sprite::new(8, 8, ColorMode::Rgba);
        cs_commands::Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(3, 3, PixelValue::Rgba(Rgba::rgb(1, 2, 3)))],
        }
        .apply(&mut chimp)
        .unwrap();
        let mut d = Document::with_sprite("chimp", chimp);
        d.artboards.push(Artboard {
            guides: vec![Guide {
                axis: cs_core::Axis::Vertical,
                position: 2,
            }],
            symmetry: cs_core::Symmetry {
                mode: cs_core::SymmetryMode::Both,
                center2: Some((5, 3)),
            },
            id: ArtboardId(7),
            name: "banana 🍌".into(),
            position: (-40, 12),
            sprite: Sprite::new(
                4,
                2,
                ColorMode::Indexed {
                    transparent_index: 0,
                },
            ),
        });
        d
    }

    #[test]
    fn roundtrip_keeps_artboards() {
        let d = doc();
        let (back, report) = read(&write(&d).unwrap()).unwrap();
        assert!(report.warnings.is_empty());
        assert_eq!(back.artboards.len(), 2);
        for (a, b) in d.artboards.iter().zip(&back.artboards) {
            assert_eq!((a.id, &a.name, a.position), (b.id, &b.name, b.position));
            assert_eq!(a.guides, b.guides);
            assert_eq!(a.symmetry, b.symmetry);
            assert_eq!(
                cs_render::render_frame(&a.sprite, 0),
                cs_render::render_frame(&b.sprite, 0)
            );
        }
    }

    fn zip_with(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in files {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn invalid_files_are_errors() {
        assert!(read(b"not a zip").is_err());
        assert!(
            read(&zip_with(&[("other.txt", b"x")])).is_err(),
            "no manifest"
        );
        let missing = br#"{"format":"chimp","version":1,"artboards":[{"id":1,"name":"a","position":[0,0],"file":"artboards/1.aseprite"}]}"#;
        assert!(
            read(&zip_with(&[(MANIFEST, missing)])).is_err(),
            "missing artboard"
        );
        let future = br#"{"format":"chimp","version":99,"artboards":[]}"#;
        assert!(matches!(
            read(&zip_with(&[(MANIFEST, future)])),
            Err(IoError::Unsupported(_))
        ));
    }
}
