# Post-its 2: herramienta Note, comandos por nota y MCP

## Contexto

Las notas (post-its) ya existen desde #24 (`cs-ui/src/notes.rs`): `Document::notes` (`Note { id,
position, size, text, color }`, `#[serde(default)]`, así que los `.chimp` sin notas abren igual), se
guardan en el manifiesto del `.chimp`, se dibujan encima de los artboards con las piezas 9-slice
`note`/`note.plain` (+ rosa, azul y verde) que dibujó el usuario —amarillo `#f8c53a`, contorno negro
de 1 px, sombra de 1×2 px y marquitas naranjas en las esquinas—, se mueven desde su barra, se agrandan
desde la esquina, se escriben con un clic (o doble clic) en el papel, cambian de color con la tira de
la derecha, se borran con "○" y tienen un menú "≡" (Edit Text, Bring to Front, Delete). Edit → New
Note pone una al centro de la vista. Todo pasa por un solo comando, `DocCommand::SetNotes(Vec<Note>)`
(reemplaza todas), y el MCP no las conoce.

Lo que falta para el encargo ("crear con herramienta o comando + clic en el lienzo", "comandos
serializables expuestos en el MCP", "no salen en los exports"):

## Objetivo

1. **Comandos por nota** (`cs-commands`): `AddNote { index, note }`, `SetNote(Note)` (la reemplaza
   por id) y `RemoveNote { id }`, con su inverso, como el resto de `DocCommand`. `SetNotes` se queda
   (orden, archivos/scripts viejos). Errores nuevos: `NoteNotFound(u32)`, `DuplicateNoteId(u32)`.
   Etiquetas de undo: "New Note", "Note", "Delete Note".
2. **Editor**: `add_note`, `set_note_text`, `set_note_color`, `remove_note` y el fin de un arrastre
   usan los comandos por nota (el historial dice qué pasó). Nuevo `duplicate_note(id)`: copia a +12,+12
   puntos de mundo, arriba de todas. `Document::note(id)` y `Document::next_note_id()` en `cs-core`.
3. **Herramienta Note** (`ToolKind::Note`, atajo **N**, en el grupo de Artboard de la barra): un clic
   en el lienzo —fuera o encima de un artboard— pone una nota amarilla centrada en el puntero y la deja
   lista para escribir. Si había una nota escribiéndose, ese clic solo termina de escribirla (no crea
   otra). Sobre una nota, el clic es de la nota (como con cualquier herramienta). En el modo símbolo y
   en el modo Texture (donde las notas no se ven) la herramienta no hace nada. Cursor: cruz. Ícono
   `icon.note` (provisional: un post-it chiquito dibujado con Lua; el usuario lo redibujará).
4. **Menú "≡" de la nota**: se agrega **Duplicate**.
5. **MCP / scripts** (`cs-agent` + `chimpsprite-mcp`): `add_note` (text, position opcional —por
   defecto a la derecha de los artboards, alineada arriba—, size opcional, color opcional),
   `set_note` (text, position, size, color), `remove_note`; `describe` agrega `sticky_notes: [{id,
   position, size, color, text}]`. Las tres van en `OPERATIONS` (Rhai/Lua las llaman por nombre).
   El tamaño mínimo es el de la UI (60×44); el MCP lo valida.
6. **Exports**: las notas viven en el documento y los exports son de sprites, así que no salen; un test
   lo fija (export PNG con y sin notas da lo mismo).

## Fuera

- Selección múltiple de notas, copiar/pegar notas entre documentos, Alt+arrastrar para duplicar.
- Que el tamaño siga al zoom (ver decisión 2).
- Formato de texto (negritas, listas), autor/fecha, reacciones.

## Decisiones que el usuario puede revisar

1. **Lo que ya existía se queda**: el diseño, la barra con "≡"/"○", la tira de colores, el pin cuando
   la nota taparía más de medio artboard y "un clic en el papel escribe". El encargo pedía "doble clic
   para editar": funciona (ya funcionaba), pero el clic simple también escribe, como estaba.
2. **Tamaño en puntos de interfaz, posición en el mundo**: la esquina sigue al zoom y al pan, pero el
   papel no se escala (el texto siempre se lee con la fuente pixel de la UI, nítido). Así estaba desde
   #24; FigJam/Miro sí escalan las notas. Si se prefiere que escalen, es cambiar `paper()` y el arrastre.
3. **Herramienta en el grupo de Artboard** (clic derecho o mantener presionado el botón de Artboard
   para elegirla) en vez de un botón nuevo: la barra de íconos tiene el alto del mockup. Atajo **N**
   (libre; FigJam usa S, que aquí es del Spray con Shift).
4. **La herramienta se queda activa** después de poner una nota (como Text en Aseprite): para seguir
   pegando notas. Un clic fuera mientras se escribe solo termina de escribir.
5. **Una nota nueva con la herramienta va centrada en el clic** (FigJam); la de Edit → New Note sigue
   al centro de la vista.
6. **Duplicate** corre la copia 12 puntos a la derecha y abajo y la deja arriba.
7. **MCP**: `add_note` sin posición la pone a 8 px a la derecha de todo lo del lienzo (como
   `add_artboard`), y el tamaño por defecto es el de la UI (86×89).
8. **Ícono provisional** `icon.note` (16×16 como el resto): el usuario lo redibujará.
