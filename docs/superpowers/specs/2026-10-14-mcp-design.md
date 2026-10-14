# MCP server (headless) — Diseño

Fecha: 2026-10-14
Estado: aprobado

Servidor MCP para que un agente (Claude u otro) cree, dibuje, anime, lea y exporte sprites sin la
UI. Primero headless; la versión "en vivo" dentro de la app reusará `cs-agent` más adelante.

## 1. Arquitectura

```
crates/cs-agent        lógica sin protocolo (también compila a wasm)
  session.rs   Session: documentos abiertos ("d1", "d2"…) con Document, History<DocCommand>, ruta
  params.rs    parámetros de cada herramienta (serde + schemars)
  target.rs    resolver artboard / capa / frame / color
  ops/         una función por grupo de herramientas → Result<Reply, AgentError>
  text.rs      píxeles como cuadrícula de texto
  quantize.rs  median cut + color más cercano
apps/chimpsprite-mcp   binario: rmcp (SDK oficial) + tokio, stdio
```

- El estado vive en memoria mientras el proceso corre. Varios documentos a la vez.
- Todo cambio es un `DocCommand` aplicado con `History`: undo/redo y cada llamada es atómica
  (`Batch`; si falla no queda nada a medias).
- Nuevo `DocCommand::SetSprite { id, sprite }` (reemplaza el sprite completo, inverso = el anterior)
  para operaciones que cambian el modo de color (cuantizar a indexado).
- `files.rs` (codificar/decodificar por extensión) se mueve de `cs-ui` a `cs-io::files` para que lo
  compartan la UI y el agente.
- Direccionamiento: `doc` obligatorio; `artboard` (nombre o id, por defecto el primero), `layer`
  (nombre o id; por defecto la capa de imagen de más arriba), `frame` (índice desde 0, por defecto 0).
- Colores: `#rgb`, `#rrggbb`, `#rrggbbaa` o `"transparent"`; en sprites indexados también un
  número (índice de paleta); en indexados un hex se convierte al índice exacto de la paleta (error si
  no está).
- Coordenadas: píxeles locales del artboard, enteros.
- Rutas: relativas al directorio de trabajo; con `--root <dir>` toda ruta debe quedar dentro.
- `Reply { text, image: Option<png> }`; los errores (`AgentError`) se devuelven como resultado de
  herramienta con `isError: true` y un mensaje claro.

## 2. Herramientas

**Documentos**: `new_document(width, height, color_mode?, name?)`, `open(path)` (.aseprite/.ase,
.chimp, .png), `save(doc, path?)` (formato por extensión; avisa si se pierden guías al guardar en
.aseprite; sin `path` usa la ruta de origen), `close(doc)`, `list_documents()`, `describe(doc)` (JSON:
artboards, capas abajo→arriba con id/nombre/tipo/padre/visible/bloqueada/opacidad/blend, frames con
duración, tags, paleta, cels con posición, tamaño e id de imagen; los cels con la misma imagen están
vinculados).

**Estructura**: `add_artboard(name, width, height, position?)`, `set_artboard(name?, position?,
width?, height?)`, `remove_artboard`; `add_layer(name?, group?, above?, into?)`, `set_layer(name?,
visible?, locked?, opacity?, blend?)`, `remove_layer`, `move_layer(above? | below? | into?)`;
`add_frame(at?, duplicate?, linked?, duration_ms?)`, `remove_frame`, `move_frame(from, to)`,
`set_frame_duration(frames?, duration_ms)`; `set_tag(name, from, to, direction?, color?, repeat?)`
(crea o reemplaza por nombre), `remove_tag(name)`; `link_cels(layer, from, to)`, `unlink_cels`,
`clear_cel`, `move_cel(dx, dy, frames?)` (mueve el cel entero; los vinculados siguen vinculados).

**Dibujo** (todas con `brush_size?` y `clip?` = rectángulo que recorta): `draw_pixels(pixels:
[{x, y, color}])`, `draw_line(points: [[x, y]…], color)` (polilínea), `draw_rect(x0, y0, x1, y1,
color, filled?)`, `draw_ellipse(…)`, `fill(x, y, color, contiguous?)` (sobre los píxeles de la capa),
`erase` = dibujar con `"transparent"`.

**Color**: `set_palette(colors)`, `recolor(map: [{from, to}], scope?)` con scope `cel` (por
defecto), `layer` (todos los frames), `frame` (todas las capas) o `sprite`; `quantize(max_colors? |
palette?, to_indexed?)`: median cut sobre todos los colores visibles (o la paleta dada), remapea cada
píxel al más cercano y deja esa paleta en el sprite; con `to_indexed` pasa el sprite a indexado (índice
0 transparente).

**Transformaciones** (`layer?` y `frames?` = rango, por defecto el frame dado; `rect?` = región, por
defecto el lienzo): `flip(axis)`, `rotate(degrees: 90|180|270)` (90/270 exigen región cuadrada),
`shift(dx, dy, wrap?)`, `copy_region(rect, from: {layer, frame}, to: {layer, frame, x, y})`. Los cels
vinculados se transforman una sola vez.

**Animación**: `inbetween(from, to, count)`: inserta `count` frames antes de `to` (exige `from < to`).
Para cada capa con cel en `from` los frames nuevos enlazan esa imagen; si también hay cel en `to` con
la misma imagen, la posición se interpola linealmente (redondeada). Duración = la de `from`.

**Ver y leer**: `render(doc, artboard?, frame?, scale?, onion?)` → imagen PNG (escala por defecto:
la mayor que deje el lado mayor ≤ 512, entre 1 y 16). `read_pixels(rect?, layer?, frame?)` → texto:
encabezado, leyenda (`.` = transparente, luego `a-z A-Z 0-9`; con más de 62 colores, códigos de dos
caracteres) y una fila por línea. Sin `layer` lee la composición visible. Máximo 128×128.

**Salida e historial**: `export(doc, path, kind: png|gif|sequence|sheet|atlas, scale?, frame?,
layout?, columns?)` (sheet/atlas escriben además el `.json`), `undo(doc)`, `redo(doc)`,
`apply_commands(doc, commands)`: `DocCommand` en JSON tal como los serializa serde; atómico.

## 3. Configuración

`.mcp.json` en la raíz del repo para Claude Code:
`{"mcpServers": {"chimpsprite": {"command": "cargo", "args": ["run", "-q", "-p",
"chimpsprite-mcp", "--release"]}}}`. README con instrucciones para otros clientes.

## Testing

`cs-agent`: tests por herramienta (efecto, undo, errores claros), roundtrip de `read_pixels`,
quantize, inbetween, transformaciones con vínculos y `--root`. Binario: test de integración que lanza
el proceso, hace `initialize`, `tools/list` y unas llamadas reales (crear, dibujar, render) por stdio.
