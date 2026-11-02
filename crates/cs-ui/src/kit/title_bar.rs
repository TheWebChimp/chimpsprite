//! La barra de título de las ventanas (mockup de componentes, frame 9, x 480, y 236 y 268): 14
//! filas (`title.bar`: el borde (5,4,3) con las esquinas de arriba redondeadas, el brillo arriba y
//! a la derecha, el bisel a la izquierda, el relleno rosa y la fila oscura de abajo; se estira a lo
//! ancho).
//!
//! - A la izquierda, opcional, el ≡ (`icon.title_menu`, 6×5) desde la columna 4; el título 2 px
//!   después (sin ≡, desde la columna 4), con las mayúsculas en la fila 4; a la derecha, el ○
//!   (`icon.title_close`, 5×5) 4 px antes del borde, que con el puntero encima es la ×
//!   (`icon.title_close.hover`). Los íconos, en las filas 5 a 9.
//! - Con el foco, los íconos van en su tinta (100,54,75) y el título en `color.text`; sin el foco,
//!   los tres en el color de la fila oscura de la barra (178,82,102).
//! - Los marcos de las ventanas (`window`, `artboard.frame`) traen la barra de 14 filas; esta se
//!   dibuja encima con su contenido.

use std::sync::Arc;

use eframe::egui::{self, Color32, Galley, Painter, Pos2, Rect, Shape, Vec2, pos2, vec2};

use crate::pixel_ui;
use crate::skin::{Piece, Skin, SkinColors, WidgetState};
use crate::tab_bar;

pub const HEIGHT: f32 = 14.0;
pub const PIECE: &str = "title.bar";
pub const MENU: &str = "icon.title_menu";
pub const CLOSE: &str = "icon.title_close";
pub const CLOSE_HOVER: &str = "icon.title_close.hover";
/// El ≡ desde la columna 4; los íconos, desde la fila 5.
pub const MENU_LEFT: f32 = 4.0;
pub const ICON_TOP: f32 = 5.0;
/// Del ≡ a la tinta del título; sin ≡, el título empieza en la columna 4.
pub const TEXT_GAP: f32 = 2.0;
pub const TEXT_LEFT: f32 = 4.0;
/// Del ○ al borde derecho (afuera): 4 columnas.
pub const CLOSE_RIGHT: f32 = 4.0;
/// Del título al ○, por lo menos.
pub const CLOSE_GAP: f32 = 3.0;
pub const CAPS_TOP: f32 = 4.0;
/// Lo que se puede tocar alrededor de cada ícono: 9×9 desde la fila 3.
pub const HIT: f32 = 9.0;
const MENU_SIZE: Vec2 = vec2(6.0, 5.0);
const CLOSE_SIZE: Vec2 = vec2(5.0, 5.0);
/// Sin la pieza de la barra: la tinta sin el foco.
const DIM_FALLBACK: Color32 = Color32::from_rgb(178, 82, 102);

/// Las piezas y los colores de la barra (se arman del skin; las ventanas de los artboards los
/// guardan sin el skin).
#[derive(Clone)]
pub struct TitleLook {
    bar: Option<Piece>,
    menu: Option<Piece>,
    close: Option<Piece>,
    close_hover: Option<Piece>,
    colors: SkinColors,
}

impl TitleLook {
    pub fn new(skin: &Skin) -> Self {
        Self::with_bar(skin, PIECE)
    }

    /// Con otra pieza de barra (`title.bar.font`: la naranja de las fuentes); sin ella, la de
    /// siempre. La tinta sin el foco sale de la fila oscura de la barra que se use.
    pub fn with_bar(skin: &Skin, bar: &str) -> Self {
        let piece = |name| skin.piece(name, WidgetState::Normal).cloned();
        TitleLook {
            bar: piece(bar).or_else(|| piece(PIECE)),
            menu: piece(MENU),
            close: piece(CLOSE),
            close_hover: piece(CLOSE_HOVER),
            colors: *skin.colors(),
        }
    }

    /// La tinta de los íconos y del título: con el foco, la de cada ícono (o `color.text` el
    /// título); sin él, la de la fila oscura de la barra.
    fn dim(&self) -> Color32 {
        self.bar
            .as_ref()
            .map_or(DIM_FALLBACK, |p| p.pixel(p.size[0] / 2, p.size[1] - 1))
    }

    pub fn text_color(&self, focused: bool) -> Color32 {
        if focused {
            self.colors.text
        } else {
            self.dim()
        }
    }
}

/// Dónde va cada cosa de una barra en `rect`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TitleParts {
    /// Lo que se toca del ≡ y del ○ (9×9 alrededor de cada uno).
    pub menu: Option<Rect>,
    pub close: Option<Rect>,
    /// De dónde a dónde puede ir la tinta del título.
    pub text: egui::Rangef,
}

/// Las partes de una barra en `rect` con o sin ≡ y ○.
pub fn layout(rect: Rect, menu: bool, close: bool) -> TitleParts {
    // 9×9 desde la fila 3, con el ícono centrado a lo ancho (el píxel de sobra, a la derecha).
    let hit = |icon: Rect| {
        Rect::from_min_size(
            pos2(
                icon.left() - ((HIT - icon.width()) / 2.0).floor(),
                icon.top() - 2.0,
            ),
            Vec2::splat(HIT),
        )
    };
    let menu_icon = Rect::from_min_size(
        pos2(rect.left() + MENU_LEFT, rect.top() + ICON_TOP),
        MENU_SIZE,
    );
    let close_icon = Rect::from_min_size(
        pos2(
            rect.right() - CLOSE_RIGHT - CLOSE_SIZE.x,
            rect.top() + ICON_TOP,
        ),
        CLOSE_SIZE,
    );
    let left = if menu {
        menu_icon.right() + TEXT_GAP
    } else {
        rect.left() + TEXT_LEFT
    };
    let right = if close {
        close_icon.left() - CLOSE_GAP
    } else {
        rect.right() - CLOSE_RIGHT
    };
    TitleParts {
        menu: menu.then(|| hit(menu_icon)),
        close: close.then(|| hit(close_icon)),
        text: egui::Rangef::new(left, right.max(left)),
    }
}

/// Los píxeles de un ícono de una tinta desde `at`, en `color`.
fn dots(piece: &Piece, at: Pos2, color: Color32) -> Vec<Shape> {
    let [w, h] = piece.size;
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| piece.rgba[((y * w + x) * 4 + 3) as usize] > 0)
        .map(|(x, y)| {
            Shape::rect_filled(
                Rect::from_min_size(at + vec2(x as f32, y as f32), Vec2::splat(1.0)),
                0.0,
                color,
            )
        })
        .collect()
}

/// Lo que dibuja la barra: `Some(hovered)` en `close` pone el ○ (la × con el puntero encima).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TitleState {
    pub focused: bool,
    pub menu: bool,
    pub close: Option<bool>,
}

/// La barra en `rect` (de [`HEIGHT`] de alto) con `title` y lo que diga `state`. `painter` arma el
/// texto. Un título que no entra se corta con "...".
pub fn bar_shapes(
    painter: &Painter,
    look: &TitleLook,
    rect: Rect,
    title: &str,
    state: TitleState,
    ppp: f32,
) -> Vec<Shape> {
    let mut shapes = match &look.bar {
        Some(p) => pixel_ui::piece_shapes(p, rect.with_max_y(rect.top() + HEIGHT), ppp),
        None => vec![Shape::rect_filled(rect, 0.0, look.colors.selection)],
    };
    let parts = layout(rect, state.menu, state.close.is_some());
    let ink = |piece: &Piece| {
        if state.focused {
            piece.pixel(first_ink(piece).0, first_ink(piece).1)
        } else {
            look.dim()
        }
    };
    let icon_at = |hit: Rect, size: Vec2| {
        pos2(
            hit.left() + ((hit.width() - size.x) / 2.0).floor(),
            rect.top() + ICON_TOP,
        )
    };
    if let (Some(hit), Some(piece)) = (parts.menu, &look.menu) {
        shapes.extend(dots(piece, icon_at(hit, MENU_SIZE), ink(piece)));
    }
    if let (Some(hit), Some(hovered)) = (parts.close, state.close) {
        let piece = if hovered {
            look.close_hover.as_ref().or(look.close.as_ref())
        } else {
            look.close.as_ref()
        };
        if let Some(piece) = piece {
            shapes.extend(dots(piece, icon_at(hit, CLOSE_SIZE), ink(piece)));
        }
    }
    let color = look.text_color(state.focused);
    if !title.is_empty()
        && let Some(galley) = fit(painter, title, parts.text.span(), color)
    {
        let at = tab_bar::text_pos(&galley, parts.text.min, rect.top() + CAPS_TOP, ppp);
        shapes.push(Shape::galley(at, galley, color));
    }
    shapes
}

/// El primer píxel con tinta de `piece` (su color es la tinta del ícono).
fn first_ink(piece: &Piece) -> (u32, u32) {
    let [w, h] = piece.size;
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .find(|&(x, y)| piece.rgba[((y * w + x) * 4 + 3) as usize] > 0)
        .unwrap_or((0, 0))
}

/// `title` cortado con "..." si su tinta no entra en `width`.
fn fit(painter: &Painter, title: &str, width: f32, color: Color32) -> Option<Arc<Galley>> {
    crate::artboard_window::fit_title(painter, title, width, color)
}

#[cfg(test)]
mod tests;
