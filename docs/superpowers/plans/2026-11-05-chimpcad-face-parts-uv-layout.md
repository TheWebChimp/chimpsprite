# Caras completas y textura por objeto y por cara — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Elegir y editar caras completas (la tapa del cilindro como una sola cara) y que cada
objeto y cada cara tenga su propio lugar en la textura, con cajas que se mueven y escalan, lugar
libre al agregar/duplicar (la textura crece si hace falta) y un botón Pack UVs.

**Architecture:** `Face.part: Option<u32>` marca a qué cara completa pertenece cada cara de la
malla; la selección en modo Face se expande a partes completas en los puntos donde se elige. Las
UVs siguen siendo por vértice de cada cara (las cajas son la caja de las UVs elegidas). Un módulo
de acomodo en cs-3d (`uv_layout`) encuentra lugar libre y empaqueta; el editor junta el cambio del
modelo y el de los pixeles de la textura en un `DocCommand::Batch` (un paso de undo).

**Tech Stack:** Rust workspace (cs-core, cs-3d, cs-commands, cs-ui), egui.

Spec: `docs/superpowers/specs/2026-11-05-chimpcad-face-parts-uv-layout-design.md`.

## Global Constraints

- Comentarios en español, como el código de alrededor; nombres en inglés. Repo `eol=lf`.
- Nunca `git stash`, `git checkout -- …`, `git restore`, `git reset`, `git clean`; no tocar `.design/`.
- Cargo con el toolchain aislado y a baja prioridad:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH; export CARGO_BUILD_JOBS=8; cmd //c "start /low /b /wait cargo test -p <crate>"`.
- Antes de cada commit: `cargo fmt -p <crates>`, sus tests y `cargo clippy -p <crates> --all-targets -- -D warnings`.
- Commits en español terminados en `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Archivos viejos se leen igual (`#[serde(default)]`; `part: None` = la cara es su propia parte).
- Cada gesto o botón = un paso de undo. Mover o escalar UVs no mueve pixeles; agregar, duplicar
  y Pack sí colocan/copian pixeles cuando lo dice la spec.
- Cubo 6 partes, plano 1, cilindro 3 (lado 0, tapa de arriba 1, tapa de abajo 2), cono 2 (lado 0,
  base 1), esfera 1.
- Aire entre bloques en la textura: 1 px. Crecer: siguiente lado de `TEXTURE_SIDES` (64→128→256→512).

---

### Task 1: `Face.part`, las partes de las primitivas y la topología

**Files:**
- Modify: `crates/cs-core/src/model.rs` (`Face` l.22; tests)
- Modify: `crates/cs-3d/src/primitives.rs` (`face()` l.73 y cada primitiva; tests)
- Modify: `crates/cs-3d/src/topology.rs` (Subdivide, Extrude)
- Modify: todos los literales `Face { verts, uvs }` (`grep -rn "Face {" crates --include=*.rs`): agregar `part: None`.

**Interfaces — Produces:**
```rust
// cs-core
pub struct Face { pub verts: Vec<u32>, pub uvs: Vec<(i32, i32)>, #[serde(default)] pub part: Option<u32> }
impl Mesh {
    /// Las caras de la cara completa de `f` (con el mismo `part`; sin `part`, solo `f`), en orden.
    pub fn part_faces(&self, f: usize) -> Vec<usize>;
    /// El siguiente id de parte libre (máximo + 1, o 0).
    pub fn next_part(&self) -> u32;
}
```

- [ ] **Step 1: Tests que fallan.** cs-core: `part_faces` con `part: None` devuelve `[f]`; con
  tres caras de `part: Some(1)` y una de `Some(0)` devuelve las tres; `next_part`; un JSON de
  `Face` sin `part` se lee con `None`. cs-3d (`primitives.rs` tests):
```rust
#[test]
fn las_primitivas_tienen_sus_caras_completas() {
    let parts = |p: Primitive| {
        let m = primitive_mesh(&p);
        let mut ids: Vec<u32> = m.faces.iter().map(|f| f.part.expect("con parte")).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    assert_eq!(parts(Primitive::Cube), vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(parts(Primitive::Plane), vec![0]);
    assert_eq!(parts(Primitive::Cylinder { sides: 8 }), vec![0, 1, 2]);
    assert_eq!(parts(Primitive::Cone { sides: 8 }), vec![0, 1]);
    assert_eq!(parts(Primitive::Sphere { sides: 8, rings: 4 }), vec![0]);
    // La tapa de arriba del cilindro: sus 8 triángulos.
    let m = primitive_mesh(&Primitive::Cylinder { sides: 8 });
    let top = m.faces.iter().position(|f| f.part == Some(1)).unwrap();
    assert_eq!(m.part_faces(top).len(), 8);
}
```
  topology tests: Subdivide de una cara con `part: Some(3)` → las 4 hijas con `Some(3)`;
  Extrude de la cara de arriba del cubo → la cara extruida conserva su parte y cada lado nuevo
  tiene una parte distinta, ninguna repetida con las de antes.
- [ ] **Step 2:** correr → fallan.
- [ ] **Step 3: Implementar.** `face(verts, uvs)` en primitives pasa a `face(verts, uvs, part)`;
  cubo: `part` = índice de la cara (0..6); plano 0; cilindro: lados 0, `cap_faces(.., top)`
  con 1 (arriba) / 2 (abajo); cono: lados 0, base 1 (`cap_faces` recibe el `part`); esfera 0.
  Topología: en Subdivide copiar `part` de la madre; en Extrude, la cara movida conserva el suyo y
  los lados nuevos toman `mesh.next_part()` uno por uno.
- [ ] **Step 4:** `cargo test -p cs-core -p cs-3d -p cs-io -p cs-render -p cs-ui` pasa.
- [ ] **Step 5: Commit** — `Modelos 3D: caras completas (part) en las primitivas y la topología`.

### Task 2: Elegir caras completas

**Files:**
- Modify: `crates/cs-3d/src/edit.rs` (`pick` y `box_select` en modo Face; helper)
- Modify: `crates/cs-ui/src/editor/texture_mode.rs` (`pick_model_face` l.113)
- Modify: `crates/cs-ui/src/editor/model_edit.rs` (donde un clic o Shift + clic en modo Face agrega o quita de la selección)
- Modify: el resaltado de caras elegidas en las vistas orto (`crates/cs-ui/src/canvas/model_edit.rs`, buscar `Item::Face` en el dibujo)

**Interfaces — Produces (cs-3d, pub):**
```rust
/// Los `Item::Face` de toda la cara completa de `(o, f)`.
pub fn part_items(model: &Model3D, o: usize, f: usize) -> Vec<Item>;
/// `sel` con cada cara elegida extendida a su cara completa (solo en modo Face).
pub fn expand_parts(model: &Model3D, sel: &Selection) -> Selection;
/// Las aristas del borde de la cara completa (las que no comparten dos caras de la parte).
pub fn part_outline(model: &Model3D, o: usize, f: usize) -> Vec<(u32, u32)>;
```

- [ ] **Step 1: Tests que fallan.** cs-3d: `pick` en modo Face sobre un gajo de la tapa del
  cilindro (vista Top) devuelve una cara de esa parte y `expand_parts` de esa selección tiene los 8
  `Item::Face` de la tapa; `box_select` en modo Face solo devuelve caras cuyas partes caen enteras
  dentro; `part_outline` de la tapa da 8 aristas (el anillo, sin las que van al centro). cs-ui
  (layout_tests, con un cilindro agregado y modo Face): clic en un gajo de la tapa en Top →
  `selection.items` = los 8 de la tapa; Shift + clic en otro gajo de la misma tapa → la tapa se
  quita entera; en modo Texture, clic en el render sobre la tapa → `selected_faces()` = sus 8.
- [ ] **Step 2:** fallan.
- [ ] **Step 3: Implementar.** `pick` sigue devolviendo una cara; quien elige (clic, Shift + clic,
  la caja, `pick_model_face`) agrega o quita `part_items` en bloque. `box_select` filtra en modo
  Face las partes no enteras y devuelve las caras de las partes enteras. El resaltado de caras en
  las vistas orto dibuja `part_outline` (una vez por parte) en vez de cada cara.
- [ ] **Step 4:** `cargo test -p cs-3d -p cs-ui` pasa.
- [ ] **Step 5: Commit** — `ChimpCAD: elegir caras completas (la tapa del cilindro de un clic)`.

### Task 3: Las cajas de UVs con asas

**Files:**
- Modify: `crates/cs-ui/src/editor/model_edit.rs` (`UvOp`, l.308)
- Modify: `crates/cs-ui/src/editor/texture_mode.rs` (`start_uv_op`/`update_uv_op`, y qué caras abarca la caja)
- Modify: `crates/cs-ui/src/canvas/uv_overlay.rs` (dibujo de la caja y sus asas, hit test, gesto)
- Test: `crates/cs-ui/src/layout_tests.rs` (helpers `with_the_front_face_chosen`, `front_uvs`, `texel`)

**Interfaces — Produces:**
```rust
// UvOp gana:
Scale { handle: (i8, i8) /* -1, 0, 1 por eje: la esquina/lado */, from: (f32, f32), bounds: ((i32, i32), (i32, i32)) },
impl Editor {
    /// Las caras que abarca la caja de UVs: con caras elegidas (modo Face), esas; en modo Object
    /// (o con la fila de un objeto/grupo activa en el outliner), todas las de esos objetos.
    pub fn uv_box_faces(&self) -> Vec<(usize, usize)>;
}
```

- [ ] **Step 1: Tests que fallan:**
  - `dragging_a_corner_handle_scales_the_uvs_in_whole_pixels`: con la cara de enfrente (caja
    0..16), arrastrar el asa de abajo a la derecha 8 texels → UVs en 0..24 (enteros), un paso de undo.
  - `shift_on_a_side_handle_scales_proportionally`.
  - `with_an_object_chosen_the_box_covers_all_its_faces`: elegir el cubo en modo Object (Texture),
    arrastrar dentro de la caja con Move → todas sus caras se mueven igual.
  - `handles_drag_with_the_pencil_too`: con el lápiz, arrastrar un asa escala y no pinta.
- [ ] **Step 2:** fallan.
- [ ] **Step 3: Implementar.** Caja = `cs_3d::uv_bounds(model, &uv_box_faces())`; 8 asas de 5×5
  (como los puntos, `UV_VERTEX_PX`) en esquinas y medios de lado, dibujadas igual que los puntos.
  Hit test: asas antes que puntos, puntos antes que el interior. `UvOp::Scale` escala con
  `cs_3d::scale_uvs` respecto del lado/esquina opuesta, factor tal que el borde arrastrado cae en
  el pixel entero bajo el puntero (mínimo 1 px); con Shift en una esquina, el mismo factor en los
  dos ejes; en un lado, Shift escala también el otro eje por el mismo factor. Con herramientas de
  pintar las asas se arrastran como los puntos (umbral de arrastre; un clic es de la herramienta).
- [ ] **Step 4:** `cargo test -p cs-ui --lib` pasa.
- [ ] **Step 5: Commit** — `ChimpCAD: caja de UVs con asas (objeto o caras completas)`.

### Task 4: Lugar libre al agregar y duplicar

**Files:**
- Create: `crates/cs-3d/src/uv_layout.rs` (declarar en `lib.rs`, `pub use` como `uv`)
- Modify: `crates/cs-ui/src/editor/model_ops.rs` (`add_primitive`, `duplicate_model_objects`, `duplicate_group`)
- Test: `crates/cs-3d/src/uv_layout.rs` tests; `crates/cs-ui/src/editor/model_tests.rs`

**Interfaces — Produces (cs-3d):**
```rust
pub type UvRect = (i32, i32, i32, i32); // x, y, w, h (w, h >= 1)
/// La caja de todas las UVs del objeto `o` (None si no tiene caras).
pub fn object_uv_rect(model: &Model3D, o: usize) -> Option<UvRect>;
/// El primer lugar (de arriba a la izquierda, en filas) de `w`×`h` dentro de `side`×`side` que no
/// toca ninguna de `taken` con `MARGIN` (1 px) de aire. None si no cabe.
pub fn find_free(taken: &[UvRect], w: i32, h: i32, side: i32) -> Option<(i32, i32)>;
/// `model` con las UVs del objeto `o` corridas para que su caja empiece en `at`.
pub fn place_object(model: &Model3D, o: usize, at: (i32, i32)) -> Model3D;
pub const MARGIN: i32 = 1;
```
Editor (privado): `fn place_new_objects(&mut self, model: &mut Model3D, new: &[usize]) -> (u32 /* lado final */, Vec<(UvRect, (i32, i32))> /* de dónde a dónde copiar pixeles */)`.

- [ ] **Step 1: Tests que fallan.** cs-3d: `find_free` en una textura vacía → (0,0); con un rect
  ocupado (0,0,48,32) un 16×16 cae en (49,0); cuando no cabe → None. cs-ui:
  - `two_cubes_do_not_share_texture`: agregar un segundo cubo → `object_uv_rect` de los dos no se
    tocan.
  - `without_room_the_texture_grows_and_keeps_its_pixels`: llenar la 64×64 con objetos hasta que
    no quepa uno más → al agregarlo la textura pasa a 128×128, los pixeles de antes siguen en su
    lugar y el nuevo no se encima; un solo paso de undo (deshacer vuelve a 64 y quita el objeto).
  - `a_duplicate_gets_its_own_place_and_a_copy_of_its_pixels`: pintar un pixel en la cara de
    enfrente del cubo, duplicar → la copia tiene UVs en otro lugar y la textura tiene ese pixel
    también en el lugar nuevo; el render de la copia se ve igual que el del original.
- [ ] **Step 2:** fallan.
- [ ] **Step 3: Implementar.** `add_primitive`: después de `push_object`, `find_free` con las cajas
  de los demás objetos (`object_uv_rect`); si no cabe, probar los lados siguientes de
  `TEXTURE_SIDES` hasta que quepa (si ni en 512 cabe, dejarlo donde está); `place_object`. Todo
  en un `DocCommand::Batch`: el `ResizeCanvas` de la textura (como `set_texture_size`) si creció,
  y el `SetModel` del modelo (el comando que usa `set_model`), con `self.named("Add Object", …)`.
  Duplicar: igual, y por cada copia agregar al Batch el copiado de pixeles de su caja vieja a la
  nueva en la textura (`Command::DrawPixels` con los pixeles leídos del sprite de la textura, en
  su capa y frame 0), como `"Duplicate Object"` / `"Duplicate Group"`.
- [ ] **Step 4:** `cargo test -p cs-3d -p cs-ui` pasa.
- [ ] **Step 5: Commit** — `ChimpCAD: cada objeto nuevo o duplicado con su lugar en la textura`.

### Task 5: Pack UVs

**Files:**
- Modify: `crates/cs-3d/src/uv_layout.rs` (`pack`)
- Modify: `crates/cs-ui/src/editor/texture_mode.rs` (`Editor::pack_uvs`)
- Modify: `crates/cs-ui/src/model_panel.rs` (botón "Pack UVs" en la fila `uv_buttons`)
- Test: `uv_layout.rs` tests; `crates/cs-ui/src/editor/texture_mode_tests.rs`; layout_tests

**Interfaces — Produces:**
```rust
// cs-3d
/// Un lugar por objeto (los bloques, de mayor a menor área; empates por índice) sin encimes con
/// MARGIN, en el menor lado de `sides` donde quepan todos. None si no caben ni en el mayor.
pub fn pack(model: &Model3D, sides: &[u32]) -> Option<(u32, Vec<(usize, (i32, i32))>)>;
// cs-ui
impl Editor { pub fn pack_uvs(&mut self) -> bool; }
```

- [ ] **Step 1: Tests que fallan.** cs-3d: `pack` de dos cubos encimados (las UVs de siempre) →
  dos lugares que no se tocan, dentro de 64. cs-ui:
  - `pack_separates_overlapping_objects_and_keeps_how_they_look`: dos cubos con las UVs de siempre
    (encimados, pintados) → `pack_uvs()` → cajas que no se tocan; el render del modelo
    (`cs_3d::render_model` con la textura) es igual antes y después; un paso de undo.
  - `pack_grows_the_texture_when_needed`.
  - El botón "Pack UVs" está en la fila de Rotate 90° / Flip en modo Texture y llama a `pack_uvs`.
- [ ] **Step 2:** fallan.
- [ ] **Step 3: Implementar.** `pack_uvs`: `pack(model, TEXTURE_SIDES desde el actual)`; leer
  primero los pixeles de la caja vieja de cada objeto (del sprite actual), luego en un
  `DocCommand::Batch`: crecer la textura si hace falta, borrar nada (los pixeles viejos pueden
  quedar), escribir cada bloque en su lugar nuevo (`DrawPixels`) y `SetModel` con las UVs corridas
  (`place_object` por objeto). `self.named("Pack UVs", …)`.
- [ ] **Step 4:** pasa.
- [ ] **Step 5: Commit** — `ChimpCAD: Pack UVs (sin encimes, llevando los pixeles)`.

### Task 6: Cierre

- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, y el check de wasm:
  `cargo check --target wasm32-unknown-unknown -p cs-core -p cs-3d -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite`.
- [ ] Captura del modo Texture con un cilindro elegido (tapa como cara completa con su caja y
  asas) para revisar que nada se corte.
- [ ] Commit si hubo cambios — `ChimpCAD caras completas y UVs: cierre`.
