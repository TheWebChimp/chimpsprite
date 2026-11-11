//! Lo que va encima de las barras de pestañas de egui_dock (ver [`super::show::show`]): el botón
//! « de cada grupo ([`collapse_buttons`]), la × del kit ([`close_marks`]), la rueda y las flechas
//! de una barra que no cabe ([`tab_wheel`], [`tab_arrows`]) y lo que se suelta pasando su última
//! pestaña ([`dropped_past_last`], [`to_the_end`]).

use eframe::egui;
use egui_dock::DockState;

use super::drop::dragged_tab;
use super::{NodeIndex, SurfaceIndex, Workspace, skin};
use crate::panel_registry::PanelId;
use crate::skin::Skin;
use crate::{dock, kit, pixel_ui};

/// El nombre del botón « de un grupo acoplado (su tooltip y su etiqueta accesible).
pub(crate) const COLLAPSE: &str = "Collapse to Icons";

/// El botón « ("Collapse to Icons") de cada grupo acoplado que tenga algo que mandar a la tira
/// (ver [`Workspace::collapsible`]), la flecha de atrás de las pestañas del kit: al final de su
/// barra de pestañas o, en un grupo sin barra, en la punta derecha de su franja de agarre mientras
/// el puntero anda por la franja o por el botón (y nada se arrastra). Devuelve el grupo cuyo
/// botón se apretó.
pub(super) fn collapse_buttons(
    ui: &mut egui::Ui,
    skin: Option<&Skin>,
    w: &Workspace,
    modal: bool,
) -> Option<(SurfaceIndex, NodeIndex)> {
    use crate::pixel_ui::Tip as _;
    let ctx = ui.ctx().clone();
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let dragging = ctx.dragged_id().is_some() && ctx.input(|i| i.pointer.is_decidedly_dragging());
    let size = egui::Vec2::splat(kit::tabs::ARROW_SIZE);
    let mut pressed = None;
    // La toolbar sola en su ventana (con Pixel UI) también lleva el suyo (ver
    // `Workspace::bare_tools`).
    let tools_window = w.bare_tools();
    let main_layer = ui.layer_id();
    for (path, leaf) in w.state.iter_leaves() {
        let window = !path.surface.is_main();
        if (window && Some(path.surface) != tools_window)
            || !leaf.rect.is_positive()
            || !w.collapsible(path.surface, path.node)
        {
            continue;
        }
        // El de una ventana va en su capa (encima de ella).
        let layer = if window {
            egui::LayerId::new(egui::Order::Middle, skin::window_area_id(path.surface))
        } else {
            main_layer
        };
        // Sin otra capa encima (otra ventana flotante, un menú).
        let ours = |p: egui::Pos2| {
            ctx.layer_id_at(p)
                .is_none_or(|l| l == main_layer || l == layer)
        };
        // En la barra de pestañas, en el lugar que ella le deja a la derecha (`COLLAPSE_ROOM`);
        // sin barra, a la izquierda de la barra de scroll del panel (no encima de ella).
        let left = match skin {
            // La toolbar con Pixel UI: justo a la derecha de su caja (la hoja es apenas más ancha
            // que ella: en su punta la tapaba). Su caja empieza en el margen izquierdo de la hoja:
            // el de siempre o, a la derecha del lienzo, un divisor menos (ver `tools_margin`).
            Some(skin) if leaf.tabs == [PanelId::Tools] => {
                let mut margin = f32::from(pixel_ui::TOOLBAR_MARGIN);
                if !window && w.tools_side() == Some(false) {
                    margin = (margin - w.sides.sep).max(0.0);
                }
                let box_width =
                    pixel_ui::toolbar_width(skin) - 2.0 * f32::from(pixel_ui::TOOLBAR_MARGIN);
                leaf.rect.left() + margin + box_width
            }
            // Sin Pixel UI, la toolbar no lleva barra de scroll del kit a la derecha: en la punta.
            None if leaf.tabs == [PanelId::Tools] => leaf.rect.right() - 1.0 - size.x,
            _ if leaf.tab_bar_hidden => {
                leaf.rect.right()
                    - f32::from(dock::FLYOUT_MARGIN)
                    - pixel_ui::SCROLL_BAR
                    - 1.0
                    - size.x
            }
            _ => leaf.rect.right() - 1.0 - size.x,
        };
        let at = egui::pos2(left, leaf.rect.top() + kit::tabs::ARROWS_TOP);
        let rect = egui::Rect::from_min_size(at.round(), size);
        if leaf.tab_bar_hidden {
            // Solo si el puntero llegó por la franja de agarre (no desde abajo, por el panel), y
            // mientras siga en ella o en el botón.
            let strip = egui::Rect::from_min_size(
                leaf.rect.min,
                egui::vec2(leaf.rect.width(), skin::GRAB_STRIP),
            );
            let inside = |r: egui::Rect| pointer.is_some_and(|p| r.contains(p) && ours(p));
            let memory = egui::Id::new(("workspace_collapse_armed", path.surface, path.node));
            let was = ctx.data(|d| d.get_temp::<bool>(memory)).unwrap_or(false);
            let armed = (was || inside(strip)) && (inside(strip) || inside(rect));
            ctx.data_mut(|d| d.insert_temp(memory, armed));
            if dragging || !armed {
                continue;
            }
        }
        let id = egui::Id::new(("workspace_collapse", path.surface, path.node));
        let mut on_window;
        let ui: &mut egui::Ui = if window {
            on_window = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(id)
                    .layer_id(layer)
                    .max_rect(rect),
            );
            on_window.set_clip_rect(rect);
            &mut on_window
        } else {
            &mut *ui
        };
        let mut response = ui.interact(rect, id, egui::Sense::click());
        if !modal {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !modal, COLLAPSE));
        let hovered = !modal && response.hovered();
        match skin {
            Some(skin) => {
                let shapes = kit::tabs::arrow_shapes(ui, skin, rect, true, (hovered, !modal));
                ui.painter().extend(shapes);
            }
            None => {
                let visuals = ui.style().interact(&response);
                ui.painter()
                    .rect_filled(rect, visuals.corner_radius, visuals.bg_fill);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "«",
                    egui::TextStyle::Body.resolve(ui.style()),
                    visuals.text_color(),
                );
            }
        }
        let response = response.tip(COLLAPSE);
        if !modal && response.clicked() {
            pressed = Some((path.surface, path.node));
        }
    }
    pressed
}

/// La × del kit de cada pestaña que se puede cerrar, encima de la de egui_dock (en su capa):
/// devuelve los paneles cuya × se apretó. Solo las de las pestañas que se ven en su barra (ver
/// `skin::TabBar`) y en las que cabe.
pub(super) fn close_marks(
    ui: &mut egui::Ui,
    bars: &[skin::TabBar],
    hits: &[skin::TabHit],
    modal: bool,
) -> Vec<PanelId> {
    let mut closed = Vec::new();
    for h in hits {
        let item = skin::tab_item(h.id);
        if item.mark == kit::tabs::TabMark::None || skin::is_dragged_tab(&h.response) {
            continue;
        }
        let Some(mark) = bars.iter().find_map(|b| {
            let i = b.tabs.iter().position(|&t| t == h.id)?;
            skin::tab_mark(b, i)
        }) else {
            continue;
        };
        let mark = mark.expand(2.0);
        let mut layer = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("workspace_close", h.id.key()))
                .layer_id(h.response.layer_id)
                .max_rect(mark),
        );
        layer.set_clip_rect(mark);
        let label = format!("{} {}", item.close.label(), item.label);
        let mut response = layer.interact(
            mark,
            egui::Id::new(("workspace_close", h.id.key())),
            egui::Sense::click(),
        );
        if !modal {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !modal, &label));
        if !modal && response.clicked() {
            closed.push(h.id);
        }
    }
    closed
}

/// Lo que corrió la rueda este frame bajo el puntero, en puntos (positivo: hacia atrás, como en
/// `kit::tabs::tab_strip`), con lo que es un renglón (ver `skin::Wheel`), si la movió. Con Ctrl
/// (o Cmd) es zoom: no cuenta.
pub(super) fn tab_wheel(ctx: &egui::Context) -> Option<skin::Wheel> {
    let at = ctx.input(|i| i.pointer.hover_pos())?;
    let notch = ctx.options(|o| o.input_options.line_scroll_speed);
    let (delta, zoom) = ctx.input(|i| {
        let zoom = |m: &egui::Modifiers| m.ctrl || m.command || m.mac_cmd;
        let mut delta = 0.0;
        let mut zooming = zoom(&i.modifiers);
        for e in &i.raw.events {
            if let egui::Event::MouseWheel {
                unit,
                delta: d,
                modifiers,
                ..
            } = e
            {
                let per = match unit {
                    egui::MouseWheelUnit::Point => 1.0,
                    egui::MouseWheelUnit::Line => notch,
                    egui::MouseWheelUnit::Page => i.viewport_rect().height(),
                };
                delta += (d.y + d.x) * per;
                zooming |= zoom(modifiers);
            }
        }
        (delta, zooming)
    });
    (delta != 0.0 && !zoom).then_some(skin::Wheel { at, delta, notch })
}

/// Las flechas del kit de cada barra que no cabe (ver `skin::TabBar`), en la capa de su barra: de
/// a una pestaña hacia atrás o hacia adelante (desde el frame siguiente). Antes, entre la última
/// que se ve y el «, un widget que se queda con el puntero: ahí egui_dock tiene lo que se pasa de
/// la siguiente (o lo de más de la última), que no se ve y no tiene que recibir el clic ni
/// arrastrarse. En una ventana flotante, arrastrarlo la mueve, como lo vacío al final de una barra
/// que cabe (que es de la ventana): devuelve la ventana y lo que se movió el puntero. Con un
/// diálogo modal, apagadas (y nada se mueve).
pub(super) fn tab_arrows(
    ui: &mut egui::Ui,
    skin: &Skin,
    bars: &[skin::TabBar],
    hits: &[skin::TabHit],
    modal: bool,
) -> Option<(SurfaceIndex, egui::Vec2)> {
    use kit::tabs::ARROW_SIZE;
    let mut moved = None;
    for bar in bars {
        let Some(arrows) = bar.arrows else { continue };
        let Some(layer) = bar
            .tabs
            .iter()
            .find_map(|&t| hits.iter().find(|h| h.id == t))
            .map(|h| h.response.layer_id)
        else {
            continue;
        };
        let last = bar.shown.last().map_or(bar.room.left(), |(_, r)| r.right());
        let rest = bar.room.with_min_x(last.min(arrows.left()));
        let mut ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("workspace_tab_arrows", bar.path.surface, bar.path.node))
                .layer_id(layer)
                .max_rect(rest),
        );
        ui.set_clip_rect(rest);
        let held = ui.interact(
            rest,
            egui::Id::new(("workspace_tab_rest", bar.path.surface, bar.path.node)),
            egui::Sense::click_and_drag(),
        );
        if !modal && !bar.path.surface.is_main() && held.dragged() {
            moved = Some((bar.path.surface, held.drag_delta()));
        }
        let halves = [
            egui::Rect::from_min_size(arrows.min, egui::Vec2::splat(ARROW_SIZE)),
            egui::Rect::from_min_size(
                arrows.min + egui::vec2(ARROW_SIZE - 1.0, 0.0),
                egui::Vec2::splat(ARROW_SIZE),
            ),
        ];
        let mut states = [(false, false); 2];
        let mut offset = bar.offset;
        for (i, (rect, label)) in halves
            .into_iter()
            .zip(["Previous tabs", "Next tabs"])
            .enumerate()
        {
            let can = !modal
                && if i == 0 {
                    bar.can_prev()
                } else {
                    bar.can_next()
                };
            let response = ui.interact(
                rect,
                egui::Id::new(("workspace_tab_arrow", bar.path.surface, bar.path.node, i)),
                egui::Sense::click(),
            );
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, can, label));
            states[i] = (can && response.hovered(), can);
            if can && response.clicked() {
                offset = if i == 0 {
                    offset.saturating_sub(1)
                } else {
                    (offset + 1).min(bar.last_offset)
                };
            }
        }
        // Solo en la primera pasada del frame: si se descarta, la segunda no recibe eventos
        // (`Context::run` le pasa una entrada vacía) y no habría clic; se deja por si eso cambia.
        if offset != bar.offset && ui.ctx().current_pass_index() == 0 {
            let memory = skin::tabs_offset_id(bar.path);
            let front = ui.data(|d| d.get_temp::<(usize, Option<PanelId>)>(memory));
            ui.data_mut(|d| d.insert_temp(memory, (offset, front.and_then(|f| f.1))));
            ui.ctx().request_repaint();
        }
        let shapes = kit::tabs::arrows_shapes(&ui, skin, arrows, states[0], states[1]);
        ui.painter().extend(shapes);
    }
    moved
}

/// La pestaña que egui_dock suelta este frame pasando la última que se ve de una barra con flechas
/// que muestra su última (ver `skin::TabBar`: ahí está lo de más de la última, `tail`, que no se
/// ve): egui_dock la pone antes de la última (suelta en una pestaña, va en su lugar), pero se ve
/// suelta en lo vacío del final de la barra, donde en una barra que cabe va al final. Devuelve la
/// que se suelta y la última de esa barra (para [`to_the_end`], después del `DockArea`).
pub(super) fn dropped_past_last(
    ctx: &egui::Context,
    bars: &[skin::TabBar],
) -> Option<(PanelId, PanelId)> {
    if !ctx.input(|i| i.pointer.primary_released()) {
        return None;
    }
    let id = dragged_tab(ctx)?;
    let p = ctx.input(|i| i.pointer.latest_pos())?;
    bars.iter().find_map(|b| {
        let last = *b.tabs.last()?;
        let row = b.row(last)?;
        (b.tail > 0.0 && b.room.contains(p) && p.x >= row.right()).then_some((id, last))
    })
}

/// Si egui_dock dejó `id` justo antes de `last`, la última de su grupo (ver
/// [`dropped_past_last`]), va al final; la de al frente sigue siendo la misma pestaña.
pub(super) fn to_the_end(state: &mut DockState<PanelId>, id: PanelId, last: PanelId) {
    let Some(path) = state.find_tab(&id) else {
        return;
    };
    let Ok(leaf) = state.leaf_mut(path.node_path()) else {
        return;
    };
    let n = leaf.tabs.len();
    if n < 2 || path.tab.0 + 2 != n || leaf.tabs[n - 1] != last {
        return;
    }
    let front = leaf.tabs.get(leaf.active.0).copied();
    leaf.tabs.swap(n - 2, n - 1);
    if let Some(i) = front.and_then(|f| leaf.tabs.iter().position(|&t| t == f)) {
        leaf.active = egui_dock::TabIndex(i);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use egui_dock::NodePath;

    use super::*;
    use crate::ChimpApp;
    use crate::workspace::DockTarget;
    use crate::workspace::test_util::{
        SIZE, click_at, drag_to, frame_in, frames_in, leaf, many_tabs_below_color, node_rect,
        snapshot, tabs_offset, test_app, wheel,
    };

    /// Color con siete paneles más en su grupo, en la columna: sus pestañas no caben.
    fn many_tabs(app: &mut ChimpApp) {
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        for id in [
            PanelId::Library,
            PanelId::Slices,
            PanelId::Artboards,
            PanelId::Lospec,
            PanelId::Groom,
            PanelId::Lumen,
            PanelId::Font,
        ] {
            app.workspace.dock(id, DockTarget::Leaf(s, n));
        }
        // Color al frente (la primera): las de la punta no se ven.
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        let path = NodePath {
            surface: s,
            node: n,
        };
        app.workspace.state.leaf_mut(path).unwrap().active = egui_dock::TabIndex(0);
    }

    /// Las × de las pestañas que se ven en `out`, de izquierda a derecha.
    fn close_marks_in(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
        let mut marks: Vec<(String, egui::Rect)> = crate::workspace::test_util::buttons(out)
            .into_iter()
            .filter(|l| l.starts_with("Close "))
            .filter_map(|l| node_rect(out, &l).map(|r| (l, r)))
            .collect();
        marks.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
        marks
    }

    #[test]
    fn many_tabs_in_a_narrow_group_page_with_the_kit_arrows() {
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut out = frames_in(&ctx, &mut app, SIZE);
        let group = leaf(&app, PanelId::Color);
        let collapse = node_rect(&out, COLLAPSE).expect("the « of the group");
        let prev = node_rect(&out, "Previous tabs").expect("the kit arrows");
        let next = node_rect(&out, "Next tabs").expect("the kit arrows");
        // Las del kit (12 × 12, juntas 23), en la fila de las flechas de la barra, a la izquierda
        // del « y sin tocarlo.
        assert_eq!(prev.size(), egui::Vec2::splat(kit::tabs::ARROW_SIZE));
        assert_eq!(next.left(), prev.left() + kit::tabs::ARROW_SIZE - 1.0);
        assert_eq!(prev.top(), group.top() + kit::tabs::ARROWS_TOP);
        assert!(next.right() < collapse.left(), "{next:?} {collapse:?}");
        let first = close_marks_in(&out);
        assert!(!first.is_empty(), "some tabs are seen");
        // De ida con la de adelante hasta que se ve la última (FontForge) y de vuelta con la de
        // atrás: cada × que se ve, entero antes de las flechas (con su aire) y nunca bajo el «.
        let check = |out: &egui::FullOutput, step: &str| {
            for (label, mark) in close_marks_in(out) {
                assert!(
                    mark.right() <= prev.left() - kit::tabs::GAP && !mark.intersects(collapse),
                    "{step}: {label} at {mark:?}, arrows at {prev:?}"
                );
            }
        };
        check(&out, "at first");
        let mut steps = 0;
        while node_rect(&out, "Close FontForge").is_none() {
            assert!(steps < 8, "the last tab comes into view");
            click_at(
                &ctx,
                &mut app,
                next.center(),
                egui::PointerButton::Primary,
                SIZE,
            );
            out = frames_in(&ctx, &mut app, SIZE);
            check(&out, &format!("forward {steps}"));
            steps += 1;
        }
        assert!(steps > 0, "it was not in view at first");
        // La última, a la vista: un clic en ella la pone al frente.
        let font = node_rect(&out, "Close FontForge").unwrap();
        let tab = egui::pos2(font.left() - 12.0, font.center().y);
        click_at(&ctx, &mut app, tab, egui::PointerButton::Primary, SIZE);
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        assert_eq!(
            app.workspace.state_leaf_tabs(s, n).unwrap().1,
            PanelId::Font
        );
        out = frames_in(&ctx, &mut app, SIZE);
        check(&out, "with the last one in front");
        // Y de vuelta al principio.
        for _ in 0..steps {
            click_at(
                &ctx,
                &mut app,
                prev.center(),
                egui::PointerButton::Primary,
                SIZE,
            );
        }
        out = frames_in(&ctx, &mut app, SIZE);
        check(&out, "back");
        assert_eq!(close_marks_in(&out), first, "the first ones again");
    }

    #[test]
    fn the_wheel_over_tabs_that_do_not_fit_pages_one_tab_per_notch() {
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        frames_in(&ctx, &mut app, SIZE);
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        let path = NodePath {
            surface: s,
            node: n,
        };
        let offset = |ctx: &egui::Context| {
            ctx.data(|d| d.get_temp::<(usize, Option<PanelId>)>(skin::tabs_offset_id(path)))
                .map(|(o, _)| o)
        };
        assert_eq!(offset(&ctx), Some(0));
        let bar = leaf(&app, PanelId::Color);
        let at = egui::pos2(bar.left() + 20.0, bar.top() + 6.0);
        let wheel = |dy: f32| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, dy),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        };
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        frame_in(&ctx, &mut app, vec![wheel(-1.0)], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(offset(&ctx), Some(1), "one tab forward");
        frame_in(&ctx, &mut app, vec![wheel(1.0)], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(offset(&ctx), Some(0), "and back");
    }

    #[test]
    fn a_tab_brought_to_the_front_is_paged_into_view() {
        // Window → panel (o Lospec) pone al frente una pestaña que no se ve: las flechas la traen.
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let out = frames_in(&ctx, &mut app, SIZE);
        assert!(node_rect(&out, "Close FontForge").is_none(), "out of view");
        app.workspace.bring_forward(&ctx, PanelId::Font);
        let out = frames_in(&ctx, &mut app, SIZE);
        let prev = node_rect(&out, "Previous tabs").unwrap();
        let mark = node_rect(&out, "Close FontForge").expect("in view");
        assert!(mark.right() <= prev.left() - kit::tabs::GAP, "{mark:?}");
    }

    /// Los botones de `out` dentro de `area`, con su rect.
    fn buttons_in(out: &egui::FullOutput, area: egui::Rect) -> Vec<(String, egui::Rect)> {
        crate::workspace::test_util::buttons(out)
            .into_iter()
            .filter_map(|l| node_rect(out, &l).map(|r| (l, r)))
            .filter(|(_, r)| area.contains_rect(*r))
            .collect()
    }

    #[test]
    fn the_wheel_over_a_floating_window_scrolls_it_and_not_the_tabs_below() {
        for pixel in [true, false] {
            let mut app = test_app();
            app.view.pixel_ui = pixel;
            many_tabs_below_color(&mut app);
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let out = frames_in(&ctx, &mut app, SIZE);
            if pixel {
                assert!(node_rect(&out, "Next tabs").is_some(), "the bar overflows");
            }
            let bar = leaf(&app, PanelId::Library);
            // La toolbar flotando encima de la barra, baja: lo suyo no cabe y se corre.
            app.workspace.float(
                PanelId::Tools,
                egui::pos2(bar.left() + 20.0, bar.top() - 70.0),
            );
            frames_in(&ctx, &mut app, SIZE);
            let (s, _) = app.workspace.leaf_of(PanelId::Tools).unwrap();
            let width = app
                .workspace
                .window_rect(&ctx, PanelId::Tools)
                .unwrap()
                .width();
            app.workspace
                .state
                .get_window_state_mut(s)
                .unwrap()
                .set_size(egui::vec2(width, 150.0));
            frames_in(&ctx, &mut app, SIZE);
            let out = frames_in(&ctx, &mut app, SIZE);
            let window = app.workspace.window_rect(&ctx, PanelId::Tools).unwrap();
            let at = egui::pos2(window.center().x, bar.top() + 6.0);
            assert!(window.shrink(8.0).contains(at), "{window:?} {at:?}");
            assert!(bar.contains(at) && at.x < bar.right() - 30.0, "{bar:?}");
            let before = buttons_in(&out, window);
            assert!(!before.is_empty(), "the toolbar's buttons");
            let offset = tabs_offset(&ctx, &app, PanelId::Library);
            assert_eq!(offset.is_some(), pixel, "the kit's bar only with Pixel UI");
            // Lo corrido de la barra en egui_dock (con Pixel UI, lo pone el kit): en los dos modos.
            let scroll = leaf_scroll(&app, PanelId::Library);
            frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
            let down = wheel(egui::MouseWheelUnit::Line, -2.0, Default::default());
            frame_in(&ctx, &mut app, vec![down], SIZE);
            frames_in(&ctx, &mut app, SIZE);
            let out = frames_in(&ctx, &mut app, SIZE);
            let scrolled = before
                .iter()
                .any(|(label, r)| node_rect(&out, label).is_some_and(|now| now.top() < r.top()));
            assert!(scrolled, "pixel {pixel}: the window's contents scroll");
            assert_eq!(
                tabs_offset(&ctx, &app, PanelId::Library),
                offset,
                "pixel {pixel}: the tabs below do not page"
            );
            assert_eq!(
                leaf_scroll(&app, PanelId::Library),
                scroll,
                "pixel {pixel}: the tabs below do not scroll"
            );
        }
    }

    /// Lo corrido de la barra de pestañas del grupo de `id` (`LeafNode::scroll` de egui_dock).
    fn leaf_scroll(app: &ChimpApp, id: PanelId) -> f32 {
        let (surface, node) = app.workspace.leaf_of(id).unwrap();
        let path = NodePath { surface, node };
        app.workspace.state.leaf(path).unwrap().scroll
    }

    #[test]
    fn the_wheel_pages_the_overflowing_bar_of_a_floating_window() {
        let mut app = test_app();
        app.workspace
            .float(PanelId::Library, egui::pos2(320.0, 120.0));
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        for id in [
            PanelId::Slices,
            PanelId::Artboards,
            PanelId::Lospec,
            PanelId::Groom,
            PanelId::Lumen,
            PanelId::Model,
            PanelId::Font,
        ] {
            app.workspace.dock(id, DockTarget::Leaf(s, n));
        }
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        frames_in(&ctx, &mut app, SIZE);
        let (s, _) = app.workspace.leaf_of(PanelId::Library).unwrap();
        let height = app
            .workspace
            .window_rect(&ctx, PanelId::Library)
            .unwrap()
            .height();
        app.workspace
            .state
            .get_window_state_mut(s)
            .unwrap()
            .set_size(egui::vec2(260.0, height));
        let out = frames_in(&ctx, &mut app, SIZE);
        assert!(node_rect(&out, "Next tabs").is_some(), "the bar overflows");
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        let path = NodePath {
            surface: s,
            node: n,
        };
        app.workspace.state.leaf_mut(path).unwrap().active = egui_dock::TabIndex(0);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Library), Some(0));
        let bar = leaf(&app, PanelId::Library);
        let at = egui::pos2(bar.left() + 20.0, bar.top() + 6.0);
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        let down = wheel(egui::MouseWheelUnit::Line, -1.0, Default::default());
        frame_in(&ctx, &mut app, vec![down], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Library), Some(1));
    }

    #[test]
    fn a_trackpad_pages_after_a_notch_and_ctrl_wheel_does_not_page() {
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        frames_in(&ctx, &mut app, SIZE);
        let bar = leaf(&app, PanelId::Color);
        let at = egui::pos2(bar.left() + 20.0, bar.top() + 6.0);
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Color), Some(0));
        // Un trackpad: de a 10 puntos por frame. Hasta juntar un renglón de la rueda, nada.
        let notch = ctx.options(|o| o.input_options.line_scroll_speed);
        let step = wheel(egui::MouseWheelUnit::Point, -10.0, Default::default());
        let frames = (notch / 10.0).ceil() as usize;
        for _ in 1..frames {
            frame_in(&ctx, &mut app, vec![step.clone()], SIZE);
        }
        assert_eq!(
            tabs_offset(&ctx, &app, PanelId::Color),
            Some(0),
            "less than a notch"
        );
        frame_in(&ctx, &mut app, vec![step], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(
            tabs_offset(&ctx, &app, PanelId::Color),
            Some(1),
            "a notch: one tab"
        );
        // Ctrl (o Cmd) con la rueda es zoom: no corre las pestañas.
        let zoom = wheel(egui::MouseWheelUnit::Line, -1.0, egui::Modifiers::COMMAND);
        frame_in(&ctx, &mut app, vec![zoom], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Color), Some(1), "zoom");
        let zoom = wheel(egui::MouseWheelUnit::Line, -1.0, egui::Modifiers::CTRL);
        frame_in(&ctx, &mut app, vec![zoom], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Color), Some(1), "zoom");
    }

    #[test]
    fn what_the_trackpad_left_on_a_bar_is_forgotten_when_the_pointer_leaves_it() {
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        frames_in(&ctx, &mut app, SIZE);
        let bar = leaf(&app, PanelId::Color);
        let at = egui::pos2(bar.left() + 20.0, bar.top() + 6.0);
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        // Casi un renglón (todo menos 10 puntos) y el puntero se va de la barra.
        let notch = ctx.options(|o| o.input_options.line_scroll_speed);
        let step = wheel(egui::MouseWheelUnit::Point, -10.0, Default::default());
        let frames = (notch / 10.0).ceil() as usize;
        for _ in 1..frames {
            frame_in(&ctx, &mut app, vec![step.clone()], SIZE);
        }
        let away = egui::pos2(bar.center().x, bar.bottom() - 20.0);
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(away)], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        // De vuelta, lo que falta del renglón no basta: empieza de cero.
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(at)], SIZE);
        frame_in(&ctx, &mut app, vec![step], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        assert_eq!(tabs_offset(&ctx, &app, PanelId::Color), Some(0));
    }

    /// Las pestañas de `out` hasta que se ve la última de `id` (FontForge), con la flecha de
    /// adelante; devuelve la salida con ella a la vista.
    fn page_to_the_end(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        mut out: egui::FullOutput,
    ) -> egui::FullOutput {
        let next = node_rect(&out, "Next tabs").expect("the kit arrows");
        let mut steps = 0;
        while node_rect(&out, "Close FontForge").is_none() {
            assert!(steps < 8, "the last tab comes into view");
            click_at(ctx, app, next.center(), egui::PointerButton::Primary, SIZE);
            out = frames_in(ctx, app, SIZE);
            steps += 1;
        }
        out
    }

    /// Lo vacío al final de una barra que no cabe con la última a la vista: entre ella y las
    /// flechas.
    fn empty_end(out: &egui::FullOutput) -> egui::Pos2 {
        let last = node_rect(out, "FontForge").expect("the last tab");
        let prev = node_rect(out, "Previous tabs").unwrap();
        assert!(prev.left() - last.right() > 2.0, "{last:?} {prev:?}");
        egui::pos2((last.right() + prev.left()) / 2.0, prev.center().y)
    }

    #[test]
    fn the_empty_end_of_an_overflowing_window_bar_still_drags_the_window() {
        let mut app = test_app();
        app.workspace
            .float(PanelId::Library, egui::pos2(320.0, 120.0));
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        for id in [
            PanelId::Slices,
            PanelId::Artboards,
            PanelId::Lospec,
            PanelId::Groom,
            PanelId::Lumen,
            PanelId::Model,
            PanelId::Font,
        ] {
            app.workspace.dock(id, DockTarget::Leaf(s, n));
        }
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        frames_in(&ctx, &mut app, SIZE);
        // Angosta: sus pestañas no caben.
        let (s, _) = app.workspace.leaf_of(PanelId::Library).unwrap();
        let height = app
            .workspace
            .window_rect(&ctx, PanelId::Library)
            .unwrap()
            .height();
        app.workspace
            .state
            .get_window_state_mut(s)
            .unwrap()
            .set_size(egui::vec2(260.0, height));
        let out = frames_in(&ctx, &mut app, SIZE);
        let out = page_to_the_end(&ctx, &mut app, out);
        let at = empty_end(&out);
        let window = app.workspace.window_rect(&ctx, PanelId::Library).unwrap();
        drag_to(&ctx, &mut app, (at, at + egui::vec2(40.0, 30.0)), SIZE);
        frames_in(&ctx, &mut app, SIZE);
        let moved = app.workspace.window_rect(&ctx, PanelId::Library).unwrap();
        assert_eq!(moved.min, window.min + egui::vec2(40.0, 30.0));
        // Ninguna pestaña se movió de grupo ni de lugar.
        let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
        assert_eq!(app.workspace.state_leaf_tabs(s, n).unwrap().0.len(), 8);
    }

    #[test]
    fn a_tab_dropped_past_the_last_tab_goes_to_the_end() {
        // Con flechas la última pestaña lleva en egui_dock lo que falta hasta ellas (`tail`): lo
        // que se suelta ahí va al final, como en lo vacío de una barra que cabe (no antes de ella).
        let mut app = test_app();
        many_tabs(&mut app);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let out = frames_in(&ctx, &mut app, SIZE);
        let out = page_to_the_end(&ctx, &mut app, out);
        let at = empty_end(&out);
        // La franja de agarre de la toolbar (sola en su grupo).
        let tools = leaf(&app, PanelId::Tools);
        let from = egui::pos2(tools.center().x, tools.top() + 1.0);
        // Como `drag_to`, pero se queda un rato en `at` antes de soltar: egui_dock retiene un
        // momento el destino de antes (el lienzo, por donde pasó).
        let press = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(from)], SIZE);
        frame_in(&ctx, &mut app, vec![press(from, true)], SIZE);
        for k in 1..=40 {
            let to = from + (at - from) * (k as f32 / 4.0).min(1.0);
            frame_in(&ctx, &mut app, vec![egui::Event::PointerMoved(to)], SIZE);
        }
        frame_in(&ctx, &mut app, vec![press(at, false)], SIZE);
        frames_in(&ctx, &mut app, SIZE);
        let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
        let tabs = app.workspace.state_leaf_tabs(s, n).unwrap().0;
        assert_eq!(tabs.len(), 9, "{tabs:?} {}", snapshot(&ctx, &app));
        assert_eq!(tabs.last(), Some(&PanelId::Tools), "{tabs:?}");
        assert_eq!(tabs[7], PanelId::Font, "{tabs:?}");
    }
}
