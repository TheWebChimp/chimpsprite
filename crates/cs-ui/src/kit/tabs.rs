//! Las pestañas (paso 4 del UI kit), con las medidas del mockup de componentes (frame 9), para la
//! barra de pestañas de documentos y las secciones del panel derecho:
//!
//! - `tab` (café) y
//!   `tab.active` (clara y abierta abajo), sobre la línea grabada, que se corta debajo de la
//!   activa; a cada lado de la activa, el tramado `tab.shade.left` / `tab.shade.right` en la fila
//!   oscura de la línea. Con el puntero encima, `tab.hover` (el relleno más claro). Los textos
//!   van 6 px adentro (el borde de 2 y 4 de aire) con las mayúsculas en la fila 6; la marca del
//!   final (○ con cambios, × para cerrar), 4 px después de la tinta y 4 antes del borde de 2.
//!   Entre pestañas, 8 px (las marcas azules del mockup).
//!
//! Si no entran, se achican parejo (las más anchas primero) hasta tres letras y "..."; si ni así
//! entran, se recorren con las flechas `tab.arrow.prev` / `tab.arrow.next` (12×12, juntas 23 de
//! ancho, 8 px después de la última que se ve) o con la rueda.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CursorIcon, Galley, Id, Rangef, Rect, Sense, Shape, Stroke, StrokeKind,
    TextStyle, Ui, Vec2, WidgetInfo, WidgetType, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::pixel_ui;
use crate::pixel_ui::Tip as _;
use crate::skin::{Skin, WidgetState};
use crate::tab_bar::{ellipsis, fit_widths, text_pos};
use crate::tabs::TabIcon;

/// Las piezas.
pub const TAB: &str = "tab";
pub const SHADE_LEFT: &str = "tab.shade.left";
pub const SHADE_RIGHT: &str = "tab.shade.right";
pub const ARROW_PREV: &str = "tab.arrow.prev";
pub const ARROW_NEXT: &str = "tab.arrow.next";
pub const DIRTY_MARK: &str = "icon.tab_dirty";
pub const CLOSE_MARK: &str = "icon.tab_close";

/// Entre una pestaña y la siguiente (y entre la última y las flechas).
pub const GAP: f32 = 8.0;
/// Las pestañas de documento: 15 de alto; la activa, 2 más (tapa la línea grabada).
pub const HEIGHT: f32 = 15.0;
pub const ACTIVE_HEIGHT: f32 = HEIGHT + pixel_ui::ETCHED_SIZE;
/// El borde (la línea y la de adentro) y el aire hasta la tinta, a cada lado.
pub const EDGE: f32 = 2.0;
pub const PAD: f32 = 4.0;
/// Las mayúsculas, en la fila 6 de la pestaña.
pub const CAPS_TOP: f32 = 6.0;
/// La marca del final: 5×5, en la fila 7, 4 px después de la tinta.
pub const MARK_SIZE: f32 = 5.0;
pub const MARK_TOP: f32 = 7.0;
pub const MARK_GAP: f32 = 4.0;
/// Lo que la zona de clic de la marca crece a cada lado.
const MARK_HIT: f32 = 2.0;
/// El cuadrito de color (de los slices) y su aire hasta el nombre.
pub const SWATCH: f32 = 7.0;
pub const SWATCH_GAP: f32 = 3.0;
/// El ícono de una pestaña de vista (▣ o Aa) y su aire hasta el nombre (`SWATCH_GAP`). Si el skin
/// trae `tab.icon.model` / `tab.icon.font`, se usan en su lugar.
pub const ICON_WIDTH: f32 = 10.0;
pub const ICON_MODEL: &str = "tab.icon.model";
pub const ICON_FONT: &str = "tab.icon.font";
/// Las flechas: dos cajas de 12×12 que comparten la columna del medio, 1 fila debajo del borde
/// de arriba de las pestañas.
pub const ARROW_SIZE: f32 = 12.0;
pub const ARROWS_WIDTH: f32 = 2.0 * ARROW_SIZE - 1.0;
pub const ARROWS_TOP: f32 = 1.0;
/// El alto de una pestaña: la activa tapa la línea grabada.
fn height(active: bool) -> f32 {
    if active { ACTIVE_HEIGHT } else { HEIGHT }
}

/// La marca del final de una pestaña de documento.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabMark {
    /// Sin marca (no se cierra).
    None,
    /// × para cerrar.
    Close,
    /// ○ (cambios sin guardar), que pasa a × con el puntero encima.
    Dirty,
}

/// Una pestaña: su nombre, el cuadrito de color (si tiene) y su marca.
#[derive(Clone, Debug)]
pub struct TabItem {
    pub label: String,
    pub color: Option<Color32>,
    pub mark: TabMark,
    pub icon: TabIcon,
}

impl TabItem {
    pub fn new(label: impl Into<String>) -> Self {
        TabItem {
            label: label.into(),
            color: None,
            mark: TabMark::None,
            icon: TabIcon::None,
        }
    }

    /// El ícono de vista antes del nombre.
    pub fn with_icon(self, icon: TabIcon) -> Self {
        TabItem { icon, ..self }
    }

    pub fn closable(label: impl Into<String>, dirty: bool) -> Self {
        TabItem {
            mark: if dirty {
                TabMark::Dirty
            } else {
                TabMark::Close
            },
            ..TabItem::new(label)
        }
    }

    pub fn colored(label: impl Into<String>, color: Color32) -> Self {
        TabItem {
            color: Some(color),
            ..TabItem::new(label)
        }
    }
}

/// Lo que se pidió en la tira.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabEvent {
    Select(usize),
    Close(usize),
}

/// Dónde va la tira: las pestañas de `left` a `right` desde la fila `top`; la línea grabada (en
/// el estilo de documento) de `line.min` a `line.max`, en las dos filas de debajo de las
/// pestañas.
#[derive(Clone, Copy, Debug)]
pub struct Strip {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub line: Rangef,
    /// Si no entran ni achicadas hasta tres letras: con `true`, flechas para recorrerlas; con
    /// `false` (la barra de documentos), se achican todas por igual hasta caber (sin marca si no
    /// le entra).
    pub scroll: bool,
}

impl Strip {
    pub fn new(left: f32, right: f32, top: f32) -> Self {
        Strip {
            left,
            right,
            top,
            line: Rangef::new(left, right),
            scroll: true,
        }
    }

    /// La fila de arriba de la línea grabada (o del borde del panel).
    pub fn bottom(&self) -> f32 {
        self.top + HEIGHT
    }
}

fn font(ui: &Ui) -> egui::FontId {
    TextStyle::Body.resolve(ui.style())
}

fn galley(ui: &Ui, text: &str) -> Arc<Galley> {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font(ui), Color32::PLACEHOLDER)
}

/// El ancho de la tinta de `text` (con la fuente pixel, el texto sin la columna de aire del
/// final).
fn ink(ui: &Ui, text: &str) -> f32 {
    crate::menus::ink_width(&galley(ui, text))
}

/// `name` entero si su tinta mide hasta `room`; si no, lo más largo que entre seguido de
/// `ellipsis()` (o nada si ni eso entra). Medido con la tinta, como los anchos de las pestañas.
fn fit_name(ui: &Ui, name: &str, room: f32) -> String {
    if ink(ui, name) <= room {
        return name.to_owned();
    }
    let chars: Vec<char> = name.chars().collect();
    let cut = |k: usize| chars[..k].iter().collect::<String>() + ellipsis();
    let fits = |k: usize| ink(ui, &cut(k)) <= room;
    if !fits(0) {
        return String::new();
    }
    let (mut lo, mut hi) = (0, chars.len());
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

/// Lo que ocupa una pestaña además de la tinta de su nombre: los bordes, el aire, el cuadrito y
/// la marca.
pub fn chrome(item: &TabItem) -> f32 {
    let swatch = if item.color.is_some() {
        SWATCH + SWATCH_GAP
    } else {
        0.0
    };
    let icon = if item.icon == TabIcon::None {
        0.0
    } else {
        ICON_WIDTH + SWATCH_GAP
    };
    let mark = if item.mark == TabMark::None {
        0.0
    } else {
        MARK_GAP + MARK_SIZE
    };
    2.0 * (EDGE + PAD) + swatch + icon + mark
}

/// El ancho justo de una pestaña.
pub fn natural_width(ui: &Ui, item: &TabItem) -> f32 {
    chrome(item) + ink(ui, &item.label)
}

/// Una pestaña acomodada: su rect (sin lo que la activa baja) y el nombre como entra.
#[derive(Clone, Debug)]
pub struct Placed {
    pub index: usize,
    pub rect: Rect,
    pub name: String,
}

/// Cómo quedan las pestañas: las que se ven y, si no entran todas, las flechas.
#[derive(Clone, Debug)]
pub struct Layout {
    pub placed: Vec<Placed>,
    pub arrows: Option<Rect>,
    pub offset: usize,
    pub last_offset: usize,
}

/// Acomoda `items` en la tira desde `offset`: achica parejo las más anchas hasta tres letras y
/// "..." y, si ni así entran, deja lugar a la derecha para las flechas.
pub fn layout(ui: &Ui, strip: &Strip, items: &[TabItem], offset: usize) -> Layout {
    let gap = GAP;
    let n = items.len();
    let natural: Vec<f32> = items.iter().map(|t| natural_width(ui, t)).collect();
    let room_all = (strip.right - strip.left - gap * n.saturating_sub(1) as f32)
        .floor()
        .max(0.0);
    // Sin flechas, el mínimo es "..." (y si ni así entran, todas iguales); con flechas, tres
    // letras y "...".
    let short = if strip.scroll {
        ink(ui, &format!("Abc{}", ellipsis()))
    } else {
        ink(ui, ellipsis())
    };
    let mins: Vec<f32> = items
        .iter()
        .zip(&natural)
        .map(|(t, &w)| w.min(chrome(t) + short))
        .collect();
    let min = mins.iter().copied().fold(0.0, f32::max);
    let fitted = fit_widths(&natural, room_all, min);
    let widths: Vec<f32> = if strip.scroll {
        fitted
            .into_iter()
            .zip(&mins)
            .map(|(w, &m)| w.max(m))
            .collect()
    } else {
        fitted
    };
    let overflow = strip.scroll
        && widths.iter().sum::<f32>() + gap * n.saturating_sub(1) as f32 > strip.right - strip.left;
    let arrows = overflow.then(|| {
        Rect::from_min_size(
            pos2(strip.right - ARROWS_WIDTH, strip.top + ARROWS_TOP),
            vec2(ARROWS_WIDTH, ARROW_SIZE),
        )
    });
    let right = arrows.map_or(strip.right, |a| a.left() - GAP);
    let fits = |from: usize, to: usize| {
        let used: f32 = widths[from..=to].iter().sum::<f32>() + gap * (to - from) as f32;
        strip.left + used <= right
    };
    let last_offset = if n == 0 {
        0
    } else {
        (0..n).find(|&o| fits(o, n - 1)).unwrap_or(n - 1)
    };
    let offset = if overflow { offset.min(last_offset) } else { 0 };
    let mut placed = Vec::new();
    let mut x = strip.left;
    for i in offset..n {
        if i > offset && x + widths[i] > right {
            break;
        }
        let rect = Rect::from_min_size(pos2(x, strip.top), vec2(widths[i], HEIGHT));
        let room = (widths[i] - chrome(&items[i])).max(0.0);
        let name = fit_name(ui, &items[i].label, room);
        placed.push(Placed {
            index: i,
            rect,
            name,
        });
        x += widths[i] + gap;
    }
    Layout {
        placed,
        arrows,
        offset,
        last_offset,
    }
}

/// Cómo se ve una pestaña.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabLook {
    pub active: bool,
    pub hovered: bool,
    /// Con el puntero sobre su marca (la ○ pasa a ×).
    pub mark_hovered: bool,
}

/// Una pestaña dibujada en `rect` (el de su fila, sin lo que la activa baja).
pub fn tab_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    item: &TabItem,
    name: &str,
    look: TabLook,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let drawn = rect.with_max_y(rect.top() + height(look.active));
    let state = if look.active {
        WidgetState::Active
    } else if look.hovered {
        WidgetState::Hover
    } else {
        WidgetState::Normal
    };
    let mut shapes = match skin.piece(TAB, state) {
        Some(piece) => pixel_ui::piece_shapes(piece, drawn, ppp),
        None => flat_tab(drawn, look.active, &colors, ppp),
    };
    let mut left = rect.left() + EDGE + PAD;
    let caps = rect.top() + CAPS_TOP;
    if let Some(color) = item.color {
        let swatch = Rect::from_min_size(pos2(left, caps), Vec2::splat(SWATCH));
        shapes.push(Shape::rect_filled(swatch, 0.0, colors.field_text));
        shapes.push(Shape::rect_filled(swatch.shrink(1.0), 0.0, color));
        left += SWATCH + SWATCH_GAP;
    }
    let color = if look.active {
        colors.text
    } else {
        colors.tab_text_dim
    };
    if item.icon != TabIcon::None {
        shapes.extend(icon_shapes(ui, skin, item.icon, pos2(left, caps), color));
        left += ICON_WIDTH + SWATCH_GAP;
    }
    // El nombre ya viene cortado para que entre (`layout`).
    let text = galley(ui, name);
    let at = text_pos(&text, left, caps, ppp);
    shapes.push(Shape::galley(at, text, color));
    if item.mark != TabMark::None {
        let mark = mark_rect(rect);
        let dot = item.mark == TabMark::Dirty && !look.mark_hovered;
        let name = if dot { DIRTY_MARK } else { CLOSE_MARK };
        match skin.piece(name, WidgetState::Normal) {
            Some(piece) => shapes.extend(pixel_ui::piece_shapes(piece, mark, ppp)),
            None => shapes.extend(flat_mark(mark, dot, colors.text_dim, ppp)),
        }
    }
    shapes
}

/// El ícono de una pestaña de vista con las mayúsculas en la fila `at.y`: la pieza del skin si la
/// trae; si no, un cuadrito con otro adentro (modelo) o "Aa" (fuente).
fn icon_shapes(ui: &Ui, skin: &Skin, icon: TabIcon, at: egui::Pos2, color: Color32) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let name = match icon {
        TabIcon::Model => ICON_MODEL,
        TabIcon::Font => ICON_FONT,
        TabIcon::None => return Vec::new(),
    };
    if let Some(piece) = skin.piece(name, WidgetState::Normal) {
        let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
        return pixel_ui::piece_shapes(piece, Rect::from_min_size(at, size), ppp);
    }
    if icon == TabIcon::Font {
        let text = galley(ui, "Aa");
        let pos = text_pos(&text, at.x, at.y, ppp);
        return vec![Shape::galley(pos, text, color)];
    }
    // ▣: 7×7 con borde de 1 y un cuadrito de 3×3 al centro.
    let outer = Rect::from_min_size(at, Vec2::splat(7.0));
    vec![
        Shape::rect_stroke(
            outer.round_to_pixels(ppp),
            0.0,
            Stroke::new(1.0, color),
            StrokeKind::Inside,
        ),
        Shape::rect_filled(outer.shrink(2.0).round_to_pixels(ppp), 0.0, color),
    ]
}

/// La marca del final de una pestaña de documento en `rect`.
pub fn mark_rect(rect: Rect) -> Rect {
    Rect::from_min_size(
        pos2(rect.right() - EDGE - PAD - MARK_SIZE, rect.top() + MARK_TOP),
        Vec2::splat(MARK_SIZE),
    )
}

/// La línea grabada de la tira (estilo documento), cortada debajo de `active` (el rect de la
/// activa, si se ve), con el tramado a sus lados.
pub fn line_shapes(ui: &Ui, skin: &Skin, strip: &Strip, active: Option<Rect>) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let top = strip.bottom();
    let spans = match active {
        Some(r) => vec![
            Rangef::new(strip.line.min, r.left()),
            Rangef::new(r.right(), strip.line.max),
        ],
        None => vec![strip.line],
    };
    let mut shapes: Vec<Shape> = spans
        .into_iter()
        .filter(|s| s.span() > 0.0)
        .map(|s| pixel_ui::etched_hline(s, top, &colors, ppp))
        .collect();
    if let Some(r) = active {
        for (name, left) in [(SHADE_LEFT, None), (SHADE_RIGHT, Some(r.right()))] {
            if let Some(piece) = skin.piece(name, WidgetState::Normal) {
                let w = piece.size[0] as f32;
                let x = left.unwrap_or(r.left() - w);
                let rect = Rect::from_min_size(pos2(x, top), vec2(w, piece.size[1] as f32));
                let clip = Rect::from_x_y_ranges(strip.line, top..=top + 1.0);
                let rect = rect.intersect(clip);
                if rect.width() > 0.0 {
                    shapes.extend(pixel_ui::piece_shapes(piece, rect, ppp));
                }
            }
        }
    }
    shapes
}

/// Las flechas en `rect` (de [`ARROWS_WIDTH`]): la de atrás y la de adelante, cada una con su
/// estado (`hovered`, `enabled`).
pub fn arrows_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    prev: (bool, bool),
    next: (bool, bool),
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let mut shapes = Vec::new();
    for (name, at, (hovered, enabled), back) in [
        (ARROW_PREV, rect.left(), prev, true),
        (ARROW_NEXT, rect.left() + ARROW_SIZE - 1.0, next, false),
    ] {
        let state = if !enabled {
            WidgetState::Disabled
        } else if hovered {
            WidgetState::Hover
        } else {
            WidgetState::Normal
        };
        let r = Rect::from_min_size(pos2(at, rect.top()), Vec2::splat(ARROW_SIZE));
        match skin.piece(name, state) {
            Some(piece) => shapes.extend(pixel_ui::piece_shapes(piece, r, ppp)),
            None => {
                shapes.push(Shape::rect_stroke(
                    r.round_to_pixels(ppp),
                    0.0,
                    Stroke::new(1.0, colors.outline),
                    StrokeKind::Inside,
                ));
                let ink = if enabled {
                    colors.text
                } else {
                    colors.tab_text_dim
                };
                shapes.extend(arrow_glyph(r, back, ink, ppp));
            }
        }
    }
    shapes
}

/// Respaldo de la flecha (sin las piezas): un triángulo de 3×5 al centro.
fn arrow_glyph(rect: Rect, back: bool, color: Color32, ppp: f32) -> Vec<Shape> {
    let c = rect.center().round();
    (0..3)
        .map(|i| {
            let i = i as f32;
            let x = if back { c.x - 1.0 + i } else { c.x + 1.0 - i };
            Shape::rect_filled(
                Rect::from_x_y_ranges(x..=x + 1.0, c.y - i - 1.0 + 1.0..=c.y + i + 1.0)
                    .round_to_pixels(ppp),
                0.0,
                color,
            )
        })
        .collect()
}

/// La tira entera: las pestañas, la línea grabada (estilo documento) y, si hace falta, las
/// flechas. `id` guarda desde cuál se ven. Devuelve lo que se pidió.
pub fn tab_strip(
    ui: &mut Ui,
    skin: &Skin,
    id: Id,
    strip: Strip,
    items: &[TabItem],
    active: Option<usize>,
) -> Option<TabEvent> {
    let n = items.len();
    let memory = id.with("tabs_offset");
    let (mut offset, seen): (usize, Option<usize>) =
        ui.data(|d| d.get_temp(memory)).unwrap_or((0, None));
    let mut lay = layout(ui, &strip, items, offset);
    // La activa nueva, a la vista.
    if lay.arrows.is_some()
        && let Some(a) = active.filter(|&a| seen != Some(a) && a < n)
    {
        if a < lay.offset {
            lay = layout(ui, &strip, items, a);
        }
        while lay.placed.last().is_some_and(|p| p.index < a) && lay.offset < lay.last_offset {
            lay = layout(ui, &strip, items, lay.offset + 1);
        }
    }
    // La rueda sobre la fila las recorre.
    if lay.arrows.is_some() {
        let row = Rect::from_x_y_ranges(strip.left..=strip.right, strip.top..=strip.top + HEIGHT);
        if ui.rect_contains_pointer(row) {
            let scroll: f32 = ui.input(|i| {
                i.raw
                    .events
                    .iter()
                    .map(|e| match e {
                        egui::Event::MouseWheel { delta, .. } => delta.y + delta.x,
                        _ => 0.0,
                    })
                    .sum()
            });
            if scroll > 0.0 {
                lay = layout(ui, &strip, items, lay.offset.saturating_sub(1));
            } else if scroll < 0.0 {
                lay = layout(ui, &strip, items, lay.offset + 1);
            }
        }
    }
    offset = lay.offset;
    let enabled = ui.is_enabled();
    let active_rect = active
        .and_then(|a| lay.placed.iter().find(|p| p.index == a))
        .map(|p| p.rect.with_max_y(p.rect.top() + ACTIVE_HEIGHT));
    let mut shapes = Vec::new();
    shapes.extend(line_shapes(ui, skin, &strip, active_rect));
    let mut event = None;
    // La activa, al final: tapa a sus vecinas si se pisan.
    let mut order: Vec<&Placed> = lay.placed.iter().collect();
    order.sort_by_key(|p| Some(p.index) == active);
    for p in order {
        let item = &items[p.index];
        let is_active = active == Some(p.index);
        let rect = p.rect;
        let hit = rect.with_max_y(rect.top() + height(is_active));
        let response = ui.interact(hit, id.with(("tab", p.index)), Sense::click());
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::Button, enabled, is_active, &item.label)
        });
        // La marca va después de la pestaña: gana el clic. Si la pestaña es más angosta que
        // su marco, no hay marca ni zona de clic (le robarían el clic a la vecina).
        let close = (item.mark != TabMark::None && rect.width() >= chrome(item)).then(|| {
            let label = format!("Close {}", item.label);
            let close = ui
                .interact(
                    mark_rect(rect).expand(MARK_HIT),
                    id.with(("tab_close", p.index)),
                    Sense::click(),
                )
                .on_hover_cursor(CursorIcon::PointingHand)
                .tip("Close");
            close.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &label));
            close
        });
        let shown_item = if close.is_some() {
            item.clone()
        } else {
            TabItem {
                mark: TabMark::None,
                ..item.clone()
            }
        };
        let look = TabLook {
            active: is_active,
            hovered: !is_active && response.hovered(),
            mark_hovered: close.as_ref().is_some_and(|c| c.hovered()),
        };
        shapes.extend(tab_shapes(ui, skin, rect, &shown_item, &p.name, look));
        let closed = close.is_some_and(|c| c.clicked())
            || (item.mark != TabMark::None && response.middle_clicked());
        if closed {
            event = Some(TabEvent::Close(p.index));
        } else if response.clicked() {
            event = Some(TabEvent::Select(p.index));
        }
    }
    if let Some(arrows) = lay.arrows {
        let last_shown = lay.placed.last().map_or(0, |p| p.index);
        let mut states = [(false, false); 2];
        let halves = [
            Rect::from_min_size(arrows.min, Vec2::splat(ARROW_SIZE)),
            Rect::from_min_size(
                arrows.min + vec2(ARROW_SIZE - 1.0, 0.0),
                Vec2::splat(ARROW_SIZE),
            ),
        ];
        for (i, (rect, label)) in halves
            .into_iter()
            .zip(["Previous tabs", "Next tabs"])
            .enumerate()
        {
            let can = if i == 0 {
                offset > 0
            } else {
                last_shown + 1 < n
            };
            let response = ui.interact(rect, id.with(label), Sense::click());
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, can, label));
            states[i] = (response.hovered(), can && enabled);
            if can && response.clicked() {
                offset = if i == 0 {
                    offset.saturating_sub(1)
                } else {
                    (offset + 1).min(lay.last_offset)
                };
            }
        }
        shapes.extend(arrows_shapes(ui, skin, arrows, states[0], states[1]));
    }
    ui.data_mut(|d| d.insert_temp(memory, (offset, active)));
    let mut painter = ui.painter().clone();
    painter.set_opacity(1.0);
    painter.extend(shapes);
    event
}

/// Respaldo de la caja de una pestaña: el fondo (la activa) o `canvas` (las demás) con un borde
/// `outline`.
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
mod tests;
