# Workspace administrable (tipo Photoshop) y formas responsivas

Fecha: 2026-11-09. Estado: diseño aprobado por el usuario en la conversación; falta el plan.

## Objetivo

Que el usuario administre su workspace como en Photoshop: cualquier panel se acopla a cualquier
lado, se agrupa en pestañas, flota en una ventana, se redimensiona o se colapsa a íconos, y el
acomodo se guarda en workspaces con nombre. Para eso los paneles tienen que ser **responsivos**:
dejar de dibujarse con coordenadas absolutas para un ancho fijo y acomodarse al ancho que les toque.

## Punto de partida

- La ventana es una pila fija de `egui::Panel` en un orden escrito en `ChimpApp::layout` /
  `editor_panels` (`crates/cs-ui/src/lib.rs`, ~1436–1949): menú, pestañas, estado, barra de
  opciones, toolbar, color, dock, timeline y lienzo. Solo se redimensionan el color, el timeline y
  la división paleta/editor.
- El dock (`dock.rs`) abre un flyout a la vez; no guarda nada. `DockPanel` (9 variantes, con
  título e ícono) es la semilla del registro de paneles.
- 6 de los 9 paneles del dock dibujan en coordenadas absolutas para un ancho fijo del mockup:
  Groom, Lumen y Library (434), Dafont y Lospec (364), FontForge (304), Modelo (227).
- 356 tests de layout (`layout_tests.rs`, 11 285 líneas) fijan posiciones exactas.

## Decisiones del usuario

1. **Alcance: todo, como Photoshop.** Toolbar, color/paleta, timeline, preview y todos los paneles
   del dock se acoplan, se agrupan, flotan y se redimensionan. Quedan fijos: menú, barra de
   pestañas de documentos, barra de opciones de herramienta (arriba), barra de estado (abajo) y el
   lienzo como hueco central del docking.
2. **Librerías existentes, no inventar:** `egui_dock` para el docking y `egui_taffy` (taffy:
   flexbox + CSS grid) para las formas. Elegidas con un prototipo (ver abajo); el usuario lo probó
   con el mouse ("se siente bien").
3. **La tira de íconos del dock** se vuelve el modo "colapsado a íconos" de Photoshop: cualquier
   grupo acoplado se puede colapsar a una tira y expandir; los flyouts siguen existiendo.
4. **Workspaces como Photoshop:** de fábrica + del usuario con nombre, Window → Workspace, Reset, y
   el acomodo actual se recuerda al cerrar. Globales (no por documento ni por tipo de pestaña).
5. **Mockups y anchos:** el mockup define cómo se ve un panel en su **ancho de referencia** (pixel a
   pixel, como hasta ahora). Más angosto, las filas se parten; más ancho, crecen los campos
   marcados; debajo del ancho mínimo, scroll vertical (nunca horizontal).

## Prototipo

Rama `proto-workspace`, commit `8a47d11`: `crates/cs-ui/examples/proto_workspace.rs` y
`PROTO_WORKSPACE_NOTES.md` (capturas en `.refs/proto-workspace/`). Resultados:

| | egui_dock 0.21 | egui_tiles 0.17 |
|---|---|---|
| Pestañas pixel art | sí, pintando las del kit encima (la librería no deja dibujarlas) | sí, limpio (`Behavior`) |
| Divisores y zonas de drop | sí (color marcador → piezas en un plugin de egui) | sí |
| Ventanas flotantes | **sí, de serie** | no (1–2 semanas hacerlas) |
| Guardar / cargar | probado (RON, ~1.6 KB) | solo guardar |
| Esfuerzo estimado | 3–5 días | 2–3 días + flotantes |

No se puede vestir en egui_dock sin fork: lo que dibuja con líneas (× y cruz de drop: se usa el modo
de áreas resaltadas), las flechas de desborde de pestañas y los textos de su menú contextual.
Depende de dos ids internos: la versión se fija.

Formas: **egui_taffy** redondea a pixeles enteros y parte bien filas anidadas (0 nodos fraccionarios
a 220, 400 y 600); egui_flex deja medios pixeles y no parte contenedores anidados.

Las dos librerías van en versiones compatibles con egui 0.36 (`egui_dock 0.21`, `egui_taffy 0.14`
con taffy 0.9). Subir a egui 0.37 (que piden las versiones nuevas) es un paso aparte, opcional.

## Arquitectura

Cuatro módulos nuevos en `crates/cs-ui`, cada uno con una sola responsabilidad. Solo `forms` conoce
egui_taffy y solo `workspace` conoce egui_dock.

### `forms`: componentes y formas

- Un panel se describe con contenedores (`row`, `column`, `grid(cols)`, `wrap`) y hojas: componentes
  del kit (`field("Family", input)`, botones, steppers, casillas, selects) o un **lienzo libre** con
  tamaño propio (vista previa de Groom, damero, vista previa de métricas, mapa de glifos), que por
  dentro se dibuja como hoy.
- Cada hoja declara su tamaño natural: alto fijo (17, 15…), ancho de referencia, ancho mínimo y si
  crece (`grow`).
- `forms` arma el árbol de taffy, calcula, **redondea a pixeles enteros** y dibuja cada componente en
  su rect con las variantes `_at(rect)` / `*_shapes` que ya tiene el kit.
- No cambia el kit, el skin ni sus piezas: decide dónde va cada cosa, no cómo se dibuja.
- Si el contenido no cabe a lo alto, scroll vertical (con el scroll pixel art de `pixel_ui`).

### `panels`: registro de paneles

- `PanelId` para cada panel acoplable: Tools, Color (paleta + editor), Timeline, Preview, Artboards,
  Library, Slices, Lospec, Dafont, Groom, Lumen, Font (FontForge), Model.
- Cada uno declara título, ícono (pieza `icon.dock.*` u otra), ancho de referencia (el del mockup),
  ancho mínimo, si se puede cerrar, y su función de dibujo `fn(&mut ChimpApp, &mut Ui)`.
- Reemplaza el `match` de `lib.rs` y los métodos `*_panel` sueltos. Los paneles que no aplican a la
  pestaña actual (p. ej. FontForge en un sprite) se ven deshabilitados con una frase, como hoy.

### `workspace`: el docking

- Un `DockState<PanelId>` de egui_dock. Las hojas son grupos de pestañas; el **lienzo** es un nodo
  especial central que no se cierra ni se mueve; las flotantes son superficies de egui_dock.
- **Colapsado a íconos:** cualquier grupo acoplado a una orilla se colapsa a una tira de íconos (con
  « en su barra o doble clic). Clic en un ícono abre ese panel como flyout (como el dock de hoy);
  otro clic lo cierra. Arrastrar un ícono fuera de la tira saca el panel para acoplarlo o hacerlo
  flotar. egui_dock no tiene este modo: el grupo colapsado es una hoja marcada `collapsed`,
  dibujada con el código actual del dock.
- **Reglas:** cada panel existe una sola vez; abrirlo desde Window lo enfoca donde esté o lo pone en
  su lugar de fábrica; cerrarlo lo saca del árbol. Ningún split deja un panel más angosto que su
  mínimo.
- **Piel:** como el prototipo: pestañas del kit pintadas encima, divisores / zonas de drop / marco de
  flotantes cambiados por piezas, splits y ventanas ajustados a pixeles enteros. egui_dock con
  versión fija (`=0.21.x`).

### `workspaces`: presets y guardado

- **De fábrica** (en código): **Essentials** (el acomodo de hoy: toolbar izquierda, color derecha,
  timeline abajo, tira de íconos con el resto), **Animation** (timeline alto y preview acoplado),
  **Tiles** (Slices y Artboards abiertos; el panel de tiles se suma si se vuelve panel propio), **Fonts** (FontForge acoplado abierto).
- **Del usuario:** con nombre, en `app.ron` (clave nueva `chimpsprite.workspaces`), junto con el
  acomodo actual (que se guarda al cerrar y se restaura al abrir).
- **Menú Window:** paneles con ✓ los abiertos; submenú Workspace con los de fábrica y los del
  usuario; New Workspace…, Delete…, Reset "Nombre" (vuelve el actual a como se guardó).
- **Errores:** un árbol guardado con paneles desconocidos descarta esas hojas; uno que no se puede
  leer usa Essentials y lo anota en el log. Nunca truena.

## Fases (un PR por fase; la app funciona al final de cada una)

- **A. `forms` + migrar los paneles de ancho fijo**, uno por commit: FontForge Config (ya probado
  en el prototipo), Groom, Lumen, Library, Dafont, Modelo (y Lospec si hace falta). En su ancho de
  referencia se ven igual que hoy; a otros anchos se reacomodan.
- **B. Registro de paneles.** Todo pasa por `panels` por dentro; por fuera nada cambia.
- **C. Docking del lado derecho:** la tira de íconos y sus flyouts + el panel de color pasan a
  egui_dock (colapsado a íconos, flotantes, arrastrar).
- **D. El resto al árbol:** toolbar, timeline y preview; workspaces de fábrica y del usuario; menú
  Window. Al final, Essentials se ve igual que hoy.
- **E. Tests de layout reescritos:** medir dentro de cada panel y en el preset Essentials, no en
  posiciones absolutas de la ventana.

## Pruebas

- **forms:** layout puro sin ventana a 220, al ancho de referencia y a 600: sin medios pixeles, sin
  encimarse, y al ancho de referencia los mismos rects que hoy.
- **workspace:** el árbol (mover, colapsar, flotar, cerrar y reabrir, guardar y cargar, árbol roto o
  con paneles desconocidos).
- **App:** tests de layout por panel y en Essentials; capturas comparadas con las de hoy al cerrar la
  fase D.
- **A mano:** arrastres reales con el mouse (los agentes solo inyectan eventos).

## Fuera de alcance

- Varios documentos lado a lado en el hueco central (los documentos siguen en la barra de pestañas).
- Workspaces por documento o por tipo de pestaña.
- Editor visual de formas dentro de la app o archivos de formas editables por el usuario.
- Mover el menú, la barra de pestañas, la barra de opciones o la barra de estado.
- Subir egui a 0.37 (paso aparte si se quiere).
