# Plan: Tilemaps 2B, píxeles sobre tilemaps

Spec: `docs/superpowers/specs/2026-11-06-tilemaps-pixels-design.md`. Rama `tiles-pixels`. TDD: cada
tarea empieza por sus tests, termina con tests + clippy en verde y un commit.

## Tarea 1: orientar tiles (`cs-core`)
- `Tile::flags`, `Tile::with_index`, `orient_tile`, `unorient_tile` (reusan `tile_source`).
- Tests: ida y vuelta con los 8 flips en un tile asimétrico; `orient_tile` == lo que dibuja
  `draw_tile_grid`.

## Tarea 2: `tiles::draw_tilemap_pixels` (`cs-commands`)
- Módulo `tiles.rs` (+ `tiles/tests.rs`): `TilesetMode`, `TilePaintError`, el índice de búsqueda con
  flips, el conteo de usos, Manual/Auto/Stack, borrado + remapeo, crecimiento de la grilla sin
  desvincular.
- Fixtures de Aseprite: `tests/fixtures/tile_pixels.{json,lua,expected}` y
  `tests/tile_pixels_aseprite.rs`.
- Unitarios: undo exacto, indexado, límites, fuera del lienzo, no tilemap, user data.

## Tarea 3: `tiles::move_tile`
- Reordenar + remapeo (flips intactos, todas las capas del tileset, user data). Tests y undo.

## Tarea 4: modo Pixels en el editor (`cs-ui`)
- `tilemap_pixels`, `tilemap_pixels_mode()`, `tiles_mode()` ajustado, `ToolOptions::tileset_mode`.
- Proxy de píxeles cacheado; `with_tool`/`view`/`apply_output`/`commit_text`/Delete pasan por
  `draw_tilemap_pixels`; ink sobre el proxy; aviso de límite; estado.
- Tests de editor.

## Tarea 5: barra de opciones, atajos y panel
- Grupos Pixels|Tiles y Manual|Auto|Stack (`bar_fields::options` / radios).
- Space+Tab y Space+1/2/3.
- Panel: paleta en Pixels.
- Tests de layout/panel.

## Tarea 6: reordenar en el panel Tileset
- Arrastrar y soltar miniaturas → `Editor::move_tile` (mueve el pincel). Tests.

## Tarea 7: MCP
- `draw_tilemap_pixels` y `move_tile` en `cs-agent` y el servidor. Tests.

## Tarea 8: verificación
- `cargo test --workspace`, clippy, fmt, wasm; demo con una capa tilemap en modo Pixels y captura;
  autorrevisión del diff; minors en `.superpowers/sdd/tiles2b-minors.md` y sección en `progress.md`.
