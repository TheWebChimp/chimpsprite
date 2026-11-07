# Movie clips, parte 3: tweens de objetos (Classic Tween, como en Flash)

Fecha: 2026-11-06 · Estado: propuesto por el agente (sin revisión del usuario todavía)

## Objetivo

Hoy cada objeto de una capa de objetos (una instancia de un símbolo) tiene keys que valen hasta la
siguiente: se mueve a saltos. Flash/Animate tiene el **Classic Tween**: entre dos keyframes de la
misma instancia, los frames de en medio se calculan solos (posición, escala, rotación, alfa y
tinte), con un **ease** de −100 a 100. En el timeline, el tramo se pinta de otro color con una
flecha de un keyframe al otro; si al tramo le falta el keyframe final, la línea sale punteada.

Esta parte agrega:

- **Transformación por key**: escala X/Y, rotación y tinte (además de posición, flips y opacidad).
- **Tweens entre keys**: un tween empieza en una key y llega a la siguiente; interpola posición,
  escala, rotación, opacidad y tinte con un ease.
- **Timeline**: clic derecho → **Create Tween** / **Remove Tween**; el tramo con tween se ve con
  fondo y flecha (punteada si está roto).
- **Inspector Object**: Scale, Rotation, Tint y, si la key vigente tiene tween, su Ease.
- **Render pixel art** en el lienzo, la reproducción y todos los exports, sin antialias.
- **MCP**: `set_object_key`/`set_instance_key` con escala, rotación y tinte; `set_tween` y
  `remove_tween`.

Fuera de alcance (anotado en `clips3-minors.md`):

- Motion Tween de Flash (tween propio con keys de propiedad y Motion Editor), guías de
  movimiento, "Orient to path", easing personalizado con curva, shape tweens;
- punto de transformación movible (el pivote es siempre el centro del símbolo);
- sesgo (skew), brillo y "Advanced" de Color Effect;
- pintar en su lugar a través de una transformación (con escala o rotación se edita aislado).

## 1. Modelo (`cs-core`)

`InstanceKey` gana cuatro campos, todos con `serde(default)` y sin escribirse si valen lo de
siempre (los archivos viejos cargan igual y los nuevos sin transformación se escriben igual):

```rust
pub struct InstanceKey {
    // …frame, source, position, flip_h, flip_v, opacity, timing…
    /// Escala en milésimas (1000 = 100 %), en X y en Y. Mínimo 1.
    pub scale: (i32, i32),
    /// Rotación en grados, horaria, sin envolver: 720 son dos vueltas (así un tween gira de más,
    /// como el "Rotate CW × 2" de Flash).
    pub rotation: i32,
    /// Tinte de Flash (Color Effect → Tint): mezcla el color del símbolo hacia `color`.
    pub tint: Option<Tint>,
    /// Classic tween desde esta key hasta la siguiente.
    pub tween: Option<Tween>,
}

pub struct Tint { pub color: Rgba /* sin alfa */, pub amount: u8 /* 0..=255, como la opacidad */ }

pub struct Tween { pub ease: Ease }

pub enum Ease {
    /// El ease de Flash: −100 (ease in, acelera) … 0 (lineal) … 100 (ease out, frena).
    /// `p(t) = t + e·t·(1 − t)` con `e = amount / 100` (cuadrático, como Flash).
    Classic(i8),
    /// Una curva del modo Anim de ChimpCAD (`Curve`: ease in/out/in-out, bounce, elastic, back,
    /// step) con intensidad 0..=100: `p(t) = t + (curve(t) − t) · amount / 100`.
    Curve { curve: Curve, amount: u8 },
}
```

- La evaluación de las curvas (`curve_value`, hoy en `cs_3d::anim`) pasa a `cs_core` como
  `Curve::value(f)`; `cs_3d::anim::curve_value` la llama (sin cambios para ChimpCAD).
- **Pose**: `InstanceTrack::pose_at(frame) -> Option<InstancePose>` da lo que se ve en un frame:

  ```rust
  pub struct InstancePose {
      /// La key vigente (fuente, flips y tiempo del símbolo salen de ella).
      pub key: InstanceKey,
      pub position: (i32, i32),
      /// 1.0 = 100 %.
      pub scale: (f32, f32),
      /// Grados, horaria.
      pub rotation: f32,
      pub opacity: u8,
      pub tint: Option<Tint>,
  }
  ```

  Sin tween (o con tween roto) es la key vigente tal cual. Con tween activo, en el frame `f` del
  tramo `[a, b)` (de la key `A` a la siguiente `B`) se toma `t = (f − a) / (b − a)`, `p = ease(t)`
  y cada valor es `A + (B − A)·p`:
  - posición: redondeada al píxel (`round`, la mitad se aleja de cero), como el "Snap" de Flash;
  - escala y rotación: sin redondear (el tamaño se redondea al dibujar);
  - opacidad: redondeada y recortada a 0..=255 (bounce/back pueden pasarse);
  - tinte: color y cantidad por canal, redondeados; una key sin tinte cuenta como el color de la
    otra con cantidad 0; con cantidad 0 el resultado es `None`.
- **Tween roto**: si no hay key siguiente o la siguiente muestra otra fuente (o ninguna), el tramo
  no se interpola (vale la key, como sin tween). El timeline lo dibuja punteado. Así Flash avisa
  de un tween sin keyframe final o entre símbolos distintos.
- `InstanceTrack::keyframe_at(frame) -> Option<InstanceKey>`: la key que tendría `frame` si se
  volviera keyframe. Si ya lo es, ella; si no, la vigente con `frame` y los valores de la pose
  (escala en milésimas y rotación en grados, redondeadas) y el mismo tween. Insertar una key en
  medio de un tween lo parte en dos tramos que siguen pasando por los mismos lugares (como
  "Insert Keyframe" en un classic tween de Flash).
- `InstanceTrack::tween_span(frame) -> Option<TweenSpan>`: el tramo con tween que contiene a
  `frame` (`start`, `end` = frame de la key siguiente o `None` si no hay, y si está roto). Lo usan
  el timeline y el inspector.
- `InstanceKey::is_plain()`: sin escala, sin rotación (múltiplo de 360) y sin flips; con
  `InstancePose::is_plain()` lo mismo para la pose (para Edit in Place).

## 2. Render (`cs-render`)

`materialize` sigue igual por fuera. Lo que dibuja un objeto en un frame (`instance_image`) usa
`pose_at`:

1. Renderiza el símbolo en `source_frame(pose.key, …)` (el tiempo del símbolo sigue corriendo
   durante el tween, desde la key vigente).
2. Aplica los flips.
3. **Escala** con vecino más cercano (`cs_raster::scale_nearest`) al tamaño
   `max(1, round(w·sx)) × max(1, round(h·sy))`.
4. **Rota** con **RotSprite** (`cs_raster::rotsprite`, el mismo de la selección; los múltiplos de
   90° son exactos y los bloques muy grandes usan Fast).
5. El centro del resultado queda en el centro del símbolo sin transformar
   (`position + (w, h) / 2`), con el mismo redondeo que la selección (`div_euclid`).
6. **Tinte**: `rgb' = rgb + (tinte − rgb)·amount / 255` redondeado; el alfa no cambia.
7. Se recorta a lo opaco; la opacidad de la pose va al cel (o a la composición de objetos), como
   hoy la de la key.

Sin escala, rotación ni tinte el resultado es exactamente el de antes (mismo camino de código). Todo
es una función pura de `(documento, frame)`: el lienzo, la reproducción, el onion skin, los
exports (PNG, GIF, sheets, motores, `.aseprite` horneado) y el `render` del MCP dan lo mismo.

`object_image` (tocar el objeto con Move, su recuadro) usa la pose: se toca y se enmarca lo que se
ve, también rotado o en medio de un tween.

`cs-render` pasa a depender de `cs-raster` (los dos son motor, sin egui, compatibles con wasm).

## 3. Comandos (`cs-commands`)

No hay primitivas nuevas: todo es `Command::SetObjects` (serializable, un paso de undo).

- `update_objects_key` y `move_objects` (y Alt+arrastrar) copian `keyframe_at(frame)` en lugar de
  la key vigente: mover o editar un objeto en medio de un tween crea ahí una key con lo que se ve
  (no salta a la key de antes).
- Nuevas en `ops`:
  - `create_tweens(sprite, layer, objects, frame)`: a la key vigente en `frame` de cada objeto le
    pone `Tween::default()` (Classic 0) si no tiene y hay una key siguiente;
  - `remove_tweens(sprite, layer, objects, frame)`: se lo quita;
  - `update_tweens(sprite, layer, objects, frame, f)`: cambia con `f` el tween de la key vigente
    (el Ease del inspector; no crea keyframes);
  - `can_create_tween(track, frame)` / `has_tween(track, frame)`: para habilitar los menús.
  Las tres devuelven `None` si nada cambia.

## 4. UI (`cs-ui`)

**Timeline** (fila plegada de una capa con un solo objeto y sub-filas de cada objeto):

- En cada celda de un tramo con tween, el fondo con el color de selección del skin al 25 % (en
  Flash es violeta) y, en lugar de la barra de hold, una **flecha**: la línea hasta la celda
  anterior a la key final, con su punta (un triángulo de 3 px de Pixel UI). Si el tween está roto,
  la línea es **punteada** (1 px sí, 1 no) y sin punta.
- Menú contextual de la celda (las dos filas), después de Insert/Delete Keyframe y un separador:
  **Create Tween** (habilitado si algún objeto de la fila tiene key vigente con una siguiente y
  sin tween) y **Remove Tween** (si alguno tiene tween ahí). En la fila plegada actúan sobre
  `key_objects` (como Insert Keyframe); un solo paso de undo.

**Inspector Object** (después de Position):

- **Scale** X % e Y % (1–2000 %), **Rotation** (°, −3600…3600), **Tint** (botón de color +
  cantidad 0–255; cantidad 0 = sin tinte).
- Si la key vigente tiene tween: un bloque **Tween** con **Ease** (select: Classic, Ease In,
  Ease Out, Ease In Out, Bounce, Elastic, Back, Step), **Amount** (−100…100 en Classic, 0…100 en
  las curvas) y el botón **Remove Tween**; si no tiene pero se puede, el botón **Create Tween**.
  Si el tween está roto, una línea: "Tween needs a next keyframe with the same symbol".
- En medio de un tween los campos muestran la pose (lo que se ve); cambiarlos crea la key ahí.

**Edit in Place**: solo con la pose sin escala, sin rotación y sin flips (si no, aislado, como ya
pasa con los volteados). La posición del símbolo es la de la pose.

**Move**: tocar y el recuadro usan la pose; la opacidad 0 que ignora el clic es la de la pose.

## 5. Archivos (`cs-io`)

Nada nuevo: los campos van en el JSON de los objetos del `.chimp` (`LayerExtra.objects`), con
`serde(default)`. Los `.chimp` viejos cargan iguales y el `.aseprite` interno y los exports llevan
el render horneado.

## 6. MCP (`cs-agent`)

- `set_object_key` y `set_instance_key` ganan `scale_x`, `scale_y` (porcentaje, decimales
  permitidos, 0.1–2000), `rotation` (grados), `tint` (color, `"#rrggbb"`) y `tint_amount`
  (0–255; 0 quita el tinte).
- **`set_tween`** `{doc, artboard?, layer, object?, frame, ease?, amount?}`: crea el tween del
  tramo que contiene `frame` (o cambia su ease). `ease`: `classic` (default), `ease_in`,
  `ease_out`, `ease_in_out`, `bounce`, `elastic`, `back`, `step`. `object` por id o nombre (sin
  él, el primero de la capa).
- **`remove_tween`** `{doc, artboard?, layer, object?, frame}`.
- `describe` ya lista las keys de cada objeto: incluye `scale`, `rotation`, `tint` y `tween`
  cuando no son los de siempre.

## 7. Pruebas

**`cs-core`**
- `Ease::Classic` en ±100 y 0 (cuadrático, como Flash); `Ease::Curve` con intensidad 0, 50 y 100.
- `pose_at`: sin tween = la key; posición/escala/rotación/opacidad/tinte interpolados y
  redondeados; tween roto (sin siguiente, otra fuente) = hold; el frame final es la key final.
- `keyframe_at` en medio de un tween: el tramo partido pasa por los mismos valores.
- JSON viejo sin los campos nuevos carga con los de siempre; una key sin transformación se
  escribe igual que antes.

**`cs-render`**
- Sin transformación, el render es idéntico al de antes.
- Escala 200 %: cada píxel se repite 2×2 alrededor del centro; 50 %: uno de cada dos.
- Rotación de 90° exacta (permutación); 45° con RotSprite (sin colores nuevos: pixel art).
- Tinte al 100 % y al 50 %; el alfa se conserva.
- Un tween de posición en el frame del medio dibuja en la posición interpolada; `object_image`
  también.

**`cs-commands`**
- `create_tweens`/`remove_tweens`/`update_tweens` con undo; sin key siguiente no se crea.
- `move_objects` y `update_objects_key` en medio de un tween crean la key con la pose.

**`cs-ui`**
- Menú del timeline: Create Tween y Remove Tween en la fila y en la sub-fila (un undo).
- El timeline dibuja el fondo y la flecha del tramo (y la línea punteada si está roto).
- Inspector: Scale/Rotation/Tint cambian la key; Ease cambia el tween sin crear keyframe.
- Edit in Place con un objeto rotado entra aislado.

**`cs-agent`**
- `set_object_key` con escala/rotación/tinte; `set_tween` + `render` del frame del medio;
  `remove_tween`.

## Ajustes al spec (descubiertos al implementar)

1. **F8 con objetos transformados.** El recorte del símbolo nuevo usaba la caja del símbolo fuente
   en la posición de cada key: con escala, rotación o tween recortaba. Ahora recorre la pose de
   cada frame y, si está transformada, toma un cuadrado que contiene cualquier giro del símbolo
   escalado (con un píxel de margen). El render sigue idéntico (lo garantiza una prueba).
2. **Escala hasta 2000 %** en el panel y en el MCP: el bloque escalado se calcula entero antes de
   recortar al lienzo (además, cada lado se limita a 4096 px como en la selección).
3. **`Editor::instance_key()` devuelve lo que se ve** (`keyframe_at`): el panel Object muestra la
   pose en medio de un tween y sus cambios crean la key ahí con esos valores.
4. **Las marcas del tween** (fondo y flecha) se ven en la fila plegada solo si la capa tiene un
   objeto (como las barras de hold); con varios, en sus sub-filas.

## Decisiones que el usuario puede revisar

1. **Classic Tween, no Motion Tween**: el tween vive en la key de inicio y llega a la siguiente
   key, como el Classic Tween de Flash; reutiliza las keys de los objetos que ya existen.
2. **Ease de Flash (−100…100) más las curvas de ChimpCAD** con intensidad (la misma lista del modo
   Anim, para no tener dos juegos de curvas).
3. **Rotación sin envolver** en lugar del "Rotate: CW/CCW × n" de Flash: para dar dos vueltas, la
   key final dice 720. Así partir un tween con un keyframe nuevo no cambia el giro.
4. **Pivote fijo en el centro del símbolo** (el punto de transformación por defecto de Flash); no
   se puede mover todavía.
5. **Pixel art**: posición redondeada al píxel en cada frame, escala por vecino más cercano y
   rotación con RotSprite (el algoritmo por defecto de la selección), nunca con antialias.
6. **Tween roto = hold** y línea punteada (sin key final o con otro símbolo al final).
7. **Fuente, flips y tiempo del símbolo no se interpolan**: salen de la key de inicio (el flip
   salta en la key final).
8. **Tinte con cantidad 0–255** (como la opacidad de la app) en lugar del 0–100 % de Flash.
9. **Create Tween se deshabilita sin key siguiente** (en Flash se puede crear y queda punteado);
   un tween puede quedar roto después (borrando la key final o cambiándole el símbolo).
10. **Con escala o rotación, Edit in Place entra aislado** (como con los flips).
11. **Colores del timeline**: el tramo con tween usa el color de selección del skin al 25 % (no
    el violeta de Flash) para seguir la paleta de la app.
