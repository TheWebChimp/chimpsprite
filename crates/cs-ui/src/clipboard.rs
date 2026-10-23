//! Copiar, cortar y pegar lo seleccionado. El portapapeles es interno (lo guarda la app, así
//! sirve entre pestañas) y guarda colores resueltos para pegar en cualquier modo de color. En
//! escritorio también se pegan imágenes copiadas en otras apps (Aseprite, el navegador…).

use cs_commands::Command;
use cs_core::{ColorMode, LayerKind, Mask, PixelValue, Rgba};
use eframe::egui;

use crate::editor::Editor;

/// De dónde pegar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteRequest {
    /// Lo copiado en ChimpSprite (el portapapeles del sistema tiene el marcador).
    Internal { new_artboard: bool },
    /// Una imagen del portapapeles del sistema; si no hay, lo copiado en ChimpSprite.
    System { new_artboard: bool },
}

/// Ctrl+V (o Ctrl+Shift+V, en un artboard nuevo) en los eventos de este frame.
///
/// egui convierte Ctrl+V en `Event::Paste` solo si el portapapeles tiene texto; si tiene solo
/// una imagen, se traga la tecla al presionarla y solo llega al soltarla. `pasted` recuerda si
/// ya llegó un `Paste` para no pegar dos veces al soltar la V.
pub fn paste_request(
    events: &[egui::Event],
    modifiers: egui::Modifiers,
    pasted: &mut bool,
) -> Option<PasteRequest> {
    let mut request = None;
    for event in events {
        match event {
            egui::Event::Paste(text) => {
                *pasted = true;
                let new_artboard = modifiers.shift;
                request = Some(if text == crate::CLIPBOARD_MARKER {
                    PasteRequest::Internal { new_artboard }
                } else {
                    PasteRequest::System { new_artboard }
                });
            }
            egui::Event::Key {
                key: egui::Key::V,
                pressed: false,
                modifiers,
                ..
            } => {
                if modifiers.command && !*pasted {
                    request = Some(PasteRequest::System {
                        new_artboard: modifiers.shift,
                    });
                }
                *pasted = false;
            }
            _ => {}
        }
    }
    request
}

/// Imagen del portapapeles del sistema (`arboard` lee primero PNG, así conserva la
/// transparencia). En web no hay acceso síncrono al portapapeles.
#[cfg(not(target_arch = "wasm32"))]
pub fn system_image() -> Option<Clip> {
    let image = arboard::Clipboard::new().ok()?.get_image().ok()?;
    Clip::from_rgba(image.width as u32, image.height as u32, &image.bytes)
}

#[cfg(target_arch = "wasm32")]
pub fn system_image() -> Option<Clip> {
    None
}

/// Píxeles copiados, relativos a la esquina de la selección.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// Esquina superior izquierda de la selección de origen.
    origin: (i32, i32),
    size: (u32, u32),
    /// Todos los puntos seleccionados (también los transparentes: dan la forma de la selección).
    pixels: Vec<(i32, i32, Rgba)>,
}

impl Clip {
    /// Imagen RGBA de otra app (p. ej. copiada en Aseprite): todos sus píxeles, con origen en 0,0.
    /// `None` si está vacía o faltan bytes.
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Self> {
        let len = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        if width == 0 || height == 0 || rgba.len() < len {
            return None;
        }
        let pixels = rgba[..len]
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (x, y) = (i as u32 % width, i as u32 / width);
                (x as i32, y as i32, Rgba::new(p[0], p[1], p[2], p[3]))
            })
            .collect();
        Some(Self {
            origin: (0, 0),
            size: (width, height),
            pixels,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// El mismo contenido, centrado en un lienzo de `width`×`height`.
    pub fn centered_in(&self, width: u32, height: u32) -> Self {
        let center = |canvas: u32, size: u32| (canvas as i32 - size as i32) / 2;
        Self {
            origin: (center(width, self.size.0), center(height, self.size.1)),
            ..self.clone()
        }
    }
}

impl Editor {
    /// Capa de imagen actual, si se puede leer píxel a píxel (no tilemaps, grupos ni instancias).
    pub(crate) fn clip_layer_ok(&self, write: bool) -> bool {
        !self.editing_mask()
            && !self.tiles_mode()
            && self
                .sprite()
                .layer(self.layer)
                .is_some_and(|l| l.kind == LayerKind::Image && !(write && l.locked))
    }

    /// Copia lo seleccionado de la capa actual (sin selección no hay nada que copiar).
    pub fn copy(&self) -> Option<Clip> {
        let mask = self.selection.as_ref()?;
        let ((x0, y0), (x1, y1)) = mask.bounds()?;
        if !self.clip_layer_ok(false) {
            return None;
        }
        let sprite = self.sprite();
        let pixels = mask
            .points()
            .map(|(x, y)| {
                let c = self.resolve(sprite.pixel(self.layer, self.frame, x, y));
                (x - x0, y - y0, c)
            })
            .collect();
        Some(Clip {
            origin: (x0, y0),
            size: ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32),
            pixels,
        })
    }

    /// Copia y deja transparente lo seleccionado (un paso de undo).
    pub fn cut(&mut self) -> Option<Clip> {
        if !self.clip_layer_ok(true) {
            return None;
        }
        self.commit_floating();
        let clip = self.copy()?;
        self.clear_selection_content();
        Some(clip)
    }

    /// Pega en la capa actual donde estaba lo copiado (corrido para que entre en el lienzo) y
    /// selecciona lo pegado, listo para arrastrarlo. Lo transparente no borra.
    pub fn paste(&mut self, clip: &Clip) {
        if !self.clip_layer_ok(true) {
            return;
        }
        self.cancel_gesture();
        let sprite = self.sprite();
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        let fit = |o: i32, size: u32, max: i32| o.min(max - size as i32).max(0);
        let (ox, oy) = (
            fit(clip.origin.0, clip.size.0, w),
            fit(clip.origin.1, clip.size.1, h),
        );
        let value = |c: Rgba| match sprite.color_mode {
            ColorMode::Rgba => PixelValue::Rgba(c),
            ColorMode::Indexed { transparent_index } => {
                PixelValue::Index(cs_io::import::nearest_index(sprite, c, transparent_index))
            }
        };
        // Lo que no entra en el lienzo (una imagen más grande) se recorta.
        let inside = |&(x, y, _): &&(i32, i32, Rgba)| {
            (0..w).contains(&(ox + x)) && (0..h).contains(&(oy + y))
        };
        let pixels: Vec<_> = clip
            .pixels
            .iter()
            .filter(inside)
            .filter(|(_, _, c)| c.a > 0)
            .map(|&(x, y, c)| (ox + x, oy + y, value(c)))
            .collect();
        let mask = Mask::from_points(
            sprite.width,
            sprite.height,
            clip.pixels
                .iter()
                .filter(inside)
                .map(|&(x, y, _)| (ox + x, oy + y)),
        );
        let applied = pixels.is_empty()
            || self.execute(Command::DrawPixels {
                layer: self.layer,
                frame: self.frame,
                pixels,
            });
        if applied {
            self.set_selection(Some(mask));
        }
    }

    /// Crea un artboard del tamaño de lo copiado (con la paleta y el modo del actual) y lo pega
    /// en su esquina.
    pub fn paste_in_new_artboard(&mut self, clip: &Clip) {
        let before = self.active;
        self.add_artboard(clip.size.0, clip.size.1);
        if self.active != before {
            self.paste(&Clip {
                origin: (0, 0),
                ..clip.clone()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{LayerId, Sprite};

    use super::*;

    fn editor_with_dot() -> (Editor, PixelValue) {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = e.palette_value(5);
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(2, 2, red)],
        });
        (e, red)
    }

    #[test]
    fn copy_needs_a_selection_and_paste_selects_what_was_pasted() {
        let (mut e, red) = editor_with_dot();
        assert_eq!(e.copy(), None);
        e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (3, 3))));
        let clip = e.copy().unwrap();
        e.add_layer(LayerKind::Image);
        let top = e.layer;
        e.paste(&clip);
        assert_eq!(
            e.sprite().pixel(top, 0, 2, 2),
            red,
            "same place, current layer"
        );
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((1, 1), (3, 3)))
        );
        e.undo();
        assert_eq!(
            e.sprite().pixel(top, 0, 2, 2),
            e.sprite().color_mode.transparent()
        );
    }

    #[test]
    fn cut_clears_and_paste_fits_inside_a_smaller_canvas() {
        let (mut e, red) = editor_with_dot();
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (7, 7))));
        let clip = e.cut().unwrap();
        let t = e.sprite().color_mode.transparent();
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), t);

        let mut small = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        small.paste(&clip);
        assert_eq!(
            small.sprite().pixel(LayerId(1), 0, 0, 0),
            red,
            "moved to fit"
        );
        assert_eq!(
            small.selection.as_ref().unwrap().bounds(),
            Some(((0, 0), (3, 3)))
        );
    }

    #[test]
    fn paste_into_an_indexed_sprite_uses_the_nearest_palette_entry() {
        let (mut e, red) = editor_with_dot();
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        let mut indexed = Editor::with_sprite(Sprite::new(
            8,
            8,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        indexed.paste(&clip);
        let v = indexed.sprite().pixel(LayerId(1), 0, 2, 2);
        assert_eq!(indexed.resolve(v), e.resolve(red));
    }

    fn release_v(modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn paste_requests_from_events() {
        use egui::{Event, Modifiers};
        let none = Modifiers::NONE;
        let shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let mut seen = false;
        let marker = Event::Paste(crate::CLIPBOARD_MARKER.into());
        // Lo copiado en ChimpSprite: llega Paste al presionar y la tecla al soltar.
        assert_eq!(
            paste_request(std::slice::from_ref(&marker), none, &mut seen),
            Some(PasteRequest::Internal {
                new_artboard: false
            })
        );
        assert_eq!(
            paste_request(&[release_v(Modifiers::COMMAND)], none, &mut seen),
            None
        );
        // Imagen de otra app: egui no manda Paste (no hay texto), solo la tecla al soltar.
        assert_eq!(
            paste_request(&[release_v(Modifiers::COMMAND)], none, &mut seen),
            Some(PasteRequest::System {
                new_artboard: false
            })
        );
        assert_eq!(
            paste_request(&[release_v(shift)], none, &mut seen),
            Some(PasteRequest::System { new_artboard: true })
        );
        assert_eq!(
            paste_request(&[marker], shift, &mut seen),
            Some(PasteRequest::Internal { new_artboard: true })
        );
        seen = false;
        assert_eq!(
            paste_request(&[Event::Paste("hello".into())], none, &mut seen),
            Some(PasteRequest::System {
                new_artboard: false
            })
        );
        assert_eq!(paste_request(&[release_v(none)], none, &mut false), None);
    }

    /// 2×1: rojo opaco y un píxel transparente.
    const RED_AND_CLEAR: [u8; 8] = [255, 0, 0, 255, 0, 0, 0, 0];

    #[test]
    fn external_images_are_pasted_centered() {
        let clip = Clip::from_rgba(2, 1, &RED_AND_CLEAR).unwrap();
        assert_eq!(clip.size(), (2, 1));
        assert_eq!(Clip::from_rgba(2, 2, &RED_AND_CLEAR), None, "faltan bytes");
        assert_eq!(Clip::from_rgba(0, 1, &[]), None);
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.paste(&clip.centered_in(8, 8));
        let red = PixelValue::Rgba(Rgba::new(255, 0, 0, 255));
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 3, 3), red);
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((3, 3), (4, 3)))
        );
    }

    #[test]
    fn images_larger_than_the_canvas_are_cropped() {
        let big = Clip::from_rgba(10, 10, &[200; 400]).unwrap();
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        e.paste(&big.centered_in(4, 4));
        let c = PixelValue::Rgba(Rgba::new(200, 200, 200, 200));
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 3, 3), c);
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((0, 0), (3, 3)))
        );
    }

    #[test]
    fn paste_in_a_new_artboard_of_the_clip_size() {
        let clip = Clip::from_rgba(2, 1, &RED_AND_CLEAR).unwrap();
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let first = e.active;
        e.paste_in_new_artboard(&clip);
        assert_eq!(e.document.artboards.len(), 2);
        assert_ne!(e.active, first);
        let sprite = e.sprite();
        assert_eq!((sprite.width, sprite.height), (2, 1));
        assert_eq!(sprite.palette, Sprite::new(1, 1, ColorMode::Rgba).palette);
        let red = PixelValue::Rgba(Rgba::new(255, 0, 0, 255));
        assert_eq!(sprite.pixel(e.layer, 0, 0, 0), red);
    }

    #[test]
    fn locked_layers_are_not_cut_or_pasted() {
        let (mut e, red) = editor_with_dot();
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        e.toggle_locked(LayerId(1));
        assert_eq!(e.cut(), None);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), red);
        let version = e.doc_version;
        e.paste(&clip);
        assert_eq!(e.doc_version, version);
    }
}
