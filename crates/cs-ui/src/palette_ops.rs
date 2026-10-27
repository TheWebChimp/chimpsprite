//! Aplicar una paleta al sprite activo (`Editor::apply_palette`).

use cs_commands::Command;
use cs_core::{ColorMode, Image, Palette, PixelBuffer, PixelValue, Rgba};

use crate::Editor;

impl Editor {
    /// Agrega al final de la paleta del sprite los colores de `palette` que no tiene (hasta 256).
    /// Un paso de undo. Devuelve si cambió.
    pub fn append_palette(&mut self, palette: &Palette) -> bool {
        let mut colors = self.sprite().palette.colors().to_vec();
        for &c in palette.colors() {
            if colors.len() >= 256 {
                break;
            }
            if !colors.contains(&c) {
                colors.push(c);
            }
        }
        self.apply_palette(Palette::new(colors))
    }

    /// Los cuadritos seleccionados de la paleta, en orden (dentro de la paleta).
    pub fn palette_range(&self) -> Option<std::ops::RangeInclusive<usize>> {
        let (a, b) = self.palette_selection?;
        let len = self.sprite().palette.len();
        let (lo, hi) = (a.min(b), a.max(b).min(len.checked_sub(1)?));
        (lo <= hi).then_some(lo..=hi)
    }

    /// Borra las entradas `range` de la paleta (queda al menos una), en un paso de undo. En
    /// indexado los píxeles siguen con su color: los índices de después se corren y los de las
    /// borradas pasan al color más parecido que queda; la entrada transparente no se borra.
    pub fn delete_palette_entries(&mut self, range: std::ops::RangeInclusive<usize>) -> bool {
        let old = self.sprite().palette.clone();
        let (lo, hi) = (
            *range.start(),
            (*range.end()).min(old.len().saturating_sub(1)),
        );
        if lo > hi || hi - lo + 1 >= old.len() {
            return false;
        }
        let kept: Vec<Rgba> = old
            .colors()
            .iter()
            .enumerate()
            .filter(|(i, _)| !(lo..=hi).contains(i))
            .map(|(_, &c)| c)
            .collect();
        let new = Palette::new(kept);
        let mut commands = Vec::new();
        let mut map: Option<Vec<u8>> = None;
        if let ColorMode::Indexed { transparent_index } = self.sprite().color_mode {
            let t = transparent_index as usize;
            // La transparente no se borra ni se corre (su índice es parte del modo).
            if t >= lo {
                return false;
            }
            let removed = hi - lo + 1;
            let table: Vec<u8> = (0..256usize)
                .map(|i| {
                    if i < lo {
                        i as u8
                    } else if i > hi {
                        (i - removed).min(255) as u8
                    } else {
                        let c = old.colors()[i];
                        new.nearest(c, transparent_index)
                    }
                })
                .collect();
            for (&id, image) in &self.sprite().images {
                if let PixelBuffer::Indexed(pixels) = image.pixels() {
                    let remapped = pixels.iter().map(|&p| table[p as usize]).collect();
                    let Some(img) = Image::from_buffer(
                        image.width(),
                        image.height(),
                        PixelBuffer::Indexed(remapped),
                    ) else {
                        continue;
                    };
                    commands.push(Command::SetImage {
                        id,
                        image: Some(std::sync::Arc::new(img)),
                    });
                }
            }
            // Los tiles de los tilesets también apuntan a la paleta: igual que las imágenes.
            for (index, tileset) in self.sprite().tilesets.iter().enumerate() {
                let mut changed = tileset.clone();
                let mut any = false;
                for tile in changed.tiles.iter_mut() {
                    if let PixelBuffer::Indexed(pixels) = tile.pixels() {
                        let remapped = pixels.iter().map(|&p| table[p as usize]).collect();
                        if let Some(img) = Image::from_buffer(
                            tile.width(),
                            tile.height(),
                            PixelBuffer::Indexed(remapped),
                        ) {
                            *tile = std::sync::Arc::new(img);
                            any = true;
                        }
                    }
                }
                if any {
                    commands.push(Command::SetTileset {
                        index,
                        tileset: Some(changed),
                    });
                }
            }
            map = Some(table);
        }
        commands.push(Command::SetPalette(new));
        if !self.execute(Command::Batch(commands)) {
            return false;
        }
        let fix = |value: PixelValue, map: &Option<Vec<u8>>| match (value, map) {
            (PixelValue::Index(i), Some(table)) => PixelValue::Index(table[i as usize]),
            (value, _) => value,
        };
        self.primary = self.fit_to_palette(fix(self.primary, &map));
        self.secondary = self.fit_to_palette(fix(self.secondary, &map));
        self.palette_entry = [None; 2];
        self.palette_selection = None;
        true
    }

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
            PixelValue::Rgba(c) => self.palette_entry[self.editing_secondary as usize]
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
    /// de undo. Con la paleta bloqueada no hace nada: el candado la protege. En indexado, los
    /// píxeles guardan su índice: los de los colores que se van quedan transparentes (por eso la
    /// app pide confirmación antes, ver `needs_new_palette_confirmation`).
    pub fn new_palette(&mut self) -> bool {
        if self.palette_locked {
            return false;
        }
        let color = if self.is_mask(self.primary) {
            Rgba::rgb(0, 0, 0)
        } else {
            self.resolve(self.primary)
        };
        self.palette_entry = [None; 2];
        self.apply_palette(Palette::new(vec![color]))
    }

    /// Elige un color como al hacer clic: `value` va al primario (clic, o el gotero con el botón
    /// izquierdo) o, con `secondary`, al secundario (clic derecho), y el editor de color pasa a
    /// editar ese color. `entry`: el cuadrito de la paleta de donde salió (`None` si no salió de
    /// uno, como con el gotero); es la entrada que edita el editor de color con la paleta
    /// desbloqueada.
    pub fn pick_color(&mut self, value: PixelValue, secondary: bool, entry: Option<u8>) {
        if secondary {
            self.secondary = value;
        } else {
            self.primary = value;
        }
        self.editing_secondary = secondary;
        self.palette_entry[secondary as usize] = entry;
    }

    /// Si "New palette" tiene que preguntar antes: en indexado los píxeles apuntan a índices de
    /// la paleta, y los que se van cambian de color.
    pub fn needs_new_palette_confirmation(&self) -> bool {
        matches!(self.sprite().color_mode, ColorMode::Indexed { .. })
    }

    /// El botón del Mask: lo elige como color primario o, si ya lo es, vuelve al de antes. Como
    /// un clic en un cuadrito, el editor de color pasa a editar el primario.
    pub fn toggle_mask(&mut self) {
        self.editing_secondary = false;
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
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, Palette, PixelValue, Rgba, Sprite};

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
    fn append_adds_only_the_missing_colors_in_one_undo_step() {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        assert!(e.apply_palette(colors(3)));
        // 0x020202 ya está; 0x030303 y 0x040404 no.
        let more = Palette::new(vec![
            Rgba::from_hex(0x020202),
            Rgba::from_hex(0x030303),
            Rgba::from_hex(0x040404),
        ]);
        assert!(e.append_palette(&more));
        assert_eq!(e.sprite().palette, colors(5));
        assert!(!e.append_palette(&more), "nothing new");
        e.undo();
        assert_eq!(e.sprite().palette, colors(3));
    }

    #[test]
    fn deleting_indexed_entries_keeps_the_pixels_colors() {
        use cs_core::{Image, ImageId, PixelBuffer};
        use std::sync::Arc;
        let mut sprite = Sprite::new(
            3,
            1,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        sprite.palette = colors(6);
        let image = Image::from_buffer(3, 1, PixelBuffer::Indexed(vec![1, 3, 5])).unwrap();
        let id = ImageId(77);
        sprite.images.insert(id, Arc::new(image.clone()));
        // Un tileset con un tile igual: sus índices también se corren.
        let mut tileset = cs_core::Tileset::new("t", (3, 1), sprite.color_mode);
        tileset.tiles.push(Arc::new(image));
        sprite.tilesets.push(tileset);
        let mut e = Editor::with_sprite(sprite);
        e.primary = PixelValue::Index(5);
        // Se borran la 2 y la 3: la 5 pasa a ser la 3 y la 3 al color más parecido que queda
        // (el 0x040404, ahora la 2).
        assert!(e.delete_palette_entries(2..=3));
        assert_eq!(e.sprite().palette.len(), 4);
        let img = e.sprite().image(id).unwrap();
        let PixelBuffer::Indexed(p) = img.pixels() else {
            panic!("indexed")
        };
        assert_eq!(p, &vec![1, 2, 3]);
        let PixelBuffer::Indexed(t) = e.sprite().tilesets[0].tiles[1].pixels() else {
            panic!("indexed tile")
        };
        assert_eq!(t, &vec![1, 2, 3], "the tile too");
        assert_eq!(e.primary, PixelValue::Index(3));
        e.undo();
        assert_eq!(e.sprite().palette.len(), 6);
        let PixelBuffer::Indexed(t) = e.sprite().tilesets[0].tiles[1].pixels() else {
            panic!("indexed tile")
        };
        assert_eq!(t, &vec![1, 3, 5], "undone in the same step");
        assert!(!e.delete_palette_entries(0..=1), "not the transparent one");
        assert!(!e.delete_palette_entries(0..=5), "not all of them");
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
        e.palette_entry = [Some(2), None];
        assert_eq!(e.palette_entry_of(e.primary), None, "locked");
        assert!(!e.set_palette_color(2, Rgba::rgb(9, 9, 9), 1));
        assert_eq!(e.sprite().palette, colors(4));
    }

    #[test]
    fn unlocked_the_picked_entry_is_edited_in_one_undo_step_per_gesture() {
        let mut e = rgba_editor();
        e.palette_locked = false;
        e.primary = e.palette_value(2);
        e.palette_entry = [Some(2), None];
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
        e.palette_locked = false;
        e.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
        assert!(e.new_palette());
        assert_eq!(e.sprite().palette.colors(), &[Rgba::rgb(1, 2, 3)]);
        e.undo();
        assert_eq!(e.sprite().palette, colors(4));
        e.primary = e.mask_value();
        assert!(e.new_palette());
        assert_eq!(e.sprite().palette.colors(), &[Rgba::rgb(0, 0, 0)]);
    }

    #[test]
    fn a_locked_palette_is_not_replaced_by_a_new_one() {
        let mut e = rgba_editor();
        assert!(e.palette_locked);
        e.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
        assert!(!e.new_palette());
        assert_eq!(e.sprite().palette, colors(4));
    }

    fn indexed_editor() -> Editor {
        let mut sprite = Sprite::new(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        sprite.palette = colors(8);
        Editor::with_sprite(sprite)
    }

    /// Lo que pasa hoy con los píxeles cuyo índice queda fuera de la paleta nueva: guardan su
    /// índice (no se tocan) y se ven transparentes mientras no haya color en ese índice.
    /// Deshacer vuelve la paleta y con ella su color.
    #[test]
    fn indexed_pixels_outside_a_new_palette_keep_their_index_and_turn_transparent() {
        let mut e = indexed_editor();
        e.palette_locked = false;
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, PixelValue::Index(5)), (2, 2, PixelValue::Index(1))],
        });
        let five = e.sprite().palette.colors()[5];
        assert_eq!(cs_render::composite_pixel(e.sprite(), 0, 1, 1), five);
        e.primary = PixelValue::Index(3);
        assert!(e.new_palette());
        assert_eq!(e.sprite().palette.colors(), &[colors(8).colors()[3]]);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), PixelValue::Index(5));
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), PixelValue::Index(1));
        assert_eq!(
            cs_render::composite_pixel(e.sprite(), 0, 1, 1),
            Rgba::TRANSPARENT
        );
        assert_eq!(e.primary, PixelValue::Index(0), "the primary is fitted");
        e.undo();
        assert_eq!(e.sprite().palette, colors(8));
        assert_eq!(cs_render::composite_pixel(e.sprite(), 0, 1, 1), five);
    }

    #[test]
    fn picking_a_color_switches_the_editor_to_it_and_remembers_its_swatch() {
        let mut e = rgba_editor();
        e.palette_locked = false;
        // Elegida la mitad de abajo, un clic en #2 pasa a editar el primario y su entrada.
        e.editing_secondary = true;
        e.pick_color(e.palette_value(2), false, Some(2));
        assert!(!e.editing_secondary);
        assert_eq!(e.edited_color(), e.palette_value(2));
        assert_eq!(e.palette_entry_of(e.edited_color()), Some(2));
        assert!(e.set_palette_color(2, Rgba::rgb(9, 9, 9), 1));
        e.end_group_of(1);
        // Clic derecho en #3: el secundario, y el editor pasa a él.
        e.pick_color(e.palette_value(3), true, Some(3));
        assert!(e.editing_secondary);
        assert_eq!(e.edited_color(), e.palette_value(3));
        assert_eq!(e.palette_entry_of(e.edited_color()), Some(3));
        // Cada color recuerda su cuadrito: volver al primario edita otra vez el #2.
        e.primary = e.palette_value(2);
        e.editing_secondary = false;
        assert_eq!(e.palette_entry_of(e.edited_color()), Some(2));
        // Intercambiar los colores intercambia sus cuadritos.
        e.swap_colors();
        assert_eq!(e.palette_entry_of(e.edited_color()), Some(3));
        // Sin cuadrito (el gotero), no hay entrada.
        e.pick_color(e.palette_value(1), false, None);
        assert_eq!(e.palette_entry_of(e.edited_color()), None);
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
