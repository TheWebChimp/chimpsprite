# Workspace: pestañas, Home, menú View y guías — Diseño

Fecha: 2026-10-14
Estado: aprobado

## 1. Pestañas de documentos

- Cada pestaña es un `DocState`: editor (con su historial), vista del lienzo, estado del timeline, ubicación
  (`DocLocation`), versión guardada y texto del nombre del artboard. La app tiene el `DocState` activo en
  `doc` y las demás en una lista; cambiar de pestaña intercambia.
- Barra de pestañas bajo el menú: **Home** + una por documento (`nombre •` si hay cambios, `×` para
  cerrar). `Ctrl+Tab` / `Ctrl+Shift+Tab` cambian de pestaña; `Ctrl+W` cierra (en web solo con `×`).
- Abrir, crear o soltar un archivo abre **una pestaña nueva**; si el archivo (misma ruta) ya está abierto,
  cambia a esa pestaña. Si la pestaña activa es un "Untitled" intacto (sin cambios ni archivo), se
  reemplaza.
- Cerrar una pestaña con cambios pregunta Save / Discard / Cancel. Salir de la app con documentos sin
  guardar muestra la lista y ofrece Save all / Discard all / Cancel.
- La app arranca en Home (con `--demo`/`?demo`, un archivo por línea de comandos o sesiones restauradas,
  en esas pestañas).

## 2. Home

- Acciones: New, Open…, Open demo.
- Escritorio: **archivos recientes** (15) y **carpetas recientes** (10, derivadas de los archivos), que
  persisten entre sesiones con el almacenamiento de eframe. Clic abre el archivo.
- Web: no hay rutas; el autosave guarda **todas** las pestañas con cambios (tope total 4 MB) y Home
  ofrece restaurarlas.

## 3. Menú View

- Show: Rulers, Guides, Pixel Grid, Grid, Selection Edges, Brush Preview, Artboard Names. `Ctrl+H`
  alterna todos los extras.
- Grid configurable por artboard (tamaño 16×16 por defecto, desplazamiento, color) + Snap to Grid.
- Guides: Snap to Guides, Lock Guides, Clear Guides.
- Onion Skin (`F3`), Timeline (`Tab`), Full Screen (`F11`, solo escritorio).
- Zoom In (`+`), Zoom Out (`-`), 100% (`Ctrl+0`), Fit (`Ctrl+Shift+0`).

## 4. Guías

- `Artboard::guides: Vec<Guide { axis: Horizontal|Vertical, position: i32 }>` en coordenadas locales,
  siempre en bordes de píxel. `Artboard::grid: GridSettings { size, offset, color }`.
- `DocCommand::SetGuides { id, guides }` y `SetGrid { id, grid }` (con undo).
- Reglas arriba e izquierda del lienzo (en píxeles del artboard activo, origen en su esquina superior
  izquierda, marcas adaptadas al zoom, indicador de la posición del cursor).
- Arrastrar desde una regla crea una guía; arrastrar una guía la mueve; soltarla sobre la regla la
  borra. Bloqueadas: no se pueden mover.
- Snap (a guías y/o grid, a menos de 6 px de pantalla): esquinas de selección, rectángulo, elipse y línea;
  desplazamiento de Mover; bordes al crear/redimensionar artboards. El píxel elegido queda del lado de la
  guía donde está el cursor.
- Archivos: guías y grid en el manifest de `.chimp`. `.aseprite` guarda el grid en su header (tamaño y
  offset) pero no guías: se avisa al guardar si se pierden.

## Testing

Lógica pura con tests: gestión de pestañas (abrir duplicado, reemplazar Untitled intacto, cerrar con
cambios), recientes, marcas de reglas, hit-test y snap de guías, comandos de guías/grid con undo,
round-trip en `.chimp` y del grid en `.aseprite`. Capturas en escritorio y web.
