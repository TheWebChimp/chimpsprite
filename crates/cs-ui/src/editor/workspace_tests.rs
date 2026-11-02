use cs_3d::{Item, OrthoView, SelectMode, Selection, ViewMap};
use cs_core::{ColorMode, FontMode, Sprite};

use super::{EditOp, Editor, KeyOp};

fn editor() -> Editor {
    Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba))
}

fn front() -> ViewMap {
    ViewMap {
        view: OrthoView::Front,
        center: [0.0; 3],
        scale: 2.0,
        origin: (100.0, 100.0),
    }
}

#[test]
fn entrar_y_salir_del_workspace_de_un_modelo() {
    let mut e = editor();
    let home = e.active;
    assert_eq!(e.workspace(), None);
    // Un modelo nuevo abre su workspace.
    assert!(e.new_model("Ship", (64, 64)));
    let ship = e.active;
    assert_ne!(ship, home);
    assert_eq!(e.workspace(), Some(ship));
    assert!(e.is_model());
    assert!(e.model_edit_on(), "the workspace is always in Edit mode");
    assert!(
        e.model_edit().fit,
        "the views fit the model when the area is known"
    );
    e.exit_workspace();
    assert_eq!(e.workspace(), None);
    assert_eq!(e.active, home, "back to the canvas artboard");
    assert!(!e.model_edit_on());
    assert!(e.enter_workspace(ship));
    assert_eq!((e.workspace(), e.active), (Some(ship), ship));
    assert!(e.model_edit_on());
}

#[test]
fn la_fuente_tambien_tiene_workspace() {
    let mut e = editor();
    let home = e.active;
    assert!(e.new_font("Mine", (8, 8), FontMode::OneBit));
    let font = e.active;
    assert_eq!(e.workspace(), Some(font));
    assert!(e.is_font());
    assert!(!e.model_edit_on());
    e.exit_workspace();
    assert_eq!(e.active, home);
}

#[test]
fn un_artboard_normal_no_tiene_workspace() {
    let mut e = editor();
    let home = e.active;
    assert!(!e.enter_workspace(home));
    assert_eq!(e.workspace(), None);
    assert!(!e.enter_workspace(cs_core::ArtboardId(999)));
    assert_eq!(e.active, home);
}

#[test]
fn deshacer_el_modelo_saca_del_workspace() {
    let mut e = editor();
    let home = e.active;
    assert!(e.new_model("Ship", (64, 64)));
    e.undo();
    assert_eq!(e.workspace(), None);
    assert_eq!(e.active, home);
    assert!(!e.model_edit_on());
}

#[test]
fn activar_un_artboard_del_lienzo_sale_del_workspace() {
    let mut e = editor();
    let home = e.active;
    assert!(e.new_model("Ship", (64, 64)));
    e.activate(home);
    assert_eq!(e.workspace(), None);
    assert_eq!(e.active, home);
}

#[test]
fn un_gesto_en_el_workspace_se_deshace_desde_el_lienzo() {
    let mut e = editor();
    assert!(e.new_model("Ship", (64, 64)));
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    });
    let ship = e.active;
    let vertex = |e: &Editor| {
        e.document
            .artboard(ship)
            .unwrap()
            .model
            .as_ref()
            .unwrap()
            .objects[0]
            .mesh
            .vertices[7]
    };
    let before = vertex(&e);
    let steps = e.history.done_count();
    e.start_model_op(EditOp::Move {
        map: front(),
        from: (132.0, 68.0),
        grab: [16.0, 16.0, 16.0],
        keyboard: false,
    });
    e.update_model_op((148.0, 68.0), false, false);
    e.finish_model_op();
    assert_eq!(vertex(&e).x, 24);
    assert_eq!(e.history.done_count(), steps + 1);
    // Mismo historial: se deshace desde el lienzo.
    e.exit_workspace();
    e.undo();
    assert_eq!(vertex(&e), before);
    assert_eq!(e.workspace(), None);
}

#[test]
fn entrar_a_otro_modelo_reinicia_el_modo_edit() {
    let mut e = editor();
    assert!(e.new_model("Ship", (64, 64)));
    let ship = e.active;
    e.set_model_grid(8);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    });
    // Volver al mismo modelo conserva la selección.
    e.exit_workspace();
    assert!(e.enter_workspace(ship));
    assert_eq!(e.model_edit().selection.items.len(), 1);
    assert_eq!(e.model_edit().grid, 8);
    // Otro modelo empieza vacío y con la cuadrícula de siempre.
    assert!(e.new_model("Other", (64, 64)));
    assert!(e.model_edit_on());
    assert!(e.model_edit().selection.is_empty());
    assert_eq!(e.model_edit().grid, super::model_edit::DEFAULT_GRID);
}

#[test]
fn salir_del_workspace_de_otro_regresa_al_lienzo_de_antes() {
    let mut e = editor();
    let home = e.active;
    assert!(e.new_model("Ship", (64, 64)));
    assert!(e.new_font("Mine", (8, 8), FontMode::OneBit));
    e.exit_workspace();
    assert_eq!(
        e.active, home,
        "the canvas artboard from before the first workspace"
    );
}

#[test]
fn encuadrar_las_vistas_con_el_area() {
    let mut e = editor();
    assert!(e.new_model("Ship", (64, 64)));
    // El cubo (±16) en un cuadrante de 100 puntos.
    e.fit_model_views(100.0);
    assert!(!e.model_edit().fit);
    assert_eq!(e.model_edit().center, [0.0; 3]);
    let z = e.model_edit().zoom;
    assert!(z > 1.0 && 32.0 * z <= 100.0, "zoom {z}");
}

#[test]
fn salir_del_workspace_cancela_el_gesto_del_modelo() {
    let mut e = editor();
    assert!(e.new_model("Ship", (64, 64)));
    e.set_model_selection(Selection {
        mode: SelectMode::Object,
        items: [Item::Object(0)].into(),
    });
    // Una G de teclado que sigue al puntero, con su vista previa.
    e.start_model_op(EditOp::Move {
        map: front(),
        from: (100.0, 100.0),
        grab: [0.0; 3],
        keyboard: true,
    });
    e.update_model_op((140.0, 100.0), false, false);
    assert!(e.model_edit().preview.is_some());
    let steps = e.history.done_count();
    // Y otra G pedida que el lienzo todavía no empezó.
    e.request_model_key_op(KeyOp::Grab);
    assert!(e.model_edit().key_request.is_some());
    e.exit_workspace();
    assert!(e.model_edit().op.is_none(), "the grab keeps running");
    assert!(e.model_edit().preview.is_none());
    assert!(
        e.model_edit().key_request.is_none(),
        "the requested G waits"
    );
    assert_eq!(e.history.done_count(), steps, "nothing committed");
}
