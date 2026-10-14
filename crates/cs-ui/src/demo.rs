//! Sprite de demostración (`CHIMPSPRITE_DEMO=1`), dibujado a través de comandos normales.

use cs_commands::Command;
use cs_core::PixelValue;

use crate::editor::Editor;

/// Dibuja una cara de chimpancé centrada usando índices de la paleta DB32.
pub fn draw(editor: &mut Editor) {
    let (w, h) = (editor.sprite.width as i32, editor.sprite.height as i32);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let r = (w.min(h) as f32) * 0.3;
    let fur = editor.palette_value(4);
    let fur_dark = editor.palette_value(3);
    let face = editor.palette_value(7);
    let eye = editor.palette_value(1);
    let shine = editor.palette_value(21);
    let mouth = editor.palette_value(2);

    let mut pixels: Vec<(i32, i32, PixelValue)> = Vec::new();
    let inside = |x: i32, y: i32, ox: f32, oy: f32, rx: f32, ry: f32| {
        let dx = (x as f32 + 0.5 - ox) / rx;
        let dy = (y as f32 + 0.5 - oy) / ry;
        dx * dx + dy * dy <= 1.0
    };
    for y in 0..h {
        for x in 0..w {
            let ear = inside(x, y, cx - r, cy, r * 0.35, r * 0.35)
                || inside(x, y, cx + r, cy, r * 0.35, r * 0.35);
            let head = inside(x, y, cx, cy, r, r);
            let muzzle = inside(x, y, cx, cy + r * 0.35, r * 0.6, r * 0.42);
            let brow = inside(x, y, cx - r * 0.3, cy - r * 0.1, r * 0.32, r * 0.3)
                || inside(x, y, cx + r * 0.3, cy - r * 0.1, r * 0.32, r * 0.3);
            let color = if muzzle || brow {
                Some(face)
            } else if head {
                Some(fur)
            } else if ear {
                Some(fur_dark)
            } else {
                None
            };
            if let Some(c) = color {
                pixels.push((x, y, c));
            }
        }
    }
    // Ojos, brillo y boca.
    let eye_y = (cy - r * 0.1) as i32;
    for ex in [(cx - r * 0.3) as i32, (cx + r * 0.3) as i32 - 1] {
        pixels.extend([(ex, eye_y, eye), (ex + 1, eye_y, eye), (ex, eye_y + 1, eye), (ex + 1, eye_y + 1, eye)]);
        pixels.push((ex, eye_y, shine));
    }
    let mouth_y = (cy + r * 0.5) as i32;
    for x in (cx - r * 0.25) as i32..=(cx + r * 0.25) as i32 {
        pixels.push((x, mouth_y, mouth));
    }
    pixels.push(((cx - r * 0.3) as i32, mouth_y - 1, mouth));
    pixels.push(((cx + r * 0.3) as i32, mouth_y - 1, mouth));

    editor.execute(Command::DrawPixels {
        layer: editor.layer,
        frame: editor.frame,
        pixels,
    });
}
