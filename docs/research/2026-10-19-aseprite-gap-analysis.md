# ChimpSprite vs Aseprite 1.3.x: análisis de gaps

Fuentes: `aseprite-features.md` y `chimpsprite-features.md` (rama `symbols`, HEAD 9eb9a83). Estados: **missing** = no existe; **partial: …** = existe incompleto; **MCP-only** = existe en cs-agent/MCP pero no en la GUI. Tamaño: S (< 1 día), M (unos días), L (1+ semanas).

---

## 0. Tiles (prioridad del usuario, va primero)

### 0.1 Qué ofrece Aseprite (referencia para el diseño)

**Modelo**
- Layer tipo `Tilemap` (layer type 2 en .aseprite) que referencia un `Tileset` por índice; varios layers pueden compartir el mismo tileset.
- Tileset: `name`, tamaño de tile (= grid; w/h), `base index` (número que ve el usuario para el primer tile, por defecto 1), user data (texto + color + properties), user data por tile (1.3), opcional archivo externo.
- Tile 0 = tile vacío (siempre transparente); los tiles se guardan como imágenes del mismo color mode que el sprite.
- Valor de tile = u32: bits 0..28 = tile index (`0x1fffffff`), bit 31 = flip X, bit 30 = flip Y, bit 29 = flip D (diagonal/transposición; X+Y+D da las 8 orientaciones).
- `Allowed flips` del tileset (X/Y/D): en modo Auto permite reconocer un tile como versión volteada de otro existente en vez de crear uno nuevo.

**Edición**
- Toggle Pixels/Tiles mode [ToggleTilesMode] {Space+Tab}: en Tiles mode las herramientas operan sobre tiles (pencil coloca el tile FG, eyedropper toma un tile, bucket rellena tiles, marquee/selección encaja en la grid de tiles, Move mueve tiles).
- Tileset modes en Pixels mode [TilesetMode] {Space+1/2/3}:
  - **Manual**: editar píxeles modifica el tile existente (cambia en todas sus instancias); nunca crea tiles nuevos.
  - **Auto**: al pintar, se crean tiles nuevos para el contenido modificado, se reutilizan iguales (incl. flips permitidos) y se borran los tiles que quedan sin uso.
  - **Stack**: cada edición añade tiles nuevos al final; no reutiliza ni borra.
- Panel de tileset en la color bar (sustituye a la paleta en Tiles mode): elegir tile FG/BG, drag & drop para reordenar (remapea los tilemaps), copiar/mover tiles [CopyTiles]/[MoveTiles], borrar/limpiar tiles, "Show only tiles", tamaño de entrada, select used/unused tiles [SelectPaletteColors].
- Mouse wheel / shortcuts para FG/BG tile; [ ] cambian tile.
- Select Tile [SelectTile] (+add/subtract/intersect), doble-click selecciona un tile de la grid; Selection as Grid [SelectionAsGrid].
- Mostrar números de tile [ShowTileNumbers]; tile preview en el cursor (pref).
- New Tilemap Layer [NewLayer tilemap ask] {Space+N} con diálogo de tileset (name, grid w/h, base index, allowed flips); Convert to Tilemap / to Layer [ConvertLayer] (corta la imagen en tiles, dedup).
- Sprite Properties: lista de tilesets (duplicar [TilesetDuplicate], borrar [TilesetDelete], alerta de tileset sin uso).
- Export Tileset [ExportTileset] / CLI `--export-tileset` (sheet de tiles + JSON).
- Lua: `Tileset`, `Tile`, `app.pixelColor.tileI/F`, `layer.tileset`, `cel.image` de tipo tilemap.

**Formato .aseprite**
- Layer chunk 0x2004 con `layer type = 2` y campo extra `tileset index` (u32).
- Tileset chunk 0x2023: id, flags (1 = archivo externo, 2 = tiles dentro del archivo, 4 = tile 0 es el vacío, 8/16/32 = match X/Y/D flip), nº de tiles, tile w/h, base index, nombre; si flag 2: imagen comprimida zlib (tira vertical tile_w × tile_h·N); si flag 1: id de archivo externo (chunk 0x2008 External Files) + id de tileset.
- User data chunk 0x2020 tras el tileset (tileset) y luego uno por tile.
- Cel chunk 0x2005 `cel type = 3` (compressed tilemap): ancho/alto en tiles, bits por tile (32), máscara de tile ID, máscaras flip X/Y/D, 10 bytes reservados, datos zlib (u32 LE por tile).

### 0.2 Gaps de ChimpSprite

| Gap | Estado | Tam. |
|---|---|---|
| Modelo `Tileset`/`Tile`/`TilemapLayer` en cs-core (tile u32 con flags, tile 0 vacío, base index, tilesets compartidos, user data por tile) | missing | M |
| Leer .aseprite: tileset chunk 0x2023 (interno y externo), layer type 2, cel type 3 | missing (hoy: layers vacíos + cels descartados, aseprite.rs:374/:473) | M |
| Escribir .aseprite: lo anterior + user data de tileset/tile; round-trip sin pérdida | missing | M |
| `.chimp`: persistir tilesets (vía inner .aseprite queda resuelto) | missing | S |
| Render de tilemaps (compositor, flips X/Y/D), thumbnails, onion | missing | M |
| New Tilemap Layer + diálogo de tileset; Convert to Tilemap/Layer | missing | M |
| Pixels/Tiles mode toggle; herramientas en Tiles mode (pencil, eraser, bucket, eyedropper, selección por tile, move) | missing | L |
| Tileset modes Manual / Auto / Stack (dedup con allowed flips, limpieza de tiles sin uso) | missing | L |
| Panel de tileset (FG/BG tile, reordenar con remap, copy/move/delete tiles, used/unused) | missing | M |
| Tile flips en GUI (botones/atajos para el tile FG) | missing | S |
| Show Tile Numbers, tile preview en cursor | missing | S |
| Sprite Properties: lista de tilesets (duplicar/borrar) | missing | S |
| Export Tileset (PNG + JSON) | missing | S |
| **Export a Tiled**: `.tsx` (imagen del tileset, tilewidth/height, columns, spacing/margin) | missing | M |
| Export a Tiled: `.tmx` y `.tmj` (un `<layer>` por tilemap layer, CSV/base64+zlib, GID con bits de flip de Tiled 31/30/29 = H/V/D; ojo: el orden difiere del de Aseprite) | missing | M |
| Export a Tiled: colisiones desde slices (`<objectgroup>` en el tile para slices dentro de un tile; object layer en el mapa para slices del sprite), properties desde user data | missing | M |
| Export a Tiled: tile animations (`<animation><frame tileid duration>`) derivadas de frames/tags del tilemap o de artboards-tile | missing | M |
| Export a motores de tilemaps (Godot TileSet `.tres`, Unity Tilemap/Rule Tile, Bevy ecs_tilemap/LDtk) | missing (nice-to-have tras Tiled) | M |
| MCP: create_tilemap_layer, set_tiles, get_tile_at, draw_on_tile, tileset ops | missing | M |
| **Tiled mode view** [TiledMode] none/X/Y/both: render repetido alrededor del sprite + pintar con wrap-around (stroke, bucket, shapes, selección envuelven) | missing | M |

---

## 1. Resto de gaps por prioridad

### P1: edición central y round-trips

| # | Gap | Aseprite cmd | Estado ChimpSprite | Tam. | Motivo |
|---|---|---|---|---|---|
| 1 | Copy / Cut / Paste (+ clipboard del SO, paste como selección flotante) | Copy, Cut, Paste | missing (no hay clipboard en ningún sitio) | M | Operación más básica; sin ella no se puede trabajar |
| 2 | Copy Merged | CopyMerged | missing | S | Viene casi gratis con #1 |
| 3 | Paste special: new layer / new sprite | NewLayer fromClipboard, NewFile fromClipboard | missing | S | Flujo diario de referencia/importación |
| 4 | Transform de selección: scale, rotate (Fast/RotSprite), skew, pivot, Enter/Esc | MaskContent | missing | L | Imprescindible para ajustar pose/forma |
| 5 | Flip H/V y Rotate 90/180 de selección/cel/canvas en GUI | Flip, Rotate | MCP-only | S | Ya existe la lógica; solo falta menú/atajos |
| 6 | Sprite Size (px/%, lock ratio, nearest/bilinear/RotSprite) | SpriteSize | missing | M | Muy común; también rompe paridad CLI/escala |
| 7 | Crop Sprite to selection / Trim Sprite | CropSprite, AutocropSprite | partial: trim solo como checkbox en Canvas Size | S | Uso constante al preparar assets |
| 8 | New Sprite dialog (tamaño, color mode, background, pixel ratio) | NewFile | partial: 64×64 RGBA fijo | S | Primera interacción de todo usuario |
| 9 | Ink modes: Alpha Compositing, Simple, Copy Alpha+Color, Lock Alpha, Shading | SetInkType | missing | M | Lock Alpha y Shading son núcleo del pixel art |
| 10 | Opacidad de tool/ink | (ink opacity) | missing | S | Necesario para Alpha Compositing |
| 11 | Brush types: circle, square, line + angle; flip/rotate brush | ChangeBrush | partial: solo cuadrado 1..32 | M | Pincel redondo es el default de Aseprite |
| 12 | Custom brush desde selección + brush slots | NewBrush, ChangeBrush slot | missing | M | Muy usado para tiles/patrones; depende de #1 |
| 13 | Atajos tamaño de brush `[ ]` / `+ -` | ChangeBrush increment-size | missing | S | Memoria muscular de todo artista |
| 14 | Pixel-perfect para tamaños > 1 y algoritmo freehand "dots" | PixelPerfectMode | partial: solo size 1 | S | Expectativa de Aseprite |
| 15 | Paint Bucket: tolerance, refer all layers, 8-connectivity, stop at grid | (bucket options) | partial: solo contiguous 4-conn | S | Relleno de zonas con antialias/sombras |
| 16 | Eyedropper: sample current layer / all layers / ref layer | (eyedropper options) | partial: solo compuesto | S | Necesario con varias capas |
| 17 | Color mode conversion en GUI (RGB / Indexed / Grayscale, con dithering) | ChangePixelFormat | MCP-only (RGBA↔Indexed); grayscale missing | M | Flujo indexado de retro/game dev |
| 18 | Grayscale color mode (leer/escribir sin convertir) | — | missing (se convierte a RGBA al abrir) | M | Rompe round-trip .aseprite |
| 19 | Duplicate layer, Merge Down, Flatten / Flatten visible | DuplicateLayer, MergeDownLayer, FlattenLayers | missing | S | Operaciones de capa diarias |
| 20 | New layer via copy / via cut | NewLayer viaCopy/viaCut | missing | S | Depende de #1; muy usado |
| 21 | Layer Properties dialog (name, blend, opacity, user data) | LayerProperties | missing (props sueltas en timeline) | S | Punto central de edición de layer |
| 22 | Blend modes restantes (Color Burn/Dodge, Soft/Hard Light, Exclusion, Divide, Hue, Saturation, Color, Luminosity) | — | partial: 9 de 19, resto render Normal | M | Archivos .aseprite se ven mal |
| 23 | Background layer (convertir to/from background) | BackgroundFromLayer, LayerFromBackground | missing | S | Round-trip y semántica de opaco |
| 24 | Reference layers | NewLayer reference | missing (layer flag no leído) | M | Muy usado para calcar; round-trip |
| 25 | Cel Properties (opacity, z-index, user data) + opacidad de cel en GUI | CelProperties | partial: opacity sin UI; z-index descartado | S | Round-trip y animación |
| 26 | Cel z-index leído/escrito/renderizado | — | missing (se descarta, aseprite.rs:431) | S | Rompe round-trip |
| 27 | User data (texto/color/properties) en sprite, layer, cel, tag, frame; editor | (user_data) | partial: solo tag color y slice | M | Rompe round-trip; metadatos para motores |
| 28 | Preservar chunks desconocidos (llenar `extra_chunks` al leer) y layer UUID, pixel ratio, color profile | — | partial: se emite pero el lector nunca lo llena | S | Garantiza round-trip sin pérdida |
| 29 | Duplicar / copiar / mover cels entre frames y layers (drag con Ctrl) | CopyCel, MoveCel | missing | M | Animación diaria |
| 30 | New empty frame, duplicate linked cels, Reverse frames | NewFrame content=empty, ReverseFrames | partial: insert empty en menú; reverse missing | S | Animación diaria |
| 31 | Frame Properties / Constant frame rate | FrameProperties | partial: duración en timeline, sin "all frames" | S | Ajuste de timing |
| 32 | Tag repeat en UI y respetado en playback | FrameTagProperties | partial: dato existe, sin UI | S | Round-trip visible + preview fiel |
| 33 | Onion skin: opacidad, tint vs merge, loop tag, layer actual, behind/front en UI | ShowOnionSkin | partial: modelo sin UI | S | Usado constantemente al animar |
| 34 | Home/End primer/último frame; first/last en tag | GotoFirstFrame… | partial: solo botones | S | Atajos esperados |
| 35 | Selection: Modify Border/Expand/Contract | ModifySelection | missing | S | Habitual para outlines/sombras |
| 36 | Fill / Stroke selection con FG | Fill, Stroke | missing | S | Atajos F/S muy usados |
| 37 | Mover selección/contenido con flechas (px y por tile) | MoveMask | missing | S | Ajuste fino |
| 38 | Shapes: Contour, Polygon, Curve, rounded rectangle | (tools D, Shift+D, Shift+L) | missing | M | Herramientas de la toolbar estándar |
| 39 | Hand / Zoom tools y zoom por teclas 1..6 | (tools H, Z), Zoom | partial: pan con Space/medio | S | Paridad de toolbar/atajos |
| 40 | Pen pressure (size/opacity) + stabilizer | ShowDynamics | missing | M | Usuarios con tableta |
| 41 | Right-click configurable y quicktool Ctrl=Move | (prefs) | partial | S | Hábito de Aseprite |
| 42 | Guardar .aseprite multi-artboard (o "export artboard as .aseprite") | — | partial: rechazado | S | Interop con pipelines Aseprite |

### P2: importante, menos frecuente

| # | Gap | Aseprite cmd | Estado | Tam. | Motivo |
|---|---|---|---|---|---|
| 1 | Replace Color dialog (tolerance) | ReplaceColor | MCP-only (recolor) | S | Recoloreo de variantes |
| 2 | Invert Color | InvertColor | missing | S | Filtro básico |
| 3 | Brightness/Contrast, Hue/Saturation (HSL/HSV), Color Curve | BrightnessContrast, HueSaturation, ColorCurve | missing | M | Ajustes de color en bloque |
| 4 | Outline FX (inside/outside, matriz) | Outline | missing | S | Muy usado en sprites de juego |
| 5 | Convolution Matrix + stock matrices, Despeckle | ConvolutionMatrix, Despeckle | missing | M | Menos frecuente en pixel art |
| 6 | Framework de filtros: preview, target selected/all cels | — | missing | M | Base común de #1–#5 |
| 7 | Shift (wrap) content | MoveMask wrap | MCP-only | S | Útil para tiles seamless |
| 8 | Palette editor (editar, borrar, reordenar, resize, remap al mover) | PaletteEditor, PaletteSize | missing | M | Trabajo con paleta limitada |
| 9 | Load/Save palette (GPL, PAL, HEX, ACT, ASE, PNG) + presets bundled | LoadPalette, SavePalette, ShowPalettePresets | missing | M | Paletas como Lospec/PICO-8 |
| 10 | Palette from sprite (quantize) en GUI | ColorQuantization | MCP-only | S | Ya existe la lógica |
| 11 | Sort & gradients de paleta, select used/unused colors | ShowPaletteSortOptions, SelectPaletteColors | missing | S | Organización de paleta |
| 12 | Color selectors extra (Tint/Shade/Tone, wheels, harmonies) | SetColorSelector | partial: egui HSV + hex | M | Comodidad |
| 13 | FG/BG index +/- `[ ] 9 0` | ChangeColor | missing | S | Atajo indexado |
| 14 | Color Range (select by color) | MaskByColor | missing | S | Selección frecuente en recolor |
| 15 | Magic wand: 8-connectivity, stop at grid | — | partial | S | Paridad |
| 16 | Load/Save selection (.msk) | LoadMask, SaveMask | missing | S | Poco uso |
| 17 | Sheet export: trim sprite/cels, border/shape/inner padding, extrude | ExportSpriteSheet | missing | M | Atlas de motor necesitan padding/extrude |
| 18 | Sheet export: Packed (MaxRects), By Rows/By Columns, constraints, power-of-two | — | partial: H/V/Grid | M | Atlas compactos |
| 19 | Sheet export: split layers/tags/slices, selected layers, merge duplicates, ignore empty | — | missing | M | Pipelines por capa/tag |
| 20 | Sheet export: filename format, ruta JSON separada, preview en diálogo | — | missing | S | Integración con motores |
| 21 | Import Sprite Sheet | ImportSpriteSheet | missing | M | Importar assets ajenos |
| 22 | Export dialog de imagen/animación: resize %, area, layers, frames/tag, direction | SaveFileCopyAs | partial: scale entero, rango en GIF | S | "Save Copy As" |
| 23 | GIF: dithering, loop count, dirección de tag | — | partial | S | Calidad de export |
| 24 | Repeat last export | RepeatLastExport | partial: opciones recordadas, sin atajo | S | Iteración rápida |
| 25 | Formatos: GIF/JPG/BMP/TGA/WebP/APNG/ICO/QOI open+save, PNG/GIF sequence open | — | missing (PNG/GIF export sí) | M | Interop general |
| 26 | Desktop autosave + crash recovery | (data recovery) | missing (solo web) | M | Pérdida de trabajo |
| 27 | Undo History panel | UndoHistory | missing | M | Navegación de historial |
| 28 | Preferences dialog (UI scale, checker, grid default, cursor, undo limit, autosave) | Options | missing | M | Configurabilidad básica |
| 29 | Keyboard Shortcuts editor (import/export) | KeyboardShortcuts | missing | M | Usuarios migrando de Aseprite |
| 30 | Preview window / fullscreen preview | TogglePreview, FullscreenPreview | missing | M | Ver animación a 1x mientras se edita |
| 31 | Playback speed, play once, loop section | SetPlaybackSpeed, TogglePlayOnce, SetLoopSection | missing | S | Revisión de animación |
| 32 | Timeline thumbnails y zoom | ToggleTimelineThumbnails | missing | M | Navegación visual |
| 33 | Solo layer, toggle other layers opacity, opacity presets Shift+1..4 | SoloLayer, ToggleOtherLayersOpacity, LayerOpacity | missing | S | Foco en una capa |
| 34 | Selección multi-layer en timeline y operaciones en bloque | — | partial: solo rango de frames | M | Operar varias capas |
| 35 | Slices: UI de 9-slice center, slice from selection, duplicate | SliceProperties, DuplicateSlice | partial | S | Dato ya existe |
| 36 | Pixel aspect ratio (view + export) | SpriteProperties | missing | S | Plataformas retro |
| 37 | Sprite Properties dialog | SpriteProperties | missing | S | Centraliza metadatos |
| 38 | Duplicate Sprite / New Sprite from Selection | DuplicateSprite, NewSpriteFromSelection | missing (artboards cubren parte) | S | Variantes |
| 39 | Grid: color/opacity, Selection as Grid, Trim by grid | GridSettings, SelectionAsGrid | partial | S | Trabajo en grid |
| 40 | Checkerboard: tamaño y colores | — | missing | S | Visibilidad sobre fondos |
| 41 | Symmetry aplicada a Text; toggle por tool | SymmetryMode | partial | S | Consistencia |
| 42 | Blur tool | (tool R Blur) | missing (Jumble sí existe) | S | Toolbar estándar |
| 43 | Mouse wheel customizable | (prefs) | missing | S | Hábitos |
| 44 | CLI batch (`-b`, `--save-as`, `--sheet`, `--scale`, `--split-*`, `--trim`) | — | missing | M | CI de assets en game dev |
| 45 | Color profiles (asignar/convertir) | — | missing (solo escribe sRGB) | M | Fidelidad color; bajo impacto en pixel art |

### P3: nice to have / nicho

| # | Gap | Aseprite cmd | Estado | Tam. | Motivo |
|---|---|---|---|---|---|
| 1 | Lua scripting, debugger, script dialogs, eventos | RunScript, Debugger | missing (MCP es la historia de automatización) | L | MCP cubre automatización; Lua solo por compatibilidad de scripts |
| 2 | Run Command palette (búsqueda de comandos, "=" math) | RunCommand | missing | S | Productividad; barato con un registro de comandos |
| 3 | Extensiones (.aseprite-extension: palettes, themes, keys, dithering) | — | missing | L | Ecosistema; empezar por paletas/keymaps |
| 4 | Temas (light/dark, custom) y UI pixel-art | — | partial: un tema oscuro | M | Ya en roadmap diferido |
| 5 | Idiomas / i18n (es/en) | — | missing | M | Usuario hispanohablante |
| 6 | Docking/workspace layouts, duplicate view | ToggleWorkspaceLayout, DuplicateView | missing | L | egui_dock posible |
| 7 | Advanced mode (ocultar UI) | AdvancedMode | missing | S | Nicho |
| 8 | SVG/CSS export | — | missing | S | Nicho |
| 9 | FLC/FLI, PCX | — | missing | S | Legado |
| 10 | Screenshot, open with app, open in folder, copy path | Screenshot, OpenInFolder, CopyPath | missing | S | Comodidad desktop |
| 11 | Reopen closed file, close all | ReopenClosedFile, CloseAllFiles | missing | S | Comodidad |
| 12 | Auto guides, layer edges | ShowAutoGuides, ShowLayerEdges | missing | S | Visualización |
| 13 | Dynamics por velocidad, gradient por presión | ShowDynamics | missing | M | Tras #40 de P1 |
| 14 | Windows Ink / Wintab selector | — | missing | S | Depende de winit |
| 15 | Custom dithering matrices, más algoritmos de quantize (Octree, CIELAB) | — | missing | M | Calidad de conversión |
| 16 | Insert Text con fuente bundled aseprite_font, bold/italic/hinting | PasteText | partial | S | Ya hay Text tool |

---

## 2. Ya más allá de Aseprite

- **Artboards**: varios sprites independientes en un canvas infinito (tamaño, paleta, color mode propios) + `.chimp` multi-artboard + atlas export.
- **Symbols / instance layers**: layer que muestra otro artboard con timeline propio (Graphic Loop/Once/Single, Movie clip), keys con posición/flip/opacidad, detección de ciclos.
- **Layer masks** no destructivas (reveal/hide/from selection, edit, apply) y **clipping masks**.
- **Rulers y guides** arrastrables con snap/lock (Aseprite no tiene rulers).
- **Symmetry** con diagonales y All-8, ejes movibles a medio píxel.
- **Normal maps** generados en export (`_n.png`).
- **Exports a motores**: Godot 4 SpriteFrames `.tres`, Unity `.png.meta` (pivots, GUIDs estables), Bevy `.ron`.
- **Web build** (WebGPU/WebGL2) con autosave en localStorage y restore de sesión.
- **Servidor MCP** (52 tools undoables, `render` como imagen, `read_pixels`), automatización por agentes IA.
- Text tool con TTF/OTF y antialias; Gradient con Bayer dither; Jumble.

---

## 3. Milestones sugeridos (P1, después de Tiles)

1. **Clipboard + Transform** (P1 #1–5, #12, #20, #36–37): copy/cut/paste/copy merged, paste special, selección flotante con scale/rotate/skew, flip/rotate en GUI, fill/stroke, nudge con flechas, custom brush desde selección.
2. **Brushes & inks** (P1 #9–11, #13–16, #38–41): ink modes con opacidad, brush circle/line/angle, atajos de tamaño, pixel-perfect general, opciones de bucket/eyedropper, Contour/Polygon/Curve/rounded rect, Hand/Zoom, presión.
3. **Sprite & layer ops** (P1 #6–8, #17, #19, #21, #23–24, #42): New Sprite dialog, Sprite Size, Crop/Trim, color mode en GUI, duplicate/merge/flatten, Layer Properties, background y reference layers, export de artboard como .aseprite.
4. **Fidelidad .aseprite** (P1 #18, #22, #25–28): grayscale, 19 blend modes, cel z-index/opacity UI, user data completo con editor, preservar chunks/UUID/pixel ratio/profile. Natural hacerlo junto o justo después del I/O de tilemaps.
5. **Animación** (P1 #29–34): copiar/mover cels por drag, reverse/linked/empty frames, Frame Properties, tag repeat, controles de onion skin, Home/End.
