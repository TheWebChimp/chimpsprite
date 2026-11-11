//! El registro de paneles: una ficha por panel de la app (título, ícono, anchos y dónde vive).
//! Es la única fuente de esos datos: el dock, la ventana de paneles y los diálogos leen de aquí en
//! lugar de repetirlos cada uno por su lado.

use crate::dock;

/// Cómo va el contenido de un panel flotante.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Body {
    /// Con scroll, del alto de su contenido.
    Fit,
    /// Con scroll, de la columna entera.
    Tall,
    /// Sin scroll, de la columna entera.
    Fixed,
}

/// Dónde vive un panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Host {
    /// Un panel del árbol del workspace que no es del dock (la toolbar): acoplado, flotando o en
    /// la tira (ahí su botón abre su flyout, como los del dock).
    Tree,
    /// La columna fija de la derecha (el color).
    Right,
    /// La franja de abajo (el timeline).
    Bottom,
    /// Una ventanita flotante (el preview).
    Window,
    /// El centro del workspace: la hoja protegida del árbol (el lienzo).
    Center,
    /// Flotando junto a su botón del dock (su cuerpo lo dice `PanelSpec::body`).
    Flyout,
}

/// Lo más angosto que el workspace deja el lienzo mismo cuando reparte un split que no alcanza
/// (ver `workspace::fit`); su hoja suma la tira de íconos o el panel 3D fijo, que en la fase D
/// todavía van dentro de ella (ver `ChimpApp::canvas_area_min`).
pub(crate) const CANVAS_MIN_WIDTH: f32 = 64.0;
/// Lo más bajo que el workspace deja el lienzo mismo (su caja y sus reglas); su hoja suma el
/// timeline cuando va dentro de ella (ver `ChimpApp::canvas_area_min`).
pub(crate) const CANVAS_MIN_HEIGHT: f32 = 64.0;

/// Los paneles de la app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelId {
    /// El lienzo: la hoja protegida al centro del workspace (no se cierra, no se arrastra, no
    /// recibe pestañas y existe siempre una vez, en la superficie principal).
    Canvas,
    /// La barra de herramientas: un panel del árbol (fase D), a la izquierda del lienzo en
    /// Essentials.
    Tools,
    /// El editor de color y la paleta.
    Color,
    /// La línea de tiempo.
    Timeline,
    /// La ventanita de preview.
    Preview,
    Artboards,
    Library,
    Slices,
    /// Las paletas de Lospec.
    Lospec,
    /// Las fuentes de dafont (solo escritorio: en la web el sitio no acepta peticiones de otras
    /// páginas).
    #[cfg(not(target_arch = "wasm32"))]
    Dafont,
    /// Groom: de pixel art falso a pixel art de verdad.
    Groom,
    /// Lumen: normales y luz para el sprite.
    Lumen,
    /// La fuente pixel del artboard activo: su mapa de caracteres.
    Font,
    /// El modelo 3D del artboard activo: sus objetos.
    Model,
}

/// La ficha de un panel.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PanelSpec {
    /// Su llave estable: es lo que se guarda en el acomodo (no cambia aunque cambie el título).
    pub key: &'static str,
    /// El título: el rótulo de su botón y su identidad en la memoria de egui.
    pub title: &'static str,
    /// Su ícono: una pieza del skin. `icon.tools` e `icon.color` todavía no existen en el skin
    /// (los usará la fase C); con `plain` basta mientras tanto.
    pub icon: &'static str,
    /// El texto del botón sin Pixel UI (o si el skin no trae el ícono).
    pub plain: &'static str,
    /// Su ancho de diseño (para la columna de abajo, su alto de fábrica).
    pub reference_width: f32,
    /// Lo más angosto en que se ve bien.
    pub min_width: f32,
    /// Si el usuario puede cerrarlo.
    pub closable: bool,
    pub host: Host,
    /// Cómo va su contenido (vale para cualquier anfitrión, no solo el flotante).
    pub body: Body,
    /// Lo más bajo en que se ve bien (0.0: sin mínimo propio).
    pub min_height: f32,
}

impl PanelId {
    /// Todos los paneles.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) const ALL: &[PanelId] = &[
        PanelId::Canvas,
        PanelId::Tools,
        PanelId::Color,
        PanelId::Timeline,
        PanelId::Preview,
        PanelId::Artboards,
        PanelId::Library,
        PanelId::Slices,
        PanelId::Lospec,
        PanelId::Dafont,
        PanelId::Groom,
        PanelId::Lumen,
        PanelId::Font,
        PanelId::Model,
    ];
    /// Todos los paneles (en la web no hay dafont).
    #[cfg(target_arch = "wasm32")]
    pub(crate) const ALL: &[PanelId] = &[
        PanelId::Canvas,
        PanelId::Tools,
        PanelId::Color,
        PanelId::Timeline,
        PanelId::Preview,
        PanelId::Artboards,
        PanelId::Library,
        PanelId::Slices,
        PanelId::Lospec,
        PanelId::Groom,
        PanelId::Lumen,
        PanelId::Font,
        PanelId::Model,
    ];

    /// Los botones del dock, en su orden.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) const DOCK: &[PanelId] = &[
        PanelId::Artboards,
        PanelId::Library,
        PanelId::Slices,
        PanelId::Lospec,
        PanelId::Dafont,
        PanelId::Groom,
        PanelId::Lumen,
        PanelId::Font,
        PanelId::Model,
    ];
    /// Los botones del dock, en su orden (en la web no hay dafont).
    #[cfg(target_arch = "wasm32")]
    pub(crate) const DOCK: &[PanelId] = &[
        PanelId::Artboards,
        PanelId::Library,
        PanelId::Slices,
        PanelId::Lospec,
        PanelId::Groom,
        PanelId::Lumen,
        PanelId::Font,
        PanelId::Model,
    ];

    /// Su ficha.
    pub(crate) fn spec(self) -> PanelSpec {
        let flyout = |key, title, icon, plain, reference_width, min_width, body| PanelSpec {
            key,
            title,
            icon,
            plain,
            reference_width,
            min_width,
            closable: true,
            host: Host::Flyout,
            body,
            min_height: 0.0,
        };
        match self {
            PanelId::Canvas => PanelSpec {
                key: "canvas",
                title: "Canvas",
                icon: "icon.canvas",
                plain: "▦",
                reference_width: CANVAS_MIN_WIDTH,
                min_width: CANVAS_MIN_WIDTH,
                closable: false,
                host: Host::Center,
                body: Body::Fixed,
                min_height: CANVAS_MIN_HEIGHT,
            },
            PanelId::Tools => PanelSpec {
                key: "tools",
                title: "Tools",
                icon: "icon.tools",
                plain: "✎",
                // Sin Pixel UI; con él, el ancho real sale del skin (`pixel_ui::toolbar_width`,
                // ver `workspace::fit::minima`), como el mínimo de Color.
                reference_width: crate::pixel_ui::TOOLBAR_WIDTH_PLAIN,
                min_width: crate::pixel_ui::TOOLBAR_WIDTH_PLAIN,
                closable: true,
                host: Host::Tree,
                body: Body::Fixed,
                min_height: 0.0,
            },
            PanelId::Color => PanelSpec {
                key: "color",
                title: "Color",
                icon: "icon.color",
                plain: "🎨",
                reference_width: crate::color_panel::DEFAULT_WIDTH,
                // Por ahora; el mínimo real depende del skin y lo calcula `color_panel::panel_min_size`.
                min_width: crate::color_panel::DEFAULT_WIDTH,
                closable: false,
                host: Host::Right,
                body: Body::Fixed,
                min_height: 0.0,
            },
            PanelId::Timeline => PanelSpec {
                key: "timeline",
                title: dock::TIMELINE,
                icon: dock::TIMELINE_ICON,
                plain: "🎞",
                // Abajo, el "ancho" es el alto de fábrica.
                reference_width: dock::TIMELINE_DEFAULT_HEIGHT,
                min_width: dock::TIMELINE_DEFAULT_HEIGHT,
                closable: true,
                host: Host::Bottom,
                body: Body::Fixed,
                min_height: 0.0,
            },
            PanelId::Preview => PanelSpec {
                key: "preview",
                title: dock::PREVIEW,
                icon: dock::PREVIEW_ICON,
                plain: "▣",
                reference_width: crate::preview::VIEW.x,
                min_width: crate::preview::MIN_VIEW.x,
                closable: true,
                host: Host::Window,
                body: Body::Fixed,
                min_height: 0.0,
            },
            PanelId::Artboards => flyout(
                "artboards",
                "Artboards",
                "icon.dock.artboards",
                "🗋",
                dock::FLYOUT_WIDTH,
                dock::FLYOUT_WIDTH,
                Body::Fit,
            ),
            PanelId::Slices => flyout(
                "slices",
                "Slices",
                "icon.dock.slices",
                "#",
                dock::FLYOUT_WIDTH,
                dock::FLYOUT_WIDTH,
                Body::Fit,
            ),
            PanelId::Lospec => flyout(
                "lospec",
                "Lospec",
                "icon.dock.lospec",
                "L",
                crate::palette_ui::LOSPEC_PANEL_W,
                crate::palette_ui::LOSPEC_PANEL_W,
                Body::Tall,
            ),
            #[cfg(not(target_arch = "wasm32"))]
            PanelId::Dafont => flyout(
                "dafont",
                "Dafont",
                "icon.dock.dafont",
                "Aa",
                crate::dafont::REFERENCE_WIDTH,
                crate::dafont::MIN_WIDTH,
                Body::Tall,
            ),
            PanelId::Groom => PanelSpec {
                min_height: crate::groom::MIN_HEIGHT,
                ..flyout(
                    "groom",
                    "Groom",
                    "icon.dock.groom",
                    "▦",
                    crate::groom::REFERENCE_WIDTH,
                    crate::groom::MIN_WIDTH,
                    Body::Tall,
                )
            },
            PanelId::Lumen => PanelSpec {
                min_height: crate::lumen::MIN_HEIGHT,
                ..flyout(
                    "lumen",
                    "Lumen",
                    "icon.dock.lumen",
                    "☀",
                    crate::lumen::REFERENCE_WIDTH,
                    crate::lumen::MIN_WIDTH,
                    Body::Tall,
                )
            },
            PanelId::Library => flyout(
                "library",
                "Library",
                "icon.dock.library",
                "▣",
                crate::library_panel::REFERENCE_WIDTH,
                crate::library_panel::MIN_WIDTH,
                Body::Tall,
            ),
            PanelId::Font => flyout(
                "font",
                "FontForge",
                "icon.dock.font",
                "FF",
                crate::font_panel::REFERENCE_WIDTH,
                crate::font_panel::MIN_WIDTH,
                Body::Fixed,
            ),
            PanelId::Model => flyout(
                "model",
                "3D",
                "icon.dock.model",
                "3D",
                crate::model_panel::REFERENCE_WIDTH,
                crate::model_panel::MIN_WIDTH,
                Body::Fixed,
            ),
        }
    }

    /// Su título (el rótulo de su botón).
    pub fn label(self) -> &'static str {
        self.spec().title
    }

    /// Su llave estable (la de su ficha): lo que se guarda en el acomodo.
    pub(crate) fn key(self) -> &'static str {
        self.spec().key
    }

    /// El panel con esa llave; `None` si no existe (o no está en esta plataforma: en la web no
    /// hay dafont).
    pub(crate) fn from_key(key: &str) -> Option<PanelId> {
        Self::ALL.iter().copied().find(|p| p.key() == key)
    }
}

/// Se guarda por su llave, no por el nombre de la variante: renombrar una no rompe lo guardado.
impl serde::Serialize for PanelId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

impl<'de> serde::Deserialize<'de> for PanelId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = <std::borrow::Cow<str>>::deserialize(deserializer)?;
        PanelId::from_key(&key)
            .ok_or_else(|| serde::de::Error::custom(format!("panel desconocido: {key:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dock_buttons_keep_their_order_titles_and_icons() {
        let titles: Vec<&str> = PanelId::DOCK.iter().map(|p| p.spec().title).collect();
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(
            titles,
            [
                "Artboards",
                "Library",
                "Slices",
                "Lospec",
                "Dafont",
                "Groom",
                "Lumen",
                "FontForge",
                "3D"
            ]
        );
        let icons: Vec<&str> = PanelId::DOCK.iter().map(|p| p.spec().icon).collect();
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(
            icons,
            [
                "icon.dock.artboards",
                "icon.dock.library",
                "icon.dock.slices",
                "icon.dock.lospec",
                "icon.dock.dafont",
                "icon.dock.groom",
                "icon.dock.lumen",
                "icon.dock.font",
                "icon.dock.model"
            ]
        );
        for p in PanelId::DOCK {
            assert!(matches!(p.spec().host, Host::Flyout), "{p:?}");
            assert_eq!(p.label(), p.spec().title);
        }
    }

    #[test]
    fn widths_come_from_each_panel_module() {
        assert_eq!(
            PanelId::Groom.spec().reference_width,
            crate::groom::REFERENCE_WIDTH
        );
        assert_eq!(PanelId::Groom.spec().min_width, crate::groom::MIN_WIDTH);
        assert_eq!(PanelId::Lumen.spec().min_width, crate::lumen::MIN_WIDTH);
        assert_eq!(
            PanelId::Library.spec().min_width,
            crate::library_panel::MIN_WIDTH
        );
        assert_eq!(PanelId::Font.spec().min_width, crate::font_panel::MIN_WIDTH);
        assert_eq!(
            PanelId::Model.spec().min_width,
            crate::model_panel::MIN_WIDTH
        );
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(PanelId::Dafont.spec().min_width, crate::dafont::MIN_WIDTH);
        for p in PanelId::ALL {
            let s = p.spec();
            assert!(
                s.min_width > 0.0 && s.min_width <= s.reference_width,
                "{p:?}"
            );
        }
    }

    #[test]
    fn keys_are_stable_unique_and_round_trip() {
        let mut seen = std::collections::HashSet::new();
        for &p in PanelId::ALL {
            assert!(seen.insert(p.key()), "{p:?}");
            assert_eq!(PanelId::from_key(p.key()), Some(p));
            let json = serde_json::to_string(&p).unwrap();
            assert_eq!(json, format!("\"{}\"", p.key()));
            assert_eq!(serde_json::from_str::<PanelId>(&json).unwrap(), p);
        }
        assert_eq!(PanelId::Font.key(), "font");
        assert_eq!(PanelId::from_key("nope"), None);
        assert!(serde_json::from_str::<PanelId>("\"nope\"").is_err());
        // En la web no hay dafont: su llave no se reconoce.
        #[cfg(target_arch = "wasm32")]
        assert_eq!(PanelId::from_key("dafont"), None);
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(PanelId::from_key("dafont"), Some(PanelId::Dafont));
    }

    #[test]
    fn the_body_and_min_height_are_in_the_spec() {
        assert_eq!(PanelId::Groom.spec().min_height, crate::groom::MIN_HEIGHT);
        assert_eq!(PanelId::Lumen.spec().min_height, crate::lumen::MIN_HEIGHT);
        assert_eq!(PanelId::Font.spec().min_height, 0.0);
        assert!(matches!(PanelId::Font.spec().body, Body::Fixed));
        assert!(matches!(PanelId::Tools.spec().body, Body::Fixed));
        assert!(matches!(PanelId::Artboards.spec().body, Body::Fit));
        assert!(matches!(PanelId::Library.spec().body, Body::Tall));
    }

    #[test]
    fn every_panel_has_one_host_and_the_fixed_ones_are_where_they_are_today() {
        assert!(matches!(PanelId::Canvas.spec().host, Host::Center));
        assert!(!PanelId::Canvas.spec().closable);
        assert_eq!(PanelId::Canvas.key(), "canvas");
        assert!(matches!(PanelId::Tools.spec().host, Host::Tree));
        assert!(PanelId::Tools.spec().closable);
        assert!(matches!(PanelId::Color.spec().host, Host::Right));
        assert!(matches!(PanelId::Timeline.spec().host, Host::Bottom));
        assert!(matches!(PanelId::Preview.spec().host, Host::Window));
        // `Host::Flyout` ya no lleva el cuerpo: está en `spec.body`.
        for (id, body) in [
            (PanelId::Font, Body::Fixed),
            (PanelId::Artboards, Body::Fit),
            (PanelId::Groom, Body::Tall),
        ] {
            let spec = id.spec();
            assert!(matches!(spec.host, Host::Flyout), "{id:?}");
            assert_eq!(spec.body, body, "{id:?}");
        }
        let mut seen = std::collections::HashSet::new();
        for p in PanelId::ALL {
            assert!(seen.insert(p.spec().title), "title repeated: {p:?}");
        }
    }
}
