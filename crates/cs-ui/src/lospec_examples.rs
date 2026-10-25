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

/// Debajo de la tira de colores: el ejemplo actual (o un hueco con spinner mientras carga) y,
/// si hay más de uno, ◀ 1/5 ▶ para pasar entre ellos. Si falla, no se muestra nada.
pub fn show(ui: &mut egui::Ui, p: &LospecPalette, cache: &mut ExampleCache, now: f64) {
    let len = p.examples.len();
    if len == 0 {
        return;
    }
    let index = cache.shown.get(&p.slug).copied().unwrap_or(0).min(len - 1);
    let url = &p.examples[index];
    let width = ui.available_width();
    match cache.images.get(url) {
        Some(Example::Ready(texture)) => {
            let size = example_size(texture.size(), width);
            ui.add(
                egui::Image::new(egui::load::SizedTexture::new(texture.id(), size))
                    .fit_to_exact_size(size),
            );
        }
        Some(Example::Failed) => {}
        Some(Example::Loading { .. }) | None => {
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(width, PLACEHOLDER_HEIGHT), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let ctx = ui.ctx().clone();
                cache.request(url, now, || lospec_client::fetch_example(&ctx, url));
                ui.put(rect, egui::Spinner::new());
            }
        }
    }
    if len > 1 {
        ui.horizontal(|ui| {
            let mut next = index;
            if crate::pixel_ui::text_button(ui, "<").clicked() {
                next = step(index, len, false);
            }
            ui.weak(format!("{}/{len}", index + 1));
            if crate::pixel_ui::text_button(ui, ">").clicked() {
                next = step(index, len, true);
            }
            if next != index {
                cache.shown.insert(p.slug.clone(), next);
            }
        });
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
