//! El workspace (fase C del docking del lado derecho): dónde está cada panel. Cada uno vive en un
//! solo lugar: en la tira de íconos (el dock de hoy), acoplado en la superficie principal (un
//! grupo de pestañas en el árbol de egui_dock) o flotando en una ventana. [`show`] muestra el
//! `DockArea` con la piel pixel art (ver `skin`) y hospeda cada panel con
//! [`ChimpApp::draw_panel`].
//!
//! Decisión del usuario (2026-11-10): un grupo acoplado con **un solo panel no lleva barra de
//! pestañas** (se ve como hoy); al pasar el puntero por su borde de arriba se resalta una franja
//! delgada para agarrarlo. Con dos o más, la barra con las pestañas del kit. Las ventanas
//! flotantes siempre llevan su barra: es su título y de ahí se arrastran.
//!
//! Solo este módulo usa egui_dock.
//!
//! El marco de las ventanas flotantes se viste con un `ui.scope` alrededor del `DockArea` cuyo
//! estilo lleva el color marcador (`skin::window_visuals`): funciona porque egui_dock 0.21.1 arma
//! el marco con `Frame::window(ui.style())` del `Ui` que se le pasa, mientras que el contenido de
//! cada pestaña usa el estilo del contexto. Si eso cambia al subir de versión, hay que revisarlo.

mod skin;

use std::collections::HashMap;

use eframe::egui;
use egui_dock::{DockArea, DockState, Node, NodePath, TabViewer, Tree};
pub(crate) use egui_dock::{NodeIndex, Split, SurfaceIndex};

use crate::ChimpApp;
use crate::app_panels::{PanelAction, PanelCtx};
use crate::panel_registry::{Body, PanelId};
use crate::skin::Skin;
use crate::{dock, kit, pixel_ui};

/// El alto de fábrica de un panel que se pone a flotar (si su ficha no pide más).
const FLOAT_HEIGHT: f32 = 300.0;

/// Dónde acoplar un panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DockTarget {
    /// Como pestaña del grupo `(superficie, nodo)`.
    Leaf(SurfaceIndex, NodeIndex),
    /// Partiendo el grupo `(superficie, nodo)`: el panel va del lado `Split`.
    Split(SurfaceIndex, NodeIndex, Split),
    /// En el grupo con el foco de la superficie principal (o el primero que haya).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "solo los tests por ahora; la tarea 6 acopla desde la tira"
        )
    )]
    Focused,
}

/// El acomodo de los paneles: el árbol de egui_dock (acoplados y flotantes) y la tira de íconos.
#[derive(Clone, Debug)]
pub(crate) struct Workspace {
    state: DockState<PanelId>,
    /// Los paneles en la tira, en el orden de `PanelId::ALL`.
    strip: Vec<PanelId>,
    /// Es el que queda en `app.workspace` mientras [`show`] tiene prestado el de verdad (los
    /// paneles se dibujan con la app entera): cambiarlo no serviría de nada.
    lent: bool,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace::essentials()
    }
}

impl Workspace {
    /// El acomodo de fábrica: Color acoplado solo en la superficie principal y los paneles del
    /// dock en la tira, en su orden.
    pub(crate) fn essentials() -> Workspace {
        Workspace {
            state: DockState::new(vec![PanelId::Color]),
            strip: PanelId::DOCK.to_vec(),
            lent: false,
        }
    }

    /// Lo que queda en `app.workspace` mientras [`show`] dibuja los paneles.
    fn placeholder() -> Workspace {
        Workspace {
            lent: true,
            ..Workspace::essentials()
        }
    }

    /// Los cambios a un workspace prestado se perderían: es un error (solo se revisa en debug).
    fn check_not_lent(&self) {
        debug_assert!(
            !self.lent,
            "app.workspace is a placeholder while workspace::show draws the panels"
        );
    }

    /// Un acomodo de demostración (`CHIMPSPRITE_DEMO_WORKSPACE`, para capturas): `library-tab`
    /// (Library en el grupo de Color), `split` (Groom debajo de Color) o `float` (Lumen
    /// flotando). `None` si no es ninguno de esos.
    pub(crate) fn demo(name: &str) -> Option<Workspace> {
        let mut w = Workspace::essentials();
        let (surface, node) = w.leaf_of(PanelId::Color)?;
        match name {
            "library-tab" => w.dock(PanelId::Library, DockTarget::Leaf(surface, node)),
            "split" => w.dock(
                PanelId::Groom,
                DockTarget::Split(surface, node, Split::Below),
            ),
            "float" => w.float(PanelId::Lumen, egui::pos2(320.0, 120.0)),
            _ => return None,
        }
        Some(w)
    }

    /// Los paneles de la tira, en orden.
    pub(crate) fn strip(&self) -> &[PanelId] {
        &self.strip
    }

    /// Si `id` está acoplado (en la superficie principal).
    pub(crate) fn docked(&self, id: PanelId) -> bool {
        self.state.find_main_surface_tab(&id).is_some()
    }

    /// Si `id` está flotando (en una ventana).
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "solo los tests por ahora; lo lee la tarea 6")
    )]
    pub(crate) fn floating(&self, id: PanelId) -> bool {
        self.state
            .find_tab(&id)
            .is_some_and(|path| !path.surface.is_main())
    }

    /// El grupo `(superficie, nodo)` donde está `id`, si está en el árbol.
    pub(crate) fn leaf_of(&self, id: PanelId) -> Option<(SurfaceIndex, NodeIndex)> {
        self.state.find_tab(&id).map(|p| (p.surface, p.node))
    }

    /// El rect del grupo de `id` (el del frame pasado), si está en el árbol.
    #[cfg(test)]
    pub(crate) fn leaf_rect(&self, id: PanelId) -> Option<egui::Rect> {
        let path = self.state.find_tab(&id)?;
        self.state.leaf(path.node_path()).ok().map(|leaf| leaf.rect)
    }

    /// Si el grupo de `id` va sin barra de pestañas.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "solo los tests por ahora; lo lee la tarea 6")
    )]
    pub(crate) fn tab_bar_hidden_of(&self, id: PanelId) -> bool {
        self.state
            .find_tab(&id)
            .and_then(|p| self.state.leaf(p.node_path()).ok())
            .is_some_and(|leaf| leaf.tab_bar_hidden)
    }

    /// Saca `id` de donde esté (la tira o el árbol). Un grupo o una ventana que se vacían
    /// desaparecen.
    fn take(&mut self, id: PanelId) {
        self.check_not_lent();
        self.strip.retain(|&p| p != id);
        if let Some(path) = self.state.find_tab(&id) {
            self.state.remove_tab(path);
        }
    }

    /// Acopla `id` en `into`. Si `into` no es un grupo que exista, en el del foco.
    pub(crate) fn dock(&mut self, id: PanelId, into: DockTarget) {
        // El grupo de destino se recuerda por otra de sus pestañas: sacar `id` de su lugar puede
        // mover los índices del árbol.
        let target = match into {
            DockTarget::Leaf(surface, node) | DockTarget::Split(surface, node, _) => self
                .state
                .leaf(NodePath { surface, node })
                .ok()
                .map(|leaf| leaf.tabs.iter().copied().find(|&t| t != id)),
            DockTarget::Focused => None,
        };
        match (target, into) {
            // Su propio grupo, donde ya está solo: nada que hacer.
            (Some(None), _) => self.strip.retain(|&p| p != id),
            (Some(Some(anchor)), DockTarget::Leaf(..)) => {
                self.take(id);
                if let Some(path) = self.state.find_tab(&anchor)
                    && let Ok(leaf) = self.state.leaf_mut(path.node_path())
                {
                    leaf.append_tab(id);
                }
            }
            (Some(Some(anchor)), DockTarget::Split(_, _, split)) => {
                self.take(id);
                if let Some(path) = self.state.find_tab(&anchor) {
                    self.state
                        .split(path.node_path(), split, 0.5, Node::leaf(id));
                }
            }
            _ => {
                self.take(id);
                self.push_focused(id);
            }
        }
        // Si el destino no quedó (no debería), al grupo del foco: nunca se pierde.
        if self.state.find_tab(&id).is_none() {
            self.push_focused(id);
        }
    }

    /// `id` al grupo con el foco de la superficie principal, o al primero que haya.
    fn push_focused(&mut self, id: PanelId) {
        match self.state.focused_leaf() {
            Some(path) if path.surface.is_main() => {
                if let Ok(leaf) = self.state.leaf_mut(path) {
                    leaf.append_tab(id);
                    return;
                }
                self.state.main_surface_mut().push_to_focused_leaf(id);
            }
            _ => self.state.main_surface_mut().push_to_focused_leaf(id),
        }
    }

    /// Pone `id` a flotar solo en una ventana nueva con la esquina en `at`.
    pub(crate) fn float(&mut self, id: PanelId, at: egui::Pos2) {
        self.take(id);
        let spec = id.spec();
        let size = egui::vec2(spec.reference_width, spec.min_height.max(FLOAT_HEIGHT));
        let surface = self.state.add_window(vec![id]);
        if let Some(window) = self.state.get_window_state_mut(surface) {
            window.set_position(at.round());
            window.set_size(size.round());
        }
    }

    /// Regresa `id` a la tira, en su lugar (el orden de `PanelId::ALL`).
    #[allow(
        clippy::wrong_self_convention,
        reason = "no es una conversión: manda el panel a la tira"
    )]
    pub(crate) fn to_strip(&mut self, id: PanelId) {
        self.take(id);
        let rank = |p: &PanelId| PanelId::ALL.iter().position(|q| q == p);
        let at = self.strip.partition_point(|p| rank(p) < rank(&id));
        self.strip.insert(at, id);
    }

    /// Lo más angosta que puede quedar la columna de la superficie principal: cada panel
    /// acoplado en su mínimo (ver [`minima`]); los que van lado a lado, sumados.
    pub(crate) fn column_min_width(&self, ui: &egui::Ui, skin: Option<&Skin>) -> f32 {
        let (sep, bar) = chrome(ui, skin);
        let m = minima(ui, skin);
        node_min(self.state.main_surface(), NodeIndex::root(), &m, sep, bar).x
    }

    /// Regresa a la tira todos los paneles del grupo `(surface, node)`.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "solo los tests por ahora; el botón de colapsar es de la tarea 6"
        )
    )]
    pub(crate) fn collapse_leaf(&mut self, surface: SurfaceIndex, node: NodeIndex) {
        let tabs = match self.state.leaf(NodePath { surface, node }) {
            Ok(leaf) => leaf.tabs.clone(),
            Err(_) => return,
        };
        for id in tabs {
            self.to_strip(id);
        }
    }

    /// La barra de pestañas de cada grupo acoplado: sin ella con un solo panel, con ella con dos
    /// o más. Las ventanas flotantes siempre la llevan (es su título y de ahí se arrastran).
    /// Devuelve si alguna cambió.
    pub(crate) fn refresh_tab_bars(&mut self) -> bool {
        let mut changed = false;
        for (path, leaf) in self.state.iter_leaves_mut() {
            let hidden = path.surface.is_main() && leaf.tabs.len() <= 1;
            changed |= leaf.tab_bar_hidden != hidden;
            leaf.tab_bar_hidden = hidden;
        }
        changed
    }

    /// Cada panel en un solo lugar: lo que está en el árbol sale de la tira (y la tira sin
    /// repetidos).
    fn tidy(&mut self) {
        let state = &self.state;
        let mut seen = Vec::new();
        self.strip.retain(|&p| {
            let keep = state.find_tab(&p).is_none() && !seen.contains(&p);
            seen.push(p);
            keep
        });
    }
}

/// Lo que egui_dock ve de los paneles: los dibuja con [`ChimpApp::draw_panel`] y junta lo que
/// piden (se aplica después del `DockArea`, con la app entera).
struct Viewer<'a> {
    app: &'a mut ChimpApp,
    modal: bool,
    /// Con Pixel UI: las pestañas del kit van encima (las de egui_dock, transparentes).
    skinned: bool,
    /// El ancho de la pestaña del kit de cada panel.
    widths: &'a HashMap<PanelId, f32>,
    /// El margen de cada panel en su grupo: el del panel derecho de hoy para Color, el de los
    /// paneles del dock para los demás.
    color_margin: egui::Margin,
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
        // El fondo del grupo (dentro del scroll de egui_dock el `max_rect` puede no serlo): hasta
        // ahí llega la lista de Lospec.
        let bottom = ui.clip_rect().bottom().min(ui.max_rect().bottom()).floor();
        // El cuerpo (sin el margen), para comparar con lo que ocupa el contenido.
        let body = ui.max_rect();
        let panel = PanelCtx {
            modal: self.modal,
            bottom: Some(bottom),
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
        egui_dock::tab_viewer::OnCloseResponse::Close
    }

    fn is_closeable(&self, tab: &PanelId) -> bool {
        tab.spec().closable
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
        style.tab_body.inner_margin = if *tab == PanelId::Color {
            self.color_margin
        } else {
            egui::Margin::same(dock::FLYOUT_MARGIN)
        };
        Some(style)
    }

    fn scroll_bars(&self, _tab: &PanelId) -> [bool; 2] {
        // El scroll es de cada panel (el del kit, o el suyo propio).
        [false, false]
    }
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

/// El ancho que el workspace le reporta a su anfitrión (desde su borde izquierdo), un
/// `egui::Panel` que se ajusta a lo que ocupa su contenido: de ese ajuste sale el ancho de
/// fábrica de la columna en los primeros frames, cuando la escala y la fuente de Pixel UI todavía
/// cambian. Como el panel derecho de antes con Color adentro (que en esos frames quedaba igual en
/// un ancho entero): el ancho en puntos enteros del `DockArea` (`dock_width`) más lo que se pase
/// el contenido de un panel acoplado; si el rect del anfitrión no era de puntos enteros
/// (`whole` falso, un frame de transición), redondeado hacia arriba. Si al contenido le sobra
/// lugar no encoge: un panel angosto no achica la columna.
fn host_width(dock_width: f32, whole: bool, docked: impl Iterator<Item = Extent>) -> f32 {
    let width = dock_width + docked.map(|e| e.delta).fold(0.0, f32::max);
    if whole { width } else { width.ceil() }
}

/// Lo mínimo de cada panel en un grupo del workspace, con el margen de su cuerpo (ver
/// [`minima`]).
pub(crate) type Minima = HashMap<PanelId, egui::Vec2>;

/// Lo mínimo de cada panel en un grupo del workspace, con el margen de su cuerpo. Color, lo que
/// pide su panel con Pixel UI (el ancho de su fila más ancha y el alto del editor entero con la
/// paleta en su mínimo); sin Pixel UI, nada (como su panel de antes). Los demás, el `min_width`
/// y el `min_height` de su ficha con el margen de los paneles del dock y, si su cuerpo va con
/// scroll, la barra (así los acomoda [`Viewer`]).
pub(crate) fn minima(ui: &egui::Ui, skin: Option<&Skin>) -> Minima {
    let margin = 2.0 * f32::from(dock::FLYOUT_MARGIN);
    PanelId::ALL
        .iter()
        .map(|&id| {
            let min = match (id, skin) {
                (PanelId::Color, Some(skin)) => egui::vec2(
                    crate::color_panel::panel_min_size(ui, skin),
                    crate::color_panel::panel_min_height(ui, skin),
                ),
                (PanelId::Color, None) => egui::Vec2::ZERO,
                _ => {
                    let spec = id.spec();
                    let bar = match spec.body {
                        Body::Fit | Body::Tall => pixel_ui::SCROLL_BAR,
                        Body::Fixed => 0.0,
                    };
                    let height = if spec.min_height > 0.0 {
                        spec.min_height + margin
                    } else {
                        0.0
                    };
                    egui::vec2(spec.min_width + margin + bar, height)
                }
            };
            (id, min)
        })
        .collect()
}

/// Lo que ocupan un divisor y una barra de pestañas con el estilo de `show`.
fn chrome(ui: &egui::Ui, skin: Option<&Skin>) -> (f32, f32) {
    let style = match skin {
        Some(skin) => skin::dock_style(ui.style(), skin),
        None => skin::plain_style(ui.style()),
    };
    (style.separator.width, style.tab_bar.height)
}

/// Lo mínimo del nodo `node` de `tree`: un grupo, lo de su panel más exigente (y su barra de
/// pestañas, si la lleva); un split, sus dos lados uno junto al otro (o uno encima del otro) con
/// el divisor entre ellos.
fn node_min(tree: &Tree<PanelId>, node: NodeIndex, m: &Minima, sep: f32, bar: f32) -> egui::Vec2 {
    if node.0 >= tree.len() {
        return egui::Vec2::ZERO;
    }
    let sides = || {
        (
            node_min(tree, node.left(), m, sep, bar),
            node_min(tree, node.right(), m, sep, bar),
        )
    };
    match &tree[node] {
        Node::Empty => egui::Vec2::ZERO,
        Node::Leaf(leaf) => {
            let mut min = leaf
                .tabs
                .iter()
                .map(|t| m.get(t).copied().unwrap_or_default())
                .fold(egui::Vec2::ZERO, egui::Vec2::max);
            if !leaf.tab_bar_hidden && !leaf.tabs.is_empty() {
                min.y += bar;
            }
            min
        }
        Node::Horizontal(_) => {
            let (a, b) = sides();
            egui::vec2(a.x + b.x + sep, a.y.max(b.y))
        }
        Node::Vertical(_) => {
            let (a, b) = sides();
            egui::vec2(a.x.max(b.x), a.y + b.y + sep)
        }
    }
}

/// La fracción de un split que mide `len` (a lo ancho o a lo alto) para que cada lado tenga al
/// menos su mínimo (`a` y `b`, más medio divisor `sep`): la misma si ya alcanza; si no, la del
/// borde de ese mínimo. Si no alcanza para los dos, el lado que no se desplaza (`keep`: `Some(true)`
/// el primero, `Some(false)` el segundo) se queda con su mínimo y el otro, con lo que sobra (lo
/// suyo va con scroll); si los dos o ninguno se desplazan, en proporción a sus mínimos.
fn fit_fraction(len: f32, fraction: f32, a: f32, b: f32, sep: f32, keep: Option<bool>) -> f32 {
    if len <= 0.0 || !len.is_finite() {
        return fraction;
    }
    let lo = a + sep / 2.0;
    let hi = len - b - sep / 2.0;
    let mid = len * fraction;
    let fitted = if lo <= hi {
        if (lo..=hi).contains(&mid) {
            return fraction;
        }
        mid.clamp(lo, hi)
    } else if keep == Some(true) {
        lo.min(len)
    } else if keep == Some(false) {
        hi.max(0.0)
    } else if a + b > 0.0 {
        len * a / (a + b)
    } else {
        return fraction;
    };
    (fitted / len).clamp(0.0, 1.0)
}

/// Si todo lo del nodo `node` de `tree` se desplaza a lo alto (sus paneles van con scroll): lo
/// que no entre en su lugar se ve igual.
fn node_scrolls(tree: &Tree<PanelId>, node: NodeIndex) -> bool {
    if node.0 >= tree.len() {
        return true;
    }
    match &tree[node] {
        Node::Empty => true,
        Node::Leaf(leaf) => leaf.tabs.iter().all(|t| t.spec().body != Body::Fixed),
        Node::Horizontal(_) | Node::Vertical(_) => {
            node_scrolls(tree, node.left()) && node_scrolls(tree, node.right())
        }
    }
}

/// Cada split de cada superficie con sus dos lados en su mínimo, si cabe (ver
/// [`fit_fraction`]). Corre después del `DockArea` con los rects de este frame y rige en el
/// siguiente, como `skin::snap`.
fn fit_splits(state: &mut DockState<PanelId>, m: &Minima, sep: f32, bar: f32) {
    for surface in state.iter_surfaces_mut() {
        let Some(tree) = surface.node_tree_mut() else {
            continue;
        };
        let fixes: Vec<(NodeIndex, f32)> = (0..tree.len())
            .filter_map(|i| {
                let node = NodeIndex(i);
                let (split, horizontal) = match &tree[node] {
                    Node::Horizontal(s) => (s, true),
                    Node::Vertical(s) => (s, false),
                    _ => return None,
                };
                let a = node_min(tree, node.left(), m, sep, bar);
                let b = node_min(tree, node.right(), m, sep, bar);
                let (len, a, b) = if horizontal {
                    (split.rect.width(), a.x, b.x)
                } else {
                    (split.rect.height(), a.y, b.y)
                };
                // A lo alto, si no alcanza, se queda con su mínimo el lado que no se desplaza.
                let keep = if horizontal {
                    None
                } else {
                    match (
                        node_scrolls(tree, node.left()),
                        node_scrolls(tree, node.right()),
                    ) {
                        (false, true) => Some(true),
                        (true, false) => Some(false),
                        _ => None,
                    }
                };
                let f = fit_fraction(len, split.fraction, a, b, sep, keep);
                (f != split.fraction).then_some((node, f))
            })
            .collect();
        for (node, f) in fixes {
            if let Node::Horizontal(s) | Node::Vertical(s) = &mut tree[node] {
                s.fraction = f;
            }
        }
    }
}

/// El id del `DockArea` del workspace.
fn dock_id() -> egui::Id {
    egui::Id::new("workspace_dock")
}

/// El workspace en `ui`: el `DockArea` con la piel (con Pixel UI) y cada panel con
/// [`ChimpApp::draw_panel`]. Lo que piden los paneles se aplica al final, con la app entera;
/// cerrar una pestaña la regresa a la tira. Con un diálogo modal (`modal`), los paneles quedan
/// apagados y las pestañas no se arrastran.
///
/// Mientras los paneles se dibujan, `app.workspace` es un acomodo de relleno (el de verdad lo
/// tiene `show`): cambiarlo desde un panel no serviría, por eso sus operaciones lo revisan en
/// debug. Lo que haya que cambiar del acomodo se pide como acción y se hace después.
pub(crate) fn show(app: &mut ChimpApp, ui: &mut egui::Ui, modal: bool) {
    let mut workspace = std::mem::replace(&mut app.workspace, Workspace::placeholder());
    workspace.tidy();
    workspace.refresh_tab_bars();
    let shared_skin = app.pixel.shared_skin();
    let skin: Option<&Skin> = shared_skin.as_deref();
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
    let mut viewer = Viewer {
        app,
        modal,
        skinned: skin.is_some(),
        widths: &widths,
        color_margin: egui::Frame::side_top_panel(ui.style()).inner_margin,
        actions: Vec::new(),
        hits: Vec::new(),
        closed: Vec::new(),
        extents: Vec::new(),
        style: ui.style().clone(),
    };
    let area = ui.available_rect_before_wrap();
    // El `DockArea`, de un ancho de puntos enteros (egui_dock solo ajusta a píxel físico) y en un
    // `Ui` aparte (ocupa todo su rect): al anfitrión se le reporta el ancho de lo acoplado (ver
    // `host_width`).
    let dock_rect = egui::Rect::from_x_y_ranges(
        area.left().ceil()..=area.right().floor().max(area.left().ceil()),
        area.y_range(),
    );
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
    let Viewer {
        app,
        actions,
        hits,
        mut closed,
        extents,
        ..
    } = viewer;
    let docked = extents.into_iter().filter(|e| workspace.docked(e.id));
    let whole = area.left() == dock_rect.left() && area.right() == dock_rect.right();
    let width = host_width(dock_rect.width(), whole, docked);
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(
        area.min,
        egui::vec2(width, area.height()),
    ));
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
        closed.extend(close_marks(ui, &hits, modal));
    }
    // Dónde puede haber marcadores este frame: el DockArea y sus ventanas, fuera del cuerpo de los
    // grupos (ver `skin::Armed`).
    if let Some(skin) = &shared_skin {
        let mut armed = skin::Armed::new(skin.clone());
        armed.areas.push(area);
        for (s, _) in workspace.state.iter_surfaces_indexed() {
            if !s.is_main()
                && let Some(rect) = ui.ctx().memory(|m| m.area_rect(skin::window_area_id(s)))
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
        skin::arm(ui.ctx(), armed);
    }
    for id in closed {
        workspace.to_strip(id);
    }
    // Si un panel se soltó en otro grupo, la barra de algún grupo cambia: este frame ya se dibujó
    // con la de antes, así que se descarta y se vuelve a dibujar (sin el parpadeo de un frame).
    if workspace.refresh_tab_bars() {
        ui.ctx().request_discard("tab bar visibility changed");
    }
    // Cada split con sus lados en su mínimo, y a puntos enteros, para el frame siguiente.
    let (sep, bar) = chrome(ui, skin);
    fit_splits(&mut workspace.state, &minima(ui, skin), sep, bar);
    skin::snap(ui.ctx(), &mut workspace.state);
    skin::snap_windows(ui.ctx(), &mut workspace.state);
    app.workspace = workspace;
    for action in actions {
        app.apply_panel_action(ui.ctx(), action);
    }
}

/// La × del kit de cada pestaña que se puede cerrar, encima de la de egui_dock (en su capa):
/// devuelve los paneles cuya × se apretó.
fn close_marks(ui: &mut egui::Ui, hits: &[skin::TabHit], modal: bool) -> Vec<PanelId> {
    let mut closed = Vec::new();
    for h in hits {
        let item = skin::tab_item(h.id);
        if item.mark == kit::tabs::TabMark::None || skin::is_dragged_tab(&h.response) {
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

    use super::*;
    use crate::panel_registry::PanelId;

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
    fn essentials_docks_color_and_strips_the_rest() {
        let w = Workspace::essentials();
        assert!(w.docked(PanelId::Color));
        assert_eq!(w.strip(), PanelId::DOCK);
    }

    #[test]
    fn a_panel_lives_in_exactly_one_place() {
        let mut w = Workspace::essentials();
        w.dock(PanelId::Library, DockTarget::Focused);
        assert!(w.docked(PanelId::Library) && !w.strip().contains(&PanelId::Library));
        w.float(PanelId::Library, egui::pos2(100.0, 100.0));
        assert!(w.floating(PanelId::Library) && !w.docked(PanelId::Library));
        w.to_strip(PanelId::Library);
        assert!(!w.floating(PanelId::Library) && w.strip().contains(&PanelId::Library));
        // La tira conserva el orden de DOCK.
        assert_eq!(w.strip(), PanelId::DOCK);
    }

    #[test]
    fn one_tab_hides_the_tab_bar_and_two_show_it() {
        let mut w = Workspace::essentials();
        w.refresh_tab_bars();
        assert!(w.tab_bar_hidden_of(PanelId::Color));
        w.dock(PanelId::Library, DockTarget::Focused);
        w.refresh_tab_bars();
        assert!(!w.tab_bar_hidden_of(PanelId::Color));
    }

    #[test]
    fn collapsing_a_group_sends_its_tabs_to_the_strip() {
        let mut w = Workspace::essentials();
        w.dock(PanelId::Library, DockTarget::Focused);
        let (surface, node) = w.leaf_of(PanelId::Library).unwrap();
        w.collapse_leaf(surface, node);
        assert!(!w.docked(PanelId::Library) && !w.docked(PanelId::Color));
        assert!(w.strip().contains(&PanelId::Library) && w.strip().contains(&PanelId::Color));
    }

    /// Cuántas veces está cada panel: en la tira y en el árbol.
    fn places(w: &Workspace, id: PanelId) -> usize {
        let in_strip = w.strip().iter().filter(|&&p| p == id).count();
        let in_tree = w.state.iter_all_tabs().filter(|(_, p)| **p == id).count();
        in_strip + in_tree
    }

    #[test]
    fn docking_into_a_split_or_a_moved_group_keeps_every_panel_once() {
        let mut w = Workspace::essentials();
        let (s, color) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Groom, DockTarget::Split(s, color, Split::Below));
        assert!(w.docked(PanelId::Groom));
        assert_ne!(w.leaf_of(PanelId::Groom), w.leaf_of(PanelId::Color));
        // Groom como pestaña del grupo de Color: su grupo desaparece y los índices se mueven.
        let (s, color) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Groom, DockTarget::Leaf(s, color));
        assert_eq!(w.leaf_of(PanelId::Groom), w.leaf_of(PanelId::Color));
        // A su propio grupo, solo: se queda.
        w.float(PanelId::Lumen, egui::pos2(10.0, 10.0));
        let (s, n) = w.leaf_of(PanelId::Lumen).unwrap();
        w.dock(PanelId::Lumen, DockTarget::Leaf(s, n));
        assert!(w.floating(PanelId::Lumen));
        // A un grupo que no existe: al del foco.
        w.dock(
            PanelId::Slices,
            DockTarget::Leaf(SurfaceIndex::main(), NodeIndex(40)),
        );
        assert!(w.docked(PanelId::Slices));
        for &id in PanelId::ALL {
            let expected = usize::from(PanelId::DOCK.contains(&id) || id == PanelId::Color);
            assert_eq!(places(&w, id), expected, "{id:?}");
        }
    }

    #[test]
    fn the_demo_workspaces_place_their_panel() {
        let w = Workspace::demo("library-tab").unwrap();
        assert_eq!(w.leaf_of(PanelId::Library), w.leaf_of(PanelId::Color));
        let w = Workspace::demo("split").unwrap();
        assert!(w.docked(PanelId::Groom));
        assert_ne!(w.leaf_of(PanelId::Groom), w.leaf_of(PanelId::Color));
        let w = Workspace::demo("float").unwrap();
        assert!(w.floating(PanelId::Lumen));
        assert!(Workspace::demo("nope").is_none());
    }

    // --- `show` en un frame sin ventana ---

    use crate::palette_library::PaletteLibrary;
    use crate::palette_library::test_util::NoPresets;
    use crate::view::ViewSettings;

    /// La app con Pixel UI y una pestaña nueva.
    fn test_app() -> ChimpApp {
        let view = ViewSettings {
            pixel_ui: true,
            ..Default::default()
        };
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
        app.new_document();
        app
    }

    /// Un frame con el workspace en el panel central, con `events`.
    fn frame(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        frame_in(ctx, app, events, egui::vec2(1280.0, 800.0))
    }

    /// Como `frame`, en una ventana de `size`.
    fn frame_in(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        events: Vec<egui::Event>,
        size: egui::Vec2,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.pixel.update(ui.ctx(), &app.view);
            egui::CentralPanel::default().show(ui, |ui| show(app, ui, false));
        });
        output.textures_delta.clear();
        output
    }

    /// Unos frames (la escala de Pixel UI rige desde el segundo); la salida del último.
    fn frames(ctx: &egui::Context, app: &mut ChimpApp) -> egui::FullOutput {
        let mut out = frame(ctx, app, Vec::new());
        for _ in 0..3 {
            out = frame(ctx, app, Vec::new());
        }
        out
    }

    /// El botón con la etiqueta `label` en `output`.
    fn button(output: &egui::FullOutput, label: &str) -> Option<egui::accesskit::NodeId> {
        let update = output.platform_output.accesskit_update.as_ref()?;
        update
            .nodes
            .iter()
            .find(|(_, n)| n.role() == egui::accesskit::Role::Button && n.label() == Some(label))
            .map(|(id, _)| *id)
    }

    fn click(target: egui::accesskit::NodeId) -> egui::Event {
        egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        })
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

    /// El mínimo de `id` en un grupo, como lo cuenta el workspace (con Pixel UI).
    fn min_of(ctx: &egui::Context, app: &ChimpApp, id: PanelId) -> egui::Vec2 {
        let mut out = egui::Vec2::ZERO;
        let input = egui::RawInput::default();
        let mut output = ctx.run_ui(input, |ui| {
            out = minima(ui, app.pixel.skin())[&id];
        });
        output.textures_delta.clear();
        out
    }

    #[test]
    fn splits_give_each_group_at_least_its_minimum() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        // Groom debajo de Color, a la mitad: en 800 de alto a Groom no le toca su mínimo.
        app.workspace = Workspace::demo("split").unwrap();
        frames(&ctx, &mut app);
        let groom = app.workspace.leaf_rect(PanelId::Groom).unwrap();
        let color = app.workspace.leaf_rect(PanelId::Color).unwrap();
        assert!(
            groom.height() >= min_of(&ctx, &app, PanelId::Groom).y,
            "{groom:?}"
        );
        assert!(
            color.height() >= min_of(&ctx, &app, PanelId::Color).y,
            "{color:?}"
        );
        // En una ventana baja no alcanza para los dos: Color, que no se desplaza, se queda con su
        // mínimo; Groom, con lo que sobra (y su scroll).
        app.workspace = Workspace::demo("split").unwrap();
        for _ in 0..6 {
            frame_in(&ctx, &mut app, Vec::new(), egui::vec2(960.0, 450.0));
        }
        let color = app.workspace.leaf_rect(PanelId::Color).unwrap();
        assert!(
            color.height() >= min_of(&ctx, &app, PanelId::Color).y,
            "{color:?}"
        );
        // Library a la derecha de Color con casi todo para Color: Library llega a su mínimo.
        let mut w = Workspace::essentials();
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Library, DockTarget::Split(s, n, Split::Right));
        if let Node::Horizontal(split) = &mut w.state.main_surface_mut()[NodeIndex::root()] {
            split.fraction = 0.95;
        }
        app.workspace = w;
        frames(&ctx, &mut app);
        let library = app.workspace.leaf_rect(PanelId::Library).unwrap();
        assert!(
            library.width() >= min_of(&ctx, &app, PanelId::Library).x,
            "{library:?}"
        );
    }

    #[test]
    fn a_split_fraction_is_moved_only_as_far_as_the_minima_need() {
        // Cabe: se queda; se pasa de un lado: al borde de ese mínimo (con medio divisor).
        assert_eq!(fit_fraction(400.0, 0.5, 100.0, 100.0, 2.0, None), 0.5);
        assert_eq!(
            fit_fraction(400.0, 0.1, 100.0, 100.0, 2.0, None),
            101.0 / 400.0
        );
        assert_eq!(
            fit_fraction(400.0, 0.9, 100.0, 100.0, 2.0, None),
            299.0 / 400.0
        );
        // No alcanza para los dos: en proporción a sus mínimos; o el que no se desplaza se queda
        // con el suyo (el otro, con scroll).
        assert_eq!(fit_fraction(100.0, 0.9, 100.0, 300.0, 0.0, None), 0.25);
        assert_eq!(
            fit_fraction(200.0, 0.9, 100.0, 300.0, 2.0, Some(true)),
            101.0 / 200.0
        );
        assert_eq!(
            fit_fraction(200.0, 0.1, 150.0, 100.0, 2.0, Some(false)),
            99.0 / 200.0
        );
        // Sin tamaño todavía: nada.
        assert_eq!(fit_fraction(0.0, 0.7, 100.0, 100.0, 2.0, None), 0.7);
    }

    #[test]
    fn floating_windows_stay_on_the_screen() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        // La ventana baja de las capturas (504 de alto con la escala 2), y una más chica que
        // Lumen; Lumen en 320, 120.
        for size in [egui::vec2(960.0, 504.0), egui::vec2(400.0, 300.0)] {
            app.workspace = Workspace::demo("float").unwrap();
            for _ in 0..6 {
                frame_in(&ctx, &mut app, Vec::new(), size);
            }
            let (surface, _) = app.workspace.leaf_of(PanelId::Lumen).unwrap();
            let rect = ctx
                .memory(|m| m.area_rect(skin::window_area_id(surface)))
                .expect("Lumen's window");
            let screen = ctx.content_rect();
            assert!(screen.contains_rect(rect), "{rect:?} in {screen:?}");
            assert_eq!(rect.min, rect.min.round());
        }
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
}
