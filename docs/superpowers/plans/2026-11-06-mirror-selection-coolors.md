# Plan: selección espejo completa y paletas de Coolors

Spec: `docs/superpowers/specs/2026-11-06-mirror-selection-coolors-design.md`. Rama
`mirror-selection`. TDD por tarea; un commit por tarea con tests + clippy en verde.

## Tarea 1 — Plan del espejo (`cs-raster/src/mirror.rs`)

- `MirrorOp` público (envuelve la matriz de `symmetry.rs`): `apply`, `inverse`, `linear`,
  `is_identity`; `mirror_ops(axes)`.
- `mirror_plan(mask, axes, center2, anchor2)`: invariancia y biyección, componentes 8-conexas,
  órbitas, representante más cercana al ancla, estabilizador; `MirrorPlan { primary, copies,
  fixed }`, `FixedPiece { points, stabilizer, copies, straight_x, straight_y, diagonal }`.
- `project(stabilizer, d)`.
- Tests: un eje con dos piezas, el lado del ancla, pieza que cruza, píxel sobre el eje, dos ejes
  (boca abajo con │ ─), diagonales, selección no simétrica → `None`, ocho copias.

## Tarea 2 — Lo flotante en espejo (`cs-ui/src/transform/mirror.rs`, `transform.rs`)

- `MirrorFloat` armado al levantar desde el plan; `Floating` usa lo primario como `source`.
- Render: lo primario con la caja, cada copia conjugada, las piezas fijas con su caja simétrica o
  su traslado proyectado; composición (lo primario al final, lo transparente no tapa).
- Hueco, `delete_floating`, `transform_box` sin levantar (caché del plan), ancla
  (`mirror_anchor`) desde los gestos de selección y al agarrar; arrastre sobre un reflejo.
- Tests (editor): nudge con │ (el de antes), escala en espejo, Flip en espejo, pieza que cruza
  solo se mueve en vertical, diagonal ╲, │ ─ con boca, agarrar el reflejo, un solo undo, Delete.

## Tarea 3 — Parser de Coolors (`cs-io/src/coolors.rs`)

- `parse_palette_text`, `PastedPalette { name, palette, source, from_link }`.
- Tests: los dos links, sin esquema, query, hex con/sin `#`, separadores, 3/8 dígitos, CSS de
  Coolors, texto que no es paleta, límite de 256, nombre.

## Tarea 4 — Integración (pestaña Lospec y Ctrl+V)

- `LospecPalette.source` (página fuera de Lospec) y `comments` con esa fuente.
- `LospecTab::start_lookup`: si el texto es una paleta pegada, el resultado sin red.
- Ctrl+V con paleta en el portapapeles: Palette Presets en Lospec con la paleta.
- Tests: `lospec_tab` (pegar link y hex), app (Ctrl+V abre la ventana con la fila).

## Tarea 5 — Demo, capturas y verificación final

- Demo `CHIMPSPRITE_DEMO_MIRROR` (cara con ojos, │ y Mirror prendidos, ojos seleccionados y
  corridos) y `CHIMPSPRITE_DEMO_COOLORS=<texto>` (ventana con la paleta pegada).
- Capturas revisadas; test workspace, clippy, fmt, wasm; autorrevisión del diff.
