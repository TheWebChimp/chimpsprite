//! Peticiones a Lospec con `ehttp` (`ureq` en un hilo en escritorio, `fetch` en web).
//!
//! Cada petición devuelve un `Pending` que la UI revisa cada frame; al llegar la respuesta se
//! pide un repintado.

use std::sync::{Arc, Mutex};

use cs_io::lospec::{self, LospecError, LospecPalette};
use eframe::egui;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchError {
    /// Sin red, timeout o DNS.
    Network,
    NotFound,
    Status(u16),
    Parse(LospecError),
    /// Imagen de ejemplo demasiado grande o que no es un PNG válido.
    #[cfg(not(target_arch = "wasm32"))]
    BadImage,
}

/// Respuesta que todavía puede no haber llegado. `poll` la entrega una sola vez.
pub struct Pending<T> {
    slot: Arc<Mutex<Option<Result<T, FetchError>>>>,
}

impl<T> Pending<T> {
    pub fn poll(&self) -> Option<Result<T, FetchError>> {
        self.slot.lock().ok()?.take()
    }

    #[cfg(test)]
    pub fn ready(result: Result<T, FetchError>) -> Self {
        Self {
            slot: Arc::new(Mutex::new(Some(result))),
        }
    }

    /// Una petición que nunca responde (conexión colgada).
    #[cfg(test)]
    pub fn never() -> Self {
        Self {
            slot: Arc::new(Mutex::new(None)),
        }
    }
}

/// Lo que se hace con una respuesta: código de estado y luego el parseo.
pub fn classify<T>(
    status: u16,
    bytes: &[u8],
    parse: impl FnOnce(&[u8]) -> Result<T, LospecError>,
) -> Result<T, FetchError> {
    check_status(status)?;
    parse(bytes).map_err(FetchError::Parse)
}

fn check_status(status: u16) -> Result<(), FetchError> {
    match status {
        200..=299 => Ok(()),
        404 => Err(FetchError::NotFound),
        other => Err(FetchError::Status(other)),
    }
}

/// `handle` recibe el código de estado y el cuerpo; corre en el hilo de `ehttp` (en escritorio).
fn fetch<T: Send + 'static>(
    ctx: &egui::Context,
    url: String,
    handle: impl FnOnce(u16, &[u8]) -> Result<T, FetchError> + Send + 'static,
) -> Pending<T> {
    let slot = Arc::new(Mutex::new(None));
    let out = Arc::clone(&slot);
    let ctx = ctx.clone();
    let mut request = ehttp::Request::get(url);
    request.headers.insert(
        "User-Agent",
        concat!("ChimpSprite/", env!("CARGO_PKG_VERSION")),
    );
    ehttp::fetch(request, move |response| {
        let result = match response {
            Ok(r) => handle(r.status, &r.bytes),
            Err(e) => {
                log::warn!("lospec request failed: {e}");
                Err(FetchError::Network)
            }
        };
        if let Ok(mut slot) = out.lock() {
            *slot = Some(result);
        }
        ctx.request_repaint();
    });
    Pending { slot }
}

pub fn fetch_palette(ctx: &egui::Context, slug: &str) -> Pending<LospecPalette> {
    let owned = slug.to_string();
    fetch(ctx, lospec::palette_url(slug), move |s, b| {
        classify(s, b, |b| lospec::parse_palette(b, &owned))
    })
}

/// Solo escritorio: el listado de Lospec no acepta peticiones desde otros sitios (sin CORS).
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_listing(
    ctx: &egui::Context,
    query: &lospec::LospecQuery,
) -> Pending<lospec::LospecPage> {
    fetch(ctx, query.listing_url(), |s, b| {
        classify(s, b, lospec::parse_listing)
    })
}

/// Solo escritorio: el CDN de las imágenes de ejemplo no manda CORS. Se decodifica en el hilo
/// de la petición para no frenar la UI.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_example(ctx: &egui::Context, url: &str) -> Pending<egui::ColorImage> {
    fetch(ctx, url.to_string(), |s, b| {
        check_status(s)?;
        crate::lospec_examples::decode_example(b)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_len(bytes: &[u8]) -> Result<usize, LospecError> {
        if bytes == b"bad" {
            Err(LospecError::InvalidJson)
        } else {
            Ok(bytes.len())
        }
    }

    #[test]
    fn status_codes_map_to_errors() {
        assert_eq!(classify(200, b"abc", parse_len), Ok(3));
        assert_eq!(classify(404, b"", parse_len), Err(FetchError::NotFound));
        assert_eq!(classify(503, b"", parse_len), Err(FetchError::Status(503)));
        assert_eq!(
            classify(200, b"bad", parse_len),
            Err(FetchError::Parse(LospecError::InvalidJson))
        );
    }

    #[test]
    fn pending_hands_the_result_once() {
        let p = Pending::ready(Ok(5));
        assert_eq!(p.poll(), Some(Ok(5)));
        assert_eq!(p.poll(), None);
    }

    /// Contra lospec.com de verdad: `cargo test -p cs-ui -- --ignored live_lospec`.
    #[test]
    #[ignore]
    #[cfg(not(target_arch = "wasm32"))]
    fn live_lospec() {
        let get = |url: String| ehttp::fetch_blocking(&ehttp::Request::get(url)).unwrap();
        let q = lospec::LospecQuery {
            tag: "gameboy".into(),
            colors: lospec::ColorCount::Any,
            sort: lospec::LospecSort::Default,
            page: 0,
        };
        let r = get(q.listing_url());
        let page = classify(r.status, &r.bytes, lospec::parse_listing).unwrap();
        assert!(!page.palettes.is_empty());
        let url = page.palettes[0].examples[0].clone();
        let r = get(url);
        check_status(r.status).unwrap();
        let image = crate::lospec_examples::decode_example(&r.bytes).unwrap();
        assert!(image.size[0] > 0 && image.size[1] > 0);
        let r = get(lospec::palette_url("sweetie-16"));
        let p = classify(r.status, &r.bytes, |b| {
            lospec::parse_palette(b, "sweetie-16")
        })
        .unwrap();
        assert_eq!(p.palette.len(), 16);
    }
}
