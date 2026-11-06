# Plan: ChimpCAD parte 4 — GIF settings y FOV

Spec: `docs/superpowers/specs/2026-11-06-chimpcad-gif-design.md`. TDD, un commit por tarea.

1. **cs-io `encode_gif` / `encode_png_strip`**: sacar el encoder de `export_gif` a
   `encode_gif(w, h, &[Vec<Rgba>], delay_ms, looping)`; tests: frames, tamaño, delays, loop
   (extensión NETSCAPE presente o no), transparencia.
2. **cs-render `BakeOptions::fov`**: `from_model` toma el fov del modelo (60° si es orto);
   `camera()` lo usa. Test: otro fov cambia el render en perspectiva y no en orto.
3. **cs-render `turntable`**: `RenderPreset`, `TurntableOptions`, `turntable_frame`,
   `turntable`. Tests: tamaño native/presets, escala, fondo/transparencia, outline on/off,
   scanlines (filas impares más oscuras), paleta gameboy (solo 4 colores), frames = ángulos,
   el frame 0 native sin efectos = bake.
4. **cs-ui `gif_dialog`**: el diálogo (dos columnas + vista previa), `build()` que da
   (nombre, bytes); app: campo `gif`, `ModelAction::Gif`, botón "GIF settings…", modal,
   `choose_save` + `write`. Tests de diálogo (textos, steppers, build decodificado) y de layout
   (los tres botones en la fila, sin salirse).
5. **FOV en el editor**: presets en el select de proyección; Alt+rueda en Persp; rótulo
   "Persp 60°". Tests.
6. Demo `CHIMPSPRITE_DEMO_GIF`, captura, revisión del diff, verificación completa, bitácora.
