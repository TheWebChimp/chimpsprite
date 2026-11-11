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
  Groom, Lumen y Library (434), Dafont y Lospec (364), FontForge (314: 304 de contenido + la
  barra de 10), Modelo (227).
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
  referencia se ven igual que hoy; a otros anchos se reacomodan. **HECHA** (guía en `docs/forms.md`):
  `crates/cs-ui/src/forms.rs` + los seis paneles migrados, cada uno con `REFERENCE_WIDTH` /
  `MIN_WIDTH`: FontForge 314 / CONFIG 150 (`MIN_WIDTH` = 314 por ahora: solo la vista Config se
  migró y llega a `CONFIG_MIN_WIDTH` = 150), Groom 434 / 216, Lumen 434 / 216, Library 434 / 160, Dafont
  364 / 210, Modelo 227 / 200. El dock sigue abriendo cada panel a su `REFERENCE_WIDTH` hasta la
  fase C. (Lospec no se migró: queda a ancho fijo; se revisa en la fase C.)

  **Excepciones a propósito a "idéntico en `REFERENCE_WIDTH`":** el logo de Groom, Lumen y Dafont
  ahora muestra su fila de arriba completa; la vista previa de Library baja medio punto a escala 2
  (antes estaba a medio pixel del skin); el Export de Luminance en Lumen ya no se sale del panel
  (Generate 50, Modify 45 y Export 41 en la columna de 140, como el mockup); con Modify abierto,
  su raya naranja baja medio punto (a escala 2, 1 pixel de pantalla) para quedar en pixel entero.

  **Pendiente para la fase D:** un test de barrido de anchos de `MIN_WIDTH` a 600 por panel (ahora
  que se dibujan a cualquier ancho, acoplados o flotando); una variante de `run_at` a escala 2;
  medir el costo de recalcular taffy al arrastrar los splits. (El ancho de la barra de scroll de
  los paneles acoplados ya se reserva en la fase C: va en su mínimo.)
- **B. Registro de paneles.** Todo pasa por `panels` por dentro; por fuera nada cambia. **HECHA**:
  `crates/cs-ui/src/panel_registry.rs` tiene `PanelId` (13 paneles; 12 en web, sin Dafont),
  `PanelSpec { title, icon, plain, reference_width, min_width, closable, host }`, `Host::{Left,
  Right, Bottom, Window, Flyout(Body)}`, `Body::{Fit, Tall, Fixed}`, `ALL` y `DOCK`
  (`dock::DockPanel` es un alias). `app_panels.rs` tiene `PanelAction`, `draw_panel(ui, id)`,
  `draw_panel_modal(ui, id, modal)`, `apply_panel_action(ctx, action)` y `panel_flyout(ctx, id,
  anchor, modal)`: todos los flyouts del dock, Artboards/Slices, Tools, Color, Timeline y el panel
  fijo de la pestaña de modelo dibujan por ahí. La ventana real de Preview sigue llamando
  `Preview::show` → `contents`; `draw_panel(ui, PanelId::Preview)` existe pero aún no tiene host
  real (para la fase C). Los hosts no cambiaron.

  **Para la fase C** (notas de la revisión final):
  - `min_width` de Tools y Color son marcadores (min == referencia) y `PanelSpec.closable` todavía
    no se lee.
  - Una sola entrada que maneje el modal y la franja del logo de Lospec (hoy Color usa
    `draw_panel_modal` y el host pone `lospec_band` fuera de `add_enabled_ui`): algo como
    `draw_panel(ui, id, &PanelCtx { modal, bottom })`.
  - Ciclo de vida de Lospec: `close_lospec_panel()` corre cada frame en que el `app_panel` del dock
    no es Lospec (`lib.rs` ~1616); acoplado, eso borraría su estado cada frame y lanzaría
    `start_search` sin parar: hace falta "visible este frame" por id; y el fondo disponible debe
    ser genérico (`ui.max_rect().bottom()`), no exclusivo de Lospec (hoy es un parámetro privado de
    `draw_panel_in`).
  - Color depende del host: sincronización de `palette_locked` antes/después (moverla a
    `draw_color`), mínimo que depende del skin (`color_panel::panel_min_size`) y el apretón de
    manos de `palette_width_settled`.
  - Preview: play/pausa y centrar viven en la barra de título de la ventana (`Preview::show`);
    acoplado hay que llevarlos a `contents`; y `ClosePreview` solo limpia `view.preview`: debe parar
    la reproducción (p. ej. `Preview::stop()`).
  - Disponibilidad por panel (gate del timeline `is_structure_locked`/`model_edit_on`, Preview
    escondido en pestaña de modelo, Dafont en wasm): `PanelId::available(&ChimpApp) -> bool`.
  - Model tiene dos hosts (flyout y panel fijo de la pestaña de modelo): imponer una instancia por
    `PanelId` por frame (estado de `model_panel` e ids globales de `forms::form`).
  - `PanelSpec` necesita: `body` como campo propio (no dentro de `Host::Flyout`), `min_height`
    (Groom/Lumen tienen `MIN_HEIGHT`), tamaños que dependen del skin (Tools con Pixel UI),
    `default_size: Vec2` (Timeline usa el "ancho" como alto) y una `key` estable para
    serde/memoria (hoy el título es la identidad).
  - Layouts guardados en desktop con Dafont deben poder leerse en web (`#[serde(other)]` o variante
    en todos los targets y filtrada).
  - Patrón del TabViewer: `mem::take` del `DockState`, juntar `PanelAction`s en un `Vec` y
    aplicarlas después de `DockArea::show_inside`.
  - Ediciones de Artboards/Slices ahora se aplican después de dibujar timeline/color/preview (un
    frame de latencia, igual que Library/Font): aceptado.
- **C. Docking del lado derecho:** la tira de íconos y sus flyouts + el panel de color pasan a
  egui_dock (colapsado a íconos, flotantes, arrastrar). **HECHA**: `PanelCtx { modal, bottom }` es
  la entrada única de `draw_panel` (el `PanelSpec` ya trae `key`, `body`, `min_height` y
  `available`; Lospec vive por "dibujado este frame"). `crates/cs-ui/src/workspace.rs` +
  `workspace/skin.rs` son los únicos que tocan `egui_dock` (=0.21.1, vestido con el skin: un grupo
  de un solo panel va sin barra de pestañas y con franja de agarre al pasar el mouse; pestañas del
  kit; separadores, zonas de soltado y marcos de ventana del skin; todo ajustado a puntos
  enteros). El panel derecho `"palette"` aloja el workspace y la tira de íconos es el grupo
  colapsado: se arrastra un ícono para acoplarlo (pestaña o división) o flotarlo; clic derecho
  Dock/Float; « Collapse to Icons (Color nunca va a la tira); `BrowseLospec` enfoca un Lospec
  acoplado. El acomodo se guarda al cerrar (`chimpsprite.workspace`, con claves estables y carga
  robusta, incluidos NaN/inf) y View → Reset Workspace lo regresa al de fábrica (aún no hay menú
  Window). El ancho de la columna es una preferencia del usuario que solo cambia al arrastrar su
  divisor; el ancho mostrado es `max(preferencia, mínimo de la columna)`, y el mínimo sale de los
  paneles acoplados (el de Color solo si está acoplado). Una pestaña soltada fuera de la columna
  y de las ventanas (en el lienzo) se pone a flotar ahí; las ventanas flotantes no quedan más
  chicas que lo mínimo de lo que llevan; en la pestaña de un modelo, el panel 3D lo hospeda su
  panel fijo y su pestaña del workspace solo dice "Shown in the 3D panel". Lo guardado lleva
  `version` (1; sin el campo, 0) para que la fase D pueda migrar.

  **Probar a mano** (los agentes solo inyectan eventos; esto necesita el mouse real):
  - [ ] Arrastrar un ícono de la tira hacia otro panel: se acopla como pestaña.
  - [ ] Arrastrarlo al borde de un panel: se divide (arriba/abajo/izquierda/derecha).
  - [ ] Arrastrarlo sobre el lienzo: queda como ventana flotante.
  - [ ] Arrastrar una pestaña acoplada (o la franja de un grupo solo) al borde de otro grupo, a su
        centro y al lienzo: lo parte, se junta como pestaña o queda flotando donde se soltó.
  - [ ] Mover y redimensionar la ventana flotante: no se sale de la pantalla y, al soltar su borde,
        vuelve a lo mínimo de lo que lleva si quedó más chica (mientras se arrastra el borde sí
        puede quedar más chica: egui_dock no deja ponerle un mínimo a sus ventanas).
  - [ ] Cerrar una pestaña con su × (también la de una ventana flotante, que no tiene otro botón
        para cerrarse): el panel vuelve a la tira.
  - [ ] « Collapse to Icons en un grupo (y que Color no ofrezca colapsar).
  - [ ] Franja de agarre de un grupo de un solo panel: aparece al pasar el mouse y arrastra el grupo.
  - [ ] Clic derecho en un ícono: Dock y Float.
  - [ ] View → Reset Workspace regresa al acomodo de fábrica.
  - [ ] Cerrar y abrir la app: el acomodo se restaura (acoplados, flotantes y divisiones).
  - [ ] Ancho de la columna: arrastrar su divisor lo cambia y se conserva; al desacoplar un panel
        ancho regresa al ancho elegido.
  - [ ] Buscar en Lospec con Lospec acoplado: la pestaña se enfoca.

  **Para la fase D** (pendientes diferidos de las revisiones de la fase C):
  - Toolbar, timeline y preview al árbol; el lienzo como nodo central.
  - Presets con nombre (de fábrica y del usuario) y menú Window con la lista de paneles.
  - Preview acoplado: sus controles (play/pausa, centrar) dentro del contenido.
  - El modal no bloquea separadores, clics en pestañas ni la tira.
  - El botón « no se alcanza con el teclado (y, sin barra de pestañas, tapa un poco del panel
    mientras se muestra). Las barras de las ventanas flotantes también le dejan su lugar aunque
    ahí no hay «.
  - Las ventanas flotantes siempre muestran barra de pestañas (aun con un solo panel).
  - `egui_dock` no tiene tamaños mínimos por panel: hoy se emulan con el mínimo de la columna, el
    ajuste de divisiones y el de las ventanas (que solo rige al soltar su borde).
  - El tamaño de las ventanas flotantes que el usuario arrastra no se ajusta a puntos enteros.
  - **Una instancia por `PanelId` por frame.** En la fase C se resolvió solo para 3D: en la
    pestaña de un modelo lo hospeda su panel fijo (`"model_panel"`) y su pestaña acoplada o
    flotante dibuja una frase deshabilitada ("Shown in the 3D panel"). La fase D lo generaliza
    (que el registro sepa qué anfitrión manda en cada frame y los demás muestren dónde está).
  - `default_size: Vec2` en la ficha (Timeline usa el "ancho" como alto).
  - Mover la sincronía de `palette_locked` a `draw_color` (hoy la hace `editor_panels` antes y
    después del workspace).
  - `ClosePreview` debe parar la reproducción (hoy solo limpia `view.preview`).
  - La suposición "superficie principal = columna derecha" está regada en `docked`,
    `refresh_tab_bars`, el botón «, `column_min_width`, `drop_target`, `from_saved` y
    `host_width`: decidir con un spike entre un `DockState` para toda la ventana (con el lienzo
    como hoja) y uno por orilla.
  - Migrar lo guardado con `SavedWorkspace::version` (ya existe: sin el campo es 0, lo de antes
    de este arreglo; la fase C escribe 1).
  - Una tira de íconos por orilla.
  - Partir `workspace.rs` (el modelo, el soltado, los ajustes de tamaño y `show`).
  - Pestañas con desbordamiento: flechas del kit en vez del scroll de egui_dock.
  - Menores: Esc no cancela un arrastre; `BrowseLospec` no sube una ventana flotante al frente;
    el soltado sobre una ventana toma la primera, no la de arriba; la ventana flotante puede
    tapar la barra de estado; reordenar/restaurar paneles faltantes en "tidy".
- **D. El resto al árbol:** toolbar, timeline y preview; workspaces de fábrica y del usuario; menú
  Window. Al final, Essentials se ve igual que hoy.

  **Diseño aprobado (2026-11-11), después del spike** (rama `proto-workspace-d`, ejemplo
  `crates/cs-ui/examples/proto_workspace_d.rs` y sus notas `PROTO_WORKSPACE_D_NOTES.md`): el usuario
  eligió **un solo `DockState` para toda la ventana** (variante 1) sobre un árbol por orilla.
  1. **Un árbol para toda la ventana.** El `DockState` abarca el área entre la barra de opciones y
     la barra de estado. El **lienzo** es una hoja protegida al centro (`PanelId::Canvas`, no se
     cierra, no se arrastra, no recibe pestañas, siempre existe una vez): `tidy_canvas` lo arregla
     el mismo frame si algo cae dentro (lo parte al lado del cuadrante), `stop_dragging` corta su
     arrastre, `separator.extra = 0`, `pin_sides` deja los lados en puntos (el lienzo absorbe los
     cambios de tamaño de la ventana) y la zona de destino "lienzo entero" se repinta como la mitad
     que de verdad queda. Se quita la suposición "superficie principal = columna derecha" de todo
     `workspace` y el archivo se parte en `model`, `drop`, `fit` y `show`.
  2. **Toolbar, timeline y preview en el árbol.** Toolbar a la izquierda y timeline abajo (como
     hoy). Preview flota por defecto (como hoy); acoplado, sus controles (play/pausa, centrar)
     viven dentro de su contenido y cerrarlo detiene la reproducción. En la pestaña de un modelo,
     una instancia por `PanelId` por frame (generaliza lo de 3D).
  3. **Tiras de íconos por orilla.** Cualquier grupo pegado a una orilla de la ventana se colapsa
     a íconos con «, en la tira de esa orilla.
  4. **Workspaces con nombre y menú Window.** De fábrica: Essentials (idéntico a hoy, 0 px),
     Animation (timeline alto y preview acoplado), Tiles (Slices y Artboards abiertos) y Fonts
     (FontForge acoplado). Del usuario: New Workspace… / Delete…. Menú Window nuevo con la lista
     de paneles (✓ los abiertos), el submenú Workspace y Reset (se muda de View). Lo guardado pasa
     a la versión 2 y el acomodo de la fase C (versión 1) se migra solo.
  5. **Pendientes de la fase C:** el modal bloquea divisores y pestañas; flechas del kit para
     pestañas desbordadas; tests de barrido de anchos en todos los paneles; medir el costo al
     arrastrar divisores. El resto de la lista "Para la fase D" de arriba, según quepa en el plan.
     **HECHO (tarea 8):**
     - Con un diálogo modal (`is_modal`) nada del workspace se agarra: el `Ui` del `DockArea` va
       apagado (sin atenuarse: se le devuelve su opacidad), así divisores, pestañas, sus × y las
       franjas de agarre de la superficie principal no reciben clics, arrastres ni clics de
       accesibilidad; las ventanas flotantes (que egui_dock arma con su propia `egui::Window`)
       llevan encima "escudos" en su capa (enteras; con Color o el Preview, que un diálogo no
       apaga, solo su marco, barras, franjas y divisores, y su fondo no la mueve: el arrastre del
       área de egui se suelta antes del `DockArea`); lo que el `DockArea` cambiaría (fracciones,
       pestaña al frente, foco, también sin foco) se repone y `on_close` no cierra. La tira, More
       Panels y el « ya iban apagados. El Preview sigue moviéndose de su título, como su ventana de
       antes.
     - Pestañas que no caben (con Pixel UI): las acomoda `kit::tabs::layout` (se achican parejo y,
       si ni así, las flechas del kit a la izquierda del «, con un punto de aire); egui_dock las
       pone igual (`minimum_width` y `LeafNode::scroll`; la última lleva de más lo que falte para
       poder correrlas, bajo las flechas). Ni la última ni su × quedan bajo el «; la que pasa al
       frente se trae a la vista; la rueda las recorre de una en una (un renglón de la rueda por
       pestaña, sumando lo de un trackpad; con Ctrl/Cmd no, es zoom), solo si la barra es lo de
       más arriba bajo el puntero y sin diálogo modal (lo de encima se corre como siempre). Lo
       vacío al final de la barra de una ventana sigue moviéndola, y lo que se suelta ahí va al
       final. Sin Pixel UI, lo de egui_dock.
     - Barrido de anchos (`width_sweep_tests`): cada panel acoplado de su mínimo a 600 de a un punto
       (5249 anchos, ~38 s en debug). Arreglado: la fila Component/Add/Unlink/Build de FontForge
       (los botones caían a medio punto). Tramos conocidos: el timeline de 64 a 126 (su fila de
       botones no se parte; solo "fuera") y Lospec de 364 a 600 (no migrado a `forms`: las filas
       de sus resultados a medio punto; solo "medio punto"). Cada tramo dice la falla que calla y
       el test falla si ya no pasa. Las holguras van nombradas (grupos de controles pegados por
       panel, uno dentro de otro por roles), sin red (Lospec y dafont con su lista ya llegada).
       Arreglado al barrer Lospec con resultados: con la tira de colores justo del ancho, el lugar
       vacío del paginador ensanchaba la lista (Save se salía 1–3 puntos).
     - **Costo de arrastrar un divisor** (`frame_time_while_dragging_a_divider`, a mano): `show`
       por frame, 60 frames moviendo el divisor entre el lienzo y una columna, con Essentials más
       Library (pestaña con Color), Groom (debajo) y Lumen (otra columna). Release, AMD Ryzen 7
       5700G, Windows 11: con Pixel UI media 1.22 ms (mediana 1.22, p95 1.70, máx 2.02); sin él,
       media 0.99 ms (p95 1.37, máx 1.88). En debug, ~16 ms. Incluye la pasada de más cuando un
       frame se descarta.
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
