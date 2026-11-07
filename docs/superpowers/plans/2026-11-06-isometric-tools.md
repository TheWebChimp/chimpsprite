# Herramientas isométricas — Implementation Plan

**Goal:** Iso Box (caja 2:1 pixel-perfect con colores por cara, contorno y highlight), Iso Blocks
(pilas de bloques en una rejilla iso), opción Isometric de la Línea y cuadrícula isométrica, con
el motor en `cs_raster::iso`, operaciones MCP y un diálogo que deja el resultado flotando.

**Architecture:** geometría pura en `cs_raster::iso` (partes por píxel: Top/Left/Right/Outline/
Highlight) y colores aparte (`IsoColors`); `cs-tools` usa `constrain_iso`; `cs-core` gana
`GridKind`; `cs-agent` arma `Command::DrawPixels`; `cs-ui/iso_dialog.rs` arma el diálogo y deja
el bloque flotando con `Editor::float_pasted` (un paso de undo al confirmar).

Spec: `docs/superpowers/specs/2026-11-06-isometric-tools-design.md`.

## Global Constraints

- Comentarios en español; textos de la UI en inglés. Repo `eol=lf`.
- Nada de `git stash` / `checkout --` / `restore` / `reset --hard` / `clean` / `add -A`.
- Cargo con el toolchain aislado, `CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-font/target`, a baja
  prioridad (`cmd //c "start /low /b /wait cargo …"`).
- Antes de cada commit: fmt, tests y clippy `-D warnings` de los crates tocados.

---

### Task 1: geometría de la caja (`cs_raster::iso::iso_box`)

**Files:** create `crates/cs-raster/src/iso.rs`, `crates/cs-raster/src/iso_tests.rs`; modify
`crates/cs-raster/src/lib.rs`.

- [ ] Tests con dibujos en texto: 2×2×2 con 2, 3 y 4 px; baldosa plana; 32×16 de 4 px; medidas;
      simetría con `left == right`; aristas en tramos de 2; sin contorno ni highlight.
- [ ] Implementar `IsoCorner`, `IsoBox`, `IsoPart`, `IsoParts`, `iso_box`.
- [ ] Commit.

### Task 2: colores (`IsoColors`, `paint_parts`, paleta)

- [ ] Tests: `from_base` (orden de luminancias, alfa), `color(part)`, `paint_parts`,
      `snapped_to(palette)` sin el transparente.
- [ ] Implementar. Commit.

### Task 3: bloques (`IsoBlocks`, `iso_blocks`)

- [ ] Tests: uno = caja; apilados = caja alta; lado a lado = caja ancha; adelante tapa a atrás;
      vacío = 0×0; contorno de la silueta.
- [ ] Implementar. Commit.

### Task 4: línea isométrica

**Files:** `cs-raster/src/constrain.rs`, `cs-tools/src/{lib,shape}.rs`,
`cs-ui/src/editor/{tool_options,gestures}.rs`, `cs-ui/src/panels/options_bar.rs`.

- [ ] Tests: `constrain_iso` (solo 0°, 90° y 2:1; tramos iguales); Line con `ctx.iso`.
- [ ] `ToolOptions::iso_lines` (serde default) y la casilla Isometric de la Línea.
- [ ] Commit.

### Task 5: cuadrícula isométrica

**Files:** `cs-core/src/sprite.rs`, `cs-raster/src/iso.rs` (`iso_grid_lines`, `snap_iso`),
`cs-ui/src/{canvas,view,guides}.rs`, `cs-io/src/chimp.rs` y los `GridSettings { .. }` literales.

- [ ] Tests: serde de `GridKind` (lo viejo es Orthogonal); segmentos de la cuadrícula; snap al
      vértice; Grid Settings con Type; `.chimp` guarda el tipo.
- [ ] Implementar; dibujarla en el lienzo. Commit.

### Task 6: MCP (`draw_iso_box`, `draw_iso_blocks`)

**Files:** `cs-agent/src/{params,dispatch}.rs`, `cs-agent/src/ops/iso.rs`,
`apps/chimpsprite-mcp/src/server.rs`.

- [ ] Tests: caja en el lugar con sus colores; colores por cara; sin contorno; indexado; bloques.
- [ ] Implementar. Commit.

### Task 7: diálogo Iso Box (modo Box) y colocación flotante

**Files:** create `crates/cs-ui/src/iso_dialog.rs` (+ `iso_dialog_tests.rs`); modify `lib.rs`,
`app_menus.rs`, `file_actions.rs` (`is_modal`), `transform.rs` (etiqueta del paso), `demo.rs`.

- [ ] Tests: Shade from sigue al base; tocar una cara lo apaga; Place deja flotando con Move y
      un paso "Iso Box"; centrado en la selección; indexado; harness de diálogo con y sin Pixel UI.
- [ ] Implementar. Commit.

### Task 8: modo Blocks del diálogo

- [ ] Tests: clic suma, clic derecho resta, cambiar columnas/filas conserva lo que entra; Place.
- [ ] Implementar. Commit.

### Task 9: verificación

- [ ] Capturas (`CHIMPSPRITE_DEMO_ISO=box|blocks|placed`, `CHIMPSPRITE_DEMO_ISO_GRID=1`) revisadas.
- [ ] `cargo test --workspace`, clippy, fmt, wasm check; autorrevisión del diff; rebase.
