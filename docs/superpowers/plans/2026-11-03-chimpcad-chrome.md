# ChimpCAD: marco de la pestaña del modelo — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** El ícono 3D del dock, el encabezado CHIMPCAD con Model | Texture, sin barra de opciones
en la pestaña del modelo, y una pestaña por objeto (ícono de tipo + nombre) arriba del panel 3D.

**Architecture:** Piezas nuevas en `assets/ui/skin.aseprite` (por el MCP de Aseprite). `Object3D`
gana `kind`. El layout de la app esconde la barra de opciones en el workspace de un modelo; el
lienzo reserva una franja de encabezado; el panel 3D cambia su título y la lista por la tira de
pestañas del kit.

**Tech Stack:** Rust 2024, egui 0.36, el UI kit propio, Aseprite (MCP) para el skin.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-03-chimpcad-chrome-design.md`; referencias del usuario en
  `D:/Work/chimpdeas/.refs/chimpcad/` (`icon-3d.png`, `mockup-workspace.png`, a 1×). Las capturas
  mandan en el acomodo; los pixeles de las piezas se copian tal cual.
- Componentes del kit; nada de piezas sueltas para estados (hover/down salen del kit).
- Textos de UI en inglés; comentarios en español como el código vecino.
- Nunca `git checkout -- …`, `git restore`, `git stash`, `git reset --hard`, `git clean`; solo
  `git add <archivos propios>`; LF; no tocar `.design/`.
- Toolchain (Git Bash): `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=8`;
  cargo siempre a prioridad baja: `cmd //c "start /low /b /wait cargo test -p <crate> <filtro>"`.
  `cargo fmt --all` y `cargo clippy --workspace --all-targets` sin avisos; commits en español
  terminados con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

### Task 1: Las piezas del skin

- Con el MCP de Aseprite (`run_lua_script` sobre `assets/ui/skin.aseprite`; ver
  `assets/ui/README.md` y cómo están las piezas `icon.dock.*` existentes: capa, celda, slices con
  1 px de margen, lugar libre), agregar:
  - `icon.dock.model`: el cubo de `icon-3d.png` sin el marco del botón, del mismo tamaño y
    alineación que los otros `icon.dock.*`.
  - `logo.chimpcad`: el logo del encabezado del boceto (arriba a la izquierda), pixel a pixel.
  - `icon.object.cube`, `icon.object.cylinder`, `icon.object.cone`, `icon.object.sphere`,
    `icon.object.plane`: los cinco íconos de la primera pestaña del boceto, en ese orden.
- Comprobar que el skin carga (`cargo test -p cs-ui skin` o las pruebas del skin) y una prueba que
  confirme que las siete piezas existen con el tamaño esperado.
- Commit: "Skin: ícono 3D del dock, logo de ChimpCAD e íconos de objetos".

### Task 2: El tipo de objeto, sin barra de opciones y el encabezado

- `cs-core`: `PrimitiveKind { Cube, Plane, Cylinder, Cone, Sphere }` y `Object3D.kind:
  Option<PrimitiveKind>` con `#[serde(default)]` (Eq/Hash como el resto). `cs-3d`/`add_primitive`
  lo llenan; `Object3D::shown_kind()` deduce de `name` si falta. Pruebas de roundtrip y de archivo
  viejo.
- La app no dibuja la barra de opciones de herramienta mientras el documento activo está en el
  workspace de un modelo (su alto pasa al área de abajo). Prueba de layout.
- El lienzo, en el workspace de un modelo: una franja de encabezado arriba del área (alto de una
  fila de botones del kit) con `logo.chimpcad` a la izquierda y los botones Model | Texture a la
  derecha; las vistas (los dos modos) empiezan debajo. Pruebas de layout (posiciones y que los
  botones ya no flotan sobre las vistas).
- Commit: "ChimpCAD: encabezado con el logo y sin barra de opciones".

### Task 3: Las pestañas de objetos en el panel 3D

- `model_panel.rs`: en lugar del título "3D" y la lista *Objects*, la tira de pestañas del kit
  (`kit::tabs`, la de la barra de documentos; extenderla si hace falta un ícono por pestaña, sin
  piezas sueltas) con una pestaña por objeto: ícono `icon.object.<kind>` + nombre; la elegida
  resaltada; clic elige el objeto (`select_model_object`); la marca de cerrar quita el objeto
  (`remove_object`, un paso de undo); flechas ◁ ▷ a la derecha si no caben. Debajo, Add/Sides y lo
  demás como hoy.
- Pruebas: una pestaña por objeto con ícono y nombre; clic elige; cerrar quita con un paso de
  undo; muchas pestañas → flechas que recorren.
- Captura (`CHIMPSPRITE_DEMO_MODEL=1`, con y sin `_TEXTURE`) comparada con el boceto.
- Commit: "Panel 3D: una pestaña por objeto con el ícono de su tipo".
