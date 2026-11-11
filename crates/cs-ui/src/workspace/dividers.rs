//! Los divisores que [`super::show::show`] viste encima de egui_dock: el de la toolbar sola junto
//! al lienzo ([`cover_tools_divider`]) y el del timeline justo debajo de él
//! ([`cover_timeline_divider`]) se ven como el borde de su panel de antes, y la línea de cada
//! divisor vertical que llega a la barra de opciones la cruza ([`paint_crossings`]).

use eframe::egui;
use egui_dock::DockState;

use super::skin;
use crate::panel_registry::PanelId;
use crate::skin::Skin;

/// El divisor entre la toolbar sola y el lado del lienzo (`divider`; `first`: la toolbar a la
/// izquierda) se ve como el borde del panel izquierdo de antes: con Pixel UI, nada (el fondo, como
/// el aire que había); sin él, el fondo y la línea de 1 punto de egui del lado del lienzo. Y no se
/// arrastra: encima de su zona va un widget que se queda con el puntero (registrado después que
/// el divisor de egui_dock, le gana; sin el cursor de cambiar el tamaño). Su lado mide siempre el
/// ancho de la toolbar (`pin_sides`).
///
/// El `DockArea` va a puntos enteros y el área puede terminar en medio punto (sin Pixel UI, la
/// barra de estado): el panel de antes llegaba hasta `bottom`, así que si el divisor llega al
/// fondo del `DockArea` (le falta menos de un punto para `bottom`), lo de debajo de la toolbar
/// (`leaf`) y de su divisor hasta ahí también lleva su fondo (y su línea). Si no llega (la toolbar
/// junto al lienzo encima de otro grupo), nada más abajo: es de ese grupo.
pub(super) fn cover_tools_divider(
    ui: &mut egui::Ui,
    skin: Option<&Skin>,
    (leaf, divider): (egui::Rect, egui::Rect),
    first: bool,
    bottom: f32,
) {
    let ppp = ui.ctx().pixels_per_point();
    // Donde lo pinta egui_dock (`show_separator`): a píxel físico.
    let x = |v: f32| (v * ppp).round() / ppp;
    let bottom = if bottom - divider.bottom() < 1.0 {
        bottom.max(divider.bottom())
    } else {
        divider.bottom()
    };
    let rect = egui::Rect::from_x_y_ranges(
        x(divider.left())..=x(divider.right()),
        divider.top()..=bottom,
    );
    ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
    if bottom > divider.bottom() {
        let below = egui::Rect::from_x_y_ranges(
            x(leaf.left().min(divider.left()))..=x(leaf.right().max(divider.right())),
            divider.bottom()..=bottom,
        );
        ui.painter()
            .rect_filled(below, 0.0, ui.visuals().panel_fill);
    }
    if skin.is_none() {
        // Como `egui::Panel` con `show_separator_line`: la línea en el borde de afuera del panel.
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        let at = if first {
            rect.right() - stroke.width / 2.0
        } else {
            rect.left() + stroke.width / 2.0
        };
        ui.painter().vline(at, rect.y_range(), stroke);
    }
    let zone = divider.expand2(egui::vec2(skin::SEPARATOR_EXTRA / 2.0, 0.0));
    let _ = ui.interact(
        zone,
        egui::Id::new("workspace_tools_divider"),
        egui::Sense::click_and_drag(),
    );
}

/// El divisor entre el lienzo y el timeline justo debajo (`divider`) se ve, en reposo, como el
/// borde del panel de abajo de antes: con Pixel UI, nada (el fondo: era el aire de abajo del
/// lienzo); sin él, el fondo y la línea de 1 punto de egui arriba, a lo ancho del panel (sin la
/// tira de `strip` de ancho, que va encima). Con el puntero encima o arrastrándolo (egui_dock pone
/// el cursor de cambiar el alto) se ve como los demás divisores. Se arrastra (es el alto del
/// timeline: ver `record_sides`).
pub(super) fn cover_timeline_divider(
    ui: &mut egui::Ui,
    skin: Option<&Skin>,
    divider: egui::Rect,
    strip: f32,
) {
    let ctx = ui.ctx().clone();
    let resizing = ctx.output(|o| o.cursor_icon == egui::CursorIcon::ResizeVertical);
    let near = ctx.input(|i| i.pointer.latest_pos()).is_some_and(|p| {
        divider.x_range().contains(p.x) && (p.y - divider.center().y).abs() <= 4.0
    });
    if resizing && near {
        return;
    }
    let ppp = ctx.pixels_per_point();
    // Donde lo pinta egui_dock (`show_separator`): a píxel físico.
    let y = |v: f32| (v * ppp).round() / ppp;
    let rect =
        egui::Rect::from_x_y_ranges(divider.x_range(), y(divider.top())..=y(divider.bottom()));
    ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
    if skin.is_none() {
        // Como `egui::Panel::bottom` con `show_separator_line`: la línea en el borde de arriba del
        // panel, por dentro.
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        let at = rect.top() + stroke.width / 2.0;
        ui.painter()
            .hline(rect.left()..=rect.right() - strip, at, stroke);
    }
}

/// La línea oscura de cada divisor vertical de la superficie principal que llega arriba del
/// `DockArea` (`area`), un punto más arriba: cruza el brillo de la línea de la barra de opciones
/// (como el borde del panel derecho de antes). El de la toolbar (`quiet`) no: no se ve (el panel
/// izquierdo de antes no tenía línea).
pub(super) fn paint_crossings(
    ui: &egui::Ui,
    skin: &Skin,
    state: &DockState<PanelId>,
    area: egui::Rect,
    sep: f32,
    quiet: Option<egui::Rect>,
) {
    let ppp = ui.ctx().pixels_per_point();
    for node in state.main_surface().iter() {
        let egui_dock::Node::Horizontal(split) = node else {
            continue;
        };
        if split.rect.top() != area.top() || !split.rect.is_positive() {
            continue;
        }
        // Donde la pinta egui_dock (`show_separator`): su borde izquierdo, a píxel físico.
        let mid = split.rect.left() + split.rect.width() * split.fraction;
        let left = ((mid - sep / 2.0) * ppp).round() / ppp;
        if quiet.is_some_and(|q| (q.center().x - mid).abs() < 0.5) {
            continue;
        }
        let cross =
            egui::Rect::from_min_size(egui::pos2(left, area.top() - 1.0), egui::vec2(1.0, 1.0));
        ui.painter().add(egui::Shape::rect_filled(
            eframe::emath::GuiRounding::round_to_pixels(cross, ppp),
            0.0,
            skin.colors().canvas,
        ));
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use crate::ChimpApp;
    use crate::dock::Edge;
    use crate::panel_registry::PanelId;
    use crate::workspace::test_util::{frames, test_app};

    #[test]
    fn the_crossings_are_not_painted_over_the_top_strip() {
        // Los cruces del divisor de la columna con la barra de opciones van un punto arriba del
        // `DockArea`: con la tira de arriba, ese renglón es el de abajo de la tira.
        let crossings = |app: &mut ChimpApp| {
            let ctx = egui::Context::default();
            let out = frames(&ctx, app);
            let color = app.pixel.shared_skin().unwrap().colors().canvas;
            let y = app.workspace.leaf_rect(PanelId::Color).unwrap().top() - 1.0;
            fn rects(shape: &egui::Shape, out: &mut Vec<(egui::Color32, egui::Rect)>) {
                match shape {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| rects(s, out)),
                    egui::Shape::Rect(r) => out.push((r.fill, r.rect)),
                    _ => {}
                }
            }
            let mut all = Vec::new();
            for c in &out.shapes {
                rects(&c.shape, &mut all);
            }
            all.into_iter()
                .filter(|(c, r)| {
                    *c == color
                        && r.width() <= 1.01
                        && r.height() <= 1.01
                        && (r.top() - y).abs() < 0.6
                })
                .count()
        };
        let mut app = test_app();
        assert!(crossings(&mut app) > 0, "Essentials: under the options bar");
        let mut app = test_app();
        app.workspace.to_strip_at(PanelId::Library, Edge::Top);
        assert_eq!(crossings(&mut app), 0, "over the top strip");
    }
}
