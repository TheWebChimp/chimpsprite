//! Skin de la UI en pixel art: piezas y colores leídos de `assets/ui/skin.aseprite`.
//!
//! - Cada pieza es un slice con nombre; su centro, si lo tiene, es el 9-slice.
//! - Los slices `color.*` dan un color: el de su píxel central.
//! - Una pieza sin dibujar (toda transparente) cuenta como que no existe.
//! - Si falta una variante (`.hover`, `.down`, `.active`), se usa la base.

pub mod nine;
#[cfg(not(target_arch = "wasm32"))]
pub mod watch;

use std::collections::{BTreeMap, HashMap};

use cs_core::Rgba;
use eframe::egui::{self, Color32};

pub use nine::{IRect, Rect9, nine_slice};

use crate::theme;

/// El skin que viaja dentro del binario.
pub const EMBEDDED: &[u8] = include_bytes!("../../../../assets/ui/skin.aseprite");

/// Colores del skin (`color.<nombre>`).
pub const COLOR_NAMES: [&str; 10] = [
    "desktop",
    "text",
    "text_dim",
    "accent",
    "panel",
    "outline",
    "selection",
    "scroll",
    "background",
    "canvas",
];

#[derive(Debug, thiserror::Error)]
pub enum SkinError {
    #[error("could not read the skin: {0}")]
    Read(#[from] cs_io::IoError),
}

/// Variante de una pieza según el estado del widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetState {
    Normal,
    Hover,
    Down,
    Active,
}

impl WidgetState {
    fn suffix(self) -> &'static str {
        match self {
            WidgetState::Normal => "",
            WidgetState::Hover => ".hover",
            WidgetState::Down => ".down",
            WidgetState::Active => ".active",
        }
    }
}

/// Una pieza recortada del skin, todavía sin subir a la GPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PieceImage {
    pub size: [u32; 2],
    /// RGBA sin premultiplicar, fila por fila.
    pub rgba: Vec<u8>,
    pub center: Option<Rect9>,
}

/// Lo que se lee de `skin.aseprite`, sin egui (se prueba sin contexto).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkinData {
    pub pieces: BTreeMap<String, PieceImage>,
    /// Por nombre, sin el prefijo `color.`.
    pub colors: BTreeMap<String, Rgba>,
}

/// Lee el skin: compone el frame 0 con las capas visibles y recorta cada slice.
pub fn decode(bytes: &[u8]) -> Result<SkinData, SkinError> {
    let (sprite, _) = cs_io::aseprite::read(bytes)?;
    let pixels = cs_render::render_frame(&sprite, 0);
    let (width, height) = (sprite.width as i32, sprite.height as i32);
    let at = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= width || y >= height {
            Rgba::TRANSPARENT
        } else {
            pixels[(y * width + x) as usize]
        }
    };
    let mut data = SkinData::default();
    for slice in &sprite.slices {
        let Some(key) = slice.key_at(0) else {
            continue;
        };
        let (x, y, w, h) = key.bounds;
        if w == 0 || h == 0 {
            continue;
        }
        if let Some(name) = slice.name.strip_prefix("color.") {
            let c = at(x + w as i32 / 2, y + h as i32 / 2);
            if c.a > 0 {
                data.colors.insert(name.to_string(), c);
            }
            continue;
        }
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for py in y..y + h as i32 {
            for px in x..x + w as i32 {
                let c = at(px, py);
                rgba.extend_from_slice(&[c.r, c.g, c.b, c.a]);
            }
        }
        if rgba.iter().skip(3).step_by(4).all(|&a| a == 0) {
            continue;
        }
        let center = key
            .center
            .and_then(|(cx, cy, cw, ch)| Rect9::new(cx, cy, cw, ch, [w, h]));
        data.pieces.insert(
            slice.name.clone(),
            PieceImage {
                size: [w, h],
                rgba,
                center,
            },
        );
    }
    Ok(data)
}

/// Colores del skin. Los que el skin no trae salen del tema oscuro (`theme.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinColors {
    pub desktop: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub panel: Color32,
    pub outline: Color32,
    pub selection: Color32,
    pub scroll: Color32,
    /// Fondo de la app: menú, pestañas, barra de opciones y paneles.
    pub background: Color32,
    /// Alrededor del lienzo (el área central, detrás de los artboards).
    pub canvas: Color32,
}

impl SkinColors {
    pub fn from_theme() -> Self {
        Self {
            desktop: theme::CANVAS_BG,
            text: egui::Visuals::dark().strong_text_color(),
            text_dim: theme::LABEL,
            accent: theme::ACCENT,
            panel: theme::PANEL,
            outline: theme::CANVAS_BORDER,
            selection: theme::ACCENT,
            scroll: theme::WIDGET_HOVER,
            background: theme::PANEL,
            canvas: theme::CANVAS_BG,
        }
    }

    fn with(colors: &BTreeMap<String, Rgba>) -> Self {
        let mut c = Self::from_theme();
        let slots = [
            ("desktop", &mut c.desktop),
            ("text", &mut c.text),
            ("text_dim", &mut c.text_dim),
            ("accent", &mut c.accent),
            ("panel", &mut c.panel),
            ("outline", &mut c.outline),
            ("selection", &mut c.selection),
            ("scroll", &mut c.scroll),
            ("background", &mut c.background),
            ("canvas", &mut c.canvas),
        ];
        for (name, slot) in slots {
            if let Some(v) = colors.get(name) {
                *slot = Color32::from_rgba_unmultiplied(v.r, v.g, v.b, v.a);
            }
        }
        c
    }
}

/// Una pieza lista para dibujar, sin suavizado.
pub struct Piece {
    pub texture: egui::TextureHandle,
    pub size: [u32; 2],
    pub center: Option<Rect9>,
}

pub struct Skin {
    pieces: HashMap<String, Piece>,
    colors: SkinColors,
}

impl Skin {
    pub fn load(ctx: &egui::Context, bytes: &[u8]) -> Result<Skin, SkinError> {
        Ok(Self::from_data(ctx, decode(bytes)?))
    }

    /// El skin embebido; si no se puede leer, todo plano con los colores del tema.
    pub fn embedded(ctx: &egui::Context) -> Skin {
        Self::load(ctx, EMBEDDED).unwrap_or_else(|e| {
            log::error!("embedded skin: {e}");
            Self::fallback()
        })
    }

    /// Sin piezas: quien dibuja usa el respaldo plano.
    pub fn fallback() -> Skin {
        Skin {
            pieces: HashMap::new(),
            colors: SkinColors::from_theme(),
        }
    }

    pub fn from_data(ctx: &egui::Context, data: SkinData) -> Skin {
        let pieces = data
            .pieces
            .into_iter()
            .map(|(name, img)| {
                let size = [img.size[0] as usize, img.size[1] as usize];
                let image = egui::ColorImage::from_rgba_unmultiplied(size, &img.rgba);
                let texture =
                    ctx.load_texture(format!("skin:{name}"), image, egui::TextureOptions::NEAREST);
                let piece = Piece {
                    texture,
                    size: img.size,
                    center: img.center,
                };
                (name, piece)
            })
            .collect();
        Skin {
            pieces,
            colors: SkinColors::with(&data.colors),
        }
    }

    /// La variante del estado o, si no está, la pieza base.
    pub fn piece(&self, name: &str, state: WidgetState) -> Option<&Piece> {
        let suffix = state.suffix();
        if !suffix.is_empty()
            && let Some(piece) = self.pieces.get(&format!("{name}{suffix}"))
        {
            return Some(piece);
        }
        self.pieces.get(name)
    }

    pub fn colors(&self) -> &SkinColors {
        &self.colors
    }

    /// El mismo skin sin las piezas `names`, para probar los respaldos.
    #[cfg(test)]
    pub fn without(mut self, names: &[&str]) -> Skin {
        for name in names {
            self.pieces.remove(*name);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, PixelValue, Slice, SliceKey, Sprite};

    use super::*;

    /// Los 22 íconos de herramientas del mockup (`icon.<nombre>`).
    const TOOL_ICONS: [&str; 22] = [
        "marquee",
        "hand",
        "pencil",
        "eraser",
        "eyedropper",
        "bucket",
        "line",
        "rectangle",
        "text",
        "contour",
        "blur",
        "move",
        "artboard",
        "ellipse_marquee",
        "lasso",
        "polygon_lasso",
        "magic_wand",
        "spray",
        "gradient",
        "jumble",
        "ellipse",
        "slice",
    ];

    fn slice(name: &str, bounds: (i32, i32, u32, u32)) -> Slice {
        Slice {
            name: name.into(),
            color: Rgba::rgb(0, 0, 255),
            text: String::new(),
            keys: vec![SliceKey {
                frame: 0,
                bounds,
                center: None,
                pivot: None,
            }],
        }
    }

    /// 12×4 guardado como `.aseprite`: `solid` (0,0)–(3,3) pintada de rojo, `empty` (4,0) sin
    /// dibujar y `color.text` con verde en su píxel central (10, 2).
    fn tiny() -> Vec<u8> {
        let mut sprite = Sprite::new(12, 4, ColorMode::Rgba);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let mut pixels: Vec<_> = (0..4)
            .flat_map(|y| (0..4).map(move |x| (x, y, red)))
            .collect();
        pixels.push((10, 2, PixelValue::Rgba(Rgba::rgb(0, 255, 0))));
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels,
        }
        .apply(&mut sprite)
        .unwrap();
        sprite.slices = vec![
            slice("solid", (0, 0, 4, 4)),
            slice("empty", (4, 0, 4, 4)),
            slice("color.text", (8, 0, 4, 4)),
        ];
        cs_io::aseprite::write(&sprite)
    }

    #[test]
    fn transparent_pieces_do_not_exist_and_colors_come_from_the_center() {
        let data = decode(&tiny()).unwrap();
        assert_eq!(data.pieces["solid"].size, [4, 4]);
        assert_eq!(&data.pieces["solid"].rgba[..4], &[255, 0, 0, 255]);
        assert!(!data.pieces.contains_key("empty"), "nothing drawn: missing");
        assert_eq!(data.colors["text"], Rgba::rgb(0, 255, 0));
    }

    #[test]
    fn missing_variants_fall_back_to_the_base_and_missing_colors_to_the_theme() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, &tiny()).unwrap();
        let solid = skin
            .piece("solid", WidgetState::Hover)
            .expect("falls back to the base");
        assert_eq!(solid.size, [4, 4]);
        assert!(skin.piece("empty", WidgetState::Normal).is_none());
        assert_eq!(skin.colors().text, Color32::from_rgb(0, 255, 0));
        assert_eq!(
            skin.colors().accent,
            theme::ACCENT,
            "not in the skin: theme color"
        );
        assert_eq!(skin.colors().background, theme::PANEL);
        assert_eq!(skin.colors().canvas, theme::CANVAS_BG);
    }

    #[test]
    fn the_embedded_skin_has_the_mockup_colors_window_and_buttons() {
        let data = decode(EMBEDDED).unwrap();
        for name in COLOR_NAMES {
            assert!(data.colors.contains_key(name), "color.{name}");
        }
        assert_eq!(
            data.colors["accent"],
            Rgba::rgb(226, 114, 133),
            "the pink title bar"
        );
        let window = &data.pieces["window.active"];
        let center = Rect9 {
            x: 3,
            y: 13,
            w: 58,
            h: 32,
        };
        assert_eq!(window.center, Some(center));
        let button = &data.pieces["button"];
        let center = Rect9 {
            x: 3,
            y: 3,
            w: 43,
            h: 10,
        };
        assert_eq!((button.size, button.center), ([49, 17], Some(center)));
        assert!(data.pieces.contains_key("button.hover"));
        assert!(data.pieces.contains_key("button.down"));
        assert!(!data.pieces.contains_key("panel"), "not drawn yet");
    }

    #[test]
    fn the_embedded_skin_has_the_tool_icons_and_slots_of_the_mockup() {
        let data = decode(EMBEDDED).unwrap();
        // `decode` descarta las piezas transparentes: si está, tiene algo dibujado.
        for name in TOOL_ICONS {
            let icon = data
                .pieces
                .get(&format!("icon.{name}"))
                .unwrap_or_else(|| panic!("icon.{name}"));
            assert_eq!((icon.size, icon.center), ([17, 17], None), "icon.{name}");
        }
        let center = Rect9 {
            x: 2,
            y: 2,
            w: 19,
            h: 19,
        };
        let alpha = |piece: &PieceImage, x: u32, y: u32| {
            piece.rgba[((y * piece.size[0] + x) * 4 + 3) as usize]
        };
        // Los tramos de la columna del mockup: el primero (arriba redondeado) y los del medio
        // miden 23×22 (borde de arriba, sin el de abajo, que es el del siguiente); el último,
        // 23×23 (con el borde de abajo, redondeado).
        for (name, height, round_top, round_bottom) in [
            ("tool.slot.first", 22, true, false),
            ("tool.slot", 22, false, false),
            ("tool.slot.last", 23, false, true),
        ] {
            for name in [name.to_owned(), format!("{name}.active")] {
                let slot = data.pieces.get(&name).unwrap_or_else(|| panic!("{name}"));
                assert_eq!(
                    (slot.size, slot.center),
                    ([23, height], Some(center)),
                    "{name}"
                );
                let bottom = height - 1;
                // Esquinas redondeadas: transparentes; las cuadradas, borde.
                for (x, y, round) in [
                    (0, 0, round_top),
                    (22, 0, round_top),
                    (0, bottom, round_bottom),
                    (22, bottom, round_bottom),
                ] {
                    let expected = if round { 0 } else { 255 };
                    assert_eq!(alpha(slot, x, y), expected, "{name} ({x},{y})");
                }
            }
        }
        // El activo es otro dibujo: el relleno del mockup es café y tramado.
        assert_ne!(
            data.pieces["tool.slot"].rgba,
            data.pieces["tool.slot.active"].rgba
        );
        assert!(
            !data.pieces.contains_key("tool.slot.hover"),
            "not drawn yet: falls back to tool.slot"
        );
        // La caja de la barra: 27 de ancho (el botón de 23 más 1 px de aire y 1 de línea a cada
        // lado) y un centro que se estira en vertical.
        let frame = &data.pieces["toolbar.box"];
        let center = Rect9 {
            x: 3,
            y: 3,
            w: 21,
            h: 1,
        };
        assert_eq!((frame.size, frame.center), ([27, 7], Some(center)));
        assert_eq!(alpha(frame, 0, 0), 0, "rounded corner");
        assert_eq!(alpha(frame, 0, 3), 255, "left line");
        assert_eq!(alpha(frame, 1, 3), 0, "the air inside");
        assert_eq!(alpha(frame, 13, 3), 0, "the buttons go inside");
    }

    #[test]
    fn the_embedded_skin_has_the_mockup_background_canvas_and_symmetry_buttons() {
        let data = decode(EMBEDDED).unwrap();
        assert_eq!(data.colors["background"], Rgba::rgb(221, 206, 191));
        assert_eq!(data.colors["canvas"], Rgba::rgb(189, 164, 153));
        let alpha = |piece: &PieceImage, x: u32, y: u32| {
            piece.rgba[((y * piece.size[0] + x) * 4 + 3) as usize]
        };
        for name in ["sym.slot", "sym.slot.active"] {
            let slot = &data.pieces[name];
            assert_eq!((slot.size, slot.center), ([13, 14], None), "{name}");
            // El borde del cuadro de 11×11 empieza en (1,1); su interior (el del ícono) está vacío.
            assert_eq!(alpha(slot, 2, 1), 255, "{name}: top border");
            assert_eq!(alpha(slot, 1, 2), 255, "{name}: left border");
            assert_eq!(alpha(slot, 6, 6), 0, "{name}: empty inside");
        }
        // Lo dibujado de cada ícono de 9×9 es su eje: una línea de 9 píxeles.
        for name in [
            "icon.sym_vertical",
            "icon.sym_horizontal",
            "icon.sym_diag_down",
            "icon.sym_diag_up",
        ] {
            let icon = &data.pieces[name];
            assert_eq!((icon.size, icon.center), ([9, 9], None), "{name}");
            for y in 0..9 {
                for x in 0..9 {
                    let on_axis = match name {
                        "icon.sym_vertical" => x == 4,
                        "icon.sym_horizontal" => y == 4,
                        "icon.sym_diag_down" => x == y,
                        _ => x + y == 8,
                    };
                    assert_eq!(alpha(icon, x, y) == 255, on_axis, "{name} ({x},{y})");
                }
            }
        }
    }

    #[test]
    fn variants_are_used_when_they_exist() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, EMBEDDED).unwrap();
        let down = skin.piece("button", WidgetState::Down).unwrap();
        let normal = skin.piece("button", WidgetState::Normal).unwrap();
        assert_ne!(down.texture.id(), normal.texture.id());
        assert!(skin.piece("window", WidgetState::Active).is_some());
    }
}
