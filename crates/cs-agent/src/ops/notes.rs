//! Las notas (post-its) del lienzo: agregar, cambiar y quitar. Son del documento (no de un
//! sprite): se guardan en el `.chimp` y no salen en los exports.

use cs_commands::DocCommand;
use cs_core::{Document, Note, NoteColor};
use serde_json::{Value, json};

use crate::error::{Result, invalid};
use crate::params::{AddNote, NoteColorArg, NoteOnly, SetNote};
use crate::{Reply, Session};

/// Separación (en el mundo) de una nota nueva sin posición: a la derecha de los artboards y
/// debajo de otra que ya esté ahí.
const GAP: i32 = 8;

fn color(c: NoteColorArg) -> NoteColor {
    match c {
        NoteColorArg::Yellow => NoteColor::Yellow,
        NoteColorArg::Pink => NoteColor::Pink,
        NoteColorArg::Blue => NoteColor::Blue,
        NoteColorArg::Green => NoteColor::Green,
    }
}

fn color_name(c: NoteColor) -> &'static str {
    match c {
        NoteColor::Yellow => "yellow",
        NoteColor::Pink => "pink",
        NoteColor::Blue => "blue",
        NoteColor::Green => "green",
    }
}

fn check_note_size([w, h]: [u32; 2]) -> Result<(u32, u32)> {
    let (min, max) = (Note::MIN_SIZE, Note::MAX_SIZE);
    if w < min.0 || h < min.1 || w > max.0 || h > max.1 {
        return Err(invalid(format!(
            "invalid note size {w}×{h}; it must be from {}×{} to {}×{}",
            min.0, min.1, max.0, max.1
        )));
    }
    Ok((w, h))
}

/// Las notas para `describe`, de abajo hacia arriba.
pub(crate) fn describe_notes(doc: &Document) -> Value {
    doc.notes
        .iter()
        .map(|n| {
            json!({
                "id": n.id,
                "position": [n.position.0, n.position.1],
                "size": [n.size.0, n.size.1],
                "color": color_name(n.color),
                "text": n.text,
            })
        })
        .collect()
}

/// Dónde va una nota sin posición: a la derecha de los artboards del lienzo, alineada arriba, y
/// más abajo mientras ya haya una nota en ese lugar.
fn default_position(doc: &Document) -> (i32, i32) {
    let (x, mut y) = doc
        .bounds()
        .map_or((0, 0), |((_, y0), (x1, _))| (x1 + GAP, y0));
    while doc.notes.iter().any(|n| n.position == (x, y)) {
        y += Note::DEFAULT_SIZE.1 as i32 + GAP;
    }
    (x, y)
}

impl Session {
    pub fn add_note(&mut self, p: AddNote) -> Result<Reply> {
        let size = p
            .size
            .map(check_note_size)
            .transpose()?
            .unwrap_or(Note::DEFAULT_SIZE);
        let doc = &self.doc(&p.doc)?.document;
        let position = p
            .position
            .map_or_else(|| default_position(doc), |[x, y]| (x, y));
        let id = doc.next_note_id();
        let note = Note {
            id,
            position,
            size,
            text: p.text,
            color: p.color.map(color).unwrap_or_default(),
        };
        let index = doc.notes.len();
        self.run(&p.doc, vec![DocCommand::AddNote { index, note }])?;
        Ok(Reply::text(format!(
            "Added note {id} at ({}, {}).",
            position.0, position.1
        )))
    }

    pub fn set_note(&mut self, p: SetNote) -> Result<Reply> {
        let doc = &self.doc(&p.doc)?.document;
        let mut note = doc
            .note(p.note)
            .cloned()
            .ok_or_else(|| invalid(format!("note {} not found", p.note)))?;
        if p.text.is_none()
            && p.position.is_none()
            && p.size.is_none()
            && p.color.is_none()
            && !p.to_front
        {
            return Err(invalid(
                "nothing to change: pass text, position, size, color or to_front",
            ));
        }
        if let Some(size) = p.size {
            note.size = check_note_size(size)?;
        }
        if let Some(text) = p.text {
            note.text = text;
        }
        if let Some([x, y]) = p.position {
            note.position = (x, y);
        }
        if let Some(c) = p.color {
            note.color = color(c);
        }
        // Al frente: se quita y se vuelve a poner arriba (un paso con el resto del cambio).
        let cmds = if p.to_front {
            vec![
                DocCommand::RemoveNote { id: note.id },
                DocCommand::AddNote {
                    index: doc.notes.len() - 1,
                    note,
                },
            ]
        } else {
            vec![DocCommand::SetNote(note)]
        };
        self.run(&p.doc, cmds)?;
        Ok(Reply::text(format!("Updated note {}.", p.note)))
    }

    pub fn remove_note(&mut self, p: NoteOnly) -> Result<Reply> {
        if self.doc(&p.doc)?.document.note(p.note).is_none() {
            return Err(invalid(format!("note {} not found", p.note)));
        }
        self.run(&p.doc, vec![DocCommand::RemoveNote { id: p.note }])?;
        Ok(Reply::text(format!("Removed note {}.", p.note)))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ops::test_util::{json as parse, p, session};

    fn notes(s: &Session) -> Vec<Note> {
        s.document("d1").unwrap().notes.clone()
    }

    #[test]
    fn add_note_defaults_to_the_right_of_the_artboards_and_stacks_down() {
        let mut s = session(16, 8);
        let r = s
            .add_note(p(json!({"doc": "d1", "text": "hola\nmundo"})))
            .unwrap();
        assert_eq!(r.text, "Added note 1 at (24, 0).");
        s.add_note(p(json!({"doc": "d1"}))).unwrap();
        let n = notes(&s);
        assert_eq!(n[0].text, "hola\nmundo");
        assert_eq!(
            (n[0].size, n[0].color),
            (Note::DEFAULT_SIZE, NoteColor::Yellow)
        );
        assert_eq!(
            n[1].position,
            (24, Note::DEFAULT_SIZE.1 as i32 + GAP),
            "the second one goes below"
        );
        assert_eq!(n[1].id, 2);
    }

    #[test]
    fn add_note_with_everything_and_bad_sizes() {
        let mut s = session(16, 8);
        s.add_note(p(json!({
            "doc": "d1", "text": "x", "position": [-40, 12], "size": [120, 70], "color": "pink"
        })))
        .unwrap();
        let n = &notes(&s)[0];
        assert_eq!(
            (n.position, n.size, n.color),
            ((-40, 12), (120, 70), NoteColor::Pink)
        );
        for size in [[10, 70], [120, 5], [5000, 70]] {
            let err = s
                .add_note(p(json!({"doc": "d1", "size": size})))
                .unwrap_err()
                .to_string();
            assert!(err.contains("invalid note size"), "{err}");
        }
        assert_eq!(notes(&s).len(), 1);
    }

    #[test]
    fn set_note_changes_what_is_passed_in_one_undo_step() {
        let mut s = session(16, 8);
        s.add_note(p(json!({"doc": "d1", "text": "a"}))).unwrap();
        s.add_note(p(json!({"doc": "d1", "text": "b"}))).unwrap();
        s.set_note(p(json!({
            "doc": "d1", "note": 1, "text": "otra", "color": "blue", "position": [3, 4],
            "to_front": true
        })))
        .unwrap();
        let n = notes(&s);
        assert_eq!(n.iter().map(|n| n.id).collect::<Vec<_>>(), [2, 1]);
        assert_eq!(
            (n[1].text.as_str(), n[1].color, n[1].position),
            ("otra", NoteColor::Blue, (3, 4))
        );
        s.undo(p(json!({"doc": "d1"}))).unwrap();
        let n = notes(&s);
        assert_eq!(n.iter().map(|n| n.id).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(n[0].text, "a");
        assert!(s.set_note(p(json!({"doc": "d1", "note": 1}))).is_err());
        assert!(
            s.set_note(p(json!({"doc": "d1", "note": 9, "text": "x"})))
                .is_err()
        );
        assert!(
            s.set_note(p(json!({"doc": "d1", "note": 1, "size": [1, 1]})))
                .is_err()
        );
    }

    #[test]
    fn remove_note_and_describe() {
        let mut s = session(16, 8);
        s.add_note(p(json!({"doc": "d1", "text": "a", "color": "green"})))
            .unwrap();
        let d = parse(&s.describe(p(json!({"doc": "d1"}))).unwrap());
        assert_eq!(
            d["sticky_notes"],
            json!([{"id": 1, "position": [24, 0], "size": [86, 89], "color": "green", "text": "a"}])
        );
        s.remove_note(p(json!({"doc": "d1", "note": 1}))).unwrap();
        assert!(notes(&s).is_empty());
        assert!(s.remove_note(p(json!({"doc": "d1", "note": 1}))).is_err());
        s.undo(p(json!({"doc": "d1"}))).unwrap();
        assert_eq!(notes(&s).len(), 1);
    }

    #[test]
    fn notes_go_by_name_too() {
        let mut s = session(4, 4);
        s.call("add_note", json!({"doc": "d1", "text": "por nombre"}))
            .unwrap();
        s.call("set_note", json!({"doc": "d1", "note": 1, "color": "pink"}))
            .unwrap();
        assert_eq!(notes(&s)[0].color, NoteColor::Pink);
        s.call("remove_note", json!({"doc": "d1", "note": 1}))
            .unwrap();
        assert!(notes(&s).is_empty());
    }

    #[test]
    fn notes_are_saved_in_chimp_but_never_exported() {
        let dir = std::env::temp_dir().join(format!("cs-agent-notes-x-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Session::new(Some(dir.clone()));
        s.new_document(p(json!({"width": 4, "height": 4}))).unwrap();
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 1, "y": 1, "color": "#f00"}]}),
        ))
        .unwrap();
        s.export(p(json!({"doc": "d1", "path": "before.png", "kind": "png"})))
            .unwrap();
        s.add_note(p(
            json!({"doc": "d1", "text": "no sale", "position": [0, 0]}),
        ))
        .unwrap();
        s.export(p(json!({"doc": "d1", "path": "after.png", "kind": "png"})))
            .unwrap();
        assert_eq!(
            std::fs::read(dir.join("before.png")).unwrap(),
            std::fs::read(dir.join("after.png")).unwrap()
        );
        s.save(p(json!({"doc": "d1", "path": "with_notes.chimp"})))
            .unwrap();
        s.open(p(json!({"path": "with_notes.chimp"}))).unwrap();
        assert_eq!(s.document("d2").unwrap().notes, notes(&s));
        std::fs::remove_dir_all(&dir).ok();
    }
}
