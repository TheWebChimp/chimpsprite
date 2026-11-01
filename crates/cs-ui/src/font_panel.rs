//! El panel Font del dock: la fuente pixel del artboard activo. Sin fuente, el botón New Font…;
//! con fuente, el mapa de caracteres por páginas (ASCII, Latin-1, Latin Ext-A y los de la
//! fuente): cada celda con su carácter y la miniatura de su glifo. Un clic va al glifo, un doble
//! clic en uno que falta lo agrega y va a él, y el menú de cada celda copia, pega, borra o quita.

use cs_core::FontInfo;
use eframe::egui::{
    self, Align2, Color32, Rect, Sense, Stroke, StrokeKind, TextStyle, Ui, Vec2, vec2,
};

use crate::editor::Editor;
use crate::font_dialog;
use crate::kit;
use crate::menus;
use crate::panel_kit::label_with_hint;
use crate::pixel_sections;
use crate::pixel_ui;
use crate::skin::Skin;
use crate::widgets;

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
/// La escala del texto del Test Drive y el aire alrededor de él.
const TEST_SCALE: u32 = 2;
const TEST_PAD: f32 = 4.0;
/// La tinta del Test Drive: oscura fija, porque el papel es blanco aunque el tema sea oscuro.
const TEST_INK: Color32 = Color32::from_rgb(5, 4, 3);
/// La columna de las etiquetas de los ajustes y los anchos de sus controles.
const LABEL_W: f32 = 90.0;
const STEPPER_W: f32 = 80.0;
const NAME_W: f32 = 160.0;
const GHOST_W: f32 = 80.0;

/// El estado del panel entre frames.
pub struct FontPanelState {
    pub page: Page,
    /// Lo escrito en el campo "Add char".
    pub add_char: String,
    /// El texto de prueba de la fuente (el del Test Drive).
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
    ui.add_space(10.0);
    test_drive(ui, editor, skin, state);
    ui.add_space(10.0);
    settings(ui, editor, skin);
    None
}

/// El Test Drive: el campo del texto de prueba y, debajo, ese texto escrito con la fuente (a
/// escala 2, en tinta oscura sobre blanco). Un clic en una letra va a su glifo.
fn test_drive(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &mut FontPanelState) {
    let Some(view) = editor.artboard().font_view() else {
        return;
    };
    label_with_hint(ui, "Test Drive", "(click a letter)");
    ui.add_space(4.0);
    let width = WIDTH - 16.0;
    match skin {
        Some(skin) => {
            kit::input::text_input(
                ui,
                skin,
                &mut state.test_text,
                width,
                "Test text",
                "Type to test",
            );
        }
        None => {
            ui.add(
                egui::TextEdit::singleline(&mut state.test_text)
                    .desired_width(width)
                    .hint_text("Type to test"),
            );
        }
    }
    ui.add_space(4.0);
    let line_h = (view.cell.1 * TEST_SCALE) as f32;
    let lines = state.test_text.lines().count().max(1);
    let (rect, _) = ui.allocate_exact_size(
        vec2(width, line_h * lines as f32 + 2.0 * TEST_PAD),
        Sense::hover(),
    );
    let paper = Rect::from_min_size(rect.min.round(), rect.size());
    let painter = ui.painter().with_clip_rect(paper.intersect(ui.clip_rect()));
    painter.rect_filled(paper, 0.0, Color32::WHITE);
    let origin = paper.min + Vec2::splat(TEST_PAD);
    for (x, y, c) in cs_tools::text::font_text(&view, &state.test_text, TEST_SCALE) {
        let color = c.map_or(TEST_INK, |c| {
            Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
        });
        painter.rect_filled(
            Rect::from_min_size(origin + vec2(x as f32, y as f32), Vec2::splat(1.0)),
            0.0,
            color,
        );
    }
    // Las letras: un rect por carácter, con el avance que usa `font_text`.
    let enabled = ui.is_enabled();
    let mut go = None;
    for (row, line) in state.test_text.lines().enumerate() {
        let mut pen = 0u32;
        for (i, ch) in line.chars().enumerate() {
            let advance = if ch == ' ' {
                view.space_width
            } else {
                view.glyphs
                    .get(&ch)
                    .map_or(view.missing_width, |g| g.advance)
            };
            let letter = Rect::from_min_size(
                origin + vec2((pen * TEST_SCALE) as f32, row as f32 * line_h),
                vec2((advance * TEST_SCALE) as f32, line_h),
            )
            .intersect(paper);
            pen += advance;
            if ch == ' ' || !letter.is_positive() {
                continue;
            }
            let response =
                ui.interact(letter, ui.id().with(("test_drive", row, i)), Sense::click());
            let label = format!("Test Drive {ch}");
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label)
            });
            if response.clicked() {
                go = Some(ch);
            }
        }
    }
    if let Some(ch) = go {
        editor.go_to_glyph(ch);
    }
}

/// Una fila de los ajustes: la etiqueta en su columna y el control.
fn row<R>(ui: &mut Ui, label: &str, control: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            vec2(LABEL_W, kit::stepper::HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(LABEL_W);
                ui.label(label);
            },
        );
        control(ui)
    })
    .inner
}

/// Un número de los ajustes: el stepper del kit (sin Pixel UI, el `DragValue` de egui).
fn number(
    ui: &mut Ui,
    skin: Option<&Skin>,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) {
    row(ui, label, |ui| match skin {
        Some(skin) => kit::stepper::stepper(ui, skin, STEPPER_W, label, value, range, 1, None),
        None => ui.add(egui::DragValue::new(value).range(range)),
    });
}

/// Una casilla: la del kit (sin Pixel UI, la de egui).
fn checkbox(ui: &mut Ui, skin: Option<&Skin>, checked: &mut bool, label: &str) -> egui::Response {
    match skin {
        Some(skin) => kit::check::checkbox(ui, skin, checked, label),
        None => ui.checkbox(checked, label),
    }
}

/// Los ajustes de la fuente: nombre, modo y celda, las medidas, el espaciado, Monospace, la letra
/// fantasma y el ancho automático del glifo actual. Todo lo cambiado en un frame (o en un
/// arrastre) es un paso de deshacer (`set_font_info_grouped`).
fn settings(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let Some(font) = editor.font().cloned() else {
        return;
    };
    let (cell_w, cell_h) = (editor.sprite().width, editor.sprite().height);
    ui.label("Font");
    ui.add_space(4.0);
    let mut info = font.clone();
    let name_id = ui.id().with("font_name");
    if let Some(name) = row(ui, "Name", |ui| {
        kit::input::buffered(ui, name_id, &font.name, |ui, text| match skin {
            Some(skin) => kit::input::text_input(ui, skin, text, NAME_W, "Font name", "Font name"),
            None => ui.add(egui::TextEdit::singleline(text).desired_width(NAME_W)),
        })
    }) {
        info.name = name;
    }
    row(ui, "Mode", |ui| ui.label(font_dialog::mode_name(font.mode)));
    row(ui, "Cell", |ui| ui.label(format!("{cell_w}×{cell_h}")));
    number(ui, skin, "Baseline", &mut info.metrics.baseline, 0..=cell_h);
    number(
        ui,
        skin,
        "Cap height",
        &mut info.metrics.cap_height,
        0..=cell_h,
    );
    number(ui, skin, "x-height", &mut info.metrics.x_height, 0..=cell_h);
    number(ui, skin, "Descent", &mut info.metrics.descent, 0..=cell_h);
    number(ui, skin, "Letter spacing", &mut info.letter_spacing, 0..=8);
    number(ui, skin, "Space width", &mut info.space_width, 0..=cell_w);
    ui.add_space(2.0);
    checkbox(ui, skin, &mut info.monospace, "Monospace");
    ui.horizontal(|ui| {
        checkbox(ui, skin, &mut info.ghost.enabled, "Ghost");
        let face = if info.ghost.monospace {
            "Hack"
        } else {
            "Ubuntu"
        };
        let ghost = &mut info.ghost.monospace;
        match skin {
            Some(skin) => {
                kit::select::select(ui, skin, "font_ghost", face, Some(GHOST_W), |ui| {
                    widgets::choice(ui, ghost, false, "Ubuntu");
                    widgets::choice(ui, ghost, true, "Hack");
                });
            }
            None => {
                egui::ComboBox::from_id_salt("font_ghost")
                    .selected_text(face)
                    .show_ui(ui, |ui| {
                        widgets::choice(ui, ghost, false, "Ubuntu");
                        widgets::choice(ui, ghost, true, "Hack");
                    });
            }
        }
    });
    // Un arrastre (de un stepper) es un solo paso de undo: se juntan los cambios mientras el
    // botón siga apretado.
    let group = ui.id().with("font_settings").value();
    if info != font {
        editor.set_font_info_grouped(info, group);
    }
    if !ui.input(|i| i.pointer.any_down()) {
        editor.end_group_of(group);
    }
    // El ancho automático del glifo actual (después: `set_font_info` lleva los anchos de antes).
    let current = editor.current_glyph().and_then(|ch| {
        let font = editor.font()?;
        Some((ch, font.glyphs[font.index_of(ch)?].advance))
    });
    if let Some((ch, advance)) = current {
        let mut auto = advance.is_none();
        let caption = format!("Auto width ({ch})");
        if checkbox(ui, skin, &mut auto, &caption).changed() {
            let advance = if auto { None } else { editor.glyph_advance() };
            editor.set_glyph_advance(ch, advance);
        }
    }
    ui.add_space(6.0);
    ui.add_enabled_ui(false, |ui| match skin {
        Some(skin) => kit::button::button(ui, skin, "Build ▾"),
        None => pixel_ui::text_button(ui, "Build ▾"),
    })
    .inner
    .on_disabled_hover_text("Coming soon");
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
