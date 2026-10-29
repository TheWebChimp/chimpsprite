# Herramientas nuevas (Hand, Contour, Blur) y grupos de la toolbar

Fecha: 2026-10-23 · Estado: aprobado

## Contexto

El usuario definió la toolbar de la UI en pixel art: **solo íconos, sin nombre**. Los íconos
salen de su mockup. Tres de esos íconos son herramientas que no existen todavía: **Hand**,
**Contour** y **Blur**. Además se reorganizan los grupos.

Esta entrega crea las herramientas y los grupos. Funciona con y sin Pixel UI. La toolbar de
íconos va en una entrega aparte, después de los colores.

## Grupos y atajos

Atajos según Aseprite.

| # | Herramienta (variantes) | Atajos |
| --- | --- | --- |
| 1 | Rectangular Marquee (Elliptical Marquee, Lasso, Polygonal Lasso, Magic Wand) | M / Shift+M, Q / Shift+Q, W |
| 2 | **Hand** | H |
| 3 | Pencil (Spray) | B, Shift+S |
| 4 | Eraser | E |
| 5 | Eyedropper | I |
| 6 | Paint Bucket (Gradient, Jumble) | G, Shift+G, Shift+R |
| 7 | Line | L |
| 8 | Rectangle (Ellipse) | U, Shift+U |
| 9 | Text | T |
| 10 | **Contour** | D |
| 11 | **Blur** | R |
| 12 | Move (Slice) | V, Shift+C |
| 13 | Artboard | Shift+O |

- Cambios de atajos: Jumble pasa de R a Shift+R; R queda para Blur.
- `ToolKind::GROUPS` pasa a tener 13 grupos en este orden. La toolbar actual (con nombres y la
  flecha de variantes) los muestra igual.
- **Elegir variantes:** clic derecho, o mantener presionado 400 ms, sobre el botón de un grupo
  abre la lista con los nombres. La flecha ▾ de hoy sigue funcionando. Con esto la futura
  toolbar de solo íconos ya tiene cómo elegir variantes.

## Hand

- Arrastrar con el botón principal desplaza la vista, igual que Espacio+arrastrar o el botón
  del medio hoy. Usa el mismo código del lienzo.
- Cursor: `Grab` en reposo y `Grabbing` mientras arrastra.
- No modifica el documento ni crea pasos de undo.
- No está bloqueada en modo Tiles.

## Contour

Igual que la herramienta Contour de Aseprite.

- **Al dibujar:** se arrastra a mano alzada. La vista previa es el trazo de 1 px que une los
  puntos.
- **Al soltar:** el trazo se cierra uniendo el último punto con el primero, y se rellena el
  polígono resultante, con su contorno incluido.
  - Con el botón principal se usa el color primario y con el secundario, el secundario.
  - El relleno usa el rasterizador de polígonos que ya existe en `cs-raster`.
- **Reglas, como las demás herramientas que pintan:** respeta la selección (recorta), la
  simetría y las capas bloqueadas, y no se usa en modo Tiles.
- **Undo:** todo el trazo es un solo paso.
- **Casos con poca forma:** un clic sin arrastrar pinta un solo píxel. Una línea sin área queda
  como el trazo.
- **Lógica:** va en `cs-tools` (`contour.rs`), sin egui, con tests.

## Blur

Como la tinta Blur de Aseprite.

- **Pincel:** del tamaño de *Brush size*, con la forma del pincel actual.
- **Cada aplicación** (al presionar, y en cada píxel que recorre el arrastre): cada píxel bajo
  el pincel toma el promedio de su vecindario de 3×3.
  - El promedio se calcula sobre los colores resueltos de la capa en ese momento.
  - El color se pondera por alfa y el alfa es el promedio.
  - Los píxeles fuera del lienzo no cuentan.
  - Pasar varias veces suaviza más.
- **Modo indexado:** el promedio se convierte al índice más cercano de la paleta
  (`nearest_index`). Si el alfa del promedio es menor de 128, se usa el índice transparente.
- **Reglas:** respeta la selección (solo cambian píxeles seleccionados) y las capas bloqueadas,
  y no se usa en modo Tiles. No usa la simetría (Aseprite tampoco la aplica al blur).
- **Undo:** todo el trazo es un solo paso, igual que con el lápiz.
- **Lógica:** el promedio es una función pura en `cs-raster` (`blur.rs`). La herramienta va en
  `cs-tools` (`blur.rs`).

## Tests

- **Grupos y atajos:** 13 grupos en el orden de la tabla; H, D, R y Shift+R llevan a la
  herramienta correcta; Artboard tiene su propio grupo.
- **Hand:** arrastrar mueve la vista y no cambia el documento (`doc_version` no cambia).
  - Se prueba con el harness del lienzo y eventos crudos.
- **Contour:**
  - Un trazo en forma de L cerrada rellena el triángulo esperado.
  - El color es el primario o el secundario según el botón.
  - Se recorta a la selección.
  - Es un paso de undo.
  - Un clic pinta un píxel.
- **Blur (función pura):**
  - Un píxel aislado se reparte en su vecindario.
  - Un área de un solo color no cambia.
  - Los bordes del lienzo no cuentan.
  - El alfa se pondera.
  - En indexado da el índice más cercano.
- **Blur (herramienta):** respeta la selección, es un paso de undo y no hace nada en una capa
  bloqueada.
- **Variantes:** clic derecho en el botón de un grupo abre la lista.
  - Se prueba con el harness.

## Pincel: forma redonda o cuadrada y grosor con Ctrl+rueda (agregado el 2026-10-23)

### Forma del pincel
- `cs_raster::BrushShape` tiene dos valores, `Round` y `Square`, y el valor por defecto es `Round`.
- `brush_points(center, size, shape)` devuelve los puntos del pincel:
  - `Square`: igual que el `brush_square` de hoy.
  - `Round`: un círculo relleno de `size` de diámetro, hecho con el mismo rasterizador de elipse
    rellena que usa la herramienta Ellipse (dentro de una caja de `size × size` centrada igual que
    el cuadrado), para que los dos se vean igual.
  - En tamaños 1 y 2 el redondo sale igual que el cuadrado.
- `stamp(points, size)` se convierte en `stamp(points, size, shape)`.
- `ToolContext` gana `brush_shape`, y todas las herramientas que usan pincel lo respetan:
  - Pencil, Spray, Eraser;
  - el contorno de Line, Rectangle y Ellipse;
  - Jumble y Blur.
- **Options:** junto a *Brush size* van dos botones para elegir la forma, Round y Square. Sus
  glifos tienen que existir en las fuentes.
- La forma se guarda con las demás opciones de herramienta. Un ajuste guardado antes de este
  cambio se lee como `Round`.
- La vista previa del pincel en el lienzo (el contorno que sigue al puntero) dibuja la forma
  elegida.

### Grosor con Ctrl+rueda
- Con Ctrl (Cmd en macOS) presionado, la rueda cambia *Brush size* en ±1 por paso, entre 1 y 32.
  - Solo pasa si la herramienta activa usa pincel (`ToolKind::uses_brush`).
  - El slider muestra el valor nuevo al momento.
- Si la herramienta no usa pincel, Ctrl+rueda hace zoom, como hoy.
- La rueda sin Ctrl sigue haciendo zoom.
- **Pasos de la rueda:** los de líneas cuentan uno por evento con `delta.y ≠ 0`. Los de píxeles
  (touchpad) se acumulan hasta sumar 50 px, para que el grosor no salte con cada evento chico.

### Tests
- **`brush_points`:**
  - Round en 1 y 2 da lo mismo que Square.
  - Round en 3 da el círculo esperado y en 5 una forma simétrica sin esquinas.
  - Square sigue igual.
- **Herramientas:**
  - Un clic de Pencil con tamaño 5 Round no pinta las esquinas del cuadrado.
  - Con Square sí las pinta.
- **Ctrl+rueda (harness del lienzo con eventos crudos):**
  - Con Pencil, un paso hacia arriba sube el grosor en 1 y no cambia el zoom.
  - Con Hand o Marquee hace zoom y no cambia el grosor.
  - El grosor no baja de 1 ni sube de 32.
- **Ajustes:** un `ToolOptions` guardado sin la forma se lee como `Round`.
