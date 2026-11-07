//! Tweens de objetos desde el editor: Create Tween / Remove Tween del timeline y el Ease del
//! panel Object.

use cs_commands::ops;
use cs_core::{LayerId, ObjectId, Tween, TweenSpan};

use crate::editor::Editor;

impl Editor {
    /// ¿Create Tween en `frame` en la fila de `layer`? Algún objeto de la fila (ver
    /// `key_objects`) tiene ahí una key vigente sin tween y con una siguiente.
    pub fn row_can_create_tween(&self, layer: LayerId, frame: usize) -> bool {
        self.key_tracks(layer)
            .any(|t| ops::can_create_tween(t, frame))
    }

    /// ¿Remove Tween en `frame` en la fila de `layer`?
    pub fn row_has_tween(&self, layer: LayerId, frame: usize) -> bool {
        self.key_tracks(layer).any(|t| ops::has_tween(t, frame))
    }

    /// Create Tween en `frame` para `objects` de `layer`: un solo paso de undo.
    pub fn create_tweens(&mut self, layer: LayerId, objects: &[ObjectId], frame: usize) {
        if let Some(cmd) = ops::create_tweens(self.sprite(), layer, objects, frame) {
            self.execute(cmd);
        }
    }

    /// Remove Tween en `frame` para `objects` de `layer`: un solo paso de undo.
    pub fn remove_tweens(&mut self, layer: LayerId, objects: &[ObjectId], frame: usize) {
        if let Some(cmd) = ops::remove_tweens(self.sprite(), layer, objects, frame) {
            self.execute(cmd);
        }
    }

    /// El tween del objeto actual en el frame actual (el de su tramo) y el tramo.
    pub fn current_tween(&self) -> Option<(Tween, TweenSpan)> {
        let track = self.object_track(self.layer)?;
        let span = track.tween_span(self.frame)?;
        let tween = track.key_at(self.frame)?.tween?;
        Some((tween, span))
    }

    /// ¿Se le puede crear un tween al objeto actual en el frame actual?
    pub fn can_create_current_tween(&self) -> bool {
        self.object_track(self.layer)
            .is_some_and(|t| ops::can_create_tween(t, self.frame))
    }

    /// Create Tween (panel Object) en los objetos que se editan, en el frame actual.
    pub fn create_current_tween(&mut self) {
        let objects = self.edited_objects();
        self.create_tweens(self.layer, &objects, self.frame);
    }

    /// Remove Tween (panel Object) en los objetos que se editan, en el frame actual.
    pub fn remove_current_tween(&mut self) {
        let objects = self.edited_objects();
        self.remove_tweens(self.layer, &objects, self.frame);
    }

    /// Cambia con `f` el tween del tramo actual de los objetos que se editan (sin crear
    /// keyframes), como parte del gesto `group`.
    pub fn update_current_tween(&mut self, group: Option<u64>, f: impl FnMut(&mut Tween)) {
        let objects = self.edited_objects();
        if let Some(cmd) = ops::update_tweens(self.sprite(), self.layer, &objects, self.frame, f) {
            match group {
                Some(g) => self.execute_grouped(cmd, g),
                None => self.execute(cmd),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ArtboardId, ColorMode, Ease, Sprite};

    use super::*;

    /// Editor con 4 frames y una capa de objetos con un objeto con keys en 0 (en (0, 0)) y 3 (en
    /// (6, 3)). Devuelve la capa y el objeto.
    fn editor() -> (Editor, LayerId, ObjectId) {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        for _ in 0..3 {
            e.insert_empty_frame();
        }
        e.add_artboard(4, 4);
        e.activate(ArtboardId(1));
        e.set_frame(0);
        e.add_instance_layer(ArtboardId(2));
        let (layer, object) = (e.layer, e.current_object().unwrap());
        e.update_instance_key(|k| k.position = (0, 0));
        e.set_frame(3);
        e.update_instance_key(|k| k.position = (6, 3));
        e.set_frame(0);
        (e, layer, object)
    }

    #[test]
    fn create_and_remove_from_the_row_are_one_undo_each() {
        let (mut e, layer, object) = editor();
        assert!(e.row_can_create_tween(layer, 1) && !e.row_has_tween(layer, 1));
        assert!(
            !e.row_can_create_tween(layer, 3),
            "no key after the last one"
        );
        let before = e.document.clone();
        e.create_tweens(layer, &[object], 1);
        assert!(e.row_has_tween(layer, 2) && !e.row_can_create_tween(layer, 2));
        e.set_frame(1);
        assert_eq!(e.instance_key().unwrap().position, (2, 1), "what is seen");
        e.remove_tweens(layer, &[object], 2);
        assert!(!e.row_has_tween(layer, 2));
        e.undo();
        assert!(e.row_has_tween(layer, 2));
        e.undo();
        assert_eq!(e.document, before);
    }

    #[test]
    fn the_object_panel_edits_the_tween_without_keyframes() {
        let (mut e, layer, _) = editor();
        e.set_frame(2);
        assert!(e.current_tween().is_none() && e.can_create_current_tween());
        e.create_current_tween();
        let (tween, span) = e.current_tween().unwrap();
        assert_eq!(tween, Tween::default());
        assert_eq!((span.start, span.end, span.broken), (0, Some(3), false));
        e.update_current_tween(None, |t| t.ease = Ease::Classic(100));
        let keys = &e.sprite().layer(layer).unwrap().objects[0].track.keys;
        assert_eq!(keys.len(), 2, "no keyframe on frame 2");
        assert_eq!(keys[0].tween.unwrap().ease, Ease::Classic(100));
        e.remove_current_tween();
        assert!(e.current_tween().is_none());
    }
}
