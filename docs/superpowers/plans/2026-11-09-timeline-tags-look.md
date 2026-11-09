# Plan: timeline, etiquetas de tags y bordes de la tabla

Spec: `docs/superpowers/specs/2026-11-09-timeline-tags-look-design.md`.

1. **Harness de pruebas:** juntar los rects lisos (con su color y recorte) del último frame y
   `color_at(x, y)` (el último rect que cubre el píxel): permite revisar líneas pixel a pixel.
2. **Margen izquierdo (TDD):** test de que la columna de la izquierda de la tabla es brillo, la
   línea oscura del ojo está 16 más allá y la fila elegida la pinta en rosa. Código: margen de 1
   a la izquierda en la grilla Pixel UI; `row_lines` dibuja el brillo de la izquierda y extiende
   las horizontales; `select` trata la primera celda con la columna del borde.
3. **Línea de arriba del encabezado (TDD):** test de la oscura en `B−2` y el brillo en `B−1` con
   tags. Código: `pixel::tags_floor` desde `tags_row`.
4. **Etiqueta y corchete (TDD):** tests de contorno, esquinas recortadas, tramo de arriba y patas
   (también un tag de un frame). Código: `pixel::tag` reescrito.
5. Captura `--demo` alineada con el mockup: imagen mockup | app | diferencias.
6. Verificación completa (tests, clippy, fmt, wasm), autorrevisión, rebase si main avanzó.
