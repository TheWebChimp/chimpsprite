# Edición de tilemaps 2A (modo Tiles) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Armar niveles con tiles desde la GUI: crear capas tilemap y tilesets, elegir un tile y
pintar, borrar, rellenar, tomar y seleccionar con tiles, mover cels por tiles, y convertir capas
de imagen ↔ tilemap (también por MCP).

**Architecture:** Con una capa tilemap activa, el editor no le pasa a las herramientas el sprite
real sino un **proxy de celdas** (`cs-ui/src/tile_ops.rs`): un `Sprite` RGBA de
`ceil(ancho/tw) × ceil(alto/th)` donde cada píxel codifica un `Tile`
(`Rgba` = `tile.0.to_le_bytes()`, el tile 0 es transparente). El puntero se pasa a celdas, la
selección del editor se pasa a celdas, y la salida de la herramienta vuelve al sprite real:
`DrawPixels` → `ops::set_tiles`, selecciones de celdas → máscara de píxeles. La vista previa usa la
misma traducción. `cs-tools` gana `ToolContext::tiles` (cuentagotas y varita leen valores exactos)
y Move mueve los cels tilemap en pasos de un tile. La conversión de capas vive en
`cs-commands::ops` (`convert_to_tilemap`, `convert_to_image`) y cambia el tipo de capa quitándola y
volviéndola a agregar en el mismo índice (sin primitiva nueva). El dibujo de una grilla de tiles
pasa de `cs-render` a `cs-core` para que `cs-commands` pueda usarlo sin depender de `cs-render`.
La UI suma el panel Tileset, el diálogo "New Tilemap Layer", "Convert to Tilemap/Image", la grilla
de tiles y los números de tile en el lienzo; el MCP suma `convert_layer`.

**Tech Stack:** Rust 2024, workspace con crates `cs-core`, `cs-raster`, `cs-render`, `cs-commands`,
`cs-io`, `cs-tools`, `cs-agent`, `cs-ui` (egui/eframe 0.36) y `apps/chimpsprite-mcp` (rmcp).

Spec: `docs/superpowers/specs/2026-10-21-tilemaps-edit-tiles-mode-design.md`
Subproyecto anterior: `docs/superpowers/specs/2026-10-19-tilemaps-core-design.md`

## Global Constraints

- El código y los comentarios siguen el estilo del repo: comentarios en español, textos de UI y
  descripciones MCP en inglés.
- Los crates de motor (`cs-core`, `cs-raster`, `cs-render`, `cs-commands`, `cs-io`, `cs-tools`,
  `cs-agent`) no dependen de egui y compilan para `wasm32-unknown-unknown`. El proxy de celdas vive
  en `cs-ui` pero no usa egui (se testea sin contexto).
- Todos los comandos son serializables y devuelven su inverso exacto (el undo restaura el sprite
  igual, `assert_eq!(s, before)`). No se agregan variantes a `Command`: el cambio de tipo de capa es
  `RemoveLayer` + `AddLayer` en el mismo índice dentro de un `Batch`.
- Campos persistidos nuevos con `#[serde(default)]` (`ViewSettings` ya tiene `#[serde(default)]` a
  nivel de struct; un `app.ron` viejo sigue cargando).
- En 2A una capa tilemap activa **siempre** se edita en modo Tiles (salvo si se está editando su
  máscara). Sin selector Píxeles/Tiles.
- Celdas del lienzo como en `ops::set_tiles`: la celda `(cx, cy)` empieza en el píxel
  `(cx·tw, cy·th)` más el resto de la posición del cel si no está alineado.
- Numeración del usuario: el vacío es 0; el tile `i > 0` es `i + base_index - 1`.
- Herramientas en modo Tiles: Pencil, Eraser, Line, Rectangle, Ellipse, Bucket, Eyedropper,
  Rect/Ellipse Marquee y Magic Wand trabajan en celdas; Move mueve el cel por tiles; Spray, Jumble,
  Gradient, Text, Lasso y Polygon Lasso no actúan ("Not available in Tiles mode"); simetría y
  pixel-perfect se ignoran; Slice y Artboard siguen igual (en píxeles).
- Fuera de alcance (2B o después): pintar píxeles de tiles, modos Manual/Auto/Stack, selector
  Píxeles/Tiles, reordenar/copiar/borrar tiles, importar tilesets PNG, deduplicar con flips, simetría
  y lazos en Tiles, tile secundario.

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

- Si compilar el binario `chimpsprite` (por ejemplo en `cargo clippy --workspace` o
  `cargo test --workspace`) falla con "Acceso denegado" (el `.exe` está bloqueado por otro proceso),
  repetir el mismo comando con `CARGO_TARGET_DIR=target/gui-check`.
- **No ejecutar la app GUI** salvo en la Task 9 (verificación visual). Ahí, antes de correrla, se
  respalda `%APPDATA%\ChimpSprite\data\app.ron` (en Git Bash: `"$APPDATA/ChimpSprite/data/app.ron"`)
  y al terminar se restaura.

Cada commit termina con una línea en blanco y
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` (con un segundo `-m`).

## File Structure

| Archivo | Cambio |
|---|---|
| `crates/cs-ui/src/tile_ops.rs` | **Nuevo.** `encode`/`decode`, `tile_number`, `TileSpace`, `TileProxy`, conversiones de selección, `numbered_cells`, `TilesetChoice`, métodos de `Editor` (modo Tiles, pincel, estado, nueva capa, convertir). |
| `crates/cs-ui/src/tilemap_dialog.rs` | **Nuevo.** Diálogos "New Tilemap Layer" y "Convert to Tilemap" (tamaño de tile). |
| `crates/cs-ui/src/editor.rs` | `ToolKind::blocked_in_tiles`/`uses_cells`, campos del pincel, contexto sobre el proxy, puntero → celdas, traducción de la salida y de la vista previa. |
| `crates/cs-ui/src/panels.rs` | Panel Tileset (`TileThumbs`, `tileset`), `color_picker` público, barra de estado. |
| `crates/cs-ui/src/lib.rs` | Módulos, panel derecho, atajos X/Y, menú Layer, diálogos, menú View, hooks de captura. |
| `crates/cs-ui/src/canvas.rs` | Grilla de tiles, números de tile, pincel en celdas. |
| `crates/cs-ui/src/view.rs` | `ViewSettings::tile_numbers`. |
| `crates/cs-ui/src/timeline.rs` | Botón "+ Tilemap" (`TimelineState::new_tilemap`). |
| `crates/cs-ui/src/tabs.rs` | `DocState::tile_thumbs`. |
| `crates/cs-ui/src/file_actions.rs` | `is_modal` con los diálogos nuevos. |
| `crates/cs-ui/src/widgets.rs` | `outlined_text`. |
| `crates/cs-ui/src/theme.rs` | `TILE_GRID`. |
| `crates/cs-tools/src/lib.rs`, `eyedropper.rs`, `select.rs`, `move_tool.rs` | `ToolContext::tiles`; valores exactos; Move por tiles. |
| `crates/cs-core/src/tilemap.rs` | `MAX_TILESET_TILES`, `Tileset::drawn_tile`, `tile_grid_pixel`, `draw_tile_grid` (movidos desde `cs-render`). |
| `crates/cs-render/src/materialize.rs` | Usa `cs_core::tile_grid_pixel`; borra sus copias. |
| `crates/cs-io/src/aseprite.rs` | Usa `cs_core::MAX_TILESET_TILES`. |
| `crates/cs-commands/src/ops.rs`, `Cargo.toml` | `MAX_TILE_SIDE`, `convert_to_tilemap`, `convert_to_image`; `cs-render` como dev-dependency. |
| `crates/cs-agent/src/params.rs`, `ops/tilemap.rs` | `ConvertLayer`, `ConvertTo`, `Session::convert_layer`. |
| `apps/chimpsprite-mcp/src/server.rs`, `tests/stdio.rs` | Herramienta `convert_layer`. |

---

### Task 1: Proxy de celdas (`cs-ui/src/tile_ops.rs`, puro)

**Files:**
- Create: `crates/cs-ui/src/tile_ops.rs`
- Modify: `crates/cs-ui/src/lib.rs:2-25` (lista de `mod`)

**Interfaces:**
- Consumes: `cs_core::{Tile, TileGrid, Tileset, Sprite, Mask, LayerKind}`, `cs_commands::Command`,
  `cs_tools::Point` (existentes).
- Produces:
  - `pub const PROXY_LAYER: LayerId` (= `LayerId(1)`, la única capa del proxy; frame 0)
  - `pub fn encode(tile: Tile) -> Rgba`, `pub fn decode(c: Rgba) -> Tile`
  - `pub fn tile_number(index: u32, base_index: i16) -> i64`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct TileSpace { pub tile_size: (u32, u32), pub offset: (i32, i32) }`
    con `TileSpace::of(sprite: &Sprite, layer: LayerId, frame: usize) -> Option<TileSpace>`,
    `cell_at(&self, p: Point) -> Point`, `cell_origin(&self, c: Point) -> Point`,
    `proxy_size(&self, canvas: (u32, u32)) -> (u32, u32)`
  - `pub struct TileProxy { pub sprite: Sprite, pub selection: Option<Mask>, pub space: TileSpace, pub layer: LayerId, pub frame: usize }`
    con `TileProxy::build(sprite: &Sprite, layer: LayerId, frame: usize, selection: Option<&Mask>) -> Option<TileProxy>`,
    `tile_at(&self, c: Point) -> Tile`, `cells(&self, cmd: &Command) -> Option<Vec<(i32, i32, Tile)>>`,
    `unchanged(&self, cells: &[(i32, i32, Tile)]) -> bool`
  - `pub fn pixels_to_cells(pixels: &Mask, space: &TileSpace, size: (u32, u32)) -> Mask`
  - `pub fn cells_to_pixels(cells: &Mask, space: &TileSpace, canvas: (u32, u32)) -> Mask`

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/cs-ui/src/tile_ops.rs` con el encabezado y solo los tests:

```rust
//! Modo Tiles: las herramientas trabajan sobre un proxy donde cada píxel es una celda del tilemap.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cs_core::{Cel, ColorMode, ImageId, Layer, LayerId, LayerKind, Mask, PixelValue, Rgba};
    use cs_core::{Sprite, Tile, TileGrid, Tileset};

    use super::*;

    #[test]
    fn encode_and_decode_round_trip_with_flips() {
        for tile in [
            Tile::EMPTY,
            Tile::new(1, false, false, false),
            Tile::new(7, true, false, true),
            Tile::new(Tile::INDEX_MASK, true, true, true),
        ] {
            assert_eq!(decode(encode(tile)), tile);
        }
        assert_eq!(encode(Tile::EMPTY), Rgba::TRANSPARENT, "the empty tile is transparent");
        assert_eq!(encode(Tile::new(5, false, false, false)), Rgba::new(5, 0, 0, 0));
        assert_eq!(tile_number(0, 1), 0, "the empty tile is always 0");
        assert_eq!(tile_number(3, 1), 3);
        assert_eq!(tile_number(3, 0), 2);
    }

    /// Sprite `w`×`h` con un tileset de 4×4 y la capa 2 tilemap; su cel en `position` tiene una
    /// grilla de 2×1 (tile 1 y tile 2 volteado en X).
    fn tiled(w: u32, h: u32, position: (i32, i32)) -> Sprite {
        let mut s = Sprite::new(w, h, ColorMode::Rgba);
        s.tilesets.push(Tileset::new("t", (4, 4), ColorMode::Rgba));
        let mut layer = Layer::new_image(LayerId(2), "map");
        layer.kind = LayerKind::Tilemap;
        layer.tileset = Some(0);
        s.layers.push(layer);
        let mut grid = TileGrid::new(2, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        grid.set(1, 0, Tile::new(2, true, false, false));
        s.tilemaps.insert(ImageId(1), Arc::new(grid));
        s.cels.insert(
            (LayerId(2), 0),
            Cel {
                position,
                opacity: 255,
                image: ImageId(1),
            },
        );
        s
    }

    #[test]
    fn proxy_covers_the_canvas_even_when_it_is_not_a_multiple_of_the_tile() {
        let s = tiled(10, 9, (0, 0));
        let proxy = TileProxy::build(&s, LayerId(2), 0, None).unwrap();
        assert_eq!((proxy.sprite.width, proxy.sprite.height), (3, 3));
        assert_eq!(proxy.tile_at((0, 0)), Tile::new(1, false, false, false));
        assert_eq!(proxy.tile_at((1, 0)), Tile::new(2, true, false, false));
        assert_eq!(proxy.tile_at((2, 2)), Tile::EMPTY);
        assert_eq!(proxy.space.offset, (0, 0));
        assert!(
            TileProxy::build(&s, LayerId(1), 0, None).is_none(),
            "image layers have no proxy"
        );
    }

    #[test]
    fn proxy_of_an_unaligned_cel_uses_the_set_tiles_correction() {
        // Cel en (6, 3): la grilla empieza en la celda (1, 0), corrida (2, 3) píxeles.
        let s = tiled(16, 8, (6, 3));
        let proxy = TileProxy::build(&s, LayerId(2), 0, None).unwrap();
        assert_eq!((proxy.sprite.width, proxy.sprite.height), (4, 2));
        assert_eq!(proxy.space.offset, (2, 3));
        assert_eq!(proxy.tile_at((0, 0)), Tile::EMPTY);
        assert_eq!(proxy.tile_at((1, 0)), Tile::new(1, false, false, false));
        assert_eq!(proxy.tile_at((2, 0)), Tile::new(2, true, false, false));
        // El puntero sobre un tile que se ve cae en la celda de ese tile.
        assert_eq!(proxy.space.cell_at((6, 3)), (1, 0));
        assert_eq!(proxy.space.cell_at((13, 6)), (2, 0));
        assert_eq!(proxy.space.cell_at((5, 3)), (0, 0));
        assert_eq!(proxy.space.cell_at((1, 1)), (-1, -1));
        assert_eq!(proxy.space.cell_origin((1, 0)), (6, 3));
        // Lo que se escribe en la celda (1, 0) del proxy es la celda (0, 0) de la grilla.
        let cmd = cs_commands::ops::set_tiles(
            &s,
            LayerId(2),
            0,
            &[(1, 0, Tile::new(2, false, false, false))],
        )
        .unwrap();
        let mut after = s.clone();
        cmd.apply(&mut after).unwrap();
        let cel = after.cel(LayerId(2), 0).unwrap();
        assert_eq!(
            after.tilemap(cel.image).unwrap().get(0, 0),
            Some(Tile::new(2, false, false, false))
        );
    }

    #[test]
    fn selections_convert_between_pixels_and_cells() {
        let s = tiled(16, 8, (0, 0));
        // El píxel de origen de la celda (1, 0) y un píxel suelto dentro de la celda (2, 1).
        let mut sel = Mask::new(16, 8);
        sel.set(4, 0, true);
        sel.set(9, 5, true);
        let proxy = TileProxy::build(&s, LayerId(2), 0, Some(&sel)).unwrap();
        let cells = proxy.selection.clone().unwrap();
        assert_eq!(cells.points().collect::<Vec<_>>(), vec![(1, 0)]);
        let back = cells_to_pixels(&cells, &proxy.space, (16, 8));
        assert_eq!(back.bounds(), Some(((4, 0), (7, 3))));
        assert_eq!(back.points().count(), 16);
    }

    #[test]
    fn proxy_strokes_become_cells_inside_the_proxy_and_the_selection() {
        let s = tiled(16, 8, (0, 0));
        // Selección de las celdas (0, 0) y (1, 0).
        let mut sel = Mask::new(16, 8);
        for y in 0..4 {
            for x in 0..8 {
                sel.set(x, y, true);
            }
        }
        let proxy = TileProxy::build(&s, LayerId(2), 0, Some(&sel)).unwrap();
        let flipped = Tile::new(2, false, true, false);
        let brush = PixelValue::Rgba(encode(flipped));
        let cmd = Command::DrawPixels {
            layer: PROXY_LAYER,
            frame: 0,
            pixels: vec![
                (0, 0, brush),
                (1, 0, brush),
                (2, 0, brush),
                (-1, 0, brush),
                (1, 0, PixelValue::Rgba(Rgba::TRANSPARENT)),
            ],
        };
        assert_eq!(
            proxy.cells(&cmd).unwrap(),
            vec![(0, 0, flipped), (1, 0, flipped), (1, 0, Tile::EMPTY)]
        );
        assert!(proxy.cells(&Command::SetGrid(Default::default())).is_none());
        assert!(proxy.unchanged(&[(0, 0, Tile::new(1, false, false, false))]));
        assert!(!proxy.unchanged(&[(0, 0, Tile::EMPTY)]));
    }
}
```

En `crates/cs-ui/src/lib.rs`, después de `mod theme;` (línea 22), agregar:

```rust
// Lo conecta el editor en la Task 3 (hasta entonces nada lo usa fuera de los tests).
#[allow(dead_code)]
mod tile_ops;
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-ui tile_ops`
Expected: error de compilación (`encode`, `decode`, `TileProxy`, `TileSpace`… no existen).

- [ ] **Step 3: Implementar**

Arriba del módulo de tests de `crates/cs-ui/src/tile_ops.rs`, después del comentario `//!`:

```rust
use std::sync::Arc;

use cs_commands::Command;
use cs_core::{Cel, ColorMode, Image, ImageId, LayerId, LayerKind, Mask, PixelValue, Rgba, Sprite};
use cs_core::Tile;
use cs_tools::Point;

/// Única capa del proxy (frame 0).
pub const PROXY_LAYER: LayerId = LayerId(1);
const PROXY_IMAGE: ImageId = ImageId(1);

/// Un tile como píxel del proxy: los 4 bytes del valor (little endian). El tile 0 es transparente.
pub fn encode(tile: Tile) -> Rgba {
    let [r, g, b, a] = tile.0.to_le_bytes();
    Rgba::new(r, g, b, a)
}

pub fn decode(c: Rgba) -> Tile {
    Tile(u32::from_le_bytes([c.r, c.g, c.b, c.a]))
}

/// Número que ve el usuario: el vacío es 0; el tile `index` es `index + base_index - 1`.
pub fn tile_number(index: u32, base_index: i16) -> i64 {
    if index == 0 {
        0
    } else {
        index as i64 + base_index as i64 - 1
    }
}

/// Geometría de las celdas de una capa tilemap en el lienzo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileSpace {
    pub tile_size: (u32, u32),
    /// Resto de la posición del cel respecto del tamaño de tile (0 si está alineado o no hay cel):
    /// la celda `(cx, cy)` empieza en el píxel `(cx·tw + offset.0, cy·th + offset.1)`, como en
    /// `ops::set_tiles`.
    pub offset: (i32, i32),
}

impl TileSpace {
    /// `None` si la capa no es tilemap o su tileset no existe.
    pub fn of(sprite: &Sprite, layer: LayerId, frame: usize) -> Option<TileSpace> {
        let l = sprite.layer(layer)?;
        if l.kind != LayerKind::Tilemap {
            return None;
        }
        let tileset = sprite.tilesets.get(l.tileset? as usize)?;
        let (tw, th) = tileset.tile_size;
        if tw == 0 || th == 0 {
            return None;
        }
        let offset = sprite.cel(layer, frame).map_or((0, 0), |c| {
            (
                c.position.0.rem_euclid(tw as i32),
                c.position.1.rem_euclid(th as i32),
            )
        });
        Some(TileSpace {
            tile_size: (tw, th),
            offset,
        })
    }

    /// Celda que contiene el píxel `p` del lienzo.
    pub fn cell_at(&self, p: Point) -> Point {
        let (tw, th) = (self.tile_size.0 as i32, self.tile_size.1 as i32);
        (
            (p.0 - self.offset.0).div_euclid(tw),
            (p.1 - self.offset.1).div_euclid(th),
        )
    }

    /// Píxel del lienzo donde empieza la celda `c`.
    pub fn cell_origin(&self, c: Point) -> Point {
        let (tw, th) = (self.tile_size.0 as i32, self.tile_size.1 as i32);
        (c.0 * tw + self.offset.0, c.1 * th + self.offset.1)
    }

    /// Celdas que cubren un lienzo de `canvas` píxeles: `ceil(ancho / tw) × ceil(alto / th)`.
    pub fn proxy_size(&self, canvas: (u32, u32)) -> (u32, u32) {
        (
            canvas.0.div_ceil(self.tile_size.0),
            canvas.1.div_ceil(self.tile_size.1),
        )
    }
}

/// Sprite donde cada píxel es una celda de la capa tilemap: lo que ven las herramientas en modo
/// Tiles.
pub struct TileProxy {
    /// Una capa de imagen (`PROXY_LAYER`) y un frame; cada píxel es `encode(tile)`.
    pub sprite: Sprite,
    /// Selección del editor en celdas: una celda está seleccionada si su píxel de origen lo está.
    pub selection: Option<Mask>,
    pub space: TileSpace,
    /// Capa y frame reales.
    pub layer: LayerId,
    pub frame: usize,
}

impl TileProxy {
    /// Proxy de la capa tilemap `layer` en `frame`, con `selection` (en píxeles) pasada a celdas.
    /// `None` si la capa no es tilemap o su tileset no existe.
    pub fn build(
        sprite: &Sprite,
        layer: LayerId,
        frame: usize,
        selection: Option<&Mask>,
    ) -> Option<TileProxy> {
        let space = TileSpace::of(sprite, layer, frame)?;
        let (pw, ph) = space.proxy_size((sprite.width, sprite.height));
        let mut image = Image::new(pw, ph, ColorMode::Rgba);
        let found = sprite
            .cel(layer, frame)
            .and_then(|c| Some((c.position, sprite.tilemap(c.image)?)));
        if let Some((position, grid)) = found {
            let (tw, th) = (space.tile_size.0 as i32, space.tile_size.1 as i32);
            // Celda del lienzo donde empieza la grilla (la misma corrección que `ops::set_tiles`).
            let origin = (position.0.div_euclid(tw), position.1.div_euclid(th));
            for cy in 0..ph as i32 {
                for cx in 0..pw as i32 {
                    let tile = u32::try_from(cx - origin.0)
                        .ok()
                        .zip(u32::try_from(cy - origin.1).ok())
                        .and_then(|(gx, gy)| grid.get(gx, gy))
                        .unwrap_or(Tile::EMPTY);
                    image
                        .set(cx as u32, cy as u32, PixelValue::Rgba(encode(tile)))
                        .expect("in bounds and RGBA");
                }
            }
        }
        let mut proxy = Sprite::new(pw, ph, ColorMode::Rgba);
        proxy.images.insert(PROXY_IMAGE, Arc::new(image));
        proxy.cels.insert(
            (PROXY_LAYER, 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: PROXY_IMAGE,
            },
        );
        Some(TileProxy {
            selection: selection.map(|m| pixels_to_cells(m, &space, (pw, ph))),
            sprite: proxy,
            space,
            layer,
            frame,
        })
    }

    /// Tile de la celda `c` (vacío fuera del proxy).
    pub fn tile_at(&self, c: Point) -> Tile {
        match self.sprite.pixel(PROXY_LAYER, 0, c.0, c.1) {
            PixelValue::Rgba(v) => decode(v),
            PixelValue::Index(_) => Tile::EMPTY,
        }
    }

    /// Celdas que pide un `DrawPixels` del proxy, con el tile decodificado: solo las que caen
    /// dentro del proxy y de la selección. `None` si el comando no es un `DrawPixels`.
    pub fn cells(&self, cmd: &Command) -> Option<Vec<(i32, i32, Tile)>> {
        let Command::DrawPixels { pixels, .. } = cmd else {
            return None;
        };
        let (w, h) = (self.sprite.width as i32, self.sprite.height as i32);
        Some(
            pixels
                .iter()
                .filter(|&&(x, y, _)| {
                    x >= 0
                        && y >= 0
                        && x < w
                        && y < h
                        && self.selection.as_ref().is_none_or(|m| m.contains(x, y))
                })
                .map(|&(x, y, v)| {
                    let tile = match v {
                        PixelValue::Rgba(c) => decode(c),
                        PixelValue::Index(_) => Tile::EMPTY,
                    };
                    (x, y, tile)
                })
                .collect(),
        )
    }

    /// `true` si poner `cells` no cambiaría ningún tile (incluido no tener celdas).
    pub fn unchanged(&self, cells: &[(i32, i32, Tile)]) -> bool {
        cells.iter().all(|&(x, y, t)| self.tile_at((x, y)) == t)
    }
}

/// Selección en píxeles → celdas (`size` = tamaño del proxy): una celda está seleccionada si su
/// píxel de origen lo está.
pub fn pixels_to_cells(pixels: &Mask, space: &TileSpace, size: (u32, u32)) -> Mask {
    let cells = (0..size.1 as i32).flat_map(|cy| (0..size.0 as i32).map(move |cx| (cx, cy)));
    Mask::from_points(
        size.0,
        size.1,
        cells.filter(|&c| {
            let (x, y) = space.cell_origin(c);
            pixels.contains(x, y)
        }),
    )
}

/// Selección en celdas → píxeles del lienzo: cada celda cubre su rectángulo de `tw × th`.
pub fn cells_to_pixels(cells: &Mask, space: &TileSpace, canvas: (u32, u32)) -> Mask {
    let space = *space;
    let (tw, th) = (space.tile_size.0 as i32, space.tile_size.1 as i32);
    let points = cells.points().flat_map(move |c| {
        let (x0, y0) = space.cell_origin(c);
        (0..th).flat_map(move |dy| (0..tw).map(move |dx| (x0 + dx, y0 + dy)))
    });
    Mask::from_points(canvas.0, canvas.1, points)
}
```

Si `rustfmt` separa o junta los `use cs_core::…`, aceptar lo que deje `cargo fmt`.

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo fmt --all && cargo test -p cs-ui tile_ops`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src/tile_ops.rs crates/cs-ui/src/lib.rs
git commit -m "feat(ui): tile cell proxy for Tiles mode" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Modo Tiles en `cs-tools` (valores exactos y Move por tiles)

**Files:**
- Modify: `crates/cs-tools/src/lib.rs:34-61` (`ToolContext`) y `:166-185` (`test_util::ctx`)
- Modify: `crates/cs-tools/src/eyedropper.rs:36-41` (`preview`) y tests
- Modify: `crates/cs-tools/src/select.rs:270-271` (varita) y tests
- Modify: `crates/cs-tools/src/move_tool.rs:27-33` (`move_output`) y tests
- Modify: `crates/cs-ui/src/editor.rs:939-940` (`context()`: campo nuevo)

**Interfaces:**
- Consumes: `Tileset::tile_size`, `Layer::tileset` (núcleo).
- Produces:
  - `ToolContext::tiles: bool` — `true` cuando `sprite` es el proxy de celdas: `Eyedropper` devuelve
    en `pick` el valor exacto de la capa (`sprite.pixel(layer, frame, p)`) y `MagicWand` selecciona
    los píxeles con valor exactamente igual al de la semilla (sin tolerancia ni composición).
  - `MoveTool` sobre una capa tilemap con tileset: `SetCel` con el delta redondeado al múltiplo de
    `tw`/`th` más cercano (`round`, mitades lejos de cero), ignorando la selección; nada si el delta
    redondeado es (0, 0), si no hay cel o si no hay tileset.

- [ ] **Step 1: Escribir los tests que fallan**

En los tests de `crates/cs-tools/src/eyedropper.rs`:

```rust
    #[test]
    fn tiles_mode_picks_the_exact_layer_value() {
        use cs_core::{PixelValue, Rgba};
        let mut s = sprite();
        // Un tile codificado tiene alfa 0: lo visible sería transparente.
        let code = PixelValue::Rgba(Rgba::new(5, 0, 0, 0));
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, code)],
        }
        .apply(&mut s)
        .unwrap();
        let mut c = ctx(&s);
        let mut e = Eyedropper::new();
        e.press(&c, (1, 1));
        assert_eq!(
            e.release(&c).pick,
            Some(PixelValue::Rgba(Rgba::TRANSPARENT))
        );
        c.tiles = true;
        e.press(&c, (1, 1));
        assert_eq!(e.release(&c).pick, Some(code));
    }
```

En los tests de `crates/cs-tools/src/select.rs`:

```rust
    #[test]
    fn magic_wand_in_tiles_mode_compares_exact_values() {
        let mut s = sprite();
        let code = |i| PixelValue::Rgba(Rgba::new(i, 0, 0, 0));
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, code(1)), (1, 0, code(1)), (2, 0, code(2))],
        }
        .apply(&mut s)
        .unwrap();
        let mut c = ctx(&s);
        c.contiguous = false;
        let mut t = MagicWand::new();
        t.press(&c, (0, 0));
        assert_eq!(
            mask_of(t.release(&c)).points().count(),
            64,
            "by color, every alpha-0 value is the same"
        );
        c.tiles = true;
        t.press(&c, (0, 0));
        assert_eq!(
            mask_of(t.release(&c)).points().collect::<Vec<_>>(),
            vec![(0, 0), (1, 0)]
        );
    }
```

En los tests de `crates/cs-tools/src/move_tool.rs`, renombrar `move_does_nothing_on_tilemap_layers`
a `move_does_nothing_on_tilemap_layers_without_tileset` (el cuerpo no cambia) y agregar:

```rust
    #[test]
    fn move_snaps_tilemap_cels_to_whole_tiles() {
        use std::sync::Arc;

        use cs_core::{ImageId, TileGrid, Tileset};
        let mut s = sprite();
        s.tilesets.push(Tileset::new("t", (4, 2), s.color_mode));
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        s.tilemaps.insert(ImageId(1), Arc::new(TileGrid::new(1, 1)));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: ImageId(1),
            },
        );
        let at = |out: ToolOutput| match out.command {
            Some(Command::SetCel { cel: Some(c), .. }) => c.position,
            other => panic!("expected SetCel, got {other:?}"),
        };
        assert_eq!(
            at(gesture(&ctx(&s), (0, 0), (3, 1))),
            (4, 2),
            "3 px → one 4 px tile; 1 px → one 2 px tile (halves round away from zero)"
        );
        assert_eq!(
            gesture(&ctx(&s), (0, 0), (1, 0)),
            ToolOutput::default(),
            "less than half a tile"
        );
        assert_eq!(at(gesture(&ctx(&s), (0, 0), (-6, 0))), (-8, 0));
        // Con selección también se mueve el cel entero.
        let mask = Mask::from_rect(8, 8, (0, 0), (1, 1));
        let mut c = ctx(&s);
        c.selection = Some(&mask);
        let out = gesture(&c, (0, 0), (4, 0));
        assert!(out.selection.is_none(), "the selection does not move");
        assert_eq!(at(out), (4, 0));
    }
```

En `crates/cs-tools/src/lib.rs`, en `test_util::ctx`, agregar `tiles: false,` después de
`selected_slice: None,` (línea 184).

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-tools`
Expected: error de compilación (`ToolContext` no tiene el campo `tiles`).

- [ ] **Step 3: Implementar**

`crates/cs-tools/src/lib.rs`, al final de `struct ToolContext` (después de `selected_slice`):

```rust
    /// Modo Tiles: `sprite` es el proxy de celdas del editor (cada píxel codifica un tile). El
    /// cuentagotas y la varita leen el valor exacto de la capa en vez del color visible.
    pub tiles: bool,
```

`crates/cs-tools/src/eyedropper.rs`, en `preview`, reemplazar el brazo `Some(p) if ctx.in_canvas(p)`
por:

```rust
            Some(p) if ctx.in_canvas(p) => ToolOutput {
                pick: Some(if ctx.tiles {
                    // Cada píxel del proxy es un tile: se toma tal cual (alfa incluido).
                    ctx.sprite.pixel(ctx.layer, ctx.frame, p.0, p.1)
                } else {
                    visible_value(ctx.sprite, ctx.frame, p)
                }),
                ..Default::default()
            },
```

`crates/cs-tools/src/select.rs`, en `MagicWand::preview`, reemplazar la línea 271
(`let matches = |x: i32, y: i32| distance(color(x, y), seed) <= ctx.tolerance;`) por:

```rust
        // En modo Tiles se comparan tiles exactos: el valor de la capa, sin tolerancia.
        let raw = |x: i32, y: i32| s.pixel(ctx.layer, ctx.frame, x, y);
        let seed_raw = raw(p.0, p.1);
        let matches = |x: i32, y: i32| {
            if ctx.tiles {
                raw(x, y) == seed_raw
            } else {
                distance(color(x, y), seed) <= ctx.tolerance
            }
        };
```

`crates/cs-tools/src/move_tool.rs`, reemplazar el bloque de las líneas 27-33 (comentario
"Las capas tilemap se editan por tiles…" y su `if`) por:

```rust
    // Capas tilemap: el cel entero se mueve en pasos de un tile (la selección no aplica).
    if let Some(layer) = sprite
        .layer(ctx.layer)
        .filter(|l| l.kind == cs_core::LayerKind::Tilemap)
    {
        let Some(tileset) = layer.tileset.and_then(|i| sprite.tilesets.get(i as usize)) else {
            return ToolOutput::default();
        };
        let snap = |d: i32, size: u32| {
            let size = size.max(1) as i32;
            (d as f64 / size as f64).round() as i32 * size
        };
        let (dx, dy) = (snap(dx, tileset.tile_size.0), snap(dy, tileset.tile_size.1));
        return match sprite.cel(ctx.layer, ctx.frame) {
            Some(cel) if (dx, dy) != (0, 0) => ToolOutput::command(Command::SetCel {
                layer: ctx.layer,
                frame: ctx.frame,
                cel: Some(Cel {
                    position: (cel.position.0 + dx, cel.position.1 + dy),
                    ..*cel
                }),
            }),
            _ => ToolOutput::default(),
        };
    }
```

`crates/cs-ui/src/editor.rs`, en `fn context`, agregar `tiles: false,` después de
`selected_slice: self.slice,` (línea 939).

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo test -p cs-tools && cargo check --workspace --all-targets`
Expected: PASS; el workspace compila.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-tools crates/cs-ui/src/editor.rs
git commit -m "feat(tools): exact values in Tiles mode and tile-stepped Move" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Editor en modo Tiles (herramientas sobre el proxy, pincel y barra de estado)

**Files:**
- Modify: `crates/cs-ui/src/editor.rs` (imports 4-15; `ToolKind` después de `uses_brush` ~166-178;
  campos de `Editor` después de `merge` ~460; `Editor::new` ~503; `context` 912-941; `with_tool`
  944-953; `pointer_hover` 818-828; `pointer_press` 962-1020; `pointer_drag` 1034-1041;
  `pointer_release` 1079-1098; `apply_output` 1150-1177; `view` 1189-1260; tests)
- Modify: `crates/cs-ui/src/tile_ops.rs` (`impl Editor`, `test_util`)
- Modify: `crates/cs-ui/src/lib.rs` (quitar el `#[allow(dead_code)]` de `mod tile_ops;`)
- Modify: `crates/cs-ui/src/panels.rs:540-551` (`status_bar`)

**Interfaces:**
- Consumes: `TileProxy`, `TileSpace`, `encode`/`decode`, `tile_number`, `cells_to_pixels`,
  `PROXY_LAYER` (Task 1); `ToolContext::tiles`, Move por tiles (Task 2); `ops::set_tiles` (núcleo).
- Produces:
  - `ToolKind::blocked_in_tiles(self) -> bool`, `ToolKind::uses_cells(self) -> bool`
  - `Editor::tiles_mode(&self) -> bool`, `Editor::tile_space(&self) -> Option<TileSpace>`,
    `Editor::active_tileset(&self) -> Option<(u32, &Tileset)>`
  - `Editor::brush_tile(&self) -> Tile` (tile 1 —o el vacío si el tileset no tiene tiles— cuando el
    pincel guardado es de otro artboard/tileset o está fuera de rango; sin flip D si los tiles no
    son cuadrados), `Editor::set_brush_tile(&mut self, tile: Tile)`
  - `Editor::tiles_status(&self) -> Option<String>`: `"Not available in Tiles mode"`,
    `"Tiles mode: the tilemap would exceed its size limit"` o `"Tiles mode: painting with tile N"`.
  - Campos `pub(crate) brush_tile: Tile`, `pub(crate) brush_tileset: Option<(ArtboardId, u32)>`,
    `pub(crate) tile_limit: bool`.
  - `#[cfg(test)] tile_ops::test_util::{tile_editor() -> Editor, tile_at(&Editor, Point) -> Tile, t(u32) -> Tile}`

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/cs-ui/src/tile_ops.rs`, al final del archivo:

```rust
#[cfg(test)]
pub(crate) mod test_util {
    use std::sync::Arc;

    use cs_core::{ColorMode, Image, LayerId, PixelValue, Rgba, Sprite, Tile, Tileset};
    use cs_tools::Point;

    use super::TileProxy;
    use crate::editor::Editor;

    /// Editor sobre un sprite de 16×16 con un tileset de 4×4 (tiles 1 y 2, distintos) y la capa
    /// tilemap 2 activa, encima de la capa 1.
    pub fn tile_editor() -> Editor {
        let mut s = Sprite::new(16, 16, ColorMode::Rgba);
        let mut ts = Tileset::new("t", (4, 4), ColorMode::Rgba);
        for r in [255u8, 0] {
            let mut img = Image::new(4, 4, ColorMode::Rgba);
            img.set(0, 0, PixelValue::Rgba(Rgba::rgb(r, 0, 255 - r)))
                .unwrap();
            ts.tiles.push(Arc::new(img));
        }
        s.tilesets.push(ts);
        let add = cs_commands::ops::new_tilemap_layer(&s, Some(LayerId(1)), 0);
        add.apply(&mut s).unwrap();
        let mut e = Editor::with_sprite(s);
        e.select_layer(LayerId(2));
        e
    }

    /// Tile de la celda `c` de la capa activa.
    pub fn tile_at(e: &Editor, c: Point) -> Tile {
        TileProxy::build(e.sprite(), e.layer, e.frame, None)
            .expect("tiles mode")
            .tile_at(c)
    }

    pub fn t(index: u32) -> Tile {
        Tile::new(index, false, false, false)
    }
}
```

En el módulo de tests de `crates/cs-ui/src/editor.rs`, después de `use super::*;`:

```rust
    use crate::tile_ops::test_util::{t, tile_at, tile_editor};
    use cs_commands::ops;
```

y estos tests (al final del módulo):

```rust
    #[test]
    fn tiles_mode_pencil_paints_the_brush_tile_with_flips_and_previews_it() {
        let mut e = tile_editor();
        assert!(e.tiles_mode());
        assert_eq!(e.brush_tile(), t(1), "the brush starts on tile 1");
        let flipped = Tile::new(2, true, false, false);
        e.set_brush_tile(flipped);
        e.pointer_press((5.5, 9.5), false, false, 0.1); // celda (1, 2)
        let preview = e.view().document.artboards[0].sprite.clone();
        let shown = TileProxy::build(&preview, LayerId(2), 0, None).unwrap();
        assert_eq!(shown.tile_at((1, 2)), flipped, "the preview shows the tile");
        assert!(e.sprite().cel(LayerId(2), 0).is_none(), "preview only");
        e.pointer_release();
        assert_eq!(tile_at(&e, (1, 2)), flipped);
        assert_eq!(tile_at(&e, (0, 0)), Tile::EMPTY);
        e.undo();
        assert!(e.sprite().cel(LayerId(2), 0).is_none());
    }

    #[test]
    fn tiles_mode_shapes_are_exact_in_cells() {
        let mut e = tile_editor();
        e.set_tool(ToolKind::Line);
        drag_local(&mut e, (1, 1), (13, 1), false, false);
        for cx in 0..4 {
            assert_eq!(tile_at(&e, (cx, 0)), t(1));
        }
        assert_eq!(tile_at(&e, (0, 1)), Tile::EMPTY);
        e.undo();
        e.set_tool(ToolKind::Rectangle);
        e.options.filled = true;
        drag_local(&mut e, (1, 1), (9, 9), false, false);
        let proxy = TileProxy::build(e.sprite(), LayerId(2), 0, None).unwrap();
        let filled: Vec<Point> = (0..4)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .filter(|&c| proxy.tile_at(c) == t(1))
            .collect();
        assert_eq!(
            filled,
            vec![
                (0, 0),
                (1, 0),
                (2, 0),
                (0, 1),
                (1, 1),
                (2, 1),
                (0, 2),
                (1, 2),
                (2, 2)
            ]
        );
    }

    #[test]
    fn tiles_mode_bucket_fills_equal_tiles_contiguous_or_not() {
        let mut e = tile_editor();
        let flipped = Tile::new(1, true, false, false);
        let cells = [
            (0, 0, t(1)),
            (1, 0, t(1)),
            (2, 0, t(2)),
            (3, 0, t(1)),
            (0, 1, flipped),
        ];
        let cmd = ops::set_tiles(e.sprite(), LayerId(2), 0, &cells).unwrap();
        e.execute(cmd);
        let row = |e: &Editor| (0..4).map(|x| tile_at(e, (x, 0))).collect::<Vec<_>>();
        e.set_tool(ToolKind::Bucket);
        e.set_brush_tile(t(2));
        click(&mut e, (5, 1), false, false); // celda (1, 0)
        assert_eq!(row(&e), vec![t(2), t(2), t(2), t(1)]);
        assert_eq!(tile_at(&e, (0, 1)), flipped, "flips count: not the same tile");
        e.undo();
        e.options.contiguous = false;
        click(&mut e, (5, 1), false, false);
        assert_eq!(row(&e), vec![t(2); 4]);
        assert_eq!(tile_at(&e, (0, 1)), flipped);
        assert_eq!(tile_at(&e, (1, 1)), Tile::EMPTY);
    }

    #[test]
    fn tiles_mode_eyedropper_picks_the_tile_and_its_flips() {
        let mut e = tile_editor();
        let flipped = Tile::new(2, false, true, false);
        let cmd = ops::set_tiles(e.sprite(), LayerId(2), 0, &[(2, 0, flipped)]).unwrap();
        e.execute(cmd);
        e.set_tool(ToolKind::Eyedropper);
        click(&mut e, (9, 1), false, false);
        assert_eq!(e.brush_tile(), flipped);
        click(&mut e, (1, 13), false, false);
        assert_eq!(e.brush_tile(), Tile::EMPTY, "clicking an empty cell picks the empty tile");
    }

    #[test]
    fn tiles_mode_eraser_leaves_the_empty_tile() {
        let mut e = tile_editor();
        click(&mut e, (1, 1), false, false);
        assert_eq!(tile_at(&e, (0, 0)), t(1));
        e.set_tool(ToolKind::Eraser);
        click(&mut e, (2, 2), false, false);
        assert_eq!(tile_at(&e, (0, 0)), Tile::EMPTY);
    }

    #[test]
    fn tiles_mode_marquee_selects_whole_cells_and_limits_painting() {
        let mut e = tile_editor();
        e.set_tool(ToolKind::RectSelect);
        drag_local(&mut e, (1, 1), (5, 5), false, false);
        let sel = e.selection.clone().unwrap();
        assert_eq!(sel.bounds(), Some(((0, 0), (7, 7))));
        assert_eq!(sel.points().count(), 64);
        e.set_tool(ToolKind::Pencil);
        drag_local(&mut e, (1, 1), (13, 1), false, false);
        assert_eq!(tile_at(&e, (1, 0)), t(1));
        assert_eq!(tile_at(&e, (2, 0)), Tile::EMPTY, "outside the selection");
    }

    #[test]
    fn tiles_mode_move_steps_by_whole_tiles() {
        let mut e = tile_editor();
        click(&mut e, (1, 1), false, false); // tile 1 en la celda (0, 0)
        e.set_tool(ToolKind::Move);
        drag_local(&mut e, (1, 1), (2, 1), false, false); // 1 px: menos de medio tile
        assert_eq!(e.sprite().cel(LayerId(2), 0).unwrap().position, (0, 0));
        drag_local(&mut e, (1, 1), (4, 2), false, false); // 3 px → 1 tile; 1 px → 0
        assert_eq!(e.sprite().cel(LayerId(2), 0).unwrap().position, (4, 0));
        assert_eq!(tile_at(&e, (1, 0)), t(1));
        assert_eq!(e.layer, LayerId(2), "Move keeps the tilemap layer");
    }

    #[test]
    fn tiles_mode_blocks_spray_and_text() {
        let mut e = tile_editor();
        e.set_tool(ToolKind::Spray);
        e.pointer_press((4.5, 4.5), false, false, 0.1);
        e.tool_tick(1000.0);
        e.pointer_release();
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Not available in Tiles mode")
        );
        e.set_tool(ToolKind::Text);
        click(&mut e, (1, 1), false, false);
        assert!(e.text.is_none());
        assert!(!e.history.can_undo(), "no commands");
        e.set_tool(ToolKind::Pencil);
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Tiles mode: painting with tile 1")
        );
        // Numeración del usuario: índice + base_index - 1.
        let mut ts = e.sprite().tilesets[0].clone();
        ts.base_index = 5;
        e.execute(Command::SetTileset {
            index: 0,
            tileset: Some(ts),
        });
        e.set_brush_tile(t(2));
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Tiles mode: painting with tile 6")
        );
    }

    #[test]
    fn tiles_mode_reports_the_grid_limit() {
        use std::sync::Arc;

        use cs_core::{Image, Tileset};
        let mut s = Sprite::new(5000, 1, ColorMode::Rgba);
        let mut ts = Tileset::new("t", (1, 1), ColorMode::Rgba);
        ts.tiles.push(Arc::new(Image::new(1, 1, ColorMode::Rgba)));
        s.tilesets.push(ts);
        let add = ops::new_tilemap_layer(&s, Some(LayerId(1)), 0);
        add.apply(&mut s).unwrap();
        let mut e = Editor::with_sprite(s);
        e.select_layer(LayerId(2));
        e.set_tool(ToolKind::Line);
        // 5000 celdas de ancho: más que MAX_TILEMAP_SIDE (4096).
        drag_local(&mut e, (0, 0), (4999, 0), false, false);
        assert!(e.sprite().cel(LayerId(2), 0).is_none());
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Tiles mode: the tilemap would exceed its size limit")
        );
    }
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-ui tiles_mode`
Expected: error de compilación (`tiles_mode`, `brush_tile`, `set_brush_tile`, `tiles_status` no
existen).

- [ ] **Step 3: Implementar**

**`crates/cs-ui/src/lib.rs`:** quitar el comentario y el `#[allow(dead_code)]` de arriba de
`mod tile_ops;` (queda solo `mod tile_ops;`).

**`crates/cs-ui/src/editor.rs`:**

1. Imports: agregar `Tile` a `use cs_core::{…}` y la línea `use crate::tile_ops::{self, TileProxy};`
   después de los `use cs_tools::…`.

2. En `impl ToolKind`, después de `uses_brush`:

```rust
    /// Herramientas que no actúan sobre una capa tilemap en modo Tiles (2A).
    pub fn blocked_in_tiles(self) -> bool {
        matches!(
            self,
            ToolKind::Spray
                | ToolKind::Jumble
                | ToolKind::Gradient
                | ToolKind::Text
                | ToolKind::Lasso
                | ToolKind::PolygonLasso
        )
    }

    /// Herramientas que en modo Tiles trabajan sobre el proxy de celdas (Move, Slice y Artboard
    /// siguen en píxeles).
    pub fn uses_cells(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Eraser
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Bucket
                | ToolKind::Eyedropper
                | ToolKind::RectSelect
                | ToolKind::EllipseSelect
                | ToolKind::MagicWand
        )
    }
```

3. Después de `struct DocView` (antes de `type MergeTarget`):

```rust
/// Qué hace un trazo en modo Tiles.
enum TilesEdit {
    Apply(Command),
    /// Nada cambia (o la capa está bloqueada).
    Nothing,
    /// La grilla pasaría el límite de tamaño de `ops::set_tiles`.
    TooLarge,
}
```

4. Al final de `struct Editor` (después de `merge`):

```rust
    /// Tile con el que pintan las herramientas en modo Tiles (índice y flips). Ver `brush_tile()`.
    pub(crate) brush_tile: Tile,
    /// Artboard y tileset para los que se eligió `brush_tile`; en otro tileset el pincel es el 1.
    pub(crate) brush_tileset: Option<(ArtboardId, u32)>,
    /// El último trazo en modo Tiles no se aplicó porque la grilla pasaría el límite de tamaño.
    pub(crate) tile_limit: bool,
```

   y en `Editor::new`, después de `slice: None,`:

```rust
            brush_tile: Tile::new(1, false, false, false),
            brush_tileset: None,
            tile_limit: false,
```

5. Reemplazar `fn context` y `fn with_tool` por:

```rust
    /// Shift restringe la forma, salvo en selección si al empezar eligió el modo Add.
    fn constrain(&self) -> bool {
        self.options.constrain && !(self.active_tool().is_selection() && self.shift_at_press)
    }

    fn context(&self) -> ToolContext<'_> {
        let secondary = if self.painting_secondary {
            self.primary
        } else {
            self.secondary
        };
        ToolContext {
            sprite: self.sprite(),
            layer: self.layer,
            frame: self.frame,
            color: self.color(),
            secondary,
            brush_size: self.options.brush_size,
            pixel_perfect: self.options.pixel_perfect,
            filled: self.options.filled,
            contiguous: self.options.contiguous,
            constrain: self.constrain(),
            selection: self.selection.as_ref(),
            tolerance: self.options.tolerance,
            sample_all: self.options.sample_all,
            spray_width: self.options.spray_width,
            spray_speed: self.options.spray_speed,
            gradient_shape: self.options.gradient_shape,
            dither: self.options.dither,
            selected_slice: self.slice,
            tiles: false,
        }
    }

    /// Contexto sobre el proxy de celdas: los dos colores son el tile del pincel codificado, el
    /// pincel se mide en celdas y la selección es la del proxy. Pixel-perfect no aplica.
    fn tiles_context<'a>(&'a self, proxy: &'a TileProxy) -> ToolContext<'a> {
        let brush = PixelValue::Rgba(tile_ops::encode(self.brush_tile()));
        ToolContext {
            sprite: &proxy.sprite,
            layer: tile_ops::PROXY_LAYER,
            frame: 0,
            color: brush,
            secondary: brush,
            brush_size: self.options.brush_size,
            pixel_perfect: false,
            filled: self.options.filled,
            contiguous: self.options.contiguous,
            constrain: self.constrain(),
            selection: proxy.selection.as_ref(),
            tolerance: 0,
            sample_all: false,
            spray_width: self.options.spray_width,
            spray_speed: self.options.spray_speed,
            gradient_shape: self.options.gradient_shape,
            dither: self.options.dither,
            selected_slice: None,
            tiles: true,
        }
    }

    /// Proxy de celdas para `kind` si la capa activa está en modo Tiles y la herramienta trabaja
    /// en celdas.
    fn cell_proxy(&self, kind: ToolKind) -> Option<TileProxy> {
        if !kind.uses_cells() || !self.tiles_mode() {
            return None;
        }
        TileProxy::build(self.sprite(), self.layer, self.frame, self.selection.as_ref())
    }

    /// Punto de mundo → punto de la herramienta: píxel local o, en modo Tiles, celda.
    fn tool_point(&self, world: (f32, f32)) -> Point {
        let p = self.to_local(self.snapped(world));
        match self.tile_space().filter(|_| self.active_tool().uses_cells()) {
            Some(space) => space.cell_at(p),
            None => p,
        }
    }

    /// Corre `f` sobre la herramienta de sprite activa con un contexto que toma prestado el
    /// documento (o el proxy de celdas en modo Tiles).
    fn with_tool<R: Default>(&mut self, f: impl FnOnce(&mut dyn Tool, &ToolContext) -> R) -> R {
        let kind = self.active_tool();
        let proxy = self.cell_proxy(kind);
        let mut tools = std::mem::replace(&mut self.tools, SpriteTools::new());
        let result = tools
            .get_mut(kind)
            .map(|tool| match &proxy {
                Some(proxy) => f(tool, &self.tiles_context(proxy)),
                None => f(tool, &self.context()),
            })
            .unwrap_or_default();
        self.tools = tools;
        result
    }
```

6. `pointer_hover`: reemplazar `let p = self.to_local(self.snapped(world));` por
   `let p = self.tool_point(world);`.

7. `pointer_press`: después de `self.stop();` agregar `self.tile_limit = false;`. Reemplazar
   `let p = self.to_local(self.snapped(world));` por:

```rust
        let p = self.tool_point(world);
        // En modo Tiles algunas herramientas no actúan (la barra de estado lo avisa).
        if self.tiles_mode() && self.active_tool().blocked_in_tiles() {
            return;
        }
```

   y en el bloque "Mover sin selección: cambia a la capa…", agregar `&& !self.tiles_mode()` a la
   condición (después de `&& self.selection.is_none()`), con el comentario
   `// En modo Tiles Move se queda en la capa tilemap.`

8. `pointer_drag`: reemplazar `let p = self.to_local(self.snapped(world));` por
   `let p = self.tool_point(world);`.

9. `pointer_release`: reemplazar `let kind = self.active_tool();` por:

```rust
        let kind = self.active_tool();
        if self.tiles_mode() && kind.blocked_in_tiles() {
            self.temp_eyedropper = false;
            return;
        }
```

10. Al principio de `apply_output`:

```rust
        if let Some(proxy) = self.cell_proxy(kind) {
            self.apply_tiles_output(kind, out, &proxy);
            return;
        }
```

    y después de `apply_output`:

```rust
    /// Resultado de una herramienta sobre el proxy → tiles y selección del sprite real.
    fn apply_tiles_output(&mut self, kind: ToolKind, out: ToolOutput, proxy: &TileProxy) {
        let mut applied = true;
        if let Some(cmd) = &out.command {
            match self.tiles_edit(proxy, cmd) {
                TilesEdit::Apply(cmd) => {
                    applied = self.execute(cmd);
                    self.tile_limit = false;
                }
                TilesEdit::Nothing => {}
                TilesEdit::TooLarge => {
                    self.tile_limit = true;
                    applied = false;
                }
            }
        }
        if let Some(PixelValue::Rgba(code)) = out.pick {
            self.set_brush_tile(tile_ops::decode(code));
        }
        if let Some(shape) = out.selection
            && applied
        {
            let s = self.sprite();
            let canvas = (s.width, s.height);
            let shape = shape.map(|m| tile_ops::cells_to_pixels(&m, &proxy.space, canvas));
            let combined = self.combine_selection(kind, shape);
            self.set_selection(combined);
        }
    }

    /// Traduce un `DrawPixels` del proxy a `ops::set_tiles` sobre la capa real.
    fn tiles_edit(&self, proxy: &TileProxy, cmd: &Command) -> TilesEdit {
        let locked = self.sprite().layer(proxy.layer).is_none_or(|l| l.locked);
        let Some(cells) = proxy
            .cells(cmd)
            .filter(|cells| !locked && !proxy.unchanged(cells))
        else {
            return TilesEdit::Nothing;
        };
        match cs_commands::ops::set_tiles(self.sprite(), proxy.layer, proxy.frame, &cells) {
            Some(cmd) => TilesEdit::Apply(cmd),
            None => TilesEdit::TooLarge,
        }
    }

    /// Vista previa de un gesto en modo Tiles: el trazo pasa por la misma traducción que al soltar.
    fn tiles_view(&self, kind: ToolKind, out: ToolOutput, proxy: &TileProxy) -> DocView<'_> {
        let s = self.sprite();
        let canvas = (s.width, s.height);
        let selection = match out.selection {
            Some(shape) => self.combine_selection(
                kind,
                shape.map(|m| tile_ops::cells_to_pixels(&m, &proxy.space, canvas)),
            ),
            None => self.selection.clone(),
        };
        let edit = out.command.as_ref().map(|cmd| self.tiles_edit(proxy, cmd));
        let Some(TilesEdit::Apply(cmd)) = edit else {
            return DocView {
                document: Cow::Borrowed(&self.document),
                selection,
            };
        };
        let mut doc = self.document.clone();
        let applied = DocCommand::OnArtboard {
            id: self.active,
            cmd,
        }
        .apply(&mut doc);
        DocView {
            document: if applied.is_ok() {
                Cow::Owned(doc)
            } else {
                Cow::Borrowed(&self.document)
            },
            selection,
        }
    }
```

11. En `view`, después del `let Some(tool) = … else { return current(); };` y antes de
    `let out = tool.preview(&self.context());`:

```rust
        if let Some(proxy) = self.cell_proxy(kind) {
            let out = tool.preview(&self.tiles_context(&proxy));
            return self.tiles_view(kind, out, &proxy);
        }
```

**`crates/cs-ui/src/tile_ops.rs`:** agregar a los imports `use cs_core::Tileset;` y
`use crate::editor::Editor;`, y antes de `#[cfg(test)] mod tests`:

```rust
impl Editor {
    /// La capa activa es tilemap con un tileset válido y no se está editando su máscara: las
    /// herramientas trabajan con tiles.
    pub fn tiles_mode(&self) -> bool {
        self.tile_space().is_some()
    }

    pub fn tile_space(&self) -> Option<TileSpace> {
        if self.editing_mask() {
            return None;
        }
        TileSpace::of(self.sprite(), self.layer, self.frame)
    }

    /// Índice y datos del tileset de la capa activa (si es tilemap y el tileset existe).
    pub fn active_tileset(&self) -> Option<(u32, &Tileset)> {
        let sprite = self.sprite();
        let layer = sprite
            .layer(self.layer)
            .filter(|l| l.kind == LayerKind::Tilemap)?;
        let index = layer.tileset?;
        Some((index, sprite.tilesets.get(index as usize)?))
    }

    /// Tile del pincel para el tileset activo. El elegido pertenece a un tileset: en otro (o si
    /// quedó fuera de rango) el pincel es el tile 1, o el vacío si el tileset no tiene tiles. Sin
    /// flip D si los tiles no son cuadrados.
    pub fn brush_tile(&self) -> Tile {
        let Some((index, ts)) = self.active_tileset() else {
            return self.brush_tile;
        };
        let fallback = Tile::new(u32::from(ts.tiles.len() > 1), false, false, false);
        if self.brush_tileset != Some((self.active, index))
            || self.brush_tile.index() as usize >= ts.tiles.len()
        {
            return fallback;
        }
        if ts.tile_size.0 != ts.tile_size.1 {
            return Tile(self.brush_tile.0 & !Tile::FLIP_D);
        }
        self.brush_tile
    }

    /// Elige el tile del pincel para el tileset activo.
    pub fn set_brush_tile(&mut self, tile: Tile) {
        self.brush_tileset = self.active_tileset().map(|(i, _)| (self.active, i));
        self.brush_tile = tile;
    }

    /// Aviso de la barra de estado en modo Tiles.
    pub fn tiles_status(&self) -> Option<String> {
        if !self.tiles_mode() {
            return None;
        }
        if self.active_tool().blocked_in_tiles() {
            return Some("Not available in Tiles mode".into());
        }
        if self.tile_limit {
            return Some("Tiles mode: the tilemap would exceed its size limit".into());
        }
        let (_, ts) = self.active_tileset()?;
        let n = tile_number(self.brush_tile().index(), ts.base_index);
        Some(format!("Tiles mode: painting with tile {n}"))
    }
}
```

**`crates/cs-ui/src/panels.rs`:** en `status_bar`, reemplazar el bloque `let on_tilemap = …` hasta
el cierre de su `if` (líneas 540-551, el aviso "Tilemap layer: tile editing comes in a later
version") por:

```rust
        if let Some(text) = editor.tiles_status() {
            ui.separator();
            ui.colored_label(theme::ACCENT, text);
        }
```

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo fmt --all && cargo test -p cs-ui && cargo clippy -p cs-ui --all-targets -- -D warnings`
Expected: PASS, sin warnings (en particular, nada de `tile_ops` queda sin usar).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui
git commit -m "feat(ui): Tiles mode: tools paint, pick and select tiles on tilemap layers" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Convertir capas (`cs-core` dibuja grillas; `ops::convert_to_tilemap` / `convert_to_image`)

**Files:**
- Modify: `crates/cs-core/src/tilemap.rs` (constante, dibujo de grillas, tests)
- Modify: `crates/cs-render/src/materialize.rs:8` (import atómico), `:10-13` (quitar `Tile` del
  `use cs_core`), `:294` y `:325` (llamadas), `:334-402` (borrar `grid_pixel`, `WARNED_DIAGONAL`,
  `tile_image`, `tile_source`)
- Modify: `crates/cs-io/src/aseprite.rs:9` (import) y `:42-44` (borrar la constante local)
- Modify: `crates/cs-commands/src/ops.rs:6-9` (imports), funciones nuevas antes de `mod tests`, tests
- Modify: `crates/cs-commands/Cargo.toml` (`[dev-dependencies]`)

**Interfaces:**
- Consumes: `Command::{SetTileset, SetTilemap, SetCel, SetImage, RemoveLayer, AddLayer}`,
  `ops::{tilemap_fits, MAX_TILEMAP_SIDE, is_linked}` (núcleo).
- Produces:
  - `cs_core::MAX_TILESET_TILES: u32` (= `1 << 16`, movida desde `cs-io`)
  - `Tileset::drawn_tile(&self, tile: Tile) -> Option<&Image>`
  - `cs_core::tile_grid_pixel(mode: ColorMode, tileset: &Tileset, grid: &TileGrid, lx: i64, ly: i64) -> Option<PixelValue>`
  - `cs_core::draw_tile_grid(mode: ColorMode, tileset: &Tileset, grid: &TileGrid) -> Option<Image>`
    (grilla entera, sin recortar; `None` si es 0×0 o pasa `MAX_TILEMAP_PIXELS`)
  - `pub const ops::MAX_TILE_SIDE: u32` (= 256)
  - `pub fn ops::convert_to_tilemap(sprite: &Sprite, layer: LayerId, tile_size: (u32, u32)) -> Option<Command>`
  - `pub fn ops::convert_to_image(sprite: &Sprite, layer: LayerId) -> Option<Command>`
  - Ambas devuelven un `Command::Batch`; el tipo de capa cambia con `RemoveLayer` + `AddLayer` en el
    mismo índice (mismo `id` y propiedades), así el inverso restaura capa, cels e imágenes exactos.

- [ ] **Step 1: Escribir los tests que fallan**

En los tests de `crates/cs-core/src/tilemap.rs`:

```rust
    #[test]
    fn tile_grids_draw_with_flips_and_skip_cells_that_draw_nothing() {
        let mode = ColorMode::Rgba;
        let red = PixelValue::Rgba(crate::Rgba::rgb(255, 0, 0));
        let mut ts = Tileset::new("t", (2, 2), mode);
        let mut img = Image::new(2, 2, mode);
        img.set(0, 0, red).unwrap();
        ts.tiles.push(Arc::new(img));
        let mut grid = TileGrid::new(3, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        grid.set(1, 0, Tile::new(1, true, true, false));
        grid.set(2, 0, Tile::new(9, false, false, false));
        let out = draw_tile_grid(mode, &ts, &grid).unwrap();
        assert_eq!((out.width(), out.height()), (6, 2));
        assert_eq!(out.get(0, 0), Some(red));
        assert_eq!(out.get(3, 1), Some(red), "X+Y flips move it to the opposite corner");
        assert_eq!(out.get(4, 0), Some(mode.transparent()), "tile 9 does not exist");
        assert_eq!(tile_grid_pixel(mode, &ts, &grid, 4, 0), None);
        assert!(draw_tile_grid(mode, &ts, &TileGrid::new(0, 0)).is_none());
        // Flip D en tiles no cuadrados: no dibuja.
        let mut wide = Tileset::new("w", (4, 2), mode);
        wide.tiles.push(Arc::new(Image::new(4, 2, mode)));
        let mut g = TileGrid::new(1, 1);
        g.set(0, 0, Tile::new(1, false, false, true));
        assert!(wide.drawn_tile(Tile::new(1, false, false, false)).is_some());
        assert!(wide.drawn_tile(Tile::new(1, false, false, true)).is_none());
        assert_eq!(tile_grid_pixel(mode, &wide, &g, 0, 0), None);
        assert_eq!(MAX_TILESET_TILES, 1 << 16);
    }
```

En `crates/cs-commands/Cargo.toml`, en `[dev-dependencies]`, agregar `cs-render.workspace = true`
(`cs-render` solo depende de `cs-commands` como dev-dependency, así que no hay ciclo de build).

En los tests de `crates/cs-commands/src/ops.rs` (después de
`duplicating_and_unlinking_tilemap_cels_copies_the_grid`):

```rust
    fn red() -> PixelValue {
        PixelValue::Rgba(Rgba::rgb(255, 0, 0))
    }

    /// Lo que se ve del frame (con los tilemaps dibujados).
    fn shown(s: &Sprite, frame: usize) -> Vec<Rgba> {
        cs_render::render_frame(
            &cs_render::materialize_tilemaps(s, frame..frame + 1),
            frame,
        )
    }

    /// 16×8 con la capa 1: un píxel rojo en la esquina de las celdas de 4×4 (0, 0) y (2, 0).
    fn two_equal_cells() -> Sprite {
        let mut s = Sprite::new(16, 8, ColorMode::Rgba);
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, red()), (8, 0, red())],
        }
        .apply(&mut s)
        .unwrap();
        s
    }

    #[test]
    fn convert_to_tilemap_dedupes_cells_keeps_the_layer_and_undoes() {
        let mut s = two_equal_cells();
        crate::Command::SetLayerProps {
            id: LayerId(1),
            props: crate::LayerProps {
                name: Some("ground".into()),
                opacity: Some(128),
                blend: Some(BlendMode::Multiply),
                locked: Some(true),
                ..Default::default()
            },
        }
        .apply(&mut s)
        .unwrap();
        let before = s.clone();
        let shown_before = shown(&s, 0);
        let cmd = convert_to_tilemap(&s, LayerId(1), (4, 4)).unwrap();
        let inv = cmd.apply(&mut s).unwrap();
        let (layer, old) = (
            s.layer(LayerId(1)).unwrap(),
            before.layer(LayerId(1)).unwrap(),
        );
        assert_eq!((layer.kind, layer.tileset), (LayerKind::Tilemap, Some(0)));
        assert_eq!(
            (&layer.name, layer.opacity, layer.blend, layer.locked, layer.visible),
            (&old.name, old.opacity, old.blend, old.locked, old.visible)
        );
        assert_eq!((layer.parent, layer.clip), (old.parent, old.clip));
        assert_eq!(s.layer_index(LayerId(1)), before.layer_index(LayerId(1)));
        let ts = &s.tilesets[0];
        assert_eq!(
            (ts.name.as_str(), ts.tile_size, ts.tiles.len()),
            ("ground", (4, 4), 2),
            "one tile plus the empty tile"
        );
        let (cel, grid) = grid_of(&s, 1, 0);
        assert_eq!((cel.position, cel.opacity, grid.width, grid.height), ((0, 0), 255, 4, 2));
        assert_eq!(grid.get(0, 0), Some(Tile::new(1, false, false, false)));
        assert_eq!(grid.get(2, 0), grid.get(0, 0));
        assert_eq!(grid.get(1, 0), Some(Tile::EMPTY));
        assert!(s.images.is_empty(), "the old image left the pool");
        assert_eq!(shown(&s, 0), shown_before);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn convert_to_tilemap_keeps_links_and_handles_unaligned_cels() {
        let mut s = two_equal_cells();
        Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
        let cel = *s.cel(LayerId(1), 0).unwrap();
        for frame in 0..2 {
            Command::SetCel {
                layer: LayerId(1),
                frame,
                cel: Some(Cel {
                    position: (3, 2),
                    ..cel
                }),
            }
            .apply(&mut s)
            .unwrap();
        }
        let before = (shown(&s, 0), shown(&s, 1));
        let cmd = convert_to_tilemap(&s, LayerId(1), (4, 4)).unwrap();
        cmd.apply(&mut s).unwrap();
        let (c0, c1) = (*s.cel(LayerId(1), 0).unwrap(), *s.cel(LayerId(1), 1).unwrap());
        assert_eq!(c0.image, c1.image, "linked cels stay linked");
        assert!(is_linked(&s, LayerId(1), 1));
        assert_eq!(c0.position, (0, 0), "the grid starts at the corner of its first cell");
        assert_eq!(s.tilesets[0].tiles.len(), 2);
        assert_eq!((shown(&s, 0), shown(&s, 1)), before);
    }

    #[test]
    fn convert_to_tilemap_rejects_bad_sizes_other_layers_and_too_many_tiles() {
        let s = two_equal_cells();
        assert!(convert_to_tilemap(&s, LayerId(1), (0, 4)).is_none());
        assert!(convert_to_tilemap(&s, LayerId(1), (MAX_TILE_SIDE + 1, 4)).is_none());
        assert!(convert_to_tilemap(&s, LayerId(9), (4, 4)).is_none());
        let t = with_tilemap_layer();
        assert!(
            convert_to_tilemap(&t, LayerId(2), (4, 4)).is_none(),
            "already a tilemap"
        );
        // 257×256 píxeles distintos con tiles de 1×1: más de MAX_TILESET_TILES tiles.
        let mut big = Sprite::new(257, 256, ColorMode::Rgba);
        let pixels = (0..256i32)
            .flat_map(|y| {
                (0..257i32).map(move |x| {
                    let c = Rgba::rgb(x as u8, y as u8, (x >> 8) as u8);
                    (x, y, PixelValue::Rgba(c))
                })
            })
            .collect();
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels,
        }
        .apply(&mut big)
        .unwrap();
        assert!(convert_to_tilemap(&big, LayerId(1), (1, 1)).is_none());
        assert!(convert_to_tilemap(&big, LayerId(1), (2, 2)).is_some());
    }

    #[test]
    fn convert_to_image_renders_the_same_keeps_off_canvas_tiles_and_undoes() {
        let mut s = with_tilemap_layer();
        let mut img = Image::new(4, 4, ColorMode::Rgba);
        img.set(1, 1, red()).unwrap();
        let cmd = set_tile_image(&s, 0, 1, img).unwrap();
        cmd.apply(&mut s).unwrap();
        let t1 = Tile::new(1, true, false, false);
        let cmd = set_tiles(&s, LayerId(2), 0, &[(-1, 0, t1), (3, 3, t1)]).unwrap();
        cmd.apply(&mut s).unwrap();
        let before = s.clone();
        let shown_before = shown(&s, 0);
        let cmd = convert_to_image(&s, LayerId(2)).unwrap();
        let inv = cmd.apply(&mut s).unwrap();
        let layer = s.layer(LayerId(2)).unwrap();
        assert_eq!((layer.kind, layer.tileset), (LayerKind::Image, None));
        assert_eq!(s.tilesets.len(), 1, "the tileset stays");
        assert!(s.tilemaps.is_empty(), "the grid left the pool");
        let cel = *s.cel(LayerId(2), 0).unwrap();
        let image = s.image(cel.image).unwrap();
        assert_eq!(cel.position, (-4, 0));
        assert_eq!(
            (image.width(), image.height()),
            (20, 16),
            "not cropped to the canvas"
        );
        assert_eq!(image.get(2, 1), Some(red()), "the off-canvas tile is kept, flipped");
        assert_eq!(shown(&s, 0), shown_before);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        assert!(convert_to_image(&s, LayerId(1)).is_none(), "image layers stay");
    }
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-core tilemap && cargo test -p cs-commands convert`
Expected: error de compilación (`draw_tile_grid`, `tile_grid_pixel`, `MAX_TILESET_TILES`,
`convert_to_tilemap`, `convert_to_image`, `MAX_TILE_SIDE` no existen).

- [ ] **Step 3: Implementar**

**`crates/cs-core/src/tilemap.rs`:** imports:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::{ColorMode, Image, PixelValue, RawChunk};
```

Después de `MAX_TILE_PIXELS`:

```rust
/// Tiles máximos de un tileset (incluido el vacío). Cada tile es una imagen aparte: con tiles de
/// 1×1 la cuenta dispararía millones de asignaciones. Lo usan el lector de `.aseprite` y la
/// conversión de capas a tilemap.
pub const MAX_TILESET_TILES: u32 = 1 << 16;
```

En `impl Tileset`, después de `new`:

```rust
    /// Imagen que dibuja `tile`, o `None` si la celda no dibuja nada: tile vacío, índice fuera de
    /// rango, imagen de otro tamaño o flip D en un tile no cuadrado.
    pub fn drawn_tile(&self, tile: Tile) -> Option<&Image> {
        let index = tile.index() as usize;
        if index == 0 {
            return None;
        }
        let image = self.tiles.get(index)?;
        let (tw, th) = self.tile_size;
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
```

Después de `impl Tileset` (antes de `#[cfg(test)]`):

```rust
/// Avisado una sola vez: flip D en tiles no cuadrados.
static WARNED_DIAGONAL: AtomicBool = AtomicBool::new(false);

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

/// Valor que dibuja la grilla en `(lx, ly)`, relativo a su esquina: una sola búsqueda de tile.
/// `None` fuera de la grilla, en celdas que no dibujan (ver `Tileset::drawn_tile`) o si el tile es
/// de otro modo de color (datos corruptos).
pub fn tile_grid_pixel(
    mode: ColorMode,
    tileset: &Tileset,
    grid: &TileGrid,
    lx: i64,
    ly: i64,
) -> Option<PixelValue> {
    let (tw, th) = tileset.tile_size;
    if lx < 0 || ly < 0 || tw == 0 || th == 0 {
        return None;
    }
    let gx = u32::try_from(lx / tw as i64).ok()?;
    let gy = u32::try_from(ly / th as i64).ok()?;
    let tile = grid.get(gx, gy)?;
    let src = tileset.drawn_tile(tile)?;
    let (sx, sy) = tile_source(
        tile,
        tw,
        th,
        (lx % tw as i64) as u32,
        (ly % th as i64) as u32,
    );
    let value = src.get(sx, sy)?;
    let same_mode = matches!(
        (value, mode),
        (PixelValue::Rgba(_), ColorMode::Rgba) | (PixelValue::Index(_), ColorMode::Indexed { .. })
    );
    same_mode.then_some(value)
}

/// La grilla entera dibujada (sin recortar), con cada tile copiado como valores y sus flips.
/// `None` si la grilla no tiene celdas o pasaría `MAX_TILEMAP_PIXELS`.
pub fn draw_tile_grid(mode: ColorMode, tileset: &Tileset, grid: &TileGrid) -> Option<Image> {
    let (tw, th) = tileset.tile_size;
    let w = grid.width as u64 * tw as u64;
    let h = grid.height as u64 * th as u64;
    if w == 0 || h == 0 || w * h > MAX_TILEMAP_PIXELS {
        return None;
    }
    let mut image = Image::new(w as u32, h as u32, mode);
    for y in 0..h as u32 {
        for x in 0..w as u32 {
            if let Some(v) = tile_grid_pixel(mode, tileset, grid, x as i64, y as i64) {
                image.set(x, y, v).expect("in bounds and same mode");
            }
        }
    }
    Some(image)
}
```

**`crates/cs-render/src/materialize.rs`:** borrar la línea 8
(`use std::sync::atomic::{AtomicBool, Ordering};`), quitar `Tile` del `use cs_core::{…}`
(líneas 10-13), reemplazar `grid_pixel(` por `cs_core::tile_grid_pixel(` en `tilemap_image`
(línea 294) y en `tilemap_pixel` (línea 325), y borrar desde el comentario de `grid_pixel`
(línea 334) hasta el final de `tile_source` (línea 402): `grid_pixel`, `WARNED_DIAGONAL`,
`tile_image` y `tile_source` ahora viven en `cs-core`. Los tests de `materialize.rs` no cambian.

**`crates/cs-io/src/aseprite.rs`:** agregar `MAX_TILESET_TILES` al `use cs_core::{…}` (línea 9) y
borrar las líneas 42-44 (comentario y `const MAX_TILESET_TILES: u32 = 1 << 16;`).

**`crates/cs-commands/src/ops.rs`:** imports (líneas 6-9):

```rust
use cs_core::{
    ArtboardId, Cel, ColorMode, Image, ImageId, InstanceKey, Layer, LayerId, LayerKind, LayerMask,
    MAX_TILESET_TILES, Mask, PixelBuffer, PixelValue, Sprite, Tile, TileGrid, Tileset,
};
```

Después de `is_linked` (antes de `#[cfg(test)]`):

```rust
/// Lado máximo de un tile al crear tilesets desde la UI o al convertir una capa.
pub const MAX_TILE_SIDE: u32 = 256;

/// Clave para comparar tiles por igualdad exacta de píxeles.
fn pixel_key(image: &Image) -> Vec<u8> {
    match image.pixels() {
        PixelBuffer::Rgba(px) => px.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect(),
        PixelBuffer::Indexed(px) => px.clone(),
    }
}

/// Un píxel que no se ve: alfa 0 en RGBA, el índice transparente en indexado.
fn is_clear(value: PixelValue, mode: ColorMode) -> bool {
    match value {
        PixelValue::Rgba(c) => c.a == 0,
        v => v == mode.transparent(),
    }
}

/// Cambia el tipo de una capa conservando id, índice y propiedades: la quita (con sus cels; las
/// imágenes y grillas que quedan sin uso salen del pool) y agrega `layer` en el mismo índice. El
/// inverso de las dos restaura todo exacto.
fn replace_layer(sprite: &Sprite, layer: Layer) -> Option<[Command; 2]> {
    let index = sprite.layer_index(layer.id)?;
    Some([
        Command::RemoveLayer { id: layer.id },
        Command::AddLayer { index, layer },
    ])
}

/// Convierte la capa de imagen `layer` en una capa tilemap con un tileset nuevo (con el nombre de
/// la capa) de tiles de `tile_size`. Cada cel se corta en celdas de la grilla del lienzo (la celda
/// `(cx, cy)` son los píxeles `[cx·tw, (cx+1)·tw)`); solo cuentan las celdas que tocan la imagen
/// del cel. Una celda transparente es el tile vacío; las demás se comparan por igualdad exacta de
/// píxeles (sin flips) y las nuevas se agregan al tileset. Cada cel pasa a una grilla cuya
/// `position` es la esquina de su primera celda, con la misma opacidad; los cels vinculados siguen
/// vinculados. Todo es un solo `Batch`. `None` si la capa no es de imagen, el tamaño de tile no es
/// válido (0, más de `MAX_TILE_SIDE` o más de `MAX_TILE_PIXELS`), harían falta más de
/// `MAX_TILESET_TILES` tiles o alguna grilla pasaría `MAX_TILEMAP_SIDE` / `tilemap_fits`.
pub fn convert_to_tilemap(
    sprite: &Sprite,
    layer: LayerId,
    tile_size: (u32, u32),
) -> Option<Command> {
    let l = sprite.layer(layer)?;
    let (tw, th) = tile_size;
    if l.kind != LayerKind::Image
        || !(1..=MAX_TILE_SIDE).contains(&tw)
        || !(1..=MAX_TILE_SIDE).contains(&th)
        || tw as u64 * th as u64 > cs_core::MAX_TILE_PIXELS
    {
        return None;
    }
    let mode = sprite.color_mode;
    let index = sprite.tilesets.len();
    let mut tileset = Tileset::new(l.name.clone(), tile_size, mode);
    let mut known: HashMap<Vec<u8>, u32> = HashMap::new();
    let mut next = sprite.next_image_id().0;
    let (mut grids, mut cels) = (Vec::new(), Vec::new());
    // Cels vinculados (misma imagen y posición) → la misma grilla.
    let mut done: HashMap<(ImageId, (i32, i32)), (ImageId, (i32, i32))> = HashMap::new();
    let (tw_i, th_i) = (tw as i32, th as i32);
    for frame in 0..sprite.frames.len() {
        let Some(cel) = sprite.cel(layer, frame).copied() else {
            continue;
        };
        let (grid_id, position) = match done.get(&(cel.image, cel.position)) {
            Some(&found) => found,
            None => {
                let Some(image) = sprite.image(cel.image) else {
                    continue;
                };
                let (px, py) = cel.position;
                let (iw, ih) = (image.width() as i32, image.height() as i32);
                if iw == 0 || ih == 0 {
                    continue;
                }
                let (cx0, cy0) = (px.div_euclid(tw_i), py.div_euclid(th_i));
                let (cx1, cy1) = ((px + iw - 1).div_euclid(tw_i), (py + ih - 1).div_euclid(th_i));
                let (gw, gh) = ((cx1 - cx0 + 1) as u32, (cy1 - cy0 + 1) as u32);
                if gw > MAX_TILEMAP_SIDE
                    || gh > MAX_TILEMAP_SIDE
                    || !tilemap_fits(gw as u64, gh as u64, tile_size)
                {
                    return None;
                }
                let mut grid = TileGrid::new(gw, gh);
                for gy in 0..gh {
                    for gx in 0..gw {
                        let (x0, y0) = ((cx0 + gx as i32) * tw_i, (cy0 + gy as i32) * th_i);
                        let mut tile = Image::new(tw, th, mode);
                        let mut clear = true;
                        for ty in 0..th {
                            for tx in 0..tw {
                                let (lx, ly) = (x0 + tx as i32 - px, y0 + ty as i32 - py);
                                if lx < 0 || ly < 0 {
                                    continue;
                                }
                                let Some(v) = image.get(lx as u32, ly as u32) else {
                                    continue;
                                };
                                clear &= is_clear(v, mode);
                                tile.set(tx, ty, v).ok()?;
                            }
                        }
                        if clear {
                            continue;
                        }
                        let key = pixel_key(&tile);
                        let i = match known.get(&key) {
                            Some(&i) => i,
                            None => {
                                if tileset.tiles.len() as u32 >= MAX_TILESET_TILES {
                                    return None;
                                }
                                let i = tileset.tiles.len() as u32;
                                tileset.tiles.push(Arc::new(tile));
                                known.insert(key, i);
                                i
                            }
                        };
                        grid.set(gx, gy, Tile::new(i, false, false, false));
                    }
                }
                let id = ImageId(next);
                next += 1;
                let position = (cx0 * tw_i, cy0 * th_i);
                grids.push(Command::SetTilemap {
                    id,
                    grid: Some(Arc::new(grid)),
                });
                done.insert((cel.image, cel.position), (id, position));
                (id, position)
            }
        };
        cels.push(Command::SetCel {
            layer,
            frame,
            cel: Some(Cel {
                position,
                opacity: cel.opacity,
                image: grid_id,
            }),
        });
    }
    let mut new_layer = l.clone();
    new_layer.kind = LayerKind::Tilemap;
    new_layer.tileset = Some(index as u32);
    let mut cmds = vec![Command::SetTileset {
        index,
        tileset: Some(tileset),
    }];
    cmds.extend(replace_layer(sprite, new_layer)?);
    cmds.extend(grids);
    cmds.extend(cels);
    Some(Command::Batch(cmds))
}

/// Convierte la capa tilemap `layer` en una capa de imagen: cada cel pasa a su grilla dibujada
/// entera (sin recortar al lienzo, para no perder contenido), en la misma posición y con la misma
/// opacidad; los cels vinculados siguen vinculados. El tileset queda en el sprite. Un solo
/// `Batch`. `None` si la capa no es tilemap o su tileset no existe.
pub fn convert_to_image(sprite: &Sprite, layer: LayerId) -> Option<Command> {
    let l = sprite.layer(layer)?;
    if l.kind != LayerKind::Tilemap {
        return None;
    }
    let tileset = sprite.tilesets.get(l.tileset? as usize)?;
    let mut next = sprite.next_image_id().0;
    let (mut images, mut cels) = (Vec::new(), Vec::new());
    let mut done: HashMap<ImageId, ImageId> = HashMap::new();
    for frame in 0..sprite.frames.len() {
        let Some(cel) = sprite.cel(layer, frame).copied() else {
            continue;
        };
        let id = match done.get(&cel.image) {
            Some(&id) => id,
            None => {
                let Some(grid) = sprite.tilemap(cel.image) else {
                    continue;
                };
                let Some(image) = cs_core::draw_tile_grid(sprite.color_mode, tileset, grid) else {
                    continue;
                };
                let id = ImageId(next);
                next += 1;
                images.push(Command::SetImage {
                    id,
                    image: Some(Arc::new(image)),
                });
                done.insert(cel.image, id);
                id
            }
        };
        cels.push(Command::SetCel {
            layer,
            frame,
            cel: Some(Cel { image: id, ..cel }),
        });
    }
    let mut new_layer = l.clone();
    new_layer.kind = LayerKind::Image;
    new_layer.tileset = None;
    let mut cmds = Vec::from(replace_layer(sprite, new_layer)?);
    cmds.extend(images);
    cmds.extend(cels);
    Some(Command::Batch(cmds))
}
```

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo fmt --all && cargo test -p cs-core && cargo test -p cs-render && cargo test -p cs-commands && cargo test -p cs-io`
Expected: PASS (los tests de materialize, incluidos los de flips y `diagonal_flip_needs_square_tiles`,
siguen pasando con el código movido).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-core crates/cs-render crates/cs-io crates/cs-commands
git commit -m "feat(commands): convert layers between image and tilemap" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: MCP `convert_layer`

**Files:**
- Modify: `crates/cs-agent/src/params.rs` (después de `GetTiles`, ~línea 865)
- Modify: `crates/cs-agent/src/ops/tilemap.rs` (imports 1-10, método nuevo, tests)
- Modify: `apps/chimpsprite-mcp/src/server.rs` (después de `get_tiles`, ~línea 236)
- Modify: `apps/chimpsprite-mcp/tests/stdio.rs:90-97` (lista de herramientas esperadas)

**Interfaces:**
- Consumes: `ops::convert_to_tilemap`, `ops::convert_to_image`, `cs_core::MAX_TILESET_TILES` (Task 4).
- Produces:
  - `pub enum ConvertTo { Tilemap, Image }` (`serde(rename_all = "lowercase")`)
  - `pub struct ConvertLayer { pub doc: String, pub artboard: Option<Ref>, pub layer: Ref, pub to: ConvertTo, pub tile_width: Option<u32>, pub tile_height: Option<u32> }`
  - `Session::convert_layer(&mut self, p: ConvertLayer) -> Result<Reply>`: a tilemap responde JSON
    `{"layer", "kind": "tilemap", "tileset", "tile_size", "tiles", "notes"}` (`tiles` cuenta el
    vacío); a imagen responde texto. Errores: tamaño de tile faltante, fuera de 1-256, tipo de capa
    equivocado, conversión imposible.
  - Herramienta MCP `convert_layer`.

- [ ] **Step 1: Escribir el test que falla**

En los tests de `crates/cs-agent/src/ops/tilemap.rs`:

```rust
    #[test]
    fn convert_layer_both_ways_with_clear_errors() {
        let mut s = session(16, 8);
        s.draw_pixels(p(json!({"doc": "d1", "pixels": [
            {"x": 0, "y": 0, "color": "#ff0000"}, {"x": 8, "y": 0, "color": "#ff0000"}]})))
            .unwrap();
        let e = s
            .convert_layer(p(json!({"doc": "d1", "layer": 1, "to": "tilemap"})))
            .unwrap_err();
        assert!(e.to_string().contains("tile_width"), "{e}");
        let e = s
            .convert_layer(p(json!({"doc": "d1", "layer": 1, "to": "tilemap",
                "tile_width": 0, "tile_height": 4})))
            .unwrap_err();
        assert!(e.to_string().contains("1-256"), "{e}");
        let r = json(
            &s.convert_layer(p(json!({"doc": "d1", "layer": 1, "to": "tilemap",
                "tile_width": 4, "tile_height": 4})))
                .unwrap(),
        );
        assert_eq!(r["tiles"], 2, "one tile plus the empty tile");
        assert_eq!(r["tileset"], 0);
        assert_eq!(px(&s, 0, 0), RED);
        assert_eq!(px(&s, 8, 0), RED);
        let e = s
            .convert_layer(p(json!({"doc": "d1", "layer": 1, "to": "tilemap",
                "tile_width": 4, "tile_height": 4})))
            .unwrap_err();
        assert!(e.to_string().contains("not an image layer"), "{e}");
        s.convert_layer(p(json!({"doc": "d1", "layer": 1, "to": "image"})))
            .unwrap();
        assert_eq!(px(&s, 8, 0), RED);
        let e = s
            .convert_layer(p(json!({"doc": "d1", "layer": 1, "to": "image"})))
            .unwrap_err();
        assert!(e.to_string().contains("not a tilemap layer"), "{e}");
    }
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-agent convert_layer`
Expected: error de compilación (`convert_layer` no existe).

- [ ] **Step 3: Implementar**

`crates/cs-agent/src/params.rs`, después de `GetTiles`:

```rust
/// Target kind of `convert_layer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConvertTo {
    Tilemap,
    Image,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConvertLayer {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Ref,
    /// "tilemap" (from an image layer) or "image" (from a tilemap layer).
    pub to: ConvertTo,
    /// Tile width in pixels (1-256); required with to = "tilemap".
    pub tile_width: Option<u32>,
    /// Tile height in pixels (1-256); required with to = "tilemap".
    pub tile_height: Option<u32>,
}
```

`crates/cs-agent/src/ops/tilemap.rs`, imports:

```rust
use crate::params::{
    AddTilemapLayer, ConvertLayer, ConvertTo, CreateTileset, GetTiles, Ref, SetTilePixels,
    SetTiles,
};
```

y en `impl Session`, después de `get_tiles`:

```rust
    pub fn convert_layer(&mut self, p: ConvertLayer) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let id = target::layer(sprite, &p.layer)?;
        let layer = sprite.layer(id).expect("resolved");
        let name = layer.name.clone();
        match p.to {
            ConvertTo::Tilemap => {
                if layer.kind != LayerKind::Image {
                    return Err(invalid(format!(
                        "layer '{name}' is not an image layer; only image layers convert to \
                         tilemaps"
                    )));
                }
                let (Some(w), Some(h)) = (p.tile_width, p.tile_height) else {
                    return Err(invalid(
                        "tile_width and tile_height are required with to = \"tilemap\"",
                    ));
                };
                if !(1..=MAX_TILE).contains(&w) || !(1..=MAX_TILE).contains(&h) {
                    return Err(invalid(format!(
                        "tile size {w}×{h} must be 1-{MAX_TILE} on each side"
                    )));
                }
                let index = sprite.tilesets.len();
                let Some(cmd) = ops::convert_to_tilemap(sprite, id, (w, h)) else {
                    return Err(invalid(format!(
                        "cannot convert '{name}' with {w}×{h} tiles: it would need more than {} \
                         distinct tiles, or a cel's grid would exceed the tilemap limits; try \
                         larger tiles",
                        cs_core::MAX_TILESET_TILES
                    )));
                };
                let tiles = match &cmd {
                    Command::Batch(cmds) => cmds.iter().find_map(|c| match c {
                        Command::SetTileset {
                            tileset: Some(ts), ..
                        } => Some(ts.tiles.len()),
                        _ => None,
                    }),
                    _ => None,
                }
                .unwrap_or(1);
                self.run_on(&p.doc, ab, vec![cmd])?;
                Ok(Reply::json(json!({
                    "layer": id.0,
                    "kind": "tilemap",
                    "tileset": index,
                    "tile_size": [w, h],
                    "tiles": tiles,
                    "notes": "tiles counts the empty tile 0; the layer keeps its id, name and properties.",
                })))
            }
            ConvertTo::Image => {
                if layer.kind != LayerKind::Tilemap {
                    return Err(invalid(format!(
                        "layer '{name}' is not a tilemap layer; only tilemap layers convert to \
                         image layers"
                    )));
                }
                let Some(cmd) = ops::convert_to_image(sprite, id) else {
                    return Err(invalid(format!(
                        "tilemap layer '{name}' has no valid tileset"
                    )));
                };
                self.run_on(&p.doc, ab, vec![cmd])?;
                Ok(Reply::text(format!(
                    "Converted '{name}' to an image layer; its tileset stays in the artboard."
                )))
            }
        }
    }
```

`apps/chimpsprite-mcp/src/server.rs`, después de `get_tiles`:

```rust
    #[tool(
        description = "Convert a layer between image and tilemap (one undo step; the layer keeps its id, name and properties). to = \"tilemap\" needs tile_width and tile_height (1-256): it cuts the image layer into cells of the canvas grid (cell (x, y) starts at pixel (x * tile_width, y * tile_height)), builds a new tileset named like the layer with the distinct cells (exact pixel match, no flips; transparent cells are the empty tile 0) and returns the tile count. to = \"image\" draws a tilemap layer back into pixels without cropping; the tileset stays."
    )]
    fn convert_layer(&self, Parameters(p): Parameters<ConvertLayer>) -> CallToolResult {
        self.with(|s| s.convert_layer(p))
    }
```

`apps/chimpsprite-mcp/tests/stdio.rs`, en la lista `for expected in [ … ]`, agregar
`"convert_layer",` después de `"inbetween",`.

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo fmt --all && cargo test -p cs-agent && cargo test -p chimpsprite-mcp`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-agent apps/chimpsprite-mcp
git commit -m "feat(mcp): convert_layer between image and tilemap" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Panel Tileset, flips del pincel y atajos X/Y

**Files:**
- Modify: `crates/cs-ui/src/tile_ops.rs` (`toggle_brush_flip`, test)
- Modify: `crates/cs-ui/src/widgets.rs` (`outlined_text`)
- Modify: `crates/cs-ui/src/panels.rs` (imports 1-10; `swatch` 376-396; `color_picker` 452;
  `TileThumbs` y `tileset` después de `color_picker`; tests ~913)
- Modify: `crates/cs-ui/src/tabs.rs:14-36` (`DocState::tile_thumbs`)
- Modify: `crates/cs-ui/src/lib.rs:399` (atajos) y `:585` (panel derecho)

**Interfaces:**
- Consumes: `Editor::{tiles_mode, active_tileset, brush_tile, set_brush_tile}`, `tile_number`
  (Tasks 1 y 3).
- Produces:
  - `Editor::toggle_brush_flip(&mut self, flip: u32)` (`Tile::FLIP_X`, `FLIP_Y` o `FLIP_D`; D se
    descarta si los tiles no son cuadrados)
  - `pub fn widgets::outlined_text(painter: &egui::Painter, pos: egui::Pos2, align: egui::Align2, text: &str, font: egui::FontId)`
  - `#[derive(Default)] pub struct panels::TileThumbs` con `fn refresh(&mut self, ctx: &egui::Context, editor: &Editor)`
    (cache por `(artboard, tileset, editor.doc_version)`)
  - `pub fn panels::tileset(ui: &mut egui::Ui, editor: &mut Editor, thumbs: &mut TileThumbs)`
  - `pub fn panels::color_picker(ui: &mut egui::Ui, editor: &mut Editor)` (antes privada)
  - `DocState::tile_thumbs: panels::TileThumbs`

- [ ] **Step 1: Escribir los tests que fallan**

En los tests de `crates/cs-ui/src/tile_ops.rs` (módulo `tests`):

```rust
    #[test]
    fn brush_tile_flips_and_resets_with_the_tileset() {
        use crate::tile_ops::test_util::{t, tile_editor};
        let mut e = tile_editor();
        assert_eq!(e.brush_tile(), t(1));
        e.toggle_brush_flip(Tile::FLIP_X);
        e.toggle_brush_flip(Tile::FLIP_D);
        assert_eq!(e.brush_tile(), Tile::new(1, true, false, true), "square tiles allow D");
        e.toggle_brush_flip(Tile::FLIP_X);
        assert_eq!(e.brush_tile(), Tile::new(1, false, false, true));
        e.set_brush_tile(t(7));
        assert_eq!(e.brush_tile(), t(1), "out of range goes back to tile 1");
        e.set_brush_tile(t(2));
        // Otra capa con otro tileset, de tiles no cuadrados.
        let mut ts = Tileset::new("wide", (4, 2), ColorMode::Rgba);
        ts.tiles.push(Arc::new(Image::new(4, 2, ColorMode::Rgba)));
        let add = cs_commands::ops::new_tilemap_layer(e.sprite(), Some(e.layer), 1);
        let id = match &add {
            Command::AddLayer { layer, .. } => layer.id,
            _ => unreachable!("new_tilemap_layer returns AddLayer"),
        };
        e.execute(Command::Batch(vec![
            Command::SetTileset {
                index: 1,
                tileset: Some(ts),
            },
            add,
        ]));
        e.select_layer(id);
        assert_eq!(e.brush_tile(), t(1), "another tileset starts on tile 1");
        e.toggle_brush_flip(Tile::FLIP_D);
        assert_eq!(e.brush_tile(), t(1), "no D on non-square tiles");
        e.select_layer(LayerId(2));
        assert_eq!(e.brush_tile(), t(1), "the brush now belongs to the other tileset");
    }
```

En el módulo `tests` de `crates/cs-ui/src/panels.rs`:

```rust
    #[test]
    fn tile_thumbnails_are_cached_until_the_document_changes() {
        let ctx = egui::Context::default();
        let mut e = crate::tile_ops::test_util::tile_editor();
        let mut thumbs = TileThumbs::default();
        thumbs.refresh(&ctx, &e);
        assert_eq!(thumbs.textures.len(), 3, "the empty tile and tiles 1 and 2");
        let first = thumbs.textures[1].id();
        thumbs.refresh(&ctx, &e);
        assert_eq!(thumbs.textures[1].id(), first, "cached");
        let cmd = cs_commands::ops::set_tiles(
            e.sprite(),
            e.layer,
            0,
            &[(0, 0, cs_core::Tile::new(1, false, false, false))],
        )
        .unwrap();
        e.execute(cmd);
        thumbs.refresh(&ctx, &e);
        assert_ne!(thumbs.textures[1].id(), first, "rebuilt after a document change");
        e.select_layer(cs_core::LayerId(1));
        thumbs.refresh(&ctx, &e);
        assert!(thumbs.textures.is_empty(), "no tileset on an image layer");
    }
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-ui brush_tile_flips && cargo test -p cs-ui tile_thumbnails`
Expected: error de compilación (`toggle_brush_flip` y `TileThumbs` no existen).

- [ ] **Step 3: Implementar**

**`crates/cs-ui/src/tile_ops.rs`**, en `impl Editor`, después de `set_brush_tile`:

```rust
    /// Invierte un flip del pincel (`Tile::FLIP_X`, `FLIP_Y` o `FLIP_D`). D solo existe con tiles
    /// cuadrados.
    pub fn toggle_brush_flip(&mut self, flip: u32) {
        let Some((_, ts)) = self.active_tileset() else {
            return;
        };
        let square = ts.tile_size.0 == ts.tile_size.1;
        let mut tile = Tile(self.brush_tile().0 ^ flip);
        if !square {
            tile = Tile(tile.0 & !Tile::FLIP_D);
        }
        self.set_brush_tile(tile);
    }
```

**`crates/cs-ui/src/widgets.rs`**, al final:

```rust
/// Texto claro con contorno oscuro: se lee sobre cualquier fondo.
pub fn outlined_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    align: egui::Align2,
    text: &str,
    font: egui::FontId,
) {
    for d in [
        egui::vec2(-1.0, 0.0),
        egui::vec2(1.0, 0.0),
        egui::vec2(0.0, -1.0),
        egui::vec2(0.0, 1.0),
    ] {
        painter.text(pos + d, align, text, font.clone(), egui::Color32::BLACK);
    }
    painter.text(pos, align, text, font, egui::Color32::WHITE);
}
```

**`crates/cs-ui/src/panels.rs`:**

1. Imports: la primera línea pasa a
   `use eframe::egui::{self, Color32, ColorImage, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2};`,
   `use cs_core::{ArtboardId, SymmetryMode};` pasa a `use cs_core::{ArtboardId, SymmetryMode, Tile};`
   y se agrega `use crate::tile_ops;`.

2. Reemplazar `swatch` por:

```rust
/// Damero mínimo para que se note la transparencia.
fn checker(painter: &egui::Painter, rect: Rect) {
    let half = rect.size() / 2.0;
    painter.rect_filled(rect, 0.0, theme::CHECKER_LIGHT);
    painter.rect_filled(
        Rect::from_min_size(rect.min, half),
        0.0,
        theme::CHECKER_DARK,
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min + half, half),
        0.0,
        theme::CHECKER_DARK,
    );
}

fn swatch(ui: &mut egui::Ui, color: Color32, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    if color.a() < 255 {
        checker(painter, rect);
    }
    painter.rect_filled(rect, 0.0, color);
    response
}
```

3. `fn color_picker` pasa a `pub fn color_picker`.

4. Después de `color_picker`:

```rust
/// Lado de las miniaturas del panel Tileset.
const THUMB: f32 = 32.0;

/// Miniaturas de los tiles del tileset activo, cacheadas por artboard, tileset y versión del
/// documento.
#[derive(Default)]
pub struct TileThumbs {
    key: Option<(ArtboardId, u32, u64)>,
    /// Una por tile (la 0 es el vacío y no se dibuja).
    textures: Vec<TextureHandle>,
}

impl TileThumbs {
    fn refresh(&mut self, ctx: &egui::Context, editor: &Editor) {
        let Some((index, ts)) = editor.active_tileset() else {
            self.key = None;
            self.textures.clear();
            return;
        };
        let key = (editor.active, index, editor.doc_version);
        if self.key == Some(key) {
            return;
        }
        let sprite = editor.sprite();
        self.textures = ts
            .tiles
            .iter()
            .enumerate()
            .map(|(i, tile)| {
                let bytes: Vec<u8> = (0..tile.height())
                    .flat_map(|y| (0..tile.width()).map(move |x| (x, y)))
                    .flat_map(|(x, y)| {
                        let c = tile
                            .get(x, y)
                            .map_or(cs_core::Rgba::TRANSPARENT, |v| cs_render::resolve(sprite, v));
                        [c.r, c.g, c.b, c.a]
                    })
                    .collect();
                let size = [tile.width() as usize, tile.height() as usize];
                let image = ColorImage::from_rgba_unmultiplied(size, &bytes);
                let name = format!("tile-{}-{index}-{i}", editor.active.0);
                ctx.load_texture(name, image, TextureOptions::NEAREST)
            })
            .collect();
        self.key = Some(key);
    }
}

/// Panel "Tileset" (reemplaza a la paleta con una capa tilemap activa): elegir el tile del pincel
/// y sus flips.
pub fn tileset(ui: &mut egui::Ui, editor: &mut Editor, thumbs: &mut TileThumbs) {
    thumbs.refresh(ui.ctx(), editor);
    let Some((_, ts)) = editor.active_tileset() else {
        return;
    };
    let (name, (tw, th), count, base) = (
        ts.name.clone(),
        ts.tile_size,
        ts.tiles.len(),
        ts.base_index,
    );
    ui.add_space(6.0);
    ui.heading("Tileset");
    ui.separator();
    ui.label(format!(
        "{name}  {tw}×{th}  {} tiles",
        count.saturating_sub(1)
    ));
    let brush = editor.brush_tile();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        for i in 0..count {
            let (rect, response) =
                ui.allocate_exact_size(Vec2::splat(THUMB + 4.0), Sense::click());
            let inner = rect.shrink(2.0);
            let painter = ui.painter();
            checker(painter, inner);
            if i == 0 {
                // El vacío: el dibujo de "transparente".
                painter.line_segment(
                    [inner.left_bottom(), inner.right_top()],
                    Stroke::new(1.5, Color32::from_rgb(220, 60, 60)),
                );
            } else if let Some(tex) = thumbs.textures.get(i) {
                let scale = THUMB / tw.max(th) as f32;
                let size = Vec2::new(tw as f32 * scale, th as f32 * scale);
                let uv = Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0));
                painter.image(
                    tex.id(),
                    Rect::from_center_size(inner.center(), size),
                    uv,
                    Color32::WHITE,
                );
            }
            let number = tile_ops::tile_number(i as u32, base);
            widgets::outlined_text(
                painter,
                inner.left_top() + Vec2::new(1.0, 0.0),
                egui::Align2::LEFT_TOP,
                &number.to_string(),
                egui::FontId::monospace(9.0),
            );
            if brush.index() as usize == i {
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(2.0, Color32::WHITE),
                    StrokeKind::Inside,
                );
            }
            if response.on_hover_text(format!("Tile {number}")).clicked() {
                editor.set_brush_tile(Tile::new(
                    i as u32,
                    brush.flip_x(),
                    brush.flip_y(),
                    brush.flip_d(),
                ));
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Flip:");
        let brush = editor.brush_tile();
        if ui
            .selectable_label(brush.flip_x(), "X")
            .on_hover_text("Flip horizontally (X)")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_X);
        }
        if ui
            .selectable_label(brush.flip_y(), "Y")
            .on_hover_text("Flip vertically (Y)")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_Y);
        }
        if ui
            .add_enabled(tw == th, egui::Button::selectable(brush.flip_d(), "D"))
            .on_hover_text("Flip diagonally (swap X and Y)")
            .on_disabled_hover_text("Needs square tiles")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_D);
        }
    });
}
```

Nota: `ui.painter()` devuelve `&Painter` atado a `ui`; si el borrow checker se queja dentro del
`for` (por el `response.on_hover_text` que no toma `ui`), mover `let painter = ui.painter();`
inmediatamente antes de cada uso. No cambia el comportamiento.

**`crates/cs-ui/src/tabs.rs`:** en `struct DocState`, después de `artboard_name`:

```rust
    /// Miniaturas del panel Tileset.
    pub tile_thumbs: crate::panels::TileThumbs,
```

y en `DocState::new`, después de `artboard_name: (ArtboardId(0), String::new()),`:
`tile_thumbs: Default::default(),`.

**`crates/cs-ui/src/lib.rs`:**

1. Panel derecho (línea 585): reemplazar `panels::palette(ui, &mut self.doc.editor);` por:

```rust
                        if self.doc.editor.tiles_mode() {
                            panels::tileset(
                                ui,
                                &mut self.doc.editor,
                                &mut self.doc.tile_thumbs,
                            );
                            // El selector de color sigue a mano (lo usa 2B).
                            ui.add_space(10.0);
                            panels::color_picker(ui, &mut self.doc.editor);
                        } else {
                            panels::palette(ui, &mut self.doc.editor);
                        }
```

2. Atajos (línea 399): reemplazar `Key::X => editor.swap_colors(),` por:

```rust
                // En modo Tiles no hay color secundario: X e Y voltean el tile del pincel.
                Key::X if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_X),
                Key::Y if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_Y),
                Key::X => editor.swap_colors(),
```

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo fmt --all && cargo test -p cs-ui && cargo clippy -p cs-ui --all-targets -- -D warnings`
Expected: PASS, sin warnings. (No ejecutar la app: el panel se verifica en la Task 9.)

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui
git commit -m "feat(ui): Tileset panel with brush tile, flips and X/Y shortcuts" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Nueva capa tilemap y diálogos de conversión (menú Layer y timeline)

**Files:**
- Create: `crates/cs-ui/src/tilemap_dialog.rs`
- Modify: `crates/cs-ui/src/tile_ops.rs` (`TilesetChoice`, métodos de `Editor`, tests)
- Modify: `crates/cs-ui/src/lib.rs` (`mod`, campos 47/81, hook de captura después de la línea 119,
  `editor_panels` ~562-572, menú Layer 490-495, `update_tilemap_dialogs`, llamada después de
  `self.update_grid_dialog(&ctx);` línea 672)
- Modify: `crates/cs-ui/src/file_actions.rs:79-86` (`is_modal`)
- Modify: `crates/cs-ui/src/timeline.rs:42-51` (`TimelineState`) y `:129-131` (botón)

**Interfaces:**
- Consumes: `ops::{new_tilemap_layer, convert_to_tilemap, convert_to_image, MAX_TILE_SIDE}`
  (núcleo y Task 4), `Editor::{tiles_mode, active_tileset}` (Task 3).
- Produces:
  - `#[derive(Clone, Debug, PartialEq)] pub enum TilesetChoice { Existing(u32), New { name: String, tile_size: (u32, u32), base_index: i16 } }`
  - `Editor::add_tilemap_layer(&mut self, choice: &TilesetChoice) -> bool` (un `Batch`:
    `SetTileset` si es nuevo + `new_tilemap_layer` sobre la capa actual; la capa nueva queda
    seleccionada)
  - `Editor::can_convert_to_tilemap(&self) -> bool`, `Editor::can_convert_to_image(&self) -> bool`,
    `Editor::convert_to_tilemap(&mut self, tile_size: (u32, u32)) -> bool`,
    `Editor::convert_to_image(&mut self) -> bool`
  - `tilemap_dialog::{DialogResult, default_tile_size, NewTilemapDialog, TileSizeDialog}` con
    `NewTilemapDialog::new(&Editor)`, `choice(&self) -> TilesetChoice`,
    `show(&mut self, &egui::Context) -> DialogResult`; `TileSizeDialog::new(&Editor)`,
    `TileSizeDialog::tile_size: (u32, u32)`, `show(...)`.
  - `TimelineState::new_tilemap: bool`; env de desarrollo `CHIMPSPRITE_DEMO_NEW_TILEMAP`.

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/cs-ui/src/tilemap_dialog.rs` solo con el encabezado y los tests:

```rust
//! Diálogos de tilemaps: nueva capa tilemap y tamaño de tile para convertir una capa.

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};

    use super::*;
    use crate::tile_ops::TilesetChoice;

    #[test]
    fn dialogs_default_to_the_work_grid_size() {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        let d = NewTilemapDialog::new(&e);
        assert_eq!(
            d.choice(),
            TilesetChoice::New {
                name: "Tileset 1".into(),
                tile_size: (16, 16),
                base_index: 1
            }
        );
        e.execute(cs_commands::Command::SetGrid(cs_core::GridSettings {
            size: (8, 300),
            offset: (0, 0),
        }));
        assert_eq!(TileSizeDialog::new(&e).tile_size, (8, 256), "clamped to 1-256");
    }
}
```

En los tests de `crates/cs-ui/src/tile_ops.rs`:

```rust
    #[test]
    fn new_tilemap_layer_with_a_new_or_existing_tileset_is_one_undo_step() {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        assert!(e.add_tilemap_layer(&TilesetChoice::New {
            name: "ground".into(),
            tile_size: (8, 4),
            base_index: 1,
        }));
        assert_ne!(e.layer, LayerId(1), "the new layer is selected");
        assert!(e.tiles_mode());
        assert_eq!(e.sprite().tilesets[0].tile_size, (8, 4));
        assert!(e.add_tilemap_layer(&TilesetChoice::Existing(0)));
        assert_eq!(e.sprite().tilesets.len(), 1);
        assert_eq!(e.sprite().layer(e.layer).unwrap().tileset, Some(0));
        e.undo();
        assert_eq!(e.sprite().layers.len(), 2);
        e.undo();
        assert_eq!(e.sprite().layers.len(), 1);
        assert!(
            e.sprite().tilesets.is_empty(),
            "the new tileset goes away with the same undo"
        );
        assert!(!e.add_tilemap_layer(&TilesetChoice::Existing(3)));
        // Cualquier modo de color: los tiles se crean en el modo del sprite.
        let indexed = ColorMode::Indexed {
            transparent_index: 0,
        };
        let mut e = Editor::with_sprite(Sprite::new(8, 8, indexed));
        assert!(e.add_tilemap_layer(&TilesetChoice::New {
            name: "t".into(),
            tile_size: (4, 4),
            base_index: 1,
        }));
        assert_eq!(
            e.sprite().tilesets[0].tiles[0].get(0, 0),
            Some(PixelValue::Index(0))
        );
    }

    #[test]
    fn convert_layers_from_the_editor() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, red)],
        });
        assert!(e.can_convert_to_tilemap() && !e.can_convert_to_image());
        assert!(!e.convert_to_tilemap((0, 4)));
        assert!(e.convert_to_tilemap((4, 4)));
        assert!(e.tiles_mode() && e.can_convert_to_image());
        assert!(e.convert_to_image());
        assert!(!e.tiles_mode());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 0, 0), red);
        e.undo();
        assert!(e.tiles_mode());
        e.undo();
        assert!(e.can_convert_to_tilemap());
        assert!(e.sprite().tilesets.is_empty());
    }
```

En `crates/cs-ui/src/lib.rs`, después de `mod tile_ops;`, agregar `mod tilemap_dialog;`.

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-ui tilemap`
Expected: error de compilación (`NewTilemapDialog`, `TileSizeDialog`, `TilesetChoice`,
`add_tilemap_layer`, `convert_to_tilemap`… no existen).

- [ ] **Step 3: Implementar**

**`crates/cs-ui/src/tile_ops.rs`:** agregar `use cs_commands::ops;` a los imports y, antes de
`impl Editor`:

```rust
/// Tileset de una capa tilemap nueva: uno del sprite o uno nuevo.
#[derive(Clone, Debug, PartialEq)]
pub enum TilesetChoice {
    Existing(u32),
    New {
        name: String,
        tile_size: (u32, u32),
        base_index: i16,
    },
}
```

y al final de `impl Editor`:

```rust
    /// Nueva capa tilemap sobre la actual, con un tileset existente o uno nuevo (en el modo de
    /// color del sprite). Un solo paso de undo; la capa nueva queda seleccionada.
    pub fn add_tilemap_layer(&mut self, choice: &TilesetChoice) -> bool {
        let sprite = self.sprite();
        let mut cmds = Vec::new();
        let index = match choice {
            TilesetChoice::Existing(i) if (*i as usize) < sprite.tilesets.len() => *i,
            TilesetChoice::Existing(_) => return false,
            TilesetChoice::New {
                name,
                tile_size,
                base_index,
            } => {
                let (w, h) = *tile_size;
                if !(1..=ops::MAX_TILE_SIDE).contains(&w) || !(1..=ops::MAX_TILE_SIDE).contains(&h)
                {
                    return false;
                }
                let mut tileset = Tileset::new(name.clone(), (w, h), sprite.color_mode);
                tileset.base_index = *base_index;
                let index = sprite.tilesets.len();
                cmds.push(Command::SetTileset {
                    index,
                    tileset: Some(tileset),
                });
                index as u32
            }
        };
        let add = ops::new_tilemap_layer(sprite, Some(self.layer), index);
        let Command::AddLayer { layer, .. } = &add else {
            unreachable!("new_tilemap_layer devuelve AddLayer");
        };
        let id = layer.id;
        cmds.push(add);
        if self.execute(Command::Batch(cmds)) {
            self.select_layer(id);
            true
        } else {
            false
        }
    }

    pub fn can_convert_to_tilemap(&self) -> bool {
        self.sprite()
            .layer(self.layer)
            .is_some_and(|l| l.kind == LayerKind::Image)
    }

    pub fn can_convert_to_image(&self) -> bool {
        self.active_tileset().is_some()
    }

    /// `Layer → Convert to Tilemap`: `false` si no se pudo (ver `ops::convert_to_tilemap`).
    pub fn convert_to_tilemap(&mut self, tile_size: (u32, u32)) -> bool {
        let Some(cmd) = ops::convert_to_tilemap(self.sprite(), self.layer, tile_size) else {
            return false;
        };
        self.execute(cmd)
    }

    /// `Layer → Convert to Image Layer`.
    pub fn convert_to_image(&mut self) -> bool {
        let Some(cmd) = ops::convert_to_image(self.sprite(), self.layer) else {
            return false;
        };
        self.execute(cmd)
    }
```

**`crates/cs-ui/src/tilemap_dialog.rs`**, arriba de los tests:

```rust
use cs_commands::ops::MAX_TILE_SIDE;
use eframe::egui;

use crate::editor::Editor;
use crate::tile_ops::TilesetChoice;
use crate::widgets;

/// Lo que el usuario decidió en este frame.
pub enum DialogResult {
    Open,
    Accept,
    Cancel,
}

/// Tamaño de tile por defecto: el de la grilla de trabajo del sprite, acotado a 1–256.
pub fn default_tile_size(editor: &Editor) -> (u32, u32) {
    let (w, h) = editor.sprite().grid.size;
    (w.clamp(1, MAX_TILE_SIDE), h.clamp(1, MAX_TILE_SIDE))
}

/// Enter acepta y Escape (o cerrar la ventana) cancela, como en los demás diálogos.
fn finish(ctx: &egui::Context, open: bool, result: DialogResult) -> DialogResult {
    let (enter, escape) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if !open || escape {
        DialogResult::Cancel
    } else if enter {
        DialogResult::Accept
    } else {
        result
    }
}

fn buttons(ui: &mut egui::Ui, result: &mut DialogResult) {
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("OK").clicked() {
            *result = DialogResult::Accept;
        }
        if ui.button("Cancel").clicked() {
            *result = DialogResult::Cancel;
        }
    });
}

/// `Layer → New Tilemap Layer…`: tileset existente o nuevo (nombre, tamaño de tile, base index).
pub struct NewTilemapDialog {
    /// `None` = tileset nuevo.
    pub existing: Option<u32>,
    pub name: String,
    pub tile_size: (u32, u32),
    pub base_index: i16,
    /// Tilesets del sprite para el selector: índice, nombre y tamaño de tile.
    tilesets: Vec<(u32, String, (u32, u32))>,
}

impl NewTilemapDialog {
    pub fn new(editor: &Editor) -> Self {
        let sprite = editor.sprite();
        Self {
            existing: None,
            name: format!("Tileset {}", sprite.tilesets.len() + 1),
            tile_size: default_tile_size(editor),
            base_index: 1,
            tilesets: sprite
                .tilesets
                .iter()
                .enumerate()
                .map(|(i, t)| (i as u32, t.name.clone(), t.tile_size))
                .collect(),
        }
    }

    pub fn choice(&self) -> TilesetChoice {
        match self.existing {
            Some(i) => TilesetChoice::Existing(i),
            None => TilesetChoice::New {
                name: self.name.clone(),
                tile_size: self.tile_size,
                base_index: self.base_index,
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> DialogResult {
        let mut result = DialogResult::Open;
        let mut open = true;
        egui::Window::new("New Tilemap Layer")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let label = |t: &(u32, String, (u32, u32))| format!("{}  ({}×{})", t.1, t.2.0, t.2.1);
                let selected = self
                    .existing
                    .and_then(|i| self.tilesets.iter().find(|t| t.0 == i))
                    .map_or_else(|| "New tileset".to_string(), label);
                egui::ComboBox::from_label("Tileset")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.existing, None, "New tileset");
                        for t in &self.tilesets {
                            ui.selectable_value(&mut self.existing, Some(t.0), label(t));
                        }
                    });
                if self.existing.is_none() {
                    egui::Grid::new("new_tileset")
                        .num_columns(2)
                        .spacing([8.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Name:");
                            ui.text_edit_singleline(&mut self.name);
                            ui.end_row();
                            ui.label("Tile width:");
                            widgets::number(ui, &mut self.tile_size.0, 1..=MAX_TILE_SIDE, " px");
                            ui.end_row();
                            ui.label("Tile height:");
                            widgets::number(ui, &mut self.tile_size.1, 1..=MAX_TILE_SIDE, " px");
                            ui.end_row();
                            ui.label("Base index:");
                            widgets::number(ui, &mut self.base_index, i16::MIN..=i16::MAX, "");
                            ui.end_row();
                        });
                }
                buttons(ui, &mut result);
            });
        finish(ctx, open, result)
    }
}

/// `Layer → Convert to Tilemap…`: tamaño de tile.
pub struct TileSizeDialog {
    pub tile_size: (u32, u32),
}

impl TileSizeDialog {
    pub fn new(editor: &Editor) -> Self {
        Self {
            tile_size: default_tile_size(editor),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> DialogResult {
        let mut result = DialogResult::Open;
        let mut open = true;
        egui::Window::new("Convert to Tilemap")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Grid::new("convert_tile_size")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Tile width:");
                        widgets::number(ui, &mut self.tile_size.0, 1..=MAX_TILE_SIDE, " px");
                        ui.end_row();
                        ui.label("Tile height:");
                        widgets::number(ui, &mut self.tile_size.1, 1..=MAX_TILE_SIDE, " px");
                        ui.end_row();
                    });
                ui.weak("Cuts the layer into cells of the canvas grid; identical cells share a tile.");
                buttons(ui, &mut result);
            });
        finish(ctx, open, result)
    }
}
```

Si el borrow checker rechaza el `label` usado dentro y fuera del `show_ui` (closures que capturan
`self.tilesets` y `self.existing` a la vez), convertir `label` en una `fn tileset_label(t: &(u32, String, (u32, u32))) -> String`
a nivel de módulo; el comportamiento es el mismo.

**`crates/cs-ui/src/timeline.rs`:** al final de `struct TimelineState`:

```rust
    /// Se pidió el diálogo "New Tilemap Layer" (lo abre la app).
    pub new_tilemap: bool,
```

y después del botón "+ Group" (líneas 129-131):

```rust
        if ui
            .button("+ Tilemap")
            .on_hover_text("New tilemap layer…")
            .clicked()
        {
            state.new_tilemap = true;
        }
```

**`crates/cs-ui/src/file_actions.rs`:** en `is_modal`, agregar
`|| self.new_tilemap.is_some()` y `|| self.convert_tilemap.is_some()` después de
`|| self.grid_dialog.is_some()`.

**`crates/cs-ui/src/lib.rs`:**

1. Campos de `ChimpApp`, después de `grid_dialog` (línea 47):

```rust
    new_tilemap: Option<tilemap_dialog::NewTilemapDialog>,
    convert_tilemap: Option<tilemap_dialog::TileSizeDialog>,
```

   e inicialización después de `grid_dialog: None,` (línea 81): `new_tilemap: None,` y
   `convert_tilemap: None,`.

2. En `ChimpApp::new`, después del bloque `CHIMPSPRITE_DEMO_CANVAS_SIZE` (termina en la línea 119):

```rust
        // Desarrollo: abre el diálogo de nueva capa tilemap para capturas (con --demo).
        if std::env::var_os("CHIMPSPRITE_DEMO_NEW_TILEMAP").is_some() && app.has_doc() {
            app.new_tilemap = Some(tilemap_dialog::NewTilemapDialog::new(&app.doc.editor));
        }
```

3. Después de `update_grid_dialog`:

```rust
    fn update_tilemap_dialogs(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.new_tilemap = None;
            self.convert_tilemap = None;
            return;
        }
        if let Some(dialog) = &mut self.new_tilemap {
            match dialog.show(ctx) {
                tilemap_dialog::DialogResult::Open => {}
                tilemap_dialog::DialogResult::Accept => {
                    let choice = dialog.choice();
                    self.new_tilemap = None;
                    self.doc.editor.add_tilemap_layer(&choice);
                }
                tilemap_dialog::DialogResult::Cancel => self.new_tilemap = None,
            }
        }
        if let Some(dialog) = &mut self.convert_tilemap {
            match dialog.show(ctx) {
                tilemap_dialog::DialogResult::Open => {}
                tilemap_dialog::DialogResult::Accept => {
                    let size = dialog.tile_size;
                    self.convert_tilemap = None;
                    if !self.doc.editor.convert_to_tilemap(size) {
                        self.show_message(
                            "Convert to Tilemap",
                            format!(
                                "Could not convert the layer: it would need more than {} \
                                 different tiles, or a cel would exceed the tilemap size limit. \
                                 Try a larger tile size.",
                                cs_core::MAX_TILESET_TILES
                            ),
                        );
                    }
                }
                tilemap_dialog::DialogResult::Cancel => self.convert_tilemap = None,
            }
        }
    }
```

   y en `ui()`, después de `self.update_grid_dialog(&ctx);` (línea 672):
   `self.update_tilemap_dialogs(&ctx);`.

4. Menú Layer (líneas 490-495), reemplazar el cuerpo del `ui.menu_button("Layer", …)` por:

```rust
                ui.menu_button("Layer", |ui| {
                    if ui.button("New Tilemap Layer…").clicked() {
                        self.new_tilemap =
                            Some(tilemap_dialog::NewTilemapDialog::new(&self.doc.editor));
                        ui.close();
                    }
                    let to_tilemap = self.doc.editor.can_convert_to_tilemap();
                    if ui
                        .add_enabled(to_tilemap, egui::Button::new("Convert to Tilemap…"))
                        .on_disabled_hover_text("Select an image layer")
                        .clicked()
                    {
                        self.convert_tilemap =
                            Some(tilemap_dialog::TileSizeDialog::new(&self.doc.editor));
                        ui.close();
                    }
                    let to_image = self.doc.editor.can_convert_to_image();
                    if ui
                        .add_enabled(to_image, egui::Button::new("Convert to Image Layer"))
                        .on_disabled_hover_text("Select a tilemap layer")
                        .clicked()
                    {
                        self.doc.editor.convert_to_image();
                        ui.close();
                    }
                    ui.separator();
                    let layer = self.doc.editor.layer;
                    timeline::instance_menu(ui, &mut self.doc.editor);
                    ui.separator();
                    timeline::mask_menu(ui, &mut self.doc.editor, layer);
                });
```

5. En `editor_panels`, después del `if self.view.timeline { … }` (termina ~línea 573):

```rust
        if std::mem::take(&mut self.doc.timeline.new_tilemap) {
            self.new_tilemap = Some(tilemap_dialog::NewTilemapDialog::new(&self.doc.editor));
        }
```

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo fmt --all && cargo test -p cs-ui && cargo clippy -p cs-ui --all-targets -- -D warnings`
Expected: PASS, sin warnings. (No ejecutar la app.)

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui
git commit -m "feat(ui): New Tilemap Layer dialog and Convert to Tilemap/Image" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Lienzo — grilla de tiles, Show Tile Numbers y pincel en celdas

**Files:**
- Modify: `crates/cs-ui/src/tile_ops.rs` (`numbered_cells`, test)
- Modify: `crates/cs-ui/src/view.rs:13-53` (`tile_numbers`) y test
- Modify: `crates/cs-ui/src/theme.rs` (`TILE_GRID`)
- Modify: `crates/cs-ui/src/canvas.rs` (import línea 14; grilla después del pixel grid ~847-861;
  números después de las marching ants ~952; pincel 969-990; constante arriba)
- Modify: `crates/cs-ui/src/lib.rs` (menú View → Show, línea 233; hook `CHIMPSPRITE_DEMO_TILES`)

**Interfaces:**
- Consumes: `TileProxy`, `TileSpace`, `tile_number` (Task 1), `Editor::tile_space`,
  `ToolKind::uses_cells` (Task 3), `widgets::outlined_text` (Task 6).
- Produces:
  - `pub fn tile_ops::numbered_cells(sprite: &Sprite, layer: LayerId, frame: usize) -> Vec<((i32, i32), i64)>`
    (esquina en píxeles y número de usuario de cada celda no vacía)
  - `ViewSettings::tile_numbers: bool` (por defecto `false`, persistido)
  - `theme::TILE_GRID: Color32`
  - Env de desarrollo `CHIMPSPRITE_DEMO_TILES`: al abrir el documento activa su capa tilemap de más
    arriba y los números de tile (para capturas).

- [ ] **Step 1: Escribir los tests que fallan**

En los tests de `crates/cs-ui/src/tile_ops.rs`:

```rust
    #[test]
    fn numbered_cells_use_the_user_numbering() {
        let mut s = tiled(16, 8, (0, 0));
        s.tilesets[0].base_index = 10;
        assert_eq!(
            numbered_cells(&s, LayerId(2), 0),
            vec![((0, 0), 10), ((4, 0), 11)]
        );
        assert!(numbered_cells(&s, LayerId(1), 0).is_empty());
    }
```

En `crates/cs-ui/src/view.rs`, en `settings_roundtrip_through_json_with_defaults`, agregar al final:

```rust
        assert!(!v.tile_numbers, "tile numbers start hidden");
        let v: ViewSettings = serde_json::from_str(r#"{"tile_numbers":true}"#).unwrap();
        assert!(v.tile_numbers && v.timeline);
```

- [ ] **Step 2: Correr y verificar que falla**

Run: `cargo test -p cs-ui numbered_cells && cargo test -p cs-ui settings_roundtrip`
Expected: error de compilación (`numbered_cells` y `tile_numbers` no existen).

- [ ] **Step 3: Implementar**

**`crates/cs-ui/src/tile_ops.rs`**, después de `cells_to_pixels`:

```rust
/// Celdas no vacías de la capa tilemap con su esquina en píxeles y su número de usuario (para
/// `View → Show Tile Numbers`). Vacío si la capa no es tilemap.
pub fn numbered_cells(sprite: &Sprite, layer: LayerId, frame: usize) -> Vec<((i32, i32), i64)> {
    let Some(proxy) = TileProxy::build(sprite, layer, frame, None) else {
        return Vec::new();
    };
    let base = sprite
        .layer(layer)
        .and_then(|l| l.tileset)
        .and_then(|i| sprite.tilesets.get(i as usize))
        .map_or(1, |t| t.base_index);
    let (w, h) = (proxy.sprite.width as i32, proxy.sprite.height as i32);
    (0..h)
        .flat_map(|cy| (0..w).map(move |cx| (cx, cy)))
        .filter_map(|c| {
            let tile = proxy.tile_at(c);
            (tile.index() != 0)
                .then(|| (proxy.space.cell_origin(c), tile_number(tile.index(), base)))
        })
        .collect()
}
```

**`crates/cs-ui/src/view.rs`:** en `struct ViewSettings`, después de `lock_guides`:

```rust
    /// Número de cada celda no vacía de la capa tilemap activa.
    pub tile_numbers: bool,
```

y en `Default`, después de `lock_guides: false,`: `tile_numbers: false,`.

**`crates/cs-ui/src/theme.rs`**, después de `BRUSH_OUTLINE`:

```rust
/// Grilla de tiles de la capa tilemap activa (naranja translúcido, distinto del grid de trabajo).
pub const TILE_GRID: Color32 = Color32::from_rgba_premultiplied(140, 77, 22, 140);
```

**`crates/cs-ui/src/canvas.rs`:**

1. Línea 14: `use crate::theme;` pasa a `use crate::{theme, widgets};`. Después de `LABEL_GAP`:

```rust
/// Los números de tile se dibujan solo si cada tile ocupa al menos esto en pantalla.
const TILE_NUMBER_MIN_PX: f32 = 16.0;
```

2. En el `for artboard in &document.artboards` del dibujo, después del bloque del pixel grid
   (`if self.zoom >= 8.0 && view_settings.shows(view_settings.pixel_grid) { … }`) y antes de
   `let active = artboard.id == editor.active;`:

```rust
            // Modo Tiles: la grilla de tiles de la capa activa, independiente del grid de trabajo.
            if artboard.id == editor.active
                && view_settings.extras
                && let Some(space) = editor.tile_space()
            {
                draw_grid(
                    &painter,
                    screen,
                    GridSettings {
                        size: space.tile_size,
                        offset: space.offset,
                    },
                    self.zoom,
                    theme::TILE_GRID,
                );
            }
```

3. Después del bloque de las marching ants (el `if let (Some(mask), Some(origin)) = …` que llama a
   `draw_marching_ants`):

```rust
        if let Some(origin) = active_origin
            && view_settings.shows(view_settings.tile_numbers)
            && let Some(space) = editor.tile_space()
            && space.tile_size.0 as f32 * self.zoom >= TILE_NUMBER_MIN_PX
            && space.tile_size.1 as f32 * self.zoom >= TILE_NUMBER_MIN_PX
            && let Some(artboard) = document.artboard(editor.active)
        {
            let font = FontId::monospace(10.0);
            let cells = crate::tile_ops::numbered_cells(&artboard.sprite, editor.layer, editor.frame);
            for ((x, y), number) in cells {
                let at = origin + Vec2::new(x as f32, y as f32) * self.zoom + Vec2::splat(2.0);
                if !rect.contains(at) {
                    continue;
                }
                widgets::outlined_text(
                    &painter,
                    at,
                    Align2::LEFT_TOP,
                    &number.to_string(),
                    font.clone(),
                );
            }
        }
```

4. En el contorno del pincel, reemplazar desde `let offset = -((size as i32 - 1) / 2);` hasta el
   final del `if tool.paints() && tool != ToolKind::Eraser && tool != ToolKind::Bucket { … }` por:

```rust
            let offset = -((size as i32 - 1) / 2);
            // En modo Tiles el pincel cubre celdas enteras.
            let cells = editor.tile_space().filter(|_| tool.uses_cells());
            let r = match cells {
                Some(space) => {
                    let (cx, cy) = space.cell_at((hx, hy));
                    let (x0, y0) = space.cell_origin((cx + offset, cy + offset));
                    let (tw, th) = space.tile_size;
                    Rect::from_min_size(
                        origin + Vec2::new(x0 as f32, y0 as f32) * self.zoom,
                        Vec2::new((size * tw) as f32, (size * th) as f32) * self.zoom,
                    )
                }
                None => Rect::from_min_size(
                    origin + Vec2::new((hx + offset) as f32, (hy + offset) as f32) * self.zoom,
                    Vec2::splat(size as f32 * self.zoom),
                ),
            };
            // Las herramientas que pintan píxeles muestran el color primario bajo el cursor.
            if cells.is_none()
                && tool.paints()
                && tool != ToolKind::Eraser
                && tool != ToolKind::Bucket
            {
                let c = editor.resolve(editor.primary);
                painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a));
            }
```

   (el `painter.rect_stroke(r, …)` que sigue no cambia).

**`crates/cs-ui/src/lib.rs`:**

1. Menú View → Show (línea 233), después de `ui.checkbox(&mut v.artboard_names, "Artboard Names");`:
   `ui.checkbox(&mut v.tile_numbers, "Tile Numbers");`.

2. Campo de `ChimpApp` (después de `convert_tilemap`):

```rust
    /// Desarrollo (`CHIMPSPRITE_DEMO_TILES`): al abrir el documento, activar su capa tilemap.
    demo_tiles: bool,
```

   inicializado en `new` con `demo_tiles: std::env::var_os("CHIMPSPRITE_DEMO_TILES").is_some(),`,
   y el método (después de `update_tilemap_dialogs`):

```rust
    /// Desarrollo: activa la capa tilemap de más arriba y los números de tile (para capturas).
    fn demo_tiles(&mut self) {
        if !self.demo_tiles || !self.has_doc() {
            return;
        }
        self.demo_tiles = false;
        self.view.tile_numbers = true;
        let editor = &mut self.doc.editor;
        let top = editor
            .sprite()
            .layers
            .iter()
            .rev()
            .find(|l| l.kind == cs_core::LayerKind::Tilemap)
            .map(|l| l.id);
        if let Some(id) = top {
            editor.select_layer(id);
        }
    }
```

   llamado en `ui()` después de `self.handle_platform_events(&ctx);`: `self.demo_tiles();`.

- [ ] **Step 4: Correr y verificar que pasa**

Run: `cargo fmt --all && cargo test -p cs-ui && cargo clippy -p cs-ui --all-targets -- -D warnings`
Expected: PASS, sin warnings. (No ejecutar la app.)

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui
git commit -m "feat(ui): tile grid, Show Tile Numbers and cell brush preview" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Verificación visual y CI

**Files:** ninguno (solo si la verificación encuentra un problema; ver Step 4).

**Interfaces:**
- Consumes: todo lo anterior; `CHIMPSPRITE_SCREENSHOT`, `CHIMPSPRITE_DEMO_TILES`,
  `CHIMPSPRITE_DEMO_NEW_TILEMAP`, el fixture `crates/cs-io/tests/fixtures/tilemap_rgba.aseprite`
  (32×24, tileset "Terrain" de 8×8, capas tilemap "Ground" y "Deco").
- Produces: capturas en `target/` revisadas; CI verde.

- [ ] **Step 1: Correr lo mismo que CI**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --target wasm32-unknown-unknown -p cs-core -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite
```

Expected: sin errores ni warnings. Si `clippy`/`test` fallan con "Acceso denegado" al enlazar
`chimpsprite.exe`, repetir con `CARGO_TARGET_DIR=target/gui-check`.

- [ ] **Step 2: Respaldar `app.ron` y capturar**

En Git Bash, desde la raíz del repo:

```bash
cp "$APPDATA/ChimpSprite/data/app.ron" target/app.ron.bak
export CARGO_TARGET_DIR=target/gui-check
CHIMPSPRITE_DEMO_TILES=1 CHIMPSPRITE_SCREENSHOT=/d/Work/chimpdeas/chimpsprite/target/tiles-mode.png cargo run -p chimpsprite -- crates/cs-io/tests/fixtures/tilemap_rgba.aseprite
CHIMPSPRITE_DEMO_NEW_TILEMAP=1 CHIMPSPRITE_SCREENSHOT=/d/Work/chimpdeas/chimpsprite/target/new-tilemap.png cargo run -p chimpsprite -- --demo
cp target/app.ron.bak "$APPDATA/ChimpSprite/data/app.ron"
```

(Si `app.ron` no existía antes del primer `cp`, en vez de restaurarlo borrar el que creó la app.)
La restauración se hace **siempre**, aunque una captura falle.

- [ ] **Step 3: Revisar las capturas**

Abrir `target/tiles-mode.png` con Read y comprobar:

1. El panel derecho muestra "Tileset" (no "Color"/paleta): "Terrain  8×8  3 tiles", una grilla de
   miniaturas con el vacío primero (damero con la diagonal roja y "0") y los tiles 1–3 con su
   número; el tile 1 lleva el borde blanco; debajo, los toggles Flip X / Y / D y el selector de
   color.
2. El lienzo muestra la grilla de tiles naranja de 8×8 alineada al mapa y, en cada celda no vacía
   de "Deco", su número con contorno legible.
3. La barra de estado dice "Tiles mode: painting with tile 1".
4. En el timeline, "Deco" está activa y aparece el botón "+ Tilemap".

Abrir `target/new-tilemap.png` y comprobar el diálogo "New Tilemap Layer": selector "Tileset" en
"New tileset", Name "Tileset 1", Tile width/height 16 px, Base index 1, OK y Cancel.

- [ ] **Step 4: Corregir si hace falta**

Si algo no se ve como se describe (texto cortado, glifo faltante, grilla corrida, miniaturas
borrosas), corregirlo en el archivo de la task correspondiente, repetir los Steps 1-3 (con el
respaldo y la restauración de `app.ron`) y commitear:

```bash
git add crates/cs-ui
git commit -m "fix(ui): polish Tiles mode visuals" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 5: Commit de formato (si `cargo fmt` cambió algo)**

```bash
git add -A && git commit -m "style: cargo fmt" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"   # solo si hubo cambios
```

---

## Self-review

**Cobertura de la spec:**
- §1 proxy (tamaño, contenido, codificación, cel no alineado): Task 1. Contexto (color/secondary,
  pincel en celdas, selección en celdas), puntero → celdas, traducción de `DrawPixels`
  (`set_tiles`, borrador = vacío, `None` → nada o aviso de límite), selección celdas → píxeles,
  vista previa: Task 3. Tabla de herramientas: Task 3 (lápiz, borrador, formas, cubeta, cuentagotas,
  marquees, varita vía Task 2) y Move por tiles (Task 2). Herramientas bloqueadas, pixel-perfect y
  simetría ignorados, aviso nuevo "Tiles mode: painting with tile N": Task 3. Grilla en el canvas:
  Task 8.
- §2 panel Tileset (encabezado, miniaturas 32 px vecino más cercano, vacío primero con su dibujo,
  números, clic, borde, cache por revisión), flips X/Y/D (D solo cuadrados), atajos X/Y, reinicio
  del pincel: Tasks 3 y 6.
- §3 nueva capa tilemap (menú y "+ Tilemap", tileset existente o nuevo, valores por defecto, un solo
  `Batch`, capa seleccionada, cualquier modo de color): Task 7.
- §4 `convert_to_tilemap` / `convert_to_image` (celdas del lienzo, deduplicación exacta sin flips,
  posición de la primera celda, opacidad, vínculos, propiedades e id, tileset con el nombre de la
  capa, un `Batch`, `None` con aviso en la UI) y menú: Tasks 4 y 7.
- §5 Show Tile Numbers (persistido, 16 px por tile, texto con contorno): Task 8.
- §6 MCP `convert_layer`: Task 5.
- §7 tests: proxy (Task 1), editor (Task 3), diálogo (Task 7), `cs-commands` (Task 4), `cs-agent`
  (Task 5), verificación visual con `app.ron` y `target/gui-check` (Task 9).

**Tipos consistentes:** `TileProxy::build(&Sprite, LayerId, usize, Option<&Mask>)`, `TileSpace`
(`Copy`), `TilesetChoice`, `Editor::brush_tile() -> Tile`, `toggle_brush_flip(u32)`,
`ops::convert_to_tilemap(&Sprite, LayerId, (u32, u32)) -> Option<Command>`,
`ops::convert_to_image(&Sprite, LayerId) -> Option<Command>`, `ops::MAX_TILE_SIDE`,
`cs_core::{MAX_TILESET_TILES, draw_tile_grid, tile_grid_pixel}` se usan con la misma firma en todas
las tasks.
