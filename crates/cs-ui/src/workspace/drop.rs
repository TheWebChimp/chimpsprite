//! Dónde cae un panel que se suelta ([`drop_target`]), cómo se resalta mientras se arrastra
//! ([`paint_drop_zone`]) y la pestaña de egui_dock que se suelta fuera de los grupos
//! ([`tab_drag_released`]: el workspace la pone a flotar donde cayó).

use eframe::egui;
use egui_dock::{DockState, NodePath};
#[cfg(test)]
use egui_dock::{Node, Tree};

use super::{DockTarget, NodeIndex, Split, SurfaceIndex, Workspace, dock_id, skin};
use crate::panel_registry::PanelId;
use crate::skin::Skin;

/// Dónde cae un panel que se suelta (ver [`drop_target`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DropTarget {
    /// Como pestaña del grupo `(superficie, nodo)`: el puntero en su centro.
    Tab(SurfaceIndex, NodeIndex),
    /// Partiendo el grupo: el puntero en la franja de un borde ([`EDGE_ZONE`] de su lado); el
    /// panel va de ese lado.
    Split(SurfaceIndex, NodeIndex, Split),
    /// En una ventana nueva con la esquina en el puntero.
    Float(egui::Pos2),
}

/// La franja de cada borde de un grupo donde soltar un panel lo parte: 1/4 de su ancho (a los
/// lados) o de su alto (arriba y abajo).
const EDGE_ZONE: f32 = 0.25;

/// Dónde cae un panel soltado con el puntero en `pointer`, con los rects de los grupos del frame
/// pasado: sobre una ventana flotante, en su grupo; sobre la columna acoplada (`dock_rect`), en
/// el grupo bajo el puntero (o el más cercano, si cae en un divisor); en el centro de un grupo,
/// como pestaña, y en la franja de un borde, partiéndolo. En cualquier otro lado, flotando.
pub(crate) fn drop_target(w: &Workspace, pointer: egui::Pos2, dock_rect: egui::Rect) -> DropTarget {
    let leaves: Vec<(NodePath, egui::Rect)> = w
        .state
        .iter_leaves()
        .filter(|(_, leaf)| !leaf.tabs.is_empty() && leaf.rect.is_positive())
        .map(|(path, leaf)| (path, leaf.rect))
        .collect();
    // Las ventanas van encima de la columna y del lienzo.
    let window = leaves
        .iter()
        .find(|(path, rect)| !path.surface.is_main() && rect.contains(pointer));
    let docked = || {
        leaves
            .iter()
            .filter(|(path, _)| path.surface.is_main())
            .min_by(|a, b| {
                let d = |r: &egui::Rect| r.distance_sq_to_pos(pointer);
                d(&a.1).total_cmp(&d(&b.1))
            })
    };
    let hit = match window {
        Some(hit) => Some(hit),
        None if dock_rect.contains(pointer) => match docked() {
            Some(hit) => Some(hit),
            // La columna vacía (Color se fue a una ventana): a la superficie principal.
            None => return DropTarget::Tab(SurfaceIndex::main(), NodeIndex::root()),
        },
        None => None,
    };
    match hit {
        Some(&(path, rect)) => match edge_of(rect, pointer) {
            Some(side) => DropTarget::Split(path.surface, path.node, side),
            None => DropTarget::Tab(path.surface, path.node),
        },
        None => DropTarget::Float(pointer),
    }
}

/// El borde de `rect` en cuya franja ([`EDGE_ZONE`]) está `p`; si está en dos (una esquina), el
/// más cercano en proporción a su franja.
fn edge_of(rect: egui::Rect, p: egui::Pos2) -> Option<Split> {
    let (w, h) = (rect.width() * EDGE_ZONE, rect.height() * EDGE_ZONE);
    [
        (p.x - rect.left(), w, Split::Left),
        (rect.right() - p.x, w, Split::Right),
        (p.y - rect.top(), h, Split::Above),
        (rect.bottom() - p.y, h, Split::Below),
    ]
    .into_iter()
    .filter(|&(d, zone, _)| zone > 0.0 && d < zone)
    .min_by(|a, b| (a.0 / a.1).total_cmp(&(b.0 / b.1)))
    .map(|(_, _, side)| side)
}

/// Lo que ocupa un panel soltado en `target` (para resaltarlo mientras se arrastra): el grupo
/// entero, si va como pestaña; su mitad de ese lado, si lo parte; nada si va a flotar.
fn drop_zone(w: &Workspace, target: DropTarget) -> Option<egui::Rect> {
    let leaf_rect = |surface, node| {
        w.state
            .leaf(NodePath { surface, node })
            .ok()
            .map(|leaf| leaf.rect)
    };
    match target {
        DropTarget::Tab(surface, node) => leaf_rect(surface, node),
        DropTarget::Split(surface, node, side) => {
            let r = leaf_rect(surface, node)?;
            let c = r.center();
            Some(match side {
                Split::Left => r.with_max_x(c.x),
                Split::Right => r.with_min_x(c.x),
                Split::Above => r.with_max_y(c.y),
                Split::Below => r.with_min_y(c.y),
            })
        }
        DropTarget::Float(_) => None,
    }
    .filter(|r| r.is_positive())
    .map(|r| egui::Rect::from_min_max(r.min.round(), r.max.round()))
}

/// Resalta dónde caería el panel que se arrastra desde la tira (encima de todo salvo el
/// fantasma): con Pixel UI, la zona de destino de egui_dock (la franja de selección en el marco
/// `menu.side`); sin él, un rect con el color de selección de egui.
pub(crate) fn paint_drop_zone(
    ui: &egui::Ui,
    skin: Option<&Skin>,
    w: &Workspace,
    target: DropTarget,
) {
    let Some(rect) = drop_zone(w, target) else {
        return;
    };
    let ctx = ui.ctx();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("workspace_drop_zone"),
    ));
    match skin {
        Some(skin) => painter.extend(skin::drop_shapes(skin, rect, ctx.pixels_per_point())),
        None => {
            let color = ui.visuals().selection.bg_fill.gamma_multiply(0.4);
            painter.rect_filled(rect, 0.0, color);
        }
    }
}

impl Workspace {
    /// Pone `id` donde cayó (ver [`drop_target`]).
    pub(crate) fn drop_panel(&mut self, id: PanelId, target: DropTarget) {
        match target {
            DropTarget::Tab(surface, node) => self.dock(id, DockTarget::Leaf(surface, node)),
            DropTarget::Split(surface, node, side) => {
                self.dock(id, DockTarget::Split(surface, node, side));
            }
            DropTarget::Float(at) => self.float(id, at),
        }
    }

    /// Le da a cada grupo de la superficie principal su rect dentro de `rect`, como lo haría
    /// [`show`] (sin el ancho de los divisores): para probar [`drop_target`] sin dibujar.
    #[cfg(test)]
    pub(crate) fn layout_for_test(&mut self, rect: egui::Rect) {
        fn place(tree: &mut Tree<PanelId>, node: NodeIndex, rect: egui::Rect) {
            if node.0 >= tree.len() {
                return;
            }
            tree[node].set_rect(rect);
            let (a, b) = match &tree[node] {
                Node::Horizontal(s) => {
                    let x = rect.left() + rect.width() * s.fraction;
                    (rect.with_max_x(x), rect.with_min_x(x))
                }
                Node::Vertical(s) => {
                    let y = rect.top() + rect.height() * s.fraction;
                    (rect.with_max_y(y), rect.with_min_y(y))
                }
                _ => return,
            };
            place(tree, node.left(), a);
            place(tree, node.right(), b);
        }
        place(self.state.main_surface_mut(), NodeIndex::root(), rect);
    }
}

/// El panel que egui_dock arrastraba y dónde se soltó, el frame en que se suelta. Mientras se
/// arrastra (por su pestaña o por la franja de un grupo sin barra), se recuerda cuál es: el frame
/// de soltar egui_dock ya no lo dice.
pub(super) fn tab_drag_released(
    ctx: &egui::Context,
    state: &DockState<PanelId>,
    hits: &[skin::TabHit],
) -> Option<(PanelId, egui::Pos2)> {
    let memory = egui::Id::new("workspace_tab_drag");
    if ctx.input(|i| i.pointer.primary_released()) {
        let id = ctx.data_mut(|d| d.remove_temp::<Option<PanelId>>(memory))??;
        return Some((id, ctx.input(|i| i.pointer.latest_pos())?.round()));
    }
    let dragged = hits
        .iter()
        .find(|h| skin::is_dragged_tab(&h.response))
        .map(|h| h.id)
        .or_else(|| {
            state
                .iter_leaves()
                .find(|(path, _)| skin::hidden_strip_dragged(ctx, dock_id(), *path))
                .and_then(|(_, leaf)| leaf.tabs.get(leaf.active.0).copied())
        });
    ctx.data_mut(|d| d.insert_temp(memory, dragged));
    None
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::*;

    #[test]
    fn the_drop_target_is_a_tab_in_the_middle_a_split_at_the_edge_and_a_window_outside() {
        let mut w = Workspace::essentials();
        let dock_rect =
            egui::Rect::from_min_size(egui::pos2(1000.0, 50.0), egui::vec2(220.0, 600.0));
        w.layout_for_test(dock_rect); // da a las hojas su rect como lo haría show()
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        assert!(
            matches!(drop_target(&w, egui::pos2(1110.0, 350.0), dock_rect), DropTarget::Tab(ss, nn) if (ss, nn) == (s, n))
        );
        assert!(matches!(
            drop_target(&w, egui::pos2(1110.0, 640.0), dock_rect),
            DropTarget::Split(_, _, egui_dock::Split::Below)
        ));
        assert!(matches!(
            drop_target(&w, egui::pos2(400.0, 300.0), dock_rect),
            DropTarget::Float(_)
        ));
    }

    #[test]
    fn the_drop_target_finds_the_group_under_the_pointer_and_each_edge() {
        let mut w = Workspace::essentials();
        let (s, color) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Groom, DockTarget::Split(s, color, Split::Below));
        let dock_rect =
            egui::Rect::from_min_size(egui::pos2(1000.0, 50.0), egui::vec2(200.0, 600.0));
        w.layout_for_test(dock_rect);
        // Color arriba (50..350), Groom abajo (350..650).
        let groom = w.leaf_of(PanelId::Groom).unwrap();
        let color = w.leaf_of(PanelId::Color).unwrap();
        let at = |x, y| drop_target(&w, egui::pos2(x, y), dock_rect);
        assert_eq!(at(1100.0, 500.0), DropTarget::Tab(groom.0, groom.1));
        assert_eq!(at(1100.0, 200.0), DropTarget::Tab(color.0, color.1));
        assert_eq!(
            at(1010.0, 500.0),
            DropTarget::Split(groom.0, groom.1, Split::Left)
        );
        assert_eq!(
            at(1190.0, 200.0),
            DropTarget::Split(color.0, color.1, Split::Right)
        );
        assert_eq!(
            at(1100.0, 60.0),
            DropTarget::Split(color.0, color.1, Split::Above)
        );
        // La esquina: el borde más cercano en proporción a su franja (10 de 50 a la izquierda,
        // 50 de 75 abajo).
        assert_eq!(
            at(1010.0, 600.0),
            DropTarget::Split(groom.0, groom.1, Split::Left)
        );
        // Y lo que se suelta ahí, ahí queda.
        w.drop_panel(PanelId::Library, at(1100.0, 500.0));
        assert_eq!(w.leaf_of(PanelId::Library), w.leaf_of(PanelId::Groom));
        w.drop_panel(PanelId::Lumen, DropTarget::Float(egui::pos2(300.0, 200.0)));
        assert!(w.floating(PanelId::Lumen));
        assert!(!w.strip().contains(&PanelId::Library) && !w.strip().contains(&PanelId::Lumen));
    }
}
