#[test]
fn long_bubble_texts_wrap_and_end_in_an_ellipsis() {
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let long = "It's not too often I get to write about a new old-school tool for making \
                    pixel art, and this one has a lot of nice ideas worth a look. "
            .repeat(3);
        let galley = super::bubble_galley(ui, &long, font.clone(), Color32::WHITE);
        assert!(
            galley.rows.len() <= super::BUBBLE_MAX_ROWS,
            "{}",
            galley.rows.len()
        );
        assert!(galley.size().x <= super::BUBBLE_MAX_WIDTH + 1.0);
        assert!(
            galley.text().ends_with(crate::tab_bar::ellipsis()),
            "{}",
            galley.text()
        );
        // Lo corto queda igual, en una línea.
        let short = super::bubble_galley(ui, "Hand (H)", font, Color32::WHITE);
        assert_eq!((short.rows.len(), short.text()), (1, "Hand (H)"));
    });
    output.textures_delta.clear();
}

#[test]
fn the_scroll_thumb_has_1_px_of_padding_on_every_side() {
    let track = Rect::from_min_size(pos2(100.0, 20.0), vec2(SCROLL_BAR, 200.0));
    // Arriba de todo y abajo de todo: a 1 px de las puntas y de los lados.
    let top = super::scroll_thumb(track, true, 1000.0, 200.0, 0.0);
    assert_eq!(top.min, pos2(101.0, 21.0));
    assert_eq!(top.width(), SCROLL_BAR - 2.0);
    let bottom = super::scroll_thumb(track, true, 1000.0, 200.0, 800.0);
    assert_eq!(bottom.bottom(), track.bottom() - 1.0);
    // A lo ancho, igual.
    let track = Rect::from_min_size(pos2(0.0, 50.0), vec2(300.0, SCROLL_BAR));
    let right = super::scroll_thumb(track, false, 900.0, 300.0, 600.0);
    assert_eq!((right.right(), right.top()), (299.0, 51.0));
}

use egui::{Event, PointerButton, RawInput};

use super::*;

fn screen() -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
        ..Default::default()
    }
}

/// Un frame; las texturas (fuentes y skin) no se suben a ninguna GPU.
fn frame(ctx: &egui::Context, input: RawInput, run_ui: impl FnMut(&mut Ui)) {
    ctx.run_ui(input, run_ui).textures_delta.clear();
}

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

/// Un frame con un botón en un panel central: su rectángulo y si se hizo clic.
fn button(ctx: &egui::Context, skin: &Skin, label: &str, events: Vec<Event>) -> (Rect, bool) {
    let mut out = (Rect::NOTHING, false);
    let input = RawInput { events, ..screen() };
    frame(ctx, input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let r = pixel_button(ui, skin, label);
            out = (r.rect, r.clicked());
        });
    });
    out
}

#[test]
fn a_button_takes_the_piece_size_and_grows_with_the_text() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let (short, _) = button(&ctx, &skin, "OK", vec![]);
    assert_eq!(short.size(), vec2(49.0, 17.0), "the piece and its light");
    let (long, _) = button(&ctx, &skin, "A much longer button label", vec![]);
    assert!(long.width() > 49.0, "{long:?}");
    assert_eq!(long.height(), 17.0);
}

#[test]
fn clicking_works_with_the_skin_and_with_the_flat_fallback() {
    let ctx = egui::Context::default();
    let skins = [
        Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
        Skin::fallback(),
    ];
    for skin in &skins {
        let (rect, _) = button(&ctx, skin, "Cancel", vec![]);
        let at = rect.center();
        button(&ctx, skin, "Cancel", vec![Event::PointerMoved(at)]);
        button(&ctx, skin, "Cancel", vec![press(at, true)]);
        let (_, clicked) = button(&ctx, skin, "Cancel", vec![press(at, false)]);
        assert!(clicked);
    }
}

#[test]
fn the_dialog_is_centered_with_its_content_inside_the_frame() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let id = Id::new("test_dialog");
    let mut content = Rect::NOTHING;
    // El área se mide en el primer frame.
    for _ in 0..3 {
        frame(&ctx, screen(), |ui| {
            pixel_dialog_at(
                ui.ctx(),
                &skin,
                id,
                "Unsaved changes",
                false,
                (Align2::CENTER_CENTER, Vec2::ZERO),
                |ui| {
                    content = ui.max_rect();
                    ui.label("Save changes before closing?");
                },
            );
        });
    }
    let frame = ctx
        .memory(|m| m.area_rect(id))
        .expect("the dialog is shown");
    // `window`: bordes de 3 px y barra de título de 14, más 4 px de margen.
    assert_eq!(content.min - frame.min, vec2(7.0, 18.0));
    assert!(
        frame.center().distance(Pos2::new(400.0, 300.0)) <= 1.0,
        "{frame:?}"
    );
}

/// Que `v` caiga en un píxel físico entero a `ppp`.
fn on_pixels(v: f32, ppp: f32) -> bool {
    ((v * ppp) - (v * ppp).round()).abs() < 1e-3
}

fn assert_on_pixels(shapes: &[Shape], ppp: f32) {
    for shape in shapes {
        let r = shape.visual_bounding_rect();
        for v in [r.min.x, r.min.y, r.max.x, r.max.y] {
            assert!(on_pixels(v, ppp), "{r:?} at {ppp}");
        }
    }
}

#[test]
fn pieces_and_the_flat_fallback_land_on_whole_physical_pixels() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let window = skin.piece(DIALOG_PIECE, WidgetState::Normal).unwrap();
    let rect = Rect::from_min_size(pos2(10.3, 20.7), vec2(100.4, 60.2));
    for ppp in [1.0, 2.0, 3.0] {
        let shapes = piece_shapes(window, rect, ppp);
        assert_eq!(shapes.len(), 9);
        assert_on_pixels(&shapes, ppp);
        let flat: Vec<Shape> = flat_shapes(&skin, rect, 13.0, ppp)
            .into_iter()
            .filter(|s| matches!(s, Shape::Rect(r) if r.fill != Color32::TRANSPARENT))
            .collect();
        assert_eq!(flat.len(), 2, "background and title bar");
        assert_on_pixels(&flat, ppp);
    }
}

#[test]
fn with_pixel_ui_scroll_areas_draw_the_design_bars() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let thumb = skin
        .piece(SCROLL_THUMB_V, WidgetState::Normal)
        .unwrap()
        .texture
        .id();
    let track = skin
        .piece(SCROLL_TRACK_V, WidgetState::Normal)
        .unwrap()
        .texture
        .id();
    // Las mallas con `id` dibujadas en un frame con el contenido más alto que el área.
    let drawn = |pixel: bool| {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            set_bubble(ui.ctx(), pixel.then(|| std::sync::Arc::new(skin.clone())));
            scroll(ui, egui::ScrollArea::vertical().max_height(100.0), |ui| {
                ui.allocate_space(vec2(50.0, 400.0));
            });
        });
        output.textures_delta.clear();
        let mut found = Vec::new();
        for clipped in &output.shapes {
            for (id, rect) in meshes(&clipped.shape) {
                if id == thumb || id == track {
                    found.push((id, rect));
                }
            }
        }
        found
    };
    let found = drawn(true);
    assert!(found.iter().any(|(id, _)| *id == track), "the track");
    let thumb_rect = found
        .iter()
        .filter(|(id, _)| *id == thumb)
        .fold(Rect::NOTHING, |b, (_, r)| b.union(*r));
    // El pulgar: 8 de ancho y, con 100 de 400 a la vista, el largo mínimo (46).
    assert_eq!(thumb_rect.width(), 8.0, "{thumb_rect:?}");
    assert_eq!(thumb_rect.height(), SCROLL_THUMB_MIN, "{thumb_rect:?}");
    assert!(drawn(false).is_empty(), "without Pixel UI, egui's bar");
}

/// Las mallas (con su textura) de una forma, aplanada.
fn meshes(shape: &Shape) -> Vec<(egui::TextureId, Rect)> {
    match shape {
        Shape::Vec(shapes) => shapes.iter().flat_map(meshes).collect(),
        Shape::Mesh(m) => vec![(m.texture_id, m.calc_bounds())],
        _ => Vec::new(),
    }
}

#[test]
fn parts_with_an_empty_source_are_not_drawn() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let window = skin.piece(DIALOG_PIECE, WidgetState::Normal).unwrap();
    // Centro de 0×0: el centro y los cuatro bordes no tienen de dónde copiar.
    let piece = Piece {
        texture: window.texture.clone(),
        size: window.size,
        center: Some(crate::skin::Rect9::new(3, 13, 0, 0, window.size).unwrap()),
        duration_ms: None,
        rgba: window.rgba.clone(),
    };
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0));
    assert_eq!(piece_shapes(&piece, rect, 1.0).len(), 4, "only the corners");
}

#[test]
fn a_row_of_buttons_fills_the_content_width_evenly() {
    let ctx = egui::Context::default();
    let skins = [
        Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
        Skin::fallback(),
    ];
    for skin in &skins {
        let id = Id::new((
            "row",
            skin.piece(BUTTON_PIECE, WidgetState::Normal).is_some(),
        ));
        let mut content = Rect::NOTHING;
        let mut rects = Vec::new();
        for _ in 0..3 {
            frame(&ctx, screen(), |ui| {
                pixel_dialog_at(
                    ui.ctx(),
                    skin,
                    id,
                    "Unsaved changes",
                    true,
                    (Align2::CENTER_CENTER, Vec2::ZERO),
                    |ui| {
                        content = ui.max_rect();
                        ui.label("A long question that is wider than the buttons, surely?");
                        rects = pixel_buttons(ui, skin, &["Save", "Discard", "Cancel"])
                            .iter()
                            .map(|r| r.rect)
                            .collect();
                    },
                );
            });
        }
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].min.x, content.min.x, "{rects:?} in {content:?}");
        assert!(
            (rects[2].max.x - content.max.x).abs() < 1.0,
            "{rects:?} in {content:?}"
        );
        for pair in rects.windows(2) {
            assert_eq!(pair[1].min.x - pair[0].max.x, BUTTON_GAP, "{rects:?}");
        }
        let widths: Vec<f32> = rects.iter().map(|r| r.width()).collect();
        for w in &widths {
            assert_eq!(w.fract(), 0.0, "whole pixels: {widths:?}");
            assert!((w - widths[0]).abs() <= 1.0, "{widths:?}");
        }
    }
}

/// Un frame con un botón de herramienta (Lasso, con variantes) en el tramo `slot` de la
/// columna: su rectángulo, si se hizo clic y las texturas que se dibujaron con su rect.
fn tool(
    ctx: &egui::Context,
    skin: &Skin,
    slot: SlotPos,
    selected: bool,
    events: Vec<Event>,
) -> (Rect, bool, Vec<(egui::TextureId, Rect)>) {
    let mut out = (Rect::NOTHING, false);
    let input = RawInput { events, ..screen() };
    let textures = run(ctx, input, |ui| {
        let r = tool_button(
            ui,
            egui::Id::new("lasso"),
            skin,
            slot,
            "icon.lasso",
            "Lasso",
            selected,
            true,
        );
        out = (r.rect, r.clicked());
    });
    (out.0, out.1, textures)
}

/// Un frame con `add` en un panel central; devuelve las texturas dibujadas con su rect.
fn run(
    ctx: &egui::Context,
    input: RawInput,
    mut add: impl FnMut(&mut Ui),
) -> Vec<(egui::TextureId, Rect)> {
    let mut output = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| add(ui));
    });
    // Las texturas (fuentes y skin) no se suben a ninguna GPU.
    output.textures_delta.clear();
    let mut textures = Vec::new();
    for clipped in &output.shapes {
        collect_meshes(&clipped.shape, &mut textures);
    }
    textures
}

fn collect_meshes(shape: &Shape, out: &mut Vec<(egui::TextureId, Rect)>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_meshes(s, out)),
        Shape::Mesh(mesh) => out.push((mesh.texture_id, mesh.calc_bounds())),
        _ => {}
    }
}

fn collect_textures(shape: &Shape, out: &mut Vec<egui::TextureId>) {
    let mut meshes = Vec::new();
    collect_meshes(shape, &mut meshes);
    out.extend(meshes.into_iter().map(|(t, _)| t));
}

/// Rect de lo dibujado con la textura `texture` (la unión de sus partes).
fn drawn(textures: &[(egui::TextureId, Rect)], texture: egui::TextureId) -> Option<Rect> {
    textures
        .iter()
        .filter(|(t, _)| *t == texture)
        .map(|(_, r)| *r)
        .reduce(|a, b| a.union(b))
}

/// Cuáles de los seis marcos de la columna se dibujaron.
fn drawn_slots(skin: &Skin, textures: &[(egui::TextureId, Rect)]) -> Vec<&'static str> {
    [
        "tool.slot.first",
        "tool.slot",
        "tool.slot.last",
        "tool.slot.first.active",
        "tool.slot.active",
        "tool.slot.last.active",
    ]
    .into_iter()
    .filter(|name| {
        let id = skin.piece(name, WidgetState::Normal).unwrap().texture.id();
        textures.iter().any(|(t, _)| *t == id)
    })
    .collect()
}

#[test]
fn a_tool_button_is_its_column_slot_with_the_icon_and_the_active_slot_when_selected() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let id = |name: &str| skin.piece(name, WidgetState::Normal).unwrap().texture.id();
    for (slot, name, active, height) in [
        (
            SlotPos::First,
            "tool.slot.first",
            "tool.slot.first.active",
            22.0,
        ),
        (SlotPos::Middle, "tool.slot", "tool.slot.active", 22.0),
        (
            SlotPos::Last,
            "tool.slot.last",
            "tool.slot.last.active",
            23.0,
        ),
    ] {
        let (rect, _, textures) = tool(&ctx, &skin, slot, false, vec![]);
        assert_eq!(rect.size(), vec2(23.0, height), "{name}: the piece size");
        assert_eq!(drawn_slots(&skin, &textures), [name], "{slot:?}");
        assert_eq!(drawn(&textures, id(name)), Some(rect), "{name}");
        // El ícono de 17 va a (3,3) del tramo: 1 px adentro del relleno, como en el mockup.
        let icon = drawn(&textures, id("icon.lasso")).expect("the icon");
        assert_eq!(icon.min - rect.min, vec2(3.0, 3.0), "{name}");
        let (_, _, textures) = tool(&ctx, &skin, slot, true, vec![]);
        // Elegido: el mismo tramo (no se hunde), con su marco rosa por dentro.
        let _ = active;
        assert_eq!(drawn_slots(&skin, &textures), [name], "{slot:?} selected");
    }
}

#[test]
fn without_first_and_last_pieces_the_ends_use_the_middle_slot() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap().without(&[
        "tool.slot.first",
        "tool.slot.first.active",
        "tool.slot.last",
        "tool.slot.last.active",
    ]);
    let middle = skin.piece("tool.slot", WidgetState::Normal).unwrap();
    for slot in [SlotPos::First, SlotPos::Last] {
        let (rect, _, textures) = tool(&ctx, &skin, slot, false, vec![]);
        assert_eq!(rect.size(), vec2(23.0, 22.0), "{slot:?}");
        assert_eq!(drawn(&textures, middle.texture.id()), Some(rect));
    }
}

#[test]
fn a_tool_button_clicks_with_the_skin_and_with_the_flat_fallback() {
    let ctx = egui::Context::default();
    let skins = [
        Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
        Skin::fallback(),
    ];
    // Con el skin, el tramo del medio mide 23×22; el respaldo, 23×23.
    for (skin, size) in skins.iter().zip([vec2(23.0, 22.0), vec2(23.0, 23.0)]) {
        let middle = SlotPos::Middle;
        let (rect, _, _) = tool(&ctx, skin, middle, false, vec![]);
        assert_eq!(rect.size(), size);
        let at = rect.center();
        tool(&ctx, skin, middle, false, vec![Event::PointerMoved(at)]);
        tool(&ctx, skin, middle, false, vec![press(at, true)]);
        let (_, clicked, _) = tool(&ctx, skin, middle, false, vec![press(at, false)]);
        assert!(clicked);
    }
}

/// Un frame con un botón de prender y apagar: su rect, si se hizo clic y las texturas
/// dibujadas.
fn toggle(
    ctx: &egui::Context,
    skin: &Skin,
    icon: &str,
    on: bool,
    events: Vec<Event>,
) -> (Rect, bool, Vec<egui::TextureId>) {
    let mut out = (Rect::NOTHING, false);
    let input = RawInput { events, ..screen() };
    let mut output = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let r = toggle_button(ui, skin, icon, "Vertical axis", on, |r, color| {
                vec![Shape::rect_filled(r, 0.0, color)]
            });
            out = (r.rect, r.clicked());
        });
    });
    output.textures_delta.clear();
    let mut textures = Vec::new();
    for clipped in &output.shapes {
        collect_textures(&clipped.shape, &mut textures);
    }
    (out.0, out.1, textures)
}

#[test]
fn a_toggle_is_the_symmetry_slot_with_its_icon_and_the_active_slot_when_on() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let id = |name: &str| skin.piece(name, WidgetState::Normal).unwrap().texture.id();
    let (rect, _, textures) = toggle(&ctx, &skin, "icon.sym_vertical", false, vec![]);
    assert_eq!(rect.size(), vec2(15.0, 16.0), "the piece size");
    assert!(textures.contains(&id("sym.slot")));
    assert!(textures.contains(&id("icon.sym_vertical")));
    let (_, _, textures) = toggle(&ctx, &skin, "icon.sym_vertical", true, vec![]);
    assert!(textures.contains(&id("sym.slot.active")));
    assert!(!textures.contains(&id("sym.slot")));
}

#[test]
fn a_toggle_clicks_with_the_skin_and_with_the_flat_fallback() {
    let ctx = egui::Context::default();
    let skins = [
        Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
        Skin::fallback(),
    ];
    for skin in &skins {
        // Sin el ícono (`icon.nope`), dibuja el respaldo.
        let (rect, _, _) = toggle(&ctx, skin, "icon.nope", false, vec![]);
        assert_eq!(rect.size(), vec2(15.0, 16.0));
        let at = rect.center();
        toggle(
            &ctx,
            skin,
            "icon.nope",
            false,
            vec![Event::PointerMoved(at)],
        );
        toggle(&ctx, skin, "icon.nope", false, vec![press(at, true)]);
        let (_, clicked, _) = toggle(&ctx, skin, "icon.nope", false, vec![press(at, false)]);
        assert!(clicked);
    }
}

/// Los colores del skin embebido.
fn mockup_colors() -> SkinColors {
    let ctx = egui::Context::default();
    *Skin::load(&ctx, crate::skin::EMBEDDED).unwrap().colors()
}

/// Los píxeles físicos de un rect a `ppp`.
fn physical(r: Rect, ppp: f32) -> [f32; 4] {
    [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp]
}

#[test]
fn the_etched_lines_are_one_dark_and_one_light_pixel_of_the_mockup() {
    let colors = mockup_colors();
    let dark = Color32::from_rgb(189, 164, 153);
    let light = Color32::from_rgb(241, 235, 219);
    assert_eq!((colors.canvas, colors.highlight), (dark, light));
    for ppp in [1.0, 2.0, 3.0] {
        // Horizontal: la oscura en la fila 69 y el brillo debajo, en la 70.
        let h = colored_rects(&etched_hline(Rangef::new(28.0, 978.0), 69.0, &colors, ppp));
        assert_eq!(
            h,
            vec![
                (
                    dark,
                    Rect::from_min_max(pos2(28.0, 69.0), pos2(978.0, 70.0))
                ),
                (
                    light,
                    Rect::from_min_max(pos2(28.0, 70.0), pos2(978.0, 71.0))
                ),
            ],
            "at {ppp}"
        );
        // Vertical: la oscura en la columna 852 y el brillo a la derecha, en la 853.
        let v = colored_rects(&etched_vline(852.0, Rangef::new(70.0, 88.0), &colors, ppp));
        assert_eq!(
            v,
            vec![
                (
                    dark,
                    Rect::from_min_max(pos2(852.0, 70.0), pos2(853.0, 88.0))
                ),
                (
                    light,
                    Rect::from_min_max(pos2(853.0, 70.0), pos2(854.0, 88.0))
                ),
            ],
            "at {ppp}"
        );
        // Cada línea mide `ppp` píxeles físicos.
        let [_, top, _, bottom] = physical(h[0].1, ppp);
        assert_eq!(bottom - top, ppp);
    }
}

#[test]
fn the_etched_lines_snap_to_physical_pixels_and_are_not_blurred() {
    let colors = mockup_colors();
    for ppp in [1.0, 2.0, 3.0] {
        let shapes = [
            etched_hline(Rangef::new(10.3, 50.6), 20.4, &colors, ppp),
            etched_vline(30.7, Rangef::new(5.2, 40.1), &colors, ppp),
        ];
        for shape in &shapes {
            let rects = colored_rects(shape);
            assert_eq!(rects.len(), 2, "{shape:?}");
            for (_, r) in rects {
                for v in physical(r, ppp) {
                    assert_eq!(v, v.round(), "{r:?} at {ppp}");
                }
            }
        }
        // Teselado: solo los dos colores, sin vértices semitransparentes de difuminado.
        let mut tessellator = egui::epaint::Tessellator::new(
            ppp,
            egui::epaint::TessellationOptions::default(),
            [1, 1],
            Vec::new(),
        );
        let clipped: Vec<egui::epaint::ClippedShape> = shapes
            .iter()
            .map(|s| egui::epaint::ClippedShape {
                clip_rect: Rect::EVERYTHING,
                shape: s.clone(),
            })
            .collect();
        for primitive in tessellator.tessellate_shapes(clipped) {
            let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                panic!("only meshes");
            };
            for v in mesh.vertices {
                assert!(
                    v.color == colors.canvas || v.color == colors.highlight,
                    "{:?} at {ppp}",
                    v.color
                );
            }
        }
    }
}

#[test]
fn the_toolbar_is_5_points_the_box_and_5_points() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let frame = skin.piece(BOX_PIECE, WidgetState::Normal).unwrap().size[0] as f32;
    assert_eq!(frame, 27.0);
    assert_eq!(toolbar_width(&skin), 5.0 + frame + 5.0);
    assert_eq!(toolbar_width(&skin), 37.0, "the mockup");
    // El respaldo: el botón de 23 con 1 px de aire y 1 de línea a cada lado.
    assert_eq!(toolbar_width(&Skin::fallback()), 37.0, "the fallback");
}

/// Un frame con dos botones dentro de la caja de la barra: el rect de la caja (lo que
/// devuelve `toolbar_box`), el de cada botón y las texturas dibujadas.
fn boxed(ctx: &egui::Context, skin: &Skin) -> (Rect, [Rect; 2], Vec<(egui::TextureId, Rect)>) {
    let mut out = (Rect::NOTHING, [Rect::NOTHING; 2]);
    let textures = run(ctx, screen(), |ui| {
        let mut buttons = [Rect::NOTHING; 2];
        let frame = toolbar_box(ui, skin, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, slot) in [SlotPos::First, SlotPos::Last].into_iter().enumerate() {
                let id = egui::Id::new(("boxed", i));
                let r = tool_button(ui, id, skin, slot, "icon.pencil", "Pencil", false, false);
                buttons[i] = r.rect;
            }
        });
        out = (frame, buttons);
    });
    (out.0, out.1, textures)
}

#[test]
fn the_toolbar_box_goes_around_the_buttons_with_one_pixel_of_air() {
    let ctx = egui::Context::default();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let (frame, [first, last], textures) = boxed(&ctx, &skin);
    let id = skin
        .piece(BOX_PIECE, WidgetState::Normal)
        .unwrap()
        .texture
        .id();
    assert_eq!(
        drawn(&textures, id),
        Some(frame),
        "the box piece, stretched"
    );
    // 1 px de línea y 1 de aire: los botones empiezan 2 px adentro, en los dos ejes.
    assert_eq!(
        first.min - frame.min,
        vec2(2.0, 2.0),
        "{first:?} in {frame:?}"
    );
    assert_eq!(
        frame.max - last.max,
        vec2(2.0, 2.0),
        "{last:?} in {frame:?}"
    );
    assert_eq!(frame.size(), vec2(27.0, 22.0 + 23.0 + 4.0));
    assert_eq!(last.min.y, first.max.y, "no gap");
    // Sin la pieza, la caja de respaldo mide lo mismo y los botones van igual.
    let fallback = Skin::fallback();
    let (frame, [first, last], _) = boxed(&ctx, &fallback);
    assert_eq!(frame.width(), 27.0);
    assert_eq!(first.min - frame.min, vec2(2.0, 2.0));
    assert_eq!(frame.max - last.max, vec2(2.0, 2.0));
}

#[test]
fn the_variants_mark_is_a_three_pixel_triangle_in_the_corner() {
    for ppp in [1.0, 2.0, 3.0] {
        let rows = more_mark(pos2(21.0, 21.0), ppp);
        assert_eq!(
            rows,
            [
                Rect::from_min_max(pos2(20.0, 18.0), pos2(21.0, 19.0)),
                Rect::from_min_max(pos2(19.0, 19.0), pos2(21.0, 20.0)),
                Rect::from_min_max(pos2(18.0, 20.0), pos2(21.0, 21.0)),
            ],
            "at {ppp}"
        );
    }
}

#[test]
fn a_pressed_label_goes_one_point_down() {
    let inner = Rect::from_min_size(pos2(10.0, 20.0), vec2(43.0, 10.0));
    let size = vec2(20.3, 7.0);
    for ppp in [1.0, 2.0, 3.0] {
        let normal = label_pos(inner, size, WidgetState::Normal, ppp);
        let hover = label_pos(inner, size, WidgetState::Hover, ppp);
        let down = label_pos(inner, size, WidgetState::Down, ppp);
        assert_eq!(hover, normal);
        assert_eq!(down - normal, vec2(0.0, 1.0), "at {ppp}");
    }
}

#[test]
fn wrap_rows_keeps_each_group_whole_and_moves_what_does_not_fit_to_the_next_row() {
    // Como el encabezado del editor de color: copiar (25), RGBA (116) y hex (46), con 2 de
    // aire entre los dos primeros y 4 menos al juntar RGBA con el hex (comparten la punta).
    let widths = [25.0, 116.0, 46.0];
    let joins = [2.0, -4.0];
    // Todo junto mide 25 + 2 + 116 - 4 + 46 = 185.
    assert_eq!(wrap_rows(185.0, &widths, &joins), vec![0..3]);
    assert_eq!(wrap_rows(400.0, &widths, &joins), vec![0..3]);
    assert_eq!(wrap_rows(184.0, &widths, &joins), vec![0..2, 2..3]);
    assert_eq!(wrap_rows(143.0, &widths, &joins), vec![0..2, 2..3]);
    assert_eq!(wrap_rows(142.0, &widths, &joins), vec![0..1, 1..2, 2..3]);
    // Más angosto que un grupo: igual va entero, solo en su fila.
    assert_eq!(wrap_rows(50.0, &widths, &joins), vec![0..1, 1..2, 2..3]);
    assert_eq!(wrap_rows(10.0, &[25.0], &[]), vec![0..1]);
    assert!(wrap_rows(10.0, &[], &[]).is_empty());
}
