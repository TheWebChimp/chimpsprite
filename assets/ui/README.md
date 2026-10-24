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
  `outline`, `selection`, `scroll`, `background`, `canvas`) dan el color de su píxel central.
  Con Pixel UI, `background` es el fondo de la app (menú, pestañas, barra de opciones y paneles)
  y `canvas`, el del área alrededor de los artboards.
- **Variantes:** `.hover`, `.down` y `.active` son opcionales; si falta una, se usa la pieza
  base.
- **Piezas sin dibujar:** una pieza totalmente transparente cuenta como que no existe; la app
  dibuja un respaldo plano con los colores del skin.
- **Íconos de herramientas:** `icon.<herramienta>` miden 17×17 sobre fondo transparente (los
  nombres están en `icon_piece`, `crates/cs-ui/src/tool_group.rs`). Se dibujan centrados en el
  marco.
- **Marcos de herramienta:** `tool.slot` (normal) y `tool.slot.active` (la herramienta elegida)
  miden 23×23, con centro (2,2,19,19). `tool.slot.hover` está sin dibujar y cae al normal.
- **Botones de simetría:** `sym.slot` (eje apagado) y `sym.slot.active` (prendido) miden 13×14,
  sin centro. El cuadro de 11×11 empieza en (1,1) y su interior de 9×9, transparente, en (2,2):
  ahí va el ícono `icon.sym_vertical`, `icon.sym_horizontal`, `icon.sym_diag_down` o
  `icon.sym_diag_up` (9×9, fondo transparente).
- **Origen:** los íconos, los marcos y los botones se copiaron del mockup del autor
  (`.design/Design.aseprite`, que no se versiona).
- **Botones:** `button`, `button.hover` y `button.down` vienen del mockup del diálogo "Unsaved
  changes".

### Probar cambios en vivo (escritorio)

    CHIMPSPRITE_SKIN=/ruta/a/skin.aseprite cargo run -p chimpsprite

Con Pixel UI encendido, la app vuelve a leer ese archivo al guardarlo (lo revisa una vez por
segundo). Si el archivo tiene un error, se queda el skin anterior y lo avisa en el log.
