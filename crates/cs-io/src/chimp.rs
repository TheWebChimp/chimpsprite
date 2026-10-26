//! Formato nativo `.chimp`: zip con un `.aseprite` por artboard y un `manifest.json`.

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

use std::sync::Arc;

use cs_core::{
    Artboard, ArtboardId, Document, Guide, InstanceTrack, LayerKind, LayerMask, Note, Sprite,
    Symmetry,
};
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
    /// Las notas (post-its) del lienzo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notes: Vec<Note>,
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
    /// Datos de capa que `.aseprite` no guarda (máscaras y recortes), por índice de capa.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LayerExtra>,
}

#[derive(Serialize, Deserialize)]
struct LayerExtra {
    index: usize,
    #[serde(default)]
    clip: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mask: Option<MaskEntry>,
    /// Keys de una capa de instancia (en el `.aseprite` interno va horneada).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instance: Option<InstanceTrack>,
}

#[derive(Serialize, Deserialize)]
struct MaskEntry {
    position: [i32; 2],
    width: u32,
    height: u32,
    default: u8,
    enabled: bool,
    /// Bytes crudos (un valor por píxel, fila por fila) dentro del zip.
    file: String,
}

/// Extras de cada capa con máscara o recorte, y los bytes de sus máscaras.
fn layer_extras(artboard: &Artboard) -> Vec<(LayerExtra, Option<Arc<Vec<u8>>>)> {
    artboard
        .sprite
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.clip || l.mask.is_some() || l.instance.is_some())
        .map(|(index, l)| {
            let mask = l.mask.as_ref().map(|m| MaskEntry {
                position: [m.position.0, m.position.1],
                width: m.width,
                height: m.height,
                default: m.default,
                enabled: m.enabled,
                file: format!("masks/{}/{index}.bin", artboard.id.0),
            });
            let data = l.mask.as_ref().map(|m| m.data.clone());
            (
                LayerExtra {
                    index,
                    clip: l.clip,
                    mask,
                    instance: l.instance.clone(),
                },
                data,
            )
        })
        .collect()
}

fn apply_extras(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    sprite: &mut Sprite,
    extras: Vec<LayerExtra>,
) -> Result<()> {
    for extra in extras {
        let count = sprite.layers.len();
        let layer = sprite.layers.get_mut(extra.index).ok_or_else(|| {
            IoError::Corrupt(format!(
                "layer {} out of range ({count} layers)",
                extra.index
            ))
        })?;
        layer.clip = extra.clip;
        let id = layer.id;
        let instance_layer = extra.instance.is_some();
        if let Some(mut track) = extra.instance {
            // Un archivo editado a mano puede traer keys desordenadas o repetidas.
            track.normalize();
            layer.kind = LayerKind::Instance;
            layer.instance = Some(track);
        }
        if let Some(m) = extra.mask {
            let data = read_entry(zip, &m.file)?;
            if data.len() != (m.width as usize) * (m.height as usize) {
                return Err(IoError::Corrupt(format!("{}: wrong mask size", m.file)));
            }
            layer.mask = Some(LayerMask {
                position: (m.position[0], m.position[1]),
                width: m.width,
                height: m.height,
                data: Arc::new(data),
                default: m.default,
                enabled: m.enabled,
            });
        }
        if instance_layer {
            // Los cels horneados no se usan: la instancia se recompone desde su fuente.
            sprite.cels.retain(|(l, _), _| *l != id);
        }
    }
    // Quita del pool las imágenes que ya no usa ningún cel.
    let used: HashSet<cs_core::ImageId> = sprite.cels.values().map(|c| c.image).collect();
    sprite.images.retain(|id, _| used.contains(id));
    Ok(())
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
        // Las instancias van horneadas para que el `.aseprite` se vea bien fuera de ChimpSprite.
        let baked = cs_render::bake_layers(doc, &artboard.sprite, cs_render::Bake::KeepTilemaps);
        zip.write_all(&aseprite::write(baked.as_ref().unwrap_or(&artboard.sprite)))?;
        let mut layers = Vec::new();
        for (extra, data) in layer_extras(artboard) {
            if let (Some(mask), Some(data)) = (&extra.mask, data) {
                zip.start_file(mask.file.as_str(), options)
                    .map_err(zip_error)?;
                zip.write_all(&data)?;
            }
            layers.push(extra);
        }
        entries.push(Entry {
            id: artboard.id.0,
            name: artboard.name.clone(),
            position: [artboard.position.0, artboard.position.1],
            file,
            guides: artboard.guides.clone(),
            symmetry: artboard.symmetry,
            layers,
        });
    }
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        artboards: entries,
        notes: doc.notes.clone(),
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
        let (mut sprite, sub) = aseprite::read(&read_entry(&mut zip, &entry.file)?)?;
        apply_extras(&mut zip, &mut sprite, entry.layers)?;
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
    let mut doc = Document {
        artboards,
        notes: manifest.notes,
    };
    // Un archivo editado a mano podría tener instancias en ciclo: se quitan esas keys.
    for id in doc.artboards.iter().map(|a| a.id).collect::<Vec<_>>() {
        if doc.cycle_free(id) {
            continue;
        }
        let name = doc.artboard(id).expect("exists").name.clone();
        let sources = doc.instance_sources(id);
        let bad: Vec<_> = sources
            .into_iter()
            .filter(|&s| doc.would_cycle(id, s))
            .collect();
        let sprite = &mut doc.artboard_mut(id).expect("exists").sprite;
        for track in sprite.layers.iter_mut().filter_map(|l| l.instance.as_mut()) {
            for key in &mut track.keys {
                if key.source.is_some_and(|s| bad.contains(&s)) {
                    key.source = None;
                }
            }
        }
        report.warn(format!(
            "{name}: removed instances that contained the artboard itself"
        ));
    }
    Ok((doc, report))
}

#[cfg(test)]
mod tests {
    use cs_core::*;

    use super::*;

    #[test]
    fn notes_roundtrip() {
        let mut d = doc();
        d.notes.push(cs_core::Note {
            id: 1,
            position: (-20, 40),
            size: (120, 90),
            text: "Revisar la\nanimación ✨".into(),
            color: cs_core::NoteColor::Blue,
        });
        let (back, _) = read(&write(&d).unwrap()).unwrap();
        assert_eq!(back.notes, d.notes);
        // Sin notas, el manifest no las menciona.
        let bytes = write(&doc()).unwrap();
        let (back, _) = read(&bytes).unwrap();
        assert!(back.notes.is_empty());
    }

    #[test]
    fn masks_and_clipping_roundtrip() {
        let mut d = doc();
        let mut mask = LayerMask::filled(8, 8, 255);
        mask.data = std::sync::Arc::new((0..64).map(|i| (i * 4) as u8).collect());
        mask.position = (-1, 2);
        mask.enabled = false;
        let sprite = &mut d.artboards[0].sprite;
        sprite.layers[0].mask = Some(mask);
        let mut top = Layer::new_image(LayerId(2), "top");
        top.clip = true;
        sprite.layers.push(top);
        let (back, _) = read(&write(&d).unwrap()).unwrap();
        let layers = &back.artboards[0].sprite.layers;
        assert_eq!(layers[0].mask, d.artboards[0].sprite.layers[0].mask);
        assert!(layers[1].clip && !layers[0].clip);
        assert!(back.artboards[1].sprite.layers[0].mask.is_none());
    }

    /// Documento con el artboard 7 (el segundo de `doc()`) instanciado en el 1 (capa de instancia id 2).
    fn with_instance(timing: Timing) -> Document {
        let mut d = doc();
        let mut key = InstanceKey::new(0, ArtboardId(7), (1, 1));
        key.timing = timing;
        key.flip_h = true;
        let mut layer = Layer::new_image(LayerId(2), "inst");
        layer.kind = LayerKind::Instance;
        layer.instance = Some(InstanceTrack { keys: vec![key] });
        d.artboards[0].sprite.layers.push(layer);
        cs_commands::Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Index(1))],
        }
        .apply(&mut d.artboards[1].sprite)
        .unwrap();
        d
    }

    #[test]
    fn instances_roundtrip_in_both_timings() {
        for timing in [
            Timing::Graphic {
                first_frame: 0,
                play: GraphicPlay::Once,
            },
            Timing::MovieClip { first_frame: 0 },
        ] {
            let d = with_instance(timing);
            let (back, report) = read(&write(&d).unwrap()).unwrap();
            assert!(report.warnings.is_empty(), "{:?}", report.warnings);
            let layer = &back.artboards[0].sprite.layers[1];
            assert_eq!(layer.kind, LayerKind::Instance);
            assert_eq!(layer.instance, d.artboards[0].sprite.layers[1].instance);
            assert!(
                back.artboards[0].sprite.cel(LayerId(2), 0).is_none(),
                "baked cels dropped"
            );
        }
    }

    #[test]
    fn inner_aseprite_has_the_instance_baked() {
        let d = with_instance(Timing::MovieClip { first_frame: 0 });
        let bytes = write(&d).unwrap();
        let mut zip = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let (inner, _) =
            aseprite::read(&read_entry(&mut zip, "artboards/1.aseprite").unwrap()).unwrap();
        let expected =
            cs_render::render_frame(&cs_render::materialize(&d, &d.artboards[0].sprite, 0..1), 0);
        assert_eq!(cs_render::render_frame(&inner, 0), expected);
    }

    #[test]
    fn unsorted_instance_keys_load_normalized() {
        let mut d = with_instance(Timing::MovieClip { first_frame: 0 });
        let track = d.artboards[0].sprite.layers[1].instance.as_mut().unwrap();
        let first = track.keys[0];
        track.keys = vec![
            InstanceKey { frame: 3, ..first },
            first,
            InstanceKey {
                frame: 3,
                position: (4, 4),
                ..first
            },
        ];
        let (back, _) = read(&write(&d).unwrap()).unwrap();
        let keys = &back.artboards[0].sprite.layers[1]
            .instance
            .as_ref()
            .unwrap()
            .keys;
        let got: Vec<_> = keys.iter().map(|k| (k.frame, k.position)).collect();
        assert_eq!(got, vec![(0, (1, 1)), (3, (4, 4))]);
    }

    #[test]
    fn cyclic_instances_load_with_a_warning() {
        let mut d = with_instance(Timing::MovieClip { first_frame: 0 });
        // El artboard 7 instancia al 1: ciclo.
        let mut layer = Layer::new_image(LayerId(2), "back");
        layer.kind = LayerKind::Instance;
        layer.instance = Some(InstanceTrack {
            keys: vec![InstanceKey::new(0, ArtboardId(1), (0, 0))],
        });
        d.artboards[1].sprite.layers.push(layer);
        let (back, report) = read(&write(&d).unwrap()).unwrap();
        assert!(!report.warnings.is_empty());
        assert!(back.artboards.iter().all(|a| back.cycle_free(a.id)));
    }

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
                axes: cs_core::SymmetryAxes {
                    vertical: true,
                    horizontal: true,
                    ..cs_core::SymmetryAxes::NONE
                },
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

    #[test]
    fn an_old_chimp_with_the_symmetry_mode_opens_with_its_axes() {
        // Manifiesto como lo escribía la versión anterior: `mode` en vez de `axes`.
        let sprite = Sprite::new(4, 4, ColorMode::Rgba);
        let file = aseprite::write(&sprite);
        let modes = [
            ("None", [false, false, false, false]),
            ("Horizontal", [true, false, false, false]),
            ("Vertical", [false, true, false, false]),
            ("Both", [true, true, false, false]),
            ("DiagonalDown", [false, false, true, false]),
            ("DiagonalUp", [false, false, false, true]),
            ("All", [true, true, true, true]),
        ];
        for (mode, [v, h, down, up]) in modes {
            let manifest = format!(
                r#"{{"format":"chimp","version":1,"artboards":[{{"id":1,"name":"a","position":[0,0],"file":"artboards/1.aseprite","symmetry":{{"mode":"{mode}","center2":[3,5]}}}}]}}"#
            );
            let zip = zip_with(&[
                (MANIFEST, manifest.as_bytes()),
                ("artboards/1.aseprite", &file),
            ]);
            let (doc, _) = read(&zip).unwrap();
            let s = doc.artboards[0].symmetry;
            let axes = s.axes;
            assert_eq!(
                [
                    axes.vertical,
                    axes.horizontal,
                    axes.diagonal_down,
                    axes.diagonal_up
                ],
                [v, h, down, up],
                "{mode}"
            );
            assert_eq!(s.center2, Some((3, 5)), "{mode}");
            // Al guardar se escribe el formato nuevo.
            let again = write(&doc).unwrap();
            let mut archive = ZipArchive::new(Cursor::new(again.as_slice())).unwrap();
            let text = String::from_utf8(read_entry(&mut archive, MANIFEST).unwrap()).unwrap();
            assert!(text.contains("\"axes\""), "{text}");
            assert!(!text.contains("\"mode\""), "{text}");
        }
    }
}
