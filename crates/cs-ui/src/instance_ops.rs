//! Instancias de artboards (símbolos) desde el editor y el panel.

use cs_commands::{Command, ops};
use cs_core::{ArtboardId, ColorMode, InstanceKey, LayerId};

use crate::editor::Editor;

impl Editor {
    /// Artboards que se pueden instanciar en el activo: no crean ciclos y el activo es RGBA.
    pub fn instance_candidates(&self) -> Vec<(ArtboardId, String)> {
        if self.sprite().color_mode != ColorMode::Rgba {
            return Vec::new();
        }
        self.document
            .artboards
            .iter()
            .filter(|a| !self.document.would_cycle(self.active, a.id))
            .map(|a| (a.id, a.name.clone()))
            .collect()
    }

    /// Nueva capa de instancia de `source` sobre la capa actual, centrada, con key en el frame actual.
    pub fn add_instance_layer(&mut self, source: ArtboardId) {
        let Some(src) = self.document.artboard(source) else {
            return;
        };
        let sprite = self.sprite();
        let position = (
            (sprite.width as i32 - src.sprite.width as i32) / 2,
            (sprite.height as i32 - src.sprite.height as i32) / 2,
        );
        let cmd = ops::new_instance_layer(
            sprite,
            Some(self.layer),
            self.frame,
            source,
            &src.name,
            position,
        );
        let id = match &cmd {
            Command::AddLayer { layer, .. } => layer.id,
            _ => unreachable!("new_instance_layer devuelve AddLayer"),
        };
        if self.execute(cmd) {
            self.layer = id;
        }
    }

    /// Key vigente de la capa actual en el frame actual (si es una capa de instancia).
    pub fn instance_key(&self) -> Option<InstanceKey> {
        let track = self.sprite().layer(self.layer)?.instance.as_ref()?;
        track.key_at(self.frame).copied()
    }

    pub fn is_instance_keyframe(&self, layer: LayerId, frame: usize) -> bool {
        self.sprite()
            .layer(layer)
            .and_then(|l| l.instance.as_ref())
            .is_some_and(|t| t.keys.iter().any(|k| k.frame == frame))
    }

    /// Cambia la key vigente de la capa actual; si el frame actual no es keyframe, crea uno.
    pub fn update_instance_key(&mut self, f: impl FnOnce(&mut InstanceKey)) {
        self.update_instance_key_grouped(None, f);
    }

    /// `update_instance_key` como parte del gesto `group` (un solo paso de undo por gesto).
    pub fn update_instance_key_grouped(
        &mut self,
        group: Option<u64>,
        f: impl FnOnce(&mut InstanceKey),
    ) {
        if let Some(cmd) = ops::update_instance_key(self.sprite(), self.layer, self.frame, f) {
            match group {
                Some(g) => self.execute_grouped(cmd, g),
                None => self.execute(cmd),
            };
        }
    }

    /// Keyframe en `frame` copiando el vigente.
    pub fn insert_instance_key(&mut self, layer: LayerId, frame: usize) {
        if let Some(cmd) = ops::update_instance_key(self.sprite(), layer, frame, |_| {}) {
            self.execute(cmd);
        }
    }

    pub fn remove_instance_key(&mut self, layer: LayerId, frame: usize) {
        if let Some(cmd) = ops::remove_instance_key(self.sprite(), layer, frame) {
            self.execute(cmd);
        }
    }

    /// "Entrar al símbolo": activa el artboard fuente de la instancia actual.
    pub fn enter_instance(&mut self) -> bool {
        let Some(source) = self.instance_key().and_then(|k| k.source) else {
            return false;
        };
        if self.document.artboard(source).is_none() {
            return false;
        }
        self.activate(source);
        true
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
        assert_eq!(layer.kind, LayerKind::Instance);
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
    fn grouped_updates_undo_as_one_step() {
        let mut e = editor();
        e.add_instance_layer(ArtboardId(2));
        e.set_frame(1);
        let original = e.sprite().layer(e.layer).unwrap().instance.clone();
        for x in 1..4 {
            e.update_instance_key_grouped(Some(7), |k| k.position = (x, 0));
        }
        assert_eq!(e.instance_key().unwrap().position, (3, 0));
        e.undo();
        assert_eq!(e.sprite().layer(e.layer).unwrap().instance, original);
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
        assert_eq!(e.sprite().layer(e.layer).unwrap().instance, original);
    }
}
