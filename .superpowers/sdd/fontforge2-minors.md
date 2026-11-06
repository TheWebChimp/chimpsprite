# FontForge parte 2 (export/import) — pendientes y minors

Rama `fontforge-export`. Spec: docs/superpowers/specs/2026-11-06-fontforge-export-import-design.md.

## Fuera de alcance (anotado)
- COLR v1 (degradados) al importar: solo capas de color sólido (v0). Los degradados se ignoran.
- Importar fuentes bitmap embebidas (EBDT/CBDT/sbix) y fuentes `.ttc` (solo la primera cara).
- Exportar el JSON de BitFontMaker2 (solo se importa).
- Hinting/instrucciones en el TTF (no hacen falta: los contornos caen en la cuadrícula) y
  entradas de `name` para la plataforma Mac.
- Kerning (parte 4): el BMFont sale con `kernings count=0`.
- BMFont binario o XML: solo el `.fnt` de texto.

## Minors
- `cmap` formato 4 con más de ~8000 tramos no contiguos desborda su longitud u16 (no pasa con
  4096 glifos importados, pero sí si alguien agrega miles de caracteres sueltos a mano): falta
  un error claro.
- U+FFFF como glifo no entra al cmap (es un no-carácter).
- Import: lo que se sale a la izquierda corre toda la fuente a la derecha y suma ese corrimiento
  a los avances (fuentes itálicas con bearing negativo quedan con una columna de aire).
- Import: `All` corta en 4096 caracteres, en orden de código (sin elegir rangos a mano).
- Import Font: la vista previa usa un texto fijo ("AaBbGg 0123"); no se puede escribir otro.
- BitFontMaker2: `basefont*`, `copy`, `descender` y `linegap` se leen pero no se usan.
- Build Font en la web: el BMFont son dos descargas seguidas (el navegador puede preguntar).
- File → Export Font… solo se habilita dentro de la pestaña de una fuente; desde el documento
  hay que abrirla primero (Library → Fonts → doble clic).
- Leftovers de la parte 1 que siguen pendientes (no eran de export/import): edge-resize del
  artboard tool en fuentes, `set_transparent_index` en 1 bit, textos de lock, hover de Delete
  Frame, reorder en SetSprite/SetDocument, arrastre del avance al cambiar de artboard, caché de
  `font_view`, ghost no cuadrado.
