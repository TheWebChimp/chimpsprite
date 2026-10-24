//! Aplicar una paleta al sprite activo (`Editor::apply_palette`).

use cs_commands::Command;
use cs_core::{Palette, PixelValue, Rgba};

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

    /// El color que edita el editor de color con Pixel UI: el primario o, si se eligió la mitad
    /// de abajo de la caja de los colores, el secundario.
    pub fn edited_color(&self) -> PixelValue {
        if self.editing_secondary {
            self.secondary
        } else {
            self.primary
        }
    }

    /// Cambia el color que edita el editor de color (ver `edited_color`).
    pub fn set_edited_color(&mut self, value: PixelValue) {
        if self.editing_secondary {
            self.secondary = value;
        } else {
            self.primary = value;
        }
    }

    /// La entrada de la paleta que el editor de color edita junto con `value` (el color que se
    /// está editando): solo con la paleta desbloqueada. En indexado, la del índice (salvo el
    /// Mask); en RGBA, la última elegida con un clic (`palette_entry`) mientras siga siendo ese
    /// color.
    pub fn palette_entry_of(&self, value: PixelValue) -> Option<u8> {
        if self.palette_locked || self.is_mask(value) {
            return None;
        }
        let colors = self.sprite().palette.colors();
        match value {
            PixelValue::Index(i) => ((i as usize) < colors.len()).then_some(i),
            PixelValue::Rgba(c) => self
                .palette_entry
                .filter(|&i| colors.get(i as usize) == Some(&c)),
        }
    }

    /// Cambia el color de la entrada `index` de la paleta (editarla "individualmente"). Con la
    /// paleta bloqueada no hace nada. Los cambios seguidos del mismo `group` (un arrastre) son
    /// un solo paso de undo.
    pub fn set_palette_color(&mut self, index: u8, color: Rgba, group: u64) -> bool {
        let palette = &self.sprite().palette;
        if self.palette_locked
            || palette
                .colors()
                .get(index as usize)
                .is_none_or(|&c| c == color)
        {
            return false;
        }
        let mut colors = palette.colors().to_vec();
        colors[index as usize] = color;
        self.execute_grouped(Command::SetPalette(Palette::new(colors)), group)
    }

    /// Una paleta nueva con solo el color primario (negro si el primario es el Mask), en un paso
    /// de undo. Se puede con la paleta bloqueada: no edita un color, la reemplaza entera.
    pub fn new_palette(&mut self) -> bool {
        let color = if self.is_mask(self.primary) {
            Rgba::rgb(0, 0, 0)
        } else {
            self.resolve(self.primary)
        };
        self.palette_entry = None;
        self.apply_palette(Palette::new(vec![color]))
    }

    /// El botón del Mask: lo elige como color primario o, si ya lo es, vuelve al de antes.
    pub fn toggle_mask(&mut self) {
        if self.is_mask(self.primary) {
            let back = self.before_mask.take().filter(|v| !self.is_mask(*v));
            self.primary = back.unwrap_or_else(|| self.palette_value(0));
        } else {
            self.before_mask = Some(self.primary);
            self.primary = self.mask_value();
        }
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

    fn rgba_editor() -> Editor {
        let mut sprite = Sprite::new(4, 4, ColorMode::Rgba);
        sprite.palette = colors(4);
        Editor::with_sprite(sprite)
    }

    #[test]
    fn the_palette_starts_locked_and_a_locked_entry_is_not_edited() {
        let mut e = rgba_editor();
        assert!(e.palette_locked);
        e.primary = e.palette_value(2);
        e.palette_entry = Some(2);
        assert_eq!(e.palette_entry_of(e.primary), None, "locked");
        assert!(!e.set_palette_color(2, Rgba::rgb(9, 9, 9), 1));
        assert_eq!(e.sprite().palette, colors(4));
    }

    #[test]
    fn unlocked_the_picked_entry_is_edited_in_one_undo_step_per_gesture() {
        let mut e = rgba_editor();
        e.palette_locked = false;
        e.primary = e.palette_value(2);
        e.palette_entry = Some(2);
        assert_eq!(e.palette_entry_of(e.primary), Some(2));
        // Otro color (o ninguna entrada elegida): no hay entrada.
        assert_eq!(
            e.palette_entry_of(PixelValue::Rgba(Rgba::rgb(1, 1, 1))),
            None
        );
        assert!(e.set_palette_color(2, Rgba::rgb(9, 9, 9), 7));
        assert!(e.set_palette_color(2, Rgba::rgb(8, 8, 8), 7));
        assert_eq!(e.sprite().palette.colors()[2], Rgba::rgb(8, 8, 8));
        e.end_group_of(7);
        e.undo();
        assert_eq!(e.sprite().palette, colors(4), "one step");
        // Fuera de la paleta, nada.
        assert!(!e.set_palette_color(9, Rgba::rgb(9, 9, 9), 7));
    }

    #[test]
    fn unlocked_an_indexed_primary_edits_its_entry_but_not_the_mask() {
        let mut e = Editor::with_sprite(Sprite::new(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        e.palette_locked = false;
        assert_eq!(e.palette_entry_of(PixelValue::Index(3)), Some(3));
        assert_eq!(e.palette_entry_of(e.mask_value()), None);
        e.palette_locked = true;
        assert_eq!(e.palette_entry_of(PixelValue::Index(3)), None);
    }

    #[test]
    fn a_new_palette_has_just_the_primary_and_is_undone() {
        let mut e = rgba_editor();
        e.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
        assert!(e.new_palette(), "also when locked");
        assert_eq!(e.sprite().palette.colors(), &[Rgba::rgb(1, 2, 3)]);
        e.undo();
        assert_eq!(e.sprite().palette, colors(4));
        e.primary = e.mask_value();
        assert!(e.new_palette());
        assert_eq!(e.sprite().palette.colors(), &[Rgba::rgb(0, 0, 0)]);
    }

    #[test]
    fn the_mask_button_picks_the_mask_and_goes_back() {
        let mut e = rgba_editor();
        let before = e.palette_value(1);
        e.primary = before;
        e.toggle_mask();
        assert!(e.is_mask(e.primary));
        e.toggle_mask();
        assert_eq!(e.primary, before);
        // Si el Mask llegó por otro lado, apagarlo vuelve al primer color.
        e.primary = e.mask_value();
        e.toggle_mask();
        assert_eq!(e.primary, e.palette_value(0));
    }

    #[test]
    fn new_editor_uses_the_given_palette() {
        let e = crate::new_editor(colors(4));
        assert_eq!(e.sprite().palette, colors(4));
        // Con menos de 22 colores, el secundario no cae fuera de la paleta.
        assert_eq!(e.secondary, e.palette_value(3));
    }
}
