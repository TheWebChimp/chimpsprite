//! Instancias de artboards (símbolos) desde el editor y el panel.

use cs_commands::{Command, DocCommand, ops};
use cs_core::{ArtboardId, InstanceKey, InstanceTrack, LayerId, LayerKind, ObjectId};

use crate::editor::Editor;

/// Artboards que se pueden instanciar, con su nombre.
pub type Candidates = Vec<(ArtboardId, String)>;

impl Editor {
    /// Artboards que se pueden instanciar en el activo: no crean ciclos y el activo es RGBA.
    /// Primero los símbolos de la biblioteca y después los del lienzo.
    pub fn instance_candidates(&self) -> Candidates {
        let (mut library, canvas) = self.instance_candidate_groups();
        library.extend(canvas);
        library
    }

    /// Los candidatos de `instance_candidates` separados en `(biblioteca, lienzo)`, cada grupo
    /// ordenado por nombre.
    pub fn instance_candidate_groups(&self) -> (Candidates, Candidates) {
        let (mut library, mut canvas) = (Vec::new(), Vec::new());
        if !self.sprite().color_mode.stores_rgba() {
            return (library, canvas);
        }
        for a in &self.document.artboards {
            if self.document.would_cycle(self.active, a.id) {
                continue;
            }
            let group = if a.in_library {
                &mut library
            } else {
                &mut canvas
            };
            group.push((a.id, a.name.clone()));
        }
        // Sin distinguir mayúsculas, como las filas de la Library.
        library.sort_by_cached_key(|c| c.1.to_lowercase());
        canvas.sort_by_cached_key(|c| c.1.to_lowercase());
        (library, canvas)
    }

    /// Objeto nuevo de `source` en el artboard activo, centrado, con key en el frame actual: en
    /// la capa actual si es de objetos (y no está bloqueada); si no, en una capa de objetos nueva
    /// sobre ella. El objeto queda seleccionado.
    pub fn add_instance_layer(&mut self, source: ArtboardId) {
        let Some(src) = self.document.artboard(source) else {
            return;
        };
        let sprite = self.sprite();
        let position = (
            (sprite.width as i32 - src.sprite.width as i32) / 2,
            (sprite.height as i32 - src.sprite.height as i32) / 2,
        );
        let name = src.name.clone();
        self.place_object(self.active, self.frame, source, &name, position);
    }

    /// Suelta el símbolo `source` en el punto `world` del lienzo: en el artboard de ese punto (en
    /// modo símbolo, solo el que se edita cuenta) agrega un objeto, como `add_instance_layer`.
    pub fn drop_symbol(
        &mut self,
        source: ArtboardId,
        world: (i32, i32),
    ) -> Result<(), &'static str> {
        let target = self
            .shown_artboard_at((world.0 as f32, world.1 as f32))
            .ok_or("Drop it on an artboard")?;
        let board = self
            .document
            .artboard(target)
            .ok_or("Drop it on an artboard")?;
        let src = self
            .document
            .artboard(source)
            .ok_or("That symbol is gone")?;
        if !board.sprite.color_mode.stores_rgba() {
            return Err("Symbols need an RGBA sprite");
        }
        if self.document.would_cycle(target, source) {
            return Err("That would put the artboard inside itself");
        }
        let local = (world.0 - board.position.0, world.1 - board.position.1);
        // El frame que se ve en ese artboard (puede no ser el activo y tener menos frames).
        let last = board.sprite.frames.len().saturating_sub(1);
        let frame = self.frame_of(target).min(last);
        let name = src.name.clone();
        self.place_object(target, frame, source, &name, local);
        Ok(())
    }

    /// Agrega un objeto de `source` en `target`: a la capa actual si `target` es el activo y esa
    /// capa es de objetos sin candado; si no, en una capa de objetos nueva (sobre la actual, si
    /// `target` es el activo). Activa `target`, la capa y deja seleccionado el objeto.
    fn place_object(
        &mut self,
        target: ArtboardId,
        frame: usize,
        source: ArtboardId,
        name: &str,
        position: (i32, i32),
    ) {
        let Some(board) = self.document.artboard(target) else {
            return;
        };
        let sprite = &board.sprite;
        let on_active = target == self.active;
        let into = on_active
            .then_some(self.layer)
            .filter(|&l| {
                sprite
                    .layer(l)
                    .is_some_and(|l| l.kind == LayerKind::Objects && !l.locked)
            })
            .and_then(|l| {
                ops::add_object(sprite, l, frame, source, name, position).map(|a| (l, a))
            });
        let (cmd, layer, object) = match into {
            Some((layer, (cmd, object))) => (cmd, layer, object),
            None => {
                let above = on_active.then_some(self.layer);
                let cmd = ops::new_object_layer(sprite, above, frame, source, name, position);
                let (layer, object) = match &cmd {
                    Command::AddLayer { layer, .. } => (layer.id, layer.objects[0].id),
                    _ => unreachable!("new_object_layer devuelve AddLayer"),
                };
                (cmd, layer, object)
            }
        };
        if self.execute_doc(DocCommand::OnArtboard { id: target, cmd }) {
            self.activate(target);
            self.select_object(layer, object, false);
        }
    }

    /// ¿La capa actual es de objetos? (El panel Object se muestra solo ahí.)
    pub fn on_object_layer(&self) -> bool {
        self.sprite()
            .layer(self.layer)
            .is_some_and(|l| l.kind == LayerKind::Objects)
    }

    /// El objeto que se edita en la capa actual: el primero de los seleccionados o, sin
    /// selección, el primero de la capa.
    pub fn current_object(&self) -> Option<ObjectId> {
        self.object_of(self.layer)
    }

    /// El objeto que se edita en `layer` (el panel, el timeline y sus keys): en la capa actual,
    /// el primero de los seleccionados; si no, el primero de la capa. Es el único lugar que
    /// decide ese "primero".
    pub(crate) fn object_of(&self, layer: LayerId) -> Option<ObjectId> {
        let l = self.sprite().layer(layer)?;
        let selected = (layer == self.layer)
            .then(|| {
                self.selected_objects()
                    .iter()
                    .find(|&&o| l.object(o).is_some())
            })
            .flatten();
        selected
            .copied()
            .or_else(|| l.objects.first().map(|o| o.id))
    }

    /// El track del objeto que se edita en `layer` (ver `object_of`).
    pub(crate) fn object_track(&self, layer: LayerId) -> Option<&InstanceTrack> {
        let object = self.object_of(layer)?;
        Some(&self.sprite().layer(layer)?.object(object)?.track)
    }

    /// Key vigente del objeto actual en el frame actual (si la capa es de objetos).
    pub fn instance_key(&self) -> Option<InstanceKey> {
        let object = self.current_object()?;
        let layer = self.sprite().layer(self.layer)?;
        layer.object(object)?.track.key_at(self.frame).copied()
    }

    pub fn is_instance_keyframe(&self, layer: LayerId, frame: usize) -> bool {
        self.object_track(layer)
            .is_some_and(|t| t.keys.iter().any(|k| k.frame == frame))
    }

    /// Cambia la key vigente de los objetos que se editan (ver `edited_objects`); donde el frame
    /// actual no es keyframe, crea uno. Un solo paso de undo.
    pub fn update_instance_key(&mut self, f: impl FnMut(&mut InstanceKey)) {
        self.update_instance_key_grouped(None, f);
    }

    /// `update_instance_key` como parte del gesto `group` (un solo paso de undo por gesto).
    pub fn update_instance_key_grouped(
        &mut self,
        group: Option<u64>,
        f: impl FnMut(&mut InstanceKey),
    ) {
        let objects = self.edited_objects();
        if let Some(cmd) =
            ops::update_objects_key(self.sprite(), self.layer, &objects, self.frame, f)
        {
            match group {
                Some(g) => self.execute_grouped(cmd, g),
                None => self.execute(cmd),
            };
        }
    }

    /// Los objetos que edita el panel Object: los seleccionados de la capa actual o, sin
    /// selección, el objeto actual.
    pub fn edited_objects(&self) -> Vec<ObjectId> {
        let Some(layer) = self.sprite().layer(self.layer) else {
            return Vec::new();
        };
        let selected: Vec<ObjectId> = self
            .selected_objects()
            .iter()
            .copied()
            .filter(|&o| layer.object(o).is_some())
            .collect();
        if selected.is_empty() {
            self.current_object().into_iter().collect()
        } else {
            selected
        }
    }

    /// El nombre del objeto actual.
    pub fn current_object_name(&self) -> Option<&str> {
        let object = self.current_object()?;
        Some(&self.sprite().layer(self.layer)?.object(object)?.name)
    }

    /// Renombra el objeto actual, como parte del gesto `group` (un paso por edición del campo).
    pub fn rename_current_object(&mut self, name: &str, group: Option<u64>) {
        let Some(object) = self.current_object() else {
            return;
        };
        if let Some(cmd) = ops::rename_object(self.sprite(), self.layer, object, name) {
            match group {
                Some(g) => self.execute_grouped(cmd, g),
                None => self.execute(cmd),
            };
        }
    }

    /// Keyframe en `frame` copiando el vigente, en el objeto que se edita en `layer`.
    pub fn insert_instance_key(&mut self, layer: LayerId, frame: usize) {
        if let Some(object) = self.object_of(layer) {
            self.insert_object_key(layer, object, frame);
        }
    }

    pub fn remove_instance_key(&mut self, layer: LayerId, frame: usize) {
        if let Some(object) = self.object_of(layer) {
            self.remove_object_key(layer, object, frame);
        }
    }

    /// Keyframe de `object` en `frame` copiando su key vigente (la sub-fila del timeline).
    pub fn insert_object_key(&mut self, layer: LayerId, object: ObjectId, frame: usize) {
        if let Some(cmd) = ops::update_object_key(self.sprite(), layer, object, frame, |_| {}) {
            self.execute(cmd);
        }
    }

    /// Quita la key de `object` que empieza en `frame` (la sub-fila del timeline).
    pub fn remove_object_key(&mut self, layer: LayerId, object: ObjectId, frame: usize) {
        if let Some(cmd) = ops::remove_object_key(self.sprite(), layer, object, frame) {
            self.execute(cmd);
        }
    }

    /// "Entrar al símbolo": con la fuente de la instancia actual en la biblioteca, entra al modo
    /// símbolo; si está en el lienzo, la activa.
    pub fn enter_instance(&mut self) -> bool {
        let Some(source) = self.instance_key().and_then(|k| k.source) else {
            return false;
        };
        match self.document.artboard(source) {
            None => false,
            Some(a) if a.in_library => self.enter_symbol(source),
            Some(_) => {
                self.activate(source);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, LayerKind, Sprite};

    use super::*;

    fn editor() -> Editor {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.insert_empty_frame();
        e.set_frame(0);
        e.add_artboard(4, 4);
        e.activate(ArtboardId(1));
        e
    }

    #[test]
    fn dropping_a_symbol_places_an_instance_where_it_lands() {
        use cs_core::{PixelValue, Rgba};
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        let symbol = e
            .convert_to_symbol("S", cs_commands::symbol::SymbolTiming::Graphic)
            .unwrap();
        // F8 deja activa la capa de objetos: el objeto soltado va ahí, arriba del convertido.
        let layers = e.sprite().layers.len();
        e.drop_symbol(symbol, (5, 6)).unwrap();
        assert_eq!(e.sprite().layers.len(), layers);
        let layer = e.sprite().layer(e.layer).unwrap();
        assert_eq!(layer.objects[1].track.keys[0].position, (5, 6));
        assert!(
            e.drop_symbol(symbol, (500, 500)).is_err(),
            "outside every artboard"
        );
    }

    #[test]
    fn dropping_on_another_artboard_uses_its_frame() {
        // El activo (1) tiene 2 frames y está en el 1; el 2 (en x = 32) tiene uno solo.
        let mut e = editor();
        e.set_frame(1);
        let symbol = {
            e.add_artboard(2, 2);
            let id = e.active;
            e.move_active_to_library();
            e.activate(ArtboardId(1));
            e.set_frame(1);
            id
        };
        let at = e.document.artboard(ArtboardId(2)).unwrap().position;
        e.drop_symbol(symbol, (at.0 + 1, at.1 + 1)).unwrap();
        assert_eq!(e.active, ArtboardId(2));
        let layer = e.sprite().layer(e.layer).unwrap();
        let keys = &layer.objects[0].track.keys;
        assert_eq!((keys[0].frame, keys[0].position), (0, (1, 1)));
    }

    #[test]
    fn candidates_are_sorted_ignoring_case() {
        let mut e = editor();
        e.document.artboard_mut(ArtboardId(2)).unwrap().name = "banana".into();
        e.add_artboard(4, 4);
        e.rename_active_artboard("Apple".into());
        e.activate(ArtboardId(1));
        let names: Vec<String> = e
            .instance_candidates()
            .into_iter()
            .filter(|(c, _)| *c != ArtboardId(1))
            .map(|(_, n)| n)
            .collect();
        assert_eq!(names, ["Apple", "banana"]);
    }

    #[test]
    fn place_edit_key_and_enter_instances() {
        let mut e = editor();
        assert_eq!(
            e.instance_candidates()
                .iter()
                .map(|c| c.0)
                .collect::<Vec<_>>(),
            vec![ArtboardId(2)]
        );
        e.add_instance_layer(ArtboardId(2));
        let layer = e.sprite().layer(e.layer).unwrap().clone();
        assert_eq!(layer.kind, LayerKind::Objects);
        assert_eq!(e.current_object(), Some(layer.objects[0].id));
        assert_eq!(e.instance_key().unwrap().position, (6, 6), "centered");
        // El artboard 2 ya no puede instanciar al 1.
        e.activate(ArtboardId(2));
        assert!(e.instance_candidates().is_empty());
        e.activate(ArtboardId(1));
        e.select_layer(layer.id);
        e.set_frame(1);
        e.update_instance_key(|k| k.flip_h = true);
        assert!(e.is_instance_keyframe(layer.id, 1));
        e.set_frame(0);
        assert!(!e.instance_key().unwrap().flip_h);
        e.remove_instance_key(layer.id, 1);
        assert!(!e.is_instance_keyframe(layer.id, 1));
        e.undo();
        assert!(e.is_instance_keyframe(layer.id, 1));
        assert!(e.enter_instance());
        assert_eq!(e.active, ArtboardId(2));
    }

    #[test]
    fn groups_do_not_leak_across_layers() {
        let mut e = editor();
        e.add_instance_layer(ArtboardId(2));
        let a = e.layer;
        // Desde una capa de imagen, otra capa de objetos (no un objeto más en la de A).
        e.select_layer(LayerId(1));
        e.add_instance_layer(ArtboardId(2));
        let b = e.layer;
        assert_ne!(a, b);
        let track = |e: &Editor, l| e.sprite().layer(l).unwrap().objects.clone();
        let (a0, b0) = (track(&e, a), track(&e, b));
        e.select_layer(a);
        e.update_instance_key_grouped(Some(7), |k| k.position = (1, 1));
        let a1 = track(&e, a);
        // Mismo widget (mismo grupo), otra capa: otro paso de undo.
        e.select_layer(b);
        e.update_instance_key_grouped(Some(7), |k| k.position = (2, 2));
        assert_ne!(track(&e, b), b0);
        e.undo();
        assert_eq!(track(&e, b), b0);
        assert_eq!(track(&e, a), a1, "undoing B leaves A's change");
        e.undo();
        assert_eq!(track(&e, a), a0);
        // Aunque la capa cambie sin pasar por select_layer, el objetivo distinto corta el grupo.
        e.layer = a;
        e.update_instance_key_grouped(Some(9), |k| k.position = (3, 3));
        e.layer = b;
        e.update_instance_key_grouped(Some(9), |k| k.position = (4, 4));
        e.undo();
        assert_eq!(track(&e, b), b0);
        assert_ne!(track(&e, a), a0);
    }

    #[test]
    fn grouped_updates_undo_as_one_step() {
        let mut e = editor();
        e.add_instance_layer(ArtboardId(2));
        e.set_frame(1);
        let original = e.sprite().layer(e.layer).unwrap().objects.clone();
        for x in 1..4 {
            e.update_instance_key_grouped(Some(7), |k| k.position = (x, 0));
        }
        assert_eq!(e.instance_key().unwrap().position, (3, 0));
        e.undo();
        assert_eq!(e.sprite().layer(e.layer).unwrap().objects, original);
        // Un grupo nuevo (o el mismo tras end_group) es otro paso.
        e.update_instance_key_grouped(Some(7), |k| k.position = (5, 0));
        e.end_group();
        e.update_instance_key_grouped(Some(7), |k| k.position = (6, 0));
        e.update_instance_key_grouped(Some(8), |k| k.position = (7, 0));
        e.undo();
        assert_eq!(e.instance_key().unwrap().position, (6, 0));
        e.undo();
        assert_eq!(e.instance_key().unwrap().position, (5, 0));
        e.undo();
        assert_eq!(e.sprite().layer(e.layer).unwrap().objects, original);
    }

    /// Un símbolo de 2×2 en la biblioteca (el artboard 2) y el activo es el 1, de 16×16.
    fn with_symbol() -> (Editor, ArtboardId) {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.add_artboard(2, 2);
        let symbol = e.active;
        e.move_active_to_library();
        e.activate(ArtboardId(1));
        (e, symbol)
    }

    #[test]
    fn dropping_twice_on_an_object_layer_adds_two_objects_to_it() {
        let (mut e, symbol) = with_symbol();
        let layers = e.sprite().layers.len();
        e.drop_symbol(symbol, (1, 1)).unwrap();
        let layer = e.layer;
        e.drop_symbol(symbol, (5, 6)).unwrap();
        assert_eq!(e.sprite().layers.len(), layers + 1, "one object layer");
        assert_eq!(e.layer, layer);
        let objects = &e.sprite().layer(layer).unwrap().objects;
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[1].track.keys[0].position, (5, 6));
        assert_eq!(
            e.selected_objects(),
            [objects[1].id],
            "the new one is selected"
        );
        // Un paso de undo por objeto.
        e.undo();
        assert_eq!(e.sprite().layer(layer).unwrap().objects.len(), 1);
        // Con la capa bloqueada, otra capa.
        e.document
            .artboard_mut(ArtboardId(1))
            .unwrap()
            .sprite
            .layer_mut(layer)
            .unwrap()
            .locked = true;
        e.drop_symbol(symbol, (2, 2)).unwrap();
        assert_ne!(e.layer, layer);
        assert_eq!(e.sprite().layers.len(), layers + 2);
    }

    #[test]
    fn new_object_goes_into_the_active_object_layer_and_is_selected() {
        let (mut e, symbol) = with_symbol();
        e.add_instance_layer(symbol);
        let layer = e.layer;
        let first = e.sprite().layer(layer).unwrap().objects[0].id;
        assert_eq!(e.selected_objects(), [first]);
        e.add_instance_layer(symbol);
        assert_eq!(e.layer, layer);
        let objects = &e.sprite().layer(layer).unwrap().objects;
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[1].track.keys[0].position, (7, 7), "centered");
        assert_eq!(e.selected_objects(), [objects[1].id]);
    }

    #[test]
    fn a_change_applies_to_every_selected_object_in_one_undo() {
        let (mut e, symbol) = with_symbol();
        e.add_instance_layer(symbol);
        e.add_instance_layer(symbol);
        e.add_instance_layer(symbol);
        let layer = e.layer;
        let ids: Vec<ObjectId> = e
            .sprite()
            .layer(layer)
            .unwrap()
            .objects
            .iter()
            .map(|o| o.id)
            .collect();
        e.set_selected_objects(vec![ids[0], ids[2]]);
        assert_eq!(e.edited_objects(), [ids[0], ids[2]]);
        e.update_instance_key(|k| k.flip_h = true);
        let flips = |e: &Editor| -> Vec<bool> {
            e.sprite()
                .layer(layer)
                .unwrap()
                .objects
                .iter()
                .map(|o| o.track.keys[0].flip_h)
                .collect()
        };
        assert_eq!(flips(&e), [true, false, true]);
        e.undo();
        assert_eq!(flips(&e), [false, false, false]);
    }

    #[test]
    fn renaming_the_current_object_sticks() {
        let (mut e, symbol) = with_symbol();
        e.add_instance_layer(symbol);
        let original = e.current_object_name().unwrap().to_string();
        e.rename_current_object("Hero", Some(3));
        e.rename_current_object("Hero2", Some(3));
        e.end_group();
        assert_eq!(e.current_object_name(), Some("Hero2"));
        // Sigue después de mover la selección y volver.
        e.clear_object_selection();
        assert_eq!(e.current_object_name(), Some("Hero2"));
        e.undo();
        assert_eq!(e.current_object_name(), Some(original.as_str()));
    }

    #[test]
    fn convert_to_symbol_selects_the_new_object() {
        use cs_core::{PixelValue, Rgba};
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(1, 1, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
        });
        e.convert_to_symbol("S", cs_commands::symbol::SymbolTiming::Graphic)
            .unwrap();
        let object = e.sprite().layer(e.layer).unwrap().objects[0].id;
        assert_eq!(e.selected_objects(), [object]);
    }
}
