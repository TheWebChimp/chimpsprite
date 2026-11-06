# Edición de tilemaps 2B: píxeles sobre tilemaps

Fecha: 2026-11-06 · Estado: decidido por el agente (ver "Decisiones que el usuario puede revisar")

## Contexto

Tiles se divide en cuatro subproyectos; el segundo, en dos:

1. Núcleo de tilemaps (PR #15, `2026-10-19-tilemaps-core-design.md`).
2. Edición de tilemaps:
   - 2A, modo Tiles (PR #16, `2026-10-21-tilemaps-edit-tiles-mode-design.md`);
   - **2B, píxeles sobre tilemaps (este documento).**
3. Exportar a Tiled.
4. Vista en mosaico.

Estado de partida: una capa tilemap activa siempre se edita en modo Tiles (se pinta con tiles). El
arte de los tiles solo se crea convirtiendo una capa de imagen o por MCP (`set_tile_pixels`). No
existe nada de 2B: ni el selector Pixels/Tiles, ni los modos Manual/Auto/Stack, ni reordenar.

## Objetivo

Como en Aseprite 1.3: sobre una capa tilemap se puede dibujar **en píxeles** con las herramientas
normales, y el tileset se actualiza solo según el **modo de tileset**:

- **Manual:** editar píxeles cambia el tile que ya está en la celda; todas sus instancias cambian.
  No crea tiles (salvo que la capa no tenga cel en ese frame, ver abajo).
- **Auto:** crea tiles cuando hace falta, reutiliza los iguales y borra los que dejan de usarse. Un
  tile que se modifica y está usado en otro lado se duplica; si solo lo usa esa celda, se modifica.
- **Stack:** nunca toca los tiles existentes: cada celda modificada pasa a un tile nuevo al final
  (o a uno igual que ya exista).

Al buscar "uno igual" se consideran los flips X/Y/D que el tileset permite (`allowed_flips`). Además,
en el panel de tileset se pueden **reordenar** tiles arrastrándolos; los mapas se remapean.

## 1. Comportamiento de referencia (Aseprite 1.3.18)

El comportamiento se fijó corriendo Aseprite en modo batch: `crates/cs-commands/tests/fixtures/`
tiene `tile_pixels.json` (escenarios), `tile_pixels.lua` (los corre con `app.useTool`,
`tilemapMode = PIXELS` y cada `tilesetMode`) y `tile_pixels.expected` (lo que dejó Aseprite). El test
`tile_pixels_aseprite.rs` corre los mismos escenarios en ChimpSprite y compara texto contra texto.

Lo que mostró Aseprite:

1. **Orden:** las celdas que toca el trazo se procesan fila por fila (y, luego x), sin importar la
   dirección del trazo.
2. **Una celda sin cambios** (el trazo pintó lo que ya había) no hace nada, ni en Stack.
3. **Transparente:** si la celda queda completamente transparente, pasa al tile vacío (0) en Auto y
   Stack. En Manual el tile se vacía pero la celda lo sigue usando.
4. **Manual:** cada celda con tile escribe sus píxeles en ese tile, a través de los flips de la celda
   (pintar (1,1) sobre una celda con flip X cambia el píxel (2,1) del tile). Varias celdas con el mismo
   tile en un trazo suman sus cambios. Las celdas vacías y las de fuera de la grilla no hacen nada.
   No deduplica. Sin cel en el frame, se comporta como Stack (crea el cel y los tiles).
5. **Auto**, por celda, con el tileset tal como va quedando en el trazo:
   1. transparente → tile 0;
   2. si hay un tile igual (exacto o con un flip permitido) → ese tile con esos flips;
   3. si el tile viejo de la celda lo usa solo esa celda en todo el sprite → se modifica en el lugar,
      con la imagen tal como se ve, y la celda pierde sus flips;
   4. si no → tile nuevo al final.

   "Usos" cuenta las celdas de todas las grillas de todas las capas que usan el tileset, en todos
   los frames; los cels vinculados comparten grilla y cuentan una vez. La cuenta se actualiza celda
   a celda: en un trazo sobre dos instancias del mismo tile, la primera recibe un tile nuevo y la
   segunda (ya única) modifica el original.

   Al terminar se borran los tiles viejos de las celdas tocadas que quedaron sin usos (no otros
   tiles sin uso que ya hubiera) y se remapean todas las grillas que usan el tileset.
6. **Stack:** transparente → 0; igual a uno existente (con flips permitidos) → ese; si no, tile nuevo
   al final. Nunca modifica ni borra.
7. **Flips:** el orden de búsqueda es sin flip, X, Y, X+Y, D, D+X, D+Y, D+X+Y (los bits D/X/Y de
   `Tile`, aplicados D, luego X, luego Y como en `cs-core`). Con un tile simétrico gana el primero de
   esa lista que coincide (Aseprite eligió X antes que X+Y, Y antes que X+Y y X+Y antes que D+X+Y).
8. **La grilla crece** si el trazo cae fuera (Auto/Stack), corriendo la posición del cel hacia la
   izquierda o arriba. Sin cel, se crea uno con la grilla mínima que abarca las celdas.
9. **Cels vinculados:** se modifica la grilla compartida (los dos frames cambian), como al pintar
   píxeles sobre cels vinculados de imagen.

Transparencia: los píxeles con alfa 0 se normalizan a `(0,0,0,0)` antes de comparar (Aseprite hace
lo mismo con `preprocess_transparent_pixels`); en indexado, el índice transparente.

## 2. Motor (`cs-commands/src/tiles.rs`, `cs-core/src/tilemap.rs`)

`cs-core`:
- `Tile::flags()` y `Tile::with_index()`.
- `orient_tile(image, tile) -> Image`: la imagen como la dibuja una celda con esos flips;
  `unorient_tile(seen, tile) -> Image`, el inverso. Usan el mismo `tile_source` del render.

`cs-commands::tiles`:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TilesetMode { Manual, #[default] Auto, Stack }

pub enum TilePaintError { NotTilemap, Nothing, TooLarge }

/// Píxeles del lienzo sobre la capa tilemap: cambios de tiles + tileset según el modo, en un Batch.
pub fn draw_tilemap_pixels(sprite, layer, frame, pixels: &[(i32, i32, PixelValue)], mode)
    -> Result<Command, TilePaintError>;

/// Mueve el tile `from` a la posición `to` (1..len) y remapea todas las grillas del tileset.
pub fn move_tile(sprite, tileset: usize, from: u32, to: u32) -> Option<Command>;
```

`draw_tilemap_pixels` arma un `Batch` con primitivas existentes (sin comandos nuevos, así el undo y
la serialización ya funcionan): `SetTileset` (si cambió), `SetTilemap` por cada grilla que cambió
(la del cel y, si Auto borró tiles, las demás remapeadas) y `SetCel` si cambia la posición o se crea
el cel (también para los vinculados, que se mueven juntos). Ignora píxeles fuera del lienzo y de otro
modo de color. Errores: capa que no es tilemap o sin tileset (`NotTilemap`), nada cambia
(`Nothing`), la grilla pasaría `MAX_TILEMAP_SIDE`/`tilemap_fits` o el tileset `MAX_TILESET_TILES`
(`TooLarge`).

La búsqueda de iguales usa un `HashMap` de la clave de píxeles normalizada → índice (el menor), que
se mantiene al agregar o modificar tiles durante el trazo.

`move_tile` también mueve `tile_user_data`. El tile 0 no se mueve ni se puede soltar en 0.

## 3. Editor (`cs-ui`)

### Modo Pixels / Tiles

- `Editor::tilemap_pixels: bool` (no se guarda; arranca en Tiles para no cambiar 2A). Es global,
  como en Aseprite: vale para cualquier capa tilemap.
- `tiles_mode()` pasa a ser "capa tilemap usable **y** modo Tiles"; `tilemap_pixels_mode()` es lo
  mismo en modo Pixels. Cambiar de modo a mitad de un trazo lo corta (`paint_target`).
- `ToolOptions::tileset_mode: TilesetMode` (se guarda; por defecto Auto, como el
  `default_tileset_mode` de Aseprite).

### Las herramientas en modo Pixels

Las herramientas ven un **proxy de píxeles**: una copia del sprite donde la capa tilemap activa es una
capa de imagen con su cel de este frame dibujado (`draw_tile_grid`). Se cachea por artboard, capa,
frame y versión del documento. Todas las herramientas que pintan o seleccionan lo usan (lápiz, spray,
borrador, cuentagotas, cubeta, degradado, formas, texto, Contour, Blur, Jumble, selecciones); Move,
Slice y Artboard siguen sobre el sprite real (Move mueve el cel tilemap en píxeles, como Aseprite).

El `DrawPixels` que sale (con simetría, Tiled Mode, recorte a la selección e ink, que lee el proxy)
pasa por `tiles::draw_tilemap_pixels` con el modo elegido. Un trazo = un paso de undo con el nombre
de la herramienta. La vista previa del trazo usa la misma traducción: en Manual se ven cambiar todas
las instancias mientras se pinta. Delete con selección borra los píxeles seleccionados por el mismo
camino. Capa bloqueada: no hace nada. Si el resultado pasaría los límites, la barra de estado lo dice
(como en 2A).

Copiar, pegar, levantar y transformar siguen sin actuar sobre capas tilemap (ya lo impide
`clip_layer_ok`).

### Barra de opciones

Con una capa tilemap activa, al principio de la barra:
- **Pixels | Tiles**: grupo de botones del kit con LED (`bar_fields::options`).
- En Pixels, **Tileset: Manual | Auto | Stack**, el mismo grupo, con los tooltips de Aseprite.

Sin Pixel UI, radios. Atajos como Aseprite: **Space+Tab** alterna Pixels/Tiles y **Space+1/2/3**
eligen Manual/Auto/Stack (con una capa tilemap activa).

### Panel derecho

En modo Pixels se ve la paleta (para elegir colores), como en Aseprite; en Tiles, el panel Tileset.

### Reordenar tiles

En el panel Tileset, arrastrar una miniatura y soltarla sobre otra mueve el tile a esa posición
(`move_tile`, un paso de undo). Mientras se arrastra se marca el destino. El tile del pincel sigue al
tile que tenía (si se movió él u otro, se actualiza su índice).

### Barra de estado

En modo Pixels sobre una capa tilemap: "Pixels mode (Auto): drawing creates and reuses tiles" (y su
variante para Manual y Stack).

## 4. MCP (`cs-agent` + `apps/chimpsprite-mcp`)

- `draw_tilemap_pixels { doc, artboard?, layer, frame?, mode?, pixels: [{x, y, color}] }`: píxeles
  del lienzo sobre una capa tilemap; `mode` = `manual` | `auto` (por defecto) | `stack`. Devuelve
  cuántos tiles tiene el tileset.
- `move_tile { doc, artboard?, tileset, from, to }`.

## 5. Tests

- `cs-core`: `orient_tile`/`unorient_tile` de ida y vuelta con los 8 flips; coinciden con el render.
- `cs-commands`: los escenarios de Aseprite (`tile_pixels_aseprite.rs`); unitarios de undo exacto,
  indexado, límites, píxeles fuera del lienzo, capa que no es tilemap, user data al borrar y mover,
  `move_tile` (remapeo con flips, otras capas, undo, bordes).
- `cs-ui`: el lápiz en modo Pixels en los tres modos con un paso de undo; la vista previa; el
  cuentagotas lee el proxy; Delete; el toggle corta el trazo; Move sigue moviendo el cel; el
  reordenar desde el editor mueve el pincel; la barra de opciones muestra los grupos.
- `cs-agent`: `draw_tilemap_pixels` y `move_tile`, con errores claros.
- Captura de la app con una capa tilemap en modo Pixels.

## Decisiones que el usuario puede revisar

1. **El selector Manual/Auto/Stack va en la barra de opciones** (lo pidió así el encargo). En Aseprite
   está arriba de la barra de colores, junto al botón Pixels/Tiles; aquí los dos van juntos en la barra
   de opciones, como grupos de texto del kit con LED (sin íconos nuevos en el skin).
2. **El modo arranca en Tiles** cada sesión (no se guarda), para que una capa tilemap se siga usando
   como en 2A; el modo de tileset sí se guarda (Auto por defecto, como Aseprite).
3. **Cels vinculados:** en modo Pixels se edita la grilla compartida (igual que Aseprite y que pintar
   píxeles). En modo Tiles (2A) `set_tiles` sigue desvinculando el cel que cambia.
4. **Píxeles fuera del lienzo se ignoran** (no hacen crecer la grilla hacia afuera).
5. **Desempate de flips** en el orden sin flip, X, Y, XY, D, DX, DY, DXY; coincide con lo que se
   probó en Aseprite, pero no se probaron todos los casos simétricos posibles.
6. **Manual sin cel** crea el cel y los tiles, como hizo Aseprite en la prueba.
7. **Reordenar** mueve un tile a la vez (Aseprite permite elegir varios y copiar con Ctrl): queda
   para después.
8. **Move en modo Pixels** mueve el cel tilemap en píxeles (puede quedar desalineado de la grilla del
   lienzo, como en Aseprite); levantar/transformar una selección sobre un tilemap no está.
