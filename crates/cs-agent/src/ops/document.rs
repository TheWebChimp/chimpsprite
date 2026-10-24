//! Documentos (crear, abrir, guardar, cerrar), `describe`, historial y comandos crudos.

use cs_commands::{DocCommand, History};
use cs_core::{AnimDirection, BlendMode, ColorMode, Document, LayerKind, Palette, Rgba, Sprite};
use cs_io::files::{self, FileKind};
use serde_json::{Value, json};

use crate::error::{Result, invalid};
use crate::params::{ApplyCommands, ColorModeArg, DocOnly, NewDocument, Open, Save};
use crate::session::OpenDoc;
use crate::{Reply, Session};

/// Lado máximo de un lienzo creado por el agente.
pub(crate) const MAX_SIDE: u32 = 4096;

pub(crate) fn check_size(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(invalid(format!(
            "invalid size {width}×{height}; each side must be 1-{MAX_SIDE}"
        )));
    }
    Ok(())
}

/// Sprite vacío; en indexado la paleta empieza con el transparente (índice 0).
pub(crate) fn blank_sprite(width: u32, height: u32, mode: ColorModeArg) -> Sprite {
    match mode {
        ColorModeArg::Rgba => Sprite::new(width, height, ColorMode::Rgba),
        ColorModeArg::Indexed => {
            let mut s = Sprite::new(
                width,
                height,
                ColorMode::Indexed {
                    transparent_index: 0,
                },
            );
            let mut colors = vec![Rgba::TRANSPARENT];
            colors.extend_from_slice(Palette::db32().colors());
            s.palette = Palette::new(colors);
            s
        }
    }
}

pub(crate) fn blend_name(b: BlendMode) -> String {
    match b {
        BlendMode::Normal => "normal".into(),
        BlendMode::Multiply => "multiply".into(),
        BlendMode::Screen => "screen".into(),
        BlendMode::Overlay => "overlay".into(),
        BlendMode::Darken => "darken".into(),
        BlendMode::Lighten => "lighten".into(),
        BlendMode::Addition => "addition".into(),
        BlendMode::Subtract => "subtract".into(),
        BlendMode::Difference => "difference".into(),
        BlendMode::Other(n) => format!("other_{n}"),
    }
}

fn direction_name(d: AnimDirection) -> &'static str {
    match d {
        AnimDirection::Forward => "forward",
        AnimDirection::Reverse => "reverse",
        AnimDirection::PingPong => "ping_pong",
        AnimDirection::PingPongReverse => "ping_pong_reverse",
    }
}

fn describe_sprite(s: &Sprite) -> Value {
    let layers: Vec<Value> = s
        .layers
        .iter()
        .map(|l| {
            let mut v = json!({
                "id": l.id.0,
                "name": l.name,
                "kind": match l.kind {
                    LayerKind::Group => "group",
                    LayerKind::Instance => "instance",
                    LayerKind::Tilemap => "tilemap",
                    LayerKind::Image => "image",
                },
                "parent": l.parent.map(|p| p.0),
                "visible": l.visible,
                "locked": l.locked,
                "opacity": l.opacity,
                "blend": blend_name(l.blend),
            });
            if let Some(track) = &l.instance {
                v["instance"] = json!(track);
            }
            if let Some(t) = l.tileset {
                v["tileset"] = json!(t);
            }
            v
        })
        .collect();
    let cels: Vec<Value> = s
        .cels
        .iter()
        .map(|(&(layer, frame), cel)| {
            let (w, h) = s
                .image(cel.image)
                .map_or((0, 0), |i| (i.width(), i.height()));
            let mut v = json!({
                "layer": layer.0,
                "frame": frame,
                "x": cel.position.0,
                "y": cel.position.1,
                "w": w,
                "h": h,
                "opacity": cel.opacity,
                "image": cel.image.0,
            });
            if let Some(g) = s.tilemap(cel.image) {
                v["grid"] = json!([g.width, g.height]);
            }
            v
        })
        .collect();
    let tags: Vec<Value> = s
        .tags
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "from": t.from,
                "to": t.to,
                "direction": direction_name(t.direction),
                "color": t.color.to_hex(),
                "repeat": t.repeat,
            })
        })
        .collect();
    json!({
        "width": s.width,
        "height": s.height,
        "color_mode": match s.color_mode {
            ColorMode::Rgba => "rgba",
            ColorMode::Indexed { .. } => "indexed",
        },
        "palette": s.palette.colors().iter().map(|c| c.to_hex()).collect::<Vec<_>>(),
        "tilesets": s.tilesets.iter().enumerate().map(|(i, t)| json!({
            "index": i,
            "name": t.name,
            "tile_size": [t.tile_size.0, t.tile_size.1],
            "tiles": t.tiles.len(),
            "base_index": t.base_index,
        })).collect::<Vec<_>>(),
        "layers_bottom_to_top": layers,
        "frame_durations_ms": s.frames.iter().map(|f| f.duration_ms).collect::<Vec<_>>(),
        "tags": tags,
        "cels": cels,
        "slices": s.slices.iter().map(|sl| json!({
            "name": sl.name,
            "color": sl.color.to_hex(),
            "text": sl.text,
            "keys": sl.keys.iter().map(|k| json!({
                "frame": k.frame,
                "x": k.bounds.0, "y": k.bounds.1, "w": k.bounds.2, "h": k.bounds.3,
                "pivot": k.pivot.map(|(x, y)| [x, y]),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn describe_document(id: &str, d: &OpenDoc) -> Value {
    let artboards: Vec<Value> = d
        .document
        .artboards
        .iter()
        .map(|a| {
            let mut v = describe_sprite(&a.sprite);
            let obj = v.as_object_mut().expect("object");
            obj.insert("id".into(), json!(a.id.0));
            obj.insert("name".into(), json!(a.name));
            obj.insert("position".into(), json!([a.position.0, a.position.1]));
            v
        })
        .collect();
    json!({
        "doc": id,
        "path": d.path.as_ref().map(|p| p.display().to_string()),
        "can_undo": d.history.can_undo(),
        "can_redo": d.history.can_redo(),
        "artboards": artboards,
        "notes": "Coordinates are artboard pixels from the top-left. Frames are 0-based. \
                  Cels sharing an image id are linked (editing one edits all).",
    })
}

impl Session {
    pub fn new_document(&mut self, p: NewDocument) -> Result<Reply> {
        check_size(p.width, p.height)?;
        let sprite = blank_sprite(p.width, p.height, p.color_mode);
        let name = p.name.unwrap_or_else(|| "Sprite".into());
        let id = self.insert(OpenDoc {
            document: Document::with_sprite(name, sprite),
            history: History::new(),
            path: None,
        });
        Ok(Reply::text(format!(
            "Created document {id}: one {}×{} artboard (id 1) with layer 1 'Layer 1' and frame 0.",
            p.width, p.height
        )))
    }

    pub fn open(&mut self, p: Open) -> Result<Reply> {
        let path = self.path(&p.path)?;
        let bytes = std::fs::read(&path)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&p.path)
            .to_string();
        let (document, report) = files::decode(&name, &bytes)?;
        let keeps_path = files::kind_of(&name) != Some(FileKind::Png);
        let id = self.insert(OpenDoc {
            document,
            history: History::new(),
            path: keeps_path.then_some(path),
        });
        let mut text = format!("Opened {name} as {id}.");
        for w in report.warnings {
            text.push_str(&format!("\nWarning: {w}"));
        }
        let description = describe_document(&id, self.doc(&id)?);
        text.push('\n');
        text.push_str(&serde_json::to_string_pretty(&description).expect("json"));
        Ok(Reply::text(text))
    }

    pub fn save(&mut self, p: Save) -> Result<Reply> {
        let path = match &p.path {
            Some(path) => self.path(path)?,
            None => self.doc(&p.doc)?.path.clone().ok_or_else(|| {
                invalid("this document has no file yet; pass a path ending in .chimp or .aseprite")
            })?,
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let d = self.doc_mut(&p.doc)?;
        let bytes = files::encode(&d.document, &name)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes)?;
        let mut text = format!("Saved {} to {}.", p.doc, path.display());
        let has_guides = d.document.artboards.iter().any(|a| !a.guides.is_empty());
        if files::kind_of(&name) == Some(FileKind::Aseprite) && has_guides {
            text.push_str(
                "\nWarning: .aseprite files don't store guides; save as .chimp to keep them.",
            );
        }
        if files::bakes_masks(&d.document, &name) {
            text.push_str(
                "\nNote: .aseprite has no layer/clipping masks; they were applied to the pixels \
                 in the file (the open document keeps them). Save as .chimp to keep them editable.",
            );
        }
        let missing = files::tilesets_missing_tiles(&d.document, &name);
        if !missing.is_empty() {
            text.push_str("\nWarning: ");
            text.push_str(&files::missing_tiles_note(&missing));
        }
        if files::converts_tilemaps(&d.document, &name) {
            text.push_str(
                "\nNote: tilemap layers with a mask or clipping were saved as image layers in \
                 that file so the mask could be applied.",
            );
        }
        d.path = Some(path);
        Ok(Reply::text(text))
    }

    pub fn close(&mut self, p: DocOnly) -> Result<Reply> {
        self.doc(&p.doc)?;
        self.docs.remove(&p.doc);
        Ok(Reply::text(format!(
            "Closed {} (unsaved changes are discarded).",
            p.doc
        )))
    }

    pub fn list_documents(&self) -> Result<Reply> {
        let docs: Vec<Value> = self
            .docs
            .iter()
            .map(|(id, d)| {
                let artboards: Vec<Value> = d
                    .document
                    .artboards
                    .iter()
                    .map(|a| {
                        json!({
                            "id": a.id.0,
                            "name": a.name,
                            "width": a.sprite.width,
                            "height": a.sprite.height,
                        })
                    })
                    .collect();
                json!({
                    "doc": id,
                    "path": d.path.as_ref().map(|p| p.display().to_string()),
                    "artboards": artboards,
                })
            })
            .collect();
        Ok(Reply::json(json!(docs)))
    }

    pub fn describe(&self, p: DocOnly) -> Result<Reply> {
        Ok(Reply::json(describe_document(&p.doc, self.doc(&p.doc)?)))
    }

    pub fn undo(&mut self, p: DocOnly) -> Result<Reply> {
        let d = self.doc_mut(&p.doc)?;
        Ok(Reply::text(if d.history.undo(&mut d.document)? {
            "Undone."
        } else {
            "Nothing to undo."
        }))
    }

    pub fn redo(&mut self, p: DocOnly) -> Result<Reply> {
        let d = self.doc_mut(&p.doc)?;
        Ok(Reply::text(if d.history.redo(&mut d.document)? {
            "Redone."
        } else {
            "Nothing to redo."
        }))
    }

    pub fn apply_commands(&mut self, p: ApplyCommands) -> Result<Reply> {
        let cmds = p
            .commands
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                serde_json::from_value::<DocCommand>(v)
                    .map_err(|e| invalid(format!("command {i} is not a valid DocCommand: {e}")))
            })
            .collect::<Result<Vec<_>>>()?;
        let n = cmds.len();
        self.run(&p.doc, vec![DocCommand::Batch(cmds)])?;
        Ok(Reply::text(format!(
            "Applied {n} command(s) as one undo step."
        )))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ops::test_util::{json as parse, p, session};

    #[test]
    fn new_document_and_describe() {
        let s = session(16, 8);
        let d = parse(&s.describe(p(json!({"doc": "d1"}))).unwrap());
        let a = &d["artboards"][0];
        assert_eq!(a["width"], 16);
        assert_eq!(a["height"], 8);
        assert_eq!(a["layers_bottom_to_top"][0]["name"], "Layer 1");
        assert_eq!(a["frame_durations_ms"], json!([100]));
        assert!(
            session(1, 1)
                .new_document(p(json!({"width": 0, "height": 4})))
                .is_err()
        );
    }

    #[test]
    fn indexed_documents_start_with_a_transparent_index() {
        let mut s = Session::new(None);
        s.new_document(p(json!({"width": 2, "height": 2, "color_mode": "indexed"})))
            .unwrap();
        let (_, sprite) = s.sprite("d1", None).unwrap();
        assert_eq!(sprite.palette.get(0), Some(Rgba::TRANSPARENT));
        assert_eq!(sprite.palette.len(), 33);
    }

    #[test]
    fn save_open_close_roundtrip() {
        let dir = std::env::temp_dir().join(format!("cs-agent-doc-{}", std::process::id()));
        let mut s = Session::new(Some(dir.clone()));
        s.new_document(p(json!({"width": 3, "height": 2, "name": "hero"})))
            .unwrap();
        assert!(s.save(p(json!({"doc": "d1"}))).is_err(), "no path yet");
        s.save(p(json!({"doc": "d1", "path": "out/hero.chimp"})))
            .unwrap();
        s.save(p(json!({"doc": "d1"}))).unwrap();
        let opened = s.open(p(json!({"path": "out/hero.chimp"}))).unwrap();
        assert!(opened.text.contains("as d2"), "{}", opened.text);
        assert_eq!(s.sprite("d2", None).unwrap().1.width, 3);
        s.save(p(json!({"doc": "d2", "path": "hero.aseprite"})))
            .unwrap();
        s.open(p(json!({"path": "hero.aseprite"}))).unwrap();
        let list = parse(&s.list_documents().unwrap());
        assert_eq!(list.as_array().unwrap().len(), 3);
        s.close(p(json!({"doc": "d1"}))).unwrap();
        assert!(s.describe(p(json!({"doc": "d1"}))).is_err());
        assert!(s.open(p(json!({"path": "../x.chimp"}))).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn saving_aseprite_reports_tilemap_notes_cleanly() {
        let dir = std::env::temp_dir().join(format!("cs-agent-notes-{}", std::process::id()));
        let mut s = Session::new(Some(dir.clone()));
        s.new_document(p(json!({"width": 8, "height": 8}))).unwrap();
        s.create_tileset(p(
            json!({"doc": "d1", "name": "t", "tile_width": 4, "tile_height": 4}),
        ))
        .unwrap();
        s.set_tile_pixels(p(json!({"doc": "d1", "tileset": 0, "tile": 1,
            "pixels": [{"x": 0, "y": 0, "color": "#ff0000"}]})))
            .unwrap();
        s.add_tilemap_layer(p(json!({"doc": "d1", "tileset": 0, "name": "map"})))
            .unwrap();
        s.set_tiles(p(
            json!({"doc": "d1", "layer": "map", "cells": [{"x": 0, "y": 0, "tile": 1}]}),
        ))
        .unwrap();
        // Máscara en la tilemap y un tileset al que le falta el tile que usa la grilla.
        let sprite = &mut s.doc_mut("d1").unwrap().document.artboards[0].sprite;
        let layer = sprite.layers.iter_mut().find(|l| l.name == "map").unwrap();
        layer.mask = Some(cs_core::LayerMask::filled(8, 8, 255));
        sprite.tilesets[0].tiles.truncate(1);
        let text = s
            .save(p(json!({"doc": "d1", "path": "notes.aseprite"})))
            .unwrap()
            .text;
        assert!(
            text.contains("\nWarning: Tileset 't' is missing tiles"),
            "{text}"
        );
        assert!(
            text.contains("\nNote: tilemap layers with a mask"),
            "{text}"
        );
        assert!(
            text.contains("\nNote: .aseprite has no layer/clipping masks"),
            "{text}"
        );
        assert!(!text.contains("  "), "no runs of spaces: {text}");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn apply_commands_is_atomic_and_undoable() {
        let mut s = session(4, 4);
        let ok = json!({"OnArtboard": {"id": 1, "cmd": {"SetFrameDuration": {"index": 0, "duration_ms": 80}}}});
        let bad = json!({"OnArtboard": {"id": 1, "cmd": {"SetFrameDuration": {"index": 5, "duration_ms": 80}}}});
        assert!(
            s.apply_commands(p(json!({"doc": "d1", "commands": [ok, bad]})))
                .is_err()
        );
        assert_eq!(s.sprite("d1", None).unwrap().1.frames[0].duration_ms, 100);
        assert!(
            s.apply_commands(p(json!({"doc": "d1", "commands": [{"Nope": 1}]})))
                .is_err()
        );
        s.apply_commands(p(json!({"doc": "d1", "commands": [ok]})))
            .unwrap();
        assert_eq!(s.sprite("d1", None).unwrap().1.frames[0].duration_ms, 80);
        assert_eq!(s.undo(p(json!({"doc": "d1"}))).unwrap().text, "Undone.");
        assert_eq!(s.sprite("d1", None).unwrap().1.frames[0].duration_ms, 100);
        assert_eq!(
            s.undo(p(json!({"doc": "d1"}))).unwrap().text,
            "Nothing to undo."
        );
        assert_eq!(s.redo(p(json!({"doc": "d1"}))).unwrap().text, "Redone.");
    }

    #[test]
    fn apply_commands_still_takes_the_old_symmetry_mode() {
        let mut s = session(4, 4);
        // Así se pedía la simetría antes, con un solo modo.
        let old = json!({"SetSymmetry": {"id": 1, "symmetry": {"mode": "Both", "center2": null}}});
        s.apply_commands(p(json!({"doc": "d1", "commands": [old]})))
            .unwrap();
        let axes = s.doc("d1").unwrap().document.artboards[0].symmetry.axes;
        assert!(axes.vertical && axes.horizontal, "{axes:?}");
        assert!(!axes.diagonal_down && !axes.diagonal_up, "{axes:?}");
        // Y con el formato nuevo.
        let new = json!({"SetSymmetry": {"id": 1, "symmetry": {"axes": {"diagonal_up": true}}}});
        s.apply_commands(p(json!({"doc": "d1", "commands": [new]})))
            .unwrap();
        let axes = s.doc("d1").unwrap().document.artboards[0].symmetry.axes;
        assert!(axes.diagonal_up && !axes.vertical, "{axes:?}");
    }
}
