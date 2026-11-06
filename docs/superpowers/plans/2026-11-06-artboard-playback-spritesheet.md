# Plan: Artboards — reproducción en el lugar y spritesheets

Spec: `docs/superpowers/specs/2026-11-06-artboard-playback-spritesheet-design.md`. TDD, un commit por
tarea (tests + clippy en verde).

1. **Motor de hojas (cs-io).** `spritesheet.rs`: `SheetOrder`, `ExpandSettings`, `auto_columns`,
   `fit`, `cell_position`, `expand_frames` (aplanado, indexado si se puede, grilla);
   `SheetGrid::from_counts`, `sprite_from_sheet_ordered`. Tests de layout, orden, spacing/border,
   indexado y la ida y vuelta.
2. **Play in Place en el editor.** `editor/in_place.rs`: estado, `toggle_play_in_place`,
   `is_playing_in_place`, `can_play_in_place`, `tick_in_place` (devuelve el próximo cambio),
   `frame_of` con el frame en el lugar para los no activos; limpieza al borrar y rango recalculado.
   Tests en `editor/in_place_tests.rs`.
3. **Canvas y app.** Textura recompuesta por frame (`texture_frames`); tick en `lib.rs` con
   `InputState::time` y `request_repaint_after`.
4. **Barra de título.** `kit::title_bar`: `TitleState::play` y `TitleParts::play`; íconos
   `icon.title_play` / `icon.title_stop` en el skin (provisionales, por MCP de Aseprite o PIL);
   `WindowLayout::play`, `TitleHit::Play`; clic sin activar. Catálogo del kit con el ▶. Tests de
   layout y de clic.
5. **Menú del artboard.** Clic derecho en la barra/etiqueta (y con la herramienta Artboard) abre el
   menú "≡"; ítems Play in Place, Expand to Spritesheet…, Slice into Frames… con
   `ArtboardRequest::{ExpandSheet, SliceFrames}`.
6. **Diálogo Expand.** `expand_dialog.rs` con vista previa; la app materializa, arma el sprite y lo
   agrega (un undo). Tests de diálogo.
7. **Slice into Frames.** `SheetDialog` con propósito (Import/Slice), modo Columns × Rows, orden y
   duración; la app agrega el artboard. Tests.
8. **Demo y capturas.** Variables `CHIMPSPRITE_DEMO_IN_PLACE`, `_EXPAND`, `_SLICE`; capturas
   revisadas.
9. **Verificación final:** test workspace, clippy, fmt, wasm; autorrevisión del diff; bitácora.
