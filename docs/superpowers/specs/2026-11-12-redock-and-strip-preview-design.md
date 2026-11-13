# Regresar paneles a una orilla, y la vista previa al reordenar la tira

Pedido del usuario (2026-11-12), probando la pastilla y el reordenar:
1. "No logro regresar esto [Color, flotando] a la lateral, está imposible."
2. "En el reorganizar de los íconos, ¿podemos mostrar fantasma de dónde caería y cómo se vería?"
   (eligió la vista previa completa).

## Diagnóstico (rama `color-redock`, `diag_color_redock.rs` en 4036dbb, hoy `color_redock.rs`; el informe fue una nota local, aquí va resumido)

1. **Ningún destino regresa una columna de alto completo.** egui_dock solo parte o junta en la
   hoja bajo el puntero (`leaf.rs:1433`, `move_tab`) y `drop::drop_target` también solo mira hojas.
   Sin Color el árbol es `H[Tools | V[Canvas / Timeline]]` y nada parte la pila entera: Color cae
   junto al lienzo solo o al timeline solo (y el timeline deja de verse "debajo del lienzo").
2. **La ventana arrastrada tapa sus destinos.** Con el puntero sobre su propia ventana, egui_dock
   lo toma como "su mismo grupo" y la vuelve a flotar (`show/mod.rs:200-216`); al arrastrar la
   pestaña la ventana no se mueve. A 960×505 la ventana a la derecha tapa la tira y el cuarto
   derecho del lienzo.
3. Casi todo el lienzo es "ventana nueva" (solo el 25 % de cada orilla parte) y la ventana nueva
   aparece en un lugar fijo (~360,105), no donde se soltó.
4. Al partir el lienzo con egui_dock, Color queda con la mitad del lienzo (463) y `show.rs:820-838`
   lo guarda como su ancho natural.
5. Aparte: soltar a la derecha de Tools truena en debug (egui `placer.rs:114` "Negative child size").
6. No hay salida sin arrastre: la pestaña no tiene menú; Window → Color solo la trae al frente;
   Reset borra todo lo demás.

## Diseño

### A. Destino de orilla (columna o fila de lado a lado)

- **Franja de orilla:** `EDGE_BAND` = 16 pt hacia adentro del área del workspace, pegada a cada
  orilla (junto a su tira). Al arrastrar una pestaña (camino de egui_dock) o un ícono de la tira
  (camino de `drop_target`) con el puntero en ella, el destino es **la orilla entera**: una columna de
  alto completo a la izquierda/derecha, o una fila de ancho completo arriba/abajo, junto a la tira.
- **Sobre una tira:** un panel que va a la tira cae en ella en esa posición (como un ícono, con la
  vista previa de C). Uno que no va a la tira (Color) toma la orilla de esa tira como en la franja.
- **`DockTarget::Edge(Edge)`:** parte la raíz de la superficie principal hacia esa orilla
  (`H[todo | panel]` a la derecha, etc.); el lado nuevo, en puntos, con el ancho (o alto) de
  siempre del panel: Color, `view.palette_width` (o su natural); los demás, su `reference_width`
  (alto: `default_size`). Soltar Color en la orilla derecha de un Essentials sin Color deja
  exactamente el árbol y los rects de Essentials.
- **Zona resaltada:** mientras el puntero está en la franja (o Color sobre la tira), se pinta la zona
  de la columna/fila que quedaría (de alto/ancho completo, con ese ancho), en puntos enteros; la de
  egui_dock no se ve en ese momento.
- **Ancho al partir el lienzo (causa 4):** cuando egui_dock parte el lienzo para un panel, el lado
  nuevo queda con el ancho de siempre del panel (no la mitad del lienzo) y no se guarda como natural.
- La ventana nueva de un "soltar en nada" queda donde se soltó (causa 3), si sale barato; si no, se
  anota.

### B. La ventana arrastrada no se tapa a sí misma

- Mientras se arrastra la **única** pestaña de una ventana flotante, esa ventana se ve translúcida
  (opacidad 0.4) y no cuenta como destino: al soltar sobre ella, cuenta lo que hay debajo
  (otra hoja, la franja de orilla, una tira, el lienzo). Las zonas que se pintan son las de lo de
  debajo.

### C. Vista previa al reordenar la tira

- Mientras se arrastra un ícono sobre una tira (la suya u otra), la tira se dibuja **como quedaría**:
  el hueco de donde salió se cierra, los demás se recorren y en el lugar de destino va el botón del
  ícono **semitransparente** (opacidad 0.5). La rayita de inserción ya no se pinta. La pastilla
  sigue al puntero. Al soltar, nada salta (la tira queda como la vista previa).
- "More Panels" y los botones de Timeline/Preview no se mueven; si el ícono cae entre los ocultos,
  la vista previa lo muestra en "More Panels" (el botón resaltado) en vez de un hueco.

### D. "Dock" y el crash

- **Menú de la pestaña de una ventana flotante:** clic derecho → "Dock" la regresa a su lugar de
  fábrica (Color: la columna derecha = `DockTarget::Edge(Right)` con su ancho; Tools: junto al
  lienzo a la izquierda; Timeline: debajo del lienzo; los del dock: su tira) — lo mismo que hace
  "Dock" desde la tira (`dock_from_strip`) donde ya existe. Preview no lleva "Dock" (flota por
  diseño).
- **El crash en debug** al soltar a la derecha de Tools: reproducir con un test, encontrar la causa
  (tamaño negativo de un hijo en el `Viewer` o en `fit`) y arreglarla.

## Lo que quedó

Lo de arriba es el diseño de partida; lo construido difiere en esto (manda esta sección).

### A. Orilla

- `Edge` no parte la raíz: baja por `edge_node` y así queda exactamente el árbol de Essentials.
  `Edge(Left)` pone la columna en la orilla, con la toolbar entre ella y el lienzo.
- El ancho de siempre lo define `SidePoints::preferred`: Color, `palette + sep/2`; la toolbar, su
  caja; el timeline, `sides.timeline`; los demás, `panel_reference`; tope del 45 % del split.
- `palette_width` (y el ancho natural, y lo que escribe arrastrar el divisor) es solo de Color en
  la columna; otro panel en la orilla derecha va de su ancho de siempre.
- La zona pintada se calcula acoplando en una copia y corriendo el mismo `pin_sides`.
- La barra de pestañas de un grupo le gana a la franja; la franja no rige sobre el propio grupo
  acoplado. Una ventana encima gana, salvo la que se ve a través.
- Causa 3 arreglada: lo que vuelve a flotar en una ventana nueva queda donde se soltó; juntarse a
  una ventana existente no la mueve.

### B. Ventana que se ve a través

- Si no hay nada debajo, flota donde se soltó. Sobre su propia ventana, `max_preference_time`
  va en 0. Una pulsación larga o un deslizón de menos de 30/6 pt no hacen nada (el umbral de
  egui_dock, `DOCK_DRAG_START`).

### C. Vista previa de la tira

- El hueco se cierra durante todo el arrastre (también con el puntero sobre el árbol); dónde cae
  se mide con los botones en reposo; el flyout abierto sigue a su botón; el menú de "More Panels"
  solo lista los propios de la tira; la rayita de inserción se quitó.
- Excepción: si una tira de orilla se queda vacía, desaparece al soltar (el árbol crece).

### D. "Dock" y el crash

- Causa real del crash: egui_dock dibuja el frame de soltar con el split a la mitad antes de
  `fit_splits`; `Viewer::squeeze` pone el panel en su mínimo ese frame y pide `request_discard`.
- Con Pixel UI, el "Dock" de la toolbar sola sale de su franja de agarre.
- Los paneles del dock regresan a la tira derecha aunque su tira esté en otra orilla.
- El menú se arma después de `DockArea::show_inside` (por el `allowed_in_windows` con estado de B).

## Pruebas

- Los `diag_…` de `diag_color_redock.rs` se convierten en tests del comportamiento nuevo (sin
  `#[ignore]`): Color flotando → soltar en la franja derecha / sobre la tira derecha → Essentials
  idéntico (árbol y rects); a 1200×800 y a 960×505.
- Franja de orilla en las cuatro orillas con una pestaña y con un ícono de la tira; la zona pintada
  es la de la columna/fila que queda.
- Soltar sobre la propia ventana (la única pestaña): cuenta lo de debajo; la ventana se pinta
  translúcida durante el arrastre y opaca después.
- Ancho al partir el lienzo: el del panel, no la mitad.
- "Dock" de la pestaña flotante para Color, Tools, Timeline y un panel del dock.
- El crash: test que lo reproduce.
- Vista previa de la tira: durante el arrastre, los rects de los botones son los del orden final y el
  del destino va con opacidad 0.5; al soltar, los rects no cambian.
- Essentials en reposo: 0 px contra main (escala 2, con y sin Pixel UI).
- Alcance real: las orillas de arriba y abajo y el modo sin Pixel UI tienen tests de modelo, no de
  la app; "Dock" de Tools, Timeline y Library solo se prueba a 1200×800.

## Decisiones que el usuario puede revisar

- La franja de orilla mide 16 pt.
- Un panel que va a la tira, soltado sobre una tira, entra a la tira (no se acopla en columna); para
  una columna de ese lado se suelta en la franja, justo antes de la tira.
- "Dock" regresa al lugar de fábrica, no al último lugar donde estuvo.
- La ventana arrastrada va al 40 % y el fantasma de la tira al 50 %.
- Color soltado junto a la toolbar queda en su mínimo (136).
- La franja de 16 pt también cubre los 16 pt izquierdos de la toolbar y las franjas de agarre de
  los grupos de arriba.
- Escape suelta los íconos de la tira (aplica la vista previa) pero cancela las pestañas.
- Con un flyout abierto, su pestaña opaca tapa al ícono semitransparente durante el arrastre.
