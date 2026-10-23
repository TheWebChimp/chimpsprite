# Transformar la selección: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lo seleccionado se levanta una sola vez y flota mientras se mueve, se escala con 8
handles o se rota por fuera de las esquinas (Fast o RotSprite). Pegar también deja el contenido
flotando. Enter o un clic afuera confirman en un solo paso de undo. Esc cancela por partes. Con
esto se arregla el bug de mover: cada arrastre ya no estampa ni levanta lo de debajo.

**Architecture:**
- **`cs-raster::transform`:** algoritmos puros sobre `Block<P>`. Hace la escala por vecino más
  cercano (con volteos), la rotación Fast, RotSprite y los giros exactos de 90°.
- **`cs-ui::transform_box`:** geometría pura, sin egui. Tiene la caja con signo y ángulo, los 8
  handles, el hit-test, y las funciones de escalar y rotar a partir del puntero.
- **`cs-ui::transform`:** un `impl Editor` aparte, con el estado `Floating`. Se ocupa de levantar,
  mover, escalar y rotar, de la vista previa (sobre un clon del documento, como los gestos de
  hoy), de confirmar, cancelar y sincronizar, y de Esc.
- **`editor.rs`:** solo los ganchos. Son el puntero, los puntos de confirmación y undo.
- **`canvas.rs`:** dibuja los handles y el cursor de rotación, elige cursores y decide qué handle
  se tocó, porque conoce el zoom.

**Tech Stack:** Rust 2024, egui/eframe 0.36.

**Spec:** `docs/superpowers/specs/2026-10-22-selection-transform-design.md`.

## Global Constraints

- **Crates del motor:** `cs-core`, `cs-raster`, `cs-commands`, `cs-tools` y `cs-io` no dependen de
  egui y compilan en wasm32. Los algoritmos de escala y rotación van en `cs-raster`, con TDD.
- **Archivos grandes:** `editor.rs` ya es muy grande. La lógica nueva va en módulos `impl Editor`
  aparte (`transform.rs`). En `editor.rs` solo se tocan los ganchos.
- **Idiomas:** código y comentarios en español; textos de la UI en inglés. Imita la densidad de
  comentarios del código vecino.
- **egui 0.36:**
  - `CentralPanel::show` recibe `&mut Ui`.
  - Ctrl+C/X/V llegan como `Event::Copy`/`Cut`/`Paste`.
  - Cada glifo tiene que existir en las fuentes por defecto. Se revisa en
    `$CARGO_HOME/registry/src/*/epaint_default_fonts-0.36.*`. ⤓, ▾ y ⋯ no existen. Este plan
    solo usa `×`, `°` y `·`, que están en `Ubuntu-Light.ttf`. El cursor de rotación se dibuja con
    líneas, sin glifos.
- **Toolchain aislado:** antes de cualquier comando cargo, en Git Bash:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH`
- **Verificación por tarea:** `cargo fmt --all` y los tests del crate tocado.
- **Verificación final:**
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo check -p cs-ui --target wasm32-unknown-unknown`
- **GUI:**
  - Compilar solo en `CARGO_TARGET_DIR=target/gui-transform`, nunca en el target por defecto.
  - Antes de abrir la app, copiar `%APPDATA%/ChimpSprite/data/app.ron` y restaurarlo al
    terminar.
- **Commits:** terminan con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Sin push ni PR:** el usuario junta todas las ramas al final.
- **Rama:** `selection-transform`. No cambiar de rama.

## Desviaciones/decisiones (tomadas al planear)

- **`Block<P>` es genérico** (`P: Copy`, más `PartialEq` para RotSprite) en vez de
  `Block` sobre `Rgba`.
  - El editor usa `Block<PixelValue>`. En modo indexado lo levantado conserva los índices exactos,
    sin cuantizar dos veces.
  - Pegar convierte los colores al levantar, igual que hoy, con `nearest_index`.
  - Los algoritmos siguen sin saber del modo de color.
- **`scale_nearest(src, width: i32, height: i32)`:** recibe tamaños con signo. Un tamaño negativo
  es un volteo, y 0 se trata como 1.
- **`Floating`:**
  - En vez de `rect` + `angle` usa `bbox: TransformBox { x0, y0, x1, y1, angle }`, que son los
    bordes donde caen el lado izquierdo/superior y el derecho/inferior del original. Con
    `x1 < x0` hay volteo horizontal.
  - Suma estos campos: `before` (la selección de antes de levantar), `canvas` (el tamaño del
    lienzo), y la caché `rendered`/`at`/`rendered_with`.
- **Límite de RotSprite:** RotSprite trabaja a ×8, o sea 64 veces más píxeles. Por eso se limita
  a bloques de hasta 256×256 (`ROTSPRITE_MAX_AREA`). Los más grandes se rotan con Fast.
- **Giros exactos:** un ángulo a menos de 1e-4 rad de un múltiplo de 90° se resuelve con una
  permutación, en Fast y en RotSprite.
- **Levantar es perezoso:** se levanta al primer cambio real del arrastre.
  - Un clic sin arrastrar dentro de la selección sigue siendo un clic normal de la herramienta de
    selección (en Replace, deselecciona), como hoy.
  - Si ya había algo flotando, ese clic no hace nada.
- **Move con selección:** ahora también flota, y se puede arrastrar desde cualquier punto del
  artboard activo, como en Aseprite.
  - Con Move, un clic sin arrastrar fuera de lo flotante lo confirma.
  - `MoveTool` (`move_tool.rs`) no cambia. Se sigue usando sin selección, en modo Tiles, en capas
    tilemap o de instancia, y editando una máscara.
- **Puntos de confirmación:**
  - El editor confirma en `set_tool`, `activate`, `select_layer`, `set_frame` y `set_selection`
    (esto cubre Deselect, Select All, Invert, Reselect y los gestos de selección). También
    confirma en `clear_selection_content` (Delete) y en `cut`.
  - `ChimpApp` confirma antes de copiar (porque `Editor::copy` es `&self`), de guardar y de
    exportar.
  - `Editor::sync_floating` corre una vez por frame, antes de los atajos. Confirma si la capa, el
    frame, el artboard o la herramienta cambiaron por otro camino (por ejemplo, ↑/↓ cambian la
    capa escribiendo `self.layer`).
  - También cancela si la capa se borró, se bloqueó o dejó de ser de imagen, o si cambió el
    tamaño del lienzo.
- **Undo y redo:**
  - Ctrl+Z mientras flota cancela la transformación, sin tocar el historial.
  - Ctrl+Y mientras flota se ignora, porque confirmar borraría el redo.
- **Esc deselecciona sin paso de undo:** la selección no está en el historial (`set_selection` no
  ejecuta comandos). Deselect funciona igual hoy.
- **Flechas:** hoy no mueven la selección (cambian frame y capa). No hay nada que adaptar.
- **Hit-test en el lienzo:** el handle o la franja de rotación los resuelve el lienzo, porque
  conoce el zoom. Después llama a `Editor::press_transform(hit, world)`, como
  `press_artboard_label`. Arrastrar dentro de la selección lo resuelve `pointer_press` en el
  editor, así se prueba sin egui.
- **Enter y Esc:** el `match` de teclas de `ChimpApp::handle_shortcuts` pasa a una función libre
  `editor_key(editor, key, shift)` en `lib.rs`. Así se puede probar, incluso desde el harness del
  lienzo.
- **Guías y grid:** no imantan los arrastres de transformación.
- **Cursores de redimensionar:** siguen la dirección del handle sin tener en cuenta el ángulo.
- **Selector de rotación:** va en el panel "Options" de la izquierda, porque la app no tiene una
  barra de opciones arriba. Como el resto de `ToolOptions`, no se guarda entre sesiones.
- **Shift en una esquina:** usa el mayor de los dos factores de escala.
- **Autosave:** guarda el documento sin lo flotante. No hace falta confirmar para eso.
- **Rama `paste-external`** (sin mergear): modifica `Editor::paste` y agrega `Clip::from_rgba`,
  `centered_in` y `paste_in_new_artboard`, que no existen en esta rama.
  - Al juntar las ramas, `paste` va a tener conflicto. Hay que quedarse con el cuerpo de esta rama
    (flotante).
  - El recorte `inside` de `paste-external` ya no hace falta: lo que cae fuera del lienzo se
    pierde al confirmar.
  - `paste_in_new_artboard` llama a `paste` y queda flotando en el artboard nuevo.

## Archivos

| Archivo | Qué hace |
| --- | --- |
| Crear `crates/cs-raster/src/transform.rs` | `Block`, `scale_nearest`, rotaciones, `RotationAlgorithm` |
| Modificar `crates/cs-raster/src/lib.rs` | `mod transform;` y reexportaciones |
| Crear `crates/cs-ui/src/transform_box.rs` | Caja, handles, hit-test, escalar y rotar (puro) |
| Crear `crates/cs-ui/src/transform.rs` | `impl Editor`: `Floating`, levantar, arrastres, confirmar, cancelar, Esc, estado |
| Modificar `crates/cs-ui/src/editor.rs` | Campos nuevos, `ToolOptions::rotation`, ganchos de confirmación, puntero, `view`, undo y tests |
| Modificar `crates/cs-ui/src/anim_ops.rs` | Confirmar en `select_layer` y `set_frame` |
| Modificar `crates/cs-ui/src/clipboard.rs` | `cut` confirma; `paste` flota; `clip_layer_ok` pasa a `pub(crate)` |
| Modificar `crates/cs-ui/src/lib.rs` | Módulos, `editor_key`, `sync_floating` por frame, copiar confirma |
| Modificar `crates/cs-ui/src/file_actions.rs` | Confirmar antes de guardar y exportar |
| Modificar `crates/cs-ui/src/canvas.rs` | Handles, hit-test, cursores, cursor de rotación, tests con el harness |
| Modificar `crates/cs-ui/src/panels.rs` | Selector `Rotation:` y estado en la barra inferior |

---

### Task 1: Bloque de píxeles y escala por vecino más cercano

**Files:**
- Create: `crates/cs-raster/src/transform.rs`
- Modify: `crates/cs-raster/src/lib.rs`

**Interfaces:**
- Produces:
  - `cs_raster::Block<P> { pub width: u32, pub height: u32, pub pixels: Vec<Option<P>> }`.
  - `Block::new(width: u32, height: u32) -> Block<P>`.
  - `Block::get(&self, x: i32, y: i32) -> Option<P>`.
  - `Block::set(&mut self, x: i32, y: i32, value: Option<P>)`.
  - `Block::points(&self) -> impl Iterator<Item = (i32, i32, P)>`.
  - `cs_raster::scale_nearest<P: Copy>(src: &Block<P>, width: i32, height: i32) -> Block<P>`.
  - `cs_raster::MAX_BLOCK_SIDE: u32 = 4096`.

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/cs-raster/src/transform.rs`, por ahora solo con este módulo de tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn block(width: u32, height: u32, values: &[Option<u8>]) -> Block<u8> {
        Block {
            width,
            height,
            pixels: values.to_vec(),
        }
    }

    /// 1 2 .
    /// 4 5 6
    fn sample() -> Block<u8> {
        block(3, 2, &[Some(1), Some(2), None, Some(4), Some(5), Some(6)])
    }

    #[test]
    fn block_get_set_and_points() {
        let mut b = Block::<u8>::new(2, 2);
        b.set(1, 0, Some(7));
        b.set(5, 5, Some(9)); // fuera: no hace nada
        assert_eq!(b.get(1, 0), Some(7));
        assert_eq!(b.get(-1, 0), None);
        assert_eq!(b.get(0, 0), None);
        assert_eq!(b.points().collect::<Vec<_>>(), vec![(1, 0, 7)]);
    }

    #[test]
    fn scaling_by_two_repeats_each_pixel() {
        let src = block(2, 1, &[Some(1), Some(2)]);
        let out = scale_nearest(&src, 4, 2);
        assert_eq!((out.width, out.height), (4, 2));
        assert_eq!(out.pixels, [1, 1, 2, 2, 1, 1, 2, 2].map(Some).to_vec());
    }

    #[test]
    fn negative_sizes_flip() {
        let x = scale_nearest(&sample(), -3, 2);
        assert_eq!(x.pixels, vec![None, Some(2), Some(1), Some(6), Some(5), Some(4)]);
        let y = scale_nearest(&sample(), 3, -2);
        assert_eq!(y.pixels, vec![Some(4), Some(5), Some(6), Some(1), Some(2), None]);
    }

    #[test]
    fn same_size_is_identity_and_zero_becomes_one() {
        assert_eq!(scale_nearest(&sample(), 3, 2), sample());
        let tiny = scale_nearest(&sample(), 0, 0);
        assert_eq!((tiny.width, tiny.height), (1, 1));
        assert_eq!(tiny.get(0, 0), Some(5), "the center of the original");
    }

    #[test]
    fn halving_takes_every_other_pixel() {
        let src = block(4, 1, &[Some(1), Some(2), Some(3), Some(4)]);
        assert_eq!(scale_nearest(&src, 2, 1).pixels, vec![Some(2), Some(4)]);
    }
}
```

En `crates/cs-raster/src/lib.rs`, después de `mod symmetry;`:

```rust
mod transform;
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-raster transform::`
Expected: FAIL de compilación con "cannot find type `Block` in this scope" y "cannot find function
`scale_nearest`".

- [ ] **Step 3: Implementar**

Arriba de `crates/cs-raster/src/transform.rs`, antes de los tests:

```rust
//! Escalar y rotar bloques de píxeles (la selección flotante). Funciones puras: no saben del modo
//! de color ni de la UI.

/// Lado máximo de un bloque transformado; más grande no se calcula.
pub const MAX_BLOCK_SIDE: u32 = 4096;

/// Bloque rectangular de píxeles. `None` es un punto fuera de la máscara: la forma de una
/// selección no rectangular viaja con los píxeles.
#[derive(Clone, Debug, PartialEq)]
pub struct Block<P> {
    pub width: u32,
    pub height: u32,
    /// Fila por fila.
    pub pixels: Vec<Option<P>>,
}

impl<P: Copy> Block<P> {
    /// Bloque vacío (todo fuera de la máscara).
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![None; (width * height) as usize],
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32)
            .then(|| (y as u32 * self.width + x as u32) as usize)
    }

    /// Fuera del bloque es `None`.
    pub fn get(&self, x: i32, y: i32) -> Option<P> {
        self.index(x, y).and_then(|i| self.pixels[i])
    }

    /// Fuera del bloque no hace nada.
    pub fn set(&mut self, x: i32, y: i32, value: Option<P>) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = value;
        }
    }

    /// Puntos con valor, en coordenadas del bloque.
    pub fn points(&self) -> impl Iterator<Item = (i32, i32, P)> + '_ {
        let w = self.width.max(1) as usize;
        self.pixels
            .iter()
            .enumerate()
            .filter_map(move |(i, v)| v.map(|v| ((i % w) as i32, (i / w) as i32, v)))
    }
}

/// Escala por vecino más cercano con muestreo inverso (el centro de cada píxel de destino).
/// Un tamaño negativo voltea ese eje; 0 cuenta como 1.
pub fn scale_nearest<P: Copy>(src: &Block<P>, width: i32, height: i32) -> Block<P> {
    let (w, h) = (width.unsigned_abs().max(1), height.unsigned_abs().max(1));
    let mut out = Block::new(w, h);
    if src.width == 0 || src.height == 0 {
        return out;
    }
    let sample = |d: u32, size: u32, src_size: u32| {
        ((d as u64 * 2 + 1) * src_size as u64 / (size as u64 * 2)) as i32
    };
    for y in 0..h {
        let sy = sample(y, h, src.height);
        let sy = if height < 0 { src.height as i32 - 1 - sy } else { sy };
        for x in 0..w {
            let sx = sample(x, w, src.width);
            let sx = if width < 0 { src.width as i32 - 1 - sx } else { sx };
            out.set(x as i32, y as i32, src.get(sx, sy));
        }
    }
    out
}
```

En `crates/cs-raster/src/lib.rs`, después de `pub use symmetry::{...};`:

```rust
pub use transform::{Block, MAX_BLOCK_SIDE, scale_nearest};
```

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all` y `cargo test -p cs-raster`
Expected: PASS (los 5 tests nuevos y los de siempre).

- [ ] **Step 5: Commit**

```bash
git add crates/cs-raster/src/transform.rs crates/cs-raster/src/lib.rs
git commit -m "cs-raster: bloque de píxeles y escala por vecino más cercano con volteos

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Rotación Fast y RotSprite

**Files:**
- Modify: `crates/cs-raster/src/transform.rs`
- Modify: `crates/cs-raster/src/lib.rs`

**Interfaces:**
- Consumes: `Block<P>`, `Block::{new, get, set, points}` (Task 1).
- Produces:
  - `cs_raster::rotated_size(width: u32, height: u32, radians: f32) -> (u32, u32)`.
  - `cs_raster::rotate_fast<P: Copy>(src: &Block<P>, radians: f32) -> (Block<P>, (i32, i32))`.
  - `cs_raster::rotsprite<P: Copy + PartialEq>(src: &Block<P>, radians: f32) -> (Block<P>, (i32, i32))`.
  - `cs_raster::RotationAlgorithm { Fast, RotSprite }` (`Default` = `RotSprite`), con
    `ALL: [RotationAlgorithm; 2]`, `label(self) -> &'static str` y
    `rotate<P: Copy + PartialEq>(self, src: &Block<P>, radians: f32) -> (Block<P>, (i32, i32))`.
  - `cs_raster::ROTSPRITE_MAX_AREA: u32 = 65_536`.
  - El segundo valor de cada rotación es el corrimiento de la esquina del bloque rotado respecto
    de la esquina del original, para que el centro quede en su lugar.
  - El ángulo positivo gira en sentido horario (con y hacia abajo).

- [ ] **Step 1: Escribir los tests que fallan**

Al módulo `tests` de `crates/cs-raster/src/transform.rs`, agregar arriba
`use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};` y estos tests:

```rust
    #[test]
    fn rotated_sizes() {
        assert_eq!(rotated_size(4, 2, 0.0), (4, 2));
        assert_eq!(rotated_size(4, 2, FRAC_PI_2), (2, 4));
        assert_eq!(rotated_size(4, 2, PI), (4, 2));
        assert_eq!(rotated_size(4, 2, -FRAC_PI_2), (2, 4));
        assert_eq!(rotated_size(4, 2, FRAC_PI_4), (5, 5));
    }

    #[test]
    fn zero_degrees_changes_nothing() {
        for algorithm in RotationAlgorithm::ALL {
            assert_eq!(algorithm.rotate(&sample(), 0.0), (sample(), (0, 0)));
            assert_eq!(algorithm.rotate(&sample(), TAU), (sample(), (0, 0)));
        }
    }

    #[test]
    fn quarter_turns_are_exact_and_carry_the_mask() {
        for algorithm in RotationAlgorithm::ALL {
            // Horario: la primera fila (1 2 .) queda como última columna.
            let (cw, offset) = algorithm.rotate(&sample(), FRAC_PI_2);
            assert_eq!((cw.width, cw.height), (2, 3));
            assert_eq!(cw.pixels, vec![Some(4), Some(1), Some(5), Some(2), Some(6), None]);
            assert_eq!(offset, (0, -1), "the center stays in place");
            let (half, offset) = algorithm.rotate(&sample(), PI);
            assert_eq!(half.pixels, vec![Some(6), Some(5), Some(4), None, Some(2), Some(1)]);
            assert_eq!(offset, (0, 0));
            let (ccw, _) = algorithm.rotate(&sample(), -FRAC_PI_2);
            assert_eq!(ccw.pixels, vec![None, Some(6), Some(2), Some(5), Some(1), Some(4)]);
        }
    }

    #[test]
    fn fast_rotation_is_clockwise() {
        let line = block(5, 1, &[Some(1), Some(2), Some(3), Some(4), Some(5)]);
        let (out, offset) = rotate_fast(&line, 30f32.to_radians());
        assert_eq!((out.width, out.height), (5, 4));
        assert_eq!(offset, (0, -2));
        assert_eq!(out.get(4, 3), Some(5), "the right end goes down");
        assert_eq!(out.get(0, 1), Some(1), "the left end goes up");
    }

    #[test]
    fn scale2x_rounds_diagonals() {
        // A .
        // . A
        let src = block(2, 2, &[Some(1), None, None, Some(1)]);
        let big = scale2x(&src);
        assert_eq!(big.get(2, 1), Some(1), "the gap between the two pixels is filled");
        assert_eq!(big.get(3, 0), None);
    }

    #[test]
    fn rotsprite_line_at_45_degrees_has_no_loose_pixels() {
        let line = block(8, 1, &[Some(1); 8]);
        let (out, _) = rotsprite(&line, FRAC_PI_4);
        let filled: Vec<(i32, i32)> = out.points().map(|(x, y, _)| (x, y)).collect();
        assert!(filled.len() >= 6, "{filled:?}");
        for &(x, y) in &filled {
            let touches = filled
                .iter()
                .any(|&(a, b)| (a, b) != (x, y) && (a - x).abs() <= 1 && (b - y).abs() <= 1);
            assert!(touches, "loose pixel at ({x}, {y})");
        }
    }

    #[test]
    fn big_blocks_use_fast() {
        let mut b = Block::<u8>::new(257, 256);
        b.set(0, 0, Some(1));
        b.set(200, 100, Some(2));
        assert_eq!(rotsprite(&b, 0.5), rotate_fast(&b, 0.5));
    }
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-raster transform::`
Expected: FAIL de compilación con "cannot find function `rotated_size`" (y `rotate_fast`,
`rotsprite`, `scale2x`, `RotationAlgorithm`).

- [ ] **Step 3: Implementar**

Arriba de `crates/cs-raster/src/transform.rs`, después del comentario `//!`:

```rust
use std::f32::consts::{FRAC_PI_2, TAU};
```

Después de `scale_nearest`:

```rust
/// Área máxima (en píxeles del bloque original) para RotSprite: trabaja a ×8, con 64 veces más
/// píxeles. Los bloques más grandes se rotan con Fast.
pub const ROTSPRITE_MAX_AREA: u32 = 256 * 256;

/// Cómo se rota la selección (opción de las herramientas de selección y Move).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RotationAlgorithm {
    /// Vecino más cercano: rápido, con bordes dentados.
    Fast,
    /// Scale2x ×8, rotación y reducción: líneas más limpias en pixel art.
    #[default]
    RotSprite,
}

impl RotationAlgorithm {
    pub const ALL: [RotationAlgorithm; 2] = [RotationAlgorithm::Fast, RotationAlgorithm::RotSprite];

    pub fn label(self) -> &'static str {
        match self {
            RotationAlgorithm::Fast => "Fast",
            RotationAlgorithm::RotSprite => "RotSprite",
        }
    }

    pub fn rotate<P: Copy + PartialEq>(self, src: &Block<P>, radians: f32) -> (Block<P>, (i32, i32)) {
        match self {
            RotationAlgorithm::Fast => rotate_fast(src, radians),
            RotationAlgorithm::RotSprite => rotsprite(src, radians),
        }
    }
}

/// Cuartos de vuelta (0–3) si el ángulo es un múltiplo de 90°.
fn quarter_turns(radians: f32) -> Option<u32> {
    let turns = radians.rem_euclid(TAU) / FRAC_PI_2;
    let q = turns.round();
    ((turns - q).abs() < 1e-4).then_some(q as u32 % 4)
}

/// Tamaño del rectángulo que contiene al bloque rotado.
pub fn rotated_size(width: u32, height: u32, radians: f32) -> (u32, u32) {
    match quarter_turns(radians) {
        Some(q) if q % 2 == 0 => (width, height),
        Some(_) => (height, width),
        None => {
            let (s, c) = radians.sin_cos();
            let (w, h) = (width as f32, height as f32);
            let side = |v: f32| (v - 1e-3).ceil().max(1.0) as u32;
            (side(w * c.abs() + h * s.abs()), side(w * s.abs() + h * c.abs()))
        }
    }
}

/// Corrimiento de la esquina para que el centro del bloque rotado quede en su lugar.
fn centered_offset(width: u32, height: u32, rotated: (u32, u32)) -> (i32, i32) {
    (
        (width as i32 - rotated.0 as i32).div_euclid(2),
        (height as i32 - rotated.1 as i32).div_euclid(2),
    )
}

/// Giro exacto de `turns` cuartos de vuelta en sentido horario (una permutación).
fn quarter<P: Copy>(src: &Block<P>, turns: u32) -> (Block<P>, (i32, i32)) {
    let (w, h) = (src.width as i32, src.height as i32);
    let (rw, rh) = if turns % 2 == 0 { (w, h) } else { (h, w) };
    let mut out = Block::new(rw as u32, rh as u32);
    for y in 0..rh {
        for x in 0..rw {
            let (sx, sy) = match turns {
                1 => (y, h - 1 - x),
                2 => (w - 1 - x, h - 1 - y),
                3 => (w - 1 - y, x),
                _ => (x, y),
            };
            out.set(x, y, src.get(sx, sy));
        }
    }
    let offset = centered_offset(src.width, src.height, (rw as u32, rh as u32));
    (out, offset)
}

/// Rotación por muestreo inverso: `sample` recibe el centro de cada píxel de destino en
/// coordenadas del bloque original (con decimales).
fn rotate_with<P: Copy>(
    width: u32,
    height: u32,
    radians: f32,
    sample: impl Fn(f32, f32) -> Option<P>,
) -> (Block<P>, (i32, i32)) {
    let size = rotated_size(width, height, radians);
    let (dx, dy) = centered_offset(width, height, size);
    let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
    let (s, c) = radians.sin_cos();
    let mut out = Block::new(size.0, size.1);
    for y in 0..size.1 as i32 {
        for x in 0..size.0 as i32 {
            let (px, py) = ((dx + x) as f32 + 0.5 - cx, (dy + y) as f32 + 0.5 - cy);
            // Rotación inversa (positivo = horario con y hacia abajo).
            let (sx, sy) = (c * px + s * py + cx, -s * px + c * py + cy);
            out.set(x, y, sample(sx, sy));
        }
    }
    (out, (dx, dy))
}

/// Rotación Fast: vecino más cercano. Los múltiplos de 90° son exactos.
pub fn rotate_fast<P: Copy>(src: &Block<P>, radians: f32) -> (Block<P>, (i32, i32)) {
    if let Some(q) = quarter_turns(radians) {
        return quarter(src, q);
    }
    rotate_with(src.width, src.height, radians, |x, y| {
        src.get(x.floor() as i32, y.floor() as i32)
    })
}

/// Scale2x (EPX) comparando valores exactos; fuera del bloque es `None` (transparente).
fn scale2x<P: Copy + PartialEq>(src: &Block<P>) -> Block<P> {
    let mut out = Block::new(src.width * 2, src.height * 2);
    for y in 0..src.height as i32 {
        for x in 0..src.width as i32 {
            let p = src.get(x, y);
            let (a, b, c, d) = (src.get(x, y - 1), src.get(x + 1, y), src.get(x - 1, y), src.get(x, y + 1));
            let e0 = if c == a && c != d && a != b { a } else { p };
            let e1 = if a == b && a != c && b != d { b } else { p };
            let e2 = if d == c && d != b && c != a { c } else { p };
            let e3 = if b == d && b != a && d != c { d } else { p };
            out.set(2 * x, 2 * y, e0);
            out.set(2 * x + 1, 2 * y, e1);
            out.set(2 * x, 2 * y + 1, e2);
            out.set(2 * x + 1, 2 * y + 1, e3);
        }
    }
    out
}

/// RotSprite: Scale2x tres veces (×8), rotación Fast a ×8 y reducción tomando el píxel central de
/// cada bloque de 8×8 (muestrear el ×8 en el centro de cada píxel de destino es lo mismo).
pub fn rotsprite<P: Copy + PartialEq>(src: &Block<P>, radians: f32) -> (Block<P>, (i32, i32)) {
    if let Some(q) = quarter_turns(radians) {
        return quarter(src, q);
    }
    if src.width * src.height > ROTSPRITE_MAX_AREA {
        return rotate_fast(src, radians);
    }
    let big = scale2x(&scale2x(&scale2x(src)));
    rotate_with(src.width, src.height, radians, |x, y| {
        big.get((x * 8.0).floor() as i32, (y * 8.0).floor() as i32)
    })
}
```

En `crates/cs-raster/src/lib.rs`, reemplazar la línea de Task 1 por:

```rust
pub use transform::{
    Block, MAX_BLOCK_SIDE, ROTSPRITE_MAX_AREA, RotationAlgorithm, rotate_fast, rotated_size,
    rotsprite, scale_nearest,
};
```

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all`, `cargo test -p cs-raster` y
`cargo check -p cs-raster --target wasm32-unknown-unknown`
Expected: PASS; wasm compila.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-raster/src
git commit -m "cs-raster: rotación Fast y RotSprite con giros exactos de 90°

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Estado flotante en el editor (levantar, confirmar, cancelar)

**Files:**
- Create: `crates/cs-ui/src/transform_box.rs`
- Create: `crates/cs-ui/src/transform.rs`
- Modify: `crates/cs-ui/src/lib.rs` (módulos)
- Modify: `crates/cs-ui/src/editor.rs` (campo, opción, ganchos, `view`, undo)
- Modify: `crates/cs-ui/src/anim_ops.rs` (`select_layer`, `set_frame`)
- Modify: `crates/cs-ui/src/clipboard.rs` (`clip_layer_ok`, `cut`)

**Interfaces:**
- Consumes: `cs_raster::{Block, MAX_BLOCK_SIDE, RotationAlgorithm, scale_nearest}` (Tasks 1 y 2).
- Produces:
  - `crate::transform_box::TransformBox { pub x0: i32, pub y0: i32, pub x1: i32, pub y1: i32, pub angle: f32 }`,
    con `new(x: i32, y: i32, width: u32, height: u32)`, `width() -> i32` y `height() -> i32`
    (con signo), `left() -> i32`, `top() -> i32` y `translated(dx: i32, dy: i32) -> Self`.
  - `crate::transform::Floating` (campos privados; `bbox: TransformBox`).
  - `Editor::floating(&self) -> Option<&Floating>` y `Editor::is_floating(&self) -> bool`.
  - `Editor::can_transform(&self) -> bool`.
  - `Editor::transform_box(&self) -> Option<TransformBox>`.
  - `Editor::lift(&mut self) -> bool`.
  - `Editor::refresh_floating(&mut self, fast: bool)`.
  - `Editor::commit_floating(&mut self) -> bool` y `Editor::cancel_floating(&mut self) -> bool`.
  - `Editor::sync_floating(&mut self)`.
  - `Editor::floating_view(&self) -> Option<DocView<'_>>` (`pub(crate)`).
  - `ToolOptions::rotation: RotationAlgorithm`.
  - `Editor::clip_layer_ok(&self, write: bool) -> bool` pasa a `pub(crate)`.
  - Campo `Editor::floating: Option<Floating>` (`pub(crate)`).

- [ ] **Step 1: Crear la caja y escribir los tests que fallan**

Crear `crates/cs-ui/src/transform_box.rs`:

```rust
//! Geometría de la caja de transformación: dónde caen los lados del original, con signo (volteos)
//! y ángulo. Pura: sin egui, en píxeles locales del artboard.

/// Caja de la transformación. `x0`/`y0` son el borde donde cae el lado izquierdo/superior del
/// original y `x1`/`y1` el del derecho/inferior (exclusivos). Con `x1 < x0` el contenido está
/// volteado. El ángulo (radianes, positivo = horario) gira alrededor del centro.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformBox {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub angle: f32,
}

impl TransformBox {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x0: x,
            y0: y,
            x1: x + width as i32,
            y1: y + height as i32,
            angle: 0.0,
        }
    }

    /// Ancho con signo (negativo = volteado).
    pub fn width(&self) -> i32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> i32 {
        self.y1 - self.y0
    }

    /// Lado izquierdo en pantalla (sin rotar).
    pub fn left(&self) -> i32 {
        self.x0.min(self.x1)
    }

    pub fn top(&self) -> i32 {
        self.y0.min(self.y1)
    }

    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        Self {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
            angle: self.angle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_sides_and_flips() {
        let b = TransformBox::new(2, 3, 4, 5);
        assert_eq!((b.width(), b.height(), b.left(), b.top()), (4, 5, 2, 3));
        let flipped = TransformBox { x0: 6, x1: 2, ..b };
        assert_eq!((flipped.width(), flipped.left()), (-4, 2));
        assert_eq!(b.translated(1, -1), TransformBox::new(3, 2, 4, 5));
    }
}
```

Crear `crates/cs-ui/src/transform.rs`, por ahora solo con los tests:

```rust
#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, LayerKind, Mask, PixelValue, Rgba, Sprite};

    use super::*;
    use crate::editor::ToolKind;
    use crate::transform_box::TransformBox;

    fn red() -> PixelValue {
        PixelValue::Rgba(Rgba::rgb(255, 0, 0))
    }

    /// 8×8 con un punto rojo en (1, 1), la selección (0, 0)–(2, 2) y una herramienta de selección.
    fn editor() -> Editor {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red())],
        });
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
        e.tool = ToolKind::RectSelect;
        e
    }

    fn px(e: &Editor, x: i32, y: i32) -> PixelValue {
        e.sprite().pixel(LayerId(1), 0, x, y)
    }

    /// Lo que muestra el lienzo (con lo flotante aplicado).
    fn shown(e: &Editor, x: i32, y: i32) -> PixelValue {
        e.view().document.artboards[0].sprite.pixel(LayerId(1), 0, x, y)
    }

    fn place(e: &mut Editor, bbox: TransformBox) {
        e.floating.as_mut().expect("floating").bbox = bbox;
        e.refresh_floating(false);
    }

    fn moved(e: &mut Editor, dx: i32, dy: i32) {
        let b = e.transform_box().unwrap().translated(dx, dy);
        place(e, b);
    }

    #[test]
    fn lifting_and_moving_only_changes_the_view_until_commit() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        moved(&mut e, 3, 2);
        assert_eq!(px(&e, 1, 1), red(), "the layer is untouched");
        assert_eq!(shown(&e, 1, 1), t);
        assert_eq!(shown(&e, 4, 3), red());
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((3, 2), (5, 4))));
        let version = e.doc_version;
        assert!(e.commit_floating());
        assert_eq!(e.doc_version, version + 1, "a single command");
        assert_eq!(px(&e, 4, 3), red());
        assert_eq!(px(&e, 1, 1), t);
        e.undo();
        assert_eq!(px(&e, 1, 1), red());
        assert_eq!(px(&e, 4, 3), t);
    }

    #[test]
    fn cancel_leaves_no_trace_and_restores_the_selection() {
        let mut e = editor();
        let before = e.selection.clone();
        let version = e.doc_version;
        assert!(e.lift());
        moved(&mut e, 1, 1);
        assert!(e.cancel_floating());
        assert!(!e.is_floating());
        assert_eq!(e.doc_version, version);
        assert_eq!(e.selection, before);
        assert_eq!(shown(&e, 1, 1), red());
        assert!(!e.cancel_floating(), "nothing left to cancel");
    }

    #[test]
    fn scaling_doubles_pixels_and_commit_selects_the_result() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        place(&mut e, TransformBox::new(0, 0, 6, 6));
        assert!(e.commit_floating());
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            assert_eq!(px(&e, x, y), red(), "({x}, {y})");
        }
        assert_eq!(px(&e, 1, 1), t);
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((0, 0), (5, 5))));
    }

    #[test]
    fn an_untouched_lift_commits_nothing() {
        let mut e = editor();
        let version = e.doc_version;
        assert!(e.lift());
        assert!(e.commit_floating());
        assert_eq!(e.doc_version, version, "no empty undo step");
    }

    #[test]
    fn changing_layer_commits_and_locking_the_layer_cancels() {
        let mut e = editor();
        e.add_layer(LayerKind::Image);
        let top = e.layer;
        e.select_layer(LayerId(1));
        assert!(e.lift());
        moved(&mut e, 4, 0);
        e.select_layer(top);
        assert!(!e.is_floating());
        assert_eq!(px(&e, 5, 1), red(), "committed on the layer it was lifted from");

        e.select_layer(LayerId(1));
        e.set_selection(Some(Mask::from_rect(8, 8, (4, 0), (6, 2))));
        assert!(e.lift());
        moved(&mut e, 0, 3);
        e.toggle_locked(LayerId(1));
        e.sync_floating();
        assert!(!e.is_floating());
        assert_eq!(px(&e, 5, 1), red(), "nothing moved");
        assert!(!e.lift(), "a locked layer cannot be lifted");
    }

    #[test]
    fn changing_tool_commits_and_undo_while_floating_only_cancels() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        moved(&mut e, 2, 0);
        e.undo();
        assert!(!e.is_floating());
        assert_eq!(px(&e, 1, 1), red(), "the drawing is still there");
        assert!(e.lift());
        moved(&mut e, 2, 0);
        e.set_tool(ToolKind::Pencil);
        assert!(!e.is_floating());
        assert_eq!(px(&e, 3, 1), red());
        e.undo();
        e.undo();
        assert_eq!(px(&e, 1, 1), t, "the next undos undo the move and the drawing");
    }
}
```

En `crates/cs-ui/src/lib.rs`, en la lista de módulos, después de `mod tilemap_dialog;`:

```rust
mod timeline;
mod transform;
mod transform_box;
```

(Reemplaza la línea `mod timeline;` existente; el orden alfabético queda igual.)

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui transform`
Expected: FAIL de compilación con "no method named `lift` found for struct `Editor`" (y
`refresh_floating`, `commit_floating`, el campo `floating`, etc.).

- [ ] **Step 3: Implementar el módulo**

Arriba de `crates/cs-ui/src/transform.rs`, antes de los tests:

```rust
//! Transformar la selección. Lo seleccionado se levanta una sola vez y flota (se mueve, se escala,
//! se rota) hasta que se confirma, en un solo paso de undo, o se cancela sin dejar rastro. Lo que
//! queda debajo mientras flota no se mezcla ni se lleva en los arrastres siguientes.

use std::borrow::Cow;

use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, LayerId, LayerKind, Mask, PixelValue, Sprite};
use cs_raster::{Block, MAX_BLOCK_SIDE, RotationAlgorithm};

use crate::editor::{DocView, Editor, ToolKind};
use crate::transform_box::TransformBox;

/// Contenido levantado de una capa mientras se transforma.
#[derive(Clone, Debug)]
pub struct Floating {
    /// Píxeles levantados, sin transformar (`None` = fuera de la máscara).
    source: Block<PixelValue>,
    /// Esquina de la selección original (o de lo pegado).
    origin: (i32, i32),
    /// Destino: escala, volteos y ángulo.
    bbox: TransformBox,
    layer: LayerId,
    frame: usize,
    artboard: ArtboardId,
    /// Viene de pegar: no deja hueco en el origen.
    from_paste: bool,
    /// Selección de antes de levantar; cancelar vuelve a ella.
    before: Option<Mask>,
    /// Tamaño del lienzo al levantar; si cambia, se cancela.
    canvas: (u32, u32),
    /// Resultado transformado y su esquina en el lienzo (caché hasta el siguiente cambio).
    rendered: Block<PixelValue>,
    at: (i32, i32),
    /// Rotación con la que se calculó `rendered`.
    rendered_with: RotationAlgorithm,
}

impl Floating {
    fn new(
        source: Block<PixelValue>,
        origin: (i32, i32),
        target: (ArtboardId, LayerId, usize),
        from_paste: bool,
        before: Option<Mask>,
        canvas: (u32, u32),
    ) -> Self {
        let bbox = TransformBox::new(origin.0, origin.1, source.width, source.height);
        Self {
            rendered: source.clone(),
            at: origin,
            rendered_with: RotationAlgorithm::Fast,
            source,
            origin,
            bbox,
            artboard: target.0,
            layer: target.1,
            frame: target.2,
            from_paste,
            before,
            canvas,
        }
    }

    /// Escala (con volteos) y rota lo levantado según la caja.
    fn render(&mut self, algorithm: RotationAlgorithm) {
        let b = self.bbox;
        let scaled = cs_raster::scale_nearest(&self.source, b.width(), b.height());
        let (rotated, (dx, dy)) = algorithm.rotate(&scaled, b.angle);
        self.rendered = rotated;
        self.at = (b.left() + dx, b.top() + dy);
        self.rendered_with = algorithm;
    }

    /// Sigue igual que al levantar: confirmar no cambiaría nada.
    fn unchanged(&self) -> bool {
        let (w, h) = (self.source.width, self.source.height);
        !self.from_paste && self.bbox == TransformBox::new(self.origin.0, self.origin.1, w, h)
    }

    /// Máscara de lo transformado; lo que cae fuera del lienzo se pierde.
    fn mask(&self) -> Option<Mask> {
        let (w, h) = self.canvas;
        let (ax, ay) = self.at;
        let points = self.rendered.points().map(|(x, y, _)| (ax + x, ay + y));
        let mask = Mask::from_points(w, h, points);
        (!mask.is_empty()).then_some(mask)
    }

    /// Comando que deja lo flotante en la capa: el hueco en el origen (si no viene de pegar) y el
    /// estampado, sin borrar con lo transparente.
    fn command(&self, sprite: &Sprite) -> Command {
        let clear = sprite.color_mode.transparent();
        let mut pixels = Vec::new();
        if !self.from_paste {
            let (ox, oy) = self.origin;
            pixels.extend(self.source.points().map(|(x, y, _)| (ox + x, oy + y, clear)));
        }
        let (ax, ay) = self.at;
        pixels.extend(
            self.rendered
                .points()
                .filter(|&(_, _, v)| cs_render::resolve(sprite, v).a > 0)
                .map(|(x, y, v)| (ax + x, ay + y, v)),
        );
        Command::DrawPixels {
            layer: self.layer,
            frame: self.frame,
            pixels,
        }
    }
}

impl Editor {
    pub fn floating(&self) -> Option<&Floating> {
        self.floating.as_ref()
    }

    pub fn is_floating(&self) -> bool {
        self.floating.is_some()
    }

    /// Se puede transformar: herramienta de selección o Move sobre una capa de imagen editable,
    /// fuera del modo Tiles y sin editar la máscara (como el portapapeles), con algo seleccionado
    /// o flotando.
    pub fn can_transform(&self) -> bool {
        (self.tool.is_selection() || self.tool == ToolKind::Move)
            && self.clip_layer_ok(true)
            && (self.floating.is_some() || self.selection.is_some())
    }

    /// Caja de lo flotante o, si no flota nada, el rectángulo que contiene la selección.
    pub fn transform_box(&self) -> Option<TransformBox> {
        if let Some(f) = &self.floating {
            return Some(f.bbox);
        }
        let ((x0, y0), (x1, y1)) = self.selection.as_ref()?.bounds()?;
        Some(TransformBox::new(
            x0,
            y0,
            (x1 - x0 + 1) as u32,
            (y1 - y0 + 1) as u32,
        ))
    }

    /// Levanta lo seleccionado de la capa actual. El documento no cambia hasta confirmar.
    pub fn lift(&mut self) -> bool {
        if self.floating.is_some() {
            return true;
        }
        if !self.clip_layer_ok(true) {
            return false;
        }
        let Some(mask) = self.selection.clone() else {
            return false;
        };
        let Some(((x0, y0), (x1, y1))) = mask.bounds() else {
            return false;
        };
        let sprite = self.sprite();
        let mut source = Block::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
        for (x, y) in mask.points() {
            source.set(x - x0, y - y0, Some(sprite.pixel(self.layer, self.frame, x, y)));
        }
        let canvas = (sprite.width, sprite.height);
        let target = (self.active, self.layer, self.frame);
        self.floating = Some(Floating::new(
            source,
            (x0, y0),
            target,
            false,
            Some(mask),
            canvas,
        ));
        self.revision += 1;
        true
    }

    /// Recalcula lo transformado y la selección. `fast`: rotación Fast (durante un arrastre de
    /// rotación); si no, la elegida en las opciones.
    pub fn refresh_floating(&mut self, fast: bool) {
        let algorithm = if fast {
            RotationAlgorithm::Fast
        } else {
            self.options.rotation
        };
        let Some(f) = self.floating.as_mut() else {
            return;
        };
        f.render(algorithm);
        self.selection = f.mask();
        self.revision += 1;
    }

    /// Deja lo flotante en su capa (un solo paso de undo) y selecciona lo transformado.
    pub fn commit_floating(&mut self) -> bool {
        let Some(f) = self.floating.take() else {
            return false;
        };
        let cmd = match self.document.artboard(f.artboard) {
            Some(artboard) if !f.unchanged() => Some(f.command(&artboard.sprite)),
            _ => None,
        };
        let applied = match cmd {
            Some(Command::DrawPixels { ref pixels, .. }) if pixels.is_empty() => true,
            Some(cmd) => self.execute_doc(DocCommand::OnArtboard { id: f.artboard, cmd }),
            None => true,
        };
        self.selection = if applied { f.mask() } else { f.before };
        self.revision += 1;
        true
    }

    /// Descarta lo flotante: todo vuelve a como estaba antes de levantar, sin pasos en el
    /// historial. Si venía de pegar, vuelve la selección de antes del pegado.
    pub fn cancel_floating(&mut self) -> bool {
        let Some(f) = self.floating.take() else {
            return false;
        };
        let s = self.sprite();
        let size = (s.width, s.height);
        self.selection = f.before.filter(|m| (m.width(), m.height()) == size);
        self.revision += 1;
        true
    }

    /// Una vez por frame: confirma si cambió la capa, el frame, el artboard o la herramienta por
    /// un camino que no confirma solo (p. ej. ↑/↓), y cancela si la capa ya no se puede editar o
    /// cambió el tamaño del lienzo.
    pub fn sync_floating(&mut self) {
        let Some(f) = &self.floating else {
            return;
        };
        let valid = self.document.artboard(f.artboard).is_some_and(|a| {
            let s = &a.sprite;
            (s.width, s.height) == f.canvas
                && f.frame < s.frames.len()
                && s
                    .layer(f.layer)
                    .is_some_and(|l| l.kind == LayerKind::Image && !l.locked)
        });
        let same_target = (f.artboard, f.layer, f.frame) == (self.active, self.layer, self.frame);
        let tool_ok = self.tool.is_selection() || self.tool == ToolKind::Move;
        if !valid {
            self.cancel_floating();
        } else if !same_target || !tool_ok || self.editing_mask() {
            self.commit_floating();
        }
    }

    /// Vista previa: el documento con lo flotante aplicado (hueco y estampado) en un clon.
    pub(crate) fn floating_view(&self) -> Option<DocView<'_>> {
        let f = self.floating.as_ref()?;
        let sprite = &self.document.artboard(f.artboard)?.sprite;
        let cmd = DocCommand::OnArtboard {
            id: f.artboard,
            cmd: f.command(sprite),
        };
        let mut doc = self.document.clone();
        let document = match cmd.apply(&mut doc) {
            Ok(_) => Cow::Owned(doc),
            Err(_) => Cow::Borrowed(&self.document),
        };
        Some(DocView {
            document,
            selection: self.selection.clone(),
        })
    }
}
```

- [ ] **Step 4: Ganchos en el editor**

En `crates/cs-ui/src/editor.rs`:

1. Imports. Después de `use crate::tile_ops::{self, TileProxy};` agregar:

```rust
use crate::transform::Floating;
```

2. `ToolOptions`. Después de `pub dither: Dither,` en la definición:

```rust
    /// Cómo se rota la selección (herramientas de selección y Move).
    pub rotation: cs_raster::RotationAlgorithm,
```

y en `impl Default for ToolOptions`, después de `dither: Dither::None,`:

```rust
            rotation: cs_raster::RotationAlgorithm::RotSprite,
```

3. Campo en `Editor`, después de `pub(crate) tile_limit: bool,`:

```rust
    /// Contenido levantado que se está moviendo, escalando o rotando (ver `transform.rs`).
    pub(crate) floating: Option<Floating>,
```

y en `Editor::new`, después de `tile_limit: false,`:

```rust
            floating: None,
```

4. `activate`: después del `return` del principio (antes de `self.stop();`):

```rust
        self.commit_floating();
```

5. `undo` y `redo`: al principio de cada uno.

```rust
    pub fn undo(&mut self) {
        // Mientras flota, deshacer cancela la transformación (no hay paso en el historial).
        if self.cancel_floating() {
            return;
        }
        self.merge = None;
```

```rust
    pub fn redo(&mut self) {
        // Rehacer con algo flotando lo perdería: se ignora hasta confirmar o cancelar.
        if self.floating.is_some() {
            return;
        }
        self.merge = None;
```

6. `set_selection`: la primera línea del cuerpo pasa a ser:

```rust
        // Cambiar la selección (deseleccionar, seleccionar todo, un gesto…) confirma lo flotante.
        self.commit_floating();
```

7. `set_tool`: reemplazar el cuerpo del `if`.

```rust
        if kind != self.tool {
            self.commit_floating();
            self.cancel_gesture();
            self.tool = kind;
        }
```

8. `clear_selection_content`: la primera línea del cuerpo pasa a ser:

```rust
        // Delete confirma lo flotante y después borra lo seleccionado.
        self.commit_floating();
```

9. `view()`: justo después del bloque `if self.artboard_tool.is_active() { ... }` y antes de
   `if let Some(cmd) = self.text_command() {`:

```rust
        if let Some(view) = self.floating_view() {
            return view;
        }
```

En `crates/cs-ui/src/anim_ops.rs`:

- `set_frame`: dentro del `if frame != self.frame {`, como primera línea:

```rust
            self.commit_floating();
```

- `select_layer`: dentro del `if id != self.layer {`, como primera línea:

```rust
                self.commit_floating();
```

En `crates/cs-ui/src/clipboard.rs`:

- `fn clip_layer_ok(&self, write: bool) -> bool` pasa a `pub(crate) fn clip_layer_ok(...)`.
- En `cut`, después del `if !self.clip_layer_ok(true) { return None; }`:

```rust
        self.commit_floating();
```

- [ ] **Step 5: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS. Pasan los 6 tests nuevos de `transform::tests` y el de `transform_box`, y los
tests de siempre siguen en verde.

- [ ] **Step 6: Commit**

```bash
git add crates/cs-ui/src
git commit -m "Editor: contenido flotante (levantar, vista previa, confirmar, cancelar)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Mover arrastrando deja el contenido flotando (bug de mover)

**Files:**
- Modify: `crates/cs-ui/src/transform_box.rs`
- Modify: `crates/cs-ui/src/transform.rs`
- Modify: `crates/cs-ui/src/editor.rs` (puntero, `moves_selection_at`, `temp_move` fuera, tests)

**Interfaces:**
- Consumes: `TransformBox`, `Editor::{lift, refresh_floating, commit_floating, transform_box, can_transform}` (Task 3).
- Produces:
  - `crate::transform_box::TransformHit` (`Move`; Task 7 agrega `Handle(Handle)` y `Rotate`).
  - `crate::transform::TransformDrag` (`pub(crate)`, campos privados).
  - `Editor::press_transform(&mut self, hit: TransformHit, world: (f32, f32))`.
  - `Editor::drag_transform(&mut self, world: (f32, f32))` (`pub(crate)`).
  - `Editor::release_transform(&mut self) -> Option<(i32, i32)>` (`pub(crate)`). Devuelve el píxel
    si fue un clic sin arrastrar dentro de la selección con una herramienta de selección.
  - `Editor::set_floating_box(&mut self, bbox: TransformBox, fast: bool) -> bool` (privada).
  - Campo `Editor::transform_drag: Option<TransformDrag>` (`pub(crate)`).
  - `Editor::moves_selection_at` mantiene la firma; ahora también es `true` con Move.
  - `Editor::is_painting()` es `true` durante un arrastre de transformación.

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/cs-ui/src/editor.rs`, dentro de `mod tests`, reemplazar el test
`dragging_inside_the_selection_with_a_selection_tool_moves_its_content` entero por:

```rust
    #[test]
    fn dragging_inside_the_selection_with_a_selection_tool_moves_its_content() {
        let mut e = editor();
        let red = e.primary;
        let t = e.sprite().color_mode.transparent();
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        e.set_tool(ToolKind::RectSelect);
        drag_local(&mut e, (0, 0), (2, 2), false, false);
        // Arrastrar desde dentro: el contenido flota y se mueve, sin cambiar de herramienta.
        drag_local(&mut e, (1, 1), (4, 3), false, false);
        assert_eq!(e.tool, ToolKind::RectSelect);
        assert_eq!(e.active_tool(), ToolKind::RectSelect);
        assert!(e.is_floating(), "it floats until it is applied");
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red, "not stamped yet");
        assert!(e.commit_floating());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), t);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 4, 3), red);
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((3, 2), (5, 4))));
        e.undo();
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red);
        // Con Shift (Add) dentro de la selección se selecciona, no se mueve.
        drag_local(&mut e, (4, 3), (6, 6), true, false);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red);
        assert!(e.selection.as_ref().unwrap().contains(6, 6));
        assert!(!e.is_floating());
        // Un clic sin arrastrar dentro de la selección deselecciona, como siempre.
        click(&mut e, (4, 4), false, false);
        assert!(e.selection.is_none());
    }

    #[test]
    fn moving_the_selection_twice_only_carries_what_was_selected_first() {
        // Bug del 2026-10-23: cada arrastre estampaba en la capa y el siguiente levantaba también
        // el dibujo que había quedado dentro de la selección.
        let mut e = editor();
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let blue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
        let t = e.sprite().color_mode.transparent();
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red), (5, 1, blue)],
        });
        e.set_tool(ToolKind::RectSelect);
        drag_local(&mut e, (0, 0), (2, 2), false, false);
        drag_local(&mut e, (1, 1), (5, 1), false, false); // el rojo pasa encima del azul
        let shown = e.view().document.artboards[0].sprite.pixel(LayerId(1), 0, 5, 1);
        assert_eq!(shown, red, "the floating content is drawn on top");
        drag_local(&mut e, (5, 1), (5, 5), false, false); // y se sigue moviendo
        assert!(e.commit_floating());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 5), red);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 1), blue, "the drawing below stays");
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), t);
        e.undo();
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red, "a single undo step");
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 5), t);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 1), blue);
    }

    #[test]
    fn the_move_tool_floats_the_selection_and_a_click_outside_applies_it() {
        let mut e = editor();
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
        e.set_tool(ToolKind::Move);
        drag_local(&mut e, (6, 6), (7, 6), false, false); // desde fuera de la selección
        assert!(e.is_floating());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red, "not stamped yet");
        click(&mut e, (6, 0), false, false); // fuera de lo flotante
        assert!(!e.is_floating());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 1), red);
    }
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui editor::tests`
Expected: FAIL en los tres tests nuevos o reescritos. Fallan en `assert!(e.is_floating())`, o en
el píxel azul de `moving_the_selection_twice_only_carries_what_was_selected_first`, porque hoy
cada arrastre estampa. El resto pasa.

- [ ] **Step 3: Implementar el arrastre**

En `crates/cs-ui/src/transform_box.rs`, antes de `#[cfg(test)]`:

```rust
/// Qué se arrastra sobre lo seleccionado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformHit {
    /// Dentro de la selección (o cualquier punto con Move): mover.
    Move,
}
```

En `crates/cs-ui/src/transform.rs`, cambiar el import de la caja por:

```rust
use crate::transform_box::{TransformBox, TransformHit};
```

y agregar, después de `impl Floating { ... }`:

```rust
/// Arrastre en curso sobre lo seleccionado o sobre la caja de transformación.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TransformDrag {
    hit: TransformHit,
    /// Píxel local donde se presionó.
    start_pixel: (i32, i32),
    /// Caja al presionar: cada movimiento se calcula desde acá.
    start_box: TransformBox,
    /// Se presionó dentro de lo seleccionado.
    inside: bool,
    /// El arrastre ya cambió la caja (y lo seleccionado ya flota).
    moved: bool,
}
```

Dentro de `impl Editor` (en `transform.rs`), agregar:

```rust
    /// Empieza a mover, escalar o rotar lo seleccionado. Se levanta recién al cambiar algo, así
    /// un clic no levanta nada.
    pub fn press_transform(&mut self, hit: TransformHit, world: (f32, f32)) {
        let Some(start_box) = self.transform_box() else {
            return;
        };
        self.stop();
        let start_pixel = self.to_local(world);
        let inside = self
            .selection
            .as_ref()
            .is_some_and(|m| m.contains(start_pixel.0, start_pixel.1));
        self.transform_drag = Some(TransformDrag {
            hit,
            start_pixel,
            start_box,
            inside,
            moved: false,
        });
        self.revision += 1;
    }

    pub(crate) fn drag_transform(&mut self, world: (f32, f32)) {
        let Some(mut drag) = self.transform_drag else {
            return;
        };
        let pixel = self.to_local(world);
        let bbox = match drag.hit {
            TransformHit::Move => drag.start_box.translated(
                pixel.0 - drag.start_pixel.0,
                pixel.1 - drag.start_pixel.1,
            ),
        };
        if Some(bbox) == self.transform_box() {
            return;
        }
        if !drag.moved {
            // Se levanta una sola vez, al primer cambio.
            if !self.lift() {
                self.transform_drag = None;
                return;
            }
            drag.moved = true;
            self.transform_drag = Some(drag);
        }
        self.set_floating_box(bbox, false);
    }

    /// Termina el arrastre. Devuelve el píxel presionado si fue un clic sin arrastrar dentro de la
    /// selección con una herramienta de selección y sin nada flotando: es un clic normal de esa
    /// herramienta (en Replace, deselecciona).
    pub(crate) fn release_transform(&mut self) -> Option<(i32, i32)> {
        let drag = self.transform_drag.take()?;
        self.revision += 1;
        if drag.moved {
            // Durante el arrastre la rotación usa Fast; al soltar se calcula la elegida una vez.
            if self
                .floating
                .as_ref()
                .is_some_and(|f| f.rendered_with != self.options.rotation)
            {
                self.refresh_floating(false);
            }
            return None;
        }
        if self.floating.is_some() {
            // Con Move, un clic fuera de lo flotante lo confirma.
            if self.tool == ToolKind::Move && !drag.inside {
                self.commit_floating();
            }
            return None;
        }
        (self.tool.is_selection() && drag.hit == TransformHit::Move).then_some(drag.start_pixel)
    }

    /// Cambia la caja de lo flotante si el resultado no pasa de `MAX_BLOCK_SIDE`; si pasa, el
    /// handle se queda en el último tamaño válido.
    fn set_floating_box(&mut self, bbox: TransformBox, fast: bool) -> bool {
        let (w, h) = cs_raster::rotated_size(
            bbox.width().unsigned_abs(),
            bbox.height().unsigned_abs(),
            bbox.angle,
        );
        if w > MAX_BLOCK_SIDE || h > MAX_BLOCK_SIDE {
            return false;
        }
        let Some(f) = self.floating.as_mut() else {
            return false;
        };
        f.bbox = bbox;
        self.refresh_floating(fast);
        true
    }
```

En `commit_floating` y en `cancel_floating`, justo después del `let Some(f) = self.floating.take() else { ... };`:

```rust
        self.transform_drag = None;
```

- [ ] **Step 4: Conectar el puntero en el editor**

En `crates/cs-ui/src/editor.rs`:

1. Imports: reemplazar `use crate::transform::Floating;` por

```rust
use crate::transform::{Floating, TransformDrag};
use crate::transform_box::TransformHit;
```

2. Struct `Editor`: borrar el campo `temp_move` y su comentario (las tres líneas
   `/// Arrastre dentro de la selección…`, `/// (como Move)…` y
   `temp_move: Option<(Point, bool)>,`). Después de `pub(crate) floating: Option<Floating>,`
   agregar:

```rust
    /// Arrastre en curso para mover, escalar o rotar lo seleccionado.
    pub(crate) transform_drag: Option<TransformDrag>,
```

   En `Editor::new`, borrar `temp_move: None,` y agregar después de `floating: None,`:

```rust
            transform_drag: None,
```

3. `active_tool`: queda así.

```rust
    /// Herramienta efectiva (considera el cuentagotas temporal).
    pub fn active_tool(&self) -> ToolKind {
        if self.temp_eyedropper {
            ToolKind::Eyedropper
        } else {
            self.tool
        }
    }
```

4. `is_painting`: queda así.

```rust
    pub fn is_painting(&self) -> bool {
        self.transform_drag.is_some()
            || self.artboard_tool.is_active()
            || self
                .tools
                .get(self.active_tool())
                .is_some_and(|t| t.is_active())
    }
```

5. `pointer_press`: reemplazar las líneas

```rust
        let shift = self.options.constrain;
        self.temp_move = self
            .moves_selection_at(world, shift, alt)
            .then(|| (self.to_local(world), false));
```

por

```rust
        let shift = self.options.constrain;
        // Arrastrar lo seleccionado (o cualquier punto con Move) lo levanta y lo deja flotando.
        if self.moves_selection_at(world, shift, alt) {
            self.press_transform(TransformHit::Move, world);
            return;
        }
        // Un clic fuera de lo flotante lo confirma antes de seguir con la herramienta.
        self.commit_floating();
```

6. `moves_selection_at`: reemplazar la función entera por

```rust
    /// Presionar en `world` arrastraría lo seleccionado (levantándolo): con Move, desde cualquier
    /// punto que no sea otro artboard; con una herramienta de selección, dentro de lo
    /// seleccionado y sin modificadores que cambien el modo de selección.
    pub fn moves_selection_at(&self, world: (f32, f32), shift: bool, alt: bool) -> bool {
        let artboard = self.document.artboard_at(world);
        if !self.can_transform() || artboard.is_some_and(|id| id != self.active) {
            return false;
        }
        if self.tool == ToolKind::Move {
            return true;
        }
        let p = self.to_local(world);
        !shift
            && !alt
            && !self.tools.get(self.tool).is_some_and(|t| t.is_pending())
            && artboard == Some(self.active)
            && self
                .selection
                .as_ref()
                .is_some_and(|m| m.contains(p.0, p.1))
    }
```

7. `pointer_drag`: queda así.

```rust
    pub fn pointer_drag(&mut self, world: (f32, f32)) {
        if self.artboard_tool.is_active() {
            self.artboard_tool.drag(world);
            return;
        }
        if self.transform_drag.is_some() {
            self.drag_transform(world);
            return;
        }
        let p = self.tool_point(world);
        self.with_tool(|tool, ctx| tool.drag(ctx, p));
    }
```

8. `pointer_release`: reemplazar todo lo que sigue al bloque `if self.artboard_tool.is_active() { ... return; }`
   por

```rust
        if self.transform_drag.is_some() {
            if let Some(from) = self.release_transform() {
                // Un clic sin arrastrar es un clic normal de la herramienta de selección.
                let kind = self.tool;
                self.gesture_mode = self.options.selection_mode;
                self.shift_at_press = false;
                self.with_tool(|tool, ctx| tool.press(ctx, from));
                let out = self.with_tool(|tool, ctx| tool.release(ctx));
                self.apply_output(kind, out);
            }
            return;
        }
        let kind = self.active_tool();
        if self.tiles_mode() && kind.blocked_in_tiles() {
            self.temp_eyedropper = false;
            return;
        }
        let out = self.with_tool(|tool, ctx| tool.release(ctx));
        self.temp_eyedropper = false;
        self.apply_output(kind, out);
    }
```

- [ ] **Step 5: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS, incluidos `moving_the_selection_twice_only_carries_what_was_selected_first` y
`move_tool_picks_the_layer_under_the_pointer` (sin selección sigue usando `MoveTool`).

- [ ] **Step 6: Commit**

```bash
git add crates/cs-ui/src
git commit -m "Mover la selección arrastrando la deja flotando: ya no estampa en cada arrastre

Arreglo del bug reportado: el segundo arrastre levantaba también el dibujo
que había quedado dentro de la selección.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Esc por partes, Enter confirma y confirmar al copiar, guardar o exportar

**Files:**
- Modify: `crates/cs-ui/src/transform.rs`
- Modify: `crates/cs-ui/src/lib.rs`
- Modify: `crates/cs-ui/src/file_actions.rs`

**Interfaces:**
- Consumes: `Editor::{cancel_floating, commit_floating, is_floating, sync_floating}` (Task 3).
- Produces:
  - `Editor::escape(&mut self)`.
  - `crate::editor_key(editor: &mut Editor, key: egui::Key, shift: bool)` (`pub(crate)`, en
    `lib.rs`).

- [ ] **Step 1: Escribir los tests que fallan**

Al módulo `tests` de `crates/cs-ui/src/transform.rs`:

```rust
    #[test]
    fn escape_cancels_the_gesture_then_the_floating_content_then_the_selection() {
        let mut e = editor();
        // Un lazo poligonal a medias: Esc lo descarta y la selección sigue.
        e.set_tool(ToolKind::PolygonLasso);
        e.pointer_press((6.5, 6.5), false, false, 0.1);
        e.pointer_release();
        assert!(e.is_pending());
        e.escape();
        assert!(!e.is_pending());
        assert!(e.selection.is_some());
        // Flotando: Esc cancela la transformación sin deseleccionar.
        e.set_tool(ToolKind::RectSelect);
        assert!(e.lift());
        moved(&mut e, 2, 2);
        e.escape();
        assert!(!e.is_floating());
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((0, 0), (2, 2))));
        assert_eq!(px(&e, 1, 1), red());
        // Sin nada más: Esc deselecciona, como Deselect.
        e.escape();
        assert!(e.selection.is_none());
        assert!(e.can_reselect());
    }
```

En `crates/cs-ui/src/lib.rs`, reemplazar `mod tests` entero por:

```rust
#[cfg(test)]
mod tests {
    use cs_core::Mask;

    use super::*;

    #[test]
    fn convert_failure_message_has_no_stray_spaces() {
        assert!(!super::CONVERT_TO_IMAGE_FAILED.contains("  "));
    }

    #[test]
    fn enter_applies_floating_content_and_escape_cancels_it() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.set_tool(ToolKind::RectSelect);
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (1, 1))));
        let drag = |e: &mut Editor| {
            e.pointer_press((0.5, 0.5), false, false, 0.1);
            e.pointer_drag((2.5, 0.5));
            e.pointer_release();
        };
        drag(&mut e);
        assert!(e.is_floating());
        editor_key(&mut e, Key::Escape, false);
        assert!(!e.is_floating() && e.selection.is_some(), "Esc cancels, keeps the selection");
        drag(&mut e);
        editor_key(&mut e, Key::Enter, false);
        assert!(!e.is_floating(), "Enter applies");
        assert!(!e.playing, "instead of playing the animation");
        assert!(e.history.can_undo());
        editor_key(&mut e, Key::Escape, false);
        assert!(e.selection.is_none(), "with nothing else, Esc deselects");
    }
}
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui escape`
Expected: FAIL de compilación con "no method named `escape`" y "cannot find function
`editor_key`".

- [ ] **Step 3: Implementar**

En `crates/cs-ui/src/transform.rs`, dentro de `impl Editor`:

```rust
    /// Esc hace una sola cosa por pulsación. Primero descarta el gesto o el texto a medio hacer,
    /// después cancela lo flotante (sin deseleccionar) y, si no hay nada de eso, deselecciona.
    pub fn escape(&mut self) {
        if self.is_pending() {
            self.cancel_gesture();
        } else if !self.cancel_floating() {
            self.deselect();
        }
    }
```

En `crates/cs-ui/src/lib.rs`:

1. En `handle_shortcuts`, reemplazar el `for (key, shift) in pressed { ... }` entero por:

```rust
        for (key, shift) in pressed {
            let editor = &mut self.doc.editor;
            let letter = key.name().chars().next().filter(|_| key.name().len() == 1);
            if let Some(tool) = letter.and_then(|c| ToolKind::from_key(c, shift)) {
                editor.set_tool(tool);
                continue;
            }
            editor_key(editor, key, shift);
        }
```

2. Antes de `#[cfg(test)] mod tests`, agregar la función libre:

```rust
/// Teclas sin Ctrl/Alt que actúan sobre el editor (las letras de herramienta van aparte).
pub(crate) fn editor_key(editor: &mut Editor, key: Key, shift: bool) {
    match key {
        Key::N if shift => editor.add_layer(cs_core::LayerKind::Image),
        Key::Enter if editor.is_pending() => editor.finish_gesture(),
        Key::Enter if editor.is_floating() => {
            editor.commit_floating();
        }
        Key::Escape => editor.escape(),
        Key::Enter => editor.toggle_play(),
        Key::Comma | Key::ArrowLeft => editor.prev_frame(),
        Key::Period | Key::ArrowRight => editor.next_frame(),
        Key::ArrowUp => editor.step_layer(true),
        Key::ArrowDown => editor.step_layer(false),
        Key::F3 => editor.onion_enabled = !editor.onion_enabled,
        // En modo Tiles no hay color secundario: X e Y voltean el tile del pincel.
        Key::X if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_X),
        Key::Y if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_Y),
        Key::X => editor.swap_colors(),
        Key::Delete | Key::Backspace => editor.clear_selection_content(),
        _ => {}
    }
}
```

3. En `ChimpApp::copy`, como primera línea (`Editor::copy` es `&self` y lee el documento):

```rust
        self.doc.editor.commit_floating();
```

4. En `fn update` (el de `eframe::App`), justo antes de `self.handle_shortcuts(&ctx);`:

```rust
        if self.has_doc() {
            // Lo flotante se confirma o cancela si la capa, el frame o la herramienta cambiaron.
            self.doc.editor.sync_floating();
        }
```

En `crates/cs-ui/src/file_actions.rs`:

- `fn save_to(&mut self, target: SaveTarget) -> bool`: primera línea del cuerpo.

```rust
        // Lo flotante todavía no está en el documento: se confirma antes de escribir.
        self.doc.editor.commit_floating();
```

- `pub(crate) fn open_export(&mut self)`: primera línea del cuerpo.

```rust
        self.doc.editor.commit_floating();
```

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src
git commit -m "Esc cancela el gesto, después lo flotante y después la selección; Enter confirma

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Pegar deja lo pegado flotando

**Files:**
- Modify: `crates/cs-ui/src/transform.rs`
- Modify: `crates/cs-ui/src/clipboard.rs`

**Interfaces:**
- Consumes: `Floating::new`, `Editor::{commit_floating, refresh_floating, escape}` (Tasks 3–5),
  `cs_raster::Block`.
- Produces: `Editor::float_pasted(&mut self, block: Block<PixelValue>, at: (i32, i32))`
  (`pub(crate)`). `Editor::paste` mantiene su firma.

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/cs-ui/src/clipboard.rs`, reemplazar el módulo `tests` entero por:

```rust
#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{LayerId, Mask, Sprite};

    use super::*;
    use crate::editor::ToolKind;

    fn editor_with_dot() -> (Editor, PixelValue) {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = e.palette_value(5);
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(2, 2, red)],
        });
        (e, red)
    }

    #[test]
    fn copy_needs_a_selection_and_paste_floats_until_applied() {
        let (mut e, red) = editor_with_dot();
        assert_eq!(e.copy(), None);
        e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (3, 3))));
        let clip = e.copy().unwrap();
        e.add_layer(LayerKind::Image);
        let top = e.layer;
        let t = e.sprite().color_mode.transparent();
        e.paste(&clip);
        assert!(e.is_floating());
        assert_eq!(e.sprite().pixel(top, 0, 2, 2), t, "not applied yet");
        let shown = e.view().document.artboards[0].sprite.pixel(top, 0, 2, 2);
        assert_eq!(shown, red, "same place, current layer");
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((1, 1), (3, 3)))
        );
        assert!(e.commit_floating());
        assert_eq!(e.sprite().pixel(top, 0, 2, 2), red);
        e.undo();
        assert_eq!(e.sprite().pixel(top, 0, 2, 2), t);
    }

    #[test]
    fn cut_clears_and_paste_fits_inside_a_smaller_canvas() {
        let (mut e, red) = editor_with_dot();
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (7, 7))));
        let clip = e.cut().unwrap();
        let t = e.sprite().color_mode.transparent();
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), t);

        let mut small = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        small.paste(&clip);
        assert!(small.commit_floating());
        assert_eq!(
            small.sprite().pixel(LayerId(1), 0, 0, 0),
            red,
            "moved to fit"
        );
        assert_eq!(
            small.selection.as_ref().unwrap().bounds(),
            Some(((0, 0), (3, 3)))
        );
    }

    #[test]
    fn paste_into_an_indexed_sprite_uses_the_nearest_palette_entry() {
        let (mut e, red) = editor_with_dot();
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        let mut indexed = Editor::with_sprite(Sprite::new(
            8,
            8,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        indexed.paste(&clip);
        assert!(indexed.commit_floating());
        let v = indexed.sprite().pixel(LayerId(1), 0, 2, 2);
        assert_eq!(indexed.resolve(v), e.resolve(red));
    }

    #[test]
    fn locked_layers_are_not_cut_or_pasted() {
        let (mut e, red) = editor_with_dot();
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        e.toggle_locked(LayerId(1));
        assert_eq!(e.cut(), None);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), red);
        let version = e.doc_version;
        e.paste(&clip);
        assert_eq!(e.doc_version, version);
        assert!(!e.is_floating());
    }

    #[test]
    fn pasted_content_floats_over_the_drawing_below_without_mixing() {
        let (mut e, red) = editor_with_dot();
        let blue = e.palette_value(9);
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(6, 2, blue)],
        });
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        e.paste(&clip); // flota en (2, 2)
        e.tool = ToolKind::RectSelect;
        // Pasa por encima del azul y se deja más abajo.
        e.pointer_press((2.5, 2.5), false, false, 0.1);
        e.pointer_drag((6.5, 2.5));
        e.pointer_drag((6.5, 5.5));
        e.pointer_release();
        assert!(e.commit_floating());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 6, 5), red);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 6, 2), blue, "the drawing below is intact");
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), red, "pasting leaves no hole");
    }

    #[test]
    fn cancelling_a_paste_restores_the_previous_selection() {
        let (mut e, _) = editor_with_dot();
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (2, 2))));
        let clip = e.copy().unwrap();
        e.set_selection(Some(Mask::from_rect(8, 8, (5, 5), (6, 6))));
        let version = e.doc_version;
        e.paste(&clip);
        e.escape();
        assert!(!e.is_floating());
        assert_eq!(e.doc_version, version);
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((5, 5), (6, 6)))
        );
    }
}
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui clipboard::`
Expected: FAIL. `copy_needs_a_selection_and_paste_floats_until_applied` falla en
`assert!(e.is_floating())`, porque hoy pegar escribe en la capa.

- [ ] **Step 3: Implementar**

En `crates/cs-ui/src/transform.rs`, dentro de `impl Editor`:

```rust
    /// Deja `block` flotando con su esquina en `at` (pegar). No deja hueco; si se cancela, vuelve
    /// la selección de antes del pegado.
    pub(crate) fn float_pasted(&mut self, block: Block<PixelValue>, at: (i32, i32)) {
        self.commit_floating();
        let s = self.sprite();
        let canvas = (s.width, s.height);
        let target = (self.active, self.layer, self.frame);
        let before = self.selection.clone();
        self.floating = Some(Floating::new(block, at, target, true, before, canvas));
        self.refresh_floating(false);
    }
```

En `crates/cs-ui/src/clipboard.rs`:

1. Imports de arriba (`Command` y `Mask` pasan a usarse solo en los tests):

```rust
use cs_core::{ColorMode, LayerKind, PixelValue, Rgba};
use cs_raster::Block;

use crate::editor::Editor;
```

2. Reemplazar `paste` entera por:

```rust
    /// Pega en la capa actual donde estaba lo copiado (corrido para que entre en el lienzo). Lo
    /// pegado queda flotando y seleccionado, listo para moverlo, escalarlo o rotarlo; al
    /// confirmar, lo transparente no borra lo de debajo.
    pub fn paste(&mut self, clip: &Clip) {
        if !self.clip_layer_ok(true) {
            return;
        }
        self.cancel_gesture();
        self.commit_floating();
        let sprite = self.sprite();
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        let fit = |o: i32, size: u32, max: i32| o.min(max - size as i32).max(0);
        let (ox, oy) = (
            fit(clip.origin.0, clip.size.0, w),
            fit(clip.origin.1, clip.size.1, h),
        );
        let clear = sprite.color_mode.transparent();
        let value = |c: Rgba| match sprite.color_mode {
            _ if c.a == 0 => clear,
            ColorMode::Rgba => PixelValue::Rgba(c),
            ColorMode::Indexed { transparent_index } => {
                PixelValue::Index(cs_io::import::nearest_index(sprite, c, transparent_index))
            }
        };
        // Los puntos transparentes también viajan: dan la forma de la selección.
        let mut block = Block::new(clip.size.0, clip.size.1);
        for &(x, y, c) in &clip.pixels {
            block.set(x, y, Some(value(c)));
        }
        self.float_pasted(block, (ox, oy));
    }
```

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src
git commit -m "Pegar deja lo pegado flotando sobre lo de debajo hasta confirmar

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Handles, escalar y rotar (geometría pura y arrastres del editor)

**Files:**
- Modify: `crates/cs-ui/src/transform_box.rs`
- Modify: `crates/cs-ui/src/transform.rs`

**Interfaces:**
- Consumes:
  - `TransformBox`, `TransformHit::Move` (Tasks 3 y 4).
  - `TransformDrag`, `Editor::{press_transform, drag_transform, set_floating_box}` (Task 4).
- Produces:
  - `crate::transform_box::Handle { NW, N, NE, E, SE, S, SW, W }`, con `ALL: [Handle; 8]` y
    `dirs(self) -> (i32, i32)`.
  - `TransformHit::{Move, Handle(Handle), Rotate}`.
  - `TransformBox::center(&self) -> (f32, f32)` y
    `TransformBox::to_frame(&self, p: (f32, f32)) -> (f32, f32)`.
  - `TransformBox::handle_point(&self, h: Handle) -> (f32, f32)` y
    `TransformBox::corners(&self) -> [(f32, f32); 4]`.
  - `transform_box::rotate_point(p: (f32, f32), c: (f32, f32), angle: f32) -> (f32, f32)`.
  - `transform_box::hit(b: &TransformBox, p: (f32, f32), handle: f32, band: f32) -> Option<TransformHit>`.
    Nunca devuelve `Move`.
  - `transform_box::resize(start: &TransformBox, handle: Handle, from: (f32, f32), to: (f32, f32), keep_ratio: bool) -> TransformBox`.
  - `transform_box::rotate(start: &TransformBox, from: (f32, f32), to: (f32, f32), snap: bool) -> TransformBox`.
  - `Editor::shows_handles(&self) -> bool`.
  - `Editor::transform_drag_hit(&self) -> Option<TransformHit>`.
  - Todas las coordenadas son locales del artboard activo, con decimales.

- [ ] **Step 1: Escribir los tests que fallan**

Al módulo `tests` de `crates/cs-ui/src/transform_box.rs` (agregar arriba
`use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};`):

```rust
    #[test]
    fn handles_rotation_band_and_misses() {
        let b = TransformBox::new(2, 2, 4, 4);
        assert_eq!(hit(&b, (6.0, 6.0), 0.25, 0.5), Some(TransformHit::Handle(Handle::SE)));
        assert_eq!(hit(&b, (4.1, 1.9), 0.25, 0.5), Some(TransformHit::Handle(Handle::N)));
        assert_eq!(hit(&b, (4.0, 4.0), 0.25, 0.5), None, "inside: the editor moves it");
        assert_eq!(hit(&b, (6.4, 1.6), 0.25, 0.5), Some(TransformHit::Rotate));
        assert_eq!(hit(&b, (7.0, 4.0), 0.25, 0.5), None, "beside a side, far from the corners");
        // Girada 90°: el handle E queda abajo del centro.
        let r = TransformBox {
            angle: FRAC_PI_2,
            ..TransformBox::new(0, 0, 4, 2)
        };
        assert_eq!(hit(&r, (2.0, 3.0), 0.25, 0.5), Some(TransformHit::Handle(Handle::E)));
    }

    #[test]
    fn dragging_a_side_moves_only_that_side_and_flips_past_the_other() {
        let b = TransformBox::new(0, 0, 4, 2);
        let e = resize(&b, Handle::E, (4.0, 1.0), (7.0, 5.0), false);
        assert_eq!(e, TransformBox::new(0, 0, 7, 2), "E ignores the vertical motion");
        let w = resize(&b, Handle::W, (0.0, 1.0), (6.0, 1.0), false);
        assert_eq!((w.x0, w.x1, w.width()), (6, 4, -2), "past the other side: flipped");
        let thin = resize(&b, Handle::E, (4.0, 1.0), (0.0, 1.0), false);
        assert_eq!(thin.width(), 1, "never 0 px");
        // Volteada, el lado derecho en pantalla es x0.
        let f = resize(&w, Handle::E, (6.0, 1.0), (8.0, 1.0), false);
        assert_eq!((f.x0, f.x1), (8, 4));
    }

    #[test]
    fn shift_on_a_corner_keeps_the_proportion() {
        let b = TransformBox::new(0, 0, 4, 2);
        let free = resize(&b, Handle::SE, (4.0, 2.0), (8.0, 3.0), false);
        assert_eq!(free, TransformBox::new(0, 0, 8, 3));
        let kept = resize(&b, Handle::SE, (4.0, 2.0), (8.0, 3.0), true);
        assert_eq!(kept, TransformBox::new(0, 0, 8, 4));
        let nw = resize(&b, Handle::NW, (0.0, 0.0), (-4.0, 0.0), true);
        assert_eq!(nw, TransformBox::new(-4, -2, 8, 4));
    }

    #[test]
    fn a_rotated_box_keeps_the_opposite_side_in_place() {
        let b = TransformBox {
            angle: FRAC_PI_2,
            ..TransformBox::new(0, 0, 4, 2)
        };
        let anchor = |bx: &TransformBox| {
            let p = (bx.x0 as f32, (bx.y0 + bx.y1) as f32 / 2.0);
            rotate_point(p, bx.center(), bx.angle)
        };
        // Girada 90°, el lado E está abajo: bajar el puntero 2 la ensancha 2.
        let r = resize(&b, Handle::E, (2.0, 3.0), (2.0, 5.0), false);
        assert_eq!(r.width(), 6);
        let (a, c) = (anchor(&b), anchor(&r));
        assert!((a.0 - c.0).abs() < 1e-4 && (a.1 - c.1).abs() < 1e-4, "{a:?} {c:?}");
    }

    #[test]
    fn rotating_follows_the_pointer_around_the_center_and_shift_snaps() {
        let b = TransformBox::new(0, 0, 4, 4);
        let quarter = rotate(&b, (5.0, 2.0), (2.0, 5.0), false);
        assert!((quarter.angle - FRAC_PI_2).abs() < 1e-5);
        assert_eq!(
            (quarter.x0, quarter.y0, quarter.x1, quarter.y1),
            (0, 0, 4, 4),
            "only the angle changes"
        );
        let back = rotate(&b, (5.0, 2.0), (2.0, -1.0), false);
        assert!((back.angle - 3.0 * FRAC_PI_2).abs() < 1e-5, "counterclockwise wraps to 270°");
        let a = 50f32.to_radians();
        let to = (2.0 + 3.0 * a.cos(), 2.0 + 3.0 * a.sin());
        let snapped = rotate(&b, (5.0, 2.0), to, true);
        assert!((snapped.angle - FRAC_PI_4).abs() < 1e-5, "{}", snapped.angle);
    }
```

Al módulo `tests` de `crates/cs-ui/src/transform.rs` (agregar arriba
`use std::f32::consts::{FRAC_PI_2, PI};` y `use crate::transform_box::{Handle, TransformHit};`):

```rust
    #[test]
    fn dragging_a_corner_handle_scales_and_shift_keeps_the_proportion() {
        let mut e = editor();
        e.press_transform(TransformHit::Handle(Handle::SE), (3.0, 3.0));
        e.pointer_drag((6.0, 4.0));
        e.pointer_release();
        assert_eq!(e.transform_box(), Some(TransformBox::new(0, 0, 6, 4)));
        assert!(e.cancel_floating());
        e.options.constrain = true;
        e.press_transform(TransformHit::Handle(Handle::SE), (3.0, 3.0));
        e.pointer_drag((6.0, 4.0));
        e.pointer_release();
        assert_eq!(e.transform_box(), Some(TransformBox::new(0, 0, 6, 6)));
    }

    #[test]
    fn dragging_around_the_center_rotates_and_shift_snaps_to_15_degrees() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        // 4×4 con el rojo en su esquina superior izquierda; centro (3, 3).
        e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (4, 4))));
        e.press_transform(TransformHit::Rotate, (6.0, 3.0));
        e.pointer_drag((3.0, 6.0)); // de la derecha a abajo: +90°
        assert_eq!(
            e.floating().unwrap().rendered_with,
            RotationAlgorithm::Fast,
            "Fast while dragging"
        );
        e.pointer_release();
        assert_eq!(
            e.floating().unwrap().rendered_with,
            RotationAlgorithm::RotSprite,
            "the chosen rotation on release"
        );
        let b = e.transform_box().unwrap();
        assert!((b.angle - FRAC_PI_2).abs() < 1e-4, "{}", b.angle);
        assert!(e.commit_floating());
        assert_eq!(px(&e, 4, 1), red(), "the top-left corner went to the top-right");
        assert_eq!(px(&e, 1, 1), t);
        e.options.constrain = true;
        e.press_transform(TransformHit::Rotate, (6.0, 3.0));
        e.pointer_drag((3.0 + 3.0 * 0.9, 3.0 + 3.0 * 0.44)); // unos 26°
        e.pointer_release();
        let b = e.transform_box().unwrap();
        assert!((b.angle - PI / 6.0).abs() < 1e-4, "{}", b.angle);
    }

    #[test]
    fn a_handle_stops_at_the_size_limit() {
        let mut e = editor();
        e.press_transform(TransformHit::Handle(Handle::E), (3.0, 1.5));
        e.pointer_drag((5000.0, 1.5));
        assert_eq!(
            e.transform_box(),
            Some(TransformBox::new(0, 0, 3, 3)),
            "too wide: keeps the last valid size"
        );
        e.pointer_drag((100.0, 1.5));
        assert_eq!(e.transform_box().unwrap().width(), 100);
        e.pointer_release();
    }

    #[test]
    fn handles_show_only_where_the_clipboard_works() {
        let mut e = editor();
        assert!(e.shows_handles());
        e.set_tool(ToolKind::Pencil);
        assert!(!e.shows_handles(), "only selection tools and Move");
        e.set_tool(ToolKind::Move);
        assert!(e.shows_handles());
        e.toggle_locked(LayerId(1));
        assert!(!e.shows_handles(), "not on a locked layer");
    }
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui transform`
Expected: FAIL de compilación con "no variant `Handle`" en `TransformHit`, "cannot find function
`resize`" y "no method named `shows_handles`".

- [ ] **Step 3: Implementar la geometría**

En `crates/cs-ui/src/transform_box.rs`:

1. Reemplazar `pub enum TransformHit { Move }` (con su comentario) por:

```rust
/// Qué se arrastra sobre lo seleccionado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformHit {
    /// Dentro de la selección (o cualquier punto con Move): mover.
    Move,
    /// Un handle: escalar moviendo ese lado o esa esquina.
    Handle(Handle),
    /// Justo por fuera de una esquina: rotar sobre el centro.
    Rotate,
}

/// Uno de los 8 handles de la caja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::NW,
        Handle::N,
        Handle::NE,
        Handle::E,
        Handle::SE,
        Handle::S,
        Handle::SW,
        Handle::W,
    ];

    /// Lado horizontal y vertical: -1 izquierda/arriba, 0 centro, 1 derecha/abajo.
    pub fn dirs(self) -> (i32, i32) {
        match self {
            Handle::NW => (-1, -1),
            Handle::N => (0, -1),
            Handle::NE => (1, -1),
            Handle::E => (1, 0),
            Handle::SE => (1, 1),
            Handle::S => (0, 1),
            Handle::SW => (-1, 1),
            Handle::W => (-1, 0),
        }
    }
}

/// Gira `p` alrededor de `c` (positivo = horario, con y hacia abajo).
pub fn rotate_point(p: (f32, f32), c: (f32, f32), angle: f32) -> (f32, f32) {
    let (s, co) = angle.sin_cos();
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    (c.0 + co * dx - s * dy, c.1 + s * dx + co * dy)
}
```

2. Dentro de `impl TransformBox`, después de `translated`:

```rust
    pub fn center(&self) -> (f32, f32) {
        (
            (self.x0 + self.x1) as f32 / 2.0,
            (self.y0 + self.y1) as f32 / 2.0,
        )
    }

    /// Punto del lienzo → marco de la caja sin rotar.
    pub fn to_frame(&self, p: (f32, f32)) -> (f32, f32) {
        rotate_point(p, self.center(), -self.angle)
    }

    /// Punto de un lado o esquina en pantalla (`dirs` como `Handle::dirs`), sin rotar.
    fn unrotated(&self, dirs: (i32, i32)) -> (f32, f32) {
        let side = |d: i32, a: i32, b: i32| match d {
            -1 => a.min(b) as f32,
            0 => (a + b) as f32 / 2.0,
            _ => a.max(b) as f32,
        };
        (side(dirs.0, self.x0, self.x1), side(dirs.1, self.y0, self.y1))
    }

    /// Posición del handle en el lienzo (rotada).
    pub fn handle_point(&self, handle: Handle) -> (f32, f32) {
        rotate_point(self.unrotated(handle.dirs()), self.center(), self.angle)
    }

    /// Esquinas rotadas en orden NW, NE, SE, SW.
    pub fn corners(&self) -> [(f32, f32); 4] {
        [Handle::NW, Handle::NE, Handle::SE, Handle::SW].map(|h| self.handle_point(h))
    }
```

3. Funciones libres, antes de `#[cfg(test)]`:

```rust
/// Handle o franja de rotación bajo `p`. `handle` es medio lado del cuadrado de cada handle y
/// `band` el ancho de la franja de rotación fuera de las esquinas, los dos en píxeles locales.
/// Dentro de la caja no devuelve nada: mover lo decide el editor con la máscara.
pub fn hit(b: &TransformBox, p: (f32, f32), handle: f32, band: f32) -> Option<TransformHit> {
    let q = b.to_frame(p);
    let near = |h: (f32, f32), r: f32| (q.0 - h.0).abs() <= r && (q.1 - h.1).abs() <= r;
    if let Some(h) = Handle::ALL
        .into_iter()
        .find(|h| near(b.unrotated(h.dirs()), handle))
    {
        return Some(TransformHit::Handle(h));
    }
    let (l, t) = b.unrotated((-1, -1));
    let (r, bottom) = b.unrotated((1, 1));
    let outside = q.0 < l || q.0 > r || q.1 < t || q.1 > bottom;
    let corner = [Handle::NW, Handle::NE, Handle::SE, Handle::SW]
        .into_iter()
        .any(|h| near(b.unrotated(h.dirs()), band));
    (outside && corner).then_some(TransformHit::Rotate)
}

/// Mueve el borde `b` (o `a`) `d` píxeles sin dejar el tamaño en 0; `start_size` da el signo.
fn move_edge(a: &mut i32, b: &mut i32, moves_b: bool, d: i32, start_size: i32) {
    let sign = start_size.signum();
    if moves_b {
        *b += d;
        if *b == *a {
            *b = *a + sign;
        }
    } else {
        *a += d;
        if *a == *b {
            *a = *b - sign;
        }
    }
}

/// Deja el tamaño en `size` (positivo) moviendo `b` (o `a`), con el signo que ya tiene.
fn set_size(a: &mut i32, b: &mut i32, moves_b: bool, size: i32) {
    let sign = if *b >= *a { 1 } else { -1 };
    if moves_b {
        *b = *a + sign * size;
    } else {
        *a = *b - sign * size;
    }
}

/// Arrastrar `handle` de `from` a `to`: se mueve ese lado o esa esquina y el lado opuesto queda
/// quieto (también con la caja girada). Más allá del lado opuesto se voltea; el mínimo es 1 px.
/// `keep_ratio` (Shift) mantiene la proporción en las esquinas.
pub fn resize(
    start: &TransformBox,
    handle: Handle,
    from: (f32, f32),
    to: (f32, f32),
    keep_ratio: bool,
) -> TransformBox {
    let (hx, hy) = handle.dirs();
    let (a, b) = (start.to_frame(from), start.to_frame(to));
    let (dx, dy) = ((b.0 - a.0).round() as i32, (b.1 - a.1).round() as i32);
    let mut out = *start;
    // Qué campo es el lado que se arrastra: con volteo, x0 es el derecho en pantalla.
    let moves_x1 = (hx == 1) == (start.x1 >= start.x0);
    let moves_y1 = (hy == 1) == (start.y1 >= start.y0);
    if hx != 0 {
        move_edge(&mut out.x0, &mut out.x1, moves_x1, dx, start.width());
    }
    if hy != 0 {
        move_edge(&mut out.y0, &mut out.y1, moves_y1, dy, start.height());
    }
    if keep_ratio && hx != 0 && hy != 0 {
        let w0 = start.width().unsigned_abs() as f32;
        let h0 = start.height().unsigned_abs() as f32;
        let s = (out.width().unsigned_abs() as f32 / w0).max(out.height().unsigned_abs() as f32 / h0);
        let size = |v: f32| (v * s).round().max(1.0) as i32;
        set_size(&mut out.x0, &mut out.x1, moves_x1, size(w0));
        set_size(&mut out.y0, &mut out.y1, moves_y1, size(h0));
    }
    // Girada, el centro se corre al cambiar el tamaño: se compensa para que el lado fijo no se
    // mueva en el lienzo.
    let anchor = |bx: &TransformBox| {
        let (cx, cy) = bx.center();
        let ax = match hx {
            0 => cx,
            _ if moves_x1 => bx.x0 as f32,
            _ => bx.x1 as f32,
        };
        let ay = match hy {
            0 => cy,
            _ if moves_y1 => bx.y0 as f32,
            _ => bx.y1 as f32,
        };
        rotate_point((ax, ay), (cx, cy), bx.angle)
    };
    let (before, after) = (anchor(start), anchor(&out));
    out.translated(
        (before.0 - after.0).round() as i32,
        (before.1 - after.1).round() as i32,
    )
}

/// Arrastrar fuera de una esquina: el ángulo sigue al puntero alrededor del centro. `snap`
/// (Shift) ajusta a pasos de 15°.
pub fn rotate(start: &TransformBox, from: (f32, f32), to: (f32, f32), snap: bool) -> TransformBox {
    use std::f32::consts::{PI, TAU};
    let c = start.center();
    let turn = (to.1 - c.1).atan2(to.0 - c.0) - (from.1 - c.1).atan2(from.0 - c.0);
    let mut angle = (start.angle + turn).rem_euclid(TAU);
    if snap {
        let step = PI / 12.0;
        angle = ((angle / step).round() * step).rem_euclid(TAU);
    }
    TransformBox { angle, ..*start }
}
```

- [ ] **Step 4: Conectar los arrastres en el editor**

En `crates/cs-ui/src/transform.rs`:

1. Import de la caja:

```rust
use crate::transform_box::{self, TransformBox, TransformHit};
```

2. `TransformDrag`: después de `hit: TransformHit,` agregar

```rust
    /// Punto local (con decimales) donde se presionó.
    start: (f32, f32),
```

3. En `press_transform`, al construir `TransformDrag`, agregar `start: self.local_point(world),`
   después de `hit,`.

4. Reemplazar `drag_transform` entera por:

```rust
    pub(crate) fn drag_transform(&mut self, world: (f32, f32)) {
        let Some(mut drag) = self.transform_drag else {
            return;
        };
        let p = self.local_point(world);
        let pixel = self.to_local(world);
        let shift = self.options.constrain;
        let bbox = match drag.hit {
            TransformHit::Move => drag.start_box.translated(
                pixel.0 - drag.start_pixel.0,
                pixel.1 - drag.start_pixel.1,
            ),
            TransformHit::Handle(handle) => {
                transform_box::resize(&drag.start_box, handle, drag.start, p, shift)
            }
            TransformHit::Rotate => transform_box::rotate(&drag.start_box, drag.start, p, shift),
        };
        if Some(bbox) == self.transform_box() {
            return;
        }
        if !drag.moved {
            // Se levanta una sola vez, al primer cambio.
            if !self.lift() {
                self.transform_drag = None;
                return;
            }
            drag.moved = true;
            self.transform_drag = Some(drag);
        }
        // Mientras se rota se usa Fast; RotSprite se calcula al soltar.
        self.set_floating_box(bbox, drag.hit == TransformHit::Rotate);
    }
```

5. Dentro de `impl Editor`, agregar:

```rust
    /// Punto de mundo → coordenadas locales del artboard activo, con decimales.
    fn local_point(&self, world: (f32, f32)) -> (f32, f32) {
        let (x, y) = self.artboard().position;
        (world.0 - x as f32, world.1 - y as f32)
    }

    /// El lienzo muestra los 8 handles (y la franja de rotación).
    pub fn shows_handles(&self) -> bool {
        self.can_transform() && !self.is_pending()
    }

    /// Qué se está arrastrando (para el cursor).
    pub fn transform_drag_hit(&self) -> Option<TransformHit> {
        self.transform_drag.map(|d| d.hit)
    }
```

- [ ] **Step 5: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cs-ui/src
git commit -m "Handles: escalar por lados y esquinas, rotar sobre el centro (Shift: proporción y 15°)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Lienzo: dibujar handles, hit-test, cursores y cursor de rotación

**Files:**
- Modify: `crates/cs-ui/src/canvas.rs`

**Interfaces:**
- Consumes:
  - `transform_box::{hit, Handle, TransformBox, TransformHit}` (Task 7).
  - `Editor::{shows_handles, transform_box, press_transform, transform_drag_hit}` (Tasks 3, 4 y 7).
  - `crate::editor_key` (Task 5).
- Produces:
  - `CanvasView::transform_hit(&self, rect: Rect, editor: &Editor, p: Pos2) -> Option<TransformHit>`
    (privada).
  - Constantes `HANDLE_PX = 8.0` y `ROTATE_PX = 12.0`.

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/cs-ui/src/canvas.rs`, dentro de `mod tests`:

1. Imports del módulo de tests:

```rust
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, PixelValue, Rgba, Sprite};
    use egui::{Event, Key, PointerButton, RawInput};

    use super::*;
```

2. Dentro de `impl Harness`, después de `fn screen`:

```rust
        /// Esquina en pantalla del píxel local `p` del artboard activo (no su centro).
        fn corner(&self, p: (i32, i32)) -> Pos2 {
            let (x, y) = self.editor.artboard().position;
            self.canvas.to_screen(self.rect, (x + p.0, y + p.1))
        }

        /// Arrastre con el botón principal de `from` a `to` (en pantalla), en varios frames.
        fn drag(&mut self, from: Pos2, to: Pos2) {
            let button = |pos, pressed| Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![Event::PointerMoved(from)]);
            self.frame(vec![button(from, true)]);
            self.frame(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
            self.frame(vec![Event::PointerMoved(to)]);
            self.frame(vec![button(to, false)]);
        }

        /// Un punto rojo en `at` y la selección `a`–`b` con el marco rectangular.
        fn selected_dot(&mut self, at: (i32, i32), a: (i32, i32), b: (i32, i32)) -> PixelValue {
            let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
            self.editor.execute(Command::DrawPixels {
                layer: LayerId(1),
                frame: 0,
                pixels: vec![(at.0, at.1, red)],
            });
            self.editor.set_tool(ToolKind::RectSelect);
            self.editor.set_selection(Some(Mask::from_rect(16, 16, a, b)));
            self.frame(vec![]);
            red
        }
```

3. Tests:

```rust
    #[test]
    fn dragging_a_corner_handle_scales_and_enter_applies() {
        let mut h = Harness::new();
        let red = h.selected_dot((2, 2), (2, 2), (3, 3));
        let t = h.editor.sprite().color_mode.transparent();
        h.drag(h.corner((4, 4)), h.corner((6, 6)));
        assert_eq!(h.editor.transform_box(), Some(TransformBox::new(2, 2, 4, 4)));
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 3, 3), t, "still floating");
        crate::editor_key(&mut h.editor, Key::Enter, false);
        assert!(!h.editor.is_floating());
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, x, y), red, "({x}, {y})");
        }
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 4, 4), t);
    }

    #[test]
    fn dragging_just_outside_a_corner_rotates() {
        let mut h = Harness::new();
        let red = h.selected_dot((2, 2), (2, 2), (5, 5));
        let t = h.editor.sprite().color_mode.transparent();
        let center = h.corner((4, 4));
        // 8 px por fuera de la esquina NE: fuera del handle, dentro de la franja de 12 px.
        let start = h.corner((6, 2)) + Vec2::new(8.0, -8.0);
        let v = start - center;
        h.drag(start, center + Vec2::new(-v.y, v.x)); // un cuarto de vuelta horario
        let angle = h.editor.transform_box().unwrap().angle;
        assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-3, "{angle}");
        crate::editor_key(&mut h.editor, Key::Enter, false);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 2), red);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 2, 2), t);
    }

    #[test]
    fn dragging_inside_the_selection_still_moves_it() {
        let mut h = Harness::new();
        let red = h.selected_dot((2, 2), (2, 2), (3, 3));
        h.drag(h.screen((2, 2)), h.screen((5, 2)));
        assert_eq!(h.editor.transform_box(), Some(TransformBox::new(5, 2, 2, 2)));
        crate::editor_key(&mut h.editor, Key::Enter, false);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 2), red);
    }
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui canvas::`
Expected: FAIL. Falta importar `TransformBox` en `canvas.rs`, y aunque se importe, el arrastre de
la esquina no escala porque el lienzo no resuelve handles: `pointer_press` hace un gesto de
selección.

- [ ] **Step 3: Implementar**

En `crates/cs-ui/src/canvas.rs`:

1. Imports, después de `use crate::guides::{...};`:

```rust
use crate::transform_box::{self, Handle, TransformBox, TransformHit};
```

2. Constantes, después de `TILE_NUMBER_MIN_PX`:

```rust
/// Lado en pantalla de cada handle de la selección, sea cual sea el zoom.
const HANDLE_PX: f32 = 8.0;
/// Franja en pantalla por fuera de cada esquina que rota la selección.
const ROTATE_PX: f32 = 12.0;
```

3. `CanvasView`: después del campo `draw_pipette`, agregar

```rust
    /// Dibujar el cursor de rotación (egui no trae uno; el del sistema se oculta).
    draw_rotate: bool,
```

   y en `Default`, después de `draw_pipette: false,`, agregar `draw_rotate: false,`.

4. `handle_input`: después de `self.draw_pipette = false;`, agregar `self.draw_rotate = false;`.

5. En el bloque que llama a `editor.pointer_press`, después de `self.slice_menu = slice.is_some();`:

```rust
            // Un handle o la franja de rotación ganan sobre el gesto de la herramienta.
            let transform = if pressed_primary {
                self.transform_hit(rect, editor, p)
            } else {
                None
            };
```

   y reemplazar el último brazo del `match over_label`
   (`_ => editor.pointer_press(world, pressed_secondary, modifiers.alt, slop),`) por:

```rust
                _ => match transform {
                    Some(hit) => editor.press_transform(hit, world),
                    None => editor.pointer_press(world, pressed_secondary, modifiers.alt, slop),
                },
```

6. Cursores. Justo antes de `let cursor = if space {`, agregar

```rust
        let transform_cursor = editor
            .transform_drag_hit()
            .or_else(|| pointer.and_then(|p| self.transform_hit(rect, editor, p)));
```

   y en la cadena, entre el brazo `else if over_label.is_some() && !editor.is_painting() { ... }` y
   `else if editor.active_tool() == ToolKind::Artboard {`, agregar:

```rust
        } else if let Some(hit) = transform_cursor {
            match hit {
                TransformHit::Rotate => {
                    self.draw_rotate = true;
                    Some(egui::CursorIcon::None)
                }
                TransformHit::Handle(handle) => Some(handle_cursor(handle)),
                TransformHit::Move => Some(egui::CursorIcon::Move),
            }
```

7. Método nuevo en `impl CanvasView` (después de `guide_at`):

```rust
    /// Handle o franja de rotación bajo `p` (en pantalla), si se muestran los handles.
    fn transform_hit(&self, rect: Rect, editor: &Editor, p: Pos2) -> Option<TransformHit> {
        if !editor.shows_handles() {
            return None;
        }
        let b = editor.transform_box()?;
        let (wx, wy) = self.to_world(rect, p);
        let (ax, ay) = editor.artboard().position;
        let local = (wx - ax as f32, wy - ay as f32);
        let handle = (HANDLE_PX / 2.0 + 1.0) / self.zoom;
        transform_box::hit(&b, local, handle, ROTATE_PX / self.zoom)
    }
```

8. En `show`, después del bloque de `draw_marching_ants`:

```rust
        if let Some(origin) = active_origin
            && editor.shows_handles()
            && let Some(b) = editor.transform_box()
        {
            draw_transform_box(&painter, &b, origin, self.zoom);
        }
```

   y al final, después del bloque `if self.draw_pipette && ... { draw_pipette(...) }`:

```rust
        if self.draw_rotate
            && let Some(p) = response.hover_pos()
        {
            draw_rotate_cursor(&painter, p);
        }
```

9. Funciones libres, después de `artboard_cursor`:

```rust
/// Cursor de redimensionar según la dirección del handle (sin tener en cuenta la rotación).
fn handle_cursor(handle: Handle) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match handle.dirs() {
        (0, _) => C::ResizeVertical,
        (_, 0) => C::ResizeHorizontal,
        (x, y) if x == y => C::ResizeNwSe,
        _ => C::ResizeNeSw,
    }
}

/// Contorno (rotado) de la caja de transformación y sus 8 handles de tamaño fijo en pantalla.
fn draw_transform_box(painter: &egui::Painter, b: &TransformBox, origin: Pos2, zoom: f32) {
    let at = |(x, y): (f32, f32)| origin + Vec2::new(x, y) * zoom;
    let outline: Vec<Pos2> = b.corners().into_iter().map(at).collect();
    painter.add(egui::Shape::closed_line(
        outline,
        Stroke::new(1.0, theme::ACCENT),
    ));
    for handle in Handle::ALL {
        let r = Rect::from_center_size(at(b.handle_point(handle)), Vec2::splat(HANDLE_PX));
        painter.rect_filled(r, 0.0, Color32::WHITE);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
    }
}

/// Flecha curva (sentido horario) en el puntero, para rotar: egui no trae ese cursor.
fn draw_rotate_cursor(painter: &egui::Painter, at: Pos2) {
    use std::f32::consts::PI;
    let r = 7.0;
    let arc: Vec<Pos2> = (0..=12)
        .map(|i| at + Vec2::angled(-PI / 2.0 + i as f32 * (1.5 * PI / 12.0)) * r)
        .collect();
    let end = arc[arc.len() - 1];
    // Al final del arco (a la izquierda) el sentido horario apunta hacia arriba.
    let head = vec![end + Vec2::new(-3.0, 4.0), end, end + Vec2::new(3.0, 4.0)];
    for (width, color) in [(3.0, Color32::BLACK), (1.5, Color32::WHITE)] {
        let stroke = Stroke::new(width, color);
        painter.add(egui::Shape::line(arc.clone(), stroke));
        painter.add(egui::Shape::line(head.clone(), stroke));
    }
}
```

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all`, `cargo test -p cs-ui` y
`cargo check -p cs-ui --target wasm32-unknown-unknown`
Expected: PASS; wasm compila.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src/canvas.rs
git commit -m "Lienzo: handles de la selección, rotar por fuera de las esquinas y cursores

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Opción Rotation (Fast / RotSprite) y estado en la barra inferior

**Files:**
- Modify: `crates/cs-ui/src/transform.rs`
- Modify: `crates/cs-ui/src/panels.rs`

**Interfaces:**
- Consumes: `ToolOptions::rotation`, `Editor::refresh_floating` (Task 3),
  `cs_raster::RotationAlgorithm::{ALL, label}` (Task 2).
- Produces: `Editor::floating_status(&self) -> Option<String>`.

- [ ] **Step 1: Escribir los tests que fallan**

Al módulo `tests` de `crates/cs-ui/src/transform.rs`:

```rust
    #[test]
    fn status_shows_size_angle_and_keys_while_floating() {
        let mut e = editor();
        assert_eq!(e.floating_status(), None);
        assert!(e.lift());
        place(
            &mut e,
            TransformBox {
                x0: 6,
                y0: 0,
                x1: 0,
                y1: 3,
                angle: FRAC_PI_2,
            },
        );
        assert_eq!(
            e.floating_status().as_deref(),
            Some("6×3  90°  Enter to apply · Esc to cancel")
        );
    }

    #[test]
    fn changing_the_rotation_option_recomputes_the_floating_content() {
        let mut e = editor();
        assert!(e.lift());
        place(
            &mut e,
            TransformBox {
                angle: 0.5,
                ..TransformBox::new(0, 0, 3, 3)
            },
        );
        assert_eq!(e.floating().unwrap().rendered_with, RotationAlgorithm::RotSprite);
        e.options.rotation = RotationAlgorithm::Fast;
        e.refresh_floating(false);
        assert_eq!(e.floating().unwrap().rendered_with, RotationAlgorithm::Fast);
    }
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui transform::tests::status`
Expected: FAIL de compilación con "no method named `floating_status`".

- [ ] **Step 3: Implementar**

En `crates/cs-ui/src/transform.rs`, dentro de `impl Editor`:

```rust
    /// Texto para la barra de estado mientras flota: tamaño, ángulo y teclas.
    pub fn floating_status(&self) -> Option<String> {
        let b = self.floating.as_ref()?.bbox;
        let degrees = (b.angle.to_degrees().round() as i32).rem_euclid(360);
        Some(format!(
            "{}×{}  {}°  Enter to apply · Esc to cancel",
            b.width().unsigned_abs(),
            b.height().unsigned_abs(),
            degrees
        ))
    }
```

En `crates/cs-ui/src/panels.rs`:

1. Imports, después de `use cs_tools::{Dither, GradientShape};`:

```rust
use cs_raster::RotationAlgorithm;
```

2. En `tool_options`, justo antes del `if matches!(tool, ToolKind::Line | ...)` que muestra
   "Shift: constrain":

```rust
    if tool.is_selection() || tool == ToolKind::Move {
        let before = editor.options.rotation;
        ui.horizontal(|ui| {
            ui.label("Rotation:");
            for algorithm in RotationAlgorithm::ALL {
                ui.radio_value(&mut editor.options.rotation, algorithm, algorithm.label());
            }
        });
        if editor.options.rotation != before {
            editor.refresh_floating(false);
        }
        ui.weak("Drag the handles to scale, or just outside a corner to rotate. Shift keeps the proportions or snaps to 15°.");
    }
```

3. En `status_bar`, después del bloque `if let Some(text) = editor.tiles_status() { ... }`:

```rust
        if let Some(text) = editor.floating_status() {
            ui.separator();
            ui.colored_label(theme::ACCENT, text);
        }
```

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo fmt --all` y `cargo test -p cs-ui`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cs-ui/src
git commit -m "Opción Rotation (Fast/RotSprite) y estado de la transformación en la barra inferior

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Verificación final y prueba manual

**Files:** ninguno (solo verificación; si algo falla, se arregla en el archivo de la tarea
correspondiente y se commitea aparte).

- [ ] **Step 1: Verificación automática**

Run (con el toolchain exportado):
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo check -p cs-ui --target wasm32-unknown-unknown`

Expected: todo en verde, sin warnings.

- [ ] **Step 2: Prueba manual con la app**

En Git Bash:

```bash
cp "$APPDATA/ChimpSprite/data/app.ron" /tmp/chimpsprite-app.ron.bak
CARGO_TARGET_DIR=target/gui-transform cargo run --bin chimpsprite
```

(en background). Comprobar:
- **Bug de mover:**
  1. Dibujar dos manchas.
  2. Seleccionar una con M y arrastrarla encima de la otra.
  3. Soltar y volver a arrastrarla.
  4. La otra mancha queda intacta.
  5. Enter aplica, y un Ctrl+Z deshace todo el movimiento.
- **Handles y escala:** se ven 8 handles del mismo tamaño en cualquier zoom. Arrastrar una
  esquina escala, con Shift se mantiene la proporción, y arrastrar un lado más allá del otro
  voltea.
- **Rotación:**
  - Justo fuera de una esquina el cursor pasa a la flecha curva.
  - Arrastrar rota sobre el centro, y con Shift va de a 15°.
  - Al soltar se ve RotSprite. Con "Rotation: Fast" en Options cambia en el momento.
- **Barra de estado:** muestra `W×H  N°  Enter to apply · Esc to cancel`.
- **Esc:**
  - Con un lazo poligonal a medias, Esc lo descarta.
  - Flotando, Esc vuelve todo atrás y la selección sigue.
  - Otro Esc deselecciona.
- **Pegar:** Ctrl+C y Ctrl+V. Lo pegado flota sobre el dibujo sin mezclarse. Se mueve, escala o
  rota, y Enter lo aplica. Ctrl+Z mientras flota lo cancela.
- **Confirmación:**
  - Cambiar de herramienta, de capa (también con ↑/↓) o de frame confirma.
  - Ctrl+S con algo flotando guarda lo transformado.
  - Bloquear la capa mientras flota lo cancela.

Al terminar, cerrar la app y restaurar:

```bash
cp /tmp/chimpsprite-app.ron.bak "$APPDATA/ChimpSprite/data/app.ron"
```

- [ ] **Step 3: Sin push ni PR**

El usuario junta todas las ramas al final. Al juntar con `paste-external` hay que resolver
`Editor::paste`: queda el cuerpo de esta rama, que deja lo pegado flotando (ver
Desviaciones/decisiones).

---

## Autorrevisión

**Cobertura del spec:**
- **§1 Algoritmos:**
  - `Block` y escala con volteos: Task 1.
  - Fast, RotSprite (Scale2x ×3, rotación y reducción) y 0/90/180/270 exactos: Task 2.
  - La máscara viaja con `None`: Tasks 1 y 2.
- **§2 Estado flotante:**
  - `Floating`: Task 3.
  - Levantar al arrastrar un handle: Task 7. Al mover arrastrando, con Move o con selección:
    Task 4. Al pegar: Task 6.
  - Levantar una sola vez y no mezclar: Task 4 (test de regresión).
  - El hueco solo en la vista previa: Task 3 (`floating_view`).
  - Fast durante el arrastre y RotSprite al soltar, con caché: Tasks 4 y 7
    (`rendered_with`, test en Task 7).
  - Confirmar con un solo `DrawPixels` sin borrar con transparentes, más la selección
    transformada: Task 3.
  - Confirmar con Enter: Task 5. Con clic afuera: Task 4. Al cambiar de herramienta, capa, frame
    o artboard: Tasks 3 y 5 (`sync_floating`). Al copiar, guardar o exportar: Task 5. Al cortar o
    borrar: Task 3.
  - Esc por partes: Task 5. Cancelar con Esc o Ctrl+Z, y el pegado vuelve a la selección
    anterior: Tasks 3, 5 y 6.
  - Mover solo cambia la caja: Task 4. Flechas: ver Desviaciones.
  - Límites (capas de imagen, sin bloquear, sin Tiles, sin máscara): `can_transform` en Task 3 y
    test `handles_show_only_where_the_clipboard_works` en Task 7.
- **§3 Lienzo:**
  - 8 handles de 8 px fijos, con selección y Move: Task 8.
  - Escalar con el lado opuesto fijo y Shift: Task 7.
  - Rotar en una franja de 12 px, con cursor dibujado y Shift de a 15°: Tasks 7 y 8.
  - Cursores por handle, Move adentro y rotación afuera: Task 8.
  - `Rotation: Fast / RotSprite` en `ToolOptions`: Tasks 3 y 9.
  - Barra de estado: Task 9.
- **§4 Casos borde:**
  - Mínimo 1 px: Task 7 (`move_edge`).
  - Lo que sale del lienzo se pierde al confirmar: Task 3 (`draw_pixels` filtra y `mask()` usa
    `from_points`).
  - Límite de 4096: Tasks 4 y 7 (test `a_handle_stops_at_the_size_limit`).
  - La capa se borra o se bloquea: Task 3 (`sync_floating`, test).
- **§5 Tests:** cada caso listado tiene su test en Tasks 1–8. Los tests de lienzo (escalar desde
  una esquina, rotar fuera de una esquina y confirmar con Enter) están en Task 8.

**Placeholders:** no quedan "TBD", "TODO" ni "similar a…". Cada paso de código trae el código
completo, y los reemplazos dicen qué líneas cambian.

**Consistencia de tipos:**
- `TransformBox { x0, y0, x1, y1, angle }` con `new(x, y, u32, u32)` en todas las tareas.
- `TransformHit::{Move, Handle(Handle), Rotate}`: Task 4 crea `Move` y Task 7 agrega el resto,
  junto con los brazos del `match` de `drag_transform`.
- `refresh_floating(fast: bool)` desde Task 3.
- `set_floating_box(bbox, fast) -> bool` desde Task 4.
- `release_transform() -> Option<(i32, i32)>` lo usa `pointer_release`.
- `editor_key(&mut Editor, Key, bool)` lo usan Task 5 y el harness de Task 8.
- `float_pasted(Block<PixelValue>, (i32, i32))` lo usa `paste`.
- `rendered_with` se lee en los tests de Tasks 7 y 9, que están dentro de `transform.rs`.
