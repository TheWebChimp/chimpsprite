--- Outline FX (docs/superpowers/plans/2026-11-06-outline-fx.md), branch outline-fx from main 84c4846 ---
Outline Task 1: complete (8f6031b..e995102). cs_raster::outline (OutlineMatrix con bits de Aseprite, presets, place, wrap) + Channels movido a cs-raster; 78 casos de Aseprite 1.3.18 (fixtures/outline_aseprite.{lua,txt}) iguales píxel a píxel.
Outline Task 2: complete (e995102..cfdeeae). Filter::Outline(OutlineFx), FilterTarget Selected/All, apply_filter_to en un paso (Batch); filtros leen el cel una vez.
Outline Task 3: complete (cfdeeae..441c9ff). Diálogo con kit (color_button, group, botones de la matriz, radios, checks, select), Preview y Apply para todos los filtros, valores recordados en la sesión, CHIMPSPRITE_DEMO_FILTER=outline. Captura revisada (.refs/tmp/font-outline-2.png): vista previa en el lienzo ok.
Outline Task 4: workspace tests, clippy -D warnings, fmt check y wasm check en verde. Minors: outline-minors.md.
--- FontForge parte 2: export/import (docs/superpowers/plans/2026-11-06-fontforge-export-import.md), rama fontforge-export desde origin/main e19a8e5 ---
FF2 Spec + plan: 9253d13 (spec docs/superpowers/specs/2026-11-06-fontforge-export-import-design.md, con "Decisiones que el usuario puede revisar"; formato de BitFontMaker2 leído del código de pentacom.jp).
FF2 Task 1: complete (bc52fbb). cs_core::build_font (FontBuild → FontInfo + Sprite) y cs_commands::font::add_font_artboard.
FF2 Task 2: complete (dac38e1). cs_io::font::outline: pixeles → contornos unidos (horario, huecos al revés, diagonales separadas, sin puntos intermedios).
FF2 Task 3: complete (c3b9083). TTF a mano (head/hhea/maxp/OS2/hmtx/cmap 4+12/loca/glyf/name/post + COLR/CPAL). 64 u/px, em = alto×64. Probado con ttf-parser, fontdue (= herramienta Texto) pixel por pixel y fontTools (recompila y lee todo).
FF2 Task 4: complete (4ab506f). BMFont .fnt texto + atlas en estantes (pot. de 2), 1 bit en blanco.
FF2 Task 5: complete (07ded54). Import TTF/OTF: tamaño sugerido por cuadrícula común, 4×4 muestras no cero, umbral, COLR v0, ida y vuelta 1 bit y color idéntica; Hack/Ubuntu reales a 16 px.
FF2 Task 6: complete (f7ac7b9). Import JSON de BitFontMaker2 (bits 1<<x, margen de 2 columnas, letterspace/64, monospace, ascender, wordspacing, valores texto o número).
FF2 Task 7+8: complete (ab3cf2c, 7b34f54). Build ▾ (menú TTF / Color TTF / BMFont) → diálogo Build Font; File → Import Font… / Export Font…; Library → Fonts → Import…; soltar .ttf importa; diálogo Import Font con vista previa. Demo: CHIMPSPRITE_DEMO_FONT_BUILD, CHIMPSPRITE_DEMO_FONT_IMPORT=<ruta>. Capturas revisadas: .refs/tmp/font-build-1.png, font-import-1.png (color), font-import-2.png (Consolas).
FF2 Task 9: complete. cargo test --workspace (0 fallas), clippy -D warnings, fmt --check y wasm check en verde. Minors: .superpowers/sdd/fontforge2-minors.md.
