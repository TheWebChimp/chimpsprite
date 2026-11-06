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
    // El objeto nuevo guarda su tipo (y el cubo inicial el suyo).
    let objects = &e.model().unwrap().objects;
    assert_eq!(objects[0].kind, Some(cs_core::PrimitiveKind::Cube));
    assert_eq!(objects[1].kind, Some(cs_core::PrimitiveKind::Cylinder));
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
fn removing_an_object_keeps_the_selection_on_the_same_objects() {
    let mut e = editor_with_model();
    e.add_primitive(Primitive::Plane);
    e.add_primitive(Primitive::Cylinder { sides: 8 });
    e.add_primitive(Primitive::Plane);
    e.select_model_object(Some(2));
    assert_eq!(
        e.model_edit()
            .selection
            .items
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![cs_3d::Item::Object(2)]
    );
    assert!(e.remove_object(0));
    assert_eq!(
        e.model_edit()
            .selection
            .items
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![cs_3d::Item::Object(1)]
    );
    // El último también se corre (no se pierde por quedar fuera de rango).
    e.select_model_object(Some(2));
    assert!(e.remove_object(0));
    assert_eq!(
        e.model_edit()
            .selection
            .items
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![cs_3d::Item::Object(1)]
    );
    // Quitar el elegido saca su selección.
    assert!(e.remove_object(1));
    assert!(e.model_edit().selection.items.is_empty());
}

#[test]
fn removing_a_lower_object_keeps_the_same_one_selected() {
    let mut e = editor_with_model();
    e.add_primitive(Primitive::Plane);
    e.add_primitive(Primitive::Cylinder { sides: 8 });
    e.select_model_object(Some(2));
    let name = e.model().unwrap().objects[2].name.clone();
    assert!(e.remove_object(0));
    assert_eq!(e.selected_object(), Some(1));
    assert_eq!(e.model().unwrap().objects[1].name, name);
    // Quitar uno posterior no mueve la elegida.
    e.select_model_object(Some(0));
    assert!(e.remove_object(1));
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
    let mut l = e.model().unwrap().light;
    let old = l;
    l.yaw += 1;
    assert!(e.set_light(l));
    assert_eq!(e.model().unwrap().light, l);
    e.undo();
    assert_eq!(e.model().unwrap().light, old);
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

#[test]
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
    // Los dos bakes (los últimos) no pisan a nada del lienzo (el modelo no está en él).
    for bake in n - 2..n {
        for other in 0..n {
            if other != bake && e.document.artboards[other].on_canvas() {
                assert!(
                    !overlap(rects[bake], rects[other]),
                    "{bake} overlaps {other}: {rects:?}"
                );
            }
        }
    }
}

#[test]
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
fn the_selected_object_resets_when_another_artboard_is_activated() {
    let mut e = editor_with_model();
    let model = e.active;
    let first = e.document.artboards[0].id;
    e.activate(first);
    assert_eq!(e.selected_object(), None);
    e.activate(model);
    assert_eq!(e.model_object, None);
}

#[test]
fn duplicating_copies_the_chosen_object_in_place_in_one_undo_step() {
    let mut e = editor_with_model();
    let mut moved = e.model().unwrap().objects[0].clone();
    moved.transform.position.x = 32;
    e.set_object(0, moved);
    let steps = e.history.done_count();
    assert!(e.duplicate_model_objects());
    let objects = &e.model().unwrap().objects;
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[1].name, format!("{} copy", objects[0].name));
    // La misma malla; sus UVs, en otro lugar de la textura.
    assert_eq!(objects[1].mesh.vertices, objects[0].mesh.vertices);
    let verts = |o: usize| -> Vec<Vec<u32>> {
        objects[o]
            .mesh
            .faces
            .iter()
            .map(|f| f.verts.clone())
            .collect()
    };
    assert_eq!(verts(1), verts(0));
    assert_ne!(
        objects[1].mesh, objects[0].mesh,
        "its own place in the texture"
    );
    assert_eq!(objects[1].transform, objects[0].transform, "in place");
    assert_eq!(objects[1].kind, objects[0].kind);
    assert_eq!(e.selected_object(), Some(1), "the copy is chosen");
    assert_eq!(e.history.done_count(), steps + 1);
    e.undo();
    assert_eq!(e.model().unwrap().objects.len(), 1);
}

#[test]
fn with_several_objects_chosen_each_one_is_duplicated() {
    let mut e = editor_with_model();
    assert!(e.add_primitive(Primitive::Plane));
    e.set_select_mode(cs_3d::SelectMode::Object);
    e.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Object,
        items: [cs_3d::Item::Object(0), cs_3d::Item::Object(1)].into(),
    });
    assert!(e.duplicate_model_objects());
    let names: Vec<String> = e
        .model()
        .unwrap()
        .objects
        .iter()
        .map(|o| o.name.clone())
        .collect();
    assert_eq!(
        names[2..],
        [format!("{} copy", names[0]), format!("{} copy", names[1])]
    );
    // Las copias quedan elegidas.
    let items = &e.model_edit().selection.items;
    assert_eq!(
        items,
        &[cs_3d::Item::Object(2), cs_3d::Item::Object(3)].into()
    );
}

#[test]
fn without_an_object_there_is_nothing_to_duplicate() {
    let mut e = editor_with_model();
    e.select_model_object(None);
    assert!(!e.can_duplicate_model_objects());
    assert!(!e.duplicate_model_objects());
    assert_eq!(e.model().unwrap().objects.len(), 1);
}

fn with_a_group(e: &mut Editor) {
    let mut m = e.model().unwrap().clone();
    m.groups.push(cs_core::Group3D {
        name: "g".into(),
        transform: Default::default(),
        hidden: false,
        locked: false,
        open: true,
        anim: Vec::new(),
    });
    m.tree = vec![cs_core::Node::Group {
        group: 0,
        children: vec![cs_core::Node::Object(0)],
    }];
    assert!(e.set_model(m));
}

#[test]
fn a_duplicate_stays_in_the_group_of_its_original() {
    let mut e = editor_with_model();
    with_a_group(&mut e);
    assert!(e.duplicate_model_objects());
    let m = e.model().unwrap();
    assert_eq!(m.object_parents(1), vec![0]);
}

#[test]
fn removing_an_object_keeps_the_others_in_their_groups() {
    let mut e = editor_with_model();
    assert!(e.add_primitive(Primitive::Plane));
    with_a_group(&mut e); // el 0 en el grupo, el 1 en la raíz
    assert!(e.remove_object(0));
    let m = e.model().unwrap();
    assert_eq!(m.object_parents(0), Vec::<usize>::new());
    assert_eq!(m.group_objects(0), Vec::<usize>::new());
}

#[test]
fn duplicating_a_group_copies_it_with_its_objects() {
    let mut e = editor_with_model();
    with_a_group(&mut e);
    let steps = e.history.done_count();
    assert!(e.duplicate_group(0));
    let m = e.model().unwrap();
    assert_eq!(m.groups.len(), 2);
    assert_eq!(m.groups[1].name, "g copy");
    assert_eq!(m.group_objects(1), vec![1]);
    // Solo el grupo lleva " copy": lo de adentro conserva su nombre.
    assert_eq!(m.objects[1].name, m.objects[0].name);
    // La copia va justo después del original.
    assert!(matches!(m.tree[1], cs_core::Node::Group { group: 1, .. }));
    assert_eq!(e.history.done_count(), steps + 1);
}

#[test]
fn duplicating_a_group_copies_its_subgroups() {
    let mut e = editor_with_model();
    with_a_group(&mut e);
    let mut m = e.model().unwrap().clone();
    m.groups.push(cs_core::Group3D {
        name: "sub".into(),
        transform: Default::default(),
        hidden: false,
        locked: false,
        open: true,
        anim: Vec::new(),
    });
    m.tree = vec![cs_core::Node::Group {
        group: 0,
        children: vec![
            cs_core::Node::Object(0),
            cs_core::Node::Group {
                group: 1,
                children: Vec::new(),
            },
        ],
    }];
    assert!(e.set_model(m));
    assert!(e.duplicate_group(0));
    let m = e.model().unwrap();
    assert_eq!(m.groups.len(), 4);
    assert_eq!(m.groups[2].name, "g copy");
    assert_eq!(m.groups[3].name, "sub");
    assert_eq!(m.group_parents(3), vec![2]);
    assert_eq!(m.group_objects(2), vec![1]);
}

/// La textura del modelo que se edita.
fn texture_of(e: &Editor) -> &Sprite {
    let id = e.model().unwrap().texture.unwrap();
    &e.document.artboard(id).unwrap().sprite
}

/// Pinta `pixels` en la textura del modelo (capa y frame 0).
fn paint_texture(e: &mut Editor, pixels: &[(i32, i32, cs_core::PixelValue)]) {
    let id = e.model().unwrap().texture.unwrap();
    let layer = texture_of(e).layers[0].id;
    assert!(e.execute_doc(cs_commands::DocCommand::OnArtboard {
        id,
        cmd: cs_commands::Command::DrawPixels {
            layer,
            frame: 0,
            pixels: pixels.to_vec(),
        },
    }));
}

/// Si las cajas de UVs de `a` y `b` se tocan (contando el pixel de aire).
fn boxes_touch(m: &cs_core::Model3D, a: usize, b: usize) -> bool {
    let (ra, rb) = (
        cs_3d::object_uv_rect(m, a).unwrap(),
        cs_3d::object_uv_rect(m, b).unwrap(),
    );
    let m = cs_3d::MARGIN;
    ra.0 < rb.0 + rb.2 + m
        && rb.0 < ra.0 + ra.2 + m
        && ra.1 < rb.1 + rb.3 + m
        && rb.1 < ra.1 + ra.3 + m
}

/// Los pixeles de la capa 0, frame 0 de `s` en `0..w` × `0..h`.
fn pixels_of(s: &Sprite, w: i32, h: i32) -> Vec<cs_core::PixelValue> {
    let layer = s.layers[0].id;
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| s.pixel(layer, 0, x, y))
        .collect()
}

#[test]
fn two_cubes_do_not_share_texture() {
    let mut e = editor_with_model();
    assert!(e.add_primitive(Primitive::Cube));
    let m = e.model().unwrap();
    assert!(!boxes_touch(m, 0, 1));
}

#[test]
fn without_room_the_texture_grows_and_keeps_its_pixels() {
    let mut e = editor_with_model();
    assert_eq!(texture_of(&e).width, 64);
    // Debajo del cubo caben tres planos de 16×16 (con su pixel de aire).
    for _ in 0..3 {
        assert!(e.add_primitive(Primitive::Plane));
        assert_eq!(texture_of(&e).width, 64);
    }
    let before = pixels_of(texture_of(&e), 64, 64);
    let steps = e.history.done_count();
    assert!(e.add_primitive(Primitive::Plane));
    let t = texture_of(&e);
    assert_eq!((t.width, t.height), (128, 128));
    // Fuera de la caja del nuevo (que trae su plantilla) los pixeles de antes siguen igual.
    let m = e.model().unwrap();
    let (nx, ny, nw, nh) = cs_3d::object_uv_rect(m, 4).unwrap();
    let outside = |(i, _): &(usize, &cs_core::PixelValue)| {
        let (x, y) = ((i % 64) as i32, (i / 64) as i32);
        !(nx..nx + nw).contains(&x) || !(ny..ny + nh).contains(&y)
    };
    let after = pixels_of(t, 64, 64);
    let kept: Vec<_> = after.iter().enumerate().filter(outside).collect();
    let was: Vec<_> = before.iter().enumerate().filter(outside).collect();
    assert_eq!(kept, was, "the old pixels stay in place");
    for o in 0..4 {
        assert!(!boxes_touch(m, o, 4), "{o}");
    }
    assert_eq!(e.history.done_count(), steps + 1, "one undo step");
    e.undo();
    assert_eq!(texture_of(&e).width, 64);
    assert_eq!(e.model().unwrap().objects.len(), 4);
}

#[test]
fn a_duplicate_gets_its_own_place_and_a_copy_of_its_pixels() {
    let mut e = editor_with_model();
    // Un pixel en la cara de enfrente del cubo (la celda (0,0)-(16,16)).
    let red = cs_core::PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    paint_texture(&mut e, &[(5, 6, red)]);
    let steps = e.history.done_count();
    assert!(e.duplicate_model_objects());
    assert_eq!(e.history.done_count(), steps + 1, "one undo step");
    let m = e.model().unwrap().clone();
    assert!(!boxes_touch(&m, 0, 1));
    let (x0, y0, _, _) = cs_3d::object_uv_rect(&m, 0).unwrap();
    let (x1, y1, _, _) = cs_3d::object_uv_rect(&m, 1).unwrap();
    let t = texture_of(&e);
    let layer = t.layers[0].id;
    assert_eq!(t.pixel(layer, 0, 5 - x0 + x1, 6 - y0 + y1), red);
    // El render de la copia se ve igual que el del original (están en el mismo lugar).
    let ab = e.document.artboard(e.model_id()).unwrap();
    let only = |keep: usize| {
        let mut only = m.clone();
        for (i, o) in only.objects.iter_mut().enumerate() {
            o.hidden = i != keep;
        }
        cs_render::model_render(&e.document, ab, &only, (64, 64), None)
    };
    assert_eq!(only(1), only(0));
    e.undo();
    assert_eq!(texture_of(&e).width, 64);
    assert_eq!(e.model().unwrap().objects.len(), 1);
}

#[test]
fn an_added_object_brings_the_template_of_its_primitive() {
    let mut e = editor_with_model();
    assert!(e.add_primitive(Primitive::Plane));
    let m = e.model().unwrap();
    let (x, y, w, h) = cs_3d::object_uv_rect(m, 1).unwrap();
    let (zx, zy, _, _) = Primitive::Plane.zone();
    let template = cs_commands::model::texture_template(&e.sprite().palette);
    let t = texture_of(&e);
    let (tl, nl) = (template.layers[0].id, t.layers[0].id);
    for dy in 0..h {
        for dx in 0..w {
            assert_eq!(
                t.pixel(nl, 0, x + dx, y + dy),
                template.pixel(tl, 0, zx + dx, zy + dy),
                "({dx}, {dy})"
            );
        }
    }
}
