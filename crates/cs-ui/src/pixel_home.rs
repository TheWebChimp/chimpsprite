//! Con Pixel UI, el Home del diseño (frame 5 de `Design.aseprite`): a la izquierda el logo y los
//! botones New file / Open file… / Open demo, separados por una línea grabada vertical; a la
//! derecha Recent Files y Recent Folders (nombre y ruta en dos líneas) y, al lado, el feed de
//! r/PixelArt. Sin barra de estado.

// En web no hay recientes: las medidas de las listas quedan sin usar.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_variables))]

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Ui, Vec2, pos2, vec2};

use crate::ChimpApp;
use crate::pixel_draw;
use crate::skin::Skin;

/// Medidas del diseño, desde la esquina de arriba a la izquierda del contenido.
const LOGO_AT: Vec2 = vec2(6.0, 21.0);
const BUTTONS_AT: Vec2 = vec2(12.0, 103.0);
const BUTTON_SIZE: Vec2 = vec2(78.0, 17.0);
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
/// Ancho de las listas; a su derecha va el feed.
const LIST_WIDTH: f32 = 230.0;
const FEED_GAP: f32 = 24.0;

/// Colores del diseño que el skin no tiene: el nombre de cada reciente y su ruta.
const NAME_COLOR: Color32 = Color32::from_rgb(0xb2, 0x52, 0x66);
const PATH_COLOR: Color32 = Color32::from_rgb(0x69, 0x66, 0x82);

/// Íconos del diseño. `a` contorno, `j` el tono claro, `.` el fondo de la app, `d` tinta, `A`–`C`
/// los grises de la hoja, `D`–`E` los amarillos de la carpeta; ` ` transparente.
const ICON_NEW: [&str; 7] = [
    "aaaa  ", //
    "a  .a ", //
    "a  ..a", //
    "a    a", //
    "a    a", //
    "a    a", //
    "aaaaaa", //
];
const ICON_OPEN: [&str; 8] = [
    "       aaj",
    "  jaaaajja",
    "  ajjjjjja",
    " aaaaaajja",
    " a     aja",
    "  a    aja",
    "  a     aa",
    "  jaaaaaaj",
];
const ICON_FILES: [&str; 11] = [
    "   dddddddd  ",
    "   dAccccBAd ",
    "   dcccccBAAd",
    "   dcccccBBBd",
    "   dccccccccd",
    "   dcBCccCBcd",
    "   dccccccccd",
    "   dcBCCCCBcd",
    "   dccccccccd",
    "   dAccccccAd",
    "   dddddddddd",
];
const ICON_FOLDERS: [&str; 9] = [
    "   ddddd      ",
    "  dDDDDEddddd ",
    "  dDDDDDDDDDEd",
    "  dDDDDDDDDDDd",
    "  dDDDDDDDDDDd",
    "  dEDDDDDDDDDd",
    "  dDEDDDDDDDDd",
    "  dEDEDDDDDDEd",
    "   dddddddddd ",
];
/// Fila de la caja del título donde empieza cada ícono.
const FILES_ICON_TOP: f32 = 3.0;
const FOLDERS_ICON_TOP: f32 = 4.0;

fn icon_color(c: u8) -> Option<Color32> {
    Some(match c {
        b'a' => Color32::from_rgb(0x59, 0x4d, 0x4d),
        b'j' => Color32::from_rgb(0xbd, 0xa4, 0x99),
        b'.' => Color32::from_rgb(0xdd, 0xce, 0xbf),
        b'c' => Color32::from_rgb(0xea, 0xe0, 0xdd),
        b'd' => Color32::from_rgb(0x05, 0x04, 0x03),
        b'A' => Color32::from_rgb(0xcb, 0xc6, 0xc1),
        b'B' => Color32::from_rgb(0xa6, 0x9e, 0x9a),
        b'C' => Color32::from_rgb(0x59, 0x57, 0x57),
        b'D' => Color32::from_rgb(0xf8, 0xc5, 0x3a),
        b'E' => Color32::from_rgb(0xe8, 0x8a, 0x36),
        _ => return None,
    })
}

fn icon_shapes(rows: &[&str], at: Pos2) -> Vec<Shape> {
    pixel_draw::ascii_shapes(rows, at, icon_color)
}

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
fn button(ui: &mut Ui, skin: &Skin, rect: Rect, icon: (&[&str], f32), label: &str) -> bool {
    let info = pixel_draw::ButtonInfo::new(ui.id().with(("home_button", label)), label);
    let (rows, icon_left) = icon;
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
        let mut shapes = icon_shapes(rows, r.min + vec2(icon_left, BUTTON_ICON_TOP));
        text.translate(r.min - rect.min);
        shapes.push(text);
        shapes
    })
    .clicked()
}

/// Un elemento de una lista: el nombre (clic) y la ruta debajo, en `top` (mayúsculas del
/// nombre). Los textos se recortan a `right`.
fn item(ui: &mut Ui, origin: Pos2, top: f32, right: f32, name: &str, path: &str) -> bool {
    let left = origin.x + LIST_TEXT_X;
    let clip = Rect::from_x_y_ranges(left..=right, top - 4.0..=top + PATH_BELOW + 10.0);
    let (name_shape, width) = text_shape(ui, name, left, top, NAME_COLOR);
    let (path_shape, _) = text_shape(ui, path, left, top + PATH_BELOW, PATH_COLOR);
    let painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
    painter.add(name_shape);
    painter.add(path_shape);
    let hit = Rect::from_min_size(pos2(left, top - 2.0), vec2(width.min(right - left), 11.0));
    let response = ui
        .interact(hit, ui.id().with(("home_item", path, name)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(path);
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, enabled, name));
    if response.hovered() {
        let y = (top + 8.0).round();
        painter.hline(left..=left + width, y, (1.0, NAME_COLOR));
    }
    response.clicked()
}

impl ChimpApp {
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
            let buttons: [(&[&str], f32, &str, HomeAction); 3] = [
                (&ICON_NEW, 5.0, "New file", HomeAction::New),
                (&ICON_OPEN, 2.0, "Open file…", HomeAction::Open),
                (&ICON_OPEN, 2.0, "Open demo", HomeAction::Demo),
            ];
            for (i, (rows, left, label, a)) in buttons.into_iter().enumerate() {
                let at = origin + BUTTONS_AT + vec2(0.0, i as f32 * BUTTON_PITCH);
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
            let list_right = origin.x + LIST_TEXT_X + LIST_WIDTH;
            let heading = |ui: &mut Ui, top: f32, icon: &[&str], icon_top: f32, title: &str| {
                let mut shapes = icon_shapes(icon, pos2(origin.x + LIST_ICON_X, top + icon_top));
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
                heading(ui, top, &ICON_FILES, FILES_ICON_TOP, "Recent Files");
                let mut y = top + HEADING_TO_ITEM;
                if files.is_empty() {
                    let (note, _) =
                        text_shape(ui, "Nothing yet.", origin.x + LIST_TEXT_X, y, PATH_COLOR);
                    ui.painter().add(note);
                    y += ITEM_PITCH;
                }
                for path in &files {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    let dir = path.parent().map(|d| d.display().to_string());
                    let shown = dir.unwrap_or_default();
                    if item(ui, origin, y, list_right, name, &shown) {
                        action = Some(HomeAction::OpenRecent(path.clone()));
                    }
                    y += ITEM_PITCH;
                }
                top = y - HEADING_TO_ITEM + LIST_GAP;
                heading(ui, top, &ICON_FOLDERS, FOLDERS_ICON_TOP, "Recent Folders");
                let mut y = top + HEADING_TO_ITEM;
                for dir in &folders {
                    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    if item(ui, origin, y, list_right, name, &dir.display().to_string()) {
                        action = Some(HomeAction::OpenIn(dir.clone()));
                    }
                    y += ITEM_PITCH;
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                heading(ui, top, &ICON_FILES, FILES_ICON_TOP, "In the browser");
                let lines = [
                    "Files open from your computer and save as downloads.",
                    "Unsaved documents are kept in this browser.",
                ];
                for (i, line) in lines.iter().enumerate() {
                    let y = top + HEADING_TO_ITEM + i as f32 * PATH_BELOW;
                    let (t, _) = text_shape(ui, line, origin.x + LIST_TEXT_X, y, PATH_COLOR);
                    ui.painter().add(t);
                }
                let _ = &mut top;
            }
        }
        // A la derecha de las listas, el feed de r/PixelArt (solo escritorio).
        #[cfg(not(target_arch = "wasm32"))]
        {
            let left = origin.x + LIST_TEXT_X + LIST_WIDTH + FEED_GAP;
            let feed = Rect::from_min_max(
                pos2(left, origin.y + FIRST_HEADING_TOP),
                pos2(area.right() - 16.0, area.bottom() - 8.0),
            );
            if feed.width() > 160.0 {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(feed));
                let fetched = self.reddit.ui(&mut child, self.reddit_fetcher.as_ref());
                if let Some(crate::reddit::FeedAction::Open(url)) = fetched {
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
            }
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
