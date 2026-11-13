//! El workspace en pantalla: [`show`] arma el `DockArea` con la piel en toda el área central,
//! hospeda cada panel ([`Viewer`]; el lienzo, con `ChimpApp::canvas_area`; la toolbar, con su
//! scroll y sus márgenes de siempre; el timeline debajo del lienzo, con el marco del panel de abajo
//! de antes; el Preview solo en su ventana, con la del diseño: ver [`PreviewWindow`], en
//! [`preview_window`](super::preview_window)) y pega lo que va encima de egui_dock (las pestañas y
//! la × del kit, la franja de agarre y el botón « de cada grupo, el "Dock" del menú de las
//! pestañas de lo que flota: ver [`tabs`](super::tabs); los
//! divisores de la toolbar y del timeline tapados: ver [`cover_tools_divider`] y
//! [`cover_timeline_divider`], en [`dividers`](super::dividers); el marco y la barra de la ventana
//! del Preview y la tira de íconos: `ChimpApp::strip_area`). Con un diálogo modal, lo de
//! [`modal`](super::modal). También lleva el ancho de la columna derecha (la preferencia del
//! usuario, `view.palette_width`, el que tomó sola, [`host_width`], y el arrastre de su divisor) y
//! el alto del timeline (`view.timeline_height`, ver [`timeline_height`]).

use std::collections::HashMap;

use eframe::egui;
use egui_dock::{DockArea, TabViewer};

use super::dividers::{cover_timeline_divider, cover_tools_divider, paint_crossings};
use super::drop::{Bounds, dragged_tab, paint_tab_drop, see_through, tab_drag_released, tab_drop};
use super::fit::{
    Minima, block_canvas_drag, chrome, fit_splits, fit_windows, fractions, half, minima, pin_sides,
    quadrant, record_sides, tidy_canvas, toolbar_width,
};
use super::modal::{Frozen, disable_unfaded, hold_windows, modal_shields, plain_preview_title};
use super::model::floats;
use super::preview_window::PreviewWindow;
use super::tabs::{
    close_marks, collapse_buttons, dock_menus, dropped_past_last, tab_arrows, tab_wheel, to_the_end,
};
use super::{SurfaceIndex, Workspace, dock_id, skin};
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
    /// Mientras se arrastra la única pestaña de una ventana flotante, ese panel (ver
    /// `drop::see_through`): el grupo de su ventana no es destino de egui_dock.
    see_through: Option<PanelId>,
    /// Si el grupo que egui_dock acaba de dibujar es el de `see_through`: la pone `ui` y la gasta
    /// `allowed_in_windows` (ver el punto 12 de `skin`).
    own_group: std::cell::Cell<bool>,
    /// Lo mínimo de cada panel en su grupo (el de `workspace_minima`, con el margen de su cuerpo).
    minima: &'a Minima,
    /// Si en este frame egui_dock suelta una pestaña (ver [`Viewer::squeeze`]).
    dropping: bool,
    /// Si algún panel se acomodó en un grupo más angosto que su mínimo (ver [`Viewer::squeeze`]).
    squeezed: bool,
}

impl Viewer<'_> {
    /// El margen del cuerpo de `tab` en su grupo: el del panel derecho de hoy para Color, el del
    /// izquierdo para la toolbar, ninguno para el lienzo y el timeline justo debajo de él (su
    /// marco es suyo), el de su ventana de antes para el Preview solo, el de los paneles del dock
    /// para los demás; pegado al divisor de la columna, un divisor menos a la izquierda.
    fn body_margin(&self, tab: &PanelId) -> egui::Margin {
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
        margin
    }

    /// El ancho mínimo de lo de adentro de `id`, si su cuerpo en `ui` es más angosto, el frame en
    /// que egui_dock suelta una pestaña (`dropping`): parte el grupo de destino por la mitad y lo
    /// dibuja así antes de que `fit_splits` le dé a cada lado su mínimo (ver el punto 15 de
    /// `skin`); partiendo uno angosto (la toolbar, de 35 pt), un lado queda más angosto que su
    /// panel. Ese frame se vuelve a dibujar (ver `show`). Solo ese: en reposo los mínimos no son
    /// exactos para todos (la toolbar flotando, el timeline angosto de algunos tests) y lo de
    /// adentro se acomoda en lo que hay, como siempre. El lienzo no (lleva el suyo), ni el
    /// Preview solo en su ventana (su ventana es la de antes, de su tamaño).
    fn squeeze(&self, ui: &egui::Ui, id: PanelId) -> Option<f32> {
        if !self.dropping
            || id == PanelId::Canvas
            || (id == PanelId::Preview && self.preview_window.is_some())
        {
            return None;
        }
        let min = self.minima.get(&id)?.x - self.body_margin(&id).sum().x;
        (ui.max_rect().width() + 0.5 < min).then_some(min)
    }

    /// Lo de adentro de `id` en `ui` (el cuerpo de su grupo).
    fn panel(&mut self, ui: &mut egui::Ui, id: PanelId) {
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
        self.own_group.set(self.see_through == Some(id));
        let spacing = ui.spacing().item_spacing;
        ui.set_style(self.style.clone());
        ui.spacing_mut().item_spacing = spacing;
        // El frame en que egui_dock suelta, en un grupo más angosto que su mínimo (ver
        // `squeeze`), lo de adentro va de su mínimo, recortado a su grupo: no se acomoda en menos
        // (la caja de la paleta de Color, de 0 de ancho en la mitad de la toolbar, medía -6 sin
        // sus bordes, y egui truena en debug con un tamaño negativo).
        if let Some(min) = self.squeeze(ui, id) {
            let body = ui.max_rect();
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(body.with_max_x(body.left() + min))
                    .layout(*ui.layout()),
            );
            child.set_clip_rect(ui.clip_rect());
            self.squeezed = true;
            self.panel(&mut child, id);
            return;
        }
        self.panel(ui, id);
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
    /// Justo después de dibujar el grupo de la ventana que se arrastra (`see_through`), no: así
    /// egui_dock no lo toma como destino y queda el de debajo (ver el punto 12 de `skin`). Por
    /// eso no es un predicado puro: fuera de egui_dock se usa `model::floats`.
    fn allowed_in_windows(&self, tab: &mut PanelId) -> bool {
        !self.own_group.take() && floats(*tab)
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
        style.tab_body.inner_margin = self.body_margin(tab);
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
    let mut style = match skin {
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
    // Color fuera de la columna, de ese mismo ancho de grupo (en la columna el divisor va dentro
    // de `palette_width` y su margen cuenta desde la línea; fuera, el grupo entero es suyo: así
    // lo de adentro mide lo mismo).
    workspace.sides.palette = Some(preferred + sep / 2.0);
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
    // Las ventanas que ya había (lo que egui_dock ponga en una ventana nueva va donde se soltó;
    // lo que junte a una de estas, no la mueve).
    let windows_before: Vec<SurfaceIndex> = workspace
        .state
        .iter_all_tabs()
        .filter(|(path, _)| !path.surface.is_main())
        .map(|(path, _)| path.surface)
        .collect();
    let pointer = ctx.input(|i| i.pointer.latest_pos());
    // La pestaña que se arrastra sobre la franja de una orilla o sobre una tira: va ahí, no donde
    // la pondría egui_dock (ver `drop::tab_drop`), y la zona que se ve es esa. Se decide con lo
    // dibujado en el último frame, antes del `DockArea` (que mueve lo soltado al empezar): lo
    // que se pinta es lo que pasa al soltar.
    let tab_drop = match (dragged_tab(&ctx), pointer) {
        (Some(id), Some(p)) => tab_drop(&workspace, &ctx, id, p).map(|d| (id, p, d)),
        _ => None,
    };
    if tab_drop.is_some() {
        skin::hide_drop_zone(&mut style);
    }
    // La ventana cuya única pestaña se arrastra se ve a través y no es destino (ver
    // `drop::see_through`). Con el puntero encima de ella, egui_dock no se queda con el grupo
    // que recordaba (al empezar, el de ella misma): lo de debajo cuenta en cuanto llega (ver el
    // punto 13 de `skin`).
    let through = see_through(&workspace.state, dragged_tab(&ctx));
    let over_own = through.zip(pointer).is_some_and(|((_, s), p)| {
        ctx.memory(|m| m.area_rect(skin::window_area_id(s)))
            .is_some_and(|r| r.contains(p))
    });
    if over_own {
        style.overlay.feel.max_preference_time = 0.0;
    }
    // Mientras se arrastra una pestaña sobre el lienzo: egui_dock pinta el lienzo entero ("como
    // pestaña"), pero lo que pasa es que se parte al lado (`tidy_canvas`): se pinta esa mitad.
    let canvas_drop = match (dragged_tab(&ctx).filter(|_| tab_drop.is_none()), pointer) {
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
        see_through: through.map(|(id, _)| id),
        own_group: Default::default(),
        minima: &m,
        dropping: ctx.input(|i| i.pointer.primary_released()) && dragged_tab(&ctx).is_some(),
        squeezed: false,
    };
    let strip = workspace.strip_at(Edge::Right).to_vec();
    // Con un diálogo modal, lo que el `DockArea` cambiaría (ver `Frozen`) se repone después, y las
    // ventanas flotantes no se mueven arrastrando su fondo (ver `hold_windows`).
    let frozen = modal.then(|| Frozen::take(&workspace.state));
    let past_last = dropped_past_last(&ctx, &bars);
    if modal {
        // Sin Pixel UI el Preview solo en su ventana se mueve de su barra de título, que es de la
        // ventana de egui (la de antes no miraba el modal): eso sigue.
        let title = bare_preview
            .filter(|_| skin.is_none())
            .and_then(|s| plain_preview_title(ui, s));
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
            disable_unfaded(ui);
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
    // Si egui_dock movió algo (un panel soltado: lo hace al empezar el `DockArea`, así que este
    // frame ya se dibujó así).
    let reshaped = before.reshaped(&workspace.state);
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
    if let Some(window) = viewer.preview_window.take()
        && let Some(skin) = skin
        && window.chrome(ui, skin, &mut workspace, viewer.app)
    {
        viewer.closed.push(PanelId::Preview);
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
        squeezed,
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
            canvas.strips.push(dock::StripArea {
                edge: Edge::Right,
                rect,
                buttons: std::mem::take(&mut canvas.dock.buttons),
            });
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
            canvas.strips.push(dock::StripArea {
                edge,
                rect,
                buttons: out.buttons,
            });
        }
    }
    // Lo de este frame, para saber dónde cae lo que se suelta en una orilla o en una tira.
    workspace.bounds = Some(Bounds {
        area,
        dock: dock_rect,
        strips: canvas
            .as_ref()
            .map(|c| c.strips.clone())
            .unwrap_or_default(),
    });
    if let Some((id, p, drop)) = tab_drop
        && !ctx.input(|i| i.pointer.primary_released())
    {
        paint_tab_drop(ui, skin, &workspace, (id, p), drop);
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
    // El ancho de la columna es de Color: solo se toma (y solo se guarda lo arrastrado de su
    // divisor, ver `record_sides`) con Color en ella.
    let column = workspace
        .column_rect()
        .filter(|_| workspace.column_panels().contains(&PanelId::Color));
    if let Some(points) = dragged.column {
        // A 1/64 de punto: sin el ruido de ir de puntos a fracción y de vuelta (un arrastre de
        // 40 puntos guarda 40, no 39.99997), y sin correr el divisor (ver `record_sides`).
        let width = (points + sep / 2.0).max(column_min);
        app.view.palette_width = Some((width * 64.0).round() / 64.0);
    } else if app.view.palette_width.is_none()
        && !reshaped
        && let Some(rect) = column
    {
        // Sin una elegida, la que toma sola (más lo que se pase Color, si está en ella); si es más
        // que su mínimo, es suya (no la de un panel acoplado ancho). No la de un frame en que
        // egui_dock acopló algo: la columna que parte va a la mitad (del frame siguiente en
        // adelante, del ancho de siempre).
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
    // Dónde puede haber marcadores este frame (con Pixel UI): el DockArea y sus ventanas, fuera
    // del cuerpo de los grupos; y la ventana que se ve a través mientras se arrastra (hasta que
    // se suelta), con o sin Pixel UI (ver `skin::Armed`).
    let mut armed = skin::Armed::new(shared_skin.clone());
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
    let faded = through.filter(|_| !ctx.input(|i| i.pointer.primary_released()));
    armed.faded = faded.map(|(_, s)| skin::window_layer(s));
    armed.faded_leaf = faded.and_then(|(id, _)| workspace.leaf_rect(id));
    skin::arm(&ctx, armed);
    // "Dock" en el menú de la pestaña de lo que flota: se acopla más abajo (con el menú cerrado).
    // Antes de cerrar lo que se cerró este frame: `hits` lleva los nodos de cuando se dibujó, y
    // cerrar una pestaña puede recorrerlos.
    let home = if modal {
        None
    } else {
        dock_menus(&ctx, &workspace, &hits)
    };
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
    // Soltada en la franja de una orilla o sobre una tira (ver `drop::tab_drop`): de donde la haya
    // puesto egui_dock, ahí.
    let ours = released.and_then(|(id, at)| {
        tab_drop
            .filter(|&(dragged, _, _)| dragged == id)
            .map(|(_, _, drop)| (id, at, drop))
    });
    if let Some((id, at, drop)) = ours {
        workspace.apply_tab_drop(id, at, drop);
        changed = true;
    } else if let Some((id, at)) = released {
        // Lo que egui_dock partió para él (junto al lienzo, p. ej.) va del ancho de siempre del
        // panel, no de la mitad con que lo parte.
        if reshaped {
            workspace.pin_new_side(id);
        }
        // Una pestaña (o un grupo sin barra) de egui_dock soltada fuera del árbol y de las
        // ventanas (en la barra de estado, la toolbar...): egui_dock no hace nada (solo suelta
        // sobre sus grupos); aquí se pone a flotar donde se soltó, como un ícono de la tira. La
        // ventana que se veía a través no cuenta (no es destino): sin nada debajo, su panel va
        // donde se soltó.
        let own = through.map(|(_, s)| s);
        let windows: Vec<egui::Rect> = workspace
            .state
            .iter_surfaces_indexed()
            .filter(|(s, _)| !s.is_main() && Some(*s) != own)
            .filter_map(|(s, _)| ctx.memory(|m| m.area_rect(skin::window_area_id(s))))
            .collect();
        let outside = !dock_rect.contains(at) && !windows.iter().any(|r| r.contains(at));
        if id != PanelId::Canvas && outside {
            workspace.float(id, at);
        } else if let Some((surface, _)) = workspace.leaf_of(id).filter(|(s, _)| !s.is_main())
            && !windows_before.contains(&surface)
        {
            // egui_dock lo puso a flotar en una ventana nueva (soltado en el anillo de "ventana
            // nueva", o en su propia ventana): su ventana, donde se soltó (egui no siempre
            // respeta la esquina del primer frame).
            workspace.placing.retain(|(p, _, _)| *p != id);
            workspace
                .placing
                .push((id, at, super::fit::NEW_WINDOW_FRAMES));
        }
    }
    if let Some((surface, node)) = collapse_buttons(ui, skin, &workspace, modal) {
        workspace.collapse_to_icons(surface, node);
    }
    if let Some(id) = home {
        workspace.dock_home(id);
        changed = true;
    }
    // Si un panel se soltó en otro grupo (o junto al lienzo), la barra de algún grupo cambia: este
    // frame ya se dibujó con la de antes, así que se descarta y se vuelve a dibujar (sin el
    // parpadeo de un frame).
    changed |= workspace.refresh_tab_bars() | bars_moved;
    // Un panel acomodado en su mínimo en un grupo más angosto (egui_dock acaba de soltar, ver
    // `Viewer::squeeze`): con los lados en su mínimo, se vuelve a dibujar.
    changed |= squeezed;
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
/// margen de ese lado cuenta desde la línea). Con Pixel UI, el timeline lleva su marco: debajo
/// del lienzo, el aire de su derecha (`timeline::pixel_panel_frame`); en otro lugar, el margen de
/// su grupo a cada lado (`timeline_margin`), así lo de adentro nunca mide menos que su mínimo
/// (`timeline::MIN_WIDTH`, 86). Debajo del lienzo eso, con la tira, es 115: el lado del lienzo
/// (y por eso lo de al lado) no se angosta de ahí, 25 más que el lienzo solo (90). Sin Pixel UI,
/// el del timeline es el de antes (`TIMELINE_PLAIN_MIN_WIDTH`, 64; con la tira, 90, el del
/// lienzo): tiene scroll a lo ancho.
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
    let below = w.timeline_below();
    if let Some(min) = m.get_mut(&PanelId::Timeline) {
        if skin.is_some() {
            min.x += if below {
                f32::from(dock::LEFT_AIR)
            } else {
                2.0 * f32::from(pixel_ui::WORKSPACE_MARGIN)
            };
        }
        if below {
            min.x += strip;
        }
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

/// Los mínimos `m` con los de los paneles pegados al divisor izquierdo de la columna derecha
/// (`beside`) un divisor (`sep`) más angostos: su margen izquierdo cuenta desde la línea, como en
/// el panel derecho de antes (ver `Viewer::tab_style_override`), así que la columna mínima (con el
/// divisor) mide el mínimo del panel y lo de adentro, lo mismo que en cualquier otro lugar.
fn beside_minima(mut m: Minima, beside: &[PanelId], sep: f32) -> Minima {
    for id in beside {
        if let Some(min) = m.get_mut(id) {
            min.x = (min.x - sep).max(0.0);
        }
    }
    m
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

#[cfg(test)]
mod tests {
    use eframe::egui;

    use egui_dock::NodePath;

    use super::*;
    use crate::workspace::test_util::{
        SIZE, button, click, close_tab, drag_to, frame, frame_in, frames, frames_in, icon_label,
        leaf, node_rect, test_app,
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
