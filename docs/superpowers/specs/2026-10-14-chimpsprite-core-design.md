# ChimpSprite — Core (subproyecto 1) — Diseño

Fecha: 2026-10-14
Estado: aprobado

## Visión

ChimpSprite es un editor de sprites y pixel art open source, escrito en Rust puro, inspirado en Aseprite
y en la línea de la suite ArtCraft (`github.com/storytold`). Es una reimplementación clean-room: no se
usa ni se consulta código de Aseprite; sí se implementa la especificación pública de su formato de
archivo.

Licencia: MIT OR Apache-2.0 (dual).

## Roadmap por subproyectos

Cada subproyecto tiene su propio ciclo spec → plan → implementación.

| #  | Subproyecto           | Contenido                                                              |
|----|-----------------------|------------------------------------------------------------------------|
| 1  | Core (este documento) | Motor, editor desktop, herramientas base, animación, `.aseprite` I/O   |
| 2  | Web / WASM            | Build de la app completa para navegador                                |
| 3  | IA / MCP              | Servidor MCP headless (`chimpsprite-cli mcp`), funciones asistidas     |
| 4  | Máscaras              | Máscaras de capa, selección avanzada (lazo, varita), máscaras guardadas |
| 5  | Game-dev pipeline     | Export a Godot/Unity/Bevy, hitboxes, normal maps, rigging 2D           |

El core se diseña para no bloquear 2–5: motor sin dependencias de UI, comandos serializables y crates
del motor compilables a `wasm32-unknown-unknown`.

## Alcance del Core (v1)

Incluido:

- Lienzo con zoom, paneo, grid de píxeles.
- Modos de color: RGBA e Indexado (hasta 256 colores, índice transparente).
- Paleta editable; conversión entre modos de color.
- Capas: imagen y grupo; visibilidad, bloqueo, opacidad, blend mode, orden.
- Frames con duración individual; timeline; reproducción; onion skin; tags de animación; cels vinculados.
- Herramientas: lápiz (tamaño, pixel-perfect), borrador, cubeta (contigua/global), línea, rectángulo,
  elipse (borde/relleno), selección rectangular + mover, cuentagotas.
- Undo/redo.
- Abrir y guardar `.aseprite` / `.ase`.
- Exportar PNG (frame), secuencia PNG, GIF animado, spritesheet (horizontal/vertical/grid) + JSON
  compatible con el formato de Aseprite.
- Importar PNG como sprite nuevo o como capa.

Fuera de alcance v1: modo Grayscale, tilemaps, slices, scripting, selección no rectangular, máscaras,
build web, MCP. (Tilemaps, slices y user data se preservan al guardar, ver IO.)

## Arquitectura

Workspace de Cargo por capas; cada crate solo depende de los de abajo.

```
chimpsprite/
├── crates/
│   ├── cs-core       # Modelo de documento puro
│   ├── cs-raster     # Algoritmos de píxel (geometría, fill, blend, pixel-perfect)
│   ├── cs-commands   # Command trait, transacciones, historial undo/redo
│   ├── cs-render     # Compositor CPU (capas, opacidad, blend, onion skin → RGBA)
│   ├── cs-io         # .aseprite R/W, PNG, GIF, spritesheet + JSON
│   ├── cs-tools      # Herramientas como máquinas de estado: input → Commands
│   └── cs-ui         # egui: lienzo, paneles, timeline, paleta
└── apps/
    └── chimpsprite   # Binario desktop (eframe + wgpu)
```

Reglas:

- `cs-core`, `cs-raster`, `cs-commands`, `cs-render`, `cs-io` y `cs-tools` no dependen de egui ni wgpu
  y compilan a `wasm32-unknown-unknown`.
- Toda mutación del documento pasa por un `Command`. UI y herramientas nunca mutan el documento directo.
- Los comandos son serializables (serde).
- Logging con el crate `log`; nada de `println!` fuera de tests.

Stack: Rust estable, `egui`/`eframe` con backend `wgpu`, `serde`, `thiserror`, `flate2`, `png`, `gif`,
`proptest` (dev).

## Modelo de datos (`cs-core`)

```rust
Sprite {
    width: u32, height: u32,
    color_mode: ColorMode,            // Rgba | Indexed { transparent_index: u8 }
    palette: Palette,                 // Vec<Rgba>; máx. 256 en Indexed
    layers: LayerTree,                // orden abajo→arriba, con grupos
    frames: Vec<Frame>,               // Frame { duration_ms: u16 }
    cels: HashMap<(LayerId, FrameIdx), Cel>,
    tags: Vec<Tag>,                   // Tag { name, from, to, direction, color }
    images: ImagePool,                // ImageId → Arc<Image>
    preserved_chunks: Vec<RawChunk>,  // datos .aseprite no interpretados
}
Layer   { id, name, kind: Image | Group { children }, visible, locked, opacity: u8, blend: BlendMode }
Cel     { position: (i32, i32), opacity: u8, content: CelContent }
CelContent = Image(ImageId) | Linked(FrameIdx)
Image   { width: u32, height: u32, pixels: PixelBuffer }
PixelBuffer = Rgba(Vec<[u8; 4]>) | Indexed(Vec<u8>)
```

Decisiones:

- Cada cel tiene su propio tamaño y offset (recortado al contenido), como en Aseprite.
- Imágenes en un pool con `Arc`; mutación con copy-on-write (`Arc::make_mut`). Cels vinculados
  comparten imagen.
- En modo Indexed los píxeles son índices `u8`; el render resuelve contra la paleta, así que editar la
  paleta recolorea al instante.
- `ConvertColorMode` mapea RGBA→Indexed por color más cercano (distancia euclidiana RGB) e
  Indexed→RGBA resolviendo la paleta.
- `BlendMode` v1: Normal, Multiply, Screen, Overlay, Darken, Lighten, Addition, Subtract, Difference.
  Modos de blend del archivo que no soportemos se conservan como valor y se renderizan como Normal.

## Comandos e historial (`cs-commands`)

```rust
trait Command: Serialize + DeserializeOwned {
    fn apply(&self, doc: &mut Sprite) -> Result<Undo, CmdError>;
}
```

- Comandos atómicos: `SetPixels`, `ReplaceCelImage`, `AddLayer`, `RemoveLayer`, `MoveLayer`,
  `SetLayerProps`, `AddFrame`, `RemoveFrame`, `MoveFrame`, `SetFrameDuration`, `LinkCel`, `UnlinkCel`,
  `SetPalette`, `ConvertColorMode`, `AddTag`, `RemoveTag`, `SetTagProps`, `ResizeCanvas`.
- `Transaction` agrupa comandos en un único paso de undo; si un comando falla, se revierten los ya
  aplicados de la transacción.
- `Undo` guarda snapshot mínimo del estado previo (referencias `Arc<Image>`, propiedades anteriores).
- Cada comando es atómico: si devuelve `CmdError` (capa bloqueada, índice inválido, etc.) el documento
  no cambia.
- Ejecutar un comando nuevo vacía la pila de redo.

## Herramientas (`cs-tools`)

- Interfaz: `on_press`, `on_drag`, `on_release(pos, modifiers, &EditorState) -> ToolOutput`.
- Durante el trazo la herramienta dibuja en un buffer de preview; al soltar emite una `Transaction`
  (un trazo = un paso de undo).
- Geometría en `cs-raster` como funciones puras: Bresenham, elipse de punto medio, rectángulo,
  flood fill (4-conexo), filtro pixel-perfect (elimina esquinas en L).
- Selección: máscara de bits en `EditorState` (no en el documento). Las herramientas de pintura
  recortan a la selección si existe. La herramienta mover desplaza el contenido seleccionado del cel
  actual.
- Color activo: en Indexed es un índice de paleta; en RGBA es un `Rgba`.

## UI y render (`cs-ui`, `cs-render`)

Layout: menú arriba, toolbar izquierda, lienzo al centro, paleta y selector de color a la derecha,
timeline abajo.

- `cs-render` compone el frame actual en CPU a un buffer RGBA (capas visibles, opacidad de capa y cel,
  blend, onion skin con tinte configurable para frames anteriores/siguientes).
- El buffer se sube como textura egui con filtro `Nearest`; encima se dibujan grid, marching ants de la
  selección y preview de la herramienta.
- Recomposición solo cuando el documento o la vista cambian (dirty flag).
- Reproducción respeta `duration_ms` de cada frame y la dirección del tag activo (forward, reverse,
  ping-pong).
- Timeline: widget custom; grilla capas × frames, indicador de cels vinculados, tags sobre la grilla,
  arrastrar para reordenar capas y frames.
- Atajos estilo Aseprite (B lápiz, E borrador, G cubeta, L línea, U rectángulo/elipse, M selección,
  V mover, I cuentagotas, Ctrl+Z/Ctrl+Y, espacio para paneo).
- Tema oscuro propio.

## IO (`cs-io`)

- `.aseprite` / `.ase` lectura y escritura según la especificación pública del formato: header, frames,
  chunks de capa (0x2004), cel (0x2005, incluyendo raw, linked y compressed), paleta (0x2019 y legacy
  0x0004), tags (0x2018), color profile. Compresión zlib vía `flate2`.
- Chunks no interpretados (user data, slices, tilesets, tilemap cels, extra cel) se guardan en
  `preserved_chunks` asociados a su posición y se reescriben al guardar.
- Export: PNG de un frame, secuencia PNG, GIF animado (con duraciones), spritesheet horizontal /
  vertical / grid con JSON en formato "hash" y "array" compatible con Aseprite (incluye `frameTags`).
- Import: PNG como sprite nuevo o como capa nueva en el sprite actual.
- Errores tipados con `thiserror`; un archivo corrupto produce un error descriptivo, nunca panic.

## Testing

- Unit tests en `cs-raster` (salida pixel exacta) y `cs-core`.
- `cs-commands`: property tests (`proptest`) — aplicar una secuencia aleatoria de comandos y deshacerla
  completa deja el documento igual al inicial; redo la reproduce.
- `cs-render`: golden tests (sprite de fixture → PNG esperado).
- `cs-io`: round-trip `.aseprite` (abrir → guardar → abrir = modelo idéntico) sobre un corpus de
  archivos de ejemplo con licencia libre creados para el proyecto.
- `cs-tools`: tests headless con eventos de mouse simulados → comandos emitidos.
- CI (GitHub Actions): `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, y
  `cargo check --target wasm32-unknown-unknown` para los crates del motor.

## Repositorio

Público en `github.com/TheWebChimp/chimpsprite`.
