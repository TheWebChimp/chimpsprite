//! Estado de la pestaña Lospec, sin egui ni red: la UI pide con `start_*`, hace la petición y
//! entrega la respuesta con `finish_*`. Las respuestas de búsquedas viejas se descartan.

use cs_io::lospec::{
    ColorCount, LospecPage, LospecPalette, LospecQuery, LospecSort, slug_from_input,
};

use crate::lospec_client::FetchError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CountKind {
    #[default]
    Any,
    Exact,
    Min,
    Max,
}

pub struct SearchStart {
    pub generation: u64,
    pub listing: LospecQuery,
    /// Slug a buscar en paralelo si el texto también podría ser el nombre de una paleta.
    pub exact: Option<String>,
}

pub struct LospecTab {
    pub tag: String,
    pub count_kind: CountKind,
    /// 1–256; solo cuenta si `count_kind` no es `Any`.
    pub count: u16,
    pub sort: LospecSort,
    pub link: String,
    query: Option<LospecQuery>,
    results: Vec<LospecPalette>,
    exact: Option<LospecPalette>,
    total: u32,
    generation: u64,
    /// Generación de la última `start_search`; la coincidencia exacta solo vale para esa.
    search_generation: u64,
    listing_loading: bool,
    lookup: Option<LospecPalette>,
    lookup_generation: u64,
    lookup_loading: bool,
    error: Option<String>,
}

impl Default for LospecTab {
    fn default() -> Self {
        Self {
            tag: String::new(),
            count_kind: CountKind::Any,
            count: 8,
            sort: LospecSort::Default,
            link: String::new(),
            query: None,
            results: Vec::new(),
            exact: None,
            total: 0,
            generation: 0,
            search_generation: 0,
            listing_loading: false,
            lookup: None,
            lookup_generation: 0,
            lookup_loading: false,
            error: None,
        }
    }
}

impl LospecTab {
    pub fn start_search(&mut self) -> SearchStart {
        let n = self.count.clamp(1, 256);
        let colors = match self.count_kind {
            CountKind::Any => ColorCount::Any,
            CountKind::Exact => ColorCount::Exact(n),
            CountKind::Min => ColorCount::Min(n),
            CountKind::Max => ColorCount::Max(n),
        };
        let query = LospecQuery {
            tag: self.tag.clone(),
            colors,
            sort: self.sort,
            page: 0,
        };
        self.generation += 1;
        self.search_generation = self.generation;
        self.query = Some(query.clone());
        self.results.clear();
        self.exact = None;
        self.total = 0;
        self.listing_loading = true;
        self.error = None;
        SearchStart {
            generation: self.generation,
            listing: query,
            exact: slug_from_input(&self.tag),
        }
    }

    pub fn start_more(&mut self) -> Option<(u64, LospecQuery)> {
        if !self.can_load_more() || self.listing_loading {
            return None;
        }
        let query = self.query.as_mut()?;
        query.page += 1;
        let query = query.clone();
        self.generation += 1;
        self.listing_loading = true;
        self.error = None;
        Some((self.generation, query))
    }

    pub fn start_lookup(&mut self) -> Option<(u64, String)> {
        self.lookup = None;
        let Some(slug) = slug_from_input(&self.link) else {
            self.error = Some("Paste a lospec.com palette link or a palette name.".into());
            return None;
        };
        self.lookup_generation += 1;
        self.lookup_loading = true;
        self.error = None;
        Some((self.lookup_generation, slug))
    }

    pub fn finish_listing(&mut self, generation: u64, result: Result<LospecPage, FetchError>) {
        if generation != self.generation {
            return;
        }
        self.listing_loading = false;
        match result {
            Ok(page) => {
                self.total = page.total;
                self.results.extend(page.palettes);
            }
            Err(FetchError::Parse(_)) => {
                self.error =
                    Some("Lospec search is unavailable. Paste a palette link instead.".into())
            }
            Err(e) => self.error = Some(message(&e)),
        }
    }

    /// La búsqueda exacta por slug es un extra: si falla, no se muestra error.
    pub fn finish_exact(&mut self, generation: u64, result: Result<LospecPalette, FetchError>) {
        if generation == self.search_generation
            && let Ok(p) = result
        {
            self.exact = Some(p);
        }
    }

    pub fn finish_lookup(&mut self, generation: u64, result: Result<LospecPalette, FetchError>) {
        if generation != self.lookup_generation {
            return;
        }
        self.lookup_loading = false;
        match result {
            Ok(p) => self.lookup = Some(p),
            Err(e) => self.error = Some(message(&e)),
        }
    }

    pub fn results(&self) -> Vec<&LospecPalette> {
        let exact_slug = self.exact.as_ref().map(|p| p.slug.as_str());
        self.exact
            .iter()
            .chain(
                self.results
                    .iter()
                    .filter(|p| Some(p.slug.as_str()) != exact_slug),
            )
            .collect()
    }

    pub fn is_exact(&self, slug: &str) -> bool {
        self.exact.as_ref().is_some_and(|p| p.slug == slug)
    }

    pub fn lookup_result(&self) -> Option<&LospecPalette> {
        self.lookup.as_ref()
    }

    pub fn can_load_more(&self) -> bool {
        (self.results.len() as u32) < self.total
    }

    pub fn listing_loading(&self) -> bool {
        self.listing_loading
    }

    pub fn lookup_loading(&self) -> bool {
        self.lookup_loading
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Para "Load more (10 of 412)".
    pub fn shown_of_total(&self) -> (usize, u32) {
        (self.results.len(), self.total)
    }
}

fn message(e: &FetchError) -> String {
    match e {
        FetchError::Network => "Couldn't reach Lospec.".into(),
        FetchError::NotFound => "Palette not found on Lospec.".into(),
        FetchError::Status(code) => format!("Lospec returned an error ({code})."),
        FetchError::Parse(_) => "That palette can't be loaded.".into(),
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{Palette, Rgba};
    use cs_io::lospec::LospecError;

    use super::*;

    fn pal(slug: &str) -> LospecPalette {
        LospecPalette {
            slug: slug.into(),
            title: slug.to_uppercase(),
            author: None,
            palette: Palette::new(vec![Rgba::from_hex(0x102030)]),
            likes: None,
            downloads: None,
        }
    }

    fn page(slugs: &[&str], total: u32) -> LospecPage {
        LospecPage {
            palettes: slugs.iter().map(|s| pal(s)).collect(),
            total,
        }
    }

    #[test]
    fn search_resets_and_load_more_appends() {
        let mut tab = LospecTab {
            tag: "gameboy".into(),
            ..LospecTab::default()
        };
        let s = tab.start_search();
        assert_eq!(s.listing.page, 0);
        assert_eq!(s.listing.tag, "gameboy");
        assert!(tab.listing_loading());
        tab.finish_listing(s.generation, Ok(page(&["a", "b"], 3)));
        assert!(!tab.listing_loading());
        assert!(tab.can_load_more());
        let (g, q) = tab.start_more().unwrap();
        assert_eq!(q.page, 1);
        tab.finish_listing(g, Ok(page(&["c"], 3)));
        assert_eq!(tab.results().len(), 3);
        assert!(!tab.can_load_more());
        assert_eq!(tab.shown_of_total(), (3, 3));
        let again = tab.start_search();
        assert!(tab.results().is_empty());
        assert_eq!(again.listing.page, 0);
    }

    #[test]
    fn color_filter_builds_the_query() {
        let mut tab = LospecTab {
            count_kind: CountKind::Max,
            count: 4,
            ..LospecTab::default()
        };
        assert_eq!(tab.start_search().listing.colors, ColorCount::Max(4));
        tab.count_kind = CountKind::Any;
        assert_eq!(tab.start_search().listing.colors, ColorCount::Any);
    }

    #[test]
    fn exact_match_goes_first_without_duplicates() {
        let mut tab = LospecTab {
            tag: "Sweetie 16".into(),
            ..LospecTab::default()
        };
        let s = tab.start_search();
        assert_eq!(s.exact.as_deref(), Some("sweetie-16"));
        tab.finish_listing(s.generation, Ok(page(&["x", "sweetie-16"], 2)));
        tab.finish_exact(s.generation, Ok(pal("sweetie-16")));
        let slugs: Vec<&str> = tab.results().iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(slugs, ["sweetie-16", "x"]);
        assert!(tab.is_exact("sweetie-16"));
    }

    #[test]
    fn exact_lookup_failure_is_silent() {
        let mut tab = LospecTab {
            tag: "nothing".into(),
            ..LospecTab::default()
        };
        let s = tab.start_search();
        tab.finish_exact(s.generation, Err(FetchError::NotFound));
        tab.finish_listing(s.generation, Ok(page(&[], 0)));
        assert_eq!(tab.error(), None);
    }

    #[test]
    fn exact_match_arrives_after_load_more() {
        let mut tab = LospecTab {
            tag: "sweetie-16".into(),
            ..LospecTab::default()
        };
        let s = tab.start_search();
        tab.finish_listing(s.generation, Ok(page(&["x"], 2)));
        let (g, _) = tab.start_more().unwrap();
        assert_ne!(g, s.generation);
        // La exacta de la búsqueda original llega con la generación vieja, pero sigue vigente.
        tab.finish_exact(s.generation, Ok(pal("sweetie-16")));
        assert!(tab.is_exact("sweetie-16"));
    }

    #[test]
    fn stale_exact_after_load_more_is_ignored() {
        let mut tab = LospecTab {
            tag: "Sweetie 16".into(),
            ..LospecTab::default()
        };
        let a = tab.start_search();
        let b = tab.start_search();
        tab.finish_listing(b.generation, Ok(page(&["x"], 2)));
        let (g, _) = tab.start_more().unwrap();
        tab.finish_exact(a.generation, Ok(pal("sweetie-16")));
        assert!(!tab.is_exact("sweetie-16"));
        assert_eq!(tab.results().len(), 1);
        tab.finish_listing(g, Ok(page(&["y"], 2)));
        assert_eq!(tab.results().len(), 2);
    }

    #[test]
    fn stale_responses_are_ignored() {
        let mut tab = LospecTab::default();
        let old = tab.start_search();
        let new = tab.start_search();
        tab.finish_listing(old.generation, Ok(page(&["old"], 1)));
        assert!(tab.results().is_empty());
        assert!(tab.listing_loading());
        tab.finish_listing(new.generation, Ok(page(&["new"], 1)));
        assert_eq!(tab.results()[0].slug, "new");
    }

    #[test]
    fn error_messages() {
        let cases = [
            (FetchError::Network, "Couldn't reach Lospec."),
            (FetchError::Status(500), "Lospec returned an error (500)."),
            (
                FetchError::Parse(LospecError::InvalidJson),
                "Lospec search is unavailable. Paste a palette link instead.",
            ),
        ];
        for (err, msg) in cases {
            let mut tab = LospecTab::default();
            let s = tab.start_search();
            tab.finish_listing(s.generation, Err(err));
            assert_eq!(tab.error(), Some(msg));
        }
        let mut tab = LospecTab {
            link: "sweetie-16".into(),
            ..LospecTab::default()
        };
        let (g, slug) = tab.start_lookup().unwrap();
        assert_eq!(slug, "sweetie-16");
        tab.finish_lookup(g, Err(FetchError::NotFound));
        assert_eq!(tab.error(), Some("Palette not found on Lospec."));
        let (g, _) = tab.start_lookup().unwrap();
        tab.finish_lookup(g, Err(FetchError::Parse(LospecError::TooManyColors)));
        assert_eq!(tab.error(), Some("That palette can't be loaded."));
        let (g, _) = tab.start_lookup().unwrap();
        tab.finish_lookup(g, Ok(pal("sweetie-16")));
        assert_eq!(tab.error(), None);
        assert_eq!(tab.lookup_result().unwrap().slug, "sweetie-16");
        tab.link = "https://example.com/x".into();
        assert!(tab.start_lookup().is_none());
        assert_eq!(
            tab.error(),
            Some("Paste a lospec.com palette link or a palette name.")
        );
    }
}
