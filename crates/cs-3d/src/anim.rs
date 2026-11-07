//! La animación de los modelos (como picoCAD 2): bloques Rotate/Move/Scale en la pista de cada
//! objeto o grupo, con su curva, ping pong y repeticiones. Aquí se evalúan: el progreso de un
//! bloque en un tiempo, la pose de cada nodo (lo que se suma a su transform) y los puntos del
//! modelo animado. También los ayudantes del timeline (carriles, ajuste del tiempo).

use std::f32::consts::PI;

use cs_core::{Axis3, Clip, ClipMotion, Curve, Model3D};

use crate::P3;
use crate::edit::to_world;

type M3 = [[f32; 3]; 3];

const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// El valor de `curve` en `f` (de 0 a 1, recortado): 0 en 0 y 1 en 1. Bounce, elastic y back
/// pueden pasarse de 1 en el medio. Es `Curve::value` (también la usan los tweens de objetos).
pub fn curve_value(curve: Curve, f: f32) -> f32 {
    curve.value(f)
}

/// Cuánto hizo el bloque `clip` en el tiempo `t` (ms): 0 antes de empezar; en el bloque, sus
/// `repeat` ciclos (con ping pong cada uno va y vuelve) pasados por la curva; después del final,
/// su último valor.
pub fn clip_progress(clip: &Clip, t: f32) -> f32 {
    let start = clip.start_ms as f32;
    if t < start {
        return 0.0;
    }
    let u = if clip.duration_ms == 0 {
        1.0
    } else {
        ((t - start) / clip.duration_ms as f32).clamp(0.0, 1.0)
    };
    let cycles = clip.repeat.max(1) as f32;
    let f = if u >= 1.0 { 1.0 } else { (u * cycles).fract() };
    let f = if clip.ping_pong {
        if f <= 0.5 { 2.0 * f } else { 2.0 - 2.0 * f }
    } else {
        f
    };
    curve_value(clip.curve, f)
}

/// Lo que la animación le hace a un nodo en un tiempo: `x ↦ origen + offset + m·(x − origen)`,
/// con `x` ya pasado por su transform (en el espacio de su padre) y el origen su posición.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodePose {
    pub m: M3,
    /// En 1/16 de unidad.
    pub offset: P3,
}

impl Default for NodePose {
    fn default() -> Self {
        NodePose {
            m: IDENTITY,
            offset: [0.0; 3],
        }
    }
}

impl NodePose {
    /// `x` (en el espacio del padre) movido por la pose de un nodo con origen `origin`.
    pub fn apply(&self, origin: P3, x: P3) -> P3 {
        let r = [x[0] - origin[0], x[1] - origin[1], x[2] - origin[2]];
        let m = mul_v(&self.m, r);
        [
            origin[0] + self.offset[0] + m[0],
            origin[1] + self.offset[1] + m[1],
            origin[2] + self.offset[2] + m[2],
        ]
    }
}

fn mul(a: &M3, b: &M3) -> M3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

fn mul_v(m: &M3, v: P3) -> P3 {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// La rotación de `angle` radianes alrededor de `axis`, con el sentido de la rotación de los
/// transforms ([`to_world`]).
pub fn axis_rotation(axis: Axis3, angle: f32) -> M3 {
    let (s, c) = angle.sin_cos();
    match axis {
        Axis3::X => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        Axis3::Y => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        Axis3::Z => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// El orden en que se aplican los bloques de una pista: por inicio y, si empiezan a la vez, por
/// cómo están guardados.
pub fn clip_order(clips: &[Clip]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..clips.len()).collect();
    order.sort_by_key(|&i| (clips[i].start_ms, i));
    order
}

/// La pose de un nodo con los bloques `clips` en el tiempo `t` (ms); `None` sin bloques.
pub fn node_pose(clips: &[Clip], t: f32) -> Option<NodePose> {
    if clips.is_empty() {
        return None;
    }
    let mut pose = NodePose::default();
    for i in clip_order(clips) {
        let clip = &clips[i];
        let v = clip_progress(clip, t);
        match clip.motion {
            ClipMotion::Rotate { axis, turns } => {
                let angle = v * turns as f32 / 1000.0 * 2.0 * PI;
                pose.m = mul(&axis_rotation(axis, angle), &pose.m);
            }
            ClipMotion::Move { offset } => {
                pose.offset[0] += v * offset.x as f32;
                pose.offset[1] += v * offset.y as f32;
                pose.offset[2] += v * offset.z as f32;
            }
            ClipMotion::Scale { factor } => {
                let s = |f: i32| 1.0 + v * (f as f32 / 1000.0 - 1.0);
                let scale = [
                    [s(factor.x), 0.0, 0.0],
                    [0.0, s(factor.y), 0.0],
                    [0.0, 0.0, s(factor.z)],
                ];
                pose.m = mul(&scale, &pose.m);
            }
        }
    }
    Some(pose)
}

/// La pose de cada objeto y grupo de un modelo en un tiempo (`None` los que no tienen bloques).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pose {
    pub objects: Vec<Option<NodePose>>,
    pub groups: Vec<Option<NodePose>>,
}

/// La pose de `model` en el tiempo `t` (ms). `None` si no tiene ningún bloque (está quieto).
pub fn pose_at(model: &Model3D, t: f32) -> Option<Pose> {
    let any = model.objects.iter().any(|o| !o.anim.is_empty())
        || model.groups.iter().any(|g| !g.anim.is_empty());
    if !any {
        return None;
    }
    Some(Pose {
        objects: model
            .objects
            .iter()
            .map(|o| node_pose(&o.anim, t))
            .collect(),
        groups: model.groups.iter().map(|g| node_pose(&g.anim, t)).collect(),
    })
}

/// Las transformaciones que llevan un punto local del objeto `o` al mundo con la pose: por cada
/// paso (el objeto y luego sus grupos, de adentro hacia afuera), el transform y su pose.
pub fn posed_chain(
    model: &Model3D,
    pose: Option<&Pose>,
    o: usize,
) -> Vec<(cs_core::Transform3D, Option<NodePose>)> {
    let chain = model.object_chain(o);
    let parents = model.object_parents(o);
    chain
        .into_iter()
        .enumerate()
        .map(|(k, t)| {
            let node = pose.and_then(|p| {
                if k == 0 {
                    p.objects.get(o).copied().flatten()
                } else {
                    parents
                        .get(k - 1)
                        .and_then(|&g| p.groups.get(g).copied().flatten())
                }
            });
            (t, node)
        })
        .collect()
}

/// Un punto por una cadena de [`posed_chain`].
pub fn through_chain(chain: &[(cs_core::Transform3D, Option<NodePose>)], p: P3) -> P3 {
    chain.iter().fold(p, |p, (t, node)| {
        let w = to_world(t, p);
        match node {
            Some(n) => {
                let origin = [
                    t.position.x as f32,
                    t.position.y as f32,
                    t.position.z as f32,
                ];
                n.apply(origin, w)
            }
            None => w,
        }
    })
}

/// Un punto local del objeto `o` en el mundo, con la pose `pose` (sin pose, como
/// [`crate::object_to_world`]).
pub fn posed_to_world(model: &Model3D, pose: Option<&Pose>, o: usize, p: P3) -> P3 {
    through_chain(&posed_chain(model, pose, o), p)
}

/// El carril de cada bloque de una pista para que no se pisen (los de un carril van uno después
/// del otro), y cuántos carriles hacen falta (al menos 1).
pub fn clip_lanes(clips: &[Clip]) -> (Vec<usize>, usize) {
    let mut lanes = vec![0; clips.len()];
    let mut ends: Vec<u32> = Vec::new();
    for i in clip_order(clips) {
        let c = &clips[i];
        match ends.iter().position(|&end| end <= c.start_ms) {
            Some(l) => {
                lanes[i] = l;
                ends[l] = c.end_ms();
            }
            None => {
                lanes[i] = ends.len();
                ends.push(c.end_ms());
            }
        }
    }
    (lanes, ends.len().max(1))
}

/// `t` ajustado al múltiplo de `step` más cercano (con `step` 0, igual).
pub fn snap_ms(t: i64, step: u32) -> i64 {
    if step == 0 {
        return t;
    }
    let s = i64::from(step);
    (t + s / 2).div_euclid(s) * s
}

#[cfg(test)]
#[path = "anim_tests.rs"]
mod tests;
