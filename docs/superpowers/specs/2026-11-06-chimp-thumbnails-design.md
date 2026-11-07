# Miniaturas de los `.chimp` en el explorador de archivos

## Contexto

Un `.chimp` es un zip (`manifest.json` + un `.aseprite` por artboard). El Explorador de Windows,
Finder y los gestores de archivos de Linux hoy le ponen un ícono genérico. Aseprite tampoco trae
miniaturas propias. El objetivo es que cada `.chimp` se vea con su dibujo, como un PNG.

La idea es la de casi todos los formatos de documento (`.kra`, `.ora`, `.docx`): **la miniatura
va adentro del archivo**, ya renderizada, y el proveedor de cada sistema solo la saca del zip (no
compone capas ni carga el documento). Así el código de cada plataforma es corto y rápido.

## 1. La miniatura dentro del `.chimp` (`cs-io/src/thumbnail.rs`)

- Entrada `thumbnail.png` en la raíz del zip, guardada sin comprimir (el PNG ya lo está). RGBA de 8
  bits.
- **Qué se dibuja:** el frame 1 del artboard activo; si el activo es una fuente (su frame 1 es un
  glifo suelto) o no existe, el primer artboard del lienzo; si no hay, el primero del documento. Se
  materializa (instancias y tilemaps) y salen las máscaras; las capas de referencia no (igual que al
  exportar PNG).
- **Tamaño:** el lado mayor ≤ 256 px. Si el sprite es más chico se amplía por el mayor factor
  entero que cabe (vecino más cercano: 16×16 → 256×256, 24×10 → 240×100); si es más grande se
  reduce con vecino más cercano hasta 256.
- **Fondo transparente** (sin damero): el Explorador y Finder dibujan la transparencia sobre su
  propio fondo, como con un PNG.
- Un frame **completamente transparente** no guarda miniatura: el explorador muestra el ícono
  normal en lugar de un cuadro vacío.
- **Quién la escribe:** `chimp::write(doc)` (miniatura automática: el primer artboard del lienzo) y
  `chimp::write_with(doc, ThumbnailFrom::…)`: `Auto`, `Of(id)` (el activo, al guardar desde la
  app) o `None` (el autosave del navegador: no gasta tiempo cada 10 s ni espacio en
  `localStorage`). `files::encode_with(doc, name, active)` lo usa al guardar; `files::encode`
  sigue igual (automático), así que el MCP también guarda miniatura.
- **Compatibilidad:** `chimp::read` ignora entradas que no conoce, así que los archivos nuevos
  abren en versiones viejas y los viejos (sin miniatura) abren igual. No cambia `VERSION`.
- **Lector barato:** `thumbnail::read_png(reader)` / `thumbnail::read(reader)` sobre cualquier
  `Read + Seek`: el zip solo lee el directorio central y la entrada `thumbnail.png`; no toca el
  manifest ni los `.aseprite`. Topes: 8 MB de PNG y 2048 px de lado (si no, error), porque lo
  corre el proceso de miniaturas del sistema con archivos que pueden venir de cualquier lado. Es
  wasm-compatible (no usa archivos).
- `thumbnail::fit(bitmap, size)`: el mismo escalado para el tamaño que pide cada sistema.
- `thumbnail::for_file(bytes, size)`: la incrustada o, si el archivo es viejo, abre el documento y
  la renderiza (lo usa el subcomando de Linux; el proveedor de Windows no, ver abajo).

## 2. Windows: `IThumbnailProvider` (`apps/chimpsprite-thumbnails`)

Crate nuevo `chimpsprite-thumbnails`, `crate-type = ["cdylib", "rlib"]` → `chimpsprite_thumbnails.dll`.
Todo lo de COM va con `#[cfg(windows)]` (crate `windows` 0.62, ya en el lock); en Linux/macOS el
crate compila con la parte C (ver 4). No está en el check de wasm de CI.

- CLSID `{4E744B03-3050-45ED-9272-5B94F8A131B4}`.
- `ThumbnailProvider` implementa `IInitializeWithStream` (la forma recomendada: el shell la puede
  correr en el proceso aislado sin acceso al archivo) e `IThumbnailProvider`.
  - `Initialize` guarda el `IStream` (una segunda vez: `ERROR_ALREADY_INITIALIZED`).
  - `GetThumbnail(cx)` envuelve el stream en un `Read + Seek`, lee `thumbnail.png` con
    `cs_io::thumbnail::read`, lo ajusta a `cx` con `fit` y crea una sección DIB de 32 bits de arriba
    abajo en BGRA sin premultiplicar, con `WTSAT_ARGB` (como el ejemplo de Microsoft).
  - Sin miniatura incrustada devuelve un error (`E_FAIL`) y el Explorador pone el ícono normal:
    la DLL no compone documentos (rápida, sin todo el motor adentro del proceso del shell).
- `DllGetClassObject` + `IClassFactory`, `DllCanUnloadNow` (cuenta objetos y `LockServer`).
- **Registro por usuario (HKCU, sin admin)** en `DllRegisterServer` / `DllUnregisterServer`
  (`regsvr32`):
  - `HKCU\Software\Classes\CLSID\{CLSID}` = "ChimpSprite Thumbnail Provider"
  - `…\{CLSID}\InprocServer32` = ruta de la DLL, `ThreadingModel` = `Apartment`
  - `HKCU\Software\Classes\.chimp\ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}` = `{CLSID}`
  - después `SHChangeNotify(SHCNE_ASSOCCHANGED)` para que el Explorador se entere.
  - Desregistrar borra la clave del CLSID y la de `ShellEx` (no toca `.chimp` en sí).
  - La lista de claves sale de una función pura (`registry_entries`) que se prueba; la escritura al
    registro no corre en los tests.
- **Tests** (solo Windows): crean el proveedor directo y por la fábrica, lo inicializan con
  `SHCreateMemStream` sobre un `.chimp` hecho con `cs_io::chimp::write`, piden 64 y 300 px y leen
  los píxeles del HBITMAP (`GetObjectW` → `DIBSECTION`); un `.chimp` sin miniatura falla; el doble
  `Initialize` falla.
- **Instalador:** hoy cargo-packager arma un NSIS sin ganchos de post-instalación (solo
  `preinstall_section` o un template `.nsi` propio) y su lista de `resources` es la misma para las
  tres plataformas. Integrarlo es: compilar `-p chimpsprite-thumbnails` en el job de Windows,
  copiar la DLL junto al `.exe` y llamar `regsvr32 /s` al instalar y `/u` al desinstalar (template
  NSIS propio). Queda pendiente (no se puede probar sin gastar una corrida de release); ver
  `docs/thumbnails.md` para registrarla a mano.

## 3. Linux: thumbnailer de freedesktop

- Subcomando del mismo binario: `chimpsprite --thumbnail <entrada> <salida> [tamaño]` (tamaño por
  omisión 256): escribe un PNG con `thumbnail::for_file` + `fit`. Sale con código 1 y un mensaje en
  stderr si falla. Funciona en las tres plataformas de escritorio (sirve para probarlo en Windows).
- `apps/chimpsprite/linux/chimpsprite.thumbnailer`:
  `Exec=chimpsprite --thumbnail %i %o %s`, `MimeType=application/x-chimpsprite;`.
- `apps/chimpsprite/linux/chimpsprite-mime.xml`: tipo `application/x-chimpsprite`, glob `*.chimp`,
  `sub-class-of application/zip` (para que el glob gane a la detección de zip por contenido).
- El `.deb` los instala en `/usr/share/thumbnailers/` y `/usr/share/mime/packages/`
  (`[package.metadata.packager.deb] files`; el trigger de shared-mime-info actualiza la base). El
  AppImage no puede instalar thumbnailers del sistema: queda documentado cómo copiarlos a mano.

## 4. macOS: esqueleto de Quick Look

Una extensión de miniaturas (`QLThumbnailProvider`) es un `.appex` firmado dentro del `.app` y se
arma con Xcode; cargo-packager no lo hace y no se puede probar desde Windows. Queda:

- En `chimpsprite-thumbnails`, una función C para todas las plataformas de escritorio
  (`chimp_thumbnail_png` / `chimp_thumbnail_free`), probada en Windows: bytes del `.chimp` → PNG
  incrustado.
- `apps/chimpsprite-thumbnails/macos/`: el header C, un `ThumbnailProvider.swift` y el `Info.plist`
  de la extensión (UTI `com.thewebchimp.chimpsprite.document`), como esqueleto documentado.

## Decisiones que el usuario puede revisar

1. **Fondo transparente** en vez de damero.
2. **Sin miniatura si el frame 1 está vacío** (el explorador muestra el ícono normal).
3. **Artboard:** el activo al guardar (salvo fuentes); el MCP y `chimp::write` usan el primero del
   lienzo. Siempre el frame 1, no el frame actual.
4. **256 px** con escalado entero; los sprites de más de 256 se reducen con vecino más cercano
   (pueden perder líneas finas).
5. **El autosave del navegador no guarda miniatura.**
6. **La DLL de Windows no renderiza archivos viejos** (sin `thumbnail.png`): hay que volver a
   guardarlos. El subcomando de Linux sí los renderiza (corre en su propio proceso).
7. **Registro por usuario (HKCU)** con `regsvr32`, sin admin; proceso aislado del shell (no se pone
   `DisableProcessIsolation`).
8. **El instalador de Windows todavía no instala la DLL**; el `.deb` sí instala el thumbnailer.
9. macOS queda como esqueleto (función C probada + Swift sin compilar).
