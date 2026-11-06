# Modelos 3D, parte 2A: modo Edit con 4 vistas, seleccionar, mover y escalar

Fecha: 2026-11-02 · Estado: implementado

Notas de la implementación (donde difiere del diseño original):

- **Desplazar las vistas orto:** arrastrar con el botón derecho o el central (Shift queda para
  agregar a la selección).
- **Tab:** en un artboard 3D entra y sale del modo Edit (no muestra la línea de tiempo); egui movía
  el foco con Tab, así que el foco se suelta el frame siguiente. Igual que G, R y S, no exige el
  puntero sobre el lienzo.
- **Selección en las vistas:** en amarillo PICO-8 (`#ffec27`), no el color de selección del skin
  (no se veía sobre el morado).
- **Con un gesto de teclado (G o S) activo**, un clic en cualquier cuadrante confirma (izquierdo) o
  cancela (derecho o central).
- **La perspectiva** usa `cs_render::model_render` al tamaño del cuadrante en pixeles del artboard.
- **Demo:** `CHIMPSPRITE_DEMO_MODEL_EDIT` (con `CHIMPSPRITE_DEMO_MODEL`) abre el modo Edit en
  vértices con uno elegido.
- **Siguiente paso decidido con el usuario:** modelos y fuentes salen del lienzo a la Library y se
  abren en su propia pestaña (spec aparte, antes de la 2B).

Referencia: picoCAD 2 (captura del usuario con las vistas Front, Top y Side en wireframe y la
perspectiva texturizada). Sigue a la parte 1 (`2026-11-01-3d-core-design.md`) y la 1.5
(`2026-11-02-3d-shading-sheet-design.md`).

La parte 2 (Modelado) se parte en dos entregas, cada una con su spec, su plan y su PR:

- **2A** (este documento): modo Edit con las 4 vistas; seleccionar objetos, vértices, aristas y
  caras; moverlos y escalarlos con el mouse.
- **2B:** extruir, dividir, unir y borrar (esbozo en la sección 7).

Decisiones tomadas con el usuario:

- **Modo Edit sobre el artboard:** el área del artboard 3D se parte en 4 cuadrantes; todo vive en
  el lienzo, sin ventanas nuevas.
- **Snapping a una cuadrícula ajustable, Ctrl = libre.**
- **Modos de selección con las teclas 1–4** (Objeto, Vértice, Arista, Cara).
- **Escalar con asas en la caja de la selección y con S + mouse.**

## 1. Modo Edit y las 4 vistas

- **Tab** (con el puntero sobre el lienzo y un artboard 3D activo) o el botón **Edit** del panel 3D
  entra y sale del modo Edit. Es estado de la UI (`Editor`), no del documento: no se guarda ni pasa
  por deshacer. Cambiar de artboard activo a uno que no es 3D lo apaga.
- El área de pixeles del artboard en el lienzo se parte en 4 cuadrantes iguales, acomodados como
  en picoCAD:

  | | |
  |---|---|
  | **Front** (X a la derecha, Y arriba) | **Perspectiva** |
  | **Top** (X a la derecha, Z abajo) | **Side** (Z a la izquierda, Y arriba) |

  Front y Top comparten la X (una sobre la otra, alineadas en pantalla).
- **Perspectiva:** el render pixel normal del modelo (con su cámara, luz, textura y contorno) a la
  mitad de la resolución del artboard (al menos 1×1), dibujado a escala entera en su cuadrante.
  Ahí todo sigue igual que hoy: orbitar (clic derecho o Alt), mover el objetivo (Shift), Ctrl +
  rueda y clic para elegir un objeto.
- **Vistas ortográficas:** wireframe dibujado en pantalla con líneas de 1 px (todas las aristas,
  sin ocultar las de atrás), recortado a su cuadrante. Colores provisionales: fondo morado y
  aristas gris claro como picoCAD; la perspectiva sobre azul marino.
- Las tres vistas comparten **un centro** (`Vec3i`) y **un zoom** (puntos de pantalla por unidad);
  al entrar al modo Edit se encuadra el modelo entero. **Ctrl + rueda** sobre una vista orto cambia
  el zoom de las tres (alrededor del puntero); **arrastrar con el botón derecho o el central** mueve
  el centro en el plano de esa vista (Shift queda para agregar a la selección; en las vistas orto
  no hay nada que orbitar). Centro y zoom son estado de la UI.
- **Guías punteadas** (como picoCAD): en cada vista orto, una cruz de líneas punteadas por el centro
  de la selección, de borde a borde del cuadrante; la vertical de Front y la de Top quedan en la
  misma columna de pantalla.
- La **cuadrícula** de snapping se dibuja tenue en las vistas orto (se omite si sus líneas quedan a
  menos de 4 puntos).
- Cada cuadrante lleva su nombre ("Front", "Top", "Side", "Persp") en una esquina, con la fuente
  pixel de la UI.

## 2. Selección

- **Modos:** 1 Objeto, 2 Vértice, 3 Arista, 4 Cara (teclas en modo Edit y cuatro botones en el
  panel). Cambiar de modo convierte la selección cuando tiene sentido (los objetos elegidos pasan a
  todos sus vértices, aristas o caras; de vértices a aristas y caras se toman las que tienen todos
  sus vértices elegidos; de aristas o caras a vértices, sus vértices).
- **Clic** elige lo que está bajo el puntero (y suelta lo demás), **Shift + clic** lo agrega o lo
  quita, **clic en vacío** limpia, **arrastrar en vacío** hace una caja de selección (con Shift,
  agrega).
- **Qué se atina** (en la vista orto bajo el puntero; si hay empate gana lo más cercano a la vista):
  - Vértice: a 4 puntos o menos.
  - Arista: a 4 puntos o menos del segmento. Las aristas salen de las caras (pares de vértices
    consecutivos, sin repetir).
  - Cara: dentro del polígono proyectado (o a 4 puntos de su centro, que se dibuja como un punto).
  - Objeto: dentro de cualquiera de sus caras.
  - Caja: vértices dentro; aristas y caras con todos sus vértices dentro; objetos con algún vértice
    dentro.
- La selección vive en el `Editor`: `ModelSelection { mode, items }`, con objetos por índice y
  vértices, aristas y caras como `(objeto, índice)` (las aristas como `(objeto, v1, v2)` con
  `v1 < v2`). Al cambiar el modelo (deshacer, panel, otro artboard) se quitan los que ya no existen.
  El objeto del panel (`model_object`) es el primero de la selección cuando hay; elegirlo en el
  panel selecciona ese objeto.
- **Cómo se ve:** los vértices como cuadritos de 3 puntos (los del objeto en modo Objeto, ninguno);
  lo seleccionado (vértices, aristas, puntos de cara y, en modo Objeto, las aristas del objeto) en
  el color de acento de la UI.

## 3. Mover

- Solo en las **vistas ortográficas**. Arrastrar con clic izquierdo algo seleccionado (o algo que
  el clic acaba de elegir) lo mueve en el plano de esa vista; el eje de profundidad no cambia.
- **Modo Objeto:** cambia `transform.position` de cada objeto elegido.
- **Modos Vértice, Arista y Cara:** mueve los vértices involucrados (sin repetir). El desplazamiento
  en el mundo se pasa a coordenadas locales de cada objeto con la inversa de su transform (rotación
  y escala) y se redondea a 1/16; la posición final del vértice en el mundo se calcula desde la
  original para que el redondeo no se acumule durante el arrastre.
- **Snapping:** el punto que se agarró (el vértice bajo el puntero, o el centro de la selección si
  se agarró una arista, una cara o un objeto) salta a la cuadrícula en los dos ejes de la vista; lo
  demás se mueve lo mismo. Cuadrícula: 1/16, 1/8, 1/4 (por defecto), 1/2 o 1 unidad, elegible en el
  panel (estado de la UI). Con **Ctrl** apretado no hay snapping: pasos de 1/16.
- **G** (con el puntero en una vista orto) empieza a mover lo seleccionado sin apretar botón: sigue
  al mouse en el plano de esa vista; clic o Enter confirma, Esc o clic derecho cancela.
- **Flechas:** mueven lo seleccionado un paso de cuadrícula en la última vista orto usada (Front
  por defecto). Fuera del modo Edit siguen como hoy.

## 4. Escalar

- **Asas:** en las vistas orto, la caja de la selección (sus límites proyectados) tiene 8 asas de
  5 puntos: las de las esquinas escalan en los dos ejes de la vista (con **Shift**, el mismo factor
  en los dos); las de los lados, en un eje. El **lado o la esquina opuesta queda fija** (el ancla).
  Con un solo vértice elegido no hay caja ni asas.
- **S** (con el puntero en una vista orto): escala igual en los tres ejes desde el centro de la
  selección; el factor es la distancia del puntero al centro dividida entre la distancia al apretar
  S. Clic o Enter confirma, Esc o clic derecho cancela.
- El snapping de las asas lleva el borde que se arrastra a la cuadrícula (Ctrl = libre); S no tiene
  snapping (factor en milésimas).
- **Modo Objeto:** cambia `transform.scale` en el eje local más alineado con el eje de la vista
  (exacto si el objeto no está rotado) y corrige `transform.position` para que el ancla no se mueva.
  La escala no baja de 1 milésima en valor absoluto; un factor negativo (arrastrar más allá del
  ancla) voltea el objeto.
- **Modos Vértice, Arista y Cara:** las posiciones en el mundo se escalan respecto al ancla y se
  regresan a coordenadas locales (redondeo a 1/16, siempre desde las originales). Arrastrar más allá
  del ancla voltea; si el factor llega a 0 los vértices quedan planos en ese eje (permitido).
- Si el factor no cambia nada (la selección ya es plana en ese eje), el asa no hace nada.

## 5. Undo, panel y código

- Cada arrastre (mover, asa, caja de selección no cuenta), cada G o S confirmado y cada flecha es
  **un paso de deshacer**: `SetModel` agrupado como los arrastres de cámara de hoy
  (`set_model_grouped` + `end_group` al soltar). Cancelar regresa el modelo del principio sin dejar
  paso.
- **Panel 3D:** arriba, el botón **Edit** (encendido en modo Edit), los cuatro botones de modo
  (Object, Vertex, Edge, Face) y un selector **Grid** (1/16, 1/8, 1/4, 1/2, 1), con componentes del
  kit. Los modos y el Grid se ven aunque el modo Edit esté apagado (el modo y el Grid se recuerdan).
- **Código:**
  - `cs-3d/src/edit.rs` (nuevo, Rust puro con pruebas): `OrthoView { Front, Top, Side }` con su
    proyección mundo → plano y de regreso; posiciones en el mundo de los vértices de un objeto;
    aristas de una malla; picks (vértice, arista, cara, objeto, caja); caja de la selección;
    `move_selection`, `scale_selection` (ancla, factor por eje, vista) y `snap`. Usa la misma
    convención del transform que el rasterizador (escala, luego rotación, luego posición).
  - `cs-ui/src/canvas/model_edit.rs` (nuevo): cuadrantes, wireframe, asas, guías, gestos y teclas
    del modo Edit. `model_input.rs` queda para la perspectiva (y para el artboard fuera del modo
    Edit).
  - `cs-ui/src/editor/model_ops.rs`: estado (`model_edit`, `ModelSelection`, centro/zoom de las
    vistas, Grid) y operaciones con undo.
  - `cs-ui/src/model_panel.rs`: los controles nuevos.
- Fuera del modo Edit nada cambia.

## 6. Pruebas

- **`edit.rs`:** proyecciones de las 3 vistas (ida y vuelta); picks con tolerancia y empate por
  profundidad; aristas sin repetir; caja de selección por modo; mover con un objeto rotado y
  escalado (el vértice termina donde se soltó, en el mundo); snapping (sí, Ctrl no); escalar con
  ancla fija, en un eje y en dos, con Shift proporcional, volteo y aplanado; objeto: escala del
  eje más alineado y posición corregida.
- **UI (`layout_tests`):** Tab entra y sale; los cuadrantes y sus nombres; 1–4 cambian de modo y
  convierten la selección; clic, Shift + clic y caja; un arrastre de mover es un solo undo; un asa
  de esquina escala en dos ejes; S y Esc cancela sin paso de undo; G y flechas; el panel.
- **Captura:** `CHIMPSPRITE_DEMO_MODEL` con el modo Edit encendido y un vértice elegido.

## 7. Esbozo de 2B (spec aparte)

- **E:** extruir las caras elegidas (duplica sus vértices, crea los lados y empieza a mover).
- **Dividir arista:** un vértice a la mitad; las caras vecinas se triangulan para respetar 3 o 4
  vértices por cara.
- **M:** unir los vértices elegidos en su centro (quita caras degeneradas).
- **Supr:** borrar vértices, aristas o caras (y lo que quede sin cara).
- Las caras nuevas toman las UVs de sus vecinas.

Fuera de 2A: rotar con el mouse, maximizar una sola vista, editar UVs (parte 3).
