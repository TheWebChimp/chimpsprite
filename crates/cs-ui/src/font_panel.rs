//! El panel Font del dock: la fuente pixel del artboard activo. Sin fuente, el botón New Font…;
//! con fuente, el mapa de caracteres por páginas (ASCII, Latin-1, Latin Ext-A y los de la
//! fuente): cada celda con su carácter y la miniatura de su glifo. Un clic va al glifo, un doble
//! clic en uno que falta lo agrega y va a él, y el menú de cada celda copia, pega, borra o quita.

use cs_core::FontInfo;
use eframe::egui::{
    self, Align2, Color32, Rect, Sense, Stroke, StrokeKind, TextStyle, Ui, Vec2, vec2,
};

use crate::editor::Editor;
use crate::kit;
use crate::menus;
use crate::panel_kit::label_with_hint;
use crate::pixel_sections;
use crate::pixel_ui;
use crate::skin::Skin;

/// Ancho del contenido.
pub const WIDTH: f32 = 300.0;
/// Una celda del mapa: el carácter arriba y la miniatura del glifo abajo.
const CELL_W: f32 = 22.0;
const CELL_H: f32 = 24.0;
const COLUMNS: usize = 13;
/// Alto del carácter de arriba de la celda (la miniatura va debajo).
const CELL_LABEL: f32 = 11.0;
/// Ancho del campo "Add char".
const ADD_WIDTH: f32 = 120.0;

/// El estado del panel entre frames.
pub struct FontPanelState {
    pub page: Page,
    /// Lo escrito en el campo "Add char".
    pub add_char: String,
    /// El texto de prueba de la fuente (lo escribe el Test Drive, que llega después).
    #[allow(dead_code)]
    pub test_text: String,
}

impl Default for FontPanelState {
    fn default() -> Self {
        FontPanelState {
            page: Page::default(),
            add_char: String::new(),
            test_text: "The quick brown fox".to_owned(),
        }
    }
}

/// Las páginas del mapa.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Ascii,
    Latin1,
    LatinExtA,
    /// Los caracteres que tiene la fuente.
    All,
}

impl Page {
    const ALL: [Page; 4] = [Page::Ascii, Page::Latin1, Page::LatinExtA, Page::All];

    fn label(self) -> &'static str {
        match self {
            Page::Ascii => "ASCII",
            Page::Latin1 => "Latin-1",
            Page::LatinExtA => "Latin Ext-A",
            Page::All => "All",
        }
    }

    /// Los caracteres de la página, en orden.
    pub fn chars(self, font: &FontInfo) -> Vec<char> {
        match self {
            Page::Ascii => ('!'..='~').collect(),
            Page::Latin1 => ('¡'..='ÿ').collect(),
            Page::LatinExtA => ('Ā'..='ſ').collect(),
            Page::All => font.glyphs.iter().map(|g| g.ch).collect(),
        }
    }
}

/// Lo que el usuario pidió en el panel (lo resuelve la app).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontAction {
    NewFont,
}

pub fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut FontPanelState,
) -> Option<FontAction> {
    let Some(count) = editor.font().map(|f| f.glyphs.len()) else {
        let new = match skin {
            Some(skin) => kit::button::button(ui, skin, "New Font…"),
            None => pixel_ui::text_button(ui, "New Font…"),
        };
        ui.add_space(4.0);
        ui.add(egui::Label::new("The active artboard is not a font.").wrap());
        return new.clicked().then_some(FontAction::NewFont);
    };
    let note = match count {
        1 => "1 glyph".to_owned(),
        n => format!("{n} glyphs"),
    };
    label_with_hint(ui, "Map", &note);
    ui.add_space(4.0);
    pages(ui, skin, state);
    map(ui, editor, skin, state);
    ui.add_space(6.0);
    add_chars(ui, editor, skin, state);
    None
}

/// La tira de las páginas: las pestañas del kit (sin Pixel UI, etiquetas de egui).
fn pages(ui: &mut Ui, skin: Option<&Skin>, state: &mut FontPanelState) {
    let active = Page::ALL.iter().position(|p| *p == state.page);
    match skin {
        Some(skin) => {
            let tabs: Vec<pixel_sections::Tab> = Page::ALL
                .iter()
                .map(|p| pixel_sections::Tab {
                    name: p.label().to_owned(),
                    color: None,
                })
                .collect();
            if let Some(i) = pixel_sections::tabs(ui, skin, &tabs, active) {
                state.page = Page::ALL[i];
            }
        }
        None => {
            ui.horizontal(|ui| {
                for page in Page::ALL {
                    if ui
                        .selectable_label(state.page == page, page.label())
                        .clicked()
                    {
                        state.page = page;
                    }
                }
            });
        }
    }
}

/// La escala de la miniatura de un glifo de `cell` (en puntos por pixel): 1 si cabe en la
/// celda; si no, la que cabe, hacia abajo en pasos de 0.5 (mínimo 0.5).
fn thumb_scale(cell: (u32, u32)) -> f32 {
    let (w, h) = (cell.0.max(1) as f32, cell.1.max(1) as f32);
    let fit = ((CELL_W - 4.0) / w).min((CELL_H - CELL_LABEL - 2.0) / h);
    if fit >= 1.0 {
        1.0
    } else {
        ((fit * 2.0).floor() / 2.0).max(0.5)
    }
}

/// La cuadrícula de la página: blanco lo que la fuente tiene, apagado lo que falta, con marco el
/// glifo actual.
fn map(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &FontPanelState) {
    let Some(font) = editor.font().cloned() else {
        return;
    };
    let chars = state.page.chars(&font);
    let current = editor.current_glyph();
    let view = editor.artboard().font_view();
    let cell_size = (editor.sprite().width, editor.sprite().height);
    let scale = thumb_scale(cell_size);
    let rows = chars.len().div_ceil(COLUMNS);
    let (rect, _) = ui.allocate_exact_size(
        vec2(CELL_W * COLUMNS as f32, CELL_H * rows as f32),
        Sense::hover(),
    );
    let origin = rect.min.round();
    let painter = ui.painter().clone();
    let colors = skin.map(|s| *s.colors());
    let (panel, text, text_dim, accent) = colors.map_or(
        (
            Color32::from_gray(200),
            Color32::BLACK,
            Color32::from_gray(120),
            Color32::from_rgb(226, 114, 133),
        ),
        |c| (c.panel, c.text, c.text_dim, c.accent),
    );
    let font_id = TextStyle::Body.resolve(ui.style());
    let enabled = ui.is_enabled();
    for (i, &ch) in chars.iter().enumerate() {
        let cell = Rect::from_min_size(
            origin + vec2((i % COLUMNS) as f32 * CELL_W, (i / COLUMNS) as f32 * CELL_H),
            vec2(CELL_W, CELL_H),
        )
        .shrink(1.0);
        let response = ui.interact(cell, ui.id().with(("font_map", ch)), Sense::click());
        let is_current = Some(ch) == current;
        let label = format!("Glyph {ch}");
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, is_current, &label)
        });
        let has = font.index_of(ch).is_some();
        let (paper, ink) = if has {
            (Color32::WHITE, text)
        } else {
            (panel, text_dim)
        };
        painter.rect_filled(cell, 0.0, paper);
        painter.text(
            cell.center_top() + vec2(0.0, 1.0),
            Align2::CENTER_TOP,
            ch.to_string(),
            font_id.clone(),
            ink,
        );
        if let Some(glyph) = view.as_ref().and_then(|v| v.glyphs.get(&ch)) {
            // La miniatura, abajo y centrada.
            let size = vec2(cell_size.0 as f32, cell_size.1 as f32) * scale;
            let thumb =
                Rect::from_center_size(cell.center_bottom() - vec2(0.0, 1.0 + size.y / 2.0), size);
            let at = thumb.min.round();
            for &(x, y, c) in &glyph.pixels {
                let color = c.map_or(text, |c| {
                    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
                });
                painter.rect_filled(
                    Rect::from_min_size(at + vec2(x as f32, y as f32) * scale, Vec2::splat(scale)),
                    0.0,
                    color,
                );
            }
        }
        if is_current {
            painter.rect_stroke(cell, 0.0, Stroke::new(1.0, accent), StrokeKind::Inside);
        }
        if response.double_clicked() && !has {
            editor.add_glyphs(&[ch]);
            editor.go_to_glyph(ch);
        } else if response.clicked() && has {
            editor.go_to_glyph(ch);
        }
        menus::context_menu(&response, |ui| glyph_menu(ui, editor, ch, has));
    }
}

/// El menú de una celda: Copy, Paste, Clear y Remove si la fuente tiene el carácter; Add si no.
fn glyph_menu(ui: &mut Ui, editor: &mut Editor, ch: char, has: bool) {
    if !has {
        if ui.add(menus::plain("Add")).clicked() {
            editor.add_glyphs(&[ch]);
            editor.go_to_glyph(ch);
            ui.close();
        }
        return;
    }
    if ui.add(menus::plain("Copy")).clicked() {
        editor.go_to_glyph(ch);
        editor.copy_glyph();
        ui.close();
    }
    if ui.add(menus::plain("Paste")).clicked() {
        editor.go_to_glyph(ch);
        editor.paste_glyph();
        ui.close();
    }
    if ui.add(menus::plain("Clear")).clicked() {
        editor.go_to_glyph(ch);
        editor.clear_glyph();
        ui.close();
    }
    menus::separator(ui);
    if ui.add(menus::plain("Remove")).clicked() {
        editor.remove_glyph(ch);
        ui.close();
    }
}

/// El campo "Add char" y el botón Add: agrega a la fuente lo escrito (sin los espacios).
fn add_chars(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &mut FontPanelState) {
    let (field, add) = ui
        .horizontal(|ui| {
            let field = match skin {
                Some(skin) => kit::input::text_input(
                    ui,
                    skin,
                    &mut state.add_char,
                    ADD_WIDTH,
                    "Add char",
                    "Add char",
                ),
                None => ui.add(
                    egui::TextEdit::singleline(&mut state.add_char)
                        .desired_width(ADD_WIDTH)
                        .hint_text("Add char"),
                ),
            };
            let add = match skin {
                Some(skin) => kit::button::button(ui, skin, "Add"),
                None => pixel_ui::text_button(ui, "Add"),
            };
            (field, add)
        })
        .inner;
    let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if add.clicked() || entered {
        let chars: Vec<char> = state
            .add_char
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if !chars.is_empty() {
            editor.add_glyphs(&chars);
        }
        state.add_char.clear();
    }
}
