//! Copiar, cortar y pegar lo seleccionado. El portapapeles es interno (lo guarda la app, así
//! sirve entre pestañas) y guarda colores resueltos para pegar en cualquier modo de color.

use cs_commands::Command;
use cs_core::{ColorMode, LayerKind, Mask, PixelValue, Rgba};

use crate::editor::Editor;

/// Píxeles copiados, relativos a la esquina de la selección.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// Esquina superior izquierda de la selección de origen.
    origin: (i32, i32),
    size: (u32, u32),
    /// Todos los puntos seleccionados (también los transparentes: dan la forma de la selección).
    pixels: Vec<(i32, i32, Rgba)>,
}

impl Editor {
    /// Capa de imagen actual, si se puede leer píxel a píxel (no tilemaps, grupos ni instancias).
    fn clip_layer_ok(&self, write: bool) -> bool {
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
        let pixels: Vec<_> = clip
            .pixels
            .iter()
            .filter(|(_, _, c)| c.a > 0)
            .map(|&(x, y, c)| (ox + x, oy + y, value(c)))
            .collect();
        let mask = Mask::from_points(
            sprite.width,
            sprite.height,
            clip.pixels.iter().map(|&(x, y, _)| (ox + x, oy + y)),
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
