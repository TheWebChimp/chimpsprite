// Un tablero de ajedrez de 2×2 en la capa actual, en todo el sprite.
let info = describe();
let a = info.artboards[0];
let cell = 2;
let pixels = [];
for y in 0..a.height {
    for x in 0..a.width {
        if ((x / cell) + (y / cell)) % 2 == 0 {
            pixels.push(#{ x: x, y: y, color: "#222034" });
        }
    }
}
draw_pixels(#{ pixels: pixels });
print(`${pixels.len()} pixels painted`);
