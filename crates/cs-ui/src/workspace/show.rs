//! El workspace en pantalla: [`show`] arma el `DockArea` con la piel en toda el área central,
//! hospeda cada panel ([`Viewer`]; el lienzo, con `ChimpApp::canvas_area`; la toolbar, con su
//! scroll y sus márgenes de siempre; el timeline debajo del lienzo, con el marco del panel de abajo
//! de antes; el Preview solo en su ventana, con la del diseño: ver [`PreviewWindow`]) y pega lo que
//! va encima de egui_dock (las pestañas y la × del kit, la franja de agarre y el botón « de cada
//! grupo, los divisores de la toolbar y del timeline tapados: ver [`cover_tools_divider`] y
//! [`cover_timeline_divider`], el marco y la barra de la ventana del Preview y la tira de íconos:
//! `ChimpApp::strip_area`). También lleva el ancho de la columna derecha (la preferencia del
//! usuario, `view.palette_width`, el que tomó sola, [`host_width`], y el arrastre de su divisor) y
//! el alto del timeline (`view.timeline_height`, ver [`timeline_height`]).

use std::collections::HashMap;

use eframe::egui;
use egui_dock::{DockArea, DockState, TabViewer};

use super::drop::{dragged_tab, tab_drag_released};
use super::fit::{
    Minima, block_canvas_drag, chrome, fit_splits, fit_windows, fractions, half, minima, pin_sides,
    quadrant, record_sides, tidy_canvas, toolbar_width,
};
use super::{NodeIndex, SurfaceIndex, Workspace, dock_id, skin};
use crate::app_panels::{PanelAction, PanelCtx};
use crate::panel_registry::{Body, PanelId};
use crate::skin::Skin;
use crate::{CanvasArea, ChimpApp};
use crate::{dock, kit, panels, pixel_ui};

/// Lo que dice la pestaña del panel 3D (acoplado o flotando) en la pestaña de un modelo, donde lo
/// hospeda su panel fijo (`"model_panel"`).
pub(crate) const MODEL_ELSEWHERE: &str = "Shown in the 3D panel";

/// Lo que egui_dock ve de los paneles: los dibuja con [`ChimpApp::draw_panel`] (el lienzo, con
/// `ChimpApp::canvas_area`) y junta lo que piden (se aplica después del `DockArea`, con la app
/// entera).
struct Viewer<'a> {
    app: &'a mut ChimpApp,
    modal: bool,
    /// Con Pixel UI: las pestañas del kit van encima (las de egui_dock, transparentes).
    skinned: bool,
    /// El ancho de la pestaña del kit de cada panel.
    widths: &'a HashMap<PanelId, f32>,
    /// El margen de cada panel en su grupo: el del panel derecho de hoy para Color, el del panel
    /// izquierdo de hoy para la toolbar (ver [`tools_margin`]), el de los paneles del dock para
    /// los demás.
    color_margin: egui::Margin,
    tools_margin: egui::Margin,
    /// Si la toolbar flota: su scroll se encoge a lo ancho (en una ventana, uno que ocupa todo lo
    /// que hay la haría crecer a lo ancho de fábrica de egui).
    tools_floating: bool,
    /// Los paneles pegados al divisor izquierdo de la columna derecha y el ancho del divisor: su
    /// margen izquierdo cuenta desde la línea, como en el panel derecho de antes (la línea era su
    /// borde, por dentro del panel).
    beside: Vec<PanelId>,
    sep: f32,
    /// Si el timeline va justo debajo del lienzo (ver `Workspace::timeline_below`): ahí va sin
    /// margen en su hoja y con el marco del panel de abajo de antes ([`timeline_frame`]), sin lo
    /// que ocupa la tira (`strip_room`, ver `dock::strip_room`), que baja a su derecha.
    timeline_below: bool,
    strip_room: f32,
    /// El margen del timeline en cualquier otro lugar.
    timeline_margin: egui::Margin,
    /// Cuánto menos margen lleva el lienzo abajo con el timeline justo debajo (el divisor ocupa
    /// ese aire) y cuánto menos el timeline arriba (sin Pixel UI, el divisor es su línea y un
    /// punto de su margen).
    canvas_bottom_less: i8,
    timeline_top_less: i8,
    /// Lo que dejó la hoja del lienzo, si se dibujó.
    canvas: Option<CanvasArea>,
    /// El Preview solo en su ventana: la de antes (ver [`PreviewWindow`]).
    preview_window: Option<PreviewWindow>,
    actions: Vec<PanelAction>,
    hits: Vec<skin::TabHit>,
    /// Los que egui_dock cerró (sin Pixel UI, con su ×): van a la tira.
    closed: Vec<PanelId>,
    /// Lo que ocupó a lo ancho el contenido de cada panel dibujado (ver [`host_width`]).
    extents: Vec<Extent>,
    /// El estilo del `Ui` anfitrión: egui_dock arma el cuerpo de cada pestaña con `Ui::new`, que
    /// toma el del contexto, y en el frame en que Pixel UI cambia el estilo los dos no coinciden
    /// (el panel derecho de antes usaba el del anfitrión, y de ese frame sale su ancho).
    style: std::sync::Arc<egui::Style>,
}

impl TabViewer for Viewer<'_> {
    type Tab = PanelId;

    fn id(&mut self, tab: &mut PanelId) -> egui::Id {
        egui::Id::new(("workspace_tab", tab.key()))
    }

    /// Con Pixel UI, el título casi sin ancho (fuente de 1 pt, transparente): el ancho de la
    /// pestaña lo pone `minimum_width` (el del kit) y no el texto más los 16 de aire de egui_dock.
    /// El nombre para los lectores de pantalla va en `on_tab_button`... después del `DockArea`.
    fn title(&mut self, tab: &mut PanelId) -> egui::WidgetText {
        let title = tab.spec().title;
        if self.skinned {
            egui::RichText::new(title)
                .size(1.0)
                .color(egui::Color32::TRANSPARENT)
                .into()
        } else {
            title.into()
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut PanelId) {
        let id = *tab;
        let spacing = ui.spacing().item_spacing;
        ui.set_style(self.style.clone());
        ui.spacing_mut().item_spacing = spacing;
        // El lienzo (y el panel 3D fijo de la pestaña de un modelo): lo de antes del
        // `CentralPanel`.
        if id == PanelId::Canvas {
            let app = &mut *self.app;
            let rect = app.canvas_area(ui, self.modal, self.canvas_bottom_less, &mut self.actions);
            self.canvas = Some(CanvasArea {
                rect,
                dock: Default::default(),
            });
            return;
        }
        // El timeline justo debajo del lienzo: lo de adentro del panel de abajo de antes (su
        // marco, sin la punta de la tira, recortado a eso). En otro lugar, como los demás.
        if id == PanelId::Timeline && self.timeline_below {
            let app = &mut *self.app;
            let panel = PanelCtx {
                modal: self.modal,
                bottom: None,
                window: None,
            };
            let shared_skin = app.pixel.shared_skin();
            let frame = timeline_frame(ui.style(), shared_skin.as_deref(), self.timeline_top_less);
            let body = ui.max_rect();
            let rect = body.with_max_x(body.right() - self.strip_room);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("timeline")
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(rect.intersect(ui.clip_rect()));
            let inner = frame.show(&mut child, |ui| {
                // Como `egui::Panel::bottom`: lo de adentro, a lo ancho del panel.
                ui.set_min_width(ui.max_rect().width());
                app.draw_panel(ui, id, &panel)
            });
            // New Tilemap Layer: su diálogo (después, con la app entera).
            self.actions.extend(inner.inner);
            return;
        }
        // La toolbar: lo de adentro del panel izquierdo de antes (su scroll; el margen se lo pone
        // `tab_style_override`). Lo que apaga el modal, `draw_panel`.
        if id == PanelId::Tools {
            let panel = PanelCtx {
                modal: self.modal,
                bottom: None,
                window: None,
            };
            let app = &mut *self.app;
            let mut area = panels::tools_scroll_area();
            if self.tools_floating {
                area = area.auto_shrink([true, false]);
            }
            area.show(ui, |ui| {
                app.draw_panel(ui, id, &panel);
            });
            return;
        }
        // El Preview solo en su ventana: lo de adentro de su ventana de antes, en ella (lo de
        // afuera, desde lo de adentro de su grupo). Con Pixel UI su marco va debajo, en el lugar
        // que se deja aquí, y su barra encima, después del `DockArea`. Acoplado (o con otras
        // pestañas), como los demás.
        if id == PanelId::Preview
            && let Some(window) = &mut self.preview_window
        {
            let rect = egui::Rect::from_min_size(ui.max_rect().min - window.inset, window.size);
            // El primer frame de su ventana es la pasada de medir de egui (su área, en otro lugar):
            // egui_dock arma el cuerpo con `Ui::new`, que no la hereda; lo de adentro, tampoco se ve.
            if window.measuring {
                ui.set_invisible();
            }
            if self.skinned {
                let mut painter = ui.painter().clone();
                painter.set_clip_rect(rect);
                let background = painter.add(egui::Shape::Noop);
                window.drawn = Some((rect, painter, background));
            }
            let panel = PanelCtx {
                modal: self.modal,
                bottom: None,
                window: Some(rect),
            };
            // Su × (sin Pixel UI): cerrarlo, como la de las pestañas.
            if let Some(PanelAction::ClosePreview) = self.app.draw_panel(ui, id, &panel) {
                self.closed.push(id);
            }
            return;
        }
        // Una instancia por panel por frame (su estado y los ids de `forms` son globales): en la
        // pestaña de un modelo, al 3D lo hospeda su panel fijo; aquí, solo dónde está.
        if id == PanelId::Model && self.app.doc.editor.model_edit_on() {
            ui.add_enabled(false, egui::Label::new(MODEL_ELSEWHERE));
            return;
        }
        // El fondo del grupo (dentro del scroll de egui_dock el `max_rect` puede no serlo): hasta
        // ahí llega la lista de Lospec.
        let bottom = ui.clip_rect().bottom().min(ui.max_rect().bottom()).floor();
        // El cuerpo (sin el margen), para comparar con lo que ocupa el contenido.
        let body = ui.max_rect();
        let panel = PanelCtx {
            modal: self.modal,
            bottom: Some(bottom),
            window: None,
        };
        // En un `scope`: su rect es lo que ocupó el contenido (el `Ui` del cuerpo ya viene
        // agrandado al cuerpo entero).
        let app = &mut *self.app;
        let inner = ui.scope(|ui| match id.spec().body {
            Body::Fixed => {
                ui.set_min_height(ui.available_height());
                app.draw_panel(ui, id, &panel)
            }
            Body::Fit | Body::Tall => {
                let area = egui::ScrollArea::vertical()
                    .id_salt(("workspace_scroll", id.key()))
                    .auto_shrink([false, false]);
                pixel_ui::scroll(ui, area, |ui| {
                    exact_width(ui, id, |ui| app.draw_panel(ui, id, &panel))
                })
            }
        });
        self.actions.extend(inner.inner);
        // Solo de Color, el que vivía en el panel derecho: de lo que ocupa sale el ancho de
        // fábrica de la columna (ver `host_width`). Los demás no la ensanchan: la columna ya
        // respeta su mínimo.
        if id == PanelId::Color {
            self.extents.push(Extent {
                id,
                delta: inner.response.rect.right() - body.right(),
            });
        }
    }

    fn on_tab_button(&mut self, tab: &mut PanelId, response: &egui::Response) {
        self.hits.push(skin::TabHit {
            id: *tab,
            response: response.clone(),
        });
    }

    fn on_close(&mut self, tab: &mut PanelId) -> egui_dock::tab_viewer::OnCloseResponse {
        self.closed.push(*tab);
        // El timeline y el Preview los saca `Workspace::sync_panel`, que recuerda dónde estaban.
        if matches!(*tab, PanelId::Timeline | PanelId::Preview) {
            egui_dock::tab_viewer::OnCloseResponse::Ignore
        } else {
            egui_dock::tab_viewer::OnCloseResponse::Close
        }
    }

    fn is_closeable(&self, tab: &PanelId) -> bool {
        tab.spec().closable
    }

    /// El lienzo no flota (egui_dock tampoco lo deja arrastrar: ver `fit::block_canvas_drag`).
    fn allowed_in_windows(&self, tab: &mut PanelId) -> bool {
        *tab != PanelId::Canvas
    }

    fn tab_style_override(
        &self,
        tab: &PanelId,
        global: &egui_dock::TabStyle,
    ) -> Option<egui_dock::TabStyle> {
        let mut style = global.clone();
        if self.skinned {
            style.minimum_width = self.widths.get(tab).copied();
        }
        let mut margin = match tab {
            // El lienzo, de borde a borde (sus márgenes son los del `CentralPanel` de antes).
            PanelId::Canvas => egui::Margin::ZERO,
            PanelId::Color => self.color_margin,
            PanelId::Tools => self.tools_margin,
            // Debajo del lienzo, el marco lo pone el timeline (el del panel de abajo de antes).
            PanelId::Timeline if self.timeline_below => egui::Margin::ZERO,
            PanelId::Timeline => self.timeline_margin,
            // Solo en su ventana, lo de adentro donde va en su ventana de antes.
            PanelId::Preview => match &self.preview_window {
                Some(w) => w.margin,
                None => egui::Margin::same(dock::FLYOUT_MARGIN),
            },
            _ => egui::Margin::same(dock::FLYOUT_MARGIN),
        };
        if self.beside.contains(tab) {
            margin.left = (f32::from(margin.left) - self.sep).max(0.0) as i8;
        }
        style.tab_body.inner_margin = margin;
        Some(style)
    }

    /// El Preview solo en su ventana lleva el fondo de su ventana de antes (el del diálogo o el
    /// de la `egui::Window`).
    fn clear_background(&self, tab: &PanelId) -> bool {
        !(*tab == PanelId::Preview && self.preview_window.is_some())
    }

    fn scroll_bars(&self, _tab: &PanelId) -> [bool; 2] {
        // El scroll es de cada panel (el del kit, o el suyo propio).
        [false, false]
    }
}

/// El Preview solo en su ventana (`Workspace::bare_preview`): su ventana es la suya de antes, no
/// la de las ventanas del workspace. La ventana de egui_dock queda como contenedor (lugar, capa,
/// acoplar): del tamaño de la de antes (`Workspace::place_preview`), sin barra de pestañas y sin
/// el fondo de su grupo; lo de adentro va donde iba en la de antes.
/// - Con Pixel UI, la del diseño (el marco `window` y su barra de título con play, centrar y la
///   ○): el marco de egui_dock (`menu.side`) se borra de su capa (`skin::hide_window_frame`), se
///   arrastra para acoplarlo de su franja de arriba (como un grupo acoplado solo) y el marco y la
///   barra del diseño se pintan aparte (`Preview::window_chrome`).
/// - Sin él, la `egui::Window` de antes: su marco es el de la ventana de egui_dock (el mismo
///   `Frame::window`) y adentro, su barra de título, "Play" y la vista (`Preview::plain_window`).
///   Se mueve de la barra (como toda ventana de egui_dock sin barra de pestañas); sin Pixel UI los
///   grupos sin barra no tienen franja de agarre, así que no se acopla arrastrándola.
struct PreviewWindow {
    /// Lo que mide, con su marco.
    size: egui::Vec2,
    /// Del borde de la ventana a lo de adentro de su grupo, y el margen de lo de adentro en su
    /// grupo (lo que hay de más del marco de egui_dock hasta donde va en la de antes).
    inset: egui::Vec2,
    margin: egui::Margin,
    /// Con Pixel UI: lo que hay arriba en el marco de egui_dock (su franja de agarre va ahí).
    frame_top: f32,
    /// Su ventana es nueva: este frame egui la mide sin mostrarla (ver [`PreviewWindow::new`]).
    measuring: bool,
    /// Con Pixel UI, lo que dejó al dibujarse: su rect y el pintor de su capa con el lugar de su
    /// marco.
    drawn: Option<(egui::Rect, egui::Painter, egui::layers::ShapeIdx)>,
}

impl PreviewWindow {
    /// La de antes del Preview, solo en su ventana: con Pixel UI, la del diseño (su diálogo, con la
    /// diferencia hasta el marco de las ventanas del workspace de margen); sin él, la de egui (el
    /// mismo marco que el de egui_dock, sin margen de más). `surface`: su ventana; si su área de
    /// egui todavía no existe, este frame es su pasada de medir (en el lugar de fábrica de egui,
    /// con el tamaño de fábrica de un área: la esquina se aplica desde el frame siguiente).
    fn new(
        ui: &egui::Ui,
        skin: Option<&Skin>,
        preview: &crate::preview::Preview,
        surface: SurfaceIndex,
    ) -> Self {
        let measuring = egui::AreaState::load(ui.ctx(), skin::window_area_id(surface)).is_none();
        match skin {
            Some(skin) => {
                let dialog = pixel_ui::dialog_margin(skin);
                let frame = skin::flyout_margin(skin);
                PreviewWindow {
                    size: preview.window_size(ui, skin),
                    inset: egui::vec2(dialog.leftf(), dialog.topf()),
                    margin: egui::Margin {
                        left: dialog.left - frame.left,
                        right: dialog.right - frame.right,
                        top: dialog.top - frame.top,
                        bottom: dialog.bottom - frame.bottom,
                    },
                    frame_top: frame.topf(),
                    measuring,
                    drawn: None,
                }
            }
            None => {
                let frame = egui::Frame::window(ui.style());
                let edge = frame.stroke.width;
                let margin = frame.inner_margin;
                PreviewWindow {
                    size: preview.plain_window_size(ui),
                    inset: egui::vec2(edge + margin.leftf(), edge + margin.topf()),
                    margin: egui::Margin::ZERO,
                    frame_top: 0.0,
                    measuring,
                    drawn: None,
                }
            }
        }
    }
}

/// El marco del timeline justo debajo del lienzo: el del panel de abajo de antes. Con Pixel UI, el
/// del diseño (`timeline::pixel_panel_frame`); sin él, el de los paneles de egui con `top_less`
/// menos de margen arriba (el divisor ocupa su línea y ese punto: ver [`cover_timeline_divider`]).
fn timeline_frame(style: &egui::Style, skin: Option<&Skin>, top_less: i8) -> egui::Frame {
    match skin {
        Some(skin) => crate::timeline::pixel_panel_frame(skin),
        None => {
            let mut frame = egui::Frame::side_top_panel(style);
            frame.inner_margin.top = (frame.inner_margin.top - top_less).max(0);
            frame
        }
    }
}

/// El alto del timeline (el del panel de abajo de antes, con su borde): el elegido
/// (`view.timeline_height`) o, si todavía no hay uno, el que egui recordaba del panel de antes
/// (su `PanelState`, guardado con la memoria de egui: así no cambia al actualizar) o el de fábrica.
fn timeline_height(app: &ChimpApp, ctx: &egui::Context) -> f32 {
    app.view
        .timeline_height
        .or_else(|| {
            egui::containers::panel::PanelState::load(ctx, egui::Id::new("timeline"))
                .map(|state| state.outer_rect.height())
                .filter(|h| h.is_finite() && *h > 0.0)
        })
        .unwrap_or(dock::TIMELINE_DEFAULT_HEIGHT)
}

/// El contenido de un panel con scroll, justo al ancho que le queda en `ui` (el del grupo sin la
/// barra): un `ScrollArea` crece a lo ancho de su contenido y, con él, su barra se iría fuera del
/// grupo. Lo que se pase se recorta (no debería: la columna respeta su mínimo).
fn exact_width<R>(ui: &mut egui::Ui, id: PanelId, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let width = ui.available_width();
    let start = ui.cursor().min;
    let max = egui::Rect::from_min_size(start, egui::vec2(width, ui.available_height()));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("workspace_body", id.key()))
            .max_rect(max),
    );
    let out = add(&mut child);
    let height = child.min_rect().bottom() - start.y;
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(start, egui::vec2(width, height)));
    out
}

/// Lo que el contenido de un panel dibujado se pasó a lo ancho del borde derecho de su cuerpo
/// (negativo si le sobró lugar).
#[derive(Clone, Copy, Debug)]
struct Extent {
    id: PanelId,
    delta: f32,
}

/// El ancho que toma sola la columna derecha (desde su línea): como el panel derecho de antes con
/// Color adentro, que se ajustaba a lo que ocupaba su contenido; de ese ajuste sale el ancho de
/// fábrica de la columna en los primeros frames, cuando la escala y la fuente de Pixel UI todavía
/// cambian. El ancho de la columna (`width`) más lo que se pase el contenido de un panel de la
/// columna; si no era de puntos enteros (`whole` falso, un frame de transición), redondeado hacia
/// arriba. Si al contenido le sobra lugar no encoge: un panel angosto no achica la columna.
fn host_width(width: f32, whole: bool, docked: impl Iterator<Item = Extent>) -> f32 {
    let width = width + docked.map(|e| e.delta).fold(0.0, f32::max);
    if whole { width } else { width.ceil() }
}

/// El workspace en `ui` (toda el área que le queda: el lienzo y lo que lo rodea): el `DockArea`
/// con la piel (con Pixel UI) y cada panel con [`ChimpApp::draw_panel`]. Lo que piden los paneles
/// se aplica al final, con la app entera; cerrar una pestaña la regresa a la tira. Con un diálogo
/// modal (`modal`), los paneles quedan apagados y las pestañas no se arrastran. Devuelve lo que
/// dejó la hoja del lienzo (su rect y lo de la tira), si se dibujó.
///
/// Mientras los paneles se dibujan, `app.workspace` es un acomodo de relleno (el de verdad lo
/// tiene `show`): cambiarlo desde un panel no serviría, por eso sus operaciones lo revisan en
/// debug. Lo que haya que cambiar del acomodo se pide como acción y se hace después.
///
/// La columna derecha (el lado derecho del lienzo, ver [`Workspace::column`]) va del ancho
/// elegido (`view.palette_width`, que solo cambia al arrastrar su divisor) o, sin uno, del que
/// tomó sola (`palette_natural_width`); nunca menos que su mínimo (sus paneles en su mínimo).
pub(crate) fn show(app: &mut ChimpApp, ui: &mut egui::Ui, modal: bool) -> Option<CanvasArea> {
    let ctx = ui.ctx().clone();
    let mut workspace = std::mem::replace(&mut app.workspace, Workspace::placeholder());
    // El timeline y el Preview, en el árbol si se quieren ver y tienen sentido (si no, fuera,
    // recordando dónde estaban): lo que hacen View → Timeline, sus botones de la tira y su atajo.
    // Apagado, el Preview no sigue reproduciendo (en la pestaña de un modelo se oculta sin
    // apagarse, como su ventana de antes).
    let model_tab = app.doc.editor.model_edit_on();
    workspace.sync_panel(
        PanelId::Timeline,
        app.view.timeline && PanelId::Timeline.available(app),
    );
    workspace.sync_panel(
        PanelId::Preview,
        app.view.preview && PanelId::Preview.available(app),
    );
    // Apagado de View → Preview o de su botón de la tira (que solo cambian `view.preview`), no
    // sigue reproduciendo; cerrarlo lo para `PanelAction::ClosePreview`.
    if !app.view.preview {
        app.preview.stop();
    }
    workspace.tidy();
    tidy_canvas(&mut workspace.state, None);
    workspace.place_restored_windows(&ctx);
    workspace.place_new_windows();
    workspace.refresh_tab_bars();
    let shared_skin = app.pixel.shared_skin();
    let skin: Option<&Skin> = shared_skin.as_deref();
    // El Preview flotando: solo en su ventana, del tamaño de su ventana de antes; en su esquina
    // hasta que se lo mueve.
    let bare_preview = workspace.bare_preview();
    let preview_window = bare_preview.map(|s| PreviewWindow::new(ui, skin, &app.preview, s));
    let anchored = workspace.place_preview(&ctx, preview_window.as_ref().map(|w| w.size));
    let preview_surface = workspace.leaf_of(PanelId::Preview).map(|(s, _)| s);
    let widths: HashMap<PanelId, f32> = match skin {
        Some(_) => PanelId::ALL
            .iter()
            .map(|&p| (p, kit::tabs::natural_width(ui, &skin::tab_item(p))))
            .collect(),
        None => HashMap::new(),
    };
    let style = match skin {
        Some(skin) => skin::dock_style(ui.style(), skin),
        None => skin::plain_style(ui.style()),
    };
    let (sep, bar) = chrome(ui, skin);
    // El mínimo del lienzo es lo que lleva su hoja este frame (la tira o el panel 3D fijo):
    // ningún lado se lo quita (ver `fit_splits` y `pin_sides`).
    let canvas_min = app.canvas_area_min();
    // La tira de íconos va a la derecha del lienzo (y del timeline, si va justo debajo); en la
    // pestaña de un modelo, el panel 3D fijo (dentro de la hoja del lienzo).
    let strip_width = if model_tab { 0.0 } else { dock::WIDTH };
    let strip_room = if model_tab {
        0.0
    } else {
        dock::strip_room(ui.style(), skin.is_some())
    };
    let m = workspace_minima(ui, skin, &workspace, sep, canvas_min, strip_width);
    // La columna derecha: del ancho elegido (o el que tomó sola), nunca menos que su mínimo (con su
    // divisor, que es la línea que el panel derecho tenía de borde). Se pide en puntos al centro
    // del divisor.
    let column_min = workspace.column_min(&m, sep, bar) + sep;
    let preferred = app
        .view
        .palette_width
        .or(app.palette_natural_width)
        .unwrap_or(crate::color_panel::DEFAULT_WIDTH);
    let column_width = preferred.max(column_min).max(1.0);
    workspace.sides.sep = sep;
    workspace.sides.bar = bar;
    workspace.sides.mins = m.clone();
    workspace.sides.column = Some(column_width - sep / 2.0);
    // La toolbar, del ancho de su caja (la del skin), como el panel izquierdo de antes.
    workspace.sides.toolbar = Some(toolbar_width(skin));
    // El timeline, de su alto (hasta el centro del divisor). Justo debajo del lienzo, el divisor
    // está donde estaba el borde del panel de abajo de antes: con Pixel UI, en el aire de abajo
    // del lienzo (fuera del panel); sin él, en su línea y un punto de su margen (dentro).
    let below = workspace.timeline_below();
    let overlap = if below && skin.is_none() { sep } else { 0.0 };
    let line = ui.visuals().widgets.noninteractive.bg_stroke.width.round();
    workspace.sides.timeline = Some(timeline_height(app, &ctx) + sep / 2.0 - overlap);
    workspace.sides.overlap = overlap;
    workspace.sides.strip = strip_room;
    let area = ui.available_rect_before_wrap();
    // El `DockArea`, en puntos enteros (egui_dock solo ajusta a píxel físico), en todo el área.
    let dock_rect =
        egui::Rect::from_min_max(area.min.ceil(), area.max.floor().max(area.min.ceil()));
    // Nada de lo que se arrastra es el lienzo; sus lados, en puntos.
    block_canvas_drag(&ctx, dock_id(), &workspace.state);
    pin_sides(&mut workspace.state, dock_rect, &mut workspace.sides);
    let before = fractions(&workspace.state);
    let was_floating: Vec<PanelId> = workspace
        .state
        .iter_all_tabs()
        .filter(|(path, _)| !path.surface.is_main())
        .map(|(_, p)| *p)
        .collect();
    // Mientras se arrastra una pestaña sobre el lienzo: egui_dock pinta el lienzo entero ("como
    // pestaña"), pero lo que pasa es que se parte al lado (`tidy_canvas`): se pinta esa mitad.
    let pointer = ctx.input(|i| i.pointer.latest_pos());
    let canvas_drop = match (dragged_tab(&ctx), pointer) {
        (Some(_), Some(p)) => workspace
            .state
            .find_tab(&PanelId::Canvas)
            .and_then(|path| workspace.state.leaf(path.node_path()).ok())
            .map(|leaf| leaf.rect)
            .filter(|r| r.contains(p))
            .map(|r| (r, half(r, quadrant(r, p)))),
        _ => None,
    };
    let mut viewer = Viewer {
        app,
        modal,
        skinned: skin.is_some(),
        widths: &widths,
        color_margin: egui::Frame::side_top_panel(ui.style()).inner_margin,
        tools_margin: tools_margin(ui.style(), skin, workspace.tools_side(), sep),
        tools_floating: workspace.floating(PanelId::Tools),
        beside: workspace.beside_column(),
        sep,
        timeline_below: below,
        strip_room,
        timeline_margin: match skin {
            Some(_) => egui::Margin::same(pixel_ui::WORKSPACE_MARGIN),
            None => egui::Frame::side_top_panel(ui.style()).inner_margin,
        },
        canvas_bottom_less: if below { (sep - overlap) as i8 } else { 0 },
        timeline_top_less: (overlap - line).max(0.0) as i8,
        canvas: None,
        preview_window,
        actions: Vec::new(),
        hits: Vec::new(),
        closed: Vec::new(),
        extents: Vec::new(),
        style: ui.style().clone(),
    };
    let strip = workspace.strip.clone();
    {
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(dock_rect));
        // El marco de las ventanas flotantes (solo lo leen los marcos).
        if let Some(skin) = skin {
            skin::window_visuals(ui.style_mut(), skin);
        }
        DockArea::new(&mut workspace.state)
            .id(dock_id())
            .style(style)
            // Con Pixel UI, la × es la del kit (abajo); sin él, la de egui_dock.
            .show_close_buttons(skin.is_none())
            .show_add_buttons(false)
            .tab_context_menus(false)
            .show_leaf_close_all_buttons(false)
            .show_leaf_collapse_buttons(false)
            .secondary_button_on_modifier(false)
            .secondary_button_context_menu(false)
            .show_secondary_button_hint(false)
            .show_tab_name_on_hover(false)
            .hidable_tab_bars(true)
            .draggable_tabs(!modal)
            .show_inside(ui, &mut viewer);
    }
    ui.advance_cursor_after_rect(area);
    // El marco y la barra de la ventana del Preview (con Pixel UI, solo en ella), en su capa y
    // encima de lo de ella: sus controles le ganan a la franja de agarre de su grupo (la de arriba,
    // que egui_dock registra después de lo de adentro). El marco de egui_dock, ahí no.
    if skin.is_some()
        && let Some(surface) = bare_preview
    {
        skin::hide_window_frame(&ctx, surface);
    }
    if let Some(PreviewWindow {
        drawn: Some((rect, painter, background)),
        frame_top,
        ..
    }) = viewer.preview_window.take()
        && let Some(skin) = skin
    {
        let mut bar = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("workspace_preview")
                .layer_id(painter.layer_id())
                .max_rect(rect),
        );
        bar.set_clip_rect(rect);
        // Como lo de adentro: invisible en la pasada de medir de su ventana, y con su opacidad.
        if !painter.is_visible() {
            bar.set_invisible();
        }
        bar.multiply_opacity(painter.opacity());
        // De su barra de título se mueve (como su ventana de antes): un widget encima de la ventana
        // de egui_dock, debajo de la franja de agarre de su grupo (la de arriba, de la que se lleva a
        // acoplar), que con el radio de egui se quedaría también con lo de abajo de ella. Los
        // controles de la barra, registrados después, le ganan.
        let top = rect.top() + frame_top + skin::GRAB_STRIP;
        let title = egui::Rect::from_x_y_ranges(
            rect.x_range(),
            top..=rect.top() + crate::kit::title_bar::HEIGHT,
        );
        let drag = bar.interact(
            title,
            egui::Id::new("workspace_preview_title"),
            egui::Sense::drag(),
        );
        if drag.dragged()
            && let Some((s, _)) = workspace.leaf_of(PanelId::Preview)
            && let Some(window) = workspace.state.get_window_state_mut(s)
        {
            window.set_position(rect.min + drag.drag_delta());
            workspace.preview_anchored = false;
            ctx.request_repaint();
        }
        let app = &mut *viewer.app;
        let editor = &app.doc.editor;
        if app
            .preview
            .window_chrome(&mut bar, editor, skin, rect, (&painter, background))
        {
            viewer.closed.push(PanelId::Preview);
        }
    }
    // Llevado a otro lugar (acoplado o a otra ventana, arrastrando su franja): deja su esquina.
    if workspace.leaf_of(PanelId::Preview).map(|(s, _)| s) != preview_surface {
        workspace.preview_anchored = false;
    }
    // El divisor de la toolbar sola junto al lienzo: ni se ve ni se arrastra.
    let tools_divider = workspace.tools_side().zip(workspace.tools_divider(sep));
    if let Some((first, divider)) = tools_divider {
        let leaf = workspace
            .leaf_of(PanelId::Tools)
            .and_then(|(surface, node)| {
                workspace
                    .state
                    .leaf(egui_dock::NodePath { surface, node })
                    .ok()
            })
            .map_or(divider, |leaf| leaf.rect);
        cover_tools_divider(ui, skin, (leaf, divider), first, area.bottom());
    }
    // El divisor del timeline justo debajo del lienzo: en reposo no se ve.
    if workspace.timeline_below()
        && let Some(divider) = workspace.timeline_divider(sep)
    {
        cover_timeline_divider(ui, skin, divider, strip_room);
    }
    let Viewer {
        app,
        actions,
        hits,
        mut closed,
        extents,
        mut canvas,
        ..
    } = viewer;
    // La tira de íconos, de alto completo a la derecha del lienzo (y del timeline justo debajo),
    // encima de los dos y de su divisor (que ahí no se agarra), como el panel de antes.
    if !model_tab
        && let Some(canvas) = &mut canvas
        && let Some(stack) = workspace.stack_rect()
    {
        let rect = stack.with_min_x(stack.right() - strip_width);
        // El divisor del timeline de debajo no se agarra a través de la tira (el borde del panel
        // de antes terminaba en ella): encima, un widget que se queda con el puntero (sin tocar
        // la zona del divisor de la columna, a la derecha de la tira).
        if let Some(divider) = workspace.timeline_divider(sep) {
            let band = egui::Rect::from_x_y_ranges(
                rect.left()..=rect.right() - 1.0,
                divider.expand(skin::SEPARATOR_EXTRA).y_range(),
            );
            let _ = ui.interact(
                band,
                egui::Id::new("workspace_strip_block"),
                egui::Sense::click_and_drag(),
            );
        }
        let timeline = app.view.timeline;
        canvas.dock = app.strip_area(ui, rect, modal, &strip);
        // Su botón del timeline: el árbol cambia en el frame siguiente.
        if app.view.timeline != timeline {
            ctx.request_repaint();
        }
    }
    // El divisor de un lado del lienzo que arrastró el usuario: lo que mide de ahí en adelante.
    // El de la columna es su preferencia (sin bajar de su mínimo, como el panel de antes).
    let dragged = record_sides(&mut workspace.state, &before, &mut workspace.sides);
    if let Some(points) = dragged.timeline {
        // El alto del panel de antes (sin medio divisor, con lo que era suyo), a 1/64 de punto.
        let height = (points - sep / 2.0 + workspace.sides.overlap)
            .max(crate::panel_registry::TIMELINE_MIN.y);
        app.view.timeline_height = Some((height * 64.0).round() / 64.0);
    }
    let column = workspace.column_rect();
    if let Some(points) = dragged.column {
        // A 1/64 de punto: sin el ruido de ir de puntos a fracción y de vuelta (un arrastre de
        // 40 puntos guarda 40, no 39.99997), y sin correr el divisor (ver `record_sides`).
        let width = (points + sep / 2.0).max(column_min);
        app.view.palette_width = Some((width * 64.0).round() / 64.0);
    } else if app.view.palette_width.is_none()
        && let Some(rect) = column
    {
        // Sin una elegida, la que toma sola (más lo que se pase Color, si está en ella); si es más
        // que su mínimo, es suya (no la de un panel acoplado ancho).
        let in_column = workspace.column_panels();
        let docked = extents.into_iter().filter(|e| in_column.contains(&e.id));
        let whole = rect.width() == rect.width().round();
        let width = host_width(rect.width(), whole, docked);
        if width > column_min + 1.0 {
            app.palette_natural_width = Some(width);
        }
    }
    // El nombre de cada pestaña para los lectores de pantalla (el título es de 1 pt) y, con
    // Pixel UI, las del kit encima y su ×.
    let active: Vec<PanelId> = workspace
        .state
        .iter_leaves()
        .filter_map(|(_, leaf)| leaf.tabs.get(leaf.active.0).copied())
        .collect();
    for h in &hits {
        let title = h.id.spec().title;
        let selected = active.contains(&h.id);
        h.response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, !modal, selected, title)
        });
    }
    if let Some(skin) = skin {
        skin::paint_tabs(ui, skin, &workspace.state, &hits, dock_id());
        closed.extend(close_marks(ui, &workspace.state, &hits, modal));
        // La línea de arriba de cada divisor vertical que llega a la barra de opciones cruza su
        // brillo (como los separadores de la barra y el borde del panel derecho de antes).
        paint_crossings(
            ui,
            skin,
            &workspace.state,
            dock_rect,
            sep,
            tools_divider.map(|(_, d)| d),
        );
    }
    // La franja de agarre del lienzo no es para agarrarlo: sin la mano encima.
    if let Some(path) = workspace.state.find_tab(&PanelId::Canvas)
        && skin::hidden_strip_hovered(&ctx, dock_id(), path.node_path())
    {
        ctx.set_cursor_icon(egui::CursorIcon::Default);
    }
    // Dónde puede haber marcadores este frame: el DockArea y sus ventanas, fuera del cuerpo de los
    // grupos (ver `skin::Armed`).
    if let Some(skin) = &shared_skin {
        let mut armed = skin::Armed::new(skin.clone());
        armed.areas.push(dock_rect);
        for (s, _) in workspace.state.iter_surfaces_indexed() {
            if !s.is_main()
                && let Some(rect) = ctx.memory(|m| m.area_rect(skin::window_area_id(s)))
            {
                armed.areas.push(rect);
            }
        }
        armed.bodies = workspace
            .state
            .iter_leaves()
            .map(|(_, leaf)| leaf.viewport)
            .filter(|r| r.is_positive())
            .collect();
        armed.canvas_drop = canvas_drop;
        skin::arm(&ctx, armed);
    }
    for id in closed {
        match id {
            // Cerrarlo es apagarlo (como su botón de la tira y View → Timeline): sale del árbol y
            // vuelve a su lugar al prenderlo.
            PanelId::Timeline => {
                app.view.timeline = false;
                workspace.sync_panel(id, false);
                ctx.request_repaint();
            }
            // También el Preview (y deja de reproducir).
            PanelId::Preview => {
                app.apply_panel_action(&ctx, PanelAction::ClosePreview);
                workspace.sync_panel(id, false);
                ctx.request_repaint();
            }
            _ => workspace.to_strip(id),
        }
    }
    let released = tab_drag_released(&ctx, &workspace.state, &hits);
    // Lo que egui_dock soltó en el lienzo (como pestaña): al lado, del lado del cuadrante.
    let mut changed = tidy_canvas(&mut workspace.state, released.map(|(_, at)| at));
    if let Some((id, at)) = released {
        // Una pestaña (o un grupo sin barra) de egui_dock soltada fuera del árbol y de las
        // ventanas (en la barra de estado, la toolbar...): egui_dock no hace nada (solo suelta
        // sobre sus grupos); aquí se pone a flotar donde se soltó, como un ícono de la tira.
        let windows: Vec<egui::Rect> = workspace
            .state
            .iter_surfaces_indexed()
            .filter(|(s, _)| !s.is_main())
            .filter_map(|(s, _)| ctx.memory(|m| m.area_rect(skin::window_area_id(s))))
            .collect();
        let outside = !dock_rect.contains(at) && !windows.iter().any(|r| r.contains(at));
        if id != PanelId::Canvas && outside {
            workspace.float(id, at);
        } else if workspace.floating(id) && !was_floating.contains(&id) {
            // egui_dock lo puso a flotar (soltado en el anillo de "ventana nueva"): su ventana,
            // donde se soltó.
            workspace.placing.retain(|(p, _, _)| *p != id);
            workspace
                .placing
                .push((id, at, super::fit::NEW_WINDOW_FRAMES));
        }
    }
    if let Some((surface, node)) = collapse_buttons(ui, skin, &workspace, modal) {
        workspace.collapse_to_icons(surface, node);
    }
    // Si un panel se soltó en otro grupo (o junto al lienzo), la barra de algún grupo cambia: este
    // frame ya se dibujó con la de antes, así que se descarta y se vuelve a dibujar (sin el
    // parpadeo de un frame).
    changed |= workspace.refresh_tab_bars();
    if changed {
        ctx.request_discard("workspace layout changed");
    }
    // Cada split con sus lados en su mínimo, y a puntos enteros, para el frame siguiente.
    let m = workspace_minima(ui, skin, &workspace, sep, canvas_min, strip_width);
    fit_splits(&mut workspace.state, &m, sep, bar);
    fit_windows(&ctx, &mut workspace, &m, sep, bar);
    skin::snap(&ctx, &mut workspace.state);
    // La del Preview en su esquina no (excepción deliberada a los puntos enteros: va donde iba su
    // ventana de antes, a medio punto si la pantalla termina en uno).
    skin::snap_windows(&ctx, &mut workspace.state, anchored);
    app.workspace = workspace;
    for action in actions {
        app.apply_panel_action(&ctx, action);
    }
    canvas
}

/// Lo mínimo de cada panel en su grupo de `w` (ver [`minima`]): el lienzo, lo que lleva su hoja
/// este frame (`canvas`); el timeline justo debajo del lienzo, la tira (`strip`) más; los pegados
/// al divisor de la columna derecha y la toolbar junto al lienzo, un divisor (`sep`) menos (su
/// margen de ese lado cuenta desde la línea).
fn workspace_minima(
    ui: &egui::Ui,
    skin: Option<&Skin>,
    w: &Workspace,
    sep: f32,
    canvas: egui::Vec2,
    strip: f32,
) -> Minima {
    let mut m = beside_minima(minima(ui, skin), &w.beside_column(), sep);
    m.insert(PanelId::Canvas, canvas);
    if w.timeline_below()
        && let Some(min) = m.get_mut(&PanelId::Timeline)
    {
        min.x += strip;
    }
    if w.tools_side().is_some()
        && let Some(min) = m.get_mut(&PanelId::Tools)
    {
        min.x = (min.x - sep).max(0.0);
    }
    m
}

/// El margen de la toolbar en su grupo: el del panel izquierdo de antes (con Pixel UI, el aire
/// del mockup alrededor de su caja; sin él, el de los paneles de egui). Si va sola junto al lienzo
/// (`side`, ver `Workspace::tools_side`), del lado del divisor cuenta desde su borde de afuera: el
/// divisor (`sep`) está donde el panel tenía su aire (con Pixel UI) o su línea de 1 punto (sin
/// él), así lo de adentro queda donde estaba.
fn tools_margin(
    style: &egui::Style,
    skin: Option<&Skin>,
    side: Option<bool>,
    sep: f32,
) -> egui::Margin {
    let (mut margin, line) = match skin {
        Some(_) => (
            egui::Margin {
                left: pixel_ui::TOOLBAR_MARGIN,
                right: pixel_ui::TOOLBAR_MARGIN,
                top: pixel_ui::TOOLBAR_TOP_MARGIN,
                bottom: 2,
            },
            0.0,
        ),
        None => (
            egui::Frame::side_top_panel(style).inner_margin,
            style.visuals.widgets.noninteractive.bg_stroke.width.round(),
        ),
    };
    let less = (sep - line).max(0.0) as i8;
    match side {
        Some(true) => margin.right = (margin.right - less).max(0),
        Some(false) => margin.left = (margin.left - less).max(0),
        None => {}
    }
    margin
}

/// El divisor entre la toolbar sola y el lado del lienzo (`divider`; `first`: la toolbar a la
/// izquierda) se ve como el borde del panel izquierdo de antes: con Pixel UI, nada (el fondo, como
/// el aire que había); sin él, el fondo y la línea de 1 punto de egui del lado del lienzo. Y no se
/// arrastra: encima de su zona va un widget que se queda con el puntero (registrado después que
/// el divisor de egui_dock, le gana; sin el cursor de cambiar el tamaño). Su lado mide siempre el
/// ancho de la toolbar (`pin_sides`).
///
/// El `DockArea` va a puntos enteros y el área puede terminar en medio punto (sin Pixel UI, la
/// barra de estado): el panel de antes llegaba hasta `bottom`, así que si el divisor llega al
/// fondo del `DockArea` (le falta menos de un punto para `bottom`), lo de debajo de la toolbar
/// (`leaf`) y de su divisor hasta ahí también lleva su fondo (y su línea). Si no llega (la toolbar
/// junto al lienzo encima de otro grupo), nada más abajo: es de ese grupo.
fn cover_tools_divider(
    ui: &mut egui::Ui,
    skin: Option<&Skin>,
    (leaf, divider): (egui::Rect, egui::Rect),
    first: bool,
    bottom: f32,
) {
    let ppp = ui.ctx().pixels_per_point();
    // Donde lo pinta egui_dock (`show_separator`): a píxel físico.
    let x = |v: f32| (v * ppp).round() / ppp;
    let bottom = if bottom - divider.bottom() < 1.0 {
        bottom.max(divider.bottom())
    } else {
        divider.bottom()
    };
    let rect = egui::Rect::from_x_y_ranges(
        x(divider.left())..=x(divider.right()),
        divider.top()..=bottom,
    );
    ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
    if bottom > divider.bottom() {
        let below = egui::Rect::from_x_y_ranges(
            x(leaf.left().min(divider.left()))..=x(leaf.right().max(divider.right())),
            divider.bottom()..=bottom,
        );
        ui.painter()
            .rect_filled(below, 0.0, ui.visuals().panel_fill);
    }
    if skin.is_none() {
        // Como `egui::Panel` con `show_separator_line`: la línea en el borde de afuera del panel.
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        let at = if first {
            rect.right() - stroke.width / 2.0
        } else {
            rect.left() + stroke.width / 2.0
        };
        ui.painter().vline(at, rect.y_range(), stroke);
    }
    let zone = divider.expand2(egui::vec2(skin::SEPARATOR_EXTRA / 2.0, 0.0));
    let _ = ui.interact(
        zone,
        egui::Id::new("workspace_tools_divider"),
        egui::Sense::click_and_drag(),
    );
}

/// El divisor entre el lienzo y el timeline justo debajo (`divider`) se ve, en reposo, como el
/// borde del panel de abajo de antes: con Pixel UI, nada (el fondo: era el aire de abajo del
/// lienzo); sin él, el fondo y la línea de 1 punto de egui arriba, a lo ancho del panel (sin la
/// tira de `strip` de ancho, que va encima). Con el puntero encima o arrastrándolo (egui_dock pone
/// el cursor de cambiar el alto) se ve como los demás divisores. Se arrastra (es el alto del
/// timeline: ver `record_sides`).
fn cover_timeline_divider(ui: &mut egui::Ui, skin: Option<&Skin>, divider: egui::Rect, strip: f32) {
    let ctx = ui.ctx().clone();
    let resizing = ctx.output(|o| o.cursor_icon == egui::CursorIcon::ResizeVertical);
    let near = ctx.input(|i| i.pointer.latest_pos()).is_some_and(|p| {
        divider.x_range().contains(p.x) && (p.y - divider.center().y).abs() <= 4.0
    });
    if resizing && near {
        return;
    }
    let ppp = ctx.pixels_per_point();
    // Donde lo pinta egui_dock (`show_separator`): a píxel físico.
    let y = |v: f32| (v * ppp).round() / ppp;
    let rect =
        egui::Rect::from_x_y_ranges(divider.x_range(), y(divider.top())..=y(divider.bottom()));
    ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
    if skin.is_none() {
        // Como `egui::Panel::bottom` con `show_separator_line`: la línea en el borde de arriba del
        // panel, por dentro.
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        let at = rect.top() + stroke.width / 2.0;
        ui.painter()
            .hline(rect.left()..=rect.right() - strip, at, stroke);
    }
}

/// Los mínimos `m` con los de los paneles pegados al divisor izquierdo de la columna derecha
/// (`beside`) un divisor (`sep`) más angostos: su margen izquierdo cuenta desde la línea, como en el
/// panel derecho de antes (ver `Viewer::tab_style_override`), así que la columna mínima mide lo
/// mismo que el panel mínimo de antes.
fn beside_minima(mut m: Minima, beside: &[PanelId], sep: f32) -> Minima {
    for id in beside {
        if let Some(min) = m.get_mut(id) {
            min.x = (min.x - sep).max(0.0);
        }
    }
    m
}

/// La línea oscura de cada divisor vertical de la superficie principal que llega arriba del
/// `DockArea` (`area`), un punto más arriba: cruza el brillo de la línea de la barra de opciones
/// (como el borde del panel derecho de antes). El de la toolbar (`quiet`) no: no se ve (el panel
/// izquierdo de antes no tenía línea).
fn paint_crossings(
    ui: &egui::Ui,
    skin: &Skin,
    state: &DockState<PanelId>,
    area: egui::Rect,
    sep: f32,
    quiet: Option<egui::Rect>,
) {
    let ppp = ui.ctx().pixels_per_point();
    for node in state.main_surface().iter() {
        let egui_dock::Node::Horizontal(split) = node else {
            continue;
        };
        if split.rect.top() != area.top() || !split.rect.is_positive() {
            continue;
        }
        // Donde la pinta egui_dock (`show_separator`): su borde izquierdo, a píxel físico.
        let mid = split.rect.left() + split.rect.width() * split.fraction;
        let left = ((mid - sep / 2.0) * ppp).round() / ppp;
        if quiet.is_some_and(|q| (q.center().x - mid).abs() < 0.5) {
            continue;
        }
        let cross =
            egui::Rect::from_min_size(egui::pos2(left, area.top() - 1.0), egui::vec2(1.0, 1.0));
        ui.painter().add(egui::Shape::rect_filled(
            eframe::emath::GuiRounding::round_to_pixels(cross, ppp),
            0.0,
            skin.colors().canvas,
        ));
    }
}

/// Si `shape` (o lo que lleva adentro) tiene un rect con un color marcador de la piel: el plugin
/// tenía que cambiarlo por una pieza del skin.
#[cfg(test)]
pub(crate) fn has_marker(shape: &egui::Shape) -> bool {
    match shape {
        egui::Shape::Vec(v) => v.iter().any(has_marker),
        egui::Shape::Rect(r) => {
            skin::MARKERS.contains(&r.fill) || skin::MARKERS.contains(&r.stroke.color)
        }
        _ => false,
    }
}

/// El nombre del botón « de un grupo acoplado (su tooltip y su etiqueta accesible).
pub(crate) const COLLAPSE: &str = "Collapse to Icons";

/// El botón « ("Collapse to Icons") de cada grupo acoplado que tenga algo que mandar a la tira
/// (ver [`Workspace::collapsible`]), la flecha de atrás de las pestañas del kit: al final de su
/// barra de pestañas o, en un grupo sin barra, en la punta derecha de su franja de agarre mientras
/// el puntero anda por la franja o por el botón (y nada se arrastra). Devuelve el grupo cuyo
/// botón se apretó.
fn collapse_buttons(
    ui: &mut egui::Ui,
    skin: Option<&Skin>,
    w: &Workspace,
    modal: bool,
) -> Option<(SurfaceIndex, NodeIndex)> {
    use crate::pixel_ui::Tip as _;
    let ctx = ui.ctx().clone();
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let dragging = ctx.dragged_id().is_some() && ctx.input(|i| i.pointer.is_decidedly_dragging());
    // Sin otra capa encima (una ventana flotante, un menú).
    let ours = |p: egui::Pos2| ctx.layer_id_at(p).is_none_or(|l| l == ui.layer_id());
    let size = egui::Vec2::splat(kit::tabs::ARROW_SIZE);
    let mut pressed = None;
    for (path, leaf) in w.state.iter_leaves() {
        if !path.surface.is_main()
            || !leaf.rect.is_positive()
            || !w.collapsible(path.surface, path.node)
        {
            continue;
        }
        // En la barra de pestañas, en el lugar que ella le deja a la derecha (`COLLAPSE_ROOM`);
        // sin barra, a la izquierda de la barra de scroll del panel (no encima de ella).
        let right = if leaf.tabs == [PanelId::Tools] {
            // La toolbar no lleva barra de scroll del kit a la derecha: en la punta.
            leaf.rect.right() - 1.0
        } else if leaf.tab_bar_hidden {
            leaf.rect.right() - f32::from(dock::FLYOUT_MARGIN) - pixel_ui::SCROLL_BAR - 1.0
        } else {
            leaf.rect.right() - 1.0
        };
        let at = egui::pos2(right - size.x, leaf.rect.top() + kit::tabs::ARROWS_TOP);
        let rect = egui::Rect::from_min_size(at.round(), size);
        if leaf.tab_bar_hidden {
            // Solo si el puntero llegó por la franja de agarre (no desde abajo, por el panel), y
            // mientras siga en ella o en el botón.
            let strip = egui::Rect::from_min_size(
                leaf.rect.min,
                egui::vec2(leaf.rect.width(), skin::GRAB_STRIP),
            );
            let inside = |r: egui::Rect| pointer.is_some_and(|p| r.contains(p) && ours(p));
            let memory = egui::Id::new(("workspace_collapse_armed", path.surface, path.node));
            let was = ctx.data(|d| d.get_temp::<bool>(memory)).unwrap_or(false);
            let armed = (was || inside(strip)) && (inside(strip) || inside(rect));
            ctx.data_mut(|d| d.insert_temp(memory, armed));
            if dragging || !armed {
                continue;
            }
        }
        let id = egui::Id::new(("workspace_collapse", path.surface, path.node));
        let response = ui
            .interact(rect, id, egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !modal, COLLAPSE));
        let hovered = !modal && response.hovered();
        match skin {
            Some(skin) => {
                let shapes = kit::tabs::arrow_shapes(ui, skin, rect, true, (hovered, !modal));
                ui.painter().extend(shapes);
            }
            None => {
                let visuals = ui.style().interact(&response);
                ui.painter()
                    .rect_filled(rect, visuals.corner_radius, visuals.bg_fill);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "«",
                    egui::TextStyle::Body.resolve(ui.style()),
                    visuals.text_color(),
                );
            }
        }
        let response = response.tip(COLLAPSE);
        if !modal && response.clicked() {
            pressed = Some((path.surface, path.node));
        }
    }
    pressed
}

/// La × del kit de cada pestaña que se puede cerrar, encima de la de egui_dock (en su capa):
/// devuelve los paneles cuya × se apretó. Solo las que se ven enteras en su barra (con muchas
/// pestañas, las de la punta quedan cortadas o fuera).
fn close_marks(
    ui: &mut egui::Ui,
    state: &DockState<PanelId>,
    hits: &[skin::TabHit],
    modal: bool,
) -> Vec<PanelId> {
    let mut closed = Vec::new();
    for h in hits {
        let item = skin::tab_item(h.id);
        if item.mark == kit::tabs::TabMark::None || skin::is_dragged_tab(&h.response) {
            continue;
        }
        let shown = state
            .find_tab(&h.id)
            .and_then(|path| state.leaf(path.node_path()).ok())
            .is_some_and(|leaf| {
                let mark = kit::tabs::mark_rect(skin::tab_row(h.response.rect));
                skin::visible_tabs(leaf.rect).contains_rect(mark)
            });
        if !shown {
            continue;
        }
        let mark = kit::tabs::mark_rect(skin::tab_row(h.response.rect)).expand(2.0);
        let mut layer = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("workspace_close", h.id.key()))
                .layer_id(h.response.layer_id)
                .max_rect(mark),
        );
        layer.set_clip_rect(mark);
        let label = format!("{} {}", item.close.label(), item.label);
        let response = layer
            .interact(
                mark,
                egui::Id::new(("workspace_close", h.id.key())),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !modal, &label));
        if !modal && response.clicked() {
            closed.push(h.id);
        }
    }
    closed
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use egui_dock::NodePath;

    use super::*;
    use crate::workspace::test_util::{button, click, frame, frames, test_app};
    use crate::workspace::{DockTarget, Workspace};

    #[test]
    fn the_host_fits_the_docked_contents_like_the_old_right_panel() {
        let e = |delta| Extent {
            id: PanelId::Color,
            delta,
        };
        // Se pasa: crece lo que se pasó.
        assert_eq!(host_width(219.0, true, [e(43.875)].into_iter()), 262.875);
        // Le sobra (o nada se dibujó): el ancho del DockArea, sin encoger.
        assert_eq!(host_width(263.0, true, [e(-40.0)].into_iter()), 263.0);
        assert_eq!(host_width(263.0, true, std::iter::empty()), 263.0);
        // Con el anfitrión en una fracción de punto: a puntos enteros, hacia arriba.
        assert_eq!(
            host_width(262.0, false, [e(-40.0), e(0.875)].into_iter()),
            263.0
        );
        assert_eq!(host_width(263.0, false, [e(0.0)].into_iter()), 263.0);
    }

    #[test]
    fn show_hides_the_bar_of_a_lone_panel_and_puts_kit_tabs_on_two() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        // Essentials: Color solo, sin barra (ni pestaña).
        let out = frames(&ctx, &mut app);
        assert!(app.pixel.skin().is_some(), "Pixel UI on");
        assert!(app.workspace.docked(PanelId::Color));
        assert!(app.workspace.tab_bar_hidden_of(PanelId::Color));
        assert!(button(&out, "Color").is_none(), "no tab for a lone panel");
        // Con Library al lado: las dos pestañas (y la × de Library; Color no se cierra).
        app.workspace.dock(PanelId::Library, DockTarget::Focused);
        let out = frames(&ctx, &mut app);
        assert!(!app.workspace.tab_bar_hidden_of(PanelId::Color));
        assert!(button(&out, "Color").is_some(), "the Color tab");
        assert!(button(&out, "Library").is_some(), "the Library tab");
        assert!(button(&out, "Close Color").is_none());
        // La × del kit la regresa a la tira.
        let close = button(&out, "Close Library").expect("Library's ×");
        frame(&ctx, &mut app, vec![click(close)]);
        frame(&ctx, &mut app, Vec::new());
        assert!(!app.workspace.docked(PanelId::Library));
        assert!(app.workspace.strip().contains(&PanelId::Library));
        assert_eq!(app.workspace.strip(), PanelId::DOCK);
    }

    #[test]
    fn a_split_and_a_floating_window_are_dressed_without_markers() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        for name in ["split", "float"] {
            app.workspace = Workspace::demo(name).unwrap();
            let out = frames(&ctx, &mut app);
            fn has_marker(shape: &egui::Shape) -> bool {
                match shape {
                    egui::Shape::Vec(v) => v.iter().any(has_marker),
                    egui::Shape::Rect(r) => {
                        skin::MARKERS.contains(&r.fill) || skin::MARKERS.contains(&r.stroke.color)
                    }
                    _ => false,
                }
            }
            assert!(
                !out.shapes.iter().any(|c| has_marker(&c.shape)),
                "{name}: every marker became a skin piece"
            );
        }
        // La ventana flotante, en puntos enteros.
        let (surface, _) = app.workspace.leaf_of(PanelId::Lumen).unwrap();
        let rect = ctx
            .memory(|m| m.area_rect(skin::window_area_id(surface)))
            .expect("Lumen's window");
        assert_eq!(rect.min, rect.min.round());
    }

    #[test]
    fn the_grab_strip_of_a_lower_lone_panel_wins_over_the_separator() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        // Groom solo, debajo de Color: los dos sin barra, con un divisor entre ellos.
        app.workspace = Workspace::demo("split").unwrap();
        let out = frames(&ctx, &mut app);
        let (surface, node) = app.workspace.leaf_of(PanelId::Groom).unwrap();
        let path = NodePath { surface, node };
        let lower = app.workspace.state.leaf(path).unwrap().rect;
        assert!(app.workspace.tab_bar_hidden_of(PanelId::Groom));
        assert!(!skin::hidden_strip_hovered(&ctx, dock_id(), path));
        assert_ne!(out.platform_output.cursor_icon, egui::CursorIcon::Grab);
        // El puntero un punto debajo del borde de arriba del grupo: la franja (no el divisor).
        let at = egui::pos2(lower.center().x, lower.top() + 1.0);
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(at)]);
        let out = frame(&ctx, &mut app, Vec::new());
        assert!(
            skin::hidden_strip_hovered(&ctx, dock_id(), path),
            "the strip is the hovered widget"
        );
        assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::Grab);
        // Y se arrastra desde ahí (no cambia el tamaño del split).
        let press = egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        };
        frame(&ctx, &mut app, vec![press]);
        for dy in [4.0, 12.0, 24.0] {
            let to = at + egui::vec2(0.0, dy);
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(to)]);
        }
        assert!(skin::hidden_strip_dragged(&ctx, dock_id(), path));
        assert_eq!(app.workspace.state.leaf(path).unwrap().rect, lower);
    }

    #[test]
    fn a_panel_action_from_a_tab_is_applied_after_the_dock() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        // Artboards junto a Color (activa): su botón Canvas Size abre el diálogo.
        app.workspace.dock(PanelId::Artboards, DockTarget::Focused);
        let out = frames(&ctx, &mut app);
        let canvas_size = button(&out, "Canvas Size").expect("Canvas Size");
        assert!(app.canvas_size.is_none());
        frame(&ctx, &mut app, vec![click(canvas_size)]);
        assert!(app.canvas_size.is_some(), "OpenCanvasSize applied");
        // Y el workspace volvió a la app.
        assert!(app.workspace.docked(PanelId::Artboards));
    }

    #[test]
    fn without_pixel_ui_the_canvas_grab_strip_takes_no_clicks() {
        // Sin Pixel UI las reglas del lienzo llegan hasta arriba de su hoja: la franja de agarre
        // (que con el lienzo no arrastra nada) no se queda con el puntero ni pone la mano.
        let mut app = test_app();
        app.view.pixel_ui = false;
        let ctx = egui::Context::default();
        frames(&ctx, &mut app);
        assert!(app.pixel.skin().is_none());
        let (surface, node) = app.workspace.leaf_of(PanelId::Canvas).unwrap();
        let leaf = app.workspace.leaf_rect(PanelId::Canvas).unwrap();
        let at = egui::pos2(leaf.center().x, leaf.top() + 1.0);
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(at)]);
        let out = frame(&ctx, &mut app, Vec::new());
        let path = NodePath { surface, node };
        assert!(!skin::hidden_strip_hovered(&ctx, dock_id(), path));
        assert_ne!(out.platform_output.cursor_icon, egui::CursorIcon::Grab);
    }
}
