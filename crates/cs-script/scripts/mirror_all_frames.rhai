// Voltea la capa actual de izquierda a derecha en todos los frames.
let frames = describe().artboards[0].frame_durations_ms.len();
flip(#{ axis: "horizontal", frames: #{ from: 0, to: frames - 1 } });
print(`${frames} frames flipped`);
