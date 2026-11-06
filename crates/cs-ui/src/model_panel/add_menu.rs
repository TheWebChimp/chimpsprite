//! El menú del + de la tira de pestañas del panel 3D, como el boceto del usuario
//! (`.refs/chimpcad/mockup-add-menu.png`, a 1×): una ventanita colgada del + (la caja de los
//! menús, `menu.list`, con su sombra) con una pestañita arriba a la derecha que envuelve al +. Arriba,
//! los cinco íconos azules de los tipos (`icon.add.*`), uno cada 25 px: un clic elige el tipo; el
//! elegido lleva un contorno blanco y una flechita sobre la línea divisoria (las dos se dibujan
//! aquí, así cualquier ícono puede ir elegido). Debajo de la línea, Sides (un stepper del kit,
//! deshabilitado para el cubo y el plano) y, abajo a la derecha, el botón Add <Tipo>, que agrega
//! el objeto y cierra la ventanita. Escape o un clic afuera la cierran sin agregar nada.

use cs_3d::Primitive;
use cs_core::PrimitiveKind;
use eframe::egui::{self, Color32, Rect, Sense, Shape, Ui, UiBuilder, Vec2, pos2, vec2};

use super::{MAX_SIDES, MIN_SIDES, ModelPanelState};
use crate::kit;
use crate::kit::button::ButtonState;
use crate::menus;
use crate::pixel_draw;
use crate::skin::{Skin, WidgetState};

/// Los tipos, en el orden del boceto.
const KINDS: [PrimitiveKind; 5] = [
    PrimitiveKind::Cube,
    PrimitiveKind::Cylinder,
    PrimitiveKind::Cone,
    PrimitiveKind::Sphere,
    PrimitiveKind::Plane,
];
/// La caja (sin la sombra ni la pestaña): 136×87, con lo alto 10 filas debajo de lo alto del +
/// (la pestaña son esas 10 filas y las 2 de la caja) y el borde derecho con el del +.
const SIZE: Vec2 = vec2(136.0, 87.0);
const TAB_H: f32 = 10.0;
/// Los íconos: cada uno centrado en su lugar de 25 px (el primero, 6 px adentro de la caja; si
/// sobra un píxel, a la derecha), con lo alto donde lo tiene en el boceto (en el orden de
/// [`KINDS`]; el cubo, más arriba). Lo que se toca es el lugar entero, de 25 de alto desde la
/// fila 5.
const ICONS_LEFT: f32 = 6.0;
const ICON_PITCH: f32 = 25.0;
const ICON_TOPS: [f32; 5] = [7.0, 9.0, 10.0, 10.0, 9.0];
const SLOT_TOP: f32 = 5.0;
/// La línea divisoria (la del separador de los menús, de borde a borde): 33 filas adentro. La
/// flechita del elegido sube 3 filas de ella, con la punta en la columna del medio del ícono.
const LINE_TOP: f32 = 33.0;
/// Sides: el texto 7 px adentro, con las mayúsculas a la altura de las del número; el stepper,
/// 32 px adentro y 40 filas abajo, de 58 de ancho.
const SIDES_LABEL_LEFT: f32 = 7.0;
const STEPPER_LEFT: f32 = 32.0;
const STEPPER_TOP: f32 = 40.0;
const STEPPER_W: f32 = 58.0;
/// Add <Tipo>: de su ancho justo, 62 filas abajo y 8 px antes del borde derecho.
const BUTTON_TOP: f32 = 62.0;
const BUTTON_RIGHT: f32 = 8.0;

/// La primitiva `kind` con `sides` lados (las esferas, con la mitad de anillos).
fn primitive(kind: PrimitiveKind, sides: u32) -> Primitive {
    let sides = sides.clamp(MIN_SIDES, MAX_SIDES);
    match kind {
        PrimitiveKind::Cube => Primitive::Cube,
        PrimitiveKind::Plane => Primitive::Plane,
        PrimitiveKind::Cylinder => Primitive::Cylinder { sides },
        PrimitiveKind::Cone => Primitive::Cone { sides },
        PrimitiveKind::Sphere => Primitive::Sphere {
            sides,
            rings: sides / 2,
        },
    }
}

/// Si `kind` tiene lados (los redondos).
fn has_sides(kind: PrimitiveKind) -> bool {
    !matches!(kind, PrimitiveKind::Cube | PrimitiveKind::Plane)
}

/// El nombre de `kind` ("Cube", "Cylinder"…).
fn name(kind: PrimitiveKind) -> &'static str {
    primitive(kind, MIN_SIDES).name()
}

/// La pieza del ícono azul de `kind`.
fn icon(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Cube => "icon.add.cube",
        PrimitiveKind::Plane => "icon.add.plane",
        PrimitiveKind::Cylinder => "icon.add.cylinder",
        PrimitiveKind::Cone => "icon.add.cone",
        PrimitiveKind::Sphere => "icon.add.sphere",
    }
}

/// El + de la tira en `rect` (un botón del kit con el ícono `icon.tab_add`) y su ventanita: lo
/// que se agregó con Add.
pub(super) fn plus_button(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    state: &mut ModelPanelState,
) -> Option<Primitive> {
    let id = ui.id().with("model_add_object");
    let plus = kit::tabs::ADD_MARK;
    let response = kit::button::icon_button_at(ui, skin, rect, id, plus, "Add object");
    let popup_id = egui::Popup::default_response_id(&response);
    if std::mem::take(&mut state.open_add_menu) {
        egui::Popup::open_id(ui.ctx(), popup_id);
    }
    // Abierta, un telón transparente debajo de ella tapa todo lo demás: un clic afuera solo la
    // cierra y no llega al widget de abajo (egui lo dejaría pasar).
    if egui::Popup::is_id_open(ui.ctx(), popup_id) {
        veil(ui.ctx(), id.with("veil"));
    }
    // La esquina de arriba a la derecha de la ventanita (con su sombra) va 10 filas debajo de lo
    // alto del +, con la caja terminando con él.
    let anchor = Rect::from_min_max(
        rect.min,
        pos2(rect.right() + menus::PANEL_SHADOW, rect.top() + TAB_H),
    );
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .anchor(anchor)
        .align(egui::RectAlign::BOTTOM_END)
        .align_alternatives(&[])
        .gap(0.0)
        .frame(egui::Frame::NONE)
        .show(|ui| window(ui, skin, rect, state))
        .and_then(|inner| {
            // Arriba de todo, como los menús que salen de un panel del dock.
            response.ctx.move_to_top(inner.response.layer_id);
            inner.inner
        })
}

/// Un área invisible de toda la ventana que se lleva los clics (y el puntero) de lo que tiene
/// debajo. La ventanita va arriba de ella (se sube al frente después).
fn veil(ctx: &egui::Context, id: egui::Id) {
    let screen = ctx.content_rect();
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.allocate_rect(
                Rect::from_min_size(screen.min, screen.size()),
                Sense::click(),
            );
        });
}

/// Lo de adentro de la ventanita colgada del + en `plus`: la caja, la pestaña, los íconos, la
/// línea, Sides y Add. Devuelve lo agregado (y entonces la cierra).
fn window(ui: &mut Ui, skin: &Skin, plus: Rect, state: &mut ModelPanelState) -> Option<Primitive> {
    let (slot, _) = ui.allocate_exact_size(SIZE + Vec2::splat(menus::PANEL_SHADOW), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), SIZE);
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let mut painter = ui.painter().clone();
    painter.set_opacity(1.0);
    // La caja con su sombra y la pestaña que envuelve al +.
    let panel = skin.piece(menus::PANEL, WidgetState::Normal);
    let fill = panel
        .and_then(|p| p.center.map(|c| p.pixel(c.x, c.y)))
        .unwrap_or(colors.panel);
    match panel {
        Some(piece) => {
            let shadow = Vec2::splat(menus::PANEL_SHADOW);
            painter.extend(crate::pixel_ui::piece_shapes(
                piece,
                Rect::from_min_max(rect.min, rect.max + shadow),
                ppp,
            ));
            tab(&painter, piece, rect, plus, ppp);
        }
        None => {
            painter.add(egui::Frame::popup(ui.style()).paint(rect));
        }
    }
    painter.extend(pixel_draw::piece_centered(
        skin,
        kit::tabs::ADD_MARK,
        plus,
        ppp,
    ));
    // La línea divisoria: la del separador de los menús, de borde a borde.
    let line = Rect::from_min_size(
        pos2(rect.left() + 1.0, rect.top() + LINE_TOP),
        vec2(rect.width() - 2.0, 2.0),
    );
    let separator = skin.piece(menus::SEPARATOR, WidgetState::Normal);
    let (light, dark) = match separator {
        Some(piece) => {
            painter.extend(crate::pixel_ui::piece_shapes(piece, line, ppp));
            let center = piece.center.map_or(0, |c| c.x);
            (piece.pixel(center, 0), piece.pixel(center, 1))
        }
        None => {
            painter.extend(pixel_rects(
                &[(line.min, colors.menu_highlight)],
                line.width(),
            ));
            (colors.menu_highlight, colors.canvas)
        }
    };
    // Los íconos.
    let base = ui.id().with("model_add_menu");
    for (k, kind) in KINDS.into_iter().enumerate() {
        let left = rect.left() + ICONS_LEFT + k as f32 * ICON_PITCH;
        let place = Rect::from_min_size(
            pos2(left, rect.top() + SLOT_TOP),
            vec2(ICON_PITCH, ICON_PITCH),
        );
        let on = state.add_kind == kind;
        let response = ui.interact(place, base.with(("kind", k)), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, name(kind))
        });
        if response.clicked() {
            state.add_kind = kind;
        }
        let Some(piece) = skin.piece(icon(kind), WidgetState::Normal) else {
            continue;
        };
        let w = piece.size[0];
        let at = pos2(
            left + ((ICON_PITCH - w as f32) / 2.0).floor(),
            rect.top() + ICON_TOPS[k],
        );
        let look = ButtonState::of(&response, true);
        painter.extend(kit::icon::icon_shapes(piece, at, look, ppp));
        if on {
            let drop = if look == ButtonState::Pressed {
                1.0
            } else {
                0.0
            };
            painter.extend(outline(piece, at + vec2(0.0, drop), colors.menu_highlight));
            let tip = at.x + ((w - 1) / 2) as f32;
            painter.extend(notch(tip, line.top(), light, dark, fill));
        }
    }
    // Sides: el texto y su stepper (sin lados, deshabilitado).
    let kind = state.add_kind;
    let stepper = Rect::from_min_size(
        pos2(rect.left() + STEPPER_LEFT, rect.top() + STEPPER_TOP),
        vec2(STEPPER_W, kit::stepper::HEIGHT),
    );
    let text = menus::galley(ui, "Sides", colors.text);
    let caps = stepper.top() + kit::input::CAPS_TOP;
    let at = crate::tab_bar::text_pos(&text, rect.left() + SIDES_LABEL_LEFT, caps, ppp);
    painter.galley(at, text, colors.text);
    let mut builder = UiBuilder::new().max_rect(stepper);
    if !has_sides(kind) {
        builder = builder.disabled();
    }
    ui.scope_builder(builder, |ui| {
        let range = MIN_SIDES..=MAX_SIDES;
        let sides = &mut state.sides;
        kit::stepper::stepper_at(ui, skin, stepper, "model_sides", sides, range, 1, None);
    });
    // Add <Tipo>.
    let label = format!("Add {}", name(kind));
    let w = kit::button::width(ui, &label);
    let button = Rect::from_min_size(
        pos2(rect.right() - BUTTON_RIGHT - w, rect.top() + BUTTON_TOP),
        vec2(w, kit::button::HEIGHT),
    );
    let add = kit::button::button_at(ui, skin, button, base.with("add"), &label);
    add.clicked().then(|| {
        ui.close();
        primitive(kind, state.sides)
    })
}

/// La pestaña que une la caja `rect` con el + en `plus`: la misma caja (`piece`) desde lo alto
/// del + hacia abajo, sin la línea entre las dos (en la fila de abajo de su borde, solo lo de
/// adentro de la pestaña) y con su sombra a la derecha hasta donde empieza la de la caja.
fn tab(painter: &egui::Painter, piece: &crate::skin::Piece, rect: Rect, plus: Rect, ppp: f32) {
    let shadow = menus::PANEL_SHADOW;
    let tall = Rect::from_min_max(
        plus.min,
        pos2(plus.right() + shadow, rect.bottom() + shadow),
    );
    let shapes = crate::pixel_ui::piece_shapes(piece, tall, ppp);
    let edge = Rect::from_min_max(plus.min, pos2(plus.right(), rect.top() + 1.0));
    let inside = Rect::from_min_max(
        pos2(plus.left() + 1.0, rect.top() + 1.0),
        pos2(plus.right(), rect.top() + 2.0),
    );
    let mut clips = vec![edge, inside];
    // La sombra de la pestaña, columna por columna, hasta la fila donde empieza la de la caja.
    let [w, h] = piece.size;
    for c in 0..shadow as u32 {
        let x = w - shadow as u32 + c;
        let start = (0..h).find(|&y| piece.pixel(x, y).a() > 0).unwrap_or(h);
        let left = plus.right() + c as f32;
        clips.push(Rect::from_min_max(
            pos2(left, plus.top()),
            pos2(left + 1.0, rect.top() + start as f32),
        ));
    }
    for clip in clips {
        painter.with_clip_rect(clip).extend(shapes.iter().cloned());
    }
}

/// El contorno de 1 px alrededor de `piece` dibujada en `at`: los píxeles vacíos que tocan uno
/// lleno por un lado (no en diagonal), en `color`.
fn outline(piece: &crate::skin::Piece, at: egui::Pos2, color: Color32) -> Vec<Shape> {
    let [w, h] = piece.size;
    let full = |x: i32, y: i32| x >= 0 && y >= 0 && piece.pixel(x as u32, y as u32).a() > 0;
    let mut cells = Vec::new();
    for y in -1..=h as i32 {
        for x in -1..=w as i32 {
            let near = full(x - 1, y) || full(x + 1, y) || full(x, y - 1) || full(x, y + 1);
            if !full(x, y) && near {
                cells.push((at + vec2(x as f32, y as f32), color));
            }
        }
    }
    pixel_rects(&cells, 1.0)
}

/// La flechita del elegido sobre la línea divisoria que empieza en la fila `top`: la línea sube
/// en una ^ de 3 filas con la punta en la columna `tip` (la clara por fuera, la oscura por
/// dentro) y adentro queda el relleno de la caja.
fn notch(tip: f32, top: f32, light: Color32, dark: Color32, fill: Color32) -> Vec<Shape> {
    let mut cells = Vec::new();
    let mut put = |dx: f32, dy: f32, color: Color32| cells.push((pos2(tip + dx, top + dy), color));
    put(0.0, -3.0, light);
    for (dx, color) in [(-1.0, light), (0.0, dark), (1.0, light)] {
        put(dx, -2.0, color);
    }
    for (dx, color) in [
        (-2.0, light),
        (-1.0, dark),
        (0.0, fill),
        (1.0, dark),
        (2.0, light),
    ] {
        put(dx, -1.0, color);
    }
    for (dx, color) in [
        (-2.0, dark),
        (-1.0, fill),
        (0.0, fill),
        (1.0, fill),
        (2.0, dark),
    ] {
        put(dx, 0.0, color);
    }
    for dx in -2..=2 {
        put(dx as f32, 1.0, fill);
    }
    pixel_rects(&cells, 1.0)
}

/// Rects de 1 fila y `width` de ancho desde cada punto, de su color.
fn pixel_rects(cells: &[(egui::Pos2, Color32)], width: f32) -> Vec<Shape> {
    cells
        .iter()
        .map(|&(at, color)| {
            Shape::rect_filled(Rect::from_min_size(at, vec2(width, 1.0)), 0.0, color)
        })
        .collect()
}

/// Sin Pixel UI: el + de la fila de egui con un menú de egui con lo mismo (los tipos, Sides y
/// Add <Tipo>). Devuelve lo agregado.
pub(super) fn plain_menu(ui: &mut Ui, state: &mut ModelPanelState) -> Option<Primitive> {
    let config = egui::containers::menu::MenuConfig::new()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    let (_, inner) = egui::containers::menu::MenuButton::new("+")
        .config(config)
        .ui(ui, |ui| {
            ui.horizontal(|ui| {
                for kind in KINDS {
                    if ui
                        .selectable_label(state.add_kind == kind, name(kind))
                        .clicked()
                    {
                        state.add_kind = kind;
                    }
                }
            });
            let kind = state.add_kind;
            ui.horizontal(|ui| {
                ui.label("Sides");
                ui.add_enabled(
                    has_sides(kind),
                    egui::DragValue::new(&mut state.sides).range(MIN_SIDES..=MAX_SIDES),
                );
            });
            let added = ui.button(format!("Add {}", name(kind))).clicked();
            added.then(|| {
                ui.close();
                primitive(kind, state.sides)
            })
        });
    inner.and_then(|i| i.inner)
}
