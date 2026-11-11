//! Las operaciones sobre el acomodo (acoplar, flotar, mandar a la tira, colapsar, enfocar), sus
//! invariantes (cada panel en un solo lugar, un árbol que egui_dock pueda dibujar) y lo que se
//! guarda y se lee de la configuración ([`SavedWorkspace`]), además de los acomodos de
//! demostración.

use eframe::egui;
use egui_dock::{DockState, Node, NodePath, Surface, TabIndex, Tree, WindowState};

use super::{NodeIndex, Split, SurfaceIndex, Workspace};
use crate::dock::Edge;
use crate::panel_registry::PanelId;

/// El alto de fábrica de un panel que se pone a flotar (si su ficha no pide más).
pub(super) const FLOAT_HEIGHT: f32 = crate::panel_registry::DEFAULT_HEIGHT;

/// Dónde estaba un panel cuando se ocultó (el timeline o el Preview, ver
/// [`Workspace::sync_panel`]): ahí vuelve.
#[derive(Clone, Debug)]
pub(super) enum Spot {
    /// En el grupo de `with`, como pestaña en el lugar `index` (sin cambiar la activa).
    Tab { with: PanelId, index: usize },
    /// Solo en su grupo, junto al nodo que tenía exactamente estos paneles (en la superficie
    /// principal o en una ventana): de ese lado y con la fracción de su split.
    Beside {
        of: Vec<PanelId>,
        side: Split,
        fraction: f32,
    },
    /// Solo en una ventana: lo de adentro, en este rect.
    Window(egui::Rect),
}

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
    /// Los paneles de la tira de la derecha (la de siempre), en orden. Hasta la versión 5 era la
    /// única tira: lo de una 5 (o anterior) que no está en el árbol va a ella.
    pub strip: Vec<String>,
    /// Los de las tiras de la izquierda, de arriba y de abajo (desde la versión 6), en orden.
    pub left: Vec<String>,
    pub top: Vec<String>,
    pub bottom: Vec<String>,
}

/// La versión de lo que se guarda ahora: 6, con una tira de íconos por orilla (fase D, tarea 6:
/// `strip` es la de la derecha y `left`, `top` y `bottom` las demás; en una 5 o anterior solo
/// había `strip`, la de la derecha). La 5: el árbol de toda la ventana con el lienzo, la
/// toolbar, el timeline y el Preview (fase D, tarea 5). En la 4 el Preview no iba en el árbol (era
/// su propia ventana): al leer una 4 (o anterior) no está y, prendido, el workspace lo pone en su
/// esquina, como estaba; en una 5 está si no va en su esquina (ver
/// [`Workspace::preview_anchored`]). En la 3 el timeline no iba en el árbol (era el panel fijo de
/// abajo): al leer una 3 (o anterior) va debajo del lienzo, como estaba; en una 4 o 5 siempre está
/// (oculto, se guarda en su lugar) y si falta, también va ahí. En la 2 la toolbar no iba en el
/// árbol (era el panel fijo de la izquierda): una toolbar que no está en el árbol de una 2 va a su
/// izquierda; en una 3 o 4, está en la tira. La 1 (y la 0, de antes del campo) es la de la fase C:
/// la columna derecha sola, sin lienzo; al leerla se le mete el lienzo a la izquierda
/// ([`Workspace::from_saved`]).
pub(crate) const SAVED_VERSION: u32 = 6;

impl Default for SavedWorkspace {
    fn default() -> Self {
        Workspace::essentials().to_saved()
    }
}

impl Workspace {
    /// El acomodo de fábrica: la toolbar a la izquierda del lienzo (del ancho de su caja), el
    /// timeline debajo del lienzo (de su alto elegido) y, a la derecha de los dos, Color acoplado
    /// solo (`H[Tools | H[V[Canvas / Timeline] | Color]]`, la columna derecha con el ancho de
    /// `view.palette_width`): la toolbar y Color de alto completo, como los paneles fijos de antes.
    /// Los paneles del dock, en la tira, en su orden. El foco, en el grupo de Color (ahí se acopla
    /// lo de la tira).
    pub(crate) fn essentials() -> Workspace {
        let mut state = DockState::new(vec![PanelId::Canvas]);
        let tree = state.main_surface_mut();
        let [canvas, _] = tree.split_left(NodeIndex::root(), 0.5, vec![PanelId::Tools]);
        let [canvas, color] = tree.split_right(canvas, 0.5, vec![PanelId::Color]);
        tree.split_below(canvas, 0.7, vec![PanelId::Timeline]);
        state.set_focused_node_and_surface(NodePath {
            surface: SurfaceIndex::main(),
            node: color,
        });
        let mut strips: [Vec<PanelId>; 4] = Default::default();
        strips[Edge::Right.index()] = PanelId::DOCK.to_vec();
        Workspace {
            state,
            strips,
            pixel: true,
            lent: false,
            placements: Vec::new(),
            placing: Vec::new(),
            sides: Default::default(),
            spots: Vec::new(),
            preview_anchored: false,
        }
    }

    /// Lo que se guarda del acomodo. Las ventanas flotantes llevan en su árbol el rect de lo de
    /// adentro (el de su grupo de arriba): de ahí sale dónde reabrirlas (ver
    /// [`Workspace::place_restored_windows`]).
    /// El timeline oculto se guarda en su lugar, y el Preview oculto, si tenía uno (al leerlos, si
    /// siguen apagados, se vuelven a ocultar). El Preview en su esquina no se guarda (al leerlo,
    /// prendido, vuelve a ella: ver [`Workspace::preview_anchored`]).
    pub(crate) fn to_saved(&self) -> SavedWorkspace {
        let mut w = self.clone();
        if w.state.find_tab(&PanelId::Timeline).is_none() {
            w.show_panel(PanelId::Timeline);
        }
        if w.state.find_tab(&PanelId::Preview).is_none()
            && w.spots.iter().any(|(p, _)| *p == PanelId::Preview)
        {
            w.show_panel(PanelId::Preview);
        }
        if w.preview_anchored
            && let Some(path) = w.state.find_tab(&PanelId::Preview)
        {
            w.state.remove_tab(path);
        }
        let keys = |edge: Edge| {
            w.strip_at(edge)
                .iter()
                .map(|p| p.key().to_owned())
                .collect()
        };
        SavedWorkspace {
            version: SAVED_VERSION,
            state: w.state.map_tabs(|p| p.key().to_owned()),
            strip: keys(Edge::Right),
            left: keys(Edge::Left),
            top: keys(Edge::Top),
            bottom: keys(Edge::Bottom),
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
    /// el árbol y los repetidos. Los del dock que no quedan en el árbol van a la tira en que se
    /// guardaron (en una 5 o anterior, la de la derecha; si no estaban en ninguna, también), y
    /// Color, si no quedó en ningún lado, a la superficie principal. Sin nada en el árbol,
    /// también Essentials.
    ///
    /// Lo de la fase C (versión 1, o 0 sin el campo) es la columna derecha sola: se le mete el
    /// lienzo a la izquierda (su ancho es `view.palette_width`, que no cambia). Lo de antes de la
    /// versión 3 no tiene la toolbar (era un panel fijo): va a la izquierda de todo, como estaba.
    /// Lo de antes de la 4 no tiene el timeline (era el panel fijo de abajo): va debajo del
    /// lienzo, como estaba (su alto es `view.timeline_height`); en una 4 sin él, también. Lo de
    /// antes de la 5 no tiene el Preview (era su propia ventana): prendido, va a su esquina.
    /// Y el lienzo queda una vez, solo y en la superficie principal (ver `fit::tidy_canvas`).
    pub(crate) fn from_saved(saved: SavedWorkspace) -> Workspace {
        let essentials = |why: &str| {
            log::warn!("discarding the saved workspace: {why}");
            Workspace::essentials()
        };
        if !sound(&saved.state) {
            return essentials("its dock tree is malformed");
        }
        // La toolbar de antes de la versión 3, el timeline de antes de la 4 y el Preview de antes de
        // la 5 no pueden venir en el árbol (no eran de él).
        let in_tree = |id: PanelId| {
            let ours = (matches!(id, PanelId::Canvas | PanelId::Color)
                || Workspace::goes_to_strip(id))
                && (saved.version >= 3 || id != PanelId::Tools);
            ours || (id == PanelId::Timeline && saved.version >= 4)
                || (id == PanelId::Preview && saved.version >= 5)
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
        // Antes de la 3, la toolbar era el panel fijo de la izquierda: a la izquierda de todo (su
        // ancho lo pone `pin_sides`). Después de `tidy_canvas`, que puede meter el lienzo partiendo
        // la raíz: la toolbar queda a la izquierda también de él.
        if saved.version < 3 {
            let main = state.main_surface_mut();
            if main.num_tabs() == 0 {
                *main = Tree::new(vec![PanelId::Tools]);
            } else {
                main.split_left(NodeIndex::root(), 0.5, vec![PanelId::Tools]);
            }
        }
        // El timeline, debajo del lienzo (su alto lo pone `pin_sides`).
        if state.find_tab(&PanelId::Timeline).is_none() {
            put_below_canvas(&mut state);
        }
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
        // Las tiras: cada panel que va en una y no quedó en el árbol, en la que se guardó (en una
        // 5 o anterior, la única era la de la derecha); los que no están en ninguna, a la derecha.
        let none = Vec::new();
        let older = saved.version < 6;
        let saved_strips = [
            (Edge::Left, if older { &none } else { &saved.left }),
            (Edge::Right, &saved.strip),
            (Edge::Top, if older { &none } else { &saved.top }),
            (Edge::Bottom, if older { &none } else { &saved.bottom }),
        ];
        w.strips = Default::default();
        for (edge, keys) in saved_strips {
            for id in keys.iter().filter_map(|k| PanelId::from_key(k)) {
                if Workspace::goes_to_strip(id)
                    && w.state.find_tab(&id).is_none()
                    && w.strip_of(id).is_none()
                {
                    w.strips[edge.index()].push(id);
                }
            }
        }
        for id in PanelId::ALL.iter().copied() {
            if Workspace::goes_to_strip(id)
                && w.state.find_tab(&id).is_none()
                && w.strip_of(id).is_none()
            {
                w.strips[Edge::Right.index()].push(id);
            }
        }
        for strip in &mut w.strips {
            strip.sort_by_key(|p| PanelId::ALL.iter().position(|q| q == p));
        }
        w
    }

    /// El timeline o el Preview a la vista (`show`) o fuera del árbol. Oculto (apagado, o no
    /// disponible: el timeline en un artboard de fuente o de modelo, los dos en la pestaña de un
    /// modelo) no ocupa lugar (lo demás crece), pero se recuerda dónde estaba ([`Spot`]) y ahí
    /// vuelve; sin un lugar (o si ya no existe), el timeline va debajo del lienzo y el Preview a
    /// flotar en su esquina. Devuelve si cambió algo.
    pub(crate) fn sync_panel(&mut self, id: PanelId, show: bool) -> bool {
        let shown = self.state.find_tab(&id).is_some();
        match (show, shown) {
            (false, true) => self.hide_panel(id),
            (true, false) => self.show_panel(id),
            _ => return false,
        }
        true
    }

    /// [`Self::sync_panel`] del timeline.
    #[cfg(test)]
    pub(crate) fn sync_timeline(&mut self, show: bool) -> bool {
        self.sync_panel(PanelId::Timeline, show)
    }

    /// Saca `id` del árbol y recuerda dónde estaba (el Preview en su esquina, no: vuelve a ella).
    fn hide_panel(&mut self, id: PanelId) {
        let Some(path) = self.state.find_tab(&id) else {
            return;
        };
        let spot = if id == PanelId::Preview && self.preview_anchored {
            None
        } else {
            self.spot_of(id, path.node_path())
        };
        let focus = self.focused_panel();
        // Si era lo único de su ventana, la ventana se va: lo que se iba a acomodar de ella también
        // (lo de las demás ventanas de un acomodo leído sigue; los índices de las superficies no
        // se mueven al quitar una).
        let alone = !path.surface.is_main()
            && self.state[path.surface]
                .iter()
                .filter(|n| n.is_leaf())
                .count()
                == 1
            && self
                .state
                .leaf(path.node_path())
                .is_ok_and(|leaf| leaf.tabs.len() == 1);
        if alone {
            self.placements.retain(|(s, _)| *s != path.surface);
        }
        self.placing.retain(|(p, _, _)| *p != id);
        self.state.remove_tab(path);
        self.spots.retain(|(p, _)| *p != id);
        if let Some(spot) = spot {
            self.spots.push((id, spot));
        }
        self.refocus(focus);
    }

    /// Dónde está `id`, del grupo `path`, para volver ahí (ver [`Spot`]).
    fn spot_of(&self, id: PanelId, path: NodePath) -> Option<Spot> {
        let leaf = self.state.leaf(path).ok()?;
        if let Some(&with) = leaf.tabs.iter().find(|&&t| t != id) {
            let index = leaf
                .tabs
                .iter()
                .position(|&t| t == id)
                .unwrap_or(leaf.tabs.len());
            return Some(Spot::Tab { with, index });
        }
        let tree = &self.state[path.surface];
        let Some(parent) = path.node.parent() else {
            // Solo en una ventana: donde estaba lo de adentro (o donde se pidió, si todavía no se
            // dibujó).
            let rect = self.root_rect(path.surface).or_else(|| {
                let (_, at, _) = self.placing.iter().find(|(p, _, _)| *p == id)?;
                Some(egui::Rect::from_min_size(*at, id.spec().default_size))
            })?;
            return Some(Spot::Window(rect));
        };
        let (horizontal, fraction) = match &tree[parent] {
            Node::Horizontal(s) => (true, s.fraction),
            Node::Vertical(s) => (false, s.fraction),
            _ => return None,
        };
        let first = path.node.is_left();
        let other = if first { parent.right() } else { parent.left() };
        let side = match (horizontal, first) {
            (true, true) => Split::Left,
            (true, false) => Split::Right,
            (false, true) => Split::Above,
            (false, false) => Split::Below,
        };
        Some(Spot::Beside {
            of: super::fit::panels_of(tree, other),
            side,
            fraction,
        })
    }

    /// Vuelve a meter `id` donde estaba (ver [`Spot`]); si ese lugar ya no existe (o no se
    /// conoce), el timeline debajo del lienzo y el Preview a flotar en su esquina.
    pub(super) fn show_panel(&mut self, id: PanelId) {
        if self.state.find_tab(&id).is_some() {
            return;
        }
        let focus = self.focused_panel();
        let spot = self
            .spots
            .iter()
            .position(|(p, _)| *p == id)
            .map(|i| self.spots.remove(i).1);
        let placed = match spot {
            // En su lugar entre las pestañas, sin cambiar la que se ve.
            Some(Spot::Tab { with, index }) => match self.state.find_tab(&with) {
                Some(path) => match self.state.leaf_mut(path.node_path()) {
                    Ok(leaf) => {
                        let index = index.min(leaf.tabs.len());
                        leaf.tabs.insert(index, id);
                        if leaf.active.0 >= index {
                            leaf.active = TabIndex(leaf.active.0 + 1);
                        }
                        true
                    }
                    Err(_) => false,
                },
                None => false,
            },
            Some(Spot::Beside { of, side, fraction }) => match node_with(&self.state, &of) {
                Some(path) => {
                    self.state.split(path, side, fraction, Node::leaf(id));
                    true
                }
                None => false,
            },
            Some(Spot::Window(rect)) => {
                let surface = self.state.add_window(vec![id]);
                if let Some(window) = self.state.get_window_state_mut(surface) {
                    window.set_position(rect.min.round());
                    window.set_size(rect.size().round());
                }
                // El rect de lo de adentro, ya en su grupo: así lo lleva lo que se guarda (un
                // acomodo leído reabre ahí sus ventanas) y, si se vuelve a ocultar antes de
                // dibujarse, recuerda el mismo lugar.
                self.state[surface][NodeIndex::root()].set_rect(rect);
                // Lo de adentro donde estaba (con el marco medido, como un acomodo leído).
                self.placements.retain(|(s, _)| *s != surface);
                self.placements.push((surface, rect));
                true
            }
            None => false,
        };
        if id == PanelId::Preview {
            self.preview_anchored = !placed;
        }
        if !placed {
            match id {
                PanelId::Preview => self.float_at_corner(id),
                _ => put_below_canvas(&mut self.state),
            }
        }
        self.refocus(focus);
    }

    /// `id` (el Preview) solo en una ventana nueva, de su tamaño de fábrica: dónde va (su esquina)
    /// lo pide `show` cada frame (ver `Workspace::place_preview`).
    fn float_at_corner(&mut self, id: PanelId) {
        let surface = self.state.add_window(vec![id]);
        if let Some(window) = self.state.get_window_state_mut(surface) {
            window.set_size(id.spec().default_size);
        }
    }

    /// El panel activo del grupo con el foco, si hay uno.
    fn focused_panel(&self) -> Option<PanelId> {
        let path = self.state.focused_leaf()?;
        let leaf = self.state.leaf(path).ok()?;
        leaf.tabs.get(leaf.active.0).copied()
    }

    /// El foco de vuelta en el grupo de `id` (egui_dock se lo da a lo que parte o quita); si ya no
    /// está, en el de Color.
    fn refocus(&mut self, id: Option<PanelId>) {
        let path = id
            .and_then(|id| self.state.find_tab(&id))
            .or_else(|| self.state.find_tab(&PanelId::Color));
        if let Some(path) = path {
            self.state.set_focused_node_and_surface(path.node_path());
        }
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
    /// (Library en el grupo de Color), `split` (Groom debajo de Color), `float` (Lumen flotando),
    /// `tools-right` (la toolbar a la derecha del lienzo), `tools-float` (la toolbar flotando),
    /// `timeline-right` (el timeline a la derecha del lienzo), `preview-docked` (el Preview
    /// acoplado debajo de Color; se ve con `view.preview`), `left-strip` (la toolbar y Groom en la
    /// tira de la izquierda) o `groom-below` (Groom debajo del lienzo: la tira de la derecha, solo
    /// junto a él). `None` si no es ninguno de esos.
    pub(crate) fn demo(name: &str) -> Option<Workspace> {
        let mut w = Workspace::essentials();
        let (surface, node) = w.leaf_of(PanelId::Color)?;
        let (cs, cn) = w.leaf_of(PanelId::Canvas)?;
        match name {
            "tools-right" => w.dock(PanelId::Tools, DockTarget::Split(cs, cn, Split::Right)),
            "tools-float" => w.float(PanelId::Tools, egui::pos2(320.0, 120.0)),
            "timeline-right" => w.dock(PanelId::Timeline, DockTarget::Split(cs, cn, Split::Right)),
            "library-tab" => w.dock(PanelId::Library, DockTarget::Leaf(surface, node)),
            "preview-docked" => w.dock(
                PanelId::Preview,
                DockTarget::Split(surface, node, Split::Below),
            ),
            "split" => w.dock(
                PanelId::Groom,
                DockTarget::Split(surface, node, Split::Below),
            ),
            "float" => w.float(PanelId::Lumen, egui::pos2(320.0, 120.0)),
            "left-strip" => {
                w.to_strip_at(PanelId::Tools, Edge::Left);
                w.to_strip_at(PanelId::Groom, Edge::Left);
            }
            "groom-below" => w.dock(PanelId::Groom, DockTarget::Split(cs, cn, Split::Below)),
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
        self.leave_strips(id);
        if let Some(path) = self.state.find_tab(&id) {
            self.state.remove_tab(path);
        }
        // Lo que se mueve de lugar deja su esquina.
        if id == PanelId::Preview {
            self.preview_anchored = false;
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
            (Some(None), _) => self.leave_strips(id),
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

    /// `id` al grupo con el foco de la superficie principal; si el foco es del lienzo o de la
    /// toolbar sola (un clic en ellos se lo da) o de nadie, al primer grupo que no sea ninguno de
    /// esos (ver [`takes_focused`]); sin ninguno, a la derecha del lienzo.
    fn push_focused(&mut self, id: PanelId) {
        if let Some(path) = self.state.focused_leaf()
            && path.surface.is_main()
            && let Ok(leaf) = self.state.leaf_mut(path)
            && takes_focused(&leaf.tabs)
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
        // La toolbar, del ancho de su caja (el que dio el anfitrión, con el skin; antes del primer
        // frame, el del skin de respaldo). Si es poco (sin Pixel UI), `fit_windows` la agranda a
        // su mínimo; más ancha, nunca se angostaría.
        let width = match (id, self.sides.toolbar) {
            (PanelId::Tools, Some(toolbar)) => toolbar,
            (PanelId::Tools, None) => {
                crate::pixel_ui::toolbar_width(&crate::skin::Skin::fallback())
            }
            _ => spec.default_size.x,
        };
        let size = egui::vec2(width, spec.default_size.y);
        let surface = self.state.add_window(vec![id]);
        if let Some(window) = self.state.get_window_state_mut(surface) {
            window.set_position(at.round());
            window.set_size(size.round());
        }
        self.placing.retain(|(p, _, _)| *p != id);
        self.placing
            .push((id, at.round(), super::fit::NEW_WINDOW_FRAMES));
    }

    /// Saca `id` de todas las tiras.
    pub(super) fn leave_strips(&mut self, id: PanelId) {
        for strip in &mut self.strips {
            strip.retain(|&p| p != id);
        }
    }

    /// Regresa `id` a la tira de la orilla más cercana a donde está (ver [`Workspace::edge_of`]),
    /// en su lugar (el orden de `PanelId::ALL`).
    #[allow(
        clippy::wrong_self_convention,
        reason = "no es una conversión: manda el panel a la tira"
    )]
    pub(crate) fn to_strip(&mut self, id: PanelId) {
        let edge = self.edge_of(id).unwrap_or(Edge::Right);
        self.to_strip_at(id, edge);
    }

    /// Manda `id` a la tira de la orilla `edge`, en su lugar (el orden de `PanelId::ALL`).
    #[allow(
        clippy::wrong_self_convention,
        reason = "no es una conversión: manda el panel a la tira"
    )]
    pub(crate) fn to_strip_at(&mut self, id: PanelId, edge: Edge) {
        if id == PanelId::Canvas {
            return;
        }
        self.take(id);
        let rank = |p: &PanelId| PanelId::ALL.iter().position(|q| q == p);
        let strip = &mut self.strips[edge.index()];
        let at = strip.partition_point(|p| rank(p) < rank(&id));
        strip.insert(at, id);
    }

    /// La orilla de `id`: la de su tira, si está en una; la de su grupo, si está en el árbol (ver
    /// [`Workspace::group_edge`]).
    pub(crate) fn edge_of(&self, id: PanelId) -> Option<Edge> {
        if let Some(edge) = self.strip_of(id) {
            return Some(edge);
        }
        let path = self.state.find_tab(&id)?;
        Some(self.group_edge(path.surface, path.node))
    }

    /// La orilla de la ventana del grupo `(surface, node)`: la que toca (bajando de la raíz, cada
    /// split a lo ancho o a lo alto de su lado), primero a los lados (izquierda o derecha) y después
    /// arriba o abajo; si toca las dos de un eje (o ninguna), cuenta el otro. Un grupo que no toca
    /// ninguna (entre otros) o una ventana flotante, la más cercana a él en el último frame (en
    /// empate, también los lados primero). Sin rects (antes de dibujarse), la derecha.
    pub(crate) fn group_edge(&self, surface: SurfaceIndex, node: NodeIndex) -> Edge {
        if surface.is_main() {
            let tree = self.state.main_surface();
            let (mut left, mut right, mut top, mut bottom) = (true, true, true, true);
            let mut n = node;
            while let Some(parent) = n.parent() {
                match &tree[parent] {
                    Node::Horizontal(_) if n.is_left() => right = false,
                    Node::Horizontal(_) => left = false,
                    Node::Vertical(_) if n.is_left() => bottom = false,
                    Node::Vertical(_) => top = false,
                    _ => {}
                }
                n = parent;
            }
            match (left, right, top, bottom) {
                (true, false, ..) => return Edge::Left,
                (false, true, ..) => return Edge::Right,
                (_, _, true, false) => return Edge::Top,
                (_, _, false, true) => return Edge::Bottom,
                (true, true, ..) => return Edge::Right,
                _ => {}
            }
        }
        let Some(area) = self.root_rect(SurfaceIndex::main()) else {
            return Edge::Right;
        };
        let rect = self
            .state
            .leaf(NodePath { surface, node })
            .ok()
            .map(|leaf| leaf.rect)
            .filter(|r| r.is_finite() && r.is_positive());
        let Some(rect) = rect else {
            return Edge::Right;
        };
        // Acoplado, lo que hay de su borde a cada orilla; una ventana (puede pasarse del área),
        // desde su centro.
        let (lo, hi) = if surface.is_main() {
            (rect.min, rect.max)
        } else {
            (rect.center(), rect.center())
        };
        let gaps = [
            (lo.x - area.left(), Edge::Left),
            (area.right() - hi.x, Edge::Right),
            (lo.y - area.top(), Edge::Top),
            (area.bottom() - hi.y, Edge::Bottom),
        ];
        gaps.into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(Edge::Right, |(_, edge)| edge)
    }

    /// "Dock" del menú de un ícono de su tira: la toolbar vuelve a un lado del lienzo, el de su
    /// tira (a su derecha, desde la de la derecha) o, desde las demás, a su lugar de fábrica (a la
    /// izquierda de todo); los demás, al grupo con el foco (ver [`DockTarget::Focused`]).
    pub(crate) fn dock_from_strip(&mut self, id: PanelId) {
        if id != PanelId::Tools {
            self.dock(id, DockTarget::Focused);
            return;
        }
        if self.strip_of(id) == Some(Edge::Right)
            && let Some((s, n)) = self.leaf_of(PanelId::Canvas)
        {
            self.dock(id, DockTarget::Split(s, n, Split::Right));
            return;
        }
        let focus = self.focused_panel();
        self.take(id);
        let main = self.state.main_surface_mut();
        if main.num_tabs() == 0 {
            *main = Tree::new(vec![id]);
        } else {
            main.split_left(NodeIndex::root(), 0.5, vec![id]);
        }
        self.refocus(focus);
    }

    /// Los paneles del grupo `(surface, node)` (ninguno si no existe).
    fn tabs_of(&self, surface: SurfaceIndex, node: NodeIndex) -> Vec<PanelId> {
        self.state
            .leaf(NodePath { surface, node })
            .map(|leaf| leaf.tabs.clone())
            .unwrap_or_default()
    }

    /// Si `id` puede ir a la tira: los del dock y la toolbar, que tienen un flyout que su botón
    /// abre. Color no (su botón no abriría nada): se queda acoplado o flotando.
    pub(crate) fn goes_to_strip(id: PanelId) -> bool {
        id == PanelId::Tools || PanelId::DOCK.contains(&id)
    }

    /// Si el botón « del grupo `(surface, node)` mandaría algo a la tira.
    pub(crate) fn collapsible(&self, surface: SurfaceIndex, node: NodeIndex) -> bool {
        self.tabs_of(surface, node)
            .into_iter()
            .any(Workspace::goes_to_strip)
    }

    /// El botón « ("Collapse to Icons") del grupo `(surface, node)`: sus paneles a la tira de su
    /// orilla (ver [`Workspace::group_edge`]), salvo los que no van ahí (Color, que se queda en el
    /// grupo).
    pub(crate) fn collapse_to_icons(&mut self, surface: SurfaceIndex, node: NodeIndex) {
        let edge = self.group_edge(surface, node);
        for id in self.tabs_of(surface, node) {
            if Workspace::goes_to_strip(id) {
                self.to_strip_at(id, edge);
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
    /// o más. Las ventanas flotantes la llevan (es su título y de ahí se arrastran), salvo la del
    /// Preview solo (su ventana es la de antes, con su barra de título: ver
    /// [`Workspace::bare_preview`]) y, con Pixel UI, la de la toolbar sola: va del ancho de su caja,
    /// con la franja de agarre de un grupo solo (ver [`Workspace::bare_tools`]). Devuelve si alguna
    /// cambió.
    pub(crate) fn refresh_tab_bars(&mut self) -> bool {
        let bare = self.bare_preview();
        let tools = self.bare_tools();
        let mut changed = false;
        for (path, leaf) in self.state.iter_leaves_mut() {
            let hidden = (path.surface.is_main() && leaf.tabs.len() <= 1)
                || Some(path.surface) == bare
                || Some(path.surface) == tools;
            changed |= leaf.tab_bar_hidden != hidden;
            leaf.tab_bar_hidden = hidden;
        }
        changed
    }

    /// La ventana donde va `id` solo (un solo grupo, con él solo), si flota así.
    fn lone_window(&self, id: PanelId) -> Option<SurfaceIndex> {
        let path = self.state.find_tab(&id)?;
        if path.surface.is_main() {
            return None;
        }
        let tree = &self.state[path.surface];
        let leaves = tree.iter().filter(|n| n.is_leaf()).count();
        (leaves == 1 && tree.num_tabs() == 1).then_some(path.surface)
    }

    /// La ventana del Preview solo (un solo grupo, con él solo): su ventana es la de antes (con
    /// Pixel UI la del diseño, sin él la `egui::Window`: su marco y su barra de título, sin
    /// pestañas; ver `show::PreviewWindow`).
    pub(crate) fn bare_preview(&self) -> Option<SurfaceIndex> {
        self.lone_window(PanelId::Preview)
    }

    /// Con Pixel UI, la ventana de la toolbar sola: sin barra de pestañas (su pestaña "Tools" con
    /// su × la hacía casi tres veces más ancha que su caja), como un grupo acoplado solo: se lleva
    /// a acoplar de su franja de agarre, se mueve de lo que no es un botón y su « (con el puntero
    /// en la franja) la manda a una tira. Sin Pixel UI no hay franja: lleva su pestaña.
    pub(crate) fn bare_tools(&self) -> Option<SurfaceIndex> {
        self.lone_window(PanelId::Tools).filter(|_| self.pixel)
    }

    /// Cada panel en un solo lugar: lo que está en el árbol sale de las tiras (y las tiras sin
    /// repetidos, ni dentro de una ni entre ellas).
    pub(super) fn tidy(&mut self) {
        let state = &self.state;
        let mut seen = Vec::new();
        for strip in &mut self.strips {
            strip.retain(|&p| {
                let keep = state.find_tab(&p).is_none() && !seen.contains(&p);
                seen.push(p);
                keep
            });
        }
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

/// Si a un grupo con `tabs` va lo que se acopla "con el foco": no al del lienzo (no recibe
/// pestañas) ni al de la toolbar o el timeline solos (con otra pestaña dejarían de verse como hoy).
fn takes_focused(tabs: &[PanelId]) -> bool {
    !tabs.contains(&PanelId::Canvas) && tabs != [PanelId::Tools] && tabs != [PanelId::Timeline]
}

/// El nodo (de cualquier superficie) con exactamente los paneles `of` (en cualquier orden).
fn node_with(state: &DockState<PanelId>, of: &[PanelId]) -> Option<NodePath> {
    let same = |mut panels: Vec<PanelId>| {
        let mut of = of.to_vec();
        panels.sort_by_key(|p| p.key());
        of.sort_by_key(|p| p.key());
        !of.is_empty() && panels == of
    };
    state.iter_surfaces_indexed().find_map(|(surface, s)| {
        let tree = s.node_tree()?;
        (0..tree.len())
            .map(NodeIndex)
            .find(|&n| !tree[n].is_empty() && same(super::fit::panels_of(tree, n)))
            .map(|node| NodePath { surface, node })
    })
}

/// El timeline solo en un grupo nuevo debajo del lienzo (donde iba el panel de abajo de antes; su
/// alto lo pone `pin_sides`). Sin lienzo (no debería pasar), en la superficie principal.
fn put_below_canvas(state: &mut DockState<PanelId>) {
    match state.find_tab(&PanelId::Canvas) {
        Some(path) => {
            state.split(
                path.node_path(),
                Split::Below,
                0.7,
                Node::leaf(PanelId::Timeline),
            );
        }
        None => {
            let main = state.main_surface_mut();
            if main.num_tabs() == 0 {
                *main = Tree::new(vec![PanelId::Timeline]);
            } else {
                main.push_to_focused_leaf(PanelId::Timeline);
            }
        }
    }
}

/// El primer grupo de `tree` (por índice) que no es el del lienzo ni el de la toolbar o el
/// timeline solos.
fn first_panel_leaf(tree: &Tree<PanelId>) -> Option<NodeIndex> {
    (0..tree.len()).map(NodeIndex).find(|&n| match &tree[n] {
        Node::Leaf(leaf) => takes_focused(&leaf.tabs),
        _ => false,
    })
}

/// `id` en el primer grupo de la superficie principal que no sea el del lienzo (ni el de la
/// toolbar sola); sin ninguno, en uno nuevo a la derecha del lienzo (o, sin lienzo, en la
/// superficie principal).
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

    /// Los paneles del grupo `n` de la superficie principal de `w` (falla si no es un grupo).
    fn leaf_tabs(w: &Workspace, n: NodeIndex) -> Vec<PanelId> {
        match &w.state.main_surface()[n] {
            Node::Leaf(leaf) => leaf.tabs.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn essentials_has_the_toolbar_left_of_the_canvas() {
        // `H[Tools | H[V[Canvas / Timeline] | Color]]`: la toolbar es un lado del lienzo; la
        // columna, Color (tarea 4: el lienzo, con el timeline debajo).
        let w = Workspace::essentials();
        let root = NodeIndex::root();
        assert_eq!(leaf_tabs(&w, root.left()), [PanelId::Tools]);
        assert_eq!(leaf_tabs(&w, root.right().left().left()), [PanelId::Canvas]);
        assert_eq!(leaf_tabs(&w, root.right().right()), [PanelId::Color]);
        assert_eq!(w.tools_side(), Some(true));
        assert_eq!(w.column_panels(), [PanelId::Color]);
        assert_eq!(w.places(PanelId::Tools), 1);
    }

    #[test]
    fn the_toolbar_goes_to_the_strip_and_stays_there_when_read_back() {
        let mut w = Workspace::essentials();
        w.to_strip(PanelId::Tools);
        assert!(!w.docked(PanelId::Tools) && !w.floating(PanelId::Tools));
        // Tarea 6: a la tira de su orilla (la izquierda).
        let left = |w: &Workspace| w.strip_at(Edge::Left).first().copied();
        assert_eq!(left(&w), Some(PanelId::Tools));
        assert_eq!(w.places(PanelId::Tools), 1);
        assert_eq!(w.tools_side(), None);
        // Guardado y leído: sigue en la tira (no se vuelve a meter a la izquierda).
        let back = Workspace::from_saved(w.to_saved());
        assert!(!back.docked(PanelId::Tools));
        assert_eq!(left(&back), Some(PanelId::Tools));
        assert_eq!(back.places(PanelId::Tools), 1);
        // De la tira, a la derecha del lienzo: ahí es su lado (de su ancho), no la columna.
        let mut w = back;
        let (s, n) = w.leaf_of(PanelId::Canvas).unwrap();
        w.dock(PanelId::Tools, DockTarget::Split(s, n, Split::Right));
        assert_eq!(w.tools_side(), Some(false));
        assert_eq!(w.column_panels(), [PanelId::Color]);
        assert!(!w.strip().contains(&PanelId::Tools));
        // Con Color flotando, la toolbar a la derecha del lienzo tampoco es la columna.
        w.float(PanelId::Color, egui::pos2(100.0, 100.0));
        assert_eq!(w.tools_side(), Some(false));
        assert_eq!(w.column(), None);
    }

    #[test]
    fn a_version_2_workspace_gets_the_toolbar_left_of_everything() {
        // Lo que guardaba la tarea 2: `H[Canvas | V[Color / Groom]]`, sin la toolbar (era el
        // panel fijo de la izquierda).
        let mut state = DockState::new(vec!["canvas".to_string()]);
        let [_, column] =
            state
                .main_surface_mut()
                .split_right(NodeIndex::root(), 0.6, vec!["color".to_string()]);
        state
            .main_surface_mut()
            .split_below(column, 0.4, vec!["groom".to_string()]);
        let saved = SavedWorkspace {
            version: 2,
            state,
            strip: Vec::new(),
            left: Vec::new(),
            top: Vec::new(),
            bottom: Vec::new(),
        };
        let ron = ron::to_string(&saved).unwrap();
        let w = Workspace::from_saved(ron::from_str(&ron).unwrap());
        let root = NodeIndex::root();
        assert_eq!(leaf_tabs(&w, root.left()), [PanelId::Tools]);
        // Tarea 4: el lienzo, con el timeline debajo.
        assert_eq!(leaf_tabs(&w, root.right().left().left()), [PanelId::Canvas]);
        assert_eq!(
            leaf_tabs(&w, root.right().left().right()),
            [PanelId::Timeline]
        );
        assert_eq!(w.tools_side(), Some(true));
        assert_eq!(w.column_panels(), [PanelId::Color, PanelId::Groom]);
        assert_eq!(w.places(PanelId::Tools), 1);
        assert!(!w.strip().contains(&PanelId::Tools));
        // Lo que se guarda ahora es la 3, y se lee igual (sin otra toolbar).
        let again = w.to_saved();
        assert_eq!(again.version, SAVED_VERSION);
        let back = Workspace::from_saved(again);
        assert_eq!(back.state_tabs(), w.state_tabs());
        assert_eq!(back.places(PanelId::Tools), 1);
    }

    #[test]
    fn a_version_2_workspace_without_the_canvas_still_gets_the_toolbar_far_left() {
        // Una 2 mal guardada, sin el lienzo (solo Color): `tidy_canvas` lo mete partiendo la raíz;
        // la toolbar va después, a la izquierda de todo: `H[Tools | H[Canvas | Color]]`.
        let saved = SavedWorkspace {
            version: 2,
            state: DockState::new(vec!["color".to_string()]),
            strip: Vec::new(),
            left: Vec::new(),
            top: Vec::new(),
            bottom: Vec::new(),
        };
        let w = Workspace::from_saved(saved);
        let root = NodeIndex::root();
        assert_eq!(leaf_tabs(&w, root.left()), [PanelId::Tools]);
        // Tarea 4: el lienzo, con el timeline debajo.
        assert_eq!(leaf_tabs(&w, root.right().left().left()), [PanelId::Canvas]);
        assert_eq!(leaf_tabs(&w, root.right().right()), [PanelId::Color]);
        assert_eq!(w.tools_side(), Some(true));
    }

    #[test]
    fn what_docks_with_the_focus_never_joins_the_lone_toolbar() {
        // Un clic en la toolbar le da el foco a su grupo: lo que se acopla "con el foco" va al de
        // Color, no como pestaña de la toolbar.
        let mut w = Workspace::essentials();
        assert!(w.focus(PanelId::Tools));
        w.dock(PanelId::Library, DockTarget::Focused);
        assert_eq!(w.leaf_of(PanelId::Library), w.leaf_of(PanelId::Color));
        let (_, tools) = w.leaf_of(PanelId::Tools).unwrap();
        assert_eq!(leaf_tabs(&w, tools), [PanelId::Tools]);
    }

    // --- Fase D, tarea 4: el timeline en el árbol ---

    /// El grupo del lienzo y el split del que cuelga (falla si no hay).
    fn canvas_parent(w: &Workspace) -> (NodeIndex, NodeIndex) {
        let (_, canvas) = w.leaf_of(PanelId::Canvas).unwrap();
        (canvas, canvas.parent().unwrap())
    }

    #[test]
    fn essentials_has_the_timeline_below_the_canvas() {
        // `H[Tools | H[V[Canvas / Timeline] | Color]]`: la toolbar y Color de alto completo, el
        // timeline entre ellos, debajo del lienzo.
        let w = Workspace::essentials();
        let root = NodeIndex::root();
        assert_eq!(leaf_tabs(&w, root.left()), [PanelId::Tools]);
        let stack = root.right().left();
        assert!(matches!(w.state.main_surface()[stack], Node::Vertical(_)));
        assert_eq!(leaf_tabs(&w, stack.left()), [PanelId::Canvas]);
        assert_eq!(leaf_tabs(&w, stack.right()), [PanelId::Timeline]);
        assert_eq!(leaf_tabs(&w, root.right().right()), [PanelId::Color]);
        assert_eq!(w.places(PanelId::Timeline), 1);
        assert!(!w.strip().contains(&PanelId::Timeline));
        assert!(w.timeline_below());
        assert_eq!(w.column_panels(), [PanelId::Color]);
        assert_eq!(w.tools_side(), Some(true));
    }

    #[test]
    fn a_version_3_workspace_gets_the_timeline_below_the_canvas() {
        // Lo que guardaba la tarea 3: `H[Tools | H[Canvas | Color]]`, sin el timeline (era el
        // panel fijo de abajo).
        let mut state = DockState::new(vec!["canvas".to_string()]);
        let [canvas, _] =
            state
                .main_surface_mut()
                .split_left(NodeIndex::root(), 0.03, vec!["tools".to_string()]);
        state
            .main_surface_mut()
            .split_right(canvas, 0.7, vec!["color".to_string()]);
        let saved = SavedWorkspace {
            version: 3,
            state,
            strip: Vec::new(),
            left: Vec::new(),
            top: Vec::new(),
            bottom: Vec::new(),
        };
        let ron = ron::to_string(&saved).unwrap();
        let w = Workspace::from_saved(ron::from_str(&ron).unwrap());
        assert!(w.timeline_below());
        let (canvas, parent) = canvas_parent(&w);
        assert_eq!(leaf_tabs(&w, parent.right()), [PanelId::Timeline]);
        assert_eq!(canvas, parent.left());
        assert_eq!(w.places(PanelId::Timeline), 1);
        assert_eq!(w.column_panels(), [PanelId::Color]);
        assert_eq!(w.tools_side(), Some(true));
        // Lo que se guarda ahora es la 6 (tarea 6: con las tiras por orilla), y se lee igual (sin otro
        // timeline).
        let again = w.to_saved();
        assert_eq!(again.version, SAVED_VERSION);
        assert_eq!(SAVED_VERSION, 6);
        let back = Workspace::from_saved(again);
        assert_eq!(back.state_tabs(), w.state_tabs());
        assert_eq!(back.places(PanelId::Timeline), 1);
        // Una 4 sin el timeline (no debería pasar: se guarda en su lugar aunque esté oculto) lo
        // recupera debajo del lienzo.
        let mut saved = Workspace::essentials().to_saved();
        saved.state.retain_tabs(|k| k != "timeline");
        let w = Workspace::from_saved(saved);
        assert!(w.timeline_below());
    }

    #[test]
    fn hiding_the_timeline_takes_it_out_and_showing_it_puts_it_back_where_it_was() {
        // Debajo del lienzo (Essentials): sin él, el lienzo llega abajo; de vuelta, igual que antes.
        let mut w = Workspace::essentials();
        let before = w.state_tabs();
        w.sync_timeline(false);
        assert_eq!(w.places(PanelId::Timeline), 0);
        assert!(!w.timeline_below());
        let (_, parent) = canvas_parent(&w);
        assert!(matches!(
            w.state.main_surface()[parent],
            Node::Horizontal(_)
        ));
        // Ocultarlo dos veces no cambia nada.
        w.sync_timeline(false);
        assert_eq!(w.places(PanelId::Timeline), 0);
        w.sync_timeline(true);
        assert_eq!(w.places(PanelId::Timeline), 1);
        assert!(w.timeline_below());
        assert_eq!(w.state_tabs(), before);
        w.sync_timeline(true);
        assert_eq!(w.places(PanelId::Timeline), 1);
        // A la derecha del lienzo: vuelve a la derecha del lienzo.
        let (s, n) = w.leaf_of(PanelId::Canvas).unwrap();
        w.dock(PanelId::Timeline, DockTarget::Split(s, n, Split::Right));
        w.sync_timeline(false);
        w.sync_timeline(true);
        let (canvas, parent) = canvas_parent(&w);
        assert!(matches!(
            w.state.main_surface()[parent],
            Node::Horizontal(_)
        ));
        assert_eq!(canvas, parent.left());
        assert_eq!(leaf_tabs(&w, parent.right()), [PanelId::Timeline]);
        // En pestaña con Color: vuelve a su grupo.
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Timeline, DockTarget::Leaf(s, n));
        w.sync_timeline(false);
        assert_eq!(
            leaf_tabs(&w, w.leaf_of(PanelId::Color).unwrap().1),
            [PanelId::Color]
        );
        w.sync_timeline(true);
        assert_eq!(w.leaf_of(PanelId::Timeline), w.leaf_of(PanelId::Color));
        // Flotando: vuelve a flotar.
        w.float(PanelId::Timeline, egui::pos2(300.0, 200.0));
        w.sync_timeline(false);
        assert_eq!(w.places(PanelId::Timeline), 0);
        w.sync_timeline(true);
        assert!(w.floating(PanelId::Timeline));
        // Cada panel del workspace, una vez (el Preview, apagado, fuera del árbol).
        for &id in PanelId::ALL.iter().filter(|&&p| p != PanelId::Preview) {
            assert_eq!(w.places(id), 1, "{id:?}");
        }
    }

    #[test]
    fn a_hidden_timeline_tab_comes_back_at_its_index_without_changing_the_active_tab() {
        // En el grupo de Color: `[Color, Timeline, Library]`, con Library a la vista.
        let mut w = Workspace::essentials();
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Timeline, DockTarget::Leaf(s, n));
        w.dock(PanelId::Library, DockTarget::Leaf(s, n));
        let group = |w: &Workspace| {
            let (s, n) = w.leaf_of(PanelId::Color).unwrap();
            let leaf = w
                .state
                .leaf(NodePath {
                    surface: s,
                    node: n,
                })
                .unwrap();
            (leaf.tabs.clone(), leaf.tabs[leaf.active.0])
        };
        assert_eq!(
            group(&w),
            (
                vec![PanelId::Color, PanelId::Timeline, PanelId::Library],
                PanelId::Library
            )
        );
        w.sync_timeline(false);
        assert_eq!(
            group(&w),
            (vec![PanelId::Color, PanelId::Library], PanelId::Library)
        );
        w.sync_timeline(true);
        assert_eq!(
            group(&w),
            (
                vec![PanelId::Color, PanelId::Timeline, PanelId::Library],
                PanelId::Library
            )
        );
    }

    #[test]
    fn a_hidden_timeline_is_saved_in_its_place() {
        let mut w = Workspace::essentials();
        let (s, n) = w.leaf_of(PanelId::Canvas).unwrap();
        w.dock(PanelId::Timeline, DockTarget::Split(s, n, Split::Right));
        w.sync_timeline(false);
        let back = Workspace::from_saved(w.to_saved());
        let (canvas, parent) = canvas_parent(&back);
        assert_eq!(canvas, parent.left());
        assert_eq!(leaf_tabs(&back, parent.right()), [PanelId::Timeline]);
        assert_eq!(back.places(PanelId::Timeline), 1);
    }

    #[test]
    fn what_docks_with_the_focus_never_joins_the_lone_timeline() {
        let mut w = Workspace::essentials();
        assert!(w.focus(PanelId::Timeline));
        w.dock(PanelId::Library, DockTarget::Focused);
        assert_eq!(w.leaf_of(PanelId::Library), w.leaf_of(PanelId::Color));
        let (_, timeline) = w.leaf_of(PanelId::Timeline).unwrap();
        assert_eq!(leaf_tabs(&w, timeline), [PanelId::Timeline]);
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
        // Quedan la toolbar, el lienzo, el timeline y Color (fase D: son hojas del árbol).
        assert_eq!(w.state.iter_leaves().count(), 4);
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
                PanelId::DOCK.contains(&id)
                    || matches!(
                        id,
                        PanelId::Color | PanelId::Canvas | PanelId::Tools | PanelId::Timeline
                    ),
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
            left: Vec::new(),
            top: Vec::new(),
            bottom: Vec::new(),
        };
        let ron = ron::to_string(&saved).unwrap();
        let w = Workspace::from_saved(ron::from_str(&ron).unwrap());
        // `H[Tools | H[Canvas | V[Color / Groom]]]`: el lienzo a la izquierda de la columna de
        // antes, y la toolbar (que era el panel fijo) a la izquierda de todo.
        let main = w.state.main_surface();
        assert!(matches!(main[NodeIndex::root()], Node::Horizontal(_)));
        let leaf = |n: NodeIndex| match &main[n] {
            Node::Leaf(leaf) => leaf.tabs.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(leaf(NodeIndex::root().left()), [PanelId::Tools]);
        let canvas_side = NodeIndex::root().right();
        assert!(matches!(main[canvas_side], Node::Horizontal(_)));
        // Tarea 4: el lienzo, con el timeline debajo.
        assert_eq!(leaf(canvas_side.left().left()), [PanelId::Canvas]);
        assert_eq!(leaf(canvas_side.left().right()), [PanelId::Timeline]);
        let column = canvas_side.right();
        match &main[column] {
            Node::Vertical(split) => assert_eq!(split.fraction, 0.4),
            other => panic!("{other:?}"),
        }
        assert_eq!(leaf(column.left()), [PanelId::Color]);
        assert_eq!(leaf(column.right()), [PanelId::Groom]);
        assert_eq!(w.places(PanelId::Canvas), 1);
        // Lo que se guarda ahora es la versión de ahora (y se lee igual, sin volver a migrar).
        let again = w.to_saved();
        assert_eq!(again.version, SAVED_VERSION);
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
            vec![
                PanelId::Tools,
                PanelId::Canvas,
                PanelId::Timeline,
                PanelId::Color,
                PanelId::Groom
            ]
        );
        assert_ne!(w.leaf_of(PanelId::Color), w.leaf_of(PanelId::Groom));
        assert!(w.docked(PanelId::Color) && w.docked(PanelId::Groom));
        assert_eq!(w.state.iter_surfaces().count(), 1, "no windows left");
        // Y se puede seguir usando: acoplar, flotar y volver a la tira.
        let mut w = w;
        w.dock(PanelId::Library, DockTarget::Focused);
        assert_eq!(w.leaf_of(PanelId::Library), w.leaf_of(PanelId::Color));
        w.to_strip(PanelId::Groom);
        // (En el orden de los nodos del árbol que queda.)
        assert_eq!(
            w.state_tabs(),
            vec![
                PanelId::Tools,
                PanelId::Color,
                PanelId::Library,
                PanelId::Canvas,
                PanelId::Timeline
            ]
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
            w.state_tabs() == Workspace::essentials().state_tabs()
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
            left: Vec::new(),
            top: Vec::new(),
            bottom: Vec::new(),
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
        // El split de Color sobre Groom (la raíz es la de la toolbar; debajo, la del lienzo y la
        // columna).
        let column = NodeIndex::root().right().right();
        let fraction = |w: &Workspace| match &w.state.main_surface()[column] {
            Node::Vertical(split) => split.fraction,
            other => panic!("{other:?}"),
        };
        let mut demo = Workspace::demo("split").unwrap();
        for (n, f) in [(NodeIndex::root(), 0.25), (NodeIndex::root().right(), 0.75)] {
            if let Node::Horizontal(split) = &mut demo.state.main_surface_mut()[n] {
                split.fraction = f;
            }
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

    #[test]
    fn the_preview_is_out_of_the_tree_until_shown_and_then_floats_at_its_corner() {
        let mut w = Workspace::essentials();
        assert_eq!(w.places(PanelId::Preview), 0, "off by default");
        assert!(w.sync_panel(PanelId::Preview, true));
        assert!(w.floating(PanelId::Preview));
        assert!(
            w.preview_anchored,
            "at the corner of the window, like before"
        );
        assert!(!w.sync_panel(PanelId::Preview, true), "already shown");
        assert!(w.sync_panel(PanelId::Preview, false));
        assert_eq!(w.places(PanelId::Preview), 0);
        // Vuelve a su esquina.
        w.sync_panel(PanelId::Preview, true);
        assert!(w.floating(PanelId::Preview) && w.preview_anchored);
        // Movida (flotando en otro lugar o acoplada) ya no va anclada.
        w.float(PanelId::Preview, egui::pos2(300.0, 200.0));
        assert!(!w.preview_anchored);
        // Cada panel del workspace, una vez.
        for &id in PanelId::ALL {
            assert_eq!(w.places(id), 1, "{id:?}");
        }
    }

    #[test]
    fn a_hidden_preview_comes_back_where_it_was() {
        let mut w = Workspace::essentials();
        // Acoplada debajo de Color.
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Preview, DockTarget::Split(s, n, Split::Below));
        let before = w.state_tabs();
        w.sync_panel(PanelId::Preview, false);
        assert_eq!(w.places(PanelId::Preview), 0);
        w.sync_panel(PanelId::Preview, true);
        assert!(w.docked(PanelId::Preview));
        assert_eq!(w.state_tabs(), before);
        // En pestaña con Color.
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Preview, DockTarget::Leaf(s, n));
        w.sync_panel(PanelId::Preview, false);
        w.sync_panel(PanelId::Preview, true);
        assert_eq!(w.leaf_of(PanelId::Preview), w.leaf_of(PanelId::Color));
        // El timeline, igual que antes (los dos se recuerdan a la vez).
        w.sync_panel(PanelId::Timeline, false);
        w.sync_panel(PanelId::Preview, false);
        w.sync_panel(PanelId::Timeline, true);
        w.sync_panel(PanelId::Preview, true);
        assert!(w.timeline_below());
        assert_eq!(w.leaf_of(PanelId::Preview), w.leaf_of(PanelId::Color));
    }

    #[test]
    fn an_anchored_preview_is_not_saved_and_a_placed_one_is() {
        // Anclada a su esquina (la de fábrica): no se guarda, y al leerla vuelve a su esquina.
        let mut w = Workspace::essentials();
        w.sync_panel(PanelId::Preview, true);
        let saved = w.to_saved();
        assert_eq!(saved.version, SAVED_VERSION);
        assert_eq!(SAVED_VERSION, 6);
        let back = Workspace::from_saved(saved);
        assert_eq!(back.places(PanelId::Preview), 0);
        // Acoplada (y oculta), sí: en su lugar.
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Preview, DockTarget::Split(s, n, Split::Below));
        w.sync_panel(PanelId::Preview, false);
        let back = Workspace::from_saved(w.to_saved());
        assert!(back.docked(PanelId::Preview));
        assert!(!back.preview_anchored);
        // Flotando en otro lugar, también.
        let mut w = Workspace::essentials();
        w.float(PanelId::Preview, egui::pos2(300.0, 200.0));
        let back = Workspace::from_saved(w.to_saved());
        assert!(back.floating(PanelId::Preview));
    }

    #[test]
    fn a_version_4_workspace_has_no_preview_in_its_tree() {
        // En la 4 el Preview era su propia ventana: una llave "preview" en el árbol no puede venir
        // de ahí y se descarta (con `view.preview`, el workspace lo pone en su esquina).
        let mut w = Workspace::essentials();
        w.float(PanelId::Preview, egui::pos2(300.0, 200.0));
        let mut saved = w.to_saved();
        saved.version = 4;
        let back = Workspace::from_saved(saved);
        assert_eq!(back.places(PanelId::Preview), 0);
        assert_eq!(back.state_tabs(), Workspace::essentials().state_tabs());
    }

    // --- Fase D, tarea 6: tiras por orilla ---

    #[test]
    fn a_group_against_an_edge_collapses_to_that_edges_strip() {
        let mut w = Workspace::essentials();
        let edge = |w: &Workspace, id| {
            let (s, n) = w.leaf_of(id).unwrap();
            w.group_edge(s, n)
        };
        // La toolbar, pegada a la izquierda; Color a la derecha (y arriba y abajo: los lados van
        // primero).
        assert_eq!(edge(&w, PanelId::Tools), Edge::Left);
        assert_eq!(edge(&w, PanelId::Color), Edge::Right);
        // Groom a la izquierda de la toolbar: « lo manda a la tira de la izquierda.
        let (s, n) = w.leaf_of(PanelId::Tools).unwrap();
        w.dock(PanelId::Groom, DockTarget::Split(s, n, Split::Left));
        assert_eq!(edge(&w, PanelId::Groom), Edge::Left);
        let (s, n) = w.leaf_of(PanelId::Groom).unwrap();
        w.collapse_to_icons(s, n);
        assert_eq!(w.strip_at(Edge::Left), [PanelId::Groom]);
        assert!(!w.strip().contains(&PanelId::Groom));
        // Un grupo a lo ancho arriba de todo (toca la izquierda, la derecha y arriba): arriba.
        w.state
            .main_surface_mut()
            .split_above(NodeIndex::root(), 0.2, vec![PanelId::Lumen]);
        w.tidy();
        assert_eq!(edge(&w, PanelId::Lumen), Edge::Top);
        let (s, n) = w.leaf_of(PanelId::Lumen).unwrap();
        w.collapse_to_icons(s, n);
        assert_eq!(w.strip_at(Edge::Top), [PanelId::Lumen]);
        // Y abajo de todo: abajo.
        w.state
            .main_surface_mut()
            .split_below(NodeIndex::root(), 0.8, vec![PanelId::Slices]);
        w.tidy();
        assert_eq!(edge(&w, PanelId::Slices), Edge::Bottom);
        let (s, n) = w.leaf_of(PanelId::Slices).unwrap();
        w.collapse_to_icons(s, n);
        assert_eq!(w.strip_at(Edge::Bottom), [PanelId::Slices]);
        // La toolbar, a la de la izquierda (en el orden de siempre: antes que Groom).
        let (s, n) = w.leaf_of(PanelId::Tools).unwrap();
        w.collapse_to_icons(s, n);
        assert_eq!(w.strip_at(Edge::Left), [PanelId::Tools, PanelId::Groom]);
        // Sin rects (nada dibujado), lo que no toca ninguna orilla va a la derecha.
        let mut w = Workspace::essentials();
        let (s, n) = w.leaf_of(PanelId::Canvas).unwrap();
        w.dock(PanelId::Font, DockTarget::Split(s, n, Split::Below));
        assert_eq!(edge(&w, PanelId::Font), Edge::Right);
        // Cada panel, una vez.
        for &id in PanelId::ALL.iter().filter(|&&p| p != PanelId::Preview) {
            assert_eq!(w.places(id), 1, "{id:?}");
        }
    }

    #[test]
    fn each_strip_is_saved_and_the_strip_of_an_older_version_is_the_right_one() {
        let mut w = Workspace::essentials();
        w.to_strip_at(PanelId::Tools, Edge::Left);
        w.to_strip_at(PanelId::Groom, Edge::Bottom);
        w.to_strip_at(PanelId::Lumen, Edge::Top);
        let saved = w.to_saved();
        assert_eq!(saved.version, SAVED_VERSION);
        assert_eq!(SAVED_VERSION, 6);
        let ron = ron::to_string(&saved).unwrap();
        let back = Workspace::from_saved(ron::from_str(&ron).unwrap());
        for edge in Edge::ALL {
            assert_eq!(back.strip_at(edge), w.strip_at(edge), "{edge:?}");
        }
        assert_eq!(back.state_tabs(), w.state_tabs());
        // Una 5 (o anterior): su tira es la de la derecha (las otras no existían).
        let mut old = w.to_saved();
        old.version = 5;
        let back = Workspace::from_saved(old);
        for edge in [Edge::Left, Edge::Top, Edge::Bottom] {
            assert!(back.strip_at(edge).is_empty(), "{edge:?}");
        }
        for id in [PanelId::Tools, PanelId::Groom, PanelId::Lumen] {
            assert!(back.strip().contains(&id), "{id:?}");
        }
        // Lo que viene en dos tiras queda en una (la primera); lo que no viene en ninguna, a la
        // derecha.
        let mut twice = w.to_saved();
        twice.left.push("groom".into());
        twice.top.retain(|k| k != "lumen");
        let back = Workspace::from_saved(twice);
        assert_eq!(back.places(PanelId::Groom), 1);
        assert_eq!(back.strip_at(Edge::Left), [PanelId::Tools, PanelId::Groom]);
        assert!(back.strip().contains(&PanelId::Lumen));
        for &id in PanelId::ALL.iter().filter(|&&p| p != PanelId::Preview) {
            assert_eq!(back.places(id), 1, "{id:?}");
        }
    }

    #[test]
    fn dock_from_a_strip_puts_the_toolbar_back_at_its_edge() {
        let mut w = Workspace::essentials();
        let essentials = w.state_tabs();
        // De la tira de la izquierda: a su lugar de fábrica (a la izquierda de todo).
        w.to_strip_at(PanelId::Tools, Edge::Left);
        w.dock_from_strip(PanelId::Tools);
        assert_eq!(leaf_tabs(&w, NodeIndex::root().left()), [PanelId::Tools]);
        assert_eq!(w.tools_side(), Some(true));
        assert_eq!(w.state_tabs(), essentials);
        // De la de la derecha: a la derecha del lienzo.
        w.to_strip_at(PanelId::Tools, Edge::Right);
        w.dock_from_strip(PanelId::Tools);
        assert_eq!(w.tools_side(), Some(false));
        // Los demás, con el foco (al grupo de Color).
        w.dock_from_strip(PanelId::Library);
        assert_eq!(w.leaf_of(PanelId::Library), w.leaf_of(PanelId::Color));
        for &id in PanelId::ALL.iter().filter(|&&p| p != PanelId::Preview) {
            assert_eq!(w.places(id), 1, "{id:?}");
        }
    }
}
