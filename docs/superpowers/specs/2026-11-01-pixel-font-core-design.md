# Editor de fuentes pixel, parte 1: el núcleo

Fecha: 2026-11-01 · Estado: implementado

## Objetivo

Reemplazar BitFontMaker2 dentro de ChimpSprite: crear fuentes pixel, editarlas con las
herramientas de siempre y usarlas en la herramienta Texto, en TTF y en juegos. La referencia de
interfaz es BitFontMaker2: el editor de un glifo grande con la letra fantasma, el mapa de
caracteres y el "Test Drive".

El proyecto completo va en cuatro partes. Cada una tiene su spec, su plan y queda usable:

1. **Núcleo** (este documento): el modelo de la fuente, el artboard de fuente, el panel FontForge y la
   herramienta Texto con fuentes del documento.
2. **Entrada y salida:** exportar TTF (1 bit, y de color COLR con la silueta de respaldo) y
   BMFont (`.png` + `.fnt`); importar TTF (incluidas las de BitFontMaker2, reconociendo su
   cuadrícula de 64 unidades por pixel) y el JSON de BitFontMaker2.
3. **Especiales A:** glifos compuestos (`á` = `a` + `´`, que se actualizan solos) y generar los
   glifos desde otra fuente o desde una imagen de letras.
4. **Especiales B:** variantes automáticas (bold, itálica, contorno, sombra; el contorno reusa el
   filtro Outline) y kerning por pares desde el Test Drive.

Decisiones tomadas con el usuario:

- **La fuente es un artboard** con información de fuente, y su sprite tiene **un frame por
  glifo**. Así sirven sin cambios las herramientas, el deshacer, copiar y pegar cels, la
  selección, el onion skin y el `.chimp`.
- **Dos modos:** 1 bit (prendido o apagado, como BitFontMaker2) o color (glifos con sus colores).
- **Medidas:** una celda fija para toda la fuente, con baseline, mayúsculas, x-height y
  descendentes, y un ancho (avance) por glifo, automático o manual, con un switch Monospace.
- **Interfaz:** el glifo actual en el lienzo (el artboard de fuente) y un panel **Font** en el
  dock con el mapa, el Test Drive y los ajustes.

Fuera de alcance de esta parte: exportar e importar, compuestos, generar desde otra fuente,
variantes, kerning y comandos MCP para fuentes.

## 1. El modelo (`cs-core`)

`Artboard` gana `font: Option<FontInfo>` (con `serde(default)`: los `.chimp` viejos abren igual).

```rust
pub struct FontInfo {
    pub name: String,
    pub mode: FontMode,          // OneBit | Color; no cambia después de crear la fuente
    pub metrics: FontMetrics,
    pub letter_spacing: u8,      // columnas entre letras, por defecto 1
    pub space_width: u8,         // ancho del espacio (U+0020), por defecto 3
    pub monospace: bool,         // todos los glifos con el ancho de la celda
    pub ghost: GhostSettings,    // la letra fantasma: { enabled, monospace }
    pub glyphs: Vec<GlyphInfo>,  // alineada con `sprite.frames`
}

pub struct FontMetrics {         // filas desde arriba de la celda
    pub baseline: u32,
    pub cap_height: u32,         // filas de las mayúsculas sobre la baseline
    pub x_height: u32,
    pub descent: u32,            // filas debajo de la baseline
}

pub struct GlyphInfo {
    pub ch: char,
    pub advance: Option<u32>,    // en columnas; `None` = automático
}
```

- **La celda** es el tamaño del sprite (por defecto 16×16). Cada glifo es el cel de la única capa
  en su frame.
- **1 bit:** sprite indexado con una paleta de dos entradas (transparente y la tinta, que es solo
  para verlo). **Color:** sprite RGBA.
- **Orden:** los frames van en el orden de los códigos Unicode, y no hay dos glifos con el mismo
  carácter.
- **Ancho automático (`advance: None`):** se calcula al usarlo, no se guarda: la última columna con pixeles más 1, más `letter_spacing`. Un glifo vacío
  usa la mitad de la celda. Con `monospace`, todos usan el ancho de la celda. El espacio (U+0020)
  no es un glifo: usa `space_width`.

Funciones del modelo (puras, con pruebas): el glifo de un carácter, el ancho efectivo de un glifo
(manual, o el automático calculado en ese momento) y el ancho de un texto.

## 2. Comandos (`cs-commands`)

Cada cambio de la fuente que no es pintar es un comando con su undo, como los de frames:

- `NewFont { name, cell: (w, h), mode }`: crea el artboard de fuente con los 94 caracteres ASCII
  imprimibles (`!` a `~`), vacíos, y medidas iniciales proporcionales a la celda (baseline en
  3/4 del alto, mayúsculas en 5/8, x-height en 3/8, descendentes hasta abajo).
- `AddGlyphs { artboard, chars }` y `RemoveGlyph { artboard, ch }`: agregan o quitan frames en su
  lugar ordenado.
- `SetGlyphAdvance { artboard, ch, advance: Option<u32> }`: `None` regresa a automático.
- `SetFontInfo { artboard, info }`: nombre, medidas, espaciado, Monospace y letra fantasma.
- `ShiftGlyph`, `CopyGlyph`/`PasteGlyph` y `ClearGlyph` usan los comandos de cels que ya hay.

Pintar un glifo es editar un cel, como ya existe. Los anchos automáticos no se recalculan después
de cada comando: se calculan al usarlos (lienzo, mapa, Test Drive, herramienta Texto).

## 3. El artboard de fuente en el lienzo (`cs-ui`)

Es la ventana de siempre de los artboards, con estas diferencias:

- **Título:** el carácter actual y el nombre, "A · MiFuente". El menú ≡ agrega *Shift ← → ↑ ↓*,
  *Copy glyph*, *Paste glyph*, *Clear glyph* y *Glyph from…* (deshabilitado hasta la parte 3).
- **Letra fantasma:** detrás de los pixeles, el carácter actual en gris claro, rasterizado con
  una fuente de egui según `GhostSettings.monospace`: Ubuntu (proporcional, por defecto) o Hack
  (monoespaciada), no las de la herramienta Texto, a la altura de las mayúsculas, apoyado
  en la baseline y desde el borde izquierdo. No se guarda ni se exporta. Se apaga desde el panel.
- **Guías** (dibujadas por el lienzo, no son pixeles):
  - la baseline, en rojo fuerte;
  - las mayúsculas, la x-height y los descendentes, punteadas y tenues;
  - el borde izquierdo y la línea del ancho, en rojo (como en BitFontMaker2).
- **El ancho se arrastra:** arrastrar la línea del ancho lo cambia por columnas enteras y deja el
  glifo en ancho manual (un paso de undo al soltar).
- **Cambiar de glifo** es cambiar el frame activo: desde el mapa o con las flechas del teclado
  (izquierda y derecha, como los frames).
- **El onion skin** que ya existe muestra el glifo anterior o el siguiente.
- **El timeline no se muestra** con un artboard de fuente activo: el panel FontForge hace sus veces.
- **1 bit:** el lápiz y las demás herramientas pintan con la tinta, sin importar el color
  elegido, y el clic derecho borra. No hay forma de meter otro color en una fuente de 1 bit.

## 4. El panel FontForge (dock)

Un ícono nuevo en el dock (`icon.dock.font`, el yunque), como Lumen o Groom, con el nombre "FontForge". Con un artboard de fuente activo muestra sus tres
secciones; si no, un botón *New Font…*. Todo con los componentes del UI kit.

- **Map:**
  - Cuadrícula de celdas con el carácter arriba y la miniatura del glifo abajo (en blanco si está
    vacío); la del glifo actual, marcada.
  - Pestañas de páginas: *ASCII*, *Latin-1*, *Latin Ext-A* y *All* (los caracteres que tiene la
    fuente). En las páginas, los caracteres que la fuente no tiene salen apagados.
  - Clic: ir a ese glifo. Doble clic en uno que no existe: agregarlo.
  - Un campo *Add char* agrega lo que se escriba (cualquier carácter, también símbolos o emojis).
  - Clic derecho en un glifo: *Copy*, *Paste*, *Clear*, *Remove*.
- **Test Drive:**
  - Un campo de texto y, debajo, el texto con la fuente a ×2 o ×3, con los anchos y el
    espaciado reales.
  - Un carácter que falta sale como un cuadrito tachado.
  - Clic sobre una letra de la vista: ir a ese glifo.
- **Font settings:**
  - Nombre; modo (solo se muestra); celda (solo se muestra).
  - Baseline, mayúsculas, x-height y descendentes con steppers; las guías del lienzo se mueven al
    cambiarlos.
  - Espacio entre letras, ancho del espacio y Monospace.
  - Letra fantasma: on/off y Monospace (Ubuntu o Hack, las fuentes de egui).
  - *Build ▾* deshabilitado, con el tooltip "Coming soon" (parte 2).

**New Font…** (también en *File → New Font…*) abre un diálogo del kit con el nombre, la celda
(ancho y alto, 16×16 por defecto) y el modo. Crea el artboard de fuente al lado de los demás,
lo activa y abre el panel; la vista encuadra el artboard nuevo al lado del panel FontForge
(`ZoomRequest::FitActiveBeside`). *New Font…* en Home abre un documento nuevo con la fuente.

## 5. La herramienta Texto con fuentes del documento (`cs-tools::text`)

- `FontChoice` gana `Document { artboard: ArtboardId, name: String }`. El selector de fuente de la
  barra de opciones agrega una sección con las fuentes del documento abierto.
- **El trazado:** cada carácter se copia pixel por pixel desde su glifo y la pluma avanza su
  ancho. La baseline de la fuente es la línea de escritura y el espacio avanza `space_width`.
- **1 bit:** la tinta es el color primario. **Color:** el glifo con sus colores.
- **Un carácter que falta:** un cuadrito tachado del alto de las mayúsculas.
- **Tamaño:** una escala entera (×1, ×2, ×3…), como hoy la Pixel 5×7.
- El texto se pega como pixeles: si después se edita la fuente, lo escrito no cambia.

`cs-tools` no conoce los artboards: la UI le pasa al trazado los glifos y las medidas de la fuente
(una vista de solo lectura que arma `cs-core`).

## 6. Pruebas

- **Modelo:** crear la fuente (94 glifos, medidas iniciales); agregar y quitar glifos en orden;
  anchos automáticos al usarlos (glifo vacío, Monospace, espaciado); el ancho de un texto; el `.chimp` con
  una fuente se guarda y se lee igual, y uno viejo abre sin fuente.
- **Comandos:** cada uno con su undo y redo.
- **Texto:** trazado con anchos, espaciado, espacio, carácter que falta, 1 bit con el color
  primario y color con sus colores, y escala.
- **UI:** las guías en sus filas; arrastrar la línea del ancho; el mapa (clic, doble clic para
  agregar, páginas); el Test Drive; en 1 bit, el lápiz pinta con la tinta.
- **Capturas:** `CHIMPSPRITE_DEMO_FONT` abre una fuente de ejemplo (unas letras dibujadas) con el
  panel Font abierto.
