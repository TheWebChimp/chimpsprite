# Tilemaps 3: exportar a Tiled (`.tmx`/`.tmj` + `.tsx`/`.tsj`) e importar

## Contexto

Subproyecto 3 de tilemaps (ver `2026-10-19-tilemaps-core-design.md`): llevar las capas tilemap a
[Tiled Map Editor](https://www.mapeditor.org/). El modelo ya guarda cada celda con el layout del GID
de Tiled: bits 0–28 índice, bit 31 flip H (`Tile::FLIP_X`), bit 30 flip V (`FLIP_Y`), bit 29 flip
D (`FLIP_D`). El orden de las operaciones también coincide: Tiled aplica primero D y después H y V
([global-tile-ids](https://doc.mapeditor.org/en/stable/reference/global-tile-ids/)), igual que
`tile_source` en `cs-core/src/tilemap.rs` (verificado contra Aseprite en 2A). Exportar es copiar los
bits de flip y traducir el índice: el tile `k ≥ 1` de nuestro tileset es el id local `k − 1` en
Tiled (nuestro 0 es el vacío, que en Tiled es el GID 0), así que `gid = firstgid + k − 1`.

Referencias del formato: [TMX](https://doc.mapeditor.org/en/stable/reference/tmx-map-format/) y
[JSON](https://doc.mapeditor.org/en/stable/reference/json-map-format/). Tiled no está instalado:
los tests parsean lo generado (quick-xml / serde_json) y comprueban los campos del formato
documentado.

## Objetivo

- File → **Export Tiled Map…**: un diálogo con formato (TMX/TMJ), tileset externo o embebido,
  codificación de los datos, frame(s) y carpeta/nombre de salida.
- File → **Import Tiled Map…** (escritorio) y soltar un `.tmx`/`.tmj` sobre la ventana: abre el
  mapa como sprite con capas tilemap.
- MCP: `export` con `kind: "tiled"` (la extensión de `path` elige TMX o TMJ).

## 1. Modelo intermedio (`cs-io/src/tiled.rs`)

Un `Map` que se escribe a XML o JSON y que también sale de leerlos (el import y los tests usan el
mismo modelo):

- `Map { width, height, tile_width, tile_height, tilesets: Vec<MapTileset>, layers: Vec<MapLayer>,
  next_layer_id, next_object_id, properties }`.
- `MapTileset { first_gid, source: Option<String>, tileset: TiledTileset }` (con `source` es externo
  y en el mapa solo van `firstgid` + `source`).
- `TiledTileset { name, tile_width, tile_height, tile_count, columns, margin, spacing,
  image: Option<TiledImage>, transformations: Option<AllowedFlips> }`.
- `MapLayer { id, name, visible, opacity, offset, mode, properties, content }` con
  `content = Tiles { width, height, gids } | Image(Option<TiledImage>) | Objects(Vec<MapObject>) |
  Group(Vec<MapLayer>)`.
- `Property { name, value: Str | Int | Float | Bool | Color }`.

Escritura: `write_tmx`, `write_tsx`, `write_tmj`, `write_tsj` (con `Encoding`). Lectura:
`read_tmx`, `read_tsx`, `read_tmj`, `read_tsj`.

## 2. Exportar (`tiled::export`)

`export(sprite, stem, &TiledOptions) -> Result<TiledExport { files, warnings }>`; `files` son
`(nombre, bytes)` relativos a la carpeta de salida (como `ExportDialog::build`).

Preparación: `bake_layers(…, KeepTilemaps)` (las instancias pasan a píxeles, los tilemaps quedan),
después `bake_masks` (las máscaras y recortes se aplican; una tilemap con máscara pasa a imagen).
Las capas de referencia no salen.

- **Tamaño del mapa**: el lienzo en tiles, redondeando hacia arriba. **Tamaño de tile del mapa**: el
  del tileset de la primera capa tilemap (de abajo hacia arriba). Sin capas tilemap no se exporta
  (error claro; el diálogo lo avisa antes).
- **Tilesets**: uno por tileset del sprite que tenga al menos un tile además del vacío, en orden;
  `firstgid` = 1 y se acumula `tilecount`. Imagen `<tileset>.png` en grilla de
  `columns = ceil(√tilecount)` columnas, margin/spacing 0, RGBA (los indexados se resuelven con la
  paleta). `allowed_flips` → `<transformations hflip vflip rotate>` (D ↔ rotate). Externo: un
  `<tileset>.tsx` (TMX) o `<tileset>.tsj` (TMJ) y el mapa lo referencia con `source`. Nombres de
  archivo saneados y sin repetir.
- **Capas tilemap** → `<layer>` del tamaño del mapa. La grilla se ubica según la posición del cel:
  `floor(pos / tile)` en celdas y el resto en `offsetx/offsety`. Celdas fuera del mapa se descartan
  (aviso). Tile `k` → `firstgid + k − 1` con los mismos bits de flip; índices que el tileset no
  tiene (tilesets externos de Aseprite incompletos) → 0 con aviso. Sin cel en el frame: todo 0.
  Una tilemap cuyo tileset tiene otro tamaño de tile que el mapa se exporta como capa de imagen
  (aviso): Tiled dibuja todas las capas de tiles sobre la grilla del mapa.
- **Capas de imagen** → `<imagelayer>` con el PNG del cel del frame (recortado al lienzo, con la
  opacidad del cel ya aplicada) en `offsetx/offsety`. Archivo `<mapa>_<id>_<capa>.png`. Sin cel:
  `<imagelayer>` sin imagen (la estructura de capas es la misma en todos los frames).
- **Grupos** → `<group>` con sus hijos.
- **Comunes**: `id` = el `LayerId` (estable entre frames), `name`, `visible`, `opacity` = capa × cel,
  `mode` (blend: multiply, screen, overlay, darken, lighten, add, difference; Subtract no existe en
  Tiled → normal con aviso).
- **Slices** → `<objectgroup name="Slices">` arriba de todo, un rectángulo por slice vigente en el
  frame, con propiedades `text`, `color`, `pivot_x/pivot_y` y `center_x/y/w/h` si las tiene.
- **Propiedades del mapa**: `frame` (desde 1) y `duration_ms` del frame.
- **Frames**: `Current` → `<stem>.tmx`; `All` → un mapa por frame (`<stem>_001.tmx`…), con los
  mismos tilesets compartidos y PNGs de capa por frame (`<stem>_001_<id>_<capa>.png`).
- **Codificación de `<data>`**: CSV, Base64, Base64 + zlib o Base64 + gzip (u32 little-endian, como
  dice el formato). En TMJ, CSV es el arreglo de números.
- Cabecera: `version="1.10"`, `orientation="orthogonal"`, `renderorder="right-down"`,
  `infinite="0"`; sin `tiledversion` (no somos Tiled).

## 3. Importar (`tiled::import`)

`import(name, bytes, load: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Result<(Sprite, warnings)>`.
`load` recibe rutas relativas a la carpeta del mapa (las de un `.tsx` se resuelven relativas al
`.tsx`).

- Solo mapas ortogonales y finitos (los infinitos con chunks: error "not supported yet").
- Sprite RGBA de `width × tile_width` por `height × tile_height`, un frame.
- Cada tileset con imagen → un `Tileset` (tiles cortados con margin/spacing/columns; `trans` →
  transparente; `transformations` → `allowed_flips`). Colecciones de imágenes: aviso, sus tiles
  quedan vacíos.
- `<layer>` → capa tilemap con la grilla del mapa en `offset`. Si usa tiles de varios tilesets se
  parte en una capa por tileset (`nombre (tileset)`), porque nuestra capa tiene un solo tileset.
- `<imagelayer>` → capa de imagen; `<group>` → grupo; `<objectgroup>`: los rectángulos pasan a
  slices (con las mismas propiedades que el export), el resto se ignora con aviso.
- Codificaciones CSV, base64, zlib y gzip (zstd: error).

## 4. UI (`cs-ui/src/tiled_dialog.rs`)

Ventana "Export Tiled Map" (`pixel_ui::app_window`, componentes del kit vía `widgets`):

```
Format     (•) TMX (XML)   ( ) TMJ (JSON)
Tilesets   (•) External .tsx   ( ) Embedded
Layer data (•) CSV  ( ) Base64  ( ) Base64 + zlib  ( ) Base64 + gzip
Frames     (•) Current (3)  ( ) All (one map each)
Name       [hero        ]
Folder     [D:/game/maps] [Browse…]        (solo escritorio)
16×12 tiles of 16×16 · 2 tilesets · 3 layers
Image layers get their own PNGs; slices become objects.
                                   [Export] [Cancel]
```

- Folder empieza en la carpeta del documento (si está guardado) o la última usada; vacía → el
  diálogo de guardar del sistema. En web no hay carpeta: se descarga un `.zip` con todo.
- Sin capas tilemap, Export queda deshabilitado con el aviso "Needs a tilemap layer (Layer →
  Convert to Tilemap)".
- Al terminar, el mensaje "Exported" con la lista de archivos y los avisos.

File → Import Tiled Map… (escritorio) abre `.tmx`/`.tmj` en una pestaña nueva sin guardar; soltar
uno también. En web el import avisa que necesita los archivos vecinos.

## 5. Tests

- `cs-io`: tiles con los 8 flips → GIDs esperados; dos tilesets con `firstgid` acumulado; mapa de
  tamaño redondeado; offset de cel no alineado; TMX y TMJ parseados (quick-xml / serde_json) con
  `version`, dimensiones, `tilecount`, `columns`, `source`, datos CSV/base64/zlib/gzip decodificados;
  grupos, imagelayer, slices, opacidad, visibilidad, blend; un mapa por frame; round-trip
  export → import (GIDs, flips, píxeles de los tiles, grupos).
- `cs-ui`: nombres de archivo del diálogo, zip en web, deshabilitado sin tilemaps; captura.

## Decisiones que el usuario puede revisar

1. **Menú**: "Export Tiled Map…" y "Import Tiled Map…" son ítems propios del menú File (junto a
   Export e Import Sprite Sheet) en lugar de un submenú Export → Tiled: así Export y su atajo no
   cambian.
2. **Tileset externo en TMJ** = `.tsj` (tileset JSON de Tiled), no `.tsx`: Tiled lee los dos, pero los
   lectores de JSON (Godot, Defold, engines web) suelen esperar todo en JSON.
3. **Capas de imagen** se exportan como `<imagelayer>` con su propio PNG (no se omiten): sirven de
   fondo/decoración y se ven en Tiled igual que en ChimpSprite.
4. **Un mapa por frame** con la opción All; por defecto solo el frame actual. Tiled no tiene
   animación de capas, solo de tiles.
5. **Columnas del tileset**: `ceil(√n)`, una imagen casi cuadrada.
6. **Tile de otro tamaño** que el del mapa → la capa sale como imagen (aviso).
7. **Slices → objetos** en una capa "Slices" con sus propiedades.
8. **Carpeta de salida directa**: con Folder lleno, Export escribe ahí sin preguntar y reemplaza
   los archivos con el mismo nombre (el mensaje final los lista).
9. **Web**: un `.zip` (el navegador bloquea muchas descargas sueltas).
10. Import: sprite RGBA, un frame, tilesets de imagen única.
