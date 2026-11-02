# Modelos 3D, parte 1.5: tabla de shading y Sheet settings

Fecha: 2026-11-02 · Estado: implementado

Referencia: las capturas de picoCAD 2 del usuario (`D:/Work/chimpdeas/.refs/picocad2-reference.png`):
"Palette controls" (la tabla SHADING) y "Sheet settings" (Zoom, Tilt, Height, Ortho, Frames,
Size (px), Export). Sigue a la parte 1 (`2026-11-01-3d-core-design.md`, PR #31).

Decisiones tomadas con el usuario:

- **La tabla de shading es por modelo**, sobre la paleta del modelo.
- **Dos pasos de sombra** (como hoy), **automáticos y editables**: la tabla vacía es la regla de la
  parte 1; solo se guardan las casillas que el usuario fija.
- **El Bake pasa a los Sheet settings de picoCAD con vista previa en vivo.**

## 1. La tabla (`cs-core`, `cs-3d`)

- `Model3D` gana `#[serde(default)] shading: ShadingTable`.
- `ShadingTable { entries: Vec<ShadeEntry> }`, `ShadeEntry { color: Rgba, steps: [Option<Rgba>; 2] }`
  (ordenada por color, sin repetidos; una entrada sin pasos fijados no se guarda). Deriva lo mismo
  que el resto de `Model3D` (sin floats).
- **Paso `k` (1 o 2) de un texel `c`:** si la tabla tiene `steps[k-1]` para `c`, ese color; si no,
  la regla automática: paso 1 = `darker(c)`, paso 2 = `darker(darker(c))` (la de la parte 1). El paso
  2 fijado no depende del paso 1.
- El rasterizador (`cs_3d`) recibe la tabla en `RenderInput` y la usa en la luz; la caché de render
  ya incluye el modelo en su llave, así que cambiar la tabla refresca el render.

## 2. Edición en el panel 3D (sección Light)

- Debajo de la luz (y antes de Outline), una cuadrícula **Shading** como la de picoCAD: arriba los colores de la paleta
  del modelo (los opacos), debajo dos filas con su paso 1 y su paso 2; si no caben en el ancho del
  panel, en bloques de 16 columnas.
- **Clic** en una casilla de sombra: un menú con la paleta del modelo para elegir el color (fija esa
  casilla). **Clic derecho**: la regresa a Auto. Las casillas fijadas llevan una marquita (un punto
  de 1 px en la esquina) para distinguirlas de las automáticas.
- Botón **Auto**: borra la tabla (todo automático).
- Cada cambio es un paso de deshacer (`SetModel`); el render se actualiza en vivo.

## 3. Bake con Sheet settings

El diálogo (antes "Bake") se llama **Sheet settings** y tiene:

- **Frames:** cuántos ángulos, 1..=64 (por defecto 8).
- **Size (px):** el lado de cada frame, 8..=512 (por defecto el lado más largo del modelo); el
  bake renderiza directo a ese tamaño (cuadrado si el modelo es cuadrado; si no, el lado más largo es
  Size y el otro sale de la proporción del modelo, al menos 1: un modelo de 16×256 con Size 64 da
  frames de 4×64).
- **Zoom:** % (por defecto 100 = la cámara del modelo; más es más cerca): la distancia de la cámara
  se divide entre Zoom/100. Rango implementado: 10..=1000, de 10 en 10.
- **Tilt:** grados enteros (por defecto el pitch del modelo), −90..=90 (así los presets Top, a 90°,
  se hornean igual que el render del modelo).
- **Height:** desplazamiento vertical del objetivo de la cámara, en 1/16 de unidad (por defecto 0).
  Rango implementado: −1024..=1024.
- El valor por defecto de Size se limita al rango del stepper (8..=512).
- Export hornea el modelo con el que se abrió el diálogo, aunque otro artboard sea el activo.
- **Ortho:** casilla (por defecto según la proyección del modelo).
- **Animate texture** (si la textura tiene más de un frame) y **Output** Frames / Spritesheet, como
  en la parte 1.
- Ya no tiene presets de cámara (siguen en el panel).
- **Vista previa en vivo:** un cuadro a la derecha con el resultado girando por los ángulos (unos 8
  por segundo), con los valores actuales, a escala entera que quepa.
- **Export:** hornea igual que la parte 1 (artboard nuevo "<modelo> · bake" a la derecha de todo, un
  paso de deshacer, no se activa).

`cs_render::BakeOptions` cambia a `{ frames, size, zoom, tilt, height, ortho, animate_texture,
output }` y `bake_model` construye la cámara desde la del modelo con esos valores.

## 4. Pruebas

- **Tabla:** vacía = render de la parte 1; una casilla de paso 1 fijada cambia los pixeles de las
  caras de 1 paso y no las de 2; Auto la limpia; `serde`/`.chimp` ida y vuelta; un `.chimp` de la
  parte 1 abre con la tabla vacía.
- **Panel:** clic en una casilla + elegir un color fija la entrada (un paso de undo); clic derecho la
  quita; Auto la vacía.
- **Bake:** Frames 12 → 12 frames; Size 48 → frames de 48×48; Zoom 200 hace el modelo más grande que
  con 100; Tilt y Height cambian la imagen; Ortho cambia la proyección; la vista previa existe y
  muestra el frame que toca; Export crea el artboard en un paso.
