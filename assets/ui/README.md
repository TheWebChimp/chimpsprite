# Assets de la UI en pixel art

Se compilan dentro del binario (`include_bytes!` en `crates/cs-ui`).

## chimpsprite.ttf

Fuente pixel de ChimpSprite, hecha por el autor del proyecto ("Chimp"). Se distribuye con la
misma licencia que el repositorio (MIT o Apache-2.0, a elección; ver `LICENSE-MIT` y
`LICENSE-APACHE` en la raíz).

- Grilla de 16 píxeles por em: se usa a 16 puntos, un píxel de la grilla por punto.
- Mayúsculas de 7 px; las colas bajan 2 px.
- Cifras monoespaciadas: todas avanzan 5 px. El "1" (3 de tinta) va centrado en su celda, 1 px
  adentro; en `.design/chimpsprite.ttf` avanza 4, así que al copiarla se le corre el trazo
  64 unidades a la derecha y se le pone un avance de 320 (como al espacio, que va de 128).
- Incluye acentos, ñ, ¿, ¡, € y °. Los glifos que no tiene (•, …, ▸) salen de las fuentes de egui.

## chimpfont.ttf

La fuente de los textos chicos con Pixel UI (`TextStyle::Small`): solo mayúsculas de 5 px y
ASCII, a 8 puntos (un píxel de su grilla, un punto). La usan la barra de estado, los globos de
los tooltips, las rutas de Home y los textos de Pixnews.

## unifont_symbols.otf y unifont_emoji.otf

Los emojis y símbolos en pixel art (16×16, de un color) de GNU Unifont 18.0.01, recortados a
esos rangos con `fontTools.subset` (`unifont_symbols`: flechas, ☀ ❤ ✨ y dingbats;
`unifont_emoji`: U+1F000..1FAFF). Van detrás de las fuentes pixel, antes que las de egui.
Licencia: SIL OFL 1.1 (ver `UNIFONT-OFL.txt`).

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
- **Variantes:** `.hover`, `.down`, `.active` y `.disabled` son opcionales; si falta una, se usa la
  pieza base.
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
  - Los combos de la barra son el select del UI kit (`select.*`, abajo); `field.combo` se borró.
  - `field.number` (10×16, centro (2,10,1,3)): el campo numérico de antes, con el brillo de
    arriba a la derecha y la esquina redondeada. Ya no lo usa la barra (sus números son los
    campos del UI kit, abajo); lo siguen usando la tira de canales del editor de color y el
    timeline.
  - La casilla de la barra es la del UI kit (`check`, abajo).
  - El campo lleva una fila de sombra (230,231,240) abajo del relleno blanco.
- **Pestañas:** `tab` (inactiva, café) mide 7×15 y `tab.active` (clara) 7×17, las dos con
  centro (3,0,1,alto): se estiran solo a lo ancho. Las esquinas de arriba son redondeadas y
  abajo quedan abiertas. La activa baja sobre las dos filas de la línea grabada de abajo y la
  interrumpe. Se copiaron sin el texto de las pestañas del mockup. Las dibuja el UI kit
  (`crates/cs-ui/src/kit/tabs.rs`, mockup de componentes, frame 9).
  - `tab`: en las filas 11, 13 y 14 el borde es (136,110,106), como en el mockup.
  - `tab.hover` (108,935, 7×15, mismo centro): no está en el diseño; es `tab` con el relleno
    (221,206,191), con el puntero encima.
  - `tab.shade.left` y `tab.shade.right` (80,935 y 87,935, 5×1): el tramado (136,110,106) de la
    fila oscura de la línea grabada, contra cada lado de la pestaña activa (x 22..26 y 100..104
    del mockup, y 125).
  - `tab.arrow.prev` y `tab.arrow.next` (1,935 y 14,935, 12×12): las flechas para recorrer las
    pestañas que no entran, copiadas del mockup (x 159..181, y 165); juntas comparten la columna
    del medio (23 de ancho). `.hover` (27,935 y 40,935): el relleno blanco; `.disabled` (53,935 y
    66,935): la flecha en (189,164,153). Las dos variantes no están en el diseño.
  - `icon.tab_dirty` (○, cambios sin guardar) e `icon.tab_close` (×, sin cambios) miden 5×5.
  - **Pestaña de panel** (la "Layer" del mockup, x 186..219, y 302): `tab.panel.active` (1,952,
    34×17, centro (3,2,28,12)), de borde negro, pegada a la caja del panel (sus 2 filas de abajo
    son el borde y el blanco de arriba del panel), sin el texto. `tab.panel` (37,952, 34×16) y
    `tab.panel.hover` (73,952) no están en el diseño: la misma forma, cerrada abajo, con el
    relleno (221,206,191) o (234,224,221).
  - `panel.box` (94,935, 8×4, centro (3,2,2,1)): la caja del panel: borde negro con las esquinas
    de arriba de 2 px, la fila blanca de arriba, (221,206,191) a la izquierda, (234,224,221)
    adentro y blanco a la derecha (x 262..264 del mockup); la esquina de arriba a la izquierda (la
    tapa la pestaña) y el borde de abajo no están en el diseño. `panel.divider` (103,935, 3×2): el
    divisor de adentro, la fila blanca sobre la (189,164,153) (y 345). El skin creció a 320×1000
    para estas piezas.
  - Home: la casita animada son `icon.home.0`, `icon.home.1` e `icon.home.2` (21×21, casita y
    humo juntos), copiados de los 3 frames de `home animation.aseprite`, de 300 ms cada uno
    (`duration=300`). Van a 3 px del borde
    izquierdo, arriba de todo. `home.ground` (58×1) es el suelo sobre la línea grabada, desde el
    borde izquierdo.
- **Artboards con Pixel UI** (la ventana del frame 4 del diseño, en (209,235), 173×123): el
  marco es `artboard.frame` y la barra (≡, el nombre y ○) es la del UI kit (`title.bar`, abajo).
  Se borraron `window.close`, `window.menu` y `artboard.menu`.
  - `artboard.frame` (24×25, centro (2,15,20,8)) sale de `window` con Lua: el mismo borde
    oscuro y las mismas esquinas redondeadas; la barra de 14 filas (borde, brillo, 11 rosas y
    la línea oscura, como la del kit) y, debajo, un anillo blanco de 1 px pegado al borde y a
    la barra. El resto de adentro es transparente: ahí van el damero y los píxeles.
  - El área de píxeles del artboard es el hueco: 2 px adentro a los lados y abajo (borde y
    blanco) y 15 abajo del borde de arriba (barra y blanco).
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
- **Botones:** `button`, `button.hover`, `button.down` y `button.disabled` (49×17, centro
  (3,3,43,9)) vienen del mockup de componentes (frame 9). La línea clara de abajo es la última
  fila de la pieza; `button.down` no se desplaza (es `button` sin la fila oscura). Los dibuja el
  botón del kit (`crates/cs-ui/src/kit/button.rs`): texto `color.text` con sombra
  `color.background` 1 px abajo (sin sombra con el puntero encima) y, deshabilitado,
  `color.button_text_disabled`.
  - `button.active` (220,100, 49×16, mismo centro que `button`) es la opción
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
  - **Grupo de botones** (`crates/cs-ui/src/kit/group.rs`, mockup de componentes, frame 9): los
    segmentos son `button`, `button.hover`, `button.down` (el elegido o presionado: baja 1 px) y
    `button.disabled` estirados sobre cada uno; entre dos, una sola columna:
    - `button.group.sep` (256,650, 1×17): (89,77,77) arriba, (189,164,153) en las filas 1 a 13
      y (89,77,77) en la del bisel (14) y la de abajo (15); la luz (241,235,219) en la 16. Copiada
      de la x 248 del mockup. Si los dos vecinos bajaron, se dibuja sin la fila de arriba.
    - `button.group.sep.disabled` (258,650, 1×17): no está en el diseño; (136,110,106) arriba
      como el borde de `button.disabled`, (189,164,153) en las filas 1 a 14, (89,77,77) y la luz.
  - **Campos de texto** (`crates/cs-ui/src/kit/input.rs`, mockup de componentes, frame 9, copiados
    píxel por píxel; todos de 17 filas, con centro de 13 filas que no se estira a lo alto):
    - `field.text` (255,675, 6×17, centro (2,1,2,13)): el borde (136,110,106) que desde la fila 7
      pasa a (89,77,77) (con un píxel (136,110,106) en la 8), el blanco, la fila 14 (230,231,240),
      el borde de abajo con la luz (241,235,219) en las esquinas y la luz debajo. Copiado de x
      202..204 y 356..358 del mockup.
    - `field.text.disabled` (263,675): el mismo con el relleno (221,206,191) y la fila 14
      (189,164,153) (x 325..327 y 431..433, y 195).
    - `field.unit` (271,675, 17×17, centro (1,1,14,13)): la caja (189,164,153) de la unidad, con
      su fila 14 (221,206,191) y el borde derecho del campo (x 388..404, y 52, sin el "px").
    - `field.prefix` (290,675, 12×17, centro (2,1,8,13)): la caja del prefijo (la "X" de
      Position X), con el borde izquierdo del campo (x 363..374, y 73, sin la letra).
    - El texto de las cajas lo escribe el campo, centrado, en `color.field_text`; la caja mide
      3 + tinta + 3, por lo menos 11.
    - Colores: `color.field_text_disabled` (189,164,153), el texto de un campo deshabilitado;
      `color.text_selection` (226,114,133) y `color.text_selected` (255,255,255), el bloque de lo
      seleccionado y su texto. El texto sin foco va en `color.field_text` (89,77,77) y con el
      foco en `color.text`.
  - **Botones con ícono** (`kit::button::icon_button_at`): el marco `button` con todos sus
    estados y una pieza `icon.*` centrada en la cara (las filas 1 a 13, entre el brillo y el
    bisel; el píxel de sobra, a la derecha y abajo). Presionado, el ícono baja 1 px con la cara;
    deshabilitado, va a media opacidad. Los íconos llevan el relleno transparente. El botón
    puede tener cualquier alto (la cara son las filas debajo del contorno hasta el bisel).
    - Un ícono de **una sola tinta** (todos sus píxeles del mismo color, como `icon.reset`) se
      porta como el texto: su color con la sombra `background` 1 px abajo (salvo con el puntero
      encima) y, deshabilitado, `color.button_text_disabled`. Copiado del mockup.
  - **LEDs** (cualquier botón puede llevar uno: `kit::button::Led`, y los segmentos del grupo):
    `button.led.on` (261,650) y `button.led.off` (269,650), 6×7: el LED verde (prendido) y el
    morado (apagado), 6×6 con su sombra (221,206,191) abajo, copiados del mockup (x 315 y 362).
    Van donde empezaría el texto (borde, bisel y 4 px adentro), con su borde de arriba en la fila
    de las mayúsculas; el texto, 3 px después. Bajan 1 px con la cara del botón presionado;
    deshabilitado, el LED queda como está. Reemplazan a los puntos viejos de la barra
    (`button.dot.on`, `button.dot.off`, `button.sep` y el color `option_text_off`), que se
    borraron del skin.
  - **Casillas y radios** (`crates/cs-ui/src/kit/check.rs`, mockup de componentes, frame 9; en
    el skin, y 488..514 desde x 30):
    - `check`, `check.on` y `check.mixed` (12×13, sin centro): vacía, marcada (la ✓ verde) e
      intermedia (la raya verde), con la fila de sombra (230,231,240) y la línea clara debajo.
      `.disabled`, copiadas del mockup; `.hover` no está en el diseño: el borde (136,110,106)
      pasa a (89,77,77). Presionadas se ven como `.hover`.
    - `radio` y `radio.on` (10×11): `menu.radio` y `menu.radio.on` con la luz (241,235,219) de
      abajo que tiene el radio deshabilitado del mockup (el normal no la trae; se agregó para que
      los dos queden iguales, como las casillas). `.disabled` del mockup y `.hover` como en las
      casillas.
    - Ocupan el alto de un botón (17): la casilla desde la fila 1 y el radio desde la 2, así su
      marca cae en las filas de las mayúsculas (4 a 9), y el texto 3 px después.
  - **Stepper** (`crates/cs-ui/src/kit/stepper.rs`, mockup de componentes, frame 9, x 301, y 256
    y x 380, y 276; en el skin, y 538 desde x 30): − [número] + en 17 filas.
    - `stepper.left` y `stepper.right` (15×17): los botones sin ícono, con la columna
      (136,110,106) que los separa del campo. `.hover` (relleno blanco), `.down` (el bisel
      invertido) y `.disabled` (borde tenue) no están en el diseño. En el mínimo (o el máximo), su
      botón se deshabilita.
    - `icon.minus` e `icon.plus` (165,538 y 173,538, 7×7, de una tinta (89,77,77)): centrados en
      la cara del botón, con la sombra blanca 1 px abajo como en el mockup (salvo con el puntero
      encima); presionado bajan 1 px.
    - `stepper.field` (158,538, 1×17, se estira; `.disabled` en 160,538): el campo, con el
      número centrado en `color.text`, las mayúsculas en la fila 5; se arrastra o se escribe.
    - `stepper.unit` (162,538, 1×17, se estira): la caja de la unidad ("ms") pegada al botón
      derecho, 3 + tinta + 3 (por lo menos 11), con el texto centrado en `color.field_text`.
  - **Barra de título** (`crates/cs-ui/src/kit/title_bar.rs`, mockup de componentes, frame 9,
    x 480, y 236 y 268; en el skin, y 0 desde x 230): la de los diálogos, Preview y los
    artboards.
    - `title.bar` (7×14, centro (3,0,1,14)): borde (5,4,3) con las esquinas de arriba
      redondeadas, brillo (246,162,168) arriba y a la derecha, bisel (202,97,115), relleno
      (226,114,133) y la fila oscura (178,82,102) abajo.
    - `icon.title_menu` (6×5, el ≡ desde la columna 4), `icon.title_close` (5×5, el ○ 4 px antes
      del borde) e `icon.title_close.hover` (la × con el puntero encima), de una tinta
      (100,54,75), en las filas 5 a 9. El título, 2 px después del ≡ (sin ≡, desde la columna
      4), con las mayúsculas en la fila 4; si no entra, se corta con "...".
    - Sin el foco, los íconos y el título van en el color de la fila oscura (178,82,102).
    - Los marcos `window` y `window.active` (64×50, centro (3,15,58,32)) y `artboard.frame`
      traen la barra de 14 filas; la del kit se dibuja encima.
  - **Select** (`crates/cs-ui/src/kit/select.rs`, mockup de componentes, frame 9: cerrado en
    x 469, y 321 y 378; abierto en x 454, y 480; en el skin, y 488 desde x 172). Todos los
    combos de la app (los de la barra y `widgets::combo`) son este.
    - Cerrado: el campo del kit (`field.text`, `.disabled`) con lo elegido 5 px adentro (las
      mayúsculas en la fila 5, `color.text`; deshabilitado, `color.field_text_disabled`) y
      `select.arrow` (195,488, 5×3, de una tinta), del color del texto, en las filas 7 a 9 y 4 px
      antes del borde derecho. Ancho justo: 5 + tinta + 5 + 5 + 4 + 1.
    - `select.open` (172,488, 6×16, centro (2,1,2,13)): el campo abierto: 14 filas de blanco y la
      fila de sombra, que queda sobre el borde del panel; sin el borde de abajo.
    - `select.list` (180,488, 8×13, centro (2,7,1,1)): el panel, desde la fila 15 del campo y 11
      px más ancho: borde (5,4,3), la fila blanca de arriba, bisel (221,206,191) a la izquierda y
      abajo y blanco a la derecha, relleno (234,224,221), la esquina de arriba a la derecha en
      escalera y la sombra de 2 px abajo a la derecha, negra al 47% (sobre (221,206,191) da el
      (117,109,101) del mockup).
    - Las opciones: filas de 15 desde la fila 4 del panel (debajo, 1 de aire, el bisel y el
      borde), el texto 6 px adentro del borde y las mayúsculas en su fila 4.
      `select.item.hover` (190,488, 3×15, centro (1,0,1,15)): la franja rosa (178,82,102) /
      (226,114,133) / (246,162,168) con el puntero encima; sin el puntero en el panel, va en la
      elegida.
  - **Slider** (`crates/cs-ui/src/kit/slider.rs`, mockup de componentes, frame 9, x 681..820,
    y 379):
    - `slider.track` (30,520, 16×9, centro (1,0,14,9)): la pista, 7 filas (189,164,153), la
      oscura (136,110,106) y la línea clara, con las puntas recortadas. Se estira a lo ancho.
      `slider.track.disabled` (48,520): sin la fila oscura (no está en el diseño).
    - `slider.thumb` (130,296, 13×14): la perilla de 11 con una columna de sombra (136,110,106) a
      cada lado sobre la pista. `.hover` (66,518: el relleno blanco, también arrastrándola) y
      `.disabled` (81,518: el borde (136,110,106) y el bisel claro) no están en el diseño.
    - **Con encabezado** (`labeled_slider`, mockup x 660..821, y 411..441): arriba, el botón de
      restaurar (el botón con ícono del kit a 13×15, con `icon.reset`), el nombre 3 px después y
      el valor pegado a la derecha, en `color.text` y con las mayúsculas en la fila 4 del botón;
      debajo, el slider. En el default, el botón queda deshabilitado.
    - `icon.reset` (160,520, 7×7): el ↺, de una sola tinta (89,77,77).
    - Ocupa el alto de un botón: la perilla desde la fila 1, la pista desde la 4. La perilla va de
      punta a punta: en el mínimo, su sombra izquierda en la primera columna de la pista.
  - **Botón de color** (`crates/cs-ui/src/kit/color_button.rs`, mockup de componentes, frame 9,
    x 472, y 423):
    - `swatch.frame` (145,296, 16×17, centro (1,8,9,5)): el borde (89,77,77) con el interior
      transparente; encima del color, el brillo de arriba a la derecha (blanco y blanco al 34%) y
      la fila de sombra (negro al 38%), que en el mockup son píxeles del amarillo (sobre ese
      amarillo dan los mismos colores). Debajo, la línea clara.
    - `.hover` (100,518: velo blanco al 19%), `.down` (118,518: velo negro al 12%) y `.disabled`
      (136,518: el borde (136,110,106), el color lavado con (221,206,191) y sin brillo) no están
      en el diseño.
    - Con alfa, detrás del color va el damero de `fgbg.checker`.
    - **Abierto** (mockup x 342..511, y 614..777): un clic abre el selector de la app. El botón
      queda en `picker.tab` (185,537, 20×20: el fondito alrededor de la muestra, que va 2 px
      adentro sin su línea clara), unida a `picker.panel` (206,537, 7×8, centro (3,3,1,1): borde
      (89,77,77) de una línea, fila blanca arriba, bisel (221,206,191) a la izquierda y abajo,
      blanco a la derecha), que empieza en la fila 18 de la pestaña. El selector va 7 px adentro
      a los lados, 3 debajo del borde y 4 arriba del de abajo; mide 156 de ancho. Un clic en la
      muestra o afuera lo cierra.
    - En el selector, el grupo RGBA, cuando va solo en su fila (el hex abajo), reparte lo que
      sobra entre sus cuatro tramos y llega al borde derecho.

- **Paleta y editor de color** (frame 4 del diseño, capa Mockup, columna x 784..972; el skin
  creció a 320×272 y estas piezas van en y 236..267):
  - `palette.box` (9×9, centro (4,4,1,1)): la caja de la paleta, con la línea grabada
    (189,164,153), 1 px de aire y la línea (89,77,77), esquinas redondeadas; adentro,
    transparente.
  - `palette.swatch` (13×13, centro (1,1,11,11)): el borde negro de un cuadrito, sin el color.
    Van uno cada 12 (comparten el borde), 4 px adentro de la caja.
  - `palette.add` (6×7): la marca Ⅲ que va a (3,3) de la celda siguiente al último color;
    agrega el color primario a la paleta.
  - `icon.copy` (4,252, 9×9, relleno transparente): las dos hojas del botón de copiar (copia el
    color en hex). El botón es el botón con ícono del UI kit (abajo), de 25 de ancho; antes era
    una pieza entera (`picker.copy`) sin estados.
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

- **Íconos, cursores y el campo de texto** (de los frames 4 y 5 de `Design.aseprite`; el skin
  creció a 320×429 y van en y 404..420). Antes estaban dibujados en el código; ahora se editan
  acá. Se dibujan a su tamaño (o centrados en su botón), sin estirarse.
  - Secciones del dock (Artboards, Slices): `icon.section.add` (7×7), `icon.section.trash`
    (11×11) e `icon.section.canvas_size` (11×9); desactivados, la app los apaga.
  - Botones del dock (centrados en `button` de 23×24): `icon.dock.artboards` (13×13),
    `icon.dock.library` (13×13), `icon.dock.slices` (19×17) e `icon.dock.lospec` (9×13).
  - Barra de estado (apoyados en la fila 17 de la barra): `icon.status.artboard` (13×13),
    `icon.status.pointer` (11×11) e `icon.status.frame` (11×12).
  - Home: `home.icon.new` (6×7) y `home.icon.open` (10×8) en los botones; `home.icon.files`
    (13×11) y `home.icon.folders` (14×9) en los títulos de las listas.
  - `bar.info` (12×13): el ícono de info de la barra de opciones (las ayudas, en su tooltip).
  - Cursores del lienzo: `cursor.cross` (7×7, el centro cae en el puntero), `cursor.eyedropper`
    (16×16, la punta abajo a la izquierda) y `cursor.rotate` (15×15, centrado). En la cruz y el
    cuentagotas lo oscuro es tinta y lo claro su contrario: cada píxel se pinta negro o blanco
    según lo que tenga debajo, como en Aseprite. El de rotar se dibuja tal cual. Si falta una
    pieza, se usa el cursor de siempre.
  - Colores: `color.link` (178,82,102), el nombre de cada reciente de Home, y `color.path`
    (105,102,130), su ruta.

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
  - − [campo] + de la opacidad y la duración: el stepper del UI kit (abajo; las piezas
    `timeline.stepper.*` se borraron). `timeline.combo` (13×17, centro (2,0,1,17)): el combo
    del modo de mezcla (sin las marcas oscuras sueltas del borde del diseño).
  - `timeline.icon.clock` (16×18) y `timeline.divider` (11×3, centro (5,0,1,3)): la línea
    grabada entre la barra y la tabla, con sus uniones a los lados de la caja.
  - `timeline.eye` / `timeline.eye.closed` (9×7): capa visible / oculta.
    `timeline.cel` / `timeline.cel.empty` (7×7): cel con imagen / vacío. El candado de la tabla
    es `palette.icon.lock`; el abierto, `palette.icon.lock.open` (7×9), es el segundo de los
    botones sueltos de la paleta, con el relleno transparente.
