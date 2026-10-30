# Movie clips, parte 2: capas de objetos (varias instancias por capa)

Fecha: 2026-10-30 · Estado: aprobado

## Objetivo

Hoy cada instancia de un símbolo es una capa (`LayerKind::Instance`, con un solo
`InstanceTrack`). Con 20 monedas hay 20 capas, y no se puede tener una capa "Escena" con varios
objetos adentro, como en Flash.

Esta parte convierte la capa de instancia en una **capa de objetos**: una capa con una lista de
objetos, cada uno con su símbolo y **sus propios keyframes**.

Decisiones tomadas con el usuario:

- **Solo las capas de objetos llevan objetos.** Las capas de píxeles siguen siendo solo píxeles.
- **Keyframes por objeto**, como en After Effects. Cada objeto anima por su cuenta y los tweens
  futuros serán por objeto.
- **Seleccionar con Move:** un clic toma el objeto visible de más arriba bajo el cursor, de
  cualquier capa de objetos, y activa su capa. Shift+clic suma objetos de la misma capa.
- **Migración automática.** Una capa de instancia vieja carga como capa de objetos con un objeto.

Fuera de alcance:

- seleccionar objetos de capas distintas a la vez;
- agrupar objetos;
- "romper" un objeto en píxeles (Ctrl+B de Flash);
- snapping y alineación;
- editar en contexto y tweens (partes siguientes).

## 1. Modelo (`cs-core`)

```rust
pub enum LayerKind { Image, Group, Objects, Tilemap }   // `Instance` se lee como `Objects`

pub struct ObjectId(pub u32);

/// Un objeto de una capa de objetos: un símbolo con su propia línea de tiempo.
pub struct SceneObject {
    pub id: ObjectId,
    pub name: String,
    pub track: InstanceTrack,
}

pub struct Layer {
    // …
    /// Objetos de una capa `Objects`, de abajo hacia arriba (el último queda encima).
    #[serde(default)]
    pub objects: Vec<SceneObject>,
}
```

- `InstanceTrack`, `InstanceKey`, `Timing` y `source_frame` no cambian. Lo que antes era "la
  instancia de la capa" ahora es el `track` de cada objeto.
- **Ids:** `ObjectId` es único dentro de la capa, y `Layer::next_object_id()` da el siguiente.
- **Migración al deserializar:**
  - el variant `Instance` de `LayerKind` se lee como `Objects` (`#[serde(alias = "Instance")]`);
  - si viene el campo viejo `instance: Some(track)` y `objects` está vacío, se convierte en
    `objects = [SceneObject { id: 1, name: <nombre de la capa>, track }]`. Se hace con un
    `Deserialize` de ayuda (`LayerRepr`), así los autosaves y los `.chimp` viejos cargan sin cambios.
- **Ayudantes que ahora recorren todos los objetos de todas las capas:**
  `Document::{instance_sources, instance_uses, next_artboard_id}` y `would_cycle`/`cycle_free`.
  `instance_uses` cuenta **objetos** (antes contaba capas).
- **`Layer::object(id)` / `object_mut(id)`.**

## 2. Render (`cs-render`)

`materialize` convierte hoy cada capa de instancia en una capa de imagen con un cel. Ahora el cel de
una capa de objetos se arma componiendo sus objetos **en orden**, cada uno con su key vigente,
`source_frame`, flips y opacidad de key. Sobre ese cel siguen aplicando, como antes, la opacidad de
la capa, el blend, la máscara y el clip. `bake_instances` y el export no cambian de firma. El límite
de profundidad sigue en 16.

## 3. Comandos (`cs-commands`)

- **`Command::SetInstanceTrack` se reemplaza por `Command::SetObjects { layer, objects }`**, que
  cambia la lista completa (con undo, igual que `SetSlices`).
  - Para no romper las macros de scripts ni MCP, `SetInstanceTrack` se elimina y sus usos pasan a
    `SetObjects`.
  - Si existe un formato serializado de comandos con ese variant (por ejemplo, un historial
    guardado), hay que verificarlo en el plan.
- **Operaciones en `ops`, que devuelven `Option<Command>`:**
  - `new_object_layer(sprite, above, frame, source, name, position)`: lo que hoy es
    `new_instance_layer`, con un objeto.
  - `add_object(sprite, layer, frame, source, name, position) -> (Command, ObjectId)`.
  - `remove_objects(sprite, layer, &[ObjectId])`: si la capa queda vacía, la capa se queda (vacía);
    no se borra sola.
  - `move_objects(sprite, layer, &[ObjectId], frame, dx, dy)`: crea la key en `frame` si no existe.
  - `update_object_key(sprite, layer, object, frame, f)` y `remove_object_key(...)`, con la misma
    semántica que las de instancia de hoy. Borrar la única key borra el objeto; si era el último,
    la capa se queda vacía (como con `remove_objects`: la capa nunca se borra sola, tampoco desde
    `remove_instance_key` del MCP). `remove_objects_key(sprite, layer, &[ObjectId], frame)` hace
    lo mismo sobre varios objetos en un solo comando (la fila plegada del timeline).
  - `reorder_objects(sprite, layer, &[ObjectId], Reorder::{Forward, Backward, Front, Back})`.
  - `duplicate_objects(sprite, layer, &[ObjectId], offset) -> (Command, Vec<ObjectId>)`.
- **Insertar o borrar frames** desplaza las keys de todos los objetos.
- **`symbol::convert_to_symbol`** deja una capa de objetos con un objeto. Convertir capas de objetos
  sigue siendo válido (anida).

## 4. UI (`cs-ui`)

**Selección**
- `Editor::selected_objects: Vec<ObjectId>` (de `editor.layer`). Se vacía al cambiar de capa y se
  poda si un objeto deja de existir.

**Herramienta Move sobre el lienzo**
- **Clic:** prueba los objetos de las capas de objetos visibles y no bloqueadas, de arriba hacia
  abajo. Gana el primer objeto cuyo píxel bajo el cursor es opaco en su frame actual. Se activa su
  capa y queda `selected_objects = [id]`.
- **Clic en vacío:** si no hay objeto bajo el cursor, se mantiene el comportamiento de hoy (mover
  el cel o la selección de la capa activa) y se vacía `selected_objects`.
- **Shift+clic:** agrega o quita un objeto de la capa activa.
- **Arrastrar:** mueve los seleccionados. Al soltar, un `move_objects` (un undo).
- **Flechas:** mueven 1 px; con Shift, 10 px.
- **Delete:** borra los seleccionados.
- **Alt+arrastrar:** duplica y mueve la copia.
- **Doble clic sobre un objeto:** si su fuente está en la biblioteca, entra al modo símbolo; si está
  en el lienzo, la selecciona y la encuadra.
- **Recuadro de selección:** un rectángulo punteado alrededor de la caja del objeto en el frame
  actual (con el color de selección del skin).

**Menú `Layer`, menú contextual del objeto y atajos**
- Bring Forward (Ctrl+↑), Send Backward (Ctrl+↓), Bring to Front (Ctrl+Shift+↑) y Send to Back
  (Ctrl+Shift+↓).
- Delete Object y Duplicate Object (Ctrl+J, si está libre en el keymap; si no, sin atajo).

**Timeline**
- La fila de una capa de objetos muestra los ◆ de todos sus objetos juntos. Su menú (Insert y
  Delete Keyframe) actúa sobre los objetos seleccionados o, si la capa tiene uno solo, sobre ese.
  Con varios y ninguno seleccionado, queda deshabilitado ("Expand the layer to edit an object's
  keys").
- Un triangulito ▸/▾ a la izquierda del nombre despliega una sub-fila por objeto (sangrada, con el
  nombre del objeto) que muestra sus ◆ y sus barras de hold.
- En la sub-fila, el clic derecho sobre un frame ofrece Insert Keyframe y Delete Keyframe de ese
  objeto. Un clic en el nombre selecciona el objeto.
- El estado desplegado se guarda en el estado del timeline, sin undo.

**Inspector**
- El panel "Instance" pasa a llamarse "Object" y edita el objeto seleccionado: nombre, fuente, modo,
  frame inicial, Loop/Once/Single, flips y opacidad.
- Con varios objetos seleccionados, aplica a todos solo el campo que se cambia.
- Sin objeto seleccionado, muestra "Select an object on the canvas". Si la capa tiene un solo
  objeto, ese cuenta como seleccionado; con varios, no se edita ninguno.
- Con un objeto que todavía no tiene key en el frame actual (ni antes), muestra "No keyframe at
  or before this frame".

**Objetos nuevos**
- **Soltar desde la Library y `New Instance`:** si la capa activa es de objetos, agregan un objeto
  ahí; si no, crean una capa de objetos encima de la activa.
- **F8:** crea la capa de objetos con el objeto nuevo.
- En todos los casos, el objeto nuevo queda seleccionado.

**Fila de la capa en el timeline**
- La etiqueta "🖼" sobre el nombre se quita en Pixel UI (en esa fuente se dibuja encima del texto).
  Si el skin tiene la pieza `icon.layer.objects`, se dibuja como ícono; si no, no se dibuja nada.

## 5. Archivos (`cs-io`)

- **`.chimp`:** `LayerExtra` guarda `objects` (cada uno con `id`, `name` y `track`). Al leer, el
  campo viejo `instance` se migra igual que en `cs-core`. Los `.chimp` viejos cargan sin cambios.
- **`.aseprite` suelto y export:** se hornea con `bake_instances`, como hoy.

## 6. MCP (`cs-agent`)

- `add_instance_layer` se queda y crea una capa de objetos con un objeto.
- **Nuevos:**
  - `add_object {doc, artboard?, layer, source, name?, frame?, x?, y?}`;
  - `set_object_key {…, layer, object, frame, x?, y?, flip_h?, flip_v?, opacity?, timing…}`;
  - `remove_object`;
  - `reorder_object {…, to: forward|backward|front|back}`.
- `set_instance_key` y `remove_instance_key` siguen funcionando sobre el primer objeto de la capa,
  por compatibilidad.
- `describe` lista `objects` (id, nombre y keys) en cada capa de objetos.

## 7. Pruebas

**`cs-core`**
- Un JSON viejo con `kind: "Instance"` e `instance: {...}` carga como `Objects` con un objeto.
- `instance_uses` cuenta objetos.
- `would_cycle` ve las fuentes de todos los objetos.

**`cs-render`**
- Dos objetos en la misma capa: se componen en orden y el de arriba tapa al de abajo.
- Opacidad de key y de capa.
- Cada objeto con su propio `source_frame`.

**`cs-commands`**
- `add_object`, `remove_objects`, `move_objects` (crea key), `reorder_objects` (los cuatro) y
  `duplicate_objects`, con undo.
- Insertar o borrar frames desplaza las keys de todos los objetos.

**`cs-io`**
- Round-trip de `.chimp` con dos objetos.
- Un `.chimp` viejo con `instance` carga migrado.

**`cs-ui`**
- Clic con Move selecciona el objeto de más arriba de otra capa y activa la capa.
- Shift+clic suma.
- Arrastrar mueve con un solo undo.
- Delete borra y Alt+arrastrar duplica.
- Bring Forward y Send Backward.
- El timeline se despliega y muestra una sub-fila por objeto.
- Soltar desde la Library en una capa de objetos agrega ahí.
- El doble clic entra al símbolo del objeto bajo el cursor.

**`cs-agent`**
- `add_object` + `describe` + `render`.
