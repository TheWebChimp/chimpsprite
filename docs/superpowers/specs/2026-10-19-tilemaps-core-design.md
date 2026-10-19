# Tilemaps, núcleo (subproyecto 1 de Tiles)

Fecha: 2026-10-19 · Estado: aprobado

## Contexto

Tiles se divide en cuatro subproyectos, en este orden:

1. **Núcleo de tilemaps** (este documento).
2. Edición de tilemaps: modo Tiles, modos de tileset Manual/Auto/Stack, panel de tileset, diálogo de
   nueva capa.
3. Exportar a Tiled Map Editor (`.tsx`, `.tmx`/`.tmj`).
4. Vista en mosaico (tiled mode).

Referencia: `docs/research/2026-10-19-aseprite-gap-analysis.md`, sección 0, y la especificación del
formato de Aseprite (`docs/ase-file-specs.md` en el repo de Aseprite).

## Objetivo

Los archivos de Aseprite con capas tilemap se abren, se ven y se guardan sin perder nada. Hoy
`cs-io/src/aseprite.rs` carga esas capas vacías (línea ~374) y descarta sus cels (línea ~473).
También se pueden crear y editar tilemaps por MCP.

Decisiones tomadas con el usuario:

- **Los tilesets pertenecen a cada artboard** (cada sprite), como en Aseprite. Compartirlos entre
  artboards queda fuera.
- **Las grillas de tiles se guardan en un pool aparte**, con el mismo espacio de ids que las
  imágenes. Así los cels tilemap reutilizan `Cel` (posición, opacidad, vínculos) sin mezclar tiles con
  píxeles.

Fuera de alcance:
- todo lo de los subproyectos 2–4;
- tilesets externos (flag 1): se cargan sin imágenes y con un aviso;
- Export Tileset;
- editar tilemaps con herramientas de la GUI.

## 1. Modelo (`cs-core/src/tilemap.rs`)

```rust
/// Valor de una celda de tilemap, con el mismo layout que Aseprite:
/// bits 0–28 índice del tile, bit 31 flip X, bit 30 flip Y, bit 29 flip D (transponer).
/// Coincide con los flags del GID de Tiled (bit 31 H, bit 30 V, bit 29 D).
pub struct Tile(pub u32);

impl Tile {
    pub const EMPTY: Tile = Tile(0);
    pub const INDEX_MASK: u32 = 0x1fff_ffff;
    pub const FLIP_X: u32 = 1 << 31;
    pub const FLIP_Y: u32 = 1 << 30;
    pub const FLIP_D: u32 = 1 << 29;
    pub fn new(index: u32, flip_x: bool, flip_y: bool, flip_d: bool) -> Tile;
    pub fn index(self) -> u32;
    pub fn flip_x(self) -> bool; // ídem flip_y, flip_d
}

/// Grilla de tiles de un cel; medidas en tiles, fila por fila.
pub struct TileGrid {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
}

impl TileGrid {
    pub fn new(width: u32, height: u32) -> TileGrid; // todo vacío
    pub fn get(&self, x: u32, y: u32) -> Option<Tile>;
    pub fn set(&mut self, x: u32, y: u32, tile: Tile) -> bool; // false si está fuera
}

/// Flips con los que un tile puede reconocerse como otro (los usa el modo Auto del subproyecto 2).
pub struct AllowedFlips { pub x: bool, pub y: bool, pub d: bool }

pub struct Tileset {
    pub name: String,
    pub tile_size: (u32, u32),
    /// Número que ve el usuario para el tile 1 (Aseprite usa 1 por defecto).
    pub base_index: i16,
    pub allowed_flips: AllowedFlips,
    /// El 0 es el tile vacío: siempre existe y es transparente. Imágenes en el modo de color del sprite.
    pub tiles: Vec<Arc<Image>>,
    /// User data del tileset y de cada tile, sin interpretar (para el round-trip exacto).
    pub user_data: Option<RawChunk>,
    pub tile_user_data: Vec<Option<RawChunk>>,
}

impl Tileset {
    pub fn new(name, tile_size, color_mode) -> Tileset; // con el tile 0 vacío
}
```

Todos los tipos llevan `Clone, Debug, PartialEq, Eq, Serialize, Deserialize`. `Tile` y
`AllowedFlips` también llevan `Copy`, y `AllowedFlips` también `Default`.

Cambios en `sprite.rs`:

- `LayerKind::Tilemap`.
- `Layer::tileset: Option<u32>`, con `#[serde(default)]`. Es el índice en `Sprite::tilesets` y solo
  tiene valor en capas tilemap.
- `Sprite::tilesets: Vec<Tileset>`, con `#[serde(default)]`.
- `Sprite::tilemaps: BTreeMap<ImageId, Arc<TileGrid>>`, con `#[serde(default, with = "as_entries")]`.
- `Sprite::next_image_id` tiene en cuenta los ids de `images` **y** los de `tilemaps`.

Un cel de una capa tilemap usa `Cel` sin cambios:

- `image` apunta a una entrada de `tilemaps`, no de `images`;
- `position` está en píxeles: es la esquina de la celda (0, 0);
- dos cels con el mismo id están vinculados, igual que con imágenes.

## 2. Render (`cs-render`)

`materialize` (hoy en `instance.rs`, que pasa a llamarse `materialize.rs`) ya convierte las capas de
instancia en capas de imagen antes de componer. Ahora también convierte las capas tilemap:

1. Para cada frame pedido con cel, arma una `Image` del modo del sprite, de
   `grid.width * tile_w` × `grid.height * tile_h`.
2. Copia ahí cada tile no vacío aplicando sus flips. El orden propuesto es D (transponer), luego X
   y luego Y. El orden definitivo lo fija el test contra los PNG que exporta Aseprite (sección 3).
3. El cel resultante conserva la `position` y la `opacity` del cel original.
4. Los tiles se copian como valores, sin mezclar. Por eso funciona igual en RGBA, en indexado y en
   escala de grises.

Casos que no dibujan nada para esa celda:
- un tileset inexistente o un índice fuera de rango;
- un flip D en un tile que no es cuadrado (además se registra con `log::warn` una vez).

Las capas tilemap ocultas siguen la misma regla que las instancias: no se dibujan al componer, pero
`bake_instances` (que pasa a llamarse `bake_layers`) sí las hornea, para conservarlas al guardar.

Con esto, canvas, onion, exports y render por MCP los dibujan sin más cambios.

## 3. Archivos (`cs-io`)

### Lectura de `.aseprite`

- **Chunk `0x2023` (Tileset):**
  - id, flags, número de tiles, tamaño de tile, base index y nombre;
  - con el flag 2, la imagen comprimida es una tira vertical de `tile_w × (tile_h · N)`, que se
    corta en N tiles;
  - el flag 4 (el tile 0 es el vacío) se asume siempre; si falta, se agrega un aviso;
  - los flags 8/16/32 van a `allowed_flips`;
  - con el flag 1 (externo) y sin el 2, el tileset queda solo con el tile 0 vacío y se agrega un
    aviso;
  - los tilesets se guardan en `sprite.tilesets` en el orden de su id.
- **User data:** los chunks `0x2020` que siguen a un tileset van, el primero a `user_data` y los
  siguientes, en orden, a `tile_user_data[i]`. Se agrega un `UserDataTarget::Tileset { index, next }`.
- **Capa tipo 2:** `LayerKind::Tilemap` con `tileset = Some(índice leído)`. Se quita el aviso actual.
- **Cel tipo 3:**
  - lee ancho, alto, bits por tile y las cuatro máscaras, y descomprime con zlib;
  - si los bits por tile no son 32, se agrega un aviso y el cel se omite;
  - cada valor se normaliza a las máscaras estándar de `Tile`;
  - la grilla se inserta en `tilemaps` con un id nuevo, como hacen las imágenes.
- **Cels vinculados (tipo 1)** en capas tilemap: apuntan al mismo id.

### Escritura de `.aseprite`

- Un chunk `0x2023` por tileset con los flags 2 y 4, más los de flips; después, su user data.
- El chunk se escribe en el primer frame, antes de las capas, que es lo que hace Aseprite.
- Capas tipo 2 con el índice de su tileset.
- Cels tipo 3 con 32 bits por tile, las máscaras estándar y los datos comprimidos con zlib.

### Formatos relacionados

- **`.chimp`:** no cambia, porque el `.aseprite` interno ya lleva todo.
- **Exportar `.aseprite`:** guarda los tilemaps como tilemaps; no se hornean.

### Tests de compatibilidad (`crates/cs-io/tests/`)

- Fixtures nuevos generados con el Aseprite real por Lua en modo batch, junto a `make.lua`:
  - `tilemap_rgba.aseprite`: dos capas tilemap que comparten un tileset de 8×8 con flips X/Y/D,
    cels vinculados, 2 frames y user data en el tileset y en un tile;
  - `tilemap_indexed.aseprite`;
  - más el PNG de cada frame exportado por Aseprite.
- Para cada uno se comprueba:
  - leer → render == PNG de Aseprite;
  - leer → escribir → leer da el mismo sprite;
  - lo escrito por ChimpSprite abierto por Aseprite (`-b --save-as`) == mismos PNG.

## 4. Comandos (`cs-commands`)

Primitivas nuevas, serializables y con inverso:

- `Command::SetTileset { index: usize, tileset: Option<Tileset> }`:
  - con `Some` en `index == len`, agrega;
  - con `Some` en `index < len`, reemplaza;
  - con `None`, quita, solo si ninguna capa lo usa (si no, error `TilesetInUse`).

  El inverso devuelve el estado anterior.
- `Command::SetTilemap { id: ImageId, grid: Option<Arc<TileGrid>> }` pone o quita una grilla del
  pool, igual que `SetImage`.

`DrawPixels` sobre una capa tilemap ya falla con `NotImageLayer`.

Ayudas en `ops`:

- `new_tilemap_layer(sprite, above, tileset) -> Command`: un `AddLayer` con `kind = Tilemap`.
- `set_tiles(sprite, layer, frame, cells: &[(i32, i32, Tile)]) -> Option<Command>`:
  - coordenadas en celdas de la grilla;
  - si no hay cel, crea uno en (0, 0) con la grilla mínima que abarque las celdas;
  - si las celdas caen fuera de la grilla, la agranda: rellena con vacíos y corre la `position` del
    cel si crece hacia la izquierda o hacia arriba;
  - si el cel está vinculado, la grilla nueva usa un id nuevo y solo ese cel deja de estar
    vinculado, igual que al pintar píxeles;
  - devuelve un `Batch` de `SetTilemap` + `SetCel`.
- `set_tile_image(sprite, tileset, index, image) -> Option<Command>`: un `SetTileset` con el tile
  reemplazado (o agregado si `index == tiles.len()`). El tile 0 no se puede cambiar.

Ajustes a operaciones existentes:

- **`remove_layer`** también quita del pool las grillas que ya no usa ningún cel. Lo mismo se aplica
  a las imágenes huérfanas al borrar capas tilemap. El inverso las restaura.
- **`ResizeCanvas`** mueve los cels tilemap como al resto, porque usan `Cel.position`.
- **`TrimToCanvas`** ignora los cels tilemap; recortar tilemaps queda para el subproyecto 2.

## 5. MCP (`cs-agent` y `apps/chimpsprite-mcp`)

- `create_tileset { doc, artboard?, name, tile_width, tile_height, base_index? }`.
- `set_tile_pixels { doc, artboard?, tileset, tile, pixels: [{x, y, color}] }`:
  - pinta un tile, y lo crea si `tile == count`;
  - el tile 0 no se puede editar.
- `add_tilemap_layer { doc, artboard?, tileset, name? }`.
- `set_tiles { doc, artboard?, layer, frame?, cells: [{x, y, tile, flip_x?, flip_y?, flip_d?}] }`.
- `get_tiles { doc, artboard?, layer, frame? }`: devuelve la grilla, el índice y los flips de cada
  celda, y la posición del cel.
- `describe` agrega a cada artboard sus `tilesets` (nombre, tamaño de tile, cantidad de tiles,
  base index) y, en las capas tilemap, `kind: "tilemap"` y `tileset`.
- El render y el export ya pasan por `materialize`/`bake_layers`.

## 6. UI mínima (`cs-ui`)

- El canvas, la miniatura y el export dialog ya lo dibujan por `materialize`/`bake_layers`.
- En el timeline, las capas tilemap llevan el prefijo `▦`, o uno que exista en la fuente (verificarlo
  con una captura). Sus cels se dibujan como los de imagen, con el vínculo incluido.
- Las herramientas de pintura y Move sobre una capa tilemap no hacen nada. La barra de estado dice
  "Tilemap layer: tile editing comes in a later version".
- `Layer → New Tilemap Layer` no existe todavía; llega en el subproyecto 2.

## 7. Tests

**`cs-core`**
- `Tile`: empaquetar y desempaquetar los flips.
- `TileGrid`: `get`/`set` con límites.
- `next_image_id` considera `tilemaps`.
- Un `Sprite` con tilesets y tilemaps pasa por JSON; los sprites viejos siguen cargando.

**`cs-render`**
- Tilemap de 2×2 con los 8 flips posibles sobre un tile asimétrico; se compara pixel a pixel.
- Indexado.
- Opacidad del cel × capa.
- Tileset inexistente y tile fuera de rango.
- Capa oculta: no se dibuja, pero sí se hornea.

**`cs-commands`**
- Undo de `SetTileset` (agregar, reemplazar, quitar) y `TilesetInUse`.
- Undo de `SetTilemap`.
- `set_tiles` crea el cel, agranda hacia la izquierda y hacia arriba (corre `position`) y desvincula.
- `remove_layer` limpia el pool y el undo lo restaura.

**`cs-io`**
- Los fixtures reales de Aseprite (sección 3).
- Round-trip de user data.
- Tileset externo: aviso.
- Bits por tile distintos de 32: aviso.

**`cs-agent`**
- `create_tileset`, `set_tile_pixels`, `add_tilemap_layer`, `set_tiles` y luego render.
- `get_tiles`.
- `describe`.
