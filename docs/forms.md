# forms: paneles responsivos

`crates/cs-ui/src/forms.rs` arma los paneles con filas, columnas y grillas (flexbox de taffy, vía
egui_taffy) en vez de coordenadas absolutas: el panel se acomoda al ancho que le toque. Solo ese
módulo hace `use egui_taffy`; los paneles usan lo que reexporta (`Tui`, `TuiBuilderLogic`, `Style`).
Todo va en puntos enteros (con la escala 2 de la UI, un punto es un pixel del skin).

## Estilos (cada uno devuelve un `Style`)

- Contenedores: `column(gap)`, `row(gap)` (si no cabe, parte), `row_nowrap`, `row_end` (a la
  derecha), `row_lines` (gap solo entre líneas; para filas con `push_right`), `group(gap)` (etiqueta
  y campo que van juntos).
- Hojas: `fixed(w, h)`, `grow(min, basis, max, h)` (crece con lo que sobre), `grow_auto(min, basis)`
  (alto de lo que tiene adentro), `share(min, h)` (partes iguales), `natural()` (mide lo que ocupó).
- Modificadores: `weight`, `full_height`, `fill_height(_auto)`, `min_height`, `air_above/below/right`,
  `push_right`.
- Entrada: `form(ui, id, gap, |tui| ..)` y `form_in_flow` (una forma entre widgets de egui).
  El `id` de una forma es global (no se combina con `ui.id()`): dos formas con el mismo `id` en
  un frame comparten el estado de taffy y, anidadas, truenan; las que se repiten (una por
  elemento de una lista) llevan algo que no se repita, como su índice.
  `canvas` es una hoja que no se mide (vistas previas y mapas que llenan lo que les toca);
  `label` dibuja una etiqueta en la fila de 17 de los controles del kit y `label_width` mide el
  ancho que necesita.

## Ejemplo

```rust
forms::form(ui, "mi_panel", 4.0, |tui| {
    tui.style(forms::row(4.0)).add(|tui| {
        tui.style(forms::fixed(37.0, 17.0)).ui(|ui| forms::label(ui, "Family", 37.0));
        tui.style(forms::grow(60.0, 160.0, Some(160.0), 17.0)).ui(|ui| {
            let w = ui.max_rect().width(); // el ancho que le tocó
            kit::input::text_input(ui, &skin, &mut text, w, "Family name", "Font");
        });
    });
});
```

## Receta de migración de un panel

1. La constante de ancho pasa a `REFERENCE_WIDTH` (con ella el dock abre el panel) y se agrega
   `MIN_WIDTH`. Dentro del panel todo sale de `ui.available_width()`, nunca de `REFERENCE_WIDTH`.
2. Las coordenadas a mano se vuelven `form` con filas y columnas; en `REFERENCE_WIDTH` cada control
   cae donde caía (los tests de layout existentes no se tocan). Lo dibujado libre va en `canvas` o en
   una hoja con `ui.max_rect()`.
3. Más angosto parte las filas, más ancho crecen los `grow`; bajo `MIN_WIDTH` hay scroll vertical.
4. Capturas antes y después al ancho de referencia: 0 pixeles distintos.

## Arnés de pruebas

`forms::testing` es solo `#[cfg(test)]`. `forms::testing::run_at(width, |ui| ..)` dibuja en un
`Ui` de ese ancho y devuelve un `Probe` con los rects de los controles (`rect(name)`), los que
están deshabilitados, los textos y las imágenes. Cada panel lo corre a varios anchos, entre ellos
`MIN_WIDTH`, `REFERENCE_WIDTH` y 600, con `assert_whole_px`, `assert_within(width)` y
`assert_no_overlap`. `forms::testing::frame_at(ctx, width, |ui| ..)` es un solo frame en un
contexto que ya existe (con las fuentes y el estilo que tenga, p. ej. los de la app con
`pixel.update`): así un barrido reusa el contexto de un ancho al siguiente. Cada entrada de
`Probe::widgets` lleva su nombre, su rect y qué es (botón, campo, etiqueta…).

`width_sweep_tests` barre cada panel acoplado (como lo hospeda el workspace, con Pixel UI y la
escala de la app) de su mínimo a 600 de a un punto (unos 35–40 s en debug), sin red (las listas de
Lospec y dafont ya llegadas). Lo que va encimado o a medio punto a propósito está ahí, nombrado y
con su porqué: los grupos de controles pegados que comparten su borde (`GLUED`, por panel y por
nombre), uno dentro de otro solo por roles (un botón con los suyos adentro, un campo con el cuadro
de su texto; nunca el mismo rect), el botón Swap colors de la toolbar y la y del cuadro del texto de
un campo. Lo que un panel todavía no cumple en un tramo va en su `KNOWN_GAPS` con el panel, el tramo
y la clase de falla que calla (medio punto, fuera o encimado); si deja de pasar en todo su tramo, el
test falla para que se borre.

## Rarezas de taffy 0.9

- `push_right` en una fila con gap pierde el gap en su línea: usa `row_lines` (y `air_right`).
- Una etiqueta que se parte dentro de una celda de fila se mide al ancho mínimo de su contenido.
- `grow_auto` y `natural` miden con lo del frame anterior; el primer frame sale más alto, así que
  lo dibujado libre se pinta solo cuando ya se ve.
- Un ancho justo (como `MIN_WIDTH` exacto) puede dar alturas de fila raras: mide antes de fijarlo.
