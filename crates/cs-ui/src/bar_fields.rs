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
//!   respaldo. Las de los menús (`menu.check`, `menu.check.on`) no sirven: son otro dibujo, de
//!   11×12.
//! - **Opciones** (los grupos de radios: Sample, Linear/Radial, Rotation): botones de un clic,
//!   uno por opción. El diseño no los muestra; van con la pieza `button` de los diálogos (49×17
//!   con centro) a 16 de alto en las filas de los campos, `button.active` el elegido (o
//!   `button.down` si el skin no lo trae), y el texto
//!   centrado con las mayúsculas en la fila de los textos. `sym.slot` no sirve: no tiene centro y
//!   no se estira.

use std::ops::RangeInclusive;

use eframe::egui::{
    self, Color32, Layout, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
    WidgetInfo, WidgetType, emath, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::menus::{self, MenuLook, galley, ink_width, solid};
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
/// Los botones de opción (los grupos de radios): la pieza de los botones de los diálogos, que
/// tiene centro y se estira a lo ancho; acá, con el alto de los campos.
const BUTTON: &str = "button";
/// El botón de la opción elegida. Sin él, `button.down`.
const BUTTON_ACTIVE: &str = "button.active";
/// De cada borde del botón de opción a la tinta de su texto: el mismo relleno que el combo.
const OPTION_TEXT_PAD: f32 = 6.0;
/// Entre botón y botón de opción, como entre los cuadritos de Mode (de pieza a pieza).
const OPTION_GAP: f32 = 2.0;

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

fn size_of(piece: &Piece) -> Vec2 {
    vec2(piece.size[0] as f32, piece.size[1] as f32)
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

/// Los grupos chicos de opciones exclusivas (los que sin Pixel UI son radios): una fila de
/// botones de un clic, uno por cada `value` con su `name`. El elegido va con `button.active`, los
/// otros con `button` (o `button.hover` con el puntero encima). Un clic elige esa opción; otro
/// clic en la elegida no la apaga, como con radios. Devuelve si cambió `current`.
pub fn options<T: PartialEq + Copy>(
    ui: &mut Ui,
    look: BarLook,
    current: &mut T,
    values: impl IntoIterator<Item = T>,
    name: impl Fn(T) -> &'static str,
) -> bool {
    let before = *current;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = OPTION_GAP;
        for value in values {
            if option_button(ui, look, *current == value, name(value)).clicked() {
                *current = value;
            }
        }
    });
    *current != before
}

/// Un botón de opción: la pieza `button` en 9-slice, de las filas de los campos (2 a 17), con
/// `label` centrado y las mayúsculas en `CAPS_TOP` también si está elegido (`on`).
fn option_button(ui: &mut Ui, look: BarLook, on: bool, label: &str) -> Response {
    let colors = look.skin.colors();
    let shown = galley(ui, label, colors.text);
    let width = OPTION_TEXT_PAD + ink_width(&shown) + OPTION_TEXT_PAD;
    let (rect, response) = ui.allocate_exact_size(vec2(width, FIELD_HEIGHT), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, enabled, on, label));
    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let drawn =
            Rect::from_min_size(pos2(rect.left().round(), look.top + FIELD_TOP), rect.size());
        // El elegido va con `button.active` (el relleno café de la herramienta activa) o, si el
        // skin no lo trae, con `button.down`.
        let piece = if on {
            look.skin
                .piece(BUTTON_ACTIVE, WidgetState::Normal)
                .or_else(|| look.skin.piece(BUTTON, WidgetState::Down))
        } else if response.hovered() {
            look.skin.piece(BUTTON, WidgetState::Hover)
        } else {
            look.skin.piece(BUTTON, WidgetState::Normal)
        };
        let mut shapes = match piece {
            Some(piece) => pixel_ui::piece_shapes(piece, drawn, ppp),
            None => {
                let mut flat = flat_box(look, drawn, ppp);
                if on {
                    flat.push(Shape::rect_stroke(
                        drawn.shrink(1.0).round_to_pixels(ppp),
                        0.0,
                        Stroke::new(1.0, colors.accent),
                        StrokeKind::Inside,
                    ));
                }
                flat
            }
        };
        let at = tab_bar::text_pos(
            &shown,
            drawn.left() + OPTION_TEXT_PAD,
            look.top + CAPS_TOP,
            ppp,
        );
        shapes.push(Shape::galley(at, shown, colors.text));
        painter(ui, look).extend(shapes);
    }
    response
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
    // Todo lo dibujado (el campo y la unidad) responde al clic y al arrastre, no solo el texto.
    let (start, end) = (range.start().to_f64(), range.end().to_f64());
    let speed = ((end - start) / 128.0).clamp(0.1, 1.0);
    let whole = Rect::from_min_max(
        field.min,
        pos2(rect.right().max(field.right()), field.bottom()),
    );
    let look = NumberLook {
        text: colors.text,
        outline: colors.outline,
        align: egui::Align::Center,
    };
    drag_number(ui, whole, inner, id_salt, value, range, speed, look).0
}

/// Cómo se ve el texto de [`drag_number`]: su color, el del cursor de escribir y hacia dónde se
/// alinea en su rect.
#[derive(Clone, Copy)]
pub(crate) struct NumberLook {
    pub text: Color32,
    pub outline: Color32,
    pub align: egui::Align,
}

/// Un número que se arrastra en toda la zona `whole` y se escribe con el `DragValue` de egui sin
/// marco que va en `inner` (el texto, alineado según el layout de `ui`). Un clic en la zona entra
/// a escribir. Lo usan el campo numérico de la barra y la tira de canales del editor de color.
/// Devuelve la respuesta del `DragValue` y la de la zona.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drag_number<Num: emath::Numeric>(
    ui: &mut Ui,
    whole: Rect,
    inner: Rect,
    id_salt: impl std::hash::Hash + std::fmt::Debug + Copy,
    value: &mut Num,
    range: RangeInclusive<Num>,
    speed: f64,
    look: NumberLook,
) -> (Response, Response) {
    let (start, end) = (range.start().to_f64(), range.end().to_f64());
    // La zona va debajo del `DragValue`, que gana donde se solapan.
    let zone = ui.interact(
        whole,
        ui.id().with(("bar_number_zone", id_salt)),
        Sense::click_and_drag(),
    );
    if zone.hovered() || zone.dragged() {
        ui.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if zone.drag_started() {
        ui.data_mut(|d| d.insert_temp(zone.id, value.to_f64()));
    }
    if zone.dragged() {
        // Se acumula en `f64`: con enteros y velocidad baja, cada paso solo no llegaría a 1.
        let mut acc = ui
            .data(|d| d.get_temp::<f64>(zone.id))
            .unwrap_or_else(|| value.to_f64());
        acc = (acc + f64::from(zone.drag_delta().x) * speed).clamp(start, end);
        ui.data_mut(|d| d.insert_temp(zone.id, acc));
        *value = Num::from_f64(acc);
    }
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(("bar_number", id_salt))
            .max_rect(inner)
            .layout(Layout::top_down(look.align)),
    );
    plain_field_style(child.style_mut(), inner.size(), look.text, look.outline);
    let response = child.add(egui::DragValue::new(value).range(range).speed(speed));
    // Un clic en la zona de afuera del texto también entra a escribir.
    if zone.clicked() {
        ui.memory_mut(|m| m.request_focus(response.id));
    }
    (response, zone)
}

/// El estilo del `DragValue` del campo: del tamaño del blanco, sin marco ni fondo (los pone la
/// pieza), con el texto en `text`, el cursor en `outline` y sin el borde de foco de egui.
pub(crate) fn plain_field_style(
    style: &mut egui::Style,
    size: Vec2,
    text: Color32,
    outline: Color32,
) {
    style.spacing.interact_size = size;
    style.spacing.button_padding = Vec2::ZERO;
    let v = &mut style.visuals;
    v.button_frame = false;
    v.text_edit_bg_color = Some(Color32::TRANSPARENT);
    v.selection.stroke = Stroke::NONE;
    // El cursor de escribir, fino y en el contorno del skin: el de egui es azul claro de 2 px y
    // sobre el blanco del campo casi no se ve.
    v.text_cursor.stroke = Stroke::new(1.0, outline);
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

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput};

    /// Un campo numérico (con unidad) en una barra de prueba, manejado con eventos crudos.
    struct Bar {
        ctx: egui::Context,
        skin: Skin,
        value: i32,
        time: f64,
        /// Lo que dibujó el último frame.
        shapes: Vec<egui::epaint::ClippedShape>,
    }

    impl Bar {
        fn new() -> Self {
            let ctx = egui::Context::default();
            let skin = Skin::embedded(&ctx);
            let mut bar = Self {
                ctx,
                skin,
                value: 50,
                time: 0.0,
                shapes: vec![],
            };
            bar.frame(vec![]);
            bar.frame(vec![]);
            bar
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.time += 0.05;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 100.0))),
                time: Some(self.time),
                focused: true,
                events,
                ..Default::default()
            };
            let (skin, value) = (&self.skin, &mut self.value);
            let mut out = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::no_frame().show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let look = BarLook { skin, top: 0.0 };
                    number(ui, look, "f", value, 0..=100, true);
                });
            });
            out.textures_delta.clear();
            self.shapes = out.shapes;
        }

        fn button(&mut self, pos: Pos2, pressed: bool) {
            self.frame(vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }

        fn click(&mut self, pos: Pos2) {
            self.frame(vec![Event::PointerMoved(pos)]);
            self.button(pos, true);
            self.button(pos, false);
        }

        /// Arrastra 30 puntos a la derecha desde `from`.
        fn drag(&mut self, from: Pos2) {
            self.frame(vec![Event::PointerMoved(from)]);
            self.button(from, true);
            for step in 1..=3 {
                self.frame(vec![Event::PointerMoved(
                    from + vec2(10.0 * step as f32, 0.0),
                )]);
            }
            self.button(from + vec2(30.0, 0.0), false);
        }

        fn key(&mut self, key: Key) {
            let ev = |pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            };
            self.frame(vec![ev(true), ev(false)]);
        }

        fn type_text(&mut self, text: &str) {
            self.frame(vec![Event::Text(text.to_owned())]);
        }

        fn focused(&self) -> bool {
            self.ctx.memory(|m| m.focused()).is_some()
        }

        /// Entra a escribir (clic al centro del blanco) y borra lo que había.
        fn start_editing(&mut self) {
            self.click(pos2(13.0, 10.0));
            self.frame(vec![]);
            assert!(self.focused(), "no entró a editar");
            self.frame(vec![Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }]);
            self.key(Key::Backspace);
        }

        /// ¿Se dibujó algo plano del color `color` dentro de `area`? (el cursor y la selección;
        /// no las piezas del skin, que son texturas).
        fn drew_color_in(&self, color: Color32, area: Rect) -> bool {
            fn hit(shape: &Shape, color: Color32, area: Rect) -> bool {
                match shape {
                    Shape::Vec(v) => v.iter().any(|s| hit(s, color, area)),
                    Shape::LineSegment { points, stroke } => {
                        stroke.color == color && area.intersects(Rect::from_points(points))
                    }
                    Shape::Rect(r) => {
                        (r.fill == color || r.stroke.color == color) && area.intersects(r.rect)
                    }
                    Shape::Mesh(m) => {
                        m.texture_id == egui::TextureId::default()
                            && m.vertices.iter().any(|v| v.color == color)
                            && area.intersects(m.calc_bounds())
                    }
                    _ => false,
                }
            }
            self.shapes.iter().any(|s| hit(&s.shape, color, area))
        }
    }

    #[test]
    fn dragging_from_the_top_row_of_the_field_changes_the_value() {
        let mut bar = Bar::new();
        bar.drag(pos2(13.0, FIELD_TOP + 0.5));
        assert_ne!(bar.value, 50);
    }

    #[test]
    fn dragging_from_the_bottom_row_and_the_border_changes_the_value() {
        let mut bar = Bar::new();
        bar.drag(pos2(13.0, FIELD_TOP + FIELD_HEIGHT - 0.5));
        assert_ne!(bar.value, 50);
        bar.value = 50;
        bar.drag(pos2(0.5, 10.0));
        assert_ne!(bar.value, 50);
    }

    #[test]
    fn dragging_from_the_px_unit_changes_the_value() {
        let mut bar = Bar::new();
        bar.drag(pos2(NUMBER_WIDTH + 10.0, 10.0));
        assert_ne!(bar.value, 50);
    }

    #[test]
    fn clicking_the_px_unit_starts_editing() {
        let mut bar = Bar::new();
        bar.click(pos2(NUMBER_WIDTH + 10.0, 10.0));
        bar.frame(vec![]);
        assert!(bar.focused());
    }

    #[test]
    fn clicking_the_top_row_starts_editing() {
        let mut bar = Bar::new();
        bar.click(pos2(13.0, FIELD_TOP + 0.5));
        bar.frame(vec![]);
        assert!(bar.focused());
    }

    #[test]
    fn escape_while_editing_leaves_edit_mode_keeping_only_the_value_already_applied() {
        let mut bar = Bar::new();
        bar.start_editing();
        bar.type_text("7");
        bar.key(Key::Escape);
        bar.frame(vec![]);
        // El `DragValue` de egui aplica el número mientras se escribe: Esc no lo revierte, pero sí
        // sale de editar sin dejar texto pendiente.
        assert_eq!(bar.value, 7);
        assert!(!bar.focused(), "Esc sale de editar");
        // Volver a editar parte del valor actual, no del texto a medias.
        bar.click(pos2(13.0, 10.0));
        bar.frame(vec![]);
        bar.frame(vec![]);
        assert_eq!(bar.value, 7, "entrar a editar no cambia el valor");
        bar.start_editing();
        bar.type_text("9");
        bar.key(Key::Enter);
        bar.frame(vec![]);
        assert_eq!(bar.value, 9);
    }

    #[test]
    fn clicking_away_commits_the_typed_value() {
        let mut bar = Bar::new();
        bar.start_editing();
        bar.type_text("42");
        bar.click(pos2(200.0, 80.0));
        bar.frame(vec![]);
        assert_eq!(bar.value, 42);
        assert!(!bar.focused());
    }

    #[test]
    fn enter_commits_the_typed_value() {
        let mut bar = Bar::new();
        bar.start_editing();
        bar.type_text("33");
        bar.key(Key::Enter);
        bar.frame(vec![]);
        assert_eq!(bar.value, 33);
    }

    #[test]
    fn while_editing_the_field_draws_an_edit_cursor_in_the_outline_color() {
        let mut bar = Bar::new();
        let outline = bar.skin.colors().outline;
        let field = Rect::from_min_size(pos2(0.0, FIELD_TOP), vec2(NUMBER_WIDTH, FIELD_HEIGHT));
        let inner = field.shrink(2.0);
        assert!(!bar.drew_color_in(outline, inner));
        bar.start_editing();
        bar.type_text("4");
        // El cursor parpadea: se mira en varios frames.
        let mut seen = false;
        for _ in 0..40 {
            bar.frame(vec![]);
            seen |= bar.drew_color_in(outline, inner);
        }
        assert!(seen, "no se dibujó cursor mientras se escribe");
    }
}
