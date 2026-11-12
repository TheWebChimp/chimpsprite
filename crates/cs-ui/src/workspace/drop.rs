//! Dónde cae un panel que se suelta ([`drop_target`]; sobre otra tira, [`strip_under`]), cómo se
//! resalta mientras se arrastra ([`paint_drop_zone`] y, sobre otra tira, [`paint_strip_zone`]) y
//! la pestaña de egui_dock que se suelta fuera de los grupos ([`tab_drag_released`]: el workspace
//! la pone a flotar donde cayó).

use eframe::egui;
use egui_dock::{DockState, NodePath};
#[cfg(test)]
use egui_dock::{Node, Tree};

use super::fit::{half, whole};
use super::{DockTarget, NodeIndex, Split, SurfaceIndex, Workspace, dock_id, skin};
use crate::dock::StripArea;
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
/// entero, si va como pestaña; su mitad de ese lado, si lo parte (la misma que se pinta al
/// arrastrar una pestaña sobre el lienzo: [`half`]); nada si va a flotar. En puntos enteros.
fn drop_zone(w: &Workspace, target: DropTarget) -> Option<egui::Rect> {
    let leaf_rect = |surface, node| {
        w.state
            .leaf(NodePath { surface, node })
            .ok()
            .map(|leaf| leaf.rect)
    };
    match target {
        DropTarget::Tab(surface, node) => leaf_rect(surface, node).map(whole),
        DropTarget::Split(surface, node, side) => Some(half(leaf_rect(surface, node)?, side)),
        DropTarget::Float(_) => None,
    }
    .filter(|r| r.is_positive())
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
    if let Some(rect) = drop_zone(w, target) {
        paint_zone(ui, skin, rect);
    }
}

/// Resalta la tira `rect` (la de otra orilla) sobre la que se arrastra un ícono: soltarlo ahí lo
/// pasa a ella (ver [`Workspace::apply_strip_request`]). Como [`paint_drop_zone`], la tira entera.
pub(crate) fn paint_strip_zone(ui: &egui::Ui, skin: Option<&Skin>, rect: egui::Rect) {
    let rect = whole(rect);
    if rect.is_positive() {
        paint_zone(ui, skin, rect);
    }
}

/// La tira de `strips` (las dibujadas en este frame) bajo `p`, si alguna: un ícono de otra tira
/// soltado ahí pasa a ella, entre sus botones (ver [`Workspace::apply_strip_request`]) y, mientras
/// se arrastra, se resalta (ver [`paint_strip_zone`]) con la línea de dónde caería (ver
/// [`paint_insertion_line`]).
pub(crate) fn strip_under(strips: &[StripArea], p: egui::Pos2) -> Option<&StripArea> {
    strips.iter().find(|s| s.rect.contains(p))
}

/// La línea de inserción `rect` de una tira (ver `dock::insertion_line`), en la capa de la zona de
/// destino (encima de ella, si la hay) y debajo del fantasma: con Pixel UI, del color de selección
/// del skin (`text_selection`); sin él, del de egui.
pub(crate) fn paint_insertion_line(ui: &egui::Ui, skin: Option<&Skin>, rect: egui::Rect) {
    if !rect.is_positive() {
        return;
    }
    let color = match skin {
        Some(skin) => skin.colors().text_selection,
        None => ui.visuals().selection.bg_fill,
    };
    zone_painter(ui.ctx()).rect_filled(rect, 0.0, color);
}

/// La capa de las zonas de destino: encima de todo salvo el fantasma.
fn zone_painter(ctx: &egui::Context) -> egui::Painter {
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("workspace_drop_zone"),
    ))
}

/// La zona de destino `rect` encima de todo salvo el fantasma (ver [`paint_drop_zone`]).
fn paint_zone(ui: &egui::Ui, skin: Option<&Skin>, rect: egui::Rect) {
    let ctx = ui.ctx();
    let painter = zone_painter(ctx);
    match skin {
        Some(skin) => painter.extend(skin::drop_shapes(skin, rect, ctx.pixels_per_point())),
        None => {
            let color = ui.visuals().selection.bg_fill.gamma_multiply(0.4);
            painter.rect_filled(rect, 0.0, color);
        }
    }
}

impl Workspace {
    /// Lo que pidió una tira (`request`; ver `dock::StripRequest`). Un ícono soltado sobre una
    /// tira (`strips`: las dibujadas en este frame) pasa a ella, entre sus botones donde se soltó
    /// (ver `StripArea::drop_index`); en otro lado, va donde cae en el árbol (ver
    /// [`drop_target`]). Uno que se reordena en su tira, al hueco que dice.
    pub(crate) fn apply_strip_request(
        &mut self,
        request: crate::dock::StripRequest,
        strips: &[StripArea],
    ) {
        use crate::dock::StripRequest;
        match request {
            StripRequest::Drop(id, pointer) => match strip_under(strips, pointer) {
                Some(strip) => self.move_in_strip(id, strip.edge, strip.drop_index(id, pointer)),
                None => {
                    let target = drop_target(self, pointer);
                    self.drop_panel(id, target);
                }
            },
            StripRequest::Reorder(id, gap) => {
                if let Some(edge) = self.strip_of(id) {
                    // El hueco cuenta al panel; el lugar, no.
                    let at = self.strip_at(edge).iter().position(|&p| p == id);
                    let index = if at.is_some_and(|at| gap > at) {
                        gap - 1
                    } else {
                        gap
                    };
                    self.move_in_strip(id, edge, index);
                }
            }
            StripRequest::Dock(id) => self.dock_from_strip(id),
            StripRequest::Float(id, at) => self.float(id, at),
        }
    }

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
    /// [`show`](fn@super::show) (sin el ancho de los divisores): para probar [`drop_target`] sin
    /// dibujar.
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
    use crate::dock::Edge;

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

    /// Essentials con la tira de la derecha `[Groom, Lumen, Library, …]` (lo demás del dock en su
    /// orden).
    fn groom_lumen_library() -> Workspace {
        let mut w = Workspace::essentials();
        let first = [PanelId::Groom, PanelId::Lumen, PanelId::Library];
        let rest = PanelId::DOCK.iter().copied().filter(|p| !first.contains(p));
        w.strips[Edge::Right.index()] = first.into_iter().chain(rest).collect();
        w
    }

    #[test]
    fn reordering_a_strip_moves_the_icon_to_the_gap_and_is_saved() {
        use crate::dock::StripRequest::Reorder;
        let base = groom_lumen_library();
        let head = |w: &Workspace| w.strip()[..3].to_vec();
        // Al principio.
        let mut w = base.clone();
        w.apply_strip_request(Reorder(PanelId::Library, 0), &[]);
        assert_eq!(head(&w), [PanelId::Library, PanelId::Groom, PanelId::Lumen]);
        // En el hueco 2 (después de Lumen).
        let mut w = base.clone();
        w.apply_strip_request(Reorder(PanelId::Groom, 2), &[]);
        assert_eq!(head(&w), [PanelId::Lumen, PanelId::Groom, PanelId::Library]);
        // Al final (el hueco después del último, o más allá).
        let mut end = base.clone();
        end.apply_strip_request(Reorder(PanelId::Groom, base.strip().len()), &[]);
        assert_eq!(end.strip().last(), Some(&PanelId::Groom));
        let mut far = base.clone();
        far.apply_strip_request(Reorder(PanelId::Groom, 99), &[]);
        assert_eq!(far.strip(), end.strip());
        // A su mismo lugar (el hueco de antes o el de después de él): nada cambia.
        for gap in [1, 2] {
            let mut same = base.clone();
            same.apply_strip_request(Reorder(PanelId::Lumen, gap), &[]);
            assert_eq!(same.strip(), base.strip(), "gap {gap}");
        }
        // Cada panel, una vez; nada se movió de las otras tiras ni del árbol.
        assert_eq!(w.state_tabs(), base.state_tabs());
        for &id in PanelId::ALL.iter().filter(|&&p| p != PanelId::Preview) {
            assert_eq!(w.places(id), 1, "{id:?}");
        }
        // Lo guardado se lee igual (en su orden, no en el de fábrica).
        let ron = ron::to_string(&w.to_saved()).unwrap();
        let back = Workspace::from_saved(ron::from_str(&ron).unwrap());
        assert_eq!(back.strip(), w.strip());
    }

    #[test]
    fn an_icon_dropped_on_another_strip_goes_where_it_was_dropped() {
        use crate::dock::{StripArea, StripRequest};
        let mut w = Workspace::essentials();
        w.to_strip_at(PanelId::Tools, Edge::Left);
        w.to_strip_at(PanelId::Groom, Edge::Left);
        // La de la izquierda, con sus dos botones (y la de la derecha, sin ninguno a la vista).
        let left = egui::Rect::from_min_size(egui::pos2(0.0, 50.0), egui::vec2(26.0, 600.0));
        let button = |i: f32| {
            egui::Rect::from_min_size(egui::pos2(3.0, 55.0 + 27.0 * i), egui::vec2(23.0, 23.0))
        };
        let strips = [
            StripArea {
                edge: Edge::Left,
                rect: left,
                buttons: vec![(PanelId::Tools, button(0.0)), (PanelId::Groom, button(1.0))],
            },
            StripArea {
                edge: Edge::Right,
                rect: left.translate(egui::vec2(1000.0, 0.0)),
                buttons: Vec::new(),
            },
        ];
        // Entre los dos: ahí.
        let between = egui::pos2(13.0, button(1.0).top() - 2.0);
        let mut a = w.clone();
        a.apply_strip_request(StripRequest::Drop(PanelId::Library, between), &strips);
        assert_eq!(
            a.strip_at(Edge::Left),
            [PanelId::Tools, PanelId::Library, PanelId::Groom]
        );
        assert!(!a.strip().contains(&PanelId::Library));
        // Antes del primero y más allá del último.
        let mut b = w.clone();
        b.apply_strip_request(
            StripRequest::Drop(PanelId::Library, pos(13.0, 51.0)),
            &strips,
        );
        assert_eq!(b.strip_at(Edge::Left)[0], PanelId::Library);
        let mut c = w.clone();
        c.apply_strip_request(
            StripRequest::Drop(PanelId::Library, pos(13.0, 600.0)),
            &strips,
        );
        assert_eq!(c.strip_at(Edge::Left).last(), Some(&PanelId::Library));
        // A una tira sin botones a la vista: al final.
        let mut d = w.clone();
        d.apply_strip_request(
            StripRequest::Drop(PanelId::Groom, pos(1013.0, 300.0)),
            &strips,
        );
        assert_eq!(d.strip().last(), Some(&PanelId::Groom));
        assert_eq!(d.strip_at(Edge::Left), [PanelId::Tools]);
        // En su propia tira (en el margen, fuera de sus botones): como reordenarla.
        let mut e = w.clone();
        e.apply_strip_request(StripRequest::Drop(PanelId::Groom, pos(1.0, 52.0)), &strips);
        assert_eq!(e.strip_at(Edge::Left), [PanelId::Groom, PanelId::Tools]);
        let mut f = w.clone();
        f.apply_strip_request(StripRequest::Drop(PanelId::Groom, pos(1.0, 600.0)), &strips);
        assert_eq!(f.strip_at(Edge::Left), w.strip_at(Edge::Left));
        for x in [&a, &b, &c, &d, &e, &f] {
            for &id in PanelId::ALL.iter().filter(|&&p| p != PanelId::Preview) {
                assert_eq!(x.places(id), 1, "{id:?}");
            }
        }
        // Pasar a otra tira en una posición, sin pasar por la app.
        let mut g = w.clone();
        g.move_in_strip(PanelId::Library, Edge::Left, 1);
        assert_eq!(g.strip_at(Edge::Left), a.strip_at(Edge::Left));
    }

    fn pos(x: f32, y: f32) -> egui::Pos2 {
        egui::pos2(x, y)
    }
}
