# ChimpCAD: gizmos — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Gizmos Move / Rotate / Scale como Blender sobre lo elegido, en Persp y en las orto.

**Architecture:** Geometría y operaciones puras en `cs-3d` (rotar selección, rotación de objeto en
ejes del mundo, geometría y hit-test del gizmo); estado, gesto (`EditOp` con vista previa), dibujo
pixel art, botones y teclas en `cs-ui`.

**Tech Stack:** Rust 2024, egui 0.36, el UI kit propio.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-03-chimpcad-gizmos-design.md`.
- Pixel art en la rejilla de la UI (`cs_3d::wire`); colores X rojo, Y verde, Z azul, activo
  `#ffec27`; tamaño fijo en pantalla.
- Un paso de undo por arrastre, con vista previa; clic sin arrastrar no cambia nada; Esc/clic
  derecho cancela.
- Componentes del kit; textos en inglés ("Move", "Rotate", "Scale"); comentarios en español.
- Nunca `git checkout -- …`, `git restore`, `git stash`, `git reset --hard`, `git clean`; solo
  `git add <archivos propios>`; LF; no tocar `.design/`.
- Toolchain de siempre, cargo a prioridad baja; fmt y clippy sin avisos; commits en español con
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

### Task 1: Geometría pura (`cs-3d`)
- `rotate_selection`, `rotate_object_world`, geometría del gizmo (flechas, aros como polilíneas,
  cubitos) para un centro, ejes y un tamaño en mundo, y hit-test en pantalla con un proyector
  (`Fn(P3) -> (f32, f32)`). Pruebas del spec §5 (cs-3d).
- Commit: "Modelos 3D: rotar la selección y la geometría de los gizmos".

### Task 2: Gizmos en las vistas orto (estado, botones, teclas, gestos)
- `Gizmo` en `ModelEdit`; grupo Move | Rotate | Scale en el encabezado (modo Model); W/E/R; dibujo
  en Front/Top/Side (dos ejes; Rotate = aro de profundidad); gestos con `EditOp` nuevo; prioridad
  sobre picks/caja. Pruebas UI del spec para las orto.
- Commit: "ChimpCAD: gizmos Move, Rotate y Scale en las vistas orto".

### Task 3: Gizmos en Persp
- Dibujo 3D sobre el render (proyección con la cámara del modelo) y los mismos gestos en Persp
  (ejes del mundo proyectados; aros como elipses). Pruebas UI y captura de cada gizmo.
- Commit: "ChimpCAD: gizmos en la vista Persp".
