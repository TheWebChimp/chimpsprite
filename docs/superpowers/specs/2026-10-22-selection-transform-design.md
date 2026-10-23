# Transformar la selección: escalar y rotar

Fecha: 2026-10-22 · Estado: aprobado

## Contexto

Pendiente #4 del análisis de brechas con Aseprite (`docs/research/2026-10-19-aseprite-gap-analysis.md`).
Parte del bloque "Clipboard + Transform": copiar/cortar/pegar y mover arrastrando dentro de la
selección llegaron en el PR #17, del que sale esta rama.

Estado de partida:
- la selección se mueve (Move, o arrastrando dentro con una herramienta de selección), pero cada
  arrastre se escribe directo en la capa;
- pegar escribe en la capa y se mezcla con lo que hay debajo;
- no hay escala ni rotación.

## Objetivo

Como en Aseprite:
- 8 handles sobre el rectángulo de la selección para **escalar**;
- arrastrar justo por fuera de una esquina para **rotar** sobre el centro;
- el contenido **flota** mientras se transforma; Enter o clic afuera confirma y Esc cancela;
- pegar deja lo pegado flotando;
- rotación **Fast** o **RotSprite**, a elección en la barra de opciones (RotSprite por defecto).

Fuera de alcance: skew, pivote movible, escala con suavizado, transformar en modo Tiles.

## 1. Algoritmos (`cs-raster/src/transform.rs`)

Funciones puras sobre un bloque de píxeles, sin UI. Trabajan sobre `Rgba` para no depender del
modo de color; el editor resuelve índices antes y cuantiza al estampar, como el portapapeles.

```rust
pub struct Block { pub width: u32, pub height: u32, pub pixels: Vec<Option<Rgba>> } // None = fuera de la máscara
pub fn scale_nearest(src: &Block, width: u32, height: u32) -> Block;
pub fn rotate_fast(src: &Block, radians: f32) -> (Block, (i32, i32));     // bloque + corrimiento de su esquina
pub fn rotsprite(src: &Block, radians: f32) -> (Block, (i32, i32));
```

- **Escala:** vecino más cercano por muestreo inverso. Un tamaño negativo (handle arrastrado más
  allá del lado opuesto) se vuelve un tamaño positivo más un volteo.
- **Rotación Fast:** muestreo inverso con vecino más cercano sobre el rectángulo que contiene la
  rotación; el corrimiento mantiene el centro en su lugar.
- **RotSprite:** Scale2x tres veces (×8, con Scale2x/EPX comparando colores exactos), rotación
  Fast a ×8 y reducción ×8 tomando el píxel central de cada bloque de 8×8.
- 0°, 90°, 180° y 270° son exactos en ambas: se resuelven con permutaciones, sin muestrear.
- La máscara viaja con los píxeles (`None`), así una selección no rectangular rota con su forma.

## 2. Estado flotante (`cs-ui/src/transform.rs`)

Un módulo nuevo con `impl Editor`, porque `editor.rs` ya es muy grande.

```rust
pub struct Floating {
    source: Block,                // píxeles levantados, sin transformar
    origin: (i32, i32),           // esquina de la selección original
    rect: (i32, i32, i32, i32),   // destino escalado: x, y, ancho, alto (con signo: volteos)
    angle: f32,                   // radianes, sobre el centro de `rect`
    layer: LayerId, frame: usize, artboard: ArtboardId,
    from_paste: bool,             // sin hueco de origen
}
```

- **Levantar:** al empezar a arrastrar un handle, al mover arrastrando dentro de la selección
  (con Move o con una herramienta de selección) o al pegar. Se levanta una sola vez: lo que
  quede debajo mientras flota no se mezcla ni se lleva en los arrastres siguientes.
  El hueco de origen queda transparente solo en la vista previa. Si viene de pegar, no hay hueco.
- **Vista previa:** `Editor::view()` aplica a un clon del documento el comando que daría el
  commit (hueco + estampado), igual que hoy con los gestos, y muestra la selección transformada.
  Durante un arrastre de rotación se usa Fast aunque la opción sea RotSprite. Al soltar se calcula
  RotSprite una vez y se guarda en caché hasta el siguiente cambio.
- **Confirmar:** un solo `DrawPixels` (borrar el origen y estampar el resultado, sin borrar con
  transparentes) más la selección nueva: la máscara transformada. Un solo paso de undo. Se
  confirma con Enter, con clic fuera de la selección, al cambiar de herramienta, capa, frame o
  artboard, al copiar, cortar o borrar y al guardar o exportar.
- **Cancelar:** Esc, o Ctrl+Z mientras flota. Vuelve todo a como estaba antes de levantar, sin
  pasos en el historial. Si venía de pegar, la selección vuelve a la anterior al pegado.
- **Mover:** arrastrar dentro de la selección levanta el contenido (si no flotaba ya) y solo cambia
  `rect`; la capa no se toca hasta confirmar. Así lo movido, pegado o escalado se puede reacomodar
  varias veces sin llevarse lo que haya debajo (bug reportado el 2026-10-23: el mover de hoy
  estampa en cada arrastre y el siguiente levanta también el dibujo que quedó dentro). Las
  flechas del teclado, si mueven la selección, siguen la misma regla.
- **Límites:** solo capas de imagen, sin bloquear, fuera del modo Tiles y sin editar la máscara,
  igual que el portapapeles. En los demás casos no aparecen handles.

## 3. Interacción en el lienzo

- **Handles:** 8 cuadrados (esquinas y medios de cada lado) sobre el rectángulo que contiene la
  selección, o el rectángulo transformado si flota. Tamaño fijo en pantalla (8 px), sea cual sea
  el zoom. Se muestran con las herramientas de selección y con Move.
- **Escalar:** arrastrar un handle mueve ese borde o esa esquina; el lado opuesto queda fijo. Shift
  en una esquina mantiene la proporción.
- **Rotar:** arrastrar en una franja de 12 px de pantalla fuera de cada esquina. El cursor cambia
  a uno de rotación (una flecha curva dibujada, porque egui no trae cursor de rotar). Shift ajusta
  a pasos de 15°.
- **Cursores:** redimensionar en la dirección de cada handle, mover dentro, rotar fuera de las
  esquinas.
- **Barra de opciones** (herramientas de selección y Move): selector `Rotation: Fast / RotSprite`,
  guardado en `ToolOptions`.
- **Barra de estado** mientras flota: tamaño, ángulo y "Enter to apply · Esc to cancel".

## 4. Errores y casos borde

- Escalar a 0 en un eje no se permite: el mínimo es 1 px.
- Lo que sale del lienzo se pierde al confirmar, como al mover hoy.
- Un bloque transformado de más de 4096×4096 no se calcula: el handle se queda en el último
  tamaño válido.
- Si la capa se borra o se bloquea mientras flota, se cancela.

## 5. Tests

- **Algoritmos:** tamaños de salida; escala ×2 y volteo; 90° y 180° exactos; rotar 0° no cambia
  nada; la máscara rota con los píxeles; RotSprite de una línea a 45° no deja píxeles sueltos.
- **Editor:** levantar y escalar; confirmar es un paso de undo; cancelar no deja rastro; pegar
  flota sobre lo que hay debajo sin mezclarse; mover con contenido flotante; confirmar al cambiar
  de capa; **regresión del bug de mover:** mover la selección encima de otro dibujo, soltar y
  volver a moverla se lleva solo lo seleccionado al principio y el otro dibujo queda intacto.
- **Lienzo** (harness con eventos crudos de egui, como en los tests de renombrar): arrastrar el
  handle de una esquina escala; arrastrar fuera de una esquina rota; Enter confirma.
