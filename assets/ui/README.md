# Assets de la UI en pixel art

Se compilan dentro del binario (`include_bytes!` en `crates/cs-ui`).

## chimpsprite.ttf

Fuente pixel de ChimpSprite, hecha por el autor del proyecto ("Chimp"). Se distribuye con la
misma licencia que el repositorio (MIT o Apache-2.0, a elección; ver `LICENSE-MIT` y
`LICENSE-APACHE` en la raíz).

- Grilla de 16 píxeles por em: se usa a 16 puntos, un píxel de la grilla por punto.
- Mayúsculas de 7 px; las colas bajan 2 px.
- Incluye acentos, ñ, ¿, ¡, € y °. Los glifos que no tiene (•, …, ▸) salen de las fuentes de egui.

## skin.aseprite

El skin (incluido el arte copiado del mockup) es del autor del proyecto y se distribuye con la
misma licencia que el repositorio, igual que la fuente.

Las piezas de la interfaz. Se dibujan en la capa **"Skin (dibuja aquí)"**; la capa "Guías" queda
oculta.

- **Piezas:** cada una es un slice con nombre (View → Slices en Aseprite). Importa el nombre, no
  la posición: para cambiar el tamaño de una pieza, se cambia su slice.
- **9-slice:** las piezas que se estiran (`window`, `button`, `panel`, `menubar`, `tab`,
  `tool.slot`…) tienen un centro.
  - Las esquinas no se deforman.
  - Los bordes se estiran a lo largo.
  - El centro se estira en las dos direcciones.
  - En `window`, la franja de arriba del centro es la barra de título (13 px).
- **Colores:** los slices `color.*` (`desktop`, `text`, `text_dim`, `accent`, `panel`,
  `outline`, `selection`, `scroll`, `background`, `canvas`, `highlight`) dan el color de su
  píxel central. Con Pixel UI, `background` es el fondo de la app (menú, pestañas, barra de
  opciones y paneles) y `canvas`, el del área alrededor de los artboards.
  - Los separadores grabados del mockup son una línea `canvas` de 1 px con una `highlight` de
    1 px debajo (horizontales) o a la derecha (verticales).
- **Variantes:** `.hover`, `.down` y `.active` son opcionales; si falta una, se usa la pieza
  base.
- **Piezas sin dibujar:** una pieza totalmente transparente cuenta como que no existe; la app
  dibuja un respaldo plano con los colores del skin.
- **Íconos de herramientas:** `icon.<herramienta>` miden 17×17 sobre fondo transparente (los
  nombres están en `icon_piece`, `crates/cs-ui/src/tool_group.rs`). Se dibujan centrados en el
  marco.
- **Marcos de herramienta:** los tramos de la columna del mockup, pegados uno debajo del otro.
  Todos con centro (2,2,19,19); el ícono va a (3,3).
  - `tool.slot.first` (el de arriba, redondeado arriba) y `tool.slot` (los del medio) miden
    23×22: traen el borde de arriba y no el de abajo, que es el del botón siguiente.
  - `tool.slot.last` (el de abajo, redondeado abajo) mide 23×23: trae los dos bordes.
  - Cada uno tiene su `.active` (la herramienta elegida): el botón activo suelto del mockup, con
    el bisel invertido (blanco ↔ (221,206,191)) y el relleno café tramado en la misma posición.
  - Si falta `.first` o `.last`, se usa `tool.slot`. `tool.slot.hover` está sin dibujar y cae
    al normal.
- **Caja de la barra:** `toolbar.box` mide 27×7, con centro (3,3,21,1): la línea redondeada
  (189,164,153) que rodea la columna, con 1 px de aire hasta los botones. Se estira en vertical.
  El panel mide 5 + el ancho de la caja + 5.
- **Botones de simetría:** `sym.slot` (eje apagado) y `sym.slot.active` (prendido) miden 15×16,
  sin centro (los agrandados del mockup, frame 4). El cuadro de 13×13 empieza en (1,1) y su
  interior de 11×11, transparente, en (2,2): ahí va el ícono `icon.sym_vertical`,
  `icon.sym_horizontal`, `icon.sym_diag_down` o `icon.sym_diag_up` (11×11, fondo
  transparente). En la barra van uno cada 17 px.
- **Barra de opciones** (frame 4 del diseño, capa Mockup):
  - Modos de selección: `icon.sel_replace`, `icon.sel_add`, `icon.sel_subtract` e
    `icon.sel_intersect` (11×11, fondo transparente), en los marcos de simetría (`sym.slot`, y
    `sym.slot.active` el modo elegido): los cuadritos de Mode del diseño son iguales a esos.
  - `field.combo` (13×16, centro (2,10,1,3)): el combo. 2 columnas de borde a la izquierda, 1 de
    relleno que se estira y, a la derecha, 10 con el brillo y la flecha ▾. El texto va 6 px
    adentro y 5 px antes de la flecha.
  - `field.number` (10×16, centro (2,10,1,3)): el campo numérico, con el brillo de arriba a la
    derecha y la esquina redondeada. En la barra mide 27.
  - `field.unit` (17×16, sin centro): la unidad "px". Empieza en la última columna blanca del
    campo y tapa sus 2 últimas columnas.
  - `field.check.on` (13×14, sin centro): la casilla marcada de la barra (no es la del menú, que
    es más chica: 11×12). El texto va 3 px después. `field.check` (sin marcar) todavía no está
    dibujada (el diseño no la muestra): hasta entonces va un cuadro plano de respaldo.
  - El combo, el campo, la unidad y la casilla llevan una fila de sombra (230,231,240) abajo del
    relleno blanco; la unidad termina en una esquina redondeada (x 318..319 del diseño).
- **Pestañas:** `tab` (inactiva, café) mide 7×15 y `tab.active` (clara) 7×17, las dos con
  centro (3,0,1,alto): se estiran solo a lo ancho. Las esquinas de arriba son redondeadas y
  abajo quedan abiertas. La activa baja sobre las dos filas de la línea grabada de abajo y la
  interrumpe. Se copiaron sin el texto de las pestañas del mockup.
  - `icon.tab_dirty` (○, cambios sin guardar) e `icon.tab_close` (×, sin cambios) miden 5×5. El
    mockup no muestra otro aspecto al pasar el puntero: no hay `.hover`.
  - Home: la casita animada son `icon.home.0`, `icon.home.1` e `icon.home.2` (21×21, casita y
    humo juntos), copiados de los 3 frames de `home animation.aseprite`, de 300 ms cada uno
    (`duration=300`). Van a 3 px del borde
    izquierdo, arriba de todo. `home.ground` (58×1) es el suelo sobre la línea grabada, desde el
    borde izquierdo.
- **Artboards con Pixel UI** (la ventana del frame 4 del diseño, en (209,235), 173×123): el
  marco es `artboard.frame`, el "○" de la derecha es `window.close` y el "≡" de la izquierda
  es `artboard.menu` (9×9, fondo transparente, rayas de 6 px en las filas 2, 4 y 6 desde la
  columna 2). `window.menu` no sirve: sus rayas miden 7.
  - `artboard.frame` (24×24, centro (2,14,20,8)) sale de `window` con Lua: el mismo borde
    oscuro y las mismas esquinas redondeadas; la barra con una fila rosa más (13 filas: borde,
    brillo, 10 rosas y la línea oscura) y, debajo, un anillo blanco de 1 px pegado al borde y
    a la barra. El resto de adentro es transparente: ahí van el damero y los píxeles.
  - El área de píxeles del artboard es el hueco: 2 px adentro a los lados y abajo (borde y
    blanco) y 14 abajo del borde de arriba (barra y blanco).
- **Animaciones:** las piezas `<nombre>.0`, `<nombre>.1`… son los frames, en orden. Lo que dura
  cada uno va en el user data del slice (Slice Properties → User Data), en una línea
  `duration=<ms>`, igual que la duración del frame en Aseprite. Sin esa línea, dura 100 ms.
  - Colores: `color.tab_text_dim` (el nombre de las pestañas inactivas) y `color.home_text`
    ("Home" y los títulos del menú).
  - Los "○ ×" sueltos del mockup son iguales a las marcas: con el puntero encima, la ○ pasa a
    ser la × de `icon.tab_close`.
- **Área de trabajo:** `workspace.box` mide 7×7, con centro (3,3,1,1): la línea clara
  redondeada (como la de `toolbar.box`), 1 px de aire y una línea (89,77,77) con las puntas
  cortadas. Rodea las reglas y el lienzo, con 5 px de fondo alrededor.
- **Reglas:** `digit.0` … `digit.9` son las cifras del mockup, de 3×5 (el 1, de 1×5), sin
  fondo; van con 1 px entre cifra y cifra. `ruler.corner` (11×11) es la esquina café con la
  carita. `color.ruler_tick` (136,110,106) es el de las marcas, el signo menos y la línea entre
  las reglas y el lienzo; el relleno es `color.background`, el brillo `color.highlight` y la
  punta de las marcas chicas `color.canvas`.
- **Degradé del menú:** `menubar.fade` mide 122×34, sin centro: el tramado café que va a
  tamaño de pieza contra la esquina de arriba a la derecha de la ventana, detrás del menú y de
  las pestañas. Como el menú de la app mide 2 filas más que el del mockup, sus 2 últimas filas
  se repiten debajo, hasta una fila antes del grabado de las pestañas.
- **Origen:** los íconos, los marcos y los botones se copiaron del mockup del autor
  (`.design/Design.aseprite`, que no se versiona).
- **Botones:** `button`, `button.hover` y `button.down` vienen del mockup del diálogo "Unsaved
  changes".
  - `button.active` (220,100, 49×17, mismo centro (3,3,43,10) que `button`) es la opción
    elegida de los grupos de la barra (Sample, Linear/Radial, Rotation). El diseño no tiene un
    botón de texto activo, así que se derivó de `button` con el mismo mapeo normal → activo que
    llevó `tool.slot` a `tool.slot.active`, leyendo los píxeles con Lua:

    | `button` | `button.active` |
    | --- | --- |
    | (89,77,77) borde y (136,110,106) sombra | igual |
    | (255,255,255) | (221,206,191) |
    | (221,206,191) | (255,255,255) |
    | (234,224,221) relleno, 12 filas | el tramado café (189,164,153)/(174,152,142) del relleno de `tool.slot.active`: la fila `r` (0 a 11) toma la fila `round(r·18/11)` de las 19 de la herramienta, y las columnas repiten su tramado de período 2 |
    | transparente | transparente |

    Si falta, la opción elegida usa `button.down`.

- **Paleta y editor de color** (frame 4 del diseño, capa Mockup, columna x 784..972; el skin
  creció a 320×272 y estas piezas van en y 236..267):
  - `palette.box` (9×9, centro (4,4,1,1)): la caja de la paleta, con la línea grabada
    (189,164,153), 1 px de aire y la línea (89,77,77), esquinas redondeadas; adentro,
    transparente.
  - `palette.swatch` (13×13, centro (1,1,11,11)): el borde negro de un cuadrito, sin el color.
    Van uno cada 12 (comparten el borde), 4 px adentro de la caja.
  - `palette.add` (6×7): la marca Ⅲ que va a (3,3) de la celda siguiente al último color;
    agrega el color primario a la paleta.
  - `picker.copy` (25×16): el botón de copiar (dos hojas), entero; copia el color en hex.
  - La tira de canales (16 de alto): `channel.r` (4×16, centro (3,0,1,16): la punta izquierda y
    el tramo rojo), `channel.g` y `channel.b` (1×16, se estiran), `channel.a` (4×16, sin
    centro: un período del damero de su línea, que se repite), `channel.hex` (5×16, centro
    (0,0,1,16): el tramo del hex con la punta derecha) y `channel.sep` (1×16, la línea entre
    tramos). Los puntos `channel.dot.r/g/b/a` (6×6) llevan las esquinas transparentes.
    `color.field_text` (89,77,77) es el color de sus números.
  - `channel.a` es blanco liso adentro (filas 1..12): el damero va solo en su línea (filas 13 y
    14). Se había copiado de las columnas 900..903, que tienen la mitad del punto del alfa, y
    al repetirse dejaba manchas lavanda detrás del texto.
  - Las puntas sueltas, para cuando el encabezado del editor se parte en filas (y 252, desde
    x 113), sacadas del mismo skin: `channel.cap.l` (3×16) es la punta izquierda de
    `channel.r` con la línea del hex (197,199,221) en vez de la roja, y abre el hex solo en su
    fila; `channel.cap.r` (2×16) es la punta derecha de `channel.hex` con la línea del alfa
    (154,151,185), y cierra el grupo RGBA cuando el hex va en otra fila.
  - `picker.sv` (7×7, centro (3,3,1,1)): la caja del cuadro S/V hasta la línea que lo separa
    de las barras (incluida). `picker.bars` (7×6, centro (3,2,1,1)): la caja de las barras de
    tono y alfa. El centro es transparente: los degradés los calcula la app.
  - `picker.cursor` (9×9) y `picker.cursor.bar` (7×7): los cursores "○ ○" sueltos debajo del
    editor; son iguales, píxel por píxel, a los dibujados sobre el cuadro S/V y las barras.
- **Botones de arriba de la paleta** (frame 4, capa Mockup, y 99..114; en el skin, y 236..251
  desde x 120):
  - `palette.buttons` (5×6, centro (2,2,1,1)): el marco del grupo de tres botones (x 786..858)
    sin íconos: la línea (89,77,77), el brillo blanco arriba y a la derecha, (221,206,191) a la
    izquierda y abajo y una fila de sombra (136,110,106). La app lo estira a 73×16.
  - `palette.buttons.sep` (3×16): entre dos botones (x 809..811): el brillo del de la izquierda,
    la línea y el brillo del siguiente. Va cada 24.
  - `palette.button` (5×6, centro (2,2,1,1)): el botón suelto del damero (x 947..970), 24×16.
  - `palette.button.active` (3×3, centro (1,1,1,1)): no está en el diseño. Es el bisel y el
    relleno del primer botón con el bisel invertido (blanco <-> (221,206,191)) y el relleno
    (189,164,153), como `tool.slot.active`. Va sobre el bisel y el relleno del damero prendido (el
    Mask elegido) o de un botón apretado.
  - El candado no se ve prendido: bloqueada lleva `palette.icon.lock` (el cerrado, como en el
    diseño) y desbloqueada `palette.icon.lock.open` (el abierto), los dos en el botón normal.
  - Íconos, con el relleno (234,224,221) transparente: `palette.icon.lock` (7×9),
    `palette.icon.menu` (7×7), `palette.icon.new` (9×9) y `palette.icon.mask` (18×9, el
    damero). Van centrados en el relleno de su botón (con piso), que es donde están en el
    diseño.
- **Colores primario y secundario** (frame 4, capa Mockup, x 33..59, y 425..473, debajo de la
  barra de íconos; el skin creció a 320×328 y estas piezas van en y 276..324). La caja de
  arriba de esa (y 393..418, con las flechas curvas) no se usa.
  - `fgbg.box` (27×49): la caja con la línea grabada y la línea (89,77,77) que separa las
    mitades. Las dos mitades (19×19 en (4,4) y (4,26)) son transparentes: los colores los pinta
    la app.
  - `fgbg.swap` (9×9): el botón redondo de intercambiar, en (9,20) de la caja; lo de afuera del
    círculo (el negro y el damero de las mitades) es transparente.
  - `fgbg.checker` (8×8): un período del damero (197,199,221)/(230,231,240) de celdas de 4×4,
    en fase con la esquina de la mitad. Va en mosaico detrás de un color con alfa.

### Probar cambios en vivo (escritorio)

    CHIMPSPRITE_SKIN=/ruta/a/skin.aseprite cargo run -p chimpsprite

Con Pixel UI encendido, la app vuelve a leer ese archivo al guardarlo (lo revisa una vez por
segundo). Si el archivo tiene un error, se queda el skin anterior y lo avisa en el log.
- **Menús desplegables** (del diseño de `Design.aseprite`, frame 3, capa "Layer 4"):
  - `menu.title.open` (25×18, centro (2,2,21,14)): la caja del título abierto, redondeada
    arriba y sin el texto. Las dos filas de abajo se dibujan sobre el borde de arriba del panel
    para unir los dos.
  - `menu.panel` (5×5, centro (2,2,1,1)): el panel en 9-slice; línea (89,77,77), bisel claro
    arriba y a la derecha y (221,206,191) a la izquierda y abajo, esquinas cortadas. En el
    diseño, la esquina de arriba a la izquierda queda tapada por la caja: en la pieza es la de
    la derecha en espejo (la usan los submenús).
  - `key.ctrl` (19×7): la tecla Ctrl como está dibujada; sus letras y esquinas son
    transparentes (en el diseño, el relleno del panel). `key` (19×7, centro (1,1,17,5)): la
    misma tecla sin letras, para las otras teclas con nombre. `key.shift` (22×7): la tecla
    Shift del diseño (Layer 4, frame 4), igual que Ctrl: letras y esquinas transparentes.
  - `menu.check.on` y `menu.check` (11×12, sin centro): las casillas marcada y sin marcar de
    los menús (Layer 4, frame 4, junto a "Onion Skin": x 34..44, y 87..98 y 102..113), con
    las 4 esquinas de afuera transparentes (el relleno (234,224,221) del panel) y una fila de
    sombra (230,231,240) abajo del interior blanco. Van al borde del texto de los ítems, 2
    filas arriba de las mayúsculas, y el texto 3 px después. No son las de la barra (13×14).
  - `color.menu_highlight` (blanco): el brillo del panel y la línea clara de los separadores;
    la otra línea es `background`. El atajo tenue (el "+N") es `ruler_tick`.
- **Tooltip de la barra de íconos** (del mockup, frame 2, junto a la barra):
  - `tooltip.bubble` (49×20, centro (11,13,34,4)): el globo naranja con la cola a la izquierda.
    La cola mide 8 de ancho y su punta está en la fila 8; el cuerpo (41×20) tiene las esquinas
    redondeadas, el brillo arriba a la derecha y una fila de sombra abajo. El centro deja la
    cola en los bordes de arriba y de la izquierda: el globo se estira a la derecha y hacia
    abajo sin deformarla.
  - La app pone la punta 6 px a la derecha del botón, con su fila en el centro del botón, y el
    texto (`color.text`) con 6 px de aire en el cuerpo.
- **Timeline** (del diseño de `Design.aseprite`, frame 4, capa Mockup; la caja va de (65,556) a
  (972,762) y es `workspace.box`). Fila de 336 a 388 del skin:
  - `timeline.group` (3×16, centro (1,1,1,14)): el marco redondeado de un grupo de botones; el
    centro es transparente. `timeline.button` (23×14): una celda vacía (la del botón suelto del
    diseño); `timeline.button.sep` (1×14): la línea tramada entre dos celdas.
  - `timeline.icon.<nombre>` (21×11, relleno transparente): el interior de cada celda, `first`,
    `prev`, `play`, `next`, `last`, `frame_add`, `frame_remove`, `layer_add`, `layer_remove`,
    `group_add`, `tilemap_add` y `onion` (el onion skin apagado, la cebolla cortada).
    `timeline.icon.onion.on` (15×17): la cebolla entera suelta del diseño, para el onion skin
    prendido.
  - `timeline.onion_range` (59×17): ◂ [antes | después] ▸ del onion skin, con su brillo abajo.
  - `timeline.stepper.minus` y `.plus` (15×17) y `timeline.stepper.field` (7×17, centro
    (0,0,1,17)): − [campo] + de la opacidad y la duración; el campo se estira por su columna
    blanca y deja el brillo de la derecha. `timeline.combo` (13×17, centro (2,0,1,17)): el combo
    del modo de mezcla (sin las marcas oscuras sueltas del borde del diseño).
  - `timeline.icon.clock` (16×18) y `timeline.divider` (11×3, centro (5,0,1,3)): la línea
    grabada entre la barra y la tabla, con sus uniones a los lados de la caja.
  - `timeline.eye` / `timeline.eye.closed` (9×7): capa visible / oculta.
    `timeline.cel` / `timeline.cel.empty` (7×7): cel con imagen / vacío. El candado de la tabla
    es `palette.icon.lock`; el abierto, `palette.icon.lock.open` (7×9), es el segundo de los
    botones sueltos de la paleta, con el relleno transparente.
