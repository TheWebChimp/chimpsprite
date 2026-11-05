# ChimpCAD outliner y barrita del objeto — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Grupos con transform, ocultar y bloquear partes en los modelos 3D; un outliner tipo
Blockbench en lugar de la tira de pestañas, y Position / Rotation / Scale en una barrita bajo el
encabezado de CHIMPCAD.

**Architecture:** `Model3D.objects` sigue plano (los índices de objeto de la selección, el pick,
las UVs y la topología no cambian). Se agregan `groups: Vec<Group3D>` y `tree: Vec<Node>` (el
árbol con el orden). Todo lo que pasa de local a mundo usa la cadena de transforms del objeto y
sus grupos (`Model3D::object_chain`). La UI nueva se arma con los componentes del kit.

**Tech Stack:** Rust (workspace: cs-core, cs-3d, cs-io, cs-render, cs-ui), egui/eframe, serde,
Aseprite MCP para las piezas del skin.

Spec: `docs/superpowers/specs/2026-11-04-chimpcad-outliner-design.md`.

## Global Constraints

- Comentarios y doc comments en español, como el código de alrededor; nombres en inglés.
- Repo `eol=lf`. Editar con la herramienta Edit o con scripts de Python escritos a archivo (los
  heredocs con muchas comillas se rompen). Nunca `git stash`, `git checkout -- …`, `git restore`,
  `git reset` ni `git clean` (el usuario edita `.design/` mientras se trabaja).
- Cada comando de cargo con el toolchain aislado y a baja prioridad:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH; export CARGO_BUILD_JOBS=8; cmd //c "start /low /b /wait cargo test -p <crate>"`.
- Antes de cada commit: `cargo fmt -p <crates tocados>`, los tests de los crates tocados y
  `cargo clippy -p <crates> --all-targets` sin warnings.
- Mensajes de commit en español; terminan con
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Archivos viejos (sin `groups`/`tree`/`hidden`/`locked`) se leen igual: `#[serde(default)]`.
- Cada cambio del modelo desde la UI es **un paso de undo** (abrir/cerrar un grupo no lo es).
- Íconos nuevos: provisionales, en `assets/ui/skin.aseprite`, con slice `icon.outliner.*`.

---

# Parte 1 — Modelo y render

### Task 1: Grupos, árbol, ocultar y bloquear en `Model3D` (cs-core) y al abrir (cs-io)

**Files:**
- Modify: `crates/cs-core/src/model.rs` (structs `Object3D` ~l.57, `Model3D` ~l.249, tests al final)
- Modify: `crates/cs-io/src/chimp.rs:281-284` (normalizar al leer)
- Modify: todos los `Object3D { … }` y `Model3D { … }` literales que no compilen (agregar
  `hidden: false, locked: false` / `groups: Vec::new(), tree: Vec::new()` o `..Default`):
  `grep -rn "Object3D {" crates --include=*.rs`.

**Interfaces — Produces (cs-core, `pub` y re-exportado desde `cs_core` como el resto de model.rs):**
```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Group3D {
    pub name: String,
    pub transform: Transform3D,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    /// Desplegado en el outliner.
    #[serde(default = "yes")]
    pub open: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Node {
    Object(usize),
    Group { group: usize, children: Vec<Node> },
}
// Object3D gana: #[serde(default)] pub hidden: bool, #[serde(default)] pub locked: bool
// Model3D gana:  #[serde(default)] pub groups: Vec<Group3D>, #[serde(default)] pub tree: Vec<Node>
impl Model3D {
    pub fn normalize_tree(&mut self);
    pub fn object_parents(&self, o: usize) -> Vec<usize>; // grupos, del más cercano al más lejano
    pub fn group_parents(&self, g: usize) -> Vec<usize>;
    pub fn object_hidden(&self, o: usize) -> bool;       // él o algún grupo padre
    pub fn object_locked(&self, o: usize) -> bool;
    pub fn object_chain(&self, o: usize) -> Vec<Transform3D>; // [objeto, padre, abuelo…]
    pub fn push_object(&mut self, obj: Object3D, parent: Option<usize>) -> usize;
    pub fn remove_object(&mut self, o: usize);           // quita y corre índices del árbol
    pub fn group_objects(&self, g: usize) -> Vec<usize>; // todos los de adentro (recursivo)
}
```

- [ ] **Step 1: Tests que fallan** (al final de `mod tests` de `model.rs`)

```rust
fn grouped() -> Model3D {
    // cube(0) suelto; grupo 0 "legs" con cube(1) y el grupo 1 "foot" con cube(2).
    let mut m = model_with(cube());
    m.objects.push(cube());
    m.objects.push(cube());
    m.groups = vec![
        Group3D { name: "legs".into(), transform: Transform3D::default(), hidden: false, locked: false, open: true },
        Group3D { name: "foot".into(), transform: Transform3D::default(), hidden: false, locked: false, open: true },
    ];
    m.tree = vec![
        Node::Object(0),
        Node::Group { group: 0, children: vec![
            Node::Object(1),
            Node::Group { group: 1, children: vec![Node::Object(2)] },
        ]},
    ];
    m
}

#[test]
fn a_model_without_a_tree_gets_one_with_its_objects_in_order() {
    let mut m = model_with(cube());
    m.objects.push(cube());
    m.normalize_tree();
    assert_eq!(m.tree, vec![Node::Object(0), Node::Object(1)]);
    // Idempotente.
    let again = m.clone();
    m.normalize_tree();
    assert_eq!(m, again);
}

#[test]
fn normalize_drops_bad_indices_and_adds_what_is_missing() {
    let mut m = grouped();
    m.tree.push(Node::Object(7)); // no existe
    m.tree.push(Node::Object(1)); // repetido
    m.objects.push(cube());       // el 3 no está en el árbol
    m.normalize_tree();
    assert_eq!(m.tree.len(), 3, "{:?}", m.tree);
    assert_eq!(m.tree[2], Node::Object(3));
    assert_eq!(m.object_parents(1), vec![0]);
}

#[test]
fn parents_hidden_locked_and_chain_are_inherited() {
    let mut m = grouped();
    assert_eq!(m.object_parents(2), vec![1, 0]);
    assert_eq!(m.object_parents(0), Vec::<usize>::new());
    assert_eq!(m.group_parents(1), vec![0]);
    m.groups[0].hidden = true;
    m.groups[1].locked = true;
    assert!(m.object_hidden(2) && m.object_hidden(1) && !m.object_hidden(0));
    assert!(m.object_locked(2) && !m.object_locked(1));
    m.groups[0].transform.position.x = 16;
    let chain = m.object_chain(2);
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[2].position.x, 16, "the outermost group last");
    assert_eq!(m.group_objects(0), vec![1, 2]);
}

#[test]
fn push_and_remove_keep_the_tree() {
    let mut m = grouped();
    let i = m.push_object(cube(), Some(1));
    assert_eq!(i, 3);
    assert_eq!(m.object_parents(3), vec![1, 0]);
    m.remove_object(1);
    // El 2 pasa a ser el 1 (y el 3 el 2), en sus mismos grupos.
    assert_eq!(m.objects.len(), 3);
    assert_eq!(m.object_parents(1), vec![1, 0]);
    assert_eq!(m.object_parents(2), vec![1, 0]);
    assert!(!format!("{:?}", m.tree).contains("Object(3)"));
}

#[test]
fn a_model_saved_before_groups_reads_without_them() {
    let m = model_with(cube());
    let mut v = serde_json::to_value(&m).unwrap();
    let o = v.as_object_mut().unwrap();
    o.remove("groups");
    o.remove("tree");
    for obj in o["objects"].as_array_mut().unwrap() {
        let obj = obj.as_object_mut().unwrap();
        obj.remove("hidden");
        obj.remove("locked");
    }
    let back: Model3D = serde_json::from_value(v).unwrap();
    assert!(back.groups.is_empty() && back.tree.is_empty());
    assert!(!back.objects[0].hidden && !back.objects[0].locked);
}
```

- [ ] **Step 2: Correr y ver que fallan** — `cargo test -p cs-core model` → no compila (tipos nuevos).

- [ ] **Step 3: Implementar** en `model.rs`:

```rust
fn yes() -> bool {
    true
}

/// Busca `target` en `nodes` y devuelve los grupos que lo contienen, del más cercano al más
/// lejano. `None` si no está.
fn path_to(nodes: &[Node], target: &Node) -> Option<Vec<usize>> {
    for n in nodes {
        if n == target {
            return Some(Vec::new());
        }
        if let Node::Group { group, children } = n {
            if let Node::Group { group: g, .. } = target
                && g == group
            {
                return Some(Vec::new());
            }
            if let Some(mut p) = path_to(children, target) {
                p.push(*group);
                return Some(p);
            }
        }
    }
    None
}

impl Model3D {
    /// Deja el árbol del outliner consistente: cada objeto y cada grupo una sola vez (se quitan
    /// los índices fuera de rango y los repetidos) y lo que falte, al final de la raíz en orden.
    pub fn normalize_tree(&mut self) {
        let (no, ng) = (self.objects.len(), self.groups.len());
        let mut seen_o = vec![false; no];
        let mut seen_g = vec![false; ng];
        fn clean(nodes: Vec<Node>, so: &mut [bool], sg: &mut [bool]) -> Vec<Node> {
            nodes
                .into_iter()
                .filter_map(|n| match n {
                    Node::Object(o) if o < so.len() && !so[o] => {
                        so[o] = true;
                        Some(Node::Object(o))
                    }
                    Node::Group { group, children } if group < sg.len() && !sg[group] => {
                        sg[group] = true;
                        Some(Node::Group { group, children: clean(children, so, sg) })
                    }
                    _ => None,
                })
                .collect()
        }
        let mut tree = clean(std::mem::take(&mut self.tree), &mut seen_o, &mut seen_g);
        tree.extend((0..ng).filter(|&g| !seen_g[g]).map(|g| Node::Group { group: g, children: Vec::new() }));
        tree.extend((0..no).filter(|&o| !seen_o[o]).map(Node::Object));
        self.tree = tree;
    }

    pub fn object_parents(&self, o: usize) -> Vec<usize> {
        path_to(&self.tree, &Node::Object(o)).unwrap_or_default()
    }

    pub fn group_parents(&self, g: usize) -> Vec<usize> {
        path_to(&self.tree, &Node::Group { group: g, children: Vec::new() }).unwrap_or_default()
    }

    pub fn object_hidden(&self, o: usize) -> bool {
        self.objects.get(o).is_some_and(|ob| ob.hidden)
            || self.object_parents(o).iter().any(|&g| self.groups.get(g).is_some_and(|g| g.hidden))
    }

    pub fn object_locked(&self, o: usize) -> bool {
        self.objects.get(o).is_some_and(|ob| ob.locked)
            || self.object_parents(o).iter().any(|&g| self.groups.get(g).is_some_and(|g| g.locked))
    }

    pub fn object_chain(&self, o: usize) -> Vec<Transform3D> {
        let mut out = vec![self.objects.get(o).map(|ob| ob.transform).unwrap_or_default()];
        out.extend(self.object_parents(o).iter().filter_map(|&g| self.groups.get(g).map(|g| g.transform)));
        out
    }

    pub fn group_objects(&self, g: usize) -> Vec<usize> {
        fn find(nodes: &[Node], g: usize) -> Option<&[Node]> {
            nodes.iter().find_map(|n| match n {
                Node::Group { group, children } if *group == g => Some(children.as_slice()),
                Node::Group { children, .. } => find(children, g),
                Node::Object(_) => None,
            })
        }
        fn collect(nodes: &[Node], out: &mut Vec<usize>) {
            for n in nodes {
                match n {
                    Node::Object(o) => out.push(*o),
                    Node::Group { children, .. } => collect(children, out),
                }
            }
        }
        let mut out = Vec::new();
        if let Some(children) = find(&self.tree, g) {
            collect(children, &mut out);
        }
        out.sort_unstable();
        out
    }

    pub fn push_object(&mut self, obj: Object3D, parent: Option<usize>) -> usize {
        self.normalize_tree();
        self.objects.push(obj);
        let i = self.objects.len() - 1;
        fn put(nodes: &mut [Node], g: usize, i: usize) -> bool {
            nodes.iter_mut().any(|n| match n {
                Node::Group { group, children } if *group == g => {
                    children.push(Node::Object(i));
                    true
                }
                Node::Group { children, .. } => put(children, g, i),
                Node::Object(_) => false,
            })
        }
        if !parent.is_some_and(|g| put(&mut self.tree, g, i)) {
            self.tree.push(Node::Object(i));
        }
        i
    }

    pub fn remove_object(&mut self, o: usize) {
        if o >= self.objects.len() {
            return;
        }
        self.objects.remove(o);
        fn fix(nodes: Vec<Node>, o: usize) -> Vec<Node> {
            nodes
                .into_iter()
                .filter_map(|n| match n {
                    Node::Object(i) if i == o => None,
                    Node::Object(i) => Some(Node::Object(if i > o { i - 1 } else { i })),
                    Node::Group { group, children } => Some(Node::Group { group, children: fix(children, o) }),
                })
                .collect()
        }
        self.tree = fix(std::mem::take(&mut self.tree), o);
        self.normalize_tree();
    }
}
```
Agregar `hidden`/`locked` a `Object3D` y `groups`/`tree` a `Model3D` (y a su `Default`:
`groups: Vec::new(), tree: Vec::new()`). En `chimp.rs`, junto a `m.shading.normalize();`:
`m.normalize_tree();`.

- [ ] **Step 4: Correr** `cargo test -p cs-core -p cs-io -p cs-3d -p cs-render -p cs-ui` → todo
  pasa (arreglar literales que no compilen).
- [ ] **Step 5: Commit** — `Modelos 3D: grupos, árbol del outliner, ocultar y bloquear (modelo)`.

### Task 2: La cadena de transforms en cs-3d (render, horneado, edición, pick)

**Files:**
- Modify: `crates/cs-3d/src/edit.rs` (`world_points` l.137, `pick` l.443, `box_select` l.496,
  `bounds` l.555, `model_bounds`, `edit_vertices`, `translate` l.617, `scale`)
- Modify: `crates/cs-3d/src/gizmo.rs:82-110` (`rotate_selection`)
- Modify: `crates/cs-3d/src/raster.rs` (`world_vertices` l.413, sus dos usos l.89 y l.751, el
  `mirrored` de l.752)
- Modify: `crates/cs-3d/src/topology.rs:461` (`out.objects.remove(o)` → `out.remove_object(o)`)
- Modify: `crates/cs-ui/src/canvas/model_edit.rs:445,1249,1365` (`world_points(&model.objects[o])`
  → `world_points(&model, o)`) y los tests que lo usan.

**Interfaces:**
- Consumes: `Model3D::object_chain`, `object_parents`, `object_hidden`, `object_locked`,
  `remove_object` (Task 1).
- Produces (cs-3d, `pub`):
```rust
pub fn object_to_world(model: &Model3D, o: usize, p: P3) -> P3;          // por la cadena
pub fn object_to_local(model: &Model3D, o: usize, w: P3, keep: P3) -> P3; // lo contrario
pub fn world_points(model: &Model3D, o: usize) -> Vec<P3>;                // CAMBIA de firma
pub fn parent_to_world(model: &Model3D, o: usize, p: P3) -> P3;           // solo los grupos
pub fn parent_to_local(model: &Model3D, o: usize, w: P3) -> P3;
pub fn pickable(model: &Model3D, o: usize) -> bool; // !object_hidden && !object_locked
```

- [ ] **Step 1: Tests que fallan** (en `edit.rs` `mod tests`; `cube_at` ya existe ahí):

```rust
fn in_group(offset: i32) -> Model3D {
    let mut m = Model3D::default();
    m.objects.push(cube_at(Vec3i::default()));
    m.groups.push(cs_core::Group3D {
        name: "g".into(),
        transform: Transform3D { position: Vec3i { x: offset, y: 0, z: 0 }, ..Default::default() },
        hidden: false, locked: false, open: true,
    });
    m.tree = vec![cs_core::Node::Group { group: 0, children: vec![cs_core::Node::Object(0)] }];
    m
}

#[test]
fn a_group_moves_its_objects_in_the_world() {
    let m = in_group(64);
    let pts = world_points(&m, 0);
    assert!(pts.iter().all(|p| p[0] >= 48.0 && p[0] <= 80.0), "{pts:?}");
    let w = object_to_world(&m, 0, [16.0, 0.0, 0.0]);
    assert_eq!(w, [80.0, 0.0, 0.0]);
    assert_eq!(object_to_local(&m, 0, w, [0.0; 3]), [16.0, 0.0, 0.0]);
}

#[test]
fn moving_an_object_inside_a_turned_group_follows_the_world() {
    let mut m = in_group(0);
    m.groups[0].transform.rotation.y = 900; // 90°: el x del grupo es el -z del mundo
    let sel = Selection { mode: SelectMode::Object, items: [Item::Object(0)].into() };
    let out = translate(&m, &sel, [16.0, 0.0, 0.0]);
    let before = object_to_world(&m, 0, [0.0; 3]);
    let after = object_to_world(&out, 0, [0.0; 3]);
    assert!((after[0] - before[0] - 16.0).abs() < 0.5, "{before:?} -> {after:?}");
    assert!((after[2] - before[2]).abs() < 0.5);
}

#[test]
fn hidden_and_locked_objects_are_not_picked() {
    let map = front_map(); // el ViewMap de Front de los tests de pick que ya hay
    let mut m = in_group(0);
    let at = map.to_screen([0.0, 0.0, 16.0]);
    assert!(pick(&m, &map, SelectMode::Object, at).is_some());
    m.groups[0].locked = true;
    assert!(pick(&m, &map, SelectMode::Object, at).is_none());
    m.groups[0].locked = false;
    m.objects[0].hidden = true;
    assert!(pick(&m, &map, SelectMode::Object, at).is_none());
    assert!(box_select(&m, &map, SelectMode::Object, (0.0, 0.0), (1e4, 1e4)).is_empty());
}
```
(Si `front_map()` no existe, construir el `ViewMap` como `front()` de
`crates/cs-ui/src/editor/model_edit_tests.rs`: `ViewMap { view: OrthoView::Front, center: [0.0;3], scale: 2.0, origin: (100.0, 100.0) }`.)

En `raster.rs` `mod tests`:
```rust
#[test]
fn lo_oculto_no_se_pinta_y_un_grupo_mueve_a_sus_hijos() {
    let mut m = model(vec![cube()]);
    m.objects[0].hidden = true;
    assert!(render_model(&input(&m, 16, &[ROJO], 1)).iter().all(|p| p.a == 0));
    let mut m = model(vec![cube()]);
    m.groups.push(cs_core::Group3D {
        name: "g".into(),
        transform: Transform3D { position: Vec3i { x: FIXED, y: 0, z: 0 }, ..Default::default() },
        hidden: false, locked: false, open: true,
    });
    m.tree = vec![cs_core::Node::Group { group: 0, children: vec![cs_core::Node::Object(0)] }];
    let img = render_model(&input(&m, 16, &[ROJO], 1));
    // El cubo (4..12) corrido una unidad (4 px) a la derecha: 8..16.
    assert_eq!(img[8 * 16 + 6].a, 0);
    assert_eq!(img[8 * 16 + 14], ROJO);
}
```

- [ ] **Step 2: Correr** `cargo test -p cs-3d` → no compila (`world_points` cambia de firma).

- [ ] **Step 3: Implementar.** En `edit.rs`:
```rust
pub fn object_to_world(model: &Model3D, o: usize, p: P3) -> P3 {
    model.object_chain(o).iter().fold(p, |p, t| to_world(t, p))
}
pub fn object_to_local(model: &Model3D, o: usize, w: P3, keep: P3) -> P3 {
    let chain = model.object_chain(o);
    // De afuera hacia adentro; `keep` solo sirve en el último (el objeto).
    let mut p = w;
    for (k, t) in chain.iter().enumerate().rev() {
        p = to_local(t, p, if k == 0 { keep } else { [0.0; 3] });
    }
    p
}
pub fn parent_to_world(model: &Model3D, o: usize, p: P3) -> P3 {
    model.object_chain(o).iter().skip(1).fold(p, |p, t| to_world(t, p))
}
pub fn parent_to_local(model: &Model3D, o: usize, w: P3) -> P3 {
    model.object_chain(o).iter().skip(1).rev().fold(w, |p, t| to_local(t, p, [0.0; 3]))
}
pub fn pickable(model: &Model3D, o: usize) -> bool {
    !model.object_hidden(o) && !model.object_locked(o)
}
pub fn world_points(model: &Model3D, o: usize) -> Vec<P3> {
    model.objects[o].mesh.vertices.iter().map(|&v| object_to_world(model, o, p3(v))).collect()
}
```
- `pick` / `box_select`: `for (o, obj) in model.objects.iter().enumerate().filter(|(o, _)| pickable(model, *o))` y `world_points(model, o)`.
- `bounds`: `object_to_world(model, o, …)`. `model_bounds`: solo los no ocultos:
  `(0..model.objects.len()).filter(|&o| !model.object_hidden(o)).flat_map(|o| world_points(model, o))`.
- `edit_vertices`: `let w = f(object_to_world(model, o, local)); … round3(object_to_local(model, o, w, local))`.
- `translate` (modo Object): por objeto, `let origin = parent_to_world(model, o, p3(t.position)); let moved = [origin[0]+d[0], origin[1]+d[1], origin[2]+d[2]]; t.position = round3(parent_to_local(model, o, moved));`.
- `scale` (modo Object): la posición igual que `translate` pero con `anchor + (origin - anchor) * f`
  en el mundo; el eje local con `local_axis` sobre la rotación del objeto (sin cambiar si hay
  grupos girados: aproximado, como ya dice el doc comment).
- `gizmo.rs` `rotate_selection` (modo Object): `let pos = rotate_point(parent_to_world(model, o, p3(t.position)), center, axis, tenths); t.position = round3(parent_to_local(model, o, pos));`.
- `raster.rs`: `world_vertices(model: &Model3D, o: usize, extra_yaw: i32) -> Vec<V3>`: toma
  `model.object_chain(o)`, le suma `extra_yaw` a `rotation.y` del **último** (el de más afuera:
  sin grupos, el objeto, como hoy), aplica `crate::edit::to_world` por la cadena y divide por
  `FIXED` (en `units`). `rasterize` y `face_outline`/`pick_*`: saltar `model.object_hidden(id)`.
  `mirrored`: XOR de los signos de escala de toda la cadena.
- `topology.rs:461`: `out.remove_object(o)`.
- cs-ui `canvas/model_edit.rs`: `cs_3d::world_points(&model, o)`; donde dibuja las
  wireframes (l.1365) saltar `model.object_hidden(o)`.

- [ ] **Step 4: Correr** `cargo test -p cs-3d -p cs-render -p cs-ui` → pasa.
- [ ] **Step 5: Commit** — `Modelos 3D: los grupos mueven a sus hijos; lo oculto no se ve ni se hornea; lo bloqueado no se elige`.

### Task 3: Las operaciones del editor respetan los grupos

**Files:**
- Modify: `crates/cs-ui/src/editor/model_ops.rs` (`add_primitive` l.139, `duplicate_model_objects`, `remove_object` l.~230)
- Modify: `crates/cs-ui/src/editor/model_ops.rs` o `editor.rs` donde está `set_model` (normalizar antes de ejecutar)
- Test: `crates/cs-ui/src/editor/model_tests.rs`

**Interfaces:**
- Consumes: `push_object`, `remove_object`, `object_parents`, `group_objects` (Task 1).
- Produces: `Editor::duplicate_group(g: usize) -> bool` (copia el grupo con todo lo de adentro, justo
  después del original; un paso de undo) — lo usa el outliner (Task 6).

- [ ] **Step 1: Tests que fallan:**
```rust
fn with_a_group(e: &mut Editor) {
    let mut m = e.model().unwrap().clone();
    m.groups.push(cs_core::Group3D { name: "g".into(), transform: Default::default(), hidden: false, locked: false, open: true });
    m.tree = vec![cs_core::Node::Group { group: 0, children: vec![cs_core::Node::Object(0)] }];
    assert!(e.set_model(m));
}

#[test]
fn a_duplicate_stays_in_the_group_of_its_original() {
    let mut e = editor_with_model();
    with_a_group(&mut e);
    assert!(e.duplicate_model_objects());
    let m = e.model().unwrap();
    assert_eq!(m.object_parents(1), vec![0]);
}

#[test]
fn removing_an_object_keeps_the_others_in_their_groups() {
    let mut e = editor_with_model();
    assert!(e.add_primitive(Primitive::Plane));
    with_a_group(&mut e); // el 0 en el grupo, el 1 en la raíz
    assert!(e.remove_object(0));
    let m = e.model().unwrap();
    assert_eq!(m.object_parents(0), Vec::<usize>::new());
    assert_eq!(m.group_objects(0), Vec::<usize>::new());
}

#[test]
fn duplicating_a_group_copies_it_with_its_objects() {
    let mut e = editor_with_model();
    with_a_group(&mut e);
    let steps = e.history.done_count();
    assert!(e.duplicate_group(0));
    let m = e.model().unwrap();
    assert_eq!(m.groups.len(), 2);
    assert_eq!(m.groups[1].name, "g copy");
    assert_eq!(m.group_objects(1), vec![1]);
    assert_eq!(e.history.done_count(), steps + 1);
}
```
- [ ] **Step 2: Correr** `cargo test -p cs-ui --lib model_tests` → fallan.
- [ ] **Step 3: Implementar.** `add_primitive`: `model.push_object(obj, parent)` con `parent` = el
  grupo de la fila activa (por ahora `None`; Task 5 lo conecta). `duplicate_model_objects`: por
  cada `o`, `let parent = model.object_parents(o).first().copied(); model.push_object(copy, parent)`.
  `remove_object`: `model.remove_object(i)` en vez de `objects.remove(i)`.
  `duplicate_group(g)`: clona `groups[g]` (nombre + " copy") como grupo nuevo `ng`, copia cada
  objeto de `group_objects(g)` con `push_object(…, Some(ng))` respetando subgrupos (recursivo:
  cada subgrupo se duplica igual dentro de `ng`), mete `Node::Group { group: ng, … }` justo
  después del original en el padre del original. `set_model`: `model.normalize_tree()` antes de
  ejecutar.
- [ ] **Step 4: Correr** `cargo test -p cs-ui --lib` → pasa.
- [ ] **Step 5: Commit** — `ChimpCAD: agregar, duplicar y quitar respetan los grupos`.

# Parte 2 — Outliner

### Task 4: Íconos provisionales del outliner en el skin

**Files:**
- Modify: `assets/ui/skin.aseprite` (vía Aseprite MCP `run_lua_script`)
- Modify: `assets/ui/README.md` (sección de íconos)
- Test: `crates/cs-ui/src/kit/tile.rs` `mod tests` (o el test de piezas del skin)

- [ ] **Step 1: Test que falla:**
```rust
#[test]
fn the_outliner_icons_are_in_the_skin() {
    let skin = skin();
    for name in ["folder", "eye", "eye_off", "lock", "unlock", "double_sided", "new_group"] {
        let piece = skin
            .piece(&format!("icon.outliner.{name}"), WidgetState::Normal)
            .unwrap_or_else(|| panic!("icon.outliner.{name}"));
        assert!(piece.size[0] <= 11 && piece.size[1] <= 11, "{name}: {:?}", piece.size);
    }
}
```
- [ ] **Step 2:** `cargo test -p cs-ui --lib the_outliner_icons` → falla.
- [ ] **Step 3: Dibujar** 9×9 cada uno con la tinta `color.ruler_tick` (136,110,106) y relleno
  `color.highlight` donde haga falta, en un hueco libre del skin (buscar con el script de
  `memory/gui-verification-workflow.md`: sin pixeles ni slices con 1 px de margen) y crear un slice
  por pieza. Formas: carpeta (pestañita arriba a la izquierda), ojo (almendra con pupila), ojo
  tachado (con la diagonal), candado cerrado y abierto, double sided (dos cuadros encimados), new
  group (carpeta con +). Documentar en `assets/ui/README.md` como provisionales.
- [ ] **Step 4:** test pasa.
- [ ] **Step 5: Commit** — `Skin: íconos provisionales del outliner`.

### Task 5: El editor: fila activa y operaciones del outliner

**Files:**
- Create: `crates/cs-ui/src/editor/outliner_ops.rs` (declarado en `editor.rs` con `mod outliner_ops;`)
- Test: `crates/cs-ui/src/editor/outliner_tests.rs`

**Interfaces — Produces:**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row { Object(usize), Group(usize) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop { Before(Row), After(Row), Into(usize /* grupo */) }
impl Editor {
    pub fn active_row(&self) -> Option<Row>;           // Group o, si no, Object(model_object)
    pub fn set_active_row(&mut self, row: Option<Row>, add: bool); // add = Shift
    pub fn toggle_hidden(&mut self, row: Row) -> bool;  // un paso de undo
    pub fn toggle_locked(&mut self, row: Row) -> bool;
    pub fn toggle_double_sided(&mut self, o: usize) -> bool;
    pub fn toggle_open(&mut self, g: usize);            // sin undo
    pub fn rename_row(&mut self, row: Row, name: String) -> bool;
    pub fn new_group(&mut self) -> bool;                // con lo elegido, o vacío
    pub fn move_row(&mut self, row: Row, to: Drop) -> bool;
    pub fn delete_row(&mut self, row: Row) -> bool;     // un grupo: con lo de adentro
}
```
La fila activa de grupo se guarda en un campo nuevo `Editor.model_group: Option<usize>`
(`model_object` sigue para objetos; elegir uno limpia el otro). `set_active_row` en modo Model
pone la selección 3D en modo Object con `Object(o)` o `group_objects(g)` (con `add`, los suma o
los quita). `add_primitive` mete lo nuevo en `model_group` o en el grupo de `model_object`.
`toggle_open`: cambia `groups[g].open` en `self.document` directo (sin `execute`), `revision += 1`.
`move_row`: rechaza meter un grupo en sí mismo o en un descendiente (`group_parents(target)`
contiene `g`, o `target == g`).

- [ ] **Step 1: Tests que fallan** (`outliner_tests.rs`, con `editor_with_model()` +
  `add_primitive(Plane)`): cada método un test — un paso de undo por toggle/rename/new_group/
  move/delete (`history.done_count()`), `toggle_open` sin paso, `new_group` con los dos objetos
  elegidos los mete en el grupo 0 en el lugar del primero, `move_row(Row::Group(0), Drop::Into(0))`
  devuelve `false`, `delete_row(Row::Group(0))` quita el grupo y sus objetos,
  `set_active_row(Some(Row::Group(0)), false)` deja la selección 3D con sus objetos.
- [ ] **Step 2:** `cargo test -p cs-ui --lib outliner_tests` → falla.
- [ ] **Step 3: Implementar** cada método sobre una copia del modelo y `self.named("<Label>", |e| e.set_model(m))`
  (labels: "Hide", "Show", "Lock", "Unlock", "Double Sided", "Rename", "New Group", "Move",
  "Delete"). Árbol: helpers privados `take_node(&mut Vec<Node>, row) -> Option<Node>` y
  `insert_node(&mut Vec<Node>, Drop, Node) -> bool`.
- [ ] **Step 4:** pasa; `cargo test -p cs-ui --lib`.
- [ ] **Step 5: Commit** — `ChimpCAD: operaciones del outliner (fila activa, ocultar, bloquear, grupos)`.

### Task 6: El outliner en el panel (en lugar de la tira de pestañas)

**Files:**
- Create: `crates/cs-ui/src/model_panel/outliner.rs` (`mod outliner;` en `model_panel.rs`)
- Modify: `crates/cs-ui/src/model_panel.rs` (`header()` → encabezado "Outliner" con Duplicate /
  New Group / +; la mitad de abajo muestra el outliner con scroll; quitar `object_tabs` y la tira)
- Modify: `crates/cs-ui/src/layout_tests.rs` (los tests de la tira de pestañas pasan a ser del
  outliner: `the_3d_panel_has_one_tab_per_object_with_the_icon_of_its_kind`,
  `the_tab_strip_has_no_model_tab_and_one_tab_per_object`, `the_object_is_removed_by_its_tab_mark…`,
  `the_duplicate_and_plus_buttons_sit_right_of_the_tab_arrows`, y los que buscan `node(&h, "Cube")`
  bajo la línea)

**Interfaces:** Consumes Task 5 (`Row`, `Drop`, métodos). Produces `outliner::show(ui, editor, skin, state) -> Option<…>` y `ModelPanelState.outliner: OutlinerState { rename: Option<(Row, String)>, drag: Option<Row> }`.

- [ ] **Step 1: Tests que fallan** en `layout_tests.rs` (con `model_panel_open(true)` y
  `add_object(&mut h, "Cylinder")`):
  - `the_outliner_lists_every_object_with_its_icon_eye_and_lock`: nodos "Cube", "Cylinder",
    "Hide Cube", "Lock Cube", "Double sided Cube"; los íconos `icon.object.cube`/`…cylinder`
    dibujados; el encabezado "Outliner" con "Duplicate", "New group", "Add object".
  - `clicking_a_row_chooses_it_and_shift_adds`: clic en "Cylinder" → `selected_object() == Some(1)`
    y la selección 3D `{Object(1)}`; Shift+clic en "Cube" → `{Object(0), Object(1)}`.
  - `the_eye_hides_in_one_step_and_the_render_changes`: clic en "Hide Cube" → `objects[0].hidden`,
    un paso, `model_pixels` cambia; otro clic lo vuelve.
  - `new_group_wraps_the_chosen_rows_and_its_triangle_folds_it`: elegir los dos, clic en "New
    group" → una fila "Group" con los dos adentro (sangría mayor: `node("Cube").left() >
    node("Group").left()`); clic en "Collapse Group" → las filas de adentro no se dibujan, sin
    paso de undo.
  - `double_click_renames_a_row`: doble clic en "Cube", escribir "Body", Enter → nombre "Body",
    un paso.
  - `dragging_a_row_onto_a_group_moves_it_inside`: con un grupo y "Cylinder" afuera, arrastrar
    "Cylinder" sobre la fila del grupo → `object_parents(1) == [0]`, un paso.
- [ ] **Step 2:** `cargo test -p cs-ui --lib outliner` → fallan.
- [ ] **Step 3: Implementar** `outliner.rs`: recorre `model.tree` (normalizado) y por fila,
  `ROW_H = 13` (como un ítem de menú), sangría `INDENT = 8` por nivel, ▸/▾ (`menus` dibuja
  flechas; o piezas `tabs.arrow.*` existentes) con nombre accesible "Expand <name>"/"Collapse
  <name>", el ícono (folder o `kind_icon`), el nombre (texto del panel; activa con el fondo de
  selección del kit `colors.selection`; elegidas en la selección 3D, resaltadas igual), y a la
  derecha `icon_toggle_at`-like de 9×9 sin marco con `kit::icon::paint` para ◧ (solo objetos), ojo
  y candado, nombres accesibles "Double sided <name>", "Hide <name>"/"Show <name>", "Lock
  <name>"/"Unlock <name>". Lo heredado (padre oculto/bloqueado) a media opacidad. Doble clic en
  el nombre: `kit::input` de texto en la fila (Enter `rename_row`, Esc cancela). Arrastre: con
  `Sense::drag()` sobre la fila; mientras arrastra, una línea de 1 px `colors.text` donde caería
  (mitad de arriba → `Before`, mitad de abajo → `After`, sobre un grupo en su tercio del medio →
  `Into`); al soltar `move_row`. Supr con el puntero sobre el outliner → `delete_row(active)`.
  En `model_panel.rs`: el encabezado reemplaza la tira (mismo `TABS_TOP`): texto "Outliner" a la
  izquierda y a la derecha `duplicate_button` (con fila activa de grupo llama `duplicate_group`),
  un `icon_button_at` "New group" (`icon.outliner.new_group`) y el `+`. Debajo: la fila de botones
  del objeto (Task 7 la mueve aquí; mientras, queda `selected()` como está) y el outliner en
  `scrolled(…)`.
- [ ] **Step 4:** `cargo test -p cs-ui --lib` → pasa (actualizar los tests viejos de la tira).
- [ ] **Step 5:** captura con `CHIMPSPRITE_DEMO_MODEL=1` (ver memoria gui-verification-workflow)
  y revisar que nada se corte.
- [ ] **Step 6: Commit** — `ChimpCAD: outliner con grupos, ojo, candado y double sided en lugar de las pestañas`.

# Parte 3 — Barrita del objeto

### Task 7: La barrita bajo el encabezado y el panel reacomodado

**Files:**
- Create: `crates/cs-ui/src/canvas/object_bar.rs` (`mod object_bar;` en `canvas.rs`)
- Modify: `crates/cs-ui/src/canvas/model_edit.rs` (`header_split` y donde se reparte el área en
  modo Model: la barrita debajo del encabezado; las vistas debajo)
- Modify: `crates/cs-ui/src/canvas/texture_mode.rs` (la barrita y debajo la barra del pincel)
- Modify: `crates/cs-ui/src/model_panel.rs` (quitar Position / Rotation / Scale / Double sided de
  `selected()`; la fila de botones del objeto arriba del outliner)
- Modify: `crates/cs-ui/src/editor/outliner_ops.rs` (transform de la fila activa)
- Test: `crates/cs-ui/src/layout_tests.rs`

**Interfaces:**
- Consumes: `active_row()` (Task 5), `bar_fields::number(ui, look, salt, &mut i32, range, show_unit)`
  y `options_bar_panel` (barra de opciones).
- Produces:
```rust
impl Editor {
    /// La transform de la fila activa, si se puede editar: hay fila activa, no está bloqueada y la
    /// selección 3D no tiene otros objetos que los suyos.
    pub fn active_transform(&self) -> Option<Transform3D>;
    /// La cambia en un paso agrupado (`set_model_grouped` con `group`).
    pub fn set_active_transform(&mut self, t: Transform3D, group: egui::Id) -> bool;
}
pub(super) fn object_bar(ui: &mut egui::Ui, rect: Rect, editor: &mut Editor); // en canvas/object_bar.rs
```

- [ ] **Step 1: Tests que fallan:**
  - `the_object_bar_sits_under_the_chimpcad_header_in_both_modes`: `canvas.cad_header()`; un texto
    "Position" debajo del encabezado y arriba de los cuadrantes (Model) o del render (Texture); en
    Texture "Brush Size" debajo de "Position"; el encabezado en el mismo lugar en los dos modos.
  - `dragging_a_position_field_moves_the_object_in_one_step`: arrastrar el campo X de Position
    (nodo con el valor) 20 px → `objects[0].transform.position.x` cambia; un paso de undo.
  - `with_a_group_active_the_bar_edits_the_group`: fila activa `Row::Group(0)`, escribir en X →
    `groups[0].transform.position.x` cambia.
  - `the_bar_is_disabled_without_an_active_row_or_when_locked`: sin fila activa los nueve campos
    deshabilitados; con el objeto bloqueado también.
  - `the_object_tab_has_no_transform_rows_any_more`: en el panel no hay textos "Position" /
    "Rotation" / "Scale" / "Double sided"; la fila "Extrude" está arriba del outliner.
- [ ] **Step 2:** `cargo test -p cs-ui --lib object_bar` → fallan.
- [ ] **Step 3: Implementar** `object_bar`: `options_bar_panel(style, skin).show(&mut child, …)`
  como la barra del pincel en `texture_mode.rs`; adentro, `row_scroll_area`, y por grupo un
  `bar_fields::label` + tres `bar_fields::number` (X/Y/Z con su letra como etiqueta corta),
  Position en 1/16 (`-POSITION_MAX..=POSITION_MAX`), Rotation en grados (`wrap_angle(r)/10`, se
  guarda `*10`), Scale en % (`/10`, se guarda `*10`, `1..=SCALE_MAX`) — los mismos límites que hoy
  en `model_panel.rs` (moverlos a un lugar común, p. ej. `crate::editor::model_limits`). Sin
  `active_transform()`, campos vacíos y deshabilitados. Un arrastre = un paso
  (`set_model_grouped` + `end_group_of` al soltar, como `selected()` hoy). En `model_edit.rs` y
  `texture_mode.rs`: reservar la barrita (`options_bar_height`) debajo de la línea del encabezado,
  dibujarla después del fondo (como la del pincel). Panel: borrar los ejes y Double sided de
  `selected()`; la fila `topology_buttons`/`uv_buttons` va arriba del outliner.
- [ ] **Step 4:** `cargo test -p cs-ui --lib` → pasa (actualizar tests de ejes del panel:
  `axis_row`, `axis_rhythm`, `every_model_panel_control_fits_beside_the_scroll_bar`, etc., que
  pasan a la barrita o se borran si ya no aplican).
- [ ] **Step 5:** captura en Model y Texture; nada cortado.
- [ ] **Step 6: Commit** — `ChimpCAD: la barrita del objeto bajo el encabezado; el panel con el outliner`.

### Task 8: Cierre

- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check` y el check de wasm de CI:
  `cargo check --target wasm32-unknown-unknown -p cs-core -p cs-3d -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite`.
- [ ] Actualizar el doc comment de arriba de `model_panel.rs` (ya no hay tira de pestañas).
- [ ] Commit — `ChimpCAD outliner: cierre`.
