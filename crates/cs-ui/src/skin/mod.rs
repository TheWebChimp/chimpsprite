//! Skin de la UI en pixel art: piezas y colores leídos de `assets/ui/skin.aseprite`.
//!
//! - Cada pieza es un slice con nombre; su centro, si lo tiene, es el 9-slice.
//! - Los slices `color.*` dan un color: el de su píxel central.
//! - Una pieza sin dibujar (toda transparente) cuenta como que no existe.
//! - Si falta una variante (`.hover`, `.down`, `.active`), se usa la base.
//! - Una animación son las piezas `<nombre>.0`, `<nombre>.1`…; cada frame dura lo que dice el
//!   texto (user data) de su slice, `duration=<ms>`, como la duración del frame en Aseprite.

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
pub const COLOR_NAMES: [&str; 15] = [
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
    "highlight",
    "tab_text_dim",
    "home_text",
    "ruler_tick",
    "menu_highlight",
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
    /// Si es un frame de una animación: cuánto dura, en milisegundos (`duration=<ms>`).
    pub duration_ms: Option<u32>,
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
                duration_ms: parse_duration(&slice.text),
            },
        );
    }
    Ok(data)
}

/// La duración de un frame en el texto de su slice: una línea `duration=<ms>` (más de 0).
fn parse_duration(text: &str) -> Option<u32> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "duration")
        .and_then(|(_, ms)| ms.trim().parse().ok())
        .filter(|&ms| ms > 0)
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
    /// Alrededor del lienzo (el área central, detrás de los artboards). Es también la línea
    /// oscura de los separadores grabados.
    pub canvas: Color32,
    /// El brillo de los separadores grabados: la línea clara al lado de la oscura.
    pub highlight: Color32,
    /// El nombre de las pestañas inactivas (el de la activa va en `text`).
    pub tab_text_dim: Color32,
    /// El texto "Home" de la barra de pestañas y los títulos del menú.
    pub home_text: Color32,
    /// Las reglas con Pixel UI: las marcas, el signo menos y la línea contra el lienzo.
    pub ruler_tick: Color32,
    /// El brillo de los menús desplegables: el bisel del panel y la línea clara de sus
    /// separadores (la otra es `background`).
    pub menu_highlight: Color32,
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
            highlight: theme::WIDGET_HOVER,
            tab_text_dim: theme::LABEL,
            home_text: egui::Visuals::dark().strong_text_color(),
            ruler_tick: theme::RULER_TICK,
            menu_highlight: theme::WIDGET_HOVER,
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
            ("highlight", &mut c.highlight),
            ("tab_text_dim", &mut c.tab_text_dim),
            ("home_text", &mut c.home_text),
            ("ruler_tick", &mut c.ruler_tick),
            ("menu_highlight", &mut c.menu_highlight),
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
#[derive(Clone)]
pub struct Piece {
    pub texture: egui::TextureHandle,
    pub size: [u32; 2],
    pub center: Option<Rect9>,
    /// Si es un frame de una animación: cuánto dura, en milisegundos.
    pub duration_ms: Option<u32>,
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
                    duration_ms: img.duration_ms,
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

    /// Los frames de la animación `name`: `name.0`, `name.1`… hasta el primero que falte.
    pub fn animation(&self, name: &str) -> Vec<&Piece> {
        (0..)
            .map_while(|i| self.pieces.get(&format!("{name}.{i}")))
            .collect()
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
    fn the_frame_duration_comes_from_the_slice_text() {
        assert_eq!(parse_duration("duration=100"), Some(100));
        assert_eq!(parse_duration("algo\nduration = 250 "), Some(250));
        assert_eq!(parse_duration("color: se usa el píxel central"), None);
        assert_eq!(
            parse_duration("duration=0"),
            None,
            "a frame lasts something"
        );
        assert_eq!(parse_duration(""), None);
    }

    #[test]
    fn animations_are_the_numbered_pieces_in_order() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, EMBEDDED).unwrap();
        let frames = skin.animation("icon.home");
        assert_eq!(frames.len(), 3);
        for (i, frame) in frames.iter().enumerate() {
            let piece = skin.piece(&format!("icon.home.{i}"), WidgetState::Normal);
            assert_eq!(frame.texture.id(), piece.unwrap().texture.id());
        }
        assert!(skin.animation("icon.nothing").is_empty());
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
        assert_eq!(skin.colors().highlight, theme::WIDGET_HOVER);
    }

    #[test]
    fn the_embedded_skin_has_the_artboard_menu_icon_of_the_mockup() {
        let data = decode(EMBEDDED).unwrap();
        let menu = &data.pieces["artboard.menu"];
        assert_eq!(menu.size, [9, 9]);
        assert_eq!(menu.center, None);
        // Tres rayas de 6 px en las filas 2, 4 y 6, desde la columna 2; el resto transparente.
        for y in 0..9 {
            for x in 0..9 {
                let i = ((y * 9 + x) * 4) as usize;
                let ink = [2, 4, 6].contains(&y) && (2..8).contains(&x);
                let expected: &[u8] = if ink {
                    &[100, 54, 75, 255]
                } else {
                    &[0, 0, 0, 0]
                };
                assert_eq!(&menu.rgba[i..i + 4], expected, "({x},{y})");
            }
        }
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
        // El brillo de los separadores grabados del mockup (debajo o a la derecha de la línea).
        assert_eq!(data.colors["highlight"], Rgba::rgb(241, 235, 219));
        let alpha = |piece: &PieceImage, x: u32, y: u32| {
            piece.rgba[((y * piece.size[0] + x) * 4 + 3) as usize]
        };
        // Los botones agrandados del mockup (frame 4): 15×16.
        for name in ["sym.slot", "sym.slot.active"] {
            let slot = &data.pieces[name];
            assert_eq!((slot.size, slot.center), ([15, 16], None), "{name}");
            // El borde del cuadro de 13×13 empieza en (1,1); su interior (el del ícono, de 11×11
            // desde (2,2)) está vacío.
            assert_eq!(alpha(slot, 2, 1), 255, "{name}: top border");
            assert_eq!(alpha(slot, 1, 2), 255, "{name}: left border");
            for (x, y) in [(2, 2), (7, 7), (12, 12)] {
                assert_eq!(alpha(slot, x, y), 0, "{name}: empty inside at ({x},{y})");
            }
            assert_eq!(alpha(slot, 13, 13), 255, "{name}: bottom right border");
        }
        // Lo dibujado de cada ícono de 11×11 es su eje: una línea de 11 píxeles.
        for name in [
            "icon.sym_vertical",
            "icon.sym_horizontal",
            "icon.sym_diag_down",
            "icon.sym_diag_up",
        ] {
            let icon = &data.pieces[name];
            assert_eq!((icon.size, icon.center), ([11, 11], None), "{name}");
            for y in 0..11 {
                for x in 0..11 {
                    let on_axis = match name {
                        "icon.sym_vertical" => x == 5,
                        "icon.sym_horizontal" => y == 5,
                        "icon.sym_diag_down" => x == y,
                        _ => x + y == 10,
                    };
                    assert_eq!(alpha(icon, x, y) == 255, on_axis, "{name} ({x},{y})");
                }
            }
        }
    }

    #[test]
    fn the_embedded_skin_has_the_tabs_home_markers_and_fade_of_the_mockup() {
        let data = decode(EMBEDDED).unwrap();
        let alpha = |piece: &PieceImage, x: u32, y: u32| {
            piece.rgba[((y * piece.size[0] + x) * 4 + 3) as usize]
        };
        // Las pestañas se estiran solo a lo ancho: 3 columnas fijas de cada lado y la del medio.
        for (name, height) in [("tab", 15), ("tab.active", 17)] {
            let tab = data.pieces.get(name).unwrap_or_else(|| panic!("{name}"));
            let center = Rect9 {
                x: 3,
                y: 0,
                w: 1,
                h: height,
            };
            assert_eq!(
                (tab.size, tab.center),
                ([7, height], Some(center)),
                "{name}"
            );
            // Esquinas de arriba redondeadas; abajo, abierta.
            for (x, y) in [(0, 0), (1, 0), (0, 1), (5, 0), (6, 0), (6, 1)] {
                assert_eq!(alpha(tab, x, y), 0, "{name} ({x},{y})");
            }
            assert_eq!(alpha(tab, 0, height - 1), 255, "{name}: the left side");
        }
        // La casita animada: 3 frames de 21×21 (casita y humo juntos) de 100 ms, como en
        // `home animation.aseprite`. Las piezas sueltas de antes ya no están.
        for i in 0..3 {
            let name = format!("icon.home.{i}");
            let icon = data.pieces.get(&name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(
                (icon.size, icon.center, icon.duration_ms),
                ([21, 21], None, Some(100)),
                "{name}"
            );
        }
        assert!(!data.pieces.contains_key("icon.home.3"));
        for name in ["icon.home", "icon.home_smoke"] {
            assert!(!data.pieces.contains_key(name), "{name}: replaced");
        }
        // Los frames son distintos (el humo se mueve) y la casita, la misma.
        assert_ne!(
            data.pieces["icon.home.0"].rgba,
            data.pieces["icon.home.1"].rgba
        );
        let ground = &data.pieces["home.ground"];
        assert_eq!(ground.size, [58, 1]);
        for name in ["icon.tab_dirty", "icon.tab_close"] {
            let mark = data.pieces.get(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!((mark.size, mark.center), ([5, 5], None), "{name}");
        }
        assert!(
            !data.pieces.contains_key("icon.tab_dirty.hover"),
            "no hover look"
        );
        assert!(
            !data.pieces.contains_key("icon.tab_close.hover"),
            "no hover look"
        );
        let fade = &data.pieces["menubar.fade"];
        assert_eq!((fade.size, fade.center), ([122, 34], None));
        // Contra el borde derecho es todo café; a la izquierda, casi nada.
        assert_eq!(alpha(fade, 121, 0), 255);
        assert_eq!(alpha(fade, 0, 0), 0);
        assert_eq!(data.colors["tab_text_dim"], Rgba::rgb(89, 77, 77));
        assert_eq!(data.colors["home_text"], Rgba::rgb(51, 39, 42));
    }

    #[test]
    fn the_embedded_skin_has_the_ruler_digits_smiley_and_workspace_box_of_the_mockup() {
        let data = decode(EMBEDDED).unwrap();
        // Las cifras del mockup miden 3×5, salvo el 1, de 1×5.
        for d in 0..10 {
            let name = format!("digit.{d}");
            let digit = data.pieces.get(&name).unwrap_or_else(|| panic!("{name}"));
            let width = if d == 1 { 1 } else { 3 };
            assert_eq!((digit.size, digit.center), ([width, 5], None), "{name}");
        }
        let corner = &data.pieces["ruler.corner"];
        assert_eq!((corner.size, corner.center), ([11, 11], None));
        // La caja: línea clara redondeada, aire y línea oscura con la punta cortada; el centro,
        // vacío, se estira.
        let frame = &data.pieces["workspace.box"];
        let center = Rect9 {
            x: 3,
            y: 3,
            w: 1,
            h: 1,
        };
        assert_eq!((frame.size, frame.center), ([7, 7], Some(center)));
        let alpha = |x: u32, y: u32| frame.rgba[((y * 7 + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0, "rounded corner");
        assert_eq!(alpha(0, 3), 255, "the light line");
        assert_eq!(alpha(1, 3), 0, "the air");
        assert_eq!(alpha(2, 3), 255, "the dark line");
        assert_eq!(alpha(2, 2), 0, "the dark line's cut corner");
        assert_eq!(alpha(3, 3), 0, "the canvas goes inside");
        assert_ne!(
            data.pieces["workspace.box"].rgba, data.pieces["toolbar.box"].rgba,
            "not the toolbar's box: this one has the dark line"
        );
        assert_eq!(data.colors["ruler_tick"], Rgba::rgb(136, 110, 106));
    }

    #[test]
    fn the_embedded_skin_has_the_menu_pieces_of_the_design() {
        let data = decode(EMBEDDED).unwrap();
        let alpha = |piece: &PieceImage, x: u32, y: u32| {
            piece.rgba[((y * piece.size[0] + x) * 4 + 3) as usize]
        };
        let rect9 = |x, y, w, h| Some(Rect9 { x, y, w, h });
        // La caja del título abierto: redondeada arriba, abierta abajo (se une al panel).
        let title = &data.pieces["menu.title.open"];
        assert_eq!((title.size, title.center), ([25, 18], rect9(2, 2, 21, 14)));
        assert_eq!(alpha(title, 0, 0), 0, "rounded corner");
        assert_eq!(alpha(title, 0, 2), 255, "the left side");
        assert_eq!(alpha(title, 12, 17), 255, "open below: filled");
        // El panel: bordes de 2 (línea y bisel) y un centro liso.
        let panel = &data.pieces["menu.panel"];
        assert_eq!((panel.size, panel.center), ([5, 5], rect9(2, 2, 1, 1)));
        for (x, y) in [(0, 0), (4, 0), (0, 4), (4, 4)] {
            assert_eq!(alpha(panel, x, y), 0, "rounded ({x},{y})");
        }
        // La tecla Ctrl como está dibujada (sin centro) y la tecla sin texto, que se estira.
        let ctrl = &data.pieces["key.ctrl"];
        assert_eq!((ctrl.size, ctrl.center), ([19, 7], None));
        let key = &data.pieces["key"];
        assert_eq!((key.size, key.center), ([19, 7], rect9(1, 1, 17, 5)));
        assert_ne!(ctrl.rgba, key.rgba, "Ctrl has its letters");
        assert_eq!(alpha(key, 0, 0), 0, "rounded corner");
        assert_eq!(alpha(key, 9, 3), 255, "no text");
        // La tecla Shift del diseño (Layer 4, frame 4), como Ctrl: sin centro, con las letras
        // y las esquinas caladas.
        let shift = &data.pieces["key.shift"];
        assert_eq!((shift.size, shift.center), ([22, 7], None));
        assert_eq!(alpha(shift, 0, 0), 0, "rounded corner");
        assert_eq!(alpha(shift, 21, 6), 0, "rounded corner");
        assert_eq!(alpha(shift, 1, 0), 255, "the top");
        assert_eq!(alpha(shift, 0, 3), 255, "the left side");
        assert_eq!(alpha(shift, 4, 1), 0, "the S, cut out");
        // La casilla marcada del diseño (Layer 4, frame 4): 11×12, sin centro, con las
        // esquinas de afuera caladas y el interior blanco. La sin marcar no está dibujada.
        let on = &data.pieces["menu.check.on"];
        assert_eq!((on.size, on.center), ([11, 12], None));
        assert_eq!(alpha(on, 0, 0), 0, "rounded corner");
        assert_eq!(alpha(on, 10, 0), 0, "rounded corner");
        assert_eq!(alpha(on, 1, 0), 255, "the top");
        assert_eq!(alpha(on, 0, 1), 255, "the left side");
        let at = |x: u32, y: u32| {
            let i = ((y * on.size[0] + x) * 4) as usize;
            Rgba::new(on.rgba[i], on.rgba[i + 1], on.rgba[i + 2], on.rgba[i + 3])
        };
        assert_eq!(at(1, 1), Rgba::rgb(255, 255, 255), "white inside");
        assert!(!data.pieces.contains_key("menu.check"), "not drawn yet");
        // El brillo del separador; la otra línea es `background` y el atajo, `ruler_tick`.
        assert_eq!(data.colors["menu_highlight"], Rgba::rgb(255, 255, 255));
        assert_eq!(data.colors["background"], Rgba::rgb(221, 206, 191));
        assert_eq!(data.colors["ruler_tick"], Rgba::rgb(136, 110, 106));
    }

    #[test]
    fn the_embedded_skin_has_the_options_bar_fields_of_the_design() {
        let data = decode(EMBEDDED).unwrap();
        let at = |piece: &PieceImage, x: u32, y: u32| {
            let i = ((y * piece.size[0] + x) * 4) as usize;
            Rgba::new(
                piece.rgba[i],
                piece.rgba[i + 1],
                piece.rgba[i + 2],
                piece.rgba[i + 3],
            )
        };
        let rect9 = |x, y, w, h| Some(Rect9 { x, y, w, h });
        let white = Rgba::rgb(255, 255, 255);
        let border = Rgba::rgb(136, 110, 106);
        // Los íconos de los modos de selección (frame 4): el interior de 11×11 de cada cuadrito,
        // que es el de `sym.slot`. Fondo transparente.
        for name in ["replace", "add", "subtract", "intersect"] {
            let icon = &data.pieces[&format!("icon.sel_{name}")];
            assert_eq!((icon.size, icon.center), ([11, 11], None), "{name}");
            let drawn = (0..11 * 11).filter(|i| icon.rgba[i * 4 + 3] == 255).count();
            assert!((10..121).contains(&drawn), "{name}: {drawn}");
        }
        // El combo: borde izquierdo de 2, relleno de 1 que se estira y a la derecha la flecha
        // (10 columnas). 16 filas con el brillo de abajo.
        let combo = &data.pieces["field.combo"];
        assert_eq!((combo.size, combo.center), ([13, 16], rect9(2, 10, 1, 3)));
        assert_eq!(at(combo, 0, 0).a, 0, "rounded corner");
        assert_eq!(at(combo, 0, 2), border, "the left side");
        assert_eq!(at(combo, 2, 5), white, "the fill");
        assert_eq!(at(combo, 12, 5), border, "the right side");
        let arrow = Rgba::rgb(5, 4, 3);
        for (x, y) in [(3, 7), (7, 7), (4, 8), (6, 8), (5, 9)] {
            assert_eq!(at(combo, x, y), arrow, "the arrow ({x},{y})");
        }
        assert_eq!(
            at(combo, 2, 15),
            Rgba::rgb(241, 235, 219),
            "the light below"
        );
        // El campo numérico: el mismo borde, con el brillo de arriba a la derecha; la unidad "px",
        // que tapa las 2 últimas columnas del campo.
        let number = &data.pieces["field.number"];
        assert_eq!((number.size, number.center), ([10, 16], rect9(2, 10, 1, 3)));
        assert_eq!(at(number, 0, 2), border);
        assert_eq!(at(number, 9, 2), border);
        assert_eq!(at(number, 9, 0).a, 0, "rounded corner");
        assert_eq!(at(number, 3, 2), Rgba::rgb(236, 235, 231), "the shine");
        let unit = &data.pieces["field.unit"];
        assert_eq!((unit.size, unit.center), ([17, 16], None));
        assert_eq!(at(unit, 0, 5), white, "the last column of the field");
        assert_eq!(at(unit, 1, 5), border, "the divider");
        assert_eq!(at(unit, 3, 5), Rgba::rgb(189, 164, 153), "the grey box");
        assert_eq!(at(unit, 4, 5), Rgba::rgb(89, 77, 77), "the p");
        assert_eq!(at(unit, 16, 0).a, 0, "rounded corner");
        // La casilla marcada de la barra: 13×14, como el cuadro de `sym.slot` con el interior
        // blanco y la marca verde. No es la del menú (11×12). La sin marcar no está dibujada.
        let check = &data.pieces["field.check.on"];
        assert_eq!((check.size, check.center), ([13, 14], None));
        assert_eq!(at(check, 0, 0).a, 0, "rounded corner");
        assert_eq!(at(check, 1, 0), border, "the top");
        assert_eq!(at(check, 1, 1), white, "white inside");
        assert_eq!(at(check, 2, 6), Rgba::rgb(71, 114, 56), "the green check");
        assert_ne!(check.size, data.pieces["menu.check.on"].size);
        assert!(!data.pieces.contains_key("field.check"), "not drawn yet");
    }

    #[test]
    fn the_embedded_skin_has_the_tooltip_bubble_of_the_mockup() {
        let data = decode(EMBEDDED).unwrap();
        let bubble = data.pieces.get("tooltip.bubble").expect("tooltip.bubble");
        // El globo junto a la barra del mockup (frame 2): la cola de 8 a la izquierda y el
        // cuerpo de 41×20. El centro deja afuera la cola (filas 8..12, columnas 0..7), las
        // esquinas redondeadas, el brillo de arriba a la derecha y la sombra de abajo.
        let center = Rect9 {
            x: 11,
            y: 13,
            w: 34,
            h: 4,
        };
        assert_eq!((bubble.size, bubble.center), ([49, 20], Some(center)));
        let at = |x: u32, y: u32| {
            let i = ((y * bubble.size[0] + x) * 4) as usize;
            Rgba::new(
                bubble.rgba[i],
                bubble.rgba[i + 1],
                bubble.rgba[i + 2],
                bubble.rgba[i + 3],
            )
        };
        let fill = Rgba::rgb(232, 138, 54);
        assert_eq!(at(0, 8), fill, "the tip of the tail");
        assert_eq!(at(0, 9), Rgba::rgb(176, 91, 44), "the tail's shadow");
        assert_eq!(at(0, 7).a, 0, "above the tail");
        assert_eq!(
            at(0, 13).a,
            0,
            "below the tail, the stretched rows are empty"
        );
        assert_eq!(at(8, 0).a, 0, "rounded corner");
        assert_eq!(at(10, 0), fill, "the top");
        assert_eq!(at(20, 10), fill, "the fill");
        assert_eq!(at(20, 19), Rgba::rgb(176, 91, 44), "the shadow below");
        assert_eq!(at(47, 2), Rgba::rgb(248, 197, 58), "the shine, top right");
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
