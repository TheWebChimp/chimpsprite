//! La columna de "otros paneles" entre el lienzo y el panel derecho (como el dock de Illustrator):
//! un botón por panel (Artboards, Slices) que lo abre flotando a su izquierda. Así esas secciones
//! no le quitan lugar a la paleta y al editor de color. Al final, el que prende y apaga el
//! timeline.

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
/// Entre el botón y su panel flotante.
const FLYOUT_GAP: f32 = 3.0;

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
}

impl DockPanel {
    #[cfg(not(target_arch = "wasm32"))]
    pub const ALL: [DockPanel; 4] = [
        DockPanel::Artboards,
        DockPanel::Slices,
        DockPanel::Lospec,
        DockPanel::Dafont,
    ];
    /// En la web no hay dafont (el sitio no acepta peticiones desde otras páginas).
    #[cfg(target_arch = "wasm32")]
    pub const ALL: [DockPanel; 3] = [DockPanel::Artboards, DockPanel::Slices, DockPanel::Lospec];

    pub fn label(self) -> &'static str {
        match self {
            DockPanel::Artboards => "Artboards",
            DockPanel::Slices => "Slices",
            DockPanel::Lospec => "Lospec",
            DockPanel::Dafont => "Dafont",
        }
    }

    /// Su ícono: una pieza del skin.
    fn icon(self) -> &'static str {
        match self {
            DockPanel::Artboards => "icon.dock.artboards",
            DockPanel::Slices => "icon.dock.slices",
            DockPanel::Lospec => "icon.dock.lospec",
            DockPanel::Dafont => "icon.dock.dafont",
        }
    }

    /// Texto del botón sin Pixel UI.
    fn plain(self) -> &'static str {
        match self {
            DockPanel::Artboards => "🗋",
            DockPanel::Slices => "#",
            DockPanel::Lospec => "L",
            DockPanel::Dafont => "Aa",
        }
    }
}

/// Lo que pidió el dock y la app resuelve (necesita más que el editor).
#[derive(Default)]
pub struct DockOutput {
    /// Abrir el diálogo Canvas Size (desde Artboards).
    pub canvas_size: bool,
    /// El panel abierto es de la app (Lospec, Dafont): cuál y su botón, para [`flyout`].
    pub app_panel: Option<(DockPanel, Rect)>,
}

/// Un botón del dock con Pixel UI: la pieza `button` según el estado (prendido, `on`, con el
/// marco rosa de la herramienta elegida) y el ícono `icon` centrado.
fn pixel_button(ui: &mut Ui, skin: &Skin, label: &str, icon: &str, on: bool) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(BUTTON_SIZE, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), BUTTON_SIZE);
    let info = pixel_draw::ButtonInfo {
        selected: Some(on),
        framed: true,
        ..pixel_draw::ButtonInfo::new(ui.id().with(("dock", label)), label)
    };
    let ppp = ui.ctx().pixels_per_point();
    pixel_draw::skin_button(ui, skin, rect, info, |r| {
        pixel_draw::piece_centered(skin, icon, r, ppp)
    })
    .tip(label)
}

/// El botón del timeline: lo muestra o lo esconde.
const TIMELINE: &str = "Timeline";
const TIMELINE_ICON: &str = "icon.timeline";

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
) -> DockOutput {
    let mut out = DockOutput::default();
    let open = open_panel(ui.ctx());
    let mut anchors = Vec::new();
    ui.spacing_mut().item_spacing.y = BUTTON_GAP;
    if skin.is_some() {
        ui.add_space(TOP_AIR);
    }
    for panel in DockPanel::ALL {
        let is_open = open == Some(panel);
        let response = match skin {
            Some(skin) => pixel_button(ui, skin, panel.label(), panel.icon(), is_open),
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
    let response = match skin {
        Some(skin) => pixel_button(ui, skin, TIMELINE, TIMELINE_ICON, *timeline),
        None => ui
            .add(
                egui::Button::new("🎞")
                    .selected(*timeline)
                    .min_size(BUTTON_SIZE),
            )
            .tip(TIMELINE),
    };
    if response.clicked() {
        *timeline = !*timeline;
    }
    let Some(panel) = open_panel(ui.ctx()) else {
        return out;
    };
    let anchor = anchors
        .iter()
        .find(|(p, _)| *p == panel)
        .map(|(_, r)| *r)
        .unwrap_or(Rect::NOTHING);
    let enabled = ui.is_enabled();
    match panel {
        DockPanel::Artboards | DockPanel::Slices => {
            flyout(ui.ctx(), panel, anchor, skin, FLYOUT_WIDTH, |ui| {
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
        DockPanel::Lospec | DockPanel::Dafont => out.app_panel = Some((panel, anchor)),
    }
    out
}

/// El panel flotante de `panel` a la izquierda de `anchor` (su botón), arriba alineado con él, de
/// `width` de contenido; con Pixel UI, en la caja del skin.
pub fn flyout(
    ctx: &egui::Context,
    panel: DockPanel,
    anchor: Rect,
    skin: Option<&Skin>,
    width: f32,
    add_contents: impl FnOnce(&mut Ui),
) {
    let pos = anchor.left_top() - vec2(FLYOUT_GAP, 0.0);
    egui::Area::new(egui::Id::new(("dock_flyout", panel.label())))
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
                ui.set_width(width);
                add_contents(ui);
            });
            // Con Pixel UI, la caja del panel detrás del contenido.
            if let Some(piece) = skin.and_then(|s| s.piece(FLYOUT_PIECE, WidgetState::Normal)) {
                let ppp = ui.ctx().pixels_per_point();
                let rect = Rect::from_min_size(
                    response.response.rect.min.round(),
                    response.response.rect.size().round(),
                );
                let shapes = pixel_ui::piece_shapes(piece, rect, ppp);
                ui.painter().set(background, Shape::Vec(shapes));
            }
        });
}
