# ChimpSprite Web (subproyecto 2) — Diseño

Fecha: 2026-10-14
Estado: aprobado

## Objetivo

La misma app de escritorio corriendo en el navegador, publicada en GitHub Pages
(`https://thewebchimp.github.io/chimpsprite/`) con deploy automático en cada push a `main`.

## Enfoque

Se compila `cs-ui` + `apps/chimpsprite` a `wasm32-unknown-unknown` con `trunk`. Lo que depende del
sistema operativo queda detrás de un módulo `platform` en `cs-ui` con implementación de escritorio y
de web. El motor (`cs-core` … `cs-io`) no cambia: ya compila a wasm y el CI lo verifica.

## Capa `platform`

- `PlatformEvent` llega por una cola que la app revisa cada frame:
  `Opened { name, bytes }`, `Error(String)`. Así abrir es igual en ambas plataformas aunque en web sea
  asíncrono.
- `open_file(filters)`: escritorio → `rfd::FileDialog` síncrono + `std::fs::read`; web →
  `rfd::AsyncFileDialog` en `wasm_bindgen_futures::spawn_local`, el resultado se empuja a la cola.
- `save_file(suggested_name, bytes) -> SaveOutcome`: escritorio → `rfd` save dialog + escritura atómica,
  devuelve `Saved(PathBuf)` o `Cancelled`; web → descarga vía Blob + `<a download>`, devuelve
  `Downloaded(name)`.
- `save_to(path, bytes)` (solo escritorio): guardar sin preguntar.
- Exportes de varios archivos (spritesheet/atlas + JSON, secuencia PNG): escritorio igual que hoy;
  web → una descarga por archivo.
- Ubicación del documento: `DocLocation { name: String, path: Option<PathBuf> }` reemplaza a
  `Option<PathBuf>`. En web `path` siempre es `None`; "Save" descarga con `name`.
- Archivos soltados en la ventana: en web egui entrega `bytes`; en escritorio `path`.
- Línea de comandos (`--demo`, ruta de archivo) solo en escritorio; en web `?demo` en la URL carga el
  documento de ejemplo.

## Específico de web

- Render: eframe + wgpu con WebGPU y respaldo WebGL2 (feature `webgl` de wgpu).
- Aviso al salir: handler `beforeunload` activo mientras haya cambios sin guardar.
- Autosave: cada 10 s (y al ocultar la pestaña) si hay cambios, el documento como `.chimp` codificado en
  base64 va a `localStorage` (clave `chimpsprite.autosave`) junto con su nombre. Tope de 4 MB
  codificados; si no cabe, se avisa una vez en la barra de estado y no se guarda. Al iniciar, si hay
  autosave, se ofrece "Restore last session" / "Discard".
- `index.html`: pantalla de carga; mensaje claro si el navegador no soporta WebGPU ni WebGL2 o si la
  inicialización falla.
- Atajos reservados por el navegador (`Ctrl+N`, `Ctrl+W`) quedan disponibles desde el menú.

## Build y deploy

- `apps/chimpsprite/index.html` + `Trunk.toml`; `trunk build --release --public-url /chimpsprite/`.
- Perfil release con `opt-level = "z"` para el wasm vía `wasm-opt -Oz` (atributo `data-wasm-opt`).
- `.github/workflows/pages.yml`: build con trunk y deploy con `actions/deploy-pages` en push a `main`.
  Pages se configura en modo "GitHub Actions".
- CI existente: agrega `cargo check --target wasm32-unknown-unknown -p chimpsprite`.

## Testing

- Unit tests de lo puro: cola de eventos, nombres de descarga, codificación/decodificación y tope del
  autosave (en escritorio).
- Verificación del deploy con una petición HTTP al sitio publicado.
- Verificación visual: el usuario abre la URL publicada.
