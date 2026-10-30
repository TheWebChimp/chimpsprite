//! Instancias de artboards (símbolos) desde el editor y el panel.

use cs_commands::{Command, DocCommand, ops};
use cs_core::{ArtboardId, InstanceKey, LayerId};

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
        library.sort_by(|a, b| a.1.cmp(&b.1));
        canvas.sort_by(|a, b| a.1.cmp(&b.1));
        (library, canvas)
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

    /// Suelta el símbolo `source` en el punto `world` del lienzo: crea una capa de instancia en el
    /// artboard de ese punto (en modo símbolo, solo el que se edita cuenta).
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
        let above = (target == self.active).then_some(self.layer);
        let cmd =
            ops::new_instance_layer(&board.sprite, above, self.frame, source, &src.name, local);
        let id = match &cmd {
            Command::AddLayer { layer, .. } => layer.id,
            _ => unreachable!("new_instance_layer devuelve AddLayer"),
        };
        if self.execute_doc(DocCommand::OnArtboard { id: target, cmd }) {
            self.activate(target);
            self.select_layer(id);
        }
        Ok(())
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
        let layers = e.sprite().layers.len();
        e.drop_symbol(symbol, (5, 6)).unwrap();
        assert_eq!(e.sprite().layers.len(), layers + 1);
        let layer = e.sprite().layer(e.layer).unwrap();
        assert_eq!(layer.instance.as_ref().unwrap().keys[0].position, (5, 6));
        assert!(
            e.drop_symbol(symbol, (500, 500)).is_err(),
            "outside every artboard"
        );
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
    fn groups_do_not_leak_across_layers() {
        let mut e = editor();
        e.add_instance_layer(ArtboardId(2));
        let a = e.layer;
        e.add_instance_layer(ArtboardId(2));
        let b = e.layer;
        let track = |e: &Editor, l| e.sprite().layer(l).unwrap().instance.clone();
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
