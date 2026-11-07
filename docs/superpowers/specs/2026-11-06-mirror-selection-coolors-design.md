# Selección espejo completa y pegar paletas de Coolors

Fecha: 2026-11-06 · Estado: propuesto por el agente (sin revisión del usuario todavía) · Rama
`mirror-selection`

Dos cosas chicas en una rama.

## 1. Selección espejo (Mirror Selection)

No existe en Aseprite. Hoy ya hay una versión mínima (PR #24): el botón `icon.mirror` en la barra
de Symmetry (a la derecha de los ejes), seleccionar refleja la forma en todos los ejes prendidos y,
al **trasladar**, cada píxel se corre según su lado de los ejes **recto** (│ ─). Le falta: las
diagonales al mover, escalar, voltear y rotar en espejo, y los casos límite (lo que cruza un eje se
parte y se encima consigo mismo).

### Modelo

- **Grupo**: las simetrías que generan los ejes prendidos (`cs_raster::symmetry::group`, 1, 2, 4 u
  8 operaciones, con su centro en medios píxeles).
- **Piezas**: la selección se parte en componentes conexas (8 vecinos). Cada operación del grupo
  lleva componentes en componentes. Si la selección no es invariante (p. ej. se hizo antes de
  prender la opción, o un reflejo cae fuera del lienzo), **no hay espejo**: se transforma normal.
- **Órbitas**: las componentes que son reflejo una de otra. De cada órbita se elige una
  **representante**: la más cercana al **ancla** (el centro del último gesto de selección, o el
  punto que se agarra con el mouse). Como el grupo está generado por reflejos, las representantes
  quedan del mismo lado de los ejes que el ancla: "el lado que estoy tocando".
  - Sin el ancla (selección hecha de otra forma), se usa el primer píxel de la selección.
- **Piezas libres**: componentes cuyo único reflejo sobre sí misma es la identidad. Las
  representantes libres forman **lo primario**: la caja de transformación (con sus handles) es la
  de lo primario. Cada copia `g` (la imagen de lo primario por `g`) lleva su **propio contenido**
  y se transforma con la transformación conjugada `g ∘ T ∘ g⁻¹`: al trasladar `d` se mueve `g(d)`
  (si lo primario se acerca al eje, el reflejo también), al escalar crece hacia el lado espejo, al
  voltear se voltea en su lugar en espejo y al rotar gira al revés.
- **Piezas sobre los ejes**: componentes que son su propio reflejo (cruzan un eje, están justo
  sobre él o son dos mitades que se tocan). No se parten: quedan simétricas.
  - Con ejes rectos (│ ─) en su estabilizador: se mueven y escalan solo de forma simétrica (su
    caja se re-centra en el eje; el ancho conserva la paridad del centro). No rotan.
  - Con una diagonal o una rotación en su estabilizador: solo se trasladan, con el desplazamiento
    proyectado a lo que deja la pieza simétrica (promedio sobre el estabilizador; p. ej. sobre ╲
    solo en la dirección de la diagonal).
  - Su órbita (si hay más ejes, p. ej. una boca abajo con │ ─) se mueve en espejo como las libres.
- **Solapes**: si al mover las piezas se encima con su reflejo, gana lo primario (se pinta al
  final); lo transparente nunca tapa lo opaco de otra pieza.
- **Varios ejes a la vez**: todo lo anterior es por grupo, así que │ ─ ╲ ╱ juntos dan 8 copias.

### Comportamiento

- Prender **Mirror Selection** (ya existe) y algún eje: seleccionar con cualquier herramienta de
  selección refleja la forma (como hoy).
- Antes de levantar, la caja de transformación muestra **solo lo primario** (el lado tocado).
- Arrastrar dentro de la selección agarra **la pieza bajo el mouse**: si todavía no flota, ese
  lado pasa a ser lo primario; si ya flota y se agarra un reflejo, el arrastre se convierte con
  `g⁻¹`, así lo agarrado sigue al mouse y lo primario se mueve en espejo.
- Flechas (nudge), handles de escala, rotación, Edit → Flip y Edit → Rotate: lo primario con la
  transformación y el resto en espejo. Se confirma en **un paso de undo** (un
  `Command::DrawPixels`, serializable) y la selección queda en el resultado.
- Delete mientras flota en espejo: borra todas las piezas.
- Pegar (Ctrl+V) no usa el espejo: lo pegado no tiene reflejos.

### Implementación

- `cs-raster/src/mirror.rs` (puro, wasm): `MirrorOp` (una simetría: `apply`, `inverse`,
  `linear`), `mirror_plan(mask, axes, center2, anchor2) -> Option<MirrorPlan>` con lo primario,
  las copias y las piezas sobre los ejes (`FixedPiece` con su estabilizador y sus copias), y
  `project(stabilizer, d)`.
- `cs-ui/src/transform/mirror.rs`: lo flotante en espejo (`MirrorFloat`): las fuentes de cada copia
  (traídas al marco de lo primario con `g⁻¹`), el render compuesto y el hueco de toda la
  selección.
- `Editor`: `mirror_anchor` (medios píxeles), caché del plan para `transform_box`.

## 2. Pegar paletas de Coolors

- `cs-io/src/coolors.rs` (puro, sin red, wasm): `parse_palette_text(text) -> Option<PastedPalette>`.
  - Links: `https://coolors.co/palette/264653-2a9d8f-…`, `https://coolors.co/264653-2a9d8f-…`, con
    o sin `https://`/`www.`, con query o `#` al final.
  - Listas de hex: `#264653, 2a9d8f e9c46a`, separadas por guiones, comas, espacios, `;` o saltos;
    con o sin `#`; de 6 u 8 dígitos (`rrggbbaa`), o 3 con `#` (`#fff`).
  - Texto con colores `#rrggbb` mezclados (los exports de Coolors: CSS, SCSS, JSON…): se toman los
    `#…`.
  - Hasta 256 colores; nada más es una paleta (se rechaza texto con palabras que no son colores).
  - Nombre: `Coolors <slug>` o `Pasted <slug>` (slug = los hex con guiones), recortado si es largo;
    la fuente (`Source: https://coolors.co/palette/<slug>`) va en los comentarios del `.gpl`.
- **Dónde**:
  - El campo del link de la pestaña/panel **Lospec** ("Paste a Lospec or Coolors link, or hex
    colors") acepta todo esto: con Fetch (o Enter) aparece la paleta en la lista, con Apply, Append
    y Save como las de Lospec, sin red. Sirve también en la web.
  - **Ctrl+V** con un link de Coolors o una lista de al menos 2 colores en el portapapeles (fuera de
    un campo de texto): abre Palette Presets en la pestaña Lospec con la paleta pegada lista.

## Decisiones que el usuario puede revisar

1. **Componentes conexas (8 vecinos)** deciden qué es una pieza; dos mitades que se tocan en el eje
   son una sola pieza simétrica y ya no se separan en espejo (para separarlas: apagar Mirror).
2. **Lo que cruza un eje no se parte**: se queda centrado (como el "clipping" del espejo de
   Blender). Solo se mueve a lo largo del eje y escala simétrico; no rota.
3. **Lo primario = el lado del ancla** (último gesto de selección o lo que se agarra); la caja
   muestra solo ese lado.
4. **Selección no simétrica → sin espejo** (se transforma normal), en vez de completarla.
5. **Solapes**: gana lo primario.
6. **Coolors**: Ctrl+V global abre el diálogo en vez de aplicar directo (no pisa la paleta sin
   querer); en el campo basta 1 color, por Ctrl+V hacen falta 2 (para no confundir un número).
7. El toggle y su ícono `icon.mirror` ya estaban en el skin: no se dibujó uno nuevo.
