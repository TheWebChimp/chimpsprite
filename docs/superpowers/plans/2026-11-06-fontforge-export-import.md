# Plan: FontForge parte 2 (exportar e importar)

Spec: `docs/superpowers/specs/2026-11-06-fontforge-export-import-design.md`. Rama
`fontforge-export`. TDD por tarea, un commit por tarea (tests + clippy + fmt en verde).

1. **`cs-core::font::build_font`** (`FontBuild` → `FontInfo` + `Sprite`) y
   **`cs-commands::font::add_font_artboard`** (`new_font_artboard` lo usa). Pruebas: orden,
   repetidos, recorte, 1 bit/color, glifos vacíos sin cel, el comando con undo.
2. **`cs-io::font::outline`**: pixeles → contornos (lados de frontera encadenados, sin puntos
   intermedios, horario afuera, huecos al revés, diagonales separadas).
3. **`cs-io::font::ttf`**: escritor de tablas (`head`, `hhea`, `maxp`, `OS/2`, `hmtx`, `cmap` 4/12,
   `loca`, `glyf`, `name`, `post`, `COLR`, `CPAL`). Pruebas con `ttf-parser` (glifos, cmap,
   avances, métricas, nombre, cajas, COLR) y con `fontdue` (dev-dependency) rasterizando al alto
   de la celda.
4. **`cs-io::font::bmfont`**: atlas en estantes y `.fnt` de texto.
5. **`cs-io::font::ttf_import`**: `ttf_info` (tamaño sugerido) e `import_ttf` (4×4 muestras, no
   cero, COLR v0, celda, medidas, avances). Ida y vuelta 1 bit y color.
6. **`cs-io::font::bitfontmaker`**: el JSON de BitFontMaker2.
7. **UI de export:** Build ▾ habilitado con su menú, diálogo Build Font, File → Export Font…,
   guardar (`platform.choose_save`/`write`; BMFont, dos archivos). Prueba de UI.
8. **UI de import:** `OpenPurpose::ImportFont`, File → Import Font…, Library → Fonts → Import…,
   diálogo Import Font con vista previa (TTF/OTF) y JSON directo; demo
   `CHIMPSPRITE_DEMO_FONT_BUILD` / `CHIMPSPRITE_DEMO_FONT_IMPORT`. Pruebas de UI.
9. **Verificación:** `cargo test --workspace`, clippy `-D warnings`, fmt, wasm check, capturas del
   diálogo Build y del Import; autorrevisión del diff; minors en
   `.superpowers/sdd/fontforge2-minors.md`; bitácora en `.superpowers/sdd/progress.md`.
