//! El workspace en pantalla: [`show`] arma el `DockArea` con la piel, hospeda cada panel
//! ([`Viewer`]) y pega lo que va encima de egui_dock (las pestañas y la × del kit, la franja de
//! agarre y el botón « de cada grupo), y le reporta al anfitrión su ancho ([`host_width`]).

use std::collections::HashMap;

use eframe::egui;
use egui_dock::{DockArea, DockState, TabViewer};

use super::drop::tab_drag_released;
use super::fit::{chrome, fit_splits, fit_windows, minima};
use super::{NodeIndex, SurfaceIndex, Workspace, dock_id, skin};
use crate::ChimpApp;
use crate::app_panels::{PanelAction, PanelCtx};
use crate::panel_registry::{Body, PanelId};
use crate::skin::Skin;
use crate::{dock, kit, pixel_ui};

/// Lo que dice la pestaña del panel 3D (acoplado o flotando) en la pestaña de un modelo, donde lo
/// hospeda su panel fijo (`"model_panel"`).
pub(crate) const MODEL_ELSEWHERE: &str = "Shown in the 3D panel";

/// Lo que egui_dock ve de los paneles: los dibuja con [`ChimpApp::draw_panel`] y junta lo que
/// piden (se aplica después del `DockArea`, con la app entera).
struct Viewer<'a> {
    app: &'a mut ChimpApp,
    modal: bool,
    /// Con Pixel UI: las pestañas del kit van encima (las de egui_dock, transparentes).
    skinned: bool,
    /// El ancho de la pestaña del kit de cada panel.
    widths: &'a HashMap<PanelId, f32>,
    /// El margen de cada panel en su grupo: el del panel derecho de hoy para Color, el de los
    /// paneles del dock para los demás.
    color_margin: egui::Margin,
    actions: Vec<PanelAction>,
    hits: Vec<skin::TabHit>,
    /// Los que egui_dock cerró (sin Pixel UI, con su ×): van a la tira.
    closed: Vec<PanelId>,
    /// Lo que ocupó a lo ancho el contenido de cada panel dibujado (ver [`host_width`]).
    extents: Vec<Extent>,
    /// El estilo del `Ui` anfitrión: egui_dock arma el cuerpo de cada pestaña con `Ui::new`, que
    /// toma el del contexto, y en el frame en que Pixel UI cambia el estilo los dos no coinciden
    /// (el panel derecho de antes usaba el del anfitrión, y de ese frame sale su ancho).
    style: std::sync::Arc<egui::Style>,
}

impl TabViewer for Viewer<'_> {
    type Tab = PanelId;

    fn id(&mut self, tab: &mut PanelId) -> egui::Id {
        egui::Id::new(("workspace_tab", tab.key()))
    }

    /// Con Pixel UI, el título casi sin ancho (fuente de 1 pt, transparente): el ancho de la
    /// pestaña lo pone `minimum_width` (el del kit) y no el texto más los 16 de aire de egui_dock.
    /// El nombre para los lectores de pantalla va en `on_tab_button`... después del `DockArea`.
    fn title(&mut self, tab: &mut PanelId) -> egui::WidgetText {
        let title = tab.spec().title;
        if self.skinned {
            egui::RichText::new(title)
                .size(1.0)
                .color(egui::Color32::TRANSPARENT)
                .into()
        } else {
            title.into()
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut PanelId) {
        let id = *tab;
        let spacing = ui.spacing().item_spacing;
        ui.set_style(self.style.clone());
        ui.spacing_mut().item_spacing = spacing;
        // Una instancia por panel por frame (su estado y los ids de `forms` son globales): en la
        // pestaña de un modelo, al 3D lo hospeda su panel fijo; aquí, solo dónde está.
        if id == PanelId::Model && self.app.doc.editor.model_edit_on() {
            ui.add_enabled(false, egui::Label::new(MODEL_ELSEWHERE));
            return;
        }
        // El fondo del grupo (dentro del scroll de egui_dock el `max_rect` puede no serlo): hasta
        // ahí llega la lista de Lospec.
        let bottom = ui.clip_rect().bottom().min(ui.max_rect().bottom()).floor();
        // El cuerpo (sin el margen), para comparar con lo que ocupa el contenido.
        let body = ui.max_rect();
        let panel = PanelCtx {
            modal: self.modal,
            bottom: Some(bottom),
        };
        // En un `scope`: su rect es lo que ocupó el contenido (el `Ui` del cuerpo ya viene
        // agrandado al cuerpo entero).
        let app = &mut *self.app;
        let inner = ui.scope(|ui| match id.spec().body {
            Body::Fixed => {
                ui.set_min_height(ui.available_height());
                app.draw_panel(ui, id, &panel)
            }
            Body::Fit | Body::Tall => {
                let area = egui::ScrollArea::vertical()
                    .id_salt(("workspace_scroll", id.key()))
                    .auto_shrink([false, false]);
                pixel_ui::scroll(ui, area, |ui| {
                    exact_width(ui, id, |ui| app.draw_panel(ui, id, &panel))
                })
            }
        });
        self.actions.extend(inner.inner);
        // Solo de Color, el que vivía en el panel derecho: de lo que ocupa sale el ancho de
        // fábrica de la columna (ver `host_width`). Los demás no la ensanchan: la columna ya
        // respeta su mínimo.
        if id == PanelId::Color {
            self.extents.push(Extent {
                id,
                delta: inner.response.rect.right() - body.right(),
            });
        }
    }

    fn on_tab_button(&mut self, tab: &mut PanelId, response: &egui::Response) {
        self.hits.push(skin::TabHit {
            id: *tab,
            response: response.clone(),
        });
    }

    fn on_close(&mut self, tab: &mut PanelId) -> egui_dock::tab_viewer::OnCloseResponse {
        self.closed.push(*tab);
        egui_dock::tab_viewer::OnCloseResponse::Close
    }

    fn is_closeable(&self, tab: &PanelId) -> bool {
        tab.spec().closable
    }

    fn tab_style_override(
        &self,
        tab: &PanelId,
        global: &egui_dock::TabStyle,
    ) -> Option<egui_dock::TabStyle> {
        let mut style = global.clone();
        if self.skinned {
            style.minimum_width = self.widths.get(tab).copied();
        }
        style.tab_body.inner_margin = if *tab == PanelId::Color {
            self.color_margin
        } else {
            egui::Margin::same(dock::FLYOUT_MARGIN)
        };
        Some(style)
    }

    fn scroll_bars(&self, _tab: &PanelId) -> [bool; 2] {
        // El scroll es de cada panel (el del kit, o el suyo propio).
        [false, false]
    }
}

/// El contenido de un panel con scroll, justo al ancho que le queda en `ui` (el del grupo sin la
/// barra): un `ScrollArea` crece a lo ancho de su contenido y, con él, su barra se iría fuera del
/// grupo. Lo que se pase se recorta (no debería: la columna respeta su mínimo).
fn exact_width<R>(ui: &mut egui::Ui, id: PanelId, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let width = ui.available_width();
    let start = ui.cursor().min;
    let max = egui::Rect::from_min_size(start, egui::vec2(width, ui.available_height()));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("workspace_body", id.key()))
            .max_rect(max),
    );
    let out = add(&mut child);
    let height = child.min_rect().bottom() - start.y;
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(start, egui::vec2(width, height)));
    out
}

/// Lo que el contenido de un panel dibujado se pasó a lo ancho del borde derecho de su cuerpo
/// (negativo si le sobró lugar).
#[derive(Clone, Copy, Debug)]
struct Extent {
    id: PanelId,
    delta: f32,
}

/// El ancho que el workspace le reporta a su anfitrión (desde su borde izquierdo), un
/// `egui::Panel` que se ajusta a lo que ocupa su contenido: de ese ajuste sale el ancho de
/// fábrica de la columna en los primeros frames, cuando la escala y la fuente de Pixel UI todavía
/// cambian. Como el panel derecho de antes con Color adentro (que en esos frames quedaba igual en
/// un ancho entero): el ancho en puntos enteros del `DockArea` (`dock_width`) más lo que se pase
/// el contenido de un panel acoplado; si el rect del anfitrión no era de puntos enteros
/// (`whole` falso, un frame de transición), redondeado hacia arriba. Si al contenido le sobra
/// lugar no encoge: un panel angosto no achica la columna.
fn host_width(dock_width: f32, whole: bool, docked: impl Iterator<Item = Extent>) -> f32 {
    let width = dock_width + docked.map(|e| e.delta).fold(0.0, f32::max);
    if whole { width } else { width.ceil() }
}

/// El workspace en `ui`: el `DockArea` con la piel (con Pixel UI) y cada panel con
/// [`ChimpApp::draw_panel`]. Lo que piden los paneles se aplica al final, con la app entera;
/// cerrar una pestaña la regresa a la tira. Con un diálogo modal (`modal`), los paneles quedan
/// apagados y las pestañas no se arrastran.
///
/// Mientras los paneles se dibujan, `app.workspace` es un acomodo de relleno (el de verdad lo
/// tiene `show`): cambiarlo desde un panel no serviría, por eso sus operaciones lo revisan en
/// debug. Lo que haya que cambiar del acomodo se pide como acción y se hace después.
pub(crate) fn show(app: &mut ChimpApp, ui: &mut egui::Ui, modal: bool) {
    let mut workspace = std::mem::replace(&mut app.workspace, Workspace::placeholder());
    workspace.tidy();
    workspace.place_restored_windows(ui.ctx());
    workspace.refresh_tab_bars();
    let shared_skin = app.pixel.shared_skin();
    let skin: Option<&Skin> = shared_skin.as_deref();
    let widths: HashMap<PanelId, f32> = match skin {
        Some(_) => PanelId::ALL
            .iter()
            .map(|&p| (p, kit::tabs::natural_width(ui, &skin::tab_item(p))))
            .collect(),
        None => HashMap::new(),
    };
    let style = match skin {
        Some(skin) => skin::dock_style(ui.style(), skin),
        None => skin::plain_style(ui.style()),
    };
    let mut viewer = Viewer {
        app,
        modal,
        skinned: skin.is_some(),
        widths: &widths,
        color_margin: egui::Frame::side_top_panel(ui.style()).inner_margin,
        actions: Vec::new(),
        hits: Vec::new(),
        closed: Vec::new(),
        extents: Vec::new(),
        style: ui.style().clone(),
    };
    let area = ui.available_rect_before_wrap();
    // El `DockArea`, de un ancho de puntos enteros (egui_dock solo ajusta a píxel físico) y en un
    // `Ui` aparte (ocupa todo su rect): al anfitrión se le reporta el ancho de lo acoplado (ver
    // `host_width`).
    let dock_rect = egui::Rect::from_x_y_ranges(
        area.left().ceil()..=area.right().floor().max(area.left().ceil()),
        area.y_range(),
    );
    {
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(dock_rect));
        // El marco de las ventanas flotantes (solo lo leen los marcos).
        if let Some(skin) = skin {
            skin::window_visuals(ui.style_mut(), skin);
        }
        DockArea::new(&mut workspace.state)
            .id(dock_id())
            .style(style)
            // Con Pixel UI, la × es la del kit (abajo); sin él, la de egui_dock.
            .show_close_buttons(skin.is_none())
            .show_add_buttons(false)
            .tab_context_menus(false)
            .show_leaf_close_all_buttons(false)
            .show_leaf_collapse_buttons(false)
            .secondary_button_on_modifier(false)
            .secondary_button_context_menu(false)
            .show_secondary_button_hint(false)
            .show_tab_name_on_hover(false)
            .hidable_tab_bars(true)
            .draggable_tabs(!modal)
            .show_inside(ui, &mut viewer);
    }
    let Viewer {
        app,
        actions,
        hits,
        mut closed,
        extents,
        ..
    } = viewer;
    let docked = extents.into_iter().filter(|e| workspace.docked(e.id));
    let whole = area.left() == dock_rect.left() && area.right() == dock_rect.right();
    let width = host_width(dock_rect.width(), whole, docked);
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(
        area.min,
        egui::vec2(width, area.height()),
    ));
    // El nombre de cada pestaña para los lectores de pantalla (el título es de 1 pt) y, con
    // Pixel UI, las del kit encima y su ×.
    let active: Vec<PanelId> = workspace
        .state
        .iter_leaves()
        .filter_map(|(_, leaf)| leaf.tabs.get(leaf.active.0).copied())
        .collect();
    for h in &hits {
        let title = h.id.spec().title;
        let selected = active.contains(&h.id);
        h.response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, !modal, selected, title)
        });
    }
    if let Some(skin) = skin {
        skin::paint_tabs(ui, skin, &workspace.state, &hits, dock_id());
        closed.extend(close_marks(ui, &workspace.state, &hits, modal));
    }
    // Dónde puede haber marcadores este frame: el DockArea y sus ventanas, fuera del cuerpo de los
    // grupos (ver `skin::Armed`).
    if let Some(skin) = &shared_skin {
        let mut armed = skin::Armed::new(skin.clone());
        armed.areas.push(area);
        for (s, _) in workspace.state.iter_surfaces_indexed() {
            if !s.is_main()
                && let Some(rect) = ui.ctx().memory(|m| m.area_rect(skin::window_area_id(s)))
            {
                armed.areas.push(rect);
            }
        }
        armed.bodies = workspace
            .state
            .iter_leaves()
            .map(|(_, leaf)| leaf.viewport)
            .filter(|r| r.is_positive())
            .collect();
        skin::arm(ui.ctx(), armed);
    }
    for id in closed {
        workspace.to_strip(id);
    }
    // Una pestaña (o un grupo sin barra) de egui_dock soltada fuera de la columna y de las
    // ventanas: egui_dock no hace nada (solo suelta sobre sus grupos y su `DockArea` es la
    // columna); aquí se pone a flotar donde se soltó, como un ícono de la tira sobre el lienzo.
    let windows: Vec<egui::Rect> = workspace
        .state
        .iter_surfaces_indexed()
        .filter(|(s, _)| !s.is_main())
        .filter_map(|(s, _)| ui.ctx().memory(|m| m.area_rect(skin::window_area_id(s))))
        .collect();
    let outside = |p: egui::Pos2| !dock_rect.contains(p) && !windows.iter().any(|r| r.contains(p));
    if let Some((id, at)) = tab_drag_released(ui.ctx(), &workspace.state, &hits)
        && outside(at)
    {
        workspace.float(id, at);
    }
    if let Some((surface, node)) = collapse_buttons(ui, skin, &workspace, modal) {
        workspace.collapse_to_icons(surface, node);
    }
    // Si un panel se soltó en otro grupo, la barra de algún grupo cambia: este frame ya se dibujó
    // con la de antes, así que se descarta y se vuelve a dibujar (sin el parpadeo de un frame).
    if workspace.refresh_tab_bars() {
        ui.ctx().request_discard("tab bar visibility changed");
    }
    // Cada split con sus lados en su mínimo, y a puntos enteros, para el frame siguiente.
    let (sep, bar) = chrome(ui, skin);
    let m = minima(ui, skin);
    fit_splits(&mut workspace.state, &m, sep, bar);
    fit_windows(ui.ctx(), &mut workspace, &m, sep, bar);
    skin::snap(ui.ctx(), &mut workspace.state);
    skin::snap_windows(ui.ctx(), &mut workspace.state);
    app.workspace = workspace;
    for action in actions {
        app.apply_panel_action(ui.ctx(), action);
    }
}

/// Si `shape` (o lo que lleva adentro) tiene un rect con un color marcador de la piel: el plugin
/// tenía que cambiarlo por una pieza del skin.
#[cfg(test)]
pub(crate) fn has_marker(shape: &egui::Shape) -> bool {
    match shape {
        egui::Shape::Vec(v) => v.iter().any(has_marker),
        egui::Shape::Rect(r) => {
            skin::MARKERS.contains(&r.fill) || skin::MARKERS.contains(&r.stroke.color)
        }
        _ => false,
    }
}

/// El nombre del botón « de un grupo acoplado (su tooltip y su etiqueta accesible).
pub(crate) const COLLAPSE: &str = "Collapse to Icons";

/// El botón « ("Collapse to Icons") de cada grupo acoplado que tenga algo que mandar a la tira
/// (ver [`Workspace::collapsible`]), la flecha de atrás de las pestañas del kit: al final de su
/// barra de pestañas o, en un grupo sin barra, en la punta derecha de su franja de agarre mientras
/// el puntero anda por la franja o por el botón (y nada se arrastra). Devuelve el grupo cuyo
/// botón se apretó.
fn collapse_buttons(
    ui: &mut egui::Ui,
    skin: Option<&Skin>,
    w: &Workspace,
    modal: bool,
) -> Option<(SurfaceIndex, NodeIndex)> {
    use crate::pixel_ui::Tip as _;
    let ctx = ui.ctx().clone();
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let dragging = ctx.dragged_id().is_some() && ctx.input(|i| i.pointer.is_decidedly_dragging());
    // Sin otra capa encima (una ventana flotante, un menú).
    let ours = |p: egui::Pos2| ctx.layer_id_at(p).is_none_or(|l| l == ui.layer_id());
    let size = egui::Vec2::splat(kit::tabs::ARROW_SIZE);
    let mut pressed = None;
    for (path, leaf) in w.state.iter_leaves() {
        if !path.surface.is_main()
            || !leaf.rect.is_positive()
            || !w.collapsible(path.surface, path.node)
        {
            continue;
        }
        // En la barra de pestañas, en el lugar que ella le deja a la derecha (`COLLAPSE_ROOM`);
        // sin barra, a la izquierda de la barra de scroll del panel (no encima de ella).
        let right = if leaf.tab_bar_hidden {
            leaf.rect.right() - f32::from(dock::FLYOUT_MARGIN) - pixel_ui::SCROLL_BAR - 1.0
        } else {
            leaf.rect.right() - 1.0
        };
        let at = egui::pos2(right - size.x, leaf.rect.top() + kit::tabs::ARROWS_TOP);
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
        let response = ui
            .interact(rect, id, egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
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
/// devuelve los paneles cuya × se apretó. Solo las que se ven enteras en su barra (con muchas
/// pestañas, las de la punta quedan cortadas o fuera).
fn close_marks(
    ui: &mut egui::Ui,
    state: &DockState<PanelId>,
    hits: &[skin::TabHit],
    modal: bool,
) -> Vec<PanelId> {
    let mut closed = Vec::new();
    for h in hits {
        let item = skin::tab_item(h.id);
        if item.mark == kit::tabs::TabMark::None || skin::is_dragged_tab(&h.response) {
            continue;
        }
        let shown = state
            .find_tab(&h.id)
            .and_then(|path| state.leaf(path.node_path()).ok())
            .is_some_and(|leaf| {
                let mark = kit::tabs::mark_rect(skin::tab_row(h.response.rect));
                skin::visible_tabs(leaf.rect).contains_rect(mark)
            });
        if !shown {
            continue;
        }
        let mark = kit::tabs::mark_rect(skin::tab_row(h.response.rect)).expand(2.0);
        let mut layer = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("workspace_close", h.id.key()))
                .layer_id(h.response.layer_id)
                .max_rect(mark),
        );
        layer.set_clip_rect(mark);
        let label = format!("{} {}", item.close.label(), item.label);
        let response = layer
            .interact(
                mark,
                egui::Id::new(("workspace_close", h.id.key())),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !modal, &label));
        if !modal && response.clicked() {
            closed.push(h.id);
        }
    }
    closed
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use egui_dock::NodePath;

    use super::*;
    use crate::workspace::test_util::{button, click, frame, frames, test_app};
    use crate::workspace::{DockTarget, Workspace};

    #[test]
    fn the_host_fits_the_docked_contents_like_the_old_right_panel() {
        let e = |delta| Extent {
            id: PanelId::Color,
            delta,
        };
        // Se pasa: crece lo que se pasó.
        assert_eq!(host_width(219.0, true, [e(43.875)].into_iter()), 262.875);
        // Le sobra (o nada se dibujó): el ancho del DockArea, sin encoger.
        assert_eq!(host_width(263.0, true, [e(-40.0)].into_iter()), 263.0);
        assert_eq!(host_width(263.0, true, std::iter::empty()), 263.0);
        // Con el anfitrión en una fracción de punto: a puntos enteros, hacia arriba.
        assert_eq!(
            host_width(262.0, false, [e(-40.0), e(0.875)].into_iter()),
            263.0
        );
        assert_eq!(host_width(263.0, false, [e(0.0)].into_iter()), 263.0);
    }

    #[test]
    fn show_hides_the_bar_of_a_lone_panel_and_puts_kit_tabs_on_two() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        // Essentials: Color solo, sin barra (ni pestaña).
        let out = frames(&ctx, &mut app);
        assert!(app.pixel.skin().is_some(), "Pixel UI on");
        assert!(app.workspace.docked(PanelId::Color));
        assert!(app.workspace.tab_bar_hidden_of(PanelId::Color));
        assert!(button(&out, "Color").is_none(), "no tab for a lone panel");
        // Con Library al lado: las dos pestañas (y la × de Library; Color no se cierra).
        app.workspace.dock(PanelId::Library, DockTarget::Focused);
        let out = frames(&ctx, &mut app);
        assert!(!app.workspace.tab_bar_hidden_of(PanelId::Color));
        assert!(button(&out, "Color").is_some(), "the Color tab");
        assert!(button(&out, "Library").is_some(), "the Library tab");
        assert!(button(&out, "Close Color").is_none());
        // La × del kit la regresa a la tira.
        let close = button(&out, "Close Library").expect("Library's ×");
        frame(&ctx, &mut app, vec![click(close)]);
        frame(&ctx, &mut app, Vec::new());
        assert!(!app.workspace.docked(PanelId::Library));
        assert!(app.workspace.strip().contains(&PanelId::Library));
        assert_eq!(app.workspace.strip(), PanelId::DOCK);
    }

    #[test]
    fn a_split_and_a_floating_window_are_dressed_without_markers() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        for name in ["split", "float"] {
            app.workspace = Workspace::demo(name).unwrap();
            let out = frames(&ctx, &mut app);
            fn has_marker(shape: &egui::Shape) -> bool {
                match shape {
                    egui::Shape::Vec(v) => v.iter().any(has_marker),
                    egui::Shape::Rect(r) => {
                        skin::MARKERS.contains(&r.fill) || skin::MARKERS.contains(&r.stroke.color)
                    }
                    _ => false,
                }
            }
            assert!(
                !out.shapes.iter().any(|c| has_marker(&c.shape)),
                "{name}: every marker became a skin piece"
            );
        }
        // La ventana flotante, en puntos enteros.
        let (surface, _) = app.workspace.leaf_of(PanelId::Lumen).unwrap();
        let rect = ctx
            .memory(|m| m.area_rect(skin::window_area_id(surface)))
            .expect("Lumen's window");
        assert_eq!(rect.min, rect.min.round());
    }

    #[test]
    fn the_grab_strip_of_a_lower_lone_panel_wins_over_the_separator() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        // Groom solo, debajo de Color: los dos sin barra, con un divisor entre ellos.
        app.workspace = Workspace::demo("split").unwrap();
        let out = frames(&ctx, &mut app);
        let (surface, node) = app.workspace.leaf_of(PanelId::Groom).unwrap();
        let path = NodePath { surface, node };
        let lower = app.workspace.state.leaf(path).unwrap().rect;
        assert!(app.workspace.tab_bar_hidden_of(PanelId::Groom));
        assert!(!skin::hidden_strip_hovered(&ctx, dock_id(), path));
        assert_ne!(out.platform_output.cursor_icon, egui::CursorIcon::Grab);
        // El puntero un punto debajo del borde de arriba del grupo: la franja (no el divisor).
        let at = egui::pos2(lower.center().x, lower.top() + 1.0);
        frame(&ctx, &mut app, vec![egui::Event::PointerMoved(at)]);
        let out = frame(&ctx, &mut app, Vec::new());
        assert!(
            skin::hidden_strip_hovered(&ctx, dock_id(), path),
            "the strip is the hovered widget"
        );
        assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::Grab);
        // Y se arrastra desde ahí (no cambia el tamaño del split).
        let press = egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        };
        frame(&ctx, &mut app, vec![press]);
        for dy in [4.0, 12.0, 24.0] {
            let to = at + egui::vec2(0.0, dy);
            frame(&ctx, &mut app, vec![egui::Event::PointerMoved(to)]);
        }
        assert!(skin::hidden_strip_dragged(&ctx, dock_id(), path));
        assert_eq!(app.workspace.state.leaf(path).unwrap().rect, lower);
    }

    #[test]
    fn a_panel_action_from_a_tab_is_applied_after_the_dock() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        // Artboards junto a Color (activa): su botón Canvas Size abre el diálogo.
        app.workspace.dock(PanelId::Artboards, DockTarget::Focused);
        let out = frames(&ctx, &mut app);
        let canvas_size = button(&out, "Canvas Size").expect("Canvas Size");
        assert!(app.canvas_size.is_none());
        frame(&ctx, &mut app, vec![click(canvas_size)]);
        assert!(app.canvas_size.is_some(), "OpenCanvasSize applied");
        // Y el workspace volvió a la app.
        assert!(app.workspace.docked(PanelId::Artboards));
    }
}
