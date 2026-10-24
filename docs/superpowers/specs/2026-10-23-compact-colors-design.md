# Colores del diseño, barra de opciones de 19 px y simetría sumable

Fecha: 2026-10-23 · Estado: aprobado

## Contexto

El usuario quiere que la UI en pixel art se vea con los colores de su diseño y que sea más
compacta. La referencia es el frame 3 de `.design/Design.aseprite` (exportado como
`design-2.png`):

- **Colores:**
  - Fondo de la app (menú, pestañas, barra de opciones, paneles): crema (221,206,191).
  - Alrededor del lienzo: café claro (189,164,153).
  - Panel de la toolbar: (234,224,221).
  - Scrollbars: morado y lila.
  - Texto: oscuro.
- **Barra de opciones:** de 19 px de alto. A la derecha dice "Symmetry" y tiene 4 botones chicos
  con ícono: eje vertical │, eje horizontal ─, diagonal ╲ y diagonal ╱.
- **Simetría "sumable":** los 4 botones se prenden y apagan por separado.

Todo esto aplica solo con **Pixel UI encendido**. Con Pixel UI apagado la app se ve igual que
hoy, salvo la simetría, que cambia de modelo en los dos modos.

## 1. Colores en toda la app

### Skin
- Se agregan dos muestras al skin, copiadas del frame 3:
  - `color.background`: el fondo de la app.
  - `color.canvas`: el color alrededor del lienzo.
- Ambas son slices de 8×8 junto a las demás `color.*`. `SkinColors` gana los dos campos y, si
  faltan, usa los colores del tema actual.

### Visuals de egui con Pixel UI
`apply_style(ctx, true)` arma los `Visuals` a partir de los colores del skin:

| Elemento de egui | Color del skin |
| --- | --- |
| `panel_fill` | `background` |
| `window_fill` | `panel` |
| `extreme_bg_color` (campos de texto, fondo de sliders) | `panel` |
| `faint_bg_color` | entre `background` y `panel` |
| `override_text_color` | `text` |
| texto débil | `text_dim` |
| `selection.bg_fill` | `selection`, con `stroke` en `outline` |
| widgets `inactive`/`hovered`/`active` | rellenos derivados de `panel` y `background`, borde `outline` |
| scrollbars | `scroll`, con el "handle" en `selection` |
| `hyperlink_color` | `accent` |
| sombras | ninguna |

- Sigue sin redondeo, como hoy.
- Por legibilidad, el tema queda claro (`dark_mode = false`).
- El fondo del lienzo (el área central alrededor de los artboards) usa `canvas` con Pixel UI.
- El tablero de cuadros de transparencia y los colores propios del lienzo (guías, ejes de
  simetría, selección) no cambian.
- Al apagar Pixel UI vuelve el tema oscuro de hoy, completo.

## 2. Barra de opciones de 19 px

- Con Pixel UI encendido, `OPTIONS_BAR_HEIGHT` es **19 puntos** (19 px de skin). Apagado sigue
  en 32.
- Los controles de la barra se ajustan para caber:
  - sliders y campos de 13–15 pt de alto;
  - botones de forma y de simetría de 13×13;
  - menos espaciado vertical.
  
  Nada se sale de la barra. Ya hay un test que lo revisa; se extiende a 19.

## 3. Simetría sumable

### Modelo (`cs-core`)
- `Symmetry.mode: SymmetryMode` se reemplaza por `axes: SymmetryAxes { vertical, horizontal,
  diagonal_down, diagonal_up: bool }`.
  - `vertical`: el eje vertical, espejo izquierda↔derecha.
  - `horizontal`: el eje horizontal, espejo arriba↔abajo.
- **Simetrías que se aplican:** el grupo que generan los ejes prendidos.
  - Ninguno: solo el trazo original.
  - Uno: ese espejo.
  - │ y ─: también la rotación de 180°.
  - ╲ y ╱: los dos espejos diagonales y la rotación de 180°. Este caso es nuevo.
  - Cualquier eje recto junto con una diagonal: las 8 simetrías del cuadrado.
  - El cálculo es puro y con tests en `cs-raster/symmetry.rs`, como hoy.
- **Compatibilidad:** al leer un documento o un autosave con el campo viejo `mode`, se convierte:
  `Horizontal` → vertical, `Vertical` → horizontal, `Both` → los dos rectos, `DiagonalDown` →
  ╲, `DiagonalUp` → ╱, `All` → los 4.
  - Eso incluye el `.chimp`. El `.aseprite` no guarda simetría.
  - Al guardar se escribe el formato nuevo.
- **Centro:** el centro y el eje que se arrastra con Ctrl siguen igual.
- **Ejes visibles:** los ejes punteados en el lienzo se dibujan para cada eje prendido.

### Interfaz
- **Barra de opciones:** "Symmetry" y 4 botones de prender y apagar, en el orden del mockup: │ ─
  ╲ ╱. Cada uno cambia solo su eje.
- **Íconos:** `icon.sym_vertical`, `icon.sym_horizontal`, `icon.sym_diag_down` e
  `icon.sym_diag_up`, copiados del frame 3 al skin.
  - Si falta un ícono, se dibuja la línea con figuras.
  - Si los botones del mockup llevan marco, se usa `tool.slot` a escala chica; si no, se copia
    el marco del mockup como `sym.slot` y `sym.slot.active`.
- **Tooltips:** "Vertical axis (left ↔ right)", "Horizontal axis (top ↔ bottom)", "Diagonal ╲" y
  "Diagonal ╱", más la ayuda de Ctrl+arrastrar.
- **Sin Pixel UI:** los mismos 4 botones de prender y apagar, dibujados con figuras.

### MCP / cs-agent
- Si usan `SymmetryMode`, se adaptan a `SymmetryAxes`.
- En la API pública del MCP, el valor viejo se sigue aceptando y se convierte, para no romper a
  quien ya lo use.

## Tests

- **Skin:** existen `color.background`, `color.canvas` y los 4 íconos de simetría, con los
  colores y los píxeles exactos del mockup.
- **Visuals:**
  - Con Pixel UI, `panel_fill` es `background` y el texto es `text`.
  - Al apagar Pixel UI vuelve el tema de hoy.
- **Barra:** con Pixel UI mide 19 y todos sus nodos caben, para Pencil, Magic Wand y Text.
- **Grupo de simetría:** hay un test por cada combinación de ejes que importa, y la cantidad de
  transformaciones es 1, 2, 4 u 8.
- **Compatibilidad:** un `.chimp` o autosave con `mode: "Both"` se lee como │+─, y así con cada
  valor viejo.
- **Interfaz** (harness con eventos crudos): clic en │ y luego en ─ prende los dos. Un trazo con
  │+─ pinta 4 puntos.
