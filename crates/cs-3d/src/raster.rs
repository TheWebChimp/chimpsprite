//! El rasterizador pixel de los modelos 3D.

use std::collections::HashMap;

use cs_core::{Camera3D, FIXED, Model3D, Projection, Rgba, ShadingTable, Vec3i};

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
    /// El cuadro de referencia de la proyección, si no es `size`: su alto da la escala (la
    /// ortográfica) o la focal (la perspectiva), y va centrado en la imagen (corrido a pixeles
    /// enteros). Así el render de `frame` es justo el recorte del centro de uno más grande: la
    /// vista se agranda hasta los bordes sin cambiar de escala. `None`: el cuadro es `size`.
    pub frame: Option<(u32, u32)>,
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
/// Un objeto bloqueado se ve (y tapa lo de atrás) pero no se elige.
pub fn pick_object(input: &RenderInput, x: u32, y: u32) -> Option<usize> {
    pick_face(input, x, y).map(|(o, _)| o)
}

/// La cara `(objeto, índice de cara)` que se ve en el pixel `(x, y)`, si hay alguna (y su objeto
/// no está bloqueado).
pub fn pick_face(input: &RenderInput, x: u32, y: u32) -> Option<(usize, usize)> {
    if x >= input.size.0 || y >= input.size.1 {
        return None;
    }
    let r = rasterize(input);
    r.ids[(y * input.size.0 + x) as usize].filter(|&(o, _)| !input.model.object_locked(o))
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
    let view = View::new(
        &input.camera.unwrap_or(input.model.camera),
        w,
        h,
        input.frame,
    );
    let mut mask = vec![false; n];
    for &(o, f) in faces {
        let Some(obj) = input.model.objects.get(o) else {
            continue;
        };
        if input.model.object_hidden(o) {
            continue;
        }
        let Some(face) = obj.mesh.faces.get(f).filter(|face| face.verts.len() >= 3) else {
            continue;
        };
        let world = world_vertices(input.model, o, input.extra_yaw);
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
#[derive(Clone, Copy, Debug)]
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
    /// La vista de `cam` en una imagen de `w`×`h` con el cuadro `frame` (ver
    /// [`RenderInput::frame`]; `None`: el cuadro es la imagen).
    fn new(cam: &Camera3D, w: u32, h: u32, frame: Option<(u32, u32)>) -> View {
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
        let (fw, fh) = frame.unwrap_or((w, h));
        let half_h = fh as f32 / 2.0;
        // El cuadro en el centro de la imagen, corrido a pixeles enteros: su render es el
        // recorte exacto del de la imagen.
        let shift = |n: u32, f: u32| (n as i64 - f as i64).div_euclid(2) as f32;
        let center = (fw as f32 / 2.0 + shift(w, fw), half_h + shift(h, fh));
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
            scale: fh as f32 / (2.0 * dist),
            center,
        }
    }

    /// Del mundo al espacio de vista: x a la derecha, y arriba, z la profundidad hacia adelante.
    fn to_view(self, p: V3) -> V3 {
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

/// Lleva puntos del mundo (en 1/16 de unidad, como los vértices) a la imagen del render de una
/// cámara, con la misma cuenta que el rasterizador: en pixeles del render, y hacia abajo (el
/// centro del pixel `(x, y)` es `(x + 0.5, y + 0.5)`). Sirve para dibujar encima del render.
#[derive(Clone, Copy, Debug)]
pub struct Projector {
    view: View,
}

impl Projector {
    /// El proyector de `camera` en un render de `size`.
    pub fn new(camera: &Camera3D, size: (u32, u32)) -> Projector {
        Projector {
            view: View::new(camera, size.0, size.1, None),
        }
    }

    /// Dónde cae `p` en el render; `None` en perspectiva si queda detrás del plano cercano.
    pub fn project(&self, p: crate::P3) -> Option<(f32, f32)> {
        let f = FIXED as f32;
        let v = self.view.to_view([p[0] / f, p[1] / f, p[2] / f]);
        if self.view.focal.is_some() && v[2] < NEAR {
            return None;
        }
        let s = self.view.project(&ClipVert {
            p: v,
            u: 0.0,
            v: 0.0,
        });
        Some((s.x as f32, s.y as f32))
    }

    /// Qué tan lejos queda `p` frente a la cámara (en unidades; más es más lejos).
    pub fn depth(&self, p: crate::P3) -> f32 {
        let f = FIXED as f32;
        self.view.to_view([p[0] / f, p[1] / f, p[2] / f])[2]
    }

    /// La base de la cámara en el mundo (unitaria): derecha, arriba y hacia adonde mira.
    pub fn axes(&self) -> [[f32; 3]; 3] {
        [self.view.right, self.view.up, self.view.fwd]
    }

    /// El rayo del punto `s` del render: su origen (en el plano de la cámara) y su dirección
    /// (hacia adelante), en 1/16 de unidad. Los puntos del rayo caen en `s`.
    pub fn ray(&self, s: (f32, f32)) -> (crate::P3, crate::P3) {
        let v = &self.view;
        let (x, y) = (s.0 - v.center.0, v.center.1 - s.1);
        let f = FIXED as f32;
        let at = |o: V3, d: V3| {
            (
                std::array::from_fn(|i| o[i] * f),
                std::array::from_fn(|i| d[i] * f),
            )
        };
        match v.focal {
            Some(focal) => {
                let d = std::array::from_fn(|i| v.fwd[i] + (v.right[i] * x + v.up[i] * y) / focal);
                at(v.eye, d)
            }
            None => {
                let o =
                    std::array::from_fn(|i| v.eye[i] + (v.right[i] * x + v.up[i] * y) / v.scale);
                at(o, v.fwd)
            }
        }
    }
}

/// Dónde cae `p` (en 1/16) en el render de `size` con `camera` (ver [`Projector`]).
pub fn project_point(camera: &Camera3D, size: (u32, u32), p: crate::P3) -> Option<(f32, f32)> {
    Projector::new(camera, size).project(p)
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

/// Los vértices del objeto `o` en el mundo (en unidades): por su transform y los de sus grupos
/// (escala → rotación X, Y, Z → traslación cada uno). `extra_yaw` se suma a la rotación Y del de
/// más afuera (sin grupos, el del objeto): así gira todo el modelo.
fn world_vertices(model: &Model3D, o: usize, extra_yaw: i32) -> Vec<V3> {
    let mut chain = model.object_chain(o);
    if let Some(t) = chain.last_mut() {
        t.rotation.y += extra_yaw;
    }
    let f = FIXED as f32;
    model.objects[o]
        .mesh
        .vertices
        .iter()
        .map(|&v| {
            let p = chain
                .iter()
                .fold(crate::edit::p3(v), |p, t| crate::edit::to_world(t, p));
            [p[0] / f, p[1] / f, p[2] / f]
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
    /// Con tramado: el nivel de sombra de la cara entre 0 y 2 (sin él, `steps` redondeado), que
    /// cada pixel pasa a un paso con la matriz de Bayer.
    level: Option<f32>,
    /// En un cuadrilátero que no es paralelogramo: las UVs de cada pixel, bilineales sobre la
    /// cara entera (si no, por triángulo).
    quad: Option<(QuadMap, &'a View)>,
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
                let quad = ctx
                    .quad
                    .as_ref()
                    .and_then(|(q, view)| q.uv_at(view, fx, fy));
                let (u, v) = quad.unwrap_or_else(|| {
                    let iw = l0 * a.iw + l1 * b.iw + l2 * c.iw;
                    (
                        (l0 * a.uw + l1 * b.uw + l2 * c.uw) / iw,
                        (l0 * a.vw + l1 * b.vw + l2 * c.vw) / iw,
                    )
                });
                let texel = sample(ctx.texture, u, v);
                if texel.a == 0 {
                    continue;
                }
                let steps = match ctx.level {
                    Some(level) => dithered_steps(level, px, py),
                    None => ctx.steps,
                };
                let color = *cache
                    .entry((texel, steps))
                    .or_insert_with(|| shade(ctx.palette, ctx.shading, texel, steps as u32));
                self.color[i] = color;
                self.depth[i] = depth;
                self.ids[i] = Some(ctx.id);
            }
        }
    }
}

/// El nivel de sombra de una cara (de 0, de frente a la luz, a 2, de espaldas) por el coseno `d`
/// entre su normal y la luz: redondeado da los pasos de siempre (0 si d > 0.5, 1 si d > 0, 2 si
/// no); con tramado, sus decimales son cuántos pixeles van al paso siguiente.
fn shade_level(d: f32) -> f32 {
    ((0.75 - d) * 2.0).clamp(0.0, 2.0)
}

/// La matriz de Bayer de 4×4 (los umbrales del tramado ordenado).
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// El paso de luz del pixel `(px, py)` de una cara de nivel `level`: el de abajo o el de arriba
/// según su umbral de Bayer (fijo en pantalla). Un nivel entero queda parejo.
fn dithered_steps(level: f32, px: u32, py: u32) -> u8 {
    let threshold = (BAYER[(py % 4) as usize][(px % 4) as usize] as f32 + 0.5) / 16.0;
    (level + threshold - 0.5).round().clamp(0.0, 2.0) as u8
}

/// Las UVs bilineales de un cuadrilátero: cada pixel cae en un punto de la cara (el rayo de la
/// cámara contra su plano), que da sus parámetros `(s, t)` sobre las cuatro esquinas (la
/// bilineal inversa) y con ellos la mezcla de las cuatro UVs. Partida en dos triángulos, una cara
/// que no es paralelogramo (un trapecio) dobla la textura en la diagonal; así un diseño simétrico
/// se ve simétrico.
#[derive(Clone, Copy)]
struct QuadMap {
    /// Una esquina, la normal y dos ejes del plano de la cara (en el espacio de vista).
    origin: V3,
    normal: V3,
    axes: [V3; 2],
    /// Las esquinas en esos ejes y sus UVs.
    corners: [(f32, f32); 4],
    uvs: [(f32, f32); 4],
}

impl QuadMap {
    /// Para una cara de cuatro vértices (`pts`, en el mundo) que no sea un paralelogramo, ni en
    /// el espacio ni en sus UVs (en uno, la bilineal es la misma que por triángulos).
    fn new(pts: &[V3], uvs: &[(i32, i32)], view: &View) -> Option<QuadMap> {
        if pts.len() != 4 || uvs.len() != 4 {
            return None;
        }
        let q: Vec<V3> = pts.iter().map(|&p| view.to_view(p)).collect();
        let uvs: [(f32, f32); 4] = std::array::from_fn(|i| (uvs[i].0 as f32, uvs[i].1 as f32));
        let n = newell(&q);
        let len = dot(n, n).sqrt();
        let e = sub(q[1], q[0]);
        let el = dot(e, e).sqrt();
        if len <= 0.0 || el <= 0.0 {
            return None;
        }
        let normal = [n[0] / len, n[1] / len, n[2] / len];
        let e1 = [e[0] / el, e[1] / el, e[2] / el];
        let e2 = cross(normal, e1);
        let corners: [(f32, f32); 4] = std::array::from_fn(|i| {
            let d = sub(q[i], q[0]);
            (dot(d, e1), dot(d, e2))
        });
        // a − b + c − d: cero en un paralelogramo.
        let skew = |p: &[(f32, f32); 4]| {
            let g = (
                p[0].0 - p[1].0 + p[2].0 - p[3].0,
                p[0].1 - p[1].1 + p[2].1 - p[3].1,
            );
            let size = (p[2].0 - p[0].0).abs() + (p[2].1 - p[0].1).abs() + 1e-6;
            (g.0.abs() + g.1.abs()) / size
        };
        if skew(&corners) < 1e-3 && skew(&uvs) < 1e-3 {
            return None;
        }
        Some(QuadMap {
            origin: q[0],
            normal,
            axes: [e1, e2],
            corners,
            uvs,
        })
    }

    /// Las UVs en el punto de pantalla `(fx, fy)`, o `None` si el rayo no corta la cara (de
    /// canto) o no se encuentran sus parámetros.
    fn uv_at(&self, view: &View, fx: f64, fy: f64) -> Option<(f32, f32)> {
        let sx = fx as f32 - view.center.0;
        let sy = view.center.1 - fy as f32;
        let (o, d) = match view.focal {
            Some(f) => ([0.0; 3], [sx / f, sy / f, 1.0]),
            None => ([sx / view.scale, sy / view.scale, 0.0], [0.0, 0.0, 1.0]),
        };
        let den = dot(d, self.normal);
        if den.abs() < 1e-6 {
            return None;
        }
        let k = dot(sub(self.origin, o), self.normal) / den;
        let hit = [o[0] + d[0] * k, o[1] + d[1] * k, o[2] + d[2] * k];
        let h = sub(hit, self.origin);
        let p = (dot(h, self.axes[0]), dot(h, self.axes[1]));
        let (s, t) = inverse_bilinear(p, &self.corners)?;
        let [a, b, c, d] = self.uvs;
        let mix = |a: f32, b: f32, c: f32, d: f32| {
            (1.0 - s) * (1.0 - t) * a + s * (1.0 - t) * b + s * t * c + (1.0 - s) * t * d
        };
        Some((mix(a.0, b.0, c.0, d.0), mix(a.1, b.1, c.1, d.1)))
    }
}

/// Los parámetros `(s, t)` de `p` en el cuadrilátero `q` (en orden alrededor):
/// `p = q0 + s·(q1 − q0) + t·(q3 − q0) + s·t·(q0 − q1 + q2 − q3)`, recortados a `[0, 1]`.
fn inverse_bilinear(p: (f32, f32), q: &[(f32, f32); 4]) -> Option<(f32, f32)> {
    let cross2 = |a: (f32, f32), b: (f32, f32)| a.0 * b.1 - a.1 * b.0;
    let e = (q[1].0 - q[0].0, q[1].1 - q[0].1);
    let f = (q[3].0 - q[0].0, q[3].1 - q[0].1);
    let g = (
        q[0].0 - q[1].0 + q[2].0 - q[3].0,
        q[0].1 - q[1].1 + q[2].1 - q[3].1,
    );
    let h = (p.0 - q[0].0, p.1 - q[0].1);
    let k2 = cross2(g, f);
    let k1 = cross2(e, f) + cross2(h, g);
    let k0 = cross2(h, e);
    // `s` sale de `t` con el componente de denominador más grande.
    let s_of = |t: f32| {
        let (dx, dy) = (e.0 + g.0 * t, e.1 + g.1 * t);
        if dx.abs() >= dy.abs() {
            (h.0 - f.0 * t) / dx
        } else {
            (h.1 - f.1 * t) / dy
        }
    };
    let ok = |s: f32, t: f32| s.is_finite() && t.is_finite();
    let near = |x: f32| (-0.01..=1.01).contains(&x);
    let (s, t) = if k2.abs() < 1e-6 * (k1.abs() + 1.0) {
        let t = -k0 / k1;
        (s_of(t), t)
    } else {
        let disc = k1 * k1 - 4.0 * k0 * k2;
        if disc < 0.0 {
            return None;
        }
        let root = disc.sqrt();
        let t1 = (-k1 - root) / (2.0 * k2);
        let t2 = (-k1 + root) / (2.0 * k2);
        let (s1, s2) = (s_of(t1), s_of(t2));
        if ok(s1, t1) && near(s1) && near(t1) {
            (s1, t1)
        } else {
            (s2, t2)
        }
    };
    ok(s, t).then(|| (s.clamp(0.0, 1.0), t.clamp(0.0, 1.0)))
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
    let view = View::new(&input.camera.unwrap_or(model.camera), w, h, input.frame);
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
        if model.object_hidden(id) {
            continue;
        }
        let world = world_vertices(model, id, input.extra_yaw);
        // Una escala negativa en un número impar de ejes (de toda la cadena) invierte el sentido
        // de las caras.
        let mirrored = model.object_chain(id).iter().fold(false, |m, t| {
            let s = t.scale;
            m ^ (s.x < 0) ^ (s.y < 0) ^ (s.z < 0)
        });
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
            let level = light.map(|to_light| {
                shade_level(dot(normal, to_light) * if front { 1.0 } else { -1.0 })
            });
            // Sin tramado, el paso más cercano: 0 con la luz de frente (d > 0.5), 1 de lado
            // (d > 0) y 2 de espaldas.
            let steps = level.map_or(0, |l| l.round() as u8);
            let level = level.filter(|_| model.light.dither);
            let quad = QuadMap::new(&pts, &face.uvs, &view).map(|q| (q, &view));
            let ctx = FaceCtx {
                id: (id, face_i),
                level,
                quad,
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
    use crate::{P3, Primitive, darker, primitive_mesh};
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
            hidden: false,
            locked: false,
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
            frame: None,
        }
    }

    fn cube() -> Object3D {
        obj(primitive_mesh(&Primitive::Cube))
    }

    #[test]
    fn lo_oculto_no_se_pinta_y_un_grupo_mueve_a_sus_hijos() {
        let mut m = model(vec![cube()]);
        m.objects[0].hidden = true;
        assert!(
            render_model(&input(&m, 16, &[ROJO], 1))
                .iter()
                .all(|p| p.a == 0)
        );
        let mut m = model(vec![cube()]);
        m.groups.push(cs_core::Group3D {
            name: "g".into(),
            transform: Transform3D {
                position: Vec3i {
                    x: FIXED,
                    y: 0,
                    z: 0,
                },
                ..Default::default()
            },
            hidden: false,
            locked: false,
            open: true,
        });
        m.tree = vec![cs_core::Node::Group {
            group: 0,
            children: vec![cs_core::Node::Object(0)],
        }];
        let img = render_model(&input(&m, 16, &[ROJO], 1));
        // El cubo (4..12) corrido una unidad (4 px) a la derecha: 8..16.
        assert_eq!(img[8 * 16 + 6].a, 0);
        assert_eq!(img[8 * 16 + 14], ROJO);
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
            dither: false,
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
    fn el_nivel_redondeado_da_los_pasos_de_siempre() {
        for (d, steps) in [(1.0, 0), (0.6, 0), (0.4, 1), (0.1, 1), (-0.1, 2), (-1.0, 2)] {
            assert_eq!(shade_level(d).round() as u8, steps, "{d}");
        }
        // Un nivel entero no se trama; uno a la mitad, mitad y mitad.
        let count = |level: f32, step: u8| {
            (0..4)
                .flat_map(|y| (0..4).map(move |x| (x, y)))
                .filter(|&(x, y)| dithered_steps(level, x, y) == step)
                .count()
        };
        assert_eq!(count(1.0, 1), 16);
        assert_eq!(count(1.5, 1), 8);
        assert_eq!(count(1.5, 2), 8);
        assert_eq!(count(0.25, 1), 4);
    }

    #[test]
    fn con_tramado_una_cara_de_lado_mezcla_dos_pasos() {
        let mut m = model(vec![cube()]);
        m.camera = CameraPreset::Iso.camera(2 * FIXED, Vec3i::default());
        m.light = Light3D {
            enabled: true,
            yaw: 1000,
            pitch: 200,
            dither: false,
        };
        let colores = |m: &Model3D| {
            let img = render_model(&input(m, 32, &[BLANCO], 1));
            // La cara de arriba (de 1 paso sin tramado), alrededor de su centro.
            let mut c: Vec<Rgba> = (6..11)
                .flat_map(|y| (14..19).map(move |x| (x, y)))
                .map(|(x, y)| img[y * 32 + x])
                .collect();
            c.sort_by_key(|c| (c.r, c.g, c.b));
            c.dedup();
            c
        };
        assert_eq!(colores(&m), vec![CLARO], "plain");
        m.light.dither = true;
        let tramado = colores(&m);
        assert_eq!(tramado.len(), 2, "{tramado:?}");
        assert!(tramado.contains(&CLARO), "{tramado:?}");
    }

    #[test]
    fn tabla_de_shading_cambia_solo_su_paso() {
        let mut m = model(vec![cube()]);
        m.camera = CameraPreset::Iso.camera(2 * FIXED, Vec3i::default());
        m.light = Light3D {
            enabled: true,
            yaw: 1000,
            pitch: 200,
            dither: false,
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

    /// Un trapecio de frente (abajo de ±1, arriba de ±0.5) con la textura entera: su espejo
    /// izquierda/derecha es él mismo.
    fn trapecio() -> Mesh {
        let f = FIXED;
        let v = |x, y| Vec3i { x, y, z: 0 };
        Mesh {
            vertices: vec![v(-f, -f), v(f, -f), v(f / 2, f), v(-f / 2, f)],
            faces: vec![Face {
                verts: vec![0, 1, 2, 3],
                uvs: vec![(0, 8), (8, 8), (8, 0), (0, 0)],
            }],
        }
    }

    #[test]
    fn un_trapecio_con_textura_simetrica_se_ve_simetrico() {
        // Una X de 8×8 (simétrica izquierda/derecha).
        let tex: Vec<Rgba> = (0..64)
            .map(|i| {
                let (x, y) = (i % 8, i / 8);
                if x == y || x == 7 - y { ROJO } else { AZUL }
            })
            .collect();
        let m = model(vec![obj(trapecio())]);
        let size = 64;
        let img = render_model(&input(&m, size, &tex, 8));
        let size = size as usize;
        let (mut covered, mut wrong) = (0, 0);
        for y in 0..size {
            for x in 0..size / 2 {
                let (l, r) = (img[y * size + x], img[y * size + size - 1 - x]);
                if l.a == 0 && r.a == 0 {
                    continue;
                }
                covered += 1;
                if l != r {
                    wrong += 1;
                }
            }
        }
        assert!(covered > 100, "{covered}");
        // Por triángulos, la diagonal desarma la X (decenas de pixeles distintos).
        assert!(wrong * 50 <= covered, "{wrong} of {covered} not mirrored");
    }

    #[test]
    fn la_bilineal_inversa_da_las_esquinas_y_el_centro() {
        let q = [(0.0, 0.0), (4.0, 0.0), (3.0, 2.0), (1.0, 2.0)];
        for (i, st) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
            .iter()
            .enumerate()
        {
            let (s, t) = inverse_bilinear(q[i], &q).unwrap();
            assert!(
                (s - st.0).abs() < 1e-4 && (t - st.1).abs() < 1e-4,
                "{i}: {s} {t}"
            );
        }
        let (s, t) = inverse_bilinear((2.0, 1.0), &q).unwrap();
        assert!((s - 0.5).abs() < 1e-4 && (t - 0.5).abs() < 1e-4, "{s} {t}");
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
    fn lo_bloqueado_se_pinta_pero_no_se_elige() {
        let mut m = model(vec![cube()]);
        m.objects[0].locked = true;
        let inp = input(&m, 16, &[ROJO], 1);
        assert_eq!(render_model(&inp)[8 * 16 + 8], ROJO);
        assert_eq!(pick_object(&inp, 8, 8), None);
        assert_eq!(pick_face(&inp, 8, 8), None);
        m.objects[0].locked = false;
        let inp = input(&m, 16, &[ROJO], 1);
        assert_eq!(pick_object(&inp, 8, 8), Some(0));
        assert!(pick_face(&inp, 8, 8).is_some());
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
    /// La envolvente convexa (en sentido antihorario con y hacia arriba) de `pts`.
    fn hull(mut pts: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
        pts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let turn = |o: (f32, f32), a: (f32, f32), b: (f32, f32)| {
            (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
        };
        let mut out: Vec<(f32, f32)> = Vec::new();
        for pass in 0..2 {
            let start = out.len();
            let iter: Vec<(f32, f32)> = if pass == 0 {
                pts.clone()
            } else {
                pts.iter().rev().copied().collect()
            };
            for p in iter {
                while out.len() >= start + 2
                    && turn(out[out.len() - 2], out[out.len() - 1], p) <= 0.0
                {
                    out.pop();
                }
                out.push(p);
            }
            out.pop();
        }
        out
    }

    #[test]
    fn el_proyector_cae_donde_pinta_el_rasterizador() {
        // Un cubo visto en diagonal, en perspectiva y en ortográfica, en un render no cuadrado:
        // un pixel se pinta si y solo si su centro cae dentro de la envolvente de las esquinas
        // proyectadas (salvo justo sobre un borde).
        let corners: Vec<P3> = (0..8)
            .map(|i| {
                let s = |b: i32| if i & b != 0 { 16.0 } else { -16.0 };
                [s(1), s(2), s(4)]
            })
            .collect();
        for projection in [
            Projection::Perspective { fov: 600 },
            Projection::Orthographic,
        ] {
            let mut m = model(vec![cube()]);
            m.camera = Camera3D {
                projection,
                yaw: 300,
                pitch: 250,
                distance: 5 * FIXED,
                target: Vec3i { x: 3, y: -2, z: 1 },
            };
            let (w, h) = (40u32, 30u32);
            let inp = RenderInput {
                size: (w, h),
                ..input(&m, 16, &[ROJO], 1)
            };
            let img = render_model(&inp);
            let pts: Vec<(f32, f32)> = corners
                .iter()
                .map(|&p| project_point(&m.camera, (w, h), p).expect("in front"))
                .collect();
            // En la imagen y hacia abajo: se da vuelta para la envolvente.
            let poly = hull(pts.iter().map(|&(x, y)| (x, -y)).collect());
            let mut painted = 0;
            for y in 0..h {
                for x in 0..w {
                    let c = (x as f32 + 0.5, -(y as f32 + 0.5));
                    let sides: Vec<f32> = (0..poly.len())
                        .map(|i| {
                            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                            ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)) / len
                        })
                        .collect();
                    if sides.iter().any(|d| d.abs() < 0.01) {
                        continue;
                    }
                    let inside = sides.iter().all(|&d| d > 0.0);
                    let on = img[(y * w + x) as usize].a != 0;
                    assert_eq!(on, inside, "{projection:?} ({x},{y})");
                    painted += on as i32;
                }
            }
            assert!(painted > 40, "{projection:?}: {painted}");
        }
    }

    #[test]
    fn el_proyector_de_frente_y_sus_rayos() {
        // Front a 2 unidades en 16×16 (ortográfica): 4 pixeles por unidad desde el centro.
        let cam = CameraPreset::Front.camera(2 * FIXED, Vec3i::default());
        let pr = Projector::new(&cam, (16, 16));
        assert_eq!(pr.project([16.0, 16.0, 0.0]), Some((12.0, 4.0)));
        assert_eq!(pr.project([-16.0, 0.0, 5.0]), Some((4.0, 8.0)));
        let [right, up, fwd] = pr.axes();
        assert_eq!(
            (right, up, fwd),
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0])
        );
        // En perspectiva, lo de detrás de la cámara no se proyecta; lo de enfrente sí, y el rayo
        // de su punto en pantalla pasa por él.
        let persp = Camera3D {
            projection: Projection::Perspective { fov: 600 },
            yaw: 300,
            pitch: 250,
            distance: 5 * FIXED,
            target: Vec3i { x: 3, y: -2, z: 1 },
        };
        for cam in [persp, cam] {
            let pr = Projector::new(&cam, (40, 30));
            let p = [10.0, -20.0, 7.0];
            let s = pr.project(p).expect("in front");
            let (o, d) = pr.ray(s);
            // La distancia de `p` a la recta del rayo, en 1/16.
            let t: f32 = (0..3).map(|i| (p[i] - o[i]) * d[i]).sum::<f32>()
                / (0..3).map(|i| d[i] * d[i]).sum::<f32>();
            let off: f32 = (0..3)
                .map(|i| (o[i] + d[i] * t - p[i]).powi(2))
                .sum::<f32>()
                .sqrt();
            assert!(off < 0.05, "{cam:?}: {off}");
            assert!(t > 0.0);
        }
        let pr = Projector::new(&persp, (40, 30));
        let behind = pr.axes()[2].map(|f| -f * 6.0 * 16.0);
        assert_eq!(pr.project(behind), None);
        assert!(pr.depth([0.0; 3]) > 0.0);
    }
    #[test]
    fn con_marco_el_render_chico_es_el_centro_del_grande() {
        // De cerca, el cubo se sale del cuadro de 32×32: en el de 48×40 con el mismo marco se ve
        // lo de afuera, y su centro es igual pixel por pixel.
        for projection in [
            Projection::Perspective { fov: 600 },
            Projection::Orthographic,
        ] {
            let mut m = model(vec![cube()]);
            m.camera = Camera3D {
                projection,
                yaw: 300,
                pitch: 250,
                distance: FIXED * 3 / 2,
                target: Vec3i { x: 3, y: -2, z: 1 },
            };
            let tex = [ROJO, AZUL, BLANCO, NEGRO];
            let small = input(&m, 32, &tex, 2);
            let big = RenderInput {
                size: (48, 40),
                frame: Some((32, 32)),
                ..small
            };
            let (a, b) = (render_model(&small), render_model(&big));
            let faces = [(0, 0), (0, 2), (0, 4)];
            let (oa, ob) = (face_outline(&small, &faces), face_outline(&big, &faces));
            for y in 0..32u32 {
                for x in 0..32u32 {
                    let (i, j) = ((y * 32 + x) as usize, ((y + 4) * 48 + x + 8) as usize);
                    assert_eq!(a[i], b[j], "{projection:?} ({x}, {y})");
                    // El borde de la imagen chica cuenta como borde de la cara; adentro, igual.
                    if (1..31).contains(&x) && (1..31).contains(&y) {
                        assert_eq!(oa[i], ob[j], "{projection:?} contorno ({x}, {y})");
                    }
                    assert_eq!(
                        pick_face(&small, x, y),
                        pick_face(&big, x + 8, y + 4),
                        "{projection:?} pick ({x}, {y})"
                    );
                }
            }
            // Afuera del marco también hay modelo (antes quedaba cortado).
            let outside = (0..40u32)
                .flat_map(|y| (0..48u32).map(move |x| (x, y)))
                .filter(|&(x, y)| !(8..40).contains(&x) || !(4..36).contains(&y))
                .any(|(x, y)| b[(y * 48 + x) as usize].a > 0);
            assert!(outside, "{projection:?}");
        }
    }
}
