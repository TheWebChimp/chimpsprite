# Modelos 3D, parte 2A (modo Edit, seleccionar, mover y escalar): plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Editar modelos 3D con el mouse como picoCAD 2: modo Edit con las vistas Front, Top y
Side en wireframe más la perspectiva; seleccionar objetos, vértices, aristas o caras; moverlos y
escalarlos con snapping a una cuadrícula.

**Architecture:** La geometría pura (vistas, picks, selección, mover, escalar, snapping, asas) va
en `cs-3d/src/edit.rs`, sin egui y con pruebas. El `Editor` (cs-ui) guarda el estado del modo Edit
(`ModelEdit`: encendido, selección, cuadrícula, centro y zoom de las vistas, vista previa y gesto
en curso) y aplica los gestos: durante el gesto solo cambia la **vista previa** (no el documento);
al soltar o confirmar, un solo `SetModel` con nombre (un paso de undo); cancelar descarta la vista
previa. El lienzo (`canvas/model_edit.rs`) parte el artboard en 4 cuadrantes, dibuja y traduce el
mouse a esos gestos. `cs-render` gana `model_render` para pintar la perspectiva de la vista previa
a la mitad de resolución.

**Tech Stack:** Rust 2024, egui 0.36, el UI kit propio.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-02-3d-modeling-edit-design.md`.
- Sin floats guardados en `cs-core`; `cs-core`, `cs-3d`, `cs-render` sin egui. El estado del modo
  Edit no se guarda en el documento.
- Unidades: posiciones del mundo en **1/16 de unidad como `f32`** (`P3 = [f32; 3]`); la
  cuadrícula en 1/16: `1, 2, 4, 8, 16` (por defecto `4` = 1/4 de unidad); escalas en milésimas.
- Textos de UI en inglés ("Edit", "Object", "Vertex", "Edge", "Face", "Grid", "Front", "Top",
  "Side", "Persp"); comentarios y nombres de prueba como en el código vecino (comentarios en
  español).
- Nunca `git checkout -- …`, `git restore`, `git stash`, `git reset --hard`, `git clean`; solo
  `git add <archivos propios>`; LF; no tocar `.design/`.
- Toolchain (Git Bash): `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=8`;
  cargo a prioridad baja: `cmd //c "start /low /b /wait cargo test -p <crate>"`. `cargo fmt` y
  `cargo clippy --workspace --all-targets` sin avisos; commits en español terminados con
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Mapa de archivos

- Create `crates/cs-3d/src/edit.rs` — vistas, transformaciones, aristas, selección, picks, caja,
  mover, escalar, snapping, asas (Tasks 1–3). `crates/cs-3d/src/lib.rs` lo exporta.
- Modify `crates/cs-render/src/model3d.rs`, `crates/cs-render/src/lib.rs` — `model_render` (Task 4).
- Create `crates/cs-ui/src/editor/model_edit.rs` — `ModelEdit`, `EditOp`, `KeyOp` y los métodos
  del `Editor` (Task 5); pruebas en `crates/cs-ui/src/editor/model_edit_tests.rs`.
- Modify `crates/cs-ui/src/editor.rs` — campo `model_edit`, módulos, `ensure_valid`.
- Modify `crates/cs-ui/src/editor/model_ops.rs` — `select_model_object` sincroniza la selección.
- Create `crates/cs-ui/src/canvas/model_edit.rs` — cuadrantes, dibujo, gestos (Tasks 6–8).
- Modify `crates/cs-ui/src/canvas.rs`, `crates/cs-ui/src/canvas/input.rs`,
  `crates/cs-ui/src/canvas/model_input.rs` — enganchar el modo Edit.
- Modify `crates/cs-ui/src/lib.rs` (`editor_key`) — Tab, 1–4, G, S, Enter, Esc, flechas.
- Modify `crates/cs-ui/src/model_panel.rs` — Edit, modos, Grid (Task 9).
- Modify `crates/cs-ui/src/demo.rs` — `CHIMPSPRITE_DEMO_MODEL_EDIT` (Task 10).
- Tests UI en `crates/cs-ui/src/layout_tests.rs`.

---

### Task 1: Vistas, transformaciones, aristas y selección (`cs-3d/src/edit.rs`)

**Files:**
- Create: `crates/cs-3d/src/edit.rs`
- Modify: `crates/cs-3d/src/lib.rs` (agregar `mod edit;` y `pub use edit::*;`)

**Interfaces — Produces:**
```rust
pub type P3 = [f32; 3];                       // mundo, en 1/16 de unidad
pub enum OrthoView { Front, Top, Side }       // Copy, Default = Front
impl OrthoView {
    pub const ALL: [OrthoView; 3];
    pub fn label(self) -> &'static str;       // "Front" | "Top" | "Side"
    pub fn plane(self, p: P3) -> (f32, f32);  // (u a la derecha, v arriba)
    pub fn depth(self, p: P3) -> f32;         // mayor = más cerca del que mira
    pub fn from_plane(self, uv: (f32, f32), base: P3) -> P3; // los 2 ejes de la vista de uv, el otro de base
    pub fn world_delta(self, d: (f32, f32)) -> P3;
    pub fn axes(self) -> (usize, usize);      // ejes del mundo de u y v
}
pub fn to_world(t: &Transform3D, local: P3) -> P3;
pub fn to_local(t: &Transform3D, world: P3, keep: P3) -> P3;
pub fn world_points(obj: &Object3D) -> Vec<P3>;
pub fn mesh_edges(mesh: &Mesh) -> Vec<(u32, u32)>;       // a < b, ordenadas, sin repetir
pub enum SelectMode { Object, Vertex, Edge, Face }       // Copy, Default = Object
impl SelectMode { pub const ALL: [SelectMode; 4]; pub fn label(self) -> &'static str; }
pub enum Item { Object(usize), Vertex(usize, u32), Edge(usize, u32, u32), Face(usize, usize) } // Ord
impl Item { pub fn object(self) -> usize; }
pub struct Selection { pub mode: SelectMode, pub items: BTreeSet<Item> } // Clone, Default, Eq
impl Selection {
    pub fn is_empty(&self) -> bool;
    pub fn first_object(&self) -> Option<usize>;
    pub fn toggle(&mut self, item: Item);
    pub fn vertices(&self, model: &Model3D) -> BTreeMap<usize, BTreeSet<u32>>;
    pub fn retain_valid(&mut self, model: &Model3D);
    pub fn converted(&self, model: &Model3D, mode: SelectMode) -> Selection;
}
```

- [ ] **Step 1: Escribir las pruebas que fallan** (al final de `edit.rs`)

```rust
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
            rotation: Vec3i { x: 300, y: 450, z: -200 },
            scale: Vec3i { x: 2000, y: 500, z: -1000 },
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
            scale: Vec3i { x: 1000, y: 0, z: 1000 },
            ..Transform3D::default()
        };
        assert_eq!(to_local(&flat, [1.0, 0.0, 2.0], [9.0, 9.0, 9.0]), [1.0, 9.0, 2.0]);
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
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cmd //c "start /low /b /wait cargo test -p cs-3d edit"`
Expected: FAIL (no compila: `edit` no existe todavía).

- [ ] **Step 3: Implementar** (`crates/cs-3d/src/edit.rs`, arriba de las pruebas)

```rust
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
                        .filter(|(_, f)| !f.verts.is_empty() && f.verts.iter().all(|v| vs.contains(v)))
                        .map(|(i, _)| Item::Face(o, i)),
                ),
            }
        }
        Selection { mode, items }
    }
}
```

- [ ] **Step 4: Correr y ver que pasan**

Run: `cmd //c "start /low /b /wait cargo test -p cs-3d edit"`
Expected: PASS (5 pruebas). Si `las_vistas_proyectan_como_picocad` falla por `-0.0` vs `0.0`
en `world_delta`, compara con `==` sobre floats (`-0.0 == 0.0` es verdadero en Rust: no debería).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-3d/src/edit.rs crates/cs-3d/src/lib.rs
git commit -m "Modelos 3D: vistas ortográficas, aristas y selección para el modo Edit

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 2: Picks, caja de selección y límites (`edit.rs`)

**Files:** Modify `crates/cs-3d/src/edit.rs`.

**Interfaces — Consumes:** Task 1. **Produces:**
```rust
pub const PICK_RADIUS: f32 = 4.0;   // puntos de pantalla
pub struct ViewMap { pub view: OrthoView, pub center: P3, pub scale: f32, pub origin: (f32, f32) } // Copy, PartialEq
impl ViewMap {
    pub fn to_screen(&self, p: P3) -> (f32, f32);
    pub fn to_plane(&self, s: (f32, f32)) -> (f32, f32);
    pub fn drag_delta(&self, d: (f32, f32)) -> P3;   // arrastre en pantalla → mundo
}
pub fn pick(model: &Model3D, map: &ViewMap, mode: SelectMode, at: (f32, f32)) -> Option<Item>;
pub fn box_select(model: &Model3D, map: &ViewMap, mode: SelectMode, a: (f32, f32), b: (f32, f32)) -> BTreeSet<Item>;
pub fn bounds(model: &Model3D, sel: &Selection) -> Option<(P3, P3)>;
pub fn model_bounds(model: &Model3D) -> Option<(P3, P3)>;
pub fn center((lo, hi): (P3, P3)) -> P3;
```
`scale` son puntos de pantalla por 1/16 de unidad; `origin` es el centro del cuadrante en
pantalla (y hacia abajo); `center` es el punto del mundo que cae en `origin`.

- [ ] **Step 1: Pruebas que fallan** (agregar en `mod tests`)

```rust
    fn front_map() -> ViewMap {
        ViewMap {
            view: OrthoView::Front,
            center: [0.0; 3],
            scale: 2.0,
            origin: (100.0, 100.0),
        }
    }

    #[test]
    fn view_map_ida_y_vuelta() {
        let m = front_map();
        assert_eq!(m.to_screen([16.0, 16.0, 0.0]), (132.0, 68.0));
        assert_eq!(m.to_plane((132.0, 68.0)), (16.0, 16.0));
        assert_eq!(m.drag_delta((4.0, -2.0)), [2.0, 1.0, 0.0]);
    }

    #[test]
    fn pick_de_vertice_gana_el_mas_cercano_a_la_vista() {
        // Los vértices 3 (x+, y+, z−) y 7 (x+, y+, z+) caen en el mismo punto de Front.
        let m = one_cube();
        let map = front_map();
        assert_eq!(
            pick(&m, &map, SelectMode::Vertex, (131.0, 69.0)),
            Some(Item::Vertex(0, 7))
        );
        assert_eq!(pick(&m, &map, SelectMode::Vertex, (120.0, 69.0)), None);
    }

    #[test]
    fn pick_de_arista_cara_y_objeto() {
        let m = one_cube();
        let map = front_map();
        // El lado derecho (x = 16) a media altura: la arista de enfrente, 5 (x+, z+) – 7.
        assert_eq!(
            pick(&m, &map, SelectMode::Edge, (132.0, 100.0)),
            Some(Item::Edge(0, 5, 7))
        );
        // El centro: la cara de enfrente (la de z+).
        let Some(Item::Face(0, f)) = pick(&m, &map, SelectMode::Face, (100.0, 100.0)) else {
            panic!("no face");
        };
        let mesh = &m.objects[0].mesh;
        assert!(mesh.faces[f].verts.iter().all(|&v| mesh.vertices[v as usize].z > 0));
        // Dos cubos alineados: gana el de enfrente.
        let mut two = one_cube();
        two.objects.insert(0, cube_at(Vec3i { x: 0, y: 0, z: -64 }));
        assert_eq!(
            pick(&two, &map, SelectMode::Object, (100.0, 100.0)),
            Some(Item::Object(1))
        );
        assert_eq!(pick(&two, &map, SelectMode::Object, (0.0, 0.0)), None);
    }

    #[test]
    fn caja_de_seleccion_por_modo() {
        let m = one_cube();
        let map = front_map();
        let corner = box_select(&m, &map, SelectMode::Vertex, (120.0, 60.0), (140.0, 80.0));
        assert_eq!(corner, [Item::Vertex(0, 3), Item::Vertex(0, 7)].into());
        let all = box_select(&m, &map, SelectMode::Face, (0.0, 0.0), (200.0, 200.0));
        assert_eq!(all.len(), 6);
        let obj = box_select(&m, &map, SelectMode::Object, (140.0, 80.0), (120.0, 60.0));
        assert_eq!(obj, [Item::Object(0)].into());
        let none = box_select(&m, &map, SelectMode::Edge, (120.0, 60.0), (140.0, 80.0));
        assert!(none.is_empty(), "no edge has both ends in the corner box");
    }

    #[test]
    fn limites_de_la_seleccion() {
        let m = one_cube();
        let s = Selection {
            mode: SelectMode::Vertex,
            items: [Item::Vertex(0, 0), Item::Vertex(0, 7)].into(),
        };
        let b = bounds(&m, &s).unwrap();
        assert_eq!(b, ([-16.0; 3], [16.0; 3]));
        assert_eq!(center(b), [0.0; 3]);
        assert_eq!(bounds(&m, &Selection::default()), None);
        assert_eq!(model_bounds(&m), Some(([-16.0; 3], [16.0; 3])));
    }
```

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-3d edit"` → FAIL (no compila).

- [ ] **Step 3: Implementar** (en `edit.rs`, antes de `mod tests`)

```rust
/// Qué tan cerca (en puntos de pantalla) hay que hacer clic de un vértice, una arista o el punto
/// de una cara.
pub const PICK_RADIUS: f32 = 4.0;

/// Cómo se ve el mundo en el cuadrante de una vista: `center` cae en `origin` (pantalla, y hacia
/// abajo) y cada 1/16 de unidad mide `scale` puntos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewMap {
    pub view: OrthoView,
    pub center: P3,
    pub scale: f32,
    pub origin: (f32, f32),
}

impl ViewMap {
    pub fn to_screen(&self, p: P3) -> (f32, f32) {
        let (u, v) = self.view.plane(p);
        let (cu, cv) = self.view.plane(self.center);
        (
            self.origin.0 + (u - cu) * self.scale,
            self.origin.1 - (v - cv) * self.scale,
        )
    }

    /// El punto del plano que se ve en `s`.
    pub fn to_plane(&self, s: (f32, f32)) -> (f32, f32) {
        let (cu, cv) = self.view.plane(self.center);
        (
            cu + (s.0 - self.origin.0) / self.scale,
            cv - (s.1 - self.origin.1) / self.scale,
        )
    }

    /// Un arrastre de `d` puntos en pantalla, como desplazamiento del mundo.
    pub fn drag_delta(&self, d: (f32, f32)) -> P3 {
        self.view
            .world_delta((d.0 / self.scale, -d.1 / self.scale))
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn dist_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    dist(p, (a.0 + t * dx, a.1 + t * dy))
}

/// Si `p` cae dentro del polígono (par-impar; sirve en los dos sentidos de giro).
fn inside(poly: &[(f32, f32)], p: (f32, f32)) -> bool {
    let mut odd = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            odd = !odd;
        }
    }
    odd
}

/// El candidato `(item, distancia, profundidad)` más cerca del puntero; a la misma distancia
/// (redondeada a 1 punto), el más cercano a la vista.
fn best(cands: Vec<(Item, f32, f32)>) -> Option<Item> {
    let mut out: Option<(Item, f32, f32)> = None;
    for (item, d, depth) in cands {
        let d = d.round();
        let better = match out {
            None => true,
            Some((_, bd, bdepth)) => d < bd || (d == bd && depth > bdepth),
        };
        if better {
            out = Some((item, d, depth));
        }
    }
    out.map(|(item, _, _)| item)
}

/// Las caras de `obj` con al menos 3 vértices válidos: índice y vértices.
fn faces_of(obj: &Object3D) -> impl Iterator<Item = (usize, Vec<u32>)> + '_ {
    let n = obj.mesh.vertices.len() as u32;
    obj.mesh.faces.iter().enumerate().filter_map(move |(i, f)| {
        let vs: Vec<u32> = f.verts.iter().copied().filter(|&v| v < n).collect();
        (vs.len() >= 3).then_some((i, vs))
    })
}

/// Lo que hay bajo `at` (pantalla) en la vista de `map`, según el modo: un vértice o una
/// arista a [`PICK_RADIUS`] o menos; una cara si `at` cae dentro (o cerca de su punto central);
/// un objeto si `at` cae dentro de alguna de sus caras.
pub fn pick(model: &Model3D, map: &ViewMap, mode: SelectMode, at: (f32, f32)) -> Option<Item> {
    let mut cands = Vec::new();
    for (o, obj) in model.objects.iter().enumerate() {
        let pts = world_points(obj);
        let scr: Vec<(f32, f32)> = pts.iter().map(|&p| map.to_screen(p)).collect();
        let depth = |i: u32| map.view.depth(pts[i as usize]);
        match mode {
            SelectMode::Vertex => {
                for (i, &s) in scr.iter().enumerate() {
                    let d = dist(s, at);
                    if d <= PICK_RADIUS {
                        cands.push((Item::Vertex(o, i as u32), d, depth(i as u32)));
                    }
                }
            }
            SelectMode::Edge => {
                for (a, b) in mesh_edges(&obj.mesh) {
                    let d = dist_to_segment(at, scr[a as usize], scr[b as usize]);
                    if d <= PICK_RADIUS {
                        cands.push((Item::Edge(o, a, b), d, (depth(a) + depth(b)) / 2.0));
                    }
                }
            }
            SelectMode::Face | SelectMode::Object => {
                for (fi, vs) in faces_of(obj) {
                    let poly: Vec<(f32, f32)> = vs.iter().map(|&v| scr[v as usize]).collect();
                    let k = poly.len() as f32;
                    let c = (
                        poly.iter().map(|p| p.0).sum::<f32>() / k,
                        poly.iter().map(|p| p.1).sum::<f32>() / k,
                    );
                    let cd = vs.iter().map(|&v| depth(v)).sum::<f32>() / k;
                    let d = if inside(&poly, at) {
                        0.0
                    } else if mode == SelectMode::Face && dist(c, at) <= PICK_RADIUS {
                        dist(c, at)
                    } else {
                        continue;
                    };
                    let item = match mode {
                        SelectMode::Face => Item::Face(o, fi),
                        _ => Item::Object(o),
                    };
                    cands.push((item, d, cd));
                }
            }
        }
    }
    best(cands)
}

/// Lo que cae en la caja de pantalla entre `a` y `b`: vértices dentro; aristas y caras con todos
/// sus vértices dentro; objetos con algún vértice dentro.
pub fn box_select(
    model: &Model3D,
    map: &ViewMap,
    mode: SelectMode,
    a: (f32, f32),
    b: (f32, f32),
) -> BTreeSet<Item> {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let mut out = BTreeSet::new();
    for (o, obj) in model.objects.iter().enumerate() {
        let inn: Vec<bool> = world_points(obj)
            .iter()
            .map(|&p| {
                let s = map.to_screen(p);
                s.0 >= x0 && s.0 <= x1 && s.1 >= y0 && s.1 <= y1
            })
            .collect();
        let is_in = |v: u32| inn[v as usize];
        match mode {
            SelectMode::Object => {
                if inn.iter().any(|&i| i) {
                    out.insert(Item::Object(o));
                }
            }
            SelectMode::Vertex => out.extend(
                (0..inn.len() as u32)
                    .filter(|&v| is_in(v))
                    .map(|v| Item::Vertex(o, v)),
            ),
            SelectMode::Edge => out.extend(
                mesh_edges(&obj.mesh)
                    .into_iter()
                    .filter(|&(a, b)| is_in(a) && is_in(b))
                    .map(|(a, b)| Item::Edge(o, a, b)),
            ),
            SelectMode::Face => out.extend(
                faces_of(obj)
                    .filter(|(_, vs)| vs.iter().all(|&v| is_in(v)))
                    .map(|(i, _)| Item::Face(o, i)),
            ),
        }
    }
    out
}

fn bounds_of(points: impl Iterator<Item = P3>) -> Option<(P3, P3)> {
    let mut out: Option<(P3, P3)> = None;
    for p in points {
        let (lo, hi) = out.get_or_insert((p, p));
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    out
}

/// La caja (en el mundo) de los vértices involucrados en la selección.
pub fn bounds(model: &Model3D, sel: &Selection) -> Option<(P3, P3)> {
    let verts = sel.vertices(model);
    bounds_of(verts.iter().flat_map(|(&o, vs)| {
        let obj = &model.objects[o];
        vs.iter()
            .map(move |&v| to_world(&obj.transform, p3(obj.mesh.vertices[v as usize])))
    }))
}

/// La caja (en el mundo) de todo el modelo.
pub fn model_bounds(model: &Model3D) -> Option<(P3, P3)> {
    bounds_of(model.objects.iter().flat_map(world_points))
}

pub fn center((lo, hi): (P3, P3)) -> P3 {
    [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ]
}
```

- [ ] **Step 4: Correr** el mismo comando → PASS.
- [ ] **Step 5: Commit** `git add crates/cs-3d/src/edit.rs` y
  "Modelos 3D: elegir vértices, aristas, caras y objetos en las vistas ortográficas" (con el
  `Co-Authored-By`).

### Task 3: Mover, escalar, snapping y asas (`edit.rs`)

**Files:** Modify `crates/cs-3d/src/edit.rs`.

**Interfaces — Produces:**
```rust
pub fn translate(model: &Model3D, sel: &Selection, d: P3) -> Model3D;
pub fn scale(model: &Model3D, sel: &Selection, anchor: P3, f: P3) -> Model3D;
pub fn snap(v: f32, grid: i32) -> f32;
pub fn snapped_delta(view: OrthoView, grab: P3, d: P3, grid: Option<i32>) -> P3;
pub struct Handle { pub u: i8, pub v: i8 }    // Copy, Eq, Hash; -1 lado menor, 1 mayor, 0 ninguno (v arriba)
impl Handle { pub const ALL: [Handle; 8]; }
pub fn plane_box(view: OrthoView, b: (P3, P3)) -> ((f32, f32), (f32, f32)); // ((min_u, min_v), (max_u, max_v))
pub fn handle_scale(view: OrthoView, b: (P3, P3), handle: Handle, to: (f32, f32), grid: Option<i32>, proportional: bool) -> (P3, P3); // (ancla, factores)
```

- [ ] **Step 1: Pruebas que fallan**

```rust
    fn sel(mode: SelectMode, items: &[Item]) -> Selection {
        Selection {
            mode,
            items: items.iter().copied().collect(),
        }
    }

    #[test]
    fn mover_un_objeto_y_un_vertice_de_un_objeto_girado() {
        let m = one_cube();
        let moved = translate(&m, &sel(SelectMode::Object, &[Item::Object(0)]), [4.0, 0.0, -2.0]);
        assert_eq!(moved.objects[0].transform.position, Vec3i { x: 4, y: 0, z: -2 });
        let mut turned = one_cube();
        turned.objects[0].transform.rotation.y = 900;
        let before = world_points(&turned.objects[0])[1];
        let after = translate(&turned, &sel(SelectMode::Vertex, &[Item::Vertex(0, 1)]), [4.0, 0.0, 0.0]);
        let now = world_points(&after.objects[0])[1];
        assert!(close(now, [before[0] + 4.0, before[1], before[2]]), "{now:?}");
        // Los demás no se mueven.
        assert_eq!(after.objects[0].mesh.vertices[0], turned.objects[0].mesh.vertices[0]);
    }

    #[test]
    fn snapping_a_la_cuadricula() {
        assert_eq!(snap(5.0, 4), 4.0);
        assert_eq!(snap(6.0, 4), 8.0);
        assert_eq!(snap(-6.0, 4), -8.0);
        let d = snapped_delta(OrthoView::Front, [3.0, 5.0, 7.0], [10.0, 0.0, 0.0], Some(4));
        assert_eq!(d, [9.0, -1.0, 0.0]);
        // Sin cuadrícula (Ctrl): pasos de 1/16.
        let d = snapped_delta(OrthoView::Front, [3.0, 5.0, 7.0], [2.4, 0.0, 0.0], None);
        assert_eq!(d, [2.0, 0.0, 0.0]);
    }

    #[test]
    fn escalar_vertices_con_ancla() {
        let m = one_cube();
        let all = sel(SelectMode::Object, &[Item::Object(0)]).converted(&m, SelectMode::Vertex);
        let out = scale(&m, &all, [-16.0, -16.0, -16.0], [2.0, 1.0, 1.0]);
        for (v, w) in out.objects[0].mesh.vertices.iter().zip(&m.objects[0].mesh.vertices) {
            assert_eq!(v.x, if w.x > 0 { 48 } else { -16 });
            assert_eq!((v.y, v.z), (w.y, w.z));
        }
        // Factor 0: quedan planos.
        let flat = scale(&m, &all, [0.0; 3], [1.0, 0.0, 1.0]);
        assert!(flat.objects[0].mesh.vertices.iter().all(|v| v.y == 0));
    }

    #[test]
    fn escalar_objetos_cambia_su_escala_y_su_posicion() {
        let m = one_cube();
        let s = sel(SelectMode::Object, &[Item::Object(0)]);
        let out = scale(&m, &s, [-16.0, 0.0, 0.0], [2.0, 1.0, 1.0]);
        let t = out.objects[0].transform;
        assert_eq!(t.scale, Vec3i { x: 2000, y: 1000, z: 1000 });
        assert_eq!(t.position.x, 16);
        // Girado 90° en Y, el eje X del mundo es el Z local.
        let mut turned = m.clone();
        turned.objects[0].transform.rotation.y = 900;
        let out = scale(&turned, &s, [0.0; 3], [2.0, 1.0, 1.0]);
        assert_eq!(out.objects[0].transform.scale.z, 2000);
        // Nunca 0; negativo voltea.
        let out = scale(&m, &s, [0.0; 3], [0.0, -1.0, 1.0]);
        assert_eq!(out.objects[0].transform.scale.x, 1);
        assert_eq!(out.objects[0].transform.scale.y, -1000);
    }

    #[test]
    fn asas_de_lado_y_de_esquina() {
        let b = ([-16.0; 3], [16.0; 3]);
        let (anchor, f) = handle_scale(OrthoView::Front, b, Handle { u: 1, v: 0 }, (47.0, 0.0), Some(4), false);
        assert_eq!(anchor[0], -16.0);
        assert_eq!(f, [2.0, 1.0, 1.0]);
        let (_, f) = handle_scale(OrthoView::Front, b, Handle { u: 1, v: 1 }, (48.0, 16.0), Some(4), true);
        assert_eq!(f, [2.0, 2.0, 1.0]);
        // En Top, v es -Z: el asa de arriba escala Z con el ancla en z = +16.
        let (anchor, f) = handle_scale(OrthoView::Top, b, Handle { u: 0, v: 1 }, (0.0, 48.0), Some(4), false);
        assert_eq!(anchor[2], 16.0);
        assert_eq!(f, [1.0, 1.0, 2.0]);
        assert_eq!(plane_box(OrthoView::Side, b), ((-16.0, -16.0), (16.0, 16.0)));
    }
```

- [ ] **Step 2: Correr** → FAIL.

- [ ] **Step 3: Implementar**

```rust
/// Cambia en `out` los vértices involucrados en `sel` (de `model`): `f` lleva su posición en el
/// mundo a la nueva. Siempre desde los de `model`, así el redondeo no se acumula en un arrastre.
fn edit_vertices(out: &mut Model3D, model: &Model3D, sel: &Selection, f: impl Fn(P3) -> P3) {
    for (o, vs) in sel.vertices(model) {
        let obj = &mut out.objects[o];
        let t = obj.transform;
        for v in vs {
            let local = p3(model.objects[o].mesh.vertices[v as usize]);
            let w = f(to_world(&t, local));
            obj.mesh.vertices[v as usize] = round3(to_local(&t, w, local));
        }
    }
}

fn objects_of(sel: &Selection, model: &Model3D) -> Vec<usize> {
    let mut out: Vec<usize> = sel
        .items
        .iter()
        .map(|i| i.object())
        .filter(|&o| o < model.objects.len())
        .collect();
    out.dedup();
    out
}

fn comp(v: &mut Vec3i, axis: usize) -> &mut i32 {
    match axis {
        0 => &mut v.x,
        1 => &mut v.y,
        _ => &mut v.z,
    }
}

/// `model` con lo elegido corrido `d` (en el mundo): en modo Objeto, la posición de los objetos;
/// si no, los vértices involucrados.
pub fn translate(model: &Model3D, sel: &Selection, d: P3) -> Model3D {
    let mut out = model.clone();
    if sel.mode == SelectMode::Object {
        for o in objects_of(sel, model) {
            let p = &mut out.objects[o].transform.position;
            for (axis, delta) in d.iter().enumerate() {
                *comp(p, axis) += delta.round() as i32;
            }
        }
        return out;
    }
    edit_vertices(&mut out, model, sel, |w| [w[0] + d[0], w[1] + d[1], w[2] + d[2]]);
    out
}

/// El eje local (0 x, 1 y, 2 z) que, ya girado, apunta más hacia el eje `axis` del mundo.
fn local_axis(t: &Transform3D, axis: usize) -> usize {
    let r = Transform3D {
        rotation: t.rotation,
        ..Transform3D::default()
    };
    let along = |k: usize| {
        let mut u = [0.0; 3];
        u[k] = 1.0;
        to_world(&r, u)[axis].abs()
    };
    (0..3)
        .max_by(|&a, &b| along(a).total_cmp(&along(b)))
        .unwrap_or(axis)
}

/// Una escala (en milésimas) por `f`: nunca 0 (queda en ±1 con el signo que toca).
fn scaled(s: i32, f: f32) -> i32 {
    match (s as f32 * f).round() as i32 {
        0 if (s < 0) != (f < 0.0) => -1,
        0 => 1,
        v => v,
    }
}

/// `model` con lo elegido escalado `f` (por eje del mundo) desde `anchor`. En modo Objeto cambia
/// la escala del eje local más alineado con cada eje del mundo (exacto sin rotación) y corrige
/// la posición para que el ancla no se mueva; si no, escala las posiciones de los vértices.
pub fn scale(model: &Model3D, sel: &Selection, anchor: P3, f: P3) -> Model3D {
    let mut out = model.clone();
    if sel.mode == SelectMode::Object {
        for o in objects_of(sel, model) {
            let t = &mut out.objects[o].transform;
            for axis in 0..3 {
                if f[axis] == 1.0 {
                    continue;
                }
                let k = local_axis(t, axis);
                let s = comp(&mut t.scale, k);
                *s = scaled(*s, f[axis]);
                let p = comp(&mut t.position, axis);
                *p = (anchor[axis] + (*p as f32 - anchor[axis]) * f[axis]).round() as i32;
            }
        }
        return out;
    }
    edit_vertices(&mut out, model, sel, |w| {
        std::array::from_fn(|i| anchor[i] + (w[i] - anchor[i]) * f[i])
    });
    out
}

/// `v` en el múltiplo de `grid` más cercano.
pub fn snap(v: f32, grid: i32) -> f32 {
    let g = grid.max(1) as f32;
    (v / g).round() * g
}

/// El desplazamiento `d` corregido para que `grab + d` caiga en la cuadrícula `grid` (en 1/16)
/// en los dos ejes de `view`; sin cuadrícula (`None`, con Ctrl), en pasos de 1/16. El eje de
/// profundidad no se mueve.
pub fn snapped_delta(view: OrthoView, grab: P3, d: P3, grid: Option<i32>) -> P3 {
    let g = grid.unwrap_or(1);
    let (a, b) = view.axes();
    let mut out = [0.0; 3];
    for i in [a, b] {
        out[i] = snap(grab[i] + d[i], g) - grab[i];
    }
    out
}

/// Un asa de la caja de la selección en el plano de la vista: en `u` (a la derecha) y en `v`
/// (arriba), −1 el lado menor, 1 el mayor y 0 ninguno.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handle {
    pub u: i8,
    pub v: i8,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle { u: -1, v: -1 },
        Handle { u: 0, v: -1 },
        Handle { u: 1, v: -1 },
        Handle { u: -1, v: 0 },
        Handle { u: 1, v: 0 },
        Handle { u: -1, v: 1 },
        Handle { u: 0, v: 1 },
        Handle { u: 1, v: 1 },
    ];
}

/// La caja `b` (del mundo) en el plano de la vista: `((min_u, min_v), (max_u, max_v))`.
pub fn plane_box(view: OrthoView, (lo, hi): (P3, P3)) -> ((f32, f32), (f32, f32)) {
    let (a, c) = (view.plane(lo), view.plane(hi));
    ((a.0.min(c.0), a.1.min(c.1)), (a.0.max(c.0), a.1.max(c.1)))
}

/// Arrastrar `handle` de la caja `b` hasta `to` (en el plano): el ancla (el lado o la esquina
/// opuesta, en el mundo) y el factor por eje del mundo. El borde arrastrado cae en la
/// cuadrícula (`None`: pasos de 1/16). Con `proportional` (Shift, en una esquina) los dos ejes
/// usan el factor que más cambió.
pub fn handle_scale(
    view: OrthoView,
    b: (P3, P3),
    handle: Handle,
    to: (f32, f32),
    grid: Option<i32>,
    proportional: bool,
) -> (P3, P3) {
    let ((u0, v0), (u1, v1)) = plane_box(view, b);
    let g = grid.unwrap_or(1);
    let side = |s: i8, lo: f32, hi: f32, at: f32| {
        let (edge, anchor) = if s > 0 { (hi, lo) } else { (lo, hi) };
        let f = if (edge - anchor).abs() < f32::EPSILON {
            1.0
        } else {
            (snap(at, g) - anchor) / (edge - anchor)
        };
        (anchor, f)
    };
    let mid = center(b);
    let (mu, mv) = view.plane(mid);
    let (au, fu) = if handle.u != 0 { side(handle.u, u0, u1, to.0) } else { (mu, 1.0) };
    let (av, fv) = if handle.v != 0 { side(handle.v, v0, v1, to.1) } else { (mv, 1.0) };
    let (fu, fv) = if proportional && handle.u != 0 && handle.v != 0 {
        let f = if (fu - 1.0).abs() >= (fv - 1.0).abs() { fu } else { fv };
        (f, f)
    } else {
        (fu, fv)
    };
    let (a, c) = view.axes();
    let mut f = [1.0; 3];
    f[a] = fu;
    f[c] = fv;
    (view.from_plane((au, av), mid), f)
}
```

- [ ] **Step 4: Correr** → PASS. También `cmd //c "start /low /b /wait cargo clippy -p cs-3d --all-targets"` sin avisos.
- [ ] **Step 5: Commit** `git add crates/cs-3d/src/edit.rs` —
  "Modelos 3D: mover y escalar la selección, snapping y asas" (con el `Co-Authored-By`).

### Task 4: `cs_render::model_render`

**Files:** Modify `crates/cs-render/src/model3d.rs`, `crates/cs-render/src/lib.rs` (agregar
`model_render` al `pub use model3d::{...}`).

**Interfaces — Produces:**
```rust
/// El render de `model` (p. ej. la vista previa del modo Edit, no el guardado en `artboard`)
/// con la textura (frame 0) y la paleta de `artboard`, a `size`. Sin caché.
pub fn model_render(doc: &Document, artboard: &Artboard, model: &cs_core::Model3D, size: (u32, u32)) -> Vec<Rgba>;
```

- [ ] **Step 1: Prueba que falla** (en `mod tests` de `model3d.rs`)

```rust
    #[test]
    fn model_render_matches_the_canvas_render_at_the_same_size() {
        let (doc, model, _) = with_model();
        let artboard = doc.artboard(model).unwrap();
        let m = artboard.model.as_ref().unwrap();
        assert_eq!(model_render(&doc, artboard, m, (SIZE, SIZE)), model_pixels(&doc, model));
        let half = model_render(&doc, artboard, m, (SIZE / 2, SIZE / 2));
        assert_eq!(half.len(), (SIZE * SIZE / 4) as usize);
        assert_eq!(center(&half, SIZE / 2).a, 255);
    }
```

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-render model_render"` → FAIL.
- [ ] **Step 3: Implementar** (después de `model_pick`)

```rust
/// El render de `model` (p. ej. la vista previa del modo Edit, no el guardado en `artboard`)
/// con la textura (frame 0) y la paleta de `artboard`, a `size`. Sin caché.
pub fn model_render(
    doc: &Document,
    artboard: &Artboard,
    model: &cs_core::Model3D,
    size: (u32, u32),
) -> Vec<Rgba> {
    let mut stack = vec![artboard.id];
    let texture = texture_frame_of(doc, model, 0, &mut stack);
    render_model(&RenderInput {
        model,
        size,
        texture: texture.as_ref().map(|(width, height, pixels)| Texture {
            width: *width,
            height: *height,
            pixels,
        }),
        palette: artboard.sprite.palette.colors(),
        camera: None,
        extra_yaw: 0,
    })
}
```

- [ ] **Step 4: Correr** → PASS.
- [ ] **Step 5: Commit** "Modelos 3D: render de un modelo cualquiera a cualquier tamaño (para el modo Edit)".

### Task 5: El estado del modo Edit en el `Editor`

**Files:**
- Create: `crates/cs-ui/src/editor/model_edit.rs`, `crates/cs-ui/src/editor/model_edit_tests.rs`
- Modify: `crates/cs-ui/src/editor.rs` (declarar `mod model_edit;` y
  `#[cfg(test)] mod model_edit_tests;` junto a `model_ops`/`model_tests`; `pub use model_edit::{EditOp, KeyOp, ModelEdit};`;
  campo `pub(crate) model_edit: ModelEdit` junto a `model_arrows`, inicializado con
  `ModelEdit::default()` donde se inicializa `model_arrows`; al final de `ensure_valid` llamar
  `self.sync_model_edit();`)
- Modify: `crates/cs-ui/src/editor/model_ops.rs` (`select_model_object` sincroniza; ver abajo)

**Interfaces — Consumes:** Tasks 1–3 (`cs_3d::{Selection, SelectMode, Item, ViewMap, OrthoView, Handle, P3, translate, scale, snapped_delta, handle_scale, box_select, bounds, model_bounds, center}`).
**Produces:**
```rust
pub struct ModelEdit {
    pub on: bool,
    pub artboard: Option<ArtboardId>,    // el modelo en edición
    pub selection: Selection,
    pub grid: i32,                       // 1/16: 1, 2, 4 (defecto), 8, 16
    pub center: P3,                      // lo que se ve en el centro de las vistas orto
    pub zoom: f32,                       // pixeles del artboard por 1/16 de unidad
    pub last_view: OrthoView,            // la de las flechas
    pub preview: Option<Model3D>,        // el modelo durante un gesto (el documento no cambia)
    pub op: Option<EditOp>,
    pub key_request: Option<KeyOp>,      // G o S esperando al lienzo (que sabe dónde está el puntero)
}
pub enum KeyOp { Grab, Scale }
pub enum EditOp {
    Move { map: ViewMap, from: (f32, f32), grab: P3, keyboard: bool },
    Handle { map: ViewMap, handle: Handle, bounds: (P3, P3) },
    Scale { map: ViewMap, center: P3, from_dist: f32 },
    Box { map: ViewMap, from: (f32, f32), to: (f32, f32), add: bool },
    Pan { map: ViewMap, from: (f32, f32), center: P3 },
}
impl Editor {
    pub fn model_edit_on(&self) -> bool;                 // on && activo == artboard && es modelo
    pub fn toggle_model_edit(&mut self);                 // al entrar encuadra el modelo
    pub fn model_edit(&self) -> &ModelEdit;
    pub fn set_select_mode(&mut self, mode: SelectMode); // convierte la selección
    pub fn set_model_selection(&mut self, sel: Selection); // sincroniza model_object
    pub fn set_model_grid(&mut self, grid: i32);
    pub fn edit_model_shown(&self) -> Option<&Model3D>;  // la vista previa o el del documento
    pub fn start_model_op(&mut self, op: EditOp);
    pub fn update_model_op(&mut self, pointer: (f32, f32), free: bool, proportional: bool);
    pub fn finish_model_op(&mut self);                   // un paso de undo (o la selección de la caja)
    pub fn cancel_model_op(&mut self) -> bool;           // sin paso de undo
    pub fn nudge_model_selection(&mut self, dx: i32, dy: i32) -> bool; // un paso de cuadrícula en last_view
    pub fn zoom_model_views(&mut self, map: &ViewMap, at: (f32, f32), steps: i32);
    pub fn request_model_key_op(&mut self, op: KeyOp);
}
```
`free` = Ctrl (sin cuadrícula); `proportional` = Shift.

- [ ] **Step 1: Pruebas que fallan** (`model_edit_tests.rs`)

```rust
use cs_3d::{Item, OrthoView, SelectMode, Selection, ViewMap};
use cs_core::{ColorMode, Sprite};

use super::{EditOp, Editor};

fn editing() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_model("Ship", (64, 64)));
    e.toggle_model_edit();
    e
}

fn front() -> ViewMap {
    ViewMap {
        view: OrthoView::Front,
        center: [0.0; 3],
        scale: 2.0,
        origin: (100.0, 100.0),
    }
}

fn vertex(e: &Editor, v: usize) -> cs_core::Vec3i {
    e.model().unwrap().objects[0].mesh.vertices[v]
}

#[test]
fn edit_se_prende_solo_en_el_modelo_activo() {
    let mut e = editing();
    assert!(e.model_edit_on());
    // Encuadra el cubo (±16) en el centro.
    assert_eq!(e.model_edit().center, [0.0; 3]);
    assert!(e.model_edit().zoom > 0.0);
    let plain = e.document.artboards[0].id;
    e.activate(plain);
    assert!(!e.model_edit_on());
    e.toggle_model_edit();
    assert!(!e.model_edit_on(), "not on a plain artboard");
}

#[test]
fn cambiar_de_modo_convierte_y_sincroniza_el_objeto() {
    let mut e = editing();
    e.set_model_selection(Selection {
        mode: SelectMode::Object,
        items: [Item::Object(0)].into(),
    });
    e.set_select_mode(SelectMode::Vertex);
    assert_eq!(e.model_edit().selection.items.len(), 8);
    assert_eq!(e.selected_object(), Some(0));
    e.select_model_object(Some(0));
    assert_eq!(e.model_edit().selection.mode, SelectMode::Vertex);
    assert_eq!(e.model_edit().selection.items.len(), 8);
}

#[test]
fn mover_es_un_paso_de_undo_y_cancelar_ninguno() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    });
    let before = vertex(&e, 7);
    let undo_steps = e.history.done_count();
    e.start_model_op(EditOp::Move {
        map: front(),
        from: (132.0, 68.0),
        grab: [16.0, 16.0, 16.0],
        keyboard: false,
    });
    // 16 puntos a la derecha = 8/16: con la cuadrícula de 4, x = 24.
    e.update_model_op((148.0, 68.0), false, false);
    assert_eq!(vertex(&e, 7), before, "the document waits for the release");
    assert_eq!(e.edit_model_shown().unwrap().objects[0].mesh.vertices[7].x, 24);
    e.finish_model_op();
    assert_eq!(vertex(&e, 7).x, 24);
    assert_eq!(e.history.done_count(), undo_steps + 1);
    e.undo();
    assert_eq!(vertex(&e, 7), before);
    // Cancelar no deja paso.
    e.start_model_op(EditOp::Scale {
        map: front(),
        center: [0.0; 3],
        from_dist: 10.0,
    });
    e.update_model_op((120.0, 100.0), false, false);
    assert!(e.cancel_model_op());
    assert_eq!(vertex(&e, 7), before);
    assert_eq!(e.history.done_count(), undo_steps);
}

#[test]
fn la_caja_elige_y_con_add_agrega() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.start_model_op(EditOp::Box {
        map: front(),
        from: (120.0, 60.0),
        to: (120.0, 60.0),
        add: false,
    });
    e.update_model_op((140.0, 80.0), false, false);
    e.finish_model_op();
    assert_eq!(
        e.model_edit().selection.items,
        [Item::Vertex(0, 3), Item::Vertex(0, 7)].into()
    );
    // Un clic en vacío (caja de 0) sin add limpia.
    e.start_model_op(EditOp::Box {
        map: front(),
        from: (10.0, 10.0),
        to: (10.0, 10.0),
        add: false,
    });
    e.finish_model_op();
    assert!(e.model_edit().selection.is_empty());
}

#[test]
fn flechas_y_pan_y_zoom() {
    let mut e = editing();
    e.set_select_mode(SelectMode::Vertex);
    e.set_model_selection(Selection {
        mode: SelectMode::Vertex,
        items: [Item::Vertex(0, 7)].into(),
    });
    let before = vertex(&e, 7);
    assert!(e.nudge_model_selection(1, 0));
    assert_eq!(vertex(&e, 7).x, before.x + 4, "one grid step in Front");
    e.start_model_op(EditOp::Pan {
        map: front(),
        from: (100.0, 100.0),
        center: [0.0; 3],
    });
    e.update_model_op((110.0, 100.0), false, false);
    e.finish_model_op();
    assert_eq!(e.model_edit().center, [-5.0, 0.0, 0.0], "the view follows the drag");
    let zoom = e.model_edit().zoom;
    e.zoom_model_views(&front(), (100.0, 100.0), 1);
    assert!(e.model_edit().zoom > zoom);
}

#[test]
fn deshacer_quita_de_la_seleccion_lo_que_ya_no_existe() {
    let mut e = editing();
    assert!(e.add_primitive(cs_3d::Primitive::Plane));
    e.set_model_selection(Selection {
        mode: SelectMode::Object,
        items: [Item::Object(1)].into(),
    });
    e.undo();
    assert!(e.model_edit().selection.is_empty());
}
```

Nota: `e.history` y `done_count()` existen (ver `Editor::named`); si `history` es privado del
módulo `editor`, `model_edit_tests` vive dentro de `editor/` y lo ve.

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui model_edit_tests"` → FAIL.

- [ ] **Step 3: Implementar** `crates/cs-ui/src/editor/model_edit.rs`

```rust
//! El modo Edit del artboard 3D (picoCAD): la selección (objetos, vértices, aristas o caras),
//! las vistas ortográficas (centro y zoom compartidos), la cuadrícula y los gestos de mover y
//! escalar. Durante un gesto solo cambia la vista previa; al terminar es un paso de undo.

use cs_3d::{Handle, OrthoView, P3, SelectMode, Selection, ViewMap};
use cs_core::{ArtboardId, Model3D};

use super::Editor;

/// La cuadrícula por defecto: 1/4 de unidad.
pub const DEFAULT_GRID: i32 = 4;
/// Las cuadrículas del panel, en 1/16 de unidad.
pub const GRIDS: [i32; 5] = [1, 2, 4, 8, 16];
/// Cuánto cambia el zoom de las vistas por paso de rueda.
const ZOOM_STEP: f32 = 1.25;
const ZOOM_MIN: f32 = 0.02;
const ZOOM_MAX: f32 = 64.0;

/// Lo que pidió el teclado y empieza el lienzo (que sabe dónde está el puntero).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOp {
    Grab,
    Scale,
}

/// Un gesto en curso en una vista ortográfica (puntos de pantalla).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditOp {
    /// Mover lo elegido; `grab` es el punto agarrado (el que cae en la cuadrícula). `keyboard`:
    /// empezó con G (sigue al puntero sin botón; clic o Enter confirma).
    Move {
        map: ViewMap,
        from: (f32, f32),
        grab: P3,
        keyboard: bool,
    },
    /// Arrastrar un asa de la caja de la selección.
    Handle {
        map: ViewMap,
        handle: Handle,
        bounds: (P3, P3),
    },
    /// S: igual en los tres ejes desde `center`; el factor es la distancia del puntero al centro
    /// entre `from_dist`.
    Scale {
        map: ViewMap,
        center: P3,
        from_dist: f32,
    },
    /// Caja de selección (`add`: con Shift, agrega).
    Box {
        map: ViewMap,
        from: (f32, f32),
        to: (f32, f32),
        add: bool,
    },
    /// Desplazar las vistas (botón derecho o central).
    Pan {
        map: ViewMap,
        from: (f32, f32),
        center: P3,
    },
}

/// El estado del modo Edit (de la UI: no se guarda ni pasa por undo).
#[derive(Clone, Debug)]
pub struct ModelEdit {
    pub on: bool,
    pub artboard: Option<ArtboardId>,
    pub selection: Selection,
    pub grid: i32,
    pub center: P3,
    /// Pixeles del artboard por 1/16 de unidad en las vistas ortográficas.
    pub zoom: f32,
    pub last_view: OrthoView,
    pub preview: Option<Model3D>,
    pub op: Option<EditOp>,
    pub key_request: Option<KeyOp>,
}

impl Default for ModelEdit {
    fn default() -> Self {
        Self {
            on: false,
            artboard: None,
            selection: Selection::default(),
            grid: DEFAULT_GRID,
            center: [0.0; 3],
            zoom: 1.0,
            last_view: OrthoView::Front,
            preview: None,
            op: None,
            key_request: None,
        }
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Editor {
    pub fn model_edit(&self) -> &ModelEdit {
        &self.model_edit
    }

    /// Si el modo Edit está prendido en el artboard activo (y es un modelo).
    pub fn model_edit_on(&self) -> bool {
        self.model_edit.on && self.model_edit.artboard == Some(self.active) && self.is_model()
    }

    /// Prende o apaga el modo Edit. Al prenderlo en otro modelo, la selección empieza vacía;
    /// siempre encuadra el modelo entero en las vistas.
    pub fn toggle_model_edit(&mut self) {
        if self.model_edit_on() {
            self.model_edit.on = false;
            self.model_edit.preview = None;
            self.model_edit.op = None;
            self.model_edit.key_request = None;
        } else if self.is_model() {
            if self.model_edit.artboard != Some(self.active) {
                let mode = self.model_edit.selection.mode;
                self.model_edit.selection = Selection {
                    mode,
                    items: Default::default(),
                };
            }
            self.model_edit.on = true;
            self.model_edit.artboard = Some(self.active);
            self.fit_model_views();
        }
        self.revision += 1;
    }

    /// Centro y zoom para que el modelo entero quepa en un cuadrante (la mitad del artboard),
    /// con aire.
    fn fit_model_views(&mut self) {
        let Some(b) = self.model().and_then(cs_3d::model_bounds) else {
            return;
        };
        let extent = (0..3).map(|i| b.1[i] - b.0[i]).fold(16.0_f32, f32::max);
        let side = self.sprite().width.min(self.sprite().height) as f32 / 2.0;
        self.model_edit.center = cs_3d::center(b);
        self.model_edit.zoom = (0.8 * side / extent).clamp(ZOOM_MIN, ZOOM_MAX);
    }

    pub fn set_select_mode(&mut self, mode: SelectMode) {
        let next = match self.model() {
            Some(m) => self.model_edit.selection.converted(m, mode),
            None => Selection {
                mode,
                items: Default::default(),
            },
        };
        self.set_model_selection(next);
    }

    /// La selección nueva; el objeto del panel pasa a ser el primero de ella (si hay).
    pub fn set_model_selection(&mut self, sel: Selection) {
        if let Some(o) = sel.first_object() {
            self.model_object = Some(o);
        }
        self.model_edit.selection = sel;
        self.revision += 1;
    }

    pub fn set_model_grid(&mut self, grid: i32) {
        self.model_edit.grid = grid.max(1);
        self.revision += 1;
    }

    /// El modelo que se ve en el modo Edit: la vista previa del gesto o el del documento.
    pub fn edit_model_shown(&self) -> Option<&Model3D> {
        self.model_edit.preview.as_ref().or(self.model())
    }

    pub fn request_model_key_op(&mut self, op: KeyOp) {
        if self.model_edit_on() && !self.model_edit.selection.is_empty() {
            self.model_edit.key_request = Some(op);
        }
    }

    pub fn start_model_op(&mut self, op: EditOp) {
        if let EditOp::Move { map, .. } | EditOp::Handle { map, .. } = op {
            self.model_edit.last_view = map.view;
        }
        self.model_edit.preview = None;
        self.model_edit.op = Some(op);
        self.revision += 1;
    }

    /// El gesto en curso sigue al puntero (`free`: Ctrl, sin cuadrícula; `proportional`: Shift).
    pub fn update_model_op(&mut self, pointer: (f32, f32), free: bool, proportional: bool) {
        let Some(op) = self.model_edit.op else {
            return;
        };
        let grid = (!free).then_some(self.model_edit.grid);
        let sel = &self.model_edit.selection;
        let preview = match (op, self.model()) {
            (
                EditOp::Move {
                    map, from, grab, ..
                },
                Some(m),
            ) => {
                let d = map.drag_delta((pointer.0 - from.0, pointer.1 - from.1));
                let d = cs_3d::snapped_delta(map.view, grab, d, grid);
                Some(cs_3d::translate(m, sel, d))
            }
            (
                EditOp::Handle {
                    map,
                    handle,
                    bounds,
                },
                Some(m),
            ) => {
                let to = map.to_plane(pointer);
                let (anchor, f) =
                    cs_3d::handle_scale(map.view, bounds, handle, to, grid, proportional);
                Some(cs_3d::scale(m, sel, anchor, f))
            }
            (
                EditOp::Scale {
                    map,
                    center,
                    from_dist,
                },
                Some(m),
            ) if from_dist > 0.0 => {
                let f = (dist(pointer, map.to_screen(center)) / from_dist * 1000.0).round() / 1000.0;
                Some(cs_3d::scale(m, sel, center, [f; 3]))
            }
            (EditOp::Box { map, from, add, .. }, _) => {
                self.model_edit.op = Some(EditOp::Box {
                    map,
                    from,
                    to: pointer,
                    add,
                });
                None
            }
            (EditOp::Pan { map, from, center }, _) => {
                let d = map.drag_delta((pointer.0 - from.0, pointer.1 - from.1));
                self.model_edit.center = [center[0] - d[0], center[1] - d[1], center[2] - d[2]];
                None
            }
            _ => None,
        };
        if preview.is_some() {
            self.model_edit.preview = preview;
        }
        self.revision += 1;
    }

    /// Termina el gesto: mover o escalar es un paso de undo; la caja cambia la selección.
    pub fn finish_model_op(&mut self) {
        let Some(op) = self.model_edit.op.take() else {
            return;
        };
        if let EditOp::Box { map, from, to, add } = op {
            if let Some(m) = self.model() {
                let mode = self.model_edit.selection.mode;
                let hit = cs_3d::box_select(m, &map, mode, from, to);
                let mut next = if add {
                    self.model_edit.selection.clone()
                } else {
                    Selection {
                        mode,
                        items: Default::default(),
                    }
                };
                next.items.extend(hit);
                self.set_model_selection(next);
            }
            return;
        }
        let label = match op {
            EditOp::Move { .. } => "Move",
            _ => "Scale",
        };
        if let Some(model) = self.model_edit.preview.take() {
            self.named(label, |e| e.set_model(model));
        }
        self.revision += 1;
    }

    /// Cancela el gesto sin tocar el documento. `false` si no había ninguno.
    pub fn cancel_model_op(&mut self) -> bool {
        self.model_edit.key_request = None;
        let had = self.model_edit.op.take().is_some();
        self.model_edit.preview = None;
        if had {
            self.revision += 1;
        }
        had
    }

    /// Una flecha: lo elegido un paso de cuadrícula en la última vista usada (`dx` a la derecha,
    /// `dy` hacia abajo en pantalla). Un paso de undo.
    pub fn nudge_model_selection(&mut self, dx: i32, dy: i32) -> bool {
        let Some(m) = self.model() else {
            return false;
        };
        if self.model_edit.selection.is_empty() {
            return false;
        }
        let g = self.model_edit.grid as f32;
        let d = self
            .model_edit
            .last_view
            .world_delta((dx as f32 * g, -dy as f32 * g));
        let next = cs_3d::translate(m, &self.model_edit.selection, d);
        self.named("Move", |e| e.set_model(next))
    }

    /// `steps` pasos de rueda sobre la vista de `map`, alrededor del punto de pantalla `at`.
    pub fn zoom_model_views(&mut self, map: &ViewMap, at: (f32, f32), steps: i32) {
        let old = self.model_edit.zoom;
        let zoom = (old * ZOOM_STEP.powi(steps)).clamp(ZOOM_MIN, ZOOM_MAX);
        // El punto bajo el puntero se queda ahí: `scale` del mapa crece igual que el zoom.
        let before = map.to_plane(at);
        let after = ViewMap {
            scale: map.scale * zoom / old,
            ..*map
        }
        .to_plane(at);
        let shift = map.view.world_delta((before.0 - after.0, before.1 - after.1));
        let c = &mut self.model_edit.center;
        for i in 0..3 {
            c[i] += shift[i];
        }
        self.model_edit.zoom = zoom;
        self.revision += 1;
    }

    /// Después de cada cambio del documento: fuera del modelo en edición no hay gesto ni vista
    /// previa, y la selección pierde lo que ya no existe.
    pub(crate) fn sync_model_edit(&mut self) {
        let edit = &mut self.model_edit;
        let model = edit
            .artboard
            .and_then(|id| self.document.artboard(id))
            .and_then(|a| a.model.as_ref());
        match model {
            Some(m) => edit.selection.retain_valid(m),
            None => {
                edit.on = false;
                edit.selection.items.clear();
            }
        }
        if edit.artboard != Some(self.active) {
            edit.op = None;
            edit.preview = None;
        }
    }
}
```

En `model_ops.rs`: el `fn set_model` privado pasa a `pub(super) fn set_model` (lo usa
`model_edit.rs`). Y `select_model_object`:

```rust
    pub fn select_model_object(&mut self, i: Option<usize>) {
        self.model_object = i;
        // En el modo Edit, elegir un objeto (panel o perspectiva) lo elige en el modo actual.
        if let Some(m) = self.model() {
            let mode = self.model_edit.selection.mode;
            let picked = cs_3d::Selection {
                mode: cs_3d::SelectMode::Object,
                items: i.filter(|&i| i < m.objects.len()).map(cs_3d::Item::Object).into_iter().collect(),
            };
            self.model_edit.selection = picked.converted(m, mode);
        }
        self.revision += 1;
    }
```

Y en `editor.rs` al final de `ensure_valid` (después de lo que ya hace): `self.sync_model_edit();`.
`activate(id)` ya llama a `ensure_valid` o cambia `active`; `model_edit_on()` revisa `artboard ==
active` igual.

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui model_edit_tests"` y
  `cmd //c "start /low /b /wait cargo test -p cs-ui model_tests"` → PASS (las de `model_tests` siguen verdes).
- [ ] **Step 5: Commit** `git add crates/cs-ui/src/editor/model_edit.rs crates/cs-ui/src/editor/model_edit_tests.rs crates/cs-ui/src/editor.rs crates/cs-ui/src/editor/model_ops.rs`
  — "Modo Edit 3D: selección, vista previa y gestos en el editor".

### Task 6: Los 4 cuadrantes en el lienzo y Tab

**Files:**
- Create: `crates/cs-ui/src/canvas/model_edit.rs`
- Modify: `crates/cs-ui/src/canvas.rs` (`mod model_edit;`; campos `edit_quads: Option<[Rect; 4]>`
  y `edit_persp: Option<(u64, egui::TextureHandle)>` inicializados en `None`; en el bucle de
  artboards de `show` (~línea 816), cuando `active && editor.model_edit_on()`, después del marco
  de la ventana llamar `self.draw_model_edit(&painter, screen, editor, document, &ctx); continue;`
  en lugar del damero, la textura y las cuadrículas; si no hay ningún artboard en edición, poner
  `self.edit_quads = None` antes del bucle)
- Modify: `crates/cs-ui/src/lib.rs` (`editor_key`: Tab)
- Test: `crates/cs-ui/src/layout_tests.rs`

**Interfaces — Produces (canvas):**
```rust
pub(crate) const FRONT: usize = 0; pub(crate) const PERSP: usize = 1;
pub(crate) const TOP: usize = 2;   pub(crate) const SIDE: usize = 3;
pub(crate) fn quadrants(screen: Rect, size: (u32, u32)) -> [Rect; 4];
pub(crate) fn quadrant_view(i: usize) -> Option<OrthoView>;   // FRONT→Front, TOP→Top, SIDE→Side
impl CanvasView {
    pub(crate) fn model_edit_quadrants(&self) -> Option<[Rect; 4]>;
    pub(crate) fn edit_view_map(&self, editor: &Editor, view: OrthoView) -> Option<ViewMap>;
    pub(super) fn draw_model_edit(&mut self, painter: &egui::Painter, screen: Rect, editor: &Editor, doc: &Document, ctx: &egui::Context);
}
```

- [ ] **Step 1: Pruebas que fallan** (layout_tests, junto a las del modelo)

```rust
fn key_tap(h: &mut LayoutHarness, key: egui::Key) {
    for pressed in [true, false] {
        h.frame_with(vec![Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        }]);
    }
    h.frame();
}

#[test]
fn tab_on_a_model_enters_edit_mode_with_four_views() {
    let mut h = with_a_model_at_800();
    let pixels = artboard_pixels(&h);
    let timeline = h.app.view.timeline;
    key_tap(&mut h, egui::Key::Tab);
    assert!(h.app.doc.editor.model_edit_on());
    assert_eq!(h.app.view.timeline, timeline, "Tab did not toggle the timeline");
    let quads = h.app.doc.canvas.model_edit_quadrants().expect("the quadrants");
    assert_eq!(quads[0].min, pixels.min);
    assert_eq!(quads[3].max, pixels.max);
    assert_eq!(quads[0].width(), pixels.width() / 2.0);
    for label in ["Front", "Top", "Side", "Persp"] {
        assert!(h.texts.iter().any(|(t, _)| t == label), "{label}");
    }
    key_tap(&mut h, egui::Key::Tab);
    assert!(!h.app.doc.editor.model_edit_on());
    assert!(h.app.doc.canvas.model_edit_quadrants().is_none());
}
```
(Si el campo del timeline no se llama `h.app.view.timeline`, busca cómo lo leen otras pruebas
con `grep -n "timeline" crates/cs-ui/src/layout_tests.rs` y usa ese.)

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui tab_on_a_model"` → FAIL.

- [ ] **Step 3: Implementar**

`editor_key` en `lib.rs`, dentro de `if editor.is_model() {` y antes de las flechas:

```rust
        // Tab entra y sale del modo Edit (en un modelo no muestra la línea de tiempo).
        if key == Key::Tab && !shift {
            editor.toggle_model_edit();
            return true;
        }
```

`canvas/model_edit.rs` (dibujo; los gestos llegan en la Task 7):

```rust
//! El modo Edit del artboard 3D en el lienzo (como picoCAD 2): el área del artboard se parte en
//! cuatro cuadrantes, Front | Persp arriba y Top | Side abajo. Las vistas ortográficas son
//! wireframes (todas las aristas) con la cuadrícula, la selección, su caja con asas y las guías
//! punteadas; la perspectiva es el render pixel a la mitad de resolución.

use std::hash::{DefaultHasher, Hash, Hasher};

use cs_3d::{Handle, Item, OrthoView, SelectMode, ViewMap};
use cs_core::Document;

use super::*;

pub(crate) const FRONT: usize = 0;
pub(crate) const PERSP: usize = 1;
pub(crate) const TOP: usize = 2;
pub(crate) const SIDE: usize = 3;

/// Colores provisionales (los de picoCAD): fondo de las vistas orto, de la perspectiva, aristas,
/// cuadrícula y separadores.
const ORTHO_BG: Color32 = Color32::from_rgb(0x83, 0x76, 0x9c);
const PERSP_BG: Color32 = Color32::from_rgb(0x1d, 0x2b, 0x53);
const EDGE: Color32 = Color32::from_rgb(0xc2, 0xc3, 0xc7);
const GRID_LINE: Color32 = Color32::from_rgba_premultiplied(0x10, 0x10, 0x20, 0x30);
const GUIDE: Color32 = Color32::from_rgb(0x5f, 0x57, 0x4f);
const SEPARATOR: Color32 = Color32::from_rgb(0x1d, 0x2b, 0x53);
/// Lado de los cuadritos de los vértices y de las asas, en puntos.
const VERTEX_PX: f32 = 3.0;
pub(crate) const HANDLE_PX: f32 = 5.0;
/// Con líneas más juntas que esto (en puntos) la cuadrícula no se dibuja.
const GRID_MIN_PX: f32 = 4.0;

/// Los cuadrantes del área `screen` de un artboard de `size` pixeles: Front, Persp, Top y Side.
/// Cortan en pixeles enteros del artboard (con ancho impar, la columna de más es de la derecha).
pub(crate) fn quadrants(screen: Rect, (w, h): (u32, u32)) -> [Rect; 4] {
    let sx = screen.width() / w.max(1) as f32;
    let sy = screen.height() / h.max(1) as f32;
    let x = screen.left() + (w / 2) as f32 * sx;
    let y = screen.top() + (h / 2) as f32 * sy;
    [
        Rect::from_min_max(screen.min, pos2(x, y)),
        Rect::from_min_max(pos2(x, screen.top()), pos2(screen.right(), y)),
        Rect::from_min_max(pos2(screen.left(), y), pos2(x, screen.bottom())),
        Rect::from_min_max(pos2(x, y), screen.max),
    ]
}

pub(crate) fn quadrant_view(i: usize) -> Option<OrthoView> {
    match i {
        FRONT => Some(OrthoView::Front),
        TOP => Some(OrthoView::Top),
        SIDE => Some(OrthoView::Side),
        _ => None,
    }
}

fn quadrant_index(view: OrthoView) -> usize {
    match view {
        OrthoView::Front => FRONT,
        OrthoView::Top => TOP,
        OrthoView::Side => SIDE,
    }
}

fn pos(p: (f32, f32)) -> Pos2 {
    pos2(p.0, p.1)
}

impl CanvasView {
    pub(crate) fn model_edit_quadrants(&self) -> Option<[Rect; 4]> {
        self.edit_quads
    }

    /// Cómo se ve el mundo en el cuadrante de `view` (con el centro y el zoom del modo Edit).
    pub(crate) fn edit_view_map(&self, editor: &Editor, view: OrthoView) -> Option<ViewMap> {
        let q = self.edit_quads?[quadrant_index(view)];
        let e = editor.model_edit();
        Some(ViewMap {
            view,
            center: e.center,
            scale: e.zoom * self.scale().x,
            origin: (q.center().x, q.center().y),
        })
    }

    pub(super) fn draw_model_edit(
        &mut self,
        painter: &egui::Painter,
        screen: Rect,
        editor: &Editor,
        doc: &Document,
        ctx: &egui::Context,
    ) {
        let sprite = editor.sprite();
        let quads = quadrants(screen, (sprite.width, sprite.height));
        self.edit_quads = Some(quads);
        let Some(model) = editor.edit_model_shown().cloned() else {
            return;
        };
        let accent = crate::pixel_ui::frame_skin(ctx).map_or(theme::ACCENT, |s| s.colors().selection);
        for (i, q) in quads.iter().enumerate() {
            painter.rect_filled(*q, 0.0, if i == PERSP { PERSP_BG } else { ORTHO_BG });
        }
        self.draw_persp(painter, quads[PERSP], editor, doc, &model, ctx);
        for view in OrthoView::ALL {
            let q = quads[quadrant_index(view)];
            let Some(map) = self.edit_view_map(editor, view) else {
                continue;
            };
            let p = painter.with_clip_rect(q.intersect(painter.clip_rect()));
            draw_grid_lines(&p, q, &map, editor.model_edit().grid);
            draw_wireframe(&p, &map, &model, &editor.model_edit().selection, accent);
            draw_selection_frame(&p, q, &map, &model, editor, accent);
            if let Some(cs_3d_box) = box_rect(editor, view) {
                p.rect_stroke(cs_3d_box, 0.0, Stroke::new(1.0, accent), egui::StrokeKind::Inside);
            }
        }
        // Separadores de 1 punto y los nombres.
        let mid_x = quads[FRONT].right();
        let mid_y = quads[FRONT].bottom();
        painter.vline(mid_x, screen.y_range(), Stroke::new(1.0, SEPARATOR));
        painter.hline(screen.x_range(), mid_y, Stroke::new(1.0, SEPARATOR));
        let font = egui::TextStyle::Body.resolve(&ctx.style());
        for (i, label) in [(FRONT, "Front"), (PERSP, "Persp"), (TOP, "Top"), (SIDE, "Side")] {
            painter.text(
                quads[i].min + vec2(3.0, 2.0),
                egui::Align2::LEFT_TOP,
                label,
                font.clone(),
                EDGE,
            );
        }
    }

    /// La perspectiva: el render pixel del modelo a la medida del cuadrante en pixeles del
    /// artboard (la mitad), cacheado mientras no cambien el modelo, el documento ni el tamaño.
    fn draw_persp(
        &mut self,
        painter: &egui::Painter,
        q: Rect,
        editor: &Editor,
        doc: &Document,
        model: &cs_core::Model3D,
        ctx: &egui::Context,
    ) {
        let size = (
            (q.width() / self.scale().x).round().max(1.0) as u32,
            (q.height() / self.scale().y).round().max(1.0) as u32,
        );
        let mut h = DefaultHasher::new();
        (model, editor.doc_version, size, editor.active).hash(&mut h);
        let key = h.finish();
        if self.edit_persp.as_ref().is_none_or(|(k, _)| *k != key) {
            let pixels = cs_render::model_render(doc, editor.artboard(), model, size);
            let rgba: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
            let image = egui::ColorImage::from_rgba_unmultiplied([size.0 as usize, size.1 as usize], &rgba);
            let opts = egui::TextureOptions::NEAREST;
            match &mut self.edit_persp {
                Some((k, t)) => {
                    t.set(image, opts);
                    *k = key;
                }
                None => self.edit_persp = Some((key, ctx.load_texture("model_edit_persp", image, opts))),
            }
        }
        if let Some((_, t)) = &self.edit_persp {
            let full = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
            painter.image(t.id(), q, full, Color32::WHITE);
        }
    }
}

/// La caja de selección en curso en la vista `view` (en pantalla).
fn box_rect(editor: &Editor, view: OrthoView) -> Option<Rect> {
    match editor.model_edit().op {
        Some(crate::editor::EditOp::Box { map, from, to, .. }) if map.view == view => {
            Some(Rect::from_two_pos(pos(from), pos(to)))
        }
        _ => None,
    }
}

/// Las líneas de la cuadrícula (cada `grid` 1/16) que caen en el cuadrante, si no quedan muy
/// juntas.
fn draw_grid_lines(p: &egui::Painter, q: Rect, map: &ViewMap, grid: i32) {
    let step = grid.max(1) as f32;
    if step * map.scale < GRID_MIN_PX {
        return;
    }
    let (u0, v1) = map.to_plane((q.left(), q.top()));
    let (u1, v0) = map.to_plane((q.right(), q.bottom()));
    let stroke = Stroke::new(1.0, GRID_LINE);
    let mut u = (u0 / step).floor() * step;
    while u <= u1 {
        let x = map.origin.0 + (u - map.view.plane(map.center).0) * map.scale;
        p.vline(x.round() + 0.5, q.y_range(), stroke);
        u += step;
    }
    let mut v = (v0 / step).floor() * step;
    while v <= v1 {
        let y = map.origin.1 - (v - map.view.plane(map.center).1) * map.scale;
        p.hline(q.x_range(), y.round() + 0.5, stroke);
        v += step;
    }
}

/// Todas las aristas; lo elegido en el color de acento: en modo Objeto, todas las aristas del
/// objeto; en Arista, las suyas; en Cara, las de la cara y su punto central; en Vértice, los
/// cuadritos. Los vértices se ven como cuadritos fuera del modo Objeto.
fn draw_wireframe(
    p: &egui::Painter,
    map: &ViewMap,
    model: &cs_core::Model3D,
    sel: &cs_3d::Selection,
    accent: Color32,
) {
    for (o, obj) in model.objects.iter().enumerate() {
        let scr: Vec<Pos2> = cs_3d::world_points(obj).iter().map(|&w| pos(map.to_screen(w))).collect();
        let whole = sel.items.contains(&Item::Object(o));
        let face_edge = |a: u32, b: u32| {
            sel.items.iter().any(|i| match *i {
                Item::Face(fo, f) if fo == o => {
                    let vs = &obj.mesh.faces[f].verts;
                    (0..vs.len()).any(|k| {
                        let (x, y) = (vs[k], vs[(k + 1) % vs.len()]);
                        (x.min(y), x.max(y)) == (a, b)
                    })
                }
                _ => false,
            })
        };
        for (a, b) in cs_3d::mesh_edges(&obj.mesh) {
            let on = whole || sel.items.contains(&Item::Edge(o, a, b)) || face_edge(a, b);
            let color = if on { accent } else { EDGE };
            p.line_segment([scr[a as usize], scr[b as usize]], Stroke::new(1.0, color));
        }
        match sel.mode {
            SelectMode::Object => {}
            SelectMode::Face => {
                for (f, face) in obj.mesh.faces.iter().enumerate() {
                    let vs: Vec<Pos2> = face.verts.iter().filter_map(|&v| scr.get(v as usize).copied()).collect();
                    if vs.is_empty() {
                        continue;
                    }
                    let c = vs.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / vs.len() as f32;
                    let on = sel.items.contains(&Item::Face(o, f));
                    p.rect_filled(Rect::from_center_size(c.to_pos2(), Vec2::splat(VERTEX_PX)), 0.0, if on { accent } else { EDGE });
                }
            }
            _ => {
                for (v, s) in scr.iter().enumerate() {
                    let on = sel.items.contains(&Item::Vertex(o, v as u32));
                    let r = Rect::from_center_size(*s, Vec2::splat(VERTEX_PX));
                    p.rect_filled(r, 0.0, if on { accent } else { SEPARATOR });
                }
            }
        }
    }
}

/// Las asas de la caja de la selección (no con un solo vértice) y la cruz punteada por su centro.
fn draw_selection_frame(
    p: &egui::Painter,
    q: Rect,
    map: &ViewMap,
    model: &cs_core::Model3D,
    editor: &Editor,
    accent: Color32,
) {
    let sel = &editor.model_edit().selection;
    let Some(b) = cs_3d::bounds(model, sel) else {
        return;
    };
    let c = pos(map.to_screen(cs_3d::center(b)));
    let dash = Stroke::new(1.0, GUIDE);
    p.extend(egui::Shape::dashed_line(&[pos2(c.x, q.top()), pos2(c.x, q.bottom())], dash, 2.0, 2.0));
    p.extend(egui::Shape::dashed_line(&[pos2(q.left(), c.y), pos2(q.right(), c.y)], dash, 2.0, 2.0));
    if sel.mode == SelectMode::Vertex && sel.items.len() == 1 {
        return;
    }
    for h in Handle::ALL {
        let r = Rect::from_center_size(handle_pos(map, b, h), Vec2::splat(HANDLE_PX));
        p.rect_filled(r, 0.0, accent);
    }
}

/// Dónde se ve el asa `h` de la caja `b` (en pantalla).
pub(crate) fn handle_pos(map: &ViewMap, b: (cs_3d::P3, cs_3d::P3), h: Handle) -> Pos2 {
    let ((u0, v0), (u1, v1)) = cs_3d::plane_box(map.view, b);
    let pick = |s: i8, lo: f32, hi: f32| match s {
        -1 => lo,
        1 => hi,
        _ => (lo + hi) / 2.0,
    };
    let w = map.view.from_plane((pick(h.u, u0, u1), pick(h.v, v0, v1)), map.center);
    pos(map.to_screen(w))
}
```

Ajusta los detalles de egui que no compilen (p. ej. el nombre de `rect_stroke`/`StrokeKind` en egui
0.36 está bien; `Shape::dashed_line` devuelve `Vec<Shape>`). No cambies la geometría.

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui tab_on_a_model"` → PASS; y
  `cmd //c "start /low /b /wait cargo test -p cs-ui model"` (las pruebas existentes del modelo siguen
  verdes; fuera del modo Edit nada cambia).
- [ ] **Step 5: Commit** — "Modo Edit 3D: Tab parte el artboard en Front, Persp, Top y Side".

### Task 7: Gestos del mouse (elegir, caja, mover, desplazar, zoom, perspectiva)

**Files:** Modify `crates/cs-ui/src/canvas/model_edit.rs`, `crates/cs-ui/src/canvas/input.rs`,
`crates/cs-ui/src/canvas/model_input.rs`; Test `crates/cs-ui/src/layout_tests.rs`.

**Comportamiento** (con `editor.model_edit_on()` y el puntero en el área del artboard,
`over_model` ya lo detecta):
- **Prensa en una vista orto** (`quadrant_view(i)` es `Some`):
  - Botón **derecho o central**: si hay un gesto de teclado (`Move { keyboard: true }` o
    `Scale`), lo cancela; si no, `EditOp::Pan`.
  - **Izquierdo**: si hay un gesto de teclado, lo confirma (`finish_model_op`). Si no: un asa a
    `HANDLE_PX` o menos del puntero (y sin Shift) → `EditOp::Handle`. Si no,
    `cs_3d::pick(model, map, mode, p)`:
    - `Some(item)` con Shift → `toggle` (sin gesto).
    - `Some(item)` sin Shift → si no estaba elegido, la selección pasa a ser solo `item`; luego
      `EditOp::Move` con `grab` = el punto del vértice (modo Vértice) o el centro de
      `cs_3d::bounds` (los demás).
    - `None` → `EditOp::Box { add: shift }`.
- **Cada frame con gesto:** `editor.update_model_op(pointer, modifiers.command, modifiers.shift)`;
  al soltar el botón (si el gesto no es de teclado) `finish_model_op()`.
- **Ctrl + rueda** sobre una vista orto: `editor.zoom_model_views(&map, p, steps)`; sobre la
  perspectiva, lo de hoy (`model_wheel`).
- **Perspectiva:** como hoy (orbitar con derecho o Alt, objetivo con Shift, clic elige el objeto),
  con el punto pasado a pixeles del artboard entero: `local = ((p - q.min) / scale) · (W / pw, H / ph)`
  donde `(pw, ph)` es el tamaño del render del cuadrante. `ModelDrag` gana `factor: f32` (1 fuera
  del modo Edit, `H / ph` en la perspectiva) que multiplica `pixels` en `pan`.

**Interfaces — Produces:**
```rust
impl CanvasView {
    /// `true` si la prensa fue del modo Edit (no llega a nada más).
    pub(super) fn model_edit_press(&mut self, editor: &mut Editor, p: Pos2, secondary: bool, middle: bool, modifiers: egui::Modifiers) -> bool;
    pub(super) fn model_edit_input(&mut self, editor: &mut Editor, pointer: Option<Pos2>, down: bool, modifiers: egui::Modifiers);
    /// `true` si la rueda fue de una vista orto.
    pub(super) fn model_edit_wheel(&mut self, editor: &mut Editor, p: Pos2, steps: i32) -> bool;
    fn quadrant_at(&self, p: Pos2) -> Option<usize>;
}
```

- [ ] **Step 1: Pruebas que fallan** (layout_tests)

```rust
fn editing_model() -> LayoutHarness {
    let mut h = with_a_model_at_800();
    key_tap(&mut h, egui::Key::Tab);
    assert!(h.app.doc.editor.model_edit_on());
    h
}

fn front_screen(h: &LayoutHarness, w: cs_3d::P3) -> Pos2 {
    let map = h
        .app
        .doc
        .canvas
        .edit_view_map(&h.app.doc.editor, cs_3d::OrthoView::Front)
        .expect("front view");
    let (x, y) = map.to_screen(w);
    pos2(x, y)
}

fn drag(h: &mut LayoutHarness, from: Pos2, to: Pos2, modifiers: egui::Modifiers) {
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    h.frame_with(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.frame_with(vec![button(to, false)]);
    h.frame();
}

#[test]
fn clicking_a_vertex_in_front_selects_it_and_shift_click_toggles() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    let corner = front_screen(&h, [16.0, 16.0, 16.0]);
    h.click(corner, PointerButton::Primary);
    assert_eq!(
        h.app.doc.editor.model_edit().selection.items,
        [cs_3d::Item::Vertex(0, 7)].into()
    );
    let other = front_screen(&h, [-16.0, -16.0, 16.0]);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::SHIFT)]);
    drag(&mut h, other, other, egui::Modifiers::SHIFT);
    assert_eq!(h.app.doc.editor.model_edit().selection.items.len(), 2);
}

#[test]
fn dragging_a_vertex_in_front_moves_it_in_one_undo_step() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    let from = front_screen(&h, [16.0, 16.0, 16.0]);
    let to = front_screen(&h, [24.0, 16.0, 16.0]);
    drag(&mut h, from, to, Default::default());
    let v = h.app.doc.editor.model().unwrap().objects[0].mesh.vertices[7];
    assert_eq!((v.x, v.y, v.z), (24, 16, 16));
    h.app.doc.editor.undo();
    let v = h.app.doc.editor.model().unwrap().objects[0].mesh.vertices[7];
    assert_eq!(v.x, 16);
}

#[test]
fn a_box_in_an_empty_spot_selects_the_vertices_inside() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    let corner = front_screen(&h, [16.0, 16.0, 16.0]);
    drag(&mut h, corner + vec2(8.0, -8.0), corner + vec2(-8.0, 8.0), Default::default());
    assert_eq!(
        h.app.doc.editor.model_edit().selection.items,
        [cs_3d::Item::Vertex(0, 3), cs_3d::Item::Vertex(0, 7)].into()
    );
}

#[test]
fn right_drag_pans_the_ortho_views_and_ctrl_wheel_zooms_them() {
    let mut h = editing_model();
    let center = h.app.doc.editor.model_edit().center;
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    let at = quads[0].center();
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![button(at, true)]);
    h.frame_with(vec![Event::PointerMoved(at + vec2(20.0, 0.0))]);
    h.frame_with(vec![button(at + vec2(20.0, 0.0), false)]);
    h.frame();
    assert!(h.app.doc.editor.model_edit().center[0] < center[0]);
    let zoom = h.app.doc.editor.model_edit().zoom;
    h.frame_with(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, 1.0),
        modifiers: egui::Modifiers::COMMAND,
    }]);
    h.frame();
    assert!(h.app.doc.editor.model_edit().zoom > zoom);
}
```
(Si `Event::MouseWheel` tiene otros campos en egui 0.36, cópialos de las pruebas existentes de
Ctrl + rueda sobre el modelo: `grep -n "MouseWheel" crates/cs-ui/src/layout_tests.rs`.)

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui vertex_in_front"` → FAIL.

- [ ] **Step 3: Implementar**

En `canvas/model_edit.rs`:

```rust
use crate::editor::EditOp;

impl CanvasView {
    fn quadrant_at(&self, p: Pos2) -> Option<usize> {
        self.edit_quads?.iter().position(|q| q.contains(p))
    }

    /// Una prensa en el área del modelo en el modo Edit. En la perspectiva sigue lo de siempre
    /// (`model_press` con el punto en pixeles del artboard entero).
    pub(super) fn model_edit_press(
        &mut self,
        rect: Rect,
        editor: &mut Editor,
        p: Pos2,
        secondary: bool,
        middle: bool,
        modifiers: egui::Modifiers,
    ) -> bool {
        let Some(i) = self.quadrant_at(p) else {
            return false;
        };
        let Some(view) = quadrant_view(i) else {
            self.persp_press(rect, editor, i, p, secondary, modifiers);
            return true;
        };
        let Some(map) = self.edit_view_map(editor, view) else {
            return false;
        };
        let at = (p.x, p.y);
        let keyboard = matches!(
            editor.model_edit().op,
            Some(EditOp::Move { keyboard: true, .. } | EditOp::Scale { .. })
        );
        if secondary || middle {
            if keyboard {
                editor.cancel_model_op();
            } else {
                editor.start_model_op(EditOp::Pan {
                    map,
                    from: at,
                    center: editor.model_edit().center,
                });
            }
            return true;
        }
        if keyboard {
            editor.finish_model_op();
            return true;
        }
        let Some(model) = editor.model().cloned() else {
            return false;
        };
        let sel = editor.model_edit().selection.clone();
        if !modifiers.shift
            && let Some(b) = cs_3d::bounds(&model, &sel)
            && !(sel.mode == SelectMode::Vertex && sel.items.len() == 1)
            && let Some(handle) = Handle::ALL
                .into_iter()
                .find(|&h| (handle_pos(&map, b, h) - p).length() <= HANDLE_PX)
        {
            editor.start_model_op(EditOp::Handle { map, handle, bounds: b });
            return true;
        }
        match cs_3d::pick(&model, &map, sel.mode, at) {
            Some(item) if modifiers.shift => {
                let mut next = sel;
                next.toggle(item);
                editor.set_model_selection(next);
            }
            Some(item) => {
                if !sel.items.contains(&item) {
                    editor.set_model_selection(cs_3d::Selection {
                        mode: sel.mode,
                        items: [item].into(),
                    });
                }
                let now = editor.model_edit().selection.clone();
                let grab = match item {
                    Item::Vertex(o, v) => cs_3d::world_points(&model.objects[o])[v as usize],
                    _ => cs_3d::bounds(&model, &now).map_or([0.0; 3], cs_3d::center),
                };
                editor.start_model_op(EditOp::Move {
                    map,
                    from: at,
                    grab,
                    keyboard: false,
                });
            }
            None => editor.start_model_op(EditOp::Box {
                map,
                from: at,
                to: at,
                add: modifiers.shift,
            }),
        }
        true
    }

    /// La prensa en la perspectiva: como fuera del modo Edit, con el punto escalado del render
    /// del cuadrante al artboard entero.
    fn persp_press(
        &mut self,
        rect: Rect,
        editor: &mut Editor,
        i: usize,
        p: Pos2,
        secondary: bool,
        modifiers: egui::Modifiers,
    ) {
        let Some(q) = self.edit_quads.map(|qs| qs[i]) else {
            return;
        };
        let (w, h) = (editor.sprite().width as f32, editor.sprite().height as f32);
        let pw = (q.width() / self.scale().x).round().max(1.0);
        let ph = (q.height() / self.scale().y).round().max(1.0);
        let local = (
            ((p.x - q.left()) / self.scale().x * w / pw).floor() as i32,
            ((p.y - q.top()) / self.scale().y * h / ph).floor() as i32,
        );
        self.model_press_at(rect, editor, p, local, secondary, modifiers, h / ph);
    }

    /// Cada frame: el gesto sigue al puntero; al soltar (si no es de teclado) termina. Un G o S
    /// pedido por el teclado empieza aquí, en la vista bajo el puntero.
    pub(super) fn model_edit_input(
        &mut self,
        editor: &mut Editor,
        pointer: Option<Pos2>,
        down: bool,
        modifiers: egui::Modifiers,
    ) {
        if let Some(req) = editor.model_edit.key_request.take()
            && let Some(p) = pointer
            && let Some(view) = self.quadrant_at(p).and_then(quadrant_view)
            && let Some(map) = self.edit_view_map(editor, view)
            && let Some(b) = editor.model().and_then(|m| cs_3d::bounds(m, &editor.model_edit().selection))
        {
            let c = cs_3d::center(b);
            let op = match req {
                crate::editor::KeyOp::Grab => EditOp::Move {
                    map,
                    from: (p.x, p.y),
                    grab: c,
                    keyboard: true,
                },
                crate::editor::KeyOp::Scale => {
                    let s = map.to_screen(c);
                    EditOp::Scale {
                        map,
                        center: c,
                        from_dist: (pos2(s.0, s.1) - p).length().max(1.0),
                    }
                }
            };
            editor.start_model_op(op);
        }
        let Some(op) = editor.model_edit().op else {
            return;
        };
        if let Some(p) = pointer {
            editor.update_model_op((p.x, p.y), modifiers.command, modifiers.shift);
        }
        let keyboard = matches!(op, EditOp::Move { keyboard: true, .. } | EditOp::Scale { .. });
        if !down && !keyboard {
            editor.finish_model_op();
        }
    }

    pub(super) fn model_edit_wheel(&mut self, editor: &mut Editor, p: Pos2, steps: i32) -> bool {
        let Some(view) = self.quadrant_at(p).and_then(quadrant_view) else {
            return false;
        };
        let Some(map) = self.edit_view_map(editor, view) else {
            return false;
        };
        editor.zoom_model_views(&map, (p.x, p.y), steps);
        true
    }
}
```
`editor.model_edit` es `pub(crate)`: el lienzo puede tomar `key_request` directo.

En `model_input.rs`: separa `model_press` en dos: `model_press` (como hoy, calcula `local` con
`editor.to_local(self.to_world(rect, p))` y llama a `model_press_at(..., 1.0)`) y
`pub(super) fn model_press_at(&mut self, rect: Rect, editor: &mut Editor, p: Pos2, local: (i32, i32), secondary: bool, modifiers: egui::Modifiers, factor: f32)`
con el cuerpo de hoy usando `local` para `pick_model_object` y guardando `factor` en
`ModelDrag { orbit, from, camera, factor }`. En `model_drag_input`, `pan(drag.camera, pixels * drag.factor, ...)`.
Agrega `#[allow(clippy::too_many_arguments)]` si clippy lo pide.

En `input.rs`:
- En la rueda (~línea 352), antes de `Self::model_wheel(editor, steps)`:
  `if editor.model_edit_on() && self.model_edit_wheel(editor, pointer, steps) { continue; }`
  (calcula `steps` igual que hoy; `pointer` es el `hover_pos` del bloque).
- En la prensa (~línea 572): reemplaza la llamada por
  ```rust
  let middle = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Middle));
  if !(editor.model_edit_on()
      && self.model_edit_press(rect, editor, p, pressed_secondary, middle, modifiers))
  {
      self.model_press(rect, editor, p, pressed_secondary, modifiers);
  }
  ```
  y para que la prensa del botón central también entre, `model_press` (la variable) incluye
  `|| (middle && editor.model_edit_on())` en la condición de `can_press` solo para este camino
  (calcula `middle` antes).
- `let model_busy = self.model_drag.is_some() || editor.model_edit().op.is_some();`
- Después de `self.model_drag_input(editor, pointer, any_down);`:
  `if editor.model_edit_on() { self.model_edit_input(editor, pointer, any_down, modifiers); }`

- [ ] **Step 4: Correr** las 4 pruebas nuevas y `cmd //c "start /low /b /wait cargo test -p cs-ui model"` → PASS.
- [ ] **Step 5: Commit** — "Modo Edit 3D: elegir, caja, mover, desplazar y zoom con el mouse".

### Task 8: Asas, S, G, flechas, Enter, Esc y teclas 1–4

**Files:** Modify `crates/cs-ui/src/lib.rs` (`editor_key`); Test `crates/cs-ui/src/layout_tests.rs`.

**Comportamiento** en `editor_key`, dentro de `if editor.is_model() {`, primero (antes de lo de
hoy), cuando `editor.model_edit_on()`:
- `Num1..=Num4` (sin Shift) → `set_select_mode(Object | Vertex | Edge | Face)`.
- `G` (sin Shift) → `request_model_key_op(KeyOp::Grab)`; `S` → `KeyOp::Scale`.
- `Enter` con gesto → `finish_model_op()`; `Escape` → `cancel_model_op()` o, sin gesto, vaciar la
  selección (`set_model_selection` con el mismo modo y sin items).
- Flechas → `nudge_model_selection(dx, dy)`.
Todas devuelven `true`. Fuera del modo Edit todo sigue igual (G/R/S/flechas de hoy).

- [ ] **Step 1: Pruebas que fallan**

```rust
#[test]
fn number_keys_switch_the_selection_mode() {
    let mut h = editing_model();
    for (key, mode) in [
        (egui::Key::Num2, cs_3d::SelectMode::Vertex),
        (egui::Key::Num3, cs_3d::SelectMode::Edge),
        (egui::Key::Num4, cs_3d::SelectMode::Face),
        (egui::Key::Num1, cs_3d::SelectMode::Object),
    ] {
        key_tap(&mut h, key);
        assert_eq!(h.app.doc.editor.model_edit().selection.mode, mode);
    }
}

#[test]
fn a_corner_handle_scales_both_axes_of_the_view() {
    let mut h = editing_model();
    // El cubo elegido en modo Objeto: su caja es ±16.
    h.app.doc.editor.select_model_object(Some(0));
    h.frame();
    let from = front_screen(&h, [16.0, 16.0, 0.0]);
    let to = front_screen(&h, [48.0, 48.0, 0.0]);
    drag(&mut h, from, to, Default::default());
    let t = h.app.doc.editor.model().unwrap().objects[0].transform;
    assert_eq!((t.scale.x, t.scale.y, t.scale.z), (2000, 2000, 1000));
}

#[test]
fn s_follows_the_pointer_and_escape_cancels_without_an_undo_step() {
    let mut h = editing_model();
    h.app.doc.editor.select_model_object(Some(0));
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    let start = front_screen(&h, [8.0, 0.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(start)]);
    let steps = h.app.doc.editor.history.done_count();
    key_tap(&mut h, egui::Key::S);
    h.frame_with(vec![Event::PointerMoved(start + vec2(20.0, 0.0))]);
    h.frame();
    assert!(h.app.doc.editor.edit_model_shown().unwrap() != h.app.doc.editor.model().unwrap());
    key_tap(&mut h, egui::Key::Escape);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
    assert_eq!(
        h.app.doc.editor.model().unwrap().objects[0].transform.scale.x,
        1000
    );
    assert!(quads[0].contains(start));
}

#[test]
fn g_then_a_click_moves_and_arrows_nudge_by_the_grid() {
    let mut h = editing_model();
    h.app.doc.editor.select_model_object(Some(0));
    let start = front_screen(&h, [0.0, 0.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(start)]);
    key_tap(&mut h, egui::Key::G);
    let to = front_screen(&h, [8.0, 0.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.click(to, PointerButton::Primary);
    assert_eq!(h.app.doc.editor.model().unwrap().objects[0].transform.position.x, 8);
    key_tap(&mut h, egui::Key::ArrowRight);
    assert_eq!(h.app.doc.editor.model().unwrap().objects[0].transform.position.x, 12);
}
```
(`h.app.doc.editor.history` debe ser visible desde `layout_tests`; si es privado, usa el método
público que ya exista para contar pasos — busca `done_count` en `editor.rs` — o compara
`can_undo()` antes y después con un modelo recién creado.)

- [ ] **Step 2: Correr** → FAIL.
- [ ] **Step 3: Implementar** en `editor_key`:

```rust
    if editor.model_edit_on() {
        use cs_3d::SelectMode;
        let mode = match key {
            _ if shift => None,
            Key::Num1 => Some(SelectMode::Object),
            Key::Num2 => Some(SelectMode::Vertex),
            Key::Num3 => Some(SelectMode::Edge),
            Key::Num4 => Some(SelectMode::Face),
            _ => None,
        };
        if let Some(mode) = mode {
            editor.set_select_mode(mode);
            return true;
        }
        let busy = editor.model_edit().op.is_some();
        let mut handled = true;
        match key {
            Key::G if !shift => editor.request_model_key_op(editor::KeyOp::Grab),
            Key::S if !shift => editor.request_model_key_op(editor::KeyOp::Scale),
            Key::Enter if busy => editor.finish_model_op(),
            Key::Escape if busy => {
                editor.cancel_model_op();
            }
            Key::Escape => {
                let mode = editor.model_edit().selection.mode;
                editor.set_model_selection(cs_3d::Selection {
                    mode,
                    items: Default::default(),
                });
            }
            Key::ArrowLeft => {
                editor.nudge_model_selection(-1, 0);
            }
            Key::ArrowRight => {
                editor.nudge_model_selection(1, 0);
            }
            Key::ArrowUp => {
                editor.nudge_model_selection(0, -1);
            }
            Key::ArrowDown => {
                editor.nudge_model_selection(0, 1);
            }
            _ => handled = false,
        }
        if handled {
            return true;
        }
    }
```
Las demás teclas siguen al resto de `editor_key`. `Tab` (Task 6) sigue funcionando porque está
antes.

- [ ] **Step 4: Correr** las 4 pruebas y `cmd //c "start /low /b /wait cargo test -p cs-ui"` completo → PASS.
- [ ] **Step 5: Commit** — "Modo Edit 3D: asas, S y G con el mouse, flechas y teclas 1–4".

### Task 9: El panel 3D (Edit, modos y Grid)

**Files:** Modify `crates/cs-ui/src/model_panel.rs`; Test `crates/cs-ui/src/layout_tests.rs`.

Arriba del cuerpo con modelo (antes de `label_with_hint(ui, "Objects", ...)`):
- Una casilla **"Edit (Tab)"** (`check(ui, skin, &mut on, "Edit (Tab)")`; si cambió,
  `editor.toggle_model_edit()`).
- Una fila con 4 radios **Object / Vertex / Edge / Face** (`radio(ui, skin, on, mode.label())`;
  clic → `editor.set_select_mode(mode)`).
- Una fila **Grid** con un select del kit (`kit::select::select(ui, skin, "model_grid", label, Some(GRID_W), list)`
  con `GRID_W = 56.0`; sin skin, `egui::ComboBox`), opciones `"1/16" "1/8" "1/4" "1/2" "1"` que
  corresponden a `GRIDS = [1, 2, 4, 8, 16]` (`crate::editor::model_edit::GRIDS`, hazlo `pub` en el
  `pub use`); elegir → `editor.set_model_grid(g)`.
- Se ven aunque el modo Edit esté apagado.

```rust
fn grid_label(g: i32) -> &'static str {
    match g {
        1 => "1/16",
        2 => "1/8",
        4 => "1/4",
        8 => "1/2",
        _ => "1",
    }
}

/// Edit (Tab), los modos de selección y la cuadrícula del modo Edit.
fn edit_rows(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let mut on = editor.model_edit_on();
    check(ui, skin, &mut on, "Edit (Tab)");
    if on != editor.model_edit_on() {
        editor.toggle_model_edit();
    }
    let current = editor.model_edit().selection.mode;
    let mut picked = None;
    ui.horizontal(|ui| {
        for mode in cs_3d::SelectMode::ALL {
            if radio(ui, skin, current == mode, mode.label()).clicked() {
                picked = Some(mode);
            }
        }
    });
    if let Some(mode) = picked {
        editor.set_select_mode(mode);
    }
    let grid = editor.model_edit().grid;
    let mut chosen = None;
    let mut list = |ui: &mut Ui| {
        for g in crate::editor::GRIDS {
            if ui.add(menus::choice(g == grid, grid_label(g))).clicked() {
                chosen = Some(g);
                ui.close();
            }
        }
    };
    ui.horizontal(|ui| {
        ui.label("Grid");
        match skin {
            Some(skin) => {
                kit::select::select(ui, skin, "model_grid", grid_label(grid), Some(GRID_W), list);
            }
            None => {
                egui::ComboBox::from_id_salt("model_grid")
                    .selected_text(grid_label(grid))
                    .show_ui(ui, |ui| list(ui));
            }
        }
    });
    if let Some(g) = chosen {
        editor.set_model_grid(g);
    }
}
```

- [ ] **Step 1: Prueba que falla** (en layout_tests, usando cómo las pruebas existentes abren el
  panel 3D: `grep -n "fn .*model_panel\|\"Bake…\"" crates/cs-ui/src/layout_tests.rs` y copia la
  forma de abrirlo)

```rust
#[test]
fn the_3d_panel_has_edit_the_modes_and_the_grid() {
    let mut h = with_a_model_at_800();
    open_the_3d_panel(&mut h); // la misma forma que las pruebas del panel que ya existen
    for t in ["Edit (Tab)", "Object", "Vertex", "Edge", "Face", "Grid", "1/4"] {
        assert!(h.texts.iter().any(|(s, _)| s == t), "{t}");
    }
    let edit = h.texts.iter().find(|(s, _)| s == "Edit (Tab)").unwrap().1;
    h.click(edit.center(), PointerButton::Primary);
    assert!(h.app.doc.editor.model_edit_on());
    let vertex = h.texts.iter().find(|(s, _)| s == "Vertex").unwrap().1;
    h.click(vertex.center(), PointerButton::Primary);
    assert_eq!(
        h.app.doc.editor.model_edit().selection.mode,
        cs_3d::SelectMode::Vertex
    );
}
```
- [ ] **Step 2: Correr** → FAIL. **Step 3:** Implementar (llamar `edit_rows(ui, editor, skin);`
  en `body` antes de "Objects"). **Step 4:** Correr → PASS.
- [ ] **Step 5: Commit** — "Panel 3D: Edit, modos de selección y cuadrícula".

### Task 10: Demo, captura, spec y pulido final

**Files:** Modify `crates/cs-ui/src/demo.rs`, `docs/superpowers/specs/2026-11-02-3d-modeling-edit-design.md`.

- [ ] **Step 1:** En `demo.rs`, dentro del bloque de `CHIMPSPRITE_DEMO_MODEL` (al final, con el
  modelo activo): si `dev_var("CHIMPSPRITE_DEMO_MODEL_EDIT").is_some()`, prender el modo Edit,
  modo Vértice y elegir `Item::Vertex(0, 7)`:

```rust
        if dev_var("CHIMPSPRITE_DEMO_MODEL_EDIT").is_some() {
            let e = &mut app.doc.editor;
            e.activate(home);
            e.toggle_model_edit();
            e.set_select_mode(cs_3d::SelectMode::Vertex);
            e.set_model_selection(cs_3d::Selection {
                mode: cs_3d::SelectMode::Vertex,
                items: [cs_3d::Item::Vertex(0, 7)].into(),
            });
        }
```
(`home` es el id del modelo que ya guarda ese bloque; si está en otro alcance, usa el id del
artboard modelo activo antes de pintar la textura.)
- [ ] **Step 2:** `cargo fmt --all`, `cmd //c "start /low /b /wait cargo clippy --workspace --all-targets"` sin avisos,
  `cmd //c "start /low /b /wait cargo test --workspace"` verde.
- [ ] **Step 3:** Captura según la memoria de verificación de GUI (target dir nuevo, variables de
  demo `CHIMPSPRITE_DEMO_MODEL=1 CHIMPSPRITE_DEMO_MODEL_EDIT=1` y la de captura); revisar que se
  ven los 4 cuadrantes, el wireframe, el vértice elegido y la caja.
- [ ] **Step 4:** En el spec, `Estado: implementado` y una lista "Notas de la implementación"
  con lo que haya cambiado (p. ej. desplazar con derecho o central).
- [ ] **Step 5: Commit** — "Modo Edit 3D: demo, captura y spec al día".
