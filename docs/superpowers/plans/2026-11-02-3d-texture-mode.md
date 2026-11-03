# Modelos 3D, parte 3 (modo Textura): plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** La pestaña de un modelo gana un modo *Texture* como el de picoCAD 2: el modelo arriba
(clic elige caras), la textura abajo (pintar con las herramientas de siempre y editar las UVs de
las caras elegidas), y el tamaño de la textura ajustable.

**Architecture:** Funciones puras de UV en `cs-3d/src/uv.rs` y `pick_face` en el rasterizador.
`ModelEdit` gana `mode: CadMode` y la vista (zoom/desplazamiento) de la textura. En *Texture* el
`Editor` deja `active` = la textura del modelo del workspace (las herramientas pintan ahí) y el
lienzo dibuja arriba el render del modelo y abajo la textura por el camino normal de un solo
artboard, recortado a su rectángulo.

**Tech Stack:** Rust 2024, egui 0.36, el UI kit propio.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-02-3d-texture-mode-design.md`.
- Acomodo de *Texture*: modelo **arriba**, textura **abajo** (como la captura de picoCAD).
- Selección en *Texture*: siempre modo Face; contornos de caras elegidas en amarillo PICO-8
  `#ffec27` sobre el render; contorno UV blanco con borde oscuro de 1 px abajo.
- UVs en pixeles enteros de la textura (`Face.uvs: Vec<(i32, i32)>`); cada gesto de UV es un paso
  de undo (`SetModel` con vista previa, como la 2A); pintar usa los comandos de siempre.
- Texture size: 16, 32, 64, 128, 256, 512; redimensiona anclado arriba a la izquierda; las UVs no
  cambian.
- Pixel art: todo lo nuevo se dibuja alineado a la rejilla de pixeles de la UI (como las vistas
  orto); componentes del kit, no piezas sueltas.
- Textos de UI en inglés ("Model", "Texture", "UV", "Rotate 90°", "Flip H", "Flip V", "Texture
  size", "No texture"); comentarios en español como el código vecino.
- Nunca `git checkout -- …`, `git restore`, `git stash`, `git reset --hard`, `git clean`; solo
  `git add <archivos propios>`; LF; no tocar `.design/`.
- Toolchain (Git Bash): `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=8`;
  cargo siempre a prioridad baja: `cmd //c "start /low /b /wait cargo test -p <crate> <filtro>"`.
  `cargo fmt --all` y `cargo clippy --workspace --all-targets` sin avisos; commits en español
  terminados con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

### Task 1: `pick_face` y las operaciones de UV (`cs-3d`)

**Files:** Modify `crates/cs-3d/src/raster.rs` (guardar en el `Raster` también el índice de cara
por pixel, junto a `ids`); Create `crates/cs-3d/src/uv.rs`; `crates/cs-3d/src/lib.rs` lo exporta.

**Interfaces — Produces:**
```rust
/// La cara (objeto, índice de cara) que se ve en el pixel `(x, y)`, si hay alguna.
pub fn pick_face(input: &RenderInput, x: u32, y: u32) -> Option<(usize, usize)>;
// uv.rs — todas sobre las caras `faces: &[(usize, usize)]` de `model`, devolviendo un modelo nuevo:
pub type UvPoint = (i32, i32);
pub fn uv_bounds(model: &Model3D, faces: &[(usize, usize)]) -> Option<(UvPoint, UvPoint)>; // (min, max) inclusivo de los vértices UV
pub fn move_uvs(model: &Model3D, faces: &[(usize, usize)], d: UvPoint) -> Model3D;
/// Escala respecto a `anchor` por `(fx, fy)` y redondea cada vértice al pixel más cercano.
pub fn scale_uvs(model: &Model3D, faces: &[(usize, usize)], anchor: (f32, f32), f: (f32, f32)) -> Model3D;
pub fn move_uv_vertex(model: &Model3D, face: (usize, usize), vertex: usize, to: UvPoint) -> Model3D;
/// Un cuarto de vuelta (en el sentido del reloj en pantalla) alrededor del centro de la caja de cada cara.
pub fn rotate_uvs_90(model: &Model3D, faces: &[(usize, usize)]) -> Model3D;
pub fn flip_uvs(model: &Model3D, faces: &[(usize, usize)], horizontal: bool) -> Model3D; // respecto al centro de la caja de cada cara
```
Las caras o índices que no existen se ignoran (sin pánico).

- [ ] **Step 1: Pruebas que fallan:** `pick_face` en un cubo de frente (32×32, ortográfica Front)
  da la cara de enfrente (la de `z+`, como en las pruebas de `pick_object`) en el centro y `None`
  en una esquina vacía. UV: `uv_bounds` de una cara; `move_uvs` (+3, −2); `scale_uvs` ×2 con el
  ancla en la esquina menor (los vértices de la esquina mayor se alejan al doble, redondeo);
  `move_uv_vertex` solo cambia ese vértice; `rotate_uvs_90` cuatro veces = original y una vez sobre
  un rectángulo 4×2 da uno 2×4 con el mismo centro (redondeo consistente); `flip_uvs` dos veces =
  original y una vez invierte el orden en x (o y); caras inexistentes no truenan.
- [ ] **Step 2:** `cargo test -p cs-3d` → fallan.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** Verde, clippy de `cs-3d`.
- [ ] **Step 5: Commit** — "Modelos 3D: elegir caras en el render y operaciones de UV".

### Task 2: El modo en el editor (`CadMode`, activo = textura)

**Files:** `crates/cs-ui/src/editor/model_edit.rs`, `crates/cs-ui/src/editor/workspace.rs`,
`crates/cs-ui/src/editor.rs` (`ensure_valid`), pruebas en
`crates/cs-ui/src/editor/model_edit_tests.rs` o `workspace_tests.rs`.

**Interfaces — Produces:**
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CadMode { #[default] Model, Texture }
// ModelEdit: pub mode: CadMode; pub texture_view: Option<TextureView> (zoom entero y desplazamiento de la textura de abajo; None = encuadrar)
impl Editor {
    pub fn cad_mode(&self) -> CadMode;
    /// Cambia de modo: en Texture activa la textura del modelo del workspace (si tiene) y pasa la
    /// selección a modo Face; en Model vuelve a activar el modelo. No toca el documento.
    pub fn set_cad_mode(&mut self, mode: CadMode);
    /// El modelo del workspace (en los dos modos), aunque el activo sea su textura.
    pub fn workspace_model(&self) -> Option<&Model3D>;
    pub fn selected_faces(&self) -> Vec<(usize, usize)>;
    pub fn set_texture_size(&mut self, side: u32) -> bool;   // ResizeCanvas anclado arriba-izquierda, un paso de undo
}
```
- `model_edit_on()` vale en los dos modos mientras el workspace sea un modelo; las funciones de la
  2A que leen `self.model()` usan `workspace_model()` (el activo puede ser la textura).
- `ensure_valid`: con workspace de modelo y modo Texture, el activo válido es la textura del modelo;
  si el modelo pierde su textura (undo), vuelve a Model.
- Salir del workspace o entrar a otro modelo deja el modo en Model.
- Las ediciones de UV del editor: `start_uv_op`/`update`/`finish`/`cancel` con vista previa (como
  la 2A: el documento solo cambia al soltar), y `rotate_selected_uvs()`, `flip_selected_uvs(h)`,
  cada una un paso de undo. `UvOp { Move { from, start }, Scale { handle, bounds }, Vertex { face,
  vertex, from } }` en coordenadas de pixel de la textura (`f32`).
- [ ] **Steps:** pruebas (cambiar de modo no deja undo; Texture activa la textura y la selección
  pasa a Face; pintar un pixel en Texture cambia la textura; Model vuelve al modelo; un Move de UV
  es un paso de undo y cancelar ninguno; rotate/flip; Texture size 64→128 conserva los pixeles y
  las UVs; quitar la textura por undo regresa a Model) → fallan → implementar → verde (todo
  `cs-ui`) → commit "Modo Textura: el editor pinta en la textura del modelo".

### Task 3: El lienzo en modo Texture (acomodo, render arriba, textura abajo)

**Files:** `crates/cs-ui/src/canvas/model_edit.rs` (o un `canvas/texture_mode.rs` nuevo),
`crates/cs-ui/src/canvas.rs`, `crates/cs-ui/src/canvas/input.rs`.

- En el workspace de un modelo con `cad_mode() == Texture`: el área (la misma de la 2A, a la
  izquierda del panel) se parte arriba/abajo a la mitad en puntos enteros, con una línea de 1 punto.
- **Arriba:** el render del modelo (`model_render` a la resolución del artboard, escala entera más
  grande que quepa, centrado, fondo `PERSP_BG`), con el contorno amarillo de las caras elegidas
  dibujado en pixeles del render (rasterizado como las vistas orto: Bresenham sobre la imagen o una
  capa encima a la misma escala). Orbitar, objetivo y Ctrl + rueda como Persp; clic → `pick_face`
  (Shift agrega/quita; vacío limpia).
- **Abajo:** la textura (artboard activo) por el camino normal del lienzo de un solo artboard,
  recortado al rectángulo de abajo, con su zoom entero/desplazamiento (`texture_view`; al entrar
  encuadra). Herramientas, cursor, cuadrícula de pixeles y atajos de pixeles funcionan ahí. Sin
  textura: "No texture" y un botón **New Texture**.
- **Botones Model / Texture** (texto, kit) arriba a la derecha del área del workspace, en los dos
  modos; el activo encendido.
- [ ] **Steps:** pruebas de layout (los botones existen y cambian de modo; en Texture el área se
  parte arriba/abajo a la izquierda del panel; clic en la cara de enfrente del render la elige;
  pintar con el lápiz abajo cambia la textura; la rueda abajo cambia el zoom de la textura y no el
  de las vistas orto) → fallan → implementar → verde → captura (`CHIMPSPRITE_DEMO_MODEL=1` y el modo
  Texture) → commit "Modo Textura: el modelo arriba y la textura abajo".

### Task 4: Las UVs abajo (contornos, asas, gestos) y el panel UV

**Files:** `crates/cs-ui/src/canvas/…` (dibujo y gestos de UV sobre la textura),
`crates/cs-ui/src/model_panel.rs` (sección *UV*), `crates/cs-ui/src/lib.rs` (si hace falta la
confirmación de achicar).

- Contorno UV de cada cara elegida (blanco con borde oscuro de 1 px) y sus vértices (3 px); caja
  con 8 asas (las que pueden cambiar algo, como la 2A), todo en pixeles de la UI alineados.
- Gestos (spec §3, §3.1): con Move o con Alt: dentro de un contorno → mover; asa → escalar
  (Shift proporcional); vértice UV → moverlo. Umbral de arrastre de 3 puntos (un clic no cambia
  nada). Cursor de mover sobre contornos/asas en esos casos.
- Panel 3D, en modo Texture: sección **UV** con **Rotate 90°**, **Flip H**, **Flip V**
  (deshabilitados sin caras elegidas) y **Texture size** (select 16…512; achicar con UVs fuera pide
  confirmación). En modo Model la sección no aparece.
- [ ] **Steps:** pruebas (arrastrar dentro del contorno con Move mueve las UVs en un paso de undo;
  Alt + arrastrar con el lápiz mueve en vez de pintar; un asa de esquina escala; un vértice UV se
  mueve solo; los botones Rotate/Flip; Texture size 128 desde el panel) → fallan → implementar →
  verde → commit "Modo Textura: editar las UVs de las caras y el tamaño de la textura".

### Task 5: Demo, spec y cierre

- `CHIMPSPRITE_DEMO_MODEL_TEXTURE` (con `CHIMPSPRITE_DEMO_MODEL`): la casita en modo Texture con la
  cara de enfrente del cubo elegida.
- Spec: `Estado: implementado` con notas.
- `cargo fmt --all`, clippy del workspace, `cargo test --workspace`, check a wasm de `cs-3d`,
  `cs-render` y `cs-ui`; captura revisando arriba/abajo, el contorno amarillo, el contorno UV y el
  panel UV.
- [ ] **Commit** — "Modo Textura: demo y spec al día".
