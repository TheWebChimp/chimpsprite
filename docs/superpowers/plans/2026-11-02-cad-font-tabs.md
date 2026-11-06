# Pestañas para modelos y fuentes: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Los modelos 3D y las fuentes pixel salen del lienzo: viven en la Library (secciones
Symbols, Models, Fonts) y se editan en su propia pestaña de la barra de documentos, que es una
vista del mismo documento (mismo `Editor`: undo, paleta, guardado).

**Architecture:** Un predicado `Artboard::on_canvas()` saca modelos y fuentes del lienzo en todo
el código. Cada `DocState` guarda sus pestañas de vista abiertas (`views: Vec<ArtboardId>`); la
barra las pone después de su documento (`TabRef::View`). El `Editor` gana un **workspace**
(`Option<ArtboardId>`): activar una pestaña de vista cambia al documento padre y entra al workspace
de ese artboard. En el workspace de un modelo, el lienzo dibuja los 4 cuadrantes de la parte 2A en
todo el área (el modo Edit con Tab sobre el artboard desaparece); en el de una fuente, solo ese
artboard (como el modo símbolo).

**Tech Stack:** Rust 2024, egui 0.36, el UI kit propio.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-02-cad-font-tabs-design.md`.
- Sin cambio de formato del `.chimp`; los archivos viejos abren con sus modelos y fuentes fuera del
  lienzo; siempre queda al menos un artboard en el lienzo (uno vacío "Sprite" si hace falta).
- La textura de un modelo es un artboard normal del lienzo.
- La pestaña de vista no tiene `DocState` propio: comparte el `Editor` del documento.
- Íconos de pestaña provisionales: `▣` modelo, `Aa` fuente (piezas del skin `tab.icon.model` /
  `tab.icon.font` si existen).
- Textos de UI en inglés ("Symbols", "Models", "Fonts", "Open", …); comentarios en español como el
  código vecino; componentes del kit, no piezas sueltas.
- Nunca `git checkout -- …`, `git restore`, `git stash`, `git reset --hard`, `git clean`; solo
  `git add <archivos propios>`; LF; no tocar `.design/`.
- Toolchain (Git Bash): `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=8`;
  cargo siempre a prioridad baja: `cmd //c "start /low /b /wait cargo test -p <crate> <filtro>"`.
  `cargo fmt --all` y `cargo clippy --workspace --all-targets` sin avisos; commits en español
  terminados con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Mapa de archivos

- `crates/cs-core/src/document.rs` — `Artboard::on_canvas`, `canvas_artboards`, `models`, `fonts`,
  `ensure_canvas_artboard` (Task 1).
- `crates/cs-commands/src/doc_command.rs` — reglas del último artboard con `on_canvas` (Task 1).
- `crates/cs-io/src/chimp.rs` — llamar `ensure_canvas_artboard` al abrir (Task 1).
- `crates/cs-ui/src/editor.rs`, `crates/cs-ui/src/editor/workspace.rs` (nuevo),
  `crates/cs-ui/src/editor/model_edit.rs`, `crates/cs-ui/src/editor/document_ops.rs` — workspace
  (Task 2).
- `crates/cs-ui/src/tabs.rs`, `crates/cs-ui/src/tab_bar.rs`, `crates/cs-ui/src/lib.rs` — pestañas
  de vista (Task 3).
- `crates/cs-ui/src/canvas.rs`, `crates/cs-ui/src/canvas/model_edit.rs`,
  `crates/cs-ui/src/canvas/input.rs`, `crates/cs-ui/src/canvas/model_input.rs` — workspace de
  modelo en todo el área (Task 4) y de fuente (Task 5).
- `crates/cs-ui/src/library_panel.rs`, `crates/cs-ui/src/lib.rs` — Library con secciones (Task 6).
- `crates/cs-ui/src/lib.rs`, `crates/cs-ui/src/instance_ops.rs`, `crates/cs-ui/src/demo.rs`,
  `crates/cs-ui/src/model_panel.rs`, `crates/cs-ui/src/font_panel.rs` — entradas (New…, doble clic
  en instancia, Bake, demo) (Task 7).
- Pruebas UI en `crates/cs-ui/src/layout_tests.rs`.

---

### Task 1: Modelos y fuentes fuera del lienzo (`cs-core`, `cs-commands`, `cs-io`)

**Interfaces — Produces:**
```rust
impl Artboard {
    /// En el lienzo: ni símbolo de la biblioteca, ni modelo 3D, ni fuente.
    pub fn on_canvas(&self) -> bool { !self.in_library && self.model.is_none() && self.font.is_none() }
}
impl Document {
    pub fn canvas_artboards(&self) -> impl DoubleEndedIterator<Item = &Artboard>; // filtra on_canvas()
    pub fn library(&self) -> impl Iterator<Item = &Artboard>;   // sin cambio: in_library (símbolos)
    pub fn models(&self) -> impl Iterator<Item = &Artboard>;    // model.is_some()
    pub fn fonts(&self) -> impl Iterator<Item = &Artboard>;     // font.is_some()
    /// Si no queda ningún artboard en el lienzo, agrega uno vacío "Sprite" del tamaño, modo de
    /// color y paleta del primero, a la derecha de todo. Devuelve si agregó.
    pub fn ensure_canvas_artboard(&mut self) -> bool;
}
```

- [ ] **Step 1: Pruebas que fallan** (en `document.rs` `mod tests`):
  - `on_canvas`: un artboard normal sí; con `in_library`, con `model` o con `font`, no.
  - `canvas_artboards`/`canvas_count` no cuentan modelos ni fuentes; `models()`/`fonts()` los listan.
  - `ensure_canvas_artboard`: un documento cuyo único artboard tiene `font` gana un artboard
    "Sprite" en el lienzo del mismo tamaño y paleta, con un id nuevo; si ya hay uno, devuelve
    `false` y no cambia nada.
  - En `doc_command.rs` (sus pruebas): con un artboard normal y un modelo, `RemoveArtboard` del
    artboard normal se rechaza (es el último del lienzo); el del modelo se puede borrar.
  - En `chimp.rs` (sus pruebas): un `.chimp` con un modelo y su textura abre con el modelo fuera
    del lienzo (`!on_canvas()`) y la textura en el lienzo; uno con solo una fuente abre con un
    artboard "Sprite" extra en el lienzo.
- [ ] **Step 2:** `cargo test -p cs-core document`, `-p cs-commands`, `-p cs-io chimp` → fallan.
- [ ] **Step 3: Implementar.** `on_canvas` en `Artboard`; `canvas_artboards` lo usa; `models`,
  `fonts`, `ensure_canvas_artboard`. En `doc_command.rs`, las dos reglas del último artboard
  (≈109 `RemoveArtboard`, ≈133 `SetArtboardInLibrary`) cuentan con `on_canvas()` /
  `canvas_count()`. En `cs-io` al terminar de leer el documento: `doc.ensure_canvas_artboard();`.
  En `cs-ui` cambia los filtros sueltos por el predicado: `canvas.rs` `shown_artboards` (≈54
  `None => !a.in_library` → `None => a.on_canvas()`), `editor.rs` `on_canvas` (≈566),
  `editor/document_ops.rs` (≈36 `!self.artboard().in_library` → `self.artboard().on_canvas()`),
  `model_panel.rs` (≈533, el selector de textura: `a.on_canvas()`).
- [ ] **Step 4:** Esas pruebas en verde y `cmd //c "start /low /b /wait cargo test --workspace"`.
  Algunas pruebas existentes de `cs-ui` que crean un modelo o una fuente y esperan verlos en el
  lienzo (p. ej. `with_a_model_at_800`, pruebas del modo Edit de la 2A, del panel Font en el
  lienzo) van a fallar: **no las arregles en esta tarea** si dependen del workspace; márcalas con
  `#[ignore = "cad-font-tabs: workspace (Task 4/5)"]` y lístalas en el reporte. Las que fallen por
  otra razón, arréglalas.
- [ ] **Step 5: Commit** — "Modelos y fuentes fuera del lienzo (on_canvas)".

### Task 2: El workspace del `Editor`

**Interfaces — Produces** (nuevo `crates/cs-ui/src/editor/workspace.rs`):
```rust
impl Editor {
    /// El modelo o la fuente que se edita en su pestaña (`None`: el lienzo).
    pub fn workspace(&self) -> Option<ArtboardId>;
    /// Entra al workspace de `id` (un modelo o una fuente): recuerda el artboard del lienzo activo
    /// y activa `id`. `false` si `id` no es modelo ni fuente.
    pub fn enter_workspace(&mut self, id: ArtboardId) -> bool;
    /// Regresa al lienzo, al artboard que estaba activo (o al primero del lienzo).
    pub fn exit_workspace(&mut self);
}
```
Campos nuevos en `Editor`: `pub(crate) workspace: Option<ArtboardId>`,
`workspace_return: Option<ArtboardId>`.

- `enter_workspace` sale del modo símbolo si estaba, guarda `workspace_return` (el activo si está
  en el lienzo), pone `workspace` y llama `activate(id)`.
- `ensure_valid`: si `workspace` apunta a un artboard que ya no existe o ya no es modelo ni fuente,
  `exit_workspace()`; si `workspace == Some(active)`, ese activo es válido aunque no esté en el
  lienzo (hoy `ensure_valid` lo cambiaría al primero del lienzo).
- `activate(other)` con `other` en el lienzo sale del workspace.
- **Modo Edit de la 2A:** `model_edit_on()` pasa a ser `self.workspace == Some(self.active) &&
  self.is_model()`. `ModelEdit.on` y `toggle_model_edit` desaparecen: el estado del modo Edit
  (selección, grid, centro, zoom) se reinicia al entrar al workspace de otro modelo
  (`ModelEdit.artboard`), y `ModelEdit` gana `pub fit: bool` (true al entrar) para que el lienzo
  encuadre el modelo cuando conozca el área. El zoom de las vistas pasa a **puntos de pantalla por
  1/16 de unidad**.
- Tab vuelve a ser solo la línea de tiempo (quitar la rama de Tab de `editor_key` y el
  `drop_focus` que agregó la 2A si ya no hace falta). La casilla "Edit (Tab)" del panel 3D se quita.
- [ ] **Step 1: Pruebas que fallan** (`crates/cs-ui/src/editor/workspace_tests.rs`):
  - `enter_workspace` de un modelo: `workspace()`, `active` y `model_edit_on()`; `exit_workspace`
    regresa al artboard del lienzo que estaba activo.
  - `enter_workspace` de un artboard normal devuelve `false`.
  - Un undo que borra el modelo (p. ej. deshacer `new_model`) saca del workspace.
  - Un gesto de la 2A dentro del workspace (mover un vértice con `start_model_op`/`update`/`finish`)
    es un paso de undo, y deshacerlo con `exit_workspace` hecho también funciona (mismo historial).
  - `activate` de un artboard del lienzo sale del workspace.
  - Las pruebas de `model_edit_tests.rs` (2A) se adaptan: `editing()` usa `enter_workspace` en lugar
    de `toggle_model_edit`; la de "solo en el modelo activo" pasa a probar `exit_workspace`.
- [ ] **Step 2:** `cargo test -p cs-ui workspace_tests` → falla.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** Verde; `cargo test -p cs-ui` (las ignoradas de la Task 1 siguen ignoradas).
- [ ] **Step 5: Commit** — "Editor: workspace para editar un modelo o una fuente fuera del lienzo".

### Task 3: Pestañas de vista en la barra

**Interfaces — Produces** (`tabs.rs`):
```rust
pub(crate) enum TabRef { Doc(usize), View(usize, ArtboardId), Page(Page) }
pub(crate) enum TabIcon { None, Model, Font }
pub(crate) struct TabInfo { pub name: String, pub dirty: bool, pub icon: TabIcon }
impl DocState { pub views: Vec<ArtboardId> }          // pestañas de vista abiertas, en orden
impl Tabs {
    /// Abre (o activa) la pestaña de `artboard` del documento activo.
    pub fn open_view(&mut self, current: &mut DocState, artboard: ArtboardId);
    /// Activa una pestaña de vista: cambia a su documento y entra al workspace.
    pub fn switch_view(&mut self, current: &mut DocState, doc: usize, artboard: ArtboardId);
    pub fn close_view(&mut self, current: &mut DocState, doc: usize, artboard: ArtboardId);
}
```
- `bar()`: por cada documento `i`, `Doc(i)` y luego `View(i, id)` por cada `id` de sus `views`;
  después las páginas.
- `bar_active()`: si hay documento activo y su editor está en workspace `Some(id)` → `View(active,
  id)`; si no `Doc(active)`.
- `bar_infos()`: la vista lleva el nombre del artboard, `dirty` = el de su documento e ícono
  `Model`/`Font`.
- `switch(index)` (a un documento) sale del workspace del documento destino (`exit_workspace`).
- `switch_view`: `switch(doc)` (si no es el activo) y `current.editor.enter_workspace(artboard)`.
- `close_view`: lo quita de `views`; si era la vista activa, `exit_workspace`.
- **Sincronía:** cada frame (en `lib.rs`, donde se actualiza el documento activo) se quitan de
  `views` los artboards que ya no existen o ya no son modelo/fuente; si se renombra, el título
  cambia solo (se lee del documento).
- Cerrar un documento cierra sus vistas (vienen con su `DocState`).
- `tab_bar.rs`: dibuja el ícono antes del nombre (`▣` o `Aa` con la fuente de la UI, o las piezas
  `tab.icon.model`/`tab.icon.font` del skin si existen); `TabAction` gana lo necesario para
  activar y cerrar una vista; `lib.rs` `tab_action` despacha a `switch_view`/`close_view`. Ctrl+Tab
  (`cycle`) recorre también las vistas.
- [ ] **Step 1: Pruebas que fallan** (en `tabs.rs` y `layout_tests.rs`):
  - `bar()` con dos documentos, el primero con una vista: `[Doc(0), View(0, id), Doc(1)]`.
  - `open_view` + `bar_active` apunta a la vista; `switch(1)` y volver con `switch_view(0, id)`
    entra al workspace; `close_view` de la activa regresa al lienzo de ese documento.
  - Cerrar el documento 0 deja `[Doc(0)]` (el que era 1).
  - Layout: con un modelo, `open_view` dibuja una pestaña con el nombre del modelo justo después
    de la del documento; clic en la pestaña del documento regresa al lienzo; clic en la de la vista
    vuelve al workspace; la `×` de la vista la cierra.
  - Borrar el modelo (o deshacer su creación) cierra su pestaña.
- [ ] **Step 2–4:** Fallan → implementar → verde (`cargo test -p cs-ui tabs`, layout, y todo `cs-ui`).
- [ ] **Step 5: Commit** — "Pestañas de vista: modelos y fuentes junto a su documento".

### Task 4: El workspace de ChimpCAD en todo el área

- Con `editor.model_edit_on()` (workspace de un modelo), `CanvasView::show` no dibuja artboards,
  reglas, cuadrículas ni nada del lienzo: llena el área del lienzo (`canvas_area`, sin reglas) con
  los 4 cuadrantes de `canvas/model_edit.rs` (`quadrants` recibe ese rectángulo y lo parte a la
  mitad en puntos enteros). `draw_model_edit` toma `area: Rect` en vez del `screen` del artboard; la
  rama de la 2A dentro del bucle de artboards se quita.
- `edit_view_map`: `scale = editor.model_edit().zoom` (puntos por 1/16; ya no multiplica por el
  zoom del lienzo). Si `model_edit().fit`, el lienzo encuadra: centro = centro del modelo, zoom
  para que quepa en el cuadrante con aire (0.8), y pone `fit = false` (método del editor
  `fit_model_views(quadrant_size: Vec2)`).
- **Persp:** el render a la resolución del artboard (`model_render` con el tamaño del sprite), a la
  escala entera más grande que quepa en el cuadrante (mínimo 1), centrado. `persp_press` mapea el
  punto a pixeles del artboard con esa escala; el `factor` de `ModelDrag` se ajusta igual.
- En el workspace del modelo: la rueda sin Ctrl, el zoom del lienzo, la mano, el scroll y las
  herramientas no hacen nada; Ctrl + rueda hace lo de la 2A (vistas orto o distancia de la cámara
  en Persp). Los gestos de la 2A siguen idénticos.
- Las pruebas de layout de la 2A (`editing_model`, `front_screen`, Tab…) se adaptan: `editing_model`
  abre el modelo en su pestaña (`open_view`) en lugar de Tab; las que se ignoraron en la Task 1 por
  esto se reactivan. Agrega:
  - el workspace dibuja los 4 cuadrantes cubriendo el área del lienzo (sin reglas) y no dibuja la
    textura ni otros artboards;
  - la perspectiva se dibuja a escala entera;
  - Ctrl + rueda sobre Front cambia el zoom de las vistas sin cambiar el zoom del lienzo.
- [ ] **Steps:** pruebas → fallan → implementar → verde → todo `cs-ui` → commit "ChimpCAD: el
  workspace del modelo en todo el área".

### Task 5: El workspace de FontForge

- En el workspace de una fuente, `shown_artboards` devuelve solo ese artboard (como el modo
  símbolo); las herramientas, el zoom, el scroll, la letra fantasma, las guías y la línea del ancho
  funcionan como hoy. Al entrar, encuadra el glifo con el panel Font al lado (`FitActiveBeside`, lo
  que hoy hace `new_font`) y abre el panel Font si estaba cerrado.
- Sin migas de pan del modo símbolo en el workspace.
- Las pruebas de la fuente ignoradas en la Task 1 se reactivan, abriendo la fuente en su pestaña.
- Prueba nueva: en el workspace de la fuente el lienzo dibuja solo ese artboard; pintar un pixel
  cambia el glifo y es un paso de undo que se deshace desde la pestaña del documento.
- [ ] **Steps:** pruebas → fallan → implementar → verde → todo `cs-ui` → commit "FontForge: el
  workspace de la fuente".

### Task 6: La Library con Symbols, Models y Fonts

- `library_panel.rs`: tres secciones con título y cuenta ("Symbols 2", "Models 1", "Fonts 0");
  vacías, una línea de ayuda ("F8 makes a symbol", "File → New 3D Model…", "File → New Font…").
- Filas de modelos: miniatura (el render del modelo, como hoy las miniaturas de símbolos), nombre,
  usos como instancia. Filas de fuentes: miniatura (la muestra de la tarjeta de la fuente o el
  glifo actual) y nombre.
- **Doble clic** en un modelo o una fuente → `LibraryAction::Open(id)` → `tabs.open_view`. En
  símbolos sigue `Edit` (modo símbolo).
- Menú de la fila: modelos y fuentes → Open, Rename (F2), Duplicate, Delete (Delete pide
  confirmación si un modelo tiene usos); modelos además Place on Canvas; arrastrar un modelo al
  lienzo pone una instancia (el `SymbolPayload` de hoy).
- La vista previa animada de arriba muestra lo elegido en cualquier sección.
- Pruebas (layout): las tres secciones con sus cuentas; doble clic en el modelo abre su pestaña;
  Rename cambia el título de la pestaña; Delete del modelo cierra su pestaña; arrastrar el modelo
  al lienzo crea una instancia.
- [ ] **Steps:** pruebas → fallan → implementar → verde → todo `cs-ui` → commit "Library: secciones
  Symbols, Models y Fonts; doble clic abre la pestaña".

### Task 7: Entradas, demo y cierre

- *File → New 3D Model…* y *New Font…* (y los botones de los paneles 3D y Font) crean como hoy y
  abren la pestaña (`open_view`) del modelo o la fuente nueva.
- Doble clic en una instancia de un modelo (`instance_ops.rs` ≈332) abre su pestaña.
- **Bake…** desde la pestaña del modelo: el artboard horneado aparece en el lienzo y se activa la
  pestaña del documento con él.
- `CHIMPSPRITE_DEMO_MODEL` abre la casita en su pestaña; `CHIMPSPRITE_DEMO_MODEL_EDIT` además en
  modo Vértice con uno elegido. Las demás demos de fuentes, si abren el panel Font, abren la fuente
  en su pestaña.
- Spec: `Estado: implementado` con notas de la implementación.
- Verificación: `cargo fmt --all`, clippy del workspace, `cargo test --workspace`,
  `cargo check --target wasm32-unknown-unknown -p cs-3d -p cs-render -p cs-ui`; captura del demo
  (ver la memoria de verificación GUI) revisando la pestaña, los 4 cuadrantes y la Library.
- [ ] **Steps:** pruebas (New 3D Model abre la pestaña; doble clic en instancia abre la pestaña;
  Bake regresa a la pestaña del documento) → fallan → implementar → verde → commit "Pestañas:
  New…, instancias, Bake y demo".
