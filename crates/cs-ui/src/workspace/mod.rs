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
//! Se parte por responsabilidad: [`model`] (las operaciones sobre el acomodo, sus invariantes y
//! lo que se guarda), [`drop`] (dónde cae un panel que se suelta), [`fit`] (los mínimos y su
//! ajuste a los splits y a las ventanas) y [`show`] (el `DockArea` y lo que lo rodea).
//!
//! El marco de las ventanas flotantes se viste con un `ui.scope` alrededor del `DockArea` cuyo
//! estilo lleva el color marcador (`skin::window_visuals`): funciona porque egui_dock 0.21.1 arma
//! el marco con `Frame::window(ui.style())` del `Ui` que se le pasa, mientras que el contenido de
//! cada pestaña usa el estilo del contexto. Si eso cambia al subir de versión, hay que revisarlo.

mod drop;
mod fit;
mod model;
mod show;
mod skin;
#[cfg(test)]
mod test_util;

use eframe::egui;
use egui_dock::DockState;
pub(crate) use egui_dock::{NodeIndex, Split, SurfaceIndex};

use crate::panel_registry::PanelId;

pub(crate) use drop::{drop_target, paint_drop_zone};
pub(crate) use model::DockTarget;
pub(crate) use show::show;
#[cfg(test)]
pub(crate) use show::{COLLAPSE, MODEL_ELSEWHERE, has_marker};

/// El acomodo de los paneles: el árbol de egui_dock (acoplados y flotantes) y la tira de íconos.
#[derive(Clone, Debug)]
pub(crate) struct Workspace {
    state: DockState<PanelId>,
    /// Los paneles en la tira, en el orden de `PanelId::ALL`.
    strip: Vec<PanelId>,
    /// Es el que queda en `app.workspace` mientras [`show`] tiene prestado el de verdad (los
    /// paneles se dibujan con la app entera): cambiarlo no serviría de nada.
    lent: bool,
    /// Las ventanas de un acomodo leído y el rect que tenía lo de adentro de cada una (ver
    /// [`Workspace::place_restored_windows`]). Se olvidan con cualquier cambio del acomodo (los
    /// índices de las superficies se mueven).
    placements: Vec<(SurfaceIndex, egui::Rect)>,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace::essentials()
    }
}

impl Workspace {
    /// Cuántas veces está `id` en el acomodo: en la tira y en el árbol (para los tests: una).
    #[cfg(test)]
    pub(crate) fn places(&self, id: PanelId) -> usize {
        let in_strip = self.strip.iter().filter(|&&p| p == id).count();
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

    /// Los paneles de la tira, en orden.
    pub(crate) fn strip(&self) -> &[PanelId] {
        &self.strip
    }

    /// Si `id` está acoplado (en la superficie principal).
    pub(crate) fn docked(&self, id: PanelId) -> bool {
        self.state.find_main_surface_tab(&id).is_some()
    }

    /// Si `id` está flotando (en una ventana).
    #[cfg_attr(not(test), expect(dead_code, reason = "solo lo leen los tests"))]
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
}

/// El id del `DockArea` del workspace.
fn dock_id() -> egui::Id {
    egui::Id::new("workspace_dock")
}
