//! Tweens de objetos (el Classic Tween de Flash): entre una key con `tween` y la siguiente, los
//! frames de en medio se calculan solos (posición, escala, rotación, opacidad y tinte) con un ease.

use serde::{Deserialize, Serialize};

use crate::{Curve, InstanceKey, InstanceTrack, Rgba, UNIT_SCALE};

/// Tinte de Flash (Color Effect → Tint): el color del símbolo se mezcla hacia `color`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tint {
    /// El alfa no se usa.
    pub color: Rgba,
    /// Cuánto se mezcla: 0 nada, 255 todo (como la opacidad de la app).
    pub amount: u8,
}

/// Cómo avanza un tween de 0 a 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ease {
    /// El ease de Flash: −100 (ease in, acelera) … 0 (lineal) … 100 (ease out, frena).
    Classic(i8),
    /// Una curva del modo Anim de ChimpCAD con intensidad 0..=100 (0 = lineal).
    Curve { curve: Curve, amount: u8 },
}

impl Default for Ease {
    fn default() -> Self {
        Ease::Classic(0)
    }
}

impl Ease {
    /// El progreso en `t` (0 a 1, recortado). Vale 0 en 0 y 1 en 1; bounce, elastic y back pueden
    /// pasarse en el medio.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            // Cuadrático, como Flash: con 100 es 2t − t², con −100 es t².
            Ease::Classic(e) => {
                let e = (e as f32 / 100.0).clamp(-1.0, 1.0);
                t + e * t * (1.0 - t)
            }
            Ease::Curve { curve, amount } => {
                let a = amount.min(100) as f32 / 100.0;
                t + (curve.value(t) - t) * a
            }
        }
    }
}

/// Classic tween de una key a la siguiente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tween {
    #[serde(default)]
    pub ease: Ease,
}

/// Lo que se ve de un objeto en un frame: la key vigente o, en un tween, los valores calculados.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstancePose {
    /// La key vigente: la fuente, los flips y el tiempo del símbolo salen de ella.
    pub key: InstanceKey,
    /// Esquina superior izquierda del símbolo sin transformar, en píxeles del padre.
    pub position: (i32, i32),
    /// 1.0 = 100 %.
    pub scale: (f32, f32),
    /// Grados, horaria.
    pub rotation: f32,
    pub opacity: u8,
    pub tint: Option<Tint>,
}

impl InstancePose {
    /// La pose de una key sola (sin tween).
    pub fn of(key: InstanceKey) -> Self {
        Self {
            key,
            position: key.position,
            scale: (
                key.scale.0 as f32 / UNIT_SCALE as f32,
                key.scale.1 as f32 / UNIT_SCALE as f32,
            ),
            rotation: key.rotation as f32,
            opacity: key.opacity,
            tint: key.tint.filter(|t| t.amount > 0),
        }
    }

    /// ¿Escala o rota? (Los flips no cuentan: se aplican aparte.)
    pub fn is_transformed(&self) -> bool {
        self.scale != (1.0, 1.0) || self.rotation.rem_euclid(360.0) != 0.0
    }

    /// Como `InstanceKey::is_plain`: sin transformación ni flips.
    pub fn is_plain(&self) -> bool {
        !self.is_transformed() && !self.key.flip_h && !self.key.flip_v
    }
}

/// Un tramo con tween: de la key que empieza en `start` a la siguiente (`end`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TweenSpan {
    pub start: usize,
    /// El frame de la key siguiente (`None` si no hay).
    pub end: Option<usize>,
    /// Sin key siguiente, o la siguiente muestra otro símbolo (o nada): no se interpola.
    pub broken: bool,
}

/// ¿El tween de `a` llega a `b`? Las dos tienen que mostrar el mismo símbolo.
fn tweens_to(a: &InstanceKey, b: &InstanceKey) -> bool {
    a.tween.is_some() && a.source.is_some() && a.source == b.source && b.frame > a.frame
}

fn lerp(a: f32, b: f32, p: f32) -> f32 {
    a + (b - a) * p
}

/// El tinte entre `a` y `b` en `p`: sin tinte cuenta como el color del otro con cantidad 0.
fn lerp_tint(a: Option<Tint>, b: Option<Tint>, p: f32) -> Option<Tint> {
    let (a, b) = match (a, b) {
        (None, None) => return None,
        (Some(a), None) => (a, Tint { amount: 0, ..a }),
        (None, Some(b)) => (Tint { amount: 0, ..b }, b),
        (Some(a), Some(b)) => (a, b),
    };
    let channel = |x: u8, y: u8| lerp(x as f32, y as f32, p).round().clamp(0.0, 255.0) as u8;
    let tint = Tint {
        color: Rgba::rgb(
            channel(a.color.r, b.color.r),
            channel(a.color.g, b.color.g),
            channel(a.color.b, b.color.b),
        ),
        amount: channel(a.amount, b.amount),
    };
    (tint.amount > 0).then_some(tint)
}

impl InstanceTrack {
    /// Índice de la key vigente en `frame`.
    fn index_at(&self, frame: usize) -> Option<usize> {
        self.keys.iter().rposition(|k| k.frame <= frame)
    }

    /// El tramo con tween que contiene a `frame` (desde su key de inicio, incluida, hasta la
    /// siguiente, sin incluir). `None` si la key vigente no tiene tween.
    pub fn tween_span(&self, frame: usize) -> Option<TweenSpan> {
        let i = self.index_at(frame)?;
        let key = &self.keys[i];
        key.tween?;
        let next = self.keys.get(i + 1);
        Some(TweenSpan {
            start: key.frame,
            end: next.map(|k| k.frame),
            broken: !next.is_some_and(|n| tweens_to(key, n)),
        })
    }

    /// Lo que se ve en `frame`: la key vigente o, dentro de un tween, los valores interpolados
    /// (la posición redondeada al píxel). `None` antes de la primera key.
    pub fn pose_at(&self, frame: usize) -> Option<InstancePose> {
        let i = self.index_at(frame)?;
        let a = self.keys[i];
        let mut pose = InstancePose::of(a);
        let Some(b) = self.keys.get(i + 1).filter(|b| tweens_to(&a, b)) else {
            return Some(pose);
        };
        if frame == a.frame {
            return Some(pose);
        }
        let ease = a.tween.expect("tweens_to").ease;
        let p = ease.apply((frame - a.frame) as f32 / (b.frame - a.frame) as f32);
        let to = InstancePose::of(*b);
        let round = |x: i32, y: i32| lerp(x as f32, y as f32, p).round() as i32;
        pose.position = (
            round(a.position.0, b.position.0),
            round(a.position.1, b.position.1),
        );
        pose.scale = (
            lerp(pose.scale.0, to.scale.0, p),
            lerp(pose.scale.1, to.scale.1, p),
        );
        pose.rotation = lerp(pose.rotation, to.rotation, p);
        pose.opacity = lerp(a.opacity as f32, b.opacity as f32, p)
            .round()
            .clamp(0.0, 255.0) as u8;
        pose.tint = lerp_tint(a.tint, b.tint, p);
        Some(pose)
    }

    /// La key que tendría `frame` si se volviera keyframe: la que empieza ahí, o la vigente
    /// movida a `frame` con los valores que se ven (en un tween, los interpolados; el tween
    /// sigue, así el tramo partido pasa por los mismos lugares). `None` antes de la primera key.
    pub fn keyframe_at(&self, frame: usize) -> Option<InstanceKey> {
        let pose = self.pose_at(frame)?;
        let key = pose.key;
        if key.frame == frame || pose == InstancePose::of(key) {
            return Some(InstanceKey { frame, ..key });
        }
        let milli = |s: f32| ((s * UNIT_SCALE as f32).round() as i32).max(1);
        Some(InstanceKey {
            frame,
            position: pose.position,
            scale: (milli(pose.scale.0), milli(pose.scale.1)),
            rotation: pose.rotation.round() as i32,
            opacity: pose.opacity,
            tint: pose.tint,
            ..key
        })
    }
}

#[cfg(test)]
#[path = "tween_tests.rs"]
mod tests;
