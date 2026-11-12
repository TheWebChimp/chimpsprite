# Regresar paneles a una orilla y vista previa de la tira Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que un panel flotando (Color en particular) se pueda regresar a una orilla de alto/ancho completo arrastrándolo o con "Dock", que la ventana arrastrada no se tape a sí misma, y que reordenar la tira muestre cómo quedaría.

**Architecture:** Un `DockTarget::Edge(Edge)` nuevo en `workspace` (parte la raíz hacia una orilla, el lado en puntos con el ancho de siempre del panel) al que llegan los dos caminos de soltar: el de egui_dock (corrigiendo en el mismo frame, como `canvas_drop`/`tidy_canvas`) y el de la tira (`drop_target`). La ventana arrastrada se excluye como destino y se atenúa. La tira dibuja su orden "como quedaría" mientras se arrastra.

**Tech Stack:** Rust 2024, egui 0.36, egui_dock `=0.21.1`, cs-ui.

**Spec:** `docs/superpowers/specs/2026-11-12-redock-and-strip-preview-design.md`. **Diagnóstico:** `.superpowers/sdd/color-redock-investigation.md` y `crates/cs-ui/src/layout_tests/diag_color_redock.rs` (4036dbb).

## Global Constraints

- Worktree `D:/Work/chimpdeas/cs-tiles`, rama `color-redock` (desde main 08d04df + 4036dbb). Reglas de agentes: `D:/Work/chimpdeas/.refs/agent-rules.md` (léelas completas; git prohibido: checkout/restore/stash/reset/clean/add -A/add .; NUNCA `python -` ni intérpretes por stdin — varios agentes se colgaron —: scripts con Write en `D:/Work/chimpdeas/.refs/tmp/` y `python archivo.py`; nunca matar procesos por nombre; solo editar dentro del worktree; `app.ron` solo respaldar y restaurar).
- Toolchain: `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-tiles/target`; todo cargo con `cmd //c "start /low /b /wait cargo …"`. Borra tus `target/gui-*` al terminar.
- egui_dock `=0.21.1`; solo `workspace` (y `dock.rs` para la tira) lo toca. Si dependes de un detalle interno nuevo de egui_dock, agrégalo a la lista de `workspace/skin.rs` (cabecera).
- **Essentials en reposo se ve igual:** capturas a escala 2 con 0 px contra main 08d04df (en reposo, Preview, pestaña de modelo, diálogo; con y sin Pixel UI) — en la tarea que cambie algo que se dibuja en reposo y en la última. Lo nuevo solo se dibuja durante un arrastre o en un menú.
- Componentes del kit; rects en puntos enteros; textos de UI en inglés, comentarios en español; repo eol=lf; commits con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Al final de cada tarea: `cargo test -p cs-ui`, `cargo clippy -p cs-ui --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check --target wasm32-unknown-unknown -p cs-ui`.

---

### Task 1: El destino de orilla y el ancho al acoplar

**Files:** `workspace/model.rs` (`DockTarget::Edge`, `Workspace::dock`), `workspace/drop.rs` (`drop_target`, zona), `workspace/show.rs` (corregir el soltar de egui_dock en el mismo frame, como `canvas_drop`; ancho natural ~820-838), `workspace/fit.rs` (lado en puntos), `workspace/skin.rs` (no mostrar la zona de egui_dock cuando rige la de orilla), `lib.rs` (pintado mientras se arrastra un ícono), `crates/cs-ui/src/layout_tests/diag_color_redock.rs` (pasan a tests del comportamiento nuevo).

**Interfaces:**
- Produces: `DockTarget::Edge(Edge)`; `pub(crate) const EDGE_BAND: f32 = 16.0;` `fn edge_under(area: Rect, strips: &[(Edge, Rect)], p: Pos2) -> Option<Edge>` (pura: la franja de cada orilla del área del workspace; las esquinas, la orilla más cercana); `Workspace::edge_zone(edge, id, area) -> Rect` (la columna/fila que quedaría, en puntos enteros); `Workspace::preferred_side(id, view) -> f32` (Color: `view.palette_width` o su natural; demás: `reference_width`; abajo/arriba: `default_size.y`).

- [ ] **Step 1: Tests que fallan.**
  - Puros: `edge_under` en las cuatro orillas, fuera de la franja, en las esquinas.
  - `Workspace`: `dock(Color, Edge(Right))` sobre un Essentials sin Color da el mismo árbol y, dibujado, los mismos rects que `Workspace::essentials()` (a 1200×800 y a 960×505); `Edge(Left)` con Library da una columna de alto completo a la izquierda de la toolbar… o junto a ella (decide y documenta: la columna va en la orilla, la toolbar queda entre ella y el lienzo); `Edge(Bottom)` da una fila de ancho completo debajo del timeline.
  - App (`diag_color_redock.rs` convertido): Color flotando → arrastrar su pestaña y soltar (i) en la franja derecha y (ii) sobre la tira derecha → Essentials idéntico; mientras se arrastra, la zona pintada es la columna (alto completo, ancho de Color); un ícono de la tira (Library) soltado en la franja izquierda queda en una columna de alto completo a la izquierda.
  - Ancho: soltar Library en el cuarto derecho del lienzo (split de egui_dock) → su lado mide su `reference_width`, no la mitad del lienzo; `palette_width`/el natural no cambian por eso.
- [ ] **Step 2:** correr → fallan.
- [ ] **Step 3: Implementar.** Pestañas: dejar que egui_dock suelte y, si el puntero estaba en la franja (o Color sobre una tira), deshacer su movimiento y aplicar `dock(id, Edge(edge))` en el mismo frame (mira cómo `canvas_drop` + `tidy_canvas` corrigen el lienzo). Íconos: `drop_target` devuelve `Edge` en la franja. Paneles que van a la tira soltados como pestaña sobre una tira: a la tira en esa posición (`move_in_strip`/`StripArea::drop_index`). La causa 3 (ventana nueva en un lugar fijo) solo si sale barato; si no, anótalo en el reporte.
- [ ] **Step 4:** `cargo test -p cs-ui` completo; clippy + fmt + wasm.
- [ ] **Step 5:** commit `"Workspace: soltar en una orilla deja una columna (o fila) de lado a lado"`.

### Task 2: La ventana arrastrada no se tapa a sí misma

**Files:** `workspace/show.rs` / `workspace/drop.rs` (soltar sobre la propia ventana), `workspace/skin.rs` (opacidad de la capa de esa ventana durante el arrastre, p. ej. en `SkinRewriter` o con la capa `window_layer(surface)`), tests.

**Interfaces:** Consumes `DockTarget::Edge`, `edge_under`, la zona de orilla (tarea 1).

- [ ] **Step 1: Tests que fallan.** Color flotando sobre el cuarto derecho del lienzo y la tira (como la captura del usuario, a 960×505): arrastrar su única pestaña y soltar con el puntero encima de su propia ventana (i) sobre la tira derecha → Essentials; (ii) sobre el cuarto derecho del lienzo → partido a la derecha del lienzo; mientras se arrastra, las formas de la capa de esa ventana van con alfa ≈ 0.4 y las zonas pintadas son las de lo de debajo; al soltar o cancelar, opaca otra vez. Una ventana con dos pestañas: arrastrar una no la atenúa (la ventana sí es destino).
- [ ] **Step 2:** correr → fallan.
- [ ] **Step 3: Implementar** (corrigiendo en el mismo frame si egui_dock la volvió a flotar; documenta el detalle interno de egui_dock del que dependas en la cabecera de `skin.rs`).
- [ ] **Step 4:** `cargo test -p cs-ui`; clippy + fmt + wasm.
- [ ] **Step 5:** commit `"Workspace: la ventana que se arrastra se ve translúcida y no tapa sus destinos"`.

### Task 3: "Dock" en la pestaña flotante y el crash en debug

**Files:** `workspace/show.rs`/`workspace/tabs.rs` (menú contextual de la pestaña de una ventana flotante, con `crate::menus::context_menu`), `workspace/model.rs` (`dock_home(id)` o reusar `dock_from_strip`), y lo que el crash pida.

**Interfaces:** Consumes `DockTarget::Edge(Right)` + `preferred_side` (tarea 1). Produces `Workspace::dock_home(id: PanelId)` (Color → `Edge(Right)`; Tools → junto al lienzo a la izquierda; Timeline → debajo del lienzo; del dock → su tira de fábrica; Preview: no aplica).

- [ ] **Step 1: Tests que fallan.**
  - Clic derecho en la pestaña "Color" de su ventana flotante → menú con "Dock" → Essentials idéntico. Igual para Tools y Timeline flotando (a su lugar de Essentials) y para Library flotando (a la tira derecha, en su lugar de fábrica). La pestaña de Preview no ofrece "Dock". Con un diálogo modal no se abre el menú.
  - El crash: test que suelta una pestaña a la derecha de Tools (como el `diag` de la investigación) y hoy truena en debug.
- [ ] **Step 2:** correr → fallan.
- [ ] **Step 3: Implementar.** El crash: encuentra la causa (qué tamaño queda negativo y por qué) antes de tocar nada; arregla la causa, no el síntoma.
- [ ] **Step 4:** `cargo test -p cs-ui`; clippy + fmt + wasm.
- [ ] **Step 5:** dos commits: `"Workspace: Dock en la pestaña de una ventana flotante"` y `"Workspace: <la causa del crash>"`.

### Task 4: La tira se ve como quedaría mientras se reordena

**Files:** `crates/cs-ui/src/dock.rs` (`show`, `StripArea`, la línea de inserción), `lib.rs` (la tira de destino), tests en `layout_tests.rs`.

**Interfaces:** Consumes `StripArea::{drop_index, line}`, `insert_index`, `move_in_strip` (ya existen).

- [ ] **Step 1: Tests que fallan.** Arrastrar Library hasta antes del primero de su tira: durante el arrastre, los rects de los botones (por nombre) son los del orden final (`[Library, Groom, Lumen, …]` en sus lugares) y el de Library se pinta con alfa ≈ 0.5; no se pinta la rayita; al soltar, los rects no cambian. Sobre otra tira (izquierda con Tools colapsada): esa tira muestra el ícono semitransparente en su lugar y sus demás se recorren; la de origen cierra el hueco. Cayendo entre los ocultos: el botón "More Panels" resaltado. Los de Timeline/Preview no se mueven.
- [ ] **Step 2:** correr → fallan.
- [ ] **Step 3: Implementar** (la tira toma un "orden de vista previa" mientras dura el arrastre; el botón fantasma es el mismo componente del kit con opacidad, sin respuesta; el arrastre sigue siendo del botón original aunque se dibuje en otro lugar — cuida el id y que no se pierda el arrastre al moverse su rect).
- [ ] **Step 4:** `cargo test -p cs-ui`; clippy + fmt + wasm; **capturas** de Essentials en reposo (base main 08d04df de `git archive` en `target/gui-r4-base`, HEAD en `target/gui-r4`; con y sin Pixel UI; en reposo, Preview, pestaña de modelo, diálogo): 0 px; borra los target y la fuente extraída.
- [ ] **Step 5:** commit `"Tira: mientras se reordena se ve como quedaría"`.
