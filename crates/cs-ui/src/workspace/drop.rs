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
/// pasado: sobre una ventana flotante, en su grupo; sobre el área del árbol (la superficie
/// principal), en el grupo bajo el puntero (o el más cercano, si cae en un divisor); en el centro
/// de un grupo, como pestaña, y en la franja de un borde, partiéndolo. El lienzo no recibe
/// pestañas: en la franja de un borde se parte a su lado y en su centro el panel flota. Fuera del
/// árbol, también flotando.
pub(crate) fn drop_target(w: &Workspace, pointer: egui::Pos2) -> DropTarget {
    let leaves: Vec<(NodePath, egui::Rect, bool)> = w
        .state
        .iter_leaves()
        .filter(|(_, leaf)| !leaf.tabs.is_empty() && leaf.rect.is_positive())
        .map(|(path, leaf)| (path, leaf.rect, leaf.tabs.contains(&PanelId::Canvas)))
        .collect();
    // Las ventanas van encima del árbol.
    let window = leaves
        .iter()
        .find(|(path, rect, _)| !path.surface.is_main() && rect.contains(pointer));
    let docked = || {
        leaves
            .iter()
            .filter(|(path, _, _)| path.surface.is_main())
            .min_by(|a, b| {
                let d = |r: &egui::Rect| r.distance_sq_to_pos(pointer);
                d(&a.1).total_cmp(&d(&b.1))
            })
    };
    let area = w.root_rect(SurfaceIndex::main());
    let hit = match window {
        Some(hit) => Some(hit),
        None if area.is_some_and(|a| a.contains(pointer)) => docked(),
        None => None,
    };
    match hit {
        Some(&(path, rect, canvas)) => match (edge_of(rect, pointer), canvas) {
            (Some(side), _) => DropTarget::Split(path.surface, path.node, side),
            (None, false) => DropTarget::Tab(path.surface, path.node),
            (None, true) => DropTarget::Float(pointer),
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

/// Dónde [`tab_drag_released`] recuerda el panel que egui_dock arrastra.
fn tab_drag_id() -> egui::Id {
    egui::Id::new("workspace_tab_drag")
}

/// El panel que egui_dock arrastraba en el frame anterior (por su pestaña o por su franja), si
/// alguno: lo que [`tab_drag_released`] recordó.
pub(super) fn dragged_tab(ctx: &egui::Context) -> Option<PanelId> {
    ctx.data(|d| d.get_temp::<Option<PanelId>>(tab_drag_id()))
        .flatten()
}

/// El panel que egui_dock arrastraba y dónde se soltó, el frame en que se suelta. Mientras se
/// arrastra (por su pestaña o por la franja de un grupo sin barra), se recuerda cuál es: el frame
/// de soltar egui_dock ya no lo dice. El lienzo no cuenta (su arrastre se corta).
pub(super) fn tab_drag_released(
    ctx: &egui::Context,
    state: &DockState<PanelId>,
    hits: &[skin::TabHit],
) -> Option<(PanelId, egui::Pos2)> {
    let memory = tab_drag_id();
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
        })
        .filter(|&id| id != PanelId::Canvas);
    ctx.data_mut(|d| d.insert_temp(memory, dragged));
    None
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::*;

    /// Lo acomoda como `show` en el área de 0..`right` × 50..650 (sin divisores), con la columna
    /// derecha (Color y lo que tenga) de `column` puntos: la de las medidas de la fase C.
    fn laid_out(w: &mut Workspace, right: f32, column: f32) {
        let area = egui::Rect::from_min_max(egui::pos2(0.0, 50.0), egui::pos2(right, 650.0));
        let mut sides = super::super::fit::SidePoints::default();
        sides.column = Some(column);
        // La toolbar de Pixel UI (0..37) a la izquierda del lienzo.
        sides.toolbar = Some(37.0);
        super::super::fit::pin_sides(&mut w.state, area, &mut sides);
        w.layout_for_test(area); // da a las hojas su rect como lo haría show()
    }

    #[test]
    fn the_drop_target_is_a_tab_in_the_middle_a_split_at_the_edge_and_a_window_outside() {
        // La toolbar de 0 a 37, el lienzo de 37 a 1000; Color, de 1000 a 1220.
        let mut w = Workspace::essentials();
        laid_out(&mut w, 1220.0, 220.0);
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        assert!(
            matches!(drop_target(&w, egui::pos2(1110.0, 350.0)), DropTarget::Tab(ss, nn) if (ss, nn) == (s, n))
        );
        assert!(matches!(
            drop_target(&w, egui::pos2(1110.0, 640.0)),
            DropTarget::Split(_, _, egui_dock::Split::Below)
        ));
        // En el centro del lienzo: flotando; en la franja de su borde, a su lado.
        assert!(matches!(
            drop_target(&w, egui::pos2(400.0, 300.0)),
            DropTarget::Float(_)
        ));
        let (cs, cn) = w.leaf_of(PanelId::Canvas).unwrap();
        assert_eq!(
            drop_target(&w, egui::pos2(100.0, 300.0)),
            DropTarget::Split(cs, cn, Split::Left)
        );
        // Fuera del árbol (más abajo, en la barra de estado): flotando.
        assert!(matches!(
            drop_target(&w, egui::pos2(1110.0, 700.0)),
            DropTarget::Float(_)
        ));
    }

    #[test]
    fn the_drop_target_finds_the_group_under_the_pointer_and_each_edge() {
        let mut w = Workspace::essentials();
        let (s, color) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Groom, DockTarget::Split(s, color, Split::Below));
        laid_out(&mut w, 1200.0, 200.0);
        // Color arriba (50..350), Groom abajo (350..650), de 1000 a 1200.
        let groom = w.leaf_of(PanelId::Groom).unwrap();
        let color = w.leaf_of(PanelId::Color).unwrap();
        let at = |x, y| drop_target(&w, egui::pos2(x, y));
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
