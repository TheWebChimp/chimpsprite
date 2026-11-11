//! Fuentes pixel de dafont.com (solo escritorio: el sitio no acepta peticiones desde otras
//! páginas). El panel del dock lista la categoría Bitmap con la vista previa, el autor, el tamaño
//! recomendado y la licencia de cada fuente; "Use" la descarga (el zip de dafont), saca el
//! `.ttf`/`.otf` y la deja en la herramienta de texto, a su tamaño y sin antialias.
//!
//! Las licencias son de cada autor (muchas solo para uso personal): el panel las muestra y
//! recuerda revisarlas.

use std::collections::HashMap;
use std::io::Read as _;

use eframe::egui::{self, Color32, Vec2, vec2};
use eframe::emath::GuiRounding as _;

use crate::forms::{self, TuiBuilderLogic as _};
use crate::lospec_client::{self, FetchError, Pending};
use crate::pixel_ui;

const SITE: &str = "https://www.dafont.com";
/// Lo que se acepta de un zip y de una fuente (de sobra para una fuente pixel).
const MAX_ZIP_BYTES: usize = 8 * 1024 * 1024;
const MAX_FONT_BYTES: u64 = 4 * 1024 * 1024;
/// Ancho de referencia del panel (el del diseño: las tarjetas de 350 y la barra de scroll); el
/// dock lo abre de este ancho. Adentro todo sale del ancho disponible.
pub const REFERENCE_WIDTH: f32 = 364.0;
/// El ancho más chico en que el panel se ve bien (medido): el campo de la muestra (`SAMPLE_W`,
/// 158) y, a su derecha, el lugar del logo (`logo_room`, 50 con el logo del skin), con "Sort by" y
/// Submit debajo; en las tarjetas, las descargas en una línea y los botones debajo. Son 208, más
/// 2: con la fila de los campos justo del ancho del campo, taffy 0.9 la mide como si cada campo
/// fuera en su fila (y la deja más alta).
pub const MIN_WIDTH: f32 = 210.0;
/// Lo que queda debajo de la lista hasta el final de la pantalla (el borde del panel y la barra
/// de estado).
const LIST_BELOW: f32 = 40.0;

/// Una fuente del listado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DafontFont {
    pub name: String,
    /// El nombre en las direcciones de dafont (`minecraft` en `minecraft.font`).
    pub slug: String,
    pub author: String,
    /// El tamaño al que se ve nítida ("Use at 16 px"), si la página lo dice.
    pub size_px: Option<u32>,
    /// "100% Free", "Free for personal use", "Public domain / GPL / OFL"…
    pub license: String,
    /// "5,204,307 downloads (5,642 yesterday)", como lo dice la página.
    pub downloads: String,
    pub preview_url: Option<String>,
    /// El botón Download del listado; no siempre es el `slug` (`vcr-osd-mono` baja con
    /// `f=vcr_osd_mono`).
    pub download_url: Option<String>,
}

impl DafontFont {
    pub fn page_url(&self) -> String {
        format!("{SITE}/{}.font", self.slug)
    }

    fn download_url(&self) -> String {
        self.download_url.clone().unwrap_or_else(|| {
            format!(
                "https://dl.dafont.com/dl/?f={}",
                self.slug.replace('-', "_")
            )
        })
    }
}

/// La página `page` (desde 1) de la categoría Bitmap en el orden `sort`; con `sample`, las
/// vistas previas muestran ese texto (la "Custom preview" de dafont).
fn listing_url(page: u32, sample: &str, sort: Sort) -> String {
    let mut query = Vec::new();
    if !sample.trim().is_empty() {
        query.push(format!("text={}", encode(sample.trim())));
    }
    // Por popularidad es lo de siempre.
    if sort != Sort::Popularity {
        query.push(format!("sort={}", sort.param()));
    }
    if page > 1 {
        query.push(format!("page={page}"));
    }
    if query.is_empty() {
        format!("{SITE}/bitmap.php")
    } else {
        format!("{SITE}/bitmap.php?{}", query.join("&"))
    }
}

/// `text` para una dirección: letras y números tal cual, los espacios como `+` y lo demás en
/// `%XX` (UTF-8).
fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// El texto entre `start` y `end` a partir de `from` (y dónde termina).
fn between<'a>(text: &'a str, from: usize, start: &str, end: &str) -> Option<(&'a str, usize)> {
    let a = text[from..].find(start)? + from + start.len();
    let b = text[a..].find(end)? + a;
    Some((&text[a..b], b + end.len()))
}

/// El texto de una página de dafont: viene en Latin-1 (si no es UTF-8 válido, cada byte es su
/// carácter).
fn decode_html(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Las entidades HTML que aparecen en los nombres.
fn unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

/// Las fuentes de una página del listado de dafont. Cada una empieza en su `lv1left`; lo que no
/// se entiende se salta.
pub fn parse_listing(html: &str) -> Vec<DafontFont> {
    const MARK: &str = "<div class=\"lv1left dfbg\">";
    let starts: Vec<usize> = html.match_indices(MARK).map(|(i, _)| i).collect();
    starts
        .iter()
        .enumerate()
        .filter_map(|(k, &start)| {
            let end = starts.get(k + 1).copied().unwrap_or(html.len());
            let block = &html[start..end];
            // Con texto propio, los enlaces llevan `?text=…` (`minecraft.font?text=Hola+mono`).
            let (href, _) = between(block, 0, "<a href=\"", "\"")?;
            let slug = href.split('?').next()?.strip_suffix(".font")?;
            let (name, _) = between(block, 0, "<strong>", "</strong>")?;
            let author = block
                .find(" by <a href=\"")
                .and_then(|i| between(block, i, "\">", "</a>"))
                .map(|(a, _)| unescape(a))
                .unwrap_or_default();
            let size_px = between(block, 0, "title=\"Use at ", " px")
                .and_then(|(n, _)| n.trim().parse().ok());
            let license = between(block, 0, "faq.php#copyright\">", "</a>")
                .map(|(l, _)| unescape(l))
                .unwrap_or_default();
            let downloads = between(block, 0, "<span class=\"light\">", "</span>")
                .map(|(d, _)| unescape(d))
                .filter(|d| d.contains("download"))
                .unwrap_or_default();
            // Las de siempre (`/img/preview/…`) o, con texto propio, las de `//img.dafont.com`.
            let preview_url = between(block, 0, "background-image:url(", ")").map(|(p, _)| {
                if p.starts_with("//") {
                    format!("https:{}", unescape(p))
                } else {
                    format!("{SITE}{}", unescape(p))
                }
            });
            let download_url = between(block, 0, "class=\"dl\"", "</a>")
                .and_then(|(link, _)| between(link, 0, "href=\"", "\""))
                .map(|(href, _)| unescape(href))
                .filter(|href| href.starts_with("//dl.dafont.com/"))
                .map(|href| format!("https:{href}"));
            (!slug.contains('"') && !slug.contains('/')).then(|| DafontFont {
                name: unescape(name),
                slug: slug.to_string(),
                author,
                size_px,
                license,
                downloads,
                preview_url,
                download_url,
            })
        })
        .collect()
}

/// La primera fuente (`.ttf` u `.otf`) de un zip de dafont: su nombre (sin extensión) y sus bytes.
pub fn extract_font(zip: &[u8]) -> Option<(String, Vec<u8>)> {
    if zip.len() > MAX_ZIP_BYTES {
        return None;
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).ok()?;
    let names: Vec<String> = archive
        .file_names()
        .filter_map(Result::ok)
        .map(|n| n.into_owned())
        .collect();
    // Primero los .ttf (los .otf de pixel suelen ser la misma fuente).
    let pick = |ext: &str| {
        names
            .iter()
            .find(|n| n.to_ascii_lowercase().ends_with(ext) && !n.contains("__MACOSX"))
            .cloned()
    };
    let name = pick(".ttf").or_else(|| pick(".otf"))?;
    let mut file = archive.by_name(&name).ok()?;
    if file.size() > MAX_FONT_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let stem = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&name)
        .rsplit_once('.')
        .map_or(name.as_str(), |(s, _)| s)
        .to_string();
    Some((stem, bytes))
}

/// Abre el panel (desde la barra de opciones de la herramienta de texto).
pub fn request_open(ctx: &egui::Context) {
    crate::dock::set_open(ctx, Some(crate::dock::DockPanel::Dafont));
}

/// Una fuente sacada del zip: su nombre y sus bytes.
type FontFile = (String, Vec<u8>);

/// Las vistas previas se piden de a estas (solo las que se ven).
const MAX_IN_FLIGHT: usize = 4;
/// Una petición que no respondió en este tiempo se da por perdida.
const TIMEOUT_SECS: f64 = 20.0;

/// Los colores del diseño (frame 7): la barra de cada fuente, su etiqueta del tamaño, las
/// tarjetas blancas con su sombra, la cuadrícula de caracteres y la etiqueta "Preview".
const RED: Color32 = Color32::from_rgb(204, 0, 0);
const BADGE: Color32 = Color32::from_rgb(70, 71, 98);
const CARD: Color32 = Color32::WHITE;
const GRID: Color32 = Color32::from_rgb(105, 102, 130);
const LABEL_DIM: Color32 = Color32::from_rgb(197, 199, 221);
/// La barra roja: 13 de alto, las mayúsculas del nombre 3 filas abajo y las de lo chico 4; 4 px
/// de aire a los lados y 2 filas hasta su tarjeta.
const HEADER_H: f32 = 13.0;
const HEADER_PAD: f32 = 4.0;
const HEADER_GAP: f32 = 2.0;
/// La etiqueta del tamaño: 2 px de cada lado del texto y 1 arriba y abajo de sus mayúsculas.
const BADGE_PAD: f32 = 2.0;
/// Lo de adentro de las tarjetas del detalle.
const CARD_PAD: f32 = 4.0;
/// El aire a los lados de la vista previa en su tarjeta.
const IMAGE_PAD: f32 = 5.0;
/// Entre el contenido y la barra de scroll.
const SCROLL_GAP: f32 = 5.0;
/// Del borde del panel a su contenido, y lo que ocupan su línea y su luz.
const PANEL_MARGIN: f32 = crate::dock::FLYOUT_MARGIN as f32;
const PANEL_EDGE: f32 = 2.0;
/// Una celda de la cuadrícula: al menos 26 de ancho (con su línea), 5 px de aire, el carácter y
/// su código arriba y el glifo debajo.
const CELL_W: f32 = 26.0;
const CELL_PAD: f32 = 5.0;
const CELL_CODE_TOP: f32 = 15.0;
const CELL_GLYPH_TOP: f32 = 22.0;
/// Los campos de arriba: el de la muestra y el del orden (en la lista), como en el diseño, de 17
/// de alto como sus botones.
const SAMPLE_W: f32 = 158.0;
const SORT_W: f32 = 89.0;
const FIELD_H: f32 = crate::pixel_ui::CONTROL_HEIGHT;
/// Entre los campos de arriba (y entre sus filas, si se parten) y entre los botones de una
/// tarjeta.
const FIELD_GAP: f32 = 5.0;
/// Entre un error y Retry (lo que había entre los elementos de una fila de egui).
const ROW_GAP: f32 = 8.0;
/// En una fila de 18 (la de `ui.horizontal`), los botones de 17 van 1 abajo y los textos de 14, 2.
const ROW_H: f32 = 18.0;
const BUTTON_DROP: f32 = 1.0;
const TEXT_DROP: f32 = 2.0;
/// La frase y el abecedario del detalle.
const PANGRAM: &str = "the quick brown fox jumps over the lazy dog";
const CHARSET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789";
/// El logo de dafont (un enlace al sitio), en la esquina de arriba a la derecha del panel, contra
/// su luz: sale del contenido `LOGO_OVERHANG` hacia arriba y hacia la derecha.
const LOGO: &str = "dafont.logo";
const LOGO_OVERHANG: f32 = PANEL_MARGIN - PANEL_EDGE;
/// Entre los campos de arriba y el logo.
const LOGO_AIR: f32 = 16.0;

/// Lo que los campos de arriba le dejan al logo de `logo_w` a la derecha del contenido: lo que
/// entra del logo y su aire.
fn logo_room(logo_w: f32) -> f32 {
    logo_w - LOGO_OVERHANG + LOGO_AIR
}

/// El orden de la lista ("Sort by" de dafont).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    Name,
    #[default]
    Popularity,
    Newest,
}

impl Sort {
    const ALL: [Sort; 3] = [Sort::Name, Sort::Popularity, Sort::Newest];

    fn label(self) -> &'static str {
        match self {
            Sort::Name => "Name",
            Sort::Popularity => "Popularity",
            Sort::Newest => "Newest",
        }
    }

    /// El valor de `sort=` en las direcciones de dafont.
    fn param(self) -> &'static str {
        match self {
            Sort::Name => "alpha",
            Sort::Popularity => "pop",
            Sort::Newest => "date",
        }
    }
}

enum Preview {
    /// La petición y cuándo se hizo.
    Loading(Pending<egui::ColorImage>, f64),
    Ready(egui::TextureHandle),
    Failed,
}

/// Para qué se baja una fuente: para usarla ya o para verla en el detalle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    Use,
    View,
}

/// El detalle de una fuente ("View more"): la fuente bajada y lo dibujado con ella.
struct Detail {
    font: DafontFont,
    loaded: Option<cs_tools::text::FontChoice>,
    /// La muestra (con el texto con que se dibujó), el abecedario chico y grande, y los
    /// caracteres de la fuente con sus glifos (se dibujan al verse).
    sample: Option<(String, Option<egui::TextureHandle>)>,
    charset: Option<[Option<egui::TextureHandle>; 2]>,
    chars: Vec<char>,
    glyphs: HashMap<char, Option<egui::TextureHandle>>,
}

impl Detail {
    fn new(font: DafontFont) -> Self {
        Self {
            font,
            loaded: None,
            sample: None,
            charset: None,
            chars: Vec::new(),
            glyphs: HashMap::new(),
        }
    }

    /// El tamaño al que se dibuja: el de la página o 16.
    fn px(&self) -> u32 {
        self.font.size_px.unwrap_or(16).clamp(4, 64)
    }
}

/// El panel de Dafont.
#[derive(Default)]
pub struct DafontPanel {
    fonts: Vec<DafontFont>,
    /// La última página pedida y su petición en curso.
    page: u32,
    listing: Option<Pending<Vec<DafontFont>>>,
    /// La lista no llegó: se puede volver a pedir la página.
    listing_failed: bool,
    /// La fuente que se está bajando y para qué.
    download: Option<(DafontFont, Pending<FontFile>, Purpose)>,
    /// Cuándo se pidió la lista o la descarga en curso.
    listing_since: f64,
    download_since: f64,
    previews: HashMap<String, Preview>,
    /// El último problema, para mostrarlo.
    error: Option<String>,
    /// No hay más páginas (la última vino vacía).
    end: bool,
    /// El texto para probar las fuentes (lo que se escribe) y el que muestra la lista.
    sample: String,
    shown_sample: String,
    sort: Sort,
    /// "View more": el detalle de una fuente en lugar de la lista.
    detail: Option<Detail>,
}

/// Lo que eligió el usuario: la fuente bajada, lista para la herramienta de texto.
pub struct Picked {
    pub font: cs_tools::text::FontChoice,
    pub size_px: Option<u32>,
}

/// Lo que se pidió en un frame del panel.
#[derive(Default)]
struct Asked {
    submit: bool,
    sort: bool,
    more: bool,
    retry: bool,
    back: bool,
    use_font: Option<DafontFont>,
    view: Option<DafontFont>,
    visible: Vec<usize>,
}

fn describe(e: &FetchError) -> String {
    match e {
        FetchError::Network => "Couldn't reach dafont.com.".into(),
        FetchError::Status(code) => format!("dafont.com returned an error ({code})."),
        _ => "dafont.com sent something unexpected.".into(),
    }
}

impl DafontPanel {
    fn load_page(&mut self, ctx: &egui::Context, page: u32) {
        self.page = page;
        self.error = None;
        self.listing_failed = false;
        self.listing_since = ctx.input(|i| i.time);
        let url = listing_url(page, &self.shown_sample, self.sort);
        self.listing = Some(lospec_client::fetch(ctx, url, |status, bytes| {
            if status != 200 {
                return Err(FetchError::Status(status));
            }
            Ok(parse_listing(&decode_html(bytes)))
        }));
    }

    /// La lista otra vez desde la primera página (otro texto u otro orden).
    fn reload(&mut self, ctx: &egui::Context) {
        self.fonts.clear();
        self.previews.clear();
        self.end = false;
        self.load_page(ctx, 1);
    }

    fn download(&mut self, ctx: &egui::Context, font: DafontFont, purpose: Purpose) {
        let pending = lospec_client::fetch(ctx, font.download_url(), |status, bytes| {
            if status != 200 {
                return Err(FetchError::Status(status));
            }
            extract_font(bytes).ok_or(FetchError::Invalid)
        });
        self.error = None;
        self.download_since = ctx.input(|i| i.time);
        self.download = Some((font, pending, purpose));
    }

    /// Desarrollo: el detalle de `font` (ya cargada) con datos de ejemplo, para capturas.
    pub fn demo_detail(&mut self, font: cs_tools::text::FontChoice) {
        let mut detail = Detail::new(DafontFont {
            name: font.name(),
            slug: "demo".into(),
            author: "ChimpSprite".into(),
            size_px: Some(16),
            license: "100% Free".into(),
            downloads: String::new(),
            preview_url: None,
            download_url: None,
        });
        detail.chars = font.chars();
        detail.loaded = Some(font);
        self.detail = Some(detail);
    }

    /// Para los tests: la lista de dos fuentes de `tests::LISTING` ya llegada y sus vistas previas
    /// sin pedir (sin red: lo que se dibuja no depende de cuándo responde dafont).
    #[cfg(test)]
    pub(crate) fn offline() -> Self {
        let mut panel = DafontPanel {
            fonts: parse_listing(tests::LISTING),
            ..Default::default()
        };
        for font in &panel.fonts {
            panel.previews.insert(font.slug.clone(), Preview::Failed);
        }
        panel
    }

    fn downloading(&self, slug: &str) -> bool {
        self.download
            .as_ref()
            .is_some_and(|(f, _, _)| f.slug == slug)
    }

    /// El panel (flotando junto a su botón del dock): la lista de fuentes o el detalle de una.
    /// Devuelve la fuente elegida cuando termina de bajar; entonces el panel se cierra.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Picked> {
        let ctx = &ui.ctx().clone();
        // Los textos chicos (chimpfont) llegan con las fuentes de la app, un frame después de
        // arrancar: hasta entonces, nada.
        let small = crate::appearance::small_family();
        if !ctx.fonts(|f| f.families().contains(&small)) {
            ctx.request_repaint();
            return None;
        }
        // Solo la primera vez: si la lista vino vacía (`end`), no se vuelve a pedir en cada frame.
        if self.fonts.is_empty() && self.listing.is_none() && self.error.is_none() && !self.end {
            self.load_page(ctx, 1);
        }
        let now = ctx.input(|i| i.time);
        let mut picked = self.poll(ctx, now);
        // Las respuestas piden su frame; este es para darse cuenta de las que no llegan.
        if self.listing.is_some() || self.download.is_some() || self.in_flight() > 0 {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        let mut asked = Asked::default();
        self.top_row(ui, &mut asked);
        if let Some(error) = &self.error {
            // El aviso (en líneas, si no cabe) y Retry a su lado, o debajo si no cabe. El aviso
            // mide lo que su texto a lo ancho del panel.
            let retry = self.listing_failed && self.detail.is_none();
            let color = Color32::from_rgb(0xc0, 0x40, 0x40);
            let font = egui::TextStyle::Body.resolve(ui.style());
            let width = ui.available_width().floor();
            let size = ui
                .painter()
                .layout(error.clone(), font, color, width)
                .size();
            forms::form(ui, "dafont_error", 0.0, |tui| {
                tui.style(forms::min_height(ROW_H, forms::row(ROW_GAP)))
                    .add(|tui| {
                        let text = forms::fixed(size.x.ceil(), size.y.ceil());
                        tui.style(forms::air_above(TEXT_DROP, text)).ui(|ui| {
                            ui.colored_label(color, error);
                        });
                        if retry {
                            let button = forms::air_above(BUTTON_DROP, forms::natural());
                            tui.style(button).ui(|ui| {
                                asked.retry = pixel_ui::text_button(ui, "Retry").clicked();
                            });
                        }
                    });
            });
        }
        // Hasta abajo de la pantalla: se ven varias fuentes (o caracteres) a la vez.
        let list_h = |ui: &egui::Ui| {
            (ctx.content_rect().bottom() - ui.cursor().top() - LIST_BELOW).max(120.0)
        };
        let downloading = self.download.as_ref().map(|(f, _, _)| f.slug.clone());
        // En el detalle la muestra sigue al campo mientras se escribe.
        let live_sample = self.sample.clone();
        if let Some(detail) = &mut self.detail {
            detail_ui(
                ui,
                detail,
                &live_sample,
                downloading.as_deref(),
                list_h,
                &mut asked,
            );
        } else {
            let height = list_h(ui);
            pixel_ui::scroll(
                ui,
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, false]),
                |ui| {
                    ui.set_max_width(ui.available_width() - SCROLL_GAP);
                    for (k, font) in self.fonts.iter().enumerate() {
                        if self.font_card(ui, k, font, &mut asked) {
                            asked.visible.push(k);
                        }
                        ui.add_space(6.0);
                    }
                    if self.listing.is_some() {
                        ui.weak("Loading");
                    } else if self.end && self.fonts.is_empty() {
                        ui.weak("No fonts found");
                    } else if !self.end && !self.fonts.is_empty() {
                        asked.more = pixel_ui::text_button(ui, "More fonts").clicked();
                    }
                },
            );
        }
        picked = picked.or_else(|| self.answer(ctx, asked, now));
        if picked.is_some() {
            crate::dock::set_open(ctx, None);
        }
        picked
    }

    /// Lo que llegó (o lo que tardó demasiado: dafont no respondió). Devuelve la fuente bajada
    /// para usarla.
    fn poll(&mut self, ctx: &egui::Context, now: f64) -> Option<Picked> {
        let late = |since: f64| now - since > TIMEOUT_SECS;
        let listing = match self.listing.as_ref().map(Pending::poll) {
            Some(None) if late(self.listing_since) => Some(Err(FetchError::Network)),
            Some(result) => result,
            None => None,
        };
        if let Some(result) = listing {
            self.listing = None;
            match result {
                Ok(fonts) if fonts.is_empty() => self.end = true,
                Ok(fonts) => self.fonts.extend(fonts),
                Err(e) => {
                    self.error = Some(describe(&e));
                    self.listing_failed = true;
                }
            }
        }
        let mut picked = None;
        let download = match self.download.as_ref().map(|(_, p, _)| p.poll()) {
            Some(None) if late(self.download_since) => Some(Err(FetchError::Network)),
            Some(result) => result,
            None => None,
        };
        if let Some(result) = download {
            let (font, _, purpose) = self.download.take().expect("polled");
            // Cada falla con su aviso: la descarga no fue un zip con la fuente o la fuente no se lee.
            let loaded = result
                .map_err(|e| match e {
                    FetchError::Invalid => {
                        "the download isn't a zip with a .ttf or .otf font.".into()
                    }
                    e => describe(&e),
                })
                .and_then(|(name, bytes)| {
                    cs_tools::text::FontChoice::load(name, &bytes)
                        .map_err(|_| "ChimpSprite can't read this font file.".to_string())
                });
            match (loaded, purpose) {
                (Ok(choice), Purpose::Use) => {
                    picked = Some(Picked {
                        font: choice,
                        size_px: font.size_px,
                    });
                }
                (Ok(choice), Purpose::View) => {
                    if let Some(detail) = &mut self.detail
                        && detail.font.slug == font.slug
                    {
                        detail.chars = choice.chars();
                        detail.loaded = Some(choice);
                    }
                }
                (Err(e), _) => self.error = Some(format!("{}: {e}", font.name)),
            }
        }
        for preview in self.previews.values_mut() {
            if let Preview::Loading(p, since) = preview {
                match p.poll() {
                    Some(Ok(image)) => {
                        *preview = Preview::Ready(ctx.load_texture(
                            "dafont_preview",
                            image,
                            egui::TextureOptions::NEAREST,
                        ));
                    }
                    Some(Err(_)) => *preview = Preview::Failed,
                    None if late(*since) => *preview = Preview::Failed,
                    None => {}
                }
            }
        }
        picked
    }

    /// Lo pedido en el frame. Devuelve la fuente del detalle si se eligió (ya está bajada).
    fn answer(&mut self, ctx: &egui::Context, asked: Asked, now: f64) -> Option<Picked> {
        // Al volver del detalle, la lista también muestra lo que se escribió ahí.
        if (asked.submit || asked.back) && self.sample.trim() != self.shown_sample {
            self.shown_sample = self.sample.trim().to_string();
            self.reload(ctx);
        }
        if asked.sort {
            self.reload(ctx);
        }
        if asked.more {
            self.load_page(ctx, self.page + 1);
        }
        if asked.retry {
            self.load_page(ctx, self.page);
        }
        if asked.back {
            self.detail = None;
        }
        if let Some(font) = asked.view {
            self.detail = Some(Detail::new(font.clone()));
            if !self.downloading(&font.slug) {
                self.download(ctx, font, Purpose::View);
            }
        }
        if let Some(font) = asked.use_font {
            // En el detalle la fuente ya está bajada; en la lista, se baja.
            let loaded = self
                .detail
                .as_ref()
                .filter(|d| d.font.slug == font.slug)
                .and_then(|d| d.loaded.clone());
            match loaded {
                Some(choice) => {
                    return Some(Picked {
                        font: choice,
                        size_px: font.size_px,
                    });
                }
                None => self.download(ctx, font, Purpose::Use),
            }
        }
        // Las vistas previas que se ven y todavía no se pidieron, de a `MAX_IN_FLIGHT`.
        for k in asked.visible {
            if self.in_flight() >= MAX_IN_FLIGHT {
                break;
            }
            let Some(font) = self.fonts.get(k) else {
                continue;
            };
            if let Some(url) = &font.preview_url
                && !self.previews.contains_key(&font.slug)
            {
                let pending = lospec_client::fetch(ctx, url.clone(), |status, bytes| {
                    if status != 200 {
                        return Err(FetchError::Status(status));
                    }
                    crate::news::decode_thumb(bytes)
                });
                self.previews
                    .insert(font.slug.clone(), Preview::Loading(pending, now));
            }
        }
        None
    }

    fn in_flight(&self) -> usize {
        self.previews
            .values()
            .filter(|p| matches!(p, Preview::Loading(..)))
            .count()
    }

    /// Arriba: "Preview" con su campo, "Sort by" con el orden (solo en la lista), Submit y el
    /// logo de dafont a la derecha. Debajo, la línea grabada.
    fn top_row(&mut self, ui: &mut egui::Ui, asked: &mut Asked) {
        let skin = pixel_ui::frame_skin(ui.ctx());
        let in_detail = self.detail.is_some();
        let logo = skin
            .as_ref()
            .and_then(|s| s.piece(LOGO, crate::skin::WidgetState::Normal).cloned());
        let logo_size = logo.as_ref().map_or(vec2(60.0, 16.0), |p| {
            vec2(p.size[0] as f32, p.size[1] as f32)
        });
        let top = ui.cursor().top();
        let right = ui.max_rect().right();
        // Los campos, cada uno con su etiqueta encima, a la izquierda del logo: si no caben en una
        // fila, "Sort by" y Submit bajan debajo de Preview.
        let sample = &mut self.sample;
        let sort = &mut self.sort;
        forms::form(ui, "dafont_top", 0.0, |tui| {
            tui.style(forms::row_nowrap(0.0)).add(|tui| {
                tui.style(forms::row(FIELD_GAP)).add(|tui| {
                    // El campo de la muestra: en el detalle, hasta el logo (no hay orden ni
                    // Submit: la muestra cambia al escribir).
                    let field = if in_detail {
                        forms::grow_auto(SAMPLE_W, SAMPLE_W)
                    } else {
                        forms::natural()
                    };
                    tui.style(field).ui(|ui| {
                        let field_w = if in_detail {
                            ui.available_width().floor()
                        } else {
                            SAMPLE_W
                        };
                        labeled(ui, "Preview", |ui| {
                            let response = match &skin {
                                Some(skin) => crate::kit::input::text_input(
                                    ui,
                                    skin,
                                    sample,
                                    field_w,
                                    "Custom preview",
                                    "Custom preview",
                                ),
                                None => ui.add(
                                    egui::TextEdit::singleline(sample)
                                        .hint_text("Custom preview")
                                        .desired_width(field_w),
                                ),
                            };
                            if response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                asked.submit = true;
                            }
                        });
                    });
                    if in_detail {
                        return;
                    }
                    tui.style(forms::natural()).ui(|ui| {
                        labeled(ui, "Sort by", |ui| {
                            let before = *sort;
                            crate::widgets::combo_sized(
                                ui,
                                "dafont_sort",
                                sort.label(),
                                Some(SORT_W),
                                FIELD_H,
                                |ui| {
                                    for choice in Sort::ALL {
                                        crate::widgets::choice(ui, sort, choice, choice.label());
                                    }
                                },
                            );
                            asked.sort = *sort != before;
                        });
                    });
                    tui.style(forms::natural()).ui(|ui| {
                        labeled(ui, " ", |ui| {
                            if pixel_ui::text_button(ui, "Submit").clicked() {
                                asked.submit = true;
                            }
                        });
                    });
                });
                // El lugar del logo.
                tui.style(forms::fixed(logo_room(logo_size.x), 0.0))
                    .add_empty();
            });
        });
        // El logo, arriba a la derecha: abre dafont.com.
        // En la esquina de arriba a la derecha del panel, contra su luz.
        let corner = egui::pos2(right + LOGO_OVERHANG, top - LOGO_OVERHANG);
        let at = egui::Rect::from_min_size(corner - vec2(logo_size.x, 0.0), logo_size);
        let response = ui
            .interact(at, ui.id().with("dafont_logo"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("dafont.com");
        match &logo {
            Some(piece) => {
                let ppp = ui.ctx().pixels_per_point();
                // Sube hasta el borde del panel, por encima del scroll: con su recorte se perdían
                // sus primeras filas y quedaba un hueco arriba. El recorte crece solo lo que el
                // logo sale del contenido (`LOGO_OVERHANG` arriba y a la derecha): con el
                // contenido desplazado, se recorta como lo demás y no se pinta sobre el marco.
                // (`with_clip_rect` intersecta con el recorte de ahora; `set_clip_rect` lo
                // cambia y deja lo demás del painter: apagado con un modal.)
                let clip = ui.clip_rect();
                let clip = clip
                    .with_min_y(clip.min.y - LOGO_OVERHANG)
                    .with_max_x(clip.max.x.max(right + LOGO_OVERHANG));
                let mut painter = ui.painter().clone();
                painter.set_clip_rect(clip);
                painter.extend(pixel_ui::piece_shapes(piece, at, ppp));
            }
            None => {
                ui.painter().text(
                    at.left_top(),
                    egui::Align2::LEFT_TOP,
                    "dafont.com",
                    egui::TextStyle::Body.resolve(ui.style()),
                    RED,
                );
            }
        }
        if response.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(SITE));
        }
        // La línea grabada a 1 fila del logo (o debajo de los campos, si bajan más).
        let line_top = ui.cursor().top().max(at.bottom() + 1.0);
        let (line, _) = ui.allocate_exact_size(
            vec2(
                ui.available_width(),
                line_top + pixel_ui::ETCHED_SIZE - ui.cursor().top(),
            ),
            egui::Sense::hover(),
        );
        if let Some(skin) = &skin {
            let ppp = ui.ctx().pixels_per_point();
            ui.painter().add(pixel_ui::etched_hline(
                // De lado a lado del panel: de la línea de un borde a la del otro.
                line.x_range().expand(PANEL_MARGIN - 1.0),
                line.bottom() - pixel_ui::ETCHED_SIZE,
                skin.colors(),
                ppp,
            ));
        }
    }

    /// Una fuente de la lista: su barra, su vista previa (la imagen de dafont a su tamaño; con
    /// el puntero encima se recorre a lo ancho) y debajo sus descargas, "Use font" y "View
    /// more". Devuelve si se ve (para pedir su vista previa). `index`, su lugar en la lista: su
    /// forma lleva un id que no se repite aunque dafont repita una fuente.
    fn font_card(
        &self,
        ui: &mut egui::Ui,
        index: usize,
        font: &DafontFont,
        asked: &mut Asked,
    ) -> bool {
        header(ui, font);
        ui.add_space(HEADER_GAP - ui.spacing().item_spacing.y);
        let width = ui.available_width();
        let texture = match self.previews.get(&font.slug) {
            Some(Preview::Ready(texture)) => Some(texture),
            _ => None,
        };
        let height = texture.map_or(40.0, |t| t.size_vec2().y);
        let card_rect = card(ui, width, height);
        let shown = ui.is_rect_visible(card_rect);
        let inner = card_rect.shrink2(vec2(IMAGE_PAD, 0.0));
        let loading = matches!(self.previews.get(&font.slug), Some(Preview::Loading(..)));
        match texture {
            Some(texture) => {
                let size = texture.size_vec2();
                // Más ancha que la tarjeta: el puntero la recorre (de su borde izquierdo al
                // derecho, la imagen de su principio a su final).
                let hover = ui.interact(
                    inner,
                    ui.id().with(("dafont_preview", &font.slug)),
                    egui::Sense::hover(),
                );
                let extra = (size.x - inner.width()).max(0.0);
                let t = hover.hover_pos().map_or(0.0, |p| {
                    ((p.x - inner.left()) / inner.width()).clamp(0.0, 1.0)
                });
                let offset = (extra * t).round();
                let shown_w = size.x.min(inner.width());
                let at = egui::Rect::from_min_size(inner.min, vec2(shown_w, size.y));
                let uv = egui::Rect::from_min_max(
                    egui::pos2(offset / size.x, 0.0),
                    egui::pos2((offset + shown_w) / size.x, 1.0),
                );
                ui.painter()
                    .with_clip_rect(inner)
                    .image(texture.id(), at, uv, Color32::WHITE);
            }
            // Sin imagen (o mientras llega), su nombre.
            None if !loading => card_text(ui, inner, &font.name),
            None => {}
        }
        // Las descargas y, a la derecha, los botones (debajo, si no caben al lado).
        forms::form(ui, ("dafont_card", index, &font.slug), 0.0, |tui| {
            tui.style(forms::min_height(ROW_H, forms::row_lines(FIELD_GAP)))
                .add(|tui| {
                    let downloads = forms::air_above(TEXT_DROP, forms::natural());
                    tui.style(forms::air_right(FIELD_GAP, downloads))
                        .ui(|ui| ui.add(egui::Label::new(&font.downloads).extend()));
                    let buttons = forms::push_right(forms::natural());
                    tui.style(forms::air_above(BUTTON_DROP, buttons)).ui(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            ui.spacing_mut().item_spacing.x = FIELD_GAP;
                            if self.downloading(&font.slug) {
                                // Un texto: 1 más abajo que los botones.
                                ui.vertical(|ui| {
                                    ui.add_space(TEXT_DROP - BUTTON_DROP);
                                    ui.weak("Downloading");
                                });
                            } else if ui
                                .add_enabled_ui(self.download.is_none(), |ui| {
                                    pixel_ui::text_button(ui, "Use font")
                                })
                                .inner
                                .clicked()
                            {
                                asked.use_font = Some(font.clone());
                            }
                            if pixel_ui::text_button(ui, "View more").clicked() {
                                asked.view = Some(font.clone());
                            }
                        });
                    });
                });
        });
        shown
    }
}

/// El detalle de una fuente: su barra, la muestra con el texto de "Custom preview" (o la frase
/// de siempre), el abecedario chico y grande, "Back" y "Use font", y la cuadrícula de todos sus
/// caracteres hasta abajo.
fn detail_ui(
    ui: &mut egui::Ui,
    detail: &mut Detail,
    sample: &str,
    downloading: Option<&str>,
    list_h: impl Fn(&egui::Ui) -> f32,
    asked: &mut Asked,
) {
    let ctx = ui.ctx().clone();
    header(ui, &detail.font);
    ui.add_space(HEADER_GAP - ui.spacing().item_spacing.y);
    let width = ui.available_width();
    let px = detail.px();
    let Some(font) = detail.loaded.clone() else {
        let busy = downloading == Some(detail.font.slug.as_str());
        ui.weak(if busy {
            "Downloading"
        } else {
            "Couldn't load this font"
        });
        buttons_row(ui, "dafont_back", asked, None);
        return;
    };
    // La muestra: se dibuja otra vez si cambió el texto.
    let text = if sample.trim().is_empty() {
        PANGRAM
    } else {
        sample.trim()
    };
    if detail.sample.as_ref().is_none_or(|(t, _)| t != text) {
        let texture = text_image(&font, text, px)
            .map(|img| ctx.load_texture("dafont_sample", img, egui::TextureOptions::NEAREST));
        detail.sample = Some((text.to_string(), texture));
    }
    let charset = detail.charset.get_or_insert_with(|| {
        [px, px * 2].map(|size| {
            text_image(&font, CHARSET, size)
                .map(|img| ctx.load_texture("dafont_charset", img, egui::TextureOptions::NEAREST))
        })
    });
    // La tarjeta de la muestra, con su etiqueta "Preview".
    let body = egui::TextStyle::Body.resolve(ui.style());
    let label = ui
        .painter()
        .layout_no_wrap("Preview".into(), body, LABEL_DIM);
    let label_h = crate::tab_bar::cap_height(&label) + 3.0;
    let sample_tex = detail.sample.as_ref().and_then(|(_, t)| t.as_ref());
    let sample_h = sample_tex.map_or(px as f32, |t| t.size_vec2().y);
    let inner = card(ui, width, CARD_PAD + label_h + sample_h + CARD_PAD);
    let ppp = ui.ctx().pixels_per_point();
    let at = crate::tab_bar::text_pos(&label, inner.left() + CARD_PAD, inner.top() + CARD_PAD, ppp);
    let painter = ui.painter().with_clip_rect(inner);
    painter.galley(at, label, LABEL_DIM);
    if let Some(texture) = sample_tex {
        image_at(
            &painter,
            texture,
            egui::pos2(inner.left() + CARD_PAD, inner.top() + CARD_PAD + label_h),
        );
    }
    ui.add_space(HEADER_GAP);
    // El abecedario, chico y grande.
    let heights: Vec<f32> = charset
        .iter()
        .map(|t| t.as_ref().map_or(0.0, |t| t.size_vec2().y))
        .collect();
    let inner = card(
        ui,
        width,
        CARD_PAD + heights.iter().sum::<f32>() + CARD_PAD * 2.0,
    );
    let painter = ui.painter().with_clip_rect(inner);
    let mut y = inner.top() + CARD_PAD;
    for (texture, h) in charset.iter().zip(&heights) {
        if let Some(texture) = texture {
            image_at(&painter, texture, egui::pos2(inner.left() + CARD_PAD, y));
        }
        y += h + CARD_PAD;
    }
    buttons_row(ui, "dafont_detail_buttons", asked, Some(&detail.font));
    // La cuadrícula: todos los caracteres de la fuente.
    let cell = vec2(
        CELL_W.max(px as f32 + 2.0 * CELL_PAD + 1.0),
        CELL_GLYPH_TOP + (px as f32 * 1.25).ceil() + CELL_PAD,
    );
    let height = list_h(ui);
    pixel_ui::scroll(
        ui,
        egui::ScrollArea::vertical()
            .max_height(height)
            .min_scrolled_height(height)
            .auto_shrink([false, false]),
        |ui| {
            ui.set_max_width(ui.available_width() - SCROLL_GAP);
            glyph_grid(ui, detail, &font, cell);
        },
    );
}

/// Debajo del detalle: "Back" y, con la fuente (`font`), "Use font" a la derecha (debajo, si no
/// caben en una fila).
fn buttons_row(ui: &mut egui::Ui, id: &str, asked: &mut Asked, font: Option<&DafontFont>) {
    forms::form(ui, id, 0.0, |tui| {
        tui.style(forms::row_lines(FIELD_GAP)).add(|tui| {
            let button = forms::air_above(BUTTON_DROP, forms::natural());
            tui.style(forms::air_right(FIELD_GAP, button.clone()))
                .ui(|ui| {
                    asked.back = pixel_ui::text_button(ui, "Back").clicked();
                });
            if let Some(font) = font {
                tui.style(forms::push_right(button)).ui(|ui| {
                    if pixel_ui::text_button(ui, "Use font").clicked() {
                        asked.use_font = Some(font.clone());
                    }
                });
            }
        });
    });
}

/// Un campo con su etiqueta encima, como los de arriba del panel.
fn labeled(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        // En una línea: taffy mide las hojas también a lo más angosto.
        ui.add(egui::Label::new(label).extend());
        add(ui);
    });
}

/// Los caracteres en celdas de `cell` (con la línea compartida): el carácter y su código en
/// hexadecimal, en chico, y su glifo dibujado con la fuente. Los glifos se dibujan al verse.
fn glyph_grid(
    ui: &mut egui::Ui,
    detail: &mut Detail,
    font: &cs_tools::text::FontChoice,
    cell: Vec2,
) {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let width = ui.available_width();
    let columns = (((width - 1.0) / cell.x).floor() as usize).max(1);
    let rows = detail.chars.len().div_ceil(columns);
    let size = vec2(
        columns.min(detail.chars.len()) as f32 * cell.x + 1.0,
        rows as f32 * cell.y + 1.0,
    );
    let (rect, _) = ui.allocate_exact_size(vec2(width, size.y), egui::Sense::hover());
    let clip = ui.clip_rect();
    let small = egui::TextStyle::Small.resolve(ui.style());
    let px = detail.px();
    let mut shapes = Vec::new();
    for (k, &c) in detail.chars.iter().enumerate() {
        let (row, col) = (k / columns, k % columns);
        let min = rect.min + vec2(col as f32 * cell.x, row as f32 * cell.y);
        let at = egui::Rect::from_min_size(min, cell + Vec2::splat(1.0));
        if !clip.intersects(at) {
            continue;
        }
        // La celda: su línea alrededor (compartida con las vecinas) y blanca adentro.
        shapes.push(egui::Shape::rect_filled(at.round_to_pixels(ppp), 0.0, GRID));
        shapes.push(egui::Shape::rect_filled(
            at.shrink(1.0).round_to_pixels(ppp),
            0.0,
            CARD,
        ));
        let left = at.left() + CELL_PAD;
        let label = ui
            .painter()
            .layout_no_wrap(c.to_string(), small.clone(), GRID);
        let code = ui
            .painter()
            .layout_no_wrap(format!("{:04X}", c as u32), small.clone(), GRID);
        let label_at = crate::tab_bar::text_pos(&label, left, at.top() + CELL_PAD + 1.0, ppp);
        let code_at = crate::tab_bar::text_pos(&code, left, at.top() + CELL_CODE_TOP, ppp);
        shapes.push(egui::Shape::galley(label_at, label, GRID));
        shapes.push(egui::Shape::galley(code_at, code, GRID));
        let glyph = detail.glyphs.entry(c).or_insert_with(|| {
            text_image(font, &c.to_string(), px)
                .map(|img| ctx.load_texture("dafont_glyph", img, egui::TextureOptions::NEAREST))
        });
        if let Some(texture) = glyph {
            // Un glifo más grande que la celda se ve recortado, sin deformarse.
            let min = egui::pos2(left, at.top() + CELL_GLYPH_TOP);
            let full = egui::Rect::from_min_size(min, texture.size_vec2());
            let shown = full.intersect(at.shrink(1.0));
            if shown.is_positive() {
                let uv = egui::Rect::from_min_max(
                    egui::pos2(0.0, 0.0),
                    egui::pos2(shown.width() / full.width(), shown.height() / full.height()),
                );
                let mut mesh = egui::Mesh::with_texture(texture.id());
                mesh.add_rect_with_uv(shown, uv, Color32::WHITE);
                shapes.push(egui::Shape::mesh(mesh));
            }
        }
    }
    ui.painter().extend(shapes);
}

/// La barra roja de una fuente: su nombre (clic: su página en dafont, con la licencia completa),
/// "by" y el autor en chico, la etiqueta del tamaño y, a la derecha, la licencia.
fn header(ui: &mut egui::Ui, font: &DafontFont) {
    let ppp = ui.ctx().pixels_per_point();
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), egui::Sense::hover());
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::TextStyle::Small.resolve(ui.style());
    let galley = |text: String, font: &egui::FontId| {
        ui.painter()
            .layout_no_wrap(text, font.clone(), Color32::WHITE)
    };
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, RED);
    let name = galley(font.name.clone(), &body);
    let name_at = crate::tab_bar::text_pos(&name, rect.left() + HEADER_PAD, rect.top() + 3.0, ppp);
    let name_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + HEADER_PAD, rect.top()),
        vec2(name.size().x, HEADER_H),
    );
    let mut x = name_rect.right() + 5.0;
    // Hasta dónde llega lo de la izquierda (la licencia no lo tapa).
    let mut end = name_rect.right();
    painter.galley(name_at, name, Color32::WHITE);
    let small_top = rect.top() + 4.0;
    if !font.author.is_empty() {
        let by = galley(format!("BY {}", font.author.to_uppercase()), &small);
        let w = by.size().x;
        painter.galley(
            crate::tab_bar::text_pos(&by, x, small_top, ppp),
            by,
            Color32::WHITE,
        );
        end = x + w;
        x += w + 4.0;
    }
    if let Some(px) = font.size_px {
        let badge = galley(format!("{px}PX"), &small);
        let caps = crate::tab_bar::cap_height(&badge);
        let back = egui::Rect::from_min_size(
            egui::pos2(x, small_top - 1.0),
            vec2(badge.size().x + 2.0 * BADGE_PAD - 1.0, caps + 2.0),
        );
        end = back.right();
        // Las esquinas cortadas, como en el diseño.
        painter.rect_filled(back.shrink2(vec2(1.0, 0.0)), 0.0, BADGE);
        painter.rect_filled(back.shrink2(vec2(0.0, 1.0)), 0.0, BADGE);
        painter.galley(
            crate::tab_bar::text_pos(&badge, x + BADGE_PAD, small_top, ppp),
            badge,
            Color32::WHITE,
        );
    }
    let license = (!font.license.is_empty()).then(|| galley(font.license.to_uppercase(), &small));
    if let Some(license) = license {
        let left = (rect.right() - HEADER_PAD - license.size().x).round();
        // Sin lugar sin encimarse (una barra angosta), no va: está en la página de la fuente.
        if left >= end {
            painter.galley(
                crate::tab_bar::text_pos(&license, left, small_top, ppp),
                license,
                Color32::WHITE,
            );
        }
    }
    let response = ui
        .interact(
            name_rect,
            ui.id().with(("dafont_name", &font.slug)),
            egui::Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("License & details on dafont.com");
    if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(font.page_url()));
    }
}

/// Una tarjeta blanca de `width` con `height` de contenido: las esquinas cortadas y, debajo, su
/// sombra (sin las esquinas). Devuelve lo de adentro.
fn card(ui: &mut egui::Ui, width: f32, height: f32) -> egui::Rect {
    let ppp = ui.ctx().pixels_per_point();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height + 1.0), egui::Sense::hover());
    let inner = egui::Rect::from_min_size(rect.min, vec2(width, height));
    let painter = ui.painter();
    let snap = |r: egui::Rect| r.round_to_pixels(ppp);
    painter.rect_filled(snap(inner.shrink2(vec2(1.0, 0.0))), 0.0, CARD);
    painter.rect_filled(snap(inner.shrink2(vec2(0.0, 1.0))), 0.0, CARD);
    let shadow = egui::Rect::from_min_max(
        egui::pos2(inner.left() + 1.0, inner.bottom()),
        egui::pos2(inner.right() - 1.0, inner.bottom() + 1.0),
    );
    let shadow_color = crate::pixel_ui::frame_skin(ui.ctx())
        .map_or(crate::theme::CANVAS_BORDER, |s| s.colors().canvas);
    painter.rect_filled(snap(shadow), 0.0, shadow_color);
    inner
}

/// El nombre de la fuente en su tarjeta, si no hay vista previa.
fn card_text(ui: &egui::Ui, inner: egui::Rect, text: &str) {
    ui.painter().text(
        inner.left_center() + vec2(CARD_PAD, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        egui::TextStyle::Body.resolve(ui.style()),
        Color32::BLACK,
    );
}

/// `texture` a su tamaño con su esquina en `at`.
fn image_at(painter: &egui::Painter, texture: &egui::TextureHandle, at: egui::Pos2) {
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(
        texture.id(),
        egui::Rect::from_min_size(at, texture.size_vec2()),
        uv,
        Color32::WHITE,
    );
}

/// `text` dibujado con `font` a `px`, en negro sobre transparente y sin antialias (lo que la
/// fuente cubre a la mitad o más).
fn text_image(font: &cs_tools::text::FontChoice, text: &str, px: u32) -> Option<egui::ColorImage> {
    let style = cs_tools::text::TextStyle {
        font: font.clone(),
        size: px,
        antialias: false,
    };
    let pixels = cs_tools::text::rasterize(text, &style);
    let (w, h) = pixels
        .iter()
        .fold((0, 0), |(w, h), &(x, y, _)| (w.max(x + 1), h.max(y + 1)));
    if w <= 0 || h <= 0 || w > 8192 || h > 1024 {
        return None;
    }
    let (w, h) = (w as usize, h as usize);
    let mut image = egui::ColorImage::filled([w, h], Color32::TRANSPARENT);
    for (x, y, coverage) in pixels {
        if x >= 0 && y >= 0 && coverage >= 128 {
            image.pixels[y as usize * w + x as usize] = Color32::BLACK;
        }
    }
    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Rect;

    /// Dos fuentes como las de la página de la categoría Bitmap (2026-10).
    pub(super) const LISTING: &str = r#"<a name="36172"></a><div class="lv1left dfbg"><a href="minecraft.font"><strong>Minecraft</strong></a>&nbsp;<span title="Euro" class="contain">&euro;</span>  by <a href="craftron-gaming.d6128">Craftron Gaming</a></div><div class="lv1right dfbg">&nbsp;<span style="cursor:help" title="Use at 16 px (or multiple of 16), anti-alias off"> (16 px)</span></div><div class="lv2right">&nbsp;<span class="light">5,204,307 downloads (5,642 yesterday)</span> &nbsp; <a class="tdn help black" style="cursor:help" target="_blank" href="./faq.php#copyright">100% Free</a></div><div class="dlbox" style="height:88px" ><a class="dl" title="5 K" href="//dl.dafont.com/dl/?f=minecraft"  rel="nofollow">&nbsp;Download&nbsp;</a></div><div style="background-image:url(/img/preview/m/i/minecraft0.png)" class="preview"><a href="minecraft.font"></a></div><div class="dfxsmall">&nbsp;</div><a name="46880"></a><div class="lv1left dfbg"><a href="kiwisoda.font"><strong>Kiwi Soda</strong></a>&nbsp;<span title="Accents" class="contain">&agrave;</span>  by <a href="jeti.d1589">jeti</a></div><div class="lv2right">&nbsp;<a class="tdn help black" href="./faq.php#copyright">Public domain / GPL / OFL</a></div><div style="background-image:url(/img/preview/k/i/kiwisoda0.png)" class="preview"></div>"#;

    #[test]
    fn the_listing_gives_name_author_size_license_and_preview() {
        let fonts = parse_listing(LISTING);
        assert_eq!(fonts.len(), 2);
        assert_eq!(
            fonts[0],
            DafontFont {
                name: "Minecraft".into(),
                slug: "minecraft".into(),
                author: "Craftron Gaming".into(),
                size_px: Some(16),
                license: "100% Free".into(),
                downloads: "5,204,307 downloads (5,642 yesterday)".into(),
                preview_url: Some("https://www.dafont.com/img/preview/m/i/minecraft0.png".into()),
                download_url: Some("https://dl.dafont.com/dl/?f=minecraft".into()),
            }
        );
        assert_eq!(fonts[1].name, "Kiwi Soda");
        assert_eq!(fonts[1].size_px, None);
        assert_eq!(fonts[1].license, "Public domain / GPL / OFL");
        assert_eq!(
            fonts[0].download_url(),
            "https://dl.dafont.com/dl/?f=minecraft"
        );
        assert_eq!(fonts[1].downloads, "", "not on the page");
        assert!(parse_listing("<html>nothing here</html>").is_empty());
        // Sin el botón en el listado, los guiones del nombre van como guiones bajos.
        let vcr = DafontFont {
            slug: "vcr-osd-mono".into(),
            download_url: None,
            ..fonts[1].clone()
        };
        assert_eq!(
            vcr.download_url(),
            "https://dl.dafont.com/dl/?f=vcr_osd_mono"
        );
    }

    #[test]
    fn text_is_drawn_black_and_without_antialias() {
        let font = cs_tools::text::FontChoice::Pixel;
        let image = text_image(&font, "A", 1).expect("pixels");
        assert!(image.pixels.contains(&Color32::BLACK));
        assert!(
            image
                .pixels
                .iter()
                .all(|&c| c == Color32::BLACK || c == Color32::TRANSPARENT)
        );
        assert!(text_image(&font, " ", 1).is_none(), "nothing to draw");
        assert!(font.chars().contains(&'A') && !font.chars().contains(&' '));
    }

    #[test]
    fn the_sample_text_goes_in_the_listing_and_its_previews_are_absolute() {
        assert_eq!(
            listing_url(1, "", Sort::Popularity),
            "https://www.dafont.com/bitmap.php"
        );
        assert_eq!(
            listing_url(2, "  ", Sort::Popularity),
            "https://www.dafont.com/bitmap.php?page=2"
        );
        assert_eq!(
            listing_url(3, "Hola mono!", Sort::Popularity),
            "https://www.dafont.com/bitmap.php?text=Hola+mono%21&page=3"
        );
        assert_eq!(
            listing_url(2, "", Sort::Newest),
            "https://www.dafont.com/bitmap.php?sort=date&page=2"
        );
        assert_eq!(
            listing_url(1, "a", Sort::Name),
            "https://www.dafont.com/bitmap.php?text=a&sort=alpha"
        );
        assert_eq!(encode("ñ"), "%C3%B1");
        let html = r#"<div class="lv1left dfbg"><a href="minecraft.font?text=Hola+mono"><strong>Minecraft</strong></a></div><div style="background-image:url(//img.dafont.com/preview_bitmap.php?text=Hola+mono&amp;ttf=minecraft0&amp;ext=1&amp;size=12)" class="preview"></div>"#;
        let fonts = parse_listing(html);
        assert_eq!(fonts[0].slug, "minecraft");
        assert_eq!(
            fonts[0].preview_url.as_deref(),
            Some(
                "https://img.dafont.com/preview_bitmap.php?text=Hola+mono&ttf=minecraft0&ext=1&size=12"
            )
        );
    }

    /// Con el contenido desplazado `scrolled` hacia arriba (la fila de arriba pintada en
    /// `content`, vista por un scroll que muestra desde más abajo): el rect del logo, su recorte y
    /// lo que se ve del scroll.
    fn logo_clip(scrolled: f32) -> (Rect, Rect, Rect) {
        let ctx = egui::Context::default();
        let skin = crate::skin::Skin::embedded(&ctx);
        let piece = skin.piece(LOGO, crate::skin::WidgetState::Normal);
        let size = piece.map(|p| vec2(p.size[0] as f32, p.size[1] as f32));
        let size = size.expect("the logo in the skin");
        ctx.set_fonts(crate::appearance::font_definitions(true));
        crate::appearance::apply_style(&ctx, Some(skin.colors()));
        crate::pixel_ui::set_bubble(&ctx, Some(std::sync::Arc::new(skin)));
        let content = Rect::from_min_size(egui::pos2(20.0, 20.0), vec2(REFERENCE_WIDTH, 500.0));
        let viewport = content.with_min_y(content.min.y + scrolled);
        let corner = egui::pos2(
            content.right() + LOGO_OVERHANG,
            content.top() - LOGO_OVERHANG,
        );
        let logo = Rect::from_min_size(corner - vec2(size.x, 0.0), size);
        let mut panel = DafontPanel::default();
        let mut clip = None;
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000.0, 1000.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
                    ui.set_clip_rect(viewport);
                    panel.top_row(ui, &mut Asked::default());
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

    /// El logo sube hasta la luz del panel, por encima del contenido: el recorte del scroll del
    /// dock, que empieza en el contenido, no le quita sus filas de arriba.
    #[test]
    fn the_logo_reaches_the_panel_edge_above_the_scroll_clip() {
        let (logo, clip, _) = logo_clip(0.0);
        assert!(clip.contains_rect(logo), "{logo:?} clipped by {clip:?}");
    }

    /// Con el contenido desplazado, el logo se recorta como lo demás: su recorte sube solo lo que
    /// el logo sale del contenido, no se pinta sobre el marco del panel.
    #[test]
    fn a_scrolled_logo_is_clipped_like_the_rest() {
        let (logo, clip, viewport) = logo_clip(5.0);
        assert!(
            clip.top() >= viewport.top() - LOGO_OVERHANG,
            "{logo:?} clipped by {clip:?}, viewport {viewport:?}"
        );
    }

    /// El panel a `width`, sin red: la lista con las dos fuentes de `LISTING` (la primera con una
    /// vista previa de 500 × 24, más ancha que las tarjetas; la segunda sin vista previa y con un
    /// autor largo), con lo que haga `setup` (el detalle, un error). Devuelve lo medido y la textura
    /// de la vista previa.
    fn panel_with(
        width: f32,
        setup: impl FnOnce(&mut DafontPanel),
    ) -> (crate::forms::testing::Probe, egui::TextureId) {
        let mut panel = DafontPanel::default();
        let mut fonts = parse_listing(LISTING);
        fonts[1].author = "Somebody With A Long Name".into();
        panel.fonts = fonts;
        panel.previews.insert("kiwisoda".into(), Preview::Failed);
        setup(&mut panel);
        let mut preview = None;
        let probe = crate::forms::testing::run_at(width, |ui| {
            if !panel.previews.contains_key("minecraft") {
                let image = egui::ColorImage::filled([500, 24], Color32::RED);
                let texture =
                    ui.ctx()
                        .load_texture("test_preview", image, egui::TextureOptions::NEAREST);
                preview = Some(texture.id());
                panel
                    .previews
                    .insert("minecraft".into(), Preview::Ready(texture));
            }
            panel.ui(ui);
        });
        (probe, preview.expect("the preview"))
    }

    /// La lista o, con `detail`, el detalle de la fuente pixel de la app.
    fn panel_at(width: f32, detail: bool) -> (crate::forms::testing::Probe, egui::TextureId) {
        panel_with(width, |panel| {
            if detail {
                panel.demo_detail(cs_tools::text::FontChoice::Pixel);
            }
        })
    }

    /// El logo del skin a `width`: en la esquina de arriba a la derecha del panel, contra su luz.
    fn logo_at(width: f32) -> Rect {
        let skin = crate::skin::Skin::embedded(&egui::Context::default());
        let piece = skin.piece(LOGO, crate::skin::WidgetState::Normal);
        let size = piece.map(|p| vec2(p.size[0] as f32, p.size[1] as f32));
        let size = size.expect("the logo in the skin");
        let corner = egui::pos2(width + LOGO_OVERHANG, -LOGO_OVERHANG);
        Rect::from_min_size(corner - vec2(size.x, 0.0), size)
    }

    /// De `MIN_WIDTH` para arriba, la lista y el detalle entran en el ancho del panel, en puntos
    /// enteros y sin taparse: la fila de arriba se parte ("Sort by" y Submit bajan debajo de
    /// Preview) antes de llegar al logo, que sigue arriba a la derecha; las tarjetas son del ancho
    /// del panel, con la vista previa recortada en ellas y los botones adentro (debajo de las
    /// descargas si no caben al lado); la licencia de la barra no tapa al autor. En el detalle el
    /// campo de la muestra llega hasta el logo.
    #[test]
    fn the_dafont_panel_reflows_at_any_width() {
        for width in [MIN_WIDTH, 300.0, REFERENCE_WIDTH, 600.0] {
            let (probe, preview) = panel_at(width, false);
            probe.assert_whole_px();
            probe.assert_within(width);
            probe.assert_no_overlap();
            // El logo, arriba a la derecha; nada de la fila de arriba lo toca.
            let logo = logo_at(width);
            assert!(
                probe.images.iter().any(|(_, r)| *r == logo),
                "the logo at {logo:?} ({width})"
            );
            let field = probe.rect("Custom preview");
            let submit = probe.rect("Submit");
            let sort = probe.rect("Sort by");
            for (name, r) in [("field", field), ("submit", submit), ("sort", sort)] {
                assert!(
                    !r.intersects(logo),
                    "{name} {r:?} under the logo at {width}"
                );
            }
            assert_eq!(field.width(), SAMPLE_W, "at {width}");
            if width >= REFERENCE_WIDTH {
                // Como en el diseño: todo en una fila.
                assert_eq!(submit.bottom(), field.bottom(), "at {width}");
                assert_eq!(sort.top(), probe.rect("Preview").top(), "at {width}");
            } else {
                assert!(
                    submit.top() > field.bottom() && sort.top() > field.bottom(),
                    "Sort by {sort:?} and Submit {submit:?} below {field:?} at {width}"
                );
                assert_eq!(sort.left(), 0.0, "at {width}");
            }
            // La vista previa, recortada en su tarjeta (del ancho del panel, sin la barra).
            let shown: Vec<Rect> = probe
                .images
                .iter()
                .filter(|(id, _)| *id == preview)
                .map(|(_, r)| *r)
                .collect();
            assert_eq!(shown.len(), 1, "at {width}");
            let card = width - SCROLL_GAP;
            assert_eq!(shown[0].left(), IMAGE_PAD, "at {width}");
            assert_eq!(
                shown[0].width(),
                500.0_f32.min(card - 2.0 * IMAGE_PAD),
                "at {width}"
            );
            // Los botones de cada tarjeta, a su derecha; debajo de las descargas si no caben.
            let downloads = probe.rect("5,204,307 downloads (5,642 yesterday)");
            let buttons: Vec<Rect> = probe
                .widgets
                .iter()
                .filter(|(n, _, _)| n == "View more")
                .map(|(_, r, _)| *r)
                .collect();
            assert_eq!(buttons.len(), 2, "at {width}");
            for view in &buttons {
                assert_eq!(view.right(), card, "at {width}");
            }
            // El de la primera tarjeta (la de las descargas).
            let use_font = probe
                .widgets
                .iter()
                .filter(|(n, _, _)| n == "Use font")
                .map(|(_, r, _)| *r)
                .min_by(|a, b| a.top().total_cmp(&b.top()))
                .expect("Use font");
            assert_eq!(
                use_font.right() + FIELD_GAP,
                buttons[0].left().min(buttons[1].left())
            );
            // Al lado de las descargas si caben (161, su aire de 5 y los botones, 106).
            if width > MIN_WIDTH {
                assert_eq!(use_font.top(), downloads.top() - 1.0, "at {width}");
            } else {
                assert!(use_font.top() > downloads.bottom(), "at {width}");
            }
            // La licencia de una barra no tapa al autor ni al tamaño (sin lugar, no se pinta);
            // desde el ancho del diseño, las dos caben.
            let licenses = [("PUBLIC", "BY SOMEBODY"), ("100% FREE", "16PX")];
            for (license, left_of_it) in licenses {
                let (_, before) = probe.text(left_of_it);
                let shown = probe.texts.iter().find(|(t, _)| t.starts_with(license));
                if width >= REFERENCE_WIDTH {
                    assert!(shown.is_some(), "{license} at {width}");
                }
                if let Some((_, r)) = shown {
                    assert!(r.left() >= before.right(), "{r:?} {before:?} at {width}");
                }
            }

            let (probe, _) = panel_at(width, true);
            probe.assert_whole_px();
            probe.assert_within(width);
            probe.assert_no_overlap();
            assert!(probe.images.iter().any(|(_, r)| *r == logo), "at {width}");
            // La muestra llega hasta el logo, con su aire (en el diseño, 314).
            let field = probe.rect("Custom preview");
            assert_eq!(field.right(), logo.left() - 16.0, "at {width}");
            assert!(!field.intersects(logo));
            assert_eq!(probe.rect("Back").left(), 0.0, "at {width}");
            assert_eq!(probe.rect("Use font").right(), width, "at {width}");
            // Lo dibujado con la fuente, recortado en el panel.
            for (_, r) in &probe.images {
                assert!(r.right() <= width || *r == logo, "{r:?} past {width}");
            }
        }
    }

    /// La licencia de una barra se pinta en cuanto cabe sin encimarse con el autor (aunque quede
    /// pegada a él) y, si no cabe, no.
    #[test]
    fn a_license_that_fits_is_painted() {
        let (wide, _) = panel_at(600.0, false);
        let (_, license) = wide.text("PUBLIC");
        let (_, by) = wide.text("BY SOMEBODY");
        let card = 600.0 - SCROLL_GAP;
        assert_eq!(license.right().round(), card - HEADER_PAD, "{license:?}");
        // El ancho del panel en que la licencia llega justo al autor.
        let just = by.right() + license.width() + HEADER_PAD + SCROLL_GAP;
        let painted = |width: f32| {
            let (probe, _) = panel_at(width, false);
            probe.texts.iter().any(|(t, _)| t.starts_with("PUBLIC"))
        };
        assert!(painted(just.ceil() + 1.0), "at {}", just.ceil() + 1.0);
        assert!(!painted(just.floor() - 1.0), "at {}", just.floor() - 1.0);
    }

    /// Un error que no cabe se parte en líneas y Retry baja debajo; uno corto lleva Retry al lado.
    #[test]
    fn errors_wrap_and_retry_follows_them() {
        let long = "Minecraft: the download isn't a zip with a .ttf or .otf font.";
        for width in [MIN_WIDTH, REFERENCE_WIDTH, 600.0] {
            for error in ["Couldn't reach dafont.com.", long] {
                let (probe, _) = panel_with(width, |panel| {
                    panel.error = Some(error.into());
                    panel.listing_failed = true;
                });
                probe.assert_whole_px();
                probe.assert_within(width);
                probe.assert_no_overlap();
                let text = probe.rect(error);
                let retry = probe.rect("Retry");
                // Debajo de la línea grabada (que va a 1 fila del logo).
                assert!(text.top() > logo_at(width).bottom(), "at {width}");
                let fits = probe.text(error).1.height() <= 14.0;
                assert_eq!(
                    fits,
                    error != long || width > MIN_WIDTH,
                    "{error} at {width}"
                );
                if text.width() + ROW_GAP + retry.width() <= width {
                    assert_eq!(retry.left(), text.right() + ROW_GAP, "at {width}");
                    assert_eq!(retry.top(), text.top() - 1.0, "at {width}");
                } else {
                    assert!(retry.top() > text.bottom(), "{error} at {width}");
                }
            }
        }
    }

    #[test]
    fn latin1_pages_keep_their_accents() {
        assert_eq!(decode_html(&[b'C', b'a', b'f', 0xe9]), "Café");
        assert_eq!(decode_html("Café".as_bytes()), "Café");
    }

    #[test]
    fn the_font_comes_out_of_the_zip() {
        use std::io::Write as _;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("readme.txt", options).unwrap();
            zip.write_all(b"free").unwrap();
            zip.start_file("Fonts/Minecraft.ttf", options).unwrap();
            zip.write_all(b"TTFDATA").unwrap();
            zip.finish().unwrap();
        }
        let (name, bytes) = extract_font(buf.get_ref()).expect("a font");
        assert_eq!(name, "Minecraft");
        assert_eq!(bytes, b"TTFDATA");
        assert!(extract_font(b"not a zip").is_none());
    }
}
