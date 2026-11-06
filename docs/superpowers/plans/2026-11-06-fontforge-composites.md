# Plan: FontForge parte 3 — compuestos y generar glifos

Spec: `docs/superpowers/specs/2026-11-06-fontforge-composites-design.md`. Rama
`fontforge-composites`. TDD por tarea, un commit por tarea (tests + clippy en verde).

1. **Modelo de compuestos** (`cs-core::font`): `GlyphComponent`, `GlyphInfo.components`
   (deja de ser `Copy`), `FontInfo::composed`, `depends_on`, ancho automático y `font_view` con lo
   que se ve. Pruebas del spec §6 (modelo).
2. **Tabla NFD y colocación** (`cs-core::font::accents`): `DECOMPOSITIONS`, fuentes de marca,
   `plan_accent(font, sprite, ch) -> Result<AccentPlan, AccentSkip>`.
3. **Comandos** (`cs-commands::font`): `set_components`, `unlink_glyph`, `remove_glyph` con
   dependientes, `auto_build_accents`, `fill_glyphs`.
4. **IO** (`cs-io::font`): `sheet::glyphs_from_sheet` + defaults; `ttf_import::fill_from_ttf`
   (alinear la baseline).
5. **Editor** (`cs-ui::editor::font_ops`): componente elegido, `set_glyph_components`,
   `nudge_component`, `unlink_current_glyph`, `auto_build_accents`, `fill_glyphs`.
6. **Lienzo:** fantasma de componentes, arrastre con Move, flechas.
7. **Panel:** sección Glyph, Generate ▾, marquita en el mapa, menú de celda.
8. **Diálogos:** Fill from TTF (Import Font en modo rellenar) y Glyphs from image; aviso de
   Auto-build.
9. **Demo y capturas**, autorrevisión del diff, verificación completa (test, clippy, fmt, wasm).
