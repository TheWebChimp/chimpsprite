# Outline como el de Aseprite — Implementation Plan

**Goal:** Edit → FX → Outline con Background Color, presets + matriz 3×3, Place, canales, Target,
Preview, Tiled y OK / Apply / Cancel, igual píxel a píxel que Aseprite 1.3.

**Architecture:** la lógica pura en `cs_raster::outline` (máscara del borde a partir de una
función "vacío", sin egui); `cs-ui/filters.rs` la usa sobre cada cel del Target y lo junta en un
`Command::Batch` (un paso de undo); `filter_dialog.rs` arma el diálogo con el kit.

Spec: `docs/superpowers/specs/2026-11-06-outline-fx-design.md`.

## Global Constraints

- Comentarios en español; textos de la UI en inglés. Repo `eol=lf`.
- Nada de `git stash` / `checkout --` / `restore` / `reset --hard` / `clean` / `add -A`.
- Cargo con el toolchain aislado, `CARGO_TARGET_DIR=D:/Work/chimpdeas/cs-font/target`, a baja
  prioridad (`cmd //c "start /low /b /wait cargo …"`).
- Antes de cada commit: fmt, tests y clippy `-D warnings` de los crates tocados.

---

### Task 1: `cs_raster::outline` + fixtures de Aseprite

**Files:** create `crates/cs-raster/src/outline.rs`, `crates/cs-raster/tests/outline_aseprite.rs`,
`crates/cs-raster/tests/fixtures/outline_aseprite.{lua,txt}`; modify `crates/cs-raster/src/lib.rs`.

- [ ] Test: `OutlineMatrix` (bits de Aseprite, centro apagado, `toggled`, `preset`).
- [ ] Test: `outline_mask` en un punto (Outside circle / square, Inside), bordes recortados y
      envueltos.
- [ ] Test: `paint_channels`.
- [ ] Test de integración: leer el fixture y comparar todos los casos.
- [ ] Implementar; `Channels` se mueve a cs-raster.
- [ ] Commit.

### Task 2: el filtro en el editor (`OutlineFx`, Target)

**Files:** modify `crates/cs-ui/src/filters.rs`, `app_menus.rs`, `demo.rs` y los usos de
`Filter::Outline`.

- [ ] Tests: outside/inside con bg, canales, selección, borde recortado, Target Selected (rango
      de cels) y All (salta capas con candado), un solo paso de undo.
- [ ] Implementar `OutlineFx`, `FilterTarget`, `Editor::apply_filter_to`.
- [ ] Commit.

### Task 3: el diálogo

**Files:** modify `crates/cs-ui/src/filter_dialog.rs`, `lib.rs` (recordar los valores),
`demo.rs` (`CHIMPSPRITE_DEMO_FILTER=outline`).

- [ ] Tests (egui headless): preset → matriz; tocar celda → custom; Apply deja un paso y no
      re-previsualiza; Apply dos veces sin cambios = dos pasos; Preview apagado no toca el lienzo
      hasta OK; Cancel tras Apply deja lo aplicado.
- [ ] Implementar el layout con `kit::color_button`, `kit::group`, botones del kit para la
      matriz, radios, casillas y el select del Target.
- [ ] Commit.

### Task 4: verificación

- [ ] `cargo test --workspace`, clippy, fmt, wasm check.
- [ ] Build release en `target/gui-N`, captura con `CHIMPSPRITE_DEMO_FILTER=outline`, revisarla.
- [ ] Autorrevisión del diff; minors en `.superpowers/sdd/outline-minors.md`; sección en
      `.superpowers/sdd/progress.md`.
