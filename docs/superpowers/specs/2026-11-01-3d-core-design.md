# Modelos 3D (tipo picoCAD 2), parte 1: núcleo y sprites

Fecha: 2026-11-01 · Estado: diseño aprobado, falta el plan

## Objetivo

Un modelador 3D low-poly con texturas pixel art dentro de ChimpSprite, en la línea de picoCAD 2.
Primero como medio para hacer sprites (modelar, texturizar y hornear a pixel art), después como
herramienta completa (exportar modelos, GIFs, jerarquía).

El proyecto va en cuatro partes; cada una tiene su spec, su plan y queda usable:

1. **Núcleo y sprites** (este documento): el modelo en el documento, primitivas con UVs
   automáticas, el artboard 3D con cámara, el render pixel con luz de rampas y contorno, y *Bake*
   a artboards (ángulos, cámaras, textura animada, spritesheet).
2. **Modelado:** seleccionar vértices, aristas y caras; mover, rotar, escalar, extruir, dividir,
   unir y borrar, con snapping a la cuadrícula; vistas ortográficas como modo de cámara.
3. **UVs:** editar en el artboard de textura qué pedazo le toca a cada cara (overlay de las caras,
   mover, rotar, voltear, auto-unwrap).
4. **picoCAD completo:** exportar OBJ/glTF con la textura y el GIF girando; importar picoCAD (si
   su formato es abierto); jerarquía y animación de objetos.

Decisiones tomadas con el usuario:

- **El modelo es un artboard 3D en el lienzo** (como las fuentes): un artboard con
  `model: Option<Model3D>` cuyo contenido es el render del modelo. La textura es otro artboard
  normal que se pinta al lado y el modelo se actualiza en vivo.
- **Render por software a la resolución del artboard** (crate nuevo `cs-3d`, Rust puro): pixel art
  exacto, funciona en web (wasm), en pruebas y por MCP.
- **Luz con rampas de la paleta:** una luz direccional oscurece cada cara bajando a los colores
  vecinos más oscuros de la paleta del documento; nunca inventa colores. Se puede apagar.
- **Una textura por modelo, con animación:** todas las caras sacan su pedazo del mismo artboard; si
  tiene frames, el frame N del render usa el frame N de la textura.
- **Bake completo:** vuelta de N ángulos, cámara ortográfica/iso además de perspectiva, textura
  animada × ángulos y salida a frames o a spritesheet.

Fuera de alcance de esta parte: edición de geometría (parte 2), edición manual de UVs (parte 3),
exportar o importar modelos, GIF girando, jerarquía y animación de objetos (parte 4) y comandos MCP
para modelos.

## 1. El modelo (`cs-core`)

`Artboard` gana `model: Option<Model3D>` (con `serde(default)`: los `.chimp` viejos abren igual).

- **Resolución:** el tamaño del sprite del artboard 3D (128×128 por defecto). El sprite tiene una
  capa y tantos frames como su textura (uno si no hay textura o es fija); sus pixeles **no se
  guardan**: los calcula el render (sección 2).
- **`Model3D`:**
  - `objects: Vec<Object3D>`: cada uno con `name`, `mesh: Mesh`, `transform` (posición, rotación
    y escala) y `double_sided: bool`.
  - `Mesh { vertices: Vec<Vec3i>, faces: Vec<Face> }`; `Face` tiene 3 o 4 índices de vértice y,
    por vértice, su coordenada en la textura (UV, en pixeles de la textura).
  - `texture: Option<ArtboardId>`: el artboard de textura.
  - `camera: Camera`: `Perspective { fov }` u `Orthographic`, con `yaw`, `pitch`, `distance` y
    `target`; presets Iso 2:1, Top, Side y Front.
  - `light: Light { enabled, yaw, pitch }`.
  - `outline: Option<Rgba>`: el contorno y su color.
- **Sin decimales** (el `Artboard` deriva `Eq`): coordenadas en enteros de punto fijo (1/16 de
  unidad), ángulos en décimas de grado, escalas en milésimas.
- **Bloqueado como las fuentes:** en un artboard 3D no se pinta ni se agregan o quitan capas o
  frames, ni se cambia el modo de color o el tamaño por los caminos normales (la misma idea que el
  `FontLock`); su contenido sale del modelo.
- **Primitivas** (funciones puras con pruebas): cubo, plano, cilindro, cono y esfera low-poly (con
  número de lados), cada una con UVs automáticas que caben en su zona de la plantilla de textura
  (sección 3).

## 2. El render pixel (`cs-3d`, crate nuevo sin egui)

- **Rasterizador por software** a la resolución del artboard: z-buffer, coordenadas de textura con
  corrección de perspectiva, muestreo *nearest* del frame que toca del artboard de textura. Caras
  de un lado (sin las de atrás), salvo los objetos `double_sided`.
- **Luz con rampas:** cada cara calcula cuánto mira a la luz y baja 0, 1 o 2 pasos; cada paso es el
  color vecino más oscuro de la paleta del documento (la lógica de rampas del ink Shading). Con la
  luz apagada, la textura tal cual.
- **Contorno:** con `outline`, los pixeles del modelo que tocan el fondo llevan 1 px del color.
- **Fondo transparente.**
- **Integración:** `cs-render` compone un artboard 3D renderizando su modelo, así que el lienzo,
  las miniaturas, los **símbolos** (instancias del modelo dentro de otros sprites) y la exportación
  lo ven sin cambios.
- **Caché** por versión del modelo, de la textura y de la cámara: solo se recalcula si algo cambió.
  A 128×128 un modelo low-poly tarda menos de un milisegundo.

## 3. El artboard 3D y el panel del dock (`cs-ui`)

- **En el lienzo:** como cualquier artboard, con su render, y una barra de título de otro color para
  distinguirlo (provisional hasta que el usuario dibuje la suya; las fuentes son naranjas).
- **Cámara sobre el artboard:** arrastrar con **clic derecho o Alt** orbita; **Shift + arrastrar**
  mueve el objetivo; **Ctrl + rueda** acerca la cámara (sin cambiar el zoom del lienzo); clic
  izquierdo elige el objeto bajo el puntero. Mover, rotar y escalar el objeto elegido: desde el panel
  o con **G / R / S** y las flechas.
- **Panel del dock (nombre e ícono provisionales: "3D"):**
  - *Objects:* la lista; **Add** con las primitivas (y su número de lados); del elegido, posición,
    rotación y escala con steppers del kit, *Double sided* y borrar.
  - *Texture:* elegir el artboard de textura entre los del documento, o **New Texture**: un artboard
    de 64×64 con una plantilla donde cada primitiva tiene su zona marcada.
  - *Camera:* perspectiva u ortográfica, presets (Iso 2:1, Top, Side, Front), ángulo e inclinación.
  - *Light & Outline:* dirección de la luz, encendida o no, y el color del contorno.
  - **Bake…**
- **Crear un modelo:** *File → New 3D Model…* (o desde el panel) pide la resolución (128×128 por
  defecto) y crea el artboard 3D con un cubo y su textura nueva con la plantilla, uno al lado del
  otro.
- **Comandos con deshacer** para todo lo anterior (objetos, transformaciones, cámara, luz, textura,
  contorno); orbitar o arrastrar es un solo paso al soltar. El modelo se guarda en el manifest del
  `.chimp`, como la fuente.

## 4. Bake (hornear a sprites)

Diálogo **Bake…**:

- **Ángulos:** 1, 4, 8, 16 o 32; el modelo gira alrededor de su eje vertical en pasos iguales desde
  su rotación actual.
- **Cámara:** la del modelo o un preset (Iso 2:1, Top, Side, Front).
- **Animate texture** (si la textura tiene más de un frame, prendido de fábrica): cada ángulo con
  todos los frames de la textura y sus duraciones.
- **Salida:**
  - *Frames:* un artboard nuevo a la resolución del modelo, un frame por ángulo; con la textura
    animada, cada ángulo es un tag ("0°", "45°", …) con sus frames.
  - *Spritesheet:* un artboard nuevo con una sola imagen en cuadrícula (filas = ángulos,
    columnas = frames de animación).
  - El artboard sale a la derecha del modelo, llamado "<modelo> · bake"; es un artboard normal y
    editable (retocar a mano, exportar PNG/GIF/sheets con lo que ya existe).
- Un solo paso de deshacer; hornear otra vez crea otro artboard.

## 5. Pruebas

- **Modelo y comandos:** cada comando con su undo y redo; primitivas (vértices, caras, UVs dentro de
  su zona); el `.chimp` guarda y lee el modelo, y uno viejo abre sin modelo; el bloqueo de
  estructura.
- **Rasterizador:** imágenes chicas exactas (un cubo de frente da su cara; la luz baja un paso las
  caras de lado; el contorno cierra la silueta; la textura animada usa el frame que toca; caras de
  atrás ocultas salvo `double_sided`).
- **Bake:** número de frames, tags y tamaño del spritesheet.
- **UI:** orbitar cambia la cámara en un paso de undo; agregar primitivas; la textura editada cambia
  el render; el panel y el diálogo.
- **Capturas:** `CHIMPSPRITE_DEMO_MODEL` abre un modelo de ejemplo (una casita) con su textura y el
  panel abierto.
