# Workspace, fase E: tests de layout por panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que los tests de layout midan dentro de cada panel y en el preset Essentials (no en posiciones absolutas de la ventana), que los arneses de Timeline y Color hospeden su panel como la app, cerrar los dos tramos conocidos del barrido de anchos (Timeline 64–126, Lospec 364–600), partir `workspace/show.rs` y arreglar los minors que dejó la fase D.

**Architecture:** Un solo lugar fija números absolutos: el test `essentials_at_1200_by_800` (los rects de cada hoja de Essentials en la ventana de siempre). Todo lo demás se mide relativo a la hoja de su panel (`h.at(id, rect)`), y los tests de un panel que se puede mover corren también con ese panel en otro acomodo (`Workspace::solo`) para probar que lo de adentro no depende de dónde está. Los arneses de `timeline/tests.rs` y `color_panel/tests.rs` pasan a una `ChimpApp` con `Workspace::solo` (el panel lo hospeda el `Viewer` de verdad). El resto son cambios locales: `pixel_ui::wrap_rows` por botón en el timeline, `forms` en las filas de Lospec.

**Tech Stack:** Rust 2024, egui/eframe 0.36, egui_dock `=0.21.1`, egui_taffy `=0.14.0`, cs-ui.

**Spec:** `docs/superpowers/specs/2026-11-09-workspace-docking-design.md` — "Se quedó para la fase E" (~línea 410) y "E. Tests de layout reescritos" (~428). Guía de formas: `docs/forms.md`.

## Global Constraints

- Worktree `D:/Work/chimpdeas/cs-tiles`, rama `workspace-layout-e` (desde main 45dc2da). Reglas de agentes: `D:/Work/chimpdeas/.refs/agent-rules.md` (léelas completas; git prohibido: checkout/restore/stash/reset/clean/add -A/add .; nunca `python -` ni intérpretes por stdin: scripts con Write en `D:/Work/chimpdeas/.refs/tmp/`; nunca matar procesos por nombre; solo editar dentro del worktree; `app.ron` solo respaldar y restaurar).
- Toolchain: `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-tiles/target`; todo cargo con `cmd //c "start /low /b /wait cargo …"`. Borra tus `target/gui-*` al terminar cada tarea (deja el último).
- egui_dock `=0.21.1`; solo `workspace` (y sus submódulos) lo usa.
- **Essentials se ve igual que hoy:** capturas a escala 2 con 0 pixeles distintos contra main (en reposo, con y sin Preview, en la pestaña de un modelo, con un diálogo; con y sin Pixel UI). Las únicas tareas que cambian pixeles son la 6 (timeline a menos de 127 pt de ancho) y la 7 (Lospec, solo medio punto en y): ninguna se ve en Essentials en reposo.
- **Ningún test pierde lo que verifica.** Al reescribir un test, cada número absoluto se convierte en `número − origen de su hoja` (a mano, en el commit queda la tabla de origen) y el origen queda fijado en `essentials_at_1200_by_800`. El número de `#[test]` de `cs-ui` no baja (se anota antes y después en el reporte de cada tarea).
- Rects y divisores en puntos enteros.
- Textos de UI en inglés; comentarios y docs en español; un commit por tarea con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; repo eol=lf.
- Al final de cada tarea: `cargo test -p cs-ui`, `cargo clippy -p cs-ui --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check --target wasm32-unknown-unknown -p cs-ui`. En la tarea 9, lo completo de `agent-rules.md` (workspace entero + wasm de todos los crates).

## File Structure

- Split `crates/cs-ui/src/workspace/show.rs` → `workspace/modal.rs` (`Frozen`, `hold_windows`, `modal_shields`), `workspace/tabs.rs` (`tab_wheel`, `tab_arrows`, `close_marks`, `dropped_past_last`, `to_the_end`, `collapse_buttons`, `COLLAPSE`), `workspace/dividers.rs` (`cover_tools_divider`, `cover_timeline_divider`, `paint_crossings`), `workspace/preview_window.rs` (`PreviewWindow` y el bloque de su marco en `show`). `show.rs` se queda con `Viewer`, `show` y sus ayudantes de tamaño.
- Modify `crates/cs-ui/src/workspace/presets.rs` (`Workspace::solo`, solo de tests), `workspace/test_util.rs` (ayudantes de tests compartidos que salen de `show.rs`).
- Modify `crates/cs-ui/src/layout_tests.rs` (ayudantes `leaf`/`at`/`with_workspace`, `essentials_at_1200_by_800`, tests reescritos).
- Modify `crates/cs-ui/src/timeline/tests.rs`, `crates/cs-ui/src/color_panel/tests.rs` (arneses por el workspace).
- Modify `crates/cs-ui/src/timeline/pixel.rs` (fila de botones que se parte), `crates/cs-ui/src/palette_ui.rs` (`lospec_row` con `forms`), `crates/cs-ui/src/width_sweep_tests.rs` (sin los dos `KNOWN_GAPS`).
- Modify `crates/cs-ui/src/workspace/modal.rs` (foco por árbol), `workspace/show.rs`/`workspace/drop.rs` (zona del lienzo), `lib.rs` + `workspace/drop.rs` (resaltar la tira).
- Modify spec (fase E HECHA, Probar a mano) en la tarea 9.

---

### Task 1: Partir `show.rs` sin cambiar nada

**Files:** `crates/cs-ui/src/workspace/show.rs` → `workspace/{modal.rs, tabs.rs, dividers.rs, preview_window.rs}`; `workspace/mod.rs` (declarar los módulos; `COLLAPSE` se reexporta desde `tabs`, lo usa `layout_tests.rs`); `workspace/test_util.rs` (ayudantes compartidos de los tests).

**Interfaces:**
- Produces: `modal::{Frozen, hold_windows, modal_shields}`, `tabs::{tab_wheel, tab_arrows, close_marks, dropped_past_last, to_the_end, collapse_buttons, COLLAPSE}`, `dividers::{cover_tools_divider, cover_timeline_divider, paint_crossings}`, `preview_window::PreviewWindow` — todo `pub(super)` (salvo `COLLAPSE`, `pub(crate)` como hoy). Mismas firmas que hoy.

- [ ] **Step 1: Anotar la línea base.** `cargo test -p cs-ui -- --list 2>/dev/null | grep -c ': test$'` → anota N en el reporte.
- [ ] **Step 2: Mover el código**, en este orden (un `cargo check -p cs-ui` después de cada uno):
  - `modal.rs`: `modal_shields` (show.rs ~1616–1709), `hold_windows` (~1749–1774), `Frozen` + `take`/`restore` (~1776–1829). Si queda limpio, también el rect del título del Preview (~713–717) y el bloque que apaga el `Ui` sin atenuarlo (~732–736) como funciones `pub(super)`; si capturan locales de `show`, se quedan.
  - `tabs.rs`: `COLLAPSE` (~1310), `collapse_buttons` (~1313–1442), `close_marks` (~1444–1489), `tab_wheel` (~1491–1521), `tab_arrows` (~1523–1614), `dropped_past_last` (~1711–1727), `to_the_end` (~1729–1747).
  - `dividers.rs`: `cover_tools_divider` (~1159–1215), `cover_timeline_divider` (~1217–1246), `paint_crossings` (~1261–1295).
  - `preview_window.rs`: `PreviewWindow` + `new` (~327–401) y el bloque del marco de su ventana en `show` (~761–818) como `fn chrome(…)`; los campos que lee `Viewer` (`inset`, `size`, `measuring`, `drawn`, `margin`) quedan `pub(super)`.
  - **El orden de las llamadas dentro de `show()` no cambia** (`Frozen::take` y `hold_windows` antes del `DockArea`; `restore`, `to_the_end` y `modal_shields` después; `dropped_past_last` con las `bars` de antes, `close_marks`/`tab_arrows` con `bars_after`). Los ids de memoria (`"workspace_collapse_armed"`, `"workspace_close"`, `"workspace_tab_arrows"`, `"workspace_modal_shield"`, `"workspace_tools_divider"`, …) no cambian.
- [ ] **Step 3: Mover los tests.** `a_modal_puts_back_no_focus_too` (~2968) a `modal.rs`. Los de integración (todos pasan por `show()`) se reparten por tema: modal (~2118–2367, 2810–2872, 2939–2966) → `modal.rs`; pestañas (~2403–2505, 2620–2634, 2689–2937, 3014–3094) → `tabs.rs`; `the_crossings_are_not_painted…` (~2507–2541) → `dividers.rs`; el resto se queda. Los ayudantes que usan varios grupos (`SIZE`, `leaf`, `snapshot`, `icon_label`, `close_tab`, `click_node`, …) pasan a `test_util.rs` como `pub(super)`; los de un solo grupo van con él. Quita el `has_marker` local duplicado (~1897) y usa el de arriba.
- [ ] **Step 4: Arreglar los enlaces de docs** que apuntan a lo movido: `show.rs:6–7`, `skin.rs:558,783` (`show::tab_arrows` → `tabs::tab_arrows`), `mod.rs:20,29,35`, `model.rs:1065`, `fit.rs:170`; en los módulos nuevos, `[`show`]` → `[`super::show::show`]`. `cargo doc -p cs-ui --no-deps 2>&1 | grep -i "unresolved link"` → nada de `workspace`.
- [ ] **Step 5: Verificar.** `cargo test -p cs-ui` → N tests, todos pasan (mismo N del Step 1). `show.rs` ≤ ~1 900 líneas con tests. clippy + fmt + wasm.
- [ ] **Step 6: Commit** `"workspace: show.rs partido en modal, tabs, dividers y preview_window (sin cambios)"`.

### Task 2: Medir por panel: `Workspace::solo`, `h.leaf`/`h.at` y el único test absoluto

**Files:** `workspace/presets.rs` (o `model.rs` si encaja mejor junto a `essentials`), `layout_tests.rs` (ayudantes y test nuevo).

**Interfaces:**
- Produces (solo `#[cfg(test)]`):
  ```rust
  impl Workspace {
      /// Un acomodo de prueba: el lienzo y `id` acoplado solo en el lado `edge` del lienzo (el resto
      /// de los paneles del dock en la tira derecha, como en Essentials; sin toolbar ni timeline si
      /// no son `id`). El ancho/alto del lado sale de la preferencia que ya usa el anfitrión
      /// (`view.palette_width` para un lado izquierdo/derecho, `view.timeline_height` abajo/arriba)
      /// o del `default_size` de la ficha.
      pub(crate) fn solo(id: PanelId, edge: Edge) -> Workspace;
  }
  ```
  y en `LayoutHarness`:
  ```rust
  /// El rect de la hoja de `id` (acoplada o en su ventana flotante) en el último frame.
  fn leaf(&self, id: PanelId) -> Rect;
  /// `r` relativo a la hoja de `id` (su esquina de arriba a la izquierda = (0, 0)).
  fn at(&self, id: PanelId, r: Rect) -> Rect;
  /// Como `start_in`, con el acomodo `w` (en vez de Essentials) desde el primer frame.
  fn with_workspace(app: ChimpApp, w: Workspace, size: Vec2) -> Self;
  /// Lo derecho de Essentials (la columna con su divisor): `Workspace::column_rect`. Solo tiene
  /// sentido en Essentials; los tests que lo usan lo dicen.
  fn column(&self) -> Rect;
  ```
  `leaf` usa `Workspace::leaf_rect`, y si no está acoplado, `window_rect(ctx, id)`.

- [ ] **Step 1: Tests que fallan.**
  - `presets.rs`: `solo_puts_the_panel_beside_the_canvas` — para `(Color, Right)`, `(Color, Left)`, `(Timeline, Bottom)`, `(Tools, Right)`, `(Library, Left)`: el panel está acoplado solo en su grupo, ese grupo toca la orilla `edge`, el lienzo existe una vez, y `sound(&w.state)`.
  - `layout_tests.rs`: `essentials_at_1200_by_800` — `LayoutHarness::with_tabs(&[("a", false)], Some(0))` (Pixel UI) y fija **con números** los rects de `leaf(Tools)`, `leaf(Canvas)`, `leaf(Timeline)`, `leaf(Color)`, `column()`, la tira derecha (`panel("dock")`) y las barras (`panel("menu")`, `"tabs"`, `"tool_options"`, `"status"`). Los números salen de los tests de hoy (p. ej. columna `937..1200 × 63..780`, tools `0..35`, lienzo desde 37, timeline desde 590, tira `911..937`) — córrelo una vez con `dbg!` para copiar los exactos y que concuerden con los que ya fijan los tests de hoy; si alguno no concuerda, NEEDS_CONTEXT. Lo mismo sin Pixel UI (`LayoutHarness::editor()`) en `essentials_at_1200_by_800_plain`.
  - `the_inside_of_a_panel_does_not_depend_on_where_it_is` — para Color: en Essentials anota `at(Color, r)` de 5 botones (`"Lock palette"` y 4 más que encuentres en `color_panel`), luego `with_workspace(…, Workspace::solo(PanelId::Color, Edge::Left), (1200, 800))` con el mismo `palette_width`: los mismos `at(Color, r)`.
- [ ] **Step 2:** correr → fallan (no existe `solo`/`leaf`/`at`).
- [ ] **Step 3: Implementar** `solo` (construir con `DockState::new(vec![PanelId::Canvas])` y `split_left/right/above/below` como `essentials()`; fracción cualquiera, `pin_sides` deja el lado en puntos en el primer frame), los cuatro ayudantes, y quitar de `LayoutHarness::panel` los casos especiales `"palette"`/`"tools"`/`"timeline"`: cada llamador pasa a `h.column()` (columna derecha en Essentials), `h.leaf(PanelId::Tools)` o `h.leaf(PanelId::Timeline)`. **Ojo:** `tools_rect`/`timeline_rect` no son exactamente la hoja (incluyen el divisor; el timeline sin la punta bajo la tira). Donde un test medía `panel("tools")`/`panel("timeline")`, conserva el mismo rect con ayudantes `h.tools_box()` / `h.timeline_box()` (que llaman `tools_rect`/`timeline_rect`) y documenta la diferencia con la hoja; no cambies ningún número en este paso.
- [ ] **Step 4:** `cargo test -p cs-ui` completo → verde; el número de tests sube en 4.
- [ ] **Step 5:** clippy + fmt + wasm; commit `"Tests de layout: medir por hoja (leaf/at), Workspace::solo y Essentials fijado una vez"`.

### Task 3: Reescribir los tests de la ventana y de la columna derecha (fase C)

**Files:** `layout_tests.rs` — tests con números absolutos en ~587, ~885, ~2551, ~4056, ~5450, ~11419, ~11456, ~11492, y los de anchos fijos de la columna (`== 263.0` ~12689/12728, `== 137.0` ~11839/12378, `panel("tools").width() == 184.0` ~720/2199). Encuéntralos con `grep -nE "pos2\([0-9]{3,}|Rect::from_min_max\(pos2\([0-9]|== (263|137|184)\.0" crates/cs-ui/src/layout_tests.rs`.

**Interfaces:** Consumes `h.leaf`, `h.at`, `h.column`, `h.tools_box`, `h.timeline_box`, `LayoutHarness::with_workspace`, `Workspace::solo` (tarea 2).

- [ ] **Step 1: Inventario.** Lista en el reporte cada test de esta tarea con: línea, qué mide, a qué hoja pertenece (Color, Tools, Timeline, Canvas, barra de la ventana) y su conversión. Lo que mide las barras fijas de la ventana (menú, pestañas, opciones, estado: no se mueven) se queda absoluto si es geometría de la ventana (alturas, `y` de las barras) y se dice así.
- [ ] **Step 2: Convertir** cada número de un panel a `h.at(id, rect)`, calculado a mano (`937.0 → 0.0` en x para la columna, etc.). Ejemplo (hoy ~11417–11426):
  ```rust
  // Antes
  assert_eq!(h.panel("palette"), Rect::from_min_max(pos2(937.0, 63.0), pos2(1200.0, 780.0)));
  ("Lock palette", pos2(947.0, 71.0), pos2(972.0, 88.0)),
  // Después: la columna se fija en essentials_at_1200_by_800; aquí, lo de adentro
  ("Lock palette", pos2(10.0, 8.0), pos2(35.0, 25.0)), // h.at(PanelId::Color, button)
  ```
  (los números exactos salen de restar el origen de la **hoja** de Color, no de la columna: verifica cuál usas y escríbelo en el comentario del test).
- [ ] **Step 3: Probarlos en otro acomodo.** Los tests de dentro de Color (botones, caja de la paleta, divisor de la paleta) corren además con `Workspace::solo(PanelId::Color, Edge::Left)` y el mismo ancho, con un ayudante `for_both(|h| …)` que corre el mismo cuerpo en Essentials y en el acomodo movido. Los anchos fijos (`263`, `137`) se quedan solo donde el test es sobre el ancho de la columna en Essentials (lo dicen su nombre y un comentario); si el test es de otra cosa, se mide relativo.
- [ ] **Step 4:** `cargo test -p cs-ui layout_tests` → verde; el `grep` del encabezado ya solo encuentra lo de `essentials_at_1200_by_800` y lo de las barras de la ventana (anotado en el reporte). `cargo test -p cs-ui` completo.
- [ ] **Step 5:** clippy + fmt + wasm; commit `"Tests de layout: la ventana y la columna derecha medidas por panel"` con la tabla de orígenes en el cuerpo.

### Task 4: Reescribir los tests de la fase D (lienzo, toolbar, timeline, Preview, tiras)

**Files:** `layout_tests.rs` — tests con números absolutos en ~12962, ~13261 (botones del timeline, `("Play", (91.0, 596.0, 116.0, 613.0))`), ~13354, ~13435 (`canvas.bottom() == 780.0`), ~13457 (`let at = pos2(400.0, 589.0)` para pasar sobre un divisor), ~13551, ~13598, ~13659, ~13724, ~13959, ~14063, ~14372, y `h.canvas.unwrap().left() == 184.0` (~12994). Mismo `grep` que la tarea 3.

**Interfaces:** Consumes lo de la tarea 2.

- [ ] **Step 1: Inventario** como en la tarea 3.
- [ ] **Step 2: Convertir.** Botones del timeline → `h.at(PanelId::Timeline, r)`; toolbar → `h.at(PanelId::Tools, r)`; Preview → relativo a su ventana (`h.leaf(PanelId::Preview)`); puntos para apuntar a un divisor o a una orilla (`pos2(400.0, 589.0)`) → se calculan de los rects (`h.leaf(PanelId::Timeline).top() - 1.0`, etc.), nunca literales; `canvas.bottom() == 780.0` → `canvas.bottom() == h.panel("status").top()` (o la relación que el test de verdad quiere: escríbela). Los tests de tiras por orilla miden la tira contra la orilla de la ventana y la hoja vecina (relaciones), no números.
- [ ] **Step 3: Otro acomodo.** Los tests de dentro del timeline (botones, grilla) corren también con `Workspace::solo(PanelId::Timeline, Edge::Bottom)` en una ventana del mismo ancho de timeline; los de dentro de la toolbar con `Workspace::solo(PanelId::Tools, Edge::Right)`.
- [ ] **Step 4:** `cargo test -p cs-ui` completo → verde; el `grep` ya solo encuentra lo permitido (anotado).
- [ ] **Step 5:** clippy + fmt + wasm; commit `"Tests de layout: lienzo, toolbar, timeline, Preview y tiras medidos por panel"` con la tabla de orígenes.

### Task 5: Los arneses de Timeline y Color hospedan su panel como la app

**Files:** `crates/cs-ui/src/timeline/tests.rs` (`Harness` ~23–96; 36 tests), `crates/cs-ui/src/color_panel/tests.rs` (`Harness` ~24–86, `right_panel` ~256–279; 27 tests con `Harness::new`, `right_panel` también en ~893).

**Interfaces:**
- Consumes: `Workspace::solo` (tarea 2); `ChimpApp::with_parts`, `app.layout(ui, false)` (como `LayoutHarness::frame_with`, layout_tests.rs ~126–218); `app.workspace.leaf_rect(id)`.
- Produces: en cada `Harness`, `app: ChimpApp` en vez de `editor: Editor` (+ `state` del timeline: el que la app ya guarda; búscalo con `grep -n "TimelineState" crates/cs-ui/src/lib.rs crates/cs-ui/src/app_panels.rs`), accesores `fn editor(&mut self) -> &mut Editor` (= `&mut self.app.doc.editor`) y, en el de timeline, `fn state(&mut self) -> &mut TimelineState`. Los rects que guarda el arnés (`nodes`, `grid`, `images`, `flat_rects`) quedan **relativos a la hoja** del panel (`r.translate(-leaf.min.to_vec2())`), así los números de los tests no cambian si el marco del arnés de hoy tenía su origen en (0, 0) de la ventana; si el marco de hoy y el del `Viewer` difieren (márgenes, `pixel_panel_frame`, `timeline_frame`), los tests que lo notan se ajustan **con la diferencia explicada en el commit** (es justo lo que la fase E busca: medir el panel como lo hospeda la app).

- [ ] **Step 1: Timeline.** `Harness::with(width, pixel)`: una `ChimpApp` con `view.pixel_ui = pixel`, el sprite de 8×8 de hoy como documento (`app.doc.editor = Editor::with_sprite(…)` o el equivalente que use `layout_tests`), `app.workspace = Workspace::solo(PanelId::Timeline, Edge::Bottom)`, `view.timeline_height = Some(190.0)`; ventana `width × 400` (el timeline a lo ancho de la ventana); `frame` corre `app.layout(ui, false)` y saca los nodos como hoy. Quita el `Panel::bottom("timeline")` y su comentario. Los 36 tests: `h.editor.` → `h.editor().`, `h.state.` → `h.state().` (sed mecánico + compilar).
- [ ] **Step 2:** `cargo test -p cs-ui timeline::tests` → si algo cambia, es por el marco: explica cada ajuste (qué margen, cuántos puntos) en el reporte; ningún test se borra.
- [ ] **Step 3: Color.** Igual con `Workspace::solo(PanelId::Color, Edge::Right)`, `view.palette_width = width` (o `None` para el de fábrica, como el `width: None` de hoy), `view.palette_height` = el `palette_height` del arnés, ventana 1000×800. `self.action` sale de lo que la app hace con la `PaletteAction` (si la app la aplica dentro del frame y ya no hay forma de leerla, el test verifica el efecto en el editor/vista en su lugar y lo dice). Quita `right_panel` (también su uso en ~893) y su comentario.
- [ ] **Step 4:** `cargo test -p cs-ui color_panel::tests` → verde (mismos criterios que el Step 2); `cargo test -p cs-ui` completo; el número de tests no baja.
- [ ] **Step 5:** clippy + fmt + wasm; commit `"Tests de Timeline y Color: el panel hospedado por el workspace, como en la app"`.

### Task 6: La fila de botones del timeline se parte de 64 a 126 pt

**Files:** `crates/cs-ui/src/timeline/pixel.rs` (`UNITS` ~249, `toolbar` ~313–346, `group` ~500, ids ~513–515), `crates/cs-ui/src/pixel_ui.rs` (`wrap_rows` ~1311, solo si hace falta), `crates/cs-ui/src/width_sweep_tests.rs` (`KNOWN_GAPS` ~52–75), `crates/cs-ui/src/timeline/tests.rs`.

**Interfaces:** Consumes `pixel_ui::wrap_rows(available, &sizes, &joins)` (joins negativos = "comparten una línea", probado en `pixel_ui/tests.rs` ~741), `BAR_ROW_H = 22`, `BAR_PAD`.

Hoy `toolbar` parte la fila solo entre unidades enteras (8 `UNITS`); por debajo de ~127 pt el grupo de Playback (`group_width(5) = 121` + `BAR_PAD` + márgenes) y otras unidades (Layers 97, Duration ~81, Opacity, OnionRange/Blend 59) no caben. Sin Pixel UI ya hay scroll horizontal y no se toca.

- [ ] **Step 1: Tests que fallan** (en `timeline/tests.rs`, con el arnés de la tarea 5):
  - `the_button_row_wraps_down_to_the_minimum` — `Harness::pixel(w)` para `w` en `[64, 80, 100, 126, 127, 200]`: ningún botón del bar se sale de `0..w` (relativo a la hoja), todos en puntos enteros, sin encimarse; con `w = 64` hay más filas que con `127`.
  - `a_split_group_keeps_its_buttons_working` — a 64 pt, clic en `"Play"` y en el último botón de Playback (los que quedan en la segunda fila del grupo): hacen lo mismo que a 600 pt.
  - `the_button_row_is_unchanged_where_it_fit` — a 600 pt y al ancho del timeline en Essentials, los rects relativos de todos los botones son los mismos de antes (cópialos de una corrida antes del cambio).
- [ ] **Step 2:** correr → fallan los dos primeros.
- [ ] **Step 3: Implementar.**
  - En `toolbar`, cada botón de un grupo es una entrada propia de `sizes` (25 por segmento) con `joins = -1.0` entre botones del mismo grupo (comparten su raya), y la junta normal entre grupos. `wrap_rows` ya parte solo donde no cabe.
  - Cada tramo de un grupo que cae en una fila se dibuja con `group(ui, skin, origin, &buttons[a..b])`. El id de `group` (hoy `("timeline_buttons", primer label)`) debe ser **estable** aunque el grupo se parta: usa `("timeline_buttons", label del grupo, a)` para que las respuestas no salten entre filas.
  - Opacity y Duration: su etiqueta/reloj y su stepper son dos entradas (la etiqueta pegada al stepper con `join` negativo cuando caben en la misma fila; si no, el stepper baja a la siguiente).
  - Lo que siga sin caber a 64 pt (una sola entrada más ancha que el panel) es NEEDS_CONTEXT con las medidas.
- [ ] **Step 4:** quitar la entrada `Gap { panel: PanelId::Timeline, from: 64.0, to: 126.0, kind: Kind::Outside, … }` de `KNOWN_GAPS`; `cargo test -p cs-ui width_sweep` → verde (sin "stale known gap"). `cargo test -p cs-ui` completo.
- [ ] **Step 5: Capturas.** Release en `target/gui-e6`, `--demo` a escala 2 con Pixel UI: Essentials 0 px contra main; una con `CHIMPSPRITE_DEMO_WORKSPACE` (o la var de demo que exista para acomodos; si no hay una para "timeline angosto", agrégala en `demo.rs`) con el timeline a ~80 pt de ancho, para el reporte.
- [ ] **Step 6:** clippy + fmt + wasm; commit `"Timeline: la fila de botones se parte por botón en anchos chicos"`.

### Task 7: Las filas de resultados de Lospec, con `forms`

**Files:** `crates/cs-ui/src/palette_ui.rs` (`lospec_row` ~1351–1482, llamado ~926–950; `LOSPEC_PANEL_W` ~719), `crates/cs-ui/src/width_sweep_tests.rs` (`KNOWN_GAPS`), tests de Lospec existentes (`grep -n "lospec" crates/cs-ui/src/layout_tests.rs crates/cs-ui/src/palette_ui*.rs`).

**Interfaces:** Consumes `forms::{form, min_height, row_lines, air_above, air_right, push_right, natural}` (ver `docs/forms.md`); plantilla: la tarjeta de `dafont.rs` (~947–978) y sus constantes `ROW_H = 18.0`, `BUTTON_DROP = 1.0`, `TEXT_DROP = 2.0`, `FIELD_GAP = 5.0` (~276–279).

Hoy cada fila son tres `ui.horizontal` con `right_to_left(Align::Center)`: el centrado de hijos de distinta altura cae a medio punto en y (`Kind::HalfPointY` en todo 364–600).

- [ ] **Step 1: Tests que fallan** (junto a los tests de Lospec que ya existen; usa `forms::testing::run_at` o el `app_for(PanelId::Lospec)` del barrido, con la página offline `lospec_page()`):
  - `lospec_rows_sit_on_whole_points` — a 364, 450 y 600: todos los widgets de las filas en puntos enteros, dentro del ancho, sin encimarse.
  - `lospec_rows_look_like_before_at_364` — a 364, los rects de título, autor, stats, swatches, pager, notas y Save/Append/Apply son los de antes **redondeados hacia abajo al punto** (cópialos de una corrida antes del cambio; la única diferencia permitida es ese medio punto en y, como las excepciones de la fase A).
- [ ] **Step 2:** correr → falla el primero.
- [ ] **Step 3: Implementar** `lospec_row` con `forms::form(ui, ("lospec_row", index, &p.slug), 0.0, |tui| …)` (id único por fila: `docs/forms.md:18–20`), tres filas `forms::row_lines` con caídas enteras como dafont (título/autor con `air_above(TEXT_DROP, …)`, stats con `push_right`, botones con `air_above(BUTTON_DROP, push_right(…))`), swatches + pager en la de en medio. Sin `Align::Center`.
- [ ] **Step 4:** quitar la entrada `Gap { panel: PanelId::Lospec, … Kind::HalfPointY … }`; `cargo test -p cs-ui width_sweep` → verde; `cargo test -p cs-ui` completo.
- [ ] **Step 5: Capturas** (release `target/gui-e7`, escala 2): Lospec acoplado con la página offline (agrega una var de demo en `demo.rs` si no hay una que lo abra offline) a 364 y a 500 pt; comparar contra main: diferencias solo de 1 px de pantalla en y en las filas. Essentials 0 px.
- [ ] **Step 6:** clippy + fmt + wasm; commit `"Lospec: las filas de resultados con forms (sin medios puntos)"`.

### Task 8: Minors de la fase D

**Files:** `workspace/modal.rs` (`Frozen`), `workspace/show.rs` (`canvas_drop` ~658–670) y `workspace/drop.rs` (`drop_zone` ~36–113), `lib.rs` (~1531–1536, el pintado del arrastre de la tira) + `workspace/drop.rs` (`paint_strip_zone`).

**Interfaces:** Produces `drop::paint_strip_zone(ui: &egui::Ui, skin: Option<&Skin>, rect: egui::Rect)` (mismo estilo que `paint_drop_zone`: `skin::drop_shapes` con skin, el relleno de selección sin skin).

- [ ] **Step 1: Foco por árbol (`Frozen::restore(None)`).** Test que falla: extiende `a_modal_puts_back_no_focus_too` — sin foco antes del modal, un clic en una hoja de una ventana flotante con el modal abierto no deja `focused_node` en el árbol de esa ventana (`state[surface].focused_leaf()` es `None` en todas las superficies). Arreglo: `take` guarda `(surface, tree.focused_leaf())` de cada superficie (`iter_surfaces_indexed`); `restore` llama `state[surface].set_focused_node(node.unwrap_or(NodeIndex(usize::MAX)))` en cada una (un nodo que no es hoja pone `None`, `tree/mod.rs` de egui_dock 0.21.1) y después pone la superficie como hoy.
- [ ] **Step 2: La zona "lienzo entero" no incluye el timeline.** Primero reprodúcelo con un test (Essentials, arrastrar la pestaña de Library sobre el lienzo cerca de su borde de abajo; la zona pintada — los rects de `skin::drop_shapes` en `shapes` — no debe bajar del borde de arriba del timeline). Si **no** se reproduce, no cambies código: deja el test (pasa) y anótalo en el reporte. Si sí: cuando el timeline está debajo del lienzo, recorta la zona al lienzo visible (`r.with_max_y(w.timeline_divider(sep).map_or(r.bottom(), |d| d.top()))`) en `canvas_drop` y en `drop_zone`.
- [ ] **Step 3: Resaltar la tira de destino.** Test que falla: arrastrar el ícono de Library de la tira derecha (en un acomodo con una tira izquierda no vacía, p. ej. Tools colapsada con «) hasta encima de la tira izquierda, sin soltar: en `shapes` hay una zona pintada sobre el rect de la tira izquierda; al soltar, Library pasa a ella (ya pasa hoy). Arreglo: en `lib.rs` (~1531), si el puntero está sobre una tira (`strips.iter().find(|(_, r)| r.contains(pointer))`), `paint_strip_zone(ui, skin, rect)` en la misma capa de primer plano que `paint_drop_zone`.
- [ ] **Step 4:** `cargo test -p cs-ui` completo; capturas de Essentials 0 px.
- [ ] **Step 5:** clippy + fmt + wasm; un commit por arreglo (`"Workspace: el modal tampoco deja foco en los árboles"`, `"Workspace: la zona del lienzo entero sin el timeline"` si aplica, `"Workspace: la tira de destino se resalta al arrastrar un ícono"`).

### Task 9: Cierre

**Files:** spec (`docs/superpowers/specs/2026-11-09-workspace-docking-design.md`), `docs/forms.md` (si Lospec agregó un patrón nuevo), `.superpowers/sdd/` (fuera de git).

- [ ] **Step 1: Spec.** En "E. Tests de layout reescritos": **HECHA** con lo que quedó (cómo se mide ahora: `essentials_at_1200_by_800` + `h.at` + `Workspace::solo`; los arneses; el timeline que se parte; Lospec en forms; los minors; `show.rs` partido), "Decisiones tomadas" y "Probar a mano" (timeline angosto con el mouse: clics en los botones de la segunda fila; Lospec acoplado y flotando; arrastrar un ícono sobre otra tira se resalta). Actualiza la línea 23 ("356 tests, 11 285 líneas") con las cifras de hoy. Los pendientes que no se hicieron (íconos `icon.tools`/`icon.color` del skin — los dibuja el usuario; la toolbar flotante más ancha con el mouse; partir `fit.rs`/`model.rs`) van en una lista "Fuera de la fase E".
- [ ] **Step 2: Verificación completa** (lo de `agent-rules.md`): `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, wasm de todos los crates; release + las 14 capturas a escala 2 de Essentials (con y sin Pixel UI) contra main: 0 px.
- [ ] **Step 3:** commit `"Workspace, fase E: spec al día"`; borrar `target/gui-*` salvo el último.
