//! Con Pixel UI, cada artboard del lienzo se ve como la ventana del mockup
//! (`.design/Design.aseprite`, frame 4, en (209,235)–(381,357)): barra de título rosa con "≡" a
//! la izquierda y "○" a la derecha, borde oscuro de 1 px con las esquinas redondeadas y cuerpo
//! claro.
//!
//! - **Piezas:** el marco es `artboard.frame` en 9-slice y la barra es la del UI kit
//!   (`kit::title_bar`: ≡, el nombre y ○; sin el foco, en el color tenue de la barra).
//! - **`artboard.frame`** sale de `window` (el marco de los diálogos pixel), con el mismo borde
//!   oscuro y las mismas esquinas redondeadas, pero sin el bisel ni el cuerpo: adentro del borde
//!   hay un anillo blanco de 1 px y el resto es transparente. Su barra mide 14 filas, como la del
//!   kit, que se dibuja encima.
//! - **Cuerpo = área de píxeles:** el damero y los píxeles del artboard quedan donde siempre, al
//!   mismo zoom; el marco se dibuja alrededor, a escala de la UI (no cambia con el zoom). Del
//!   borde de la ventana al área de píxeles hay borde (1) + blanco (1) a los lados y abajo, y
//!   barra (14) + blanco (1) arriba.
//! - **Pixel perfect:** los píxeles se dibujan en su rect redondeado a píxeles de pantalla
//!   ([`WindowLayout::pixels_on_screen`]) y el marco se arma alrededor de ese mismo rect. Con un
//!   pan o un zoom fraccionarios, el blanco mide siempre 1 px de la UI de cada lado.
//! - **Barra angosta:** primero se pierde el nombre y después los íconos; la barra queda siempre.

use std::sync::Arc;

use eframe::egui::{Color32, FontId, Painter, Pos2, Rect, Shape, Vec2, pos2, vec2};
use eframe::emath::GuiRounding as _;

use crate::kit::title_bar::{self, TitleLook, TitleState};
use crate::pixel_ui;
use crate::skin::{Piece, Skin, SkinColors, WidgetState};

/// Del borde de la ventana al área de píxeles (izquierda, arriba, derecha, abajo), en puntos:
/// borde + blanco a los lados y abajo; barra + blanco arriba.
pub const INSETS: [f32; 4] = [
    BORDER + PADDING,
    TITLE_BAR + PADDING,
    BORDER + PADDING,
    BORDER + PADDING,
];
/// Borde oscuro de la ventana.
const BORDER: f32 = 1.0;
/// Blanco entre el borde (o la barra) y los píxeles.
const PADDING: f32 = 1.0;
/// Alto de la barra de título: la del UI kit (borde, brillo, 11 filas rosas y la línea oscura).
pub const TITLE_BAR: f32 = title_bar::HEIGHT;
/// La barra lleva los íconos si entran los dos con su aire.
const ICONS_MIN_WIDTH: f32 = 2.0 * (title_bar::HIT + 2.0);
/// Lo que se agrega al final de un título que no entra.
const ELLIPSIS: &str = "...";

const FRAME_PIECE: &str = "artboard.frame";

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
    /// La ventana alrededor del área de píxeles `pixels` (en pantalla), tal como se dibuja
    /// (ver [`Self::pixels_on_screen`]).
    pub fn around(pixels: Rect, pixels_per_point: f32) -> WindowLayout {
        let frame = Self::frame_around(pixels, pixels_per_point);
        let bar = Rect::from_min_size(frame.min, vec2(frame.width(), TITLE_BAR));
        // Los dos íconos (con su aire) si entran; el nombre, entre ellos si queda lugar.
        let icons = frame.width() >= ICONS_MIN_WIDTH;
        let parts = title_bar::layout(bar, icons, icons);
        let (menu, close) = (parts.menu, parts.close);
        let text = (icons && parts.text.span() > 0.0)
            .then(|| Rect::from_x_y_ranges(parts.text, bar.y_range()));
        WindowLayout {
            frame,
            bar,
            menu,
            close,
            text,
        }
    }

    /// Dónde se dibujan de verdad los píxeles del área `pixels`: redondeada a píxeles de
    /// pantalla. El lienzo dibuja el damero y la textura en este rect (si quedaran en una
    /// fracción, la GPU los cortaría en el píxel más cercano y el marco no sabría dónde).
    pub fn pixels_on_screen(pixels: Rect, pixels_per_point: f32) -> Rect {
        pixels.round_to_pixels(pixels_per_point)
    }

    /// Solo el marco de `around`, sin el resto: alcanza para descartar los artboards fuera de
    /// la vista o para saber cuál está arriba bajo el puntero.
    ///
    /// Va pegado al rect de [`Self::pixels_on_screen`]: con la escala entera de Pixel UI, `INSETS`
    /// son píxeles de pantalla enteros y el marco cae justo en píxeles.
    pub fn frame_around(pixels: Rect, pixels_per_point: f32) -> Rect {
        let [left, top, right, bottom] = INSETS;
        let p = Self::pixels_on_screen(pixels, pixels_per_point);
        Rect::from_min_max(
            pos2(p.min.x - left, p.min.y - top),
            pos2(p.max.x + right, p.max.y + bottom),
        )
    }

    /// Qué parte de la barra está bajo `p`, si está en la barra. Abajo es semiabierta: la fila
    /// `y == bar.bottom()` ya es la blanca de debajo de la barra.
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
    title: TitleLook,
    colors: SkinColors,
}

impl ArtboardWindow {
    pub fn from_skin(skin: &Skin) -> Self {
        let piece = |name| skin.piece(name, WidgetState::Normal).cloned();
        Self {
            frame: piece(FRAME_PIECE),
            title: TitleLook::new(skin),
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

    /// El marco, que va detrás del damero y los píxeles (el centro del 9-slice es transparente
    /// y queda tapado).
    pub fn frame_shapes(&self, layout: &WindowLayout, active: bool, ppp: f32) -> Vec<Shape> {
        let shapes = match &self.frame {
            Some(piece) => pixel_ui::piece_shapes(piece, layout.frame, ppp),
            None => {
                // Respaldo plano, también con el anillo blanco adentro del borde.
                let mut shapes = pixel_ui::flat_frame(&self.colors, layout.frame, TITLE_BAR, ppp);
                let white = Rect::from_min_max(
                    pos2(layout.frame.left() + BORDER, layout.bar.bottom()),
                    layout.frame.max - Vec2::splat(BORDER),
                );
                shapes.push(Shape::rect_filled(white, 0.0, Color32::WHITE));
                shapes
            }
        };
        tinted(shapes, self.tint(active))
    }

    /// La barra del UI kit con el ≡, el nombre `title` y el ○, encima del marco; sin el foco
    /// (`active`), en el color tenue de la barra.
    pub fn bar_shapes(
        &self,
        painter: &Painter,
        layout: &WindowLayout,
        title: &str,
        active: bool,
        ppp: f32,
    ) -> Vec<Shape> {
        let icons = layout.menu.is_some();
        let state = TitleState {
            focused: active,
            menu: icons,
            // El "○" es decorativo por ahora: qué hace lo va a definir el usuario.
            close: icons.then_some(false),
        };
        let title = if layout.text.is_some() { title } else { "" };
        title_bar::bar_shapes(painter, &self.title, layout.bar, title, state, ppp)
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
pub(crate) fn tinted(shapes: Vec<Shape>, tint: Color32) -> Vec<Shape> {
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
    fn the_window_wraps_the_pixels_with_a_14_point_title_bar_and_a_white_point() {
        let l = WindowLayout::around(pixels(128.0, 64.0), 1.0);
        // Borde (1) + blanco (1) a los lados y abajo; barra (14) + blanco (1) arriba.
        assert_eq!(
            l.frame,
            Rect::from_min_max(pos2(98.0, 185.0), pos2(230.0, 266.0))
        );
        assert_eq!(
            l.bar,
            Rect::from_min_max(pos2(98.0, 185.0), pos2(230.0, 199.0))
        );
        // Lo que se toca de los íconos de la barra del kit: 9×9 desde la fila 3, alrededor del
        // ≡ (columna 4) y del ○ (4 antes del borde).
        assert_eq!(
            l.menu,
            Some(Rect::from_min_size(pos2(101.0, 188.0), Vec2::splat(9.0)))
        );
        assert_eq!(
            l.close,
            Some(Rect::from_min_size(pos2(219.0, 188.0), Vec2::splat(9.0)))
        );
        // El nombre, 2 px después del ≡ y hasta 3 antes del ○.
        assert_eq!(l.text.unwrap().x_range(), 110.0..=218.0);
        assert_eq!(l.hit(pos2(104.0, 193.0)), Some(TitleHit::Menu));
        assert_eq!(l.hit(pos2(223.0, 193.0)), Some(TitleHit::Close));
        assert_eq!(l.hit(pos2(150.0, 193.0)), Some(TitleHit::Bar));
        assert_eq!(
            l.hit(pos2(150.0, 201.0)),
            None,
            "the pixels are not the bar"
        );
        // Semiabierta: la fila blanca de debajo de la barra ya no es la barra.
        assert_eq!(l.hit(pos2(150.0, 198.5)), Some(TitleHit::Bar));
        assert_eq!(l.hit(pos2(150.0, 199.0)), None, "the white row");
        assert_eq!(l.hit(pos2(150.0, 200.0)), None, "the first row of pixels");
    }

    #[test]
    fn the_frame_hugs_the_drawn_pixels_with_one_white_point_at_any_zoom_and_pan() {
        // Los píxeles se dibujan en su rect redondeado a píxeles físicos (como el lienzo); el
        // marco tiene que quedar a borde (1) + blanco (1) de ese rect, sin importar la fracción.
        for ppp in [1.0, 2.0] {
            for zoom in [1.0, 8.0] {
                for pan in [
                    vec2(0.0, 0.0),
                    vec2(0.37, 0.81),
                    vec2(0.5, 0.25),
                    vec2(0.73, 0.5),
                ] {
                    let pixels = Rect::from_min_size(
                        pos2(100.0, 200.0) + pan,
                        vec2(16.0 * zoom, 16.0 * zoom),
                    );
                    let drawn = WindowLayout::pixels_on_screen(pixels, ppp);
                    assert_eq!(drawn, pixels.round_to_pixels(ppp));
                    let frame = WindowLayout::frame_around(pixels, ppp);
                    let at = format!("ppp {ppp}, zoom {zoom}, pan {pan:?}");
                    assert_eq!(frame.left() + 1.0 + 1.0, drawn.left(), "left, {at}");
                    assert_eq!(frame.right() - 1.0 - 1.0, drawn.right(), "right, {at}");
                    assert_eq!(frame.bottom() - 1.0 - 1.0, drawn.bottom(), "bottom, {at}");
                    assert_eq!(frame.top() + TITLE_BAR + 1.0, drawn.top(), "top, {at}");
                    // El marco cae en píxeles físicos enteros.
                    for v in [frame.left(), frame.top(), frame.right(), frame.bottom()] {
                        assert_eq!((v * ppp).fract(), 0.0, "{v}, {at}");
                    }
                }
            }
        }
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
        assert_eq!(l.frame.height(), TITLE_BAR + 1.0 + 1.0 + 1.0 + 1.0);
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
