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
}

impl Filter {
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
            && self.execute(cs_commands::Command::DrawPixels {
                layer: self.layer,
                frame: self.frame,
                pixels,
            })
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
