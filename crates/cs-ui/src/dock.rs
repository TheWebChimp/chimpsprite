//! La columna de "otros paneles" entre el lienzo y el panel derecho (como el dock de Illustrator):
//! un botón por panel (Artboards, Slices, Library…) que lo abre flotando a su izquierda. Así esas secciones
//! no le quitan lugar a la paleta y al editor de color. Hasta abajo, el que prende y apaga el
//! timeline y arriba de él, el de la ventanita de preview.
//!
//! Fase D, tarea 6: hay una tira por orilla de la ventana ([`Edge`]). La de la derecha es la de
//! siempre (a la derecha del lienzo, con los botones del timeline y del Preview); las otras tres
//! van en su orilla, fuera del árbol del workspace, y solo se ven si tienen algo (ver
//! `workspace::show`). Sus flyouts salen hacia adentro: a la derecha de la tira de la izquierda,
//! debajo de la de arriba y encima de la de abajo. Si los botones no caben, los del final se van a
//! un botón "More Panels" que los lista (nunca se recortan ni se enciman).

use eframe::egui::{self, Rect, Sense, Shape, Ui, Vec2, vec2};

use crate::panel_registry::Body;
use crate::pixel_draw;
use crate::pixel_ui;
use crate::pixel_ui::Tip as _;
use crate::skin::{Skin, WidgetState};

/// Los botones del dock (mockup de componentes, frame 9): `button` del skin en 9-slice, con el
/// cuerpo de 23×23 y la línea clara debajo (24 en total, como en el mockup), uno cada 27.
pub const BUTTON_SIZE: Vec2 = vec2(23.0, 23.0);
const BUTTON_GAP: f32 = 4.0;
/// Arriba de los botones: el primero empieza a la altura de la caja del lienzo (sus reglas).
const TOP_AIR: f32 = 5.0;
/// Debajo del botón del timeline, hasta el borde de abajo del dock.
const BOTTOM_AIR: f32 = 5.0;
/// Ancho del dock: los botones y 3 de aire hasta el panel derecho.
pub const WIDTH: f32 = 26.0;
pub const RIGHT_MARGIN: i8 = 3;
/// Del borde de la caja del lienzo a los botones (x 754 → 758 en el diseño).
pub const LEFT_AIR: i8 = 3;
/// Ancho del contenido de los paneles flotantes.
pub(crate) const FLYOUT_WIDTH: f32 = 210.0;
/// Marco de los paneles flotantes (mockup de componentes, frame 9, la "T" junto a la columna):
/// el del menú de las herramientas (`menu.side`: borde (89,77,77), brillo arriba y a la derecha,
/// bisel a la izquierda y abajo), con la pestaña que envuelve al botón apretado; y su aire adentro.
const FLYOUT_PIECE: &str = "menu.side";
pub const FLYOUT_MARGIN: i8 = 8;
/// Entre el botón y su panel flotante (la pestaña los une).
const FLYOUT_GAP: f32 = 4.0;

/// La orilla de la ventana de una tira de íconos (ver `Workspace::strip_at`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// Las cuatro, en el orden de su lugar en `Workspace::strips` (ver [`Edge::index`]).
    pub(crate) const ALL: [Edge; 4] = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];

    /// Su lugar en [`Edge::ALL`].
    pub(crate) fn index(self) -> usize {
        match self {
            Edge::Left => 0,
            Edge::Right => 1,
            Edge::Top => 2,
            Edge::Bottom => 3,
        }
    }

    /// Si su tira va a lo alto (la de la izquierda y la de la derecha) o a lo ancho.
    pub(crate) fn vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

/// Dónde va un panel flotante: junto a su botón y dentro de la columna del dock (de la altura de
/// las reglas del lienzo al botón del timeline). En las tiras de arriba y de abajo, `column` es el
/// área del workspace (el flyout sale hacia adentro y no pasa de ella).
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub button: Rect,
    pub column: Rect,
    pub(crate) edge: Edge,
}

impl Anchor {
    /// Hasta dónde llega abajo el flyout (con su marco): el fondo de la columna o, en la tira de
    /// abajo, su botón.
    pub(crate) fn bottom(&self) -> f32 {
        match self.edge {
            Edge::Bottom => self.button.top() - FLYOUT_GAP,
            _ => self.column.bottom(),
        }
    }
}

/// Los paneles del dock: son los del registro (ver [`crate::panel_registry`]).
pub type DockPanel = crate::panel_registry::PanelId;

/// Lo que pidió el dock y la app resuelve (necesita más que el editor).
#[derive(Default)]
pub struct DockOutput {
    /// El panel abierto: cuál y dónde, para [`flyout`] (la app lo dibuja con su registro).
    pub app_panel: Option<(DockPanel, Anchor)>,
    /// Sacar un panel de la tira (lo resuelve la app con su workspace).
    pub request: Option<StripRequest>,
    /// El panel cuyo ícono se arrastra fuera de la tira y dónde está el puntero (para resaltar
    /// dónde caería).
    pub dragging: Option<(DockPanel, egui::Pos2)>,
    /// Los botones a la vista de los paneles de la tira, en su orden (sin "More Panels" ni los del
    /// timeline y el Preview): la app los guarda en su [`StripArea`].
    pub buttons: Vec<(DockPanel, Rect)>,
}

/// Una tira dibujada en el frame: su orilla, su rect y los botones a la vista de sus paneles (ver
/// [`DockOutput::buttons`]). Un ícono soltado sobre ella cae entre sus botones (ver
/// [`StripArea::drop_index`]) y, mientras se arrastra encima, se marca dónde (ver
/// [`StripArea::line`]).
#[derive(Clone, Debug)]
pub(crate) struct StripArea {
    pub edge: Edge,
    pub rect: Rect,
    pub buttons: Vec<(DockPanel, Rect)>,
}

impl StripArea {
    fn rects(&self) -> Vec<Rect> {
        self.buttons.iter().map(|(_, r)| *r).collect()
    }

    /// El lugar en la tira (contado sin `panel`, si ya estaba en ella) donde cae `panel` soltado
    /// con el puntero en `p`: el hueco entre sus botones de [`insert_index`]; más allá del último
    /// a la vista (o sobre "More Panels"), al final (`usize::MAX`).
    pub(crate) fn drop_index(&self, panel: DockPanel, p: egui::Pos2) -> usize {
        let gap = insert_index(&self.rects(), self.edge.vertical(), p);
        if gap >= self.buttons.len() {
            return usize::MAX;
        }
        self.buttons[..gap]
            .iter()
            .filter(|(q, _)| *q != panel)
            .count()
    }

    /// La línea de inserción con el puntero en `p` (ver [`insertion_line`]).
    pub(crate) fn line(&self, p: egui::Pos2) -> Rect {
        let rects = self.rects();
        let vertical = self.edge.vertical();
        insertion_line(
            &rects,
            self.rect,
            vertical,
            insert_index(&rects, vertical, p),
        )
    }
}

/// El grueso de la línea de inserción.
const LINE: f32 = 2.0;

/// En qué hueco entre `buttons` (los de una tira, en orden; a lo alto si `vertical`) cae el
/// puntero `pointer`: cuántos tienen el centro antes de él a lo largo de la tira (de 0, antes del
/// primero, a `buttons.len()`, después del último).
pub(crate) fn insert_index(buttons: &[Rect], vertical: bool, pointer: egui::Pos2) -> usize {
    let along = |p: egui::Pos2| if vertical { p.y } else { p.x };
    buttons
        .iter()
        .filter(|r| along(r.center()) < along(pointer))
        .count()
}

/// La línea de inserción del hueco `index` entre `buttons` (ver [`insert_index`]) de la tira
/// `strip`: [`LINE`] de grueso, a lo ancho del botón, centrada en el hueco entre dos; antes del
/// primero o después del último, pegada a él (sin salirse de la tira). En puntos enteros. Sin
/// botones, ninguna (`Rect::NOTHING`).
pub(crate) fn insertion_line(buttons: &[Rect], strip: Rect, vertical: bool, index: usize) -> Rect {
    let (Some(first), Some(last)) = (buttons.first(), buttons.last()) else {
        return Rect::NOTHING;
    };
    let along = |r: &Rect| if vertical { r.y_range() } else { r.x_range() };
    let ends = along(&strip);
    let start = if index == 0 {
        (along(first).min - LINE).max(ends.min)
    } else if index >= buttons.len() {
        (along(last).max + LINE).min(ends.max) - LINE
    } else {
        (along(&buttons[index - 1]).max + along(&buttons[index]).min) / 2.0 - LINE / 2.0
    }
    .round();
    let line = egui::Rangef::new(start, start + LINE);
    let across = if vertical {
        first.x_range()
    } else {
        first.y_range()
    };
    let across = egui::Rangef::new(across.min.round(), across.max.round());
    if vertical {
        Rect::from_x_y_ranges(across, line)
    } else {
        Rect::from_x_y_ranges(line, across)
    }
}

/// Lo que se pidió desde la tira para uno de sus paneles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StripRequest {
    /// Su ícono se arrastró y se soltó con el puntero ahí, fuera de la tira. Sobre otra tira, cae
    /// entre sus botones donde estaba el puntero: el lugar lo calcula quien lo aplica, con los
    /// botones de esa tira ([`StripArea::drop_index`]; esta tira no los conoce).
    Drop(DockPanel, egui::Pos2),
    /// Su ícono se arrastró a lo largo de su tira y se soltó en el hueco `usize` entre sus
    /// paneles (contando al mismo panel: 0, antes del primero; la cantidad de paneles, al final).
    /// Solo si cambia el orden.
    Reorder(DockPanel, usize),
    /// "Dock" de su menú contextual: al grupo con el foco de la columna.
    Dock(DockPanel),
    /// "Float" de su menú: en una ventana con la esquina ahí, junto a la tira.
    Float(DockPanel, egui::Pos2),
}

/// El nombre de las opciones del menú contextual de un ícono de la tira.
pub(crate) const MENU_DOCK: &str = "Dock";
pub(crate) const MENU_FLOAT: &str = "Float";
/// El botón de los paneles que no caben en la tira (su tooltip y su etiqueta accesible): un clic
/// los lista y elegir uno abre su flyout junto a él.
pub(crate) const MORE: &str = "More Panels";
/// Su ícono: la flecha de "siguiente" de las pestañas del kit (sin el skin, `»`).
const MORE_ICON: &str = crate::kit::tabs::ARROW_NEXT;

/// Un botón del dock con Pixel UI: la pieza `button` según el estado (prendido, `on`, con el
/// marco rosa de la herramienta elegida) y el ícono `icon` centrado (o `plain`, si el skin no lo
/// trae).
fn pixel_button(
    ui: &mut Ui,
    skin: &Skin,
    label: &str,
    (icon, plain): (&str, &str),
    on: bool,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(BUTTON_SIZE, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), BUTTON_SIZE);
    let info = pixel_draw::ButtonInfo {
        selected: Some(on),
        ..pixel_draw::ButtonInfo::new(ui.id().with(("dock", label)), label)
    };
    let ppp = ui.ctx().pixels_per_point();
    let response = pixel_draw::skin_button(ui, skin, rect, info, |r| {
        pixel_draw::piece_centered(skin, icon, r, ppp)
    });
    plain_icon(ui, skin, icon, plain, rect);
    response.tip(label)
}

/// Si el skin no trae la pieza `icon`, el texto `plain` centrado en `rect`: así el botón no queda
/// vacío mientras falta el ícono.
fn plain_icon(ui: &Ui, skin: &Skin, icon: &str, plain: &str, rect: Rect) {
    if skin.piece(icon, WidgetState::Normal).is_none() {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let color = skin.colors().text;
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            plain,
            font,
            color,
        );
    }
}

/// El marco de la tira de íconos (y del panel 3D fijo que va en su lugar en la pestaña de un
/// modelo): el de los paneles de egui, sin margen a la izquierda (el aire es el del lienzo) y con
/// [`RIGHT_MARGIN`] a la derecha.
pub(crate) fn strip_frame(style: &egui::Style) -> egui::Frame {
    let mut frame = egui::Frame::side_top_panel(style);
    frame.inner_margin.left = 0;
    frame.inner_margin.right = RIGHT_MARGIN;
    frame
}

/// El marco de la tira de la orilla `edge`: el de la derecha es [`strip_frame`]; los demás, el mismo
/// girado (el aire de [`RIGHT_MARGIN`] del lado de la orilla, nada del lado de adentro y, a lo
/// largo, el margen de los paneles de egui).
pub(crate) fn edge_frame(style: &egui::Style, edge: Edge) -> egui::Frame {
    let mut frame = egui::Frame::side_top_panel(style);
    let along = frame.inner_margin.top;
    let m = &mut frame.inner_margin;
    match edge {
        Edge::Right => return strip_frame(style),
        Edge::Left => (m.left, m.right) = (RIGHT_MARGIN, 0),
        Edge::Top => {
            (m.top, m.bottom) = (RIGHT_MARGIN, 0);
            (m.left, m.right) = (along, along);
        }
        Edge::Bottom => {
            (m.top, m.bottom) = (0, RIGHT_MARGIN);
            (m.left, m.right) = (along, along);
        }
    }
    frame
}

/// Lo que la tira le quita a lo que va a su izquierda (el lienzo y el timeline de debajo): su
/// ancho; sin Pixel UI, menos su línea de la izquierda (`egui::Panel` la pone en su margen de
/// afuera y lo de al lado empieza ahí: así estaba cuando la tira era un panel de la hoja del
/// lienzo).
pub(crate) fn strip_room(style: &egui::Style, pixel_ui: bool) -> f32 {
    if pixel_ui {
        WIDTH
    } else {
        WIDTH - style.visuals.widgets.noninteractive.bg_stroke.width.round()
    }
}

/// Lo más corta que puede quedar la tira de la derecha sin encimar nada: su marco, el aire de
/// arriba y de abajo (con Pixel UI), el botón "More Panels" y los dos del timeline y del Preview.
/// Es el mínimo a lo alto de la hoja del lienzo (la tira va a su lado; ver
/// `ChimpApp::canvas_area_min`): con menos lugar los paneles se van a "More Panels" (ver
/// [`show`]), pero esos tres botones caben siempre.
pub(crate) fn strip_min_len(pixel_ui: bool) -> f32 {
    // El margen del marco de los paneles de egui no depende del estilo.
    let frame = egui::Frame::side_top_panel(&egui::Style::default()).inner_margin;
    let air = if pixel_ui { TOP_AIR + BOTTOM_AIR } else { 0.0 };
    frame.sum().y + air + 3.0 * BUTTON_SIZE.y + 2.0 * BUTTON_GAP
}

/// El botón del timeline: lo muestra o lo esconde.
pub(crate) const TIMELINE: &str = "Timeline";
pub(crate) const TIMELINE_ICON: &str = "icon.timeline";
/// El alto de fábrica de la línea de tiempo.
pub(crate) const TIMELINE_DEFAULT_HEIGHT: f32 = 190.0;
/// El botón de la ventanita de preview: la abre o la cierra.
pub(crate) const PREVIEW: &str = "Preview";
pub(crate) const PREVIEW_ICON: &str = "icon.preview";

/// Un botón del dock que prende o apaga `on`: con Pixel UI, el del skin; sin él, el de egui con
/// `plain`.
fn toggle(ui: &mut Ui, skin: Option<&Skin>, label: &str, icon: &str, plain: &str, on: &mut bool) {
    let response = match skin {
        Some(skin) => pixel_button(ui, skin, label, (icon, plain), *on),
        None => ui
            .add(egui::Button::new(plain).selected(*on).min_size(BUTTON_SIZE))
            .tip(label),
    };
    if response.clicked() {
        *on = !*on;
    }
}

/// El panel abierto (en la memoria de egui, por pestaña de la app: no se guarda).
fn open_id() -> egui::Id {
    egui::Id::new("dock_open")
}

pub fn open_panel(ctx: &egui::Context) -> Option<DockPanel> {
    ctx.data(|d| d.get_temp::<Option<DockPanel>>(open_id()))
        .flatten()
}

pub fn set_open(ctx: &egui::Context, panel: Option<DockPanel>) {
    ctx.data_mut(|d| d.insert_temp(open_id(), panel));
}

/// Lo que la app le da a una tira de íconos para dibujarla (ver [`show`]).
pub(crate) struct StripCtx<'a> {
    /// Su orilla.
    pub edge: Edge,
    /// Su rect, con su marco (el de su [`StripArea`]): arrastrar un ícono dentro de él lo
    /// reordena; soltarlo fuera, lo saca.
    pub rect: Rect,
    /// Sus paneles, en orden (los que no están acoplados ni flotando).
    pub panels: &'a [DockPanel],
    /// Los botones del timeline y del Preview, hasta abajo (solo los lleva la de la derecha).
    pub toggles: Option<(&'a mut bool, &'a mut bool)>,
    /// El área del workspace: ahí caben los flyouts de las tiras de arriba y de abajo.
    pub room: Rect,
    /// El ancho del contenido del flyout de cada panel: "Float" lo pone a flotar donde quedaría
    /// su flyout (la toolbar, del ancho de su caja).
    pub flyout_width: &'a dyn Fn(DockPanel) -> f32,
}

/// Cuántos botones de `len` de largo (más [`BUTTON_GAP`] entre ellos) caben en `room`.
fn buttons_in(room: f32, len: f32) -> usize {
    ((room + BUTTON_GAP) / (len + BUTTON_GAP)).floor().max(0.0) as usize
}

/// El dock: un botón por panel de la tira (`strip`, los que no están acoplados ni flotando) y el
/// panel abierto, flotando junto a su botón (hacia adentro de la ventana). Un clic en un botón abre
/// (o cierra) su panel; arrastrarlo más de lo que egui todavía cuenta como clic (6 puntos) lo
/// levanta (con un fantasma bajo el puntero): a lo largo de la tira, una línea marca el hueco
/// donde caería y soltarlo ahí lo reordena ([`StripRequest::Reorder`]); soltarlo fuera de la tira
/// pide sacarlo ([`StripRequest::Drop`]); su menú contextual tiene "Dock" y "Float". Si no caben
/// todos, los del final se van a "More Panels" ([`MORE`]), que los lista: elegir uno abre su
/// flyout junto a él.
pub(crate) fn show(ui: &mut Ui, skin: Option<&Skin>, strip: StripCtx<'_>) -> DockOutput {
    let StripCtx {
        edge,
        rect: own,
        panels,
        toggles,
        room,
        flyout_width,
    } = strip;
    let mut out = DockOutput::default();
    let open = open_panel(ui.ctx()).filter(|p| panels.contains(p));
    let mut anchors = Vec::new();
    let mut buttons = Vec::new();
    // El ícono que se arrastra dentro de la tira: cuál, dónde está el puntero y si se soltó.
    let mut along_strip = None;
    let vertical = edge.vertical();
    if vertical {
        ui.spacing_mut().item_spacing.y = BUTTON_GAP;
    } else {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP;
    }
    // Lo de adentro del marco.
    let strip_rect = ui.max_rect();
    let mut column = ui.max_rect();
    // A lo largo de la tira: lo que hay para los botones de los paneles (sin el aire de las
    // puntas ni los del timeline y el Preview, con un aire entre los dos grupos).
    let along = if vertical {
        strip_rect.height()
    } else {
        strip_rect.width()
    };
    let air = if skin.is_some() {
        TOP_AIR + BOTTOM_AIR
    } else {
        0.0
    };
    let tail = match toggles {
        Some(_) => 2.0 * BUTTON_SIZE.y + 2.0 * BUTTON_GAP,
        None => 0.0,
    };
    let fit = buttons_in(along - air - tail, BUTTON_SIZE.y);
    let (shown, hidden) = if panels.len() <= fit {
        (panels, &[][..])
    } else {
        panels.split_at(fit.saturating_sub(1))
    };
    let inner = |ui: &mut Ui| {
        if skin.is_some() {
            ui.add_space(TOP_AIR);
            if vertical {
                column.min.y += TOP_AIR;
                column.max.y -= BOTTOM_AIR;
            }
        }
        for &panel in shown {
            let is_open = open == Some(panel);
            let response = match skin {
                Some(skin) => pixel_button(
                    ui,
                    skin,
                    panel.label(),
                    (panel.spec().icon, panel.spec().plain),
                    is_open,
                ),
                None => ui
                    .add(
                        egui::Button::new(panel.spec().plain)
                            .selected(is_open)
                            .min_size(BUTTON_SIZE),
                    )
                    .tip(panel.label()),
            };
            // También se arrastra (egui lo cuenta como clic mientras no se mueva más de 6 puntos).
            let response = response.interact(Sense::click_and_drag());
            if response.clicked() {
                set_open(ui.ctx(), (!is_open).then_some(panel));
            }
            // Soltar un ícono sobre la tira (con su marco) no lo saca: lo reordena.
            let pointer = ui.input(|i| i.pointer.latest_pos());
            let outside = pointer.filter(|p| !own.contains(*p));
            let inside = pointer.filter(|p| own.contains(*p));
            if response.dragged() {
                let delta = response.total_drag_delta().unwrap_or_default();
                ghost(ui, skin, panel, response.rect.translate(delta.round()));
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                out.dragging = outside.map(|p| (panel, p));
                along_strip = inside.map(|p| (panel, p, false));
            }
            if response.drag_stopped() {
                if let Some(p) = outside {
                    out.request = Some(StripRequest::Drop(panel, p));
                }
                along_strip = inside.map(|p| (panel, p, true));
            }
            crate::menus::context_menu(&response, |ui| {
                if ui.add(crate::menus::plain(MENU_DOCK)).clicked() {
                    out.request = Some(StripRequest::Dock(panel));
                    ui.close();
                }
                if ui.add(crate::menus::plain(MENU_FLOAT)).clicked() {
                    let at = float_corner(edge, panel, response.rect, flyout_width(panel));
                    out.request = Some(StripRequest::Float(panel, at.round()));
                    ui.close();
                }
            });
            anchors.push((panel, response.rect));
            buttons.push((panel, response.rect));
        }
        // Los que no caben, en "More Panels": prendido si el abierto es uno de ellos.
        if !hidden.is_empty() {
            let is_open = open.is_some_and(|p| hidden.contains(&p));
            let response = match skin {
                Some(skin) => pixel_button(ui, skin, MORE, (MORE_ICON, "»"), is_open),
                None => ui
                    .add(
                        egui::Button::new("»")
                            .selected(is_open)
                            .min_size(BUTTON_SIZE),
                    )
                    .tip(MORE),
            };
            let align = match edge {
                Edge::Right => egui::RectAlign::LEFT_START,
                Edge::Left => egui::RectAlign::RIGHT_START,
                Edge::Top => egui::RectAlign::BOTTOM_START,
                Edge::Bottom => egui::RectAlign::TOP_START,
            };
            crate::menus::click_menu(&response, align, |ui| {
                for &panel in hidden {
                    if ui.add(crate::menus::plain(panel.label())).clicked() {
                        set_open(ui.ctx(), Some(panel));
                        ui.close();
                    }
                }
            });
            anchors.extend(hidden.iter().map(|&p| (p, response.rect)));
        }
        // Hasta abajo del dock, el del timeline y arriba de él, el del preview.
        if let Some((timeline, preview)) = toggles {
            let bottom = egui::Layout::bottom_up(egui::Align::Min);
            ui.with_layout(bottom, |ui| {
                if skin.is_some() {
                    ui.add_space(BOTTOM_AIR);
                }
                toggle(ui, skin, TIMELINE, TIMELINE_ICON, "🎞", timeline);
                toggle(ui, skin, PREVIEW, PREVIEW_ICON, "▣", preview);
            });
        }
    };
    if vertical {
        inner(ui);
    } else {
        let row = egui::Layout::left_to_right(egui::Align::Min);
        ui.with_layout(row, inner);
        column = room;
    }
    // Un ícono que se arrastra a lo largo de la tira: la línea en el hueco donde caería y, al
    // soltarlo, a ese hueco (más allá del último a la vista, al final; en su mismo lugar, nada).
    if let Some((panel, p, dropped)) = along_strip {
        let rects: Vec<Rect> = buttons.iter().map(|(_, r)| *r).collect();
        let gap = insert_index(&rects, vertical, p);
        if dropped {
            let gap = if gap == rects.len() {
                panels.len()
            } else {
                gap
            };
            let at = shown.iter().position(|&q| q == panel).unwrap_or(gap);
            if gap != at && gap != at + 1 {
                out.request = Some(StripRequest::Reorder(panel, gap));
            }
        } else {
            let line = insertion_line(&rects, own, vertical, gap);
            crate::workspace::paint_insertion_line(ui, skin, line);
        }
    }
    out.buttons = buttons;
    let Some(panel) = open_panel(ui.ctx()).filter(|p| panels.contains(p)) else {
        return out;
    };
    let button = anchors
        .iter()
        .find(|(p, _)| *p == panel)
        .map(|(_, r)| *r)
        .unwrap_or(Rect::NOTHING);
    let anchor = Anchor {
        button,
        column,
        edge,
    };
    // Todos los paneles abiertos los dibuja la app (`ChimpApp::panel_flyout`).
    match panel {
        DockPanel::Artboards
        | DockPanel::Slices
        | DockPanel::Lospec
        | DockPanel::Groom
        | DockPanel::Lumen
        | DockPanel::Library
        | DockPanel::Font
        | DockPanel::Model
        // La toolbar cerrada (en la tira de la orilla de donde estaba).
        | DockPanel::Tools => out.app_panel = Some((panel, anchor)),
        #[cfg(not(target_arch = "wasm32"))]
        DockPanel::Dafont => out.app_panel = Some((panel, anchor)),
        // Los demás nunca llegan a la tira: el botón « de un grupo deja a Color donde está
        // (`Workspace::goes_to_strip`).
        DockPanel::Canvas
        | DockPanel::Color
        | DockPanel::Timeline
        | DockPanel::Preview => {}
    }
    out
}

/// Dónde pone "Float" la esquina de la ventana de `panel` desde su botón `button` de la tira de
/// `edge`: donde quedaría su flyout (de `width` de contenido), hacia adentro de la ventana.
fn float_corner(edge: Edge, panel: DockPanel, button: Rect, width: f32) -> egui::Pos2 {
    match edge {
        Edge::Right => button.left_top() - vec2(flyout_reach(width), 0.0),
        Edge::Left => button.right_top() + vec2(FLYOUT_GAP, 0.0),
        Edge::Top => button.left_bottom() + vec2(0.0, FLYOUT_GAP),
        Edge::Bottom => button.left_top() - vec2(0.0, flyout_reach(panel.spec().default_size.y)),
    }
}

/// El fantasma del ícono de `panel` que se arrastra, con el botón en `button` (donde quedó el
/// suyo, corrido lo que se arrastró): con Pixel UI, la pastilla del kit con su título
/// (`kit::tabs::drag_tag_shapes`) con la esquina en la del botón; sin él, el título en un
/// recuadro.
fn ghost(ui: &Ui, skin: Option<&Skin>, panel: DockPanel, button: Rect) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("dock_ghost"),
    ));
    let title = panel.spec().title;
    let Some(skin) = skin else {
        let galley = painter.layout_no_wrap(
            title.to_owned(),
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
        );
        let rect = Rect::from_min_size(button.min, galley.size() + vec2(8.0, 4.0));
        painter.add(egui::Frame::popup(ui.style()).inner_margin(0).paint(rect));
        painter.galley(rect.min + vec2(4.0, 2.0), galley, ui.visuals().text_color());
        return;
    };
    painter.extend(crate::kit::tabs::drag_tag_shapes(
        ui, skin, button.min, title,
    ));
}

/// El panel flotante de `panel` a la izquierda de su botón, de `width` de contenido. Arriba va
/// alineado con el botón; si no cabe hacia abajo, sube hasta donde haga falta (no más arriba de
/// la columna del dock) y lo que no quepa en la columna se ve con scroll. Los altos (`tall`:
/// Lospec, Dafont, Groom, Lumen, Library) ocupan la columna entera, de arriba a abajo. Con Pixel UI, en la caja
/// del skin, con el botón hecho pestaña de la caja.
pub fn flyout(
    ctx: &egui::Context,
    panel: DockPanel,
    anchor: Anchor,
    skin: Option<&Skin>,
    width: f32,
    tall: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    let body = if tall { Body::Tall } else { Body::Fit };
    flyout_with(ctx, panel, anchor, skin, width, body, add_contents);
}

/// Como [`flyout`] alto, pero sin scroll propio: el contenido recibe toda la columna (`width` de
/// ancho por el alto de la columna, sin el marco) y acomoda él mismo lo fijo y lo que se desplaza
/// (el panel FontForge, con su título y su pie fijos).
pub fn flyout_fixed(
    ctx: &egui::Context,
    panel: DockPanel,
    anchor: Anchor,
    skin: Option<&Skin>,
    width: f32,
    add_contents: impl FnOnce(&mut Ui),
) {
    flyout_with(ctx, panel, anchor, skin, width, Body::Fixed, add_contents);
}

fn flyout_with(
    ctx: &egui::Context,
    panel: DockPanel,
    anchor: Anchor,
    skin: Option<&Skin>,
    width: f32,
    body: Body,
    add_contents: impl FnOnce(&mut Ui),
) {
    let tall = body != Body::Fit;
    let id = egui::Id::new(("dock_flyout", panel.label()));
    let Anchor {
        button,
        column,
        edge,
    } = anchor;
    // El alto de la vez pasada, para saber si cabe.
    let height = ctx.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let frame_len = 2.0 * FLYOUT_MARGIN as f32;
    // Dónde va (junto a su botón, hacia adentro de la ventana) y lo que cabe a lo alto, sin el
    // marco (el de egui mide más o menos lo mismo).
    let (pos, pivot, max_height) = match edge {
        Edge::Right | Edge::Left => {
            let top = if tall {
                column.top().round()
            } else {
                flyout_top(button.top(), height, column)
            };
            let max_height = (column.height() - frame_len).max(BUTTON_SIZE.y);
            if edge == Edge::Right {
                let pos = egui::pos2(button.left() - FLYOUT_GAP, top);
                (pos, egui::Align2::RIGHT_TOP, max_height)
            } else {
                let pos = egui::pos2(button.right() + FLYOUT_GAP, top);
                (pos, egui::Align2::LEFT_TOP, max_height)
            }
        }
        Edge::Top | Edge::Bottom => {
            // A lo ancho, alineado con su botón si cabe en el área (si no, más a la izquierda).
            let x = button
                .left()
                .min(column.right() - (width + frame_len))
                .max(column.left())
                .round();
            if edge == Edge::Top {
                let top = button.bottom() + FLYOUT_GAP;
                let max_height = (column.bottom() - top - frame_len).max(BUTTON_SIZE.y);
                (egui::pos2(x, top), egui::Align2::LEFT_TOP, max_height)
            } else {
                let bottom = button.top() - FLYOUT_GAP;
                let max_height = (bottom - column.top() - frame_len).max(BUTTON_SIZE.y);
                (egui::pos2(x, bottom), egui::Align2::LEFT_BOTTOM, max_height)
            }
        }
    };
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .pivot(pivot)
        .show(ctx, |ui| {
            let frame = match skin {
                Some(_) => egui::Frame::NONE.inner_margin(FLYOUT_MARGIN),
                None => egui::Frame::popup(ui.style()),
            };
            // Lugar para la caja, detrás del contenido (se llena al final, cuando se sabe su tamaño).
            let background = ui.painter().add(Shape::Noop);
            let response = frame.show(ui, |ui| {
                // El Area da de alto lo que midió la vez pasada: sin esto, el panel no crecería
                // cuando su contenido crece.
                ui.set_max_height(max_height);
                if body == Body::Fixed {
                    ui.set_width(width);
                    ui.set_height(max_height);
                    add_contents(ui);
                    return;
                }
                let mut area = egui::ScrollArea::vertical()
                    .id_salt(id.with("scroll"))
                    .max_height(max_height);
                if tall {
                    area = area
                        .min_scrolled_height(max_height)
                        .auto_shrink([true, false]);
                }
                pixel_ui::scroll(ui, area, |ui| {
                    ui.set_width(width);
                    add_contents(ui);
                });
            });
            let rect = response.response.rect;
            ui.ctx().data_mut(|d| d.insert_temp(id, rect.height()));
            // Con Pixel UI, la caja del panel detrás del contenido y el botón hecho pestaña.
            if let (Some(skin), Some(piece)) = (
                skin,
                skin.and_then(|s| s.piece(FLYOUT_PIECE, WidgetState::Normal)),
            ) {
                let ppp = ui.ctx().pixels_per_point();
                let rect = Rect::from_min_size(rect.min.round(), rect.size().round());
                let mut shapes = pixel_ui::piece_shapes(piece, rect, ppp);
                let icon = panel.spec().icon;
                shapes.extend(match edge {
                    Edge::Right => tab_shapes(skin, piece, icon, rect, button, ppp),
                    _ => edge_tab_shapes(skin, piece, icon, (rect, button), edge, ppp),
                });
                ui.painter().set(background, Shape::Vec(shapes));
                plain_icon(ui, skin, panel.spec().icon, panel.spec().plain, button);
            }
        });
}

/// Ancho del panel fijo de `width` de contenido (ver [`fixed_panel`]): su caja y el aire hasta
/// el panel derecho.
pub fn fixed_panel_width(width: f32) -> f32 {
    width + 2.0 * FLYOUT_MARGIN as f32 + RIGHT_MARGIN as f32
}

/// En lugar de la columna del dock (la pestaña de un modelo): un panel fijo que no se cierra, en
/// la caja de los paneles flotantes pero sin pestaña, a lo alto de la columna (de la altura de las
/// reglas del lienzo a la del botón del timeline). Como en [`flyout_fixed`], el contenido recibe
/// `width` de ancho por todo el alto, sin el marco.
pub fn fixed_panel(
    ui: &mut Ui,
    skin: Option<&Skin>,
    width: f32,
    add_contents: impl FnOnce(&mut Ui),
) {
    let mut column = ui.max_rect();
    if skin.is_some() {
        column.min.y += TOP_AIR;
        column.max.y -= BOTTOM_AIR;
    }
    let size = vec2(width + 2.0 * FLYOUT_MARGIN as f32, column.height());
    let rect = Rect::from_min_size(column.min.round(), size.round());
    let ppp = ui.ctx().pixels_per_point();
    let piece = skin.and_then(|s| s.piece(FLYOUT_PIECE, WidgetState::Normal));
    let shape = match piece {
        Some(piece) => Shape::Vec(pixel_ui::piece_shapes(piece, rect, ppp)),
        None => egui::Frame::popup(ui.style()).inner_margin(0).paint(rect),
    };
    ui.painter().add(shape);
    let inner = rect.shrink(FLYOUT_MARGIN as f32);
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.set_width(width);
        ui.set_height(inner.height());
        add_contents(ui);
    });
}

/// Cuánto entra en el lienzo, desde su borde derecho, un panel flotante de `width` de contenido:
/// el contenido, el marco a los dos lados y el aire hasta su botón.
pub fn flyout_reach(width: f32) -> f32 {
    width + 2.0 * FLYOUT_MARGIN as f32 + FLYOUT_GAP
}

/// Dónde empieza un panel de `height` cuyo botón está en `button_top`: a su altura si cabe en
/// `column`; si no, más arriba, hasta el borde de arriba de la columna.
fn flyout_top(button_top: f32, height: f32, column: Rect) -> f32 {
    let room = column.bottom() - height;
    button_top.min(room).max(column.top()).round()
}

/// El botón del panel abierto como pestaña de su caja (`panel_rect`): la caja del skin desde el
/// borde del panel hasta el del botón, sin la línea que los separa, y el ícono encima. Solo se
/// pinta: el clic sigue llegando al botón de abajo (que cierra el panel).
fn tab_shapes(
    skin: &Skin,
    piece: &crate::skin::Piece,
    icon: &str,
    panel_rect: Rect,
    button: Rect,
    ppp: f32,
) -> Vec<Shape> {
    let Some(center) = piece.center else {
        return Vec::new();
    };
    let tab = Rect::from_min_max(
        egui::pos2(panel_rect.right() - 1.0, button.top()).round(),
        egui::pos2(button.right(), button.bottom()).round(),
    );
    let mut shapes = pixel_ui::piece_shapes(piece, tab, ppp);
    // Los bordes de la caja (lo que queda fuera del centro del 9-slice).
    let left = center.x as f32;
    let right = (piece.size[0] - center.x - center.w) as f32;
    let top = center.y as f32;
    let bottom = (piece.size[1] - center.y - center.h) as f32;
    let fill = piece.pixel(center.x, center.y);
    // Sin la línea entre la caja y la pestaña: el relleno de una a la otra.
    let seam = Rect::from_min_max(
        egui::pos2(panel_rect.right() - right, tab.top() + top),
        egui::pos2(tab.left() + left, tab.bottom() - bottom),
    );
    shapes.push(Shape::rect_filled(seam, 0.0, fill));
    // Los bordes de arriba y de abajo siguen derecho de la caja a la pestaña: desde la línea
    // oscura de la caja o, si los dos empiezan (o terminan) a la misma altura, tapando también
    // la esquina de la caja.
    let line = |x0: f32, y: f32, color: egui::Color32| {
        let r = Rect::from_min_max(egui::pos2(x0, y), egui::pos2(tab.left() + left, y + 1.0));
        Shape::rect_filled(r, 0.0, color)
    };
    let from = |aligned: bool| {
        if aligned {
            panel_rect.right() - right
        } else {
            tab.left()
        }
    };
    let x0 = from((tab.top() - panel_rect.top()).abs() < 0.5);
    for k in 0..center.y {
        shapes.push(line(x0, tab.top() + k as f32, piece.pixel(center.x, k)));
    }
    let x0 = from((tab.bottom() - panel_rect.bottom()).abs() < 0.5);
    let below = center.y + center.h;
    for k in below..piece.size[1] {
        let y = tab.bottom() - (piece.size[1] - k) as f32;
        shapes.push(line(x0, y, piece.pixel(center.x, k)));
    }
    shapes.extend(pixel_draw::piece_centered(skin, icon, button, ppp));
    shapes
}

/// Como [`tab_shapes`] (el de la tira de la derecha) para las otras orillas: el botón del panel
/// abierto como pestaña de su caja (`panel_rect`), del lado de la caja que da a la tira (`edge`:
/// la de la izquierda, la de arriba o la de abajo), sin la línea que los separa y con los bordes
/// de los lados siguiendo derecho de la pestaña a la caja.
fn edge_tab_shapes(
    skin: &Skin,
    piece: &crate::skin::Piece,
    icon: &str,
    (panel_rect, button): (Rect, Rect),
    edge: Edge,
    ppp: f32,
) -> Vec<Shape> {
    let Some(center) = piece.center else {
        return Vec::new();
    };
    let tab = match edge {
        Edge::Left => Rect::from_min_max(
            button.min.round(),
            egui::pos2(panel_rect.left() + 1.0, button.bottom()).round(),
        ),
        Edge::Top => Rect::from_min_max(
            button.min.round(),
            egui::pos2(button.right(), panel_rect.top() + 1.0).round(),
        ),
        Edge::Bottom => Rect::from_min_max(
            egui::pos2(button.left(), panel_rect.bottom() - 1.0).round(),
            button.max.round(),
        ),
        Edge::Right => return tab_shapes(skin, piece, icon, panel_rect, button, ppp),
    };
    // Lo mismo para las tres orillas, en dos ejes: `a`, donde se juntan la pestaña y la caja (x a
    // la izquierda, y arriba o abajo), y `c`, el otro. `before`: la pestaña va antes de la caja
    // en `a` (a la izquierda o arriba).
    let across = edge != Edge::Left;
    let before = edge != Edge::Bottom;
    let at = |a: f32, c: f32| {
        if across {
            egui::pos2(c, a)
        } else {
            egui::pos2(a, c)
        }
    };
    let span_of = |r: Rect| if across { r.y_range() } else { r.x_range() };
    let cross_of = |r: Rect| if across { r.x_range() } else { r.y_range() };
    let (tab_a, panel_a, tab_c, panel_c) = (
        span_of(tab),
        span_of(panel_rect),
        cross_of(tab),
        cross_of(panel_rect),
    );
    // Los bordes de la caja (lo que queda fuera del centro del 9-slice), en cada eje: antes y
    // después del centro.
    let (cx, cy, cw, ch) = (center.x, center.y, center.w, center.h);
    let (w, h) = (piece.size[0], piece.size[1]);
    let border = |lo: u32, len: u32, size: u32| (lo as f32, (size - lo - len) as f32);
    let (a_lo, a_hi) = if across {
        border(cy, ch, h)
    } else {
        border(cx, cw, w)
    };
    let (c_lo, c_hi) = if across {
        border(cx, cw, w)
    } else {
        border(cy, ch, h)
    };
    // Cada columna (o fila) del borde de la caja en `c`: su color, del centro del otro eje.
    let pixel = |k: u32| {
        if across {
            piece.pixel(k, cy)
        } else {
            piece.pixel(cx, k)
        }
    };
    let (c_first, c_after, c_size) = if across {
        (cx, cx + cw, w)
    } else {
        (cy, cy + ch, h)
    };
    let solid = |r: Rect, color: egui::Color32| Shape::rect_filled(r, 0.0, color);
    let mut shapes = pixel_ui::piece_shapes(piece, tab, ppp);
    // Sin la línea entre la pestaña y la caja: de lo de adentro de una a lo de adentro de la otra.
    let (a0, a1) = if before {
        (tab_a.max - a_hi, panel_a.min + a_lo)
    } else {
        (panel_a.max - a_hi, tab_a.min + a_lo)
    };
    let seam = Rect::from_min_max(at(a0, tab_c.min + c_lo), at(a1, tab_c.max - c_hi));
    shapes.push(solid(seam, piece.pixel(cx, cy)));
    // Los bordes de los lados, de la pestaña a la caja (hasta dentro de la caja si su borde de ese
    // lado va a la misma altura; si no, solo la pestaña).
    let span = |aligned: bool| match (before, aligned) {
        (_, true) => (a0, a1),
        (true, false) => (a0, tab_a.max),
        (false, false) => (tab_a.min, a1),
    };
    let (s0, s1) = span((tab_c.min - panel_c.min).abs() < 0.5);
    for k in 0..c_first {
        let c = tab_c.min + k as f32;
        shapes.push(solid(
            Rect::from_min_max(at(s0, c), at(s1, c + 1.0)),
            pixel(k),
        ));
    }
    let (s0, s1) = span((tab_c.max - panel_c.max).abs() < 0.5);
    for k in c_after..c_size {
        let c = tab_c.max - (c_size - k) as f32;
        shapes.push(solid(
            Rect::from_min_max(at(s0, c), at(s1, c + 1.0)),
            pixel(k),
        ));
    }
    shapes.extend(pixel_draw::piece_centered(skin, icon, button, ppp));
    shapes
}

/// Un panel flotante abierto dibujado en `panel_rect` con la pestaña que envuelve al botón
/// `button` (con el ícono `icon`): el UI Catalog lo muestra así, sin el contenido.
pub(crate) fn open_preview_shapes(
    skin: &Skin,
    panel_rect: Rect,
    button: Rect,
    icon: &str,
    ppp: f32,
) -> Vec<Shape> {
    let Some(piece) = skin.piece(FLYOUT_PIECE, WidgetState::Normal) else {
        return Vec::new();
    };
    let mut shapes = pixel_ui::piece_shapes(piece, panel_rect, ppp);
    shapes.extend(tab_shapes(skin, piece, icon, panel_rect, button, ppp));
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El alto del panel después de un cuadro con contenido de `content` de alto.
    fn panel_height(ctx: &egui::Context, content: f32) -> f32 {
        let column = Rect::from_min_max(egui::pos2(400.0, 20.0), egui::pos2(430.0, 580.0));
        let anchor = Anchor {
            button: Rect::from_min_size(egui::pos2(403.0, 20.0), BUTTON_SIZE),
            column,
            edge: Edge::Right,
        };
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 600.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            flyout(
                ui.ctx(),
                DockPanel::Slices,
                anchor,
                None,
                100.0,
                false,
                |ui| {
                    ui.allocate_exact_size(vec2(100.0, content), Sense::hover());
                },
            );
        });
        output.textures_delta.clear();
        let id = egui::Id::new(("dock_flyout", DockPanel::Slices.label()));
        ctx.data(|d| d.get_temp::<f32>(id)).unwrap()
    }

    #[test]
    fn the_panel_grows_with_its_contents_up_to_the_column() {
        let ctx = egui::Context::default();
        for _ in 0..3 {
            panel_height(&ctx, 50.0);
        }
        let small = panel_height(&ctx, 50.0);
        // Más contenido (un artboard nuevo): crece, aunque el Area recuerde el alto de antes.
        panel_height(&ctx, 200.0);
        let grown = panel_height(&ctx, 200.0);
        assert!(grown >= small + 150.0, "{small} → {grown}");
        // Más de lo que cabe en la columna: llega a ella y lo demás va con scroll.
        panel_height(&ctx, 2000.0);
        let full = panel_height(&ctx, 2000.0);
        assert!(full <= 560.0 + 1.0, "{full}");
    }

    /// Tres botones de la tira, uno cada 27 desde `start` a lo largo (a lo alto si `vertical`), a
    /// 3 de la orilla de una tira de 26 de ancho.
    fn three_buttons(vertical: bool, start: f32) -> Vec<Rect> {
        (0..3)
            .map(|i| {
                let along = start + i as f32 * (BUTTON_SIZE.y + BUTTON_GAP);
                let min = if vertical {
                    egui::pos2(3.0, along)
                } else {
                    egui::pos2(along, 3.0)
                };
                Rect::from_min_size(min, BUTTON_SIZE)
            })
            .collect()
    }

    #[test]
    fn the_insert_index_counts_the_button_centers_before_the_pointer() {
        for vertical in [true, false] {
            // Los botones: 10..33, 37..60 y 64..87 (sus centros en 21.5, 48.5 y 75.5).
            let buttons = three_buttons(vertical, 10.0);
            let at = |along: f32| {
                // Del otro lado, el centro de la tira (no cuenta).
                let p = if vertical {
                    egui::pos2(13.0, along)
                } else {
                    egui::pos2(along, 13.0)
                };
                insert_index(&buttons, vertical, p)
            };
            let case = format!("vertical {vertical}");
            assert_eq!(at(0.0), 0, "{case}: above the first");
            assert_eq!(at(20.0), 0, "{case}: on the first, before its center");
            assert_eq!(at(25.0), 1, "{case}: on the first, after its center");
            assert_eq!(at(35.0), 1, "{case}: between the first and the second");
            assert_eq!(at(50.0), 2, "{case}: on the second, after its center");
            assert_eq!(at(80.0), 3, "{case}: on the last, after its center");
            assert_eq!(at(300.0), 3, "{case}: after the last");
            assert_eq!(insert_index(&[], vertical, egui::pos2(5.0, 5.0)), 0);
        }
    }

    #[test]
    fn the_insertion_line_is_2_thick_across_the_button_in_the_gap_or_at_the_ends() {
        for vertical in [true, false] {
            let buttons = three_buttons(vertical, 10.0);
            let strip = if vertical {
                Rect::from_min_size(egui::Pos2::ZERO, vec2(26.0, 300.0))
            } else {
                Rect::from_min_size(egui::Pos2::ZERO, vec2(300.0, 26.0))
            };
            // A lo largo de la tira, `a..b`; a lo ancho, el del botón (3..26).
            let line = |a: f32, b: f32| {
                if vertical {
                    Rect::from_x_y_ranges(3.0..=26.0, a..=b)
                } else {
                    Rect::from_x_y_ranges(a..=b, 3.0..=26.0)
                }
            };
            let case = format!("vertical {vertical}");
            // Antes del primero, pegada a él; en el hueco de 4 entre dos, centrada; después del
            // último, pegada a él.
            assert_eq!(
                insertion_line(&buttons, strip, vertical, 0),
                line(8.0, 10.0),
                "{case}"
            );
            assert_eq!(
                insertion_line(&buttons, strip, vertical, 1),
                line(34.0, 36.0),
                "{case}"
            );
            assert_eq!(
                insertion_line(&buttons, strip, vertical, 2),
                line(61.0, 63.0),
                "{case}"
            );
            assert_eq!(
                insertion_line(&buttons, strip, vertical, 3),
                line(87.0, 89.0),
                "{case}"
            );
            for i in 0..=3 {
                let r = insertion_line(&buttons, strip, vertical, i);
                assert!(strip.contains_rect(r), "{case}: {i} {r:?} in the strip");
                assert_eq!(
                    (r.min.round(), r.max.round()),
                    (r.min, r.max),
                    "{case}: {i}"
                );
            }
            // Pegados a las puntas de la tira: la línea no se sale de ella.
            let tight = three_buttons(vertical, 0.0);
            let short = if vertical {
                strip.with_max_y(tight[2].bottom())
            } else {
                strip.with_max_x(tight[2].right())
            };
            assert_eq!(
                insertion_line(&tight, short, vertical, 0),
                line(0.0, 2.0),
                "{case}"
            );
            assert_eq!(
                insertion_line(&tight, short, vertical, 3),
                line(75.0, 77.0),
                "{case}"
            );
            // Sin botones, ninguna.
            assert!(!insertion_line(&[], strip, vertical, 0).is_positive());
        }
    }

    #[test]
    fn a_tall_panel_moves_up_but_not_above_the_column() {
        let column = Rect::from_min_max(egui::pos2(0.0, 100.0), egui::pos2(30.0, 700.0));
        assert_eq!(
            flyout_top(300.0, 200.0, column),
            300.0,
            "fits below its button"
        );
        assert_eq!(flyout_top(300.0, 500.0, column), 200.0, "moves up to fit");
        assert_eq!(
            flyout_top(300.0, 900.0, column),
            100.0,
            "the column's top at most"
        );
    }
}
