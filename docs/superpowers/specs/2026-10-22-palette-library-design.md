# Biblioteca de paletas

Fecha: 2026-10-22 · Estado: aprobado

## Contexto

El usuario quiere bajar paletas de Lospec y usarlas en ChimpSprite. Se divide en dos partes:

1. **Biblioteca de paletas, como la de Aseprite (este documento).**
2. Buscador de Lospec: busca en `lospec.com`, descarga y guarda en la biblioteca. Spec aparte.
   En escritorio tendrá buscador completo. En web, solo pegar el link o el nombre de una paleta,
   porque el listado de Lospec no acepta peticiones desde otros sitios y el JSON de cada paleta sí.

Estado de partida:
- el panel de paleta solo muestra los colores y elige el primario y el secundario;
- no hay lectores ni escritores de archivos de paleta;
- los sprites nuevos siempre usan DB32 (`Palette::db32()` en `Sprite::new`).

## Objetivo

Como en Aseprite:
- presets incluidos y una carpeta de presets del usuario;
- cargar y guardar paletas en los formatos comunes;
- elegir la paleta por defecto de los sprites nuevos.

Fuera de alcance: los formatos `.act` y `.col`, editar colores de la paleta (agregar, quitar,
reordenar) y el buscador de Lospec.

## 1. Formatos (`cs-io/src/palette_files.rs`)

Funciones puras, sin acceso a disco:

```rust
pub enum PaletteFormat { Gpl, Hex, Pal, Png, Aseprite }
pub fn format_for(file_name: &str) -> Option<PaletteFormat>;            // por extensión
pub fn read_palette(format: PaletteFormat, data: &[u8]) -> Result<NamedPalette, PaletteError>;
pub fn write_palette(format: PaletteFormat, palette: &NamedPalette) -> Vec<u8>; // Gpl, Hex o Pal
pub struct NamedPalette { pub name: String, pub palette: Palette }
```

- **`.gpl` (GIMP):** cabecera `GIMP Palette`, línea `Name:` opcional, comentarios `#` y líneas
  `R G B [nombre]`. Se ignoran `Columns:` y los nombres de color. Al escribir se pone `Name:`.
- **`.hex`:** un color `rrggbb` por línea, con o sin `#`. Se ignoran las líneas vacías.
- **`.pal` (JASC):** `JASC-PAL`, `0100`, la cantidad y una línea `R G B` por color.
- **`.png`:** si es indexado, su paleta (PLTE). Si no, los colores únicos opacos en orden de
  aparición; con más de 256 es un error.
- **`.ase` / `.aseprite`:** la paleta del sprite, con el lector que ya existe.
- **Nombre:** `Name:` del `.gpl` si lo hay; si no, el nombre del archivo sin extensión.
- **Errores:** archivo vacío, sin colores, más de 256 colores, cabecera inválida o un número fuera
  de 0–255. Siempre indican la línea.
- Con más de 256 colores se rechaza el archivo, no se trunca.

## 2. Presets incluidos (`cs-core/src/palettes.rs`)

Unas 10 paletas clásicas de dominio público o CC0 compiladas en el binario, con su autor:
DB16 y DB32 (DawnBringer), PICO-8, Game Boy (4 verdes), NES (54 colores), Sweetie 16 (GrafxKid),
Endesga 32, AAP-64, CGA (16) y 1-bit (blanco y negro). Antes de incluir cada una se verifica su
licencia en su página de Lospec. Si alguna no es libre, se reemplaza por otra que sí lo sea.

`Palette::db32()` sigue existiendo y es la paleta por defecto de fábrica.

## 3. Biblioteca (`cs-ui/src/palette_library.rs`)

```rust
pub struct PaletteLibrary { builtin: Vec<Preset>, user: Vec<Preset>, default: Option<String> }
pub struct Preset { pub name: String, pub palette: Palette, pub source: PresetSource }
pub enum PresetSource { Builtin, UserFile(String) /* nombre de archivo */ }
```

- **Escritorio:** carpeta `palettes/` dentro de `eframe::storage_dir("ChimpSprite")`. Se lee al
  iniciar y al abrir la ventana de presets, así aparece lo que el usuario soltó ahí a mano. Lee
  todos los formatos de la sección 1; los archivos que no se pueden leer se saltan y se avisa
  una vez en el log.
- **Web:** clave `chimpsprite.palettes` en `localStorage`, con una lista JSON de
  `{ name, colors: ["rrggbb", …] }`.
- **Guardar como preset:** escribe `<nombre>.gpl` (en web, en la lista). Si ya existe, pregunta si
  reemplazarlo. El nombre se limpia de caracteres inválidos para archivos.
- **Borrar un preset de usuario:** botón en la ventana, con confirmación. Los incluidos no se
  borran.
- **Paleta por defecto:** se guarda el nombre del preset con la persistencia de eframe, como los
  archivos recientes. Si el preset ya no existe, se usa DB32.

## 4. Interfaz

En el panel de paleta, al lado de la etiqueta "Palette":

- **Presets ▾:** abre la ventana **Palette Presets**. Tiene un campo de búsqueda por nombre y dos
  grupos, Built-in y User. Cada fila muestra el nombre, la cantidad de colores y una tira con los
  colores (hasta 64 visibles; si hay más, termina en "…"). Clic en una fila aplica la paleta al
  sprite actual. Clic derecho en un preset de usuario abre Delete y Set as Default.
- **Menú ⋯:**
  - **Load Palette…:** abre un archivo de cualquier formato de la sección 1 y lo aplica.
  - **Save Palette…:** guarda la paleta actual como `.gpl`, `.hex` o `.pal`.
  - **Save as Preset…:** pide un nombre y guarda en la carpeta de usuario.
  - **Set as Default:** la paleta actual pasa a ser la de los sprites nuevos. Si no es un preset,
    primero se guarda como uno.
  - **Load Default:** aplica la paleta por defecto.
  - **Open Folder:** abre la carpeta `palettes/` en el explorador. Solo en escritorio.

## 5. Aplicar una paleta

`Editor::apply_palette(palette)` ejecuta `Command::SetPalette`: un paso de undo.

- **RGBA:** cambian los colores del panel; los píxeles quedan igual.
- **Indexado:** cada índice toma el color nuevo de su posición, como en Aseprite. Si la paleta
  nueva es más corta, los índices que sobran se ven transparentes hasta que se cargue una más
  larga. El índice transparente no cambia.
- Si el color primario o el secundario eran índices fuera de la paleta nueva, pasan al 0.
- **Sprites nuevos:** `new_document` usa la paleta por defecto en vez de DB32.

## 6. Tests

- **Formatos:** leer y escribir de ida y vuelta `.gpl`, `.hex` y `.pal`; `.gpl` con comentarios,
  sin `Name:` y con nombres de color; `.hex` con y sin `#`; errores con número de línea; más de
  256 colores rechazado; `.png` indexado y RGBA; paleta de un `.aseprite`.
- **Biblioteca:** leer una carpeta temporal con archivos válidos e inválidos; guardar como preset
  y reemplazar; la paleta por defecto cae a DB32 si el preset no existe; la codificación de web
  (JSON) de ida y vuelta.
- **Editor:** aplicar en RGBA y en indexado es un paso de undo; los colores primario y secundario
  se ajustan a la paleta nueva; un sprite nuevo usa la paleta por defecto.
