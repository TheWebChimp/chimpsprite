//! Las operaciones sobre el acomodo (acoplar, flotar, mandar a la tira, colapsar, enfocar), sus
//! invariantes (cada panel en un solo lugar, un árbol que egui_dock pueda dibujar) y lo que se
//! guarda y se lee de la configuración ([`SavedWorkspace`]), además de los acomodos de
//! demostración.

use eframe::egui;
use egui_dock::{DockState, Node, NodePath, Surface, TabIndex, Tree, WindowState};

use super::{NodeIndex, Split, SurfaceIndex, Workspace};
use crate::panel_registry::PanelId;

/// El alto de fábrica de un panel que se pone a flotar (si su ficha no pide más).
pub(super) const FLOAT_HEIGHT: f32 = 300.0;

/// Dónde acoplar un panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DockTarget {
    /// Como pestaña del grupo `(superficie, nodo)`.
    Leaf(SurfaceIndex, NodeIndex),
    /// Partiendo el grupo `(superficie, nodo)`: el panel va del lado `Split`.
    Split(SurfaceIndex, NodeIndex, Split),
    /// En el grupo con el foco de la superficie principal (o el primero que haya), nunca en el
    /// del lienzo (ver [`Workspace::push_focused`]).
    Focused,
}

/// El acomodo como se guarda en la configuración de la app (`chimpsprite.workspace`): cada panel
/// por su llave estable, para que lo de otra versión o de otra plataforma se lea igual (lo que no
/// se conozca se descarta al leerlo: ver [`Workspace::from_saved`]). El ancho de la columna derecha
/// no va aquí: es `ViewSettings::palette_width`; los demás lados del lienzo salen de las
/// fracciones (ver `fit::pin_sides`).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct SavedWorkspace {
    /// La versión de lo guardado, para migrarlo (ver [`SAVED_VERSION`]). Sin el campo, 0: el de
    /// antes de que existiera (el mismo formato que la versión 1).
    #[serde(default)]
    pub version: u32,
    /// El árbol de egui_dock: lo acoplado y las ventanas flotantes.
    pub state: DockState<String>,
    /// Los paneles de la tira, en orden. Al leer no hace falta (la tira es lo del dock que no está
    /// en el árbol), pero así el archivo se entiende solo.
    pub strip: Vec<String>,
}

/// La versión de lo que se guarda ahora: 2, el árbol de toda la ventana con el lienzo (fase D).
/// La 1 (y la 0, de antes del campo) es la de la fase C: la columna derecha sola, sin lienzo; al
/// leerla se le mete el lienzo a la izquierda ([`Workspace::from_saved`]).
pub(crate) const SAVED_VERSION: u32 = 2;

impl Default for SavedWorkspace {
    fn default() -> Self {
        Workspace::essentials().to_saved()
    }
}

impl Workspace {
    /// El acomodo de fábrica: el lienzo y, a su derecha, Color acoplado solo (`H[Canvas |
    /// Color]`, la columna derecha con el ancho de `view.palette_width`), y los paneles del dock
    /// en la tira, en su orden. El foco, en el grupo de Color (ahí se acopla lo de la tira).
    pub(crate) fn essentials() -> Workspace {
        let mut state = DockState::new(vec![PanelId::Canvas]);
        let [_, color] =
            state
                .main_surface_mut()
                .split_right(NodeIndex::root(), 0.5, vec![PanelId::Color]);
        state.set_focused_node_and_surface(NodePath {
            surface: SurfaceIndex::main(),
            node: color,
        });
        Workspace {
            state,
            strip: PanelId::DOCK.to_vec(),
            lent: false,
            placements: Vec::new(),
            placing: Vec::new(),
            sides: Default::default(),
        }
    }

    /// Lo que se guarda del acomodo. Las ventanas flotantes llevan en su árbol el rect de lo de
    /// adentro (el de su grupo de arriba): de ahí sale dónde reabrirlas (ver
    /// [`Workspace::place_restored_windows`]).
    pub(crate) fn to_saved(&self) -> SavedWorkspace {
        SavedWorkspace {
            version: SAVED_VERSION,
            state: self.state.map_tabs(|p| p.key().to_owned()),
            strip: self.strip.iter().map(|p| p.key().to_owned()).collect(),
        }
    }

    /// El rect del nodo de arriba de la superficie `surface` (lo de adentro de una ventana) en el
    /// último frame, si tiene uno.
    pub(super) fn root_rect(&self, surface: SurfaceIndex) -> Option<egui::Rect> {
        self.state
            .iter_surfaces_indexed()
            .find(|(s, _)| *s == surface)
            .and_then(|(_, s)| s.node_tree())
            .filter(|tree| !tree.is_empty())
            .and_then(|tree| tree[NodeIndex::root()].rect())
            .filter(|r| r.is_finite() && r.is_positive())
    }

    /// El acomodo guardado, sin confiar en él (puede ser de otra versión, de otra plataforma o
    /// estar roto): un árbol mal armado da [`Workspace::essentials`] (con un aviso en el log); de
    /// lo demás se descartan los paneles que no se conocen (en la web, dafont), los que no van en
    /// el árbol y los repetidos. Los del dock que no quedan en el árbol van a la tira, y Color, si
    /// no quedó en ningún lado, a la superficie principal. Sin nada en el árbol, también
    /// Essentials.
    ///
    /// Lo de la fase C (versión 1, o 0 sin el campo) es la columna derecha sola: se le mete el
    /// lienzo a la izquierda (su ancho es `view.palette_width`, que no cambia). Y el lienzo queda
    /// una vez, solo y en la superficie principal (ver `fit::tidy_canvas`).
    pub(crate) fn from_saved(saved: SavedWorkspace) -> Workspace {
        let essentials = |why: &str| {
            log::warn!("discarding the saved workspace: {why}");
            Workspace::essentials()
        };
        if !sound(&saved.state) {
            return essentials("its dock tree is malformed");
        }
        let in_tree = |id: PanelId| {
            matches!(id, PanelId::Canvas | PanelId::Color) || Workspace::goes_to_strip(id)
        };
        let mut state: DockState<Option<PanelId>> = saved
            .state
            .map_tabs(|k| PanelId::from_key(k).filter(|&id| in_tree(id)));
        // Los que sobran se quitan uno por uno con `remove_tab` de egui_dock, que acomoda bien lo
        // que queda (`filter_map_tabs` deja huecos si el hermano de lo que quita es un split).
        loop {
            let mut seen = Vec::new();
            let extra = state.iter_all_tabs().find_map(|(path, tab)| match tab {
                Some(id) if !seen.contains(id) => {
                    seen.push(*id);
                    None
                }
                _ => Some(path),
            });
            match extra {
                Some(path) => {
                    state.remove_tab(path);
                }
                None => break,
            }
        }
        let mut state = state.filter_map_tabs(|tab| *tab);
        if state.iter_all_tabs().next().is_none() {
            return essentials("no known panel in it");
        }
        // La fase C: la columna sola. El lienzo, a su izquierda (la fracción la pone `pin_sides`).
        if saved.version < 2 && state.find_tab(&PanelId::Canvas).is_none() {
            let main = state.main_surface_mut();
            if main.num_tabs() == 0 {
                *main = Tree::new(vec![PanelId::Canvas]);
            } else {
                main.split_left(NodeIndex::root(), 0.5, vec![PanelId::Canvas]);
            }
        }
        super::fit::tidy_canvas(&mut state, None);
        // Lo que egui_dock da por bueno al dibujar: la pestaña activa y el grupo con el foco de
        // cada árbol, existentes, y números finitos (RON lee `NaN` e `inf`: con una fracción así
        // los rects salen NaN, egui se queja en debug y en release nunca se arregla). Los rects
        // los vuelve a calcular cada frame; `scroll` y las fracciones no.
        for (_, leaf) in state.iter_leaves_mut() {
            if leaf.active.0 >= leaf.tabs.len() {
                leaf.active = TabIndex(0);
            }
            if !leaf.scroll.is_finite() {
                leaf.scroll = 0.0;
            }
        }
        for surface in state.iter_surfaces_mut() {
            for node in surface.iter_nodes_mut() {
                if let Node::Horizontal(split) | Node::Vertical(split) = node {
                    split.fraction = if split.fraction.is_finite() {
                        split.fraction.clamp(0.0, 1.0)
                    } else {
                        0.5
                    };
                }
            }
            // Lo guardado de la ventana (la posición y el tamaño pedidos) no se puede revisar
            // desde aquí: se descarta y la ventana se acomoda con `placements`.
            if let Surface::Window(_, window) = surface {
                *window = WindowState::default();
            }
        }
        for surface in state.iter_surfaces_mut() {
            if let Some(tree) = surface.node_tree_mut() {
                let focused = tree
                    .focused_leaf()
                    .is_some_and(|n| n.0 < tree.len() && tree[n].is_leaf());
                if !focused {
                    let first = first_leaf(tree).unwrap_or(NodeIndex(tree.len()));
                    tree.set_focused_node(first);
                }
            }
        }
        if state.find_tab(&PanelId::Color).is_none() {
            push_beside_canvas(&mut state, PanelId::Color);
        }
        // El foco, en el grupo de Color (o el primero que no sea el lienzo) de la superficie
        // principal.
        let main = state.main_surface();
        let node = main
            .find_tab(&PanelId::Color)
            .map(|(node, _)| node)
            .or_else(|| first_panel_leaf(main))
            .or_else(|| first_leaf(main))
            .unwrap_or(NodeIndex(main.len()));
        state.set_focused_node_and_surface(NodePath {
            surface: SurfaceIndex::main(),
            node,
        });
        if !sound(&state) {
            return essentials("its dock tree is malformed");
        }
        let mut w = Workspace {
            state,
            ..Workspace::essentials()
        };
        // Lo guardado de cada ventana: el rect de lo de adentro.
        w.placements = w
            .state
            .iter_surfaces_indexed()
            .filter(|(s, _)| !s.is_main())
            .filter_map(|(s, _)| w.root_rect(s).map(|r| (s, r)))
            .collect();
        w.strip = PanelId::DOCK
            .iter()
            .copied()
            .filter(|p| w.state.find_tab(p).is_none())
            .collect();
        w
    }

    /// Lo que queda en `app.workspace` mientras [`show`] dibuja los paneles.
    pub(super) fn placeholder() -> Workspace {
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

    /// Saca `id` de donde esté (la tira o el árbol). Un grupo o una ventana que se vacían
    /// desaparecen. El lienzo no se mueve.
    fn take(&mut self, id: PanelId) {
        self.check_not_lent();
        if id == PanelId::Canvas {
            return;
        }
        self.placements.clear();
        self.strip.retain(|&p| p != id);
        if let Some(path) = self.state.find_tab(&id) {
            self.state.remove_tab(path);
        }
    }

    /// Acopla `id` en `into`. Si `into` no es un grupo que exista, en el del foco. El lienzo no
    /// se mueve ni recibe pestañas: lo que se acoplaría como pestaña suya va a su derecha.
    pub(crate) fn dock(&mut self, id: PanelId, into: DockTarget) {
        if id == PanelId::Canvas {
            return;
        }
        let into = match into {
            DockTarget::Leaf(surface, node) if self.is_canvas_leaf(surface, node) => {
                DockTarget::Split(surface, node, Split::Right)
            }
            other => other,
        };
        // El grupo de destino se recuerda por otra de sus pestañas: sacar `id` de su lugar puede
        // mover los índices del árbol.
        self.placements.clear();
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

    /// Si `(surface, node)` es el grupo del lienzo.
    fn is_canvas_leaf(&self, surface: SurfaceIndex, node: NodeIndex) -> bool {
        self.state
            .leaf(NodePath { surface, node })
            .is_ok_and(|leaf| leaf.tabs.contains(&PanelId::Canvas))
    }

    /// `id` al grupo con el foco de la superficie principal; si el foco es del lienzo (un clic en
    /// él se lo da) o de nadie, al primer grupo que no sea el lienzo; sin ninguno, a la derecha
    /// del lienzo.
    fn push_focused(&mut self, id: PanelId) {
        if let Some(path) = self.state.focused_leaf()
            && path.surface.is_main()
            && !self.is_canvas_leaf(path.surface, path.node)
            && let Ok(leaf) = self.state.leaf_mut(path)
        {
            leaf.append_tab(id);
            return;
        }
        push_beside_canvas(&mut self.state, id);
    }

    /// Pone `id` a flotar solo en una ventana nueva con la esquina en `at` (el lienzo no flota).
    pub(crate) fn float(&mut self, id: PanelId, at: egui::Pos2) {
        if id == PanelId::Canvas {
            return;
        }
        self.take(id);
        let spec = id.spec();
        let size = egui::vec2(spec.reference_width, spec.min_height.max(FLOAT_HEIGHT));
        let surface = self.state.add_window(vec![id]);
        if let Some(window) = self.state.get_window_state_mut(surface) {
            window.set_position(at.round());
            window.set_size(size.round());
        }
        self.placing.retain(|(p, _, _)| *p != id);
        self.placing
            .push((id, at.round(), super::fit::NEW_WINDOW_FRAMES));
    }

    /// Regresa `id` a la tira, en su lugar (el orden de `PanelId::ALL`).
    #[allow(
        clippy::wrong_self_convention,
        reason = "no es una conversión: manda el panel a la tira"
    )]
    pub(crate) fn to_strip(&mut self, id: PanelId) {
        if id == PanelId::Canvas {
            return;
        }
        self.take(id);
        let rank = |p: &PanelId| PanelId::ALL.iter().position(|q| q == p);
        let at = self.strip.partition_point(|p| rank(p) < rank(&id));
        self.strip.insert(at, id);
    }

    /// Los paneles del grupo `(surface, node)` (ninguno si no existe).
    fn tabs_of(&self, surface: SurfaceIndex, node: NodeIndex) -> Vec<PanelId> {
        self.state
            .leaf(NodePath { surface, node })
            .map(|leaf| leaf.tabs.clone())
            .unwrap_or_default()
    }

    /// Si `id` puede ir a la tira: los del dock, que tienen un flyout que su botón abre. Color no
    /// (su botón no abriría nada): se queda acoplado o flotando.
    pub(crate) fn goes_to_strip(id: PanelId) -> bool {
        PanelId::DOCK.contains(&id)
    }

    /// Si el botón « del grupo `(surface, node)` mandaría algo a la tira.
    pub(crate) fn collapsible(&self, surface: SurfaceIndex, node: NodeIndex) -> bool {
        self.tabs_of(surface, node)
            .into_iter()
            .any(Workspace::goes_to_strip)
    }

    /// El botón « ("Collapse to Icons") del grupo `(surface, node)`: sus paneles a la tira, salvo
    /// los que no van ahí (Color, que se queda en el grupo).
    pub(crate) fn collapse_to_icons(&mut self, surface: SurfaceIndex, node: NodeIndex) {
        for id in self.tabs_of(surface, node) {
            if Workspace::goes_to_strip(id) {
                self.to_strip(id);
            }
        }
    }

    /// Pone `id` (acoplado o flotando) como la pestaña activa de su grupo y le da el foco.
    /// Devuelve si estaba en el árbol.
    pub(crate) fn focus(&mut self, id: PanelId) -> bool {
        self.check_not_lent();
        let Some(path) = self.state.find_tab(&id) else {
            return false;
        };
        // Los dos solo fallan si el camino no existe, y lo acaba de dar `find_tab`.
        let _ = self.state.set_active_tab(path);
        self.state.set_focused_node_and_surface(path.node_path());
        true
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
    pub(super) fn tidy(&mut self) {
        let state = &self.state;
        let mut seen = Vec::new();
        self.strip.retain(|&p| {
            let keep = state.find_tab(&p).is_none() && !seen.contains(&p);
            seen.push(p);
            keep
        });
    }
}

/// Si un árbol leído se puede usar: la superficie principal primero (y solo ella), y en cada árbol
/// cada split con sus dos lados y cada nodo colgando de un split; cada ventana, con algún grupo.
/// egui_dock no revisa nada de eso al leer, y con un árbol así se cae al dibujarlo.
fn sound<T>(state: &DockState<T>) -> bool {
    let mut surfaces = state.iter_surfaces();
    let main = matches!(surfaces.next(), Some(Surface::Main(tree)) if tree_sound(tree));
    main && surfaces.all(|s| match s {
        Surface::Empty => true,
        Surface::Main(_) => false,
        Surface::Window(tree, _) => tree_sound(tree) && first_leaf(tree).is_some(),
    })
}

/// Si en `tree` cada split tiene sus dos lados y cada nodo cuelga de un split.
fn tree_sound<T>(tree: &Tree<T>) -> bool {
    (0..tree.len()).map(NodeIndex).all(|n| {
        let node = &tree[n];
        let hangs = node.is_empty() || n.parent().is_none_or(|p| tree[p].is_parent());
        let sides = !node.is_parent()
            || (n.right().0 < tree.len()
                && !tree[n.left()].is_empty()
                && !tree[n.right()].is_empty());
        hangs && sides
    })
}

/// El primer grupo de `tree` (por índice), si tiene alguno.
fn first_leaf<T>(tree: &Tree<T>) -> Option<NodeIndex> {
    (0..tree.len()).map(NodeIndex).find(|&n| tree[n].is_leaf())
}

/// El primer grupo de `tree` (por índice) que no es el del lienzo.
fn first_panel_leaf(tree: &Tree<PanelId>) -> Option<NodeIndex> {
    (0..tree.len()).map(NodeIndex).find(|&n| match &tree[n] {
        Node::Leaf(leaf) => !leaf.tabs.contains(&PanelId::Canvas),
        _ => false,
    })
}

/// `id` en el primer grupo de la superficie principal que no sea el del lienzo; sin ninguno, en
/// uno nuevo a la derecha del lienzo (o, sin lienzo, en la superficie principal).
fn push_beside_canvas(state: &mut DockState<PanelId>, id: PanelId) {
    let main = state.main_surface_mut();
    if let Some(n) = first_panel_leaf(main)
        && let Node::Leaf(leaf) = &mut main[n]
    {
        leaf.append_tab(id);
        return;
    }
    match main.find_tab(&PanelId::Canvas) {
        Some((canvas, _)) => {
            main.split_right(canvas, 0.5, vec![id]);
        }
        None if first_leaf(main).is_some() => main.push_to_focused_leaf(id),
        None => *main = Tree::new(vec![id]),
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::*;
    use crate::workspace::skin;
    use crate::workspace::test_util::{frames, test_app};

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
    fn collapsing_to_icons_leaves_color_where_it_is() {
        let mut w = Workspace::essentials();
        // Color solo: nada que mandar a la tira (su botón no abriría nada).
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        assert!(!w.collapsible(s, n));
        w.collapse_to_icons(s, n);
        assert!(w.docked(PanelId::Color) && !w.strip().contains(&PanelId::Color));
        // Con Library al lado: Library a la tira, Color se queda.
        w.dock(PanelId::Library, DockTarget::Focused);
        let (s, n) = w.leaf_of(PanelId::Library).unwrap();
        assert!(w.collapsible(s, n));
        w.collapse_to_icons(s, n);
        assert!(w.docked(PanelId::Color) && !w.strip().contains(&PanelId::Color));
        assert!(!w.docked(PanelId::Library));
        assert_eq!(w.strip(), PanelId::DOCK);
        // Groom solo debajo de Color: a la tira, y su grupo desaparece.
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Groom, DockTarget::Split(s, n, Split::Below));
        let (s, n) = w.leaf_of(PanelId::Groom).unwrap();
        w.collapse_to_icons(s, n);
        assert_eq!(w.strip(), PanelId::DOCK);
        // Quedan el lienzo y Color (fase D: el lienzo es una hoja del árbol).
        assert_eq!(w.state.iter_leaves().count(), 2);
    }

    #[test]
    fn focusing_a_panel_makes_its_tab_the_active_one() {
        let mut w = Workspace::essentials();
        w.dock(PanelId::Lospec, DockTarget::Focused);
        w.dock(PanelId::Groom, DockTarget::Focused);
        let active = |w: &Workspace| {
            let (s, n) = w.leaf_of(PanelId::Lospec).unwrap();
            let leaf = w
                .state
                .leaf(NodePath {
                    surface: s,
                    node: n,
                })
                .unwrap();
            leaf.tabs[leaf.active.0]
        };
        assert_eq!(active(&w), PanelId::Groom);
        assert!(w.focus(PanelId::Lospec));
        assert_eq!(active(&w), PanelId::Lospec);
        // En la tira no hay pestaña que enfocar.
        assert!(!w.focus(PanelId::Slices));
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
            let expected = usize::from(
                PanelId::DOCK.contains(&id) || matches!(id, PanelId::Color | PanelId::Canvas),
            );
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

    // --- Guardar y leer el acomodo ---

    #[test]
    fn a_saved_workspace_round_trips() {
        let mut w = Workspace::essentials();
        w.dock(PanelId::Library, DockTarget::Focused);
        w.float(PanelId::Lumen, egui::pos2(300.0, 200.0));
        let ron = ron::to_string(&w.to_saved()).unwrap();
        let back = Workspace::from_saved(ron::from_str(&ron).unwrap());
        assert!(back.docked(PanelId::Library) && back.floating(PanelId::Lumen));
        assert_eq!(back.strip().len(), w.strip().len());
    }

    #[test]
    fn a_saved_workspace_carries_its_version_and_one_without_it_is_version_0() {
        let saved = Workspace::demo("split").unwrap().to_saved();
        assert_eq!(saved.version, SAVED_VERSION);
        let ron = ron::to_string(&saved).unwrap();
        let back: SavedWorkspace = ron::from_str(&ron).unwrap();
        assert_eq!(back.version, SAVED_VERSION);
        // Lo de antes (sin el campo) es la versión 0, y se lee igual.
        let before = ron.replacen(&format!("version:{SAVED_VERSION},"), "", 1);
        assert_ne!(before, ron);
        let back: SavedWorkspace = ron::from_str(&before).unwrap();
        assert_eq!(back.version, 0);
        assert!(Workspace::from_saved(back).docked(PanelId::Groom));
    }

    #[test]
    fn a_version_1_workspace_gets_the_canvas_left_of_its_column_which_keeps_its_width() {
        // Lo que guardaba la fase C: la columna derecha sola (Color sobre Groom), sin lienzo.
        let mut state = DockState::new(vec!["color".to_string()]);
        state
            .main_surface_mut()
            .split_below(NodeIndex::root(), 0.4, vec!["groom".to_string()]);
        let saved = SavedWorkspace {
            version: 1,
            state,
            strip: Vec::new(),
        };
        let ron = ron::to_string(&saved).unwrap();
        let w = Workspace::from_saved(ron::from_str(&ron).unwrap());
        // `H[Canvas | V[Color / Groom]]`: el lienzo a la izquierda de la columna de antes.
        let main = w.state.main_surface();
        assert!(matches!(main[NodeIndex::root()], Node::Horizontal(_)));
        let leaf = |n: NodeIndex| match &main[n] {
            Node::Leaf(leaf) => leaf.tabs.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(leaf(NodeIndex::root().left()), [PanelId::Canvas]);
        let column = NodeIndex::root().right();
        match &main[column] {
            Node::Vertical(split) => assert_eq!(split.fraction, 0.4),
            other => panic!("{other:?}"),
        }
        assert_eq!(leaf(column.left()), [PanelId::Color]);
        assert_eq!(leaf(column.right()), [PanelId::Groom]);
        assert_eq!(w.places(PanelId::Canvas), 1);
        // Lo que se guarda ahora es la versión 2 (y se lee igual, sin volver a migrar).
        let again = w.to_saved();
        assert_eq!(again.version, 2);
        let back = Workspace::from_saved(again);
        assert_eq!(back.state_tabs(), w.state_tabs());
        // Y la columna, del ancho elegido (el de `view.palette_width`, que no va en el árbol).
        let mut app = test_app();
        app.view.palette_width = Some(300.0);
        app.workspace = Workspace::from_saved(ron::from_str(&ron).unwrap());
        frames(&egui::Context::default(), &mut app);
        let column = app.workspace.column_rect().expect("the right column");
        assert_eq!(column.width(), 300.0, "{column:?}");
        assert!(app.workspace.docked(PanelId::Groom));
    }

    #[test]
    fn unknown_or_repeated_panels_are_dropped_and_missing_ones_go_to_the_strip() {
        let mut saved = Workspace::essentials().to_saved();
        saved.state.push_to_focused_leaf("nope".to_string());
        saved.state.push_to_focused_leaf("color".to_string()); // repetido
        saved.strip.retain(|k| k != "groom"); // falta
        let w = Workspace::from_saved(saved);
        assert!(w.strip().contains(&PanelId::Groom));
        assert_eq!(
            w.state_tabs()
                .iter()
                .filter(|p| **p == PanelId::Color)
                .count(),
            1
        );
    }

    #[test]
    fn a_group_or_a_window_of_unknown_panels_goes_away_without_breaking_the_tree() {
        // Lo de escritorio leído en la web: "nope" a la izquierda de un split (Color sobre Groom) y
        // otro solo en una ventana (como dafont).
        let mut saved = Workspace::demo("split").unwrap().to_saved();
        saved
            .state
            .main_surface_mut()
            .split_left(NodeIndex::root(), 0.3, vec!["nope".to_string()]);
        saved.state.add_window(vec!["dafont?".to_string()]);
        let w = Workspace::from_saved(saved);
        assert!(sound(&w.state));
        assert_eq!(
            w.state_tabs(),
            vec![PanelId::Canvas, PanelId::Color, PanelId::Groom]
        );
        assert_ne!(w.leaf_of(PanelId::Color), w.leaf_of(PanelId::Groom));
        assert!(w.docked(PanelId::Color) && w.docked(PanelId::Groom));
        assert_eq!(w.state.iter_surfaces().count(), 1, "no windows left");
        // Y se puede seguir usando: acoplar, flotar y volver a la tira.
        let mut w = w;
        w.dock(PanelId::Library, DockTarget::Focused);
        assert_eq!(w.leaf_of(PanelId::Library), w.leaf_of(PanelId::Color));
        w.to_strip(PanelId::Groom);
        assert_eq!(
            w.state_tabs(),
            vec![PanelId::Canvas, PanelId::Color, PanelId::Library]
        );
    }

    #[test]
    fn color_missing_from_a_saved_workspace_goes_back_to_the_main_surface() {
        let mut saved = Workspace::demo("library-tab").unwrap().to_saved();
        saved.state.retain_tabs(|k| k != "color");
        let w = Workspace::from_saved(saved);
        assert!(w.docked(PanelId::Color) && w.docked(PanelId::Library));
        assert!(!w.strip().contains(&PanelId::Library));
        // Color flotando (la columna vacía) se respeta.
        let mut w = Workspace::essentials();
        w.float(PanelId::Color, egui::pos2(100.0, 100.0));
        let back = Workspace::from_saved(w.to_saved());
        assert!(back.floating(PanelId::Color) && !back.docked(PanelId::Color));
        assert_eq!(back.strip(), PanelId::DOCK);
    }

    #[test]
    fn a_malformed_or_empty_saved_workspace_gives_essentials() {
        let is_essentials = |w: &Workspace| {
            w.state_tabs() == vec![PanelId::Canvas, PanelId::Color]
                && w.docked(PanelId::Color)
                && w.strip() == PanelId::DOCK
        };
        // Un split sin uno de sus lados.
        let mut saved = Workspace::demo("split").unwrap().to_saved();
        saved.state.main_surface_mut()[NodeIndex(2)] = Node::Empty;
        assert!(is_essentials(&Workspace::from_saved(saved)));
        // Nada que se conozca.
        let saved = SavedWorkspace {
            version: SAVED_VERSION,
            state: DockState::new(vec!["nope".to_string()]),
            strip: Vec::new(),
        };
        assert!(is_essentials(&Workspace::from_saved(saved)));
        // Sin campos: el de fábrica.
        let saved: SavedWorkspace = ron::from_str("()").unwrap();
        assert!(is_essentials(&Workspace::from_saved(saved)));
        // Sin la tira: se arma sola.
        let mut saved = Workspace::demo("split").unwrap().to_saved();
        saved.strip.clear();
        let w = Workspace::from_saved(saved);
        assert!(w.docked(PanelId::Groom));
        assert!(!w.strip().contains(&PanelId::Groom));
        assert_eq!(w.strip().len(), PanelId::DOCK.len() - 1);
    }

    #[test]
    fn non_finite_numbers_in_a_saved_workspace_are_repaired_and_it_draws() {
        // El split de Color sobre Groom (la raíz es la del lienzo y la columna).
        let fraction = |w: &Workspace| match &w.state.main_surface()[NodeIndex::root().right()] {
            Node::Vertical(split) => split.fraction,
            other => panic!("{other:?}"),
        };
        let mut demo = Workspace::demo("split").unwrap();
        if let Node::Horizontal(split) = &mut demo.state.main_surface_mut()[NodeIndex::root()] {
            split.fraction = 0.75;
        }
        let split = ron::to_string(&demo.to_saved()).unwrap();
        assert_eq!(split.matches("fraction:0.5,").count(), 1, "{split}");
        let ctx = egui::Context::default();
        for (bad, fixed) in [
            ("NaN", 0.5),
            ("inf", 0.5),
            ("-inf", 0.5),
            ("-3.0", 0.0),
            ("3.0", 1.0),
        ] {
            let text = split.replace("fraction:0.5,", &format!("fraction:{bad},"));
            let w = Workspace::from_saved(ron::from_str(&text).unwrap());
            assert_eq!(fraction(&w), fixed, "{bad}");
            // Y se dibuja (en debug, egui revisa que ningún rect sea NaN).
            let mut app = test_app();
            app.workspace = w;
            frames(&ctx, &mut app);
            assert!(fraction(&app.workspace).is_finite(), "{bad}");
            let groom = app.workspace.leaf_rect(PanelId::Groom).unwrap();
            assert!(groom.is_finite() && groom.is_positive(), "{bad}: {groom:?}");
        }
        // Una ventana con su posición pedida en NaN: se descarta y la ventana se dibuja.
        let float = ron::to_string(&Workspace::demo("float").unwrap().to_saved()).unwrap();
        assert_eq!(float.matches("x:320.0").count(), 1, "{float}");
        let text = float.replace("x:320.0", "x:NaN");
        let mut app = test_app();
        app.workspace = Workspace::from_saved(ron::from_str(&text).unwrap());
        frames(&ctx, &mut app);
        let (surface, _) = app.workspace.leaf_of(PanelId::Lumen).unwrap();
        let rect = ctx
            .memory(|m| m.area_rect(skin::window_area_id(surface)))
            .expect("Lumen's window");
        assert!(rect.is_finite(), "{rect:?}");
    }
}
