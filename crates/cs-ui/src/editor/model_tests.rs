use cs_3d::Primitive;
use cs_core::{ColorMode, FontMode, Rgba, Sprite};

use super::Editor;

fn editor_with_model() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_model("Ship", (64, 64)));
    e
}

#[test]
fn a_new_model_is_active_with_its_cube_selected() {
    let e = editor_with_model();
    assert_eq!(e.document.artboards.len(), 3);
    assert!(e.is_model());
    assert_eq!(e.model().unwrap().objects.len(), 1);
    assert_eq!(e.selected_object(), Some(0));
    assert!(e.model().unwrap().texture.is_some());
}

#[test]
fn adding_a_primitive_selects_it_and_undo_removes_it() {
    let mut e = editor_with_model();
    assert!(e.add_primitive(Primitive::Cylinder { sides: 8 }));
    assert_eq!(e.model().unwrap().objects.len(), 2);
    assert_eq!(e.selected_object(), Some(1));
    e.undo();
    assert_eq!(e.model().unwrap().objects.len(), 1);
}

#[test]
fn removing_and_setting_objects() {
    let mut e = editor_with_model();
    e.add_primitive(Primitive::Plane);
    let mut obj = e.model().unwrap().objects[0].clone();
    obj.name = "Hull".into();
    assert!(e.set_object(0, obj));
    assert_eq!(e.model().unwrap().objects[0].name, "Hull");
    assert!(e.remove_object(1));
    assert!(!e.remove_object(5));
    assert_eq!(e.model().unwrap().objects.len(), 1);
    assert_eq!(e.selected_object(), Some(0));
}

#[test]
fn camera_light_outline_and_texture_undo() {
    let mut e = editor_with_model();
    let mut cam = e.model().unwrap().camera;
    cam.distance += 100;
    let before = e.model().unwrap().camera;
    e.set_camera(cam);
    assert_eq!(e.model().unwrap().camera, cam);
    e.undo();
    assert_eq!(e.model().unwrap().camera, before);
    let c = Rgba::rgb(1, 2, 3);
    e.set_outline(Some(c));
    assert_eq!(e.model().unwrap().outline, Some(c));
    e.set_texture(None);
    assert_eq!(e.model().unwrap().texture, None);
    let l = e.model().unwrap().light;
    e.set_light(l);
}

#[test]
fn grouped_model_changes_are_one_undo_step() {
    let mut e = editor_with_model();
    let before = e.model().unwrap().clone();
    for i in 1..4 {
        let mut m = e.model().unwrap().clone();
        m.camera.distance += i;
        assert!(e.set_model_grouped(m, eframe::egui::Id::new("cam")));
    }
    e.end_group();
    e.undo();
    assert_eq!(e.model().unwrap(), &before);
}

#[test]
fn painting_on_a_model_does_nothing() {
    let mut e = editor_with_model();
    e.tool = crate::ToolKind::Pencil;
    let doc = e.document.clone();
    let pos = e.document.artboard(e.active).unwrap().position;
    e.pointer_press((pos.0 as f32 + 3.5, pos.1 as f32 + 3.5), false, false, 2.0);
    e.pointer_drag((pos.0 as f32 + 6.5, pos.1 as f32 + 6.5));
    e.pointer_release();
    assert_eq!(e.document, doc);
    e.undo();
    assert_eq!(e.document.artboards.len(), 1);
}

#[test]
fn fonts_and_models_lock_the_structure() {
    let e = editor_with_model();
    assert!(e.is_structure_locked());
    assert!(!e.is_font());
    assert!(!e.can_resize_sprite());
    let mut f = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(!f.is_structure_locked());
    f.new_font("F", (8, 8), FontMode::OneBit);
    assert!(f.is_structure_locked());
}
