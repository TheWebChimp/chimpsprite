//! Con Pixel UI, cada artboard del lienzo se ve como la ventana del mockup
//! (`.design/Design.aseprite`, frame 4, en (209,235)–(381,357)): barra de título rosa con "≡" a
//! la izquierda y "○" a la derecha, borde oscuro de 1 px con las esquinas redondeadas y cuerpo
//! claro.
//!
//! - **Piezas:** el marco es `window` en 9-slice (el mismo de los diálogos pixel), el "○" es
//!   `window.close` y el "≡" es `artboard.menu` (el `window.menu` del skin tiene las rayas de 7 px;
//!   las del mockup miden 6). Ventana del mockup = las tres piezas, sin un píxel de diferencia.
//! - **Cuerpo = área de píxeles:** el damero y los píxeles del artboard quedan donde siempre, al
//!   mismo zoom; el marco se dibuja alrededor, a escala de la UI (no cambia con el zoom). Del
//!   borde de la ventana al área de píxeles hay 2 px a los lados (borde y bisel), 12 arriba (la
//!   barra de título) y 1 abajo (el borde). Las dos esquinas del bisel en la fila de abajo quedan
//!   bajo los píxeles.
//! - **Barra angosta:** primero se pierde el nombre y después los íconos; la barra queda siempre.

use std::sync::Arc;

use eframe::egui::{Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
use eframe::emath::GuiRounding as _;

use crate::pixel_ui;
use crate::skin::{Piece, Skin, SkinColors, WidgetState};

/// Del borde de la ventana al área de píxeles (izquierda, arriba, derecha, abajo), en puntos.
pub const INSETS: [f32; 4] = [2.0, TITLE_BAR, 2.0, 1.0];
/// Alto de la barra de título: borde, brillo, 9 filas rosas y la línea oscura de abajo.
pub const TITLE_BAR: f32 = 12.0;
/// Franja donde se centra el título: la de arriba del 9-slice, como en `pixel_dialog`.
const TITLE_BAND: f32 = 13.0;
/// Lado de los íconos de la barra (`artboard.menu`, `window.close`) y su margen al borde.
const ICON: f32 = 9.0;
const ICON_INSET: f32 = 2.0;
/// Aire entre un ícono y el título.
const TEXT_GAP: f32 = 2.0;
/// Lo que se agrega al final de un título que no entra.
const ELLIPSIS: &str = "...";

const FRAME_PIECE: &str = "window";
const MENU_PIECE: &str = "artboard.menu";
const CLOSE_PIECE: &str = "window.close";

/// Dónde va cada parte de la ventana de un artboard, en pantalla.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowLayout {
    /// Toda la ventana: el área de píxeles más `INSETS`.
    pub frame: Rect,
    /// La barra de título, a todo el ancho de la ventana.
    pub bar: Rect,
    /// "≡" y "○"; `None` si la ventana es demasiado angosta.
    pub menu: Option<Rect>,
    pub close: Option<Rect>,
    /// Espacio para el título, entre los íconos; `None` si no hay íconos.
    pub text: Option<Rect>,
}

/// Parte de la barra de título bajo el puntero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleHit {
    /// La barra: clic selecciona, arrastre mueve, doble clic renombra.
    Bar,
    /// "≡": selecciona el artboard (no hay menú contextual de artboards).
    Menu,
    /// "○": decorativo por ahora.
    Close,
}

impl WindowLayout {
    /// La ventana alrededor del área de píxeles `pixels` (en pantalla), alineada a píxeles de
    /// pantalla hacia afuera para no dejar huecos entre el marco y los píxeles.
    pub fn around(pixels: Rect, pixels_per_point: f32) -> WindowLayout {
        let frame = Self::frame_around(pixels, pixels_per_point);
        let bar = Rect::from_min_size(frame.min, vec2(frame.width(), TITLE_BAR));
        // Los dos íconos con su margen a cada lado.
        let icons = frame.width() >= 2.0 * (ICON_INSET + ICON);
        let menu = icons
            .then(|| Rect::from_min_size(frame.min + Vec2::splat(ICON_INSET), Vec2::splat(ICON)));
        let close = icons.then(|| {
            Rect::from_min_size(
                pos2(frame.max.x - ICON_INSET - ICON, frame.min.y + ICON_INSET),
                Vec2::splat(ICON),
            )
        });
        let text = match (menu, close) {
            (Some(m), Some(c)) if c.left() - TEXT_GAP > m.right() + TEXT_GAP => Some(
                Rect::from_x_y_ranges(m.right() + TEXT_GAP..=c.left() - TEXT_GAP, bar.y_range()),
            ),
            _ => None,
        };
        WindowLayout {
            frame,
            bar,
            menu,
            close,
            text,
        }
    }

    /// Solo el marco de `around`, sin el resto: alcanza para descartar los artboards fuera de
    /// la vista o para saber cuál está arriba bajo el puntero.
    pub fn frame_around(pixels: Rect, pixels_per_point: f32) -> Rect {
        let ppp = pixels_per_point;
        let [left, top, right, bottom] = INSETS;
        let min = (pixels.min.to_vec2() * ppp).floor() / ppp;
        let max = (pixels.max.to_vec2() * ppp).ceil() / ppp;
        Rect::from_min_max(
            pos2(min.x - left, min.y - top),
            pos2(max.x + right, max.y + bottom),
        )
    }

    /// Qué parte de la barra está bajo `p`, si está en la barra. Abajo es semiabierta: la fila
    /// `y == bar.bottom()` ya es del área de píxeles.
    pub fn hit(&self, p: Pos2) -> Option<TitleHit> {
        if !self.bar.contains(p) || p.y >= self.bar.bottom() {
            return None;
        }
        Some(if self.menu.is_some_and(|r| r.contains(p)) {
            TitleHit::Menu
        } else if self.close.is_some_and(|r| r.contains(p)) {
            TitleHit::Close
        } else {
            TitleHit::Bar
        })
    }
}

/// Las piezas y los colores de la ventana de los artboards. La app lo arma del skin en cada
/// frame (como las reglas pixel); `None` en el lienzo: sin Pixel UI.
#[derive(Clone)]
pub struct ArtboardWindow {
    frame: Option<Piece>,
    menu: Option<Piece>,
    close: Option<Piece>,
    colors: SkinColors,
}

impl ArtboardWindow {
    pub fn from_skin(skin: &Skin) -> Self {
        let piece = |name| skin.piece(name, WidgetState::Normal).cloned();
        Self {
            frame: piece(FRAME_PIECE),
            menu: piece(MENU_PIECE),
            close: piece(CLOSE_PIECE),
            colors: *skin.colors(),
        }
    }

    /// Fuente del título: la proporcional, que con Pixel UI es la fuente pixel.
    pub fn font() -> FontId {
        FontId::proportional(crate::appearance::PIXEL_FONT_SIZE)
    }

    /// Tinte del marco: blanco (sin cambio) en el activo; los inactivos, un poco apagados hacia
    /// `text_dim`.
    fn tint(&self, active: bool) -> Color32 {
        if active {
            Color32::WHITE
        } else {
            Color32::WHITE.lerp_to_gamma(self.colors.text_dim, 0.3)
        }
    }

    /// El marco, que va detrás del damero y los píxeles (el centro del 9-slice queda tapado).
    pub fn frame_shapes(&self, layout: &WindowLayout, active: bool, ppp: f32) -> Vec<Shape> {
        let shapes = match &self.frame {
            Some(piece) => pixel_ui::piece_shapes(piece, layout.frame, ppp),
            None => pixel_ui::flat_frame(&self.colors, layout.frame, TITLE_BAR, ppp),
        };
        tinted(shapes, self.tint(active))
    }

    /// Los íconos y el título `title`, que van encima de la barra.
    pub fn bar_shapes(
        &self,
        painter: &Painter,
        layout: &WindowLayout,
        title: &str,
        active: bool,
        ppp: f32,
    ) -> Vec<Shape> {
        let mut shapes = Vec::new();
        let ink = self.colors.text;
        if let Some(r) = layout.menu {
            shapes.extend(match &self.menu {
                Some(piece) => pixel_ui::piece_shapes(piece, r, ppp),
                // Respaldo: tres rayas de 6 px, como las del mockup.
                None => (0..3)
                    .map(|i| {
                        let row = Rect::from_min_size(
                            r.min + vec2(2.0, 2.0 + 2.0 * i as f32),
                            vec2(6.0, 1.0),
                        );
                        Shape::rect_filled(row.round_to_pixels(ppp), 0.0, ink)
                    })
                    .collect(),
            });
        }
        if let Some(r) = layout.close {
            // "○" decorativo por ahora: qué hace lo va a definir el usuario.
            shapes.extend(match &self.close {
                Some(piece) => pixel_ui::piece_shapes(piece, r, ppp),
                None => vec![Shape::circle_stroke(r.center(), 2.5, Stroke::new(1.0, ink))],
            });
        }
        let mut shapes = tinted(shapes, self.tint(active));
        let color = if active {
            self.colors.text
        } else {
            self.colors.text_dim
        };
        if let Some(area) = layout.text
            && !title.is_empty()
            && let Some(galley) = fit_title(painter, title, area.width(), color)
        {
            let band = Rect::from_min_size(layout.bar.min, vec2(layout.bar.width(), TITLE_BAND));
            let y = band.center().y - galley.size().y / 2.0;
            let at = pos2(area.left(), y).round_to_pixels(ppp);
            shapes.push(Shape::galley(at, galley, color));
        }
        shapes
    }
}

/// `title` en la fuente del título, cortado con "..." si no entra en `width`; `None` si ni los
/// puntos entran.
pub fn fit_title(
    painter: &Painter,
    title: &str,
    width: f32,
    color: Color32,
) -> Option<Arc<eframe::egui::Galley>> {
    let layout = |text: String| painter.layout_no_wrap(text, ArtboardWindow::font(), color);
    let full = layout(title.to_owned());
    if full.size().x <= width {
        return Some(full);
    }
    let chars: Vec<char> = title.chars().collect();
    let cut = |n: usize| layout(chars[..n].iter().collect::<String>() + ELLIPSIS);
    // El prefijo más largo que entra con los puntos (búsqueda binaria: el ancho crece con n).
    let (mut lo, mut hi) = (0, chars.len());
    if cut(0).size().x > width {
        return None;
    }
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if cut(mid).size().x <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some(cut(lo))
}

/// Las mallas de `shapes` multiplicadas por `tint`.
fn tinted(shapes: Vec<Shape>, tint: Color32) -> Vec<Shape> {
    if tint == Color32::WHITE {
        return shapes;
    }
    shapes
        .into_iter()
        .map(|shape| match shape {
            Shape::Mesh(mut mesh) => {
                for v in &mut Arc::make_mut(&mut mesh).vertices {
                    v.color = v.color * tint;
                }
                Shape::Mesh(mesh)
            }
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(100.0, 200.0), vec2(w, h))
    }

    #[test]
    fn the_window_wraps_the_pixels_with_a_12_point_title_bar_on_top() {
        let l = WindowLayout::around(pixels(128.0, 64.0), 1.0);
        assert_eq!(
            l.frame,
            Rect::from_min_max(pos2(98.0, 188.0), pos2(230.0, 265.0))
        );
        assert_eq!(
            l.bar,
            Rect::from_min_max(pos2(98.0, 188.0), pos2(230.0, 200.0))
        );
        assert_eq!(l.bar.bottom(), 200.0, "the bar ends where the pixels start");
        // Los íconos como en el mockup: a 2 px de la esquina de la ventana.
        assert_eq!(
            l.menu,
            Some(Rect::from_min_size(pos2(100.0, 190.0), Vec2::splat(9.0)))
        );
        assert_eq!(
            l.close,
            Some(Rect::from_min_size(pos2(219.0, 190.0), Vec2::splat(9.0)))
        );
        assert_eq!(l.text.unwrap().x_range(), 111.0..=217.0);
        assert_eq!(l.hit(pos2(104.0, 194.0)), Some(TitleHit::Menu));
        assert_eq!(l.hit(pos2(223.0, 194.0)), Some(TitleHit::Close));
        assert_eq!(l.hit(pos2(150.0, 194.0)), Some(TitleHit::Bar));
        assert_eq!(
            l.hit(pos2(150.0, 201.0)),
            None,
            "the pixels are not the bar"
        );
        // Semiabierta: la fila de arriba del área de píxeles es de los píxeles.
        assert_eq!(l.hit(pos2(150.0, 199.5)), Some(TitleHit::Bar));
        assert_eq!(l.hit(pos2(150.0, 200.0)), None, "the boundary row");
    }

    #[test]
    fn a_narrow_window_drops_the_title_then_the_icons() {
        // 22 de ancho: los dos íconos pegados, sin lugar para el título.
        let l = WindowLayout::around(pixels(18.0, 4.0), 1.0);
        assert!(l.menu.is_some() && l.close.is_some() && l.text.is_none());
        let l = WindowLayout::around(pixels(17.0, 4.0), 1.0);
        assert!(l.menu.is_none() && l.close.is_none() && l.text.is_none());
        assert_eq!(l.hit(l.bar.center()), Some(TitleHit::Bar));
        // Medio píxel de artboard (zoom 0.5): la barra queda, de 5 de ancho.
        let l = WindowLayout::around(pixels(0.5, 0.5), 1.0);
        assert_eq!(l.frame.width(), 5.0);
        assert_eq!(l.bar.height(), TITLE_BAR);
    }

    #[test]
    fn long_titles_are_cut_with_dots() {
        let ctx = eframe::egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let painter = ui.painter();
            let full = fit_title(painter, "Artboard 1  64×64", 1000.0, Color32::BLACK).unwrap();
            assert_eq!(full.text(), "Artboard 1  64×64");
            let cut = fit_title(painter, "Artboard 1  64×64", 40.0, Color32::BLACK).unwrap();
            assert!(cut.text().ends_with("..."), "{}", cut.text());
            assert!(cut.size().x <= 40.0);
            assert!(fit_title(painter, "Artboard", 1.0, Color32::BLACK).is_none());
        });
        output.textures_delta.clear();
    }
}
