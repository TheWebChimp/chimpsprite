# Herramientas 2: selección completa, Spray, Jumble, Gradient y Texto — Diseño

Fecha: 2026-10-14
Estado: aprobado (el usuario pidió avanzar sin más preguntas)

## 1. Barra de herramientas por grupos (como Aseprite)

Cada fila es un grupo y muestra la última herramienta usada del grupo; el botón `▸` (o clic
derecho) abre las demás. Repetir el atajo de un grupo alterna entre sus herramientas.

| Grupo | Herramientas (atajo) |
|---|---|
| Selección | Rectangular Marquee (M), Elliptical Marquee (Shift+M), Lasso (Q), Polygonal Lasso (Shift+Q), Magic Wand (W) |
| Mover | Move (V) |
| Dibujo | Pencil (B), Spray (Shift+S) |
| Borrar | Eraser (E) |
| Color | Eyedropper (I) |
| Relleno | Paint Bucket (G), Gradient (Shift+G) |
| Línea | Line (L) |
| Figuras | Rectangle (U), Ellipse (Shift+U) |
| Jumble | Jumble (R) |
| Texto | Text (T) |
| Artboard | Artboard (Shift+O) |

## 2. Selección

- `Mask` gana: `from_ellipse`, `from_polygon` (centros de píxel dentro del polígono, regla par-impar),
  `union`, `subtract`, `intersect`, `inverted`.
- Modos: Replace / Add / Subtract / Intersect, elegibles en las opciones. Al presionar: Shift = Add,
  Alt = Subtract, Shift+Alt = Intersect. Shift mantenido después de empezar (sin haberlo usado para el
  modo) restringe a cuadrado/círculo.
- Rectangular y elíptica: arrastrar. Clic sin arrastrar en Replace deselecciona.
- Lasso: trazo libre que se cierra solo al soltar.
- Polygonal Lasso: cada clic agrega un vértice (una línea elástica sigue al cursor); se cierra con clic
  en el primer vértice, doble clic o Enter; Esc cancela.
- Magic Wand: clic; opciones Tolerance (0–255, diferencia máxima por canal), Contiguous y Sample
  (capa actual / todas las capas visibles).
- Menú **Select**: All (Ctrl+A), Deselect (Ctrl+D), Reselect (Ctrl+Shift+D), Inverse (Ctrl+Shift+I).
- Alt solo es cuentagotas temporal en herramientas que pintan.

## 3. Spray

Puntos aleatorios dentro de un círculo de `Spray width` alrededor del cursor, cada uno con el
pincel actual; `Spray speed` controla cuántos por segundo y sigue rociando aunque el cursor no se
mueva. Generador pseudoaleatorio propio (xorshift) sembrado por trazo, así la vista previa es estable.

## 4. Jumble

Pincel que revuelve: cada píxel del trazo toma el valor de un píxel vecino aleatorio (radio = tamaño
del pincel) de la capa tal como estaba al empezar el trazo. Respeta selección y simetría.

## 5. Gradient

Arrastrar del inicio al fin; rellena la selección (o todo el lienzo) de la capa actual del color
primario al secundario. Opciones: forma **Linear** o **Radial**; dithering **None**, **Bayer 2×2**,
**4×4** y **8×8**. Sin dithering interpola RGBA (en indexado usa el color más cercano de la paleta);
con dithering cada píxel es primario o secundario según la matriz de Bayer (el clásico tramado de
pixel art). Shift restringe el ángulo a 45°. Clic derecho invierte los colores.

## 6. Texto

Clic en el lienzo abre un panel flotante con: texto (varias líneas), fuente, tamaño y antialias. La
vista previa se ve en el lienzo; **OK** lo aplica con el color primario como un paso de undo,
**Cancel** o Esc lo descarta. Fuentes: **Pixel 5×7** (integrada, tamaño = escala entera 1–8) y las
vectoriales **Hack** y **Ubuntu Light** (de `epaint_default_fonts`), rasterizadas con `fontdue`;
en escritorio también **Load font…** (.ttf/.otf). Sin antialias cada píxel es todo o nada (cobertura
≥ 50 %); con antialias, en RGBA se mezcla sobre lo existente y en indexado se ignora. Respeta selección.

## 7. MCP

`cs-agent` gana `gradient(x0, y0, x1, y1, from, to, shape, dither, clip)` y `draw_text(x, y, text,
font, size, color, antialias)`.

## Testing

Tests por herramienta en `cs-tools` (máscaras y modos, polígono, varita con tolerancia, spray
determinista dentro del radio, jumble solo usa colores que ya estaban dentro del radio,
gradiente lineal/radial con cada dithering, texto pixel y vectorial), editor (modos con
modificadores, polígono por clics, reselect/inverse) y MCP. Capturas de la app.
