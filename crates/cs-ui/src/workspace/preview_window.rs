//! La ventana del Preview solo en su ventana: la suya de antes ([`PreviewWindow`]), no la de las
//! ventanas del workspace. [`super::show::show`] la arma antes del `DockArea`, el `Viewer` pone lo
//! de adentro y, con Pixel UI, su marco y su barra van encima después ([`PreviewWindow::chrome`]).

use eframe::egui;

use super::{SurfaceIndex, Workspace, skin};
use crate::ChimpApp;
use crate::panel_registry::PanelId;
use crate::pixel_ui;
use crate::skin::Skin;

/// El Preview solo en su ventana (`Workspace::bare_preview`): su ventana es la suya de antes, no
/// la de las ventanas del workspace. La ventana de egui_dock queda como contenedor (lugar, capa,
/// acoplar): del tamaño de la de antes (`Workspace::place_preview`), sin barra de pestañas y sin
/// el fondo de su grupo; lo de adentro va donde iba en la de antes.
/// - Con Pixel UI, la del diseño (el marco `window` y su barra de título con play, centrar y la
///   ○): el marco de egui_dock (`menu.side`) se borra de su capa (`skin::hide_window_frame`), se
///   arrastra para acoplarlo de su franja de arriba (como un grupo acoplado solo) y el marco y la
///   barra del diseño se pintan aparte (`Preview::window_chrome`).
/// - Sin él, la `egui::Window` de antes: su marco es el de la ventana de egui_dock (el mismo
///   `Frame::window`) y adentro, su barra de título, "Play" y la vista (`Preview::plain_window`).
///   Se mueve de la barra (como toda ventana de egui_dock sin barra de pestañas); sin Pixel UI los
///   grupos sin barra no tienen franja de agarre, así que no se acopla arrastrándola.
pub(super) struct PreviewWindow {
    /// Lo que mide, con su marco.
    pub(super) size: egui::Vec2,
    /// Del borde de la ventana a lo de adentro de su grupo, y el margen de lo de adentro en su
    /// grupo (lo que hay de más del marco de egui_dock hasta donde va en la de antes).
    pub(super) inset: egui::Vec2,
    pub(super) margin: egui::Margin,
    /// Con Pixel UI: lo que hay arriba en el marco de egui_dock (su franja de agarre va ahí).
    frame_top: f32,
    /// Su ventana es nueva: este frame egui la mide sin mostrarla (ver [`PreviewWindow::new`]).
    pub(super) measuring: bool,
    /// Con Pixel UI, lo que dejó al dibujarse: su rect y el pintor de su capa con el lugar de su
    /// marco.
    pub(super) drawn: Option<(egui::Rect, egui::Painter, egui::layers::ShapeIdx)>,
}

impl PreviewWindow {
    /// La de antes del Preview, solo en su ventana: con Pixel UI, la del diseño (su diálogo, con la
    /// diferencia hasta el marco de las ventanas del workspace de margen); sin él, la de egui (el
    /// mismo marco que el de egui_dock, sin margen de más). `surface`: su ventana; si su área de
    /// egui todavía no existe, este frame es su pasada de medir (en el lugar de fábrica de egui,
    /// con el tamaño de fábrica de un área: la esquina se aplica desde el frame siguiente).
    pub(super) fn new(
        ui: &egui::Ui,
        skin: Option<&Skin>,
        preview: &crate::preview::Preview,
        surface: SurfaceIndex,
    ) -> Self {
        let measuring = egui::AreaState::load(ui.ctx(), skin::window_area_id(surface)).is_none();
        match skin {
            Some(skin) => {
                let dialog = pixel_ui::dialog_margin(skin);
                let frame = skin::flyout_margin(skin);
                PreviewWindow {
                    size: preview.window_size(ui, skin),
                    inset: egui::vec2(dialog.leftf(), dialog.topf()),
                    margin: egui::Margin {
                        left: dialog.left - frame.left,
                        right: dialog.right - frame.right,
                        top: dialog.top - frame.top,
                        bottom: dialog.bottom - frame.bottom,
                    },
                    frame_top: frame.topf(),
                    measuring,
                    drawn: None,
                }
            }
            None => {
                let frame = egui::Frame::window(ui.style());
                let edge = frame.stroke.width;
                let margin = frame.inner_margin;
                PreviewWindow {
                    size: preview.plain_window_size(ui),
                    inset: egui::vec2(edge + margin.leftf(), edge + margin.topf()),
                    margin: egui::Margin::ZERO,
                    frame_top: 0.0,
                    measuring,
                    drawn: None,
                }
            }
        }
    }

    /// Con Pixel UI, el marco y la barra de su ventana (`skin`), si se dibujó (`drawn`): en su
    /// capa y encima de lo de ella, así sus controles le ganan a la franja de agarre de su grupo
    /// (la de arriba, que egui_dock registra después de lo de adentro). De su barra de título se
    /// mueve su ventana de `workspace` (y deja su esquina). Devuelve si se apretó su ○.
    pub(super) fn chrome(
        self,
        ui: &mut egui::Ui,
        skin: &Skin,
        workspace: &mut Workspace,
        app: &mut ChimpApp,
    ) -> bool {
        let Some((rect, painter, background)) = self.drawn else {
            return false;
        };
        let ctx = ui.ctx().clone();
        let mut bar = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("workspace_preview")
                .layer_id(painter.layer_id())
                .max_rect(rect),
        );
        bar.set_clip_rect(rect);
        // Como lo de adentro: invisible en la pasada de medir de su ventana, y con su opacidad.
        if !painter.is_visible() {
            bar.set_invisible();
        }
        bar.multiply_opacity(painter.opacity());
        // De su barra de título se mueve (como su ventana de antes): un widget encima de la ventana
        // de egui_dock, debajo de la franja de agarre de su grupo (la de arriba, de la que se lleva
        // a acoplar), que con el radio de egui se quedaría también con lo de abajo de ella. Los
        // controles de la barra, registrados después, le ganan.
        let top = rect.top() + self.frame_top + skin::GRAB_STRIP;
        let title = egui::Rect::from_x_y_ranges(
            rect.x_range(),
            top..=rect.top() + crate::kit::title_bar::HEIGHT,
        );
        let drag = bar.interact(
            title,
            egui::Id::new("workspace_preview_title"),
            egui::Sense::drag(),
        );
        if drag.dragged()
            && let Some((s, _)) = workspace.leaf_of(PanelId::Preview)
            && let Some(window) = workspace.state.get_window_state_mut(s)
        {
            window.set_position(rect.min + drag.drag_delta());
            workspace.preview_anchored = false;
            ctx.request_repaint();
        }
        let editor = &app.doc.editor;
        app.preview
            .window_chrome(&mut bar, editor, skin, rect, (&painter, background))
    }
}
