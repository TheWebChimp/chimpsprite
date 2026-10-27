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

use crate::lospec_client::{self, FetchError, Pending};
use crate::pixel_ui;

const SITE: &str = "https://www.dafont.com";
/// Lo que se acepta de un zip y de una fuente (de sobra para una fuente pixel).
const MAX_ZIP_BYTES: usize = 8 * 1024 * 1024;
const MAX_FONT_BYTES: u64 = 4 * 1024 * 1024;
/// Ancho del panel: las tarjetas de 350 del diseño y la barra de scroll.
pub const WIDTH: f32 = 364.0;
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
const CARD_SHADOW: Color32 = Color32::from_rgb(189, 164, 153);
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
/// La frase y el abecedario del detalle.
const PANGRAM: &str = "the quick brown fox jumps over the lazy dog";
const CHARSET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789";
/// El logo de dafont (un enlace al sitio).
const LOGO: &str = "dafont.logo";

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
        ui.set_width(WIDTH);
        self.top_row(ui, &mut asked);
        if let Some(error) = &self.error {
            ui.horizontal(|ui| {
                ui.colored_label(Color32::from_rgb(0xc0, 0x40, 0x40), error);
                if self.listing_failed && self.detail.is_none() {
                    asked.retry = pixel_ui::lit_button(ui, "Retry").clicked();
                }
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
                        if self.font_card(ui, font, &mut asked) {
                            asked.visible.push(k);
                        }
                        ui.add_space(6.0);
                    }
                    if self.listing.is_some() {
                        ui.weak("Loading");
                    } else if self.end && self.fonts.is_empty() {
                        ui.weak("No fonts found");
                    } else if !self.end && !self.fonts.is_empty() {
                        asked.more = pixel_ui::lit_button(ui, "More fonts").clicked();
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
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            // El campo de la muestra: en el detalle, hasta el logo (no hay orden ni Submit: la
            // muestra cambia al escribir).
            let field_w = if in_detail {
                (WIDTH - logo_size.x - 10.0).max(SAMPLE_W)
            } else {
                SAMPLE_W
            };
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label("Preview");
                let response = match &skin {
                    Some(skin) => crate::pixel_sections::text_field_sized(
                        ui,
                        skin,
                        &mut self.sample,
                        vec2(field_w, FIELD_H),
                        "Custom preview",
                        "Custom preview",
                    ),
                    None => ui.add(
                        egui::TextEdit::singleline(&mut self.sample)
                            .hint_text("Custom preview")
                            .desired_width(field_w),
                    ),
                };
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    asked.submit = true;
                }
            });
            if !in_detail {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label("Sort by");
                    let before = self.sort;
                    crate::widgets::combo_sized(
                        ui,
                        "dafont_sort",
                        self.sort.label(),
                        Some(SORT_W),
                        FIELD_H,
                        |ui| {
                            for sort in Sort::ALL {
                                crate::widgets::choice(ui, &mut self.sort, sort, sort.label());
                            }
                        },
                    );
                    asked.sort = self.sort != before;
                });
            }
            if !in_detail {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(" ");
                    if pixel_ui::lit_button(ui, "Submit").clicked() {
                        asked.submit = true;
                    }
                });
            }
        });
        // El logo, arriba a la derecha: abre dafont.com.
        // En la esquina de arriba a la derecha del panel, contra su luz.
        let corner = egui::pos2(
            right + PANEL_MARGIN - PANEL_EDGE,
            top - PANEL_MARGIN + PANEL_EDGE,
        );
        let at = egui::Rect::from_min_size(corner - vec2(logo_size.x, 0.0), logo_size);
        let response = ui
            .interact(at, ui.id().with("dafont_logo"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("dafont.com");
        match &logo {
            Some(piece) => {
                let ppp = ui.ctx().pixels_per_point();
                ui.painter().extend(pixel_ui::piece_shapes(piece, at, ppp));
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
    /// more". Devuelve si se ve (para pedir su vista previa).
    fn font_card(&self, ui: &mut egui::Ui, font: &DafontFont, asked: &mut Asked) -> bool {
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
        // Las descargas y los botones.
        ui.horizontal(|ui| {
            ui.label(&font.downloads);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                if pixel_ui::lit_button(ui, "View more").clicked() {
                    asked.view = Some(font.clone());
                }
                if self.downloading(&font.slug) {
                    ui.weak("Downloading");
                } else if ui
                    .add_enabled_ui(self.download.is_none(), |ui| {
                        pixel_ui::lit_button(ui, "Use font")
                    })
                    .inner
                    .clicked()
                {
                    asked.use_font = Some(font.clone());
                }
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
        ui.horizontal(|ui| {
            asked.back = pixel_ui::lit_button(ui, "Back").clicked();
        });
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
    ui.horizontal(|ui| {
        asked.back = pixel_ui::lit_button(ui, "Back").clicked();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if pixel_ui::lit_button(ui, "Use font").clicked() {
                asked.use_font = Some(detail.font.clone());
            }
        });
    });
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
        x += w + 4.0;
    }
    if let Some(px) = font.size_px {
        let badge = galley(format!("{px}PX"), &small);
        let caps = crate::tab_bar::cap_height(&badge);
        let back = egui::Rect::from_min_size(
            egui::pos2(x, small_top - 1.0),
            vec2(badge.size().x + 2.0 * BADGE_PAD - 1.0, caps + 2.0),
        );
        // Las esquinas cortadas, como en el diseño.
        painter.rect_filled(back.shrink2(vec2(1.0, 0.0)), 0.0, BADGE);
        painter.rect_filled(back.shrink2(vec2(0.0, 1.0)), 0.0, BADGE);
        painter.galley(
            crate::tab_bar::text_pos(&badge, x + BADGE_PAD, small_top, ppp),
            badge,
            Color32::WHITE,
        );
    }
    if !font.license.is_empty() {
        let license = galley(font.license.to_uppercase(), &small);
        let left = rect.right() - HEADER_PAD - license.size().x;
        painter.galley(
            crate::tab_bar::text_pos(&license, left.round(), small_top, ppp),
            license,
            Color32::WHITE,
        );
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
    painter.rect_filled(snap(shadow), 0.0, CARD_SHADOW);
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

    /// Dos fuentes como las de la página de la categoría Bitmap (2026-10).
    const LISTING: &str = r#"<a name="36172"></a><div class="lv1left dfbg"><a href="minecraft.font"><strong>Minecraft</strong></a>&nbsp;<span title="Euro" class="contain">&euro;</span>  by <a href="craftron-gaming.d6128">Craftron Gaming</a></div><div class="lv1right dfbg">&nbsp;<span style="cursor:help" title="Use at 16 px (or multiple of 16), anti-alias off"> (16 px)</span></div><div class="lv2right">&nbsp;<span class="light">5,204,307 downloads (5,642 yesterday)</span> &nbsp; <a class="tdn help black" style="cursor:help" target="_blank" href="./faq.php#copyright">100% Free</a></div><div class="dlbox" style="height:88px" ><a class="dl" title="5 K" href="//dl.dafont.com/dl/?f=minecraft"  rel="nofollow">&nbsp;Download&nbsp;</a></div><div style="background-image:url(/img/preview/m/i/minecraft0.png)" class="preview"><a href="minecraft.font"></a></div><div class="dfxsmall">&nbsp;</div><a name="46880"></a><div class="lv1left dfbg"><a href="kiwisoda.font"><strong>Kiwi Soda</strong></a>&nbsp;<span title="Accents" class="contain">&agrave;</span>  by <a href="jeti.d1589">jeti</a></div><div class="lv2right">&nbsp;<a class="tdn help black" href="./faq.php#copyright">Public domain / GPL / OFL</a></div><div style="background-image:url(/img/preview/k/i/kiwisoda0.png)" class="preview"></div>"#;

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
