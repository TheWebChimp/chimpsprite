# La pastilla al arrastrar y reordenar la tira

Pedido del usuario (2026-11-12), probando la fase E:
1. "Cuando mueves algo, la pestañita moviéndose debería verse así": una pastilla con el nombre del
   panel (imagen de referencia, 1x: 34×18 para "Color").
2. "Deberías poder reordenar los íconos [de la tira de paneles] a tu gusto."

## 1. La pastilla al arrastrar (Pixel UI)

- **Pieza `tab.drag`** en `assets/ui/skin.aseprite` (ya agregada, slice en (276, 901), 5×18, centro
  de nueve partes `(2, 2, 1, 14)`): contorno `canvas` `#bda499` con las esquinas recortadas, aro
  `highlight` `#f1ebdb` y relleno `background` `#ddcebf`, copiada de la imagen del usuario. Se estira a
  lo ancho; el alto es fijo (18).
- **Componente del kit** `kit::tabs::drag_tag_shapes(ui, skin, min: Pos2, label) -> Vec<Shape>` (y
  `drag_tag_size(ui, label) -> Vec2`): la pieza estirada al ancho del texto + 6 de aire a cada lado
  (ancho = texto + 12), 18 de alto, el texto en la fuente de la app y el color `text` del skin, con
  sus mayúsculas a 6 de arriba (como la imagen: "Color" mide 34×18). En puntos enteros. Si el skin
  no trae la pieza, se dibuja con esos tres colores (como las otras piezas con respaldo).
- **Quién la usa** (las tres sombras de arrastre, en la capa `Tooltip` como hoy, corridas lo que se
  arrastró, con su esquina donde estaba la de lo agarrado):
  - la pestaña acoplada que se arrastra (`workspace/skin.rs`, `paint_tabs` → `ghost`);
  - la franja de agarre de un grupo sin barra (mismo `ghost`);
  - el ícono de la tira (`dock.rs`, `ghost`): ya no el botón con la pestaña, solo la pastilla.
- **Sin Pixel UI** no cambia (el título en un recuadro; las pestañas, las de egui_dock).
- El catálogo del kit (UI Catalog) muestra la pastilla junto a las pestañas.

## 2. Reordenar la tira

- Arrastrar un ícono **a lo largo de su propia tira** lo reordena: mientras está encima, una
  **línea de inserción** marca entre qué íconos caerá (2 de grueso, el color de selección del skin
  `text_selection` — sin skin, el de selección de egui —, a lo ancho del botón, centrada en el
  hueco entre botones; antes del primero o después del último, en el borde). Al soltar, se
  reacomoda. Funciona igual en las cuatro tiras (a lo alto en las de los lados, a lo ancho en las
  de arriba y abajo).
- **Sobre otra tira**: además del resaltado de la tira (fase E), la misma línea; al soltar, el ícono
  cae en esa posición (ya no al final).
- Soltarlo en su mismo lugar no cambia nada. Soltar más allá del último visible (o sobre "More
  Panels") lo pone al final. Los íconos dentro de "More Panels" no se arrastran; los botones de
  Timeline y Preview no se reordenan ni cuentan como posición.
- El orden se guarda solo: las tiras ya son listas ordenadas en lo guardado
  (`chimpsprite.workspace`); no cambia `SAVED_VERSION`.
- Por dentro: `StripRequest::Reorder(panel, index)`; `Workspace::to_strip_at(id, edge, index:
  Option<usize>)` (o una función nueva `move_in_strip`) inserta en `index`; el índice se calcula de
  los rects de los botones visibles (función pura, con tests).

## Pruebas

- Pura: el índice de inserción para un puntero (antes del primero, entre dos, después del último,
  tiras verticales y horizontales, con "More Panels").
- `Workspace`: reordenar dentro de una tira y pasar a otra en una posición; lo guardado se lee igual.
- App (layout_tests): arrastrar un ícono dentro de su tira lo reordena; mientras se arrastra se pinta
  la línea en el hueco correcto; sobre otra tira, línea + resaltado y al soltar cae en su posición.
- La sombra de arrastre de una pestaña, de una franja y de un ícono es la pieza `tab.drag` con el
  nombre del panel (y ninguna pieza de pestaña en la capa del fantasma); su tamaño para "Color" es
  34×18.
- Essentials en reposo: 0 px contra main (capturas a escala 2, con y sin Pixel UI).

## Decisiones que el usuario puede revisar

- La pastilla va donde estaba lo agarrado (su esquina de arriba a la izquierda, corrida lo
  arrastrado), no centrada en el puntero.
- Sin Pixel UI, la sombra no cambia.
- La línea de inserción es del color de selección (como la zona de soltar), no una pieza nueva.
