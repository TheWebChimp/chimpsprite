# Modelos 3D, parte 2B: extruir, dividir, unir y borrar

Fecha: 2026-11-03 · Estado: implementado

Notas de la implementación: en modo Objeto, Extrude, Subdivide y Merge están deshabilitados (Delete
borra los objetos); durante un Extrude no cambian la selección, el modo, G/S ni las flechas; al
deshacer o rehacer una operación que cambia cuántos vértices o caras tiene un objeto, su selección se
vacía (no se restaura); dividir no parte aristas cuyo punto medio cae en un extremo.

Sigue a la 2A (`2026-11-02-3d-modeling-edit-design.md`), a las pestañas de ChimpCAD y a los gizmos
(`2026-11-03-chimpcad-gizmos-design.md`).

Decisiones tomadas con el usuario:

- **Teclas y botones:** atajos y una fila de botones en la pestaña del objeto que se activan según
  lo elegido.
- **Dividir = caras en 4** (Subdivide como Blender).
- **Extruir: sale y la mueves** (pegada al mouse por su normal; clic confirma, Esc cancela sin dejar
  nada).

## 1. Las operaciones (`cs-3d`, puras y probadas)

- **Extrude** (caras elegidas): duplica los vértices de las caras elegidas, mueve las caras a los
  vértices nuevos y crea una cara de lado (cuadrilátero) por cada arista del borde de la región
  elegida (las aristas que comparten dos caras elegidas no generan lado). Las caras nuevas de lado
  toman las UVs de la cara de la que salen (el pedazo de la arista, estirado), para que se vean con
  textura. Devuelve el modelo nuevo y la selección nueva (las caras extruidas).
- **Subdivide** (caras elegidas): cada cara elegida se parte en 4: un cuadrilátero en 4
  cuadriláteros (vértices a la mitad de cada arista + el centro), un triángulo en 4 triángulos. Una
  cara vecina **no** elegida que comparta una arista partida gana el vértice de la mitad y se
  re-arma para seguir con 3 o 4 vértices (un triángulo con una arista partida → 2 triángulos; un
  cuadrilátero con una → 1 cuadrilátero + 1 triángulo… sin huecos). Las UVs se reparten igual
  (mitades y centro en UV). Los vértices de la mitad se comparten entre las caras que usan la arista.
- **Merge** (vértices, aristas o caras elegidas): todos los vértices involucrados de cada objeto se
  unen en uno, en el centro de su caja; las caras que quedan con vértices repetidos los pierden y
  las que quedan con menos de 3 se borran. Por objeto (no une entre objetos distintos).
- **Delete:** vértices → se borran y también las caras que los usan; aristas → se borran las caras
  que las usan; caras → se borran esas caras. En los tres casos, los vértices que se quedan sin
  ninguna cara se borran y los índices se compactan. En modo Objeto, borra los objetos (como la ○
  de la pestaña).

## 2. Cómo se llega (`cs-ui`)

- **Teclas** (en el workspace del modelo, modo *Model*): **Ctrl+E** Extrude (en esta vista gana
  sobre Export), **Ctrl+D** Subdivide (en esta vista gana sobre Deselect, que aquí no aplica),
  **M** Merge, **Supr / Retroceso** Delete.
- **Botones** en la pestaña del objeto (modo *Model*), una fila arriba de Position: **Extrude**,
  **Subdivide**, **Merge**, **Delete** (botones del kit), deshabilitados cuando no aplican a lo
  elegido (Extrude y Subdivide: hay caras elegidas; Merge: 2 o más vértices involucrados; Delete:
  hay algo elegido).
- **Extrude interactivo:** después de extruir, la selección pasa a las caras nuevas y empieza un
  movimiento de teclado (como G) que sigue al mouse **a lo largo de la normal promedio** de las caras
  (en Persp) o en el plano de la vista (en las orto) con snapping a la cuadrícula; **clic o Enter**
  confirma (extruir + mover = **un** paso de undo); **Esc o clic derecho** cancela y el modelo queda
  como antes (sin paso de undo).
- Subdivide, Merge y Delete son un paso de undo cada uno.

## 3. Pruebas

- `cs-3d`: extruir una cara del cubo da 8+4 vértices y 6+4 caras, la cara extruida queda en los
  vértices nuevos, los lados cierran sin huecos, las UVs de los lados están dentro de la textura;
  extruir dos caras vecinas no crea lado entre ellas. Subdividir una cara del cubo: 4 caras nuevas,
  las 4 vecinas re-armadas sin huecos (cada arista compartida por exactamente 2 caras), UVs
  repartidas. Merge de los 4 vértices de una cara: queda un vértice y las caras degeneradas se van.
  Delete de un vértice, una arista y una cara, con los índices compactados; objetos en modo Objeto.
- UI: Ctrl+E + mover + clic es un paso de undo y Esc no deja nada; Ctrl+D, M y Supr son un paso
  cada uno; los botones se habilitan según lo elegido.
