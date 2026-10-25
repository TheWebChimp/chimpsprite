//! Pixnews: las novedades del mundo del pixel art en Home, de varias fuentes (una pestaña por
//! fuente), como grilla de miniaturas; clic abre la publicación en el navegador. Cada fuente se
//! pide la primera vez que se abre su pestaña (una vez por sesión: Reddit limita los pedidos) y
//! con Refresh. Las miniaturas, solo las de las celdas visibles y de a `MAX_IN_FLIGHT`; se
//! decodifican en el hilo de la petición y se guardan como texturas. Solo escritorio.
//!
//! Fuentes: r/PixelArt (feed Atom; el JSON da 403 sin cuenta), itch.io (RSS de juegos con el tag
//! pixel-art), Mastodon (RSS de #pixelart), Retronator (RSS de su Tumblr), Pixel Joint (no tiene
//! feed: la galería en HTML) y PixelArtJourney (la API pública de Bluesky).

use std::collections::HashMap;

use eframe::egui::{self, Color32, Rect, Sense, Vec2, vec2};

use crate::lospec_client::{self, FetchError, Pending};
use crate::pixel_ui::Tip as _;

/// Tope del cuerpo de una miniatura.
const MAX_THUMB_BYTES: usize = 6 * 1024 * 1024;
/// Tope de ancho y alto de una miniatura decodificada.
const MAX_THUMB_SIDE: u32 = 2048;
/// Miniaturas pidiéndose a la vez.
const MAX_IN_FLIGHT: usize = 4;
/// Una petición que no respondió en este tiempo se da por perdida.
const TIMEOUT_SECS: f64 = 20.0;
/// Lado de la caja de cada miniatura y aire entre celdas.
const CELL: f32 = 150.0;
const GAP: f32 = 10.0;
/// Hasta este lado una miniatura es pixel art chico (Pixel Joint): se agranda por un entero, sin
/// suavizar.
const SMALL_SIDE: usize = 200;

/// Una fuente de Pixnews.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Reddit,
    Itch,
    Mastodon,
    Retronator,
    PixelJoint,
    PixelArtJourney,
}

impl Source {
    pub const ALL: [Source; 6] = [
        Source::Reddit,
        Source::Itch,
        Source::Mastodon,
        Source::Retronator,
        Source::PixelJoint,
        Source::PixelArtJourney,
    ];

    /// El nombre de la pestaña.
    pub fn label(self) -> &'static str {
        match self {
            Source::Reddit => "r/PixelArt",
            Source::Itch => "itch.io",
            Source::Mastodon => "Mastodon",
            Source::Retronator => "Retronator",
            Source::PixelJoint => "Pixel Joint",
            Source::PixelArtJourney => "PixelArtJourney",
        }
    }

    /// Qué se muestra, al lado del nombre.
    pub fn detail(self) -> &'static str {
        match self {
            Source::Reddit => "Top this week",
            Source::Itch => "New pixel art games",
            Source::Mastodon => "#pixelart",
            Source::Retronator => "Daily picks",
            Source::PixelJoint => "Gallery",
            Source::PixelArtJourney => "On Bluesky",
        }
    }

    /// Su ícono: una pieza del skin.
    pub fn icon(self) -> &'static str {
        match self {
            Source::Reddit => "news.icon.reddit",
            Source::Itch => "news.icon.itch",
            Source::Mastodon => "news.icon.mastodon",
            Source::Retronator => "news.icon.retronator",
            Source::PixelJoint => "news.icon.pixeljoint",
            Source::PixelArtJourney => "news.icon.pixelartjourney",
        }
    }

    /// De dónde se lee.
    #[cfg_attr(test, allow(dead_code))]
    pub fn feed_url(self) -> &'static str {
        match self {
            Source::Reddit => "https://www.reddit.com/r/PixelArt/top/.rss?t=week&limit=30",
            Source::Itch => "https://itch.io/games/tag-pixel-art.xml",
            Source::Mastodon => "https://mastodon.social/tags/pixelart.rss",
            Source::Retronator => "https://www.retronator.com/rss",
            Source::PixelJoint => "https://pixeljoint.com/pixels/new_icons.asp",
            Source::PixelArtJourney => {
                "https://public.api.bsky.app/xrpc/app.bsky.feed.getAuthorFeed?actor=pixelartjourney.bsky.social&limit=30&filter=posts_with_media"
            }
        }
    }

    /// La página de la fuente (para abrirla en el navegador).
    pub fn site_url(self) -> &'static str {
        match self {
            Source::Reddit => "https://www.reddit.com/r/PixelArt/top/?t=week",
            Source::Itch => "https://itch.io/games/tag-pixel-art",
            Source::Mastodon => "https://mastodon.social/tags/pixelart",
            Source::Retronator => "https://www.retronator.com/",
            Source::PixelJoint => "https://pixeljoint.com/pixels/new_icons.asp",
            Source::PixelArtJourney => "https://bsky.app/profile/pixelartjourney.bsky.social",
        }
    }

    /// Lee lo que devolvió `feed_url`.
    pub fn parse(self, bytes: &[u8]) -> Result<Vec<Post>, FetchError> {
        match self {
            Source::Reddit => parse_reddit(bytes),
            Source::Itch => parse_itch(bytes),
            Source::Mastodon => parse_mastodon(bytes),
            Source::Retronator => parse_retronator(bytes),
            Source::PixelJoint => parse_pixeljoint(bytes),
            Source::PixelArtJourney => parse_bluesky(bytes),
        }
    }
}

/// Una publicación con imagen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Post {
    pub title: String,
    pub author: String,
    /// La página de la publicación.
    pub url: String,
    /// La miniatura que se muestra.
    pub image_url: String,
}

/// Lo de adentro de `<tag ...>…</tag>` en `entry` (sin los atributos de la etiqueta ni un
/// `CDATA` que lo envuelva).
fn element<'a>(entry: &'a str, tag: &str) -> Option<&'a str> {
    let mut from = 0;
    // `<tag` seguido de `>` o de un espacio (no `<tagOtra`).
    let open = loop {
        let at = from + entry[from..].find(&format!("<{tag}"))?;
        let next = entry[at + tag.len() + 1..].chars().next()?;
        if next == '>' || next.is_whitespace() {
            break at;
        }
        from = at + 1;
    };
    let body = open + entry[open..].find('>')? + 1;
    let close = body + entry[body..].find(&format!("</{tag}>"))?;
    let inner = entry[body..close].trim();
    Some(
        inner
            .strip_prefix("<![CDATA[")
            .and_then(|s| s.strip_suffix("]]>"))
            .unwrap_or(inner),
    )
}

/// El valor del atributo `attr` (entre `"` o `'`) de la primera etiqueta `<tag ...>` de `text`.
fn attribute<'a>(text: &'a str, tag: &str, attr: &str) -> Option<&'a str> {
    let open = text.find(&format!("<{tag} "))?;
    let end = open + text[open..].find('>')?;
    attr_in(&text[open..end], attr)
}

/// El valor de `attr` en el encabezado de una etiqueta.
fn attr_in<'a>(head: &'a str, attr: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let key = format!("{attr}={quote}");
        if let Some(at) = head.find(&key) {
            let at = at + key.len();
            let len = head[at..].find(quote)?;
            return Some(&head[at..at + len]);
        }
    }
    None
}

/// Los escapes de XML y HTML (y los números `&#N;`).
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail.find(';').filter(|&e| e <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// El texto de un pedazo de HTML: sin etiquetas, sin escapes y con los espacios juntos. Las
/// etiquetas de bloque (párrafos, saltos) separan con un espacio; las de adentro de una línea
/// (`<span>`, `<a>`…) no.
fn strip_tags(html: &str) -> String {
    const BLOCKS: [&str; 8] = ["p", "br", "div", "li", "figure", "h1", "h2", "h3"];
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        text.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            rest = &rest[open..];
            break;
        };
        let tag = rest[open + 1..open + close]
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if BLOCKS.contains(&tag.as_str()) {
            text.push(' ');
        }
        rest = &rest[open + close + 1..];
    }
    text.push_str(rest);
    unescape(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Las entradas de un feed: lo que hay entre `<open>` y `</close>`.
fn entries<'a>(text: &'a str, open: &'a str, close: &'a str) -> impl Iterator<Item = &'a str> {
    text.split(open)
        .skip(1)
        .filter_map(move |e| e.split(close).next())
}

fn text(bytes: &[u8]) -> Result<&str, FetchError> {
    std::str::from_utf8(bytes).map_err(|_| FetchError::Invalid)
}

/// Un título de una línea a partir de un texto largo (los posts de Mastodon no tienen título).
/// Sin los caracteres invisibles que acompañan a los emojis (el selector de variante y la unión
/// de secuencias): la fuente de los emojis no los tiene y se verían como cajitas.
fn headline(text: &str) -> String {
    const MAX: usize = 80;
    let text: String = text
        .chars()
        .filter(|c| !matches!(c, '\u{FE0E}' | '\u{FE0F}' | '\u{200D}'))
        .collect();
    let text = text.as_str();
    if text.chars().count() <= MAX {
        return text.to_owned();
    }
    let cut: String = text.chars().take(MAX).collect();
    format!("{}…", cut.trim_end())
}

/// r/PixelArt: el feed Atom. Salta los que no traen miniatura (texto) y los marcados NSFW.
pub fn parse_reddit(bytes: &[u8]) -> Result<Vec<Post>, FetchError> {
    let text = text(bytes)?;
    if !text.contains("<feed") {
        return Err(FetchError::Invalid);
    }
    Ok(entries(text, "<entry>", "</entry>")
        .filter(|entry| !entry.contains("term=\"nsfw\""))
        .filter_map(|entry| {
            let author = element(entry, "name").map(unescape).unwrap_or_default();
            Some(Post {
                title: headline(&unescape(element(entry, "title")?)),
                author: author.trim().trim_start_matches("/u/").to_owned(),
                url: unescape(attribute(entry, "link", "href")?),
                image_url: unescape(attribute(entry, "media:thumbnail", "url")?),
            })
        })
        .collect())
}

/// itch.io: el RSS de un tag. El autor es el subdominio del juego.
pub fn parse_itch(bytes: &[u8]) -> Result<Vec<Post>, FetchError> {
    let text = text(bytes)?;
    if !text.contains("<rss") {
        return Err(FetchError::Invalid);
    }
    Ok(entries(text, "<item>", "</item>")
        .filter_map(|item| {
            let url = unescape(element(item, "link")?);
            let author = url
                .split("://")
                .nth(1)
                .and_then(|host| host.split('.').next())
                .unwrap_or_default()
                .to_owned();
            let title = element(item, "plainTitle").or_else(|| element(item, "title"))?;
            Some(Post {
                title: headline(&unescape(title)),
                author,
                url,
                image_url: unescape(element(item, "imageurl")?),
            })
        })
        .collect())
}

/// Mastodon: el RSS de un hashtag. Solo los posts con una imagen (no video) que no sea para
/// adultos; el título es el comienzo del texto y la imagen, la versión chica.
pub fn parse_mastodon(bytes: &[u8]) -> Result<Vec<Post>, FetchError> {
    let text = text(bytes)?;
    if !text.contains("<rss") {
        return Err(FetchError::Invalid);
    }
    Ok(entries(text, "<item>", "</item>")
        .filter_map(|item| {
            let url = unescape(element(item, "link")?);
            // La primera imagen (los videos y GIFs animados vienen como mp4).
            let image = item.split("<media:content ").skip(1).find_map(|media| {
                let head = &media[..media.find('>')?];
                let rest = media.split("</media:content>").next().unwrap_or_default();
                let image = attr_in(head, "type")?.starts_with("image/");
                (image && !rest.contains(">adult<")).then(|| attr_in(head, "url"))?
            })?;
            let author = url
                .split("/@")
                .nth(1)
                .and_then(|a| a.split('/').next())
                .unwrap_or_default()
                .to_owned();
            let body = strip_tags(&unescape(element(item, "description").unwrap_or_default()));
            Some(Post {
                title: headline(&body),
                author,
                url,
                image_url: unescape(image).replacen("/original/", "/small/", 1),
            })
        })
        .collect())
}

/// Retronator: el RSS de su Tumblr. La imagen es la primera de la descripción.
pub fn parse_retronator(bytes: &[u8]) -> Result<Vec<Post>, FetchError> {
    let text = text(bytes)?;
    if !text.contains("<rss") {
        return Err(FetchError::Invalid);
    }
    Ok(entries(text, "<item>", "</item>")
        .filter_map(|item| {
            let description = unescape(element(item, "description")?);
            let image = attribute(&description, "img", "src")?.to_owned();
            Some(Post {
                title: headline(&strip_tags(&unescape(element(item, "title")?))),
                author: "Retronator".to_owned(),
                url: unescape(element(item, "link")?),
                image_url: unescape(&image),
            })
        })
        .collect())
}

/// Pixel Joint: no tiene feed. La galería en HTML: cada obra es un `imgbox` con su enlace y su
/// ícono de 100×100; el título está en el `alt` y el autor en el tooltip ("By …").
pub fn parse_pixeljoint(bytes: &[u8]) -> Result<Vec<Post>, FetchError> {
    const SITE: &str = "https://pixeljoint.com";
    // Página en Latin-1: los bytes de más de 127 se leen como su carácter.
    let text: String = bytes.iter().map(|&b| b as char).collect();
    if !text.contains("imgbox") {
        return Err(FetchError::Invalid);
    }
    Ok(text
        .split("<div class='imgbox'")
        .skip(1)
        .filter_map(|block| {
            let link = attribute(block, "a", "href")?;
            let image = attribute(block, "img", "src")?;
            let title = attribute(block, "img", "alt")?;
            let title = title.trim_end_matches(" icon/pixelart").trim();
            let author = block
                .split("By <a")
                .nth(1)
                .and_then(|a| a.split('>').nth(1))
                .and_then(|a| a.split('<').next())
                .unwrap_or_default();
            let absolute = |p: &str| {
                if p.starts_with("http") {
                    p.to_owned()
                } else {
                    format!("{SITE}{p}")
                }
            };
            Some(Post {
                title: headline(&unescape(title)),
                author: unescape(author),
                url: absolute(link),
                image_url: absolute(image),
            })
        })
        .collect())
}

/// PixelArtJourney: los posts con imágenes de su cuenta de Bluesky (la API pública,
/// `getAuthorFeed`). La imagen es la miniatura de la primera.
pub fn parse_bluesky(bytes: &[u8]) -> Result<Vec<Post>, FetchError> {
    let json: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| FetchError::Invalid)?;
    let feed = json["feed"].as_array().ok_or(FetchError::Invalid)?;
    Ok(feed
        .iter()
        .filter_map(|item| {
            let post = &item["post"];
            let embed = &post["embed"];
            let images = embed["images"]
                .as_array()
                .or_else(|| embed["media"]["images"].as_array())?;
            let thumb = images.first()?["thumb"].as_str()?;
            let handle = post["author"]["handle"].as_str()?;
            let key = post["uri"].as_str()?.rsplit('/').next()?;
            let text = post["record"]["text"].as_str().unwrap_or_default();
            Some(Post {
                title: headline(&text.split_whitespace().collect::<Vec<_>>().join(" ")),
                author: handle.to_owned(),
                url: format!("https://bsky.app/profile/{handle}/post/{key}"),
                // El CDN manda WebP; con `@png` lo da en PNG (sin pérdida, mejor para pixel art).
                image_url: format!("{thumb}@png"),
            })
        })
        .collect())
}

/// Bytes de una miniatura (JPEG, PNG o GIF; de un GIF, el primer cuadro) → imagen para egui.
/// Corre en el hilo de la petición.
pub fn decode_thumb(bytes: &[u8]) -> Result<egui::ColorImage, FetchError> {
    if bytes.len() > MAX_THUMB_BYTES {
        return Err(FetchError::Invalid);
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| FetchError::Invalid)?;
    let (w, h) = reader.into_dimensions().map_err(|_| FetchError::Invalid)?;
    if w == 0 || h == 0 || w > MAX_THUMB_SIDE || h > MAX_THUMB_SIDE {
        return Err(FetchError::Invalid);
    }
    let decoded = image::load_from_memory(bytes).map_err(|_| FetchError::Invalid)?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}

enum Listing {
    Loading(Pending<Vec<Post>>, f64),
    Ready(Vec<Post>),
    Failed(FetchError),
}

enum Thumb {
    Loading(Pending<egui::ColorImage>, f64),
    Ready(egui::TextureHandle),
    Failed,
}

/// Cómo se piden los listados y las miniaturas (los tests no salen a la red).
pub trait Fetcher {
    fn listing(&self, ctx: &egui::Context, source: Source) -> Pending<Vec<Post>>;
    fn thumb(&self, ctx: &egui::Context, url: &str) -> Pending<egui::ColorImage>;
}

/// La red de verdad, con `ehttp` (los tests de la app usan [`Offline`]).
#[cfg_attr(test, allow(dead_code))]
pub struct Online;

impl Fetcher for Online {
    fn listing(&self, ctx: &egui::Context, source: Source) -> Pending<Vec<Post>> {
        lospec_client::fetch(ctx, source.feed_url().to_owned(), move |status, bytes| {
            lospec_client::check_status(status)?;
            source.parse(bytes)
        })
    }

    fn thumb(&self, ctx: &egui::Context, url: &str) -> Pending<egui::ColorImage> {
        lospec_client::fetch(ctx, url.to_owned(), |status, bytes| {
            lospec_client::check_status(status)?;
            decode_thumb(bytes)
        })
    }
}

/// Sin red: nada llega nunca (los tests de la app muestran Home sin salir a internet).
#[cfg(test)]
pub struct Offline;

#[cfg(test)]
impl Fetcher for Offline {
    fn listing(&self, _: &egui::Context, _: Source) -> Pending<Vec<Post>> {
        Pending::never()
    }
    fn thumb(&self, _: &egui::Context, _: &str) -> Pending<egui::ColorImage> {
        Pending::never()
    }
}

/// El de la app: la red (en los tests, [`Offline`]).
pub fn default_fetcher() -> Box<dyn Fetcher> {
    #[cfg(test)]
    return Box::new(Offline);
    #[cfg(not(test))]
    Box::new(Online)
}

/// Lo que pidió el feed.
pub enum FeedAction {
    /// Abrir esta URL en el navegador.
    Open(String),
}

/// Pixnews: la fuente elegida, los listados que ya se pidieron y las miniaturas (por URL).
pub struct News {
    pub source: Source,
    listings: HashMap<Source, Listing>,
    thumbs: HashMap<String, Thumb>,
}

impl Default for News {
    fn default() -> Self {
        Self {
            source: Source::Reddit,
            listings: HashMap::new(),
            thumbs: HashMap::new(),
        }
    }
}

impl News {
    /// Entrega lo que llegó (o se venció) desde el frame anterior.
    fn poll(&mut self, ctx: &egui::Context, now: f64) {
        for listing in self.listings.values_mut() {
            if let Listing::Loading(pending, started) = listing {
                match pending.poll() {
                    Some(Ok(posts)) => *listing = Listing::Ready(posts),
                    Some(Err(e)) => *listing = Listing::Failed(e),
                    None if now - *started > TIMEOUT_SECS => {
                        *listing = Listing::Failed(FetchError::Network);
                    }
                    None => {}
                }
            }
        }
        for (url, thumb) in self.thumbs.iter_mut() {
            if let Thumb::Loading(pending, started) = thumb {
                match pending.poll() {
                    Some(Ok(image)) => {
                        // El pixel art chico se agranda sin suavizar.
                        let small = image.size[0].max(image.size[1]) <= SMALL_SIDE;
                        let options = if small {
                            egui::TextureOptions::NEAREST
                        } else {
                            egui::TextureOptions::LINEAR
                        };
                        *thumb = Thumb::Ready(ctx.load_texture(url.clone(), image, options));
                    }
                    Some(Err(_)) => *thumb = Thumb::Failed,
                    None if now - *started > TIMEOUT_SECS => *thumb = Thumb::Failed,
                    None => {}
                }
            }
        }
    }

    fn in_flight(&self) -> usize {
        self.thumbs
            .values()
            .filter(|t| matches!(t, Thumb::Loading(..)))
            .count()
    }

    /// Elige la fuente (se pide la primera vez que se muestra).
    pub fn select(&mut self, source: Source) {
        self.source = source;
    }

    /// El feed de la fuente elegida en `ui`: una fila con su nombre, Open y Refresh, y la grilla
    /// (con su scroll). Pide el listado la primera vez y las miniaturas de las celdas visibles.
    pub fn ui(&mut self, ui: &mut egui::Ui, fetcher: &dyn Fetcher) -> Option<FeedAction> {
        let ctx = ui.ctx().clone();
        let now = ui.input(|i| i.time);
        let source = self.source;
        self.listings
            .entry(source)
            .or_insert_with(|| Listing::Loading(fetcher.listing(&ctx, source), now));
        self.poll(&ctx, now);
        let loading = self
            .listings
            .values()
            .any(|l| matches!(l, Listing::Loading(..)));
        // Para revisar los plazos aunque no llegue nada; sin el foco de la ventana, no (lo que
        // llegue pide su repintado igual).
        if (loading || self.in_flight() > 0) && ui.input(|i| i.focused) {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        let mut action = None;
        ui.horizontal(|ui| {
            ui.strong(source.label());
            ui.weak(format!("· {}", source.detail()));
            if crate::pixel_ui::text_button(ui, "Open")
                .tip(source.site_url())
                .clicked()
            {
                action = Some(FeedAction::Open(source.site_url().to_owned()));
            }
            let done = matches!(
                self.listings.get(&source),
                Some(Listing::Ready(_) | Listing::Failed(_))
            );
            if done && crate::pixel_ui::text_button(ui, "Refresh").clicked() {
                self.listings.remove(&source);
                ctx.request_repaint();
            }
        });
        ui.add_space(6.0);
        let posts = match self.listings.get(&source) {
            None | Some(Listing::Loading(..)) => {
                ui.weak(format!("Loading {}…", source.label()));
                return action;
            }
            Some(Listing::Failed(e)) => {
                ui.weak(match e {
                    FetchError::Network => format!("Couldn't reach {}.", source.label()),
                    FetchError::Status(429) => format!(
                        "{} is busy right now. Try Refresh in a minute.",
                        source.label()
                    ),
                    FetchError::Status(code) => {
                        format!("{} returned an error ({code}).", source.label())
                    }
                    _ => format!("{} sent something unexpected.", source.label()),
                });
                return action;
            }
            Some(Listing::Ready(posts)) if posts.is_empty() => {
                ui.weak("Nothing with images right now.");
                return action;
            }
            Some(Listing::Ready(posts)) => posts.clone(),
        };
        let columns = ((ui.available_width() + GAP) / (CELL + GAP))
            .floor()
            .max(1.0) as usize;
        egui::ScrollArea::vertical()
            .id_salt(("news_feed", source.label()))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in posts.chunks(columns) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = GAP;
                        for post in row {
                            if self.cell(ui, post, fetcher, now) {
                                action = Some(FeedAction::Open(post.url.clone()));
                            }
                        }
                    });
                    ui.add_space(GAP);
                }
            });
        action
    }

    /// Una celda: la miniatura (o su hueco) y debajo el título y el autor. Pide la miniatura si
    /// se ve. Devuelve si se hizo clic.
    fn cell(&mut self, ui: &mut egui::Ui, post: &Post, fetcher: &dyn Fetcher, now: f64) -> bool {
        let response = ui
            .vertical(|ui| {
                ui.set_width(CELL);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL), Sense::hover());
                if ui.is_rect_visible(rect)
                    && !self.thumbs.contains_key(&post.image_url)
                    && self.in_flight() < MAX_IN_FLIGHT
                {
                    let pending = fetcher.thumb(ui.ctx(), &post.image_url);
                    self.thumbs
                        .insert(post.image_url.clone(), Thumb::Loading(pending, now));
                }
                let painter = ui.painter();
                painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
                match self.thumbs.get(&post.image_url) {
                    Some(Thumb::Ready(texture)) => {
                        let size = fit(texture.size_vec2(), rect.size());
                        // En puntos enteros: el pixel art agrandado queda nítido.
                        let min = (rect.center() - size / 2.0).round();
                        let at = Rect::from_min_size(min, size);
                        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        painter.image(texture.id(), at, uv, Color32::WHITE);
                    }
                    Some(Thumb::Failed) => {
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "×",
                            egui::TextStyle::Body.resolve(ui.style()),
                            ui.visuals().weak_text_color(),
                        );
                    }
                    _ => {}
                }
                // Los textos chicos.
                ui.add(
                    egui::Label::new(egui::RichText::new(post.title.as_str()).small()).truncate(),
                );
                ui.label(egui::RichText::new(post.author.as_str()).small().weak());
            })
            .response;
        let response = ui
            .interact(response.rect, ui.id().with(&post.url), Sense::click())
            .tip(post.title.as_str())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response.clicked()
    }
}

/// `size` achicado (o agrandado) para entrar entero en `room`, sin deformar. Lo chico (pixel art)
/// se agranda por un entero, para que siga nítido.
fn fit(size: Vec2, room: Vec2) -> Vec2 {
    if size.x <= 0.0 || size.y <= 0.0 {
        return Vec2::ZERO;
    }
    let scale = (room.x / size.x).min(room.y / size.y);
    let scale = if scale >= 1.0 { scale.floor() } else { scale };
    vec2((size.x * scale).floor(), (size.y * scale).floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reddit_keeps_posts_with_a_thumbnail() {
        let entry = |title: &str, thumb: bool, nsfw: bool| {
            let thumb = if thumb {
                r#"<media:thumbnail url="https://preview/x.png?width=640&amp;crop=smart" />"#
            } else {
                ""
            };
            let nsfw = if nsfw {
                r#"<category term="nsfw" />"#
            } else {
                ""
            };
            format!(
                r#"<entry><author><name>/u/ana</name></author>{nsfw}<id>t3_x</id>{thumb}
                <link href="https://www.reddit.com/r/PixelArt/comments/x/{title}/" />
                <title>{title} &amp; &quot;co&quot;</title></entry>"#
            )
        };
        let feed = format!(
            r#"<?xml version="1.0"?><feed xmlns="http://www.w3.org/2005/Atom"><title>top</title>{}{}{}</feed>"#,
            entry("ok", true, false),
            entry("text", false, false),
            entry("nsfw", true, true)
        );
        assert_eq!(
            parse_reddit(feed.as_bytes()).unwrap(),
            vec![Post {
                title: "ok & \"co\"".into(),
                author: "ana".into(),
                url: "https://www.reddit.com/r/PixelArt/comments/x/ok/".into(),
                image_url: "https://preview/x.png?width=640&crop=smart".into(),
            }]
        );
        assert_eq!(
            parse_reddit(b"<html>blocked</html>"),
            Err(FetchError::Invalid)
        );
    }

    #[test]
    fn itch_reads_the_plain_title_the_image_and_the_author_from_the_link() {
        let rss = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>x</title>
            <item><guid>g</guid><title>Game [Free] [Windows]</title><plainTitle>Game &amp; Co</plainTitle>
            <imageurl>https://img.itch.zone/a/315x250%23c/b.png</imageurl>
            <link>https://maker.itch.io/game</link>
            <description><![CDATA[About <img src="x"/>]]></description></item>
            <item><title>No image</title><link>https://a.itch.io/b</link></item>
            </channel></rss>"#;
        assert_eq!(
            parse_itch(rss.as_bytes()).unwrap(),
            vec![Post {
                title: "Game & Co".into(),
                author: "maker".into(),
                url: "https://maker.itch.io/game".into(),
                image_url: "https://img.itch.zone/a/315x250%23c/b.png".into(),
            }]
        );
    }

    #[test]
    fn mastodon_keeps_image_posts_with_their_small_version() {
        let item = |user: &str, media: &str| {
            format!(
                r#"<item><link>https://mastodon.social/@{user}/1</link>
                <description>&lt;p&gt;Hello &lt;a href="x"&gt;#&lt;span&gt;pixelart&lt;/span&gt;&lt;/a&gt;&lt;/p&gt;</description>
                {media}</item>"#
            )
        };
        let image = r#"<media:content url="https://files.mastodon.social/a/original/b.png" type="image/png" medium="image"><media:rating scheme="urn:simple">nonadult</media:rating></media:content>"#;
        let video = r#"<media:content url="https://files.mastodon.social/a/original/b.mp4" type="video/mp4" medium="image"></media:content>"#;
        let adult = r#"<media:content url="https://files.mastodon.social/a/original/c.png" type="image/png"><media:rating scheme="urn:simple">adult</media:rating></media:content>"#;
        let rss = format!(
            r#"<rss version="2.0"><channel>{}{}{}</channel></rss>"#,
            item("ana", image),
            item("bo", video),
            item("cy", adult)
        );
        assert_eq!(
            parse_mastodon(rss.as_bytes()).unwrap(),
            vec![Post {
                title: "Hello #pixelart".into(),
                author: "ana".into(),
                url: "https://mastodon.social/@ana/1".into(),
                image_url: "https://files.mastodon.social/a/small/b.png".into(),
            }]
        );
    }

    #[test]
    fn retronator_takes_the_first_image_of_the_description() {
        let rss = r#"<rss version="2.0"><channel><item><title>Golden Horn</title>
            <description>&lt;figure&gt;&lt;img src="https://64.media.tumblr.com/a/s640x960/b.png"/&gt;&lt;/figure&gt;&lt;img src="https://other.png"/&gt;</description>
            <link>https://www.retronator.com/post/1/golden-horn</link></item>
            <item><title>Text only</title><description>hi</description><link>https://x</link></item>
            </channel></rss>"#;
        assert_eq!(
            parse_retronator(rss.as_bytes()).unwrap(),
            vec![Post {
                title: "Golden Horn".into(),
                author: "Retronator".into(),
                url: "https://www.retronator.com/post/1/golden-horn".into(),
                image_url: "https://64.media.tumblr.com/a/s640x960/b.png".into(),
            }]
        );
    }

    #[test]
    fn pixel_joint_reads_the_gallery_html() {
        let html = r#"<html><div class='imgbox' id='bx17123'><a href='/pixelart/17123.htm' class='imglink'><img src='/files/icons/colossal_katamari_icon.png' alt="Colossal Katamari icon/pixelart" width='100' height='100' /></a></div><div class="tooltip"><strong><a href='/pixelart/17123.htm'>Colossal Katamari</a><br /><div class='small'>By <a href='/p/7755.htm' onmouseover="ddm(this)" target="_top">Snake</a></strong> (Level 2)</div></div></html>"#;
        assert_eq!(
            parse_pixeljoint(html.as_bytes()).unwrap(),
            vec![Post {
                title: "Colossal Katamari".into(),
                author: "Snake".into(),
                url: "https://pixeljoint.com/pixelart/17123.htm".into(),
                image_url: "https://pixeljoint.com/files/icons/colossal_katamari_icon.png".into(),
            }]
        );
        assert_eq!(
            parse_pixeljoint(b"<html>no</html>"),
            Err(FetchError::Invalid)
        );
    }

    #[test]
    fn bluesky_reads_the_author_feed() {
        let json = r#"{"feed":[
            {"post":{"uri":"at://did:plc:x/app.bsky.feed.post/3abc","author":{"handle":"pixelartjourney.bsky.social"},
             "record":{"text":"Cozy Greenhouse\n#pixelart"},
             "embed":{"$type":"app.bsky.embed.images#view","images":[{"thumb":"https://cdn.bsky.app/t.jpg","fullsize":"https://cdn.bsky.app/f.jpg"}]}}},
            {"post":{"uri":"at://did/app.bsky.feed.post/2","author":{"handle":"h"},"record":{"text":"no media"}}}
        ]}"#;
        assert_eq!(
            parse_bluesky(json.as_bytes()).unwrap(),
            vec![Post {
                title: "Cozy Greenhouse #pixelart".into(),
                author: "pixelartjourney.bsky.social".into(),
                url: "https://bsky.app/profile/pixelartjourney.bsky.social/post/3abc".into(),
                image_url: "https://cdn.bsky.app/t.jpg@png".into(),
            }]
        );
    }

    #[test]
    fn helpers_unescape_strip_and_shorten() {
        assert_eq!(
            unescape("a &amp; b &#32;&#x41; &lt;i&gt; &bogus"),
            "a & b  A <i> &bogus"
        );
        assert_eq!(strip_tags("<p>Hi <b>there</b>&nbsp;!</p>"), "Hi there !");
        assert_eq!(headline("Love \u{2764}\u{FE0F} it"), "Love \u{2764} it");
        let long = "x".repeat(100);
        assert_eq!(headline(&long).chars().count(), 81);
        assert_eq!(
            element("<a><title>t</title><titleX>u</titleX></a>", "title"),
            Some("t")
        );
        assert_eq!(element("<b><titleX>u</titleX></b>", "title"), None);
    }

    #[test]
    fn thumbs_decode_png_and_reject_garbage() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert_eq!(decode_thumb(&png).unwrap().size, [3, 2]);
        assert_eq!(decode_thumb(b"not an image"), Err(FetchError::Invalid));
    }

    #[test]
    fn fit_keeps_the_aspect_and_grows_small_art_by_whole_steps() {
        assert_eq!(
            fit(vec2(300.0, 150.0), vec2(150.0, 150.0)),
            vec2(150.0, 75.0)
        );
        assert_eq!(
            fit(vec2(100.0, 100.0), vec2(150.0, 150.0)),
            vec2(100.0, 100.0)
        );
        assert_eq!(
            fit(vec2(50.0, 40.0), vec2(150.0, 150.0)),
            vec2(150.0, 120.0)
        );
    }

    /// Contra las fuentes de verdad: `cargo test -p cs-ui -- --ignored live_news`. Pixel Joint
    /// queda afuera (su sitio pide no ser leído por bots; la app lo pide como cualquier usuario).
    #[test]
    #[ignore]
    #[cfg(not(target_arch = "wasm32"))]
    fn live_news() {
        for source in Source::ALL {
            if source == Source::PixelJoint {
                continue;
            }
            let mut request = ehttp::Request::get(source.feed_url());
            request.headers.insert("User-Agent", "ChimpSprite/test");
            let r = ehttp::fetch_blocking(&request).unwrap();
            lospec_client::check_status(r.status).unwrap();
            let posts = source.parse(&r.bytes).unwrap();
            assert!(!posts.is_empty(), "{source:?}");
            let r = ehttp::fetch_blocking(&ehttp::Request::get(&posts[0].image_url)).unwrap();
            lospec_client::check_status(r.status).unwrap();
            decode_thumb(&r.bytes).unwrap_or_else(|e| panic!("{source:?}: {e:?}"));
        }
    }

    /// Un `Fetcher` que responde enseguida, sin red.
    struct Canned(Vec<Post>);

    impl Fetcher for Canned {
        fn listing(&self, _: &egui::Context, _: Source) -> Pending<Vec<Post>> {
            Pending::ready(Ok(self.0.clone()))
        }
        fn thumb(&self, _: &egui::Context, _: &str) -> Pending<egui::ColorImage> {
            Pending::ready(Ok(egui::ColorImage::new([2, 2], vec![Color32::RED; 4])))
        }
    }

    #[test]
    fn the_feed_shows_the_posts_and_a_click_opens_one() {
        let ctx = egui::Context::default();
        let posts: Vec<Post> = (0..3)
            .map(|i| Post {
                title: format!("Post {i}"),
                author: "ana".into(),
                url: format!("https://example.com/p/{i}"),
                image_url: format!("https://i/{i}.jpg"),
            })
            .collect();
        let fetcher = Canned(posts);
        let mut news = News::default();
        let run = |events: Vec<egui::Event>, news: &mut News| {
            let mut action = None;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 600.0))),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    action = news.ui(ui, &fetcher);
                });
            });
            out.textures_delta.clear();
            let texts: Vec<String> = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                    _ => None,
                })
                .collect();
            (action, texts)
        };
        // Primer frame: pide el listado; el segundo ya lo tiene y pide las miniaturas.
        run(vec![], &mut news);
        let (_, texts) = run(vec![], &mut news);
        assert!(texts.iter().any(|t| t == "Post 0"), "{texts:?}");
        run(vec![], &mut news);
        assert_eq!(
            news.thumbs
                .values()
                .filter(|t| matches!(t, Thumb::Ready(_)))
                .count(),
            3,
            "the visible thumbs arrived"
        );
        // Otra fuente: se pide aparte.
        news.select(Source::Itch);
        run(vec![], &mut news);
        assert!(news.listings.contains_key(&Source::Itch));
        news.select(Source::Reddit);
        // Clic en la primera celda (la miniatura empieza abajo del encabezado).
        let at = egui::pos2(60.0, 120.0);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        run(vec![egui::Event::PointerMoved(at)], &mut news);
        run(vec![press(true)], &mut news);
        let (action, _) = run(vec![press(false)], &mut news);
        match action {
            Some(FeedAction::Open(url)) => assert_eq!(url, "https://example.com/p/0"),
            None => panic!("the click opens the post"),
        }
    }
}
