//! Los controles de la barra de opciones con Pixel UI, como en el diseño (`Design.aseprite`,
//! frame 4, capa Mockup). Las medidas son en filas y columnas desde el borde de arriba de la
//! barra (la y 71 del diseño):
//!
//! - **Textos:** las mayúsculas en la fila 5 (y 76), en el color `text`. Se dibujan a mano porque
//!   egui centra el texto en la fila y lo deja una más abajo.
//! - **Combo:** el select del UI kit (`kit::select`, mockup de componentes, frame 9), desde la
//!   fila de los campos: el texto 5 px adentro, el ▾ a la derecha y su lista propia debajo.
//! - **Número:** el campo numérico del UI kit (`kit::input`), de 27 de ancho, con un `DragValue`
//!   de egui sin marco adentro: se arrastra o se escribe como siempre. Con unidad, la caja
//!   `field.unit` ("px") a la derecha; con letra, la caja `field.prefix` a la izquierda. Sus
//!   mayúsculas, en la fila 5 del campo (una más abajo que los textos de la barra).
//! - **Casilla:** la del UI kit (`kit::check`, mockup de componentes, frame 9), desde la fila de
//!   los campos: la casilla de 12×13 en la fila 3 y el texto 3 px después, en la fila de los
//!   textos.
//! - **Opciones** (los grupos de radios: Sample, Linear/Radial, Rotation): el grupo de botones
//!   del UI kit (`kit::group`, mockup de componentes, frame 9) con LEDs, desde la fila de los
//!   campos: sus mayúsculas caen en la fila de los textos.

use std::ops::RangeInclusive;

use eframe::egui::{
    self, Color32, Rect, Response, Sense, Ui, Vec2, WidgetInfo, WidgetType, emath, pos2, vec2,
};

use crate::kit::button::Led;
use crate::kit::input::{self, Affixes, NumberOptions};
pub(crate) use crate::kit::input::{NumberLook, drag_number};
use crate::kit::{self, GroupItem};
use crate::menus::{self, galley, solid};
use crate::pixel_ui;
use crate::pixel_ui::Tip as _;
use crate::skin::Skin;
use crate::tab_bar;

/// La fila de las mayúsculas de los textos, desde el borde de arriba de la barra: la del texto de
/// los campos (`kit::input::CAPS_TOP` debajo de su borde), así todo queda en una fila.
pub const CAPS_TOP: f32 = FIELD_TOP + crate::kit::input::CAPS_TOP;
/// La fila de arriba del combo y del campo numérico (y 73 en el diseño), que miden 16 con el
/// brillo de abajo.
pub(crate) const FIELD_TOP: f32 = 2.0;
pub(crate) const FIELD_HEIGHT: f32 = pixel_ui::CONTROL_HEIGHT;

/// `add` con lo de arriba a la altura de los campos de la barra (`FIELD_TOP` debajo de su
/// borde): para un control que no lo hace solo (un botón con ícono, por ejemplo).
pub(crate) fn at_field_top<R>(ui: &mut Ui, look: BarLook, add: impl FnOnce(&mut Ui) -> R) -> R {
    let drop = (look.top + FIELD_TOP - ui.cursor().top()).max(0.0);
    ui.vertical(|ui| {
        ui.add_space(drop);
        add(ui)
    })
    .inner
}
/// El campo numérico de borde a borde: 1 de borde, 25 de blanco y 1 de borde (en el diseño, x
/// 278..304, donde la unidad pone su línea divisoria).
const NUMBER_WIDTH: f32 = 27.0;

/// Lo que necesitan los controles pixel de la barra: el skin y la fila de arriba de la barra.
#[derive(Clone, Copy)]
pub struct BarLook<'a> {
    pub skin: &'a Skin,
    pub top: f32,
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

/// El ícono de info: una pieza del skin.
const INFO: &str = "bar.info";

/// El ícono de info, centrado en la fila de los campos, con `text` en su tooltip. Su nombre
/// accesible es `text`.
pub fn info(ui: &mut Ui, look: BarLook, text: &str) -> Response {
    let size = crate::pixel_draw::piece_size(look.skin, INFO);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, enabled, text));
    if ui.is_rect_visible(rect) {
        let top = look.top + FIELD_TOP + ((FIELD_HEIGHT - size.y) / 2.0).floor();
        let origin = pos2(rect.left().round(), top);
        let ppp = ui.ctx().pixels_per_point();
        let shapes = crate::pixel_draw::piece_at(look.skin, INFO, origin, ppp);
        painter(ui, look).extend(shapes);
    }
    response.tip(text)
}

/// Un combo: el select del UI kit (`kit::select`) con `selected` adentro, desde la fila de los
/// campos. `content` arma las opciones (`choices` o `widgets::choice`).
pub fn combo<R>(
    ui: &mut Ui,
    look: BarLook,
    id_salt: &str,
    selected: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> Response {
    combo_sized(ui, look, id_salt, selected, None, FIELD_HEIGHT, content)
}

/// El ancho de un [`combo`] con `selected` (para que uno con varias opciones mida lo de la más
/// larga y no brinque al cambiar).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn combo_width(ui: &Ui, _skin: &Skin, selected: &str) -> f32 {
    kit::select::width(ui, selected)
}

/// Como [`combo`], con `width`, por lo menos de ese ancho (si no, el de su texto). El select mide
/// siempre lo de un campo (`height` solo reserva el lugar).
pub fn combo_sized<R>(
    ui: &mut Ui,
    look: BarLook,
    id_salt: &str,
    selected: &str,
    width: Option<f32>,
    height: f32,
    content: impl FnOnce(&mut Ui) -> R,
) -> Response {
    let natural = kit::select::width(ui, selected);
    let width = width.map_or(natural, |w| w.max(natural)).round();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    // En la fila del diseño, en puntos enteros (con la escala entera, píxeles del skin enteros).
    let drawn = Rect::from_min_size(
        pos2(rect.left().round(), look.top + FIELD_TOP),
        vec2(width, kit::select::HEIGHT),
    );
    let id = ui.id().with(("bar_combo", id_salt));
    kit::select::select_at(ui, look.skin, drawn, id, selected, content)
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

/// Los grupos chicos de opciones exclusivas (los que sin Pixel UI son radios): el grupo de botones
/// del UI kit ([`kit::group`](crate::kit::group)), cada opción con su LED (prendido el de la
/// elegida) y su `name`, en las
/// filas de los campos. Un clic elige esa opción; otro clic en la elegida no la apaga, como con
/// radios. Devuelve si cambió `current`.
pub fn options<T: PartialEq + Copy>(
    ui: &mut Ui,
    look: BarLook,
    current: &mut T,
    values: impl IntoIterator<Item = T>,
    name: impl Fn(T) -> &'static str,
) -> bool {
    let values: Vec<T> = values.into_iter().collect();
    let items: Vec<GroupItem> = values
        .iter()
        .map(|&v| GroupItem::with_led(name(v), Led::lit(v == *current)))
        .collect();
    // Un id por grupo (puede haber varios en la barra).
    let id = ui.next_auto_id();
    let width = kit::group::width(ui, &items);
    let (rect, _) = ui.allocate_exact_size(vec2(width, FIELD_HEIGHT), Sense::hover());
    let at = pos2(rect.left().round(), look.top + FIELD_TOP);
    let painter = painter(ui, look);
    let picked = kit::group::button_group_at(ui, look.skin, &painter, id, at, &items, &|i| {
        values[i] == *current
    });
    match picked {
        Some(i) if values[i] != *current => {
            *current = values[i];
            true
        }
        _ => false,
    }
}

/// El botón de la forma del pincel, como en el diseño: un `button` de 25×16 con la huella del
/// pincel (la de `brush_points`, la misma que pinta) en el café oscuro de los íconos, a su tamaño
/// hasta `BRUSH_ICON_MAX` (más grande se ve como el de 9); el personalizado, con sus colores si
/// pinta con ellos (`colors`). Un clic pasa al redondo, al cuadrado y, si hay, al personalizado.
pub fn brush_shape(
    ui: &mut Ui,
    look: BarLook,
    shape: &mut cs_raster::BrushShape,
    size: u32,
    colors: bool,
) -> Response {
    use cs_raster::BrushShape;
    let (rect, _) = ui.allocate_exact_size(vec2(BRUSH_BUTTON_W, FIELD_HEIGHT), Sense::hover());
    let drawn = Rect::from_min_size(
        pos2(rect.left().round(), look.top + FIELD_TOP),
        vec2(BRUSH_BUTTON_W, pixel_ui::CONTROL_BODY),
    );
    let label = match shape {
        BrushShape::Round => "Round brush",
        BrushShape::Square => "Square brush",
        BrushShape::Custom => "Custom brush",
    };
    let id = ui.id().with("brush_shape");
    let ink = look.skin.colors().tab_text_dim;
    let points: Vec<((i32, i32), Color32)> = match shape {
        BrushShape::Custom if colors => cs_raster::custom_brush()
            .into_iter()
            .map(|(p, c)| (p, crate::panels::to_color32(c)))
            .collect(),
        _ => cs_raster::brush_points((0, 0), size.clamp(1, BRUSH_ICON_MAX), *shape)
            .into_iter()
            .map(|p| (p, ink))
            .collect(),
    };
    let ppp = ui.ctx().pixels_per_point();
    let response = crate::pixel_draw::skin_button(
        ui,
        look.skin,
        drawn,
        crate::pixel_draw::ButtonInfo::new(id, label),
        |inner| {
            // Centrada en el relleno del botón (sin el brillo de arriba ni las filas de abajo).
            let center = pos2(
                (inner.left() + BRUSH_ICON_CENTER.x).floor(),
                (inner.top() + BRUSH_ICON_CENTER.y).floor(),
            );
            // El personalizado puede ser más grande que el botón: lo que entra.
            points
                .iter()
                .filter(|&&((x, y), _)| x.abs() <= 10 && y.abs() <= 5)
                .map(|&((x, y), color)| {
                    let at = center + vec2(x as f32, y as f32);
                    solid(Rect::from_min_size(at, Vec2::splat(1.0)), color, ppp)
                })
                .collect()
        },
    );
    if response.clicked() {
        *shape = next_shape(*shape);
    }
    response
}

/// La forma que sigue al tocar el botón: redonda, cuadrada y, si hay uno (Edit → New Brush), el
/// pincel personalizado.
pub fn next_shape(shape: cs_raster::BrushShape) -> cs_raster::BrushShape {
    use cs_raster::BrushShape;
    match shape {
        BrushShape::Round => BrushShape::Square,
        BrushShape::Square if !cs_raster::custom_brush().is_empty() => BrushShape::Custom,
        BrushShape::Square | BrushShape::Custom => BrushShape::Round,
    }
}

/// El botón de la forma del pincel: su ancho, el píxel del centro de la huella (desde su esquina)
/// y el tamaño más grande que se dibuja.
const BRUSH_BUTTON_W: f32 = 25.0;
const BRUSH_ICON_CENTER: Vec2 = vec2(12.0, 7.0);
const BRUSH_ICON_MAX: u32 = 9;

/// Un campo numérico del UI kit ([`kit::input`](crate::kit::input)) en las filas de los campos:
/// de `NUMBER_WIDTH` y, con `px`, la caja de la unidad a la derecha (el campo crece lo que mide la
/// caja menos su columna de borde). El número va centrado en el blanco; se arrastra o, con un
/// clic, se escribe. Devuelve su respuesta.
pub fn number<Num: emath::Numeric>(
    ui: &mut Ui,
    look: BarLook,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    px: bool,
) -> Response {
    let affixes = if px {
        Affixes::suffix("px")
    } else {
        Affixes::NONE
    };
    let unit = affixes
        .suffix
        .map_or(0.0, |u| input::box_width(ui, u) - input::OUTLINE);
    let options = NumberOptions {
        affixes,
        align: egui::Align::Center,
        speed: None,
    };
    bar_number(
        ui,
        look,
        NUMBER_WIDTH + unit,
        id_salt,
        value,
        range,
        options,
    )
}

/// Un campo numérico con su unidad (el `%` de Sprite Size) en la caja de la derecha, de
/// `SUFFIX_FIELD_WIDTH`, con el número a la izquierda.
pub fn number_with_suffix<Num: emath::Numeric>(
    ui: &mut Ui,
    look: BarLook,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    suffix: &str,
) -> Response {
    let options = NumberOptions {
        affixes: Affixes::suffix(suffix),
        ..Default::default()
    };
    bar_number(ui, look, SUFFIX_FIELD_WIDTH, id_salt, value, range, options)
}

/// Un campo numérico con una letra adelante (la `W` de "W 11.47"), en la caja del prefijo, de
/// `PREFIX_FIELD_WIDTH`, con el número en el blanco que queda.
pub fn number_with_prefix<Num: emath::Numeric>(
    ui: &mut Ui,
    look: BarLook,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    prefix: &str,
) -> Response {
    let (start, end) = (range.start().to_f64(), range.end().to_f64());
    let options = NumberOptions {
        affixes: Affixes::prefix(prefix),
        speed: Some(((end - start) / 128.0).clamp(0.01, 1.0)),
        ..Default::default()
    };
    bar_number(ui, look, PREFIX_FIELD_WIDTH, id_salt, value, range, options)
}

/// El campo de un eje (la barrita del objeto de ChimpCAD): la caja del prefijo con la letra
/// `axis` y el número centrado en el blanco, de `AXIS_FIELD_WIDTH`; un punto arrastrado es uno
/// más. Su nombre accesible es `name` ("Position X"). Sin `value`, el campo vacío y
/// deshabilitado.
pub fn axis_number(
    ui: &mut Ui,
    look: BarLook,
    name: &str,
    axis: &str,
    value: Option<&mut i32>,
    range: RangeInclusive<i32>,
) -> Response {
    let affixes = Affixes::prefix(axis);
    let enabled = ui.is_enabled() && value.is_some();
    let response = match value {
        Some(value) => {
            let options = NumberOptions {
                affixes,
                align: egui::Align::Center,
                speed: Some(1.0),
            };
            bar_number(ui, look, AXIS_FIELD_WIDTH, name, value, range, options)
        }
        None => {
            let size = vec2(AXIS_FIELD_WIDTH, FIELD_HEIGHT);
            let (slot, _) = ui.allocate_exact_size(size, Sense::hover());
            let rect = Rect::from_min_size(
                pos2(slot.left().round(), look.top + FIELD_TOP),
                vec2(AXIS_FIELD_WIDTH, input::HEIGHT),
            );
            let id = ui.id().with(("bar_axis_empty", name));
            let response = ui.interact(rect, id, Sense::hover());
            let state = input::InputState::Disabled;
            let shapes = input::frame_shapes(ui, look.skin, rect, state, affixes);
            let mut painter = painter(ui, look);
            painter.set_opacity(1.0);
            painter.extend(shapes);
            response
        }
    };
    response.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, enabled, name));
    response
}

/// [`axis_number`], de borde a borde: la caja de la letra y lugar para "-4096".
const AXIS_FIELD_WIDTH: f32 = 41.0;

/// El campo numérico del kit de `width` en las filas de los campos de la barra.
fn bar_number<Num: emath::Numeric>(
    ui: &mut Ui,
    look: BarLook,
    width: f32,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    options: NumberOptions,
) -> Response {
    let (slot, _) = ui.allocate_exact_size(vec2(width, FIELD_HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(
        pos2(slot.left().round(), look.top + FIELD_TOP),
        vec2(width, input::HEIGHT),
    );
    let painter = painter(ui, look);
    input::number_input_at(
        ui, look.skin, &painter, rect, id_salt, value, range, options,
    )
}

/// [`number_with_prefix`] y [`number_with_suffix`], de borde a borde.
const PREFIX_FIELD_WIDTH: f32 = 37.0;
const SUFFIX_FIELD_WIDTH: f32 = 59.0;

/// Una casilla con su texto, como `ui.checkbox(checked, label)`: la del UI kit (`kit::check`),
/// desde la fila de los campos (sus mayúsculas caen en la fila de los textos).
pub fn checkbox(ui: &mut Ui, look: BarLook, checked: &mut bool, label: &str) -> Response {
    at_field_top(ui, look, |ui| {
        kit::check::checkbox(ui, look.skin, checked, label)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Shape};

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
