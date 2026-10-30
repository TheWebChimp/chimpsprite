//! La columna de "otros paneles" entre el lienzo y el panel derecho (como el dock de Illustrator):
//! un botón por panel (Artboards, Slices) que lo abre flotando a su izquierda. Así esas secciones
//! no le quitan lugar a la paleta y al editor de color. Hasta abajo, el que prende y apaga el
//! timeline y arriba de él, el de la ventanita de preview.

use eframe::egui::{self, Rect, Sense, Shape, Ui, Vec2, vec2};

use crate::editor::Editor;
use crate::panels;
use crate::pixel_draw;
use crate::pixel_ui;
use crate::pixel_ui::Tip as _;
use crate::skin::{Skin, WidgetState};
use cs_core::ArtboardId;

/// Los botones del dock (frame 4 del diseño): `button` del skin en 9-slice, de 23×24, con 3 de
/// aire entre uno y otro.
pub const BUTTON_SIZE: Vec2 = vec2(23.0, 24.0);
const BUTTON_GAP: f32 = 3.0;
/// Arriba de los botones: el primero empieza a la altura de la caja del lienzo (sus reglas).
const TOP_AIR: f32 = 5.0;
/// Debajo del botón del timeline, hasta el borde de abajo del dock.
const BOTTOM_AIR: f32 = 5.0;
/// Ancho del dock: los botones y 3 de aire hasta el panel derecho.
pub const WIDTH: f32 = 26.0;
pub const RIGHT_MARGIN: i8 = 3;
/// Del borde de la caja del lienzo a los botones (x 754 → 758 en el diseño).
pub const LEFT_AIR: i8 = 3;
/// Ancho del contenido de los paneles flotantes.
const FLYOUT_WIDTH: f32 = 210.0;
/// Marco de los paneles flotantes: la pieza `panel` del skin y su aire adentro.
const FLYOUT_PIECE: &str = "menu.panel";
pub const FLYOUT_MARGIN: i8 = 8;
/// Entre el botón y su panel flotante (la pestaña los une).
const FLYOUT_GAP: f32 = 4.0;

/// Dónde va un panel flotante: junto a su botón y dentro de la columna del dock (de la altura de
/// las reglas del lienzo al botón del timeline).
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub button: Rect,
    pub column: Rect,
}

/// Los paneles del dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockPanel {
    Artboards,
    Slices,
    /// Las paletas de Lospec y las fuentes de dafont (solo escritorio): su contenido lo pone la
    /// app (ver [`DockOutput::app_panel`]).
    Lospec,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Dafont,
    /// Groom: de pixel art falso a pixel art de verdad (su contenido lo pone la app).
    Groom,
    /// Lumen: normales y luz para el sprite (su contenido lo pone la app).
    Lumen,
    /// Los símbolos de la biblioteca (su contenido lo pone la app).
    Library,
}

impl DockPanel {
    #[cfg(not(target_arch = "wasm32"))]
    pub const ALL: [DockPanel; 7] = [
        DockPanel::Artboards,
        DockPanel::Library,
        DockPanel::Slices,
        DockPanel::Lospec,
        DockPanel::Dafont,
        DockPanel::Groom,
        DockPanel::Lumen,
    ];
    /// En la web no hay dafont (el sitio no acepta peticiones desde otras páginas).
    #[cfg(target_arch = "wasm32")]
    pub const ALL: [DockPanel; 6] = [
        DockPanel::Artboards,
        DockPanel::Library,
        DockPanel::Slices,
        DockPanel::Lospec,
        DockPanel::Groom,
        DockPanel::Lumen,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DockPanel::Artboards => "Artboards",
            DockPanel::Slices => "Slices",
            DockPanel::Lospec => "Lospec",
            DockPanel::Dafont => "Dafont",
            DockPanel::Groom => "Groom",
            DockPanel::Lumen => "Lumen",
            DockPanel::Library => "Library",
        }
    }

    /// Su ícono: una pieza del skin.
    fn icon(self) -> &'static str {
        match self {
            DockPanel::Artboards => "icon.dock.artboards",
            DockPanel::Slices => "icon.dock.slices",
            DockPanel::Lospec => "icon.dock.lospec",
            DockPanel::Dafont => "icon.dock.dafont",
            DockPanel::Groom => "icon.dock.groom",
            DockPanel::Lumen => "icon.dock.lumen",
            DockPanel::Library => "icon.dock.library",
        }
    }

    /// Texto del botón sin Pixel UI.
    fn plain(self) -> &'static str {
        match self {
            DockPanel::Artboards => "🗋",
            DockPanel::Slices => "#",
            DockPanel::Lospec => "L",
            DockPanel::Dafont => "Aa",
            DockPanel::Groom => "▦",
            DockPanel::Lumen => "☀",
            DockPanel::Library => "▣",
        }
    }
}

/// Lo que pidió el dock y la app resuelve (necesita más que el editor).
#[derive(Default)]
pub struct DockOutput {
    /// Abrir el diálogo Canvas Size (desde Artboards).
    pub canvas_size: bool,
    /// El panel abierto es de la app (Lospec, Dafont): cuál y dónde, para [`flyout`].
    pub app_panel: Option<(DockPanel, Anchor)>,
}

/// Un botón del dock con Pixel UI: la pieza `button` según el estado (prendido, `on`, con el
/// marco rosa de la herramienta elegida) y el ícono `icon` centrado (o `plain`, si el skin no lo
/// trae).
fn pixel_button(
    ui: &mut Ui,
    skin: &Skin,
    label: &str,
    (icon, plain): (&str, &str),
    on: bool,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(BUTTON_SIZE, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), BUTTON_SIZE);
    let info = pixel_draw::ButtonInfo {
        selected: Some(on),
        ..pixel_draw::ButtonInfo::new(ui.id().with(("dock", label)), label)
    };
    let ppp = ui.ctx().pixels_per_point();
    let response = pixel_draw::skin_button(ui, skin, rect, info, |r| {
        pixel_draw::piece_centered(skin, icon, r, ppp)
    });
    plain_icon(ui, skin, icon, plain, rect);
    response.tip(label)
}

/// Si el skin no trae la pieza `icon`, el texto `plain` centrado en `rect`: así el botón no queda
/// vacío mientras falta el ícono.
fn plain_icon(ui: &Ui, skin: &Skin, icon: &str, plain: &str, rect: Rect) {
    if skin.piece(icon, WidgetState::Normal).is_none() {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let color = skin.colors().text;
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            plain,
            font,
            color,
        );
    }
}

/// El botón del timeline: lo muestra o lo esconde.
const TIMELINE: &str = "Timeline";
const TIMELINE_ICON: &str = "icon.timeline";
/// El botón de la ventanita de preview: la abre o la cierra.
const PREVIEW: &str = "Preview";
const PREVIEW_ICON: &str = "icon.preview";

/// Un botón del dock que prende o apaga `on`: con Pixel UI, el del skin; sin él, el de egui con
/// `plain`.
fn toggle(ui: &mut Ui, skin: Option<&Skin>, label: &str, icon: &str, plain: &str, on: &mut bool) {
    let response = match skin {
        Some(skin) => pixel_button(ui, skin, label, (icon, plain), *on),
        None => ui
            .add(egui::Button::new(plain).selected(*on).min_size(BUTTON_SIZE))
            .tip(label),
    };
    if response.clicked() {
        *on = !*on;
    }
}

/// El panel abierto (en la memoria de egui, por pestaña de la app: no se guarda).
fn open_id() -> egui::Id {
    egui::Id::new("dock_open")
}

pub fn open_panel(ctx: &egui::Context) -> Option<DockPanel> {
    ctx.data(|d| d.get_temp::<Option<DockPanel>>(open_id()))
        .flatten()
}

pub fn set_open(ctx: &egui::Context, panel: Option<DockPanel>) {
    ctx.data_mut(|d| d.insert_temp(open_id(), panel));
}

/// El dock: sus botones y el panel abierto, flotando a la izquierda de su botón.
pub fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    name_buffer: &mut (ArtboardId, String),
    skin: Option<&Skin>,
    timeline: &mut bool,
    preview: &mut bool,
) -> DockOutput {
    let mut out = DockOutput::default();
    let open = open_panel(ui.ctx());
    let mut anchors = Vec::new();
    ui.spacing_mut().item_spacing.y = BUTTON_GAP;
    let mut column = ui.max_rect();
    if skin.is_some() {
        ui.add_space(TOP_AIR);
        column.min.y += TOP_AIR;
        column.max.y -= BOTTOM_AIR;
    }
    for panel in DockPanel::ALL {
        let is_open = open == Some(panel);
        let response = match skin {
            Some(skin) => pixel_button(
                ui,
                skin,
                panel.label(),
                (panel.icon(), panel.plain()),
                is_open,
            ),
            None => ui
                .add(
                    egui::Button::new(panel.plain())
                        .selected(is_open)
                        .min_size(BUTTON_SIZE),
                )
                .tip(panel.label()),
        };
        if response.clicked() {
            set_open(ui.ctx(), (!is_open).then_some(panel));
        }
        anchors.push((panel, response.rect));
    }
    // Hasta abajo del dock, el del timeline y arriba de él, el del preview.
    let bottom = egui::Layout::bottom_up(egui::Align::Min);
    ui.with_layout(bottom, |ui| {
        if skin.is_some() {
            ui.add_space(BOTTOM_AIR);
        }
        toggle(ui, skin, TIMELINE, TIMELINE_ICON, "🎞", timeline);
        toggle(ui, skin, PREVIEW, PREVIEW_ICON, "▣", preview);
    });
    let Some(panel) = open_panel(ui.ctx()) else {
        return out;
    };
    let button = anchors
        .iter()
        .find(|(p, _)| *p == panel)
        .map(|(_, r)| *r)
        .unwrap_or(Rect::NOTHING);
    let anchor = Anchor { button, column };
    let enabled = ui.is_enabled();
    match panel {
        DockPanel::Artboards | DockPanel::Slices => {
            flyout(ui.ctx(), panel, anchor, skin, FLYOUT_WIDTH, false, |ui| {
                ui.add_enabled_ui(enabled, |ui| match (panel, skin) {
                    (DockPanel::Artboards, Some(skin)) => {
                        out.canvas_size = panels::pixel_artboards(ui, editor, name_buffer, skin);
                    }
                    (DockPanel::Artboards, None) => {
                        out.canvas_size = panels::artboards(ui, editor, name_buffer);
                    }
                    (DockPanel::Slices, Some(skin)) => panels::pixel_slices(ui, editor, skin),
                    (_, _) => panels::slices(ui, editor),
                });
            });
        }
        DockPanel::Lospec
        | DockPanel::Dafont
        | DockPanel::Groom
        | DockPanel::Lumen
        | DockPanel::Library => out.app_panel = Some((panel, anchor)),
    }
    out
}

/// El panel flotante de `panel` a la izquierda de su botón, de `width` de contenido. Arriba va
/// alineado con el botón; si no cabe hacia abajo, sube hasta donde haga falta (no más arriba de
/// la columna del dock) y lo que no quepa en la columna se ve con scroll. Los altos (`tall`:
/// Lospec, Dafont, Groom, Lumen, Library) ocupan la columna entera, de arriba a abajo. Con Pixel UI, en la caja
/// del skin, con el botón hecho pestaña de la caja.
pub fn flyout(
    ctx: &egui::Context,
    panel: DockPanel,
    anchor: Anchor,
    skin: Option<&Skin>,
    width: f32,
    tall: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    let id = egui::Id::new(("dock_flyout", panel.label()));
    let Anchor { button, column } = anchor;
    // El alto de la vez pasada, para saber si cabe.
    let height = ctx.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let top = if tall {
        column.top().round()
    } else {
        flyout_top(button.top(), height, column)
    };
    let pos = egui::pos2(button.left() - FLYOUT_GAP, top);
    // Lo que cabe en la columna, sin el marco (el de egui mide más o menos lo mismo).
    let max_height = (column.height() - 2.0 * FLYOUT_MARGIN as f32).max(BUTTON_SIZE.y);
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .pivot(egui::Align2::RIGHT_TOP)
        .show(ctx, |ui| {
            let frame = match skin {
                Some(_) => egui::Frame::NONE.inner_margin(FLYOUT_MARGIN),
                None => egui::Frame::popup(ui.style()),
            };
            // Lugar para la caja, detrás del contenido (se llena al final, cuando se sabe su tamaño).
            let background = ui.painter().add(Shape::Noop);
            let response = frame.show(ui, |ui| {
                // El Area da de alto lo que midió la vez pasada: sin esto, el panel no crecería
                // cuando su contenido crece.
                ui.set_max_height(max_height);
                let mut area = egui::ScrollArea::vertical()
                    .id_salt(id.with("scroll"))
                    .max_height(max_height);
                if tall {
                    area = area
                        .min_scrolled_height(max_height)
                        .auto_shrink([true, false]);
                }
                pixel_ui::scroll(ui, area, |ui| {
                    ui.set_width(width);
                    add_contents(ui);
                });
            });
            let rect = response.response.rect;
            ui.ctx().data_mut(|d| d.insert_temp(id, rect.height()));
            // Con Pixel UI, la caja del panel detrás del contenido y el botón hecho pestaña.
            if let (Some(skin), Some(piece)) = (
                skin,
                skin.and_then(|s| s.piece(FLYOUT_PIECE, WidgetState::Normal)),
            ) {
                let ppp = ui.ctx().pixels_per_point();
                let rect = Rect::from_min_size(rect.min.round(), rect.size().round());
                let mut shapes = pixel_ui::piece_shapes(piece, rect, ppp);
                shapes.extend(tab_shapes(skin, piece, panel, rect, button, ppp));
                ui.painter().set(background, Shape::Vec(shapes));
                plain_icon(ui, skin, panel.icon(), panel.plain(), button);
            }
        });
}

/// Dónde empieza un panel de `height` cuyo botón está en `button_top`: a su altura si cabe en
/// `column`; si no, más arriba, hasta el borde de arriba de la columna.
fn flyout_top(button_top: f32, height: f32, column: Rect) -> f32 {
    let room = column.bottom() - height;
    button_top.min(room).max(column.top()).round()
}

/// El botón del panel abierto como pestaña de su caja (`panel_rect`): la caja del skin desde el
/// borde del panel hasta el del botón, sin la línea que los separa, y el ícono encima. Solo se
/// pinta: el clic sigue llegando al botón de abajo (que cierra el panel).
fn tab_shapes(
    skin: &Skin,
    piece: &crate::skin::Piece,
    panel: DockPanel,
    panel_rect: Rect,
    button: Rect,
    ppp: f32,
) -> Vec<Shape> {
    let Some(center) = piece.center else {
        return Vec::new();
    };
    let tab = Rect::from_min_max(
        egui::pos2(panel_rect.right() - 1.0, button.top()).round(),
        egui::pos2(button.right(), button.bottom()).round(),
    );
    let mut shapes = pixel_ui::piece_shapes(piece, tab, ppp);
    // Los bordes de la caja (lo que queda fuera del centro del 9-slice).
    let left = center.x as f32;
    let right = (piece.size[0] - center.x - center.w) as f32;
    let top = center.y as f32;
    let bottom = (piece.size[1] - center.y - center.h) as f32;
    let fill = piece.pixel(center.x, center.y);
    // Sin la línea entre la caja y la pestaña: el relleno de una a la otra.
    let seam = Rect::from_min_max(
        egui::pos2(panel_rect.right() - right, tab.top() + top),
        egui::pos2(tab.left() + left, tab.bottom() - bottom),
    );
    shapes.push(Shape::rect_filled(seam, 0.0, fill));
    // Los bordes de arriba y de abajo siguen derecho de la caja a la pestaña: desde la línea
    // oscura de la caja o, si los dos empiezan (o terminan) a la misma altura, tapando también
    // la esquina de la caja.
    let line = |x0: f32, y: f32, color: egui::Color32| {
        let r = Rect::from_min_max(egui::pos2(x0, y), egui::pos2(tab.left() + left, y + 1.0));
        Shape::rect_filled(r, 0.0, color)
    };
    let from = |aligned: bool| {
        if aligned {
            panel_rect.right() - right
        } else {
            tab.left()
        }
    };
    let x0 = from((tab.top() - panel_rect.top()).abs() < 0.5);
    for k in 0..center.y {
        shapes.push(line(x0, tab.top() + k as f32, piece.pixel(center.x, k)));
    }
    let x0 = from((tab.bottom() - panel_rect.bottom()).abs() < 0.5);
    let below = center.y + center.h;
    for k in below..piece.size[1] {
        let y = tab.bottom() - (piece.size[1] - k) as f32;
        shapes.push(line(x0, y, piece.pixel(center.x, k)));
    }
    shapes.extend(pixel_draw::piece_centered(skin, panel.icon(), button, ppp));
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El alto del panel después de un cuadro con contenido de `content` de alto.
    fn panel_height(ctx: &egui::Context, content: f32) -> f32 {
        let column = Rect::from_min_max(egui::pos2(400.0, 20.0), egui::pos2(430.0, 580.0));
        let anchor = Anchor {
            button: Rect::from_min_size(egui::pos2(403.0, 20.0), BUTTON_SIZE),
            column,
        };
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 600.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            flyout(
                ui.ctx(),
                DockPanel::Slices,
                anchor,
                None,
                100.0,
                false,
                |ui| {
                    ui.allocate_exact_size(vec2(100.0, content), Sense::hover());
                },
            );
        });
        output.textures_delta.clear();
        let id = egui::Id::new(("dock_flyout", DockPanel::Slices.label()));
        ctx.data(|d| d.get_temp::<f32>(id)).unwrap()
    }

    #[test]
    fn the_panel_grows_with_its_contents_up_to_the_column() {
        let ctx = egui::Context::default();
        for _ in 0..3 {
            panel_height(&ctx, 50.0);
        }
        let small = panel_height(&ctx, 50.0);
        // Más contenido (un artboard nuevo): crece, aunque el Area recuerde el alto de antes.
        panel_height(&ctx, 200.0);
        let grown = panel_height(&ctx, 200.0);
        assert!(grown >= small + 150.0, "{small} → {grown}");
        // Más de lo que cabe en la columna: llega a ella y lo demás va con scroll.
        panel_height(&ctx, 2000.0);
        let full = panel_height(&ctx, 2000.0);
        assert!(full <= 560.0 + 1.0, "{full}");
    }

    #[test]
    fn a_tall_panel_moves_up_but_not_above_the_column() {
        let column = Rect::from_min_max(egui::pos2(0.0, 100.0), egui::pos2(30.0, 700.0));
        assert_eq!(
            flyout_top(300.0, 200.0, column),
            300.0,
            "fits below its button"
        );
        assert_eq!(flyout_top(300.0, 500.0, column), 200.0, "moves up to fit");
        assert_eq!(
            flyout_top(300.0, 900.0, column),
            100.0,
            "the column's top at most"
        );
    }
}
