# ChimpCAD: gizmos de mover, rotar y escalar (como Blender)

Fecha: 2026-11-03 · Estado: diseño aprobado

Sigue a `2026-11-03-chimpcad-panel-tabs-design.md`. Pedido del usuario: "las herramientas de
escalación y rotación que tiene Blender encima de los objetos".

Decisiones tomadas con el usuario:

- **En Persp y en las vistas orto** (Front, Top, Side), **un gizmo a la vez**: Move, Rotate o
  Scale.
- Se elige con **botones en el encabezado** (junto a Model | Texture) o con **W / E / R**.
- Actúa sobre **lo elegido** en cualquier modo de selección.

## 1. Elegir el gizmo

- `ModelEdit` gana `gizmo: Gizmo { None, Move, Rotate, Scale }` (estado de UI; por defecto `Move`).
- En el encabezado de ChimpCAD, en modo *Model*, a la izquierda de Model | Texture: un grupo de
  botones del kit **Move | Rotate | Scale** (el activo encendido; clic en el activo lo apaga →
  `None`).
- Teclas en el workspace (modo *Model*): **W** Move, **E** Rotate, **R** Scale (la misma tecla
  otra vez apaga). G y S modales, las asas, arrastrar y las flechas siguen como hoy. (Extruir, en la
  2B, queda en Ctrl+E.)
- En modo *Texture* no hay gizmos.

## 2. Dónde y cómo se ve

- Aparece en el **centro de la caja de la selección** (el mismo que usan G y S), si hay selección.
- **Colores de eje** (como Blender): X rojo, Y verde, Z azul; el eje bajo el puntero o en uso, en
  amarillo PICO-8 (`#ffec27`). Todo en pixel art: rasterizado a la rejilla de pixeles de la UI
  (`cs_3d::wire`, como las vistas orto), con un tamaño fijo en pantalla (no cambia con el zoom).
- **Persp:** el gizmo completo en 3D, proyectado con la cámara del modelo, dibujado sobre el
  render.
  - Move: tres flechas desde el centro + un cuadrito en el centro (mover en el plano de la vista).
  - Rotate: tres aros (círculos alrededor de cada eje, proyectados; elipses en pantalla).
  - Scale: tres ejes con un cubito en la punta + un cubito en el centro (escala pareja).
- **Front / Top / Side:** solo los dos ejes del plano de la vista (Move y Scale), y en Rotate solo
  el aro del eje de profundidad (el que gira en ese plano), como un círculo.

## 3. Gestos

- Arrastrar una **flecha** mueve lo elegido solo en ese eje del mundo; el cuadrito del centro, en
  el plano de la vista. Con snapping a la cuadrícula (Ctrl = libre), como mover hoy.
- Arrastrar un **aro** gira lo elegido alrededor de ese eje del mundo, por el centro del gizmo: el
  ángulo sale del giro del puntero alrededor del centro en pantalla. **En pasos de 15°**; con
  **Ctrl**, de 1°.
  - Objetos: la rotación del objeto (`transform.rotation`, décimas de grado) — girando en ejes del
    mundo; se compone con la rotación actual y se guarda como ángulos X, Y, Z.
  - Vértices, aristas, caras: las posiciones en el mundo de los vértices giran alrededor del
    centro y vuelven a coordenadas locales (redondeo a 1/16, desde las originales).
- Arrastrar un **cubito de eje** escala en ese eje del mundo; el **cubito del centro**, parejo en
  los tres. El factor sale de la distancia del puntero al centro sobre el eje (en pantalla)
  dividida entre la del inicio; en milésimas (sin snapping). Mismas reglas que las asas de la 2A
  (objeto: eje local más alineado y posición; vértices: respecto al centro; nunca escala 0 en
  objetos).
- Cada arrastre es **un paso de undo** con vista previa (como la 2A); un clic sin arrastrar (menos
  de 3 puntos) no hace nada. Esc o clic derecho durante el arrastre cancela.
- El gizmo gana sobre elegir y sobre la caja de selección cuando el puntero está sobre él (a 4
  puntos de una flecha, un aro o un cubito).

## 4. Código

- `cs-3d`: rotar la selección (`rotate_selection(model, sel, center, axis, angle)`) y la composición
  de rotaciones de objeto en ejes del mundo (`rotate_object_world`), con pruebas; la geometría del
  gizmo (puntos de flechas, aros y cubitos en el mundo para un centro y un tamaño) y su hit-test en
  pantalla dado un proyector, puras y probadas.
- `cs-ui`: el estado (`Gizmo`, el gesto como un `EditOp` nuevo con vista previa), el dibujo en
  Persp y en las orto, los botones del encabezado y las teclas.

## 5. Pruebas

- `cs-3d`: rotar 90° un vértice en (16,0,0) alrededor de Z por el origen da (0,16,0); cuatro veces
  90° = original; rotación de objeto en ejes del mundo (un objeto con Y=45° girado 90° en X del
  mundo) coincide con aplicar las dos matrices; hit-test de flecha, aro y cubito.
- UI: botones y W/E/R cambian el gizmo (y lo apagan); arrastrar la flecha X en Front mueve solo en
  X un paso de undo; arrastrar el aro en Front gira en pasos de 15° (Ctrl de 1°); el cubito X escala
  solo X; el cubito del centro escala parejo; clic sin arrastrar no cambia nada; Esc cancela; el
  gizmo gana sobre la caja de selección; en Persp el gizmo se dibuja sobre el render.
- Captura con el modelo demo y cada gizmo.
