// Voltea la capa actual de izquierda a derecha en todos los frames.
let frames = 0;
for a in describe().artboards {
    if a.id == current_artboard() { frames = a.frame_durations_ms.len(); }
}
flip(#{ axis: "horizontal", frames: #{ from: 0, to: frames - 1 } });
print(`${frames} frames flipped`);
