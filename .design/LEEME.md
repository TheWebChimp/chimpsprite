# Plantillas de diseño de la UI

## font.aseprite: tu fuente

- La grilla tiene celdas de **8×10 px**, 16 por fila. La grilla de Aseprite ya está configurada a ese tamaño.
- Dibuja cada letra en la capa **"Glifos (dibuja aquí)"**, en la celda donde está su referencia azul. Al terminar, oculta la capa "Referencia".
- El orden es ASCII del 32 (espacio) al 126 (`~`), y después: `á é í ó ú ü ñ Á É Í Ó Ú Ü Ñ ¿ ¡ « » € °`.
- **Línea base:** la línea roja tenue de cada celda, en la fila 7 contando desde 0. La parte de abajo de las letras se apoya ahí. Las colas de `g`, `p`, `q`, `y` y `j` bajan a las filas 8 y 9. Las mayúsculas con acento usan la parte de arriba de la celda.
- **Ancho:** cada letra puede ser tan ancha como quieras dentro de su celda. ChimpSprite la mide sola desde el borde izquierdo de la celda hasta el último píxel con tinta, así que la fuente sale proporcional. Pega cada letra al **borde izquierdo** de su celda.
- **Color:** cualquiera. Solo importa si el píxel tiene tinta o es transparente.
- El slice `glyphs` marca la grilla. Su texto guarda los parámetros, y se pueden ajustar:
  - `space=3`: ancho del espacio;
  - `spacing=1`: separación entre letras.
- Si quieres otro tamaño de celda, avísame y regenero la plantilla.

## skin.aseprite: las piezas de la interfaz

- Dibuja en la capa **"Skin (dibuja aquí)"**. Cada rectángulo cuadriculado es el lugar de una pieza, marcado con un **slice con nombre**. Activa View → Slices para ver los nombres.
- Si una pieza necesita otro tamaño, **cambia el slice**: muévelo, agrándalo o achícalo. Lo que importa es el nombre, no la posición.
- **9-slice:** las piezas que se estiran (`window`, `button`, `panel`, `menubar`, `tab`, `tool.slot`…) ya tienen un centro marcado.
  - Las esquinas no se deforman.
  - Los bordes se estiran a lo largo.
  - El centro se estira en las dos direcciones.

  Ajusta el centro con la herramienta Slice si tu diseño lo necesita.
  - En `window`, la franja de arriba, por encima del centro, es la **barra de título**. Hoy mide 13 px.
- **Colores:** los cuadrados de 8×8 de arriba (`color.desktop`, `color.text`, `color.accent`…) ya tienen los colores de tu mockup. Se toma el píxel del centro.
- **Íconos:** `icon.<herramienta>` miden 16×16. `tool.slot` es el marco que va detrás del ícono, con sus variantes `.hover` y `.active`.
- **Variantes:** `.hover`, `.down` y `.active` son opcionales. Si alguna falta, se usa la pieza base.
- Si quieres agregar una pieza que no está, crea un slice nuevo con un nombre descriptivo y avísame.
