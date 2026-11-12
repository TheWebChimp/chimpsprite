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
        ("icon.tab_add", [5, 5]),
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

/// `shapes` rasterizadas a 1x en `rect` (fondo transparente): teseladas sin difuminado, cada
/// triángulo pinta los píxeles cuyo centro cae dentro, con la textura del skin (las piezas de
/// `skin`) o la de las fuentes, sin suavizado. Fila por fila, desde la esquina de `rect`.
/// Supone mallas de cuadros (dos triángulos seguidos por cuadro, como rects, piezas y letras).
fn raster(ctx: &egui::Context, skin: &Skin, shapes: Vec<Shape>, rect: Rect) -> Vec<Color32> {
    use egui::epaint::{ClippedShape, Primitive, TessellationOptions, Tessellator};
    let (font_size, font) = ctx.fonts(|f| (f.font_image_size(), f.image()));
    let options = TessellationOptions {
        feathering: false,
        ..Default::default()
    };
    let clipped = shapes
        .into_iter()
        .map(|shape| ClippedShape {
            clip_rect: Rect::EVERYTHING,
            shape,
        })
        .collect();
    let primitives =
        Tessellator::new(1.0, options, font_size, Vec::new()).tessellate_shapes(clipped);
    // La textura de una malla: tamaño y píxeles.
    let texture = |id: TextureId| -> ([usize; 2], Vec<Color32>) {
        if id == TextureId::default() {
            return (font.size, font.pixels.clone());
        }
        let piece = [DRAG, TAB, "button"]
            .iter()
            .flat_map(|n| {
                [WidgetState::Normal, WidgetState::Active, WidgetState::Hover]
                    .map(|s| skin.piece(n, s))
            })
            .flatten()
            .find(|p| p.texture.id() == id)
            .expect("a known piece");
        let [w, h] = piece.size;
        let pixels = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| piece.pixel(x, y))
            .collect();
        ([w as usize, h as usize], pixels)
    };
    let (w, h) = (rect.width() as usize, rect.height() as usize);
    let mut out = vec![Color32::TRANSPARENT; w * h];
    for primitive in primitives {
        let Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let (size, texels) = texture(mesh.texture_id);
        // Cada píxel, una vez por cuadro (en su diagonal caen sus dos triángulos): los rects y
        // las letras son de dos triángulos cada uno.
        let mut done = vec![usize::MAX; w * h];
        for (k, tri) in mesh.indices.chunks(3).enumerate() {
            let quad = k / 2;
            let [a, b, c] = [0, 1, 2].map(|i| mesh.vertices[tri[i] as usize]);
            let area =
                (b.pos - a.pos).x * (c.pos - a.pos).y - (b.pos - a.pos).y * (c.pos - a.pos).x;
            if area == 0.0 {
                continue;
            }
            for (i, slot) in out.iter_mut().enumerate() {
                if done[i] == quad {
                    continue;
                }
                let p = rect.min + vec2((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
                let edge =
                    |u: Pos2, v: Pos2| ((v - u).x * (p - u).y - (v - u).y * (p - u).x) / area;
                let (wa, wb, wc) = (edge(b.pos, c.pos), edge(c.pos, a.pos), edge(a.pos, b.pos));
                if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                    continue;
                }
                done[i] = quad;
                let uv = a.uv.to_vec2() * wa + b.uv.to_vec2() * wb + c.uv.to_vec2() * wc;
                let tx = ((uv.x * size[0] as f32) as usize).min(size[0] - 1);
                let ty = ((uv.y * size[1] as f32) as usize).min(size[1] - 1);
                let src = texels[ty * size[0] + tx] * a.color;
                let keep = 255 - src.a() as u32;
                let blend = |s: u8, d: u8| (s as u32 + d as u32 * keep / 255) as u8;
                *slot = Color32::from_rgba_premultiplied(
                    blend(src.r(), slot.r()),
                    blend(src.g(), slot.g()),
                    blend(src.b(), slot.b()),
                    blend(src.a(), slot.a()),
                );
            }
        }
    }
    out
}

/// La imagen del usuario (a 1x): la pastilla de "Color" que se arrastra, 34×18, en la esquina
/// (3, 3) de un PNG de 43×22. Cada fila, una letra por color: `.` transparente, `o` el contorno
/// (`canvas`), `h` el aro (`highlight`), `b` el relleno (`background`) y `#` el texto.
const COLOR_TAG: [&str; 18] = [
    "..oooooooooooooooooooooooooooooo..",
    ".ohhhhhhhhhhhhhhhhhhhhhhhhhhhhhho.",
    "ohbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbho",
    "ohbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbho",
    "ohbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbho",
    "ohbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbho",
    "ohbbbbb##bbbbbbb#bbbbbbbbbbbbbbbho",
    "ohbbbb#bb#bb##bb#bbb##bb#b##bbbbho",
    "ohbbbb#bbbb#bb#b#bb#bb#b##bbbbbbho",
    "ohbbbb#bbbb#bb#b#bb#bb#b#bbbbbbbho",
    "ohbbbb#bb#b#bb#b#bb#bb#b#bbbbbbbho",
    "ohbbbbb##bbb##bbb#bb##bb#bbbbbbbho",
    "ohbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbho",
    "ohbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbho",
    "ohbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbho",
    "ohbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbho",
    ".ohhhhhhhhhhhhhhhhhhhhhhhhhhhhhho.",
    "..oooooooooooooooooooooooooooooo..",
];

/// [`COLOR_TAG`] en colores: los de la imagen (el texto, `#050403`).
fn color_tag() -> Vec<Color32> {
    COLOR_TAG
        .iter()
        .flat_map(|row| row.chars())
        .map(|c| match c {
            '.' => Color32::TRANSPARENT,
            'o' => Color32::from_rgb(0xbd, 0xa4, 0x99),
            'h' => Color32::from_rgb(0xf1, 0xeb, 0xdb),
            'b' => Color32::from_rgb(0xdd, 0xce, 0xbf),
            _ => Color32::from_rgb(0x05, 0x04, 0x03),
        })
        .collect()
}

/// Las filas de `pixels` (de `w` de ancho) con las letras de [`COLOR_TAG`] (`?` si no es
/// ninguno de sus colores), para ver la diferencia.
fn rows(pixels: &[Color32], w: usize) -> Vec<String> {
    let reference: Vec<(Color32, char)> = color_tag()
        .into_iter()
        .zip(COLOR_TAG.concat().chars())
        .collect();
    pixels
        .chunks(w)
        .map(|row| {
            row.iter()
                .map(|p| {
                    reference
                        .iter()
                        .find(|(c, _)| c == p)
                        .map_or('?', |(_, l)| *l)
                })
                .collect()
        })
        .collect()
}

#[test]
fn the_drag_tag_for_color_is_34_by_18() {
    let (ctx, _) = pixel_ctx();
    let mut size = Vec2::ZERO;
    frame(&ctx, vec![], |ui| size = drag_tag_size(ui, "Color"));
    assert_eq!(size, vec2(34.0, 18.0));
}

#[test]
fn the_drag_tag_is_the_users_image_with_the_piece_or_without_it() {
    let (ctx, skin) = pixel_ctx();
    // Los colores de la imagen son los del skin.
    let colors = *skin.colors();
    assert_eq!(colors.text, Color32::from_rgb(0x05, 0x04, 0x03));
    assert_eq!(colors.canvas, Color32::from_rgb(0xbd, 0xa4, 0x99));
    assert_eq!(colors.highlight, Color32::from_rgb(0xf1, 0xeb, 0xdb));
    assert_eq!(colors.background, Color32::from_rgb(0xdd, 0xce, 0xbf));
    let piece = skin
        .piece(DRAG, WidgetState::Normal)
        .expect("the tab.drag piece");
    assert_eq!(piece.size, [5, 18]);
    let at = pos2(20.0, 40.0);
    let rect = Rect::from_min_size(at, vec2(34.0, 18.0));
    for skin in [skin.clone(), skin.clone().without(&[DRAG])] {
        let shapes = frame(&ctx, vec![], |ui| {
            ui.painter().extend(drag_tag_shapes(ui, &skin, at, "Color"));
        });
        let got = raster(&ctx, &skin, shapes, rect);
        assert_eq!(
            rows(&got, 34),
            COLOR_TAG.to_vec(),
            "with the piece: {}",
            skin.piece(DRAG, WidgetState::Normal).is_some()
        );
        assert_eq!(got, color_tag());
    }
}

#[test]
fn the_middle_button_does_not_close_a_tab_whose_close_is_not_close() {
    let (ctx, skin) = pixel_ctx();
    let items = vec![
        TabItem::closable("Cube", true).with_close(CloseVerb::Delete),
        TabItem::closable("Plane", true).with_close(CloseVerb::Delete),
    ];
    let id = egui::Id::new("strip");
    let strip = Strip::new(AT.x, AT.x + 400.0, AT.y);
    let at = pos2(AT.x + 20.0, AT.y + 6.0);
    let mut closed = false;
    for ev in [
        Event::PointerMoved(at),
        Event::PointerButton {
            pos: at,
            button: PointerButton::Middle,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: at,
            button: PointerButton::Middle,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ] {
        frame(&ctx, vec![ev], |ui| {
            let e = tab_strip(ui, &skin, id, strip, &items, Some(1));
            closed |= matches!(e, Some(TabEvent::Close(_)));
        });
    }
    assert!(!closed);
}
