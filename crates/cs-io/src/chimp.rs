//! Formato nativo `.chimp`: zip con un `.aseprite` por artboard y un `manifest.json`.

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

use std::sync::Arc;

use cs_core::{
    Artboard, ArtboardId, Document, FontInfo, Guide, InstanceTrack, LayerKind, LayerMask, Model3D,
    Note, ObjectId, SceneObject, Sprite, Symmetry,
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
    /// Símbolo de la biblioteca (no se ve en el lienzo).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    library: bool,
    /// Datos de capa que `.aseprite` no guarda (máscaras y recortes), por índice de capa.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LayerExtra>,
    /// La fuente del artboard (si es una fuente pixel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font: Option<FontInfo>,
    /// El modelo 3D del artboard (si es un modelo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<Model3D>,
}

#[derive(Serialize, Deserialize)]
struct LayerExtra {
    index: usize,
    #[serde(default)]
    clip: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mask: Option<MaskEntry>,
    /// Objetos de una capa de objetos (en el `.aseprite` interno va horneada). `Some` (aunque
    /// esté vacía) marca la capa como de objetos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    objects: Option<Vec<SceneObject>>,
    /// Archivos viejos: las keys de una capa de instancia (una instancia por capa). Solo se lee;
    /// al cargar pasa a ser el único objeto de la capa.
    #[serde(default, skip_serializing)]
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
        .filter(|(_, l)| l.clip || l.mask.is_some() || l.kind == LayerKind::Objects)
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
                    objects: (l.kind == LayerKind::Objects).then(|| l.objects.clone()),
                    instance: None,
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
        let objects = match (extra.objects, extra.instance) {
            (Some(objects), _) => Some(objects),
            // Archivo viejo: la instancia pasa a ser el único objeto de la capa.
            (None, Some(track)) => Some(vec![SceneObject {
                id: ObjectId(1),
                name: layer.name.clone(),
                track,
            }]),
            (None, None) => None,
        };
        let object_layer = objects.is_some();
        if let Some(mut objects) = objects {
            // Un archivo editado a mano puede traer keys desordenadas o repetidas, o ids
            // repetidos (se quedan con el primero).
            let mut seen = HashSet::new();
            objects.retain(|o| seen.insert(o.id));
            for object in &mut objects {
                object.track.normalize();
            }
            layer.kind = LayerKind::Objects;
            layer.objects = objects;
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
        if object_layer {
            // Los cels horneados no se usan: los objetos se recomponen desde sus fuentes.
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
        // Los objetos van horneados para que el `.aseprite` se vea bien fuera de ChimpSprite.
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
            library: artboard.in_library,
            layers,
            font: artboard.font.clone(),
            model: artboard.model.clone(),
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
        // Una fuente editada a mano: sus glifos se ordenan y se emparejan con los frames.
        let mut font = entry.font;
        if let Some(font) = &mut font
            && cs_core::normalize_font(font, &mut sprite)
        {
            report.warn(format!(
                "{}: the font's glyph list did not match its frames and was fixed",
                entry.name
            ));
        }
        // Un modelo editado a mano que no sirve (caras rotas, varias capas o frames) se quita.
        // La tabla de shading se ordena y limpia (sin aviso: no cambia cómo se ve).
        let mut model = entry.model;
        if let Some(m) = &mut model {
            m.shading.normalize();
        }
        let model = model.filter(|m| match m.check(&sprite) {
            Ok(()) => true,
            Err(e) => {
                report.warn(format!("{}: the 3D model was dropped: {e}", entry.name));
                false
            }
        });
        artboards.push(Artboard {
            model,
            in_library: entry.library,
            font,
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
        for object in sprite.layers.iter_mut().flat_map(|l| l.objects.iter_mut()) {
            for key in &mut object.track.keys {
                if key.source.is_some_and(|s| bad.contains(&s)) {
                    key.source = None;
                }
            }
        }
        report.warn(format!(
            "{name}: removed instances that contained the artboard itself"
        ));
    }
    // Un archivo con solo modelos o fuentes se abre con un sprite vacío en el lienzo.
    doc.ensure_canvas_artboard();
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

    fn object(track: InstanceTrack) -> SceneObject {
        SceneObject {
            id: ObjectId(1),
            name: "inst".into(),
            track,
        }
    }

    /// Documento con el artboard 7 (el segundo de `doc()`) instanciado en el 1 (capa de objetos
    /// id 2, con un objeto).
    fn with_instance(timing: Timing) -> Document {
        let mut d = doc();
        let mut key = InstanceKey::new(0, ArtboardId(7), (1, 1));
        key.timing = timing;
        key.flip_h = true;
        let mut layer = Layer::new_image(LayerId(2), "inst");
        layer.kind = LayerKind::Objects;
        layer.objects = vec![object(InstanceTrack { keys: vec![key] })];
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
            assert_eq!(layer.kind, LayerKind::Objects);
            assert_eq!(layer.objects, d.artboards[0].sprite.layers[1].objects);
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
        let track = &mut d.artboards[0].sprite.layers[1].objects[0].track;
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
        let keys = &back.artboards[0].sprite.layers[1].objects[0].track.keys;
        let got: Vec<_> = keys.iter().map(|k| (k.frame, k.position)).collect();
        assert_eq!(got, vec![(0, (1, 1)), (3, (4, 4))]);
    }

    #[test]
    fn cyclic_instances_load_with_a_warning() {
        let mut d = with_instance(Timing::MovieClip { first_frame: 0 });
        // El artboard 7 instancia al 1: ciclo.
        let mut layer = Layer::new_image(LayerId(2), "back");
        layer.kind = LayerKind::Objects;
        layer.objects = vec![object(InstanceTrack {
            keys: vec![InstanceKey::new(0, ArtboardId(1), (0, 0))],
        })];
        d.artboards[1].sprite.layers.push(layer);
        let (back, report) = read(&write(&d).unwrap()).unwrap();
        assert!(!report.warnings.is_empty());
        assert!(back.artboards.iter().all(|a| back.cycle_free(a.id)));
    }

    #[test]
    fn several_objects_and_empty_object_layers_roundtrip() {
        let mut d = with_instance(Timing::MovieClip { first_frame: 0 });
        let sprite = &mut d.artboards[0].sprite;
        let mut second = sprite.layers[1].objects[0].clone();
        second.id = ObjectId(4);
        second.name = "other".into();
        second.track.keys[0].position = (3, 0);
        sprite.layers[1].objects.push(second);
        let mut empty = Layer::new_image(LayerId(3), "empty");
        empty.kind = LayerKind::Objects;
        sprite.layers.push(empty);
        let (back, _) = read(&write(&d).unwrap()).unwrap();
        let layers = &back.artboards[0].sprite.layers;
        assert_eq!(layers[1].objects, d.artboards[0].sprite.layers[1].objects);
        assert_eq!(layers[2].kind, LayerKind::Objects);
        assert!(layers[2].objects.is_empty());
    }

    /// Reescribe el manifest de un `.chimp` con `edit`, dejando igual las demás entradas.
    fn with_manifest(bytes: &[u8], edit: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
        let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut out = ZipWriter::new(Cursor::new(Vec::new()));
        let names: Vec<String> = zip.file_names().map(|n| n.unwrap().to_string()).collect();
        for name in names {
            let mut data = read_entry(&mut zip, &name).unwrap();
            if name == MANIFEST {
                let mut manifest: serde_json::Value = serde_json::from_slice(&data).unwrap();
                edit(&mut manifest);
                data = serde_json::to_vec(&manifest).unwrap();
            }
            out.start_file(name.as_str(), SimpleFileOptions::default())
                .unwrap();
            out.write_all(&data).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    #[test]
    fn old_instance_layers_load_as_object_layers_with_one_object() {
        let d = with_instance(Timing::MovieClip { first_frame: 0 });
        // El manifest de antes: la capa 1 del artboard 1 con `instance` en lugar de `objects`.
        let old = with_manifest(&write(&d).unwrap(), |m| {
            m["artboards"][0]["layers"] = serde_json::json!([{
                "index": 1,
                "clip": false,
                "instance": {"keys": [
                    {"frame": 2, "source": 7, "position": [5, 6], "flip_h": false,
                     "flip_v": false, "opacity": 255,
                     "timing": {"Graphic": {"first_frame": 0, "play": "Loop"}}},
                    {"frame": 0, "source": 7, "position": [1, 1], "flip_h": true,
                     "flip_v": false, "opacity": 90,
                     "timing": {"MovieClip": {"first_frame": 0}}}
                ]}
            }]);
        });
        let (back, report) = read(&old).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let layer = &back.artboards[0].sprite.layers[1];
        assert_eq!(layer.kind, LayerKind::Objects);
        assert_eq!(layer.objects.len(), 1);
        let object = &layer.objects[0];
        assert_eq!((object.id, object.name.as_str()), (ObjectId(1), "inst"));
        let keys: Vec<_> = object
            .track
            .keys
            .iter()
            .map(|k| (k.frame, k.position, k.opacity))
            .collect();
        assert_eq!(keys, vec![(0, (1, 1), 90), (2, (5, 6), 255)], "normalized");
        assert!(
            back.artboards[0].sprite.cel(LayerId(2), 0).is_none(),
            "baked cels dropped"
        );
        // Al volver a guardar se escribe `objects`.
        let bytes = write(&back).unwrap();
        let mut zip = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&read_entry(&mut zip, MANIFEST).unwrap()).unwrap();
        let extra = &manifest["artboards"][0]["layers"][0];
        assert!(extra.get("instance").is_none());
        assert_eq!(extra["objects"][0]["id"], 1);
    }

    #[test]
    fn a_font_artboard_roundtrips_with_its_glyphs() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        font.glyphs[0].advance = Some(3);
        let mut sprite = font_sprite((8, 8), FontMode::OneBit, font.glyphs.len());
        let layer = sprite.layers[0].id;
        let mut image = Image::new(8, 8, sprite.color_mode);
        image.set(2, 3, ONE_BIT_INK).unwrap();
        let id = sprite.next_image_id();
        sprite.images.insert(id, std::sync::Arc::new(image));
        sprite.cels.insert(
            (layer, 5),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
        doc.artboards.push(Artboard {
            model: None,
            id: ArtboardId(2),
            name: "Mine".into(),
            position: (20, 0),
            sprite,
            guides: Vec::new(),
            symmetry: Default::default(),
            in_library: false,
            font: Some(font.clone()),
        });
        let bytes = write(&doc).unwrap();
        let (back, _) = read(&bytes).unwrap();
        let a = back.artboard(ArtboardId(2)).unwrap();
        assert_eq!(a.font.as_ref(), Some(&font));
        assert_eq!(a.sprite.frames.len(), font.glyphs.len());
        assert_eq!(a.sprite.pixel(a.sprite.layers[0].id, 5, 2, 3), ONE_BIT_INK);
        assert!(back.artboard(ArtboardId(1)).unwrap().font.is_none());
    }

    #[test]
    fn a_model_artboard_roundtrips_with_its_model() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let (model, texture, cmd) =
            cs_commands::model::new_model_artboard(&doc, "Cube", (24, 24), (10, 0));
        cmd.apply(&mut doc).unwrap();
        let m = doc.artboard_mut(model).unwrap().model.as_mut().unwrap();
        m.outline = Some(Rgba::rgb(1, 2, 3));
        m.shading
            .set(Rgba::rgb(200, 100, 50), 2, Some(Rgba::rgb(9, 8, 7)));
        let (back, report) = read(&write(&doc).unwrap()).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let a = back.artboard(model).unwrap();
        assert_eq!(a.model, doc.artboard(model).unwrap().model);
        assert_eq!(
            a.model
                .as_ref()
                .unwrap()
                .shading
                .get(Rgba::rgb(200, 100, 50), 2),
            Some(Rgba::rgb(9, 8, 7))
        );
        assert_eq!(a.model.as_ref().unwrap().texture, Some(texture));
        assert!(back.artboard(texture).unwrap().model.is_none());
        assert!(back.artboard(ArtboardId(1)).unwrap().model.is_none());
        // El `.aseprite` interno del modelo trae el render.
        let shown = cs_render::render_frame(&cs_render::materialize(&back, &a.sprite, 0..1), 0);
        let expected = cs_render::render_frame(
            &cs_render::materialize(&doc, &doc.artboard(model).unwrap().sprite, 0..1),
            0,
        );
        assert_eq!(shown, expected);
        // Sin materializar: los pixeles guardados ya son el render (lo que ve otro programa).
        let stored = cs_render::render_frame(&a.sprite, 0);
        assert!(stored.iter().any(|c| c.a > 0), "the stored render is empty");
        assert_eq!(stored, expected);
    }

    #[test]
    fn a_model_opens_off_the_canvas_and_its_texture_on_it() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let (model, texture, cmd) =
            cs_commands::model::new_model_artboard(&doc, "Cube", (24, 24), (10, 0));
        cmd.apply(&mut doc).unwrap();
        let (back, _) = read(&write(&doc).unwrap()).unwrap();
        assert!(!back.artboard(model).unwrap().on_canvas());
        assert!(back.artboard(texture).unwrap().on_canvas());
    }

    #[test]
    fn a_font_only_chimp_opens_with_an_extra_sprite_on_the_canvas() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let font = FontInfo::new("Mine", 8, FontMode::OneBit);
        let a = &mut doc.artboards[0];
        a.sprite = font_sprite((8, 8), FontMode::OneBit, font.glyphs.len());
        a.font = Some(font);
        let (back, _) = read(&write(&doc).unwrap()).unwrap();
        assert_eq!(back.artboards.len(), 2);
        assert_eq!(back.canvas_count(), 1);
        let sprite = back.canvas_artboards().next().unwrap();
        assert_eq!(sprite.name, "Sprite");
        assert!(back.artboard(ArtboardId(1)).unwrap().font.is_some());
    }

    #[test]
    fn a_hand_edited_shading_table_loads_normalized() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let (model, _, cmd) =
            cs_commands::model::new_model_artboard(&doc, "Cube", (24, 24), (10, 0));
        cmd.apply(&mut doc).unwrap();
        let c = |v| Rgba::rgb(v, v, v);
        let e = |color, a, b| ShadeEntry {
            color,
            steps: [a, b],
        };
        // Desordenada, con un repetido y una vacía (lo que dejaría un archivo editado a mano).
        doc.artboard_mut(model)
            .unwrap()
            .model
            .as_mut()
            .unwrap()
            .shading = ShadingTable {
            entries: vec![
                e(c(200), Some(c(1)), None),
                e(c(50), None, None),
                e(c(100), None, Some(c(2))),
                e(c(200), None, Some(c(3))),
            ],
        };
        let (back, _) = read(&write(&doc).unwrap()).unwrap();
        let shading = &back
            .artboard(model)
            .unwrap()
            .model
            .as_ref()
            .unwrap()
            .shading;
        assert_eq!(
            shading.entries,
            vec![e(c(100), None, Some(c(2))), e(c(200), None, Some(c(3)))]
        );
    }

    #[test]
    fn a_chimp_without_model_opens_with_none() {
        let bytes = write(&doc()).unwrap();
        let mut zip = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let manifest = String::from_utf8(read_entry(&mut zip, MANIFEST).unwrap()).unwrap();
        assert!(
            !manifest.contains("\"model\""),
            "no model key when there is none"
        );
        let (back, _) = read(&bytes).unwrap();
        assert!(back.artboards.iter().all(|a| a.model.is_none()));
    }

    #[test]
    fn a_bad_model_is_dropped_with_a_warning() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let (model, _, cmd) =
            cs_commands::model::new_model_artboard(&doc, "Cube", (24, 24), (10, 0));
        cmd.apply(&mut doc).unwrap();
        let m = doc.artboard_mut(model).unwrap().model.as_mut().unwrap();
        m.objects[0].mesh.faces[0].verts[0] = 999;
        let (back, report) = read(&write(&doc).unwrap()).unwrap();
        assert!(back.artboard(model).unwrap().model.is_none());
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
    }

    #[test]
    fn a_font_with_a_bad_glyph_list_loads_fixed_with_a_warning() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let font = FontInfo::new("Mine", 8, FontMode::OneBit);
        let mut sprite = font_sprite((8, 8), FontMode::OneBit, 4);
        let layer = sprite.layers[0].id;
        let mut image = Image::new(8, 8, sprite.color_mode);
        image.set(1, 1, ONE_BIT_INK).unwrap();
        let id = sprite.next_image_id();
        sprite.images.insert(id, std::sync::Arc::new(image));
        sprite.cels.insert(
            (layer, 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
        doc.artboards.push(Artboard {
            model: None,
            id: ArtboardId(2),
            name: "Mine".into(),
            position: (20, 0),
            sprite,
            guides: Vec::new(),
            symmetry: Default::default(),
            in_library: false,
            font: Some(font),
        });
        // A mano: 'B' (frame 0, pintado), 'A', 'A' repetida; el frame 3 sin glifo.
        let bad = with_manifest(&write(&doc).unwrap(), |m| {
            m["artboards"][1]["font"]["glyphs"] = serde_json::json!([
                {"ch": "B", "advance": 2}, {"ch": "A"}, {"ch": "A"}
            ]);
        });
        let (back, report) = read(&bad).unwrap();
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(report.warnings[0].contains("Mine"), "{:?}", report.warnings);
        let a = back.artboard(ArtboardId(2)).unwrap();
        let font = a.font.as_ref().unwrap();
        assert_eq!(font.check(&a.sprite), Ok(()));
        let chars: Vec<char> = font.glyphs.iter().map(|g| g.ch).collect();
        assert_eq!(chars, vec!['A', 'B', '\u{E000}', '\u{E001}']);
        let b = font.index_of('B').unwrap();
        assert_eq!(a.sprite.pixel(layer, b, 1, 1), ONE_BIT_INK);
        assert_eq!(font.glyphs[b].advance, Some(2));
        // Más glifos que frames: se cortan.
        let long = with_manifest(&write(&doc).unwrap(), |_| {});
        let (back, report) = read(&long).unwrap();
        assert_eq!(report.warnings.len(), 1, "94 glyphs for 4 frames");
        let a = back.artboard(ArtboardId(2)).unwrap();
        assert_eq!(a.font.as_ref().unwrap().check(&a.sprite), Ok(()));
    }

    #[test]
    fn library_roundtrip() {
        let mut d = doc();
        d.artboards[1].in_library = true;
        // El host usa el símbolo de la biblioteca en una capa de instancia.
        let symbol = d.artboards[1].id;
        let host = &mut d.artboards[0].sprite;
        let add = cs_commands::ops::new_object_layer(host, None, 0, symbol, "Banana", (1, 2));
        add.apply(host).unwrap();
        let instance = host.layers.last().unwrap().id;
        let (back, _) = read(&write(&d).unwrap()).unwrap();
        assert!(back.artboards[1].in_library);
        assert!(!back.artboards[0].in_library);
        assert_eq!(back, d);
        let layer = back.artboards[0].sprite.layer(instance).unwrap();
        assert_eq!(layer.kind, LayerKind::Objects);
        let key = &layer.objects[0].track.keys[0];
        assert_eq!((key.source, key.position), (Some(symbol), (1, 2)));
        assert!(back.artboard(symbol).is_some_and(|a| a.in_library));
        assert_eq!(back.instance_sources(back.artboards[0].id), vec![symbol]);
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
            model: None,
            in_library: false,
            font: None,
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
