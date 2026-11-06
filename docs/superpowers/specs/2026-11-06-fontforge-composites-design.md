# FontForge, parte 3: glifos compuestos y generar glifos

Fecha: 2026-11-06 · Estado: implementado

Sigue al núcleo (`2026-11-01-pixel-font-core-design.md`) y a exportar/importar
(`2026-11-06-fontforge-export-import-design.md`). Una fuente es un artboard con `font`, con un
frame por glifo del tamaño de la celda.

## Objetivo

- **Glifos compuestos**, como los *composites* de TrueType: `á` = `a` + `´` desplazado. Se dibujan
  en vivo (si cambia la `a`, cambia la `á`), se pueden **desvincular** (pasar a pixeles propios)
  y sus componentes se mueven con flechas o arrastrando.
- **Auto-build accents:** arma los acentuados latinos (Latin-1 Supplement, Latin Extended-A y las
  cuatro de coma de Latin Extended-B: Ș ș Ț ț) desde la base y la marca, con una tabla de
  descomposición canónica (NFD) embebida y reglas de colocación.
- **Generar glifos:** (a) **rellenar los que faltan desde un TTF** (rasterizado a la celda de la
  fuente, con la baseline alineada); (b) **desde una imagen**: una hoja de glifos en cuadrícula
  (PNG), eligiendo la celda, el orden de los caracteres y los colores de tinta y fondo.
- Fuera: small caps automáticas (anotado en minors), comandos MCP de fuentes.

## 1. El modelo (`cs-core::font`)

```rust
pub struct GlyphComponent {
    pub ch: char,             // el glifo que se dibuja
    pub offset: (i32, i32),   // en pixeles, desde la esquina de la celda
}

pub struct GlyphInfo {
    pub ch: char,
    pub advance: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<GlyphComponent>,   // vacío: glifo normal
}
```

- Un glifo es **compuesto** si tiene componentes. Puede además tener pixeles propios (su cel), que
  van **encima** de los componentes: así se retoca un compuesto sin desvincularlo.
- **Lo que se ve** (`FontInfo::composed`): los componentes en orden, cada uno con *lo que se ve* de
  su glifo (recursivo: un compuesto puede usar otro compuesto) recorrido por su offset, y encima
  los pixeles propios; recortado a la celda. Un componente cuyo carácter no está en la fuente no
  dibuja nada. Los ciclos no se pueden crear (los comandos los rechazan) y, por si un archivo trae
  uno, la recursión se corta en 8 niveles y no vuelve a entrar a un glifo de la cadena.
- `font_view` (Test Drive, herramienta Texto, mapa, Library, export) y el **ancho automático**
  usan lo que se ve. El ancho automático de un compuesto es el mayor entre el de su **primer
  componente** (la base, con su offset x) y lo dibujado + `letter_spacing`: `á` avanza como `a`
  aunque `a` tenga ancho manual (lo que en TrueType es `USE_MY_METRICS`).
- `.chimp`: va en `FontInfo` (serde), y los viejos abren igual.

## 2. Acentos (`cs-core::font::accents`)

- **Tabla NFD embebida** (165 caracteres, generada con `unicodedata` de Python): carácter → base +
  marca combinante (U+0300…U+0328).
- **De dónde sale la marca:** cada marca combinante tiene sus fuentes, en orden de preferencia.
  Para **minúsculas**: la marca espaciadora (`´` U+00B4, `` ` ``, `ˆ` U+02C6 o `^`, `˜` U+02DC o
  `~`, `¯`, `˘`, `˙`, `¨`, `˚`, `˝`, `ˇ`, `¸`, `˛`, `,` para la coma de abajo) y al final la
  combinante. Para **mayúsculas**: primero la **combinante** (U+0301…), después las espaciadoras.
  La convención es: *quien quiera una versión más baja/plana de la marca para mayúsculas la dibuja
  en el código combinante* (U+0301 = "acute.cap"); si no, se usa la misma.
- **Base:** la letra; para `i` y `j` con marca de arriba, `ı` (U+0131) / `ȷ` (U+0237) si la fuente
  los tiene (sin punto).
- **Colocación** (sobre la tinta, no la celda):
  - **Arriba** (todas menos las de abajo): centrada sobre la tinta de la base (redondeo hacia la
    izquierda) y con **1 px de aire** entre la fila de abajo de la marca y la de arriba de la base.
    Si no cabe en la celda, sube lo que pueda (fila 0) y se cuenta como "apretada".
  - **Cedilla y coma de abajo:** centradas, pegadas debajo de la tinta de la base que está sobre la
    baseline (la fila de arriba de la marca justo en la baseline). Si la base tiene descendente
    (`ģ`), la marca va **arriba** (como se dibuja la ģ) y girada no: se usa tal cual.
  - **Ogonek:** debajo, pegada a la baseline, con su borde derecho en el derecho de la tinta de la
    base.
  - **Caron de ď, ľ, ť, Ľ:** en esas cuatro, si la fuente tiene `'` (U+0027) o `’` (U+2019), se
    usa ese apóstrofo a la derecha de la tinta (1 px de aire) con su arriba en el arriba de la
    base; si no, la caron normal.
- **Auto-build accents** arma todos los de la tabla cuya base tenga tinta y que tengan alguna
  fuente de marca con tinta. **No toca** los glifos que ya tienen pixeles propios o componentes
  (los vacíos sí se arman); los que faltan se agregan. Un solo paso de deshacer. Devuelve cuántos
  armó, cuántos quedaron apretados y cuántos no pudo (sin base o sin marca), y la app lo dice en
  un aviso.
- Lo mismo para un solo carácter: *Build composite* en el menú de una celda del mapa y *Build* en
  la sección Glyph; ahí, como lo pidió el usuario, **sí reemplaza** un glifo dibujado o compuesto.

## 3. Comandos (`cs-commands::font`)

- `set_components(artboard, ch, components)` (`SetFont`); `None` si no cambia o si haría un
  ciclo.
- `unlink_glyph(artboard, ch)`: lo que se ve pasa al cel del glifo (imagen nueva) y se borran los
  componentes. Un paso.
- `remove_glyph` primero **desvincula** los compuestos que usaban el glifo (para no perder su
  dibujo), todo en el mismo paso.
- `auto_build_accents(artboard, only: Option<&[char]>) -> Option<(DocCommand, AccentReport)>`.
- `fill_glyphs(artboard, glyphs: &[GlyphBuild], replace) -> Option<(DocCommand, usize)>`: pone los
  pixeles de cada glifo (agrega los que faltan; los que existen solo si están vacíos —sin pixeles
  ni componentes— o con `replace`, que además les quita los componentes). En 1 bit todo pixel es
  tinta; en color, sin color es negro. Un paso.

## 4. Generar glifos (`cs-io::font`)

- **Desde un TTF** (`ttf_import::fill_from_ttf`): se rasteriza con `import_ttf` (tamaño y umbral
  elegidos, caracteres Latin o All) y cada glifo se recorre en y para que su baseline caiga en la
  de la fuente; lo que se salga de la celda se recorta. Luego `fill_glyphs`.
- **Desde una imagen** (`font::sheet::glyphs_from_sheet`): la hoja se parte con la `SheetGrid` de
  Import Sprite Sheet (celda, aire entre celdas, margen). Las celdas, de izquierda a derecha y de
  arriba abajo, toman los caracteres del texto de orden (saltos de línea ignorados; un espacio
  salta una celda). Cada pixel de la celda: si es transparente (alfa < 128) o del color de **fondo**
  (distancia ≤ 48 en RGB), apagado; en 1 bit, prendido si está más cerca de la **tinta** que del
  fondo; en color, con su color. La fila de la baseline dentro de la celda de la hoja (por defecto
  la de la fuente) se alinea con la de la fuente. Por defecto: celda = la de la fuente, fondo = el
  pixel de arriba a la izquierda, tinta = el color más común que no es fondo, orden = ASCII de `!`
  a `~`.

## 5. UI (`cs-ui`)

- **Lienzo:** en un glifo compuesto, sus componentes se ven como **fantasma** (azul translúcido)
  entre la letra fantasma y los pixeles propios; el componente elegido, más fuerte y con su caja
  punteada. Con la herramienta **Move**, un clic sobre un componente lo elige y arrastrarlo lo
  mueve por pixeles enteros (un paso al soltar); las **flechas** lo mueven 1 px (10 con Shift).
  Con otras herramientas se pinta como siempre (los pixeles propios).
- **Panel FontForge, vista Edit:** debajo del Test Drive (para no mover las filas del mockup), la
  sección **Glyph** del glifo actual:
  - si es compuesto, una fila por componente: el carácter, sus offsets X e Y (steppers del kit),
    un botón para elegirlo y uno para quitarlo;
  - un campo *Component* con botón *Add* (agrega ese carácter como componente en (0, 0));
  - *Unlink* (deshabilitado si no es compuesto) y *Build* (arma el compuesto del carácter actual
    si está en la tabla).
- **Pie:** *Generate ▾* a la izquierda de *Build ▾*: *Auto-build accents*, *Fill from TTF…* y
  *Glyphs from image…*.
- **Mapa:** los compuestos llevan una marquita azul de 3×3 en la esquina de arriba a la
  izquierda; su menú agrega *Unlink*; el de un carácter de la tabla, *Build composite*.
- **Fill from TTF…:** el diálogo Import Font de la parte 2 en modo "rellenar": sin Mode (el de la
  fuente), con *Replace drawn glyphs* y el botón *Fill*.
- **Glyphs from image…:** diálogo del kit con la vista de la hoja y la cuadrícula, celda, aire,
  margen, fila de la baseline, tinta y fondo (selectores de color), el texto del orden, *Replace
  drawn glyphs* y *Import*.
- Capturas: `CHIMPSPRITE_DEMO_FONT_COMPOSITE=1` (con `_FONT`) arma los acentos de la fuente de
  ejemplo y abre `á` con la `´` elegida (`CHIMPSPRITE_DEMO_FONT_SCROLL=1` baja el panel hasta la
  sección Glyph); `CHIMPSPRITE_DEMO_FONT_SHEET=<png>` abre Glyphs from image….

## 6. Pruebas

- **Modelo:** lo que se ve (componentes con offset, propios encima, recorte, carácter que falta,
  ciclo cortado, anidado); ancho automático de un compuesto (sigue a la base manual); JSON con y
  sin componentes.
- **Acentos:** tabla (165, sin repetidos, base y marca correctas en muestras); colocación arriba
  con 1 px, centrada; cedilla debajo; ogonek a la derecha; ģ arriba; ı para í; mayúscula con la
  combinante; apretada; Auto-build no toca los dibujados.
- **Comandos:** set/unlink/remove con dependientes/auto-build/fill, cada uno con su undo; ciclo
  rechazado.
- **IO:** hoja → glifos (orden, espacio salta, 1 bit por cercanía, color, baseline); fill desde un
  TTF nuestro alinea la baseline.
- **UI:** arrastrar un componente con Move; flechas; Unlink desde el panel; Auto-build desde el
  menú.

## Decisiones que el usuario puede revisar

1. **Export aplanado:** TTF, COLR y BMFont escriben lo que se ve (los contornos se juntan pixel a
   pixel, también donde la marca toca la base, como en la cedilla). No se escriben `glyf`
   compuestos: la fuente pesa un poco más pero se ve igual en todos lados y el código es el mismo.
2. **Pixeles propios encima de los componentes** (un compuesto se puede retocar sin desvincularlo).
3. **La "versión cap" de una marca es su código combinante** (U+0301 para `´` de mayúsculas). No
   hay nombres de glifo tipo `acute.cap` porque los glifos se identifican por carácter.
4. **1 px de aire** entre la marca y la base; si no cabe, la marca sube hasta la fila 0 y se avisa.
5. El ancho automático de un compuesto sigue a su **primer componente** (o a lo dibujado, si es
   más ancho).
6. **Remove** de un glifo usado como componente desvincula primero a los que lo usan.
7. Auto-build **no pisa** glifos dibujados ni compuestos existentes.
8. Mover componentes en el lienzo **solo con Move** (con las demás herramientas se pinta).
9. Hoja de glifos: umbral fijo de 48 en RGB para el fondo, alfa < 128 transparente.
10. La sección Glyph va **debajo del Test Drive** (hay que bajar el panel para verla) para que el
    mapa, Add y el Test Drive sigan en las filas del mockup.
11. Glyphs from Image empieza con el orden ASCII desde `!`; una hoja que empieza en otra letra
    necesita editar el texto del orden.
