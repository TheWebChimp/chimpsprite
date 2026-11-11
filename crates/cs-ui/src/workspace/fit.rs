//! Lo mínimo de cada panel y de cada grupo ([`minima`], [`node_min`]) y cómo se ajustan a él los
//! splits ([`fit_splits`]) y las ventanas flotantes ([`fit_windows`]); también dónde se reabren
//! las ventanas de un acomodo leído ([`Workspace::place_restored_windows`]).

use std::collections::HashMap;

use eframe::egui;
use egui_dock::{DockState, Node, Tree};

use super::{NodeIndex, SurfaceIndex, Workspace, skin};
use crate::panel_registry::{Body, PanelId};
use crate::skin::Skin;
use crate::{dock, pixel_ui};

impl Workspace {
    /// Lo más angosta que puede quedar la columna de la superficie principal: cada panel
    /// acoplado en su mínimo (ver [`minima`]); los que van lado a lado, sumados.
    pub(crate) fn column_min_width(&self, ui: &egui::Ui, skin: Option<&Skin>) -> f32 {
        let (sep, bar) = chrome(ui, skin);
        let m = minima(ui, skin);
        node_min(self.state.main_surface(), NodeIndex::root(), &m, sep, bar).x
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
/// paleta en su mínimo); sin Pixel UI, nada (como su panel de antes). Los demás, el `min_width`
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

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::*;
    use crate::ChimpApp;
    use crate::workspace::test_util::{frame, frame_in, frames, test_app};
    use crate::workspace::{DockTarget, Split};

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
