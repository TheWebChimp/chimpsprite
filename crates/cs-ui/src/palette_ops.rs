//! Aplicar una paleta al sprite activo (`Editor::apply_palette`).

use cs_commands::Command;
use cs_core::{Palette, PixelValue};

use crate::Editor;

impl Editor {
    /// Cambia la paleta del sprite activo en un paso de undo. En RGBA los píxeles no cambian; en
    /// indexado cada índice toma el color nuevo de su posición, como en Aseprite.
    pub fn apply_palette(&mut self, palette: Palette) -> bool {
        if palette.is_empty() || palette == self.sprite().palette {
            return false;
        }
        if !self.execute(Command::SetPalette(palette)) {
            return false;
        }
        self.primary = self.fit_to_palette(self.primary);
        self.secondary = self.fit_to_palette(self.secondary);
        true
    }

    /// Un índice fuera de la paleta pasa al 0 (el Mask se queda como está).
    fn fit_to_palette(&self, value: PixelValue) -> PixelValue {
        match value {
            PixelValue::Index(i)
                if !self.is_mask(value) && i as usize >= self.sprite().palette.len() =>
            {
                PixelValue::Index(0)
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Palette, PixelValue, Rgba, Sprite};

    use crate::Editor;

    fn colors(n: u32) -> Palette {
        Palette::new((0..n).map(|i| Rgba::from_hex(i * 0x010101)).collect())
    }

    #[test]
    fn rgba_apply_is_one_undo_step_and_keeps_pixels() {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let before = e.sprite().palette.clone();
        assert!(e.apply_palette(colors(4)));
        assert_eq!(e.sprite().palette.len(), 4);
        e.undo();
        assert_eq!(e.sprite().palette, before);
        assert!(
            !e.apply_palette(before.clone()),
            "la misma paleta no hace nada"
        );
        assert!(
            !e.apply_palette(Palette::new(Vec::new())),
            "vacía no se aplica"
        );
    }

    #[test]
    fn indexed_colors_outside_the_new_palette_go_to_zero() {
        let mut e = Editor::with_sprite(Sprite::new(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        e.primary = PixelValue::Index(30);
        e.secondary = PixelValue::Index(2);
        assert!(e.apply_palette(colors(8)));
        assert_eq!(e.primary, PixelValue::Index(0));
        assert_eq!(e.secondary, PixelValue::Index(2));
    }

    #[test]
    fn new_editor_uses_the_given_palette() {
        let e = crate::new_editor(colors(4));
        assert_eq!(e.sprite().palette, colors(4));
        // Con menos de 22 colores, el secundario no cae fuera de la paleta.
        assert_eq!(e.secondary, e.palette_value(3));
    }
}
