# Workspace, fase D: toda la ventana en el árbol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que toda la ventana (entre la barra de opciones y la de estado) sea un solo árbol de docking con el lienzo protegido al centro; toolbar, timeline y preview acoplables; tiras de íconos por orilla; workspaces con nombre y menú Window — con Essentials idéntico a hoy.

**Architecture:** El `DockState<PanelId>` de `workspace` pasa de la columna derecha a toda el área central. `PanelId::Canvas` es una hoja más, protegida por normalizaciones después de cada frame (`tidy_canvas`, `pin_sides`) y por cortar su arrastre, como probó el spike (`proto-workspace-d`, variante 1). Los anfitriones fijos de hoy (paneles de egui para Tools, Color/columna, Timeline; la ventana de Preview) desaparecen: todo panel se dibuja en su hoja por `draw_panel`. Encima, presets con nombre y el menú Window.

**Tech Stack:** Rust 2024, egui/eframe 0.36, egui_dock `=0.21.1`, cs-ui.

**Spec:** `docs/superpowers/specs/2026-11-09-workspace-docking-design.md` (fase D: "Diseño aprobado (2026-11-11)" y la lista "Para la fase D"). **Spike:** `git -C D:/Work/chimpdeas/cs-font show 639761a:crates/cs-ui/examples/proto_workspace_d.rs` (funciones `one_ui`, `tidy_canvas` ~3162, `pin_sides` ~845, bloqueo del arrastre ~435/2582, `place_window`, el reemplazo de la zona sobre el lienzo) y `…:crates/cs-ui/examples/PROTO_WORKSPACE_D_NOTES.md`.

## Global Constraints

- Worktree `D:/Work/chimpdeas/cs-tiles`, rama `workspace-docking-d`. Reglas de agentes: `D:/Work/chimpdeas/.refs/agent-rules.md` (léelas completas; git prohibido: checkout/restore/stash/reset/clean/add -A/add .; nunca `python -`; nunca matar procesos por nombre).
- Toolchain: `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-tiles/target`; todo cargo con `cmd //c "start /low /b /wait cargo …"`. El disco estuvo al 97%: borra tus `target/gui-*` al terminar cada tarea.
- egui_dock `=0.21.1`; solo `workspace` (y sus submódulos) lo usa.
- **Con Essentials la app se ve igual que hoy** (capturas a escala 2 con 0 pixeles distintos en reposo, con y sin Preview abierto, en la pestaña de un modelo y con un diálogo abierto). Los tests existentes pasan; si uno mide un detalle de implementación (un id de `egui::Panel` que ya no existe, p. ej. `panel("tools")`), se adapta **sin cambiar la geometría que verifica** y se justifica en el commit; si eso no se puede, NEEDS_CONTEXT.
- Cada panel existe una sola vez (tira, acoplado o flotando). El lienzo existe exactamente una vez, solo en su grupo, en la superficie principal.
- Rects y divisores en puntos enteros.
- Textos de UI en inglés; comentarios y docs en español; un commit por tarea con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Al final de cada tarea: `cargo test -p cs-ui`, `cargo clippy -p cs-ui --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check --target wasm32-unknown-unknown -p cs-ui`.

## File Structure

- Split `crates/cs-ui/src/workspace.rs` → `workspace/mod.rs` (API y `Workspace`), `workspace/model.rs` (operaciones, invariantes, `from_saved`), `workspace/drop.rs` (`drop_target`, soltar desde la tira, soltar pestañas), `workspace/fit.rs` (mínimos, `fit_splits`, `fit_windows`, `pin_sides`, `tidy_canvas`), `workspace/show.rs` (`show`, `Viewer`, «, ×, franja), `workspace/skin.rs` (ya existe).
- Modify `crates/cs-ui/src/panel_registry.rs` (`PanelId::Canvas`, `default_size`), `app_panels.rs` (Canvas, Tools, Timeline, Preview por el workspace), `lib.rs` (`editor_panels` sin anfitriones fijos), `preview.rs` (controles en `contents`), `dock.rs` (tiras por orilla), `app_menus.rs`/`app_actions.rs` (menú Window).
- Create `crates/cs-ui/src/workspace/presets.rs` (de fábrica y del usuario).

---

### Task 1: Partir `workspace.rs` sin cambiar nada

**Files:** `crates/cs-ui/src/workspace.rs` → `crates/cs-ui/src/workspace/{mod.rs, model.rs, drop.rs, fit.rs, show.rs}` (+ `skin.rs` igual). Tests: los existentes (se mueven con su código).

- [ ] **Step 1:** Mover el código por responsabilidad (lista de arriba), con `pub(super)`/`pub(crate)` mínimos; los tests del módulo van al submódulo que prueban. Sin renombrar API pública del crate.
- [ ] **Step 2:** `cargo test -p cs-ui` → mismos tests, mismos resultados (número igual).
- [ ] **Step 3:** clippy + fmt + wasm + commit `"workspace: partido en model, drop, fit y show (sin cambios)"`.

### Task 2: El lienzo como hoja protegida y el árbol en toda la ventana (Color sigue a la derecha)

**Files:** `panel_registry.rs` (`PanelId::Canvas`: llave `"canvas"`, no acoplable desde la tira, no cerrable, `Host::Center`), `workspace/model.rs`, `workspace/fit.rs` (`tidy_canvas`, `pin_sides`), `workspace/show.rs` (Viewer: Canvas dibuja el lienzo de hoy — el contenido de `CentralPanel` en `editor_panels` — con el estilo/márgenes del `CentralPanel`; `stop_dragging` sobre el arrastre de su pestaña/franja), `workspace/skin.rs` (zona de destino sobre el lienzo repintada como la mitad que queda; `separator.extra = 0`), `lib.rs` (`editor_panels`: el `DockArea` ocupa el área central donde hoy están `CentralPanel` + el panel derecho `"palette"`; Tools y Timeline siguen siendo `egui::Panel` en esta tarea), `workspace/model.rs` `from_saved` (migrar versión 1 → 2: meter el lienzo a la izquierda de la columna guardada).

**Interfaces:** Produces `PanelId::Canvas`; `Workspace::essentials()` = `H[Canvas | Color]` con el lado derecho en puntos; `fn tidy_canvas(&mut DockState<PanelId>, release: Option<Pos2>)` y `fn pin_sides(&mut DockState<PanelId>, area: Rect, sides: &mut SidePoints)` (de `proto_workspace_d.rs`, con tests puros); `docked/column_min_width/drop_target/from_saved/host_width` sin la suposición "superficie principal = columna derecha" (la columna derecha es "el lado derecho del lienzo").

- [ ] **Step 1: Tests que fallan** (puros, en `workspace/fit.rs` y `model.rs`):
  - `tidy_canvas`: si Library cae como pestaña junto al lienzo, queda partido al lado del cuadrante del soltado y el lienzo queda solo; si el lienzo fue a otra superficie o desapareció, vuelve a la principal una vez.
  - `pin_sides`: al cambiar el área de 1000→800 de ancho, el lado derecho conserva sus puntos y el lienzo absorbe la diferencia; al volver a 1000 el lado sigue igual (sin perder puntos — el spike perdía 11).
  - `from_saved` de un `SavedWorkspace` versión 1 (la columna sola) da el lienzo a la izquierda y la columna con su ancho.
  - Layout test: con Essentials, la geometría del lienzo (`app.doc.canvas` rect / el `canvas` del arnés) y del panel de color es la de hoy.
- [ ] **Step 2–5:** correr → falla; implementar portando del spike; `cargo test -p cs-ui` completo; capturas a escala 2 `--demo`, con un diálogo, en la pestaña de un modelo y `CHIMPSPRITE_DEMO_WORKSPACE=library-tab|split|float` antes/después (Essentials 0 px); clippy + fmt + wasm + commit `"Workspace: el lienzo como hoja protegida y el árbol en toda la ventana"`.

### Task 3: El toolbar en el árbol

**Files:** `lib.rs` (`editor_panels`: sin `Panel::left("tools")`), `workspace/model.rs` (Essentials: `H[Tools | H[Canvas | Color]]` con Tools en puntos = `pixel_ui::toolbar_width(skin)`), `workspace/show.rs` (Tools con su `tools_scroll_area` y márgenes de hoy), `panel_registry.rs` (`Host` de Tools = árbol; se puede cerrar → tira izquierda; su `min_width` = su ancho real del skin).

- [ ] **Step 1: Tests que fallan:** Essentials con la geometría de hoy del toolbar y sus botones (reusa aserciones de los tests de layout de `panel("tools")`; adapta los que buscan el id `"tools"` a la hoja de Tools sin cambiar las medidas); mover Tools a la derecha del lienzo con `dock`/`DockTarget` lo deja ahí y el lienzo se ensancha; cerrarlo lo manda a una tira.
- [ ] **Step 2–5:** como la tarea 2 (capturas 0 px en Essentials); commit `"Workspace: el toolbar es un panel del árbol"`.

### Task 4: El timeline en el árbol

**Files:** `lib.rs` (sin `Panel::bottom("timeline")`), `workspace/model.rs` (Essentials: `H[Tools | V[H[Canvas | Color]... ]]` — reproduce la anidación de hoy: Tools y Color de alto completo, Timeline entre ellos debajo del lienzo; su alto en puntos = el de hoy, `dock::TIMELINE_DEFAULT_HEIGHT` o el guardado), `panel_registry.rs` (`default_size: Vec2` en la ficha; Timeline `(_, 190)`), `workspace/show.rs` (Timeline con `timeline::pixel_panel_frame` y su `available()`: cuando no está disponible (estructura bloqueada / modo modelo) su hoja dibuja nada y se colapsa a 0 de alto sin perder su lugar; `view.timeline` lo oculta igual), `app_actions.rs`/`keymap.rs` (el toggle de Timeline muestra/oculta su hoja en vez del `Panel`).

- [ ] **Step 1: Tests que fallan:** Essentials con la geometría de hoy del timeline (adapta los de `panel("timeline")`); el toggle View → Timeline lo oculta y lo vuelve a mostrar en su lugar; en modo modelo no se dibuja; moverlo a la derecha lo acopla ahí (lo que probó el spike).
- [ ] **Step 2–5:** como la tarea 2; commit `"Workspace: el timeline es un panel del árbol"`.

### Task 5: Preview en el árbol

**Files:** `preview.rs` (`contents` con sus controles play/pausa y centrar cuando no hay barra de título de ventana; `Preview::stop()`), `workspace/model.rs` (Essentials: Preview flotante en la posición de hoy (`OFFSET`, esquina abajo-derecha del lienzo) cuando `view.preview`), `workspace/show.rs` (Preview en hoja o ventana), `app_panels.rs` (`ClosePreview` llama `stop()`), `lib.rs` (sin la ventana propia de Preview).

- [ ] **Step 1: Tests que fallan:** con Preview encendido, Essentials lo muestra flotando donde hoy y con su tamaño (adapta los tests de Preview sin cambiar medidas); acoplado junto a Color muestra sus controles dentro y funcionan; cerrarlo detiene la reproducción; View → Preview lo muestra/oculta.
- [ ] **Step 2–5:** como la tarea 2 (captura con Preview abierto 0 px); commit `"Workspace: Preview es un panel del árbol (flota por defecto)"`.

### Task 6: Una instancia por panel y tiras por orilla

**Files:** `workspace/show.rs`/`app_panels.rs` (registro de "quién manda" por frame: si un panel ya se dibujó en el frame, los demás anfitriones muestran "Shown in …" — generaliza el caso de 3D), `dock.rs` + `workspace/model.rs` (tiras por orilla: `strips: [Vec<PanelId>; 4]` o por `Edge`; la de la derecha es la de hoy; « de un grupo pegado a una orilla manda sus paneles a la tira de esa orilla; cerrar una pestaña la manda a la tira de la orilla más cercana a donde estaba), `workspace/model.rs` `from_saved` (la tira guardada de versión 1 es la derecha).

- [ ] **Step 1: Tests que fallan:** un grupo pegado a la izquierda colapsa a una tira a la izquierda (con sus íconos y flyouts); Essentials sin tira izquierda visible (vacía = oculta, 0 px); la pestaña de un modelo con 3D acoplado sigue igual que en la fase C.
- [ ] **Step 2–5:** como la tarea 2; commit `"Workspace: una instancia por panel y tiras de íconos por orilla"`.

### Task 7: Workspaces con nombre y menú Window

**Files:** Create `workspace/presets.rs`; Modify `app_menus.rs`/`app_actions.rs` (menú Window entre View y Help: lista de paneles con ✓ (clic = mostrar en su lugar o mandar a la tira), submenú Workspace (Essentials, Animation, Tiles, Fonts, separador, los del usuario), New Workspace…, Delete…, Reset "<nombre>"; quitar Reset Workspace de View), `lib.rs` (`save()`/carga: `SavedWorkspace` versión 2 con `current: String` y `user: Vec<(String, SavedLayout)>`), `workspace/model.rs`.

**Interfaces:** `fn factory(name: &str) -> Option<Workspace>` con `"Essentials"`, `"Animation"` (Timeline alto ~ 40% y Preview acoplado a la derecha del timeline), `"Tiles"` (Slices y Artboards acoplados en pestañas a la derecha junto a Color), `"Fonts"` (FontForge acoplado a la derecha); el acomodo actual se guarda al cerrar como hoy; elegir un workspace lo carga; Reset vuelve el actual a como se guardó (o a su versión de fábrica); New Workspace… pide nombre con un diálogo del kit (copiar de los diálogos existentes, p. ej. el de renombrar artboard).

- [ ] **Step 1: Tests que fallan:** cada preset de fábrica respeta invariantes y se dibuja sin pánico; Essentials se ve igual que hoy; New Workspace "Mine" + cambiar a Animation + volver a "Mine" restaura el acomodo; Delete lo quita; migración de versión 1 → 2; el menú Window lista los 13 paneles (12 en web) con su ✓.
- [ ] **Step 2–5:** como la tarea 2 (capturas de los 4 presets revisadas con Read); commit `"Workspaces con nombre (Essentials, Animation, Tiles, Fonts y los del usuario) y menú Window"`.

### Task 8: Pendientes de la fase C

**Files:** `workspace/show.rs`, `workspace/skin.rs`, tests de layout.

- [ ] **Step 1: Tests que fallan:**
  - con un diálogo modal abierto, arrastrar un divisor, cambiar de pestaña o arrastrar de la tira no hace nada;
  - con muchas pestañas en un grupo angosto aparecen las flechas del kit (`kit::tabs`) y la última pestaña / su × no queda bajo el «;
  - test de barrido de anchos: cada panel acoplado a anchos de `min_width` a 600 en pasos de 1 (o de 2 si es lento; documenta el tiempo) con `forms::testing` → sin medios pixeles, nada fuera, nada encimado;
  - medición (no test que falle): tiempo por frame de `workspace::show` arrastrando un divisor 60 frames con los paneles de Essentials + 3 acoplados; regístralo en el reporte y en el spec.
- [ ] **Step 2–5:** correr → falla; implementar; `cargo test -p cs-ui` completo; commit `"Workspace: modal, flechas de pestañas y barrido de anchos"`.

### Task 9: Cierre de la fase D

- [ ] **Step 1:** `grep -rn "egui_dock" crates/ --include=*.rs` → solo `workspace/`. `grep -rn "Panel::left(\"tools\")\|Panel::bottom(\"timeline\")\|Panel::right(\"palette\")" crates/` → vacío (salvo tests adaptados con comentario).
- [ ] **Step 2:** CI completo: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, check de wasm de todos los paquetes como CI.
- [ ] **Step 3:** Spec: fase D HECHA (qué quedó, decisiones, lista "Probar a mano" actualizada: arrastrar a los 4 lados del lienzo, mover toolbar/timeline/preview, presets, menú Window, tiras por orilla) y lo que queda para la fase E.
- [ ] **Step 4:** commit `"Fase D del workspace: toda la ventana en el árbol (docs)"`.
