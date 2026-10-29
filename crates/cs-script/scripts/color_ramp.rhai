// Una rampa de 7 tonos a partir del color primario: las sombras se van hacia azul y las luces
// hacia amarillo (como se hace a mano en pixel art). Se agregan al final de la paleta.
fn channel(hex, i) {
    parse_int(hex.sub_string(1 + i * 2, 2), 16).to_float() / 255.0
}

fn to_hsv(r, g, b) {
    let mx = max(r, max(g, b));
    let mn = min(r, min(g, b));
    let d = mx - mn;
    let h = 0.0;
    if d > 0.0 {
        if mx == r {
            h = 60.0 * (((g - b) / d) % 6.0);
        } else if mx == g {
            h = 60.0 * ((b - r) / d + 2.0);
        } else {
            h = 60.0 * ((r - g) / d + 4.0);
        }
    }
    if h < 0.0 { h += 360.0; }
    let s = if mx > 0.0 { d / mx } else { 0.0 };
    [h, s, mx]
}

fn to_hex(h, s, v) {
    let c = v * s;
    let x = c * (1.0 - abs((h / 60.0) % 2.0 - 1.0));
    let m = v - c;
    let rgb = if h < 60.0 { [c, x, 0.0] }
        else if h < 120.0 { [x, c, 0.0] }
        else if h < 180.0 { [0.0, c, x] }
        else if h < 240.0 { [0.0, x, c] }
        else if h < 300.0 { [x, 0.0, c] }
        else { [c, 0.0, x] };
    let out = "#";
    for k in rgb {
        let n = max(0, min(255, ((k + m) * 255.0).round().to_int()));
        out += if n < 16 { `0${n.to_hex()}` } else { n.to_hex() };
    }
    out
}

// Gira el tono `h` hacia `target` hasta `step` grados.
fn toward(h, target, step) {
    let diff = ((target - h + 540.0) % 360.0) - 180.0;
    let turn = if abs(diff) < step { diff } else if diff > 0.0 { step } else { -step };
    (h + turn + 360.0) % 360.0
}

let base = primary();
if base == "transparent" { throw "Pick an opaque primary color first"; }
let hsv = to_hsv(channel(base, 0), channel(base, 1), channel(base, 2));
let ramp = [];
for i in [3, 2, 1] {
    let k = i.to_float();
    ramp.push(to_hex(toward(hsv[0], 240.0, 12.0 * k), min(1.0, hsv[1] + 0.08 * k), max(0.0, hsv[2] - 0.16 * k)));
}
ramp.push(to_hex(hsv[0], hsv[1], hsv[2]));
for i in [1, 2, 3] {
    let k = i.to_float();
    ramp.push(to_hex(toward(hsv[0], 60.0, 10.0 * k), max(0.0, hsv[1] - 0.12 * k), min(1.0, hsv[2] + 0.12 * k)));
}
let palette = [];
for a in describe().artboards {
    if a.id == current_artboard() { palette = a.palette; }
}
for c in ramp { palette.push(c); }
set_palette(#{ colors: palette });
let text = "";
for (c, i) in ramp {
    if i > 0 { text += ", "; }
    text += c;
}
print(`Ramp: [${text}]`);
