# ChimpCAD parte 4: GIF settings (turntable a GIF animado) y FOV

Fecha: 2026-11-06 · Estado: diseño (decidido por el agente, ver "Decisiones que el usuario puede
revisar")

Referencia: el collage de picoCAD 2 (`.refs/picocad2-reference.png`), la ventanita "GIF settings"
(Outline off/color, Scanlines, FOV perspective, botón Render, popup Color) y "Overview" (lista de
objetos con candado y ojo).

## 1. Qué hace

En el panel 3D, en la fila de **Shading…** y **Sheet settings…**, un tercer botón
**GIF settings…** abre un diálogo (mismo estilo que Sheet settings: etiquetas a la izquierda,
steppers del kit, vista previa animada a la derecha) que exporta el modelo girando (turntable)
directo a un archivo, sin crear artboards:

- **GIF animado** (un frame por ángulo, con su duración y loop) o
- **PNG** tira horizontal con los mismos frames (para quien quiera la hoja con los efectos del
  diálogo; la hoja "normal" sigue siendo Sheet settings → Spritesheet, que crea un artboard).

En escritorio pregunta dónde guardar (rfd); en web descarga, como todos los exports
(`Platform::choose_save` + `Platform::write`).

### Campos

Columna **Camera** (los de Sheet settings, mismos rangos):
Frames, Size (px), Zoom (%), Tilt (°), Height, Ortho y **FOV (°)** (10–150, deshabilitado con
Ortho).

Columna **Look**:
- **Render**: `native` (el tamaño de Size, con la proporción del modelo), `nintendo_ds` (256×192),
  `gameboy` (160×144 y la paleta de 4 verdes del DMG), `pico-8` (128×128 y la paleta de 16 de
  PICO-8). En los presets de pantalla el modelo se encuadra en la pantalla (lo de la cámara igual
  que Size en native, con el cuadro de la proporción del modelo centrado) y el render llega hasta
  los bordes; Size se deshabilita.
- **Scale**: escala entera 1–8 (vecino más cercano).
- **Outline**: casilla + color (de entrada el contorno del modelo; apagado = sin contorno, aunque
  el modelo tenga uno). Reusa el contorno de `cs_3d::render_model`.
- **Scanlines**: casilla + intensidad (%) 10–100: oscurece las filas impares de la imagen final
  (ya escalada), solo en pixeles opacos. A escala 2+ queda el efecto CRT (una fila oscura por
  fila de pixel).
- **Background**: casilla + color; apagado = transparente.
- **Frame (ms)**: 10–1000, de 10 en 10 (GIF guarda centésimas).
- **Loop**: infinito o una sola vez.
- **Format**: GIF / PNG.

Vista previa: el frame del ángulo actual, girando a la duración elegida; a la escala elegida si
cabe, si no a la mayor que quepa (así se ven las scanlines). Debajo, el tamaño final
("256×192 ×2, 8 frames").

## 2. Motor

- `cs_render::BakeOptions` gana `fov: i32` (décimas de grado): el de la cámara del modelo o 60°.
  `camera()` lo usa en perspectiva. Sheet settings no lo muestra (queda el del modelo).
- `cs_render::turntable` (nuevo, `crates/cs-render/src/turntable.rs`):
  `TurntableOptions { bake, outline, scanlines: Option<u8>, preset: RenderPreset, scale,
  frame_ms, background, looping }`, `turntable_frame(doc, model, &o, angle, scale)` y
  `turntable(doc, model, &o) -> Turntable { width, height, frames }`. Pipeline por frame:
  render (con el contorno elegido) → paleta del preset (gameboy: por luminancia en 4 niveles;
  pico-8: el más cercano) → fondo → escala → scanlines.
- `cs_io::export::encode_gif(width, height, frames, delay_ms, looping)`: lo de `export_gif`
  sacado a una función que recibe pixeles RGBA; `export_gif` la usa. Mismo crate `gif` (puro
  Rust, compila en wasm). La tira PNG se arma con `Bitmap::blit` y `to_png`.

## 3. FOV en la vista Persp

- La fila de la cámara del panel: el select de proyección ofrece `Perspective 30°/45°/60°/90°` y
  `Orthographic`; lo mostrado dice el FOV actual ("Perspective 60°").
- En la vista **Persp** del workspace: **Alt + rueda** cambia el FOV de 5 en 5 (10–150°), un paso
  de undo por ráfaga, como la rueda de la cámara; el rótulo del cuadrante dice "Persp 60°".

## 4. Overview (outliner)

El outliner ya tiene lo de la Overview de picoCAD 2 (lista de objetos, candado, ojo, elegir con
clic) y más (grupos, renombrar, arrastrar, double sided). No se agrega nada. Los objetos ocultos
no salen en el GIF (el render ya los salta).

## Decisiones que el usuario puede revisar

1. **Exporta a archivo, no crea artboard**: el GIF no pasa por el documento. Si quieres editar
   los frames, Sheet settings sigue horneando a un artboard.
2. **PNG = tira horizontal** de los frames procesados (no una rejilla ni JSON).
3. **Presets**: native, nintendo_ds 256×192, gameboy 160×144 (4 verdes DMG #0f380f #306230
   #8bac0f #9bbc0f, por luminancia), pico-8 128×128 (16 colores, el más cercano). La paleta se
   aplica antes de las scanlines: con scanlines el GIF tiene los tonos oscurecidos además.
4. **Scanlines a la resolución final** (después de Scale), filas impares, solo pixeles opacos.
5. **FOV** 10–150°; en el editor con Alt+rueda en Persp y con presets en el select (no hay
   stepper nuevo en el panel para no mover el layout del boceto).
6. **Textura animada**: el GIF usa el frame 0 de la textura (como la vista previa de Sheet
   settings). Anotado en minors.
7. Las opciones del diálogo no se guardan en el modelo (se recalculan al abrir desde la cámara y
   el contorno del modelo).
8. Nombre sugerido: `<modelo>.gif` / `<modelo>.png`.
