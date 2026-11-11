# Workspace, fase C: docking del lado derecho Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que la columna derecha (Color) y los paneles de la tira de íconos (Artboards, Library, Slices, Lospec, Dafont, Groom, Lumen, FontForge, 3D) se puedan acoplar en pestañas y divisiones, flotar en ventanas, redimensionar y regresar a la tira, con el acomodo guardado al cerrar — viéndose igual que hoy con el acomodo de fábrica.

**Architecture:** Un módulo `crates/cs-ui/src/workspace.rs` es el único que conoce egui_dock (`=0.21.1`): guarda `Workspace { state: DockState<PanelId>, strip: Vec<PanelId> }`, viste egui_dock con el skin (técnicas del prototipo `proto-workspace`, commit `8a47d11`) y muestra el `DockArea` en el panel derecho de la ventana (el que hoy aloja Color, conservando su id `"palette"`). Cada pestaña se dibuja con `ChimpApp::draw_panel` del registro (fase B) y sus `PanelAction` se juntan y se aplican después. La tira de íconos de la derecha es el "grupo colapsado a íconos": muestra `workspace.strip` y sigue abriendo flyouts como hoy; sus íconos se arrastran al área acoplada o al lienzo (ventana flotante). Primero se resuelven los acoplamientos a anfitrión que dejó la fase B (spec, "Para la fase C").

**Tech Stack:** Rust 2024, egui/eframe 0.36, egui_dock 0.21.1 (feature `serde`), cs-ui.

**Spec:** `docs/superpowers/specs/2026-11-09-workspace-docking-design.md` (fase C y "Para la fase C"). **Prototipo:** `git -C D:/Work/chimpdeas/cs-tiles show 8a47d11:crates/cs-ui/examples/proto_workspace.rs` (copia en `D:/Work/chimpdeas/.refs/tmp/proto_ws.rs`): estilo de egui_dock (~713-760), `DockArea` (~773-781), pestañas del kit pintadas encima (~783-860), plugin `SkinRewriter` de colores marcador (~100-160), `snap_dock` (~1290), `TabViewer` (~1214-1290). Notas del prototipo: `crates/cs-ui/examples/PROTO_WORKSPACE_NOTES.md` en ese commit.

## Global Constraints

- Worktree `D:/Work/chimpdeas/cs-tiles`, rama `workspace-docking-c`. Reglas de agentes: `D:/Work/chimpdeas/.refs/agent-rules.md` (léelas completas; git prohibido: checkout/restore/stash/reset/clean/add -A/add .; nunca `python -`; nunca matar procesos por nombre).
- Toolchain: `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-tiles/target`; todo cargo con `cmd //c "start /low /b /wait cargo …"`.
- Dependencia: `egui_dock = { version = "=0.21.1", features = ["serde"] }`. Solo `workspace.rs` hace `use egui_dock`.
- **Con el acomodo de fábrica (Essentials) la app se ve igual que hoy** (capturas a escala 2 con 0 pixeles distintos en reposo): Color a la derecha sin barra de pestañas, la tira de íconos con sus flyouts. Los tests existentes pasan sin cambios; el panel anfitrión de la derecha conserva el id `"palette"`.
- **Decisión del usuario (2026-11-10):** un grupo acoplado con **un solo panel no lleva barra de pestañas** (se ve como hoy); al pasar el mouse por su borde de arriba aparece una franja delgada para agarrarlo y arrastrarlo. Con 2 o más paneles sí hay barra de pestañas (pestañas del kit).
- Cada panel existe una sola vez: o está en la tira, o acoplado, o flotando. Cerrar una pestaña o una ventana flotante lo regresa a la tira (nunca desaparece).
- Todos los rects y splits en puntos enteros (con la escala 2 de la UI, cada punto es un pixel del skin).
- La pestaña de un modelo 3D (su panel fijo `"model_panel"`) no cambia en esta fase.
- Textos de UI en inglés; comentarios y docs en español; un commit por tarea con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Al final de cada tarea: `cargo test -p cs-ui`, `cargo clippy -p cs-ui --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check --target wasm32-unknown-unknown -p cs-ui`.

## File Structure

- Modify `crates/cs-ui/src/app_panels.rs` — entrada única `draw_panel(ui, id, &PanelCtx)` (tarea 1), "visto este frame" (tarea 3).
- Modify `crates/cs-ui/src/panel_registry.rs` — `key`, `body`, `min_height`, `available`, serde por llave (tarea 2).
- Create `crates/cs-ui/src/workspace.rs` — `Workspace`, operaciones del árbol, piel, `DockArea`, `TabViewer` (tareas 4-6).
- Create `crates/cs-ui/src/workspace/skin.rs` — estilo, pestañas del kit, plugin de colores marcador, ajuste a pixeles (tarea 4).
- Modify `crates/cs-ui/src/lib.rs` — el panel derecho `"palette"` aloja el `DockArea`; la tira toma sus íconos de `workspace.strip` (tarea 5).
- Modify `crates/cs-ui/src/dock.rs` — íconos de la tira desde `Workspace::strip`; arrastrar íconos (tarea 6).
- Modify `crates/cs-ui/src/app_menus.rs` / `app_actions.rs` — Window → Reset Workspace (tarea 7).

---

### Task 1: Entrada única `draw_panel(ui, id, &PanelCtx)`

**Files:** Modify `crates/cs-ui/src/app_panels.rs` (`draw_panel`, `draw_panel_modal`, `draw_panel_in`, `panel_flyout`, `draw_color`, `draw_lospec`), `crates/cs-ui/src/lib.rs` (los anfitriones que llaman a `draw_panel*`). Test: `mod tests` de `app_panels.rs`.

**Interfaces:**
- Produces: `pub(crate) struct PanelCtx { pub modal: bool, pub bottom: Option<f32> }` (`Default`: `modal: false, bottom: None`) y `pub(crate) fn draw_panel(&mut self, ui: &mut egui::Ui, id: PanelId, ctx: &PanelCtx) -> Option<PanelAction>`. `draw_panel` hace él mismo el `add_enabled_ui(!ctx.modal)` (salvo Color, que maneja el modal por dentro como hoy) y dibuja la franja del logo de Lospec **fuera** de ese scope, igual que hoy `panel_flyout`. `bottom` es el fondo disponible del panel (lo usa Lospec para medir su lista; si es `None`, `ui.max_rect().bottom()`). Desaparecen `draw_panel_modal` y `draw_panel_in`; `panel_flyout` solo arma el flyout y llama a `draw_panel`.

- [ ] **Step 1: Test que falla** — en `app_panels.rs`:

```rust
#[test]
fn a_modal_disables_every_panel_but_color_keeps_its_palette() {
    let mut app = test_app();
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let modal = PanelCtx { modal: true, bottom: None };
    // Library con modal: su botón Import… sale deshabilitado.
    let out = run_frame(&ctx, |ui| { app.draw_panel(ui, PanelId::Library, &modal); });
    assert!(disabled_buttons(&out).iter().any(|l| l == "Import…"));
    // Color con modal: el menú Presets ⏷ se apaga pero la paleta sigue viva.
    let out = run_frame(&ctx, |ui| { app.draw_panel(ui, PanelId::Color, &modal); });
    assert!(!buttons(&out).iter().any(|l| l == "Presets ⏷"));
}
```

(`test_app`, `run_frame`, `buttons`, `disabled_buttons`: usa/crea los helpers del módulo de tests; `run_frame` corre un frame con un `CentralPanel` y regresa el `FullOutput`; `buttons`/`disabled_buttons` leen los nodos de accesskit como `layout_tests.rs` (~161-210). Si la etiqueta real de Library/Color difiere, usa la real.)

- [ ] **Step 2:** `cargo test -q -p cs-ui --lib a_modal_disables` → no compila (`PanelCtx` no existe).
- [ ] **Step 3:** implementar; actualizar todos los llamadores (flyouts, Tools, Color, Timeline, modelo, Preview) a `draw_panel(ui, id, &PanelCtx { modal, bottom })`, quitando su propio `add_enabled_ui(!modal)` donde `draw_panel` ya lo hace (cuidado: el panel de Tools hoy pone `tools_scroll_area()` antes del `add_enabled_ui`; conserva ese orden: el scroll lo pone el anfitrión).
- [ ] **Step 4:** `cargo test -p cs-ui` completo sin tocar tests existentes; capturas `--demo`, `CHIMPSPRITE_DEMO_LOSPEC=1`, `CHIMPSPRITE_DEMO_GROOM=empty` y una con un diálogo abierto (p. ej. Canvas Size: `CHIMPSPRITE_DEMO_ARTBOARDS=2` + el que abra Canvas Size en demo.rs, o `New Sprite`) antes/después: 0 pixeles distintos.
- [ ] **Step 5:** clippy + fmt + wasm + commit `"Paneles: una sola entrada draw_panel con PanelCtx (modal y fondo)"`.

---

### Task 2: Ficha completa y llaves estables

**Files:** Modify `crates/cs-ui/src/panel_registry.rs`. Test: su `mod tests`.

**Interfaces:**
- Produces en `PanelSpec`: `pub key: &'static str` (estable para guardar: `"tools"`, `"color"`, `"timeline"`, `"preview"`, `"artboards"`, `"library"`, `"slices"`, `"lospec"`, `"dafont"`, `"groom"`, `"lumen"`, `"font"`, `"model"`), `pub body: Body` (sale de `Host::Flyout(Body)`: `Host::Flyout` queda sin dato y `body` vale para cualquier anfitrión; Tools/Color/Timeline/Preview = `Body::Fixed`), `pub min_height: f32` (Groom `groom::MIN_HEIGHT`, Lumen `lumen::MIN_HEIGHT`, los demás 0.0).
- `impl PanelId { pub(crate) fn from_key(key: &str) -> Option<PanelId>; pub(crate) fn key(self) -> &'static str }` — `from_key("dafont")` es `None` en wasm.
- `impl serde::Serialize for PanelId` (escribe la llave) y `impl<'de> serde::Deserialize<'de> for PanelId` (lee la llave; llave desconocida → error de serde, que la tarea 7 filtra antes con `filter_map_tabs` sobre `DockState<String>`).
- `pub(crate) fn available(self, app: &ChimpApp) -> bool` (en `app_panels.rs` o donde haya acceso a `ChimpApp`): Timeline = `!editor.is_structure_locked() && !editor.model_edit_on()` (las condiciones de hoy en `editor_panels`); Preview = no estar en la pestaña de un modelo; el resto `true`.

- [ ] **Step 1: Tests que fallan:**

```rust
#[test]
fn keys_are_stable_unique_and_round_trip() {
    let mut seen = std::collections::HashSet::new();
    for &p in PanelId::ALL {
        assert!(seen.insert(p.key()), "{p:?}");
        assert_eq!(PanelId::from_key(p.key()), Some(p));
        let ron = ron::to_string(&p).unwrap();
        assert_eq!(ron, format!("\"{}\"", p.key()));
        assert_eq!(ron::from_str::<PanelId>(&ron).unwrap(), p);
    }
    assert_eq!(PanelId::Font.key(), "font");
    assert_eq!(PanelId::from_key("nope"), None);
}

#[test]
fn the_body_and_min_height_are_in_the_spec() {
    assert_eq!(PanelId::Groom.spec().min_height, crate::groom::MIN_HEIGHT);
    assert!(matches!(PanelId::Font.spec().body, Body::Fixed));
    assert!(matches!(PanelId::Artboards.spec().body, Body::Fit));
    assert!(matches!(PanelId::Library.spec().body, Body::Tall));
}
```

(Si `ron` no está en las dependencias de dev de cs-ui, usa `serde_json`, que ya usan otros tests.)

- [ ] **Step 2–5:** correr → falla; implementar (y adaptar `panel_flyout`/`dock` a `spec.body`); `cargo test -p cs-ui` completo sin tocar tests existentes; clippy + fmt + wasm; commit `"Registro: llaves estables, body y min_height en la ficha, y disponibilidad"`.

---

### Task 3: "Visto este frame" y el ciclo de vida de Lospec

**Files:** Modify `crates/cs-ui/src/app_panels.rs`, `crates/cs-ui/src/lib.rs` (el `close_lospec_panel()` que corre cuando el `app_panel` del dock no es Lospec, ~1618), `crates/cs-ui/src/palette_ui.rs`.

**Interfaces:** Produces `ChimpApp::panels_drawn: Vec<PanelId>` (o `HashSet`) que `draw_panel` llena y el inicio de cada frame vacía, y `ChimpApp::was_drawn(id) -> bool` (el frame anterior). La regla de Lospec pasa a ser: al final del frame, si Lospec **no** se dibujó en este frame, `close_lospec_panel()`.

- [ ] **Step 1: Test que falla** — dibujar Lospec por `draw_panel` en dos frames seguidos fuera de un flyout (en un `CentralPanel`, como lo hará el docking) y comprobar que su ventana/estado (`lospec_dock`) **sobrevive** entre frames (hoy se borraría porque el `app_panel` del dock no es Lospec) y que `start_search` se lanzó una sola vez (cuenta con un contador `#[cfg(test)]` o revisando que la búsqueda en curso es la misma). Y otro: si en un frame no se dibuja, al terminar ese frame se cierra.
- [ ] **Step 2–5:** correr → falla; implementar; `cargo test -p cs-ui` completo sin tocar tests existentes (los de Lospec en `layout_tests.rs` ~1000 y ~2534 deben seguir pasando); clippy + fmt + wasm; commit `"Paneles: visto este frame; Lospec se cierra solo cuando deja de dibujarse"`.

---

### Task 4: Módulo `workspace` con egui_dock vestido

**Files:** Create `crates/cs-ui/src/workspace.rs`, `crates/cs-ui/src/workspace/skin.rs`; Modify `crates/cs-ui/Cargo.toml` (`egui_dock = { version = "=0.21.1", features = ["serde"] }`), `crates/cs-ui/src/lib.rs` (`mod workspace;`). Test: `mod tests` en `workspace.rs`.

**Interfaces:**
- Produces:
  - `pub(crate) struct Workspace { state: egui_dock::DockState<PanelId>, strip: Vec<PanelId> }` con `pub(crate) fn essentials() -> Workspace` (Color acoplado solo en la superficie principal; `strip` = `PanelId::DOCK` en su orden), `fn strip(&self) -> &[PanelId]`, `fn docked(&self, id: PanelId) -> bool`, `fn floating(&self, id: PanelId) -> bool`.
  - Operaciones (todas mantienen "cada panel una sola vez"): `fn dock(&mut self, id: PanelId, into: DockTarget)` (`DockTarget::Leaf(SurfaceIndex, NodeIndex)` agrega como pestaña; `DockTarget::Split(SurfaceIndex, NodeIndex, egui_dock::Split)` divide; `DockTarget::Focused` usa `push_to_focused_leaf`), `fn float(&mut self, id: PanelId, at: egui::Pos2)` (`add_window` y coloca la ventana), `fn to_strip(&mut self, id: PanelId)` (lo saca del árbol y lo pone en la tira en su orden de `DOCK`), `fn collapse_leaf(&mut self, surface, node)` (todas sus pestañas a la tira).
  - `pub(crate) fn show(app: &mut ChimpApp, ui: &mut egui::Ui, modal: bool)` — muestra el `DockArea` con la piel en `ui` (toma `app.workspace` con `std::mem::take`, arma un `TabViewer` que llama `app.draw_panel(ui, id, &PanelCtx { modal, bottom: None })` y junta las `PanelAction` en un `Vec`, regresa el workspace y aplica las acciones con `app.apply_panel_action(ctx, a)` al final). Cerrar una pestaña (`on_close`) o una ventana → `to_strip`.
  - Un grupo con **una sola pestaña** lleva `set_tab_bar_hidden(true)`; con 2+ `false` (se recalcula cada frame); `DockArea::hidable_tab_bars(true)` y la franja de mostrar la barra con `style.tab_bar.show_tab_bar_size` (un par de puntos) y `show_tab_bar_hover_expand`, vestida con el skin.
- Piel (`skin.rs`, portada del prototipo): `egui_dock::Style` desde el skin (barra transparente, alto del kit, separadores de `pixel_ui::ETCHED_SIZE` con colores marcador, overlay `HighlightedAreas` con color marcador, `surface_fade_opacity` 1.0); pestañas del kit pintadas encima en la misma capa con el rect de `TabViewer::on_tab_button`; el fantasma de la pestaña arrastrada; plugin de egui `SkinRewriter` que cambia los rects de color marcador por piezas (`etched_hline/vline`, el 9-slice de zona de destino, `menu.side` para el marco de ventanas flotantes); `snap(&mut DockState)` que redondea las fracciones de los splits y la posición de las ventanas a puntos enteros al soltarlas (el prototipo lo hace en `snap_dock`).

- [ ] **Step 1: Tests que fallan** (en `workspace.rs`, sin ventana):

```rust
#[test]
fn essentials_docks_color_and_strips_the_rest() {
    let w = Workspace::essentials();
    assert!(w.docked(PanelId::Color));
    assert_eq!(w.strip(), PanelId::DOCK);
}

#[test]
fn a_panel_lives_in_exactly_one_place() {
    let mut w = Workspace::essentials();
    w.dock(PanelId::Library, DockTarget::Focused);
    assert!(w.docked(PanelId::Library) && !w.strip().contains(&PanelId::Library));
    w.float(PanelId::Library, egui::pos2(100.0, 100.0));
    assert!(w.floating(PanelId::Library) && !w.docked(PanelId::Library));
    w.to_strip(PanelId::Library);
    assert!(!w.floating(PanelId::Library) && w.strip().contains(&PanelId::Library));
    // La tira conserva el orden de DOCK.
    assert_eq!(w.strip(), PanelId::DOCK);
}

#[test]
fn one_tab_hides_the_tab_bar_and_two_show_it() {
    let mut w = Workspace::essentials();
    w.refresh_tab_bars();
    assert!(w.tab_bar_hidden_of(PanelId::Color));
    w.dock(PanelId::Library, DockTarget::Focused);
    w.refresh_tab_bars();
    assert!(!w.tab_bar_hidden_of(PanelId::Color));
}

#[test]
fn collapsing_a_group_sends_its_tabs_to_the_strip() {
    let mut w = Workspace::essentials();
    w.dock(PanelId::Library, DockTarget::Focused);
    let (surface, node) = w.leaf_of(PanelId::Library).unwrap();
    w.collapse_leaf(surface, node);
    assert!(!w.docked(PanelId::Library) && !w.docked(PanelId::Color));
    assert!(w.strip().contains(&PanelId::Library) && w.strip().contains(&PanelId::Color));
}
```

(`refresh_tab_bars`, `tab_bar_hidden_of`, `leaf_of` son helpers `pub(crate)` del módulo; Color puede ir a la tira aunque no esté en `DOCK`: la tira acepta cualquier panel acoplable y ordena por `ALL`.)

- [ ] **Step 2:** correr → no compila.
- [ ] **Step 3:** implementar el modelo (operaciones) y luego `show` + piel, portando del prototipo. Agrega una variable de demo `CHIMPSPRITE_DEMO_WORKSPACE=<escenario>` (`library-tab`: Library acoplado junto a Color; `split`: Groom debajo de Color; `float`: Lumen flotando) que arme el workspace al arrancar, para capturas.
- [ ] **Step 4:** `cargo test -p cs-ui` completo; las capturas de esta tarea se toman en la tarea 5 (cuando ya se usa en la app).
- [ ] **Step 5:** clippy + fmt + wasm + commit `"workspace: egui_dock con la piel pixel art (pestañas, divisores, zonas y ventanas)"`.

---

### Task 5: El panel derecho aloja el workspace; la tira muestra `strip`

**Files:** Modify `crates/cs-ui/src/lib.rs` (`editor_panels`: el `Panel::right("palette")` sigue con ese id, su ancho guardado/apretón de manos y su línea grabada, pero adentro llama a `workspace::show`; el panel `"dock"` sigue igual), `crates/cs-ui/src/dock.rs` (`show` dibuja los botones de `app.workspace.strip()` en vez de `PanelId::DOCK`; los flyouts de la tira siguen como hoy). Test: `layout_tests.rs` (tests nuevos) y los existentes.

**Interfaces:** Consumes `Workspace`, `workspace::show`. Produces `ChimpApp::workspace: Workspace` (por defecto `essentials()`).

- [ ] **Step 1: Tests que fallan** (en `layout_tests.rs`, con el arnés de siempre):
  - con Essentials, la geometría del panel `"palette"` y de sus controles es la de hoy (reusa las aserciones de un test de layout existente del panel de color como referencia, p. ej. el que mide `panel("palette")`),
  - con `CHIMPSPRITE_DEMO_WORKSPACE=library-tab` armado por código (`app.workspace.dock(PanelId::Library, DockTarget::Focused)`): hay pestañas "Color" y "Library" en la columna derecha y el botón "Library" ya **no** está en la tira,
  - cerrar la pestaña Library (clic en su ×) la regresa a la tira.
- [ ] **Step 2:** correr → falla.
- [ ] **Step 3:** integrar. Con Essentials el `DockArea` muestra solo Color, sin barra (grupo de una pestaña) → debe verse idéntico al panel de hoy; si el `DockArea` agrega márgenes, quítalos en el estilo.
- [ ] **Step 4:** `cargo test -p cs-ui` completo sin tocar tests existentes; capturas a escala 2 `--demo` antes/después (0 pixeles distintos en reposo) y de los tres escenarios de `CHIMPSPRITE_DEMO_WORKSPACE` (revisadas con Read: pestañas del kit, divisor grabado, ventana flotante con marco `menu.side`, todo en pixeles enteros).
- [ ] **Step 5:** clippy + fmt + wasm + commit `"Workspace en la columna derecha: Color acoplado y la tira de íconos como grupo colapsado"`.

---

### Task 6: Sacar paneles de la tira (arrastrar y menú)

**Files:** Modify `crates/cs-ui/src/dock.rs` (botones de la tira: arrastre y menú contextual), `crates/cs-ui/src/workspace.rs` (dónde cae: hoja bajo el puntero, división en el borde, o flotante sobre el lienzo), `crates/cs-ui/src/lib.rs` (aplicar el resultado). Test: `workspace.rs` y `layout_tests.rs`.

**Interfaces:** Produces `workspace::drop_target(&Workspace, pointer: egui::Pos2, dock_rect: egui::Rect) -> DropTarget` con `DropTarget::{Tab(SurfaceIndex, NodeIndex), Split(SurfaceIndex, NodeIndex, egui_dock::Split), Float(egui::Pos2)}` (función pura sobre los rects de las hojas, testeable), y en la tira: arrastrar un ícono más de 6 puntos lo "levanta" (se dibuja un fantasma con el ícono y el título bajo el puntero); al soltarlo sobre la columna acoplada → `Tab` (centro de una hoja) o `Split` (franja de 1/4 en el borde de una hoja); en cualquier otro lado → `Float` en el puntero. Clic sigue abriendo el flyout como hoy. Clic derecho en un ícono → menú "Dock" (→ `DockTarget::Focused`) y "Float" (→ ventana junto a la tira). En la barra de un grupo acoplado (o en la franja de un grupo de una pestaña), un botón « "Collapse to Icons" → `collapse_leaf`.

- [ ] **Step 1: Tests que fallan:**

```rust
#[test]
fn the_drop_target_is_a_tab_in_the_middle_a_split_at_the_edge_and_a_window_outside() {
    let mut w = Workspace::essentials();
    let dock_rect = egui::Rect::from_min_size(egui::pos2(1000.0, 50.0), egui::vec2(220.0, 600.0));
    w.layout_for_test(dock_rect); // da a las hojas su rect como lo haría show()
    let (s, n) = w.leaf_of(PanelId::Color).unwrap();
    assert!(matches!(drop_target(&w, egui::pos2(1110.0, 350.0), dock_rect), DropTarget::Tab(ss, nn) if (ss, nn) == (s, n)));
    assert!(matches!(drop_target(&w, egui::pos2(1110.0, 640.0), dock_rect), DropTarget::Split(_, _, egui_dock::Split::Below)));
    assert!(matches!(drop_target(&w, egui::pos2(400.0, 300.0), dock_rect), DropTarget::Float(_)));
}
```

  y en `layout_tests.rs`: arrastrar el botón "Groom" de la tira (press, moves, release con eventos crudos como los tests existentes) hasta el centro de la columna derecha lo deja como pestaña junto a Color; arrastrarlo al lienzo crea una ventana flotante con Groom; clic simple sigue abriendo el flyout; el menú contextual "Dock" lo acopla.
- [ ] **Step 2–5:** correr → falla; implementar; `cargo test -p cs-ui` completo sin tocar tests existentes; capturas del fantasma al arrastrar (con un plugin de eventos como el del prototipo, o un estado de demo) y de los resultados; clippy + fmt + wasm; commit `"Workspace: arrastrar paneles de la tira para acoplarlos o flotarlos"`.

---

### Task 7: Guardar el acomodo y Reset Workspace

**Files:** Modify `crates/cs-ui/src/workspace.rs` (serde), `crates/cs-ui/src/lib.rs` (`save()` ~2052 y la carga al arrancar), `crates/cs-ui/src/app_menus.rs` + `app_actions.rs` (Window → Reset Workspace). Test: `workspace.rs` y un test de guardar/cargar como los existentes de `ViewSettings`.

**Interfaces:** Produces `const WORKSPACE_KEY: &str = "chimpsprite.workspace"`; `Workspace::to_saved(&self) -> SavedWorkspace` y `Workspace::from_saved(SavedWorkspace) -> Workspace` donde `SavedWorkspace { state: egui_dock::DockState<String>, strip: Vec<String>, width: Option<f32> }` (serde); `from_saved` convierte con `filter_map_tabs(|k| PanelId::from_key(k))`, descarta paneles desconocidos o repetidos, mete a la tira los acoplables que falten, y si queda inválido (sin Color en ningún lado y vacío) regresa `essentials()` con un `log::warn!`. El ancho del panel derecho sigue en `ViewSettings.palette_width` (no lo dupliques: `width` es `None` mientras así sea; quítalo del struct si no hace falta).

- [ ] **Step 1: Tests que fallan:**

```rust
#[test]
fn a_saved_workspace_round_trips() {
    let mut w = Workspace::essentials();
    w.dock(PanelId::Library, DockTarget::Focused);
    w.float(PanelId::Lumen, egui::pos2(300.0, 200.0));
    let ron = ron::to_string(&w.to_saved()).unwrap();
    let back = Workspace::from_saved(ron::from_str(&ron).unwrap());
    assert!(back.docked(PanelId::Library) && back.floating(PanelId::Lumen));
    assert_eq!(back.strip().len(), w.strip().len());
}

#[test]
fn unknown_or_repeated_panels_are_dropped_and_missing_ones_go_to_the_strip() {
    let mut saved = Workspace::essentials().to_saved();
    saved.state.push_to_focused_leaf("nope".to_string());
    saved.state.push_to_focused_leaf("color".to_string()); // repetido
    saved.strip.retain(|k| k != "groom"); // falta
    let w = Workspace::from_saved(saved);
    assert!(w.strip().contains(&PanelId::Groom));
    assert_eq!(w.state_tabs().iter().filter(|p| **p == PanelId::Color).count(), 1);
}
```

  y uno de la app: con un workspace cambiado, `save()` + arrancar una app nueva con ese storage lo restaura; Window → Reset Workspace vuelve a Essentials (un solo paso, sin undo: no es del documento).
- [ ] **Step 2–5:** correr → falla; implementar; `cargo test -p cs-ui` completo sin tocar tests existentes; clippy + fmt + wasm; commit `"Workspace: se guarda al cerrar, se restaura al abrir y Window → Reset Workspace"`.

---

### Task 8: Cierre de la fase C

**Files:** Modify `docs/superpowers/specs/2026-11-09-workspace-docking-design.md` (fase C hecha; qué quedó para D), `docs/forms.md` si cambió algo de `forms`.

- [ ] **Step 1:** `grep -rn "egui_dock" crates/ --include=*.rs` → solo `workspace.rs` y `workspace/skin.rs`.
- [ ] **Step 2:** Verificación completa como CI: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check --target wasm32-unknown-unknown -p cs-core -p cs-3d -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite`.
- [ ] **Step 3:** Escribir en el spec la lista para probar a mano (arrastres reales con el mouse: acoplar como pestaña, dividir, flotar, mover la ventana, cerrar → tira, colapsar a íconos, Reset Workspace, cerrar y abrir la app) y lo que queda para la fase D (toolbar, timeline y preview en el árbol; el lienzo como nodo central; presets con nombre; menú Window con la lista de paneles).
- [ ] **Step 4:** commit `"Fase C del workspace: docking del lado derecho (docs)"`.
