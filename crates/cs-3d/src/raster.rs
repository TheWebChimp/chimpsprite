//! El rasterizador pixel de los modelos 3D.

use std::collections::HashMap;

use cs_core::{Camera3D, FIXED, Model3D, Object3D, Projection, Rgba, ShadingTable, Vec3i};

use crate::shade;

/// Una textura RGBA de `width`×`height` (fila por fila).
#[derive(Clone, Copy, Debug)]
pub struct Texture<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [Rgba],
}

/// Lo que hace falta para pintar un modelo.
#[derive(Clone, Copy, Debug)]
pub struct RenderInput<'a> {
    pub model: &'a Model3D,
    pub size: (u32, u32),
    /// `None`: las caras son de un gris medio (128,128,128).
    pub texture: Option<Texture<'a>>,
    /// La paleta del documento, para la rampa de la luz.
    pub palette: &'a [Rgba],
    /// `None`: la cámara del modelo.
    pub camera: Option<Camera3D>,
    /// Décimas de grado sumadas a la rotación Y de todos los objetos (al hornear).
    pub extra_yaw: i32,
}

/// Pinta el modelo en una imagen de `size.0`×`size.1` con fondo transparente.
pub fn render_model(input: &RenderInput) -> Vec<Rgba> {
    let mut r = rasterize(input);
    if let Some(color) = input.model.outline {
        outline(&mut r.color, r.w, r.h, color);
    }
    r.color
}

/// El índice del objeto que se ve en el pixel `(x, y)`, si hay alguno (sin contar el contorno).
pub fn pick_object(input: &RenderInput, x: u32, y: u32) -> Option<usize> {
    if x >= input.size.0 || y >= input.size.1 {
        return None;
    }
    let r = rasterize(input);
    r.ids[(y * input.size.0 + x) as usize].map(|(o, _)| o)
}

/// La cara `(objeto, índice de cara)` que se ve en el pixel `(x, y)`, si hay alguna.
pub fn pick_face(input: &RenderInput, x: u32, y: u32) -> Option<(usize, usize)> {
    if x >= input.size.0 || y >= input.size.1 {
        return None;
    }
    let r = rasterize(input);
    r.ids[(y * input.size.0 + x) as usize]
}

/// El contorno de 1 px de las caras `faces` `(objeto, cara)` en el render de `input`: por cada
/// cara, los pixeles que cubre su polígono (con la misma regla que el render, aunque esté tapada
/// o de espaldas) y que tienen un vecino de 4 conexión fuera de él. Fila por fila, `size.0` ×
/// `size.1`.
pub fn face_outline(input: &RenderInput, faces: &[(usize, usize)]) -> Vec<bool> {
    let (w, h) = input.size;
    let n = (w as usize) * (h as usize);
    let mut out = vec![false; n];
    if n == 0 || faces.is_empty() {
        return out;
    }
    let view = View::new(&input.camera.unwrap_or(input.model.camera), w, h);
    let mut mask = vec![false; n];
    for &(o, f) in faces {
        let Some(obj) = input.model.objects.get(o) else {
            continue;
        };
        let Some(face) = obj.mesh.faces.get(f).filter(|face| face.verts.len() >= 3) else {
            continue;
        };
        let world = world_vertices(obj, input.extra_yaw);
        let Some(mut poly) = face
            .verts
            .iter()
            .map(|&v| {
                world.get(v as usize).map(|&p| ClipVert {
                    p: view.to_view(p),
                    u: 0.0,
                    v: 0.0,
                })
            })
            .collect::<Option<Vec<ClipVert>>>()
        else {
            continue;
        };
        if view.focal.is_some() {
            poly = clip_near(&poly);
            if poly.len() < 3 {
                continue;
            }
        }
        let screen: Vec<ScreenVert> = poly.iter().map(|c| view.project(c)).collect();
        mask.fill(false);
        for i in 1..screen.len() - 1 {
            cover_triangle(&mut mask, w, h, &screen[0], &screen[i], &screen[i + 1]);
        }
        let (wu, hu) = (w as usize, h as usize);
        for y in 0..hu {
            for x in 0..wu {
                let i = y * wu + x;
                if !mask[i] {
                    continue;
                }
                let edge = x == 0
                    || y == 0
                    || x + 1 == wu
                    || y + 1 == hu
                    || !mask[i - 1]
                    || !mask[i + 1]
                    || !mask[i - wu]
                    || !mask[i + wu];
                if edge {
                    out[i] = true;
                }
            }
        }
    }
    out
}

/// Marca en `mask` los pixeles que cubre el triángulo (de cualquier orientación), con la regla
/// top-left del render.
fn cover_triangle(
    mask: &mut [bool],
    w: u32,
    h: u32,
    a: &ScreenVert,
    b: &ScreenVert,
    c: &ScreenVert,
) {
    let (a, mut b, mut c) = (a, b, c);
    let mut area = edge(a, b, c.x, c.y);
    if area < 0.0 {
        std::mem::swap(&mut b, &mut c);
        area = -area;
    }
    if area <= 0.0 || !area.is_finite() {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as u32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as u32;
    let max_x = (a.x.max(b.x).max(c.x).ceil().max(0.0) as u32).min(w);
    let max_y = (a.y.max(b.y).max(c.y).ceil().max(0.0) as u32).min(h);
    let tl = [top_left(b, c), top_left(c, a), top_left(a, b)];
    let inside = |v: f64, tl: bool| v > 0.0 || (v == 0.0 && tl);
    for py in min_y..max_y {
        let fy = py as f64 + 0.5;
        for px in min_x..max_x {
            let fx = px as f64 + 0.5;
            if inside(edge(b, c, fx, fy), tl[0])
                && inside(edge(c, a, fx, fy), tl[1])
                && inside(edge(a, b, fx, fy), tl[2])
            {
                mask[(py * w + px) as usize] = true;
            }
        }
    }
}

/// El color de las caras cuando no hay textura.
const GRAY: Rgba = Rgba::new(128, 128, 128, 255);
/// El plano cercano de la perspectiva, en unidades frente a la cámara.
const NEAR: f32 = 1.0 / 64.0;

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Décimas de grado a radianes.
fn rad(tenths: i32) -> f32 {
    (tenths as f32 / 10.0).to_radians()
}

/// De punto fijo (1/16) a unidades.
fn units(v: Vec3i) -> V3 {
    let f = FIXED as f32;
    [v.x as f32 / f, v.y as f32 / f, v.z as f32 / f]
}

/// Dirección unitaria con giro `yaw` alrededor de +Y (0 = hacia +Z, 900 = hacia +X) y altura
/// `pitch` hacia +Y. Sirve para la posición de la cámara y para el lado de donde viene la luz.
fn orbit_dir(yaw: i32, pitch: i32) -> V3 {
    let (y, p) = (rad(yaw), rad(pitch));
    [y.sin() * p.cos(), p.sin(), y.cos() * p.cos()]
}

/// La cámara lista para proyectar: base ortonormal y escala de pantalla.
struct View {
    eye: V3,
    right: V3,
    up: V3,
    fwd: V3,
    /// `Some(focal en pixeles)` en perspectiva; `None` en ortográfica.
    focal: Option<f32>,
    /// Pixeles por unidad en la ortográfica.
    scale: f32,
    center: (f32, f32),
}

impl View {
    fn new(cam: &Camera3D, w: u32, h: u32) -> View {
        let dist = cam.distance.max(1) as f32 / FIXED as f32;
        let dir = orbit_dir(cam.yaw, cam.pitch);
        let target = units(cam.target);
        let eye = [
            target[0] + dir[0] * dist,
            target[1] + dir[1] * dist,
            target[2] + dir[2] * dist,
        ];
        let fwd = [-dir[0], -dir[1], -dir[2]];
        let yaw = rad(cam.yaw);
        let right = [yaw.cos(), 0.0, -yaw.sin()];
        let up = cross(right, fwd);
        let half_h = h as f32 / 2.0;
        let focal = match cam.projection {
            Projection::Perspective { fov } => {
                let fov = rad(fov.clamp(10, 1700));
                Some(half_h / (fov / 2.0).tan())
            }
            Projection::Orthographic => None,
        };
        View {
            eye,
            right,
            up,
            fwd,
            focal,
            scale: h as f32 / (2.0 * dist),
            center: (w as f32 / 2.0, half_h),
        }
    }

    /// Del mundo al espacio de vista: x a la derecha, y arriba, z la profundidad hacia adelante.
    fn to_view(&self, p: V3) -> V3 {
        let d = sub(p, self.eye);
        [dot(d, self.right), dot(d, self.up), dot(d, self.fwd)]
    }

    fn project(&self, c: &ClipVert) -> ScreenVert {
        let [x, y, z] = c.p;
        let (s, iw, depth) = match self.focal {
            Some(f) => (f / z, 1.0 / z, 1.0 / z),
            None => (self.scale, 1.0, -z),
        };
        ScreenVert {
            x: snap(self.center.0 + x * s),
            y: snap(self.center.1 - y * s),
            depth,
            iw,
            uw: c.u * iw,
            vw: c.v * iw,
        }
    }
}

/// Vértice en espacio de vista con su UV (para recortar).
#[derive(Clone, Copy)]
struct ClipVert {
    p: V3,
    u: f32,
    v: f32,
}

/// Vértice en pantalla (y hacia abajo). `depth` es afín en pantalla y mayor = más cerca; `iw`,
/// `uw`, `vw` son 1/w, u/w y v/w para la corrección de perspectiva.
#[derive(Clone, Copy)]
struct ScreenVert {
    x: f64,
    y: f64,
    depth: f32,
    iw: f32,
    uw: f32,
    vw: f32,
}

/// Recorta el polígono contra `z >= NEAR` (Sutherland–Hodgman).
fn clip_near(poly: &[ClipVert]) -> Vec<ClipVert> {
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let (ina, inb) = (a.p[2] >= NEAR, b.p[2] >= NEAR);
        if ina {
            out.push(a);
        }
        if ina != inb {
            let t = (NEAR - a.p[2]) / (b.p[2] - a.p[2]);
            let l = |x: f32, y: f32| x + (y - x) * t;
            out.push(ClipVert {
                p: [l(a.p[0], b.p[0]), l(a.p[1], b.p[1]), NEAR],
                u: l(a.u, b.u),
                v: l(a.v, b.v),
            });
        }
    }
    out
}

/// Los vértices del objeto en el mundo: escala → rotación X, Y (+ `extra_yaw`), Z → traslación.
fn world_vertices(obj: &Object3D, extra_yaw: i32) -> Vec<V3> {
    let t = &obj.transform;
    let s = [
        t.scale.x as f32 / 1000.0,
        t.scale.y as f32 / 1000.0,
        t.scale.z as f32 / 1000.0,
    ];
    let (sx, cx) = rad(t.rotation.x).sin_cos();
    let (sy, cy) = rad(t.rotation.y + extra_yaw).sin_cos();
    let (sz, cz) = rad(t.rotation.z).sin_cos();
    let pos = units(t.position);
    obj.mesh
        .vertices
        .iter()
        .map(|&v| {
            let u = units(v);
            let (x, y, z) = (u[0] * s[0], u[1] * s[1], u[2] * s[2]);
            let (y, z) = (y * cx - z * sx, y * sx + z * cx);
            let (x, z) = (x * cy + z * sy, -x * sy + z * cy);
            let (x, y) = (x * cz - y * sz, x * sz + y * cz);
            [x + pos[0], y + pos[1], z + pos[2]]
        })
        .collect()
}

/// Normal (sin normalizar) del polígono por el método de Newell.
fn newell(pts: &[V3]) -> V3 {
    let mut n = [0.0; 3];
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}

/// Subpixeles por pixel de la rejilla de pantalla.
const SUBPIXELS: f64 = 256.0;

/// Ajusta una coordenada de pantalla a la rejilla de 1/256 de pixel: así las funciones de borde
/// en `f64` son exactas y un borde compartido (la diagonal de un cuadrilátero) no deja huecos.
fn snap(v: f32) -> f64 {
    ((v as f64).clamp(-1e7, 1e7) * SUBPIXELS).round() / SUBPIXELS
}

/// Función de borde: positiva si `p` queda del lado interior de `a→b` (y hacia abajo, triángulo
/// orientado con área positiva).
fn edge(a: &ScreenVert, b: &ScreenVert, px: f64, py: f64) -> f64 {
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

/// Regla top-left: un borde de arriba (horizontal hacia la derecha) o de la izquierda (hacia
/// arriba) se queda con los pixeles que caen justo sobre él.
fn top_left(a: &ScreenVert, b: &ScreenVert) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    (dy == 0.0 && dx > 0.0) || dy < 0.0
}

struct Raster {
    w: u32,
    h: u32,
    color: Vec<Rgba>,
    depth: Vec<f32>,
    /// `(objeto, cara)` visibles en cada pixel.
    ids: Vec<Option<(usize, usize)>>,
}

/// Lo que comparte cada pixel de una cara.
struct FaceCtx<'a> {
    /// `(objeto, cara)`.
    id: (usize, usize),
    steps: u8,
    texture: Option<&'a Texture<'a>>,
    palette: &'a [Rgba],
    shading: &'a ShadingTable,
}

impl Raster {
    fn triangle(
        &mut self,
        a: &ScreenVert,
        b: &ScreenVert,
        c: &ScreenVert,
        ctx: &FaceCtx,
        cache: &mut HashMap<(Rgba, u8), Rgba>,
    ) {
        let (a, mut b, mut c) = (a, b, c);
        let mut area = edge(a, b, c.x, c.y);
        if area < 0.0 {
            std::mem::swap(&mut b, &mut c);
            area = -area;
        }
        if area <= 0.0 || !area.is_finite() {
            return;
        }
        let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as u32;
        let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as u32;
        let max_x = (a.x.max(b.x).max(c.x).ceil().max(0.0) as u32).min(self.w);
        let max_y = (a.y.max(b.y).max(c.y).ceil().max(0.0) as u32).min(self.h);
        let tl = [top_left(b, c), top_left(c, a), top_left(a, b)];
        let inside = |w: f64, tl: bool| w > 0.0 || (w == 0.0 && tl);
        for py in min_y..max_y {
            let fy = py as f64 + 0.5;
            for px in min_x..max_x {
                let fx = px as f64 + 0.5;
                let w0 = edge(b, c, fx, fy);
                let w1 = edge(c, a, fx, fy);
                let w2 = edge(a, b, fx, fy);
                if !(inside(w0, tl[0]) && inside(w1, tl[1]) && inside(w2, tl[2])) {
                    continue;
                }
                let (l0, l1, l2) = ((w0 / area) as f32, (w1 / area) as f32, (w2 / area) as f32);
                let i = (py * self.w + px) as usize;
                let depth = l0 * a.depth + l1 * b.depth + l2 * c.depth;
                if depth <= self.depth[i] {
                    continue;
                }
                let iw = l0 * a.iw + l1 * b.iw + l2 * c.iw;
                let u = (l0 * a.uw + l1 * b.uw + l2 * c.uw) / iw;
                let v = (l0 * a.vw + l1 * b.vw + l2 * c.vw) / iw;
                let texel = sample(ctx.texture, u, v);
                if texel.a == 0 {
                    continue;
                }
                let color = *cache
                    .entry((texel, ctx.steps))
                    .or_insert_with(|| shade(ctx.palette, ctx.shading, texel, ctx.steps as u32));
                self.color[i] = color;
                self.depth[i] = depth;
                self.ids[i] = Some(ctx.id);
            }
        }
    }
}

/// Muestreo *nearest*: el texel `floor(u), floor(v)`, repetido con `rem_euclid`.
fn sample(texture: Option<&Texture>, u: f32, v: f32) -> Rgba {
    let Some(t) = texture else {
        return GRAY;
    };
    let x = (u.floor() as i64).rem_euclid(t.width as i64);
    let y = (v.floor() as i64).rem_euclid(t.height as i64);
    t.pixels[(y * t.width as i64 + x) as usize]
}

fn rasterize(input: &RenderInput) -> Raster {
    let (w, h) = input.size;
    let n = (w as usize) * (h as usize);
    let mut r = Raster {
        w,
        h,
        color: vec![Rgba::TRANSPARENT; n],
        depth: vec![f32::NEG_INFINITY; n],
        ids: vec![None; n],
    };
    if n == 0 {
        return r;
    }
    let model = input.model;
    let view = View::new(&input.camera.unwrap_or(model.camera), w, h);
    // Una textura vacía o incompleta cuenta como si no hubiera.
    let texture = input.texture.as_ref().filter(|t| {
        t.width > 0 && t.height > 0 && t.pixels.len() >= (t.width as usize) * (t.height as usize)
    });
    let light = model
        .light
        .enabled
        .then(|| orbit_dir(model.light.yaw, model.light.pitch));
    let mut cache = HashMap::new();
    for (id, obj) in model.objects.iter().enumerate() {
        let world = world_vertices(obj, input.extra_yaw);
        let s = obj.transform.scale;
        // Una escala negativa en un número impar de ejes invierte el sentido de las caras.
        let mirrored = (s.x < 0) ^ (s.y < 0) ^ (s.z < 0);
        for (face_i, face) in obj.mesh.faces.iter().enumerate() {
            if face.verts.len() < 3 || face.uvs.len() != face.verts.len() {
                continue;
            }
            let Some(pts) = face
                .verts
                .iter()
                .map(|&v| world.get(v as usize).copied())
                .collect::<Option<Vec<V3>>>()
            else {
                continue;
            };
            let mut normal = newell(&pts);
            if mirrored {
                normal = [-normal[0], -normal[1], -normal[2]];
            }
            let len = dot(normal, normal).sqrt();
            if len <= 0.0 {
                continue;
            }
            let normal = [normal[0] / len, normal[1] / len, normal[2] / len];
            let mut poly: Vec<ClipVert> = pts
                .iter()
                .zip(&face.uvs)
                .map(|(&p, &(u, v))| ClipVert {
                    p: view.to_view(p),
                    u: u as f32,
                    v: v as f32,
                })
                .collect();
            if view.focal.is_some() {
                poly = clip_near(&poly);
                if poly.len() < 3 {
                    continue;
                }
            }
            let screen: Vec<ScreenVert> = poly.iter().map(|c| view.project(c)).collect();
            // Área con y hacia abajo: una cara antihoraria vista de frente da negativa.
            let mut area = 0.0;
            for i in 0..screen.len() {
                let (a, b) = (&screen[i], &screen[(i + 1) % screen.len()]);
                area += a.x * b.y - b.x * a.y;
            }
            if mirrored {
                area = -area;
            }
            let front = area < 0.0;
            if area == 0.0 || (!front && !obj.double_sided) {
                continue;
            }
            let steps = match light {
                None => 0,
                Some(to_light) => {
                    let d = dot(normal, to_light) * if front { 1.0 } else { -1.0 };
                    if d > 0.5 {
                        0
                    } else if d > 0.0 {
                        1
                    } else {
                        2
                    }
                }
            };
            let ctx = FaceCtx {
                id: (id, face_i),
                steps,
                texture,
                palette: input.palette,
                shading: &model.shading,
            };
            for i in 1..screen.len() - 1 {
                r.triangle(&screen[0], &screen[i], &screen[i + 1], &ctx, &mut cache);
            }
        }
    }
    r
}

/// Pinta de `color` todo pixel transparente con un vecino opaco de 4 conexión.
fn outline(img: &mut [Rgba], w: u32, h: u32, color: Rgba) {
    let (w, h) = (w as usize, h as usize);
    let opaque: Vec<bool> = img.iter().map(|p| p.a > 0).collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if opaque[i] {
                continue;
            }
            let near = (x > 0 && opaque[i - 1])
                || (x + 1 < w && opaque[i + 1])
                || (y > 0 && opaque[i - w])
                || (y + 1 < h && opaque[i + w]);
            if near {
                img[i] = color;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Primitive, darker, primitive_mesh};
    use cs_core::{
        CameraPreset, FIXED, Face, Light3D, Mesh, Object3D, Projection, Transform3D, Vec3i,
    };

    const ROJO: Rgba = Rgba::new(255, 0, 0, 255);
    const AZUL: Rgba = Rgba::new(0, 0, 255, 255);
    const BLANCO: Rgba = Rgba::new(255, 255, 255, 255);
    const CLARO: Rgba = Rgba::new(180, 180, 180, 255);
    const OSCURO: Rgba = Rgba::new(90, 90, 90, 255);
    const NEGRO: Rgba = Rgba::new(0, 0, 0, 255);

    fn obj(mesh: Mesh) -> Object3D {
        Object3D {
            name: "o".into(),
            mesh,
            transform: Transform3D::default(),
            double_sided: false,
            kind: None,
        }
    }

    /// Un modelo sin luz con la cámara Front a 2 unidades: en 16×16 caben 4 unidades de alto
    /// (4 pixeles por unidad), así que el cubo de ±1 cubre los pixeles 4..12.
    fn model(objects: Vec<Object3D>) -> Model3D {
        Model3D {
            objects,
            camera: CameraPreset::Front.camera(2 * FIXED, Vec3i::default()),
            light: Light3D {
                enabled: false,
                ..Light3D::default()
            },
            ..Model3D::default()
        }
    }

    fn input<'a>(m: &'a Model3D, size: u32, tex: &'a [Rgba], tw: u32) -> RenderInput<'a> {
        RenderInput {
            model: m,
            size: (size, size),
            texture: Some(Texture {
                width: tw,
                height: tex.len() as u32 / tw,
                pixels: tex,
            }),
            palette: &[BLANCO, CLARO, OSCURO, NEGRO],
            camera: None,
            extra_yaw: 0,
        }
    }

    fn cube() -> Object3D {
        obj(primitive_mesh(&Primitive::Cube))
    }

    #[test]
    fn cubo_de_frente_cubre_un_cuadrado_centrado() {
        let m = model(vec![cube()]);
        let img = render_model(&input(&m, 16, &[ROJO], 1));
        assert_eq!(img.len(), 256);
        for y in 0..16 {
            for x in 0..16 {
                let p = img[y * 16 + x];
                let dentro = (4..12).contains(&x) && (4..12).contains(&y);
                if dentro {
                    assert_eq!(p, ROJO, "({x},{y})");
                } else {
                    assert_eq!(p.a, 0, "({x},{y})");
                }
            }
        }
    }

    #[test]
    fn sin_textura_es_gris_medio() {
        let m = model(vec![cube()]);
        let mut inp = input(&m, 16, &[ROJO], 1);
        inp.texture = None;
        let img = render_model(&inp);
        assert_eq!(img[8 * 16 + 8], Rgba::new(128, 128, 128, 255));
    }

    #[test]
    fn luz_en_iso_da_tres_pasos() {
        let mut m = model(vec![cube()]);
        m.camera = CameraPreset::Iso.camera(2 * FIXED, Vec3i::default());
        // Luz desde la derecha, un poco desde arriba y desde atrás: la cara derecha recibe
        // de lleno (0 pasos), la de arriba de lado (1) y la de frente nada (2).
        m.light = Light3D {
            enabled: true,
            yaw: 1000,
            pitch: 200,
        };
        let inp = input(&m, 32, &[BLANCO], 1);
        let img = render_model(&inp);
        let mut colores: Vec<Rgba> = img.iter().copied().filter(|p| p.a > 0).collect();
        colores.sort_by_key(|c| (c.r, c.g, c.b));
        colores.dedup();
        let uno = darker(inp.palette, BLANCO);
        let dos = darker(inp.palette, uno);
        assert_eq!((uno, dos), (CLARO, OSCURO));
        assert_eq!(colores, vec![OSCURO, CLARO, BLANCO]);
        // La cara derecha queda a la derecha del centro, la de frente a la izquierda.
        assert_eq!(img[18 * 32 + 22], BLANCO);
        assert_eq!(img[18 * 32 + 10], OSCURO);
        assert_eq!(img[8 * 32 + 16], CLARO);
    }

    #[test]
    fn tabla_de_shading_cambia_solo_su_paso() {
        let mut m = model(vec![cube()]);
        m.camera = CameraPreset::Iso.camera(2 * FIXED, Vec3i::default());
        m.light = Light3D {
            enabled: true,
            yaw: 1000,
            pitch: 200,
        };
        let antes = render_model(&input(&m, 32, &[BLANCO], 1));
        m.shading.set(BLANCO, 1, Some(ROJO));
        let despues = render_model(&input(&m, 32, &[BLANCO], 1));
        // Cara de 1 paso (arriba): ahora roja. Caras de 0 y 2 pasos: igual.
        assert_eq!(antes[8 * 32 + 16], CLARO);
        assert_eq!(despues[8 * 32 + 16], ROJO);
        assert_eq!(despues[18 * 32 + 22], BLANCO);
        assert_eq!(despues[18 * 32 + 10], OSCURO);
    }

    fn plane_2x1() -> Mesh {
        let f = FIXED;
        let v = |x, y| Vec3i { x, y, z: 0 };
        Mesh {
            vertices: vec![v(-f, -f), v(f, -f), v(f, f), v(-f, f)],
            faces: vec![Face {
                verts: vec![0, 1, 2, 3],
                uvs: vec![(0, 1), (2, 1), (2, 0), (0, 0)],
            }],
        }
    }

    #[test]
    fn plano_por_detras_no_se_ve_salvo_doble_cara() {
        let m = model(vec![obj(plane_2x1())]);
        let mut doble = m.clone();
        doble.objects[0].double_sided = true;
        let mut inp = input(&m, 16, &[ROJO], 1);
        let mut back = CameraPreset::Front.camera(2 * FIXED, Vec3i::default());
        back.yaw = 1800;
        inp.camera = Some(back);
        assert!(render_model(&inp).iter().all(|p| p.a == 0));
        inp.model = &doble;
        assert_eq!(render_model(&inp)[8 * 16 + 8], ROJO);
    }

    #[test]
    fn extra_yaw_gira_los_objetos() {
        let m = model(vec![obj(plane_2x1())]);
        let mut inp = input(&m, 16, &[ROJO], 1);
        assert_eq!(render_model(&inp)[8 * 16 + 8], ROJO);
        inp.extra_yaw = 1800;
        assert!(render_model(&inp).iter().all(|p| p.a == 0));
    }

    #[test]
    fn contorno_pinta_los_vecinos() {
        let mut m = model(vec![cube()]);
        m.outline = Some(NEGRO);
        let img = render_model(&input(&m, 16, &[ROJO], 1));
        assert_eq!(img[8 * 16 + 3], NEGRO);
        assert_eq!(img[3 * 16 + 8], NEGRO);
        assert_eq!(img[8 * 16 + 8], ROJO);
        // En diagonal no es vecino de 4 conexión, y lejos tampoco.
        assert_eq!(img[3 * 16 + 3].a, 0);
        assert_eq!(img[0].a, 0);
    }

    #[test]
    fn uv_de_textura_2x1() {
        let m = model(vec![obj(plane_2x1())]);
        let img = render_model(&input(&m, 16, &[ROJO, AZUL], 2));
        for y in 4..12 {
            for x in 4..8 {
                assert_eq!(img[y * 16 + x], ROJO, "({x},{y})");
            }
            for x in 8..12 {
                assert_eq!(img[y * 16 + x], AZUL, "({x},{y})");
            }
        }
    }

    #[test]
    fn texel_transparente_no_pinta() {
        let m = model(vec![obj(plane_2x1())]);
        let img = render_model(&input(&m, 16, &[ROJO, Rgba::TRANSPARENT], 2));
        assert_eq!(img[8 * 16 + 5], ROJO);
        assert_eq!(img[8 * 16 + 10].a, 0);
    }

    #[test]
    fn zbuffer_gana_lo_mas_cercano() {
        // Un plano azul adelante (z = +2) tapa al cubo rojo del centro, sin importar el orden.
        // Todos los UV del plano caen en el texel 0 (azul) y los del cubo en el 1 (rojo).
        let con_uv = |mut o: Object3D, uv: (i32, i32)| {
            for f in &mut o.mesh.faces {
                f.uvs.iter_mut().for_each(|t| *t = uv);
            }
            o
        };
        let mut cerca = con_uv(obj(plane_2x1()), (0, 0));
        cerca.transform.position.z = 2 * FIXED;
        let mut m = model(vec![cerca, con_uv(cube(), (1, 0))]);
        m.camera.distance = 4 * FIXED;
        let tex = [AZUL, ROJO];
        let img = render_model(&input(&m, 32, &tex, 2));
        assert_eq!(img[16 * 32 + 16], AZUL);
        m.objects.reverse();
        let img = render_model(&input(&m, 32, &tex, 2));
        assert_eq!(img[16 * 32 + 16], AZUL);
        // Sin el plano se ve el cubo.
        m.objects.truncate(1);
        let img = render_model(&input(&m, 32, &tex, 2));
        assert_eq!(img[16 * 32 + 16], ROJO);
    }

    #[test]
    fn perspectiva_de_frente_centrada() {
        let mut m = model(vec![cube()]);
        m.camera.projection = Projection::Perspective { fov: 600 };
        m.camera.distance = 4 * FIXED;
        let img = render_model(&input(&m, 16, &[ROJO], 1));
        // La cara de frente (z = 3 de la cámara) mide 2·focal/3 ≈ 9.2 px: cubre 3..=12 sin
        // huecos en la diagonal que comparten sus dos triángulos.
        for y in 3..13 {
            for x in 3..13 {
                assert_eq!(img[y * 16 + x], ROJO, "({x},{y})");
            }
        }
        assert_eq!(img[8 * 16 + 8], ROJO);
        assert_eq!(img[0].a, 0);
        // Simétrica de izquierda a derecha y de arriba abajo.
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(img[y * 16 + x].a, img[y * 16 + 15 - x].a, "({x},{y})");
                assert_eq!(img[y * 16 + x].a, img[(15 - y) * 16 + x].a, "({x},{y})");
            }
        }
    }

    #[test]
    fn camara_detras_del_objeto_no_revienta() {
        // La cámara dentro del cubo, en perspectiva: recorta contra el plano cercano.
        let mut m = model(vec![cube()]);
        m.camera.projection = Projection::Perspective { fov: 600 };
        m.camera.distance = FIXED / 2;
        let img = render_model(&input(&m, 16, &[ROJO], 1));
        assert_eq!(img.len(), 256);
    }

    #[test]
    fn pick_devuelve_el_objeto_del_centro() {
        let mut lejos = cube();
        lejos.transform.position.x = 20 * FIXED;
        let m = model(vec![lejos, cube()]);
        let inp = input(&m, 16, &[ROJO], 1);
        assert_eq!(pick_object(&inp, 8, 8), Some(1));
        assert_eq!(pick_object(&inp, 0, 0), None);
        assert_eq!(pick_object(&inp, 99, 99), None);
    }

    #[test]
    fn pick_face_da_la_cara_de_enfrente() {
        let m = model(vec![cube()]);
        let inp = input(&m, 32, &[ROJO], 1);
        let (o, f) = pick_face(&inp, 16, 16).expect("hay cara en el centro");
        assert_eq!(o, 0);
        let mesh = &m.objects[0].mesh;
        assert!(
            mesh.faces[f]
                .verts
                .iter()
                .all(|&v| mesh.vertices[v as usize].z > 0),
            "la cara elegida debe ser la de z+"
        );
        assert_eq!(pick_face(&inp, 0, 0), None);
        assert_eq!(pick_face(&inp, 99, 99), None);
    }

    #[test]
    fn contorno_de_la_cara_de_enfrente() {
        let m = model(vec![cube()]);
        let inp = input(&m, 16, &[ROJO], 1);
        let front = pick_face(&inp, 8, 8).expect("la cara de enfrente");
        assert!(face_outline(&inp, &[]).iter().all(|on| !on));
        let out = face_outline(&inp, &[front]);
        let at = |x: usize, y: usize| out[y * 16 + x];
        // La cara cubre 4..12: el contorno es su primera y su última fila y columna.
        for k in 4..12 {
            assert!(
                at(k, 4) && at(k, 11) && at(4, k) && at(11, k),
                "borde en {k}"
            );
        }
        assert!(!at(8, 8) && !at(5, 5) && !at(10, 10), "adentro no");
        assert!(
            !at(3, 8) && !at(12, 8) && !at(8, 3) && !at(8, 12),
            "afuera no"
        );
        assert_eq!(out.iter().filter(|&&on| on).count(), 28);
    }
}
