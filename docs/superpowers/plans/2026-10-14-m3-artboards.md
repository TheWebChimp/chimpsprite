# M3 — Documento + artboards — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Un documento con varios artboards (sprites independientes) en un lienzo infinito, con comandos e historial a nivel documento y una herramienta Artboard para crear, mover y redimensionar.

**Architecture:** `Document`/`Artboard` en `cs-core` envuelven el `Sprite` existente. `cs-commands` agrega `DocCommand` y vuelve `History` genérico sobre un trait `Undoable` (lo implementan `Command` sobre `Sprite` y `DocCommand` sobre `Document`). La herramienta Artboard vive en `cs-tools` y trabaja en coordenadas de mundo. La UI dibuja cada artboard con su propia textura.

Spec: sección "Artboards" de `docs/superpowers/specs/2026-10-14-chimpsprite-core-design.md`.

## Global Constraints

- Las de M1/M2. `Sprite` se vuelve serializable (serde) para que `DocCommand` lo sea.
- No se puede eliminar el último artboard (`CmdError::LastArtboard`).
- Atajo de la herramienta Artboard: Shift+O.

---

### Task 1: `Document` y `Artboard` en `cs-core`; `Sprite` serializable

**Produces:** `ArtboardId(u32)`, `Artboard { id, name, position: (i32, i32), sprite }` + `Artboard::rect() -> ((i32, i32), (i32, i32))` (min inclusivo, max exclusivo), `Document { artboards }` + `Document::{with_sprite(name, sprite), artboard(id), artboard_mut(id), index(id), next_artboard_id(), artboard_at(p: (f32, f32)) -> Option<ArtboardId>` (el de más arriba), `bounds() -> Option<((i32, i32), (i32, i32))>}`. `Sprite: Serialize + Deserialize` (mapas con claves compuestas serializados como listas).

**Tests:** `artboard_at` devuelve el de arriba cuando se solapan y `None` en vacío; `bounds` une todos; JSON round-trip de un `Sprite` con cels.

### Task 2: `ResizeCanvas`, `DocCommand`, `History` genérico

**Produces:** `Command::ResizeCanvas { width, height, offset: (i32, i32) }` (inverso: tamaño viejo y `-offset`; error `InvalidSize` si algún lado es 0). `trait Undoable: Sized { type Target; fn apply(&self, target: &mut Self::Target) -> Result<Self, CmdError>; }` implementado por `Command` y `DocCommand`. `History<C: Undoable>`. `DocCommand::{OnArtboard { id, cmd }, AddArtboard { index, artboard }, RemoveArtboard { id }, SetArtboardProps { id, name: Option<String>, position: Option<(i32, i32)> }, Batch(Vec<DocCommand>)}`. Errores nuevos: `ArtboardNotFound`, `DuplicateArtboardId`, `LastArtboard`, `InvalidSize`.

**Tests:** resize con offset mueve cels y undo restaura; add/remove/props round-trip; no se elimina el último; batch con fallo revierte; `OnArtboard` aplica al sprite correcto; property test existente sigue pasando con `History<Command>`; property test nuevo de documento (dibujar en artboards aleatorios, agregar/mover artboards, undo todo = inicial).

### Task 3: Herramienta Artboard en `cs-tools`

**Produces:** `ArtboardTool` con `press(&mut self, doc: &Document, p: (f32, f32), slop: f32) -> Option<ArtboardId>` (devuelve el artboard a activar), `drag(&mut self, p: (f32, f32))`, `release(&mut self, doc: &Document) -> Option<DocCommand>`, `preview(&self, doc: &Document) -> Option<DocCommand>`, `is_active()`. Prioridad de hit: borde/esquina de un artboard (dentro de `slop`) → redimensionar; interior → mover; vacío → crear. Crear produce `AddArtboard` con un sprite nuevo del modo de color y paleta del artboard de referencia (el activo pasado en `press`). Redimensionar produce `Batch[OnArtboard(ResizeCanvas), SetArtboardProps(position)]` para que el contenido quede fijo en el mundo; tamaño mínimo 1×1.

**Tests:** arrastrar en vacío crea con el rect correcto; arrastrar dentro mueve; arrastrar el borde izquierdo agranda, cambia la posición y el contenido conserva su posición de mundo; clic sin arrastre no produce comando.

### Task 4: Editor + UI

- `Editor` pasa a tener `document: Document`, `active: ArtboardId`, `History<DocCommand>`; `sprite()` devuelve el sprite activo; `execute(Command)` envuelve en `OnArtboard`; `execute_doc(DocCommand)`; recuerda capa/frame por artboard; al activar otro artboard descarta la selección; `ToolKind::Artboard`.
- Las herramientas de sprite reciben coordenadas locales (mundo − posición del activo). Un clic sobre otro artboard lo activa antes de empezar el gesto.
- Lienzo: zoom/paneo en mundo, ajuste inicial a `bounds()`; cada artboard con damero, textura propia, grid, borde y nombre; el activo con borde de acento; vista previa de la herramienta Artboard aplicando su comando sobre un clon del documento.
- Panel "Artboards" (derecha): lista (clic activa), nombre editable del activo, botones nuevo (64×64 a la derecha del último) y eliminar (deshabilitado con uno solo).
- Barra de estado: nombre del artboard activo y coordenadas locales.
- Demo: dos artboards (chimpancé 64×64 y plátano 32×32).

**Tests (editor):** dibujar afecta solo al artboard activo; undo cronológico entre artboards; activar otro descarta la selección y recuerda la capa; eliminar el último no hace nada.

**Verificación:** fmt, clippy, tests, captura, app abierta para el usuario, PR con CI verde.
