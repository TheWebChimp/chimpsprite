//! Editar modelos con el mouse en las vistas ortográficas (Front, Top y Side, como picoCAD):
//! qué se ve en cada una, qué hay bajo el puntero, la selección (objetos, vértices, aristas o
//! caras) y moverla o escalarla. Todo en el mundo, en 1/16 de unidad (el punto fijo de los
//! vértices) como `f32`.

use std::collections::{BTreeMap, BTreeSet};

use cs_core::{Mesh, Model3D, Object3D, Transform3D, Vec3i};

/// Un punto del mundo en 1/16 de unidad.
pub type P3 = [f32; 3];

/// Las vistas ortográficas del modo Edit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OrthoView {
    /// Desde +Z: X a la derecha, Y arriba.
    #[default]
    Front,
    /// Desde +Y: X a la derecha, Z hacia abajo.
    Top,
    /// Desde +X: Z a la izquierda, Y arriba.
    Side,
}

impl OrthoView {
    pub const ALL: [OrthoView; 3] = [OrthoView::Front, OrthoView::Top, OrthoView::Side];

    pub fn label(self) -> &'static str {
        match self {
            OrthoView::Front => "Front",
            OrthoView::Top => "Top",
            OrthoView::Side => "Side",
        }
    }

    /// El punto en el plano de la vista: `u` a la derecha y `v` arriba.
    pub fn plane(self, p: P3) -> (f32, f32) {
        match self {
            OrthoView::Front => (p[0], p[1]),
            OrthoView::Top => (p[0], -p[2]),
            OrthoView::Side => (-p[2], p[1]),
        }
    }

    /// Qué tan cerca está `p` del que mira (mayor = más cerca).
    pub fn depth(self, p: P3) -> f32 {
        match self {
            OrthoView::Front => p[2],
            OrthoView::Top => p[1],
            OrthoView::Side => p[0],
        }
    }

    /// El punto del mundo con `(u, v)` en el plano y la profundidad de `base`.
    pub fn from_plane(self, (u, v): (f32, f32), base: P3) -> P3 {
        match self {
            OrthoView::Front => [u, v, base[2]],
            OrthoView::Top => [u, base[1], -v],
            OrthoView::Side => [base[0], v, -u],
        }
    }

    /// El desplazamiento del mundo que corre `d` en el plano (sin cambiar la profundidad).
    pub fn world_delta(self, d: (f32, f32)) -> P3 {
        self.from_plane(d, [0.0; 3])
    }

    /// Los ejes del mundo (0 x, 1 y, 2 z) de `u` y de `v`.
    pub fn axes(self) -> (usize, usize) {
        match self {
            OrthoView::Front => (0, 1),
            OrthoView::Top => (0, 2),
            OrthoView::Side => (2, 1),
        }
    }
}

fn sin_cos(tenths: i32) -> (f32, f32) {
    (tenths as f32 / 10.0).to_radians().sin_cos()
}

pub(crate) fn p3(v: Vec3i) -> P3 {
    [v.x as f32, v.y as f32, v.z as f32]
}

#[allow(dead_code)] // lo usan las siguientes tareas del modo Edit
pub(crate) fn round3(p: P3) -> Vec3i {
    Vec3i {
        x: p[0].round() as i32,
        y: p[1].round() as i32,
        z: p[2].round() as i32,
    }
}

/// Un punto local del objeto en el mundo, como el rasterizador: escala → rotación X, Y, Z →
/// posición.
pub fn to_world(t: &Transform3D, p: P3) -> P3 {
    let (x, y, z) = (
        p[0] * t.scale.x as f32 / 1000.0,
        p[1] * t.scale.y as f32 / 1000.0,
        p[2] * t.scale.z as f32 / 1000.0,
    );
    let (sx, cx) = sin_cos(t.rotation.x);
    let (sy, cy) = sin_cos(t.rotation.y);
    let (sz, cz) = sin_cos(t.rotation.z);
    let (y, z) = (y * cx - z * sx, y * sx + z * cx);
    let (x, z) = (x * cy + z * sy, -x * sy + z * cy);
    let (x, y) = (x * cz - y * sz, x * sz + y * cz);
    [
        x + t.position.x as f32,
        y + t.position.y as f32,
        z + t.position.z as f32,
    ]
}

/// Lo contrario de [`to_world`]. En un eje con escala 0 no hay vuelta: queda el de `keep`.
pub fn to_local(t: &Transform3D, w: P3, keep: P3) -> P3 {
    let (x, y, z) = (
        w[0] - t.position.x as f32,
        w[1] - t.position.y as f32,
        w[2] - t.position.z as f32,
    );
    let (sx, cx) = sin_cos(t.rotation.x);
    let (sy, cy) = sin_cos(t.rotation.y);
    let (sz, cz) = sin_cos(t.rotation.z);
    let (x, y) = (x * cz + y * sz, -x * sz + y * cz);
    let (x, z) = (x * cy - z * sy, x * sy + z * cy);
    let (y, z) = (y * cx + z * sx, -y * sx + z * cx);
    let back = |v: f32, s: i32, k: f32| if s == 0 { k } else { v * 1000.0 / s as f32 };
    [
        back(x, t.scale.x, keep[0]),
        back(y, t.scale.y, keep[1]),
        back(z, t.scale.z, keep[2]),
    ]
}

/// Los vértices del objeto en el mundo.
pub fn world_points(obj: &Object3D) -> Vec<P3> {
    obj.mesh
        .vertices
        .iter()
        .map(|&v| to_world(&obj.transform, p3(v)))
        .collect()
}

/// Las aristas de la malla: los pares de vértices seguidos de cada cara, `(a, b)` con `a < b`,
/// ordenadas y sin repetir (ni las que tocan vértices que no existen).
pub fn mesh_edges(mesh: &Mesh) -> Vec<(u32, u32)> {
    let n = mesh.vertices.len() as u32;
    let mut set = BTreeSet::new();
    for f in &mesh.faces {
        for i in 0..f.verts.len() {
            let (a, b) = (f.verts[i], f.verts[(i + 1) % f.verts.len()]);
            if a != b && a < n && b < n {
                set.insert((a.min(b), a.max(b)));
            }
        }
    }
    set.into_iter().collect()
}

/// Qué se elige con el mouse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectMode {
    #[default]
    Object,
    Vertex,
    Edge,
    Face,
}

impl SelectMode {
    pub const ALL: [SelectMode; 4] = [
        SelectMode::Object,
        SelectMode::Vertex,
        SelectMode::Edge,
        SelectMode::Face,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SelectMode::Object => "Object",
            SelectMode::Vertex => "Vertex",
            SelectMode::Edge => "Edge",
            SelectMode::Face => "Face",
        }
    }
}

/// Algo elegido: el objeto y lo que es dentro de él (vértice; arista `(a, b)` con `a < b`; cara).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Item {
    Object(usize),
    Vertex(usize, u32),
    Edge(usize, u32, u32),
    Face(usize, usize),
}

impl Item {
    pub fn object(self) -> usize {
        match self {
            Item::Object(o) | Item::Vertex(o, _) | Item::Edge(o, _, _) | Item::Face(o, _) => o,
        }
    }

    fn exists(self, model: &Model3D) -> bool {
        let Some(obj) = model.objects.get(self.object()) else {
            return false;
        };
        let n = obj.mesh.vertices.len() as u32;
        match self {
            Item::Object(_) => true,
            Item::Vertex(_, v) => v < n,
            Item::Edge(_, a, b) => mesh_edges(&obj.mesh).contains(&(a, b)),
            Item::Face(_, f) => f < obj.mesh.faces.len(),
        }
    }

    /// Los vértices que mueve (un objeto, todos los suyos).
    fn vertices(self, model: &Model3D) -> Vec<u32> {
        let mesh = &model.objects[self.object()].mesh;
        let n = mesh.vertices.len() as u32;
        match self {
            Item::Object(_) => (0..n).collect(),
            Item::Vertex(_, v) => vec![v],
            Item::Edge(_, a, b) => vec![a, b],
            Item::Face(_, f) => mesh.faces[f]
                .verts
                .iter()
                .copied()
                .filter(|&v| v < n)
                .collect(),
        }
    }
}

/// Lo elegido en el modo Edit (todo del mismo tipo que `mode`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub mode: SelectMode,
    pub items: BTreeSet<Item>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// El objeto del primer elegido (el de índice menor).
    pub fn first_object(&self) -> Option<usize> {
        self.items.iter().map(|i| i.object()).min()
    }

    /// Lo agrega si no estaba; si estaba, lo quita.
    pub fn toggle(&mut self, item: Item) {
        if !self.items.remove(&item) {
            self.items.insert(item);
        }
    }

    /// Los vértices involucrados por objeto (sin repetir; solo de lo que existe).
    pub fn vertices(&self, model: &Model3D) -> BTreeMap<usize, BTreeSet<u32>> {
        let mut out: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
        for &item in self.items.iter().filter(|i| i.exists(model)) {
            out.entry(item.object())
                .or_default()
                .extend(item.vertices(model));
        }
        out
    }

    /// Quita lo que ya no existe en `model` (después de deshacer, por ejemplo).
    pub fn retain_valid(&mut self, model: &Model3D) {
        self.items.retain(|i| i.exists(model));
    }

    /// La misma selección en otro modo: los objetos de lo elegido, o los vértices involucrados y
    /// las aristas y caras que tienen todos sus vértices entre ellos.
    pub fn converted(&self, model: &Model3D, mode: SelectMode) -> Selection {
        let mut items = BTreeSet::new();
        if mode == SelectMode::Object {
            items.extend(
                self.items
                    .iter()
                    .filter(|i| i.exists(model))
                    .map(|i| Item::Object(i.object())),
            );
            return Selection { mode, items };
        }
        for (o, vs) in self.vertices(model) {
            let mesh = &model.objects[o].mesh;
            match mode {
                SelectMode::Vertex => items.extend(vs.iter().map(|&v| Item::Vertex(o, v))),
                SelectMode::Edge => items.extend(
                    mesh_edges(mesh)
                        .into_iter()
                        .filter(|(a, b)| vs.contains(a) && vs.contains(b))
                        .map(|(a, b)| Item::Edge(o, a, b)),
                ),
                _ => items.extend(
                    mesh.faces
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| {
                            !f.verts.is_empty() && f.verts.iter().all(|v| vs.contains(v))
                        })
                        .map(|(i, _)| Item::Face(o, i)),
                ),
            }
        }
        Selection { mode, items }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Primitive, primitive_mesh};
    use cs_core::{Model3D, Object3D, Transform3D, Vec3i};

    pub(super) fn cube_at(position: Vec3i) -> Object3D {
        Object3D {
            name: "Cube".into(),
            mesh: primitive_mesh(&Primitive::Cube),
            transform: Transform3D {
                position,
                ..Transform3D::default()
            },
            double_sided: false,
        }
    }

    pub(super) fn one_cube() -> Model3D {
        Model3D {
            objects: vec![cube_at(Vec3i::default())],
            ..Model3D::default()
        }
    }

    fn close(a: P3, b: P3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
    }

    #[test]
    fn las_vistas_proyectan_como_picocad() {
        let p = [1.0, 2.0, 3.0];
        assert_eq!(OrthoView::Front.plane(p), (1.0, 2.0));
        assert_eq!(OrthoView::Front.depth(p), 3.0);
        assert_eq!(OrthoView::Top.plane(p), (1.0, -3.0));
        assert_eq!(OrthoView::Top.depth(p), 2.0);
        assert_eq!(OrthoView::Side.plane(p), (-3.0, 2.0));
        assert_eq!(OrthoView::Side.depth(p), 1.0);
        for v in OrthoView::ALL {
            assert_eq!(v.from_plane(v.plane(p), p), p, "{v:?}");
        }
        assert_eq!(OrthoView::Top.world_delta((4.0, 2.0)), [4.0, 0.0, -2.0]);
        assert_eq!(OrthoView::Side.axes(), (2, 1));
    }

    #[test]
    fn to_local_deshace_to_world() {
        let t = Transform3D {
            position: Vec3i { x: 5, y: -3, z: 7 },
            rotation: Vec3i {
                x: 300,
                y: 450,
                z: -200,
            },
            scale: Vec3i {
                x: 2000,
                y: 500,
                z: -1000,
            },
        };
        let p = [3.0, -7.0, 11.0];
        assert!(close(to_local(&t, to_world(&t, p), [0.0; 3]), p));
        // 90° en Y: +X local apunta a -Z en el mundo.
        let yaw = Transform3D {
            rotation: Vec3i { x: 0, y: 900, z: 0 },
            ..Transform3D::default()
        };
        assert!(close(to_world(&yaw, [16.0, 0.0, 0.0]), [0.0, 0.0, -16.0]));
        // Con escala 0 en un eje, ese eje se queda con `keep`.
        let flat = Transform3D {
            scale: Vec3i {
                x: 1000,
                y: 0,
                z: 1000,
            },
            ..Transform3D::default()
        };
        assert_eq!(
            to_local(&flat, [1.0, 0.0, 2.0], [9.0, 9.0, 9.0]),
            [1.0, 9.0, 2.0]
        );
    }

    #[test]
    fn el_cubo_tiene_12_aristas() {
        let edges = mesh_edges(&primitive_mesh(&Primitive::Cube));
        assert_eq!(edges.len(), 12);
        assert!(edges.iter().all(|&(a, b)| a < b));
    }

    #[test]
    fn convertir_la_seleccion_entre_modos() {
        let m = one_cube();
        let obj = Selection {
            mode: SelectMode::Object,
            items: [Item::Object(0)].into(),
        };
        assert_eq!(obj.converted(&m, SelectMode::Vertex).items.len(), 8);
        assert_eq!(obj.converted(&m, SelectMode::Edge).items.len(), 12);
        assert_eq!(obj.converted(&m, SelectMode::Face).items.len(), 6);
        // Los vértices de una cara dan esa cara (y sus 4 aristas).
        let face = &m.objects[0].mesh.faces[0];
        let verts = Selection {
            mode: SelectMode::Vertex,
            items: face.verts.iter().map(|&v| Item::Vertex(0, v)).collect(),
        };
        assert_eq!(
            verts.converted(&m, SelectMode::Face).items,
            [Item::Face(0, 0)].into()
        );
        assert_eq!(verts.converted(&m, SelectMode::Edge).items.len(), 4);
        assert_eq!(
            verts.converted(&m, SelectMode::Object).items,
            [Item::Object(0)].into()
        );
    }

    #[test]
    fn retain_valid_quita_lo_que_ya_no_existe() {
        let m = one_cube();
        let mut s = Selection {
            mode: SelectMode::Vertex,
            items: [Item::Vertex(0, 1), Item::Vertex(0, 99), Item::Vertex(3, 0)].into(),
        };
        s.retain_valid(&m);
        assert_eq!(s.items, [Item::Vertex(0, 1)].into());
        assert_eq!(s.first_object(), Some(0));
        s.toggle(Item::Vertex(0, 1));
        assert!(s.is_empty());
    }
}
