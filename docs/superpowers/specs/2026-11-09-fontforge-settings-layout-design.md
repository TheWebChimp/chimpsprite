# FontForge: la vista de ajustes como el mockup

## Contexto

La vista de ajustes del panel FontForge (el engrane) era una columna larga: título "Font", una
fila por campo, las medidas en dos columnas y la familia al final, con scroll aun en pantallas
grandes. El usuario dibujó cómo la quiere: `.refs/fontforge/settings-mockup.png` (960×504, a 1x:
cada pixel es un punto de UI). Regla: el mockup manda en la geometría (posiciones, tamaños, filas,
columnas); el centrado de los textos y la tipografía fina son criterio nuestro.

## Medidas (del mockup, en puntos del panel: su borde es la fila y la columna 0)

El panel mide 330×408 por fuera (x 463–792, y 70–477 en el mockup): el mismo ancho de hoy
(`font_panel::WIDTH` 314 + 2×8 de margen). Las líneas del título (29) y del pie (alto − 33) no
cambian. El contenido que se desplaza empieza en (8, 35); abajo, "y" es desde ahí.

| Qué | x | y | tamaño |
|---|---|---|---|
| Logo nuevo (pieza `fontforge.logo`) | 6 (del panel) | 2 (del panel) | 141×28, pisa la línea del título |
| Etiquetas Family / Style / Weight | 0 | 1 / 22 / 43 (filas de 17) | |
| Campos Family y Style | 37 | 1 / 22 | 160 |
| Weight (select) | 37 | 43 | 112 |
| Casilla Ghost | 154 | 42 (la casilla 1 más arriba que el select, como en el mockup) | |
| Select de la fantasma (Ubuntu/Hack) | 200 | 43 | 80 |
| Italic / Monospace / Auto width (A) | 37 / 85 / 154 | 64 | |
| Mode · 1 bit · Cell · 16×16 (texto; x es donde empieza la tinta) | 0 / 74 / 151 / 234 | 85 | |
| Baseline, x-height, Letter spacing, Cap height, Descent, Space width | etiqueta 0, stepper 74 (62 de ancho) | 111 + 21·i | |
| Recuadro de la vista previa | 146 | 111 | 166 (hasta 2 antes del borde del contenido) × 121 |
| Family "…" (n styles) | 0 | 244 (sus mayúsculas en la 250) | |
| Radios de los estilos (con Rebuild) | 0 | 268 + 21·i | |
| New Variant… | 0 | 291 + 21·(n−1) | |

Las etiquetas van en la fila de su control con las mayúsculas 6 abajo, como en el mockup (el texto
de egui las tiene 4 abajo de su caja: se pinta 2 abajo de la fila), y con la tinta en su x (sin el
aire de la primera letra, como el del 1).

## Objetivo

1. **Logo nuevo**: el chango herrero martillando una "A" en el yunque, con chispas, y "FontForge"
   en letras de piedra, recortado pixel a pixel del mockup (lo que era fondo del panel o la línea
   del título queda transparente). Reemplaza a la pieza `fontforge.logo` del skin (se borra la
   vieja). Se dibuja después de la línea del título, porque la pisa (el yunque y los pies).
2. **Sin el título "Font"**; la geometría de la tabla de arriba. Ghost y su select en la fila de
   Weight; Italic, Monospace y Auto width (A) en una fila (Auto width solo con glifo actual, como
   hoy).
3. **Métricas en una columna** con los mismos steppers del kit.
4. **Vista previa de métricas** en el recuadro gris: el glifo actual (si está vacío, una letra de
   muestra dibujada: H, x, A, a, n, o o la primera con pixeles) a escala entera sobre el damero
   del lienzo (sus colores, celdas de 16 pixeles), con las guías del lienzo: baseline sólida,
   cap height, x-height y descent punteadas (los colores de `font_overlay`), el borde izquierdo y
   la línea del ancho de avance. Con el puntero sobre la fila de un stepper, su línea se resalta
   (roja sólida de 2); Letter spacing y Space width, que no tienen guía, muestran la suya solo
   entonces (el espaciado después del avance; el ancho del espacio desde el borde izquierdo). La
   lógica (escala, celda, líneas) es una función sin egui de pintura, con tests.
5. **Familia** como en el mockup: la etiqueta con la nota, un radio por estilo (Rebuild en los
   generados) y New Variant… debajo.
6. **Demo**: `CHIMPSPRITE_DEMO_FONT_CONFIG` abre una fuente nueva de 16×16, de un estilo, con la
   vista de ajustes (como el mockup).

Comportamiento, comandos y undo no cambian.

## Fuera

- El texto "Auto width (!)" del mockup: queda el de hoy, "Auto width (A)" con el glifo actual.
- La tipografía de las casillas: el kit pone la casilla 2 filas arriba de las mayúsculas; en el
  mockup el texto está 2 más abajo. Se respeta la casilla (geometría) y el kit para el texto.

## Decisiones que el usuario puede revisar

- **El recuadro gris es una vista previa de métricas** (el mockup lo deja vacío): glifo actual o
  una letra de muestra, a escala entera sobre el gris del lienzo, con las guías de baseline,
  x-height, cap height y descent y el ancho de avance; al pasar el puntero sobre un stepper (su
  fila entera, etiqueta incluida), su línea se resalta. Letter spacing y Space width solo dibujan
  su línea al resaltarse.
- La casilla Ghost va 1 punto más arriba que el select de su fila, como en el mockup.
- Con varios estilos, los radios van cada 21 (el paso de las filas del formulario).
- Sin Pixel UI se usa la misma geometría con los controles de egui.
