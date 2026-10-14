# M2 — Herramientas + selección — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Completar las herramientas de dibujo de la v1: cubeta (contigua/global), línea, rectángulo, elipse (borde/relleno), selección rectangular, mover y cuentagotas, con la selección recortando la pintura.

**Architecture:** Geometría nueva en `cs-raster` (funciones puras). `Mask` en `cs-core` (bitmask del tamaño del lienzo; será la base de las máscaras del subproyecto 4). El trait `Tool` pasa a recibir el sprite en `ToolContext<'a>` y devuelve un `ToolOutput` (comando, color elegido, cambio de selección). La vista previa se vuelve exacta: el editor aplica el comando de preview sobre un clon barato del sprite (imágenes en `Arc`) y compone ese clon.

**Tech Stack:** igual que M1.

Spec: `docs/superpowers/specs/2026-10-14-chimpsprite-core-design.md` · Plan previo: `2026-10-14-m1-engine-and-canvas.md`

## Global Constraints

- Las mismas de M1 (capas sin egui, toda mutación vía `Command`, logging con `log`).
- La selección vive en el `Editor`, no en el documento.
- Herramientas de pintura (lápiz, borrador, línea, rect, elipse, cubeta) recortan a la selección si existe; mover no.
- Atajos: B lápiz, E borrador, G cubeta, L línea, U rectángulo/elipse (alterna), M selección, V mover, I cuentagotas, Alt+clic = cuentagotas temporal, Shift = restringir (45°/cuadrado/círculo), Ctrl+A seleccionar todo, Ctrl+D deseleccionar, Supr borrar contenido seleccionado.

---

### Task 1: Geometría en `cs-raster`

**Produces:** `rect_outline(a, b) -> Vec<Point>`, `rect_filled(a, b)`, `ellipse_outline(a, b)` (inscrita en la caja `a`–`b`, algoritmo de Zingl, sin duplicados), `ellipse_filled(a, b)`, `flood_fill(width: u32, height: u32, start: Point, matches: impl Fn(i32, i32) -> bool) -> Vec<Point>` (4-conexo), `constrain_line(a, b) -> Point` (ajusta `b` a 0°/45°/90°), `constrain_square(a, b) -> Point`, `stamp(points: &[Point], size: u32) -> Vec<Point>` (pincel sobre cada punto, sin duplicados, orden estable).

**Tests:**
- rect 1×1 → un punto; rect 3×3 outline → 8 puntos, filled → 9; caja invertida = misma salida.
- ellipse: caja degenerada → un punto; caja plana (0,0)-(3,0) → 4 puntos en la fila 0; para cajas 5×5 y 8×6: todos los puntos dentro de la caja, toca los 4 bordes, simétrica horizontal y verticalmente, sin duplicados; filled ⊇ outline y contiene el centro.
- flood_fill: región en L se llena completa y no cruza diagonales; start fuera de rango → vacío.
- constrain_line (0,0)-(5,1) → (5,0); (0,0)-(4,5) → (5,5) o (4,4) (diagonal); constrain_square (0,0)-(3,-7) → (7,-7).

### Task 2: `Mask` en `cs-core` + lectura de píxeles

**Produces:** `Mask { width, height, bits }` con `new(w, h)`, `from_rect(w, h, a: (i32, i32), b: (i32, i32))` (inclusivo, normalizado, recortado al lienzo), `all(w, h)`, `contains(x: i32, y: i32) -> bool`, `is_empty()`, `bounds() -> Option<((i32, i32), (i32, i32))>`, `translated(dx, dy) -> Mask`, `points() -> impl Iterator<Item = (i32, i32)>`. `Sprite::pixel(layer, frame, x, y) -> PixelValue` (valor del cel; transparente si no hay cel o cae fuera).

**Tests:** from_rect recorta y normaliza; translated recorta al salir del lienzo; bounds de máscara vacía = None; `pixel` respeta posición del cel.

### Task 3: Nueva interfaz de herramientas + lápiz/borrador migrados

**Produces:**
```rust
pub struct ToolContext<'a> { pub sprite: &'a Sprite, pub layer: LayerId, pub frame: usize, pub color: PixelValue,
    pub brush_size: u32, pub pixel_perfect: bool, pub filled: bool, pub contiguous: bool,
    pub constrain: bool, pub selection: Option<&'a Mask> }
#[derive(Default)] pub struct ToolOutput { pub command: Option<Command>, pub pick: Option<PixelValue>, pub selection: Option<Option<Mask>> }
pub trait Tool {
    fn press(&mut self, ctx: &ToolContext, p: Point);
    fn drag(&mut self, ctx: &ToolContext, p: Point);
    fn release(&mut self, ctx: &ToolContext) -> ToolOutput;
    fn preview(&self, ctx: &ToolContext) -> ToolOutput;   // lo que pasaría si se soltara ahora
    fn is_active(&self) -> bool;
}
```
Tests de M1 adaptados (lápiz, borrador, pixel-perfect) al nuevo `ToolOutput`.

### Task 4: Línea, rectángulo, elipse, cubeta, cuentagotas

- `ShapeTool::{line(), rectangle(), ellipse()}`: press fija inicio, drag actualiza fin (con `constrain`), release → `DrawPixels` con `stamp(puntos, brush_size)`; rect/elipse usan `filled`.
- `Bucket`: al soltar, lee `ctx.sprite.pixel(...)`; si el color objetivo == color de relleno no hace nada; `contiguous` → `flood_fill`, si no → todos los píxeles iguales al objetivo.
- `Eyedropper`: press/drag registran punto; release → `pick = Some(pixel de la capa actual)`.

**Tests:** línea con Shift queda horizontal; rect relleno 3×3 = 9 píxeles; cubeta contigua no cruza un muro; cubeta global pinta píxeles desconectados; cubeta sobre el mismo color → sin comando; cuentagotas devuelve el valor del píxel.

### Task 5: Selección y mover

- `RectSelect`: drag define rect; release → `selection = Some(Some(Mask::from_rect))`; clic sin arrastre → `Some(None)` (deseleccionar). `preview` devuelve la selección en curso.
- `MoveTool`: con selección, release → `DrawPixels` [transparente en cada punto de la máscara] + [píxeles no transparentes desplazados], y `selection = Some(Some(mask.translated))`; sin selección, `SetCel` con la posición del cel desplazada. Desplazamiento 0 → nada.

**Tests:** mover con selección deja hueco transparente y copia en destino; la selección se desplaza; mover sin selección cambia `cel.position`; undo restaura.

### Task 6: Editor + UI

- `Editor`: `selection: Option<Mask>`, `filled`, `contiguous`; aplica `ToolOutput` (recorta `DrawPixels` a la selección para herramientas de pintura, aplica `pick` a primario/secundario según botón, aplica cambio de selección); Alt al presionar = cuentagotas temporal; `composite()` devuelve el render del sprite con el preview aplicado sobre un clon; `select_all`, `deselect`, `clear_selection_content`.
- Lienzo: recompone cada frame mientras hay herramienta activa; dibuja contorno de la selección (marching ants animadas) y la selección en curso.
- Toolbar con las 9 herramientas y opciones contextuales (tamaño de pincel, pixel-perfect, relleno, contiguo).
- Atajos de la sección Global Constraints.

**Tests (editor, sin UI):** pintar con selección recorta; Alt+clic elige color; clic derecho con cuentagotas fija secundario; Supr limpia la selección; Ctrl+A/Ctrl+D.

**Verificación:** `cargo test --workspace`; captura con `CHIMPSPRITE_DEMO=1 CHIMPSPRITE_SCREENSHOT=...`; abrir la app para el usuario; PR a `main` con CI en verde.
