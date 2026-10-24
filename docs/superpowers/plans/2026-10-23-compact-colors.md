# Colores del diseño, barra de opciones de 19 px y simetría sumable: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Con Pixel UI encendido, la app toma los colores del mockup (fondo crema, lienzo café claro)
y la barra de opciones mide 19 px. En los dos modos, la simetría pasa a ser 4 ejes que se prenden
por separado (│ ─ ╲ ╱), y se aplica el grupo que generan. Los documentos, autosaves y llamadas MCP
con el `mode` viejo se siguen leyendo.

**Architecture:**
- **`assets/ui/skin.aseprite`:** las muestras `color.background` y `color.canvas`, los marcos
  `sym.slot` / `sym.slot.active` (13×14) y los íconos `icon.sym_*` (9×9). Se copian del frame 3 de
  `.design/Design.aseprite` con un script Lua en modo batch.
- **`cs-core::document`:** `SymmetryAxes` (4 `bool`) reemplaza a `SymmetryMode`.
  - `Symmetry` se escribe con `axes`.
  - Se lee con `#[serde(from = "SymmetryRepr")]`, que acepta `axes` y el `mode` viejo (un enum
    privado, `LegacyMode`).
- **`cs-raster::symmetry`:** el grupo de simetrías sale de una tabla de matrices enteras de 2×2 (1,
  2, 4 u 8 transformaciones) en vez de cerrar espejos. `symmetric_points` y `symmetrize` reciben
  `SymmetryAxes`.
- **`cs-ui`:**
  - **`appearance`:** `pixel_visuals(&SkinColors)` arma los `Visuals`, y
    `apply_style(ctx, Option<&SkinColors>)` los aplica.
  - **`skin`:** `SkinColors` gana `background` y `canvas`.
  - **`canvas`:** `CanvasView::background` (con `background_for(skin)`) pinta alrededor de los
    artboards.
  - **`panels`:** cuatro botones de prender y apagar (`SymAxis`), y la barra de opciones
    `options_bar_height(skin)` / `options_bar_frame` / `tool_options_bar(ui, editor, skin)`.
  - **`pixel_ui`:** `toggle_button`, que dibuja `sym.slot` con su ícono.

**Tech Stack:**
- Rust 2024, egui/eframe 0.36 y serde / serde_json.
- Solo para la Task 1: Aseprite 1.3 en modo batch y Python con Pillow para verificar.

**Spec:** `docs/superpowers/specs/2026-10-23-compact-colors-design.md`.

## Global Constraints

- **Crates del motor:** `cs-core`, `cs-raster`, `cs-commands`, `cs-tools` y `cs-io` no dependen de
  egui y compilan a wasm32. Este plan toca `cs-core`, `cs-raster` y `cs-io` sin agregarles
  dependencias.
- **Idiomas:** código y comentarios en español; textos de la UI en inglés. Imita la densidad de
  comentarios del código vecino.
- **Glifos:** cada glifo de la UI tiene que existir en las fuentes de egui o en `chimpsprite.ttf`.
  - Revisado con fontTools: ╲ (U+2572) y ╱ (U+2571) solo están en Hack, que es monoespaciada. No
    están en Ubuntu-Light, NotoEmoji, emoji-icon-font ni `chimpsprite.ttf`. Los tooltips usan `\`
    y `/`.
  - ↔ (U+2194) está en NotoEmoji, que es parte de la familia proporcional.
- **Toolchain aislado:** antes de cualquier comando cargo, en Git Bash:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH`
- **Verificación por tarea:** `cargo fmt --all` y los tests de los crates tocados.
- **Verificación final:**
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo check -p cs-ui --target wasm32-unknown-unknown`
- **Disco:** en D: quedan unos 8 GB.
  - No se crean otros `target`.
  - Si hace falta una copia de prueba, usa `CARGO_TARGET_DIR=D:/Work/chimpdeas/chimpsprite/target` y
    se borra al terminar.
- **GUI:**
  - Se compila solo con `CARGO_TARGET_DIR=target/gui-pixel`.
  - Si el exe está bloqueado porque el usuario lo tiene abierto, se salta la prueba manual.
  - Nunca se mata un proceso que no se lanzó en esta sesión.
  - Antes de abrir la app se copia `%APPDATA%/ChimpSprite/data/app.ron`, y se restaura al
    terminar.
- **`.design/`:** no se modifica nada adentro. `Design.aseprite` solo se abre para leer. Se
  verifica con sha256 antes y después.
- **Commits:** terminan con la línea `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Sin push ni PR.**
- **Rama:** `compact-colors`. No cambiar de rama.

## Desviaciones/decisiones (tomadas al planear)

Las medidas se tomaron en el mockup: `Design.aseprite`, frame 3 de Aseprite (el 2 desde 0 en la
CLI). El frame se volvió a exportar al planear y es idéntico, píxel por píxel, a `design-2.png`.

### Colores

- **Fondo de la app:** (221,206,191), por ejemplo en (917,72), dentro de la barra.
- **Alrededor del lienzo:** (189,164,153), en (300,300).
  - Es el mismo café del relleno de `tool.slot.active` y de la línea bajo la barra de opciones.

### Barra de opciones

- La barra va de y=70 a y=88: 19 px.
  - Arriba, una línea clara (241,235,219).
  - En el medio, 17 filas de fondo.
  - Abajo, una línea (189,164,153).
- "Symmetry" y los 4 botones están a la derecha.

### Botones de simetría

- **Celdas:** cada botón ocupa una celda de 13×14.
  - Las 4 celdas están en x = 920, 934, 948 y 962 (cada 14 px), en el orden │ ─ ╲ ╱.
  - La fila de la barra empieza en y=73. Hay una segunda copia en y=105.
- **Partes de cada botón:**
  - Un cuadro de 11×11 con borde (136,110,106) y esquinas redondeadas. Empieza en (1,1) de la
    celda.
  - Su interior, de 9×9, empieza en (2,2). Ahí va el ícono, sobre el fondo de la barra.
  - Los íconos son una línea de 9 px punteada en dos tonos, (136,110,106) y (189,164,153): │ en
    x=4, ─ en y=4, ╲ en x=y y ╱ en x+y=8.
- **Los botones llevan marco propio, distinto de `tool.slot`.** Se copia como `sym.slot` y
  `sym.slot.active`, en vez de usar `tool.slot` chico.
- **Hay dos estados dibujados:**
  - **La fila de la barra (y=73) es el normal (`sym.slot`).** Tiene un brillo (241,235,219) abajo.
  - **La copia de y=105 es el prendido (`sym.slot.active`).** Tiene un halo (241,235,219) alrededor
    y una sombra (189,164,153) abajo y en las esquinas de abajo.
  - El mockup no dice cuál es cuál. Se toma la copia de abajo como prendida porque es la única
    diferencia, y no es la que está en la barra.
  - El script comprueba que los íconos son idénticos en las dos filas y que los 4 marcos de cada
    fila son iguales.
- **Tamaño 13×14, no 13×13 como dice el spec.** El brillo de abajo (normal) y el halo (prendido)
  ocupan una fila más que el cuadro.
  - Los botones de forma del pincel sí son de 13×13. Se dibujan con figuras y no tienen pieza en
    el mockup.
  - Los dos caben en la barra: 14 ≤ 19.
- **Sin 9-slice:** las piezas se dibujan a su tamaño.
  - El fondo de la barra y el interior del cuadro quedan transparentes en los marcos.
  - En los íconos, el fondo queda transparente.

### Ubicación en `skin.aseprite` (320×232)

- Hoy hay 48 slices, y lo dibujado ocupa estas filas: y 4–11, 20–67, 100–122, 128–144, 150–166 y
  176–192.
- **Muestras de color:** 8×8 junto a las demás `color.*`, que van cada 12 px:
  - `color.background` en (100,4);
  - `color.canvas` en (112,4).
- **Fila y=200:** `sym.slot` en (4,200,13,14) y `sym.slot.active` en (21,200,13,14).
- **Fila y=218:** los íconos de 9×9 en x = 4, 17, 30 y 43: `icon.sym_vertical`,
  `icon.sym_horizontal`, `icon.sym_diag_down` e `icon.sym_diag_up`.
- **Sin superposiciones:**
  - La zona y 196–231 está vacía, y la de las muestras nuevas también.
  - El script falla (`assertEmpty`) si va a pintar sobre algo.
  - Quedan 56 slices.

### Colores en egui (`pixel_visuals`)

- **El tema sigue siendo `Theme::Dark` para egui.** `theme::apply` lo fija con `set_theme`, y se
  cambian los `Visuals` de ese tema.
  - Con Pixel UI, esos `Visuals` son claros (`dark_mode = false`), como pide el spec.
  - Al apagar Pixel UI, `theme::apply` vuelve a poner los oscuros de hoy. Un test compara con
    `theme::apply` sobre un contexto nuevo.
- **Scrollbars:** egui no deja dar color propio a la barra de scroll.
  - El fondo de la barra es `extreme_bg_color`, el mismo de los campos de texto. El spec lo pide
    en `panel`, así que el fondo de la barra queda `panel`, no `scroll`.
  - La manija usa `widgets.*.bg_fill`, porque se pone `spacing.scroll.foreground_color = false`
    con Pixel UI:
    - en reposo: `selection` (lila);
    - al pasar o arrastrar: `scroll` (morado).
  - Efecto lateral: las casillas, los radios y los sliders, que también usan `bg_fill`, quedan
    lila.
- **Fondos de los widgets (`weak_bg_fill`, botones y combos):**
  - `inactive`: `panel`;
  - `hovered`: mitad `panel` y mitad `background`;
  - `active` y `open`: `background`.
  - Todos con borde `outline` y texto `text`.
- **Separadores:** `noninteractive.bg_stroke` en `canvas`, como la línea bajo la barra del mockup.
  - El spec no lo dice. `outline` (casi negro) marcaba demasiado.
- **Otros colores:**
  - `faint_bg_color` y el hover salen de mezclar `background` y `panel` al 50%, con
    `lerp_to_gamma`.
  - `code_bg_color` es `panel`, y `window_stroke` es `outline`.
  - Sin sombras: `Shadow::NONE` en ventanas y popups.
- **`apply_style(ctx, pixel: Option<&SkinColors>)`** reemplaza al `bool` de hoy. Para armar los
  `Visuals` necesita los colores.
- **Recarga en vivo del skin (`CHIMPSPRITE_SKIN`):** un skin recargado vuelve a aplicar sus colores.
  - Usa un campo nuevo, `PixelUi::skin_changed`.
  - No se usa `applied = None`, que haría que el zoom del usuario se pisara al apagar Pixel UI.
- **Fondo del lienzo:** `CanvasView::background`, que `lib.rs` pone en cada frame con
  `canvas::background_for(skin)`.
  - Las reglas, el damero, las guías y los ejes no cambian.

### Barra de 19

- **Altos:**
  - `OPTIONS_BAR_HEIGHT` sigue siendo `const` 32.
  - Se agregan `PIXEL_OPTIONS_BAR_HEIGHT = 19.0` y `options_bar_height(skin)`.
- **Margen:** el marco del panel con Pixel UI (`options_bar_frame`) va sin margen vertical,
  `Margin::symmetric(8, 0)`.
  - Medido al planear con la fuente pixel a 16: el texto mide 15 de alto; los sliders, campos y
    combos, 15; los botones de simetría, 14; y los de forma, 13.
  - Todo cabe en 19.
- **Controles con Pixel UI:**
  - `interact_size.y = 13`;
  - `button_padding.y = 0` e `item_spacing.y = 0`;
  - la barra de scroll de la fila es flotante.
- **Firmas:**
  - `tool_options_bar(ui, editor, skin)` recibe el skin.
  - `tool_options(ui, editor, compact)` y `brush_shape_buttons(ui, shape, side)` ganan un
    parámetro.

### Simetría

- **Nombres viejos:** el `mode` viejo nombraba la dirección del espejo, no el eje.
  - `Horizontal` (izquierda ↔ derecha) pasa a `vertical`.
  - `Vertical` (arriba ↔ abajo) pasa a `horizontal`.
  - `Both` pasa a │+─; `DiagonalDown`, a ╲; `DiagonalUp`, a ╱; y `All`, a los 4.
- **Formato:** `Symmetry` se escribe `{"axes":{…},"center2":…}`.
  - Al leer, si vienen `axes` y `mode`, manda `axes`. Sin ninguno, todo apagado.
  - `SymmetryAxes` lleva `#[serde(default)]`: un eje que falta es `false`.
- **El grupo va por tabla, no cerrando espejos.** Esto arregla un cuelgue de hoy.
  - Hoy, con ╲ y un centro cuyos x e y en medios píxeles tienen distinta paridad, el espejo ╲
    redondeado no es su propio inverso. Por ejemplo, el centro por defecto de un sprite de 8×7.
  - Componerlo consigo mismo da una traslación, y el `while` de `symmetric_points` no termina
    nunca.
  - Se comprobó replicando las fórmulas en Python: con 8×7, `DiagonalDown` y `All` crecen sin fin.
  - Con la tabla siempre son 1, 2, 4 u 8 copias. Un test lo cubre con un centro de (8,7).
- **Fórmulas:** con centros pares, las transformaciones dan exactamente lo mismo que las fórmulas
  de hoy. Los tests de hoy se mantienen con los nombres nuevos.
- **Botones:** `SymAxis` (privado en `panels.rs`) dice el ícono, si un eje está prendido y cómo
  cambiarlo.
  - No hay botón "Off": todo apagado es "Off".
- **Tooltip y nombre accesible:**
  - "Vertical axis (left ↔ right)"
  - "Horizontal axis (top ↔ bottom)"
  - "Diagonal axis \\"
  - "Diagonal axis /"
  - Con algún eje prendido, se agrega "Ctrl+drag the dashed axes to move them".
  - Hoy, "Diagonal ╲" sale como un cuadro vacío, porque ese glifo no está en las fuentes.
- **Ctrl+arrastrar:**
  - El centro siempre.
  - La línea vertical, si │ está prendido; la horizontal, si ─ está prendido.
  - Con solo diagonales, solo el centro, igual que hoy.
- **Ícono que falta en el skin:** se dibuja la línea con figuras (`axis_shapes`) en el interior
  del marco.
- **Sin Pixel UI:** los 4 botones, de 22×22 como hoy y dibujados con figuras.

### Compatibilidad

- **`.chimp` y autosave:** los dos pasan por `cs_io::chimp::read`, y su `manifest.json` es JSON.
  - El autosave es `[{name, chimp: base64(.chimp)}]`.
  - El test del autosave usa un autosave real escrito con el código de `d45ccca`: un sprite de
    4×4 con `{"mode":"Both","center2":[3,5]}`.
- **Sin otros lugares que cambien:**
  - `.aseprite` no guarda simetría.
  - `app.ron` tampoco: guarda `ToolOptions`, la vista y los recientes.
- **MCP / cs-agent:** no usan `SymmetryMode`.
  - El único camino es `apply_commands`, que recibe `DocCommand` crudos (`SetSymmetry`,
    `AddArtboard`) como JSON.
  - El `mode` viejo sigue entrando por el `serde(from)`. Se agrega un test, y el esquema MCP no
    cambia.

### Prototipo y medición

- **Prototipo:** todo el código de este plan se probó al planear, en una copia del repo con el
  `target` principal:
  - `cargo test -p cs-ui`: 313 tests en verde;
  - `cargo test --workspace`: en verde;
  - `cargo clippy --workspace --all-targets -- -D warnings`: limpio;
  - `cargo check` / `clippy -p cs-ui --target wasm32-unknown-unknown`: limpio.
  - La copia comparte `target` con el repo: la primera compilación de cada Task recompila los
    crates del workspace. Es lo esperado.
- **Aseprite abierto:** al planear, el usuario tenía abierto `Aseprite.exe`. No se cierra.
  - Huella de `Design.aseprite` al planear: sha256 `89fce03fbf9b6517a5f3364bad83ecf04546d72258af072ec8626e65c8c1edad`, mtime 2026-10-23 19:44:32.
  - Si en la Task 1 no coincide, no se asume nada: se mira el mtime y se repite la verificación de
    píxeles con un frame 3 recién exportado.

## Archivos

| Archivo | Qué hace |
| --- | --- |
| Modificar `assets/ui/skin.aseprite` | `color.background`, `color.canvas`, `sym.slot`, `sym.slot.active`, `icon.sym_*` |
| Modificar `assets/ui/README.md` | Colores nuevos y piezas de simetría |
| Modificar `crates/cs-ui/src/skin/mod.rs` | `COLOR_NAMES`, `SkinColors::{background, canvas}`; tests |
| Modificar `crates/cs-ui/src/appearance.rs` | `pixel_visuals`, `apply_style(ctx, Option<&SkinColors>)`, `skin_changed`; test |
| Modificar `crates/cs-ui/src/canvas.rs` | `background`, `background_for`; ejes por eje; tests |
| Modificar `crates/cs-ui/src/lib.rs` | Pone el fondo del lienzo; la barra con `skin` y 19 de alto |
| Modificar `crates/cs-ui/src/layout_tests.rs` | Tests de la app con Pixel UI (fondo y barra) |
| Modificar `crates/cs-core/src/document.rs` | `SymmetryAxes`, `LegacyMode`, `SymmetryRepr`; tests de serde |
| Modificar `crates/cs-raster/src/symmetry.rs` | Grupo por tabla; tests |
| Modificar `crates/cs-raster/src/lib.rs` | Reexporta `SymmetryAxes` |
| Modificar `crates/cs-io/src/chimp.rs` | Fixture con ejes; test del manifiesto viejo |
| Modificar `crates/cs-agent/src/ops/document.rs` | Test de `apply_commands` con `mode` |
| Modificar `crates/cs-ui/src/autosave.rs` | Test del autosave viejo |
| Modificar `crates/cs-ui/src/editor.rs` | `mirror` con `axes`; test |
| Modificar `crates/cs-ui/src/demo.rs`, `crates/cs-ui/src/tool_tests.rs` | `axes` en vez de `mode` |
| Modificar `crates/cs-ui/src/panels.rs` | `SymAxis`, 4 botones, barra de 19; tests y `BarHarness` |
| Modificar `crates/cs-ui/src/pixel_ui.rs` | `toggle_button`; tests |

---

### Task 1: Skin: colores de fondo y del lienzo, marcos e íconos de simetría del mockup

**Files:**
- Modify: `assets/ui/skin.aseprite`
- Modify: `assets/ui/README.md`
- Modify: `crates/cs-ui/src/skin/mod.rs` (test)
- Scripts temporales (no se versionan): `/tmp/chimpsprite-copy-sym.lua`,
  `/tmp/chimpsprite-slices.lua` y `/tmp/chimpsprite-sym-check.py`

**Interfaces:**
- Produces:
  - Colores del skin `background` = (221,206,191) y `canvas` = (189,164,153), en
    `SkinData::colors`. Los slices `color.*` son de 8×8, en (100,4) y (112,4).
  - Slices sin centro:
    - `sym.slot` (4,200,13,14) y `sym.slot.active` (21,200,13,14);
    - `icon.sym_vertical` (4,218,9,9), `icon.sym_horizontal` (17,218,9,9),
      `icon.sym_diag_down` (30,218,9,9) e `icon.sym_diag_up` (43,218,9,9).
  - Las tasks siguientes los leen con `crate::skin::EMBEDDED` (ya existe).

- [ ] **Step 1: Escribir el test del skin (falla con el skin de hoy)**

En `crates/cs-ui/src/skin/mod.rs`, dentro de `mod tests`, antes de
`fn variants_are_used_when_they_exist`:

```rust
    #[test]
    fn the_embedded_skin_has_the_mockup_background_canvas_and_symmetry_buttons() {
        let data = decode(EMBEDDED).unwrap();
        assert_eq!(data.colors["background"], Rgba::rgb(221, 206, 191));
        assert_eq!(data.colors["canvas"], Rgba::rgb(189, 164, 153));
        let alpha = |piece: &PieceImage, x: u32, y: u32| {
            piece.rgba[((y * piece.size[0] + x) * 4 + 3) as usize]
        };
        for name in ["sym.slot", "sym.slot.active"] {
            let slot = &data.pieces[name];
            assert_eq!((slot.size, slot.center), ([13, 14], None), "{name}");
            // El borde del cuadro de 11×11 empieza en (1,1); su interior (el del ícono) está vacío.
            assert_eq!(alpha(slot, 2, 1), 255, "{name}: top border");
            assert_eq!(alpha(slot, 1, 2), 255, "{name}: left border");
            assert_eq!(alpha(slot, 6, 6), 0, "{name}: empty inside");
        }
        // Lo dibujado de cada ícono de 9×9 es su eje: una línea de 9 píxeles.
        for name in [
            "icon.sym_vertical",
            "icon.sym_horizontal",
            "icon.sym_diag_down",
            "icon.sym_diag_up",
        ] {
            let icon = &data.pieces[name];
            assert_eq!((icon.size, icon.center), ([9, 9], None), "{name}");
            for y in 0..9 {
                for x in 0..9 {
                    let on_axis = match name {
                        "icon.sym_vertical" => x == 4,
                        "icon.sym_horizontal" => y == 4,
                        "icon.sym_diag_down" => x == y,
                        _ => x + y == 8,
                    };
                    assert_eq!(alpha(icon, x, y) == 255, on_axis, "{name} ({x},{y})");
                }
            }
        }
    }
```

- [ ] **Step 2: Correrlo y comprobar que falla**

Run: `cargo test -p cs-ui the_embedded_skin_has_the_mockup_background`
Expected: FAIL con un panic de `data.colors["background"]`: la clave no existe.

- [ ] **Step 3: Huella de `.design/` y PNG del skin de antes**

En Git Bash, desde la raíz del repo:

```bash
ASEPRITE="/c/Program Files (x86)/Steam/steamapps/common/Aseprite/Aseprite.exe"
find .design -type f -print0 | sort -z | xargs -0 sha256sum > /tmp/chimpsprite-design-before.sha
grep Design.aseprite /tmp/chimpsprite-design-before.sha
ls -l --time-style=full-iso .design/Design.aseprite
"$ASEPRITE" -b assets/ui/skin.aseprite --save-as /tmp/chimpsprite-skin-before.png
```

Anotar el sha y la hora que muestra `ls`. Al planear eran `89fce03fbf9b6517a5f3364bad83ecf04546d72258af072ec8626e65c8c1edad`
y `2026-10-23 19:44:32`.

- [ ] **Step 4: Escribir el script que copia colores, marcos e íconos**

Crear `/tmp/chimpsprite-copy-sym.lua`:

```lua
-- Copia al skin los colores de fondo y del lienzo, los marcos de los botones de simetría y sus
-- 4 íconos, del mockup (Design.aseprite, frame 3 en la numeración de Aseprite = frame 3 de Lua).
-- Uso: Aseprite.exe -b --script-param design=<Design.aseprite> --script-param skin=<skin.aseprite>
--      --script chimpsprite-copy-sym.lua
-- Design.aseprite solo se lee: se aplana en memoria y se cierra sin guardar.
local designPath = app.params["design"]
local skinPath = app.params["skin"]
assert(designPath and skinPath, "faltan --script-param design=... y skin=...")

local rgba = app.pixelColor.rgba
-- Fondo de la app (y de la barra de opciones, detrás de los botones).
local BACKGROUND = rgba(221, 206, 191, 255)
-- Alrededor del lienzo.
local CANVAS = rgba(189, 164, 153, 255)
-- Borde de los botones de simetría.
local FRAME = rgba(136, 110, 106, 255)
local CLEAR = rgba(0, 0, 0, 0)

-- Cada botón ocupa una celda de 13x14: el cuadro de 11x11 empieza en (1,1) y su interior de
-- 9x9 (donde va el ícono) en (2,2). Abajo y alrededor van los brillos y la sombra del mockup.
local CELL_W, CELL_H = 13, 14
local ICON = 9
local INSET = 2
-- Las 4 celdas de una fila van cada 14 px, en el orden de la barra: │ ─ ╲ ╱.
local PITCH = 14
local NAMES = { "sym_vertical", "sym_horizontal", "sym_diag_down", "sym_diag_up" }
-- Fila de arriba: los botones normales, en la barra de opciones (y 70–88 del mockup).
local NORMAL = { x = 920, y = 73 }
-- Fila de abajo: la copia con brillo alrededor, que se toma como el botón prendido.
local ACTIVE = { x = 920, y = 105 }

-- Destinos en el skin (zona libre: y 196–231).
local SWATCHES = {
  { name = "color.background", color = BACKGROUND, x = 100, y = 4 },
  { name = "color.canvas",     color = CANVAS,     x = 112, y = 4 },
}
local SLOTS = {
  { name = "sym.slot",        src = NORMAL, x = 4,  y = 200 },
  { name = "sym.slot.active", src = ACTIVE, x = 21, y = 200 },
}
local ICONS_Y = 218
local function iconX(i) return 4 + 13 * (i - 1) end

-- Mockup aplanado, frame 3.
local design = app.open(designPath)
assert(#design.frames >= 3, "el mockup no tiene frame 3")
local flat = Image(design.spec)
flat:drawSprite(design, 3)
design:close()

-- Los colores se comprueban en el mockup, no se suponen.
assert(flat:getPixel(917, 72) == BACKGROUND, "(917,72) no es el fondo de la barra")
assert(flat:getPixel(300, 300) == CANVAS, "(300,300) no es el color del lienzo")

local function inInterior(x, y)
  return x >= INSET and x < INSET + ICON and y >= INSET and y < INSET + ICON
end

-- El marco (todo menos el interior) de las 4 celdas de una fila es el mismo: solo cambia el ícono.
local function assertSameFrames(row, label)
  for i = 2, 4 do
    local ox = row.x + PITCH * (i - 1)
    for y = 0, CELL_H - 1 do
      for x = 0, CELL_W - 1 do
        if not inInterior(x, y) then
          assert(flat:getPixel(ox + x, row.y + y) == flat:getPixel(row.x + x, row.y + y),
            string.format("%s: el marco %d difiere en (%d,%d)", label, i, ox + x, row.y + y))
        end
      end
    end
  end
  -- El cuadro de 11x11 está donde se espera: borde arriba e izquierda.
  assert(flat:getPixel(row.x + 2, row.y + 1) == FRAME, label .. ": no está el borde de arriba")
  assert(flat:getPixel(row.x + 1, row.y + 2) == FRAME, label .. ": no está el borde izquierdo")
end
assertSameFrames(NORMAL, "normal")
assertSameFrames(ACTIVE, "active")

-- Los íconos de las dos filas son iguales: el estado solo cambia el marco.
for i = 1, 4 do
  local dx = PITCH * (i - 1)
  for y = INSET, INSET + ICON - 1 do
    for x = INSET, INSET + ICON - 1 do
      assert(flat:getPixel(NORMAL.x + dx + x, NORMAL.y + y)
          == flat:getPixel(ACTIVE.x + dx + x, ACTIVE.y + y),
        string.format("%s: el ícono cambia entre filas en (%d,%d)", NAMES[i], x, y))
    end
  end
end

local skin = app.open(skinPath)
local layer
for _, l in ipairs(skin.layers) do
  -- "Skin (dibuja aquí)": se compara el prefijo para no depender del acento.
  if l.name:sub(1, 4) == "Skin" then layer = l end
end
assert(layer, "no está la capa 'Skin (dibuja aquí)'")

-- La capa a tamaño de lienzo (el cel puede estar recortado a lo dibujado).
local full = Image(skin.spec)
local cel = layer:cel(1)
if cel then full:drawImage(cel.image, cel.position) end

local function sliceNamed(name)
  for _, s in ipairs(skin.slices) do
    if s.name == name then return s end
  end
  return nil
end

-- Pone (o crea) el slice `name` en `bounds`, sin centro.
local function setSlice(name, bounds)
  local s = sliceNamed(name)
  if not s then
    s = skin:newSlice(bounds)
    s.name = name
  end
  s.bounds = bounds
  s.center = nil
end

-- Lo que se va a pintar tiene que estar vacío: no se pisa ninguna pieza.
local function assertEmpty(x0, y0, w, h, name)
  for y = y0, y0 + h - 1 do
    for x = x0, x0 + w - 1 do
      assert(app.pixelColor.rgbaA(full:getPixel(x, y)) == 0,
        string.format("%s: (%d,%d) del skin ya tiene algo", name, x, y))
    end
  end
end

for _, s in ipairs(SWATCHES) do
  assertEmpty(s.x, s.y, 8, 8, s.name)
  for y = 0, 7 do
    for x = 0, 7 do full:drawPixel(s.x + x, s.y + y, s.color) end
  end
  setSlice(s.name, Rectangle(s.x, s.y, 8, 8))
end

for _, s in ipairs(SLOTS) do
  assertEmpty(s.x, s.y, CELL_W, CELL_H, s.name)
  for y = 0, CELL_H - 1 do
    for x = 0, CELL_W - 1 do
      local px = flat:getPixel(s.src.x + x, s.src.y + y)
      -- El fondo de la barra y el interior (el ícono va aparte) quedan transparentes.
      if px == BACKGROUND or inInterior(x, y) then px = CLEAR end
      full:drawPixel(s.x + x, s.y + y, px)
    end
  end
  setSlice(s.name, Rectangle(s.x, s.y, CELL_W, CELL_H))
end

for i, name in ipairs(NAMES) do
  local sx = NORMAL.x + PITCH * (i - 1) + INSET
  local sy = NORMAL.y + INSET
  local dx = iconX(i)
  assertEmpty(dx, ICONS_Y, ICON, ICON, name)
  for y = 0, ICON - 1 do
    for x = 0, ICON - 1 do
      local px = flat:getPixel(sx + x, sy + y)
      if px == BACKGROUND then px = CLEAR end
      full:drawPixel(dx + x, ICONS_Y + y, px)
    end
  end
  setSlice("icon." .. name, Rectangle(dx, ICONS_Y, ICON, ICON))
end

skin:newCel(layer, 1, full, Point(0, 0))
skin:saveAs(skinPath)
skin:close()
print("ok")
```

- [ ] **Step 5: Correr el script sobre `assets/ui/skin.aseprite`**

```bash
"$ASEPRITE" -b --script-param design=.design/Design.aseprite --script-param skin=assets/ui/skin.aseprite --script /tmp/chimpsprite-copy-sym.lua
```

Expected: imprime `ok`. Si un `assert` falla, el mockup cambió o el skin ya tiene algo en esos
lugares. En ese caso: no tocar `.design/`, avisar con el mensaje del assert y parar.

- [ ] **Step 6: Verificar los slices**

Crear `/tmp/chimpsprite-slices.lua` (solo lee):

```lua
-- Lista capas y slices (bounds y centro) del sprite abierto, sin modificarlo.
local s = app.activeSprite
for i, l in ipairs(s.layers) do print("layer", i, l.name, "visible", l.isVisible) end
for _, sl in ipairs(s.slices) do
  local b, c = sl.bounds, sl.center
  local cs = c and string.format("%d,%d,%d,%d", c.x, c.y, c.width, c.height) or "-"
  print("slice", sl.name, b.x, b.y, b.width, b.height, "center", cs)
end
```

```bash
"$ASEPRITE" -b assets/ui/skin.aseprite --script /tmp/chimpsprite-slices.lua | grep -E '^slice	(color\.(background|canvas)|sym|icon\.sym)' | sort
"$ASEPRITE" -b assets/ui/skin.aseprite --script /tmp/chimpsprite-slices.lua | grep -c '^slice'
```

Expected (separado por tabs):

```
slice	color.background	100	4	8	8	center	-
slice	color.canvas	112	4	8	8	center	-
slice	icon.sym_diag_down	30	218	9	9	center	-
slice	icon.sym_diag_up	43	218	9	9	center	-
slice	icon.sym_horizontal	17	218	9	9	center	-
slice	icon.sym_vertical	4	218	9	9	center	-
slice	sym.slot	4	200	13	14	center	-
slice	sym.slot.active	21	200	13	14	center	-
```

y `56`: eran 48 slices, y se agregan 8.

- [ ] **Step 7: Verificar los píxeles contra el mockup**

```bash
"$ASEPRITE" -b assets/ui/skin.aseprite --save-as /tmp/chimpsprite-skin-after.png
"$ASEPRITE" -b .design/Design.aseprite --frame-range 2,2 --save-as /tmp/chimpsprite-design-frame3.png
```

`--frame-range` cuenta desde 0: `2,2` es el frame 3 de Aseprite.

Crear `/tmp/chimpsprite-sym-check.py`:

```python
# Compara los colores, marcos e íconos de simetría del skin con el mockup (frame 3), y revisa
# que fuera de esos rectángulos el skin no cambió.
# Uso: python chimpsprite-sym-check.py <skin-antes.png> <skin-después.png> <design-frame3.png>
import sys
from PIL import Image

before = Image.open(sys.argv[1]).convert("RGBA")
skin = Image.open(sys.argv[2]).convert("RGBA")
design = Image.open(sys.argv[3]).convert("RGBA")
BACKGROUND = (221, 206, 191, 255)
CANVAS = (189, 164, 153, 255)

# Muestras de color: 8x8 lisas, del color medido en el mockup.
for name, x, y, color, probe in [
    ("color.background", 100, 4, BACKGROUND, (917, 72)),
    ("color.canvas", 112, 4, CANVAS, (300, 300)),
]:
    diff = sum(1 for j in range(8) for i in range(8) if skin.getpixel((x + i, y + j)) != color)
    print(f"{name} diff {diff} mockup {design.getpixel(probe) == color}")


def interior(x, y):
    return 2 <= x <= 10 and 2 <= y <= 10


# Marcos de 13x14: igual al mockup salvo el fondo y el interior, que quedan transparentes.
for name, sx, sy, dx in [("sym.slot", 920, 73, 4), ("sym.slot.active", 920, 105, 21)]:
    diff = clear = 0
    for y in range(14):
        for x in range(13):
            p = skin.getpixel((dx + x, 200 + y))
            d = design.getpixel((sx + x, sy + y))
            if p[3] == 0:
                clear += 1
                diff += not (d == BACKGROUND or interior(x, y))
            else:
                diff += p != d or interior(x, y)
    print(f"{name} diff {diff} clear {clear}")

# Íconos de 9x9: el interior de cada botón normal, con el fondo transparente.
for i, name in enumerate(["sym_vertical", "sym_horizontal", "sym_diag_down", "sym_diag_up"]):
    sx, sy, dx = 920 + 14 * i + 2, 73 + 2, 4 + 13 * i
    diff = drawn = 0
    for y in range(9):
        for x in range(9):
            p = skin.getpixel((dx + x, 218 + y))
            d = design.getpixel((sx + x, sy + y))
            if p[3] == 0:
                diff += d != BACKGROUND
            else:
                drawn += 1
                diff += p != d
    print(f"icon.{name} diff {diff} drawn {drawn}")

rects = [(100, 4, 8, 8), (112, 4, 8, 8), (4, 200, 13, 14), (21, 200, 13, 14)]
rects += [(4 + 13 * i, 218, 9, 9) for i in range(4)]
inside = lambda x, y: any(rx <= x < rx + w and ry <= y < ry + h for rx, ry, w, h in rects)
changed = sum(
    1
    for y in range(before.height)
    for x in range(before.width)
    if not inside(x, y) and before.getpixel((x, y)) != skin.getpixel((x, y))
)
print("size", before.size == skin.size, "changed outside", changed)
```

```bash
python /tmp/chimpsprite-sym-check.py /tmp/chimpsprite-skin-before.png /tmp/chimpsprite-skin-after.png /tmp/chimpsprite-design-frame3.png
```

Git Bash convierte las rutas `/tmp/...` de los argumentos para el Python de Windows.

Expected (medido al planear):

```
color.background diff 0 mockup True
color.canvas diff 0 mockup True
sym.slot diff 0 clear 135
sym.slot.active diff 0 clear 93
icon.sym_vertical diff 0 drawn 9
icon.sym_horizontal diff 0 drawn 9
icon.sym_diag_down diff 0 drawn 9
icon.sym_diag_up diff 0 drawn 9
size True changed outside 0
```

Qué se lee:
- `diff 0`: cada píxel opaco es idéntico al del mockup, y cada transparente era fondo o el
  interior del cuadro.
- `drawn 9`: cada ícono es una línea de 9 píxeles.
- `changed outside 0`: el resto del skin sigue intacto.

- [ ] **Step 8: Comprobar que `.design/` no cambió**

```bash
sha256sum -c /tmp/chimpsprite-design-before.sha
ls -l --time-style=full-iso .design/Design.aseprite
git status --short
```

Expected:
- Todas las líneas terminan en `OK`.
- El mtime es el mismo que en el Step 3.
- `git status` muestra solo `M assets/ui/skin.aseprite`, `M crates/cs-ui/src/skin/mod.rs` y
  `?? .design/`.

Si el sha de `Design.aseprite` cambió:
- Si el mtime es posterior a la corrida y hay un Aseprite abierto por el usuario, lo guardó él.
  En ese caso:
  - Se repite el Step 7 con un frame 3 recién exportado.
  - Se anota en el reporte.
  - No se toca `.design/`.
- Si no, se para y se avisa.

- [ ] **Step 9: Correr el test del skin**

Run: `cargo test -p cs-ui skin::`
Expected: PASS (los tests del skin, incluido el nuevo).

- [ ] **Step 10: README de los assets**

En `assets/ui/README.md`, reemplazar:

```markdown
- **Colores:** los slices `color.*` (`desktop`, `text`, `text_dim`, `accent`, `panel`,
  `outline`, `selection`, `scroll`) dan el color de su píxel central.
```

por:

```markdown
- **Colores:** los slices `color.*` (`desktop`, `text`, `text_dim`, `accent`, `panel`,
  `outline`, `selection`, `scroll`, `background`, `canvas`) dan el color de su píxel central.
  Con Pixel UI, `background` es el fondo de la app (menú, pestañas, barra de opciones y paneles)
  y `canvas`, el del área alrededor de los artboards.
```

Y después de la línea `- **Marcos de herramienta:** …` (y su continuación), agregar:

```markdown
- **Botones de simetría:** `sym.slot` (eje apagado) y `sym.slot.active` (prendido) miden 13×14,
  sin centro. El cuadro de 11×11 empieza en (1,1) y su interior de 9×9, transparente, en (2,2):
  ahí va el ícono `icon.sym_vertical`, `icon.sym_horizontal`, `icon.sym_diag_down` o
  `icon.sym_diag_up` (9×9, fondo transparente).
```

- [ ] **Step 11: Commit**

```bash
cargo fmt --all
git add assets/ui/skin.aseprite assets/ui/README.md crates/cs-ui/src/skin/mod.rs
git commit -m "Skin: colores de fondo y del lienzo, y botones de simetría del mockup

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Pixel UI con los colores del skin (egui y fondo del lienzo)

**Files:**
- Modify: `crates/cs-ui/src/skin/mod.rs`
- Modify: `crates/cs-ui/src/appearance.rs`
- Modify: `crates/cs-ui/src/canvas.rs`
- Modify: `crates/cs-ui/src/lib.rs`
- Modify: `crates/cs-ui/src/panels.rs` (una línea de test)
- Test: `crates/cs-ui/src/layout_tests.rs`

**Interfaces:**
- Consumes: `color.background` y `color.canvas` del skin (Task 1).
- Produces:
  - `SkinColors::{background, canvas}: Color32`. Sin el skin, valen `theme::PANEL` y
    `theme::CANVAS_BG`.
  - `pub fn appearance::pixel_visuals(c: &SkinColors) -> egui::Visuals`.
  - `pub fn appearance::apply_style(ctx: &egui::Context, pixel: Option<&SkinColors>)`.
  - `CanvasView::background: Color32` (`pub`).
  - `pub fn canvas::background_for(skin: Option<&Skin>) -> Color32`.

- [ ] **Step 1: Escribir los tests**

En `crates/cs-ui/src/appearance.rs`, `mod tests`, antes de
`fn switching_off_gives_back_the_users_zoom`:

```rust
    #[test]
    fn with_pixel_ui_the_egui_colors_come_from_the_skin_and_off_they_are_the_theme_again() {
        let ctx = egui::Context::default();
        let mut pixel = PixelUi::from_env();
        let mut view = ViewSettings {
            pixel_ui: true,
            ui_scale: 2,
            ..Default::default()
        };
        frame(&ctx, &mut pixel, &view);
        frame(&ctx, &mut pixel, &view);
        let colors = *pixel.skin().expect("on").colors();
        assert_eq!(colors.background, Color32::from_rgb(221, 206, 191));
        let visuals = ctx.style_of(egui::Theme::Dark).visuals.clone();
        assert_eq!(visuals.panel_fill, colors.background);
        assert_eq!(visuals.override_text_color, Some(colors.text));
        assert_eq!(visuals.weak_text_color, Some(colors.text_dim));
        assert_eq!(visuals.window_fill, colors.panel);
        assert_eq!(visuals.extreme_bg_color, colors.panel);
        assert_eq!(visuals.selection.bg_fill, colors.selection);
        assert_eq!(visuals.widgets.inactive.bg_stroke.color, colors.outline);
        assert_eq!(visuals.window_shadow, Shadow::NONE);
        assert!(!visuals.dark_mode);
        assert_eq!(visuals.widgets.inactive.corner_radius, CornerRadius::ZERO);
        assert!(
            !ctx.style_of(egui::Theme::Dark)
                .spacing
                .scroll
                .foreground_color
        );

        view.pixel_ui = false;
        frame(&ctx, &mut pixel, &view);
        let plain = egui::Context::default();
        theme::apply(&plain);
        assert_eq!(
            ctx.style_of(egui::Theme::Dark).visuals,
            plain.style_of(egui::Theme::Dark).visuals,
            "the dark theme of always"
        );
        assert!(
            ctx.style_of(egui::Theme::Dark)
                .spacing
                .scroll
                .foreground_color
        );
    }
```

En `crates/cs-ui/src/skin/mod.rs`, al final de
`fn missing_variants_fall_back_to_the_base_and_missing_colors_to_the_theme`, después del
`assert_eq!` de `accent`:

```rust
        assert_eq!(skin.colors().background, theme::PANEL);
        assert_eq!(skin.colors().canvas, theme::CANVAS_BG);
```

En `crates/cs-ui/src/canvas.rs`, `mod tests`, antes de
`fn the_round_brush_outline_follows_its_pixels`:

```rust
    #[test]
    fn with_pixel_ui_the_canvas_surround_is_the_skin_canvas_color() {
        let ctx = egui::Context::default();
        let skin = Skin::embedded(&ctx);
        assert_eq!(background_for(None), theme::CANVAS_BG);
        assert_eq!(
            background_for(Some(&skin)),
            Color32::from_rgb(189, 164, 153)
        );
        // El lienzo pinta su fondo con ese color.
        let mut h = Harness::new();
        h.canvas.background = background_for(Some(&skin));
        let mut shapes = Vec::new();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            ..Default::default()
        };
        let output = h.ctx.run_ui(input, |ui| {
            egui::CentralPanel::no_frame().show(ui, |ui| {
                h.canvas.show(ui, &mut h.editor, None, &h.view);
            });
        });
        for clipped in output.shapes {
            shapes.push(clipped.shape);
        }
        let fills_with_canvas = shapes.iter().any(
            |s| matches!(s, egui::Shape::Rect(r) if r.fill == Color32::from_rgb(189, 164, 153)),
        );
        assert!(fills_with_canvas);
    }
```

En `crates/cs-ui/src/layout_tests.rs`, antes de
`fn without_pixel_ui_the_named_toolbar_keeps_its_width`:

```rust
#[test]
fn with_pixel_ui_the_canvas_has_the_skin_canvas_color() {
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let h = LayoutHarness::new(view, true);
    let canvas = h.app.pixel.skin().expect("on").colors().canvas;
    assert_eq!(h.app.doc.canvas.background, canvas);
    // Sin Pixel UI, el fondo del tema.
    let h = LayoutHarness::editor();
    assert_eq!(h.app.doc.canvas.background, crate::theme::CANVAS_BG);
}
```

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui with_pixel_ui`
Expected: FAIL de compilación. Faltan `SkinColors::background`/`canvas`,
`CanvasView::background`, `background_for` y `Shadow`.

- [ ] **Step 3: Colores nuevos en `SkinColors`**

En `crates/cs-ui/src/skin/mod.rs`:

```rust
/// Colores del skin (`color.<nombre>`).
pub const COLOR_NAMES: [&str; 10] = [
    "desktop",
    "text",
    "text_dim",
    "accent",
    "panel",
    "outline",
    "selection",
    "scroll",
    "background",
    "canvas",
];
```

En `pub struct SkinColors`, después de `pub scroll: Color32,`:

```rust
    /// Fondo de la app: menú, pestañas, barra de opciones y paneles.
    pub background: Color32,
    /// Alrededor del lienzo (el área central, detrás de los artboards).
    pub canvas: Color32,
```

En `from_theme`, después de `scroll: theme::WIDGET_HOVER,`:

```rust
            background: theme::PANEL,
            canvas: theme::CANVAS_BG,
```

En `with`, después de `("scroll", &mut c.scroll),`:

```rust
            ("background", &mut c.background),
            ("canvas", &mut c.canvas),
```

- [ ] **Step 4: `pixel_visuals` y `apply_style` con los colores del skin**

En `crates/cs-ui/src/appearance.rs`, cambiar los `use` de arriba por:

```rust
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Shadow,
    Stroke, TextStyle,
};

use crate::skin::{Skin, SkinColors};
```

Reemplazar toda la función `apply_style` (desde su comentario `/// El tema de siempre…` hasta su
`}` final) por:

```rust
/// Mitad y mitad de `a` y `b`.
fn mix(a: Color32, b: Color32) -> Color32 {
    a.lerp_to_gamma(b, 0.5)
}

/// Los colores de egui con Pixel UI, armados con los del skin: tema claro, sin redondeo ni
/// sombras.
/// - Paneles en `background`; ventanas, campos de texto y el fondo de las barras de scroll en
///   `panel`.
/// - Texto en `text` y el débil en `text_dim`; selección `selection` con borde `outline`.
/// - Botones (`weak_bg_fill`) en `panel`, más oscuros al pasar y al presionar; borde `outline`.
/// - `bg_fill` (casillas, sliders y la manija del scroll) en `selection`, y `scroll` al pasar o
///   arrastrar.
/// - Separadores en `canvas`, como la línea bajo la barra del mockup.
pub fn pixel_visuals(c: &SkinColors) -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.dark_mode = false;
    v.override_text_color = Some(c.text);
    v.weak_text_color = Some(c.text_dim);
    v.panel_fill = c.background;
    v.window_fill = c.panel;
    v.window_stroke = Stroke::new(1.0, c.outline);
    v.extreme_bg_color = c.panel;
    v.code_bg_color = c.panel;
    v.faint_bg_color = mix(c.background, c.panel);
    v.selection.bg_fill = c.selection;
    v.selection.stroke = Stroke::new(1.0, c.outline);
    v.hyperlink_color = c.accent;
    v.window_shadow = Shadow::NONE;
    v.popup_shadow = Shadow::NONE;
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c.background;
    w.noninteractive.weak_bg_fill = c.background;
    w.noninteractive.bg_stroke = Stroke::new(1.0, c.canvas);
    w.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
    w.noninteractive.corner_radius = CornerRadius::ZERO;
    let states = [
        (&mut w.inactive, c.panel, c.selection),
        (&mut w.hovered, mix(c.panel, c.background), c.scroll),
        (&mut w.active, c.background, c.scroll),
        (&mut w.open, c.background, c.selection),
    ];
    for (state, weak, strong) in states {
        state.weak_bg_fill = weak;
        state.bg_fill = strong;
        state.bg_stroke = Stroke::new(1.0, c.outline);
        state.fg_stroke = Stroke::new(1.0, c.text);
        state.corner_radius = CornerRadius::ZERO;
        state.expansion = 0.0;
    }
    v
}

/// El tema de siempre. Con `pixel` (los colores del skin), además, la fuente pixel a 16 en los
/// textos proporcionales y los colores de `pixel_visuals`, sin nada redondeado.
pub fn apply_style(ctx: &egui::Context, pixel: Option<&SkinColors>) {
    theme::apply(ctx);
    let defaults = egui::Style::default().text_styles;
    ctx.all_styles_mut(|style| {
        style.text_styles = defaults.clone();
        // La manija del scroll: `bg_fill` con Pixel UI; el color del texto, como siempre, sin él.
        style.spacing.scroll.foreground_color = pixel.is_none();
        let Some(colors) = pixel else {
            return;
        };
        for text in [
            TextStyle::Small,
            TextStyle::Body,
            TextStyle::Button,
            TextStyle::Heading,
        ] {
            style
                .text_styles
                .insert(text, FontId::proportional(PIXEL_FONT_SIZE));
        }
        style.visuals = pixel_visuals(colors);
    });
}
```

En `pub struct PixelUi`, después del campo `scale_ready`:

```rust
    /// El skin se recargó (`CHIMPSPRITE_SKIN`): sus colores se vuelven a aplicar a egui.
    skin_changed: bool,
```

En `PixelUi::from_env`, después de `scale_ready: false,`:

```rust
            skin_changed: false,
```

En `PixelUi::update`, reemplazar:

```rust
        if self.applied == Some(state) {
            return;
        }
        self.applied = Some(state);
        ctx.set_fonts(font_definitions(view.pixel_ui));
        apply_style(ctx, view.pixel_ui);
```

por:

```rust
        if self.applied == Some(state) && !std::mem::take(&mut self.skin_changed) {
            return;
        }
        self.applied = Some(state);
        ctx.set_fonts(font_definitions(view.pixel_ui));
        let colors = self.skin.as_ref().map(|skin| *skin.colors());
        apply_style(ctx, colors.as_ref().filter(|_| view.pixel_ui));
```

En `PixelUi::load_skin`, reemplazar:

```rust
                    Ok(skin) => self.skin = Some(skin),
```

por:

```rust
                    Ok(skin) => {
                        self.skin = Some(skin);
                        // Sus colores se vuelven a aplicar a egui en `update`.
                        self.skin_changed = true;
                    }
```

No se usa `self.applied = None`: `was_on` saldría falso y el zoom del usuario se pisaría.

En `crates/cs-ui/src/panels.rs`, test `with_pixel_ui_style_every_node_of_the_options_bar_fits_inside_it`,
reemplazar:

```rust
        crate::appearance::apply_style(&h.ctx, true);
```

por:

```rust
        let skin = Skin::embedded(&h.ctx);
        crate::appearance::apply_style(&h.ctx, Some(skin.colors()));
```

- [ ] **Step 5: El fondo del lienzo**

En `crates/cs-ui/src/canvas.rs`, después de
`use crate::guides::{RULER_PX, SNAP_PX, SnapContext, after_drag, hit_guide, nice_step};`:

```rust
use crate::skin::Skin;
```

Al final de `pub struct CanvasView`, reemplazar:

```rust
    /// Ctrl+rueda de touchpad: píxeles acumulados que todavía no suman un paso de grosor.
    wheel_rest: f32,
}
```

por:

```rust
    /// Ctrl+rueda de touchpad: píxeles acumulados que todavía no suman un paso de grosor.
    wheel_rest: f32,
    /// Color alrededor de los artboards (`background_for`); lo pone la app en cada frame.
    pub background: Color32,
}

/// Color alrededor de los artboards: `canvas` del skin con Pixel UI; si no, el del tema.
pub fn background_for(skin: Option<&Skin>) -> Color32 {
    skin.map_or(theme::CANVAS_BG, |skin| skin.colors().canvas)
}
```

En `impl Default for CanvasView`, después de `wheel_rest: 0.0,`:

```rust
            background: theme::CANVAS_BG,
```

En `CanvasView::show`, reemplazar:

```rust
        painter.rect_filled(rect, 0.0, theme::CANVAS_BG);
```

por:

```rust
        painter.rect_filled(rect, 0.0, self.background);
```

En `crates/cs-ui/src/lib.rs`, `editor_panels`, justo después de
`let skin = self.pixel.skin();`:

```rust
        // Alrededor de los artboards: el color `canvas` del skin con Pixel UI.
        self.doc.canvas.background = canvas::background_for(skin);
```

- [ ] **Step 6: Correr los tests**

Run: `cargo fmt --all && cargo test -p cs-ui`
Expected: PASS, incluidos `with_pixel_ui_the_egui_colors_come_from_the_skin_…`,
`with_pixel_ui_the_canvas_surround_is_the_skin_canvas_color` y
`with_pixel_ui_the_canvas_has_the_skin_canvas_color`.

- [ ] **Step 7: Commit**

```bash
git add crates/cs-ui/src/skin/mod.rs crates/cs-ui/src/appearance.rs crates/cs-ui/src/canvas.rs crates/cs-ui/src/lib.rs crates/cs-ui/src/panels.rs crates/cs-ui/src/layout_tests.rs
git commit -m "Pixel UI: los colores de egui y el fondo del lienzo salen del skin

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Simetría sumable: `SymmetryAxes`, el grupo y los cuatro botones

**Files:**
- Modify: `crates/cs-core/src/document.rs`
- Modify: `crates/cs-raster/src/symmetry.rs` (completo)
- Modify: `crates/cs-raster/src/lib.rs`
- Modify: `crates/cs-io/src/chimp.rs` (fixture de test)
- Modify: `crates/cs-ui/src/editor.rs`
- Modify: `crates/cs-ui/src/demo.rs`
- Modify: `crates/cs-ui/src/tool_tests.rs`
- Modify: `crates/cs-ui/src/canvas.rs`
- Modify: `crates/cs-ui/src/panels.rs`

**Interfaces:**
- Produces:
  - `pub struct cs_core::SymmetryAxes { pub vertical, pub horizontal, pub diagonal_down, pub diagonal_up: bool }`:
    - es `Copy`, `Default`, `PartialEq`, `Eq`, `Hash`, `Serialize` y `Deserialize`, con
      `#[serde(default)]`;
    - `pub const NONE: Self`;
    - `pub fn any(self) -> bool`.
  - `pub struct cs_core::Symmetry { pub axes: SymmetryAxes, pub center2: Option<(i32, i32)> }`:
    - se escribe con `axes`;
    - se lee con `axes` o con el `mode` viejo.
  - `cs_core::SymmetryMode` deja de existir.
  - `cs_raster::symmetric_points(p: Point, axes: SymmetryAxes, center2: (i32, i32)) -> Vec<Point>`
    y `cs_raster::symmetrize<T: Copy>(pixels: &[(i32, i32, T)], axes: SymmetryAxes, center2) -> Vec<(i32, i32, T)>`.
  - `cs_raster::SymmetryAxes`, reexportado.
  - En `panels.rs`, privados:
    - `enum SymAxis` con `on(SymmetryAxes) -> bool` y `toggled(SymmetryAxes) -> SymmetryAxes`;
    - `const SYMMETRY_AXES: [(SymAxis, &str); 4]`;
    - `symmetry_toggle(ui, SymAxis, &str, bool) -> Response`;
    - `axis_shapes(Rect, SymAxis, Color32) -> Vec<Shape>`.
    - La Task 5 los usa.

- [ ] **Step 1: Tests de serde en `cs-core`**

En `crates/cs-core/src/document.rs`, al final de `mod tests` (antes de su `}`):

```rust
    #[test]
    fn the_old_symmetry_mode_reads_as_axes() {
        let axes = |v, h, d, u| SymmetryAxes {
            vertical: v,
            horizontal: h,
            diagonal_down: d,
            diagonal_up: u,
        };
        let cases = [
            ("None", axes(false, false, false, false)),
            ("Horizontal", axes(true, false, false, false)),
            ("Vertical", axes(false, true, false, false)),
            ("Both", axes(true, true, false, false)),
            ("DiagonalDown", axes(false, false, true, false)),
            ("DiagonalUp", axes(false, false, false, true)),
            ("All", axes(true, true, true, true)),
        ];
        for (mode, expected) in cases {
            // Así escribía la simetría la versión anterior.
            let json = format!(r#"{{"mode":"{mode}","center2":[5,3]}}"#);
            let s: Symmetry = serde_json::from_str(&json).unwrap();
            assert_eq!(s.axes, expected, "{mode}");
            assert_eq!(s.center2, Some((5, 3)), "{mode}");
        }
        let old: Symmetry = serde_json::from_str(r#"{"mode":"Both","center2":null}"#).unwrap();
        assert_eq!(old.center2, None);
    }

    #[test]
    fn the_symmetry_is_written_with_axes_and_reads_back() {
        let s = Symmetry {
            axes: SymmetryAxes {
                vertical: true,
                diagonal_up: true,
                ..SymmetryAxes::NONE
            },
            center2: Some((4, 6)),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            r#"{"axes":{"vertical":true,"horizontal":false,"diagonal_down":false,"diagonal_up":true},"center2":[4,6]}"#
        );
        assert_eq!(serde_json::from_str::<Symmetry>(&json).unwrap(), s);
        // Si vienen los dos, manda `axes`; sin nada, todo apagado.
        let both: Symmetry =
            serde_json::from_str(r#"{"axes":{"horizontal":true},"mode":"All"}"#).unwrap();
        assert_eq!(
            both.axes,
            SymmetryAxes {
                horizontal: true,
                ..SymmetryAxes::NONE
            }
        );
        assert_eq!(
            serde_json::from_str::<Symmetry>("{}").unwrap(),
            Symmetry::default()
        );
    }

    #[test]
    fn an_old_artboard_with_mode_still_loads() {
        let mut json = serde_json::to_value(&doc().artboards[1]).unwrap();
        json["symmetry"] = serde_json::json!({"mode": "DiagonalUp", "center2": null});
        let artboard: Artboard = serde_json::from_value(json).unwrap();
        assert!(artboard.symmetry.axes.diagonal_up);
        assert!(!artboard.symmetry.axes.vertical);
    }
```

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-core symmetry`
Expected: FAIL de compilación: `SymmetryAxes` no existe y `Symmetry` no tiene `axes`.

- [ ] **Step 3: El modelo nuevo en `cs-core`**

En `crates/cs-core/src/document.rs`, reemplazar desde
`/// Modos de simetría para dibujar. Los ejes pasan por un centro en **medios píxeles**.` hasta el
`}` de `pub struct Symmetry` (el enum `SymmetryMode` y la struct) por:

```rust
/// Ejes de simetría para dibujar; se prenden y apagan por separado. Se aplica el grupo de
/// simetrías que generan los ejes prendidos. Los ejes pasan por un centro en **medios píxeles**.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct SymmetryAxes {
    /// Eje vertical │: espejo izquierda ↔ derecha.
    pub vertical: bool,
    /// Eje horizontal ─: espejo arriba ↔ abajo.
    pub horizontal: bool,
    /// Diagonal ╲.
    pub diagonal_down: bool,
    /// Diagonal ╱.
    pub diagonal_up: bool,
}

impl SymmetryAxes {
    /// Todos apagados: solo el trazo original.
    pub const NONE: Self = Self {
        vertical: false,
        horizontal: false,
        diagonal_down: false,
        diagonal_up: false,
    };

    /// `true` si hay al menos un eje prendido.
    pub fn any(self) -> bool {
        self.vertical || self.horizontal || self.diagonal_down || self.diagonal_up
    }
}

/// El modo único de antes (`mode`). Solo se lee, para abrir documentos y autosaves viejos.
#[derive(Deserialize)]
enum LegacyMode {
    None,
    /// Espejo izquierda ↔ derecha: el eje vertical.
    Horizontal,
    /// Espejo arriba ↔ abajo: el eje horizontal.
    Vertical,
    Both,
    DiagonalDown,
    DiagonalUp,
    All,
}

impl From<LegacyMode> for SymmetryAxes {
    fn from(mode: LegacyMode) -> Self {
        let mut axes = SymmetryAxes::NONE;
        match mode {
            LegacyMode::None => {}
            LegacyMode::Horizontal => axes.vertical = true,
            LegacyMode::Vertical => axes.horizontal = true,
            LegacyMode::Both => {
                axes.vertical = true;
                axes.horizontal = true;
            }
            LegacyMode::DiagonalDown => axes.diagonal_down = true,
            LegacyMode::DiagonalUp => axes.diagonal_up = true,
            LegacyMode::All => {
                axes = SymmetryAxes {
                    vertical: true,
                    horizontal: true,
                    diagonal_down: true,
                    diagonal_up: true,
                }
            }
        }
        axes
    }
}

/// Simetría de un artboard. Se escribe con `axes`; al leer también acepta el `mode` de antes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "SymmetryRepr")]
pub struct Symmetry {
    pub axes: SymmetryAxes,
    /// Centro × 2 (medios píxeles); `None` = centro del artboard.
    pub center2: Option<(i32, i32)>,
}

/// Lo que se lee de una simetría: el formato nuevo (`axes`) o el viejo (`mode`). Si vienen los
/// dos, manda `axes`.
#[derive(Deserialize)]
struct SymmetryRepr {
    #[serde(default)]
    axes: Option<SymmetryAxes>,
    #[serde(default)]
    mode: Option<LegacyMode>,
    #[serde(default)]
    center2: Option<(i32, i32)>,
}

impl From<SymmetryRepr> for Symmetry {
    fn from(repr: SymmetryRepr) -> Self {
        Symmetry {
            axes: repr
                .axes
                .or(repr.mode.map(SymmetryAxes::from))
                .unwrap_or_default(),
            center2: repr.center2,
        }
    }
}
```

Run: `cargo test -p cs-core`
Expected: PASS (`cs-core` no usa la simetría en otro lado).

- [ ] **Step 4: El grupo por tabla en `cs-raster` (tests y código)**

Reemplazar todo `crates/cs-raster/src/symmetry.rs` por:

```rust
use std::collections::HashSet;

use crate::Point;

pub use cs_core::SymmetryAxes;

/// Una de las 8 simetrías del cuadrado: matriz entera `[a, b, c, d]` que lleva la distancia al
/// centro, en medios píxeles, de `(dx, dy)` a `(a·dx + b·dy, c·dx + d·dy)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Transform([i32; 4]);

const IDENTITY: Transform = Transform([1, 0, 0, 1]);
/// Espejo izquierda ↔ derecha (eje vertical │).
const FLIP_X: Transform = Transform([-1, 0, 0, 1]);
/// Espejo arriba ↔ abajo (eje horizontal ─).
const FLIP_Y: Transform = Transform([1, 0, 0, -1]);
const ROTATE_180: Transform = Transform([-1, 0, 0, -1]);
/// Espejo sobre la diagonal ╲: (dx, dy) → (dy, dx).
const DIAGONAL_DOWN: Transform = Transform([0, 1, 1, 0]);
/// Espejo sobre la diagonal ╱: (dx, dy) → (−dy, −dx).
const DIAGONAL_UP: Transform = Transform([0, -1, -1, 0]);
const ROTATE_90: Transform = Transform([0, -1, 1, 0]);
const ROTATE_270: Transform = Transform([0, 1, -1, 0]);

/// Las 8 simetrías del cuadrado.
const SQUARE: [Transform; 8] = [
    IDENTITY,
    FLIP_X,
    FLIP_Y,
    ROTATE_180,
    DIAGONAL_DOWN,
    DIAGONAL_UP,
    ROTATE_90,
    ROTATE_270,
];

/// El grupo que generan los ejes prendidos, con la identidad primero. Va por tabla (no se
/// cierran los espejos uno con otro): así siempre son 1, 2, 4 u 8 transformaciones, aunque el
/// centro no caiga en el medio de un píxel para las diagonales.
fn group(axes: SymmetryAxes) -> &'static [Transform] {
    let SymmetryAxes {
        vertical,
        horizontal,
        diagonal_down,
        diagonal_up,
    } = axes;
    match (vertical, horizontal, diagonal_down, diagonal_up) {
        (false, false, false, false) => &[IDENTITY],
        (true, false, false, false) => &[IDENTITY, FLIP_X],
        (false, true, false, false) => &[IDENTITY, FLIP_Y],
        (true, true, false, false) => &[IDENTITY, FLIP_X, FLIP_Y, ROTATE_180],
        (false, false, true, false) => &[IDENTITY, DIAGONAL_DOWN],
        (false, false, false, true) => &[IDENTITY, DIAGONAL_UP],
        (false, false, true, true) => &[IDENTITY, DIAGONAL_DOWN, DIAGONAL_UP, ROTATE_180],
        // Un eje recto con una diagonal genera la rotación de 90°: el cuadrado completo.
        _ => &SQUARE,
    }
}

fn apply(t: Transform, (x, y): Point, (cx2, cy2): (i32, i32)) -> Point {
    let [a, b, c, d] = t.0;
    // Centro del píxel menos el centro de simetría, en medios píxeles.
    let (dx, dy) = (2 * x + 1 - cx2, 2 * y + 1 - cy2);
    let (qx, qy) = (cx2 + a * dx + b * dy, cy2 + c * dx + d * dy);
    // De medios píxeles al píxel. Si el centro cae en un borde (solo pasa con las diagonales y
    // un centro de x e y con distinta paridad), va al píxel de la izquierda o de arriba.
    ((qx - 1).div_euclid(2), (qy - 1).div_euclid(2))
}

/// Todas las copias de `p` (incluido `p`), sin repetir.
pub fn symmetric_points(p: Point, axes: SymmetryAxes, center2: (i32, i32)) -> Vec<Point> {
    let mut out = Vec::with_capacity(8);
    for &t in group(axes) {
        let q = apply(t, p, center2);
        if !out.contains(&q) {
            out.push(q);
        }
    }
    out
}

/// Copias de una lista de píxeles con su valor; el primero que cae en un lugar gana.
pub fn symmetrize<T: Copy>(
    pixels: &[(i32, i32, T)],
    axes: SymmetryAxes,
    center2: (i32, i32),
) -> Vec<(i32, i32, T)> {
    if !axes.any() {
        return pixels.to_vec();
    }
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(pixels.len() * group(axes).len());
    for &(x, y, v) in pixels {
        for (qx, qy) in symmetric_points((x, y), axes, center2) {
            if seen.insert((qx, qy)) {
                out.push((qx, qy, v));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ejes en el orden de la barra: │ ─ ╲ ╱.
    fn axes(
        vertical: bool,
        horizontal: bool,
        diagonal_down: bool,
        diagonal_up: bool,
    ) -> SymmetryAxes {
        SymmetryAxes {
            vertical,
            horizontal,
            diagonal_down,
            diagonal_up,
        }
    }

    const V: SymmetryAxes = SymmetryAxes {
        vertical: true,
        ..SymmetryAxes::NONE
    };
    const H: SymmetryAxes = SymmetryAxes {
        horizontal: true,
        ..SymmetryAxes::NONE
    };
    const DOWN: SymmetryAxes = SymmetryAxes {
        diagonal_down: true,
        ..SymmetryAxes::NONE
    };
    const UP: SymmetryAxes = SymmetryAxes {
        diagonal_up: true,
        ..SymmetryAxes::NONE
    };

    fn sorted(mut v: Vec<Point>) -> Vec<Point> {
        v.sort();
        v
    }

    fn compose(t: Transform, u: Transform) -> Transform {
        let ([a, b, c, d], [e, f, g, h]) = (t.0, u.0);
        Transform([a * e + b * g, a * f + b * h, c * e + d * g, c * f + d * h])
    }

    #[test]
    fn vertical_axis_with_even_and_odd_widths() {
        // Ancho 8: eje entre 3 y 4 (center2 = 8).
        assert_eq!(symmetric_points((1, 2), V, (8, 8)), vec![(1, 2), (6, 2)]);
        // Ancho 7: eje en el centro del píxel 3 (center2 = 7); ese píxel es su propio reflejo.
        assert_eq!(symmetric_points((3, 0), V, (7, 7)), vec![(3, 0)]);
        assert_eq!(symmetric_points((0, 0), V, (7, 7)), vec![(0, 0), (6, 0)]);
    }

    #[test]
    fn horizontal_axis_and_both_straight_axes() {
        assert_eq!(symmetric_points((2, 1), H, (8, 8)), vec![(2, 1), (2, 6)]);
        assert_eq!(
            sorted(symmetric_points((1, 2), axes(true, true, false, false), (8, 8))),
            vec![(1, 2), (1, 5), (6, 2), (6, 5)]
        );
    }

    #[test]
    fn diagonals() {
        // Sprite 8×8: ╲ intercambia x e y; ╱ refleja sobre la antidiagonal.
        assert_eq!(symmetric_points((1, 5), DOWN, (8, 8)), vec![(1, 5), (5, 1)]);
        assert_eq!(symmetric_points((1, 5), UP, (8, 8)), vec![(1, 5), (2, 6)]);
        // Centro desplazado: ╲ por (6, 2) en medios píxeles.
        assert_eq!(symmetric_points((3, 1), DOWN, (12, 4)), vec![(3, 1), (5, -1)]);
    }

    #[test]
    fn both_diagonals_add_the_half_turn() {
        // ╲ + ╱: los dos espejos diagonales y la rotación de 180°.
        assert_eq!(
            sorted(symmetric_points((1, 2), axes(false, false, true, true), (8, 8))),
            vec![(1, 2), (2, 1), (5, 6), (6, 5)]
        );
    }

    #[test]
    fn a_straight_axis_and_a_diagonal_give_the_eight_copies() {
        let eight = vec![
            (1, 2),
            (1, 5),
            (2, 1),
            (2, 6),
            (5, 1),
            (5, 6),
            (6, 2),
            (6, 5),
        ];
        for a in [
            axes(true, false, true, false),
            axes(false, true, false, true),
            axes(true, true, true, false),
            axes(true, true, true, true),
        ] {
            assert_eq!(sorted(symmetric_points((1, 2), a, (8, 8))), eight, "{a:?}");
        }
        // El centro exacto no se duplica.
        assert_eq!(
            symmetric_points((3, 3), axes(true, true, true, true), (7, 7)),
            vec![(3, 3)]
        );
    }

    #[test]
    fn every_combination_is_a_group_of_1_2_4_or_8() {
        for bits in 0..16u8 {
            let a = axes(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            let g = group(a);
            let straight = usize::from(a.vertical) + usize::from(a.horizontal);
            let diagonal = usize::from(a.diagonal_down) + usize::from(a.diagonal_up);
            let expected = match (straight, diagonal) {
                (0, 0) => 1,
                (1, 0) | (0, 1) => 2,
                (2, 0) | (0, 2) => 4,
                _ => 8,
            };
            assert_eq!(g.len(), expected, "{a:?}");
            assert_eq!(g[0], IDENTITY, "{a:?}");
            // Cerrado: componer dos transformaciones del grupo da otra del grupo.
            for &t in g {
                for &u in g {
                    assert!(g.contains(&compose(t, u)), "{a:?}");
                }
            }
        }
    }

    #[test]
    fn an_off_center_diagonal_still_gives_a_few_copies() {
        // 8×7: el centro de ╲ no cae en el medio de un píxel. Cerrar los espejos uno con otro
        // (como antes) no terminaba nunca; por tabla son a lo sumo las del grupo.
        assert_eq!(symmetric_points((1, 2), DOWN, (8, 7)).len(), 2);
        assert!(symmetric_points((1, 2), axes(true, true, true, true), (8, 7)).len() <= 8);
    }

    #[test]
    fn symmetrize_keeps_values_and_dedupes() {
        let px = [(0, 0, 'a'), (7, 0, 'b')];
        let out = symmetrize(&px, V, (8, 8));
        assert_eq!(out, vec![(0, 0, 'a'), (7, 0, 'a')], "first writer wins");
        assert_eq!(symmetrize(&px, SymmetryAxes::NONE, (8, 8)), px.to_vec());
    }
}
```

En `crates/cs-raster/src/lib.rs`, reemplazar:

```rust
pub use symmetry::{SymmetryMode, symmetric_points, symmetrize};
```

por:

```rust
pub use symmetry::{SymmetryAxes, symmetric_points, symmetrize};
```

Run: `cargo fmt --all && cargo test -p cs-raster symmetry`
Expected: PASS (8 tests).

El test `an_off_center_diagonal_still_gives_a_few_copies` documenta el cuelgue de hoy. Con el
código viejo no falla: se cuelga. No hace falta correrlo contra el código viejo.

- [ ] **Step 5: El fixture de `cs-io`**

En `crates/cs-io/src/chimp.rs`, `fn doc()` de `mod tests`, reemplazar:

```rust
            symmetry: cs_core::Symmetry {
                mode: cs_core::SymmetryMode::Both,
                center2: Some((5, 3)),
            },
```

por:

```rust
            symmetry: cs_core::Symmetry {
                axes: cs_core::SymmetryAxes {
                    vertical: true,
                    horizontal: true,
                    ..cs_core::SymmetryAxes::NONE
                },
                center2: Some((5, 3)),
            },
```

Run: `cargo test -p cs-io chimp`
Expected: PASS.

- [ ] **Step 6: Tests de la UI (fallan hasta el Step 8)**

En `crates/cs-ui/src/canvas.rs`, `mod tests`, antes de
`fn the_round_brush_outline_follows_its_pixels`:

```rust
    #[test]
    fn a_click_with_both_straight_axes_paints_four_pixels() {
        let mut h = Harness::new();
        h.editor.set_symmetry(cs_core::Symmetry {
            axes: cs_core::SymmetryAxes {
                vertical: true,
                horizontal: true,
                ..cs_core::SymmetryAxes::NONE
            },
            center2: None,
        });
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        h.click(h.screen((1, 2)), PointerButton::Primary);
        let primary = h.editor.primary;
        let sprite = h.editor.sprite();
        let painted: Vec<(i32, i32)> = (0..16)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .filter(|&(x, y)| sprite.pixel(LayerId(1), 0, x, y) == primary)
            .collect();
        assert_eq!(painted, vec![(1, 2), (14, 2), (1, 13), (14, 13)]);
    }
```

En `crates/cs-ui/src/panels.rs`, `mod tests`, en `impl BarHarness`, después de `fn has`:

```rust
        /// Clic con el botón principal en el centro del nodo `text`, en tres frames.
        fn click(&mut self, text: &str) {
            let at = self.rect(text).center();
            let press = |pressed| Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![Event::PointerMoved(at)]);
            self.frame(vec![press(true)]);
            self.frame(vec![press(false)]);
        }
```

Y antes de `fn the_options_bar_is_one_row_of_the_same_height_with_every_tool`:

```rust
    #[test]
    fn each_symmetry_button_turns_its_own_axis_on_and_off() {
        let mut h = BarHarness::new(1200.0);
        let [vertical, horizontal, down, up] = SYMMETRY_AXES.map(|(_, name)| name);
        h.click(vertical);
        h.click(horizontal);
        let axes = h.editor.symmetry().axes;
        assert!(axes.vertical && axes.horizontal, "{axes:?}");
        assert!(!axes.diagonal_down && !axes.diagonal_up, "{axes:?}");
        h.click(up);
        h.click(vertical);
        assert_eq!(
            h.editor.symmetry().axes,
            SymmetryAxes {
                horizontal: true,
                diagonal_up: true,
                ..SymmetryAxes::NONE
            }
        );
        assert!(!h.editor.symmetry().axes.diagonal_down, "{down} untouched");
        // Cada clic es un paso de undo.
        h.editor.undo();
        assert!(h.editor.symmetry().axes.vertical);
    }
```

Los tests que nombran el botón "Off" pasan a nombrar el eje vertical. En `panels.rs`:
- En `the_options_bar_has_the_brush_with_the_pencil_but_not_with_the_hand`:
  - `assert!(h.has("Off"), "Symmetry");` pasa a
    `assert!(h.has("Vertical axis (left ↔ right)"), "Symmetry");`.
  - `assert!(h.has("Off"), "Symmetry is always there");` pasa a
    `assert!(h.has("Vertical axis (left ↔ right)"), "Symmetry is always there");`.
- En `symmetry_is_at_the_right_end_of_the_bar_in_the_usual_order`:
  - `SYMMETRY_MODES.iter()` pasa a `SYMMETRY_AXES.iter()`.
- En `the_options_scroll_sideways_with_the_wheel_when_they_do_not_fit`:
  - `h.rect("Off")` pasa a `h.rect("Vertical axis (left ↔ right)")`.
- En `the_toolbar_has_only_the_tool_buttons`:
  - `["Options", "Symmetry", "Brush size", "Off"]` pasa a
    `["Options", "Symmetry", "Brush size", "Vertical axis (left ↔ right)"]`.

Run: `cargo test -p cs-ui`
Expected: FAIL de compilación (`SymmetryMode` ya no existe).

- [ ] **Step 7: Lo mecánico de `cs-ui`: espejo, demo, tests y ejes del lienzo**

En `crates/cs-ui/src/editor.rs`, `fn mirror`, reemplazar:

```rust
        } if symmetry.mode != cs_core::SymmetryMode::None => Command::DrawPixels {
            layer,
            frame,
            pixels: cs_raster::symmetrize(&pixels, symmetry.mode, center2),
```

por:

```rust
        } if symmetry.axes.any() => Command::DrawPixels {
            layer,
            frame,
            pixels: cs_raster::symmetrize(&pixels, symmetry.axes, center2),
```

En `editor.rs`, test `symmetric_strokes_are_one_undo_step_and_respect_selection`, reemplazar:

```rust
        use cs_core::{Symmetry, SymmetryMode};
        let mut e = editor(); // 8×8
        e.set_symmetry(Symmetry {
            mode: SymmetryMode::Horizontal,
```

por:

```rust
        use cs_core::{Symmetry, SymmetryAxes};
        let mut e = editor(); // 8×8
        e.set_symmetry(Symmetry {
            axes: SymmetryAxes {
                vertical: true,
                ..SymmetryAxes::NONE
            },
```

En `crates/cs-ui/src/demo.rs`, reemplazar `mode: cs_core::SymmetryMode::Horizontal,` por:

```rust
        axes: cs_core::SymmetryAxes {
            vertical: true,
            ..cs_core::SymmetryAxes::NONE
        },
```

En `crates/cs-ui/src/tool_tests.rs`:
- `use cs_core::{Mask, Symmetry, SymmetryMode};` pasa a `use cs_core::{Mask, Symmetry, SymmetryAxes};`.
- `use cs_core::{Mask, Rgba, Symmetry, SymmetryMode};` pasa a
  `use cs_core::{Mask, Rgba, Symmetry, SymmetryAxes};`.
- Las dos apariciones de `mode: SymmetryMode::Horizontal,` pasan a:

```rust
            axes: SymmetryAxes {
                vertical: true,
                ..SymmetryAxes::NONE
            },
```

En `crates/cs-ui/src/canvas.rs`:
- `use cs_core::{Artboard, ArtboardId, Axis, GridSettings, Guide, Mask, SymmetryMode};` pasa a
  `use cs_core::{Artboard, ArtboardId, Axis, GridSettings, Guide, Mask};`.
- Se borran `fn has_vertical_axis` y `fn has_horizontal_axis`, que van justo después de
  `enum SymHandle`.
- En `symmetry_input`:
  - `if symmetry.mode == SymmetryMode::None {` pasa a `if !symmetry.axes.any() {`;
  - `} else if has_vertical_axis(symmetry.mode)` pasa a `} else if symmetry.axes.vertical`;
  - `} else if has_horizontal_axis(symmetry.mode)` pasa a `} else if symmetry.axes.horizontal`.
- En `draw_symmetry`, reemplazar:

```rust
        let mode = editor.symmetry().mode;
        if mode == SymmetryMode::None {
            return;
        }
```

por:

```rust
        let axes = editor.symmetry().axes;
        if !axes.any() {
            return;
        }
```

  y en la misma función:
  - `if has_vertical_axis(mode) {` pasa a `if axes.vertical {`;
  - `if has_horizontal_axis(mode) {` pasa a `if axes.horizontal {`;
  - `if matches!(mode, SymmetryMode::DiagonalDown | SymmetryMode::All) {` pasa a
    `if axes.diagonal_down {`;
  - `if matches!(mode, SymmetryMode::DiagonalUp | SymmetryMode::All) {` pasa a
    `if axes.diagonal_up {`.

Así, los ejes punteados se dibujan uno por cada eje prendido.

- [ ] **Step 8: Los cuatro botones en la barra**

En `crates/cs-ui/src/panels.rs`, cambiar
`use cs_core::{ArtboardId, Palette, PixelValue, SymmetryMode, Tile};` por
`use cs_core::{ArtboardId, Palette, PixelValue, SymmetryAxes, Tile};`.

Reemplazar desde `const SYMMETRY_MODES` hasta el `}` final de `fn draw_symmetry_icon` (justo antes
de `/// Damero mínimo para que se note la transparencia.`) por:

```rust
/// Un eje de simetría de la barra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymAxis {
    Vertical,
    Horizontal,
    DiagonalDown,
    DiagonalUp,
}

impl SymAxis {
    /// Si el eje está prendido en `axes`.
    fn on(self, axes: SymmetryAxes) -> bool {
        match self {
            SymAxis::Vertical => axes.vertical,
            SymAxis::Horizontal => axes.horizontal,
            SymAxis::DiagonalDown => axes.diagonal_down,
            SymAxis::DiagonalUp => axes.diagonal_up,
        }
    }

    /// `axes` con este eje cambiado y los demás igual.
    fn toggled(self, mut axes: SymmetryAxes) -> SymmetryAxes {
        let flag = match self {
            SymAxis::Vertical => &mut axes.vertical,
            SymAxis::Horizontal => &mut axes.horizontal,
            SymAxis::DiagonalDown => &mut axes.diagonal_down,
            SymAxis::DiagonalUp => &mut axes.diagonal_up,
        };
        *flag = !*flag;
        axes
    }
}

/// Los ejes en el orden del mockup (│ ─ ╲ ╱), con su nombre, que es también el tooltip. Las
/// diagonales se nombran con `\` y `/`: ╲ y ╱ no están en las fuentes proporcionales.
const SYMMETRY_AXES: [(SymAxis, &str); 4] = [
    (SymAxis::Vertical, "Vertical axis (left ↔ right)"),
    (SymAxis::Horizontal, "Horizontal axis (top ↔ bottom)"),
    (SymAxis::DiagonalDown, "Diagonal axis \\"),
    (SymAxis::DiagonalUp, "Diagonal axis /"),
];

/// Ayuda de los ejes, en el tooltip de los botones de simetría cuando hay alguno prendido.
const AXES_HINT: &str = "Ctrl+drag the dashed axes to move them";

/// Symmetry, al final de la barra de opciones. Se arma de derecha a izquierda (la barra lo pone
/// pegado al borde): "Center axes" si los ejes se movieron, los cuatro ejes en orden inverso y el
/// título. En pantalla se leen │ ─ ╲ ╱. Cada botón prende o apaga solo su eje.
fn symmetry_buttons(ui: &mut egui::Ui, editor: &mut Editor) {
    let current = editor.symmetry();
    if current.axes.any() && current.center2.is_some() && ui.small_button("Center axes").clicked() {
        editor.set_symmetry(cs_core::Symmetry {
            center2: None,
            ..current
        });
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (axis, tip) in SYMMETRY_AXES.into_iter().rev() {
            let response = symmetry_toggle(ui, axis, tip, axis.on(current.axes));
            let response = if current.axes.any() {
                response.on_hover_text(format!("{tip}\n{AXES_HINT}"))
            } else {
                response.on_hover_text(tip)
            };
            if response.clicked() {
                editor.set_symmetry(cs_core::Symmetry {
                    axes: axis.toggled(current.axes),
                    ..current
                });
            }
        }
    });
    ui.label("Symmetry");
}

/// Un botón de prender y apagar un eje, dibujado con figuras.
fn symmetry_toggle(ui: &mut egui::Ui, axis: SymAxis, tip: &str, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, tip)
    });
    let visuals = ui.style().interact_selectable(&response, on);
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, visuals.weak_bg_fill);
    let color = if on {
        theme::ACCENT
    } else {
        visuals.fg_stroke.color
    };
    let r = rect.shrink(5.0);
    painter.rect_stroke(
        r,
        1.0,
        Stroke::new(1.0, color.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );
    painter.extend(axis_shapes(r, axis, color));
    response
}

/// El eje como una línea de lado a lado de `r`.
fn axis_shapes(r: Rect, axis: SymAxis, color: Color32) -> Vec<egui::Shape> {
    let stroke = Stroke::new(1.5, color);
    let c = r.center();
    let segment = match axis {
        SymAxis::Vertical => [egui::pos2(c.x, r.top()), egui::pos2(c.x, r.bottom())],
        SymAxis::Horizontal => [egui::pos2(r.left(), c.y), egui::pos2(r.right(), c.y)],
        SymAxis::DiagonalDown => [r.left_top(), r.right_bottom()],
        SymAxis::DiagonalUp => [r.left_bottom(), r.right_top()],
    };
    vec![egui::Shape::line_segment(segment, stroke)]
}
```

- [ ] **Step 9: Correr todo**

Run:
- `cargo fmt --all`
- `cargo test -p cs-core -p cs-raster -p cs-io -p cs-ui`
- `cargo check --workspace --all-targets`

Expected: todo en verde, incluidos `each_symmetry_button_turns_its_own_axis_on_and_off` y
`a_click_with_both_straight_axes_paints_four_pixels`. `cs-agent`, `cs-commands` y el MCP
compilan sin cambios: solo usan `Symmetry` con `Default::default()`.

Y además: `grep -rn SymmetryMode crates apps`
Expected: sin resultados.

- [ ] **Step 10: Commit**

```bash
git add crates/cs-core/src/document.rs crates/cs-raster/src/symmetry.rs crates/cs-raster/src/lib.rs crates/cs-io/src/chimp.rs crates/cs-ui/src/editor.rs crates/cs-ui/src/demo.rs crates/cs-ui/src/tool_tests.rs crates/cs-ui/src/canvas.rs crates/cs-ui/src/panels.rs
git commit -m "Simetría sumable: cuatro ejes que se prenden por separado y su grupo

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Compatibilidad: `.chimp`, autosave y MCP con el `mode` de antes

**Files:**
- Test: `crates/cs-io/src/chimp.rs`
- Test: `crates/cs-ui/src/autosave.rs`
- Test: `crates/cs-agent/src/ops/document.rs`

**Interfaces:**
- Consumes: `Symmetry` y `SymmetryAxes` con su `serde(from)` (Task 3).
- Produces: solo tests de regresión, que fijan que lo viejo se sigue leyendo por los tres caminos
  reales.

Estos tests pasan desde la Task 3: el código de compatibilidad está en `SymmetryRepr`. Si alguno
falla, el arreglo va en `crates/cs-core/src/document.rs`, no en el test.

- [ ] **Step 1: `.chimp` con el manifiesto viejo**

En `crates/cs-io/src/chimp.rs`, al final de `mod tests`:

```rust
    #[test]
    fn an_old_chimp_with_the_symmetry_mode_opens_with_its_axes() {
        // Manifiesto como lo escribía la versión anterior: `mode` en vez de `axes`.
        let sprite = Sprite::new(4, 4, ColorMode::Rgba);
        let file = aseprite::write(&sprite);
        let modes = [
            ("None", [false, false, false, false]),
            ("Horizontal", [true, false, false, false]),
            ("Vertical", [false, true, false, false]),
            ("Both", [true, true, false, false]),
            ("DiagonalDown", [false, false, true, false]),
            ("DiagonalUp", [false, false, false, true]),
            ("All", [true, true, true, true]),
        ];
        for (mode, [v, h, down, up]) in modes {
            let manifest = format!(
                r#"{{"format":"chimp","version":1,"artboards":[{{"id":1,"name":"a","position":[0,0],"file":"artboards/1.aseprite","symmetry":{{"mode":"{mode}","center2":[3,5]}}}}]}}"#
            );
            let zip = zip_with(&[
                (MANIFEST, manifest.as_bytes()),
                ("artboards/1.aseprite", &file),
            ]);
            let (doc, _) = read(&zip).unwrap();
            let s = doc.artboards[0].symmetry;
            let axes = s.axes;
            assert_eq!(
                [
                    axes.vertical,
                    axes.horizontal,
                    axes.diagonal_down,
                    axes.diagonal_up
                ],
                [v, h, down, up],
                "{mode}"
            );
            assert_eq!(s.center2, Some((3, 5)), "{mode}");
            // Al guardar se escribe el formato nuevo.
            let again = write(&doc).unwrap();
            let mut archive = ZipArchive::new(Cursor::new(again.as_slice())).unwrap();
            let text = String::from_utf8(read_entry(&mut archive, MANIFEST).unwrap()).unwrap();
            assert!(text.contains("\"axes\""), "{text}");
            assert!(!text.contains("\"mode\""), "{text}");
        }
    }
```

- [ ] **Step 2: Un autosave real de la versión anterior**

En `crates/cs-ui/src/autosave.rs`, `mod tests`, antes de `fn too_large_documents_are_rejected`.
La constante es un autosave escrito con el código de `d45ccca` (`encode` de "hero.chimp"): un
sprite de 4×4 con `{"mode": "Both", "center2": [3, 5]}`. Se copia tal cual, en una sola línea:

```rust
    /// Autosave real de la versión anterior: "hero.chimp", un sprite de 4×4 con la simetría
    /// `{"mode": "Both", "center2": [3, 5]}` en su manifiesto.
    const OLD_AUTOSAVE: &str = r#"[{"name":"hero.chimp","chimp":"UEsDBBQAAAAIAAAAIQAnyrMt8wAAAJ8BAAAUAAAAYXJ0Ym9hcmRzLzEuYXNlcHJpdGW1jzFIQmEYRc+PD1ydgqZ+IqKt50MKSRCClmwQByMaVCzBNxhFBCI0RBC4tARRDrW05Sw0uDRGc5RDBbUGNVZw4xcice+MF77v3nNm4OnC4OFhMcAag1iMC4kR4z8YM/D5Hum3RoARIGr7lfzxAIxa+3sz/EQwbhOChamUoJKMCw7zc4LHzUnB/XlF8HZ9Kvj6mBGcvGYFYdcXzB7VBJnqtCCXmRAEqaQgnd4W+MU9wWrtWVBuSVC4fRHc9L4Fkkta7Y5gf/dAUA1DwUo+J9iZbwoug8BtWC4L7hpXbtvxoqC54cu5eNaZD8gQZalUX9+y8R9QSwMEFAAAAAgAAAAhACE8PA+jAAAAMgEAAA0AAABtYW5pZmVzdC5qc29uVY7BCoMwEETvfsWw59JqSy8e+xvFQ6orBhojm1AQ8d9LVk1pDpvZzczLLgVAvRdnItWgdrBuolMafliC9SPVqLQ3El/eSBeoxrMAgEUrQLY7XNqOxnGCDSxeWTqdfLBxA27xdMrjGSh31eREb9/KyT9fqrMJPImN/OOG2TmOMlOdFwLI+U6zDx+H7AWo5TGyXP+WAG4/B3DPutnVqneqTbF+AVBLAQIUABQAAAAIAAAAIQAnyrMt8wAAAJ8BAAAUAAAAAAAAAAAAAACkgQAAAABhcnRib2FyZHMvMS5hc2Vwcml0ZVBLAQIUABQAAAAIAAAAIQAhPDwPowAAADIBAAANAAAAAAAAAAAAAACkgSUBAABtYW5pZmVzdC5qc29uUEsFBgAAAAACAAIAfQAAAPMBAAAAAA=="}]"#;

    #[test]
    fn an_old_autosave_restores_the_symmetry_as_axes() {
        let back = decode(OLD_AUTOSAVE).unwrap();
        assert_eq!(back[0].name, "hero.chimp");
        let symmetry = back[0].doc.artboards[0].symmetry;
        assert_eq!(
            symmetry.axes,
            cs_core::SymmetryAxes {
                vertical: true,
                horizontal: true,
                ..cs_core::SymmetryAxes::NONE
            }
        );
        assert_eq!(symmetry.center2, Some((3, 5)));
    }
```

- [ ] **Step 3: MCP: `apply_commands` con el `SetSymmetry` viejo**

En `crates/cs-agent/src/ops/document.rs`, al final de `mod tests`:

```rust
    #[test]
    fn apply_commands_still_takes_the_old_symmetry_mode() {
        let mut s = session(4, 4);
        // Así se pedía la simetría antes, con un solo modo.
        let old = json!({"SetSymmetry": {"id": 1, "symmetry": {"mode": "Both", "center2": null}}});
        s.apply_commands(p(json!({"doc": "d1", "commands": [old]})))
            .unwrap();
        let axes = s.doc("d1").unwrap().document.artboards[0].symmetry.axes;
        assert!(axes.vertical && axes.horizontal, "{axes:?}");
        assert!(!axes.diagonal_down && !axes.diagonal_up, "{axes:?}");
        // Y con el formato nuevo.
        let new = json!({"SetSymmetry": {"id": 1, "symmetry": {"axes": {"diagonal_up": true}}}});
        s.apply_commands(p(json!({"doc": "d1", "commands": [new]})))
            .unwrap();
        let axes = s.doc("d1").unwrap().document.artboards[0].symmetry.axes;
        assert!(axes.diagonal_up && !axes.vertical, "{axes:?}");
    }
```

- [ ] **Step 4: Correrlos**

Run:
- `cargo fmt --all`
- `cargo test -p cs-io an_old_chimp`
- `cargo test -p cs-ui an_old_autosave`
- `cargo test -p cs-agent apply_commands`

Expected: PASS los tres (más `apply_commands_is_atomic_and_undoable`).

Para ver que prueban algo, quitar un momento `.or(repr.mode.map(SymmetryAxes::from))` en
`SymmetryRepr` y volver a correr: deben fallar los tres. Después, deshacer el cambio.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-io/src/chimp.rs crates/cs-ui/src/autosave.rs crates/cs-agent/src/ops/document.rs
git commit -m "Tests: los .chimp, autosaves y llamadas MCP con el modo de simetría viejo se siguen leyendo

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Barra de opciones de 19 con Pixel UI y botones de simetría del skin

**Files:**
- Modify: `crates/cs-ui/src/pixel_ui.rs`
- Modify: `crates/cs-ui/src/panels.rs`
- Modify: `crates/cs-ui/src/lib.rs`
- Test: `crates/cs-ui/src/layout_tests.rs`

**Interfaces:**
- Consumes:
  - `sym.slot`, `sym.slot.active` e `icon.sym_*` (Task 1);
  - `apply_style(ctx, Option<&SkinColors>)` (Task 2);
  - `SymAxis`, `SYMMETRY_AXES`, `symmetry_toggle`, `axis_shapes` y `BarHarness::click` (Task 3).
- Produces:
  - `pub fn pixel_ui::toggle_button(ui: &mut Ui, skin: &Skin, icon: &str, label: &str, on: bool, fallback: impl FnOnce(Rect, Color32) -> Vec<Shape>) -> Response`.
  - `pub const panels::PIXEL_OPTIONS_BAR_HEIGHT: f32 = 19.0`.
  - `pub fn panels::options_bar_height(skin: Option<&Skin>) -> f32`.
  - `pub fn panels::options_bar_frame(style: &egui::Style, skin: Option<&Skin>) -> egui::Frame`.
  - `pub fn panels::tool_options_bar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>)`.
  - `SymAxis::icon(self) -> &'static str`.

- [ ] **Step 1: Tests de `toggle_button`**

En `crates/cs-ui/src/pixel_ui.rs`, `mod tests`, antes de
`fn the_toolbar_is_as_wide_as_the_slot_plus_the_margin`:

```rust
    /// Un frame con un botón de prender y apagar: su rect, si se hizo clic y las texturas
    /// dibujadas.
    fn toggle(
        ctx: &egui::Context,
        skin: &Skin,
        icon: &str,
        on: bool,
        events: Vec<Event>,
    ) -> (Rect, bool, Vec<egui::TextureId>) {
        let mut out = (Rect::NOTHING, false);
        let input = RawInput { events, ..screen() };
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let r = toggle_button(ui, skin, icon, "Vertical axis", on, |r, color| {
                    vec![Shape::rect_filled(r, 0.0, color)]
                });
                out = (r.rect, r.clicked());
            });
        });
        output.textures_delta.clear();
        let mut textures = Vec::new();
        for clipped in &output.shapes {
            collect_textures(&clipped.shape, &mut textures);
        }
        (out.0, out.1, textures)
    }

    #[test]
    fn a_toggle_is_the_symmetry_slot_with_its_icon_and_the_active_slot_when_on() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let id = |name: &str| skin.piece(name, WidgetState::Normal).unwrap().texture.id();
        let (rect, _, textures) = toggle(&ctx, &skin, "icon.sym_vertical", false, vec![]);
        assert_eq!(rect.size(), vec2(13.0, 14.0), "the piece size");
        assert!(textures.contains(&id("sym.slot")));
        assert!(textures.contains(&id("icon.sym_vertical")));
        let (_, _, textures) = toggle(&ctx, &skin, "icon.sym_vertical", true, vec![]);
        assert!(textures.contains(&id("sym.slot.active")));
        assert!(!textures.contains(&id("sym.slot")));
    }

    #[test]
    fn a_toggle_clicks_with_the_skin_and_with_the_flat_fallback() {
        let ctx = egui::Context::default();
        let skins = [
            Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
            Skin::fallback(),
        ];
        for skin in &skins {
            // Sin el ícono (`icon.nope`), dibuja el respaldo.
            let (rect, _, _) = toggle(&ctx, skin, "icon.nope", false, vec![]);
            assert_eq!(rect.size(), vec2(13.0, 14.0));
            let at = rect.center();
            toggle(
                &ctx,
                skin,
                "icon.nope",
                false,
                vec![Event::PointerMoved(at)],
            );
            toggle(&ctx, skin, "icon.nope", false, vec![press(at, true)]);
            let (_, clicked, _) = toggle(&ctx, skin, "icon.nope", false, vec![press(at, false)]);
            assert!(clicked);
        }
    }
```

- [ ] **Step 2: Tests de la barra**

En `crates/cs-ui/src/panels.rs`, `mod tests`, reemplazar desde `struct BarHarness {` hasta el
`}` de `fn new` (inclusive; `impl BarHarness {` queda abierto) por:

```rust
    struct BarHarness {
        ctx: egui::Context,
        editor: Editor,
        /// Con Pixel UI: el skin embebido (y el estilo de `apply_style` con sus colores).
        skin: Option<Skin>,
        time: f64,
        width: f32,
        bar: Rect,
        nodes: Vec<(String, Rect)>,
    }

    impl BarHarness {
        fn new(width: f32) -> Self {
            Self::with_skin(width, false)
        }

        /// Con `pixel`, la barra de Pixel UI: el estilo con los colores del skin y la fuente
        /// pixel, y el skin embebido.
        fn with_skin(width: f32, pixel: bool) -> Self {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let skin = pixel.then(|| Skin::embedded(&ctx));
            if let Some(skin) = &skin {
                ctx.set_fonts(crate::appearance::font_definitions(true));
                crate::appearance::apply_style(&ctx, Some(skin.colors()));
            }
            let mut h = Self {
                ctx,
                editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
                skin,
                time: 0.0,
                width,
                bar: Rect::NOTHING,
                nodes: Vec::new(),
            };
            h.frame(vec![]);
            h
        }
```

(El resto del `impl` sigue igual.)

En `BarHarness::frame`, reemplazar:

```rust
            let editor = &mut self.editor;
            let mut bar = Rect::NOTHING;
            let mut output = self.ctx.run_ui(input, |ui| {
                bar = egui::Panel::top("tool_options")
                    .exact_size(OPTIONS_BAR_HEIGHT)
                    .show(ui, |ui| tool_options_bar(ui, editor))
                    .response
                    .rect;
            });
```

por:

```rust
            let editor = &mut self.editor;
            let skin = self.skin.as_ref();
            let mut bar = Rect::NOTHING;
            let mut output = self.ctx.run_ui(input, |ui| {
                bar = egui::Panel::top("tool_options")
                    .exact_size(options_bar_height(skin))
                    .frame(options_bar_frame(ui.style(), skin))
                    .show(ui, |ui| tool_options_bar(ui, editor, skin))
                    .response
                    .rect;
            });
```

Reemplazar todo el test `with_pixel_ui_style_every_node_of_the_options_bar_fits_inside_it` por:

```rust
    #[test]
    fn with_pixel_ui_the_bar_is_19_and_every_node_fits_inside_it() {
        let mut h = BarHarness::with_skin(800.0, true);
        for kind in [ToolKind::Pencil, ToolKind::MagicWand, ToolKind::Text] {
            h.editor.set_tool(kind);
            h.frame(vec![]);
            h.frame(vec![]);
            assert_eq!(h.bar.height(), 19.0, "{kind:?}");
            assert!(h.nodes.len() > 2, "{kind:?}: {:?}", h.nodes);
            for (text, rect) in &h.nodes {
                assert!(
                    rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom(),
                    "{kind:?}: {text:?} {rect:?} out of {:?}",
                    h.bar
                );
            }
        }
    }
```

Antes de `fn the_options_bar_is_one_row_of_the_same_height_with_every_tool`:

```rust
    #[test]
    fn with_pixel_ui_the_symmetry_buttons_are_the_skin_pieces_and_toggle_their_axis() {
        let mut h = BarHarness::with_skin(1200.0, true);
        let [vertical, horizontal, ..] = SYMMETRY_AXES.map(|(_, name)| name);
        // Los botones miden lo que la pieza `sym.slot` y la forma del pincel, 13×13.
        assert_eq!(h.rect(vertical).size(), Vec2::new(13.0, 14.0));
        assert_eq!(h.rect("Round").size(), Vec2::splat(13.0));
        h.click(vertical);
        h.click(horizontal);
        let axes = h.editor.symmetry().axes;
        assert!(axes.vertical && axes.horizontal, "{axes:?}");
    }
```

En el test `the_shape_buttons_pick_round_or_square`, reemplazar
`rects = brush_shape_buttons(ui, shape).map(|r| r.rect);` por
`rects = brush_shape_buttons(ui, shape, 20.0).map(|r| r.rect);`.

En `crates/cs-ui/src/layout_tests.rs`, antes de
`fn without_pixel_ui_the_named_toolbar_keeps_its_width`:

```rust
#[test]
fn with_pixel_ui_the_options_bar_is_19_high() {
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let h = LayoutHarness::new(view, true);
    assert_eq!(h.panel("tool_options").height(), 19.0);
    // Sin Pixel UI, la de siempre.
    assert_eq!(
        LayoutHarness::editor().panel("tool_options").height(),
        32.0
    );
}
```

- [ ] **Step 3: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui`
Expected: FAIL de compilación: faltan `toggle_button`, `options_bar_height`, `options_bar_frame`
y el tercer parámetro de `tool_options_bar` y `brush_shape_buttons`.

- [ ] **Step 4: `toggle_button` en `pixel_ui.rs`**

En `crates/cs-ui/src/pixel_ui.rs`, después de `pub const TOOLBAR_MARGIN: i8 = 4;`:

```rust
/// Marco de los botones de prender y apagar de la barra de opciones (los ejes de simetría);
/// `.active` cuando están prendidos.
const TOGGLE_PIECE: &str = "sym.slot";
/// Tamaño del botón de prender y apagar de respaldo (el de la pieza del mockup).
const FALLBACK_TOGGLE_SIZE: Vec2 = Vec2::new(13.0, 14.0);
/// Esquina del ícono dentro de `sym.slot`: el interior del cuadro de 11×11 empieza en (2,2).
const TOGGLE_ICON_OFFSET: Vec2 = Vec2::new(2.0, 2.0);
/// Lado del interior de `sym.slot`, donde va el ícono.
const TOGGLE_ICON_SIZE: Vec2 = Vec2::new(9.0, 9.0);
```

Antes de `/// Triangulito de 3×3 px que marca un grupo con variantes,`:

```rust
/// Botón de prender y apagar de la barra de opciones: el marco `sym.slot` (`sym.slot.active`
/// con `on`) a tamaño de pieza y el ícono `icon` en su interior.
///
/// - Sin el marco: el respaldo plano, con un borde `accent` si está prendido.
/// - Sin el ícono: lo que dibuje `fallback` en el interior (rect y color del texto).
/// - `label` es el nombre accesible.
pub fn toggle_button(
    ui: &mut Ui,
    skin: &Skin,
    icon: &str,
    label: &str,
    on: bool,
    fallback: impl FnOnce(Rect, Color32) -> Vec<Shape>,
) -> Response {
    let size = skin
        .piece(TOGGLE_PIECE, WidgetState::Normal)
        .map_or(FALLBACK_TOGGLE_SIZE, |p| {
            vec2(p.size[0] as f32, p.size[1] as f32)
        });
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let colors = *skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let state = if on {
        WidgetState::Active
    } else {
        WidgetState::Normal
    };
    let mut shapes = match skin.piece(TOGGLE_PIECE, state) {
        Some(piece) => piece_shapes(piece, rect, ppp),
        None => {
            let mut flat = flat_shapes(skin, rect, 0.0, ppp);
            if on {
                flat.push(Shape::rect_stroke(
                    rect.shrink(1.0).round_to_pixels(ppp),
                    0.0,
                    Stroke::new(1.0, colors.accent),
                    StrokeKind::Inside,
                ));
            }
            flat
        }
    };
    let inner =
        Rect::from_min_size(rect.min + TOGGLE_ICON_OFFSET, TOGGLE_ICON_SIZE).round_to_pixels(ppp);
    match skin.piece(icon, WidgetState::Normal) {
        Some(piece) => {
            let icon_size = vec2(piece.size[0] as f32, piece.size[1] as f32);
            let at = Rect::from_min_size(inner.min, icon_size);
            shapes.extend(piece_shapes(piece, at, ppp));
        }
        None => shapes.extend(fallback(inner, colors.text)),
    }
    ui.painter().extend(shapes);
    response
}
```

- [ ] **Step 5: La barra de 19 en `panels.rs`**

Reemplazar desde `pub const OPTIONS_BAR_HEIGHT: f32 = 32.0;` hasta el `}` de
`pub fn tool_options_bar` por:

```rust
pub const OPTIONS_BAR_HEIGHT: f32 = 32.0;
/// Alto de la barra de opciones con Pixel UI: la del mockup, de 19 px de skin.
pub const PIXEL_OPTIONS_BAR_HEIGHT: f32 = 19.0;
/// Alto de los controles de la barra con Pixel UI (sliders, campos, casillas y botones).
const PIXEL_CONTROL_HEIGHT: f32 = 13.0;

/// Alto de la barra de opciones: `PIXEL_OPTIONS_BAR_HEIGHT` con Pixel UI (`skin`) y
/// `OPTIONS_BAR_HEIGHT` sin él.
pub fn options_bar_height(skin: Option<&Skin>) -> f32 {
    if skin.is_some() {
        PIXEL_OPTIONS_BAR_HEIGHT
    } else {
        OPTIONS_BAR_HEIGHT
    }
}

/// El marco del panel de la barra: con Pixel UI, sin margen vertical (los controles de 13 y el
/// texto de la fuente pixel ocupan el alto justo); sin él, el de siempre.
pub fn options_bar_frame(style: &egui::Style, skin: Option<&Skin>) -> egui::Frame {
    let frame = egui::Frame::side_top_panel(style);
    match skin {
        Some(_) => frame.inner_margin(egui::Margin::symmetric(8, 0)),
        None => frame,
    }
}

/// Barra de opciones, debajo de las pestañas: las opciones de la herramienta activa en una sola
/// fila y Symmetry a la derecha. Si las opciones no caben, hacen scroll horizontal con la rueda o
/// la barra (sin arrastre); no se parten en dos renglones. Con `skin` (Pixel UI), los controles
/// son de 13 de alto y los botones de simetría usan las piezas del skin.
pub fn tool_options_bar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>) {
    // Los textos no se parten: la fila se alarga y hace scroll.
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    // La rueda de siempre (vertical) mueve la fila de lado, sin Shift.
    ui.style_mut().always_scroll_the_only_direction = true;
    if skin.is_some() {
        let spacing = ui.spacing_mut();
        spacing.interact_size.y = PIXEL_CONTROL_HEIGHT;
        spacing.button_padding.y = 0.0;
        spacing.item_spacing.y = 0.0;
        // La barra de scroll de la fila flota sobre los controles, sin sumar alto.
        spacing.scroll.floating = true;
    }
    // De derecha a izquierda: Symmetry queda pegado al borde y las opciones usan lo que sobra.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        symmetry_buttons(ui, editor, skin);
        ui.separator();
        options_scroll_area().show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                tool_options(ui, editor, skin.is_some())
            });
        });
    });
    text_window(ui.ctx(), editor);
}
```

En `fn tool_options`, cambiar la firma y su comentario por:

```rust
/// Las opciones de la herramienta activa, en fila: cada etiqueta al lado de su control y un
/// separador entre grupos. La ayuda de la herramienta va en el mismo orden que antes. `compact`
/// (Pixel UI): los botones de forma son de 13×13.
fn tool_options(ui: &mut egui::Ui, editor: &mut Editor, compact: bool) {
```

y reemplazar `brush_shape_buttons(ui, &mut options.brush_shape);` por:

```rust
        let side = if compact { PIXEL_CONTROL_HEIGHT } else { 20.0 };
        brush_shape_buttons(ui, &mut options.brush_shape, side);
```

En `fn brush_shape_buttons`, reemplazar el comentario, la firma y la primera línea del `map`:

```rust
/// Botones Round y Square de la forma del pincel, cuadrados de `side`. El círculo y el cuadrado
/// se pintan en vez de usar glifos: `●` solo está en Hack, que no entra en la familia
/// proporcional de egui.
fn brush_shape_buttons(
    ui: &mut egui::Ui,
    shape: &mut BrushShape,
    side: f32,
) -> [egui::Response; 2] {
    ui.horizontal(|ui| {
        [(BrushShape::Round, "Round"), (BrushShape::Square, "Square")].map(|(value, name)| {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
```

y en la misma función reemplazar `let icon = rect.shrink(5.0);` por:

```rust
            // El dibujo deja un cuarto del lado de margen: 5 de 20, 3 de 13.
            let icon = rect.shrink((side / 4.0).floor());
```

- [ ] **Step 6: Los botones de simetría con el skin**

En `impl SymAxis`, antes de `fn on`:

```rust
    /// Ícono del skin del eje.
    fn icon(self) -> &'static str {
        match self {
            SymAxis::Vertical => "icon.sym_vertical",
            SymAxis::Horizontal => "icon.sym_horizontal",
            SymAxis::DiagonalDown => "icon.sym_diag_down",
            SymAxis::DiagonalUp => "icon.sym_diag_up",
        }
    }
```

Reemplazar el comentario y la firma de `symmetry_buttons` por:

```rust
/// Symmetry, al final de la barra de opciones. Se arma de derecha a izquierda (la barra lo pone
/// pegado al borde): "Center axes" si los ejes se movieron, los cuatro ejes en orden inverso y el
/// título. En pantalla se leen │ ─ ╲ ╱. Cada botón prende o apaga solo su eje. Con `skin`
/// (Pixel UI), los botones son las piezas `sym.slot` con su ícono.
fn symmetry_buttons(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>) {
```

y dentro del `for`, reemplazar:

```rust
            let response = symmetry_toggle(ui, axis, tip, axis.on(current.axes));
```

por:

```rust
            let on = axis.on(current.axes);
            let response = match skin {
                Some(skin) => {
                    pixel_ui::toggle_button(ui, skin, axis.icon(), tip, on, |r, color| {
                        axis_shapes(r, axis, color)
                    })
                }
                None => symmetry_toggle(ui, axis, tip, on),
            };
```

- [ ] **Step 7: La app usa la barra de 19**

En `crates/cs-ui/src/lib.rs`, `editor_panels`, reemplazar:

```rust
        // La barra de opciones, a todo el ancho debajo de las pestañas.
        egui::Panel::top("tool_options")
            .exact_size(panels::OPTIONS_BAR_HEIGHT)
            .show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    panels::tool_options_bar(ui, &mut self.doc.editor)
                });
            });
        // La toolbar antes que la línea de tiempo: va a todo el alto, hasta la barra de estado, y
        // la línea de tiempo empieza a su derecha.
        let skin = self.pixel.skin();
```

por:

```rust
        let skin = self.pixel.skin();
        // La barra de opciones, a todo el ancho debajo de las pestañas: 19 de alto con Pixel UI.
        egui::Panel::top("tool_options")
            .exact_size(panels::options_bar_height(skin))
            .frame(panels::options_bar_frame(ui.style(), skin))
            .show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    panels::tool_options_bar(ui, &mut self.doc.editor, skin)
                });
            });
        // La toolbar antes que la línea de tiempo: va a todo el alto, hasta la barra de estado, y
        // la línea de tiempo empieza a su derecha.
```

La línea de la Task 2 (`self.doc.canvas.background = canvas::background_for(skin);`, con su
comentario) queda justo después, igual.

- [ ] **Step 8: Correr los tests**

Run:
- `cargo fmt --all`
- `cargo test -p cs-ui`
- `cargo clippy -p cs-ui --all-targets -- -D warnings`

Expected: todo en verde, incluidos:
- `a_toggle_is_the_symmetry_slot_with_its_icon_…` y `a_toggle_clicks_…`;
- `with_pixel_ui_the_bar_is_19_and_every_node_fits_inside_it`;
- `with_pixel_ui_the_symmetry_buttons_are_the_skin_pieces_…`;
- `with_pixel_ui_the_options_bar_is_19_high`.

Al planear, con Pixel UI, los nodos de la barra medían:
- textos, sliders, campos y combos: 15;
- botones de simetría: 14;
- botones de forma: 13.

- [ ] **Step 9: Commit**

```bash
git add crates/cs-ui/src/pixel_ui.rs crates/cs-ui/src/panels.rs crates/cs-ui/src/lib.rs crates/cs-ui/src/layout_tests.rs
git commit -m "Pixel UI: barra de opciones de 19 px con los botones de simetría del skin

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Verificación final y prueba manual

**Files:** ninguno. Es solo verificación: si algo falla, se arregla en el archivo de la tarea
correspondiente y se commitea aparte.

- [ ] **Step 1: Verificación automática**

Run (con el toolchain exportado):
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo check -p cs-ui --target wasm32-unknown-unknown`
- `grep -rn "SymmetryMode" crates apps` (sin resultados)

Expected: todo en verde, sin warnings.

- [ ] **Step 2: Compilar la app (si el exe no está en uso)**

```bash
CARGO_TARGET_DIR=target/gui-pixel cargo build --bin chimpsprite
```

Si falla con "Access is denied" o "being used by another process" sobre
`target/gui-pixel/debug/chimpsprite.exe`, el usuario tiene la app abierta. En ese caso:
- Se salta la prueba manual y se anota en el reporte.
- No se cierra ni se mata ese proceso.

- [ ] **Step 3: Prueba manual**

```bash
cp "$APPDATA/ChimpSprite/data/app.ron" /tmp/chimpsprite-app.ron.bak
CARGO_TARGET_DIR=target/gui-pixel cargo run --bin chimpsprite
```

(en background). Comprobar:
- **Pixel UI apagado:**
  - Tema oscuro de siempre y barra de 32.
  - A la derecha, "Symmetry" con 4 botones │ ─ ╲ ╱ dibujados, sin "Off".
  - Prender │ y ─: el lienzo muestra los dos ejes punteados. Un clic con el lápiz pinta 4
    puntos.
  - Prender además ╲: 8 puntos.
  - Ctrl+arrastrar la línea vertical la mueve. "Center axes" la devuelve al centro.
  - Tooltip de ╲: "Diagonal axis \", con la ayuda de Ctrl+arrastrar en la segunda línea.
- **View → Pixel UI (2×):**
  - Menú, pestañas, barra de opciones y paneles en crema (221,206,191).
  - Alrededor del lienzo, café claro (189,164,153). Texto oscuro.
  - Barra de opciones de 19 px. Los botones de simetría son los cuadritos del mockup; el
    prendido lleva el halo claro.
  - Comparar con `.design/Design.aseprite`, frame 3, sin guardarlo.
  - Scroll del panel de paleta: manija lila y morado al pasar.
  - Con la herramienta Text, la barra sigue en una fila y nada se sale.
- **Abrir un `.chimp` viejo** (de antes de este cambio) que tenga simetría: los ejes vienen
  prendidos como estaban.
- **Volver a apagar Pixel UI:** todo vuelve al tema oscuro, con la barra de 32.

Al terminar, cerrar la app que se abrió en este paso y restaurar:

```bash
cp /tmp/chimpsprite-app.ron.bak "$APPDATA/ChimpSprite/data/app.ron"
```

- [ ] **Step 4: Sin push ni PR**

El usuario junta las ramas al final.

---

## Autorrevisión

**Cobertura del spec:**
- **§1 Colores:**
  - `color.background` y `color.canvas` en el skin (8×8 junto a los `color.*`), copiados del
    frame 3: Task 1.
  - `SkinColors` gana los dos campos, con respaldo del tema: Task 2.
  - La tabla de `Visuals` es `pixel_visuals`: Task 2.
    - `panel_fill`, `window_fill`, `extreme_bg_color`, `faint_bg_color`, texto, texto débil,
      selección, widgets, hipervínculo y sombras.
    - Los scrollbars van con la desviación anotada.
    - Sin redondeo y `dark_mode = false`.
  - Fondo del lienzo en `canvas`: Task 2 (`CanvasView::background`).
  - Al apagar vuelve el tema de hoy: test en la Task 2.
- **§2 Barra de 19:**
  - 19 con Pixel UI y 32 sin él: Task 5.
  - Controles de 13–15 (medidos), botones de 13×13 / 13×14 y menos espaciado: Task 5.
  - El test de "todo cabe" pasa a 19 con Pencil, Magic Wand y Text: Task 5.
- **§3 Simetría:**
  - `SymmetryAxes` y el grupo, con 1, 2, 4 u 8 y un test por combinación: Task 3.
  - Compatibilidad del `mode` viejo en `.chimp` y autosave, con escritura en el formato nuevo:
    Tasks 3 y 4.
  - Centro y Ctrl+arrastrar sin cambios; un eje punteado por cada eje prendido: Task 3.
  - Los 4 botones en el orden │ ─ ╲ ╱, cada uno con su eje: Tasks 3 y 5.
  - Íconos `icon.sym_*`, con figuras si faltan: Tasks 1 y 5.
  - Marcos `sym.slot` / `.active` copiados del mockup, porque los botones llevan marco propio:
    Task 1.
  - Tooltips con la ayuda de Ctrl+arrastrar. ╲ y ╱ van como `\` y `/` (ver Glifos): Task 3.
  - Sin Pixel UI, los mismos 4 botones con figuras: Task 3.
  - MCP / cs-agent: no usan el enum, y el `mode` viejo entra por `apply_commands`: Task 4.
- **§Tests:**
  - Skin (colores y píxeles exactos, con el script y el test): Task 1.
  - Visuals: Task 2.
  - Barra: Task 5.
  - Grupo: Task 3.
  - Compatibilidad: Tasks 3 y 4.
  - Interfaz (clic en │ y luego en ─; un trazo con │+─ pinta 4 puntos): Task 3, y con el skin en
    la Task 5.

**Placeholders:** no quedan "TBD", "TODO" ni "similar a…". Cada paso de código trae el código
completo, y los reemplazos dicen qué líneas cambian.

**Consistencia de tipos:**
- **`SymmetryAxes { vertical, horizontal, diagonal_down, diagonal_up }`, `NONE` y `any()`:**
  Tasks 3, 4 y 5.
- **`Symmetry { axes, center2 }`:** Tasks 3 y 4.
- **`symmetric_points(Point, SymmetryAxes, (i32, i32))` y `symmetrize(&[(i32, i32, T)], SymmetryAxes, (i32, i32))`:**
  Task 3 (los usa `editor::mirror`).
- **`apply_style(&Context, Option<&SkinColors>)` y `pixel_visuals(&SkinColors)`:** Tasks 2 y 5.
- **`CanvasView::background` y `background_for(Option<&Skin>)`:** Task 2, que `lib.rs` sigue
  usando en la Task 5.
- **`SymAxis::{on, toggled, icon}`, `SYMMETRY_AXES`, `symmetry_toggle(ui, SymAxis, &str, bool)` y
  `axis_shapes(Rect, SymAxis, Color32)`:** Tasks 3 y 5.
- **`toggle_button(ui, &Skin, icon, label, on, fallback)`:** Task 5.
- **`options_bar_height(Option<&Skin>)`, `options_bar_frame(&Style, Option<&Skin>)`,
  `tool_options_bar(ui, editor, Option<&Skin>)`, `tool_options(ui, editor, bool)` y
  `brush_shape_buttons(ui, shape, f32)`:** Task 5.
- **`BarHarness::{new, with_skin, click}`:** `click` en la Task 3; `with_skin` en la Task 5.
