# Herramientas isométricas (Iso Box, Iso Blocks, línea 2:1 y cuadrícula isométrica)

Fecha: 2026-11-06 · Estado: diseño del agente (sin revisar por el usuario)

Pedido: herramientas isométricas para pixel art inspiradas en **Isobox for Aseprite** (darkwark) e
**IsoBuilder** (hasudelfuturo): un generador de cajas isométricas pixel-perfect (2:1), líneas
isométricas limpias, una cuadrícula isométrica en el lienzo y, si encaja, un "builder" sencillo
de bloques apilados. Motor puro (sin egui, wasm) con tests píxel a píxel, comandos serializables
expuestos en el MCP, un paso de undo y UI con el kit.

## 1. Qué hacen las referencias

- **Isobox** (script Lua): pide `X size (Left)`, `Y size (Right)` (en unidades de 2 px: cada
  unidad es 2 px de ancho y 1 de alto) y `Z size (Height)` (en px); colores por cara (arriba,
  izquierda, derecha; de fábrica, tonos del color de frente), color del contorno (negro de
  fábrica) y de "highlight" (líneas interiores claras y un píxel blanco en la esquina del
  frente); dos tipos de esquina, **2 px y 3 px** (el píxel o los píxeles donde se juntan las
  aristas: 2 px = ancho par, 3 px = una columna central). La versión Lite solo dibuja el contorno.
  El autor sugiere "dibujar la de 3 px y espejarla" para tener esquinas de 4 px.
- **IsoBuilder** (extensión de Aseprite): se dibujan las vistas Front, Side y Top y "Render" arma
  la vista isométrica en una capa Preview editable, con colores por cara, con sombras o en
  colores de normal map.
- **Aseprite** no tiene cuadrícula isométrica en una versión publicada; hay un PR en borrador
  (#4818) que agrega `Type: Orthogonal / Isometric` a Grid Settings con Snap, y un issue (#5612)
  que pide 16×8 (2:1) de fábrica. La herramienta Línea de Aseprite con Shift ya pega a 2:1; aquí
  también (`cs_raster::constrain_line`).

## 2. Geometría: `cs_raster::iso` (sin egui, sin `Rgba` en la geometría)

```rust
pub enum IsoCorner { Two, Three, Four }          // ancho de la esquina de arriba (y de abajo)
pub struct IsoBox { pub left: u32, pub right: u32, pub height: u32, pub corner: IsoCorner }
pub enum IsoPart { Top, Left, Right, Outline, Highlight }
pub struct IsoParts { pub width: u32, pub height: u32, pub parts: Vec<Option<IsoPart>> }
pub fn iso_box(shape: &IsoBox, outline: bool, highlight: bool) -> IsoParts;
```

- `left` / `right`: unidades de cada arista de arriba (≥ 1): una unidad = un tramo de 2 px de
  ancho y 1 de alto, como Isobox. `height`: píxeles de las caras de los costados (0 = una baldosa
  plana).
- **La cara de arriba** es un rombo hecho de tramos de 2 px. La arista de atrás-izquierda baja
  `left` filas (un tramo por fila, 2 px a la izquierda por fila); la de atrás-derecha, `right`
  filas. Arriba se juntan según la esquina: con **2 px** comparten el tramo de la punta, con
  **3 px** se enciman un píxel (una columna central) y con **4 px** quedan lado a lado (punta
  plana de 4). Las puntas de los costados son de 2 filas (el tramo de una arista y el de la
  siguiente, uno debajo del otro) y la de abajo repite la de arriba (el rombo gira 180° sobre sí
  mismo).
  Medidas: ancho `2·(left + right) − o` con `o = 2, 1, 0` para 2, 3 y 4 px; alto
  `left + right + height`. Con `left = right = 8`, 4 px y alto 0: la baldosa clásica de 32×16.
- **Los costados**: la silueta es la cara de arriba corrida hacia abajo de 0 a `height` píxeles.
  Lo que no es la cara de arriba es la cara izquierda o la derecha según el lado de la arista
  vertical del frente (la que baja de la punta de abajo del rombo); con 3 px, la columna del
  medio es la arista misma.
- **Contorno** (`outline`): los píxeles de la silueta con algún vecino de los 4 afuera (sale una
  línea de 1 px en los costados verticales y tramos de 2 px en las aristas 2:1, sin escalones
  dobles).
- **Highlight** (`highlight`): el borde de adelante de la cara de arriba (los píxeles de arriba que
  tienen debajo un costado) y, con 3 px, la columna de la arista del frente. Sin highlight, esos
  píxeles son de su cara (la columna central, de la izquierda).
- Tests con dibujos en texto (cada carácter una parte) para cajas chicas de las tres esquinas, y
  propiedades: ancho/alto, simetría izquierda/derecha cuando `left == right`, tramos de 2 px en
  cada fila de las aristas, contorno de 1 px de grueso.

## 3. Colores: `IsoColors`

```rust
pub struct IsoColors { pub top: Rgba, pub left: Rgba, pub right: Rgba, pub outline: Rgba, pub highlight: Rgba }
impl IsoColors { pub fn from_base(base: Rgba) -> Self; pub fn color(&self, part: IsoPart) -> Rgba; }
pub fn paint_parts(parts: &IsoParts, colors: &IsoColors) -> Vec<Option<Rgba>>;
```

- `from_base`: luz de arriba a la izquierda, como Isobox ("tonos del color de frente"): izquierda
  = el color base; arriba, más claro; derecha, más oscura; contorno, muy oscuro; highlight, casi
  blanco. Los tonos corren el matiz como en el pixel art (las luces hacia el amarillo, las
  sombras hacia el azul) en HSV. Test: luminancia `highlight > top > left > right > outline` para
  colores medios; el alfa del base se conserva.
- **Snap to palette**: cada color se cambia por el más cercano de la paleta del sprite (sin el
  transparente). En indexado siempre (el bloque se arma con índices).

## 4. Bloques apilados (el builder): `IsoBlocks`

```rust
pub struct IsoBlocks { pub columns: u32, pub rows: u32, pub heights: Vec<u32>,
                       pub size: u32, pub block_height: u32, pub corner: IsoCorner }
pub fn iso_blocks(blocks: &IsoBlocks, outline: bool, highlight: bool) -> IsoParts;
```

- Una rejilla de `columns × rows` celdas (las columnas siguen la arista de atrás-derecha, las filas
  la de atrás-izquierda) con la altura de cada pila (0 = vacía). Cada bloque es una `IsoBox` de
  `size × size` unidades y `block_height` px; el de la celda `(c, r)` en el nivel `k` va corrido
  `(2·size·(c − r), size·(c + r) − block_height·k)`.
- Se pintan de atrás hacia adelante (orden por `c + r + k`, que alcanza para bloques de una
  rejilla) con sus caras y su highlight; el **contorno** es el de la silueta de todo el conjunto
  (las juntas entre bloques al mismo nivel no se marcan; las de distinto nivel se ven por el
  highlight y el cambio de cara).
- Tests: un bloque solo = `iso_box`; dos bloques apilados = una caja del doble de alto (sin
  highlight); dos lado a lado = una caja del doble de ancho (sin contorno ni highlight); el de
  adelante tapa al de atrás.

## 5. Línea isométrica y cuadrícula

- `cs_raster::constrain_iso(a, b)`: como `constrain_line` pero solo horizontal, vertical y 2:1
  (sin 45° ni 1:2), con los mismos tramos de 2 iguales.
- **Herramienta Línea**: opción nueva **Isometric** en la barra (checkbox del kit, se guarda con
  las opciones). Prendida, la línea siempre pega a 2:1 / vertical / horizontal sin Shift (con
  Shift también). `ToolOptions::iso_lines` → `ToolContext::iso`.
- **Cuadrícula isométrica**: `GridSettings::kind: GridKind { Orthogonal, Isometric }`
  (`serde(default)`, Orthogonal). En Grid Settings, un select **Type** (como el PR de Aseprite);
  elegir Isometric con un tamaño cuadrado lo pasa a 2:1 (16×16 → 16×8). Se dibujan los rombos
  de `size.0 × size.1` con sus vértices en múltiplos de medio tamaño desde el offset (líneas
  `(x−ox)/w ± (y−oy)/h = k + ½`). La geometría (los segmentos dentro del lienzo) es
  `cs_raster::iso_grid_lines`, con tests.
- **Snap to Grid** con la cuadrícula isométrica pega a los vértices de los rombos.
- Se guarda en `.chimp` (en el manifiesto: `.aseprite` no tiene dónde) y viaja en los comandos
  (`Command::SetGrid`). En `.aseprite` queda solo el tamaño y el offset.

## 6. El diálogo Iso Box (Edit → Iso Box…)

```
 Iso Box
 Shape  (•) Box  ( ) Blocks
 ── Box ─────────────────────────────
 Left [ 4 ]  Right [ 4 ]  Height [ 8 px]
 ── Blocks ──────────────────────────
 Columns [3] Rows [3] Size [2] Height [4 px]
 [0][1][2]
 [0][0][1]     clic = +1, clic derecho = −1
 [3][0][0]
 ────────────────────────────────────
 Corner (•) 2 px ( ) 3 px ( ) 4 px
 [x] Shade from [■ base]      (si no, los cinco colores a mano)
 Top [■] Left [■] Right [■]
 [x] Outline [■]   [x] Highlight [■]
 [ ] Snap to palette
 ┌ vista previa ampliada (checker) ┐
 W × H px
                         [Place] [Cancel]
```

- Componentes del kit vía `widgets` (number, radio, checkbox, color_edit) y botones del kit para
  las celdas de la rejilla de alturas (con su número). Sin Pixel UI, los de egui.
- **Shade from**: prendido, los cinco colores salen del base (el primario al abrir) y los botones
  de cada cara los muestran; tocar uno a mano apaga Shade from (los demás quedan como estaban).
- **Place** (o Enter): el resultado queda **flotando** en la capa actual, centrado en el lienzo
  (o en la selección, si hay), seleccionado y con la herramienta Move, como un pegado: se mueve,
  escala o rota con el transform existente y al confirmarse es **un** paso de undo ("Iso Box"),
  sin dejar hueco; Escape lo descarta. Lo transparente no borra lo de debajo.
- Requiere una capa de imagen editable (como pegar); si no, el menú queda deshabilitado.
- Los valores se recuerdan en la sesión (no en `app.ron`); los colores, desde el primario al
  abrir si Shade from está prendido.
- Demo: `CHIMPSPRITE_DEMO_ISO=box|blocks|placed` (abre el diálogo o deja una caja flotando) y
  `CHIMPSPRITE_DEMO_ISO_GRID=1` (cuadrícula isométrica visible).

## 7. MCP y scripts

- `draw_iso_box`: `doc`, `artboard?`, `layer?`, `frame`, `x`, `y` (esquina de arriba a la
  izquierda de la caja), `left`, `right`, `height`, `corner` (2, 3 o 4; 3 de fábrica como
  Isobox), `color` (base) o `top`/`left_color`/`right_color`, `outline`
  (color o `false`), `highlight` (color o `false`), `clip`. Es un `Command::DrawPixels` (un paso).
- `draw_iso_blocks`: igual, con `heights` (filas de alturas), `size` y `block_height`.
- Están en `OPERATIONS` (los scripts Rhai/Lua las llaman igual) y en el servidor MCP.

## Decisiones que el usuario puede revisar

1. **Esquinas 2, 3 y 4 px**: Isobox tiene 2 y 3; la de 4 sale gratis con la misma geometría (es
   la baldosa clásica de 32×16) y es la que piden en los comentarios de Isobox.
2. **Unidades como Isobox**: Left/Right en tramos de 2 px, Height en px. En el diálogo se ve el
   tamaño final en px debajo de la vista previa.
3. **Luz desde arriba a la izquierda**: arriba la más clara, izquierda el color base, derecha la
   más oscura. Para la luz del otro lado se cambian los colores a mano (no hay botón).
4. **Highlight** = borde de adelante de la cara de arriba + (con 3 px) la arista vertical del
   frente; no el "píxel blanco" de Isobox. Sin highlight, la columna central es de la cara
   izquierda.
5. **Contorno** solo de la silueta (las aristas interiores se marcan con el highlight y el
   cambio de color), como Isobox.
6. **Se coloca flotando** (como un pegado) en la capa actual, centrado en el lienzo o en la
   selección; no crea una capa nueva (con Layer → New Layer antes se tiene en capa aparte).
7. **Builder = rejilla de alturas** (pilas de bloques iguales) dentro del mismo diálogo, en vez
   de las tres vistas de IsoBuilder; las juntas entre bloques del mismo nivel no se dibujan.
8. **Línea isométrica** como opción de la herramienta Línea (no una herramienta aparte); incluye
   la horizontal y la vertical.
9. **Cuadrícula isométrica** como tipo de la cuadrícula del sprite (Grid Settings → Type), como
   el PR de Aseprite; se dibuja con líneas finas (no escalonadas) igual que la ortogonal.
10. **Menú**: Edit → Iso Box… (junto a Insert Text), sin atajo ni ícono en la barra de
    herramientas.
11. **Primario negro o transparente**: con Shade from, el diálogo toma el primario como base solo
    si es opaco y no casi negro (algún canal ≥ 40); si no, queda el base de la vez anterior (de
    fábrica, un azul): del negro no salen tonos más oscuros para la cara derecha ni el contorno.
