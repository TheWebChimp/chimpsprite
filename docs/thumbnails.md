# Miniaturas de los `.chimp` en el explorador de archivos

Desde esta versión, al guardar un `.chimp` se incrusta `thumbnail.png` en el zip: el frame 1 del
artboard activo (o el primero del lienzo), con el lado mayor de hasta 256 px y ampliado por un factor
entero para que se vea pixel art, con fondo transparente. Un frame vacío no guarda miniatura. Los
archivos guardados antes no la tienen: hay que abrirlos y volver a guardarlos.

Para ver qué miniatura sale de un archivo, sin el explorador:

```
chimpsprite --thumbnail dibujo.chimp dibujo.png 256
```

(Si el archivo es viejo, el subcomando abre el documento y la renderiza.)

## Windows (Explorador)

La extensión es `chimpsprite_thumbnails.dll` (crate `apps/chimpsprite-thumbnails`), un
`IThumbnailProvider`. Se registra **por usuario** (HKCU), sin permisos de administrador.

### Registrarla a mano

En una terminal (PowerShell), desde la raíz del repo:

```powershell
cargo build --release -p chimpsprite-thumbnails
# Una carpeta fija: el registro guarda la ruta, y el Explorador deja la DLL cargada (bloqueada).
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\ChimpSprite\shell" | Out-Null
Copy-Item target\release\chimpsprite_thumbnails.dll "$env:LOCALAPPDATA\ChimpSprite\shell\"
regsvr32 "$env:LOCALAPPDATA\ChimpSprite\shell\chimpsprite_thumbnails.dll"
```

`regsvr32` avisa "DllRegisterServer ... succeeded". Escribe solo estas claves:

- `HKCU\Software\Classes\CLSID\{4E744B03-3050-45ED-9272-5B94F8A131B4}` (y su `InprocServer32` con la
  ruta de la DLL, `ThreadingModel = Apartment`)
- `HKCU\Software\Classes\.chimp\ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}` = el CLSID de
  arriba

Después:

1. Guarda un `.chimp` con un ChimpSprite de esta rama (o vuelve a guardar uno viejo).
2. En el Explorador, Ver → Iconos grandes / muy grandes. Si sigue el ícono viejo, refresca (F5).
3. Si aun así no cambia: Ver → Opciones → Ver → desmarca "Mostrar siempre iconos, nunca vistas en
   miniatura", y limpia la caché de miniaturas (Liberador de espacio en disco → "Miniaturas").

### Quitarla

```powershell
regsvr32 /u "$env:LOCALAPPDATA\ChimpSprite\shell\chimpsprite_thumbnails.dll"
```

Para reemplazar la DLL por una nueva, desregístrala, espera a que el "COM Surrogate"
(`dllhost.exe`) la suelte (o cierra sesión), cópiala y vuelve a registrarla.

### En el instalador (pendiente)

El instalador NSIS de cargo-packager no tiene ganchos de post-instalación (solo
`preinstall_section` o un template `.nsi` propio) y sus `resources` son los mismos para las tres
plataformas. Para incluirla:

1. En `.github/workflows/release.yml`, en el job de Windows: `cargo build --release -p
   chimpsprite-thumbnails --target x86_64-pc-windows-msvc`.
2. Un template NSIS propio (`[package.metadata.packager.nsis] template = …`) que copie la DLL junto
   al `.exe` y corra `"$SYSDIR\regsvr32.exe" /s "$INSTDIR\chimpsprite_thumbnails.dll"` al instalar y
   `/u` al desinstalar (antes de borrar archivos). Como el registro es en HKCU, encaja con la
   instalación por usuario.

## Linux

El `.deb` instala:

- `/usr/share/thumbnailers/chimpsprite.thumbnailer` (llama a `chimpsprite --thumbnail %i %o %s`)
- `/usr/share/mime/packages/chimpsprite.xml` (el tipo `application/x-chimpsprite`, `*.chimp`)

Nautilus, Nemo, Caja y Thunar (con tumbler) los usan. Con el AppImage hay que copiarlos a mano
(`~/.local/share/thumbnailers/` y `~/.local/share/mime/packages/` + `update-mime-database
~/.local/share/mime`), con `chimpsprite` en el `PATH`. Para regenerar las miniaturas: borrar
`~/.cache/thumbnails/`.

## macOS (esqueleto)

Una extensión de miniaturas de Quick Look es un `.appex` firmado dentro del `.app` y se arma con
Xcode; cargo-packager no lo hace y no se pudo probar. Lo que hay en
`apps/chimpsprite-thumbnails/macos/`:

- `chimp_thumbnail.h`: la función C `chimp_thumbnail_png` (probada en los tests de Rust); compilar
  la biblioteca con `cargo rustc --release -p chimpsprite-thumbnails --crate-type staticlib`.
- `ThumbnailProvider.swift` y `Info.plist`: la extensión (sin compilar).

Falta: un proyecto de Xcode con el target de la extensión, que el `Info.plist` de la app exporte el
UTI `com.thewebchimp.chimpsprite.document` (`UTExportedTypeDeclarations`, extensión `chimp`,
conforme a `public.zip-archive`), meter el `.appex` en `ChimpSprite.app/Contents/PlugIns/` y firmar.
