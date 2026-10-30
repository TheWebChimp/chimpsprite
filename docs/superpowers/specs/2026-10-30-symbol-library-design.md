# Movie clips, parte 1: Convertir en símbolo (F8) y biblioteca

Fecha: 2026-10-30 · Estado: aprobado e implementado

## Objetivo

Que los símbolos (PR #14) se sientan como los movie clips de Flash. Hoy cualquier artboard visible
se puede instanciar con su propia línea de tiempo. Faltan cuatro piezas, en este orden:

1. **F8 + biblioteca** (este documento).
2. Editar en contexto.
3. Tweens y transformación.

Decisiones tomadas con el usuario:

- **F8 convierte capas o píxeles.** Con marquesina, se cortan los píxeles de la selección en las
  capas seleccionadas. Sin marquesina, se convierten las capas seleccionadas completas. En los dos
  casos queda una instancia en su lugar.
- **Se lleva toda la animación.** El símbolo tiene todos los frames del host y la instancia
  sincronizada, así que se ve igual que antes de convertir.
- **Biblioteca y lienzo conviven.** Los símbolos de F8 viven ocultos en la biblioteca. Los
  artboards visibles se siguen pudiendo instanciar. Un artboard pasa a la biblioteca con "Move to
  Library" y un símbolo sale al lienzo con "Place on Canvas".
- **Modo símbolo aislado.** Un símbolo de la biblioteca se edita en un modo donde el lienzo muestra
  solo ese símbolo, con una migaja de pan para salir (el "Edit Symbol" de Flash). Editar en contexto
  le agrega después el resto de la escena atenuado.
- **Modelo: el símbolo sigue siendo un artboard** con una marca `in_library` (enfoque A). Se
  reutilizan las instancias, el render, los ciclos, el `.chimp`, el MCP y el export.

Fuera de alcance:

- punto de registro o pivote (llega con los tweens);
- "New Symbol" vacío (Ctrl+F8);
- carpetas y búsqueda en la biblioteca;
- tilemaps dentro de un símbolo;
- editar en contexto y tweens (documentos propios).

## 1. Modelo (`cs-core`)

```rust
pub struct Artboard {
    // …campos existentes…
    /// En la biblioteca: no se dibuja en el lienzo ni sale en el panel de Artboards; solo se usa
    /// como fuente de instancias (y se edita en modo símbolo).
    #[serde(default)]
    pub in_library: bool,
}
```

En `Document`:

- `canvas_artboards()` itera los que tienen `in_library == false`.
- `library()` itera los que tienen `in_library == true`.
- `bounds()` y `artboard_at()` usan solo `canvas_artboards()`, así que "fit all", el clic y el
  arrastre ignoran la biblioteca.
- `instance_uses(id) -> usize`: cuántas capas de instancia, en todo el documento, tienen alguna key
  con `source == id`. Es el "×3" de la biblioteca y el aviso al borrar.
- `position` se conserva estando en la biblioteca. "Place on Canvas" lo devuelve ahí. Si ese lugar
  choca con otro artboard del lienzo, lo pone a la derecha de todos, como un artboard nuevo.

Los documentos y autosaves viejos cargan con `in_library = false` y no cambian.

## 2. Comandos (`cs-commands`)

Primitiva nueva, serializable y con undo:

```rust
DocCommand::SetArtboardInLibrary { id: ArtboardId, in_library: bool }
```

Operaciones de alto nivel en `ops`:

```rust
/// Lo que F8 convierte.
pub enum SymbolSource {
    /// Las capas completas.
    Layers(Vec<LayerId>),
    /// Los píxeles de la máscara en esas capas, en todos los frames.
    Pixels { layers: Vec<LayerId>, selection: Mask },
}

pub fn convert_to_symbol(
    doc: &Document,
    host: ArtboardId,
    source: &SymbolSource,
    name: &str,
    timing: SymbolTiming,
) -> Result<(DocCommand, ArtboardId, LayerId), ConvertError>;

/// El Type del diálogo; se traduce al `Timing` de la key con `first_frame: 0`.
pub enum SymbolTiming { Graphic, MovieClip }

pub fn move_to_library(doc: &Document, id: ArtboardId) -> DocCommand;
pub fn place_on_canvas(doc: &Document, id: ArtboardId) -> DocCommand;
```

`convert_to_symbol` devuelve un solo `DocCommand::Batch` (un Ctrl+Z lo deshace todo), el id del
símbolo nuevo y el de la capa de instancia nueva.

1. **El símbolo.** Es un sprite con el modo de color, la paleta y **todos los frames, con sus
   duraciones**, del host. Lleva copias de las capas convertidas y de sus padres (grupos) hasta la
   raíz, conservando la jerarquía, la opacidad, el blend, las máscaras, el clip, la visibilidad y el
   bloqueo. Con `Pixels`, cada cel conserva solo lo que cae dentro de la máscara.
2. **El recorte.** El rectángulo es la unión, en todos los frames, de los píxeles no transparentes
   de esas capas. Con `Pixels` se intersecta con la caja de la máscara. Si las capas convertidas son
   de instancia, se usa la caja de su render materializado. El sprite del símbolo mide ese
   rectángulo, y los cels y las máscaras de capa se desplazan para quedar en coordenadas del
   símbolo. Si el rectángulo queda vacío, se devuelve `ConvertError::Empty`.
3. **El artboard.** Se agrega con `AddArtboard`, `in_library: true`, el id `next_artboard_id()` y
   `position` a la derecha de los artboards del lienzo (para cuando salga al lienzo).
4. **El host.**
   - Con `Layers`: se borran las capas. Un grupo que quede vacío también se borra.
   - Con `Pixels`: se borran los píxeles de la máscara en esas capas, en todos los frames.
5. **La instancia.** Una capa `Instance`, con el nombre del símbolo, se agrega en el lugar de la
   capa convertida de más arriba (mismo padre, mismo índice). Lleva una key en el frame 0 con
   `source` igual al símbolo, `position` igual a la esquina del recorte, sin flip, opacidad 255 y
   `timing`:
   - `Graphic { first_frame: 0, play: Loop }`, o
   - `MovieClip { first_frame: 0 }`.

Con las mismas duraciones, los dos modos dan el mismo frame del símbolo en cada frame del host. Así,
**`render_frame(materialize(host))` es idéntico, pixel por pixel, antes y después de convertir**, en
todos los frames. Lo garantiza una prueba.

`ConvertError` tiene cuatro casos:

- `NotRgba`: el host no es RGBA. Las instancias ya lo exigen.
- `Tilemap`: hay una capa de tilemap entre lo convertido.
- `Empty`: no hay pintura que convertir.
- `NoLayers`: no hay capas.

Las capas de instancia sí se pueden convertir (símbolos anidados). Nunca forman un ciclo, porque el
símbolo es nuevo.

## 3. F8 y el diálogo (`cs-ui`)

- **Dónde está:**
  - acción `ConvertToSymbol`, con F8 en el keymap (configurable);
  - `Layer → Convert to Symbol…`;
  - el menú contextual del panel de capas;
  - el menú contextual del lienzo cuando hay marquesina.
- **Qué toma:**
  - con marquesina, `SymbolSource::Pixels` con las capas seleccionadas en el panel (o la activa);
  - sin marquesina, `SymbolSource::Layers` con las capas seleccionadas.
- **Si no se puede:** con un `ConvertError`, la acción queda deshabilitada en los menús y F8 deja el
  motivo en la barra de estado:
  - "Symbols need an RGBA sprite";
  - "Tilemap layers can't go into a symbol";
  - "Nothing to convert".
- **Diálogo "Convert to Symbol"**, con el estilo de los diálogos pixel que ya existen:
  - **Name:** el primer `Symbol N` libre. Si se convierte una sola capa, su nombre.
  - **Type:** Graphic (predeterminado) o Movie Clip.
  - **Botones:** OK / Cancel. Enter confirma y Esc cancela.
- **Al confirmar:**
  - se aplica el batch;
  - la instancia nueva queda como capa activa;
  - el símbolo queda seleccionado en la Biblioteca;
  - la marquesina se borra.

## 4. Panel de Biblioteca (`cs-ui`)

`DockPanel::Library`, junto a Artboards y Slices. El layout lo propone la implementación, con las
piezas del skin; el usuario lo afinará después con un mockup.

- **Vista previa** arriba: el símbolo seleccionado, centrado, al mayor zoom entero que quepa,
  reproduciendo su animación en loop con las duraciones de sus frames.
- **Lista:**
  - una fila por símbolo, ordenada por nombre;
  - cada fila con miniatura (frame 0, ajustada al alto de la fila), nombre y usos ("×3", o nada si
    no se usa);
  - la fila seleccionada, resaltada.
- **Sin símbolos:** "Select layers or pixels and press F8 to make a symbol."
- **Interacciones:**
  - **Clic:** selecciona.
  - **Doble clic:** entra al modo símbolo.
  - **F2:** renombrar.
  - **Arrastrar una fila al lienzo:** crea una capa de instancia en el artboard bajo el cursor,
    encima de la capa activa (si ese artboard es el activo; si no, arriba de todo), con una key en
    el frame actual y la esquina superior izquierda donde se suelta. Si el artboard no es RGBA o se
    formaría un ciclo, no se suelta y la barra de estado dice por qué.
  - **Menú contextual:**
    - Edit Symbol;
    - Rename;
    - Duplicate (copia en la biblioteca, "Name copy");
    - Place on Canvas;
    - Delete. Si el símbolo está en uso, primero confirma: "Used by N instances. Delete anyway?".
      Las instancias quedan sin fuente, igual que hoy al borrar un artboard instanciado, y Ctrl+Z
      lo restaura.
- **Panel de Artboards:** el menú contextual de un artboard gana **Move to Library**. Si es el
  artboard activo, el activo pasa al siguiente del lienzo. Si era el único del lienzo, la opción
  queda deshabilitada: el lienzo nunca queda vacío.
- **`New Instance`** (timeline): primero los símbolos de la biblioteca y, después de un
  separador, los artboards del lienzo. Se ocultan los que formarían un ciclo y los que no son RGBA,
  como hoy.

## 5. Modo símbolo (`cs-ui`)

- **Estado:** `Editor::symbol_stack: Vec<ArtboardId>`. Vacío es el modo normal. El último elemento
  es el símbolo que se está editando.
- **Para entrar:**
  - doble clic en la Biblioteca;
  - "Edit Symbol";
  - doble clic con Move sobre una instancia cuya fuente está en la biblioteca. Si la fuente está en
    el lienzo, se conserva el comportamiento de hoy: se selecciona y se encuadra.

  Al entrar se apila el símbolo, se activa y se encuadra.
- **Dentro del modo:**
  - el lienzo dibuja **solo** el artboard del símbolo, en su `position`, sin los demás artboards ni
    las notas;
  - todas las herramientas, el timeline, las capas y el playback funcionan normal sobre él;
  - un doble clic sobre una instancia anidada que también está en la biblioteca apila otro nivel.
- **Migaja de pan:** una barra delgada arriba del lienzo, con `Document › Symbol 1 › Symbol 2`, que
  se ve solo en el modo símbolo. Clic en un nivel vuelve a él. "Document" sale del modo y regresa al
  artboard y a la vista que había antes de entrar.
- **Para salir:** también con doble clic en un área vacía del lienzo, como en Flash.
- **Si el símbolo deja de existir** (Ctrl+Z o un script): la pila se recorta hasta el último nivel
  que todavía existe.
- El modo símbolo no se guarda en el archivo. Al abrir, siempre arranca en modo normal.

## 6. Archivos (`cs-io`)

- **`.chimp`:** el `Entry` del manifest gana
  `#[serde(default, skip_serializing_if = "is_false")] library: bool`. El `.aseprite` interno de
  cada símbolo se guarda igual que el de cualquier artboard. Los `.chimp` viejos cargan sin
  cambios.
- **Export de documento** (PNG/GIF de todos los artboards, sheets, Godot/Unity/Bevy por artboard):
  solo los artboards del lienzo. Los símbolos llegan horneados a través de sus instancias, con
  `materialize`/`bake_instances` como hoy.
- **`.aseprite` suelto:** sin cambios. Se hornean las instancias.

## 7. MCP (`cs-agent`)

- **`convert_to_symbol`** `{ document, artboard, layers, selection?: rect, name, timing }`. Devuelve
  el id del símbolo y el de la capa de instancia. Un rectángulo alcanza como selección para el
  agente.
- **`move_to_library` / `place_on_canvas`** `{ document, artboard }`.
- **`describe`:** cada artboard incluye `"in_library": true|false`.

## 8. Pruebas

**`cs-core`**
- `canvas_artboards`, `library`, `bounds` y `artboard_at` ignoran la biblioteca.
- `instance_uses` cuenta capas, no keys.
- Un `Document` sin el campo `in_library` deserializa con `false`.

**`cs-commands`**
- Convertir capas: el render materializado del host es idéntico en todos los frames, antes y
  después, con duraciones desiguales, en Graphic y en Movie Clip.
- Convertir píxeles con marquesina: lo de afuera queda en el host, lo de adentro pasa al símbolo, y
  el render es idéntico.
- El símbolo se recorta a la unión de la pintura de todos los frames.
- La jerarquía de grupos se conserva y un grupo que queda vacío en el host se borra.
- Convertir una capa de instancia hace un anidado que renderiza igual.
- Los cuatro casos de `ConvertError`.
- Un solo undo deja el documento exactamente como estaba, y el redo lo repite.
- `move_to_library` y `place_on_canvas`, con undo, y `place_on_canvas` cuando su lugar choca con
  otro artboard.

**`cs-io`**
- Round-trip de `.chimp` con un símbolo en la biblioteca y una instancia suya.
- Un `.chimp` viejo carga con todo en el lienzo.

**`cs-ui`**
- F8 sin marquesina convierte las capas seleccionadas. Con marquesina, convierte los píxeles.
- El diálogo propone el nombre y crea con el Type elegido.
- La biblioteca lista ordenado, con usos. El doble clic entra al modo símbolo.
- El lienzo en modo símbolo dibuja solo el símbolo. La migaja sale y restaura la vista.
- Arrastrar de la biblioteca al lienzo crea la instancia donde se suelta.
- Move to Library se deshabilita con el último artboard del lienzo.
- Borrar un símbolo en uso pide confirmación.

**`cs-agent`**
- `convert_to_symbol` + `render` da lo mismo que antes de convertir.

## Ajustes al spec (descubiertos al planear e implementar)

1. **"Capas seleccionadas" = `Editor::range_layers()`.** Son las capas del rango del timeline o,
   si no hay rango, la capa actual. El panel de capas no tiene multi-selección.
2. **Recorte de las capas de instancia.** Para una capa de instancia se usa el rectángulo completo
   del sprite fuente en cada key, porque `cs-commands` no puede depender de `cs-render`. El render
   sigue siendo idéntico.
3. **Con marquesina solo se cortan capas de imagen.**
4. **"Move to Library"** está en el menú "≡" del artboard (Pixel UI) y en `Sprite → Move to
   Library`.
5. **Mensajes de F8:** los errores se muestran con un mensaje modal ("Convert to Symbol").
6. **La biblioteca es un `app_panel` del dock** y su estado es por pestaña.
7. **Conversiones rechazadas.** Para garantizar el render idéntico, `ConvertError` también
   rechaza:
   - `Scattered`: capas de grupos distintos o con otras capas en medio;
   - `Locked`: capas bloqueadas, en el modo píxeles;
   - `Clipping`: un par de clipping que quedaría partido;
   - `BlendMode`: capas que dibujan con un modo de fusión distinto de Normal.

   El blend de los grupos se permite porque el render lo ignora.
8. **Modo símbolo:**
   - Activar cualquier artboard que no esté en la pila sale del modo.
   - El tacho del panel Artboards, la herramienta Artboard y el menú "≡" no borran, duplican ni
     mueven el símbolo que se está editando.
   - Salir con doble clic en vacío es solo con la herramienta Move.

