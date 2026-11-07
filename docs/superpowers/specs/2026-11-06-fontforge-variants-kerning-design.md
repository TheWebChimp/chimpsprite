# FontForge, parte 4: variantes de la fuente y kerning

Fecha: 2026-11-06 · Estado: implementado

Sigue al núcleo (`2026-11-01-pixel-font-core-design.md`), a las pestañas
(`2026-11-02-cad-font-tabs-design.md`), a exportar/importar
(`2026-11-06-fontforge-export-import-design.md`) y a los compuestos
(`2026-11-06-fontforge-composites-design.md`). Una fuente es un artboard con `font` (Library →
Fonts, su propia pestaña), con un frame por glifo del tamaño de la celda.

## Objetivo

- **Kerning:** pares de kerning (carácter con carácter) y **clases** (como las *kerning classes*
  de FontForge y los grupos `kern1`/`kern2` de UFO), editables en una vista **Kerning** del panel
  FontForge con un texto de prueba en vivo. Se respetan en todo lo que escribe con la fuente (Test
  Drive, herramienta Texto, tarjeta y Library) y se exportan (tablas `kern` y `GPOS` del TTF;
  `kerning` del BMFont) y se importan de un TTF/OTF (`GPOS` y `kern`).
- **Variantes (estilos):** las fuentes de una **familia** (Regular, Bold, Italic, Bold Italic,
  Outline, Shadow o cualquier otro nombre), cada una su propio artboard de fuente. Se **generan**
  desde otra fuente con transformaciones de pixeles (como *Element → Style* de FontForge:
  engrosar, sesgar, contorno, sombra) o como copia para dibujarlas a mano, se pueden **reconstruir**
  desde su fuente, y se exportan con los nombres y metadatos de estilo correctos (un archivo por
  variante, o la familia entera de una vez).
- Fuera: interpolar pesos (variable fonts), kerning vertical o contextual, comandos MCP.

## 1. Kerning: el modelo (`cs-core::font_kerning`)

```rust
pub enum KernSide { Char(char), Class(String) }   // en el texto: "A" o "@round"
pub enum ClassSide { First, Second }              // el de la izquierda o el de la derecha
pub struct KernClass { pub name: String, pub side: ClassSide, pub chars: Vec<char> }
pub struct KernPair { pub first: KernSide, pub second: KernSide, pub value: i32 }
pub struct FontKerning {
    pub pairs: Vec<KernPair>,      // ordenados por (first, second), sin repetir
    pub classes: Vec<KernClass>,
}
```

- `FontInfo` gana `kerning: FontKerning` (con `serde(default)` y sin escribirse vacío: los
  `.chimp` viejos abren igual y los nuevos sin kerning no cambian).
- **El valor** va en pixeles y se suma al avance del primer carácter: negativo acerca, positivo
  separa. En la UI, de −ancho de la celda a +ancho de la celda.
- **Las clases** tienen nombre y lado: una clase *First* agrupa caracteres cuando van a la
  izquierda del par y una *Second* cuando van a la derecha (como FontForge: "First char" / "Second
  char"). Un carácter cuenta en la primera clase de ese lado que lo tenga.
- **Qué par vale** para `a` seguida de `b` (como UFO): `a b` exacto; si no, `a @Second(b)`; si no,
  `@First(a) b`; si no, `@First(a) @Second(b)`; si no, 0. Así un par exacto (también de valor 0)
  es una **excepción** a la clase.
- `flatten(has)` da todos los pares con valor distinto de 0 entre caracteres que tiene la fuente:
  es lo que usan el render y los exports.
- `PixelFontView` gana `kerning: BTreeMap<(char, char), i32>` (lo aplanado) y
  `FontInfo::text_width` lo suma.

## 2. Kerning al escribir (`cs-tools::text`)

- `font_layout(view, text)` da, por carácter, su línea, su posición (la pluma, ya con el kerning
  del par anterior) y su avance; `font_text` dibuja con eso y los paneles lo usan para saber qué
  letra está bajo el puntero. El kerning se aplica entre dos glifos seguidos que la fuente tiene
  (no a través de un espacio ni de un carácter que falta).
- Así lo respetan el Test Drive, la vista Kerning, la herramienta Texto, la tarjeta del lienzo y
  la muestra de la Library, sin tocarlos uno por uno.

## 3. Variantes: el modelo (`cs-core::font_style`)

```rust
pub struct FontStyle {
    pub name: String,      // "Regular", "Bold", "Italic", "Bold Italic", "Outline"…
    pub weight: u16,       // 100–900 (usWeightClass); Regular 400, Bold 700
    pub italic: bool,
    pub slant: u32,        // la itálica: 1 columna cada `slant` filas (0: derecha)
    pub source: Option<StyleSource>,   // de dónde se generó (para Rebuild)
}
pub struct StyleSource { pub artboard: ArtboardId, pub recipe: VariantRecipe }
pub enum VariantKind { Bold, Italic, BoldItalic, Outline, Shadow, Copy }
pub struct VariantRecipe { pub kind: VariantKind, pub slant: u32, pub color: Rgba, pub square: bool }
```

- `FontInfo` gana `style: FontStyle` (por defecto Regular 400, sin escribirse si es el de
  siempre).
- **La familia es `FontInfo.name`** (lo que ya era el nombre de la fuente y del TTF). Las fuentes
  del documento con el mismo `name` son una familia; cada una es su propio artboard, con sus
  glifos, medidas y kerning.
- **El nombre completo** (`full_name`): la familia y el estilo ("Mine Bold"); el Regular es solo la
  familia ("Mine"). Es el nombre del artboard (Library, pestaña, título del glifo, selector de la
  herramienta Texto). Renombrar el artboard cambia la familia: si el nombre nuevo termina en el
  estilo ("Other Bold"), se quita; Duplicate le pone "copy" a la familia ("Mine copy Bold").
- Nombres para los formatos (`FontStyle::legacy_names`, la regla RIBBI de OpenType): si el estilo
  es Regular, Bold, Italic o Bold Italic, la familia es la familia y la subfamilia el estilo; si
  no, la familia "legacy" lleva el estilo sin "Italic" ("Mine Outline", "Mine Light") y la
  subfamilia es "Regular" o "Italic"; la familia y subfamilia tipográficas (IDs 16 y 17) son las de
  verdad. Bold (para `fsSelection` y `macStyle`) = la subfamilia legacy empieza con "Bold".

## 4. Generar una variante (`cs-raster::font_style`)

`generate_variant(font, sprite, source: ArtboardId, recipe) -> (FontInfo, Sprite)`: la misma
familia, con el estilo de la receta, el mismo orden de glifos y el kerning, el espaciado, el
espacio, Monospace y la letra fantasma de la fuente de origen.

Cada glifo transforma **sus pixeles propios** (los compuestos siguen siendo compuestos):

- **Bold:** cada pixel también se pinta 1 a la derecha (en color, con el color del de su
  izquierda). Es el "bold" clásico de las fuentes pixel.
- **Italic:** las filas de arriba de la baseline se corren a la derecha 1 columna cada `slant`
  filas, contando desde la fila de arriba de la baseline (que no se mueve); las de abajo de la
  baseline (los descendentes) no se mueven, para que nada se salga por la izquierda. `slant` por
  defecto 3 (≈ 18°), de 1 a 8.
- **Bold Italic:** Bold y luego Italic.
- **Outline:** el contorno de afuera con la matriz del filtro Outline (`cs_raster::outline_mask`:
  Circle = 4 vecinos, Square = 8). En 1 bit, la letra **hueca** (solo el contorno, como el estilo
  Outline clásico de Mac); en color, la letra con el contorno en el color elegido (negro por
  defecto).
- **Shadow:** en color, la letra con su sombra 1 abajo a la derecha (lo que no tapa la letra) en el
  color elegido; en 1 bit, hueca con el borde de abajo y la derecha de 2 (el estilo Shadow de
  Mac): `(contorno ∪ contorno corrido (1, 1)) − letra`, con el contorno de 4 vecinos.
- **Copy:** igual (para dibujar la variante a mano).

Después, la celda: si algo quedó a la izquierda o arriba de la celda, todo se corre lo necesario
(la baseline también) y la celda crece lo que haga falta a la derecha y abajo (tope 256). Los
anchos manuales crecen lo que crece la letra a la derecha más lo que se corrió todo (Bold 1;
Outline 1 + 1 = 2; Shadow 1 en color y 2 + 1 = 3 en 1 bit; Italic 0: la itálica no cambia los
anchos, el automático ya ve la inclinación). Los descendentes crecen lo que creció la celda abajo.

**Compuestos:** sus componentes quedan con los mismos offsets (los pixeles de cada glifo se
transforman igual), salvo en las itálicas, donde el offset en x de cada componente se ajusta a lo
que se corre su altura (`s(centro + dy) − s(centro)`, con `centro` la fila del medio de la tinta
del componente): así la `´` de la `á` itálica queda sobre la `a` itálica.

**Rebuild:** una variante generada guarda su fuente y su receta (`style.source`); *Rebuild* la
genera otra vez desde su fuente (glifos, medidas y kerning nuevos; el nombre y el estilo se
quedan) en un paso de deshacer. Si la fuente ya no está, Rebuild se deshabilita.

## 5. Comandos (`cs-commands::font`)

- `set_kerning(artboard, kerning)`: el kerning entero (un `SetFont`); `None` si no cambia. La UI
  agrupa el arrastre de un stepper o de una letra en un paso.
- `add_font_artboard` (ya está) agrega la variante generada.
- `replace_font(artboard, font, sprite)`: `SetSprite` + `SetFont` en un paso (Rebuild).

## 6. Exportar (`cs-io::font`)

### TTF

- **`name`:** familia y subfamilia legacy (IDs 1 y 2), nombre completo (4: "Mine Bold"; el
  Regular, "Mine"), ID único (3), versión (5), PostScript (6: "Mine-Bold"; el Regular, "Mine") y,
  si el estilo no es RIBBI, familia y subfamilia tipográficas (16 y 17).
- **`OS/2`:** `usWeightClass` = el peso; `fsSelection` = ITALIC, BOLD o REGULAR (y OUTLINED si es
  un Outline generado), con USE_TYPO_METRICS. **`head.macStyle`:** bold, italic, outline y shadow.
  **`post.italicAngle`** = −atan(1/slant) en grados (0 sin slant) y **`hhea.caretSlopeRise/Run`**
  = (slant, 1) en las itálicas.
- **`kern`** (versión 0, una subtabla formato 0, horizontal): los pares aplanados por glifo, en
  unidades (pixeles × 64). **`GPOS`** (1.0): scripts `DFLT` y `latn`, la feature `kern` y un
  lookup de tipo 2 (PairPos formato 1, `XAdvance` del primero) con los mismos pares. Las dos, para
  que lo lean los programas nuevos (HarfBuzz, navegadores) y los viejos (`kern`: fontdue, GDI,
  FreeType `FT_Get_Kerning`). Sin pares, ninguna de las dos. Más de 10 000 pares aplanados son un
  error ("too many kerning pairs") porque no caben en una subtabla.

### BMFont

`info face` = la familia legacy, `bold` e `italic` del estilo; al final `kernings count=N` y una
línea `kerning first=… second=… amount=…` por par aplanado.

### La familia de una vez

En el diálogo **Build Font**, si la fuente tiene otros estilos en el documento, una casilla
**"Whole family (N styles)"**: escribe un archivo por estilo (`Mine-Regular.ttf`,
`Mine-Bold.ttf`…; en BMFont, cada `.fnt` con su `.png`) al lado del que se elige. Un estilo solo
se llama `Mine.ttf` si es el Regular y `Mine-Bold.ttf` si no.

## 7. Importar (`cs-io::font::ttf_import`)

- **Kerning:** de la feature `kern` del `GPOS` (lookups PairPos formato 1 y 2: el `XAdvance` del
  primero) y, si no tiene, de la tabla `kern` (subtablas horizontales); solo entre los caracteres
  importados, escalado a pixeles y redondeado, sin los 0. Entran como pares de caracteres (las
  clases no se reconstruyen).
- **Estilo:** nombre de la subfamilia tipográfica (17) o la legacy (2); peso de `OS/2`; itálica de
  `fsSelection`/`macStyle`; `slant` de `post.italicAngle` (round(1/tan|ángulo|)). La familia, la
  tipográfica (16) o la legacy (1), como ya se hacía. Importar `Mine-Bold.ttf` en un documento que
  ya tiene "Mine" lo hace parte de la familia.
- `FontBuild` gana `style` y `kerning: Vec<(char, char, i32)>` (`build_font` los pasa a la fuente,
  sin los pares de caracteres que no quedaron).
- **Ida y vuelta:** exportar y volver a importar da los mismos pares aplanados y el mismo estilo.

## 8. UI (`cs-ui`)

- **Panel FontForge, título:** un tercer ícono, **Kerning** (`icon.fontforge.kern`, en (275, 9),
  a la izquierda de los ajustes; ícono provisional: el usuario lo redibujará) que abre la vista
  Kerning.
- **Vista Kerning:**
  - "Kerning" con la cuenta de pares; el campo del texto de prueba (el mismo del Test Drive) y su
    papel a ×2 con el kerning aplicado.
  - **Clic en una letra** elige el par que forma con la de antes (marcado en el papel); **arrastrar
    la letra** a los lados cambia ese par por pixeles enteros (un paso al soltar).
  - **El par elegido:** "A V", su valor (stepper), de dónde sale ("pair", "@round · V", …) y
    *Remove*; si sale de una clase, *Exception* crea el par exacto con ese valor. Cambiar el valor
    de un par que no existe lo crea.
  - **Pairs:** la lista de pares (cada uno con su stepper y Remove; elegir uno lo elige en el
    papel si está en el texto) y una fila para agregar: First, Second (un carácter o `@clase`) y
    *Add*.
  - **Classes:** cada clase con su nombre, su lado (First/Second) y sus caracteres (un campo de
    texto), y Remove; *New class*.
- **Vista de ajustes (Font Settings):** "Name" pasa a **Family**; abajo, **Style** (nombre),
  **Weight** (Thin 100 … Black 900) e **Italic**; y la sección **Family "Mine"**: un renglón por
  estilo de la familia (el actual marcado; clic abre su pestaña) con *Rebuild* en los generados, y
  **New Variant…**.
- **New Variant…:** diálogo del kit: de qué fuente sale, el tipo (Bold, Italic, Bold Italic,
  Outline, Shadow, Copy), la inclinación (Italic), la matriz (Outline: Circle/Square), el color
  (Outline y Shadow en fuentes de color), el nombre del estilo y el peso (los de siempre del tipo,
  editables) y una vista previa "AaBbGg 0123" con la variante. *Create* agrega la fuente, abre su
  pestaña y el panel. Si ya hay un estilo con ese nombre en la familia, avisa y no deja crear.
- **Build Font:** la casilla de la familia (sección 6) y la lista de archivos.
- Capturas: `CHIMPSPRITE_DEMO_FONT_KERN=1` (con `_FONT`) pone unos pares y una clase y abre la vista
  Kerning; `CHIMPSPRITE_DEMO_FONT_STYLES=1` genera Bold e Italic y abre los ajustes con la familia;
  `CHIMPSPRITE_DEMO_FONT_VARIANT=1` abre New Variant….

## 9. Pruebas

- **Kerning (modelo):** precedencia (exacto > char·clase > clase·char > clase·clase), excepción de
  0, aplanado solo con caracteres de la fuente, orden y sin repetir al poner pares, `@clase` al
  leer, JSON con y sin kerning.
- **Texto:** `font_layout`/`font_text` aplican el par, no a través de espacios ni faltantes;
  `text_width` con kerning.
- **Estilos:** `full_name`, quitar el estilo del nombre, `legacy_names` RIBBI y no RIBBI, JSON
  viejo sin estilo.
- **Variantes:** Bold (pixeles y color), Italic (filas, descendentes quietos, compuestos
  ajustados), Bold Italic, Outline hueco y de color (Circle/Square), Shadow 1 bit y color, celda
  que crece y se corre, anchos manuales, kerning copiado, Copy.
- **Comandos:** set_kerning y replace_font con su undo.
- **TTF:** name/OS2/head/post de un Bold, un Italic y un Outline (leídos con `ttf-parser`); `kern`
  leído por fontdue (`horizontal_kern`) y `GPOS` por `ttf-parser`; ida y vuelta de pares y estilo;
  sin kerning no hay tablas; más de 10 000, error.
- **BMFont:** `bold`/`italic` y las líneas `kerning`.
- **UI:** el ícono Kerning abre la vista; clic en una letra elige el par y arrastrarla lo cambia
  en un paso; agregar un par y una clase; la familia en los ajustes; New Variant crea el Bold y
  abre su pestaña; Rebuild; Build con la familia escribe un archivo por estilo.

## Decisiones que el usuario puede revisar

1. **Una variante es otro artboard de fuente** (con sus glifos), no una vista en vivo de la
   Regular: se puede retocar a mano como cualquier fuente, y *Rebuild* la vuelve a generar si la
   Regular cambió (pisando los retoques, con Undo).
2. **La familia es el nombre de la fuente**: mismo nombre = misma familia; el artboard se llama
   "Familia Estilo". Renombrar una variante la saca de la familia (no hay "renombrar la familia
   entera").
3. **Bold = 1 px a la derecha**; **Italic = 1 columna cada 3 filas** sobre la baseline, sin mover
   los descendentes; **Outline** con la matriz del filtro Outline (hueca en 1 bit); **Shadow** 1
   px abajo a la derecha (en 1 bit, hueca con el borde de abajo y la derecha doble, como Mac).
4. Los anchos manuales crecen con la letra (Bold +1, Outline +2, Shadow +1 en color y +3 en
   1 bit); la itálica no los cambia. La celda crece lo que haga falta (no se recorta nada).
5. **Kerning en pixeles enteros**, con clases First/Second y la precedencia de UFO (los pares
   exactos son excepciones).
6. **Se exportan `kern` y `GPOS` aplanados** (pares de glifos, sin `PairPos` formato 2 de clases):
   simple y suficiente para fuentes pixel; tope de 10 000 pares.
7. **Al importar**, el kerning entra como pares de caracteres (sin reconstruir clases) y solo entre
   los caracteres importados.
8. El kerning **no cruza espacios**: un par con el espacio no se puede definir (el espacio no es
   un glifo de la fuente).
9. La vista Kerning es una **tercera vista** del panel (ícono provisional) en vez de meter todo en
   el Test Drive de la vista Edit, que ya es larga.
10. Archivos de la familia: `Familia-Estilo.ttf` (estilo sin espacios); un Regular solo sigue
    saliendo `Familia.ttf` como antes.
