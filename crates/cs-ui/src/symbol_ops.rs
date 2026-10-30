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

    /// Revisa sin armar el símbolo (para deshabilitar menús).
    pub fn can_convert_to_symbol(&self) -> Result<(), ConvertError> {
        symbol::check_convert(&self.document, self.active, &self.symbol_source())
    }

    /// Convierte, selecciona la capa de objetos nueva y su objeto, borra la marquesina y recuerda el símbolo como
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
        self.select_layer(c.layer);
        // El objeto de la capa nueva queda seleccionado (como al soltar un símbolo).
        let object = self
            .sprite()
            .layer(c.layer)
            .and_then(|l| l.objects.first())
            .map(|o| o.id);
        if let Some(object) = object {
            self.select_object(c.layer, object, false);
        }
        self.deselect();
        self.library_selected = Some(c.symbol);
        Ok(c.symbol)
    }

    /// El artboard que se ve bajo `world` (mundo): en el modo símbolo solo el símbolo; si no, el
    /// de más arriba del lienzo.
    pub fn shown_artboard_at(&self, world: (f32, f32)) -> Option<ArtboardId> {
        if self.in_symbol_mode() {
            self.artboard().contains(world).then_some(self.active)
        } else {
            self.document.artboard_at(world)
        }
    }

    /// ¿Se puede mandar el activo a la biblioteca? No en el modo símbolo (el activo ya es un
    /// símbolo) ni si es el último del lienzo.
    pub fn can_move_active_to_library(&self) -> bool {
        !self.in_symbol_mode() && symbol::move_to_library(&self.document, self.active).is_some()
    }

    /// Manda el artboard activo a la biblioteca y activa otro del lienzo. `false` si no se puede.
    pub fn move_active_to_library(&mut self) -> bool {
        if !self.can_move_active_to_library() {
            return false;
        }
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

    /// Entra a editar `id` (un símbolo de la biblioteca): lo apila y lo activa. Guarda el artboard
    /// activo de antes para volver.
    pub fn enter_symbol(&mut self, id: ArtboardId) -> bool {
        if !self.document.artboard(id).is_some_and(|a| a.in_library) {
            return false;
        }
        // Si ya está en la pila, se vuelve a ese nivel.
        if let Some(i) = self.symbol_stack.iter().position(|&s| s == id) {
            self.exit_symbol_to(i + 1);
            return true;
        }
        if self.symbol_stack.is_empty() {
            self.symbol_return = Some(self.active);
        }
        self.symbol_stack.push(id);
        self.activate(id);
        true
    }

    /// Vuelve al nivel `depth` de la pila (0 = el documento). Activa lo que había ahí.
    pub fn exit_symbol_to(&mut self, depth: usize) {
        if depth >= self.symbol_stack.len() {
            return;
        }
        self.symbol_stack.truncate(depth);
        // `ensure_valid` activa el último de la pila o, sin pila, `symbol_return` (o el primero
        // del lienzo).
        self.ensure_valid();
    }

    /// La migaja de pan: "Document" y los nombres de la pila.
    pub fn symbol_breadcrumb(&self) -> Vec<String> {
        let names = self
            .symbol_stack
            .iter()
            .filter_map(|&id| self.document.artboard(id))
            .map(|a| a.name.clone());
        std::iter::once("Document".to_string())
            .chain(names)
            .collect()
    }

    /// ¿Se está editando un símbolo?
    pub fn in_symbol_mode(&self) -> bool {
        !self.symbol_stack.is_empty()
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
        assert_eq!(e.sprite().layer(e.layer).unwrap().kind, LayerKind::Objects);
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

    #[test]
    fn the_symbol_being_edited_cannot_be_deleted_as_an_artboard() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        let symbol = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
        e.add_artboard(4, 4);
        assert!(e.can_remove_active_artboard());
        assert!(e.enter_symbol(symbol));
        assert!(!e.can_remove_active_artboard());
        assert!(!e.can_move_active_to_library());
        e.remove_active_artboard();
        assert!(e.document.artboard(symbol).is_some(), "still there");
        assert_eq!(e.active, symbol);
        assert!(!e.move_active_to_library());
    }

    #[test]
    fn entering_and_leaving_symbols() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        let inner = e.convert_to_symbol("Inner", SymbolTiming::Graphic).unwrap();
        let host = ArtboardId(1);
        assert!(e.enter_symbol(inner));
        assert_eq!(
            (e.active, e.symbol_breadcrumb()),
            (inner, vec!["Document".to_string(), "Inner".into()])
        );
        assert!(!e.enter_symbol(host), "only library artboards");
        e.exit_symbol_to(0);
        assert!(!e.in_symbol_mode());
        assert_eq!(e.active, host);
    }

    #[test]
    fn undoing_the_symbol_away_trims_the_stack() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        let s = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
        e.enter_symbol(s);
        e.undo(); // deshace la conversión: el símbolo ya no existe
        assert!(!e.in_symbol_mode());
        assert_eq!(e.active, ArtboardId(1));
    }

    #[test]
    fn activating_a_canvas_artboard_leaves_symbol_mode() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        let s = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
        assert!(e.enter_symbol(s));
        // Un clic en el lienzo no activa el host (no se ve).
        e.pointer_press((1.5, 1.5), false, false, 0.0);
        e.pointer_release();
        assert_eq!(e.active, s);
        // Activarlo desde otro lado (el panel de Artboards) sí sale del modo símbolo.
        e.activate(ArtboardId(1));
        assert!(!e.in_symbol_mode());
    }
}
