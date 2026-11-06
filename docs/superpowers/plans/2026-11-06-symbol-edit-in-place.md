# Plan: Edit in Place de símbolos (movie clips parte 2)

Spec: `docs/superpowers/specs/2026-11-06-symbol-edit-in-place-design.md`. Rama `movie-clips-2`.
TDD por tarea; un commit por tarea con tests + clippy en verde.

## Tarea 1 — Pila con niveles en su lugar (`symbol_ops.rs`, `editor.rs`)

- `SymbolLevel { id, place: Option<InPlace> }` reemplaza a `ArtboardId` en `symbol_stack`.
- `truncate_symbols(depth)`: recorta devolviendo las `home` (del más adentro al más afuera).
  Lo usan `exit_symbol_to`, `activate` (al salir del modo) y `ensure_valid`.
- `enter_symbol_in_place(layer, object) -> bool` y `sync_in_place()` (posiciones; en
  `ensure_valid`, que también recorta si la instancia ya no está).
- `document_for_save()`: copia con las `home`.
- Migaja: "Scene".
- Tests: entrar, posición, frame inicial, anidado, volteado → aislado, undo que borra el objeto,
  guardar.

## Tarea 2 — Doble clic, Esc y menú del objeto

- `open_object_at` entra en su lugar; `enter_instance` sigue aislado.
- Doble clic dentro del modo en su lugar: no encuadra.
- `escape()`: sale un nivel si no hay nada que cancelar.
- Menú contextual del objeto: "Edit in Place" y "Edit Symbol".
- Guardar/autosave con `document_for_save`.
- Tests: doble clic (editor), pintar en su lugar (píxel correcto, un undo), Esc.

## Tarea 3 — Contexto atenuado en el lienzo

- `Editor::in_place_context(document) -> Option<(Document, Vec<ArtboardId>)>` (copia sin la
  instancia editada; los artboards a dibujar).
- `CanvasView`: texturas del contexto (se recomponen con la revisión, la pila o al pintar), se
  dibujan antes del símbolo con el velo; sin ventana ni nombres para el símbolo en su lugar.
- Tests: contexto oculta la instancia (también anidada); el lienzo compone el contexto.

## Tarea 4 — Ícono de la capa de objetos

- Piezas `icon.layer.objects(.selected)` en `assets/ui/skin.aseprite` (provisional).
- Test: con el skin real, la fila de la capa de objetos dibuja la pieza.

## Tarea 5 — Demo, capturas y verificación

- `CHIMPSPRITE_DEMO_SYMBOL=inplace` (o similar) para la captura.
- Verificación completa (tests, clippy, fmt, wasm) y capturas.
