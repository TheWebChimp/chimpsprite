# Movie clips parte 1 (F8 + biblioteca): plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convertir capas o píxeles en un símbolo con F8, guardar los símbolos en una biblioteca
oculta y editarlos en un modo símbolo aislado.

**Architecture:** El símbolo sigue siendo un `Artboard`, con `in_library: true`.

- **Motor** (`cs-core` y `cs-commands`): la marca, una primitiva con undo y
  `symbol::convert_to_symbol`, que arma un solo `DocCommand::Batch`.
- **Fuera del motor:** `cs-io` guarda la marca, `cs-agent` la expone y `cs-ui` agrega F8, el panel
  Library, el modo símbolo y el filtro de artboards de biblioteca en el lienzo.

**Tech Stack:** Rust 2024, egui 0.36 / eframe, serde, proptest. Pruebas con `cargo test`.

**Spec:** `docs/superpowers/specs/2026-10-30-symbol-library-design.md`. Las secciones "Ajustes
al spec" de este plan mandan sobre el spec.

## Global Constraints

- Toolchain: antes de cualquier `cargo`, en Git Bash:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH`.
- Builds de prueba de la app: `CARGO_TARGET_DIR=target/gui-check22` (y de ahí en adelante, uno
  nuevo por build). Nunca matar la app del usuario.
- CI corre `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` y `cargo check --target wasm32-unknown-unknown -p cs-core -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite`.
  Todo debe pasar.
- Los crates del motor no dependen de egui y deben compilar para wasm32.
- Comentarios y docs en español, en el estilo del código vecino (`///` en español, frases cortas).
  Los textos de la UI van en inglés.
- El repo usa `eol=lf`: si un script de edición deja CRLF, quitarlo con `sed -i 's/\r$//'` antes de
  commitear.
- Mensajes de commit en español, terminados en
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Commit sin preguntar y sin push.
- No tocar `.design/Design.aseprite` ni `.design/chimpfont.ttf`: son trabajo en curso del usuario.
  Nunca usar `git stash`.

## Ajustes al spec (descubiertos al planear)

1. **"Capas seleccionadas" = `Editor::range_layers()`.** El panel de capas no tiene
   multi-selección. `range_layers()` (`crates/cs-ui/src/anim_ops.rs:441`) devuelve las capas del
   rango del timeline o, si no hay rango, la capa actual.
2. **Recorte de capas de instancia.** `cs-commands` no puede depender de `cs-render` (solo como
   dev-dependency), así que para una capa de instancia se usa el rectángulo completo del sprite
   fuente en cada key (`position` + tamaño), no la caja opaca. El render sigue siendo idéntico.
3. **Con marquesina solo se cortan capas de imagen.** Las capas de grupo o de instancia que estén
   en el rango se ignoran en el modo píxeles.
4. **"Move to Library" va en el menú "≡" del artboard en el lienzo** (`CanvasView::artboard_menu`,
   `crates/cs-ui/src/canvas.rs:1368`). El panel de Artboards no tiene menú contextual.
5. **Errores de F8:** se muestran con `ChimpApp::show_message("Convert to Symbol", …)`. No hay un
   API para mensajes en la barra de estado.
6. **La biblioteca es un `app_panel` del dock** (como Groom y Lumen), porque necesita la vista del
   lienzo para entrar al modo símbolo.

## File Structure

| Archivo | Responsabilidad |
|---|---|
| `crates/cs-core/src/document.rs` (modificar) | `Artboard::in_library`, `canvas_artboards`, `library`, `instance_uses`, filtro en `bounds`/`artboard_at` |
| `crates/cs-commands/src/doc_command.rs` (modificar) | `DocCommand::SetArtboardInLibrary`, guardas de "último artboard del lienzo" |
| `crates/cs-commands/src/symbol.rs` (crear) | `SymbolSource`, `SymbolTiming`, `ConvertError`, `Converted`, `convert_to_symbol`, `move_to_library`, `place_on_canvas` |
| `crates/cs-commands/src/symbol/tests.rs` (crear) | pruebas de conversión (render idéntico) |
| `crates/cs-commands/src/lib.rs` (modificar) | `pub mod symbol;` |
| `crates/cs-io/src/chimp.rs` (modificar) | `Entry::library` |
| `crates/cs-io/src/export.rs` (modificar) | el atlas usa solo `canvas_artboards()` |
| `crates/cs-tools/src/artboard.rs` (modificar) | `hit_test` ignora la biblioteca |
| `crates/cs-agent/src/{params.rs,dispatch.rs,ops/structure.rs,ops/document.rs}` + `apps/chimpsprite-mcp/src/server.rs` (modificar) | `convert_to_symbol`, `move_to_library`, `place_on_canvas`, `in_library` en `describe` |
| `crates/cs-ui/src/symbol_ops.rs` (crear) | métodos de `Editor`: `convert_to_symbol`, `move_active_to_library`, `place_on_canvas`, `enter_symbol`, `exit_symbol_to`, `symbol_stack`, nombres libres |
| `crates/cs-ui/src/symbol_dialog.rs` (crear) | diálogo "Convert to Symbol" |
| `crates/cs-ui/src/library_panel.rs` (crear) | panel Library (vista previa, lista, menú contextual, arrastrar) |
| `crates/cs-ui/src/canvas.rs`, `canvas/input.rs`, `editor.rs`, `editor/gestures.rs`, `editor/document_ops.rs`, `panels/inspectors.rs` (modificar) | filtro de biblioteca, modo símbolo, migaja de pan, soltar desde la biblioteca, Move to Library |
| `crates/cs-ui/src/{keymap.rs,app_actions.rs,app_menus.rs,timeline.rs,lib.rs,file_actions.rs,dock.rs,dialog_tests.rs}` (modificar) | acción F8, menús, diálogo, panel |

---

### Task 1: La marca `in_library` en el modelo

**Files:**
- Modify: `crates/cs-core/src/document.rs` (struct `Artboard` en :129, `with_sprite` :211, `artboard_at` :255, `bounds` :264, tests :323 y :363)
- Modify: los demás literales de `Artboard { … }` (agregar `in_library: false`):
  - `crates/cs-io/src/chimp.rs:236` y `:421`
  - `crates/cs-io/src/files.rs:176` y `:207`
  - `crates/cs-io/src/export.rs:684`
  - `crates/cs-render/src/materialize.rs:372`
  - `crates/cs-commands/src/doc_command.rs:218` y `:249`
  - `crates/cs-commands/tests/doc_undo_roundtrip.rs:62`
  - `crates/cs-agent/src/ops/structure.rs:59`
  - `crates/cs-agent/src/target.rs:192`
  - `crates/cs-tools/src/artboard.rs:170`
  - `crates/cs-ui/src/editor/document_ops.rs:14` y `:179`

  `document_ops.rs:157` usa `..self.artboard().clone()`: ahí poner `in_library: false` explícito,
  para que duplicar desde la biblioteca deje la copia en el lienzo.

**Interfaces:**
- Produces:
  - `Artboard.in_library: bool`
  - `Document::canvas_artboards(&self) -> impl DoubleEndedIterator<Item = &Artboard>`
  - `Document::library(&self) -> impl Iterator<Item = &Artboard>`
  - `Document::instance_uses(&self, id: ArtboardId) -> usize`
  - `Document::canvas_count(&self) -> usize`
  - `bounds()` y `artboard_at()` ignoran la biblioteca.

- [ ] **Step 1: Pruebas que fallan.** En el `mod tests` de `crates/cs-core/src/document.rs`:

```rust
fn library_doc() -> Document {
    let mut d = Document::with_sprite("a", Sprite::new(4, 4, ColorMode::Rgba));
    d.artboards.push(Artboard {
        id: ArtboardId(2),
        name: "sym".into(),
        position: (100, 100),
        sprite: Sprite::new(8, 8, ColorMode::Rgba),
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library: true,
    });
    d
}

#[test]
fn the_library_is_not_on_the_canvas() {
    let d = library_doc();
    let canvas: Vec<_> = d.canvas_artboards().map(|a| a.id).collect();
    let library: Vec<_> = d.library().map(|a| a.id).collect();
    assert_eq!(canvas, vec![ArtboardId(1)]);
    assert_eq!(library, vec![ArtboardId(2)]);
    assert_eq!(d.canvas_count(), 1);
    assert_eq!(d.bounds(), Some(((0, 0), (4, 4))));
    assert_eq!(d.artboard_at((101.0, 101.0)), None);
    assert_eq!(d.artboard_at((1.0, 1.0)), Some(ArtboardId(1)));
}

#[test]
fn instance_uses_counts_layers_not_keys() {
    let mut d = library_doc();
    let sprite = &mut d.artboards[0].sprite;
    for id in [10, 11] {
        let mut layer = Layer::new_image(LayerId(id), "i");
        layer.kind = LayerKind::Instance;
        let mut track = InstanceTrack { keys: vec![InstanceKey::new(0, ArtboardId(2), (0, 0))] };
        track.set_key(InstanceKey::new(3, ArtboardId(2), (1, 1)));
        layer.instance = Some(track);
        sprite.layers.push(layer);
    }
    assert_eq!(d.instance_uses(ArtboardId(2)), 2);
    assert_eq!(d.instance_uses(ArtboardId(1)), 0);
}

#[test]
fn old_documents_load_everything_on_the_canvas() {
    let mut json = serde_json::to_value(library_doc()).unwrap();
    json["artboards"][1].as_object_mut().unwrap().remove("in_library");
    let d: Document = serde_json::from_value(json).unwrap();
    assert!(!d.artboards[1].in_library);
}
```

  (Si el `mod tests` no importa `Layer`, `LayerKind`, `InstanceKey`, `InstanceTrack`, `LayerId` o
  `ColorMode`, agregarlos al `use`. Si `serde_json` no es dev-dependency de `cs-core`, agregarlo en
  `crates/cs-core/Cargo.toml` como `serde_json.workspace = true` en `[dev-dependencies]`.)

- [ ] **Step 2: Ver que fallan.** `cargo test -p cs-core the_library` → error de compilación (falta
  `in_library`).

- [ ] **Step 3: Implementar.** En `Artboard`, después de `symmetry`:

```rust
    /// En la biblioteca: no se dibuja en el lienzo ni sale en el panel de Artboards; es la fuente
    /// de instancias y se edita en modo símbolo.
    #[serde(default)]
    pub in_library: bool,
```

  En `impl Document`:

```rust
    /// Los artboards del lienzo (todos menos los de la biblioteca), en z-order.
    pub fn canvas_artboards(&self) -> impl DoubleEndedIterator<Item = &Artboard> {
        self.artboards.iter().filter(|a| !a.in_library)
    }

    /// Los símbolos de la biblioteca, en el orden del documento.
    pub fn library(&self) -> impl Iterator<Item = &Artboard> {
        self.artboards.iter().filter(|a| a.in_library)
    }

    /// Cuántos artboards hay en el lienzo.
    pub fn canvas_count(&self) -> usize {
        self.canvas_artboards().count()
    }

    /// Cuántas capas de instancia, en todo el documento, muestran `id` en alguna key.
    pub fn instance_uses(&self, id: ArtboardId) -> usize {
        self.artboards
            .iter()
            .flat_map(|a| a.sprite.layers.iter())
            .filter_map(|l| l.instance.as_ref())
            .filter(|t| t.keys.iter().any(|k| k.source == Some(id)))
            .count()
    }
```

  En `artboard_at` y `bounds`, cambiar `self.artboards.iter()` por `self.canvas_artboards()`. En
  `with_sprite` y en los literales de la lista de Files, agregar `in_library: false`.

- [ ] **Step 4: Ver que pasan.** `cargo test --workspace` → todo verde. (Los literales de otros
  crates tienen que compilar.)

- [ ] **Step 5: Commit.** `git add -A crates && git commit -m "Símbolos: marca in_library en los artboards; bounds y artboard_at ignoran la biblioteca"`

---

### Task 2: Mover a la biblioteca y sacar al lienzo (comandos)

**Files:**
- Modify: `crates/cs-commands/src/doc_command.rs` (enum :8, `apply` :49, `label` ~:190, tests)
- Create: `crates/cs-commands/src/symbol.rs` (con `move_to_library` y `place_on_canvas`; la conversión llega en la Task 3)
- Modify: `crates/cs-commands/src/lib.rs` (`pub mod symbol;`)

**Interfaces:**
- Consumes: `Document::canvas_count`, `canvas_artboards`, `bounds` (Task 1).
- Produces:
  - `DocCommand::SetArtboardInLibrary { id: ArtboardId, in_library: bool }`, con label "Library".
  - `RemoveArtboard` y `SetArtboardInLibrary { in_library: true }` devuelven
    `CmdError::LastArtboard` si dejarían el lienzo sin artboards.
  - `symbol::move_to_library(doc: &Document, id: ArtboardId) -> Option<DocCommand>`
  - `symbol::place_on_canvas(doc: &Document, id: ArtboardId) -> Option<DocCommand>`
  - `symbol::free_position(doc: &Document) -> (i32, i32)`

- [ ] **Step 1: Pruebas que fallan.** En `crates/cs-commands/src/symbol/tests.rs` (crear; en
  `symbol.rs` poner `#[cfg(test)] mod tests;`):

```rust
use cs_core::*;

use super::*;

/// Documento con un artboard de 8×8 en el lienzo (id 1).
fn doc() -> Document {
    Document::with_sprite("host", Sprite::new(8, 8, ColorMode::Rgba))
}

fn add(doc: &mut Document, id: u32, position: (i32, i32), in_library: bool) {
    doc.artboards.push(Artboard {
        id: ArtboardId(id),
        name: format!("a{id}"),
        position,
        sprite: Sprite::new(4, 4, ColorMode::Rgba),
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library,
    });
}

#[test]
fn move_to_library_and_back_with_undo() {
    let mut d = doc();
    add(&mut d, 2, (20, 0), false);
    let cmd = move_to_library(&d, ArtboardId(2)).unwrap();
    let undo = cmd.apply(&mut d).unwrap();
    assert!(d.artboard(ArtboardId(2)).unwrap().in_library);
    undo.apply(&mut d).unwrap();
    assert!(!d.artboard(ArtboardId(2)).unwrap().in_library);
}

#[test]
fn the_last_canvas_artboard_cannot_leave() {
    let mut d = doc();
    add(&mut d, 2, (20, 0), true);
    assert_eq!(move_to_library(&d, ArtboardId(1)), None);
    let forced = DocCommand::SetArtboardInLibrary { id: ArtboardId(1), in_library: true };
    assert_eq!(forced.apply(&mut d), Err(CmdError::LastArtboard));
    let remove = DocCommand::RemoveArtboard { id: ArtboardId(1) };
    assert_eq!(remove.apply(&mut d), Err(CmdError::LastArtboard));
    // Un símbolo sí se puede borrar.
    DocCommand::RemoveArtboard { id: ArtboardId(2) }.apply(&mut d).unwrap();
}

#[test]
fn place_on_canvas_keeps_its_spot_or_goes_to_the_right() {
    let mut d = doc();
    add(&mut d, 2, (40, 0), true);
    let cmd = place_on_canvas(&d, ArtboardId(2)).unwrap();
    cmd.apply(&mut d).unwrap();
    assert_eq!(d.artboard(ArtboardId(2)).unwrap().position, (40, 0));

    let mut d = doc();
    add(&mut d, 2, (2, 2), true); // choca con el host (0,0)-(8,8)
    place_on_canvas(&d, ArtboardId(2)).unwrap().apply(&mut d).unwrap();
    assert_eq!(d.artboard(ArtboardId(2)).unwrap().position, (8 + 16, 0));
    assert!(!d.artboard(ArtboardId(2)).unwrap().in_library);
}
```

- [ ] **Step 2: Ver que fallan.** `cargo test -p cs-commands symbol::` → no compila.

- [ ] **Step 3: Implementar el comando.** En `DocCommand` agregar:

```rust
    /// Manda un artboard a la biblioteca (`true`) o lo saca al lienzo (`false`).
    SetArtboardInLibrary {
        id: ArtboardId,
        in_library: bool,
    },
```

  En `apply`:

```rust
            DocCommand::SetArtboardInLibrary { id, in_library } => {
                let artboard = doc.artboard(*id).ok_or(CmdError::ArtboardNotFound(*id))?;
                let old = artboard.in_library;
                if *in_library && !old && doc.canvas_count() == 1 {
                    return Err(CmdError::LastArtboard);
                }
                doc.artboard_mut(*id).expect("checked").in_library = *in_library;
                Ok(DocCommand::SetArtboardInLibrary { id: *id, in_library: old })
            }
```

  En `RemoveArtboard`, reemplazar el `if doc.artboards.len() == 1` por:

```rust
                let on_canvas = !doc.artboards[index].in_library;
                if doc.artboards.len() == 1 || (on_canvas && doc.canvas_count() == 1) {
                    return Err(CmdError::LastArtboard);
                }
```

  En `label`: `DocCommand::SetArtboardInLibrary { .. } => "Library",`.

- [ ] **Step 4: Implementar `symbol.rs`:**

```rust
//! Símbolos de la biblioteca: convertir capas o píxeles en un símbolo (F8), mandar artboards a
//! la biblioteca y sacarlos al lienzo.

use cs_core::{Artboard, ArtboardId, Document};

use crate::DocCommand;

#[cfg(test)]
mod tests;

/// Separación entre un artboard nuevo y los demás, como en `Editor::add_artboard`.
const GAP: i32 = 16;

/// Lugar libre para un artboard nuevo en el lienzo: a la derecha de todos.
pub fn free_position(doc: &Document) -> (i32, i32) {
    match doc.bounds() {
        Some(((_, y0), (x1, _))) => (x1 + GAP, y0),
        None => (0, 0),
    }
}

/// Manda `id` a la biblioteca. `None` si ya está ahí o si es el último artboard del lienzo.
pub fn move_to_library(doc: &Document, id: ArtboardId) -> Option<DocCommand> {
    let artboard = doc.artboard(id)?;
    (!artboard.in_library && doc.canvas_count() > 1)
        .then_some(DocCommand::SetArtboardInLibrary { id, in_library: true })
}

/// Saca `id` de la biblioteca a su lugar o, si choca con otro artboard del lienzo, a la derecha
/// de todos.
pub fn place_on_canvas(doc: &Document, id: ArtboardId) -> Option<DocCommand> {
    let artboard = doc.artboard(id)?;
    if !artboard.in_library {
        return None;
    }
    let show = DocCommand::SetArtboardInLibrary { id, in_library: false };
    if !collides(doc, artboard) {
        return Some(show);
    }
    Some(DocCommand::Batch(vec![
        DocCommand::SetArtboardProps { id, name: None, position: Some(free_position(doc)) },
        show,
    ]))
}

/// ¿El rectángulo de `artboard` se encima con el de algún artboard del lienzo?
fn collides(doc: &Document, artboard: &Artboard) -> bool {
    let ((ax0, ay0), (ax1, ay1)) = artboard.rect();
    doc.canvas_artboards().any(|other| {
        let ((bx0, by0), (bx1, by1)) = other.rect();
        ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1
    })
}
```

  (`Artboard::rect()` ya existe y devuelve `((x0, y0), (x1, y1))` con el máximo exclusivo; la usa
  `Document::bounds`.) En `crates/cs-commands/src/lib.rs` agregar `pub mod symbol;`.

- [ ] **Step 5: Ver que pasan.** `cargo test -p cs-commands` → verde.

- [ ] **Step 6: Commit.** `git commit -am "Símbolos: comando SetArtboardInLibrary, mover a la biblioteca y sacar al lienzo"` (agregar antes `git add crates/cs-commands/src/symbol.rs crates/cs-commands/src/symbol`).

---

### Task 3: Convertir capas en símbolo

**Files:**
- Modify: `crates/cs-commands/src/symbol.rs`
- Modify: `crates/cs-commands/src/symbol/tests.rs`

**Interfaces:**
- Consumes: `free_position` (Task 2), `ops::new_instance_layer`, `ops::remove_layer_tree`
  (`crates/cs-commands/src/ops.rs:42`, `:257`).
- Produces:

```rust
pub enum SymbolSource { Layers(Vec<LayerId>), Pixels { layers: Vec<LayerId>, selection: Mask } }
pub enum SymbolTiming { Graphic, MovieClip }      // + fn timing(self) -> Timing
pub enum ConvertError { NotRgba, Tilemap, Empty, NoLayers } // thiserror, Display en inglés
pub struct Converted { pub command: DocCommand, pub symbol: ArtboardId, pub instance: LayerId }
pub fn convert_to_symbol(doc: &Document, host: ArtboardId, source: &SymbolSource,
                         name: &str, timing: SymbolTiming) -> Result<Converted, ConvertError>
```

- [ ] **Step 1: Pruebas que fallan** (en `symbol/tests.rs`):

```rust
const RED: PixelValue = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
const BLUE: PixelValue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));

/// Host de 8×8 con 3 frames de duraciones desiguales. Capa 1 (abajo): un píxel rojo que se
/// mueve (2,3) → (3,3) → (4,4). Capa 2 (arriba): un píxel azul fijo en (6,6).
fn animated() -> Document {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    s.frames[0].duration_ms = 100;
    for (index, ms) in [(1, 50), (2, 200)] {
        crate::Command::AddFrame { index, frame: Frame { duration_ms: ms } }.apply(&mut s).unwrap();
    }
    crate::ops::new_layer(&s.clone(), Some(LayerId(1)), LayerKind::Image).apply(&mut s).unwrap();
    for (frame, (x, y)) in [(0, (2, 3)), (1, (3, 3)), (2, (4, 4))] {
        crate::Command::DrawPixels { layer: LayerId(1), frame, pixels: vec![(x, y, RED)] }
            .apply(&mut s)
            .unwrap();
    }
    for frame in 0..3 {
        crate::Command::DrawPixels { layer: LayerId(2), frame, pixels: vec![(6, 6, BLUE)] }
            .apply(&mut s)
            .unwrap();
    }
    Document::with_sprite("host", s)
}

/// Cada frame del host, materializado y renderizado.
fn rendered(doc: &Document, id: ArtboardId) -> Vec<Vec<Rgba>> {
    let s = &doc.artboard(id).unwrap().sprite;
    (0..s.frames.len())
        .map(|f| cs_render::render_frame(&cs_render::materialize(doc, s, f..f + 1), f))
        .collect()
}

#[test]
fn converting_layers_looks_the_same_in_both_timings() {
    for timing in [SymbolTiming::Graphic, SymbolTiming::MovieClip] {
        let mut d = animated();
        let before = rendered(&d, ArtboardId(1));
        let source = SymbolSource::Layers(vec![LayerId(1)]);
        let c = convert_to_symbol(&d, ArtboardId(1), &source, "Dot", timing).unwrap();
        let undo = c.command.apply(&mut d).unwrap();
        assert_eq!(rendered(&d, ArtboardId(1)), before, "{timing:?}");

        let symbol = d.artboard(c.symbol).unwrap();
        assert!(symbol.in_library);
        assert_eq!(symbol.name, "Dot");
        // La unión de (2,3), (3,3) y (4,4): 3×2 desde (2,3).
        assert_eq!((symbol.sprite.width, symbol.sprite.height), (3, 2));
        assert_eq!(symbol.sprite.frames, d.artboard(ArtboardId(1)).unwrap().sprite.frames);

        let host = &d.artboard(ArtboardId(1)).unwrap().sprite;
        assert!(host.layer(LayerId(1)).is_none(), "the layer moved into the symbol");
        let layer = host.layer(c.instance).unwrap();
        assert_eq!((layer.kind, layer.name.as_str()), (LayerKind::Instance, "Dot"));
        let key = layer.instance.as_ref().unwrap().keys[0];
        assert_eq!((key.frame, key.source, key.position), (0, Some(c.symbol), (2, 3)));
        assert_eq!(key.timing, timing.timing());
        // La instancia queda donde estaba la capa: debajo de la capa 2.
        assert!(host.layer_index(c.instance) < host.layer_index(LayerId(2)));

        undo.apply(&mut d).unwrap();
        assert_eq!(d, animated(), "one undo restores everything");
    }
}

#[test]
fn groups_keep_their_structure_and_empty_ones_go_away() {
    let mut d = animated();
    let s = &mut d.artboards[0].sprite;
    // Grupo 3 con la capa 1 adentro.
    crate::ops::new_layer(&s.clone(), Some(LayerId(2)), LayerKind::Group).apply(s).unwrap();
    crate::ops::move_layer(&s.clone(), LayerId(1), crate::ops::LayerDrop::Into(LayerId(3)))
        .unwrap()
        .apply(s)
        .unwrap();
    let before = rendered(&d, ArtboardId(1));
    // Convertir solo la capa 1: el grupo 3 queda vacío en el host y se borra.
    let c = convert_to_symbol(
        &d, ArtboardId(1), &SymbolSource::Layers(vec![LayerId(1)]), "Dot", SymbolTiming::Graphic,
    )
    .unwrap();
    c.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);
    let symbol = &d.artboard(c.symbol).unwrap().sprite;
    assert_eq!(symbol.layer(LayerId(1)).unwrap().parent, Some(LayerId(3)));
    assert_eq!(symbol.layer(LayerId(3)).unwrap().kind, LayerKind::Group);
    // En el host, la instancia ocupa el lugar del grupo, que ya no existe.
    let host = &d.artboard(ArtboardId(1)).unwrap().sprite;
    assert!(host.layer(LayerId(3)).is_none());
    assert_eq!(host.layer(c.instance).unwrap().parent, None);
}

#[test]
fn converting_an_instance_layer_nests_and_looks_the_same() {
    let mut d = animated();
    let first = convert_to_symbol(
        &d, ArtboardId(1), &SymbolSource::Layers(vec![LayerId(1)]), "Dot", SymbolTiming::Graphic,
    )
    .unwrap();
    first.command.apply(&mut d).unwrap();
    let before = rendered(&d, ArtboardId(1));
    let outer = convert_to_symbol(
        &d,
        ArtboardId(1),
        &SymbolSource::Layers(vec![first.instance]),
        "Outer",
        SymbolTiming::Graphic,
    )
    .unwrap();
    outer.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);
    assert_eq!(d.instance_sources(outer.symbol), vec![first.symbol]);
}

#[test]
fn convert_errors() {
    let d = animated();
    let layers = |ids: &[u32]| SymbolSource::Layers(ids.iter().map(|&i| LayerId(i)).collect());
    let run = |d: &Document, src: &SymbolSource| {
        convert_to_symbol(d, ArtboardId(1), src, "S", SymbolTiming::Graphic).err()
    };
    assert_eq!(run(&d, &layers(&[])), Some(ConvertError::NoLayers));
    assert_eq!(run(&d, &layers(&[99])), Some(ConvertError::NoLayers));

    let mut empty = d.clone();
    let s = &mut empty.artboards[0].sprite;
    crate::ops::new_layer(&s.clone(), None, LayerKind::Image).apply(s).unwrap();
    assert_eq!(run(&empty, &layers(&[3])), Some(ConvertError::Empty));

    let mut indexed = d.clone();
    indexed.artboards[0].sprite.color_mode = ColorMode::Indexed;
    assert_eq!(run(&indexed, &layers(&[1])), Some(ConvertError::NotRgba));

    let mut tiles = d.clone();
    tiles.artboards[0].sprite.layers[0].kind = LayerKind::Tilemap;
    assert_eq!(run(&tiles, &layers(&[1])), Some(ConvertError::Tilemap));
}
```

  (Si `ColorMode` no tiene una variante `Indexed` con ese nombre, usar la variante indexada que
  exista en `crates/cs-core/src/color.rs`.)

- [ ] **Step 2: Ver que fallan.** `cargo test -p cs-commands symbol::` → no compila.

- [ ] **Step 3: Implementar.** Agregar a `symbol.rs` (los `use` necesarios: `std::collections::BTreeSet`,
  `cs_core::{ColorMode, GraphicPlay, Image, LayerId, LayerKind, Mask, PixelValue, Sprite, Timing}`,
  `crate::{Command, ops}`):

```rust
/// Lo que F8 convierte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolSource {
    /// Las capas completas (con sus hijas, si son grupos).
    Layers(Vec<LayerId>),
    /// Los píxeles de `selection` en esas capas, en todos los frames (solo capas de imagen).
    Pixels { layers: Vec<LayerId>, selection: Mask },
}

/// El "Type" del diálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolTiming {
    Graphic,
    MovieClip,
}

impl SymbolTiming {
    /// El `Timing` de la key de la instancia nueva: desde el primer frame del símbolo.
    pub fn timing(self) -> Timing {
        match self {
            SymbolTiming::Graphic => Timing::Graphic { first_frame: 0, play: GraphicPlay::Loop },
            SymbolTiming::MovieClip => Timing::MovieClip { first_frame: 0 },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    #[error("Symbols need an RGBA sprite")]
    NotRgba,
    #[error("Tilemap layers can't go into a symbol")]
    Tilemap,
    #[error("Nothing to convert")]
    Empty,
    #[error("Select the layers to convert")]
    NoLayers,
}

/// Lo que arma `convert_to_symbol`: un solo comando (un Ctrl+Z) y los ids nuevos.
#[derive(Clone, Debug, PartialEq)]
pub struct Converted {
    pub command: DocCommand,
    pub symbol: ArtboardId,
    pub instance: LayerId,
}

/// Convierte capas o píxeles del artboard `host` en un símbolo nuevo de la biblioteca y deja en
/// su lugar una capa de instancia que se ve igual en todos los frames.
pub fn convert_to_symbol(
    doc: &Document,
    host: ArtboardId,
    source: &SymbolSource,
    name: &str,
    timing: SymbolTiming,
) -> Result<Converted, ConvertError> {
    let sprite = &doc.artboard(host).ok_or(ConvertError::NoLayers)?.sprite;
    let (listed, selection) = match source {
        SymbolSource::Layers(layers) => (layers, None),
        SymbolSource::Pixels { layers, selection } => (layers, Some(selection)),
    };
    let roots = roots(sprite, listed);
    let top = *roots.last().ok_or(ConvertError::NoLayers)?;
    if sprite.color_mode != ColorMode::Rgba {
        return Err(ConvertError::NotRgba);
    }
    let converted: BTreeSet<LayerId> = roots
        .iter()
        .flat_map(|&id| sprite.layers[sprite.subtree_range(id).expect("root exists")].iter())
        .map(|l| l.id)
        .collect();
    let layer = |id: LayerId| sprite.layer(id).expect("converted layer exists");
    if converted.iter().any(|&id| layer(id).kind == LayerKind::Tilemap) {
        return Err(ConvertError::Tilemap);
    }
    // En el modo píxeles solo cuentan las capas de imagen.
    let cut: BTreeSet<LayerId> = match selection {
        None => converted.clone(),
        Some(_) => converted.iter().copied().filter(|&id| layer(id).kind == LayerKind::Image).collect(),
    };
    let rect = content_rect(doc, sprite, &cut, selection).ok_or(ConvertError::Empty)?;

    let symbol_id = doc.next_artboard_id();
    let symbol = Artboard {
        id: symbol_id,
        name: name.to_string(),
        position: free_position(doc),
        sprite: symbol_sprite(sprite, &roots, &cut, selection, rect),
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library: true,
    };

    // La instancia va encima de la capa convertida de más arriba (mismo padre); al borrar las
    // capas, queda en su lugar.
    let Command::AddLayer { index, mut layer } =
        ops::new_instance_layer(sprite, Some(top), 0, symbol_id, name, (rect.0, rect.1))
    else {
        unreachable!("new_instance_layer devuelve AddLayer");
    };
    if let Some(track) = &mut layer.instance {
        track.keys[0].timing = timing.timing();
    }
    let instance = layer.id;
    let mut host_cmds = vec![Command::AddLayer { index, layer }];
    match selection {
        None => host_cmds.extend(remove_layers(sprite, &roots)),
        Some(mask) => host_cmds.extend(erase_pixels(sprite, &cut, mask)),
    }

    let command = DocCommand::Batch(vec![
        DocCommand::AddArtboard { index: doc.artboards.len(), artboard: Box::new(symbol) },
        DocCommand::OnArtboard { id: host, cmd: Command::Batch(host_cmds) },
    ]);
    Ok(Converted { command, symbol: symbol_id, instance })
}

/// Las capas pedidas que existen, sin repetir y sin las que ya están dentro de otra pedida, de
/// abajo hacia arriba.
fn roots(sprite: &Sprite, listed: &[LayerId]) -> Vec<LayerId> {
    let exists: Vec<LayerId> = listed.iter().copied().filter(|&id| sprite.layer(id).is_some()).collect();
    let mut roots: Vec<LayerId> = exists
        .iter()
        .copied()
        .filter(|&id| !exists.iter().any(|&other| other != id && sprite.is_descendant(id, other)))
        .collect();
    roots.sort_by_key(|&id| sprite.layer_index(id));
    roots.dedup();
    roots
}

/// El rectángulo `(x, y, w, h)` con lo que se convierte en algún frame, recortado al lienzo.
fn content_rect(
    doc: &Document,
    sprite: &Sprite,
    layers: &BTreeSet<LayerId>,
    selection: Option<&Mask>,
) -> Option<(i32, i32, u32, u32)> {
    let (cw, ch) = (sprite.width as i32, sprite.height as i32);
    let mut acc: Option<(i32, i32, i32, i32)> = None;
    let mut add = |x0: i32, y0: i32, x1: i32, y1: i32| {
        let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(cw - 1), y1.min(ch - 1));
        if x0 > x1 || y0 > y1 {
            return;
        }
        acc = Some(match acc {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    };
    for &id in layers {
        let layer = sprite.layer(id).expect("listed layer exists");
        match layer.kind {
            LayerKind::Image => {
                for frame in 0..sprite.frames.len() {
                    let Some(cel) = sprite.cel(id, frame) else { continue };
                    let Some(image) = sprite.image(cel.image) else { continue };
                    for (u, v) in visible_pixels(image) {
                        let (x, y) = (cel.position.0 + u, cel.position.1 + v);
                        if selection.is_none_or(|m| m.contains(x, y)) {
                            add(x, y, x, y);
                        }
                    }
                }
            }
            LayerKind::Instance => {
                for key in layer.instance.iter().flat_map(|t| t.keys.iter()) {
                    let Some(src) = key.source.and_then(|s| doc.artboard(s)) else { continue };
                    let (w, h) = (src.sprite.width as i32, src.sprite.height as i32);
                    add(key.position.0, key.position.1, key.position.0 + w - 1, key.position.1 + h - 1);
                }
            }
            LayerKind::Group | LayerKind::Tilemap => {}
        }
    }
    acc.map(|(x0, y0, x1, y1)| (x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32))
}

/// Las coordenadas de los píxeles con alfa > 0 de una imagen RGBA.
fn visible_pixels(image: &Image) -> impl Iterator<Item = (i32, i32)> + '_ {
    (0..image.height()).flat_map(move |v| {
        (0..image.width()).filter_map(move |u| match image.get(u, v) {
            Some(PixelValue::Rgba(c)) if c.a > 0 => Some((u as i32, v as i32)),
            _ => None,
        })
    })
}

/// El sprite del símbolo: copia del host con las capas convertidas y sus grupos padre,
/// desplazado para que `rect` empiece en (0, 0).
fn symbol_sprite(
    host: &Sprite,
    roots: &[LayerId],
    cut: &BTreeSet<LayerId>,
    selection: Option<&Mask>,
    rect: (i32, i32, u32, u32),
) -> Sprite {
    let (dx, dy) = (-rect.0, -rect.1);
    let mut keep: BTreeSet<LayerId> = cut.clone();
    for &root in roots {
        let mut parent = host.layer(root).and_then(|l| l.parent);
        while let Some(p) = parent {
            keep.insert(p);
            parent = host.layer(p).and_then(|l| l.parent);
        }
        // En el modo capas también van los grupos convertidos (cut ya trae sus hijas).
        if selection.is_none() {
            keep.insert(root);
        }
    }
    let mut s = host.clone();
    s.width = rect.2;
    s.height = rect.3;
    s.layers.retain(|l| keep.contains(&l.id));
    s.cels.retain(|(layer, _), _| cut.contains(layer));
    s.slices.clear();
    s.tilesets.clear();
    s.tilemaps.clear();
    s.extra_chunks.clear();
    if let Some(mask) = selection {
        // Cada imagen una sola vez (los cels vinculados la comparten).
        let mut done = BTreeSet::new();
        for cel in s.cels.values() {
            if !done.insert(cel.image) {
                continue;
            }
            let Some(image) = s.images.get(&cel.image) else { continue };
            let mut kept = (**image).clone();
            for v in 0..kept.height() {
                for u in 0..kept.width() {
                    let (x, y) = (cel.position.0 + u as i32, cel.position.1 + v as i32);
                    if !mask.contains(x, y) {
                        kept.set(u, v, PixelValue::Rgba(cs_core::Rgba::TRANSPARENT)).expect("in bounds");
                    }
                }
            }
            s.images.insert(cel.image, std::sync::Arc::new(kept));
        }
    }
    for cel in s.cels.values_mut() {
        cel.position = (cel.position.0 + dx, cel.position.1 + dy);
    }
    for layer in &mut s.layers {
        if let Some(mask) = &mut layer.mask {
            mask.position = (mask.position.0 + dx, mask.position.1 + dy);
        }
        if let Some(track) = &mut layer.instance {
            for key in &mut track.keys {
                key.position = (key.position.0 + dx, key.position.1 + dy);
            }
        }
    }
    let used: BTreeSet<_> = s.cels.values().map(|c| c.image).collect();
    s.images.retain(|id, _| used.contains(id));
    s
}

/// Borra del host las capas convertidas y, después, los grupos que queden vacíos.
fn remove_layers(sprite: &Sprite, roots: &[LayerId]) -> Vec<Command> {
    let mut sim = sprite.clone();
    let mut cmds = Vec::new();
    let mut run = |cmd: Command, sim: &mut Sprite, cmds: &mut Vec<Command>| {
        cmd.apply(sim).expect("removal is valid");
        cmds.push(cmd);
    };
    for &root in roots.iter().rev() {
        if let Some(cmd) = ops::remove_layer_tree(&sim, root) {
            run(cmd, &mut sim, &mut cmds);
        }
    }
    let mut parents: Vec<LayerId> = roots
        .iter()
        .filter_map(|&r| sprite.layer(r).and_then(|l| l.parent))
        .collect();
    while let Some(group) = parents.pop() {
        let empty = sim.layer(group).is_some() && !sim.layers.iter().any(|l| l.parent == Some(group));
        if empty {
            let up = sim.layer(group).and_then(|l| l.parent);
            run(Command::RemoveLayer { id: group }, &mut sim, &mut cmds);
            parents.extend(up);
        }
    }
    cmds
}

/// Borra los píxeles de `mask` en las capas `cut`, en todos los frames.
fn erase_pixels(sprite: &Sprite, cut: &BTreeSet<LayerId>, mask: &Mask) -> Vec<Command> {
    let clear = PixelValue::Rgba(cs_core::Rgba::TRANSPARENT);
    let mut cmds = Vec::new();
    for &layer in cut {
        for frame in 0..sprite.frames.len() {
            let Some(cel) = sprite.cel(layer, frame) else { continue };
            let Some(image) = sprite.image(cel.image) else { continue };
            let pixels: Vec<_> = visible_pixels(image)
                .map(|(u, v)| (cel.position.0 + u, cel.position.1 + v))
                .filter(|&(x, y)| mask.contains(x, y))
                .map(|(x, y)| (x, y, clear))
                .collect();
            if !pixels.is_empty() {
                cmds.push(Command::DrawPixels { layer, frame, pixels });
            }
        }
    }
    cmds
}
```

  Notas:
  - La instancia se agrega **antes** de borrar las capas. Por eso `remove_layers` simula sobre el
    sprite **original**: los `RemoveLayer` van por id y no les afecta la capa nueva. Si el batch
    falla al aplicarse porque la capa nueva cambió los índices, simular sobre un clon que ya tenga
    aplicado el `AddLayer`.
  - Si `Image::set` no existe con esa firma, usar el setter que tenga `crates/cs-core/src/image.rs:95`.
  - Si `Option::is_none_or` no está disponible en el toolchain, usar
    `selection.map_or(true, |m| m.contains(x, y))`.

- [ ] **Step 4: Ver que pasan.** `cargo test -p cs-commands symbol::` → verde. Si
  `rendered(...) == before` falla, imprimir los dos frames y revisar el desplazamiento de cels y
  máscaras.

- [ ] **Step 5: Clippy y commit.** `cargo clippy -p cs-commands --all-targets -- -D warnings`, y
  después `git add -A crates/cs-commands && git commit -m "Símbolos: convert_to_symbol (capas): el símbolo se lleva toda la animación y el host se ve igual"`.

---

### Task 4: Convertir píxeles (marquesina)

**Files:**
- Modify: `crates/cs-commands/src/symbol/tests.rs` (el código del modo píxeles ya está en la Task 3)

**Interfaces:**
- Consumes: `convert_to_symbol` con `SymbolSource::Pixels`.

- [ ] **Step 1: Pruebas:**

```rust
#[test]
fn converting_pixels_cuts_the_selection_in_every_frame() {
    let mut d = animated();
    let before = rendered(&d, ArtboardId(1));
    // La marquesina toma (2,3) y (3,3) pero no (4,4) ni el azul.
    let selection = Mask::from_rect(8, 8, (2, 3), (3, 3));
    let source = SymbolSource::Pixels { layers: vec![LayerId(1), LayerId(2)], selection };
    let c = convert_to_symbol(&d, ArtboardId(1), &source, "Cut", SymbolTiming::Graphic).unwrap();
    c.command.apply(&mut d).unwrap();
    assert_eq!(rendered(&d, ArtboardId(1)), before);

    let host = &d.artboard(ArtboardId(1)).unwrap().sprite;
    assert_eq!(host.pixel(LayerId(1), 0, 2, 3), PixelValue::Rgba(Rgba::TRANSPARENT));
    assert_eq!(host.pixel(LayerId(1), 2, 4, 4), RED, "outside the marquee stays");
    assert_eq!(host.pixel(LayerId(2), 0, 6, 6), BLUE);

    let symbol = &d.artboard(c.symbol).unwrap().sprite;
    assert_eq!((symbol.width, symbol.height), (2, 1));
    assert_eq!(symbol.pixel(LayerId(1), 0, 0, 0), RED);
    assert_eq!(symbol.pixel(LayerId(1), 1, 1, 0), RED);
    // La capa 2 viaja al símbolo, pero el azul quedó fuera de la marquesina.
    assert_eq!(symbol.pixel(LayerId(2), 0, 0, 0), PixelValue::Rgba(Rgba::TRANSPARENT));
}

#[test]
fn an_empty_marquee_is_nothing_to_convert() {
    let d = animated();
    let selection = Mask::from_rect(8, 8, (0, 0), (1, 1));
    let source = SymbolSource::Pixels { layers: vec![LayerId(1)], selection };
    assert_eq!(
        convert_to_symbol(&d, ArtboardId(1), &source, "S", SymbolTiming::Graphic).err(),
        Some(ConvertError::Empty)
    );
}
```

- [ ] **Step 2: Correr.** `cargo test -p cs-commands symbol::` → verde. Si falla, revisar
  `symbol_sprite` (que la capa no convertida quede afuera) y `erase_pixels`.

- [ ] **Step 3: Commit.** `git commit -am "Símbolos: convertir los píxeles de la marquesina, en todos los frames"`

---

### Task 5: `.chimp`, export del atlas y la herramienta Artboard

**Files:**
- Modify: `crates/cs-io/src/chimp.rs` (`Entry` :34-47, `write` :173-181, `read` :236-243, tests :276+)
- Modify: `crates/cs-io/src/export.rs` (`export_atlas` :472: líneas :480, :485, :491, :521)
- Modify: `crates/cs-tools/src/artboard.rs` (`hit_test` :29-30)

**Interfaces:**
- Consumes: `Artboard.in_library`, `Document::canvas_artboards` (Task 1).

- [ ] **Step 1: Pruebas que fallan.** En los tests de `chimp.rs`:

```rust
#[test]
fn library_roundtrip() {
    let mut d = doc();
    d.artboards[1].in_library = true;
    let (back, _) = read(&write(&d).unwrap()).unwrap();
    assert!(back.artboards[1].in_library);
    assert!(!back.artboards[0].in_library);
}
```

  En los tests de `crates/cs-tools/src/artboard.rs`, una prueba de que `hit_test` sobre un artboard
  de biblioteca devuelve `None`. Usar el mismo armado de documento de los tests vecinos y marcar
  `in_library = true` el artboard golpeado.

- [ ] **Step 2: Ver que fallan.** `cargo test -p cs-io library_roundtrip` → falla
  (`in_library` vuelve `false`).

- [ ] **Step 3: Implementar.**
  - **`Entry`:**

```rust
    /// Símbolo de la biblioteca (no se ve en el lienzo).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    library: bool,
```

  - **`write`:** `library: artboard.in_library,`.
  - **`read`:** `in_library: entry.library,`.
  - **`export_atlas`:** reemplazar las cuatro iteraciones de `doc.artboards` por
    `doc.canvas_artboards()`. Si alguna necesita `len()` o indexar, usar
    `let boards: Vec<&Artboard> = doc.canvas_artboards().collect();` al principio de la función.
  - **`hit_test`:** `for artboard in doc.canvas_artboards().rev() {`.

- [ ] **Step 4: Ver que pasan.** `cargo test -p cs-io -p cs-tools` → verde.

- [ ] **Step 5: Commit.** `git commit -am "Símbolos: el .chimp guarda la biblioteca; el atlas y la herramienta Artboard la ignoran"`

---

### Task 6: MCP

**Files:**
- Modify: `crates/cs-agent/src/params.rs` (cerca de `AddInstanceLayer`, :743)
- Modify: `crates/cs-agent/src/dispatch.rs` (`OPERATIONS` :50 y `dispatch!` :129)
- Modify: `crates/cs-agent/src/ops/structure.rs` (handlers cerca de `add_instance_layer`, :443; tests cerca de :784)
- Modify: `crates/cs-agent/src/ops/document.rs` (`describe_document` :167-190)
- Modify: `apps/chimpsprite-mcp/src/server.rs` (tools cerca de :184)

**Interfaces:**
- Consumes:
  - `cs_commands::symbol::{convert_to_symbol, move_to_library, place_on_canvas, SymbolSource, SymbolTiming}`
  - `self.run_on(&doc, host, cmds)`, que ejecuta comandos de sprite;
  - la forma de ejecutar un `DocCommand` que usa `add_artboard` (`structure.rs:59`). Reutilizar
    ese mismo camino.

- [ ] **Step 1: Prueba que falla** (en los tests de `structure.rs`):

```rust
#[test]
fn convert_to_symbol_moves_layers_into_the_library() {
    let mut s = session(8, 8);
    s.draw_pixels(p(json!({"doc": "d1", "pixels": [[2, 3, "#ff0000"]]}))).unwrap();
    let before = s.render_rgba("d1", None, 0);
    s.convert_to_symbol(p(json!({"doc": "d1", "layers": ["Layer 1"], "name": "Dot"}))).unwrap();
    assert_eq!(s.render_rgba("d1", None, 0), before);
    let described = crate::ops::test_util::json(&s.describe(p(json!({"doc": "d1"}))).unwrap());
    assert_eq!(described["artboards"][1]["name"], "Dot");
    assert_eq!(described["artboards"][1]["in_library"], true);
    s.place_on_canvas(p(json!({"doc": "d1", "artboard": "Dot"}))).unwrap();
    let described = crate::ops::test_util::json(&s.describe(p(json!({"doc": "d1"}))).unwrap());
    assert_eq!(described["artboards"][1]["in_library"], false);
}
```

  Antes de escribirla, revisar cómo los tests vecinos pintan y renderizan. Si no existen
  `draw_pixels` ni `render_rgba` con esos nombres, usar las operaciones y helpers reales que usan
  los tests de `add_instance_layer`, por ejemplo un `render` que devuelve PNG comparado byte por
  byte.

- [ ] **Step 2: Implementar.**
  - **Params** (con `#[derive(Debug, Deserialize, JsonSchema)]` y doc comments en inglés, que van al
    schema):

```rust
/// Turn layers (or the pixels inside a rectangle) into a library symbol, leaving an instance.
pub struct ConvertToSymbol {
    pub doc: String,
    /// Host artboard (default: the first one).
    pub artboard: Option<Ref>,
    /// Layers to convert, by name or id.
    pub layers: Vec<Ref>,
    /// Optional rectangle [x, y, width, height]: only these pixels are cut (image layers only).
    pub rect: Option<[i32; 4]>,
    /// Symbol name.
    pub name: String,
    /// "graphic" (default) or "movie_clip".
    #[serde(default)]
    pub timing: Option<String>,
}

/// Move an artboard to the library or back onto the canvas.
pub struct LibraryArtboard {
    pub doc: String,
    pub artboard: Ref,
}
```

  - **Handlers** en `structure.rs`:
    - `convert_to_symbol`: resuelve el artboard y las capas con los helpers de `target`, como hace
      `add_instance_layer`.
    - **`rect`:** se vuelve
      `Mask::from_rect(w, h, (x, y), (x + rw - 1, y + rh - 1))` y se pasa como
      `SymbolSource::Pixels`. Sin `rect`, es `SymbolSource::Layers`.
    - **`timing`:** `"movie_clip"` es `SymbolTiming::MovieClip`; ausente o `"graphic"` es
      `SymbolTiming::Graphic`; cualquier otro valor es `Err(invalid(...))`.
    - **Ejecución:** el `Converted.command` se corre con el mismo camino de `DocCommand` que usa
      `add_artboard`.
    - **Respuesta:** `Reply::text(format!("symbol {} with instance layer {}", symbol.0, instance.0))`.
    - **`ConvertError`:** se vuelve `invalid(e.to_string())`.
    - `move_to_library` y `place_on_canvas` hacen lo mismo con las funciones de `symbol`. Si la
      función devuelve `None`, responden `Err(invalid("already there, or the last canvas artboard"))`.
  - **Registro:** agregar `"convert_to_symbol"`, `"move_to_library"` y `"place_on_canvas"` a
    `OPERATIONS` y al `dispatch!`, y los tres tools en `server.rs`, copiando el patrón de
    `add_instance_layer`.
  - **`describe_document`:** `obj.insert("in_library".into(), json!(a.in_library));`.

- [ ] **Step 3: Ver que pasan.** `cargo test -p cs-agent && cargo build -p chimpsprite-mcp` → verde.

- [ ] **Step 4: Commit.** `git commit -am "Símbolos: convert_to_symbol, move_to_library y place_on_canvas en MCP; in_library en describe"`

---

### Task 7: El lienzo y los paneles ignoran la biblioteca

**Files:**
- Modify: `crates/cs-ui/src/canvas.rs`:
  - `fit` :326
  - `color_under` :297
  - `covers` :489
  - loop de texturas :633
  - loop principal :691
  - guías :796
  - simetría :842
  - `artboard_menus` :1338
  - `can_delete` :1399
  - `title_at` :1425 y :1445
- Modify: `crates/cs-ui/src/canvas/input.rs`:
  - `under_front_artboard` :12
  - botón ○ :494
  - cursor de la herramienta Artboard :654
- Modify: `crates/cs-ui/src/editor.rs` (`Editor::new` :357/:360, `ensure_valid` :442)
- Modify: `crates/cs-ui/src/editor/gestures.rs` (:340, :348, :390). Ya pasan por
  `artboard_at` y `hit_test`, que filtran; solo revisar.
- Modify: `crates/cs-ui/src/editor/document_ops.rs` (`remove_active_artboard` :31)
- Modify: `crates/cs-ui/src/panels/inspectors.rs`:
  - `artboards` :18 y :33-44
  - `pixel_artboards` :76 y :89-98
- Modify: `crates/cs-ui/src/instance_ops.rs` (`instance_candidates` :10-19)
- Modify: `crates/cs-ui/src/timeline.rs` (`instance_menu` :1320-1334)
- Modify: `crates/cs-ui/src/export_dialog.rs:378` (cuenta "N artboards" del atlas)
- Test: `crates/cs-ui/src/editor/tests.rs`, `crates/cs-ui/src/layout_tests.rs`

**Interfaces:**
- Consumes: `canvas_artboards`, `canvas_count`, `library` (Task 1).
- Produces:
  - `Editor::instance_candidates()` devuelve primero los símbolos de la biblioteca y luego los
    artboards del lienzo.
  - Nuevo `Editor::instance_candidate_groups() -> (Vec<(ArtboardId, String)>, Vec<(ArtboardId, String)>)`
    con `(biblioteca, lienzo)`.

- [ ] **Step 1: Pruebas que fallan** (en `editor/tests.rs`):

```rust
#[test]
fn library_artboards_are_not_activated_by_clicks_or_listed() {
    let mut e = editor();
    e.add_artboard(4, 4); // id 2, a la derecha
    let id = ArtboardId(2);
    e.document.artboard_mut(id).unwrap().in_library = true;
    e.activate(ArtboardId(1));
    let rect = e.document.artboard(id).unwrap().rect();
    let inside = ((rect.0.0 + 1) as f32, (rect.0.1 + 1) as f32);
    assert_eq!(e.document.artboard_at(inside), None);
    let (library, canvas) = e.instance_candidate_groups();
    assert_eq!(library.iter().map(|c| c.0).collect::<Vec<_>>(), vec![id]);
    assert!(canvas.is_empty(), "the active artboard can't hold itself");
}

#[test]
fn a_new_editor_starts_on_a_canvas_artboard() {
    let mut doc = Document::with_sprite("a", Sprite::new(4, 4, ColorMode::Rgba));
    let mut symbol = doc.artboards[0].clone();
    symbol.id = ArtboardId(2);
    symbol.in_library = true;
    doc.artboards.insert(0, symbol);
    let e = Editor::new(doc);
    assert_eq!(e.active, ArtboardId(1));
}
```

  Ajustar el constructor (`Editor::new(doc)` o el que exista en `editor.rs:357`) y los `use` a lo
  que hay en el archivo.

- [ ] **Step 2: Ver que fallan.** `cargo test -p cs-ui library_artboards` → falla.

- [ ] **Step 3: Implementar.**
  - **`Editor::new` y `ensure_valid`:** el fallback es
    `document.canvas_artboards().next().unwrap_or(&document.artboards[0]).id`.
  - **`canvas.rs`:** en los loops de dibujo, guías, `covers`, `artboard_menus` y `title_at`, iterar
    `editor.document.canvas_artboards()` en lugar de `.artboards.iter()`. Reglas:
    - `covers` y `under_front_artboard` deben seguir usando el **mismo orden filtrado** que el
      loop de dibujo, para que los índices de `covers[k+1..]` (simetría, :842) cuadren. Calcular
      `k` sobre `canvas_artboards()`.
    - El loop de texturas (:633) puede seguir creando textura solo para los del lienzo. La
      Task 10 agrega el caso del modo símbolo.
  - **`can_delete` (canvas :1399, input :494, inspectors :18/:76):** `editor.document.canvas_count() > 1`.
  - **`remove_active_artboard`:** cambiar `if len() > 1` por `if self.document.canvas_count() > 1`.
    Después de borrar, activar `canvas_artboards().last()`.
  - **Listas del panel Artboards (:33-44, :89-98):** filtrar con `canvas_artboards()`.
  - **`instance_candidate_groups()`** en `instance_ops.rs`: aplica el mismo filtro que
    `instance_candidates` (no ciclo, host RGBA) y separa por `in_library`. Cada grupo va ordenado
    por nombre. `instance_candidates()` devuelve `library` seguido de `canvas`.
  - **`instance_menu`:** dibuja primero los de la biblioteca, después `ui.separator()` si los dos
    grupos tienen elementos, y luego los del lienzo.
  - **`export_dialog.rs:378`:** contar con `canvas_count()`.

- [ ] **Step 4: Correr todo.** `cargo test -p cs-ui` → verde. Los tests existentes de layout de
  artboards siguen pasando porque no hay biblioteca.

- [ ] **Step 5: Commit.** `git commit -am "Símbolos: el lienzo, el panel de Artboards y Delete ignoran la biblioteca; New Instance lista primero los símbolos"`

---

### Task 8: F8, el diálogo "Convert to Symbol" y Move to Library

**Files:**
- Create: `crates/cs-ui/src/symbol_ops.rs` (con `mod symbol_ops;` en `lib.rs`)
- Create: `crates/cs-ui/src/symbol_dialog.rs` (con `mod symbol_dialog;` en `lib.rs`), copiando la
  estructura de `tilemap_dialog.rs` (`DialogResult`, `finish`, `dialog_window`)
- Modify: `crates/cs-ui/src/keymap.rs`:
  - `Action` :127
  - `COMMANDS` :191 (subir el largo)
  - `names` :252: `("Layer", "Convert to Symbol")`
  - `gate` :312: `Gate::NotPainting`
  - `defaults` :352: `vec![k(Key::F8)]`
- Modify: `crates/cs-ui/src/app_actions.rs` (`run_action` :64)
- Modify: `crates/cs-ui/src/app_menus.rs` (menú Layer :767-857)
- Modify: `crates/cs-ui/src/timeline.rs` (`TimelineState` :136, menú contextual de capa :1059)
- Modify: `crates/cs-ui/src/canvas/input.rs` (menú de selección :512) y `canvas.rs`:
  - campo `symbol_request: bool` en `CanvasView`, igual que `artboard_request`
  - menú "≡" `artboard_menu` :1368: agregar "Move to Library"
- Modify: `crates/cs-ui/src/lib.rs`:
  - campo `symbol_dialog: Option<symbol_dialog::SymbolDialog>` (:164 y :295)
  - consumir las banderas (:1159)
  - `update_symbol_dialog`
- Modify: `crates/cs-ui/src/file_actions.rs` (`is_modal` :97)
- Modify: `crates/cs-ui/src/dialog_tests.rs` (`frame()` :67 llama a `update_symbol_dialog`; tests)

**Interfaces:**
- Consumes: `symbol::{convert_to_symbol, move_to_library, SymbolSource, SymbolTiming, ConvertError}`.
- Produces (en `symbol_ops.rs`, `impl Editor`):

```rust
/// Lo que F8 convertiría ahora: con marquesina, sus píxeles en `range_layers()`; si no, esas capas.
pub fn symbol_source(&self) -> SymbolSource;
/// El nombre que propone el diálogo: el de la capa si es una sola y no es la marquesina; si no,
/// el primer "Symbol N" libre.
pub fn suggested_symbol_name(&self) -> String;
/// Revisa sin aplicar (para deshabilitar menús).
pub fn can_convert_to_symbol(&self) -> Result<(), ConvertError>;
/// Convierte, selecciona la instancia nueva, borra la marquesina y recuerda el símbolo como
/// `library_selected`.
pub fn convert_to_symbol(&mut self, name: &str, timing: SymbolTiming) -> Result<ArtboardId, ConvertError>;
/// Manda el artboard activo a la biblioteca y activa otro del lienzo. `false` si no se puede.
pub fn move_active_to_library(&mut self) -> bool;
```

  y el campo `pub library_selected: Option<ArtboardId>` en `Editor` (`editor.rs:259`, inicializado
  en `None`).

- [ ] **Step 1: Pruebas que fallan.** En `symbol_ops.rs` (`#[cfg(test)] mod tests`, con el helper
  `editor()` de `instance_ops.rs:113`):

```rust
#[test]
fn f8_converts_the_current_layer_or_the_marquee() {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    e.execute(Command::DrawPixels { layer: e.layer, frame: 0, pixels: vec![(2, 2, red), (5, 5, red)] });
    assert_eq!(e.suggested_symbol_name(), "Layer 1");
    e.selection = Some(Mask::from_rect(8, 8, (0, 0), (3, 3)));
    assert_eq!(e.suggested_symbol_name(), "Symbol 1");
    let symbol = e.convert_to_symbol("Symbol 1", SymbolTiming::Graphic).unwrap();
    assert!(e.selection.is_none());
    assert_eq!(e.library_selected, Some(symbol));
    assert_eq!(e.sprite().layer(e.layer).unwrap().kind, LayerKind::Instance);
    // Solo (2,2) estaba en la marquesina: el símbolo mide 1×1.
    assert_eq!(e.document.artboard(symbol).unwrap().sprite.width, 1);
    // De vuelta en la capa 1, con marquesina: el siguiente nombre libre.
    e.select_layer(LayerId(1));
    assert_eq!(e.suggested_symbol_name(), "Layer 1");
    e.selection = Some(Mask::from_rect(8, 8, (4, 4), (7, 7)));
    assert_eq!(e.suggested_symbol_name(), "Symbol 2");
    e.selection = None;
    e.undo();
    assert_eq!(e.document.artboards.len(), 1);
}

#[test]
fn move_to_library_needs_another_canvas_artboard() {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(!e.move_active_to_library());
    e.add_artboard(4, 4);
    let moved = e.active;
    assert!(e.move_active_to_library());
    assert!(e.document.artboard(moved).unwrap().in_library);
    assert_ne!(e.active, moved);
}
```

  En `dialog_tests.rs`, siguiendo el ejemplo de `convert_tilemap` (:520):

```rust
#[test]
fn convert_to_symbol_dialog_creates_with_the_chosen_type() {
    for pixel in [false, true] {
        let mut h = DialogHarness::new(pixel);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let editor = &mut h.app.doc.editor;
        editor.execute(Command::DrawPixels { layer: editor.layer, frame: 0, pixels: vec![(1, 1, red)] });
        h.app.open_convert_to_symbol();
        h.settle();
        h.click("Movie Clip");
        h.key(egui::Key::Enter);
        assert!(h.app.symbol_dialog.is_none());
        let e = &h.app.doc.editor;
        let key = e.sprite().layer(e.layer).unwrap().instance.as_ref().unwrap().keys[0];
        assert_eq!(key.timing, SymbolTiming::MovieClip.timing(), "pixel ui {pixel}");
    }
}
```

- [ ] **Step 2: Ver que fallan.** `cargo test -p cs-ui f8_ convert_to_symbol_dialog move_to_library` → no compila.

- [ ] **Step 3: Implementar.**
  - **`symbol_ops.rs`:** las funciones de arriba.
    - `symbol_source` usa `self.selection.clone()` y `self.range_layers()`.
    - `convert_to_symbol`:
      1. llama a `cs_commands::symbol::convert_to_symbol(&self.document, self.active, &src, name, timing)?`;
      2. ejecuta `self.execute_doc(c.command)` (si devuelve `false`, `Err(ConvertError::Empty)`);
      3. hace `self.select_layer(c.instance)`, `self.deselect()` y `self.library_selected = Some(c.symbol)`.
    - **Nombre libre:** el primer `n >= 1` tal que ningún artboard se llame `format!("Symbol {n}")`.
  - **`symbol_dialog.rs`:** `pub struct SymbolDialog { pub name: String, pub timing: SymbolTiming }`
    con `new(editor)` y `show(ctx, skin) -> DialogResult`. El cuerpo es un `egui::Grid` con:
    - `"Name"` + `widgets::text_line_named(ui, &mut self.name, "Name")`;
    - `"Type"` + `widgets::radio_value(ui, &mut self.timing, SymbolTiming::Graphic, "Graphic")` y
      `widgets::radio_value(ui, &mut self.timing, SymbolTiming::MovieClip, "Movie Clip")`.

    Título del diálogo: `"Convert to Symbol"`.
  - **`lib.rs`:**
    - `pub fn open_convert_to_symbol(&mut self)`: si
      `self.doc.editor.can_convert_to_symbol()` da `Err(e)`, llamar a
      `self.show_message("Convert to Symbol", e.to_string())`; si no,
      `self.symbol_dialog = Some(SymbolDialog::new(&self.doc.editor))`.
    - `fn update_symbol_dialog(&mut self, ctx)`, como `update_tilemap_dialogs`. En `Accept`,
      `editor.convert_to_symbol(&name, timing)`, y si da `Err(e)`, `show_message`.
    - Agregar `self.symbol_dialog.is_some()` en `is_modal`.
    - Consumir `std::mem::take(&mut self.doc.timeline.convert_symbol)` y
      `std::mem::take(&mut self.doc.canvas.symbol_request)` → `open_convert_to_symbol()`.
  - **`app_actions.rs`:** `ConvertToSymbol => self.open_convert_to_symbol(),`.
  - **`app_menus.rs` (menú Layer):** después de "New Instance":

```rust
let convert = editor.can_convert_to_symbol();
if ui
    .add_enabled(convert.is_ok(), menus::item("Convert to Symbol…", action(keymap::Action::ConvertToSymbol)))
    .on_disabled_hover_text(convert.err().map(|e| e.to_string()).unwrap_or_default())
    .clicked()
{
    open_symbol = true; // abrir después del closure, que tiene prestado `editor`
    ui.close();
}
```

    (Usar una variable local con el patrón que usa `app_menus.rs` para abrir diálogos fuera del
    préstamo; ver :831-839.)
  - **Menú contextual de capa** (`timeline.rs:1059`): el ítem "Convert to Symbol…" pone
    `state.convert_symbol = true` (campo nuevo `pub convert_symbol: bool` en `TimelineState`).
  - **Menú de selección del lienzo** (`canvas/input.rs:512`): el mismo ítem pone
    `self.symbol_request = true`.
  - **Menú "≡" del artboard** (`canvas.rs:1368`): después de "Duplicate":

```rust
if ui
    .add_enabled(editor.document.canvas_count() > 1, menus::plain("Move to Library"))
    .clicked()
{
    editor.move_active_to_library();
    ui.close();
}
```

- [ ] **Step 4: Ver que pasan.** `cargo test -p cs-ui` → verde, incluido
  `every_action_has_its_defaults_and_no_two_share_one`.

- [ ] **Step 5: Commit.** `git commit -am "Símbolos: F8 / Convert to Symbol con su diálogo, y Move to Library en el menú del artboard"` (agregar antes los archivos nuevos).

---

### Task 9: Panel Library

**Files:**
- Create: `crates/cs-ui/src/library_panel.rs` (con `mod library_panel;` en `lib.rs`)
- Modify: `crates/cs-ui/src/dock.rs`:
  - `DockPanel::Library` en el enum :47
  - `ALL` :63 y :73 (después de `Artboards`; subir los largos)
  - `label` :81: `"Library"`
  - `icon` :93: `"icon.dock.library"`
  - `plain` :105: `"▣"`
  - `match` :232: `out.app_panel = Some((panel, anchor))`
- Modify: `crates/cs-ui/src/lib.rs` (dispatch de `app_panel` :1186-1193 → `self.library_panel(ctx, anchor, modal)`)
- Modify: `assets/ui/skin.aseprite` (pieza `icon.dock.library`; ver Step 4) y `assets/ui/README.md:212`
- Test: `crates/cs-ui/src/library_panel.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Document::library`, `instance_uses`, `Editor::library_selected` (Task 8),
  `symbol::place_on_canvas`, y `Editor::enter_symbol` (Task 10). Hasta la Task 10, el doble clic
  y "Edit Symbol" emiten la acción pero el app la ignora.
- Produces:

```rust
pub enum LibraryAction { Edit(ArtboardId), PlaceOnCanvas(ArtboardId), Delete(ArtboardId),
                         Duplicate(ArtboardId), Rename(ArtboardId, String) }
/// Las filas de la lista: (id, nombre, usos), ordenadas por nombre (sin distinguir mayúsculas).
pub fn library_rows(doc: &Document) -> Vec<(ArtboardId, String, usize)>;
/// Dibuja el panel; devuelve lo que el usuario pidió.
pub fn show(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>, state: &mut LibraryState)
    -> Option<LibraryAction>;
pub struct LibraryState { pub renaming: Option<(ArtboardId, String)>, pub confirm_delete: Option<ArtboardId> }
/// El frame de la vista previa en `t_ms` milisegundos, en loop con las duraciones del símbolo.
pub fn preview_frame(sprite: &Sprite, t_ms: u64) -> usize;
```

  Payload de arrastre: `pub struct SymbolPayload(pub ArtboardId);`, que lo consume la Task 11.

- [ ] **Step 1: Pruebas que fallan:**

```rust
#[test]
fn rows_are_sorted_by_name_with_their_uses() {
    let mut doc = Document::with_sprite("host", Sprite::new(8, 8, ColorMode::Rgba));
    for (id, name) in [(2, "walk"), (3, "Coin")] {
        let mut a = doc.artboards[0].clone();
        a.id = ArtboardId(id);
        a.name = name.into();
        a.in_library = true;
        doc.artboards.push(a);
    }
    let cmd = cs_commands::ops::new_instance_layer(&doc.artboards[0].sprite, None, 0, ArtboardId(3), "c", (0, 0));
    cmd.apply(&mut doc.artboards[0].sprite).unwrap();
    assert_eq!(
        library_rows(&doc),
        vec![(ArtboardId(3), "Coin".to_string(), 1), (ArtboardId(2), "walk".to_string(), 0)]
    );
}

#[test]
fn preview_loops_with_the_symbol_durations() {
    let mut s = Sprite::new(2, 2, ColorMode::Rgba);
    s.frames = vec![Frame { duration_ms: 100 }, Frame { duration_ms: 50 }];
    assert_eq!(preview_frame(&s, 0), 0);
    assert_eq!(preview_frame(&s, 99), 0);
    assert_eq!(preview_frame(&s, 100), 1);
    assert_eq!(preview_frame(&s, 150), 0);
}
```

- [ ] **Step 2: Ver que fallan.** `cargo test -p cs-ui library_panel` → no compila.

- [ ] **Step 3: Implementar.**
  - **`library_rows`:** recorre `doc.library()` y ordena por `name.to_lowercase()`.
  - **`preview_frame`:** `let total: u64 = sum(duration_ms)`. Si `total == 0`, devuelve 0. Si no,
    `t % total` y avanza frame por frame.
  - **Ventana:** el panel se dibuja con `dock::flyout(ctx, DockPanel::Library, anchor, skin, WIDTH, true, |ui| …)`,
    como Groom (`lib.rs:439-468`), con `WIDTH` = el ancho de Groom.
  - **Vista previa arriba:** 96 px de alto. Muestra el símbolo de `editor.library_selected`
    renderizado con
    `cs_render::render_frame(&cs_render::materialize(&doc, &sprite, f..f + 1), f)`, con
    `f = preview_frame(sprite, ctx.input(|i| i.time) as u64 * 1000)`. Se pinta como textura con el
    mayor zoom entero que quepa, con `TextureOptions::NEAREST`. Reutilizar el mismo
    helper de textura que usa Groom o Lumen para su vista previa. Llamar a `ctx.request_repaint()`
    mientras haya un símbolo con más de un frame.
  - **Filas:** de 20 px de alto, con una miniatura del frame 0 (ajustada a 16×16, `NEAREST`), el
    nombre y `"×N"` si `N > 0`. La fila seleccionada usa el color de selección de las listas del
    skin. Si `skin` es `None`, usar `ui.selectable_label`.
  - **Interacción de cada fila:**
    - un clic pone `editor.library_selected = Some(id)`;
    - doble clic → `Edit(id)`;
    - F2 con la fila seleccionada pone `state.renaming`, y la fila muestra
      `widgets::text_line_named(ui, &mut name, "Symbol name")`;
    - Enter → `Rename(id, name)`; Esc cancela;
    - `response.dnd_set_drag_payload(SymbolPayload(id))` (requiere `Sense::click_and_drag()`).
  - **Menú contextual** (`menus::context_menu(&row, |ui| …)`):
    - Edit Symbol;
    - Rename;
    - Duplicate;
    - Place on Canvas;
    - Delete. Si `doc.instance_uses(id) > 0`, pone `state.confirm_delete = Some(id)`, y el panel
      muestra abajo "Used by N instances. Delete anyway?" con [Delete] [Cancel]. Si no se usa,
      devuelve `Delete(id)` directo.
  - **Sin símbolos:** `"Select layers or pixels and press F8 to make a symbol."`, envuelto.
  - **En `lib.rs`, cómo se maneja cada `LibraryAction`:**
    - `Rename` → `editor.execute_doc(DocCommand::SetArtboardProps { id, name: Some(n), position: None })`.
    - `Duplicate` → clonar el artboard con `id = next_artboard_id()`, `name = format!("{name} copy")`
      e `in_library = true`, y `AddArtboard` al final.
    - `PlaceOnCanvas` → `symbol::place_on_canvas` y después `editor.activate(id)` +
      `canvas.request_zoom(ZoomRequest::CenterActive)`.
    - `Delete` → `DocCommand::RemoveArtboard`; si era `library_selected`, se vuelve `None`.
    - `Edit` → `self.enter_symbol(id)` (Task 10; mientras tanto, no hace nada).
  - **Estado:** `LibraryState` vive en `ChimpApp` como campo `library: library_panel::LibraryState`.

- [ ] **Step 4: Ícono.** Dibujar `icon.dock.library` en `assets/ui/skin.aseprite` con el MCP de
  Aseprite (`run_lua_script`; layer 2, cel en 0,0, 320×930).
  - **Forma:** 3 libros de pie, del mismo tamaño y paleta que los otros `icon.dock.*`. Antes, leer
    los slices de `icon.dock.artboards` para copiar el tamaño.
  - **Ubicación:** buscar un hueco libre (revisar píxeles y slices, con 1 px de margen) y agregar
    el slice con ese nombre.
  - **Documentación:** agregar la pieza a la tabla de `assets/ui/README.md`.
  - **Si el MCP de Aseprite no está conectado:** dejar el `plain` "▣" (el dock lo usa si falta la
    pieza; confirmar en `dock.rs` que así es) y avisarle al usuario que falta el ícono.

- [ ] **Step 5: Ver que pasan.** `cargo test -p cs-ui` → verde.

- [ ] **Step 6: Commit.** `git commit -am "Símbolos: panel Library en el dock (vista previa animada, lista con usos, renombrar, duplicar, sacar al lienzo, borrar con confirmación)"`

---

### Task 10: Modo símbolo y migaja de pan

**Files:**
- Modify: `crates/cs-ui/src/symbol_ops.rs` (pila y métodos)
- Modify: `crates/cs-ui/src/editor.rs`:
  - campo `pub symbol_stack: Vec<ArtboardId>` + `return_view: Option<…>`; ver Interfaces
  - `ensure_valid` :442: recortar la pila
- Modify: `crates/cs-ui/src/canvas.rs`:
  - loops de dibujo, texturas y `covers`: con `symbol_stack` no vacía, solo el último
  - `fit`: ajusta al símbolo
  - la migaja de pan
- Modify: `crates/cs-ui/src/canvas/input.rs`:
  - :589-594: doble clic sobre una instancia de biblioteca → `enter_symbol`
  - doble clic en vacío → salir un nivel
- Modify: `crates/cs-ui/src/instance_ops.rs` (`enter_instance` :95) y
  `panels/inspectors.rs:422-425` ("Edit Symbol")
- Modify: `crates/cs-ui/src/lib.rs` (`LibraryAction::Edit`)
- Test: `crates/cs-ui/src/symbol_ops.rs` y `crates/cs-ui/src/layout_tests.rs`

**Interfaces:**
- Produces (en `impl Editor`):

```rust
/// Entra a editar `id` (un símbolo de la biblioteca): lo apila y lo activa. Guarda el artboard
/// activo de antes para volver.
pub fn enter_symbol(&mut self, id: ArtboardId) -> bool;
/// Vuelve al nivel `depth` de la pila (0 = el documento). Activa lo que había ahí.
pub fn exit_symbol_to(&mut self, depth: usize);
/// La migaja de pan: "Document" y los nombres de la pila.
pub fn symbol_breadcrumb(&self) -> Vec<String>;
/// ¿Se está editando un símbolo?
pub fn in_symbol_mode(&self) -> bool;
```

  Campos de `Editor`: `pub symbol_stack: Vec<ArtboardId>` y
  `symbol_return: Option<ArtboardId>` (el artboard activo al entrar desde el documento).

  `CanvasView` guarda `symbol_return_view: Option<(f32, Vec2)>` (zoom y pan). Al salir a
  "Document" se restaura. Al entrar se llama a `fit` sobre el símbolo.

- [ ] **Step 1: Pruebas que fallan** (`symbol_ops.rs`):

```rust
#[test]
fn entering_and_leaving_symbols() {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    e.execute(Command::DrawPixels { layer: e.layer, frame: 0, pixels: vec![(1, 1, red)] });
    let inner = e.convert_to_symbol("Inner", SymbolTiming::Graphic).unwrap();
    let host = ArtboardId(1);
    assert!(e.enter_symbol(inner));
    assert_eq!((e.active, e.symbol_breadcrumb()), (inner, vec!["Document".to_string(), "Inner".into()]));
    assert!(!e.enter_symbol(host), "only library artboards");
    e.exit_symbol_to(0);
    assert!(!e.in_symbol_mode());
    assert_eq!(e.active, host);
}

#[test]
fn undoing_the_symbol_away_trims_the_stack() {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    e.execute(Command::DrawPixels { layer: e.layer, frame: 0, pixels: vec![(1, 1, red)] });
    let s = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
    e.enter_symbol(s);
    e.undo(); // deshace la conversión: el símbolo ya no existe
    assert!(!e.in_symbol_mode());
    assert_eq!(e.active, ArtboardId(1));
}
```

  En `layout_tests.rs`: con un símbolo creado y `editor.enter_symbol(id)`, después de un frame:
  - existe el texto `"Document"` (la migaja de pan);
  - hacer clic en él deja `in_symbol_mode() == false`;
  - en el modo símbolo, el loop de dibujo pinta un solo artboard.

  Para lo último, usar el mismo método con que los tests existentes cuentan artboards dibujados.
  Si no existe ninguno, comprobar que `editor.document.artboard_at` del host no activa nada al
  hacer clic sobre él: en el modo símbolo, los clics sobre otros artboards no deben activarlos
  (ver el Step 3).

- [ ] **Step 2: Ver que fallan.** `cargo test -p cs-ui entering_and_leaving undoing_the_symbol` → no compila.

- [ ] **Step 3: Implementar.**
  - **`enter_symbol(id)`:**
    1. falla si `!artboard(id).in_library`;
    2. si la pila está vacía, guarda `symbol_return = Some(self.active)`;
    3. `push(id)` y `activate(id)`; devuelve `true`.
  - **`exit_symbol_to(depth)`:** `truncate(depth)`; activa `last()` o, si queda vacía,
    `symbol_return.take()` (si existe), y si no, el primero del lienzo.
  - **`ensure_valid`:** recorta la pila al primer id que ya no exista. Si queda vacía y `active` no
    existe o está en la biblioteca, vuelve como en `exit_symbol_to(0)`.
  - **Clics en el modo símbolo:** en `editor/gestures.rs:348`, si `in_symbol_mode()`, no activar
    otro artboard al hacer clic.
  - **`canvas.rs` en el modo símbolo:** el loop de dibujo (:691), el de texturas (:633) y `covers`
    iteran solo `editor.artboard()`. No se dibujan las notas (:997). `fit()` usa
    `editor.artboard().rect()` en vez de `document.bounds()`.
  - **Migaja de pan:** en `CanvasView::show`, si `in_symbol_mode()`, se reserva una franja de 14 px
    arriba del área del lienzo (debajo de las reglas, si están) con el fondo del panel del skin
    (`colors.panel`). Ahí se escribe `"Document › Inner › …"` con la fuente pixel; cada parte es
    clicable (`ui.interact`) y un clic llama a `exit_symbol_to(i)`.
    - Al salir a 0 se pide restaurar `symbol_return_view`.
    - El separador es `" > "`, porque la fuente pixel no tiene `›`: comprobar con
      `fonts.has_glyph`, y si no está, usar `>`.
  - **Doble clic** (`canvas/input.rs:589`): con Move sobre una instancia cuya fuente es de
    biblioteca, llama a `enter_symbol(source)` + `fit`. Si la fuente está en el lienzo, se queda el
    comportamiento actual. Un doble clic con Move fuera de todo píxel del artboard activo, estando
    en el modo símbolo, llama a `exit_symbol_to(len - 1)`.
  - **"Edit Symbol"** del inspector de instancia (`inspectors.rs:422`) usa la misma regla que el
    doble clic.
  - **`LibraryAction::Edit(id)`** en `lib.rs` llama a `enter_symbol(id)` y pide
    `ZoomRequest::Fit`.

- [ ] **Step 4: Ver que pasan.** `cargo test -p cs-ui` → verde.

- [ ] **Step 5: Commit.** `git commit -am "Símbolos: modo símbolo aislado con migaja de pan; doble clic entra y sale"`

---

### Task 11: Arrastrar de la biblioteca al lienzo

**Files:**
- Modify: `crates/cs-ui/src/canvas.rs` o `canvas/input.rs` (sobre la `response` del lienzo, :585)
- Modify: `crates/cs-ui/src/instance_ops.rs`:
  `pub fn drop_symbol(&mut self, source: ArtboardId, world: (i32, i32)) -> Result<(), &'static str>`
- Test: `crates/cs-ui/src/instance_ops.rs` (tests)

**Interfaces:**
- Consumes: `library_panel::SymbolPayload` (Task 9) y `ops::new_instance_layer`.

- [ ] **Step 1: Prueba que falla:**

```rust
#[test]
fn dropping_a_symbol_places_an_instance_where_it_lands() {
    let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    e.execute(Command::DrawPixels { layer: e.layer, frame: 0, pixels: vec![(1, 1, red)] });
    let symbol = e.convert_to_symbol("S", SymbolTiming::Graphic).unwrap();
    let layers = e.sprite().layers.len();
    e.drop_symbol(symbol, (5, 6)).unwrap();
    assert_eq!(e.sprite().layers.len(), layers + 1);
    let layer = e.sprite().layer(e.layer).unwrap();
    assert_eq!(layer.instance.as_ref().unwrap().keys[0].position, (5, 6));
    assert!(e.drop_symbol(symbol, (500, 500)).is_err(), "outside every artboard");
}
```

- [ ] **Step 2: Implementar `drop_symbol`.**
  1. `let target = self.document.artboard_at((x as f32, y as f32)).ok_or("Drop it on an artboard")?;`
     (en el modo símbolo, el destino es siempre `self.active` si el punto está adentro).
  2. Rechazar con `"Symbols need an RGBA sprite"` si el destino no es RGBA, y con
     `"That would put the artboard inside itself"` si `would_cycle(target, source)`.
  3. Local = `world - artboard.position`.
  4. `above = (target == self.active).then_some(self.layer)`; si es otro artboard, `None`
     (arriba de todo).
  5. Armar con `ops::new_instance_layer(sprite, above, self.frame, source, &name, local)` y
     ejecutar con `execute_doc(DocCommand::OnArtboard { id: target, cmd })`.
  6. `activate(target)` y seleccionar la capa nueva.

  En el lienzo:

```rust
if let Some(payload) = response.dnd_release_payload::<crate::library_panel::SymbolPayload>() {
    if let Some(pos) = response.hover_pos() {
        let w = self.to_world(rect, pos);
        if let Err(why) = editor.drop_symbol(payload.0, (w.x.floor() as i32, w.y.floor() as i32)) {
            self.drop_error = Some(why); // el app lo muestra con show_message
        }
    }
}
```

  Para mostrar el error, agregar `drop_error: Option<&'static str>` a `CanvasView`; `lib.rs` lo
  consume con `show_message("Library", why)`. Ajustar `to_world` a su firma real (`canvas.rs:281`).

- [ ] **Step 3: Ver que pasan.** `cargo test -p cs-ui dropping_a_symbol` → verde.

- [ ] **Step 4: Commit.** `git commit -am "Símbolos: arrastrar un símbolo de la biblioteca al lienzo crea la instancia donde se suelta"`

---

### Task 12: Verificación completa, prueba visual y PR

**Files:**
- Modify: `docs/superpowers/specs/2026-10-30-symbol-library-design.md`: estado "aprobado" y agregar
  la sección "Ajustes al spec" de este plan.

- [ ] **Step 1: CI local:**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace \
 && cargo check --target wasm32-unknown-unknown -p cs-core -p cs-raster -p cs-commands -p cs-render -p cs-tools -p cs-io -p cs-agent -p cs-ui -p chimpsprite
```

  Esperado: todo verde.

- [ ] **Step 2: Prueba visual.**
  1. Compilar: `CARGO_TARGET_DIR=target/gui-check22 cargo build --release -p chimpsprite`.
  2. Respaldar `%APPDATA%/ChimpSprite/data/app.ron`.
  3. Si hace falta para la captura, agregar en `crates/cs-ui/src/demo.rs` una variable
     `CHIMPSPRITE_DEMO_SYMBOL=1`. Debe pintar un par de píxeles, convertirlos en símbolo, abrir el
     panel Library y, con `=edit`, entrar al modo símbolo.
  4. Capturar con `CHIMPSPRITE_SCREENSHOT=D:/Work/chimpdeas/.refs/tmp/symbols.png CHIMPSPRITE_SCREENSHOT_DELAY=2 timeout 40 target/gui-check22/release/chimpsprite.exe --demo`.
  5. Revisar la imagen y restaurar `app.ron`.

- [ ] **Step 3: Abrir la app para el usuario.** `(target/gui-check22/release/chimpsprite.exe > /dev/null 2>&1 &)`.

- [ ] **Step 4: Commit, push y PR** (el usuario ya pidió mandarlo a main):

```bash
git push -u origin movie-clips
gh pr create --base main --head movie-clips --title "Movie clips 1: Convertir en símbolo (F8) y biblioteca" --body "…resumen…

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```

  Hacer el merge (squash) solo cuando CI esté verde y el usuario lo confirme.
