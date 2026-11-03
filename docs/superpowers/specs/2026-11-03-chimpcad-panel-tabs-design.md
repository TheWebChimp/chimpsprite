# ChimpCAD: el panel 3D por pestañas (sin tanto scroll)

Fecha: 2026-11-03 · Estado: diseño aprobado

**Cambio (2026-11-03, boceto `D:/Work/chimpdeas/.refs/chimpcad/mockup-panel-split.png`):** el panel
se parte en dos a lo alto. **Arriba**, siempre visible, lo general del modelo (lo que era la pestaña
del modelo: modos de selección y Grid en Model, Texture y Texture size, Camera, Light & Outline con
Shading, Sheet settings…), con su propio scroll. **Abajo**, la tira de pestañas de objetos (ya **sin
la pestaña fija del modelo**) con ◁ ▷ y `+`, y debajo el contenido del objeto elegido (en Texture:
Rotate 90°, Flip H, Flip V arriba; luego Position, Rotation, Scale y Double sided), con su scroll.
Sin objeto elegido, la parte de abajo muestra solo la tira. La línea entre las dos mitades va a la
mitad del alto del panel, como el boceto.

Referencia del usuario: `D:/Work/chimpdeas/.refs/chimpcad/mockup-object-tab.png` (a 1×; las
capturas mandan en el acomodo). Sigue a `2026-11-03-chimpcad-chrome-design.md`.

Decisiones tomadas con el usuario:

- El panel se reparte por pestañas para que no haya tanto scroll.
- **La pestaña de un objeto** trae solo lo del objeto, compacto: **Position**, **Rotation** y
  **Scale**, cada una en **una fila con X, Y, Z** (como el boceto).
- **Lo de todo el modelo va en una pestaña del modelo**, fija al principio de la tira.
- **Un botón `+`** a la derecha de las flechas ◁ ▷ agrega objetos.

## 1. La tira de pestañas

- **Primera pestaña, fija: el modelo.** Su etiqueta son los cinco íconos de tipo seguidos
  (`icon.object.cube`, `…cylinder`, `…cone`, `…sphere`, `…plane`), como la primera pestaña del boceto;
  no se cierra (sin ○). Está activa cuando no hay ningún objeto elegido; clic en ella deja sin objeto
  elegido (`select_model_object(None)`).
- **Después, una pestaña por objeto** (como hoy: ícono de su tipo + nombre + ○ = Delete).
- **A la derecha:** las flechas ◁ ▷ (como hoy) y un botón **`+`** (botón del kit, del alto de las
  flechas). El `+` abre un menú con las primitivas (Cube, Plane, Cylinder, Cone, Sphere) y, en el
  mismo menú, el stepper **Sides** (para cilindro, cono y esfera). Elegir una la agrega y elige su
  pestaña. El select "Add" y el stepper "Sides" del cuerpo del panel desaparecen.

### 1.1 El menú del `+` (boceto del usuario)

Referencia: `D:/Work/chimpdeas/.refs/chimpcad/mockup-add-menu.png` (156×118 a 1×; manda en el
acomodo). El `+` abre una ventanita pegada al botón (con la pestañita que la une al `+` arriba a la
derecha y su sombra, como los flyouts del dock):

- Arriba, **cinco íconos azules** (cubo, cilindro, cono, esfera, plano) — piezas nuevas
  `icon.add.cube`, `icon.add.cylinder`, `icon.add.cone`, `icon.add.sphere`, `icon.add.plane`,
  copiadas del boceto. Clic elige el tipo; el elegido lleva el contorno blanco y la flechita sobre la
  línea divisoria (como el boceto).
- Una línea divisoria; debajo, **Sides** (stepper del kit; deshabilitado para cubo y plano).
- Abajo a la derecha, un botón **Add <Tipo>** ("Add Cube", "Add Cylinder", …) que agrega el objeto,
  elige su pestaña y cierra la ventanita.

## 2. La pestaña de un objeto

- **Position**, **Rotation**, **Scale**: un título y una fila con tres steppers, cada uno con su
  **etiqueta de eje** pegada a la izquierda (una cajita oscura con `X`, `Y` o `Z`, como el boceto),
  luego `−`, el valor y `+`. Los tres caben en el ancho del panel (se achican con él; si no caben,
  bajan a dos filas). Mismas unidades y rangos que hoy.
- Debajo: **Double sided**.
- En modo *Texture*: la sección **UV** (Rotate 90°, Flip H, Flip V; Texture size se va a la pestaña
  del modelo).
- Sin botón Delete (la ○ de la pestaña lo hace).

## 3. La pestaña del modelo

- En modo *Model*: los modos de selección (Object, Vertex, Edge, Face) y **Grid**.
- **Texture** (elegir la textura, New Texture) y **Texture size**.
- **Camera**, **Light & Outline** con la tabla **Shading**.
- **Sheet settings…** (el Bake).

## 4. Pruebas

- La tira: la pestaña del modelo es la primera, no se cierra, está activa sin objeto elegido y
  clic la activa; el `+` agrega una primitiva (con Sides) y elige su pestaña.
- La pestaña de un objeto muestra Position/Rotation/Scale en filas de tres steppers con su etiqueta
  de eje, y cambiar un valor es un paso de undo como hoy; no tiene Texture, Camera ni Light.
- La pestaña del modelo tiene Texture, Camera, Light, Shading y Sheet settings, y no las filas de
  transform.
- Con el panel abierto y un objeto elegido, el contenido de la pestaña del objeto cabe sin scroll en
  la ventana de la captura (1920×1009).
- Captura comparada con el boceto.
