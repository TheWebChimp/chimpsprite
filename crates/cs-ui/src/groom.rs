//! Groom, en el dock: de una imagen con pixel art falso (agrandada y borrosa, como las de una IA)
//! a pixel art de verdad. Se importa la imagen (o se toma la capa actual), se ajustan los pasos
//! (la cuadrícula, quitar el fondo, el contorno y los colores) mirando la vista previa, y el
//! resultado va a un sprite nuevo o a una capa.

use cs_core::Rgba;
use cs_io::export::Bitmap;
use cs_raster::GridGuess;
use eframe::egui::{
    self, Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, Ui, pos2, vec2,
};

use crate::forms::{self, TuiBuilderLogic as _};
use crate::panel_kit::{
    HINT_GAP, box_color, child, double_box, hint, hint_on_baseline, label_with_hint,
    notched_outline, row_bottom,
};
use crate::{pixel_ui, widgets};

/// Las imágenes que se pueden importar (en la web, solo PNG).
pub const FILTER: &[crate::platform::FileFilter] = &[crate::platform::FileFilter {
    name: "Images",
    extensions: if cfg!(target_arch = "wasm32") {
        &["png"]
    } else {
        &["png", "jpg", "jpeg", "gif", "webp"]
    },
}];

/// El panel (del diseño, desde la esquina de su contenido): el ancho del mockup (el del panel
/// flotante; adentro todo sale del ancho disponible) y el alto más chico.
pub const REFERENCE_WIDTH: f32 = 434.0;
/// Lo más angosto en que se ve bien (medido): los tres botones del pie en una fila (New Sprite,
/// Add as Layer y Add as Artboard miden 216 con su aire). Las columnas, una debajo de otra.
pub const MIN_WIDTH: f32 = 216.0;
const MIN_HEIGHT: f32 = 391.0;
/// La ilustración del panel sin imagen (los changos), del usuario, a 1x.
const EMPTY_ART: &[u8] = include_bytes!("../../../assets/ui/groom_empty.aseprite");
/// De la ilustración del panel sin imagen a la frase de abajo.
const EMPTY_GAP: f32 = 8.0;
/// Lo más ancho de la frase del panel sin imagen.
const EMPTY_TEXT_WIDTH: f32 = 300.0;
/// El encabezado: el logo (sobre el margen del panel) y, a la derecha, los botones para traer la
/// imagen (con `HEADER_GAP` entre ellos; si no caben, debajo del logo). La línea grabada va
/// `HEADER_LINE` debajo y las columnas empiezan `BODY_AIR` debajo.
const LOGO_AT: egui::Vec2 = vec2(-3.0, -1.0);
const HEADER_HEIGHT: f32 = 19.0;
const HEADER_GAP: f32 = 4.0;
const HEADER_LINE: f32 = 2.0;
const BODY_AIR: f32 = 5.0;
/// Las columnas: la de la izquierda (los pasos y la vista previa) y la de la derecha (los colores
/// y las sugerencias), de su ancho en el mockup; se achican hasta lo más ancho que no se parte
/// (medido: los campos del offset, `OFFSET_LEFT` y dos campos de 37 con 3 entre ellos, 167; la
/// fila de "Colors" con "Brightness" y la de "+n more groups" con "Merge All Suggested", 157 más
/// los dígitos) y crecen parejas con lo que sobre.
const LEFT_WIDTH: f32 = 181.0;
const LEFT_MIN: f32 = 167.0;
const RIGHT_WIDTH: f32 = 221.0;
const RIGHT_MIN: f32 = 167.0;
/// Entre las dos columnas (con la línea grabada `DIVIDER_AIR` a la derecha de la de la
/// izquierda) y, a la derecha de la de los colores, aire.
const COLUMN_GAP: f32 = 28.0;
const DIVIDER_AIR: f32 = 4.0;
const RIGHT_AIR: f32 = 4.0;
/// Lo más angosto en que las columnas van lado a lado; más angosto, la de los colores baja
/// debajo de la de los pasos, `STACK_GAP` debajo.
const SIDE_BY_SIDE: f32 = LEFT_MIN + COLUMN_GAP + RIGHT_MIN + RIGHT_AIR;
const STACK_GAP: f32 = 8.0;
/// La columna de "Offset" en la fila de la cuadrícula.
const OFFSET_LEFT: f32 = 90.0;
/// Las cajas de la vista previa y de las sugerencias, `BOXES_AIR` debajo de lo de arriba de la
/// columna más alta y a la misma altura: llegan hasta lo que va debajo de ellas (el tamaño y la
/// nota; "Merge All Suggested"), sin bajar de un mínimo. Una debajo de otra, la vista previa
/// tiene su alto y la de las sugerencias llega hasta abajo.
const BOXES_AIR: f32 = 4.0;
const BELOW_BOXES: f32 = 38.0;
const MIN_BOX_HEIGHT: f32 = 60.0;
const STACKED_PREVIEW: f32 = 120.0;
/// Del borde de una caja a lo de adentro: las dos líneas y 2 de aire.
const BOX_PAD: f32 = 5.0;
/// Abajo del todo, los botones (enteros dentro del contenido: lo de afuera lo recorta el
/// scroll) y, `FOOTER_AIR` arriba de ellos, la línea grabada.
const FOOTER_BUTTONS: f32 = pixel_ui::CONTROL_HEIGHT;
const FOOTER_AIR: f32 = 5.0;
/// Una sugerencia por fila.
const SUGGESTION_ROW: f32 = 15.0;
/// La lupa sobre la vista previa: su lado y cuántos píxeles del resultado se ven en ella.
const LOUPE: f32 = 120.0;
const LOUPE_PIXELS: f32 = 15.0;

/// Lo que se ve en la vista previa.
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    /// Con los colores juntados.
    Result,
    /// Con "Before" presionado: sin el grupo que se está probando o, si no hay, sin juntar
    /// ninguno.
    Before,
    /// Con los juntados y además este grupo (el que se está por juntar).
    Trying(Vec<Rgba>),
}

/// Los ajustes de los pasos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub grid: GridGuess,
    /// Las celdas siguen los bordes reales (para píxeles de tamaños irregulares).
    pub follow_edges: bool,
    pub background: bool,
    pub tolerance: u8,
    pub clean: bool,
    /// Un contorno de 1 px del color primario alrededor de todo.
    pub ring: bool,
    pub reduce: bool,
    pub colors: u16,
}

impl Settings {
    fn new(grid: GridGuess) -> Self {
        Self {
            grid,
            follow_edges: true,
            background: true,
            tolerance: 32,
            clean: true,
            ring: false,
            reduce: false,
            colors: 32,
        }
    }
}

/// El resultado: los píxeles (recortados a lo pintado) y sus colores, del más usado al menos.
#[derive(Clone, Debug, PartialEq)]
pub struct Groomed {
    pub name: String,
    pub image: Bitmap,
    pub colors: Vec<(Rgba, usize)>,
}

/// Colores juntados a mano: los de cada grupo pasan a ser el primero (el más usado).
pub type Merges = Vec<Vec<Rgba>>;

/// Cuántos colores se muestran para elegir (los más usados).
const MAX_SWATCHES: usize = 96;
/// Un cuadrito de color y su separación.
const SWATCH: f32 = 11.0;
const SWATCH_GAP: f32 = 2.0;

/// En qué orden se ven los colores (siempre los más usados; solo cambia el orden).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorSort {
    #[default]
    MostUsed,
    Hue,
    Brightness,
    Saturation,
}

impl ColorSort {
    const ALL: [ColorSort; 4] = [
        ColorSort::MostUsed,
        ColorSort::Hue,
        ColorSort::Brightness,
        ColorSort::Saturation,
    ];

    fn label(self) -> &'static str {
        match self {
            ColorSort::MostUsed => "Most Used",
            ColorSort::Hue => "Hue",
            ColorSort::Brightness => "Brightness",
            ColorSort::Saturation => "Saturation",
        }
    }
}

/// `colors` (del más usado al menos) en el orden `sort`: por tono (los grises al final, de
/// oscuro a claro), de oscuro a claro, o de más a menos saturado. Con empate, el más usado.
fn sorted_colors(colors: &[(Rgba, usize)], sort: ColorSort) -> Vec<(Rgba, usize)> {
    let mut out = colors.to_vec();
    let hsv = |c: Rgba| egui::ecolor::Hsva::from_srgba_unmultiplied([c.r, c.g, c.b, 255]);
    // Lo que ve el ojo: el verde pesa más que el rojo y el azul.
    let luma = |c: Rgba| 299 * c.r as u32 + 587 * c.g as u32 + 114 * c.b as u32;
    // En pasos enteros (los `f32` no se comparan de una).
    let step = |v: f32| (v * 1000.0).round() as i32;
    match sort {
        ColorSort::MostUsed => {}
        ColorSort::Hue => out.sort_by_key(|&(c, _)| {
            let h = hsv(c);
            let gray = h.s < 0.12;
            (gray, if gray { 0 } else { step(h.h) }, luma(c))
        }),
        ColorSort::Brightness => out.sort_by_key(|&(c, _)| luma(c)),
        ColorSort::Saturation => out.sort_by_key(|&(c, _)| std::cmp::Reverse(step(hsv(c).s))),
    }
    out
}

/// Una paleta para ajustar el resultado: la del documento o una de la biblioteca.
pub struct PaletteChoice {
    pub name: String,
    pub colors: Vec<Rgba>,
}

/// La paleta del documento, la primera de la lista.
pub const CURRENT_PALETTE: &str = "Current Palette";
/// Sin ajustar a una paleta: los colores que salen de los pasos (antes de la lista).
const ORIGINAL_COLORS: &str = "Original Colors";

/// Lo que se pidió desde el panel (la app lo resuelve).
pub enum GroomAction {
    Import,
    UseLayer,
    /// Abrir Lospec (para guardar una paleta y ajustarse a ella).
    BrowseLospec,
    NewSprite(Groomed),
    AddLayer(Groomed),
    AddArtboard(Groomed),
}

pub struct GroomPanel {
    /// La imagen de partida y su nombre.
    source: Option<(String, Bitmap)>,
    settings: Option<Settings>,
    /// Los colores juntados a mano (se aplican al final, cada vez).
    merges: Merges,
    /// Los colores elegidos para juntar.
    picked: Vec<Rgba>,
    /// Qué tan parecidos tienen que ser los colores para sugerir juntarlos.
    similar: u16,
    /// El orden de los colores.
    sort: ColorSort,
    /// La paleta a la que se ajusta el resultado (`ORIGINAL_COLORS`: a ninguna) y sus colores
    /// (los de ahora: la del documento cambia).
    fit_name: String,
    fit_colors: Option<Vec<Rgba>>,
    /// Los pasos con `settings`, el primario y la paleta de entonces, sin juntar colores (lo
    /// pesado).
    base: Option<(Settings, Rgba, Option<Vec<Rgba>>, Groomed)>,
    /// `base` con los colores juntados de entonces.
    result: Option<(Merges, Groomed)>,
    /// La sugerencia bajo el mouse la vez pasada (la vista previa la prueba).
    hovered_group: Option<Vec<Rgba>>,
    /// Lo que muestra la vista previa y su textura.
    texture: Option<(Shown, TextureHandle)>,
    /// La textura de la ilustración sin imagen (se carga la primera vez que se ve).
    empty_texture: Option<TextureHandle>,
}

impl Default for GroomPanel {
    fn default() -> Self {
        Self {
            source: None,
            settings: None,
            merges: Merges::new(),
            picked: Vec::new(),
            similar: 40,
            sort: ColorSort::default(),
            fit_name: ORIGINAL_COLORS.into(),
            fit_colors: None,
            base: None,
            result: None,
            hovered_group: None,
            texture: None,
            empty_texture: None,
        }
    }
}

/// El encabezado en `tui`: el logo (sin Pixel UI, el título) y, a la derecha, importar una
/// imagen o tomar la capa actual (debajo del logo si no caben). Devuelve su rect.
fn header(
    tui: &mut forms::Tui,
    logo: Option<&crate::skin::Piece>,
    action: &mut Option<GroomAction>,
) -> Rect {
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
                    // con él se perdía su fila de arriba. El recorte crece solo eso (1 arriba,
                    // 3 a la izquierda): con el contenido desplazado, el logo se recorta como lo
                    // demás y no se pinta sobre el marco. (`with_clip_rect` intersecta con el
                    // recorte de ahora; `set_clip_rect` lo cambia y deja lo demás del painter:
                    // invisible en la pasada que mide, apagado con un modal.)
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
                        "Pixel Groom".into(),
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
        let buttons = tui.style(forms::push_right(forms::natural())).ui(|ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                if pixel_ui::text_button(ui, "Import Image…").clicked() {
                    *action = Some(GroomAction::Import);
                }
                if pixel_ui::text_button(ui, "Use Layer").clicked() {
                    *action = Some(GroomAction::UseLayer);
                }
            });
            ui.max_rect()
        });
        title.union(buttons)
    })
}

impl GroomPanel {
    /// Empieza con `image`: detecta su cuadrícula.
    pub fn load(&mut self, name: String, image: Bitmap) {
        let grid = cs_raster::detect_grid(&image.pixels, image.width, image.height);
        self.settings = Some(Settings::new(grid));
        self.source = Some((name, image));
        self.base = None;
        self.result = None;
        self.merges.clear();
        self.picked.clear();
    }

    /// El resultado con los ajustes de ahora: los pasos se rehacen solo si cambiaron los
    /// ajustes (o el primario) y los colores juntados, solo si cambiaron ellos.
    fn groomed(&mut self, primary: Rgba) -> Option<&Groomed> {
        let (name, source) = self.source.as_ref()?;
        let settings = self.settings?;
        let fit = &self.fit_colors;
        if !matches!(&self.base,
            Some((s, p, f, _)) if *s == settings && *p == primary && f == fit)
        {
            let palette = fit.as_deref();
            let base = process(name, source, settings, primary, palette, &Merges::new());
            self.base = Some((settings, primary, fit.clone(), base));
            self.result = None;
        }
        if !matches!(&self.result, Some((m, _)) if *m == self.merges) {
            let (_, _, _, base) = self.base.as_ref()?;
            self.result = Some((self.merges.clone(), merged(base, &self.merges)));
            self.texture = None;
        }
        self.result.as_ref().map(|(_, g)| g)
    }

    /// Desarrollo (capturas): ajusta a la paleta `name`.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn fit_to(&mut self, name: &str) {
        self.fit_name = name.into();
    }

    /// Desarrollo (capturas): elige los `n` colores más usados.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pick_most_used(&mut self, primary: Rgba, n: usize) {
        if let Some(groomed) = self.groomed(primary) {
            self.picked = groomed.colors.iter().take(n).map(|(c, _)| *c).collect();
        }
    }

    /// Sin imagen: la ilustración de los changos y, debajo, la frase (de lo más ancha
    /// `EMPTY_TEXT_WIDTH`), centradas en `ui`.
    fn empty_ui(&mut self, ui: &mut Ui) {
        const TEXT: &str = "Bring an image that looks like pixel art (blurry, scaled up) to make \
                            it real pixel art.";
        let area = ui.max_rect();
        if self.empty_texture.is_none()
            && let Some(art) = empty_art()
        {
            let texture = ui
                .ctx()
                .load_texture("groom_empty", art, TextureOptions::NEAREST);
            self.empty_texture = Some(texture);
        }
        let galley = egui::WidgetText::from(TEXT).into_galley(
            ui,
            Some(egui::TextWrapMode::Wrap),
            EMPTY_TEXT_WIDTH.min(area.width()),
            egui::TextStyle::Body,
        );
        let art_size = self
            .empty_texture
            .as_ref()
            .map_or(egui::Vec2::ZERO, |t| t.size_vec2());
        let art = empty_art_rect(area, art_size, galley.size().y);
        if let Some(texture) = &self.empty_texture {
            ui.painter().image(
                texture.id(),
                art,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        // Del ancho de la frase (con las líneas centradas), en puntos enteros.
        let text_width = galley.size().x.ceil();
        let text = Rect::from_min_size(
            pos2(
                (area.center().x - text_width / 2.0).round(),
                art.bottom() + EMPTY_GAP,
            ),
            vec2(text_width, galley.size().y),
        );
        let centered = egui::Layout::top_down(egui::Align::Center);
        child(ui, text, centered).add(egui::Label::new(TEXT).wrap());
    }

    /// El grupo que se está por juntar: el de la sugerencia bajo el mouse o, si no, los colores
    /// elegidos (el más usado primero, como lo deja Merge).
    fn trying(&self, groomed: &Groomed) -> Option<Vec<Rgba>> {
        if let Some(group) = &self.hovered_group {
            return Some(group.clone());
        }
        (self.picked.len() >= 2).then(|| merge_order(self.picked.clone(), groomed))
    }

    /// El panel, como en el diseño, del ancho de `ui`: arriba el logo y los botones para traer la
    /// imagen; a la izquierda los pasos y la vista previa, a la derecha los colores y las
    /// sugerencias (las dos cajas de abajo a la misma altura, hasta el pie), y abajo, lo que se
    /// hace con el resultado. Si las columnas no caben lado a lado (`SIDE_BY_SIDE`), la de los
    /// colores va debajo de la de los pasos.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        primary: Rgba,
        palettes: &[PaletteChoice],
    ) -> Option<GroomAction> {
        let mut action = None;
        self.fit_colors = (self.fit_name != ORIGINAL_COLORS)
            .then(|| palettes.iter().find(|p| p.name == self.fit_name))
            .flatten()
            .map(|p| p.colors.clone());
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
            s.piece("groom.logo", crate::skin::WidgetState::Normal)
                .cloned()
        });
        // Las líneas grabadas van detrás de todo: su lugar, antes; se dibujan al final, cuando se
        // sabe dónde quedó cada cosa.
        let lines = ui.painter().add(egui::Shape::Noop);
        let side_by_side = width >= SIDE_BY_SIDE;
        let mut top = Rect::NOTHING;
        let mut steps = None;
        let mut footer = None;
        let mut lospec = false;
        forms::form(ui, "groom", 0.0, |tui| {
            let panel = forms::min_height(height, forms::column(0.0));
            tui.style(panel).add(|tui| {
                top = header(tui, logo.as_ref(), &mut action);
                if self.source.is_none() {
                    // Los changos y la frase, en lo que queda hasta donde iría el pie.
                    let area = forms::fill_height(0.0, forms::Style::default());
                    forms::canvas(tui, forms::air_above(BODY_AIR, area), |ui| {
                        self.empty_ui(ui);
                    });
                    tui.style(forms::fixed(0.0, FOOTER_AIR + FOOTER_BUTTONS))
                        .add_empty();
                    return;
                }
                let left = forms::grow_auto(LEFT_MIN, LEFT_WIDTH);
                let right = forms::grow_auto(RIGHT_MIN, RIGHT_WIDTH);
                let groomed;
                if side_by_side {
                    // Arriba, los pasos y los colores; abajo, las dos cajas, hasta el pie.
                    let columns = forms::air_above(BODY_AIR, forms::row_nowrap(COLUMN_GAP));
                    groomed = tui.style(columns).add(|tui| {
                        let (rect, browse) = self.steps_ui(tui, left.clone(), palettes);
                        (steps, lospec) = (Some(rect), browse);
                        let groomed = self.groomed(primary).cloned();
                        if let Some(groomed) = &groomed {
                            let right = forms::air_right(RIGHT_AIR, right.clone());
                            self.colors_ui(tui, right, groomed);
                        }
                        groomed
                    });
                    if let Some(groomed) = &groomed {
                        let boxes = MIN_BOX_HEIGHT + BELOW_BOXES;
                        let row = forms::fill_height(boxes, forms::row_nowrap(COLUMN_GAP));
                        tui.style(forms::air_above(BOXES_AIR, row)).add(|tui| {
                            self.preview_ui(tui, forms::full_height(left), groomed);
                            let right = forms::air_right(RIGHT_AIR, forms::full_height(right));
                            self.suggestions_ui(tui, right, groomed);
                        });
                    }
                } else {
                    // Una debajo de otra: los pasos y la vista previa; los colores y las
                    // sugerencias, hasta el pie.
                    let row = |air| forms::air_above(air, forms::row_nowrap(0.0));
                    groomed = tui.style(row(BODY_AIR)).add(|tui| {
                        let (rect, browse) = self.steps_ui(tui, left.clone(), palettes);
                        (steps, lospec) = (Some(rect), browse);
                        self.groomed(primary).cloned()
                    });
                    if let Some(groomed) = &groomed {
                        let h = STACKED_PREVIEW + BELOW_BOXES;
                        let preview = forms::grow(LEFT_MIN, LEFT_WIDTH, None, h);
                        tui.style(row(BOXES_AIR))
                            .add(|tui| self.preview_ui(tui, preview, groomed));
                        tui.style(row(STACK_GAP))
                            .add(|tui| self.colors_ui(tui, right.clone(), groomed));
                        let boxes = MIN_BOX_HEIGHT + BELOW_BOXES;
                        tui.style(forms::fill_height(boxes, row(BOXES_AIR)))
                            .add(|tui| {
                                self.suggestions_ui(tui, forms::full_height(right), groomed)
                            });
                    }
                }
                // Abajo, a la derecha: lo que se hace con el resultado.
                let buttons = forms::min_height(FOOTER_BUTTONS, forms::natural());
                let row = forms::air_above(FOOTER_AIR, forms::row_nowrap(0.0));
                footer = Some(tui.style(row).add(|tui| {
                    tui.style(forms::push_right(buttons)).ui(|ui| {
                        let Some(groomed) = &groomed else {
                            return ui.max_rect();
                        };
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            if pixel_ui::text_button(ui, "New Sprite").clicked() {
                                action = Some(GroomAction::NewSprite(groomed.clone()));
                            }
                            if pixel_ui::text_button(ui, "Add as Layer").clicked() {
                                action = Some(GroomAction::AddLayer(groomed.clone()));
                            }
                            if pixel_ui::text_button(ui, "Add as Artboard").clicked() {
                                action = Some(GroomAction::AddArtboard(groomed.clone()));
                            }
                        });
                        ui.max_rect()
                    })
                }));
            });
        });
        if lospec {
            action = Some(GroomAction::BrowseLospec);
        }
        // Las líneas grabadas: debajo del encabezado y, con imagen, entre las columnas (si van
        // lado a lado) y arriba de los botones de abajo. De borde a borde del panel.
        if let Some(skin) = &skin {
            let colors = skin.colors();
            let ppp = ui.ctx().pixels_per_point();
            let margin = f32::from(crate::dock::FLYOUT_MARGIN) - 1.0;
            let across = egui::Rangef::new(origin.x - margin, origin.x + width + margin);
            let header_line = top.bottom() + HEADER_LINE;
            let mut shapes = vec![pixel_ui::etched_hline(across, header_line, colors, ppp)];
            if let Some(footer) = footer {
                let footer_line = footer.top() - FOOTER_AIR;
                if let Some(steps) = steps.filter(|_| side_by_side) {
                    shapes.push(pixel_ui::etched_vline(
                        steps.right() + DIVIDER_AIR,
                        egui::Rangef::new(header_line, footer_line),
                        colors,
                        ppp,
                    ));
                }
                shapes.push(pixel_ui::etched_hline(across, footer_line, colors, ppp));
            }
            ui.painter().set(lines, egui::Shape::Vec(shapes));
        }
        action
    }

    /// Lo de arriba de la columna de la izquierda, en una hoja de `style` de `tui`: los pasos.
    /// Devuelve su rect y si se pidió abrir Lospec.
    fn steps_ui(
        &mut self,
        tui: &mut forms::Tui,
        style: forms::Style,
        palettes: &[PaletteChoice],
    ) -> (Rect, bool) {
        tui.style(style).ui(|ui| {
            let lospec = self.settings_ui(ui, palettes);
            // Con el aire de egui de abajo (lo que sigue va a su paso).
            ui.add_space(0.0);
            (ui.max_rect(), lospec)
        })
    }

    /// Lo de arriba de la columna de la derecha, en una hoja de `style` de `tui`: los colores y
    /// la tolerancia de las sugerencias.
    fn colors_ui(&mut self, tui: &mut forms::Tui, style: forms::Style, groomed: &Groomed) {
        tui.style(style).ui(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            self.swatches(ui, groomed);
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(5.0);
                ui.label("Suggested");
                widgets::number(ui, &mut self.similar, 1..=255, "");
                ui.label("tolerance");
            });
            // Con el aire de egui de abajo (lo que sigue va a su paso).
            ui.add_space(0.0);
        });
    }

    /// La vista previa en una hoja de `style` de `tui` (su caja, lo que la hoja deja sin lo de
    /// abajo).
    fn preview_ui(&mut self, tui: &mut forms::Tui, style: forms::Style, groomed: &Groomed) {
        forms::canvas(tui, style, |ui| {
            let box_height = (ui.max_rect().height() - BELOW_BOXES).max(MIN_BOX_HEIGHT);
            self.preview(ui, groomed, box_height);
        });
    }

    /// Las sugerencias en una hoja de `style` de `tui` (su caja, lo que la hoja deja sin lo de
    /// abajo).
    fn suggestions_ui(&mut self, tui: &mut forms::Tui, style: forms::Style, groomed: &Groomed) {
        forms::canvas(tui, style, |ui| {
            let box_height = (ui.max_rect().height() - BELOW_BOXES).max(MIN_BOX_HEIGHT);
            self.hovered_group = self.suggestions(ui, groomed, box_height);
        });
    }

    /// Los pasos: la cuadrícula (el tamaño de cada píxel de verdad y dónde empieza), el fondo,
    /// el contorno, los colores y la paleta. Devuelve si se pidió abrir Lospec.
    fn settings_ui(&mut self, ui: &mut Ui, palettes: &[PaletteChoice]) -> bool {
        let Some((_, source)) = &self.source else {
            return false;
        };
        let (width, height) = (source.width, source.height);
        let Some(settings) = self.settings.as_mut() else {
            return false;
        };
        let left = ui.cursor().left();
        let to_offset = |ui: &mut Ui| {
            let x = left + OFFSET_LEFT - ui.cursor().left();
            ui.add_space(x.max(0.0));
        };
        ui.horizontal(|ui| {
            ui.label("Cell")
                .on_hover_text("How big each real pixel is in the image");
            to_offset(ui);
            ui.label("Offset").on_hover_text("Where the grid starts");
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            widgets::prefixed_number(ui, &mut settings.grid.cell.0, 1.0..=width as f32, "W");
            widgets::prefixed_number(ui, &mut settings.grid.cell.1, 1.0..=height as f32, "H");
            to_offset(ui);
            let (cx, cy) = settings.grid.cell;
            widgets::prefixed_number(ui, &mut settings.grid.offset.0, -cx..=cx, "X");
            widgets::prefixed_number(ui, &mut settings.grid.offset.1, -cy..=cy, "Y");
        });
        ui.add_space(2.0);
        row_bottom(ui, |ui| {
            if pixel_ui::text_button(ui, "Detect Grid").clicked() {
                settings.grid = cs_raster::detect_grid(&source.pixels, width, height);
            }
            to_offset(ui);
            widgets::checkbox(ui, &mut settings.follow_edges, "Follow Edges")
                .on_hover_text("Let each cell snap to the real edges (for pixels of uneven sizes)");
        });
        ui.add_space(6.0);
        row_bottom(ui, |ui| {
            widgets::checkbox(ui, &mut settings.background, "Remove Background");
            ui.add_enabled_ui(settings.background, |ui| {
                widgets::number(ui, &mut settings.tolerance, 0..=255, "");
            });
        });
        widgets::checkbox(ui, &mut settings.clean, "Clean Outline");
        row_bottom(ui, |ui| {
            // La casilla pone sus mayúsculas `kit::check::CAPS_TOP` debajo de la fila.
            let caps = ui.cursor().top().round() + crate::kit::check::CAPS_TOP;
            let check = widgets::checkbox(ui, &mut settings.ring, "Add Outline");
            let left = (check.rect.right() + HINT_GAP).round();
            hint_on_baseline(
                ui,
                "(primary color)",
                left,
                caps + crate::tab_bar::CAP_HEIGHT,
            );
        });
        row_bottom(ui, |ui| {
            widgets::checkbox(ui, &mut settings.reduce, "Reduce Colors");
            ui.add_enabled_ui(settings.reduce, |ui| {
                widgets::number(ui, &mut settings.colors, 2..=256, "");
            });
        });
        // Ajustar a una paleta: cada color, el más parecido de ella.
        let mut lospec = false;
        ui.horizontal(|ui| {
            ui.label("Palette")
                .on_hover_text("Change each color to the most similar one in the palette");
            {
                let room = ui.available_width().floor();
                // Lo que el combo pone alrededor del nombre (el aire y la flecha).
                let font = egui::TextStyle::Body.resolve(ui.style());
                let shown = crate::tab_bar::fit_text(ui, &self.fit_name, &font, room - 24.0);
                let height = crate::bar_fields::FIELD_HEIGHT;
                widgets::combo_sized(ui, "groom_fit", &shown, Some(room), height, |ui| {
                    let original = ORIGINAL_COLORS.to_string();
                    widgets::choice(ui, &mut self.fit_name, original, ORIGINAL_COLORS);
                    crate::kit::select::list_separator(ui);
                    for palette in palettes {
                        let name = palette.name.clone();
                        widgets::choice(ui, &mut self.fit_name, name, &palette.name);
                    }
                    crate::kit::select::list_separator(ui);
                    if ui
                        .add(crate::menus::choice(false, "Browse Lospec…"))
                        .clicked()
                    {
                        lospec = true;
                        ui.close();
                    }
                });
            }
        });
        lospec
    }

    /// Junta `group` (el primero queda; los demás pasan a ser él) y suelta los elegidos.
    fn merge(&mut self, group: Vec<Rgba>) {
        if group.len() > 1 {
            self.merges.push(group);
        }
        self.picked.clear();
    }

    /// Grupos de colores parecidos para juntar, en su caja: un clic elige los de uno (para
    /// revisarlos y juntarlos con Merge) y "Merge All" los junta todos. Devuelve el que está bajo
    /// el mouse.
    fn suggestions(
        &mut self,
        ui: &mut Ui,
        groomed: &Groomed,
        box_height: f32,
    ) -> Option<Vec<Rgba>> {
        let groups = cs_raster::suggest_merges(&groomed.colors, self.similar as u32);
        let width = ui.max_rect().width();
        let (frame, _) = ui.allocate_exact_size(vec2(width, box_height), Sense::hover());
        double_box(ui, frame);
        let inner = frame.shrink(BOX_PAD);
        if groups.is_empty() {
            ui.painter().text(
                inner.left_top(),
                egui::Align2::LEFT_TOP,
                "No similar colors.",
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            );
            return None;
        }
        let pitch = SWATCH + SWATCH_GAP;
        let per_row = ((inner.width() + SWATCH_GAP) / pitch).floor() as usize;
        let fits = ((inner.height() + SUGGESTION_ROW - SWATCH) / SUGGESTION_ROW).floor() as usize;
        let shown = groups.len().min(fits);
        let border = pixel_ui::control_border(ui.ctx());
        let accent = crate::theme::ACCENT;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let ink = ui.visuals().text_color();
        let mut chosen = None;
        let mut hovered = None;
        for (i, group) in groups.iter().take(shown).enumerate() {
            let line = Rect::from_min_size(
                inner.min + vec2(0.0, i as f32 * SUGGESTION_ROW),
                vec2(inner.width(), SWATCH),
            );
            let response = ui
                .interact(line, ui.id().with(("groom_suggestion", i)), Sense::click())
                .on_hover_text("Pick these colors (the first one stays)");
            if response.clicked() {
                chosen = Some(group.clone());
            }
            if response.hovered() {
                hovered = Some(group.clone());
            }
            let painter = ui.painter();
            let picked = self.picked == *group;
            if picked || response.hovered() {
                let fill = if picked { accent } else { border };
                painter.rect_filled(line.expand(1.0), 0.0, fill.gamma_multiply(0.35));
            }
            // El que queda, una flecha y los que pasan a ser él; al final, cuántos más.
            let cells = per_row.saturating_sub(3);
            let mut x = line.left();
            for (k, &color) in group.iter().take(cells + 1).enumerate() {
                let rect = Rect::from_min_size(pos2(x, line.top()), vec2(SWATCH, SWATCH));
                painter.rect_filled(rect, 0.0, crate::panels::to_color32(color));
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
                x += pitch;
                if k == 0 {
                    let arrow = pos2(x + SWATCH / 2.0, line.center().y);
                    painter.text(arrow, egui::Align2::CENTER_CENTER, "←", font.clone(), ink);
                    x += pitch;
                }
            }
            if group.len() > cells + 1 {
                let more = format!("+{}", group.len() - cells - 1);
                let at = pos2(line.right(), line.center().y);
                painter.text(at, egui::Align2::RIGHT_CENTER, more, font.clone(), ink);
            }
        }
        if let Some(group) = chosen {
            self.picked = if self.picked == group {
                Vec::new()
            } else {
                group
            };
        }
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            if groups.len() > shown {
                hint(ui, &format!("+{} more groups", groups.len() - shown));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if pixel_ui::text_button(ui, "Merge All Suggested").clicked() {
                    self.picked.clear();
                    self.merges.extend(groups);
                }
            });
        });
        hovered
    }

    /// Los colores del resultado (los más usados primero): un clic elige o suelta uno, y
    /// "Merge" junta los elegidos en el más usado de ellos.
    fn swatches(&mut self, ui: &mut Ui, groomed: &Groomed) {
        ui.horizontal(|ui| {
            ui.add_space(5.0);
            label_with_hint(ui, "Colors", "(click to pick)");
            // A la derecha, el orden.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let height = crate::bar_fields::FIELD_HEIGHT;
                let sort = &mut self.sort;
                widgets::combo_sized(ui, "groom_sort", sort.label(), None, height, |ui| {
                    for option in ColorSort::ALL {
                        widgets::choice(ui, sort, option, option.label());
                    }
                })
                .on_hover_text("Sort the colors");
            });
        });
        let shown = groomed.colors.len().min(MAX_SWATCHES);
        let ordered = sorted_colors(&groomed.colors[..shown], self.sort);
        let pitch = SWATCH + SWATCH_GAP;
        let width = ui.max_rect().width();
        let per_row = ((width - 2.0 * BOX_PAD + SWATCH_GAP) / pitch)
            .floor()
            .max(1.0) as usize;
        let rows = shown.div_ceil(per_row).max(1);
        let more = groomed.colors.len() > shown;
        let text_row = if more { 12.0 } else { 0.0 };
        let height = 2.0 * BOX_PAD + rows as f32 * pitch - SWATCH_GAP + text_row;
        let (frame, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        double_box(ui, frame);
        let inner = frame.shrink(BOX_PAD);
        let border = pixel_ui::control_border(ui.ctx());
        let accent = crate::theme::ACCENT;
        for (i, &(color, count)) in ordered.iter().enumerate() {
            let (col, row) = ((i % per_row) as f32, (i / per_row) as f32);
            let rect =
                Rect::from_min_size(inner.min + vec2(col, row) * pitch, vec2(SWATCH, SWATCH));
            let response = ui
                .interact(rect, ui.id().with(("groom_swatch", i)), Sense::click())
                .on_hover_text(format!("{} · {count} px", color.to_hex()));
            if response.clicked() {
                match self.picked.iter().position(|&c| c == color) {
                    Some(at) => {
                        self.picked.remove(at);
                    }
                    None => self.picked.push(color),
                }
            }
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, crate::panels::to_color32(color));
            let picked = self.picked.contains(&color);
            let (stroke, width) = if picked { (accent, 2.0) } else { (border, 1.0) };
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(width, stroke),
                egui::StrokeKind::Inside,
            );
        }
        // Los que no se muestran, dentro de la caja, abajo.
        if more {
            let rect = Rect::from_min_max(pos2(inner.left(), inner.bottom() - text_row), inner.max);
            let mut text = child(ui, rect, egui::Layout::left_to_right(egui::Align::Min));
            let more = format!("+{} more", groomed.colors.len() - shown);
            label_with_hint(&mut text, &more, "(Reduce Colors first)");
        }
        ui.add_space(3.0);
        ui.allocate_ui_with_layout(
            vec2(width, 16.0),
            egui::Layout::right_to_left(egui::Align::Min),
            |ui| {
                let can_merge = self.picked.len() >= 2;
                if ui
                    .add_enabled_ui(can_merge, |ui| pixel_ui::text_button(ui, "Merge"))
                    .inner
                    .clicked()
                    && can_merge
                {
                    // El más usado de los elegidos queda; los demás pasan a ser él.
                    let group = merge_order(self.picked.clone(), groomed);
                    self.merge(group);
                }
                if !self.merges.is_empty() && pixel_ui::text_button(ui, "Reset Merges").clicked() {
                    self.merges.clear();
                    self.picked.clear();
                }
            },
        );
    }

    /// El resultado al tamaño más grande que entra (en múltiplos enteros si se agranda), sobre
    /// el damero. Con un grupo por juntar (una sugerencia bajo el mouse o colores elegidos) se ve
    /// cómo quedaría, y con "Before" presionado, cómo era sin juntar ninguno. Encima, la lupa;
    /// un clic elige (o suelta) el color del píxel.
    fn preview(&mut self, ui: &mut Ui, groomed: &Groomed, box_height: f32) {
        if groomed.image.width == 0 || groomed.image.height == 0 {
            return;
        }
        let before = ui
            .data(|d| d.get_temp::<bool>(before_id()))
            .unwrap_or(false);
        let trying = self.trying(groomed);
        let can_compare = trying.is_some() || !self.merges.is_empty();
        let shown = match trying {
            _ if before && can_compare => Shown::Before,
            Some(group) => Shown::Trying(group),
            None => Shown::Result,
        };
        let image = match &shown {
            Shown::Result => groomed.image.clone(),
            // Lo de antes: sin lo que se prueba o, si no se prueba nada, sin juntar ninguno.
            Shown::Before if self.trying(groomed).is_some() => groomed.image.clone(),
            Shown::Before => match &self.base {
                Some((_, _, _, base)) => base.image.clone(),
                None => groomed.image.clone(),
            },
            Shown::Trying(group) => merged(groomed, &vec![group.clone()]).image,
        };
        if !matches!(&self.texture, Some((s, _)) if *s == shown) {
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|c| [c.r, c.g, c.b, c.a])
                .collect();
            let color = ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &bytes,
            );
            let texture = ui
                .ctx()
                .load_texture("groom_preview", color, TextureOptions::NEAREST);
            self.texture = Some((shown.clone(), texture));
        }
        let Some((_, texture)) = &self.texture else {
            return;
        };
        // La caja: una línea y, adentro, el damero con la imagen al centro.
        let width = ui.max_rect().width();
        let (frame, _) = ui.allocate_exact_size(vec2(width, box_height), Sense::hover());
        ui.painter()
            .extend(notched_outline(frame, box_color(ui), 1.0));
        let well = frame.shrink(2.0);
        crate::panels::checker(&ui.painter_at(well), well);
        let fit = (well.width() / image.width as f32).min(well.height() / image.height as f32);
        let k = if fit >= 1.0 { fit.floor() } else { fit };
        let size = vec2(image.width as f32, image.height as f32) * k;
        let min = (well.center() - size / 2.0).round();
        let rect = Rect::from_min_size(min, size);
        let response = ui.interact(rect, ui.id().with("groom_preview"), Sense::click());
        let painter = ui.painter_at(rect);
        let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), rect, full, Color32::WHITE);
        // El píxel bajo el mouse: la lupa y, con un clic, elegir su color. (Para capturas, con
        // CHIMPSPRITE_DEMO_GROOM_LOUPE, en el centro.)
        let demo = crate::demo::dev_var("CHIMPSPRITE_DEMO_GROOM_LOUPE").map(|_| rect.center());
        if let Some(pointer) = response.hover_pos().or(demo) {
            let at = ((pointer - rect.min) / k).floor();
            let (x, y) = (at.x as u32, at.y as u32);
            if x < image.width && y < image.height {
                let color = image.pixels[(y * image.width + x) as usize];
                loupe(ui, texture, &image, (x, y), color, pointer);
                if response.clicked() && color.a > 0 {
                    match self.picked.iter().position(|&c| c == color) {
                        Some(at) => {
                            self.picked.remove(at);
                        }
                        None => self.picked.push(color),
                    }
                }
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(format!(
                "{}×{} · {} colors",
                groomed.image.width,
                groomed.image.height,
                groomed.colors.len()
            ));
            // "Before": mientras está presionado, lo de antes de juntar.
            if can_compare {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let held = pixel_ui::text_button(ui, "Before")
                        .on_hover_text("Hold to see it before merging")
                        .is_pointer_button_down_on();
                    ui.data_mut(|d| d.insert_temp(before_id(), held));
                });
            }
        });
        let note = match shown {
            Shown::Trying(_) => "Previewing the merge (Merge keeps it).",
            Shown::Before => "Before merging.",
            Shown::Result => "Hover to zoom; click a pixel to pick its color.",
        };
        hint(ui, note);
    }
}

/// Dónde se guarda si "Before" está presionado (se lee al pintar la vista previa, arriba del
/// botón: vale lo de la vez pasada).
fn before_id() -> egui::Id {
    egui::Id::new("groom_before")
}

/// La lupa junto al mouse: los píxeles alrededor de `(x, y)` en grande, el del centro marcado y
/// su color.
fn loupe(
    ui: &Ui,
    texture: &TextureHandle,
    image: &Bitmap,
    (x, y): (u32, u32),
    color: Rgba,
    pointer: egui::Pos2,
) {
    let layer = egui::LayerId::new(egui::Order::Tooltip, ui.id().with("groom_loupe"));
    let painter = ui.ctx().layer_painter(layer);
    let screen = ui.ctx().content_rect();
    // A la derecha y abajo del mouse; si no cabe, del otro lado.
    let mut min = pointer + vec2(18.0, 18.0);
    if min.x + LOUPE > screen.right() {
        min.x = pointer.x - 18.0 - LOUPE;
    }
    if min.y + LOUPE + 16.0 > screen.bottom() {
        min.y = pointer.y - 18.0 - LOUPE - 16.0;
    }
    let rect = Rect::from_min_size(min.round(), vec2(LOUPE, LOUPE));
    let cell = LOUPE / LOUPE_PIXELS;
    let half = (LOUPE_PIXELS / 2.0).floor();
    let (w, h) = (image.width as f32, image.height as f32);
    // Los píxeles de `(x - half, y - half)` a `(x + half, y + half)` (lo de afuera, vacío).
    let from = vec2(x as f32 - half, y as f32 - half);
    let uv = Rect::from_min_size(
        pos2(from.x / w, from.y / h),
        vec2(LOUPE_PIXELS / w, LOUPE_PIXELS / h),
    );
    let clipped = painter.with_clip_rect(rect);
    clipped.rect_filled(rect, 0.0, crate::theme::CANVAS_BG);
    crate::panels::checker(&clipped, rect);
    // La textura no se repite: fuera de ella queda el damero.
    let inside = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let visible = uv.intersect(inside);
    if visible.is_positive() {
        let to_screen = |p: egui::Pos2| {
            rect.min
                + vec2(
                    (p.x - uv.min.x) / uv.width(),
                    (p.y - uv.min.y) / uv.height(),
                ) * LOUPE
        };
        let target = Rect::from_min_max(to_screen(visible.min), to_screen(visible.max));
        clipped.image(texture.id(), target, visible, Color32::WHITE);
    }
    // El píxel del centro.
    let center = Rect::from_min_size(rect.min + vec2(half, half) * cell, vec2(cell, cell));
    let stroke = |c| egui::Stroke::new(1.0, c);
    clipped.rect_stroke(
        center,
        0.0,
        stroke(Color32::BLACK),
        egui::StrokeKind::Outside,
    );
    clipped.rect_stroke(
        center,
        0.0,
        stroke(Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect,
        0.0,
        stroke(pixel_ui::control_border(ui.ctx())),
        egui::StrokeKind::Outside,
    );
    let text = if color.a == 0 {
        format!("{x}, {y} · empty")
    } else {
        format!("{x}, {y} · {}", color.to_hex())
    };
    let label = Rect::from_min_size(rect.left_bottom(), vec2(LOUPE, 16.0));
    painter.rect_filled(label, 0.0, crate::theme::CANVAS_BG);
    painter.text(
        label.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::TextStyle::Body.resolve(ui.style()),
        Color32::WHITE,
    );
}

/// `group` ordenado para juntarlo: el más usado (en `groomed`) primero, que es el que queda.
fn merge_order(mut group: Vec<Rgba>, groomed: &Groomed) -> Vec<Rgba> {
    let count = |c: &Rgba| {
        groomed
            .colors
            .iter()
            .find(|(k, _)| k == c)
            .map_or(0, |(_, n)| *n)
    };
    group.sort_by_key(|c| std::cmp::Reverse(count(c)));
    group
}

/// `groomed` con los colores de cada grupo de `merges` cambiados por el primero del grupo, en
/// orden: un grupo puede juntar un color en el que ya se juntaron otros (A en B y luego B en
/// C deja A en C).
fn merged(groomed: &Groomed, merges: &Merges) -> Groomed {
    let mut image = groomed.image.clone();
    let mut target: std::collections::HashMap<Rgba, Rgba> = std::collections::HashMap::new();
    for group in merges {
        let Some((&to, rest)) = group.split_first() else {
            continue;
        };
        // Lo que ya iba a un color de este grupo, ahora va al primero.
        for value in target.values_mut() {
            if rest.contains(value) {
                *value = to;
            }
        }
        for &c in rest {
            if c != to {
                target.entry(c).or_insert(to);
            }
        }
    }
    if !target.is_empty() {
        for p in image.pixels.iter_mut() {
            if let Some(&to) = target.get(p) {
                *p = to;
            }
        }
    }
    Groomed {
        name: groomed.name.clone(),
        colors: color_counts(&image),
        image,
    }
}

/// Los colores de `image` y cuántos píxeles tiene cada uno, del más usado al menos.
fn color_counts(image: &Bitmap) -> Vec<(Rgba, usize)> {
    let mut counts: std::collections::HashMap<Rgba, usize> = std::collections::HashMap::new();
    for &c in image.pixels.iter().filter(|c| c.a > 0) {
        *counts.entry(c).or_default() += 1;
    }
    let mut colors: Vec<(Rgba, usize)> = counts.into_iter().collect();
    colors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_hex().cmp(&b.0.to_hex())));
    colors
}

/// Los pasos sobre `source`: un color por celda, el fondo, el contorno, los colores y el recorte
/// a lo pintado.
pub fn process(
    name: &str,
    source: &Bitmap,
    settings: Settings,
    primary: Rgba,
    palette: Option<&[Rgba]>,
    merges: &Merges,
) -> Groomed {
    let sample = if settings.follow_edges {
        cs_raster::sample_following_edges
    } else {
        cs_raster::sample_grid
    };
    let (mut pixels, w, h) = sample(&source.pixels, source.width, source.height, settings.grid);
    if settings.background {
        pixels = cs_raster::remove_background(&pixels, w, h, settings.tolerance);
    }
    if settings.clean || settings.ring {
        let ring = settings.ring.then_some(primary);
        if settings.clean {
            pixels = cs_raster::clean_outline(&pixels, w, h, ring);
        } else if let Some(color) = ring {
            // Solo el contorno, sin tocar lo demás.
            let cleaned = cs_raster::clean_outline(&pixels, w, h, Some(color));
            pixels = pixels
                .iter()
                .zip(cleaned)
                .map(|(&p, c)| if p.a == 0 && c == color { c } else { p })
                .collect();
        }
    }
    if settings.reduce {
        pixels = cs_raster::reduce_colors(&pixels, settings.colors as usize).0;
    }
    if let Some(palette) = palette {
        fit_to_palette(&mut pixels, palette);
    }
    let image = trimmed(&pixels, w, h);
    let groomed = Groomed {
        name: name.to_string(),
        colors: color_counts(&image),
        image,
    };
    // Al final, los grupos juntados a mano (el recorte no cambia: solo cambian colores).
    merged(&groomed, merges)
}

/// Cada píxel pintado, el color más parecido de `palette` (sin sus transparentes), con su alfa.
fn fit_to_palette(pixels: &mut [Rgba], palette: &[Rgba]) {
    let palette: Vec<Rgba> = palette.iter().copied().filter(|c| c.a > 0).collect();
    if palette.is_empty() {
        return;
    }
    let mut memo: std::collections::HashMap<Rgba, Rgba> = std::collections::HashMap::new();
    for p in pixels.iter_mut().filter(|p| p.a > 0) {
        let to = *memo.entry(*p).or_insert_with(|| {
            *palette
                .iter()
                .min_by_key(|c| perceived_distance(**c, *p))
                .expect("not empty")
        });
        *p = Rgba { a: p.a, ..to };
    }
}

/// Qué tan distintos se ven dos colores ("redmean": el rojo y el azul pesan según qué tan rojo
/// es el par, más cerca de lo que ve el ojo que la distancia RGB).
fn perceived_distance(a: Rgba, b: Rgba) -> u32 {
    let mean = (a.r as i32 + b.r as i32) / 2;
    let (dr, dg, db) = (
        a.r as i32 - b.r as i32,
        a.g as i32 - b.g as i32,
        a.b as i32 - b.b as i32,
    );
    (((512 + mean) * dr * dr) >> 8) as u32
        + (4 * dg * dg) as u32
        + (((767 - mean) * db * db) >> 8) as u32
}

/// `pixels` recortado a lo pintado (entero si no hay nada).
fn trimmed(pixels: &[Rgba], w: u32, h: u32) -> Bitmap {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (i, c) in pixels.iter().enumerate() {
        if c.a > 0 {
            let (x, y) = (i as u32 % w, i as u32 / w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    if x0 > x1 {
        return Bitmap {
            width: w,
            height: h,
            pixels: pixels.to_vec(),
        };
    }
    let (tw, th) = (x1 - x0 + 1, y1 - y0 + 1);
    Bitmap {
        width: tw,
        height: th,
        pixels: (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .map(|(x, y)| pixels[(y * w + x) as usize])
            .collect(),
    }
}

/// Una imagen de cualquier formato común (PNG, JPEG, GIF, WebP en escritorio; PNG en la web).
pub fn decode_image(bytes: &[u8]) -> Result<Bitmap, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let decoded = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels = rgba
            .pixels()
            .map(|p| Rgba::new(p[0], p[1], p[2], p[3]))
            .collect();
        Ok(Bitmap {
            width,
            height,
            pixels,
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        cs_io::import::decode_png(bytes).map_err(|e| e.to_string())
    }
}

/// La ilustración del panel sin imagen ([`EMPTY_ART`]), su primer frame.
fn empty_art() -> Option<ColorImage> {
    let (sprite, _) = cs_io::aseprite::read(EMPTY_ART).ok()?;
    let pixels: Vec<u8> = cs_render::render_frame(&sprite, 0)
        .iter()
        .flat_map(|c| [c.r, c.g, c.b, c.a])
        .collect();
    Some(ColorImage::from_rgba_unmultiplied(
        [sprite.width as usize, sprite.height as usize],
        &pixels,
    ))
}

/// Dónde va la ilustración de `art` en `area`: centrada a lo ancho y, con la frase de `text_h`
/// debajo, a lo alto (arriba del todo si no cabe), en puntos enteros.
fn empty_art_rect(area: Rect, art: egui::Vec2, text_h: f32) -> Rect {
    let total = art.y + EMPTY_GAP + text_h;
    let top = area.top() + ((area.height() - total) / 2.0).max(0.0);
    Rect::from_min_size(pos2(area.center().x - art.x / 2.0, top).round(), art)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_art_is_the_users_monkeys_at_1x() {
        let art = empty_art().expect("the embedded art");
        assert_eq!(art.size, [117, 68]);
        assert!(art.pixels.iter().any(|c| c.a() == 255));
    }

    #[test]
    fn the_empty_art_and_its_text_are_centered_in_the_empty_panel() {
        let area = Rect::from_min_size(pos2(10.0, 30.0), vec2(REFERENCE_WIDTH, 300.0));
        let art = empty_art_rect(area, vec2(117.0, 68.0), 20.0);
        assert_eq!(art.size(), vec2(117.0, 68.0));
        // Centrada a lo ancho (en puntos enteros) y, con la frase de 20 debajo, a lo alto.
        assert_eq!(art.left(), (10.0 + (REFERENCE_WIDTH - 117.0) / 2.0).round());
        let total = 68.0 + EMPTY_GAP + 20.0;
        assert_eq!(art.top(), (30.0 + (300.0 - total) / 2.0).round());
        // Si no cabe, arriba del todo.
        let short = Rect::from_min_size(pos2(0.0, 0.0), vec2(REFERENCE_WIDTH, 50.0));
        assert_eq!(empty_art_rect(short, vec2(117.0, 68.0), 20.0).top(), 0.0);
    }

    #[test]
    fn without_an_image_the_panel_shows_the_monkeys_over_the_text() {
        let ctx = egui::Context::default();
        let mut panel = GroomPanel::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                panel.ui(ui, Rgba::rgb(0, 0, 0), &[]);
            });
        });
        output.textures_delta.clear();
        let mut art = None;
        let mut text = None;
        for c in &output.shapes {
            match &c.shape {
                egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default() => {
                    let r =
                        Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>());
                    if r.size() == vec2(117.0, 68.0) {
                        art = Some(r);
                    }
                }
                egui::Shape::Text(t) if t.galley.text().starts_with("Bring an image") => {
                    text = Some(t.visual_bounding_rect());
                }
                _ => {}
            }
        }
        let (art, text) = (art.expect("the monkeys"), text.expect("the text"));
        assert!(
            text.top() >= art.bottom() + EMPTY_GAP - 1.0,
            "{art:?} {text:?}"
        );
        assert!(
            (text.center().x - art.center().x).abs() <= 1.0,
            "{art:?} {text:?}"
        );
    }

    /// De `MIN_WIDTH` para arriba, con imagen o sin ella, todo entra en el ancho del panel, en
    /// puntos enteros y sin que nada tape a nada. Con imagen, las dos columnas van lado a lado
    /// desde `SIDE_BY_SIDE` (a 400, la de los pasos en su mínimo) y, más angosto, la de los
    /// colores baja debajo de la de los pasos.
    #[test]
    fn the_groom_panel_reflows_at_any_width() {
        use crate::forms::testing::run_at;
        for width in [MIN_WIDTH, 300.0, 400.0, REFERENCE_WIDTH, 600.0] {
            for with_image in [false, true] {
                let mut panel = GroomPanel::default();
                if with_image {
                    panel.load("big".into(), big());
                }
                let probe = run_at(width, |ui| {
                    panel.ui(ui, Rgba::rgb(0, 0, 0), &[]);
                });
                probe.assert_whole_px();
                probe.assert_within(width);
                probe.assert_no_overlap();
                // El encabezado en una fila desde 282 (logo, aire y botones); más angosto, los
                // botones bajan debajo del logo.
                let import = probe.rect("Import Image…");
                assert_eq!(import.top() == 0.0, width >= 300.0, "{import:?} at {width}");
                if !with_image {
                    continue;
                }
                // Lo último de los pasos (la paleta) y lo primero de los colores (su orden).
                let (palette, sort) = (probe.rect("Original Colors"), probe.rect("Most Used"));
                if width < SIDE_BY_SIDE {
                    assert!(
                        sort.top() > palette.bottom(),
                        "one under the other at {width}: {palette:?} {sort:?}"
                    );
                } else {
                    assert!(
                        sort.left() > palette.right() && sort.top() < palette.top(),
                        "side by side at {width}: {palette:?} {sort:?}"
                    );
                    // La paleta llega al borde de la columna de los pasos: en su mínimo a 400.
                    if width == 400.0 {
                        assert_eq!(palette.right(), LEFT_MIN, "at {width}");
                    }
                }
                probe.rect("New Sprite");
            }
        }
    }

    /// Lo que pinta `panel` en `content` (con el skin y las fuentes de la app, escala 1) visto
    /// por un scroll que muestra `viewport` (más abajo que `content`: desplazado): sus shapes,
    /// cada uno con su recorte.
    fn painted(
        panel: &mut GroomPanel,
        content: Rect,
        viewport: Rect,
    ) -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        let skin = crate::skin::Skin::embedded(&ctx);
        ctx.set_fonts(crate::appearance::font_definitions(true));
        crate::appearance::apply_style(&ctx, Some(skin.colors()));
        crate::pixel_ui::set_bubble(&ctx, Some(std::sync::Arc::new(skin)));
        let mut shapes = Vec::new();
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
                    panel.ui(ui, Rgba::rgb(0, 0, 0), &[]);
                });
            });
            output.textures_delta.clear();
            shapes = output.shapes;
        }
        shapes
    }

    /// El rect de una malla.
    fn mesh_rect(shape: &egui::Shape) -> Option<Rect> {
        match shape {
            egui::Shape::Mesh(m) => {
                let points: Vec<_> = m.vertices.iter().map(|v| v.pos).collect();
                Some(Rect::from_points(&points))
            }
            _ => None,
        }
    }

    /// Con el contenido desplazado `scrolled` hacia arriba: el rect del logo, su recorte y lo que
    /// se ve del scroll.
    fn logo_clip(scrolled: f32) -> (Rect, Rect, Rect) {
        let skin = crate::skin::Skin::embedded(&egui::Context::default());
        let piece = skin.piece("groom.logo", crate::skin::WidgetState::Normal);
        let size = piece.map(|p| vec2(p.size[0] as f32, p.size[1] as f32));
        let size = size.expect("the logo in the skin");
        let content = Rect::from_min_size(pos2(10.0, 10.0), vec2(REFERENCE_WIDTH, 500.0));
        let viewport = content.with_min_y(content.min.y + scrolled);
        let logo = Rect::from_min_size(content.min + LOGO_AT, size);
        let shapes = painted(&mut GroomPanel::default(), content, viewport);
        let clip = shapes
            .iter()
            .find(|c| mesh_rect(&c.shape) == Some(logo))
            .map(|c| c.clip_rect)
            .expect("the logo");
        (logo, clip, viewport)
    }

    /// El logo sube 1 por encima del contenido (sobre el margen del panel): el recorte del scroll
    /// del dock, que empieza en el contenido, no le quita su fila de arriba.
    #[test]
    fn the_logo_keeps_its_top_row_above_the_scroll_clip() {
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

    /// La línea grabada entre las columnas: una, a 4 de la de los pasos, solo si van lado a lado.
    #[test]
    fn the_divider_is_drawn_only_side_by_side() {
        for width in [MIN_WIDTH, 300.0, 400.0, REFERENCE_WIDTH, 600.0] {
            let mut panel = GroomPanel::default();
            panel.load("big".into(), big());
            let content = Rect::from_min_size(pos2(10.0, 10.0), vec2(width, 600.0));
            let shapes = painted(&mut panel, content, content);
            // Las líneas grabadas de pie: mallas sin textura de 2 de ancho (oscura y clara).
            fn plain_meshes(shape: &egui::Shape, out: &mut Vec<Rect>) {
                match shape {
                    egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| plain_meshes(s, out)),
                    egui::Shape::Mesh(m) if m.texture_id == egui::TextureId::default() => {
                        out.extend(mesh_rect(shape));
                    }
                    _ => {}
                }
            }
            let mut meshes = Vec::new();
            shapes
                .iter()
                .for_each(|c| plain_meshes(&c.shape, &mut meshes));
            let dividers: Vec<Rect> = meshes
                .into_iter()
                .filter(|r| r.width() == 2.0 && r.height() > 20.0)
                .collect();
            if width >= SIDE_BY_SIDE {
                assert_eq!(dividers.len(), 1, "one at {width}: {dividers:?}");
                // La columna de los pasos: en su mínimo a 400, de su ancho en la referencia y,
                // a 600, con la mitad de lo que sobra.
                let left = match width as u32 {
                    400 => LEFT_MIN,
                    434 => LEFT_WIDTH,
                    _ => LEFT_WIDTH + (width - REFERENCE_WIDTH) / 2.0,
                };
                let x = dividers[0].left() - content.left();
                assert_eq!(x, left + DIVIDER_AIR, "at {width}");
            } else {
                assert!(dividers.is_empty(), "none at {width}: {dividers:?}");
            }
        }
    }

    /// 4×3 celdas de 5 px (rojo sobre blanco), agrandado.
    fn big() -> Bitmap {
        let (cw, ch, k) = (4u32, 3u32, 5u32);
        let white = Rgba::rgb(255, 255, 255);
        let red = Rgba::rgb(200, 30, 30);
        let pixels = (0..ch * k)
            .flat_map(|y| (0..cw * k).map(move |x| (x, y)))
            .map(|(x, y)| {
                let (cx, cy) = (x / k, y / k);
                if (1..3).contains(&cx) && cy == 1 {
                    red
                } else {
                    white
                }
            })
            .collect();
        Bitmap {
            width: cw * k,
            height: ch * k,
            pixels,
        }
    }

    #[test]
    fn the_steps_give_the_trimmed_pixels() {
        let source = big();
        let grid = GridGuess {
            cell: (5.0, 5.0),
            offset: (0.0, 0.0),
        };
        let groomed = process(
            "x",
            &source,
            Settings::new(grid),
            Rgba::rgb(0, 0, 0),
            None,
            &Vec::new(),
        );
        assert_eq!(
            (groomed.image.width, groomed.image.height),
            (2, 1),
            "only the red"
        );
        assert_eq!(groomed.colors.len(), 1);
        // Con contorno: 4×3.
        let mut settings = Settings::new(grid);
        settings.ring = true;
        let ringed = process(
            "x",
            &source,
            settings,
            Rgba::rgb(0, 0, 0),
            None,
            &Vec::new(),
        );
        assert_eq!((ringed.image.width, ringed.image.height), (4, 3));
        assert_eq!(ringed.colors.len(), 2);
        // Juntar el contorno negro con el rojo: queda un color, el primero del grupo.
        let red = Rgba::rgb(200, 30, 30);
        let merged = process(
            "x",
            &source,
            settings,
            Rgba::rgb(0, 0, 0),
            None,
            &vec![vec![red, Rgba::rgb(0, 0, 0)]],
        );
        assert_eq!(merged.colors, vec![(red, 8)], "2 red + 6 of outline");
    }

    #[test]
    fn the_merge_being_tried_is_the_hovered_suggestion_or_the_picked_colors() {
        let mut panel = GroomPanel::default();
        panel.load("x".into(), big());
        panel.settings.as_mut().unwrap().ring = true;
        let groomed = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap().clone();
        let (red, black) = (Rgba::rgb(200, 30, 30), Rgba::rgb(0, 0, 0));
        assert_eq!(panel.trying(&groomed), None);
        // Elegidos: el más usado (el contorno negro, 6 contra 2) queda primero.
        panel.picked = vec![red, black];
        assert_eq!(panel.trying(&groomed), Some(vec![black, red]));
        // La sugerencia bajo el mouse manda.
        panel.hovered_group = Some(vec![red, black]);
        assert_eq!(panel.trying(&groomed), Some(vec![red, black]));
        // Juntar no rehace los pasos: solo cambia colores sobre lo de antes.
        panel.merges.push(vec![black, red]);
        let merged = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap();
        assert_eq!(merged.colors, vec![(black, 8)]);
        assert_eq!(
            panel.base.as_ref().unwrap().3,
            groomed,
            "the steps are kept"
        );
    }

    #[test]
    fn the_colors_sort_by_hue_brightness_or_saturation() {
        let (red, dark_red) = (Rgba::rgb(220, 30, 30), Rgba::rgb(90, 10, 10));
        let (blue, gray) = (Rgba::rgb(40, 60, 220), Rgba::rgb(128, 128, 128));
        let (white, black) = (Rgba::rgb(255, 255, 255), Rgba::rgb(0, 0, 0));
        let colors = [
            (gray, 9),
            (blue, 8),
            (white, 7),
            (red, 6),
            (black, 5),
            (dark_red, 4),
        ];
        let order = |sort| -> Vec<Rgba> {
            sorted_colors(&colors, sort)
                .into_iter()
                .map(|(c, _)| c)
                .collect()
        };
        assert_eq!(order(ColorSort::MostUsed), colors.map(|(c, _)| c).to_vec());
        // Los rojos (de oscuro a claro), el azul y al final los grises, de oscuro a claro.
        assert_eq!(
            order(ColorSort::Hue),
            vec![dark_red, red, blue, black, gray, white]
        );
        assert_eq!(
            order(ColorSort::Brightness),
            vec![black, dark_red, blue, red, gray, white]
        );
        // Los más saturados primero; los grises (sin saturación) al final, por uso.
        assert_eq!(order(ColorSort::Saturation)[3..], [gray, white, black]);
    }

    #[test]
    fn fitting_to_a_palette_takes_the_most_similar_color() {
        let source = big();
        let grid = GridGuess {
            cell: (5.0, 5.0),
            offset: (0.0, 0.0),
        };
        let (wine, sky, clear) = (
            Rgba::rgb(150, 20, 40),
            Rgba::rgb(100, 180, 250),
            Rgba::TRANSPARENT,
        );
        let palette = [sky, clear, wine];
        let settings = Settings::new(grid);
        let fitted = process(
            "x",
            &source,
            settings,
            Rgba::rgb(0, 0, 0),
            Some(&palette),
            &Vec::new(),
        );
        assert_eq!(fitted.colors, vec![(wine, 2)], "the red goes to the wine");
        // Sin colores (solo transparentes), no cambia nada.
        let none = process(
            "x",
            &source,
            settings,
            Rgba::rgb(0, 0, 0),
            Some(&[clear]),
            &Vec::new(),
        );
        assert_eq!(none.colors, vec![(Rgba::rgb(200, 30, 30), 2)]);
    }

    #[test]
    fn a_merge_can_take_a_color_that_others_were_merged_into() {
        let (a, b, c) = (
            Rgba::rgb(10, 0, 0),
            Rgba::rgb(20, 0, 0),
            Rgba::rgb(30, 0, 0),
        );
        let image = Bitmap {
            width: 6,
            height: 1,
            pixels: vec![a, b, b, c, c, c],
        };
        let groomed = Groomed {
            name: "x".into(),
            colors: color_counts(&image),
            image,
        };
        // A en B y luego (con los colores de ahora) B en C: todo queda en C.
        let merges = vec![vec![b, a], vec![c, b]];
        assert_eq!(merged(&groomed, &merges).colors, vec![(c, 6)]);
    }

    #[test]
    fn the_result_is_only_recomputed_when_something_changes() {
        let mut panel = GroomPanel::default();
        panel.load("x".into(), big());
        let first = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap().clone();
        let again = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap().clone();
        assert_eq!(first, again);
        panel.settings.as_mut().unwrap().ring = true;
        let ringed = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap();
        assert_ne!(ringed.image.width, first.image.width);
    }
}
