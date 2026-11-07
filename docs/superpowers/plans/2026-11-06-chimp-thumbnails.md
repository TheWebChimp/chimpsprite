# Plan: miniaturas de los `.chimp` en el explorador de archivos

Spec: `docs/superpowers/specs/2026-11-06-chimp-thumbnails-design.md`. Rama `chimp-thumbnails`. TDD:
cada tarea empieza por sus tests, termina con tests + clippy en verde y un commit.

## Tarea 1: `cs-io/src/thumbnail.rs`
- `ENTRY`, `SIZE`, `fit`, `pick`, `render`, `read_png`/`read` (sobre `Read + Seek`, con topes),
  `for_file`. `import::decode_png` con tope configurable.
- Tests: `fit` amplía por entero y reduce; `pick` (activo, fuente → lienzo, inexistente);
  `render` (instancias, referencias fuera, vacío → `None`); `read` de un zip sin la entrada →
  `None`; PNG gigante → error.

## Tarea 2: escribirla al guardar
- `chimp::ThumbnailFrom`, `chimp::write_with`; `write` = `Auto`. `files::encode_with`.
- `cs-ui`: `save_bytes` pasa el artboard activo; el autosave usa `None`.
- Tests: el `.chimp` trae `thumbnail.png` del activo; sin miniatura si está vacío; un `.chimp`
  viejo sin la entrada abre igual; el autosave no la trae; `save_bytes` usa el activo.

## Tarea 3: subcomando `chimpsprite --thumbnail` + archivos de Linux
- `apps/chimpsprite/src/thumbnail_cli.rs` (parseo + correr), llamado desde `main` antes de abrir
  la ventana. `linux/chimpsprite.thumbnailer`, `linux/chimpsprite-mime.xml`, `deb.files`.
- Tests: parseo (tamaño por omisión, errores), corrida con archivos temporales (incrustada y
  archivo viejo), el `Exec` del `.thumbnailer` lo entiende el parser, el XML tiene el glob.

## Tarea 4: crate `chimpsprite-thumbnails` (Windows)
- `ThumbnailProvider` (`IInitializeWithStream` + `IThumbnailProvider`), adaptador `IStream` →
  `Read + Seek`, DIB BGRA, fábrica, `DllGetClassObject`/`DllCanUnloadNow`,
  `DllRegisterServer`/`DllUnregisterServer` con `registry_entries`.
- Tests: proveedor directo y por fábrica con `SHCreateMemStream`, píxeles del HBITMAP, sin
  miniatura → error, doble `Initialize` → error, claves del registro.

## Tarea 5: función C + esqueleto de macOS + docs
- `chimp_thumbnail_png`/`chimp_thumbnail_free` (todas las plataformas de escritorio), header,
  Swift e `Info.plist`; `docs/thumbnails.md` (registrar a mano, Linux, macOS, instalador).
- Tests de la función C (ok, sin miniatura, puntero nulo).

## Tarea 6: verificación completa (test, clippy, fmt, wasm), autorrevisión, minors, rebase.
