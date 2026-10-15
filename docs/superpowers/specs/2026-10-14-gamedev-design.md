# Pipeline game-dev — Diseño

Fecha: 2026-10-14
Estado: aprobado (el usuario pidió avanzar sin preguntas)

## 1. Slices (hitboxes y pivotes)

- Modelo nativo compatible con Aseprite: `Sprite::slices: Vec<Slice { name, color, text, keys }>`,
  `SliceKey { frame, bounds, center (9-slice), pivot }`. Una clave vale desde su frame hasta la
  siguiente; ancho 0 = el slice desaparece desde ese frame.
- `.aseprite`: chunk 0x2022 leído y escrito (con user data: color y texto), ya no como bytes crudos.
- `Command::SetSlices`; insertar o quitar frames corre las claves (y lo deshace).
- Herramienta **Slice** (Shift+C): arrastrar crea un slice (vale en todos los frames); con uno
  seleccionado, arrastrar adentro lo mueve y desde un borde lo redimensiona, con una clave en el
  frame actual (hitboxes por frame). Clic sobre un slice lo selecciona.
- Panel **Slices**: lista, nombre, color, datos (texto), rectángulo y pivote del frame actual,
  quitar la clave del frame, ocultar desde el frame, borrar. View › Show › Slices.
- JSON de spritesheet/atlas: `meta.slices` en el formato de Aseprite (escalado; en atlas con
  prefijo `artboard/` y frames globales).

## 2. Exportación para motores (Export y MCP)

Todas usan la hoja con el layout elegido.
- **Godot 4**: `SpriteFrames` `.tres` con una animación por tag (dirección aplicada, ping-pong sin
  repetir extremos), velocidad = frame más corto y duraciones relativas; `loop` si el tag no tiene
  repeticiones. Carpeta `res://` configurable.
- **Unity**: `<hoja>.png.meta` con textura sprite múltiple, filtro point, sin compresión, un sprite
  por frame (y invertida como Unity) y pivote del primer slice con pivote; guid y spriteID estables.
- **Bevy**: `.ron` con `tile_size`, `columns`, `rows` (para `TextureAtlasLayout::from_grid`),
  animaciones (frames y duraciones) y slices.

## 3. Normal maps

`cs_render::normal_map`: altura = distancia al borde (bisel de 3 px) + 35 % de luminosidad, Sobel,
convención OpenGL (verde arriba, como Godot y Unity); transparentes con normal plana y alfa 0.
Opción "Also export normal map (_n.png)" para PNG, hoja y motores; cada frame se calcula aparte.

## MCP

`set_slice(name, frame?, x, y, w, h, pivot?, color?, text?)`, `remove_slice`; `describe` incluye los
slices; `export` gana `godot`, `unity`, `bevy`, `normal_map` y `godot_folder`.
