//! Con un diálogo modal, lo de egui_dock no se toca (ver el modal en [`super::show::show`]): lo
//! que el `DockArea` cambiaría se repone después ([`Frozen`]), las ventanas flotantes no se mueven
//! arrastrando su fondo ([`hold_windows`]) y encima de lo que se agarraría de ellas va un escudo
//! ([`modal_shields`]).

use eframe::egui;
use egui_dock::DockState;

use super::{NodeIndex, SurfaceIndex, skin};
use crate::panel_registry::PanelId;
use crate::skin::Skin;

/// Sin Pixel UI, la barra de título del Preview solo en su ventana (`surface`): es de la ventana
/// de egui (la de antes no miraba el modal), así que de ahí se sigue moviendo (ver
/// [`hold_windows`]). `None` si su área todavía no existe.
pub(super) fn plain_preview_title(
    ui: &egui::Ui,
    surface: SurfaceIndex,
) -> Option<(SurfaceIndex, egui::Rect)> {
    let area = ui
        .ctx()
        .memory(|m| m.area_rect(skin::window_area_id(surface)))?;
    let bottom = area.top() + crate::preview::Preview::plain_title_bottom(ui);
    Some((surface, area.with_max_y(bottom)))
}

/// Apaga `ui` (no recibe clics ni arrastres, tampoco los de accesibilidad, y no se resalta) sin
/// que se vea distinto: apagar un `Ui` lo atenúa, y aquí se le devuelve su opacidad.
pub(super) fn disable_unfaded(ui: &mut egui::Ui) {
    let opacity = ui.opacity();
    ui.disable();
    ui.set_opacity(opacity);
}

/// Con un diálogo modal, lo de las ventanas flotantes que se agarraría (ver el modal en
/// [`super::show::show`]): egui_dock las arma con su propia `egui::Window`, que no hereda lo
/// apagado del `Ui` del `DockArea`, así que encima de lo suyo, en su capa, va un widget que se
/// queda con el puntero sin hacer nada (registrado después, le gana a lo de debajo). En una ventana
/// con Color o con el Preview (que un diálogo no apaga), solo lo que la mueve, la cambia de tamaño,
/// la parte o la acopla: su marco (y el borde de afuera del que se agarra), sus barras de
/// pestañas, sus franjas de agarre (con Pixel UI) y sus divisores. Cualquier otra, entera: lo de
/// adentro ya va apagado y arrastrarlo movería la ventana.
pub(super) fn modal_shields(ui: &mut egui::Ui, skin: Option<&Skin>, state: &DockState<PanelId>) {
    let ctx = ui.ctx().clone();
    let grab = ctx.global_style().interaction.resize_grab_radius_side;
    for (surface, s) in state.iter_surfaces_indexed() {
        let Some(tree) = s.node_tree().filter(|_| !surface.is_main()) else {
            continue;
        };
        let Some(area) = ctx.memory(|m| m.area_rect(skin::window_area_id(surface))) else {
            continue;
        };
        let outer = area.expand(grab);
        let live = tree
            .iter()
            .filter_map(|n| n.tabs())
            .flatten()
            .any(|t| matches!(t, PanelId::Color | PanelId::Preview));
        let mut rects = Vec::new();
        if !live {
            rects.push(outer);
        } else {
            // El marco (con el borde de afuera): de lo de afuera a lo del árbol.
            let inner = tree
                .root_node()
                .and_then(|n| n.rect())
                .filter(|r| r.is_positive())
                .unwrap_or(area);
            rects.extend([
                outer.with_max_y(inner.top()),
                outer.with_min_y(inner.bottom()),
                egui::Rect::from_x_y_ranges(outer.left()..=inner.left(), inner.y_range()),
                egui::Rect::from_x_y_ranges(inner.right()..=outer.right(), inner.y_range()),
            ]);
            for (i, node) in tree.iter().enumerate() {
                let n = NodeIndex(i);
                match node {
                    egui_dock::Node::Leaf(leaf) if leaf.tab_bar_hidden => {
                        if skin.is_some() {
                            rects.push(egui::Rect::from_min_size(
                                leaf.rect.min,
                                egui::vec2(leaf.rect.width(), skin::GRAB_STRIP),
                            ));
                        }
                    }
                    egui_dock::Node::Leaf(leaf) if leaf.viewport.is_positive() => {
                        rects.push(leaf.rect.with_max_y(leaf.viewport.top()));
                    }
                    egui_dock::Node::Horizontal(split) => {
                        if let (Some(a), Some(b)) = (tree[n.left()].rect(), tree[n.right()].rect())
                        {
                            let gap = egui::Rect::from_x_y_ranges(
                                a.right()..=b.left(),
                                split.rect.y_range(),
                            );
                            rects.push(gap.expand2(egui::vec2(skin::SEPARATOR_EXTRA, 0.0)));
                        }
                    }
                    egui_dock::Node::Vertical(split) => {
                        if let (Some(a), Some(b)) = (tree[n.left()].rect(), tree[n.right()].rect())
                        {
                            let gap = egui::Rect::from_x_y_ranges(
                                split.rect.x_range(),
                                a.bottom()..=b.top(),
                            );
                            rects.push(gap.expand2(egui::vec2(0.0, skin::SEPARATOR_EXTRA)));
                        }
                    }
                    _ => {}
                }
            }
        }
        let layer = egui::LayerId::new(egui::Order::Middle, skin::window_area_id(surface));
        let mut shield = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("workspace_modal_shield", surface))
                .layer_id(layer)
                .max_rect(outer),
        );
        shield.set_clip_rect(outer);
        for (k, rect) in rects.into_iter().enumerate() {
            if rect.is_positive() {
                let id = egui::Id::new(("workspace_modal_shield", surface, k));
                let _ = shield.interact(rect, id, egui::Sense::click_and_drag());
            }
        }
    }
}

/// Con un diálogo modal, ninguna ventana flotante se mueve arrastrando lo suyo: egui_dock las arma
/// con una `egui::Window` que se arrastra de cualquier lugar de su fondo (también de lo de un
/// panel apagado o del margen de Color), y `modal_shields` solo tapa lo que la acopla o la parte.
/// Antes del `DockArea`, si lo que se arrastra es el fondo de una de ellas (el widget "move" de su
/// área), se suelta: así no se mueve ni este frame. Salvo `title` (sin Pixel UI, la barra de
/// título del Preview solo en su ventana, que es de la ventana de egui) si el arrastre empezó ahí.
pub(super) fn hold_windows(
    ctx: &egui::Context,
    state: &DockState<PanelId>,
    title: Option<(SurfaceIndex, egui::Rect)>,
) {
    let Some(dragged) = ctx.dragged_id() else {
        return;
    };
    let press = ctx.input(|i| i.pointer.press_origin());
    let window = state
        .iter_surfaces_indexed()
        .map(|(s, _)| s)
        .find(|s| !s.is_main() && dragged == skin::window_area_id(*s).with("move"));
    if let Some(s) = window {
        let from_title = title.is_some_and(|(t, r)| t == s && press.is_some_and(|p| r.contains(p)));
        if !from_title {
            ctx.stop_dragging();
        }
    }
}

/// Lo que el `DockArea` cambia con el puntero, el teclado o un clic de accesibilidad: la fracción
/// de cada split, la pestaña al frente de cada grupo y el grupo con el foco (el de la app y el de
/// cada árbol). Con un diálogo modal se toma antes y se repone después (ver el modal en
/// [`super::show::show`]): nada de eso cambia aunque algo se cuele (en una ventana flotante, un
/// clic de accesibilidad no pasa por `modal_shields`, y egui_dock enfoca el grupo de un clic en su
/// cuerpo aunque haya un escudo encima).
pub(super) struct Frozen {
    splits: Vec<(egui_dock::NodePath, f32)>,
    active: Vec<(egui_dock::NodePath, egui_dock::TabIndex)>,
    focused: Option<egui_dock::NodePath>,
    /// El nodo con el foco de cada árbol: egui_dock guarda uno por superficie, aparte del de la
    /// superficie con el foco (`focused`).
    trees: Vec<(SurfaceIndex, Option<NodeIndex>)>,
}

impl Frozen {
    pub(super) fn take(state: &DockState<PanelId>) -> Frozen {
        Frozen {
            splits: state
                .iter_all_nodes()
                .filter_map(|(path, node)| match node {
                    egui_dock::Node::Horizontal(s) | egui_dock::Node::Vertical(s) => {
                        Some((path, s.fraction))
                    }
                    _ => None,
                })
                .collect(),
            active: state
                .iter_leaves()
                .map(|(path, leaf)| (path, leaf.active))
                .collect(),
            focused: state.focused_leaf(),
            trees: state
                .iter_surfaces_indexed()
                .filter_map(|(s, surface)| Some((s, surface.node_tree()?.focused_leaf())))
                .collect(),
        }
    }

    pub(super) fn restore(self, state: &mut DockState<PanelId>) {
        for (path, fraction) in self.splits {
            if let Ok(egui_dock::Node::Horizontal(s) | egui_dock::Node::Vertical(s)) =
                state.node_mut(path)
            {
                s.fraction = fraction;
            }
        }
        for (path, active) in self.active {
            if let Ok(leaf) = state.leaf_mut(path)
                && active.0 < leaf.tabs.len()
            {
                leaf.active = active;
            }
        }
        // Sin foco antes, sin foco después: egui_dock no tiene cómo quitarlo, pero un nodo que no
        // es una hoja lo deja sin él, en cada árbol (`Tree::set_focused_node`) y en la app
        // (`set_focused_node_and_surface`, que solo quita la superficie).
        for (surface, node) in self.trees {
            if let Some(tree) = state
                .get_surface_mut(surface)
                .and_then(|s| s.node_tree_mut())
            {
                tree.set_focused_node(node.unwrap_or(NodeIndex(usize::MAX)));
            }
        }
        let none = egui_dock::NodePath {
            surface: SurfaceIndex::main(),
            node: NodeIndex(usize::MAX),
        };
        state.set_focused_node_and_surface(self.focused.unwrap_or(none));
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use egui_dock::NodePath;

    use super::*;
    use crate::ChimpApp;
    use crate::workspace::test_util::{
        SIZE, button, click, click_at, close_tab, drag_to, frame_in, frame_with, frames_in,
        icon_label, leaf, many_tabs_below_color, node_rect, snapshot, tabs_offset, test_app, wheel,
    };
    use crate::workspace::{DockTarget, Workspace};

    /// Lo que se prueba con un diálogo abierto: un nombre, el acomodo de antes y lo que hace el
    /// usuario (con la salida del último frame).
    struct ModalCase {
        name: &'static str,
        /// Solo con Pixel UI (sin él no hay franja de agarre).
        pixel_only: bool,
        setup: fn(&mut ChimpApp),
        act: fn(&egui::Context, &mut ChimpApp, &egui::FullOutput, egui::Vec2),
        /// La ventana.
        size: egui::Vec2,
    }

    /// Cada caso con Pixel UI y sin él, sin diálogo y con uno abierto (`ChimpApp::is_modal`): sin
    /// diálogo lo que hace el usuario cambia el acomodo (si no, el caso no probaría nada); con
    /// uno, nada cambia.
    fn assert_blocked_by_a_modal(cases: &[ModalCase]) {
        for case in cases {
            for pixel in [true, false] {
                if case.pixel_only && !pixel {
                    continue;
                }
                for modal in [false, true] {
                    let mut app = test_app();
                    app.view.pixel_ui = pixel;
                    (case.setup)(&mut app);
                    let ctx = egui::Context::default();
                    ctx.enable_accesskit();
                    let size = case.size;
                    let mut out = frames_in(&ctx, &mut app, size);
                    if modal {
                        app.show_message("Test", "A modal dialog");
                        out = frames_in(&ctx, &mut app, size);
                        assert!(app.is_modal());
                    }
                    let before = snapshot(&ctx, &app);
                    (case.act)(&ctx, &mut app, &out, size);
                    frames_in(&ctx, &mut app, size);
                    let after = snapshot(&ctx, &app);
                    let what = format!("{}, pixel {pixel}, modal {modal}", case.name);
                    if modal {
                        assert_eq!(after, before, "{what}: nothing changes");
                    } else {
                        assert_ne!(after, before, "{what}: without a dialog it changes");
                    }
                }
            }
        }
    }

    /// Un clic con el botón principal en el centro del nodo `label` de `out`.
    fn click_node(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        out: &egui::FullOutput,
        size: egui::Vec2,
        label: &str,
    ) {
        let rect = node_rect(out, label).unwrap_or_else(|| {
            let all = crate::workspace::test_util::buttons(out);
            panic!("no {label:?} in {all:?}")
        });
        click_at(ctx, app, rect.center(), egui::PointerButton::Primary, size);
    }

    /// Library con Slices en pestañas, flotando (Slices al frente).
    fn library_and_slices_floating(app: &mut ChimpApp) {
        app.workspace
            .float(PanelId::Library, egui::pos2(320.0, 120.0));
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        app.workspace.dock(PanelId::Slices, DockTarget::Leaf(s, n));
    }

    /// Lo mismo, una encima de la otra en su ventana.
    fn library_over_slices_floating(app: &mut ChimpApp) {
        app.workspace
            .float(PanelId::Library, egui::pos2(320.0, 120.0));
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        app.workspace.dock(
            PanelId::Slices,
            DockTarget::Split(s, n, egui_dock::Split::Below),
        );
    }

    #[test]
    fn with_a_modal_dialog_the_dividers_do_not_move() {
        assert_blocked_by_a_modal(&[
            ModalCase {
                name: "the divider between Color and Library",
                pixel_only: false,
                setup: |app| {
                    let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
                    let below = DockTarget::Split(s, n, egui_dock::Split::Below);
                    app.workspace.dock(PanelId::Library, below);
                },
                act: |ctx, app, _, size| {
                    let (color, below) = (leaf(app, PanelId::Color), leaf(app, PanelId::Library));
                    let at = egui::pos2(color.center().x, (color.bottom() + below.top()) / 2.0);
                    drag_to(ctx, app, (at, at + egui::vec2(0.0, 30.0)), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the divider of the right column",
                pixel_only: false,
                setup: |_| {},
                act: |ctx, app, _, size| {
                    let (canvas, color) = (leaf(app, PanelId::Canvas), leaf(app, PanelId::Color));
                    let at = egui::pos2((canvas.right() + color.left()) / 2.0, color.center().y);
                    drag_to(ctx, app, (at, at - egui::vec2(30.0, 0.0)), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "a divider in a floating window",
                pixel_only: false,
                setup: library_over_slices_floating,
                act: |ctx, app, _, size| {
                    let (top, below) = (leaf(app, PanelId::Library), leaf(app, PanelId::Slices));
                    let at = egui::pos2(top.center().x, (top.bottom() + below.top()) / 2.0);
                    drag_to(ctx, app, (at, at + egui::vec2(0.0, 20.0)), size);
                },
                size: SIZE,
            },
        ]);
    }

    #[test]
    fn with_a_modal_dialog_the_tabs_do_not_switch_close_or_move() {
        assert_blocked_by_a_modal(&[
            ModalCase {
                name: "clicking an inactive tab",
                pixel_only: false,
                setup: |app| app.workspace = Workspace::demo("library-tab").unwrap(),
                act: |ctx, app, out, size| click_node(ctx, app, out, size, "Color"),
                size: SIZE,
            },
            ModalCase {
                name: "an accessibility click on an inactive tab",
                pixel_only: false,
                setup: |app| app.workspace = Workspace::demo("library-tab").unwrap(),
                act: |ctx, app, out, size| {
                    let node = button(out, "Color").expect("the Color tab");
                    frame_in(ctx, app, vec![click(node)], size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the × of a tab",
                pixel_only: false,
                setup: |app| app.workspace = Workspace::demo("library-tab").unwrap(),
                act: |ctx, app, out, size| close_tab(ctx, app, out, size, PanelId::Library),
                size: SIZE,
            },
            ModalCase {
                name: "dragging a tab onto the canvas",
                pixel_only: false,
                setup: |app| app.workspace = Workspace::demo("library-tab").unwrap(),
                act: |ctx, app, out, size| {
                    let tab = node_rect(out, "Library").unwrap().center();
                    let canvas = leaf(app, PanelId::Canvas).center();
                    drag_to(ctx, app, (tab, canvas), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "clicking an inactive tab of a floating window",
                pixel_only: false,
                setup: library_and_slices_floating,
                act: |ctx, app, out, size| click_node(ctx, app, out, size, "Library"),
                size: SIZE,
            },
            ModalCase {
                name: "an accessibility click on a tab of a floating window",
                pixel_only: false,
                setup: library_and_slices_floating,
                act: |ctx, app, out, size| {
                    let node = button(out, "Library").expect("the Library tab");
                    frame_in(ctx, app, vec![click(node)], size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the × of a tab of a floating window",
                pixel_only: false,
                setup: library_and_slices_floating,
                act: |ctx, app, out, size| close_tab(ctx, app, out, size, PanelId::Slices),
                size: SIZE,
            },
        ]);
    }

    #[test]
    fn with_a_modal_dialog_the_strip_does_nothing() {
        assert_blocked_by_a_modal(&[
            ModalCase {
                name: "clicking a strip icon",
                pixel_only: false,
                setup: |_| {},
                act: |ctx, app, out, size| {
                    let label = icon_label(app, PanelId::Lospec);
                    click_node(ctx, app, out, size, label);
                },
                size: SIZE,
            },
            ModalCase {
                name: "dragging a strip icon onto the canvas",
                pixel_only: false,
                setup: |_| {},
                act: |ctx, app, out, size| {
                    let label = icon_label(app, PanelId::Lospec);
                    let icon = node_rect(out, label).unwrap().center();
                    let canvas = leaf(app, PanelId::Canvas).center();
                    drag_to(ctx, app, (icon, canvas), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the context menu of a strip icon",
                pixel_only: false,
                setup: |_| {},
                act: |ctx, app, out, size| {
                    let label = icon_label(app, PanelId::Lospec);
                    let icon = node_rect(out, label).unwrap().center();
                    let out = click_at(ctx, app, icon, egui::PointerButton::Secondary, size);
                    // Abierto (sin diálogo): Dock.
                    if let Some(dock) = node_rect(&out, crate::dock::MENU_DOCK) {
                        click_at(ctx, app, dock.center(), egui::PointerButton::Primary, size);
                    }
                },
                size: SIZE,
            },
            ModalCase {
                name: "More Panels",
                pixel_only: false,
                setup: |_| {},
                // Baja: la tira de la derecha no tiene lugar para todos sus íconos.
                act: |ctx, app, out, size| {
                    let more = if app.pixel.skin().is_some() {
                        crate::dock::MORE
                    } else {
                        "»"
                    };
                    click_node(ctx, app, out, size, more);
                },
                size: egui::vec2(1280.0, 240.0),
            },
        ]);
    }

    #[test]
    fn with_a_modal_dialog_floating_windows_are_not_dragged_or_docked() {
        assert_blocked_by_a_modal(&[
            ModalCase {
                name: "the grab strip of a lone floating toolbar",
                pixel_only: true,
                setup: |app| app.workspace = Workspace::demo("tools-float").unwrap(),
                act: |ctx, app, _, size| {
                    let tools = leaf(app, PanelId::Tools);
                    let at = egui::pos2(tools.center().x, tools.top() + 1.0);
                    let to = leaf(app, PanelId::Canvas).center();
                    drag_to(ctx, app, (at, to), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the empty end of the tab bar of a floating window",
                pixel_only: false,
                setup: |app| {
                    app.workspace
                        .float(PanelId::Library, egui::pos2(320.0, 120.0))
                },
                act: |ctx, app, out, size| {
                    let tab = node_rect(out, "Library").unwrap();
                    let at = egui::pos2(tab.right() + 20.0, tab.center().y);
                    drag_to(ctx, app, (at, at + egui::vec2(40.0, 30.0)), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the padding of Color in its window",
                pixel_only: false,
                setup: |app| {
                    app.workspace
                        .float(PanelId::Color, egui::pos2(320.0, 120.0))
                },
                act: |ctx, app, _, size| {
                    let color = leaf(app, PanelId::Color);
                    let at = egui::pos2(color.left() + 2.0, color.center().y);
                    drag_to(ctx, app, (at, at + egui::vec2(40.0, 30.0)), size);
                },
                size: SIZE,
            },
            ModalCase {
                name: "the grab strip of the Preview in its window",
                pixel_only: true,
                setup: |app| app.view.preview = true,
                act: |ctx, app, _, size| {
                    let preview = leaf(app, PanelId::Preview);
                    let at = egui::pos2(preview.center().x, preview.top() + 1.0);
                    let to = leaf(app, PanelId::Color).center();
                    drag_to(ctx, app, (at, to), size);
                },
                size: SIZE,
            },
        ]);
    }

    #[test]
    fn with_a_modal_dialog_the_preview_window_still_moves_from_its_title() {
        // Como su ventana de antes (y como sus controles): un diálogo no la apaga.
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            app.view.preview = true;
            let ctx = egui::Context::default();
            frames_in(&ctx, &mut app, SIZE);
            app.show_message("Test", "A modal dialog");
            frames_in(&ctx, &mut app, SIZE);
            let window = app.workspace.window_rect(&ctx, PanelId::Preview).unwrap();
            // En su barra de título (sin Pixel UI, la de la `egui::Window` de antes).
            let title = if pixel { 9.0 } else { 17.0 };
            let at = egui::pos2(window.left() + 40.0, window.top() + title);
            drag_to(&ctx, &mut app, (at, at - egui::vec2(30.0, 20.0)), SIZE);
            frames_in(&ctx, &mut app, SIZE);
            let moved = app.workspace.window_rect(&ctx, PanelId::Preview).unwrap();
            assert_eq!(
                moved.min,
                window.min - egui::vec2(30.0, 20.0),
                "pixel {pixel}"
            );
            assert!(app.workspace.floating(PanelId::Preview));
        }
    }

    #[test]
    fn with_a_modal_dialog_the_wheel_scrolls_the_dialog_and_not_the_tabs_below() {
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            many_tabs_below_color(&mut app);
            let ctx = egui::Context::default();
            frames_in(&ctx, &mut app, SIZE);
            let bar = leaf(&app, PanelId::Library);
            app.show_message("Test", "A modal dialog");
            // Un diálogo (después del workspace, como los de la app) encima de la barra, con un
            // scroll que se corre con la rueda.
            let dialog = egui::Rect::from_min_size(
                egui::pos2(bar.left() + 10.0, bar.top() - 60.0),
                egui::vec2(200.0, 150.0),
            );
            let scrolled = std::cell::Cell::new(0.0);
            let draw = |ctx: &egui::Context| {
                egui::Window::new("Dialog")
                    .fixed_pos(dialog.min)
                    .fixed_size(dialog.size())
                    .show(ctx, |ui| {
                        let out =
                            egui::ScrollArea::vertical()
                                .id_salt("dialog_lines")
                                .show(ui, |ui| {
                                    for k in 0..80 {
                                        ui.label(format!("Line {k}"));
                                    }
                                });
                        scrolled.set(out.state.offset.y);
                    });
            };
            for _ in 0..4 {
                frame_with(&ctx, &mut app, Vec::new(), SIZE, draw);
            }
            assert!(app.is_modal());
            let offset = tabs_offset(&ctx, &app, PanelId::Library);
            let at = egui::pos2(bar.left() + 40.0, bar.top() + 6.0);
            frame_with(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(at)],
                SIZE,
                draw,
            );
            let down = wheel(egui::MouseWheelUnit::Line, -2.0, Default::default());
            frame_with(&ctx, &mut app, vec![down], SIZE, draw);
            for _ in 0..10 {
                frame_with(&ctx, &mut app, Vec::new(), SIZE, draw);
            }
            assert!(
                scrolled.get() > 0.0,
                "pixel {pixel}: the dialog's list scrolls"
            );
            assert_eq!(
                tabs_offset(&ctx, &app, PanelId::Library),
                offset,
                "pixel {pixel}: the tabs below do not page"
            );
        }
    }

    #[test]
    fn with_a_modal_dialog_a_window_with_color_does_not_move_from_a_disabled_panel() {
        // Color sigue vivo con un diálogo; Library, debajo en su ventana, va apagado: arrastrar
        // su fondo no mueve la ventana (ni el relleno de Color: ver el caso del modal).
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            app.workspace
                .float(PanelId::Color, egui::pos2(320.0, 120.0));
            let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
            let below = DockTarget::Split(s, n, egui_dock::Split::Below);
            app.workspace.dock(PanelId::Library, below);
            let ctx = egui::Context::default();
            frames_in(&ctx, &mut app, SIZE);
            app.show_message("Test", "A modal dialog");
            frames_in(&ctx, &mut app, SIZE);
            let window = app.workspace.window_rect(&ctx, PanelId::Color).unwrap();
            let library = leaf(&app, PanelId::Library);
            let at = egui::pos2(library.center().x, library.bottom() - 6.0);
            drag_to(&ctx, &mut app, (at, at + egui::vec2(40.0, 30.0)), SIZE);
            frames_in(&ctx, &mut app, SIZE);
            assert_eq!(
                app.workspace.window_rect(&ctx, PanelId::Color),
                Some(window),
                "pixel {pixel}"
            );
        }
    }

    #[test]
    fn a_modal_puts_back_no_focus_too() {
        let mut w = Workspace::essentials();
        // Sin foco (un nodo que no es una hoja lo quita), tampoco en el árbol.
        unfocus(&mut w.state);
        assert_eq!(w.state.focused_leaf(), None);
        let frozen = Frozen::take(&w.state);
        let (surface, node) = w.leaf_of(PanelId::Color).unwrap();
        w.state
            .set_focused_node_and_surface(NodePath { surface, node });
        assert!(w.state.focused_leaf().is_some());
        frozen.restore(&mut w.state);
        assert_eq!(w.state.focused_leaf(), None);
        // Tampoco en el árbol: cada superficie guarda su propio nodo con el foco.
        assert_eq!(focused_trees(&w.state), []);
        // En la app: sin foco antes, un clic en el cuerpo de una ventana flotante (egui_dock le da
        // el foco a su grupo aunque haya un escudo encima) no le deja el foco a su árbol.
        for modal in [false, true] {
            let mut app = test_app();
            app.workspace
                .float(PanelId::Library, egui::pos2(320.0, 120.0));
            let ctx = egui::Context::default();
            frames_in(&ctx, &mut app, SIZE);
            unfocus(&mut app.workspace.state);
            if modal {
                app.show_message("Test", "A modal dialog");
                frames_in(&ctx, &mut app, SIZE);
                assert!(app.is_modal());
            }
            let library = leaf(&app, PanelId::Library);
            click_at(
                &ctx,
                &mut app,
                library.center(),
                egui::PointerButton::Primary,
                SIZE,
            );
            frames_in(&ctx, &mut app, SIZE);
            let focused = focused_trees(&app.workspace.state);
            if modal {
                assert_eq!(focused, [], "with a dialog");
                assert_eq!(app.workspace.state.focused_leaf(), None);
            } else {
                let (surface, node) = app.workspace.leaf_of(PanelId::Library).unwrap();
                assert_eq!(
                    focused,
                    [(surface, node)],
                    "without a dialog the click focuses"
                );
            }
        }
    }

    /// El nodo con el foco de cada árbol (egui_dock guarda uno por superficie).
    fn focused_trees(state: &DockState<PanelId>) -> Vec<(SurfaceIndex, NodeIndex)> {
        state
            .iter_surfaces_indexed()
            .filter_map(|(s, surface)| Some((s, surface.node_tree()?.focused_leaf()?)))
            .collect()
    }

    /// Sin foco en ninguna superficie ni en ningún árbol.
    fn unfocus(state: &mut DockState<PanelId>) {
        for tree in state.iter_surfaces_mut().filter_map(|s| s.node_tree_mut()) {
            tree.set_focused_node(NodeIndex(usize::MAX));
        }
        state.set_focused_node_and_surface(NodePath {
            surface: SurfaceIndex::main(),
            node: NodeIndex(usize::MAX),
        });
        assert_eq!(focused_trees(state), []);
    }
}
