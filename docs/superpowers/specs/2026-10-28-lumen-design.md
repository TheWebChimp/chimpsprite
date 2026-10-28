# Lumen: normales y luz para pixel art

Fecha: 2026-10-28 · Estado: aprobado ("hazlo y te lo mejoro terminando")

## Contexto

El usuario quiere una herramienta para hacer los mapas de normales de un sprite, basada en
[Sprite Fusion Sprite Normals](https://github.com/Hugo-Dz/spritefusion-sprite-normals) (Rust,
MIT, Hugo Duprez), con los controles de **Pixel Art Lighter** (imagen de referencia
`image_1973`): tres mapas (Albedo, Normal, Luminance) con Browse/Remove y
Generate/Modify/Export; una fila de vista (View, Light Color, Light Cursor, Follow Mouse,
BG Color, Detect BG, Hide BG); sliders con reset (Height, Falloff, Brightness, Ambient,
Luminance) y una vista previa grande con la luz.

Nombre: **Lumen**. Ícono del dock: un foco (`.design/lumen-icon.png`, 26×29 con el marco del
botón). Vive en el dock como Pixel Groom y ocupa la columna entera (`tall`). El layout lo
propongo yo con el estilo de Groom; el usuario lo corrige al final.

Hoy ya existe `cs_render::normal_map` (bisel por distancia al borde + algo de luminosidad,
Sobel) y Export tiene la casilla "Also export normal map (_n.png)", con constantes fijas.

## 1. Motor (cs-render, sin egui, compila en wasm)

### `normal.rs`: el algoritmo de Sprite Fusion
Se porta el algoritmo de Sprite Fusion (lib.rs, relief.rs, field.rs, distance.rs,
components.rs, color.rs, settings.rs) sobre `cs_core::Rgba`, sin la dependencia `image`. El
encabezado del archivo lleva el crédito y el aviso MIT de Hugo Duprez; el README lo menciona en
créditos.

- `NormalSettings` (mismos nombres, rangos y defaults): `alpha_threshold` 0.03,
  `shape_radius` 4, `shape_strength` 1.15, `shape_softness` 1.15, `detail_strength` 6,
  `detail_scale` 0.6, `normal_strength` 1.5, `format` OpenGL/DirectX, `invert_x`, `invert_y`.
  `clamped()` en lugar de `validate()` (la UI nunca da error, se ajusta al rango).
- Cada isla de 8-vecinos se procesa por separado (como Sprite Fusion), con relleno alrededor.
- Altura = forma (distancia euclidiana exacta al borde, `ln_1p`, gaussiana) + detalle
  (luminosidad Oklab centrada, gaussiana enmascarada fina y ancha).
- `normal_map(pixels, w, h, &NormalSettings) -> Vec<Rgba>`: mismas dimensiones; los
  transparentes quedan (128,128,255) con su alfa.
- Reemplaza al algoritmo actual. Export, el agente (MCP) y `Bitmap::normal_from_frame` lo usan
  con los ajustes por defecto, o con los de Lumen cuando se exporta desde la app (ver 4).

### `lighting.rs`: la luz
- `Light { x, y, height, falloff, color, brightness, ambient, luminance }` en píxeles del
  sprite (x, y pueden caer fuera de la imagen).
- `lit(albedo, normal, luminance: Option<&[u8]>, w, h, &Light, format) -> Vec<Rgba>`:
  - normal decodificada (si es DirectX, se voltea el verde);
  - dirección a la luz desde (x+0.5, y+0.5, 0) hasta (lx, ly, height), con la y de imagen
    hacia abajo convertida a la convención de la normal;
  - difusa = max(0, n·l) · atenuación · brightness, con
    atenuación = 1 / (1 + (d / falloff)²) (falloff es la distancia donde la luz baja a la mitad);
  - color = albedo · (ambient + difusa · color de luz) + albedo · máscara · luminance;
  - se satura a 255; el alfa es el del albedo.
- `luminance_mask(pixels, threshold) -> Vec<u8>`: 255 donde la luminosidad Oklab pasa el umbral
  (lo que "brilla solo": ojos, fuego), 0 en lo demás y en lo transparente.
- `detect_background(pixels, w, h) -> Option<Rgba>`: el color de las cuatro esquinas si al menos
  tres coinciden y es opaco (para imágenes importadas con fondo sólido).

## 2. Panel Lumen (cs-ui/src/lumen.rs)

`DockPanel::Lumen`, nombre "Lumen", ícono `icon.dock.lumen` (glifo de respaldo "☀"), `tall`.
Mismo ancho que Groom (434) y las mismas piezas: cajas de doble línea, líneas grabadas, notas
en small caps, `pixel_ui::CONTROL_HEIGHT`. Se mueven a un módulo común (`panel_kit.rs`) las
funciones de dibujo que Groom ya tiene y Lumen necesita (caja doble, hint, línea grabada,
child) para no duplicarlas.

### Arriba: logo y la imagen
Texto "Lumen" como logo (pieza `lumen.logo` en el skin cuando el usuario la dibuje; mientras,
texto) y a la derecha **Use Layer** / **Import…** como en Groom: cargan el **Albedo** (la capa
actual en el frame actual, o una imagen PNG/JPEG/GIF/WebP). Al cargar se genera solo el
normal (y la máscara de luminance queda vacía).

### Los tres mapas
Tres columnas iguales: **Albedo**, **Normal**, **Luminance**. Cada una:
- título con nota del tamaño (`32×32`) o "none";
- miniatura del mapa en caja doble;
- Albedo: **Browse** / **Remove**.
- Normal y Luminance: **Browse** / **Remove**, y **Generate** / **Modify** / **Export**.
  - Generate recalcula desde el albedo (Normal: con sus ajustes; Luminance: con su umbral).
  - Browse carga uno hecho a mano (debe medir lo mismo que el albedo; si no, aviso y no se
    carga). Un mapa cargado no se regenera solo.
  - Modify abre/cierra debajo de la fila de mapas los ajustes de ese generador; al moverlos se
    regenera (si el mapa venía de Browse, pasa a ser generado):
    - Normal: Shape, Shape Radius, Softness, Detail, Detail Scale, Strength (sliders), Format
      OpenGL/DirectX, Invert X, Invert Y.
    - Luminance: Threshold.
  - Export guarda el mapa como PNG (`<nombre>_n.png`, `<nombre>_e.png`).

### Vista
- **View**: Final, Albedo, Normal, Luminance, Light Only (la luz sobre gris).
- **Light Color** (botón con la muestra, abre el selector de color de egui).
- **Light Cursor**: Advanced (punto, aro del falloff y línea de la altura), Simple (punto),
  Hidden.
- **Follow Mouse**: la luz sigue al mouse sobre la vista previa; si no, se arrastra.
- **BG Color** + **Detect BG** + **Hide BG**: el color de fondo del albedo (Detect lo toma de las
  esquinas); con Hide BG ese color cuenta como transparente para generar y para la vista.

### Sliders (con ↺ que vuelve al default)
Height (px, 1–200, 24), Falloff (px, 1–400, 48), Brightness (0–10, 2), Ambient (0–100 %, 25),
Luminance (0–100 %, 100). Valor a la derecha del título, como en la referencia.

### Vista previa
Caja doble grande hasta las salidas, damero de fondo, zoom entero (rueda) y **Fit**; la luz se
mueve arrastrando (o siguiendo al mouse). Los mapas se calculan en caché: los sliders y la luz
solo vuelven a iluminar; Generate/Modify solo recalculan su mapa.

### Abajo
Línea grabada y **New Sprite** / **Add as Layer** / **Add as Artboard** con lo que muestra View
(Final hornea la luz; Normal da el mapa de normales, etc.), como en Groom.

## 3. Integración
- `lib.rs`: `lumen: lumen::LumenPanel`, `lumen_panel()` como `groom_panel()`, acciones
  Import/BrowseMap/UseLayer/Export/NewSprite/AddLayer/AddArtboard; nuevos
  `platform::OpenPurpose::LumenAlbedo`, `LumenNormal`, `LumenLuminance` y guardado de PNG.
- Demo: `CHIMPSPRITE_DEMO_LUMEN=<imagen>` abre Lumen con esa imagen (para screenshots).
- Skin: `icon.dock.lumen` con el foco del usuario (sin el marco del botón).

## 4. Export usa los ajustes de Lumen
`ExportDialog` recibe los `NormalSettings` de Lumen y los usa para `_n.png`; si Lumen nunca se
abrió, los defaults. El agente usa los defaults.

## Fuera de alcance (después)
- Varias luces, sombras proyectadas, specular.
- Procesar todos los frames de una animación dentro de Lumen (Export ya lo hace por frame).
- Pintar normales a mano.

## Pruebas
- Motor: islas separadas, transparencia neutra con su alfa, un cuadrado "abulta" hacia afuera
  (la normal del borde izquierdo apunta a la izquierda), DirectX voltea el verde, la distancia
  exacta contra fuerza bruta (de Sprite Fusion); luz: un píxel que mira a la luz brilla más que
  uno que no, ambient 100 % = albedo, luminance enciende la máscara sin luz, falloff atenúa.
- UI: la caché (mover la luz no regenera el normal), Browse de tamaño distinto se rechaza,
  Detect BG; screenshot con `CHIMPSPRITE_DEMO_LUMEN`.
