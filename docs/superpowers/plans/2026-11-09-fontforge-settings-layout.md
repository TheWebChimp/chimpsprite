# Plan: la vista de ajustes de FontForge como el mockup

Spec: `docs/superpowers/specs/2026-11-09-fontforge-settings-layout-design.md`. Rama
`fontforge-settings-layout`. TDD: cada tarea empieza por sus tests, termina con tests + clippy en
verde y un commit.

## Tarea 1: el logo nuevo
- Recortar el logo del mockup con PIL (fondo y línea del título → transparentes) y ponerlo en
  `assets/ui/skin.aseprite` con Lua (MCP de Aseprite): la slice `fontforge.logo` pasa a (4, 560)
  141×28 y se borra la pieza vieja.
- `font_panel::header`: el logo en (6, 2) del panel, pintado después de la línea del título.
- Test: `the_font_panel_keeps_its_logo_and_build_while_the_middle_scrolls` con (6, 2) y 141×28.

## Tarea 2: la vista previa de métricas (`font_metrics_preview.rs`)
- `Metric` (las seis filas), `preview_char` (glifo actual o letra de muestra), `layout` (escala
  entera, celda centrada, líneas como rects con color, punteadas o no, y la resaltada) y `paint`.
- Tests de la lógica: escala y centrado, filas de las líneas, resaltado, Letter spacing y Space
  width solo al resaltarse, la letra de muestra.

## Tarea 3: la vista de ajustes con la geometría del mockup
- `settings` y `family_section` con posiciones fijas desde la esquina del contenido (constantes
  comentadas con su origen en el mockup); el hover de las filas de los steppers alimenta la vista
  previa.
- Tests de layout: posiciones de campos, casillas, steppers, recuadro, familia y New Variant; que
  la vista previa resalte la línea con el puntero encima. Ajustar los tests existentes (dos
  columnas → una).

## Tarea 4: demo y capturas
- `CHIMPSPRITE_DEMO_FONT_CONFIG` en `demo.rs`: fuente nueva de 16×16, un estilo, vista Config.
- Captura a escala 2, recortes lado a lado con el mockup; corregir diferencias.

## Tarea 5: verificación final
- `cargo test --workspace`, clippy `-D warnings`, `fmt --check`, check de wasm; autorrevisión del
  diff; `git fetch` + rebase si main avanzó.
