# Buscador de Lospec

Fecha: 2026-10-22 · Estado: aprobado

## Contexto

Segunda parte de la biblioteca de paletas (`2026-10-22-palette-library-design.md`): buscar
paletas en `lospec.com`, aplicarlas y guardarlas como presets de usuario. Se implementa después
de la biblioteca porque usa sus presets, su `Save as Preset` y `Editor::apply_palette`.

Lo que ofrece Lospec (verificado el 2026-10-22):

- **Paleta por slug:** `https://lospec.com/palette-list/<slug>.json`. Es oficial y manda
  `Access-Control-Allow-Origin: *`, así que funciona en web y en escritorio. Devuelve
  `{ name, author, colors: ["rrggbb", …] }`. Si el slug no existe, responde 404.
- **Listado:**
  `https://lospec.com/palette-list/load?colorNumberFilterType=<f>&colorNumber=<n>&page=<p>&tag=<t>&sortingType=<s>`.
  No está documentado y **no tiene CORS**, así que solo funciona en escritorio. Devuelve
  `{ palettes: [...], totalCount }`, con 10 paletas por página. Cada paleta trae `title`, `slug`,
  `colors`, `tags`, `likes`, `downloads` (texto con comas: `"25,876"`), `numberOfColors` y
  `user.name`.
  - `colorNumberFilterType`: `any`, `exact`, `min` o `max`.
  - `sortingType`: `default`, `alphabetical`, `downloads` o `newest`.
  - **El texto solo filtra por tag.** El listado ignora `title`, `search` y `name`. Para buscar
    por nombre solo sirve el slug exacto.

Decisión del usuario (opción C): en escritorio, buscador completo; en web, solo pegar un link o
un nombre.

## Objetivo

- Buscar en Lospec con todos sus filtros: tag, cantidad de colores, orden y paginación (solo en
  escritorio).
- Traer una paleta pegando su link, su slug o su nombre (en web y en escritorio).
- Aplicar o guardar cualquier resultado.

Fuera de alcance: likes, comentarios, cuenta de Lospec, subir paletas, cachear resultados entre
sesiones y miniaturas descargadas (la tira de colores se dibuja desde los datos).

## 1. Lógica pura (`cs-io/src/lospec.rs`)

Sin red. Todo se prueba con fixtures.

```rust
pub enum ColorCount { Any, Exact(u16), Min(u16), Max(u16) }
pub enum LospecSort { Default, Alphabetical, Downloads, Newest }
pub struct LospecQuery { pub tag: String, pub colors: ColorCount, pub sort: LospecSort, pub page: u32 }

impl LospecQuery { pub fn listing_url(&self) -> String; }
pub fn palette_url(slug: &str) -> String;                 // …/palette-list/<slug>.json
pub fn page_url(slug: &str) -> String;                    // …/palette-list/<slug> (para # Source:)

pub struct LospecPalette {
    pub slug: String, pub title: String, pub author: Option<String>,
    pub palette: Palette, pub likes: Option<u32>, pub downloads: Option<u32>,
}
pub struct LospecPage { pub palettes: Vec<LospecPalette>, pub total: u32 }

pub fn parse_listing(json: &[u8]) -> Result<LospecPage, LospecError>;
pub fn parse_palette(json: &[u8], slug: &str) -> Result<LospecPalette, LospecError>;
pub fn slug_from_input(input: &str) -> Option<String>;
```

- **`listing_url`:**
  - El tag se pasa en minúsculas, sin espacios al inicio ni al final, y codificado para URL.
  - Con `Any`, `colorNumber` vale 8 (Lospec lo pide aunque no lo use).
- **`parse_listing`:**
  - Un `downloads` con comas se convierte a número; si no se puede leer, queda `None`.
  - Una paleta con un color inválido o con más de 256 colores se salta. El resto de la página se
    conserva.
- **`parse_palette`:**
  - `name` pasa a `title`.
  - Si `author` viene vacío, queda `None`.
  - Con más de 256 colores o un color inválido devuelve error.
- **`slug_from_input`:**
  - Acepta `https://lospec.com/palette-list/sweetie-16`, con o sin `www.`, `http`, `.json`, `/`
    final, query o `#`.
  - Acepta un slug (`sweetie-16`) o un nombre (`Sweetie 16`): todo pasa a minúsculas, cada
    espacio o carácter fuera de `[a-z0-9-]` pasa a `-`, y los guiones repetidos se juntan.
  - Devuelve `None` si queda vacío o si es una URL de otro sitio o de otra sección de Lospec.
- **`LospecError`:** `InvalidJson`, `TooManyColors` y `BadColor(index)`.

## 2. Cliente HTTP (`cs-ui/src/lospec_client.rs`)

Con `ehttp`: en escritorio usa `ureq` en un hilo y en web usa `fetch`. Es una sola API para las
dos plataformas, y la app no necesita tokio.

```rust
pub struct Pending<T> { slot: Arc<Mutex<Option<Result<T, FetchError>>>> }
impl<T> Pending<T> { pub fn poll(&self) -> Option<Result<T, FetchError>>; }

pub fn fetch_listing(ctx: &egui::Context, query: &LospecQuery) -> Pending<LospecPage>;   // solo escritorio
pub fn fetch_palette(ctx: &egui::Context, slug: &str) -> Pending<LospecPalette>;
pub enum FetchError { Network, NotFound, Status(u16), Parse(LospecError) }
```

- Se manda el header `User-Agent: ChimpSprite/<versión>`.
- Al llegar la respuesta se llama a `ctx.request_repaint()`.
- `fetch_listing` no se compila en wasm (`cfg(not(target_arch = "wasm32"))`).
- Solo se pide al presionar Enter o un botón, nunca con cada tecla.
- Hay una petición a la vez por tipo. Una búsqueda nueva descarta la respuesta pendiente.

## 3. Estado de la pestaña (`cs-ui/src/lospec_tab.rs`)

```rust
pub struct LospecTab {
    query: LospecQuery,                 // solo escritorio
    results: Vec<LospecPalette>, total: u32,
    listing: Option<Pending<LospecPage>>,
    link_input: String, link_result: Option<LospecPalette>,
    lookup: Option<Pending<LospecPalette>>,
    status: TabStatus,                  // Idle | Loading | Error(String)
}
```

- **Search:**
  - Pone `page = 0`, vacía los resultados y pide el listado.
  - Si el texto también forma un slug, pide esa paleta en paralelo. Si existe, sale primero en
    los resultados, marcada como "Exact match", y no se repite si también vino en el listado.
- **Load more:** pide `page + 1` y agrega los resultados al final. Solo aparece si
  `results.len() < total`.
- **Fetch (campo de link):** pide la paleta del slug y la muestra en una sola fila.
- La lógica de estado está separada del dibujo, para probarla inyectando respuestas.

## 4. Interfaz

La ventana **Palette Presets** tiene dos pestañas: **Library** (la de la biblioteca) y **Lospec**.

**Escritorio:**

```
Tag [ gameboy        ]  Colors [Any ▾] [ 8 ]  Sort [Default ▾]  [Search]
Paste a Lospec link or palette name [                    ] [Fetch]
───────────────────────────────────────────────────────────────────────
Ice Cream GB · Kerrie Lake · 4 colors · ♥1645 · ⤓25,876   [Apply] [Save]
■■■■
Kirokaze Gameboy · … · In library                          [Apply] [Save]
…
                    [Load more (10 of 412)]
```

- El número de colores solo se puede editar si el filtro no es `Any`. Va de 1 a 256.
- La tira de colores usa el mismo widget que la pestaña Library (hasta 64 colores visibles y
  "…" si hay más).
- **"In library"** sale cuando ya hay un preset de usuario con ese nombre.
- Debajo de cada fila va un link "lospec.com", que abre la página de la paleta en el navegador.
- Mientras carga, se ve un spinner en lugar de "Search" o "Load more".

**Web:** solo el campo de link con **Fetch** y la fila del resultado. Debajo, la nota "Search is
available in the desktop app".

**Botones de cada fila:**
- **Save:** la guarda con el `Save as Preset` de la biblioteca, como `<título>.gpl` con `Name:`,
  `# Author: <autor>` y `# Source: <page_url>`. Si ya existe, pregunta si reemplazarla. En web
  se guarda en la lista de `localStorage`, que no tiene comentarios, así que el autor no se
  guarda.
- **Apply:** `Editor::apply_palette(palette)`, un paso de undo. Si no hay un preset de usuario
  con ese nombre, también la guarda; si ya lo hay, no lo toca ni pregunta.

## 5. Errores

Todos se muestran en la pestaña, en una línea en rojo. Nunca hay panic.

| Caso | Mensaje |
| --- | --- |
| Sin red, timeout o falla de DNS | "Couldn't reach Lospec." |
| 404 en `fetch_palette` | "Palette not found on Lospec." |
| Otro código distinto de 200 | "Lospec returned an error (<código>)." |
| JSON del listado ilegible | "Lospec search is unavailable. Paste a palette link instead." |
| JSON de una paleta ilegible o más de 256 colores | "That palette can't be loaded." |
| `slug_from_input` devuelve `None` | "Paste a lospec.com palette link or a palette name." |

Si la búsqueda exacta por slug falla mientras el listado responde bien, no se muestra ningún
error.

## 6. Dependencias

- `cs-ui`: `ehttp = "0.5"` (o la versión vigente compatible con egui 0.36).
- `cs-io`: `serde_json` (ya está en el workspace).
- Antes de cerrar, hay que verificar que `cargo check -p cs-ui --target wasm32-unknown-unknown`
  sigue compilando.

## 7. Tests

- **`cs-io` (fixtures en `crates/cs-io/tests/fixtures/lospec/`, respuestas reales guardadas):**
  - `listing_url` con cada filtro y orden, y con un tag con espacios y caracteres especiales.
  - `parse_listing` de una página real: total, descargas con comas, autor; se salta una paleta
    rota sin perder el resto de la página.
  - `parse_palette` de `sweetie-16.json`; también con autor vacío, más de 256 colores y un color
    inválido.
  - `slug_from_input`: URL con y sin `.json`, `www.`, `/`, query y `#`; nombre con espacios y
    acentos; URL de otro sitio (`None`); texto vacío (`None`).
- **`cs-ui` (sin red, inyectando respuestas en los `Pending`):**
  - Search reinicia la página y Load more agrega resultados.
  - La coincidencia exacta sale primero y no se duplica.
  - Una búsqueda nueva descarta la respuesta vieja.
  - Cada `FetchError` muestra su mensaje.
  - Apply guarda solo si no existe y es un paso de undo.
  - Save con un nombre que ya existe pide confirmación.
- **Contra Lospec real:** un test `#[ignore]` que busca `gameboy` y trae `sweetie-16`. Se corre
  a mano con `cargo test -- --ignored`.
