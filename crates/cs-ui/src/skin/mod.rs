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
pub const COLOR_NAMES: [&str; 16] = [
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
    "field_text",
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
    /// El texto de la tira de canales del editor de color (y su borde).
    pub field_text: Color32,
    /// Los enlaces de Home (el nombre de cada reciente).
    pub link: Color32,
    /// Lo que acompaña a un enlace (la ruta de cada reciente).
    pub path: Color32,
    /// Los ejes de simetría sobre el lienzo.
    pub symmetry: Color32,
    /// Lo elegido en la línea de tiempo (la capa activa, el frame actual, el rango): el relleno,
    /// la línea oscura y su brillo.
    pub timeline_selection: Color32,
    pub timeline_selection_dark: Color32,
    pub timeline_selection_light: Color32,
    /// El texto de un botón deshabilitado (el botón se ve igual, sin apagarse).
    pub button_text_disabled: Color32,
    /// El texto de las opciones no elegidas de un grupo de botones.
    pub option_text_off: Color32,
    /// El borde oscuro de los controles (campos, botones, cuadritos de color).
    pub control_border: Color32,
    /// La línea oscura a cada lado del contorno blanco de un rango de cels del timeline.
    pub cel_range: Color32,
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
            field_text: egui::Visuals::dark().strong_text_color(),
            link: theme::ACCENT,
            path: theme::LABEL,
            symmetry: theme::ACCENT,
            timeline_selection: theme::ACCENT,
            timeline_selection_dark: theme::CANVAS_BORDER,
            timeline_selection_light: theme::WIDGET_HOVER,
            button_text_disabled: theme::LABEL,
            option_text_off: theme::LABEL,
            control_border: theme::CANVAS_BORDER,
            cel_range: theme::CANVAS_BORDER,
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
            ("field_text", &mut c.field_text),
            ("link", &mut c.link),
            ("path", &mut c.path),
            ("symmetry", &mut c.symmetry),
            ("timeline_selection", &mut c.timeline_selection),
            ("timeline_selection_dark", &mut c.timeline_selection_dark),
            ("timeline_selection_light", &mut c.timeline_selection_light),
            ("button_text_disabled", &mut c.button_text_disabled),
            ("option_text_off", &mut c.option_text_off),
            ("control_border", &mut c.control_border),
            ("cel_range", &mut c.cel_range),
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
    /// Los píxeles (RGBA sin premultiplicar, fila por fila), para quien los lee uno por uno
    /// (los cursores, que se invierten según lo de abajo).
    pub rgba: Vec<u8>,
}

impl Piece {
    /// El color del píxel `(x, y)` (transparente fuera de la pieza).
    pub fn pixel(&self, x: u32, y: u32) -> Color32 {
        let [w, h] = self.size;
        if x >= w || y >= h {
            return Color32::TRANSPARENT;
        }
        let i = ((y * w + x) * 4) as usize;
        let c = &self.rgba[i..i + 4];
        Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
    }
}

#[derive(Clone)]
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
                    rgba: img.rgba,
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

    /// El mismo skin con solo las piezas `names` (y sus variantes `.hover`, `.down`, `.active`):
    /// una copia chica para guardar en la memoria de egui.
    pub fn subset(&self, names: &[&str]) -> Skin {
        let pieces = self
            .pieces
            .iter()
            .filter(|(name, _)| {
                names.iter().any(|n| {
                    name.as_str() == *n
                        || name
                            .strip_prefix(*n)
                            .is_some_and(|rest| [".hover", ".down", ".active"].contains(&rest))
                })
            })
            .map(|(name, piece)| (name.clone(), piece.clone()))
            .collect();
        Skin {
            pieces,
            colors: self.colors,
        }
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
mod tests;
