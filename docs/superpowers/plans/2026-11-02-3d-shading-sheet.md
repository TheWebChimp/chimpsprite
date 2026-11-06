# Modelos 3D, parte 1.5 (tabla de shading y Sheet settings): plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rampas de luz editables por modelo (tabla de shading tipo picoCAD) y el Bake como los
Sheet settings de picoCAD (Frames, Size, Zoom, Tilt, Height, Ortho) con vista previa.

**Architecture:** `ShadingTable` vive en `Model3D` (cs-core) y el rasterizador (cs-3d) la consulta
antes de la regla automática. El panel 3D (cs-ui) la edita con `SetModel`. `cs_render::BakeOptions`
cambia a los parámetros de picoCAD y el diálogo Bake se rehace como "Sheet settings" con vista
previa.

**Tech Stack:** Rust 2024, egui 0.36, el UI kit propio.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-02-3d-shading-sheet-design.md`.
- Sin floats guardados en `cs-core` (todo lo de `Model3D` deriva `Eq, Hash`); `cs-core`, `cs-3d`,
  `cs-render` sin egui.
- Textos de UI en inglés ("Shading", "Auto", "Sheet settings", "Frames", "Size (px)", "Zoom", "Tilt",
  "Height", "Ortho", "Export"); comentarios en español.
- Nunca `git checkout -- …`, `git restore`, `git stash`, `git reset --hard`, `git clean`; solo
  `git add <archivos propios>`; LF; no tocar `.design/`.
- Toolchain: `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=8`;
  cargo a prioridad baja `cmd //c "start /low /b /wait cargo test -p <crate>"`. fmt y clippy
  (`--all-targets`) sin avisos; commits en español con
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

### Task 1: La tabla de shading (modelo + render)

**Files:** Modify `crates/cs-core/src/model.rs` (tipos y campo), `crates/cs-3d/src/raster.rs` (~328, donde hoy se aplica `darker` `ctx.steps` veces) y `RenderInput` (campo nuevo `shading: &'a ShadingTable` o leerlo de `input.model.shading`, preferible lo segundo para no tocar a los que construyen `RenderInput`).

**Interfaces — Produces:**
```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShadeEntry { pub color: Rgba, pub steps: [Option<Rgba>; 2] }
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShadingTable { pub entries: Vec<ShadeEntry> }   // ordenada por (r,g,b,a), sin repetidos ni entradas vacías
impl ShadingTable {
    pub fn get(&self, color: Rgba, step: usize /* 1 o 2 */) -> Option<Rgba>;
    pub fn set(&mut self, color: Rgba, step: usize, value: Option<Rgba>);  // None = Auto; borra la entrada si queda vacía
}
// Model3D: #[serde(default)] pub shading: ShadingTable
// cs_3d: pub fn shade(palette: &[Rgba], table: &ShadingTable, c: Rgba, steps: u32) -> Rgba
//   steps 0 → c; 1 → table.get(c,1) o darker(c); 2 → table.get(c,2) o darker(darker(c))
```
El rasterizador usa `shade` en lugar del `fold` de `darker`.

- [ ] **Step 1:** Pruebas: `set`/`get` (orden, sin repetidos, `None` borra la entrada vacía); `shade` con tabla vacía = la regla de la parte 1; con `set(blanco, 1, rojo)` el paso 1 es rojo y el paso 2 sigue automático; un render Iso con luz donde la cara de 1 paso cambia al fijar la tabla y la de 2 pasos no; JSON de un `Model3D` sin `shading` (como lo escribía la parte 1) abre con la tabla vacía.
- [ ] **Step 2:** `cargo test -p cs-core model` y `cargo test -p cs-3d` → fallan.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** Verde; `cargo build --workspace --all-targets`.
- [ ] **Step 5:** Commit "Modelos 3D: tabla de shading por modelo (rampas de luz editables)".

### Task 2: La cuadrícula Shading en el panel 3D

**Files:** Modify `crates/cs-ui/src/model_panel.rs` (sección Light & Outline), `crates/cs-ui/src/editor/model_ops.rs` (`set_shading(&mut self, table: ShadingTable) -> bool`, un `SetModel`); Test `crates/cs-ui/src/layout_tests.rs`.

- Debajo de la luz: etiqueta "Shading" y un botón "Auto" (deshabilitado si la tabla está vacía).
- Cuadrícula: columnas = colores opacos de la paleta del sprite del modelo (`editor.sprite().palette`), en bloques de 16 columnas; fila 1 el color, filas 2 y 3 su paso 1 y paso 2 (`cs_3d::shade(…, 1/2)`). Celdas de 12×12 puntos con 1 px de separación, borde `(136,110,106)` como la cuadrícula del mapa de FontForge; las casillas fijadas llevan un punto de 1 px `(5,4,3)`/blanco en la esquina de arriba a la derecha.
- Clic en una casilla de paso: `menus::context_menu`-style popup (usar el patrón de `menus::dropdown` o un `Popup` como el del select) con las muestras de la paleta; elegir una → `set(color, paso, Some(elegido))` y `set_shading`. Clic derecho → `set(color, paso, None)`. Nombres accesibles: "Shade {i} step {k}" (i = índice en la paleta).
- "Auto" → `set_shading(ShadingTable::default())`.

- [ ] **Step 1:** Pruebas de layout: con un modelo, clic en "Shade 0 step 1" y elegir otro color fija la entrada (un paso de undo, el render cambia); clic derecho la quita; "Auto" vacía la tabla; todas las casillas dentro del flyout.
- [ ] **Step 2–4:** fallan → implementar → verde (`cargo test -p cs-ui` completo una vez).
- [ ] **Step 5:** Commit "Modelos 3D: la cuadrícula Shading del panel 3D".

### Task 3: Sheet settings (bake + diálogo con vista previa)

**Files:** Modify `crates/cs-render/src/bake3d.rs` (`BakeOptions`, `bake_model`), `crates/cs-ui/src/bake_dialog.rs` (rehacer como "Sheet settings"), `crates/cs-ui/src/editor/model_ops.rs` (`bake`), tests en ambos.

**Interfaces — Produces:**
```rust
pub struct BakeOptions {
    pub frames: u32,          // 1..=64
    pub size: u32,            // 8..=512, ancho del frame; alto = round(size·h/w) del modelo
    pub zoom: u32,            // %, 100 = cámara del modelo; distancia_bake = distancia·100/zoom (mín 1·FIXED)
    pub tilt: i32,            // décimas de grado, −890..=890 (pitch)
    pub height: i32,          // 1/16 unidad, se suma a target.y
    pub ortho: bool,
    pub animate_texture: bool,
    pub output: BakeOutput,
}
impl BakeOptions { pub fn from_model(m: &Model3D, size: (u32, u32), texture_frames: usize) -> Self }
// defaults: frames 8, size = ancho del modelo, zoom 100, tilt = m.camera.pitch, height 0,
// ortho = matches!(m.camera.projection, Orthographic), animate = texture_frames > 1, output Frames.
pub fn bake_preview(doc: &Document, model: ArtboardId, options: &BakeOptions, angle: u32) -> Option<(u32, u32, Vec<Rgba>)>
// un solo frame (ángulo `angle`, frame de textura 0) para la vista previa
```
- `bake_model` arma la cámara: la del modelo con `pitch = tilt`, `distance` ajustada por zoom, `target.y += height`, proyección `Orthographic` si `ortho` (si no, `Perspective` con el fov del modelo o 600). Renderiza a `(size, alto)`. Ángulos: `extra_yaw = a·3600/frames`. Tags y spritesheet como en la parte 1.
- **Diálogo "Sheet settings":** a la izquierda los campos (steppers del kit: Frames, Size (px), Zoom (%), Tilt (°), Height; casilla Ortho; casilla Animate texture si aplica; grupo Frames / Spritesheet), a la derecha la vista previa (cuadro de 96×96 puntos con fondo damero, la imagen de `bake_preview` a la mayor escala entera que quepa), girando: `angle = (time·8) as u32 % frames`, con `ctx.request_repaint_after(125 ms)`. Botón "Export" (en vez de OK) y "Cancel". La vista previa se recalcula solo cuando cambian las opciones o el ángulo (caché simple en el diálogo).

- [ ] **Step 1:** Pruebas en `bake3d.rs`: frames 12 → 12 frames; size 48 en un modelo 128×128 → frames 48×48; zoom 200 → más pixeles opacos que zoom 100; tilt distinto y height distinto cambian la imagen; ortho true vs false cambia la imagen; `bake_preview` == frame `angle` del bake sin animación. Prueba de UI (DialogHarness): el diálogo dibuja "Sheet settings" y "Export"; Export crea el artboard en un paso; cambiar Frames a 4 y exportar da 4 frames.
- [ ] **Step 2–4:** fallan → implementar → verde (`cargo test -p cs-render -p cs-ui`).
- [ ] **Step 5:** Commit "Modelos 3D: Sheet settings (Frames, Size, Zoom, Tilt, Height, Ortho) con vista previa".

### Task 4: Cierre

- [ ] Spec: estado "implementado" y cualquier ajuste hecho al implementar.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --all --check`, `cargo check --target wasm32-unknown-unknown -p cs-3d -p cs-render -p cs-ui` limpios.
- [ ] Revisión en pantalla (la hace el controlador con `CHIMPSPRITE_DEMO_MODEL` y abriendo Sheet settings si hay forma; si no, con pruebas).
- [ ] Commit "Modelos 3D 1.5: spec implementado".
