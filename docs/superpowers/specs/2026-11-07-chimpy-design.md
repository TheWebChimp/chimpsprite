# Chimpy: el asistente escondido (easter egg tipo Clippy)

## Contexto

Con Pixel UI, arriba a la derecha corre un chango (`assets/ui/running.aseprite`, pintado sobre el
degradé de la barra de pestañas con `Logo::paint_at`, sin interacción). El encargo: un easter egg
como Clippy, el clip de Microsoft Office: una mascota que aparece a "ayudar", juguetona, escondida
(opt-in) y que **no moleste**: nunca roba el foco ni tapa el lienzo de forma molesta, se va sola y
no aparece más de cada tantos minutos.

## Objetivo

1. **Cómo se activa (escondido)**: cinco clics seguidos (en menos de 2,5 s entre el primero y el
   quinto) al chango que corre arriba a la derecha. La primera vez, Chimpy se presenta. Desde ahí
   queda "descubierto" y aparece **Help → Chimpy** (un check para apagarlo o prenderlo) y el check
   **Chimpy Tips** en Edit → Preferences → General. Con Chimpy prendido, los cinco clics piden un
   consejo en el momento. Todo se guarda entre sesiones (`chimpsprite.mascot` en el almacenamiento
   de eframe, aparte de `ViewSettings`: Reset de Preferences no lo apaga ni lo "olvida").
2. **Dónde y cómo se ve**: Chimpy cuelga de un brazo del borde de arriba del lienzo, en la esquina
   de arriba a la izquierda (debajo de la regla), y a su derecha sale el globo de los tooltips
   (`tooltip.bubble`, en 9-slice: la cola a la izquierda apunta a su cara). En el globo, el
   consejo (fuente chica, como los tooltips) y una fila de botones del kit (`kit::button`):
   - Si el consejo sugiere algo que la app puede hacer: **Yes** (lo hace) y **No, thanks**.
   - Si es solo información: **OK** (y, si sirve, un segundo botón con lo que lleva a más:
     p. ej. **Shortcuts…** abre Keyboard Shortcuts).
   Clic en Chimpy: otro consejo ("¿Sabías que…?").
3. **Animación** (pocos frames, asset aparte `assets/ui/mascot.aseprite`, 32×40, con tags):
   - Entrada: baja deslizándose desde arriba del lienzo (≈0,3 s, recortado por la caja del
     lienzo, como si bajara de una liana).
   - Hablando (el primer segundo y medio): abre y cierra la boca (`talk`).
   - Quieto: respira/parpadea (`idle`, el parpadeo corto entre esperas largas).
   - Salida: sube de vuelta (≈0,25 s).
   El arte es **provisional** (dibujado con Lua en Aseprite a partir de los colores del chango que
   corre); el usuario lo redibujará: el código solo pide los tags `idle` y `talk`.
4. **Cuándo aparece** (lógica pura, sin egui, en `mascot.rs`, con tests):
   - Nunca si está apagado, en Home, con un diálogo modal abierto, escribiendo en un campo o con
     el puntero apretado (pintando, arrastrando); en ese caso el consejo contextual espera hasta
     10 s a que se desocupe, y si no, se pierde.
   - Ni antes de 60 s de abierta la app, ni antes de 4 min desde el último consejo automático.
   - Contextuales (se detectan comparando lo que se ve en cada frame con el anterior, en la misma
     pestaña y el mismo artboard), una vez cada uno (se recuerdan entre sesiones):
     | id | Cuándo | Texto (resumen) | Botones |
     |---|---|---|---|
     | `walk-cycle` | el artboard pasa de ≤2 a ≥3 frames con Onion Skin apagado | "It looks like you're drawing a walk cycle! Want to turn on Onion Skin?" | Yes → Onion Skin |
     | `mirror` | se hace una selección con algún eje de simetría prendido y sin Mirror Selection | "Symmetry is on. Want Mirror Selection? …" | Yes → la prende |
     | `play-in-place` | el lienzo pasa a tener ≥2 artboards y el activo tiene más de un frame | "Want this artboard to play right on the canvas? (Play in Place, in the artboard's menu)" | Yes → Play in Place del activo |
     | `tool.<herramienta>` | se elige una herramienta con el mouse (no con su atajo) | "Tip: press B to pick the Pencil." + un truco verificado de esa herramienta | OK, Shortcuts… |
     | `font` | se abre/activa una fuente (FontForge) | kerning y variantes (Bold, Italic, Outline, Shadow) | OK |
     | `model` | se abre/activa un modelo 3D (ChimpCAD) | Ctrl+E extruye, Ctrl+D subdivide en modo Model | OK |
   - "¿Sabías que…?" (en orden, uno cada vez): si pasaron 8 min desde el último consejo y el
     usuario lleva 5 s sin tocar nada. Mirror Selection, tweens, Play in Place, FontForge,
     ChimpCAD, post-its (N), Coolors/Lospec, Tiled Mode, Ctrl+H, Expand to Spritesheet/Slice into
     Frames, Groom, Lumen, Space+arrastrar, Ctrl+rueda, Keyboard Shortcuts. Al terminarse, los
     automáticos paran; pedidos (cinco clics o clic en Chimpy) siguen dando vueltas.
   - Los atajos de los textos salen del keymap (si el usuario cambió una tecla, el consejo dice la
     suya; si una acción quedó sin atajo, ese consejo no sale).
5. **Se va solo**: a los 14 s (el tiempo no corre con el puntero encima del globo o de Chimpy), al
   tocar un botón, o apenas el usuario empieza a pintar (aprieta en el lienzo).
6. **Demo** para capturas: `CHIMPSPRITE_DEMO_MASCOT=<id>` (p. ej. `walk-cycle`, `tool.pencil`,
   `hello`) muestra ese globo al arrancar, sin esperar ni cambiar lo guardado.

## Fuera

- Arrastrar a Chimpy a otro lado, otros personajes (Clippy tenía varios), sonidos propios.
- Consejos que dependan de analizar el dibujo (detectar de verdad un walk cycle).
- Traducir los textos.

## Decisiones que el usuario puede revisar

1. **Nombre "Chimpy"** (por Clippy). Está en las constantes de textos de `mascot.rs`.
2. **Activación con 5 clics al chango que corre** (en vez de un código Konami: las flechas, B y A
   ya son atajos de la app). Una vez descubierto, Help → Chimpy y Preferences lo prenden/apagan.
3. **Colgado arriba a la izquierda del lienzo** (en vez de abajo como Clippy): así el globo de los
   tooltips (cola a la izquierda, arriba) se reutiliza tal cual, y lo de arriba a la izquierda del
   lienzo suele estar vacío (los artboards se centran). El globo usa el naranja de los tooltips.
4. **Ritmo**: 60 s de gracia al abrir, 4 min entre automáticos, 8 min (y 5 s quieto) para los
   "¿Sabías que…?", 14 s en pantalla. Cada contextual sale una sola vez en la vida (se guarda).
5. **"walk cycle"** se dispara con cualquier animación de 3+ frames (como el chiste de Clippy con
   las cartas: adivina en grande).
6. **El arte es provisional**: `assets/ui/mascot.aseprite` (tags `idle` y `talk`), para redibujar.
7. **No hay botón "no me muestres más"** en el globo (Clippy lo tenía): se apaga en Help o en
   Preferences, para que un clic apurado no lo mate sin querer.
