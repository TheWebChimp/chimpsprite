# Modelos 3D, parte 3: modo Textura (UVs) en ChimpCAD

Fecha: 2026-11-02 · Estado: diseño aprobado

Referencia: la captura de picoCAD 2 del usuario (modo textura: el modelo arriba con la cara
elegida, la textura abajo con la paleta y el contorno blanco de la cara). Sigue a la parte 2A
(`2026-11-02-3d-modeling-edit-design.md`) y a las pestañas (`2026-11-02-cad-font-tabs-design.md`).

Decisiones tomadas con el usuario:

- **Dos modos en la pestaña del modelo:** *Model* (las 4 vistas de hoy) y *Texture*.
- **Texture = modelo arriba, textura abajo** (como la captura de picoCAD; las capturas mandan en el
  acomodo).
- **El editor de abajo es la textura completa con el contorno UV de las caras elegidas.** Se pinta
  con las herramientas de siempre.
- **Más resolución = agrandar el pedazo de textura de la cara;** si no cabe, se agranda la textura.

Fuera de alcance: pintar directo sobre el modelo 3D, auto-unwrap (acomodar todas las caras solas),
varias texturas por modelo.

## 1. Los dos modos

- `ModelEdit` gana `mode: CadMode { Model, Texture }` (estado de UI por modelo, como la selección;
  al abrir un modelo nuevo, *Model*).
- Dos botones de texto del kit arriba a la derecha del área del workspace, **Model** y
  **Texture** (el activo se ve encendido), como los íconos de picoCAD. Iconos provisionales hasta que
  el usuario dibuje los suyos.
- Cambiar de modo no toca el documento ni el undo. La selección de caras se comparte: en *Texture*
  la selección está siempre en modo Face (al entrar se convierte como al apretar 4).

## 2. El modo Texture: el acomodo

- El área del workspace (a la izquierda del panel abierto) se parte **horizontalmente**: arriba el
  modelo, abajo la textura; la línea divisoria a la mitad en puntos enteros, de 1 punto.
- **Arriba (modelo):** el render pixel del modelo (como Persp: resolución del artboard, escala
  entera más grande que quepa, centrado) sobre el fondo azul marino. Orbitar (derecho o Alt),
  mover el objetivo (Shift) y Ctrl + rueda como en Persp.
  - **Clic en una cara** la elige (y deja las demás); **Shift + clic** la agrega o la quita;
    clic en vacío vacía la selección. `cs_3d` gana `pick_face(input, x, y) -> Option<(usize,
    usize)>` (objeto, cara) junto a `pick_object`.
  - Las caras elegidas se marcan sobre el render con su contorno de 1 px en amarillo PICO-8
    (`#ffec27`), en pixeles del render.
- **Abajo (textura):** el artboard de textura del modelo, a escala entera, con la cuadrícula de
  pixeles y los colores de la paleta del documento (la paleta del panel de colores de siempre).
  - **Pintar:** las herramientas de siempre (lápiz, borrador, relleno, línea, rectángulo, gotero,
    selección…) pintan en la textura, con su undo normal. El modelo de arriba se actualiza en vivo.
  - **Zoom y desplazamiento** de la textura: rueda (zoom anclado al puntero), barra espaciadora o
    botón central para desplazar, como el lienzo; al entrar encuadra la textura en el área de abajo.
  - Sin textura asignada: un aviso "No texture" y el botón **New Texture** (el del panel 3D).
- Los atajos de pixeles (Ctrl+V, Delete, Fill…) en *Texture* actúan sobre la textura; en *Model*
  siguen bloqueados como hoy.

## 3. Las UVs de las caras elegidas (abajo)

- Cada cara elegida dibuja su **contorno UV** (el polígono de sus `uvs`, en pixeles de la textura)
  en blanco con un borde oscuro de 1 px para que se lea sobre cualquier color; sus vértices UV como
  cuadritos de 3 px.
- La **caja** de todas las UVs elegidas lleva 8 asas (como la 2A).
- **Gestos** (con la herramienta Move, o con cualquier herramienta si el puntero está sobre un asa
  o dentro de un contorno con **Alt** apretado — ver 3.1):
  - Arrastrar **dentro de un contorno**: mueve las UVs de las caras elegidas (en pixeles enteros).
  - Arrastrar un **asa**: escala las UVs respecto al lado o esquina opuesta (Shift = proporcional),
    redondeando a pixeles enteros. Agrandar = más pixeles en la cara.
  - Arrastrar un **vértice UV**: mueve solo ese vértice (de esa cara).
  - Cada gesto es un paso de undo (`SetModel` con vista previa, como la 2A).
- **Botones** en el panel 3D (sección *UV*, visible en modo *Texture* con caras elegidas):
  **Rotate 90°** (gira las UVs de cada cara elegida un cuarto de vuelta alrededor del centro de su
  caja), **Flip H**, **Flip V**. Un paso de undo cada uno.
- Las UVs pueden salir de la textura (se repite, como hoy el muestreo con `rem_euclid`); no se
  recortan.

### 3.1 Quién gana: pintar o mover UVs

- Con la herramienta **Move** (V), los gestos de UV de arriba.
- Con cualquier otra herramienta, el clic pinta; para mover UVs sin cambiar de herramienta se
  mantiene **Alt** (el cursor cambia a mover sobre un contorno o asa).

### 3.2 Pintar solo la cara elegida

- Con caras elegidas, solo se pinta dentro de sus contornos UV: la selección de pixeles de la
  textura es la unión de esos polígonos (las herramientas ya respetan la selección). Sin caras
  elegidas se pinta libre.
- Al elegir caras con un clic arriba, la textura de abajo se acerca y se centra en su pedazo.

## 4. Tamaño de la textura

- En la sección *UV* del panel 3D, **Texture size**: un select con 16, 32, 64, 128, 256 y 512 (el
  lado; la textura es cuadrada o se agranda en los dos ejes por igual).
- Cambiarlo redimensiona el artboard de textura anclado arriba a la izquierda (`ResizeCanvas`): lo
  pintado se queda donde estaba, el espacio nuevo es transparente; al achicar se recorta. Las UVs
  no cambian (siguen en los mismos pixeles). Un paso de undo.
- Achicar con UVs fuera del tamaño nuevo pide confirmación ("Some faces use pixels outside the new
  size").

## 5. Cómo encaja con el editor

- En *Texture*, el `Editor` pinta en la textura: `active` = el artboard de textura, y el workspace
  sigue siendo el modelo (`workspace` = modelo). `ensure_valid` acepta como activo la textura del
  modelo del workspace mientras el modo sea *Texture*. Volver a *Model* activa otra vez el modelo.
  `model_edit_on()` vale en los dos modos (es "estoy en la pestaña del modelo"); las funciones que
  necesitan el modelo lo toman del workspace.
- El lienzo, en *Texture*: arriba dibuja el render del modelo del workspace; abajo dibuja la textura
  con el camino normal del lienzo (un solo artboard, como el workspace de la fuente) dentro del
  rectángulo de abajo, con su propio zoom y desplazamiento guardados en el `ModelEdit`.
- Las ediciones de UV son funciones puras nuevas en `cs-3d/src/uv.rs` (mover, escalar con ancla,
  mover un vértice, rotar 90°, voltear, caja de las UVs elegidas), con pruebas.

## 6. Pruebas

- **`cs-3d`:** `pick_face` (cara de enfrente del cubo; vacío fuera); las operaciones de UV
  (mover, escalar con ancla y redondeo, vértice, rotar 90° cuatro veces = igual, voltear dos veces =
  igual, caja).
- **Editor:** cambiar de modo no deja undo; en *Texture* el activo es la textura y pintar un pixel
  cambia la textura y el render del modelo; volver a *Model* activa el modelo; un gesto de UV es un
  paso de undo; Texture size agranda sin perder pixeles y es un paso de undo.
- **UI (`layout_tests`):** los botones Model/Texture; el acomodo arriba/abajo cubre el área a la
  izquierda del panel; clic en una cara del render la elige y su contorno aparece abajo; arrastrar
  dentro del contorno (con Move) mueve las UVs; Alt + arrastrar con el lápiz mueve en vez de pintar;
  Rotate 90° y Flip; Texture size.
- **Captura:** `CHIMPSPRITE_DEMO_MODEL_TEXTURE` abre la casita en modo *Texture* con la cara de
  enfrente elegida.
