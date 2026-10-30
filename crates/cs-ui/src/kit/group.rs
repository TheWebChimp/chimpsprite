//! El grupo de botones (paso 2 del UI kit): botones de texto pegados, uno por opción, con el
//! contorno solo alrededor del grupo, como en el mockup de componentes (frame 9).
//!
//! - Cada segmento es el botón de [`super::button`] sin el contorno del lado de su vecino: la
//!   columna `#ddcebf` a la izquierda, 4 px de relleno, [el punto y 3 px], la tinta, 4 px y la
//!   columna blanca a la derecha. Alto, texto y sombra, los del botón.
//! - Entre dos segmentos, una sola columna: la pieza `button.group.sep` (1×17: el contorno arriba,
//!   `#bda499` en las filas 1 a 13 y el contorno en la del bisel, la 14).
//! - El elegido (y el presionado) se ve como el botón presionado: su cara baja 1 px (pieza
//!   `button.down`, sin el bisel oscuro) y su punto y su texto bajan con ella.
//! - Con puntos: `button.group.dot.on` (verde, el elegido) o `button.group.dot.off` (morado),
//!   6×7 con su sombra, donde empezaría el texto y en la fila de sus mayúsculas.

use eframe::egui::{
    self, Id, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use super::button::{self, ButtonState};
use crate::pixel_ui;
use crate::skin::{Piece, Skin, WidgetState};

/// La columna entre dos segmentos (y su variante `.disabled`).
pub const SEP: &str = "button.group.sep";
/// Los puntos de las opciones: el de la elegida y el de las demás.
pub const DOT_ON: &str = "button.group.dot.on";
pub const DOT_OFF: &str = "button.group.dot.off";
/// La columna del separador.
pub const SEP_W: f32 = 1.0;
/// El punto: 6 de ancho (y 7 de alto con su sombra); después, 3 px hasta la tinta.
pub const DOT_W: f32 = 6.0;
pub const DOT_GAP: f32 = 3.0;

/// Una opción del grupo: su texto y si lleva punto.
#[derive(Clone, Copy, Debug)]
pub struct GroupItem<'a> {
    pub label: &'a str,
    pub dot: bool,
}

impl<'a> GroupItem<'a> {
    pub fn text(label: &'a str) -> Self {
        GroupItem { label, dot: false }
    }

    pub fn with_dot(label: &'a str) -> Self {
        GroupItem { label, dot: true }
    }
}

/// Cómo se ve un segmento: su estado (el de la interacción) y si es el elegido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentLook {
    pub state: ButtonState,
    pub on: bool,
}

impl SegmentLook {
    /// El estado que se dibuja: deshabilitado manda; si no, el elegido se ve presionado.
    pub fn shown(self) -> ButtonState {
        match self.state {
            ButtonState::Disabled => ButtonState::Disabled,
            _ if self.on => ButtonState::Pressed,
            state => state,
        }
    }
}

/// El ancho de un segmento sin las columnas de los costados (contorno o separador):
/// `1 + 4 + [6 + 3] + tinta + 4 + 1`.
pub fn segment_width(ui: &Ui, item: &GroupItem) -> f32 {
    let dot = if item.dot { DOT_W + DOT_GAP } else { 0.0 };
    2.0 * button::PAD_X + dot + button::ink_width(ui, item.label)
}

/// El ancho del grupo: los segmentos, un separador entre cada dos y el contorno a los costados.
pub fn width(ui: &Ui, items: &[GroupItem]) -> f32 {
    let segments: f32 = items.iter().map(|item| segment_width(ui, item)).sum();
    let seps = items.len().saturating_sub(1) as f32 * SEP_W;
    2.0 * button::OUTLINE + segments + seps
}

/// Los rects de los segmentos de un grupo que empieza en `at`, cada uno con las columnas de sus
/// costados (los vecinos comparten la del separador), de [`button::HEIGHT`] de alto.
pub fn segment_rects(ui: &Ui, items: &[GroupItem], at: Pos2) -> Vec<Rect> {
    let mut left = at.x;
    items
        .iter()
        .map(|item| {
            let w = segment_width(ui, item) + 2.0 * button::OUTLINE;
            let rect = Rect::from_min_size(pos2(left, at.y), vec2(w, button::HEIGHT));
            left += w - SEP_W;
            rect
        })
        .collect()
}

/// Un grupo con su ancho justo; `active(i)`: si la opción `i` es la elegida. Devuelve la opción
/// en la que se hizo clic. Con `ui` deshabilitado, todo el grupo se ve deshabilitado.
pub fn button_group(
    ui: &mut Ui,
    skin: &Skin,
    items: &[GroupItem],
    active: &dyn Fn(usize) -> bool,
) -> Option<usize> {
    let id = ui.next_auto_id();
    let size = vec2(width(ui, items), button::HEIGHT);
    let (slot, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter().clone();
    button_group_at(ui, skin, &painter, id, slot.min.round(), items, active)
}

/// [`button_group`] ya ubicado: la fila de arriba en `at`, con el id `id` y dibujado con
/// `painter` (la barra de opciones lo recorta a sus filas).
pub fn button_group_at(
    ui: &mut Ui,
    skin: &Skin,
    painter: &Painter,
    id: Id,
    at: Pos2,
    items: &[GroupItem],
    active: &dyn Fn(usize) -> bool,
) -> Option<usize> {
    let enabled = ui.is_enabled();
    let rects = segment_rects(ui, items, at);
    let mut clicked = None;
    let mut looks = Vec::with_capacity(items.len());
    let mut focus = None;
    for (i, (item, rect)) in items.iter().zip(&rects).enumerate() {
        let response: Response = ui.interact(*rect, id.with(("segment", i)), Sense::click());
        let on = active(i);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, on, item.label)
        });
        if response.clicked() {
            clicked = Some(i);
        }
        if response.has_focus() {
            focus = Some(*rect);
        }
        looks.push(SegmentLook {
            state: ButtonState::of(&response, enabled),
            on,
        });
    }
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = group_shapes(ui, skin, at, items, &looks);
    if let Some(rect) = focus {
        shapes.push(Shape::rect_stroke(
            rect.expand(1.0).round_to_pixels(ppp),
            0.0,
            Stroke::new(1.0, skin.colors().accent),
            StrokeKind::Outside,
        ));
    }
    // Deshabilitado se ve entero (sin la transparencia de egui), como el botón.
    let mut painter = painter.clone();
    painter.set_opacity(1.0);
    painter.extend(shapes);
    clicked
}

/// Un grupo que empieza en `at` dibujado con un `looks[i]` por segmento (sin interacción: el
/// catálogo los muestra así).
pub fn group_shapes(
    ui: &Ui,
    skin: &Skin,
    at: Pos2,
    items: &[GroupItem],
    looks: &[SegmentLook],
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let rects = segment_rects(ui, items, at);
    let shown: Vec<ButtonState> = looks.iter().map(|l| l.shown()).collect();
    let mut shapes = Vec::new();
    // Las caras: cada una es el botón de su estado estirado sobre el segmento con las columnas de
    // los costados; los separadores tapan las que se pisan.
    for (rect, state) in rects.iter().zip(&shown) {
        shapes.extend(button::frame_shapes(skin, *rect, *state, ppp));
    }
    for i in 1..rects.len() {
        let x = rects[i].left();
        // Si los dos de los costados bajaron, arriba no hay contorno.
        let sunk = shown[i - 1] == ButtonState::Pressed && shown[i] == ButtonState::Pressed;
        let disabled = shown[i] == ButtonState::Disabled;
        shapes.extend(sep_shapes(skin, pos2(x, at.y), sunk, disabled, ppp));
    }
    // Lo de adentro: el punto y el texto, que bajan con la cara.
    for ((item, rect), (look, state)) in items.iter().zip(&rects).zip(looks.iter().zip(&shown)) {
        let mut left = rect.left() + button::OUTLINE + button::PAD_X;
        if item.dot {
            let drop = if *state == ButtonState::Pressed {
                button::PRESS_DROP
            } else {
                0.0
            };
            let name = if look.on { DOT_ON } else { DOT_OFF };
            if let Some(piece) = skin.piece(name, WidgetState::Normal) {
                let dot =
                    Rect::from_min_size(pos2(left, at.y + button::CAPS_TOP + drop), size_of(piece));
                shapes.extend(pixel_ui::piece_shapes(piece, dot, ppp));
            }
            left += DOT_W + DOT_GAP;
        }
        shapes.extend(button::label_shapes(
            ui, skin, item.label, left, at.y, *state,
        ));
    }
    shapes
}

fn size_of(piece: &Piece) -> egui::Vec2 {
    vec2(piece.size[0] as f32, piece.size[1] as f32)
}

/// El separador en la columna `at.x` desde la fila `at.y`; `sunk`: sin la fila de arriba (los dos
/// vecinos bajaron). Sin la pieza, una línea del color del contorno.
fn sep_shapes(skin: &Skin, at: Pos2, sunk: bool, disabled: bool, ppp: f32) -> Vec<Shape> {
    let skip = if sunk { 1.0 } else { 0.0 };
    let state = if disabled {
        WidgetState::Disabled
    } else {
        WidgetState::Normal
    };
    let rect = Rect::from_min_size(pos2(at.x, at.y + skip), vec2(SEP_W, button::HEIGHT - skip));
    match skin.piece(SEP, state) {
        Some(piece) => {
            let h = piece.size[1] as f32;
            let screen = rect.round_to_pixels(ppp);
            let uv = Rect::from_min_max(pos2(0.0, skip / h), pos2(1.0, 1.0));
            vec![Shape::image(
                piece.texture.id(),
                screen,
                uv,
                egui::Color32::WHITE,
            )]
        }
        None => vec![Shape::rect_filled(
            rect.with_max_y(rect.bottom() - 1.0).round_to_pixels(ppp),
            0.0,
            skin.colors().outline,
        )],
    }
}

#[cfg(test)]
mod tests;
