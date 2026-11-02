# Pestañas para modelos y fuentes (ChimpCAD y FontForge fuera del lienzo)

Fecha: 2026-11-02 · Estado: diseño aprobado

Sigue a la parte 2A de los modelos 3D (`2026-11-02-3d-modeling-edit-design.md`) y al editor de
fuentes (`2026-11-01-pixel-font-core-design.md`). Va antes de la parte 2B (extruir, dividir, unir,
borrar).

## Objetivo

Los modelos 3D (ChimpCAD) y las fuentes pixel (FontForge) dejan de verse como artboards en el
lienzo. Viven en la **Library** del documento y se editan en **su propia pestaña**, con todo el
espacio del lienzo, como una ventana de picoCAD.

Decisiones tomadas con el usuario:

- **Se siguen guardando dentro del `.chimp`.** El modelo usa la paleta del documento y su artboard
  de textura, y se puede usar como instancia (símbolo) en otros sprites. Los archivos viejos abren
  igual.
- **La lista está en la Library**, con secciones Symbols, Models y Fonts.
- **Pestaña propia en la barra de documentos**, que es una vista del mismo documento.

## 1. Fuera del lienzo (`cs-core`)

- Un solo predicado decide qué está en el lienzo: `Artboard::on_canvas()` = `!in_library &&
  model.is_none() && font.is_none()`. `Document::canvas_artboards()` y `Editor::on_canvas` lo usan;
  así exportar, el agente, las herramientas y el panel Artboards dejan de ver modelos y fuentes sin
  tocar cada sitio.
- `Document::library()` sigue listando los símbolos (`in_library`); `models()` y `fonts()` listan
  los artboards con `model` o `font`.
- **Sin cambio de formato:** `model` y `font` ya se guardan; lo que cambia es que ya no cuentan
  como lienzo. Al abrir un `.chimp` viejo, sus modelos y fuentes aparecen en la Library.
- **Siempre queda al menos un artboard en el lienzo:** si al abrir un documento no queda ninguno
  (p. ej. solo tenía una fuente), se agrega uno vacío del tamaño del primero ("Sprite"), sin paso
  de undo. Las reglas de "no borrar el último artboard del lienzo" cuentan con `on_canvas()`.
- La **textura** de un modelo es un artboard normal: sigue en el lienzo y en el selector de
  textura del panel 3D.
- **Instancias:** un modelo se sigue pudiendo usar como instancia en otros sprites (se compone con
  su render); una fuente no (como hoy).

## 2. La Library con secciones (`cs-ui/library_panel.rs`)

- Tres secciones con su título y su cuenta: **Symbols**, **Models**, **Fonts** (una sección vacía
  se ve con una línea de ayuda corta: "File → New 3D Model…", "File → New Font…", "F8 makes a
  symbol").
- Cada fila: miniatura (el render del modelo; la muestra de la fuente, como su tarjeta), nombre y,
  en modelos, los usos como instancia.
- **Doble clic** (o *Open* en el menú de la fila): abre su pestaña (sección 3). En símbolos sigue
  siendo el modo símbolo de hoy.
- Menú de la fila: Open, Rename (F2), Duplicate, Delete (con confirmación si un modelo tiene
  usos); en modelos también *Place on Canvas* (una instancia) y arrastrar al lienzo pone una
  instancia, como los símbolos.
- La vista previa animada de arriba muestra lo elegido de cualquier sección.

## 3. Las pestañas (`cs-ui/tabs.rs`, `tab_bar.rs`, `lib.rs`)

- `TabRef` gana `View { doc, artboard }`: una pestaña que **no tiene `DocState` propio**; apunta al
  documento padre y a su modelo o fuente.
- Va en la barra justo después de la pestaña de su documento, con el ícono del tipo (provisional:
  `▣` modelo, `Aa` fuente; las piezas del skin `tab.icon.model` y `tab.icon.font` si existen) y el
  nombre del artboard. `TabInfo` gana el ícono. Se marca sucia cuando su documento lo está.
- **Activarla** cambia al `DocState` del padre (como cambiar de documento) y pone el editor en el
  **workspace** de ese artboard: `Editor.workspace: Option<ArtboardId>` (estado de UI), con
  `active` = ese artboard. Activar la pestaña del documento sale del workspace y regresa al artboard
  del lienzo que estaba activo.
- **Compartido con el documento:** un solo `Editor` (historial de undo, paleta, guardado). Ctrl+S
  en la pestaña guarda el `.chimp`; cerrar el documento cierra sus pestañas; si el artboard se
  borra (o un undo lo quita), su pestaña se cierra sola; renombrarlo cambia el título.
- Cerrar la pestaña no pregunta nada (los cambios están en el documento).
- Las pestañas de modelos y fuentes no se guardan: al abrir el documento solo está la suya.
- *File → New 3D Model…* y *New Font…* crean el artboard (y la textura del modelo, en el lienzo) y
  abren su pestaña. El botón "New…" de cada panel hace lo mismo.

## 4. El workspace de ChimpCAD

- El área del lienzo completa (sin reglas, sin el resto de artboards) son los **4 cuadrantes**
  Front | Persp / Top | Side de la parte 2A, ahora en pantalla y no dentro de un artboard: el área
  se parte a la mitad en puntos de pantalla enteros.
- **Siempre en modo Edit:** no hace falta Tab (en el workspace, Tab no hace nada especial). La
  selección, el Grid, los gestos, las teclas y el undo son los de la 2A.
- **Persp:** el render pixel del modelo a la resolución del artboard (la "Size" del modelo), a la
  escala entera más grande que quepa en su cuadrante, centrado; orbitar, mover el objetivo y Ctrl +
  rueda como hoy.
- **Vistas orto:** el zoom de las vistas pasa a puntos de pantalla por 1/16 de unidad (ya no
  depende del zoom del lienzo, que no aplica); al abrir, encuadran el modelo.
- El zoom del lienzo, el scroll y la mano no aplican en el workspace; el panel 3D está a la
  derecha como hoy. Las herramientas de pintar no hacen nada sobre el workspace.
- El código de la 2A se reutiliza: `draw_model_edit` y los gestos reciben el rectángulo del área en
  vez del del artboard. El modo Edit dentro del lienzo (Tab sobre el artboard) desaparece, porque
  los modelos ya no están en el lienzo.

## 5. El workspace de FontForge

- El área del lienzo muestra **solo el glifo actual** de la fuente (un artboard en modo de un solo
  artboard, como el modo símbolo): se pinta con las herramientas de siempre, con la letra fantasma,
  las guías y la línea del ancho de hoy, y el zoom y el scroll del lienzo funcionan.
- Al abrir encuadra el glifo con el panel Font al lado (como hoy `FitActiveBeside`).
- El panel Font (mapa de caracteres, métricas) es el de hoy.

## 6. Lo que cambia para lo existente

- **Símbolos:** igual que hoy (modo símbolo con migas en la pestaña del documento).
- **Panel Artboards:** solo lista artboards del lienzo (por `on_canvas()`).
- **Bake** de un modelo: el artboard horneado aparece en el lienzo (como hoy) y la pestaña del
  documento se activa para mostrarlo.
- **Agente/MCP y exportar:** dejan de ver modelos y fuentes como artboards del lienzo (por el
  predicado); exportar una fuente o un modelo sigue por sus propios caminos.

## 7. Pruebas

- **`cs-core`:** `on_canvas()`; `canvas_artboards`, `models`, `fonts`; el último artboard del
  lienzo no se borra contando con el predicado.
- **`cs-io`:** un `.chimp` viejo con modelo y fuente abre con ellos fuera del lienzo y en la
  Library; uno que solo tiene una fuente gana un artboard vacío en el lienzo.
- **UI (`layout_tests`):** la Library muestra las tres secciones; doble clic en un modelo abre su
  pestaña después de la del documento, con el ícono y el nombre; el workspace dibuja los 4
  cuadrantes en todo el área; un arrastre en la pestaña es un paso de undo que se deshace desde la
  pestaña del documento; cerrar el documento cierra la pestaña; borrar el modelo cierra su pestaña;
  la fuente abre su glifo con el panel Font; el lienzo ya no dibuja modelos ni fuentes.
- **Captura:** `CHIMPSPRITE_DEMO_MODEL` abre la casita en su pestaña.
