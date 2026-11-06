use cs_3d::{Item, OrthoView, SelectMode, Selection, ViewMap};
use cs_core::{ColorMode, Sprite};

use super::{EditOp, Editor};

fn editing() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    // Un modelo nuevo se abre en su workspace, siempre en modo Edit.
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

fn vertex(e: &Editor, v: usize) -> cs_core::Vec3i {
    e.model().unwrap().objects[0].mesh.vertices[v]
}

#[test]
fn edit_esta_prendido_solo_en_el_workspace() {
    let mut e = editing();
    assert!(e.model_edit_on());
    // Encuadra el cubo (±16) en el centro.
    assert_eq!(e.model_edit().center, [0.0; 3]);
    assert!(e.model_edit().zoom > 0.0);
    e.exit_workspace();
    assert!(!e.model_edit_on(), "not on the canvas");
}

#[test]
fn un_modelo_se_abre_en_edge_y_sin_seleccion() {
    let e = editing();
    assert_eq!(e.model_edit().selection.mode, SelectMode::Edge);
    assert!(e.model_edit().selection.is_empty());
}

#[test]
fn de_objeto_a_un_subelemento_empieza_sin_seleccion() {
    let mut e = editing();
    e.set_model_selection(Selection {
        mode: SelectMode::Object,
        items: [Item::Object(0)].into(),
    });
    for mode in [SelectMode::Vertex, SelectMode::Edge, SelectMode::Face] {
        e.set_model_selection(Selection {
            mode: SelectMode::Object,
            items: [Item::Object(0)].into(),
        });
        e.set_select_mode(mode);
        assert_eq!(e.model_edit().selection.mode, mode);
        assert!(e.model_edit().selection.is_empty(), "{mode:?}");
    }
    // Entre subelementos se convierte, y de ahi a Object tambien.
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    });
    e.set_select_mode(SelectMode::Object);
    assert_eq!(e.model_edit().selection.items, [Item::Object(0)].into());
}

#[test]
fn elegir_un_objeto_pasa_a_modo_objeto() {
    let mut e = editing();
    assert_eq!(e.model_edit().selection.mode, SelectMode::Edge);
    e.select_model_object(Some(0));
    assert_eq!(e.model_edit().selection.mode, SelectMode::Object);
    assert_eq!(e.model_edit().selection.items, [Item::Object(0)].into());
    assert_eq!(e.selected_object(), Some(0));
    // Ya en modo Object, se queda ahi.
    e.select_model_object(Some(0));
    assert_eq!(e.model_edit().selection.mode, SelectMode::Object);
    // Ninguno: modo Object y vacio.
    e.select_model_object(None);
    assert!(e.model_edit().selection.is_empty());
}

#[test]
fn clic_sin_arrastrar_en_lo_ya_elegido_lo_deja_solo() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 3), Item::Vertex(0, 7)].into(),
    });
    let steps = e.history.done_count();
    let move_from = |e: &mut Editor| {
        e.start_model_op(EditOp::Move {
            map: front(),
            from: (132.0, 68.0),
            grab: [16.0, 16.0, 16.0],
            keyboard: false,
        });
        e.model_edit.click_item = Some(Item::Vertex(0, 7));
    };
    // Un arrastre mueve todo lo elegido y no cambia la seleccion.
    move_from(&mut e);
    e.update_model_op((148.0, 68.0), false, false);
    e.finish_model_op();
    assert_eq!(e.model_edit().selection.items.len(), 2);
    assert_eq!(e.history.done_count(), steps + 1);
    // Un clic (sin pasar el umbral) deja solo ese item.
    move_from(&mut e);
    e.update_model_op((133.0, 68.0), false, false);
    e.finish_model_op();
    assert_eq!(e.model_edit().selection.items, [Item::Vertex(0, 7)].into());
    assert_eq!(e.history.done_count(), steps + 1);
    // El marcador no sobrevive al gesto.
    assert!(e.model_edit().click_item.is_none());
}

#[test]
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
fn elegir_un_objeto_fuera_del_workspace_no_toca_la_seleccion_de_edit() {
    let mut e = editing();
    let ship = e.active;
    e.set_select_mode(SelectMode::Vertex);
    let picked = Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    };
    e.set_model_selection(picked.clone());
    e.exit_workspace();
    assert!(!e.model_edit_on());
    e.select_model_object(Some(0));
    assert!(e.enter_workspace(ship));
    assert_eq!(e.model_edit().selection, picked);
}

#[test]
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

fn vertices(items: &[u32]) -> Selection {
    Selection {
        mode: SelectMode::Vertex,
        items: items.iter().map(|&v| Item::Vertex(0, v)).collect(),
    }
}

#[test]
fn deshacer_y_rehacer_devuelven_la_seleccion_3d() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(vertices(&[7]));
    assert!(e.nudge_model_selection(1, 0));
    // Después del paso se elige otra cosa: deshacer vuelve a lo de antes del paso…
    e.set_model_selection(vertices(&[2, 3]));
    e.undo();
    assert_eq!(e.model_edit().selection, vertices(&[7]));
    // …y rehacer, a lo que había al deshacer.
    e.redo();
    assert_eq!(e.model_edit().selection, vertices(&[2, 3]));
}

#[test]
fn deshacer_un_delete_devuelve_las_caras_borradas_elegidas() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Face);
    let faces = Selection {
        mode: SelectMode::Face,
        items: [Item::Face(0, 0)].into(),
    };
    e.set_model_selection(faces.clone());
    assert!(e.delete_model_selection());
    assert!(e.model_edit().selection.is_empty());
    e.undo();
    assert_eq!(e.model_edit().selection, faces);
}

#[test]
fn un_paso_nuevo_despues_de_deshacer_olvida_la_seleccion_de_rehacer() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(vertices(&[7]));
    assert!(e.nudge_model_selection(1, 0));
    e.undo();
    e.set_model_selection(vertices(&[1]));
    assert!(e.nudge_model_selection(0, 1));
    assert_eq!(e.step_selections.len(), e.history.done_count());
    e.undo();
    assert_eq!(e.model_edit().selection, vertices(&[1]));
}
