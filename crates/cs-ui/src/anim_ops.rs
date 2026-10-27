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
            && self.execute_named(cmd, "Duplicate Frame")
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

    /// `set_layer_props` con `label` como nombre del paso.
    fn named_props(&mut self, id: LayerId, props: LayerProps, label: &'static str) {
        self.execute_named(Command::SetLayerProps { id, props }, label);
    }

    pub fn toggle_visible(&mut self, id: LayerId) {
        if let Some(l) = self.sprite().layer(id) {
            let visible = Some(!l.visible);
            self.named_props(
                id,
                LayerProps {
                    visible,
                    ..Default::default()
                },
                "Layer Visibility",
            );
        }
    }

    pub fn toggle_locked(&mut self, id: LayerId) {
        if let Some(l) = self.sprite().layer(id) {
            let locked = Some(!l.locked);
            self.named_props(
                id,
                LayerProps {
                    locked,
                    ..Default::default()
                },
                "Layer Lock",
            );
        }
    }

    pub fn set_opacity(&mut self, id: LayerId, opacity: u8) {
        if self
            .sprite()
            .layer(id)
            .is_some_and(|l| l.opacity != opacity)
        {
            self.named_props(
                id,
                LayerProps {
                    opacity: Some(opacity),
                    ..Default::default()
                },
                "Layer Opacity",
            );
        }
    }

    pub fn set_blend(&mut self, id: LayerId, blend: BlendMode) {
        if self.sprite().layer(id).is_some_and(|l| l.blend != blend) {
            self.named_props(
                id,
                LayerProps {
                    blend: Some(blend),
                    ..Default::default()
                },
                "Blend Mode",
            );
        }
    }

    pub fn rename_layer(&mut self, id: LayerId, name: String) {
        if self.sprite().layer(id).is_some_and(|l| l.name != name) && !name.is_empty() {
            self.named_props(
                id,
                LayerProps {
                    name: Some(name),
                    ..Default::default()
                },
                "Rename Layer",
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
            self.execute_named(cmd, "Link Cels");
        }
    }

    pub fn unlink(&mut self, layer: LayerId, frame: usize) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(cmd) = ops::unlink_cel(self.sprite(), layer, frame) {
            self.execute_named(cmd, "Unlink Cels");
        }
    }

    /// Vincula los cels del rango seleccionado de una capa (un paso de undo).
    pub fn link_range(&mut self, layer: LayerId, from: usize, to: usize) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(cmd) = ops::link_range(self.sprite(), layer, from, to) {
            self.execute_named(cmd, "Link Cels");
        }
    }

    pub fn unlink_range(&mut self, layer: LayerId, from: usize, to: usize) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(cmd) = ops::unlink_range(self.sprite(), layer, from, to) {
            self.execute_named(cmd, "Unlink Cels");
        }
    }

    /// Agrega un color al final de la paleta (si no está ya).
    /// Agrega `color` como una entrada nueva al final de la paleta (aunque ya esté: así se puede
    /// partir de un color y editarlo) y la elige como primario. `false` si ya hay 256.
    pub fn add_to_palette(&mut self, color: Rgba) -> bool {
        self.grow_palette(color, 1)
    }

    /// Agrega `n` entradas con `color` al final de la paleta (hasta 256), en un paso de undo, y
    /// elige la primera de ellas. Devuelve si cambió.
    pub fn grow_palette(&mut self, color: Rgba, n: usize) -> bool {
        let palette = &self.sprite().palette;
        let n = n.min(256 - palette.len().min(256));
        if n == 0 {
            return false;
        }
        let index = palette.len() as u8;
        let mut colors = palette.colors().to_vec();
        colors.extend(std::iter::repeat_n(color, n));
        if !self.execute(Command::SetPalette(cs_core::Palette::new(colors))) {
            return false;
        }
        let value = self.palette_value(index);
        self.pick_color(value, false, Some(index));
        true
    }

    /// Elige el rango de cels de `(capa, frame)` a `(capa, frame)` (de cualquier punta).
    pub fn select_cels(&mut self, from: (LayerId, usize), to: (LayerId, usize)) {
        let frames = (from.1, to.1);
        self.frames_picked = false;
        self.frame_range = Some(frames);
        self.layer_range = Some((from.0, to.0, frames));
    }

    /// Las capas del rango de cels, en el orden del timeline; sin rango, la activa.
    pub fn range_layers(&self) -> Vec<LayerId> {
        let order = self.timeline_layers();
        let at = |id: LayerId| order.iter().position(|l| *l == id);
        match self.layer_range {
            Some((a, b, frames)) if self.frame_range == Some(frames) => match (at(a), at(b)) {
                (Some(a), Some(b)) => order[a.min(b)..=a.max(b)].to_vec(),
                _ => vec![self.layer],
            },
            _ => vec![self.layer],
        }
    }

    /// Los cels del rango (capas × frames), aunque estén vacíos.
    pub fn selected_cels(&self) -> Vec<(LayerId, usize)> {
        let (a, b) = self.selected_frames();
        self.range_layers()
            .into_iter()
            .flat_map(|l| (a..=b).map(move |f| (l, f)))
            .collect()
    }

    /// Si hay un rango de más de un cel.
    pub fn has_cel_range(&self) -> bool {
        self.selected_cels().len() > 1
    }

    /// Borra los cels del rango (un paso de undo).
    pub fn clear_selected_cels(&mut self) {
        self.commit_floating();
        let cmds: Vec<Command> = self
            .selected_cels()
            .into_iter()
            .filter(|&(l, f)| self.sprite().cel(l, f).is_some())
            .map(|(layer, frame)| Command::SetCel {
                layer,
                frame,
                cel: None,
            })
            .collect();
        if !cmds.is_empty() {
            self.execute_named(Command::Batch(cmds), "Clear Cels");
        }
    }

    /// Mueve (o copia, con `copy`) los cels del rango `layers` capas (en el orden del timeline,
    /// hacia abajo) y `frames` frames, y el rango con ellos (un paso de undo). Solo entre capas
    /// de imagen (las tilemap, en su misma capa); lo que caería fuera, no se mueve.
    pub fn move_selected_cels(&mut self, layers: isize, frames: isize, copy: bool) {
        if layers == 0 && frames == 0 && !copy {
            return;
        }
        self.commit_floating();
        let order = self.timeline_layers();
        let count = self.frame_count() as isize;
        let sprite = self.sprite();
        let kind = |id: LayerId| sprite.layer(id).map(|l| l.kind);
        let moves: Vec<_> = self
            .selected_cels()
            .into_iter()
            .filter_map(|(layer, f)| {
                let at = order.iter().position(|l| *l == layer)? as isize + layers;
                let to = *order.get(usize::try_from(at).ok()?)?;
                let frame = f as isize + frames;
                let fits = match kind(layer)? {
                    LayerKind::Image => kind(to) == Some(LayerKind::Image),
                    LayerKind::Tilemap => to == layer,
                    _ => false,
                };
                (fits && (0..count).contains(&frame)).then_some(((layer, f), (to, frame as usize)))
            })
            .collect();
        if moves.is_empty() {
            return;
        }
        let Some(cmd) = ops::move_cels(sprite, &moves, copy) else {
            return;
        };
        let label = if copy { "Copy Cels" } else { "Move Cels" };
        if !self.execute_named(cmd, label) {
            return;
        }
        // El rango va con los cels.
        let shift = |id: LayerId| {
            order
                .iter()
                .position(|l| *l == id)
                .and_then(|i| {
                    order.get((i as isize + layers).clamp(0, order.len() as isize - 1) as usize)
                })
                .copied()
                .unwrap_or(id)
        };
        let frame = |f: usize| (f as isize + frames).clamp(0, count - 1) as usize;
        let (a, b) = self.selected_frames();
        let range_layers = self.range_layers();
        let (first, last) = (range_layers[0], *range_layers.last().unwrap_or(&self.layer));
        self.select_cels((shift(first), frame(a)), (shift(last), frame(b)));
        let active = shift(self.layer);
        self.select_layer(active);
        self.set_frame(frame(self.frame));
        self.frame_range = Some((frame(a), frame(b)));
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

    /// Tres capas de imagen (de arriba a abajo en el timeline: 3, 2, 1) y 4 frames, con un
    /// punto en el cel de la capa 1, frame 0, y otro en la capa 2, frame 1.
    fn cels_editor() -> (Editor, [LayerId; 3]) {
        let mut e = editor_with_frames(4);
        e.add_layer(LayerKind::Image);
        e.add_layer(LayerKind::Image);
        let order = e.timeline_layers();
        let (top, mid, bottom) = (order[0], order[1], order[2]);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        for (layer, frame) in [(bottom, 0), (mid, 1)] {
            e.execute(Command::DrawPixels {
                layer,
                frame,
                pixels: vec![(0, 0, red)],
            });
        }
        (e, [top, mid, bottom])
    }

    #[test]
    fn a_cel_range_spans_layers_and_frames() {
        let (mut e, [top, mid, bottom]) = cels_editor();
        e.select_layer(bottom);
        e.select_cels((bottom, 0), (mid, 1));
        assert_eq!(
            e.range_layers(),
            vec![mid, bottom],
            "in the order of the timeline"
        );
        assert_eq!(e.selected_cels().len(), 4);
        assert!(e.has_cel_range());
        // Otro rango de frames (el encabezado): solo la capa activa.
        e.frame_range = Some((2, 3));
        assert_eq!(e.range_layers(), vec![bottom]);
        assert!(!e.selected_cels().contains(&(top, 2)));
    }

    #[test]
    fn deleting_a_cel_range_clears_its_cels_in_one_step() {
        let (mut e, [_, mid, bottom]) = cels_editor();
        e.select_cels((bottom, 0), (mid, 1));
        e.clear_selected_cels();
        assert!(e.sprite().cel(bottom, 0).is_none() && e.sprite().cel(mid, 1).is_none());
        e.undo();
        assert!(e.sprite().cel(bottom, 0).is_some() && e.sprite().cel(mid, 1).is_some());
    }

    #[test]
    fn dragging_a_cel_range_moves_or_copies_its_cels() {
        let (mut e, [top, mid, bottom]) = cels_editor();
        e.select_layer(bottom);
        e.select_cels((mid, 0), (bottom, 1));
        // Una capa arriba y dos frames después.
        e.move_selected_cels(-1, 2, false);
        assert!(e.sprite().cel(bottom, 0).is_none() && e.sprite().cel(mid, 1).is_none());
        assert!(e.sprite().cel(mid, 2).is_some(), "the bottom cel");
        assert!(e.sprite().cel(top, 3).is_some(), "the middle cel");
        assert_eq!(e.range_layers(), vec![top, mid], "the range goes with them");
        assert_eq!(e.selected_frames(), (2, 3));
        e.undo();
        assert!(e.sprite().cel(bottom, 0).is_some() && e.sprite().cel(mid, 2).is_none());
        // Copiando: los de antes se quedan y las copias llevan su propia imagen.
        e.select_layer(bottom);
        e.select_cels((mid, 0), (bottom, 1));
        e.move_selected_cels(0, 2, true);
        let (original, copy) = (e.sprite().cel(bottom, 0), e.sprite().cel(bottom, 2));
        assert!(original.is_some() && copy.is_some());
        assert_ne!(original.unwrap().image, copy.unwrap().image);
        // Lo que caería fuera de los frames no se mueve.
        e.select_cels((bottom, 0), (bottom, 0));
        e.move_selected_cels(0, 9, false);
        assert!(e.sprite().cel(bottom, 0).is_some());
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

#[cfg(test)]
/// Los cels enlazados siguen enlazados con cualquier cosa que se haga en uno: lo que se pinta o
/// se cambia en uno se ve en todos.
mod linked_cels {
    use cs_commands::Command;
    use cs_core::{ColorMode, Mask, PixelValue, Rgba, Sprite};

    use crate::editor::{Editor, ToolKind};

    fn linked() -> (Editor, cs_core::LayerId) {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.insert_empty_frame();
        e.insert_empty_frame();
        let l = e.layer;
        e.execute(Command::DrawPixels {
            layer: l,
            frame: 0,
            pixels: (0..8)
                .flat_map(|y| (0..8).map(move |x| (x, y, PixelValue::Rgba(Rgba::rgb(200, 10, 10)))))
                .collect(),
        });
        e.link_range(l, 0, 2);
        e.set_frame(1);
        (e, l)
    }

    fn assert_linked(name: &str, e: &Editor, l: cs_core::LayerId) {
        let ids: Vec<_> = (0..3)
            .map(|f| e.sprite().cel(l, f).map(|c| c.image))
            .collect();
        assert!(ids[0] == ids[1] && ids[1] == ids[2], "{name}: {ids:?}");
        let px: Vec<_> = (0..3).map(|f| e.sprite().pixel(l, f, 3, 3)).collect();
        assert!(px[0] == px[1] && px[1] == px[2], "{name}: {px:?}");
    }

    #[test]
    fn every_edit_keeps_linked_cels_linked() {
        let at = |x: i32, y: i32| (x as f32 + 0.5, y as f32 + 0.5);
        for tool in [
            ToolKind::Pencil,
            ToolKind::Eraser,
            ToolKind::Bucket,
            ToolKind::Gradient,
            ToolKind::Line,
            ToolKind::Rectangle,
            ToolKind::Ellipse,
            ToolKind::Spray,
            ToolKind::Blur,
            ToolKind::Jumble,
            ToolKind::Contour,
        ] {
            let (mut e, l) = linked();
            e.set_tool(tool);
            e.pointer_press(at(1, 1), false, false, 0.1);
            e.pointer_drag(at(5, 5));
            e.tool_tick(50.0);
            e.pointer_release();
            assert_linked(&format!("{tool:?}"), &e, l);
        }
        type Edit = fn(&mut Editor);
        let ops: Vec<(&str, Edit)> = vec![
            ("move", |e| {
                e.set_tool(ToolKind::Move);
                e.select_all();
                e.lift();
                e.nudge(2, 0);
                e.commit_floating();
            }),
            ("move_drag", |e| {
                e.set_tool(ToolKind::RectSelect);
                e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (4, 4))));
                e.pointer_press((2.5, 2.5), false, false, 0.1);
                e.pointer_drag((5.5, 5.5));
                e.pointer_release();
                e.commit_floating();
            }),
            ("paste", |e| {
                e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (2, 2))));
                let c = e.copy().unwrap();
                e.paste(&c);
                e.nudge(3, 3);
                e.commit_floating();
            }),
            ("flip", |e| {
                e.deselect();
                e.flip(true);
            }),
            ("flip_sel", |e| {
                e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (4, 4))));
                e.flip(true);
                e.commit_floating();
            }),
            ("rotate", |e| {
                e.rotate_content(crate::edit_ops::Turn::Clockwise);
                e.commit_floating();
            }),
            ("shift", |e| {
                e.shift_content(1, 0);
            }),
            ("invert", |e| {
                e.invert_colors();
            }),
            ("fill", |e| {
                e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (4, 4))));
                e.fill_selection();
            }),
            ("clear", |e| {
                e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (7, 7))));
                e.clear_selection_content();
            }),
            ("cut", |e| {
                e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (4, 4))));
                e.cut();
            }),
            ("filter", |e| {
                e.apply_filter(&crate::filters::Filter::HueSaturation {
                    hue: 90,
                    saturation: 0,
                    lightness: 0,
                });
            }),
        ];
        for (name, op) in ops {
            let (mut e, l) = linked();
            op(&mut e);
            assert_linked(name, &e, l);
        }
    }
}
