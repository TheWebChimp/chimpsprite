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
}

/// Lo que se hace con una respuesta: código de estado y luego el parseo.
pub fn classify<T>(
    status: u16,
    bytes: &[u8],
    parse: impl FnOnce(&[u8]) -> Result<T, LospecError>,
) -> Result<T, FetchError> {
    match status {
        200..=299 => parse(bytes).map_err(FetchError::Parse),
        404 => Err(FetchError::NotFound),
        other => Err(FetchError::Status(other)),
    }
}

fn fetch<T: Send + 'static>(
    ctx: &egui::Context,
    url: String,
    parse: impl FnOnce(&[u8]) -> Result<T, LospecError> + Send + 'static,
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
            Ok(r) => classify(r.status, &r.bytes, parse),
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
    fetch(ctx, lospec::palette_url(slug), move |b| {
        lospec::parse_palette(b, &owned)
    })
}

/// Solo escritorio: el listado de Lospec no acepta peticiones desde otros sitios (sin CORS).
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_listing(
    ctx: &egui::Context,
    query: &lospec::LospecQuery,
) -> Pending<lospec::LospecPage> {
    fetch(ctx, query.listing_url(), lospec::parse_listing)
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
}
