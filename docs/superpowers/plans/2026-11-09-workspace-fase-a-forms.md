# Workspace, fase A: formas responsivas (`forms`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que los paneles de ancho fijo (FontForge Config, Groom, Lumen, Library, Dafont, Modelo) se armen con filas/columnas de taffy y se acomoden a cualquier ancho, viéndose idénticos a hoy en su ancho de referencia (el del mockup).

**Architecture:** Un módulo nuevo `crates/cs-ui/src/forms.rs` envuelve egui_taffy (flexbox/grid de taffy): da el punto de entrada `form()`, constructores de estilos (`row`, `column`, `fixed`, `grow`, `share`) y, para tests, un arnés `forms::testing::run_at(width, …)` con verificaciones genéricas (pixeles enteros, nada fuera del ancho, nada encimado). Después cada panel se migra en su propia tarea: deja de usar coordenadas absolutas y su ancho fijo pasa a ser el *ancho de referencia* que el dock le sigue dando. Los tests de layout que ya existen (que fijan la geometría del mockup al ancho de referencia) son la red de seguridad: tienen que seguir pasando sin cambios.

**Tech Stack:** Rust 2024, egui/eframe 0.36, egui_taffy 0.14 (taffy 0.9), kit pixel art de cs-ui.

**Spec:** `docs/superpowers/specs/2026-11-09-workspace-docking-design.md` (fase A).

## Global Constraints

- Worktree `D:/Work/chimpdeas/cs-tiles`, rama `workspace-docking`. Reglas de agentes: `D:/Work/chimpdeas/.refs/agent-rules.md` (léelas completas; git prohibido: checkout/restore/stash/reset/clean/add -A/add .; nunca `python -`; nunca matar procesos por nombre).
- Toolchain: `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-tiles/target`; todo cargo con `cmd //c "start /low /b /wait cargo …"`.
- Dependencia: `egui_taffy = "=0.14.0"` (versión fija; es la que pide egui 0.36). Solo `forms.rs` hace `use egui_taffy`; los paneles usan lo que `forms` reexporta.
- El mockup manda: en el ancho de referencia de cada panel la geometría queda **idéntica** a hoy (los tests de layout existentes no se tocan, salvo que un test mida algo que dependía de un detalle de implementación y no de la geometría; justifícalo en el commit).
- Más angosto que la referencia: las filas se parten; más ancho: crecen los campos `grow`; por debajo del ancho mínimo del panel, scroll vertical (nunca horizontal).
- Todos los rects en puntos enteros (con la escala 2 de la UI, cada punto es un pixel del skin).
- Textos de UI en inglés; comentarios y docs en español; un commit por tarea con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Al final de cada tarea: `cargo test -p cs-ui`, `cargo clippy -p cs-ui --all-targets -- -D warnings`, `cargo fmt --all --check`.

## File Structure

- Create `crates/cs-ui/src/forms.rs` — envoltorio de egui_taffy: `form`, estilos, `label`, y `#[cfg(test)] pub(crate) mod testing` (arnés + verificaciones).
- Modify `crates/cs-ui/Cargo.toml` — dependencia `egui_taffy`.
- Modify `crates/cs-ui/src/lib.rs` — `mod forms;`.
- Modify (una tarea cada uno) `font_panel.rs` (vista Config), `groom.rs`, `lumen.rs`, `library_panel.rs`, `dafont.rs`, `model_panel.rs` — cambiar coordenadas absolutas por `forms`; su constante de ancho queda como *ancho de referencia* (`REFERENCE_WIDTH`) y se agrega `MIN_WIDTH`.
- Tests: `forms.rs` (unitarios) y un test responsivo por panel en el módulo de tests de ese panel.

---

### Task 1: Módulo `forms` sobre egui_taffy, con arnés de pruebas

**Files:**
- Create: `crates/cs-ui/src/forms.rs`
- Modify: `crates/cs-ui/Cargo.toml` (sección `[dependencies]`)
- Modify: `crates/cs-ui/src/lib.rs` (lista de `mod`)

**Interfaces:**
- Produces (lo usan las tareas 2–7):
  - `pub(crate) use egui_taffy::{Tui, TuiBuilderLogic};` y `pub(crate) use egui_taffy::taffy::Style;`
  - `pub(crate) fn form(ui: &mut Ui, id: impl std::hash::Hash, gap: f32, add: impl FnOnce(&mut Tui))` — una columna del ancho disponible.
  - `pub(crate) fn column(gap: f32) -> Style`, `row(gap: f32) -> Style` (parte filas), `row_nowrap(gap: f32) -> Style`
  - `pub(crate) fn fixed(w: f32, h: f32) -> Style`, `grow(min: f32, basis: f32, max: Option<f32>, h: f32) -> Style`, `share(min: f32, h: f32) -> Style` (crecen parejo desde base 0)
  - `pub(crate) fn label_width(ui: &Ui, text: &str) -> f32` (ancho de tinta + 1, entero)
  - tests: `forms::testing::run_at(width: f32, draw: impl FnMut(&mut Ui)) -> Probe`, `Probe { widgets: Vec<(String, Rect)> }`, `Probe::assert_whole_px(&self)`, `Probe::assert_within(&self, width: f32)`, `Probe::assert_no_overlap(&self)`, `Probe::rect(&self, name: &str) -> Rect`.

- [ ] **Step 1: Dependencia y módulo vacío**

En `crates/cs-ui/Cargo.toml`, en `[dependencies]` (orden alfabético con lo demás):

```toml
# Formas responsivas (flexbox / grid de taffy) para los paneles: solo las usa `forms`. Versión
# fija: es la que pide egui 0.36.
egui_taffy = "=0.14.0"
```

En `crates/cs-ui/src/lib.rs`, junto a los demás `mod` privados (orden alfabético): `mod forms;`

- [ ] **Step 2: Escribir los tests que fallan (al final de `forms.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::testing::run_at;
    use super::*;

    /// Una forma chica como las de los paneles: etiqueta + campo que crece y tres botones parejos.
    fn sample(ui: &mut Ui) {
        let skin = crate::pixel_ui::frame_skin(ui.ctx()).expect("skin");
        form(ui, "sample", 4.0, |tui| {
            tui.style(row(4.0)).add(|tui| {
                let lw = 37.0;
                tui.style(fixed(lw, 17.0)).ui(|ui| label(ui, "Family", lw));
                tui.style(grow(60.0, 160.0, Some(160.0), 17.0)).ui(|ui| {
                    let w = ui.max_rect().width();
                    let mut text = String::from("Mine");
                    crate::kit::input::text_input(ui, &skin, &mut text, w, "Family name", "Font");
                });
            });
            tui.style(row(4.0)).add(|tui| {
                for name in ["Generate", "Build", "Export"] {
                    tui.style(share(50.0, 17.0)).ui(|ui| {
                        let w = ui.max_rect().width();
                        crate::kit::button::button_sized(ui, &skin, name, w);
                    });
                }
            });
        });
    }

    #[test]
    fn a_form_lands_on_whole_points_inside_its_width_at_any_width() {
        for width in [120.0, 220.0, 400.0, 600.0] {
            let probe = run_at(width, sample);
            probe.assert_whole_px();
            probe.assert_within(width);
            probe.assert_no_overlap();
        }
    }

    #[test]
    fn narrow_rows_wrap_and_wide_ones_grow() {
        let wide = run_at(600.0, sample);
        let (g, b) = (wide.rect("Generate"), wide.rect("Build"));
        assert_eq!(g.top(), b.top(), "one row at 600");
        assert_eq!(wide.rect("Family name").width(), 160.0, "grows up to its max");
        let narrow = run_at(120.0, sample);
        let (g, x) = (narrow.rect("Generate"), narrow.rect("Export"));
        assert!(x.top() > g.top(), "the buttons wrap at 120: {g:?} {x:?}");
    }

    #[test]
    fn shared_widths_differ_by_at_most_one_point() {
        let probe = run_at(400.0, sample);
        let ws: Vec<f32> = ["Generate", "Build", "Export"]
            .iter()
            .map(|n| probe.rect(n).width())
            .collect();
        let (min, max) = ws.iter().fold((f32::MAX, 0.0f32), |(a, b), &w| (a.min(w), b.max(w)));
        assert!(max - min <= 1.0, "{ws:?}");
    }
}
```

- [ ] **Step 3: Correr para ver que fallan**

Run: `cmd //c "start /low /b /wait cargo test -q -p cs-ui --lib forms"`
Expected: no compila (`form`, `row`, `testing`… no existen).

- [ ] **Step 4: Implementar `forms.rs`**

```rust
//! Formas responsivas: los paneles se arman con filas, columnas y grillas (el flexbox / CSS grid de
//! taffy, vía egui_taffy) en vez de coordenadas absolutas, así se acomodan al ancho que les toque.
//! Los tamaños van en puntos enteros: con la escala de la UI cada punto es un pixel del skin.
//! Solo este módulo conoce egui_taffy; los paneles usan lo que reexporta.

use eframe::egui::{self, Id, Ui, vec2};
use egui_taffy::taffy::prelude::*;

pub(crate) use egui_taffy::taffy::Style;
pub(crate) use egui_taffy::{Tui, TuiBuilderLogic};

/// Una forma del ancho disponible de `ui`: una columna con `gap` entre sus hijos.
pub(crate) fn form(ui: &mut Ui, id: impl std::hash::Hash, gap: f32, add: impl FnOnce(&mut Tui)) {
    egui_taffy::tui(ui, Id::new(id))
        .reserve_available_width()
        .style(Style {
            size: Size {
                width: percent(1.0),
                height: auto(),
            },
            ..column(gap)
        })
        .show(add);
}

/// Hijos uno debajo de otro, con `gap` entre ellos.
pub(crate) fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        gap: length(gap),
        ..Default::default()
    }
}

/// Hijos uno al lado del otro; si no caben, siguen en la fila de abajo.
pub(crate) fn row(gap: f32) -> Style {
    Style {
        flex_wrap: FlexWrap::Wrap,
        ..row_nowrap(gap)
    }
}

/// Hijos uno al lado del otro, sin partir la fila.
pub(crate) fn row_nowrap(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        gap: length(gap),
        align_items: Some(AlignItems::Start),
        size: Size {
            width: percent(1.0),
            height: auto(),
        },
        ..Default::default()
    }
}

/// Una hoja de `w` × `h` que no se encoge.
pub(crate) fn fixed(w: f32, h: f32) -> Style {
    Style {
        size: Size {
            width: length(w),
            height: length(h),
        },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

/// Una hoja de alto `h` que mide `basis` de ancho y crece (hasta `max`, si hay) o se achica
/// (hasta `min`) con lo que sobre o falte en su fila.
pub(crate) fn grow(min: f32, basis: f32, max: Option<f32>, h: f32) -> Style {
    Style {
        flex_grow: 1.0,
        flex_shrink: 1.0,
        flex_basis: length(basis),
        min_size: Size {
            width: length(min),
            height: length(h),
        },
        max_size: Size {
            width: max.map_or(auto(), length),
            height: length(h),
        },
        size: Size {
            width: auto(),
            height: length(h),
        },
        ..Default::default()
    }
}

/// Una hoja que reparte el ancho de su fila en partes iguales con sus hermanas `share` (desde
/// base 0), sin bajar de `min`.
pub(crate) fn share(min: f32, h: f32) -> Style {
    Style {
        flex_basis: length(0.0),
        ..grow(min, 0.0, None, h)
    }
}

/// El ancho que necesita `text` en la fuente de la UI (tinta + 1), en puntos enteros.
pub(crate) fn label_width(ui: &Ui, text: &str) -> f32 {
    crate::kit::button::ink_width(ui, text).ceil() + 1.0
}

/// Una etiqueta de `width` en la fila de 17 de los controles del kit (las mayúsculas a la altura
/// de las de los campos), con nombre para los lectores de pantalla.
pub(crate) fn label(ui: &mut Ui, text: &str, width: f32) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, crate::kit::input::HEIGHT), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    crate::pixel_ui::small_text_at(ui, rect, text);
}
```

Notas para el implementador:
- `crate::kit::button::ink_width` y `crate::kit::button::button_sized` existen (los usó el prototipo `crates/cs-ui/examples/proto_workspace.rs` de la rama `proto-workspace`, commit `8a47d11`, función `form_taffy`); si alguna es privada, hazla `pub(crate)`.
- `crate::pixel_ui::small_text_at` **puede no existir**: busca cómo el panel de FontForge dibuja sus etiquetas (`text_at` en `font_panel.rs`) y usa/expón esa función con ese nombre (`pub(crate) fn small_text_at(ui: &Ui, rect: Rect, text: &str)`) en `pixel_ui.rs`, moviéndola sin cambiar cómo dibuja. Agrega `use eframe::egui;` si hace falta.
- Si la API de egui_taffy 0.14 difiere en algún nombre (`reserve_available_width`, `show`, `add`, `ui`), ajusta según el código fuente en `D:/Work/chimpdeas/.rust-toolchain/cargo/registry/src/*/egui_taffy-0.14.0/`.

- [ ] **Step 5: El arnés `testing` (en `forms.rs`, antes de `mod tests`)**

```rust
/// Para los tests de los paneles: dibuja en un `Ui` de `width` (con el skin y las fuentes de la
/// app, escala 1) y junta los widgets con nombre que quedaron.
#[cfg(test)]
pub(crate) mod testing {
    use eframe::egui::{self, Pos2, RawInput, Rect, Ui, Vec2, accesskit::Role};

    pub(crate) struct Probe {
        /// Los controles con nombre (botones, casillas, campos, selects, etiquetas) y su rect.
        pub widgets: Vec<(String, Rect)>,
    }

    /// Lo que se mide: hojas que el usuario ve (no los grupos que las contienen).
    const LEAVES: &[Role] = &[
        Role::Button,
        Role::CheckBox,
        Role::TextInput,
        Role::ComboBox,
        Role::SpinButton,
        Role::Slider,
        Role::Label,
        Role::RadioButton,
    ];

    pub(crate) fn run_at(width: f32, mut draw: impl FnMut(&mut Ui)) -> Probe {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = crate::skin::Skin::embedded(&ctx);
        ctx.set_fonts(crate::appearance::font_definitions(true));
        crate::appearance::apply_style(&ctx, Some(skin.colors()));
        let mut widgets = Vec::new();
        // Tres frames: las fuentes llegan en el primero y taffy mide en el segundo.
        for _ in 0..3 {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(2000.0, 2000.0))),
                ..Default::default()
            };
            let output = ctx.run_ui(input, |ui| {
                let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(width, 1900.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.set_width(width);
                    draw(ui);
                });
            });
            let nodes = output
                .platform_output
                .accesskit_update
                .map(|u| u.nodes)
                .unwrap_or_default();
            widgets = nodes
                .iter()
                .filter(|(_, n)| LEAVES.contains(&n.role()))
                .filter_map(|(_, n)| {
                    let b = n.bounds()?;
                    let rect = Rect::from_min_max(
                        Pos2::new(b.x0 as f32 - 10.0, b.y0 as f32 - 10.0),
                        Pos2::new(b.x1 as f32 - 10.0, b.y1 as f32 - 10.0),
                    );
                    Some((n.label()?.to_string(), rect))
                })
                .collect();
        }
        Probe { widgets }
    }

    impl Probe {
        pub(crate) fn rect(&self, name: &str) -> Rect {
            self.widgets
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, r)| *r)
                .unwrap_or_else(|| panic!("no {name:?} in {:?}", self.widgets))
        }

        /// Cada control empieza y termina en puntos enteros (nada de medio pixel del skin).
        pub(crate) fn assert_whole_px(&self) {
            for (name, r) in &self.widgets {
                for v in [r.min.x, r.min.y, r.max.x, r.max.y] {
                    assert_eq!(v, v.round(), "{name:?} at {r:?}");
                }
            }
        }

        /// Nada se sale a la derecha del ancho del panel.
        pub(crate) fn assert_within(&self, width: f32) {
            for (name, r) in &self.widgets {
                assert!(r.min.x >= 0.0 && r.max.x <= width, "{name:?} at {r:?} past {width}");
            }
        }

        /// Ningún control tapa a otro.
        pub(crate) fn assert_no_overlap(&self) {
            for (i, (a, ra)) in self.widgets.iter().enumerate() {
                for (b, rb) in &self.widgets[i + 1..] {
                    let inter = ra.intersect(*rb);
                    assert!(
                        inter.width() <= 0.0 || inter.height() <= 0.0,
                        "{a:?} {ra:?} overlaps {b:?} {rb:?}"
                    );
                }
            }
        }
    }
}
```

(Si `Skin::embedded` o `appearance::font_definitions/apply_style` tienen otra firma, cópiala de `crates/cs-ui/src/color_panel/tests.rs`, `Harness::new`.)

- [ ] **Step 6: Correr los tests**

Run: `cmd //c "start /low /b /wait cargo test -q -p cs-ui --lib forms"`
Expected: 3 passed. Si `assert_whole_px` falla, revisa que el `max_rect` de prueba empiece en puntos enteros y que taffy tenga el redondeo activado (`taffy` lo trae por defecto; egui_taffy lo respeta).

- [ ] **Step 7: Verificación y commit**

Run: `cmd //c "start /low /b /wait cargo clippy -q -p cs-ui --all-targets -- -D warnings"` y `cargo fmt --all --check` → limpios.

```bash
git add crates/cs-ui/Cargo.toml Cargo.lock crates/cs-ui/src/forms.rs crates/cs-ui/src/lib.rs crates/cs-ui/src/pixel_ui.rs crates/cs-ui/src/kit/button.rs
git commit -m "forms: filas, columnas y hojas de taffy (egui_taffy) para paneles responsivos" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
(Agrega solo los archivos que de verdad cambiaste.)

---

## Receta común de migración (tareas 2–7)

Cada panel se migra igual. Léela antes de cada tarea:

1. **Ancho de referencia.** La constante de ancho del panel (`WIDTH`) se renombra `REFERENCE_WIDTH` (el dock y `lib.rs` la siguen usando para abrir el flyout de ese ancho; nada cambia de tamaño todavía) y se agrega `pub const MIN_WIDTH: f32` = el ancho más chico en que el panel se ve bien (elígelo midiendo; típicamente 200–240). Dentro del panel **nada** usa `REFERENCE_WIDTH`: todo sale de `ui.available_width()`.
2. **Contenido a `forms`.** Las cuentas de coordenadas (`origin + vec2(x, y)`, `at(x, y)`, `child(ui, rect, …)` con rects calculados a mano, constantes `*_X`, columnas `LEFT/RIGHT/DIVIDER`) se reemplazan por `forms::form` con `row`/`column`/`fixed`/`grow`/`share`. Los gaps y tamaños de los estilos se eligen para que **en el ancho de referencia** cada control caiga exactamente donde hoy (los tests de layout existentes lo verifican). Los lienzos libres (vistas previas, dameros, mapas) se vuelven una hoja `fixed`/`grow` y por dentro se dibujan como hoy con `ui.max_rect()`.
3. **Columnas lado a lado** (Groom, Lumen): un `row` con dos hijos `column`; cada columna con `grow(min, basis, …)` de su ancho de referencia, de modo que por debajo de `MIN_WIDTH * 2` la segunda columna baje debajo de la primera. Las líneas grabadas entre columnas se dibujan solo si las columnas están lado a lado (compara los rects de las dos columnas).
4. **Scroll.** Si el contenido no cabe a lo alto, el panel ya vive en un scroll del dock (`Body::Tall`) o maneja su scroll (`Body::Fixed`): no agregues scroll horizontal.
5. **Test responsivo** en el módulo de tests del panel (ver cada tarea): `forms::testing::run_at` a `MIN_WIDTH`, `REFERENCE_WIDTH` y `600.0` con `assert_whole_px`, `assert_within`, `assert_no_overlap`.
6. **Red de seguridad:** `cargo test -p cs-ui` completo debe pasar **sin tocar** los tests de layout existentes del panel. Además, captura antes/después al ancho de referencia (variables de demo del panel en `crates/cs-ui/src/demo.rs`) y compara con PIL: 0 pixeles distintos dentro del panel (si hay diferencias, explícalas y corrígelas).
7. Commit por panel.

---

### Task 2: FontForge, vista Config, con `forms`

**Files:**
- Modify: `crates/cs-ui/src/font_panel.rs` (la vista Config / ajustes: constantes `FIELD_X`, `GHOST_X`, `MODE_X`… y la función que la dibuja; la busca `grep -n "MODE_X\|GHOST_X\|FIELD_X" crates/cs-ui/src/font_panel.rs`)
- Test: el módulo de tests de `font_panel.rs` (o `layout_tests.rs` donde están `the_font_settings_are_the_mockup_layout`; el test nuevo va en `font_panel.rs` si tiene `mod tests`, si no, en `layout_tests.rs`).

**Interfaces:**
- Consumes: `forms::{form, row, column, fixed, grow, share, label, label_width, Tui, TuiBuilderLogic}`, `forms::testing::run_at`.
- Produces: `font_panel::REFERENCE_WIDTH` (antes `WIDTH`), `font_panel::MIN_WIDTH`.

- [ ] **Step 1: Test responsivo que falla**

```rust
#[test]
fn the_font_settings_reflow_at_any_width() {
    use crate::forms::testing::run_at;
    for width in [MIN_WIDTH, REFERENCE_WIDTH, 600.0] {
        let mut editor = crate::editor::Editor::with_sprite(cs_core::Sprite::new(8, 8, cs_core::ColorMode::Rgba));
        editor.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
        let mut state = FontPanelState { view: PanelView::Config, ..Default::default() };
        let probe = run_at(width, |ui| {
            config_view(ui, &mut editor, &mut state);
        });
        probe.assert_whole_px();
        probe.assert_within(width);
        probe.assert_no_overlap();
        // Lo que tiene que estar a cualquier ancho.
        for name in ["Family", "Style", "Ghost", "Italic", "Monospace", "Baseline", "New Variant…"] {
            probe.rect(name);
        }
    }
}
```

Ajusta los nombres `FontPanelState`, `PanelView::Config` y `config_view` a los reales (`grep -n "PanelView::Config\|fn .*config" crates/cs-ui/src/font_panel.rs`); si la vista Config no es una función propia, extráela primero a `fn config_view(ui: &mut Ui, editor: &mut Editor, state: &mut FontPanelState)` sin cambiar lo que dibuja (commit aparte "font_panel: la vista Config en su propia función").

- [ ] **Step 2:** Correr `cargo test -q -p cs-ui --lib the_font_settings_reflow_at_any_width` → falla (`MIN_WIDTH` no existe o algo se sale/encima a `MIN_WIDTH`).
- [ ] **Step 3:** Migrar la vista Config según la receta (puntos 1–4). Referencia de cómo queda con taffy: `form_taffy` del prototipo (`proto_workspace.rs`, rama `proto-workspace`). En el ancho de referencia los números de `the_font_settings_are_the_mockup_layout` (comentados desde el mockup) tienen que seguir saliendo iguales.
- [ ] **Step 4:** `cargo test -q -p cs-ui` completo → todo pasa (incluido `the_font_settings_are_the_mockup_layout` sin cambios).
- [ ] **Step 5:** Captura antes/después (`CHIMPSPRITE_DEMO_FONT=1 CHIMPSPRITE_DEMO_FONT_CONFIG=sample`, build release en `target/gui-a2`, respaldo/restauración de app.ron con tu nombre) y diferencia de pixeles dentro del panel = 0.
- [ ] **Step 6:** clippy + fmt + commit `"FontForge: los ajustes con forms (se acomodan al ancho; idénticos al mockup en su ancho)"`.

### Task 3: Pixel Groom con `forms`

**Files:** Modify `crates/cs-ui/src/groom.rs` (constantes `WIDTH`, `LEFT`, `RIGHT`, `DIVIDER`, `COLUMNS_TOP`, `at(...)`, `child(...)` en `GroomPanel::ui` y `settings_ui`; `library_panel.rs` usa `groom::WIDTH`: cámbialo a `groom::REFERENCE_WIDTH`). Test en `mod tests` de `groom.rs`.

**Interfaces:** Consumes `forms`; Produces `groom::REFERENCE_WIDTH`, `groom::MIN_WIDTH`.

- [ ] **Step 1: Test responsivo que falla** (con y sin imagen)

```rust
#[test]
fn the_groom_panel_reflows_at_any_width() {
    use crate::forms::testing::run_at;
    for width in [MIN_WIDTH, REFERENCE_WIDTH, 600.0] {
        for with_image in [false, true] {
            let mut panel = GroomPanel::default();
            if with_image {
                panel.load("big".into(), big());
            }
            let probe = run_at(width, |ui| {
                panel.ui(ui, Rgba::rgb(0, 0, 0), &[]);
            });
            probe.assert_whole_px();
            probe.assert_within(width);
            probe.assert_no_overlap();
            probe.rect("Import Image…");
        }
    }
}
```

(`big()` ya existe en los tests de `groom.rs`; si `load` tiene otra firma, usa la del demo `CHIMPSPRITE_DEMO_GROOM` en `demo.rs`.)

- [ ] **Step 2:** correr → falla.
- [ ] **Step 3:** migrar según la receta, con el punto 3 (dos columnas que bajan una debajo de otra por debajo de `MIN_WIDTH * 2`; la línea grabada vertical solo si están lado a lado). El panel sin imagen (los changos + la frase) ya está centrado con `empty_art_rect`: déjalo, solo que el área salga de `forms`/`available_width`.
- [ ] **Step 4:** `cargo test -q -p cs-ui` completo sin tocar los tests de Groom existentes.
- [ ] **Step 5:** captura antes/después con `CHIMPSPRITE_DEMO_GROOM=<imagen>` y `=empty` → 0 pixeles distintos dentro del panel.
- [ ] **Step 6:** clippy + fmt + commit `"Pixel Groom con forms: dos columnas que bajan si no caben"`.

### Task 4: Lumen con `forms`

**Files:** Modify `crates/cs-ui/src/lumen.rs` (`WIDTH = 434` y el layout de `Lumen::ui`, líneas ~462–574). Test en su `mod tests`.

**Interfaces:** Consumes `forms`; Produces `lumen::REFERENCE_WIDTH`, `lumen::MIN_WIDTH`.

- [ ] **Step 1: Test responsivo que falla**

```rust
#[test]
fn the_lumen_panel_reflows_at_any_width() {
    use crate::forms::testing::run_at;
    for width in [MIN_WIDTH, REFERENCE_WIDTH, 600.0] {
        let mut lumen = Lumen::default();
        let probe = run_at(width, |ui| {
            lumen.ui(ui);
        });
        probe.assert_whole_px();
        probe.assert_within(width);
        probe.assert_no_overlap();
    }
}
```

(Si `Lumen` necesita una imagen para mostrar sus controles, cárgala como lo hace `CHIMPSPRITE_DEMO_LUMEN` en `demo.rs` y prueba ambos estados, como en Groom.)

- [ ] **Step 2–6:** igual que la tarea 3 (receta, test completo, capturas con `CHIMPSPRITE_DEMO_LUMEN=<albedo>`, commit `"Lumen con forms: se acomoda al ancho"`).

### Task 5: Library con `forms`

**Files:** Modify `crates/cs-ui/src/library_panel.rs` (`WIDTH = groom::WIDTH` → `REFERENCE_WIDTH` propio de 434, y el layout de `show`). Test en su módulo de tests o en `layout_tests.rs` junto a los de Library.

**Interfaces:** Produces `library_panel::REFERENCE_WIDTH`, `library_panel::MIN_WIDTH`.

- [ ] **Step 1: Test responsivo que falla** — usa `run_at` con la firma real de `library_panel::show` (`grep -n "pub fn show" -A 12 crates/cs-ui/src/library_panel.rs`), con un documento que tenga un símbolo, un modelo y una fuente (cópialo de los tests de Library en `layout_tests.rs`), a `MIN_WIDTH`, `REFERENCE_WIDTH` y 600, con las tres verificaciones y `probe.rect("Import…")`.
- [ ] **Step 2–6:** receta; capturas con `CHIMPSPRITE_DEMO_LIBRARY` (o la variable que abre Library en `demo.rs`); commit `"Library con forms: las secciones se acomodan al ancho"`.

### Task 6: Dafont con `forms`

**Files:** Modify `crates/cs-ui/src/dafont.rs` (`WIDTH = 364`, `top_row` y las tarjetas de ~350 de ancho; el logo sigue en la esquina con el painter de recorte ampliado que ya tiene). Test en su `mod tests`.

**Interfaces:** Produces `dafont::REFERENCE_WIDTH`, `dafont::MIN_WIDTH`.

- [ ] **Step 1: Test responsivo que falla** — `run_at` dibujando `DafontPanel::ui` con una lista cargada a mano (sin red: llena `fonts` con 2 entradas como lo hacen los tests existentes de `dafont.rs`; si no hay ejemplo, crea las entradas con los campos públicos/`pub(crate)` del struct), a `MIN_WIDTH`, `REFERENCE_WIDTH` y 600, con las tres verificaciones y `probe.rect("Submit")`.
- [ ] **Step 2–6:** receta (las tarjetas ocupan el ancho disponible; la fila de arriba parte "Sort by"/Submit abajo si no cabe; el logo se queda arriba a la derecha); capturas con `CHIMPSPRITE_DEMO_DAFONT=1`; commit `"Dafont con forms: la fila de arriba y las tarjetas se acomodan al ancho"`.

### Task 7: Panel del modelo 3D con `forms`

**Files:** Modify `crates/cs-ui/src/model_panel.rs` (`WIDTH = 227`, y los `min(available_width)` parciales de ~462). Test en su módulo de tests.

**Interfaces:** Produces `model_panel::REFERENCE_WIDTH`, `model_panel::MIN_WIDTH`.

- [ ] **Step 1: Test responsivo que falla** — `run_at` con la firma real de `model_panel::show` y un modelo de ejemplo (cópialo de los tests de ChimpCAD), a `MIN_WIDTH`, `REFERENCE_WIDTH` y 600, con las tres verificaciones.
- [ ] **Step 2–6:** receta; capturas con `CHIMPSPRITE_DEMO_MODEL` (la que abre ChimpCAD en `demo.rs`); commit `"Panel del modelo con forms: se acomoda al ancho"`.

### Task 8: Cierre de la fase A

**Files:** Modify `docs/superpowers/specs/2026-11-09-workspace-docking-design.md` (sección "Fases": marcar A hecha con notas), `assets/ui/README.md` o un `docs/forms.md` corto (cómo armar un panel con `forms`, con el ejemplo de la tarea 1).

- [ ] **Step 1:** `grep -rn "groom::WIDTH\|lumen::WIDTH\|dafont::WIDTH\|font_panel::WIDTH\|model_panel::WIDTH\|library_panel::WIDTH" crates/` → vacío (todo usa `REFERENCE_WIDTH`).
- [ ] **Step 2:** Verificación completa como CI: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, y `cargo check --target wasm32-unknown-unknown -p cs-core -p cs-3d -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite` (egui_taffy tiene que compilar para wasm).
- [ ] **Step 3:** Escribir `docs/forms.md` (≤ 60 líneas): qué es `forms`, los estilos, la receta de migración y el arnés de pruebas.
- [ ] **Step 4:** Commit `"Fase A del workspace: forms y paneles responsivos (docs)"`.
