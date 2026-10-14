# M4 — Capas, timeline y animación — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Convertir ChimpSprite en editor de animación: panel de capas (con grupos), timeline de frames con cels, duración por frame, tags, reproducción, onion skin, cels vinculados y modos de mezcla.

**Architecture:** Modelo y reglas en `cs-core` (tags, árbol de capas, lógica de reproducción pura). Comandos nuevos en `cs-commands`, todos con inverso; las operaciones compuestas (duplicar frame, vincular/desvincular cels) son helpers que arman `Batch`. `cs-render` agrega los modos de mezcla y el onion skin. La UI agrega un panel inferior de timeline.

Spec: secciones "Alcance", "Modelo de datos" y "UI y render" de `docs/superpowers/specs/2026-10-14-chimpsprite-core-design.md`.

## Global Constraints

- Las de M1–M3.
- Invariante del árbol de capas: en `Sprite::layers` (abajo → arriba) los descendientes de un grupo van contiguos inmediatamente después del grupo (mismo orden que el formato `.aseprite`). Todo comando que mueva capas valida el invariante y falla sin cambios si se rompe.
- Los tags no se mueven al reordenar frames (como en Aseprite); al insertar/quitar frames sus rangos se ajustan, y el inverso restaura los tags exactos.
- Atajos (estilo Aseprite): `Enter` reproducir/pausar, `,` / `.` y `←` / `→` frame anterior/siguiente, `↑` / `↓` capa de arriba/abajo, `Alt+N` nuevo frame (duplica el actual), `Shift+N` nueva capa, `F3` onion skin.

---

### Task 1: Modelo (`cs-core`)

**Produces:** `Tag { name, from, to, direction: AnimDirection, color: Rgba }`, `AnimDirection::{Forward, Reverse, PingPong}`, `Sprite::tags: Vec<Tag>`; helpers de árbol: `Sprite::{subtree_range(id) -> Option<Range<usize>>, depth(id) -> usize, is_visible(id) -> bool, is_valid_tree() -> bool}`; reproducción pura: `Playback { frame, forward }` + `Playback::step(&mut self, sprite, range: (usize, usize), direction)` y `Sprite::tag_at(frame) -> Option<&Tag>`.

**Tests:** `subtree_range` de un grupo con hijos; `is_valid_tree` detecta hijos separados del grupo; `step` en Forward/Reverse/PingPong (incluye rangos de 1 frame).

### Task 2: Comandos (`cs-commands`)

**Produces:** `SetTags(Vec<Tag>)`, `MoveLayer { id, index, parent }` (mueve el bloque del subárbol; `index` en la lista sin el bloque), `MoveFrame { from, to }`, `SetFrameDuration { index, duration_ms }`, `LayerProps.blend`. `AddFrame`/`RemoveFrame` ajustan tags. Helpers en `ops.rs`: `duplicate_frame(sprite, index, linked) -> Command`, `link_cel(sprite, layer, frame, source_frame) -> Command`, `unlink_cel(sprite, layer, frame) -> Option<Command>`, `new_layer(sprite, above, kind) -> Command`. Error `InvalidLayerTree`.

**Tests:** round-trip de cada comando; mover un grupo mueve sus hijos; meter una capa dentro de un grupo; mover un grupo dentro de sí mismo falla; tags se ajustan con Add/RemoveFrame y vuelven exactos con undo; duplicar frame vinculado comparte imagen y sin vincular la copia; property test ampliado.

### Task 3: Render (`cs-render`)

**Produces:** los 9 `BlendMode` (Other → Normal); `render_onion(sprite, frame, &OnionSkin) -> Vec<Rgba>` con `OnionSkin { before, after, opacity, tint }` (frames anteriores teñidos de rojo, siguientes de azul, opacidad decreciente, debajo del frame actual).

**Tests:** cada modo de mezcla contra valores calculados a mano; onion con before=1 muestra el frame anterior teñido donde el actual es transparente.

### Task 4: UI

- Editor: `playing`, `tick(dt)`, selección de rango de frames, acciones de capas/frames/cels/tags, grupos colapsados, onion skin.
- Panel inferior Timeline: fila de controles (play, primero/anterior/siguiente/último, + frame, − frame, + capa, + grupo, − capa, onion), opacidad y modo de mezcla de la capa activa; grilla capas × frames (ojo, candado, nombre con sangría por grupo, cels ●/○, vínculo entre cels vinculados), encabezado de frames con número y duración, tags como barras de color arriba; clic selecciona capa+frame; Shift+clic en encabezados selecciona rango; arrastrar filas de capas y encabezados de frames reordena; menús contextuales (duración, nuevo tag, propiedades de tag, vincular/desvincular/borrar cel, renombrar capa).
- Atajos de Global Constraints. Los artboards no activos muestran su frame recordado.

**Verificación:** fmt, clippy, tests, captura con demo animado, app abierta para el usuario, PR con CI verde.
