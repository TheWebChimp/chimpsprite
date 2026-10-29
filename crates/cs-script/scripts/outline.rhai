// Contorno de 1 px con el color primario alrededor de lo pintado en la capa actual
// (con selección, solo dentro de ella).
let px = pixels();
let w = width();
let h = height();
let color = primary();
let selected = #{};
let sel = selection();
if sel != () {
    for p in sel { selected[`${p[0]},${p[1]}`] = true; }
}
let out = [];
for y in 0..h {
    for x in 0..w {
        if px[y][x] != "transparent" { continue; }
        if sel != () && !(`${x},${y}` in selected) { continue; }
        let touches = false;
        for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let nx = x + d[0];
            let ny = y + d[1];
            if nx >= 0 && ny >= 0 && nx < w && ny < h && px[ny][nx] != "transparent" {
                touches = true;
            }
        }
        if touches { out.push(#{ x: x, y: y, color: color }); }
    }
}
if out.is_empty() {
    print("Nothing to outline on this layer");
} else {
    draw_pixels(#{ pixels: out });
    print(`Outline: ${out.len()} pixels`);
}
