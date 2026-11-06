# Edit → FX → Outline como el de Aseprite

Fecha: 2026-11-06 · Estado: diseño del agente (sin revisar por el usuario)

Pedido: que el Outline de Edit → FX sea como el de Aseprite 1.3 (Background Color, presets de
forma y la matriz de 3×3, Place, canales, destino, Preview, Tiled y OK / Apply / Cancel), con la
lógica pura en un crate de motor, comparada píxel a píxel contra Aseprite y pensada para
reutilizarse en las variantes outline/shadow de las fuentes pixel (FontForge parte 4).

Hoy: `Filter::Outline { color, square }` en `crates/cs-ui/src/filters.rs`, solo afuera, con 4 u 8
vecinos y con lo de fuera del lienzo como transparente.

## 1. Cómo lo hace Aseprite (medido con Aseprite 1.3.18 en batch)

`crates/cs-raster/tests/fixtures/outline_aseprite.lua` corre `app.command.Outline` sobre casos
fijos y 40 al azar y escribe `outline_aseprite.txt` (entrada y salida en texto). Lo que sale:

- **Vacío** = alfa 0 **o** exactamente el color de fondo (Background Color; en Mask, ninguno).
- **La matriz**: 9 bits por filas desde arriba a la izquierda (bit = `(dy+1)*3 + (dx+1)`); el
  centro (bit 4) no cuenta. Presets (octal, como Aseprite): Circle `0252` (los 4 de los lados),
  Square `0757` (los 8), Horizontal `0050` (izquierda y derecha), Vertical `0202` (arriba y abajo).
- **Outside**: se pinta cada píxel vacío con **algún** vecino de la matriz no vacío.
  **Inside**: cada píxel no vacío con algún vecino de la matriz vacío.
- **Bordes del lienzo**: sin Tiled, el vecino de afuera es el píxel del borde más cercano
  (coordenadas recortadas: así, Inside no marca el borde del lienzo y Outside no "ve" nada
  afuera). Con Tiled (`x`, `y` o `both`), el eje envuelve al otro lado.
- **Canales**: el píxel pintado toma de `color` solo los canales elegidos (R, G, B, A); los demás
  quedan como estaban. Un vacío (0,0,0,0) con solo RGB sigue invisible; con solo A queda negro.
- **Se reemplaza**, no se mezcla: un color con alfa 128 deja el píxel con alfa 128.
- **Selección**: solo cambian los píxeles seleccionados; los vecinos de afuera de la selección
  igual cuentan.

## 2. Motor: `cs_raster::outline` (sin egui, sin `Rgba` obligatorio)

```rust
pub struct OutlineMatrix(u16);            // 9 bits; el centro siempre apagado
impl OutlineMatrix {
    pub const CIRCLE / SQUARE / HORIZONTAL / VERTICAL;
    pub fn from_bits(u16) -> Self;  pub fn bits(self) -> u16;
    pub fn get(self, dx, dy) -> bool;  pub fn toggled(self, dx, dy) -> Self;
    pub fn preset(self) -> Option<OutlinePreset>;   // None = custom
}
pub enum OutlinePreset { Circle, Square, Horizontal, Vertical }  // ALL, label(), matrix()
pub enum OutlinePlace { Outside, Inside }
pub struct OutlineShape { pub matrix, pub place, pub wrap_x: bool, pub wrap_y: bool }

/// Dónde va el borde: un `Vec<bool>` de `width × height` (por filas).
pub fn outline_mask(width, height, empty: impl Fn(i32, i32) -> bool, shape: &OutlineShape) -> Vec<bool>;

pub struct Channels { r, g, b, a }                 // se mueve desde cs-ui (Color Curve lo usa)
pub fn paint_channels(pixel: Rgba, color: Rgba, channels: Channels) -> Rgba;
```

- `outline_mask` es la función pura que reusan las fuentes: un glifo es una máscara (vacío =
  sin tinta); el outline de 1 px de una fuente es `outline_mask` con Outside, y uno de N px es
  repetirlo; la sombra es la máscara corrida (no es parte de esta tarea).
- Todo con `serde` (los comandos y el MCP podrán describir un outline).
- Tests: unitarios de la matriz y los presets; y `tests/outline_aseprite.rs`, que lee el
  fixture y compara cada caso píxel a píxel.

## 3. El filtro en cs-ui

- `Filter::Outline(OutlineFx)` con `OutlineFx { color: Rgba, background: Rgba (alfa 0 = Mask),
  shape: OutlineShape, channels: Channels }`. `Filter::apply` deja de servir para Outline (mira la
  imagen entera): `Editor::apply_filter` calcula la máscara del cel y pinta los seleccionados.
- **Target** (`FilterTarget`): `Selected` (el rango de cels del timeline, o el cel actual) o
  `All` (todos los cels de las capas de imagen visibles y sin candado), como el botón Target de
  Aseprite. Todo en **un** paso de undo (`Command::Batch` de `DrawPixels`), calculado con lo de
  antes (los cels enlazados reciben lo mismo dos veces, sin sumarse). La selección del lienzo
  vale para todos los cels.
- `Editor::apply_filter_to(filter, target)`; `apply_filter` sigue siendo el del cel actual.

## 4. El diálogo (Pixel UI, componentes del kit)

```
 Color [■]   Background [▦]          (color_button del kit; Background con alfa 0 = Mask)
 Shape [Circle|Square|Horizontal|Vertical]   (grupo de botones; ninguno prendido = custom)
 ┌─┬─┬─┐
 │ │■│ │   Place (•) Outside   ( ) Inside       (radios del kit)
 ├─┼─┼─┤
 │■│·│■│   la matriz: 9 botones del kit de 17×17; prendido = presionado con un cuadrito del
 ├─┼─┼─┤   color del borde; el del centro, deshabilitado (es el píxel)
 │ │■│ │
 └─┴─┴─┘
 Channels [x]R [x]G [x]B [x]A
 Target  [Selected Cels ▾]
 [x] Preview   [x] Tiled
                         [OK] [Apply] [Cancel]
```

- Elegir un preset llena la matriz; tocar una celda la cambia y el preset queda en custom
  (ningún botón del grupo prendido).
- **Preview** (prendido de entrada): el resultado en el lienzo mientras se cambian los valores,
  como los otros filtros (se aplica y se deshace). Apagado, el lienzo queda como estaba hasta OK
  o Apply.
- **Tiled**: envuelve en los bordes. De entrada, prendido si View → Tiled Mode está prendido (y
  en sus ejes); al prenderlo a mano, en los ejes de Tiled Mode o, si está apagado, en los dos.
- **OK** (o Enter): deja lo aplicado (un paso) y cierra. **Apply**: lo deja (un paso) y sigue
  abierto; el lienzo ya muestra el resultado y no se vuelve a previsualizar hasta cambiar algo;
  Apply otra vez sin cambios aplica de nuevo (un borde más grueso). **Cancel** / Escape / ×:
  quita la vista previa (lo ya aplicado con Apply queda).
- Los demás filtros ganan el botón Apply con la misma lógica (es el mismo diálogo).
- Los valores (menos el color) se recuerdan mientras la app está abierta: al abrir de nuevo,
  la última forma, place, fondo, canales y target; el color, el primario.
- Demo: `CHIMPSPRITE_DEMO_FILTER=outline` abre el diálogo.

## Decisiones que el usuario puede revisar

1. **Bordes del lienzo como Aseprite** (recortar al borde): antes lo de afuera era transparente,
   así que un sprite que tocaba el borde ahora no recibe outline del lado del borde con Inside y
   tampoco "ve" nada fuera del lienzo. Es lo que hace Aseprite con un cel del tamaño del lienzo.
2. **Presets como grupo de texto** (Circle / Square / Horizontal / Vertical) en vez de los
   íconos de Aseprite: no hay íconos de esas formas en el skin; con texto es del kit sin piezas
   nuevas. Si los quieres como íconos, se dibujan y se cambia `GroupItem::text` por `icon`.
3. **Target "All Cels"** = todos los cels de las capas de imagen visibles y sin candado (como
   el "all cels" de Aseprite); las capas tilemap, grupos y ocultas se saltan.
4. **Background Color en Mask** se marca con alfa 0 en el botón de color (checker), sin
   un botón "Mask" aparte.
5. **Apply** no existe en el Outline de Aseprite (solo OK / Cancel); se agregó porque lo pidió el
   encargo, con la semántica de arriba (aplicar y seguir; no previsualizar de nuevo hasta un
   cambio).
6. **Recordar los valores** solo en la sesión (no en `app.ron`); el color siempre el primario.
7. En **indexado**, los canales se aplican sobre el color y se busca el índice más cercano (como
   el resto de los filtros); Aseprite en indexado solo ofrece el canal Index.
