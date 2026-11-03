use eframe::egui::{Event, Modifiers, PointerButton, Pos2, TextureId};

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

/// Lo dibujado: las imágenes (textura y rect) y los textos (posición, color y galería).
#[derive(Default)]
struct Drawn {
    images: Vec<(TextureId, Rect)>,
    texts: Vec<(Pos2, Color32, Arc<Galley>)>,
}

fn drawn(shapes: &[Shape]) -> Drawn {
    fn walk(shape: &Shape, out: &mut Drawn) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::default() => {
                out.images.push((mesh.texture_id, mesh.calc_bounds()));
            }
            Shape::Text(text) => {
                out.texts
                    .push((text.pos, text.fallback_color, text.galley.clone()));
            }
            _ => {}
        }
    }
    let mut out = Drawn::default();
    shapes.iter().for_each(|s| walk(s, &mut out));
    out
}

impl Drawn {
    /// La caja de cada dibujo de la pieza `name` (`state`): las partes de un 9-slice que se tocan,
    /// juntas.
    fn boxes(&self, skin: &Skin, name: &str, state: WidgetState) -> Vec<Rect> {
        let id = skin.piece(name, state).unwrap().texture.id();
        let mut out: Vec<Rect> = Vec::new();
        for (_, r) in self.images.iter().filter(|(t, _)| *t == id) {
            match out.iter_mut().find(|b| b.expand(0.5).intersects(*r)) {
                Some(b) => *b = b.union(*r),
                None => out.push(*r),
            }
        }
        out
    }
}

/// La columna donde empieza la tinta y la fila de arriba de las mayúsculas.
fn ink_at(at: Pos2, galley: &Galley) -> (f32, f32) {
    let row = &galley.rows[0];
    let glyph = &row.glyphs[0];
    (
        at.x + glyph.pos.x + glyph.uv_rect.offset.x,
        at.y + row.pos.y + glyph.pos.y - crate::tab_bar::CAP_HEIGHT,
    )
}

const AT: Pos2 = pos2(27.0, 30.0);

fn doc_items() -> Vec<TabItem> {
    vec![
        TabItem::closable("Design.chimp", true),
        TabItem::closable("Otro.chimp", true),
    ]
}

#[test]
fn document_tabs_measure_like_the_mockup_with_8_px_between_them() {
    let (ctx, _) = pixel_ctx();
    let mut lay = None;
    let mut inks = vec![];
    frame(&ctx, vec![], |ui| {
        let strip = Strip::new(AT.x, AT.x + 400.0, AT.y);
        lay = Some(layout(ui, &strip, &doc_items(), 0));
        inks = doc_items().iter().map(|t| ink(ui, &t.label)).collect();
    });
    let lay = lay.unwrap();
    // En el mockup: "Design.chimp" de la x 27 a la 99 (73 = 2 + 4 + 52 de tinta + 4 + 5 de la
    // marca + 4 + 2) y "Otro.chimp" de la 108 a la 172: 8 columnas entre las dos (las marcas
    // azules). Nuestra fuente deja 1 px más de tinta en cada nombre (el punto).
    assert_eq!(inks, vec![53.0, 45.0]);
    let rects: Vec<Rect> = lay.placed.iter().map(|p| p.rect).collect();
    let first = 2.0 + 4.0 + inks[0] + 4.0 + 5.0 + 4.0 + 2.0;
    let second = 2.0 + 4.0 + inks[1] + 4.0 + 5.0 + 4.0 + 2.0;
    assert_eq!(
        rects,
        vec![
            Rect::from_min_size(AT, vec2(first, HEIGHT)),
            Rect::from_min_size(pos2(AT.x + first + 8.0, AT.y), vec2(second, HEIGHT)),
        ]
    );
    assert_eq!(GAP, 8.0);
    assert!(lay.arrows.is_none());
}

#[test]
fn a_tab_has_its_text_6_px_in_on_row_6_and_its_mark_6_px_before_the_edge() {
    let (ctx, skin) = pixel_ctx();
    let colors = *skin.colors();
    let rect = Rect::from_min_size(AT, vec2(73.0, HEIGHT));
    let items = doc_items();
    for (active, hovered, piece, state, height, color) in [
        (
            true,
            false,
            TAB,
            WidgetState::Active,
            ACTIVE_HEIGHT,
            colors.text,
        ),
        (
            false,
            false,
            TAB,
            WidgetState::Normal,
            HEIGHT,
            colors.tab_text_dim,
        ),
        (
            false,
            true,
            TAB,
            WidgetState::Hover,
            HEIGHT,
            colors.tab_text_dim,
        ),
    ] {
        let shapes = frame(&ctx, vec![], |ui| {
            let look = TabLook {
                active,
                hovered,
                mark_hovered: false,
            };
            let shapes = tab_shapes(ui, &skin, rect, &items[0], "Design.chimp", look);
            ui.painter().extend(shapes);
        });
        let d = drawn(&shapes);
        assert_eq!(
            d.boxes(&skin, piece, state),
            vec![rect.with_max_y(rect.top() + height)],
            "active {active}, hovered {hovered}"
        );
        let (at, c, galley) = d.texts.last().unwrap();
        assert_eq!(*c, color);
        assert_eq!(ink_at(*at, galley), (AT.x + 6.0, AT.y + 6.0));
        // La ○ de los cambios sin guardar: 5×5 en la fila 7, 6 px antes del borde derecho.
        let mark = d.boxes(&skin, DIRTY_MARK, WidgetState::Normal);
        assert_eq!(
            mark,
            vec![Rect::from_min_size(
                pos2(rect.right() - 11.0, AT.y + 7.0),
                Vec2::splat(5.0)
            )]
        );
    }
    // Las piezas: la de pasar el puntero no es la normal.
    assert_ne!(
        skin.piece(TAB, WidgetState::Hover).unwrap().texture.id(),
        skin.piece(TAB, WidgetState::Normal).unwrap().texture.id()
    );
}

#[test]
fn the_etched_line_is_cut_under_the_active_tab_with_the_shade_on_both_sides() {
    let (ctx, skin) = pixel_ctx();
    let strip = Strip::new(AT.x, AT.x + 200.0, AT.y);
    let active = Rect::from_min_size(pos2(AT.x + 20.0, AT.y), vec2(73.0, ACTIVE_HEIGHT));
    let shapes = frame(&ctx, vec![], |ui| {
        ui.painter()
            .extend(line_shapes(ui, &skin, &strip, Some(active)));
    });
    let d = drawn(&shapes);
    let row = AT.y + HEIGHT;
    assert_eq!(strip.bottom(), row);
    // El tramado de la fila oscura: 5 px contra cada lado de la activa.
    assert_eq!(
        d.boxes(&skin, SHADE_LEFT, WidgetState::Normal),
        vec![Rect::from_min_size(
            pos2(active.left() - 5.0, row),
            vec2(5.0, 1.0)
        )]
    );
    assert_eq!(
        d.boxes(&skin, SHADE_RIGHT, WidgetState::Normal),
        vec![Rect::from_min_size(
            pos2(active.right(), row),
            vec2(5.0, 1.0)
        )]
    );
    // La línea (rects de colores), cortada debajo de la activa.
    let mut lines = Vec::new();
    fn rects(shape: &Shape, out: &mut Vec<Rect>) {
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| rects(s, out)),
            Shape::Mesh(m) if m.texture_id == TextureId::default() => {
                // Cada rect de color de la malla: 4 vértices.
                for quad in m.vertices.chunks(4) {
                    let r = quad.iter().fold(Rect::NOTHING, |r, v| {
                        r.union(Rect::from_min_max(v.pos, v.pos))
                    });
                    out.push(r);
                }
            }
            _ => {}
        }
    }
    shapes.iter().for_each(|s| rects(s, &mut lines));
    assert!(!lines.is_empty());
    for r in &lines {
        assert!(
            r.right() <= active.left() || r.left() >= active.right(),
            "{r:?} under the active tab"
        );
    }
}

#[test]
fn tabs_that_do_not_fit_scroll_with_the_arrows_and_the_arrows_change_with_hover_and_ends() {
    let (ctx, skin) = pixel_ctx();
    let items: Vec<TabItem> = (1..=8)
        .map(|i| TabItem::new(format!("Artboard {i}")))
        .collect();
    let id = egui::Id::new("strip");
    let strip = Strip::new(0.0, 200.0, 10.0);
    let run = |events: Vec<Event>| {
        let mut lay = None;
        let shapes = frame(&ctx, events, |ui| {
            lay = Some(layout(ui, &strip, &items, 0));
            tab_strip(ui, &skin, id, strip, &items, Some(0));
        });
        (shapes, lay.unwrap())
    };
    let (shapes, lay) = run(vec![]);
    let arrows = lay.arrows.expect("arrows");
    // Las flechas: 23×12 contra el borde derecho, 1 fila debajo de las pestañas.
    assert_eq!(
        arrows,
        Rect::from_min_size(pos2(200.0 - 23.0, 11.0), vec2(23.0, 12.0))
    );
    let d = drawn(&shapes);
    // Al principio, la de atrás deshabilitada.
    assert_eq!(d.boxes(&skin, ARROW_PREV, WidgetState::Disabled).len(), 1);
    assert_eq!(d.boxes(&skin, ARROW_NEXT, WidgetState::Normal).len(), 1);
    // Un clic en la de adelante corre la tira una pestaña.
    let next = Rect::from_min_size(
        arrows.min + vec2(ARROW_SIZE - 1.0, 0.0),
        Vec2::splat(ARROW_SIZE),
    )
    .center();
    let press = |pressed| Event::PointerButton {
        pos: next,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    run(vec![Event::PointerMoved(next), press(true)]);
    run(vec![press(false)]);
    let (shapes, _) = run(vec![]);
    let d = drawn(&shapes);
    assert_eq!(d.boxes(&skin, ARROW_PREV, WidgetState::Normal).len(), 1);
    assert_eq!(d.boxes(&skin, ARROW_NEXT, WidgetState::Hover).len(), 1);
    let first = d
        .texts
        .iter()
        .map(|t| t.2.text().to_string())
        .next()
        .unwrap();
    assert_ne!(first, "Artboard 1", "scrolled by one");
}

#[test]
fn clicking_selects_and_the_mark_or_the_middle_button_closes() {
    let (ctx, skin) = pixel_ctx();
    let items = doc_items();
    let id = egui::Id::new("strip");
    let strip = Strip::new(AT.x, AT.x + 400.0, AT.y);
    let run = |events: Vec<Event>| {
        let mut event = None;
        frame(&ctx, events, |ui| {
            event = tab_strip(ui, &skin, id, strip, &items, Some(0));
        });
        event
    };
    let second = Rect::from_min_size(pos2(AT.x + 81.0, AT.y), vec2(65.0, HEIGHT));
    let click = |pos: Pos2, button| {
        vec![
            vec![Event::PointerMoved(pos)],
            vec![Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
            vec![Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        ]
    };
    let mut last = None;
    for events in click(
        second.left_center() + vec2(10.0, 0.0),
        PointerButton::Primary,
    ) {
        last = run(events);
    }
    assert_eq!(last, Some(TabEvent::Select(1)));
    for events in click(mark_rect(second).center(), PointerButton::Primary) {
        last = run(events);
    }
    assert_eq!(last, Some(TabEvent::Close(1)));
    for events in click(
        second.left_center() + vec2(10.0, 0.0),
        PointerButton::Middle,
    ) {
        last = run(events);
    }
    assert_eq!(last, Some(TabEvent::Close(1)));
}

#[test]
fn the_tab_pieces_have_the_mockup_sizes() {
    let (_, skin) = pixel_ctx();
    for (name, size) in [
        ("tab.arrow.prev", [12, 12]),
        ("tab.arrow.next", [12, 12]),
        ("tab.shade.left", [5, 1]),
        ("tab.shade.right", [5, 1]),
        ("tab.hover", [7, 15]),
    ] {
        let piece = skin.piece(name, WidgetState::Normal).unwrap();
        assert_eq!(piece.size, size, "{name}");
    }
}

#[test]
fn a_tab_with_a_piece_icon_draws_it_bottom_aligned_before_the_name() {
    let (ctx, skin) = pixel_ctx();
    let rect = Rect::from_min_size(AT, vec2(60.0, HEIGHT));
    for (name, size) in [
        ("icon.object.cone", vec2(9.0, 10.0)),
        ("icon.object.plane", vec2(8.0, 8.0)),
    ] {
        let item = TabItem::closable("Cone", true).with_icon(TabIcon::Piece(name));
        let shapes = frame(&ctx, vec![], |ui| {
            let look = TabLook {
                active: true,
                hovered: false,
                mark_hovered: false,
            };
            ui.painter()
                .extend(tab_shapes(ui, &skin, rect, &item, "Cone", look));
        });
        let d = drawn(&shapes);
        // Como en el boceto: el ícono abajo en la fila 13 (los de 10 de alto, desde la 4), 6 px
        // adentro; el nombre, 3 después del lugar del ícono (`ICON_WIDTH`), en la fila 6.
        let left = AT.x + EDGE + PAD;
        assert_eq!(
            d.boxes(&skin, name, WidgetState::Normal),
            vec![Rect::from_min_size(
                pos2(left, AT.y + ICON_BOTTOM - size.y),
                size
            )],
            "{name}"
        );
        let (at, _, galley) = d.texts.first().expect("the name").clone();
        assert_eq!(
            ink_at(at, &galley),
            (left + ICON_WIDTH + SWATCH_GAP, AT.y + CAPS_TOP)
        );
        assert_eq!(
            chrome(&item),
            chrome(&TabItem::closable("", true)) + ICON_WIDTH + SWATCH_GAP
        );
    }
}
