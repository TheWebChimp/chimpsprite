# Tilemaps, núcleo — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Abrir, ver y guardar sin pérdidas los `.aseprite` con capas tilemap (tilesets, grillas,
flips, cels vinculados, user data), y poder crear y editar tilemaps por MCP.

**Architecture:** `cs-core` gana `Tile`, `TileGrid`, `Tileset` y `AllowedFlips`; cada `Sprite` guarda
sus `tilesets` y un pool `tilemaps` de grillas con el mismo espacio de ids que `images`, así un cel
tilemap es un `Cel` normal cuyo `image` apunta a `tilemaps`. Antes de componer,
`cs_render::materialize` (módulo renombrado `instance.rs` → `materialize.rs`) convierte las capas
tilemap en capas de imagen copiando cada tile con sus flips; el compositor, el onion, los exports y
el render MCP no cambian. `bake_instances` pasa a ser `bake_layers(doc, sprite, Bake)`: los formatos
de solo píxeles hornean todo (`Bake::Everything`) y `.aseprite` hornea solo instancias
(`Bake::KeepTilemaps`). `cs-io` lee y escribe los chunks `0x2023`, las capas tipo 2 y los cels tipo 3;
dos primitivas nuevas (`SetTileset`, `SetTilemap`) más ayudas en `ops` alimentan las herramientas MCP.

**Tech Stack:** Rust 2024, workspace con crates `cs-core`, `cs-render`, `cs-commands`, `cs-io`,
`cs-tools`, `cs-agent`, `cs-ui` (egui 0.36) y `apps/chimpsprite-mcp` (rmcp). Fixtures generados con
Aseprite 1.3.18 por Lua en modo batch.

Spec: `docs/superpowers/specs/2026-10-19-tilemaps-core-design.md`
Formato `.aseprite`: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>

## Global Constraints

- El código y los comentarios siguen el estilo del repo: comentarios en español, textos de UI y
  descripciones MCP en inglés.
- Los crates de motor (`cs-core`, `cs-render`, `cs-commands`, `cs-io`, `cs-tools`, `cs-agent`) no
  dependen de egui y compilan para `wasm32-unknown-unknown`.
- Todos los comandos son serializables y devuelven su inverso.
- Los `.chimp` existentes deben seguir cargando: campos nuevos con `#[serde(default)]`.
- Los tilesets pertenecen a cada artboard (`Sprite::tilesets`); no se comparten entre artboards.
- Las grillas viven en `Sprite::tilemaps`, con el mismo espacio de ids que `Sprite::images`
  (`next_image_id` considera ambos pools). Dos cels con el mismo id están vinculados.
- `Tile` usa el layout de Aseprite: bits 0–28 índice, bit 31 flip X, bit 30 flip Y, bit 29 flip D.
- El tile 0 de todo tileset es el vacío: siempre existe, es transparente y no se edita.
- Los tiles se copian como valores (sin mezclar): igual en RGBA e indexado.
- El orden de los flips al dibujar lo fijan los PNG que exporta Aseprite (Task 5); el punto de
  partida es D (transponer), luego X, luego Y.
- Fuera de alcance: modo Tiles en la GUI, modos Manual/Auto/Stack, panel de tileset, diálogo de nueva
  capa tilemap, export a Tiled, tiled mode, tilesets externos (se cargan vacíos con aviso) y Export
  Tileset.
- `.chimp` no cambia: el `.aseprite` interno ya guarda los tilemaps.

## Toolchain

Antes de cualquier comando `cargo`, en Git Bash:

```bash
export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH
```

CI (`.github/workflows/ci.yml`) corre:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --target wasm32-unknown-unknown -p cs-core -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite
```

Aseprite real (solo para Task 5 y Task 8):
`"/c/Program Files (x86)/Steam/steamapps/common/Aseprite/Aseprite.exe"`, usable con `-b --script`.

Cada commit termina con una línea en blanco y
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` (con un segundo `-m`).

## File Structure

| Archivo | Cambio |
|---|---|
| `crates/cs-core/src/tilemap.rs` | **Nuevo.** `Tile`, `TileGrid`, `AllowedFlips`, `Tileset`. |
| `crates/cs-core/src/sprite.rs` | `LayerKind::Tilemap`, `Layer::tileset`, `Sprite::tilesets`, `Sprite::tilemaps`, `Sprite::tilemap`, `next_image_id`. |
| `crates/cs-core/src/lib.rs` | `mod tilemap; pub use tilemap::*;`. |
| `crates/cs-render/src/instance.rs` → `materialize.rs` | **Renombrado.** `Bake`, `bake_layers`, `materialize_tilemaps`, tilemaps en `materialize`. |
| `crates/cs-render/src/lib.rs`, `Cargo.toml` | Reexports; dependencia `log`. |
| `crates/cs-commands/src/command.rs` | `SetTileset`, `SetTilemap`, `CmdError::TilesetInUse`, limpieza de pools en `RemoveLayer`. |
| `crates/cs-commands/src/ops.rs` | `new_tilemap_layer`, `set_tiles`, `set_tile_image`; copia de grillas al duplicar/desvincular. |
| `crates/cs-io/src/aseprite.rs` | Chunk `0x2023`, capas tipo 2, cels tipo 3, user data de tilesets. |
| `crates/cs-io/src/files.rs`, `chimp.rs`, `export.rs` | `bake_layers` en lugar de `bake_instances`. |
| `crates/cs-io/tests/fixtures/tilemaps.lua` + `tilemap_*.aseprite`/`.png` | **Nuevos.** Fixtures del Aseprite real. |
| `crates/cs-io/tests/aseprite_interop.rs` | Tests de tilemaps contra Aseprite. |
| `crates/cs-agent/src/ops/tilemap.rs` | **Nuevo.** Herramientas MCP de tilemaps. |
| `crates/cs-agent/src/params.rs`, `ops/mod.rs`, `ops/document.rs`, `ops/view.rs`, `ops/color.rs`, `target.rs` | Parámetros, `describe`, export, guardas. |
| `apps/chimpsprite-mcp/src/server.rs` | Registro de las cinco herramientas. |
| `crates/cs-tools/src/move_tool.rs`, `eyedropper.rs`, `select.rs` | Move no hace nada en tilemaps; lectura de lo visible con tilemaps. |
| `crates/cs-ui/src/timeline.rs`, `panels.rs`, `canvas.rs`, `export_dialog.rs` | Prefijo `▦`, aviso en la barra de estado, pipeta, export. |

---

### Task 1: Modelo de tilemaps en `cs-core`

**Files:**
- Create: `crates/cs-core/src/tilemap.rs`
- Modify: `crates/cs-core/src/lib.rs`
- Modify: `crates/cs-core/src/sprite.rs` (`LayerKind` 31-37, `Layer` 39-59, `Layer::new_image` 62-76, `Sprite` 174-197, `Sprite::new` 276-291, `next_image_id` 309-311)
- Modify: `crates/cs-io/src/aseprite.rs:404-420` (literal `Layer { … }`)
- Modify: `crates/cs-commands/src/ops.rs:25-29` (prefijo de `new_layer`)
- Modify: `crates/cs-agent/src/ops/document.rs:77-81` (`match l.kind`)

**Interfaces:**
- Produces:
  - `pub struct Tile(pub u32)` (`Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize`) con
    `Tile::EMPTY`, `Tile::INDEX_MASK`, `Tile::FLIP_X`, `Tile::FLIP_Y`, `Tile::FLIP_D`,
    `Tile::new(index: u32, flip_x: bool, flip_y: bool, flip_d: bool) -> Tile`,
    `index(self) -> u32`, `flip_x(self) -> bool`, `flip_y(self) -> bool`, `flip_d(self) -> bool`
  - `pub struct TileGrid { pub width: u32, pub height: u32, pub tiles: Vec<Tile> }` con
    `TileGrid::new(width: u32, height: u32) -> TileGrid`, `get(&self, x: u32, y: u32) -> Option<Tile>`,
    `set(&mut self, x: u32, y: u32, tile: Tile) -> bool`
  - `pub struct AllowedFlips { pub x: bool, pub y: bool, pub d: bool }` (`Copy`, `Default`)
  - `pub struct Tileset { pub name: String, pub tile_size: (u32, u32), pub base_index: i16, pub allowed_flips: AllowedFlips, pub tiles: Vec<Arc<Image>>, pub user_data: Option<RawChunk>, pub tile_user_data: Vec<Option<RawChunk>> }`
    con `Tileset::new(name: impl Into<String>, tile_size: (u32, u32), color_mode: ColorMode) -> Tileset`
  - `LayerKind::Tilemap`; `Layer::tileset: Option<u32>`
  - `Sprite::tilesets: Vec<Tileset>`; `Sprite::tilemaps: BTreeMap<ImageId, Arc<TileGrid>>`
  - `Sprite::tilemap(&self, id: ImageId) -> Option<&Arc<TileGrid>>`
  - `Sprite::next_image_id` = máximo de `images` y `tilemaps` + 1
  - `ops::new_layer(…, LayerKind::Tilemap)` nombra la capa "Tilemap N"; `describe` usa `"tilemap"`.

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/cs-core/src/tilemap.rs` solo con el módulo de tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cel, ImageId, LayerId, LayerKind, PixelValue, Sprite};

    #[test]
    fn tiles_pack_index_and_flips_like_aseprite() {
        let t = Tile::new(5, true, false, true);
        assert_eq!(t.0, 5 | 0x8000_0000 | 0x2000_0000);
        assert_eq!(
            (t.index(), t.flip_x(), t.flip_y(), t.flip_d()),
            (5, true, false, true)
        );
        assert_eq!(Tile::new(0, false, true, false).0, 0x4000_0000);
        assert_eq!(
            Tile::new(0x3fff_ffff, false, false, false).index(),
            0x1fff_ffff,
            "the index is masked"
        );
        assert_eq!(Tile::EMPTY.index(), 0);
    }

    #[test]
    fn grid_get_and_set_respect_bounds() {
        let mut g = TileGrid::new(3, 2);
        assert_eq!(g.tiles.len(), 6);
        assert_eq!(g.get(2, 1), Some(Tile::EMPTY));
        assert!(g.set(2, 1, Tile(7)));
        assert_eq!(g.get(2, 1), Some(Tile(7)));
        assert_eq!(g.tiles[5], Tile(7), "row by row");
        assert!(!g.set(3, 0, Tile(1)));
        assert!(!g.set(0, 2, Tile(1)));
        assert_eq!(g.get(3, 0), None);
        // Una grilla corrupta (menos tiles que celdas) no entra en pánico.
        let broken = TileGrid {
            width: 2,
            height: 2,
            tiles: vec![Tile(1)],
        };
        assert_eq!(broken.get(1, 1), None);
    }

    #[test]
    fn new_tileset_has_the_empty_tile() {
        let ts = Tileset::new(
            "t",
            (4, 2),
            ColorMode::Indexed {
                transparent_index: 3,
            },
        );
        assert_eq!(ts.tiles.len(), 1);
        assert_eq!((ts.tiles[0].width(), ts.tiles[0].height()), (4, 2));
        assert_eq!(ts.tiles[0].get(0, 0), Some(PixelValue::Index(3)));
        assert_eq!(ts.base_index, 1);
        assert_eq!(ts.allowed_flips, AllowedFlips::default());
        assert!(ts.user_data.is_none() && ts.tile_user_data.is_empty());
    }

    #[test]
    fn image_ids_are_shared_with_tilemaps() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        s.tilemaps
            .insert(ImageId(9), Arc::new(TileGrid::new(1, 1)));
        assert_eq!(s.next_image_id(), ImageId(10));
        s.images
            .insert(ImageId(12), Arc::new(Image::new(1, 1, ColorMode::Rgba)));
        assert_eq!(s.next_image_id(), ImageId(13));
        assert!(s.tilemap(ImageId(9)).is_some());
        assert!(s.tilemap(ImageId(12)).is_none());
    }

    #[test]
    fn sprites_with_tilemaps_serialize_and_old_sprites_still_load() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        let mut ts = Tileset::new("ground", (4, 4), ColorMode::Rgba);
        ts.tiles
            .push(Arc::new(Image::new(4, 4, ColorMode::Rgba)));
        ts.user_data = Some(RawChunk {
            kind: 0x2020,
            data: vec![1, 0, 0, 0, 2, 0, b'h', b'i'],
        });
        ts.tile_user_data = vec![
            None,
            Some(RawChunk {
                kind: 0x2020,
                data: vec![0; 4],
            }),
        ];
        ts.allowed_flips.x = true;
        s.tilesets.push(ts);
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        let mut grid = TileGrid::new(2, 1);
        grid.set(1, 0, Tile::new(1, true, false, false));
        s.tilemaps.insert(ImageId(1), Arc::new(grid));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (4, 0),
                opacity: 255,
                image: ImageId(1),
            },
        );
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Sprite>(&json).unwrap(), s);

        // Un sprite guardado antes de los tilemaps (sin esos campos) sigue cargando.
        let mut old = serde_json::to_value(Sprite::new(2, 2, ColorMode::Rgba)).unwrap();
        let obj = old.as_object_mut().unwrap();
        obj.remove("tilesets");
        obj.remove("tilemaps");
        obj["layers"][0].as_object_mut().unwrap().remove("tileset");
        let back: Sprite = serde_json::from_value(old).unwrap();
        assert!(back.tilesets.is_empty() && back.tilemaps.is_empty());
        assert_eq!(back.layers[0].tileset, None);
    }
}
```

Agregar en `crates/cs-core/src/lib.rs` `mod tilemap;` (después de `mod sprite;`) y
`pub use tilemap::*;` (después de `pub use sprite::*;`).

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-core tilemap`
Expected: error de compilación (`Tile`, `TileGrid`, `Tileset`… no existen).

- [ ] **Step 3: Implementar**

Arriba de los tests en `crates/cs-core/src/tilemap.rs`:

```rust
//! Tilemaps: tilesets (las imágenes de los tiles) y las grillas de tiles de los cels.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{ColorMode, Image, RawChunk};

/// Valor de una celda de tilemap, con el mismo layout que Aseprite:
/// bits 0–28 índice del tile, bit 31 flip X, bit 30 flip Y, bit 29 flip D (transponer).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile(pub u32);

impl Tile {
    pub const EMPTY: Tile = Tile(0);
    pub const INDEX_MASK: u32 = 0x1fff_ffff;
    pub const FLIP_X: u32 = 1 << 31;
    pub const FLIP_Y: u32 = 1 << 30;
    pub const FLIP_D: u32 = 1 << 29;

    pub fn new(index: u32, flip_x: bool, flip_y: bool, flip_d: bool) -> Tile {
        let flag = |on: bool, bit: u32| if on { bit } else { 0 };
        Tile(
            (index & Self::INDEX_MASK)
                | flag(flip_x, Self::FLIP_X)
                | flag(flip_y, Self::FLIP_Y)
                | flag(flip_d, Self::FLIP_D),
        )
    }

    pub fn index(self) -> u32 {
        self.0 & Self::INDEX_MASK
    }

    pub fn flip_x(self) -> bool {
        self.0 & Self::FLIP_X != 0
    }

    pub fn flip_y(self) -> bool {
        self.0 & Self::FLIP_Y != 0
    }

    pub fn flip_d(self) -> bool {
        self.0 & Self::FLIP_D != 0
    }
}

/// Grilla de tiles de un cel; medidas en tiles, fila por fila.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileGrid {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
}

impl TileGrid {
    /// Grilla con todas las celdas vacías.
    pub fn new(width: u32, height: u32) -> TileGrid {
        TileGrid {
            width,
            height,
            tiles: vec![Tile::EMPTY; width as usize * height as usize],
        }
    }

    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height).then(|| y as usize * self.width as usize + x as usize)
    }

    pub fn get(&self, x: u32, y: u32) -> Option<Tile> {
        self.tiles.get(self.offset(x, y)?).copied()
    }

    /// `false` si la celda cae fuera de la grilla.
    pub fn set(&mut self, x: u32, y: u32, tile: Tile) -> bool {
        match self.offset(x, y).and_then(|i| self.tiles.get_mut(i)) {
            Some(t) => {
                *t = tile;
                true
            }
            None => false,
        }
    }
}

/// Flips con los que un tile puede reconocerse como otro (los usa el modo Auto del subproyecto 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowedFlips {
    pub x: bool,
    pub y: bool,
    pub d: bool,
}

/// Imágenes de los tiles que usan las capas tilemap de un sprite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tileset {
    pub name: String,
    pub tile_size: (u32, u32),
    /// Número que ve el usuario para el tile 1 (Aseprite usa 1 por defecto).
    pub base_index: i16,
    #[serde(default)]
    pub allowed_flips: AllowedFlips,
    /// El 0 es el tile vacío: siempre existe y es transparente. Imágenes en el modo de color del
    /// sprite y del tamaño `tile_size`.
    pub tiles: Vec<Arc<Image>>,
    /// User data del tileset y de cada tile, sin interpretar (para el round-trip exacto).
    #[serde(default)]
    pub user_data: Option<RawChunk>,
    #[serde(default)]
    pub tile_user_data: Vec<Option<RawChunk>>,
}

impl Tileset {
    /// Tileset con solo el tile 0 (vacío).
    pub fn new(name: impl Into<String>, tile_size: (u32, u32), color_mode: ColorMode) -> Tileset {
        Tileset {
            name: name.into(),
            tile_size,
            base_index: 1,
            allowed_flips: AllowedFlips::default(),
            tiles: vec![Arc::new(Image::new(tile_size.0, tile_size.1, color_mode))],
            user_data: None,
            tile_user_data: Vec::new(),
        }
    }
}
```

En `crates/cs-core/src/sprite.rs`:

1. Imports: `use crate::{ColorMode, Image, Palette, PixelValue, Tag, TileGrid, Tileset};`.
2. `LayerKind`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Image,
    Group,
    /// Muestra otro artboard (un símbolo) con su propia línea de tiempo; no tiene cels.
    Instance,
    /// Grilla de tiles de un tileset del sprite; sus cels apuntan a `Sprite::tilemaps`.
    Tilemap,
}
```

3. Al final de `struct Layer`, después de `instance`:

```rust
    /// Índice en `Sprite::tilesets`; solo en capas `LayerKind::Tilemap`.
    #[serde(default)]
    pub tileset: Option<u32>,
```

4. En `Layer::new_image`, agregar `tileset: None,` después de `instance: None,`.
5. Al final de `struct Sprite`, después de `slices`:

```rust
    /// Tilesets del sprite; las capas tilemap los referencian por índice.
    #[serde(default)]
    pub tilesets: Vec<Tileset>,
    /// Grillas de los cels tilemap. Comparten el espacio de ids con `images`.
    #[serde(default, with = "as_entries")]
    pub tilemaps: BTreeMap<ImageId, Arc<TileGrid>>,
```

6. En `Sprite::new`, agregar `tilesets: Vec::new(),` y `tilemaps: BTreeMap::new(),` después de
   `slices: Vec::new(),`.
7. Reemplazar `next_image_id` y agregar `tilemap` después de `image`:

```rust
    /// Id libre para `images` o `tilemaps` (comparten el espacio de ids).
    pub fn next_image_id(&self) -> ImageId {
        let max = self.images.keys().chain(self.tilemaps.keys()).map(|k| k.0).max();
        ImageId(max.map_or(1, |m| m + 1))
    }
```

```rust
    pub fn tilemap(&self, id: ImageId) -> Option<&Arc<TileGrid>> {
        self.tilemaps.get(&id)
    }
```

En `crates/cs-io/src/aseprite.rs`, en el literal `sprite.layers.push(Layer { … })` de `read_layer`,
agregar `tileset: None,` después de `instance: None,` (Task 4 lo completa).

En `crates/cs-commands/src/ops.rs`, `new_layer`:

```rust
    let prefix = match kind {
        LayerKind::Group => "Group",
        LayerKind::Instance => "Instance",
        LayerKind::Tilemap => "Tilemap",
        LayerKind::Image => "Layer",
    };
```

En `crates/cs-agent/src/ops/document.rs`, `describe_sprite`:

```rust
                "kind": match l.kind {
                    LayerKind::Group => "group",
                    LayerKind::Instance => "instance",
                    LayerKind::Tilemap => "tilemap",
                    LayerKind::Image => "image",
                },
```

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo test -p cs-core && cargo check --workspace --all-targets`
Expected: todos los tests de `cs-core` pasan; el workspace compila.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-core crates/cs-io/src/aseprite.rs crates/cs-commands/src/ops.rs crates/cs-agent/src/ops/document.rs
git commit -m "feat(core): tilesets, tile grids and tilemap layers" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Materializar tilemaps en `cs-render` (y `bake_layers`)

**Files:**
- Rename: `crates/cs-render/src/instance.rs` → `crates/cs-render/src/materialize.rs` (`git mv`)
- Modify: `crates/cs-render/src/lib.rs:5-7`
- Modify: `crates/cs-render/Cargo.toml` (`log`)
- Modify (callers de `bake_instances`): `crates/cs-io/src/files.rs:86`, `crates/cs-io/src/chimp.rs:158`,
  `crates/cs-io/src/export.rs:464`, `crates/cs-agent/src/ops/view.rs:141`,
  `crates/cs-ui/src/export_dialog.rs:142` y `:174`

**Interfaces:**
- Consumes: `Tile`, `TileGrid`, `Tileset`, `Sprite::tilemap`, `LayerKind::Tilemap`, `Layer::tileset` (Task 1).
- Produces:
  - `pub enum Bake { Everything, KeepTilemaps }` (`Clone, Copy, Debug, PartialEq, Eq`)
  - `pub fn materialize<'a>(doc: &Document, sprite: &'a Sprite, frames: Range<usize>) -> Cow<'a, Sprite>`:
    instancias y tilemaps pasan a `LayerKind::Image`; los cels tilemap de `frames` pasan a imágenes
    (misma `position` y `opacity`); las capas ocultas quedan sin cels; `Cow::Borrowed` si no hay nada
    que convertir.
  - `pub fn materialize_tilemaps<'a>(sprite: &'a Sprite, frames: Range<usize>) -> Cow<'a, Sprite>`:
    lo mismo solo para tilemaps, sin documento (para herramientas).
  - `pub fn bake_layers(doc: &Document, sprite: &Sprite, bake: Bake) -> Option<Sprite>`: todos los
    frames, incluidas las capas ocultas; `None` si no hay nada que hornear. Reemplaza a
    `bake_instances` (que deja de existir).
  - En la salida materializada de tilemaps, `tilesets` y `tilemaps` quedan vacíos.

- [ ] **Step 1: Renombrar el módulo y escribir los tests que fallan**

```bash
git mv crates/cs-render/src/instance.rs crates/cs-render/src/materialize.rs
```

En `crates/cs-render/src/lib.rs`, reemplazar `mod instance;` y su `pub use` por:

```rust
mod materialize;
mod normal;
pub use materialize::{Bake, bake_layers, materialize, materialize_tilemaps};
pub use normal::normal_map;
```

En `crates/cs-render/Cargo.toml`, en `[dependencies]`, agregar `log.workspace = true`.

En los tests existentes de `materialize.rs`, reemplazar las tres llamadas
`bake_instances(&d, X)` por `bake_layers(&d, X, Bake::Everything)` (en
`hidden_instance_layers_are_not_materialized` y `bake_covers_every_frame`).

Agregar al final del módulo de tests de `materialize.rs` (usa `artboard`, `host`, `doc` y los
imports `use cs_core::*;` ya presentes):

```rust
    // --- Tilemaps ---

    /// Color distinto en cada píxel de un tile de 3×3 (asimétrico en todas las direcciones).
    fn asymmetric(x: u32, y: u32) -> Rgba {
        Rgba::rgb(40 + x as u8 * 70, 40 + y as u8 * 70, 200)
    }

    fn rgba_tileset() -> Tileset {
        let mut ts = Tileset::new("t", (3, 3), ColorMode::Rgba);
        let mut tile = Image::new(3, 3, ColorMode::Rgba);
        for y in 0..3 {
            for x in 0..3 {
                tile.set(x, y, PixelValue::Rgba(asymmetric(x, y))).unwrap();
            }
        }
        ts.tiles.push(Arc::new(tile));
        ts
    }

    /// Sprite cuya capa 1 es tilemap con `tileset` (índice 0) y `grid` en el frame 0, con 2 px de
    /// margen a la derecha y abajo.
    fn tiled(mode: ColorMode, tileset: Tileset, grid: TileGrid, position: (i32, i32)) -> Sprite {
        let (tw, th) = tileset.tile_size;
        let mut s = Sprite::new(grid.width * tw + 2, grid.height * th + 2, mode);
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        s.tilesets.push(tileset);
        s.tilemaps.insert(ImageId(1), Arc::new(grid));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position,
                opacity: 255,
                image: ImageId(1),
            },
        );
        s
    }

    #[test]
    fn tilemaps_draw_every_flip_combination() {
        // 8 celdas (4×2): la celda i tiene flip X si i&1, Y si i&2 y D si i&4.
        let mut grid = TileGrid::new(4, 2);
        for i in 0..8u32 {
            grid.set(i % 4, i / 4, Tile::new(1, i & 1 != 0, i & 2 != 0, i & 4 != 0));
        }
        let s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (0, 0));
        let m = materialize_tilemaps(&s, 0..1);
        assert_eq!(m.layer(LayerId(1)).unwrap().kind, LayerKind::Image);
        let pixels = render_frame(&m, 0);
        let w = s.width as usize;
        for i in 0..8u32 {
            // Referencia independiente, como operaciones de imagen: transponer (D), espejar en X y
            // después en Y.
            let mut t: [[Rgba; 3]; 3] =
                std::array::from_fn(|y| std::array::from_fn(|x| asymmetric(x as u32, y as u32)));
            if i & 4 != 0 {
                let src = t;
                t = std::array::from_fn(|y| std::array::from_fn(|x| src[x][y]));
            }
            if i & 1 != 0 {
                for row in &mut t {
                    row.reverse();
                }
            }
            if i & 2 != 0 {
                t.reverse();
            }
            let (cx, cy) = ((i % 4) as usize * 3, (i / 4) as usize * 3);
            for (y, row) in t.iter().enumerate() {
                for (x, expected) in row.iter().enumerate() {
                    assert_eq!(
                        pixels[(cy + y) * w + cx + x],
                        *expected,
                        "cell {i} pixel ({x}, {y})"
                    );
                }
            }
        }
    }

    #[test]
    fn indexed_tilemaps_copy_palette_indices() {
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        let mut ts = Tileset::new("t", (2, 2), mode);
        let mut tile = Image::new(2, 2, mode);
        tile.set(0, 0, PixelValue::Index(5)).unwrap();
        ts.tiles.push(Arc::new(tile));
        let mut grid = TileGrid::new(2, 1);
        grid.set(1, 0, Tile::new(1, true, false, false));
        let s = tiled(mode, ts, grid, (0, 0));
        let m = materialize_tilemaps(&s, 0..1);
        // Celda (1, 0) espejada en X: el índice 5 pasa de (0, 0) a (1, 0) del tile, (3, 0) del lienzo.
        assert_eq!(m.pixel(LayerId(1), 0, 3, 0), PixelValue::Index(5));
        assert_eq!(m.pixel(LayerId(1), 0, 2, 0), PixelValue::Index(0));
        assert_eq!(render_frame(&m, 0)[3], s.palette.get(5).unwrap());
    }

    #[test]
    fn tilemap_cels_keep_position_and_opacity() {
        let mut grid = TileGrid::new(1, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        let mut s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (1, 2));
        s.cels.get_mut(&(LayerId(1), 0)).unwrap().opacity = 128;
        s.layers[0].opacity = 128;
        let m = materialize_tilemaps(&s, 0..1);
        let cel = m.cel(LayerId(1), 0).unwrap();
        assert_eq!((cel.position, cel.opacity), ((1, 2), 128));
        let px = render_frame(&m, 0)[2 * s.width as usize + 1];
        assert_eq!((px.r, px.g, px.b), (40, 40, 200));
        assert!((60..=68).contains(&px.a), "alpha {}", px.a);
    }

    #[test]
    fn missing_tilesets_and_tiles_draw_nothing() {
        let mut grid = TileGrid::new(2, 1);
        grid.set(0, 0, Tile::new(9, false, false, false));
        grid.set(1, 0, Tile::new(1, false, false, false));
        let s = tiled(ColorMode::Rgba, rgba_tileset(), grid.clone(), (0, 0));
        let pixels = render_frame(&materialize_tilemaps(&s, 0..1), 0);
        assert_eq!(pixels[0].a, 0, "tile 9 does not exist");
        assert_eq!(pixels[3], asymmetric(0, 0), "tile 1 does");
        let mut s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (0, 0));
        s.layers[0].tileset = Some(7);
        let m = materialize_tilemaps(&s, 0..1);
        assert!(m.cels.is_empty());
        assert!(render_frame(&m, 0).iter().all(|c| c.a == 0));
    }

    #[test]
    fn diagonal_flip_needs_square_tiles() {
        let mut ts = Tileset::new("t", (2, 1), ColorMode::Rgba);
        let mut tile = Image::new(2, 1, ColorMode::Rgba);
        tile.set(0, 0, PixelValue::Rgba(Rgba::rgb(1, 2, 3))).unwrap();
        ts.tiles.push(Arc::new(tile));
        let mut grid = TileGrid::new(2, 1);
        grid.set(0, 0, Tile::new(1, false, false, true));
        grid.set(1, 0, Tile::new(1, false, false, false));
        let s = tiled(ColorMode::Rgba, ts, grid, (0, 0));
        let pixels = render_frame(&materialize_tilemaps(&s, 0..1), 0);
        assert_eq!(pixels[0].a, 0, "D flip on a 2×1 tile is not drawn");
        assert_eq!(pixels[2], Rgba::rgb(1, 2, 3));
    }

    #[test]
    fn hidden_tilemaps_are_not_drawn_but_are_baked() {
        let mut grid = TileGrid::new(1, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        let mut s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (0, 0));
        s.layers[0].visible = false;
        let d = Document::with_sprite("t", s);
        let sprite = &d.artboards[0].sprite;
        assert!(materialize(&d, sprite, 0..1).cels.is_empty());
        let baked = bake_layers(&d, sprite, Bake::Everything).unwrap();
        assert!(baked.cel(LayerId(1), 0).is_some());
        assert!(baked.tilesets.is_empty() && baked.tilemaps.is_empty());
        assert!(
            bake_layers(&d, sprite, Bake::KeepTilemaps).is_none(),
            "no instances to bake"
        );
    }

    #[test]
    fn linked_tilemap_cels_stay_linked_and_instances_show_tilemaps() {
        let mut grid = TileGrid::new(1, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        let mut s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (0, 0));
        Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
        let cel = *s.cel(LayerId(1), 0).unwrap();
        s.cels.insert((LayerId(1), 1), cel);
        let m = materialize_tilemaps(&s, 0..2);
        assert_eq!(
            m.cel(LayerId(1), 0).unwrap().image,
            m.cel(LayerId(1), 1).unwrap().image
        );
        // Un artboard con tilemaps visto como instancia.
        let mut d = doc(host(vec![InstanceKey::new(0, ArtboardId(3), (2, 2))]));
        d.artboards.push(artboard(3, s));
        let px = render_frame(&materialize(&d, &d.artboards[0].sprite, 0..1), 0);
        assert_eq!(px[2 * 8 + 2], asymmetric(0, 0));
    }
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-render`
Expected: error de compilación (`materialize_tilemaps`, `bake_layers`, `Bake` no existen).

- [ ] **Step 3: Implementar**

En `crates/cs-render/src/materialize.rs`, reemplazar todo lo que está arriba de `fn materialize_root`
(doc del módulo, imports, `MAX_DEPTH`, `materialize` y `bake_instances`) por:

```rust
//! Materializar: antes de componer, las capas que no son de imagen (instancias y tilemaps) se
//! convierten en capas de imagen con cels temporales; el compositor solo ve píxeles.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cs_core::{
    ArtboardId, Cel, ColorMode, Document, Image, ImageId, InstanceKey, InstanceTrack, LayerKind,
    PixelValue, Rgba, Sprite, Tile, TileGrid, Tileset, source_frame,
};

use crate::render_frame;

/// Profundidad máxima de instancias anidadas (defensa contra ciclos en datos corruptos).
const MAX_DEPTH: u32 = 16;

/// Qué capas convierte `bake_layers` en capas de imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bake {
    /// Instancias y tilemaps: para formatos de solo píxeles (PNG, GIF, hojas, atlas, motores).
    Everything,
    /// Solo instancias: para `.aseprite` (y el `.aseprite` interno del `.chimp`), que guarda los
    /// tilemaps como tilemaps.
    KeepTilemaps,
}

/// Copia del sprite donde las capas de instancia y las tilemap son capas de imagen con cels en
/// los frames de `frames`. Sin esas capas devuelve el mismo sprite. Es para mostrar: las capas
/// ocultas quedan sin cels.
pub fn materialize<'a>(
    doc: &Document,
    sprite: &'a Sprite,
    frames: Range<usize>,
) -> Cow<'a, Sprite> {
    let instances = materialize_root(doc, sprite, frames.clone(), true);
    with_tilemaps(instances, frames, true)
}

/// Como `materialize`, pero solo tilemaps y sin documento: para herramientas que leen lo visible.
pub fn materialize_tilemaps<'a>(sprite: &'a Sprite, frames: Range<usize>) -> Cow<'a, Sprite> {
    with_tilemaps(Cow::Borrowed(sprite), frames, true)
}

/// Todos los frames, incluidas las capas ocultas (sus píxeles se guardan como los de cualquier
/// capa oculta). `None` si no hay nada que hornear.
pub fn bake_layers(doc: &Document, sprite: &Sprite, bake: Bake) -> Option<Sprite> {
    let all = 0..sprite.frames.len();
    let baked = materialize_root(doc, sprite, all.clone(), false);
    let baked = match bake {
        Bake::Everything => with_tilemaps(baked, all, false),
        Bake::KeepTilemaps => baked,
    };
    match baked {
        Cow::Borrowed(_) => None,
        Cow::Owned(s) => Some(s),
    }
}
```

`materialize_root` y `materialize_at` quedan igual. En `instance_image`, reemplazar la línea

```rust
    let pixels = render_frame(&materialize_at(doc, source, sf..sf + 1, stack, true), sf);
```

por

```rust
    // El símbolo puede tener tilemaps: también se convierten antes de componerlo.
    let inner = with_tilemaps(
        materialize_at(doc, source, sf..sf + 1, stack, true),
        sf..sf + 1,
        true,
    );
    let pixels = render_frame(&inner, sf);
```

Debajo de `instance_image` (antes de `#[cfg(test)]`):

```rust
/// `sprite` con las capas tilemap convertidas, o el mismo si no tiene.
fn with_tilemaps<'a>(
    sprite: Cow<'a, Sprite>,
    frames: Range<usize>,
    skip_hidden: bool,
) -> Cow<'a, Sprite> {
    match tilemaps_baked(&sprite, frames, skip_hidden) {
        Some(baked) => Cow::Owned(baked),
        None => sprite,
    }
}

/// Copia con las capas tilemap convertidas en capas de imagen: un cel por frame de `frames` con
/// la grilla dibujada. `None` si no hay capas tilemap. Con `skip_hidden`, las ocultas quedan sin
/// cels. La copia ya no tiene tilesets ni grillas.
fn tilemaps_baked(sprite: &Sprite, frames: Range<usize>, skip_hidden: bool) -> Option<Sprite> {
    if !sprite.layers.iter().any(|l| l.kind == LayerKind::Tilemap) {
        return None;
    }
    let mut out = sprite.clone();
    let mut next = sprite.next_image_id().0;
    // Los cels vinculados (misma grilla, mismo tileset) siguen compartiendo imagen.
    let mut done: HashMap<(ImageId, u32), ImageId> = HashMap::new();
    for layer in &mut out.layers {
        if layer.kind != LayerKind::Tilemap {
            continue;
        }
        layer.kind = LayerKind::Image;
        let index = layer.tileset.take();
        let tileset = index.and_then(|i| sprite.tilesets.get(i as usize));
        let draw = !skip_hidden || layer.visible;
        for frame in 0..sprite.frames.len() {
            let Some(cel) = out.cels.remove(&(layer.id, frame)) else {
                continue;
            };
            if !draw || !frames.contains(&frame) {
                continue;
            }
            let (Some(i), Some(tileset), Some(grid)) = (index, tileset, sprite.tilemap(cel.image))
            else {
                continue;
            };
            let key = (cel.image, i);
            let image = match done.get(&key) {
                Some(&id) => id,
                None => {
                    let id = ImageId(next);
                    next += 1;
                    let pixels = tilemap_image(sprite.color_mode, tileset, grid);
                    out.images.insert(id, Arc::new(pixels));
                    done.insert(key, id);
                    id
                }
            };
            out.cels.insert((layer.id, frame), Cel { image, ..cel });
        }
    }
    out.tilesets.clear();
    out.tilemaps.clear();
    Some(out)
}

/// Imagen de una grilla: cada tile se copia como valores (sin mezclar), con sus flips.
fn tilemap_image(mode: ColorMode, tileset: &Tileset, grid: &TileGrid) -> Image {
    let (tw, th) = tileset.tile_size;
    let mut image = Image::new(grid.width * tw, grid.height * th, mode);
    for gy in 0..grid.height {
        for gx in 0..grid.width {
            let Some(tile) = grid.get(gx, gy) else {
                continue;
            };
            let Some(src) = tile_image(tileset, tile) else {
                continue;
            };
            for y in 0..th {
                for x in 0..tw {
                    let (sx, sy) = tile_source(tile, tw, th, x, y);
                    if let Some(v) = src.get(sx, sy) {
                        // Un tile de otro modo de color (datos corruptos) no se dibuja.
                        let _ = image.set(gx * tw + x, gy * th + y, v);
                    }
                }
            }
        }
    }
    image
}

/// Avisado una sola vez: flip D en tiles no cuadrados.
static WARNED_DIAGONAL: AtomicBool = AtomicBool::new(false);

/// Imagen del tile, o `None` si la celda no dibuja nada: tile vacío, índice fuera de rango, imagen
/// de otro tamaño o flip D en un tile no cuadrado.
fn tile_image(tileset: &Tileset, tile: Tile) -> Option<&Image> {
    let index = tile.index() as usize;
    if index == 0 {
        return None;
    }
    let image = tileset.tiles.get(index)?;
    let (tw, th) = tileset.tile_size;
    if (image.width(), image.height()) != (tw, th) {
        return None;
    }
    if tile.flip_d() && tw != th {
        if !WARNED_DIAGONAL.swap(true, Ordering::Relaxed) {
            log::warn!("diagonal flip on non-square tiles ({tw}×{th}) is not drawn");
        }
        return None;
    }
    Some(image)
}

/// Píxel del tile original que se ve en `(x, y)` del tile volteado. Equivale a aplicar a la imagen
/// D (transponer), después X y después Y; el orden lo confirma `tilemap_file_renders_like_aseprite`
/// en `cs-io/tests/aseprite_interop.rs`.
fn tile_source(tile: Tile, w: u32, h: u32, x: u32, y: u32) -> (u32, u32) {
    let mut sx = if tile.flip_x() { w - 1 - x } else { x };
    let mut sy = if tile.flip_y() { h - 1 - y } else { y };
    if tile.flip_d() {
        std::mem::swap(&mut sx, &mut sy);
    }
    (sx, sy)
}
```

Callers de `bake_instances` (todas las líneas son `cs_render::bake_instances(doc, sprite)`):

- `crates/cs-io/src/files.rs:86`: `cs_render::bake_layers(doc, &only.sprite, cs_render::Bake::KeepTilemaps)`;
  el comentario de la rama pasa a "`.aseprite` no tiene instancias, máscaras ni recortes: se aplican
  a los píxeles. Los tilemaps se guardan como tilemaps."
- `crates/cs-io/src/chimp.rs:158`: `cs_render::bake_layers(doc, &artboard.sprite, cs_render::Bake::KeepTilemaps)`.
- `crates/cs-io/src/export.rs:464` (`export_atlas`): `cs_render::bake_layers(doc, &artboard.sprite, cs_render::Bake::Everything)`.
- `crates/cs-agent/src/ops/view.rs:141` (`export`): `cs_render::bake_layers(&self.doc(&p.doc)?.document, sprite, cs_render::Bake::Everything)`;
  comentario: "Las instancias y los tilemaps se hornean en capas de imagen antes de exportar."
- `crates/cs-ui/src/export_dialog.rs:142` y `:174`: `cs_render::bake_layers(&editor.document, editor.sprite(), cs_render::Bake::Everything)`;
  comentario de la 142: "Las instancias y los tilemaps se hornean en capas normales para exportar."

Verificar que no quedan usos: `grep -rn "bake_instances\|mod instance" crates apps` solo debe
encontrar `bakes_instances` en `crates/cs-io/src/files.rs` y sus tests (esa función no cambia).

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo test -p cs-render && cargo check --workspace --all-targets`
Expected: PASS y el workspace compila. Si `tilemap_cels_keep_position_and_opacity` falla solo por el
rango de alpha, ajustar el rango al valor real de `mul(mul(255, 128), 128)` (≈ 64), no la
implementación.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-render crates/cs-io/src crates/cs-agent/src/ops/view.rs crates/cs-ui/src/export_dialog.rs
git commit -m "feat(render): materialize tilemap layers; bake_instances becomes bake_layers" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Comandos de tilemaps en `cs-commands`

**Files:**
- Modify: `crates/cs-commands/src/command.rs` (imports 4-7, `CmdError` 10-46, `enum Command` 117-122, `apply` 199-213, `remove_layer` 373-390, tests)
- Modify: `crates/cs-commands/src/ops.rs` (imports 6-9, después de `new_instance_layer`; `duplicate_frame` 138-167, `unlink_range` 282-308, `unlink_cel` 311-332; tests)

**Interfaces:**
- Consumes: `Tile`, `TileGrid`, `Tileset`, `Sprite::tilemaps`, `Sprite::tilemap`, `LayerKind::Tilemap` (Task 1).
- Produces:
  - `Command::SetTileset { index: usize, tileset: Option<Tileset> }`: `Some` con `index == len`
    agrega; `Some` con `index < len` reemplaza; `None` quita, solo el último (`index == len - 1`,
    si no `IndexOutOfRange`) y solo si ninguna capa lo usa (`TilesetInUse`). Inverso: el estado
    anterior.
  - `Command::SetTilemap { id: ImageId, grid: Option<Arc<TileGrid>> }` (como `SetImage`).
  - `CmdError::TilesetInUse(usize)`.
  - `Command::RemoveLayer` también quita de `images` y `tilemaps` los ids que usaban sus cels y ya
    no usa ningún cel; el inverso los restaura.
  - `ops::new_tilemap_layer(sprite: &Sprite, above: Option<LayerId>, tileset: u32) -> Command` (un `AddLayer`).
  - `ops::set_tiles(sprite: &Sprite, layer: LayerId, frame: usize, cells: &[(i32, i32, Tile)]) -> Option<Command>`:
    celdas de la grilla del lienzo (la celda `(x, y)` empieza en el píxel `(x·tw, y·th)`, corrida
    por el resto de la posición del cel); devuelve `Batch[SetTilemap, SetCel]`.
  - `ops::set_tile_image(sprite: &Sprite, tileset: usize, index: usize, image: Image) -> Option<Command>`.
  - `duplicate_frame`, `unlink_range` y `unlink_cel` copian la grilla de los cels tilemap.

- [ ] **Step 1: Escribir los tests que fallan**

En los tests de `crates/cs-commands/src/command.rs`, agregar `use std::sync::Arc;` al principio del
módulo y:

```rust
    fn add_tileset(s: &mut Sprite, index: usize, name: &str) -> Command {
        Command::SetTileset {
            index,
            tileset: Some(Tileset::new(name, (4, 4), ColorMode::Rgba)),
        }
        .apply(s)
        .unwrap()
    }

    #[test]
    fn set_tileset_adds_replaces_and_removes_with_undo() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        let empty = s.clone();
        let inv = add_tileset(&mut s, 0, "a");
        assert_eq!(s.tilesets.len(), 1);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, empty);
        add_tileset(&mut s, 0, "a");
        add_tileset(&mut s, 1, "b");
        let two = s.clone();
        let inv = add_tileset(&mut s, 0, "c");
        assert_eq!(s.tilesets[0].name, "c");
        inv.apply(&mut s).unwrap();
        assert_eq!(s, two);
        assert_eq!(
            Command::SetTileset {
                index: 5,
                tileset: Some(Tileset::new("x", (4, 4), ColorMode::Rgba)),
            }
            .apply(&mut s),
            Err(CmdError::IndexOutOfRange(5))
        );
        // Quitar: solo el último y solo si ninguna capa lo usa.
        assert_eq!(
            Command::SetTileset {
                index: 0,
                tileset: None
            }
            .apply(&mut s),
            Err(CmdError::IndexOutOfRange(0))
        );
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(1);
        assert_eq!(
            Command::SetTileset {
                index: 1,
                tileset: None
            }
            .apply(&mut s),
            Err(CmdError::TilesetInUse(1))
        );
        s.layers[0].tileset = Some(0);
        let before = s.clone();
        let inv = Command::SetTileset {
            index: 1,
            tileset: None,
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(s.tilesets.len(), 1);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn set_tilemap_roundtrip() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        let before = s.clone();
        let grid = Arc::new(TileGrid::new(2, 2));
        let inv = Command::SetTilemap {
            id: ImageId(3),
            grid: Some(grid.clone()),
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(s.tilemap(ImageId(3)), Some(&grid));
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn removing_a_layer_frees_unused_images_and_grids() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        dot(1).apply(&mut s).unwrap();
        let mut layer = Layer::new_image(LayerId(2), "map");
        layer.kind = LayerKind::Tilemap;
        layer.tileset = Some(0);
        s.layers.push(layer);
        s.tilemaps
            .insert(ImageId(5), Arc::new(TileGrid::new(1, 1)));
        s.cels.insert(
            (LayerId(2), 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: ImageId(5),
            },
        );
        let before = s.clone();
        let inv = Command::RemoveLayer { id: LayerId(2) }
            .apply(&mut s)
            .unwrap();
        assert!(s.tilemaps.is_empty());
        assert_eq!(s.images.len(), 1, "the other layer's image stays");
        let inv1 = Command::RemoveLayer { id: LayerId(1) }
            .apply(&mut s)
            .unwrap();
        assert!(s.images.is_empty());
        inv1.apply(&mut s).unwrap();
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn resize_moves_tilemap_cels_and_trim_ignores_them() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        s.layers[0].kind = LayerKind::Tilemap;
        s.tilemaps
            .insert(ImageId(1), Arc::new(TileGrid::new(4, 4)));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: ImageId(1),
            },
        );
        Command::ResizeCanvas {
            width: 4,
            height: 4,
            offset: (-2, -2),
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(s.cel(LayerId(1), 0).unwrap().position, (-2, -2));
        let before = s.clone();
        Command::TrimToCanvas.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }
```

En `commands_serialize`, agregar al arreglo `cmds`:

```rust
            Command::SetTileset {
                index: 0,
                tileset: Some(Tileset::new("t", (8, 8), ColorMode::Rgba)),
            },
            Command::SetTilemap {
                id: ImageId(4),
                grid: Some(Arc::new(TileGrid::new(2, 1))),
            },
```

En los tests de `crates/cs-commands/src/ops.rs`:

```rust
    /// Sprite de 16×16 con un tileset de 4×4 (dos tiles además del vacío) y una capa tilemap
    /// (id 2) encima de la capa 1.
    fn with_tilemap_layer() -> Sprite {
        let mut s = Sprite::new(16, 16, ColorMode::Rgba);
        let mut ts = Tileset::new("t", (4, 4), ColorMode::Rgba);
        for _ in 0..2 {
            ts.tiles
                .push(Arc::new(Image::new(4, 4, ColorMode::Rgba)));
        }
        crate::Command::SetTileset {
            index: 0,
            tileset: Some(ts),
        }
        .apply(&mut s)
        .unwrap();
        new_tilemap_layer(&s.clone(), Some(LayerId(1)), 0)
            .apply(&mut s)
            .unwrap();
        s
    }

    fn grid_of(s: &Sprite, layer: u32, frame: usize) -> (Cel, TileGrid) {
        let cel = *s.cel(LayerId(layer), frame).unwrap();
        (cel, TileGrid::clone(s.tilemap(cel.image).unwrap()))
    }

    #[test]
    fn new_tilemap_layer_uses_the_tileset() {
        let s = with_tilemap_layer();
        let layer = s.layer(LayerId(2)).unwrap();
        assert_eq!(
            (layer.kind, layer.tileset, layer.name.as_str()),
            (LayerKind::Tilemap, Some(0), "Tilemap 2")
        );
    }

    #[test]
    fn set_tiles_creates_grows_and_unlinks() {
        let mut s = with_tilemap_layer();
        let t = |i| Tile::new(i, false, false, false);
        // Sin cel: se crea en (0, 0) con la grilla mínima que llega a la celda (2, 1).
        set_tiles(&s, LayerId(2), 0, &[(2, 1, t(1))])
            .unwrap()
            .apply(&mut s)
            .unwrap();
        let (cel, grid) = grid_of(&s, 2, 0);
        assert_eq!((cel.position, grid.width, grid.height), ((0, 0), 3, 2));
        assert_eq!(grid.get(2, 1), Some(t(1)));
        // Hacia la izquierda y hacia arriba: la grilla crece y el cel se corre.
        let before = s.clone();
        let inv = set_tiles(&s, LayerId(2), 0, &[(-1, -2, t(2))])
            .unwrap()
            .apply(&mut s)
            .unwrap();
        let (cel, grid) = grid_of(&s, 2, 0);
        assert_eq!((cel.position, grid.width, grid.height), ((-4, -8), 4, 4));
        assert_eq!(grid.get(0, 0), Some(t(2)));
        assert_eq!(grid.get(3, 3), Some(t(1)), "the old tile moved with the grid");
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        // Si nada cambia, no hay comando.
        assert!(set_tiles(&s, LayerId(2), 0, &[(2, 1, t(1))]).is_none());
        // Vinculado: solo el cel editado recibe una grilla nueva.
        crate::Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
        link_cel(&s, LayerId(2), 1, 0)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        set_tiles(&s, LayerId(2), 1, &[(0, 0, t(2))])
            .unwrap()
            .apply(&mut s)
            .unwrap();
        let ((c0, g0), (c1, g1)) = (grid_of(&s, 2, 0), grid_of(&s, 2, 1));
        assert_ne!(c0.image, c1.image);
        assert_eq!((g0.get(0, 0), g1.get(0, 0)), (Some(Tile::EMPTY), Some(t(2))));
        // En una capa de imagen, o sin celdas, no hace nada.
        assert!(set_tiles(&s, LayerId(1), 0, &[(0, 0, t(1))]).is_none());
        assert!(set_tiles(&s, LayerId(2), 0, &[]).is_none());
    }

    #[test]
    fn set_tile_image_adds_and_replaces_but_not_tile_zero() {
        let mut s = with_tilemap_layer();
        let mut img = Image::new(4, 4, ColorMode::Rgba);
        img.set(1, 1, PixelValue::Rgba(Rgba::rgb(9, 9, 9))).unwrap();
        let before = s.clone();
        let inv = set_tile_image(&s, 0, 1, img.clone())
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(*s.tilesets[0].tiles[1], img);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        set_tile_image(&s, 0, 3, img.clone())
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(s.tilesets[0].tiles.len(), 4, "index == len adds a tile");
        assert!(set_tile_image(&s, 0, 0, img.clone()).is_none(), "tile 0 is the empty tile");
        assert!(set_tile_image(&s, 0, 9, img.clone()).is_none(), "out of range");
        assert!(set_tile_image(&s, 1, 1, img.clone()).is_none(), "no tileset 1");
        assert!(
            set_tile_image(&s, 0, 3, img).is_none(),
            "same image: nothing changes"
        );
        let small = Image::new(2, 2, ColorMode::Rgba);
        assert!(set_tile_image(&s, 0, 1, small).is_none(), "wrong size");
    }

    #[test]
    fn duplicating_and_unlinking_tilemap_cels_copies_the_grid() {
        let mut s = with_tilemap_layer();
        set_tiles(&s, LayerId(2), 0, &[(0, 0, Tile::new(1, false, false, false))])
            .unwrap()
            .apply(&mut s)
            .unwrap();
        duplicate_frame(&s.clone(), 0, false)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        let ((c0, g0), (c1, g1)) = (grid_of(&s, 2, 0), grid_of(&s, 2, 1));
        assert_ne!(c0.image, c1.image);
        assert_eq!(g0, g1);
        link_cel(&s, LayerId(2), 1, 0)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        unlink_cel(&s, LayerId(2), 1)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        let cel = *s.cel(LayerId(2), 1).unwrap();
        assert_ne!(cel.image, c0.image);
        assert!(s.tilemap(cel.image).is_some());
    }
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-commands`
Expected: error de compilación (`SetTileset`, `set_tiles`… no existen).

- [ ] **Step 3: Implementar**

En `crates/cs-commands/src/command.rs`:

1. Agregar `TileGrid, Tileset` al `use cs_core::{…}`.
2. En `CmdError`, al final:

```rust
    #[error("tileset {0} is used by a layer")]
    TilesetInUse(usize),
```

3. En `enum Command`, después de `SetInstanceTrack`:

```rust
    /// Primitiva: agrega (`index == len`), reemplaza (`index < len`) o quita (`None`) un tileset.
    /// Solo se quita el último (quitar otro correría los índices de las capas) y solo si ninguna
    /// capa lo usa.
    SetTileset {
        index: usize,
        tileset: Option<Tileset>,
    },
    /// Primitiva: pone (o quita, con `None`) una grilla de tiles del pool.
    SetTilemap {
        id: ImageId,
        grid: Option<Arc<TileGrid>>,
    },
```

4. En `Command::apply`, después de la rama de `SetInstanceTrack`:

```rust
            Command::SetTileset { index, tileset } => set_tileset(doc, *index, tileset.as_ref()),
            Command::SetTilemap { id, grid } => {
                let old = match grid {
                    Some(g) => doc.tilemaps.insert(*id, g.clone()),
                    None => doc.tilemaps.remove(id),
                };
                Ok(Command::SetTilemap { id: *id, grid: old })
            }
```

5. Debajo de `set_cel`:

```rust
fn set_tileset(
    doc: &mut Sprite,
    index: usize,
    tileset: Option<&Tileset>,
) -> Result<Command, CmdError> {
    let len = doc.tilesets.len();
    match tileset {
        Some(t) if index < len => {
            let old = std::mem::replace(&mut doc.tilesets[index], t.clone());
            Ok(Command::SetTileset {
                index,
                tileset: Some(old),
            })
        }
        Some(t) if index == len => {
            doc.tilesets.push(t.clone());
            Ok(Command::SetTileset {
                index,
                tileset: None,
            })
        }
        Some(_) => Err(CmdError::IndexOutOfRange(index)),
        None => {
            if index + 1 != len {
                return Err(CmdError::IndexOutOfRange(index));
            }
            if doc.layers.iter().any(|l| l.tileset == Some(index as u32)) {
                return Err(CmdError::TilesetInUse(index));
            }
            Ok(Command::SetTileset {
                index,
                tileset: doc.tilesets.pop(),
            })
        }
    }
}
```

6. Reemplazar `remove_layer` por:

```rust
fn remove_layer(doc: &mut Sprite, id: LayerId) -> Result<Command, CmdError> {
    let index = doc.layer_index(id).ok_or(CmdError::LayerNotFound(id))?;
    if doc.layers.iter().any(|l| l.parent == Some(id)) {
        return Err(CmdError::LayerHasChildren(id));
    }
    let layer = doc.layers.remove(index);
    let keys: Vec<_> = doc.cels.keys().filter(|(l, _)| *l == id).copied().collect();
    let mut inverse = vec![Command::AddLayer { index, layer }];
    let mut freed = Vec::new();
    for key in keys {
        let cel = doc.cels.remove(&key);
        freed.extend(cel.map(|c| c.image));
        inverse.push(Command::SetCel {
            layer: key.0,
            frame: key.1,
            cel,
        });
    }
    // Las imágenes y grillas que ya no usa ningún cel salen del pool (el inverso las restaura).
    freed.sort();
    freed.dedup();
    for image in freed {
        if doc.cels.values().any(|c| c.image == image) {
            continue;
        }
        if let Some(old) = doc.images.remove(&image) {
            inverse.push(Command::SetImage {
                id: image,
                image: Some(old),
            });
        }
        if let Some(old) = doc.tilemaps.remove(&image) {
            inverse.push(Command::SetTilemap {
                id: image,
                grid: Some(old),
            });
        }
    }
    Ok(Command::Batch(inverse))
}
```

`ResizeCanvas` no cambia (ya corre todos los cels). `TrimToCanvas` no cambia: los cels tilemap no
tienen entrada en `images` y el `let Some(image) = doc.images.get(…) else { continue }` ya los salta;
lo verifica `resize_moves_tilemap_cels_and_trim_ignores_them`.

En `crates/cs-commands/src/ops.rs`:

1. Imports: `use cs_core::{ArtboardId, Cel, ColorMode, Image, ImageId, InstanceKey, Layer, LayerId, LayerKind, LayerMask, Mask, PixelBuffer, PixelValue, Sprite, Tile, TileGrid};`.
2. Después de `remove_instance_key`:

```rust
/// Capa tilemap nueva sobre `above` que dibuja con el tileset `tileset`.
pub fn new_tilemap_layer(sprite: &Sprite, above: Option<LayerId>, tileset: u32) -> Command {
    let Command::AddLayer { index, mut layer } = new_layer(sprite, above, LayerKind::Tilemap)
    else {
        unreachable!("new_layer devuelve AddLayer");
    };
    layer.tileset = Some(tileset);
    Command::AddLayer { index, layer }
}

/// Pone tiles en el cel tilemap `(layer, frame)`. Las coordenadas son celdas de la grilla del
/// lienzo: la celda `(x, y)` empieza en el píxel `(x · ancho_tile, y · alto_tile)` (corrida por el
/// resto de la posición del cel si no está alineado). Sin cel, crea uno en (0, 0). Si las celdas
/// caen fuera de la grilla, la agranda con vacíos y corre la posición del cel si crece hacia la
/// izquierda o hacia arriba. Un cel vinculado recibe una grilla propia. `None` si la capa no es
/// tilemap, su tileset no existe, no hay celdas o nada cambia.
pub fn set_tiles(
    sprite: &Sprite,
    layer: LayerId,
    frame: usize,
    cells: &[(i32, i32, Tile)],
) -> Option<Command> {
    let l = sprite.layer(layer)?;
    if l.kind != LayerKind::Tilemap || cells.is_empty() || frame >= sprite.frames.len() {
        return None;
    }
    let tileset = sprite.tilesets.get(l.tileset? as usize)?;
    let (tw, th) = (tileset.tile_size.0 as i32, tileset.tile_size.1 as i32);
    let old_cel = sprite.cel(layer, frame).copied();
    let old_grid: Option<&TileGrid> = old_cel.and_then(|c| sprite.tilemap(c.image)).map(|g| &**g);
    let (ow, oh) = old_grid.map_or((0, 0), |g| (g.width as i32, g.height as i32));
    let position = old_cel.map_or((0, 0), |c| c.position);
    // Celda del lienzo donde empieza la grilla actual.
    let origin = (position.0.div_euclid(tw), position.1.div_euclid(th));
    let (mut x0, mut y0, mut x1, mut y1) = (0, 0, ow, oh);
    for &(x, y, _) in cells {
        let (rx, ry) = (x - origin.0, y - origin.1);
        (x0, y0, x1, y1) = (x0.min(rx), y0.min(ry), x1.max(rx + 1), y1.max(ry + 1));
    }
    let mut grid = TileGrid::new((x1 - x0) as u32, (y1 - y0) as u32);
    if let Some(old) = old_grid {
        for y in 0..old.height {
            for x in 0..old.width {
                let tile = old.get(x, y).unwrap_or(Tile::EMPTY);
                grid.set((x as i32 - x0) as u32, (y as i32 - y0) as u32, tile);
            }
        }
    }
    for &(x, y, tile) in cells {
        grid.set((x - origin.0 - x0) as u32, (y - origin.1 - y0) as u32, tile);
    }
    let new_position = (position.0 + x0 * tw, position.1 + y0 * th);
    if old_grid == Some(&grid) && new_position == position {
        return None;
    }
    // Un cel vinculado recibe una grilla nueva; si no, se reemplaza la suya.
    let id = match old_cel {
        Some(c) if !is_linked(sprite, layer, frame) => c.image,
        _ => sprite.next_image_id(),
    };
    Some(Command::Batch(vec![
        Command::SetTilemap {
            id,
            grid: Some(Arc::new(grid)),
        },
        Command::SetCel {
            layer,
            frame,
            cel: Some(Cel {
                position: new_position,
                opacity: old_cel.map_or(255, |c| c.opacity),
                image: id,
            }),
        },
    ]))
}

/// Reemplaza la imagen del tile `index` del tileset `tileset`, o la agrega si
/// `index == tiles.len()`. `None` si es el tile 0 (el vacío), el índice o el tileset no existen,
/// la imagen no tiene el tamaño o el modo de color del tileset, o nada cambia.
pub fn set_tile_image(
    sprite: &Sprite,
    tileset: usize,
    index: usize,
    image: Image,
) -> Option<Command> {
    let ts = sprite.tilesets.get(tileset)?;
    if index == 0 || index > ts.tiles.len() || (image.width(), image.height()) != ts.tile_size {
        return None;
    }
    let same_mode = matches!(
        (image.pixels(), sprite.color_mode),
        (PixelBuffer::Rgba(_), ColorMode::Rgba) | (PixelBuffer::Indexed(_), ColorMode::Indexed { .. })
    );
    if !same_mode {
        return None;
    }
    let mut new = ts.clone();
    if index == new.tiles.len() {
        new.tiles.push(Arc::new(image));
    } else {
        new.tiles[index] = Arc::new(image);
    }
    (new != *ts).then_some(Command::SetTileset {
        index: tileset,
        tileset: Some(new),
    })
}

/// Copia del contenido de un cel (imagen o grilla de tiles) con el id `to`.
fn copy_content(sprite: &Sprite, from: ImageId, to: ImageId) -> Command {
    match sprite.tilemap(from) {
        Some(grid) => Command::SetTilemap {
            id: to,
            grid: Some(grid.clone()),
        },
        None => Command::SetImage {
            id: to,
            image: sprite.image(from).cloned(),
        },
    }
}
```

3. En `duplicate_frame`, `unlink_range` y `unlink_cel`, reemplazar cada

```rust
            cmds.push(Command::SetImage {
                id,
                image: sprite.image(cel.image).cloned(),
            });
```

(en `unlink_cel` es el primer elemento del `vec![…]`: `Command::SetImage { id, image: sprite.image(cel.image).cloned() }`)
por `copy_content(sprite, cel.image, id)` (`cmds.push(copy_content(sprite, cel.image, id));` en las
dos primeras).

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo test -p cs-commands && cargo test --workspace`
Expected: PASS (incluidos `undo_roundtrip` y los tests de `cs-ui` que borran capas).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-commands
git commit -m "feat(commands): tileset and tilemap primitives, set_tiles and pool cleanup on layer removal" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Leer y escribir tilemaps en `.aseprite`

**Files:**
- Modify: `crates/cs-io/src/aseprite.rs` (imports 7-10, constantes 23-39, `UserDataTarget` 112-118, `read` 171-301, `read_layer` 370-420, `read_cel` 443-480, `write` 618-655, `layer_chunk` 722-741, `cel_chunk` 811-838, tests 872-)
- Modify: `crates/cs-io/src/files.rs` (tests)

**Interfaces:**
- Consumes: `Tileset`, `TileGrid`, `Tile`, `AllowedFlips`, `RawChunk`, `ops::new_tilemap_layer`, `ops::set_tiles` (Tasks 1, 3); `cs_render::materialize_tilemaps` (Task 2).
- Produces:
  - `aseprite::read` carga tilesets (en `sprite.tilesets`, en orden de id), capas tipo 2 con
    `tileset` (índice en `sprite.tilesets`), cels tipo 3 (grillas en `sprite.tilemaps`) y user data de
    tileset y tiles (`RawChunk { kind: 0x2020, data }`; un chunk vacío, flags 0, es `None`; en
    `tile_user_data` solo se crean posiciones hasta el último `Some`).
  - `aseprite::write` escribe esos chunks como Aseprite.
  - Avisos: tileset externo ("…external file…"), formato viejo de tile vacío, cel con bits por tile
    distintos de 32 ("…N bits per tile…"), capa con tileset inexistente.

Layouts del spec oficial (`ase-file-specs.md`) que se implementan:

```
Tileset Chunk (0x2023)
    DWORD       Tileset ID
    DWORD       Tileset flags
                  1 - Include link to external file
                  2 - Include tiles inside this file
                  4 - Tilemaps using this tileset use tile ID=0 as empty tile
                  8 / 16 / 32 - Auto mode matches X / Y / D(iagonal) flips
    DWORD       Number of tiles
    WORD        Tile Width
    WORD        Tile Height
    SHORT       Base Index
    BYTE[14]    Reserved
    STRING      Name of the tileset
    + If flag 1 is set
      DWORD     ID of the external file
      DWORD     Tileset ID in the external file
    + If flag 2 is set
      DWORD     Data length of the compressed Tileset image
      PIXEL[]   Compressed Tileset image (zlib): (Tile Width) x (Tile Height x Number of Tiles)

Layer Chunk (0x2004), después de STRING Layer name:
    + If layer type = 2
      DWORD     Tileset index
    + If file header flags have bit 4:
      UUID      Layer's universally unique identifier

Cel Chunk (0x2005), cel type 3 (Compressed Tilemap), después del header común:
      WORD      Width in number of tiles
      WORD      Height in number of tiles
      WORD      Bits per tile (at the moment it's always 32-bit per tile)
      DWORD     Bitmask for tile ID (e.g. 0x1fffffff for 32-bit tiles)
      DWORD     Bitmask for X flip
      DWORD     Bitmask for Y flip
      DWORD     Bitmask for diagonal flip (swap X/Y axis)
      BYTE[10]  Reserved
      TILE[]    Row by row, from top to bottom tile by tile, compressed with ZLIB

User Data Chunk (0x2020), caso especial 2:
   After the Tileset chunk, it could be followed by a user data chunk (empty or not) and then all
   the user data chunks of the tiles ordered by tile index, or it could be followed by none user
   data chunk (if the file was created in an older Aseprite version of if no tile has user data).
```

"Number of tiles" incluye el tile 0 (el vacío), que también va en la tira de imágenes. Aseprite
escribe los tilesets en el primer frame, después de la paleta y antes de las capas.

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/cs-io/src/aseprite.rs`, tests: actualizar `assert_equivalent`:

1. Después de `assert_eq!(a.grid, b.grid);` agregar `assert_eq!(a.tilesets, b.tilesets);`.
2. En la tupla de capas, agregar `la.tileset` / `lb.tileset` (como último elemento de cada lado).
3. Dentro de `if let (Some(ca), Some(cb))`, después de comparar imágenes:
   `assert_eq!(a.tilemap(ca.image), b.tilemap(cb.image));`.
4. Reemplazar el loop de render por:

```rust
    let (ma, mb) = (
        cs_render::materialize_tilemaps(a, 0..a.frames.len()),
        cs_render::materialize_tilemaps(b, 0..b.frames.len()),
    );
    for f in 0..a.frames.len() {
        assert_eq!(cs_render::render_frame(&ma, f), cs_render::render_frame(&mb, f));
    }
```

Agregar al módulo de tests:

```rust
    /// Sprite con una capa de imagen y una tilemap (id 2) con un tileset de 4×4 con user data;
    /// 2 frames con el cel tilemap vinculado, flips y la grilla corrida hacia la izquierda.
    fn tilemap_sprite(mode: ColorMode) -> Sprite {
        let mut s = Sprite::new(16, 12, mode);
        let value = |i: u8| match mode {
            ColorMode::Rgba => PixelValue::Rgba(Rgba::new(i * 30, 200 - i * 20, i * 7, 255)),
            ColorMode::Indexed { .. } => PixelValue::Index(i + 1),
        };
        let mut ts = Tileset::new("terrain ✓", (4, 4), mode);
        for i in 1..=3u8 {
            let mut tile = Image::new(4, 4, mode);
            tile.set(0, 0, value(i)).unwrap();
            tile.set(3, 1, value(i + 1)).unwrap();
            ts.tiles.push(Arc::new(tile));
        }
        ts.base_index = 0;
        ts.allowed_flips = AllowedFlips {
            x: true,
            y: false,
            d: true,
        };
        // Texto "grass" y color (flags 1 | 2).
        let mut ud = Vec::new();
        ud.w_u32(3);
        ud.w_string("grass");
        ud.extend_from_slice(&[1, 2, 3, 255]);
        ts.user_data = Some(RawChunk {
            kind: CHUNK_USER_DATA,
            data: ud,
        });
        let mut tile_ud = Vec::new();
        tile_ud.w_u32(1);
        tile_ud.w_string("solid");
        ts.tile_user_data = vec![
            None,
            None,
            Some(RawChunk {
                kind: CHUNK_USER_DATA,
                data: tile_ud,
            }),
        ];
        Command::SetTileset {
            index: 0,
            tileset: Some(ts),
        }
        .apply(&mut s)
        .unwrap();
        ops::new_tilemap_layer(&s.clone(), Some(LayerId(1)), 0)
            .apply(&mut s)
            .unwrap();
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, value(4))],
        }
        .apply(&mut s)
        .unwrap();
        let cells = [
            (0, 0, Tile::new(1, false, false, false)),
            (1, 0, Tile::new(2, true, false, false)),
            (2, 1, Tile::new(3, false, true, true)),
            (3, 2, Tile::new(1, true, true, false)),
            (-1, 0, Tile::new(2, false, false, false)),
        ];
        ops::set_tiles(&s, LayerId(2), 0, &cells)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        ops::duplicate_frame(&s.clone(), 0, true)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        s
    }

    #[test]
    fn tilemaps_roundtrip_in_both_modes() {
        for mode in [
            ColorMode::Rgba,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ] {
            let s = tilemap_sprite(mode);
            assert_eq!(s.cel(LayerId(2), 0).unwrap().position, (-4, 0));
            let back = roundtrip(&s);
            assert_equivalent(&s, &back);
            assert_eq!(back.layers[1].kind, LayerKind::Tilemap);
            assert_eq!(back.layers[1].tileset, Some(0));
            assert!(!linked_pairs(&back).is_empty(), "linked tilemap cels");
            let rendered = cs_render::render_frame(&cs_render::materialize_tilemaps(&back, 0..1), 0);
            assert!(rendered.iter().any(|c| c.a > 0));
        }
    }

    #[test]
    fn tilemap_write_is_a_fixpoint() {
        let once = write(&roundtrip(&tilemap_sprite(ColorMode::Rgba)));
        let (again, _) = read(&once).unwrap();
        assert_eq!(write(&again), once);
    }

    #[test]
    fn corrupted_tilemap_files_never_panic() {
        let bytes = write(&tilemap_sprite(ColorMode::Rgba));
        for len in 0..bytes.len() {
            let _ = read(&bytes[..len]);
        }
        for i in 0..bytes.len() {
            let mut b = bytes.clone();
            b[i] ^= 0xA5;
            let _ = read(&b);
        }
    }

    /// `.aseprite` de 8×8 y un frame con los chunks dados.
    fn file_with(chunks: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        write_header(&mut out, &Sprite::new(8, 8, ColorMode::Rgba));
        write_frame(&mut out, 100, chunks);
        out
    }

    fn tilemap_layer_chunk() -> Vec<u8> {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        layer_chunk(&s, &s.layers[0])
    }

    fn tileset_header(flags: u32, count: u32, size: u16, name: &str) -> Vec<u8> {
        let mut d = Vec::new();
        d.w_u32(0);
        d.w_u32(flags);
        d.w_u32(count);
        d.w_u16(size);
        d.w_u16(size);
        d.w_i16(1);
        d.w_zeros(14);
        d.w_string(name);
        d
    }

    #[test]
    fn external_tilesets_load_empty_with_a_warning() {
        let mut ts = tileset_header(TILESET_EXTERNAL | TILESET_EMPTY_ZERO, 3, 4, "outside");
        ts.w_u32(1);
        ts.w_u32(0);
        let (s, report) = read(&file_with(&[
            (CHUNK_TILESET, ts),
            (CHUNK_LAYER, tilemap_layer_chunk()),
        ]))
        .unwrap();
        assert_eq!(s.tilesets.len(), 1);
        assert_eq!(s.tilesets[0].tiles.len(), 1, "only the empty tile");
        assert_eq!(s.layers[0].kind, LayerKind::Tilemap);
        assert_eq!(s.layers[0].tileset, Some(0));
        assert!(
            report.warnings.iter().any(|w| w.contains("external")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn tilemap_cels_that_are_not_32_bit_are_skipped_with_a_warning() {
        let mut ts = tileset_header(TILESET_EMBEDDED | TILESET_EMPTY_ZERO, 1, 2, "t");
        let pixels = deflate(&[0; 16]);
        ts.w_u32(pixels.len() as u32);
        ts.extend_from_slice(&pixels);
        let mut cel = Vec::new();
        cel.w_u16(0);
        cel.w_i16(0);
        cel.w_i16(0);
        cel.w_u8(255);
        cel.w_u16(CEL_TILEMAP);
        cel.w_i16(0);
        cel.w_zeros(5);
        cel.w_u16(1);
        cel.w_u16(1);
        cel.w_u16(16);
        cel.w_u32(0x1fff);
        cel.w_u32(0x2000);
        cel.w_u32(0x4000);
        cel.w_u32(0x8000);
        cel.w_zeros(10);
        cel.extend_from_slice(&deflate(&[1, 0]));
        let (s, report) = read(&file_with(&[
            (CHUNK_TILESET, ts),
            (CHUNK_LAYER, tilemap_layer_chunk()),
            (CHUNK_CEL, cel),
        ]))
        .unwrap();
        assert!(s.cels.is_empty());
        assert!(
            report.warnings.iter().any(|w| w.contains("16 bits")),
            "{:?}",
            report.warnings
        );
    }
```

(`Arc` llega por `use super::*` desde `use std::sync::Arc;` del módulo; `Tileset`, `Tile`,
`AllowedFlips`, `RawChunk`, `Image` por `use cs_core::*;`.)

En los tests de `crates/cs-io/src/files.rs`:

```rust
    #[test]
    fn saving_aseprite_and_chimp_keeps_tilemaps() {
        use cs_core::{LayerId, LayerKind, Tile};
        let mut sprite = Sprite::new(8, 8, ColorMode::Rgba);
        let mut ts = cs_core::Tileset::new("t", (4, 4), ColorMode::Rgba);
        ts.tiles.push(std::sync::Arc::new(cs_core::Image::new(
            4,
            4,
            ColorMode::Rgba,
        )));
        cs_commands::Command::SetTileset {
            index: 0,
            tileset: Some(ts),
        }
        .apply(&mut sprite)
        .unwrap();
        cs_commands::ops::new_tilemap_layer(&sprite.clone(), None, 0)
            .apply(&mut sprite)
            .unwrap();
        let flipped = Tile::new(1, true, false, false);
        cs_commands::ops::set_tiles(&sprite, LayerId(2), 0, &[(1, 1, flipped)])
            .unwrap()
            .apply(&mut sprite)
            .unwrap();
        let d = Document::with_sprite("map", sprite);
        for name in ["x.aseprite", "x.chimp"] {
            let (back, report) = decode(name, &encode(&d, name).unwrap()).unwrap();
            assert!(report.warnings.is_empty(), "{name}: {:?}", report.warnings);
            let s = &back.artboards[0].sprite;
            assert_eq!(s.layers[1].kind, LayerKind::Tilemap, "{name}");
            assert_eq!(s.tilesets, d.artboards[0].sprite.tilesets, "{name}");
            let cel = s.cel(LayerId(2), 0).unwrap();
            assert_eq!(s.tilemap(cel.image).unwrap().get(1, 1), Some(flipped), "{name}");
        }
    }
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-io --lib`
Expected: error de compilación (`CHUNK_TILESET`, `TILESET_EXTERNAL`… no existen); después de
agregar solo las constantes, fallan los round-trips (las capas tilemap se leen como imagen).

- [ ] **Step 3: Implementar**

En `crates/cs-io/src/aseprite.rs`:

1. Imports:

```rust
use cs_core::{
    AllowedFlips, AnimDirection, BlendMode, Cel, ColorMode, Frame, Image, Layer, LayerId,
    LayerKind, Palette, PixelBuffer, RawChunk, Rgba, Slice, SliceKey, Sprite, Tag, Tile, TileGrid,
    Tileset,
};
```

2. Constantes, junto a las de chunks y cels:

```rust
const CHUNK_TILESET: u16 = 0x2023;

const TILESET_EXTERNAL: u32 = 1;
const TILESET_EMBEDDED: u32 = 2;
const TILESET_EMPTY_ZERO: u32 = 4;
const TILESET_FLIP_X: u32 = 8;
const TILESET_FLIP_Y: u32 = 16;
const TILESET_FLIP_D: u32 = 32;
```

3. `UserDataTarget`, nueva variante:

```rust
    /// Tras un tileset (índice en la lista que se va leyendo): `next` 0 es el tileset; `i + 1`,
    /// su tile `i`.
    Tileset { index: usize, next: usize },
```

4. En `read`, junto a `let mut user_data = UserDataTarget::None;`:

```rust
    // Tilesets con su id del archivo; al final se ordenan por id.
    let mut tilesets: Vec<(u32, Tileset)> = Vec::new();
```

5. En el `match kind` de chunks, antes de `CHUNK_COLOR_PROFILE => {}`:

```rust
                CHUNK_TILESET => {
                    tilesets.push(read_tileset(&mut c, depth, mode, &mut report)?);
                    user_data = UserDataTarget::Tileset {
                        index: tilesets.len() - 1,
                        next: 0,
                    };
                }
```

   y en `CHUNK_USER_DATA => match &mut user_data {`, antes de `UserDataTarget::None`:

```rust
                    UserDataTarget::Tileset { index, next } => {
                        let data = c.rest().to_vec();
                        // Un chunk vacío (flags 0) es lo mismo que no tener user data.
                        let chunk = (data.get(..4) != Some(&[0, 0, 0, 0][..])).then_some(RawChunk {
                            kind: CHUNK_USER_DATA,
                            data,
                        });
                        let tileset = &mut tilesets[*index].1;
                        match *next {
                            0 => tileset.user_data = chunk,
                            n if n <= tileset.tiles.len() => {
                                if let Some(chunk) = chunk {
                                    let i = n - 1;
                                    if tileset.tile_user_data.len() <= i {
                                        tileset.tile_user_data.resize(i + 1, None);
                                    }
                                    tileset.tile_user_data[i] = Some(chunk);
                                }
                            }
                            _ => {}
                        }
                        *next += 1;
                    }
```

6. Al final de `read`, antes de `Ok((sprite, report))`:

```rust
    // Los tilesets quedan en el orden de su id; las capas pasan de id a índice.
    tilesets.sort_by_key(|(id, _)| *id);
    let ids: Vec<u32> = tilesets.iter().map(|(id, _)| *id).collect();
    for layer in &mut sprite.layers {
        let Some(id) = layer.tileset else {
            continue;
        };
        match ids.iter().position(|&t| t == id) {
            Some(i) => layer.tileset = Some(i as u32),
            None => report.warn(format!(
                "tilemap layer '{}' uses missing tileset {id}",
                layer.name
            )),
        }
    }
    sprite.tilesets = tilesets.into_iter().map(|(_, t)| t).collect();
```

7. Debajo de `read_old_palette`:

```rust
fn read_tileset(
    c: &mut Reader,
    depth: Depth,
    mode: ColorMode,
    report: &mut LoadReport,
) -> Result<(u32, Tileset)> {
    let id = c.u32()?;
    let flags = c.u32()?;
    let count = c.u32()?;
    let tw = c.u16()? as u32;
    let th = c.u16()? as u32;
    let base_index = c.i16()?;
    c.skip(14)?;
    let name = c.string()?;
    if tw == 0 || th == 0 {
        return Err(IoError::Corrupt(format!(
            "tileset '{name}' has {tw}×{th} tiles"
        )));
    }
    let mut tileset = Tileset::new(name, (tw, th), mode);
    tileset.base_index = base_index;
    tileset.allowed_flips = AllowedFlips {
        x: flags & TILESET_FLIP_X != 0,
        y: flags & TILESET_FLIP_Y != 0,
        d: flags & TILESET_FLIP_D != 0,
    };
    if flags & TILESET_EMPTY_ZERO == 0 {
        report.warn(format!(
            "tileset '{}' uses an old empty-tile format; tile 0 is treated as empty",
            tileset.name
        ));
    }
    if flags & TILESET_EXTERNAL != 0 {
        c.skip(8)?;
    }
    if flags & TILESET_EMBEDDED != 0 {
        let len = c.u32()? as usize;
        let too_big = || IoError::Corrupt(format!("tileset '{}' is too large", tileset.name));
        let strip_h = th.checked_mul(count).ok_or_else(too_big)?;
        let expected = (tw as usize)
            .checked_mul(strip_h as usize)
            .and_then(|n| n.checked_mul(depth.bytes_per_pixel()))
            .ok_or_else(too_big)?;
        let raw = inflate(c.bytes(len)?, expected)?;
        let strip = decode_pixels(tw, strip_h, &raw, depth)?;
        if count > 0 {
            tileset.tiles = (0..count)
                .map(|i| Arc::new(strip.crop(0, i * th, tw, th)))
                .collect();
        }
    } else if flags & TILESET_EXTERNAL != 0 {
        report.warn(format!(
            "tileset '{}' is stored in an external file and was loaded without tiles",
            tileset.name
        ));
    }
    Ok((id, tileset))
}
```

8. En `read_layer`, reemplazar el `let kind = match kind_code { … };` por:

```rust
    let (kind, tileset) = match kind_code {
        0 => (LayerKind::Image, None),
        1 => (LayerKind::Group, None),
        // Id del tileset en el archivo; `read` lo pasa a índice al terminar.
        2 => (LayerKind::Tilemap, Some(c.u32()?)),
        other => {
            report.warn(format!(
                "unknown layer type {other}; loaded as an image layer"
            ));
            (LayerKind::Image, None)
        }
    };
```

   y en el literal `Layer { … }`, `tileset: None,` pasa a `tileset,`.

9. En `read_cel`, reemplazar la rama `CEL_TILEMAP => { … }` por:

```rust
        CEL_TILEMAP => {
            let w = c.u16()? as u32;
            let h = c.u16()? as u32;
            let bits = c.u16()?;
            let index_mask = c.u32()?;
            let (x_mask, y_mask, d_mask) = (c.u32()?, c.u32()?, c.u32()?);
            c.skip(10)?;
            if bits != 32 {
                report.warn(format!(
                    "a tilemap cel with {bits} bits per tile was skipped"
                ));
                return Ok(());
            }
            if sprite.layers[layer_index].kind != LayerKind::Tilemap {
                report.warn("a tilemap cel in a non-tilemap layer was skipped");
                return Ok(());
            }
            let raw = inflate(c.rest(), w as usize * h as usize * 4)?;
            // Cada valor se normaliza a las máscaras estándar de `Tile`.
            let tiles = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| {
                    let v = u32::from_le_bytes(*b);
                    Tile::new(
                        v & index_mask,
                        v & x_mask != 0,
                        v & y_mask != 0,
                        v & d_mask != 0,
                    )
                })
                .collect();
            let id = sprite.next_image_id();
            sprite.tilemaps.insert(
                id,
                Arc::new(TileGrid {
                    width: w,
                    height: h,
                    tiles,
                }),
            );
            Cel {
                position: (x, y),
                opacity,
                image: id,
            }
        }
```

   Los cels vinculados (`CEL_LINKED`) ya copian el `Cel`, así que en capas tilemap apuntan a la
   misma grilla.

10. En `write`, dentro de `if frame == 0 {`, entre la paleta y el `for layer in &sprite.layers`:

```rust
            // Como Aseprite: los tilesets van antes de las capas, cada uno seguido de su user data.
            for (index, tileset) in sprite.tilesets.iter().enumerate() {
                chunks.push((CHUNK_TILESET, tileset_chunk(index, tileset)));
                let has_data = tileset.user_data.is_some()
                    || tileset.tile_user_data.iter().any(Option::is_some);
                if has_data {
                    chunks.push((CHUNK_USER_DATA, user_data_bytes(tileset.user_data.as_ref())));
                    for i in 0..tileset.tiles.len() {
                        let tile = tileset.tile_user_data.get(i).and_then(Option::as_ref);
                        chunks.push((CHUNK_USER_DATA, user_data_bytes(tile)));
                    }
                }
            }
```

11. `layer_chunk`: el tipo pasa a

```rust
    d.w_u16(match layer.kind {
        LayerKind::Group => 1,
        LayerKind::Tilemap => 2,
        LayerKind::Image | LayerKind::Instance => 0,
    });
```

   y después de `d.w_string(&layer.name);`:

```rust
    if layer.kind == LayerKind::Tilemap {
        d.w_u32(layer.tileset.unwrap_or(0));
    }
```

12. Debajo de `color_user_data`:

```rust
fn tileset_chunk(index: usize, tileset: &Tileset) -> Vec<u8> {
    let (tw, th) = tileset.tile_size;
    let f = tileset.allowed_flips;
    let flag = |on: bool, bit: u32| if on { bit } else { 0 };
    let flags = TILESET_EMBEDDED
        | TILESET_EMPTY_ZERO
        | flag(f.x, TILESET_FLIP_X)
        | flag(f.y, TILESET_FLIP_Y)
        | flag(f.d, TILESET_FLIP_D);
    let mut d = Vec::new();
    d.w_u32(index as u32);
    d.w_u32(flags);
    d.w_u32(tileset.tiles.len() as u32);
    d.w_u16(tw.min(u16::MAX as u32) as u16);
    d.w_u16(th.min(u16::MAX as u32) as u16);
    d.w_i16(tileset.base_index);
    d.w_zeros(14);
    d.w_string(&tileset.name);
    // Tira vertical de todos los tiles, incluido el 0.
    let raw: Vec<u8> = tileset.tiles.iter().flat_map(|t| raw_pixels(t)).collect();
    let compressed = deflate(&raw);
    d.w_u32(compressed.len() as u32);
    d.extend_from_slice(&compressed);
    d
}

/// Bytes de un chunk de user data; vacío (flags 0) si no hay.
fn user_data_bytes(chunk: Option<&RawChunk>) -> Vec<u8> {
    chunk.map_or_else(|| vec![0; 4], |c| c.data.clone())
}
```

13. Reemplazar `cel_chunk` por:

```rust
/// Contenido de un cel según su capa.
enum CelBody<'a> {
    Pixels(&'a Image),
    Tiles(&'a TileGrid),
}

fn cel_chunk(sprite: &Sprite, layer_index: usize, layer: LayerId, frame: usize) -> Option<Vec<u8>> {
    let cel = sprite.cel(layer, frame)?;
    let body = match sprite.layer(layer)?.kind {
        LayerKind::Tilemap => CelBody::Tiles(sprite.tilemap(cel.image)?),
        _ => CelBody::Pixels(sprite.image(cel.image)?),
    };
    let mut d = Vec::new();
    d.w_u16(layer_index as u16);
    d.w_i16(cel.position.0.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    d.w_i16(cel.position.1.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    d.w_u8(cel.opacity);
    // Vinculado si un frame anterior de la misma capa tiene el mismo cel.
    let link = (0..frame).find(|&f| sprite.cel(layer, f) == Some(cel));
    match (link, body) {
        (Some(source), _) => {
            d.w_u16(CEL_LINKED);
            d.w_i16(0);
            d.w_zeros(5);
            d.w_u16(source as u16);
        }
        (None, CelBody::Pixels(image)) => {
            d.w_u16(CEL_COMPRESSED);
            d.w_i16(0);
            d.w_zeros(5);
            d.w_u16(image.width() as u16);
            d.w_u16(image.height() as u16);
            d.extend_from_slice(&deflate(&raw_pixels(image)));
        }
        (None, CelBody::Tiles(grid)) => {
            d.w_u16(CEL_TILEMAP);
            d.w_i16(0);
            d.w_zeros(5);
            d.w_u16(grid.width as u16);
            d.w_u16(grid.height as u16);
            d.w_u16(32);
            d.w_u32(Tile::INDEX_MASK);
            d.w_u32(Tile::FLIP_X);
            d.w_u32(Tile::FLIP_Y);
            d.w_u32(Tile::FLIP_D);
            d.w_zeros(10);
            let raw: Vec<u8> = grid.tiles.iter().flat_map(|t| t.0.to_le_bytes()).collect();
            d.extend_from_slice(&deflate(&raw));
        }
    }
    Some(d)
}
```

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo test -p cs-io`
Expected: PASS, incluidos `aseprite_interop` (los archivos sin tilemaps no cambian), los proptest y
`saving_aseprite_and_chimp_keeps_tilemaps`. Si `corrupted_tilemap_files_never_panic` entra en
pánico, el mensaje indica la línea: cubrir ese caso con un `Err(IoError::Corrupt(…))`, nunca con
`unwrap`.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-io/src
git commit -m "feat(io): read and write Aseprite tilesets, tilemap layers and tilemap cels" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Fixtures reales de Aseprite y tests de compatibilidad

**Files:**
- Create: `crates/cs-io/tests/fixtures/tilemaps.lua`
- Create (generados por Aseprite): `crates/cs-io/tests/fixtures/tilemap_rgba.aseprite`,
  `tilemap_rgba_0.png`, `tilemap_rgba_1.png`, `tilemap_indexed.aseprite`, `tilemap_indexed_0.png`,
  `tilemap_indexed_1.png`
- Modify: `crates/cs-io/tests/aseprite_interop.rs`
- Modify (solo si el orden de flips resulta otro): `crates/cs-render/src/materialize.rs` (`tile_source` y la referencia de `tilemaps_draw_every_flip_combination`)

**Interfaces:**
- Consumes: `aseprite::read`/`write` (Task 4), `cs_render::materialize_tilemaps` (Task 2), `fixtures/frames.lua` (existente: exporta cada frame compuesto como PNG RGBA).
- Produces: fixtures commiteados; el orden de flips de `tile_source` confirmado contra Aseprite.

- [ ] **Step 1: Escribir el generador Lua**

Crear `crates/cs-io/tests/fixtures/tilemaps.lua`:

```lua
-- Genera sprites con capas tilemap para probar la compatibilidad de ChimpSprite. Uso:
--   aseprite -b --script-param out=tilemap_rgba.aseprite --script-param mode=rgb --script tilemaps.lua
--   aseprite -b --script-param out=tilemap_indexed.aseprite --script-param mode=indexed --script tilemaps.lua
-- Dos capas tilemap comparten un tileset de 8×8 ("Terrain") con user data en el tileset y en el
-- tile 2; el mapa usa los 8 flips posibles, está vinculado en los 2 frames y la otra capa se mueve.
local out = app.params["out"]
local indexed = app.params["mode"] == "indexed"

local spr = Sprite(32, 24, ColorMode.RGB)
app.activeSprite = spr
spr.gridBounds = Rectangle(0, 0, 8, 8)

local function rgba(r, g, b, a) return Color{ r = r, g = g, b = b, a = a or 255 } end

-- Fondo normal, para que los tilemaps se compongan sobre algo.
local back = spr.layers[1]
back.name = "Back"
local bg = Image(32, 24, ColorMode.RGB)
for y = 0, 23 do
  for x = 0, 31 do bg:drawPixel(x, y, rgba(20 + x * 4, 40, 60 + y * 4)) end
end
spr:newCel(back, 1, bg, Point(0, 0))
spr:newEmptyFrame(2)
spr:newCel(back, 2, bg, Point(0, 0))

-- Dos capas tilemap; la segunda pasa a usar el tileset de la primera.
app.command.NewLayer{ name = "Ground", tilemap = true, gridBounds = Rectangle(0, 0, 8, 8) }
local ground = app.activeLayer
app.command.NewLayer{ name = "Deco", tilemap = true, gridBounds = Rectangle(0, 0, 8, 8) }
local deco = app.activeLayer
local ts = ground.tileset
local unused = deco.tileset
deco.tileset = ts
spr:deleteTileset(unused)
ts.name = "Terrain"
ts.data = "biome=grass"
ts.color = rgba(10, 200, 30)

-- Tiles 1..3: asimétricos (borde izquierdo e inferior, triángulo y dos puntos) para que cada
-- flip se distinga.
local function tile_image(seed)
  local img = Image(8, 8, ColorMode.RGB)
  for y = 0, 7 do
    for x = 0, 7 do
      if x == 0 or y == 7 then
        img:drawPixel(x, y, rgba(255, 60 * seed, 0))
      elseif x > y then
        img:drawPixel(x, y, rgba(0, 120, 40 * seed + 80, 200))
      end
    end
  end
  img:drawPixel(1, 1, rgba(255, 255, 255))
  img:drawPixel(6, 2, rgba(0, 0, 0))
  return img
end
for i = 1, 3 do
  local tile = spr:newTile(ts)
  tile.image = tile_image(i)
end
ts:tile(2).data = "solid"
ts:tile(2).color = rgba(200, 0, 0)

local FLIP_X, FLIP_Y, FLIP_D = 0x20000000, 0x40000000, 0x80000000
local function tile(i, fx, fy, fd)
  local flags = 0
  if fx then flags = flags | FLIP_X end
  if fy then flags = flags | FLIP_Y end
  if fd then flags = flags | FLIP_D end
  return app.pixelColor.tile(i, flags)
end

local function tilemap(w, h, f)
  local img = Image(ImageSpec{ width = w, height = h, colorMode = ColorMode.TILEMAP })
  for y = 0, h - 1 do
    for x = 0, w - 1 do img:drawPixel(x, y, f(x, y)) end
  end
  return img
end

-- Ground: 4×3 celdas; las 8 primeras usan las 8 combinaciones de flips, la última fila vacía.
local ground_map = tilemap(4, 3, function(x, y)
  if y == 2 then return tile(0, false, false, false) end
  local combo = y * 4 + x
  return tile(1 + combo % 3, (combo & 1) ~= 0, (combo & 2) ~= 0, (combo & 4) ~= 0)
end)
spr:newCel(ground, 1, ground_map, Point(0, 0))
spr:newCel(ground, 2, ground_map, Point(0, 0))
app.range.layers = { ground }
app.range.frames = { spr.frames[1], spr.frames[2] }
app.command.LinkCels()
app.range:clear()

-- Deco: 2×1 celdas que se mueven y se aclaran en el frame 2.
local deco_map = tilemap(2, 1, function(x, y)
  if x == 0 then return tile(3, true, false, false) end
  return tile(2, false, false, true)
end)
spr:newCel(deco, 1, deco_map, Point(8, 16))
local moved = spr:newCel(deco, 2, deco_map, Point(16, 8))
moved.opacity = 160

if indexed then
  app.command.ChangePixelFormat{ format = "indexed" }
end
spr:saveAs(out)
```

- [ ] **Step 2: Generar los fixtures con el Aseprite real**

```bash
ASE="/c/Program Files (x86)/Steam/steamapps/common/Aseprite/Aseprite.exe"
cd /d/Work/chimpdeas/chimpsprite/crates/cs-io/tests/fixtures && \
"$ASE" -b --script-param out=tilemap_rgba.aseprite --script-param mode=rgb --script tilemaps.lua && \
"$ASE" -b --script-param out=tilemap_indexed.aseprite --script-param mode=indexed --script tilemaps.lua && \
"$ASE" -b --script-param file=tilemap_rgba.aseprite --script-param out=tilemap_rgba_ --script frames.lua && \
"$ASE" -b --script-param file=tilemap_indexed.aseprite --script-param out=tilemap_indexed_ --script frames.lua && \
ls -la tilemap_*
```

Expected: seis archivos (`tilemap_rgba.aseprite`, `tilemap_rgba_0.png`, `tilemap_rgba_1.png` y lo
mismo con `indexed`). Abrir los PNG con Read y comprobar que se ven el fondo, los tiles con flips y
la capa Deco.

Si Aseprite rechaza algo, imprime el error de Lua con su línea en la consola. Iterar así (sin tocar
el resto del plan):

- `app.pixelColor.tile` no existe → devolver el valor crudo: `return i | flags`.
- `Image(ImageSpec{ … colorMode = ColorMode.TILEMAP })` o `spr:newCel(ground, f, map, …)` fallan →
  crear el cel vacío con `local cel = spr:newCel(ground, 1)` y dibujar las celdas en
  `cel.image:drawPixel(x, y, tile(...))` (en ese caso, para el frame 2 usar
  `spr:newCel(ground, 2, cel.image, cel.position)` antes de `LinkCels`).
- `app.command.NewLayer{ tilemap = true, … }` no deja la capa nueva en `app.activeLayer` → tomarla
  con `spr.layers[#spr.layers]`.
- `layer.tileset = ts` o `spr:deleteTileset` fallan → dejar dos tilesets (Deco con el suyo, con los
  mismos tiles creados con un segundo loop de `spr:newTile`) y ajustar en el Step 3 la aserción de
  tileset compartido a "las dos capas tienen tileset" y `tilesets.len() == 2`.
- Para ver qué quedó en el archivo, un script de inspección temporal (no se commitea):
  `"$ASE" -b --script-param file=tilemap_rgba.aseprite --script inspect.lua` con
  `local s = app.open(app.params["file"]) for i, t in ipairs(s.tilesets) do print(i, t.name, t.grid.tileSize.width) end for _, l in ipairs(s.layers) do print(l.name, l.isTilemap) end`.

- [ ] **Step 3: Escribir los tests de compatibilidad**

En `crates/cs-io/tests/aseprite_interop.rs`:

1. En el comentario de cabecera, agregar "Tilemaps: `fixtures/tilemaps.lua`." al final del primer
   párrafo.
2. En `assert_matches_aseprite_within`, antes del `for frame`, agregar
   `let composed = cs_render::materialize_tilemaps(sprite, 0..sprite.frames.len());` y cambiar
   `let ours = cs_render::render_frame(sprite, frame);` por
   `let ours = cs_render::render_frame(&composed, frame);`.
3. Agregar:

```rust
#[test]
fn tilemap_file_renders_like_aseprite() {
    let s = load("tilemap_rgba");
    let maps: Vec<_> = s
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Tilemap)
        .collect();
    assert_eq!(maps.len(), 2);
    assert!(maps.iter().all(|l| l.tileset == Some(0)), "shared tileset");
    assert_eq!(s.tilesets.len(), 1);
    let ts = &s.tilesets[0];
    assert_eq!((ts.name.as_str(), ts.tile_size, ts.tiles.len()), ("Terrain", (8, 8), 4));
    assert!(ts.user_data.is_some(), "tileset user data");
    assert!(
        ts.tile_user_data.get(2).is_some_and(Option::is_some),
        "tile 2 user data: {:?}",
        ts.tile_user_data
    );
    let ground = maps[0].id;
    let cel = s.cel(ground, 0).unwrap();
    assert_eq!(cel.image, s.cel(ground, 1).unwrap().image, "linked cels");
    let grid = s.tilemap(cel.image).unwrap();
    assert!(grid.width >= 4 && grid.height >= 2);
    assert!(grid.tiles.iter().any(|t| t.flip_x()));
    assert!(grid.tiles.iter().any(|t| t.flip_y()));
    assert!(grid.tiles.iter().any(|t| t.flip_d()));
    assert_matches_aseprite(&s, "tilemap_rgba");
}

#[test]
fn indexed_tilemap_file_renders_like_aseprite() {
    let s = load("tilemap_indexed");
    assert!(matches!(s.color_mode, cs_core::ColorMode::Indexed { .. }));
    assert!(s.layers.iter().any(|l| l.kind == LayerKind::Tilemap));
    assert_matches_aseprite(&s, "tilemap_indexed");
}

#[test]
fn saving_tilemaps_keeps_everything_aseprite_wrote() {
    for name in ["tilemap_rgba", "tilemap_indexed"] {
        let s = load(name);
        let (back, report) = aseprite::read(&aseprite::write(&s)).unwrap();
        assert!(report.warnings.is_empty(), "{name}: {:?}", report.warnings);
        assert_eq!(back.layers, s.layers, "{name}");
        assert_eq!(back.tilesets, s.tilesets, "{name}");
        assert_eq!(back.frames, s.frames, "{name}");
        for layer in &s.layers {
            for f in 0..s.frames.len() {
                let (a, b) = (s.cel(layer.id, f), back.cel(layer.id, f));
                assert_eq!(
                    a.map(|c| (c.position, c.opacity)),
                    b.map(|c| (c.position, c.opacity)),
                    "{name} {} frame {f}",
                    layer.name
                );
                if let (Some(a), Some(b)) = (a, b) {
                    assert_eq!(s.tilemap(a.image), back.tilemap(b.image));
                    assert_eq!(s.image(a.image), back.image(b.image));
                }
            }
        }
        assert_matches_aseprite(&back, name);
    }
}

/// Lo que escribe ChimpSprite, abierto por el Aseprite real, se ve igual que el original. Necesita
/// Aseprite: `cargo test -p cs-io --test aseprite_interop -- --ignored` (ruta en `ASEPRITE`, o la
/// de Steam por defecto).
#[test]
#[ignore = "needs Aseprite; set ASEPRITE to its executable"]
fn aseprite_opens_the_tilemaps_we_write() {
    let exe = std::env::var("ASEPRITE").unwrap_or_else(|_| {
        r"C:\Program Files (x86)\Steam\steamapps\common\Aseprite\Aseprite.exe".into()
    });
    let dir = std::env::temp_dir().join("chimpsprite-tilemaps");
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["tilemap_rgba", "tilemap_indexed"] {
        let s = load(name);
        let file = dir.join(format!("{name}.aseprite"));
        std::fs::write(&file, aseprite::write(&s)).unwrap();
        let prefix = dir.join(format!("{name}_"));
        let status = std::process::Command::new(&exe)
            .arg("-b")
            .arg("--script-param")
            .arg(format!("file={}", file.display()))
            .arg("--script-param")
            .arg(format!("out={}", prefix.display()))
            .arg("--script")
            .arg(format!("{FIXTURES}/frames.lua"))
            .status()
            .unwrap();
        assert!(status.success(), "{name}: Aseprite failed");
        for f in 0..s.frames.len() {
            let read_png = |path: String| import::decode_png(&std::fs::read(path).unwrap()).unwrap();
            let ours = read_png(format!("{}{f}.png", prefix.display()));
            let theirs = read_png(format!("{FIXTURES}/{name}_{f}.png"));
            assert_eq!(ours.pixels, theirs.pixels, "{name} frame {f}");
        }
    }
}
```

- [ ] **Step 4: Correr, fijar el orden de flips y verificar que pasa**

Run: `cargo test -p cs-io --test aseprite_interop`
Expected: PASS. Si `tilemap_file_renders_like_aseprite` falla y el peor píxel cae en una celda con
flip D combinado con X o Y (las celdas 4–7 de la fila 1 de Ground), el orden de Aseprite es "X e Y
primero, D al final". En ese caso, en `crates/cs-render/src/materialize.rs`:

- `tile_source` pasa a:

```rust
fn tile_source(tile: Tile, w: u32, h: u32, x: u32, y: u32) -> (u32, u32) {
    let (a, b) = if tile.flip_d() { (y, x) } else { (x, y) };
    let sx = if tile.flip_x() { w - 1 - a } else { a };
    let sy = if tile.flip_y() { h - 1 - b } else { b };
    (sx, sy)
}
```

  con el doc comment "Equivale a aplicar a la imagen X, después Y y después D (transponer)…".
- En `tilemaps_draw_every_flip_combination`, mover el bloque `if i & 4 != 0 { … }` después de los
  de X e Y y actualizar el comentario de la referencia.
- Correr `cargo test -p cs-render && cargo test -p cs-io --test aseprite_interop`; los dos pasan.

Si falla por otra razón (colores de bordes semitransparentes, tolerancia), comparar el peor píxel
con el PNG y corregir la causa en `cs-io`/`cs-render`, no la tolerancia.

Después, con Aseprite instalado:

Run: `cargo test -p cs-io --test aseprite_interop -- --ignored`
Expected: PASS (`aseprite_opens_the_tilemaps_we_write`).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-io/tests crates/cs-render/src/materialize.rs
git commit -m "test(io): real Aseprite tilemap fixtures and interop tests" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Tilemaps por MCP (`cs-agent` + `chimpsprite-mcp`)

**Files:**
- Create: `crates/cs-agent/src/ops/tilemap.rs`
- Modify: `crates/cs-agent/src/ops/mod.rs` (`mod tilemap;`)
- Modify: `crates/cs-agent/src/params.rs` (después de `RemoveInstanceKey`)
- Modify: `crates/cs-agent/src/ops/document.rs` (`describe_sprite` 69-149)
- Modify: `crates/cs-agent/src/target.rs:67-72` (mensaje de `image_layer`)
- Modify: `crates/cs-agent/src/ops/color.rs:141-146` (`quantize`)
- Modify: `apps/chimpsprite-mcp/src/server.rs` (después de `remove_instance_key`, ~línea 201)

**Interfaces:**
- Consumes: `Command::SetTileset`, `ops::new_tilemap_layer`, `ops::set_tiles`, `ops::set_tile_image` (Task 3); `Tileset::new`, `Tile::new` (Task 1); `cs_render::materialize` (Task 2).
- Produces:
  - `Session::create_tileset(CreateTileset)`, `Session::set_tile_pixels(SetTilePixels)`,
    `Session::add_tilemap_layer(AddTilemapLayer)`, `Session::set_tiles(SetTiles)`,
    `Session::get_tiles(&self, GetTiles)`; todas `-> Result<Reply>`.
  - `describe`: cada artboard con `"tilesets": [{index, name, tile_size, tiles, base_index}]`; capas
    tilemap con `"kind": "tilemap"` y `"tileset"`; cels tilemap con `"grid": [cols, rows]`.
  - Cinco herramientas MCP con los mismos nombres.

- [ ] **Step 1: Escribir el test que falla**

Crear `crates/cs-agent/src/ops/tilemap.rs` solo con:

```rust
#[cfg(test)]
mod tests {
    use cs_core::Rgba;
    use serde_json::json;

    use crate::Session;
    use crate::ops::test_util::{json, p, session};

    const RED: Rgba = Rgba::rgb(255, 0, 0);
    const BLUE: Rgba = Rgba::rgb(0, 0, 255);

    fn px(s: &Session, x: usize, y: usize) -> Rgba {
        let (_, sprite) = s.sprite("d1", None).unwrap();
        let d = &s.doc("d1").unwrap().document;
        cs_render::render_frame(&cs_render::materialize(d, sprite, 0..1), 0)
            [y * sprite.width as usize + x]
    }

    #[test]
    fn tilemaps_create_paint_place_read_and_describe() {
        let mut s = session(16, 8);
        s.create_tileset(p(json!({"doc": "d1", "name": "terrain",
            "tile_width": 4, "tile_height": 4})))
            .unwrap();
        s.set_tile_pixels(p(json!({"doc": "d1", "tileset": 0, "tile": 1, "pixels": [
            {"x": 0, "y": 0, "color": "#ff0000"}, {"x": 3, "y": 0, "color": "#0000ff"}]})))
            .unwrap();
        assert!(
            s.set_tile_pixels(p(json!({"doc": "d1", "tileset": 0, "tile": 0,
                "pixels": [{"x": 0, "y": 0, "color": "#ff0000"}]})))
                .is_err(),
            "tile 0 cannot be edited"
        );
        assert!(
            s.set_tile_pixels(p(json!({"doc": "d1", "tileset": 0, "tile": 3, "pixels": []})))
                .is_err(),
            "tile 3 would skip tile 2"
        );
        s.add_tilemap_layer(p(json!({"doc": "d1", "tileset": 0, "name": "map"})))
            .unwrap();
        s.set_tiles(p(json!({"doc": "d1", "layer": "map", "cells": [
            {"x": 1, "y": 1, "tile": 1}, {"x": 2, "y": 1, "tile": 1, "flip_x": true}]})))
            .unwrap();
        // Celda (1, 1) = píxeles 4..8 × 4..8; la (2, 1) está espejada.
        assert_eq!(px(&s, 4, 4), RED);
        assert_eq!(px(&s, 7, 4), BLUE);
        assert_eq!(px(&s, 11, 4), RED);
        assert_eq!(px(&s, 8, 4), BLUE);
        assert!(
            s.set_tiles(p(json!({"doc": "d1", "layer": "map",
                "cells": [{"x": 0, "y": 0, "tile": 5}]})))
                .is_err(),
            "tile out of range"
        );
        assert!(
            s.set_tiles(p(json!({"doc": "d1", "layer": 1,
                "cells": [{"x": 0, "y": 0, "tile": 1}]})))
                .is_err(),
            "not a tilemap layer"
        );

        let tiles = json(&s.get_tiles(p(json!({"doc": "d1", "layer": "map"}))).unwrap());
        assert_eq!(tiles["cel"]["columns"], 3);
        assert_eq!(tiles["cel"]["rows"], 2);
        assert_eq!(tiles["cel"]["first_cell"], json!([0, 0]));
        assert_eq!(tiles["rows"][1][2]["tile"], 1);
        assert_eq!(tiles["rows"][1][2]["flip_x"], true);
        assert!(tiles["rows"][1][1].get("flip_x").is_none());

        let described = json(&s.describe(p(json!({"doc": "d1"}))).unwrap());
        let artboard = &described["artboards"][0];
        assert_eq!(artboard["tilesets"][0]["name"], "terrain");
        assert_eq!(artboard["tilesets"][0]["tiles"], 2);
        assert_eq!(artboard["tilesets"][0]["tile_size"], json!([4, 4]));
        let layer = &artboard["layers_bottom_to_top"][1];
        assert_eq!(layer["kind"], "tilemap");
        assert_eq!(layer["tileset"], 0);
        let cel = artboard["cels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["layer"] == layer["id"])
            .unwrap();
        assert_eq!(cel["grid"], json!([3, 2]));
    }
}
```

Agregar `mod tilemap;` en `crates/cs-agent/src/ops/mod.rs` (después de `mod structure;`).

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-agent tilemaps_create`
Expected: error de compilación (`create_tileset`… no existen).

- [ ] **Step 3: Implementar**

En `crates/cs-agent/src/params.rs`, después de `RemoveInstanceKey`:

```rust
// --- Tilemaps ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTileset {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub name: String,
    /// Tile width in pixels (1-256).
    pub tile_width: u32,
    /// Tile height in pixels (1-256).
    pub tile_height: u32,
    /// Number shown for tile 1 in the UI (default 1, like Aseprite).
    pub base_index: Option<i16>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTilePixels {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Tileset index (0-based, as listed by describe).
    pub tileset: usize,
    /// Tile to paint. Tile 0 is the empty tile and cannot be edited; use the current tile count
    /// to add a new tile.
    pub tile: usize,
    /// Pixels in tile coordinates.
    pub pixels: Vec<PixelArg>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddTilemapLayer {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Tileset index (0-based).
    pub tileset: usize,
    /// Layer name (default "Tilemap N").
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TileCell {
    /// Canvas grid cell: cell (x, y) starts at pixel (x * tile_width, y * tile_height).
    pub x: i32,
    pub y: i32,
    /// Tile index (0 = empty, erases the cell).
    pub tile: u32,
    #[serde(default)]
    pub flip_x: bool,
    #[serde(default)]
    pub flip_y: bool,
    /// Diagonal flip (swap X/Y); needs square tiles.
    #[serde(default)]
    pub flip_d: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTiles {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// A tilemap layer.
    pub layer: Ref,
    #[serde(default)]
    pub frame: usize,
    pub cells: Vec<TileCell>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTiles {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// A tilemap layer.
    pub layer: Ref,
    #[serde(default)]
    pub frame: usize,
}
```

En `crates/cs-agent/src/ops/tilemap.rs`, arriba de los tests:

```rust
//! Tilemaps: tilesets, tiles y grillas de las capas tilemap.

use cs_commands::{Command, ops};
use cs_core::{Image, LayerId, LayerKind, Sprite, Tile, Tileset};
use serde_json::{Value, json};

use crate::error::{Result, invalid};
use crate::params::{AddTilemapLayer, CreateTileset, GetTiles, Ref, SetTilePixels, SetTiles};
use crate::target;
use crate::{Reply, Session};

/// Lado máximo de un tile.
const MAX_TILE: u32 = 256;
/// Coordenada máxima (en celdas, en valor absoluto) que acepta `set_tiles`.
const MAX_CELL: i32 = 4096;

/// Capa tilemap indicada, con el índice y el tileset que usa.
fn tilemap_layer<'a>(sprite: &'a Sprite, r: &Ref) -> Result<(LayerId, usize, &'a Tileset)> {
    let id = target::layer(sprite, r)?;
    let layer = sprite.layer(id).expect("resolved");
    if layer.kind != LayerKind::Tilemap {
        return Err(invalid(format!(
            "layer '{}' is not a tilemap layer; create one with add_tilemap_layer",
            layer.name
        )));
    }
    let index = layer.tileset.map_or(usize::MAX, |i| i as usize);
    let tileset = sprite.tilesets.get(index).ok_or_else(|| {
        invalid(format!("tilemap layer '{}' has no valid tileset", layer.name))
    })?;
    Ok((id, index, tileset))
}

fn tileset_at(sprite: &Sprite, index: usize) -> Result<&Tileset> {
    sprite.tilesets.get(index).ok_or_else(|| {
        invalid(format!(
            "tileset {index} not found; the artboard has {} tileset(s) (create one with create_tileset)",
            sprite.tilesets.len()
        ))
    })
}

impl Session {
    pub fn create_tileset(&mut self, p: CreateTileset) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (w, h) = (p.tile_width, p.tile_height);
        if !(1..=MAX_TILE).contains(&w) || !(1..=MAX_TILE).contains(&h) {
            return Err(invalid(format!(
                "tile size {w}×{h} must be 1-{MAX_TILE} on each side"
            )));
        }
        let mut tileset = Tileset::new(p.name.clone(), (w, h), sprite.color_mode);
        if let Some(base) = p.base_index {
            tileset.base_index = base;
        }
        let index = sprite.tilesets.len();
        let cmd = Command::SetTileset {
            index,
            tileset: Some(tileset),
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!(
            "Created tileset {index} '{}' with {w}×{h} tiles. Tile 0 is the empty tile; add tiles \
             with set_tile_pixels (tile 1 first).",
            p.name
        )))
    }

    pub fn set_tile_pixels(&mut self, p: SetTilePixels) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let ts = tileset_at(sprite, p.tileset)?;
        if p.tile == 0 {
            return Err(invalid("tile 0 is the empty tile and cannot be edited"));
        }
        let count = ts.tiles.len();
        if p.tile > count {
            return Err(invalid(format!(
                "tile {} out of range; the tileset has {count} tiles (use tile {count} to add one)",
                p.tile
            )));
        }
        let (w, h) = ts.tile_size;
        let mut image = ts
            .tiles
            .get(p.tile)
            .map_or_else(|| Image::new(w, h, sprite.color_mode), |t| Image::clone(t));
        for px in &p.pixels {
            if px.x < 0 || px.y < 0 || px.x as u32 >= w || px.y as u32 >= h {
                return Err(invalid(format!(
                    "pixel ({}, {}) is outside the {w}×{h} tile",
                    px.x, px.y
                )));
            }
            let value = target::color(sprite, &px.color)?;
            image
                .set(px.x as u32, px.y as u32, value)
                .map_err(|e| invalid(e.to_string()))?;
        }
        let added = p.tile == count;
        let Some(cmd) = ops::set_tile_image(sprite, p.tileset, p.tile, image) else {
            return Ok(Reply::text("Nothing changed."));
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(if added {
            format!("Added tile {} to tileset {}.", p.tile, p.tileset)
        } else {
            format!("Painted tile {} of tileset {}.", p.tile, p.tileset)
        }))
    }

    pub fn add_tilemap_layer(&mut self, p: AddTilemapLayer) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        tileset_at(sprite, p.tileset)?;
        // Arriba de todo, en el nivel raíz.
        let Command::AddLayer { index, mut layer } =
            ops::new_tilemap_layer(sprite, None, p.tileset as u32)
        else {
            unreachable!("new_tilemap_layer devuelve AddLayer");
        };
        if let Some(name) = &p.name {
            layer.name = name.clone();
        }
        let reply = format!(
            "Added tilemap layer '{}' using tileset {}.",
            layer.name, p.tileset
        );
        self.run_on(&p.doc, ab, vec![Command::AddLayer { index, layer }])?;
        Ok(Reply::text(reply))
    }

    pub fn set_tiles(&mut self, p: SetTiles) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (layer, _, tileset) = tilemap_layer(sprite, &p.layer)?;
        let frame = target::frame(sprite, p.frame)?;
        if p.cells.is_empty() {
            return Err(invalid("cells is empty"));
        }
        let count = tileset.tiles.len();
        let square = tileset.tile_size.0 == tileset.tile_size.1;
        let mut cells = Vec::with_capacity(p.cells.len());
        for c in &p.cells {
            if c.x.abs() > MAX_CELL || c.y.abs() > MAX_CELL {
                return Err(invalid(format!(
                    "cell ({}, {}) is too far; use -{MAX_CELL}..{MAX_CELL}",
                    c.x, c.y
                )));
            }
            if c.tile as usize >= count {
                return Err(invalid(format!(
                    "tile {} out of range; the tileset has {count} tiles (0 = empty)",
                    c.tile
                )));
            }
            if c.flip_d && !square {
                return Err(invalid("flip_d needs square tiles"));
            }
            cells.push((c.x, c.y, Tile::new(c.tile, c.flip_x, c.flip_y, c.flip_d)));
        }
        let Some(cmd) = ops::set_tiles(sprite, layer, frame, &cells) else {
            return Ok(Reply::text("Nothing changed."));
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!(
            "Set {} cell(s) on frame {frame}.",
            cells.len()
        )))
    }

    pub fn get_tiles(&self, p: GetTiles) -> Result<Reply> {
        let (_, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (layer, index, tileset) = tilemap_layer(sprite, &p.layer)?;
        let frame = target::frame(sprite, p.frame)?;
        let (tw, th) = tileset.tile_size;
        let found = sprite
            .cel(layer, frame)
            .and_then(|c| Some((c, sprite.tilemap(c.image)?)));
        let Some((cel, grid)) = found else {
            return Ok(Reply::json(json!({
                "layer": layer.0,
                "frame": frame,
                "tileset": index,
                "tile_size": [tw, th],
                "cel": null,
                "rows": [],
            })));
        };
        let rows: Vec<Value> = (0..grid.height)
            .map(|y| {
                let row: Vec<Value> = (0..grid.width)
                    .map(|x| {
                        let t = grid.get(x, y).unwrap_or(Tile::EMPTY);
                        let mut v = json!({ "tile": t.index() });
                        if t.flip_x() {
                            v["flip_x"] = json!(true);
                        }
                        if t.flip_y() {
                            v["flip_y"] = json!(true);
                        }
                        if t.flip_d() {
                            v["flip_d"] = json!(true);
                        }
                        v
                    })
                    .collect();
                Value::from(row)
            })
            .collect();
        let first_cell = [
            cel.position.0.div_euclid(tw as i32),
            cel.position.1.div_euclid(th as i32),
        ];
        Ok(Reply::json(json!({
            "layer": layer.0,
            "frame": frame,
            "tileset": index,
            "tile_size": [tw, th],
            "cel": {
                "x": cel.position.0,
                "y": cel.position.1,
                "opacity": cel.opacity,
                "first_cell": first_cell,
                "columns": grid.width,
                "rows": grid.height,
            },
            "rows": rows,
            "notes": "rows[r][c] is canvas cell (first_cell[0] + c, first_cell[1] + r); tile 0 is empty.",
        })))
    }
}
```

En `crates/cs-agent/src/ops/document.rs`, `describe_sprite`:

1. Dentro del `.map(|l| { … })` de capas, después de `if let Some(track) = &l.instance { … }`:

```rust
            if let Some(t) = l.tileset {
                v["tileset"] = json!(t);
            }
```

2. En el `.map` de cels, reemplazar el `json!({ … })` final por:

```rust
            let mut v = json!({
                "layer": layer.0,
                "frame": frame,
                "x": cel.position.0,
                "y": cel.position.1,
                "w": w,
                "h": h,
                "opacity": cel.opacity,
                "image": cel.image.0,
            });
            if let Some(g) = s.tilemap(cel.image) {
                v["grid"] = json!([g.width, g.height]);
            }
            v
```

3. En el `json!({ … })` final, después de `"palette": …,`:

```rust
        "tilesets": s.tilesets.iter().enumerate().map(|(i, t)| json!({
            "index": i,
            "name": t.name,
            "tile_size": [t.tile_size.0, t.tile_size.1],
            "tiles": t.tiles.len(),
            "base_index": t.base_index,
        })).collect::<Vec<_>>(),
```

En `crates/cs-agent/src/target.rs`, en `image_layer`, reemplazar el `if l.kind != LayerKind::Image { … }` por:

```rust
    if l.kind != LayerKind::Image {
        let what = match l.kind {
            LayerKind::Group => "a group",
            LayerKind::Instance => "an instance layer",
            LayerKind::Tilemap => "a tilemap layer (use set_tiles or set_tile_pixels)",
            LayerKind::Image => unreachable!("checked above"),
        };
        return Err(invalid(format!(
            "layer '{}' is {what}, not an image layer",
            l.name
        )));
    }
```

En `crates/cs-agent/src/ops/color.rs`, `quantize`, después del chequeo de `ColorMode::Rgba`:

```rust
        // Los tiles también tendrían que convertirse; queda para cuando se editen tilemaps.
        if !sprite.tilesets.is_empty() {
            return Err(invalid("quantize does not support artboards with tilesets yet"));
        }
```

En `apps/chimpsprite-mcp/src/server.rs`, después de `remove_instance_key`:

```rust
    // --- Tilemaps ---

    #[tool(
        description = "Create an empty tileset in an artboard (tile 0 is always the empty tile). Paint tiles with set_tile_pixels and draw with them on a tilemap layer (add_tilemap_layer, set_tiles)."
    )]
    fn create_tileset(&self, Parameters(p): Parameters<CreateTileset>) -> CallToolResult {
        self.with(|s| s.create_tileset(p))
    }

    #[tool(
        description = "Paint pixels of one tile, in tile coordinates. Use tile = the current tile count to add a new tile; tile 0 cannot be edited. Every cel using the tile updates."
    )]
    fn set_tile_pixels(&self, Parameters(p): Parameters<SetTilePixels>) -> CallToolResult {
        self.with(|s| s.set_tile_pixels(p))
    }

    #[tool(description = "Add a tilemap layer on top that draws tiles from a tileset.")]
    fn add_tilemap_layer(&self, Parameters(p): Parameters<AddTilemapLayer>) -> CallToolResult {
        self.with(|s| s.add_tilemap_layer(p))
    }

    #[tool(
        description = "Place tiles on a tilemap layer. Cells are canvas grid cells: cell (x, y) starts at pixel (x * tile_width, y * tile_height). Tile 0 erases a cell; optional flip_x / flip_y / flip_d. The layer's grid grows as needed."
    )]
    fn set_tiles(&self, Parameters(p): Parameters<SetTiles>) -> CallToolResult {
        self.with(|s| s.set_tiles(p))
    }

    #[tool(
        description = "Read a tilemap layer's grid on a frame: tile index and flips of each cell, and the cel position."
    )]
    fn get_tiles(&self, Parameters(p): Parameters<GetTiles>) -> CallToolResult {
        self.with(|s| s.get_tiles(p))
    }
```

El render (`view.rs::render`, `read_pixels`) ya pasa por `cs_render::materialize` y `export` por
`bake_layers(…, Bake::Everything)` (Task 2): no cambian.

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo test -p cs-agent && cargo build -p chimpsprite-mcp`
Expected: PASS y el servidor compila. Si `session(16, 8)`/`describe` usan otras claves (por
ejemplo `layers_bottom_to_top`), leer `describe_sprite` y ajustar el JSON del test, no la
implementación.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-agent apps/chimpsprite-mcp
git commit -m "feat(mcp): create tilesets, paint tiles, add tilemap layers, set and read tiles" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: UI mínima y herramientas (`cs-tools`, `cs-ui`)

**Files:**
- Modify: `crates/cs-tools/src/move_tool.rs:24-37` (`move_output`) y tests
- Modify: `crates/cs-tools/src/eyedropper.rs:51-53` (`visible_value`) y tests
- Modify: `crates/cs-tools/src/select.rs:261-268` (varita, `sample_all`)
- Modify: `crates/cs-ui/src/timeline.rs:538-545` (prefijo)
- Modify: `crates/cs-ui/src/panels.rs:533-539` (`status_bar`)
- Modify: `crates/cs-ui/src/canvas.rs:1000-1005` (pipeta)

**Interfaces:**
- Consumes: `cs_render::materialize_tilemaps` (Task 2), `LayerKind::Tilemap` (Task 1).
- Produces: Move no emite comandos sobre capas tilemap; la pipeta, el eyedropper y la varita con
  "sample all" ven los tilemaps; el timeline muestra `▦ nombre`; la barra de estado avisa
  "Tilemap layer: tile editing comes in a later version".

- [ ] **Step 1: Escribir los tests que fallan**

En los tests de `crates/cs-tools/src/move_tool.rs`:

```rust
    #[test]
    fn move_does_nothing_on_tilemap_layers() {
        let mut s = with_dot();
        s.layers[0].kind = LayerKind::Tilemap;
        let out = gesture(&ctx(&s), (0, 0), (2, 3));
        assert!(out.command.is_none());
    }
```

En los tests de `crates/cs-tools/src/eyedropper.rs`:

```rust
    #[test]
    fn picks_colors_from_tilemap_layers() {
        use std::sync::Arc;

        use cs_core::{Cel, Image, ImageId, LayerKind, PixelValue, Rgba, Tile, TileGrid, Tileset};
        let mut s = sprite();
        let mut ts = Tileset::new("t", (2, 2), s.color_mode);
        let mut img = Image::new(2, 2, s.color_mode);
        img.set(0, 0, red()).unwrap();
        ts.tiles.push(Arc::new(img));
        s.tilesets.push(ts);
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        let mut grid = TileGrid::new(2, 1);
        grid.set(1, 0, Tile::new(1, false, false, false));
        s.tilemaps.insert(ImageId(1), Arc::new(grid));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: ImageId(1),
            },
        );
        assert_eq!(visible_value(&s, 0, (2, 0)), red());
        assert_eq!(
            visible_value(&s, 0, (0, 0)),
            PixelValue::Rgba(Rgba::TRANSPARENT)
        );
    }
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-tools`
Expected: FAIL en los dos tests nuevos (Move emite `SetCel`; `visible_value` devuelve transparente
en (2, 0)).

- [ ] **Step 3: Implementar**

`crates/cs-tools/src/move_tool.rs`, en `move_output`, después del chequeo de `locked`:

```rust
    // Las capas tilemap se editan por tiles (llega en el subproyecto 2): Move no hace nada.
    if sprite
        .layer(ctx.layer)
        .is_some_and(|l| l.kind == cs_core::LayerKind::Tilemap)
    {
        return ToolOutput::default();
    }
```

`crates/cs-tools/src/eyedropper.rs`, `visible_value`, reemplazar
`let c = cs_render::composite_pixel(sprite, frame, p.0, p.1);` por:

```rust
    // Los tilemaps se convierten en píxeles antes de leer lo visible.
    let composed = cs_render::materialize_tilemaps(sprite, frame..frame + 1);
    let c = cs_render::composite_pixel(&composed, frame, p.0, p.1);
```

`crates/cs-tools/src/select.rs`, en `preview` de la varita, después de `let s = ctx.sprite;`:

```rust
        let composed = cs_render::materialize_tilemaps(s, ctx.frame..ctx.frame + 1);
```

y en la closure `color`, `cs_render::composite_pixel(s, ctx.frame, x, y)` pasa a
`cs_render::composite_pixel(&composed, ctx.frame, x, y)`.

`crates/cs-ui/src/canvas.rs`, pipeta (~línea 1004), reemplazar
`inside.then(|| cs_render::composite_pixel(s, editor.frame, x, y))` por:

```rust
                inside.then(|| {
                    let frame = editor.frame;
                    let composed = cs_render::materialize_tilemaps(s, frame..frame + 1);
                    cs_render::composite_pixel(&composed, frame, x, y)
                })
```

`crates/cs-ui/src/timeline.rs`, reemplazar el `let label = if … else { … };` del nombre de capa por:

```rust
        let label = match layer.kind {
            LayerKind::Group => format!("📁 {}", layer.name),
            // El mismo ícono que la herramienta Artboard: la capa muestra otro artboard.
            LayerKind::Instance => format!("🖼 {}", layer.name),
            // Grilla de tiles.
            LayerKind::Tilemap => format!("▦ {}", layer.name),
            LayerKind::Image => layer.name.clone(),
        };
```

(Los cels de una capa tilemap se dibujan con el código existente, incluido el indicador de vínculo,
porque `is_linked` compara ids.)

`crates/cs-ui/src/panels.rs`, `status_bar`, después del bloque `on_instance`:

```rust
        let on_tilemap = s
            .layer(editor.layer)
            .is_some_and(|l| l.kind == cs_core::LayerKind::Tilemap);
        let tool = editor.active_tool();
        if on_tilemap && (tool.paints() || tool == ToolKind::Move) {
            ui.separator();
            ui.colored_label(
                theme::ACCENT,
                "Tilemap layer: tile editing comes in a later version",
            );
        }
```

Las herramientas de pintura ya no hacen nada sobre una capa tilemap: `DrawPixels` falla con
`NotImageLayer` y `Editor::execute_doc` solo lo registra con `log::warn`.

Callers de `materialize` en la UI (verificados, no cambian): `canvas.rs:796` (canvas y onion),
`export_dialog.rs` (Task 2). No hay otros `render_frame` en `cs-ui`.

- [ ] **Step 4: Correr y verificar en la app**

Run: `cargo test -p cs-tools && cargo test --workspace`
Expected: PASS.

Captura con el fixture (Git Bash):

```bash
CHIMPSPRITE_SCREENSHOT=/d/Work/chimpdeas/chimpsprite/target/tilemap.png cargo run -p chimpsprite -- crates/cs-io/tests/fixtures/tilemap_rgba.aseprite
```

Abrir `target/tilemap.png` con Read y comprobar:

1. El canvas muestra el mapa igual que `crates/cs-io/tests/fixtures/tilemap_rgba_0.png`.
2. En el timeline, "Ground" y "Deco" llevan el prefijo `▦` visible (no un cuadrado vacío de glifo
   faltante). Si el glifo no existe en la fuente, probar `⊞` y después `#`, repitiendo la captura.
3. Los cels de Ground en los frames 1 y 2 se ven vinculados.

A mano (`cargo run -p chimpsprite -- crates/cs-io/tests/fixtures/tilemap_rgba.aseprite`): con la
capa Ground activa, elegir Pencil y Move: la barra de estado muestra el aviso y pintar o arrastrar
no cambia nada. Guardar como `.aseprite` y como `.chimp`, reabrir: se ve igual. Exportar PNG y GIF:
los tilemaps aparecen.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-tools crates/cs-ui
git commit -m "feat(ui): show tilemap layers in the timeline and keep tools off them" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Verificación final y PR

- [ ] **Step 1: Correr lo mismo que CI**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --target wasm32-unknown-unknown -p cs-core -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite
```

Expected: sin errores ni warnings.

- [ ] **Step 2: Interop con Aseprite real**

```bash
cargo test -p cs-io --test aseprite_interop -- --ignored
```

Expected: PASS (Aseprite abre lo que escribe ChimpSprite y exporta los mismos PNG).

Además, con el MCP: crear un documento, `create_tileset`, `set_tile_pixels`, `add_tilemap_layer`,
`set_tiles` y `save` como `.aseprite`; abrirlo con Aseprite en batch:

```bash
"/c/Program Files (x86)/Steam/steamapps/common/Aseprite/Aseprite.exe" -b test.aseprite --save-as test.png
```

Expected: `test.png` igual al `render` del MCP.

- [ ] **Step 3: Commit de formato (si `cargo fmt` cambió algo) y PR**

```bash
git add -A && git commit -m "style: cargo fmt" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"   # solo si hubo cambios
git push -u origin tilemaps
gh pr create --title "Tilemaps: núcleo (abrir, ver, guardar y editar por MCP)" --body "…"
```

El cuerpo del PR resume la spec (modelo, materialize/bake_layers, `.aseprite`, comandos, MCP, UI
mínima, fixtures reales y el orden de flips confirmado) y termina con:

```
🤖 Generated with [Claude Code](https://claude.com/claude-code)
```
