# Movie clips parte 2 (capas de objetos): plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que una capa de objetos tenga varios objetos (instancias de símbolos), cada uno con sus
propios keyframes, y que se puedan seleccionar, mover, ordenar y animar por separado.

**Architecture:**
- `LayerKind::Instance` + `Layer.instance: Option<InstanceTrack>` pasan a
  `LayerKind::Objects` + `Layer.objects: Vec<SceneObject>`.
- El motor (render, comandos, `.chimp`, MCP, herramienta Move) se migra en una sola tarea para que
  todo compile. Después se agregan las operaciones nuevas y la UI (selección en el lienzo, orden,
  timeline desplegable e inspector).

**Tech Stack:** Rust 2024, egui 0.36, serde.

**Spec:** `docs/superpowers/specs/2026-10-30-object-layers-design.md`. Mapa de los usos actuales:
ver "Mapa del código" abajo.

## Global Constraints

- Toolchain: antes de cualquier `cargo`, en Git Bash:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH`.
- CI corre `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` y
  `CARGO_TARGET_DIR=target/wasm-check2 cargo check --target wasm32-unknown-unknown -p cs-core -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite`.
  Todo debe pasar antes de cada commit.
- Los crates del motor no dependen de egui y compilan para wasm32.
- Comentarios en español, en el estilo del código vecino. Los textos de la UI van en inglés.
- El repo usa `eol=lf`: si un script de edición deja CRLF, quitarlo con `sed -i 's/\r$//'`.
- Commits en español, terminados en `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
  (o la línea de atribución que indique el harness). Sin push.
- No tocar `.design/`. Nunca usar `git stash`. Hacer stage solo de los archivos propios, con rutas
  explícitas.
- **Compatibilidad:**
  - los `.chimp` y autosaves viejos (con `LayerExtra.instance`) cargan como capas de objetos con un
    objeto;
  - el JSON viejo de `Layer` (`kind: "Instance"`, `instance: {...}`) sigue deserializando;
  - toda capa de instancia existente se ve exactamente igual.

## Mapa del código (usos actuales, de un relevamiento)

- **cs-core:**
  - `sprite.rs:36` (`LayerKind::Instance`), `:58-60` (`Layer.instance`), `:83`
    (`new_image`);
  - `instance.rs` (`InstanceKey`, `InstanceTrack`, `key_at`, `set_key`, `normalize`,
    `source_frame`, más la prueba de serde en `:251`);
  - `document.rs:243` `instance_uses`, `:266-272` `next_artboard_id`, `:305`
    `instance_sources`, `:322` `would_cycle`, `:337` `cycle_free`.
- **cs-commands:**
  - `command.rs:124` `SetInstanceTrack` (apply en `:216-230`, label en `:288`); `resize_canvas`
    en `:331`; `remap_instances` en `:684-703` (lo usan `add_frame` `:723` y `remove_frame`
    `:761`, y `:765-772`);
  - `ops.rs:27`, `:42-59` `new_instance_layer`, `:63-76` `update_instance_key`, `:79-90`
    `remove_instance_key`;
  - `symbol.rs:254-261` (arma la capa de instancia), `:423-424` `content_rect`, `:507-510`
    `symbol_sprite`;
  - `sprite_ops.rs:26` `can_transform`; `history.rs:47` (comentario).
- **cs-render:** `materialize.rs`:
  - `materialize_at` en `:78`, con el loop de las capas de instancia en `:93-121`;
  - `instance_image` en `:127-179`, que dibuja una key y devuelve `(position, Image)`.
- **cs-tools:** `move_tool.rs:52-63` `move_output` (capa de instancia → `update_instance_key`),
  `release` en `:105` y `preview` en `:111`.
- **cs-io:**
  - `chimp.rs:52-62` `LayerExtra.instance`; `:82`, `:98` (al guardar); `:121-127` y `:142-145`
    (`apply_extras` al cargar); `:255-275` (reparación de ciclos);
  - `files.rs:89-95` `bakes_instances`;
  - `aseprite.rs:654` y `:1056`.
- **cs-agent:**
  - `ops/structure.rs`: `add_instance_layer` `:453`, `convert_to_symbol` `:482-529`,
    `set_instance_key` `:553-604`, `remove_instance_key` `:606-616`;
  - `ops/document.rs:69-97` `describe_sprite`, y `apply_commands` en `:340-352` (deserializa
    `DocCommand` en JSON crudo);
  - `target.rs:71`; `params.rs:744`, `:785`, `:804`; `dispatch.rs`;
  - `apps/chimpsprite-mcp/src/server.rs:187-218`.
- **cs-ui:**
  - `timeline.rs`:
    - `layer_row` en `:720`;
    - la etiqueta "🖼" en `:1002`;
    - las celdas con ◆ y holds en `:1136-1162`;
    - el menú de celda en `:1233-1256`;
    - `instance_menu` en `:1330-1349`, que se usa en `:1082` y en `app_menus.rs:866`;
  - `panels/inspectors.rs:304-426` `instance` (lo llama `lib.rs:1203`);
  - `instance_ops.rs` (todo);
  - `canvas/input.rs:614-631` (doble clic), `canvas.rs:643` (drop) y `:698` (caché de texturas);
  - `library_panel.rs:100`, `:402`; `color_panel.rs:285`; `panels.rs:617-622`;
    `symbol_ops.rs:42-55`.
- **Choque de nombres:** `cs_commands::symbol::Converted.instance` es un `LayerId`. Se renombra a
  `layer` en la Task 1.

---

### Task 1: Migración del modelo y del motor a capas de objetos

Es la tarea más grande. Al terminar, todo compila y se comporta **exactamente igual que hoy**: cada
capa de objetos que viene de una capa de instancia tiene un objeto. La UI solo se adapta para
compilar, y el objeto que edita es el primero de la capa.

**Files:** todos los sitios del "Mapa del código" de cs-core, cs-commands, cs-render, cs-tools,
cs-io y cs-agent, más los ajustes mínimos de cs-ui para que compile.

**Interfaces producidas:**

```rust
// cs-core/src/sprite.rs
pub enum LayerKind { Image, Group, #[serde(alias = "Instance")] Objects, Tilemap }

// cs-core/src/instance.rs (o un módulo nuevo objects.rs)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneObject { pub id: ObjectId, pub name: String, pub track: InstanceTrack }

// Layer
#[serde(default)] pub objects: Vec<SceneObject>,   // reemplaza `instance`
impl Layer {
    pub fn object(&self, id: ObjectId) -> Option<&SceneObject>;
    pub fn object_mut(&mut self, id: ObjectId) -> Option<&mut SceneObject>;
    pub fn next_object_id(&self) -> ObjectId;       // max + 1, desde 1
    /// Todas las fuentes de todas las keys de todos los objetos.
    pub fn object_sources(&self) -> impl Iterator<Item = ArtboardId> + '_;
}

// cs-commands
Command::SetObjects { layer: LayerId, objects: Vec<SceneObject> }   // reemplaza SetInstanceTrack
// error: NotObjectLayer(LayerId) (renombrar NotInstanceLayer)
pub fn ops::new_object_layer(sprite, above, frame, source, name, position) -> Command  // ex new_instance_layer
pub fn ops::update_object_key(sprite, layer, object: ObjectId, frame, f) -> Option<Command>
pub fn ops::remove_object_key(sprite, layer, object: ObjectId, frame) -> Option<Command>
// symbol::Converted { command, symbol, layer: LayerId }   (ex `instance`)
```

**Requisitos:**
- **Migración de serde para `Layer`:** usar una representación auxiliar (`#[serde(from = "LayerRepr")]`
  o un `deserialize_with`) que acepte el campo viejo `instance: Option<InstanceTrack>`. Si viene
  `Some(track)` y `objects` está vacío, queda
  `objects = vec![SceneObject { id: ObjectId(1), name: layer.name.clone(), track }]`. Al serializar
  solo se escribe `objects`.
  - La prueba existente `instance_layers_serialize_and_old_layers_still_load` se adapta.
  - Se agrega una prueba con JSON viejo literal: `"kind":"Instance","instance":{...}`.
- **`SetObjects`:** normaliza el track de cada objeto, verifica que la capa sea `Objects`
  (`NotObjectLayer`) y que los ids no se repitan, reemplaza la lista y devuelve la anterior.
  - `resize_canvas` desplaza las posiciones de las keys de todos los objetos.
  - `remap_instances` remapea los frames de todos los objetos.
- **Render (`materialize_at`):** para una capa `Objects` visible, en cada frame:
  1. compone **en orden** la imagen de cada objeto (`instance_image` pasa a recibir el `&InstanceTrack`
     del objeto) sobre una imagen RGBA del tamaño del rectángulo que las une a todas, respetando la
     opacidad de key de cada objeto (alfa por objeto);
  2. inserta **un** cel con esa imagen, con opacidad 255.

  Hoy la opacidad de la key va en `cel.opacity`; ahora va dentro de la imagen compuesta. Comprobar
  con las pruebas existentes de opacidad de key × opacidad de capa que el resultado es idéntico (o
  difiere por redondeo ±1; en ese caso documentarlo y ajustar solo esa prueba con tolerancia ±1).
- **`Document`:** `instance_uses` cuenta **objetos** cuyo track usa `id`.
  `next_artboard_id`, `instance_sources`, `would_cycle` y `cycle_free` recorren todos los objetos.
- **`cs-tools/move_tool.rs`:** sobre una capa `Objects` mueve el **primer** objeto, por ahora.
  La Task 4 lo cambia a la selección.
- **`cs-io/chimp.rs`:**
  - `LayerExtra` gana `#[serde(default, skip_serializing_if = "Vec::is_empty")] objects: Vec<SceneObject>`
    y conserva `instance: Option<InstanceTrack>`, que solo se lee.
  - Al cargar, si `objects` está vacío y `instance` es `Some`, se migra.
  - Al guardar, se escribe `objects`.
  - La reparación de ciclos recorre todos los objetos.
- **`cs-agent`:**
  - `add_instance_layer` usa `new_object_layer`;
  - `set_instance_key` y `remove_instance_key` actúan sobre el **primer objeto** de la capa;
  - `describe_sprite` emite `kind: "objects"` y
    `objects: [{id, name, keys}]`, y además `instance: <track del primero>` por compatibilidad.
- **`cs-ui`:** todo lo que hoy lee `layer.instance` pasa a usar el primer objeto. Se agrega a
  `Editor` un `fn current_object(&self) -> Option<ObjectId>`, que por ahora devuelve el primer
  objeto de la capa actual. `is_instance_keyframe`, `instance_key`, `update_instance_key*`,
  `insert_instance_key`, `remove_instance_key`, el inspector y el timeline usan ese objeto.
- **Pruebas:** todas las existentes siguen pasando (adaptadas a los nombres nuevos). Se agregan:
  - JSON viejo de `Layer`;
  - un `.chimp` con `LayerExtra.instance` que carga migrado (escribir el manifest a mano en la
    prueba);
  - el render de una capa con **dos** objetos, donde el de arriba tapa al de abajo y cada uno usa su
    propio `source_frame`;
  - `instance_uses` contando 2 en una capa con dos objetos del mismo símbolo.

- [ ] Implementar con TDD, empezando por las pruebas nuevas de serde y de render de dos objetos.
- [ ] CI local completa (ver Global Constraints).
- [ ] Commit: "Capas de objetos: el modelo, el render, los comandos, el .chimp y MCP pasan de una
  instancia por capa a una lista de objetos (los archivos viejos migran solos)".

---

### Task 2: Operaciones sobre objetos (`cs-commands::ops`)

**Interfaces producidas** (`crates/cs-commands/src/ops.rs` o un `ops/objects.rs` nuevo):

```rust
pub enum Reorder { Forward, Backward, Front, Back }
pub fn add_object(sprite: &Sprite, layer: LayerId, frame: usize, source: ArtboardId, name: &str,
                  position: (i32, i32)) -> Option<(Command, ObjectId)>;
pub fn remove_objects(sprite: &Sprite, layer: LayerId, ids: &[ObjectId]) -> Option<Command>;
/// Desplaza los objetos en `frame` (crea la key ahí si no existe, copiando la vigente).
pub fn move_objects(sprite: &Sprite, layer: LayerId, ids: &[ObjectId], frame: usize,
                    dx: i32, dy: i32) -> Option<Command>;
pub fn reorder_objects(sprite: &Sprite, layer: LayerId, ids: &[ObjectId], to: Reorder) -> Option<Command>;
/// Copias con ids nuevos, nombre "<nombre> copy", desplazadas `offset` en todas sus keys.
pub fn duplicate_objects(sprite: &Sprite, layer: LayerId, ids: &[ObjectId], offset: (i32, i32))
                    -> Option<(Command, Vec<ObjectId>)>;
```

Todas devuelven un solo `SetObjects` (un undo). Devuelven `None` si la capa no es de objetos, si
no hay ids válidos o si nada cambia.

**Reglas de `reorder_objects`:**
- conserva el orden relativo de los objetos movidos;
- `Forward` sube cada bloque seleccionado una posición, por encima del siguiente no seleccionado;
- `Backward` hace lo mismo hacia abajo;
- `Front` y `Back` los mandan a un extremo.

`remove_objects` deja la capa vacía si no queda ningún objeto; no la borra.
`remove_object_key`, de la Task 1, borra el objeto si era su única key.

**Pruebas:**
- cada operación con undo;
- `move_objects` en un frame sin key, que crea la key y deja la de antes intacta;
- los cuatro modos de `reorder` con selecciones no contiguas;
- `duplicate` con ids nuevos y desplazamiento;
- `remove` de todos los objetos, con la capa vacía.

Insertar o borrar frames desplaza las keys de todos los objetos (prueba con dos objetos; si la
Task 1 ya la cubre, no repetirla).

- [ ] TDD, CI local y commit: "Capas de objetos: agregar, borrar, mover, ordenar y duplicar objetos".

---

### Task 3: MCP para objetos

**Files:** `crates/cs-agent/src/{params.rs,dispatch.rs,ops/structure.rs}` y
`apps/chimpsprite-mcp/src/server.rs`.

**Ops nuevas**, con el patrón de `add_instance_layer`:
- `add_object {doc, artboard?, layer, source, name?, frame?, x?, y?}`:
  - capa y fuente por nombre o id (`target::layer`, `target::artboard`);
  - comprueba ciclos con `would_cycle` y que el host sea RGBA;
  - devuelve el id del objeto.
- `set_object_key {doc, artboard?, layer, object, frame?, source?, x?, y?, flip_h?, flip_v?, opacity?, timing?}`,
  igual que `set_instance_key` pero por objeto (el objeto se da por id o por nombre).
- `remove_object {doc, artboard?, layer, object}`.
- `reorder_object {doc, artboard?, layer, object, to: "forward"|"backward"|"front"|"back"}`.

**Prueba:** `add_object` dos veces en la misma capa → `describe` lista 2 objetos → `render`
cambia → `reorder_object` front → `remove_object`.

- [ ] TDD, CI local, `cargo build -p chimpsprite-mcp` y commit: "Capas de objetos: add_object,
  set_object_key, remove_object y reorder_object en MCP".

---

### Task 4: Seleccionar y mover objetos en el lienzo

**Interfaces producidas (cs-ui, `Editor`):**

```rust
pub selected_objects: Vec<ObjectId>,           // objetos seleccionados de `self.layer`
/// El objeto visible de más arriba bajo el punto de mundo (en el artboard mostrado), de cualquier
/// capa de objetos visible y no bloqueada: (capa, objeto).
pub fn object_at(&self, world: (f32, f32)) -> Option<(LayerId, ObjectId)>;
pub fn select_object(&mut self, layer: LayerId, object: ObjectId, additive: bool);
pub fn clear_object_selection(&mut self);
pub fn delete_selected_objects(&mut self) -> bool;
pub fn nudge_selected_objects(&mut self, dx: i32, dy: i32) -> bool;
/// La caja (en píxeles del sprite) de un objeto en el frame actual, para el recuadro.
pub fn object_bounds(&self, layer: LayerId, object: ObjectId) -> Option<(i32, i32, u32, u32)>;
```

**Reglas:**
- **`current_object()`** (Task 1) pasa a devolver el primero de `selected_objects`, o el primer
  objeto de la capa si no hay selección.
- **`object_at`:**
  - recorre las capas de arriba hacia abajo y, dentro de cada capa, los objetos de arriba hacia
    abajo;
  - para cada uno renderiza su key vigente en el frame actual (reusar `cs_render` vía `materialize`
    de un sprite con solo ese objeto, o un helper público nuevo en `cs-render`:
    `object_image(doc, sprite, layer, object, frame) -> Option<((i32, i32), Image)>`);
  - gana el primero cuyo píxel bajo el punto tenga alfa > 0.
  - Cachear por `(revision, frame)` si hace falta para el hover.
- **`selected_objects`:** se vacía al cambiar de capa con `select_layer` y se poda en `ensure_valid`
  y después de undo/redo.

**Herramienta Move** (en `cs-tools/move_tool.rs` y en la entrada del lienzo de `cs-ui`):
- **Al presionar:** si `object_at` encuentra un objeto, se activa su capa y el objeto queda
  seleccionado (con Shift: se agrega o se quita, solo si es de la capa activa).
  - **Arrastrar:** mueve `selected_objects`, con vista previa durante el arrastre. Al soltar se hace
    un solo `ops::move_objects` en el frame actual.
  - **Con Alt:** antes de mover, `duplicate_objects` con offset (0,0); la copia queda seleccionada y
    es la que se mueve. Todo en un solo undo, usando `Editor::named`.
- **Sin objeto bajo el cursor:** se queda el comportamiento de hoy y se vacía `selected_objects`.
- **Flechas** con objetos seleccionados (y sin marquesina): `nudge` de 1 px; con Shift, 10 px.
- **Delete** con objetos seleccionados: los borra (tiene prioridad sobre borrar el cel o la
  selección).
- **Doble clic** con Move: usa `object_at`.
  - Si encuentra un objeto, selecciona ese objeto y entra a su fuente si está en la biblioteca
    (`enter_symbol`); si su fuente está en el lienzo, la activa y la encuadra (como hoy).
  - Si no encuentra nada y está en el modo símbolo, sale un nivel (como hoy).
- **Recuadro:** por cada objeto seleccionado, un rectángulo punteado de 1 px (color de selección
  del skin, o `theme` sin skin) alrededor de `object_bounds`, dibujado en el lienzo encima del
  artboard.

**Pruebas (editor):**
- `object_at` elige el objeto de arriba entre dos superpuestos de capas distintas e ignora capas
  ocultas o bloqueadas;
- clic → selecciona y activa la capa;
- Shift → suma;
- arrastrar → un undo y la key en el frame actual;
- Alt+arrastrar → duplica;
- flechas;
- Delete;
- doble clic sobre un objeto de biblioteca → modo símbolo.

**Prueba de layout:** el recuadro se dibuja.

- [ ] TDD, CI local y commit: "Capas de objetos: seleccionar, mover, duplicar y borrar objetos
  con Move".

---

### Task 5: Orden de objetos (menús y atajos)

- **Keymap:** acciones `BringForward` (Ctrl+↑), `SendBackward` (Ctrl+↓), `BringToFront`
  (Ctrl+Shift+↑), `SendToBack` (Ctrl+Shift+↓), `DuplicateObject` (Ctrl+J si está libre en el
  keymap; si no, sin atajo por defecto) y `DeleteObject` (sin atajo propio; Delete ya lo cubre).
  - Todas en la sección "Object" de `names()`, con gate `NotPainting`.
  - Están habilitadas solo con `selected_objects` no vacío.
- **Menú `Layer`:** submenú "Arrange", con los cuatro de orden, más Duplicate Object y Delete
  Object.
- **Menú contextual del lienzo:** con Move y clic derecho sobre un objeto, primero se selecciona el
  objeto y luego aparecen los mismos ítems.
- **Editor:** `reorder_selected(to: Reorder) -> bool` y `duplicate_selected() -> bool`. Duplicate
  usa un offset de (2, 2) y deja seleccionadas las copias.
- **Pruebas:** los atajos llaman a la operación correcta (con un harness de layout que presiona la
  tecla) y el submenú se deshabilita sin selección.

- [ ] TDD, CI local y commit: "Capas de objetos: Arrange (traer al frente, mandar atrás),
  duplicar y borrar desde menús y atajos".

---

### Task 6: Timeline desplegable

- **`TimelineState`:** `expanded_objects: HashSet<LayerId>`.
- **Fila de una capa de objetos:**
  - un triángulo ▸/▾ (pieza de skin `icon.tree.collapsed`/`expanded` si existe; si no, los
    caracteres `>` y `v`) a la izquierda del nombre lo alterna;
  - la celda muestra los ◆ de la **unión** de las keys de todos sus objetos. Las barras de hold
    salen solo si hay un único objeto.
- **Desplegada:** debajo va una sub-fila por objeto, de arriba hacia abajo (el último de la lista
  primero).
  - Lleva sangría y el nombre del objeto. Un clic en el nombre llama a `select_object`.
  - Muestra sus ◆ y sus barras de hold, con el mismo dibujo de hoy.
  - En el clic derecho sobre una celda, Insert Keyframe y Delete Keyframe operan sobre ese objeto
    (`update_object_key`, `remove_object_key`).
  - La sub-fila del objeto seleccionado se resalta.
- **Altura:** las sub-filas usan la altura de fila normal. El scroll del timeline las incluye.
- **Etiqueta "🖼":** se quita del nombre de la capa en Pixel UI. Si existe la pieza
  `icon.layer.objects`, se dibuja como ícono; si no, nada. Sin Pixel UI, se queda el texto "🖼 ".
- **Pruebas (layout):**
  - con dos objetos y la capa desplegada aparecen los dos nombres;
  - un clic en el nombre selecciona el objeto;
  - el Insert Keyframe de la sub-fila crea la key en ese objeto;
  - en Pixel UI, la etiqueta no contiene "🖼".

- [ ] TDD, CI local y commit: "Capas de objetos: el timeline se despliega con una fila por objeto".

---

### Task 7: Inspector "Object" y entradas de objetos nuevos

- **Inspector** (`panels/inspectors.rs:304-426`):
  - se renombra el panel a "Object";
  - muestra el nombre (editable) y edita el objeto `current_object()`, con los mismos campos de hoy;
  - con varios seleccionados, el campo que cambia se aplica a todos (un undo, con `named`);
  - sin capa de objetos o sin objetos, muestra "Select an object on the canvas".
- **Objetos nuevos:**
  - `drop_symbol` y `add_instance_layer` (`New Instance`): si el destino es el artboard activo y la
    capa activa es `Objects`, hacen `add_object` en esa capa; si no, `new_object_layer` como hoy.
  - En los dos casos el objeto nuevo queda seleccionado.
  - Renombrar en la UI "New Instance" → "New Object" (el submenú). El nombre del archivo
    `instance_ops.rs` se puede quedar.
- **F8:** después de convertir, selecciona el objeto de la capa nueva.
- **Otros sitios:**
  - el estado de `panels.rs:617-622` pasa a decir "Object layer: edit the symbol (double-click)";
  - `color_panel.rs:285` usa `current_object()`.
- **Pruebas:**
  - soltar dos veces sobre una capa de objetos activa deja 2 objetos en la misma capa;
  - el inspector, con dos seleccionados, cambia flip_h en ambos con un undo;
  - el renombrado del objeto queda.

- [ ] TDD, CI local y commit: "Capas de objetos: inspector Object y los objetos nuevos van a la
  capa de objetos activa".

---

### Task 8: Verificación, prueba visual y PR

- [ ] CI local completa.
- [ ] Extender `CHIMPSPRITE_DEMO_SYMBOL` en `crates/cs-ui/src/demo.rs`: con el valor `objects`,
  después de convertir el sombrero, agregar dos objetos más del símbolo en la misma capa (en
  posiciones distintas), seleccionar uno y desplegar la capa en el timeline.
- [ ] Captura:
  1. respaldar `%APPDATA%/ChimpSprite/data/app.ron`;
  2. `CHIMPSPRITE_DEMO_SYMBOL=objects CHIMPSPRITE_SCREENSHOT=D:/Work/chimpdeas/.refs/tmp/objects.png CHIMPSPRITE_SCREENSHOT_DELAY=2 timeout 40 target/gui-checkN/release/chimpsprite.exe --demo`;
  3. revisar la imagen y restaurar.
- [ ] Revisión final de toda la rama, push y PR. El merge, solo con la confirmación del usuario.
