//! Editar un símbolo en su lugar (Edit in Place).

use cs_commands::symbol::SymbolTiming;
use cs_commands::{Command, DocCommand, ops};
use cs_core::{ArtboardId, ColorMode, LayerId, ObjectId, PixelValue, Rgba, Sprite};

use super::{Editor, ToolKind};

const RED: PixelValue = PixelValue::Rgba(Rgba::rgb(255, 0, 0));

/// Un host de 8×8 en (100, 50) con una instancia del símbolo "S" (2×2, sacado de los píxeles
/// (2,2) y (3,3)). Devuelve el editor, el símbolo, su capa y su objeto.
fn host_with_symbol() -> (Editor, ArtboardId, LayerId, ObjectId) {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    e.document.artboards[0].position = (100, 50);
    e.execute(Command::DrawPixels {
        layer: e.layer,
        frame: 0,
        pixels: vec![(2, 2, RED), (3, 3, RED)],
    });
    let symbol = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
    let layer = e.layer;
    let object = e.sprite().layer(layer).unwrap().objects[0].id;
    (e, symbol, layer, object)
}

fn position(e: &Editor, id: ArtboardId) -> (i32, i32) {
    e.document.artboard(id).unwrap().position
}

#[test]
fn entering_in_place_shows_the_symbol_where_its_instance_is() {
    let (mut e, s, layer, object) = host_with_symbol();
    let host = e.active;
    let home = position(&e, s);
    assert!(e.enter_symbol_in_place(layer, object));
    assert_eq!(e.active, s);
    assert!(e.in_symbol_mode() && e.editing_in_place());
    assert_eq!(position(&e, s), (102, 52), "host + key position");
    assert_eq!(e.symbol_breadcrumb(), vec!["Scene".to_string(), "S".into()]);
    // "Fit" abarca la escena de alrededor.
    assert_eq!(e.in_place_bounds(), Some(((100, 50), (108, 58))));
    // Salir devuelve la posición de la biblioteca y el host.
    e.exit_symbol_to(0);
    assert!(!e.in_symbol_mode());
    assert_eq!((e.active, position(&e, s)), (host, home));
    assert_eq!(e.in_place_bounds(), None);
}

#[test]
fn painting_in_place_paints_the_symbol_pixel_under_the_cursor() {
    let (mut e, s, layer, object) = host_with_symbol();
    assert!(e.enter_symbol_in_place(layer, object));
    e.set_tool(ToolKind::Pencil);
    let blue = Rgba::rgb(0, 0, 255);
    e.primary = PixelValue::Rgba(blue);
    // (103, 52) en el mundo es el píxel (1, 0) del símbolo.
    e.pointer_press((103.5, 52.5), false, false, 0.0);
    e.pointer_release();
    let sprite = &e.document.artboard(s).unwrap().sprite;
    assert_eq!(cs_render::composite_pixel(sprite, 0, 1, 0), blue);
    // Un paso de undo.
    e.undo();
    let sprite = &e.document.artboard(s).unwrap().sprite;
    assert_eq!(cs_render::composite_pixel(sprite, 0, 1, 0).a, 0);
    assert!(e.editing_in_place(), "undoing the stroke stays inside");
}

#[test]
fn escape_leaves_one_level_when_there_is_nothing_to_cancel() {
    let (mut e, s, layer, object) = host_with_symbol();
    let home = position(&e, s);
    assert!(e.enter_symbol_in_place(layer, object));
    // Con una marquesina, Esc primero la quita.
    e.select_all();
    e.escape();
    assert!(e.selection.is_none() && e.in_symbol_mode());
    e.escape();
    assert!(!e.in_symbol_mode());
    assert_eq!(position(&e, s), home);
    // También sale del modo aislado.
    assert!(e.enter_symbol(s));
    e.escape();
    assert!(!e.in_symbol_mode());
}

#[test]
fn nested_in_place_levels_add_their_positions() {
    // T dentro de S dentro del host: primero T (los píxeles) y después S con la capa de T.
    let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
    e.document.artboards[0].position = (10, 20);
    e.execute(Command::DrawPixels {
        layer: e.layer,
        frame: 0,
        pixels: vec![(4, 5, RED), (6, 6, RED)],
    });
    let t = e.convert_to_symbol("T", SymbolTiming::Graphic).unwrap();
    let s = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
    let host = e.active;
    let s_layer = e.layer;
    let s_object = e.sprite().layer(s_layer).unwrap().objects[0].id;
    // T, corrido (2, 1) adentro de S.
    let inner = &e.document.artboard(s).unwrap().sprite;
    let t_layer = inner.layers[0].id;
    let t_object = inner.layers[0].objects[0].id;
    let cmd = ops::move_objects(inner, t_layer, &[t_object], 0, 2, 1).unwrap();
    assert!(e.execute_doc(DocCommand::OnArtboard { id: s, cmd }));
    assert!(e.enter_symbol_in_place(s_layer, s_object));
    assert_eq!(position(&e, s), (14, 25));
    assert!(e.enter_symbol_in_place(t_layer, t_object));
    assert_eq!(e.active, t);
    assert_eq!(position(&e, t), (16, 26), "host + S + T");
    assert_eq!(
        e.symbol_breadcrumb(),
        vec!["Scene".to_string(), "S".into(), "T".into()]
    );
    e.escape();
    assert_eq!(e.active, s);
    assert_eq!(position(&e, s), (14, 25));
    e.escape();
    assert_eq!(e.active, host);
}

#[test]
fn a_flipped_instance_is_edited_isolated() {
    let (mut e, s, layer, object) = host_with_symbol();
    let cmd = ops::update_object_key(e.sprite(), layer, object, 0, |k| k.flip_h = true).unwrap();
    e.execute(cmd);
    let home = position(&e, s);
    assert!(e.enter_symbol_in_place(layer, object));
    assert_eq!(e.active, s);
    assert!(!e.editing_in_place());
    assert_eq!(position(&e, s), home);
}

#[test]
fn the_symbol_starts_at_the_frame_its_instance_shows() {
    // Un host de 3 frames: el símbolo también tiene 3. Con la instancia empezando en el 1
    // (Loop), en el frame 1 del host muestra el 2.
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    for index in 1..3 {
        e.execute(Command::AddFrame {
            index,
            frame: cs_core::Frame { duration_ms: 100 },
        });
    }
    e.execute(Command::DrawPixels {
        layer: e.layer,
        frame: 0,
        pixels: vec![(2, 2, RED)],
    });
    let host = e.active;
    let s = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
    let layer = e.layer;
    let object = e.sprite().layer(layer).unwrap().objects[0].id;
    let cmd = ops::update_object_key(e.sprite(), layer, object, 0, |k| {
        k.timing = cs_core::Timing::Graphic {
            first_frame: 1,
            play: cs_core::GraphicPlay::Loop,
        }
    })
    .unwrap();
    e.execute(cmd);
    e.frame = 1;
    assert!(e.enter_symbol_in_place(layer, object));
    assert_eq!((e.active, e.frame), (s, 2));
    e.escape();
    assert_eq!((e.active, e.frame), (host, 1), "the host frame is back");
}

#[test]
fn undo_that_removes_the_instance_leaves_in_place() {
    let (mut e, s, layer, object) = host_with_symbol();
    let home = position(&e, s);
    let host = e.active;
    assert!(e.enter_symbol_in_place(layer, object));
    // Un script borra el objeto del host.
    let cmd =
        ops::remove_objects(&e.document.artboard(host).unwrap().sprite, layer, &[object]).unwrap();
    assert!(e.execute_doc(DocCommand::OnArtboard { id: host, cmd }));
    assert!(!e.in_symbol_mode());
    assert_eq!((e.active, position(&e, s)), (host, home));
}

#[test]
fn an_undo_that_moves_the_instance_moves_the_symbol() {
    let (mut e, s, layer, object) = host_with_symbol();
    let cmd = ops::move_objects(e.sprite(), layer, &[object], 0, 3, 0).unwrap();
    e.execute(cmd);
    assert!(e.enter_symbol_in_place(layer, object));
    assert_eq!(position(&e, s), (105, 52));
    e.undo();
    assert!(e.editing_in_place());
    assert_eq!(position(&e, s), (102, 52));
}

#[test]
fn saving_writes_the_library_position() {
    let (mut e, s, layer, object) = host_with_symbol();
    let home = position(&e, s);
    assert!(matches!(
        e.document_for_save(),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(e.enter_symbol_in_place(layer, object));
    assert_ne!(position(&e, s), home);
    assert_eq!(e.document_for_save().artboard(s).unwrap().position, home);
}

#[test]
fn duplicating_a_symbol_edited_in_place_copies_it_at_its_library_position() {
    let (mut e, s, layer, object) = host_with_symbol();
    let home = position(&e, s);
    assert!(e.enter_symbol_in_place(layer, object));
    let copy = e.duplicate_library_item(s).unwrap();
    let artboard = e.document.artboard(copy).unwrap();
    assert_eq!(
        (artboard.name.as_str(), artboard.position),
        ("S copy", home)
    );
    assert!(artboard.in_library);
    // Sin editar en su lugar, la de siempre.
    e.exit_symbol_to(0);
    let again = e.duplicate_library_item(s).unwrap();
    assert_eq!(position(&e, again), home);
    e.undo();
    assert!(e.document.artboard(again).is_none(), "one undo step");
}

#[test]
fn place_on_canvas_while_editing_in_place_leaves_first() {
    let (mut e, s, layer, object) = host_with_symbol();
    let host = e.active;
    let home = position(&e, s);
    assert!(e.enter_symbol_in_place(layer, object));
    let undos = e.history.done().count();
    assert!(e.place_on_canvas(s));
    assert!(!e.in_symbol_mode(), "out of the symbol mode");
    assert_eq!(e.active, s);
    let artboard = e.document.artboard(s).unwrap();
    assert!(artboard.on_canvas());
    // Se coloca desde su posición de la biblioteca (o en un lugar libre), no desde la de edición
    // encima del host.
    let ((x0, y0), (x1, y1)) = artboard.rect();
    let ((hx0, hy0), (hx1, hy1)) = e.document.artboard(host).unwrap().rect();
    assert!(
        x1 <= hx0 || hx1 <= x0 || y1 <= hy0 || hy1 <= y0,
        "not over the host"
    );
    assert_eq!(e.history.done().count(), undos + 1, "one undo step");
    e.undo();
    let artboard = e.document.artboard(s).unwrap();
    assert!(artboard.in_library);
    assert_eq!(artboard.position, home);
    // Aislado, igual: sale del modo símbolo.
    assert!(e.enter_symbol(s));
    assert!(e.place_on_canvas(s));
    assert!(!e.in_symbol_mode() && e.active == s);
    // Lo que no es de la biblioteca no sale de ningún lado.
    assert!(!e.place_on_canvas(host));
}

#[test]
fn double_click_on_an_instance_enters_in_place() {
    let (mut e, s, _, _) = host_with_symbol();
    assert_eq!(e.open_object_at((102.5, 52.5)), Some(true));
    assert_eq!(e.active, s);
    assert!(e.editing_in_place());
    // "Edit Symbol" (el inspector) sigue aislado.
    e.exit_symbol_to(0);
    assert!(e.enter_instance());
    assert!(e.in_symbol_mode() && !e.editing_in_place());
}

#[test]
fn the_context_hides_the_edited_instance_and_keeps_the_others() {
    let (mut e, s, layer, object) = host_with_symbol();
    let host = e.active;
    // Otra instancia del mismo símbolo en la capa.
    let _ = e.drop_symbol(s, (104, 54));
    assert_eq!(e.sprite().layer(layer).unwrap().objects.len(), 2);
    assert!(e.in_place_context(&e.document).is_none(), "not in place");
    assert!(e.enter_symbol_in_place(layer, object));
    let context = e.in_place_context(&e.document).expect("in place");
    assert_eq!(context.artboards, vec![host]);
    let objects = &context
        .document
        .artboard(host)
        .unwrap()
        .sprite
        .layer(layer)
        .unwrap()
        .objects;
    assert_eq!(objects.len(), 1);
    assert_ne!(objects[0].id, object);
    // El documento de verdad no cambia.
    assert_eq!(
        e.document
            .artboard(host)
            .unwrap()
            .sprite
            .layer(layer)
            .unwrap()
            .objects
            .len(),
        2
    );
}

#[test]
fn a_nested_context_copies_the_middle_symbol_without_the_instance() {
    let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
    e.execute(Command::DrawPixels {
        layer: e.layer,
        frame: 0,
        pixels: vec![(4, 5, RED)],
    });
    let _t = e.convert_to_symbol("T", SymbolTiming::Graphic).unwrap();
    let s = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
    let host = e.active;
    let s_layer = e.layer;
    let s_object = e.sprite().layer(s_layer).unwrap().objects[0].id;
    assert!(e.enter_symbol_in_place(s_layer, s_object));
    let inner = &e.sprite().layers[0];
    let (t_layer, t_object) = (inner.id, inner.objects[0].id);
    assert!(e.enter_symbol_in_place(t_layer, t_object));
    let context = e.in_place_context(&e.document).unwrap();
    assert_eq!(context.artboards, vec![host]);
    let doc = &context.document;
    // El host apunta a una copia de S sin T; S sigue con T.
    let key = &doc
        .artboard(host)
        .unwrap()
        .sprite
        .layer(s_layer)
        .unwrap()
        .objects[0]
        .track
        .keys[0];
    let copy = key.source.unwrap();
    assert_ne!(copy, s);
    assert!(
        doc.artboard(copy)
            .unwrap()
            .sprite
            .layer(t_layer)
            .unwrap()
            .objects
            .is_empty()
    );
    assert_eq!(
        doc.artboard(s)
            .unwrap()
            .sprite
            .layer(t_layer)
            .unwrap()
            .objects
            .len(),
        1
    );
    // Aislado en S y en su lugar en T: el contexto es S (sin T).
    e.exit_symbol_to(0);
    assert!(e.enter_symbol(s));
    assert!(e.enter_symbol_in_place(t_layer, t_object));
    let context = e.in_place_context(&e.document).unwrap();
    assert_eq!(context.artboards, vec![s]);
    let doc = &context.document;
    assert!(
        doc.artboard(s)
            .unwrap()
            .sprite
            .layer(t_layer)
            .unwrap()
            .objects
            .is_empty()
    );
}

#[test]
fn scaled_or_rotated_instances_are_edited_isolated() {
    for change in [
        (|k: &mut cs_core::InstanceKey| k.rotation = 90) as fn(&mut cs_core::InstanceKey),
        |k| k.scale = (2000, 1000),
    ] {
        let (mut e, s, layer, object) = host_with_symbol();
        let cmd = ops::update_object_key(e.sprite(), layer, object, 0, change).unwrap();
        e.execute(cmd);
        assert!(e.enter_symbol_in_place(layer, object));
        assert_eq!(e.active, s);
        assert!(!e.editing_in_place());
    }
}

#[test]
fn in_place_inside_a_tween_uses_where_the_instance_is_on_that_frame() {
    let (mut e, s, layer, object) = host_with_symbol();
    e.insert_empty_frame();
    e.insert_empty_frame();
    // Del frame 0 en (2, 2) al 2 en (6, 4), con tween: en el frame 1 está en (4, 3).
    let cmd =
        ops::update_object_key(e.sprite(), layer, object, 2, |k| k.position = (6, 4)).unwrap();
    e.execute(cmd);
    let cmd = ops::create_tweens(e.sprite(), layer, &[object], 0).unwrap();
    e.execute(cmd);
    e.set_frame(1);
    assert!(e.enter_symbol_in_place(layer, object));
    assert!(e.editing_in_place());
    assert_eq!(position(&e, s), (104, 53));
}
