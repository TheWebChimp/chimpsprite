use cs_3d::{Item, Primitive, SelectMode, Selection};
use cs_core::{ColorMode, Node, Sprite};

use super::{DropAt, Editor, Row};

/// Un modelo con dos objetos: el cubo (0) y un plano (1).
fn editor_with_model() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_model("Ship", (64, 64)));
    assert!(e.add_primitive(Primitive::Plane));
    e
}

fn objects(e: &Editor) -> Vec<usize> {
    let mut out: Vec<usize> = e
        .model_edit()
        .selection
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Object(o) => Some(*o),
            _ => None,
        })
        .collect();
    out.sort_unstable();
    out
}

fn select_objects(e: &mut Editor, os: &[usize]) {
    e.set_model_selection(Selection {
        mode: SelectMode::Object,
        items: os.iter().map(|&o| Item::Object(o)).collect(),
    });
}

/// Un modelo con el grupo 0 que tiene a los dos objetos.
fn editor_with_group() -> Editor {
    let mut e = editor_with_model();
    select_objects(&mut e, &[0, 1]);
    assert!(e.new_group());
    e
}

fn tree(e: &Editor) -> Vec<Node> {
    e.model().unwrap().tree.clone()
}

#[test]
fn the_active_row_is_the_group_or_else_the_object() {
    let mut e = editor_with_group();
    assert_eq!(e.active_row(), Some(Row::Group(0)));
    e.select_model_object(Some(1));
    assert_eq!(e.active_row(), Some(Row::Object(1)));
    e.set_active_row(None, false);
    assert_eq!(e.active_row(), None);
}

#[test]
fn choosing_a_group_selects_its_objects() {
    let mut e = editor_with_group();
    e.set_active_row(Some(Row::Object(0)), false);
    assert_eq!(objects(&e), vec![0]);
    assert_eq!(e.selected_object(), Some(0));
    e.set_active_row(Some(Row::Group(0)), false);
    assert_eq!(e.active_row(), Some(Row::Group(0)));
    assert_eq!(e.model_edit().selection.mode, SelectMode::Object);
    assert_eq!(objects(&e), vec![0, 1]);
    assert_eq!(e.selected_object(), None);
}

#[test]
fn shift_adds_or_removes_a_row() {
    let mut e = editor_with_model();
    e.set_active_row(Some(Row::Object(0)), false);
    e.set_active_row(Some(Row::Object(1)), true);
    assert_eq!(objects(&e), vec![0, 1]);
    assert_eq!(e.active_row(), Some(Row::Object(1)));
    e.set_active_row(Some(Row::Object(0)), true);
    assert_eq!(objects(&e), vec![1]);
    assert_eq!(e.active_row(), Some(Row::Object(0)));
}

#[test]
fn hiding_is_one_undo_step() {
    let mut e = editor_with_group();
    let steps = e.history.done_count();
    assert!(e.toggle_hidden(Row::Object(1)));
    assert!(e.model().unwrap().objects[1].hidden);
    assert_eq!(e.history.done_count(), steps + 1);
    assert!(e.toggle_hidden(Row::Group(0)));
    assert!(e.model().unwrap().groups[0].hidden);
    assert_eq!(e.history.done_count(), steps + 2);
    e.undo();
    assert!(!e.model().unwrap().groups[0].hidden);
    assert!(!e.toggle_hidden(Row::Group(7)));
}

#[test]
fn locking_is_one_undo_step() {
    let mut e = editor_with_group();
    let steps = e.history.done_count();
    assert!(e.toggle_row_locked(Row::Object(0)));
    assert!(e.model().unwrap().objects[0].locked);
    assert!(e.toggle_row_locked(Row::Group(0)));
    assert!(e.model().unwrap().groups[0].locked);
    assert_eq!(e.history.done_count(), steps + 2);
    assert!(e.toggle_row_locked(Row::Object(0)));
    assert!(!e.model().unwrap().objects[0].locked);
}

#[test]
fn double_sided_is_one_undo_step() {
    let mut e = editor_with_model();
    let steps = e.history.done_count();
    assert!(e.toggle_double_sided(1));
    assert!(e.model().unwrap().objects[1].double_sided);
    assert_eq!(e.history.done_count(), steps + 1);
    assert!(!e.toggle_double_sided(9));
}

#[test]
fn opening_a_group_is_not_an_undo_step() {
    let mut e = editor_with_group();
    let steps = e.history.done_count();
    assert!(e.model().unwrap().groups[0].open);
    let revision = e.revision;
    e.toggle_open(0);
    assert!(!e.model().unwrap().groups[0].open);
    assert_eq!(e.history.done_count(), steps);
    assert!(e.revision > revision);
}

#[test]
fn renaming_is_one_undo_step() {
    let mut e = editor_with_group();
    let steps = e.history.done_count();
    assert!(e.rename_row(Row::Group(0), "Wings".into()));
    assert_eq!(e.model().unwrap().groups[0].name, "Wings");
    assert!(e.rename_row(Row::Object(1), "Wing".into()));
    assert_eq!(e.model().unwrap().objects[1].name, "Wing");
    assert_eq!(e.history.done_count(), steps + 2);
    assert!(!e.rename_row(Row::Object(1), "  ".into()));
    assert!(!e.rename_row(Row::Object(1), "Wing".into()));
    assert_eq!(e.history.done_count(), steps + 2);
}

#[test]
fn a_new_group_takes_the_chosen_objects_where_the_first_was() {
    let mut e = editor_with_model();
    assert!(e.add_primitive(Primitive::Cylinder { sides: 8 }));
    select_objects(&mut e, &[2, 1]);
    let steps = e.history.done_count();
    assert!(e.new_group());
    assert_eq!(e.history.done_count(), steps + 1);
    let model = e.model().unwrap();
    assert_eq!(model.groups.len(), 1);
    assert_eq!(model.groups[0].name, "Group");
    assert_eq!(
        model.tree,
        vec![
            Node::Object(0),
            Node::Group {
                group: 0,
                children: vec![Node::Object(1), Node::Object(2)],
            },
        ]
    );
    assert_eq!(e.active_row(), Some(Row::Group(0)));
    // Sin nada elegido: uno vacío al final, con otro nombre.
    e.set_active_row(None, false);
    assert!(e.new_group());
    let model = e.model().unwrap();
    assert_eq!(model.groups[1].name, "Group 2");
    assert_eq!(
        model.tree.last(),
        Some(&Node::Group {
            group: 1,
            children: Vec::new(),
        })
    );
}

#[test]
fn moving_a_row_is_one_undo_step() {
    let mut e = editor_with_group();
    assert!(e.add_primitive(Primitive::Cube));
    // Lo nuevo entra en el grupo activo.
    assert_eq!(e.model().unwrap().object_parents(2), vec![0]);
    let steps = e.history.done_count();
    assert!(e.move_row(Row::Object(2), DropAt::Before(Row::Group(0))));
    assert_eq!(e.history.done_count(), steps + 1);
    assert_eq!(tree(&e)[0], Node::Object(2));
    assert!(e.move_row(Row::Object(2), DropAt::Into(0)));
    assert_eq!(e.model().unwrap().object_parents(2), vec![0]);
    assert!(e.move_row(Row::Object(0), DropAt::After(Row::Object(2))));
    assert_eq!(
        tree(&e),
        vec![Node::Group {
            group: 0,
            children: vec![Node::Object(1), Node::Object(2), Node::Object(0)],
        }]
    );
    assert_eq!(e.history.done_count(), steps + 3);
}

#[test]
fn a_group_does_not_go_into_itself_or_a_child() {
    let mut e = editor_with_group();
    e.set_active_row(None, false);
    assert!(e.new_group());
    assert!(e.move_row(Row::Group(1), DropAt::Into(0)));
    let steps = e.history.done_count();
    assert!(!e.move_row(Row::Group(0), DropAt::Into(0)));
    assert!(!e.move_row(Row::Group(0), DropAt::Into(1)));
    assert!(!e.move_row(Row::Group(0), DropAt::Before(Row::Object(0))));
    assert!(!e.move_row(Row::Object(0), DropAt::After(Row::Object(0))));
    assert_eq!(e.history.done_count(), steps);
}

#[test]
fn deleting_a_group_takes_what_is_inside() {
    let mut e = editor_with_group();
    // Otro objeto afuera y un subgrupo vacío adentro.
    e.set_active_row(None, false);
    assert!(e.add_primitive(Primitive::Cube));
    e.set_active_row(None, false);
    assert!(e.new_group());
    assert!(e.move_row(Row::Group(1), DropAt::Into(0)));
    // La selección de lo que queda se corre.
    select_objects(&mut e, &[1, 2]);
    let steps = e.history.done_count();
    assert!(e.delete_row(Row::Group(0)));
    assert_eq!(e.history.done_count(), steps + 1);
    let model = e.model().unwrap();
    assert_eq!(model.objects.len(), 1);
    assert!(model.groups.is_empty());
    assert_eq!(model.tree, vec![Node::Object(0)]);
    assert_eq!(objects(&e), vec![0]);
    e.undo();
    assert_eq!(e.model().unwrap().objects.len(), 3);
    assert_eq!(e.model().unwrap().groups.len(), 2);
}

#[test]
fn deleting_a_group_renumbers_the_others() {
    let mut e = editor_with_model();
    select_objects(&mut e, &[0]);
    assert!(e.new_group());
    select_objects(&mut e, &[1]);
    assert!(e.new_group());
    assert!(e.rename_row(Row::Group(1), "Back".into()));
    assert!(e.delete_row(Row::Group(0)));
    let model = e.model().unwrap();
    assert_eq!(model.groups.len(), 1);
    assert_eq!(model.groups[0].name, "Back");
    assert_eq!(
        model.tree,
        vec![Node::Group {
            group: 0,
            children: vec![Node::Object(0)],
        }]
    );
}

#[test]
fn deleting_an_object_row() {
    let mut e = editor_with_model();
    assert!(e.delete_row(Row::Object(0)));
    assert_eq!(e.model().unwrap().objects.len(), 1);
    assert!(!e.delete_row(Row::Group(0)));
}
