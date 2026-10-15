# Máscaras — Diseño

Fecha: 2026-10-14
Estado: aprobado (el usuario pidió avanzar sin preguntas)

## Modelo

- `Layer::mask: Option<LayerMask>`: escala de grises (255 muestra, 0 oculta) sobre un rectángulo del
  lienzo (`position`, `width`, `height`, `data`); fuera vale `default`. `enabled` la apaga sin borrarla.
  Una máscara en un grupo afecta a todas sus capas (se multiplica con las de los ancestros).
- `Layer::clip: bool` (máscara de recorte): la capa solo se ve donde su **capa base** tiene píxeles.
  La base es la primera capa de abajo, con el mismo padre, que no recorta (varias capas seguidas
  pueden recortarse a la misma base). Base oculta → las recortadas también. Un grupo como base no
  recorta.
- Las máscaras son estáticas en el tiempo (iguales en todos los frames) y se mueven con
  Canvas Size.

## Comandos

`SetLayerMask { id, mask }`, `DrawMask { layer, pixels: [(x, y, nivel)] }` (agranda la máscara si
hace falta) y `LayerProps::clip`. Operaciones: `new_mask` (mostrar todo / ocultar todo / desde la
selección), `apply_mask` (multiplica el alfa de los cels por la máscara, conserva vínculos) y
`mask_to_selection`.

## Render

`render_frame` y `composite_pixel` multiplican el alfa de cada píxel por la máscara (capa y
ancestros) y, si recorta, por el alfa de la base (sus píxeles × su máscara). `mask_overlay` tiñe de
rojo lo oculto mientras se edita una máscara. `bake_masks` aplica máscaras y recortes a los píxeles.

## Editor y UI

- `edit_mask`: con la máscara seleccionada, las herramientas pintan niveles: luminancia × alfa del
  color (blanco muestra, negro o borrador ocultan). Supr en una selección oculta.
- Timeline: miniatura de la máscara a la derecha del nombre (clic: pintar máscara / capa; tachada si
  está desactivada) y flecha de recorte. Menú contextual y menú **Layer**: Add Layer Mask (Reveal All,
  Hide All, From Selection), Edit Mask, Disable/Enable, Mask to Selection, Apply Mask, Delete Mask,
  Clipping Mask (Ctrl+Alt+G).
- Barra de estado: aviso de que se está editando una máscara.

## Archivos

- `.chimp`: el manifest guarda por artboard `layers: [{index, clip, mask}]` y los datos de cada
  máscara en `masks/<artboard>/<capa>.bin`.
- `.aseprite` no tiene máscaras: al guardar se aplican a los píxeles (`bake_masks`) y se avisa; el
  documento abierto las conserva.

## MCP

`add_mask(layer, hide_all?, reveal?)`, `remove_mask`, `apply_mask`; `set_layer` gana `clip` y
`mask_enabled`; `draw_*`, `fill` y `gradient` ganan `on_mask` para pintar la máscara.
