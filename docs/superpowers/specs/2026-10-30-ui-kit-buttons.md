# UI kit, paso 1: botones y pestaña "UI Catalog"

Fecha: 2026-10-30 · Estado: aprobado

## Objetivo

Primer paso de la estandarización de la UI. Se hace un componente a la vez: se deja listo, el
usuario lo prueba en la app y se sigue con el siguiente.

El diseño de referencia es la hoja de componentes del usuario: `.design/Design.aseprite`, capa
**Mockup**, **frame 9** (exportada en `D:/Work/chimpdeas/.refs/tmp/mockup_f9_ref.png`, 1:1).

Este paso cubre el **botón de texto** en sus cuatro estados y una pestaña **UI Catalog** que lo
muestra. Los botones segmentados y los botones con ícono son el paso siguiente.

## 1. Piezas del skin (ya copiadas del mockup a `assets/ui/skin.aseprite`)

Las cuatro miden 49×17, con centro 9-slice `x 3, y 3, w 43, h 9`.

| Pieza | Estado | Dibujo |
|---|---|---|
| `button` | Normal | Borde `#594d4d`; la fila de arriba blanca; la columna izquierda `#ddcebf` y la derecha blanca; relleno `#eae0dd`; abajo `#ddcebf` y bisel `#886e6a`; borde inferior con esquinas `#f1ebdb`, y debajo una línea clara `#f1ebdb` |
| `button.hover` | Hover | Relleno blanco; la fila de arriba y la columna derecha `#eae0dd`; el resto igual que Normal |
| `button.down` | Presionado | **No se desplaza.** Es Normal sin el bisel `#886e6a`: el cuerpo mide 15 px más la línea clara, y la última fila de la pieza es transparente |
| `button.disabled` | Deshabilitado (**nueva**) | Borde `#886e6a`, que en las filas de abajo cambia a `#594d4d` (intencional); la columna izquierda `#f1ebdb`; abajo `#f1ebdb` y `#bda499` |

La línea clara de abajo es **parte de la pieza**. El botón ocupa 17 px de alto en el layout, y no
hace falta dibujar aparte ninguna "luz inferior" (hoy existe `under_light` en `pixel_ui.rs`, que
se reemplaza).

## 2. Medidas del texto (del mockup)

- **Fuente:** pixel (chimpsprite.ttf), con mayúsculas de 6 px.
- **Tope de las mayúsculas:** 4 px debajo del borde superior de la pieza (el borde, más 3 filas de
  aire).
- **Relleno horizontal:** 5 px de relleno entre el borde y la tinta del texto a cada lado. El ancho
  del botón es `1 + 5 + tinta + 5 + 1`, con un mínimo opcional que pide quien lo llama. Si el
  ancho pedido es mayor, el texto se centra.
- **Color del texto:**
  - Normal, Hover y Presionado: `#050403`, que es casi negro. Agregar el color al skin como
    `color.button_text` si no existe; si ya existe un color equivalente, usar ese.
  - Deshabilitado: `#886e6a`, que ya está como `color.button_text_disabled`.
- **Sombra del texto:** `#ddcebf` en (+1, +1), en **Normal, Presionado y Deshabilitado**. En
  **Hover no hay sombra**. Se dibuja primero la sombra y encima el texto.
- **Presionado:** el texto no se mueve.

## 3. Componente (`cs-ui`)

- **Módulo nuevo `crates/cs-ui/src/kit/`** (el "UI kit"), con `kit/button.rs`:
  - las constantes con nombre (`HEIGHT = 17`, `PAD_X = 5`, `CAPS_TOP = 4`, `SHADOW = (1, 1)`);
  - `pub enum ButtonState { Normal, Hover, Pressed, Disabled }`;
  - `pub fn button(ui, skin, label) -> Response`, para uso real (el estado sale de la interacción
    y de `ui.is_enabled()`);
  - `pub fn button_sized(ui, skin, label, min_width)`;
  - `pub fn button_shapes(skin, rect, label, state, ...)`, que dibuja un estado forzado (lo usa
    el catálogo).
- **`WidgetState`** gana `Disabled`, y `Skin::piece` busca `name.disabled`.
- **Unificar:** `pixel_ui::pixel_button`, `pixel_buttons`, `pixel_button_sized`,
  `button_of_size`, `text_button*`, `dialog_buttons`, `pixel_home::button` y las demás rutas de
  botón de **texto** pasan por `kit::button`.
  - Quitar `lit_button` (alias muerto) y `under_light`.
  - Conservar el anillo de foco que tiene `button_of_size`, ahora para todos los botones.
  - Los botones con ícono y los segmentados **no** se tocan en este paso.
- **Sin skin:** se queda el botón de egui de hoy.
- **Layout:** donde algo dependía de 16 px de alto para un botón de texto, ahora son 17. Hay que
  ajustar los layout tests a las medidas nuevas (las del mockup mandan).

## 4. Pestaña "UI Catalog"

- **Tipo de pestaña:** hoy Home es `Tabs.active == None`. Se agrega un tipo de pestaña no
  documento: el catálogo.
  - Se abre desde `Help → UI Catalog`, y con la variable de desarrollo
    `CHIMPSPRITE_DEMO_CATALOG=1` (para capturas).
  - Aparece en la barra de pestañas como una pestaña más, "UI Catalog", que se puede cerrar.
- **Contenido:** una página con scroll, el fondo del workspace y secciones con título. En este paso
  hay una sección, **Buttons**, con el layout del mockup:
  - una fila "Button baseline text", con un botón y a su lado un texto suelto, para comparar la
    línea base;
  - una columna con los cuatro estados forzados (Normal, Hover, Pressed y Disabled), cada uno con
    su etiqueta a la derecha;
  - un botón **real**, que muestra hover y presionado al usarlo, y cuenta los clics ("Clicked 3×");
  - las medidas anotadas, en texto chico gris: `h 17`, `pad 5`, `caps 4`.
- **Crecimiento:** el catálogo crece en los pasos siguientes. Cada componente nuevo agrega su
  sección.

## 5. Pruebas

- **Geometría:** el botón mide 17 de alto; el ancho es `tinta + 12`; el tope de las mayúsculas
  está en `top + 4`; la tinta empieza en `left + 6`.
- **Estados:**
  - Hover usa `button.hover` y no tiene sombra;
  - Disabled usa `button.disabled` y el color deshabilitado;
  - Pressed usa `button.down` y el texto no se mueve.
- **Diálogos:** los botones de los diálogos (OK y Cancel) siguen funcionando con Enter y Esc
  (`dialog_tests`).
- **Catálogo:** se abre desde el menú y muestra los cuatro estados.
