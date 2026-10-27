//! La ventanita de preview (como la de Aseprite): el artboard activo en el frame que se ve (sigue
//! la animación), centrado sobre el damero, en la ventana de la app. De fábrica, al zoom entero
//! más grande que entra; la rueda lo cambia de a uno.

use eframe::egui::{
    self, Align2, Color32, ColorImage, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2,
    vec2,
};
use eframe::emath::GuiRounding as _;

use crate::editor::Editor;
use crate::skin::Skin;
use crate::theme;
use cs_core::ArtboardId;

/// Lo de adentro de la ventana: el lugar del dibujo.
pub const VIEW: Vec2 = vec2(192.0, 144.0);
/// Los cuadros del damero, en puntos.
const CHECKER_CELL: f32 = 4.0;
const MAX_ZOOM: f32 = 32.0;
/// De la esquina de abajo a la derecha de la pantalla.
const OFFSET: Vec2 = vec2(-48.0, -48.0);

#[derive(Default)]
pub struct Preview {
    texture: Option<TextureHandle>,
    checker: Option<TextureHandle>,
    /// Lo que tiene la textura: el artboard, su frame y la revisión del documento.
    rendered: Option<(ArtboardId, usize, u64)>,
    /// El zoom elegido con la rueda; `None`, el que entra.
    zoom: Option<f32>,
}

impl Preview {
    /// La ventana. Devuelve si se cerró con la ×.
    pub fn show(&mut self, ctx: &egui::Context, editor: &Editor, skin: Option<&Skin>) -> bool {
        let anchor = (Align2::RIGHT_BOTTOM, OFFSET);
        let (_, closed) =
            crate::pixel_ui::app_window(ctx, skin, "preview", "Preview", true, anchor, |ui| {
                self.contents(ui, editor, skin)
            });
        closed
    }

    /// Recompone la textura si cambió el artboard, el frame o el documento (o hay un trazo).
    fn update(&mut self, ctx: &egui::Context, editor: &Editor) {
        let view = editor.view();
        let document = view.document.as_ref();
        let Some(artboard) = document.artboard(editor.active) else {
            return;
        };
        let frame = editor.frame_of(artboard.id);
        let key = (artboard.id, frame, editor.revision);
        if self.texture.is_some() && self.rendered == Some(key) && !editor.is_painting() {
            return;
        }
        let sprite = &*cs_render::materialize(document, &artboard.sprite, frame..frame + 1);
        let pixels = cs_render::render_frame(sprite, frame);
        let bytes: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
        let size = [sprite.width as usize, sprite.height as usize];
        let image = ColorImage::from_rgba_unmultiplied(size, &bytes);
        match &mut self.texture {
            Some(tex) => tex.set(image, TextureOptions::NEAREST),
            None => {
                self.texture = Some(ctx.load_texture("preview", image, TextureOptions::NEAREST));
            }
        }
        self.rendered = Some(key);
    }

    fn contents(&mut self, ui: &mut Ui, editor: &Editor, skin: Option<&Skin>) {
        self.update(ui.ctx(), editor);
        let (rect, response) = ui.allocate_exact_size(VIEW, Sense::hover());
        let ppp = ui.ctx().pixels_per_point();
        let painter = ui.painter_at(rect);
        let background = skin.map_or(theme::CANVAS_BG, |s| s.colors().canvas);
        painter.rect_filled(rect, 0.0, background);
        let Some((texture, size)) = self.texture.as_ref().map(|t| (t.id(), t.size_vec2())) else {
            return;
        };
        let mut zoom = self.zoom.unwrap_or_else(|| fit_zoom(size));
        // La rueda: un paso por movimiento.
        if response.hovered() {
            let steps: Vec<bool> = ui.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel { delta, .. } if delta.y != 0.0 => {
                            Some(delta.y > 0.0)
                        }
                        _ => None,
                    })
                    .collect()
            });
            for closer in steps {
                zoom = step_zoom(zoom, closer);
                self.zoom = Some(zoom);
            }
        }
        let shown = size * zoom;
        // En puntos enteros: los píxeles del dibujo caen justo en los de la pantalla.
        let min = (rect.center() - shown / 2.0).round();
        let image = Rect::from_min_size(min, shown).round_to_pixels(ppp);
        let checker = self.checker(ui.ctx());
        let uv = Rect::from_min_max(Pos2::ZERO, (shown / (CHECKER_CELL * 2.0)).to_pos2());
        painter.image(checker, image, uv, Color32::WHITE);
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(texture, image, full, Color32::WHITE);
    }

    fn checker(&mut self, ctx: &egui::Context) -> egui::TextureId {
        self.checker
            .get_or_insert_with(|| {
                let (a, b) = (theme::CHECKER_LIGHT, theme::CHECKER_DARK);
                let image = ColorImage::new([2, 2], vec![a, b, b, a]);
                ctx.load_texture("preview_checker", image, TextureOptions::NEAREST_REPEAT)
            })
            .id()
    }
}

/// El zoom entero más grande con el que el dibujo entra; si no entra ni a 1, el que lo achica
/// justo.
fn fit_zoom(size: Vec2) -> f32 {
    let fit = (VIEW.x / size.x).min(VIEW.y / size.y);
    if fit >= 1.0 {
        fit.floor().min(MAX_ZOOM)
    } else {
        fit
    }
}

/// Un paso de zoom: de a uno arriba de 1 y a la mitad o al doble abajo.
fn step_zoom(zoom: f32, closer: bool) -> f32 {
    match (closer, zoom >= 1.0) {
        (true, true) => (zoom.floor() + 1.0).min(MAX_ZOOM),
        (true, false) => (zoom * 2.0).min(1.0),
        (false, true) if zoom > 1.0 => zoom.ceil() - 1.0,
        (false, _) => (zoom / 2.0).max(1.0 / 16.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_zoom_fits_the_drawing_in_whole_steps() {
        assert_eq!(fit_zoom(vec2(16.0, 16.0)), 9.0);
        assert_eq!(fit_zoom(vec2(64.0, 32.0)), 3.0);
        assert_eq!(fit_zoom(vec2(2.0, 2.0)), MAX_ZOOM);
        assert_eq!(fit_zoom(vec2(384.0, 144.0)), 0.5);
    }

    #[test]
    fn the_wheel_steps_the_zoom() {
        assert_eq!(step_zoom(3.0, true), 4.0);
        assert_eq!(step_zoom(3.0, false), 2.0);
        assert_eq!(step_zoom(1.0, false), 0.5);
        assert_eq!(step_zoom(0.5, true), 1.0);
        assert_eq!(step_zoom(MAX_ZOOM, true), MAX_ZOOM);
    }
}
