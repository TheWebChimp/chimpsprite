use super::*;
use cs_core::NoteColor;

fn editor() -> Editor {
    Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba))
}

fn last_step(e: &Editor) -> Option<&'static str> {
    e.history.done().last()
}

#[test]
fn each_note_change_is_its_own_undo_step_with_its_name() {
    let mut e = editor();
    let a = e.add_note((10, -4)).unwrap();
    assert_eq!(last_step(&e), Some("New Note"));
    let note = e.document.note(a).unwrap();
    assert_eq!(
        (note.position, note.size),
        ((10, -4), crate::notes::NEW_SIZE)
    );
    assert!(e.set_note_text(a, "hola\nmundo".into()));
    assert_eq!(last_step(&e), Some("Note"));
    assert!(
        !e.set_note_text(a, "hola\nmundo".into()),
        "same text: no step"
    );
    assert!(e.set_note_color(a, NoteColor::Green));
    assert_eq!(last_step(&e), Some("Note"));
    let mut moved = e.document.note(a).unwrap().clone();
    moved.position = (30, 30);
    assert!(e.set_note(moved.clone()));
    assert!(!e.set_note(moved), "unchanged: no step");
    assert!(e.remove_note(a));
    assert_eq!(last_step(&e), Some("Delete Note"));
    assert!(e.document.notes.is_empty());
    assert!(!e.remove_note(a), "already gone");
    // Todo se deshace paso a paso.
    e.undo();
    assert_eq!(e.document.note(a).unwrap().position, (30, 30));
    e.undo();
    e.undo();
    assert_eq!(e.document.note(a).unwrap().color, NoteColor::Yellow);
    e.undo();
    assert_eq!(e.document.note(a).unwrap().text, "");
    e.undo();
    assert!(e.document.notes.is_empty());
}

#[test]
fn duplicate_copies_a_note_a_bit_down_and_right_on_top() {
    let mut e = editor();
    let a = e.add_note((0, 0)).unwrap();
    let b = e.add_note((50, 0)).unwrap();
    e.set_note_text(a, "copy me".into());
    e.set_note_color(a, NoteColor::Blue);
    let c = e.duplicate_note(a).unwrap();
    assert_ne!(c, a);
    assert_ne!(c, b);
    let ids: Vec<u32> = e.document.notes.iter().map(|n| n.id).collect();
    assert_eq!(ids, [a, b, c], "the copy on top");
    let (orig, copy) = (e.document.note(a).unwrap(), e.document.note(c).unwrap());
    assert_eq!(copy.position, (orig.position.0 + 12, orig.position.1 + 12));
    assert_eq!(
        (&copy.text, copy.color, copy.size),
        (&orig.text, orig.color, orig.size)
    );
    assert_eq!(last_step(&e), Some("New Note"));
    assert!(e.duplicate_note(99).is_none());
}

#[test]
fn bring_to_front_moves_the_note_last() {
    let mut e = editor();
    let a = e.add_note((0, 0)).unwrap();
    let b = e.add_note((5, 0)).unwrap();
    assert!(e.note_to_front(a));
    let ids: Vec<u32> = e.document.notes.iter().map(|n| n.id).collect();
    assert_eq!(ids, [b, a]);
    assert!(!e.note_to_front(a), "already on top");
}
