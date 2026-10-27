//! Copiar, cortar y pegar lo seleccionado. El portapapeles es interno (lo guarda la app, así
//! sirve entre pestañas) y guarda colores resueltos para pegar en cualquier modo de color. En
//! escritorio también se pegan imágenes copiadas en otras apps (Aseprite, el navegador…).

use cs_core::{ColorMode, LayerKind, PixelValue, Rgba};
use cs_raster::{Block, MAX_BLOCK_SIDE};
use eframe::egui;

use crate::editor::{Editor, ToolKind};

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

    /// Copia lo seleccionado de la capa actual (sin selección no hay nada que copiar). Lo flotante
    /// se confirma antes, así se copia lo que se ve.
    pub fn copy(&mut self) -> Option<Clip> {
        self.commit_floating();
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

    /// Edit → Copy Merged (Ctrl+Shift+C): copia lo seleccionado como se ve, con todas las capas
    /// visibles juntas.
    pub fn copy_merged(&mut self) -> Option<Clip> {
        self.commit_floating();
        let mask = self.selection.as_ref()?;
        let ((x0, y0), (x1, y1)) = mask.bounds()?;
        let view = self.view();
        let document = view.document.as_ref();
        let artboard = document.artboard(self.active)?;
        let frame = self.frame;
        let sprite = cs_render::materialize(document, &artboard.sprite, frame..frame + 1);
        let pixels = mask
            .points()
            .map(|(x, y)| {
                (
                    x - x0,
                    y - y0,
                    cs_render::composite_pixel(&sprite, frame, x, y),
                )
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
        self.named("Cut", Self::clear_selection_content);
        Some(clip)
    }

    /// Pega en la capa actual donde estaba lo copiado (o centrado, si es una imagen de otra app;
    /// corrido para que entre en el lienzo). Lo pegado queda flotando y seleccionado, listo para
    /// moverlo, escalarlo o rotarlo. Una imagen más grande que el lienzo flota entera y se recorta
    /// al confirmar; lo transparente no borra lo de debajo. Con una herramienta de dibujo pasa a
    /// Move (como Aseprite), así lo pegado sigue flotando en vez de confirmarse al frame siguiente.
    pub fn paste(&mut self, clip: &Clip) {
        self.paste_clip(clip, true);
    }

    /// `keep_floating`: cambiar a Move si la herramienta no deja transformar.
    fn paste_clip(&mut self, clip: &Clip, keep_floating: bool) {
        if !self.clip_layer_ok(true) {
            return;
        }
        self.cancel_gesture();
        self.commit_floating();
        // Antes de levantar: cambiar de herramienta confirma lo flotante.
        if keep_floating && !(self.tool.is_selection() || self.tool == ToolKind::Move) {
            self.set_tool(ToolKind::Move);
        }
        // Si va a quedar flotando, más grande que `MAX_BLOCK_SIDE` no se podría mover ni
        // transformar: se conserva el centro (en su lugar) y se descarta lo que sobra a los
        // costados. Si se confirma en el momento (artboard nuevo) se pega entero.
        let keep = |size: u32| {
            if keep_floating {
                size.min(MAX_BLOCK_SIDE)
            } else {
                size
            }
        };
        let size = (keep(clip.size.0), keep(clip.size.1));
        let crop = (
            ((clip.size.0 - size.0) / 2) as i32,
            ((clip.size.1 - size.1) / 2) as i32,
        );
        let origin = (clip.origin.0 + crop.0, clip.origin.1 + crop.1);
        let sprite = self.sprite();
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        let fit = |o: i32, size: u32, max: i32| o.min(max - size as i32).max(0);
        let (ox, oy) = (fit(origin.0, size.0, w), fit(origin.1, size.1, h));
        let clear = sprite.color_mode.transparent();
        let value = |c: Rgba| match sprite.color_mode {
            _ if c.a == 0 => clear,
            ColorMode::Rgba | ColorMode::Grayscale => PixelValue::Rgba(c),
            ColorMode::Indexed { transparent_index } => {
                PixelValue::Index(cs_io::import::nearest_index(sprite, c, transparent_index))
            }
        };
        // Los puntos transparentes también viajan: dan la forma de la selección.
        let mut block = Block::new(size.0, size.1);
        for &(x, y, c) in &clip.pixels {
            block.set(x - crop.0, y - crop.1, Some(value(c)));
        }
        self.float_pasted(block, (ox, oy));
    }

    /// Crea un artboard del tamaño de lo copiado (con la paleta y el modo del actual) y lo pega
    /// en su esquina, ya aplicado: el artboard tiene justo ese tamaño, no hay nada que acomodar.
    pub fn paste_in_new_artboard(&mut self, clip: &Clip) {
        let before = self.active;
        self.add_artboard(clip.size.0, clip.size.1);
        if self.active != before {
            // Se confirma en el momento: la herramienta no cambia.
            let clip = Clip {
                origin: (0, 0),
                ..clip.clone()
            };
            self.paste_clip(&clip, false);
            self.commit_floating();
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{LayerId, Mask, Sprite};

    use super::*;
    use crate::editor::ToolKind;

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
    fn copy_merged_takes_every_visible_layer() {
        let (mut e, red) = editor_with_dot();
        e.add_layer(LayerKind::Image);
        let top = e.layer;
        let blue = e.palette_value(9);
        e.execute(Command::DrawPixels {
            layer: top,
            frame: 0,
            pixels: vec![(3, 2, blue)],
        });
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (3, 2))));
        let merged = e.copy_merged().expect("selection");
        let colors: Vec<_> = merged.pixels.iter().map(|p| p.2).collect();
        assert_eq!(colors, vec![e.resolve(red), e.resolve(blue)]);
        // Copy solo copia la capa actual.
        let plain = e.copy().expect("selection");
        assert_eq!(plain.pixels[0].2.a, 0);
    }

    #[test]
    fn copy_needs_a_selection_and_paste_floats_until_applied() {
        let (mut e, red) = editor_with_dot();
        assert_eq!(e.copy(), None);
        e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (3, 3))));
        let clip = e.copy().unwrap();
        e.add_layer(LayerKind::Image);
        let top = e.layer;
        let t = e.sprite().color_mode.transparent();
        e.paste(&clip);
        e.sync_floating(); // el frame siguiente no lo confirma
        assert!(e.is_floating());
        assert_eq!(e.sprite().pixel(top, 0, 2, 2), t, "not applied yet");
        let shown = e.view().document.artboards[0].sprite.pixel(top, 0, 2, 2);
        assert_eq!(shown, red, "same place, current layer");
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((1, 1), (3, 3)))
        );
        assert!(e.commit_floating());
        assert_eq!(e.sprite().pixel(top, 0, 2, 2), red);
        e.undo();
        assert_eq!(e.sprite().pixel(top, 0, 2, 2), t);
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
        assert!(small.commit_floating());
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
        assert!(indexed.commit_floating());
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
    fn external_images_are_pasted_centered_and_float() {
        let clip = Clip::from_rgba(2, 1, &RED_AND_CLEAR).unwrap();
        assert_eq!(clip.size(), (2, 1));
        assert_eq!(Clip::from_rgba(2, 2, &RED_AND_CLEAR), None, "faltan bytes");
        assert_eq!(Clip::from_rgba(0, 1, &[]), None);
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let t = e.sprite().color_mode.transparent();
        e.paste(&clip.centered_in(8, 8));
        e.sync_floating();
        assert!(
            e.is_floating(),
            "an external image floats like an internal clip"
        );
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 3, 3), t, "not applied yet");
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((3, 3), (4, 3)))
        );
        assert!(e.commit_floating());
        let red = PixelValue::Rgba(Rgba::new(255, 0, 0, 255));
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 3, 3), red);
        assert_eq!(
            e.sprite().pixel(LayerId(1), 0, 4, 3),
            t,
            "the clear pixel does not erase"
        );
    }

    #[test]
    fn images_larger_than_the_canvas_float_whole_and_are_cropped_on_apply() {
        let big = Clip::from_rgba(10, 10, &[200; 400]).unwrap();
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        e.paste(&big.centered_in(4, 4));
        e.sync_floating();
        assert_eq!(
            e.transform_box().map(|b| (b.width(), b.height())),
            Some((10, 10)),
            "the whole image floats: it can be scaled down before applying"
        );
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((0, 0), (3, 3))),
            "the selection is the part inside the canvas"
        );
        assert!(e.commit_floating());
        let c = PixelValue::Rgba(Rgba::new(200, 200, 200, 200));
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 3, 3), c);
    }

    #[test]
    fn paste_in_a_new_artboard_of_the_clip_size_applies_at_once() {
        let clip = Clip::from_rgba(2, 1, &RED_AND_CLEAR).unwrap();
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let first = e.active;
        e.paste_in_new_artboard(&clip);
        e.sync_floating();
        assert_eq!(e.tool, ToolKind::Pencil, "nothing floats: the tool stays");
        assert_eq!(e.document.artboards.len(), 2);
        assert_ne!(e.active, first);
        assert!(
            !e.is_floating(),
            "the artboard is the clip's size: nothing to place"
        );
        let sprite = e.sprite();
        assert_eq!((sprite.width, sprite.height), (2, 1));
        assert_eq!(sprite.palette, Sprite::new(1, 1, ColorMode::Rgba).palette);
        let red = PixelValue::Rgba(Rgba::new(255, 0, 0, 255));
        assert_eq!(sprite.pixel(e.layer, 0, 0, 0), red);
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((0, 0), (1, 0)))
        );
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
        assert!(!e.is_floating());
    }

    #[test]
    fn pasted_content_floats_over_the_drawing_below_without_mixing() {
        let (mut e, red) = editor_with_dot();
        let blue = e.palette_value(9);
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(6, 2, blue)],
        });
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        e.paste(&clip); // flota en (2, 2)
        e.sync_floating();
        e.tool = ToolKind::RectSelect;
        // Pasa por encima del azul y se deja más abajo.
        e.pointer_press((2.5, 2.5), false, false, 0.1);
        e.pointer_drag((6.5, 2.5));
        e.pointer_drag((6.5, 5.5));
        e.pointer_release();
        assert!(e.commit_floating());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 6, 5), red);
        assert_eq!(
            e.sprite().pixel(LayerId(1), 0, 6, 2),
            blue,
            "the drawing below is intact"
        );
        assert_eq!(
            e.sprite().pixel(LayerId(1), 0, 2, 2),
            red,
            "pasting leaves no hole"
        );
    }

    #[test]
    fn cancelling_a_paste_restores_the_previous_selection() {
        let (mut e, _) = editor_with_dot();
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        e.set_selection(Some(Mask::from_rect(8, 8, (5, 5), (6, 6))));
        let version = e.doc_version;
        e.paste(&clip);
        e.sync_floating();
        e.escape();
        assert!(!e.is_floating());
        assert_eq!(e.doc_version, version);
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((5, 5), (6, 6)))
        );
    }

    #[test]
    fn pasting_with_a_drawing_tool_switches_to_move_and_keeps_floating() {
        let (mut e, red) = editor_with_dot();
        assert_eq!(e.tool, ToolKind::Pencil);
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        e.paste(&clip);
        e.sync_floating();
        assert!(e.is_floating(), "the next frame does not apply it");
        assert_eq!(e.tool, ToolKind::Move);
        assert!(e.commit_floating());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), red);
    }

    #[test]
    fn pasting_with_a_selection_tool_keeps_the_tool_and_floats() {
        let (mut e, _) = editor_with_dot();
        e.set_tool(ToolKind::RectSelect);
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        e.paste(&clip);
        e.sync_floating();
        assert!(e.is_floating());
        assert_eq!(e.tool, ToolKind::RectSelect);
    }

    #[test]
    fn pasting_an_external_image_with_a_drawing_tool_switches_to_move() {
        let clip = Clip::from_rgba(2, 1, &RED_AND_CLEAR).unwrap();
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        assert_eq!(e.tool, ToolKind::Pencil);
        e.paste(&clip.centered_in(8, 8));
        e.sync_floating();
        assert!(e.is_floating());
        assert_eq!(e.tool, ToolKind::Move);
    }

    #[test]
    fn a_paste_wider_than_the_block_limit_is_cropped_and_can_be_moved() {
        let side = cs_raster::MAX_BLOCK_SIDE;
        let mut rgba = vec![0u8; 5000 * 10 * 4];
        // Se conserva el centro: las columnas 452..4548. Una marca roja en la primera que queda y
        // una azul en la última que se descarta a la izquierda.
        let mut mark = |x: usize, c: [u8; 4]| {
            let at = (5 * 5000 + x) * 4;
            rgba[at..at + 4].copy_from_slice(&c);
        };
        mark(452, [255, 0, 0, 255]);
        mark(451, [0, 0, 255, 255]);
        let clip = Clip::from_rgba(5000, 10, &rgba).unwrap();
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.paste(&clip.centered_in(8, 8));
        assert!(e.is_floating());
        let b = e.transform_box().unwrap();
        assert!(
            b.width() as u32 <= side && b.height() as u32 <= side,
            "{b:?}"
        );
        assert_eq!(b.width() as u32, side);
        assert_eq!(b.height(), 10);
        // Se mueve con Move.
        assert_eq!(e.tool, ToolKind::Move);
        e.pointer_press((1.5, 1.5), false, false, 0.1);
        e.pointer_drag((3.5, 2.5));
        e.pointer_release();
        assert_eq!(e.transform_box(), Some(b.translated(2, 1)));
        assert!(e.commit_floating());
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let t = e.sprite().color_mode.transparent();
        // Lo pegado quedó en (0, 0) (corrido para entrar en el lienzo) y se movió (2, 1).
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 6), red);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 6), t, "cropped out");
    }

    #[test]
    fn paste_in_a_new_artboard_keeps_the_whole_image_even_past_the_block_limit() {
        let mut rgba = vec![0u8; 5000 * 10 * 4];
        rgba[..4].copy_from_slice(&[255, 0, 0, 255]);
        let last = (9 * 5000 + 4999) * 4;
        rgba[last..last + 4].copy_from_slice(&[255, 0, 0, 255]);
        let clip = Clip::from_rgba(5000, 10, &rgba).unwrap();
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.paste_in_new_artboard(&clip);
        assert!(!e.is_floating());
        let s = e.sprite();
        assert_eq!((s.width, s.height), (5000, 10));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        assert_eq!(s.pixel(LayerId(1), 0, 0, 0), red);
        assert_eq!(s.pixel(LayerId(1), 0, 4999, 9), red);
    }
}
