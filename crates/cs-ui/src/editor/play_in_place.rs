//! Play in Place: artboards que se animan solos en el lienzo mientras se edita otro. Es estado de
//! la sesión (de la pestaña): no está en el documento, no crea pasos de undo ni marca cambios.
//!
//! - El rango es el tag del frame de edición que tenía el artboard al prender (o todos los frames)
//!   y la dirección, la del tag; siempre en loop (el `repeat` del tag no cuenta).
//! - El activo no se anima en el lugar: muestra su frame de edición. Su reproducción queda en
//!   pausa y sigue al activar otro.

use cs_core::{AnimDirection, ArtboardId, Playback, Sprite};

use super::Editor;

/// La reproducción de un artboard en el lugar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InPlace {
    pub playback: Playback,
    /// Inclusivo.
    pub range: (usize, usize),
    pub direction: AnimDirection,
    /// Milisegundos que lleva en el frame de ahora.
    pub clock: f64,
}

impl InPlace {
    /// Desde `frame`, con el tag que lo contiene o, si no hay, todos los frames.
    fn starting(sprite: &Sprite, frame: usize) -> InPlace {
        let frame = frame.min(sprite.frames.len().saturating_sub(1));
        let (range, direction) = range_at(sprite, frame);
        InPlace {
            playback: Playback::new(frame, direction),
            range,
            direction,
            clock: 0.0,
        }
    }
}

/// El tag que contiene `frame` (rango y dirección) o todos los frames hacia adelante.
fn range_at(sprite: &Sprite, frame: usize) -> ((usize, usize), AnimDirection) {
    match sprite.tag_at(frame) {
        Some(tag) => ((tag.from, tag.to), tag.direction),
        None => (
            (0, sprite.frames.len().saturating_sub(1)),
            AnimDirection::Forward,
        ),
    }
}

impl Editor {
    /// Se puede animar en el lugar: un artboard normal (ni fuente ni modelo) con más de un frame.
    pub fn can_play_in_place(&self, id: ArtboardId) -> bool {
        self.document
            .artboard(id)
            .is_some_and(|a| a.font.is_none() && a.model.is_none() && a.sprite.frames.len() > 1)
    }

    pub fn is_playing_in_place(&self, id: ArtboardId) -> bool {
        self.in_place.contains_key(&id)
    }

    /// Prende o apaga Play in Place de `id` sin activarlo. Devuelve si quedó prendido.
    pub fn toggle_play_in_place(&mut self, id: ArtboardId) -> bool {
        if self.in_place.remove(&id).is_some() {
            return false;
        }
        if !self.can_play_in_place(id) {
            return false;
        }
        let frame = self.edit_frame_of(id);
        let sprite = &self.document.artboard(id).expect("checked").sprite;
        self.in_place.insert(id, InPlace::starting(sprite, frame));
        true
    }

    /// El frame que muestra `id` con Play in Place (siempre dentro de sus frames).
    pub(crate) fn in_place_frame(&self, id: ArtboardId) -> Option<usize> {
        let play = self.in_place.get(&id)?;
        let count = self.document.artboard(id)?.sprite.frames.len();
        Some(play.playback.frame.min(count.saturating_sub(1)))
    }

    /// Hay algún artboard animándose en el lugar (que no sea el activo).
    pub fn any_in_place(&self) -> bool {
        self.in_place.keys().any(|&id| id != self.active)
    }

    /// Avanza `dt_ms` las reproducciones en el lugar de los que no son el activo. Descarta las de
    /// artboards que ya no están o ya no se pueden animar y corrige los rangos que quedaron fuera
    /// de sus frames. Devuelve los milisegundos hasta el próximo cambio de frame (`None` si no
    /// hay nada que animar) y si cambió algún frame.
    pub fn tick_in_place(&mut self, dt_ms: f64) -> (Option<f64>, bool) {
        let stale: Vec<ArtboardId> = self
            .in_place
            .keys()
            .copied()
            .filter(|&id| !self.can_play_in_place(id))
            .collect();
        for id in stale {
            self.in_place.remove(&id);
        }
        let mut next: Option<f64> = None;
        let mut changed = false;
        let active = self.active;
        for (&id, play) in self.in_place.iter_mut() {
            if id == active {
                continue;
            }
            let sprite = &self.document.artboard(id).expect("checked above").sprite;
            let count = sprite.frames.len();
            // Un undo o el timeline le cambiaron los frames o los tags: si su rango ya no es un
            // tag ni todos los frames, se rehace desde el frame que mostraba.
            let valid = sprite
                .tags
                .iter()
                .any(|t| (t.from, t.to) == play.range && t.direction == play.direction)
                || (play.range == (0, count - 1) && play.direction == AnimDirection::Forward);
            if !valid || play.range.1 >= count || play.playback.frame >= count {
                let frame = play.playback.frame.min(count - 1);
                *play = InPlace::starting(sprite, frame);
            }
            play.clock += dt_ms;
            // Tope para no girar sin fin si la app estuvo pausada mucho tiempo.
            for _ in 0..count * 4 {
                let duration = sprite.frames[play.playback.frame].duration_ms.max(1) as f64;
                if play.clock < duration {
                    break;
                }
                play.clock -= duration;
                let before = play.playback.frame;
                play.playback.step(play.range, play.direction);
                changed |= play.playback.frame != before;
            }
            play.clock = play.clock.min(1000.0);
            let duration = sprite.frames[play.playback.frame].duration_ms.max(1) as f64;
            let left = (duration - play.clock).max(0.0);
            next = Some(next.map_or(left, |n: f64| n.min(left)));
        }
        (next, changed)
    }
}
