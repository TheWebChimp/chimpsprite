use cs_3d::{Item, OrthoView, SelectMode, Selection, ViewMap};
use cs_core::{ColorMode, Model3D, Sprite};

use super::{EditOp, Editor, GizmoView, PerspMap};

fn editing() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_model("Ship", (64, 64)));
    e
}

fn front() -> ViewMap {
    ViewMap {
        view: OrthoView::Front,
        center: [0.0; 3],
        scale: 2.0,
        origin: (100.0, 100.0),
    }
}

fn model(e: &Editor) -> &Model3D {
    e.model().unwrap()
}

/// La cara de arriba del cubo (todos sus vértices con y > 0).
fn top_face(e: &Editor) -> usize {
    let mesh = &model(e).objects[0].mesh;
    mesh.faces
        .iter()
        .position(|f| f.verts.iter().all(|&v| mesh.vertices[v as usize].y > 0))
        .expect("the top face")
}

fn faces(e: &mut Editor, fs: &[usize]) {
    e.set_select_mode(SelectMode::Face);
    e.set_model_selection(Selection {
        mode: SelectMode::Face,
        items: fs.iter().map(|&f| Item::Face(0, f)).collect(),
    });
}

fn vertices(e: &mut Editor, vs: &[u32]) {
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: vs.iter().map(|&v| Item::Vertex(0, v)).collect(),
    });
}

/// Los `y` de los vértices de la cara `f` del modelo del documento.
fn face_ys(e: &Editor, f: usize) -> Vec<i32> {
    let mesh = &model(e).objects[0].mesh;
    mesh.faces[f]
        .verts
        .iter()
        .map(|&v| mesh.vertices[v as usize].y)
        .collect()
}

#[test]
fn extrude_then_move_and_confirm_is_one_undo_step() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    let steps = e.history.done_count();
    assert!(e.can_extrude());
    assert!(e.start_extrude(GizmoView::Ortho(front()), (100.0, 100.0)));
    assert!(matches!(e.model_edit().op, Some(EditOp::Extrude { .. })));
    // 32 puntos hacia arriba = 16/16: una unidad.
    e.update_model_op((100.0, 68.0), false, false);
    assert_eq!(model(&e).objects[0].mesh.vertices.len(), 8, "preview only");
    let shown = e.edit_model_shown().unwrap();
    assert_eq!(shown.objects[0].mesh.vertices.len(), 12);
    e.finish_model_op();
    assert!(e.model_edit().op.is_none());
    assert_eq!(e.history.done_count(), steps + 1);
    assert_eq!(e.history.done().last(), Some("Extrude"));
    let m = model(&e);
    assert_eq!(m.objects[0].mesh.vertices.len(), 12);
    assert_eq!(m.objects[0].mesh.faces.len(), 10);
    assert_eq!(face_ys(&e, top), vec![32; 4], "the face moved out");
    // La selección son las caras extruidas.
    assert_eq!(e.model_edit().selection.items, [Item::Face(0, top)].into());
    e.undo();
    assert_eq!(model(&e).objects[0].mesh.vertices.len(), 8);
    assert_eq!(e.history.done_count(), steps);
}

#[test]
fn cancelling_an_extrude_leaves_nothing() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    let before = model(&e).clone();
    let sel = e.model_edit().selection.clone();
    let steps = e.history.done_count();
    assert!(e.start_extrude(GizmoView::Ortho(front()), (100.0, 100.0)));
    e.update_model_op((100.0, 68.0), false, false);
    assert!(e.cancel_model_op());
    assert_eq!(model(&e), &before);
    assert_eq!(e.edit_model_shown(), Some(&before));
    assert_eq!(e.history.done_count(), steps);
    assert_eq!(e.model_edit().selection, sel);
    assert!(e.model_edit().extrusion.is_none());
}

#[test]
fn extruding_vertices_selects_the_new_faces_and_cancel_brings_the_vertices_back() {
    let mut e = editing();
    let top = top_face(&e);
    let verts = model(&e).objects[0].mesh.faces[top].verts.clone();
    vertices(&mut e, &verts);
    let sel = e.model_edit().selection.clone();
    assert!(e.can_extrude(), "the face of those vertices");
    assert!(e.start_extrude(GizmoView::Ortho(front()), (100.0, 100.0)));
    assert_eq!(e.model_edit().selection.mode, SelectMode::Face);
    e.cancel_model_op();
    assert_eq!(e.model_edit().selection, sel);
}

#[test]
fn confirming_without_moving_still_extrudes() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    assert!(e.start_extrude(GizmoView::Ortho(front()), (100.0, 100.0)));
    e.finish_model_op();
    assert_eq!(model(&e).objects[0].mesh.vertices.len(), 12);
}

#[test]
fn in_the_perspective_the_extrude_follows_the_normal() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    let pm = PerspMap {
        camera: model(&e).camera,
        size: (64, 64),
        min: (0.0, 0.0),
        scale: (4.0, 4.0),
    };
    // El centro de la cara de arriba y una unidad más arriba, en pantalla.
    let from = pm.to_screen([0.0, 16.0, 0.0]).unwrap();
    let to = pm.to_screen([0.0, 32.0, 0.0]).unwrap();
    assert!(e.start_extrude(GizmoView::Persp(pm), from));
    // El puntero se sale un poco de la línea de la normal: solo cuenta lo que va a lo largo.
    e.update_model_op((to.0 + 3.0, to.1), false, false);
    e.finish_model_op();
    let mesh = &model(&e).objects[0].mesh;
    for &v in &mesh.faces[top].verts {
        let p = mesh.vertices[v as usize];
        assert_eq!(p.y, 32, "{p:?}");
        assert_eq!((p.x.abs(), p.z.abs()), (16, 16), "{p:?}");
    }
}

#[test]
fn extrude_moved_is_one_step() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    let steps = e.history.done_count();
    assert!(e.extrude_moved([0.0, 16.0, 0.0]));
    assert_eq!(e.history.done_count(), steps + 1);
    assert_eq!(face_ys(&e, top), vec![32; 4]);
}

#[test]
fn subdivide_merge_and_delete_are_one_named_step_each() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    let steps = e.history.done_count();
    assert!(e.subdivide_model_selection());
    assert_eq!(e.history.done_count(), steps + 1);
    assert_eq!(e.history.done().last(), Some("Subdivide"));
    // 6 - 1 + 4 caras (las vecinas se re-arman: también hay más).
    assert!(model(&e).objects[0].mesh.faces.len() > 6);
    assert_eq!(e.model_edit().selection.items.len(), 4, "the 4 new faces");

    let mut e = editing();
    let top = top_face(&e);
    let verts = model(&e).objects[0].mesh.faces[top].verts.clone();
    vertices(&mut e, &verts);
    let steps = e.history.done_count();
    assert!(e.merge_model_selection());
    assert_eq!(e.history.done_count(), steps + 1);
    assert_eq!(e.history.done().last(), Some("Merge"));
    assert_eq!(model(&e).objects[0].mesh.vertices.len(), 5);

    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    let steps = e.history.done_count();
    assert!(e.delete_model_selection());
    assert_eq!(e.history.done_count(), steps + 1);
    assert_eq!(e.history.done().last(), Some("Delete"));
    assert_eq!(model(&e).objects[0].mesh.faces.len(), 5);
    assert!(e.model_edit().selection.is_empty());
}

#[test]
fn what_applies_depends_on_the_selection() {
    let mut e = editing();
    let can = |e: &Editor| {
        (
            e.can_extrude(),
            e.can_subdivide(),
            e.can_merge(),
            e.can_delete_model_selection(),
        )
    };
    vertices(&mut e, &[]);
    assert_eq!(can(&e), (false, false, false, false), "nothing");
    vertices(&mut e, &[0]);
    assert_eq!(can(&e), (false, false, false, true), "one vertex");
    vertices(&mut e, &[0, 1]);
    assert_eq!(can(&e), (false, false, true, true), "two vertices");
    faces(&mut e, &[0]);
    assert_eq!(can(&e), (true, true, true, true), "a face");
    // En modo Object solo se borra.
    e.select_model_object(Some(0));
    assert_eq!(can(&e), (false, false, false, true), "an object");
    e.request_extrude();
    assert!(e.model_edit().key_request.is_none());
    let steps = e.history.done_count();
    assert!(!e.subdivide_model_selection());
    assert!(!e.merge_model_selection());
    assert_eq!(e.history.done_count(), steps);
}

#[test]
fn delete_in_object_mode_removes_the_objects() {
    let mut e = editing();
    assert!(e.add_primitive(cs_3d::Primitive::Cube));
    assert_eq!(model(&e).objects.len(), 2);
    e.select_model_object(Some(0));
    let steps = e.history.done_count();
    assert!(e.delete_model_selection());
    assert_eq!(e.history.done_count(), steps + 1);
    assert_eq!(model(&e).objects.len(), 1);
    assert_eq!(e.selected_object(), Some(0), "the panel keeps an object");
    e.undo();
    assert_eq!(model(&e).objects.len(), 2);
}

#[test]
fn nothing_runs_during_a_gesture() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    e.start_model_op(EditOp::Move {
        map: front(),
        from: (100.0, 100.0),
        grab: [0.0; 3],
        keyboard: true,
    });
    assert!(!e.can_extrude());
    assert!(!e.delete_model_selection());
    assert!(!e.subdivide_model_selection());
}

#[test]
fn the_selection_does_not_change_during_an_extrude() {
    let mut e = editing();
    assert!(e.add_primitive(cs_3d::Primitive::Cube));
    let top = top_face(&e);
    faces(&mut e, &[top]);
    assert!(e.start_extrude(GizmoView::Ortho(front()), (100.0, 100.0)));
    let during = e.model_edit().selection.clone();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Face,
        items: [Item::Face(0, 0)].into(),
    });
    e.select_model_object(Some(1));
    assert_eq!(e.model_edit().selection, during);
    assert!(matches!(e.model_edit().op, Some(EditOp::Extrude { .. })));
    e.update_model_op((100.0, 68.0), false, false);
    e.finish_model_op();
    assert_eq!(e.history.done().last(), Some("Extrude"));
    assert_eq!(face_ys(&e, top), vec![32; 4]);
}

#[test]
fn arrows_do_not_nudge_during_an_extrude() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    assert!(e.start_extrude(GizmoView::Ortho(front()), (100.0, 100.0)));
    let shown = e.edit_model_shown().cloned();
    let steps = e.history.done_count();
    assert!(!e.nudge_model_selection(1, 0));
    assert_eq!(e.history.done_count(), steps);
    assert_eq!(e.edit_model_shown().cloned(), shown);
}

#[test]
fn undoing_a_topology_change_brings_back_the_selection_of_that_topology() {
    let mut e = editing();
    let top = top_face(&e);
    faces(&mut e, &[top]);
    let before = e.model_edit().selection.clone();
    assert!(e.subdivide_model_selection());
    let after = e.model_edit().selection.clone();
    assert_eq!(after.items.len(), 4);
    // Deshacer vuelve a la cara de antes; rehacer, a las cuatro (las dos, de su topología).
    e.undo();
    assert_eq!(e.model_edit().selection, before);
    e.redo();
    assert_eq!(e.model_edit().selection, after);

    let mut e = editing();
    let verts = model(&e).objects[0].mesh.faces[top].verts.clone();
    vertices(&mut e, &verts);
    let before = e.model_edit().selection.clone();
    assert!(e.merge_model_selection());
    assert_eq!(e.model_edit().selection.items.len(), 1);
    e.undo();
    assert_eq!(e.model_edit().selection, before);
    // Mover (sin cambiar la topología) y deshacer deja la selección.
    vertices(&mut e, &[0, 1]);
    e.nudge_model_selection(1, 0);
    e.undo();
    assert_eq!(e.model_edit().selection.items.len(), 2);
}

/// Lo de las vistas con el cubo (0) oculto o bloqueado y otro cubo (1) libre.
fn with_a_second_cube() -> Editor {
    let mut e = editing();
    assert!(e.add_primitive(cs_3d::Primitive::Cube));
    e
}

#[test]
fn hidden_or_locked_parts_are_not_edited_from_the_views() {
    use super::Row;
    let can = |e: &Editor| {
        (
            e.can_extrude(),
            e.can_subdivide(),
            e.can_merge(),
            e.can_delete_model_selection(),
        )
    };
    for hide in [false, true] {
        let mut e = with_a_second_cube();
        let toggled = if hide {
            e.toggle_hidden(Row::Object(0))
        } else {
            e.toggle_row_locked(Row::Object(0))
        };
        assert!(toggled);
        let top = top_face(&e);
        faces(&mut e, &[top]);
        assert_eq!(can(&e), (false, false, false, false), "hide {hide}");
        let steps = e.history.done_count();
        assert!(!e.subdivide_model_selection());
        assert!(!e.merge_model_selection());
        assert!(!e.delete_model_selection());
        assert!(!e.extrude_moved([0.0, 16.0, 0.0]));
        e.request_extrude();
        assert!(e.model_edit().key_request.is_none());
        assert_eq!(e.history.done_count(), steps);
        // Con una cara del otro cubo también: solo se toca esa.
        let mut sel = e.model_edit().selection.clone();
        sel.items.insert(Item::Face(1, top));
        e.set_model_selection(sel);
        assert!(e.can_subdivide());
        let before = model(&e).objects[0].mesh.clone();
        assert!(e.subdivide_model_selection());
        assert_eq!(model(&e).objects[0].mesh, before, "hide {hide}");
        assert!(model(&e).objects[1].mesh.faces.len() > 6);
        // En modo Object, Supr no borra el oculto o bloqueado.
        e.set_model_selection(Selection {
            mode: SelectMode::Object,
            items: [Item::Object(0), Item::Object(1)].into_iter().collect(),
        });
        assert!(e.delete_model_selection());
        assert_eq!(model(&e).objects.len(), 1, "hide {hide}");
        let left = &model(&e).objects[0];
        assert!(left.hidden || left.locked, "hide {hide}");
    }
}

#[test]
fn merge_skips_the_vertices_of_a_locked_part() {
    use super::Row;
    let mut e = with_a_second_cube();
    assert!(e.toggle_row_locked(Row::Object(0)));
    vertices(&mut e, &[0, 1]);
    assert!(!e.can_merge());
    assert!(!e.merge_model_selection());
}
