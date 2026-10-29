//! Imágenes de ejemplo de las paletas de Lospec (solo escritorio: el CDN no manda CORS).
//!
//! Se piden solo para las filas visibles, se decodifican en el hilo de `ehttp` y se guardan como
//! texturas en una caché por URL que vive mientras la ventana de presets está abierta.

use std::collections::HashMap;

use cs_io::lospec::LospecPalette;
use eframe::egui;

use crate::lospec_client::{self, FetchError, Pending};
use crate::palette_ui::REQUEST_TIMEOUT_SECS;

/// Tope del cuerpo de una imagen de ejemplo.
pub const MAX_EXAMPLE_BYTES: usize = 4 * 1024 * 1024;
/// Tope de ancho y alto: más grande no hace falta y podría pasar el máximo de la GPU.
const MAX_EXAMPLE_SIDE: u32 = 4096;
/// Aumento máximo de un ejemplo respecto de su tamaño original.
const MAX_SCALE: f32 = 4.0;
/// Alto del hueco mientras carga, para que las filas no salten.
const PLACEHOLDER_HEIGHT: f32 = 64.0;
/// Imágenes pidiéndose a la vez; las demás esperan a un cuadro siguiente.
const MAX_IN_FLIGHT: usize = 4;

/// Ancho y alto según el encabezado IHDR, sin decodificar. `None` si no es un PNG.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(SIGNATURE) || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    let read = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    Some((read(16)?, read(20)?))
}

/// Bytes de la respuesta → imagen para egui. Se corre en el hilo de `ehttp`.
pub fn decode_example(bytes: &[u8]) -> Result<egui::ColorImage, FetchError> {
    if bytes.len() > MAX_EXAMPLE_BYTES {
        return Err(FetchError::BadImage);
    }
    // Se mira el tamaño antes de decodificar: un PNG chico puede descomprimirse en cientos de MB.
    match png_size(bytes) {
        Some((w, h)) if w <= MAX_EXAMPLE_SIDE && h <= MAX_EXAMPLE_SIDE => {}
        _ => return Err(FetchError::BadImage),
    }
    let bitmap = cs_io::import::decode_png(bytes).map_err(|_| FetchError::BadImage)?;
    let rgba: Vec<u8> = bitmap
        .pixels
        .iter()
        .flat_map(|c| [c.r, c.g, c.b, c.a])
        .collect();
    let size = [bitmap.width as usize, bitmap.height as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(size, &rgba))
}

/// Tamaño en pantalla: al ancho disponible, pero como mucho ×4, sin deformar.
fn example_size(native: [usize; 2], width: f32) -> egui::Vec2 {
    let native = egui::vec2(native[0] as f32, native[1] as f32);
    if native.x <= 0.0 || native.y <= 0.0 {
        return egui::Vec2::ZERO;
    }
    native * (width / native.x).min(MAX_SCALE)
}

/// Índice siguiente (o anterior) dando la vuelta.
fn step(index: usize, len: usize, forward: bool) -> usize {
    if forward {
        (index + 1) % len
    } else {
        (index + len - 1) % len
    }
}

enum Example {
    Loading {
        started: f64,
        pending: Pending<egui::ColorImage>,
    },
    Ready(egui::TextureHandle),
    /// No se vuelve a pedir mientras la ventana siga abierta.
    Failed,
}

/// Caché de ejemplos por URL y el ejemplo que muestra cada fila (por slug).
#[derive(Default)]
pub struct ExampleCache {
    images: HashMap<String, Example>,
    shown: HashMap<String, usize>,
}

impl ExampleCache {
    /// Pide `url` con `start` si no se pidió antes y hay lugar. `now`: `i.time` de egui.
    fn request(&mut self, url: &str, now: f64, start: impl FnOnce() -> Pending<egui::ColorImage>) {
        if self.images.contains_key(url) || self.in_flight() >= MAX_IN_FLIGHT {
            return;
        }
        let pending = start();
        self.images.insert(
            url.to_string(),
            Example::Loading {
                started: now,
                pending,
            },
        );
    }

    /// Pasa a textura lo que llegó y da por fallado lo que venció el plazo.
    pub fn poll(
        &mut self,
        now: f64,
        mut upload: impl FnMut(&str, egui::ColorImage) -> egui::TextureHandle,
    ) {
        for (url, example) in &mut self.images {
            let Example::Loading { started, pending } = example else {
                continue;
            };
            let result = pending.poll().or_else(|| {
                (now - *started > REQUEST_TIMEOUT_SECS).then_some(Err(FetchError::Network))
            });
            match result {
                Some(Ok(image)) => *example = Example::Ready(upload(url, image)),
                Some(Err(e)) => {
                    log::warn!("lospec example {url} failed: {e:?}");
                    *example = Example::Failed;
                }
                None => {}
            }
        }
    }

    pub fn is_busy(&self) -> bool {
        self.in_flight() > 0
    }

    fn in_flight(&self) -> usize {
        self.images
            .values()
            .filter(|e| matches!(e, Example::Loading { .. }))
            .count()
    }
}

/// El marco de los ejemplos: 1 px blanco alrededor y la sombra debajo (como en el diseño).
const FRAME: egui::Color32 = egui::Color32::WHITE;
const FRAME_SHADOW: egui::Color32 = egui::Color32::from_rgb(136, 110, 106);
/// Las piezas del paginador (◀ 1/5 ▶).
const PAGER_PREV: &str = "lospec.pager.prev";
const PAGER_FIELD: &str = "lospec.pager.field";
const PAGER_NEXT: &str = "lospec.pager.next";

/// El ejemplo actual a todo el ancho, en su marco (o un hueco mientras carga). Si falla, nada.
pub fn image(ui: &mut egui::Ui, p: &LospecPalette, cache: &mut ExampleCache, now: f64) {
    let len = p.examples.len();
    if len == 0 {
        return;
    }
    let index = cache.shown.get(&p.slug).copied().unwrap_or(0).min(len - 1);
    let url = &p.examples[index];
    let width = ui.available_width();
    let inner_w = (width - 2.0).max(1.0);
    let size = match cache.images.get(url) {
        Some(Example::Ready(texture)) => example_size(texture.size(), inner_w).floor(),
        Some(Example::Failed) => return,
        _ => egui::vec2(inner_w, PLACEHOLDER_HEIGHT),
    };
    let (rect, _) = ui.allocate_exact_size(size + egui::vec2(2.0, 3.0), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let ppp = ui.ctx().pixels_per_point();
    let framed = egui::Rect::from_min_size(rect.min, size + egui::vec2(2.0, 2.0));
    let painter = ui.painter();
    use eframe::emath::GuiRounding as _;
    painter.rect_filled(framed.round_to_pixels(ppp), 0.0, FRAME);
    let shadow = egui::Rect::from_min_max(
        egui::pos2(framed.left(), framed.bottom()),
        egui::pos2(framed.right(), framed.bottom() + 1.0),
    );
    painter.rect_filled(shadow.round_to_pixels(ppp), 0.0, FRAME_SHADOW);
    let at = egui::Rect::from_min_size(framed.min + egui::vec2(1.0, 1.0), size);
    match cache.images.get(url) {
        Some(Example::Ready(texture)) => {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(texture.id(), at, uv, egui::Color32::WHITE);
        }
        _ => {
            let ctx = ui.ctx().clone();
            cache.request(url, now, || lospec_client::fetch_example(&ctx, url));
            ui.put(at, egui::Spinner::new());
        }
    }
}

/// ◀ 1/5 ▶ para pasar entre los ejemplos, si hay más de uno: las piezas del diseño (sin ellas,
/// botones de texto).
pub fn pager(ui: &mut egui::Ui, p: &LospecPalette, cache: &mut ExampleCache) {
    let len = p.examples.len();
    if len < 2 {
        return;
    }
    let index = cache.shown.get(&p.slug).copied().unwrap_or(0).min(len - 1);
    let text = format!("{}/{len}", index + 1);
    let mut next = index;
    let skin = crate::pixel_ui::frame_skin(ui.ctx());
    let piece = |name: &str| {
        skin.as_ref()
            .and_then(|s| s.piece(name, crate::skin::WidgetState::Normal).cloned())
    };
    match (piece(PAGER_PREV), piece(PAGER_FIELD), piece(PAGER_NEXT)) {
        (Some(prev), Some(field), Some(next_piece)) => {
            let ppp = ui.ctx().pixels_per_point();
            let size = |p: &crate::skin::Piece| egui::vec2(p.size[0] as f32, p.size[1] as f32);
            let total = size(&prev).x + size(&field).x + size(&next_piece).x;
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(total, size(&prev).y), egui::Sense::hover());
            let prev_at = egui::Rect::from_min_size(rect.min, size(&prev));
            let field_at = egui::Rect::from_min_size(prev_at.right_top(), size(&field));
            let next_at = egui::Rect::from_min_size(field_at.right_top(), size(&next_piece));
            let mut shapes = Vec::new();
            for (piece, at) in [(&prev, prev_at), (&field, field_at), (&next_piece, next_at)] {
                shapes.extend(crate::pixel_ui::piece_shapes(piece, at, ppp));
            }
            let ink = crate::pixel_ui::control_border(ui.ctx());
            let galley = crate::menus::galley(ui, &text, ink);
            // Centrado en lo blanco del campo (sin sus filas de abajo).
            let left = (field_at.center().x - galley.size().x / 2.0).round();
            let caps = (field_at.top() + 4.0).round();
            let pos = crate::tab_bar::text_pos(&galley, left, caps, ppp);
            shapes.push(egui::Shape::galley(pos, galley, ink));
            ui.painter().extend(shapes);
            let button = |at: egui::Rect, label: &str| {
                ui.interact(
                    at,
                    ui.id().with(("lospec_pager", &p.slug, label)),
                    egui::Sense::click(),
                )
                .on_hover_text(label)
            };
            if button(prev_at, "Previous example").clicked() {
                next = step(index, len, false);
            }
            if button(next_at, "Next example").clicked() {
                next = step(index, len, true);
            }
        }
        _ => {
            ui.horizontal(|ui| {
                if crate::pixel_ui::text_button(ui, "<").clicked() {
                    next = step(index, len, false);
                }
                ui.weak(&text);
                if crate::pixel_ui::text_button(ui, ">").clicked() {
                    next = step(index, len, true);
                }
            });
        }
    }
    if next != index {
        cache.shown.insert(p.slug.clone(), next);
    }
}

#[cfg(test)]
mod tests {
    use cs_core::Rgba;
    use cs_io::export::Bitmap;

    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut b = Bitmap::new(width, height);
        b.pixels[0] = Rgba::new(10, 20, 30, 255);
        b.to_png().unwrap()
    }

    fn image() -> egui::ColorImage {
        egui::ColorImage::new([2, 1], vec![egui::Color32::RED; 2])
    }

    #[test]
    fn png_size_reads_the_header() {
        assert_eq!(png_size(&png(3, 2)), Some((3, 2)));
        assert_eq!(png_size(b"GIF89a................"), None);
        assert_eq!(png_size(&png(3, 2)[..20]), None);
    }

    #[test]
    fn decodes_small_pngs() {
        let img = decode_example(&png(3, 2)).unwrap();
        assert_eq!(img.size, [3, 2]);
        assert_eq!(img.pixels[0], egui::Color32::from_rgb(10, 20, 30));
        assert_eq!(decode_example(b"<html>"), Err(FetchError::BadImage));
    }

    #[test]
    fn rejects_big_bodies_and_huge_images() {
        let mut big = png(1, 1);
        big.resize(MAX_EXAMPLE_BYTES + 1, 0);
        assert_eq!(decode_example(&big), Err(FetchError::BadImage));
        assert_eq!(
            decode_example(&png(MAX_EXAMPLE_SIDE + 1, 1)),
            Err(FetchError::BadImage)
        );
    }

    #[test]
    fn examples_fit_the_row_up_to_four_times() {
        assert_eq!(example_size([100, 50], 300.0), egui::vec2(300.0, 150.0));
        assert_eq!(example_size([10, 5], 300.0), egui::vec2(40.0, 20.0));
        assert_eq!(example_size([400, 200], 200.0), egui::vec2(200.0, 100.0));
        assert_eq!(example_size([0, 0], 200.0), egui::Vec2::ZERO);
    }

    #[test]
    fn stepping_wraps_around() {
        assert_eq!(step(0, 3, true), 1);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(1, 3, false), 0);
    }

    #[test]
    fn cache_loads_once_and_remembers_failures() {
        let ctx = egui::Context::default();
        let upload = |name: &str, img| ctx.load_texture(name, img, egui::TextureOptions::NEAREST);
        let mut cache = ExampleCache::default();
        cache.request("a", 0.0, || Pending::ready(Ok(image())));
        cache.request("b", 0.0, || Pending::ready(Err(FetchError::NotFound)));
        assert!(cache.is_busy());
        cache.poll(0.0, upload);
        assert!(!cache.is_busy());
        assert!(matches!(cache.images["a"], Example::Ready(ref t) if t.size() == [2, 1]));
        assert!(matches!(cache.images["b"], Example::Failed));
        // Ni lo listo ni lo fallado se vuelve a pedir.
        cache.request("a", 1.0, || panic!("ya está cargada"));
        cache.request("b", 1.0, || panic!("ya falló"));
    }

    #[test]
    fn stalled_examples_fail_after_the_deadline() {
        let ctx = egui::Context::default();
        let upload = |name: &str, img| ctx.load_texture(name, img, egui::TextureOptions::NEAREST);
        let mut cache = ExampleCache::default();
        cache.request("a", 100.0, Pending::never);
        cache.poll(110.0, upload);
        assert!(cache.is_busy());
        cache.poll(100.0 + REQUEST_TIMEOUT_SECS + 1.0, upload);
        assert!(!cache.is_busy());
        assert!(matches!(cache.images["a"], Example::Failed));
    }

    #[test]
    fn only_a_few_examples_load_at_once() {
        let mut cache = ExampleCache::default();
        for i in 0..MAX_IN_FLIGHT + 2 {
            cache.request(&format!("u{i}"), 0.0, Pending::never);
        }
        assert_eq!(cache.in_flight(), MAX_IN_FLIGHT);
        assert_eq!(
            cache.images.len(),
            MAX_IN_FLIGHT,
            "las demás se piden después"
        );
    }
}
