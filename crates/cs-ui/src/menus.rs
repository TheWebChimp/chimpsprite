//! Los menús de la barra con Pixel UI, como en el diseño (`Design.aseprite`, frame 3, capa
//! "Layer 4"):
//!
//! - **Títulos:** subrayan su primera letra (la app no tiene mnemónicos). El título abierto va en
//!   una caja redondeada arriba (`menu.title.open`) que se une al panel sin línea entre los dos.
//! - **Panel:** `menu.panel` en 9-slice, colgado de la caja: mismo borde izquierdo, y su borde de
//!   arriba empieza donde termina la caja.
//! - **Ítems:** filas de 13 px con el texto a la izquierda y el atajo a la derecha. Ctrl y Shift
//!   son las teclas dibujadas (`key.ctrl`, `key.shift`); las demás con nombre (Alt, Del…) van en `key`
//!   estirada con el nombre en la fuente pixel. Entre tecla y tecla, un + de 5×5 hecho con rects,
//!   y al final la tecla en el color tenue (`ruler_tick`, el del diseño).
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
    Sense, Shape, Style, TextStyle, Ui, UiBuilder, UiKind, UiStackInfo, Widget, WidgetInfo,
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
const KEY_CTRL: &str = "key.ctrl";
const KEY_SHIFT: &str = "key.shift";
const KEY: &str = "key";
const CHECK: &str = "menu.check";
const CHECK_ON: &str = "menu.check.on";

/// La caja del título abierto: empieza 5 px antes del texto (línea, bisel y 3 de relleno) y 5
/// filas arriba de las mayúsculas, y termina 4 px después del texto (la fuente deja 1 vacío).
const BOX_LEFT: f32 = 5.0;
const BOX_RIGHT: f32 = 4.0;
const BOX_TOP: f32 = 5.0;
/// El borde de arriba del panel, 11 filas debajo de las mayúsculas del título. La caja baja 2
/// filas más y tapa ese borde y el bisel.
const PANEL_TOP: f32 = 11.0;
const BOX_JOIN: f32 = 2.0;
/// Del borde del panel al contenido: la línea y el bisel arriba y abajo; a los lados, además, 3
/// px de relleno (el texto de los ítems queda 5 px adentro, como en el diseño).
const PANEL_MARGIN: Margin = Margin {
    left: 5,
    right: 5,
    top: 2,
    bottom: 2,
};
/// Lo que la fila con el puntero encima se extiende a cada lado, hasta los biseles.
const ROW_BLEED: f32 = 3.0;
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
/// Las teclas con nombre en la fuente pixel: 1 fila arriba y 1 abajo de las mayúsculas, y 2 px
/// de cada lado de la tinta.
const KEY_PAD_X: f32 = 2.0;
const KEY_PAD_Y: f32 = 1.0;
/// La casilla: 2 filas arriba de las mayúsculas, del tamaño de la pieza (la de respaldo, la del
/// diseño) y con 3 px de aire hasta el texto.
const CHECK_TOP: f32 = 2.0;
const FALLBACK_CHECK_SIZE: egui::Vec2 = vec2(11.0, 12.0);
const CHECK_GAP: f32 = 3.0;
/// La flecha de los submenús: un triángulo de 3×5 a 1 fila de las mayúsculas.
const ARROW_TOP: f32 = 1.0;

/// Lo que dibujan los menús con Pixel UI: las piezas del skin y sus colores, más dónde están las
/// mayúsculas de la barra. Viaja en la pila de `Ui`s de cada panel.
#[derive(Clone)]
pub(crate) struct MenuLook {
    title_open: Option<Piece>,
    panel: Option<Piece>,
    key_ctrl: Option<Piece>,
    key_shift: Option<Piece>,
    key: Option<Piece>,
    check: Option<Piece>,
    check_on: Option<Piece>,
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
            key_ctrl: piece(KEY_CTRL),
            key_shift: piece(KEY_SHIFT),
            key: piece(KEY),
            check: piece(CHECK),
            check_on: piece(CHECK_ON),
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
    // izquierda (`tab_bar::MENU_CAPS_TOP` y `tab_bar::MENU_LEFT`): la de File empieza en la
    // esquina de la ventana.
    let panel_top = look.caps_top + PANEL_TOP;
    let title_box = Rect::from_min_max(
        pos2(text_left - BOX_LEFT, look.caps_top - BOX_TOP),
        pos2(text_left + text.size().x + BOX_RIGHT, panel_top + BOX_JOIN),
    );
    let config = menu::MenuConfig::new().style(pixel_menu_style);
    // El panel cuelga de la caja: su esquina de arriba a la izquierda, siempre la misma.
    let anchor = Rect::from_min_max(
        pos2(title_box.left(), panel_top),
        pos2(title_box.right(), panel_top),
    );
    let inner = Popup::menu(&response)
        .anchor(anchor)
        .align(RectAlign::BOTTOM_START)
        .align_alternatives(&[])
        .gap(0.0)
        .frame(Frame::NONE.inner_margin(PANEL_MARGIN))
        .style(pixel_menu_style)
        .info(
            UiStackInfo::new(UiKind::Menu)
                .with_tag_value(menu::MenuConfig::MENU_CONFIG_TAG, config),
        )
        .show(|ui| panel(ui, look, Some(title_box), content));
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
        // Las filas de la caja que bajan sobre el panel; las de arriba van en la barra.
        ui.painter()
            .with_clip_rect(rect)
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
    shortcut: Option<&'a str>,
}

/// Un ítem con atajo, como `Button::new(label).shortcut_text(shortcut)`.
pub(crate) fn item<'a>(label: &'a str, shortcut: &'a str) -> Item<'a> {
    Item {
        label,
        shortcut: Some(shortcut),
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
                Some(keys) => button.shortcut_text(keys).ui(ui),
                None => button.ui(ui),
            };
        };
        let keys = self.shortcut.unwrap_or_default();
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
        let keys_width = parts_width(ui, &look, &parts);
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
            if enabled && response.hovered() {
                shapes.push(row_highlight(&look, rect, ppp));
            }
            let at = Rect::from_min_size(pos2(rect.left(), caps - CHECK_TOP), box_size);
            shapes.extend(check_shapes(&look, at, on, enabled, ppp));
            let pos = tab_bar::text_pos(&text, rect.left() + indent, caps, ppp);
            shapes.push(Shape::galley(pos, text, text_color));
            row_painter(ui, &look).extend(shapes);
        }
        response
    }
}

/// La casilla en `rect`: `menu.check.on` marcada y `menu.check` sin marcar. Sin la pieza, un
/// cuadro plano (`panel` con borde `outline`) y, si está marcada, un cuadrito `text` adentro.
/// Deshabilitada, más clara.
fn check_shapes(look: &MenuLook, rect: Rect, on: bool, enabled: bool, ppp: f32) -> Vec<Shape> {
    let piece = if on { &look.check_on } else { &look.check };
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
    solid(
        rect.expand2(vec2(ROW_BLEED, 0.0)),
        look.colors.selection,
        ppp,
    )
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

/// La pieza dibujada de la tecla `name`, si el skin la trae: Ctrl y Shift.
fn drawn_key<'a>(look: &'a MenuLook, name: &str) -> Option<&'a Piece> {
    match name {
        "Ctrl" => look.key_ctrl.as_ref(),
        "Shift" => look.key_shift.as_ref(),
        _ => None,
    }
}

/// El ancho de una tecla dibujada: la pieza de Ctrl o Shift tal cual; las demás, el nombre con
/// 2 px de cada lado de la tinta.
fn cap_width(ui: &Ui, look: &MenuLook, name: &str) -> f32 {
    if let Some(piece) = drawn_key(look, name) {
        return piece.size[0] as f32;
    }
    ink_width(&galley(ui, name, Color32::PLACEHOLDER)) + 2.0 * KEY_PAD_X
}

/// La tinta de un texto de la fuente pixel: su ancho menos el píxel vacío del final.
pub(crate) fn ink_width(text: &Galley) -> f32 {
    (text.size().x - 1.0).max(0.0).round()
}

fn part_width(ui: &Ui, look: &MenuLook, part: &KeyPart<'_>) -> f32 {
    match part {
        KeyPart::Cap(name) => cap_width(ui, look, name),
        KeyPart::Plus => KEY_GAP + PLUS_SIZE + KEY_GAP,
        KeyPart::Key(key) => ink_width(&galley(ui, key, Color32::PLACEHOLDER)),
    }
}

fn parts_width(ui: &Ui, look: &MenuLook, parts: &[KeyPart<'_>]) -> f32 {
    parts.iter().map(|p| part_width(ui, look, p)).sum()
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
    let mut x = right - parts_width(ui, look, parts);
    let mut shapes = Vec::new();
    for part in parts {
        let width = part_width(ui, look, part);
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

/// Una tecla dibujada en `left`: Ctrl o Shift, la pieza tal cual a la altura de las mayúsculas; las
/// demás, `key` estirada 1 fila más arriba y abajo con el nombre en el color del panel (como las
/// letras caladas de Ctrl).
fn cap_shapes(
    ui: &Ui,
    look: &MenuLook,
    name: &str,
    left: f32,
    caps: f32,
    tint: Color32,
    ppp: f32,
) -> Vec<Shape> {
    if let Some(piece) = drawn_key(look, name) {
        let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
        let rect = Rect::from_min_size(pos2(left, caps), size);
        return tinted(pixel_ui::piece_shapes(piece, rect, ppp), tint);
    }
    let text = galley(ui, name, look.colors.panel);
    let width = ink_width(&text) + 2.0 * KEY_PAD_X;
    let rect = Rect::from_min_size(
        pos2(left, caps - KEY_PAD_Y),
        vec2(width, CAP_HEIGHT + 2.0 * KEY_PAD_Y),
    );
    let mut shapes = match &look.key {
        Some(piece) => tinted(pixel_ui::piece_shapes(piece, rect, ppp), tint),
        None => vec![solid(rect, look.colors.ruler_tick, ppp)],
    };
    let at = tab_bar::text_pos(&text, left + KEY_PAD_X, caps, ppp);
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
    let open = menu::SubMenu::new()
        .config(config)
        .show(ui, &response, |ui| panel(ui, &look, None, content));
    if enabled && (open.is_some() || response.hovered()) {
        painter.set(back, row_highlight(&look, rect, ppp));
    }
    InnerResponse::new(open.map(|i| i.inner), response)
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
    let min_width = anchor.width() - PANEL_MARGIN.sum().x;
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
        let (rect, response) = row(ui, &label, 0.0, self.label);
        let selected = self.selected;
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, self.label)
        });
        if ui.is_rect_visible(rect) {
            let ppp = ui.ctx().pixels_per_point();
            let mut shapes = row_shapes(ui, &look, rect, &label, text_color, &response);
            if selected && !response.hovered() {
                shapes.insert(0, row_highlight(&look, rect, ppp));
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
