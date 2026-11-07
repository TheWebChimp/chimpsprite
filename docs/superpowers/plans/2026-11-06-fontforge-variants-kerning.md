# Plan: FontForge parte 4 — variantes y kerning

Spec: `docs/superpowers/specs/2026-11-06-fontforge-variants-kerning-design.md`. Rama
`fontforge-variants`. TDD por tarea, un commit por tarea (tests + clippy en verde).

1. **Modelo de kerning** (`cs-core::font_kerning`): `KernSide`, `ClassSide`, `KernClass`,
   `KernPair`, `FontKerning` (precedencia, `set`/`remove`, `flatten`, `parse_side`);
   `FontInfo.kerning`, `PixelFontView.kerning`, `text_width` con kerning. `FontBuild.kerning`.
2. **Texto con kerning** (`cs-tools::text`): `font_layout` + `font_text` con el par; los paneles
   (Test Drive, vista previa de Import) usan `font_layout` para las letras.
3. **Modelo de estilos** (`cs-core::font_style`): `FontStyle`, `StyleSource`, `VariantKind`,
   `VariantRecipe`, `full_name`, `family_from_name`, `legacy_names`; `FontInfo.style`,
   `FontBuild.style`. El nombre del artboard = `full_name` (set_font_named, rename, duplicate,
   título, herramienta Texto).
4. **Generar variantes** (`cs-raster::font_style`): Bold, Italic, Bold Italic, Outline, Shadow,
   Copy; celda, anchos, compuestos. Comandos `set_kerning` y `replace_font`.
5. **TTF**: nombres/OS2/head/post/hhea del estilo; tablas `kern` y `GPOS`.
6. **BMFont**: `bold`/`italic` y `kerning`.
7. **Importar**: kerning (GPOS + kern) y estilo desde un TTF; ida y vuelta.
8. **Editor** (`font_ops`): kerning (par elegido, set con grupo), familia (`family_fonts`),
   `add_variant`, `rebuild_variant`, archivos de la familia en `font_io`.
9. **Panel: vista Kerning** (ícono, papel con pares, lista de pares y clases).
10. **Panel: estilo y familia** en los ajustes; diálogo **New Variant…**; Build Font con la
    familia.
11. **Demo y capturas**, autorrevisión del diff, verificación completa (test, clippy, fmt, wasm),
    rebase sobre `origin/main`.
