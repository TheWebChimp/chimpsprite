//! Los controles de la barra de opciones con Pixel UI, como en el diseño (`Design.aseprite`,
//! frame 4, capa Mockup). Las medidas son en filas y columnas desde el borde de arriba de la
//! barra (la y 71 del diseño):
//!
//! - **Textos:** las mayúsculas en la fila 5 (y 76), en el color `text`. Se dibujan a mano porque
//!   egui centra el texto en la fila y lo deja una más abajo.
//! - **Combo:** `field.combo` en 9-slice, de las filas 2 a 17. El texto empieza 6 px adentro
//!   (borde y 5 de relleno), y entre su tinta y la parte de la flecha (las 10 columnas del borde
//!   derecho de la pieza) quedan 5. La lista es el panel de los menús (`menus::dropdown`).
//! - **Número:** `field.number`, de 27 de ancho, con un `DragValue` de egui sin marco encima: se
//!   arrastra o se escribe como siempre. Con unidad, `field.unit` ("px") tapa sus 2 últimas
//!   columnas.
//! - **Casilla:** `field.check.on`, 13×14 desde la fila 3, y el texto 3 px después. Sin marcar va
//!   `field.check`; mientras el skin no la traiga (el diseño no la muestra), un cuadro plano de
//!   respaldo, como en los menús.

use std::ops::RangeInclusive;
use std::sync::Arc;

use eframe::egui::{
    self, Color32, Galley, Layout, Mesh, Rect, Response, Sense, Shape, Stroke, StrokeKind,
    TextStyle, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType, emath, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::menus::{self, MenuLook};
use crate::pixel_ui;
use crate::skin::{Piece, Skin, WidgetState};
use crate::tab_bar;

const COMBO: &str = "field.combo";
const NUMBER: &str = "field.number";
const UNIT: &str = "field.unit";
const CHECK: &str = "field.check";
const CHECK_ON: &str = "field.check.on";

/// La fila de las mayúsculas de los textos, desde el borde de arriba de la barra.
pub const CAPS_TOP: f32 = 5.0;
/// La fila de arriba del combo y del campo numérico (y 73 en el diseño), que miden 16 con el
/// brillo de abajo.
const FIELD_TOP: f32 = 2.0;
const FIELD_HEIGHT: f32 = 16.0;
/// Combo: de su borde izquierdo a la tinta del texto, y de la tinta a la parte de la flecha.
const COMBO_TEXT_LEFT: f32 = 6.0;
const COMBO_TEXT_GAP: f32 = 5.0;
/// Sin la pieza: el ancho de la parte de la flecha.
const FALLBACK_COMBO_RIGHT: f32 = 10.0;
/// El campo numérico de borde a borde: 1 de borde, 25 de blanco y 1 de borde (en el diseño, x
/// 278..304, donde la unidad pone su línea divisoria).
const NUMBER_WIDTH: f32 = 27.0;
/// La unidad empieza en la última columna blanca del campo y tapa su esquina.
const UNIT_OVERLAP: f32 = 2.0;
const FALLBACK_UNIT_WIDTH: f32 = 17.0;
/// La casilla, una fila más abajo que los campos (en el diseño, y 74..87).
const CHECK_TOP: f32 = 3.0;
const FALLBACK_CHECK_SIZE: Vec2 = vec2(13.0, 14.0);
/// De la casilla a la tinta de su texto.
const CHECK_GAP: f32 = 3.0;

/// Lo que necesitan los controles pixel de la barra: el skin y la fila de arriba de la barra.
#[derive(Clone, Copy)]
pub struct BarLook<'a> {
    pub skin: &'a Skin,
    pub top: f32,
}

impl BarLook<'_> {
    fn piece(&self, name: &str) -> Option<&Piece> {
        self.skin.piece(name, WidgetState::Normal)
    }
}

fn galley(ui: &Ui, text: &str, color: Color32) -> Arc<Galley> {
    ui.painter()
        .layout_no_wrap(text.to_owned(), TextStyle::Body.resolve(ui.style()), color)
}

/// La tinta de un texto de la fuente pixel: su ancho menos el píxel vacío del final.
fn ink_width(text: &Galley) -> f32 {
    (text.size().x - 1.0).max(0.0).round()
}

fn size_of(piece: &Piece) -> Vec2 {
    vec2(piece.size[0] as f32, piece.size[1] as f32)
}

/// Un rect de color liso, alineado a píxeles físicos.
fn solid(rect: Rect, color: Color32, ppp: f32) -> Shape {
    let mut mesh = Mesh::default();
    mesh.add_colored_rect(rect.round_to_pixels(ppp), color);
    Shape::mesh(mesh)
}

/// Un cuadro plano de respaldo (sin la pieza): `panel` con borde `outline`.
fn flat_box(look: BarLook, rect: Rect, ppp: f32) -> Vec<Shape> {
    let colors = look.skin.colors();
    vec![
        solid(rect, colors.panel, ppp),
        Shape::rect_stroke(
            rect.round_to_pixels(ppp),
            0.0,
            Stroke::new(1.0, colors.outline),
            StrokeKind::Inside,
        ),
    ]
}

/// El pintor de los controles: el de `ui`, recortado a lo ancho como el scroll de la barra pero
/// con todas las filas de la barra (los campos bajan más que la fila de egui).
fn painter(ui: &Ui, look: BarLook) -> egui::Painter {
    let clip = ui.clip_rect();
    let rows = egui::Rangef::new(look.top, look.top + FIELD_TOP + FIELD_HEIGHT + 1.0);
    ui.painter()
        .with_clip_rect(Rect::from_x_y_ranges(clip.x_range(), rows))
}

/// Un texto de la barra en `color`, con las mayúsculas en `CAPS_TOP`.
fn text(ui: &mut Ui, look: BarLook, text: &str, color: Color32) -> Response {
    let shown = galley(ui, text, color);
    let (rect, response) = ui.allocate_exact_size(shown.size(), Sense::hover());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, enabled, text));
    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let at = tab_bar::text_pos(&shown, rect.left(), look.top + CAPS_TOP, ppp);
        painter(ui, look).galley(at, shown, color);
    }
    response
}

/// Una etiqueta de la barra, en `text`.
pub fn label(ui: &mut Ui, look: BarLook, label: &str) -> Response {
    text(ui, look, label, look.skin.colors().text)
}

/// Una ayuda de la barra, en el color tenue de `ui.weak`.
pub fn hint(ui: &mut Ui, look: BarLook, hint: &str) -> Response {
    let color = ui.visuals().weak_text_color();
    text(ui, look, hint, color)
}

/// Un combo: `field.combo` con `selected` adentro y su lista en el panel de los menús. `content`
/// arma las opciones (con [`menus::choice`]). Su nombre accesible es `selected`.
pub fn combo<R>(
    ui: &mut Ui,
    look: BarLook,
    id_salt: &str,
    selected: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> Response {
    let colors = look.skin.colors();
    let shown = galley(ui, selected, colors.text);
    let piece = look.piece(COMBO);
    let right = piece
        .and_then(|p| p.center.map(|c| (p.size[0] - c.x - c.w) as f32))
        .unwrap_or(FALLBACK_COMBO_RIGHT);
    let width = COMBO_TEXT_LEFT + ink_width(&shown) + COMBO_TEXT_GAP + right;
    let (rect, _) = ui.allocate_exact_size(vec2(width, FIELD_HEIGHT), Sense::hover());
    let response = ui.interact(rect, ui.id().with(("bar_combo", id_salt)), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, enabled, selected));
    // En la fila del diseño, en puntos enteros (con la escala entera, píxeles del skin enteros).
    let drawn = Rect::from_min_size(pos2(rect.left().round(), look.top + FIELD_TOP), rect.size());
    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let mut shapes = match piece {
            Some(piece) => pixel_ui::piece_shapes(piece, drawn, ppp),
            None => {
                let mut flat = flat_box(look, drawn, ppp);
                // La flecha de respaldo: ▾ de 5×3 con rects.
                let tip = pos2(drawn.right() - right + 2.0, look.top + CAPS_TOP + 4.0);
                for (i, w) in [5.0, 3.0, 1.0].into_iter().enumerate() {
                    let row = Rect::from_min_size(
                        pos2(tip.x + (5.0 - w) / 2.0, tip.y + i as f32),
                        vec2(w, 1.0),
                    );
                    flat.push(solid(row, colors.text, ppp));
                }
                flat
            }
        };
        let at = tab_bar::text_pos(
            &shown,
            drawn.left() + COMBO_TEXT_LEFT,
            look.top + CAPS_TOP,
            ppp,
        );
        shapes.push(Shape::galley(at, shown, colors.text));
        painter(ui, look).extend(shapes);
    }
    let menu_look = MenuLook::new(look.skin, look.top);
    menus::dropdown(&menu_look, &response, drawn, content);
    response
}

/// Las opciones de un combo de `values`, cada una con su `name`: elegir una cambia `current`.
pub fn choices<T: PartialEq + Copy>(
    ui: &mut Ui,
    current: &mut T,
    values: impl IntoIterator<Item = T>,
    name: impl Fn(T) -> &'static str,
) {
    for value in values {
        if ui
            .add(menus::choice(*current == value, name(value)))
            .clicked()
        {
            *current = value;
        }
    }
}

/// Un campo numérico: `field.number` y, con `px`, la unidad `field.unit` a la derecha. Adentro va
/// un `DragValue` de egui sin marco (se arrastra o, con un clic, se escribe), con la velocidad
/// según el rango. Devuelve su respuesta.
pub fn number<Num: emath::Numeric>(
    ui: &mut Ui,
    look: BarLook,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    px: bool,
) -> Response {
    let colors = *look.skin.colors();
    let unit = px.then(|| look.piece(UNIT));
    let unit_width = match unit {
        Some(piece) => piece.map_or(FALLBACK_UNIT_WIDTH, |p| size_of(p).x) - UNIT_OVERLAP,
        None => 0.0,
    };
    let (rect, _) = ui.allocate_exact_size(
        vec2(NUMBER_WIDTH + unit_width, FIELD_HEIGHT),
        Sense::hover(),
    );
    let field = Rect::from_min_size(
        pos2(rect.left().round(), look.top + FIELD_TOP),
        vec2(NUMBER_WIDTH, FIELD_HEIGHT),
    );
    let ppp = ui.ctx().pixels_per_point();
    let back = painter(ui, look);
    let mut shapes = match look.piece(NUMBER) {
        Some(piece) => pixel_ui::piece_shapes(piece, field, ppp),
        None => flat_box(look, field, ppp),
    };
    if let Some(unit) = unit {
        let at = pos2(field.right() - UNIT_OVERLAP, field.top());
        shapes.extend(match unit {
            Some(piece) => {
                pixel_ui::piece_shapes(piece, Rect::from_min_size(at, size_of(piece)), ppp)
            }
            None => flat_box(
                look,
                Rect::from_min_size(at, vec2(FALLBACK_UNIT_WIDTH, FIELD_HEIGHT)),
                ppp,
            ),
        });
    }
    back.extend(shapes);
    // El número, en el blanco del campo y con las mayúsculas en la fila de los textos: el
    // `DragValue` centra su texto en un rect del alto del texto.
    let probe = galley(ui, "0", colors.text);
    let caps = -tab_bar::text_pos(&probe, 0.0, 0.0, ppp).y;
    let inner = Rect::from_min_size(
        pos2(field.left() + 1.0, look.top + CAPS_TOP - caps),
        vec2(NUMBER_WIDTH - 2.0, probe.size().y),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(("bar_number", id_salt))
            .max_rect(inner)
            .layout(Layout::top_down(egui::Align::Center)),
    );
    plain_field_style(child.style_mut(), inner.size(), colors.text);
    let (start, end) = (range.start().to_f64(), range.end().to_f64());
    let speed = ((end - start) / 128.0).clamp(0.1, 1.0);
    child.add(egui::DragValue::new(value).range(range).speed(speed))
}

/// El estilo del `DragValue` del campo: del tamaño del blanco, sin marco ni fondo (los pone la
/// pieza), con el texto en `text` y sin el borde de foco de egui.
fn plain_field_style(style: &mut egui::Style, size: Vec2, text: Color32) {
    style.spacing.interact_size = size;
    style.spacing.button_padding = Vec2::ZERO;
    let v = &mut style.visuals;
    v.button_frame = false;
    v.text_edit_bg_color = Some(Color32::TRANSPARENT);
    v.selection.stroke = Stroke::NONE;
    v.override_text_color = Some(text);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_fill = Color32::TRANSPARENT;
        w.weak_bg_fill = Color32::TRANSPARENT;
        w.bg_stroke = Stroke::NONE;
        w.expansion = 0.0;
    }
}

/// Una casilla con su texto, como `ui.checkbox(checked, label)`: `field.check.on` marcada y
/// `field.check` sin marcar (sin la pieza, un cuadro plano y, marcada, un cuadrito `text`).
pub fn checkbox(ui: &mut Ui, look: BarLook, checked: &mut bool, label: &str) -> Response {
    let colors = look.skin.colors();
    let shown = galley(ui, label, colors.text);
    let on_piece = look.piece(CHECK_ON);
    let off_piece = look.piece(CHECK);
    let box_size = on_piece.or(off_piece).map_or(FALLBACK_CHECK_SIZE, size_of);
    let size = vec2(box_size.x + CHECK_GAP + shown.size().x, shown.size().y);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let on = *checked;
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, label));
    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let at = Rect::from_min_size(pos2(rect.left().round(), look.top + CHECK_TOP), box_size);
        let mut shapes = match if on { on_piece } else { off_piece } {
            Some(piece) => pixel_ui::piece_shapes(piece, at, ppp),
            None => {
                let mut flat = flat_box(look, at, ppp);
                if on {
                    flat.push(solid(at.shrink(3.0), colors.text, ppp));
                }
                flat
            }
        };
        let pos = tab_bar::text_pos(&shown, at.right() + CHECK_GAP, look.top + CAPS_TOP, ppp);
        shapes.push(Shape::galley(pos, shown, colors.text));
        painter(ui, look).extend(shapes);
    }
    response
}
