//! Los gizmos de mover, rotar y escalar (como Blender): girar lo elegido alrededor de un eje del
//! mundo, la geometría del gizmo en el mundo y qué asa queda bajo el puntero en pantalla.
//!
//! Sentido de giro: el mismo que [`to_world`] (mano derecha). Un ángulo positivo alrededor de
//! X lleva +Y hacia +Z; alrededor de Y, +Z hacia +X; alrededor de Z, +X hacia +Y.

use cs_core::{Model3D, Transform3D, Vec3i};

use crate::{
    P3, SelectMode, Selection, dist_to_segment, edit_vertices, movable_objects, p3,
    parent_to_local, parent_to_world, round3,
};

/// `p` girado `tenths` décimas de grado alrededor del eje `axis` (0 x, 1 y, 2 z) del mundo que
/// pasa por `center`.
pub fn rotate_point(p: P3, center: P3, axis: usize, tenths: i32) -> P3 {
    let (s, c) = (tenths as f32 / 10.0).to_radians().sin_cos();
    let (x, y, z) = (p[0] - center[0], p[1] - center[1], p[2] - center[2]);
    let (x, y, z) = match axis {
        0 => (x, y * c - z * s, y * s + z * c),
        1 => (x * c + z * s, y, -x * s + z * c),
        _ => (x * c - y * s, x * s + y * c, z),
    };
    [x + center[0], y + center[1], z + center[2]]
}

type M3 = [[f64; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
}

/// La matriz de girar `tenths` alrededor del eje `axis` (el mismo sentido que [`rotate_point`]).
fn axis_matrix(axis: usize, tenths: i32) -> M3 {
    let (s, c) = (tenths as f64 / 10.0).to_radians().sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// Radianes → décimas de grado, entre -179.9° y 180°.
fn tenths_of(rad: f64) -> i32 {
    let v = (rad.to_degrees() * 10.0).round() as i32;
    if v <= -1800 { v + 3600 } else { v }
}

/// La rotación del objeto girada además `tenths` alrededor del eje `axis` del mundo (por su
/// origen), otra vez como ángulos X, Y, Z en décimas de grado (redondeados).
///
/// Como en [`to_world`](crate::to_world), la matriz del objeto es `Rz · Ry · Rx`; la nueva es
/// `R(axis) · Rz · Ry · Rx` y se separa otra vez en X, Y, Z (con Y entre -90° y 90°). En el
/// bloqueo (Y = ±90°, donde X y Z giran lo mismo) X queda en 0 y todo el giro va a Z.
pub fn rotate_object_world(t: &Transform3D, axis: usize, tenths: i32) -> Transform3D {
    if tenths == 0 {
        return *t;
    }
    let r = t.rotation;
    let m = mul(
        &axis_matrix(axis, tenths),
        &mul(
            &axis_matrix(2, r.z),
            &mul(&axis_matrix(1, r.y), &axis_matrix(0, r.x)),
        ),
    );
    let y = (-m[2][0]).clamp(-1.0, 1.0).asin();
    let (x, z) = if y.cos() < 1e-4 {
        (0.0, (-m[0][1]).atan2(m[1][1]))
    } else {
        (m[2][1].atan2(m[2][2]), m[1][0].atan2(m[0][0]))
    };
    Transform3D {
        rotation: Vec3i {
            x: tenths_of(x),
            y: tenths_of(y),
            z: tenths_of(z),
        },
        ..*t
    }
}

/// `model` con lo elegido girado `tenths` alrededor del eje `axis` del mundo que pasa por
/// `center`. Lo bloqueado no gira. En modo Objeto el origen gira en el mundo, pero el giro propio
/// se suma a la rotación del objeto como si su padre fuera el mundo: aproximado si sus grupos
/// están girados.
pub fn rotate_selection(
    model: &Model3D,
    sel: &Selection,
    center: P3,
    axis: usize,
    tenths: i32,
) -> Model3D {
    let mut out = model.clone();
    if tenths == 0 {
        return out;
    }
    if sel.mode == SelectMode::Object {
        // El objeto gira por su origen; su origen gira alrededor del centro.
        for o in movable_objects(sel, model) {
            // El origen gira en el mundo; su posición vive en el espacio de sus grupos.
            let origin = parent_to_world(model, o, p3(model.objects[o].transform.position));
            let pos = round3(parent_to_local(
                model,
                o,
                rotate_point(origin, center, axis, tenths),
            ));
            let t = &mut out.objects[o].transform;
            *t = rotate_object_world(t, axis, tenths);
            t.position = pos;
        }
        return out;
    }
    edit_vertices(&mut out, model, sel, |w| {
        rotate_point(w, center, axis, tenths)
    });
    out
}

/// Qué gizmo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GizmoKind {
    Move,
    Rotate,
    Scale,
}

/// Un asa del gizmo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GizmoPart {
    /// La flecha de un eje (mover en ese eje).
    Axis(usize),
    /// El cuadrito del centro (mover en el plano de la vista).
    Plane,
    /// El aro de un eje (girar alrededor de ese eje).
    Ring(usize),
    /// El cubito en la punta de un eje (escalar en ese eje).
    ScaleAxis(usize),
    /// El cubito del centro (escala pareja).
    ScaleUniform,
}

/// Cuántos puntos tiene cada aro.
pub const RING_POINTS: usize = 32;

/// La mitad del lado (en puntos de pantalla) del cuadrito y de los cubitos.
pub const GIZMO_BOX: f32 = 2.0;

/// La forma de un asa en el mundo.
#[derive(Clone, Debug, PartialEq)]
pub enum HandleGeom {
    /// Una flecha: el asta de `from` a `tip` y la base de la punta (4 puntos alrededor del eje).
    Arrow { from: P3, tip: P3, base: [P3; 4] },
    /// Un aro cerrado.
    Ring(Vec<P3>),
    /// Un eje de `from` a `tip` con un cubito en la punta.
    Stick { from: P3, tip: P3 },
    /// Un cuadrito o cubito en pantalla, centrado en el punto.
    Box(P3),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GizmoHandle {
    pub part: GizmoPart,
    pub geom: HandleGeom,
}

/// El gizmo entero: sus asas, del centro hacia afuera.
#[derive(Clone, Debug, PartialEq)]
pub struct GizmoShape {
    pub handles: Vec<GizmoHandle>,
}

/// El gizmo `kind` en `center`, de `size` (en el mundo: el largo de las flechas y el radio de
/// los aros), con solo los ejes de `axes`.
pub fn gizmo_shape(kind: GizmoKind, center: P3, size: f32, axes: [bool; 3]) -> GizmoShape {
    let at = |from: P3, axis: usize, d: f32| {
        let mut p = from;
        p[axis] += d;
        p
    };
    let mut handles = Vec::new();
    match kind {
        GizmoKind::Move => handles.push(GizmoHandle {
            part: GizmoPart::Plane,
            geom: HandleGeom::Box(center),
        }),
        GizmoKind::Scale => handles.push(GizmoHandle {
            part: GizmoPart::ScaleUniform,
            geom: HandleGeom::Box(center),
        }),
        GizmoKind::Rotate => {}
    }
    for axis in (0..3).filter(|&a| axes[a]) {
        // Los otros dos ejes, en el orden del giro positivo (de `b` hacia `c`).
        let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
        let tip = at(center, axis, size);
        let (part, geom) = match kind {
            GizmoKind::Move => {
                let mid = at(center, axis, size * 0.75);
                let w = size * 0.08;
                let base = [at(mid, b, w), at(mid, c, w), at(mid, b, -w), at(mid, c, -w)];
                (
                    GizmoPart::Axis(axis),
                    HandleGeom::Arrow {
                        from: center,
                        tip,
                        base,
                    },
                )
            }
            GizmoKind::Rotate => {
                let pts = (0..RING_POINTS)
                    .map(|i| {
                        let a = i as f32 / RING_POINTS as f32 * std::f32::consts::TAU;
                        at(at(center, b, size * a.cos()), c, size * a.sin())
                    })
                    .collect();
                (GizmoPart::Ring(axis), HandleGeom::Ring(pts))
            }
            GizmoKind::Scale => (
                GizmoPart::ScaleAxis(axis),
                HandleGeom::Stick { from: center, tip },
            ),
        };
        handles.push(GizmoHandle { part, geom });
    }
    GizmoShape { handles }
}

/// La distancia (en pantalla) de `at` al cuadrito de [`GIZMO_BOX`] centrado en `s` (0 dentro).
fn box_dist(at: (f32, f32), s: (f32, f32)) -> f32 {
    ((at.0 - s.0).abs().max((at.1 - s.1).abs()) - GIZMO_BOX).max(0.0)
}

impl GizmoShape {
    /// El asa a `tol` puntos o menos de `at` (pantalla), la más cercana; `project` lleva el
    /// mundo a la pantalla.
    pub fn hit(
        &self,
        project: impl Fn(P3) -> (f32, f32),
        at: (f32, f32),
        tol: f32,
    ) -> Option<GizmoPart> {
        let mut out: Option<(GizmoPart, f32)> = None;
        for h in &self.handles {
            let d = match &h.geom {
                HandleGeom::Arrow { from, tip, .. } => {
                    dist_to_segment(at, project(*from), project(*tip))
                }
                HandleGeom::Ring(pts) => {
                    let s: Vec<(f32, f32)> = pts.iter().map(|&p| project(p)).collect();
                    (0..s.len())
                        .map(|i| dist_to_segment(at, s[i], s[(i + 1) % s.len()]))
                        .fold(f32::INFINITY, f32::min)
                }
                HandleGeom::Stick { from, tip } => {
                    let t = project(*tip);
                    dist_to_segment(at, project(*from), t).min(box_dist(at, t))
                }
                HandleGeom::Box(p) => box_dist(at, project(*p)),
            };
            // A la misma distancia gana la de antes (el centro va primero).
            if d <= tol && out.is_none_or(|(_, best)| d < best) {
                out = Some((h.part, d));
            }
        }
        out.map(|(part, _)| part)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Item, PICK_RADIUS, to_world};
    use cs_core::{Face, Mesh, Object3D};

    fn close(a: P3, b: P3, eps: f32) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() <= eps)
    }

    fn one_vertex(v: Vec3i) -> Model3D {
        Model3D {
            objects: vec![Object3D {
                name: "P".into(),
                mesh: Mesh {
                    vertices: vec![v],
                    faces: vec![Face {
                        verts: vec![0],
                        uvs: vec![(0, 0)],
                        part: None,
                    }],
                },
                transform: Transform3D::default(),
                double_sided: false,
                kind: None,
                hidden: false,
                locked: false,
            }],
            ..Model3D::default()
        }
    }

    fn sel(mode: SelectMode, items: &[Item]) -> Selection {
        Selection {
            mode,
            items: items.iter().copied().collect(),
        }
    }

    #[test]
    fn el_sentido_de_giro_es_el_de_to_world() {
        // Girar el punto en el mundo = la rotación del objeto de to_world.
        for axis in 0..3 {
            let mut t = Transform3D::default();
            match axis {
                0 => t.rotation.x = 300,
                1 => t.rotation.y = 300,
                _ => t.rotation.z = 300,
            }
            let p = [3.0, 5.0, 7.0];
            assert!(close(
                rotate_point(p, [0.0; 3], axis, 300),
                to_world(&t, p),
                1e-4
            ));
        }
        assert!(close(
            rotate_point([16.0, 0.0, 0.0], [0.0; 3], 2, 900),
            [0.0, 16.0, 0.0],
            1e-4
        ));
        assert!(close(
            rotate_point([0.0, 16.0, 0.0], [0.0; 3], 0, 900),
            [0.0, 0.0, 16.0],
            1e-4
        ));
        assert!(close(
            rotate_point([0.0, 0.0, 16.0], [0.0; 3], 1, 900),
            [16.0, 0.0, 0.0],
            1e-4
        ));
        // Por un centro que no es el origen.
        assert!(close(
            rotate_point([20.0, 4.0, 0.0], [4.0, 4.0, 0.0], 2, 900),
            [4.0, 20.0, 0.0],
            1e-4
        ));
    }

    #[test]
    fn girar_un_vertice_90_en_z_y_cuatro_veces_vuelve() {
        let m = one_vertex(Vec3i { x: 16, y: 0, z: 0 });
        let s = sel(SelectMode::Vertex, &[Item::Vertex(0, 0)]);
        let r = rotate_selection(&m, &s, [0.0; 3], 2, 900);
        assert_eq!(r.objects[0].mesh.vertices[0], Vec3i { x: 0, y: 16, z: 0 });
        let mut m4 = m.clone();
        for _ in 0..4 {
            m4 = rotate_selection(&m4, &s, [0.0; 3], 2, 900);
        }
        assert_eq!(m4, m);
        // Un objeto girado: el vértice gira en el mundo, no en lo local.
        let mut turned = one_vertex(Vec3i { x: 16, y: 0, z: 0 });
        turned.objects[0].transform.rotation.y = 900;
        let before = to_world(&turned.objects[0].transform, [16.0, 0.0, 0.0]);
        let r = rotate_selection(&turned, &s, [0.0; 3], 0, 900);
        let v = r.objects[0].mesh.vertices[0];
        let after = to_world(
            &r.objects[0].transform,
            [v.x as f32, v.y as f32, v.z as f32],
        );
        assert!(
            close(after, rotate_point(before, [0.0; 3], 0, 900), 1e-3),
            "{after:?}"
        );
    }

    #[test]
    fn rotacion_de_objeto_en_ejes_del_mundo() {
        let t = Transform3D {
            rotation: Vec3i { x: 0, y: 450, z: 0 },
            ..Transform3D::default()
        };
        let r = rotate_object_world(&t, 0, 900);
        assert_eq!(r.position, t.position);
        assert_eq!(r.scale, t.scale);
        for p in [
            [16.0, 0.0, 0.0],
            [0.0, 16.0, 0.0],
            [0.0, 0.0, 16.0],
            [5.0, -7.0, 11.0],
        ] {
            let want = rotate_point(to_world(&t, p), [0.0; 3], 0, 900);
            assert!(close(to_world(&r, p), want, 0.05), "{p:?}: {r:?}");
        }
        // Varias combinaciones, cerca y lejos del bloqueo (Y = ±90°).
        for (rot, axis, ang) in [
            ((300, 200, -100), 1, 450),
            ((0, 900, 0), 2, 300),
            ((100, -850, 400), 0, -1200),
            ((0, 0, 0), 1, 900),
            ((1800, 0, 0), 2, 150),
        ] {
            let t = Transform3D {
                position: Vec3i { x: 3, y: -2, z: 9 },
                rotation: Vec3i {
                    x: rot.0,
                    y: rot.1,
                    z: rot.2,
                },
                scale: Vec3i {
                    x: 1000,
                    y: 2000,
                    z: 500,
                },
            };
            let r = rotate_object_world(&t, axis, ang);
            for p in [
                [16.0, 0.0, 0.0],
                [0.0, 16.0, 0.0],
                [0.0, 0.0, 16.0],
                [5.0, -7.0, 11.0],
            ] {
                let want = rotate_point(to_world(&t, p), [3.0, -2.0, 9.0], axis, ang);
                assert!(close(to_world(&r, p), want, 0.15), "{rot:?} {p:?}: {r:?}");
            }
        }
        // Sin bloqueo raro: girar 90° en Y desde cero da Y = 90°.
        let r = rotate_object_world(&Transform3D::default(), 1, 900);
        assert_eq!(r.rotation, Vec3i { x: 0, y: 900, z: 0 });
    }

    #[test]
    fn girar_objetos_mueve_su_posicion_alrededor_del_centro() {
        let mut m = one_vertex(Vec3i { x: 16, y: 0, z: 0 });
        m.objects[0].transform.position = Vec3i { x: 32, y: 0, z: 0 };
        let s = sel(SelectMode::Object, &[Item::Object(0)]);
        let r = rotate_selection(&m, &s, [0.0; 3], 2, 900);
        let t = r.objects[0].transform;
        assert_eq!(t.position, Vec3i { x: 0, y: 32, z: 0 });
        assert_eq!(t.rotation, Vec3i { x: 0, y: 0, z: 900 });
        // La malla no cambia.
        assert_eq!(r.objects[0].mesh, m.objects[0].mesh);
        assert!(close(
            to_world(&t, [16.0, 0.0, 0.0]),
            [0.0, 48.0, 0.0],
            1e-3
        ));
    }

    /// Como la vista Front: X a la derecha, Y arriba, 2 puntos por 1/16, el origen en (100, 100).
    fn front(p: P3) -> (f32, f32) {
        (100.0 + p[0] * 2.0, 100.0 - p[1] * 2.0)
    }

    #[test]
    fn la_forma_del_gizmo() {
        let g = gizmo_shape(GizmoKind::Move, [1.0, 2.0, 3.0], 16.0, [true; 3]);
        assert_eq!(g.handles.len(), 4);
        assert_eq!(g.handles[0].part, GizmoPart::Plane);
        match &g.handles[1].geom {
            HandleGeom::Arrow { from, tip, base } => {
                assert_eq!(*from, [1.0, 2.0, 3.0]);
                assert_eq!(*tip, [17.0, 2.0, 3.0]);
                assert!(base.iter().all(|b| b[0] > 1.0 && b[0] < 17.0));
            }
            g => panic!("{g:?}"),
        }
        let g = gizmo_shape(GizmoKind::Rotate, [0.0; 3], 16.0, [false, false, true]);
        assert_eq!(g.handles.len(), 1);
        match &g.handles[0].geom {
            HandleGeom::Ring(pts) => {
                assert_eq!(pts.len(), RING_POINTS);
                for p in pts {
                    assert!((p[0].hypot(p[1]) - 16.0).abs() < 1e-3 && p[2] == 0.0);
                }
            }
            g => panic!("{g:?}"),
        }
        let g = gizmo_shape(GizmoKind::Scale, [0.0; 3], 16.0, [true, true, false]);
        let parts: Vec<GizmoPart> = g.handles.iter().map(|h| h.part).collect();
        assert_eq!(
            parts,
            [
                GizmoPart::ScaleUniform,
                GizmoPart::ScaleAxis(0),
                GizmoPart::ScaleAxis(1)
            ]
        );
    }

    #[test]
    fn hit_de_flecha() {
        let g = gizmo_shape(GizmoKind::Move, [0.0; 3], 16.0, [true; 3]);
        let hit = |x, y| g.hit(front, (x, y), PICK_RADIUS);
        // La flecha X va de (100, 100) a (132, 100); la Y, de (100, 100) a (100, 68).
        assert_eq!(hit(120.0, 103.0), Some(GizmoPart::Axis(0)));
        assert_eq!(hit(133.0, 100.0), Some(GizmoPart::Axis(0)));
        assert_eq!(hit(101.0, 80.0), Some(GizmoPart::Axis(1)));
        // La Z se ve de punta: el cuadrito del centro gana.
        assert_eq!(hit(100.0, 100.0), Some(GizmoPart::Plane));
        assert_eq!(hit(120.0, 105.0), None);
        assert_eq!(hit(140.0, 100.0), None);
    }

    #[test]
    fn hit_de_aro() {
        let g = gizmo_shape(GizmoKind::Rotate, [0.0; 3], 16.0, [true; 3]);
        let hit = |x, y| g.hit(front, (x, y), PICK_RADIUS);
        // El aro Z es un círculo de radio 32 puntos; el X, una raya vertical; el Y, horizontal.
        let d = 32.0 / 2f32.sqrt();
        assert_eq!(hit(100.0 + d + 1.0, 100.0 - d), Some(GizmoPart::Ring(2)));
        assert_eq!(hit(102.0, 80.0), Some(GizmoPart::Ring(0)));
        assert_eq!(hit(120.0, 101.0), Some(GizmoPart::Ring(1)));
        assert_eq!(hit(115.0, 85.0), None);
        // Solo el de profundidad (como en Front): el centro queda libre.
        let g = gizmo_shape(GizmoKind::Rotate, [0.0; 3], 16.0, [false, false, true]);
        assert_eq!(g.hit(front, (100.0, 100.0), PICK_RADIUS), None);
        assert_eq!(
            g.hit(front, (100.0, 69.0), PICK_RADIUS),
            Some(GizmoPart::Ring(2))
        );
    }

    #[test]
    fn hit_de_cubito() {
        let g = gizmo_shape(GizmoKind::Scale, [0.0; 3], 16.0, [true, true, false]);
        let hit = |x, y| g.hit(front, (x, y), PICK_RADIUS);
        // El cubito X en (132, 100), de 4 puntos de lado.
        assert_eq!(hit(137.0, 103.0), Some(GizmoPart::ScaleAxis(0)));
        assert_eq!(hit(100.0, 64.0), Some(GizmoPart::ScaleAxis(1)));
        assert_eq!(hit(101.0, 101.0), Some(GizmoPart::ScaleUniform));
        assert_eq!(hit(139.0, 100.0), None);
    }
}
