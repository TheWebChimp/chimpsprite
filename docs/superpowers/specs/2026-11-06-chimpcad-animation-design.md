# ChimpCAD parte 5: animación de objetos (como picoCAD 2)

Fecha: 2026-11-06 · Estado: diseño (decidido por el agente, ver "Decisiones que el usuario puede
revisar")

Referencia: el collage de picoCAD 2 (`.refs/picocad2-reference.png`), el panel "Animation" de la
"gamecube": la Persp arriba a la izquierda, a su derecha la ventanita del bloque elegido
("Rotate": Axis x/y/z, Rotations 0.3, Curve bounce, Ping pong, la papelera) y abajo el timeline:
barra con el nombre del objeto ("lid"), ⏮ ▶ ⏭ y el tiempo "3.85/5s"; debajo, las pistas con los
bloques ("⟳z") y la línea del tiempo actual que se arrastra.

## 1. Qué hace

La pestaña de un modelo gana un tercer modo, **Anim**, junto a **Model** y **Texture** (el grupo
de la derecha del encabezado). En Anim:

- **Arriba**: la Persp (como en modo Texture: orbitar, mover el objetivo, rueda, Home) mostrando
  el modelo **en el tiempo actual**. Un clic elige el objeto que se ve (su pista queda elegida).
- **Barra** (donde Texture tiene la del pincel): los botones ⏮ (al principio) ▶/⏸ (reproducir /
  pausa) ⏭ (al final), el tiempo `1.20/2.00s`, **Length** (ms, la duración de la animación),
  **Loop** y, a la derecha, **Add**: un grupo `Rotate | Move | Scale | Spin | Bob` que agrega un
  bloque a la pista elegida en el tiempo actual.
- **Abajo**: el timeline. A la izquierda la columna de nombres (una pista por fila del outliner:
  objetos y grupos, con su sangría e ícono; lo de adentro de un grupo cerrado no se ve); arriba una
  regla con los segundos; en cada pista, sus bloques (con su ícono de tipo y eje, p. ej. `⟳z`); la
  línea del tiempo actual cruza todo. A la derecha del timeline, el **panel del bloque** elegido.

### Bloques

Un bloque (clip) vive en la pista de un objeto o de un grupo y tiene inicio y duración (ms), un
tipo con sus valores, una curva, ping pong y repeticiones:

| Tipo | Valores | Qué hace en su progreso `v` (0→1) |
|---|---|---|
| Rotate | eje x/y/z, vueltas (milésimas, p. ej. 0.30) | gira `v·vueltas` alrededor del eje que pasa por el origen del nodo |
| Move | desplazamiento x/y/z (1/16 de unidad) | suma `v·desplazamiento` a la posición |
| Scale | factor x/y/z (milésimas) | escala por `1 + v·(factor−1)` desde el origen del nodo |

**Spin** y **Bob** son atajos del botón Add: Spin = Rotate y, 1 vuelta, lineal, toda la
duración; Bob = Move y +0.25, ease in/out, ping pong, 1 s.

**Curvas**: linear, ease in, ease out, ease in/out, bounce, elastic, back, step (salto al final).
Todas valen 0 en 0 y 1 en 1 (bounce/elastic/back pasan de 1 en medio).

**Repeticiones** (1–99) y **ping pong**: el progreso lineal `u` del bloque se reparte en
`repeticiones` ciclos; con ping pong cada ciclo va y vuelve (0→1→0). La curva se aplica a cada
ida. Antes del inicio el bloque no hace nada (`v=0`); después del final se queda en su último
valor (con ping pong, 0; con vueltas enteras, igual que al principio).

**Composición**: en cada nodo, los bloques se aplican en orden de inicio (y de creación si
empiezan a la vez): cada Rotate/Scale multiplica una matriz `A` por la izquierda y cada Move suma
al desplazamiento `d`. El nodo con transform `T` (escala → rotación → posición `p`) queda
`x ↦ p + d + A·(R·S·x)`: la animación pasa **por el origen del nodo y en los ejes de su padre**.
Para girar una tapa sobre su bisagra se mete la tapa en un grupo cuyo origen es la bisagra (como
en picoCAD 2, donde el pivote es el origen del objeto). Los grupos se componen de adentro hacia
afuera, igual que sus transforms.

La animación es parte del documento (se guarda, se deshace) y su vista es de la UI: el tiempo
actual, si se reproduce y el bloque elegido no se guardan ni pasan por undo.

### Timeline

- Un clic en la regla (o arrastrar en ella) mueve el tiempo actual (y pausa).
- Un clic en una pista vacía la elige (y elige su objeto o grupo en el outliner y las vistas).
- Un clic en un bloque lo elige; arrastrarlo lo mueve; arrastrar sus bordes (3 px) lo estira.
  Todo se ajusta a 50 ms (Shift: a 10 ms), queda dentro de `[0, Length]` y dura al menos 50 ms.
  Un gesto = un paso de undo (vista previa mientras se arrastra, Esc cancela).
- Bloques que se pisan en la misma pista se reparten en carriles (la pista se hace más alta).
- Supr con el puntero sobre el timeline borra el bloque elegido.
- La rueda sobre el timeline lo desplaza verticalmente si no cabe.

### Panel del bloque

Como la ventanita de picoCAD 2, con componentes del kit: título (el tipo), **Axis** x/y/z o los
tres valores (X/Y/Z), **Curve** (select), **Ping pong**, **Repeat**, **Start** y **Length** (ms)
y **Delete**. Cada cambio es un paso de undo. Sin bloque elegido: "No block" y una ayuda corta.

### Reproducción

▶ avanza el tiempo con el reloj de egui (repinta cada frame mientras corre); con Loop vuelve a 0
al pasar Length, sin Loop se detiene al final. Enter (con el puntero en el lienzo, sin campo de
texto con foco) reproduce/pausa en modo Anim. Las vistas orto del modo Model muestran siempre la
pose de reposo (se edita sobre ella); la pose animada se ve en la Persp del modo Anim y en los
exports.

## 2. Export

**Sheet settings** y **GIF settings** ganan un select **Motion**:

- `Turntable` (como hasta ahora): `Frames` ángulos alrededor de Y.
- `Animation`: la cámara fija (sin giro) y `Frames` muestras de la animación en `t = i·Length/Frames`.
- `Both`: las mismas muestras y, a la vez, una vuelta completa del turntable a lo largo de la
  animación.

En Animation/Both la duración de cada frame es `Length/Frames` (redondeada): en Sheet settings
cuando la textura no está animada, y en GIF settings el campo Frame (ms) se ignora y muestra la
calculada (deshabilitado). Las vistas previas muestran la animación.

El giro del turntable pasa a ser un giro del **modelo entero** alrededor del eje vertical que pasa
por el objetivo de la cámara, aplicado después de todo (transforms, grupos y animación). Antes se
sumaba a la rotación Y de cada nodo de la raíz (cada objeto giraba sobre su propio origen); con
objetos en el origen es lo mismo, con objetos corridos ahora orbitan juntos como en picoCAD.

## 3. Motor

- `cs_core::model`: `Clip { start_ms, duration_ms, motion: ClipMotion, curve: Curve, ping_pong,
  repeat }`, `ClipMotion::{Rotate { axis, turns }, Move { offset }, Scale { factor }}`,
  `Axis3`, `Curve`. `Object3D.anim: Vec<Clip>` y `Group3D.anim: Vec<Clip>` (serde default: los
  archivos viejos abren sin animación); `Model3D.anim_ms: u32` (serde default 2000). Todo entero
  (el modelo es `Hash`/`Eq`).
- `cs_3d::anim` (nuevo): `curve_value(curve, f)`, `clip_progress(clip, t)`, `NodePose` (matriz y
  desplazamiento), `node_pose(clips, t)`, `Pose { objects, groups }`, `pose_at(model, t)`,
  `posed_to_world(model, pose, o, p)` y los ayudantes del timeline (`clip_lanes`, `snap_ms`).
- `cs_3d::RenderInput` gana `time_ms: Option<u32>`: con `Some`, el render (y el pick y el contorno
  de caras) usa la pose de ese tiempo. `extra_yaw` pasa a ser el giro del modelo entero.
- `cs_render`: `BakeOptions.motion: Motion` (`Turntable` por defecto) y
  `BakeOptions::sample(i) -> (yaw, Option<t>)`; `model_render_at(…, time)` y
  `model_pick_at(…)` para el modo Anim.

## 4. UI

- `editor::CadMode::Anim`; `ModelEdit.anim: AnimView { time_ms, playing, looping, clip, … }`.
- `editor/anim_ops.rs` (nuevo): agregar, cambiar, mover/estirar (vista previa + un paso al
  soltar), borrar bloques; Length; elegir pista/bloque; reloj de reproducción.
- `canvas/anim_mode.rs` (nuevo): el área del modo Anim (Persp, barra, timeline, panel del
  bloque), dibujado con piezas y componentes del kit (grupo de íconos del timeline para ⏮ ▶ ⏭,
  steppers, checks, select, botones).

## 5. Fuera de alcance (anotado en anim-minors.md)

- Keyframes libres por propiedad (picoCAD 2 también usa bloques).
- Vistas orto animadas y gizmos sobre la pose animada.
- Copiar/pegar bloques, multi-selección de bloques, zoom horizontal del timeline.
- Textura animada sincronizada con el tiempo de la animación.

## Decisiones que el usuario puede revisar

1. **Modo Anim** en el encabezado (Model | Texture | Anim) en lugar de un panel flotante: sigue
   el patrón de Texture (Persp arriba, cosa propia abajo) y el layout del picoCAD 2.
2. **Una pista por fila del outliner** (objetos y grupos), no solo la del objeto elegido como en
   picoCAD 2: se ve toda la animación junta. Los bloques que se pisan se apilan en carriles.
3. **Bloques, no keyframes**: Rotate/Move/Scale con curva, ping pong y repeticiones; Spin y Bob
   como atajos.
4. **Pivote = origen del nodo**, en los ejes del padre; para bisagras se usa un grupo.
5. **Después de un bloque se queda su último valor** (así "abrir" y luego "cerrar" son dos
   bloques con vueltas opuestas, como en la gamecube de la referencia).
6. **Length por defecto 2 s**; ajuste a 50 ms (Shift 10 ms).
7. **Turntable = giro del modelo entero** alrededor del eje vertical del objetivo de la cámara
   (antes: cada nodo de la raíz sobre su origen).
8. **Motion en los exports**: Turntable / Animation / Both, con `Frames` como número de muestras y
   la duración por frame calculada en Animation/Both.
9. Las vistas orto y los gizmos del modo Model siempre muestran la pose de reposo.
