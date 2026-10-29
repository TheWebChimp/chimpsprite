# UI en pixel art: base y primera pantalla

Fecha: 2026-10-23 · Estado: aprobado

## Contexto

El usuario quiere que la interfaz de ChimpSprite sea pixel art, como la de Aseprite. Va a ir
**poco a poco, pantalla por pantalla**, y se afina sobre la marcha.

Material del usuario en `.design/` (sin versionar):

- **`Design.aseprite`, mockups:**
  - Frame 0: el diálogo "Unsaved changes" en pixel art al lado del actual, un botón, ventanas con
    barra rosa (≡ y ○), una pestaña con × y los slots de herramientas.
  - Frame 1: la app completa.
- **`skin.aseprite`:** plantilla de piezas, cada una un slice con nombre:
  - Ya dibujados: los colores `color.*`, `window` y `window.active` (barra de título de 13 px),
    `window.menu*` y `window.close*`.
  - Con slice pero sin dibujar: `button*`, `tab*`, `panel`, `popup`, `menubar`, `menu.item.hover`,
    `tool.slot*` e `icon.*`.
  - Reglas en `.design/LEEME.md`:
    - las piezas que se estiran usan 9-slice;
    - cada color sale del píxel central de su slice;
    - si una variante (`.hover`, `.down`, `.active`) falta, se usa la base.
- **`chimpsprite.ttf`:** fuente pixel del usuario (autor "Chimp"). Su grilla es de 16 píxeles por
  em; las mayúsculas miden 7 px y las colas bajan 2. Incluye acentos, ñ, ¿, € y °.
- **`picotron-v7`:** su licencia es no comercial y prohíbe redistribuir y modificar. **No entra al
  repo.**

Esta primera entrega arma la base que comparten todas las pantallas y migra la primera: el
diálogo "Unsaved changes".

## Objetivo

- Interruptor **View → Pixel UI**:
  - Encendido: fuente pixel, escala entera y las pantallas ya migradas dibujadas con el skin.
  - Apagado: todo igual que hoy.
- Lectura del skin desde `skin.aseprite`, con recarga en vivo en escritorio.
- Piezas reutilizables `pixel_dialog` y `pixel_button`.
- Diálogo "Unsaved changes" con el skin.

Fuera de alcance: las demás pantallas (Home, marco de la app, paneles, menús, pestañas, toolbar,
íconos), el editor de fuentes y la mascota. Cada una tendrá su propia entrega.

## 1. Archivos

- `assets/ui/skin.aseprite` y `assets/ui/chimpsprite.ttf`, versionados. Se compilan dentro del
  binario con `include_bytes!`.
- `assets/ui/README.md` explica dos cosas:
  - el origen de la fuente: del autor del proyecto, con la misma licencia del repo;
  - las reglas del skin (resumen de `.design/LEEME.md`).
- Los botones del mockup se copian de `Design.aseprite` a los slices `button`, `button.hover` y
  `button.down` de `skin.aseprite`. El usuario lo autorizó.
- `.design/` sigue sin versionar.

## 2. Skin (`crates/cs-ui/src/skin/`)

```rust
pub struct Skin { pieces: HashMap<String, Piece>, colors: SkinColors }
pub struct Piece { texture: egui::TextureHandle, size: [u32; 2], center: Option<Rect9> }
pub struct SkinColors { desktop, text, text_dim, accent, panel, outline, selection, scroll: Color32 }

impl Skin {
    pub fn load(ctx: &egui::Context, bytes: &[u8]) -> Result<Skin, SkinError>;
    pub fn piece(&self, name: &str, state: WidgetState) -> Option<&Piece>;   // con la caída a la base
    pub fn colors(&self) -> &SkinColors;
}
```

- **Lectura:** con `cs_io::aseprite::read`, igual que se abre un `.aseprite`.
  - Se compone el frame 0 con las capas visibles.
  - Cada slice da un recorte; su `center`, si existe, es el 9-slice.
  - Los slices `color.*` dan el color del píxel central.
- **Texturas:** con `TextureOptions::NEAREST`, sin suavizado.
- **Piezas sin dibujar:** una pieza totalmente transparente cuenta como que no existe.
  - Quien la usa dibuja un respaldo plano con los colores del skin: fondo `panel`, borde
    `outline`, 1 px y sin redondeo.
  - Así todo funciona aunque falten piezas.
- **Colores que faltan:** usan los del tema actual (`theme.rs`).
- **Geometría 9-slice**, como función pura y sin egui:
  - `nine_slice(dest: Rect, size: [u32;2], center: Rect9, scale: u32) -> [(src, dest); 9]`.
  - Las esquinas no se deforman.
  - Los bordes se estiran a lo largo y el centro en las dos direcciones.
  - Todo se redondea a píxeles enteros de pantalla.
- **Recarga en vivo:** solo en escritorio, si existe la variable de entorno `CHIMPSPRITE_SKIN`.
  - Se lee el archivo de esa ruta y se vuelve a leer cuando cambia su fecha de modificación (se
    revisa como mucho una vez por segundo).
  - Si el archivo está roto, se queda el skin anterior y se avisa una vez en el log.

## 3. Interruptor y apariencia

- `ViewSettings` gana `pixel_ui: bool` (por defecto `false`) y `ui_scale: u8` (1, 2 o 3; por
  defecto 2). Se guardan como los demás ajustes de View.
- Menú **View → Pixel UI** (casilla) y **View → UI Scale ▸ 1× / 2× / 3×**, activo solo con
  Pixel UI.
- Encendido:
  - `pixels_per_point = ui_scale` (entero).
  - La fuente `chimpsprite` a 16 puntos es la proporcional por defecto. La monoespaciada queda
    igual.
  - Sin redondeo en ventanas, botones ni menús.
  - El color de texto sale del skin.
- Apagado: vuelve al tema actual y a la escala del sistema.
- El cambio se aplica en caliente, sin reiniciar.
- **El lienzo no cambia:** el zoom del sprite no depende de la escala de la UI.

## 4. Piezas reutilizables (`crates/cs-ui/src/pixel_ui.rs`)

- `pixel_dialog(ctx, id, title, |ui| …)`: ventana fija en el centro, igual que los diálogos
  actuales.
  - El marco es `window.active` en 9-slice.
  - El título va centrado en la barra de 13 px, con `color.text`.
  - El contenido va en el área del centro del 9-slice, con un margen de 4 px.
- `pixel_button(ui, label) -> Response`:
  - Usa `button` en reposo, `button.hover` con el puntero encima y `button.down` mientras se
    presiona.
  - El tamaño se ajusta al texto con un margen de 4 px a cada lado, sin bajar del tamaño mínimo
    de la pieza.
  - Devuelve un `Response` normal (clic, foco y teclado).
- Las dos caen al respaldo plano si falta la pieza.

## 5. Primera pantalla: "Unsaved changes"

En `file_actions.rs` (`unsaved_dialog`):

- **Con Pixel UI encendido:**
  - Se dibuja con `pixel_dialog`.
  - Muestra el mismo texto y la misma lista de documentos.
  - Botones con `pixel_button`: Save / Discard / Cancel, o Save all / Discard all / Cancel al
    salir.
- **Con Pixel UI apagado:** queda como hoy.
- La lógica (qué hace cada botón, Enter y Esc) es la misma en los dos modos.

## 6. Tests

- **Skin real:** se carga el skin embebido.
  - Están todos los colores del mockup.
  - `window.active` tiene centro.
  - `button` existe después de copiarlo del mockup.
- **Respaldo:** una pieza transparente cuenta como faltante, y una variante que falta cae a la
  base.
- **9-slice:**
  - las esquinas conservan su tamaño ×scale;
  - con el destino del mismo tamaño que la pieza, cada recorte cae en su lugar;
  - con un destino más chico que las esquinas, nada se sale del rectángulo.
- **Diálogo (harness del lienzo con eventos crudos):**
  - Con Pixel UI encendido, el diálogo muestra los tres botones.
  - Clic en Cancel lo cierra sin cambios.
  - Clic en Discard cierra la pestaña.
- **Ajustes:** `ViewSettings` se guarda y se lee de vuelta con `pixel_ui` y `ui_scale`.
- **Prueba manual:** se enciende Pixel UI y se cierra una pestaña con cambios. El diálogo debe
  verse como el mockup.
