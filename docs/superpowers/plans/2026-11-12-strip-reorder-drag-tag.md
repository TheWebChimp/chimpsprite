# La pastilla al arrastrar y reordenar la tira Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que lo que se arrastra en el workspace (pestaña, franja, ícono de la tira) se vea como la pastilla `tab.drag` con el nombre del panel, y que los íconos de las tiras se puedan reordenar arrastrándolos, con línea de inserción.

**Architecture:** Un componente del kit (`kit::tabs::drag_tag_shapes`) dibuja la pastilla con la pieza nueva `tab.drag`; los tres `ghost` lo llaman. En `dock.rs`, el arrastre de un ícono calcula un índice de inserción (función pura) sobre su tira o la tira de destino, pinta la línea y al soltar pide `StripRequest::Reorder` / `Drop` con índice; `Workspace` inserta en ese índice.

**Tech Stack:** Rust 2024, egui 0.36, egui_dock `=0.21.1`, cs-ui.

**Spec:** `docs/superpowers/specs/2026-11-12-strip-reorder-drag-tag-design.md`.

## Global Constraints

- Worktree `D:/Work/chimpdeas/cs-tiles`, rama `strip-reorder-drag-tag` (desde main 84abeae). Reglas de agentes: `D:/Work/chimpdeas/.refs/agent-rules.md` (léelas completas; git prohibido: checkout/restore/stash/reset/clean/add -A/add .; nunca `python -` ni intérpretes por stdin: scripts con Write en `D:/Work/chimpdeas/.refs/tmp/`; nunca matar procesos por nombre; solo editar dentro del worktree; `app.ron` solo respaldar y restaurar).
- Toolchain: `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-tiles/target`; todo cargo con `cmd //c "start /low /b /wait cargo …"`. Borra tus `target/gui-*` al terminar.
- La pieza `tab.drag` ya está en `assets/ui/skin.aseprite` (slice (276, 901), 5×18, centro `(2, 2, 1, 14)`); no edites el skin. El skin embebido se regenera como lo hagan las otras piezas (revisa cómo se cargan: si hace falta exportar algo, hazlo con el mismo mecanismo que usa el repo y dilo).
- **Essentials en reposo se ve igual:** capturas a escala 2 con 0 px distintos contra main 84abeae (en reposo, Preview, pestaña de modelo, diálogo; con y sin Pixel UI). Lo nuevo solo se dibuja durante un arrastre.
- Componentes del kit, no piezas sueltas; rects en puntos enteros.
- Textos de UI en inglés; comentarios y docs en español; repo eol=lf; commits con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Al final de cada tarea: `cargo test -p cs-ui`, `cargo clippy -p cs-ui --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check --target wasm32-unknown-unknown -p cs-ui`.

---

### Task 1: La pastilla `tab.drag` en las tres sombras de arrastre

**Files:** `crates/cs-ui/src/kit/tabs.rs` (+ `kit/tabs/tests.rs`), `crates/cs-ui/src/workspace/skin.rs` (`paint_tabs` → `ghost`, ~817–960), `crates/cs-ui/src/dock.rs` (`ghost`, ~483–540), `crates/cs-ui/src/kit/catalog.rs` (la pastilla junto a las pestañas), tests en `layout_tests.rs` / `workspace/*`.

**Interfaces:**
- Produces: `pub const DRAG: &str = "tab.drag";`, `pub fn drag_tag_size(ui: &Ui, label: &str) -> Vec2` (texto + 12 de ancho, 18 de alto, enteros) y `pub fn drag_tag_shapes(ui: &Ui, skin: &Skin, min: Pos2, label: &str) -> Vec<Shape>` en `kit::tabs`.

- [ ] **Step 1: Tests que fallan.**
  - `kit/tabs/tests.rs`: `drag_tag_size(ui, "Color") == vec2(34.0, 18.0)`; con el skin embebido, `drag_tag_shapes` rasterizada a 1x (usa el helper de raster de los tests del kit) da exactamente la imagen de referencia: contorno `#bda499` con esquinas recortadas, aro `#f1ebdb`, relleno `#ddcebf`, y el texto "Color" con su primera columna de tinta en x 6 y sus mayúsculas de y 6 a 11 (la referencia: `D:/Work/chimpdeas/.refs/tmp/drag-ghost/color_tab.png`, recorta su rect (3,3)–(37,21)).
  - App: arrastrar la pestaña de Color (Essentials con Library acoplada junto a Color para que haya barra), la franja de un grupo sin barra y un ícono de la tira: en la capa `Tooltip` del fantasma hay una malla con la textura de `tab.drag` y el texto del panel, y ninguna de `tab`/`tab.active`/botón.
- [ ] **Step 2:** correr → fallan.
- [ ] **Step 3: Implementar** el componente (pieza estirada con `pixel_draw` como las otras piezas de nueve partes; respaldo con los tres colores si falta la pieza) y cambiar los tres `ghost` para que lo usen con `min` = la esquina de lo agarrado corrida lo arrastrado (redondeado). Sin Pixel UI no cambia. Catálogo: una fila con la pastilla "Color".
- [ ] **Step 4:** `cargo test -p cs-ui` completo; clippy + fmt + wasm.
- [ ] **Step 5:** commit `"Workspace: lo que se arrastra se ve como una pastilla con su nombre"`.

### Task 2: Reordenar los íconos de la tira (y soltar en otra en su posición)

**Files:** `crates/cs-ui/src/dock.rs` (`show` ~288–470: arrastre del ícono, `StripRequest`, `DockOutput`), `crates/cs-ui/src/workspace/model.rs` (`to_strip_at` ~848), `crates/cs-ui/src/workspace/drop.rs` (`apply_strip_request` ~145–170, `strip_under`, `paint_strip_zone`), `crates/cs-ui/src/lib.rs` (~1150: pintado mientras se arrastra), tests.

**Interfaces:**
- Produces: `fn insert_index(buttons: &[Rect], vertical: bool, pointer: Pos2) -> usize` (pura, en `dock.rs`: índice entre 0 y `buttons.len()` según el centro de cada botón a lo largo de la tira); `fn insertion_line(buttons: &[Rect], strip: Rect, vertical: bool, index: usize) -> Rect` (2 de grueso, a lo ancho del botón, centrada en el hueco; en los bordes, pegada al primero/último); `StripRequest::Reorder(DockPanel, usize)`; `StripRequest::Drop` lleva el índice cuando cae sobre otra tira (o una variante `ToStrip(DockPanel, Edge, usize)`: elige una y documéntala); `Workspace::to_strip_at(id, edge, index: Option<usize>)` (o `move_in_strip`): inserta en `index` (`None` = al final; el índice cuenta sin el propio panel si ya estaba en esa tira).
- Para pintar la línea sobre otra tira, `dock::show` de cada tira necesita los rects de sus botones visibles: guárdalos en `DockOutput` (p. ej. `buttons: Vec<(DockPanel, Rect)>` — ya existe `anchors`, reúsalo si sirve) y que `lib.rs` los pase a quien pinta.

- [ ] **Step 1: Tests que fallan.**
  - Puros: `insert_index` antes del primero, entre el 1 y el 2, después del último, en vertical y en horizontal; `insertion_line` en esos casos (enteros, dentro de la tira).
  - `Workspace`: con la tira derecha `[Groom, Lumen, Library, …]`, `Reorder(Library, 0)` la deja primera; `Reorder(Groom, 2)` (después de Lumen) da `[Lumen, Groom, Library, …]`; reordenar a su mismo lugar no cambia nada; pasar Library a la tira izquierda en el índice 1 la inserta ahí; `to_saved`/`from_saved` conserva el orden.
  - App (`layout_tests.rs`, con `dock_drag`/`drag_in_steps`): arrastrar el ícono de Library hasta antes del primero de su tira → al soltar es el primero; mientras está encima se pinta la línea de inserción en el hueco correcto (rect esperado de `insertion_line`) y no se pinta el resaltado de tira (es la suya); sobre la tira izquierda (no vacía), resaltado + línea, y al soltar cae en esa posición; soltarlo en su mismo lugar no cambia el orden; los botones de Timeline/Preview no cambian.
- [ ] **Step 2:** correr → fallan.
- [ ] **Step 3: Implementar.** En `dock::show`, mientras un ícono se arrastra con el puntero dentro de su tira: `insert_index` sobre los botones visibles (sin "More Panels" ni los de Timeline/Preview), pintar la línea en la capa de primer plano (como `paint_strip_zone`; con skin `text_selection`, sin skin el color de selección de egui) y, al soltar ahí, `StripRequest::Reorder` si el índice cambia el orden. Fuera de su tira y sobre otra: `lib.rs` pinta resaltado + línea con los rects de la otra tira y `apply_strip_request` inserta en el índice. Cuidado: hoy `out.dragging` solo se llena fuera de la tira de origen y el ícono que se arrastra sigue contando como clic hasta 6 pt (no lo rompas).
- [ ] **Step 4:** `cargo test -p cs-ui` completo; clippy + fmt + wasm; capturas de Essentials en reposo (release en `target/gui-s2` y la base main en `target/gui-s2-base` construida de `git archive 84abeae`, sin checkouts ni worktrees nuevos): 0 px con y sin Pixel UI; borra los target y la fuente extraída al terminar.
- [ ] **Step 5:** commit `"Tira: los íconos se reordenan arrastrándolos, con línea de inserción"`.
