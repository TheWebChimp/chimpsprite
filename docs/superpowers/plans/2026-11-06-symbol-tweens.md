# Plan: tweens de objetos (movie clips parte 3)

Spec: `docs/superpowers/specs/2026-11-06-symbol-tweens-design.md`. Rama `movie-clips-3`.
TDD por tarea; un commit por tarea con tests + clippy en verde.

## Tarea 1 — Modelo y pose (`cs-core`)

- `Curve::value(f)` en `cs_core::model` (la fórmula de `cs_3d::anim::curve_value`, que la llama).
- `InstanceKey`: `scale`, `rotation`, `tint`, `tween` con `serde(default, skip_serializing_if)`.
  `Tint`, `Tween`, `Ease` (`Ease::apply(t)`).
- `InstancePose`, `InstanceTrack::{pose_at, keyframe_at, tween_span}`, `TweenSpan`,
  `InstanceKey::is_plain`, `InstancePose::is_plain`.
- Ajustar los literales de `InstanceKey` en otros crates (`cs-commands`, `cs-io`, `cs-render`).
- Tests: eases, pose (cada propiedad, redondeo, roto, frame final), keyframe_at, JSON viejo/nuevo.

## Tarea 2 — Render transformado (`cs-render`)

- Dependencia `cs-raster`.
- `instance_image` con la pose: flips → `scale_nearest` → `rotsprite` → centrado → tinte → recorte;
  la opacidad del cel sale de la pose.
- Tests: identidad, 200 %, 50 %, 90°, 45° sin colores nuevos, tinte, tween a mitad (render y
  `object_image`).

## Tarea 3 — Operaciones (`cs-commands`)

- `update_objects_key` y `shift_objects` con `keyframe_at`.
- `create_tweens`, `remove_tweens`, `update_tweens`, `can_create_tween`, `has_tween`.
- Tests con undo; mover a mitad del tween.

## Tarea 4 — Timeline (`cs-ui`)

- `Editor::{row_can_create_tween, row_has_tween, create_tween, remove_tween}` (+ por objeto).
- `object_keys`: fondo y flecha del tramo; punteada si está roto.
- Menús de la fila y la sub-fila.
- Tests: menú (un undo), dibujo del tramo.

## Tarea 5 — Inspector, Edit in Place y Move (`cs-ui`)

- `instance_key()` devuelve `keyframe_at` (la pose); Scale, Rotation, Tint; bloque Tween (Ease,
  Amount, Create/Remove).
- `enter_symbol_in_place`/`in_place_position` con la pose y `is_plain`.
- `object_at` con la opacidad de la pose.
- Tests.

## Tarea 6 — MCP (`cs-agent`, `chimpsprite-mcp`)

- `KeyChange` con escala, rotación y tinte; `set_tween`, `remove_tween` (dispatch, server).
- Tests.

## Tarea 7 — Demo, capturas y verificación

- `CHIMPSPRITE_DEMO_SYMBOL=tween`: el sombrero con un tween (mueve, escala, gira), la capa
  desplegada y el frame del medio.
- Capturas; verificación completa (tests, clippy, fmt, wasm); autorrevisión del diff; minors.
