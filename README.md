# ChimpSprite

Editor de sprites y pixel art open source, escrito en Rust puro. Inspirado en Aseprite y en la línea de
la suite [ArtCraft](https://github.com/storytold). Reimplementación clean-room.

> Estado: **pre-alpha** — en desarrollo activo.

## Correr

```sh
cargo run -p chimpsprite
```

## Game dev

- **Slices** (Shift+C) para hitboxes y pivotes por frame, compatibles con los de Aseprite.
- **Export** › Godot 4 (`SpriteFrames` `.tres`), Unity (`.png.meta` con un sprite por frame) y Bevy
  (`.ron` con la grilla, animaciones y slices). Para Bevy:
  `TextureAtlasLayout::from_grid(tile_size, columns, rows, None, None)`.
- **Normal maps** opcionales (`_n.png`) para luces 2D, con el algoritmo de
  [Sprite Fusion Sprite Normals](https://github.com/Hugo-Dz/spritefusion-sprite-normals).
- **Lumen** (en el dock): genera o carga el normal map y la máscara de luminance de un sprite y los
  prueba con una luz que se arrastra (altura, falloff, brillo, ambiente); exporta los mapas o
  hornea la luz en una capa.

## Servidor MCP (agentes)

`chimpsprite-mcp` deja que un agente (Claude, etc.) cree, dibuje, anime, lea y exporte sprites sin la
UI: documentos, artboards, capas, frames, tags, líneas/figuras/relleno, recolor, reducción de paleta,
transformaciones, inbetween, `render` (devuelve la imagen), `read_pixels` (cuadrícula de texto) y
export a PNG/GIF/secuencia/spritesheet/atlas. Todo con undo/redo.

```sh
cargo build --release -p chimpsprite-mcp
```

Claude Code lo toma de [`.mcp.json`](.mcp.json) al abrir este repo. En otros clientes MCP, registra el
binario `target/release/chimpsprite-mcp` (stdio). Con `--root <carpeta>` solo puede leer y escribir
archivos dentro de esa carpeta.

## Arquitectura

Workspace por capas: `cs-core` (modelo) → `cs-raster` → `cs-commands` (undo/redo) → `cs-render` →
`cs-tools` → `cs-ui` (egui) → `apps/chimpsprite`. `cs-agent` (herramientas para agentes) →
`apps/chimpsprite-mcp`. Ver `docs/superpowers/specs/`.

## Licencia

Dual: [MIT](LICENSE-MIT) o [Apache-2.0](LICENSE-APACHE), a tu elección.

El generador de normal maps (`crates/cs-render/src/normal.rs` y `field.rs`) está portado de
[Sprite Fusion Sprite Normals](https://github.com/Hugo-Dz/spritefusion-sprite-normals),
© 2026 Hugo Duprez, licencia MIT (el aviso va en el encabezado de `normal.rs`).
