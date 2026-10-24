# Toolbar a todo el alto y barra de opciones: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** El acomodo de Aseprite. Las opciones de la herramienta activa y Symmetry pasan a una barra
horizontal debajo de las pestañas. La toolbar va a todo el alto, desde esa barra hasta la de
estado, y la línea de tiempo empieza a su derecha. Con Pixel UI, la toolbar mide el marco
`tool.slot` más 4 puntos de margen a cada lado.

**Architecture:**
- **`cs-ui::panels`:**
  - `tool_options_bar(ui, editor)` arma la fila: las opciones en un `ScrollArea` horizontal y
    Symmetry pegado a la derecha.
  - `tool_options` pasa a ser una fila con separadores entre grupos.
  - `toolbar` queda solo con los botones de grupo.
  - `OPTIONS_BAR_HEIGHT` es el alto fijo del panel.
- **`cs-ui::pixel_ui`:** `toolbar_width(skin)` es el ancho de la columna de íconos, que sale de la
  pieza `tool.slot`. `TOOLBAR_MARGIN` es el margen.
- **`cs-ui` (`lib.rs`):**
  - `ChimpApp::layout(ui, modal)` declara todos los paneles en el orden nuevo. `ui()` lo llama.
  - Devuelve el rect del lienzo, para los tests.
- **`cs-ui::layout_tests`** (nuevo, solo tests): `LayoutHarness` corre la app entera con
  `egui::Context::run_ui` y lee el rect de cada panel con `PanelState::load`.

**Tech Stack:** Rust 2024, egui/eframe 0.36 (`egui::Panel`, `ScrollArea`, accesskit en los tests).

**Spec:** `docs/superpowers/specs/2026-10-23-context-bar-design.md`.

## Global Constraints

- **Crates del motor:** `cs-core`, `cs-raster`, `cs-commands`, `cs-tools` y `cs-io` no dependen de
  egui. Este plan no los toca: todo es `cs-ui`.
- **Idiomas:** código y comentarios en español; textos de la UI en inglés. Imita la densidad de
  comentarios del código vecino.
- **Glifos:** cada glifo de la UI tiene que existir en las fuentes por defecto de egui o en
  `chimpsprite.ttf`.
  - Los textos nuevos (`Symmetry` como etiqueta y el tooltip
    `Ctrl+drag the dashed axes to move them`) son ASCII.
  - Los demás textos ya están hoy en la UI (`…`, `°`, `↔`, `╲`, `╱`).
- **Toolchain aislado:** antes de cualquier comando cargo, en Git Bash:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH`
- **Verificación por tarea:** `cargo fmt --all` y los tests del crate (`cargo test -p cs-ui`).
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
- **Commits:** terminan con la línea `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Sin push ni PR.**
- **Rama:** `context-bar`. No cambiar de rama.

## Desviaciones/decisiones (tomadas al planear)

### Cómo reparte egui el espacio (0.36)

- `egui::Panel::{top, bottom, left, right}(id)` reemplaza a `TopBottomPanel` y `SidePanel`.
  - Se muestra con `.show(ui, …)` sobre el `Ui` de la ventana, no sobre el `Context`.
  - `top` y `bottom` no se pueden redimensionar por defecto.
  - `exact_size`, `default_size` y el rect que guarda son **exteriores**: incluyen el margen del
    `Frame` y la línea que separa el panel del resto.
    - egui reserva esa línea en el margen exterior del lado de adentro: 1 punto, el ancho de
      `widgets.noninteractive.bg_stroke`.
- **Orden:** cada panel toma una franja del rect que queda libre, en el orden en que se declara
  (`available_rect_before_wrap`). El primero se queda con el borde entero.
- **Rect guardado:** cada panel guarda su rect exterior en `PanelState`, con el mismo `id`
  (`egui::Id::new("tools")`). Los tests lo leen con `PanelState::load(ctx, id)`. Es el rect del
  frame anterior, y de sobra el actual después de `run_ui`.

### Orden de hoy y orden nuevo

- **Hoy:**
  - En `ui()` se declaran `menu` (top), `tabs` (top) y `status` (bottom).
  - Después, `editor_panels` declara `timeline` (bottom), `tools` (left) y `palette` (right), y
    el lienzo (`CentralPanel`).
  - Como la línea de tiempo va antes que los laterales, ocupa todo el ancho. La toolbar y el
    panel derecho terminan encima de ella.
- **Nuevo:**
  - `menu`, `tabs` y `status` quedan igual. La barra de estado ya se declara antes que los
    laterales, así que sigue a todo el ancho.
  - Después vienen `tool_options` (top, nuevo), `tools` (left), `timeline` (bottom), `palette`
    (right) y el lienzo.
- **Panel derecho:** la línea de tiempo sigue declarándose antes que él. Por eso sigue pasando por
  debajo, como hoy, y el panel derecho no cambia de forma. Es lo que pide el spec ("la altura que
  tenga hoy respecto a la línea de tiempo").
- **La barra de opciones va dentro de `editor_panels`**, así en Home no existe. Igual se declara
  antes que los laterales: `editor_panels` corre después de `status`, y un panel `top` toma la
  franja de arriba de lo que queda, justo debajo de las pestañas.

### `ChimpApp::layout`

- `eframe::App::ui` pide un `&mut eframe::Frame`, y los tests no tienen cómo armarlo. Los paneles
  se mueven a `fn layout(&mut self, ui, modal) -> Option<egui::Rect>`, sin otro cambio.
- `layout` devuelve el rect del lienzo (`None` en Home). Solo lo usan los tests: el lienzo es un
  `CentralPanel`, que no guarda estado. `ui()` lo descarta.
- **`LayoutHarness`** va en un archivo nuevo, `layout_tests.rs`, como `tool_tests.rs`.
  - Arma la app con `ChimpApp::with_parts` (lo mismo que `DialogHarness` en `file_actions.rs`).
  - Corre `app.pixel.update` y `app.layout` en `run_ui`, a 1200×800.
  - `NoPresets`, el `PresetStore` vacío de los tests, se mueve de `file_actions.rs` a
    `palette_library::test_util` para usarlo en los dos lugares.

### Barra de opciones

- **Alto fijo:** `OPTIONS_BAR_HEIGHT = 32.0`, exterior. Incluye el margen de 2 arriba y abajo del
  frame y la línea de abajo.
  - Con la fuente pixel de 16 también entra en una fila.
  - El test pasa por las 22 herramientas y pide el mismo alto y todos los widgets dentro.
- **Symmetry está fijo a la derecha, fuera del scroll.** Las opciones hacen scroll en lo que
  queda. Así Symmetry no se va de la vista en una ventana angosta.
  - El spec pide "al final de la misma fila, alineado a la derecha", y lo cumple.
  - La fila se arma de derecha a izquierda (`Layout::right_to_left`):
    1. Symmetry;
    2. un separador;
    3. el `ScrollArea` con las opciones, de izquierda a derecha adentro.
  - Por eso `symmetry_buttons` agrega los siete modos en orden inverso: en pantalla se leen en el
    orden de siempre.
  - Lleva la etiqueta `Symmetry` a la izquierda, que reemplaza al título del panel.
  - El botón `Center axes` (cuando los ejes se movieron) queda a la derecha de los modos.
- **La ayuda de los ejes ("Ctrl+drag the dashed axes to move them")** pasa al tooltip de los
  botones de simetría cuando hay una activa. En la fila ocuparía unos 230 puntos fijos a la
  derecha.
- **Nombre accesible de los botones de simetría:** `widget_info` con el texto de su tooltip
  (`Off`, `Horizontal (left ↔ right)`, …). Hoy no tienen, y los tests los buscan así.
- **Scroll:** `ScrollArea::horizontal()` con `ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL`,
  igual que `tools_scroll_area`. Se arrastra solo con la barra.
  - Dentro de la barra, `style.always_scroll_the_only_direction = true` hace que la rueda de
    siempre (vertical) mueva la fila de lado.
  - Sin esto, egui solo la mueve con Shift+rueda. Se probó: sin la línea, el test de la rueda
    falla (el rect de `Round` no se mueve).
- **No se parte:** `style.wrap_mode = Some(TextWrapMode::Extend)` en la barra. Un texto largo
  (Slice, Polygon Lasso) alarga la fila en vez de partirse.
- **Contenido:** `tool_options` muestra lo mismo que hoy y en el mismo orden, pero en fila.
  - Los `ui.horizontal` que juntaban etiqueta y control ya no hacen falta: Magic Wand (Sample),
    Gradient (Linear/Radial) y Rotation.
  - El slider de Brush size va antes de Round/Square.
  - `Groups::next` pone un separador antes de cada grupo menos el primero. Cada ayuda (`ui.weak`)
    es su propio grupo.
- **Hand:** el spec dice "deja la barra vacía, pero visible". Hand no tiene opciones, pero hoy
  `tool_options` le muestra su ayuda ("Drag to move the view…"), y el spec pide conservar las
  ayudas. Se conserva: la barra muestra esa ayuda y Symmetry. El test pide que no estén Brush
  size ni Round/Square.
- **Modo Tiles:** hoy `tool_options` no distingue el modo Tiles. Muestra lo de la herramienta
  activa, igual que fuera de él. La barra hace lo mismo, sin código aparte.
- **`text_window`** (la ventana flotante del texto) se llamaba al final de `toolbar`. Pasa al
  final de `tool_options_bar`.
- **Con un diálogo abierto**, la barra va dentro de `add_enabled_ui(!modal)`, como la toolbar.

### Toolbar

- **Sin Pixel UI:** la barra con nombres sigue a 184, con su título `Tools` y sin Options ni
  Symmetry. El spec pide "solo los botones de grupo, nada debajo".
- **Con Pixel UI:** la columna va sin título, porque `Tools` no entra en 31 puntos.
- **Ancho con Pixel UI:**
  - `pixel_ui::toolbar_width(skin)` = ancho de `tool.slot` (23 en el skin de hoy; 23 en el
    respaldo) + 2 × `TOOLBAR_MARGIN` (4) = 31. Es el ancho del frame.
  - El panel usa `exact_size(toolbar_width + línea)`. La línea, de 1 punto, es la que egui pone
    en el margen exterior.
  - El frame es `Frame::side_top_panel` con `inner_margin(Margin::symmetric(4, 2))`.
  - El test pide que el frame mida `slot + 8` y que el botón Pencil quede a 4 puntos de cada
    borde.
- **Scroll de la toolbar:** sigue `tools_scroll_area` (vertical, rueda y barra). La barra de
  scroll de egui es flotante (`floating_allocated_width = 0`), así que no le quita ancho a la
  columna.

### Riesgos medidos al planear

- **La fila de controles de la línea de tiempo** (⏮ … `+ Frame`, `+ Layer`, opacidad, blend)
  mide unos 1050 puntos y no se parte.
  - Antes tenía todo el ancho. Ahora tiene el ancho menos la toolbar: a 1200 sin Pixel UI le
    quedan 1016, y egui recorta los últimos ~34 puntos de esa fila.
  - Con Pixel UI a 2× ya pasaba antes en casi cualquier ventana.
  - El spec no lo cubre y no se cambia aquí. El test del panel derecho usa
    `timeline.right() >= screen.right()` por eso. Queda anotado para la prueba manual.
- **Disco:** al planear, D: quedó con unos 4.5 GB libres. Antes de compilar la GUI (Task 4) hay
  que mirar el espacio libre.

### Prototipo

Todo el código de este plan se probó al planear, en un worktree aparte (`--detach`, ya borrado):
- `cargo test -p cs-ui`: 300 tests en verde (1 ignorado). La cuenta, task por task, es:
  - antes de empezar: 287;
  - Task 1: 292;
  - Task 2: 297;
  - Task 3: 300.
- `cargo clippy --workspace --all-targets -- -D warnings`: limpio.
- `cargo check -p cs-ui --target wasm32-unknown-unknown`: limpio.
- `cargo test --workspace` no llegó a terminar, porque el disco se llenó. Los demás crates no
  cambian.
- Los mensajes de "Expected: FAIL" de cada task son los que dio el prototipo.

## Archivos

| Archivo | Qué hace |
| --- | --- |
| Modificar `crates/cs-ui/src/panels.rs` | `toolbar` solo con los grupos; `OPTIONS_BAR_HEIGHT`, `tool_options_bar`, `options_scroll_area`, `Groups`; `tool_options` en fila; `symmetry_buttons` de derecha a izquierda con nombres accesibles; tests con `BarHarness` |
| Modificar `crates/cs-ui/src/pixel_ui.rs` | `TOOLBAR_MARGIN`, `slot_size`, `toolbar_width`; test |
| Modificar `crates/cs-ui/src/lib.rs` | `mod layout_tests`; `layout()`; `editor_panels` con la barra de opciones, el orden nuevo, el ancho de Pixel UI y el rect del lienzo |
| Crear `crates/cs-ui/src/layout_tests.rs` | `LayoutHarness` y los tests de acomodo |
| Modificar `crates/cs-ui/src/palette_library.rs` | `test_util::NoPresets` |
| Modificar `crates/cs-ui/src/file_actions.rs` | Los tests usan `test_util::NoPresets` |

---

### Task 1: Barra de opciones debajo de las pestañas

**Files:**
- Modify: `crates/cs-ui/src/panels.rs`:
  - `toolbar` y `tool_options`, de `/// Herramientas, Options y Symmetry.` hasta antes de
    `/// Botones Round y Square`;
  - `symmetry_buttons`;
  - `mod tests`.
- Modify: `crates/cs-ui/src/lib.rs` (`editor_panels`).

**Interfaces:**
- Produces (en `crate::panels`):
  - `pub const OPTIONS_BAR_HEIGHT: f32` (32.0), el alto exterior del panel `tool_options`.
  - `pub fn tool_options_bar(ui: &mut egui::Ui, editor: &mut Editor)`.
  - `pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>)`: la misma
    firma, ahora sin Options ni Symmetry.
  - Los botones de simetría tienen de nombre accesible el texto de `SYMMETRY_MODES`, por ejemplo
    `"Off"`.
  - En el panel de la app, el id `"tool_options"`.

- [ ] **Step 1: Escribir los tests**

En `crates/cs-ui/src/panels.rs`, al final de `mod tests` (antes de su `}`), agregar:

```rust
    /// Nombre (o texto, en las etiquetas) y rect de cada nodo del árbol de accesibilidad.
    fn nodes(output: &egui::FullOutput) -> Vec<(String, Rect)> {
        let Some(update) = &output.platform_output.accesskit_update else {
            return Vec::new();
        };
        update
            .nodes
            .iter()
            .filter_map(|(_, n)| {
                let b = n.bounds()?;
                let text = n.label().or_else(|| n.value())?.to_string();
                let rect = Rect::from_min_max(
                    pos2(b.x0 as f32, b.y0 as f32),
                    pos2(b.x1 as f32, b.y1 as f32),
                );
                Some((text, rect))
            })
            .collect()
    }

    /// La barra de opciones en su panel de arriba, como en la app, manejada con eventos crudos (el
    /// reloj avanza 50 ms por frame). Guarda el rect del panel y los nodos de accesibilidad.
    struct BarHarness {
        ctx: egui::Context,
        editor: Editor,
        time: f64,
        width: f32,
        bar: Rect,
        nodes: Vec<(String, Rect)>,
    }

    impl BarHarness {
        fn new(width: f32) -> Self {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut h = Self {
                ctx,
                editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
                time: 0.0,
                width,
                bar: Rect::NOTHING,
                nodes: Vec::new(),
            };
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.time += 0.05;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(self.width, 300.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let editor = &mut self.editor;
            let mut bar = Rect::NOTHING;
            let mut output = self.ctx.run_ui(input, |ui| {
                bar = egui::Panel::top("tool_options")
                    .exact_size(OPTIONS_BAR_HEIGHT)
                    .show(ui, |ui| tool_options_bar(ui, editor))
                    .response
                    .rect;
            });
            output.textures_delta.clear();
            self.bar = bar;
            self.nodes = nodes(&output);
        }

        fn has(&self, text: &str) -> bool {
            self.nodes.iter().any(|(t, _)| t == text)
        }

        fn rect(&self, text: &str) -> Rect {
            self.nodes
                .iter()
                .find(|(t, _)| t == text)
                .map(|(_, r)| *r)
                .unwrap_or_else(|| panic!("no {text:?} in {:?}", self.nodes))
        }
    }

    #[test]
    fn the_options_bar_has_the_brush_with_the_pencil_but_not_with_the_hand() {
        let mut h = BarHarness::new(1200.0);
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        for text in ["Brush size", "Round", "Square", "Pixel-perfect"] {
            assert!(h.has(text), "{text}");
        }
        // Etiqueta y control uno al lado del otro: la forma va a la derecha del texto.
        assert!(h.rect("Round").left() > h.rect("Brush size").right());
        assert!(h.has("Off"), "Symmetry");
        h.editor.set_tool(ToolKind::Hand);
        h.frame(vec![]);
        for text in ["Brush size", "Round", "Square"] {
            assert!(!h.has(text), "{text}");
        }
        assert!(h.has("Off"), "Symmetry is always there");
        assert!(h.has("Symmetry"));
    }

    #[test]
    fn symmetry_is_at_the_right_end_of_the_bar_in_the_usual_order() {
        let h = BarHarness::new(1200.0);
        let names: Vec<&str> = SYMMETRY_MODES.iter().map(|(_, tip)| *tip).collect();
        let rects: Vec<Rect> = names.iter().map(|n| h.rect(n)).collect();
        for pair in rects.windows(2) {
            assert!(pair[0].right() <= pair[1].left(), "{names:?}: {rects:?}");
        }
        let last = rects[rects.len() - 1];
        assert!(
            h.bar.right() - last.right() < 12.0,
            "{last:?} in {:?}",
            h.bar
        );
        assert!(h.rect("Symmetry").right() <= rects[0].left());
    }

    #[test]
    fn the_options_bar_is_one_row_of_the_same_height_with_every_tool() {
        let mut h = BarHarness::new(800.0);
        for &kind in ToolKind::GROUPS.iter().flat_map(|g| g.iter()) {
            h.editor.set_tool(kind);
            h.frame(vec![]);
            assert_eq!(h.bar.height(), OPTIONS_BAR_HEIGHT, "{kind:?}");
            for (text, rect) in &h.nodes {
                assert!(
                    rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom(),
                    "{kind:?}: {text:?} {rect:?} out of {:?}",
                    h.bar
                );
            }
        }
    }

    #[test]
    fn the_options_scroll_sideways_with_the_wheel_when_they_do_not_fit() {
        let mut h = BarHarness::new(320.0);
        let before = h.rect("Round");
        let over = pos2(40.0, h.bar.center().y);
        h.frame(vec![Event::PointerMoved(over)]);
        h.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -60.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        }]);
        for _ in 0..20 {
            h.frame(vec![]);
        }
        let after = h.rect("Round");
        assert!(after.left() < before.left(), "{before:?} -> {after:?}");
        assert_eq!(after.top(), before.top(), "still one row");
        assert!(h.rect("Off").right() <= 320.0, "Symmetry stays in view");
    }

    #[test]
    fn the_toolbar_has_only_the_tool_buttons() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        for skin in [None, Some(&skin)] {
            let mut output = ctx.run_ui(RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| toolbar(ui, &mut editor, skin));
            });
            output.textures_delta.clear();
            let texts: Vec<String> = nodes(&output).into_iter().map(|(t, _)| t).collect();
            for gone in ["Options", "Symmetry", "Brush size", "Off"] {
                assert!(!texts.iter().any(|t| t == gone), "{gone} in {texts:?}");
            }
            assert!(texts.iter().any(|t| t.contains("Pencil")), "{texts:?}");
        }
    }
```

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui panels::`
Expected: FAIL al compilar, con `cannot find function tool_options_bar in this scope` y
`cannot find value OPTIONS_BAR_HEIGHT in this scope`.

- [ ] **Step 3: Implementar la barra**

En `crates/cs-ui/src/panels.rs`:

1. Reemplazar todo desde la línea `/// Herramientas, Options y Symmetry. …` (el doc de
   `toolbar`) hasta la línea anterior a
   `/// Botones Round y Square de la forma del pincel.`. Eso son `toolbar` y `tool_options`
   enteros. Va esto:

```rust
/// La barra de herramientas: solo los botones de grupo. Con `skin` (Pixel UI) es una columna de
/// íconos; sin él, filas con nombre bajo el título "Tools". Las opciones de la herramienta y
/// Symmetry van en la barra de opciones (`tool_options_bar`).
pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>) {
    match skin {
        Some(skin) => tool_column(ui, editor, skin),
        None => {
            ui.add_space(6.0);
            ui.heading("Tools");
            ui.separator();
            ui.spacing_mut().item_spacing.y = 3.0;
            for index in 0..ToolKind::GROUPS.len() {
                tool_group(ui, editor, index);
            }
        }
    }
}

/// Alto de la barra de opciones, con su margen: el mismo con todas las herramientas, así el lienzo
/// no se mueve al cambiar de herramienta.
pub const OPTIONS_BAR_HEIGHT: f32 = 32.0;

/// Barra de opciones, debajo de las pestañas: las opciones de la herramienta activa en una sola
/// fila y Symmetry a la derecha. Si las opciones no caben, hacen scroll horizontal con la rueda o
/// la barra (sin arrastre); no se parten en dos renglones.
pub fn tool_options_bar(ui: &mut egui::Ui, editor: &mut Editor) {
    // Los textos no se parten: la fila se alarga y hace scroll.
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    // La rueda de siempre (vertical) mueve la fila de lado, sin Shift.
    ui.style_mut().always_scroll_the_only_direction = true;
    // De derecha a izquierda: Symmetry queda pegado al borde y las opciones usan lo que sobra.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        symmetry_buttons(ui, editor);
        ui.separator();
        options_scroll_area().show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                tool_options(ui, editor)
            });
        });
    });
    text_window(ui.ctx(), editor);
}

/// Scroll de las opciones de la barra: horizontal, solo con la rueda y la barra.
fn options_scroll_area() -> egui::ScrollArea {
    use egui::containers::scroll_area::ScrollSource;
    egui::ScrollArea::horizontal()
        .id_salt("tool_options_scroll")
        .auto_shrink([false, false])
        .scroll_source(ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL)
}

/// Separadores entre los grupos de la barra de opciones: una línea antes de cada grupo, menos el
/// primero.
#[derive(Default)]
struct Groups {
    started: bool,
}

impl Groups {
    fn next(&mut self, ui: &mut egui::Ui) {
        if self.started {
            ui.separator();
        }
        self.started = true;
    }
}

/// Las opciones de la herramienta activa, en fila: cada etiqueta al lado de su control y un
/// separador entre grupos. La ayuda de la herramienta va en el mismo orden que antes.
fn tool_options(ui: &mut egui::Ui, editor: &mut Editor) {
    let tool = editor.tool;
    let mut groups = Groups::default();
    let options = &mut editor.options;
    if tool.uses_brush() {
        groups.next(ui);
        ui.label("Brush size");
        ui.add(egui::Slider::new(
            &mut options.brush_size,
            1..=MAX_BRUSH_SIZE,
        ))
        .on_hover_text("Ctrl+wheel over the canvas changes it");
        brush_shape_buttons(ui, &mut options.brush_shape);
    }
    if matches!(tool, ToolKind::Pencil | ToolKind::Eraser) {
        groups.next(ui);
        ui.checkbox(&mut options.pixel_perfect, "Pixel-perfect");
    }
    if matches!(tool, ToolKind::Rectangle | ToolKind::Ellipse) {
        groups.next(ui);
        ui.checkbox(&mut options.filled, "Filled");
    }
    if tool == ToolKind::Rectangle {
        groups.next(ui);
        ui.label("Corner radius");
        ui.add(egui::Slider::new(&mut options.corner_radius, 0..=32))
            .on_hover_text("0 = square corners; large values make a pill or a circle");
    }
    if tool.is_selection() {
        groups.next(ui);
        ui.label("Mode");
        for mode in SelectionMode::ALL {
            ui.radio_value(&mut options.selection_mode, mode, mode.label());
        }
    }
    match tool {
        ToolKind::Hand => {
            groups.next(ui);
            ui.weak("Drag to move the view. Space+drag or the middle button do it with any tool.");
        }
        ToolKind::MagicWand => {
            groups.next(ui);
            ui.label("Tolerance");
            ui.add(egui::Slider::new(&mut options.tolerance, 0..=255));
            ui.checkbox(&mut options.contiguous, "Contiguous");
            ui.label("Sample:");
            ui.radio_value(&mut options.sample_all, false, "Layer");
            ui.radio_value(&mut options.sample_all, true, "All layers");
        }
        ToolKind::Lasso => {
            groups.next(ui);
            ui.weak("Drag around the area; it closes when you release.");
        }
        ToolKind::PolygonLasso => {
            groups.next(ui);
            ui.weak("Click to add points. Click the first point, double-click or press Enter to close; Esc cancels.");
        }
        ToolKind::Bucket => {
            groups.next(ui);
            ui.checkbox(&mut options.contiguous, "Contiguous");
        }
        ToolKind::Spray => {
            groups.next(ui);
            ui.label("Spray width");
            ui.add(egui::Slider::new(&mut options.spray_width, 1..=64));
            ui.label("Spray speed");
            ui.add(egui::Slider::new(&mut options.spray_speed, 1..=100));
        }
        ToolKind::Gradient => {
            groups.next(ui);
            ui.radio_value(&mut options.gradient_shape, GradientShape::Linear, "Linear");
            ui.radio_value(&mut options.gradient_shape, GradientShape::Radial, "Radial");
            egui::ComboBox::from_id_salt("dither")
                .selected_text(options.dither.label())
                .show_ui(ui, |ui| {
                    for d in Dither::ALL {
                        ui.selectable_value(&mut options.dither, d, d.label());
                    }
                });
            groups.next(ui);
            ui.weak("Drag from the primary to the secondary color. Right-drag swaps them.");
        }
        ToolKind::Jumble => {
            groups.next(ui);
            ui.weak("Scrambles pixels within the brush radius.");
        }
        ToolKind::Contour => {
            groups.next(ui);
            ui.weak("Draw the outline: it closes and fills when you release.");
        }
        ToolKind::Blur => {
            groups.next(ui);
            ui.weak("Softens the pixels under the brush. Go over an area again to blur it more.");
        }
        ToolKind::Text => {
            groups.next(ui);
            text_style_options(ui, editor);
            groups.next(ui);
            ui.weak("Click on the canvas to place text.");
        }
        ToolKind::Slice => {
            groups.next(ui);
            ui.weak("Drag to create a slice (hitbox, pivot…). Drag the selected one to move it or its edges to resize it: changes apply from the current frame on.");
        }
        _ => {}
    }
    if tool.is_selection() || tool == ToolKind::Move {
        groups.next(ui);
        let before = editor.options.rotation;
        ui.label("Rotation:");
        for algorithm in RotationAlgorithm::ALL {
            ui.radio_value(&mut editor.options.rotation, algorithm, algorithm.label());
        }
        if editor.options.rotation != before {
            editor.refresh_floating(false);
        }
        groups.next(ui);
        ui.weak("Drag the handles to scale, or just outside a corner to rotate. Shift keeps the proportions or snaps to 15°.");
    }
    if matches!(
        tool,
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse | ToolKind::Gradient
    ) || matches!(tool, ToolKind::RectSelect | ToolKind::EllipseSelect)
    {
        groups.next(ui);
        ui.weak("Shift: constrain");
    }
    if tool.paints() {
        groups.next(ui);
        ui.weak("Alt+click: pick color");
    }
}
```

2. Reemplazar la función `fn symmetry_buttons(ui: &mut egui::Ui, editor: &mut Editor) { … }`
   entera (la que está justo después de `const SYMMETRY_MODES`, hasta antes de
   `fn draw_symmetry_icon`) por:

```rust
/// Ayuda de los ejes, en el tooltip de los botones de simetría cuando hay una activa.
const AXES_HINT: &str = "Ctrl+drag the dashed axes to move them";

/// Symmetry, al final de la barra de opciones. Se arma de derecha a izquierda (la barra lo pone
/// pegado al borde): "Center axes" si los ejes se movieron, los siete modos en orden inverso y el
/// título. En pantalla se leen en el orden de siempre.
fn symmetry_buttons(ui: &mut egui::Ui, editor: &mut Editor) {
    let current = editor.symmetry();
    if current.mode != SymmetryMode::None
        && current.center2.is_some()
        && ui.small_button("Center axes").clicked()
    {
        editor.set_symmetry(cs_core::Symmetry {
            center2: None,
            ..current
        });
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (mode, tip) in SYMMETRY_MODES.into_iter().rev() {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            let selected = current.mode == mode;
            let enabled = ui.is_enabled();
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    enabled,
                    selected,
                    tip,
                )
            });
            let visuals = ui.style().interact_selectable(&response, selected);
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, visuals.weak_bg_fill);
            let color = if selected {
                theme::ACCENT
            } else {
                visuals.fg_stroke.color
            };
            draw_symmetry_icon(painter, rect.shrink(5.0), mode, color);
            let response = if current.mode == SymmetryMode::None {
                response.on_hover_text(tip)
            } else {
                response.on_hover_text(format!("{tip}\n{AXES_HINT}"))
            };
            if response.clicked() {
                editor.set_symmetry(cs_core::Symmetry { mode, ..current });
            }
        }
    });
    ui.label("Symmetry");
}
```

`SYMMETRY_MODES`, `draw_symmetry_icon`, `brush_shape_buttons`, `text_style_options` y
`text_window` quedan como están.

En `crates/cs-ui/src/lib.rs`, en `fn editor_panels`, al principio, antes de
`if self.view.timeline {`:

```rust
        // La barra de opciones, a todo el ancho debajo de las pestañas.
        egui::Panel::top("tool_options")
            .exact_size(panels::OPTIONS_BAR_HEIGHT)
            .show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    panels::tool_options_bar(ui, &mut self.doc.editor)
                });
            });
```

- [ ] **Step 4: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS, 292 tests (los 5 nuevos de `panels`).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src/panels.rs crates/cs-ui/src/lib.rs
git commit -m "Barra de opciones: las opciones de la herramienta y Symmetry pasan a una fila bajo las pestañas

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: La toolbar a todo el alto y la línea de tiempo a su derecha

**Files:**
- Modify: `crates/cs-ui/src/palette_library.rs` (al final) y `crates/cs-ui/src/file_actions.rs`
  (`mod tests`)
- Create: `crates/cs-ui/src/layout_tests.rs`
- Modify: `crates/cs-ui/src/lib.rs` (`mod`, `editor_panels`, `ui`)

**Interfaces:**
- Consumes: el panel `"tool_options"` y `panels::OPTIONS_BAR_HEIGHT` (Task 1).
- Produces:
  - `ChimpApp::layout(&mut self, ui: &mut egui::Ui, modal: bool) -> Option<egui::Rect>`, privado.
    Devuelve el rect del lienzo, o `None` en Home.
  - `fn editor_panels(&mut self, ui: &mut egui::Ui, modal: bool) -> egui::Rect`.
  - `crate::palette_library::test_util::NoPresets` (`pub(crate)`, solo en tests).
  - `layout_tests::LayoutHarness`, con `new(view, doc)`, `editor()`, `frame()`, `panel(id)` y
    `screen()`. La Task 3 lo extiende.

- [ ] **Step 1: Mover `NoPresets`**

Al final de `crates/cs-ui/src/palette_library.rs`:

```rust
#[cfg(test)]
pub(crate) mod test_util {
    use std::path::PathBuf;

    use cs_io::palette_files::NamedPalette;

    use super::{PresetStore, StoredPreset};

    /// Sin presets de usuario: los tests no tocan la carpeta del usuario.
    pub(crate) struct NoPresets;

    impl PresetStore for NoPresets {
        fn load(&self) -> Vec<StoredPreset> {
            Vec::new()
        }

        fn save(&mut self, _: &NamedPalette, _: &[String]) -> Result<(), String> {
            Ok(())
        }

        fn delete(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }

        fn folder(&self) -> Option<PathBuf> {
            None
        }
    }
}
```

En `crates/cs-ui/src/file_actions.rs`, en `mod tests`, reemplazar:

```rust
    use std::path::PathBuf;

    use cs_io::palette_files::NamedPalette;
    use egui::{Event, PointerButton, Pos2, RawInput, Rect, Vec2};

    use crate::palette_library::{PaletteLibrary, PresetStore, StoredPreset};
    use crate::view::ViewSettings;
```

por:

```rust
    use egui::{Event, PointerButton, Pos2, RawInput, Rect, Vec2};

    use crate::palette_library::PaletteLibrary;
    use crate::palette_library::test_util::NoPresets;
    use crate::view::ViewSettings;
```

y borrar el bloque que sigue, desde `/// Sin presets de usuario: los tests no tocan la carpeta del
usuario.` hasta el `}` del `impl PresetStore for NoPresets` inclusive.

Run: `cargo test -p cs-ui file_actions::`
Expected: PASS, sin cambios.

- [ ] **Step 2: Escribir los tests de acomodo**

Crear `crates/cs-ui/src/layout_tests.rs`:

```rust
//! Acomodo de los paneles de la ventana: la app entera, con una pestaña abierta, manejada con
//! eventos crudos. Los rects de los paneles salen del `PanelState` que egui guarda de cada uno.

use eframe::egui::{self, Pos2, RawInput, Rect, Vec2, containers::panel::PanelState};

use crate::ChimpApp;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::view::ViewSettings;

struct LayoutHarness {
    ctx: egui::Context,
    app: ChimpApp,
    time: f64,
    /// Rect del lienzo en el último frame (`None` en Home).
    canvas: Option<Rect>,
}

impl LayoutHarness {
    /// La app con `view`, una pestaña nueva si `doc`, y tres frames: con Pixel UI la escala
    /// entera rige desde el segundo.
    fn new(view: ViewSettings, doc: bool) -> Self {
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
        if doc {
            app.new_document();
        }
        let mut h = Self {
            ctx: egui::Context::default(),
            app,
            time: 0.0,
            canvas: None,
        };
        for _ in 0..3 {
            h.frame();
        }
        h
    }

    /// El editor con los paneles de siempre (línea de tiempo visible).
    fn editor() -> Self {
        Self::new(ViewSettings::default(), true)
    }

    fn frame(&mut self) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(self.time),
            ..Default::default()
        };
        let app = &mut self.app;
        let mut canvas = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            app.pixel.update(&ctx, &app.view);
            canvas = app.layout(ui, false);
        });
        // Las texturas (fuentes, skin y lienzo) no se suben a ninguna GPU.
        output.textures_delta.clear();
        self.canvas = canvas;
    }

    /// Rect exterior del panel `id` (con su margen y la línea que lo separa del resto).
    fn panel(&self, id: &str) -> Rect {
        PanelState::load(&self.ctx, egui::Id::new(id))
            .unwrap_or_else(|| panic!("no panel {id:?}"))
            .outer_rect
    }

    fn screen(&self) -> Rect {
        self.ctx.content_rect()
    }
}

#[test]
fn the_toolbar_goes_down_beside_the_timeline() {
    let h = LayoutHarness::editor();
    let (tools, timeline) = (h.panel("tools"), h.panel("timeline"));
    assert!(
        tools.bottom() > timeline.top(),
        "full height: {tools:?} vs {timeline:?}"
    );
    assert!(
        tools.right() <= timeline.left(),
        "the timeline starts to its right: {tools:?} vs {timeline:?}"
    );
    assert_eq!(
        tools.bottom(),
        timeline.bottom(),
        "both end on the status bar"
    );
}

#[test]
fn the_options_bar_is_between_the_tabs_and_the_canvas() {
    let h = LayoutHarness::editor();
    let (tabs, options) = (h.panel("tabs"), h.panel("tool_options"));
    let canvas = h.canvas.expect("a document is open");
    assert!(tabs.bottom() <= options.top(), "{tabs:?} vs {options:?}");
    assert!(
        options.bottom() <= canvas.top(),
        "{options:?} vs {canvas:?}"
    );
    assert!(
        options.bottom() <= h.panel("tools").top(),
        "the toolbar starts under it"
    );
    assert_eq!(options.width(), h.screen().width(), "full width");
}

#[test]
fn the_status_bar_spans_the_window_under_everything() {
    let h = LayoutHarness::editor();
    let status = h.panel("status");
    assert_eq!(status.x_range(), h.screen().x_range());
    assert_eq!(status.bottom(), h.screen().bottom());
    for id in ["tools", "timeline", "palette"] {
        assert!(h.panel(id).bottom() <= status.top(), "{id}");
    }
}

#[test]
fn the_right_panel_keeps_ending_above_the_timeline() {
    let h = LayoutHarness::editor();
    let (palette, timeline) = (h.panel("palette"), h.panel("timeline"));
    assert!(
        palette.bottom() <= timeline.top(),
        "{palette:?} vs {timeline:?}"
    );
    // La fila de controles de la línea de tiempo no se parte: si es más ancha que el panel, el
    // rect del panel pasa del borde (egui recorta lo que sobra).
    assert!(
        timeline.right() >= h.screen().right(),
        "under the right panel too: {timeline:?}"
    );
}

#[test]
fn on_home_there_is_no_options_bar() {
    let h = LayoutHarness::new(ViewSettings::default(), false);
    assert!(h.canvas.is_none());
    assert!(PanelState::load(&h.ctx, egui::Id::new("tool_options")).is_none());
    assert!(PanelState::load(&h.ctx, egui::Id::new("tools")).is_none());
}
```

En `crates/cs-ui/src/lib.rs`, después de `mod instance_ops;`:

```rust
#[cfg(test)]
mod layout_tests;
```

- [ ] **Step 3: Sacar los paneles de `ui()` a `layout()`, sin cambiar el orden**

En `crates/cs-ui/src/lib.rs`, en `fn ui` de `impl eframe::App for ChimpApp`, reemplazar desde
`egui::Panel::top("menu").show(ui, |ui| {` hasta el `}` del `if self.has_doc() { … } else { … }`
(justo antes de `self.update_canvas_size(&ctx);`) por:

```rust
        self.layout(ui, modal);
```

(La línea `let modal = self.is_modal();` de arriba queda.)

Reemplazar el doc y la firma de `editor_panels`:

```rust
    /// Paneles del editor (cuando hay un documento activo).
    fn editor_panels(&mut self, ui: &mut egui::Ui, modal: bool) {
```

por:

```rust
    /// Los paneles de la ventana, en el orden en que egui les reparte el espacio: el primero que
    /// se declara se queda con el borde. Devuelve el rect del lienzo (`None` en Home).
    fn layout(&mut self, ui: &mut egui::Ui, modal: bool) -> Option<egui::Rect> {
        egui::Panel::top("menu").show(ui, |ui| {
            ui.add_enabled_ui(!modal, |ui| self.menu_bar(ui));
        });
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_enabled_ui(!modal, |ui| self.tab_bar(ui));
        });
        // Antes que los laterales: la barra de estado va a todo el ancho.
        egui::Panel::bottom("status").show(ui, |ui| {
            let notice = self
                .autosaver
                .disabled_reason
                .as_ref()
                .map(|r| format!("Autosave off: {r}"));
            if self.has_doc() {
                panels::status_bar(ui, &self.doc.editor, &self.doc.canvas, notice.as_deref());
            } else {
                ui.label(format!("ChimpSprite {}", env!("CARGO_PKG_VERSION")));
            }
        });
        if self.has_doc() {
            Some(self.editor_panels(ui, modal))
        } else {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| self.home(ui));
            });
            None
        }
    }

    /// Paneles del editor (cuando hay un documento activo). Devuelve el rect del lienzo.
    fn editor_panels(&mut self, ui: &mut egui::Ui, modal: bool) -> egui::Rect {
```

Y al final de `editor_panels`, reemplazar:

```rust
        egui::CentralPanel::no_frame().show(ui, |ui| {
            self.doc.canvas.show(
                ui,
                &mut self.doc.editor,
                self.canvas_size.as_mut(),
                &self.view,
            )
        });
    }
```

por:

```rust
        egui::CentralPanel::no_frame()
            .show(ui, |ui| {
                self.doc.canvas.show(
                    ui,
                    &mut self.doc.editor,
                    self.canvas_size.as_mut(),
                    &self.view,
                )
            })
            .response
            .rect
    }
```

- [ ] **Step 4: Correr los tests y comprobar que falla el del alto**

Run: `cargo fmt --all` y `cargo test -p cs-ui layout_tests`
Expected:
- FAIL `the_toolbar_goes_down_beside_the_timeline` con
  `full height: [[0.0 78.0] - [184.0 587.0]] vs [[0.0 587.0] - [1200.0 777.0]]`. Es el orden de
  hoy: la toolbar termina donde empieza la línea de tiempo.
- Los otros 4 pasan: la barra de opciones (Task 1), la de estado y el panel derecho ya están
  bien.

- [ ] **Step 5: El orden nuevo**

En `editor_panels`, quitar el bloque del panel `"tools"`, que hoy va después de
`if std::mem::take(&mut self.doc.timeline.new_tilemap) { … }`:

```rust
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(184.0)
            .show(ui, |ui| {
                // Con la UI a 2× o 3× las herramientas y sus opciones no caben: scroll vertical.
                let skin = self.pixel.skin();
                panels::tools_scroll_area().show(ui, |ui| {
                    ui.add_enabled_ui(!modal, |ui| panels::toolbar(ui, &mut self.doc.editor, skin));
                });
            });
```

y ponerlo entre el panel `"tool_options"` (Task 1) y `if self.view.timeline {`, así:

```rust
        // La toolbar antes que la línea de tiempo: va a todo el alto, hasta la barra de estado, y
        // la línea de tiempo empieza a su derecha.
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(184.0)
            .show(ui, |ui| {
                // Con la UI a 2× o 3× y la ventana baja, la barra no cabe: scroll vertical.
                let skin = self.pixel.skin();
                panels::tools_scroll_area().show(ui, |ui| {
                    ui.add_enabled_ui(!modal, |ui| panels::toolbar(ui, &mut self.doc.editor, skin));
                });
            });
        // La línea de tiempo antes que el panel derecho: pasa por debajo de él, como siempre.
        if self.view.timeline {
```

El resto de `editor_panels` queda en su lugar, en este orden:
1. el panel `"timeline"`;
2. `new_tilemap`;
3. el panel `"palette"`;
4. el lienzo.

- [ ] **Step 6: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS, 297 tests (los 5 de `layout_tests`).

- [ ] **Step 7: Commit**

```bash
git add crates/cs-ui/src/lib.rs crates/cs-ui/src/layout_tests.rs crates/cs-ui/src/palette_library.rs crates/cs-ui/src/file_actions.rs
git commit -m "Acomodo: la toolbar va a todo el alto y la línea de tiempo empieza a su derecha

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Con Pixel UI, la toolbar mide el marco más su margen

**Files:**
- Modify: `crates/cs-ui/src/pixel_ui.rs`
- Modify: `crates/cs-ui/src/lib.rs` (`editor_panels`)
- Test: `crates/cs-ui/src/layout_tests.rs`, `crates/cs-ui/src/pixel_ui.rs`

**Interfaces:**
- Consumes: `LayoutHarness` y el panel `"tools"` (Task 2).
- Produces (en `crate::pixel_ui`):
  - `pub const TOOLBAR_MARGIN: i8` (4);
  - `fn slot_size(skin: &Skin) -> Vec2` (privado);
  - `pub fn toolbar_width(skin: &Skin) -> f32`.

- [ ] **Step 1: Escribir los tests**

En `crates/cs-ui/src/pixel_ui.rs`, en `mod tests`, antes de
`fn the_variants_mark_is_a_three_pixel_triangle_in_the_corner`:

```rust
    #[test]
    fn the_toolbar_is_as_wide_as_the_slot_plus_the_margin() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let slot = skin.piece(SLOT_PIECE, WidgetState::Normal).unwrap().size[0] as f32;
        assert_eq!(toolbar_width(&skin), slot + 8.0);
        assert_eq!(
            toolbar_width(&Skin::fallback()),
            23.0 + 8.0,
            "the fallback slot"
        );
    }
```

En `crates/cs-ui/src/layout_tests.rs`, el harness guarda los botones del árbol de accesibilidad:

1. En `struct LayoutHarness`, después del campo `canvas`:

```rust
    /// Botones del último frame: nombre y rect (en puntos).
    buttons: Vec<(String, Rect)>,
```

2. En `new`, reemplazar:

```rust
        let mut h = Self {
            ctx: egui::Context::default(),
            app,
            time: 0.0,
            canvas: None,
        };
```

por:

```rust
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut h = Self {
            ctx,
            app,
            time: 0.0,
            canvas: None,
            buttons: Vec::new(),
        };
```

3. En `frame`, después de `self.canvas = canvas;`:

```rust
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
                            Pos2::new(b.x0 as f32, b.y0 as f32),
                            Pos2::new(b.x1 as f32, b.y1 as f32),
                        );
                        Some((n.label()?.to_string(), rect))
                    })
                    .collect()
            })
            .unwrap_or_default();
```

4. Después de `fn frame`:

```rust
    fn button(&self, label: &str) -> Rect {
        self.buttons
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
    }
```

5. Al final del archivo:

```rust
#[test]
fn with_pixel_ui_the_toolbar_is_the_slot_plus_its_margin() {
    use crate::skin::WidgetState;
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let h = LayoutHarness::new(view, true);
    let skin = h.app.pixel.skin().expect("the integer scale is in effect");
    let slot = skin.piece("tool.slot", WidgetState::Normal).unwrap().size[0] as f32;
    let line = h
        .ctx
        .global_style()
        .visuals
        .widgets
        .noninteractive
        .bg_stroke
        .width
        .round();
    let tools = h.panel("tools");
    assert_eq!(tools.width() - line, slot + 8.0, "{tools:?}");
    // El marco del lápiz, a 4 puntos de cada borde (sin contar la línea).
    let pencil = h.button("Pencil");
    assert_eq!(pencil.width(), slot);
    assert_eq!(pencil.left() - tools.left(), 4.0, "{pencil:?} in {tools:?}");
    assert_eq!(tools.right() - line - pencil.right(), 4.0);
    // Y sigue a todo el alto, al lado de la línea de tiempo.
    assert!(tools.bottom() > h.panel("timeline").top());
}

#[test]
fn without_pixel_ui_the_named_toolbar_keeps_its_width() {
    let h = LayoutHarness::editor();
    assert_eq!(h.panel("tools").width(), 184.0);
}
```

- [ ] **Step 2: Correrlos y comprobar que fallan**

Run: `cargo test -p cs-ui toolbar`
Expected: FAIL al compilar con `cannot find function toolbar_width in this scope` (en
`pixel_ui.rs`).

Para ver fallar el test de acomodo solo: comentar temporalmente
`the_toolbar_is_as_wide_as_the_slot_plus_the_margin` y correr
`cargo test -p cs-ui with_pixel_ui_the_toolbar`. Da FAIL con `left: 183.0`, `right: 31.0`, porque
hoy el panel mide 184 también con Pixel UI. Después se descomenta.

- [ ] **Step 3: Implementar**

En `crates/cs-ui/src/pixel_ui.rs`:

1. Después de `const MORE_MARK: f32 = 3.0;`:

```rust
/// Margen de la barra de íconos a cada lado del marco, en puntos.
pub const TOOLBAR_MARGIN: i8 = 4;
```

2. Antes del doc de `pub fn tool_button` (`/// Botón de herramienta de la barra de íconos: el
   marco …`):

```rust
/// Tamaño del marco de los botones de herramienta: el de la pieza `tool.slot` o el de respaldo.
fn slot_size(skin: &Skin) -> Vec2 {
    skin.piece(SLOT_PIECE, WidgetState::Normal)
        .map_or(FALLBACK_SLOT_SIZE, |p| {
            vec2(p.size[0] as f32, p.size[1] as f32)
        })
}

/// Ancho de la barra de íconos sin la línea que la separa del lienzo: el marco `tool.slot` más
/// `TOOLBAR_MARGIN` a cada lado. Sale de la pieza, así sigue al skin.
pub fn toolbar_width(skin: &Skin) -> f32 {
    slot_size(skin).x + 2.0 * f32::from(TOOLBAR_MARGIN)
}
```

3. En `tool_button`, reemplazar:

```rust
    let size = base.map_or(FALLBACK_SLOT_SIZE, |p| {
        vec2(p.size[0] as f32, p.size[1] as f32)
    });
```

por:

```rust
    let size = slot_size(skin);
```

(`base` se sigue usando para los bordes de la marca de variantes.)

En `crates/cs-ui/src/lib.rs`, en `editor_panels`, reemplazar el panel `"tools"` que dejó la
Task 2:

```rust
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(184.0)
            .show(ui, |ui| {
                // Con la UI a 2× o 3× y la ventana baja, la barra no cabe: scroll vertical.
                let skin = self.pixel.skin();
                panels::tools_scroll_area().show(ui, |ui| {
```

por:

```rust
        let skin = self.pixel.skin();
        let (width, frame) = match skin {
            // Con Pixel UI, la columna de íconos: el marco más el margen a cada lado, y la línea
            // que separa el panel del lienzo (egui la pone en el margen exterior).
            Some(skin) => {
                let line = ui
                    .style()
                    .visuals
                    .widgets
                    .noninteractive
                    .bg_stroke
                    .width
                    .round();
                let margin = egui::Margin::symmetric(pixel_ui::TOOLBAR_MARGIN, 2);
                (
                    pixel_ui::toolbar_width(skin) + line,
                    egui::Frame::side_top_panel(ui.style()).inner_margin(margin),
                )
            }
            // Sin Pixel UI, la barra con nombres.
            None => (184.0, egui::Frame::side_top_panel(ui.style())),
        };
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(width)
            .frame(frame)
            .show(ui, |ui| {
                // Con la UI a 2× o 3× y la ventana baja, la barra no cabe: scroll vertical.
                panels::tools_scroll_area().show(ui, |ui| {
```

El resto del panel (`ui.add_enabled_ui(!modal, |ui| panels::toolbar(ui, &mut self.doc.editor,
skin));` y los cierres) queda igual. El comentario de la Task 2 ("La toolbar antes que la línea
de tiempo…") queda arriba de `let skin`.

- [ ] **Step 4: Correr los tests**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS, 300 tests: `the_toolbar_is_as_wide_as_the_slot_plus_the_margin`,
`with_pixel_ui_the_toolbar_is_the_slot_plus_its_margin` y
`without_pixel_ui_the_named_toolbar_keeps_its_width`.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src/pixel_ui.rs crates/cs-ui/src/lib.rs crates/cs-ui/src/layout_tests.rs
git commit -m "Pixel UI: la toolbar mide el marco tool.slot más su margen

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Verificación final y prueba manual

**Files:** ninguno. Es solo verificación: si algo falla, se arregla en el archivo de la tarea
correspondiente y se commitea aparte.

- [ ] **Step 1: Verificación automática**

Run (con el toolchain exportado):
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo check -p cs-ui --target wasm32-unknown-unknown`

Expected: todo en verde, sin warnings.

- [ ] **Step 2: Espacio en disco y compilar la app (si el exe no está en uso)**

```bash
df -h /d | tail -1
```

Si quedan menos de 3 GB libres, no se compila: se salta la prueba manual y se anota en el
reporte. No se borra nada que no haya creado esta sesión.

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
- **Home:** no hay barra de opciones ni toolbar; la barra de estado dice la versión.
- **New (Pixel UI apagado):**
  - De arriba a abajo: menú, pestañas y la barra de opciones a todo el ancho.
  - La toolbar con nombres (184) va desde la barra de opciones hasta la barra de estado.
  - La línea de tiempo empieza a la derecha de la toolbar y pasa por debajo del panel derecho.
  - La barra de estado va a todo el ancho.
  - Ver si la fila de controles de la línea de tiempo queda cortada a la derecha (riesgo
    anotado) y anotarlo.
- **Barra de opciones:**
  - Con el Pencil: Brush size, el slider, Round/Square, Pixel-perfect y "Alt+click: pick color",
    con separadores entre grupos.
  - Symmetry está a la derecha, con la etiqueta y los siete botones en el orden de siempre (Off
    primero).
  - Al elegir Horizontal, el tooltip agrega "Ctrl+drag the dashed axes to move them". Al mover
    un eje aparece "Center axes".
  - Al pasar por Hand, Text, Slice, Magic Wand, Gradient y Move, el lienzo no se mueve: la barra
    tiene siempre el mismo alto.
  - Con Text, el combo Font y "Load font…" funcionan. Al hacer clic en el lienzo se abre la
    ventana Text.
- **Ventana angosta (~700 de ancho), con Slice:**
  - La rueda sobre la barra la mueve de lado.
  - Arrastrar sobre ella no la mueve.
  - Symmetry no se va.
- **Modo Tiles** (`--demo` con `CHIMPSPRITE_DEMO_TILES=1`): la barra muestra las opciones de la
  herramienta, como antes.
- **View → Pixel UI (2×):**
  - La toolbar es la columna de íconos, sin título, con 4 puntos de margen a cada lado del
    marco.
  - Va a todo el alto, y la línea de tiempo empieza a su derecha.
- **UI Scale 3× con la ventana baja:** la rueda mueve la columna de íconos.

Al terminar, cerrar la app que se abrió en este paso y restaurar:

```bash
cp /tmp/chimpsprite-app.ron.bak "$APPDATA/ChimpSprite/data/app.ron"
```

- [ ] **Step 4: Sin push ni PR**

El usuario junta las ramas al final.

---

## Autorrevisión

**Cobertura del spec:**
- **Acomodo:**
  - Menú y pestañas sin cambios, y la barra de opciones nueva a todo el ancho: Task 1 (panel) y
    Task 2 (`the_options_bar_is_between_the_tabs_and_the_canvas`).
  - El cuerpo: la toolbar a todo el alto, la línea de tiempo a su derecha, y el panel derecho
    como hoy respecto a la línea de tiempo. Lo hace la Task 2 con
    `the_toolbar_goes_down_beside_the_timeline` y
    `the_right_panel_keeps_ending_above_the_timeline`.
  - Barra de estado a todo el ancho: Task 2 (`the_status_bar_spans_the_window_under_everything`).
  - El orden de los paneles en egui: Task 2, Step 5. Ver "Orden de hoy y orden nuevo".
- **Toolbar:**
  - Solo los botones de grupo y el scroll sin arrastre: Task 1 (`toolbar`,
    `the_toolbar_has_only_the_tool_buttons`). `tools_scroll_area` no cambia.
  - El ancho con Pixel UI sale de la pieza más 4 a cada lado: Task 3.
  - Sin Pixel UI sigue a 184: Task 3 (`without_pixel_ui_the_named_toolbar_keeps_its_width`).
- **Barra de opciones:**
  - Una fila con las opciones de hoy y la ayuda, en horizontal y con separadores: Task 1
    (`tool_options`, `Groups`).
  - Symmetry al final, a la derecha: Task 1 (`symmetry_is_at_the_right_end_of_the_bar_in_the_usual_order`).
  - Alto fijo, y Hand sin opciones pero con la barra visible: Task 1
    (`the_options_bar_is_one_row_of_the_same_height_with_every_tool`).
  - Scroll horizontal con la rueda, sin partirse en dos renglones: Task 1
    (`the_options_scroll_sideways_with_the_wheel_when_they_do_not_fit`).
  - Sin barra en Home: Task 2 (`on_home_there_is_no_options_bar`).
  - Modo Tiles: igual que hoy, sin código aparte (ver Desviaciones). Se prueba a mano en la
    Task 4.
- **Tests del spec:**
  - Orden de los paneles: los cuatro pedidos están en `layout_tests.rs` (Task 2).
  - Contenido:
    - Pencil con Brush size y la forma;
    - Hand sin ellos;
    - Symmetry siempre;
    - la toolbar sin "Options" ni "Symmetry".

    Todos son de la Task 1.
  - Ancho con Pixel UI: Task 3.

**Placeholders:** no quedan "TBD", "TODO" ni "similar a…". Cada paso de código trae el código
completo, y los reemplazos dicen qué líneas cambian.

**Consistencia de tipos:**
- **`OPTIONS_BAR_HEIGHT: f32` y `tool_options_bar(&mut Ui, &mut Editor)`:** Tasks 1 y 2. El id
  del panel es `"tool_options"` en `lib.rs` y en los dos harness.
- **`toolbar(&mut Ui, &mut Editor, Option<&Skin>)`:** sin cambios de firma.
- **`layout(&mut self, &mut Ui, bool) -> Option<Rect>` y
  `editor_panels(&mut self, &mut Ui, bool) -> Rect`:** Task 2. `LayoutHarness::frame` usa
  `layout`.
- **`TOOLBAR_MARGIN: i8`, `slot_size(&Skin) -> Vec2` y `toolbar_width(&Skin) -> f32`:** Task 3.
  `Margin::symmetric` recibe `i8`; `toolbar_width` lo pasa a `f32` con `f32::from`.
- **`test_util::NoPresets`:** Task 2, usado por `file_actions.rs` y `layout_tests.rs`.
