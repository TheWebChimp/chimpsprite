use cs_3d::{Item, OrthoView, SelectMode, Selection, ViewMap};
use cs_core::{ColorMode, Sprite};

use super::{EditOp, Editor};

fn editing() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_model("Ship", (64, 64)));
    e.toggle_model_edit();
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

fn vertex(e: &Editor, v: usize) -> cs_core::Vec3i {
    e.model().unwrap().objects[0].mesh.vertices[v]
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn edit_se_prende_solo_en_el_modelo_activo() {
    let mut e = editing();
    assert!(e.model_edit_on());
    // Encuadra el cubo (±16) en el centro.
    assert_eq!(e.model_edit().center, [0.0; 3]);
    assert!(e.model_edit().zoom > 0.0);
    let plain = e.document.artboards[0].id;
    e.activate(plain);
    assert!(!e.model_edit_on());
    e.toggle_model_edit();
    assert!(!e.model_edit_on(), "not on a plain artboard");
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn cambiar_de_modo_convierte_y_sincroniza_el_objeto() {
    let mut e = editing();
    e.set_model_selection(Selection {
        mode: SelectMode::Object,
        items: [Item::Object(0)].into(),
    });
    e.set_select_mode(SelectMode::Vertex);
    assert_eq!(e.model_edit().selection.items.len(), 8);
    assert_eq!(e.selected_object(), Some(0));
    e.select_model_object(Some(0));
    assert_eq!(e.model_edit().selection.mode, SelectMode::Vertex);
    assert_eq!(e.model_edit().selection.items.len(), 8);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn mover_es_un_paso_de_undo_y_cancelar_ninguno() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    });
    let before = vertex(&e, 7);
    let undo_steps = e.history.done_count();
    e.start_model_op(EditOp::Move {
        map: front(),
        from: (132.0, 68.0),
        grab: [16.0, 16.0, 16.0],
        keyboard: false,
    });
    // 16 puntos a la derecha = 8/16: con la cuadrícula de 4, x = 24.
    e.update_model_op((148.0, 68.0), false, false);
    assert_eq!(vertex(&e, 7), before, "the document waits for the release");
    assert_eq!(
        e.edit_model_shown().unwrap().objects[0].mesh.vertices[7].x,
        24
    );
    e.finish_model_op();
    assert_eq!(vertex(&e, 7).x, 24);
    assert_eq!(e.history.done_count(), undo_steps + 1);
    e.undo();
    assert_eq!(vertex(&e, 7), before);
    // Cancelar no deja paso.
    e.start_model_op(EditOp::Scale {
        map: front(),
        center: [0.0; 3],
        from_dist: 10.0,
    });
    e.update_model_op((120.0, 100.0), false, false);
    assert!(e.cancel_model_op());
    assert_eq!(vertex(&e, 7), before);
    assert_eq!(e.history.done_count(), undo_steps);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn la_caja_elige_y_con_add_agrega() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.start_model_op(EditOp::Box {
        map: front(),
        from: (120.0, 60.0),
        to: (120.0, 60.0),
        add: false,
    });
    e.update_model_op((140.0, 80.0), false, false);
    e.finish_model_op();
    assert_eq!(
        e.model_edit().selection.items,
        [Item::Vertex(0, 3), Item::Vertex(0, 7)].into()
    );
    // Un clic en vacío (caja de 0) sin add limpia.
    e.start_model_op(EditOp::Box {
        map: front(),
        from: (10.0, 10.0),
        to: (10.0, 10.0),
        add: false,
    });
    e.finish_model_op();
    assert!(e.model_edit().selection.is_empty());
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn flechas_y_pan_y_zoom() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    });
    let before = vertex(&e, 7);
    assert!(e.nudge_model_selection(1, 0));
    assert_eq!(vertex(&e, 7).x, before.x + 4, "one grid step in Front");
    e.start_model_op(EditOp::Pan {
        map: front(),
        from: (100.0, 100.0),
        center: [0.0; 3],
    });
    e.update_model_op((110.0, 100.0), false, false);
    e.finish_model_op();
    assert_eq!(
        e.model_edit().center,
        [-5.0, 0.0, 0.0],
        "the view follows the drag"
    );
    let zoom = e.model_edit().zoom;
    e.zoom_model_views(&front(), (100.0, 100.0), 1);
    assert!(e.model_edit().zoom > zoom);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn deshacer_quita_de_la_seleccion_lo_que_ya_no_existe() {
    let mut e = editing();
    assert!(e.add_primitive(cs_3d::Primitive::Plane));
    e.set_model_selection(Selection {
        mode: SelectMode::Object,
        items: [Item::Object(1)].into(),
    });
    e.undo();
    assert!(e.model_edit().selection.is_empty());
}

#[test]
fn el_estado_de_la_ui_de_edit_no_cambia_la_revision() {
    let mut e = editing();
    let revision = e.revision;
    e.start_model_op(EditOp::Pan {
        map: front(),
        from: (100.0, 100.0),
        center: [0.0; 3],
    });
    for x in [105.0, 110.0, 120.0] {
        e.update_model_op((x, 100.0), false, false);
    }
    e.finish_model_op();
    e.start_model_op(EditOp::Box {
        map: front(),
        from: (120.0, 60.0),
        to: (120.0, 60.0),
        add: false,
    });
    e.update_model_op((140.0, 80.0), false, false);
    e.finish_model_op();
    e.zoom_model_views(&front(), (100.0, 100.0), 1);
    e.set_model_grid(8);
    e.set_select_mode(SelectMode::Vertex);
    assert!(!e.cancel_model_op());
    assert_eq!(e.revision, revision);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn elegir_un_objeto_de_otro_modelo_no_toca_la_seleccion_de_edit() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    let picked = Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    };
    e.set_model_selection(picked.clone());
    assert!(e.new_model("Other", (64, 64)));
    assert!(!e.model_edit_on());
    e.select_model_object(Some(0));
    assert_eq!(e.model_edit().selection, picked);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn deshacer_a_media_mano_cancela_el_gesto() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    });
    let before = vertex(&e, 7);
    let steps = e.history.done_count();
    assert!(e.nudge_model_selection(1, 0));
    e.start_model_op(EditOp::Move {
        map: front(),
        from: (140.0, 68.0),
        grab: [20.0, 16.0, 16.0],
        keyboard: false,
    });
    e.update_model_op((156.0, 68.0), false, false);
    e.undo();
    assert!(e.model_edit().op.is_none() && e.model_edit().preview.is_none());
    e.finish_model_op();
    assert_eq!(vertex(&e, 7), before, "the undo stays");
    assert_eq!(e.history.done_count(), steps);
}
