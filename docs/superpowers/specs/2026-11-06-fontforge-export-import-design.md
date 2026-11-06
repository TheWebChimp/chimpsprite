# FontForge, parte 2: exportar e importar fuentes pixel

Fecha: 2026-11-06 · Estado: diseño

Sigue al núcleo (`2026-11-01-pixel-font-core-design.md`) y a las pestañas
(`2026-11-02-cad-font-tabs-design.md`). Una fuente es un artboard con `font` (Library → Fonts,
su propia pestaña), con un frame por glifo del tamaño de la celda.

## Objetivo

- **Exportar** una fuente pixel a **TTF** (contornos de pixeles), a **TTF de color** (COLR v0 +
  CPAL, con la silueta como respaldo) y a **BMFont** (`.fnt` de texto + atlas `.png`).
- **Importar** un **TTF/OTF** (rasterizado a la celda, con tamaño en px y umbral; las fuentes
  pixel que vienen en TTF se reconocen solas) y el **JSON de BitFontMaker2**.
- **UI:** el botón *Build ▾* del panel FontForge se habilita y abre el menú de export; *File →
  Import Font…* / *Export Font…* y *Import…* en Library → Fonts.

Todo en Rust puro y compatible con wasm: el TTF se escribe a mano en `cs-io` (sin crates nuevos:
son 11 tablas sencillas) y se lee con `ttf-parser` (ya estaba en el árbol por `fontdue`).

## 1. El modelo de intercambio (`cs-core::font`)

`build_font(FontBuild) -> (FontInfo, Sprite)`: arma el sprite de una fuente desde pixeles (lo que
devuelve cualquier importador). `FontBuild` lleva nombre, modo, celda, medidas, espaciado, ancho
del espacio, Monospace y la lista de glifos `(char, advance: Option<u32>, pixels)` (los pixeles
como en `GlyphPixels`: `None` = tinta de 1 bit; en color, su `Rgba`). Ordena, quita repetidos,
recorta lo que cae fuera de la celda y no crea cel para los glifos vacíos.

`cs-commands::font::add_font_artboard(doc, font, sprite, position)` agrega ese artboard (lo usa
también `new_font_artboard`).

## 2. TTF (`cs-io::font::ttf`)

- **Unidades:** 1 pixel = **64 unidades** (la cuadrícula de BitFontMaker2) y
  `unitsPerEm = alto de la celda × 64`: a `alto` px la fuente sale pixel por pixel. Si la celda
  es tan alta que pasaría de 16384, se bajan las unidades por pixel.
- **Métricas:** la línea completa es la celda: `ascender = baseline × 64`,
  `descender = −(alto − baseline) × 64`, `lineGap = 0`; las mismas en OS/2 (typo y win, con
  `USE_TYPO_METRICS`). `sCapHeight` y `sxHeight` de las medidas de la fuente.
- **Glifos:** `.notdef` (un cuadrito hueco del alto de las mayúsculas, con el ancho de la
  celda), el espacio (avance `space_width`, sin contorno) y uno por glifo de la fuente con su
  avance efectivo (manual, automático o Monospace). El origen es el borde izquierdo de la celda.
- **Contornos:** cada pixel es un cuadrado, pero los pixeles vecinos (por lado) se unen: se
  juntan los lados de frontera, se encadenan en lazos y se quitan los puntos intermedios de los
  lados rectos. Los contornos de afuera van en sentido horario y los huecos (de verdad) al revés.
  Dos pixeles que solo se tocan por la esquina quedan como dos contornos. Solo puntos sobre la
  curva, coordenadas con las banderas cortas de TrueType.
- **Tablas:** `head`, `hhea`, `maxp` (1.0), `OS/2` (v4), `hmtx`, `cmap` (formato 4 para el BMP y
  12 si hay caracteres fuera de él), `loca` (larga), `glyf`, `name` (familia, subfamilia
  "Regular", nombre completo, versión, nombre PostScript sin espacios, ID único), `post` (3.0,
  `isFixedPitch` con Monospace), con checksums y `checkSumAdjustment`.
- **Color (COLR v0 + CPAL v0):** el glifo base guarda la silueta (todos sus pixeles) para los
  programas sin color; cada color del glifo es un glifo capa sin código Unicode, en el orden del
  primer pixel de cada color (de arriba a abajo). La paleta CPAL tiene los colores distintos de
  toda la fuente, con su alfa. Solo para fuentes de color.

## 3. BMFont (`cs-io::font::bmfont`)

- **Atlas:** cada glifo recortado a sus pixeles, con 1 pixel de separación, en estantes
  (ordenados por alto) dentro de un ancho de potencia de 2 (el menor que deja el atlas más o
  menos cuadrado); el alto, el que haga falta (también potencia de 2).
- **1 bit:** la tinta sale **blanca** (para teñirla en el motor); color: sus colores.
- **`.fnt` de texto:** `info` (face, size = alto de la celda, unicode=1, spacing=1,1),
  `common` (lineHeight = alto de la celda, base = baseline, scaleW/H, pages=1), `page id=0
  file="<nombre>.png"`, `chars` con el espacio y cada glifo (`xoffset`/`yoffset` desde la
  esquina de la celda, `xadvance` = avance efectivo), `kernings count=0`.

## 4. Importar TTF/OTF (`cs-io::font::ttf_import`)

- **Info previa** (`ttf_info`): nombre de la familia, unitsPerEm, si tiene COLR, cuántos
  caracteres, y el **tamaño sugerido**: si todas las coordenadas (puntos, avances, ascender y
  descender) caen en una cuadrícula común de `g` unidades, la fuente es pixel y el tamaño es
  `unitsPerEm / g` (los TTF de BitFontMaker2 y los nuestros dan su celda exacta); si no, 16 px.
- **Opciones:** tamaño en px (4–256), umbral de cobertura (1–100 %, 50 por defecto),
  caracteres (ASCII, Latin = hasta U+017F, o All; con All se toman a lo más 4096) y modo (1 bit
  o Color; Color solo si tiene COLR).
- **Rasterizado:** escala `px / unitsPerEm`, baseline en `round(ascender × escala)`; cada pixel
  con 4×4 muestras y la regla de devanado no cero de TrueType (por renglón de muestras); un pixel
  se prende si su cobertura llega al umbral. En color, las capas COLR se pintan en orden, cada una
  con su color sólido (los degradados de COLR v1 quedan fuera); los glifos sin color, con negro.
- **Celda:** alto = `ascender − descender`; ancho = el avance mayor (incluido `.notdef`, que en
  los nuestros guarda el ancho de la celda) o lo dibujado; si algo se sale por arriba, abajo o la
  izquierda, la celda crece y se recorre. Tope de 256×256.
- **Medidas:** cap height y x-height de OS/2 (si no, de lo dibujado en `H` y `x`); descent hasta
  abajo. Espaciado 1; el espacio, del avance de U+0020; cada avance queda automático si coincide
  con el automático y manual si no; si todos los glifos avanzan la celda, Monospace.
- **Ida y vuelta:** exportar y volver a importar da los mismos pixeles, los mismos avances
  efectivos, la misma celda y la misma baseline (pruebas en 1 bit y en color).

## 5. Importar el JSON de BitFontMaker2 (`cs-io::font::bitfontmaker`)

El formato real (leído del código de la página, `savetofile`/`loadData`):

```json
{"65":[0,0,…16 enteros…], "name":"MyFont", "copy":"", "letterspace":"64",
 "basefont_size":"512", "basefont_left":"62", "basefont_top":"0", "basefont":"Arial",
 "monospace":true, "monospacewidth":"8", "ascender":"12", "descender":"4", "linegap":"1",
 "wordspacing":"5"}
```

- Claves numéricas (> 30) = código Unicode en decimal; valor = 16 filas de arriba a abajo; el bit
  `1 << x` es la columna `x` desde la izquierda. Cuadrícula de 16×16 con la baseline en la fila
  12 y el borde izquierdo de las letras en la columna 2.
- Se importa como fuente de 1 bit de celda 16×16, baseline 12 (o `ascender`, si viene), los
  glifos recorridos a la izquierda hasta 2 columnas (lo que se pueda sin perder pixeles: el
  margen de BitFontMaker2 no es parte de la letra), `letterspace / 64` de espaciado,
  `wordspacing` de espacio (5 si no viene) y, con `monospace`, todos con el avance manual
  `monospacewidth`. Los valores pueden venir como texto o número.

## 6. UI (`cs-ui`)

- **Build ▾** (pie del panel FontForge) se habilita con una fuente activa y abre un menú del kit:
  *TTF…*, *Color TTF (COLR)…* (deshabilitado en 1 bit: "Only for color fonts") y *BMFont (.fnt +
  .png)…*. Cada uno abre el diálogo **Build Font** con ese formato elegido.
- **Build Font** (diálogo del kit): formato (radios), nombre de la familia (el de la fuente),
  y una línea con lo que se va a escribir (`MiFuente.ttf`, o `MiFuente.fnt + MiFuente.png`).
  *Build* guarda con el diálogo de la plataforma (en la web, descarga; el BMFont son dos
  descargas).
- **File → Import Font…** (`.ttf`, `.otf`, `.json`) y **File → Export Font…** (con una fuente
  activa: el mismo diálogo, con TTF o COLR según el modo). En **Library → Fonts**, un botón
  *Import…* debajo de la lista.
- **Import Font** (para TTF/OTF): tamaño (con el sugerido), umbral, caracteres, modo y una
  **vista previa** con "AaBbGg 0123" rasterizado con esas opciones. *Import* crea la fuente en el
  documento abierto (o en uno nuevo desde Home), abre su pestaña y el panel. El JSON de
  BitFontMaker2 se importa directo, sin diálogo.
- Captura: `CHIMPSPRITE_DEMO_FONT_BUILD=1` abre la fuente de ejemplo con el diálogo Build Font, y
  `CHIMPSPRITE_DEMO_FONT_IMPORT=<ruta.ttf>` el de importar.

## 7. Pruebas

- **Contornos:** un pixel = un cuadrado horario de 4 puntos; una L de 3 pixeles = un contorno de
  6 puntos; un anillo = contorno de afuera + hueco al revés; diagonal = 2 contornos.
- **TTF:** se vuelve a leer con `ttf-parser`: número de glifos, cmap, avances, unitsPerEm,
  ascender/descender, nombre de la familia, cajas de los glifos; `fontdue` (la de la herramienta
  Texto) lo carga y lo rasteriza al alto de la celda igual que los pixeles; COLR: capas y colores
  con `paint_color_glyph`; caracteres fuera del BMP (formato 12).
- **BMFont:** el `.fnt` (líneas y valores) y que cada glifo del atlas tenga sus pixeles.
- **Import:** el tamaño sugerido de un TTF nuestro es la celda; ida y vuelta 1 bit y color;
  umbral; BitFontMaker2 (bits, recorrido, monospace, valores como texto).
- **UI:** Build habilitado con fuente; el menú abre el diálogo con el formato; Import de JSON crea
  la fuente y abre su pestaña.

## Decisiones que el usuario puede revisar

1. **64 unidades por pixel** y `unitsPerEm = alto × 64` (a `alto` px, nítida). Alternativa:
   1024 fijo como BitFontMaker2 (celdas de 16) o 128 por pixel.
2. **La línea es la celda entera** (ascender + descender = alto, lineGap 0), no las medidas de
   cap/descent: así el texto en otros programas se ve con el mismo alto de línea que la celda.
3. **TTF escrito a mano** en vez de `write-fonts`: sin dependencias nuevas, pequeño y fácil de
   probar; si un día hace falta hinting o kerning (parte 4), se puede cambiar.
4. **Build ▾ es un menú** (TTF / Color TTF / BMFont) que abre un diálogo; *Export Font…* del
   menú File abre el mismo diálogo.
5. **BMFont en blanco** para 1 bit (el motor lo tiñe), atlas en estantes con 1 px de aire,
   potencias de 2.
6. **Import:** 4×4 muestras por pixel y umbral 50 %; el tamaño sugerido por la cuadrícula común
   de las coordenadas; All toma hasta 4096 caracteres; COLR v1 (degradados) no.
7. **BitFontMaker2:** se quitan hasta 2 columnas de margen izquierdo; monospace → avances
   manuales (nuestro Monospace es el ancho de la celda, que en BitFontMaker2 es siempre 16).
8. El `.notdef` exportado guarda el ancho de la celda como avance (así la ida y vuelta conserva
   la celda).
