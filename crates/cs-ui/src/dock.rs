//! La columna de "otros paneles" entre el lienzo y el panel derecho (como el dock de Illustrator):
//! un botón por panel (Artboards, Slices) que lo abre flotando a su izquierda. Así esas secciones
//! no le quitan lugar a la paleta y al editor de color.

use eframe::egui::{self, Color32, Rect, Sense, Shape, Ui, Vec2, WidgetInfo, WidgetType, vec2};

use crate::editor::Editor;
use crate::panels;
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use cs_core::ArtboardId;

/// Los botones del dock (frame 4 del diseño): `button` del skin en 9-slice, de 23×24, con 3 de
/// aire entre uno y otro.
pub const BUTTON_SIZE: Vec2 = vec2(23.0, 24.0);
const BUTTON_GAP: f32 = 3.0;
const BUTTON_PIECE: &str = "button";
/// Ancho del dock: los botones y 3 de aire hasta el panel derecho.
pub const WIDTH: f32 = 26.0;
pub const RIGHT_MARGIN: i8 = 3;
/// Del borde de la caja del lienzo a los botones (x 754 → 758 en el diseño).
pub const LEFT_AIR: i8 = 3;
/// Ancho del contenido de los paneles flotantes.
const FLYOUT_WIDTH: f32 = 210.0;
/// Marco de los paneles flotantes: la pieza `panel` del skin y su aire adentro.
const FLYOUT_PIECE: &str = "panel";
const FLYOUT_MARGIN: i8 = 8;
/// Entre el botón y su panel flotante.
const FLYOUT_GAP: f32 = 3.0;

/// Ícono de Artboards: la hoja con la esquina doblada. `a` contorno, `j` y `k` el doblez, `.` el
/// fondo de la app; ` ` transparente.
const ICON_ARTBOARDS: [&str; 13] = [
    " aaaaaaaaa   ",
    "a......jjka  ",
    "a.......jkka ",
    "a.......jkkka",
    "a.......jjjja",
    "a..........ja",
    "a...........a",
    "a...........a",
    "a...........a",
    "aj..........a",
    "a.j.........a",
    "aj.j........a",
    " aaaaaaaaaaa ",
];
/// Ícono de Slices: la grilla azul del 9-slice. `F` el borde, `G` las líneas, `.` el fondo.
const ICON_SLICES: [&str; 17] = [
    "      .     .      ",
    "      .     .      ",
    "   .FFFFFFFFFFF.   ",
    "   F  G     G  F   ",
    "   F  G     G  F   ",
    " ..FGGFGGGGGFGGF.. ",
    "   F  G     G  F   ",
    "   F  G     G  F   ",
    "   F  G     G  F   ",
    "   F  G     G  F   ",
    "   F  G     G  F   ",
    " ..FGGFGGGGGFGGF.. ",
    "   F  G     G  F   ",
    "   F  G     G  F   ",
    "   .FFFFFFFFFFF.   ",
    "      .     .      ",
    "      .     .      ",
];

/// Ícono de Lospec: la "L" blanca con borde. `j` el borde, `b` el blanco, `i` la sombra.
const ICON_LOSPEC: [&str; 13] = [
    "jjjjj    ",
    "jbbbj    ",
    "jbbbj    ",
    "jbbbj    ",
    "jbbbj    ",
    "jbbbj    ",
    "jbbbjjjjj",
    "jbbbjbbbj",
    "jbbbjbbbj",
    "jbbbbbbbj",
    "jbbbbbbbj",
    "ijjjjjjji",
    "iiiiiiiii",
];

fn icon_color(c: u8) -> Option<Color32> {
    Some(match c {
        b'b' => Color32::WHITE,
        b'i' => Color32::from_rgb(0x88, 0x6e, 0x6a),
        b'a' => Color32::from_rgb(0x59, 0x4d, 0x4d),
        b'j' => Color32::from_rgb(0xbd, 0xa4, 0x99),
        b'k' => Color32::from_rgb(0xf1, 0xeb, 0xdb),
        b'.' => Color32::from_rgb(0xdd, 0xce, 0xbf),
        b'F' => Color32::from_rgb(0x2b, 0x4e, 0x95),
        b'G' => Color32::from_rgb(0x27, 0x89, 0xcd),
        _ => return None,
    })
}

/// Los paneles del dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockPanel {
    Artboards,
    Slices,
    /// No es un panel flotante: abre la ventana de presets en la pestaña Lospec (no cabe acá).
    Lospec,
}

impl DockPanel {
    pub const ALL: [DockPanel; 3] = [DockPanel::Artboards, DockPanel::Slices, DockPanel::Lospec];

    pub fn label(self) -> &'static str {
        match self {
            DockPanel::Artboards => "Artboards",
            DockPanel::Slices => "Slices",
            DockPanel::Lospec => "Lospec",
        }
    }

    fn icon(self) -> &'static [&'static str] {
        match self {
            DockPanel::Artboards => &ICON_ARTBOARDS,
            DockPanel::Slices => &ICON_SLICES,
            DockPanel::Lospec => &ICON_LOSPEC,
        }
    }

    /// Texto del botón sin Pixel UI.
    fn plain(self) -> &'static str {
        match self {
            DockPanel::Artboards => "🗋",
            DockPanel::Slices => "#",
            DockPanel::Lospec => "L",
        }
    }
}

/// Lo que pidió el dock y la app resuelve (necesita más que el editor).
#[derive(Default)]
pub struct DockOutput {
    /// Abrir el diálogo Canvas Size (desde Artboards).
    pub canvas_size: bool,
    /// Abrir o cerrar la ventana de Lospec.
    pub toggle_lospec: bool,
}

/// Un botón del dock con Pixel UI: la pieza `button` según el estado (`Active` si su panel está
/// abierto) y el ícono centrado.
fn pixel_button(ui: &mut Ui, skin: &Skin, panel: DockPanel, open: bool) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(BUTTON_SIZE, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), BUTTON_SIZE);
    let response = ui.interact(rect, ui.id().with(("dock", panel.label())), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, open, panel.label()));
    let state = if response.is_pointer_button_down_on() {
        WidgetState::Down
    } else if open {
        WidgetState::Active
    } else if response.hovered() {
        WidgetState::Hover
    } else {
        WidgetState::Normal
    };
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = skin
        .piece(BUTTON_PIECE, state)
        .map(|p| pixel_ui::piece_shapes(p, rect, ppp))
        .unwrap_or_default();
    let rows = panel.icon();
    let size = vec2(rows[0].len() as f32, rows.len() as f32);
    let down = if state == WidgetState::Down { 1.0 } else { 0.0 };
    let at = (rect.min + (rect.size() - size) / 2.0).floor() + vec2(0.0, down);
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            if let Some(color) = icon_color(c) {
                let px = Rect::from_min_size(at + vec2(x as f32, y as f32), Vec2::splat(1.0));
                shapes.push(Shape::rect_filled(px, 0.0, color));
            }
        }
    }
    ui.painter().extend(shapes);
    response.on_hover_text(panel.label())
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

/// El dock: sus botones y el panel abierto, flotando a la izquierda de su botón. `lospec_open`:
/// la ventana de Lospec está abierta (su botón se ve prendido).
pub fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    name_buffer: &mut (ArtboardId, String),
    skin: Option<&Skin>,
    lospec_open: bool,
) -> DockOutput {
    let mut out = DockOutput::default();
    let open = open_panel(ui.ctx());
    let mut anchors = Vec::new();
    ui.spacing_mut().item_spacing.y = BUTTON_GAP;
    for panel in DockPanel::ALL {
        let is_open = match panel {
            DockPanel::Lospec => lospec_open,
            _ => open == Some(panel),
        };
        let response = match skin {
            Some(skin) => pixel_button(ui, skin, panel, is_open),
            None => ui
                .add(
                    egui::Button::new(panel.plain())
                        .selected(is_open)
                        .min_size(BUTTON_SIZE),
                )
                .on_hover_text(panel.label()),
        };
        if response.clicked() {
            if panel == DockPanel::Lospec {
                out.toggle_lospec = true;
            } else {
                set_open(ui.ctx(), (!is_open).then_some(panel));
            }
        }
        anchors.push((panel, response.rect));
    }
    let Some(panel) = open_panel(ui.ctx()) else {
        return out;
    };
    let anchor = anchors
        .iter()
        .find(|(p, _)| *p == panel)
        .map(|(_, r)| *r)
        .unwrap_or(Rect::NOTHING);
    out.canvas_size = flyout(ui, panel, anchor, editor, name_buffer, skin);
    out
}

/// El panel flotante a la izquierda de `anchor` (su botón), arriba alineado con él.
fn flyout(
    ui: &mut Ui,
    panel: DockPanel,
    anchor: Rect,
    editor: &mut Editor,
    name_buffer: &mut (ArtboardId, String),
    skin: Option<&Skin>,
) -> bool {
    let enabled = ui.is_enabled();
    let pos = anchor.left_top() - vec2(FLYOUT_GAP, 0.0);
    let mut canvas_size = false;
    egui::Area::new(egui::Id::new(("dock_flyout", panel.label())))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .pivot(egui::Align2::RIGHT_TOP)
        .show(ui.ctx(), |ui| {
            let frame = match skin {
                Some(_) => egui::Frame::NONE.inner_margin(FLYOUT_MARGIN),
                None => egui::Frame::popup(ui.style()),
            };
            // Lugar para la caja, detrás del contenido (se llena al final, cuando se sabe su tamaño).
            let background = ui.painter().add(Shape::Noop);
            let response = frame.show(ui, |ui| {
                ui.set_width(FLYOUT_WIDTH);
                ui.add_enabled_ui(enabled, |ui| match (panel, skin) {
                    (DockPanel::Artboards, Some(skin)) => {
                        canvas_size = panels::pixel_artboards(ui, editor, name_buffer, skin);
                    }
                    (DockPanel::Artboards, None) => {
                        canvas_size = panels::artboards(ui, editor, name_buffer);
                    }
                    (DockPanel::Slices, Some(skin)) => panels::pixel_slices(ui, editor, skin),
                    (DockPanel::Slices, None) => panels::slices(ui, editor),
                    (DockPanel::Lospec, _) => {}
                });
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
    canvas_size
}
