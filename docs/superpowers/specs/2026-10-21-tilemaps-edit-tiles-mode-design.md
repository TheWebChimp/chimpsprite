# Edición de tilemaps 2A: modo Tiles

Fecha: 2026-10-21 · Estado: aprobado

## Contexto

Tiles se divide en cuatro subproyectos, y el segundo se parte en dos:

1. Núcleo de tilemaps. Mergeado en el PR #15; spec en `2026-10-19-tilemaps-core-design.md`.
2. Edición de tilemaps:
   - **2A, modo Tiles: armar niveles con tiles (este documento).**
   - 2B, píxeles sobre tilemaps: pintar el arte de los tiles con los modos Manual, Auto y Stack,
     deduplicar con flips, reordenar, copiar y borrar tiles, y el selector Píxeles/Tiles.
3. Exportar a Tiled.
4. Vista en mosaico.

Estado de partida, tras el núcleo:
- las capas tilemap se ven;
- se editan solo por MCP;
- en la GUI, las herramientas y Move no hacen nada sobre ellas, y la barra de estado lo avisa.

## Objetivo

Armar niveles desde la GUI:
- crear capas tilemap y tilesets;
- elegir un tile y pintar, borrar, rellenar, tomar y seleccionar con tiles;
- convertir una capa de imagen en tileset + tilemap, que es la forma de crear tiles en 2A;
- convertir una capa tilemap de vuelta a imagen.

## 1. Modo Tiles y herramientas en espacio de celdas

En 2A, una capa tilemap activa **siempre** se edita en modo Tiles. El selector llega en 2B.

### El proxy de celdas (`cs-ui/src/tile_ops.rs`)

Con una capa tilemap activa, el editor no le pasa a las herramientas el sprite real, sino un
**proxy**: un `Sprite` RGBA donde cada píxel es una celda.

- **Celdas del lienzo:** la celda `(cx, cy)` empieza en el píxel `(cx·tw, cy·th)`, igual que en
  `ops::set_tiles`.
- **Tamaño del proxy:** `ceil(ancho / tw) × ceil(alto / th)`. El proxy cubre todo el lienzo, aunque
  el cel no lo cubra.
- **Contenido:** una sola capa de imagen con el frame actual. Cada píxel guarda el `Tile` de esa
  celda, o el vacío si cae fuera de la grilla.
- **Codificación:** `Rgba::from(tile.0.to_le_bytes())`, así que el tile 0 es `(0,0,0,0)`, es decir
  transparente. Hay funciones `encode(Tile) -> Rgba` y `decode(Rgba) -> Tile`.
- Si el cel no está alineado, la celda se calcula con la misma corrección que usa `ops::set_tiles`.

El `ToolContext` se arma sobre el proxy:

- `color` y `secondary` son el tile del pincel, codificado y con sus flips.
- `brush_size` se cuenta en celdas.
- `selection` es la selección del editor convertida a celdas: una celda está seleccionada si su
  píxel de origen lo está.

El puntero se convierte de píxeles del lienzo a celdas antes de llegar a la herramienta.

### Traducir el resultado

`ToolOutput.command` se traduce del proxy al sprite real:

- `DrawPixels { pixels }` pasa a `ops::set_tiles` con `(cx, cy, decode(color))`. El borrador escribe
  transparente, que es el tile vacío.
- Si `set_tiles` devuelve `None` (no hubo cambios, o la grilla excedería el límite), no se ejecuta
  nada. Si fue por el límite, la barra de estado lo dice.
- `ToolOutput.selection` (marquee y varita) se convierte de celdas a una máscara de píxeles: cada
  celda seleccionada cubre su rectángulo de `tw × th`.
- La vista previa del trazo pasa por la misma traducción, así que se ve el tile mientras se pinta.

### Herramientas en modo Tiles

| Herramienta | Comportamiento |
|---|---|
| Pencil / Eraser | pone el tile del pincel o el vacío; tamaño de pincel en celdas |
| Line, Rectangle, Ellipse | formas exactas en celdas; con contorno o rellenas según la opción |
| Bucket | rellena las celdas con el mismo tile, flips incluidos; contigua o no según la opción |
| Eyedropper | toma el tile y sus flips de la celda; un clic sobre el vacío elige el vacío |
| Rect/Ellipse Marquee, Magic Wand | seleccionan celdas completas; la varita compara tiles exactos |
| Move | mueve el cel en pasos de un tile: el delta en píxeles se redondea al múltiplo de `tw`/`th` |

Spray, Jumble, Gradient, Text, Lasso, Polygon Lasso y la simetría no actúan en modo Tiles, y la barra
de estado dice "Not available in Tiles mode". Pixel-perfect se ignora.

El aviso del núcleo, "tile editing comes in a later version", se reemplaza por uno nuevo:
"Tiles mode: painting with tile N", con N en la numeración del usuario, `índice + base_index - 1`.

### Grilla en el canvas

Con una capa tilemap activa, el canvas dibuja la grilla del tamaño de tile de su tileset, alineada al
lienzo. Es independiente de la grilla de trabajo de `View`.

## 2. Panel de tileset (`cs-ui/src/panels.rs`)

Cuando la capa activa es tilemap, el panel derecho muestra "Tileset" en lugar de la paleta. El
selector de color sigue debajo, porque se usa en 2B.

- **Encabezado:** el nombre del tileset, su tamaño de tile y la cantidad de tiles.
- **Grilla de tiles:**
  - cada miniatura se escala a un tamaño fijo, por ejemplo 32 px, con vecino más cercano;
  - el vacío va primero, con el dibujo de "transparente";
  - cada tile muestra su número en la numeración del usuario, y el vacío muestra "0";
  - un clic elige el tile del pincel, y el elegido lleva un borde;
  - las miniaturas se cachean por tileset y se invalidan con la revisión del documento.
- **Flips del pincel:** toggles X, Y y D. D solo está habilitado si el tile es cuadrado.
  - Atajos: `X` y `Y` mientras haya una capa tilemap activa.
  - `X` normalmente intercambia los colores, pero en modo Tiles no hay color secundario que
    intercambiar, así que se reutiliza.
  - D va solo por toggle.
- El tile del pincel se guarda en el editor (`brush_tile: Tile`) y se reinicia a 1 si el tileset
  cambia o el índice queda fuera de rango.

## 3. Nueva capa tilemap (`cs-ui`)

`Layer → New Tilemap Layer…` y un botón "+ Tilemap" en el timeline abren un diálogo modal:

- **Tileset:** uno existente del sprite, o "New tileset".
- **Si es nuevo:**
  - nombre (por defecto "Tileset N");
  - ancho y alto del tile, de 1 a 256 (por defecto el tamaño de la grilla de trabajo del sprite, o
    16×16);
  - base index (por defecto 1).
- **OK** ejecuta un solo `Batch`: `SetTileset` (si el tileset es nuevo) más
  `ops::new_tilemap_layer` sobre la capa actual. La capa nueva queda seleccionada.

Funciona con cualquier modo de color del sprite: los tiles se crean en el mismo modo.

## 4. Convertir capas (`cs-commands/src/ops.rs` + menú)

### `Layer → Convert to Tilemap`

Solo aplica a una capa de imagen. Pide el tamaño de tile en un diálogo, con los mismos valores por
defecto que la sección 3. Luego hace `ops::convert_to_tilemap(sprite, layer, tile_size) -> Option<Command>`:

1. Para cada frame con cel, corta el contenido en celdas alineadas a la grilla del lienzo (celda
   `(cx, cy)` = píxeles `[cx·tw, (cx+1)·tw)`). Solo se consideran las celdas que tocan la imagen del
   cel.
2. Toma cada celda como una imagen de `tw × th` en el modo del sprite.
   - Si es completamente transparente, es el tile vacío.
   - Si no, se busca un tile idéntico, por igualdad exacta de píxeles, sin flips. Si no hay, se
     agrega uno al tileset.
3. Cada cel pasa a un cel tilemap:
   - `position` es la esquina de la primera celda;
   - se conserva la `opacity` del cel;
   - los cels vinculados siguen vinculados, porque comparten la grilla.
4. La capa se reemplaza por una capa tilemap con el mismo `id`, nombre, visibilidad, bloqueo,
   opacidad, blend, padre, máscara y clip. El tileset es nuevo y se llama como la capa.
5. Todo es un solo `Batch` y un solo paso de undo:
   - `SetTileset` agrega el tileset;
   - los `SetTilemap` y `SetCel` arman los cels;
   - después se cambia el tipo de capa.

Para cambiar el tipo de capa, el plan evalúa una primitiva `SetLayerKind { id, kind, tileset }` o
quitar y volver a agregar la capa en el mismo índice. Elige la opción que deje el undo exacto, con los
cels, sin duplicar lógica.

Si alguna celda tiene un tamaño de tile que excedería los límites del núcleo, la conversión devuelve
`None` y la UI lo avisa. Lo mismo si hay más de `MAX_TILESET_TILES` tiles distintos.

### `Layer → Convert to Image Layer`

Solo aplica a una capa tilemap. Hace `ops::convert_to_image(sprite, layer) -> Option<Command>`:
- cada cel tilemap se reemplaza por su imagen, la que ya arma `materialize` sin recortar al lienzo,
  para no perder contenido;
- la capa pasa a imagen, conservando las mismas propiedades;
- es un solo `Batch`;
- el tileset queda en el sprite; borrarlo es opcional y no forma parte de esta operación.

## 5. Vista

- `View → Show Tile Numbers` es un toggle que se guarda en las opciones de vista.
- Con una capa tilemap activa, el canvas escribe el número de cada celda no vacía en la numeración
  del usuario, con texto chico y contorno.
- Solo se dibuja si el zoom deja al menos 16 px de pantalla por tile.

## 6. MCP (`cs-agent` + `apps/chimpsprite-mcp`)

`convert_layer { doc, artboard?, layer, to: "tilemap" | "image", tile_width?, tile_height? }`.
- El tamaño de tile es obligatorio si `to = "tilemap"`.
- Devuelve la cantidad de tiles del tileset creado.

## 7. Tests

**`cs-ui`**
- **Proxy:**
  - `encode`/`decode` de valores con flips, de ida y vuelta;
  - un proxy de un tilemap con un cel no alineado;
  - el tamaño del proxy con lienzos que no son múltiplo del tile.
- **Editor en modo Tiles:**
  - el lápiz pone el tile del pincel con sus flips;
  - la línea y el rectángulo con relleno quedan exactos en celdas;
  - la cubeta contigua rellena solo los tiles iguales y conectados, y la no contigua todos los
    iguales;
  - el cuentagotas toma el tile y sus flips, y actualiza `brush_tile`;
  - el borrador deja el tile vacío;
  - el marquee selecciona celdas completas;
  - Move avanza en pasos de un tile;
  - spray y texto no producen comandos.
- **Diálogo de nueva capa:** con tileset existente y con tileset nuevo; un solo undo.

**`cs-commands`**
- **`convert_to_tilemap`:**
  - con dos celdas iguales, el tileset queda con 1 tile más el vacío;
  - el render es idéntico antes y después, con `materialize`;
  - los cels vinculados siguen vinculados;
  - un cel no alineado funciona;
  - el undo restaura exacto;
  - se conservan las propiedades de la capa.
- **`convert_to_image`:** el render es idéntico, con un cel parcialmente fuera del lienzo; el undo
  restaura exacto.

**`cs-agent`**
- `convert_layer` en ambas direcciones, con errores claros: tamaño de tile faltante, tipo de capa
  equivocado.

**Verificación visual**
- Capturas de la app con el panel de tileset, la grilla de tiles y los números.
- Antes de las capturas se respalda `app.ron` y se compila con `CARGO_TARGET_DIR=target/gui-check`.

## Fuera de alcance (2B o después)

- Pintar píxeles sobre tilemaps y los modos Manual/Auto/Stack.
- El selector Píxeles/Tiles.
- Reordenar, copiar, mover y borrar tiles.
- Importar un tileset desde un PNG.
- Deduplicar con flips al convertir.
- Simetría en modo Tiles.
- Lazos en modo Tiles.
- Tile BG / clic derecho para el tile secundario.
