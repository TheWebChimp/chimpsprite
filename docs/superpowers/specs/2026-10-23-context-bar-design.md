# Toolbar a todo el alto y barra de opciones arriba

Fecha: 2026-10-23 · Estado: aprobado

## Contexto

Hoy el panel izquierdo de herramientas tiene arriba la toolbar y abajo las secciones Options y
Symmetry. La línea de tiempo (panel inferior) pasa por debajo de él, así que la toolbar ocupa
solo la parte de arriba de la ventana.

El usuario quiere el acomodo de Aseprite:
- La toolbar va a todo el alto, desde debajo de la barra de opciones hasta encima de la barra de
  estado.
- Las opciones de la herramienta activa pasan a una franja horizontal debajo de las pestañas.
- La barra de estado sigue a todo el ancho, abajo de todo.

## Acomodo, de arriba hacia abajo

1. Menú (sin cambios).
2. Pestañas (sin cambios).
3. **Barra de opciones (nueva):** a todo el ancho.
4. **El cuerpo,** de izquierda a derecha:
   - **toolbar**, a todo el alto del cuerpo;
   - **el centro:** el lienzo y, debajo, la línea de tiempo, que ahora empieza a la derecha de
     la toolbar;
   - **el panel derecho:** Color, Palette, Artboards, etc., sin cambios. Va a la altura que tenga
     hoy respecto a la línea de tiempo.
5. Barra de estado (sin cambios), a todo el ancho.

En egui:
- Los paneles `top` del menú, las pestañas y la barra de opciones se declaran antes que los
  laterales.
- La barra de estado (`bottom`) también se declara antes que los laterales, para que quede a
  todo el ancho.
- El panel izquierdo de la toolbar se declara antes que la línea de tiempo, para que la línea de
  tiempo quede a su derecha.
- Hay que respetar el orden actual del panel derecho respecto a la línea de tiempo, para que el
  panel derecho no cambie de forma.

## Toolbar

- Solo lleva los botones de grupo, nada debajo. Mantiene el scroll que solo responde a la rueda
  y a la barra.
- **Ancho con Pixel UI encendido:** el del marco `tool.slot` más 4 puntos de margen a cada lado.
  Se calcula desde la pieza, no con un número fijo.
- **Ancho con Pixel UI apagado:** la toolbar con nombres sigue a 184 de ancho, pero sin Options
  ni Symmetry.

## Barra de opciones (`panels::tool_options_bar`)

- **Forma:** una sola fila horizontal con las opciones de la herramienta activa, las mismas que
  hoy hay en `tool_options`:
  - Brush size, la forma Round/Square, Pixel-perfect;
  - el modo de selección y las opciones de Magic Wand;
  - las de Text, Gradient, Bucket, etc.;
  - el texto de ayuda que hoy aparece al final ("Alt+click: pick color", etc.).
  
  Se reacomodan en horizontal: etiqueta y control uno al lado del otro, con un separador entre
  grupos de opciones.
- **Symmetry** va al final de la misma fila, alineado a la derecha, con sus botones de siempre.
- **Altura fija:** igual para todas las herramientas, para que el lienzo no se mueva al cambiar
  de herramienta. Una herramienta sin opciones (Hand) deja la barra vacía, pero visible.
- **Si no cabe:** la fila hace scroll horizontal con la rueda o la barra, sin arrastre. No se
  parte en dos renglones.
- **Pantallas sin documento (Home):** la barra no se muestra, igual que hoy no se muestran las
  herramientas.
- **Modo Tiles:** muestra lo que hoy muestra `tool_options` en ese modo.

## Tests

- **Orden de los paneles** (harness con la app completa o con el layout):
  - El rectángulo de la toolbar llega más abajo que el borde de arriba de la línea de tiempo.
  - La toolbar no se encima con la línea de tiempo.
  - La barra de opciones está entre las pestañas y el lienzo.
  - La barra de estado ocupa el ancho de la ventana.
- **Contenido:**
  - Con el Pencil, la barra muestra Brush size y la forma.
  - Con Hand, ninguna de las dos.
  - Symmetry siempre está.
  - La toolbar ya no muestra "Options" ni "Symmetry".
- **Ancho con Pixel UI:** la toolbar mide el ancho del marco más el margen.
