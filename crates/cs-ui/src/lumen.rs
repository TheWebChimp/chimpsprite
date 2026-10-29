//! Lumen, en el dock: los mapas para iluminar un sprite en un motor (normales y lo que brilla
//! solo) con una luz de prueba encima. El albedo es la capa actual o una imagen; el normal map
//! sale del algoritmo de Sprite Fusion (`cs_render::normal_map`) o de un archivo, y la máscara
//! de luminance, de los colores claros o de un archivo. Los controles siguen a Pixel Art Lighter.

use cs_core::Rgba;
use cs_io::export::Bitmap;
use cs_render::{Light, NormalFormat, NormalSettings};
use eframe::egui::{
    self, Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, Ui, pos2, vec2,
};

use crate::panel_kit::{box_color, child, double_box, hint, label_with_hint, notched_outline};
use crate::pixel_ui::Tip as _;
use crate::skin::WidgetState;
use crate::{pixel_ui, widgets};

/// El panel, desde la esquina de su contenido: su ancho (el de Groom) y el alto más chico.
pub const WIDTH: f32 = 434.0;
const MIN_HEIGHT: f32 = 391.0;
/// La línea grabada debajo del encabezado y donde empieza lo de abajo.
const HEADER_LINE: f32 = 21.0;
const TOP: f32 = 25.0;
/// Las tres columnas (los mapas, los sliders): su ancho y la separación.
const COLUMN: f32 = 140.0;
const COLUMN_GAP: f32 = 7.0;
/// La caja de la miniatura de cada mapa.
const THUMB_BOX: f32 = 40.0;
/// Entre los botones de una fila.
const BUTTON_GAP: f32 = 2.0;

/// El ancho de cada uno de `n` botones parejos en una columna.
fn button_width(n: usize) -> f32 {
    ((COLUMN - (n - 1) as f32 * BUTTON_GAP) / n as f32).floor()
}
/// Del borde de una caja a lo de adentro: las dos líneas y 2 de aire.
const BOX_PAD: f32 = 5.0;
/// Debajo de la vista previa: el tamaño y el zoom.
const BELOW_PREVIEW: f32 = 26.0;
const MIN_PREVIEW: f32 = 40.0;
/// La línea de abajo y los botones, desde el fondo del panel.
const FOOTER_LINE: f32 = 22.0;
const FOOTER_BUTTONS: f32 = pixel_ui::CONTROL_HEIGHT;
/// El zoom más grande de la vista previa.
const MAX_ZOOM: f32 = 32.0;
/// El gris del albedo en "Light Only".
const LIGHT_ONLY_GRAY: u8 = 160;

/// Uno de los tres mapas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapKind {
    Albedo,
    Normal,
    Luminance,
}

impl MapKind {
    fn label(self) -> &'static str {
        match self {
            MapKind::Albedo => "Albedo",
            MapKind::Normal => "Normal",
            MapKind::Luminance => "Luminance",
        }
    }

    /// El final del nombre del archivo al exportarlo.
    fn suffix(self) -> &'static str {
        match self {
            MapKind::Albedo => "",
            MapKind::Normal => "_n",
            MapKind::Luminance => "_e",
        }
    }
}

/// Lo que se ve en la vista previa.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Final,
    Albedo,
    Normal,
    Luminance,
    /// La luz sobre el sprite en gris.
    LightOnly,
}

impl View {
    const ALL: [View; 5] = [
        View::Final,
        View::Albedo,
        View::Normal,
        View::Luminance,
        View::LightOnly,
    ];

    fn label(self) -> &'static str {
        match self {
            View::Final => "Final",
            View::Albedo => "Albedo",
            View::Normal => "Normal",
            View::Luminance => "Luminance",
            View::LightOnly => "Light Only",
        }
    }
}

/// Cómo se dibuja la luz sobre la vista previa.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LightCursor {
    /// El punto, el aro del falloff y la línea de la altura.
    #[default]
    Advanced,
    Simple,
    Hidden,
}

impl LightCursor {
    const ALL: [LightCursor; 3] = [
        LightCursor::Advanced,
        LightCursor::Simple,
        LightCursor::Hidden,
    ];

    fn label(self) -> &'static str {
        match self {
            LightCursor::Advanced => "Advanced",
            LightCursor::Simple => "Simple",
            LightCursor::Hidden => "Hidden",
        }
    }
}

/// De dónde sale un mapa: del albedo, de un archivo o de ningún lado.
#[derive(Clone, Debug, PartialEq)]
enum Source<T> {
    None,
    Generated,
    Loaded(T),
}

/// Los sliders de la luz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Altura de la luz, en píxeles.
    pub height: f32,
    /// Distancia a la que la luz baja a la mitad, en píxeles.
    pub falloff: f32,
    pub brightness: f32,
    /// 0..100 %.
    pub ambient: f32,
    /// 0..100 %.
    pub luminance: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            height: 24.0,
            falloff: 48.0,
            brightness: 2.0,
            ambient: 25.0,
            luminance: 100.0,
        }
    }
}

/// Lo que se pidió desde el panel (la app lo resuelve).
pub enum LumenAction {
    /// Cargar un mapa desde un archivo.
    Browse(MapKind),
    UseLayer,
    /// Guardar un PNG con este nombre.
    Export(String, Bitmap),
    NewSprite(String, Bitmap),
    AddLayer(String, Bitmap),
    AddArtboard(String, Bitmap),
}

/// Lo que se calculó y con qué: si cambia, se vuelve a calcular.
type NormalKey = (u64, Option<Rgba>, NormalSettings);
type MaskKey = (u64, Option<Rgba>, u32);

pub struct LumenPanel {
    /// El albedo y su nombre.
    albedo: Option<(String, Bitmap)>,
    /// Cambia con cada albedo nuevo (para la caché).
    generation: u64,
    normal: Source<Vec<Rgba>>,
    luminance: Source<Vec<u8>>,
    /// Los ajustes del normal map (también los usa Export).
    pub normals: NormalSettings,
    /// Desde qué luminosidad (0..1) un color brilla solo.
    threshold: f32,
    /// Los ajustes de qué generador se ven.
    modify: Option<MapKind>,
    view: View,
    light_color: Rgba,
    cursor: LightCursor,
    follow_mouse: bool,
    /// El color de fondo del albedo y si cuenta como transparente.
    background: Option<Rgba>,
    hide_background: bool,
    params: Params,
    /// Dónde está la luz, en píxeles del albedo (`None`: el lugar de siempre).
    light_at: Option<(f32, f32)>,
    /// El zoom (`None`: que entre) y el desplazamiento de la imagen en la vista previa.
    zoom: Option<f32>,
    pan: egui::Vec2,
    normal_cache: Option<(NormalKey, Vec<Rgba>)>,
    mask_cache: Option<(MaskKey, Vec<u8>)>,
    /// La imagen de la vista previa y su textura.
    texture: Option<(Bitmap, TextureHandle)>,
    /// Las miniaturas de los mapas, con la imagen de cada una.
    thumbs: Vec<(MapKind, Bitmap, TextureHandle)>,
    /// Un aviso para el usuario (un mapa que no se pudo cargar).
    notice: Option<String>,
}

impl Default for LumenPanel {
    fn default() -> Self {
        Self {
            albedo: None,
            generation: 0,
            normal: Source::Generated,
            luminance: Source::None,
            normals: NormalSettings::default(),
            threshold: 0.85,
            modify: None,
            view: View::default(),
            light_color: Rgba::rgb(255, 214, 120),
            cursor: LightCursor::default(),
            follow_mouse: false,
            background: None,
            hide_background: false,
            params: Params::default(),
            light_at: None,
            zoom: None,
            pan: egui::Vec2::ZERO,
            normal_cache: None,
            mask_cache: None,
            texture: None,
            thumbs: Vec::new(),
            notice: None,
        }
    }
}

impl LumenPanel {
    /// Empieza con `image` de albedo: el normal map se genera y la máscara queda vacía.
    pub fn load(&mut self, name: String, image: Bitmap) {
        self.background = cs_render::detect_background(&image.pixels, image.width, image.height);
        self.hide_background = false;
        self.albedo = Some((name, image));
        self.generation += 1;
        self.normal = Source::Generated;
        self.luminance = Source::None;
        self.light_at = None;
        self.zoom = None;
        self.pan = egui::Vec2::ZERO;
        self.notice = None;
    }

    /// Carga un mapa de un archivo. Los de normales y luminance tienen que medir lo mismo que
    /// el albedo.
    pub fn load_map(&mut self, kind: MapKind, name: String, image: Bitmap) -> Result<(), String> {
        if kind == MapKind::Albedo {
            self.load(name, image);
            return Ok(());
        }
        let Some((_, albedo)) = &self.albedo else {
            return Err("Load the albedo first.".into());
        };
        if (image.width, image.height) != (albedo.width, albedo.height) {
            return Err(format!(
                "The {} map is {}×{}; it must be {}×{} like the albedo.",
                kind.label().to_lowercase(),
                image.width,
                image.height,
                albedo.width,
                albedo.height
            ));
        }
        match kind {
            MapKind::Normal => self.normal = Source::Loaded(image.pixels),
            _ => {
                // Lo claro brilla: el gris del píxel por su alfa.
                let mask = image
                    .pixels
                    .iter()
                    .map(|p| (p.to_gray().r as u16 * p.a as u16 / 255) as u8)
                    .collect();
                self.luminance = Source::Loaded(mask);
            }
        }
        Ok(())
    }

    /// Desarrollo (capturas): con los ajustes del generador de `kind` abiertos (y la máscara
    /// generada, si es la de luminance).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open_settings(&mut self, kind: MapKind) {
        self.modify = Some(kind);
        self.generate(kind);
    }

    /// Un aviso para mostrar en el panel.
    pub fn notify(&mut self, text: String) {
        self.notice = Some(text);
    }

    /// El fondo que cuenta como transparente.
    fn hidden_background(&self) -> Option<Rgba> {
        self.background.filter(|_| self.hide_background)
    }

    /// El albedo como se usa: sin el fondo, si se esconde.
    fn albedo_pixels(&self) -> Option<Vec<Rgba>> {
        let (_, image) = self.albedo.as_ref()?;
        let hidden = self.hidden_background();
        Some(
            image
                .pixels
                .iter()
                .map(|&p| {
                    if Some(p) == hidden {
                        Rgba::TRANSPARENT
                    } else {
                        p
                    }
                })
                .collect(),
        )
    }

    /// El normal map de ahora: el generado (de la caché si nada cambió), el cargado o, sin
    /// ninguno, plano.
    fn normal_pixels(&mut self) -> Option<Vec<Rgba>> {
        let albedo = self.albedo_pixels()?;
        let (_, image) = self.albedo.as_ref()?;
        let (w, h) = (image.width, image.height);
        Some(match &self.normal {
            Source::Loaded(pixels) => pixels.clone(),
            Source::None => albedo
                .iter()
                .map(|p| Rgba::new(128, 128, 255, p.a))
                .collect(),
            Source::Generated => {
                let key = (self.generation, self.hidden_background(), self.normals);
                if !matches!(&self.normal_cache, Some((k, _)) if *k == key) {
                    let map = cs_render::normal_map(&albedo, w, h, &self.normals);
                    self.normal_cache = Some((key, map));
                }
                self.normal_cache.as_ref().map(|(_, m)| m.clone())?
            }
        })
    }

    /// La máscara de luminance de ahora (`None` si no hay).
    fn mask(&mut self) -> Option<Vec<u8>> {
        let albedo = self.albedo_pixels()?;
        match &self.luminance {
            Source::None => None,
            Source::Loaded(mask) => Some(mask.clone()),
            Source::Generated => {
                let key = (
                    self.generation,
                    self.hidden_background(),
                    self.threshold.to_bits(),
                );
                if !matches!(&self.mask_cache, Some((k, _)) if *k == key) {
                    let mask = cs_render::luminance_mask(&albedo, self.threshold);
                    self.mask_cache = Some((key, mask));
                }
                self.mask_cache.as_ref().map(|(_, m)| m.clone())
            }
        }
    }

    /// La luz con los sliders de ahora.
    fn light(&self, w: u32, h: u32) -> Light {
        let (x, y) = self.light_at.unwrap_or((w as f32 / 2.0, h as f32 / 4.0));
        Light {
            x,
            y,
            height: self.params.height,
            falloff: self.params.falloff,
            color: self.light_color,
            brightness: self.params.brightness,
            ambient: self.params.ambient / 100.0,
            luminance: self.params.luminance / 100.0,
        }
    }

    /// Un mapa como imagen (para exportarlo o verlo).
    fn map_image(&mut self, kind: MapKind) -> Option<Bitmap> {
        let albedo = self.albedo_pixels()?;
        let (_, image) = self.albedo.as_ref()?;
        let (width, height) = (image.width, image.height);
        let pixels = match kind {
            MapKind::Albedo => albedo,
            MapKind::Normal => self.normal_pixels()?,
            MapKind::Luminance => {
                let mask = self.mask().unwrap_or_else(|| vec![0; albedo.len()]);
                mask.iter()
                    .zip(&albedo)
                    .map(|(&v, p)| Rgba::new(v, v, v, p.a))
                    .collect()
            }
        };
        Some(Bitmap {
            width,
            height,
            pixels,
        })
    }

    /// Lo que muestra View.
    pub fn shown(&mut self) -> Option<Bitmap> {
        let (_, image) = self.albedo.as_ref()?;
        let (w, h) = (image.width, image.height);
        let lit = |panel: &mut Self, albedo: Vec<Rgba>| -> Option<Bitmap> {
            let normal = panel.normal_pixels()?;
            let mask = panel.mask();
            let light = panel.light(w, h);
            // Lo cargado también se lee en el formato de los ajustes.
            let format = panel.normals.format;
            let pixels = cs_render::lit(&albedo, &normal, mask.as_deref(), w, &light, format);
            Some(Bitmap {
                width: w,
                height: h,
                pixels,
            })
        };
        match self.view {
            View::Final => {
                let albedo = self.albedo_pixels()?;
                lit(self, albedo)
            }
            View::LightOnly => {
                let gray = Rgba::rgb(LIGHT_ONLY_GRAY, LIGHT_ONLY_GRAY, LIGHT_ONLY_GRAY);
                let albedo = self.albedo_pixels()?;
                let albedo = albedo.iter().map(|p| Rgba { a: p.a, ..gray }).collect();
                lit(self, albedo)
            }
            View::Albedo => self.map_image(MapKind::Albedo),
            View::Normal => self.map_image(MapKind::Normal),
            View::Luminance => self.map_image(MapKind::Luminance),
        }
    }

    fn name(&self) -> String {
        self.albedo
            .as_ref()
            .map_or_else(|| "lumen".into(), |(n, _)| n.clone())
    }

    /// El panel: arriba el logo; los tres mapas; los ajustes del generador que se modifica; la
    /// vista y los sliders; la vista previa y, abajo, lo que se hace con lo que se ve.
    pub fn ui(&mut self, ui: &mut Ui) -> Option<LumenAction> {
        let mut action = None;
        let origin = ui.cursor().min;
        let at = |x: f32, y: f32| origin + vec2(x, y);
        let available = ui.available_height();
        let height = if available.is_finite() {
            available.max(MIN_HEIGHT)
        } else {
            MIN_HEIGHT
        };
        let skin = pixel_ui::frame_skin(ui.ctx());
        let ppp = ui.ctx().pixels_per_point();
        // El encabezado: el logo y, a la derecha, el crédito del algoritmo.
        let logo = skin.as_ref().and_then(|s| {
            s.piece("lumen.logo", crate::skin::WidgetState::Normal)
                .cloned()
        });
        match logo {
            Some(piece) => {
                let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
                let rect = Rect::from_min_size(origin + vec2(-3.0, -1.0), size);
                ui.painter()
                    .extend(pixel_ui::piece_shapes(&piece, rect, ppp));
            }
            None => {
                ui.painter().text(
                    at(0.0, 0.0),
                    egui::Align2::LEFT_TOP,
                    "Lumen",
                    egui::TextStyle::Heading.resolve(ui.style()),
                    ui.visuals().strong_text_color(),
                );
            }
        }
        let header = Rect::from_min_max(at(0.0, 0.0), at(WIDTH, HEADER_LINE - 2.0));
        let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
        // El crédito del algoritmo de las normales.
        hint(
            &mut child(ui, header, right_to_left),
            "normals: sprite fusion",
        );
        let footer_line = height - FOOTER_LINE;
        let mut lines = Vec::new();
        let margin = f32::from(crate::dock::FLYOUT_MARGIN) - 1.0;
        let across = egui::Rangef::new(origin.x - margin, origin.x + WIDTH + margin);
        let top_down = egui::Layout::top_down(egui::Align::Min);
        let body = Rect::from_min_max(at(0.0, TOP), at(WIDTH, footer_line));
        let mut body = child(ui, body, top_down);
        self.maps(&mut body, &mut action);
        body.add_space(5.0);
        let controls_line = body.cursor().top();
        // Debajo: a la izquierda (bajo Albedo) la vista y la luz, o con Modify los ajustes del
        // generador; a la derecha, la vista previa con el zoom abajo.
        let below = controls_line + 6.0;
        let left = Rect::from_min_max(
            pos2(origin.x, below),
            pos2(origin.x + COLUMN, origin.y + footer_line),
        );
        let mut left = child(ui, left, top_down);
        left.spacing_mut().item_spacing.y = 3.0;
        match self.modify {
            Some(kind) => self.generator_settings(&mut left, kind),
            None => {
                self.view_controls(&mut left);
                left.add_space(2.0);
                self.sliders(&mut left);
            }
        }
        let right = Rect::from_min_max(
            pos2(origin.x + COLUMN + COLUMN_GAP, below),
            pos2(origin.x + WIDTH, origin.y + footer_line - 3.0),
        );
        let mut right = child(ui, right, top_down);
        let preview_height = (right.max_rect().height() - BELOW_PREVIEW).max(MIN_PREVIEW);
        self.preview(&mut right, preview_height);
        if let Some(skin) = &skin {
            let colors = skin.colors();
            lines.push(pixel_ui::etched_hline(
                across,
                origin.y + HEADER_LINE,
                colors,
                ppp,
            ));
            lines.push(pixel_ui::etched_hline(across, controls_line, colors, ppp));
            lines.push(pixel_ui::etched_hline(
                across,
                origin.y + footer_line,
                colors,
                ppp,
            ));
            ui.painter().extend(lines);
        }
        // Abajo, a la derecha: lo que muestra View, a un sprite, una capa o un artboard.
        let footer = Rect::from_min_max(at(0.0, height - FOOTER_BUTTONS), at(WIDTH, height));
        let mut footer = child(ui, footer, egui::Layout::right_to_left(egui::Align::Min));
        footer.add_enabled_ui(self.albedo.is_some(), |ui| {
            let name = format!("{} ({})", self.name(), self.view.label());
            if pixel_ui::text_button(ui, "Add as Artboard").clicked() {
                action = self
                    .shown()
                    .map(|b| LumenAction::AddArtboard(name.clone(), b));
            }
            if pixel_ui::text_button(ui, "Add as Layer").clicked() {
                action = self.shown().map(|b| LumenAction::AddLayer(name.clone(), b));
            }
            if pixel_ui::text_button(ui, "New Sprite").clicked() {
                action = self
                    .shown()
                    .map(|b| LumenAction::NewSprite(name.clone(), b));
            }
        });
        ui.allocate_rect(
            Rect::from_min_size(origin, vec2(WIDTH, height)),
            Sense::hover(),
        );
        action
    }

    /// Los tres mapas en columnas: título y tamaño, miniatura y botones.
    fn maps(&mut self, ui: &mut Ui, action: &mut Option<LumenAction>) {
        let size = self
            .albedo
            .as_ref()
            .map(|(_, b)| format!("{}×{}", b.width, b.height));
        let top = ui.cursor().top();
        let left = ui.cursor().left();
        let mut bottom = top;
        for (i, kind) in [MapKind::Albedo, MapKind::Normal, MapKind::Luminance]
            .into_iter()
            .enumerate()
        {
            let x = left + i as f32 * (COLUMN + COLUMN_GAP);
            let rect = Rect::from_min_size(pos2(x, top), vec2(COLUMN, 200.0));
            let mut column = child(ui, rect, egui::Layout::top_down(egui::Align::Min));
            column.spacing_mut().item_spacing.y = 3.0;
            let present = match kind {
                MapKind::Albedo => self.albedo.is_some(),
                MapKind::Normal => self.albedo.is_some() && self.normal != Source::None,
                MapKind::Luminance => self.albedo.is_some() && self.luminance != Source::None,
            };
            let note = match (&size, kind) {
                (None, _) => "none".to_string(),
                (Some(_), _) if !present => "none".to_string(),
                (Some(s), MapKind::Albedo) => s.clone(),
                (Some(_), MapKind::Normal) => self.source_note(&self.normal).into(),
                (Some(_), MapKind::Luminance) => self.source_note(&self.luminance).into(),
            };
            label_with_hint(&mut column, kind.label(), &note);
            self.thumbnail(&mut column, kind, present);
            column.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = BUTTON_GAP;
                if pixel_ui::text_button_sized(ui, "Browse", button_width(2)).clicked() {
                    *action = Some(LumenAction::Browse(kind));
                }
                let remove = ui.add_enabled_ui(present, |ui| {
                    pixel_ui::text_button_sized(ui, "Remove", button_width(2))
                });
                if remove.inner.clicked() && present {
                    self.remove(kind);
                }
            });
            column.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = BUTTON_GAP;
                if kind == MapKind::Albedo {
                    if pixel_ui::text_button_sized(ui, "Use Layer", COLUMN).clicked() {
                        *action = Some(LumenAction::UseLayer);
                    }
                    return;
                }
                let third = button_width(3);
                ui.add_enabled_ui(self.albedo.is_some(), |ui| {
                    if pixel_ui::text_button_sized(ui, "Generate", third).clicked() {
                        self.generate(kind);
                    }
                    let open = self.modify == Some(kind);
                    let modify = pixel_ui::text_button_sized(ui, "Modify", third)
                        .on_hover_text("Settings to generate this map");
                    if modify.clicked() {
                        self.modify = if open { None } else { Some(kind) };
                    }
                    if open {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                modify.rect.left_bottom() + vec2(2.0, 1.0),
                                vec2(modify.rect.width() - 4.0, 1.0),
                            ),
                            0.0,
                            crate::theme::ACCENT,
                        );
                    }
                });
                ui.add_enabled_ui(present, |ui| {
                    if pixel_ui::text_button_sized(ui, "Export", third).clicked()
                        && let Some(image) = self.map_image(kind)
                    {
                        let file = format!("{}{}.png", self.name(), kind.suffix());
                        *action = Some(LumenAction::Export(file, image));
                    }
                });
            });
            bottom = bottom.max(column.min_rect().bottom());
        }
        ui.allocate_rect(
            Rect::from_min_max(pos2(left, top), pos2(left + WIDTH, bottom)),
            Sense::hover(),
        );
        if let Some(notice) = &self.notice {
            let notice = notice.clone();
            ui.horizontal(|ui| {
                hint(ui, &notice);
                if ui.small_button("×").clicked() {
                    self.notice = None;
                }
            });
        }
    }

    fn source_note<T>(&self, source: &Source<T>) -> &'static str {
        match source {
            Source::None => "none",
            Source::Generated => "generated",
            Source::Loaded(_) => "loaded",
        }
    }

    /// La miniatura de un mapa en su caja doble (vacía si no hay).
    fn thumbnail(&mut self, ui: &mut Ui, kind: MapKind, present: bool) {
        let (frame, _) = ui.allocate_exact_size(vec2(COLUMN, THUMB_BOX), Sense::hover());
        double_box(ui, frame);
        let well = frame.shrink(BOX_PAD);
        if !present {
            return;
        }
        let Some(image) = self.map_image(kind) else {
            return;
        };
        crate::panels::checker(&ui.painter_at(well), well);
        // La textura se rehace solo si el mapa cambió.
        let cached = self
            .thumbs
            .iter()
            .position(|(k, b, _)| *k == kind && *b == image);
        let texture = match cached {
            Some(i) => self.thumbs[i].2.clone(),
            None => {
                let texture = texture_of(ui, &format!("lumen_thumb_{kind:?}"), &image);
                self.thumbs.retain(|(k, _, _)| *k != kind);
                self.thumbs.push((kind, image.clone(), texture.clone()));
                texture
            }
        };
        let fit = (well.width() / image.width as f32).min(well.height() / image.height as f32);
        let k = if fit >= 1.0 { fit.floor() } else { fit };
        let size = vec2(image.width as f32, image.height as f32) * k;
        let rect = Rect::from_min_size((well.center() - size / 2.0).round(), size);
        let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        ui.painter_at(well)
            .image(texture.id(), rect, full, Color32::WHITE);
    }

    /// Generate: el mapa vuelve a salir del albedo.
    fn generate(&mut self, kind: MapKind) {
        match kind {
            MapKind::Albedo => {}
            MapKind::Normal => self.normal = Source::Generated,
            MapKind::Luminance => self.luminance = Source::Generated,
        }
    }

    /// Remove: sin ese mapa (sin albedo, el panel queda vacío).
    fn remove(&mut self, kind: MapKind) {
        match kind {
            MapKind::Albedo => {
                *self = Self {
                    normals: self.normals,
                    ..Self::default()
                }
            }
            MapKind::Normal => self.normal = Source::None,
            MapKind::Luminance => self.luminance = Source::None,
        }
    }

    /// Los ajustes del generador de `kind` (Modify), uno debajo del otro en la columna de la
    /// izquierda. Al moverlos, el mapa pasa a ser el generado.
    fn generator_settings(&mut self, ui: &mut Ui, kind: MapKind) {
        let d = NormalSettings::default();
        match kind {
            MapKind::Normal => {
                let before = self.normals;
                let s = &mut self.normals;
                let fmt = |v: f32| format!("{v:.2}");
                let strength = cs_render::normal::STRENGTH;
                let radius = cs_render::normal::SHAPE_RADIUS;
                let scale = cs_render::normal::DETAIL_SCALE;
                let px = |v: f32| format!("{v:.1} px");
                stacked_slider(
                    ui,
                    "Shape",
                    &mut s.shape_strength,
                    strength,
                    d.shape_strength,
                    fmt,
                );
                stacked_slider(
                    ui,
                    "Shape Radius",
                    &mut s.shape_radius,
                    radius,
                    d.shape_radius,
                    px,
                );
                stacked_slider(
                    ui,
                    "Softness",
                    &mut s.shape_softness,
                    strength,
                    d.shape_softness,
                    fmt,
                );
                stacked_slider(
                    ui,
                    "Detail",
                    &mut s.detail_strength,
                    strength,
                    d.detail_strength,
                    fmt,
                );
                stacked_slider(
                    ui,
                    "Detail Scale",
                    &mut s.detail_scale,
                    scale,
                    d.detail_scale,
                    fmt,
                );
                stacked_slider(
                    ui,
                    "Strength",
                    &mut s.normal_strength,
                    strength,
                    d.normal_strength,
                    fmt,
                );
                ui.horizontal(|ui| {
                    ui.label("Format");
                    let label = |f: NormalFormat| match f {
                        NormalFormat::OpenGl => "OpenGL",
                        NormalFormat::DirectX => "DirectX",
                    };
                    let height = crate::bar_fields::FIELD_HEIGHT;
                    widgets::combo_sized(ui, "lumen_format", label(s.format), None, height, |ui| {
                        for f in [NormalFormat::OpenGl, NormalFormat::DirectX] {
                            widgets::choice(ui, &mut s.format, f, label(f));
                        }
                    })
                    .on_hover_text("OpenGL: Godot, Unity, Bevy. DirectX: Unreal");
                });
                ui.horizontal(|ui| {
                    widgets::checkbox(ui, &mut s.invert_x, "Invert X");
                    widgets::checkbox(ui, &mut s.invert_y, "Invert Y");
                });
                if self.normals != before {
                    self.normal = Source::Generated;
                }
            }
            _ => {
                let before = self.threshold;
                let mut percent = self.threshold * 100.0;
                let fmt = |v: f32| format!("{v:.0}%");
                stacked_slider(ui, "Threshold", &mut percent, (0.0, 100.0), 85.0, fmt);
                self.threshold = percent / 100.0;
                hint(ui, "colors this bright glow");
                hint(ui, "on their own");
                if self.threshold != before {
                    self.luminance = Source::Generated;
                }
            }
        }
    }

    /// View y el color de la luz; si sigue al mouse; el fondo; cómo se dibuja la luz.
    fn view_controls(&mut self, ui: &mut Ui) {
        let height = crate::bar_fields::FIELD_HEIGHT;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.label("View");
            let view = &mut self.view;
            // Angosto: el color de la luz no llega a la vista previa.
            widgets::combo_sized(ui, "lumen_view", view.label(), Some(56.0), height, |ui| {
                for v in View::ALL {
                    widgets::choice(ui, view, v, v.label());
                }
            });
            ui.label("Light");
            color_button(ui, &mut self.light_color);
        });
        widgets::checkbox(ui, &mut self.follow_mouse, "Follow Mouse")
            .tip("The light follows the mouse over the preview; otherwise, drag it");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.label("BG");
            let mut bg = self.background.unwrap_or(Rgba::rgb(0, 0, 0));
            if color_button(ui, &mut bg) {
                self.background = Some(bg);
            }
            if pixel_ui::text_button_sized(ui, "Detect BG", 50.0).clicked()
                && let Some((_, image)) = &self.albedo
            {
                self.background =
                    cs_render::detect_background(&image.pixels, image.width, image.height);
            }
            widgets::checkbox(ui, &mut self.hide_background, "Hide BG")
                .tip("The background color counts as transparent");
        });
        ui.horizontal(|ui| {
            ui.label("Cursor");
            let cursor = &mut self.cursor;
            widgets::combo_sized(
                ui,
                "lumen_cursor",
                cursor.label(),
                Some(72.0),
                height,
                |ui| {
                    for c in LightCursor::ALL {
                        widgets::choice(ui, cursor, c, c.label());
                    }
                },
            );
        });
    }

    /// Los sliders de la luz, uno debajo del otro.
    fn sliders(&mut self, ui: &mut Ui) {
        let d = Params::default();
        let p = &mut self.params;
        let px = |v: f32| format!("{v:.0} px");
        let percent = |v: f32| format!("{v:.0}%");
        let one = |v: f32| format!("{v:.1}");
        stacked_slider(ui, "Height", &mut p.height, (1.0, 200.0), d.height, px);
        stacked_slider(ui, "Falloff", &mut p.falloff, (1.0, 400.0), d.falloff, px);
        stacked_slider(
            ui,
            "Brightness",
            &mut p.brightness,
            (0.0, 10.0),
            d.brightness,
            one,
        );
        stacked_slider(
            ui,
            "Ambient",
            &mut p.ambient,
            (0.0, 100.0),
            d.ambient,
            percent,
        );
        stacked_slider(
            ui,
            "Luminance",
            &mut p.luminance,
            (0.0, 100.0),
            d.luminance,
            percent,
        );
    }

    /// La vista previa: el damero y lo que muestra View, con la luz encima. Se arrastra la luz
    /// (o sigue al mouse), la rueda hace zoom y el botón derecho mueve la imagen.
    fn preview(&mut self, ui: &mut Ui, box_height: f32) {
        let width = ui.max_rect().width();
        let (frame, _) = ui.allocate_exact_size(vec2(width, box_height), Sense::hover());
        ui.painter()
            .extend(notched_outline(frame, box_color(ui), 1.0));
        let well = frame.shrink(2.0);
        crate::panels::checker(&ui.painter_at(well), well);
        let Some(image) = self.shown() else {
            ui.painter().text(
                well.center(),
                egui::Align2::CENTER_CENTER,
                "Browse an image or use the current layer.",
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            );
            ui.add_space(4.0);
            return;
        };
        if !matches!(&self.texture, Some((b, _)) if *b == image) {
            let texture = texture_of(ui, "lumen_preview", &image);
            self.texture = Some((image.clone(), texture));
        }
        let Some((_, texture)) = &self.texture else {
            return;
        };
        let (w, h) = (image.width as f32, image.height as f32);
        let fit = (well.width() / w).min(well.height() / h);
        let fit = if fit >= 1.0 { fit.floor() } else { fit };
        let k = self.zoom.unwrap_or(fit);
        let size = vec2(w, h) * k;
        let min = (well.center() + self.pan - size / 2.0).round();
        let rect = Rect::from_min_size(min, size);
        let response = ui.interact(well, ui.id().with("lumen_preview"), Sense::click_and_drag());
        let painter = ui.painter_at(well);
        let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), rect, full, Color32::WHITE);
        // La luz: arrastrándola, o siguiendo al mouse.
        let to_sprite = |p: egui::Pos2| ((p.x - rect.min.x) / k, (p.y - rect.min.y) / k);
        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        if let Some(p) = pointer {
            let dragging = response.dragged_by(egui::PointerButton::Primary)
                || response.clicked_by(egui::PointerButton::Primary);
            if dragging || (self.follow_mouse && response.hovered()) {
                self.light_at = Some(to_sprite(p));
            }
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            self.pan += response.drag_delta();
        }
        if response.hovered() {
            let scroll: f32 = ui.input(|i| {
                i.raw
                    .events
                    .iter()
                    .map(|e| match e {
                        egui::Event::MouseWheel { delta, .. } => delta.y,
                        _ => 0.0,
                    })
                    .sum()
            });
            if scroll != 0.0 {
                let step = if scroll > 0.0 { 1.0 } else { -1.0 };
                let next = (k.floor() + step).clamp(1.0, MAX_ZOOM);
                self.zoom = Some(next);
            }
        }
        let light = self.light(image.width, image.height);
        let at = rect.min + vec2(light.x, light.y) * k;
        self.light_gizmo(&painter, at, k, &light);
        // Debajo: el tamaño, la vista y el zoom.
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(format!("{}×{}", image.width, image.height));
            hint(ui, "drag: light · wheel: zoom · right drag: pan");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if pixel_ui::text_button_sized(ui, "Fit", 26.0).clicked() {
                    self.zoom = None;
                    self.pan = egui::Vec2::ZERO;
                }
                let (minus, plus) = zoom_buttons(ui);
                if plus {
                    self.zoom = Some((k.floor() + 1.0).min(MAX_ZOOM));
                }
                if minus {
                    self.zoom = Some((k.floor() - 1.0).max(1.0));
                }
                ui.label(format!("{:.0}%", k * 100.0));
            });
        });
    }

    /// La luz sobre la vista previa: el punto con su color y, en Advanced, el aro del falloff
    /// (punteado) y la línea de la altura.
    fn light_gizmo(&self, painter: &egui::Painter, at: egui::Pos2, k: f32, light: &Light) {
        if self.cursor == LightCursor::Hidden {
            return;
        }
        let color = crate::panels::to_color32(Rgba {
            a: 255,
            ..light.color
        });
        let dark = Color32::from_rgb(34, 32, 52);
        if self.cursor == LightCursor::Advanced {
            let radius = light.falloff * k;
            let steps = ((radius * 0.6) as usize).clamp(24, 160);
            let ring: Vec<egui::Pos2> = (0..=steps)
                .map(|i| {
                    let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                    at + vec2(a.cos(), a.sin()) * radius
                })
                .collect();
            painter.extend(egui::Shape::dashed_line(
                &ring,
                egui::Stroke::new(1.0, color),
                4.0,
                4.0,
            ));
            let tip = at + vec2(0.0, (light.height * k).min(radius.max(8.0)));
            painter.line_segment([at, tip], egui::Stroke::new(1.0, color));
        }
        // El foquito del skin, centrado en la luz; sin skin, un punto del color.
        let skin = pixel_ui::frame_skin(painter.ctx());
        match skin
            .as_ref()
            .and_then(|s| s.piece(LIGHT_ICON, WidgetState::Normal))
        {
            Some(bulb) => {
                let size = vec2(bulb.size[0] as f32, bulb.size[1] as f32);
                let rect = Rect::from_min_size((at - size / 2.0).round(), size);
                let ppp = painter.ctx().pixels_per_point();
                painter.extend(pixel_ui::piece_shapes(bulb, rect, ppp));
            }
            None => {
                painter.circle(at, 4.0, color, egui::Stroke::new(1.0, dark));
            }
        }
    }
}

/// El foquito que marca la luz en la vista previa (el ícono de Lumen en el dock).
const LIGHT_ICON: &str = "icon.dock.lumen";

/// − y + del zoom, juntos (la pieza `lumen.zoom` del skin; sin ella, dos botones). Devuelve si
/// se tocó cada uno. Va en un layout de derecha a izquierda.
fn zoom_buttons(ui: &mut Ui) -> (bool, bool) {
    let skin = pixel_ui::frame_skin(ui.ctx());
    let Some(piece) = skin
        .as_ref()
        .and_then(|s| s.piece("lumen.zoom", WidgetState::Normal))
    else {
        let plus = pixel_ui::text_button_sized(ui, "+", 18.0).clicked();
        let minus = pixel_ui::text_button_sized(ui, "-", 18.0).clicked();
        return (minus, plus);
    };
    let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), size);
    let ppp = ui.ctx().pixels_per_point();
    ui.painter()
        .extend(pixel_ui::piece_shapes(piece, rect, ppp));
    // Cada mitad, hasta la línea del medio (sin la sombra de abajo).
    let half = (size.x / 2.0).floor();
    let hit = |from: f32, to: f32| {
        Rect::from_min_max(
            pos2(rect.left() + from, rect.top()),
            pos2(rect.left() + to, rect.bottom() - 2.0),
        )
    };
    let minus = ui
        .interact(
            hit(0.0, half),
            ui.id().with("lumen_zoom_out"),
            Sense::click(),
        )
        .tip("Zoom out");
    let plus = ui
        .interact(
            hit(half, size.x),
            ui.id().with("lumen_zoom_in"),
            Sense::click(),
        )
        .tip("Zoom in");
    (minus.clicked(), plus.clicked())
}

/// El alto de un slider: el título con su valor y la barra.
const SLIDER_HEIGHT: f32 = 26.0;

/// Un slider del ancho de la columna, debajo de lo anterior.
fn stacked_slider(
    ui: &mut Ui,
    title: &str,
    value: &mut f32,
    range: (f32, f32),
    default: f32,
    format: impl Fn(f32) -> String,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(COLUMN, SLIDER_HEIGHT), Sense::hover());
    slider(ui, rect, title, value, range, default, format);
}

/// Un slider en `rect`: el título, ↺ (al default) y el valor a la derecha; debajo, la barra.
fn slider(
    ui: &mut Ui,
    rect: Rect,
    title: &str,
    value: &mut f32,
    (lo, hi): (f32, f32),
    default: f32,
    format: impl Fn(f32) -> String,
) {
    let mut cell = child(ui, rect, egui::Layout::top_down(egui::Align::Min));
    cell.spacing_mut().item_spacing.y = 1.0;
    // La barra, pegada a su título.
    cell.spacing_mut().interact_size.y = 12.0;
    cell.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.label(title);
        let reset = ui
            .add_enabled(
                *value != default,
                egui::Button::new("↺").frame(false).small(),
            )
            .tip("Back to the default");
        if reset.clicked() {
            *value = default;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format(*value));
        });
    });
    let skin = pixel_ui::frame_skin(ui.ctx());
    let pieces = skin.as_ref().and_then(|s| {
        Some((
            s.piece("slider.track", WidgetState::Normal)?,
            s.piece("slider.thumb", WidgetState::Normal)?,
        ))
    });
    let Some((track, thumb)) = pieces else {
        cell.spacing_mut().slider_width = rect.width() - 2.0;
        cell.add(egui::Slider::new(value, lo..=hi).show_value(false));
        return;
    };
    // La barra abajo de `rect`: el riel (centrado en el alto de la perilla) y la perilla, que
    // va de punta a punta del riel.
    let knob = vec2(thumb.size[0] as f32, thumb.size[1] as f32);
    let bar = Rect::from_min_max(
        pos2(rect.left(), rect.bottom() - knob.y).round(),
        rect.right_bottom().round(),
    );
    let rail_h = track.size[1] as f32;
    let rail = Rect::from_min_size(
        pos2(bar.left(), bar.top() + ((knob.y - rail_h) / 2.0).floor()),
        vec2(bar.width(), rail_h),
    );
    let response = ui.interact(
        bar,
        ui.id().with(("lumen_slider", title)),
        Sense::click_and_drag(),
    );
    let travel = (bar.width() - knob.x).max(1.0);
    if let Some(p) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked() || response.is_pointer_button_down_on())
    {
        let t = ((p.x - bar.left() - knob.x / 2.0) / travel).clamp(0.0, 1.0);
        *value = lo + t * (hi - lo);
    }
    let t = if hi > lo {
        ((*value - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let ppp = ui.ctx().pixels_per_point();
    let painter = ui.painter();
    painter.extend(pixel_ui::piece_shapes(track, rail, ppp));
    let knob_rect = Rect::from_min_size(pos2((bar.left() + t * travel).round(), bar.top()), knob);
    painter.extend(pixel_ui::piece_shapes(thumb, knob_rect, ppp));
}

/// Desarrollo: con este dato en el contexto, el próximo [`color_button`] se abre.
pub(crate) const DEMO_PICKER: &str = "lumen_demo_picker";

/// El ancho del selector de color que abre [`color_button`].
const PICKER_WIDTH: f32 = 160.0;

/// Un botón con la muestra de `color` que abre el selector. Devuelve si cambió.
fn color_button(ui: &mut Ui, color: &mut Rgba) -> bool {
    let skin = pixel_ui::frame_skin(ui.ctx());
    let frame = skin
        .as_ref()
        .and_then(|s| Some((s, s.piece("swatch.frame", WidgetState::Normal)?)));
    match frame {
        Some((skin, frame)) => pixel_color_button(ui, skin, frame, color),
        None => egui_color_button(ui, color),
    }
}

/// La muestra en su marco del skin (el color, con la fila de abajo más oscura); un clic abre
/// el selector de color de la app debajo.
fn pixel_color_button(
    ui: &mut Ui,
    skin: &crate::skin::Skin,
    frame: &crate::skin::Piece,
    color: &mut Rgba,
) -> bool {
    let size = vec2(frame.size[0] as f32, frame.size[1] as f32);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let rect = Rect::from_min_size(rect.min.round(), size);
    let fill = Color32::from_rgb(color.r, color.g, color.b);
    let shade = |v: u8| (v as f32 * 0.62).round() as u8;
    let dark = Color32::from_rgb(shade(color.r), shade(color.g), shade(color.b));
    // Adentro del marco: 1 px de cada lado; abajo, la fila oscura, la línea y el brillo.
    let inner = Rect::from_min_size(rect.min + vec2(1.0, 1.0), vec2(size.x - 2.0, size.y - 4.0));
    let painter = ui.painter();
    painter.rect_filled(inner, 0.0, fill);
    painter.rect_filled(
        Rect::from_min_size(inner.left_bottom(), vec2(inner.width(), 1.0)),
        0.0,
        dark,
    );
    let ppp = ui.ctx().pixels_per_point();
    painter.extend(pixel_ui::piece_shapes(frame, rect, ppp));
    // Desarrollo: el de la luz (el primero) abierto, para capturas.
    let demo = egui::Id::new(DEMO_PICKER);
    let first = ui.ctx().data(|d| d.get_temp::<egui::Id>(demo));
    if first == Some(egui::Id::NULL) {
        ui.ctx().data_mut(|d| d.insert_temp(demo, response.id));
    }
    if first.is_some_and(|id| id == egui::Id::NULL || id == response.id) {
        egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&response));
    }
    let look = crate::menus::MenuLook::new(skin, rect.top());
    let mut changed = false;
    crate::menus::dropdown_panel(&look, &response, rect, |ui| {
        ui.set_width(PICKER_WIDTH);
        let id = response.id.with("picker");
        let next = crate::color_panel::picker(ui, skin, id, *color);
        let next = Rgba { a: 255, ..next };
        if next != *color {
            *color = next;
            changed = true;
        }
    });
    changed
}

/// Sin Pixel UI: el botón de color de egui.
fn egui_color_button(ui: &mut Ui, color: &mut Rgba) -> bool {
    let mut c = Color32::from_rgb(color.r, color.g, color.b);
    // Una muestra chica, cuadrada.
    ui.spacing_mut().interact_size.x = 16.0;
    let changed =
        egui::color_picker::color_edit_button_srgba(ui, &mut c, egui::color_picker::Alpha::Opaque)
            .changed();
    if changed {
        *color = Rgba::rgb(c.r(), c.g(), c.b());
    }
    changed
}

/// La textura de `image`, con vecino más cercano.
fn texture_of(ui: &Ui, name: &str, image: &Bitmap) -> TextureHandle {
    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|c| [c.r, c.g, c.b, c.a])
        .collect();
    let color =
        ColorImage::from_rgba_unmultiplied([image.width as usize, image.height as usize], &bytes);
    ui.ctx().load_texture(name, color, TextureOptions::NEAREST)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un cuadrado de 6×6 con un píxel claro en el centro, en un fondo verde de 10×10.
    fn sprite() -> Bitmap {
        let bg = Rgba::rgb(0, 200, 0);
        let pixels = (0..100)
            .map(|i| {
                let (x, y) = (i % 10, i / 10);
                if (x, y) == (5, 5) {
                    Rgba::rgb(255, 250, 230)
                } else if (2..8).contains(&x) && (2..8).contains(&y) {
                    Rgba::rgb(120, 60, 40)
                } else {
                    bg
                }
            })
            .collect();
        Bitmap {
            width: 10,
            height: 10,
            pixels,
        }
    }

    #[test]
    fn loading_detects_the_background_and_generates_the_normals() {
        let mut panel = LumenPanel::default();
        panel.load("box".into(), sprite());
        assert_eq!(panel.background, Some(Rgba::rgb(0, 200, 0)));
        assert_eq!(panel.normal, Source::Generated);
        assert_eq!(panel.luminance, Source::None);
        assert!(panel.mask().is_none());
        // Con el fondo escondido, el normal map es transparente afuera.
        panel.hide_background = true;
        let normal = panel.normal_pixels().unwrap();
        assert_eq!(normal[0].a, 0);
        assert_eq!(normal[55].a, 255);
    }

    #[test]
    fn moving_the_light_does_not_regenerate_the_normals() {
        let mut panel = LumenPanel::default();
        panel.load("box".into(), sprite());
        panel.shown().unwrap();
        let first = panel.normal_cache.as_ref().unwrap().1.as_ptr();
        panel.light_at = Some((0.0, 0.0));
        panel.params.brightness = 5.0;
        panel.shown().unwrap();
        assert_eq!(panel.normal_cache.as_ref().unwrap().1.as_ptr(), first);
        panel.normals.normal_strength = 3.0;
        panel.shown().unwrap();
        assert_ne!(panel.normal_cache.as_ref().unwrap().1.as_ptr(), first);
    }

    #[test]
    fn a_map_of_another_size_is_rejected() {
        let mut panel = LumenPanel::default();
        let small = Bitmap {
            width: 2,
            height: 2,
            pixels: vec![Rgba::rgb(128, 128, 255); 4],
        };
        assert!(
            panel
                .load_map(MapKind::Normal, "n".into(), small.clone())
                .is_err()
        );
        panel.load("box".into(), sprite());
        assert!(panel.load_map(MapKind::Normal, "n".into(), small).is_err());
        assert_eq!(panel.normal, Source::Generated);
        let mask = Bitmap {
            width: 10,
            height: 10,
            pixels: vec![Rgba::rgb(255, 255, 255); 100],
        };
        panel
            .load_map(MapKind::Luminance, "e".into(), mask)
            .unwrap();
        assert_eq!(panel.mask().unwrap(), vec![255; 100]);
    }

    #[test]
    fn the_generated_mask_takes_the_bright_pixel() {
        let mut panel = LumenPanel::default();
        panel.load("box".into(), sprite());
        panel.generate(MapKind::Luminance);
        panel.hide_background = true;
        let mask = panel.mask().unwrap();
        assert_eq!(mask[55], 255);
        assert_eq!(mask.iter().filter(|&&v| v > 0).count(), 1);
    }

    #[test]
    fn each_view_shows_its_map() {
        let mut panel = LumenPanel::default();
        panel.load("box".into(), sprite());
        panel.view = View::Albedo;
        assert_eq!(panel.shown().unwrap(), sprite());
        panel.view = View::Normal;
        let normal = panel.shown().unwrap();
        // El píxel claro del centro sobresale: hay relieve.
        assert_ne!(normal.pixels[55], Rgba::new(128, 128, 255, 255));
        panel.remove(MapKind::Normal);
        let flat = panel.shown().unwrap();
        assert!(
            flat.pixels
                .iter()
                .all(|p| (p.r, p.g, p.b) == (128, 128, 255))
        );
        panel.view = View::Final;
        panel.params.ambient = 100.0;
        panel.params.brightness = 0.0;
        assert_eq!(panel.shown().unwrap(), sprite(), "only ambient = albedo");
    }
}
