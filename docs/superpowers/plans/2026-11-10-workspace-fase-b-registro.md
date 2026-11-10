# Workspace, fase B: registro de paneles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que todo panel acoplable tenga un id y una ficha (título, ícono, anchos, dónde vive) y se dibuje por una sola entrada `draw_panel(id) -> Option<PanelAction>` + `apply_panel_action(action)`, sin ningún cambio visible: la base para que la fase C meta cualquier panel en egui_dock.

**Architecture:** Un módulo nuevo `crates/cs-ui/src/panel_registry.rs` con `PanelId` (13 paneles), `PanelSpec` y `Host`. `dock::DockPanel` pasa a ser un alias de `PanelId` y toma título/ícono de la ficha. En `ChimpApp` (archivo nuevo `crates/cs-ui/src/app_panels.rs`) cada panel se parte en **dibujar** (préstamos disjuntos de campos, devuelve una acción) y **aplicar** (`&mut self` completo, después). Los anfitriones de hoy (flyouts del dock, `Panel::left/right/bottom`, la ventana de Preview) se quedan igual pero llaman a `draw_panel`. Los tests existentes (que fijan geometría, ids de flyout y etiquetas) son la red de seguridad: no se tocan.

**Tech Stack:** Rust 2024, egui/eframe 0.36, cs-ui.

**Spec:** `docs/superpowers/specs/2026-11-09-workspace-docking-design.md` (fase B; sección `panels`).

## Global Constraints

- Worktree `D:/Work/chimpdeas/cs-tiles`, rama `workspace-registry`. Reglas de agentes: `D:/Work/chimpdeas/.refs/agent-rules.md` (léelas completas; git prohibido: checkout/restore/stash/reset/clean/add -A/add .; nunca `python -`; nunca matar procesos por nombre).
- Toolchain: `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-tiles/target`; todo cargo con `cmd //c "start /low /b /wait cargo …"`.
- **Sin cambios visibles ni de comportamiento.** Todos los tests existentes pasan sin modificarse (los ids de flyout `("dock_flyout", label)`, las etiquetas de los botones y los rects quedan iguales). Si un test existente tiene que cambiar, para y reporta NEEDS_CONTEXT.
- Los títulos de la ficha son exactamente los de hoy: "Artboards", "Slices", "Lospec", "Dafont", "Groom", "Lumen", "Library", "FontForge", "3D", "Timeline", "Preview", más "Tools" y "Color" para los que hoy no tienen.
- Dafont solo existe fuera de wasm (`cfg(not(target_arch = "wasm32"))`), como hoy.
- Textos de UI en inglés; comentarios y docs en español; un commit por tarea con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Al final de cada tarea: `cargo test -p cs-ui`, `cargo clippy -p cs-ui --all-targets -- -D warnings`, `cargo fmt --all --check`; la última tarea corre además el check de wasm igual que CI.

## File Structure

- Create `crates/cs-ui/src/panel_registry.rs` — `PanelId`, `PanelSpec`, `Host`, fichas y órdenes (`ALL`, `DOCK`). Sin egui más allá de tipos simples; testeable solo.
- Create `crates/cs-ui/src/app_panels.rs` — `impl ChimpApp { fn draw_panel, fn apply_panel_action }` y `enum PanelAction`; aquí se mudan los cuerpos de `groom_panel`, `lumen_panel`, `font_panel`, `model_panel`, `library_panel`, `dafont_panel` (lib.rs) y `lospec_panel` (palette_ui.rs).
- Modify `crates/cs-ui/src/dock.rs` — `DockPanel` = alias de `PanelId`; `label/icon/plain` salen de la ficha; Artboards/Slices dejan de dibujarse dentro de `dock::show`.
- Modify `crates/cs-ui/src/lib.rs` — el `match` de `app_panel` se vuelve una llamada genérica; las regiones fijas (Tools, Color, Timeline, panel del modelo) llaman a `draw_panel`.
- Modify `crates/cs-ui/src/preview.rs` — separar el contenido de la ventana.
- Modify los `MIN_WIDTH` de groom, lumen, library_panel, dafont, model_panel, font_panel — quitar sus `#[expect(dead_code)]` / `cfg_attr(…expect(dead_code))` cuando la ficha los lea.

---

### Task 1: `PanelId`, `PanelSpec` y `DockPanel` como alias

**Files:**
- Create: `crates/cs-ui/src/panel_registry.rs`
- Modify: `crates/cs-ui/src/lib.rs` (`mod panel_registry;`), `crates/cs-ui/src/dock.rs:47-137` (enum, `ALL`, `label`, `icon`, `plain`) y `:190-194` (consts de Timeline/Preview)
- Modify: `groom.rs:37`, `lumen.rs:30`, `library_panel.rs:31`, `dafont.rs:33`, `model_panel.rs:39-42`, `font_panel.rs:40` (quitar el `expect(dead_code)` de `MIN_WIDTH`)
- Test: `mod tests` de `panel_registry.rs`

**Interfaces:**
- Produces:
  - `pub enum PanelId { Tools, Color, Timeline, Preview, Artboards, Library, Slices, Lospec, #[cfg(not(target_arch = "wasm32"))] Dafont, Groom, Lumen, Font, Model }` (`Clone, Copy, Debug, PartialEq, Eq, Hash`)
  - `pub enum Body { Fit, Tall, Fixed }` (se mueve de dock.rs, hoy privado, a `panel_registry` como `pub(crate)`)
  - `pub(crate) enum Host { Left, Right, Bottom, Window, Flyout(Body) }`
  - `pub(crate) struct PanelSpec { pub title: &'static str, pub icon: &'static str, pub plain: &'static str, pub reference_width: f32, pub min_width: f32, pub closable: bool, pub host: Host }`
  - `impl PanelId { pub(crate) fn spec(self) -> PanelSpec; pub fn label(self) -> &'static str; pub(crate) const ALL: &[PanelId]; pub(crate) const DOCK: &[PanelId] }` (`DOCK` = el orden de botones de hoy: Artboards, Library, Slices, Lospec, Dafont (no wasm), Groom, Lumen, Font, Model)
  - En dock.rs: `pub type DockPanel = crate::panel_registry::PanelId;` y el `ALL` del dock pasa a ser `PanelId::DOCK`.

- [ ] **Step 1: Tests que fallan** (en `panel_registry.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dock_buttons_keep_their_order_titles_and_icons() {
        let titles: Vec<&str> = PanelId::DOCK.iter().map(|p| p.spec().title).collect();
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(
            titles,
            ["Artboards", "Library", "Slices", "Lospec", "Dafont", "Groom", "Lumen", "FontForge", "3D"]
        );
        let icons: Vec<&str> = PanelId::DOCK.iter().map(|p| p.spec().icon).collect();
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(
            icons,
            [
                "icon.dock.artboards", "icon.dock.library", "icon.dock.slices", "icon.dock.lospec",
                "icon.dock.dafont", "icon.dock.groom", "icon.dock.lumen", "icon.dock.font",
                "icon.dock.model"
            ]
        );
        for p in PanelId::DOCK {
            assert!(matches!(p.spec().host, Host::Flyout(_)), "{p:?}");
            assert_eq!(p.label(), p.spec().title);
        }
    }

    #[test]
    fn widths_come_from_each_panel_module() {
        assert_eq!(PanelId::Groom.spec().reference_width, crate::groom::REFERENCE_WIDTH);
        assert_eq!(PanelId::Groom.spec().min_width, crate::groom::MIN_WIDTH);
        assert_eq!(PanelId::Lumen.spec().min_width, crate::lumen::MIN_WIDTH);
        assert_eq!(PanelId::Library.spec().min_width, crate::library_panel::MIN_WIDTH);
        assert_eq!(PanelId::Font.spec().min_width, crate::font_panel::MIN_WIDTH);
        assert_eq!(PanelId::Model.spec().min_width, crate::model_panel::MIN_WIDTH);
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(PanelId::Dafont.spec().min_width, crate::dafont::MIN_WIDTH);
        for p in PanelId::ALL {
            let s = p.spec();
            assert!(s.min_width > 0.0 && s.min_width <= s.reference_width, "{p:?}");
        }
    }

    #[test]
    fn every_panel_has_one_host_and_the_fixed_ones_are_where_they_are_today() {
        assert!(matches!(PanelId::Tools.spec().host, Host::Left));
        assert!(matches!(PanelId::Color.spec().host, Host::Right));
        assert!(matches!(PanelId::Timeline.spec().host, Host::Bottom));
        assert!(matches!(PanelId::Preview.spec().host, Host::Window));
        assert!(matches!(PanelId::Font.spec().host, Host::Flyout(Body::Fixed)));
        assert!(matches!(PanelId::Artboards.spec().host, Host::Flyout(Body::Fit)));
        assert!(matches!(PanelId::Groom.spec().host, Host::Flyout(Body::Tall)));
        let mut seen = std::collections::HashSet::new();
        for p in PanelId::ALL {
            assert!(seen.insert(p.spec().title), "title repeated: {p:?}");
        }
    }
}
```

- [ ] **Step 2:** `cmd //c "start /low /b /wait cargo test -q -p cs-ui --lib panel_registry"` → no compila (el módulo no existe).

- [ ] **Step 3: Implementar.** Fichas (valores de hoy, de `dock.rs` y de cada módulo):

| Panel | title | icon | plain | reference_width | min_width | closable | host |
|---|---|---|---|---|---|---|---|
| Tools | "Tools" | "icon.tools" | "✎" | `pixel_ui::toolbar_width(None)` no sirve (depende del skin): usa la constante `pixel_ui::TOOLBAR_WIDTH_PLAIN` = 184 que hoy está escrita a mano en lib.rs (créala con ese valor y úsala también en lib.rs) | 184 | false | Left |
| Color | "Color" | "icon.color" | "🎨" | 220 (`color_panel::DEFAULT_WIDTH`, créala con el 220 de lib.rs y úsala allí) | 220 (por ahora; el mínimo real depende del skin y lo sigue calculando `color_panel::panel_min_size`) | false | Right |
| Timeline | `dock::TIMELINE` ("Timeline") | `dock::TIMELINE_ICON` | "🎞" | 190 (alto por defecto de hoy; para Bottom el "ancho" es el alto) | 190 | true | Bottom |
| Preview | `dock::PREVIEW` ("Preview") | `dock::PREVIEW_ICON` | "▣" | `preview::VIEW.x` | `preview::MIN_VIEW.x` | true | Window |
| Artboards | "Artboards" | "icon.dock.artboards" | "🗋" | `dock::FLYOUT_WIDTH` (210) | 210 | true | Flyout(Fit) |
| Slices | "Slices" | "icon.dock.slices" | "#" | 210 | 210 | true | Flyout(Fit) |
| Lospec | "Lospec" | "icon.dock.lospec" | "L" | `palette_ui::LOSPEC_PANEL_W` (364, hazla `pub(crate)`) | 364 | true | Flyout(Tall) |
| Dafont | "Dafont" | "icon.dock.dafont" | "Aa" | `dafont::REFERENCE_WIDTH` | `dafont::MIN_WIDTH` | true | Flyout(Tall) |
| Groom | "Groom" | "icon.dock.groom" | "▦" | `groom::REFERENCE_WIDTH` | `groom::MIN_WIDTH` | true | Flyout(Tall) |
| Lumen | "Lumen" | "icon.dock.lumen" | "☀" | `lumen::REFERENCE_WIDTH` | `lumen::MIN_WIDTH` | true | Flyout(Tall) |
| Library | "Library" | "icon.dock.library" | "▣" | `library_panel::REFERENCE_WIDTH` | `library_panel::MIN_WIDTH` | true | Flyout(Tall) |
| Font | "FontForge" | "icon.dock.font" | "FF" | `font_panel::REFERENCE_WIDTH` | `font_panel::MIN_WIDTH` | true | Flyout(Fixed) |
| Model | "3D" | "icon.dock.model" | "3D" | `model_panel::REFERENCE_WIDTH` | `model_panel::MIN_WIDTH` | true | Flyout(Fixed) |

(Íconos "icon.tools"/"icon.color" no tienen que existir en el skin todavía: los usará la fase C; anótalo en el doc de `icon`.)

Luego en `dock.rs`: borra el enum y `label/icon/plain` (o déjalos delegando a la ficha), `pub type DockPanel = crate::panel_registry::PanelId;`, `Body` se usa desde `panel_registry`, el `ALL` del dock (que recorre los botones) usa `PanelId::DOCK`, y `flyout(tall)` / `flyout_fixed` siguen igual por fuera. Quita los `expect(dead_code)` de los `MIN_WIDTH` que ahora se leen.

- [ ] **Step 4:** `cargo test -q -p cs-ui --lib panel_registry` → 3 passed; luego `cargo test -p cs-ui` completo → todo pasa sin tocar tests existentes.
- [ ] **Step 5:** clippy + fmt + commit `"Registro de paneles: PanelId con su ficha (título, ícono, anchos, dónde vive)"`.

---

### Task 2: `PanelAction`, `draw_panel` y `apply_panel_action` para los flyouts del dock

**Files:**
- Create: `crates/cs-ui/src/app_panels.rs` (`mod app_panels;` en lib.rs)
- Modify: `crates/cs-ui/src/lib.rs:536-857` (mudar `groom_panel`, `lumen_panel`, `font_panel`, `model_panel` (solo la versión flyout), `library_panel`, `dafont_panel`) y `:1897-1912` (el `match`); `crates/cs-ui/src/palette_ui.rs:330-406` (`lospec_panel`).
- Test: `mod tests` de `app_panels.rs` y los tests existentes.

**Interfaces:**
- Consumes: `PanelId`, `PanelSpec`, `Host`, `Body` (tarea 1).
- Produces:
  - `pub(crate) enum PanelAction { Groom(groom::GroomAction), Lumen(lumen::LumenAction), Font(font_panel::FontAction), Model(model_panel::ModelAction), Library(library_panel::LibraryAction), #[cfg(not(target_arch = "wasm32"))] Dafont(dafont::Picked), Lospec(palette_ui::LospecAction) }` (usa los tipos de acción reales; si alguno se llama distinto, usa el real).
  - `impl ChimpApp { pub(crate) fn draw_panel(&mut self, ui: &mut egui::Ui, id: PanelId) -> Option<PanelAction>; pub(crate) fn apply_panel_action(&mut self, ctx: &egui::Context, action: PanelAction); fn panel_flyout(&mut self, ctx: &egui::Context, id: PanelId, anchor: dock::Anchor, modal: bool) }`.

Reglas del corte (de cómo están hoy, ver lib.rs:536-857):
- `draw_panel` hace solo lo que hoy pasa **dentro** del closure del flyout (incluido lo que se prepara justo antes y no necesita `&mut self` entero, p. ej. el `Vec<PaletteChoice>` de Groom y el `primary`), con préstamos disjuntos de campos (`&mut self.groom`, `&mut self.doc.editor`, `self.pixel.skin()`…). Devuelve la acción que hoy se guarda en la variable local `action`.
- `apply_panel_action` hace lo que hoy pasa **después** del flyout (los `match` de `GroomAction`, `LumenAction`, … tal cual, incluido el `return` temprano de Lumen convertido en `return` de la rama).
- `panel_flyout` reemplaza a los seis `x_panel`: elige `dock::flyout(ctx, id, anchor, skin, spec.reference_width, tall)` o `dock::flyout_fixed(…)` según `spec.host`, envuelve en `ui.add_enabled_ui(!modal, …)` como hoy, llama `draw_panel` y después `apply_panel_action`.
- Lospec: el "tomar `self.lospec_dock` / crear y buscar / poner de vuelta" y el `lospec_band` fuera del scope habilitado se quedan dentro de su rama de `draw_panel`/`panel_flyout` exactamente en el mismo orden; necesita `anchor.column.bottom()`: pásalo con un campo temporal `self.flyout_bottom: Option<f32>` que `panel_flyout` pone antes de dibujar, o con un parámetro extra privado solo para Lospec (elige lo más simple y coméntalo).
- El `match dock_out.app_panel` de lib.rs:1897 queda `if let Some((id, anchor)) = dock_out.app_panel { self.panel_flyout(ui.ctx(), id, anchor, modal) }` y lo de `close_lospec_panel` sigue igual.

- [ ] **Step 1: Test que falla** — en `app_panels.rs`, un test que arma un `ChimpApp` de prueba (cópialo de `layout_tests.rs`/`dialog_tests.rs`: `ChimpApp::with_parts(Default::default(), pixel_ui(), PaletteLibrary::new(Box::new(NoPresets)))` + `new_document()`) y, para cada `id` en `PanelId::DOCK` con `Host::Flyout`, corre un frame con `draw_panel(ui, id)` dentro de un `CentralPanel`, sin pánico y devolviendo `None` sin interacción:

```rust
#[test]
fn every_dock_panel_draws_through_the_registry() {
    let mut app = test_app();
    let ctx = egui::Context::default();
    for &id in crate::panel_registry::PanelId::DOCK {
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                assert!(app.draw_panel(ui, id).is_none(), "{id:?}");
            });
        });
    }
}
```

- [ ] **Step 2:** correr → no compila (`draw_panel` no existe).
- [ ] **Step 3:** implementar el corte con las reglas de arriba, un panel a la vez, corriendo los tests del panel después de cada uno (`--lib groom`, `lumen`, `font`, `model`, `library`, `dafont`, `lospec`/`palette_ui`).
- [ ] **Step 4:** `cargo test -p cs-ui` completo → todo pasa sin tocar tests existentes. Captura con `CHIMPSPRITE_DEMO_GROOM=empty`, `CHIMPSPRITE_DEMO_LIBRARY` (o la variable que abre Library en demo.rs), `CHIMPSPRITE_DEMO_LOSPEC` y `CHIMPSPRITE_DEMO_FONT=1` antes/después (build release en `target/gui-b2`, respaldo/restauración de app.ron con tu nombre): 0 pixeles distintos.
- [ ] **Step 5:** clippy + fmt + commit `"Registro de paneles: los flyouts del dock se dibujan y aplican por draw_panel / apply_panel_action"`.

---

### Task 3: Artboards y Slices por el registro

**Files:** Modify `crates/cs-ui/src/dock.rs:243-306` (`show`: dejar de dibujar Artboards/Slices adentro; devolverlos en `app_panel` como los demás), `crates/cs-ui/src/app_panels.rs` (ramas nuevas), `crates/cs-ui/src/lib.rs:1833,1874` (el `canvas_size` de `DockOutput`).

**Interfaces:** Produces `PanelAction::OpenCanvasSize` (lo que hoy dispara `out.canvas_size` → `self.open_canvas_size()`).

- [ ] **Step 1: Test que falla** — en `app_panels.rs`: `draw_panel(ui, PanelId::Artboards)` y `draw_panel(ui, PanelId::Slices)` con un editor de prueba no entran en pánico, y `apply_panel_action(ctx, PanelAction::OpenCanvasSize)` deja el diálogo de Canvas Size abierto (el mismo estado que verifica hoy el test que cubre `open_canvas_size`; búscalo con `grep -n "open_canvas_size\|Canvas Size" crates/cs-ui/src/*tests*.rs`).
- [ ] **Step 2:** correr → falla.
- [ ] **Step 3:** mover las llamadas `panels::pixel_artboards` / `panels::artboards` / `pixel_slices` / `slices` de `dock::show` a `draw_panel` (con `&mut self.doc.editor`, `&mut self.doc.artboard_name`, `skin`), conservando el `add_enabled_ui(enabled)` con `enabled = ui.is_enabled()` del dock (hoy dock.rs:284) — `panel_flyout` lo recibe por `modal`, revisa que sea el mismo valor —, y `DockOutput.canvas_size` desaparece (la acción la lleva `PanelAction::OpenCanvasSize`). `dock::show` ya no necesita `&mut Editor` ni `&mut (ArtboardId, String)` si nada más los usa: quítalos de su firma.
- [ ] **Step 4:** `cargo test -p cs-ui` completo sin tocar tests existentes; captura `CHIMPSPRITE_DEMO_ARTBOARDS=2` antes/después con 0 pixeles distintos.
- [ ] **Step 5:** clippy + fmt + commit `"Registro de paneles: Artboards y Slices también por draw_panel"`.

---

### Task 4: Las regiones fijas (Tools, Color, Timeline, panel del modelo) y el contenido de Preview

**Files:** Modify `crates/cs-ui/src/lib.rs:1636-1891` (`editor_panels`), `crates/cs-ui/src/app_panels.rs`, `crates/cs-ui/src/preview.rs:82` (`Preview::show`).

**Interfaces:** Produces `PanelAction::Palette(panels::PaletteAction)`, `PanelAction::InstanceZoom` (lo que hoy hace `panels::instance` → `doc.canvas.request_zoom`), `PanelAction::NewTilemap` si aplica (`doc.timeline.new_tilemap`), `PanelAction::ClosePreview`; y `Preview::contents(&mut self, ui: &mut egui::Ui, editor: &Editor, skin: Option<&Skin>, canvas: Option<(egui::TextureId, egui::Vec2)>)` (lo de adentro de la ventana) llamado por `Preview::show`.

- [ ] **Step 1: Test que falla** — en `app_panels.rs`: `draw_panel` para `Tools`, `Color`, `Timeline` y `Preview` no entra en pánico con el app de prueba, y para `Model` en una pestaña de modelo (abre una como lo hacen los tests de ChimpCAD, p. ej. `opening_a_model_view_shows_the_3d_panel_without_the_dock` en layout_tests.rs) tampoco.
- [ ] **Step 2:** correr → falla (`draw_panel` aún no conoce esos ids).
- [ ] **Step 3:** en `editor_panels`, cada `Panel::left("tools")`, `Panel::right("palette")`, `Panel::bottom("timeline")` y `Panel::right("model_panel")` se queda igual (ids, tamaños, frames, el apretón de manos de `palette_width_settled`, el guardado del ancho, la línea grabada) pero su **contenido** pasa a `self.draw_panel(ui, PanelId::X)` y la acción se aplica donde hoy (Color: donde está `palette_action` en 1871; Model: donde está 1877). `Preview::show` arma la ventana como hoy y llama a `contents` adentro; `draw_panel(ui, Preview)` llama a `contents`. Las fichas `Host::Left/Right/Bottom/Window` no cambian a nadie de lugar en esta fase.
- [ ] **Step 4:** `cargo test -p cs-ui` completo sin tocar tests existentes; capturas `--demo` (lienzo con toolbar, color, timeline, preview abierto: `CHIMPSPRITE_DEMO` por defecto + `Preview` encendido como lo hacen los tests de Preview) y de una pestaña de modelo (`CHIMPSPRITE_DEMO_MODEL`) antes/después: 0 pixeles distintos.
- [ ] **Step 5:** clippy + fmt + commit `"Registro de paneles: toolbar, color, timeline, modelo y preview por draw_panel"`.

---

### Task 5: Cierre de la fase B

**Files:** Modify `docs/superpowers/specs/2026-11-09-workspace-docking-design.md` (fase B hecha: 3-5 líneas con `PanelId`/`PanelSpec`/`draw_panel`/`apply_panel_action` y lo que queda para C), `docs/forms.md` no cambia.

- [ ] **Step 1:** `grep -rn "fn groom_panel\|fn lumen_panel\|fn font_panel\|fn model_panel\|fn library_panel\|fn dafont_panel\|fn lospec_panel" crates/cs-ui/src` → vacío (todo vive en `app_panels.rs`).
- [ ] **Step 2:** Verificación completa como CI: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check --target wasm32-unknown-unknown -p cs-core -p cs-3d -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite` (Dafont fuera de wasm).
- [ ] **Step 3:** actualizar el spec; commit `"Fase B del workspace: registro de paneles (docs)"`.
