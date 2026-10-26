//! Con Pixel UI, el Home del diseño (frame 5 de `Design.aseprite`): a la izquierda el logo y los
//! botones New file / Open file / Open demo, separados por una línea grabada vertical; a la
//! derecha Recent Files y Recent Folders (nombre y ruta en dos líneas) y, al lado, el feed de
//! r/PixelArt. Sin barra de estado.

// En web no hay recientes: las medidas de las listas quedan sin usar.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_variables))]

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Ui, Vec2, pos2, vec2};

use crate::ChimpApp;
use crate::pixel_draw;
use crate::pixel_ui::Tip as _;
use crate::skin::Skin;

/// Medidas del diseño, desde la esquina de arriba a la izquierda del contenido.
const LOGO_AT: Vec2 = vec2(6.0, 21.0);
const BUTTONS_AT: Vec2 = vec2(12.0, 103.0);
const BUTTON_SIZE: Vec2 = vec2(71.0, 17.0);
/// De un botón al de su derecha (New file, Open file en la primera fila; Open demo debajo).
const BUTTON_STEP_X: f32 = 74.0;
/// De un botón al siguiente.
const BUTTON_PITCH: f32 = 20.0;
/// Dentro del botón: el texto empieza en la x 15 con las mayúsculas en la fila 4; el ícono, en
/// la fila 4.
const BUTTON_TEXT_LEFT: f32 = 15.0;
const BUTTON_CAPS_TOP: f32 = 4.0;
const BUTTON_ICON_TOP: f32 = 4.0;
/// La línea grabada vertical (oscura y clara) que separa la columna de la izquierda.
const DIVIDER_X: f32 = 168.0;
/// Las listas: el ícono del título en la x 177 (su caja), el texto en la 193; la caja del
/// primer título en la fila 37.
const LIST_ICON_X: f32 = 177.0;
const LIST_TEXT_X: f32 = 193.0;
const FIRST_HEADING_TOP: f32 = 37.0;
/// Dentro de la fila del título: las mayúsculas en la 5.
const HEADING_CAPS_TOP: f32 = 5.0;
/// Del título al primer elemento (mayúsculas del nombre) y de un elemento al siguiente.
const HEADING_TO_ITEM: f32 = 21.0;
const ITEM_PITCH: f32 = 32.0;
/// La ruta, debajo del nombre.
const PATH_BELOW: f32 = 10.0;
/// Del último elemento de una lista al título de la siguiente (además de `ITEM_PITCH`).
const LIST_GAP: f32 = 21.0;
/// Hasta dónde llegan los textos de las listas: a la izquierda de Pixnews.
const LIST_RIGHT: f32 = 384.0;
/// Pixnews (frame 5): el título (pieza) y el subtítulo, la línea de abajo, la columna de íconos
/// de las fuentes y la línea vertical que la separa del feed.
const NEWS_TITLE_AT: Vec2 = vec2(393.0, 13.0);
const NEWS_SUBTITLE_CAPS: f32 = 27.0;
const NEWS_LINE_Y: f32 = 37.0;
/// El borde izquierdo de la pestaña activa, los íconos y la línea vertical.
const NEWS_TAB_LEFT: f32 = 391.0;
const NEWS_ICON_X: f32 = 393.0;
const NEWS_DIVIDER_X: f32 = 421.0;
/// El primer ícono y de uno al siguiente; cada pestaña mide 32 de alto.
const NEWS_FIRST_ICON: f32 = 39.0;
const NEWS_TAB_STEP: f32 = 32.0;
/// El aire del feed dentro de su área.
const NEWS_PAD: f32 = 10.0;

/// Los íconos: piezas del skin.
const ICON_NEW: &str = "home.icon.new";
const ICON_OPEN: &str = "home.icon.open";
const ICON_DEMO: &str = "home.icon.demo";
const ICON_FILES: &str = "home.icon.files";
const ICON_FOLDERS: &str = "home.icon.folders";
/// Fila de la caja del título donde empieza cada ícono.
const FILES_ICON_TOP: f32 = 3.0;
const FOLDERS_ICON_TOP: f32 = 4.0;

fn text_shape(ui: &Ui, text: &str, left: f32, cap_top: f32, color: Color32) -> (Shape, f32) {
    pixel_draw::caps_text(ui, text, left, cap_top, color)
}

/// Lo que se pidió desde Home.
enum HomeAction {
    New,
    Open,
    Demo,
    #[cfg(not(target_arch = "wasm32"))]
    OpenRecent(std::path::PathBuf),
    #[cfg(not(target_arch = "wasm32"))]
    OpenIn(std::path::PathBuf),
}

/// Un botón de la columna: la pieza `button` del skin, el ícono y el texto.
fn button(ui: &mut Ui, skin: &Skin, rect: Rect, icon: (&str, f32), label: &str) -> bool {
    let info = pixel_draw::ButtonInfo::new(ui.id().with(("home_button", label)), label);
    let (icon, icon_left) = icon;
    let ppp = ui.ctx().pixels_per_point();
    // El texto se arma antes (necesita `ui`) y baja con el botón apretado.
    let left = rect.left() + BUTTON_TEXT_LEFT;
    let (mut text, _) = text_shape(
        ui,
        label,
        left,
        rect.top() + BUTTON_CAPS_TOP,
        skin.colors().text,
    );
    pixel_draw::skin_button(ui, skin, rect, info, |r| {
        let at = r.min + vec2(icon_left, BUTTON_ICON_TOP);
        let mut shapes = pixel_draw::piece_at(skin, icon, at, ppp);
        text.translate(r.min - rect.min);
        shapes.push(text);
        shapes
    })
    .clicked()
}

/// Un elemento de una lista: el nombre (clic) y la ruta debajo, en `top` (mayúsculas del
/// nombre). Los textos se recortan a `right`.
fn item(
    ui: &mut Ui,
    skin: &Skin,
    origin: Pos2,
    top: f32,
    right: f32,
    name: &str,
    path: &str,
) -> bool {
    let colors = skin.colors();
    let left = origin.x + LIST_TEXT_X;
    let clip = Rect::from_x_y_ranges(left..=right, top - 4.0..=top + PATH_BELOW + 10.0);
    let (name_shape, width) = text_shape(ui, name, left, top, colors.link);
    // La ruta, en los textos chicos.
    let (path_shape, _) = pixel_draw::caps_text_in(
        ui,
        egui::TextStyle::Small,
        path,
        left,
        top + PATH_BELOW,
        colors.path,
    );
    let painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
    painter.add(name_shape);
    painter.add(path_shape);
    let hit = Rect::from_min_size(pos2(left, top - 2.0), vec2(width.min(right - left), 11.0));
    let response = ui
        .interact(hit, ui.id().with(("home_item", path, name)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .tip(path);
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, enabled, name));
    if response.hovered() {
        let y = (top + 8.0).round();
        painter.hline(left..=left + width, y, (1.0, colors.link));
    }
    response.clicked()
}

impl ChimpApp {
    /// Pixnews: el título, la columna de fuentes (la elegida, en su pestaña) y el feed de la
    /// elegida. Devuelve la URL a abrir, si se tocó algo.
    #[cfg(not(target_arch = "wasm32"))]
    fn pixnews(&mut self, ui: &mut Ui, origin: Pos2, area: Rect) -> Option<String> {
        use crate::news::Source;
        let skin = self.pixel.skin()?;
        let colors = *skin.colors();
        let ppp = ui.ctx().pixels_per_point();
        let mut shapes = pixel_draw::piece_at(skin, "news.title", origin + NEWS_TITLE_AT, ppp);
        let subtitle = "Latest news in the pixel art world";
        let left = origin.x + NEWS_TITLE_AT.x;
        let caps = origin.y + NEWS_SUBTITLE_CAPS;
        shapes.push(text_shape(ui, subtitle, left, caps, colors.text).0);
        // Las líneas (1 px, el color `canvas`): la de abajo del título, la vertical y el marco de
        // la pestaña elegida, que las une abierto hacia el feed.
        let line = colors.canvas;
        let px = |x0: f32, x1: f32, y0: f32, y1: f32| {
            Shape::rect_filled(
                Rect::from_min_max(
                    pos2(origin.x + x0, origin.y + y0),
                    pos2(origin.x + x1, origin.y + y1),
                ),
                0.0,
                line,
            )
        };
        let right = area.right() - origin.x - 1.0;
        let bottom = area.bottom() - origin.y;
        let selected = Source::ALL
            .iter()
            .position(|s| *s == self.news.source)
            .unwrap_or(0) as f32;
        let top = NEWS_LINE_Y + selected * NEWS_TAB_STEP;
        let (l, d) = (NEWS_TAB_LEFT, NEWS_DIVIDER_X);
        if selected == 0.0 {
            shapes.push(px(l + 3.0, right, NEWS_LINE_Y, NEWS_LINE_Y + 1.0));
        } else {
            shapes.push(px(d, right, NEWS_LINE_Y, NEWS_LINE_Y + 1.0));
            shapes.push(px(d, d + 1.0, NEWS_LINE_Y, top));
            shapes.push(px(l + 3.0, d + 1.0, top, top + 1.0));
        }
        // Las esquinas redondeadas y el lado izquierdo.
        shapes.push(px(l + 2.0, l + 3.0, top + 1.0, top + 2.0));
        shapes.push(px(l + 1.0, l + 2.0, top + 2.0, top + 3.0));
        shapes.push(px(l, l + 1.0, top + 3.0, top + 29.0));
        shapes.push(px(l + 1.0, l + 2.0, top + 29.0, top + 30.0));
        shapes.push(px(l + 2.0, l + 3.0, top + 30.0, top + 31.0));
        shapes.push(px(l + 3.0, d + 1.0, top + 31.0, top + 32.0));
        shapes.push(px(d, d + 1.0, top + 31.0, bottom));
        ui.painter().extend(shapes);
        // Los íconos de las fuentes: clic elige.
        for (i, source) in Source::ALL.into_iter().enumerate() {
            let at = origin + vec2(NEWS_ICON_X, NEWS_FIRST_ICON + i as f32 * NEWS_TAB_STEP);
            let size = pixel_draw::piece_size(skin, source.icon());
            let rect = Rect::from_min_size(at, size);
            ui.painter()
                .extend(pixel_draw::piece_at(skin, source.icon(), at, ppp));
            let response = ui
                .interact(rect, ui.id().with(("news_source", i)), Sense::click())
                .tip(format!("{} · {}", source.label(), source.detail()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let on = self.news.source == source;
            let enabled = ui.is_enabled();
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, source.label())
            });
            if response.clicked() {
                self.news.select(source);
            }
        }
        // El feed de la elegida, a la derecha de la línea vertical.
        let feed = Rect::from_min_max(
            pos2(
                origin.x + d + 1.0 + NEWS_PAD,
                origin.y + NEWS_LINE_Y + 1.0 + NEWS_PAD,
            ),
            pos2(area.right() - NEWS_PAD, area.bottom() - NEWS_PAD),
        );
        if feed.width() < 160.0 {
            return None;
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(feed));
        self.news
            .ui(&mut child, self.news_fetcher.as_ref())
            .map(|crate::news::FeedAction::Open(url)| url)
    }

    /// El Home de Pixel UI en todo `ui` (ver el módulo).
    pub(crate) fn pixel_home(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let area = ui.max_rect();
        let origin = area.min.round();
        let mut action = None;
        {
            let Some(skin) = self.pixel.skin() else {
                return;
            };
            // El logo (o, si no se pudo leer, el nombre).
            let logo = self
                .logo
                .get_or_insert_with(|| crate::logo::Logo::embedded(&ctx));
            match logo {
                Some(logo) => logo.paint_at(ui, origin + LOGO_AT),
                None => {
                    let (name, _) = text_shape(
                        ui,
                        "ChimpSprite",
                        origin.x + 12.0,
                        origin.y + 40.0,
                        skin.colors().text,
                    );
                    ui.painter().add(name);
                }
            }
            let buttons: [(&str, f32, &str, HomeAction); 3] = [
                (ICON_NEW, 5.0, "New file", HomeAction::New),
                // Tres puntos de texto, como en el diseño (no `…`).
                (ICON_OPEN, 2.0, "Open file", HomeAction::Open),
                (ICON_DEMO, 3.0, "Open demo", HomeAction::Demo),
            ];
            for (i, (rows, left, label, a)) in buttons.into_iter().enumerate() {
                // Dos por fila.
                let (col, row) = ((i % 2) as f32, (i / 2) as f32);
                let at = origin + BUTTONS_AT + vec2(col * BUTTON_STEP_X, row * BUTTON_PITCH);
                if button(
                    ui,
                    skin,
                    Rect::from_min_size(at, BUTTON_SIZE),
                    (rows, left),
                    label,
                ) {
                    action = Some(a);
                }
            }
            // La línea grabada vertical, a todo el alto.
            let colors = skin.colors();
            let x = origin.x + DIVIDER_X;
            let dark = Rect::from_x_y_ranges(x..=x + 1.0, area.y_range());
            ui.painter().rect_filled(dark, 0.0, colors.canvas);
            ui.painter()
                .rect_filled(dark.translate(vec2(1.0, 0.0)), 0.0, colors.highlight);
            // Las listas.
            let list_right = origin.x + LIST_RIGHT;
            let ppp = ui.ctx().pixels_per_point();
            let heading = |ui: &mut Ui, top: f32, icon: &str, icon_top: f32, title: &str| {
                let at = pos2(origin.x + LIST_ICON_X, top + icon_top);
                let mut shapes = pixel_draw::piece_at(skin, icon, at, ppp);
                let (text, _) = text_shape(
                    ui,
                    title,
                    origin.x + LIST_TEXT_X,
                    top + HEADING_CAPS_TOP,
                    colors.text,
                );
                shapes.push(text);
                ui.painter().extend(shapes);
            };
            let mut top = origin.y + FIRST_HEADING_TOP;
            #[cfg(not(target_arch = "wasm32"))]
            {
                let files = self.recent.files().to_vec();
                let folders = self.recent.folders();
                heading(ui, top, ICON_FILES, FILES_ICON_TOP, "Recent Files");
                let mut y = top + HEADING_TO_ITEM;
                if files.is_empty() {
                    let (note, _) =
                        text_shape(ui, "Nothing yet.", origin.x + LIST_TEXT_X, y, colors.path);
                    ui.painter().add(note);
                    y += ITEM_PITCH;
                }
                for path in &files {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    let dir = path.parent().map(|d| d.display().to_string());
                    let shown = dir.unwrap_or_default();
                    if item(ui, skin, origin, y, list_right, name, &shown) {
                        action = Some(HomeAction::OpenRecent(path.clone()));
                    }
                    y += ITEM_PITCH;
                }
                top = y - HEADING_TO_ITEM + LIST_GAP;
                heading(ui, top, ICON_FOLDERS, FOLDERS_ICON_TOP, "Recent Folders");
                let mut y = top + HEADING_TO_ITEM;
                for dir in &folders {
                    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    if item(
                        ui,
                        skin,
                        origin,
                        y,
                        list_right,
                        name,
                        &dir.display().to_string(),
                    ) {
                        action = Some(HomeAction::OpenIn(dir.clone()));
                    }
                    y += ITEM_PITCH;
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                heading(ui, top, ICON_FILES, FILES_ICON_TOP, "In the browser");
                let lines = [
                    "Files open from your computer and save as downloads.",
                    "Unsaved documents are kept in this browser.",
                ];
                for (i, line) in lines.iter().enumerate() {
                    let y = top + HEADING_TO_ITEM + i as f32 * PATH_BELOW;
                    let (t, _) = text_shape(ui, line, origin.x + LIST_TEXT_X, y, colors.path);
                    ui.painter().add(t);
                }
                let _ = &mut top;
            }
        }
        // A la derecha de las listas, Pixnews (solo escritorio: no todas las fuentes mandan CORS).
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(url) = self.pixnews(ui, origin, area) {
            ctx.open_url(egui::OpenUrl::new_tab(url));
        }
        match action {
            Some(HomeAction::New) => self.new_document(),
            Some(HomeAction::Open) => self.open_dialog(&ctx),
            Some(HomeAction::Demo) => self.open_demo(),
            #[cfg(not(target_arch = "wasm32"))]
            Some(HomeAction::OpenRecent(path)) => {
                if path.exists() {
                    self.platform
                        .open_path(&path, crate::platform::OpenPurpose::Open);
                } else {
                    self.recent.remove(&path);
                    self.show_message("File not found", path.display().to_string());
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            Some(HomeAction::OpenIn(dir)) => {
                self.platform.open_file_in(
                    crate::file_actions::SPRITE_FILTERS,
                    &dir,
                    crate::platform::OpenPurpose::Open,
                    &ctx,
                );
            }
            None => {}
        }
        ui.allocate_rect(area, Sense::hover());
    }
}
