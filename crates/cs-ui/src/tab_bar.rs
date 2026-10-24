//! Barra de pestañas con Pixel UI, como en el mockup:
//!
//! - **Home:** sin caja. La casita animada (`icon.home.0`, `.1`, `.2`: casita y humo juntos, de
//!   `home animation.aseprite`), "Home" y el suelo (`home.ground`) sobre la línea grabada. El
//!   frame sale del tiempo; se pide un repintado justo cuando toca el siguiente.
//! - **Documentos:** una caja por pestaña. La activa es `tab.active`, clara y abierta abajo: tapa
//!   e interrumpe la línea grabada. Las demás son `tab`, café. Con Home activo, todas son `tab`.
//! - **La marca del final:** ○ (`icon.tab_dirty`) con cambios sin guardar y × (`icon.tab_close`)
//!   sin ellos o con el puntero encima. Un clic en ella cierra la pestaña (preguntando si tiene
//!   cambios). Si la pestaña es más angosta que su marco, no hay marca.
//! - **Muchas pestañas:** se achican parejo y los nombres terminan en "..." (la fuente pixel no
//!   tiene "…"). Nunca hay scroll ni segunda fila.
//!
//! Al final de la barra del menú, contra el borde derecho de la ventana, va el degradé tramado
//! `menubar.fade`, detrás del menú y de las pestañas.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CursorIcon, Galley, Rangef, Rect, Sense, Shape, Stroke, StrokeKind, TextStyle,
    Ui, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use crate::tabs::TabInfo;

/// Alto de la barra del menú con Pixel UI: las filas 34 a 47 del mockup.
pub const MENU_HEIGHT: f32 = 14.0;
/// Margen de arriba del menú con Pixel UI. egui centra el texto en el botón del menú, más alto
/// que la barra: con −2, las mayúsculas quedan en la fila 3, como en el mockup.
pub const MENU_TEXT_TOP: i8 = -2;
/// La fila de arriba de las mayúsculas de los títulos del menú, como en el mockup.
pub const MENU_CAPS_TOP: f32 = 3.0;
/// Los títulos del menú con Pixel UI: el primero empieza 3 px adentro (margen más relleno) y
/// entre la tinta de uno y la del siguiente hay 11 px, como en el mockup. Las letras de la fuente
/// pixel dejan 1 px vacío al final, así que entre texto y texto quedan 10: el relleno de los dos
/// lados y el espacio.
pub const MENU_LEFT: i8 = 0;
pub const MENU_ITEM_PADDING: f32 = 3.0;
pub const MENU_ITEM_GAP: f32 = 4.0;
/// Alto de la barra de pestañas con Pixel UI, con la línea grabada de abajo: las filas 48 a 70
/// del mockup.
pub const TABS_HEIGHT: f32 = 23.0;

const TAB_PIECE: &str = "tab";
/// La animación de la casita: `icon.home.0`, `icon.home.1`…
const HOME_ICON: &str = "icon.home";
/// Lo que dura un frame sin `duration=` en su slice: lo de Aseprite por defecto.
const DEFAULT_FRAME_MS: u32 = 100;
const HOME_GROUND: &str = "home.ground";
const DIRTY_MARK: &str = "icon.tab_dirty";
const CLOSE_MARK: &str = "icon.tab_close";
const FADE_PIECE: &str = "menubar.fade";

/// Los frames de la casita (21×21), a 3 px del borde izquierdo y arriba de todo: la casita
/// empieza 1 px adentro del frame, así que su cuerpo cae donde está en el mockup.
const HOME_ICON_X: f32 = 3.0;
/// "Home": a 26 px del borde, con las mayúsculas desde la fila 11.
const HOME_TEXT_X: f32 = 26.0;
const HOME_TEXT_TOP: f32 = 11.0;
/// Donde empieza la primera pestaña; antes queda Home.
const FIRST_TAB_X: f32 = 58.0;
/// Aire entre pestaña y pestaña.
const TAB_GAP: f32 = 6.0;
/// Fila de arriba de las cajas.
const TAB_TOP: f32 = 6.0;
/// Alto de la caja inactiva y de la activa, que baja sobre las dos filas de la línea grabada.
const TAB_HEIGHT: f32 = 15.0;
const ACTIVE_TAB_HEIGHT: f32 = TAB_HEIGHT + pixel_ui::ETCHED_SIZE;
/// Del borde de la caja al nombre, y de la marca al borde: la línea, el brillo y 3 px de aire.
const TAB_PADDING: f32 = 5.0;
/// Las mayúsculas del nombre empiezan 5 filas debajo del borde de arriba.
const TAB_TEXT_TOP: f32 = 5.0;
/// Entre el nombre y la marca, el espacio de la fuente pixel.
const MARK_GAP: f32 = 5.0;
/// Lado de la marca y su fila, 7 debajo del borde de arriba.
const MARK_SIZE: f32 = 5.0;
const MARK_TOP: f32 = 7.0;
/// Lo que la marca agranda su zona de clic a cada lado.
const MARK_HIT: f32 = 2.0;
/// Alto de las mayúsculas de la fuente pixel.
const CAP_HEIGHT: f32 = 7.0;
/// El final de los nombres cortados: "…" si la fuente pixel lo tiene; si no, tres puntos (el
/// "…" de la fuente de respaldo de egui no es pixel art y se ve borroso).
pub(crate) fn ellipsis() -> &'static str {
    if crate::appearance::pixel_font_has('…') {
        "…"
    } else {
        "..."
    }
}

/// El frame de una animación con frames de `durations` milisegundos que se repite desde el
/// tiempo 0, en el tiempo `time` (segundos), y los segundos que le faltan para pasar al
/// siguiente. `None` sin frames.
pub(crate) fn animation_frame(durations: &[u32], time: f64) -> Option<(usize, f64)> {
    // En microsegundos enteros: 0,1 s cae justo en el borde del frame, sin errores de redondeo.
    let total: u64 = durations.iter().map(|&ms| u64::from(ms) * 1000).sum();
    if total == 0 {
        return None;
    }
    let mut t = ((time.max(0.0) * 1e6).round() as u64) % total;
    for (i, &ms) in durations.iter().enumerate() {
        let d = u64::from(ms) * 1000;
        if t < d {
            return Some((i, (d - t) as f64 / 1e6));
        }
        t -= d;
    }
    None
}

/// Lo que se pidió en la barra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabAction {
    Home,
    Switch(usize),
    /// Cerrar (preguntando si tiene cambios): la marca o el botón del medio.
    Close(usize),
}

/// El degradé tramado del final de la barra, a tamaño de pieza contra la esquina de arriba a la
/// derecha de `screen`. `None` si el skin no lo trae.
pub(crate) fn fade_shapes(skin: &Skin, screen: Rect, pixels_per_point: f32) -> Option<Vec<Shape>> {
    let piece = skin.piece(FADE_PIECE, WidgetState::Normal)?;
    let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
    let rect = Rect::from_min_size(pos2(screen.right() - size.x, screen.top()), size);
    Some(pixel_ui::piece_shapes(piece, rect, pixels_per_point))
}

/// La barra de pestañas con el skin en todo `ui.max_rect()` (de `TABS_HEIGHT` de alto, con la
/// línea grabada en las dos filas de abajo). `tabs` son las pestañas en orden y `active`, la del
/// documento a la vista (`None`: Home). Devuelve lo que se pidió.
pub(crate) fn pixel_tab_bar(
    ui: &mut Ui,
    skin: &Skin,
    tabs: &[TabInfo],
    active: Option<usize>,
) -> Option<TabAction> {
    let bar = ui.max_rect();
    ui.allocate_rect(bar, Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let layout = layout_tabs(ui, bar, tabs);
    let active_rect = active.and_then(|i| layout.get(i)).map(|t| t.rect(true));
    // La línea grabada, cortada debajo de la pestaña activa (su caja la reemplaza).
    let etched_top = bar.bottom() - pixel_ui::ETCHED_SIZE;
    let colors = *skin.colors();
    let spans = match active_rect {
        Some(r) => vec![
            Rangef::new(bar.left(), r.left()),
            Rangef::new(r.right(), bar.right()),
        ],
        None => vec![bar.x_range()],
    };
    for x in spans.into_iter().filter(|x| x.span() > 0.0) {
        ui.painter()
            .add(pixel_ui::etched_hline(x, etched_top, &colors, ppp));
    }
    let mut action = home(ui, skin, bar, etched_top).then_some(TabAction::Home);
    for (i, tab) in layout.iter().enumerate() {
        let is_active = active == Some(i);
        if let Some(a) = tab_widget(ui, skin, tab, &tabs[i], i, is_active) {
            action = Some(a);
        }
    }
    action
}

/// Home: la casita con el humo, "Home" y el suelo sobre la línea grabada (fila `ground`). No
/// tiene caja ni estado activo. Devuelve si se hizo clic.
fn home(ui: &mut Ui, skin: &Skin, bar: Rect, ground: f32) -> bool {
    let ppp = ui.ctx().pixels_per_point();
    let rect = Rect::from_min_size(bar.min, vec2(FIRST_TAB_X - TAB_GAP, ground - bar.top()));
    let response = ui.interact(rect, ui.id().with("home"), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Home"));
    let mut shapes = Vec::new();
    if let Some(piece) = skin.piece(HOME_GROUND, WidgetState::Normal) {
        let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
        let at = Rect::from_min_size(pos2(bar.left(), ground), size);
        shapes.extend(pixel_ui::piece_shapes(piece, at, ppp));
    }
    let frames = skin.animation(HOME_ICON);
    let durations: Vec<u32> = frames
        .iter()
        .map(|f| f.duration_ms.unwrap_or(DEFAULT_FRAME_MS))
        .collect();
    let time = ui.input(|i| i.time);
    if let Some((i, left)) = animation_frame(&durations, time) {
        let piece = frames[i];
        let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
        let at = Rect::from_min_size(bar.min + vec2(HOME_ICON_X, 0.0), size);
        shapes.extend(pixel_ui::piece_shapes(piece, at, ppp));
        // Un solo frame no se anima, y sin el foco de la ventana la casita se queda quieta para
        // no repintar la app en segundo plano.
        if frames.len() > 1 && ui.input(|i| i.focused) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(left));
        }
    }
    let color = skin.colors().home_text;
    let galley = ui
        .painter()
        .layout_no_wrap("Home".to_owned(), font(ui), color);
    let at = text_pos(
        &galley,
        bar.left() + HOME_TEXT_X,
        bar.top() + HOME_TEXT_TOP,
        ppp,
    );
    shapes.push(Shape::galley(at, galley, color));
    ui.painter().extend(shapes);
    response.clicked()
}

/// Una pestaña acomodada: dónde empieza, su ancho y el nombre como entra.
struct PlacedTab {
    left: f32,
    top: f32,
    width: f32,
    text: Arc<Galley>,
}

impl PlacedTab {
    /// La caja: la activa baja sobre la línea grabada.
    fn rect(&self, active: bool) -> Rect {
        let height = if active {
            ACTIVE_TAB_HEIGHT
        } else {
            TAB_HEIGHT
        };
        Rect::from_min_size(pos2(self.left, self.top), vec2(self.width, height))
    }
}

/// La fuente de las pestañas: la de los textos (la pixel con Pixel UI).
fn font(ui: &Ui) -> egui::FontId {
    TextStyle::Body.resolve(ui.style())
}

/// Ancho de `text` sin cortar.
fn text_width(ui: &Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font(ui), Color32::WHITE)
        .size()
        .x
        .ceil()
}

/// Lo que ocupa una pestaña además del nombre: los bordes, el espacio y la marca.
const TAB_CHROME: f32 = TAB_PADDING + MARK_GAP + MARK_SIZE + TAB_PADDING;

/// Acomoda las pestañas en una fila desde `FIRST_TAB_X`. Si no entran, se achican parejo (las
/// más anchas primero) hasta lo justo para "…" y la marca; si ni así entran, todas por igual
/// hasta caber. Los anchos son enteros.
fn layout_tabs(ui: &Ui, bar: Rect, tabs: &[TabInfo]) -> Vec<PlacedTab> {
    let natural: Vec<f32> = tabs
        .iter()
        .map(|t| text_width(ui, &t.name) + TAB_CHROME)
        .collect();
    let gaps = TAB_GAP * tabs.len().saturating_sub(1) as f32;
    let room = (bar.right() - bar.left() - FIRST_TAB_X - gaps)
        .floor()
        .max(0.0);
    let min = text_width(ui, ellipsis()) + TAB_CHROME;
    let widths = fit_widths(&natural, room, min);
    let mut left = bar.left() + FIRST_TAB_X;
    tabs.iter()
        .zip(widths)
        .map(|(tab, width)| {
            let room = width - TAB_CHROME;
            let name = truncated(ui, &tab.name, room);
            let text = ui
                .painter()
                .layout_no_wrap(name, font(ui), Color32::PLACEHOLDER);
            let placed = PlacedTab {
                left,
                top: bar.top() + TAB_TOP,
                width,
                text,
            };
            left += width + TAB_GAP;
            placed
        })
        .collect()
}

/// Los anchos de las pestañas para que sumen a lo sumo `room`: los `natural` si entran; si no, un
/// tope parejo (las que miden menos se quedan como están), sin bajar de `min`. Si ni con `min`
/// entran, todas miden lo mismo. Siempre enteros.
fn fit_widths(natural: &[f32], room: f32, min: f32) -> Vec<f32> {
    let n = natural.len();
    if n == 0 || natural.iter().sum::<f32>() <= room {
        return natural.to_vec();
    }
    if min * n as f32 > room {
        let each = (room / n as f32).floor().max(0.0);
        return vec![each; n];
    }
    // El tope más grande con el que entran: se reparte de la más angosta a la más ancha.
    let mut sorted = natural.to_vec();
    sorted.sort_by(f32::total_cmp);
    let mut left = room;
    let mut cap = min;
    for (i, w) in sorted.iter().enumerate() {
        let even = (left / (n - i) as f32).floor();
        if *w > even {
            cap = even.max(min);
            break;
        }
        left -= w;
    }
    natural.iter().map(|w| w.min(cap)).collect()
}

/// `name` entero si mide hasta `room`; si no, lo más largo que entre seguido de `ellipsis()` (o
/// nada si ni eso entra).
fn truncated(ui: &Ui, name: &str, room: f32) -> String {
    if text_width(ui, name) <= room {
        return name.to_owned();
    }
    let chars: Vec<char> = name.chars().collect();
    let end = ellipsis();
    let cut = |k: usize| chars[..k].iter().collect::<String>() + end;
    // Búsqueda binaria del prefijo más largo que entra con el final.
    let (mut lo, mut hi) = (0, chars.len());
    let fits = |k: usize| text_width(ui, &cut(k)) <= room;
    if !fits(0) {
        return String::new();
    }
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

/// Dónde va un texto para que sus mayúsculas empiecen en la fila `cap_top`, alineado a píxeles.
pub(crate) fn text_pos(
    galley: &Galley,
    left: f32,
    cap_top: f32,
    pixels_per_point: f32,
) -> egui::Pos2 {
    let baseline = galley
        .rows
        .first()
        .and_then(|row| row.glyphs.first().map(|g| row.pos.y + g.pos.y))
        .unwrap_or(CAP_HEIGHT);
    pos2(left, cap_top - (baseline - CAP_HEIGHT)).round_to_pixels(pixels_per_point)
}

/// Una pestaña: la caja, el nombre y la marca. Devuelve lo que se pidió en ella.
fn tab_widget(
    ui: &mut Ui,
    skin: &Skin,
    tab: &PlacedTab,
    info: &TabInfo,
    index: usize,
    active: bool,
) -> Option<TabAction> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let rect = tab.rect(active);
    let enabled = ui.is_enabled();
    let response = ui.interact(rect, ui.id().with(("tab", index)), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, active, &info.name)
    });
    let mark = Rect::from_min_size(
        pos2(
            rect.right() - TAB_PADDING - MARK_SIZE,
            rect.top() + MARK_TOP,
        ),
        vec2(MARK_SIZE, MARK_SIZE),
    );
    // En una pestaña más angosta que su marco no hay marca ni zona de clic: se saldrían de la
    // caja y le robarían el clic a la vecina. Se cierra con el botón del medio.
    let close = (rect.width() >= TAB_CHROME).then(|| {
        // Después de la pestaña: la marca gana el clic.
        let close = ui
            .interact(
                mark.expand(MARK_HIT),
                ui.id().with(("tab_close", index)),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Close");
        let label = format!("Close {}", info.name);
        close.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        close
    });
    // Pasar el puntero por la caja no cambia nada: el mockup no muestra otro aspecto.
    let state = if active {
        WidgetState::Active
    } else {
        WidgetState::Normal
    };
    let mut shapes = match skin.piece(TAB_PIECE, state) {
        Some(piece) => pixel_ui::piece_shapes(piece, rect, ppp),
        None => flat_tab(rect, active, &colors, ppp),
    };
    let color = if active {
        colors.text
    } else {
        colors.tab_text_dim
    };
    let at = text_pos(
        &tab.text,
        rect.left() + TAB_PADDING,
        rect.top() + TAB_TEXT_TOP,
        ppp,
    );
    shapes.push(Shape::galley(at, tab.text.clone(), color));
    if let Some(close) = &close {
        // La ○ de los cambios sin guardar pasa a × con el puntero encima: un clic la cierra.
        let dot = info.dirty && !close.hovered();
        let mark_piece = if dot { DIRTY_MARK } else { CLOSE_MARK };
        match skin.piece(mark_piece, WidgetState::Normal) {
            Some(piece) => shapes.extend(pixel_ui::piece_shapes(piece, mark, ppp)),
            None => shapes.extend(flat_mark(mark, dot, colors.text_dim, ppp)),
        }
    }
    ui.painter().extend(shapes);
    let close_clicked = close.is_some_and(|c| c.clicked());
    if close_clicked || response.middle_clicked() {
        Some(TabAction::Close(index))
    } else if response.clicked() {
        Some(TabAction::Switch(index))
    } else {
        None
    }
}

/// Respaldo de la caja: el fondo (la activa) o `canvas` (las demás) con un borde `outline`.
fn flat_tab(rect: Rect, active: bool, colors: &crate::skin::SkinColors, ppp: f32) -> Vec<Shape> {
    let rect = rect.round_to_pixels(ppp);
    let fill = if active {
        colors.background
    } else {
        colors.canvas
    };
    vec![
        Shape::rect_filled(rect, 0.0, fill),
        Shape::rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, colors.outline),
            StrokeKind::Inside,
        ),
    ]
}

/// Respaldo de la marca: un cuadradito (con cambios) o una × de líneas.
fn flat_mark(rect: Rect, dirty: bool, color: Color32, ppp: f32) -> Vec<Shape> {
    let r = rect.round_to_pixels(ppp);
    let stroke = Stroke::new(1.0, color);
    if dirty {
        vec![Shape::rect_stroke(r, 0.0, stroke, StrokeKind::Inside)]
    } else {
        vec![
            Shape::line_segment([r.left_top(), r.right_bottom()], stroke),
            Shape::line_segment([r.right_top(), r.left_bottom()], stroke),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(frame, segundos hasta el siguiente)`, con los segundos redondeados al microsegundo.
    fn at(durations: &[u32], time: f64) -> Option<(usize, f64)> {
        animation_frame(durations, time).map(|(i, left)| (i, (left * 1e6).round() / 1e6))
    }

    #[test]
    fn the_home_frame_follows_the_time_and_wraps_around() {
        let home = [100, 100, 100];
        assert_eq!(at(&home, 0.0), Some((0, 0.1)));
        assert_eq!(at(&home, 0.1), Some((1, 0.1)));
        assert_eq!(at(&home, 0.25), Some((2, 0.05)));
        // Al final vuelve a empezar.
        assert_eq!(at(&home, 0.3), Some((0, 0.1)));
        assert_eq!(at(&home, 0.65), Some((0, 0.05)));
        assert_eq!(at(&home, 3.0 * 1000.0 + 0.12), Some((1, 0.08)));
        // Duraciones distintas.
        assert_eq!(at(&[50, 200], 0.06), Some((1, 0.19)));
        assert_eq!(at(&[], 1.0), None);
    }

    #[test]
    fn widths_that_fit_stay_natural() {
        assert_eq!(fit_widths(&[50.0, 70.0], 200.0, 30.0), vec![50.0, 70.0]);
    }

    #[test]
    fn the_widest_tabs_shrink_first_to_an_even_cap() {
        // 300 de lugar: la de 40 queda; las otras dos, 130 cada una.
        let w = fit_widths(&[40.0, 200.0, 180.0], 300.0, 30.0);
        assert_eq!(w, vec![40.0, 130.0, 130.0]);
        assert!(w.iter().sum::<f32>() <= 300.0);
    }

    #[test]
    fn names_are_cut_with_three_dots_since_the_pixel_font_has_no_ellipsis() {
        assert_eq!(ellipsis(), "...");
    }

    #[test]
    fn below_the_minimum_every_tab_gets_the_same() {
        assert_eq!(fit_widths(&[100.0; 4], 100.0, 30.0), vec![25.0; 4]);
        assert_eq!(fit_widths(&[100.0; 3], 0.0, 30.0), vec![0.0; 3]);
    }
}
