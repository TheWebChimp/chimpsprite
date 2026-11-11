//! Lo mínimo de cada panel y de cada grupo ([`minima`], [`node_min`]) y cómo se ajustan a él los
//! splits ([`fit_splits`]) y las ventanas flotantes ([`fit_windows`]); también dónde se reabren
//! las ventanas de un acomodo leído ([`Workspace::place_restored_windows`]) y las nuevas
//! ([`Workspace::place_new_windows`]).
//!
//! El lienzo es una hoja más del árbol, protegida ([`tidy_canvas`]: una vez, en la superficie
//! principal y solo en su grupo), y lo que lo rodea va en puntos, no en fracciones
//! ([`pin_sides`]: al cambiar la ventana, el lienzo absorbe la diferencia).

use std::collections::HashMap;

use eframe::egui;
use egui_dock::{DockState, Node, Split, TabIndex, Tree};

use super::{NodeIndex, SurfaceIndex, Workspace, skin};
use crate::panel_registry::{Body, PanelId};
use crate::skin::Skin;
use crate::{dock, pixel_ui};

impl Workspace {
    /// Lo más angosta que puede quedar la columna derecha (el lado derecho del lienzo, ver
    /// [`Workspace::column`]): cada panel acoplado en ella en su mínimo (ver [`minima`]); los que
    /// van lado a lado, sumados. Sin columna, 0.
    pub(super) fn column_min(&self, m: &Minima, sep: f32, bar: f32) -> f32 {
        let tree = self.state.main_surface();
        match chain(tree).into_iter().find(|level| level.column) {
            Some(level) => node_min(tree, level.other, m, sep, bar).x,
            None => 0.0,
        }
    }

    /// El nodo de la columna derecha de la superficie principal: el lado derecho del lienzo que
    /// llega a la orilla derecha (el del primer split a lo ancho, de la raíz al lienzo, con el
    /// lienzo a la izquierda). Es lo que en la fase C era el panel derecho (`"palette"`).
    pub(crate) fn column(&self) -> Option<NodeIndex> {
        chain(self.state.main_surface())
            .into_iter()
            .find(|level| level.column)
            .map(|level| level.other)
    }

    /// El rect de la columna derecha en el último frame, con su divisor (desde el borde derecho
    /// del lado del lienzo): el que tenía el panel derecho de antes.
    pub(crate) fn column_rect(&self) -> Option<egui::Rect> {
        let tree = self.state.main_surface();
        let level = chain(tree).into_iter().find(|level| level.column)?;
        let parent = tree[level.parent].rect()?;
        let canvas_side = tree[level.parent.left()].rect()?;
        let rect = parent.with_min_x(canvas_side.right());
        (rect.is_positive() && rect.is_finite()).then_some(rect)
    }

    /// Los paneles de la columna derecha (ver [`Workspace::column`]).
    pub(crate) fn column_panels(&self) -> Vec<PanelId> {
        let tree = self.state.main_surface();
        self.column()
            .map(|n| panels_of(tree, n))
            .unwrap_or_default()
    }

    /// Los paneles cuyo grupo va pegado al divisor izquierdo de la columna derecha (lo que se
    /// llega bajando por la izquierda de sus splits a lo ancho): su margen izquierdo empieza en
    /// la línea, como el del panel derecho de antes (ver `show::Viewer`).
    pub(super) fn beside_column(&self) -> Vec<PanelId> {
        fn walk(tree: &Tree<PanelId>, n: NodeIndex, out: &mut Vec<PanelId>) {
            if n.0 >= tree.len() {
                return;
            }
            match &tree[n] {
                Node::Leaf(leaf) => out.extend(leaf.tabs.iter().copied()),
                Node::Horizontal(_) => walk(tree, n.left(), out),
                Node::Vertical(_) => {
                    walk(tree, n.left(), out);
                    walk(tree, n.right(), out);
                }
                Node::Empty => {}
            }
        }
        let mut out = Vec::new();
        if let Some(n) = self.column() {
            walk(self.state.main_surface(), n, &mut out);
        }
        out
    }

    /// Las ventanas que se acaban de abrir, en la esquina que se les pidió: egui no siempre la
    /// respeta en el primer frame de la ventana (el spike lo vio con una soltada en 528,132 que
    /// quedó en 350,100), así que se le vuelve a pedir unos frames ([`NEW_WINDOW_FRAMES`]).
    pub(super) fn place_new_windows(&mut self) {
        let state = &mut self.state;
        self.placing.retain_mut(|(id, at, frames)| {
            if let Some(path) = state.find_tab(id)
                && !path.surface.is_main()
                && let Some(window) = state.get_window_state_mut(path.surface)
            {
                window.set_position(*at);
            }
            *frames = frames.saturating_sub(1);
            *frames > 0
        });
    }

    /// Las ventanas de un acomodo leído, con lo de adentro donde estaba y de su tamaño. Cada frame
    /// hasta que se aprieta un botón del puntero: los primeros frames la escala y el estilo de
    /// Pixel UI todavía cambian (egui acomoda la ventana en una pantalla que no es la de después).
    /// La esquina y el tamaño que se le piden a la ventana son los de su área (con el marco): el
    /// marco se mide en el frame anterior (lo que su área tiene de más que lo de adentro).
    pub(super) fn place_restored_windows(&mut self, ctx: &egui::Context) {
        if self.placements.is_empty() {
            return;
        }
        if ctx.input(|i| i.pointer.any_pressed() || i.pointer.any_down()) {
            self.placements.clear();
            return;
        }
        for (surface, want) in self.placements.clone() {
            let area = ctx.memory(|m| m.area_rect(skin::window_area_id(surface)));
            let (at, size) = match (area, self.root_rect(surface)) {
                (Some(area), Some(now)) => (
                    area.min + (want.min - now.min),
                    want.size() + (area.size() - now.size()),
                ),
                _ => (want.min, want.size()),
            };
            if let Some(window) = self.state.get_window_state_mut(surface) {
                window.set_position(at.round());
                window.set_size(size.round());
            }
        }
    }
}

/// Lo mínimo de cada panel en un grupo del workspace, con el margen de su cuerpo (ver
/// [`minima`]).
pub(super) type Minima = HashMap<PanelId, egui::Vec2>;

/// Lo mínimo de cada panel en un grupo del workspace, con el margen de su cuerpo. Color, lo que
/// pide su panel con Pixel UI (el ancho de su fila más ancha y el alto del editor entero con la
/// paleta en su mínimo); sin Pixel UI, nada (como su panel de antes). El lienzo, el `min_width`
/// de su ficha (sin margen: su hoja va de borde a borde). Los demás, el `min_width`
/// y el `min_height` de su ficha con el margen de los paneles del dock y, si su cuerpo va con
/// scroll, la barra (así los acomoda [`Viewer`]).
pub(super) fn minima(ui: &egui::Ui, skin: Option<&Skin>) -> Minima {
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
                // El lienzo no tiene margen ni scroll: lo de su ficha (el workspace le suma lo
                // que lleva su hoja, ver `ChimpApp::canvas_area_min`).
                (PanelId::Canvas, _) => egui::vec2(id.spec().min_width, id.spec().min_height),
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
pub(super) fn chrome(ui: &egui::Ui, skin: Option<&Skin>) -> (f32, f32) {
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
pub(super) fn fit_splits(state: &mut DockState<PanelId>, m: &Minima, sep: f32, bar: f32) {
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
                // Si no alcanza, se queda con su mínimo el lado del lienzo (nunca cede: lleva la
                // tira y el timeline); si no, a lo alto, el lado que no se desplaza.
                let keep = if has_canvas(tree, node.left()) {
                    Some(true)
                } else if has_canvas(tree, node.right()) {
                    Some(false)
                } else if horizontal {
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

/// Cada ventana flotante de al menos lo mínimo de lo que lleva (ver [`node_min`]), sin pasar de
/// la pantalla, para que nada se recorte a lo ancho. egui_dock no deja ponerle un tamaño mínimo a
/// sus ventanas: como `skin::snap_windows` con su posición, se le pide el tamaño para el frame
/// siguiente. Con un botón apretado no: mientras se arrastra su borde puede quedar más chica, y
/// al soltarlo vuelve a su mínimo.
pub(super) fn fit_windows(ctx: &egui::Context, w: &mut Workspace, m: &Minima, sep: f32, bar: f32) {
    if ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    let screen = ctx.content_rect().size();
    let windows: Vec<SurfaceIndex> = w
        .state
        .iter_surfaces_indexed()
        .filter(|(s, surface)| !s.is_main() && !surface.is_empty())
        .map(|(s, _)| s)
        .collect();
    for s in windows {
        let area = ctx.memory(|mem| mem.area_rect(skin::window_area_id(s)));
        let (Some(area), Some(inner)) = (area, w.root_rect(s)) else {
            continue;
        };
        let need = node_min(&w.state[s], NodeIndex::root(), m, sep, bar);
        let grow = (need - inner.size()).max(egui::Vec2::ZERO);
        if grow.x < 0.5 && grow.y < 0.5 {
            continue;
        }
        let size = (area.size() + grow).ceil().min(screen.floor());
        if size != area.size()
            && let Some(window) = w.state.get_window_state_mut(s)
        {
            window.set_size(size);
        }
    }
}

/// Cuántos frames se le vuelve a pedir su esquina a una ventana nueva (ver
/// [`Workspace::place_new_windows`]).
pub(super) const NEW_WINDOW_FRAMES: u8 = 3;

// ---------------------------------------------------------------------------------------------
// El lienzo, protegido.

/// El cuadrante de `rect` en que cae `p` (como egui_dock en `resolve_traditional`: por las dos
/// diagonales).
pub(super) fn quadrant(rect: egui::Rect, p: egui::Pos2) -> Split {
    let a = (p - rect.center()) / rect.size().max(egui::Vec2::splat(1.0));
    match (a.x - a.y > 0.0, -a.x - a.y > 0.0) {
        (true, true) => Split::Above,
        (false, true) => Split::Left,
        (true, false) => Split::Right,
        (false, false) => Split::Below,
    }
}

/// La mitad de `rect` del lado `side`.
pub(super) fn half(rect: egui::Rect, side: Split) -> egui::Rect {
    let c = rect.center();
    match side {
        Split::Left => rect.with_max_x(c.x),
        Split::Right => rect.with_min_x(c.x),
        Split::Above => rect.with_max_y(c.y),
        Split::Below => rect.with_min_y(c.y),
    }
}

/// El lienzo está una vez, en la superficie principal y solo en su grupo (egui_dock no tiene hojas
/// protegidas: se deja que pase y se arregla el mismo frame). Lo que egui_dock le haya metido como
/// pestaña (soltado en su centro) se parte al lado, del lado del cuadrante donde se soltó
/// (`release`; sin él, a la derecha). Si estaba en una ventana o repetido, se queda el de la
/// superficie principal; si no está, vuelve partiendo la raíz (a la izquierda de lo que haya).
/// Devuelve si arregló algo.
pub(super) fn tidy_canvas(state: &mut DockState<PanelId>, release: Option<egui::Pos2>) -> bool {
    let mut changed = false;
    // Una sola vez, de preferencia el de la superficie principal.
    loop {
        let paths: Vec<_> = state
            .iter_all_tabs()
            .filter(|(_, t)| **t == PanelId::Canvas)
            .map(|(p, _)| p)
            .collect();
        let keep = paths.iter().position(|p| p.surface.is_main()).unwrap_or(0);
        let extra = paths
            .iter()
            .enumerate()
            .find(|&(i, p)| i != keep || !p.surface.is_main())
            .map(|(_, p)| *p);
        match extra {
            Some(path) => {
                state.remove_tab(path);
                changed = true;
            }
            None => break,
        }
    }
    if state.find_tab(&PanelId::Canvas).is_none() {
        let main = state.main_surface_mut();
        if main.num_tabs() == 0 {
            *main = Tree::new(vec![PanelId::Canvas]);
        } else {
            main.split(
                NodeIndex::root(),
                Split::Left,
                0.5,
                Node::leaf(PanelId::Canvas),
            );
        }
        changed = true;
    }
    // Solo en su grupo.
    let Some(path) = state.find_tab(&PanelId::Canvas) else {
        return changed;
    };
    let Ok(leaf) = state.leaf_mut(path.node_path()) else {
        return changed;
    };
    let rect = leaf.rect;
    let extras: Vec<PanelId> = leaf
        .tabs
        .iter()
        .copied()
        .filter(|&t| t != PanelId::Canvas)
        .collect();
    leaf.tabs.retain(|&t| t == PanelId::Canvas);
    if leaf.active != TabIndex(0) {
        leaf.active = TabIndex(0);
        changed = true;
    }
    for id in extras {
        let side = match release {
            Some(p) if rect.is_positive() => quadrant(rect, p),
            _ => Split::Right,
        };
        if let Some(at) = state.find_tab(&PanelId::Canvas) {
            state.split(at.node_path(), side, 0.5, Node::leaf(id));
        }
        changed = true;
    }
    changed
}

/// El panel que egui_dock no deja arrastrar: el lienzo. egui_dock no tiene "esta pestaña no se
/// arrastra", así que si egui empezó a arrastrar su franja de agarre (o su pestaña, que no lleva)
/// se suelta antes de que egui_dock lo vea (ver `skin::canvas_drag_ids`). Devuelve si lo soltó.
pub(super) fn block_canvas_drag(
    ctx: &egui::Context,
    dock_id: egui::Id,
    state: &DockState<PanelId>,
) -> bool {
    let Some(path) = state.find_tab(&PanelId::Canvas) else {
        return false;
    };
    let ids = skin::canvas_drag_ids(dock_id, path);
    if ctx.dragged_id().is_some_and(|d| ids.contains(&d)) {
        ctx.stop_dragging();
        return true;
    }
    false
}

// ---------------------------------------------------------------------------------------------
// Los lados del lienzo, en puntos.

/// Si el lienzo cuelga del nodo `node` de `tree`.
fn has_canvas(tree: &Tree<PanelId>, node: NodeIndex) -> bool {
    panels_of(tree, node).contains(&PanelId::Canvas)
}

/// Lo que mide en puntos cada lado del lienzo. egui_dock reparte cada split en fracciones, así
/// que al cambiar la ventana (o lo acoplado) los lados crecerían y se encogerían con ella: cada
/// split del camino de la raíz al lienzo deja su lado sin lienzo de lo que mide aquí y el lienzo
/// se queda con el resto (ver [`pin_sides`]). Cada lado se mide del borde de afuera de su split
/// al centro de su divisor.
///
/// Lo que mide un lado solo cambia cuando el usuario arrastra su divisor ([`record_sides`]): lo
/// que lo aprieta en una ventana chica (`fit_splits`) no se le queda, y al volver a agrandarla
/// regresa a sus puntos (el spike perdía 11 así). No se guarda: un acomodo leído toma lo de cada
/// lado de su fracción y del rect guardado.
#[derive(Clone, Debug, Default)]
pub(crate) struct SidePoints {
    /// Lo que mide cada lado que se conoce, por su identidad ([`Side`]): los índices del árbol se
    /// mueven.
    sides: Vec<(Side, f32)>,
    /// Lo que mide la columna derecha (ver [`Workspace::column`]): la pide el anfitrión (de
    /// `view.palette_width`); sin ella, como los demás lados.
    pub column: Option<f32>,
    /// El ancho de los divisores (el lado del lienzo de cada split empieza medio divisor después
    /// de su centro).
    pub sep: f32,
    /// Lo mínimo de cada panel (ver [`minima`]; el del lienzo, lo que lleva su hoja este frame) y
    /// el alto de una barra de pestañas: ningún lado le quita al lado del lienzo su mínimo (sin
    /// ellos, nada se limita).
    pub mins: Minima,
    pub bar: f32,
    /// Con qué se acomodaron por última vez: el área, la forma del árbol y la columna.
    last: Option<(egui::Rect, Shape, Option<f32>)>,
}

impl SidePoints {
    fn get(&self, side: &Side) -> Option<f32> {
        self.sides.iter().find(|(s, _)| s == side).map(|(_, p)| *p)
    }

    /// Lo más que puede medir el lado `level` en un split de `len` (a su eje): lo que deja el
    /// lado del lienzo en su mínimo (el lienzo gana; el lado se aprieta o se desplaza).
    fn room(&self, tree: &Tree<PanelId>, level: &Level, len: f32) -> f32 {
        let canvas_side = if level.first {
            level.parent.right()
        } else {
            level.parent.left()
        };
        let min = node_min(tree, canvas_side, &self.mins, self.sep, self.bar);
        let min = if level.horizontal { min.x } else { min.y };
        (len - min - self.sep / 2.0).max(0.0)
    }

    fn set(&mut self, side: Side, points: f32) {
        match self.sides.iter_mut().find(|(s, _)| *s == side) {
            Some((_, p)) => *p = points,
            None => self.sides.push((side, points)),
        }
    }
}

/// La identidad de un lado del lienzo: la orientación de su split, si va antes del lienzo
/// (izquierda o arriba) y los paneles que lleva.
#[derive(Clone, Debug, PartialEq)]
struct Side {
    horizontal: bool,
    first: bool,
    panels: Vec<PanelId>,
}

/// La forma de la superficie principal: cada nodo (su clase y sus paneles), sin rects ni
/// fracciones.
type Shape = Vec<(u8, Vec<PanelId>)>;

fn shape(tree: &Tree<PanelId>) -> Shape {
    tree.iter()
        .map(|node| match node {
            Node::Empty => (0, Vec::new()),
            Node::Leaf(leaf) => (1, leaf.tabs.clone()),
            Node::Horizontal(_) => (2, Vec::new()),
            Node::Vertical(_) => (3, Vec::new()),
        })
        .collect()
}

/// Un split del camino de la raíz al lienzo.
#[derive(Clone, Copy, Debug)]
struct Level {
    /// El split.
    parent: NodeIndex,
    /// Su lado sin el lienzo.
    other: NodeIndex,
    horizontal: bool,
    /// Si `other` va antes que el lienzo (a la izquierda o arriba).
    first: bool,
    /// Si `other` es la columna derecha (ver [`Workspace::column`]).
    column: bool,
}

/// Los splits del camino de la raíz al lienzo de `tree`, de arriba abajo.
fn chain(tree: &Tree<PanelId>) -> Vec<Level> {
    let Some((canvas, _)) = tree.find_tab(&PanelId::Canvas) else {
        return Vec::new();
    };
    let mut up = Vec::new();
    let mut n = canvas;
    while let Some(p) = n.parent() {
        up.push((p, n));
        n = p;
    }
    let mut column = false;
    up.into_iter()
        .rev()
        .filter_map(|(parent, child)| {
            let horizontal = match &tree[parent] {
                Node::Horizontal(_) => true,
                Node::Vertical(_) => false,
                _ => return None,
            };
            let first = child.is_right();
            let is_column = horizontal && !first && !column;
            column |= is_column;
            Some(Level {
                parent,
                other: if first { parent.left() } else { parent.right() },
                horizontal,
                first,
                column: is_column,
            })
        })
        .collect()
}

/// Los paneles del nodo `node` de `tree` y de lo que cuelga de él.
fn panels_of(tree: &Tree<PanelId>, node: NodeIndex) -> Vec<PanelId> {
    if node.0 >= tree.len() {
        return Vec::new();
    }
    match &tree[node] {
        Node::Empty => Vec::new(),
        Node::Leaf(leaf) => leaf.tabs.clone(),
        Node::Horizontal(_) | Node::Vertical(_) => {
            let mut out = panels_of(tree, node.left());
            out.extend(panels_of(tree, node.right()));
            out
        }
    }
}

/// Lo que pide de fábrica el nodo `node` a lo ancho (`horizontal`) o a lo alto: un grupo, lo de
/// su panel más grande (Color, el ancho de fábrica de la columna; los del dock, su ancho de
/// referencia con su margen y la barra de scroll); un split, sus dos lados juntos.
fn reference(tree: &Tree<PanelId>, node: NodeIndex, horizontal: bool) -> f32 {
    if node.0 >= tree.len() {
        return 0.0;
    }
    let of = |id: PanelId| {
        let spec = id.spec();
        if !horizontal {
            return spec.min_height.max(super::model::FLOAT_HEIGHT);
        }
        match id {
            PanelId::Color => crate::color_panel::DEFAULT_WIDTH,
            _ => {
                let bar = match spec.body {
                    Body::Fit | Body::Tall => pixel_ui::SCROLL_BAR,
                    Body::Fixed => 0.0,
                };
                spec.reference_width + 2.0 * f32::from(dock::FLYOUT_MARGIN) + bar
            }
        }
    };
    match &tree[node] {
        Node::Empty => 0.0,
        Node::Leaf(leaf) => leaf.tabs.iter().map(|&t| of(t)).fold(0.0, f32::max),
        Node::Horizontal(_) | Node::Vertical(_) => {
            let (a, b) = (
                reference(tree, node.left(), horizontal),
                reference(tree, node.right(), horizontal),
            );
            let along = matches!(tree[node], Node::Horizontal(_)) == horizontal;
            if along { a + b } else { a.max(b) }
        }
    }
}

/// El split `node` (a lo ancho o a lo alto) de `tree`: su rect y su fracción.
fn split_of(tree: &Tree<PanelId>, node: NodeIndex) -> Option<(egui::Rect, f32)> {
    match &tree[node] {
        Node::Horizontal(s) | Node::Vertical(s) => Some((s.rect, s.fraction)),
        _ => None,
    }
}

/// Lo que mide el lado `level` en el último frame (del borde de afuera de su split al centro del
/// divisor), si el split ya se dibujó.
fn measured(tree: &Tree<PanelId>, level: &Level) -> Option<f32> {
    let (rect, f) = split_of(tree, level.parent)?;
    if !(rect.is_positive() && rect.is_finite()) {
        return None;
    }
    let len = if level.horizontal {
        rect.width()
    } else {
        rect.height()
    };
    Some(if level.first {
        len * f
    } else {
        len * (1.0 - f)
    })
}

/// Deja cada lado del lienzo de la superficie principal de lo que mide en `sides` (los que no
/// conoce, de lo que miden en el último frame o, si son nuevos, de lo que piden de fábrica, a lo
/// más el 45 % para que el lienzo no desaparezca) y el lienzo con lo demás de `area`. Solo si
/// cambió algo desde la última vez (el área, la forma del árbol o la columna pedida): si no, se
/// comería el arrastre de un divisor y lo que `fit_splits` ajusta.
pub(super) fn pin_sides(state: &mut DockState<PanelId>, area: egui::Rect, sides: &mut SidePoints) {
    let tree = state.main_surface_mut();
    let now = (area, shape(tree), sides.column);
    let same_column = |a: Option<f32>, b: Option<f32>| match (a, b) {
        (Some(a), Some(b)) => (a - b).abs() < 0.01,
        (a, b) => a == b,
    };
    if let Some((area, shape, column)) = &sides.last
        && *area == now.0
        && *shape == now.1
        && same_column(*column, now.2)
    {
        return;
    }
    let mut rect = area;
    let mut alive = Vec::new();
    for level in chain(tree) {
        let len = if level.horizontal {
            rect.width()
        } else {
            rect.height()
        };
        if !(len > 1.0 && len.is_finite()) {
            break;
        }
        let side = Side {
            horizontal: level.horizontal,
            first: level.first,
            panels: panels_of(tree, level.other),
        };
        alive.push(side.clone());
        let points = match (level.column, sides.column) {
            (true, Some(points)) => points,
            _ => match sides.get(&side) {
                Some(points) => points,
                None => {
                    let fresh = || {
                        let r = reference(tree, level.other, level.horizontal) + sides.sep / 2.0;
                        r.min(len * 0.45)
                    };
                    let points = measured(tree, &level).unwrap_or_else(fresh);
                    sides.set(side, points);
                    points
                }
            },
        }
        // Lo guardado no cambia: si ahora no cabe, se aprieta solo mientras no quepa.
        .clamp(0.0, sides.room(tree, &level, len));
        let f = if level.first {
            points / len
        } else {
            1.0 - points / len
        };
        if let Node::Horizontal(s) | Node::Vertical(s) = &mut tree[level.parent] {
            s.fraction = f.clamp(0.0, 1.0);
        }
        // El lado del lienzo, para el split de adentro.
        let half = sides.sep / 2.0;
        rect = match (level.horizontal, level.first) {
            (true, true) => rect.with_min_x(rect.left() + len * f + half),
            (true, false) => rect.with_max_x(rect.left() + len * f - half),
            (false, true) => rect.with_min_y(rect.top() + len * f + half),
            (false, false) => rect.with_max_y(rect.top() + len * f - half),
        };
    }
    // Los lados que ya no existen se olvidan.
    sides.sides.retain(|(side, _)| alive.contains(side));
    sides.last = Some(now);
}

/// Las fracciones de la superficie principal y su forma antes del `DockArea`, para ver después
/// qué divisor arrastró el usuario ([`record_sides`]).
pub(super) struct Fractions {
    shape: Shape,
    of: Vec<(NodeIndex, f32)>,
}

pub(super) fn fractions(state: &DockState<PanelId>) -> Fractions {
    let tree = state.main_surface();
    Fractions {
        shape: shape(tree),
        of: (0..tree.len())
            .map(NodeIndex)
            .filter_map(|n| split_of(tree, n).map(|(_, f)| (n, f)))
            .collect(),
    }
}

/// Lo que el usuario cambió arrastrando (o con el teclado) un divisor del camino al lienzo en el
/// `DockArea` de este frame (egui_dock es el único que mueve las fracciones mientras se dibuja):
/// es lo que ese lado mide de ahí en adelante, sin quitarle al lienzo su mínimo. Devuelve lo nuevo
/// de la columna derecha, si fue la suya (el anfitrión lo guarda como su preferencia).
///
/// El doble clic en un divisor (egui_dock lo pone a la mitad) no es una elección: se deshace.
pub(super) fn record_sides(
    state: &mut DockState<PanelId>,
    before: &Fractions,
    sides: &mut SidePoints,
) -> Option<f32> {
    let tree = state.main_surface_mut();
    if shape(tree) != before.shape {
        return None;
    }
    let mut column = None;
    for level in chain(tree) {
        let Some((rect, f)) = split_of(tree, level.parent) else {
            continue;
        };
        let Some(&(_, was)) = before.of.iter().find(|(n, _)| *n == level.parent) else {
            continue;
        };
        if was == f {
            continue;
        }
        if f == 0.5 {
            if let Node::Horizontal(s) | Node::Vertical(s) = &mut tree[level.parent] {
                s.fraction = was;
            }
            continue;
        }
        let Some(points) = measured(tree, &level) else {
            continue;
        };
        let len = if level.horizontal {
            rect.width()
        } else {
            rect.height()
        };
        let points = points.min(sides.room(tree, &level, len));
        let side = Side {
            horizontal: level.horizontal,
            first: level.first,
            panels: panels_of(tree, level.other),
        };
        sides.set(side, points);
        if level.column {
            column = Some(points);
            // El anfitrión la va a pedir así: no hay que volver a acomodarla (de una fracción a
            // puntos y de vuelta, un arrastre lento se correría medio punto por frame).
            if let Some(last) = &mut sides.last {
                last.2 = Some(points);
            }
        }
    }
    column
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::*;
    use crate::ChimpApp;
    use crate::workspace::test_util::{frame, frame_in, frames, test_app};
    use crate::workspace::{DockTarget, Split};

    // --- El lienzo, protegido (`tidy_canvas`) ---

    /// Cuántas veces está el lienzo en `state` y si está solo en su grupo de la superficie
    /// principal.
    fn canvas_ok(state: &DockState<PanelId>) -> bool {
        let all: Vec<_> = state
            .iter_all_tabs()
            .filter(|(_, t)| **t == PanelId::Canvas)
            .map(|(p, _)| p)
            .collect();
        all.len() == 1
            && all[0].surface.is_main()
            && state
                .leaf(all[0].node_path())
                .is_ok_and(|leaf| leaf.tabs == [PanelId::Canvas])
    }

    /// Los rects de cada grupo de la superficie principal en `area` (sin divisores).
    fn laid_out(w: &mut Workspace, area: egui::Rect) {
        w.layout_for_test(area);
    }

    #[test]
    fn a_panel_dropped_as_a_tab_of_the_canvas_is_split_beside_it_on_the_drop_side() {
        let area = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 600.0));
        for (at, side) in [
            (egui::pos2(100.0, 300.0), Split::Left),
            (egui::pos2(700.0, 300.0), Split::Right),
            (egui::pos2(390.0, 40.0), Split::Above),
            (egui::pos2(390.0, 560.0), Split::Below),
        ] {
            let mut w = Workspace::essentials();
            laid_out(&mut w, area);
            // Lo que hace egui_dock al soltar Library en el centro del lienzo: su pestaña.
            let (s, n) = w.leaf_of(PanelId::Canvas).unwrap();
            w.strip.retain(|&p| p != PanelId::Library);
            w.state
                .leaf_mut(egui_dock::NodePath {
                    surface: s,
                    node: n,
                })
                .unwrap()
                .append_tab(PanelId::Library);
            tidy_canvas(&mut w.state, Some(at));
            assert!(canvas_ok(&w.state), "{side:?}");
            assert!(w.docked(PanelId::Library), "{side:?}");
            assert_ne!(w.leaf_of(PanelId::Library), w.leaf_of(PanelId::Canvas));
            // Del lado del cuadrante donde se soltó (con el lienzo de 0..780 a lo ancho).
            laid_out(&mut w, area);
            let lib = w.leaf_rect(PanelId::Library).unwrap();
            let canvas = w.leaf_rect(PanelId::Canvas).unwrap();
            let ok = match side {
                Split::Left => lib.right() <= canvas.left(),
                Split::Right => lib.left() >= canvas.right(),
                Split::Above => lib.bottom() <= canvas.top(),
                Split::Below => lib.top() >= canvas.bottom(),
            };
            assert!(ok, "{side:?}: {lib:?} vs {canvas:?}");
            assert_eq!(w.places(PanelId::Library), 1);
        }
    }

    #[test]
    fn the_canvas_goes_back_to_the_main_surface_once() {
        // En una ventana (como si egui_dock lo hubiera flotado).
        let mut w = Workspace::essentials();
        let path = w.state.find_tab(&PanelId::Canvas).unwrap();
        w.state.remove_tab(path);
        w.state.add_window(vec![PanelId::Canvas]);
        assert!(!canvas_ok(&w.state));
        tidy_canvas(&mut w.state, None);
        assert!(canvas_ok(&w.state));
        assert_eq!(w.state.iter_surfaces().count(), 1, "the window went away");
        assert!(w.docked(PanelId::Color));
        // Dos veces.
        let mut w = Workspace::essentials();
        w.state.add_window(vec![PanelId::Canvas]);
        tidy_canvas(&mut w.state, None);
        assert!(canvas_ok(&w.state));
        // Sin lienzo (lo guardado no lo trae): de vuelta, una vez, junto a lo que hay.
        let mut w = Workspace::essentials();
        let path = w.state.find_tab(&PanelId::Canvas).unwrap();
        w.state.remove_tab(path);
        tidy_canvas(&mut w.state, None);
        assert!(canvas_ok(&w.state));
        assert!(w.docked(PanelId::Color));
        tidy_canvas(&mut w.state, None);
        assert_eq!(w.places(PanelId::Canvas), 1);
        // Con la superficie principal vacía (Color flotando, sin nada más): el lienzo solo.
        let mut w = Workspace::essentials();
        w.float(PanelId::Color, egui::pos2(100.0, 100.0));
        let path = w.state.find_tab(&PanelId::Canvas).unwrap();
        w.state.remove_tab(path);
        tidy_canvas(&mut w.state, None);
        assert!(canvas_ok(&w.state));
        assert!(w.floating(PanelId::Color));
    }

    // --- Los lados en puntos (`pin_sides`) ---

    /// La distancia de la orilla de afuera de cada lado del split `node` (horizontal) a su divisor.
    fn left_points(w: &Workspace, node: NodeIndex) -> f32 {
        match &w.state.main_surface()[node] {
            Node::Horizontal(s) => s.rect.width() * s.fraction,
            other => panic!("{other:?}"),
        }
    }

    fn right_points(w: &Workspace, node: NodeIndex) -> f32 {
        match &w.state.main_surface()[node] {
            Node::Horizontal(s) => s.rect.width() * (1.0 - s.fraction),
            other => panic!("{other:?}"),
        }
    }

    /// El split del que cuelga el grupo de `id` en la superficie principal.
    fn split_of_panel(w: &Workspace, id: PanelId) -> NodeIndex {
        let (_, leaf) = w.leaf_of(id).unwrap();
        leaf.parent().unwrap()
    }

    /// Groom a la izquierda del lienzo y Color a su derecha: `H[H[Groom | Canvas] | Color]`.
    fn groom_left_of_the_canvas() -> Workspace {
        let mut w = Workspace::essentials();
        let (s, n) = w.leaf_of(PanelId::Canvas).unwrap();
        w.dock(PanelId::Groom, DockTarget::Split(s, n, Split::Left));
        w
    }

    fn area(width: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(38.0, 63.0), egui::vec2(width, 700.0))
    }

    #[test]
    fn the_sides_keep_their_points_when_the_window_shrinks_and_grows_back() {
        let mut w = groom_left_of_the_canvas();
        let mut sides = SidePoints::default();
        // El primer acomodo: cada lado de lo que pide su ficha (lo nuevo, a lo más 45 %).
        pin_sides(&mut w.state, area(1000.0), &mut sides);
        laid_out(&mut w, area(1000.0));
        let root = split_of_panel(&w, PanelId::Groom);
        let inner = split_of_panel(&w, PanelId::Color);
        let groom = left_points(&w, root);
        let color = right_points(&w, inner);
        // Color, el ancho de fábrica de la columna; Groom, lo que pide (a lo más el 45 % de lo que
        // queda: 780).
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(near(color, crate::color_panel::DEFAULT_WIDTH), "{color}");
        assert!(near(groom, 0.45 * 780.0), "{groom}");
        // Más angosta: los lados igual, el lienzo absorbe la diferencia.
        pin_sides(&mut w.state, area(800.0), &mut sides);
        laid_out(&mut w, area(800.0));
        assert!(near(left_points(&w, root), groom));
        assert!(near(right_points(&w, inner), color));
        let canvas = w.leaf_rect(PanelId::Canvas).unwrap();
        assert!(near(canvas.width(), 800.0 - groom - color), "{canvas:?}");
        // Si algo los apretó en esa ventana (como `fit_splits` cuando no alcanza), al volver al
        // ancho de antes regresan a sus puntos: no se pierde ninguno (el spike perdía 11).
        if let Node::Horizontal(s) = &mut w.state.main_surface_mut()[root] {
            s.fraction = 233.0 / 580.0;
        }
        laid_out(&mut w, area(800.0));
        pin_sides(&mut w.state, area(800.0), &mut sides);
        laid_out(&mut w, area(800.0));
        assert!(
            near(left_points(&w, root), 233.0),
            "a steady frame leaves it alone"
        );
        pin_sides(&mut w.state, area(1000.0), &mut sides);
        laid_out(&mut w, area(1000.0));
        assert!(near(left_points(&w, root), groom));
        assert!(near(right_points(&w, inner), color));
    }

    #[test]
    fn a_dragged_divider_is_the_sides_new_size_and_the_host_sets_the_columns() {
        let mut w = groom_left_of_the_canvas();
        let mut sides = SidePoints::default();
        pin_sides(&mut w.state, area(1000.0), &mut sides);
        laid_out(&mut w, area(1000.0));
        let root = split_of_panel(&w, PanelId::Groom);
        let inner = split_of_panel(&w, PanelId::Color);
        // El usuario arrastra el divisor de Groom 30 puntos a la derecha (egui_dock cambia la
        // fracción): eso es lo que mide de ahí en adelante.
        let before = fractions(&w.state);
        if let Node::Horizontal(s) = &mut w.state.main_surface_mut()[root] {
            s.fraction += 30.0 / s.rect.width();
        }
        assert_eq!(
            record_sides(&mut w.state, &before, &mut sides),
            None,
            "not the column"
        );
        let groom = left_points(&w, root);
        pin_sides(&mut w.state, area(900.0), &mut sides);
        laid_out(&mut w, area(900.0));
        assert!((left_points(&w, root) - groom).abs() < 1e-3);
        // La columna derecha (pegada a la orilla derecha) la pide el anfitrión.
        sides.column = Some(300.0);
        pin_sides(&mut w.state, area(900.0), &mut sides);
        laid_out(&mut w, area(900.0));
        assert!((right_points(&w, inner) - 300.0).abs() < 1e-3);
        // Y su divisor arrastrado se le reporta.
        let before = fractions(&w.state);
        if let Node::Horizontal(s) = &mut w.state.main_surface_mut()[inner] {
            s.fraction -= 40.0 / s.rect.width();
        }
        let column = record_sides(&mut w.state, &before, &mut sides).expect("the column moved");
        assert!((column - 340.0).abs() < 1e-3, "{column}");
    }

    #[test]
    fn no_side_takes_the_canvas_minimum_and_it_gets_its_points_back_later() {
        let mut w = groom_left_of_the_canvas();
        let mut sides = SidePoints::default();
        // El lienzo pide 300 (lo que lleva su hoja); los demás, nada.
        sides.mins.insert(PanelId::Canvas, egui::vec2(300.0, 0.0));
        pin_sides(&mut w.state, area(1000.0), &mut sides);
        laid_out(&mut w, area(1000.0));
        let root = split_of_panel(&w, PanelId::Groom);
        let groom = left_points(&w, root);
        assert!((groom - 0.45 * 780.0).abs() < 1e-3, "{groom}");
        // En 600 no caben Groom (351), Color (220) y el lienzo (300): el lienzo gana.
        pin_sides(&mut w.state, area(600.0), &mut sides);
        laid_out(&mut w, area(600.0));
        let canvas = w.leaf_rect(PanelId::Canvas).unwrap();
        assert!(canvas.width() >= 300.0 - 1e-3, "{canvas:?}");
        assert!((left_points(&w, root) - 80.0).abs() < 1e-3);
        // Al volver, Groom recupera lo suyo (lo guardado no cambió).
        pin_sides(&mut w.state, area(1000.0), &mut sides);
        laid_out(&mut w, area(1000.0));
        assert!((left_points(&w, root) - groom).abs() < 1e-3);
        // Un arrastre que se come el lienzo se guarda hasta donde deja su mínimo.
        let before = fractions(&w.state);
        if let Node::Horizontal(s) = &mut w.state.main_surface_mut()[root] {
            s.fraction = 0.95;
        }
        record_sides(&mut w.state, &before, &mut sides);
        pin_sides(&mut w.state, area(900.0), &mut sides);
        laid_out(&mut w, area(900.0));
        let canvas = w.leaf_rect(PanelId::Canvas).unwrap();
        assert!(canvas.width() >= 300.0 - 1e-3, "{canvas:?}");
    }

    #[test]
    fn a_double_click_on_a_canvas_divider_does_not_count_as_a_choice() {
        let mut w = groom_left_of_the_canvas();
        let mut sides = SidePoints {
            column: Some(263.0),
            ..Default::default()
        };
        pin_sides(&mut w.state, area(1000.0), &mut sides);
        laid_out(&mut w, area(1000.0));
        let root = NodeIndex::root();
        let was = match &w.state.main_surface()[root] {
            Node::Horizontal(s) => s.fraction,
            other => panic!("{other:?}"),
        };
        // egui_dock, con el doble clic en el divisor de la columna: a la mitad.
        let before = fractions(&w.state);
        if let Node::Horizontal(s) = &mut w.state.main_surface_mut()[root] {
            s.fraction = 0.5;
        }
        assert_eq!(record_sides(&mut w.state, &before, &mut sides), None);
        match &w.state.main_surface()[root] {
            Node::Horizontal(s) => assert_eq!(s.fraction, was),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_sides_that_go_away_are_forgotten() {
        let mut w = groom_left_of_the_canvas();
        let mut sides = SidePoints::default();
        pin_sides(&mut w.state, area(1000.0), &mut sides);
        assert_eq!(sides.sides.len(), 2, "{:?}", sides.sides);
        w.to_strip(PanelId::Groom);
        pin_sides(&mut w.state, area(1000.0), &mut sides);
        assert_eq!(sides.sides.len(), 1, "{:?}", sides.sides);
        assert_eq!(sides.sides[0].0.panels, [PanelId::Color]);
    }

    #[test]
    fn with_color_floating_its_minimum_does_not_count_for_the_column() {
        // Lo que la columna derecha pide es lo de sus paneles: Color flotando no cuenta (en su
        // ventana), aunque sea más que el de lo acoplado (aquí, un Library de 50 de mentiras: el
        // de verdad es más ancho que Color).
        let mut w = Workspace::essentials();
        w.float(PanelId::Color, egui::pos2(100.0, 100.0));
        w.dock(PanelId::Library, DockTarget::Focused);
        let mut m = Minima::new();
        m.insert(PanelId::Color, egui::vec2(137.0, 400.0));
        m.insert(PanelId::Library, egui::vec2(50.0, 0.0));
        assert_eq!(w.column_min(&m, 0.0, 0.0), 50.0);
        // Con Color acoplado en la columna, el suyo sí.
        w.dock(PanelId::Color, DockTarget::Focused);
        assert_eq!(w.column_min(&m, 0.0, 0.0), 137.0);
    }

    // --- Lo de antes ---

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

    /// Lo mínimo de lo de adentro de la ventana `surface` (con Pixel UI).
    fn window_min(ctx: &egui::Context, app: &ChimpApp, surface: SurfaceIndex) -> egui::Vec2 {
        let mut out = egui::Vec2::ZERO;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let skin = app.pixel.skin();
            let (sep, bar) = chrome(ui, skin);
            let tree = &app.workspace.state[surface];
            out = node_min(tree, NodeIndex::root(), &minima(ui, skin), sep, bar);
        });
        output.textures_delta.clear();
        out
    }

    #[test]
    fn a_floating_window_never_gets_smaller_than_its_contents_minimum() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        app.workspace = Workspace::demo("float").unwrap();
        frames(&ctx, &mut app);
        let (surface, _) = app.workspace.leaf_of(PanelId::Lumen).unwrap();
        // Como si se achicara arrastrando su borde (y se soltara).
        app.workspace
            .state
            .get_window_state_mut(surface)
            .unwrap()
            .set_size(egui::vec2(120.0, 150.0));
        for _ in 0..4 {
            frame(&ctx, &mut app, Vec::new());
        }
        let min = window_min(&ctx, &app, surface);
        let inner = app.workspace.root_rect(surface).expect("Lumen's window");
        assert!(min.x > 120.0 && min.y > 150.0, "{min:?}");
        assert!(inner.width() >= min.x, "{inner:?} vs {min:?}");
        assert!(inner.height() >= min.y, "{inner:?} vs {min:?}");
        // En una pantalla más chica que su mínimo, cabe en la pantalla.
        app.workspace
            .state
            .get_window_state_mut(surface)
            .unwrap()
            .set_size(egui::vec2(120.0, 150.0));
        for _ in 0..6 {
            frame_in(&ctx, &mut app, Vec::new(), egui::vec2(400.0, 300.0));
        }
        let rect = ctx
            .memory(|m| m.area_rect(skin::window_area_id(surface)))
            .expect("Lumen's window");
        assert!(ctx.content_rect().contains_rect(rect), "{rect:?}");
        assert!(rect.width() > 200.0, "{rect:?}");
    }
}
