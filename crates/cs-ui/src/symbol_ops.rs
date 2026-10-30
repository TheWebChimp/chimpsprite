//! Símbolos de la biblioteca desde el editor: F8 (Convert to Symbol) y Move to Library.

use cs_commands::symbol::{self, ConvertError, SymbolSource, SymbolTiming};
use cs_core::ArtboardId;

use crate::editor::Editor;

impl Editor {
    /// Lo que F8 convertiría ahora: con marquesina, sus píxeles en `range_layers()`; si no, esas
    /// capas.
    pub fn symbol_source(&self) -> SymbolSource {
        let layers = self.range_layers();
        match self.selection.clone() {
            Some(selection) => SymbolSource::Pixels { layers, selection },
            None => SymbolSource::Layers(layers),
        }
    }

    /// El nombre que propone el diálogo: el de la capa si es una sola y no es la marquesina; si
    /// no, el primer "Symbol N" libre.
    pub fn suggested_symbol_name(&self) -> String {
        if let SymbolSource::Layers(layers) = self.symbol_source()
            && let [only] = layers.as_slice()
            && let Some(layer) = self.sprite().layer(*only)
        {
            return layer.name.clone();
        }
        let taken = |name: &str| self.document.artboards.iter().any(|a| a.name == name);
        (1..)
            .map(|n| format!("Symbol {n}"))
            .find(|name| !taken(name))
            .expect("siempre hay un nombre libre")
    }

    /// Revisa sin aplicar (para deshabilitar menús).
    pub fn can_convert_to_symbol(&self) -> Result<(), ConvertError> {
        symbol::convert_to_symbol(
            &self.document,
            self.active,
            &self.symbol_source(),
            "",
            SymbolTiming::Graphic,
        )
        .map(|_| ())
    }

    /// Convierte, selecciona la instancia nueva, borra la marquesina y recuerda el símbolo como
    /// `library_selected`.
    pub fn convert_to_symbol(
        &mut self,
        name: &str,
        timing: SymbolTiming,
    ) -> Result<ArtboardId, ConvertError> {
        let source = self.symbol_source();
        let c = symbol::convert_to_symbol(&self.document, self.active, &source, name, timing)?;
        if !self.execute_doc(c.command) {
            return Err(ConvertError::Empty);
        }
        self.select_layer(c.instance);
        self.deselect();
        self.library_selected = Some(c.symbol);
        Ok(c.symbol)
    }

    /// Manda el artboard activo a la biblioteca y activa otro del lienzo. `false` si no se puede.
    pub fn move_active_to_library(&mut self) -> bool {
        let Some(cmd) = symbol::move_to_library(&self.document, self.active) else {
            return false;
        };
        if !self.execute_doc(cmd) {
            return false;
        }
        // Fuera del modo símbolo, el activo siempre es uno del lienzo.
        let next = self.document.canvas_artboards().next_back().map(|a| a.id);
        if let Some(next) = next {
            self.activate(next);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, LayerKind, Mask, PixelValue, Rgba, Sprite};

    use super::*;

    #[test]
    fn f8_converts_the_current_layer_or_the_marquee() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(2, 2, red), (5, 5, red)],
        });
        assert_eq!(e.suggested_symbol_name(), "Layer 1");
        e.selection = Some(Mask::from_rect(8, 8, (0, 0), (3, 3)));
        assert_eq!(e.suggested_symbol_name(), "Symbol 1");
        assert_eq!(e.can_convert_to_symbol(), Ok(()));
        let symbol = e
            .convert_to_symbol("Symbol 1", SymbolTiming::Graphic)
            .unwrap();
        assert!(e.selection.is_none());
        assert_eq!(e.library_selected, Some(symbol));
        assert_eq!(e.sprite().layer(e.layer).unwrap().kind, LayerKind::Instance);
        // Solo (2,2) estaba en la marquesina: el símbolo mide 1×1.
        assert_eq!(e.document.artboard(symbol).unwrap().sprite.width, 1);
        // De vuelta en la capa 1, con marquesina: el siguiente nombre libre.
        e.select_layer(LayerId(1));
        assert_eq!(e.suggested_symbol_name(), "Layer 1");
        e.selection = Some(Mask::from_rect(8, 8, (4, 4), (7, 7)));
        assert_eq!(e.suggested_symbol_name(), "Symbol 2");
        e.selection = None;
        e.undo();
        assert_eq!(e.document.artboards.len(), 1);
    }

    #[test]
    fn move_to_library_needs_another_canvas_artboard() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        assert!(!e.move_active_to_library());
        e.add_artboard(4, 4);
        let moved = e.active;
        assert!(e.move_active_to_library());
        assert!(e.document.artboard(moved).unwrap().in_library);
        assert_ne!(e.active, moved);
    }
}
