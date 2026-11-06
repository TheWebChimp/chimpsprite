//! Símbolos de la biblioteca desde el editor: F8 (Convert to Symbol) y Move to Library.

use cs_commands::symbol::{self, ConvertError, SymbolSource, SymbolTiming};
use std::borrow::Cow;

use cs_core::{ArtboardId, Document, LayerId, ObjectId, source_frame};

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

    /// El artboard que se ve bajo `world` (mundo): en el modo símbolo o en el workspace de una
    /// fuente solo el activo; si no, el de más arriba del lienzo.
    pub fn shown_artboard_at(&self, world: (f32, f32)) -> Option<ArtboardId> {
        if self.single_artboard() {
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

    /// Entra a editar `id` (un símbolo de la biblioteca) aislado: lo apila y lo activa. Guarda el
    /// artboard activo de antes para volver.
    pub fn enter_symbol(&mut self, id: ArtboardId) -> bool {
        if !self.document.artboard(id).is_some_and(|a| a.is_symbol()) {
            return false;
        }
        // Si ya está en la pila, se vuelve a ese nivel.
        if let Some(i) = self.symbol_stack.iter().position(|s| s.id == id) {
            self.exit_symbol_to(i + 1);
            return true;
        }
        if self.symbol_stack.is_empty() {
            self.symbol_return = Some(self.active);
        }
        self.symbol_stack.push(SymbolLevel { id, place: None });
        self.activate(id);
        true
    }

    /// Entra a editar en su lugar el símbolo del objeto `object` de la capa `layer` del activo
    /// (Edit in Place): el símbolo se ve donde está la instancia, sobre el resto atenuado, y
    /// empieza en el frame que la instancia muestra ahora. Si el objeto está volteado (o su
    /// símbolo ya está en la pila) entra aislado. `false` si la fuente no es un símbolo de la
    /// biblioteca.
    pub fn enter_symbol_in_place(&mut self, layer: LayerId, object: ObjectId) -> bool {
        let (parent, parent_frame) = (self.active, self.frame);
        let sprite = self.sprite();
        let Some(key) = sprite
            .layer(layer)
            .and_then(|l| l.object(object))
            .and_then(|o| o.track.key_at(parent_frame))
        else {
            return false;
        };
        let Some(symbol) = key
            .source
            .and_then(|id| self.document.artboard(id))
            .filter(|a| a.is_symbol())
        else {
            return false;
        };
        let id = symbol.id;
        if key.flip_h || key.flip_v || self.symbol_stack.iter().any(|s| s.id == id) {
            return self.enter_symbol(id);
        }
        let start = source_frame(key, sprite, parent_frame, &symbol.sprite);
        let home = symbol.position;
        if self.symbol_stack.is_empty() {
            self.symbol_return = Some(parent);
        }
        self.symbol_stack.push(SymbolLevel {
            id,
            place: Some(InPlace {
                parent,
                parent_frame,
                layer,
                object,
                home,
            }),
        });
        self.sync_in_place();
        self.activate(id);
        self.frame = start.min(self.sprite().frames.len().saturating_sub(1));
        true
    }

    /// Vuelve al nivel `depth` de la pila (0 = el documento). Activa lo que había ahí.
    pub fn exit_symbol_to(&mut self, depth: usize) {
        if depth >= self.symbol_stack.len() {
            return;
        }
        self.truncate_symbols(depth);
        // `ensure_valid` activa el último de la pila o, sin pila, `symbol_return` (o el primero
        // del lienzo).
        self.ensure_valid();
    }

    /// Corta la pila en `depth`: los que se editaban en su lugar vuelven a su posición de la
    /// biblioteca (del de adentro al de afuera).
    pub(crate) fn truncate_symbols(&mut self, depth: usize) {
        while self.symbol_stack.len() > depth {
            let level = self.symbol_stack.pop().expect("len > depth");
            // Si salió al lienzo (Place on Canvas), su posición ya es la del lienzo.
            if let Some(place) = level.place
                && let Some(a) = self.document.artboard_mut(level.id)
                && a.is_symbol()
            {
                a.position = place.home;
            }
        }
    }

    /// Pone cada símbolo que se edita en su lugar donde está su instancia: la posición del padre
    /// (ya puesta, si también está en su lugar) más la de la key del objeto en el frame del
    /// padre. Si la instancia ya no está (un undo), la pila se corta antes de ese nivel.
    pub(crate) fn sync_in_place(&mut self) {
        for i in 0..self.symbol_stack.len() {
            let level = self.symbol_stack[i];
            let Some(place) = level.place else {
                continue;
            };
            match in_place_position(&self.document, level.id, &place) {
                Some(position) => {
                    if let Some(a) = self.document.artboard_mut(level.id) {
                        a.position = position;
                    }
                }
                None => {
                    self.truncate_symbols(i);
                    return;
                }
            }
        }
    }

    /// El documento para guardar: con los símbolos que se editan en su lugar en su posición de
    /// la biblioteca (la de edición no se guarda).
    pub fn document_for_save(&self) -> Cow<'_, Document> {
        if !self.symbol_stack.iter().any(|l| l.place.is_some()) {
            return Cow::Borrowed(&self.document);
        }
        let mut doc = self.document.clone();
        for level in self.symbol_stack.iter().rev() {
            if let Some(place) = level.place
                && let Some(a) = doc.artboard_mut(level.id)
            {
                a.position = place.home;
            }
        }
        Cow::Owned(doc)
    }

    /// La migaja de pan: "Scene" y los nombres de la pila.
    pub fn symbol_breadcrumb(&self) -> Vec<String> {
        let names = self
            .symbol_stack
            .iter()
            .filter_map(|l| self.document.artboard(l.id))
            .map(|a| a.name.clone());
        std::iter::once("Scene".to_string()).chain(names).collect()
    }

    /// ¿Se está editando un símbolo?
    pub fn in_symbol_mode(&self) -> bool {
        !self.symbol_stack.is_empty()
    }

    /// ¿El símbolo que se edita está en su lugar (Edit in Place)?
    pub fn editing_in_place(&self) -> bool {
        self.symbol_stack.last().is_some_and(|l| l.place.is_some())
    }

    /// Lo que abarca la escena alrededor del símbolo que se edita en su lugar (para "Fit"): todo
    /// el lienzo si la cadena de niveles en su lugar empieza en el documento; si no, el símbolo
    /// aislado de abajo. `None` si no se edita en su lugar.
    pub fn in_place_bounds(&self) -> Option<((i32, i32), (i32, i32))> {
        if !self.editing_in_place() {
            return None;
        }
        match self.symbol_stack.iter().rposition(|l| l.place.is_none()) {
            None => self.document.bounds(),
            Some(i) => self
                .document
                .artboard(self.symbol_stack[i].id)
                .map(|a| a.rect()),
        }
    }

    /// Lo que se ve atenuado alrededor del símbolo que se edita en su lugar, a partir de
    /// `document` (el que muestra el lienzo): lo que se veía donde empieza la cadena de niveles en
    /// su lugar (los artboards del lienzo, o el símbolo aislado de abajo), en una copia donde la
    /// instancia que se edita no está. Si está anidada, los símbolos de en medio se copian sin
    /// ella (las demás instancias de esos símbolos no cambian). `None` si no se edita en su lugar.
    pub fn in_place_context(&self, document: &Document) -> Option<InPlaceContext> {
        let stack = &self.symbol_stack;
        if !self.editing_in_place() {
            return None;
        }
        // El primero de la cadena de niveles en su lugar que llega hasta arriba.
        let start = stack
            .iter()
            .rposition(|l| l.place.is_none())
            .map_or(0, |i| i + 1);
        let mut doc = document.clone();
        // La copia del símbolo del nivel de abajo (sin lo que se edita); el de arriba se quita.
        let mut copy: Option<ArtboardId> = None;
        for i in (start..stack.len()).rev() {
            let level = stack[i];
            let place = level.place.expect("in place");
            let edit = |a: &mut cs_core::Artboard| {
                if let Some(layer) = a.sprite.layer_mut(place.layer) {
                    match copy {
                        None => layer.objects.retain(|o| o.id != place.object),
                        Some(c) => {
                            if let Some(o) = layer.objects.iter_mut().find(|o| o.id == place.object)
                            {
                                for k in &mut o.track.keys {
                                    if k.source == Some(level.id) {
                                        k.source = Some(c);
                                    }
                                }
                            }
                        }
                    }
                }
            };
            if i > start {
                // El padre es otro símbolo de la cadena: se copia con el cambio.
                let mut parent = doc.artboard(place.parent)?.clone();
                parent.id = doc.next_artboard_id();
                edit(&mut parent);
                copy = Some(parent.id);
                doc.artboards.push(parent);
            } else {
                edit(doc.artboard_mut(place.parent)?);
            }
        }
        let artboards = match start {
            0 => doc.canvas_artboards().map(|a| a.id).collect(),
            _ => vec![stack[start - 1].id],
        };
        Some(InPlaceContext {
            document: doc,
            artboards,
        })
    }
}

/// El contexto de Edit in Place (ver `Editor::in_place_context`).
pub struct InPlaceContext {
    /// La copia del documento sin la instancia que se edita.
    pub document: Document,
    /// Los artboards que se dibujan atenuados, en su orden.
    pub artboards: Vec<ArtboardId>,
}

/// Un nivel del modo símbolo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolLevel {
    pub id: ArtboardId,
    /// `Some`: en su lugar, dentro de una instancia; `None`: aislado.
    pub place: Option<InPlace>,
}

/// Dónde se edita un símbolo en su lugar: la instancia (un objeto) de su padre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InPlace {
    /// El artboard donde está la instancia: uno del lienzo o el nivel anterior de la pila.
    pub parent: ArtboardId,
    /// El frame del padre al entrar: el contexto se ve congelado en él.
    pub parent_frame: usize,
    pub layer: LayerId,
    pub object: ObjectId,
    /// La posición del símbolo en la biblioteca antes de entrar (vuelve al salir).
    pub home: (i32, i32),
}

/// Dónde se ve `id` editado en su lugar `place`; `None` si la instancia ya no lo muestra (sin el
/// objeto, sin key, con otra fuente o volteada).
fn in_place_position(doc: &Document, id: ArtboardId, place: &InPlace) -> Option<(i32, i32)> {
    let parent = doc.artboard(place.parent)?;
    let key = parent
        .sprite
        .layer(place.layer)?
        .object(place.object)?
        .track
        .key_at(place.parent_frame)?;
    (key.source == Some(id) && !key.flip_h && !key.flip_v).then(|| {
        (
            parent.position.0 + key.position.0,
            parent.position.1 + key.position.1,
        )
    })
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
            (inner, vec!["Scene".to_string(), "Inner".into()])
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
