# ChimpCAD: el panel 3D por pestañas — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pestaña fija del modelo + una por objeto; la del objeto con transform compacto en filas
X/Y/Z; lo del modelo en su pestaña; un `+` para agregar objetos.

**Architecture:** `kit::stepper` gana una etiqueta de eje opcional; `model_panel.rs` arma la tira
(pestaña del modelo, objetos, ◁ ▷, `+`) y reparte el cuerpo según la pestaña activa.

**Tech Stack:** Rust 2024, egui 0.36, el UI kit propio.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-03-chimpcad-panel-tabs-design.md`; boceto
  `D:/Work/chimpdeas/.refs/chimpcad/mockup-object-tab.png` (a 1×, manda en el acomodo).
- Componentes del kit (nada de piezas sueltas); textos en inglés; comentarios en español.
- Nunca `git checkout -- …`, `git restore`, `git stash`, `git reset --hard`, `git clean`; solo
  `git add <archivos propios>`; LF; no tocar `.design/`.
- Toolchain: como siempre (`RUSTUP_HOME`/`CARGO_HOME` en `/d/Work/chimpdeas/.rust-toolchain`,
  `CARGO_BUILD_JOBS=8`, cargo a prioridad baja con `cmd //c "start /low /b /wait cargo …"`); fmt y
  clippy sin avisos; commits en español con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

### Task 1: Stepper con etiqueta de eje y la pestaña de un objeto compacta

- `kit::stepper`: una variante con etiqueta de eje a la izquierda (cajita del color oscuro del kit
  con la letra, como el boceto; mismo alto que el stepper), sin romper los usos actuales. Prueba de
  geometría del kit.
- La pestaña de un objeto: Position / Rotation / Scale con un título y una fila de tres steppers
  etiquetados (X, Y, Z) cada una; si el panel es angosto, dos filas. Double sided debajo. Quitar el
  botón Delete. En modo Texture, la sección UV sin Texture size.
- Pruebas de layout (filas de tres con etiquetas; un cambio = un paso de undo; nada de Texture,
  Camera ni Light en esta pestaña) y captura comparada con el boceto.
- Commit: "Panel 3D: la pestaña del objeto con el transform en filas X, Y, Z".

### Task 2: La pestaña del modelo y el botón +

- La tira: primera pestaña fija del modelo (etiqueta = los cinco íconos de tipo, sin cierre;
  activa sin objeto elegido; clic → `select_model_object(None)`), luego las de objetos, y a la
  derecha las flechas y un botón `+` del kit con un menú de primitivas y el stepper Sides; elegir
  agrega y elige el objeto. Quitar Add/Sides del cuerpo.
- La pestaña del modelo: modos de selección y Grid (en Model), Texture (+ Texture size), Camera,
  Light & Outline con Shading, Sheet settings….
- Pruebas (la pestaña del modelo es la primera, sin cierre, activa sin objeto; `+` agrega con
  Sides; contenido de cada pestaña; sin scroll con un objeto elegido a 1920×1009) y captura.
- Commit: "Panel 3D: pestaña del modelo y botón + para agregar objetos".
