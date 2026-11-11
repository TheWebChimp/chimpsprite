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
use crate::app_panels::{PanelAction, PanelCtx, PanelHost};
use crate::dock::Edge;
use crate::panel_registry::{Body, PanelId};
use crate::skin::Skin;
use crate::{CanvasArea, ChimpApp};
use crate::{dock, kit, panels, pixel_ui};

/// Lo que dice la pestaña del panel 3D (acoplado o flotando) en la pestaña de un modelo, donde lo
/// hospeda su panel fijo (`"model_panel"`; ver `ChimpApp::claim_panel`).
#[cfg(test)]
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
    /// Si la toolbar va justo a la izquierda del lienzo (ver `Workspace::tools_beside_canvas`):
    /// si no, el lienzo lleva su margen de ese lado.
    tools_beside: bool,
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
            let air = (self.canvas_bottom_less, self.tools_beside);
            let rect = app.canvas_area(ui, self.modal, air, &mut self.actions);
            self.canvas = Some(CanvasArea {
                rect,
                dock: Default::default(),
                strips: Vec::new(),
            });
            return;
        }
        // Una instancia por panel por frame (su estado y los ids de `forms` son globales): si otro
        // anfitrión ya lo tiene (en la pestaña de un modelo, al 3D lo hospeda su panel fijo), aquí
        // solo dónde está. También la toolbar, el timeline de debajo del lienzo y el Preview en
        // su ventana (sus ramas de abajo).
        if let Err(owner) = self.app.claim_panel(id, PanelHost::Workspace) {
            ui.add_enabled(false, egui::Label::new(owner.shown_in()));
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
            // egui_dock arma el cuerpo con `Ui::new`, que no la hereda; lo de adentro, tampoco se
            // ve.
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
        // Con un diálogo modal no se cierra nada (por si un clic se coló hasta su ×).
        if self.modal {
            return egui_dock::tab_viewer::OnCloseResponse::Ignore;
        }
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
    // En la pestaña de un modelo, el 3D es de su panel fijo (en la hoja del lienzo): se lo queda
    // antes de que se dibuje cualquier grupo (su pestaña, acoplada o flotando, dice dónde está).
    if model_tab {
        let _ = app.claim_panel(PanelId::Model, PanelHost::ModelPanel);
    }
    workspace.pixel = app.pixel.skin().is_some();
    workspace.sync_panel(
        PanelId::Timeline,
        app.view.timeline && PanelId::Timeline.available(app),
    );
    workspace.sync_panel(
        PanelId::Preview,
        app.view.preview && PanelId::Preview.available(app),
    );
    // Lo que Window prendió, ya en su lugar: al frente, con el foco y su ventana encima.
    if let Some(id) = workspace.pending_focus.take() {
        workspace.bring_forward(&ctx, id);
    }
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
    let mut widths: HashMap<PanelId, f32> = match skin {
        Some(_) => PanelId::ALL
            .iter()
            .map(|&p| (p, kit::tabs::natural_width(ui, &skin::tab_item(p))))
            .collect(),
        None => HashMap::new(),
    };
    // Con Pixel UI, las pestañas de cada barra donde las pone el kit (achicadas o con sus flechas
    // si no caben; ver `skin::TabBar`): egui_dock las pone igual. La rueda sobre una barra que no
    // cabe la recorre de una en una (la de egui_dock, que la correría a su manera, no llega), si
    // la barra es lo de más arriba bajo el puntero (no a través de una ventana flotante, un menú
    // o un diálogo) y no hay un diálogo modal (lo de debajo de él no se toca; su scroll, sí).
    let bars = match skin {
        Some(_) => {
            let main_layer = ui.layer_id();
            let on_top = |surface: SurfaceIndex, p: egui::Pos2| {
                let top = ctx.layer_id_at(p);
                if surface.is_main() {
                    top.is_none_or(|l| l == main_layer)
                } else {
                    top == Some(skin::window_layer(surface))
                }
            };
            // Solo en la primera pasada del frame: si se descarta, la segunda no recibe eventos
            // (`Context::run` le pasa una entrada vacía), así que no habría nada que sumar dos
            // veces; se deja por si eso cambia.
            let first = ctx.current_pass_index() == 0;
            let wheel = if modal || !first {
                None
            } else {
                tab_wheel(&ctx)
            };
            let bars = skin::plan_bars(ui, &workspace.state, wheel, on_top);
            let pointer = ctx.input(|i| i.pointer.hover_pos());
            if !modal
                && pointer.is_some_and(|p| {
                    bars.iter().any(|b| {
                        b.arrows.is_some() && b.room.contains(p) && on_top(b.path.surface, p)
                    })
                })
            {
                ctx.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
            bars
        }
        None => Vec::new(),
    };
    for bar in &bars {
        if let Ok(leaf) = workspace.state.leaf_mut(bar.path) {
            leaf.scroll = -bar.scroll;
        }
        for (i, &id) in bar.tabs.iter().enumerate() {
            widths.insert(id, bar.dock_width(i));
        }
    }
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
    // Las tiras de la izquierda, de arriba y de abajo van en su orilla, fuera del árbol: la de la
    // izquierda de alto completo y las otras dos entre ella y la orilla derecha. Vacías (o en la
    // pestaña de un modelo, donde no hay dock), no están: no ocupan nada.
    let mut inner = area;
    let mut edge_strips: Vec<(Edge, egui::Rect)> = Vec::new();
    for edge in [Edge::Left, Edge::Top, Edge::Bottom] {
        if model_tab || workspace.strip_at(edge).is_empty() {
            continue;
        }
        let rect = match edge {
            Edge::Left => inner.with_max_x(inner.left() + dock::WIDTH),
            Edge::Top => inner.with_max_y(inner.top() + dock::WIDTH),
            _ => inner.with_min_y(inner.bottom() - dock::WIDTH),
        };
        match edge {
            Edge::Left => inner.min.x = rect.right(),
            Edge::Top => inner.min.y = rect.bottom(),
            _ => inner.max.y = rect.top(),
        }
        edge_strips.push((edge, rect));
    }
    // El `DockArea`, en puntos enteros (egui_dock solo ajusta a píxel físico), en todo el área
    // (sin esas tiras).
    let dock_rect =
        egui::Rect::from_min_max(inner.min.ceil(), inner.max.floor().max(inner.min.ceil()));
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
    // La orilla de cada panel del árbol antes de dibujarlo: cerrar su pestaña lo manda a la tira de
    // esa orilla (egui_dock lo saca del árbol al cerrarla con su ×).
    let edges: Vec<(PanelId, Edge)> = workspace
        .state
        .iter_all_tabs()
        .map(|(path, &id)| (id, workspace.group_edge(path.surface, path.node)))
        .collect();
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
        tools_beside: workspace.tools_beside_canvas(),
        canvas: None,
        preview_window,
        actions: Vec::new(),
        hits: Vec::new(),
        closed: Vec::new(),
        extents: Vec::new(),
        style: ui.style().clone(),
    };
    let strip = workspace.strip_at(Edge::Right).to_vec();
    // Con un diálogo modal, lo que el `DockArea` cambiaría (ver `Frozen`) se repone después, y las
    // ventanas flotantes no se mueven arrastrando su fondo (ver `hold_windows`).
    let frozen = modal.then(|| Frozen::take(&workspace.state));
    let past_last = dropped_past_last(&ctx, &bars);
    if modal {
        // Sin Pixel UI el Preview solo en su ventana se mueve de su barra de título, que es de la
        // ventana de egui (la de antes no miraba el modal): eso sigue.
        let title = bare_preview.filter(|_| skin.is_none()).and_then(|s| {
            let area = ctx.memory(|m| m.area_rect(skin::window_area_id(s)))?;
            let bottom = area.top() + crate::preview::Preview::plain_title_bottom(ui);
            Some((s, area.with_max_y(bottom)))
        });
        hold_windows(&ctx, &workspace.state, title);
    }
    {
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(dock_rect));
        // El marco de las ventanas flotantes (solo lo leen los marcos).
        if let Some(skin) = skin {
            skin::window_visuals(ui.style_mut(), skin);
        }
        // Con un diálogo modal, lo de egui_dock de la superficie principal (divisores, pestañas y
        // sus ×, franjas de agarre) queda apagado: no recibe clics ni arrastres (tampoco los de
        // accesibilidad) y no se resalta. Sin que se vea distinto: apagar un `Ui` lo atenúa, y aquí
        // se le devuelve su opacidad. El cuerpo de cada pestaña no: egui_dock lo arma con
        // `Ui::new` (lo apaga `draw_panel`, salvo Color y el Preview, como siempre). Las ventanas
        // flotantes tampoco lo heredan (son `egui::Window` aparte): ver `modal_shields`.
        if modal {
            let opacity = ui.opacity();
            ui.disable();
            ui.set_opacity(opacity);
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
    if let Some(frozen) = frozen {
        frozen.restore(&mut workspace.state);
    }
    if let Some((id, last)) = past_last {
        to_the_end(&mut workspace.state, id, last);
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
        // de egui_dock, debajo de la franja de agarre de su grupo (la de arriba, de la que se lleva
        // a acoplar), que con el radio de egui se quedaría también con lo de abajo de ella. Los
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
    // Con un diálogo modal, las ventanas flotantes no se agarran (encima de lo de ellas).
    if modal {
        modal_shields(ui, skin, &workspace.state);
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
        cover_tools_divider(ui, skin, (leaf, divider), first, inner.bottom());
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
    // encima de los dos y de su divisor (que ahí no se agarra), como el panel de antes. Las de las
    // otras orillas van aunque esa no se pueda dibujar (sin el rect del lienzo y el timeline).
    if !model_tab && let Some(canvas) = &mut canvas {
        // Un panel que dejó las tiras (se acopló o flota) ya no se abre desde ellas.
        if crate::dock::open_panel(&ctx).is_some_and(|p| workspace.strip_of(p).is_none()) {
            crate::dock::set_open(&ctx, None);
        }
        if let Some(stack) = workspace.stack_rect() {
            let rect = stack.with_min_x(stack.right() - strip_width);
            // El divisor del timeline de debajo no se agarra a través de la tira (el borde del
            // panel de antes terminaba en ella): encima, un widget que se queda con el puntero
            // (sin tocar la zona del divisor de la columna, a la derecha de la tira).
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
            canvas.dock = app.strip_area(ui, (rect, dock_rect), modal, Edge::Right, &strip);
            canvas.strips.push((Edge::Right, rect));
            // Su botón del timeline: el árbol cambia en el frame siguiente.
            if app.view.timeline != timeline {
                ctx.request_repaint();
            }
        }
        // Las de las otras orillas (si tienen algo).
        for &(edge, rect) in &edge_strips {
            let panels = workspace.strip_at(edge).to_vec();
            let out = app.strip_area(ui, (rect, dock_rect), modal, edge, &panels);
            let dock = &mut canvas.dock;
            dock.app_panel = dock.app_panel.or(out.app_panel);
            dock.request = dock.request.or(out.request);
            dock.dragging = dock.dragging.or(out.dragging);
            canvas.strips.push((edge, rect));
        }
    }
    // El divisor de un lado del lienzo que arrastró el usuario: lo que mide de ahí en adelante.
    // El de la columna es su preferencia (sin bajar de su mínimo, como el panel de antes).
    // Un divisor que cambió en el mismo frame de un doble clic: el de egui_dock (a la mitad).
    let double_clicked = ctx.input(|i| {
        i.pointer
            .button_double_clicked(egui::PointerButton::Primary)
    });
    let dragged = record_sides(
        &mut workspace.state,
        &before,
        &mut workspace.sides,
        double_clicked,
    );
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
    // Las barras con el ancho que sus grupos tomaron en este frame: si egui_dock tendría que haber
    // puesto sus pestañas de otra forma (un grupo cambió de ancho), el frame se vuelve a dibujar.
    let bars_after = match skin {
        Some(_) => skin::plan_bars(ui, &workspace.state, None, |_, _| false),
        None => Vec::new(),
    };
    let bars_moved = bars
        .iter()
        .map(skin::TabBar::dock_layout)
        .ne(bars_after.iter().map(skin::TabBar::dock_layout));
    let bars = bars_after;
    for h in &hits {
        let title = h.id.spec().title;
        let selected = active.contains(&h.id);
        h.response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, !modal, selected, title)
        });
        // Su lugar, el de la del kit (en egui_dock la última puede llevar de más).
        if let Some(row) = bars.iter().find_map(|b| b.row(h.id)) {
            ctx.accesskit_node_builder(h.response.id, |node| {
                node.set_bounds(egui::accesskit::Rect {
                    x0: row.min.x.into(),
                    y0: row.min.y.into(),
                    x1: row.max.x.into(),
                    y1: row.max.y.into(),
                });
            });
        }
    }
    if let Some(skin) = skin {
        skin::paint_tabs(ui, skin, &workspace.state, (&hits, &bars), dock_id());
        closed.extend(close_marks(ui, &bars, &hits, modal));
        // Lo vacío al final de la barra de una ventana que no cabe: de ahí se mueve la ventana.
        if let Some((s, delta)) = tab_arrows(ui, skin, &bars, &hits, modal)
            && let Some(area) = ctx.memory(|m| m.area_rect(skin::window_area_id(s)))
            && let Some(window) = workspace.state.get_window_state_mut(s)
        {
            window.set_position(area.min + delta);
            ctx.request_repaint();
        }
        // La línea de arriba de cada divisor vertical que llega a la barra de opciones cruza su
        // brillo (como los separadores de la barra y el borde del panel derecho de antes). Con la
        // tira de arriba, el `DockArea` no llega a la barra: ese renglón es de la tira.
        if dock_rect.top() <= area.top() {
            paint_crossings(
                ui,
                skin,
                &workspace.state,
                dock_rect,
                sep,
                tools_divider.map(|(_, d)| d),
            );
        }
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
            // A la tira de la orilla de donde estaba.
            _ => match edges.iter().find(|(p, _)| *p == id) {
                Some(&(_, edge)) => workspace.to_strip_at(id, edge),
                None => workspace.to_strip(id),
            },
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
    changed |= workspace.refresh_tab_bars() | bars_moved;
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
/// (`beside`) un divisor (`sep`) más angostos: su margen izquierdo cuenta desde la línea, como en
/// el panel derecho de antes (ver `Viewer::tab_style_override`), así que la columna mínima mide lo
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
    let size = egui::Vec2::splat(kit::tabs::ARROW_SIZE);
    let mut pressed = None;
    // La toolbar sola en su ventana (con Pixel UI) también lleva el suyo (ver
    // `Workspace::bare_tools`).
    let tools_window = w.bare_tools();
    let main_layer = ui.layer_id();
    for (path, leaf) in w.state.iter_leaves() {
        let window = !path.surface.is_main();
        if (window && Some(path.surface) != tools_window)
            || !leaf.rect.is_positive()
            || !w.collapsible(path.surface, path.node)
        {
            continue;
        }
        // El de una ventana va en su capa (encima de ella).
        let layer = if window {
            egui::LayerId::new(egui::Order::Middle, skin::window_area_id(path.surface))
        } else {
            main_layer
        };
        // Sin otra capa encima (otra ventana flotante, un menú).
        let ours = |p: egui::Pos2| {
            ctx.layer_id_at(p)
                .is_none_or(|l| l == main_layer || l == layer)
        };
        // En la barra de pestañas, en el lugar que ella le deja a la derecha (`COLLAPSE_ROOM`);
        // sin barra, a la izquierda de la barra de scroll del panel (no encima de ella).
        let left = match skin {
            // La toolbar con Pixel UI: justo a la derecha de su caja (la hoja es apenas más ancha
            // que ella: en su punta la tapaba). Su caja empieza en el margen izquierdo de la hoja:
            // el de siempre o, a la derecha del lienzo, un divisor menos (ver `tools_margin`).
            Some(skin) if leaf.tabs == [PanelId::Tools] => {
                let mut margin = f32::from(pixel_ui::TOOLBAR_MARGIN);
                if !window && w.tools_side() == Some(false) {
                    margin = (margin - w.sides.sep).max(0.0);
                }
                let box_width =
                    pixel_ui::toolbar_width(skin) - 2.0 * f32::from(pixel_ui::TOOLBAR_MARGIN);
                leaf.rect.left() + margin + box_width
            }
            // Sin Pixel UI, la toolbar no lleva barra de scroll del kit a la derecha: en la punta.
            None if leaf.tabs == [PanelId::Tools] => leaf.rect.right() - 1.0 - size.x,
            _ if leaf.tab_bar_hidden => {
                leaf.rect.right()
                    - f32::from(dock::FLYOUT_MARGIN)
                    - pixel_ui::SCROLL_BAR
                    - 1.0
                    - size.x
            }
            _ => leaf.rect.right() - 1.0 - size.x,
        };
        let at = egui::pos2(left, leaf.rect.top() + kit::tabs::ARROWS_TOP);
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
        let mut on_window;
        let ui: &mut egui::Ui = if window {
            on_window = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(id)
                    .layer_id(layer)
                    .max_rect(rect),
            );
            on_window.set_clip_rect(rect);
            &mut on_window
        } else {
            &mut *ui
        };
        let mut response = ui.interact(rect, id, egui::Sense::click());
        if !modal {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
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
/// devuelve los paneles cuya × se apretó. Solo las de las pestañas que se ven en su barra (ver
/// `skin::TabBar`) y en las que cabe.
fn close_marks(
    ui: &mut egui::Ui,
    bars: &[skin::TabBar],
    hits: &[skin::TabHit],
    modal: bool,
) -> Vec<PanelId> {
    let mut closed = Vec::new();
    for h in hits {
        let item = skin::tab_item(h.id);
        if item.mark == kit::tabs::TabMark::None || skin::is_dragged_tab(&h.response) {
            continue;
        }
        let Some(mark) = bars.iter().find_map(|b| {
            let i = b.tabs.iter().position(|&t| t == h.id)?;
            skin::tab_mark(b, i)
        }) else {
            continue;
        };
        let mark = mark.expand(2.0);
        let mut layer = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("workspace_close", h.id.key()))
                .layer_id(h.response.layer_id)
                .max_rect(mark),
        );
        layer.set_clip_rect(mark);
        let label = format!("{} {}", item.close.label(), item.label);
        let mut response = layer.interact(
            mark,
            egui::Id::new(("workspace_close", h.id.key())),
            egui::Sense::click(),
        );
        if !modal {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !modal, &label));
        if !modal && response.clicked() {
            closed.push(h.id);
        }
    }
    closed
}

/// Lo que corrió la rueda este frame bajo el puntero, en puntos (positivo: hacia atrás, como en
/// `kit::tabs::tab_strip`), con lo que es un renglón (ver `skin::Wheel`), si la movió. Con Ctrl
/// (o Cmd) es zoom: no cuenta.
fn tab_wheel(ctx: &egui::Context) -> Option<skin::Wheel> {
    let at = ctx.input(|i| i.pointer.hover_pos())?;
    let notch = ctx.options(|o| o.input_options.line_scroll_speed);
    let (delta, zoom) = ctx.input(|i| {
        let zoom = |m: &egui::Modifiers| m.ctrl || m.command || m.mac_cmd;
        let mut delta = 0.0;
        let mut zooming = zoom(&i.modifiers);
        for e in &i.raw.events {
            if let egui::Event::MouseWheel {
                unit,
                delta: d,
                modifiers,
                ..
            } = e
            {
                let per = match unit {
                    egui::MouseWheelUnit::Point => 1.0,
                    egui::MouseWheelUnit::Line => notch,
                    egui::MouseWheelUnit::Page => i.viewport_rect().height(),
                };
                delta += (d.y + d.x) * per;
                zooming |= zoom(modifiers);
            }
        }
        (delta, zooming)
    });
    (delta != 0.0 && !zoom).then_some(skin::Wheel { at, delta, notch })
}

/// Las flechas del kit de cada barra que no cabe (ver `skin::TabBar`), en la capa de su barra: de
/// a una pestaña hacia atrás o hacia adelante (desde el frame siguiente). Antes, entre la última
/// que se ve y el «, un widget que se queda con el puntero: ahí egui_dock tiene lo que se pasa de
/// la siguiente (o lo de más de la última), que no se ve y no tiene que recibir el clic ni
/// arrastrarse. En una ventana flotante, arrastrarlo la mueve, como lo vacío al final de una barra
/// que cabe (que es de la ventana): devuelve la ventana y lo que se movió el puntero. Con un
/// diálogo modal, apagadas (y nada se mueve).
fn tab_arrows(
    ui: &mut egui::Ui,
    skin: &Skin,
    bars: &[skin::TabBar],
    hits: &[skin::TabHit],
    modal: bool,
) -> Option<(SurfaceIndex, egui::Vec2)> {
    use kit::tabs::ARROW_SIZE;
    let mut moved = None;
    for bar in bars {
        let Some(arrows) = bar.arrows else { continue };
        let Some(layer) = bar
            .tabs
            .iter()
            .find_map(|&t| hits.iter().find(|h| h.id == t))
            .map(|h| h.response.layer_id)
        else {
            continue;
        };
        let last = bar.shown.last().map_or(bar.room.left(), |(_, r)| r.right());
        let rest = bar.room.with_min_x(last.min(arrows.left()));
        let mut ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("workspace_tab_arrows", bar.path.surface, bar.path.node))
                .layer_id(layer)
                .max_rect(rest),
        );
        ui.set_clip_rect(rest);
        let held = ui.interact(
            rest,
            egui::Id::new(("workspace_tab_rest", bar.path.surface, bar.path.node)),
            egui::Sense::click_and_drag(),
        );
        if !modal && !bar.path.surface.is_main() && held.dragged() {
            moved = Some((bar.path.surface, held.drag_delta()));
        }
        let halves = [
            egui::Rect::from_min_size(arrows.min, egui::Vec2::splat(ARROW_SIZE)),
            egui::Rect::from_min_size(
                arrows.min + egui::vec2(ARROW_SIZE - 1.0, 0.0),
                egui::Vec2::splat(ARROW_SIZE),
            ),
        ];
        let mut states = [(false, false); 2];
        let mut offset = bar.offset;
        for (i, (rect, label)) in halves
            .into_iter()
            .zip(["Previous tabs", "Next tabs"])
            .enumerate()
        {
            let can = !modal
                && if i == 0 {
                    bar.can_prev()
                } else {
                    bar.can_next()
                };
            let response = ui.interact(
                rect,
                egui::Id::new(("workspace_tab_arrow", bar.path.surface, bar.path.node, i)),
                egui::Sense::click(),
            );
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, can, label));
            states[i] = (can && response.hovered(), can);
            if can && response.clicked() {
                offset = if i == 0 {
                    offset.saturating_sub(1)
                } else {
                    (offset + 1).min(bar.last_offset)
                };
            }
        }
        // Solo en la primera pasada del frame: si se descarta, la segunda no recibe eventos
        // (`Context::run` le pasa una entrada vacía) y no habría clic; se deja por si eso cambia.
        if offset != bar.offset && ui.ctx().current_pass_index() == 0 {
            let memory = skin::tabs_offset_id(bar.path);
            let front = ui.data(|d| d.get_temp::<(usize, Option<PanelId>)>(memory));
            ui.data_mut(|d| d.insert_temp(memory, (offset, front.and_then(|f| f.1))));
            ui.ctx().request_repaint();
        }
        let shapes = kit::tabs::arrows_shapes(&ui, skin, arrows, states[0], states[1]);
        ui.painter().extend(shapes);
    }
    moved
}

/// Con un diálogo modal, lo de las ventanas flotantes que se agarraría (ver el modal en [`show`]):
/// egui_dock las arma con su propia `egui::Window`, que no hereda lo apagado del `Ui` del
/// `DockArea`, así que encima de lo suyo, en su capa, va un widget que se queda con el puntero sin
/// hacer nada (registrado después, le gana a lo de debajo). En una ventana con Color o con el
/// Preview (que un diálogo no apaga), solo lo que la mueve, la cambia de tamaño, la parte o la
/// acopla: su marco (y el borde de afuera del que se agarra), sus barras de pestañas, sus franjas
/// de agarre (con Pixel UI) y sus divisores. Cualquier otra, entera: lo de adentro ya va apagado y
/// arrastrarlo movería la ventana.
fn modal_shields(ui: &mut egui::Ui, skin: Option<&Skin>, state: &DockState<PanelId>) {
    let ctx = ui.ctx().clone();
    let grab = ctx.global_style().interaction.resize_grab_radius_side;
    for (surface, s) in state.iter_surfaces_indexed() {
        let Some(tree) = s.node_tree().filter(|_| !surface.is_main()) else {
            continue;
        };
        let Some(area) = ctx.memory(|m| m.area_rect(skin::window_area_id(surface))) else {
            continue;
        };
        let outer = area.expand(grab);
        let live = tree
            .iter()
            .filter_map(|n| n.tabs())
            .flatten()
            .any(|t| matches!(t, PanelId::Color | PanelId::Preview));
        let mut rects = Vec::new();
        if !live {
            rects.push(outer);
        } else {
            // El marco (con el borde de afuera): de lo de afuera a lo del árbol.
            let inner = tree
                .root_node()
                .and_then(|n| n.rect())
                .filter(|r| r.is_positive())
                .unwrap_or(area);
            rects.extend([
                outer.with_max_y(inner.top()),
                outer.with_min_y(inner.bottom()),
                egui::Rect::from_x_y_ranges(outer.left()..=inner.left(), inner.y_range()),
                egui::Rect::from_x_y_ranges(inner.right()..=outer.right(), inner.y_range()),
            ]);
            for (i, node) in tree.iter().enumerate() {
                let n = NodeIndex(i);
                match node {
                    egui_dock::Node::Leaf(leaf) if leaf.tab_bar_hidden => {
                        if skin.is_some() {
                            rects.push(egui::Rect::from_min_size(
                                leaf.rect.min,
                                egui::vec2(leaf.rect.width(), skin::GRAB_STRIP),
                            ));
                        }
                    }
                    egui_dock::Node::Leaf(leaf) if leaf.viewport.is_positive() => {
                        rects.push(leaf.rect.with_max_y(leaf.viewport.top()));
                    }
                    egui_dock::Node::Horizontal(split) => {
                        if let (Some(a), Some(b)) = (tree[n.left()].rect(), tree[n.right()].rect())
                        {
                            let gap = egui::Rect::from_x_y_ranges(
                                a.right()..=b.left(),
                                split.rect.y_range(),
                            );
                            rects.push(gap.expand2(egui::vec2(skin::SEPARATOR_EXTRA, 0.0)));
                        }
                    }
                    egui_dock::Node::Vertical(split) => {
                        if let (Some(a), Some(b)) = (tree[n.left()].rect(), tree[n.right()].rect())
                        {
                            let gap = egui::Rect::from_x_y_ranges(
                                split.rect.x_range(),
                                a.bottom()..=b.top(),
                            );
                            rects.push(gap.expand2(egui::vec2(0.0, skin::SEPARATOR_EXTRA)));
                        }
                    }
                    _ => {}
                }
            }
        }
        let layer = egui::LayerId::new(egui::Order::Middle, skin::window_area_id(surface));
        let mut shield = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("workspace_modal_shield", surface))
                .layer_id(layer)
                .max_rect(outer),
        );
        shield.set_clip_rect(outer);
        for (k, rect) in rects.into_iter().enumerate() {
            if rect.is_positive() {
                let id = egui::Id::new(("workspace_modal_shield", surface, k));
                let _ = shield.interact(rect, id, egui::Sense::click_and_drag());
            }
        }
    }
}

/// La pestaña que egui_dock suelta este frame pasando la última que se ve de una barra con flechas
/// que muestra su última (ver `skin::TabBar`: ahí está lo de más de la última, `tail`, que no se
/// ve): egui_dock la pone antes de la última (suelta en una pestaña, va en su lugar), pero se ve
/// suelta en lo vacío del final de la barra, donde en una barra que cabe va al final. Devuelve la
/// que se suelta y la última de esa barra (para [`to_the_end`], después del `DockArea`).
fn dropped_past_last(ctx: &egui::Context, bars: &[skin::TabBar]) -> Option<(PanelId, PanelId)> {
    if !ctx.input(|i| i.pointer.primary_released()) {
        return None;
    }
    let id = dragged_tab(ctx)?;
    let p = ctx.input(|i| i.pointer.latest_pos())?;
    bars.iter().find_map(|b| {
        let last = *b.tabs.last()?;
        let row = b.row(last)?;
        (b.tail > 0.0 && b.room.contains(p) && p.x >= row.right()).then_some((id, last))
    })
}

/// Si egui_dock dejó `id` justo antes de `last`, la última de su grupo (ver
/// [`dropped_past_last`]), va al final; la de al frente sigue siendo la misma pestaña.
fn to_the_end(state: &mut DockState<PanelId>, id: PanelId, last: PanelId) {
    let Some(path) = state.find_tab(&id) else {
        return;
    };
    let Ok(leaf) = state.leaf_mut(path.node_path()) else {
        return;
    };
    let n = leaf.tabs.len();
    if n < 2 || path.tab.0 + 2 != n || leaf.tabs[n - 1] != last {
        return;
    }
    let front = leaf.tabs.get(leaf.active.0).copied();
    leaf.tabs.swap(n - 2, n - 1);
    if let Some(i) = front.and_then(|f| leaf.tabs.iter().position(|&t| t == f)) {
        leaf.active = egui_dock::TabIndex(i);
    }
}

/// Con un diálogo modal, ninguna ventana flotante se mueve arrastrando lo suyo: egui_dock las arma
/// con una `egui::Window` que se arrastra de cualquier lugar de su fondo (también de lo de un
/// panel apagado o del margen de Color), y `modal_shields` solo tapa lo que la acopla o la parte.
/// Antes del `DockArea`, si lo que se arrastra es el fondo de una de ellas (el widget "move" de su
/// área), se suelta: así no se mueve ni este frame. Salvo `title` (sin Pixel UI, la barra de
/// título del Preview solo en su ventana, que es de la ventana de egui) si el arrastre empezó ahí.
fn hold_windows(
    ctx: &egui::Context,
    state: &DockState<PanelId>,
    title: Option<(SurfaceIndex, egui::Rect)>,
) {
    let Some(dragged) = ctx.dragged_id() else {
        return;
    };
    let press = ctx.input(|i| i.pointer.press_origin());
    let window = state
        .iter_surfaces_indexed()
        .map(|(s, _)| s)
        .find(|s| !s.is_main() && dragged == skin::window_area_id(*s).with("move"));
    if let Some(s) = window {
        let from_title = title.is_some_and(|(t, r)| t == s && press.is_some_and(|p| r.contains(p)));
        if !from_title {
            ctx.stop_dragging();
        }
    }
}

/// Lo que el `DockArea` cambia con el puntero, el teclado o un clic de accesibilidad: la fracción
/// de cada split, la pestaña al frente de cada grupo y el grupo con el foco. Con un diálogo modal
/// se toma antes y se repone después (ver el modal en [`show`]): nada de eso cambia aunque algo se
/// cuele (en una ventana flotante, un clic de accesibilidad no pasa por `modal_shields`).
struct Frozen {
    splits: Vec<(egui_dock::NodePath, f32)>,
    active: Vec<(egui_dock::NodePath, egui_dock::TabIndex)>,
    focused: Option<egui_dock::NodePath>,
}

impl Frozen {
    fn take(state: &DockState<PanelId>) -> Frozen {
        Frozen {
            splits: state
                .iter_all_nodes()
                .filter_map(|(path, node)| match node {
                    egui_dock::Node::Horizontal(s) | egui_dock::Node::Vertical(s) => {
                        Some((path, s.fraction))
                    }
                    _ => None,
                })
                .collect(),
            active: state
                .iter_leaves()
                .map(|(path, leaf)| (path, leaf.active))
                .collect(),
            focused: state.focused_leaf(),
        }
    }

    fn restore(self, state: &mut DockState<PanelId>) {
        for (path, fraction) in self.splits {
            if let Ok(egui_dock::Node::Horizontal(s) | egui_dock::Node::Vertical(s)) =
                state.node_mut(path)
            {
                s.fraction = fraction;
            }
        }
        for (path, active) in self.active {
            if let Ok(leaf) = state.leaf_mut(path)
                && active.0 < leaf.tabs.len()
            {
                leaf.active = active;
            }
        }
        // Sin foco antes, sin foco después: egui_dock no tiene cómo quitarlo, pero un camino que
        // no es una hoja lo deja sin él (`set_focused_node_and_surface`).
        let none = egui_dock::NodePath {
            surface: SurfaceIndex::main(),
            node: NodeIndex(usize::MAX),
        };
        state.set_focused_node_and_surface(self.focused.unwrap_or(none));
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use egui_dock::NodePath;

    use super::*;
    use crate::workspace::test_util::{
        button, click, click_at, drag_to, frame, frame_in, frame_with, frames, frames_in,
        node_rect, test_app,
    };
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

    /// La ventana de los tests del modal.
    const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

    /// Lo que se ve del acomodo: cada grupo (su superficie, sus pestañas, la activa y su rect), el
    /// rect de cada ventana flotante, las tiras, el ancho de la columna, el flyout abierto y si
    /// hay un menú abierto.
    fn snapshot(ctx: &egui::Context, app: &ChimpApp) -> String {
        let w = &app.workspace;
        let leaves: Vec<_> = w
            .state
            .iter_leaves()
            .map(|(path, leaf)| (path.surface, leaf.tabs.clone(), leaf.active.0, leaf.rect))
            .collect();
        let windows: Vec<_> = PanelId::ALL
            .iter()
            .filter(|&&p| w.floating(p))
            .map(|&p| (p, w.window_rect(ctx, p)))
            .collect();
        let menu = egui::Popup::is_any_open(ctx);
        format!(
            "{leaves:?}\n{windows:?}\n{:?}\n{:?} {:?} menu {menu}",
            w.strips,
            app.view.palette_width,
            crate::dock::open_panel(ctx),
        )
    }

    /// Lo que se prueba con un diálogo abierto: un nombre, el acomodo de antes y lo que hace el
    /// usuario (con la salida del último frame).
    struct ModalCase {
        name: &'static str,
        /// Solo con Pixel UI (sin él no hay franja de agarre).
        pixel_only: bool,
        setup: fn(&mut ChimpApp),
        act: fn(&egui::Context, &mut ChimpApp, &egui::FullOutput, egui::Vec2),
        /// La ventana.
        size: egui::Vec2,
    }

    /// Cada caso con Pixel UI y sin él, sin diálogo y con uno abierto (`ChimpApp::is_modal`): sin
    /// diálogo lo que hace el usuario cambia el acomodo (si no, el caso no probaría nada); con
    /// uno, nada cambia.
    fn assert_blocked_by_a_modal(cases: &[ModalCase]) {
        for case in cases {
            for pixel in [true, false] {
                if case.pixel_only && !pixel {
                    continue;
                }
                for modal in [false, true] {
                    let mut app = test_app();
                    app.view.pixel_ui = pixel;
                    (case.setup)(&mut app);
                    let ctx = egui::Context::default();
                    ctx.enable_accesskit();
                    let size = case.size;
                    let mut out = frames_in(&ctx, &mut app, size);
                    if modal {
                        app.show_message("Test", "A modal dialog");
                        out = frames_in(&ctx, &mut app, size);
                        assert!(app.is_modal());
                    }
                    let before = snapshot(&ctx, &app);
                    (case.act)(&ctx, &mut app, &out, size);
                    frames_in(&ctx, &mut app, size);
                    let after = snapshot(&ctx, &app);
                    let what = format!("{}, pixel {pixel}, modal {modal}", case.name);
                    if modal {
                        assert_eq!(after, before, "{what}: nothing changes");
                    } else {
                        assert_ne!(after, before, "{what}: without a dialog it changes");
                    }
                }
            }
        }
    }

    fn leaf(app: &ChimpApp, id: PanelId) -> egui::Rect {
        app.workspace
            .leaf_rect(id)
            .unwrap_or_else(|| panic!("{id:?} in the tree"))
    }

    /// Un clic con el botón principal en el centro del nodo `label` de `out`.
    fn click_node(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        out: &egui::FullOutput,
        size: egui::Vec2,
        label: &str,
    ) {
        let rect = node_rect(out, label).unwrap_or_else(|| {
            let all = crate::workspace::test_util::buttons(out);
            panic!("no {label:?} in {all:?}")
        });
        click_at(ctx, app, rect.center(), egui::PointerButton::Primary, size);
    }

    /// El nombre del ícono de `id` en la tira: su título con Pixel UI; sin él, lo que dice el
    /// botón de egui.
    fn icon_label(app: &ChimpApp, id: PanelId) -> &'static str {
        if app.pixel.skin().is_some() {
            id.spec().title
        } else {
            id.spec().plain
        }
    }

    /// Library con Slices en pestañas, flotando (Slices al frente).
    fn library_and_slices_floating(app: &mut ChimpApp) {
        app.workspace
            .float(PanelId::Library, egui::pos2(320.0, 120.0));
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        app.workspace.dock(PanelId::Slices, DockTarget::Leaf(s, n));
    }

    /// Lo mismo, una encima de la otra en su ventana.
    fn library_over_slices_floating(app: &mut ChimpApp) {
        app.workspace
            .float(PanelId::Library, egui::pos2(320.0, 120.0));
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        app.workspace.dock(
            PanelId::Slices,
            DockTarget::Split(s, n, egui_dock::Split::Below),
        );
    }

    /// La × de la pestaña `id` (con Pixel UI, la del kit; sin él, la de egui_dock, al final de
    /// su pestaña).
    fn close_tab(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        out: &egui::FullOutput,
        size: egui::Vec2,
        id: PanelId,
    ) {
        let title = id.spec().title;
        let at = if app.pixel.skin().is_some() {
            node_rect(out, &format!("Close {title}")).unwrap().center()
        } else {
            let tab = node_rect(out, title).unwrap();
            egui::pos2(tab.right() - 12.0, tab.center().y)
        };
        click_at(ctx, app, at, egui::PointerButton::Primary, size);
    }

    #[test]
    fn with_a_modal_dialog_the_dividers_do_not_move() {
        assert_blocked_by_a_modal(&[
            ModalCase {
                name: "the divider between Color and Library",
                pixel_only: false,
                setup: |app| {
                    let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
                    let below = DockTarget::Split(s, n, egui_dock::Split::Below);
                    app.workspace.dock(PanelId::Library, below);
                },
                act: |ctx, app, _, size| {
                    let (color, below) = (leaf(app, PanelId::Color), leaf(app, PanelId::Library));
                    let at = egui::pos2(color.center().x, (color.bottom() + below.top()) / 2.0);
                    drag_to(ctx, app, (at, at + egui::vec2(0.0, 30.0)), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the divider of the right column",
                pixel_only: false,
                setup: |_| {},
                act: |ctx, app, _, size| {
                    let (canvas, color) = (leaf(app, PanelId::Canvas), leaf(app, PanelId::Color));
                    let at = egui::pos2((canvas.right() + color.left()) / 2.0, color.center().y);
                    drag_to(ctx, app, (at, at - egui::vec2(30.0, 0.0)), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "a divider in a floating window",
                pixel_only: false,
                setup: library_over_slices_floating,
                act: |ctx, app, _, size| {
                    let (top, below) = (leaf(app, PanelId::Library), leaf(app, PanelId::Slices));
                    let at = egui::pos2(top.center().x, (top.bottom() + below.top()) / 2.0);
                    drag_to(ctx, app, (at, at + egui::vec2(0.0, 20.0)), size);
                },
                size: SIZE,
            },
        ]);
    }

    #[test]
    fn with_a_modal_dialog_the_tabs_do_not_switch_close_or_move() {
        assert_blocked_by_a_modal(&[
            ModalCase {
                name: "clicking an inactive tab",
                pixel_only: false,
                setup: |app| app.workspace = Workspace::demo("library-tab").unwrap(),
                act: |ctx, app, out, size| click_node(ctx, app, out, size, "Color"),
                size: SIZE,
            },
            ModalCase {
                name: "an accessibility click on an inactive tab",
                pixel_only: false,
                setup: |app| app.workspace = Workspace::demo("library-tab").unwrap(),
                act: |ctx, app, out, size| {
                    let node = button(out, "Color").expect("the Color tab");
                    frame_in(ctx, app, vec![click(node)], size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the × of a tab",
                pixel_only: false,
                setup: |app| app.workspace = Workspace::demo("library-tab").unwrap(),
                act: |ctx, app, out, size| close_tab(ctx, app, out, size, PanelId::Library),
                size: SIZE,
            },
            ModalCase {
                name: "dragging a tab onto the canvas",
                pixel_only: false,
                setup: |app| app.workspace = Workspace::demo("library-tab").unwrap(),
                act: |ctx, app, out, size| {
                    let tab = node_rect(out, "Library").unwrap().center();
                    let canvas = leaf(app, PanelId::Canvas).center();
                    drag_to(ctx, app, (tab, canvas), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "clicking an inactive tab of a floating window",
                pixel_only: false,
                setup: library_and_slices_floating,
                act: |ctx, app, out, size| click_node(ctx, app, out, size, "Library"),
                size: SIZE,
            },
            ModalCase {
                name: "an accessibility click on a tab of a floating window",
                pixel_only: false,
                setup: library_and_slices_floating,
                act: |ctx, app, out, size| {
                    let node = button(out, "Library").expect("the Library tab");
                    frame_in(ctx, app, vec![click(node)], size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the × of a tab of a floating window",
                pixel_only: false,
                setup: library_and_slices_floating,
                act: |ctx, app, out, size| close_tab(ctx, app, out, size, PanelId::Slices),
                size: SIZE,
            },
        ]);
    }

    #[test]
    fn with_a_modal_dialog_the_strip_does_nothing() {
        assert_blocked_by_a_modal(&[
            ModalCase {
                name: "clicking a strip icon",
                pixel_only: false,
                setup: |_| {},
                act: |ctx, app, out, size| {
                    let label = icon_label(app, PanelId::Lospec);
                    click_node(ctx, app, out, size, label);
                },
                size: SIZE,
            },
            ModalCase {
                name: "dragging a strip icon onto the canvas",
                pixel_only: false,
                setup: |_| {},
                act: |ctx, app, out, size| {
                    let label = icon_label(app, PanelId::Lospec);
                    let icon = node_rect(out, label).unwrap().center();
                    let canvas = leaf(app, PanelId::Canvas).center();
                    drag_to(ctx, app, (icon, canvas), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the context menu of a strip icon",
                pixel_only: false,
                setup: |_| {},
                act: |ctx, app, out, size| {
                    let label = icon_label(app, PanelId::Lospec);
                    let icon = node_rect(out, label).unwrap().center();
                    let out = click_at(ctx, app, icon, egui::PointerButton::Secondary, size);
                    // Abierto (sin diálogo): Dock.
                    if let Some(dock) = node_rect(&out, crate::dock::MENU_DOCK) {
                        click_at(ctx, app, dock.center(), egui::PointerButton::Primary, size);
                    }
                },
                size: SIZE,
            },
            ModalCase {
                name: "More Panels",
                pixel_only: false,
                setup: |_| {},
                // Baja: la tira de la derecha no tiene lugar para todos sus íconos.
                act: |ctx, app, out, size| {
                    let more = if app.pixel.skin().is_some() {
                        crate::dock::MORE
                    } else {
                        "»"
                    };
                    click_node(ctx, app, out, size, more);
                },
                size: egui::vec2(1280.0, 240.0),
            },
        ]);
    }

    #[test]
    fn with_a_modal_dialog_floating_windows_are_not_dragged_or_docked() {
        assert_blocked_by_a_modal(&[
            ModalCase {
                name: "the grab strip of a lone floating toolbar",
                pixel_only: true,
                setup: |app| app.workspace = Workspace::demo("tools-float").unwrap(),
                act: |ctx, app, _, size| {
                    let tools = leaf(app, PanelId::Tools);
                    let at = egui::pos2(tools.center().x, tools.top() + 1.0);
                    let to = leaf(app, PanelId::Canvas).center();
                    drag_to(ctx, app, (at, to), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the empty end of the tab bar of a floating window",
                pixel_only: false,
                setup: |app| {
                    app.workspace
                        .float(PanelId::Library, egui::pos2(320.0, 120.0))
                },
                act: |ctx, app, out, size| {
                    let tab = node_rect(out, "Library").unwrap();
                    let at = egui::pos2(tab.right() + 20.0, tab.center().y);
                    drag_to(ctx, app, (at, at + egui::vec2(40.0, 30.0)), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the padding of Color in its window",
                pixel_only: false,
                setup: |app| {
                    app.workspace
                        .float(PanelId::Color, egui::pos2(320.0, 120.0))
                },
                act: |ctx, app, _, size| {
                    let color = leaf(app, PanelId::Color);
                    let at = egui::pos2(color.left() + 2.0, color.center().y);
                    drag_to(ctx, app, (at, at + egui::vec2(40.0, 30.0)), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the grab strip of the Preview in its window",
                pixel_only: true,
                setup: |app| app.view.preview = true,
                act: |ctx, app, _, size| {
                    let preview = leaf(app, PanelId::Preview);
                    let at = egui::pos2(preview.center().x, preview.top() + 1.0);
                    let to = leaf(app, PanelId::Color).center();
                    drag_to(ctx, app, (at, to), size);
                },
                size: SIZE,
            },
        ]);
    }

    #[test]
    fn with_a_modal_dialog_the_preview_window_still_moves_from_its_title() {
        // Como su ventana de antes (y como sus controles): un diálogo no la apaga.
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            app.view.preview = true;
            let ctx = egui::Context::default();
            frames_in(&ctx, &mut app, SIZE);
            app.show_message("Test", "A modal dialog");
            frames_in(&ctx, &mut app, SIZE);
            let window = app.workspace.window_rect(&ctx, PanelId::Preview).unwrap();
            // En su barra de título (sin Pixel UI, la de la `egui::Window` de antes).
            let title = if pixel { 9.0 } else { 17.0 };
            let at = egui::pos2(window.left() + 40.0, window.top() + title);
            drag_to(&ctx, &mut app, (at, at - egui::vec2(30.0, 20.0)), SIZE);
            frames_in(&ctx, &mut app, SIZE);
            let moved = app.workspace.window_rect(&ctx, PanelId::Preview).unwrap();
            assert_eq!(
                moved.min,
                window.min - egui::vec2(30.0, 20.0),
                "pixel {pixel}"
            );
            assert!(app.workspace.floating(PanelId::Preview));
        }
    }

    /// Color con siete paneles más en su grupo, en la columna: sus pestañas no caben.
    fn many_tabs(app: &mut ChimpApp) {
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        for id in [
            PanelId::Library,
            PanelId::Slices,
            PanelId::Artboards,
            PanelId::Lospec,
            PanelId::Groom,
            PanelId::Lumen,
            PanelId::Font,
        ] {
            app.workspace.dock(id, DockTarget::Leaf(s, n));
        }
        // Color al frente (la primera): las de la punta no se ven.
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        let path = NodePath {
            surface: s,
            node: n,
        };
        app.workspace.state.leaf_mut(path).unwrap().active = egui_dock::TabIndex(0);
    }

    /// Las × de las pestañas que se ven en `out`, de izquierda a derecha.
    fn close_marks_in(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        let mut marks: Vec<(String, egui::Rect)> = crate::workspace::test_util::buttons(out)
            .into_iter()
            .filter(|l| l.starts_with("Close "))
            .filter_map(|l| node_rect(out, &l).map(|r| (l, r)))
            .collect();
        marks.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
        marks
    }

    #[test]
    fn many_tabs_in_a_narrow_group_page_with_the_kit_arrows() {
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut out = frames_in(&ctx, &mut app, SIZE);
        let group = leaf(&app, PanelId::Color);
        let collapse = node_rect(&out, COLLAPSE).expect("the « of the group");
        let prev = node_rect(&out, "Previous tabs").expect("the kit arrows");
        let next = node_rect(&out, "Next tabs").expect("the kit arrows");
        // Las del kit (12 × 12, juntas 23), en la fila de las flechas de la barra, a la izquierda
        // del « y sin tocarlo.
        assert_eq!(prev.size(), egui::Vec2::splat(kit::tabs::ARROW_SIZE));
        assert_eq!(next.left(), prev.left() + kit::tabs::ARROW_SIZE - 1.0);
        assert_eq!(prev.top(), group.top() + kit::tabs::ARROWS_TOP);
        assert!(next.right() < collapse.left(), "{next:?} {collapse:?}");
        let first = close_marks_in(&out);
        assert!(!first.is_empty(), "some tabs are seen");
        // De ida con la de adelante hasta que se ve la última (FontForge) y de vuelta con la de
        // atrás: cada × que se ve, entero antes de las flechas (con su aire) y nunca bajo el «.
        let check = |out: &egui::FullOutput, step: &str| {
            for (label, mark) in close_marks_in(out) {
                assert!(
                    mark.right() <= prev.left() - kit::tabs::GAP && !mark.intersects(collapse),
                    "{step}: {label} at {mark:?}, arrows at {prev:?}"
                );
            }
        };
        check(&out, "at first");
        let mut steps = 0;
        while node_rect(&out, "Close FontForge").is_none() {
            assert!(steps < 8, "the last tab comes into view");
            click_at(
                &ctx,
                &mut app,
                next.center(),
                egui::PointerButton::Primary,
                SIZE,
            );
            out = frames_in(&ctx, &mut app, SIZE);
            check(&out, &format!("forward {steps}"));
            steps += 1;
        }
        assert!(steps > 0, "it was not in view at first");
        // La última, a la vista: un clic en ella la pone al frente.
        let font = node_rect(&out, "Close FontForge").unwrap();
        let tab = egui::pos2(font.left() - 12.0, font.center().y);
        click_at(&ctx, &mut app, tab, egui::PointerButton::Primary, SIZE);
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        assert_eq!(
            app.workspace.state_leaf_tabs(s, n).unwrap().1,
            PanelId::Font
        );
        out = frames_in(&ctx, &mut app, SIZE);
        check(&out, "with the last one in front");
        // Y de vuelta al principio.
        for _ in 0..steps {
            click_at(
                &ctx,
                &mut app,
                prev.center(),
                egui::PointerButton::Primary,
                SIZE,
            );
        }
        out = frames_in(&ctx, &mut app, SIZE);
        check(&out, "back");
        assert_eq!(close_marks_in(&out), first, "the first ones again");
    }

    #[test]
    fn the_wheel_over_tabs_that_do_not_fit_pages_one_tab_per_notch() {
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        frames_in(&ctx, &mut app, SIZE);
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        let path = NodePath {
            surface: s,
            node: n,
        };
        let offset = |ctx: &egui::Context| {
            ctx.data(|d| d.get_temp::<(usize, Option<PanelId>)>(skin::tabs_offset_id(path)))
                .map(|(o, _)| o)
        };
        assert_eq!(offset(&ctx), Some(0));
        let bar = leaf(&app, PanelId::Color);
        let at = egui::pos2(bar.left() + 20.0, bar.top() + 6.0);
        let wheel = |dy: f32| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, dy),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        };
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        frame_in(&ctx, &mut app, vec![wheel(-1.0)], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(offset(&ctx), Some(1), "one tab forward");
        frame_in(&ctx, &mut app, vec![wheel(1.0)], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(offset(&ctx), Some(0), "and back");
    }

    #[test]
    fn the_crossings_are_not_painted_over_the_top_strip() {
        // Los cruces del divisor de la columna con la barra de opciones van un punto arriba del
        // `DockArea`: con la tira de arriba, ese renglón es el de abajo de la tira.
        let crossings = |app: &mut ChimpApp| {
            let ctx = egui::Context::default();
            let out = frames(&ctx, app);
            let color = app.pixel.shared_skin().unwrap().colors().canvas;
            let y = app.workspace.leaf_rect(PanelId::Color).unwrap().top() - 1.0;
            fn rects(shape: &egui::Shape, out: &mut Vec<(egui::Color32, egui::Rect)>) {
                match shape {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| rects(s, out)),
                    egui::Shape::Rect(r) => out.push((r.fill, r.rect)),
                    _ => {}
                }
            }
            let mut all = Vec::new();
            for c in &out.shapes {
                rects(&c.shape, &mut all);
            }
            all.into_iter()
                .filter(|(c, r)| {
                    *c == color
                        && r.width() <= 1.01
                        && r.height() <= 1.01
                        && (r.top() - y).abs() < 0.6
                })
                .count()
        };
        let mut app = test_app();
        assert!(crossings(&mut app) > 0, "Essentials: under the options bar");
        let mut app = test_app();
        app.workspace.to_strip_at(PanelId::Library, Edge::Top);
        assert_eq!(crossings(&mut app), 0, "over the top strip");
    }

    #[test]
    fn a_strip_icon_dropped_on_another_strip_moves_to_it() {
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            app.workspace = Workspace::demo("left-strip").unwrap();
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let out = frames_in(&ctx, &mut app, SIZE);
            let groom = node_rect(&out, icon_label(&app, PanelId::Groom))
                .unwrap()
                .center();
            let library = node_rect(&out, icon_label(&app, PanelId::Library))
                .unwrap()
                .center();
            drag_to(&ctx, &mut app, (groom, library), SIZE);
            let w = &app.workspace;
            assert_eq!(
                w.strip_of(PanelId::Groom),
                Some(Edge::Right),
                "pixel {pixel}"
            );
            assert!(!w.docked(PanelId::Groom) && !w.floating(PanelId::Groom));
            assert_eq!(w.places(PanelId::Groom), 1);
        }
    }

    #[test]
    fn without_pixel_ui_the_close_button_sends_the_panel_to_the_strip_of_its_edge() {
        // La × de egui_dock (sin Pixel UI): como la del kit, a la tira de la orilla del grupo.
        let mut app = test_app();
        app.view.pixel_ui = false;
        let (s, n) = app.workspace.leaf_of(PanelId::Canvas).unwrap();
        app.workspace.dock(
            PanelId::Groom,
            DockTarget::Split(s, n, egui_dock::Split::Left),
        );
        let (s, n) = app.workspace.leaf_of(PanelId::Groom).unwrap();
        app.workspace.dock(PanelId::Library, DockTarget::Leaf(s, n));
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        app.workspace.dock(PanelId::Slices, DockTarget::Leaf(s, n));
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let out = frames_in(&ctx, &mut app, SIZE);
        assert!(app.pixel.skin().is_none());
        for (id, want) in [
            (PanelId::Library, Edge::Left),
            (PanelId::Slices, Edge::Right),
        ] {
            assert_eq!(app.workspace.edge_of(id), Some(want), "{id:?}");
            let out = if id == PanelId::Library {
                out.clone()
            } else {
                frames_in(&ctx, &mut app, SIZE)
            };
            close_tab(&ctx, &mut app, &out, SIZE, id);
            frames_in(&ctx, &mut app, SIZE);
            assert_eq!(app.workspace.strip_of(id), Some(want), "{id:?}");
        }
    }

    #[test]
    fn the_toolbar_and_the_timeline_below_are_claimed_by_the_workspace() {
        // Una instancia por panel por frame: también la toolbar y el timeline de debajo del
        // lienzo (sus ramas propias del `Viewer`) se reservan para el workspace.
        let mut app = test_app();
        assert!(app.workspace.timeline_below());
        frames(&egui::Context::default(), &mut app);
        for id in [PanelId::Tools, PanelId::Timeline] {
            assert_eq!(
                app.claim_panel(id, PanelHost::Flyout),
                Err(PanelHost::Workspace),
                "{id:?}"
            );
        }
    }

    #[test]
    fn a_tab_brought_to_the_front_is_paged_into_view() {
        // Window → panel (o Lospec) pone al frente una pestaña que no se ve: las flechas la traen.
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let out = frames_in(&ctx, &mut app, SIZE);
        assert!(node_rect(&out, "Close FontForge").is_none(), "out of view");
        app.workspace.bring_forward(&ctx, PanelId::Font);
        let out = frames_in(&ctx, &mut app, SIZE);
        let prev = node_rect(&out, "Previous tabs").unwrap();
        let mark = node_rect(&out, "Close FontForge").expect("in view");
        assert!(mark.right() <= prev.left() - kit::tabs::GAP, "{mark:?}");
    }

    /// Library con siete paneles más en su grupo, debajo de Color en la columna (Library al
    /// frente): sus pestañas no caben y su barra queda a media altura (algo puede ir encima).
    fn many_tabs_below_color(app: &mut ChimpApp) {
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        let below = DockTarget::Split(s, n, egui_dock::Split::Below);
        app.workspace.dock(PanelId::Library, below);
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        for id in [
            PanelId::Slices,
            PanelId::Artboards,
            PanelId::Lospec,
            PanelId::Groom,
            PanelId::Lumen,
            PanelId::Model,
            PanelId::Font,
        ] {
            app.workspace.dock(id, DockTarget::Leaf(s, n));
        }
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        let path = NodePath {
            surface: s,
            node: n,
        };
        app.workspace.state.leaf_mut(path).unwrap().active = egui_dock::TabIndex(0);
    }

    /// Desde cuál pestaña se ve la barra del grupo de `id` (`None` sin Pixel UI: no hay flechas).
    fn tabs_offset(ctx: &egui::Context, app: &ChimpApp, id: PanelId) -> Option<usize> {
        let (surface, node) = app.workspace.leaf_of(id)?;
        let path = NodePath { surface, node };
        ctx.data(|d| d.get_temp::<(usize, Option<PanelId>)>(skin::tabs_offset_id(path)))
            .map(|(o, _)| o)
    }

    /// Un evento de la rueda (`delta` hacia arriba si es positivo) en `unit`, con `modifiers`.
    fn wheel(unit: egui::MouseWheelUnit, dy: f32, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::MouseWheel {
            unit,
            delta: egui::vec2(0.0, dy),
            phase: egui::TouchPhase::Move,
            modifiers,
        }
    }

    /// Los botones de `out` dentro de `area`, con su rect.
    fn buttons_in(out: &egui::FullOutput, area: egui::Rect) -> Vec<(String, egui::Rect)> {
        crate::workspace::test_util::buttons(out)
            .into_iter()
            .filter_map(|l| node_rect(out, &l).map(|r| (l, r)))
            .filter(|(_, r)| area.contains_rect(*r))
            .collect()
    }

    #[test]
    fn the_wheel_over_a_floating_window_scrolls_it_and_not_the_tabs_below() {
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            many_tabs_below_color(&mut app);
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let out = frames_in(&ctx, &mut app, SIZE);
            if pixel {
                assert!(node_rect(&out, "Next tabs").is_some(), "the bar overflows");
            }
            let bar = leaf(&app, PanelId::Library);
            // La toolbar flotando encima de la barra, baja: lo suyo no cabe y se corre.
            app.workspace.float(
                PanelId::Tools,
                egui::pos2(bar.left() + 20.0, bar.top() - 70.0),
            );
            frames_in(&ctx, &mut app, SIZE);
            let (s, _) = app.workspace.leaf_of(PanelId::Tools).unwrap();
            let width = app
                .workspace
                .window_rect(&ctx, PanelId::Tools)
                .unwrap()
                .width();
            app.workspace
                .state
                .get_window_state_mut(s)
                .unwrap()
                .set_size(egui::vec2(width, 150.0));
            frames_in(&ctx, &mut app, SIZE);
            let out = frames_in(&ctx, &mut app, SIZE);
            let window = app.workspace.window_rect(&ctx, PanelId::Tools).unwrap();
            let at = egui::pos2(window.center().x, bar.top() + 6.0);
            assert!(window.shrink(8.0).contains(at), "{window:?} {at:?}");
            assert!(bar.contains(at) && at.x < bar.right() - 30.0, "{bar:?}");
            let before = buttons_in(&out, window);
            assert!(!before.is_empty(), "the toolbar's buttons");
            let offset = tabs_offset(&ctx, &app, PanelId::Library);
            assert_eq!(offset.is_some(), pixel, "the kit's bar only with Pixel UI");
            // Lo corrido de la barra en egui_dock (con Pixel UI, lo pone el kit): en los dos modos.
            let scroll = leaf_scroll(&app, PanelId::Library);
            frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
            let down = wheel(egui::MouseWheelUnit::Line, -2.0, Default::default());
            frame_in(&ctx, &mut app, vec![down], SIZE);
            frames_in(&ctx, &mut app, SIZE);
            let out = frames_in(&ctx, &mut app, SIZE);
            let scrolled = before
                .iter()
                .any(|(label, r)| node_rect(&out, label).is_some_and(|now| now.top() < r.top()));
            assert!(scrolled, "pixel {pixel}: the window's contents scroll");
            assert_eq!(
                tabs_offset(&ctx, &app, PanelId::Library),
                offset,
                "pixel {pixel}: the tabs below do not page"
            );
            assert_eq!(
                leaf_scroll(&app, PanelId::Library),
                scroll,
                "pixel {pixel}: the tabs below do not scroll"
            );
        }
    }

    /// Lo corrido de la barra de pestañas del grupo de `id` (`LeafNode::scroll` de egui_dock).
    fn leaf_scroll(app: &ChimpApp, id: PanelId) -> f32 {
        let (surface, node) = app.workspace.leaf_of(id).unwrap();
        let path = NodePath { surface, node };
        app.workspace.state.leaf(path).unwrap().scroll
    }

    #[test]
    fn the_wheel_pages_the_overflowing_bar_of_a_floating_window() {
        let mut app = test_app();
        app.workspace
            .float(PanelId::Library, egui::pos2(320.0, 120.0));
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        for id in [
            PanelId::Slices,
            PanelId::Artboards,
            PanelId::Lospec,
            PanelId::Groom,
            PanelId::Lumen,
            PanelId::Model,
            PanelId::Font,
        ] {
            app.workspace.dock(id, DockTarget::Leaf(s, n));
        }
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        frames_in(&ctx, &mut app, SIZE);
        let (s, _) = app.workspace.leaf_of(PanelId::Library).unwrap();
        let height = app
            .workspace
            .window_rect(&ctx, PanelId::Library)
            .unwrap()
            .height();
        app.workspace
            .state
            .get_window_state_mut(s)
            .unwrap()
            .set_size(egui::vec2(260.0, height));
        let out = frames_in(&ctx, &mut app, SIZE);
        assert!(node_rect(&out, "Next tabs").is_some(), "the bar overflows");
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        let path = NodePath {
            surface: s,
            node: n,
        };
        app.workspace.state.leaf_mut(path).unwrap().active = egui_dock::TabIndex(0);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Library), Some(0));
        let bar = leaf(&app, PanelId::Library);
        let at = egui::pos2(bar.left() + 20.0, bar.top() + 6.0);
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        let down = wheel(egui::MouseWheelUnit::Line, -1.0, Default::default());
        frame_in(&ctx, &mut app, vec![down], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Library), Some(1));
    }

    #[test]
    fn with_a_modal_dialog_the_wheel_scrolls_the_dialog_and_not_the_tabs_below() {
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            many_tabs_below_color(&mut app);
            let ctx = egui::Context::default();
            frames_in(&ctx, &mut app, SIZE);
            let bar = leaf(&app, PanelId::Library);
            app.show_message("Test", "A modal dialog");
            // Un diálogo (después del workspace, como los de la app) encima de la barra, con un
            // scroll que se corre con la rueda.
            let dialog = egui::Rect::from_min_size(
                egui::pos2(bar.left() + 10.0, bar.top() - 60.0),
                egui::vec2(200.0, 150.0),
            );
            let scrolled = std::cell::Cell::new(0.0);
            let draw = |ctx: &egui::Context| {
                egui::Window::new("Dialog")
                    .fixed_pos(dialog.min)
                    .fixed_size(dialog.size())
                    .show(ctx, |ui| {
                        let out =
                            egui::ScrollArea::vertical()
                                .id_salt("dialog_lines")
                                .show(ui, |ui| {
                                    for k in 0..80 {
                                        ui.label(format!("Line {k}"));
                                    }
                                });
                        scrolled.set(out.state.offset.y);
                    });
            };
            for _ in 0..4 {
                frame_with(&ctx, &mut app, Vec::new(), SIZE, draw);
            }
            assert!(app.is_modal());
            let offset = tabs_offset(&ctx, &app, PanelId::Library);
            let at = egui::pos2(bar.left() + 40.0, bar.top() + 6.0);
            frame_with(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(at)],
                SIZE,
                draw,
            );
            let down = wheel(egui::MouseWheelUnit::Line, -2.0, Default::default());
            frame_with(&ctx, &mut app, vec![down], SIZE, draw);
            for _ in 0..10 {
                frame_with(&ctx, &mut app, Vec::new(), SIZE, draw);
            }
            assert!(
                scrolled.get() > 0.0,
                "pixel {pixel}: the dialog's list scrolls"
            );
            assert_eq!(
                tabs_offset(&ctx, &app, PanelId::Library),
                offset,
                "pixel {pixel}: the tabs below do not page"
            );
        }
    }

    #[test]
    fn a_trackpad_pages_after_a_notch_and_ctrl_wheel_does_not_page() {
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        frames_in(&ctx, &mut app, SIZE);
        let bar = leaf(&app, PanelId::Color);
        let at = egui::pos2(bar.left() + 20.0, bar.top() + 6.0);
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Color), Some(0));
        // Un trackpad: de a 10 puntos por frame. Hasta juntar un renglón de la rueda, nada.
        let notch = ctx.options(|o| o.input_options.line_scroll_speed);
        let step = wheel(egui::MouseWheelUnit::Point, -10.0, Default::default());
        let frames = (notch / 10.0).ceil() as usize;
        for _ in 1..frames {
            frame_in(&ctx, &mut app, vec![step.clone()], SIZE);
        }
        assert_eq!(
            tabs_offset(&ctx, &app, PanelId::Color),
            Some(0),
            "less than a notch"
        );
        frame_in(&ctx, &mut app, vec![step], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(
            tabs_offset(&ctx, &app, PanelId::Color),
            Some(1),
            "a notch: one tab"
        );
        // Ctrl (o Cmd) con la rueda es zoom: no corre las pestañas.
        let zoom = wheel(egui::MouseWheelUnit::Line, -1.0, egui::Modifiers::COMMAND);
        frame_in(&ctx, &mut app, vec![zoom], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Color), Some(1), "zoom");
        let zoom = wheel(egui::MouseWheelUnit::Line, -1.0, egui::Modifiers::CTRL);
        frame_in(&ctx, &mut app, vec![zoom], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Color), Some(1), "zoom");
    }

    #[test]
    fn what_the_trackpad_left_on_a_bar_is_forgotten_when_the_pointer_leaves_it() {
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        frames_in(&ctx, &mut app, SIZE);
        let bar = leaf(&app, PanelId::Color);
        let at = egui::pos2(bar.left() + 20.0, bar.top() + 6.0);
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        // Casi un renglón (todo menos 10 puntos) y el puntero se va de la barra.
        let notch = ctx.options(|o| o.input_options.line_scroll_speed);
        let step = wheel(egui::MouseWheelUnit::Point, -10.0, Default::default());
        let frames = (notch / 10.0).ceil() as usize;
        for _ in 1..frames {
            frame_in(&ctx, &mut app, vec![step.clone()], SIZE);
        }
        let away = egui::pos2(bar.center().x, bar.bottom() - 20.0);
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(away)], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        // De vuelta, lo que falta del renglón no basta: empieza de cero.
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        frame_in(&ctx, &mut app, vec![step], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Color), Some(0));
    }

    #[test]
    fn with_a_modal_dialog_a_window_with_color_does_not_move_from_a_disabled_panel() {
        // Color sigue vivo con un diálogo; Library, debajo en su ventana, va apagado: arrastrar
        // su fondo no mueve la ventana (ni el relleno de Color: ver el caso del modal).
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            app.workspace
                .float(PanelId::Color, egui::pos2(320.0, 120.0));
            let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
            let below = DockTarget::Split(s, n, egui_dock::Split::Below);
            app.workspace.dock(PanelId::Library, below);
            let ctx = egui::Context::default();
            frames_in(&ctx, &mut app, SIZE);
            app.show_message("Test", "A modal dialog");
            frames_in(&ctx, &mut app, SIZE);
            let window = app.workspace.window_rect(&ctx, PanelId::Color).unwrap();
            let library = leaf(&app, PanelId::Library);
            let at = egui::pos2(library.center().x, library.bottom() - 6.0);
            drag_to(&ctx, &mut app, (at, at + egui::vec2(40.0, 30.0)), SIZE);
            frames_in(&ctx, &mut app, SIZE);
            assert_eq!(
                app.workspace.window_rect(&ctx, PanelId::Color),
                Some(window),
                "pixel {pixel}"
            );
        }
    }

    #[test]
    fn a_modal_puts_back_no_focus_too() {
        let mut w = Workspace::essentials();
        // Sin foco (un camino que no es una hoja lo quita).
        let none = NodePath {
            surface: egui_dock::SurfaceIndex::main(),
            node: egui_dock::NodeIndex(usize::MAX),
        };
        w.state.set_focused_node_and_surface(none);
        assert_eq!(w.state.focused_leaf(), None);
        let frozen = Frozen::take(&w.state);
        let (surface, node) = w.leaf_of(PanelId::Color).unwrap();
        w.state
            .set_focused_node_and_surface(NodePath { surface, node });
        assert!(w.state.focused_leaf().is_some());
        frozen.restore(&mut w.state);
        assert_eq!(w.state.focused_leaf(), None);
    }

    /// Las pestañas de `out` hasta que se ve la última de `id` (FontForge), con la flecha de
    /// adelante; devuelve la salida con ella a la vista.
    fn page_to_the_end(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        mut out: egui::FullOutput,
    ) -> egui::FullOutput {
        let next = node_rect(&out, "Next tabs").expect("the kit arrows");
        let mut steps = 0;
        while node_rect(&out, "Close FontForge").is_none() {
            assert!(steps < 8, "the last tab comes into view");
            click_at(ctx, app, next.center(), egui::PointerButton::Primary, SIZE);
            out = frames_in(ctx, app, SIZE);
            steps += 1;
        }
        out
    }

    /// Lo vacío al final de una barra que no cabe con la última a la vista: entre ella y las
    /// flechas.
    fn empty_end(out: &egui::FullOutput) -> egui::Pos2 {
        let last = node_rect(out, "FontForge").expect("the last tab");
        let prev = node_rect(out, "Previous tabs").unwrap();
        assert!(prev.left() - last.right() > 2.0, "{last:?} {prev:?}");
        egui::pos2((last.right() + prev.left()) / 2.0, prev.center().y)
    }

    #[test]
    fn the_empty_end_of_an_overflowing_window_bar_still_drags_the_window() {
        let mut app = test_app();
        app.workspace
            .float(PanelId::Library, egui::pos2(320.0, 120.0));
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        for id in [
            PanelId::Slices,
            PanelId::Artboards,
            PanelId::Lospec,
            PanelId::Groom,
            PanelId::Lumen,
            PanelId::Model,
            PanelId::Font,
        ] {
            app.workspace.dock(id, DockTarget::Leaf(s, n));
        }
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        frames_in(&ctx, &mut app, SIZE);
        // Angosta: sus pestañas no caben.
        let (s, _) = app.workspace.leaf_of(PanelId::Library).unwrap();
        let height = app
            .workspace
            .window_rect(&ctx, PanelId::Library)
            .unwrap()
            .height();
        app.workspace
            .state
            .get_window_state_mut(s)
            .unwrap()
            .set_size(egui::vec2(260.0, height));
        let out = frames_in(&ctx, &mut app, SIZE);
        let out = page_to_the_end(&ctx, &mut app, out);
        let at = empty_end(&out);
        let window = app.workspace.window_rect(&ctx, PanelId::Library).unwrap();
        drag_to(&ctx, &mut app, (at, at + egui::vec2(40.0, 30.0)), SIZE);
        frames_in(&ctx, &mut app, SIZE);
        let moved = app.workspace.window_rect(&ctx, PanelId::Library).unwrap();
        assert_eq!(moved.min, window.min + egui::vec2(40.0, 30.0));
        // Ninguna pestaña se movió de grupo ni de lugar.
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        assert_eq!(app.workspace.state_leaf_tabs(s, n).unwrap().0.len(), 8);
    }

    #[test]
    fn a_tab_dropped_past_the_last_tab_goes_to_the_end() {
        // Con flechas la última pestaña lleva en egui_dock lo que falta hasta ellas (`tail`): lo
        // que se suelta ahí va al final, como en lo vacío de una barra que cabe (no antes de ella).
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let out = frames_in(&ctx, &mut app, SIZE);
        let out = page_to_the_end(&ctx, &mut app, out);
        let at = empty_end(&out);
        // La franja de agarre de la toolbar (sola en su grupo).
        let tools = leaf(&app, PanelId::Tools);
        let from = egui::pos2(tools.center().x, tools.top() + 1.0);
        // Como `drag_to`, pero se queda un rato en `at` antes de soltar: egui_dock retiene un
        // momento el destino de antes (el lienzo, por donde pasó).
        let press = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(from)], SIZE);
        frame_in(&ctx, &mut app, vec![press(from, true)], SIZE);
        for k in 1..=40 {
            let to = from + (at - from) * (k as f32 / 4.0).min(1.0);
            frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(to)], SIZE);
        }
        frame_in(&ctx, &mut app, vec![press(at, false)], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        let tabs = app.workspace.state_leaf_tabs(s, n).unwrap().0;
        assert_eq!(tabs.len(), 9, "{tabs:?} {}", snapshot(&ctx, &app));
        assert_eq!(tabs.last(), Some(&PanelId::Tools), "{tabs:?}");
        assert_eq!(tabs[7], PanelId::Font, "{tabs:?}");
    }

    /// Medición (no revisa nada): lo que tarda `show` por frame mientras se arrastra un divisor 60
    /// frames (uno por punto), con Essentials y tres paneles acoplados más (Library en pestaña con
    /// Color, Groom debajo de ellos y Lumen en una columna entre el lienzo y la de Color), con y
    /// sin Pixel UI. Se corre a mano, en release:
    /// `cargo test --release -p cs-ui frame_time -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, run by hand in release"]
    fn frame_time_while_dragging_a_divider() {
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
            app.workspace.dock(PanelId::Library, DockTarget::Leaf(s, n));
            let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
            let below = DockTarget::Split(s, n, egui_dock::Split::Below);
            app.workspace.dock(PanelId::Groom, below);
            let (s, n) = app.workspace.leaf_of(PanelId::Canvas).unwrap();
            let right = DockTarget::Split(s, n, egui_dock::Split::Right);
            app.workspace.dock(PanelId::Lumen, right);
            let ctx = egui::Context::default();
            frames_in(&ctx, &mut app, SIZE);
            frames_in(&ctx, &mut app, SIZE);
            // El divisor entre el lienzo y Lumen, de a un punto a la izquierda.
            let (canvas, lumen) = (leaf(&app, PanelId::Canvas), leaf(&app, PanelId::Lumen));
            let from = egui::pos2((canvas.right() + lumen.left()) / 2.0, canvas.center().y);
            let press = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(from)], SIZE);
            frame_in(&ctx, &mut app, vec![press(from, true)], SIZE);
            let mut times = Vec::new();
            for k in 1..=60 {
                let at = from - egui::vec2(k as f32, 0.0);
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
                    events: vec![egui::Event::PointerMoved(at)],
                    ..Default::default()
                };
                let mut took = std::time::Duration::ZERO;
                let mut output = ctx.run_ui(input, |ui| {
                    app.pixel.update(ui.ctx(), &app.view);
                    app.begin_panel_frame();
                    egui::CentralPanel::default().show(ui, |ui| {
                        let start = std::time::Instant::now();
                        show(&mut app, ui, false);
                        took += start.elapsed();
                    });
                });
                output.textures_delta.clear();
                times.push(took.as_secs_f64() * 1000.0);
            }
            frame_in(&ctx, &mut app, vec![press(from, false)], SIZE);
            assert!(
                leaf(&app, PanelId::Canvas).right() < canvas.right(),
                "it was dragged"
            );
            times.sort_by(f64::total_cmp);
            let mean = times.iter().sum::<f64>() / times.len() as f64;
            eprintln!(
                "show while dragging (pixel {pixel}): mean {mean:.2} ms, median {:.2} ms, \
                 p95 {:.2} ms, min {:.2} ms, max {:.2} ms over {} frames",
                times[times.len() / 2],
                times[times.len() * 95 / 100],
                times[0],
                times[times.len() - 1],
                times.len()
            );
        }
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
