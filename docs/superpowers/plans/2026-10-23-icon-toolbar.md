# Toolbar de solo íconos (UI en pixel art): plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Con Pixel UI encendido, la toolbar es una columna de 13 botones con el ícono del mockup
(marco `tool.slot` o `tool.slot.active`). Cada botón muestra la variante recordada de su grupo,
que se guarda con `ToolOptions`. La lista de variantes lleva íconos y hay tooltip con nombre y
atajo. Además, el scroll del panel ya no se arrastra, y un trazo a medias se corta si se entra
al modo máscara o al modo Tiles.

**Architecture:**
- **`assets/ui/skin.aseprite`:** 22 íconos `icon.*` de 17×17 y los marcos `tool.slot` /
  `tool.slot.active` de 23×23, copiados de `.design/Design.aseprite` (frame 2) con un script Lua
  en modo batch.
- **`cs-ui::editor`:** `GroupChoice`, la variante recordada de cada grupo, vive en
  `ToolOptions::group_choice`. `Editor::set_tool` la actualiza y `Editor::group_tool` dice qué
  muestra cada grupo.
- **`cs-ui::tool_group`** (nuevo): `group_button`, la lógica de un grupo (clic, clic derecho,
  mantener presionado, lista y tooltip) que usan las dos toolbars.
- **`cs-ui::pixel_ui`:** `tool_button`, que dibuja el marco, el ícono y la marca de variantes.
- **`cs-ui::panels`:** `toolbar(ui, editor, skin)` elige entre la columna de íconos y las filas
  con nombre. `tools_scroll_area()` arma el scroll del panel sin arrastre.
- **Editor:** `set_edit_mask`, `select_layer` y `add_mask` cortan el trazo si cambia a qué pinta
  la herramienta.

**Tech Stack:** Rust 2024, egui/eframe 0.36, Aseprite 1.3 en modo batch (solo para la Task 1),
Python con Pillow (solo para verificar la Task 1).

**Spec:** `docs/superpowers/specs/2026-10-23-icon-toolbar-design.md`.

## Global Constraints

- **Crates del motor:** `cs-core`, `cs-raster`, `cs-commands`, `cs-tools` y `cs-io` no dependen de
  egui. Este plan no los toca.
- **Idiomas:** código y comentarios en español; textos de la UI en inglés. Imita la densidad de
  comentarios del código vecino.
- **Glifos:** cada glifo de la UI tiene que existir en las fuentes por defecto de egui o en
  `chimpsprite.ttf`. Los textos nuevos (`Right-click or hold for more` y los nombres de las
  herramientas) son ASCII. La marca de variantes se dibuja con rectángulos, sin glifos. Los emoji
  de la barra con nombres ya están en las fuentes de egui.
- **Toolchain aislado:** antes de cualquier comando cargo, en Git Bash:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH`
- **Verificación por tarea:** `cargo fmt --all` y los tests del crate tocado (`cargo test -p cs-ui`).
- **Verificación final:**
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo check -p cs-ui --target wasm32-unknown-unknown`
- **GUI:**
  - Se compila solo con `CARGO_TARGET_DIR=target/gui-pixel`, nunca en el target por defecto.
  - El usuario puede tener abierto ese exe. Si el archivo está bloqueado, se salta la prueba
    manual.
  - Nunca se mata un proceso que no se lanzó en esta sesión.
  - Antes de abrir la app se copia `%APPDATA%/ChimpSprite/data/app.ron`, y se restaura al
    terminar.
- **`.design/`:** no se modifica nada adentro. `Design.aseprite` solo se abre para leer. Se
  verifica con sha256 antes y después.
- **Commits:** terminan con la línea `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Sin push ni PR.**
- **Rama:** `icon-toolbar`. No cambiar de rama.

## Desviaciones/decisiones (tomadas al planear)

Medidas tomadas en el mockup (`Design.aseprite`, frame 2 de Aseprite, que es el frame 1 en la
numeración desde 0 de la CLI), con los píxeles exportados y Lua en modo batch.

### Íconos

- **Miden 17×17, no 16×16.**
  - En el mockup, ocho dibujos pasan de 16 px:
    - 17 de alto: lápiz, goma, gotero y balde.
    - 17 de ancho: Text.
    - 17×17: Artboard, varita y Jumble.
  - El spec pide no recortar sin revisar, así que todas las celdas pasan a 17×17. No se
    recorta ningún píxel.
  - Los tests del skin piden 17×17.
- **Se copian donde están en el mockup, sin volver a centrarlos.**
  - Cada cuadrito del mockup mide 23×23: borde de 1, bisel de 1, relleno de 19, bisel de 1 y
    borde de 1.
  - Los 22 dibujos entran en la ventana de 17×17 que empieza a (3,3) del cuadrito, es decir, a
    1 px dentro del relleno. El anillo de 1 px que queda afuera es todo relleno: el script lo
    comprueba con `assertRing`.
  - `tool_button` centra el ícono de 17 en el marco de 23, o sea a (3,3). Así marco más ícono
    quedan iguales, píxel por píxel, que el cuadrito del mockup.
  - Volver a centrar los íconos de 13 o 15 px los correría medio píxel respecto al mockup.
- **Fondo transparente:** el relleno claro del cuadrito, (234,224,221), se vuelve transparente
  en todos los íconos. Se revisó ícono por ícono, y ningún dibujo usa ese color:
  - En la mayoría, todo el relleno toca el borde de la ventana.
  - Las únicas zonas de relleno encerradas son el interior de Rectangle, Ellipse, Text, Lasso y
    Polygonal Lasso, el hueco del asa del balde y dos huecos de 1 px entre la estrella y el
    mango de la varita.
  - Todas son fondo del cuadrito y, sobre el marco activo, tienen que verse café.
- **Medidas** (bbox del dibujo dentro del relleno de 19×19 y origen del cuadrito):

| Ícono | Cuadrito (x,y) | Dibujo | Ícono | Cuadrito (x,y) | Dibujo |
| --- | --- | --- | --- | --- | --- |
| marquee | (33,76) | 13×13 | ellipse_marquee | (462,218) | 13×13 |
| hand | (33,98) | 12×13 | lasso | (462,243) | 15×15 |
| pencil | (33,120) | 9×17 | polygon_lasso | (488,243) | 15×15 |
| eraser | (33,142) | 11×17 | magic_wand | (514,218) | 17×17 |
| eyedropper | (33,164) | 9×17 | spray | (332,193) | 14×16 |
| bucket | (33,186) | 13×17 | gradient | (514,243) | 13×13 |
| line | (33,208) | 15×15 | jumble | (410,268) | 17×17 |
| rectangle | (33,230) | 13×11 | ellipse | (560,283) | 13×11 |
| text | (33,252) | 17×13 | slice | (436,218) | 6×17 |
| contour | (33,274) | 15×15 | | | |
| blur | (33,296) | 12×15 | | | |
| move | (33,318) | 15×15 | | | |
| artboard | (33,340) | 17×17 | | | |

  - La columna de la toolbar es un cuadrito cada 22 px, desde (33,76): los cuadritos comparten
    la línea del borde.
  - El rect que se copia es (x+3, y+3, 17, 17).

### Marcos

- **`tool.slot`:** el cuadrito claro vacío de (493,181), de 23×23.
  - Borde negro (5,4,3) de 1 px.
  - Bisel blanco arriba y a la derecha; (221,206,191) a la izquierda y abajo.
  - Relleno de 19×19.
- **`tool.slot.active`:** el cuadrito café de (596,218), al lado del lápiz claro de (570,218).
  - Bisel (136,110,106) arriba y a la derecha, y relleno (189,164,153).
  - Para sacar el lápiz, todo el relleno de 19×19 se pinta de (189,164,153).
  - Se verificó contra el lápiz claro: donde el lápiz no tiene dibujo, el activo ya es
    (189,164,153) liso. Los únicos píxeles que cambian son los 129 del lápiz.
- **Las 4 esquinas** de los dos marcos son transparentes: son redondeadas y en el mockup
  muestran el fondo.
- **9-slice:** el centro es (2,2,19,19). Las esquinas y los bordes son borde + bisel, y el
  relleno es liso, así que se estira sin deformar nada.
- **`tool.slot.hover`:** queda sin dibujar y su slice pasa a 23×23 con el mismo centro, para
  cuando se dibuje.
- **Se usa el cuadrito suelto, no el de la columna.** Los cuadritos de la columna del mockup
  comparten el borde y lo tienen en (89,77,77). Los sueltos tienen borde negro y esquinas
  redondeadas, y son los que el spec nombra ("el cuadrito claro vacío").
  - En la app, los botones van uno debajo del otro sin espacio (`item_spacing.y = 0`), así que
    entre dos marcos queda una línea negra de 2 px.
  - Si el usuario quiere la columna con borde compartido, se cambia el espaciado a −1 o se
    dibuja otra pieza.

### Ubicación en `skin.aseprite`

- Los 19 slices `icon.*` que ya existían (vacíos, de 16×16 en y=130 y y=150) se reutilizan:
  cambian de bounds.
- Se crean `icon.hand`, `icon.contour` e `icon.blur`.
- **Fila y=128:** los 13 íconos de la columna, en x = 4, 24, …, 244.
- **Fila y=150:** las 9 variantes, en x = 4, 24, …, 164.
- **Fila y=100:** los marcos, en las x de siempre: `tool.slot` (4), `tool.slot.hover` (30) y
  `tool.slot.active` (56).
- **Sin superposiciones:**
  - Lo dibujado del skin ocupa y 4–71 y 176–194. Las filas nuevas ocupan y 100–122 y 128–166, y
    las pestañas `tab*` (y 76–89) siguen vacías.
  - Fuera de esos rects el skin no cambia: lo comprueba `chimpsprite-skin-rest.py`.

### Código

- **La variante recordada va en `ToolOptions::group_choice`** (`GroupChoice`).
  - Así está en el `Editor` (`editor.options`), se guarda con las opciones y pasa a cada pestaña
    nueva, como el resto de `ToolOptions`.
  - Se guarda como una lista de nombres de variante (`["RectSelect","Hand",…]`), uno por grupo.
  - Un nombre desconocido, de otro grupo o que falta vuelve a la primera herramienta del grupo.
  - Un ajuste viejo sin el campo usa la primera de cada grupo (`#[serde(default)]`).
  - `ToolKind` no pasa a ser `Serialize`.
- **`Editor::set_tool` siempre recuerda la herramienta** en su grupo, también desde el teclado.
- **`Editor::group_tool(g)`:** la herramienta activa si es del grupo; si no, la recordada.
- **`group_button` recibe `skin: Option<&Skin>`**, un parámetro más que en el spec: la lista de
  variantes lo necesita para los íconos. La firma es
  `group_button(ui, editor, skin, group, draw)`.
- **`ButtonState`:** `{ selected, more }`, si el grupo tiene la herramienta activa y si tiene
  variantes.
- **Tooltip:**
  - Es el mismo en las dos toolbars, porque las dos usan `group_button`. La barra con nombres
    gana la segunda línea "Right-click or hold for more" en los grupos con variantes.
  - La barra con nombres conserva su botón `⏷`, sin otros cambios.
- **Lista de variantes:**
  - Con skin, cada fila es ícono (17×17 puntos, o sea, la escala de la UI), nombre y atajo.
  - Sin skin, queda como hoy: emoji, nombre y atajo.
  - Con skin pero sin ese ícono, también va el emoji.
- **Nombre accesible** de cada botón de la columna: el nombre de la herramienta que muestra el
  grupo, por ejemplo "Rectangular Marquee", o "Lasso" si se eligió Lasso.
- **`tool_button`:**
  - **Marco:** `tool.slot.active` si está elegido; si no, `.down` presionado y `.hover` con el
    puntero encima. Hoy los dos caen a `tool.slot`.
  - **Ícono:** presionado baja 1 px, con `label_pos`, la misma regla que el texto de los botones.
  - **Sin el ícono:** la inicial del nombre con la fuente.
  - **Sin el marco:** el respaldo plano y, si está elegido, un borde `accent` de 1 px.
- **Marca de variantes:**
  - Un triángulo rectángulo de 3×3 px en `text_dim`, dibujado con tres rectángulos de 1, 2 y
    3 px.
  - Va en la esquina de abajo a la derecha del relleno: píxeles 18–20 del marco.
  - No se mueve al presionar. No pisa ningún ícono de los grupos con variantes.
- **Panel:** se queda en 184. La columna va a la izquierda, debajo del título "Tools" (que se
  conserva, como Options y Symmetry).
- **Scroll del panel:** `ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL`.
  - El valor por defecto de egui (`DragScroll::OnTouch`) ya no arrastra con mouse, pero sí con
    pantalla táctil. Ahí, mantener presionado un botón movía el panel.
  - El test simula un `Event::Touch` para que egui detecte la pantalla táctil. Sin el arreglo,
    falla con un desplazamiento de 47.5.
- **Cortar el trazo:**
  - `Tool::cancel` no hace nada en las herramientas de pincel (solo `PolygonLasso` lo
    implementa), así que `cancel_gesture` solo no termina un trazo de Blur.
  - Se agrega `Editor::abort_stroke`, privado: llama a `cancel_gesture()` y además deja la
    herramienta como recién creada (`SpriteTools::reset`, sin tocar `cs-tools`).
  - Se usa cuando cambia `paint_target()` = `(editing_mask(), tiles_mode())` a mitad de un
    trazo:
    - en `set_edit_mask` (nuevo; el timeline lo usa en vez de asignar el campo);
    - en `select_layer`;
    - en `add_mask`.
  - Además, `pointer_drag` y `pointer_release` cortan el trazo si la herramienta quedó
    bloqueada, aunque el modo haya cambiado por otro camino.
  - Esc (`cancel_gesture`) no cambia.
  - `reset` de Spray reinicia su contador de trazos: solo cambia la semilla del azar.
- **Nombre de test:** `tools_blocked_in_tiles_mode_hide_their_outline` solo prueba
  `tool_blocked()` para Spray, Blur y Jumble en modo Tiles, y que Pencil no está bloqueado.
  Pasa a llamarse `spray_blur_and_jumble_are_blocked_in_tiles_mode_and_the_pencil_is_not`.
- **Prototipo:** todo el código de este plan se probó al planear en una copia del repo:
  - `cargo test -p cs-ui`: 286 tests en verde.
  - `cargo clippy -p cs-ui --all-targets -- -D warnings`: limpio.
  - `cargo check -p cs-ui --target wasm32-unknown-unknown`: limpio.
- **`Design.aseprite` cambió mientras se planeaba.** Su mtime pasó a 13:03:25, unos 10 s después
  de una corrida del script.
  - Había un `Aseprite.exe` abierto que no se lanzó desde aquí: fue el usuario.
  - Los píxeles exportados de los dos frames son idénticos, y una segunda corrida dejó el sha
    igual.
  - Si en la Task 1 el sha de después no coincide, no se asume nada: se mira el mtime y se repite
    la verificación de píxeles con un frame 2 recién exportado.

## Archivos

| Archivo | Qué hace |
| --- | --- |
| Modificar `assets/ui/skin.aseprite` | 22 `icon.*` de 17×17; `tool.slot`, `tool.slot.active` (y el slice de `.hover`) de 23×23 |
| Modificar `assets/ui/README.md` | Tamaños y origen de íconos y marcos |
| Modificar `crates/cs-ui/src/skin/mod.rs` | Test del skin con los íconos y los marcos |
| Modificar `crates/cs-ui/src/editor.rs` | `ToolKind::group`, `GroupChoice`, `ToolOptions::group_choice`, `group_tool`; `SpriteTools::reset`, `paint_target`, `cancel_if_target_changed`, `abort_stroke`; cortes en `pointer_drag` y `pointer_release` |
| Crear `crates/cs-ui/src/tool_group.rs` | `ButtonState`, `emoji`, `icon_piece`, `tooltip`, `variants_list`, `group_button` (y `Hold`, `hold_state`, `variants_popup` movidos de `panels.rs`) |
| Modificar `crates/cs-ui/src/pixel_ui.rs` | `tool_button`, `more_mark` |
| Modificar `crates/cs-ui/src/panels.rs` | `tool_group` con `group_button`, `tool_column`, `tools_scroll_area`, `toolbar(ui, editor, skin)`; tests con harness |
| Modificar `crates/cs-ui/src/lib.rs` | `mod tool_group`; el panel pasa el skin y usa `tools_scroll_area` |
| Modificar `crates/cs-ui/src/mask_ops.rs` | `set_edit_mask`; `add_mask` corta el trazo |
| Modificar `crates/cs-ui/src/anim_ops.rs` | `select_layer` corta el trazo |
| Modificar `crates/cs-ui/src/timeline.rs` | Usa `set_edit_mask` |
| Modificar `crates/cs-ui/src/tool_tests.rs` | Tests de la variante recordada y de cortar el trazo; renombre |
| Modificar `crates/cs-ui/src/canvas.rs` | Test con el harness del lienzo |

---

### Task 1: Skin: íconos y marcos de herramienta del mockup

**Files:**
- Modify: `assets/ui/skin.aseprite`
- Modify: `assets/ui/README.md`
- Modify: `crates/cs-ui/src/skin/mod.rs` (tests)
- Scripts temporales (no se versionan): `/tmp/chimpsprite-copy-icons.lua`,
  `/tmp/chimpsprite-slices.lua`, `/tmp/chimpsprite-icons-check.py` y
  `/tmp/chimpsprite-skin-rest.py`

**Interfaces:**
- Produces:
  - Slices `icon.<id>` de 17×17, sin centro, en (4+20·i, 128) para los 13 de la columna y
    (4+20·i, 150) para las 9 variantes. Los `<id>`, en orden, son:
    - columna: `marquee`, `hand`, `pencil`, `eraser`, `eyedropper`, `bucket`, `line`,
      `rectangle`, `text`, `contour`, `blur`, `move` y `artboard`;
    - variantes: `ellipse_marquee`, `lasso`, `polygon_lasso`, `magic_wand`, `spray`,
      `gradient`, `jumble`, `ellipse` y `slice`.
  - `tool.slot` (4,100,23,23), `tool.slot.hover` (30,100,23,23, sin dibujar) y
    `tool.slot.active` (56,100,23,23), todos con centro (2,2,19,19).
  - Las tasks siguientes los leen con `crate::skin::EMBEDDED` (ya existe).

- [ ] **Step 1: Escribir el test del skin (falla con el skin de hoy)**

En `crates/cs-ui/src/skin/mod.rs`, dentro de `mod tests`, después de `use super::*;`, agregar:

```rust
    /// Los 22 íconos de herramientas del mockup (`icon.<nombre>`).
    const TOOL_ICONS: [&str; 22] = [
        "marquee",
        "hand",
        "pencil",
        "eraser",
        "eyedropper",
        "bucket",
        "line",
        "rectangle",
        "text",
        "contour",
        "blur",
        "move",
        "artboard",
        "ellipse_marquee",
        "lasso",
        "polygon_lasso",
        "magic_wand",
        "spray",
        "gradient",
        "jumble",
        "ellipse",
        "slice",
    ];
```

Y después del test `the_embedded_skin_has_the_mockup_colors_window_and_buttons`:

```rust
    #[test]
    fn the_embedded_skin_has_the_tool_icons_and_slots_of_the_mockup() {
        let data = decode(EMBEDDED).unwrap();
        // `decode` descarta las piezas transparentes: si está, tiene algo dibujado.
        for name in TOOL_ICONS {
            let icon = data
                .pieces
                .get(&format!("icon.{name}"))
                .unwrap_or_else(|| panic!("icon.{name}"));
            assert_eq!((icon.size, icon.center), ([17, 17], None), "icon.{name}");
        }
        let center = Rect9 {
            x: 2,
            y: 2,
            w: 19,
            h: 19,
        };
        for name in ["tool.slot", "tool.slot.active"] {
            let slot = &data.pieces[name];
            assert_eq!(
                (slot.size, slot.center),
                ([23, 23], Some(center)),
                "{name}"
            );
            // Esquinas redondeadas: transparentes.
            assert_eq!(slot.rgba[3], 0, "{name}");
        }
        assert!(
            !data.pieces.contains_key("tool.slot.hover"),
            "not drawn yet: falls back to tool.slot"
        );
    }
```

- [ ] **Step 2: Correrlo y comprobar que falla**

Run: `cargo test -p cs-ui the_embedded_skin_has_the_tool_icons`
Expected: FAIL con el panic `icon.marquee`, porque hoy los slices `icon.*` están vacíos y
`decode` los descarta.

- [ ] **Step 3: Huella de `.design/` y PNG del skin de antes**

En Git Bash, desde la raíz del repo:

```bash
ASEPRITE="/c/Program Files (x86)/Steam/steamapps/common/Aseprite/Aseprite.exe"
find .design -type f -print0 | sort -z | xargs -0 sha256sum > /tmp/chimpsprite-design-before.sha
ls -l --time-style=full-iso .design/Design.aseprite
"$ASEPRITE" -b assets/ui/skin.aseprite --save-as /tmp/chimpsprite-skin-before.png
```

Anotar la hora que muestra `ls`.

- [ ] **Step 4: Escribir el script que copia íconos y marcos**

Crear `/tmp/chimpsprite-copy-icons.lua`:

```lua
-- Copia los íconos de herramientas y los marcos tool.slot / tool.slot.active del mockup
-- (Design.aseprite, frame 2 en la numeración de Aseprite = frame 2 de Lua) al skin.
-- Uso: Aseprite.exe -b --script-param design=<Design.aseprite> --script-param skin=<skin.aseprite>
--      --script chimpsprite-copy-icons.lua
-- Design.aseprite solo se lee: se aplana en memoria y se cierra sin guardar.
local designPath = app.params["design"]
local skinPath = app.params["skin"]
assert(designPath and skinPath, "faltan --script-param design=... y skin=...")

local rgba = app.pixelColor.rgba
-- Relleno claro de los cuadritos: es el fondo del cuadrito, no parte de ningún dibujo.
local FILL = rgba(234, 224, 221, 255)
-- Relleno del cuadrito activo (café).
local ACTIVE_FILL = rgba(189, 164, 153, 255)
local CLEAR = rgba(0, 0, 0, 0)

-- Un cuadrito mide 23x23: borde de 1, bisel de 1, relleno de 19, bisel de 1, borde de 1.
local SLOT = 23
-- Los dibujos caben en 17x17 a 3 px del borde del cuadrito (1 px dentro del relleno).
local ICON = 17
local INSET = 3

-- Origen (esquina del borde) de cada cuadrito del mockup y destino del ícono en el skin.
local ICONS = {
  -- Columna de la toolbar: un cuadrito cada 22 px (comparten el borde), desde (33,76).
  { "marquee",         33,  76, 4,   128 },
  { "hand",            33,  98, 24,  128 },
  { "pencil",          33, 120, 44,  128 },
  { "eraser",          33, 142, 64,  128 },
  { "eyedropper",      33, 164, 84,  128 },
  { "bucket",          33, 186, 104, 128 },
  { "line",            33, 208, 124, 128 },
  { "rectangle",       33, 230, 144, 128 },
  { "text",            33, 252, 164, 128 },
  { "contour",         33, 274, 184, 128 },
  { "blur",            33, 296, 204, 128 },
  { "move",            33, 318, 224, 128 },
  { "artboard",        33, 340, 244, 128 },
  -- Variantes, sueltas al lado.
  { "ellipse_marquee", 462, 218, 4,   150 },
  { "lasso",           462, 243, 24,  150 },
  { "polygon_lasso",   488, 243, 44,  150 },
  { "magic_wand",      514, 218, 64,  150 },
  { "spray",           332, 193, 84,  150 },
  { "gradient",        514, 243, 104, 150 },
  { "jumble",          410, 268, 124, 150 },
  { "ellipse",         560, 283, 144, 150 },
  { "slice",           436, 218, 164, 150 },
}

-- Marcos: el cuadrito claro vacío y el café del lápiz activo (sin el lápiz).
local SLOTS = {
  { name = "tool.slot",        src = { x = 493, y = 181 }, dst = { x = 4,  y = 100 }, active = false },
  { name = "tool.slot.active", src = { x = 596, y = 218 }, dst = { x = 56, y = 100 }, active = true },
}
-- Esquinas fijas (borde + bisel) y relleno liso de 19x19 que se estira.
local SLOT_CENTER = Rectangle(2, 2, 19, 19)

-- Mockup aplanado, frame 2.
local design = app.open(designPath)
assert(#design.frames >= 2, "el mockup no tiene frame 2")
local flat = Image(design.spec)
flat:drawSprite(design, 2)
design:close()

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

-- Pone (o crea) el slice `name` en `bounds`, con `center` o sin centro.
local function setSlice(name, bounds, center)
  local s = sliceNamed(name)
  if not s then
    s = skin:newSlice(bounds)
    s.name = name
  end
  s.bounds = bounds
  s.center = center
end

-- Comprueba que el cuadrito de (sx,sy) está donde se espera: el anillo de 1 px del relleno
-- alrededor de la ventana de 17x17 es todo relleno (si no, el dibujo no entra o el mockup se
-- movió).
local function assertRing(name, sx, sy)
  for y = 2, SLOT - 3 do
    for x = 2, SLOT - 3 do
      local inWindow = x >= INSET and x < INSET + ICON and y >= INSET and y < INSET + ICON
      if not inWindow then
        assert(flat:getPixel(sx + x, sy + y) == FILL,
          string.format("%s: (%d,%d) no es relleno", name, sx + x, sy + y))
      end
    end
  end
end

for _, it in ipairs(ICONS) do
  local name, sx, sy, dx, dy = it[1], it[2], it[3], it[4], it[5]
  assertRing(name, sx, sy)
  for y = 0, ICON - 1 do
    for x = 0, ICON - 1 do
      local px = flat:getPixel(sx + INSET + x, sy + INSET + y)
      if px == FILL then px = CLEAR end
      full:drawPixel(dx + x, dy + y, px)
    end
  end
  setSlice("icon." .. name, Rectangle(dx, dy, ICON, ICON), nil)
end

local BLACK = rgba(5, 4, 3, 255)
for _, s in ipairs(SLOTS) do
  -- Borde negro de 1 px con las esquinas redondeadas (la esquina no es borde).
  assert(flat:getPixel(s.src.x + 1, s.src.y) == BLACK, s.name .. ": no está el borde de arriba")
  assert(flat:getPixel(s.src.x, s.src.y + 1) == BLACK, s.name .. ": no está el borde izquierdo")
  assert(flat:getPixel(s.src.x, s.src.y) ~= BLACK, s.name .. ": la esquina no es redondeada")
  for y = 0, SLOT - 1 do
    for x = 0, SLOT - 1 do
      local px = flat:getPixel(s.src.x + x, s.src.y + y)
      local corner = (x == 0 or x == SLOT - 1) and (y == 0 or y == SLOT - 1)
      local inFill = x >= 2 and x <= SLOT - 3 and y >= 2 and y <= SLOT - 3
      if corner then
        -- Esquinas redondeadas: afuera del borde.
        px = CLEAR
      elseif s.active and inFill then
        -- Se borra el lápiz: todo el relleno queda café.
        px = ACTIVE_FILL
      end
      full:drawPixel(s.dst.x + x, s.dst.y + y, px)
    end
  end
  setSlice(s.name, Rectangle(s.dst.x, s.dst.y, SLOT, SLOT), SLOT_CENTER)
end
-- El hover no está dibujado: solo toma el mismo tamaño para cuando se dibuje.
setSlice("tool.slot.hover", Rectangle(30, 100, SLOT, SLOT), SLOT_CENTER)

skin:newCel(layer, 1, full, Point(0, 0))
skin:saveAs(skinPath)
skin:close()
print("ok")
```

- [ ] **Step 5: Correr el script sobre `assets/ui/skin.aseprite`**

```bash
"$ASEPRITE" -b --script-param design=.design/Design.aseprite --script-param skin=assets/ui/skin.aseprite --script /tmp/chimpsprite-copy-icons.lua
```

Expected: imprime `ok`. Si un `assert` falla, el mockup cambió: no tocar `.design/`, avisar
con el mensaje del assert y parar.

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
"$ASEPRITE" -b assets/ui/skin.aseprite --script /tmp/chimpsprite-slices.lua | grep -E '^slice	(icon|tool)' | sort
"$ASEPRITE" -b assets/ui/skin.aseprite --script /tmp/chimpsprite-slices.lua | grep -c '^slice'
```

Expected (separado por tabs):

```
slice	icon.artboard	244	128	17	17	center	-
slice	icon.blur	204	128	17	17	center	-
slice	icon.bucket	104	128	17	17	center	-
slice	icon.contour	184	128	17	17	center	-
slice	icon.ellipse	144	150	17	17	center	-
slice	icon.ellipse_marquee	4	150	17	17	center	-
slice	icon.eraser	64	128	17	17	center	-
slice	icon.eyedropper	84	128	17	17	center	-
slice	icon.gradient	104	150	17	17	center	-
slice	icon.hand	24	128	17	17	center	-
slice	icon.jumble	124	150	17	17	center	-
slice	icon.lasso	24	150	17	17	center	-
slice	icon.line	124	128	17	17	center	-
slice	icon.magic_wand	64	150	17	17	center	-
slice	icon.marquee	4	128	17	17	center	-
slice	icon.move	224	128	17	17	center	-
slice	icon.pencil	44	128	17	17	center	-
slice	icon.polygon_lasso	44	150	17	17	center	-
slice	icon.rectangle	144	128	17	17	center	-
slice	icon.slice	164	150	17	17	center	-
slice	icon.spray	84	150	17	17	center	-
slice	icon.text	164	128	17	17	center	-
slice	tool.slot	4	100	23	23	center	2,2,19,19
slice	tool.slot.active	56	100	23	23	center	2,2,19,19
slice	tool.slot.hover	30	100	23	23	center	2,2,19,19
```

y `48`: eran 45 slices, y se agregan `icon.hand`, `icon.contour` e `icon.blur`.

- [ ] **Step 7: Verificar los píxeles contra el mockup**

```bash
"$ASEPRITE" -b assets/ui/skin.aseprite --save-as /tmp/chimpsprite-skin-after.png
"$ASEPRITE" -b .design/Design.aseprite --frame-range 1,1 --save-as /tmp/chimpsprite-design-frame2.png
```

(`--frame-range` cuenta desde 0: `1,1` es el frame 2 de Aseprite.)

Crear `/tmp/chimpsprite-icons-check.py`:

```python
# Compara cada ícono y marco del skin con su recorte del mockup (frame 2).
# Uso: python chimpsprite-icons-check.py <skin.png> <design-frame2.png>
import sys
from PIL import Image

skin = Image.open(sys.argv[1]).convert("RGBA")
design = Image.open(sys.argv[2]).convert("RGBA")
FILL = (234, 224, 221, 255)
ACTIVE_FILL = (189, 164, 153, 255)
ICONS = [
    ("marquee", 33, 76, 4, 128), ("hand", 33, 98, 24, 128), ("pencil", 33, 120, 44, 128),
    ("eraser", 33, 142, 64, 128), ("eyedropper", 33, 164, 84, 128), ("bucket", 33, 186, 104, 128),
    ("line", 33, 208, 124, 128), ("rectangle", 33, 230, 144, 128), ("text", 33, 252, 164, 128),
    ("contour", 33, 274, 184, 128), ("blur", 33, 296, 204, 128), ("move", 33, 318, 224, 128),
    ("artboard", 33, 340, 244, 128),
    ("ellipse_marquee", 462, 218, 4, 150), ("lasso", 462, 243, 24, 150),
    ("polygon_lasso", 488, 243, 44, 150), ("magic_wand", 514, 218, 64, 150),
    ("spray", 332, 193, 84, 150), ("gradient", 514, 243, 104, 150),
    ("jumble", 410, 268, 124, 150), ("ellipse", 560, 283, 144, 150),
    ("slice", 436, 218, 164, 150),
]
for name, sx, sy, dx, dy in ICONS:
    diff = clear = 0
    for y in range(17):
        for x in range(17):
            p = skin.getpixel((dx + x, dy + y))
            d = design.getpixel((sx + 3 + x, sy + 3 + y))
            if p[3] == 0:
                clear += 1
                if d != FILL:
                    diff += 1
            elif p != d or p == FILL:
                diff += 1
    # Lo que queda afuera de la ventana de 17x17 (1 px del relleno) es todo relleno.
    lost = sum(
        1
        for y in range(19)
        for x in range(19)
        if not (1 <= x <= 17 and 1 <= y <= 17)
        and design.getpixel((sx + 2 + x, sy + 2 + y)) != FILL
    )
    print(f"icon.{name} diff {diff} clear {clear} lost {lost}")

for name, sx, sy, dx, active in [("tool.slot", 493, 181, 4, False), ("tool.slot.active", 596, 218, 56, True)]:
    diff = clear = pencil = 0
    for y in range(23):
        for x in range(23):
            p = skin.getpixel((dx + x, 100 + y))
            d = design.getpixel((sx + x, sy + y))
            corner = x in (0, 22) and y in (0, 22)
            fill = 2 <= x <= 20 and 2 <= y <= 20
            if corner:
                clear += p[3] == 0
                diff += p[3] != 0
            elif active and fill:
                # Donde el lápiz claro (570,218) no tiene dibujo, el activo es café liso.
                if design.getpixel((570 + x, 218 + y)) == FILL:
                    diff += d != ACTIVE_FILL
                else:
                    pencil += 1
                diff += p != ACTIVE_FILL
            else:
                diff += p != d
    print(f"{name} diff {diff} clear {clear} pencil {pencil}")
print("window title", skin.getpixel((30, 26)), "button", skin.getpixel((20, 185)))
```

Crear `/tmp/chimpsprite-skin-rest.py`:

```python
# Fuera de los rectángulos de los íconos y marcos nuevos, el skin no cambió.
# Uso: python chimpsprite-skin-rest.py <skin-antes.png> <skin-después.png>
import sys
from PIL import Image

before = Image.open(sys.argv[1]).convert("RGBA")
after = Image.open(sys.argv[2]).convert("RGBA")
rects = [(4 + 20 * i, 128, 17, 17) for i in range(13)] + [(4 + 20 * i, 150, 17, 17) for i in range(9)]
rects += [(4, 100, 23, 23), (56, 100, 23, 23)]
inside = lambda x, y: any(rx <= x < rx + w and ry <= y < ry + h for rx, ry, w, h in rects)
changed = sum(
    1
    for y in range(before.height)
    for x in range(before.width)
    if not inside(x, y) and before.getpixel((x, y)) != after.getpixel((x, y))
)
print("size", before.size == after.size, "changed outside", changed)
```

```bash
python /tmp/chimpsprite-icons-check.py /tmp/chimpsprite-skin-after.png /tmp/chimpsprite-design-frame2.png
python /tmp/chimpsprite-skin-rest.py /tmp/chimpsprite-skin-before.png /tmp/chimpsprite-skin-after.png
```

(Git Bash convierte las rutas `/tmp/...` de los argumentos para el Python de Windows.)

Expected:

```
icon.marquee diff 0 clear 253 lost 0
icon.hand diff 0 clear 164 lost 0
icon.pencil diff 0 clear 160 lost 0
icon.eraser diff 0 clear 114 lost 0
icon.eyedropper diff 0 clear 202 lost 0
icon.bucket diff 0 clear 96 lost 0
icon.line diff 0 clear 246 lost 0
icon.rectangle diff 0 clear 245 lost 0
icon.text diff 0 clear 190 lost 0
icon.contour diff 0 clear 139 lost 0
icon.blur diff 0 clear 171 lost 0
icon.move diff 0 clear 200 lost 0
icon.artboard diff 0 clear 120 lost 0
icon.ellipse_marquee diff 0 clear 265 lost 0
icon.lasso diff 0 clear 240 lost 0
icon.polygon_lasso diff 0 clear 229 lost 0
icon.magic_wand diff 0 clear 175 lost 0
icon.spray diff 0 clear 149 lost 0
icon.gradient diff 0 clear 124 lost 0
icon.jumble diff 0 clear 64 lost 0
icon.ellipse diff 0 clear 249 lost 0
icon.slice diff 0 clear 208 lost 0
tool.slot diff 0 clear 4 pencil 0
tool.slot.active diff 0 clear 4 pencil 129
window title (226, 114, 133, 255) button (234, 224, 221, 255)
size True changed outside 0
```

Qué se lee:
- `diff 0`: cada píxel opaco es idéntico al del mockup, y cada transparente era relleno.
- `lost 0`: no quedó dibujo afuera de los 17×17.
- `pencil 129`: los únicos píxeles cambiados del marco activo son los del lápiz.
- La última línea: el resto del skin (ventana y botones) sigue intacto.

- [ ] **Step 8: Comprobar que `.design/` no cambió**

```bash
sha256sum -c /tmp/chimpsprite-design-before.sha
ls -l --time-style=full-iso .design/Design.aseprite
git status --short
```

Expected:
- Todas las líneas terminan en `OK`.
- El mtime es el mismo que en el Step 3.
- `git status` muestra solo `M assets/ui/skin.aseprite` y `?? .design/`.

Si el sha de `Design.aseprite` cambió:
- Si el mtime es posterior a la corrida y hay un Aseprite abierto por el usuario, lo guardó él.
  En ese caso:
  - Se repite el Step 7 con un frame 2 recién exportado.
  - Se anota en el reporte.
  - No se toca `.design/`.
- Si no, se para y se avisa.

- [ ] **Step 9: Correr el test del skin**

Run: `cargo test -p cs-ui skin::`
Expected: PASS (los tests del skin, incluido el nuevo).

- [ ] **Step 10: README de los assets**

En `assets/ui/README.md`, reemplazar la línea:

```markdown
- **Íconos:** `icon.<herramienta>` miden 16×16; `tool.slot` es el marco detrás del ícono.
```

por:

```markdown
- **Íconos de herramientas:** `icon.<herramienta>` miden 17×17 sobre fondo transparente (los
  nombres están en `icon_piece`, `crates/cs-ui/src/tool_group.rs`). Se dibujan centrados en el
  marco.
- **Marcos de herramienta:** `tool.slot` (normal) y `tool.slot.active` (la herramienta elegida)
  miden 23×23, con centro (2,2,19,19). `tool.slot.hover` está sin dibujar y cae al normal.
- **Origen:** los íconos, los marcos y los botones se copiaron del mockup del autor
  (`.design/Design.aseprite`, que no se versiona).
```

y la línea siguiente, `- **Botones:** …`, queda igual.

- [ ] **Step 11: Commit**

```bash
cargo fmt --all
git add assets/ui/skin.aseprite assets/ui/README.md crates/cs-ui/src/skin/mod.rs
git commit -m "Skin: íconos de herramientas y marcos tool.slot del mockup

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: La variante recordada de cada grupo en `ToolOptions`

**Files:**
- Modify: `crates/cs-ui/src/editor.rs`
- Test: `crates/cs-ui/src/tool_tests.rs`

**Interfaces:**
- Produces:
  - `ToolKind::group(self) -> usize`: índice en `ToolKind::GROUPS`.
  - `pub struct GroupChoice`, que es `Copy`, `PartialEq`, `Default`, `Serialize` y
    `Deserialize`:
    - `get(&self, group: usize) -> ToolKind`;
    - `remember(&mut self, kind: ToolKind)`.
  - `ToolOptions::group_choice: GroupChoice` (`pub`).
  - `Editor::group_tool(&self, group: usize) -> ToolKind`.
  - `Editor::set_tool` recuerda `kind` en su grupo.

- [ ] **Step 1: Escribir los tests**

En `crates/cs-ui/src/tool_tests.rs`, antes de `fn the_editor_paints_with_the_chosen_brush_shape`:

```rust
#[test]
fn a_group_remembers_the_last_tool_chosen_in_it() {
    let marquee = ToolKind::RectSelect.group();
    let mut e = editor(ToolKind::Pencil);
    assert_eq!(e.group_tool(marquee), ToolKind::RectSelect, "the first one");
    e.set_tool(ToolKind::Lasso);
    e.set_tool(ToolKind::Pencil);
    assert_eq!(e.group_tool(marquee), ToolKind::Lasso);
    // La activa manda en su grupo aunque se haya recordado otra.
    let pencil = ToolKind::Pencil.group();
    e.options.group_choice.remember(ToolKind::Spray);
    assert_eq!(e.group_tool(pencil), ToolKind::Pencil);
}

#[test]
fn the_group_choice_is_saved_with_the_tool_options() {
    use crate::editor::{GroupChoice, ToolOptions};
    let mut options = ToolOptions::default();
    options.group_choice.remember(ToolKind::Lasso);
    options.group_choice.remember(ToolKind::Slice);
    let json = serde_json::to_string(&options).unwrap();
    assert!(json.contains(r#""Lasso""#), "{json}");
    let back: ToolOptions = serde_json::from_str(&json).unwrap();
    assert_eq!(back.group_choice, options.group_choice);

    // Un ajuste de antes no la trae: cada grupo con su primera herramienta.
    let old: ToolOptions = serde_json::from_str(r#"{"brush_size":4}"#).unwrap();
    assert_eq!(old.group_choice, GroupChoice::default());
    assert_eq!(old.group_choice.get(0), ToolKind::RectSelect);

    // Nombres desconocidos, de otro grupo o que faltan: la primera del grupo.
    let odd: ToolOptions =
        serde_json::from_str(r#"{"group_choice":["Teleport","Pencil","Spray"]}"#).unwrap();
    assert_eq!(odd.group_choice.get(0), ToolKind::RectSelect, "unknown");
    assert_eq!(
        odd.group_choice.get(1),
        ToolKind::Hand,
        "Pencil is not a Hand tool"
    );
    assert_eq!(odd.group_choice.get(2), ToolKind::Spray);
    assert_eq!(odd.group_choice.get(5), ToolKind::Bucket, "missing");
}
```

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui group`
Expected: FAIL al compilar: `no method named group`, `cannot find type GroupChoice`, `no field
group_choice`.

- [ ] **Step 3: Implementar**

En `crates/cs-ui/src/editor.rs`:

1. En `impl ToolKind`, justo después de `GROUPS`:

```rust
    /// Índice en `GROUPS` del grupo que tiene a `self`.
    pub fn group(self) -> usize {
        ToolKind::GROUPS
            .iter()
            .position(|g| g.contains(&self))
            .expect("every tool is in a group")
    }
```

2. Antes de `/// Tamaño máximo del pincel (slider de Options y Ctrl+rueda).`:

```rust
/// La herramienta que muestra cada grupo de la barra: la última elegida en él.
///
/// Se guarda con las opciones como una lista de nombres de variante (`"Lasso"`), uno por grupo en
/// el orden de `GROUPS`. Si falta un nombre, no se conoce o es de otro grupo, ese grupo vuelve a su
/// primera herramienta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupChoice([ToolKind; ToolKind::GROUPS.len()]);

impl Default for GroupChoice {
    fn default() -> Self {
        Self(ToolKind::GROUPS.map(|g| g[0]))
    }
}

impl GroupChoice {
    /// La herramienta recordada del grupo `group`.
    pub fn get(&self, group: usize) -> ToolKind {
        self.0[group]
    }

    /// Recuerda `kind` como la elegida de su grupo.
    pub fn remember(&mut self, kind: ToolKind) {
        self.0[kind.group()] = kind;
    }
}

impl serde::Serialize for GroupChoice {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|k| format!("{k:?}")))
    }
}

impl<'de> serde::Deserialize<'de> for GroupChoice {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let names = <Vec<String> as serde::Deserialize>::deserialize(d)?;
        let mut choice = GroupChoice::default();
        for (slot, (group, name)) in choice.0.iter_mut().zip(ToolKind::GROUPS.iter().zip(&names)) {
            if let Some(&kind) = group.iter().find(|k| format!("{k:?}") == *name) {
                *slot = kind;
            }
        }
        Ok(choice)
    }
}
```

3. En `struct ToolOptions`, después de `pub rotation: cs_raster::RotationAlgorithm,`:

```rust
    /// La variante que muestra cada grupo de la barra.
    pub group_choice: GroupChoice,
```

y en `impl Default for ToolOptions`, después de `rotation: cs_raster::RotationAlgorithm::RotSprite,`:

```rust
            group_choice: GroupChoice::default(),
```

4. Reemplazar `set_tool` y agregar `group_tool` justo después:

```rust
    /// Cambia de herramienta descartando gestos de varios clics a medias. La herramienta queda
    /// como la elegida de su grupo en la barra.
    pub fn set_tool(&mut self, kind: ToolKind) {
        self.options.group_choice.remember(kind);
        if kind != self.tool {
            self.commit_floating();
            self.cancel_gesture();
            self.tool = kind;
        }
    }

    /// La herramienta que muestra el grupo `group` de la barra: la activa si es de ese grupo; si
    /// no, la última elegida en él.
    pub fn group_tool(&self, group: usize) -> ToolKind {
        if ToolKind::GROUPS[group].contains(&self.tool) {
            self.tool
        } else {
            self.options.group_choice.get(group)
        }
    }
```

- [ ] **Step 4: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS. Hasta la Task 3, `cargo build` avisa que `group_tool` no se usa fuera de los
tests; es esperado.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src/editor.rs crates/cs-ui/src/tool_tests.rs
git commit -m "cs-ui: la variante recordada de cada grupo va en ToolOptions

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Botón de grupo compartido (`tool_group.rs`)

**Files:**
- Create: `crates/cs-ui/src/tool_group.rs`
- Modify: `crates/cs-ui/src/panels.rs`
- Modify: `crates/cs-ui/src/lib.rs`

**Interfaces:**
- Consumes: `Editor::group_tool`, `Editor::set_tool` (Task 2) y `skin::{Skin, WidgetState}`.
- Produces (en `crate::tool_group`):
  - `pub struct ButtonState { pub selected: bool, pub more: bool }`
  - `pub fn emoji(kind: ToolKind) -> &'static str`
  - `pub fn icon_piece(kind: ToolKind) -> &'static str`, por ejemplo `"icon.lasso"`.
  - `pub fn tooltip(kind: ToolKind, more: bool) -> String`
  - `pub fn variants_list(ui: &mut Ui, skin: Option<&Skin>, current: ToolKind, group: &[ToolKind]) -> Option<ToolKind>`
  - `pub fn group_button(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, group: usize, draw: impl FnOnce(&mut Ui, ToolKind, ButtonState) -> Response) -> Response`
  - En `panels.rs`, `fn tool_group(ui, editor, index: usize) -> egui::Response`: ya no recibe
    el slice del grupo.

- [ ] **Step 1: Escribir los tests en el módulo nuevo**

Crear `crates/cs-ui/src/tool_group.rs` solo con:

```rust
//! Botón de un grupo de la barra de herramientas, compartido por la barra con nombres y la de
//! íconos (Pixel UI): clic para elegir la herramienta del grupo; clic derecho o mantener
//! presionado para la lista de variantes.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tooltip_has_the_name_the_shortcut_and_how_to_see_the_variants() {
        assert_eq!(tooltip(ToolKind::Hand, false), "Hand (H)");
        assert_eq!(
            tooltip(ToolKind::Lasso, true),
            "Lasso (Q)\nRight-click or hold for more"
        );
    }

    #[test]
    fn every_tool_has_its_own_icon_in_the_embedded_skin() {
        let data = crate::skin::decode(crate::skin::EMBEDDED).unwrap();
        let kinds: Vec<ToolKind> = ToolKind::GROUPS
            .iter()
            .flat_map(|g| g.iter().copied())
            .collect();
        for (i, &kind) in kinds.iter().enumerate() {
            let name = icon_piece(kind);
            assert_eq!(data.pieces[name].size, [17, 17], "{kind:?}");
            assert!(
                kinds[..i].iter().all(|&k| icon_piece(k) != name),
                "{name} twice"
            );
        }
    }
}
```

En `crates/cs-ui/src/lib.rs`, después de `mod timeline;`:

```rust
mod tool_group;
```

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui tool_group`
Expected: FAIL al compilar: `cannot find function tooltip`, `cannot find function icon_piece`,
`cannot find type ToolKind`.

- [ ] **Step 3: Implementar `tool_group.rs`**

Reemplazar todo `crates/cs-ui/src/tool_group.rs` por:

```rust
//! Botón de un grupo de la barra de herramientas, compartido por la barra con nombres y la de
//! íconos (Pixel UI): clic para elegir la herramienta del grupo; clic derecho o mantener
//! presionado para la lista de variantes.

use eframe::egui::{self, Response, Ui, vec2};

use crate::editor::{Editor, ToolKind};
use crate::skin::{Skin, WidgetState};

/// Mantener presionado el botón de un grupo este tiempo (en segundos) abre la lista de variantes.
const LONG_PRESS_S: f64 = 0.4;

/// Segunda línea del tooltip de los grupos con variantes.
const MORE_HINT: &str = "Right-click or hold for more";

/// Cómo se dibuja el botón de un grupo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonState {
    /// La herramienta activa es de este grupo.
    pub selected: bool,
    /// El grupo tiene más de una herramienta.
    pub more: bool,
}

/// Emoji de la barra con nombres (están en las fuentes de egui).
pub fn emoji(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::RectSelect => "🔲",
        ToolKind::EllipseSelect => "🔘",
        ToolKind::Lasso => "➰",
        ToolKind::PolygonLasso => "🔷",
        ToolKind::MagicWand => "✨",
        ToolKind::Move => "✚",
        ToolKind::Hand => "✋",
        ToolKind::Contour => "➿",
        ToolKind::Blur => "☁",
        ToolKind::Pencil => "✏",
        ToolKind::Spray => "💨",
        ToolKind::Eraser => "🗑",
        ToolKind::Eyedropper => "💧",
        ToolKind::Bucket => "🍶",
        ToolKind::Gradient => "🌈",
        ToolKind::Line => "📏",
        ToolKind::Rectangle => "⬜",
        ToolKind::Ellipse => "⭕",
        ToolKind::Jumble => "🎲",
        ToolKind::Text => "🔤",
        ToolKind::Slice => "✂",
        ToolKind::Artboard => "🖼",
    }
}

/// Pieza del skin con el ícono de la herramienta.
pub fn icon_piece(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::RectSelect => "icon.marquee",
        ToolKind::EllipseSelect => "icon.ellipse_marquee",
        ToolKind::Lasso => "icon.lasso",
        ToolKind::PolygonLasso => "icon.polygon_lasso",
        ToolKind::MagicWand => "icon.magic_wand",
        ToolKind::Move => "icon.move",
        ToolKind::Hand => "icon.hand",
        ToolKind::Contour => "icon.contour",
        ToolKind::Blur => "icon.blur",
        ToolKind::Pencil => "icon.pencil",
        ToolKind::Spray => "icon.spray",
        ToolKind::Eraser => "icon.eraser",
        ToolKind::Eyedropper => "icon.eyedropper",
        ToolKind::Bucket => "icon.bucket",
        ToolKind::Gradient => "icon.gradient",
        ToolKind::Line => "icon.line",
        ToolKind::Rectangle => "icon.rectangle",
        ToolKind::Ellipse => "icon.ellipse",
        ToolKind::Jumble => "icon.jumble",
        ToolKind::Text => "icon.text",
        ToolKind::Slice => "icon.slice",
        ToolKind::Artboard => "icon.artboard",
    }
}

/// "Nombre (Atajo)" y, si el grupo tiene variantes, una segunda línea que dice cómo verlas.
pub fn tooltip(kind: ToolKind, more: bool) -> String {
    let first = format!("{} ({})", kind.label(), kind.shortcut());
    if more {
        format!("{first}\n{MORE_HINT}")
    } else {
        first
    }
}

/// Qué pasa con el botón de un grupo mantenido presionado, en este frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hold {
    /// Nada que hacer.
    Idle,
    /// Se cumplió el tiempo: abrir la lista.
    Open,
    /// Se suelta el botón con la lista abierta por esta presión: el clic no elige ni cierra.
    Release,
}

/// Lleva la cuenta de una presión larga con el botón principal sobre `response`.
fn hold_state(ui: &Ui, response: &Response) -> Hold {
    let id = response.id.with("long_press");
    let opened = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let (down, held) = ui.input(|i| {
        let held = i.pointer.press_start_time().map(|t| i.time - t);
        (i.pointer.primary_down(), held.unwrap_or(0.0))
    });
    if down && response.is_pointer_button_down_on() {
        if opened {
            return Hold::Idle;
        }
        if held >= LONG_PRESS_S {
            ui.data_mut(|d| d.insert_temp(id, true));
            return Hold::Open;
        }
        // Sin eventos egui no vuelve a pintar: pide un frame para cuando se cumpla el tiempo.
        ui.ctx()
            .request_repaint_after_secs((LONG_PRESS_S - held) as f32);
        return Hold::Idle;
    }
    if opened {
        ui.data_mut(|d| d.remove::<bool>(id));
        return Hold::Release;
    }
    Hold::Idle
}

/// Lista de variantes de un grupo junto al puntero. Se abre con clic derecho o manteniendo
/// presionado, y un clic normal en el botón la cierra. Usa el mismo id que `context_menu`.
fn variants_popup(response: &Response, hold: Hold, add_contents: impl FnOnce(&mut Ui)) {
    use egui::{Popup, PopupCloseBehavior, SetOpenCommand};
    let open = if hold == Hold::Open || response.secondary_clicked() {
        Some(SetOpenCommand::Bool(true))
    } else if response.clicked() && hold != Hold::Release {
        Some(SetOpenCommand::Bool(false))
    } else {
        None
    };
    let mut popup = Popup::menu(response).open_memory(open).at_pointer_fixed();
    if hold == Hold::Release {
        // Soltar el botón cuenta como clic: no tiene que cerrar la lista recién abierta.
        popup = popup.close_behavior(PopupCloseBehavior::IgnoreClicks);
    }
    popup.show(add_contents);
}

/// Las herramientas de un grupo, una por fila con su atajo. Con skin, cada una lleva su ícono a
/// tamaño de pieza (la escala de la UI); sin skin o si falta el ícono, el emoji de siempre.
/// Devuelve la elegida y cierra el menú.
pub fn variants_list(
    ui: &mut Ui,
    skin: Option<&Skin>,
    current: ToolKind,
    group: &[ToolKind],
) -> Option<ToolKind> {
    let mut pick = None;
    for &kind in group {
        let selected = current == kind;
        let icon = skin.and_then(|s| s.piece(icon_piece(kind), WidgetState::Normal));
        let item = match icon {
            Some(piece) => {
                let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
                let image =
                    egui::Image::new(egui::load::SizedTexture::new(piece.texture.id(), size))
                        .fit_to_exact_size(size);
                egui::Button::selectable(selected, (image, kind.label()))
            }
            None => egui::Button::selectable(selected, format!("{} {}", emoji(kind), kind.label())),
        };
        if ui.add(item.shortcut_text(kind.shortcut())).clicked() {
            pick = Some(kind);
            ui.close();
        }
    }
    pick
}

/// El botón del grupo `group` de `ToolKind::GROUPS`. `draw` lo dibuja con la herramienta que
/// muestra el grupo y su estado, y devuelve su `Response`.
///
/// - Clic: elige esa herramienta.
/// - Clic derecho o mantener presionado 400 ms (si el grupo tiene variantes): abre la lista;
///   soltar después de mantener no cuenta como clic.
/// - Elegir en la lista: cambia la herramienta, que queda recordada en el grupo.
/// - Tooltip: "Nombre (Atajo)" y, con variantes, cómo verlas.
pub fn group_button(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    group: usize,
    draw: impl FnOnce(&mut Ui, ToolKind, ButtonState) -> Response,
) -> Response {
    let members = ToolKind::GROUPS[group];
    let shown = editor.group_tool(group);
    let state = ButtonState {
        selected: members.contains(&editor.tool),
        more: members.len() > 1,
    };
    let response = draw(ui, shown, state);
    let hold = if state.more {
        hold_state(ui, &response)
    } else {
        Hold::Idle
    };
    let mut pick = None;
    if response.clicked() && hold != Hold::Release {
        pick = Some(shown);
    }
    if state.more {
        let current = editor.tool;
        variants_popup(&response, hold, |ui| {
            if let Some(kind) = variants_list(ui, skin, current, members) {
                pick = Some(kind);
            }
        });
    }
    if let Some(kind) = pick {
        editor.set_tool(kind);
    }
    response.on_hover_text(tooltip(shown, state.more))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tooltip_has_the_name_the_shortcut_and_how_to_see_the_variants() {
        assert_eq!(tooltip(ToolKind::Hand, false), "Hand (H)");
        assert_eq!(
            tooltip(ToolKind::Lasso, true),
            "Lasso (Q)\nRight-click or hold for more"
        );
    }

    #[test]
    fn every_tool_has_its_own_icon_in_the_embedded_skin() {
        let data = crate::skin::decode(crate::skin::EMBEDDED).unwrap();
        let kinds: Vec<ToolKind> = ToolKind::GROUPS
            .iter()
            .flat_map(|g| g.iter().copied())
            .collect();
        for (i, &kind) in kinds.iter().enumerate() {
            let name = icon_piece(kind);
            assert_eq!(data.pieces[name].size, [17, 17], "{kind:?}");
            assert!(
                kinds[..i].iter().all(|&k| icon_piece(k) != name),
                "{name} twice"
            );
        }
    }
}
```

- [ ] **Step 4: La barra con nombres usa `group_button`**

En `crates/cs-ui/src/panels.rs`:

1. Cambiar `use crate::{theme, tile_ops, widgets};` por:

```rust
use crate::{theme, tile_ops, tool_group, widgets};
```

2. Borrar, desde `fn icon(kind: ToolKind) -> &'static str {` hasta el final de
   `fn group_menu(...)`: `icon`, `LONG_PRESS_S`, `Hold`, `hold_state`, `variants_popup`, el
   `tool_group` viejo y `group_menu`. En su lugar va:

```rust
/// Fila de un grupo en la barra con nombres: el botón con la herramienta que muestra el grupo y,
/// si hay más, un desplegable con las demás (también con clic derecho o manteniendo presionado),
/// como en Aseprite. Devuelve la respuesta del botón.
fn tool_group(ui: &mut egui::Ui, editor: &mut Editor, index: usize) -> egui::Response {
    let group = ToolKind::GROUPS[index];
    let more = group.len() > 1;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let width = ui.available_width() - if more { 22.0 } else { 0.0 };
        let response = tool_group::group_button(ui, editor, None, index, |ui, shown, state| {
            let label = format!("{} {}", tool_group::emoji(shown), shown.label());
            let button = egui::Button::selectable(state.selected, label)
                .shortcut_text("")
                .truncate();
            ui.add_sized([width, 24.0], button)
        });
        if more {
            let current = editor.tool;
            let mut pick = None;
            ui.menu_button("⏷", |ui| {
                pick = tool_group::variants_list(ui, None, current, group);
            })
            .response
            .on_hover_text("More tools (or right-click, or hold)");
            if let Some(kind) = pick {
                editor.set_tool(kind);
            }
        }
        response
    })
    .inner
}
```

3. En `toolbar`, cambiar el bucle:

```rust
    for (index, group) in ToolKind::GROUPS.iter().enumerate() {
        tool_group(ui, editor, index, group);
    }
```

por:

```rust
    for index in 0..ToolKind::GROUPS.len() {
        tool_group(ui, editor, index);
    }
```

4. En el test helper `group_frame`, cambiar
   `rect = tool_group(ui, editor, group, ToolKind::GROUPS[group]).rect;` por:

```rust
                rect = tool_group(ui, editor, group).rect;
```

- [ ] **Step 5: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS: los 2 tests de `tool_group`, y `right_click_or_holding_a_group_button_opens_its_variants`
sin cambios.

- [ ] **Step 6: Commit**

```bash
git add crates/cs-ui/src/tool_group.rs crates/cs-ui/src/panels.rs crates/cs-ui/src/lib.rs
git commit -m "cs-ui: botón de grupo compartido con tooltip y variante recordada en el editor

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: `tool_button`: marco, ícono y marca de variantes

**Files:**
- Modify: `crates/cs-ui/src/pixel_ui.rs`

**Interfaces:**
- Consumes: `piece_shapes`, `flat_shapes`, `borders`, `label_pos` y `FALLBACK_BUTTON`, que ya
  existen en `pixel_ui.rs`, y los slices de la Task 1.
- Produces:
  - `pub fn tool_button(ui: &mut Ui, skin: &Skin, icon: &str, label: &str, selected: bool, more: bool) -> Response`:
    tamaño de `tool.slot` (23×23), o 23×23 de respaldo; nombre accesible `label`.
  - `fn more_mark(corner: Pos2, pixels_per_point: f32) -> [Rect; 3]`.

- [ ] **Step 1: Escribir los tests**

En `crates/cs-ui/src/pixel_ui.rs`, dentro de `mod tests`, antes de
`fn a_pressed_label_goes_one_point_down`:

```rust
    /// Un frame con un botón de herramienta (Lasso, con variantes): su rectángulo, si se hizo
    /// clic y las texturas que se dibujaron.
    fn tool(
        ctx: &egui::Context,
        skin: &Skin,
        selected: bool,
        events: Vec<Event>,
    ) -> (Rect, bool, Vec<egui::TextureId>) {
        let mut out = (Rect::NOTHING, false);
        let input = RawInput { events, ..screen() };
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let r = tool_button(ui, skin, "icon.lasso", "Lasso", selected, true);
                out = (r.rect, r.clicked());
            });
        });
        // Las texturas (fuentes y skin) no se suben a ninguna GPU.
        output.textures_delta.clear();
        let mut textures = Vec::new();
        for clipped in &output.shapes {
            collect_textures(&clipped.shape, &mut textures);
        }
        (out.0, out.1, textures)
    }

    fn collect_textures(shape: &Shape, out: &mut Vec<egui::TextureId>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_textures(s, out)),
            Shape::Mesh(mesh) => out.push(mesh.texture_id),
            _ => {}
        }
    }

    #[test]
    fn a_tool_button_is_the_slot_with_its_icon_and_the_active_slot_when_selected() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let id = |name: &str| skin.piece(name, WidgetState::Normal).unwrap().texture.id();
        let (rect, _, textures) = tool(&ctx, &skin, false, vec![]);
        assert_eq!(rect.size(), vec2(23.0, 23.0), "the slot size");
        assert!(textures.contains(&id("tool.slot")));
        assert!(textures.contains(&id("icon.lasso")));
        let (_, _, textures) = tool(&ctx, &skin, true, vec![]);
        assert!(textures.contains(&id("tool.slot.active")));
        assert!(!textures.contains(&id("tool.slot")));
    }

    #[test]
    fn a_tool_button_clicks_with_the_skin_and_with_the_flat_fallback() {
        let ctx = egui::Context::default();
        let skins = [
            Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
            Skin::fallback(),
        ];
        for skin in &skins {
            let (rect, _, _) = tool(&ctx, skin, false, vec![]);
            assert_eq!(rect.size(), vec2(23.0, 23.0));
            let at = rect.center();
            tool(&ctx, skin, false, vec![Event::PointerMoved(at)]);
            tool(&ctx, skin, false, vec![press(at, true)]);
            let (_, clicked, _) = tool(&ctx, skin, false, vec![press(at, false)]);
            assert!(clicked);
        }
    }

    #[test]
    fn the_variants_mark_is_a_three_pixel_triangle_in_the_corner() {
        for ppp in [1.0, 2.0, 3.0] {
            let rows = more_mark(pos2(21.0, 21.0), ppp);
            assert_eq!(
                rows,
                [
                    Rect::from_min_max(pos2(20.0, 18.0), pos2(21.0, 19.0)),
                    Rect::from_min_max(pos2(19.0, 19.0), pos2(21.0, 20.0)),
                    Rect::from_min_max(pos2(18.0, 20.0), pos2(21.0, 21.0)),
                ],
                "at {ppp}"
            );
        }
    }
```

El ícono que baja 1 px al presionar lo cubre el test que ya existe,
`a_pressed_label_goes_one_point_down`: `tool_button` usa el mismo `label_pos`.

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui pixel_ui`
Expected: FAIL al compilar: `cannot find function tool_button`, `cannot find function more_mark`.

- [ ] **Step 3: Implementar**

En `crates/cs-ui/src/pixel_ui.rs`, después de
`const FALLBACK_CLOSE_SIZE: Vec2 = Vec2::new(9.0, 9.0);`:

```rust
/// Marco de los botones de herramienta; `.active` cuando su grupo tiene la herramienta activa.
const SLOT_PIECE: &str = "tool.slot";
/// Tamaño del botón de herramienta de respaldo (el del marco del mockup).
const FALLBACK_SLOT_SIZE: Vec2 = Vec2::new(23.0, 23.0);
/// Lado de la marca de variantes, en píxeles del skin.
const MORE_MARK: f32 = 3.0;
```

Y antes de `#[cfg(test)]`:

```rust
/// Botón de herramienta de la barra de íconos: el marco `tool.slot` (`tool.slot.active` con
/// `selected`) a tamaño de pieza y el ícono `icon` centrado en él. Presionado, el ícono baja 1 px.
/// Con `more`, un triangulito de 3×3 en la esquina de abajo a la derecha del relleno marca que
/// hay variantes.
///
/// Sin el ícono se dibuja la inicial de `label`; sin el marco, el respaldo plano (con un borde
/// `accent` si está elegido). `label` es también el nombre accesible.
// La toolbar de íconos lo usa desde la Task 5.
#[cfg_attr(not(test), allow(dead_code))]
pub fn tool_button(
    ui: &mut Ui,
    skin: &Skin,
    icon: &str,
    label: &str,
    selected: bool,
    more: bool,
) -> Response {
    let base = skin.piece(SLOT_PIECE, WidgetState::Normal);
    let size = base.map_or(FALLBACK_SLOT_SIZE, |p| {
        vec2(p.size[0] as f32, p.size[1] as f32)
    });
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let pressed = response.is_pointer_button_down_on();
    let state = if selected {
        WidgetState::Active
    } else if pressed {
        WidgetState::Down
    } else if response.hovered() {
        WidgetState::Hover
    } else {
        WidgetState::Normal
    };
    let colors = *skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = match skin.piece(SLOT_PIECE, state) {
        Some(piece) => piece_shapes(piece, rect, ppp),
        None => {
            let mut flat = flat_shapes(skin, rect, 0.0, ppp);
            if selected {
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
    let icon_state = if pressed {
        WidgetState::Down
    } else {
        WidgetState::Normal
    };
    match skin.piece(icon, WidgetState::Normal) {
        Some(piece) => {
            let icon_size = vec2(piece.size[0] as f32, piece.size[1] as f32);
            let at = label_pos(rect, icon_size, icon_state, ppp);
            shapes.extend(piece_shapes(piece, Rect::from_min_size(at, icon_size), ppp));
        }
        None => {
            let initial: String = label.chars().take(1).collect();
            let font = TextStyle::Button.resolve(ui.style());
            let galley = ui.painter().layout_no_wrap(initial, font, colors.text);
            let at = label_pos(rect, galley.size(), icon_state, ppp);
            shapes.push(Shape::galley(at, galley, colors.text));
        }
    }
    if more {
        let [_, _, right, bottom] = base.map_or(FALLBACK_BUTTON, borders);
        let corner = rect.max - vec2(right, bottom);
        shapes.extend(
            more_mark(corner, ppp)
                .into_iter()
                .map(|r| Shape::rect_filled(r, 0.0, colors.text_dim)),
        );
    }
    ui.painter().extend(shapes);
    response
}

/// Triangulito de 3×3 px que marca un grupo con variantes, con el ángulo recto abajo a la
/// derecha, en `corner`: tres filas de 1, 2 y 3 px, alineadas a píxeles de pantalla.
fn more_mark(corner: Pos2, pixels_per_point: f32) -> [Rect; 3] {
    let top = corner.y - MORE_MARK;
    [1.0, 2.0, 3.0].map(|w| {
        let y = top + w - 1.0;
        Rect::from_min_max(pos2(corner.x - w, y), pos2(corner.x, y + 1.0))
            .round_to_pixels(pixels_per_point)
    })
}
```

Notas:
- Con el marco del skin, `borders` da (2, 2, 2, 2), así que `corner` es el píxel (21,21) del
  marco y la marca ocupa los píxeles 18–20 del relleno.
- El ícono de 17 queda en (3,3) del marco de 23, igual que en el mockup.

- [ ] **Step 4: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS (los 3 tests nuevos de `pixel_ui` y todos los de antes).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src/pixel_ui.rs
git commit -m "Pixel UI: botón de herramienta con marco, ícono y marca de variantes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Toolbar de solo íconos con Pixel UI

**Files:**
- Modify: `crates/cs-ui/src/panels.rs`
- Modify: `crates/cs-ui/src/pixel_ui.rs` (se quita el `allow(dead_code)`)
- Modify: `crates/cs-ui/src/lib.rs`

**Interfaces:**
- Consumes:
  - `tool_group::{group_button, icon_piece}` (Task 3);
  - `pixel_ui::tool_button` (Task 4);
  - `PixelUi::skin() -> Option<&Skin>` (ya existe).
- Produces:
  - `pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>)`;
  - `fn tool_column(ui: &mut egui::Ui, editor: &mut Editor, skin: &Skin)`.

- [ ] **Step 1: Escribir el harness y los tests**

En `crates/cs-ui/src/panels.rs`, al final de `mod tests` (antes de la última `}`):

```rust
    /// La columna de íconos con el skin embebido, manejada con eventos crudos (el reloj avanza
    /// 50 ms por frame). Guarda los botones del árbol de accesibilidad (nombre y rect) y las
    /// texturas dibujadas.
    struct ColumnHarness {
        ctx: egui::Context,
        editor: Editor,
        skin: Skin,
        time: f64,
        /// Alto de la pantalla: con 150 la columna (13 × 23) no entra y hay scroll.
        height: f32,
        buttons: Vec<(String, Rect)>,
        textures: Vec<egui::TextureId>,
    }

    impl ColumnHarness {
        fn new(height: f32) -> Self {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
            let mut h = Self {
                ctx,
                editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
                skin,
                time: 0.0,
                height,
                buttons: Vec::new(),
                textures: Vec::new(),
            };
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.frame_after(0.05, events);
        }

        fn frame_after(&mut self, dt: f64, events: Vec<Event>) {
            self.time += dt;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(200.0, self.height),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let (editor, skin) = (&mut self.editor, &self.skin);
            let mut output = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| tool_column(ui, editor, skin));
            });
            output.textures_delta.clear();
            self.textures.clear();
            for clipped in &output.shapes {
                collect_textures(&clipped.shape, &mut self.textures);
            }
            self.buttons = output
                .platform_output
                .accesskit_update
                .map(|update| {
                    update
                        .nodes
                        .iter()
                        .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
                        .filter_map(|(_, n)| {
                            let b = n.bounds()?;
                            let rect = Rect::from_min_max(
                                pos2(b.x0 as f32, b.y0 as f32),
                                pos2(b.x1 as f32, b.y1 as f32),
                            );
                            Some((n.label()?.to_string(), rect))
                        })
                        .collect()
                })
                .unwrap_or_default();
        }

        fn at(&self, label: &str) -> Pos2 {
            self.buttons
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, r)| r.center())
                .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
        }

        fn labels(&self) -> Vec<&str> {
            self.buttons.iter().map(|(l, _)| l.as_str()).collect()
        }

        fn press(&mut self, label: &str, button: PointerButton) -> Pos2 {
            let pos = self.at(label);
            self.frame(vec![Event::PointerMoved(pos)]);
            self.frame(vec![mouse(pos, button, true)]);
            pos
        }

        fn click(&mut self, label: &str, button: PointerButton) {
            let pos = self.press(label, button);
            self.frame(vec![mouse(pos, button, false)]);
            self.frame(vec![]);
        }

        fn texture(&self, piece: &str) -> egui::TextureId {
            self.skin
                .piece(piece, crate::skin::WidgetState::Normal)
                .unwrap()
                .texture
                .id()
        }
    }

    fn collect_textures(shape: &egui::Shape, out: &mut Vec<egui::TextureId>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_textures(s, out)),
            egui::Shape::Mesh(mesh) => out.push(mesh.texture_id),
            _ => {}
        }
    }

    /// Nombre accesible de una fila de la lista de variantes: el nombre y el atajo.
    fn item(kind: ToolKind) -> String {
        format!("{} {}", kind.label(), kind.shortcut())
    }

    #[test]
    fn the_icon_column_has_a_named_button_per_group_and_a_click_picks() {
        let mut h = ColumnHarness::new(600.0);
        let mut column = h.buttons.clone();
        column.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
        let names: Vec<&str> = column.iter().map(|(l, _)| l.as_str()).collect();
        let expected: Vec<&str> = ToolKind::GROUPS.iter().map(|g| g[0].label()).collect();
        assert_eq!(names, expected);
        assert!(
            column.iter().all(|(_, r)| r.size() == Vec2::splat(23.0)),
            "{column:?}"
        );
        h.click("Paint Bucket", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Bucket);
    }

    #[test]
    fn right_click_lists_the_five_marquee_tools_and_holding_opens_the_list_too() {
        use egui::Popup;
        let mut h = ColumnHarness::new(600.0);
        h.click("Rectangular Marquee", PointerButton::Secondary);
        assert!(Popup::is_any_open(&h.ctx));
        for &kind in ToolKind::GROUPS[0] {
            assert!(h.labels().contains(&item(kind).as_str()), "{kind:?}");
        }
        assert_eq!(h.editor.tool, ToolKind::Pencil, "right-click does not pick");
        h.frame(vec![escape()]);
        h.frame(vec![]);
        assert!(!Popup::is_any_open(&h.ctx));

        // Mantener presionado 400 ms abre la lista.
        h.press("Rectangular Marquee", PointerButton::Primary);
        h.frame_after(0.45, vec![]);
        h.frame(vec![]);
        assert!(Popup::is_any_open(&h.ctx), "held for 400 ms");
    }

    #[test]
    fn the_group_shows_and_picks_the_variant_chosen_in_the_list() {
        let mut h = ColumnHarness::new(600.0);
        h.click("Rectangular Marquee", PointerButton::Secondary);
        h.click(&item(ToolKind::Lasso), PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Lasso);
        h.click("Pencil", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        assert!(h.textures.contains(&h.texture("icon.lasso")));
        assert!(!h.textures.contains(&h.texture("icon.marquee")));
        // El botón del grupo ahora es Lasso: un clic vuelve a Lasso.
        h.click("Lasso", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Lasso);
    }
```

(Los nombres de la lista llevan el atajo porque `Button::shortcut_text` agrega su texto al
nombre accesible: "Lasso Q".)

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui panels::`
Expected: FAIL al compilar: `cannot find function tool_column`, `cannot find type Skin`.

- [ ] **Step 3: Implementar la columna**

En `crates/cs-ui/src/panels.rs`:

1. Imports: cambiar `use crate::{theme, tile_ops, tool_group, widgets};` por:

```rust
use crate::skin::Skin;
use crate::{pixel_ui, theme, tile_ops, tool_group, widgets};
```

2. Después de `fn tool_group(...)`:

```rust
/// La barra de íconos de Pixel UI: una columna con un botón por grupo, alineada a la izquierda.
/// Los marcos van pegados, uno debajo del otro.
fn tool_column(ui: &mut egui::Ui, editor: &mut Editor, skin: &Skin) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for index in 0..ToolKind::GROUPS.len() {
            tool_group::group_button(ui, editor, Some(skin), index, |ui, shown, state| {
                pixel_ui::tool_button(
                    ui,
                    skin,
                    tool_group::icon_piece(shown),
                    shown.label(),
                    state.selected,
                    state.more,
                )
            });
        }
    });
}
```

3. Reemplazar el principio de `toolbar`, hasta el bucle de grupos inclusive, por:

```rust
/// Herramientas, Options y Symmetry. Con `skin` (Pixel UI) las herramientas son una columna de
/// íconos; sin él, filas con nombre.
pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>) {
    ui.add_space(6.0);
    ui.heading("Tools");
    ui.separator();
    ui.spacing_mut().item_spacing.y = 3.0;
    match skin {
        Some(skin) => tool_column(ui, editor, skin),
        None => {
            for index in 0..ToolKind::GROUPS.len() {
                tool_group(ui, editor, index);
            }
        }
    }
```

El resto de `toolbar` (Options, Symmetry y `text_window`) queda igual.

En `crates/cs-ui/src/pixel_ui.rs`, borrar las dos líneas de arriba de `pub fn tool_button`:

```rust
// La toolbar de íconos lo usa desde la Task 5.
#[cfg_attr(not(test), allow(dead_code))]
```

En `crates/cs-ui/src/lib.rs`, dentro del panel `"tools"`, reemplazar:

```rust
                egui::ScrollArea::vertical()
                    .id_salt("tools_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!modal, |ui| panels::toolbar(ui, &mut self.doc.editor));
                    });
```

por:

```rust
                let skin = self.pixel.skin();
                egui::ScrollArea::vertical()
                    .id_salt("tools_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!modal, |ui| {
                            panels::toolbar(ui, &mut self.doc.editor, skin)
                        });
                    });
```

`self.pixel.skin()` ya devuelve `None` con Pixel UI apagado o mientras la escala entera no
rige, así que la barra con nombres sigue igual.

- [ ] **Step 4: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS (los 3 tests nuevos de `panels` y todos los de antes).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src/panels.rs crates/cs-ui/src/pixel_ui.rs crates/cs-ui/src/lib.rs
git commit -m "Pixel UI: toolbar de solo íconos con la variante recordada de cada grupo

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: El scroll del panel de herramientas no se arrastra

**Files:**
- Modify: `crates/cs-ui/src/panels.rs`
- Modify: `crates/cs-ui/src/lib.rs`

**Interfaces:**
- Produces: `pub fn tools_scroll_area() -> egui::ScrollArea` en `panels.rs`: id `tools_scroll`,
  sin encoger y con scroll solo por la barra y la rueda.

- [ ] **Step 1: Poner el harness dentro del scroll y escribir los tests**

En `ColumnHarness` (`panels.rs`, `mod tests`):

1. Agregar el campo, después de `textures`:

```rust
        offset: f32,
```

2. En `new`, después de `textures: Vec::new(),`:

```rust
                offset: 0.0,
```

3. En `frame_after`, reemplazar:

```rust
            let mut output = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| tool_column(ui, editor, skin));
            });
            output.textures_delta.clear();
```

por:

```rust
            let mut offset = 0.0;
            let mut output = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    offset = tools_scroll_area()
                        .show(ui, |ui| tool_column(ui, editor, skin))
                        .state
                        .offset
                        .y;
                });
            });
            output.textures_delta.clear();
            self.offset = offset;
```

4. Cambiar el comentario del struct a: "La columna de íconos con el skin embebido dentro del
   scroll del panel, manejada con eventos crudos (el reloj avanza 50 ms por frame). Guarda los
   botones del árbol de accesibilidad (nombre y rect), las texturas dibujadas y el
   desplazamiento del scroll."

Y al final de `mod tests`:

```rust
    #[test]
    fn dragging_over_a_tool_button_does_not_scroll_the_panel() {
        let mut h = ColumnHarness::new(150.0);
        // Con una pantalla táctil, egui arrastra el contenido de un ScrollArea por defecto.
        let touch = |phase| Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos: pos2(199.0, 1.0),
            force: None,
        };
        h.frame(vec![
            touch(egui::TouchPhase::Start),
            touch(egui::TouchPhase::End),
        ]);
        assert!(h.ctx.input(|i| i.has_touch_screen()));
        let from = h.press("Pencil", PointerButton::Primary);
        for dy in [10.0, 25.0, 40.0] {
            h.frame(vec![Event::PointerMoved(from - Vec2::new(0.0, dy))]);
        }
        h.frame(vec![mouse(
            from - Vec2::new(0.0, 40.0),
            PointerButton::Primary,
            false,
        )]);
        h.frame(vec![]);
        assert_eq!(h.offset, 0.0, "the panel did not scroll");
    }

    #[test]
    fn holding_a_tool_button_in_the_scrolling_panel_opens_its_variants() {
        let mut h = ColumnHarness::new(150.0);
        h.press("Pencil", PointerButton::Primary);
        h.frame_after(0.45, vec![]);
        h.frame(vec![]);
        assert!(egui::Popup::is_any_open(&h.ctx));
        assert!(h.labels().contains(&item(ToolKind::Spray).as_str()));
        assert_eq!(h.offset, 0.0);
    }
```

- [ ] **Step 2: Agregar `tools_scroll_area` con el comportamiento de hoy y ver el test fallar**

En `panels.rs`, después de `fn tool_column(...)`:

```rust
/// Scroll del panel de herramientas: solo con la rueda y la barra. Arrastrar (también con el
/// dedo) no lo mueve, así mantener presionado un botón de grupo abre sus variantes.
pub fn tools_scroll_area() -> egui::ScrollArea {
    use egui::containers::scroll_area::ScrollSource;
    egui::ScrollArea::vertical()
        .id_salt("tools_scroll")
        .auto_shrink([false, false])
        .scroll_source(ScrollSource::default())
}
```

Run: `cargo test -p cs-ui scroll`
Expected: FAIL en `dragging_over_a_tool_button_does_not_scroll_the_panel` con `left: 47.5,
right: 0.0`, que es el comportamiento de hoy. `holding_a_tool_button_in_the_scrolling_panel_opens_its_variants`
pasa.

- [ ] **Step 3: Implementar**

En `tools_scroll_area`, cambiar `.scroll_source(ScrollSource::default())` por:

```rust
        .scroll_source(ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL)
```

En `crates/cs-ui/src/lib.rs`, en el panel `"tools"`, reemplazar:

```rust
                let skin = self.pixel.skin();
                egui::ScrollArea::vertical()
                    .id_salt("tools_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!modal, |ui| {
                            panels::toolbar(ui, &mut self.doc.editor, skin)
                        });
                    });
```

por:

```rust
                let skin = self.pixel.skin();
                panels::tools_scroll_area().show(ui, |ui| {
                    ui.add_enabled_ui(!modal, |ui| {
                        panels::toolbar(ui, &mut self.doc.editor, skin)
                    });
                });
```

El comentario de arriba ("Con la UI a 2× o 3× … scroll vertical.") queda.

- [ ] **Step 4: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src/panels.rs crates/cs-ui/src/lib.rs
git commit -m "Panel de herramientas: el scroll solo va con la rueda y la barra, no se arrastra

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Entrar al modo máscara o al modo Tiles a mitad de un trazo lo corta

**Files:**
- Modify: `crates/cs-ui/src/editor.rs`
- Modify: `crates/cs-ui/src/mask_ops.rs`
- Modify: `crates/cs-ui/src/anim_ops.rs`
- Modify: `crates/cs-ui/src/timeline.rs`
- Test: `crates/cs-ui/src/tool_tests.rs`
- Test: `crates/cs-ui/src/canvas.rs`

**Interfaces:**
- Produces:
  - `Editor::set_edit_mask(&mut self, on: bool)` (`pub`).
  - `pub(crate) fn paint_target(&self) -> (bool, bool)`.
  - `pub(crate) fn cancel_if_target_changed(&mut self, before: (bool, bool))`.
  - `fn abort_stroke(&mut self)` (privado) y `SpriteTools::reset(kind)` (privado).

- [ ] **Step 1: Escribir los tests y renombrar el test viejo**

En `crates/cs-ui/src/tool_tests.rs`, en `mod on_a_mask`, renombrar:

```rust
    fn tools_blocked_in_tiles_mode_hide_their_outline() {
```

a:

```rust
    fn spray_blur_and_jumble_are_blocked_in_tiles_mode_and_the_pencil_is_not() {
```

Y al final de `mod on_a_mask`:

```rust
    /// Editor con un punto rojo, Blur de 3 px y una máscara que muestra todo, sin editarla.
    fn blur_beside_a_mask() -> Editor {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.primary = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        stroke(&mut e, &[(4, 4)], false);
        e.add_mask(LayerId(1), NewMask::RevealAll);
        e.set_edit_mask(false);
        assert!(!e.editing_mask());
        e.set_tool(ToolKind::Blur);
        e.options.brush_size = 3;
        e
    }

    #[test]
    fn entering_the_mask_mid_stroke_cancels_the_blur_stroke() {
        let mut e = blur_beside_a_mask();
        let version = e.doc_version;
        e.pointer_press(at((3, 4)), false, false, 0.1);
        assert!(e.is_painting());
        e.set_edit_mask(true);
        assert!(!e.is_painting(), "the stroke is cancelled");
        let before = mask(&e);
        e.pointer_drag(at((5, 4)));
        e.pointer_release();
        assert_eq!(mask(&e), before, "nothing reached the mask");
        assert_eq!(e.doc_version, version, "nothing was applied");
    }

    #[test]
    fn a_stroke_left_blocked_by_the_mask_is_cancelled_on_the_next_drag() {
        let mut e = blur_beside_a_mask();
        e.pointer_press(at((3, 4)), false, false, 0.1);
        // Sin pasar por `set_edit_mask`.
        e.edit_mask = true;
        e.pointer_drag(at((5, 4)));
        assert!(!e.is_painting());
    }

    #[test]
    fn switching_to_a_tilemap_layer_mid_stroke_cancels_the_stroke() {
        let mut e = crate::tile_ops::test_util::tile_editor();
        e.select_layer(LayerId(1));
        assert!(!e.tiles_mode());
        e.set_tool(ToolKind::Blur);
        e.pointer_press(at((3, 4)), false, false, 0.1);
        assert!(e.is_painting());
        e.select_layer(LayerId(2));
        assert!(e.tiles_mode());
        assert!(!e.is_painting(), "the stroke is cancelled");
    }
```

En `crates/cs-ui/src/canvas.rs`, al final de `mod tests`:

```rust
    #[test]
    fn entering_the_mask_mid_blur_stroke_leaves_no_stroke_hanging() {
        use crate::mask_ops::NewMask;
        let mut h = Harness::new();
        h.editor.add_mask(LayerId(1), NewMask::RevealAll);
        h.editor.set_edit_mask(false);
        h.editor.set_tool(ToolKind::Blur);
        let (from, to) = (h.screen((4, 4)), h.screen((8, 4)));
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        h.frame(vec![Event::PointerMoved(from)]);
        h.frame(vec![button(from, true)]);
        assert!(h.editor.is_painting());
        h.editor.set_edit_mask(true);
        h.frame(vec![Event::PointerMoved(to)]);
        h.frame(vec![button(to, false)]);
        h.frame(vec![]);
        assert!(!h.editor.is_painting(), "no stroke left hanging");
    }
```

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui mask`
Expected: FAIL al compilar: `no method named set_edit_mask`.

- [ ] **Step 3: Implementar en el editor**

En `crates/cs-ui/src/editor.rs`:

1. En `impl SpriteTools`, antes de `fn get_mut`:

```rust
    /// Deja la herramienta `kind` como recién creada: descarta un trazo a medias aunque la
    /// herramienta no sepa cancelarse (`Tool::cancel` no hace nada en las de pincel).
    fn reset(&mut self, kind: ToolKind) {
        match kind {
            ToolKind::RectSelect => self.rect_select = Marquee::new(MarqueeShape::Rectangle),
            ToolKind::EllipseSelect => self.ellipse_select = Marquee::new(MarqueeShape::Ellipse),
            ToolKind::Lasso => self.lasso = Lasso::new(),
            ToolKind::PolygonLasso => self.polygon_lasso = PolygonLasso::new(),
            ToolKind::MagicWand => self.magic_wand = MagicWand::new(),
            ToolKind::Pencil => self.pencil = Pencil::new(),
            ToolKind::Spray => self.spray = Spray::new(),
            ToolKind::Eraser => self.eraser = Eraser::new(),
            ToolKind::Bucket => self.bucket = Bucket::new(),
            ToolKind::Gradient => self.gradient = GradientTool::new(),
            ToolKind::Line => self.line = ShapeTool::new(ShapeKind::Line),
            ToolKind::Rectangle => self.rectangle = ShapeTool::new(ShapeKind::Rectangle),
            ToolKind::Ellipse => self.ellipse = ShapeTool::new(ShapeKind::Ellipse),
            ToolKind::Jumble => self.jumble = Jumble::new(),
            ToolKind::Slice => self.slice = SliceTool::new(),
            ToolKind::Move => self.mover = MoveTool::new(),
            ToolKind::Eyedropper => self.eyedropper = Eyedropper::new(),
            ToolKind::Contour => self.contour = Contour::new(),
            ToolKind::Blur => self.blur = Blur::new(),
            ToolKind::Text | ToolKind::Artboard | ToolKind::Hand => {}
        }
    }
```

2. Después de `pub fn cancel_gesture(&mut self) { … }`:

```rust
    /// A qué pinta la herramienta ahora: `(editando la máscara, modo Tiles)`.
    pub(crate) fn paint_target(&self) -> (bool, bool) {
        (self.editing_mask(), self.tiles_mode())
    }

    /// Si a mitad de un trazo cambió a qué pinta la herramienta (`before` es el
    /// [`paint_target`](Self::paint_target) de antes del cambio), lo corta sin aplicarlo.
    pub(crate) fn cancel_if_target_changed(&mut self, before: (bool, bool)) {
        if self.is_painting() && self.paint_target() != before {
            self.abort_stroke();
        }
    }

    /// Corta el trazo en curso sin aplicarlo (`cancel_gesture` y la herramienta como recién
    /// creada). Si no, el trazo quedaría colgado: al soltar, una herramienta bloqueada no se
    /// suelta.
    fn abort_stroke(&mut self) {
        self.cancel_gesture();
        self.tools.reset(self.active_tool());
        self.temp_eyedropper = false;
    }
```

3. En `pointer_drag`, reemplazar las dos últimas líneas:

```rust
        let p = self.tool_point(world);
        self.with_tool(|tool, ctx| tool.drag(ctx, p));
```

por:

```rust
        // Si la herramienta quedó bloqueada a mitad del trazo (entró al modo máscara o al modo
        // Tiles sin pasar por `set_edit_mask`/`select_layer`), el trazo se corta.
        if self.tool_blocked() {
            if self.is_painting() {
                self.abort_stroke();
            }
            return;
        }
        let p = self.tool_point(world);
        self.with_tool(|tool, ctx| tool.drag(ctx, p));
```

4. En `pointer_release`, reemplazar:

```rust
        if self.tool_blocked() {
            self.temp_eyedropper = false;
            return;
        }
```

por:

```rust
        if self.tool_blocked() {
            if self.is_painting() {
                self.abort_stroke();
            }
            self.temp_eyedropper = false;
            return;
        }
```

- [ ] **Step 4: Usarlo donde cambia el modo**

En `crates/cs-ui/src/mask_ops.rs`, antes de `/// La herramienta activa no actúa ahora: …`:

```rust
    /// Entra o sale de la edición de la máscara de la capa actual. Si eso cambia a qué pinta la
    /// herramienta a mitad de un trazo, el trazo se corta.
    pub fn set_edit_mask(&mut self, on: bool) {
        let before = self.paint_target();
        self.edit_mask = on;
        self.cancel_if_target_changed(before);
    }
```

y en `add_mask`, reemplazar:

```rust
            self.layer = id;
            self.edit_mask = true;
```

por:

```rust
            let before = self.paint_target();
            self.layer = id;
            self.edit_mask = true;
            self.cancel_if_target_changed(before);
```

En `crates/cs-ui/src/anim_ops.rs`, `select_layer` queda así:

```rust
    /// Selecciona la capa para pintar sus píxeles (no su máscara).
    pub fn select_layer(&mut self, id: LayerId) {
        if self.sprite().layer(id).is_some() {
            let before = self.paint_target();
            if id != self.layer {
                self.commit_floating();
                self.end_group();
                // El aviso de límite de tamaño era de la capa anterior.
                self.tile_limit = false;
            }
            self.layer = id;
            self.edit_mask = false;
            // Pasar a una capa tilemap (o salir de la máscara) a mitad de un trazo lo corta.
            self.cancel_if_target_changed(before);
        }
    }
```

En `crates/cs-ui/src/timeline.rs`, en los dos lugares, después de `editor.select_layer(id);`:
- `editor.edit_mask = !editing_now;` pasa a `editor.set_edit_mask(!editing_now);` (el chip de la
  máscara en `layer_row`);
- `editor.edit_mask = !editing;` pasa a `editor.set_edit_mask(!editing);` (el menú
  Edit Mask / Edit Layer Pixels en `mask_menu`).

- [ ] **Step 5: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS. Pasan los 4 tests nuevos y el renombrado, y siguen en verde
`blur_and_jumble_do_nothing_and_say_so`, `the_pencil_still_paints_the_mask` y los de modo
Tiles.

- [ ] **Step 6: Commit**

```bash
git add crates/cs-ui/src/editor.rs crates/cs-ui/src/mask_ops.rs crates/cs-ui/src/anim_ops.rs crates/cs-ui/src/timeline.rs crates/cs-ui/src/tool_tests.rs crates/cs-ui/src/canvas.rs
git commit -m "Editor: entrar al modo máscara o al modo Tiles a mitad de un trazo lo corta

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Verificación final y prueba manual

**Files:** ninguno. Es solo verificación: si algo falla, se arregla en el archivo de la tarea
correspondiente y se commitea aparte.

- [ ] **Step 1: Verificación automática**

Run (con el toolchain exportado):
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo check -p cs-ui --target wasm32-unknown-unknown`

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
  - La barra de herramientas es la de siempre: emoji, nombre y `⏷`.
  - El tooltip de los grupos con variantes tiene una segunda línea, "Right-click or hold for
    more".
- **View → Pixel UI (2×):**
  - "Tools" es una columna de 13 cuadritos con íconos a la izquierda del panel.
  - El del lápiz está café: es la herramienta activa.
  - Marquee, Pencil, Bucket, Rectangle y Move llevan el triangulito en la esquina inferior
    derecha.
  - Comparar con `.design/Design.aseprite`, frame 2.
- **Tooltip:** "Pencil (B)" y, debajo, "Right-click or hold for more".
- **Variantes:**
  - Clic derecho en Marquee: la lista tiene 5 filas, con el ícono, el nombre y el atajo de cada
    una.
  - Mantener presionado 400 ms también la abre. Soltar no elige.
  - Elegir Lasso: el cuadrito de Marquee pasa a mostrar el lazo.
  - Elegir Pencil y volver a hacer clic en ese cuadrito: vuelve a Lasso.
- **Presionar** un cuadrito: el ícono baja 1 px.
- **Teclado:** Shift+G elige Gradient y el cuadrito del balde muestra Gradient.
- **UI Scale 3× con la ventana baja:**
  - La rueda mueve el panel.
  - Mantener presionado un botón y moverse no lo arrastra.
- **Recordar:** cerrar la app y volver a abrirla. El grupo Marquee sigue mostrando Lasso.

Al terminar, cerrar la app que se abrió en este paso y restaurar:

```bash
cp /tmp/chimpsprite-app.ron.bak "$APPDATA/ChimpSprite/data/app.ron"
```

- [ ] **Step 4: Sin push ni PR**

El usuario junta las ramas al final.

---

## Autorrevisión

**Cobertura del spec:**
- **§1 Skin:**
  - Los íconos y marcos se copian con un script Lua en modo batch, y `.design/` solo se lee:
    Task 1, Steps 4–8, con sha256 antes y después.
  - `icon.<id>` con los 22 nombres del spec, sobre fondo transparente y sin el marco: Task 1.
  - Los `icon.*` existentes se reutilizan y se crean los que faltan: Task 1 (`setSlice`).
  - Los íconos de más de 16 px no se recortan, se avisan: van en celdas de 17×17 (ver
    Desviaciones), con `lost 0` en la verificación.
  - `tool.slot` con 9-slice, `tool.slot.active` sin el lápiz y `tool.slot.hover` sin dibujar:
    Task 1.
  - Respaldo del ícono con la inicial y del marco plano: Task 4, en `tool_button`; el test con
    `Skin::fallback()` lo cubre.
- **§2 Botón de grupo:**
  - `group_button` en `tool_group.rs`, con clic, clic derecho, mantener 400 ms y soltar sin
    clic: Task 3.
  - La variante recordada en el `Editor` y guardada con `ToolOptions`: Task 2.
  - Lista con íconos con Pixel UI y como hoy sin él: Task 3 (`variants_list`), probada en la
    Task 5.
  - Tooltip "Nombre (Atajo)" más la segunda línea: Task 3.
- **§3 Toolbar de íconos:**
  - Columna de 13 con `tool.slot` / `tool.slot.active` a tamaño de pieza y el ícono centrado:
    Tasks 4 y 5.
  - El ícono baja 1 px al presionar: Task 4 (`label_pos`).
  - Marca de 3×3 en `text_dim`, con figuras: Task 4.
  - Options y Symmetry siguen igual, el panel queda en 184 y la columna a la izquierda: Task 5.
- **§4 Arreglos:**
  - Scroll sin arrastre: Task 6.
  - `cancel_gesture` al entrar a máscara o Tiles: Task 7 (`abort_stroke` lo llama).
  - Renombre del test: Task 7.
- **§5 Tests:**
  - Skin (22 íconos no transparentes, marcos, píxel a píxel con el script): Task 1.
  - Variante recordada (Lasso vuelve, sobrevive a guardar y leer, ajuste viejo): Tasks 2 y 5.
  - Toolbar de íconos (13 nombres, clic, 5 variantes, mantener, `icon.lasso` dibujado):
    Task 5.
  - Scroll (arrastrar no mueve, mantener abre): Task 6.
  - Cancelar (`is_painting()` en falso con un Blur a medias): Task 7, con el editor y con el
    harness del lienzo.

**Placeholders:** no quedan "TBD", "TODO" ni "similar a…". Cada paso de código trae el código
completo y los reemplazos dicen qué líneas cambian.

**Consistencia de tipos:**
- **`GroupChoice::{get, remember}`, `ToolOptions::group_choice`, `ToolKind::group()` y
  `Editor::group_tool(usize)`:** Tasks 2, 3 y 5.
- **`group_button`:** `group_button(ui, editor, skin: Option<&Skin>, group: usize, draw)`, con
  `draw: FnOnce(&mut Ui, ToolKind, ButtonState) -> Response`. Tasks 3 y 5.
- **`ButtonState { selected, more }`:** Tasks 3 y 5.
- **`variants_list(ui, Option<&Skin>, ToolKind, &[ToolKind]) -> Option<ToolKind>` e
  `icon_piece(ToolKind) -> &'static str`:** Tasks 3 y 5.
- **`tool_button(ui, &Skin, icon: &str, label: &str, selected: bool, more: bool) -> Response`
  y `more_mark(Pos2, f32) -> [Rect; 3]`:** Tasks 4 y 5.
- **`toolbar(ui, editor, Option<&Skin>)`, `tool_column(ui, editor, &Skin)` y
  `tools_scroll_area() -> egui::ScrollArea`:** Tasks 5 y 6.
- **`set_edit_mask(bool)`, `paint_target() -> (bool, bool)`, `cancel_if_target_changed((bool,
  bool))` y `abort_stroke()`:** Task 7.
