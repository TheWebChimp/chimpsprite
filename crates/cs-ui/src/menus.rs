//! Los menús de la barra con Pixel UI, como en el diseño (`Design.aseprite`, frame 3, capa
//! "Layer 4"):
//!
//! - **Títulos:** subrayan su primera letra (la app no tiene mnemónicos). El título abierto va en
//!   una caja redondeada arriba (`menu.title.open`) que se une al panel sin línea entre los dos.
//! - **Panel:** `menu.panel` en 9-slice, colgado de la caja: mismo borde izquierdo, y su borde de
//!   arriba empieza donde termina la caja.
//! - **Ítems:** filas de 13 px con el texto a la izquierda y el atajo a la derecha. Las teclas con
//!   nombre (Ctrl, Shift, Alt, Del…) son badges: `key` estirada con el nombre calado en
//!   chimpfont, en mayúsculas. Entre tecla y tecla, un + de 5×5 hecho con rects, y al final la
//!   tecla en el color tenue (`ruler_tick`, el del diseño).
//! - **Separadores:** la línea grabada del diseño, `menu_highlight` y debajo `background`.
//! - **Casillas:** las casillas del diseño, `menu.check.on` (marcada) y `menu.check` (sin
//!   marcar), de 11×12, al borde del texto de los ítems, 2 filas arriba de las mayúsculas, y el
//!   texto 3 px después. Si el skin no trae la pieza, un cuadro plano de respaldo.
//! - **Sin diseño todavía:** la fila con el puntero encima va en `selection`; los ítems
//!   deshabilitados, en `text_dim`; los submenús llevan un ▸ hecho con rects y el mismo panel.
//!
//! Los ítems ([`item`], [`plain`]), las casillas ([`check`]), los separadores ([`separator`]) y
//! los submenús
//! ([`submenu`]) se ven así solo dentro de un panel pixel; en cualquier otro lado (o sin Pixel
//! UI) son los de egui de siempre.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, Frame, Galley, InnerResponse, Margin, Mesh, Popup, Rect, RectAlign, Response,
    Sense, Shape, Style, TextStyle, Ui, UiBuilder, UiKind, UiStackInfo, Vec2, Widget, WidgetInfo,
    WidgetType, containers::menu, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::pixel_ui;
use crate::skin::{Piece, Skin, SkinColors, WidgetState};
use crate::tab_bar;

/// La marca de un panel pixel en la pila de `Ui`s: adentro, los ítems usan [`MenuLook`].
const LOOK_TAG: &str = "chimpsprite_pixel_menu";

const TITLE_OPEN: &str = "menu.title.open";
const PANEL: &str = "menu.panel";
const KEY: &str = "key";
const CHECK: &str = "menu.check";
const CHECK_ON: &str = "menu.check.on";
/// Las casillas sobre la fila con el puntero: el borde y la luz de abajo en los rosas.
const CHECK_HOVER: &str = "menu.check.hover";
const CHECK_ON_HOVER: &str = "menu.check.on.hover";

/// La caja del título abierto: empieza 5 px antes del texto (línea, bisel y 3 de relleno) y 5
/// filas arriba de las mayúsculas, y termina 4 px después del texto (la fuente deja 1 vacío).
const BOX_LEFT: f32 = 5.0;
const BOX_RIGHT: f32 = 4.0;
const BOX_TOP: f32 = 5.0;
/// El borde de arriba del panel, 11 filas debajo de las mayúsculas del título. La caja baja 2
/// filas más y tapa ese borde y el bisel.
const PANEL_TOP: f32 = 11.0;
const BOX_JOIN: f32 = 2.0;
/// El panel de un menú de la barra va 1 px abajo de donde lo deja la caja; a la izquierda, su
/// borde sigue el de la caja, como en el diseño (frame 4: la línea de File baja derecho).
const PANEL_SHIFT: Vec2 = vec2(0.0, 1.0);
/// Del borde del panel al contenido: la línea y el bisel arriba y abajo, y 1 fila de aire para
/// que la fila con el puntero no toque el bisel (abajo, 2: esa fila baja 1 más que la suya); a
/// los lados, además, 3 px de relleno (el texto de los ítems queda 5 px adentro, como en el
/// diseño).
const PANEL_MARGIN: Margin = Margin {
    left: 5,
    right: 5,
    top: 3,
    bottom: 4,
};
/// Lo que la fila con el puntero encima se extiende a cada lado, hasta los biseles, y abajo.
const ROW_BLEED: f32 = 3.0;
const ROW_HOVER_BELOW: f32 = 1.0;
/// Alto de una fila: 3 filas, las mayúsculas (7) y 3 más (las colas y 1).
pub const ROW_HEIGHT: f32 = 13.0;
const ROW_CAPS_TOP: f32 = 3.0;
/// Espacio mínimo entre el texto de un ítem y su atajo.
const SHORTCUT_GAP: f32 = 12.0;
/// El separador: 2 filas de aire, la línea clara, la oscura y 2 de aire.
const SEPARATOR_HEIGHT: f32 = 6.0;
const SEPARATOR_AIR: f32 = 2.0;
/// El subrayado de la primera letra de los títulos, 8 filas debajo de las mayúsculas.
const UNDERLINE_TOP: f32 = 8.0;
/// Alto de las mayúsculas de la fuente pixel.
const CAP_HEIGHT: f32 = 7.0;
/// El + entre teclas: 5×5, a 1 fila de las mayúsculas, con 1 px de aire a cada lado.
const PLUS_SIZE: f32 = 5.0;
const PLUS_TOP: f32 = 1.0;
const KEY_GAP: f32 = 1.0;
/// Las teclas con nombre (Ctrl, Shift, Alt, Del, F11…) en chimpfont, en mayúsculas como la badge
/// DEL del diseño: de 7 de alto (las mayúsculas de 5 con 1 fila arriba y abajo), 3 px antes de
/// la tinta y 4 después.
const KEY_PAD_X: f32 = 3.0;
const KEY_PAD_RIGHT: f32 = 4.0;
const KEY_PAD_Y: f32 = 1.0;
const KEY_HEIGHT: f32 = 7.0;

/// El nombre de una tecla en chimpfont (mayúsculas).
fn key_galley(ui: &Ui, name: &str, color: Color32) -> Arc<Galley> {
    // Sin la familia de los textos chicos (fuentes sin cargar todavía), la de siempre.
    let small = crate::appearance::small_family();
    let font = if ui.ctx().fonts(|f| f.families().contains(&small)) {
        egui::FontId::new(crate::appearance::SMALL_FONT_SIZE, small)
    } else {
        font(ui)
    };
    ui.painter()
        .layout_no_wrap(name.to_uppercase(), font, color)
}
/// La casilla: 2 filas arriba de las mayúsculas, del tamaño de la pieza (la de respaldo, la del
/// diseño) y con 3 px de aire hasta el texto.
const CHECK_TOP: f32 = 2.0;
const FALLBACK_CHECK_SIZE: egui::Vec2 = vec2(11.0, 12.0);
const CHECK_GAP: f32 = 3.0;
/// Lo que egui separa el panel de un submenú del de su menú (la mitad de los márgenes y 2, menos
/// el margen derecho del panel).
const SUBMENU_PULL: f32 = 2.0;
/// La flecha de los submenús: un triángulo de 3×5 a 1 fila de las mayúsculas.
const ARROW_TOP: f32 = 1.0;

/// Lo que dibujan los menús con Pixel UI: las piezas del skin y sus colores, más dónde están las
/// mayúsculas de la barra. Viaja en la pila de `Ui`s de cada panel.
#[derive(Clone)]
pub(crate) struct MenuLook {
    title_open: Option<Piece>,
    panel: Option<Piece>,
    key: Option<Piece>,
    check: Option<Piece>,
    check_on: Option<Piece>,
    check_hover: Option<Piece>,
    check_on_hover: Option<Piece>,
    /// El círculo de las opciones de una lista (`menu.radio`, `menu.radio.on` la elegida).
    radio: Option<Piece>,
    radio_on: Option<Piece>,
    colors: SkinColors,
    /// La fila de arriba de las mayúsculas de los títulos.
    caps_top: f32,
    /// La opacidad del panel (egui lo muestra con un fundido). Los ítems deshabilitados pintan
    /// con esta, sin el fundido de egui: el gris es `text_dim`.
    opacity: f32,
}

impl MenuLook {
    /// `bar_top`: la fila de arriba del menú; las mayúsculas de los títulos van
    /// `tab_bar::MENU_CAPS_TOP` filas más abajo.
    pub fn new(skin: &Skin, bar_top: f32) -> Self {
        let piece = |name| skin.piece(name, WidgetState::Normal).cloned();
        Self {
            title_open: piece(TITLE_OPEN),
            panel: piece(PANEL),
            key: piece(KEY),
            check: piece(CHECK),
            check_on: piece(CHECK_ON),
            check_hover: piece(CHECK_HOVER),
            check_on_hover: piece(CHECK_ON_HOVER),
            radio: piece("menu.radio"),
            radio_on: piece("menu.radio.on"),
            colors: *skin.colors(),
            caps_top: bar_top + tab_bar::MENU_CAPS_TOP,
            opacity: 1.0,
        }
    }
}

/// El [`MenuLook`] del panel pixel donde está `ui`, si está en uno.
fn look(ui: &Ui) -> Option<MenuLook> {
    ui.stack()
        .iter()
        .find_map(|s| s.info.tags.get_downcast::<MenuLook>(LOOK_TAG))
        .cloned()
}

/// El estilo de adentro de los menús pixel: sin el marco de egui (el panel lo dibuja el skin),
/// con los márgenes del panel y las filas pegadas.
fn pixel_menu_style(style: &mut Style) {
    menu::menu_style(style);
    style.spacing.menu_margin = PANEL_MARGIN;
    style.spacing.item_spacing.y = 0.0;
    let v = &mut style.visuals;
    v.window_fill = Color32::TRANSPARENT;
    v.window_stroke = egui::Stroke::NONE;
    v.popup_shadow = egui::Shadow::NONE;
    v.menu_corner_radius = egui::CornerRadius::ZERO;
}

fn font(ui: &Ui) -> egui::FontId {
    TextStyle::Body.resolve(ui.style())
}

pub(crate) fn galley(ui: &Ui, text: &str, color: Color32) -> Arc<Galley> {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font(ui), color)
}

/// Un texto como se muestra: "…" con tres puntos si la fuente pixel no lo tiene (el de la
/// fuente de respaldo de egui no es pixel art), como en las pestañas.
fn shown(text: &str) -> String {
    text.replace('…', tab_bar::ellipsis())
}

/// Un rect de color liso, alineado a píxeles físicos (como los separadores grabados).
pub(crate) fn solid(rect: Rect, color: Color32, ppp: f32) -> Shape {
    let mut mesh = Mesh::default();
    mesh.add_colored_rect(rect.round_to_pixels(ppp), color);
    Shape::mesh(mesh)
}

/// Un menú de la barra: con `look`, el título del diseño y el panel pixel; sin él, el
/// `menu_button` de egui.
pub(crate) fn bar_menu<R>(
    ui: &mut Ui,
    look: Option<&MenuLook>,
    title: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    let Some(look) = look else {
        return ui.menu_button(title, content);
    };
    let ppp = ui.ctx().pixels_per_point();
    // Debajo del texto: la caja si está abierto o el subrayado si no (se sabe después).
    let back = ui.painter().add(Shape::Noop);
    let color = look.colors.home_text;
    let text = galley(ui, title, color);
    let padding = ui.spacing().button_padding.x;
    let size = vec2(text.size().x + 2.0 * padding, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, title));
    let text_left = rect.left() + padding;
    let at = tab_bar::text_pos(&text, text_left, look.caps_top, ppp);
    let underline = first_letter_width(&text).map(|w| {
        let line =
            Rect::from_min_size(pos2(text_left, look.caps_top + UNDERLINE_TOP), vec2(w, 1.0));
        solid(line, color, ppp)
    });
    ui.painter().galley(at, text.clone(), color);
    // La caja: alrededor del texto, como en el diseño. La barra deja lugar justo arriba y a la
    // izquierda (`tab_bar::MENU_CAPS_TOP` y `tab_bar::MENU_LEFT`): la de File queda a 1 px
    // de la esquina de la ventana.
    let panel_top = look.caps_top + PANEL_TOP;
    let title_box = Rect::from_min_max(
        pos2(text_left - BOX_LEFT, look.caps_top - BOX_TOP),
        pos2(
            text_left + text.size().x + BOX_RIGHT,
            panel_top + PANEL_SHIFT.y + BOX_JOIN,
        ),
    );
    let config = menu::MenuConfig::new().style(pixel_menu_style);
    // El panel cuelga de la caja, corrido `PANEL_SHIFT` (1 fila abajo): su
    // esquina de arriba a la izquierda, siempre la misma.
    let anchor = Rect::from_min_max(
        pos2(title_box.left(), panel_top),
        pos2(title_box.right(), panel_top),
    )
    .translate(PANEL_SHIFT);
    let mut popup = Popup::menu(&response)
        .anchor(anchor)
        .align(RectAlign::BOTTOM_START)
        .align_alternatives(&[])
        .gap(0.0)
        .frame(Frame::NONE.inner_margin(PANEL_MARGIN))
        .style(pixel_menu_style)
        .info(
            UiStackInfo::new(UiKind::Menu)
                .with_tag_value(menu::MenuConfig::MENU_CONFIG_TAG, config),
        );
    // Con otro menú de la barra abierto, pasar por este lo abre (y el otro se cierra), como en
    // las barras de menú de siempre.
    let open_id = egui::Id::new("bar_menu_open");
    let other_open = ui
        .ctx()
        .data(|d| d.get_temp::<egui::Id>(open_id))
        .is_some_and(|id| id != response.id);
    // Desarrollo: el menú `CHIMPSPRITE_DEMO_MENU` abierto, para capturas.
    let demo = crate::demo::dev_var("CHIMPSPRITE_DEMO_MENU") == Some(title);
    if (other_open && response.hovered()) || demo {
        popup = popup.open_memory(Some(egui::SetOpenCommand::Bool(true)));
    }
    let inner = popup.show(|ui| panel(ui, look, Some(title_box), content));
    ui.ctx().data_mut(|d| {
        if inner.is_some() {
            d.insert_temp(open_id, response.id);
        } else if d.get_temp::<egui::Id>(open_id) == Some(response.id) {
            d.remove::<egui::Id>(open_id);
        }
    });
    let back_shapes = if inner.is_some() {
        box_shapes(look, title_box, ppp)
    } else {
        underline.into_iter().collect()
    };
    ui.painter().set(back, Shape::Vec(back_shapes));
    InnerResponse::new(inner.map(|i| i.inner), response)
}

/// El ancho de la tinta de la primera letra de `text`: su avance menos el píxel vacío que deja
/// la fuente pixel.
fn first_letter_width(text: &Galley) -> Option<f32> {
    let glyph = text.rows.first()?.glyphs.first()?;
    Some((glyph.advance_width - 1.0).max(1.0).round())
}

/// La caja del título abierto en `rect`; sin la pieza, un rect `panel` con borde `outline`.
fn box_shapes(look: &MenuLook, rect: Rect, ppp: f32) -> Vec<Shape> {
    match &look.title_open {
        Some(piece) => pixel_ui::piece_shapes(piece, rect, ppp),
        None => vec![solid(rect, look.colors.panel, ppp)],
    }
}

/// El panel pixel alrededor de `content`. `title_box`: la caja del título de la barra, que se
/// dibuja encima del borde de arriba del panel para unirlos.
fn panel<R>(
    ui: &mut Ui,
    look: &MenuLook,
    title_box: Option<Rect>,
    content: impl FnOnce(&mut Ui) -> R,
) -> R {
    let ppp = ui.ctx().pixels_per_point();
    let back = ui.painter().add(Shape::Noop);
    let join = ui.painter().add(Shape::Noop);
    if let Some(title_box) = title_box {
        // El panel no es más angosto que la caja.
        ui.set_min_width(title_box.width() - PANEL_MARGIN.sum().x);
    }
    let look = MenuLook {
        opacity: ui.painter().opacity(),
        ..look.clone()
    };
    let info = UiStackInfo::default().with_tag_value(LOOK_TAG, look.clone());
    let inner = ui.scope_builder(UiBuilder::new().ui_stack_info(info), content);
    let content_rect = inner.response.rect.union(ui.min_rect());
    let rect = Rect::from_min_max(
        content_rect.min - vec2(PANEL_MARGIN.left as f32, PANEL_MARGIN.top as f32),
        content_rect.max + vec2(PANEL_MARGIN.right as f32, PANEL_MARGIN.bottom as f32),
    );
    let shapes = match &look.panel {
        Some(piece) => pixel_ui::piece_shapes(piece, rect, ppp),
        None => vec![
            solid(rect, look.colors.panel, ppp),
            Shape::rect_stroke(
                rect.round_to_pixels(ppp),
                0.0,
                egui::Stroke::new(1.0, look.colors.outline),
                egui::StrokeKind::Inside,
            ),
        ],
    };
    ui.painter().set(back, Shape::Vec(shapes));
    if let Some(title_box) = title_box {
        // Las filas de la caja que bajan sobre el panel; las de arriba van en la barra. También
        // la fila entre la barra y el panel (`PANEL_SHIFT`: la barra no pinta fuera de sí).
        let gap = Rect::from_min_max(
            pos2(title_box.left(), rect.top() - PANEL_SHIFT.y),
            pos2(title_box.right(), title_box.bottom()),
        );
        ui.painter()
            .with_clip_rect(rect.union(gap))
            .set(join, Shape::Vec(box_shapes(&look, title_box, ppp)));
    }
    inner.inner
}

/// El pintor de las filas: el de `ui` con la opacidad del panel (sin el fundido de egui para
/// lo deshabilitado).
fn row_painter(ui: &Ui, look: &MenuLook) -> egui::Painter {
    let mut painter = ui.painter().clone();
    painter.set_opacity(look.opacity);
    painter
}

/// Un ítem de menú: el texto y su atajo (`""`: sin atajo, como los de egui).
pub(crate) struct Item<'a> {
    label: &'a str,
    shortcut: Option<std::borrow::Cow<'a, str>>,
}

/// Un ítem con atajo, como `Button::new(label).shortcut_text(shortcut)`. El atajo puede ser un
/// texto propio (el del registro de atajos).
pub(crate) fn item<'a>(label: &'a str, shortcut: impl Into<std::borrow::Cow<'a, str>>) -> Item<'a> {
    Item {
        label,
        shortcut: Some(shortcut.into()),
    }
}

/// Un ítem sin atajo, como `Button::new(label)`.
pub(crate) fn plain(label: &str) -> Item<'_> {
    Item {
        label,
        shortcut: None,
    }
}

impl Widget for Item<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Some(look) = look(ui) else {
            let button = egui::Button::new(self.label);
            return match self.shortcut {
                Some(keys) => button.shortcut_text(keys.as_ref()).ui(ui),
                None => button.ui(ui),
            };
        };
        let keys = self.shortcut.as_deref().unwrap_or_default();
        let parts = shortcut_parts(keys);
        let a11y = if keys.is_empty() {
            self.label.to_owned()
        } else {
            format!("{} {keys}", self.label)
        };
        let enabled = ui.is_enabled();
        let colors = &look.colors;
        let text_color = if enabled {
            colors.text
        } else {
            colors.text_dim
        };
        let label = galley(ui, &shown(self.label), text_color);
        let keys_width = parts_width(ui, &parts);
        let gap = if parts.is_empty() { 0.0 } else { SHORTCUT_GAP };
        let (rect, response) = row(ui, &label, gap + keys_width, &a11y);
        if ui.is_rect_visible(rect) {
            let ppp = ui.ctx().pixels_per_point();
            let mut shapes = row_shapes(ui, &look, rect, &label, text_color, &response);
            let caps = rect.top() + ROW_CAPS_TOP;
            shapes.extend(parts_shapes(
                ui,
                &look,
                &parts,
                rect.right(),
                caps,
                enabled,
                ppp,
            ));
            row_painter(ui, &look).extend(shapes);
        }
        response
    }
}

/// Una casilla de menú, como `Checkbox::new(checked, label)`.
pub(crate) struct Check<'a> {
    checked: &'a mut bool,
    label: &'a str,
}

/// Una casilla que prende y apaga `checked`, como `ui.checkbox(checked, label)`.
pub(crate) fn check<'a>(checked: &'a mut bool, label: &'a str) -> Check<'a> {
    Check { checked, label }
}

impl Widget for Check<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Check { checked, label } = self;
        let Some(look) = look(ui) else {
            return egui::Checkbox::new(checked, label).ui(ui);
        };
        let enabled = ui.is_enabled();
        let colors = &look.colors;
        let text_color = if enabled {
            colors.text
        } else {
            colors.text_dim
        };
        let text = galley(ui, &shown(label), text_color);
        let box_size = look
            .check_on
            .as_ref()
            .or(look.check.as_ref())
            .map_or(FALLBACK_CHECK_SIZE, |p| {
                vec2(p.size[0] as f32, p.size[1] as f32)
            });
        let indent = box_size.x + CHECK_GAP;
        let width = (indent + text.size().x).ceil();
        let (rect, mut response) = ui.allocate_at_least(vec2(width, ROW_HEIGHT), Sense::click());
        if response.clicked() {
            *checked = !*checked;
            response.mark_changed();
        }
        let on = *checked;
        response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, label));
        if ui.is_rect_visible(rect) {
            let ppp = ui.ctx().pixels_per_point();
            let caps = rect.top() + ROW_CAPS_TOP;
            let mut shapes = Vec::new();
            let hovered = enabled && response.hovered();
            if hovered {
                shapes.push(row_highlight(&look, rect, ppp));
            }
            let at = Rect::from_min_size(pos2(rect.left(), caps - CHECK_TOP), box_size);
            shapes.extend(check_shapes(&look, at, on, enabled, hovered, ppp));
            let pos = tab_bar::text_pos(&text, rect.left() + indent, caps, ppp);
            shapes.push(Shape::galley(pos, text, text_color));
            row_painter(ui, &look).extend(shapes);
        }
        response
    }
}

/// La casilla en `rect`: `menu.check.on` marcada y `menu.check` sin marcar (`hovered`: sobre la
/// fila con el puntero, sus versiones `.hover`). Sin la pieza, un cuadro plano (`panel` con borde
/// `outline`) y, si está marcada, un cuadrito `text` adentro. Deshabilitada, más clara.
fn check_shapes(
    look: &MenuLook,
    rect: Rect,
    on: bool,
    enabled: bool,
    hovered: bool,
    ppp: f32,
) -> Vec<Shape> {
    let hover = if on {
        &look.check_on_hover
    } else {
        &look.check_hover
    };
    let piece = match (hovered, hover) {
        (true, Some(_)) => hover,
        _ if on => &look.check_on,
        _ => &look.check,
    };
    if let Some(piece) = piece {
        let tint = if enabled {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(128)
        };
        return tinted(pixel_ui::piece_shapes(piece, rect, ppp), tint);
    }
    let square = Rect::from_min_size(rect.min, egui::Vec2::splat(rect.width()));
    let colors = &look.colors;
    let mut shapes = vec![
        solid(square, colors.panel, ppp),
        Shape::rect_stroke(
            square.round_to_pixels(ppp),
            0.0,
            egui::Stroke::new(1.0, colors.outline),
            egui::StrokeKind::Inside,
        ),
    ];
    if on {
        shapes.push(solid(square.shrink(3.0), colors.text, ppp));
    }
    shapes
}

/// Las mallas de `shapes` teñidas con `tint` (blanco: tal cual; más transparente para lo
/// deshabilitado).
fn tinted(mut shapes: Vec<Shape>, tint: Color32) -> Vec<Shape> {
    for shape in &mut shapes {
        if let Shape::Mesh(mesh) = shape {
            for v in &mut Arc::make_mut(mesh).vertices {
                v.color = tint;
            }
        }
    }
    shapes
}

/// Reserva una fila para un texto de `label` más `extra` a la derecha (a todo el ancho del panel)
/// y le pone el nombre accesible `a11y`.
fn row(ui: &mut Ui, label: &Galley, extra: f32, a11y: &str) -> (Rect, Response) {
    let width = (label.size().x + extra).ceil();
    let (rect, response) = ui.allocate_at_least(vec2(width, ROW_HEIGHT), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, a11y));
    (rect, response)
}

/// El fondo de la fila (en `selection` con el puntero encima o `highlighted`) y su texto.
fn row_shapes(
    ui: &Ui,
    look: &MenuLook,
    rect: Rect,
    label: &Arc<Galley>,
    color: Color32,
    response: &Response,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = Vec::new();
    if ui.is_enabled() && response.hovered() {
        shapes.push(row_highlight(look, rect, ppp));
    }
    let at = tab_bar::text_pos(label, rect.left(), rect.top() + ROW_CAPS_TOP, ppp);
    shapes.push(Shape::galley(at, label.clone(), color));
    shapes
}

/// La fila resaltada, de bisel a bisel.
fn row_highlight(look: &MenuLook, rect: Rect, ppp: f32) -> Shape {
    // El rosa de lo elegido de lado a lado, 1 fila más abajo que la fila; los biseles del panel
    // a su altura, en el rosa oscuro (el de la izquierda) y el claro (el de la derecha).
    let colors = &look.colors;
    let fill = Rect::from_min_max(
        rect.min - vec2(ROW_BLEED, 0.0),
        rect.max + vec2(ROW_BLEED, ROW_HOVER_BELOW),
    );
    let bevel = |x: f32| Rect::from_min_max(pos2(x, fill.top()), pos2(x + 1.0, fill.bottom()));
    Shape::Vec(vec![
        solid(fill, colors.timeline_selection, ppp),
        solid(
            bevel(fill.left() - 1.0),
            colors.timeline_selection_dark,
            ppp,
        ),
        solid(bevel(fill.right()), colors.timeline_selection_light, ppp),
    ])
}

/// Una parte de un atajo.
#[derive(Clone, Debug, PartialEq, Eq)]
enum KeyPart<'a> {
    /// Una tecla dibujada: Ctrl o una con nombre (Shift, Del…).
    Cap(&'a str),
    /// El + entre dos partes.
    Plus,
    /// La tecla final de un carácter, en el color tenue.
    Key(&'a str),
}

/// Las partes de un atajo como los de egui: `"Ctrl+Shift+S"` son Ctrl, +, Shift, + y S; `"Del"`,
/// una tecla dibujada; `"+"`, la tecla +.
fn shortcut_parts(keys: &str) -> Vec<KeyPart<'_>> {
    let keys = keys.trim();
    if keys.is_empty() {
        return Vec::new();
    }
    // La tecla final puede ser el propio "+" ("Ctrl++").
    let (mods, last) = match keys.strip_suffix('+') {
        Some(rest) if rest.is_empty() || rest.ends_with('+') => {
            (rest.strip_suffix('+').unwrap_or(rest), "+")
        }
        _ => match keys.rsplit_once('+') {
            Some((mods, last)) => (mods, last),
            None => ("", keys),
        },
    };
    let mut parts = Vec::new();
    for m in mods.split('+').filter(|m| !m.is_empty()) {
        parts.push(KeyPart::Cap(m));
        parts.push(KeyPart::Plus);
    }
    // Las teclas de un carácter van como texto; las de nombre (Del, Tab, F11…), dibujadas.
    if last.chars().count() > 1 {
        parts.push(KeyPart::Cap(last));
    } else {
        parts.push(KeyPart::Key(last));
    }
    parts
}

/// El ancho de una badge: la tinta del nombre en chimpfont con su aire a cada lado.
fn cap_width(ui: &Ui, name: &str) -> f32 {
    key_ink(&key_galley(ui, name, Color32::PLACEHOLDER)) + KEY_PAD_X + KEY_PAD_RIGHT
}

/// La tinta del nombre de una badge: chimpfont deja 2 px vacíos después de la última letra (la
/// fuente pixel, 1).
fn key_ink(text: &Galley) -> f32 {
    let small = text
        .job
        .sections
        .first()
        .is_some_and(|s| s.format.font_id.family == crate::appearance::small_family());
    if small {
        (text.size().x - 2.0).max(0.0).round()
    } else {
        ink_width(text)
    }
}

/// La tinta de un texto de la fuente pixel: su ancho menos el píxel vacío del final.
pub(crate) fn ink_width(text: &Galley) -> f32 {
    (text.size().x - 1.0).max(0.0).round()
}

fn part_width(ui: &Ui, part: &KeyPart<'_>) -> f32 {
    match part {
        KeyPart::Cap(name) => cap_width(ui, name),
        KeyPart::Plus => KEY_GAP + PLUS_SIZE + KEY_GAP,
        KeyPart::Key(key) => ink_width(&galley(ui, key, Color32::PLACEHOLDER)),
    }
}

fn parts_width(ui: &Ui, parts: &[KeyPart<'_>]) -> f32 {
    parts.iter().map(|p| part_width(ui, p)).sum()
}

/// Las formas de un atajo cuya tinta termina en `right`, con las mayúsculas en la fila `caps`.
/// Deshabilitado, todo más claro.
fn parts_shapes(
    ui: &Ui,
    look: &MenuLook,
    parts: &[KeyPart<'_>],
    right: f32,
    caps: f32,
    enabled: bool,
    ppp: f32,
) -> Vec<Shape> {
    let colors = &look.colors;
    let dim = if enabled {
        colors.ruler_tick
    } else {
        colors.text_dim
    };
    let tint = if enabled {
        Color32::WHITE
    } else {
        Color32::from_white_alpha(128)
    };
    let mut x = right - parts_width(ui, parts);
    let mut shapes = Vec::new();
    for part in parts {
        let width = part_width(ui, part);
        match part {
            KeyPart::Cap(name) => shapes.extend(cap_shapes(ui, look, name, x, caps, tint, ppp)),
            KeyPart::Plus => {
                let left = x + KEY_GAP;
                let top = caps + PLUS_TOP;
                let mid = (PLUS_SIZE / 2.0).floor();
                let across = Rect::from_min_size(pos2(left, top + mid), vec2(PLUS_SIZE, 1.0));
                let down = Rect::from_min_size(pos2(left + mid, top), vec2(1.0, PLUS_SIZE));
                shapes.push(solid(across, dim, ppp));
                shapes.push(solid(down, dim, ppp));
            }
            KeyPart::Key(key) => {
                let text = galley(ui, key, dim);
                let at = tab_bar::text_pos(&text, x, caps, ppp);
                shapes.push(Shape::galley(at, text, dim));
            }
        }
        x += width;
    }
    shapes
}

/// Una badge en `left`, a la altura de las mayúsculas de la fila: `key` estirada con el nombre
/// en chimpfont, en el color del panel (las letras caladas).
fn cap_shapes(
    ui: &Ui,
    look: &MenuLook,
    name: &str,
    left: f32,
    caps: f32,
    tint: Color32,
    ppp: f32,
) -> Vec<Shape> {
    let text = key_galley(ui, name, look.colors.panel);
    let width = key_ink(&text) + KEY_PAD_X + KEY_PAD_RIGHT;
    // Desde las mayúsculas de la fila.
    let rect = Rect::from_min_size(pos2(left, caps), vec2(width, KEY_HEIGHT));
    let mut shapes = match &look.key {
        Some(piece) => tinted(pixel_ui::piece_shapes(piece, rect, ppp), tint),
        None => vec![solid(rect, look.colors.ruler_tick, ppp)],
    };
    let at = tab_bar::text_pos(&text, left + KEY_PAD_X, caps + KEY_PAD_Y, ppp);
    shapes.push(Shape::galley(at, text, look.colors.panel));
    shapes
}

/// Un separador: en un panel pixel, la línea grabada del diseño de bisel a bisel; si no, el de
/// egui.
pub(crate) fn separator(ui: &mut Ui) {
    let Some(look) = look(ui) else {
        ui.separator();
        return;
    };
    let ppp = ui.ctx().pixels_per_point();
    // Como el de egui: en la pasada de medida no pide ancho (si no, el panel tomaría todo el
    // que le ofrecen); después, el del panel.
    let width = if ui.is_sizing_pass() {
        0.0
    } else {
        ui.available_width()
    };
    let (rect, _) = ui.allocate_exact_size(vec2(width, SEPARATOR_HEIGHT), Sense::hover());
    let light = Rect::from_min_size(
        pos2(rect.left() - ROW_BLEED, rect.top() + SEPARATOR_AIR),
        vec2(rect.width() + 2.0 * ROW_BLEED, 1.0),
    );
    ui.painter().extend([
        solid(light, look.colors.menu_highlight, ppp),
        solid(light.translate(vec2(0.0, 1.0)), look.colors.background, ppp),
    ]);
}

/// Un submenú: en un panel pixel, una fila con ▸ y el mismo panel al costado; si no, el
/// `menu_button` de egui.
pub(crate) fn submenu<R>(
    ui: &mut Ui,
    label: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    let Some(look) = look(ui) else {
        return ui.menu_button(label, content);
    };
    let ppp = ui.ctx().pixels_per_point();
    let enabled = ui.is_enabled();
    let colors = look.colors;
    let text_color = if enabled {
        colors.text
    } else {
        colors.text_dim
    };
    let back = ui.painter().add(Shape::Noop);
    let text = galley(ui, &shown(label), text_color);
    let (rect, response) = row(ui, &text, SHORTCUT_GAP + 3.0, label);
    let caps = rect.top() + ROW_CAPS_TOP;
    let at = tab_bar::text_pos(&text, rect.left(), caps, ppp);
    let mut shapes = vec![Shape::galley(at, text, text_color)];
    // ▸: columnas de 5, 3 y 1 filas, la última contra la derecha.
    for (i, h) in [5.0, 3.0, 1.0].into_iter().enumerate() {
        let x = rect.right() - 3.0 + i as f32;
        let top = caps + ARROW_TOP + (5.0 - h) / 2.0;
        shapes.push(solid(
            Rect::from_min_size(pos2(x, top), vec2(1.0, h)),
            text_color,
            ppp,
        ));
    }
    let painter = row_painter(ui, &look);
    painter.extend(shapes);
    let config = menu::MenuConfig::new().style(pixel_menu_style);
    // egui deja el submenú 2 px más allá del borde del panel; con la fila acortada, su panel
    // queda pegado al de este menú.
    let mut anchor = response.clone();
    anchor.interact_rect.max.x -= SUBMENU_PULL;
    let open = menu::SubMenu::new()
        .config(config)
        .show(ui, &anchor, |ui| panel(ui, &look, None, content));
    if enabled && (open.is_some() || response.hovered()) {
        painter.set(back, row_highlight(&look, rect, ppp));
    }
    InnerResponse::new(open.map(|i| i.inner), response)
}

/// El menú contextual de `response` (se abre con el clic derecho, en el puntero): con Pixel UI,
/// el panel de los menús de la barra con sus filas, separadores y casillas; si no, el de egui.
pub(crate) fn context_menu(
    response: &Response,
    content: impl FnOnce(&mut Ui),
) -> Option<InnerResponse<()>> {
    let Some(skin) = pixel_ui::frame_skin(&response.ctx) else {
        return response.context_menu(content);
    };
    let look = MenuLook::new(&skin, 0.0);
    let config = menu::MenuConfig::new().style(pixel_menu_style);
    Popup::context_menu(response)
        .frame(Frame::NONE.inner_margin(PANEL_MARGIN))
        .style(pixel_menu_style)
        .info(
            UiStackInfo::new(UiKind::Menu)
                .with_tag_value(menu::MenuConfig::MENU_CONFIG_TAG, config),
        )
        .show(|ui| panel(ui, &look, None, content))
}

/// La lista de un combo pixel (los de la barra de opciones): el mismo panel de los menús, colgado
/// debajo de `anchor` (lo dibujado del combo) y al menos de su ancho. Se abre y se cierra con los
/// clics en `response`, como `Popup::menu`; elegir una fila la cierra.
pub(crate) fn dropdown<R>(
    look: &MenuLook,
    response: &Response,
    anchor: Rect,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let config = menu::MenuConfig::new().style(pixel_menu_style);
    // Un ancla angosta (el "≡" de un artboard) no deja un ancho negativo: egui lo rechaza.
    let min_width = (anchor.width() - PANEL_MARGIN.sum().x).max(0.0);
    Popup::menu(response)
        .anchor(anchor)
        .align(RectAlign::BOTTOM_START)
        .align_alternatives(&[RectAlign::TOP_START])
        .gap(0.0)
        .frame(Frame::NONE.inner_margin(PANEL_MARGIN))
        .style(pixel_menu_style)
        .info(
            UiStackInfo::new(UiKind::Menu)
                .with_tag_value(menu::MenuConfig::MENU_CONFIG_TAG, config),
        )
        .show(|ui| {
            ui.set_min_width(min_width);
            panel(ui, look, None, content)
        })
        .map(|i| i.inner)
}

/// El menú de las variantes de una herramienta (frame del diseño): el panel sale a la derecha del
/// botón `slot`, arriba alineado con él y compartiendo su línea, y el botón queda abierto, unido
/// al panel (sin su borde de la derecha). `open`: abrirlo o cerrarlo en este frame; con
/// `keep_open`, un clic (el de soltar después de mantener) no lo cierra.
pub(crate) fn side_menu<R>(
    look: &MenuLook,
    response: &Response,
    slot: Rect,
    open: Option<egui::SetOpenCommand>,
    keep_open: bool,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let config = menu::MenuConfig::new().style(pixel_menu_style);
    // El panel empieza en la línea de la derecha del botón.
    let anchor = Rect::from_min_max(slot.min, slot.max - vec2(1.0, 0.0));
    let mut popup = Popup::menu(response)
        .open_memory(open)
        .anchor(anchor)
        .align(RectAlign::RIGHT_START)
        .align_alternatives(&[])
        .gap(0.0)
        .frame(Frame::NONE.inner_margin(PANEL_MARGIN))
        .style(pixel_menu_style)
        .info(
            UiStackInfo::new(UiKind::Menu)
                .with_tag_value(menu::MenuConfig::MENU_CONFIG_TAG, config),
        );
    if keep_open {
        popup = popup.close_behavior(egui::PopupCloseBehavior::IgnoreClicks);
    }
    let inner = popup.show(|ui| panel(ui, look, None, content))?;
    // La unión: donde el panel toca al botón, ni la línea del botón ni la del panel; arriba, el
    // brillo sigue de uno al otro.
    let ppp = response.ctx.pixels_per_point();
    let painter = response.ctx.layer_painter(inner.response.layer_id);
    let (x, top, bottom) = (slot.right() - 1.0, slot.top(), slot.bottom());
    let fill = Rect::from_min_max(pos2(x, top + 2.0), pos2(x + 2.0, bottom - 2.0));
    // Arriba, la línea sigue del botón al panel (sus esquinas redondeadas dejaban un hueco).
    let line = Rect::from_min_max(pos2(x - 2.0, top), pos2(x + 3.0, top + 1.0));
    let shine = Rect::from_min_max(pos2(x, top + 1.0), pos2(x + 2.0, top + 2.0));
    let bevel = Rect::from_min_max(pos2(x, bottom - 2.0), pos2(x + 1.0, bottom - 1.0));
    painter.extend([
        solid(line, look.colors.control_border, ppp),
        solid(fill, look.colors.panel, ppp),
        solid(shine, look.colors.menu_highlight, ppp),
        solid(bevel, look.colors.background, ppp),
    ]);
    Some(inner.inner)
}

/// Una herramienta en el menú de sus variantes: su ícono (`icon`, de 17), su nombre y su atajo,
/// en una fila de su alto. `selected`: la que está elegida (su nombre en el color de lo elegido).
pub(crate) fn tool_row(
    ui: &mut Ui,
    icon: Option<&crate::skin::Piece>,
    label: &str,
    shortcut: &str,
    selected: bool,
) -> Response {
    let Some(look) = look(ui) else {
        return ui.add(egui::Button::selectable(selected, label).shortcut_text(shortcut));
    };
    let colors = &look.colors;
    let color = if selected {
        colors.timeline_selection_dark
    } else {
        colors.text
    };
    let text = galley(ui, label, color);
    let parts = shortcut_parts(shortcut);
    let icon_w = icon.map_or(0.0, |p| p.size[0] as f32 + TOOL_ICON_GAP);
    let width = (icon_w + text.size().x + SHORTCUT_GAP + parts_width(ui, &parts)).ceil();
    let (rect, response) = ui.allocate_at_least(vec2(width, TOOL_ROW_HEIGHT), Sense::click());
    let enabled = ui.is_enabled();
    // El nombre accesible, como el de los ítems: el nombre y el atajo.
    let a11y = format!("{label} {shortcut}");
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, selected, &a11y));
    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let mut shapes = Vec::new();
        if enabled && response.hovered() {
            shapes.push(row_highlight(&look, rect, ppp));
        }
        if let Some(piece) = icon {
            let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
            let at = pos2(rect.left(), (rect.center().y - size.y / 2.0).floor());
            shapes.extend(pixel_ui::piece_shapes(
                piece,
                Rect::from_min_size(at, size),
                ppp,
            ));
        }
        let caps = (rect.center().y - CAP_HEIGHT / 2.0).floor();
        let at = tab_bar::text_pos(&text, rect.left() + icon_w, caps, ppp);
        shapes.push(Shape::galley(at, text, color));
        shapes.extend(parts_shapes(
            ui,
            &look,
            &parts,
            rect.right(),
            caps,
            enabled,
            ppp,
        ));
        row_painter(ui, &look).extend(shapes);
    }
    response
}

/// Las filas del menú de herramientas: del alto del ícono (17) y 2 de aire arriba y abajo; el
/// texto, 4 después del ícono.
const TOOL_ROW_HEIGHT: f32 = 21.0;
const TOOL_ICON_GAP: f32 = 4.0;

/// Una opción de una lista, como `Button::selectable(selected, label)`.
pub(crate) struct Choice<'a> {
    selected: bool,
    label: &'a str,
}

/// Una opción de una lista: en un panel pixel, una fila como las de los menús, en `selection`
/// si es la elegida (o tiene el puntero encima); si no, la opción de egui.
pub(crate) fn choice(selected: bool, label: &str) -> Choice<'_> {
    Choice { selected, label }
}

impl Widget for Choice<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Some(look) = look(ui) else {
            return egui::Button::selectable(self.selected, self.label).ui(ui);
        };
        let enabled = ui.is_enabled();
        let text_color = if enabled {
            look.colors.text
        } else {
            look.colors.text_dim
        };
        let label = galley(ui, &shown(self.label), text_color);
        let selected = self.selected;
        // A la izquierda, el radio (prendido en la elegida), como la casilla de `Check`.
        let radio = if selected {
            &look.radio_on
        } else {
            &look.radio
        };
        let radio = radio.clone().or_else(|| look.radio.clone());
        let indent = radio.as_ref().map_or(0.0, |p| p.size[0] as f32 + CHECK_GAP);
        let (rect, response) = row(ui, &label, indent, self.label);
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, self.label)
        });
        if ui.is_rect_visible(rect) {
            let ppp = ui.ctx().pixels_per_point();
            let mut shapes = row_shapes(ui, &look, rect, &label, text_color, &response);
            // El texto, después del radio.
            if let Some(Shape::Text(text)) = shapes.last_mut() {
                text.pos.x += indent;
            }
            if selected && !response.hovered() {
                shapes.insert(0, row_highlight(&look, rect, ppp));
            }
            if let Some(piece) = &radio {
                let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
                let caps = rect.top() + ROW_CAPS_TOP;
                // Centrado con las mayúsculas (como la casilla).
                let top = (caps + (CAP_HEIGHT - size.y) / 2.0).floor();
                let at = Rect::from_min_size(pos2(rect.left(), top), size);
                shapes.extend(pixel_ui::piece_shapes(piece, at, ppp));
            }
            row_painter(ui, &look).extend(shapes);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_split_into_caps_pluses_and_the_key() {
        use KeyPart::*;
        assert_eq!(shortcut_parts("Ctrl+N"), vec![Cap("Ctrl"), Plus, Key("N")]);
        assert_eq!(
            shortcut_parts("Ctrl+Shift+S"),
            vec![Cap("Ctrl"), Plus, Cap("Shift"), Plus, Key("S")]
        );
        assert_eq!(shortcut_parts("Del"), vec![Cap("Del")]);
        assert_eq!(shortcut_parts("+"), vec![Key("+")]);
        assert_eq!(shortcut_parts("-"), vec![Key("-")]);
        assert_eq!(shortcut_parts("Ctrl++"), vec![Cap("Ctrl"), Plus, Key("+")]);
        assert_eq!(shortcut_parts("Ctrl+0"), vec![Cap("Ctrl"), Plus, Key("0")]);
        assert_eq!(shortcut_parts(""), vec![]);
    }
}
