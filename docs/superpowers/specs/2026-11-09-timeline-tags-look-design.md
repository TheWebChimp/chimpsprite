# Timeline: etiquetas de los tags y bordes de la tabla (diseño)

Referencia: `.refs/timeline/mockup.png` (236×138 a 1x, puntos de UI). Medido con PIL contra una
captura `--demo` alineada: la barra coincide pixel a pixel con desplazamiento (29, 346) en la
captura a 1x, así que todo lo de abajo se compara en coordenadas del mockup.

## Lo que cambia (medido)

Con `L` = columna donde empieza el contenido del primer frame del tag, `R` = el final de la celda
del último frame (`L + 13·n`), `T` = arriba del piso del tag y `B` = el final de la fila de tags
(arriba de los números). En el mockup: `L = 129`, `R = 181`, `T = 49`, `B = 66`.

1. **Etiqueta del tag.** La cajita naranja no cambia (de `L+1`, 11 de alto, tinta + 3 de aire a
   cada lado) pero lleva contorno negro de 1 px con las esquinas recortadas: arriba en `T−1`,
   abajo en `T+11`, a la izquierda en `L`, a la derecha en el final de la cajita + 1.
2. **Corchete.** Pasa de una línea naranja de 1 px a una línea naranja con contorno negro de 1 px
   a cada lado:
   - arriba, en `T+5`, de la cajita (atraviesa su contorno derecho) hasta `R−3`; el contorno va en
     `T+4` (hasta `R−3`; la esquina de afuera `R−2` queda recortada) y en `T+6` (hasta `R−4`);
   - la pata derecha en `R−3` (antes `R−2`, sobre la línea oscura de la tabla; ahora el contorno
     negro es el que va sobre ella) de `T+5` a `B−2`, con contorno en `R−4` y `R−2`;
   - la pata izquierda en `L+1` desde debajo de la cajita hasta `B−2`, con contorno en `L` y `L+2`;
   - las patas terminan abiertas en `B−2` (sin tapa).
   - Si la cajita es más ancha que el tag (no cabe el tramo de arriba), la pata derecha baja
     también desde debajo de la cajita.
3. **Línea de arriba del encabezado.** Con tags, la fila de tags termina con la línea grabada de
   las filas (oscura en `B−2`, brillo en `B−1`) de lado a lado: el encabezado queda con bordes
   completos como las filas de capas. Las patas del corchete van encima de la oscura; el frame
   elegido, encima de las patas. Sin tags la línea de arriba sigue siendo la de la barra.
4. **Margen izquierdo.** La tabla (tags, encabezado y capas) empieza 1 columna más adentro de la
   caja (`x = 12`, antes 11) y esa columna es el brillo del borde izquierdo de la tabla (la oscura
   es el borde de la caja). Las líneas horizontales empiezan en esa columna. La fila elegida pinta
   esa columna en rosa (brillo) con las esquinas en rosa oscuro.

Lo demás (alturas de filas, posiciones verticales, la barra, los números, los cels) ya coincide.

## Dónde más está la etiqueta de tag

Solo en el timeline (`timeline/pixel.rs::tag`). El editor de símbolos usa la misma grilla. No hay
otra tira de tags dibujada en la app (preview, export y Play in Place solo leen los tags).

## Decisiones que el usuario puede revisar

- El contorno es el negro del mockup (`#050403`) para cualquier color de tag (en un tag negro no se
  distingue, como en Aseprite).
- Los pisos siguen a 13: con el contorno, la cajita de un piso toca la del de abajo sin aire.
- El texto del nombre no cambia (la tipografía del mockup tiene brillos naranja claro que son de
  su render, no de la geometría).
