# Movie clips, parte 2: editar un símbolo en su lugar (Edit in Place)

Fecha: 2026-11-06 · Estado: propuesto por el agente (sin revisión del usuario todavía)

## Objetivo

Hoy un símbolo de la biblioteca se edita en el **modo símbolo aislado** (parte 1): el lienzo
muestra solo el símbolo. Flash/Animate tiene además **Edit in Place**: doble clic en una instancia
del escenario y el símbolo se edita ahí mismo, con el resto de la escena atenuado alrededor.

Esta parte agrega Edit in Place y conserva la edición aislada (en Flash existen las dos):

- **Doble clic con Move sobre una instancia** de un símbolo de la biblioteca: entra **en su lugar**.
- **Library (doble clic, "Edit Symbol")** y el botón "Edit Symbol" del inspector: entran
  **aislado**, como hoy.
- El menú contextual del objeto en el lienzo ofrece las dos: **Edit in Place** y **Edit Symbol**.

De paso: el ícono de la capa de objetos en el timeline con Pixel UI (pieza del skin en lugar del
emoji 🖼, que la fuente pixel dibuja encima del nombre).

Fuera de alcance:

- editar en su lugar una instancia volteada (ver Decisiones);
- editar en su lugar la fuente de una instancia que está en el lienzo (se conserva lo de hoy:
  seleccionar y encuadrar ese artboard), un modelo 3D o una fuente;
- tweens y transformación (parte 3).

## 1. Estado (`cs-ui`, `Editor`)

La pila del modo símbolo pasa de `Vec<ArtboardId>` a `Vec<SymbolLevel>`:

```rust
pub struct SymbolLevel {
    pub id: ArtboardId,
    /// `Some`: en su lugar, dentro de la instancia `place`; `None`: aislado.
    pub place: Option<InPlace>,
}

pub struct InPlace {
    /// El artboard donde está la instancia: uno del lienzo o el nivel anterior de la pila.
    pub parent: ArtboardId,
    /// El frame del padre al entrar: el contexto se ve congelado en ese frame.
    pub parent_frame: usize,
    pub layer: LayerId,
    pub object: ObjectId,
    /// La posición del símbolo en la biblioteca antes de entrar (se devuelve al salir).
    pub home: (i32, i32),
}
```

- **Dónde se ve el símbolo.** Mientras se edita en su lugar, la `position` del artboard del
  símbolo (que en la biblioteca solo sirve para "Place on Canvas") vale la del padre más la
  `position` de la key vigente del objeto en `parent_frame`. Así todo lo que ya usa la posición del
  activo (el lienzo, la entrada, las reglas, la selección, el pincel) funciona sin cambios. Al
  salir se devuelve `home`. La posición se recalcula en `ensure_valid` (un undo puede mover la key).
- **Guardar** usa una copia con las `home` devueltas: el archivo nunca guarda la posición de
  edición.
- **Si la instancia deja de existir** (un undo borra el objeto o su key, o cambia su fuente), la
  pila se recorta antes de ese nivel, como cuando el símbolo deja de existir.
- **Entrar en su lugar** (`enter_symbol_in_place(layer, object)` sobre el activo):
  - la fuente de la key del objeto en el frame actual tiene que ser un símbolo de la biblioteca y
    el objeto no puede estar volteado; si no, se entra aislado (o, si no es símbolo, lo de hoy);
  - se apila, se activa y el frame del símbolo pasa al que la instancia muestra en ese frame del
    padre (`source_frame`), para que se vea igual que antes del doble clic;
  - la vista (zoom y desplazamiento) no cambia.
- **Salir**: un nivel con **Esc** (si no hay nada que cancelar antes: gesto, flotante, objetos
  seleccionados o marquesina), con doble clic fuera del símbolo (Move) o con la migaja de pan.
  Esc también sale de un nivel aislado.

## 2. El lienzo

Con el nivel de arriba de la pila en su lugar:

- **Contexto atenuado**: el "contexto" es lo que se veía en el nivel donde empieza la cadena de
  niveles en su lugar: todos los artboards del lienzo (si la cadena empieza en el documento) o el
  símbolo aislado de abajo. Se compone desde una copia del documento donde **la instancia que se
  edita no está** (si está anidada, se copian los símbolos intermedios sin ella), así no hay un
  fantasma con otro frame debajo. Se dibuja con su damero, sin ventanas, barras, nombres ni guías,
  y encima un velo del color de fondo del lienzo al 55 %. Las demás instancias del mismo símbolo
  que se ven en el contexto se actualizan en vivo.
- **El símbolo**: encima, en su lugar, opaco y sin la ventana de Pixel UI (con el borde de 2 px del
  activo). Todas las herramientas pintan en él; el timeline, las capas y el inspector son los del
  símbolo.
- **Clics fuera del símbolo** no activan otros artboards (como en el modo aislado).
- **Migaja de pan**: la de la parte 1, ahora con **"Scene"** como primer nivel (Flash dice
  "Scene 1"): `Scene › Coin › Shine`. Clic en un nivel vuelve a él.

## 3. Timeline (`cs-ui`)

- En su lugar, el timeline muestra la línea de tiempo del símbolo (el activo). El playback reproduce
  solo el símbolo; el contexto queda congelado.
- Con Pixel UI, la fila de una capa de objetos dibuja la pieza `icon.layer.objects` (9×9) donde
  los grupos tienen la carpeta y los tilemaps su ícono, y `icon.layer.objects.selected` en la capa
  activa. **Ícono provisional** dibujado con Lua (un cuadro con montañas y sol, "imagen"); el
  usuario lo redibujará.

## 4. Undo

Entrar y salir no son pasos de undo (como en Flash). Lo que se pinta adentro es un paso por gesto,
como siempre, y se aplica al símbolo: todas sus instancias cambian.

## 5. Pruebas

**`cs-ui` (editor)**
- Doble clic sobre una instancia entra en su lugar: el activo es el símbolo, su posición es la de
  la instancia, el frame es el que mostraba y la migaja dice `Scene › S`.
- Pintar en su lugar pinta en el píxel del símbolo bajo el cursor (no corrido) y es un undo.
- Esc sale un nivel y devuelve la posición `home`; también en el modo aislado.
- Anidado: en su lugar dentro de en su lugar suma las posiciones; Esc sale de a uno.
- Una instancia volteada entra aislado.
- Un undo que borra el objeto recorta la pila y devuelve la posición.
- La copia para guardar tiene las `home`.
- El contexto oculta la instancia editada (también anidada) y deja las demás.

**`cs-ui` (lienzo y timeline)**
- El lienzo dibuja el contexto atenuado y el símbolo en su lugar (la textura del contexto existe y
  la del símbolo se dibuja en la posición de la instancia).
- Con Pixel UI, la fila de la capa de objetos dibuja `icon.layer.objects`.

## Decisiones que el usuario puede revisar

1. **Doble clic = en su lugar; Library e inspector = aislado.** Como en Flash (doble clic en el
   escenario edita en su lugar; "Edit Symbol" aísla). El menú del objeto ofrece las dos.
2. **Instancias volteadas se editan aisladas.** Pintar a través de un flip necesita invertir la
   entrada y todos los overlays; llega con la transformación (parte 3).
3. **El contexto queda congelado en el frame del padre** de cuando se entró, y el símbolo empieza en
   el frame que la instancia mostraba ahí. Mover el timeline mueve solo el símbolo.
4. **La instancia que se edita no se dibuja en el contexto**; el símbolo va encima de todo, opaco
   (sin la opacidad de su key ni lo que la tape en el padre), para poder ver lo que se pinta.
5. **Atenuado = velo del color del fondo al 55 %** sobre el contexto, sin ventanas ni nombres.
6. **"Document" → "Scene"** en la migaja (aislado y en su lugar).
7. **Esc sale un nivel** cuando no hay nada más que cancelar, también en el modo aislado.
8. **La posición del símbolo se usa para verlo en su lugar** y se devuelve al salir; guardar escribe
   la de antes. Un script/MCP que lea `describe` mientras se edita ve la posición de edición.
9. **La vista no se mueve al entrar en su lugar** (el aislado sigue encuadrando).
10. **Ícono provisional** `icon.layer.objects` (9×9, "imagen" con montañas), para redibujar.
