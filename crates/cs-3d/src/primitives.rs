//! Primitivas (cubo, plano, cilindro, cono, esfera) centradas en el origen, dentro de un cubo de
//! −1..+1 unidades (−16..16 en punto fijo), con +Y hacia arriba. Las caras van en sentido
//! antihorario visto desde afuera. Los UV caen en las zonas de la plantilla de 64×64.

use cs_core::{FIXED, Face, Mesh, PrimitiveKind, Vec3i};

/// Tamaño de la plantilla de textura de un modelo nuevo.
pub const TEMPLATE_SIZE: (u32, u32) = (64, 64);

type Rect = (i32, i32, i32, i32);

/// Celdas de 16×16 de la tapa de arriba del cilindro y de la base (cilindro y cono).
const CAP_TOP: Rect = (0, 48, 16, 16);
const CAP_BOTTOM: Rect = (16, 48, 16, 16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Cube,
    Plane,
    Cylinder { sides: u32 },
    Cone { sides: u32 },
    Sphere { sides: u32, rings: u32 },
}

impl Primitive {
    pub fn kind(&self) -> PrimitiveKind {
        match self {
            Primitive::Cube => PrimitiveKind::Cube,
            Primitive::Plane => PrimitiveKind::Plane,
            Primitive::Cylinder { .. } => PrimitiveKind::Cylinder,
            Primitive::Cone { .. } => PrimitiveKind::Cone,
            Primitive::Sphere { .. } => PrimitiveKind::Sphere,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Primitive::Cube => "Cube",
            Primitive::Plane => "Plane",
            Primitive::Cylinder { .. } => "Cylinder",
            Primitive::Cone { .. } => "Cone",
            Primitive::Sphere { .. } => "Sphere",
        }
    }

    /// Zona principal `(x, y, w, h)` de la plantilla.
    pub fn zone(&self) -> Rect {
        match self {
            Primitive::Cube => (0, 0, 48, 32),
            Primitive::Plane => (48, 0, 16, 16),
            Primitive::Cylinder { .. } => (0, 32, 32, 16),
            Primitive::Cone { .. } => (32, 32, 32, 16),
            Primitive::Sphere { .. } => (32, 48, 32, 16),
        }
    }

    /// Todas las zonas donde pueden caer sus UV (la principal y las de las tapas).
    pub fn zones(&self) -> Vec<Rect> {
        let mut z = vec![self.zone()];
        match self {
            Primitive::Cylinder { .. } => z.extend([CAP_TOP, CAP_BOTTOM]),
            Primitive::Cone { .. } => z.push(CAP_BOTTOM),
            _ => {}
        }
        z
    }
}

fn v3(x: i32, y: i32, z: i32) -> Vec3i {
    Vec3i { x, y, z }
}

fn face(verts: &[u32], uvs: &[(i32, i32)]) -> Face {
    Face {
        verts: verts.to_vec(),
        uvs: uvs.to_vec(),
    }
}

/// Punto del círculo de radio `r` (en punto fijo) en el ángulo `i/n` de vuelta; antihorario
/// visto desde arriba (+Y), con x = cos y z = −sin.
fn ring_point(i: u32, n: u32, r: f32, y: i32) -> Vec3i {
    let a = std::f32::consts::TAU * i as f32 / n as f32;
    v3(
        (r * a.cos()).round() as i32,
        y,
        (-r * a.sin()).round() as i32,
    )
}

/// UV del vértice `i` de una tapa circular inscrita en la celda `cell`. `up_sign` es +1 si
/// "arriba" en la textura es −Z (tapa de arriba) y −1 si es +Z (tapa de abajo).
fn cap_uv(i: u32, n: u32, cell: Rect, up_sign: f32) -> (i32, i32) {
    let a = std::f32::consts::TAU * i as f32 / n as f32;
    let (cx, cy) = (cell.0 as f32 + 8.0, cell.1 as f32 + 8.0);
    (
        (cx + 8.0 * a.cos()).round() as i32,
        (cy - up_sign * 8.0 * a.sin()).round() as i32,
    )
}

/// Tapa de `n` lados en abanico de triángulos: agrega un vértice al centro (a la altura `y`) y
/// una cara por lado; los vértices del anillo son `base..base+n`. La de abajo va en orden inverso.
fn cap_faces(
    vertices: &mut Vec<Vec3i>,
    faces: &mut Vec<Face>,
    n: u32,
    base: u32,
    y: i32,
    top: bool,
) {
    let cell = if top { CAP_TOP } else { CAP_BOTTOM };
    let sign = if top { 1.0 } else { -1.0 };
    let center = vertices.len() as u32;
    vertices.push(v3(0, y, 0));
    let c_uv = (cell.0 + 8, cell.1 + 8);
    for i in 0..n {
        let j = (i + 1) % n;
        let (a, b) = if top { (i, j) } else { (j, i) };
        faces.push(face(
            &[center, base + a, base + b],
            &[c_uv, cap_uv(a, n, cell, sign), cap_uv(b, n, cell, sign)],
        ));
    }
}

pub fn primitive_mesh(p: &Primitive) -> Mesh {
    match *p {
        Primitive::Cube => cube(),
        Primitive::Plane => plane(),
        Primitive::Cylinder { sides } => cylinder(sides.max(3)),
        Primitive::Cone { sides } => cone(sides.max(3)),
        Primitive::Sphere { sides, rings } => sphere(sides.max(3), rings.max(2)),
    }
}

/// Cubo: zona (0,0,48,32) en 3×2 celdas de 16×16. Fila 0: frente, derecha, atrás; fila 1:
/// izquierda, arriba, abajo. Cada cara llena su celda, derecha vista desde afuera con +Y arriba.
/// En la cara de arriba, "arriba" de la textura es −Z; en la de abajo, +Z.
fn cube() -> Mesh {
    let f = FIXED;
    // Índice del vértice por signo: bit0 = x>0, bit1 = y>0, bit2 = z>0.
    let vertices = (0..8)
        .map(|i| {
            let s = |b: i32| if i & b != 0 { f } else { -f };
            v3(s(1), s(2), s(4))
        })
        .collect();
    let idx =
        |x: i32, y: i32, z: i32| (x > 0) as u32 | ((y > 0) as u32) << 1 | ((z > 0) as u32) << 2;
    // Cada cara: esquinas (abajo-izq, abajo-der, arriba-der, arriba-izq) y celda (col, fila).
    type Corners = [(i32, i32, i32); 4];
    let defs: [(Corners, (i32, i32)); 6] = [
        ([(-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1)], (0, 0)),
        ([(1, -1, 1), (1, -1, -1), (1, 1, -1), (1, 1, 1)], (1, 0)),
        ([(1, -1, -1), (-1, -1, -1), (-1, 1, -1), (1, 1, -1)], (2, 0)),
        ([(-1, -1, -1), (-1, -1, 1), (-1, 1, 1), (-1, 1, -1)], (0, 1)),
        ([(-1, 1, 1), (1, 1, 1), (1, 1, -1), (-1, 1, -1)], (1, 1)),
        ([(-1, -1, -1), (1, -1, -1), (1, -1, 1), (-1, -1, 1)], (2, 1)),
    ];
    let faces = defs
        .iter()
        .map(|(c, (col, row))| {
            let (x0, y0) = (col * 16, row * 16);
            let verts: Vec<u32> = c.iter().map(|&(x, y, z)| idx(x, y, z)).collect();
            face(
                &verts,
                &[(x0, y0 + 16), (x0 + 16, y0 + 16), (x0 + 16, y0), (x0, y0)],
            )
        })
        .collect();
    Mesh { vertices, faces }
}

/// Plano de 2×2 en XY mirando a +Z (hacia una cámara Front); llena la zona (48,0,16,16).
fn plane() -> Mesh {
    let f = FIXED;
    Mesh {
        vertices: vec![v3(-f, -f, 0), v3(f, -f, 0), v3(f, f, 0), v3(-f, f, 0)],
        faces: vec![face(&[0, 1, 2, 3], &[(48, 16), (64, 16), (64, 0), (48, 0)])],
    }
}

/// Cilindro: anillo de abajo (0..n), anillo de arriba (n..2n); lados en la tira (0,32,32,16).
fn cylinder(n: u32) -> Mesh {
    let f = FIXED;
    let mut vertices: Vec<Vec3i> = (0..n).map(|i| ring_point(i, n, f as f32, -f)).collect();
    vertices.extend((0..n).map(|i| ring_point(i, n, f as f32, f)));
    let mut faces = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        let (u0, u1) = (i as i32 * 32 / n as i32, (i as i32 + 1) * 32 / n as i32);
        faces.push(face(
            &[i, j, n + j, n + i],
            &[(u0, 48), (u1, 48), (u1, 32), (u0, 32)],
        ));
    }
    cap_faces(&mut vertices, &mut faces, n, n, f, true);
    cap_faces(&mut vertices, &mut faces, n, 0, -f, false);
    Mesh { vertices, faces }
}

/// Cono: anillo de la base (0..n) y la punta (n); lados en (32,32,32,16), base como la del cilindro.
fn cone(n: u32) -> Mesh {
    let f = FIXED;
    let mut vertices: Vec<Vec3i> = (0..n).map(|i| ring_point(i, n, f as f32, -f)).collect();
    vertices.push(v3(0, f, 0));
    let mut faces = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        let (u0, u1) = (
            32 + i as i32 * 32 / n as i32,
            32 + (i as i32 + 1) * 32 / n as i32,
        );
        faces.push(face(&[i, j, n], &[(u0, 48), (u1, 48), ((u0 + u1) / 2, 32)]));
    }
    cap_faces(&mut vertices, &mut faces, n, 0, -f, false);
    Mesh { vertices, faces }
}

/// Esfera: polo de arriba (0), anillos de `n` vértices, polo de abajo (último); UV en la tira
/// (32,48,32,16) con u = longitud y v = latitud.
fn sphere(n: u32, rings: u32) -> Mesh {
    let f = FIXED as f32;
    let mut vertices = vec![v3(0, FIXED, 0)];
    for k in 1..rings {
        let phi = std::f32::consts::PI * k as f32 / rings as f32;
        let y = (f * phi.cos()).round() as i32;
        for i in 0..n {
            vertices.push(ring_point(i, n, f * phi.sin(), y));
        }
    }
    vertices.push(v3(0, -FIXED, 0));
    let bottom = vertices.len() as u32 - 1;
    let ring = |k: u32, i: u32| 1 + (k - 1) * n + i % n;
    let u = |i: u32| 32 + i as i32 * 32 / n as i32;
    let v = |k: u32| 48 + (16 * k as i32) / rings as i32;
    let mut faces = Vec::new();
    for i in 0..n {
        let um = (u(i) + u(i + 1)) / 2;
        faces.push(face(
            &[0, ring(1, i), ring(1, i + 1)],
            &[(um, v(0)), (u(i), v(1)), (u(i + 1), v(1))],
        ));
        for k in 1..rings - 1 {
            faces.push(face(
                &[
                    ring(k, i),
                    ring(k + 1, i),
                    ring(k + 1, i + 1),
                    ring(k, i + 1),
                ],
                &[
                    (u(i), v(k)),
                    (u(i), v(k + 1)),
                    (u(i + 1), v(k + 1)),
                    (u(i + 1), v(k)),
                ],
            ));
        }
        let k = rings - 1;
        faces.push(face(
            &[ring(k, i), bottom, ring(k, i + 1)],
            &[(u(i), v(k)), (um, v(rings)), (u(i + 1), v(k))],
        ));
    }
    Mesh { vertices, faces }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Primitive> {
        vec![
            Primitive::Cube,
            Primitive::Plane,
            Primitive::Cylinder { sides: 8 },
            Primitive::Cylinder { sides: 3 },
            Primitive::Cone { sides: 8 },
            Primitive::Cone { sides: 5 },
            Primitive::Sphere { sides: 8, rings: 4 },
            Primitive::Sphere { sides: 6, rings: 2 },
        ]
    }

    #[test]
    fn indices_and_uvs_are_consistent() {
        for p in all() {
            let m = primitive_mesh(&p);
            assert!(!m.faces.is_empty(), "{p:?}");
            for fc in &m.faces {
                assert!(fc.verts.len() >= 3, "{p:?}");
                assert!(fc.verts.len() <= 4, "{p:?}");
                assert_eq!(fc.verts.len(), fc.uvs.len(), "{p:?}");
                assert!(fc.verts.iter().all(|&v| (v as usize) < m.vertices.len()));
            }
        }
    }

    #[test]
    fn uvs_inside_zones() {
        for p in all() {
            let zones = p.zones();
            for fc in primitive_mesh(&p).faces {
                for (u, v) in fc.uvs {
                    assert!(
                        zones
                            .iter()
                            .any(|&(x, y, w, h)| u >= x && u <= x + w && v >= y && v <= y + h),
                        "{p:?} uv ({u},{v})"
                    );
                }
            }
        }
    }

    #[test]
    fn cube_counts() {
        let m = primitive_mesh(&Primitive::Cube);
        assert_eq!(m.vertices.len(), 8);
        assert_eq!(m.faces.len(), 6);
    }

    #[test]
    fn model_with_every_primitive_passes_check() {
        use cs_core::{ColorMode, Model3D, Object3D, Sprite, Transform3D};
        let model = Model3D {
            objects: all()
                .iter()
                .map(|p| Object3D {
                    name: p.name().into(),
                    mesh: primitive_mesh(p),
                    transform: Transform3D::default(),
                    double_sided: false,
                    kind: Some(p.kind()),
                    hidden: false,
                    locked: false,
                })
                .collect(),
            ..Model3D::default()
        };
        let sprite = Sprite::new(64, 64, ColorMode::Rgba);
        assert_eq!(model.check(&sprite), Ok(()));
    }

    #[test]
    fn cylinder_counts() {
        let m = primitive_mesh(&Primitive::Cylinder { sides: 8 });
        assert_eq!(m.faces.len(), 24);
        assert_eq!(m.faces.iter().filter(|f| f.verts.len() == 4).count(), 8);
        assert_eq!(m.faces.iter().filter(|f| f.verts.len() == 3).count(), 16);
    }

    #[test]
    fn normals_point_outward_and_vertices_in_bounds() {
        for p in all() {
            let m = primitive_mesh(&p);
            let f = FIXED;
            for v in &m.vertices {
                assert!(v.x.abs() <= f && v.y.abs() <= f && v.z.abs() <= f, "{p:?}");
            }
            for fc in &m.faces {
                let g = |i: usize| m.vertices[fc.verts[i] as usize];
                let (a, b, c) = (g(0), g(1), g(2));
                let u = (b.x - a.x, b.y - a.y, b.z - a.z);
                let w = (c.x - a.x, c.y - a.y, c.z - a.z);
                let n = (
                    u.1 * w.2 - u.2 * w.1,
                    u.2 * w.0 - u.0 * w.2,
                    u.0 * w.1 - u.1 * w.0,
                );
                let (mut cx, mut cy, mut cz) = (0, 0, 0);
                for i in 0..fc.verts.len() {
                    cx += g(i).x;
                    cy += g(i).y;
                    cz += g(i).z;
                }
                // El centro va multiplicado por el número de vértices (signo no cambia).
                let dot = n.0 as i64 * cx as i64 + n.1 as i64 * cy as i64 + n.2 as i64 * cz as i64;
                // El plano pasa por el origen: su centro es 0 y se revisa la normal (+Z).
                if matches!(p, Primitive::Plane) {
                    assert!(n.2 > 0, "plano debe mirar a +Z");
                } else {
                    assert!(dot > 0, "{p:?} {fc:?}");
                }
            }
        }
    }

    #[test]
    fn names_and_zone() {
        assert_eq!(Primitive::Cube.name(), "Cube");
        assert_eq!(Primitive::Plane.zone(), (48, 0, 16, 16));
        assert_eq!(Primitive::Cube.zone(), (0, 0, 48, 32));
        assert_eq!(TEMPLATE_SIZE, (64, 64));
    }
}
