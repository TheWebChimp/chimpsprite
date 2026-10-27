use cs_commands::Command;
use cs_core::{ColorMode, LayerId, PixelValue, Rgba, Sprite};
use eframe::emath::GuiRounding as _;
use egui::{Event, Key, PointerButton, RawInput};

use super::*;

#[test]
fn checker_cells_are_16_sprite_pixels_at_any_zoom() {
    // Un artboard de 64×64 lleva 4×4 cuadros (la textura repite 2×2 veces su par
    // claro/oscuro) al 100 % y al 800 %: al hacer zoom los cuadros crecen con el dibujo.
    assert_eq!(checker_uv(64, 64).max, Pos2::new(2.0, 2.0));
    assert_eq!(checker_uv(40, 8).max, Pos2::new(1.25, 0.25));
    assert_eq!(checker_uv(40, 8).min, Pos2::ZERO);
}

/// Lienzo de 800×600 manejado con eventos crudos, con el reloj avanzando 50 ms por frame.
struct Harness {
    ctx: egui::Context,
    canvas: CanvasView,
    editor: Editor,
    view: ViewSettings,
    time: f64,
    rect: Rect,
    /// Cursor pedido en el último frame.
    cursor: egui::CursorIcon,
    /// Lo dibujado en el último frame, aplanado.
    shapes: Vec<egui::Shape>,
    /// La textura de la nota sin el puntero encima, `note.plain` (con `with_note`).
    note_texture: Option<egui::TextureId>,
}

impl Harness {
    fn new() -> Self {
        let mut h = Self {
            ctx: egui::Context::default(),
            canvas: CanvasView::default(),
            editor: Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba)),
            view: ViewSettings::default(),
            time: 0.0,
            rect: Rect::NOTHING,
            cursor: egui::CursorIcon::Default,
            shapes: Vec::new(),
            note_texture: None,
        };
        h.frame(vec![]);
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let mut rect = Rect::NOTHING;
        let mut output = self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::no_frame().show(ui, |ui| {
                rect = ui.max_rect();
                self.canvas.show(ui, &mut self.editor, None, &self.view);
            });
        });
        output.textures_delta.clear();
        self.rect = rect;
        self.cursor = output.platform_output.cursor_icon;
        self.shapes.clear();
        for clipped in output.shapes {
            flatten(clipped.shape, &mut self.shapes);
        }
    }

    /// Rects de las mallas con la textura `id` en el último frame.
    fn meshes(&self, id: egui::TextureId) -> Vec<Rect> {
        self.shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Mesh(m) if m.texture_id == id => Some(m.calc_bounds()),
                _ => None,
            })
            .collect()
    }

    /// Textos del último frame con su rect.
    fn texts(&self) -> Vec<(String, Rect)> {
        self.shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_owned(),
                    Rect::from_min_size(t.pos, t.galley.size()),
                )),
                _ => None,
            })
            .collect()
    }

    /// Área de píxeles del artboard `i` en pantalla.
    fn pixels_of(&self, i: usize) -> Rect {
        let ((x0, y0), (x1, y1)) = self.editor.document.artboards[i].rect();
        Rect::from_min_max(
            self.canvas.to_screen(self.rect, (x0, y0)),
            self.canvas.to_screen(self.rect, (x1, y1)),
        )
    }

    fn click(&mut self, pos: Pos2, button: PointerButton) {
        let ev = |pressed| Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![ev(true)]);
        self.frame(vec![ev(false)]);
    }

    fn key(&mut self, key: Key) {
        let ev = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        self.frame(vec![ev(true), ev(false)]);
    }

    /// Un evento de rueda con el puntero sobre el píxel (8, 8).
    /// `modifiers` quedan mantenidos (`ModifiersChanged`) hasta el próximo cambio.
    fn wheel(&mut self, unit: egui::MouseWheelUnit, dy: f32, modifiers: egui::Modifiers) {
        let pos = self.screen((8, 8));
        self.frame(vec![
            Event::ModifiersChanged(modifiers),
            Event::PointerMoved(pos),
        ]);
        self.frame(vec![Event::MouseWheel {
            unit,
            delta: Vec2::new(0.0, dy),
            phase: egui::TouchPhase::Move,
            modifiers,
        }]);
    }

    /// Centro en pantalla del píxel local `p` del artboard activo.
    fn screen(&self, p: (i32, i32)) -> Pos2 {
        let (x, y) = self.editor.artboard().position;
        self.canvas.to_screen(self.rect, (x + p.0, y + p.1)) + Vec2::splat(self.canvas.zoom / 2.0)
    }

    /// Esquina en pantalla del píxel local `p` del artboard activo (no su centro).
    fn corner(&self, p: (i32, i32)) -> Pos2 {
        let (x, y) = self.editor.artboard().position;
        self.canvas.to_screen(self.rect, (x + p.0, y + p.1))
    }

    /// Arrastre con el botón principal de `from` a `to` (en pantalla), en varios frames.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.drag_with(from, to, PointerButton::Primary);
    }

    /// Arrastre con `button` de `from` a `to` (en pantalla), en varios frames.
    fn drag_with(&mut self, from: Pos2, to: Pos2, button: PointerButton) {
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        self.frame(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
        self.frame(vec![Event::PointerMoved(to)]);
        self.frame(vec![button(to, false)]);
    }

    /// Un punto rojo en `at` y la selección `a`–`b` con el marco rectangular.
    fn selected_dot(&mut self, at: (i32, i32), a: (i32, i32), b: (i32, i32)) -> PixelValue {
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        self.editor.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(at.0, at.1, red)],
        });
        self.editor.set_tool(ToolKind::RectSelect);
        self.editor
            .set_selection(Some(Mask::from_rect(16, 16, a, b)));
        self.frame(vec![]);
        red
    }
}

#[test]
fn the_other_button_cancels_a_stroke_without_starting_another() {
    let mut h = Harness::new();
    let t = h.editor.sprite().color_mode.transparent();
    let event = |pos, button, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    };
    let (a, b) = (h.screen((2, 2)), h.screen((6, 2)));
    h.frame(vec![Event::PointerMoved(a)]);
    h.frame(vec![event(a, PointerButton::Primary, true)]);
    h.frame(vec![Event::PointerMoved(b)]);
    assert!(h.editor.is_painting());
    // El derecho, con el izquierdo todavía abajo: se cancela.
    h.frame(vec![event(b, PointerButton::Secondary, true)]);
    assert!(!h.editor.is_painting(), "cancelled, and no new stroke");
    h.frame(vec![event(b, PointerButton::Secondary, false)]);
    h.frame(vec![event(b, PointerButton::Primary, false)]);
    h.frame(vec![]);
    for x in 2..=6 {
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, x, 2), t, "({x}, 2)");
    }
}

#[test]
fn while_painting_only_the_active_artboard_is_composed() {
    let mut h = Harness::new();
    let first = h.editor.active;
    h.editor.add_artboard(8, 8);
    h.editor.activate(first);
    h.frame(vec![]);
    let event = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let (a, b) = (h.screen((2, 2)), h.screen((6, 2)));
    h.frame(vec![Event::PointerMoved(a)]);
    h.frame(vec![event(a, true)]);
    assert!(h.editor.is_painting());
    let before = h.canvas.composed;
    h.frame(vec![Event::PointerMoved(b)]);
    h.frame(vec![]);
    assert_eq!(
        h.canvas.composed - before,
        2,
        "one per frame: the active one"
    );
    // Al soltar cambia el documento: se componen todos.
    let before = h.canvas.composed;
    h.frame(vec![event(b, false)]);
    h.frame(vec![]);
    assert!(h.canvas.composed - before >= 2, "both again");
}

#[test]
fn dragging_a_corner_handle_scales_and_enter_applies() {
    let mut h = Harness::new();
    let red = h.selected_dot((2, 2), (2, 2), (3, 3));
    let t = h.editor.sprite().color_mode.transparent();
    // Los handles quedan por fuera de la caja: se toma la esquina un poco hacia afuera.
    let out = Vec2::splat(3.0);
    h.drag(h.corner((4, 4)) + out, h.corner((6, 6)) + out);
    assert_eq!(
        h.editor.transform_box(),
        Some(TransformBox::new(2, 2, 4, 4))
    );
    assert_eq!(
        h.editor.sprite().pixel(LayerId(1), 0, 3, 3),
        t,
        "still floating"
    );
    crate::editor_key(&mut h.editor, Key::Enter, false);
    assert!(!h.editor.is_floating());
    for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
        assert_eq!(
            h.editor.sprite().pixel(LayerId(1), 0, x, y),
            red,
            "({x}, {y})"
        );
    }
    assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 4, 4), t);
}

#[test]
fn dragging_just_outside_a_corner_rotates() {
    let mut h = Harness::new();
    let red = h.selected_dot((2, 2), (2, 2), (5, 5));
    let t = h.editor.sprite().color_mode.transparent();
    let center = h.corner((4, 4));
    // 16 px por fuera de la esquina NE: más allá del handle (10 px), dentro de la franja.
    let start = h.corner((6, 2)) + Vec2::new(16.0, -16.0);
    let v = start - center;
    h.drag(start, center + Vec2::new(-v.y, v.x)); // un cuarto de vuelta horario
    let angle = h.editor.transform_box().unwrap().angle;
    assert!(
        (angle - std::f32::consts::FRAC_PI_2).abs() < 1e-3,
        "{angle}"
    );
    crate::editor_key(&mut h.editor, Key::Enter, false);
    assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 2), red);
    assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 2, 2), t);
}

#[test]
fn dragging_inside_the_selection_still_moves_it() {
    let mut h = Harness::new();
    let red = h.selected_dot((2, 2), (2, 2), (3, 3));
    h.drag(h.screen((2, 2)), h.screen((5, 2)));
    assert_eq!(
        h.editor.transform_box(),
        Some(TransformBox::new(5, 2, 2, 2))
    );
    crate::editor_key(&mut h.editor, Key::Enter, false);
    assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 2), red);
}

/// Zoom 4 (los handles de 10 px cubren más de un píxel del sprite).
fn zoomed_out() -> Harness {
    let mut h = Harness::new();
    h.canvas.zoom = 4.0;
    h.canvas.pan = -Vec2::splat(8.0) * 4.0;
    h.frame(vec![]);
    h
}

#[test]
fn a_small_selection_moves_from_inside_at_low_zoom() {
    let mut h = zoomed_out();
    let red = h.selected_dot((2, 2), (2, 2), (3, 3));
    h.drag(h.screen((2, 2)), h.screen((5, 2)));
    assert_eq!(
        h.editor.transform_box(),
        Some(TransformBox::new(5, 2, 2, 2))
    );
    crate::editor_key(&mut h.editor, Key::Enter, false);
    assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 2), red);
}

#[test]
fn pressing_just_inside_an_edge_moves_instead_of_scaling() {
    let mut h = zoomed_out();
    h.selected_dot((2, 2), (2, 2), (5, 5));
    // 2 px de pantalla adentro del lado E, a media altura.
    let from = h.corner((6, 4)) - Vec2::new(2.0, 0.0);
    h.drag(from, from + Vec2::new(12.0, 0.0));
    assert_eq!(
        h.editor.transform_box(),
        Some(TransformBox::new(5, 2, 4, 4))
    );
}

#[test]
fn no_handles_during_another_selection_gesture() {
    let mut h = Harness::new();
    h.selected_dot((2, 2), (2, 2), (5, 5));
    let press = |pos| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    };
    // Un marco nuevo desde fuera de la selección pasa por encima del handle SE.
    let from = h.screen((12, 12));
    let se = h.corner((6, 6)) + Vec2::new(3.0, 3.0);
    h.frame(vec![Event::PointerMoved(from)]);
    h.frame(vec![press(from)]);
    h.frame(vec![Event::PointerMoved(from.lerp(se, 0.5))]);
    h.frame(vec![Event::PointerMoved(se)]);
    h.frame(vec![]);
    assert!(h.editor.is_painting() && !h.editor.is_transforming());
    assert_eq!(h.cursor, egui::CursorIcon::Crosshair);
}

#[test]
fn with_pixel_ui_the_tools_draw_the_design_cursors() {
    let mut h = Harness::new();
    h.canvas.pixel_ui = true;
    // Sin los cursores del skin, los del sistema.
    h.frame(vec![Event::PointerMoved(h.screen((3, 3)))]);
    h.frame(vec![]);
    assert_eq!(h.cursor, egui::CursorIcon::Crosshair, "no skin cursors");
    let data = crate::skin::decode(crate::skin::EMBEDDED).expect("the embedded skin");
    h.canvas.cursors = Some(pixel_cursors::CursorSet::from_pieces(|name| {
        data.pieces.get(name).map(|p| (p.size, p.rgba.clone()))
    }));
    // Los píxeles de los cursores: de un punto, con una de las dos tintas.
    let ink = |h: &Harness| -> Vec<Rect> {
        h.shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r)
                    if r.rect.size() == Vec2::splat(1.0)
                        && [pixel_cursors::INK, Color32::WHITE].contains(&r.fill) =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect()
    };
    let p = h.screen((3, 3)) + Vec2::splat(0.5);
    for tool in [ToolKind::Pencil, ToolKind::RectSelect, ToolKind::Eyedropper] {
        h.editor.set_tool(tool);
        h.frame(vec![Event::PointerMoved(p)]);
        h.frame(vec![]);
        assert_eq!(h.cursor, egui::CursorIcon::None, "{tool:?}");
        assert!(!ink(&h).is_empty(), "{tool:?}");
    }
    // Las esquinas de selección, por fuera del píxel (3, 3).
    h.editor.set_tool(ToolKind::RectSelect);
    h.frame(vec![]);
    let cell = Rect::from_min_max(h.corner((3, 3)), h.corner((4, 4)));
    let bounds = ink(&h).iter().fold(Rect::NOTHING, |b, r| b.union(*r));
    assert_eq!(bounds, cell.expand(3.0));
    // Cada píxel se invierte según lo de debajo: sobre un píxel negro, blanco; sobre uno
    // blanco, la tinta.
    let fill_at = |h: &Harness, q: Pos2| {
        h.shapes.iter().rev().find_map(|s| match s {
            egui::Shape::Rect(r) if r.rect.size() == Vec2::splat(1.0) && r.rect.contains(q) => {
                Some(r.fill)
            }
            _ => None,
        })
    };
    for (color, expected) in [
        (Rgba::rgb(0, 0, 0), Color32::WHITE),
        (Rgba::rgb(255, 255, 255), pixel_cursors::INK),
    ] {
        h.editor.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(2, 2, PixelValue::Rgba(color))],
        });
        h.frame(vec![]);
        // El vértice de arriba a la izquierda, sobre el píxel (2, 2) del sprite.
        let q = h.corner((3, 3)) - Vec2::splat(0.5);
        assert_eq!(fill_at(&h, q), Some(expected), "{color:?}");
    }
    // Sin Pixel UI, el del sistema.
    h.canvas.pixel_ui = false;
    h.editor.set_tool(ToolKind::Pencil);
    h.frame(vec![]);
    assert_eq!(h.cursor, egui::CursorIcon::Crosshair);
}

#[test]
fn right_click_inside_the_selection_opens_its_menu_to_flip() {
    let mut h = Harness::new();
    let red = h.selected_dot((2, 3), (2, 2), (5, 5));
    h.click(h.screen((3, 3)), PointerButton::Secondary);
    h.frame(vec![]);
    let item = h
        .texts()
        .into_iter()
        .find(|(t, _)| t == "Flip Horizontal")
        .map(|(_, r)| r)
        .unwrap_or_else(|| panic!("no menu in {:?}", h.texts()));
    assert!(h.texts().iter().any(|(t, _)| t == "Flip Vertical"));
    assert!(
        !h.editor.is_floating(),
        "the right click does not reach the tool"
    );
    h.click(item.center(), PointerButton::Primary);
    h.frame(vec![]);
    assert!(h.editor.is_floating());
    crate::editor_key(&mut h.editor, Key::Enter, false);
    assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 3), red);
}

#[test]
fn the_symmetry_axis_moves_with_the_artboard_while_dragging_it() {
    let mut h = Harness::new();
    let blue = Color32::from_rgb(0, 73, 255);
    h.canvas.symmetry_color = Some(blue);
    h.editor.set_symmetry(cs_core::Symmetry {
        axes: cs_core::SymmetryAxes {
            vertical: true,
            ..Default::default()
        },
        center2: None,
    });
    h.editor.set_tool(ToolKind::Artboard);
    h.frame(vec![]);
    let axis_x = |h: &Harness| {
        h.shapes
            .iter()
            .find_map(|s| match s {
                egui::Shape::Rect(r)
                    if r.fill == blue && r.rect.width() == 1.0 && r.rect.height() > 1.0 =>
                {
                    Some(r.rect.left())
                }
                _ => None,
            })
            .expect("the vertical axis")
    };
    let before = axis_x(&h);
    // Se agarra el artboard y se mueve 3 píxeles, sin soltar.
    let from = h.screen((4, 4));
    let to = from + Vec2::new(3.0 * h.canvas.zoom, 0.0);
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame(vec![Event::PointerMoved(from)]);
    h.frame(vec![press(from, true)]);
    h.frame(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
    h.frame(vec![Event::PointerMoved(to)]);
    assert_eq!(axis_x(&h), before + 3.0 * h.canvas.zoom);
}

#[test]
fn with_pixel_ui_the_symmetry_axes_are_one_point_lines_with_handles() {
    let mut h = Harness::new();
    let blue = Color32::from_rgb(0, 73, 255);
    h.canvas.symmetry_color = Some(blue);
    h.editor.set_symmetry(cs_core::Symmetry {
        axes: cs_core::SymmetryAxes {
            vertical: true,
            diagonal_down: true,
            ..Default::default()
        },
        center2: None,
    });
    h.frame(vec![]);
    let fills = |color: Color32| -> Vec<Rect> {
        h.shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r) if r.fill == color => Some(r.rect),
                _ => None,
            })
            .collect()
    };
    let pixels = h.pixels_of(0);
    let lines = fills(blue);
    // El eje vertical: 1 punto de ancho, de handle a handle (5 adentro de cada borde).
    let vertical = lines
        .iter()
        .find(|r| r.width() == 1.0 && r.height() > 1.0)
        .expect("the vertical axis");
    assert_eq!(vertical.top(), pixels.top().round() + 5.0);
    assert_eq!(vertical.bottom(), pixels.bottom().round() - 5.0);
    // La diagonal: una escalera de puntos.
    assert!(
        lines
            .iter()
            .filter(|r| r.size() == Vec2::splat(1.0))
            .count()
            > 10
    );
    // Los handles de 5×5 (tinta), arriba y abajo del eje.
    let handles: Vec<Rect> = fills(crate::pixel_cursors::INK)
        .into_iter()
        .filter(|r| r.size() == Vec2::splat(5.0))
        .collect();
    assert_eq!(handles.len(), 2, "{handles:?}");
    assert!(
        handles
            .iter()
            .all(|r| r.center().x.floor() == vertical.left())
    );
}

#[test]
fn with_zoom_the_symmetry_handles_stick_to_the_edges_of_the_view() {
    let mut h = Harness::new();
    h.canvas.symmetry_color = Some(Color32::from_rgb(0, 73, 255));
    h.editor.set_symmetry(cs_core::Symmetry {
        axes: cs_core::SymmetryAxes {
            vertical: true,
            ..Default::default()
        },
        center2: None,
    });
    h.frame(vec![]);
    // Tan cerca que el artboard pasa los bordes de la vista.
    h.canvas.zoom = 64.0;
    h.canvas.request_zoom(ZoomRequest::CenterActive);
    h.frame(vec![]);
    h.frame(vec![]);
    let pixels = h.pixels_of(0);
    assert!(pixels.top() < h.rect.top() && pixels.bottom() > h.rect.bottom());
    let view = h.canvas.visible(h.rect);
    let handles: Vec<Rect> = h
        .shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r)
                if r.fill == crate::pixel_cursors::INK && r.rect.size() == Vec2::splat(5.0) =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .collect();
    assert_eq!(handles.len(), 2, "{handles:?}");
    let top = handles.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
    let bottom = handles.iter().map(|r| r.bottom()).fold(f32::MIN, f32::max);
    assert_eq!(top, view.top().round(), "under the ruler");
    assert_eq!(bottom, view.bottom().round(), "at the bottom of the view");
}

#[test]
fn the_symmetry_control_moves_with_any_tool() {
    let mut h = Harness::new();
    h.canvas.symmetry_color = Some(Color32::from_rgb(0, 73, 255));
    h.editor.set_tool(ToolKind::Pencil);
    h.editor.set_symmetry(cs_core::Symmetry {
        axes: cs_core::SymmetryAxes {
            vertical: true,
            ..Default::default()
        },
        center2: None,
    });
    h.frame(vec![]);
    let before = h.editor.symmetry().center2;
    let pixels = h.pixels_of(0);
    // El handle de arriba del eje vertical (en el centro del artboard).
    let from = Pos2::new(pixels.center().x, pixels.top() + 2.0);
    let to = from + Vec2::new(h.canvas.zoom * 2.0, 0.0);
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame(vec![Event::PointerMoved(from)]);
    h.frame(vec![press(from, true)]);
    h.frame(vec![Event::PointerMoved(to)]);
    h.frame(vec![press(to, false)]);
    h.frame(vec![]);
    assert_ne!(h.editor.symmetry().center2, before, "the axis moved");
    let sprite = h.editor.sprite();
    let t = sprite.color_mode.transparent();
    let layer = h.editor.layer;
    let drawn = (0..sprite.width as i32)
        .flat_map(|x| (0..sprite.height as i32).map(move |y| (x, y)))
        .any(|(x, y)| sprite.pixel(layer, 0, x, y) != t);
    assert!(!drawn, "the pencil did not draw");
}

#[test]
fn hovering_the_handles_shows_resize_and_rotate_cursors() {
    let mut h = Harness::new();
    h.selected_dot((2, 2), (2, 2), (5, 5));
    let hover = |h: &mut Harness, p: Pos2| {
        h.frame(vec![Event::PointerMoved(p)]);
        h.frame(vec![]);
        h.cursor
    };
    // Los handles quedan por fuera de la caja.
    let (se, ne, n, w) = (
        h.corner((6, 6)) + Vec2::new(3.0, 3.0),
        h.corner((6, 2)) + Vec2::new(3.0, -3.0),
        h.corner((4, 2)) + Vec2::new(0.0, -3.0),
        h.corner((2, 4)) + Vec2::new(-3.0, 0.0),
    );
    let inside = h.screen((3, 3));
    let outside = h.corner((6, 2)) + Vec2::new(16.0, -16.0);
    assert_eq!(hover(&mut h, se), egui::CursorIcon::ResizeNwSe);
    assert_eq!(hover(&mut h, ne), egui::CursorIcon::ResizeNeSw);
    assert_eq!(hover(&mut h, n), egui::CursorIcon::ResizeVertical);
    assert_eq!(hover(&mut h, w), egui::CursorIcon::ResizeHorizontal);
    assert_eq!(hover(&mut h, inside), egui::CursorIcon::Move);
    assert_eq!(
        hover(&mut h, outside),
        egui::CursorIcon::None,
        "rotate: drawn by hand"
    );
    // Con una herramienta de dibujo no hay handles.
    h.editor.set_tool(ToolKind::Pencil);
    assert_eq!(hover(&mut h, se), egui::CursorIcon::Crosshair);
}

#[test]
fn dragging_the_selection_keeps_the_move_cursor_off_the_artboard() {
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::RectSelect);
    h.editor
        .set_selection(Some(cs_core::Mask::from_rect(16, 16, (2, 2), (6, 6))));
    let ev = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let from = h.screen((4, 4));
    // Fuera del artboard pero dentro del lienzo.
    let off = Pos2::new(h.rect.right() - 2.0, from.y);
    let world = h.canvas.to_world(h.rect, off);
    assert!(h.editor.document.artboard_at(world).is_none(), "{world:?}");
    h.frame(vec![Event::PointerMoved(from)]);
    assert_eq!(h.cursor, egui::CursorIcon::Move);
    h.frame(vec![ev(from, true)]);
    h.frame(vec![Event::PointerMoved(from + Vec2::new(5.0, 0.0))]);
    h.frame(vec![Event::PointerMoved(off)]);
    h.frame(vec![]);
    assert!(h.editor.is_transforming());
    assert_eq!(h.cursor, egui::CursorIcon::Move, "still moving");
    h.frame(vec![ev(off, false)]);
    h.frame(vec![]);
    assert!(!h.editor.is_transforming());
}

#[test]
fn double_clicking_an_artboard_label_renames_it() {
    let mut h = Harness::new();
    let label = h.canvas.to_screen(h.rect, h.editor.artboard().position)
        - Vec2::new(-10.0, LABEL_GAP + LABEL_SIZE / 2.0);
    h.click(label, PointerButton::Primary);
    h.click(label, PointerButton::Primary);
    h.frame(vec![]);
    assert!(h.canvas.renaming.is_some(), "double click opens the field");
    let mut erase = vec![];
    for _ in 0..20 {
        for pressed in [true, false] {
            erase.push(Event::Key {
                key: Key::Backspace,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            });
        }
    }
    h.frame(erase);
    h.frame(vec![Event::Text("Hero".into())]);
    h.key(Key::Enter);
    h.frame(vec![]);
    assert_eq!(h.editor.artboard().name, "Hero");
    assert!(h.canvas.renaming.is_none());
    assert_eq!(
        h.editor.artboard().position,
        (0, 0),
        "the clicks did not move it"
    );
}

#[test]
fn right_clicking_a_slice_selects_it_and_opens_its_menu() {
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::Slice);
    let drag = |e: &mut Editor, a: (f32, f32), b: (f32, f32)| {
        e.pointer_press(a, false, false, 0.1);
        e.pointer_drag(b);
        e.pointer_release();
    };
    drag(&mut h.editor, (2.5, 2.5), (6.5, 6.5));
    drag(&mut h.editor, (10.5, 10.5), (12.5, 12.5));
    assert_eq!(h.editor.slice, Some(1));
    let before = h.editor.sprite().slices.clone();
    h.frame(vec![]);
    h.click(h.screen((4, 4)), PointerButton::Secondary);
    h.frame(vec![]);
    assert_eq!(h.editor.slice, Some(0));
    assert!(egui::Popup::is_any_open(&h.ctx), "context menu");
    assert_eq!(h.editor.sprite().slices, before, "nothing created or moved");
    // Fuera de un slice no hay menú.
    h.key(Key::Escape);
    h.click(h.screen((14, 1)), PointerButton::Secondary);
    h.frame(vec![]);
    assert!(!egui::Popup::is_any_open(&h.ctx));
}

#[test]
fn autoscroll_only_near_edges_and_grows_with_distance() {
    let area = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 300.0));
    assert_eq!(
        autoscroll_velocity(area, Pos2::new(200.0, 150.0)),
        Vec2::ZERO
    );
    let near = autoscroll_velocity(area, Pos2::new(390.0, 150.0));
    let far = autoscroll_velocity(area, Pos2::new(460.0, 150.0));
    assert!(near.x > 0.0 && far.x > near.x && near.y == 0.0);
    let up_left = autoscroll_velocity(area, Pos2::new(-500.0, -500.0));
    assert_eq!(
        up_left,
        Vec2::new(-AUTOSCROLL_SPEED, -AUTOSCROLL_SPEED),
        "saturates"
    );
}
#[test]
fn a_right_drag_does_not_lift_or_move_the_selection() {
    for tool in [ToolKind::Move, ToolKind::RectSelect] {
        let mut h = Harness::new();
        let red = h.selected_dot((2, 2), (2, 2), (3, 3));
        let t = h.editor.sprite().color_mode.transparent();
        h.editor.set_tool(tool);
        h.frame(vec![]);
        let version = h.editor.doc_version;
        h.drag_with(h.screen((2, 2)), h.screen((6, 6)), PointerButton::Secondary);
        assert!(!h.editor.is_floating(), "{tool:?}");
        assert_eq!(h.editor.doc_version, version, "{tool:?}");
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 2, 2), red);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 6, 6), t);
    }
}

#[test]
fn the_hand_tool_pans_the_view_without_touching_the_document() {
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::Hand);
    let from = h.screen((8, 8));
    h.frame(vec![Event::PointerMoved(from)]);
    assert_eq!(h.cursor, egui::CursorIcon::Grab, "at rest");
    let (pan, version) = (h.canvas.pan, h.editor.doc_version);
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let to = from + Vec2::new(40.0, 0.0);
    h.frame(vec![button(from, true)]);
    h.frame(vec![Event::PointerMoved(from + Vec2::new(20.0, 0.0))]);
    assert_eq!(h.cursor, egui::CursorIcon::Grabbing, "while dragging");
    h.frame(vec![Event::PointerMoved(to)]);
    h.frame(vec![button(to, false)]);
    let moved = h.canvas.pan - pan;
    assert!(moved.x > 15.0 && moved.y == 0.0, "{moved:?}");
    assert_eq!(h.editor.doc_version, version, "nothing painted");
    assert!(h.editor.sprite().cels.is_empty());
    h.frame(vec![]);
    assert_eq!(h.cursor, egui::CursorIcon::Grab);
    assert_eq!(h.editor.tool, ToolKind::Hand);
}

#[test]
fn any_tool_moves_a_guide_unless_they_are_locked() {
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::Pencil);
    // Una guía horizontal en la fila 8, desde la regla.
    let ruler = Pos2::new(h.rect.center().x, h.rect.top() + RULER_PX / 2.0);
    h.drag(ruler, h.screen((8, 8)));
    assert_eq!(h.editor.guides()[0].position, 8);
    let before = h.editor.sprite().clone();
    // Con el lápiz se agarra y se mueve (no pinta).
    let on_guide = h.canvas.to_screen(h.rect, (4, 8));
    let target = h.canvas.to_screen(h.rect, (4, 12));
    h.drag(on_guide, target);
    assert_eq!(h.editor.guides()[0].position, 12, "moved with the pencil");
    assert_eq!(*h.editor.sprite(), before, "the pencil did not draw");
    // Bloqueadas, no se mueven.
    h.view.lock_guides = true;
    let on_guide = h.canvas.to_screen(h.rect, (4, 12));
    h.drag(on_guide, h.screen((4, 14)));
    assert_eq!(h.editor.guides()[0].position, 12, "locked");
}

#[test]
fn with_the_hand_tool_a_ruler_drag_still_creates_a_guide() {
    // El mismo arrastre con Pencil: la vista solo se mueve por el auto-scroll del borde.
    let ruler_drag = |tool| {
        let mut h = Harness::new();
        h.editor.set_tool(tool);
        let ruler = Pos2::new(h.rect.center().x, h.rect.top() + RULER_PX / 2.0);
        h.frame(vec![Event::PointerMoved(ruler)]);
        assert_eq!(h.cursor, egui::CursorIcon::ResizeVertical, "over the ruler");
        h.drag(ruler, h.screen((8, 8)));
        h.frame(vec![]);
        assert_eq!(h.editor.guides().len(), 1, "a guide was created");
        assert_eq!(h.editor.guides()[0].axis, Axis::Horizontal);
        (h.editor.guides()[0].position, h.canvas.pan)
    };
    assert_eq!(
        ruler_drag(ToolKind::Hand),
        ruler_drag(ToolKind::Pencil),
        "the hand did not pan"
    );
}

#[test]
fn with_pixel_rulers_the_ruler_is_12_thick_for_guides_and_the_pointer() {
    let mut h = Harness::new();
    let skin = Skin::embedded(&h.ctx);
    h.canvas.pixel_rulers = Some(crate::pixel_rulers::PixelRulers::from_skin(&skin));
    h.frame(vec![]);
    // La fila 11 (la línea oscura) todavía es regla; la 12 ya es lienzo.
    let on = Pos2::new(h.rect.center().x, h.rect.top() + 11.5);
    h.frame(vec![Event::PointerMoved(on)]);
    assert_eq!(h.cursor, egui::CursorIcon::ResizeVertical, "over the ruler");
    let off = Pos2::new(h.rect.center().x, h.rect.top() + 12.5);
    h.frame(vec![Event::PointerMoved(off)]);
    assert_ne!(h.cursor, egui::CursorIcon::ResizeVertical, "on the canvas");
    h.drag(on, h.screen((8, 8)));
    h.frame(vec![]);
    assert_eq!(h.editor.guides().len(), 1, "a guide was created");
    assert_eq!(h.editor.guides()[0].axis, Axis::Horizontal);
    // Las coordenadas no cambian: el mismo píxel bajo el mismo punto.
    let p = h.screen((3, 5));
    h.frame(vec![Event::PointerMoved(p)]);
    assert_eq!(h.canvas.hover, Some((3, 5)));
}

#[test]
fn with_the_hand_tool_an_artboard_label_is_not_renamed_or_moved() {
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::Hand);
    let label = h.canvas.to_screen(h.rect, h.editor.artboard().position)
        - Vec2::new(-10.0, LABEL_GAP + LABEL_SIZE / 2.0);
    h.click(label, PointerButton::Primary);
    h.click(label, PointerButton::Primary);
    h.frame(vec![]);
    assert!(h.canvas.renaming.is_none(), "no rename field");
    assert_eq!(h.cursor, egui::CursorIcon::Grab);
    assert_eq!(h.editor.artboard().position, (0, 0));
}

#[test]
fn ctrl_wheel_changes_the_brush_size_of_brush_tools() {
    use egui::{Modifiers, MouseWheelUnit};
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::Pencil);
    let zoom = h.canvas.zoom;
    h.wheel(MouseWheelUnit::Line, 1.0, Modifiers::COMMAND);
    assert_eq!(h.editor.options.brush_size, 2, "one step up");
    assert_eq!(h.canvas.zoom, zoom, "no zoom");
    h.wheel(MouseWheelUnit::Line, -1.0, Modifiers::COMMAND);
    h.wheel(MouseWheelUnit::Line, -1.0, Modifiers::COMMAND);
    assert_eq!(h.editor.options.brush_size, 1, "not below 1");
    h.editor.options.brush_size = 32;
    h.wheel(MouseWheelUnit::Line, 1.0, Modifiers::COMMAND);
    assert_eq!(h.editor.options.brush_size, 32, "not above 32");
    // Touchpad: 50 px por paso.
    h.editor.options.brush_size = 1;
    h.wheel(MouseWheelUnit::Point, 30.0, Modifiers::COMMAND);
    assert_eq!(h.editor.options.brush_size, 1, "30 px are not a step yet");
    h.wheel(MouseWheelUnit::Point, 30.0, Modifiers::COMMAND);
    assert_eq!(h.editor.options.brush_size, 2);
    assert_eq!(h.canvas.zoom, zoom, "still no zoom");
}

#[test]
fn ctrl_wheel_does_nothing_during_a_stroke() {
    use egui::{Modifiers, MouseWheelUnit};
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::Pencil);
    let zoom = h.canvas.zoom;
    let pos = h.screen((8, 8));
    h.frame(vec![Event::PointerMoved(pos)]);
    h.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    assert!(h.editor.is_painting());
    h.wheel(MouseWheelUnit::Line, 1.0, Modifiers::COMMAND);
    assert_eq!(h.editor.options.brush_size, 1, "same size mid-stroke");
    assert_eq!(h.canvas.zoom, zoom, "and no zoom");
}

#[test]
fn ctrl_wheel_zooms_without_a_brush_and_the_plain_wheel_always_zooms() {
    use egui::{Modifiers, MouseWheelUnit};
    for tool in [ToolKind::Hand, ToolKind::RectSelect] {
        let mut h = Harness::new();
        h.editor.set_tool(tool);
        let zoom = h.canvas.zoom;
        h.wheel(MouseWheelUnit::Line, -1.0, Modifiers::COMMAND);
        assert!(h.canvas.zoom < zoom, "{tool:?} zooms out");
        assert_eq!(h.editor.options.brush_size, 1);
    }
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::Pencil);
    let zoom = h.canvas.zoom;
    h.wheel(MouseWheelUnit::Line, -1.0, Modifiers::NONE);
    assert!(h.canvas.zoom < zoom, "without Ctrl it zooms");
    assert_eq!(h.editor.options.brush_size, 1);
}

#[test]
fn pixel_wheel_steps_add_up_to_50_px() {
    use egui::MouseWheelUnit::{Line, Point};
    let mut rest = 0.0;
    assert_eq!(brush_wheel_steps(&mut rest, Point, 20.0), 0);
    assert_eq!(brush_wheel_steps(&mut rest, Point, 20.0), 0);
    assert_eq!(brush_wheel_steps(&mut rest, Point, 20.0), 1); // 60: quedan 10
    assert_eq!(brush_wheel_steps(&mut rest, Point, 120.0), 2); // 130: quedan 30
    assert_eq!(brush_wheel_steps(&mut rest, Point, -80.0), -1); // -50: quedan 0
    assert_eq!(
        brush_wheel_steps(&mut rest, Line, 3.0),
        1,
        "one per line event"
    );
    assert_eq!(brush_wheel_steps(&mut rest, Line, -0.5), -1);
    assert_eq!(brush_wheel_steps(&mut rest, Line, 0.0), 0);
}

#[test]
fn with_pixel_ui_the_canvas_surround_is_the_skin_canvas_color() {
    let ctx = egui::Context::default();
    let skin = Skin::embedded(&ctx);
    assert_eq!(background_for(None), theme::CANVAS_BG);
    assert_eq!(
        background_for(Some(&skin)),
        Color32::from_rgb(189, 164, 153)
    );
    // El lienzo pinta su fondo con ese color.
    let mut h = Harness::new();
    h.canvas.background = background_for(Some(&skin));
    let mut shapes = Vec::new();
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
        ..Default::default()
    };
    let output = h.ctx.run_ui(input, |ui| {
        egui::CentralPanel::no_frame().show(ui, |ui| {
            h.canvas.show(ui, &mut h.editor, None, &h.view);
        });
    });
    for clipped in output.shapes {
        shapes.push(clipped.shape);
    }
    let fills_with_canvas = shapes
        .iter()
        .any(|s| matches!(s, egui::Shape::Rect(r) if r.fill == Color32::from_rgb(189, 164, 153)));
    assert!(fills_with_canvas);
}

#[test]
fn a_click_with_both_straight_axes_paints_four_pixels() {
    let mut h = Harness::new();
    h.editor.set_symmetry(cs_core::Symmetry {
        axes: cs_core::SymmetryAxes {
            vertical: true,
            horizontal: true,
            ..cs_core::SymmetryAxes::NONE
        },
        center2: None,
    });
    assert_eq!(h.editor.tool, ToolKind::Pencil);
    h.click(h.screen((1, 2)), PointerButton::Primary);
    let primary = h.editor.primary;
    let sprite = h.editor.sprite();
    let painted: Vec<(i32, i32)> = (0..16)
        .flat_map(|y| (0..16).map(move |x| (x, y)))
        .filter(|&(x, y)| sprite.pixel(LayerId(1), 0, x, y) == primary)
        .collect();
    assert_eq!(painted, vec![(1, 2), (14, 2), (1, 13), (14, 13)]);
}

#[test]
fn the_round_brush_outline_follows_its_pixels() {
    use cs_tools::BrushShape;
    // Redondo de 3: una cruz de 5 píxeles, con 12 lados de borde.
    let cross = cs_raster::brush_points((0, 0), 3, BrushShape::Round);
    let edges = brush_edges(&cross);
    assert_eq!(edges.len(), 12);
    assert!(edges.contains(&((0, -1), (1, -1))), "top of the cross");
    assert!(!edges.contains(&((-1, -1), (0, -1))), "no square corner");
    assert_eq!(brush_rows(&cross), vec![(-1, 0, 0), (0, -1, 1), (1, 0, 0)]);
    let square = cs_raster::brush_points((0, 0), 2, BrushShape::Square);
    assert_eq!(brush_edges(&square).len(), 8);
    // Sentido horario: el pincel queda hacia (-dy, dx) de cada lado (ahí se dibuja el trazo).
    for (a, b) in edges {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let inside = (a.0.min(b.0) + (-dy).min(0), a.1.min(b.1) + dx.min(0));
        assert!(cross.contains(&inside), "{a:?}->{b:?}");
    }
}

#[test]
fn round_outline_segments_close_the_inner_corners() {
    use cs_tools::BrushShape;
    // Zoom 10 y trazo de 1: medio píxel de pantalla hacia adentro.
    let cross = cs_raster::brush_points((0, 0), 3, BrushShape::Round);
    let at = |(x, y): (i32, i32)| Pos2::new(x as f32 * 10.0, y as f32 * 10.0);
    let segments = outline_segments(&cross, at, 1.0);
    assert_eq!(segments.len(), 12);
    // Arriba del brazo izquierdo, hacia la derecha: termina en una esquina interior y se
    // alarga un ancho de trazo para tapar el hueco con el lado del brazo de arriba.
    assert!(
        segments.contains(&[Pos2::new(-10.0, 0.5), Pos2::new(1.0, 0.5)]),
        "{segments:?}"
    );
    // Arriba del brazo de arriba: dos esquinas exteriores, sin alargar (no sobresale).
    assert!(segments.contains(&[Pos2::new(0.0, -9.5), Pos2::new(10.0, -9.5)]));
    // Cuadrado de 2: solo esquinas exteriores.
    let square = cs_raster::brush_points((0, 0), 2, BrushShape::Square);
    for [a, b] in outline_segments(&square, at, 1.0) {
        for p in [a, b] {
            assert!(
                (0.0..=20.0).contains(&p.x) && (0.0..=20.0).contains(&p.y),
                "{p:?} sticks out"
            );
        }
    }
}

#[test]
fn leftover_wheel_pixels_do_not_carry_into_the_next_gesture() {
    use egui::{Modifiers, MouseWheelUnit::Point};
    let mut h = Harness::new();
    h.editor.set_tool(ToolKind::Pencil);
    let ctrl = Modifiers::COMMAND;
    // Soltar Ctrl.
    h.wheel(Point, 30.0, ctrl);
    h.frame(vec![Event::ModifiersChanged(Modifiers::NONE)]);
    h.wheel(Point, 30.0, ctrl);
    assert_eq!(h.editor.options.brush_size, 1, "Ctrl released in between");
    // Una rueda sin Ctrl.
    h.wheel(Point, 1.0, Modifiers::NONE);
    h.wheel(Point, 30.0, ctrl);
    assert_eq!(h.editor.options.brush_size, 1, "plain wheel in between");
    // Otra herramienta.
    h.editor.set_tool(ToolKind::Hand);
    h.wheel(Point, 30.0, ctrl);
    h.editor.set_tool(ToolKind::Pencil);
    h.wheel(Point, 30.0, ctrl);
    assert_eq!(h.editor.options.brush_size, 1, "tool changed in between");
    // Sin nada en el medio sí se suman.
    h.wheel(Point, 30.0, ctrl);
    assert_eq!(h.editor.options.brush_size, 2);
}

#[test]
fn a_guide_is_cut_where_an_artboard_in_front_covers_it() {
    let r = |a: f32, b: f32| egui::Rangef::new(a, b);
    assert_eq!(uncovered(r(0.0, 100.0), &[]), vec![r(0.0, 100.0)]);
    assert_eq!(
        uncovered(r(0.0, 100.0), &[r(30.0, 60.0)]),
        vec![r(0.0, 30.0), r(60.0, 100.0)]
    );
    assert_eq!(
        uncovered(r(0.0, 100.0), &[r(-10.0, 50.0)]),
        vec![r(50.0, 100.0)]
    );
    assert!(uncovered(r(0.0, 100.0), &[r(-10.0, 200.0)]).is_empty());
}

#[test]
fn the_symmetry_is_cut_where_an_artboard_in_front_covers_it() {
    let front = Rect::from_min_max(Pos2::new(40.0, 0.0), Pos2::new(60.0, 100.0));
    // Un eje horizontal (de 1 px de alto) que cruza el de delante: dos pedazos.
    let axis = Rect::from_min_max(Pos2::new(0.0, 50.0), Pos2::new(100.0, 51.0));
    assert_eq!(
        rect_uncovered(axis, &[front]),
        vec![
            Rect::from_min_max(Pos2::new(0.0, 50.0), Pos2::new(40.0, 51.0)),
            Rect::from_min_max(Pos2::new(60.0, 50.0), Pos2::new(100.0, 51.0)),
        ]
    );
    assert!(
        rect_uncovered(
            Rect::from_min_size(Pos2::new(45.0, 10.0), Vec2::splat(5.0)),
            &[front]
        )
        .is_empty()
    );
    // Una diagonal: sin el tramo que pasa por debajo.
    let pieces = segment_uncovered([Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0)], &[front])
        .into_iter()
        .map(|[a, b]| [a.round(), b.round()])
        .collect::<Vec<_>>();
    assert_eq!(
        pieces,
        vec![
            [Pos2::new(0.0, 0.0), Pos2::new(40.0, 40.0)],
            [Pos2::new(60.0, 60.0), Pos2::new(100.0, 100.0)],
        ]
    );
}

#[test]
fn entering_or_leaving_the_mask_shows_at_once() {
    use crate::mask_ops::NewMask;
    let mut h = Harness::new();
    h.editor.add_mask(LayerId(1), NewMask::HideAll);
    h.editor.set_edit_mask(false);
    h.frame(vec![]);
    let revision = h.editor.revision;
    assert_eq!(h.canvas.rendered_mask, None);
    // Sin otro cambio en el documento, el rojo de la máscara aparece en el mismo frame.
    h.editor.set_edit_mask(true);
    h.frame(vec![]);
    assert_eq!(h.editor.revision, revision, "the document did not change");
    assert_eq!(
        h.canvas.rendered_mask,
        Some(LayerId(1)),
        "recomposed with the mask"
    );
    h.editor.set_edit_mask(false);
    h.frame(vec![]);
    assert_eq!(h.canvas.rendered_mask, None, "and without it");
}

#[test]
fn entering_the_mask_mid_blur_stroke_leaves_no_stroke_hanging() {
    use crate::mask_ops::NewMask;
    let mut h = Harness::new();
    h.editor.add_mask(LayerId(1), NewMask::RevealAll);
    h.editor.set_edit_mask(false);
    h.editor.set_tool(ToolKind::Blur);
    let (from, to) = (h.screen((4, 4)), h.screen((8, 4)));
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame(vec![Event::PointerMoved(from)]);
    h.frame(vec![button(from, true)]);
    assert!(h.editor.is_painting());
    h.editor.set_edit_mask(true);
    h.frame(vec![Event::PointerMoved(to)]);
    h.frame(vec![button(to, false)]);
    h.frame(vec![]);
    assert!(!h.editor.is_painting(), "no stroke left hanging");
}

/// Aplana los `Shape::Vec`.
fn flatten(shape: egui::Shape, out: &mut Vec<egui::Shape>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.into_iter().for_each(|s| flatten(s, out)),
        other => out.push(other),
    }
}

/// Con Pixel UI: la ventana del skin alrededor de los artboards.
fn with_window(h: &mut Harness) -> Skin {
    let skin = Skin::embedded(&h.ctx);
    h.canvas.artboard_window = Some(ArtboardWindow::from_skin(&skin));
    h.frame(vec![]);
    skin
}

fn piece_id(skin: &Skin, name: &str) -> egui::TextureId {
    skin.piece(name, crate::skin::WidgetState::Normal)
        .unwrap_or_else(|| panic!("{name}"))
        .texture
        .id()
}

fn union(rects: &[Rect]) -> Rect {
    rects.iter().fold(Rect::NOTHING, |a, r| a.union(*r))
}

#[test]
fn with_pixel_ui_the_window_wraps_the_pixels_and_its_bar_does_not_zoom() {
    let mut h = Harness::new();
    let skin = with_window(&mut h);
    let mut bars = Vec::new();
    for zoom in [1.0, 8.0, 16.0] {
        h.canvas.zoom = zoom;
        h.frame(vec![]);
        let pixels = h.pixels_of(0);
        let frame = union(&h.meshes(piece_id(&skin, "artboard.frame")));
        // Borde + 1 de blanco a los lados y abajo; barra de 13 + 1 de blanco arriba.
        assert_eq!(
            frame,
            Rect::from_min_max(
                pixels.min - Vec2::new(2.0, 14.0),
                pixels.max + Vec2::new(2.0, 2.0)
            ),
            "zoom {zoom}"
        );
        bars.push(pixels.top() - frame.top());
        // Los íconos van en la barra, encima del área de píxeles.
        for name in ["artboard.menu", "window.close"] {
            let icon = union(&h.meshes(piece_id(&skin, name)));
            assert!(
                icon.bottom() <= pixels.top() && icon.top() >= frame.top(),
                "{name} {icon:?}"
            );
        }
        // El nombre va en la barra, en lugar de la etiqueta de arriba (a zoom 1 el
        // artboard de 16 no tiene lugar para el nombre y a 8 se corta con "...").
        if zoom < 16.0 {
            continue;
        }
        let label = label_text(h.editor.artboard());
        let (_, text) = h
            .texts()
            .into_iter()
            .find(|(t, _)| *t == label)
            .unwrap_or_else(|| panic!("the name in the bar: {:?}", h.texts()));
        assert!(text.center().y > frame.top() && text.center().y < pixels.top());
        // Centrado en la franja de 13 que empieza debajo de la fila rosa agregada arriba.
        let band_center = frame.top() + 1.0 + 13.0 / 2.0;
        assert_eq!(text.top(), (band_center - text.height() / 2.0).round());
    }
    assert_eq!(
        bars,
        vec![14.0; 3],
        "the bar (13) and the white (1) do not zoom"
    );
}

#[test]
fn with_pixel_ui_the_pixels_and_the_checker_do_not_move() {
    let mut h = Harness::new();
    let id = h.editor.artboard().id;
    let drawn = |h: &Harness| {
        let tex = h.canvas.textures[&id].id();
        let checker = h.canvas.checker.as_ref().unwrap().id();
        (h.meshes(tex), h.meshes(checker))
    };
    let off = drawn(&h);
    assert_eq!(off.0, vec![h.pixels_of(0)]);
    with_window(&mut h);
    assert_eq!(drawn(&h), off);
}

#[test]
fn dragging_the_title_bar_moves_the_artboard() {
    let mut h = Harness::new();
    h.canvas.zoom = 8.0;
    with_window(&mut h);
    let from = h.pixels_of(0).left_top() + Vec2::new(60.0, -6.0);
    h.frame(vec![Event::PointerMoved(from)]);
    assert_eq!(h.cursor, egui::CursorIcon::Grab);
    h.drag(from, from + Vec2::new(16.0, 8.0));
    assert_eq!(h.editor.artboard().position, (2, 1));
    assert!(h.editor.sprite().cels.is_empty(), "nothing painted");
}

#[test]
fn double_clicking_the_title_bar_renames_the_artboard() {
    let mut h = Harness::new();
    with_window(&mut h);
    let bar = h.pixels_of(0).left_top() + Vec2::new(40.0, -6.0);
    h.click(bar, PointerButton::Primary);
    h.click(bar, PointerButton::Primary);
    h.frame(vec![]);
    assert!(h.canvas.renaming.is_some(), "double click opens the field");
    assert_eq!(h.editor.artboard().position, (0, 0));
}

#[test]
fn the_menu_icon_opens_the_artboard_menu_and_the_circle_deletes_it() {
    let mut h = Harness::new();
    h.editor.add_artboard(8, 8);
    let first = h.editor.document.artboards[0].id;
    assert_ne!(h.editor.active, first);
    with_window(&mut h);
    let pixels = h.pixels_of(0);
    // "≡": lo selecciona y abre su menú, sin moverlo.
    let menu = pixels.left_top() + Vec2::new(-2.0 + 2.0 + 4.5, -14.0 + 7.5);
    h.click(menu, PointerButton::Primary);
    assert_eq!(h.editor.active, first);
    assert_eq!(
        h.editor.artboard().position,
        (0, 0),
        "a click does not move it"
    );
    assert!(h.canvas.renaming.is_none());
    h.frame(vec![]);
    let texts: Vec<String> = h
        .shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    for item in ["Rename", "Duplicate", "Canvas Size", "Delete"] {
        assert!(texts.iter().any(|t| t == item), "{item} in {texts:?}");
    }
    h.click(menu, PointerButton::Primary);
    // "○", a 2 px del borde derecho de la ventana: borra el artboard (se deshace).
    let circle = Pos2::new(pixels.right() + 2.0 - 2.0 - 4.5, pixels.top() - 14.0 + 7.5);
    h.click(circle, PointerButton::Primary);
    assert!(h.editor.document.artboard(first).is_none(), "deleted");
    h.editor.undo();
    assert!(h.editor.document.artboard(first).is_some(), "undone");
}

/// Una nota en el mundo (0, 0) con Pixel UI y su papel en pantalla.
fn with_note(h: &mut Harness) -> (u32, Rect) {
    let skin = with_window(h);
    // Sin el puntero encima, la nota desde la línea de la barra (la pieza de la barra,
    // recortada).
    h.note_texture = Some(piece_id(&skin, "note"));
    crate::pixel_ui::set_bubble(&h.ctx, Some(std::sync::Arc::new(skin)));
    h.editor.set_tool(ToolKind::Pencil);
    let id = h.editor.add_note((0, 0)).unwrap();
    h.frame(vec![]);
    let at = h.canvas.to_screen(h.rect, (0, 0)).round();
    let size = crate::notes::NEW_SIZE;
    (
        id,
        Rect::from_min_size(at, Vec2::new(size.0 as f32, size.1 as f32)),
    )
}

#[test]
fn a_click_on_a_note_writes_in_it() {
    let mut h = Harness::new();
    let (id, paper) = with_note(&mut h);
    let at = paper.center();
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame(vec![Event::PointerMoved(at)]);
    h.frame(vec![press(true)]);
    h.frame(vec![press(false)]);
    h.frame(vec![]);
    h.frame(vec![Event::Text("hola".into())]);
    h.frame(vec![]);
    h.canvas.notes.finish_editing(&mut h.editor);
    let note = h.editor.document.notes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(note.text, "hola");
}

#[test]
fn typing_in_a_new_note_writes_its_text() {
    let mut h = Harness::new();
    let (id, _) = with_note(&mut h);
    h.canvas.notes.start_editing(&mut h.editor, id);
    h.frame(vec![]);
    h.frame(vec![]);
    h.frame(vec![Event::Text("hola".into())]);
    h.frame(vec![]);
    h.canvas.notes.finish_editing(&mut h.editor);
    let note = h.editor.document.notes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(note.text, "hola");
}

#[test]
fn a_note_is_drawn_over_the_artboard_and_takes_the_clicks() {
    let mut h = Harness::new();
    let (_, paper) = with_note(&mut h);
    let texture = h.note_texture.unwrap();
    let drawn = h
        .shapes
        .iter()
        .any(|s| matches!(s, egui::Shape::Mesh(m) if m.texture_id == texture));
    assert!(drawn, "the note piece");
    let before = h.editor.sprite().clone();
    // Un clic en el cuerpo de la nota, sobre el artboard: no pinta.
    h.click(paper.center(), PointerButton::Primary);
    assert_eq!(*h.editor.sprite(), before, "the pencil did not draw");
}

#[test]
fn far_away_a_note_is_its_pin_and_opens_with_the_pointer_over_it() {
    let mut h = Harness::new();
    with_note(&mut h);
    let texture = h.note_texture.unwrap();
    let drew_note = |h: &Harness| {
        h.shapes
            .iter()
            .any(|s| matches!(s, egui::Shape::Mesh(m) if m.texture_id == texture))
    };
    assert!(drew_note(&h), "near: the note");
    // Lejos (el artboard de 16 a 2 puntos por píxel: 32), la nota taparía todo: su pin.
    h.canvas.zoom = 2.0;
    h.frame(vec![egui::Event::PointerMoved(Pos2::new(790.0, 590.0))]);
    assert!(!drew_note(&h), "far: only its pin");
    let pin = h.canvas.to_screen(h.rect, (0, 0)).round() + Vec2::new(5.0, 11.0 + 5.0);
    h.frame(vec![egui::Event::PointerMoved(pin)]);
    h.frame(vec![]);
    assert!(drew_note(&h), "the pointer over the pin opens it");
}

#[test]
fn the_bar_of_a_note_shows_only_with_the_pointer_over_it() {
    let mut h = Harness::new();
    let skin = with_window(&mut h);
    let bar = piece_id(&skin, "note");
    crate::pixel_ui::set_bubble(&h.ctx, Some(std::sync::Arc::new(skin)));
    h.editor.add_note((0, 0)).unwrap();
    h.frame(vec![]);
    let paper = h.canvas.to_screen(h.rect, (0, 0)).round();
    let width = crate::notes::NEW_SIZE.0 as f32;
    // Sin la barra, su línea es el borde de arriba del papel (en tinta).
    let top_line = |h: &Harness| {
        h.shapes.iter().any(|s| {
            matches!(s, egui::Shape::Rect(r)
                if r.fill == Color32::from_rgb(5, 4, 3)
                    && r.rect == Rect::from_min_size(paper + Vec2::new(0.0, 11.0), Vec2::new(width, 1.0)))
        })
    };
    let drew = |h: &Harness| {
        h.shapes
            .iter()
            .any(|s| matches!(s, egui::Shape::Mesh(m) if m.texture_id == bar))
    };
    assert!(drew(&h) && top_line(&h), "away: from the line of the bar");
    h.frame(vec![egui::Event::PointerMoved(
        paper + Vec2::new(40.0, 40.0),
    )]);
    h.frame(vec![]);
    assert!(drew(&h) && !top_line(&h), "hovered: with its bar");
}

#[test]
fn the_color_strip_shows_with_the_bar_and_a_swatch_changes_the_color() {
    let mut h = Harness::new();
    let (id, paper) = with_note(&mut h);
    // Con el puntero en la nota aparece la tira; el rosa es el segundo cuadrito.
    h.frame(vec![egui::Event::PointerMoved(paper.center())]);
    h.frame(vec![]);
    let pink = Pos2::new(paper.right() + 3.0 + 8.0, paper.top() + 1.0 + 8.0 + 11.0);
    h.click(pink, PointerButton::Primary);
    let color = |h: &Harness| {
        h.editor
            .document
            .notes
            .iter()
            .find(|n| n.id == id)
            .unwrap()
            .color
    };
    assert_eq!(color(&h), cs_core::NoteColor::Pink);
    h.editor.undo();
    assert_eq!(color(&h), cs_core::NoteColor::Yellow, "one step");
}

#[test]
fn dragging_the_bar_moves_the_note_in_one_undo_step() {
    let mut h = Harness::new();
    let (id, paper) = with_note(&mut h);
    let zoom = h.canvas.zoom;
    let bar = paper.left_top() + Vec2::new(40.0, 5.0);
    // La barra sale con el puntero sobre el papel.
    h.frame(vec![Event::PointerMoved(paper.center())]);
    h.drag(bar, bar + Vec2::new(zoom * 3.0, zoom * 2.0));
    let note = |h: &Harness| {
        h.editor
            .document
            .notes
            .iter()
            .find(|n| n.id == id)
            .cloned()
            .unwrap()
    };
    assert_eq!(note(&h).position, (3, 2));
    h.editor.undo();
    assert_eq!(note(&h).position, (0, 0), "one step");
}

#[test]
fn the_corner_resizes_and_the_circle_deletes_the_note() {
    let mut h = Harness::new();
    let (id, paper) = with_note(&mut h);
    let corner = paper.right_bottom() - Vec2::splat(3.0);
    h.drag(corner, corner + Vec2::new(30.0, 20.0));
    let note = h
        .editor
        .document
        .notes
        .iter()
        .find(|n| n.id == id)
        .cloned()
        .unwrap();
    assert_eq!(
        note.size,
        (crate::notes::NEW_SIZE.0 + 30, crate::notes::NEW_SIZE.1 + 20)
    );
    // El "○", contra el borde derecho de la nota ya agrandada.
    let circle = Pos2::new(paper.right() + 30.0 - 9.0, paper.top() + 6.0);
    h.click(circle, PointerButton::Primary);
    assert!(h.editor.document.notes.is_empty(), "deleted");
    h.editor.undo();
    assert_eq!(h.editor.document.notes.len(), 1, "undone");
}

#[test]
fn typing_in_a_note_saves_its_text_in_one_step() {
    let mut h = Harness::new();
    let (id, _) = with_note(&mut h);
    h.canvas.notes.start_editing(&mut h.editor, id);
    h.frame(vec![]);
    h.frame(vec![egui::Event::Text("hola".into())]);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    h.frame(vec![]);
    let note = h.editor.document.notes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(note.text, "hola");
    h.editor.undo();
    let note = h.editor.document.notes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(note.text, "", "one step");
}

/// Dónde está dibujado el texto `label` en el último frame (los "…" del menú pixel son los
/// de la fuente, `tab_bar::ellipsis`).
fn text_rect(h: &Harness, label: &str) -> Option<Rect> {
    let shown = label.replace('…', crate::tab_bar::ellipsis());
    h.shapes.iter().find_map(|s| match s {
        egui::Shape::Text(t) if t.galley.text() == shown => Some(t.visual_bounding_rect()),
        _ => None,
    })
}

/// Con Pixel UI (el desplegable pixel, anclado al "≡" de 9 px) se abre sin tronar y cada
/// opción cierra el menú.
#[test]
fn every_item_of_the_artboard_menu_works() {
    for item in [
        "Rename",
        "Properties",
        "Duplicate",
        "Sprite Size",
        "Canvas Size",
        "Delete",
    ] {
        let mut h = Harness::new();
        h.editor.add_artboard(8, 8);
        let skin = with_window(&mut h);
        // Con el skin del frame, como en la app: el menú es el desplegable pixel.
        crate::pixel_ui::set_bubble(&h.ctx, Some(std::sync::Arc::new(skin)));
        let pixels = h.pixels_of(0);
        let menu = pixels.left_top() + Vec2::new(-2.0 + 2.0 + 4.5, -14.0 + 7.5);
        h.click(menu, PointerButton::Primary);
        h.frame(vec![]);
        let at = text_rect(&h, item).unwrap_or_else(|| panic!("no {item}"));
        h.click(at.center(), PointerButton::Primary);
        for _ in 0..3 {
            h.frame(vec![]);
        }
        assert!(text_rect(&h, item).is_none(), "{item}: the menu closed");
    }
}

#[test]
fn inactive_windows_are_dimmed_and_the_active_one_is_not() {
    let mut h = Harness::new();
    h.editor.add_artboard(16, 16);
    h.canvas.zoom = 4.0;
    let skin = with_window(&mut h);
    let id = piece_id(&skin, "artboard.frame");
    let tints: Vec<(Rect, Color32)> = h
        .shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Mesh(m) if m.texture_id == id => {
                Some((m.calc_bounds(), m.vertices[0].color))
            }
            _ => None,
        })
        .collect();
    let (first, second) = (h.pixels_of(0), h.pixels_of(1));
    let tint_of = |pixels: Rect| {
        tints
            .iter()
            .filter(|(r, _)| r.intersects(pixels.expand(1.0)))
            .map(|(_, c)| *c)
            .collect::<HashSet<_>>()
    };
    // El activo es el último que se agregó.
    assert_eq!(tint_of(second), HashSet::from([Color32::WHITE]));
    let dim = tint_of(first);
    assert_eq!(dim.len(), 1);
    assert!(!dim.contains(&Color32::WHITE), "{dim:?}");
}

#[test]
fn a_narrow_artboard_keeps_its_bar_inside_the_window() {
    let mut h = Harness::new();
    let skin = with_window(&mut h);
    let label = label_text(h.editor.artboard());
    // 16 px de artboard a 1.5 (ventana de 28): íconos sin nombre; a 1 y 0.5, solo la barra.
    for (zoom, icons) in [(1.5, true), (1.0, false), (0.5, false)] {
        h.canvas.zoom = zoom;
        h.frame(vec![]);
        let pixels = h.pixels_of(0);
        let frames = h.meshes(piece_id(&skin, "artboard.frame"));
        let frame = union(&frames);
        assert_eq!(frame.width(), pixels.width() + 4.0, "zoom {zoom}");
        assert_eq!(pixels.top() - frame.top(), 14.0, "zoom {zoom}");
        for name in ["artboard.menu", "window.close"] {
            let drawn = h.meshes(piece_id(&skin, name));
            assert_eq!(!drawn.is_empty(), icons, "{name} at {zoom}");
            assert!(drawn.iter().all(|r| frame.contains_rect(*r)), "{name}");
        }
        assert!(
            h.texts().iter().all(|(t, _)| !t.starts_with(&label[..3])),
            "no name at {zoom}"
        );
    }
}

/// Dos artboards a zoom 8 con la herramienta Artboard: el de atrás (0) en (0, 0) y el de
/// adelante (1) en `front`, el de atrás activo.
fn two_windows(front: (i32, i32)) -> (Harness, ArtboardId, ArtboardId) {
    let mut h = Harness::new();
    h.canvas.zoom = 8.0;
    h.editor.add_artboard(16, 16);
    h.editor.document.artboards[1].position = front;
    let (back, front) = (
        h.editor.document.artboards[0].id,
        h.editor.document.artboards[1].id,
    );
    h.editor.activate(back);
    h.editor.set_tool(ToolKind::Artboard);
    with_window(&mut h);
    (h, back, front)
}

#[test]
fn a_bar_covered_by_another_artboard_does_not_take_the_click() {
    // Los píxeles del de adelante tapan toda la barra del de atrás.
    let (mut h, back, front) = two_windows((0, -16));
    let at = h.pixels_of(0).left_top() + Vec2::new(60.0, -10.0);
    assert!(h.pixels_of(1).contains(at));
    h.frame(vec![Event::PointerMoved(at)]);
    assert_eq!(h.cursor, egui::CursorIcon::Move, "the front artboard");
    h.drag(at, at + Vec2::new(16.0, 8.0));
    assert_eq!(h.editor.active, front);
    assert_eq!(h.editor.document.artboards[0].position, (0, 0));
    assert_eq!(h.editor.document.artboards[1].position, (2, -15));
    assert_ne!(h.editor.active, back);
}

#[test]
fn a_visible_bar_under_another_window_still_moves_its_artboard() {
    let (mut h, back, _) = two_windows((0, -40));
    let at = h.pixels_of(0).left_top() + Vec2::new(60.0, -10.0);
    h.frame(vec![Event::PointerMoved(at)]);
    assert_eq!(h.cursor, egui::CursorIcon::Grab);
    h.drag(at, at + Vec2::new(16.0, 8.0));
    assert_eq!(h.editor.active, back);
    assert_eq!(h.editor.document.artboards[0].position, (2, 1));
    assert_eq!(h.editor.document.artboards[1].position, (0, -40));
}

/// Un artboard de 16×16 a zoom 8, con Pixel UI y la herramienta Artboard.
fn artboard_tool_window() -> Harness {
    let mut h = Harness::new();
    h.canvas.zoom = 8.0;
    h.editor.set_tool(ToolKind::Artboard);
    with_window(&mut h);
    h
}

fn rect_of(h: &Harness) -> ((i32, i32), (u32, u32)) {
    let a = &h.editor.document.artboards[0];
    (a.position, (a.sprite.width, a.sprite.height))
}

#[test]
fn with_the_artboard_tool_the_top_edge_resizes_from_the_bar() {
    let mut h = artboard_tool_window();
    let at = h.pixels_of(0).center_top() - Vec2::new(0.0, 3.0);
    h.frame(vec![Event::PointerMoved(at)]);
    assert_eq!(h.cursor, egui::CursorIcon::ResizeVertical);
    h.drag(at, at - Vec2::new(0.0, 16.0));
    assert_eq!(rect_of(&h), ((0, -2), (16, 18)));
}

#[test]
fn with_the_artboard_tool_the_middle_of_the_bar_moves() {
    let mut h = artboard_tool_window();
    let at = h.pixels_of(0).center_top() - Vec2::new(0.0, 10.0);
    h.frame(vec![Event::PointerMoved(at)]);
    assert_eq!(h.cursor, egui::CursorIcon::Grab);
    h.drag(at, at + Vec2::new(16.0, 8.0));
    assert_eq!(rect_of(&h), ((2, 1), (16, 16)));
}

#[test]
fn with_the_artboard_tool_a_top_corner_resizes_from_the_bar() {
    let mut h = artboard_tool_window();
    // Sobre la barra (y el "≡"), a 2 de la esquina de los píxeles.
    let at = h.pixels_of(0).left_top() + Vec2::new(2.0, -3.0);
    h.frame(vec![Event::PointerMoved(at)]);
    assert_eq!(h.cursor, egui::CursorIcon::ResizeNwSe);
    h.drag(at, at - Vec2::new(16.0, 16.0));
    assert_eq!(rect_of(&h), ((-2, -2), (18, 18)));
    let mut h = artboard_tool_window();
    let at = h.pixels_of(0).right_top() + Vec2::new(-2.0, -3.0);
    h.frame(vec![Event::PointerMoved(at)]);
    assert_eq!(h.cursor, egui::CursorIcon::ResizeNeSw);
    h.drag(at, at + Vec2::new(16.0, -16.0));
    assert_eq!(rect_of(&h), ((0, -2), (18, 18)));
}

#[test]
fn the_first_row_of_pixels_is_not_the_bar() {
    let mut h = Harness::new();
    h.canvas.zoom = 8.0;
    with_window(&mut h);
    let at = h.pixels_of(0).center_top();
    h.frame(vec![Event::PointerMoved(at)]);
    assert_ne!(h.cursor, egui::CursorIcon::Grab);
    assert!(!h.canvas.over_label);
}

#[test]
fn an_off_screen_artboard_draws_no_window() {
    let mut h = Harness::new();
    let skin = with_window(&mut h);
    h.canvas.pan += Vec2::new(5000.0, 0.0);
    h.frame(vec![]);
    for name in ["artboard.frame", "artboard.menu", "window.close"] {
        assert!(h.meshes(piece_id(&skin, name)).is_empty(), "{name}");
    }
    let checker = h.canvas.checker.as_ref().unwrap().id();
    assert!(h.meshes(checker).is_empty());
}

#[test]
fn without_pixel_ui_the_label_and_the_orange_border_stay() {
    let h = Harness::new();
    let pixels = h.pixels_of(0);
    let label = label_text(h.editor.artboard());
    let (_, text) = h
        .texts()
        .into_iter()
        .find(|(t, _)| *t == label)
        .expect("the label");
    assert_eq!(text.bottom(), pixels.top() - LABEL_GAP);
    assert_eq!(text.left(), pixels.left());
    let border = h.shapes.iter().any(|s| {
        matches!(s, egui::Shape::Rect(r)
            if r.rect == pixels && r.stroke == Stroke::new(2.0, theme::ACCENT))
    });
    assert!(border, "the orange border of the active artboard");
    let skin = Skin::embedded(&h.ctx);
    assert!(h.meshes(piece_id(&skin, "artboard.frame")).is_empty());
}

#[test]
fn with_a_fractional_pan_the_frame_hugs_the_drawn_pixels_with_one_white_point() {
    let mut h = Harness::new();
    let skin = with_window(&mut h);
    let id = h.editor.artboard().id;
    for zoom in [1.0, 8.0] {
        for pan in [
            Vec2::new(0.37, 0.81),
            Vec2::new(0.62, 0.5),
            Vec2::new(0.5, 0.12),
        ] {
            h.canvas.zoom = zoom;
            h.canvas.pan = pan;
            h.frame(vec![]);
            let at = format!("zoom {zoom}, pan {pan:?}");
            // Los píxeles y el damero se dibujan alineados a píxeles de pantalla.
            let pixels = h.pixels_of(0).round_to_pixels(1.0);
            let tex = h.canvas.textures[&id].id();
            assert_eq!(h.meshes(tex), vec![pixels], "texture, {at}");
            let checker = h.canvas.checker.as_ref().unwrap().id();
            assert_eq!(h.meshes(checker), vec![pixels], "checker, {at}");
            // El marco: borde (1) + blanco (1) a los lados y abajo, barra (13) + blanco (1)
            // arriba; ni un píxel más ni menos.
            let frame = union(&h.meshes(piece_id(&skin, "artboard.frame")));
            assert_eq!(
                frame,
                Rect::from_min_max(
                    pixels.min - Vec2::new(2.0, 14.0),
                    pixels.max + Vec2::new(2.0, 2.0)
                ),
                "{at}"
            );
        }
    }
}

/// Las líneas de 1 px dibujadas con `color` (guías), como `(x o y, eje)`.
fn lines_of(h: &Harness, color: Color32) -> Vec<(f32, Axis)> {
    h.shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::LineSegment { points, stroke } if stroke.color == color => {
                let [a, b] = *points;
                Some(if a.x == b.x {
                    (a.x, Axis::Vertical)
                } else {
                    (a.y, Axis::Horizontal)
                })
            }
            _ => None,
        })
        .collect()
}

#[test]
fn with_a_fractional_pan_texture_corners_guides_and_hover_agree() {
    let mut h = Harness::new();
    h.view.extras = true;
    h.view.guides = true;
    h.editor.set_guides(vec![
        Guide {
            axis: Axis::Vertical,
            position: 3,
        },
        Guide {
            axis: Axis::Horizontal,
            position: 5,
        },
    ]);
    let id = h.editor.artboard().id;
    h.canvas.zoom = 8.0;
    for pan in [
        Vec2::new(0.37, 0.81),
        Vec2::new(0.62, 0.5),
        Vec2::new(-0.5, 0.12),
        Vec2::new(-3.7, 11.49),
    ] {
        h.canvas.pan = pan;
        h.frame(vec![]);
        let at = format!("pan {pan:?}");
        // La textura dibujada y `to_screen` de las esquinas son el mismo rect.
        let drawn = h.meshes(h.canvas.textures[&id].id());
        assert_eq!(drawn, vec![h.pixels_of(0)], "texture vs to_screen, {at}");
        let drawn = drawn[0];
        // Las guías caen en la fila/columna de píxeles de pantalla que empieza en su borde.
        let mut guides = lines_of(&h, theme::GUIDE_LINE);
        guides.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut expected = vec![
            (drawn.left() + 3.0 * 8.0 + 0.5, Axis::Vertical),
            (drawn.top() + 5.0 * 8.0 + 0.5, Axis::Horizontal),
        ];
        expected.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(guides, expected, "guides, {at}");
        // El píxel bajo el puntero es el que se ve: justo a cada lado del borde (3, 3).
        let edge = drawn.min + Vec2::splat(3.0 * 8.0);
        h.frame(vec![Event::PointerMoved(edge + Vec2::splat(0.05))]);
        assert_eq!(h.canvas.hover, Some((3, 3)), "inside, {at}");
        h.frame(vec![Event::PointerMoved(edge - Vec2::splat(0.05))]);
        assert_eq!(h.canvas.hover, Some((2, 2)), "outside, {at}");
    }
}

#[test]
fn the_origin_snaps_to_physical_pixels() {
    let mut h = Harness::new();
    for ppp in [1.0, 1.5, 2.0] {
        h.canvas.ppp = ppp;
        h.canvas.pan = Vec2::new(0.37, -0.81);
        let o = h.canvas.origin(h.rect) * ppp;
        assert_eq!(o, o.round(), "ppp {ppp}");
        let corner = h.canvas.to_screen(h.rect, (3, -2)) * ppp;
        assert_eq!(corner, corner.round(), "ppp {ppp}");
    }
}

#[test]
fn panning_keeps_the_stored_pan_and_zoom_keeps_the_point_under_the_cursor() {
    let mut h = Harness::new();
    h.canvas.zoom = 8.0;
    let start = h.canvas.origin(h.rect);
    // Pans fraccionarios repetidos: el pan guardado no se redondea, no hay deriva.
    for _ in 0..10 {
        h.canvas.pan += Vec2::new(0.3, -0.3);
    }
    h.frame(vec![]);
    let moved = h.canvas.origin(h.rect) - start;
    assert_eq!(moved, Vec2::new(3.0, -3.0));
    // Zoom hacia el cursor: el punto del mundo bajo el cursor sigue ahí (a < 1 px físico).
    for ppp in [1.0, 2.0] {
        h.canvas.ppp = ppp;
        for step_in in [true, true, false, true, false, false] {
            let anchor = Pos2::new(311.37, 207.81);
            let (wx, wy) = h.canvas.to_world(h.rect, anchor);
            h.canvas.zoom_at(h.rect, anchor, step_in);
            let back = h.canvas.origin(h.rect) + Vec2::new(wx, wy) * h.canvas.zoom;
            let off = (back - anchor) * ppp;
            assert!(
                off.x.abs() <= 1.0 && off.y.abs() <= 1.0,
                "ppp {ppp}, zoom {}: {off:?}",
                h.canvas.zoom
            );
        }
    }
}
