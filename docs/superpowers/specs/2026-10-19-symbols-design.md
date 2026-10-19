# Símbolos: instancias de artboards con línea de tiempo propia

Fecha: 2026-10-19 · Estado: aprobado

## Objetivo

Poder colocar un artboard dentro de otro como **instancia**, al estilo de los símbolos de Adobe
Animate: la instancia tiene su propia línea de tiempo (la del artboard fuente), se reutiliza en
muchos lugares y editar el artboard fuente actualiza todas sus instancias.

Decisiones tomadas con el usuario:

- **Artboard = símbolo.** No hay biblioteca oculta; cualquier artboard puede instanciarse.
- **Dos modos de tiempo**, como Animate: *Graphic* (sincronizado con los frames del padre) y
  *Movie clip* (corre por tiempo, independiente de los frames del padre).
- **Por keyframe:** fuente, posición, flip H/V, opacidad y tiempo. Sin interpolación: cada keyframe
  vale hasta el siguiente. Todo pixel-perfect.

Fuera de alcance: tweens, escala/rotación, "convertir selección en símbolo" (F8), biblioteca
oculta, instancias en artboards no RGBA.

## 1. Modelo de datos (`cs-core`)

Nuevo módulo `instance.rs`:

```rust
pub enum LayerKind { Image, Group, Instance }

pub struct Layer {
    // …campos existentes…
    #[serde(default)]
    pub instance: Option<InstanceTrack>, // Some solo si kind == Instance
}

pub struct InstanceTrack {
    /// Ordenadas por `frame`; cada una vale desde su frame hasta la siguiente.
    pub keys: Vec<InstanceKey>,
}

pub struct InstanceKey {
    pub frame: usize,
    /// `None` = la capa no muestra nada desde este frame.
    pub source: Option<ArtboardId>,
    /// Esquina superior izquierda del símbolo, en píxeles del sprite padre.
    pub position: (i32, i32),
    pub flip_h: bool,
    pub flip_v: bool,
    pub opacity: u8,
    pub timing: Timing,
}

pub enum Timing {
    Graphic { first_frame: usize, play: GraphicPlay },
    MovieClip { first_frame: usize },
}

pub enum GraphicPlay { Loop, Once, Single }
```

`InstanceTrack` ofrece `key_at(frame)`, `set_key(key)` y `remove_key(frame)`, con el mismo
patrón que `Slice`.

Las capas de instancia no tienen cels: `Sprite::cels` nunca contiene entradas para ellas. Sí
admiten `visible`, `locked`, `opacity`, `blend`, `mask`, `clip` y `parent`, igual que una capa de
imagen.

### Mapeo de frames (`instance::source_frame`)

```rust
pub fn source_frame(key: &InstanceKey, parent: &Sprite, frame: usize, source: &Sprite) -> usize
```

Sea `n = source.frames.len()`, `d = frame − key.frame` y `f0 = first_frame.min(n − 1)`.

- `Graphic::Loop` → `(f0 + d) % n`
- `Graphic::Once` → `(f0 + d).min(n − 1)`
- `Graphic::Single` → `f0`
- `MovieClip`:
  - `t` = suma de `duration_ms` de los frames del padre en `key.frame..frame`.
  - `T` = duración total del símbolo.
  - Se avanza desde `f0` sumando las duraciones del símbolo con `t % T`, en loop.
  - Si `T == 0`, devuelve `f0`.

Ambos modos son funciones puras de `(key, frame)`, así que la reproducción, el export y los tests
dan exactamente lo mismo.

### Ciclos

`Document::instance_sources(id)` da los artboards que `id` instancia directamente.
`Document::would_cycle(host, source)` es verdadero si `host == source` o si `host` es alcanzable
desde `source`. Lo usan:

- los comandos, que rechazan la operación;
- la carga del `.chimp`, que descarta la key culpable y deja un aviso en `LoadReport`.

`source` debe existir y ser RGBA. Si no, la key se ignora al render y se muestra un aviso en la UI.

## 2. Render (`cs-render`)

Las funciones existentes (`render_frame`, `composite_pixel`, `render_onion`, `bake_masks`) siguen
recibiendo un `&Sprite`. Las instancias se resuelven antes con **materialización**:

```rust
/// Copia del sprite donde cada capa de instancia pasa a ser una capa de imagen con un cel
/// en `frame` (o sin cel si no muestra nada). `Cow::Borrowed` si no hay instancias.
pub fn materialize(doc: &Document, sprite: &Sprite, frame: usize) -> Cow<'_, Sprite>;

/// Lo mismo para todos los frames (para exportar a formatos sin instancias).
pub fn bake_instances(doc: &Document, sprite: &Sprite) -> Option<Sprite>;
```

Cómo se arma el cel de una instancia en un frame:

1. Toma la key vigente con `key_at(frame)`.
2. Renderiza recursivamente el artboard fuente en `source_frame(...)` con
   `render_frame(&materialize(doc, source, sf), sf)`.
3. Aplica `flip_h` y `flip_v`.
4. Recorta el resultado al bounding box opaco.
5. Usa `position` ajustada por el recorte y `cel.opacity = key.opacity`.

La opacidad, el blend, las máscaras y el clip de la capa los resuelve el render existente sin
cambios. Por defensa ante datos corruptos, la recursión tiene un límite de profundidad de 16.
Pasado ese límite, la instancia no se dibuja.

Los llamadores que hoy renderizan un artboard (canvas, miniaturas, export, `cs-agent`) llaman a
`materialize` antes. Para las capas de instancia, `coverage` usa el sprite materializado.

## 3. Comandos (`cs-commands`)

Todos los comandos son serializables y tienen undo, como los existentes:

- `AddInstanceLayer { artboard, source, frame, position }` crea la capa sobre la capa actual con
  una key `Graphic { first_frame: 0, play: Loop }`, sin flip y con opacidad 255.
- `SetInstanceKey { artboard, layer, key }` inserta o reemplaza la key.
- `RemoveInstanceKey { artboard, layer, frame }` borra la key. No se permite borrar la única key;
  en ese caso se borra la capa.

Ajustes a operaciones existentes:

- **Insertar o borrar frames** desplaza `frame` de las keys, igual que los slices.
- **Borrar un artboard que otros instancian** deja esas keys con `source` colgante. La key no se
  dibuja y el panel la marca. El undo lo restaura todo.
- **Duplicar un artboard** conserva sus instancias.

## 4. UI (`cs-ui`, `cs-tools`)

- **`Capa → Nueva instancia…`** abre un selector de artboards. Se ocultan los que crearían un
  ciclo y los que no son RGBA. La capa se crea con una key en el frame actual, centrada en el
  artboard.
- **Panel de capas:** las capas de instancia llevan su propio ícono y el nombre del artboard
  fuente por defecto.
- **Timeline:** en las capas de instancia se dibuja un ◆ por key y una barra de "hold" hasta la
  siguiente. Clic derecho ofrece "Insertar keyframe" (copia la vigente) y "Borrar keyframe".
- **Panel de propiedades de instancia**, visible cuando la capa actual es de instancia. Edita la
  key vigente y, si el frame actual no es keyframe, primero crea una:
  - fuente;
  - modo (Graphic o Movie clip);
  - frame inicial;
  - Loop / Una vez / Fijo (solo en Graphic);
  - flip H y flip V;
  - opacidad.
- **Herramienta Move** sobre una capa de instancia arrastra la instancia y genera un
  `SetInstanceKey` al soltar, creando key si hace falta.
- **Herramientas de pintura** sobre una capa de instancia: el cursor queda deshabilitado y la
  barra de estado dice "Capa de instancia: edita el artboard fuente".
- **Doble clic** con Move sobre una instancia selecciona el artboard fuente y lo encuadra, como
  "entrar al símbolo". El playback del artboard fuente es independiente.

## 5. Archivos (`cs-io`)

- **`.chimp`:** `LayerExtra` gana `instance: Option<InstanceTrack>`.
  - Al guardar, el `.aseprite` interno contiene la capa horneada como capa de imagen normal, para
    que se vea bien si se abre en Aseprite.
  - Al cargar, la capa cuyo `LayerExtra` trae `instance` se convierte en `LayerKind::Instance`,
    sus cels horneados se descartan y se valida que no haya ciclos.
  - Los archivos `.chimp` existentes cargan sin cambios: `serde(default)`.
- **`.aseprite` suelto:** se guarda `bake_instances` sobre el sprite (antes de `bake_masks`).
  `files::bakes_instances(doc, name)` avisa en el diálogo de guardar, igual que con las máscaras.
- **PNG, GIF, sheets y Godot/Unity/Bevy:** pasan por `materialize`/`bake_instances` en el punto
  donde hoy obtienen el sprite. No necesitan más cambios.

## 6. MCP (`cs-agent`)

Nuevas operaciones, que delegan en los comandos:

- `add_instance_layer`
- `set_instance_key`
- `remove_instance_key`
- `list_instances`: capas de instancia y sus keys de un artboard.

`render`/`export` en `cs-agent` usan `materialize`.

## 7. Tests

**`cs-core`**
- `source_frame` en los cuatro modos: loop, una vez, fijo y movie clip con duraciones desiguales.
- Movie clip con `T == 0`.
- `key_at`, `set_key` y `remove_key`.
- `would_cycle` en tres casos: directo, indirecto (A→B→C→A) y sin ciclo.

**`cs-render`**
- Instancia simple en posición.
- Flip H y flip V.
- Opacidad de key × opacidad de capa.
- Anidado de dos niveles.
- Instancia bajo una máscara de capa.
- Fuente colgante: no dibuja.
- Límite de profundidad.

**`cs-commands`**
- Undo y redo de los tres comandos.
- Insertar y borrar frames desplaza las keys.
- Se rechaza crear un ciclo.

**`cs-io`**
- Round-trip `.chimp` con instancias en Graphic y Movie clip.
- El `.aseprite` interno trae cels horneados.
- Guardar como `.aseprite` hornea, y el resultado compara pixel a pixel contra
  `render_frame(materialize(...))`.
- Un `.chimp` con un ciclo carga con aviso.

**`cs-agent`**
- `add_instance_layer` + `render`.
