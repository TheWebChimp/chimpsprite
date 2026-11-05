# ChimpCAD: caras completas y la textura por objeto y por cara

Fecha: 2026-11-05 · Estado: diseño aprobado

Pedido del usuario: que las texturas no estén todas juntas en el mismo lugar (hoy cada tipo de
primitiva usa zonas fijas de la textura: todos los cubos comparten las mismas celdas), que se
puedan editar y cambiar de tamaño de manera individual, y poder trabajar a nivel de **cara
completa** de un objeto (por ejemplo, todos los gajos de la tapa redonda del cilindro).

Decisiones tomadas con el usuario:

- **Las dos cosas:** cada objeto tiene su bloque en la textura (se mueve y escala entero) y,
  adentro, cada cara completa se mueve y escala por separado.
- **Caras completas de las primitivas:** cubo 6, plano 1, cilindro 3 (tapa de arriba, tapa de
  abajo, lado curvo entero como una tira), cono 2 (base y lado), esfera 1.
- **Tamaño:** asas en la caja de lo elegido (esquinas y lados; pixeles enteros; Shift =
  proporcional) **y** los puntos libres de cada vértice, como ahora.
- **Sin espacio:** la textura crece sola al siguiente tamaño (64→128→256…) conservando los
  pixeles; un botón **Pack UVs** reacomoda todo sin encimar.

## 1. Caras completas (`part`)

- `cs_core::Face` gana `#[serde(default)] pub part: Option<u32>`: el id de la cara completa a la
  que pertenece dentro de su objeto. `None` (archivos viejos, caras sueltas) = la cara es su
  propia parte. Helper `Mesh::part_faces(f) -> Vec<usize>`: las caras con el mismo `part` que `f`
  (o solo `f` si es `None`).
- **Primitivas** (`crates/cs-3d/src/primitives.rs`): cubo `part` 0..5 (una por cara); plano 0;
  cilindro: lado 0, tapa de arriba 1, tapa de abajo 2; cono: lado 0, base 1; esfera: 0.
- **Topología** (`crates/cs-3d/src/topology.rs`): Subdivide — las caras hijas heredan el `part`
  de la madre. Extrude — la cara extruida conserva su `part`; las caras de los lados nuevos son
  partes nuevas (un `part` nuevo por cada lado: el siguiente id libre del objeto). Merge — las
  caras que quedan conservan el suyo. Delete — sin cambios.
- **Selección:** en modo Face (las vistas y el render de arriba del modo Texture) elegir una cara
  elige su cara completa (`Item::Face` de todas sus caras); Shift suma o quita la cara completa;
  la caja de selección elige caras completas con todas sus caras adentro. El contorno de lo
  elegido es el borde exterior de la cara completa (sin las aristas internas entre sus caras).
- Lo que ya trabaja sobre caras elegidas (Extrude, Delete, pintar limitado a las caras en modo
  Texture, Rotate 90° / Flip) trabaja sobre todas las caras de la parte, sin cambios de lógica
  porque la selección ya las incluye.

## 2. La textura por objeto y por cara

- **Las cajas** (en la textura de abajo, modo Texture):
  - Con un objeto elegido (modo Object en las vistas, o su fila en el outliner) se ve la caja de
    todas sus UVs; con caras completas elegidas, la caja de sus UVs.
  - Dentro de la caja, arrastrar mueve las UVs (como Move hoy, sin Alt con cualquier
    herramienta: ver abajo); las **asas** (4 esquinas y 4 lados, cuadritos como los puntos)
    escalan las UVs respecto del lado opuesto, en pixeles enteros; con Shift, proporcional.
  - Los **puntos** de cada vértice siguen libres (los de la cara elegida), como ahora.
  - Cada gesto, un paso de undo. Mover o escalar UVs no mueve los pixeles (como hoy).
  - Con el lápiz u otras herramientas de pintar: los puntos y las asas se arrastran (como los
    puntos hoy); arrastrar dentro de la caja pinta (mover todo sigue siendo con Move o Alt).
- **Lugar propio al agregar:** un objeto nuevo pone su acomodo de siempre (el de la primitiva)
  en el primer espacio libre de la textura, sin encimarse con las UVs de otros objetos, con 1 px
  de aire (búsqueda en filas, de arriba a la izquierda). Si no cabe, la textura crece al
  siguiente tamaño (`set_texture_size`, que conserva los pixeles) y se acomoda ahí, en el mismo
  paso de undo que el agregar.
- **Duplicar** (objeto o grupo): las UVs de la copia van a un lugar libre como al agregar, y los
  pixeles de su zona se copian al lugar nuevo (la copia se ve igual que el original y se puede
  repintar aparte). Un paso de undo.
- **Pack UVs** (botón en la fila de botones del objeto en modo Texture, junto a Rotate 90° / Flip
  H / Flip V): reacomoda todos los objetos del modelo sin encimar, cada objeto como un bloque
  (sus caras conservan su acomodo relativo), de mayor a menor, **llevando los pixeles** de cada
  bloque a su lugar nuevo. Si dos objetos usaban la misma zona, cada uno se lleva su copia de esos
  pixeles. Si no cabe, crece la textura. Un paso de undo.

## 3. Tests

- cs-core: `part` con serde default; `part_faces`.
- cs-3d: las primitivas con sus partes (cilindro 3, cono 2, esfera 1, cubo 6, plano 1);
  Subdivide hereda, Extrude crea partes nuevas para los lados.
- Selección: clic en un gajo de la tapa elige toda la tapa; la caja de selección por partes.
- UVs: escalar con un asa (pixeles enteros, Shift proporcional), un paso de undo; la caja de un
  objeto mueve todas sus UVs.
- Lugar libre: dos cubos agregados no se enciman; sin espacio, la textura crece y conserva los
  pixeles; duplicar copia los pixeles al lugar nuevo.
- Pack UVs: sin encimes después, los pixeles de cada objeto se ven igual en el render antes y
  después (comparar el render del modelo).
