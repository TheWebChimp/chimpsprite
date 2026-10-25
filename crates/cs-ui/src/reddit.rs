//! El feed de r/PixelArt en Home: el top de la semana como grilla de miniaturas. Se lee el feed
//! RSS público (Reddit ya no deja leer el JSON sin cuenta: 403) y la miniatura de cada post; clic
//! abre el post en el navegador. Solo escritorio: ni el feed ni las imágenes mandan CORS.
//!
//! Se pide una vez por sesión, al mostrar Home (Reddit limita los pedidos: 429), y con Refresh.
//! Las miniaturas, solo las de las celdas visibles y de a `MAX_IN_FLIGHT`; se decodifican en el
//! hilo de la petición y se guardan como texturas.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Rect, RichText, Sense, Vec2, vec2};

use crate::lospec_client::{self, FetchError, Pending};

/// El listado: el feed Atom del top de la semana.
#[cfg_attr(test, allow(dead_code))]
pub const LISTING_URL: &str = "https://www.reddit.com/r/PixelArt/top/.rss?t=week&limit=30";
pub const SUBREDDIT_URL: &str = "https://www.reddit.com/r/PixelArt/top/?t=week";
/// Tope del cuerpo de una miniatura.
const MAX_THUMB_BYTES: usize = 4 * 1024 * 1024;
/// Tope de ancho y alto de una miniatura decodificada.
const MAX_THUMB_SIDE: u32 = 2048;
/// Miniaturas pidiéndose a la vez.
const MAX_IN_FLIGHT: usize = 4;
/// Una petición que no respondió en este tiempo se da por perdida.
const TIMEOUT_SECS: f64 = 20.0;
/// Lado de la caja de cada miniatura y aire entre celdas.
const CELL: f32 = 150.0;
const GAP: f32 = 10.0;

/// Un post con imagen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Post {
    pub title: String,
    pub author: String,
    /// La página del post en reddit.com.
    pub url: String,
    /// La miniatura que se muestra.
    pub image_url: String,
}

/// Lo de adentro de `<tag ...>…</tag>` en `entry` (sin los atributos de la etiqueta).
fn element<'a>(entry: &'a str, tag: &str) -> Option<&'a str> {
    let open = entry.find(&format!("<{tag}"))?;
    let body = open + entry[open..].find('>')? + 1;
    let close = body + entry[body..].find(&format!("</{tag}>"))?;
    Some(&entry[body..close])
}

/// El valor del atributo `attr` de la primera etiqueta `<tag ...>` de `entry`.
fn attribute<'a>(entry: &'a str, tag: &str, attr: &str) -> Option<&'a str> {
    let open = entry.find(&format!("<{tag} "))?;
    let end = open + entry[open..].find('>')?;
    let head = &entry[open..end];
    let key = format!("{attr}=\"");
    let at = head.find(&key)? + key.len();
    let len = head[at..].find('"')?;
    Some(&head[at..at + len])
}

/// Los escapes de XML (y los números `&#N;`).
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

/// Los posts con miniatura del feed Atom de un subreddit. Salta los que no traen miniatura (texto)
/// y los de NSFW (si el feed los marcara).
pub fn parse_listing(bytes: &[u8]) -> Result<Vec<Post>, FetchError> {
    let text = std::str::from_utf8(bytes).map_err(|_| FetchError::Invalid)?;
    if !text.contains("<feed") {
        return Err(FetchError::Invalid);
    }
    Ok(text
        .split("<entry>")
        .skip(1)
        .filter_map(|entry| {
            let entry = entry.split("</entry>").next()?;
            if entry.contains("term=\"nsfw\"") {
                return None;
            }
            let author = element(entry, "name").map(unescape).unwrap_or_default();
            Some(Post {
                title: unescape(element(entry, "title")?.trim()),
                author: author.trim().trim_start_matches("/u/").to_owned(),
                url: unescape(attribute(entry, "link", "href")?),
                image_url: unescape(attribute(entry, "media:thumbnail", "url")?),
            })
        })
        .collect())
}

/// Bytes de una miniatura (JPEG o PNG) → imagen para egui. Corre en el hilo de la petición.
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
    Idle,
    Loading(Pending<Vec<Post>>, f64),
    Ready(Vec<Post>),
    Failed(FetchError),
}

enum Thumb {
    Loading(Pending<egui::ColorImage>, f64),
    Ready(egui::TextureHandle),
    Failed,
}

/// El feed: el listado y las miniaturas que ya llegaron (por URL).
pub struct RedditFeed {
    listing: Listing,
    thumbs: HashMap<String, Thumb>,
}

impl Default for RedditFeed {
    fn default() -> Self {
        Self {
            listing: Listing::Idle,
            thumbs: HashMap::new(),
        }
    }
}

/// Cómo se piden el listado y las miniaturas (los tests no salen a la red).
pub trait Fetcher {
    fn listing(&self, ctx: &egui::Context) -> Pending<Vec<Post>>;
    fn thumb(&self, ctx: &egui::Context, url: &str) -> Pending<egui::ColorImage>;
}

/// La red de verdad, con `ehttp` (los tests de la app usan [`Offline`]).
#[cfg_attr(test, allow(dead_code))]
pub struct Online;

impl Fetcher for Online {
    fn listing(&self, ctx: &egui::Context) -> Pending<Vec<Post>> {
        lospec_client::fetch(ctx, LISTING_URL.to_owned(), |status, bytes| {
            lospec_client::check_status(status)?;
            parse_listing(bytes)
        })
    }

    fn thumb(&self, ctx: &egui::Context, url: &str) -> Pending<egui::ColorImage> {
        lospec_client::fetch(ctx, url.to_owned(), |status, bytes| {
            lospec_client::check_status(status)?;
            decode_thumb(bytes)
        })
    }
}

/// Sin red: el listado nunca llega (los tests de la app muestran Home sin salir a Reddit).
#[cfg(test)]
pub struct Offline;

#[cfg(test)]
impl Fetcher for Offline {
    fn listing(&self, _: &egui::Context) -> Pending<Vec<Post>> {
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

impl RedditFeed {
    /// Entrega lo que llegó (o se venció) desde el frame anterior.
    fn poll(&mut self, ctx: &egui::Context, now: f64) {
        if let Listing::Loading(pending, started) = &self.listing {
            match pending.poll() {
                Some(Ok(posts)) => self.listing = Listing::Ready(posts),
                Some(Err(e)) => self.listing = Listing::Failed(e),
                None if now - started > TIMEOUT_SECS => {
                    self.listing = Listing::Failed(FetchError::Network);
                }
                None => {}
            }
        }
        for (url, thumb) in self.thumbs.iter_mut() {
            if let Thumb::Loading(pending, started) = thumb {
                match pending.poll() {
                    Some(Ok(image)) => {
                        let texture =
                            ctx.load_texture(url.clone(), image, egui::TextureOptions::LINEAR);
                        *thumb = Thumb::Ready(texture);
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

    /// El feed en `ui`: el encabezado y la grilla (con su scroll). Pide el listado la primera
    /// vez y las miniaturas de las celdas visibles.
    pub fn ui(&mut self, ui: &mut egui::Ui, fetcher: &dyn Fetcher) -> Option<FeedAction> {
        let ctx = ui.ctx().clone();
        let now = ui.input(|i| i.time);
        if matches!(self.listing, Listing::Idle) {
            self.listing = Listing::Loading(fetcher.listing(&ctx), now);
        }
        self.poll(&ctx, now);
        let waiting = matches!(self.listing, Listing::Loading(..)) || self.in_flight() > 0;
        // Para revisar los plazos aunque no llegue nada; sin el foco de la ventana, no (lo que
        // llegue pide su repintado igual).
        if waiting && ui.input(|i| i.focused) {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        let mut action = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("r/PixelArt").strong());
            ui.weak("· Top this week");
            if ui
                .small_button("Open")
                .on_hover_text(SUBREDDIT_URL)
                .clicked()
            {
                action = Some(FeedAction::Open(SUBREDDIT_URL.to_owned()));
            }
            let reload = matches!(self.listing, Listing::Ready(_) | Listing::Failed(_));
            if reload && ui.small_button("Refresh").clicked() {
                self.listing = Listing::Idle;
                self.thumbs.clear();
                ctx.request_repaint();
            }
        });
        ui.add_space(6.0);
        let posts = match &self.listing {
            Listing::Idle | Listing::Loading(..) => {
                ui.weak("Loading r/PixelArt…");
                return action;
            }
            Listing::Failed(e) => {
                ui.weak(match e {
                    FetchError::Network => "Couldn't reach Reddit.".to_owned(),
                    FetchError::Status(429) => {
                        "Reddit is busy right now. Try Refresh in a minute.".to_owned()
                    }
                    FetchError::Status(code) => format!("Reddit returned an error ({code})."),
                    _ => "Reddit sent something unexpected.".to_owned(),
                });
                return action;
            }
            Listing::Ready(posts) if posts.is_empty() => {
                ui.weak("No posts with images this week.");
                return action;
            }
            Listing::Ready(posts) => posts.clone(),
        };
        let columns = ((ui.available_width() + GAP) / (CELL + GAP))
            .floor()
            .max(1.0) as usize;
        egui::ScrollArea::vertical()
            .id_salt("reddit_feed")
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

    /// Una celda: la miniatura (o su hueco) y debajo el título, los votos y el autor. Pide la
    /// miniatura si se ve. Devuelve si se hizo clic.
    fn cell(&mut self, ui: &mut egui::Ui, post: &Post, fetcher: &dyn Fetcher, now: f64) -> bool {
        let width = CELL;
        let response = ui
            .vertical(|ui| {
                ui.set_width(width);
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
                        let at = Rect::from_center_size(rect.center(), size);
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
                ui.add(egui::Label::new(post.title.as_str()).truncate());
                ui.weak(format!("u/{}", post.author));
            })
            .response;
        let response = ui
            .interact(response.rect, ui.id().with(&post.url), Sense::click())
            .on_hover_text(post.title.as_str())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response.clicked()
    }
}

/// `size` achicado (o agrandado) para entrar entero en `room`, sin deformar.
fn fit(size: Vec2, room: Vec2) -> Vec2 {
    if size.x <= 0.0 || size.y <= 0.0 {
        return Vec2::ZERO;
    }
    let scale = (room.x / size.x).min(room.y / size.y);
    vec2((size.x * scale).floor(), (size.y * scale).floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una entrada del feed Atom de Reddit (como llega, con los escapes).
    fn entry(title: &str, thumb: bool, nsfw: bool) -> String {
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
            r#"<entry><author><name>/u/ana</name><uri>https://www.reddit.com/user/ana</uri></author>
            <category term="PixelArt" label="r/PixelArt"/>{nsfw}<content type="html">&lt;p&gt;hi&lt;/p&gt;</content>
            <id>t3_x</id>{thumb}<link href="https://www.reddit.com/r/PixelArt/comments/x/{title}/" />
            <title>{title} &amp; &quot;co&quot;</title></entry>"#
        )
    }

    fn feed(entries: &[String]) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><feed xmlns="http://www.w3.org/2005/Atom"><title>top</title>{}</feed>"#,
            entries.concat()
        )
        .into_bytes()
    }

    #[test]
    fn the_feed_keeps_posts_with_a_thumbnail() {
        let bytes = feed(&[
            entry("ok", true, false),
            entry("text", false, false),
            entry("nsfw", true, true),
        ]);
        let posts = parse_listing(&bytes).unwrap();
        assert_eq!(
            posts,
            vec![Post {
                title: "ok & \"co\"".into(),
                author: "ana".into(),
                url: "https://www.reddit.com/r/PixelArt/comments/x/ok/".into(),
                image_url: "https://preview/x.png?width=640&crop=smart".into(),
            }]
        );
    }

    #[test]
    fn a_bad_feed_is_invalid() {
        assert_eq!(
            parse_listing(b"<html>blocked</html>"),
            Err(FetchError::Invalid)
        );
        assert_eq!(parse_listing(&[0xff, 0xfe]), Err(FetchError::Invalid));
    }

    #[test]
    fn xml_escapes_are_undone() {
        assert_eq!(
            unescape("a &amp; b &#32;&#x41; &lt;i&gt; &bogus"),
            "a & b  A <i> &bogus"
        );
    }

    #[test]
    fn thumbs_decode_png_and_reject_garbage() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(3, 2, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let image = decode_thumb(&png).unwrap();
        assert_eq!(image.size, [3, 2]);
        assert_eq!(decode_thumb(b"not an image"), Err(FetchError::Invalid));
    }

    /// Contra reddit.com de verdad: `cargo test -p cs-ui -- --ignored live_reddit`.
    #[test]
    #[ignore]
    #[cfg(not(target_arch = "wasm32"))]
    fn live_reddit() {
        let mut request = ehttp::Request::get(LISTING_URL);
        request.headers.insert("User-Agent", "ChimpSprite/test");
        let r = ehttp::fetch_blocking(&request).unwrap();
        lospec_client::check_status(r.status).unwrap();
        let posts = parse_listing(&r.bytes).unwrap();
        assert!(!posts.is_empty());
        let r = ehttp::fetch_blocking(&ehttp::Request::get(&posts[0].image_url)).unwrap();
        lospec_client::check_status(r.status).unwrap();
        let image = decode_thumb(&r.bytes).unwrap();
        assert!(image.size[0] > 0 && image.size[1] > 0);
    }

    #[test]
    fn fit_keeps_the_aspect() {
        assert_eq!(
            fit(vec2(300.0, 150.0), vec2(150.0, 150.0)),
            vec2(150.0, 75.0)
        );
        assert_eq!(
            fit(vec2(50.0, 100.0), vec2(150.0, 150.0)),
            vec2(75.0, 150.0)
        );
    }

    /// Un `Fetcher` que responde enseguida, sin red.
    struct Canned(Vec<Post>);

    impl Fetcher for Canned {
        fn listing(&self, _: &egui::Context) -> Pending<Vec<Post>> {
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
                url: format!("https://www.reddit.com/p/{i}"),
                image_url: format!("https://i/{i}.jpg"),
            })
            .collect();
        let fetcher = Canned(posts);
        let mut feed = RedditFeed::default();
        let run = |events: Vec<egui::Event>, feed: &mut RedditFeed| {
            let mut action = None;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 600.0))),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    action = feed.ui(ui, &fetcher);
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
        run(vec![], &mut feed);
        let (_, texts) = run(vec![], &mut feed);
        assert!(texts.iter().any(|t| t == "Post 0"), "{texts:?}");
        let (_, _) = run(vec![], &mut feed);
        assert_eq!(
            feed.thumbs
                .values()
                .filter(|t| matches!(t, Thumb::Ready(_)))
                .count(),
            3,
            "the visible thumbs arrived"
        );
        // Clic en la primera celda (la miniatura empieza abajo del encabezado).
        let at = egui::pos2(60.0, 120.0);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        run(vec![egui::Event::PointerMoved(at)], &mut feed);
        run(vec![press(true)], &mut feed);
        let (action, _) = run(vec![press(false)], &mut feed);
        match action {
            Some(FeedAction::Open(url)) => assert_eq!(url, "https://www.reddit.com/p/0"),
            None => panic!("the click opens the post"),
        }
    }
}
