use cs_core::{ColorMode, Sprite};
use eframe::egui::{Event, PointerButton, Pos2, RawInput, pos2};

use super::*;

#[test]
fn tile_thumbnails_are_cached_until_a_tile_image_changes() {
    let ctx = egui::Context::default();
    let mut e = crate::tile_ops::test_util::tile_editor();
    let mut thumbs = TileThumbs::default();
    thumbs.refresh(&ctx, &e);
    assert_eq!(thumbs.textures.len(), 3, "the empty tile and tiles 1 and 2");
    let first = thumbs.textures[1].id();
    thumbs.refresh(&ctx, &e);
    assert_eq!(thumbs.textures[1].id(), first, "cached");
    let cmd = cs_commands::ops::set_tiles(
        e.sprite(),
        e.layer,
        0,
        &[(0, 0, cs_core::Tile::new(1, false, false, false))],
    )
    .unwrap();
    e.execute(cmd);
    thumbs.refresh(&ctx, &e);
    assert_eq!(
        thumbs.textures[1].id(),
        first,
        "a set_tiles stroke keeps the thumbnails"
    );
    let mut img = cs_core::Image::new(4, 4, ColorMode::Rgba);
    img.set(1, 1, cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(9, 9, 9)))
        .unwrap();
    let cmd = cs_commands::ops::set_tile_image(e.sprite(), 0, 1, img).unwrap();
    e.execute(cmd);
    thumbs.refresh(&ctx, &e);
    assert_ne!(
        thumbs.textures[1].id(),
        first,
        "rebuilt after replacing a tile image"
    );
    e.select_layer(cs_core::LayerId(1));
    thumbs.refresh(&ctx, &e);
    assert!(thumbs.textures.is_empty(), "no tileset on an image layer");
}

/// Corre un frame con un slider de opacidad conectado como en el panel de instancia;
/// devuelve el rect del slider.
fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<Event>) -> Rect {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
        events,
        ..Default::default()
    };
    let mut rect = Rect::NOTHING;
    let mut output = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut opacity = editor.instance_key().unwrap().opacity;
            let r = ui.add(egui::Slider::new(&mut opacity, 0..=255));
            if let Some(group) = gesture_group(editor, &[&r]) {
                editor.update_instance_key_grouped(Some(group), |k| k.opacity = opacity);
            }
            gesture_end(editor, &[&r]);
            rect = r.rect;
        });
    });
    output.textures_delta.clear();
    rect
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

#[test]
fn slider_click_then_drag_is_one_undo_step() {
    let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
    e.add_artboard(4, 4);
    e.activate(cs_core::ArtboardId(1));
    e.add_instance_layer(cs_core::ArtboardId(2));
    let original = e.instance_key().unwrap();
    let ctx = egui::Context::default();
    let r = frame(&ctx, &mut e, vec![]);
    let y = r.center().y;
    let at = |t: f32| pos2(r.left() + r.width() * t, y);
    // Clic en el 20 % (el valor salta), arrastre hasta el 30 % y suelta.
    frame(&ctx, &mut e, vec![Event::PointerMoved(at(0.2))]);
    frame(&ctx, &mut e, vec![button(at(0.2), true)]);
    for t in [0.22, 0.25, 0.3] {
        frame(&ctx, &mut e, vec![Event::PointerMoved(at(t))]);
    }
    frame(&ctx, &mut e, vec![button(at(0.3), false)]);
    frame(&ctx, &mut e, vec![]);
    let dragged = e.instance_key().unwrap().opacity;
    assert_ne!(dragged, original.opacity, "the slider moved");
    // Un segundo clic es otro paso.
    frame(&ctx, &mut e, vec![Event::PointerMoved(at(0.6))]);
    frame(&ctx, &mut e, vec![button(at(0.6), true)]);
    frame(&ctx, &mut e, vec![button(at(0.6), false)]);
    frame(&ctx, &mut e, vec![]);
    assert_ne!(e.instance_key().unwrap().opacity, dragged);
    e.undo();
    assert_eq!(e.instance_key().unwrap().opacity, dragged);
    e.undo();
    assert_eq!(e.instance_key().unwrap(), original);
}

/// Corre un frame con el grupo `group` de la barra en el instante `time`; devuelve el rect
/// de su botón.
fn group_frame(
    ctx: &egui::Context,
    editor: &mut Editor,
    group: usize,
    time: f64,
    events: Vec<Event>,
) -> Rect {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
        time: Some(time),
        events,
        ..Default::default()
    };
    let mut rect = Rect::NOTHING;
    let mut output = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            rect = tool_group(ui, editor, group).rect;
        });
    });
    output.textures_delta.clear();
    rect
}

fn mouse(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    }
}

fn escape() -> Event {
    Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

#[test]
fn right_click_or_holding_a_group_button_opens_its_variants() {
    use PointerButton::{Primary, Secondary};
    use egui::Popup;
    let ctx = egui::Context::default();
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    let bucket = ToolKind::GROUPS
        .iter()
        .position(|g| g.contains(&ToolKind::Bucket))
        .unwrap();
    let c = group_frame(&ctx, &mut e, bucket, 0.0, vec![]).center();
    group_frame(&ctx, &mut e, bucket, 0.05, vec![Event::PointerMoved(c)]);

    // Clic derecho: abre la lista y no cambia de herramienta.
    group_frame(&ctx, &mut e, bucket, 0.1, vec![mouse(c, Secondary, true)]);
    group_frame(&ctx, &mut e, bucket, 0.15, vec![mouse(c, Secondary, false)]);
    group_frame(&ctx, &mut e, bucket, 0.2, vec![]);
    assert!(Popup::is_any_open(&ctx), "right-click opens the list");
    assert_eq!(e.tool, ToolKind::Pencil);
    group_frame(&ctx, &mut e, bucket, 0.25, vec![escape()]);
    group_frame(&ctx, &mut e, bucket, 0.3, vec![]);
    assert!(!Popup::is_any_open(&ctx));

    // Mantener presionado: a los 400 ms se abre; al soltar sigue abierta y no elige.
    group_frame(&ctx, &mut e, bucket, 1.0, vec![mouse(c, Primary, true)]);
    group_frame(&ctx, &mut e, bucket, 1.2, vec![]);
    assert!(!Popup::is_any_open(&ctx), "not yet");
    group_frame(&ctx, &mut e, bucket, 1.45, vec![]);
    group_frame(&ctx, &mut e, bucket, 1.5, vec![]);
    assert!(Popup::is_any_open(&ctx), "held for 400 ms");
    group_frame(&ctx, &mut e, bucket, 1.55, vec![mouse(c, Primary, false)]);
    group_frame(&ctx, &mut e, bucket, 1.6, vec![]);
    assert!(Popup::is_any_open(&ctx), "releasing keeps it open");
    assert_eq!(e.tool, ToolKind::Pencil, "and does not pick the tool");
    group_frame(&ctx, &mut e, bucket, 1.65, vec![escape()]);
    group_frame(&ctx, &mut e, bucket, 1.7, vec![]);
    assert!(!Popup::is_any_open(&ctx));

    // Un clic corto elige la herramienta del grupo.
    group_frame(&ctx, &mut e, bucket, 2.0, vec![mouse(c, Primary, true)]);
    group_frame(&ctx, &mut e, bucket, 2.1, vec![mouse(c, Primary, false)]);
    group_frame(&ctx, &mut e, bucket, 2.15, vec![]);
    assert_eq!(e.tool, ToolKind::Bucket);
    assert!(!Popup::is_any_open(&ctx));
}

#[test]
fn the_shape_buttons_pick_round_or_square() {
    use cs_tools::BrushShape;
    let ctx = egui::Context::default();
    let run = |shape: &mut BrushShape, events: Vec<Event>| -> [Rect; 2] {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
            events,
            ..Default::default()
        };
        let mut rects = [Rect::NOTHING; 2];
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                rects = brush_shape_buttons(ui, shape, 20.0).map(|r| r.rect);
            });
        });
        output.textures_delta.clear();
        rects
    };
    let click = |shape: &mut BrushShape, p: Pos2| {
        run(shape, vec![Event::PointerMoved(p)]);
        run(shape, vec![mouse(p, PointerButton::Primary, true)]);
        run(shape, vec![mouse(p, PointerButton::Primary, false)]);
    };
    let mut shape = BrushShape::Round;
    let [round, square] = run(&mut shape, vec![]);
    assert!(
        round.width() > 0.0 && square.left() >= round.right(),
        "side by side"
    );
    click(&mut shape, square.center());
    assert_eq!(shape, BrushShape::Square);
    click(&mut shape, round.center());
    assert_eq!(shape, BrushShape::Round);
}

/// La columna de íconos con el skin embebido dentro del scroll del panel, manejada con
/// eventos crudos (el reloj avanza 50 ms por frame). Guarda los botones del árbol de
/// accesibilidad (nombre y rect), las texturas dibujadas y el desplazamiento del scroll.
struct ColumnHarness {
    ctx: egui::Context,
    editor: Editor,
    skin: Skin,
    time: f64,
    /// Alto de la pantalla: con 150 la columna (13 × 22) no entra y hay scroll.
    height: f32,
    buttons: Vec<(String, Rect)>,
    textures: Vec<egui::TextureId>,
    /// Lo dibujado con cada textura y su rect.
    meshes: Vec<(egui::TextureId, Rect)>,
    /// Los textos dibujados: el texto, su rect y su color.
    texts: Vec<(String, Rect, Color32)>,
    /// Los textos del árbol de accesibilidad (rol `Label`).
    label_nodes: Vec<String>,
    offset: f32,
    /// Sin Pixel UI: la barra con nombres (`toolbar` sin skin) en vez de la columna.
    named: bool,
}

impl ColumnHarness {
    fn new(height: f32) -> Self {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        Self::with_skin(ctx, skin, height)
    }

    fn with_skin(ctx: egui::Context, skin: Skin, height: f32) -> Self {
        ctx.enable_accesskit();
        let mut h = Self {
            ctx,
            editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
            skin,
            time: 0.0,
            height,
            buttons: Vec::new(),
            textures: Vec::new(),
            meshes: Vec::new(),
            texts: Vec::new(),
            label_nodes: Vec::new(),
            offset: 0.0,
            named: false,
        };
        h.frame(vec![]);
        h
    }

    /// Con la fuente pixel y los colores del skin, como con Pixel UI encendido. La fuente rige
    /// desde el frame siguiente.
    fn pixel_style(mut self) -> Self {
        self.ctx
            .set_fonts(crate::appearance::font_definitions(true));
        crate::appearance::apply_style(&self.ctx, Some(self.skin.colors()));
        self.frame(vec![]);
        self
    }

    /// Pixel UI apagado: la barra con nombres, sin skin.
    fn named(mut self) -> Self {
        self.named = true;
        self.frame(vec![]);
        self
    }

    /// Deja el puntero quieto sobre el botón `label` hasta que pasa la demora del tooltip.
    /// Devuelve el rect del botón.
    fn hover(&mut self, label: &str) -> Rect {
        let rect = self.rect(label);
        self.frame(vec![Event::PointerMoved(rect.center())]);
        let delay = self.ctx.global_style().interaction.tooltip_delay as f64;
        self.frame_after(delay + 0.1, vec![]);
        self.frame(vec![]);
        // Que termine de aparecer (el área de egui entra con un fundido).
        self.frame_after(0.5, vec![]);
        rect
    }

    fn rect(&self, label: &str) -> Rect {
        self.buttons
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.frame_after(0.05, events);
    }

    fn frame_after(&mut self, dt: f64, events: Vec<Event>) {
        self.time += dt;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(200.0, self.height),
            )),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let (editor, skin, named) = (&mut self.editor, &self.skin, self.named);
        let mut offset = 0.0;
        let mut output = self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                offset = tools_scroll_area()
                    .show(ui, |ui| {
                        if named {
                            toolbar(ui, editor, None);
                        } else {
                            tool_column(ui, editor, skin);
                        }
                    })
                    .state
                    .offset
                    .y;
            });
        });
        output.textures_delta.clear();
        self.offset = offset;
        self.meshes.clear();
        self.texts.clear();
        for clipped in &output.shapes {
            collect_meshes(&clipped.shape, &mut self.meshes);
            collect_texts(&clipped.shape, &mut self.texts);
        }
        self.label_nodes = output
            .platform_output
            .accesskit_update
            .as_ref()
            .map(|update| {
                update
                    .nodes
                    .iter()
                    .filter(|(_, n)| n.role() == egui::accesskit::Role::Label)
                    .filter_map(|(_, n)| n.label().or(n.value()).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        self.textures = self.meshes.iter().map(|(t, _)| *t).collect();
        self.buttons = output
            .platform_output
            .accesskit_update
            .map(|update| {
                update
                    .nodes
                    .iter()
                    .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
                    .filter_map(|(_, n)| {
                        let b = n.bounds()?;
                        let rect = Rect::from_min_max(
                            pos2(b.x0 as f32, b.y0 as f32),
                            pos2(b.x1 as f32, b.y1 as f32),
                        );
                        Some((n.label()?.to_string(), rect))
                    })
                    // Solo los botones de herramienta: sin los de la caja de los colores.
                    .filter(|(l, _)| !fg_bg::LABELS.contains(&l.as_str()))
                    .collect()
            })
            .unwrap_or_default();
    }

    fn at(&self, label: &str) -> Pos2 {
        self.buttons
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
    }

    fn labels(&self) -> Vec<&str> {
        self.buttons.iter().map(|(l, _)| l.as_str()).collect()
    }

    fn press(&mut self, label: &str, button: PointerButton) -> Pos2 {
        let pos = self.at(label);
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![mouse(pos, button, true)]);
        pos
    }

    fn click(&mut self, label: &str, button: PointerButton) {
        let pos = self.press(label, button);
        self.frame(vec![mouse(pos, button, false)]);
        self.frame(vec![]);
    }

    fn texture(&self, piece: &str) -> egui::TextureId {
        self.skin
            .piece(piece, crate::skin::WidgetState::Normal)
            .unwrap()
            .texture
            .id()
    }
}

fn collect_meshes(shape: &egui::Shape, out: &mut Vec<(egui::TextureId, Rect)>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_meshes(s, out)),
        egui::Shape::Mesh(mesh) => out.push((mesh.texture_id, mesh.calc_bounds())),
        _ => {}
    }
}

fn collect_texts(shape: &egui::Shape, out: &mut Vec<(String, Rect, Color32)>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_texts(s, out)),
        egui::Shape::Text(text) => {
            // El color con que se ve: el de la galería, si lo trae; si no, el de respaldo.
            let color = text
                .galley
                .job
                .sections
                .first()
                .map(|s| s.format.color)
                .filter(|c| *c != Color32::PLACEHOLDER)
                .unwrap_or(text.fallback_color);
            out.push((
                text.galley.text().to_owned(),
                text.visual_bounding_rect(),
                color,
            ));
        }
        _ => {}
    }
}

/// Lo dibujado con `texture`, unido en un rect.
fn drawn_union(h: &ColumnHarness, texture: egui::TextureId) -> Option<Rect> {
    h.meshes
        .iter()
        .filter(|(t, _)| *t == texture)
        .map(|(_, r)| *r)
        .reduce(|a, b| a.union(b))
}

#[test]
fn with_pixel_ui_the_tooltip_is_the_bubble_pointing_at_the_button() {
    let mut h = ColumnHarness::new(600.0).pixel_style();
    let bubble = h.texture("tooltip.bubble");
    assert_eq!(drawn_union(&h, bubble), None, "no bubble before hovering");
    let button = h.hover("Hand");
    let drawn = drawn_union(&h, bubble).expect("the bubble is drawn");
    // La punta de la cola, 6 px a la derecha del botón (el aire del mockup), y su fila de
    // arriba en el centro del botón: el cuerpo empieza 8 filas antes.
    assert_eq!(drawn.left(), button.right() + 6.0, "{drawn:?} {button:?}");
    assert_eq!(drawn.top() + 8.0, button.center().y, "{drawn:?} {button:?}");
    // Una línea en los textos chicos (mayúsculas de 5): 6 de aire arriba y abajo, y la fila
    // de sombra.
    assert_eq!(drawn.height(), 18.0, "{drawn:?}");
    let (text, rect, color) = h
        .texts
        .iter()
        .find(|(t, ..)| t == "Hand (H)")
        .cloned()
        .unwrap_or_else(|| panic!("the tooltip text in {:?}", h.texts));
    assert_eq!(color, h.skin.colors().text, "{text}");
    // El texto va en el cuerpo (sin la cola de 8), con 6 px de aire a la izquierda.
    let body = Rect::from_min_max(drawn.min + Vec2::new(8.0, 0.0), drawn.max);
    assert!(body.contains_rect(rect), "{rect:?} in {body:?}");
    assert_eq!(rect.left(), body.left() + 6.0, "{rect:?} {body:?}");
    // El tooltip sigue teniendo su texto en el árbol de accesibilidad.
    assert!(
        h.label_nodes.iter().any(|l| l == "Hand (H)"),
        "{:?}",
        h.label_nodes
    );
}

#[test]
fn with_pixel_ui_the_bubble_grows_with_the_second_line_of_the_groups() {
    let mut h = ColumnHarness::new(600.0).pixel_style();
    let button = h.hover("Rectangular Marquee");
    let drawn = drawn_union(&h, h.texture("tooltip.bubble")).expect("the bubble is drawn");
    assert_eq!(drawn.left(), button.right() + 6.0, "{drawn:?} {button:?}");
    assert_eq!(drawn.top() + 8.0, button.center().y, "{drawn:?} {button:?}");
    assert!(drawn.height() > 20.0, "two lines: {drawn:?}");
    let text = tool_group::tooltip(&egui::Context::default(), ToolKind::RectSelect, true);
    let (_, rect, _) = h
        .texts
        .iter()
        .find(|(t, ..)| *t == text)
        .cloned()
        .unwrap_or_else(|| panic!("the tooltip text in {:?}", h.texts));
    let body = Rect::from_min_max(drawn.min + Vec2::new(8.0, 0.0), drawn.max);
    assert!(body.contains_rect(rect), "{rect:?} in {body:?}");
    assert_eq!(rect.left(), body.left() + 6.0, "{rect:?} {body:?}");
}

#[test]
fn without_pixel_ui_the_tooltip_is_the_egui_one_and_there_is_no_bubble() {
    let mut h = ColumnHarness::new(600.0).named();
    let hand = h
        .labels()
        .into_iter()
        .find(|l| l.contains("Hand"))
        .expect("the Hand button")
        .to_owned();
    h.hover(&hand);
    assert!(
        h.texts.iter().any(|(t, ..)| t == "Hand (H)"),
        "the egui tooltip: {:?}",
        h.texts
    );
    assert_eq!(drawn_union(&h, h.texture("tooltip.bubble")), None);
}

/// Nombre accesible de una fila de la lista de variantes: el nombre y el atajo.
fn item(kind: ToolKind) -> String {
    format!("{} {}", kind.label(), kind.shortcut())
}

#[test]
fn the_icon_column_has_a_named_button_per_group_and_a_click_picks() {
    let mut h = ColumnHarness::new(600.0);
    let mut column = h.buttons.clone();
    column.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
    let names: Vec<&str> = column.iter().map(|(l, _)| l.as_str()).collect();
    let expected: Vec<&str> = ToolKind::GROUPS.iter().map(|g| g[0].label()).collect();
    assert_eq!(names, expected);
    // Los tramos del mockup: 23×22 y el último, con el borde de abajo, 23×23.
    let last = column.len() - 1;
    for (i, (label, r)) in column.iter().enumerate() {
        let height = if i == last { 23.0 } else { 22.0 };
        assert_eq!(r.size(), Vec2::new(23.0, height), "{label}");
    }
    h.click("Paint Bucket", PointerButton::Primary);
    assert_eq!(h.editor.tool, ToolKind::Bucket);
}

/// Botones de la columna de arriba abajo.
fn column(h: &ColumnHarness) -> Vec<(String, Rect)> {
    let mut column = h.buttons.clone();
    column.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
    column
}

#[test]
fn the_buttons_are_stacked_without_gaps_in_one_column() {
    let skins = |ctx: &egui::Context| {
        [
            Skin::load(ctx, crate::skin::EMBEDDED).unwrap(),
            Skin::fallback(),
        ]
    };
    for i in 0..2 {
        let ctx = egui::Context::default();
        let skin = skins(&ctx).into_iter().nth(i).unwrap();
        let h = ColumnHarness::with_skin(ctx, skin, 600.0);
        let column = column(&h);
        assert_eq!(column.len(), ToolKind::GROUPS.len());
        for pair in column.windows(2) {
            let ((_, above), (label, below)) = (&pair[0], &pair[1]);
            assert_eq!(below.top(), above.bottom(), "{label}: no gap ({i})");
            assert_eq!(below.left(), above.left(), "{label}: one column ({i})");
        }
    }
}

#[test]
fn the_first_button_is_rounded_on_top_the_last_below_and_the_active_one_is_framed_in_pink() {
    let h = ColumnHarness::new(600.0);
    let column = column(&h);
    // Los botones que se dibujaron enteros con `piece` (sus partes del 9-slice, unidas, cubren
    // el botón) y si quedan partes de `piece` fuera de los botones.
    let drawn = |h: &ColumnHarness, piece: &str| -> Vec<Rect> {
        let id = h.texture(piece);
        let parts: Vec<Rect> = h
            .meshes
            .iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, r)| *r)
            .collect();
        assert!(
            parts
                .iter()
                .all(|p| column.iter().any(|(_, r)| r.contains_rect(*p))),
            "{piece}: {parts:?}"
        );
        column
            .iter()
            .map(|(_, r)| *r)
            .filter(|r| {
                let inside = parts.iter().filter(|p| r.contains_rect(**p));
                inside.copied().reduce(|a, b| a.union(b)) == Some(*r)
            })
            .collect()
    };
    let rect = |i: usize| column[i].1;
    let last = column.len() - 1;
    // Con el lápiz (un grupo del medio): el primer tramo arriba, el último abajo y los del
    // medio con `tool.slot`, también el del lápiz (la elegida no se hunde: lleva su marco
    // rosa por dentro del borde).
    assert_eq!(h.editor.tool, ToolKind::Pencil);
    let pencil = column.iter().position(|(l, _)| l == "Pencil").unwrap();
    assert_eq!(drawn(&h, "tool.slot.first"), [rect(0)]);
    assert_eq!(drawn(&h, "tool.slot.last"), [rect(last)]);
    let middle: Vec<Rect> = (1..last).map(rect).collect();
    assert_eq!(drawn(&h, "tool.slot"), middle);
    assert!(drawn(&h, "tool.slot.active").is_empty(), "{pencil}");
}

#[test]
fn the_column_goes_in_the_toolbar_box_with_one_pixel_of_air() {
    let h = ColumnHarness::new(600.0);
    let column = column(&h);
    let id = h.texture("toolbar.box");
    let frame = h
        .meshes
        .iter()
        .filter(|(t, _)| *t == id)
        .map(|(_, r)| *r)
        .reduce(|a, b| a.union(b))
        .expect("the box is drawn");
    let (first, last) = (column[0].1, column[column.len() - 1].1);
    assert_eq!(first.min - frame.min, Vec2::splat(2.0), "{frame:?}");
    assert_eq!(frame.max - last.max, Vec2::splat(2.0), "{frame:?}");
    // El mockup: 27 × 291.
    assert_eq!(frame.size(), Vec2::new(27.0, 291.0));
}

#[test]
fn right_click_lists_the_five_marquee_tools_and_holding_opens_the_list_too() {
    use egui::Popup;
    let mut h = ColumnHarness::new(600.0);
    h.click("Rectangular Marquee", PointerButton::Secondary);
    assert!(Popup::is_any_open(&h.ctx));
    for &kind in ToolKind::GROUPS[0] {
        assert!(h.labels().contains(&item(kind).as_str()), "{kind:?}");
    }
    assert_eq!(h.editor.tool, ToolKind::Pencil, "right-click does not pick");
    h.frame(vec![escape()]);
    h.frame(vec![]);
    assert!(!Popup::is_any_open(&h.ctx));

    // Mantener presionado 400 ms abre la lista.
    h.press("Rectangular Marquee", PointerButton::Primary);
    h.frame_after(0.45, vec![]);
    h.frame(vec![]);
    assert!(Popup::is_any_open(&h.ctx), "held for 400 ms");
}

#[test]
fn the_group_shows_and_picks_the_variant_chosen_in_the_list() {
    let mut h = ColumnHarness::new(600.0);
    h.click("Rectangular Marquee", PointerButton::Secondary);
    h.click(&item(ToolKind::Lasso), PointerButton::Primary);
    assert_eq!(h.editor.tool, ToolKind::Lasso);
    h.click("Pencil", PointerButton::Primary);
    assert_eq!(h.editor.tool, ToolKind::Pencil);
    assert!(h.textures.contains(&h.texture("icon.lasso")));
    assert!(!h.textures.contains(&h.texture("icon.marquee")));
    // El botón del grupo ahora es Lasso: un clic vuelve a Lasso.
    h.click("Lasso", PointerButton::Primary);
    assert_eq!(h.editor.tool, ToolKind::Lasso);
}

/// Ids de los botones de la columna (por nombre), con o sin un widget antes de la columna.
fn column_ids(prefix: bool) -> Vec<(String, egui::accesskit::NodeId)> {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    let mut output = ctx.run_ui(RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            if prefix {
                let _ = ui.button("Before");
            }
            tool_column(ui, &mut editor, &skin);
        });
    });
    output.textures_delta.clear();
    let mut ids: Vec<_> = output
        .platform_output
        .accesskit_update
        .unwrap()
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
        .filter_map(|(id, n)| Some((n.label()?.to_string(), *id)))
        .filter(|(l, _)| l != "Before" && !fg_bg::LABELS.contains(&l.as_str()))
        .collect();
    ids.sort();
    ids
}

#[test]
fn each_group_button_has_its_own_id_that_does_not_depend_on_what_comes_before() {
    let plain = column_ids(false);
    assert_eq!(plain.len(), ToolKind::GROUPS.len());
    let mut unique: Vec<_> = plain.iter().map(|(_, id)| *id).collect();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), plain.len());
    assert_eq!(plain, column_ids(true));
}

#[test]
fn dragging_over_a_tool_button_does_not_scroll_the_panel() {
    let mut h = ColumnHarness::new(150.0);
    // Con una pantalla táctil, egui arrastra el contenido de un ScrollArea por defecto.
    let touch = |phase| Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(1),
        phase,
        pos: pos2(199.0, 1.0),
        force: None,
    };
    h.frame(vec![
        touch(egui::TouchPhase::Start),
        touch(egui::TouchPhase::End),
    ]);
    assert!(h.ctx.input(|i| i.has_touch_screen()));
    let from = h.press("Pencil", PointerButton::Primary);
    for dy in [10.0, 25.0, 40.0] {
        h.frame(vec![Event::PointerMoved(from - Vec2::new(0.0, dy))]);
    }
    h.frame(vec![mouse(
        from - Vec2::new(0.0, 40.0),
        PointerButton::Primary,
        false,
    )]);
    h.frame(vec![]);
    assert_eq!(h.offset, 0.0, "the panel did not scroll");
}

#[test]
fn holding_a_tool_button_in_the_scrolling_panel_opens_its_variants() {
    let mut h = ColumnHarness::new(150.0);
    h.press("Pencil", PointerButton::Primary);
    h.frame_after(0.45, vec![]);
    h.frame(vec![]);
    assert!(egui::Popup::is_any_open(&h.ctx));
    assert!(h.labels().contains(&item(ToolKind::Spray).as_str()));
    assert_eq!(h.offset, 0.0);
}

/// Nombre (o texto, en las etiquetas) y rect de cada nodo del árbol de accesibilidad.
fn nodes(output: &egui::FullOutput) -> Vec<(String, Rect)> {
    let Some(update) = &output.platform_output.accesskit_update else {
        return Vec::new();
    };
    update
        .nodes
        .iter()
        .filter_map(|(_, n)| {
            let b = n.bounds()?;
            let text = n.label().or_else(|| n.value())?.to_string();
            let rect = Rect::from_min_max(
                pos2(b.x0 as f32, b.y0 as f32),
                pos2(b.x1 as f32, b.y1 as f32),
            );
            Some((text, rect))
        })
        .collect()
}

/// La barra de opciones en su panel de arriba, como en la app, manejada con eventos crudos (el
/// reloj avanza 50 ms por frame). Guarda el rect del panel y los nodos de accesibilidad.
struct BarHarness {
    ctx: egui::Context,
    editor: Editor,
    /// Con Pixel UI: el skin embebido (y el estilo de `apply_style` con sus colores).
    skin: Option<Skin>,
    time: f64,
    width: f32,
    bar: Rect,
    nodes: Vec<(String, Rect)>,
    /// Lo dibujado en el último frame.
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl BarHarness {
    fn new(width: f32) -> Self {
        Self::with_skin(width, false)
    }

    /// Con `pixel`, la barra de Pixel UI: el estilo con los colores del skin y la fuente
    /// pixel, y el skin embebido.
    fn with_skin(width: f32, pixel: bool) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = pixel.then(|| Skin::embedded(&ctx));
        if let Some(skin) = &skin {
            ctx.set_fonts(crate::appearance::font_definitions(true));
            crate::appearance::apply_style(&ctx, Some(skin.colors()));
        }
        let mut h = Self {
            ctx,
            editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
            skin,
            time: 0.0,
            width,
            bar: Rect::NOTHING,
            nodes: Vec::new(),
            shapes: Vec::new(),
        };
        h.frame(vec![]);
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(self.width, 300.0),
            )),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let editor = &mut self.editor;
        let skin = self.skin.as_ref();
        let mut bar = Rect::NOTHING;
        let mut output = self.ctx.run_ui(input, |ui| {
            bar = options_bar_panel(ui.style(), skin)
                .show(ui, |ui| tool_options_bar(ui, editor, skin))
                .response
                .rect;
        });
        output.textures_delta.clear();
        self.bar = bar;
        self.nodes = nodes(&output);
        self.shapes = output.shapes;
    }

    /// Los rects de color liso dibujados (los separadores grabados).
    fn colored_rects(&self) -> Vec<(Color32, Rect)> {
        self.shapes
            .iter()
            .flat_map(|c| pixel_ui::colored_rects(&c.shape))
            .collect()
    }

    /// Las líneas de egui del color de sus separadores (`ui.separator()`, la del panel).
    fn separator_lines(&self) -> usize {
        let color = self
            .ctx
            .global_style()
            .visuals
            .widgets
            .noninteractive
            .bg_stroke
            .color;
        fn count(shape: &egui::Shape, color: Color32) -> usize {
            match shape {
                egui::Shape::LineSegment { stroke, .. } => usize::from(stroke.color == color),
                egui::Shape::Vec(shapes) => shapes.iter().map(|s| count(s, color)).sum(),
                _ => 0,
            }
        }
        self.shapes.iter().map(|c| count(&c.shape, color)).sum()
    }

    fn has(&self, text: &str) -> bool {
        self.nodes.iter().any(|(t, _)| t == text)
    }

    /// Clic con el botón principal en el centro del nodo `text`, en tres frames.
    fn click(&mut self, text: &str) {
        let at = self.rect(text).center();
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![press(true)]);
        self.frame(vec![press(false)]);
    }

    fn rect(&self, text: &str) -> Rect {
        self.nodes
            .iter()
            .find(|(t, _)| t == text)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no {text:?} in {:?}", self.nodes))
    }

    /// Dónde se dibujó la pieza `name` del skin, de izquierda a derecha.
    fn drawn(&self, name: &str) -> Vec<Rect> {
        let skin = self.skin.as_ref().expect("with Pixel UI");
        let id = skin
            .piece(name, crate::skin::WidgetState::Normal)
            .unwrap()
            .texture
            .id();
        let mut meshes = Vec::new();
        for clipped in &self.shapes {
            collect_meshes(&clipped.shape, &mut meshes);
        }
        let mut rects: Vec<Rect> = meshes
            .into_iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, r)| r)
            .collect();
        rects.sort_by(|a, b| a.left().total_cmp(&b.left()));
        rects
    }

    /// Como `drawn`, con las partes de un 9-slice (que se tocan) unidas en un rect.
    fn drawn_boxes(&self, name: &str) -> Vec<Rect> {
        let mut boxes: Vec<Rect> = Vec::new();
        for rect in self.drawn(name) {
            match boxes.iter_mut().find(|b| b.intersects(rect)) {
                Some(b) => *b = b.union(rect),
                None => boxes.push(rect),
            }
        }
        boxes
    }

    /// Los textos dibujados en el último frame: texto, tinta (para la fuente pixel, el rect
    /// visible) y color.
    fn texts(&self) -> Vec<(String, Rect, Color32)> {
        let mut texts = Vec::new();
        for clipped in &self.shapes {
            collect_texts(&clipped.shape, &mut texts);
        }
        texts
    }

    /// El color y la tinta del primer texto `text` dibujado.
    fn text(&self, text: &str) -> (Color32, Rect) {
        self.texts()
            .into_iter()
            .find(|(t, _, _)| t == text)
            .map(|(_, r, c)| (c, r))
            .unwrap_or_else(|| panic!("no text {text:?} in {:?}", self.texts()))
    }

    /// Clic en el combo `text` y dos frames más: la lista se mide en el primero (invisible) y
    /// se muestra en el segundo.
    fn open(&mut self, text: &str) {
        self.click(text);
        self.frame(vec![]);
        self.frame(vec![]);
    }

    /// Arrastre con el botón principal desde `from`, `dx` puntos a la derecha en 4 pasos.
    fn drag(&mut self, from: Pos2, dx: f32) {
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![press(from, true)]);
        let mut at = from;
        for _ in 0..4 {
            at.x += dx / 4.0;
            self.frame(vec![Event::PointerMoved(at)]);
        }
        self.frame(vec![press(at, false)]);
        self.frame(vec![]);
    }

    /// Clic con el botón principal en `at`, en tres frames.
    fn click_at(&mut self, at: Pos2) {
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![press(true)]);
        self.frame(vec![press(false)]);
    }

    /// Escribe `text` y aprieta Enter.
    fn type_and_enter(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_owned())]);
        let key = |pressed| Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        self.frame(vec![key(true)]);
        self.frame(vec![key(false)]);
        self.frame(vec![]);
    }
}

#[test]
fn the_options_bar_has_the_brush_with_the_pencil_but_not_with_the_hand() {
    let mut h = BarHarness::new(1200.0);
    assert_eq!(h.editor.tool, ToolKind::Pencil);
    for text in ["Brush size", "Round", "Square", "Pixel-perfect"] {
        assert!(h.has(text), "{text}");
    }
    // Etiqueta y control uno al lado del otro: la forma va a la derecha del texto.
    assert!(h.rect("Round").left() > h.rect("Brush size").right());
    assert!(h.has("Vertical axis (left ↔ right)"), "Symmetry");
    h.editor.set_tool(ToolKind::Hand);
    h.frame(vec![]);
    for text in ["Brush size", "Round", "Square"] {
        assert!(!h.has(text), "{text}");
    }
    assert!(
        h.has("Vertical axis (left ↔ right)"),
        "Symmetry is always there"
    );
    assert!(h.has("Symmetry"));
}

#[test]
fn symmetry_is_at_the_right_end_of_the_bar_in_the_usual_order() {
    let h = BarHarness::new(1200.0);
    let names: Vec<&str> = SYMMETRY_AXES.iter().map(|(_, tip)| *tip).collect();
    let rects: Vec<Rect> = names.iter().map(|n| h.rect(n)).collect();
    for pair in rects.windows(2) {
        assert!(pair[0].right() <= pair[1].left(), "{names:?}: {rects:?}");
    }
    // Después de los ejes, el espejo de la selección, contra el borde derecho.
    let last = rects[rects.len() - 1];
    let mirror = h.rect("Mirror selection");
    assert!(last.right() <= mirror.left(), "{last:?} {mirror:?}");
    assert!(
        h.bar.right() - mirror.right() < 12.0,
        "{mirror:?} in {:?}",
        h.bar
    );
    assert!(h.rect("Symmetry").right() <= rects[0].left());
}

#[test]
fn each_symmetry_button_turns_its_own_axis_on_and_off() {
    let mut h = BarHarness::new(1200.0);
    let [vertical, horizontal, down, up] = SYMMETRY_AXES.map(|(_, name)| name);
    h.click(vertical);
    h.click(horizontal);
    let axes = h.editor.symmetry().axes;
    assert!(axes.vertical && axes.horizontal, "{axes:?}");
    assert!(!axes.diagonal_down && !axes.diagonal_up, "{axes:?}");
    h.click(up);
    h.click(vertical);
    assert_eq!(
        h.editor.symmetry().axes,
        SymmetryAxes {
            horizontal: true,
            diagonal_up: true,
            ..SymmetryAxes::NONE
        }
    );
    assert!(!h.editor.symmetry().axes.diagonal_down, "{down} untouched");
    // Cada clic es un paso de undo.
    h.editor.undo();
    assert!(h.editor.symmetry().axes.vertical);
}

#[test]
fn with_pixel_ui_the_symmetry_buttons_are_the_skin_pieces_and_toggle_their_axis() {
    let mut h = BarHarness::with_skin(1200.0, true);
    let [vertical, horizontal, down, up] = SYMMETRY_AXES.map(|(_, name)| name);
    // Los botones miden lo que la pieza `sym.slot` (15×16, la del mockup) y la forma del
    // pincel, el botón de 25×16 del diseño.
    assert_eq!(h.rect(vertical).size(), Vec2::new(15.0, 16.0));
    assert_eq!(h.rect("Round brush").size(), Vec2::new(25.0, 16.0));
    // En orden y del mismo tamaño, con 2 de aire entre uno y otro.
    let buttons = [vertical, horizontal, down, up].map(|name| h.rect(name));
    for pair in buttons.windows(2) {
        assert_eq!(pair[1].left() - pair[0].right(), 2.0, "{pair:?}");
    }
    // Como en el mockup (frame 4): un botón cada 17 px, 2 filas debajo del borde de arriba
    // de la barra y 3 arriba del grabado de abajo (1 y los 2 de aire), en puntos enteros (con la escala entera,
    // píxeles del skin enteros). Los íconos, de 11×11, 2 px adentro.
    let slots = h.drawn("sym.slot");
    assert_eq!(slots.len(), 4, "{slots:?}");
    for pair in slots.windows(2) {
        assert_eq!(pair[1].left() - pair[0].right(), 2.0, "{pair:?}");
    }
    let names = ["vertical", "horizontal", "diag_down", "diag_up"];
    for (slot, name) in slots.iter().zip(names) {
        assert_eq!(slot.top(), h.bar.top() + 2.0, "{slot:?} in {:?}", h.bar);
        assert_eq!(slot.bottom(), h.bar.bottom() - 2.0 - 3.0, "{slot:?}");
        assert_eq!(slot.left(), slot.left().round(), "{slot:?}");
        let icon = h.drawn(&format!("icon.sym_{name}"));
        let expected = Rect::from_min_size(slot.min + Vec2::splat(2.0), Vec2::splat(11.0));
        assert_eq!(icon, vec![expected], "{name}");
    }
    // De la tinta de "Symmetry" al cuadro del primer botón, 5 px: el texto deja 1 px vacío y
    // la pieza empieza con una columna transparente, así que entre los rects quedan 3. Del
    // último al borde derecho, 3 (en el mockup, la tinta termina en la x 903, los cuadros
    // van de la 909 a la 974 y el área de la app termina en la 977). Las mayúsculas, en la
    // fila 5 de la barra (y 76).
    assert_eq!(slots[0].left() - h.rect("Symmetry").right(), 3.0);
    let (_, ink) = h.text("Symmetry");
    assert_eq!(slots[0].left() + 1.0 - ink.right(), 5.0, "{ink:?}");
    assert_eq!(ink.top(), h.bar.top() + 6.0, "{ink:?}");
    // Después de los ejes, a 5, el botón del espejo (25×16 con su ícono, como en el diseño)
    // y del espejo al borde, 3. Un clic lo prende.
    let mirror = h.rect("Mirror Selection");
    assert_eq!(mirror.size(), Vec2::new(25.0, 16.0));
    assert_eq!(mirror.left() - slots[3].right(), 5.0);
    assert_eq!(h.bar.right() - mirror.right(), 3.0, "{:?}", h.bar);
    assert!(
        h.drawn_boxes("icon.mirror")
            .iter()
            .any(|r| mirror.contains_rect(*r)),
        "the icon"
    );
    h.click("Mirror Selection");
    h.frame(vec![]);
    assert!(h.editor.options.mirror_selection);
    // A 2×, igual: sin medio punto (que sería medio píxel del skin).
    let mut double = BarHarness::with_skin(1200.0, true);
    double.ctx.set_pixels_per_point(2.0);
    double.frame(vec![]);
    double.frame(vec![]);
    for slot in double.drawn("sym.slot") {
        assert_eq!(slot.top(), double.bar.top() + 2.0, "{slot:?}");
    }
    h.click(vertical);
    h.click(horizontal);
    let axes = h.editor.symmetry().axes;
    assert!(axes.vertical && axes.horizontal, "{axes:?}");
}

#[test]
fn the_options_bar_is_one_row_of_the_same_height_with_every_tool() {
    let mut h = BarHarness::new(800.0);
    for &kind in ToolKind::GROUPS.iter().flat_map(|g| g.iter()) {
        h.editor.set_tool(kind);
        h.frame(vec![]);
        assert_eq!(h.bar.height(), OPTIONS_BAR_HEIGHT, "{kind:?}");
        for (text, rect) in &h.nodes {
            assert!(
                rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom(),
                "{kind:?}: {text:?} {rect:?} out of {:?}",
                h.bar
            );
        }
    }
}

#[test]
fn with_pixel_ui_the_hints_go_behind_the_info_icon() {
    let mut h = BarHarness::with_skin(800.0, true);
    h.editor.set_tool(ToolKind::Rectangle);
    h.frame(vec![]);
    // Un solo ícono de 12×13 con las dos ayudas en su tooltip, una por línea.
    let label = "Shift: constrain
Alt+click: pick color";
    let (_, icon) = h
        .nodes
        .iter()
        .find(|(t, _)| t == label)
        .unwrap_or_else(|| panic!("no info icon in {:?}", h.nodes));
    assert_eq!(icon.size(), Vec2::new(12.0, 13.0));
    // Y su texto no está escrito en la barra.
    let written = |shape: &egui::Shape| matches!(shape, egui::Shape::Text(t) if t.galley.text().contains("Alt+click"));
    assert!(!h.shapes.iter().any(|c| written(&c.shape)));
    // Sin Pixel UI, siguen como texto.
    let mut h = BarHarness::new(800.0);
    h.editor.set_tool(ToolKind::Rectangle);
    h.frame(vec![]);
    assert!(h.has("Shift: constrain") && h.has("Alt+click: pick color"));
}

#[test]
fn with_pixel_ui_the_bar_is_23_and_every_node_fits_inside_it() {
    let mut h = BarHarness::with_skin(800.0, true);
    for kind in [
        ToolKind::Pencil,
        ToolKind::RectSelect,
        ToolKind::MagicWand,
        ToolKind::Text,
        ToolKind::Gradient,
    ] {
        h.editor.set_tool(kind);
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(h.bar.height(), 23.0, "{kind:?}");
        assert!(h.nodes.len() > 2, "{kind:?}: {:?}", h.nodes);
        for (text, rect) in &h.nodes {
            assert!(
                rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom(),
                "{kind:?}: {text:?} {rect:?} out of {:?}",
                h.bar
            );
        }
        // Las piezas de los controles, en las 19 filas de arriba del grabado.
        for piece in [
            "field.combo",
            "field.number",
            "field.unit",
            "field.check.on",
            "sym.slot",
            "sym.slot.active",
            "button",
            "button.hover",
            "button.down",
            "button.sep",
            "button.dot.on",
            "button.dot.off",
        ] {
            for rect in h.drawn(piece) {
                assert!(
                    rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom() - 2.0,
                    "{kind:?}: {piece} {rect:?} out of {:?}",
                    h.bar
                );
            }
        }
        // Las herramientas con grupos de radios (Sample, Linear/Radial, Rotation) los
        // dibujan como botones: la prueba de arriba no es vacía para ellos.
        if kind != ToolKind::Pencil && kind != ToolKind::Text {
            assert!(!h.drawn("button.dot.on").is_empty(), "{kind:?}: no buttons");
        }
    }
}

/// La barra con Pixel UI y `tool`, después de dos frames (los textos y las piezas se
/// acomodan en el segundo).
fn pixel_bar(tool: ToolKind) -> BarHarness {
    let mut h = BarHarness::with_skin(1200.0, true);
    h.editor.set_tool(tool);
    h.frame(vec![]);
    h.frame(vec![]);
    h
}

/// Las piezas de los modos: `sym.slot` y `sym.slot.active` a la izquierda de Symmetry.
fn mode_slots(h: &BarHarness) -> (Vec<Rect>, Vec<Rect>) {
    let left_of_symmetry = |r: &Rect| r.right() < h.rect("Symmetry").left();
    let normal = h.drawn("sym.slot").into_iter().filter(left_of_symmetry);
    let active = h
        .drawn("sym.slot.active")
        .into_iter()
        .filter(left_of_symmetry);
    (normal.collect(), active.collect())
}

const MODE_ICONS: [&str; 4] = [
    "icon.sel_replace",
    "icon.sel_add",
    "icon.sel_subtract",
    "icon.sel_intersect",
];

#[test]
fn with_pixel_ui_the_marquee_mode_is_four_icon_buttons_like_in_the_design() {
    let h = pixel_bar(ToolKind::RectSelect);
    let bar = h.bar;
    // Como en el diseño (frame 4, barra desde (28,71)): la tinta de "Mode" en x 35..54 y las
    // mayúsculas en la y 76, en el color del texto.
    let colors = *h.skin.as_ref().unwrap().colors();
    let (color, ink) = h.text("Mode");
    assert_eq!(color, colors.text);
    assert_eq!(ink.min, bar.min + Vec2::new(7.0, 6.0), "{ink:?}");
    assert_eq!(ink.width(), 20.0);
    // Los cuatro cuadritos: las piezas en x 60, 77, 94 y 111 y en la y 73 (la pieza tiene
    // una columna transparente antes del cuadro, que va en 61). Replace es el elegido.
    let (normal, active) = mode_slots(&h);
    let expected: Vec<Rect> = [32.0, 49.0, 66.0, 83.0]
        .map(|x| Rect::from_min_size(bar.min + Vec2::new(x, 2.0), Vec2::new(15.0, 16.0)))
        .into();
    assert_eq!(active, vec![expected[0]]);
    assert_eq!(normal, expected[1..].to_vec());
    // Cada ícono, de 11×11, en el interior de su cuadro (2 px adentro de la pieza).
    for (icon, slot) in MODE_ICONS.into_iter().zip(&expected) {
        let at = Rect::from_min_size(slot.min + Vec2::splat(2.0), Vec2::splat(11.0));
        assert_eq!(h.drawn(icon), vec![at], "{icon}");
    }
    // Un botón por modo, con su nombre; ya no están los radios.
    for mode in SelectionMode::ALL {
        assert_eq!(h.rect(mode.label()).size(), Vec2::new(15.0, 16.0));
        let radio_text = h.texts().into_iter().any(|(t, _, _)| t == mode.label());
        assert!(!radio_text, "{mode:?}: no radio text");
    }
    // Del último cuadro al grupo de Rotation, el aire de entre grupos.
    assert_eq!(h.rect("Rotation:").left() - expected[3].right(), 14.0);
}

#[test]
fn with_pixel_ui_clicking_a_mode_icon_picks_that_mode_only() {
    let mut h = pixel_bar(ToolKind::RectSelect);
    assert_eq!(h.editor.options.selection_mode, SelectionMode::Replace);
    let subtract = SelectionMode::Subtract.label();
    let at = h.rect(subtract);
    h.click(subtract);
    h.frame(vec![]);
    assert_eq!(h.editor.options.selection_mode, SelectionMode::Subtract);
    // El marco activo pasa al botón elegido; los otros tres, normales.
    let (normal, active) = mode_slots(&h);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].left(), at.left().round());
    assert_eq!(normal.len(), 3);
    // Otro clic en el mismo no lo apaga: siempre hay un modo, como con radios.
    h.click(subtract);
    assert_eq!(h.editor.options.selection_mode, SelectionMode::Subtract);
    h.click(SelectionMode::Intersect.label());
    assert_eq!(h.editor.options.selection_mode, SelectionMode::Intersect);
}

#[test]
fn with_pixel_ui_the_mode_buttons_have_the_mode_name_as_tooltip() {
    let mut h = pixel_bar(ToolKind::MagicWand);
    let add = SelectionMode::Add.label();
    // El botón no tiene texto: el nombre aparece solo en el tooltip.
    let drawn = |h: &BarHarness| h.texts().into_iter().any(|(t, _, _)| t == add);
    assert!(!drawn(&h), "no text before hovering");
    let at = h.rect(add).center();
    h.frame(vec![Event::PointerMoved(at)]);
    for _ in 0..20 {
        h.frame(vec![]);
    }
    assert!(drawn(&h), "tooltip {add:?} in {:?}", h.texts());
}

#[test]
fn with_pixel_ui_brush_size_is_the_number_field_with_its_px_unit() {
    let mut h = pixel_bar(ToolKind::Pencil);
    let bar = h.bar;
    // "Brush Size" en x 227..274 del diseño (7 desde el borde) y, 3 px después de la tinta,
    // el campo de x 278 a 304 y la unidad "px" hasta la 319: 42 de ancho en total, de la y
    // 73 a la 88.
    let (_, ink) = h.text("Brush Size");
    assert_eq!(ink.min, bar.min + Vec2::new(7.0, 6.0), "{ink:?}");
    assert_eq!(ink.width(), 46.0);
    let field = h.drawn_boxes("field.number");
    let left = ink.right() + 3.0;
    assert_eq!(
        field,
        vec![Rect::from_min_size(
            pos2(left, bar.top() + 2.0),
            Vec2::new(27.0, 17.0)
        )]
    );
    let unit = h.drawn_boxes("field.unit");
    assert_eq!(
        unit,
        vec![Rect::from_min_size(
            pos2(left + 25.0, bar.top() + 2.0),
            Vec2::new(17.0, 17.0)
        )]
    );
    // El número, en la fila de los textos (los dígitos miden 6, una fila menos que las
    // mayúsculas, y apoyan en la misma línea: terminan en la fila 11) y adentro del blanco.
    let size = h.editor.options.brush_size;
    let (color, digits) = h.text(&size.to_string());
    assert_eq!(color, h.skin.as_ref().unwrap().colors().text);
    assert_eq!(digits.bottom(), bar.top() + 13.0, "{digits:?}");
    assert!(digits.left() > left + 1.0 && digits.right() < left + 26.0);
    // Arrastrar a la derecha lo agranda.
    h.drag(field[0].center(), 40.0);
    let dragged = h.editor.options.brush_size;
    assert!(dragged > size, "{size} -> {dragged}");
    // Un clic lo deja escribir: el número escrito reemplaza al de antes.
    h.click_at(field[0].center());
    h.type_and_enter("12");
    assert_eq!(h.editor.options.brush_size, 12);
    // Fuera del rango, el máximo.
    h.click_at(field[0].center());
    h.type_and_enter("500");
    assert_eq!(h.editor.options.brush_size, MAX_BRUSH_SIZE);
}

#[test]
fn with_pixel_ui_fields_without_a_unit_have_no_px() {
    let h = pixel_bar(ToolKind::MagicWand);
    // Tolerance no tiene unidad: el campo solo.
    let (_, ink) = h.text("Tolerance");
    let field = h.drawn_boxes("field.number");
    assert_eq!(field.len(), 1, "{field:?}");
    assert_eq!(field[0].left(), ink.right() + 3.0);
    assert_eq!(field[0].size(), Vec2::new(27.0, 17.0));
    assert_eq!(h.drawn("field.unit"), vec![]);
    // Spray: el ancho en px y la velocidad sin unidad.
    let h = pixel_bar(ToolKind::Spray);
    assert_eq!(
        h.drawn_boxes("field.number").len(),
        3,
        "brush, width and speed"
    );
    assert_eq!(h.drawn_boxes("field.unit").len(), 2, "brush and width");
}

#[test]
fn with_pixel_ui_a_combo_is_the_field_combo_and_picking_an_option_works() {
    let mut h = pixel_bar(ToolKind::Gradient);
    let bar = h.bar;
    // El tramado es una lista de verdad: el único combo del diseño, de la y 73 a la 88 (la
    // forma, Linear/Radial, son botones).
    let combos = h.drawn_boxes("field.combo");
    assert_eq!(combos.len(), 1, "{combos:?}");
    // El texto, 6 px adentro y en la fila de los textos; 5 px de su tinta a la parte de la
    // flecha, las últimas 10 columnas (en el diseño, x 128..212, tinta en 134..197).
    let current = h.editor.options.dither;
    let (_, ink) = h.text(current.label());
    let combo = combos[0];
    assert_eq!(combo.top(), bar.top() + 2.0);
    assert_eq!(combo.height(), 17.0);
    assert_eq!(ink.left() - combo.left(), 6.0);
    assert_eq!(ink.top(), bar.top() + 6.0);
    assert_eq!(combo.right() - ink.right(), 5.0 + 10.0);
    // Un clic abre la lista en el panel de los menús, colgada del combo.
    let other = Dither::ALL.into_iter().find(|d| *d != current).unwrap();
    assert!(!h.has(other.label()), "closed: only the chosen one");
    h.open(current.label());
    assert!(h.has(other.label()), "{:?}", h.nodes);
    let panel = h.drawn_boxes("menu.panel");
    assert_eq!(panel.len(), 1, "{panel:?}");
    assert_eq!(panel[0].left_top(), combo.left_bottom());
    assert!(panel[0].width() >= combo.width());
    h.click(other.label());
    assert_eq!(h.editor.options.dither, other);
    for _ in 0..10 {
        h.frame(vec![]);
    }
    assert_eq!(h.drawn_boxes("menu.panel"), vec![], "closed after picking");
    assert!(h.has(other.label()) && !h.has(current.label()));
}

/// Las cajas de los botones de opción dibujados (`button`, o `button.hover` con el puntero
/// encima), de izquierda a derecha.
fn option_buttons(h: &BarHarness) -> Vec<Rect> {
    let mut boxes = h.drawn_boxes("button");
    boxes.extend(h.drawn_boxes("button.hover"));
    boxes.sort_by(|a, b| a.left().total_cmp(&b.left()));
    boxes
}

/// Un grupo de radios de la barra con Pixel UI: la herramienta, sus opciones (el nombre de
/// cada botón) y cuál está elegida.
struct RadioGroup {
    tool: ToolKind,
    options: Vec<&'static str>,
    current: fn(&Editor) -> &'static str,
}

fn radio_groups() -> Vec<RadioGroup> {
    vec![
        RadioGroup {
            tool: ToolKind::MagicWand,
            options: vec!["Layer", "All layers"],
            current: |e| {
                if e.options.sample_all {
                    "All layers"
                } else {
                    "Layer"
                }
            },
        },
        RadioGroup {
            tool: ToolKind::Gradient,
            options: vec!["Linear", "Radial"],
            current: |e| match e.options.gradient_shape {
                GradientShape::Linear => "Linear",
                GradientShape::Radial => "Radial",
            },
        },
        RadioGroup {
            tool: ToolKind::RectSelect,
            options: RotationAlgorithm::ALL.map(|r| r.label()).into(),
            current: |e| e.options.rotation.label(),
        },
    ]
}

/// La caja del botón `label` (su nodo, con la pieza dibujada debajo) y si es el elegido (con
/// el punto verde, `button.dot.on`, adentro).
fn button_box(h: &BarHarness, label: &str) -> (Rect, bool) {
    // El nodo del botón es el botón dibujado (los del grupo se tocan: sus piezas se juntan
    // en una caja).
    let b = h.rect(label);
    assert!(
        option_buttons(h).iter().any(|g| g.contains_rect(b)),
        "no button under {label:?}"
    );
    let on = h
        .drawn_boxes("button.dot.on")
        .iter()
        .any(|d| b.contains_rect(*d));
    let off = h
        .drawn_boxes("button.dot.off")
        .iter()
        .any(|d| b.contains_rect(*d));
    assert!(on != off, "{label}: one dot");
    (b, on)
}

#[test]
fn with_pixel_ui_each_radio_option_is_a_button_of_the_group_with_its_dot() {
    for group in radio_groups() {
        let h = pixel_bar(group.tool);
        let bar = h.bar;
        let tool = group.tool;
        let current = (group.current)(&h.editor);
        let colors = *h.skin.as_ref().unwrap().colors();
        // Un botón por opción, como en el diseño: el elegido con el punto verde y su texto en
        // el color del texto; los demás, con el anillo gris y el texto apagado.
        let options: Vec<Rect> = group
            .options
            .iter()
            .map(|label| {
                let (b, on) = button_box(&h, label);
                assert_eq!(on, *label == current, "{tool:?}: {label}");
                // En las filas de los campos (la y 73 a la 88 del diseño).
                assert_eq!(b.top(), bar.top() + 2.0, "{tool:?}: {label} {b:?}");
                assert_eq!(b.height(), 16.0, "{tool:?}: {label}");
                let (color, ink) = h.text(label);
                let want = if on {
                    colors.text
                } else {
                    colors.option_text_off
                };
                assert_eq!(color, want, "{tool:?}: {label}");
                assert_eq!(ink.top(), bar.top() + 6.0, "{tool:?}: {label} {ink:?}");
                // La tinta 12 px adentro (el punto de 7 y su aire) y 4 antes del borde.
                assert_eq!(ink.left(), b.left() + 12.0, "{tool:?}: {label}");
                assert_eq!(ink.right() + 5.0, b.right(), "{tool:?}: {label}");
                b
            })
            .collect();
        // Pegados: cada uno empieza en la última columna del anterior, con el separador.
        for pair in options.windows(2) {
            assert_eq!(pair[1].left(), pair[0].right() - 1.0, "{tool:?}: {pair:?}");
        }
        let span = options.iter().fold(Rect::NOTHING, |a, b| a.union(*b));
        let seps = h
            .drawn_boxes("button.sep")
            .into_iter()
            .filter(|r| span.intersects(*r))
            .count();
        assert_eq!(seps, options.len() - 1, "{tool:?}");
        // Ya no hay combo para el grupo.
        for label in &group.options {
            let combo = h
                .drawn_boxes("field.combo")
                .into_iter()
                .any(|c| c.contains(h.rect(label).center()));
            assert!(!combo, "{tool:?}: {label} in a combo");
        }
    }
}

#[test]
fn with_pixel_ui_one_click_on_a_radio_button_picks_that_option() {
    for group in radio_groups() {
        let mut h = pixel_bar(group.tool);
        let tool = group.tool;
        for label in group.options.iter().rev().chain(&group.options) {
            h.click(label);
            assert_eq!((group.current)(&h.editor), *label, "{tool:?}");
            h.frame(vec![]);
            // El punto verde pasa al elegido; los otros, con el anillo.
            for other in &group.options {
                assert_eq!(button_box(&h, other).1, other == label, "{tool:?}: {other}");
            }
        }
    }
}

#[test]
fn with_pixel_ui_the_brush_shape_button_shows_the_brush_at_its_size_up_to_nine() {
    let mut h = pixel_bar(ToolKind::Pencil);
    let ink = h.skin.as_ref().unwrap().colors().tab_text_dim;
    let icon = |h: &BarHarness, label: &str| {
        let button = h.rect(label);
        h.colored_rects()
            .into_iter()
            .filter(|(c, r)| *c == ink && button.contains_rect(*r))
            .count()
    };
    // Redondo de 1: un punto. Cuadrado de 3: 3×3. De 20: como el de 9 (el más grande).
    assert_eq!(icon(&h, "Round brush"), 1);
    h.click("Round brush");
    assert_eq!(h.editor.options.brush_shape, cs_tools::BrushShape::Square);
    h.editor.options.brush_size = 3;
    h.frame(vec![]);
    assert_eq!(icon(&h, "Square brush"), 9);
    h.editor.options.brush_size = 20;
    h.frame(vec![]);
    assert_eq!(icon(&h, "Square brush"), 81);
    h.click("Square brush");
    assert_eq!(h.editor.options.brush_shape, cs_tools::BrushShape::Round);
}

#[test]
fn with_pixel_ui_pixel_perfect_is_the_bar_checkbox_and_a_click_toggles_it() {
    let mut h = pixel_bar(ToolKind::Pencil);
    let bar = h.bar;
    // Sin marcar: la pieza no está dibujada y va el cuadro de respaldo.
    h.editor.options.pixel_perfect = false;
    h.frame(vec![]);
    assert_eq!(h.drawn("field.check.on"), vec![], "unchecked");
    let before = h.rect("Pixel Perfect");
    h.click("Pixel Perfect");
    h.frame(vec![]);
    assert!(h.editor.options.pixel_perfect);
    // Marcada: la casilla del diseño, 13×14 de la y 74 a la 87, 5 px después del control
    // anterior (en el diseño, del campo; acá, de los botones de forma), y la tinta del texto
    // 3 px después, en la fila de los textos.
    let check = h.drawn("field.check.on");
    let at = Rect::from_min_size(pos2(before.left(), bar.top() + 3.0), Vec2::new(13.0, 14.0));
    assert_eq!(check, vec![at]);
    assert_eq!(check[0].left() - h.rect("Round brush").right(), 5.0);
    let (_, ink) = h.text("Pixel Perfect");
    assert_eq!(ink.left() - check[0].right(), 3.0);
    assert_eq!(ink.top(), bar.top() + 6.0);
    h.click("Pixel Perfect");
    h.frame(vec![]);
    assert!(!h.editor.options.pixel_perfect);
    assert_eq!(h.drawn("field.check.on"), vec![]);
}

#[test]
fn with_pixel_ui_every_text_of_the_bar_is_in_the_design_row() {
    for tool in [
        ToolKind::Pencil,
        ToolKind::RectSelect,
        ToolKind::MagicWand,
        ToolKind::Text,
        ToolKind::Gradient,
        ToolKind::Rectangle,
    ] {
        let h = pixel_bar(tool);
        // "Load font…" y "Dafont…" son botones de texto (centrado en el botón): el diseño no
        // los tiene.
        let buttons = ["Load font", "Dafont"];
        for (text, ink, _) in h
            .texts()
            .into_iter()
            .filter(|(t, _, _)| !buttons.contains(&t.as_str()))
        {
            // Las mayúsculas en la fila 5 (los textos sin mayúsculas, como los números o
            // "px", no bajan de ahí).
            assert!(ink.top() >= h.bar.top() + 6.0, "{tool:?}: {text:?} {ink:?}");
            let caps = text.chars().any(|c| c.is_ascii_uppercase());
            if caps {
                assert_eq!(ink.top(), h.bar.top() + 6.0, "{tool:?}: {text:?} {ink:?}");
            }
        }
    }
}

#[test]
fn without_pixel_ui_the_bar_keeps_the_egui_controls() {
    let mut h = BarHarness::new(1200.0);
    assert!(h.has("Brush size") && h.has("Pixel-perfect"));
    assert!(!h.has("Brush Size") && !h.has("Pixel Perfect"));
    // El slider del pincel (su texto de valor) sigue ahí.
    h.editor.set_tool(ToolKind::RectSelect);
    h.frame(vec![]);
    // Los cuatro radios con su texto, y Rotation con los suyos.
    for mode in SelectionMode::ALL {
        assert!(
            h.texts().iter().any(|(t, _, _)| t == mode.label()),
            "{mode:?}"
        );
    }
    h.click(SelectionMode::Add.label());
    assert_eq!(h.editor.options.selection_mode, SelectionMode::Add);
    h.editor.set_tool(ToolKind::Gradient);
    h.frame(vec![]);
    assert!(h.has("Linear") && h.has("Radial"), "both radios");
    // Ninguna pieza del skin: no hay skin.
    assert!(h.skin.is_none());
}

#[test]
fn with_pixel_ui_the_corner_radius_button_turns_rounding_off_and_back_on() {
    let mut h = BarHarness::with_skin(1200.0, true);
    h.editor.set_tool(ToolKind::Rectangle);
    h.frame(vec![]);
    h.frame(vec![]);
    assert_eq!(h.rect("Corner radius").size(), Vec2::new(25.0, 16.0));
    h.click("Corner radius");
    h.frame(vec![]);
    assert_eq!(
        h.editor.options.corner_radius, DEFAULT_CORNER_RADIUS,
        "on, with the default"
    );
    h.editor.options.corner_radius = 7;
    h.frame(vec![]);
    h.click("Corner radius");
    h.frame(vec![]);
    assert_eq!(h.editor.options.corner_radius, 0, "off");
    h.click("Corner radius");
    h.frame(vec![]);
    assert_eq!(h.editor.options.corner_radius, 7, "back to the last one");
}

#[test]
fn with_pixel_ui_the_bar_ends_in_an_etched_line_and_symmetry_has_one_before_it() {
    let h = BarHarness::with_skin(1200.0, true);
    let colors = *h.skin.as_ref().unwrap().colors();
    let (dark, light) = (colors.canvas, colors.highlight);
    let rects = h.colored_rects();
    let bar = h.bar;
    assert_eq!(bar.height(), 23.0);
    // Abajo, a todo el ancho: la línea oscura en la fila 21 de la barra y el brillo en la 22
    // (19 filas de controles y 2 de aire).
    let line = bar.bottom() - 2.0;
    let x = bar.x_range();
    for (color, y) in [(dark, line), (light, line + 1.0)] {
        let want = Rect::from_x_y_ranges(x, y..=y + 1.0);
        assert!(rects.contains(&(color, want)), "{want:?} in {rects:?}");
    }
    // Antes de "Symmetry", con 5 px entre el brillo y el texto (en el mockup, x 852/853 y
    // "Symmetry" desde x 859). Va desde el brillo de la línea de las pestañas (una fila
    // arriba de la barra) hasta la línea de abajo, sin tocarla: y 70..87 en el mockup.
    let symmetry = h.rect("Symmetry");
    let left = symmetry.left() - 5.0 - 2.0;
    let rows = egui::Rangef::new(bar.top() - 1.0, line);
    for (color, x) in [(dark, left), (light, left + 1.0)] {
        let want = Rect::from_x_y_ranges(x..=x + 1.0, rows);
        assert!(rects.contains(&(color, want)), "{want:?} in {rects:?}");
    }
}

#[test]
fn with_pixel_ui_the_bar_has_one_etched_separator_and_no_egui_lines() {
    let mut h = BarHarness::with_skin(1200.0, true);
    let dark = h.skin.as_ref().unwrap().colors().canvas;
    for kind in [ToolKind::Pencil, ToolKind::MagicWand, ToolKind::Rectangle] {
        h.editor.set_tool(kind);
        h.frame(vec![]);
        h.frame(vec![]);
        assert_eq!(h.separator_lines(), 0, "{kind:?}");
        // Un solo vertical, el de Symmetry: el diseño (frame 4) no tiene líneas entre los
        // grupos de opciones, solo aire.
        let vertical = h
            .colored_rects()
            .into_iter()
            .filter(|(c, r)| *c == dark && r.width() == 1.0 && r.height() > 1.0)
            .count();
        assert_eq!(vertical, 1, "{kind:?}");
    }
    // Sin Pixel UI, los de siempre.
    let h = BarHarness::new(1200.0);
    assert!(h.separator_lines() > 0);
    let thin: Vec<_> = h
        .colored_rects()
        .into_iter()
        .filter(|(_, r)| r.width() == 1.0 || r.height() == 1.0)
        .collect();
    assert_eq!(thin, vec![], "no etched lines");
}

#[test]
fn the_options_scroll_sideways_with_the_wheel_when_they_do_not_fit() {
    let mut h = BarHarness::new(320.0);
    let before = h.rect("Round");
    let over = pos2(40.0, h.bar.center().y);
    h.frame(vec![Event::PointerMoved(over)]);
    h.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -60.0),
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    }]);
    for _ in 0..20 {
        h.frame(vec![]);
    }
    let after = h.rect("Round");
    assert!(after.left() < before.left(), "{before:?} -> {after:?}");
    assert_eq!(after.top(), before.top(), "still one row");
    assert!(
        h.rect("Vertical axis (left ↔ right)").right() <= 320.0,
        "Symmetry stays in view"
    );
}

#[test]
fn the_toolbar_has_only_the_tool_buttons() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
    let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    for skin in [None, Some(&skin)] {
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| toolbar(ui, &mut editor, skin));
        });
        output.textures_delta.clear();
        let texts: Vec<String> = nodes(&output).into_iter().map(|(t, _)| t).collect();
        for gone in [
            "Options",
            "Symmetry",
            "Brush size",
            "Vertical axis (left ↔ right)",
        ] {
            assert!(!texts.iter().any(|t| t == gone), "{gone} in {texts:?}");
        }
        assert!(texts.iter().any(|t| t.contains("Pencil")), "{texts:?}");
    }
}
