use cs_3d::Primitive;
use cs_core::{ColorMode, FontMode, Rgba, Sprite};

use super::Editor;

fn editor_with_model() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_model("Ship", (64, 64)));
    e
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn a_new_model_is_active_with_its_cube_selected() {
    let e = editor_with_model();
    assert_eq!(e.document.artboards.len(), 3);
    assert!(e.is_model());
    assert_eq!(e.model().unwrap().objects.len(), 1);
    assert_eq!(e.selected_object(), Some(0));
    assert!(e.model().unwrap().texture.is_some());
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn adding_a_primitive_selects_it_and_undo_removes_it() {
    let mut e = editor_with_model();
    assert!(e.add_primitive(Primitive::Cylinder { sides: 8 }));
    assert_eq!(e.model().unwrap().objects.len(), 2);
    assert_eq!(e.selected_object(), Some(1));
    e.undo();
    assert_eq!(e.model().unwrap().objects.len(), 1);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
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
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
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
    let mut l = e.model().unwrap().light;
    let old = l;
    l.yaw += 1;
    assert!(e.set_light(l));
    assert_eq!(e.model().unwrap().light, l);
    e.undo();
    assert_eq!(e.model().unwrap().light, old);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
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
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
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
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
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

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn other_writers_cannot_touch_a_model_sprite() {
    let mut e = editor_with_model();
    e.select_all();
    let doc = e.document.clone();
    e.primary = cs_core::PixelValue::Rgba(Rgba::rgb(9, 9, 9));
    assert!(!e.fill_selection());
    let filter = crate::filters::Filter::BrightnessContrast {
        brightness: 50,
        contrast: 0,
    };
    assert!(!e.apply_filter(&filter));
    assert_eq!(e.document, doc);
    e.undo();
    assert_eq!(
        e.document.artboards.len(),
        1,
        "only the creation was in history"
    );
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn arrow_rotation_wraps_and_arrow_scale_keeps_sign_and_range() {
    use super::ModelArrows;
    let mut e = editor_with_model();
    let mut obj = e.model().unwrap().objects[0].clone();
    obj.transform.rotation.y = 1750;
    obj.transform.scale = cs_core::Vec3i {
        x: -1000,
        y: 10,
        z: 9_800,
    };
    assert!(e.set_object(0, obj));
    e.set_model_arrows(ModelArrows::Rotate);
    assert!(e.arrow_on_object(1, 0));
    assert_eq!(e.model().unwrap().objects[0].transform.rotation.y, -1700);
    e.set_model_arrows(ModelArrows::Scale);
    // Achicar: el negativo sigue negativo y nada baja de 10 (1 %).
    assert!(e.arrow_on_object(-1, 0));
    let s = e.model().unwrap().objects[0].transform.scale;
    assert_eq!((s.x, s.y), (-909, 10));
    // Agrandar: nada pasa de 10 000 (1000 %).
    assert!(e.arrow_on_object(1, 0));
    assert!(e.arrow_on_object(1, 0));
    let s = e.model().unwrap().objects[0].transform.scale;
    assert_eq!((s.x, s.y, s.z), (-1100, 12, 10_000));
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn two_bakes_do_not_overlap() {
    let mut e = editor_with_model();
    let options = cs_render::BakeOptions {
        frames: 4,
        ..cs_render::BakeOptions::from_model(e.model().unwrap(), (e.sprite().width, 1), 0)
    };
    assert!(e.bake(options));
    assert!(e.bake(options));
    assert!(e.is_model(), "the model stays active");
    let n = e.document.artboards.len();
    let rects: Vec<_> = e.document.artboards.iter().map(|a| a.rect()).collect();
    type Box2 = ((i32, i32), (i32, i32));
    let overlap =
        |(a0, a1): Box2, (b0, b1): Box2| a0.0 < b1.0 && b0.0 < a1.0 && a0.1 < b1.1 && b0.1 < a1.1;
    // Los dos bakes (los últimos) no pisan a nada.
    for bake in n - 2..n {
        for other in 0..n {
            if other != bake {
                assert!(
                    !overlap(rects[bake], rects[other]),
                    "{bake} overlaps {other}: {rects:?}"
                );
            }
        }
    }
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn the_palette_panel_edits_the_ramp_palette_of_a_model() {
    let mut e = editor_with_model();
    let palette = cs_core::Palette::new(vec![Rgba::rgb(10, 20, 30), Rgba::rgb(40, 50, 60)]);
    assert!(e.apply_palette(palette.clone()));
    assert_eq!(e.sprite().palette, palette);
    assert!(e.is_model());
    e.undo();
    assert_ne!(e.sprite().palette, palette);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn the_selected_object_resets_when_another_artboard_is_activated() {
    let mut e = editor_with_model();
    let model = e.active;
    let first = e.document.artboards[0].id;
    e.activate(first);
    assert_eq!(e.selected_object(), None);
    e.activate(model);
    assert_eq!(e.model_object, None);
}
