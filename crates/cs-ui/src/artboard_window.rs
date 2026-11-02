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
//! - **Fuentes:** los artboards de fuente llevan la barra naranja (`title.bar.font`; sin ella, la
//!   rosa). De lejos, cuando la ventana del glifo quedaría más chica que la tarjeta en los dos
//!   lados, se dibuja la tarjeta ([`WindowLayout::card`], referencia `ff_zoom.png`): la barra con
//!   el nombre de la fuente y un cuerpo blanco con "Abcde 12345" escrito con la fuente, a 1 punto
//!   por píxel, a 5 del anillo gris de cada lado.

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
/// La barra de los artboards de fuente: la del kit en naranja.
const FONT_BAR_PIECE: &str = "title.bar.font";

/// Lo que se escribe en la tarjeta de una fuente.
pub const CARD_SAMPLE: &str = "Abcde\n12345";
/// Del anillo gris de la tarjeta a la muestra, de cada lado (4 de blanco).
const CARD_PAD: f32 = 5.0;
/// El anillo de adentro del borde de la tarjeta (el blanco de `artboard.frame` teñido).
const CARD_RING: Color32 = Color32::from_rgb(212, 209, 210);
/// La tinta de las fuentes de 1 bit en la tarjeta.
const CARD_INK: Color32 = Color32::from_rgb(5, 4, 3);

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
    /// En la tarjeta de una fuente vista de lejos, dónde va la muestra; `None` en las ventanas
    /// de siempre.
    pub card: Option<Rect>,
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
        Self::with_frame(Self::frame_around(pixels, pixels_per_point), None)
    }

    /// La tarjeta de una fuente desde `min` (la esquina de la ventana del glifo) con una muestra
    /// de `sample` puntos: la barra, el anillo y 5 puntos del anillo a la muestra de cada lado.
    pub fn card(min: Pos2, sample: Vec2) -> WindowLayout {
        let inset = BORDER + CARD_PAD;
        let size = vec2(
            sample.x + 2.0 * inset,
            TITLE_BAR + CARD_PAD + sample.y + inset,
        );
        let text = Rect::from_min_size(min + vec2(inset, TITLE_BAR + CARD_PAD), sample);
        Self::with_frame(Rect::from_min_size(min, size), Some(text))
    }

    /// La barra y sus partes de una ventana que ocupa `frame`.
    fn with_frame(frame: Rect, card: Option<Rect>) -> WindowLayout {
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
            card,
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
    /// `y == bar.bottom()` ya es la blanca de debajo de la barra. En la tarjeta, todo es barra
    /// (no hay píxeles que pintar: un clic la activa y un arrastre la mueve).
    pub fn hit(&self, p: Pos2) -> Option<TitleHit> {
        let area = if self.card.is_some() {
            self.frame
        } else {
            self.bar
        };
        if !area.contains(p) || p.y >= area.bottom() {
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
    /// La barra naranja de los artboards de fuente.
    font_title: TitleLook,
    colors: SkinColors,
}

/// La muestra de la tarjeta de una fuente: "Abcde 12345" escrito con ella, a 1 punto por píxel.
#[derive(Clone, Debug, PartialEq)]
pub struct CardSample {
    /// Lo que ocupa: la caja de la tinta o, si la fuente no tiene nada dibujado, la de las
    /// celdas y los anchos.
    pub size: Vec2,
    /// Cada píxel desde la esquina de `size`, con su color.
    pixels: Vec<(i32, i32, Color32)>,
}

impl CardSample {
    pub fn new(view: &cs_core::PixelFontView) -> CardSample {
        let text = cs_tools::text::font_text(view, CARD_SAMPLE, 1);
        let xs = || text.iter().map(|&(x, _, _)| x);
        let ys = || text.iter().map(|&(_, y, _)| y);
        let (Some(x0), Some(x1), Some(y0), Some(y1)) =
            (xs().min(), xs().max(), ys().min(), ys().max())
        else {
            // Sin tinta: el renglón más ancho por las líneas de la celda.
            let width = |line: &str| -> u32 {
                line.chars()
                    .map(|ch| match ch {
                        ' ' => view.space_width,
                        _ => view
                            .glyphs
                            .get(&ch)
                            .map_or(view.missing_width, |g| g.advance),
                    })
                    .sum()
            };
            let w = CARD_SAMPLE.lines().map(width).max().unwrap_or(0);
            let h = CARD_SAMPLE.lines().count() as u32 * view.cell.1;
            return CardSample {
                size: vec2(w as f32, h as f32),
                pixels: Vec::new(),
            };
        };
        let pixels = text
            .into_iter()
            .map(|(x, y, c)| {
                let color = c.map_or(CARD_INK, |c| {
                    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
                });
                (x - x0, y - y0, color)
            })
            .collect();
        CardSample {
            size: vec2((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32),
            pixels,
        }
    }
}

impl ArtboardWindow {
    pub fn from_skin(skin: &Skin) -> Self {
        let piece = |name| skin.piece(name, WidgetState::Normal).cloned();
        Self {
            frame: piece(FRAME_PIECE),
            title: TitleLook::new(skin),
            font_title: TitleLook::with_bar(skin, FONT_BAR_PIECE),
            colors: *skin.colors(),
        }
    }

    /// La tarjeta de una fuente (`layout` de [`WindowLayout::card`]): el marco de los artboards
    /// con el anillo gris, el cuerpo blanco y la muestra. La barra va aparte, encima.
    pub fn card_shapes(
        &self,
        layout: &WindowLayout,
        sample: &CardSample,
        active: bool,
        ppp: f32,
    ) -> Vec<Shape> {
        let frame = layout.frame;
        // El anillo blanco del marco, teñido de gris (el borde casi negro casi no cambia).
        let mut shapes = match &self.frame {
            Some(piece) => tinted(pixel_ui::piece_shapes(piece, frame, ppp), CARD_RING),
            None => {
                let mut shapes = pixel_ui::flat_frame(&self.colors, frame, TITLE_BAR, ppp);
                let ring = Rect::from_min_max(
                    pos2(frame.left() + BORDER, layout.bar.bottom()),
                    frame.max - Vec2::splat(BORDER),
                );
                shapes.push(Shape::rect_filled(ring, 0.0, CARD_RING));
                shapes
            }
        };
        let body = Rect::from_min_max(
            pos2(
                frame.left() + BORDER + PADDING,
                layout.bar.bottom() + PADDING,
            ),
            frame.max - Vec2::splat(BORDER + PADDING),
        );
        shapes.push(Shape::rect_filled(body, 0.0, Color32::WHITE));
        let mut shapes = tinted(shapes, self.tint(active));
        if let Some(text) = layout.card {
            shapes.extend(sample.pixels.iter().map(|&(x, y, color)| {
                let at = text.min + vec2(x as f32, y as f32);
                Shape::rect_filled(Rect::from_min_size(at, Vec2::splat(1.0)), 0.0, color)
            }));
        }
        shapes
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
    /// (`active`), en el color tenue de la barra. La de un artboard de fuente (`font`) es la
    /// naranja.
    pub fn bar_shapes(
        &self,
        painter: &Painter,
        layout: &WindowLayout,
        title: &str,
        active: bool,
        font: bool,
        ppp: f32,
    ) -> Vec<Shape> {
        let look = if font { &self.font_title } else { &self.title };
        let icons = layout.menu.is_some();
        let state = TitleState {
            focused: active,
            menu: icons,
            // El "○" es decorativo por ahora: qué hace lo va a definir el usuario.
            close: icons.then_some(false),
        };
        let title = if layout.text.is_some() { title } else { "" };
        title_bar::bar_shapes(painter, look, layout.bar, title, state, ppp)
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
    fn the_card_wraps_the_sample_with_the_bar_and_five_points_to_the_ring() {
        // La referencia (`ff_zoom.png`): una muestra de 24×15 da una tarjeta de 36×40.
        let l = WindowLayout::card(pos2(15.0, 15.0), vec2(24.0, 15.0));
        assert_eq!(
            l.frame,
            Rect::from_min_max(pos2(15.0, 15.0), pos2(51.0, 55.0))
        );
        assert_eq!(
            l.bar,
            Rect::from_min_max(pos2(15.0, 15.0), pos2(51.0, 29.0))
        );
        // El texto, 5 después del anillo gris (x 16, y 29).
        assert_eq!(
            l.card,
            Some(Rect::from_min_size(pos2(21.0, 34.0), vec2(24.0, 15.0)))
        );
        assert!(l.menu.is_some() && l.close.is_some() && l.text.is_some());
        // Toda la tarjeta es la barra: se toma para activar y mover el artboard.
        assert_eq!(l.hit(pos2(30.0, 40.0)), Some(TitleHit::Bar));
        assert_eq!(l.hit(pos2(30.0, 54.5)), Some(TitleHit::Bar));
        assert_eq!(l.hit(pos2(30.0, 55.0)), None);
        assert_eq!(l.hit(l.menu.unwrap().center()), Some(TitleHit::Menu));
        // Las ventanas de siempre no son tarjetas.
        assert_eq!(WindowLayout::around(pixels(16.0, 16.0), 1.0).card, None);
    }

    fn view(glyphs: &[(char, Vec<(u32, u32)>)]) -> cs_core::PixelFontView {
        cs_core::PixelFontView {
            cell: (8, 10),
            metrics: cs_core::FontMetrics::for_cell(10),
            space_width: 4,
            missing_width: 5,
            glyphs: glyphs
                .iter()
                .map(|(ch, px)| {
                    let pixels = px.iter().map(|&(x, y)| (x, y, None)).collect();
                    (*ch, cs_core::GlyphPixels { pixels, advance: 4 })
                })
                .collect(),
        }
    }

    #[test]
    fn the_sample_is_cut_to_its_ink_or_the_cells_when_nothing_is_drawn() {
        // Todo el texto con glifos de un píxel en (1,2): la tinta va de la primera letra a la
        // última y de un renglón al otro (10 filas por renglón).
        let dot = vec![(1, 2)];
        let all: Vec<_> = "Abcde12345".chars().map(|c| (c, dot.clone())).collect();
        let s = CardSample::new(&view(&all));
        assert_eq!(s.size, vec2(4.0 * 4.0 + 1.0, 11.0));
        assert_eq!(s.pixels.first(), Some(&(0, 0, CARD_INK)));
        // Sin nada dibujado: los anchos (5 × 4) y dos celdas de alto.
        let empty: Vec<_> = "Abcde12345".chars().map(|c| (c, vec![])).collect();
        let s = CardSample::new(&view(&empty));
        assert_eq!(s.size, vec2(20.0, 20.0));
        assert!(s.pixels.is_empty());
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
