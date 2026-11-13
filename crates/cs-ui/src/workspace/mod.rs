//! El workspace: dónde está cada panel. Cada uno vive en un solo lugar: en la tira de íconos (el
//! dock de hoy), acoplado en la superficie principal (un grupo de pestañas en el árbol de
//! egui_dock) o flotando en una ventana. [`show()`] muestra el `DockArea` con la piel pixel art
//! (ver `skin`) y hospeda cada panel con [`ChimpApp::draw_panel`](crate::ChimpApp::draw_panel).
//!
//! Fase D (diseño aprobado el 2026-11-11, después del spike `proto_workspace_d`): **un solo árbol
//! para toda el área central** (entre la barra de opciones y la barra de estado; la toolbar va en
//! él), con el **lienzo como hoja protegida** (`PanelId::Canvas`: no se cierra, no se arrastra, no
//! recibe pestañas y está siempre una vez, en la superficie principal; ver `fit::tidy_canvas`). Lo
//! que lo rodea va en puntos (`fit::pin_sides`): al cambiar la ventana, el lienzo absorbe la
//! diferencia. La columna derecha de la fase C es ahora el lado derecho del lienzo
//! ([`Workspace::column`]); su ancho sigue siendo la preferencia del usuario
//! (`view.palette_width`). La toolbar y el timeline son hojas del árbol (tareas 3 y 4), y el
//! Preview también (tarea 5: de fábrica flota en la esquina de abajo a la derecha, como su ventana
//! de antes, y su ventana es la de antes: con Pixel UI la del diseño, sin él la `egui::Window`).
//! Ocultos (el timeline y el Preview se prenden y se apagan), salen del árbol y vuelven a su lugar
//! ([`Workspace::sync_panel`]). Hay una tira de íconos por orilla (tarea 6,
//! [`Workspace::strip_at`]): la de la derecha va a la derecha del lienzo (y del timeline, si va
//! justo debajo de él), encima de su hoja; las otras tres, en su orilla, fuera del árbol (vacías no
//! ocupan nada). La dibuja `show` (ver `show::show`); « de un grupo manda sus paneles a la de su
//! orilla y cerrar una pestaña, a la de la orilla más cercana ([`Workspace::group_edge`]). Cada
//! panel se dibuja en un solo anfitrión por frame (ver `ChimpApp::claim_panel`).
//!
//! Decisión del usuario (2026-11-10): un grupo acoplado con **un solo panel no lleva barra de
//! pestañas** (se ve como hoy); al pasar el puntero por su borde de arriba se resalta una franja
//! delgada para agarrarlo. Con dos o más, la barra con las pestañas del kit. Las ventanas flotantes
//! siempre llevan su barra: es su título y de ahí se arrastran. Salvo una: el Preview solo en su
//! ventana lleva la suya de antes (con su barra de título: de ahí se mueve y, con Pixel UI, de su
//! franja de arriba se lleva a acoplar, como un grupo solo; ver `preview_window::PreviewWindow`).
//!
//! Solo este módulo usa egui_dock.
//!
//! Se parte por responsabilidad: [`model`] (las operaciones sobre el acomodo, sus invariantes y lo
//! que se guarda), [`drop`](mod@drop) (dónde cae un panel que se suelta), [`fit`] (los mínimos y su
//! ajuste a los splits y a las ventanas), [`show`](mod@show) (el `DockArea` y lo que lo rodea),
//! lo que `show` pega encima de egui_dock: [`tabs`] (las pestañas del kit, sus flechas y el «),
//! [`dividers`] (los divisores tapados y sus cruces), [`preview_window`] (la ventana de antes del
//! Preview) y [`modal`] (lo que no se toca con un diálogo), y [`presets`] (los workspaces con
//! nombre de Window → Workspace: los de fábrica y los del usuario).
//!
//! El marco de las ventanas flotantes se viste con un `ui.scope` alrededor del `DockArea` cuyo
//! estilo lleva el color marcador (`skin::window_visuals`): funciona porque egui_dock 0.21.1 arma
//! el marco con `Frame::window(ui.style())` del `Ui` que se le pasa, mientras que el contenido de
//! cada pestaña usa el estilo del contexto. Si eso cambia al subir de versión, hay que revisarlo.

mod dividers;
mod drop;
mod fit;
mod modal;
mod model;
mod presets;
mod preview_window;
mod show;
mod skin;
mod tabs;
#[cfg(test)]
mod test_util;

use eframe::egui;
use egui_dock::DockState;
pub(crate) use egui_dock::{NodeIndex, Split, SurfaceIndex};

use crate::dock::Edge;
use crate::panel_registry::PanelId;

#[cfg(test)]
pub(crate) use drop::EDGE_BAND;
pub(crate) use drop::{drop_target, paint_drop_zone, paint_strip_zone, strip_under};
pub(crate) use model::{DockTarget, SAVED_VERSION, SavedWorkspace};
pub(crate) use presets::{FACTORY, Shown, Workspaces};
pub(crate) use show::show;
#[cfg(test)]
pub(crate) use show::{MODEL_ELSEWHERE, has_marker};
#[cfg(test)]
pub(crate) use skin::painted_drop_zones;
#[cfg(test)]
pub(crate) use tabs::COLLAPSE;

/// Lo mínimo de `id` en un grupo, como lo cuenta el workspace (ver `fit::minima`; sin lo que le
/// cambia `show` según dónde va: el timeline debajo del lienzo, lo pegado a la columna).
#[cfg(test)]
pub(crate) fn panel_minimum(
    ui: &egui::Ui,
    skin: Option<&crate::skin::Skin>,
    id: PanelId,
) -> egui::Vec2 {
    fit::minima(ui, skin)[&id]
}

/// El acomodo de los paneles: el árbol de egui_dock (acoplados y flotantes) y la tira de íconos.
#[derive(Clone, Debug)]
pub(crate) struct Workspace {
    state: DockState<PanelId>,
    /// Los paneles de la tira de íconos de cada orilla (por `Edge::index`; la de la derecha es la
    /// de siempre), cada una en el orden de `PanelId::ALL`.
    strips: [Vec<PanelId>; 4],
    /// Con Pixel UI (lo pone `show` cada frame): la toolbar sola en su ventana va sin barra de
    /// pestañas, con la franja de agarre (ver [`Workspace::refresh_tab_bars`]). Sin él no hay
    /// franja, así que lleva su pestaña.
    pixel: bool,
    /// Es el que queda en `app.workspace` mientras [`show()`] tiene prestado el de verdad (los
    /// paneles se dibujan con la app entera): cambiarlo no serviría de nada.
    lent: bool,
    /// Las ventanas de un acomodo leído y el rect que tenía lo de adentro de cada una (ver
    /// [`Workspace::place_restored_windows`]). Se olvidan con cualquier cambio del acomodo (los
    /// índices de las superficies se mueven).
    placements: Vec<(SurfaceIndex, egui::Rect)>,
    /// Las ventanas recién abiertas: su panel, la esquina pedida y cuántos frames más pedírsela
    /// (ver [`Workspace::place_new_windows`]).
    placing: Vec<(PanelId, egui::Pos2, u8)>,
    /// Lo que mide en puntos cada lado del lienzo (ver `fit::pin_sides`).
    sides: fit::SidePoints,
    /// Dónde estaba cada panel oculto (el timeline, el Preview; ver [`Workspace::sync_panel`]).
    spots: Vec<(PanelId, model::Spot)>,
    /// Si el Preview flota en su esquina (la de su ventana de antes: ver
    /// `Workspace::place_preview`): ahí lo pone [`Workspace::sync_panel`] si no tiene otro lugar, y
    /// ahí sigue (también al cambiar el tamaño de la app) hasta que se lo mueve. Anclado, no se
    /// guarda: al leer el acomodo vuelve a su esquina.
    preview_anchored: bool,
    /// Un panel que Window pidió mostrar (el timeline o el Preview apagados): `show` lo enfoca y
    /// sube su ventana ([`Workspace::bring_forward`]) en cuanto vuelve al árbol, en el mismo frame.
    pub(crate) pending_focus: Option<PanelId>,
    /// Lo dibujado en el último frame que dice dónde cae lo que se suelta en una orilla (el área
    /// del workspace, la del árbol y las tiras; ver `drop::Bounds`). Lo pone `show`.
    bounds: Option<drop::Bounds>,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace::essentials()
    }
}

impl Workspace {
    /// Cuántas veces está `id` en el acomodo: en las tiras y en el árbol (para los tests: una).
    #[cfg(test)]
    pub(crate) fn places(&self, id: PanelId) -> usize {
        let in_strip = self.strips.iter().flatten().filter(|&&p| p == id).count();
        let in_tree = self
            .state
            .iter_all_tabs()
            .filter(|(_, p)| **p == id)
            .count();
        in_strip + in_tree
    }

    /// Todos los paneles del árbol (acoplados y flotantes), con repetidos si los hubiera.
    #[cfg(test)]
    pub(crate) fn state_tabs(&self) -> Vec<PanelId> {
        self.state.iter_all_tabs().map(|(_, p)| *p).collect()
    }

    /// Los paneles de la tira de la derecha (la de siempre), en orden.
    #[cfg(test)]
    pub(crate) fn strip(&self) -> &[PanelId] {
        self.strip_at(Edge::Right)
    }

    /// Los paneles de la tira de la orilla `edge`, en orden.
    pub(crate) fn strip_at(&self, edge: Edge) -> &[PanelId] {
        &self.strips[edge.index()]
    }

    /// En qué tira está `id`, si está en una.
    pub(crate) fn strip_of(&self, id: PanelId) -> Option<Edge> {
        Edge::ALL
            .into_iter()
            .find(|e| self.strips[e.index()].contains(&id))
    }

    /// Si `id` está acoplado (en la superficie principal: en cualquier lado del lienzo).
    pub(crate) fn docked(&self, id: PanelId) -> bool {
        self.state.find_main_surface_tab(&id).is_some()
    }

    /// Si `id` está flotando (en una ventana).
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
    pub(crate) fn leaf_rect(&self, id: PanelId) -> Option<egui::Rect> {
        let path = self.state.find_tab(&id)?;
        self.state.leaf(path.node_path()).ok().map(|leaf| leaf.rect)
    }

    /// El rect de la ventana flotante de `id` (el de su área de egui, con su marco), si flota.
    #[cfg(test)]
    pub(crate) fn window_rect(&self, ctx: &egui::Context, id: PanelId) -> Option<egui::Rect> {
        let path = self.state.find_tab(&id)?;
        if path.surface.is_main() {
            return None;
        }
        ctx.memory(|m| m.area_rect(skin::window_area_id(path.surface)))
    }

    /// Los paneles del grupo `(surface, node)` y el que está a la vista, si es un grupo.
    #[cfg(test)]
    pub(crate) fn state_leaf_tabs(
        &self,
        surface: SurfaceIndex,
        node: NodeIndex,
    ) -> Option<(Vec<PanelId>, PanelId)> {
        let leaf = self
            .state
            .leaf(egui_dock::NodePath { surface, node })
            .ok()?;
        Some((leaf.tabs.clone(), *leaf.tabs.get(leaf.active.0)?))
    }

    /// La capa de egui de la ventana flotante `surface`.
    #[cfg(test)]
    pub(crate) fn window_layer(&self, surface: SurfaceIndex) -> egui::LayerId {
        egui::LayerId::new(egui::Order::Middle, skin::window_area_id(surface))
    }

    /// La forma de la superficie principal: cada nodo (su clase y sus paneles), sin rects ni
    /// fracciones (dos acomodos con la misma son el mismo árbol).
    #[cfg(test)]
    pub(crate) fn main_shape(&self) -> fit::Shape {
        fit::shape(self.state.main_surface())
    }

    /// Si el grupo de `id` va sin barra de pestañas.
    #[cfg(test)]
    pub(crate) fn tab_bar_hidden_of(&self, id: PanelId) -> bool {
        self.state
            .find_tab(&id)
            .and_then(|p| self.state.leaf(p.node_path()).ok())
            .is_some_and(|leaf| leaf.tab_bar_hidden)
    }
}

/// El id del `DockArea` del workspace.
fn dock_id() -> egui::Id {
    egui::Id::new("workspace_dock")
}
