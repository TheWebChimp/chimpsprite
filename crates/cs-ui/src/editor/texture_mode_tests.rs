use cs_3d::{Item, SelectMode, Selection};
use cs_core::{ArtboardId, ColorMode, LayerId, PixelValue, Rgba, Sprite};

use super::model_edit::UvOp;
use super::{CadMode, Editor, ToolKind};

/// Un modelo nuevo (con su textura de 64×64) en su workspace.
fn editing() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_model("Ship", (64, 64)));
    e
}

fn texture_of(e: &Editor) -> ArtboardId {
    e.workspace_model()
        .unwrap()
        .texture
        .expect("a new model has a texture")
}

/// En modo Texture con la cara 0 del cubo elegida.
fn texturing() -> Editor {
    let mut e = editing();
    e.set_cad_mode(CadMode::Texture);
    e.set_model_selection(Selection {
        mode: SelectMode::Face,
        items: [Item::Face(0, 0)].into(),
    });
    e
}

fn face_uvs(e: &Editor, f: usize) -> Vec<(i32, i32)> {
    e.workspace_model().unwrap().objects[0].mesh.faces[f]
        .uvs
        .clone()
}

#[test]
fn cambiar_de_modo_no_toca_el_documento_ni_el_undo() {
    let mut e = editing();
    let steps = e.history.done_count();
    let version = e.doc_version;
    assert_eq!(e.cad_mode(), CadMode::Model);
    e.set_cad_mode(CadMode::Texture);
    assert_eq!(e.cad_mode(), CadMode::Texture);
    e.set_cad_mode(CadMode::Model);
    assert_eq!(e.history.done_count(), steps, "no undo steps");
    assert_eq!(e.doc_version, version, "the document is untouched");
}

#[test]
fn texture_activa_la_textura_y_la_seleccion_pasa_a_caras() {
    let mut e = editing();
    let ship = e.active;
    let texture = texture_of(&e);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 0)].into(),
    });
    e.set_cad_mode(CadMode::Texture);
    assert_eq!(e.active, texture, "the tools paint on the texture");
    assert_eq!(
        e.workspace(),
        Some(ship),
        "the workspace is still the model"
    );
    assert!(e.model_edit_on(), "still the model's tab");
    assert!(!e.is_model(), "the active is a plain artboard");
    assert!(e.single_artboard(), "only the texture is seen and touched");
    assert_eq!(e.workspace_model().map(|m| m.objects.len()), Some(1));
    assert_eq!(e.model_edit().selection.mode, SelectMode::Face);
    // Volver a Model activa otra vez el modelo; la selección de caras se queda.
    e.set_cad_mode(CadMode::Model);
    assert_eq!(e.active, ship);
    assert!(e.model_edit_on());
    assert_eq!(e.model_edit().selection.mode, SelectMode::Face);
}

#[test]
fn en_texture_se_pinta_en_la_textura() {
    let mut e = editing();
    e.set_cad_mode(CadMode::Texture);
    let texture = texture_of(&e);
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    e.primary = red;
    e.tool = ToolKind::Pencil;
    let (x, y) = e.artboard().position;
    e.pointer_press((x as f32 + 3.5, y as f32 + 2.5), false, false, 0.1);
    e.pointer_release();
    let sprite = &e.document.artboard(texture).unwrap().sprite;
    let layer = sprite.layers.last().unwrap().id;
    assert_eq!(sprite.pixel(layer, 0, 3, 2), red);
    assert_eq!(e.active, texture, "still on the texture");
    assert_eq!(e.workspace(), Some(e.model_id()));
}

#[test]
fn las_funciones_del_modelo_usan_el_del_workspace_en_texture() {
    let mut e = texturing();
    let ship = e.workspace().unwrap();
    assert_eq!(e.model_id(), ship);
    e.select_model_object(None);
    assert_eq!(e.selected_object(), None);
    e.select_model_object(Some(0));
    assert_eq!(e.selected_object(), Some(0));
    // En Texture la selección se queda en caras (elegir el objeto en el panel no la cambia).
    assert_eq!(e.model_edit().selection.mode, SelectMode::Face);
    assert_eq!(e.selected_faces(), vec![(0, 0)]);
    // Un cambio del modelo (el panel) va al artboard del modelo, no a la textura.
    let mut camera = e.workspace_model().unwrap().camera;
    camera.distance += 10;
    assert!(e.set_camera(camera));
    assert_eq!(e.workspace_model().unwrap().camera, camera);
    assert!(e.document.artboard(e.active).unwrap().model.is_none());
    assert_eq!(e.model_texture_frames(), 1);
    assert_eq!(
        e.cad_mode(),
        CadMode::Texture,
        "a model change keeps the mode"
    );
}

#[test]
fn selected_faces_son_las_caras_elegidas() {
    let e = texturing();
    assert_eq!(e.selected_faces(), vec![(0, 0)]);
}

#[test]
fn quitar_un_objeto_deja_las_caras_elegidas_en_la_misma_cara() {
    let mut e = editing();
    e.add_primitive(cs_3d::Primitive::Plane);
    e.add_primitive(cs_3d::Primitive::Plane);
    e.set_cad_mode(CadMode::Texture);
    e.set_model_selection(Selection {
        mode: SelectMode::Face,
        items: [Item::Face(2, 0)].into(),
    });
    let face = e.workspace_model().unwrap().objects[2].mesh.faces[0].clone();
    assert!(e.remove_object(0));
    assert_eq!(e.selected_faces(), vec![(1, 0)]);
    assert_eq!(e.workspace_model().unwrap().objects[1].mesh.faces[0], face);
}

#[test]
fn mover_uvs_es_un_paso_de_undo_y_cancelar_ninguno() {
    let mut e = texturing();
    let before = face_uvs(&e, 0);
    let steps = e.history.done_count();
    // Cancelar: el documento no cambia.
    e.start_uv_op(UvOp::Move { from: (5.0, 5.0) });
    e.update_uv_op((9.0, 7.0));
    assert_ne!(
        e.edit_model_shown().unwrap().objects[0].mesh.faces[0].uvs,
        before,
        "the preview moved"
    );
    assert_eq!(face_uvs(&e, 0), before, "only the preview");
    assert!(e.cancel_uv_op());
    assert_eq!(face_uvs(&e, 0), before);
    assert_eq!(e.history.done_count(), steps, "cancel leaves no step");
    // Soltar: un paso de undo.
    e.start_uv_op(UvOp::Move { from: (5.0, 5.0) });
    e.update_uv_op((9.2, 6.6));
    e.update_uv_op((8.0, 3.0));
    e.finish_uv_op();
    let moved: Vec<_> = before.iter().map(|&(x, y)| (x + 3, y - 2)).collect();
    assert_eq!(face_uvs(&e, 0), moved);
    assert_eq!(e.history.done_count(), steps + 1);
    // Un clic sin arrastre no deja paso.
    e.start_uv_op(UvOp::Move { from: (5.0, 5.0) });
    e.update_uv_op((5.2, 5.3));
    e.finish_uv_op();
    assert_eq!(e.history.done_count(), steps + 1);
    e.undo();
    assert_eq!(face_uvs(&e, 0), before);
    assert_eq!(e.cad_mode(), CadMode::Texture);
    assert_eq!(e.active, texture_of(&e));
}

#[test]
fn mover_un_vertice_uv_cambia_solo_ese() {
    let mut e = texturing();
    let before = face_uvs(&e, 0);
    let steps = e.history.done_count();
    let at = |k: usize| (before[k].0 as f32, before[k].1 as f32);
    e.start_uv_op(UvOp::Vertex {
        face: (0, 0),
        vertex: 1,
        from: at(1),
    });
    e.update_uv_op((at(1).0 + 2.4, at(1).1 - 1.0));
    e.finish_uv_op();
    let uvs = face_uvs(&e, 0);
    assert_eq!(uvs[1], (before[1].0 + 2, before[1].1 - 1));
    for k in [0, 2, 3] {
        assert_eq!(uvs[k], before[k], "vertex {k}");
    }
    assert_eq!(e.history.done_count(), steps + 1);
    e.undo();
    assert_eq!(face_uvs(&e, 0), before);
}

#[test]
fn girar_y_voltear_las_uvs_un_paso_cada_uno() {
    let mut e = texturing();
    let before = face_uvs(&e, 0);
    let steps = e.history.done_count();
    assert!(e.rotate_selected_uvs());
    assert_eq!(e.history.done_count(), steps + 1);
    for _ in 0..3 {
        assert!(e.rotate_selected_uvs());
    }
    assert_eq!(face_uvs(&e, 0), before, "four quarter turns");
    assert!(e.flip_selected_uvs(true));
    assert_ne!(face_uvs(&e, 0), before);
    assert!(e.flip_selected_uvs(true));
    assert!(e.flip_selected_uvs(false));
    assert!(e.flip_selected_uvs(false));
    assert_eq!(face_uvs(&e, 0), before);
    assert_eq!(e.history.done_count(), steps + 8);
    // Sin caras elegidas no hacen nada.
    e.set_model_selection(Selection {
        mode: SelectMode::Face,
        items: Default::default(),
    });
    assert!(!e.rotate_selected_uvs());
    assert!(!e.flip_selected_uvs(true));
}

#[test]
fn texture_size_agranda_sin_perder_pixeles_ni_uvs() {
    let mut e = texturing();
    let texture = texture_of(&e);
    let uvs = face_uvs(&e, 0);
    let sprite = &e.document.artboard(texture).unwrap().sprite;
    assert_eq!((sprite.width, sprite.height), (64, 64));
    let layer: LayerId = sprite.layers.last().unwrap().id;
    let probe = [(0, 0), (17, 33), (63, 63)].map(|(x, y)| sprite.pixel(layer, 0, x, y));
    let steps = e.history.done_count();
    assert!(e.set_texture_size(128));
    let sprite = &e.document.artboard(texture).unwrap().sprite;
    assert_eq!((sprite.width, sprite.height), (128, 128));
    let after = [(0, 0), (17, 33), (63, 63)].map(|(x, y)| sprite.pixel(layer, 0, x, y));
    assert_eq!(after, probe, "anchored top-left");
    assert_eq!(
        sprite.pixel(layer, 0, 100, 100),
        ColorMode::Rgba.transparent(),
        "the new space is clear"
    );
    assert_eq!(face_uvs(&e, 0), uvs, "the UVs stay on the same pixels");
    assert_eq!(e.history.done_count(), steps + 1, "one undo step");
    assert_eq!(e.active, texture);
    // El mismo tamaño no hace nada; achicar recorta.
    assert!(!e.set_texture_size(128));
    assert!(e.set_texture_size(32));
    let sprite = &e.document.artboard(texture).unwrap().sprite;
    assert_eq!((sprite.width, sprite.height), (32, 32));
    e.undo();
    e.undo();
    let sprite = &e.document.artboard(texture).unwrap().sprite;
    assert_eq!((sprite.width, sprite.height), (64, 64));
}

#[test]
fn quitar_la_textura_por_undo_regresa_a_model() {
    let mut e = editing();
    let ship = e.active;
    assert!(e.set_texture(None));
    // Sin textura, Texture se queda en el modelo (el lienzo avisa "No texture").
    e.set_cad_mode(CadMode::Texture);
    assert_eq!(e.cad_mode(), CadMode::Texture);
    assert_eq!(e.active, ship);
    assert!(e.model_edit_on());
    // New Texture en modo Texture activa la textura nueva.
    assert!(e.new_texture());
    let texture = texture_of(&e);
    assert_eq!(e.active, texture);
    // Deshacerla regresa a Model, en el modelo.
    e.undo();
    assert_eq!(e.cad_mode(), CadMode::Model);
    assert_eq!(e.active, ship);
    assert_eq!(e.workspace(), Some(ship));
    assert!(e.model_edit_on());
}

#[test]
fn salir_o_entrar_a_otro_modelo_deja_el_modo_en_model() {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    let home = e.active;
    assert!(e.new_model("A", (32, 32)));
    let a = e.active;
    assert!(e.new_model("B", (32, 32)));
    let b = e.active;
    e.set_cad_mode(CadMode::Texture);
    // Entrar al mismo (su pestaña otra vez) conserva el modo.
    assert!(e.enter_workspace(b));
    assert_eq!(e.cad_mode(), CadMode::Texture);
    assert!(e.enter_workspace(a));
    assert_eq!(e.cad_mode(), CadMode::Model);
    assert_eq!(e.active, a);
    e.set_cad_mode(CadMode::Texture);
    e.exit_workspace();
    assert_eq!(e.cad_mode(), CadMode::Model);
    assert_eq!(e.workspace(), None);
    assert_eq!(e.active, home, "back to the canvas artboard of before");
    assert!(!e.model_edit_on());
}

#[test]
fn activar_otro_artboard_en_texture_sale_del_workspace() {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    let home = e.active;
    assert!(e.new_model("A", (32, 32)));
    e.set_cad_mode(CadMode::Texture);
    e.activate(home);
    assert_eq!(e.workspace(), None);
    assert_eq!(e.cad_mode(), CadMode::Model);
    assert!(!e.model_edit_on());
}

#[test]
fn los_atajos_de_pixeles_solo_se_bloquean_en_model() {
    let mut e = editing();
    assert!(e.pixel_actions_blocked());
    e.set_cad_mode(CadMode::Texture);
    assert!(!e.pixel_actions_blocked(), "they act on the texture");
    e.exit_workspace();
    assert!(!e.pixel_actions_blocked());
}

#[test]
fn sin_textura_los_atajos_de_pixeles_siguen_bloqueados() {
    let mut e = editing();
    assert!(e.set_texture(None));
    e.set_cad_mode(CadMode::Texture);
    // El activo sigue siendo el modelo: su sprite escondido no se toca.
    assert!(e.is_model());
    assert!(e.pixel_actions_blocked());
}

#[test]
fn en_texture_la_seleccion_se_queda_en_caras() {
    let mut e = texturing();
    for mode in [SelectMode::Object, SelectMode::Vertex, SelectMode::Edge] {
        e.set_select_mode(mode);
        assert_eq!(e.model_edit().selection.mode, SelectMode::Face, "{mode:?}");
        assert_eq!(e.selected_faces(), vec![(0, 0)]);
    }
    // En Model sí cambia.
    e.set_cad_mode(CadMode::Model);
    e.set_select_mode(SelectMode::Vertex);
    assert_eq!(e.model_edit().selection.mode, SelectMode::Vertex);
}

#[test]
fn texture_size_solo_entre_16_y_512() {
    let mut e = texturing();
    let steps = e.history.done_count();
    for side in [0, 8, 15, 513, 1024] {
        assert!(!e.set_texture_size(side), "{side}");
    }
    assert_eq!(e.history.done_count(), steps);
    assert!(e.set_texture_size(16));
    assert!(e.set_texture_size(512));
}

#[test]
fn new_texture_en_texture_conserva_el_objeto_del_panel() {
    let mut e = editing();
    assert!(e.set_texture(None));
    e.set_cad_mode(CadMode::Texture);
    e.select_model_object(Some(0));
    assert!(e.new_texture());
    assert_eq!(e.active, texture_of(&e));
    assert_eq!(e.selected_object(), Some(0));
}

/// Pinta con el lápiz el pixel `(x, y)` de la textura (el activo) y dice si cambió.
fn paints_at(e: &mut Editor, (x, y): (i32, i32)) -> bool {
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    e.primary = red;
    e.tool = ToolKind::Pencil;
    let (ax, ay) = e.artboard().position;
    e.pointer_press(
        ((ax + x) as f32 + 0.5, (ay + y) as f32 + 0.5),
        false,
        false,
        0.1,
    );
    e.pointer_release();
    let sprite = e.sprite();
    let layer = sprite.layers.last().unwrap().id;
    sprite.pixel(layer, 0, x, y) == red
}

fn face(f: usize) -> Selection {
    Selection {
        mode: SelectMode::Face,
        items: [Item::Face(0, f)].into(),
    }
}

#[test]
fn con_una_cara_elegida_solo_se_pinta_dentro_de_su_contorno() {
    let mut e = editing();
    e.set_cad_mode(CadMode::Texture);
    let steps = e.history.done_count();
    e.set_model_selection(face(0));
    // La cara de enfrente del cubo usa los pixeles 0..16 de la textura.
    let mask = e.selection.clone().expect("the face's pixels are selected");
    assert_eq!(mask.bounds(), Some(((0, 0), (15, 15))));
    assert_eq!(mask.points().count(), 256);
    assert_eq!(e.history.done_count(), steps, "selecting is not undoable");
    assert!(e.selection_is_face_mask());
    assert!(!e.shows_handles(), "no transform box on the face's pixels");
    assert!(!paints_at(&mut e, (20, 5)), "outside the face");
    assert!(paints_at(&mut e, (5, 5)), "inside the face");
    // Otra cara: la selección la sigue.
    e.set_model_selection(face(1));
    assert_eq!(
        e.selection.as_ref().and_then(|m| m.bounds()),
        Some(((16, 0), (31, 15)))
    );
    // Mover sus UVs también.
    e.start_uv_op(UvOp::Move { from: (20.0, 5.0) });
    e.update_uv_op((24.0, 7.0));
    e.finish_uv_op();
    assert_eq!(
        e.selection.as_ref().and_then(|m| m.bounds()),
        Some(((20, 2), (35, 17)))
    );
    // Deshacer el movimiento la regresa.
    e.undo();
    assert_eq!(
        e.selection.as_ref().and_then(|m| m.bounds()),
        Some(((16, 0), (31, 15)))
    );
    // Sin caras, se pinta libre.
    e.set_model_selection(Selection {
        mode: SelectMode::Face,
        items: Default::default(),
    });
    assert!(e.selection.is_none());
    assert!(paints_at(&mut e, (40, 40)));
}

#[test]
fn la_seleccion_de_la_cara_se_va_al_salir_de_texture_y_vuelve_al_entrar() {
    let mut e = texturing();
    assert!(e.selection.is_some());
    e.set_cad_mode(CadMode::Model);
    assert!(e.selection.is_none(), "Model has no pixel selection");
    e.set_cad_mode(CadMode::Texture);
    assert!(e.selection_is_face_mask(), "the face is still chosen");
    e.exit_workspace();
    assert!(e.selection.is_none());
}

#[test]
fn la_marquesina_del_usuario_gana_hasta_que_cambian_las_caras() {
    let mut e = texturing();
    // Seleccionar todo con caras elegidas: se respeta.
    e.select_all();
    assert!(!e.selection_is_face_mask());
    assert_eq!(
        e.selection.as_ref().map(|m| m.points().count()),
        Some(64 * 64)
    );
    // Pintar no la cambia.
    assert!(paints_at(&mut e, (40, 40)));
    assert_eq!(
        e.selection.as_ref().map(|m| m.points().count()),
        Some(64 * 64)
    );
    // Elegir otra cara vuelve a la de la cara.
    e.set_model_selection(face(2));
    assert!(e.selection_is_face_mask());
    // Deseleccionar: se pinta libre hasta que cambien las caras.
    e.deselect();
    assert!(e.selection.is_none());
    assert!(paints_at(&mut e, (60, 60)));
    // Al quitar las caras no se toca una marquesina del usuario.
    e.select_all();
    e.set_model_selection(Selection {
        mode: SelectMode::Face,
        items: Default::default(),
    });
    assert!(e.selection.is_some());
}

#[test]
fn texture_size_pregunta_si_alguna_uv_queda_fuera() {
    let e = texturing();
    // El cubo usa hasta el pixel 48 de la plantilla de 64.
    assert!(!e.texture_size_cuts_uvs(128), "growing never asks");
    assert!(!e.texture_size_cuts_uvs(64));
    assert!(e.texture_size_cuts_uvs(32));
    assert!(e.texture_size_cuts_uvs(16));
}

#[test]
fn una_marquesina_igual_a_la_de_la_cara_es_del_usuario() {
    let mut e = texturing();
    assert!(e.selection_is_face_mask());
    let same = e.selection.clone();
    e.set_selection(same);
    assert!(!e.selection_is_face_mask(), "the user's own, even if equal");
}

#[test]
fn volver_a_elegir_la_misma_cara_regresa_su_seleccion() {
    let mut e = editing();
    e.set_cad_mode(CadMode::Texture);
    let camera = cs_core::CameraPreset::Front.camera(4 * cs_core::FIXED, Default::default());
    assert!(e.set_camera(camera));
    let ship = e.document.artboard(e.model_id()).unwrap();
    let center = (ship.sprite.width as i32 / 2, ship.sprite.height as i32 / 2);
    e.pick_model_face(center, false);
    assert_eq!(e.selected_faces().len(), 1);
    let mask = e.selection.clone();
    assert!(e.selection_is_face_mask());
    // Ctrl+D y otra vez la misma cara: vuelve la selección de la cara.
    e.deselect();
    assert!(e.selection.is_none());
    e.pick_model_face(center, false);
    assert!(e.selection_is_face_mask());
    assert_eq!(e.selection, mask);
    // Igual después de Ctrl+A.
    e.select_all();
    assert!(!e.selection_is_face_mask());
    e.pick_model_face(center, false);
    assert!(e.selection_is_face_mask());
    assert_eq!(e.selection, mask);
}
