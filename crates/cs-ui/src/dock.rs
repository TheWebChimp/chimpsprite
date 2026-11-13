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
    /// Lo que se pidió para un panel de la tira: sacarlo (soltado fuera, o "Dock" y "Float" de su
    /// menú) o reordenarlo en ella. Lo resuelve la app con su workspace.
    pub request: Option<StripRequest>,
    /// El panel cuyo ícono se arrastra fuera de la tira y dónde está el puntero (para resaltar
    /// dónde caería).
    pub dragging: Option<(DockPanel, egui::Pos2)>,
    /// Los botones a la vista de los paneles de la tira, en su orden (sin "More Panels" ni los del
    /// timeline y el Preview): la app los guarda en su `StripArea`. Mientras la tira se ve como
    /// quedaría (ver [`show`]), los de antes, en reposo.
    pub buttons: Vec<(DockPanel, Rect)>,
}

/// Una tira dibujada en el frame: su orilla, su rect y los botones a la vista de sus paneles en
/// reposo (ver [`DockOutput::buttons`]: mientras se ve la vista previa de un arrastre, los de
/// antes). Un ícono soltado sobre ella cae entre sus botones (ver [`StripArea::drop_index`]) y,
/// mientras se arrastra encima, la tira se dibuja como quedaría (ver [`show`]).
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
}

/// La capa de las zonas de destino de lo que se arrastra (la de un panel en el workspace o la de
/// una tira): encima de todo salvo el fantasma.
pub(crate) fn drop_zone_painter(ctx: &egui::Context) -> egui::Painter {
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("workspace_drop_zone"),
    ))
}

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

/// `panels` (los de una tira) con `panel` en el lugar `index`, contado sin él (más allá del
/// final, al final): como queda la tira al soltarlo ahí (ver `Workspace::move_in_strip`).
pub(crate) fn landed(panels: &[DockPanel], panel: DockPanel, index: usize) -> Vec<DockPanel> {
    let mut out: Vec<DockPanel> = panels.iter().copied().filter(|&p| p != panel).collect();
    out.insert(index.min(out.len()), panel);
    out
}

/// Con cuánta opacidad va, en la vista previa de una tira, el botón del panel que se arrastra en
/// el lugar donde caería.
pub(crate) const PREVIEW_OPACITY: f32 = 0.5;

/// El ícono de una tira que se arrastra: su panel, el id de su botón y el rect de su botón cuando
/// empezó. Lo recuerda su botón (ver [`show`]): mientras se ve la vista previa, la tira lo dibuja
/// en otro lugar (o no lo dibuja) y el arrastre sigue en ese id y ese rect.
#[derive(Clone, Copy, Debug)]
struct IconDrag {
    panel: DockPanel,
    id: egui::Id,
    rect: Rect,
}

fn icon_drag_id() -> egui::Id {
    egui::Id::new("dock_icon_drag")
}

/// El ícono que se arrastra (o que se soltó en este frame), si es el arrastre de egui: si ya no lo
/// es (se canceló, o la tira dejó de dibujarse), se olvida.
fn icon_drag(ctx: &egui::Context) -> Option<IconDrag> {
    let drag = ctx.data(|d| d.get_temp::<IconDrag>(icon_drag_id()))?;
    if ctx.is_being_dragged(drag.id) || ctx.drag_stopped_id() == Some(drag.id) {
        return Some(drag);
    }
    ctx.data_mut(|d| d.remove::<IconDrag>(icon_drag_id()));
    None
}

/// Dónde recuerda la tira de `edge` sus botones en reposo: mientras se ve la vista previa, son
/// los que dicen dónde cae lo que se suelta (no se mueven con ella).
fn rest_id(edge: Edge) -> egui::Id {
    egui::Id::new(("dock_rest", edge.index()))
}

/// El id del `Ui` del botón de egui (sin Pixel UI) del ícono de `panel`: fijo, así el botón tiene
/// el mismo id aunque la vista previa lo dibuje en otro lugar de la tira.
fn icon_ui_id(panel: DockPanel) -> egui::Id {
    egui::Id::new(("dock_icon", panel))
}

/// El id del botón semitransparente de `panel` en la vista previa (ver [`faded_button`]).
fn faded_id(panel: DockPanel) -> egui::Id {
    egui::Id::new(("dock_icon_faded", panel))
}

/// Lo que se pidió desde la tira para uno de sus paneles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StripRequest {
    /// Su ícono se arrastró y se soltó con el puntero ahí, fuera de la tira. Sobre otra tira, cae
    /// entre sus botones donde estaba el puntero: el lugar lo calcula quien lo aplica, con los
    /// botones de esa tira (`StripArea::drop_index`; esta tira no los conoce).
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
    plain_icon(ui, skin, icon, plain, rect, 1.0);
    response.tip(label)
}

/// El botón semitransparente del ícono de `panel` en la vista previa de una tira (donde caería el
/// que se arrastra): el mismo componente que los demás (prendido, `on`, si su panel está abierto)
/// con [`PREVIEW_OPACITY`], sin responder a nada. Ocupa en la tira lo mismo que su botón.
/// Devuelve su rect.
fn faded_button(ui: &mut Ui, skin: Option<&Skin>, panel: DockPanel, on: bool) -> Rect {
    let (label, spec) = (panel.label(), panel.spec());
    let Some(skin) = skin else {
        // El botón de egui, en un `Ui` de id fijo como los de verdad (mide lo mismo que el suyo).
        let builder = egui::UiBuilder::new().id(faded_id(panel));
        return ui
            .scope_builder(builder, |ui| {
                ui.set_opacity(PREVIEW_OPACITY);
                ui.add(
                    egui::Button::new(spec.plain)
                        .selected(on)
                        .min_size(BUTTON_SIZE),
                )
            })
            .inner
            .rect;
    };
    let (rect, _) = ui.allocate_exact_size(BUTTON_SIZE, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), BUTTON_SIZE);
    let info = pixel_draw::ButtonInfo {
        selected: Some(on),
        ..pixel_draw::ButtonInfo::new(faded_id(panel), label)
    };
    let ppp = ui.ctx().pixels_per_point();
    pixel_draw::faded_skin_button(ui, skin, rect, info, PREVIEW_OPACITY, |r| {
        pixel_draw::piece_centered(skin, spec.icon, r, ppp)
    });
    plain_icon(ui, skin, spec.icon, spec.plain, rect, PREVIEW_OPACITY);
    rect
}

/// Si el skin no trae la pieza `icon`, el texto `plain` centrado en `rect` (con `opacity`, si es
/// menos que 1): así el botón no queda vacío mientras falta el ícono.
fn plain_icon(ui: &Ui, skin: &Skin, icon: &str, plain: &str, rect: Rect, opacity: f32) {
    if skin.piece(icon, WidgetState::Normal).is_none() {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let color = skin.colors().text;
        let mut painter = ui.painter().clone();
        if opacity < 1.0 {
            painter.set_opacity(opacity);
        }
        painter.text(
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
    /// Un panel que se arrastra desde fuera de las tiras (su pestaña, con egui_dock) con el
    /// puntero sobre esta tira, donde cae al soltarlo: se ve en la vista previa (ver [`show`]).
    pub incoming: Option<DockPanel>,
}

/// Cuántos botones de `len` de largo (más [`BUTTON_GAP`] entre ellos) caben en `room`.
fn buttons_in(room: f32, len: f32) -> usize {
    ((room + BUTTON_GAP) / (len + BUTTON_GAP)).floor().max(0.0) as usize
}

/// Los paneles de `panels` que se ven en una tira donde caben `fit` botones y los que van a "More
/// Panels" (los del final; con ellos, "More Panels" ocupa el último lugar).
fn split_shown(panels: &[DockPanel], fit: usize) -> (&[DockPanel], &[DockPanel]) {
    if panels.len() <= fit {
        (panels, &[][..])
    } else {
        panels.split_at(fit.saturating_sub(1))
    }
}

/// El dock: un botón por panel de la tira (`strip`, los que no están acoplados ni flotando) y el
/// panel abierto, flotando junto a su botón (hacia adentro de la ventana). Un clic en un botón abre
/// (o cierra) su panel; arrastrarlo más de lo que egui todavía cuenta como clic (6 puntos) lo
/// levanta (con un fantasma bajo el puntero): soltarlo a lo largo de la tira lo reordena
/// ([`StripRequest::Reorder`]); soltarlo fuera de la tira pide sacarlo ([`StripRequest::Drop`]);
/// su menú contextual tiene "Dock" y "Float". Si no caben todos, los del final se van a "More
/// Panels" ([`MORE`]), que los lista: elegir uno abre su flyout junto a él.
///
/// Mientras se arrastra un ícono (de esta tira u otra, ver [`IconDrag`]) o una pestaña que cae en
/// ella (`incoming`), la tira se dibuja como quedaría al soltarlo: con el puntero sobre ella, el
/// panel va en su lugar de destino (el de [`StripArea::drop_index`] con los botones en reposo),
/// semitransparente ([`faded_button`]), y los demás se recorren; el ícono de esta tira con el
/// puntero fuera de ella ya no está (se cierra su hueco). Si caería entre los de "More Panels", ese
/// botón se ve prendido. El arrastre sigue siendo del botón de antes: su id y su rect (sin
/// dibujarse ahí). "More Panels", el del timeline y el del Preview no se mueven.
pub(crate) fn show(ui: &mut Ui, skin: Option<&Skin>, strip: StripCtx<'_>) -> DockOutput {
    let StripCtx {
        edge,
        rect: own,
        panels,
        toggles,
        room,
        flyout_width,
        incoming,
    } = strip;
    let ctx = ui.ctx().clone();
    let mut out = DockOutput::default();
    let open = open_panel(&ctx).filter(|p| panels.contains(p));
    let mut anchors = Vec::new();
    let mut buttons = Vec::new();
    // Las respuestas de los botones de los paneles (y la del que se arrastra): se atienden al final.
    let mut responses: Vec<(DockPanel, egui::Response)> = Vec::new();
    // El ícono que se arrastra dentro de la tira: cuál, dónde está el puntero y si se soltó.
    let mut along_strip = None;
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let over = pointer.filter(|p| own.contains(*p));
    // La vista previa: el ícono de esta tira que se arrastra (`mine`) y lo que caería en ella con
    // el puntero encima (en su lugar de destino, con los botones en reposo).
    let drag = icon_drag(&ctx);
    let mine = drag.filter(|d| panels.contains(&d.panel));
    let coming = drag
        .map(|d| d.panel)
        .filter(|p| !panels.contains(p))
        .or(incoming);
    let rest = StripArea {
        edge,
        rect: own,
        buttons: ctx
            .data(|d| d.get_temp::<Vec<(DockPanel, Rect)>>(rest_id(edge)))
            .unwrap_or_default(),
    };
    let landing = mine
        .map(|d| d.panel)
        .or(coming)
        .zip(over)
        .map(|(p, at)| (p, landed(panels, p, rest.drop_index(p, at))));
    let previewing = mine.is_some() || landing.is_some();
    let (order, faded) = match (landing, mine) {
        (Some((p, order)), _) => (order, Some(p)),
        (None, Some(d)) => (
            panels.iter().copied().filter(|&p| p != d.panel).collect(),
            None,
        ),
        (None, None) => (panels.to_vec(), None),
    };
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
    let (shown, hidden) = split_shown(&order, fit);
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
            if faded == Some(panel) {
                anchors.push((panel, faded_button(ui, skin, panel, is_open)));
                continue;
            }
            let response = match skin {
                Some(skin) => pixel_button(
                    ui,
                    skin,
                    panel.label(),
                    (panel.spec().icon, panel.spec().plain),
                    is_open,
                ),
                // En un `Ui` de id fijo: el botón tiene el mismo id en cualquier lugar de la tira.
                None => {
                    let builder = egui::UiBuilder::new().id(icon_ui_id(panel));
                    ui.scope_builder(builder, |ui| {
                        ui.add(
                            egui::Button::new(panel.spec().plain)
                                .selected(is_open)
                                .min_size(BUTTON_SIZE),
                        )
                        .tip(panel.label())
                    })
                    .inner
                }
            };
            // También se arrastra (egui lo cuenta como clic mientras no se mueva más de 6 puntos).
            let response = response.interact(Sense::click_and_drag());
            anchors.push((panel, response.rect));
            buttons.push((panel, response.rect));
            responses.push((panel, response));
        }
        // Los que no caben, en "More Panels": prendido si el abierto es uno de ellos o si ahí
        // caería el que se arrastra.
        if !hidden.is_empty() {
            let is_open = open.is_some_and(|p| hidden.contains(&p));
            let lit = is_open || faded.is_some_and(|p| hidden.contains(&p));
            let response = match skin {
                Some(skin) => pixel_button(ui, skin, MORE, (MORE_ICON, "»"), lit),
                None => ui
                    .add(egui::Button::new("»").selected(lit).min_size(BUTTON_SIZE))
                    .tip(MORE),
            };
            let align = match edge {
                Edge::Right => egui::RectAlign::LEFT_START,
                Edge::Left => egui::RectAlign::RIGHT_START,
                Edge::Top => egui::RectAlign::BOTTOM_START,
                Edge::Bottom => egui::RectAlign::TOP_START,
            };
            crate::menus::click_menu(&response, align, |ui| {
                for &panel in hidden.iter().filter(|p| panels.contains(p)) {
                    if ui.add(crate::menus::plain(panel.label())).clicked() {
                        set_open(ui.ctx(), Some(panel));
                        ui.close();
                    }
                }
            });
            anchors.extend(hidden.iter().map(|&p| (p, response.rect)));
        }
        // El que se arrastra de esta tira sigue en su botón de antes (su id, su rect), aunque se
        // dibuje en otro lugar o no se dibuje.
        if let Some(d) = mine {
            let response = ui.interact(d.rect, d.id, Sense::click_and_drag());
            anchors.push((d.panel, d.rect));
            responses.push((d.panel, response));
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
    // Soltar un ícono sobre la tira (con su marco) no lo saca: lo reordena.
    let outside = pointer.filter(|p| !own.contains(*p));
    for (panel, response) in responses {
        if response.clicked() {
            set_open(&ctx, (open != Some(panel)).then_some(panel));
        }
        if response.dragged() {
            // Lo recuerda para la vista previa (la de esta tira y la de las demás).
            let drag = IconDrag {
                panel,
                id: response.id,
                rect: response.rect,
            };
            ctx.data_mut(|d| d.insert_temp(icon_drag_id(), drag));
            let delta = response.total_drag_delta().unwrap_or_default();
            ghost(ui, skin, panel, response.rect.translate(delta.round()));
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            out.dragging = outside.map(|p| (panel, p));
            along_strip = over.map(|p| (panel, p, false));
        }
        // (Lo recordado sigue este frame: las tiras que se dibujan después también muestran cómo
        // queda; desde el siguiente, `icon_drag` lo olvida.)
        if response.drag_stopped() {
            if let Some(p) = outside {
                out.request = Some(StripRequest::Drop(panel, p));
            }
            along_strip = over.map(|p| (panel, p, true));
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
    }
    // Lo que dice dónde cae lo que se suelta: los botones en reposo (con la vista previa, los
    // recordados de antes, que no se mueven con ella).
    let this = if previewing {
        rest
    } else {
        ctx.data_mut(|d| d.insert_temp(rest_id(edge), buttons.clone()));
        StripArea {
            edge,
            rect: own,
            buttons,
        }
    };
    // Un ícono soltado a lo largo de la tira: a ese lugar (más allá del último a la vista, al
    // final; en su mismo lugar, nada), como si cayera de otra tira. Es el de la vista previa.
    if let Some((panel, p, true)) = along_strip {
        let at = panels.iter().position(|&q| q == panel);
        let index = this
            .drop_index(panel, p)
            .min(panels.len().saturating_sub(1));
        if Some(index) != at {
            // El lugar no lo cuenta; el hueco de `Reorder`, sí.
            let gap = if at.is_some_and(|at| index >= at) {
                index + 1
            } else {
                index
            };
            out.request = Some(StripRequest::Reorder(panel, gap));
        }
    }
    out.buttons = this.buttons;
    let Some(panel) = open_panel(&ctx).filter(|p| panels.contains(p)) else {
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
                plain_icon(ui, skin, panel.spec().icon, panel.spec().plain, button, 1.0);
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
    fn a_panel_lands_in_its_place_counted_without_it() {
        use crate::panel_registry::PanelId::{Groom, Library, Lumen, Slices};
        let strip = [Groom, Library, Lumen];
        assert_eq!(landed(&strip, Library, 0), [Library, Groom, Lumen]);
        assert_eq!(landed(&strip, Library, 1), strip, "its own place");
        assert_eq!(landed(&strip, Groom, 2), [Library, Lumen, Groom]);
        assert_eq!(
            landed(&strip, Groom, usize::MAX),
            [Library, Lumen, Groom],
            "past the end"
        );
        assert_eq!(
            landed(&strip, Slices, 1),
            [Groom, Slices, Library, Lumen],
            "from elsewhere"
        );
        assert_eq!(landed(&[], Slices, 3), [Slices]);
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
