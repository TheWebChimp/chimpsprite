# Toolbar de solo íconos (UI en pixel art)

Fecha: 2026-10-23 · Estado: aprobado

## Contexto

En la UI en pixel art, el usuario quiere la toolbar **solo con íconos, sin nombre**. Los íconos
vienen de su mockup (`.design/Design.aseprite`, frame 2 en la numeración de Aseprite):

- **La columna de la toolbar** (x≈33–56, y≈76–362) tiene los 13 íconos de los grupos, en orden:
  Marquee, Hand (mano rosa), Pencil, Eraser, Eyedropper, Paint Bucket, Line, Rectangle, Text
  ("Abc"), Contour (remolino), Blur (dedo), Move (flechas en cruz) y Artboard (hoja con marcas de
  corte).
- **Al lado** (aprox. x 325–620, y 85–340) hay íconos sueltos. El usuario confirmó que estos
  son las variantes:

| Variante | Ícono del mockup |
| --- | --- |
| Elliptical Marquee | círculo punteado |
| Lasso | lazo libre con colita |
| Polygonal Lasso | lazo rectangular con colita |
| Magic Wand | varita con estrella |
| Spray | lata de spray |
| Gradient | cuadro azul degradado |
| Jumble | dado / cubo lila con puntitos |
| Ellipse (figura) | óvalo |
| Slice | navaja / cúter azul |

- **Los marcos** (`tool.slot`):
  - Cada ícono va en un cuadrito claro con borde oscuro.
  - En la fila de los dos lápices hay un marco claro (normal) y uno café (activo).
  - Hay también un cuadrito vacío que sirve de referencia para el marco sin ícono.

La toolbar con nombres (sin Pixel UI) no cambia.

## Objetivo

- Con Pixel UI encendido, la toolbar es una columna de botones con ícono.
- Cada botón muestra el ícono de la herramienta activa de su grupo (la variante elegida).
- Variantes con clic derecho o manteniendo presionado, igual que hoy, pero con los íconos en la
  lista.
- Tooltip con el nombre y el atajo.

## 1. Skin

Los íconos y los marcos se copian de `Design.aseprite` a `assets/ui/skin.aseprite` con un script
de Aseprite en modo batch, como se hizo con los botones.

- Todo se copia solo leyendo `.design/`; nunca se modifica.

### Íconos
- Un slice `icon.<id>` de 16×16 por herramienta, sobre fondo transparente. Lleva solo el dibujo,
  sin el marco del cuadrito.
- `<id>` es el nombre en minúsculas de la herramienta:
  - Los 13 principales: `marquee`, `hand`, `pencil`, `eraser`, `eyedropper`, `bucket`, `line`,
    `rectangle`, `text`, `contour`, `blur`, `move`, `artboard`.
  - Las 9 variantes: `ellipse_marquee`, `lasso`, `polygon_lasso`, `magic_wand`, `spray`,
    `gradient`, `jumble`, `ellipse`, `slice`.
- Los slices `icon.*` que ya existen y son de herramientas, se reutilizan. Los demás se crean.
- Si en el mockup un ícono no mide 16×16, se centra en la celda de 16×16. Si es más grande, se
  avisa en el reporte y no se recorta sin revisar.

### Marcos
- **`tool.slot`:** el cuadrito claro vacío, con 9-slice para que el borde no se deforme.
- **`tool.slot.active`:** el cuadrito café del lápiz activo, sin el lápiz.
- **`tool.slot.hover`:** queda sin dibujar (cae al normal) hasta que el usuario lo diseñe.

### Respaldo
- Si falta un ícono, el botón muestra la inicial del nombre de la herramienta con la fuente.
- Si falta el marco, se usa el respaldo plano de siempre.

## 2. Botón de grupo (`crates/cs-ui/src/tool_group.rs`)

Se separa la lógica de un grupo para que la usen las dos toolbars.

- **Funciones:**
  - `group_button(ui, editor, group_index, draw: impl FnOnce(&mut Ui, ToolKind, ButtonState) -> Response)`.
  - Clic: elige la herramienta del grupo que se recuerda.
  - Clic derecho o mantener 400 ms: abre la lista de variantes. Se conserva la regla de que
    soltar después de mantener no cuenta como clic.
  - Elegir en la lista: actualiza la herramienta y la que se recuerda.
- **La variante recordada de cada grupo** pasa al `Editor` (`group_choice: [ToolKind; 13]`, o
  lo que encaje con `GROUPS`). Ya no vive en la memoria de egui con un id que cambia.
  - Se guarda junto con `ToolOptions` en lugar de aparte.
- **En la lista** cada variante tiene su ícono (16×16 a la escala de la UI) y su nombre.
  - Sin Pixel UI, la lista queda como hoy, solo con nombres.
- **Tooltip:**
  - "Nombre (Atajo)".
  - En los grupos con variantes se agrega una segunda línea: "Right-click or hold for more".

## 3. Toolbar de íconos (`panels.rs`)

- Con Pixel UI encendido (`skin()` disponible), `toolbar` dibuja una columna con los 13 grupos.
  - Cada botón es `tool.slot` (o `tool.slot.active` si su grupo tiene la herramienta activa) a
    tamaño de pieza, con el ícono centrado.
  - Al presionar, el ícono baja 1 px, igual que en los botones.
- **Marca de variantes:** los grupos con variantes llevan un triangulito de 3×3 px de skin en
  la esquina inferior derecha.
  - Se dibuja con figuras, sin glifos.
  - Usa el color `text_dim` del skin.
- **Debajo de la columna,** las secciones Options y Symmetry siguen como hoy.
- **Panel:** el ancho actual de 184 se mantiene, y la columna de íconos va alineada a la
  izquierda.

## 4. Arreglos pendientes de la revisión anterior

- **Scroll del panel de herramientas:** solo con la rueda y la barra.
  - Se usa `scroll_source` sin arrastre, para que mantener presionado un botón no haga scroll.
- **Trazo a medias:** si a mitad de un trazo se entra al modo máscara o al modo Tiles, el gesto
  se cancela (`cancel_gesture`), para que el trazo no quede colgado.
- **Nombre de test:** se renombra `tools_blocked_in_tiles_mode_hide_their_outline` según lo
  que de verdad prueba.

## 5. Tests

- **Skin:**
  - Están los 22 `icon.*`, cada uno de 16×16 y no transparente.
  - `tool.slot` y `tool.slot.active` existen.
  - La copia coincide píxel a píxel con el mockup; se verifica con el script.
- **Variante recordada:**
  - Elegir Lasso en el grupo Marquee y cambiar de herramienta: un clic en el grupo vuelve a
    Lasso.
  - Sobrevive a guardar y leer `ToolOptions`.
  - Un ajuste guardado sin ella usa la primera herramienta de cada grupo.
- **Toolbar de íconos** (harness con eventos crudos, Pixel UI encendido):
  - Hay 13 botones con su nombre accesible.
  - Un clic elige.
  - Clic derecho abre la lista con 5 variantes en Marquee.
  - Mantener presionado abre la lista.
  - Con Lasso elegido, el botón del grupo dibuja el ícono `icon.lasso`.
- **Scroll:** arrastrar sobre un botón no mueve el scroll, y mantenerlo presionado abre la
  lista.
- **Cancelar:** entrar al modo máscara con un trazo de Blur a medias deja `is_painting()` en
  falso.
