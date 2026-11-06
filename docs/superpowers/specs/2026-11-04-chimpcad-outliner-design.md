# ChimpCAD: outliner con grupos y la barrita del objeto

Fecha: 2026-11-04 · Estado: diseño aprobado

Referencia del usuario: Blockbench (outliner con carpetas, ojo y candado; el transform en su
panel) y `D:/Work/chimpdeas/.refs/chimpcad/mockup-mode-icons.png` (el panel de hoy). La idea:
agrupar partes, prenderlas, apagarlas o bloquearlas, y economizar espacio llevando el transform
del objeto a una barrita arriba de las vistas.

Decisiones tomadas con el usuario:

- **Grupos = carpetas con su transform** (como Blockbench): posición, rotación y escala propias
  que mueven a sus hijos; se anidan. Sirven para la animación de después.
- **Ocultar (👁) quita la parte del horneado** (Sheet settings) además de las vistas: así se
  hornean variantes.
- **Bloquear (🔒):** se ve, pero no se elige ni se mueve en las vistas; sus campos, deshabilitados.
- **La barrita** con Position / Rotation / Scale va **fija debajo del encabezado de CHIMPCAD**.
- **Los botones del objeto** (Extrude / Subdivide / Merge / Delete; en Texture Rotate 90° / Flip
  H / Flip V) van **encima del outliner**; **Double sided** pasa a un ícono en la fila de cada
  objeto del outliner.
- **Íconos provisionales** (carpeta, ojo, candado, double sided, New Group): los dibujo en el
  estilo del skin y el usuario los reemplaza después.

Se entrega en tres partes, cada una con sus commits y probada en la app:
1. Modelo y render (grupos, ocultar, bloquear), sin UI nueva.
2. El outliner en lugar de la tira de pestañas.
3. La barrita del objeto y el panel reacomodado.

## 1. El modelo (cs-core)

- `Group3D { name, transform: Transform3D, hidden: bool, locked: bool, open: bool }`.
- `Object3D` gana `#[serde(default)] hidden: bool` y `#[serde(default)] locked: bool`.
- `Model3D` gana `#[serde(default)] groups: Vec<Group3D>` y `#[serde(default)] tree:
  Vec<Node>`, el árbol del outliner con su orden: `enum Node { Object(usize), Group { group:
  usize, children: Vec<Node> } }`. (No se llama `outline`: ese campo ya es el color del contorno.)
- `objects` sigue siendo la lista plana de siempre: la selección, el pick, las UVs, la topología
  y el deshacer de la selección siguen usando sus índices.
- **Invariante:** cada objeto aparece exactamente una vez en `tree` y cada grupo también;
  `Model3D::normalize_tree()` lo repara (agrega al final de la raíz lo que falte, quita índices
  fuera de rango o repetidos). Un archivo viejo trae `tree` vacío: al leerlo se arma con los
  objetos en su orden.
- **Herencia:** `effective_hidden(o)`, `effective_locked(o)` (la parte o cualquier grupo padre) y
  `world_transform(o)` (la del objeto compuesta con la de sus grupos, de adentro hacia afuera:
  escala → rotación → traslación de cada nivel, como `world_vertices` hoy).
- Las operaciones del modelo que cambian la lista de objetos (agregar, quitar, duplicar, la
  topología que borra objetos) mantienen `tree` al día: un helper `Model3D::remove_object(i)` (quita
  y corre los índices) y `Model3D::push_object(obj, parent: Option<usize>)`.

## 2. Render, vistas y horneado (cs-3d, cs-render, cs-ui)

- `world_vertices` usa `world_transform`: los grupos mueven a sus hijos en el render, las vistas
  orto, el pick, el gizmo y el horneado.
- Lo oculto (efectivo) no se dibuja en ninguna vista ni en el render ni sale en el horneado, y no
  se elige.
- Lo bloqueado (efectivo) se dibuja pero el pick de objetos/caras/aristas/vértices, la caja de
  selección, el gizmo y G/S/flechas lo ignoran; elegirlo desde el outliner sí se puede (para
  verlo en la barrita, deshabilitada).
- **Duplicar** un objeto deja la copia en el mismo grupo, justo después; duplicar un grupo copia el
  grupo con todo lo de adentro. **Borrar** un grupo borra lo de adentro. Todo, un paso de undo.

## 3. El outliner (panel derecho, abajo)

Reemplaza la tira de pestañas de objetos (y su ○).

- **Encabezado:** el título "Outliner" y, a la derecha, tres botones de 13×13 del kit
  (`icon_button_at`): **Duplicate** (`icon.copy`, el de hoy), **New Group** (mete lo elegido en un
  grupo nuevo en el lugar del primero elegido; sin nada elegido, uno vacío al final) y **+** (el
  menú de agregar de hoy; lo nuevo entra en el grupo elegido o en el del objeto elegido).
- **Cada fila:** sangría por profundidad; ▸/▾ en los grupos (abre y cierra: `Group3D.open`, no
  es un paso de undo); el ícono (carpeta, o el tipo del objeto como hoy); el nombre; a la derecha
  ◧ Double sided (solo objetos), 👁 y 🔒 (cada clic es un paso de undo). Lo heredado de un padre
  oculto o bloqueado se ve atenuado.
- **Clic** en la fila: es la **fila activa** (un objeto o un grupo; reemplaza a la pestaña
  elegida, `model_object`) y, en modo Model, la selección 3D pasa a modo Object con sus objetos
  (un grupo: todos los de adentro). **Shift + clic:** agrega o quita de la selección (la activa,
  la última tocada). Lo elegido en las vistas se resalta en el outliner: es la misma selección;
  elegir un objeto en las vistas lo hace la fila activa.
- **Doble clic** en el nombre: renombrar en el lugar (Enter confirma, Esc cancela; un paso de
  undo).
- **Arrastrar** una fila: la reordena o la mete en otro grupo, con una línea que marca dónde cae
  (entre filas = antes/después; sobre un grupo = adentro). Un grupo no se mete en sí mismo ni en
  un hijo suyo. Un paso de undo. La transform del mundo no se conserva: la parte toma la del grupo
  nuevo (como un cambio de padre sin compensar; YAGNI).
- **Supr** con el puntero en el outliner, o el botón Delete, borra lo elegido.
- Con muchas filas, el outliner hace scroll.
- **Íconos nuevos (provisionales):** `icon.outliner.folder`, `icon.outliner.eye`,
  `icon.outliner.eye_off`, `icon.outliner.lock`, `icon.outliner.unlock`,
  `icon.outliner.double_sided` y `icon.outliner.new_group`, en `assets/ui/skin.aseprite`.

## 4. La barrita del objeto y el panel

- **La barrita:** una fila fija debajo del encabezado de CHIMPCAD, del alto de la barra de
  opciones (23), con su separador grabado: `Position X Y Z   Rotation X Y Z   Scale X Y Z` con los
  campos numéricos compactos de la barra de opciones (`bar_fields::number`: se escriben o se
  arrastran), en las unidades de hoy (1/16, °, %). Un arrastre es un paso de undo. Edita la **fila activa**
  del outliner (un objeto o un grupo: la transform del grupo). Sin fila activa, o con varias
  partes elegidas en las vistas que no son la fila activa y sus hijos, los campos vacíos y
  deshabilitados; con la fila activa bloqueada, deshabilitados. Si no cabe, scroll de lado.
- **En modo Texture**, la barrita y debajo la barra del pincel; el encabezado no se mueve al
  cambiar de modo.
- **El panel derecho:** arriba lo general de hoy (modos y Grid, Texture, Camera, Light & Outline,
  Shading… / Sheet settings…); abajo la fila de botones del objeto y el outliner. La línea que
  parte el panel queda donde está.
- Salen del panel: la tira de pestañas, y Position / Rotation / Scale / Double sided.
- No cambian los atajos (1–4, G, S, W/E/R, Ctrl+J, Ctrl+E, Ctrl+D, M, Supr), los gizmos ni el modo
  Texture.

## 5. Tests

- Modelo: herencia de oculto, bloqueado y transform; `normalize_tree`; leer un modelo sin
  `groups`/`tree`; agregar, quitar y duplicar mantienen el árbol.
- Render/horneado: lo oculto no se pinta ni sale; un grupo movido mueve a sus hijos; lo bloqueado
  no se elige.
- Outliner (layout_tests): elegir, Shift + clic, renombrar, arrastrar a otro grupo (un paso de
  undo), ojo y candado (un paso cada uno), New Group, abrir/cerrar sin undo.
- Barrita: un paso de undo por arrastre; deshabilitada con varias, ninguna o bloqueada; edita el
  grupo elegido.
