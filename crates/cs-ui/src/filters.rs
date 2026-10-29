//! Los filtros de Edit (Replace Color, Adjustments y FX), como en Aseprite: trabajan sobre lo
//! seleccionado de la capa actual (sin selección, la capa entera) en el frame actual, y cada uno
//! es un paso de undo. Los de color cambian cada píxel solo; Outline, Despeckle y Convolution
//! Matrix miran a los vecinos.

use cs_core::{Mask, PixelValue, Rgba};

use crate::editor::Editor;

/// Un núcleo de Convolution Matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kernel {
    Blur,
    Sharpen,
    Edges,
    Emboss,
}

impl Kernel {
    pub const ALL: [Kernel; 4] = [Kernel::Blur, Kernel::Sharpen, Kernel::Edges, Kernel::Emboss];

    pub fn label(self) -> &'static str {
        match self {
            Kernel::Blur => "Blur 3×3",
            Kernel::Sharpen => "Sharpen 3×3",
            Kernel::Edges => "Find Edges 3×3",
            Kernel::Emboss => "Emboss 3×3",
        }
    }

    /// Los pesos (de arriba a la izquierda, por filas) y el divisor.
    fn weights(self) -> ([i32; 9], i32) {
        match self {
            Kernel::Blur => ([1, 1, 1, 1, 1, 1, 1, 1, 1], 9),
            Kernel::Sharpen => ([0, -1, 0, -1, 5, -1, 0, -1, 0], 1),
            Kernel::Edges => ([-1, -1, -1, -1, 8, -1, -1, -1, -1], 1),
            Kernel::Emboss => ([-2, -1, 0, -1, 1, 1, 0, 1, 2], 1),
        }
    }
}

/// La curva de Color Curve: de 2 a [`Curve::MAX`] puntos (entrada, salida), ordenados por la
/// entrada; los de las puntas siempre en 0 y 255 de entrada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Curve {
    points: [(u8, u8); Curve::MAX],
    len: u8,
}

impl Curve {
    pub const MAX: usize = 8;

    /// La diagonal: no cambia nada.
    pub fn identity() -> Self {
        let mut points = [(0, 0); Curve::MAX];
        points[1] = (255, 255);
        Self { points, len: 2 }
    }

    pub fn points(&self) -> &[(u8, u8)] {
        &self.points[..self.len as usize]
    }

    /// Agrega un punto donde corresponde por su entrada; devuelve su índice (`None` si ya hay
    /// [`Curve::MAX`] o uno con esa entrada).
    pub fn insert(&mut self, x: u8, y: u8) -> Option<usize> {
        let len = self.len as usize;
        if len >= Curve::MAX || self.points().iter().any(|p| p.0 == x) {
            return None;
        }
        let at = self.points().iter().position(|p| p.0 > x)?;
        self.points.copy_within(at..len, at + 1);
        self.points[at] = (x, y);
        self.len += 1;
        Some(at)
    }

    /// Mueve el punto `i`: la salida libre; la entrada entre la de sus vecinos (las puntas no se
    /// corren de lado).
    pub fn move_point(&mut self, i: usize, x: u8, y: u8) {
        let last = self.len as usize - 1;
        let x = if i == 0 {
            0
        } else if i == last {
            255
        } else {
            x.clamp(self.points[i - 1].0 + 1, self.points[i + 1].0 - 1)
        };
        self.points[i] = (x, y);
    }

    /// Quita el punto `i` (no las puntas).
    pub fn remove(&mut self, i: usize) {
        let len = self.len as usize;
        if i == 0 || i + 1 >= len {
            return;
        }
        self.points.copy_within(i + 1..len, i);
        self.len -= 1;
    }

    /// La salida de cada entrada: una curva suave que pasa por los puntos sin pasarse de ellos
    /// (cúbica monótona, Fritsch–Carlson).
    pub fn lut(&self) -> [u8; 256] {
        let p: Vec<(f32, f32)> = self
            .points()
            .iter()
            .map(|&(x, y)| (x as f32, y as f32))
            .collect();
        let n = p.len();
        let slopes: Vec<f32> = p
            .windows(2)
            .map(|w| (w[1].1 - w[0].1) / (w[1].0 - w[0].0))
            .collect();
        let mut tangents = vec![0.0; n];
        tangents[0] = slopes[0];
        tangents[n - 1] = slopes[n - 2];
        for i in 1..n - 1 {
            tangents[i] = if slopes[i - 1] * slopes[i] <= 0.0 {
                0.0
            } else {
                (slopes[i - 1] + slopes[i]) / 2.0
            };
        }
        for i in 0..n - 1 {
            if slopes[i] == 0.0 {
                tangents[i] = 0.0;
                tangents[i + 1] = 0.0;
                continue;
            }
            let (a, b) = (tangents[i] / slopes[i], tangents[i + 1] / slopes[i]);
            let h = a.hypot(b);
            if h > 3.0 {
                tangents[i] = 3.0 * a / h * slopes[i];
                tangents[i + 1] = 3.0 * b / h * slopes[i];
            }
        }
        let mut lut = [0u8; 256];
        for (x, out) in lut.iter_mut().enumerate() {
            let x = x as f32;
            let i = p.windows(2).position(|w| x <= w[1].0).unwrap_or(n - 2);
            let ((x0, y0), (x1, y1)) = (p[i], p[i + 1]);
            let h = x1 - x0;
            let t = (x - x0) / h;
            let (t2, t3) = (t * t, t * t * t);
            let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
                + (t3 - 2.0 * t2 + t) * h * tangents[i]
                + (-2.0 * t3 + 3.0 * t2) * y1
                + (t3 - t2) * h * tangents[i + 1];
            *out = y.round().clamp(0.0, 255.0) as u8;
        }
        lut
    }
}

/// Un filtro con sus valores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    /// Los píxeles a menos de `tolerance` (en cada canal) de `from` pasan a ser `to`.
    ReplaceColor {
        from: Rgba,
        to: Rgba,
        tolerance: u8,
    },
    /// De -100 a 100.
    BrightnessContrast {
        brightness: i32,
        contrast: i32,
    },
    /// El tono de -180 a 180; la saturación y la luz de -100 a 100.
    HueSaturation {
        hue: i32,
        saturation: i32,
        lightness: i32,
    },
    /// Un borde de `color` afuera de lo pintado; `square`: también en diagonal.
    Outline {
        color: Rgba,
        square: bool,
    },
    /// La mediana de cada canal en un cuadrado de `size` (impar) alrededor.
    Despeckle {
        size: u32,
    },
    Convolution(Kernel),
    /// Los canales elegidos pasan por la curva.
    ColorCurve {
        curve: Curve,
        channels: Channels,
    },
}

/// Los canales que cambia Color Curve (como los botones R, G, B y A de Aseprite).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channels {
    pub r: bool,
    pub g: bool,
    pub b: bool,
    pub a: bool,
}

impl Default for Channels {
    /// Los de color, como en Aseprite.
    fn default() -> Self {
        Self {
            r: true,
            g: true,
            b: true,
            a: false,
        }
    }
}

impl Filter {
    /// El nombre del filtro (el de su ventana y su paso en el historial).
    pub fn title(&self) -> &'static str {
        match self {
            Filter::ReplaceColor { .. } => "Replace Color",
            Filter::BrightnessContrast { .. } => "Brightness/Contrast",
            Filter::HueSaturation { .. } => "Hue/Saturation",
            Filter::Outline { .. } => "Outline",
            Filter::Despeckle { .. } => "Despeckle",
            Filter::Convolution(_) => "Convolution Matrix",
            Filter::ColorCurve { .. } => "Color Curve",
        }
    }

    /// El color nuevo de un píxel (los que miran a los vecinos usan `at`, el color en el lienzo).
    fn apply(&self, x: i32, y: i32, at: &dyn Fn(i32, i32) -> Rgba) -> Rgba {
        let c = at(x, y);
        match *self {
            Filter::ReplaceColor {
                from,
                to,
                tolerance,
            } => {
                let near = |a: u8, b: u8| a.abs_diff(b) <= tolerance;
                if near(c.r, from.r) && near(c.g, from.g) && near(c.b, from.b) && near(c.a, from.a)
                {
                    to
                } else {
                    c
                }
            }
            Filter::BrightnessContrast {
                brightness,
                contrast,
            } => {
                if c.a == 0 {
                    return c;
                }
                let factor = (100 + contrast).max(0) as f32 / 100.0;
                let shift = brightness as f32 * 2.55;
                let f = |v: u8| {
                    ((v as f32 - 128.0) * factor + 128.0 + shift)
                        .round()
                        .clamp(0.0, 255.0) as u8
                };
                Rgba::new(f(c.r), f(c.g), f(c.b), c.a)
            }
            Filter::HueSaturation {
                hue,
                saturation,
                lightness,
            } => {
                if c.a == 0 {
                    return c;
                }
                let (h, s, l) = to_hsl(c);
                let h = (h + hue as f32).rem_euclid(360.0);
                let s = (s * (1.0 + saturation as f32 / 100.0)).clamp(0.0, 1.0);
                let amount = lightness as f32 / 100.0;
                let l = if amount > 0.0 {
                    l + (1.0 - l) * amount
                } else {
                    l + l * amount
                };
                let (r, g, b) = from_hsl(h, s, l.clamp(0.0, 1.0));
                Rgba::new(r, g, b, c.a)
            }
            Filter::Outline { color, square } => {
                if c.a > 0 {
                    return c;
                }
                let sides = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                let corners = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
                let touches = |n: &[(i32, i32)]| n.iter().any(|(dx, dy)| at(x + dx, y + dy).a > 0);
                if touches(&sides) || (square && touches(&corners)) {
                    color
                } else {
                    c
                }
            }
            Filter::Despeckle { size } => {
                let r = (size.max(1) / 2) as i32;
                let mut around = Vec::new();
                for dy in -r..=r {
                    for dx in -r..=r {
                        around.push(at(x + dx, y + dy));
                    }
                }
                let median = |f: fn(&Rgba) -> u8| {
                    let mut v: Vec<u8> = around.iter().map(f).collect();
                    v.sort_unstable();
                    v[v.len() / 2]
                };
                Rgba::new(
                    median(|c| c.r),
                    median(|c| c.g),
                    median(|c| c.b),
                    median(|c| c.a),
                )
            }
            Filter::ColorCurve { curve, channels } => {
                let lut = curve.lut();
                let f = |v: u8, on: bool| if on { lut[v as usize] } else { v };
                Rgba::new(
                    f(c.r, channels.r),
                    f(c.g, channels.g),
                    f(c.b, channels.b),
                    f(c.a, channels.a),
                )
            }
            Filter::Convolution(kernel) => {
                if c.a == 0 {
                    return c;
                }
                let (w, div) = kernel.weights();
                let mut sum = [0i32; 3];
                for (i, weight) in w.iter().enumerate() {
                    let n = at(x + i as i32 % 3 - 1, y + i as i32 / 3 - 1);
                    // Lo transparente cuenta como el píxel del centro (no oscurece los bordes).
                    let n = if n.a == 0 { c } else { n };
                    for (s, v) in sum.iter_mut().zip([n.r, n.g, n.b]) {
                        *s += weight * v as i32;
                    }
                }
                let f = |s: i32| (s / div).clamp(0, 255) as u8;
                Rgba::new(f(sum[0]), f(sum[1]), f(sum[2]), c.a)
            }
        }
    }
}

fn to_hsl(c: Rgba) -> (f32, f32, f32) {
    let (r, g, b) = (c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (f(r), f(g), f(b))
}

impl Editor {
    /// Aplica `filter` a lo seleccionado de la capa actual (sin selección, a la capa entera).
    /// Devuelve si cambió algo.
    pub fn apply_filter(&mut self, filter: &Filter) -> bool {
        self.commit_floating();
        if !self.clip_layer_ok(true) {
            return false;
        }
        let sprite = self.sprite();
        let mask = self
            .selection
            .clone()
            .unwrap_or_else(|| Mask::all(sprite.width, sprite.height));
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        let at = |x: i32, y: i32| {
            if x < 0 || y < 0 || x >= w || y >= h {
                Rgba::TRANSPARENT
            } else {
                self.resolve(sprite.pixel(self.layer, self.frame, x, y))
            }
        };
        let changed: Vec<(i32, i32, Rgba)> = mask
            .points()
            .filter_map(|(x, y)| {
                let new = filter.apply(x, y, &at);
                (new != at(x, y)).then_some((x, y, new))
            })
            .collect();
        let pixels: Vec<(i32, i32, PixelValue)> = changed
            .into_iter()
            .map(|(x, y, c)| (x, y, self.value_of(c)))
            .collect();
        !pixels.is_empty()
            && self.execute_named(
                cs_commands::Command::DrawPixels {
                    layer: self.layer,
                    frame: self.frame,
                    pixels,
                },
                filter.title(),
            )
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, Sprite};

    use super::*;

    const RED: Rgba = Rgba::rgb(255, 0, 0);

    /// 5×5 con un punto rojo en el centro.
    fn editor() -> Editor {
        let mut e = Editor::with_sprite(Sprite::new(5, 5, ColorMode::Rgba));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(2, 2, PixelValue::Rgba(RED))],
        });
        e
    }

    fn at(e: &Editor, x: i32, y: i32) -> Rgba {
        e.resolve(e.sprite().pixel(LayerId(1), 0, x, y))
    }

    #[test]
    fn replace_color_changes_the_close_ones() {
        let mut e = editor();
        let blue = Rgba::rgb(0, 0, 255);
        let near = Rgba::rgb(250, 0, 0);
        assert!(
            !e.apply_filter(&Filter::ReplaceColor {
                from: near,
                to: blue,
                tolerance: 4
            }),
            "too far"
        );
        assert!(e.apply_filter(&Filter::ReplaceColor {
            from: near,
            to: blue,
            tolerance: 5
        }));
        assert_eq!(at(&e, 2, 2), blue);
    }

    #[test]
    fn brightness_contrast_and_hue_saturation_keep_the_transparency() {
        let mut e = editor();
        e.apply_filter(&Filter::BrightnessContrast {
            brightness: -100,
            contrast: 0,
        });
        assert_eq!(at(&e, 2, 2), Rgba::rgb(0, 0, 0));
        assert_eq!(at(&e, 0, 0).a, 0);
        let mut e = editor();
        e.apply_filter(&Filter::HueSaturation {
            hue: 120,
            saturation: 0,
            lightness: 0,
        });
        assert_eq!(at(&e, 2, 2), Rgba::rgb(0, 255, 0), "red turns green");
        e.apply_filter(&Filter::HueSaturation {
            hue: 0,
            saturation: -100,
            lightness: 0,
        });
        let c = at(&e, 2, 2);
        assert!(c.r == c.g && c.g == c.b, "gray: {c:?}");
    }

    #[test]
    fn outline_draws_around_with_or_without_corners() {
        let mut e = editor();
        let black = Rgba::rgb(0, 0, 0);
        e.apply_filter(&Filter::Outline {
            color: black,
            square: false,
        });
        assert_eq!(at(&e, 1, 2), black);
        assert_eq!(at(&e, 2, 3), black);
        assert_eq!(at(&e, 1, 1).a, 0, "no corners");
        assert_eq!(at(&e, 2, 2), RED);
        let mut e = editor();
        e.apply_filter(&Filter::Outline {
            color: black,
            square: true,
        });
        assert_eq!(at(&e, 1, 1), black);
    }

    #[test]
    fn despeckle_removes_a_lone_pixel_and_blur_spreads_it() {
        let mut e = editor();
        assert!(e.apply_filter(&Filter::Despeckle { size: 3 }));
        assert_eq!(at(&e, 2, 2).a, 0, "a lone speck goes away");
        let mut e = editor();
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: (0..5)
                .flat_map(|y| (0..5).map(move |x| (x, y, PixelValue::Rgba(Rgba::rgb(0, 0, 0)))))
                .chain([(2, 2, PixelValue::Rgba(Rgba::rgb(90, 90, 90)))])
                .collect(),
        });
        e.apply_filter(&Filter::Convolution(Kernel::Blur));
        assert_eq!(at(&e, 2, 2), Rgba::rgb(10, 10, 10));
        assert_eq!(at(&e, 1, 1), Rgba::rgb(10, 10, 10));
    }

    #[test]
    fn the_curve_passes_through_its_points_and_edits_them() {
        let identity = Curve::identity();
        assert!(
            identity
                .lut()
                .iter()
                .enumerate()
                .all(|(i, &v)| v as usize == i)
        );
        let mut c = Curve::identity();
        let i = c.insert(128, 200).expect("room");
        assert_eq!(i, 1);
        let lut = c.lut();
        assert_eq!(lut[128], 200);
        assert!(
            lut.windows(2).all(|w| w[0] <= w[1]),
            "no overshoot: monotone"
        );
        c.move_point(1, 255, 10);
        assert_eq!(c.points()[1], (254, 10), "between its neighbors");
        c.move_point(0, 90, 40);
        assert_eq!(c.points()[0], (0, 40), "the ends stay at the sides");
        c.remove(0);
        assert_eq!(c.points().len(), 3, "the ends stay");
        c.remove(1);
        assert_eq!(c.points().len(), 2);
        let mut e = editor();
        let mut dark = Curve::identity();
        dark.move_point(1, 255, 0);
        e.apply_filter(&Filter::ColorCurve {
            curve: dark,
            channels: Channels::default(),
        });
        assert_eq!(at(&e, 2, 2), Rgba::rgb(0, 0, 0));
    }

    #[test]
    fn the_curve_changes_only_the_chosen_channels() {
        let mut e = editor();
        let before = at(&e, 2, 2);
        // Al revés: 0 → 255 y 255 → 0.
        let mut reversed = Curve::identity();
        reversed.move_point(0, 0, 255);
        reversed.move_point(1, 255, 0);
        let channels = Channels {
            r: false,
            g: true,
            b: false,
            a: false,
        };
        e.apply_filter(&Filter::ColorCurve {
            curve: reversed,
            channels,
        });
        let after = at(&e, 2, 2);
        assert_eq!((after.r, after.b, after.a), (before.r, before.b, before.a));
        assert_eq!(after.g, 255 - before.g);
    }

    #[test]
    fn filters_stay_inside_the_selection() {
        let mut e = editor();
        e.set_selection(Some(Mask::from_rect(5, 5, (0, 0), (1, 4))));
        let black = Rgba::rgb(0, 0, 0);
        e.apply_filter(&Filter::Outline {
            color: black,
            square: false,
        });
        assert_eq!(at(&e, 1, 2), black, "inside");
        assert_eq!(at(&e, 3, 2).a, 0, "outside the selection");
    }
}
