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

use crate::forms::{self, TuiBuilderLogic as _};
use crate::panel_kit::{
    box_color, child, double_box, hint, hint_galley, hint_wrapped, label_with_hint,
    notched_outline, row_bottom,
};
use crate::pixel_ui::Tip as _;
use crate::skin::WidgetState;
use crate::{kit, pixel_ui, widgets};

/// El panel, desde la esquina de su contenido: el ancho del diseño (el de Groom, el del panel
/// flotante; adentro todo sale del ancho disponible) y el alto más chico.
pub const REFERENCE_WIDTH: f32 = 434.0;
/// Lo más angosto en que se ve bien (medido): los tres botones del pie en una fila (New Sprite,
/// Add as Layer y Add as Artboard miden 216 con su aire). Los mapas, los controles y la vista
/// previa, uno debajo de otro.
pub const MIN_WIDTH: f32 = 216.0;
/// Con Pixel UI, los sliders llevan el encabezado del UI kit (30 de alto cada uno) y abrir y
/// quitar van en la caja de cada mapa (sin su fila de botones).
const MIN_HEIGHT: f32 = 391.0;
/// El encabezado: el logo (sobre el margen del panel) y, a la derecha, el crédito del algoritmo
/// (con `HEADER_GAP` entre ellos; si no cabe, debajo del logo). La línea grabada va
/// `HEADER_LINE` debajo y los mapas empiezan `BODY_AIR` debajo.
const LOGO_AT: egui::Vec2 = vec2(-3.0, -1.0);
const HEADER_HEIGHT: f32 = 19.0;
const HEADER_GAP: f32 = 4.0;
const HEADER_LINE: f32 = 2.0;
const BODY_AIR: f32 = 6.0;
/// El crédito del algoritmo de las normales.
const CREDIT: &str = "normals: sprite fusion";
/// Las tres columnas de los mapas: su ancho en el diseño (también el de los controles, debajo de
/// Albedo) y la separación. Se achican hasta `MAP_MIN` (medido: Generate con su ancho justo, 50,
/// y Modify y Export de 41, el justo de Export, con 2 entre ellos) y crecen parejas. Más angosto
/// que `MAPS_ACROSS`, van una debajo de otra, `MAPS_STACK_GAP` entre ellas.
const COLUMN: f32 = 140.0;
const MAP_MIN: f32 = 136.0;
const COLUMN_GAP: f32 = 7.0;
const MAPS_ACROSS: f32 = 3.0 * MAP_MIN + 2.0 * COLUMN_GAP;
const MAPS_STACK_GAP: f32 = 6.0;
/// La caja de la miniatura de cada mapa.
const THUMB_BOX: f32 = 40.0;
/// Entre los botones de una fila.
const BUTTON_GAP: f32 = 2.0;
/// Debajo de los mapas (y del aviso): el aviso, `NOTICE_AIR` debajo; la línea grabada,
/// `CONTROLS_LINE` debajo, y lo de abajo, `CONTROLS_AIR` debajo de ella.
const NOTICE_AIR: f32 = 3.0;
const CONTROLS_LINE: f32 = 8.0;
const CONTROLS_AIR: f32 = 6.0;
/// Lo de abajo: a la izquierda los controles (de la columna de Albedo, desde lo más ancho que no
/// se parte, medido: la fila del fondo, 129) y a la derecha la vista previa (de las otras dos
/// columnas, desde lo que entra la frase de la caja vacía, 184), que crece el doble para que los
/// controles sigan del ancho de Albedo. Más angosto que `SIDE_BY_SIDE`, la vista previa va
/// `STACK_GAP` debajo de los controles, de `STACKED_PREVIEW` de alto por lo menos.
const LEFT_MIN: f32 = 129.0;
const PREVIEW: f32 = 2.0 * COLUMN + COLUMN_GAP;
const PREVIEW_MIN: f32 = 184.0;
const SIDE_BY_SIDE: f32 = LEFT_MIN + COLUMN_GAP + PREVIEW_MIN;
const STACK_GAP: f32 = 8.0;
const STACKED_PREVIEW: f32 = 120.0;

/// Los anchos de los botones de una columna de `width`: cada uno de `n` parejos.
fn button_width(width: f32, n: usize) -> f32 {
    ((width - (n - 1) as f32 * BUTTON_GAP) / n as f32).floor()
}
/// Del borde de una caja a lo de adentro: las dos líneas y 2 de aire.
const BOX_PAD: f32 = 5.0;
/// Debajo de la vista previa: el tamaño y el zoom.
const BELOW_PREVIEW: f32 = 26.0;
const MIN_PREVIEW: f32 = 40.0;
/// Cómo se usa la vista previa (al lado del tamaño si cabe; si no, en su tooltip).
const PREVIEW_HELP: &str = "drag: light · wheel: zoom · right drag: pan";
/// El botón de que entre la imagen y, entre él y lo de su lado, aire.
const FIT_WIDTH: f32 = 26.0;
const ZOOM_GAP: f32 = 2.0;
/// Abajo del todo, los botones (enteros dentro del contenido: lo de afuera lo recorta el
/// scroll) y, `FOOTER_AIR` arriba de ellos, la línea grabada; la vista previa termina
/// `PREVIEW_AIR` arriba de la línea.
const FOOTER_BUTTONS: f32 = pixel_ui::CONTROL_HEIGHT;
const FOOTER_AIR: f32 = 5.0;
const PREVIEW_AIR: f32 = 3.0;
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

/// El encabezado en `tui`: el logo (sin Pixel UI, el título) y, a la derecha, el crédito del
/// algoritmo de las normales (debajo del logo si no cabe). Devuelve su rect.
fn header(tui: &mut forms::Tui, logo: Option<&crate::skin::Piece>) -> Rect {
    tui.style(forms::row(0.0)).add(|tui| {
        let natural = forms::air_right(HEADER_GAP, forms::natural());
        let title = tui.style(natural).ui(|ui| {
            match logo {
                Some(piece) => {
                    let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
                    let (slot, _) = ui.allocate_exact_size(
                        vec2(size.x + LOGO_AT.x, HEADER_HEIGHT),
                        Sense::hover(),
                    );
                    let rect = Rect::from_min_size(slot.min + LOGO_AT, size);
                    let ppp = ui.ctx().pixels_per_point();
                    // Sube y sale sobre el margen del panel, por encima del recorte del scroll:
                    // con él se perdían su fila de arriba y sus columnas de la izquierda. El
                    // recorte crece solo eso (1 arriba, 3 a la izquierda): con el contenido
                    // desplazado, el logo se recorta como lo demás y no se pinta sobre el marco.
                    // (`with_clip_rect` intersecta con el recorte de ahora; `set_clip_rect` lo
                    // cambia y deja lo demás del painter: invisible en la pasada que mide,
                    // apagado con un modal.)
                    let clip = ui.clip_rect();
                    let mut painter = ui.painter().clone();
                    painter.set_clip_rect(
                        clip.with_min_x(clip.min.x + LOGO_AT.x)
                            .with_min_y(clip.min.y + LOGO_AT.y),
                    );
                    painter.extend(pixel_ui::piece_shapes(piece, rect, ppp));
                }
                None => {
                    let galley = ui.painter().layout_no_wrap(
                        "Lumen".into(),
                        egui::TextStyle::Heading.resolve(ui.style()),
                        ui.visuals().strong_text_color(),
                    );
                    let size = vec2(galley.size().x.ceil(), HEADER_HEIGHT);
                    let (slot, _) = ui.allocate_exact_size(size, Sense::hover());
                    ui.painter()
                        .galley(slot.min, galley, ui.visuals().strong_text_color());
                }
            }
            ui.max_rect()
        });
        let credit = tui.style(forms::push_right(forms::natural())).ui(|ui| {
            // A la derecha y centrado a lo alto en la fila del encabezado.
            let size = vec2(hint_galley(ui, CREDIT).size().x.ceil(), HEADER_HEIGHT);
            let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
            ui.allocate_ui_with_layout(size, right_to_left, |ui| hint(ui, CREDIT));
            ui.max_rect()
        });
        title.union(credit)
    })
}

/// Modify en la columna del diseño (la de 140): el ancho del mockup.
const MODIFY: f32 = 45.0;

/// Los anchos de Generate, Modify y Export en una columna de `width`. Generate, con su ancho justo
/// (o un tercio, si cabe); Modify, la mitad de lo que queda sin bajar de `MODIFY`, y Export, el
/// resto, sin bajar de su ancho justo (en la columna del diseño: 50, 45 y 41, como el mockup;
/// en la más angosta, Modify cede lo que le falta a Export).
fn map_button_widths(ui: &Ui, width: f32) -> (f32, f32, f32) {
    let third = button_width(width, 3);
    let (generate, export_min) = match pixel_ui::frame_skin(ui.ctx()) {
        Some(_) => (
            kit::button::width(ui, "Generate").max(third),
            kit::button::width(ui, "Export"),
        ),
        None => (third, 0.0),
    };
    let rest = width - generate - 2.0 * BUTTON_GAP;
    let modify = (rest / 2.0).floor().max(MODIFY).min(rest - export_min);
    (generate, modify, rest - modify)
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

    /// El panel, del ancho de `ui`: arriba el logo; los tres mapas; a la izquierda (debajo de
    /// Albedo) la vista y los sliders, o los ajustes del generador que se modifica, y a la derecha
    /// la vista previa (hasta el pie); abajo, lo que se hace con lo que se ve. Si los mapas no
    /// caben lado a lado (`MAPS_ACROSS`), van uno debajo de otro; si los controles y la vista
    /// previa no caben (`SIDE_BY_SIDE`), la vista previa va debajo.
    pub fn ui(&mut self, ui: &mut Ui) -> Option<LumenAction> {
        let mut action = None;
        let origin = ui.cursor().min;
        let width = ui.available_width().floor();
        let available = ui.available_height();
        let height = if available.is_finite() {
            available.max(MIN_HEIGHT)
        } else {
            MIN_HEIGHT
        };
        let skin = pixel_ui::frame_skin(ui.ctx());
        let logo = skin.as_ref().and_then(|s| {
            s.piece("lumen.logo", crate::skin::WidgetState::Normal)
                .cloned()
        });
        // Las líneas grabadas van detrás de todo: su lugar, antes; se dibujan al final, cuando se
        // sabe dónde quedó cada cosa.
        let lines = ui.painter().add(egui::Shape::Noop);
        let mut top = Rect::NOTHING;
        let mut controls = Rect::NOTHING;
        let mut footer = Rect::NOTHING;
        forms::form(ui, "lumen", 0.0, |tui| {
            let panel = forms::min_height(height, forms::column(0.0));
            tui.style(panel).add(|tui| {
                top = header(tui, logo.as_ref());
                self.maps_ui(tui, width >= MAPS_ACROSS, &mut action);
                if let Some(notice) = self.notice.clone() {
                    let row = forms::air_above(NOTICE_AIR, forms::Style::default());
                    tui.style(row).ui(|ui| self.notice_ui(ui, &notice));
                }
                controls = self.below_maps(tui, width >= SIDE_BY_SIDE);
                // Abajo, a la derecha: lo que muestra View, a un sprite, una capa o un artboard.
                let buttons = forms::min_height(FOOTER_BUTTONS, forms::natural());
                let row = forms::air_above(FOOTER_AIR, forms::row_nowrap(0.0));
                footer = tui.style(row).add(|tui| {
                    tui.style(forms::push_right(buttons)).ui(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            self.footer_buttons(ui, &mut action);
                        });
                        ui.max_rect()
                    })
                });
            });
        });
        // Las líneas grabadas: debajo del encabezado, entre los mapas y lo de abajo y arriba de
        // los botones de abajo. De borde a borde del panel.
        if let Some(skin) = &skin {
            let colors = skin.colors();
            let ppp = ui.ctx().pixels_per_point();
            let margin = f32::from(crate::dock::FLYOUT_MARGIN) - 1.0;
            let across = egui::Rangef::new(origin.x - margin, origin.x + width + margin);
            let shapes = [
                top.bottom() + HEADER_LINE,
                controls.top() - CONTROLS_AIR,
                footer.top() - FOOTER_AIR,
            ]
            .map(|y| pixel_ui::etched_hline(across, y, colors, ppp));
            ui.painter().set(lines, egui::Shape::Vec(shapes.into()));
        }
        action
    }

    /// New Sprite, Add as Layer y Add as Artboard con lo que muestra View (apagados sin albedo).
    fn footer_buttons(&mut self, ui: &mut Ui, action: &mut Option<LumenAction>) {
        ui.add_enabled_ui(self.albedo.is_some(), |ui| {
            let name = format!("{} ({})", self.name(), self.view.label());
            if pixel_ui::text_button(ui, "New Sprite").clicked() {
                *action = self
                    .shown()
                    .map(|b| LumenAction::NewSprite(name.clone(), b));
            }
            if pixel_ui::text_button(ui, "Add as Layer").clicked() {
                *action = self.shown().map(|b| LumenAction::AddLayer(name.clone(), b));
            }
            if pixel_ui::text_button(ui, "Add as Artboard").clicked() {
                *action = self
                    .shown()
                    .map(|b| LumenAction::AddArtboard(name.clone(), b));
            }
        });
    }

    /// Los tres mapas en `tui`, `BODY_AIR` debajo del encabezado: lado a lado (`across`), del
    /// mismo ancho, o uno debajo de otro.
    fn maps_ui(&mut self, tui: &mut forms::Tui, across: bool, action: &mut Option<LumenAction>) {
        let kinds = [MapKind::Albedo, MapKind::Normal, MapKind::Luminance];
        let column = forms::grow_auto(MAP_MIN, COLUMN);
        if across {
            let row = forms::air_above(BODY_AIR, forms::row_nowrap(COLUMN_GAP));
            tui.style(row).add(|tui| {
                for kind in kinds {
                    tui.style(column.clone())
                        .ui(|ui| self.map(ui, kind, action));
                }
            });
            return;
        }
        for (i, kind) in kinds.into_iter().enumerate() {
            let air = if i == 0 { BODY_AIR } else { MAPS_STACK_GAP };
            let row = forms::air_above(air, forms::row_nowrap(0.0));
            tui.style(row).add(|tui| {
                tui.style(column.clone())
                    .ui(|ui| self.map(ui, kind, action));
            });
        }
    }

    /// Un aviso (un mapa que no se pudo cargar) y la ✕ para quitarlo; si no cabe, en varias
    /// líneas.
    fn notice_ui(&mut self, ui: &mut Ui, notice: &str) {
        ui.horizontal(|ui| {
            let skin = crate::pixel_ui::frame_skin(ui.ctx());
            let close = pixel_ui::ICON_BUTTON.x + ui.spacing().item_spacing.x;
            hint_wrapped(ui, notice, ui.available_width() - close);
            if crate::pixel_ui::icon_button(ui, skin.as_deref(), "icon.tab_close", "×", None)
                .clicked()
            {
                self.notice = None;
            }
        });
    }

    /// Lo de debajo de los mapas en `tui`: a la izquierda (debajo de Albedo) la vista y la luz,
    /// o con Modify los ajustes del generador; a la derecha, la vista previa con el zoom abajo,
    /// hasta el pie. Si no caben lado a lado, la vista previa va debajo. Devuelve el rect de los
    /// controles.
    fn below_maps(&mut self, tui: &mut forms::Tui, side_by_side: bool) -> Rect {
        let air = CONTROLS_LINE + CONTROLS_AIR;
        let left = forms::grow_auto(LEFT_MIN, COLUMN);
        if side_by_side {
            // Del alto de los controles por lo menos (la vista previa no se mide).
            let row = forms::fill_height_auto(forms::row_nowrap(COLUMN_GAP));
            return tui.style(forms::air_above(air, row)).add(|tui| {
                let rect = tui.style(left).ui(|ui| self.controls(ui));
                // Crece el doble que los controles: siguen del ancho de la columna de Albedo.
                let right = forms::grow_auto(PREVIEW_MIN, PREVIEW);
                let right = forms::full_height(forms::weight(2.0, right));
                // Los controles pueden llegar a su aire de abajo; la vista previa, no.
                self.preview_ui(tui, forms::air_below(PREVIEW_AIR, right));
                rect
            });
        }
        let row = forms::air_above(air, forms::row_nowrap(0.0));
        let rect = tui
            .style(row)
            .add(|tui| tui.style(left).ui(|ui| self.controls(ui)));
        let preview = forms::fill_height(STACKED_PREVIEW + BELOW_PREVIEW, forms::Style::default());
        let preview = forms::air_below(PREVIEW_AIR, preview);
        self.preview_ui(tui, forms::air_above(STACK_GAP, preview));
        rect
    }

    /// Los controles de la columna de la izquierda en `ui`: la vista y los sliders de la luz o,
    /// con Modify, los ajustes del generador. Devuelve su rect.
    fn controls(&mut self, ui: &mut Ui) -> Rect {
        ui.spacing_mut().item_spacing.y = 3.0;
        match self.modify {
            Some(kind) => self.generator_settings(ui, kind),
            None => {
                self.view_controls(ui);
                ui.add_space(2.0);
                self.sliders(ui);
            }
        }
        ui.max_rect()
    }

    /// La vista previa en una hoja de `style` de `tui` (su caja, lo que la hoja deja sin lo de
    /// abajo).
    fn preview_ui(&mut self, tui: &mut forms::Tui, style: forms::Style) {
        forms::canvas(tui, style, |ui| {
            let box_height = (ui.max_rect().height() - BELOW_PREVIEW).max(MIN_PREVIEW);
            self.preview(ui, box_height);
        });
    }

    /// Un mapa en `ui` (una columna): título y tamaño, miniatura y botones.
    fn map(&mut self, ui: &mut Ui, kind: MapKind, action: &mut Option<LumenAction>) {
        ui.spacing_mut().item_spacing.y = 3.0;
        let width = ui.max_rect().width();
        let size = self
            .albedo
            .as_ref()
            .map(|(_, b)| format!("{}×{}", b.width, b.height));
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
        label_with_hint(ui, kind.label(), &note);
        // Con Pixel UI, abrir y quitar son íconos en la caja de la miniatura (así no hace falta
        // el scroll); sin él, la fila de botones de siempre.
        let icons = self.thumbnail(ui, kind, present);
        if icons.browse {
            *action = Some(LumenAction::Browse(kind));
        }
        if icons.remove && present {
            self.remove(kind);
        }
        if !icons.shown {
            row_bottom(ui, |ui| {
                ui.spacing_mut().item_spacing.x = BUTTON_GAP;
                let half = button_width(width, 2);
                if pixel_ui::text_button_sized(ui, "Browse", half).clicked() {
                    *action = Some(LumenAction::Browse(kind));
                }
                let remove = ui.add_enabled_ui(present, |ui| {
                    pixel_ui::text_button_sized(ui, "Remove", half)
                });
                if remove.inner.clicked() && present {
                    self.remove(kind);
                }
            });
        }
        // Los botones (de 17) abajo de su fila, en puntos enteros.
        row_bottom(ui, |ui| {
            ui.spacing_mut().item_spacing.x = BUTTON_GAP;
            if kind == MapKind::Albedo {
                if pixel_ui::text_button_sized(ui, "Use Layer", width).clicked() {
                    *action = Some(LumenAction::UseLayer);
                }
                return;
            }
            let (generate, modify_w, export_w) = map_button_widths(ui, width);
            ui.add_enabled_ui(self.albedo.is_some(), |ui| {
                if pixel_ui::text_button_sized(ui, "Generate", generate).clicked() {
                    self.generate(kind);
                }
                let open = self.modify == Some(kind);
                let modify = pixel_ui::text_button_sized(ui, "Modify", modify_w)
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
                if pixel_ui::text_button_sized(ui, "Export", export_w).clicked()
                    && let Some(image) = self.map_image(kind)
                {
                    let file = format!("{}{}.png", self.name(), kind.suffix());
                    *action = Some(LumenAction::Export(file, image));
                }
            });
        });
    }

    fn source_note<T>(&self, source: &Source<T>) -> &'static str {
        match source {
            Source::None => "none",
            Source::Generated => "generated",
            Source::Loaded(_) => "loaded",
        }
    }

    /// La miniatura de un mapa en su caja doble (vacía si no hay) y, con Pixel UI, abajo a la
    /// derecha los íconos de abrir (la carpeta) y quitar (la ✕, apagada si no hay mapa).
    fn thumbnail(&mut self, ui: &mut Ui, kind: MapKind, present: bool) -> MapIcons {
        let (frame, _) =
            ui.allocate_exact_size(vec2(ui.max_rect().width(), THUMB_BOX), Sense::hover());
        double_box(ui, frame);
        let well = frame.shrink(BOX_PAD);
        if present {
            self.thumbnail_image(ui, kind, well);
        }
        map_icons(ui, kind, frame, present)
    }

    /// La imagen del mapa `kind`, ajustada a `well` sobre el damero.
    fn thumbnail_image(&mut self, ui: &mut Ui, kind: MapKind, well: Rect) {
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
                // Las casillas (de 17) abajo de su fila, en puntos enteros.
                row_bottom(ui, |ui| {
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
            // El botón y la casilla (de 17) abajo de la fila, en puntos enteros (centrados en la
            // fila de 18 quedaban a medio punto); el nombre y el color, centrados como siempre.
            row_bottom(ui, |ui| {
                if pixel_ui::text_button_sized(ui, "Detect BG", 50.0).clicked()
                    && let Some((_, image)) = &self.albedo
                {
                    self.background =
                        cs_render::detect_background(&image.pixels, image.width, image.height);
                }
                widgets::checkbox(ui, &mut self.hide_background, "Hide BG")
                    .tip("The background color counts as transparent");
            });
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
        // Debajo: el tamaño, cómo se usa (si cabe antes del zoom; si no, en el tooltip de la
        // vista previa) y el zoom.
        let size_text = format!("{}×{}", image.width, image.height);
        let zoom_text = format!("{:.0}%", k * 100.0);
        let gap = ui.spacing().item_spacing.x;
        let help = hint_galley(ui, PREVIEW_HELP).size().x;
        let row = body_width(ui, &size_text) + gap + help + gap + zoom_width(ui, &zoom_text);
        let help_fits = row <= width;
        if !help_fits {
            response.tip(PREVIEW_HELP);
        }
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(size_text);
            if help_fits {
                hint(ui, PREVIEW_HELP);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = ZOOM_GAP;
                // El botón (de 17) abajo de la fila, en puntos enteros (centrado en la fila de
                // 18 quedaba a medio punto); el zoom y el porcentaje, centrados como siempre.
                let slot = vec2(FIT_WIDTH, ui.spacing().interact_size.y);
                let bottom = egui::Layout::left_to_right(egui::Align::Max);
                let fit = ui.allocate_ui_with_layout(slot, bottom, |ui| {
                    pixel_ui::text_button_sized(ui, "Fit", FIT_WIDTH)
                });
                if fit.inner.clicked() {
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
                ui.label(zoom_text);
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

/// El ancho de `text` como lo pone `ui.label`.
fn body_width(ui: &Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), font, Color32::PLACEHOLDER);
    galley.size().x
}

/// El ancho del zoom de debajo de la vista previa: el porcentaje `percent`, − y + y Fit.
fn zoom_width(ui: &Ui, percent: &str) -> f32 {
    let skin = pixel_ui::frame_skin(ui.ctx());
    let buttons = skin
        .as_ref()
        .and_then(|s| s.piece("lumen.zoom", WidgetState::Normal))
        .map_or(18.0 + ZOOM_GAP + 18.0, |p| p.size[0] as f32);
    body_width(ui, percent) + ZOOM_GAP + buttons + ZOOM_GAP + FIT_WIDTH
}

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
/// Lo que pasó con los íconos de una caja de mapa.
#[derive(Default)]
struct MapIcons {
    /// Si se dibujaron (con Pixel UI y las piezas en el skin).
    shown: bool,
    browse: bool,
    remove: bool,
}

/// Los íconos de abrir (`icon.lumen.browse`, la carpeta) y quitar (`icon.lumen.remove`, la ✕),
/// con su contorno blanco de 1 px, abajo a la derecha de la caja `frame`: los contornos a 1 px uno
/// del otro, 6 px arriba del borde de abajo de la caja y 7 antes del de la derecha. Quitar,
/// apagado sin mapa.
fn map_icons(ui: &mut Ui, kind: MapKind, frame: Rect, present: bool) -> MapIcons {
    const BROWSE: &str = "icon.lumen.browse";
    const REMOVE: &str = "icon.lumen.remove";
    const RIGHT: f32 = 7.0;
    const BOTTOM: f32 = 6.0;
    const GAP: f32 = 1.0;
    let Some(skin) = pixel_ui::frame_skin(ui.ctx()) else {
        return MapIcons::default();
    };
    let (Some(browse), Some(remove)) = (
        skin.piece(BROWSE, WidgetState::Normal),
        skin.piece(REMOVE, WidgetState::Normal),
    ) else {
        return MapIcons::default();
    };
    let size = |p: &crate::skin::Piece| vec2(p.size[0] as f32, p.size[1] as f32);
    let remove_rect = Rect::from_min_size(
        pos2(
            frame.right() - RIGHT - size(remove).x,
            frame.bottom() - BOTTOM - size(remove).y,
        ),
        size(remove),
    );
    let browse_rect = Rect::from_min_size(
        pos2(
            remove_rect.left() - GAP - size(browse).x,
            frame.bottom() - BOTTOM - size(browse).y,
        ),
        size(browse),
    );
    let ppp = ui.ctx().pixels_per_point();
    let mut out = MapIcons {
        shown: true,
        ..Default::default()
    };
    for (piece, rect, label, enabled) in [
        (browse, browse_rect, "Browse…", true),
        (remove, remove_rect, "Remove", present),
    ] {
        let id = ui.id().with(("lumen_map_icon", kind.label(), label));
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        // Con 1 de más alrededor; el pixel entre los dos es de quitar (no se pisan).
        let hit = match label {
            "Remove" => rect.expand(1.0),
            _ => rect.expand(1.0).with_max_x(rect.right()),
        };
        let response = ui.interact(hit, id, sense);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        let mut shapes = pixel_ui::piece_shapes(piece, rect, ppp);
        if !enabled {
            // Apagado, a media opacidad.
            for shape in &mut shapes {
                if let egui::Shape::Mesh(mesh) = shape {
                    for v in &mut std::sync::Arc::make_mut(mesh).vertices {
                        v.color = Color32::from_white_alpha(110);
                    }
                }
            }
        }
        ui.painter().extend(shapes);
        if enabled {
            let response = response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip(label);
            if response.clicked() {
                match label {
                    "Remove" => out.remove = true,
                    _ => out.browse = true,
                }
            }
        }
    }
    out
}

fn stacked_slider(
    ui: &mut Ui,
    title: &str,
    value: &mut f32,
    range: (f32, f32),
    default: f32,
    format: impl Fn(f32) -> String,
) {
    let height = if pixel_ui::frame_skin(ui.ctx()).is_some() {
        // Sin las 2 filas vacías de debajo de la perilla: el aire entre controles ya las da.
        kit::slider::LABELED_HEIGHT - 2.0
    } else {
        SLIDER_HEIGHT
    };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.max_rect().width(), height), Sense::hover());
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
    // Con Pixel UI, el slider con encabezado del UI kit (botón de restaurar, nombre y valor).
    if let Some(skin) = pixel_ui::frame_skin(ui.ctx()) {
        let at = Rect::from_min_size(
            rect.min.round(),
            vec2(rect.width().round(), kit::slider::LABELED_HEIGHT),
        );
        let id = ui.id().with(("lumen_slider", title));
        kit::slider::labeled_slider_at(ui, &skin, at, id, title, value, lo..=hi, default, format);
        return;
    }
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
    cell.spacing_mut().slider_width = rect.width() - 2.0;
    cell.add(egui::Slider::new(value, lo..=hi).show_value(false));
}

/// Un botón con la muestra de `color` que abre el selector (opaco). Devuelve si cambió.
fn color_button(ui: &mut Ui, color: &mut Rgba) -> bool {
    widgets::color_edit(ui, color, false, "Color").changed()
}

/// La textura de `image`, con vecino más cercano.
pub(crate) fn texture_of(ui: &Ui, name: &str, image: &Bitmap) -> TextureHandle {
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

    /// Un estado del panel para los tests: su nombre y cómo se llega a él.
    type State = (&'static str, fn(&mut LumenPanel));

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

    /// De `MIN_WIDTH` para arriba, vacío, con imagen, con los ajustes del normal map o con un
    /// aviso, todo entra en el ancho del panel, en puntos enteros y sin que nada tape a nada. Los
    /// mapas van lado a lado desde `MAPS_ACROSS` (ahí, en su mínimo) y, más angosto, uno debajo
    /// de otro; los controles y la vista previa, lado a lado desde `SIDE_BY_SIDE` (con los
    /// controles del ancho de la columna de Albedo, si los mapas van lado a lado) y, más
    /// angosto, la vista previa debajo.
    #[test]
    fn the_lumen_panel_reflows_at_any_width() {
        use crate::forms::testing::run_at;
        let states: [State; 4] = [
            ("empty", |_| {}),
            ("image", |p| p.load("box".into(), sprite())),
            ("normal", |p| {
                p.load("box".into(), sprite());
                p.open_settings(MapKind::Normal);
            }),
            ("notice", |p| {
                p.load("box".into(), sprite());
                p.notify("The normal map is 2×2; it must be 10×10 like the albedo.".into());
            }),
        ];
        let widths = [
            MIN_WIDTH,
            300.0,
            SIDE_BY_SIDE,
            400.0,
            MAPS_ACROSS,
            REFERENCE_WIDTH,
            600.0,
        ];
        for width in widths {
            for (state, setup) in states {
                let mut lumen = LumenPanel::default();
                setup(&mut lumen);
                let probe = run_at(width, |ui| {
                    lumen.ui(ui);
                });
                let at = format!("{state} at {width}");
                probe.assert_whole_px();
                probe.assert_within(width);
                probe.assert_no_overlap();
                // Los mapas: Albedo (Use Layer) y Normal (su Generate: el primero, arriba a la
                // izquierda; el de Luminance va a su derecha o debajo).
                let albedo = probe.rect("Use Layer");
                let normal = probe
                    .widgets
                    .iter()
                    .filter(|(n, _)| n == "Generate")
                    .map(|(_, r)| *r)
                    .min_by(|a, b| {
                        (a.top(), a.left())
                            .partial_cmp(&(b.top(), b.left()))
                            .unwrap()
                    })
                    .expect("Generate");
                if width >= MAPS_ACROSS {
                    assert_eq!(normal.top(), albedo.top(), "side by side {at}");
                    let column = ((width - 2.0 * COLUMN_GAP) / 3.0).round();
                    assert!((albedo.width() - column).abs() <= 1.0, "{albedo:?} {at}");
                    assert_eq!(normal.left(), albedo.right() + COLUMN_GAP, "{at}");
                } else {
                    assert!(normal.top() > albedo.bottom(), "one under the other {at}");
                    assert_eq!(albedo.width(), width, "{at}");
                }
                // El pie, a la derecha.
                assert_eq!(probe.rect("Add as Artboard").right(), width, "{at}");
                if state == "empty" || state == "normal" {
                    continue;
                }
                // Los controles (Cursor) y, debajo de la vista previa, el tamaño de la imagen.
                let (cursor, size) = (probe.rect("Cursor"), probe.rect("10×10"));
                if width >= SIDE_BY_SIDE {
                    assert!(size.left() > cursor.right(), "side by side {at}");
                    if width >= MAPS_ACROSS {
                        assert_eq!(size.left(), albedo.right() + COLUMN_GAP, "{at}");
                    }
                } else {
                    assert_eq!(size.left(), 0.0, "under the controls {at}");
                }
                assert_eq!(probe.rect("Fit").right(), width, "{at}");
            }
        }
    }

    /// En `REFERENCE_WIDTH`, la fila de botones de cada mapa es la del mockup: Use Layer a todo
    /// lo ancho de Albedo y, en Normal y Luminance, Generate de su ancho justo (50), Modify de 45
    /// y Export con el resto (41), 2 entre ellos, hasta el borde de su columna de 140.
    #[test]
    fn the_map_buttons_match_the_mockup_at_the_reference_width() {
        use crate::forms::testing::run_at;
        let mut lumen = LumenPanel::default();
        lumen.load("box".into(), sprite());
        let probe = run_at(REFERENCE_WIDTH, |ui| {
            lumen.ui(ui);
        });
        let all = |name: &str| {
            let mut rects: Vec<(f32, f32)> = probe
                .widgets
                .iter()
                .filter(|(n, _)| n == name)
                .map(|(_, r)| (r.left(), r.width()))
                .collect();
            rects.sort_by(|a, b| a.partial_cmp(b).unwrap());
            rects
        };
        assert_eq!(all("Use Layer"), [(0.0, 140.0)]);
        assert_eq!(all("Generate"), [(147.0, 50.0), (294.0, 50.0)]);
        assert_eq!(all("Modify"), [(199.0, 45.0), (346.0, 45.0)]);
        assert_eq!(all("Export"), [(246.0, 41.0), (393.0, 41.0)]);
    }

    /// Con el contenido de `REFERENCE_WIDTH` desplazado `scrolled` hacia arriba (pintado en
    /// `content`, visto por un scroll que muestra desde más abajo): el rect del logo, su recorte y
    /// lo que se ve del scroll.
    fn logo_clip(scrolled: f32) -> (Rect, Rect, Rect) {
        let ctx = egui::Context::default();
        let skin = crate::skin::Skin::embedded(&ctx);
        let piece = skin.piece("lumen.logo", WidgetState::Normal);
        let size = piece.map(|p| vec2(p.size[0] as f32, p.size[1] as f32));
        let size = size.expect("the logo in the skin");
        ctx.set_fonts(crate::appearance::font_definitions(true));
        crate::appearance::apply_style(&ctx, Some(skin.colors()));
        crate::pixel_ui::set_bubble(&ctx, Some(std::sync::Arc::new(skin)));
        let content = Rect::from_min_size(pos2(10.0, 10.0), vec2(REFERENCE_WIDTH, 500.0));
        let viewport = content.with_min_y(content.min.y + scrolled);
        let logo = Rect::from_min_size(content.min + LOGO_AT, size);
        let mut panel = LumenPanel::default();
        let mut clip = None;
        // Tres frames: las fuentes llegan en el primero y taffy mide en el segundo.
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 1000.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
                    ui.set_width(content.width());
                    ui.set_clip_rect(viewport);
                    panel.ui(ui);
                });
            });
            output.textures_delta.clear();
            clip = output.shapes.iter().find_map(|c| match &c.shape {
                egui::Shape::Mesh(m) => {
                    let points: Vec<_> = m.vertices.iter().map(|v| v.pos).collect();
                    (Rect::from_points(&points) == logo).then_some(c.clip_rect)
                }
                _ => None,
            });
        }
        (logo, clip.expect("the logo"), viewport)
    }

    /// El logo sale 1 arriba y 3 a la izquierda del contenido (sobre el margen del panel): el
    /// recorte del scroll del dock, que empieza en el contenido, no le quita esas filas.
    #[test]
    fn the_logo_keeps_its_overhang_above_the_scroll_clip() {
        let (logo, clip, _) = logo_clip(0.0);
        assert!(clip.contains_rect(logo), "{logo:?} clipped by {clip:?}");
    }

    /// Con el contenido desplazado, el logo se recorta como lo demás: su recorte sube solo lo que
    /// el logo sale del contenido (1), no se pinta sobre el marco del panel.
    #[test]
    fn a_scrolled_logo_is_clipped_like_the_rest() {
        let (logo, clip, viewport) = logo_clip(5.0);
        assert!(
            clip.top() >= viewport.top() + LOGO_AT.y,
            "{logo:?} clipped by {clip:?}, viewport {viewport:?}"
        );
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
