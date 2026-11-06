//! La animación del modelo en el modo Anim: el tiempo actual y la reproducción (estado de la UI,
//! no se guarda ni pasa por undo) y los bloques de cada pista (cada cambio es un `SetModel` con
//! el modelo entero, un paso de undo). Mover o estirar un bloque con el puntero es un gesto: la
//! vista previa va en `model_edit.preview` y al soltar es un solo paso.

use cs_core::{Axis3, Clip, ClipMotion, Curve, FIXED, Model3D, Vec3i};

use super::{CadMode, Editor, Row};

/// Lo más corto que puede durar un bloque y el paso del ajuste de los arrastres (ms).
pub const MIN_CLIP_MS: u32 = 50;
pub const SNAP_MS: u32 = 50;
/// Lo que puede durar la animación (ms).
pub const ANIM_LENGTH: std::ops::RangeInclusive<u32> = 100..=60_000;
/// Lo que dura un bloque nuevo (si cabe).
const NEW_CLIP_MS: u32 = 1000;

/// Qué parte de un bloque se arrastra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipEdge {
    /// El bloque entero (se mueve).
    Body,
    /// El borde izquierdo (cambia el inicio y la duración; el final se queda).
    Start,
    /// El borde derecho (cambia la duración).
    End,
}

/// Un arrastre de un bloque en curso.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipDrag {
    pub row: Row,
    pub index: usize,
    pub edge: ClipEdge,
    /// El tiempo (ms) bajo el puntero al presionar.
    pub from_ms: f32,
    /// El bloque como estaba.
    pub original: Clip,
}

/// Los bloques que agrega el botón Add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewClip {
    Rotate,
    Move,
    Scale,
    /// Una vuelta en Y, lineal, toda la animación.
    Spin,
    /// Sube un cuarto de unidad y vuelve, suave, en un segundo.
    Bob,
}

impl NewClip {
    pub const ALL: [NewClip; 5] = [
        NewClip::Rotate,
        NewClip::Move,
        NewClip::Scale,
        NewClip::Spin,
        NewClip::Bob,
    ];

    pub fn label(self) -> &'static str {
        match self {
            NewClip::Rotate => "Rotate",
            NewClip::Move => "Move",
            NewClip::Scale => "Scale",
            NewClip::Spin => "Spin",
            NewClip::Bob => "Bob",
        }
    }

    /// El bloque nuevo en el tiempo `at` de una animación de `length` ms.
    pub fn clip(self, at: u32, length: u32) -> Clip {
        let length = length.max(MIN_CLIP_MS);
        let fit = |start: u32, dur: u32| {
            let start = if start + MIN_CLIP_MS > length {
                0
            } else {
                start
            };
            (start, dur.min(length - start).max(MIN_CLIP_MS))
        };
        let (start, dur) = fit(at, NEW_CLIP_MS);
        match self {
            NewClip::Rotate => Clip::new(
                ClipMotion::Rotate {
                    axis: Axis3::Y,
                    turns: 250,
                },
                start,
                dur,
            ),
            NewClip::Move => Clip::new(
                ClipMotion::Move {
                    offset: Vec3i {
                        x: 0,
                        y: FIXED,
                        z: 0,
                    },
                },
                start,
                dur,
            ),
            NewClip::Scale => Clip::new(
                ClipMotion::Scale {
                    factor: Vec3i {
                        x: 1500,
                        y: 1500,
                        z: 1500,
                    },
                },
                start,
                dur,
            ),
            NewClip::Spin => Clip::new(
                ClipMotion::Rotate {
                    axis: Axis3::Y,
                    turns: 1000,
                },
                0,
                length,
            ),
            NewClip::Bob => Clip {
                curve: Curve::EaseInOut,
                ping_pong: true,
                ..Clip::new(
                    ClipMotion::Move {
                        offset: Vec3i {
                            x: 0,
                            y: FIXED / 4,
                            z: 0,
                        },
                    },
                    start,
                    dur,
                )
            },
        }
    }
}

/// El estado del modo Anim (de la UI): el tiempo actual, la reproducción y el bloque elegido.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimView {
    /// El tiempo actual (ms).
    pub time_ms: u32,
    pub playing: bool,
    /// Al llegar al final vuelve a empezar (si no, se detiene).
    pub looping: bool,
    /// El bloque elegido: su pista y su índice en ella.
    pub clip: Option<(Row, usize)>,
    /// Desde cuándo se reproduce: el instante (s del reloj de la UI) y el tiempo de entonces.
    /// `None` hasta el primer `tick_anim`.
    pub clock: Option<(f64, u32)>,
    pub drag: Option<ClipDrag>,
}

impl Default for AnimView {
    fn default() -> Self {
        AnimView {
            time_ms: 0,
            playing: false,
            looping: true,
            clip: None,
            clock: None,
            drag: None,
        }
    }
}

/// Los bloques de la pista `row` de `model`.
pub fn row_clips(model: &Model3D, row: Row) -> Option<&Vec<Clip>> {
    match row {
        Row::Object(o) => model.objects.get(o).map(|x| &x.anim),
        Row::Group(g) => model.groups.get(g).map(|x| &x.anim),
    }
}

fn row_clips_mut(model: &mut Model3D, row: Row) -> Option<&mut Vec<Clip>> {
    match row {
        Row::Object(o) => model.objects.get_mut(o).map(|x| &mut x.anim),
        Row::Group(g) => model.groups.get_mut(g).map(|x| &mut x.anim),
    }
}

/// `clip` arreglado para que valga: dura al menos 1 ms y se repite al menos una vez.
fn valid(mut clip: Clip) -> Clip {
    clip.duration_ms = clip.duration_ms.max(1);
    clip.repeat = clip.repeat.clamp(1, 99);
    clip
}

impl Editor {
    pub fn anim(&self) -> &AnimView {
        &self.model_edit.anim
    }

    /// Lo que dura la animación del modelo que se edita (ms).
    pub fn anim_length(&self) -> u32 {
        self.edit_model_shown()
            .map_or(cs_core::DEFAULT_ANIM_MS, |m| m.anim_ms)
    }

    /// El tiempo en que se ve el modelo en las vistas: el actual en modo Anim; en reposo en los
    /// otros.
    pub fn anim_time(&self) -> Option<u32> {
        (self.model_edit.mode == CadMode::Anim).then_some(self.model_edit.anim.time_ms)
    }

    /// Va al tiempo `t` (recortado a la animación) y pausa.
    pub fn set_anim_time(&mut self, t: u32) {
        let length = self.anim_length();
        let a = &mut self.model_edit.anim;
        a.playing = false;
        a.clock = None;
        a.time_ms = t.min(length);
    }

    /// Reproduce o pausa. Al final y sin loop, reproducir vuelve a empezar.
    pub fn toggle_anim_play(&mut self) {
        let length = self.anim_length();
        let a = &mut self.model_edit.anim;
        a.clock = None;
        if a.playing {
            a.playing = false;
        } else {
            if a.time_ms >= length {
                a.time_ms = 0;
            }
            a.playing = true;
        }
    }

    pub fn set_anim_looping(&mut self, looping: bool) {
        self.model_edit.anim.looping = looping;
    }

    /// Avanza la reproducción al instante `now` (s del reloj de la UI). Devuelve si sigue
    /// reproduciendo (hay que volver a pintar).
    pub fn tick_anim(&mut self, now: f64) -> bool {
        let length = self.anim_length().max(1);
        let a = &mut self.model_edit.anim;
        if !a.playing {
            return false;
        }
        let (since, from) = *a.clock.get_or_insert((now, a.time_ms));
        let t = from as f64 + (now - since).max(0.0) * 1000.0;
        if t >= length as f64 {
            if a.looping {
                let t = (t as u64 % u64::from(length)) as u32;
                a.time_ms = t;
                a.clock = Some((now, t));
            } else {
                a.time_ms = length;
                a.playing = false;
                a.clock = None;
                return false;
            }
        } else {
            a.time_ms = t as u32;
        }
        true
    }

    /// La duración de la animación (recortada a [`ANIM_LENGTH`]). Un paso de undo. El tiempo
    /// actual se recorta.
    pub fn set_anim_length(&mut self, ms: u32) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        model.anim_ms = ms.clamp(*ANIM_LENGTH.start(), *ANIM_LENGTH.end());
        let done = self.named("Animation length", |e| e.set_model(model));
        let length = self.anim_length();
        let a = &mut self.model_edit.anim;
        a.time_ms = a.time_ms.min(length);
        done
    }

    /// El bloque elegido, si todavía existe.
    pub fn selected_clip(&self) -> Option<(Row, usize, Clip)> {
        let (row, i) = self.model_edit.anim.clip?;
        let clip = *row_clips(self.edit_model_shown()?, row)?.get(i)?;
        Some((row, i, clip))
    }

    /// Elige el bloque `i` de la pista `row` (y la pista como fila activa); `None` no elige
    /// ninguno.
    pub fn select_clip(&mut self, clip: Option<(Row, usize)>) {
        self.model_edit.anim.clip = clip;
        if let Some((row, _)) = clip
            && self.active_row() != Some(row)
        {
            self.set_active_row(Some(row), false);
        }
        self.revision += 1;
    }

    /// Agrega un bloque `kind` a la pista activa, en el tiempo actual, y lo elige. Un paso de
    /// undo. `false` sin pista activa.
    pub fn add_clip(&mut self, kind: NewClip) -> bool {
        let Some(row) = self.active_row() else {
            return false;
        };
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let clip = kind.clip(
            cs_3d::snap_ms(i64::from(self.model_edit.anim.time_ms), SNAP_MS) as u32,
            model.anim_ms,
        );
        let Some(clips) = row_clips_mut(&mut model, row) else {
            return false;
        };
        clips.push(clip);
        let index = clips.len() - 1;
        if !self.named("Add block", |e| e.set_model(model)) {
            return false;
        }
        self.select_clip(Some((row, index)));
        true
    }

    /// Cambia el bloque `i` de la pista `row`. Un paso de undo.
    pub fn set_clip(&mut self, row: Row, i: usize, clip: Clip) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let Some(slot) = row_clips_mut(&mut model, row).and_then(|c| c.get_mut(i)) else {
            return false;
        };
        *slot = valid(clip);
        self.named("Edit block", |e| e.set_model(model))
    }

    /// Borra el bloque elegido. Un paso de undo.
    pub fn delete_clip(&mut self) -> bool {
        let Some((row, i, _)) = self.selected_clip() else {
            return false;
        };
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        if let Some(clips) = row_clips_mut(&mut model, row) {
            clips.remove(i);
        }
        if !self.named("Delete block", |e| e.set_model(model)) {
            return false;
        }
        self.model_edit.anim.clip = None;
        true
    }

    /// Empieza a arrastrar `edge` del bloque `i` de la pista `row`, agarrado en el tiempo `at`
    /// (ms). El bloque queda elegido.
    pub fn start_clip_drag(&mut self, row: Row, i: usize, edge: ClipEdge, at: f32) -> bool {
        let Some(original) = self
            .edited_model()
            .and_then(|m| row_clips(m, row))
            .and_then(|c| c.get(i))
            .copied()
        else {
            return false;
        };
        self.cancel_model_op();
        self.select_clip(Some((row, i)));
        self.model_edit.anim.drag = Some(ClipDrag {
            row,
            index: i,
            edge,
            from_ms: at,
            original,
        });
        true
    }

    /// El arrastre en curso con el puntero en el tiempo `at` (ms), ajustado a `step` ms: la
    /// vista previa del bloque movido o estirado, dentro de la animación.
    pub fn drag_clip_to(&mut self, at: f32, step: u32) {
        let Some(d) = self.model_edit.anim.drag else {
            return;
        };
        let Some(mut model) = self.edited_model().cloned() else {
            return;
        };
        let length = i64::from(model.anim_ms);
        let o = d.original;
        let delta = (at - d.from_ms).round() as i64;
        let (start, end) = (i64::from(o.start_ms), i64::from(o.end_ms()));
        let min = i64::from(MIN_CLIP_MS);
        let (new_start, new_end) = match d.edge {
            ClipEdge::Body => {
                let dur = end - start;
                let s = cs_3d::snap_ms(start + delta, step).clamp(0, (length - dur).max(0));
                (s, s + dur)
            }
            ClipEdge::Start => {
                let s = cs_3d::snap_ms(start + delta, step).clamp(0, (end - min).max(0));
                (s, end)
            }
            ClipEdge::End => {
                let e =
                    cs_3d::snap_ms(end + delta, step).clamp(start + min, length.max(start + min));
                (start, e)
            }
        };
        let Some(slot) = row_clips_mut(&mut model, d.row).and_then(|c| c.get_mut(d.index)) else {
            return;
        };
        slot.start_ms = new_start as u32;
        slot.duration_ms = (new_end - new_start).max(1) as u32;
        self.model_edit.preview = Some(model);
        self.revision += 1;
    }

    /// Suelta el arrastre: si cambió, un paso de undo.
    pub fn end_clip_drag(&mut self) -> bool {
        let Some(d) = self.model_edit.anim.drag.take() else {
            return false;
        };
        let Some(model) = self.model_edit.preview.take() else {
            return false;
        };
        let label = match d.edge {
            ClipEdge::Body => "Move block",
            ClipEdge::Start | ClipEdge::End => "Resize block",
        };
        self.named(label, |e| e.set_model(model))
    }

    /// Cancela el arrastre sin tocar el documento. `false` si no había.
    pub fn cancel_clip_drag(&mut self) -> bool {
        if self.model_edit.anim.drag.take().is_none() {
            return false;
        }
        self.model_edit.preview = None;
        self.revision += 1;
        true
    }

    /// Las teclas del modo Anim: Enter reproduce o pausa, Esc cancela el arrastre (o deja sin
    /// bloque elegido), Supr/Retroceso borra el bloque elegido y las flechas izquierda y derecha
    /// mueven el tiempo 100 ms (Shift: 10). Devuelve si la usó.
    pub fn anim_key(&mut self, key: eframe::egui::Key, shift: bool) -> bool {
        use eframe::egui::Key;
        let step = if shift { 10 } else { 100 };
        match key {
            Key::Enter => self.toggle_anim_play(),
            Key::Escape => {
                if !self.cancel_clip_drag() {
                    self.select_clip(None);
                }
            }
            Key::Delete | Key::Backspace => {
                self.delete_clip();
            }
            Key::ArrowLeft => self.set_anim_time(self.model_edit.anim.time_ms.saturating_sub(step)),
            Key::ArrowRight => self.set_anim_time(self.model_edit.anim.time_ms + step),
            _ => return false,
        }
        true
    }
}
