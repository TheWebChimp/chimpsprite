# Plan: ChimpCAD parte 5, animación de objetos

Spec: `docs/superpowers/specs/2026-11-06-chimpcad-animation-design.md`. Rama `chimpcad-anim`
(worktree `cs-cad`). TDD por tarea, un commit por tarea con tests + clippy en verde.

1. **Datos** (cs-core): `Axis3`, `Curve`, `ClipMotion`, `Clip`, `Object3D.anim`,
   `Group3D.anim`, `Model3D.anim_ms` con serde default. Tests: archivo viejo sin campos, ida y
   vuelta por JSON. Arreglar todos los literales.
2. **Evaluación** (cs-3d `anim.rs`): `curve_value`, `clip_progress` (repeticiones, ping pong,
   antes/después), `node_pose` (orden, composición), `pose_at`, `posed_to_world`, `clip_lanes`,
   `snap_ms`. Tests de cada curva en 0/1, monotonía de las ease, bounce, composición de rotación y
   desplazamiento, pivote del grupo (la tapa sobre la bisagra).
3. **Render** (cs-3d raster): `RenderInput.time_ms`; `world_vertices` con la pose; `extra_yaw`
   como giro del modelo entero alrededor del objetivo. Tests: render en t con una tapa que gira
   ≠ reposo; turntable de dos objetos corridos orbita.
4. **Export** (cs-render): `Motion`, `BakeOptions::motion`, muestras (yaw, t), duraciones en
   bake y GIF, `model_render_at`/`model_pick_at`. Tests: frames distintos en Animation, duración
   `Length/Frames`, Turntable igual que antes.
5. **Editor** (cs-ui `editor/anim_ops.rs`): `CadMode::Anim`, `AnimView`, agregar/cambiar/
   borrar bloques, gesto de mover/estirar con vista previa y un paso de undo, Length, reloj.
   Tests en `editor/anim_tests.rs`.
6. **Lienzo** (cs-ui `canvas/anim_mode.rs`): layout, Persp animada y pick, barra de transporte,
   timeline (regla, pistas, carriles, bloques, línea del tiempo), entrada del timeline, panel del
   bloque, botón Anim del encabezado. Tests de layout/entrada con el harness del lienzo.
7. **Diálogos**: select Motion en Sheet settings y GIF settings; Frame (ms) calculado.
8. **Demo y verificación**: `CHIMPSPRITE_DEMO_ANIM` (caja con tapa que se abre, en modo Anim,
   en t=0.6 s), capturas, GIF exportado revisado con PIL, test/clippy/fmt/wasm.
