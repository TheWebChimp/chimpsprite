//! Fuentes pixel de dafont.com (solo escritorio: el sitio no acepta peticiones desde otras
//! páginas). La ventana lista la categoría Bitmap con la vista previa, el autor, el tamaño
//! recomendado y la licencia de cada fuente; "Use" la descarga (el zip de dafont), saca el
//! `.ttf`/`.otf` y la deja en la herramienta de texto, a su tamaño y sin antialias.
//!
//! Las licencias son de cada autor (muchas solo para uso personal): la ventana las muestra y
//! recuerda revisarlas.

use std::collections::HashMap;
use std::io::Read as _;

use eframe::egui::{self, Color32, Vec2, vec2};

use crate::lospec_client::{self, FetchError, Pending};
use crate::pixel_ui;
use crate::skin::Skin;

const SITE: &str = "https://www.dafont.com";
/// Lo que se acepta de un zip y de una fuente (de sobra para una fuente pixel).
const MAX_ZIP_BYTES: usize = 8 * 1024 * 1024;
const MAX_FONT_BYTES: u64 = 4 * 1024 * 1024;
/// El ancho de las vistas previas en la lista.
const PREVIEW_W: f32 = 320.0;

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
    pub preview_url: Option<String>,
}

impl DafontFont {
    pub fn page_url(&self) -> String {
        format!("{SITE}/{}.font", self.slug)
    }

    fn download_url(&self) -> String {
        format!("https://dl.dafont.com/dl/?f={}", self.slug)
    }
}

/// La página `page` (desde 1) de la categoría Bitmap.
fn listing_url(page: u32) -> String {
    if page <= 1 {
        format!("{SITE}/bitmap.php")
    } else {
        format!("{SITE}/bitmap.php?page={page}")
    }
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
            let (slug, _) = between(block, 0, "<a href=\"", ".font\"")?;
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
            let preview_url =
                between(block, 0, "background-image:url(", ")").map(|(p, _)| format!("{SITE}{p}"));
            (!slug.contains('"') && !slug.contains('/')).then(|| DafontFont {
                name: unescape(name),
                slug: slug.to_string(),
                author,
                size_px,
                license,
                preview_url,
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

/// Pide abrir la ventana (desde la barra de opciones de la herramienta de texto).
pub fn request_open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dafont_open_request"), true));
}

fn take_open_request(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("dafont_open_request")))
        .unwrap_or(false)
}

/// Una fuente sacada del zip: su nombre y sus bytes.
type FontFile = (String, Vec<u8>);

enum Preview {
    Loading(Pending<egui::ColorImage>),
    Ready(egui::TextureHandle),
    Failed,
}

/// La ventana de Dafont.
#[derive(Default)]
pub struct DafontWindow {
    open: bool,
    fonts: Vec<DafontFont>,
    /// La última página pedida y su petición en curso.
    page: u32,
    listing: Option<Pending<Vec<DafontFont>>>,
    /// La fuente que se está bajando.
    download: Option<(DafontFont, Pending<FontFile>)>,
    previews: HashMap<String, Preview>,
    /// El último problema, para mostrarlo.
    error: Option<String>,
    /// No hay más páginas (la última vino vacía).
    end: bool,
}

/// Lo que eligió el usuario: la fuente bajada, lista para la herramienta de texto.
pub struct Picked {
    pub font: cs_tools::text::FontChoice,
    pub size_px: Option<u32>,
}

fn describe(e: &FetchError) -> String {
    match e {
        FetchError::Network => "Couldn't reach dafont.com.".into(),
        FetchError::Status(code) => format!("dafont.com returned an error ({code})."),
        _ => "dafont.com sent something unexpected.".into(),
    }
}

impl DafontWindow {
    fn load_page(&mut self, ctx: &egui::Context, page: u32) {
        self.page = page;
        self.listing = Some(lospec_client::fetch(
            ctx,
            listing_url(page),
            |status, bytes| {
                if status != 200 {
                    return Err(FetchError::Status(status));
                }
                Ok(parse_listing(&decode_html(bytes)))
            },
        ));
    }

    fn download(&mut self, ctx: &egui::Context, font: DafontFont) {
        let pending = lospec_client::fetch(ctx, font.download_url(), |status, bytes| {
            if status != 200 {
                return Err(FetchError::Status(status));
            }
            extract_font(bytes).ok_or(FetchError::Invalid)
        });
        self.error = None;
        self.download = Some((font, pending));
    }

    /// Muestra la ventana (con `skin`, la de Pixel UI) si está abierta o se pidió abrirla.
    /// Devuelve la fuente elegida cuando termina de bajar.
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> Option<Picked> {
        if take_open_request(ctx) {
            self.open = true;
        }
        if !self.open {
            return None;
        }
        if self.fonts.is_empty() && self.listing.is_none() && self.error.is_none() {
            self.load_page(ctx, 1);
        }
        // Lo que llegó.
        if let Some(result) = self.listing.as_ref().and_then(Pending::poll) {
            self.listing = None;
            match result {
                Ok(fonts) if fonts.is_empty() => self.end = true,
                Ok(fonts) => self.fonts.extend(fonts),
                Err(e) => self.error = Some(describe(&e)),
            }
        }
        let mut picked = None;
        if let Some(result) = self.download.as_ref().and_then(|(_, p)| p.poll()) {
            let (font, _) = self.download.take().expect("polled");
            match result.and_then(|(name, bytes)| {
                cs_tools::text::FontChoice::load(name, &bytes).map_err(|_| FetchError::Invalid)
            }) {
                Ok(choice) => {
                    picked = Some(Picked {
                        font: choice,
                        size_px: font.size_px,
                    });
                    self.open = false;
                }
                Err(e) => self.error = Some(format!("{}: {}", font.name, describe(&e))),
            }
        }
        for preview in self.previews.values_mut() {
            if let Preview::Loading(p) = preview
                && let Some(result) = p.poll()
            {
                *preview = match result {
                    Ok(image) => Preview::Ready(ctx.load_texture(
                        "dafont_preview",
                        image,
                        egui::TextureOptions::NEAREST,
                    )),
                    Err(_) => Preview::Failed,
                };
            }
        }
        if self.listing.is_some() || self.download.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        let mut chosen = None;
        let mut more = false;
        let (_, closed) = pixel_ui::app_window(
            ctx,
            skin,
            "dafont_window",
            "Dafont · Pixel fonts",
            true,
            (egui::Align2::CENTER_CENTER, Vec2::ZERO),
            |ui| {
                ui.set_width(PREVIEW_W + 140.0);
                ui.label(
                    "Pixel (bitmap) fonts from dafont.com. Each font has its author's license: \
                     check it before using the font in your work.",
                );
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::from_rgb(0xc0, 0x40, 0x40), error);
                }
                ui.add_space(4.0);
                pixel_ui::scroll(
                    ui,
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .auto_shrink([false, true]),
                    |ui| {
                        for font in &self.fonts {
                            if self.font_row(ui, font) {
                                chosen = Some(font.clone());
                            }
                            ui.add_space(6.0);
                        }
                        if self.listing.is_some() {
                            ui.weak("Loading");
                        } else if !self.end && !self.fonts.is_empty() {
                            more = pixel_ui::text_button(ui, "More fonts").clicked();
                        }
                    },
                );
            },
        );
        if closed {
            self.open = false;
        }
        if let Some(font) = chosen {
            self.download(ctx, font);
        }
        if more {
            self.load_page(ctx, self.page + 1);
        }
        // Las vistas previas que todavía no se pidieron (las de lo listado).
        for font in &self.fonts {
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
                    .insert(font.slug.clone(), Preview::Loading(pending));
            }
        }
        picked
    }

    /// Una fila: la vista previa, el nombre, el autor, el tamaño, la licencia y "Use". Devuelve
    /// si se eligió.
    fn font_row(&self, ui: &mut egui::Ui, font: &DafontFont) -> bool {
        let mut use_it = false;
        ui.horizontal_top(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(PREVIEW_W, 40.0), egui::Sense::hover());
            match self.previews.get(&font.slug) {
                Some(Preview::Ready(texture)) => {
                    let size = texture.size_vec2();
                    let scale = (PREVIEW_W / size.x).min(40.0 / size.y).min(1.0);
                    let at = egui::Rect::from_min_size(rect.min, size * scale);
                    ui.painter().image(
                        texture.id(),
                        at,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                Some(Preview::Failed) => {
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        &font.name,
                        egui::TextStyle::Body.resolve(ui.style()),
                        ui.visuals().text_color(),
                    );
                }
                _ => {}
            }
            ui.vertical(|ui| {
                ui.strong(&font.name);
                if !font.author.is_empty() {
                    ui.label(egui::RichText::new(format!("by {}", font.author)).small());
                }
                if let Some(px) = font.size_px {
                    ui.label(egui::RichText::new(format!("Use at {px} px")).small());
                }
                let license = if font.license.is_empty() {
                    "License not stated: check its page"
                } else {
                    font.license.as_str()
                };
                ui.label(egui::RichText::new(license).small().weak());
                // La página de la fuente: su licencia completa y lo que dice el autor.
                if ui
                    .link(egui::RichText::new("License & details").small())
                    .clicked()
                {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(font.page_url()));
                }
                let downloading = self
                    .download
                    .as_ref()
                    .is_some_and(|(f, _)| f.slug == font.slug);
                if downloading {
                    ui.weak("Downloading");
                } else {
                    ui.add_enabled_ui(self.download.is_none(), |ui| {
                        use_it = pixel_ui::text_button(ui, "Use").clicked();
                    });
                }
            });
        });
        use_it
    }
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
                preview_url: Some("https://www.dafont.com/img/preview/m/i/minecraft0.png".into()),
            }
        );
        assert_eq!(fonts[1].name, "Kiwi Soda");
        assert_eq!(fonts[1].size_px, None);
        assert_eq!(fonts[1].license, "Public domain / GPL / OFL");
        assert_eq!(
            fonts[0].download_url(),
            "https://dl.dafont.com/dl/?f=minecraft"
        );
        assert!(parse_listing("<html>nothing here</html>").is_empty());
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
