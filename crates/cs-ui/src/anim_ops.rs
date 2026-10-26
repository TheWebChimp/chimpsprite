//! Operaciones del timeline sobre el `Editor`: frames, capas, cels, tags y reproducción.

use cs_commands::ops::{self, LayerDrop};
use cs_commands::{Command, LayerProps};
use cs_core::{AnimDirection, BlendMode, Frame, LayerId, LayerKind, Playback, Rgba, Tag};

use crate::editor::Editor;

/// Colores que se van alternando para los tags nuevos.
const TAG_COLORS: [Rgba; 6] = [
    Rgba::rgb(0xf2, 0x8c, 0x28),
    Rgba::rgb(0x5b, 0x6e, 0xe1),
    Rgba::rgb(0x6a, 0xbe, 0x30),
    Rgba::rgb(0xd9, 0x57, 0x63),
    Rgba::rgb(0x5f, 0xcd, 0xe4),
    Rgba::rgb(0xd7, 0x7b, 0xba),
];

impl Editor {
    // --- Frames y reproducción ---------------------------------------------------------------

    pub fn frame_count(&self) -> usize {
        self.sprite().frames.len()
    }

    pub fn set_frame(&mut self, frame: usize) {
        let frame = frame.min(self.frame_count() - 1);
        if frame != self.frame {
            self.commit_floating();
            self.end_group();
            self.frame = frame;
            self.revision += 1;
        }
    }

    pub fn next_frame(&mut self) {
        self.set_frame((self.frame + 1) % self.frame_count());
    }

    pub fn prev_frame(&mut self) {
        let n = self.frame_count();
        self.set_frame((self.frame + n - 1) % n);
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.playback = None;
    }

    /// Reproduce el tag que contiene al frame actual o, si no hay, todos los frames.
    pub fn toggle_play(&mut self) {
        if self.playing {
            self.stop();
            return;
        }
        let (range, direction) = match self.sprite().tag_at(self.frame) {
            Some(tag) => ((tag.from, tag.to), tag.direction),
            None => ((0, self.frame_count() - 1), AnimDirection::Forward),
        };
        self.playback = Some((Playback::new(self.frame, direction), range, direction));
        self.play_clock = 0.0;
        self.playing = true;
    }

    /// Avanza la reproducción `dt_ms` milisegundos según la duración de cada frame.
    pub fn tick(&mut self, dt_ms: f64) {
        if !self.playing {
            return;
        }
        let Some((mut playback, range, direction)) = self.playback else {
            return;
        };
        self.play_clock += dt_ms;
        // Tope para no girar sin fin si la app estuvo pausada mucho tiempo.
        for _ in 0..self.frame_count() * 4 {
            let duration = self.sprite().frames[self.frame].duration_ms.max(1) as f64;
            if self.play_clock < duration {
                break;
            }
            self.play_clock -= duration;
            playback.step(range, direction);
            self.set_frame(playback.frame);
        }
        self.play_clock = self.play_clock.min(1000.0);
        self.playback = Some((playback, range, direction));
    }

    /// Frames afectados por las acciones de rango: la selección o el frame actual.
    pub fn selected_frames(&self) -> (usize, usize) {
        self.frame_range
            .map(|(a, b)| (a.min(b), a.max(b)))
            .unwrap_or((self.frame, self.frame))
    }

    /// Duplica el frame actual (copias independientes) y se mueve a la copia.
    pub fn duplicate_frame(&mut self) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(cmd) = ops::duplicate_frame(self.sprite(), self.frame, false)
            && self.execute(cmd)
        {
            self.set_frame(self.frame + 1);
        }
    }

    pub fn insert_empty_frame(&mut self) {
        let duration_ms = self.sprite().frames[self.frame].duration_ms;
        if self.execute(Command::AddFrame {
            index: self.frame + 1,
            frame: Frame { duration_ms },
        }) {
            self.set_frame(self.frame + 1);
        }
    }

    pub fn remove_frame(&mut self) {
        if self.frame_count() > 1 {
            self.execute(Command::RemoveFrame { index: self.frame });
            self.frame_range = None;
        }
    }

    pub fn move_frame(&mut self, from: usize, to: usize) {
        if from != to && self.execute(Command::MoveFrame { from, to }) {
            self.frame = to;
            self.frame_range = None;
        }
    }

    /// Cambia la duración de los frames seleccionados (un solo paso de undo).
    pub fn set_duration(&mut self, duration_ms: u16) {
        let (a, b) = self.selected_frames();
        let cmds = (a..=b)
            .map(|index| Command::SetFrameDuration { index, duration_ms })
            .collect();
        self.execute(Command::Batch(cmds));
    }

    /// Cambia la duración de un frame concreto.
    pub fn set_duration_of(&mut self, index: usize, duration_ms: u16) {
        self.execute(Command::SetFrameDuration { index, duration_ms });
    }

    // --- Tags --------------------------------------------------------------------------------

    /// Crea un tag con los frames seleccionados.
    pub fn add_tag(&mut self) {
        let (from, to) = self.selected_frames();
        let mut tags = self.sprite().tags.clone();
        let n = tags.len();
        tags.push(Tag {
            name: format!("Tag {}", n + 1),
            from,
            to,
            direction: AnimDirection::Forward,
            color: TAG_COLORS[n % TAG_COLORS.len()],
            repeat: 0,
        });
        self.execute(Command::SetTags(tags));
    }

    pub fn set_tag(&mut self, index: usize, tag: Tag) {
        let mut tags = self.sprite().tags.clone();
        if index < tags.len() && tags[index] != tag {
            tags[index] = tag;
            self.execute(Command::SetTags(tags));
        }
    }

    pub fn remove_tag(&mut self, index: usize) {
        let mut tags = self.sprite().tags.clone();
        if index < tags.len() {
            tags.remove(index);
            self.execute(Command::SetTags(tags));
        }
    }

    // --- Capas -------------------------------------------------------------------------------

    /// Selecciona la capa para pintar sus píxeles (no su máscara).
    pub fn select_layer(&mut self, id: LayerId) {
        if self.sprite().layer(id).is_some() {
            let before = self.paint_target();
            if id != self.layer {
                self.commit_floating();
                self.end_group();
                // El aviso de límite de tamaño era de la capa anterior.
                self.tile_limit = false;
            }
            self.layer = id;
            self.edit_mask = false;
            // Pasar a una capa tilemap (o salir de la máscara) a mitad de un trazo lo corta.
            self.cancel_if_target_changed(before);
        }
    }

    /// Capas en el orden del timeline (arriba → abajo), omitiendo las de grupos colapsados.
    pub fn timeline_layers(&self) -> Vec<LayerId> {
        let sprite = self.sprite();
        sprite
            .layers
            .iter()
            .rev()
            .filter(|l| {
                !sprite.layers.iter().any(|g| {
                    self.collapsed.contains(&(self.active, g.id))
                        && sprite.is_descendant(l.id, g.id)
                })
            })
            .map(|l| l.id)
            .collect()
    }

    /// Selecciona la capa de arriba (`up`) o de abajo en el timeline.
    pub fn step_layer(&mut self, up: bool) {
        let rows = self.timeline_layers();
        let Some(i) = rows.iter().position(|&id| id == self.layer) else {
            return;
        };
        let next = if up {
            i.checked_sub(1)
        } else {
            Some(i + 1).filter(|&n| n < rows.len())
        };
        if let Some(n) = next {
            self.layer = rows[n];
        }
    }

    pub fn toggle_collapsed(&mut self, id: LayerId) {
        let key = (self.active, id);
        if !self.collapsed.remove(&key) {
            self.collapsed.insert(key);
        }
    }

    pub fn add_layer(&mut self, kind: LayerKind) {
        let cmd = ops::new_layer(self.sprite(), Some(self.layer), kind);
        let id = match &cmd {
            Command::AddLayer { layer, .. } => layer.id,
            _ => unreachable!("new_layer devuelve AddLayer"),
        };
        if self.execute(cmd) {
            self.layer = id;
        }
    }

    /// Quita la capa activa (y su contenido si es grupo). Siempre queda al menos una capa.
    pub fn remove_layer(&mut self) {
        let sprite = self.sprite();
        let Some(range) = sprite.subtree_range(self.layer) else {
            return;
        };
        if range.len() >= sprite.layers.len() {
            return;
        }
        if let Some(cmd) = ops::remove_layer_tree(sprite, self.layer) {
            self.execute(cmd);
        }
    }

    pub fn move_layer(&mut self, id: LayerId, drop: LayerDrop) {
        if let Some(cmd) = ops::move_layer(self.sprite(), id, drop) {
            self.execute(cmd);
        }
    }

    pub fn set_layer_props(&mut self, id: LayerId, props: LayerProps) {
        self.execute(Command::SetLayerProps { id, props });
    }

    pub fn toggle_visible(&mut self, id: LayerId) {
        if let Some(l) = self.sprite().layer(id) {
            let visible = Some(!l.visible);
            self.set_layer_props(
                id,
                LayerProps {
                    visible,
                    ..Default::default()
                },
            );
        }
    }

    pub fn toggle_locked(&mut self, id: LayerId) {
        if let Some(l) = self.sprite().layer(id) {
            let locked = Some(!l.locked);
            self.set_layer_props(
                id,
                LayerProps {
                    locked,
                    ..Default::default()
                },
            );
        }
    }

    pub fn set_opacity(&mut self, id: LayerId, opacity: u8) {
        if self
            .sprite()
            .layer(id)
            .is_some_and(|l| l.opacity != opacity)
        {
            self.set_layer_props(
                id,
                LayerProps {
                    opacity: Some(opacity),
                    ..Default::default()
                },
            );
        }
    }

    pub fn set_blend(&mut self, id: LayerId, blend: BlendMode) {
        if self.sprite().layer(id).is_some_and(|l| l.blend != blend) {
            self.set_layer_props(
                id,
                LayerProps {
                    blend: Some(blend),
                    ..Default::default()
                },
            );
        }
    }

    pub fn rename_layer(&mut self, id: LayerId, name: String) {
        if self.sprite().layer(id).is_some_and(|l| l.name != name) && !name.is_empty() {
            self.set_layer_props(
                id,
                LayerProps {
                    name: Some(name),
                    ..Default::default()
                },
            );
        }
    }

    // --- Cels --------------------------------------------------------------------------------

    /// Vincula el cel con el del frame anterior de la misma capa.
    pub fn link_to_previous(&mut self, layer: LayerId, frame: usize) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(prev) = frame.checked_sub(1)
            && let Some(cmd) = ops::link_cel(self.sprite(), layer, frame, prev)
        {
            self.execute(cmd);
        }
    }

    pub fn unlink(&mut self, layer: LayerId, frame: usize) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(cmd) = ops::unlink_cel(self.sprite(), layer, frame) {
            self.execute(cmd);
        }
    }

    /// Vincula los cels del rango seleccionado de una capa (un paso de undo).
    pub fn link_range(&mut self, layer: LayerId, from: usize, to: usize) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(cmd) = ops::link_range(self.sprite(), layer, from, to) {
            self.execute(cmd);
        }
    }

    pub fn unlink_range(&mut self, layer: LayerId, from: usize, to: usize) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(cmd) = ops::unlink_range(self.sprite(), layer, from, to) {
            self.execute(cmd);
        }
    }

    /// Agrega un color al final de la paleta (si no está ya).
    /// Agrega `color` como una entrada nueva al final de la paleta (aunque ya esté: así se puede
    /// partir de un color y editarlo) y la elige como primario. `false` si ya hay 256.
    pub fn add_to_palette(&mut self, color: Rgba) -> bool {
        let palette = &self.sprite().palette;
        if palette.len() >= 256 {
            return false;
        }
        let index = palette.len() as u8;
        let mut colors = palette.colors().to_vec();
        colors.push(color);
        if !self.execute(Command::SetPalette(cs_core::Palette::new(colors))) {
            return false;
        }
        let value = self.palette_value(index);
        self.pick_color(value, false, Some(index));
        true
    }

    pub fn clear_cel(&mut self, layer: LayerId, frame: usize) {
        if self.sprite().cel(layer, frame).is_some() {
            self.execute(Command::SetCel {
                layer,
                frame,
                cel: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, PixelValue, Sprite};

    use super::*;

    fn editor_with_frames(n: usize) -> Editor {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        for _ in 1..n {
            e.insert_empty_frame();
        }
        e.set_frame(0);
        e
    }

    #[test]
    fn playback_respects_durations_and_tags() {
        let mut e = editor_with_frames(4);
        e.set_frame(1);
        e.frame_range = Some((1, 2));
        e.add_tag();
        e.toggle_play();
        e.tick(99.0);
        assert_eq!(e.frame, 1);
        e.tick(1.0);
        assert_eq!(e.frame, 2);
        e.tick(100.0);
        assert_eq!(e.frame, 1, "vuelve al inicio del tag");
        e.toggle_play();
        e.tick(500.0);
        assert_eq!(e.frame, 1, "pausado no avanza");
    }

    #[test]
    fn painting_stops_playback() {
        let mut e = editor_with_frames(2);
        e.toggle_play();
        e.pointer_press((0.5, 0.5), false, false, 0.1);
        e.pointer_release();
        assert!(!e.playing);
    }

    #[test]
    fn duplicate_frame_copies_and_moves_to_copy() {
        let mut e = editor_with_frames(1);
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(1, 1, PixelValue::Rgba(Rgba::rgb(9, 9, 9)))],
        });
        e.duplicate_frame();
        assert_eq!((e.frame_count(), e.frame), (2, 1));
        assert_eq!(
            e.sprite().pixel(e.layer, 1, 1, 1),
            PixelValue::Rgba(Rgba::rgb(9, 9, 9))
        );
        assert!(!ops::is_linked(e.sprite(), e.layer, 1));
    }

    #[test]
    fn durations_apply_to_selection_in_one_undo_step() {
        let mut e = editor_with_frames(3);
        e.frame_range = Some((2, 0));
        e.set_duration(40);
        assert!(e.sprite().frames.iter().all(|f| f.duration_ms == 40));
        e.undo();
        assert!(e.sprite().frames.iter().all(|f| f.duration_ms == 100));
    }

    #[test]
    fn layers_add_select_step_collapse_and_keep_one() {
        let mut e = editor_with_frames(1);
        e.add_layer(LayerKind::Group);
        let group = e.layer;
        e.add_layer(LayerKind::Image);
        // La capa nueva queda encima del grupo, en el mismo nivel.
        e.move_layer(e.layer, LayerDrop::Into(group));
        let child = e.layer;
        assert_eq!(e.timeline_layers().len(), 3);
        e.toggle_collapsed(group);
        assert_eq!(e.timeline_layers(), vec![group, LayerId(1)]);
        e.select_layer(group);
        e.step_layer(false);
        assert_eq!(e.layer, LayerId(1));
        e.toggle_collapsed(group);
        e.select_layer(group);
        e.remove_layer();
        assert!(
            e.sprite().layer(child).is_none(),
            "borra el grupo con su contenido"
        );
        e.remove_layer();
        assert_eq!(e.sprite().layers.len(), 1, "siempre queda una capa");
    }

    #[test]
    fn visibility_lock_opacity_blend_rename() {
        let mut e = editor_with_frames(1);
        let id = e.layer;
        e.toggle_visible(id);
        e.toggle_locked(id);
        e.set_opacity(id, 50);
        e.set_blend(id, BlendMode::Multiply);
        e.rename_layer(id, "ink".into());
        let l = e.sprite().layer(id).unwrap();
        assert!(!l.visible && l.locked);
        assert_eq!(
            (l.opacity, l.blend, l.name.as_str()),
            (50, BlendMode::Multiply, "ink")
        );
        for _ in 0..5 {
            e.undo();
        }
        assert_eq!(e.sprite().layer(id).unwrap().name, "Layer 1");
    }

    #[test]
    fn palette_add_appends_even_repeated_colors_and_is_undoable() {
        let mut e = editor_with_frames(1);
        let n = e.sprite().palette.len();
        assert!(e.add_to_palette(Rgba::rgb(1, 2, 3)));
        assert!(
            e.add_to_palette(Rgba::rgb(1, 2, 3)),
            "a repeated color is a new entry"
        );
        assert_eq!(e.sprite().palette.len(), n + 2);
        assert_eq!(
            e.primary,
            e.palette_value((n + 1) as u8),
            "the new entry is picked"
        );
        e.undo();
        assert_eq!(e.sprite().palette.len(), n + 1);
    }

    #[test]
    fn link_unlink_clear() {
        let mut e = editor_with_frames(2);
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 1, 1)))],
        });
        e.link_to_previous(e.layer, 1);
        assert!(ops::is_linked(e.sprite(), e.layer, 1));
        e.unlink(e.layer, 1);
        assert!(!ops::is_linked(e.sprite(), e.layer, 1));
        e.clear_cel(e.layer, 1);
        assert!(e.sprite().cel(e.layer, 1).is_none());
    }
}
