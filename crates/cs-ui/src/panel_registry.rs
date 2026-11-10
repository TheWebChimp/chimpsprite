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
    /// La columna fija de la izquierda (las herramientas).
    Left,
    /// La columna fija de la derecha (el color).
    Right,
    /// La franja de abajo (el timeline).
    Bottom,
    /// Una ventanita flotante (el preview).
    Window,
    /// Flotando junto a su botón del dock, con este cuerpo.
    Flyout(Body),
}

/// Los paneles de la app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelId {
    /// La barra de herramientas.
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
    /// Las paletas de Lospec (su contenido lo pone la app, ver [`dock::DockOutput::app_panel`]).
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
#[expect(
    dead_code,
    reason = "`min_width` y `closable` los leerá el dibujo de paneles de las siguientes tareas"
)]
pub(crate) struct PanelSpec {
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
}

impl PanelId {
    /// Todos los paneles.
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "la ventana de paneles lo recorrerá")
    )]
    pub(crate) const ALL: &[PanelId] = &[
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
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "la ventana de paneles lo recorrerá")
    )]
    pub(crate) const ALL: &[PanelId] = &[
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
        let flyout = |title, icon, plain, reference_width, min_width, body| PanelSpec {
            title,
            icon,
            plain,
            reference_width,
            min_width,
            closable: true,
            host: Host::Flyout(body),
        };
        match self {
            PanelId::Tools => PanelSpec {
                title: "Tools",
                icon: "icon.tools",
                plain: "✎",
                reference_width: crate::pixel_ui::TOOLBAR_WIDTH_PLAIN,
                min_width: crate::pixel_ui::TOOLBAR_WIDTH_PLAIN,
                closable: false,
                host: Host::Left,
            },
            PanelId::Color => PanelSpec {
                title: "Color",
                icon: "icon.color",
                plain: "🎨",
                reference_width: crate::color_panel::DEFAULT_WIDTH,
                // Por ahora; el mínimo real depende del skin y lo calcula `color_panel::panel_min_size`.
                min_width: crate::color_panel::DEFAULT_WIDTH,
                closable: false,
                host: Host::Right,
            },
            PanelId::Timeline => PanelSpec {
                title: dock::TIMELINE,
                icon: dock::TIMELINE_ICON,
                plain: "🎞",
                // Abajo, el "ancho" es el alto de fábrica.
                reference_width: dock::TIMELINE_DEFAULT_HEIGHT,
                min_width: dock::TIMELINE_DEFAULT_HEIGHT,
                closable: true,
                host: Host::Bottom,
            },
            PanelId::Preview => PanelSpec {
                title: dock::PREVIEW,
                icon: dock::PREVIEW_ICON,
                plain: "▣",
                reference_width: crate::preview::VIEW.x,
                min_width: crate::preview::MIN_VIEW.x,
                closable: true,
                host: Host::Window,
            },
            PanelId::Artboards => flyout(
                "Artboards",
                "icon.dock.artboards",
                "🗋",
                dock::FLYOUT_WIDTH,
                dock::FLYOUT_WIDTH,
                Body::Fit,
            ),
            PanelId::Slices => flyout(
                "Slices",
                "icon.dock.slices",
                "#",
                dock::FLYOUT_WIDTH,
                dock::FLYOUT_WIDTH,
                Body::Fit,
            ),
            PanelId::Lospec => flyout(
                "Lospec",
                "icon.dock.lospec",
                "L",
                crate::palette_ui::LOSPEC_PANEL_W,
                crate::palette_ui::LOSPEC_PANEL_W,
                Body::Tall,
            ),
            #[cfg(not(target_arch = "wasm32"))]
            PanelId::Dafont => flyout(
                "Dafont",
                "icon.dock.dafont",
                "Aa",
                crate::dafont::REFERENCE_WIDTH,
                crate::dafont::MIN_WIDTH,
                Body::Tall,
            ),
            PanelId::Groom => flyout(
                "Groom",
                "icon.dock.groom",
                "▦",
                crate::groom::REFERENCE_WIDTH,
                crate::groom::MIN_WIDTH,
                Body::Tall,
            ),
            PanelId::Lumen => flyout(
                "Lumen",
                "icon.dock.lumen",
                "☀",
                crate::lumen::REFERENCE_WIDTH,
                crate::lumen::MIN_WIDTH,
                Body::Tall,
            ),
            PanelId::Library => flyout(
                "Library",
                "icon.dock.library",
                "▣",
                crate::library_panel::REFERENCE_WIDTH,
                crate::library_panel::MIN_WIDTH,
                Body::Tall,
            ),
            PanelId::Font => flyout(
                "FontForge",
                "icon.dock.font",
                "FF",
                crate::font_panel::REFERENCE_WIDTH,
                crate::font_panel::MIN_WIDTH,
                Body::Fixed,
            ),
            PanelId::Model => flyout(
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
            assert!(matches!(p.spec().host, Host::Flyout(_)), "{p:?}");
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
    fn every_panel_has_one_host_and_the_fixed_ones_are_where_they_are_today() {
        assert!(matches!(PanelId::Tools.spec().host, Host::Left));
        assert!(matches!(PanelId::Color.spec().host, Host::Right));
        assert!(matches!(PanelId::Timeline.spec().host, Host::Bottom));
        assert!(matches!(PanelId::Preview.spec().host, Host::Window));
        assert!(matches!(
            PanelId::Font.spec().host,
            Host::Flyout(Body::Fixed)
        ));
        assert!(matches!(
            PanelId::Artboards.spec().host,
            Host::Flyout(Body::Fit)
        ));
        assert!(matches!(
            PanelId::Groom.spec().host,
            Host::Flyout(Body::Tall)
        ));
        let mut seen = std::collections::HashSet::new();
        for p in PanelId::ALL {
            assert!(seen.insert(p.spec().title), "title repeated: {p:?}");
        }
    }
}
