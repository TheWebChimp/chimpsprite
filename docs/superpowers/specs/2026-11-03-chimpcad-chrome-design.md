# ChimpCAD: el marco de la pestaña del modelo (ícono, encabezado, pestañas de objetos)

Fecha: 2026-11-03 · Estado: diseño aprobado

Referencias del usuario (copiadas a `D:/Work/chimpdeas/.refs/chimpcad/`):

- `icon-3d.png`: el ícono del botón 3D del dock (un cubo azul dentro del marco del botón).
- `mockup-workspace.png`: la pestaña de un modelo en modo Texture, a 1× (las capturas mandan en el
  acomodo).

Sigue a la parte 3 (`2026-11-02-3d-texture-mode-design.md`).

## 1. El ícono 3D del dock

- La pieza `icon.dock.model` del skin (`assets/ui/skin.aseprite`) con el cubo azul de
  `icon-3d.png` (solo el ícono, sin el marco del botón, que ya dibuja el dock). El dock ya la usa;
  sin la pieza seguía mostrando el texto "3D".

## 2. En la pestaña de un modelo

Según el boceto:

- **Desaparece la barra de opciones de herramienta** (Brush Size, Pixel Perfect, Ink, Symmetry…):
  en la pestaña del modelo no se dibuja y su alto se lo queda el área de abajo. Se quedan las
  herramientas (columna de la izquierda) y los colores (columna de la derecha).
- **Encabezado de ChimpCAD** arriba del área del workspace, de lado a lado (sobre las vistas, a la
  izquierda del panel): una franja con el logo **CHIMPCAD** a la izquierda (pieza `logo.chimpcad`,
  copiada pixel a pixel del boceto) y los botones **Model | Texture** a la derecha (los mismos de la
  parte 3, que dejan de flotar sobre las vistas). Las vistas empiezan debajo de la franja.
- El resto del acomodo de los dos modos no cambia.

## 3. Las pestañas de objetos (panel 3D)

- Arriba del panel 3D, en lugar del título "3D" y de la lista *Objects*, una **tira de pestañas**
  (el componente de pestañas del kit, el de la barra de documentos): **una pestaña por objeto** del
  modelo, con el **ícono de su tipo** (cubo, cilindro, cono, esfera, plano) y su nombre.
  - Clic en una pestaña elige ese objeto (como hoy elegirlo en la lista).
  - La marca `○` de la pestaña quita el objeto (un paso de undo, como Delete).
  - Si no caben, las flechas **◁ ▷** a la derecha recorren las pestañas (como el boceto).
- Debajo de la tira, igual que hoy: **Add** (primitivas) y **Sides**, y las secciones del objeto
  elegido, la textura, la cámara, la luz… (sin la lista *Objects*).
- **Íconos de tipo:** piezas `icon.object.cube`, `icon.object.cylinder`, `icon.object.cone`,
  `icon.object.sphere`, `icon.object.plane`, copiadas del boceto.
- **El tipo del objeto se guarda:** `Object3D` gana `#[serde(default)] kind: Option<PrimitiveKind>`
  (Cube, Plane, Cylinder, Cone, Sphere). Agregar una primitiva lo llena. En archivos viejos (sin
  `kind`) se deduce del nombre si es el de una primitiva ("Cube", "Cylinder"…); si no, se usa el
  ícono del cubo.

## 4. Pruebas

- `cs-core`/`cs-io`: `kind` se guarda y se lee; un objeto viejo sin `kind` abre igual.
- UI: en la pestaña del modelo no se dibuja la barra de opciones (y sí en el lienzo); el
  encabezado con el logo y Model | Texture está arriba del área y las vistas empiezan debajo; la
  tira tiene una pestaña por objeto con su ícono y nombre; clic elige; `○` quita con un paso de
  undo; con muchos objetos aparecen las flechas.
- Captura con `CHIMPSPRITE_DEMO_MODEL` (y `_TEXTURE`) comparada con el boceto.
