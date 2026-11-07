# Artboards: reproducción en el lugar y spritesheets

Fecha: 2026-11-06 · Estado: decidido por el agente (ver "Decisiones que el usuario puede revisar")

## Contexto

Un documento `.chimp` tiene varios artboards en un canvas infinito; cada uno es un sprite completo
(capas, frames, tags). Hoy solo se anima el artboard activo (Play del timeline, `Editor::toggle_play`)
y el Preview. Los demás muestran su frame recordado (`Editor::frame_of`). Para hojas de sprites
existe File → Import Sprite Sheet… (`sheet_dialog.rs`, `cs_io::import::{SheetGrid,
sprite_from_sheet}`), que abre una pestaña nueva, y File → Export Sprite Sheet (a archivo).

Dos ideas del usuario:

- **(a)** Dejar uno o varios artboards animándose en el canvas mientras se edita otro.
- **(b)** Convertir un artboard animado en una hoja (otro artboard) y la inversa: partir un artboard
  que es una hoja en frames.

## (a) Reproducción en el lugar ("Play in Place")

### Comportamiento

- Cada ventana de artboard (Pixel UI) lleva en la barra de título un ▶ entre el título y el ○
  (`icon.title_play`); encendido se ve como la pausa ‖ (`icon.title_pause`). Clic lo prende o lo apaga **sin
  activar el artboard** (se sigue editando el activo). También está en el menú del artboard
  ("Play in Place", casilla). Sin Pixel UI, solo desde el menú.
- Solo se ofrece en artboards normales con más de un frame (no en fuentes ni modelos 3D, que tienen
  su propia reproducción).
- Un artboard con Play in Place que **no es el activo** se anima en el canvas: respeta la duración de
  cada frame y la dirección del tag (Forward, Reverse, Ping-pong, Ping-pong reverse) con
  `cs_core::Playback`, igual que el Play del timeline. Rango: el tag que contiene el frame que tenía
  el artboard al prender; si no hay tag, todos los frames. Siempre en loop: el `repeat` del tag se
  ignora (es para dejarlo "vivo" de fondo).
- **El activo** sigue con su timeline normal: muestra su frame de edición (o el Play del timeline).
  Su Play in Place queda prendido pero en pausa; al activar otro artboard vuelve a animarse desde
  donde iba.
- El frame "de edición" recordado de cada artboard (`views`) no cambia: al activar un artboard que
  se estaba animando, se edita el frame en el que se dejó.
- Si el artboard se borra, la reproducción se descarta; si cambian sus frames o tags (otro
  artboard, undo), el rango se recalcula al vuelo para no salirse.

### Estado

- Vive en el `Editor` (por pestaña): `playing_in_place: BTreeMap<ArtboardId, PlayInPlace>` con el `Playback`,
  el rango, la dirección y el reloj. **No** es parte del documento: no crea pasos de undo, no marca
  el documento como modificado y no se guarda en el `.chimp`. Se recuerda mientras la pestaña está
  abierta (incluido cambiar de pestaña y volver).
- El tiempo sale de `InputState::time` (no de `stable_dt`, que en reposo es el dt estimado); el
  editor avanza con `tick_play_in_place(dt)`, que devuelve cuánto falta para el próximo cambio. La app
  pide repintar justo para ese momento (`request_repaint_after`), así el canvas no se repinta a
  60 fps sin necesidad.
- El canvas recompone la textura de un artboard cuando cambia su frame mostrado aunque la revisión
  del documento no cambie (`texture_frames`), y solo la de ese artboard.

## (b) Spritesheets

### Menú del artboard

El menú "≡" del artboard (Pixel UI) se abre también con **clic derecho en la barra de título** (o en
la etiqueta del nombre sin Pixel UI) y con clic derecho en cualquier parte del artboard con la
herramienta Artboard. Se agregan, tras un separador:

- Play in Place (casilla)
- Expand to Spritesheet…
- Slice into Frames…

Los tres van deshabilitados en fuentes y modelos; Play in Place y Expand piden más de un frame.

### Expand to Spritesheet…

Diálogo (pixel art, componentes del kit vía `widgets`):

- **Frames:** All / un tag (desplegable) / rango From–To.
- **Columns / Rows:** con "Auto" (por defecto) columnas = `ceil(sqrt(n))` y filas = `ceil(n/cols)`.
  Sin Auto se elige Columns o Rows y el otro se calcula para que entren los `n` frames.
- **Order:** By Rows (izquierda a derecha, luego abajo) o By Columns (arriba a abajo, luego a la
  derecha).
- **Spacing** (entre celdas) y **Border** (margen alrededor), en píxeles.
- Vista previa achicada con el borde de cada celda y el tamaño resultante.

Crea un artboard nuevo "`<nombre>` Sheet" a la derecha de todo (un paso de undo,
`add_artboard_with_sprite`) y lo activa. Una sola capa con todos los frames **aplanados** (lo que se
ve: capas visibles, instancias resueltas con `cs_render::materialize`, sin capas de referencia),
como el Export Sprite Sheet. La paleta se copia. Si el sprite era indexado y todos los colores de la
hoja están en la paleta, la hoja queda indexada; si no, RGBA. La grilla del artboard nuevo se pone
del tamaño de la celda (con su borde) para que se vean las celdas, solo si no hay spacing.

### Slice into Frames…

Reutiliza el diálogo de Import Sprite Sheet (`SheetDialog`) con otro título y botón ("Slice"):

- Fuente: el frame mostrado del artboard, aplanado (lo mismo que se ve).
- Dos formas de partir: **Cell Size** (como el import: tamaño, padding, offset) o **Columns × Rows**
  (calcula la celda: `(ancho − offset − padding·(cols−1)) / cols`).
- **Order** By Rows / By Columns (el import también lo gana).
- Duración de cada frame (ms), 100 por defecto.
- La sugerencia inicial: si el artboard tiene grilla distinta de la de por defecto y entra justa,
  esa celda; si no, la heurística del import (tira de cuadrados).

Crea un artboard nuevo "`<nombre>` Frames" del tamaño de una celda, con un frame por celda (las
celdas vacías del final no se agregan), la paleta del original y lo activa (un paso de undo).

### Motor (cs-io, sin egui, wasm)

Módulo nuevo `cs_io::spritesheet`:

- `SheetOrder { ByRows, ByColumns }`.
- `ExpandSettings { columns, rows, spacing, border, order }` y `auto_columns(n)`,
  `fit(n, fixed)`.
- `cell_position(i, cols, rows, order) -> (col, row)` y `sheet_size(...)`.
- `expand_frames(sprite, frames: &[usize], settings) -> Sprite` (el sprite ya materializado).
- `SheetGrid::from_counts(size, cols, rows, padding, offset)` y
  `sprite_from_sheet_ordered(sheet, grid, order, duration)`; `sprite_from_sheet` sigue igual
  (por filas, 100 ms).

## Pruebas

- Motor: auto columnas (1→1×1, 5→3×2, 9→3×3, 10→4×3), orden por filas/columnas, spacing/border en
  el tamaño y las posiciones, rango y tag, indexado conservado o RGBA, ida y vuelta Expand → Slice
  da los mismos píxeles y duraciones.
- Editor: tick en el lugar con duraciones y ping-pong; el activo no cambia de frame ni de `views`;
  no hay undo ni revisión del documento; borrar el artboard limpia; rango recalculado si se quitan
  frames; tiempo hasta el próximo cambio.
- Canvas: el ▶ de la barra prende sin activar; clic derecho en la barra abre el menú; la textura del
  artboard animado se recompone al cambiar de frame.
- Diálogos: Expand crea el artboard y se deshace con un Ctrl+Z; Slice crea el artboard animado.

## Decisiones que el usuario puede revisar

1. **Capas aplanadas** en Expand (como Export Sprite Sheet y la hoja de un motor). "Conservar capas"
   queda para después (anotado en minors): con grupos, máscaras, tilemaps e instancias por frame no
   hay una traducción directa.
2. **Loop siempre** en Play in Place: se ignora el `repeat` del tag.
3. **El activo no se anima en el lugar**: muestra su frame de edición; su ▶ queda prendido en pausa
   y se reanuda al activar otro.
4. **No se guarda en el `.chimp`** ni en el autosave: es estado de la sesión (de la pestaña). No hay
   undo ni "modificado".
5. **El ▶ va a la izquierda del ○** y no activa el artboard. Íconos provisionales (5×5) en el skin;
   el usuario puede redibujarlos.
6. El clic derecho sobre los píxeles sigue siendo del color secundario de las herramientas: el menú
   sale con clic derecho en la barra (o en cualquier parte con la herramienta Artboard).
7. Slice parte **el frame mostrado** del artboard (si ya era animado, solo ese frame).
8. Los artboards nuevos se llaman "`<nombre>` Sheet" / "`<nombre>` Frames" y van a la derecha de
   todo, como Duplicate.
