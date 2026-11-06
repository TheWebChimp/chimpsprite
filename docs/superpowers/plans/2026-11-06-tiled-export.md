# Plan: Tilemaps 3, exportar a Tiled (e importar)

Spec: `docs/superpowers/specs/2026-11-06-tiled-export-design.md`. Rama `tiled-export`. TDD: cada
tarea empieza por sus tests, termina con tests + clippy en verde y un commit.

## Tarea 1: modelo + TMX/TSX (`cs-io/src/tiled.rs`, `tiled/tmx.rs`)
- Dependencias `quick-xml` y `base64` (ya en el lock).
- `Map`, `MapTileset`, `TiledTileset`, `MapLayer`, `LayerContent`, `Property`, `Encoding`.
- Codificar/decodificar datos de capa (CSV, base64, zlib, gzip).
- `write_tmx`/`write_tsx` y `read_tmx`/`read_tsx`. Tests: ida y vuelta de un mapa a mano con
  todas las codificaciones; escapes XML; el XML parseado tiene los atributos del formato.

## Tarea 2: TMJ/TSJ (`tiled/tmj.rs`)
- `write_tmj`/`write_tsj`, `read_tmj`/`read_tsj` con serde_json. Tests: ida y vuelta y campos.

## Tarea 3: Sprite → Map (`tiled/export.rs`)
- `TiledOptions`, `TiledFrames`, `export()`: bake, tilesets (PNG en grilla), capas, grupos,
  imagelayers, slices, propiedades, frames, nombres saneados, avisos.
- Tests: 8 flips → GIDs, dos tilesets, offsets, tamaño redondeado, capa con otro tile size, blend,
  hidden/opacity, all frames, sin tilemaps → error.

## Tarea 4: Map → Sprite (`tiled/import.rs`)
- `import()` con cargador de archivos vecinos; partir capas multi-tileset; slices.
- Tests: round-trip export → import; trans; margin/spacing; infinito → error.

## Tarea 5: diálogo y menú (`cs-ui`)
- `tiled_dialog.rs`, `Platform::choose_folder`, ítems de File, zip en web, import en escritorio y al
  soltar. Tests de nombres/zip/deshabilitado y de menú.

## Tarea 6: MCP `kind: "tiled"` (`cs-agent`)

## Tarea 7: demo `CHIMPSPRITE_DEMO_WINDOW=tiled`, captura, verificación completa, minors.
