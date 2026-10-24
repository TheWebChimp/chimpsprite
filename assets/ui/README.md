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
- **Botones de simetría:** `sym.slot` (eje apagado) y `sym.slot.active` (prendido) miden 13×14,
  sin centro. El cuadro de 11×11 empieza en (1,1) y su interior de 9×9, transparente, en (2,2):
  ahí va el ícono `icon.sym_vertical`, `icon.sym_horizontal`, `icon.sym_diag_down` o
  `icon.sym_diag_up` (9×9, fondo transparente).
- **Pestañas:** `tab` (inactiva, café) mide 7×15 y `tab.active` (clara) 7×17, las dos con
  centro (3,0,1,alto): se estiran solo a lo ancho. Las esquinas de arriba son redondeadas y
  abajo quedan abiertas. La activa baja sobre las dos filas de la línea grabada de abajo y la
  interrumpe. Se copiaron sin el texto de las pestañas del mockup.
  - `icon.tab_dirty` (○, cambios sin guardar) e `icon.tab_close` (×, sin cambios) miden 5×5. El
    mockup no muestra otro aspecto al pasar el puntero: no hay `.hover`.
  - Home: la casita animada son `icon.home.0`, `icon.home.1` e `icon.home.2` (21×21, casita y
    humo juntos), copiados de los 3 frames de `home animation.aseprite`. Van a 3 px del borde
    izquierdo, arriba de todo. `home.ground` (58×1) es el suelo sobre la línea grabada, desde el
    borde izquierdo.
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
  las pestañas.
- **Origen:** los íconos, los marcos y los botones se copiaron del mockup del autor
  (`.design/Design.aseprite`, que no se versiona).
- **Botones:** `button`, `button.hover` y `button.down` vienen del mockup del diálogo "Unsaved
  changes".

### Probar cambios en vivo (escritorio)

    CHIMPSPRITE_SKIN=/ruta/a/skin.aseprite cargo run -p chimpsprite

Con Pixel UI encendido, la app vuelve a leer ese archivo al guardarlo (lo revisa una vez por
segundo). Si el archivo tiene un error, se queda el skin anterior y lo avisa en el log.
