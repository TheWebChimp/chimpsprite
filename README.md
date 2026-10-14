# ChimpSprite

Editor de sprites y pixel art open source, escrito en Rust puro. Inspirado en Aseprite y en la línea de
la suite [ArtCraft](https://github.com/storytold). Reimplementación clean-room.

> Estado: **pre-alpha** — en desarrollo activo.

## Correr

```sh
cargo run -p chimpsprite
```

## Arquitectura

Workspace por capas: `cs-core` (modelo) → `cs-raster` → `cs-commands` (undo/redo) → `cs-render` →
`cs-tools` → `cs-ui` (egui) → `apps/chimpsprite`. Ver `docs/superpowers/specs/`.

## Licencia

Dual: [MIT](LICENSE-MIT) o [Apache-2.0](LICENSE-APACHE), a tu elección.
