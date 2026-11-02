//! El panel FontForge del dock: la fuente pixel del artboard activo. Arriba, fijo, el logo; abajo,
//! fijo, el botón Build; en medio, con scroll, lo demás. Sin fuente, el botón New Font…; con
//! fuente, el mapa de caracteres por páginas (ASCII, Latin-1, Latin Ext-A y los de la fuente):
//! cada celda con el glifo dibujado o, si está vacío o falta, su carácter en gris. Un clic va al
//! glifo, un doble clic en uno que falta lo agrega y va a él, y el menú de cada celda copia,
//! pega, borra o quita.
//!
//! Las medidas salen del mockup (Design.aseprite, frame 10), desde la esquina de arriba a la
//! izquierda del panel (su línea de borde es la fila y la columna 0).

use cs_core::FontInfo;
use eframe::egui::{
    self, Align2, Color32, Rect, Sense, Shape, Stroke, StrokeKind, TextStyle, Ui, UiBuilder, Vec2,
    pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::dock::FLYOUT_MARGIN;
use crate::editor::Editor;
use crate::font_dialog;
use crate::kit;
use crate::menus;
use crate::panel_kit::label_with_hint;
use crate::pixel_sections;
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use crate::widgets;

/// Ancho de adentro del panel (sin su marco): el contenido y la barra del scroll.
pub const WIDTH: f32 = CONTENT_W + pixel_ui::SCROLL_BAR;
/// Ancho del contenido que se desplaza (x 8 a 312 en el mockup; la barra, de 312 a 322).
const CONTENT_W: f32 = 304.0;
/// El logo del título, en (8, 8).
const LOGO: &str = "fontforge.logo";
/// La línea grabada bajo el título: la fila clara en la 29 y la oscura en la 30.
const HEADER_LINE: f32 = 29.0;
/// La del pie, a 33 filas del borde de abajo (375 y 376 en un panel de 408).
const FOOTER_LINE: f32 = 33.0;
/// El pie, de la fila de abajo de su línea hasta la de antes del bisel de abajo (377 a 405).
const FOOTER_TOP: f32 = FOOTER_LINE - 2.0;
const FOOTER_BOTTOM: f32 = 2.0;
/// Del borde de arriba del medio (lo que se desplaza) a lo primero, "Map" (sus mayúsculas en la
/// fila 41), y lo que hay entre las cosas del medio, para que caigan donde en el mockup: las
/// pestañas de la 59 a la 75, la cuadrícula desde la 85, la fila de Add desde la 276, "Test
/// Drive" en la 301, su campo desde la 311 y el papel desde la 332.
const MIDDLE_TOP_AIR: f32 = 4.0;
const MAP_TO_TABS: f32 = 6.0;
const TABS_TO_GRID: f32 = 0.0;
const GRID_TO_ADD: f32 = 6.0;
const ADD_TO_TEST: f32 = 0.0;
const TEST_LABEL_TO_FIELD: f32 = 0.0;
const TEST_FIELD_TO_PAPER: f32 = 4.0;
/// Una celda del mapa: 20×22 de blanco dentro de líneas de 1 que comparte con las vecinas.
const CELL_W: f32 = 20.0;
const CELL_H: f32 = 22.0;
const PITCH_X: f32 = CELL_W + 1.0;
const PITCH_Y: f32 = CELL_H + 1.0;
const COLUMNS: usize = 13;
/// Las líneas del mapa, el gris de los caracteres sin dibujar y el marco del glifo actual.
const GRID: Color32 = Color32::from_rgb(136, 110, 106);
const MISSING_INK: Color32 = Color32::from_rgb(197, 199, 221);
const CURRENT: Color32 = Color32::from_rgb(226, 114, 133);
/// Ancho del campo "Add char" (el botón Add va pegado, sobre su borde) y el aire hasta Add All.
const ADD_WIDTH: f32 = 120.0;
const ADD_ALL_GAP: f32 = 4.0;
/// Ancho del campo del Test Drive y de su papel.
const TEST_W: f32 = 284.0;
/// La escala del texto del Test Drive y el aire alrededor de él.
const TEST_SCALE: u32 = 2;
const TEST_PAD: f32 = 4.0;
/// La tinta del Test Drive: oscura fija, porque el papel es blanco aunque el tema sea oscuro.
const TEST_INK: Color32 = Color32::from_rgb(5, 4, 3);
/// La columna de las etiquetas de los ajustes y los anchos de sus controles.
const LABEL_W: f32 = 90.0;
/// Los números de las medidas y el espaciado van en dos columnas: cada una, la etiqueta y su
/// stepper, con aire entre las dos.
const PAIR_LABEL_W: f32 = 74.0;
const PAIR_STEPPER_W: f32 = 62.0;
const PAIR_GAP: f32 = 8.0;
const NAME_W: f32 = 160.0;
const GHOST_W: f32 = 80.0;

/// Los dos íconos del título (13×14 con su sombra clara): los ajustes de la fuente en (292, 9) y
/// el mapa en (309, 9).
const CONFIG_ICON: &str = "icon.fontforge.config";
const EDIT_ICON: &str = "icon.fontforge.edit";
const CONFIG_AT: Vec2 = vec2(292.0, 9.0);
const EDIT_AT: Vec2 = vec2(309.0, 9.0);
const ICON_SIZE: Vec2 = vec2(13.0, 14.0);

/// Lo que se ve en el medio del panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PanelView {
    /// El mapa, Add y el Test Drive.
    #[default]
    Edit,
    /// Los ajustes de la fuente.
    Config,
}

/// El estado del panel entre frames.
pub struct FontPanelState {
    pub view: PanelView,
    pub page: Page,
    /// Lo escrito en el campo "Add char".
    pub add_char: String,
    /// El texto de prueba de la fuente (el del Test Drive).
    pub test_text: String,
}

impl Default for FontPanelState {
    fn default() -> Self {
        FontPanelState {
            view: PanelView::default(),
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

    /// Los caracteres de la página que la fuente no tiene (en All, ninguno).
    fn missing(self, font: &FontInfo) -> Vec<char> {
        self.chars(font)
            .into_iter()
            .filter(|&ch| font.index_of(ch).is_none())
            .collect()
    }
}

/// Lo que el usuario pidió en el panel (lo resuelve la app).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontAction {
    NewFont,
}

/// El panel en `ui` (lo de adentro del marco, de la columna entera: ver
/// [`crate::dock::flyout_fixed`]): el título fijo, el medio con scroll y el pie fijo.
pub fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut FontPanelState,
) -> Option<FontAction> {
    let inner = ui.max_rect();
    // La caja del panel, como la pinta el dock: en puntos enteros.
    let outer = inner.expand(FLYOUT_MARGIN as f32);
    let outer = Rect::from_min_size(outer.min.round(), outer.size().round());
    header(ui, skin, state, outer, editor.font().is_some());
    footer(ui, skin, outer);
    let middle = Rect::from_x_y_ranges(
        outer.shrink(FLYOUT_MARGIN as f32).x_range(),
        outer.top() + HEADER_LINE + pixel_ui::ETCHED_SIZE..=outer.bottom() - FOOTER_LINE,
    );
    let area = egui::ScrollArea::vertical()
        .id_salt("font_panel_middle")
        .auto_shrink([false, false])
        .max_height(middle.height())
        .min_scrolled_height(middle.height());
    let action = ui
        .scope_builder(UiBuilder::new().max_rect(middle), |ui| {
            pixel_ui::scroll(ui, area, |ui| {
                ui.add_space(MIDDLE_TOP_AIR);
                let action = body(ui, editor, skin, state);
                ui.add_space(MIDDLE_TOP_AIR);
                action
            })
        })
        .inner;
    // Todo el panel es de la columna (el medio no lo dice: está en su propio `Ui`).
    ui.advance_cursor_after_rect(inner);
    action
}

/// Lo que se desplaza: New Font… sin fuente; con fuente, la vista elegida.
fn body(
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
    if state.view == PanelView::Config {
        settings(ui, editor, skin);
        return None;
    }
    let note = match count {
        1 => "1 glyph".to_owned(),
        n => format!("{n} glyphs"),
    };
    // Sin el aire de egui entre una cosa y otra: el de cada lugar, el del mockup.
    ui.spacing_mut().item_spacing.y = 0.0;
    label_with_hint(ui, "Map", &note);
    ui.add_space(MAP_TO_TABS);
    pages(ui, skin, state);
    ui.add_space(TABS_TO_GRID);
    map(ui, editor, skin, state);
    ui.add_space(GRID_TO_ADD);
    add_chars(ui, editor, skin, state);
    ui.add_space(ADD_TO_TEST);
    test_drive(ui, editor, skin, state);
    None
}

/// El título: el logo en (8, 8) (sin Pixel UI, "FontForge" en texto), con fuente los íconos de
/// las dos vistas a la derecha, y la línea grabada debajo.
fn header(ui: &mut Ui, skin: Option<&Skin>, state: &mut FontPanelState, outer: Rect, font: bool) {
    let at = outer.min + vec2(FLYOUT_MARGIN as f32, FLYOUT_MARGIN as f32);
    let logo = skin.and_then(|s| s.piece(LOGO, WidgetState::Normal));
    let ppp = ui.ctx().pixels_per_point();
    match logo {
        Some(piece) => {
            let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
            let shapes = pixel_ui::piece_shapes(piece, Rect::from_min_size(at, size), ppp);
            ui.painter().extend(shapes);
        }
        None => {
            ui.painter().text(
                at,
                Align2::LEFT_TOP,
                "FontForge",
                TextStyle::Heading.resolve(ui.style()),
                ui.visuals().strong_text_color(),
            );
        }
    }
    let icons = [
        (CONFIG_ICON, CONFIG_AT, "Font Settings", PanelView::Config),
        (EDIT_ICON, EDIT_AT, "Edit Glyphs", PanelView::Edit),
    ];
    let pixel_icons = skin.filter(|s| {
        icons
            .iter()
            .all(|(icon, ..)| s.piece(icon, WidgetState::Normal).is_some())
    });
    match (font, pixel_icons) {
        (false, _) => {}
        (true, Some(skin)) => {
            for (icon, offset, label, view) in icons {
                let on = state.view == view;
                if view_icon(ui, skin, icon, outer.min + offset, label, on) {
                    state.view = view;
                }
            }
        }
        (true, None) => {
            // Sin Pixel UI, botones de egui con el nombre, a la derecha del título.
            let row = Rect::from_min_max(
                outer.min + vec2(FLYOUT_MARGIN as f32, CONFIG_AT.y),
                pos2(
                    outer.right() - FLYOUT_MARGIN as f32,
                    outer.top() + HEADER_LINE,
                ),
            );
            let layout = egui::Layout::right_to_left(egui::Align::Min);
            ui.scope_builder(UiBuilder::new().max_rect(row).layout(layout), |ui| {
                for (_, _, label, view) in icons.into_iter().rev() {
                    let on = state.view == view;
                    if ui
                        .add(egui::Button::new(label).selected(on).small())
                        .clicked()
                    {
                        state.view = view;
                    }
                }
            });
        }
    }
    let line = outer.top() + HEADER_LINE;
    lit_line(ui, skin, outer, line);
}

/// Un ícono del título que cambia la vista: suelto, con los estados del kit (con el puntero
/// encima se aclara; apretado, baja 1). Devuelve si se hizo clic.
fn view_icon(ui: &mut Ui, skin: &Skin, icon: &str, at: egui::Pos2, label: &str, on: bool) -> bool {
    let rect = Rect::from_min_size(at, ICON_SIZE);
    let id = ui.id().with(("font_view", label));
    let response = ui.interact(rect, id, Sense::click());
    kit::icon::paint(ui, skin, icon, rect.min, &response);
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, label));
    use pixel_ui::Tip as _;
    response.tip(label).clicked()
}

/// La línea grabada al revés de [`pixel_ui::etched_hline`]: la clara (el blanco del brillo de los
/// menús) arriba, en la fila `top`, y la oscura (`canvas`) debajo, de lado a lado del panel
/// `outer`, por dentro de su borde. Sin Pixel UI, la línea de egui.
fn lit_line(ui: &Ui, skin: Option<&Skin>, outer: Rect, top: f32) {
    let x = outer.x_range().shrink(1.0);
    match skin {
        Some(skin) => {
            let ppp = ui.ctx().pixels_per_point();
            let colors = skin.colors();
            let light = Rect::from_x_y_ranges(x, top..=top + 1.0);
            let mut mesh = egui::Mesh::default();
            mesh.add_colored_rect(light.round_to_pixels(ppp), colors.menu_highlight);
            mesh.add_colored_rect(
                light.translate(vec2(0.0, 1.0)).round_to_pixels(ppp),
                colors.canvas,
            );
            ui.painter().add(Shape::mesh(mesh));
        }
        None => {
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            ui.painter().hline(x, top + 1.0, stroke);
        }
    }
}

/// El pie: la línea grabada y el botón Build (deshabilitado: "Coming soon") a la derecha,
/// centrado en el alto del pie.
fn footer(ui: &mut Ui, skin: Option<&Skin>, outer: Rect) {
    lit_line(ui, skin, outer, outer.bottom() - FOOTER_LINE);
    let label = "Build ▾";
    let rows = (outer.bottom() - FOOTER_TOP)..=(outer.bottom() - FOOTER_BOTTOM);
    let center = (rows.start() + rows.end()) / 2.0;
    let right = outer.right() - FLYOUT_MARGIN as f32;
    let id = ui.id().with("font_build");
    ui.add_enabled_ui(false, |ui| match skin {
        Some(skin) => {
            let size = vec2(kit::button::width(ui, label), kit::button::HEIGHT);
            let min = pos2(right - size.x, center - size.y / 2.0).round();
            kit::button::button_at(ui, skin, Rect::from_min_size(min, size), id, label)
        }
        None => {
            let size = vec2(60.0, 20.0);
            let min = pos2(right - size.x, center - size.y / 2.0).round();
            ui.put(Rect::from_min_size(min, size), egui::Button::new(label))
        }
    })
    .inner
    .on_disabled_hover_text("Coming soon");
}

/// El Test Drive: el campo del texto de prueba y, debajo, ese texto escrito con la fuente (a
/// escala 2, en tinta oscura sobre blanco). Un clic en una letra va a su glifo.
fn test_drive(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &mut FontPanelState) {
    let Some(view) = editor.artboard().font_view() else {
        return;
    };
    label_with_hint(ui, "Test Drive", "(click a letter)");
    ui.add_space(TEST_LABEL_TO_FIELD);
    let width = TEST_W;
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
    ui.add_space(TEST_FIELD_TO_PAPER);
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
type Number<'a> = (&'a str, &'a mut u32, std::ops::RangeInclusive<u32>);

/// Un número en su columna: la etiqueta de `PAIR_LABEL_W` y el stepper.
fn number_cell(ui: &mut Ui, skin: Option<&Skin>, (label, value, range): Number) {
    ui.allocate_ui_with_layout(
        vec2(PAIR_LABEL_W, kit::stepper::HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(PAIR_LABEL_W);
            ui.label(label);
        },
    );
    match skin {
        Some(skin) => {
            kit::stepper::stepper(ui, skin, PAIR_STEPPER_W, label, value, range, 1, None);
        }
        None => {
            ui.add(egui::DragValue::new(value).range(range));
        }
    }
}

/// Dos números en una fila, cada uno en su columna.
fn number_pair(ui: &mut Ui, skin: Option<&Skin>, left: Number, right: Number) {
    ui.horizontal(|ui| {
        number_cell(ui, skin, left);
        ui.add_space(PAIR_GAP);
        number_cell(ui, skin, right);
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
    // Las mayúsculas y la x-height hasta la baseline; los descendentes, de ella hasta abajo.
    let baseline = font.metrics.baseline;
    let m = &mut info.metrics;
    number_pair(
        ui,
        skin,
        ("Baseline", &mut m.baseline, 0..=cell_h),
        ("Cap height", &mut m.cap_height, 0..=baseline),
    );
    number_pair(
        ui,
        skin,
        ("x-height", &mut m.x_height, 0..=baseline),
        ("Descent", &mut m.descent, 0..=cell_h - baseline.min(cell_h)),
    );
    // Con la baseline movida, lo demás se acomoda (solo si se cambió algo: abrir el panel no
    // deja pasos).
    if info.metrics != font.metrics {
        info.metrics = info.metrics.clamped(cell_h);
    }
    number_pair(
        ui,
        skin,
        ("Letter spacing", &mut info.letter_spacing, 0..=8),
        ("Space width", &mut info.space_width, 0..=cell_w),
    );
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

/// La escala del glifo de `cell` en su celda (en puntos por pixel): 1 si cabe con 1 de aire por
/// lado (18×20); si no, la que cabe, hacia abajo en pasos de 0.5 (mínimo 0.5).
fn thumb_scale(cell: (u32, u32)) -> f32 {
    let (w, h) = (cell.0.max(1) as f32, cell.1.max(1) as f32);
    let fit = ((CELL_W - 2.0) / w).min((CELL_H - 2.0) / h);
    if fit >= 1.0 {
        1.0
    } else {
        ((fit * 2.0).floor() / 2.0).max(0.5)
    }
}

/// La cuadrícula de la página, de 13 columnas con líneas de 1 compartidas: blanco lo que la
/// fuente tiene, del color del panel lo que falta, con marco por dentro el glifo actual. Cada
/// celda muestra una sola cosa: el glifo dibujado o, si está vacío o falta, su carácter en gris.
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
    let columns = chars.len().min(COLUMNS);
    let (rect, _) = ui.allocate_exact_size(
        vec2(PITCH_X * columns as f32 + 1.0, PITCH_Y * rows as f32 + 1.0),
        Sense::hover(),
    );
    let origin = rect.min.round();
    let painter = ui.painter().clone();
    let missing_paper = skin.map_or(Color32::from_gray(200), |s| s.colors().panel);
    let font_id = TextStyle::Body.resolve(ui.style());
    let enabled = ui.is_enabled();
    for (i, &ch) in chars.iter().enumerate() {
        let lines = Rect::from_min_size(
            origin
                + vec2(
                    (i % COLUMNS) as f32 * PITCH_X,
                    (i / COLUMNS) as f32 * PITCH_Y,
                ),
            vec2(PITCH_X + 1.0, PITCH_Y + 1.0),
        );
        let cell = lines.shrink(1.0);
        let response = ui.interact(cell, ui.id().with(("font_map", ch)), Sense::click());
        let is_current = Some(ch) == current;
        let label = format!("Glyph {ch}");
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, is_current, &label)
        });
        let has = font.index_of(ch).is_some();
        painter.rect_filled(lines, 0.0, GRID);
        let paper = if has { Color32::WHITE } else { missing_paper };
        painter.rect_filled(cell, 0.0, paper);
        let glyph = view
            .as_ref()
            .and_then(|v| v.glyphs.get(&ch))
            .filter(|g| !g.pixels.is_empty());
        match glyph {
            Some(glyph) => {
                // El glifo (su celda entera), centrado.
                let size = vec2(cell_size.0 as f32, cell_size.1 as f32) * scale;
                let at = Rect::from_center_size(cell.center(), size).min.round();
                for &(x, y, c) in &glyph.pixels {
                    let color = c.map_or(TEST_INK, |c| {
                        Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
                    });
                    let pixel = Rect::from_min_size(
                        at + vec2(x as f32, y as f32) * scale,
                        Vec2::splat(scale),
                    );
                    painter.rect_filled(pixel.intersect(cell), 0.0, color);
                }
            }
            None => {
                // Centrado contando su espacio para descendentes: las mayúsculas quedan en la
                // fila 6 de la celda, como en el mockup.
                painter.text(
                    cell.center() - vec2(0.0, 2.0),
                    Align2::CENTER_CENTER,
                    ch.to_string(),
                    font_id.clone(),
                    MISSING_INK,
                );
            }
        }
        if is_current {
            painter.rect_stroke(cell, 0.0, Stroke::new(1.0, CURRENT), StrokeKind::Inside);
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

/// El campo "Add char" con el botón Add pegado (sobre su borde derecho), que agrega a la fuente
/// lo escrito (sin los espacios), y Add All: todo lo de la página que la fuente no tiene, en un
/// paso de deshacer.
fn add_chars(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &mut FontPanelState) {
    let missing = editor
        .font()
        .map_or_else(Vec::new, |font| state.page.missing(font));
    let (field, add, add_all) = match skin {
        Some(skin) => {
            let (row, _) = ui.allocate_exact_size(
                vec2(ui.available_width(), kit::input::HEIGHT),
                Sense::hover(),
            );
            let at = row.min.round();
            let field_rect = Rect::from_min_size(at, vec2(ADD_WIDTH, kit::input::HEIGHT));
            let field = ui
                .scope_builder(UiBuilder::new().max_rect(field_rect), |ui| {
                    kit::input::text_input(
                        ui,
                        skin,
                        &mut state.add_char,
                        ADD_WIDTH,
                        "Add char",
                        "Add char",
                    )
                })
                .inner;
            let add_rect = Rect::from_min_size(
                pos2(field_rect.right() - kit::input::OUTLINE, at.y),
                vec2(kit::button::width(ui, "Add"), kit::button::HEIGHT),
            );
            let add = kit::button::button_at(ui, skin, add_rect, ui.id().with("font_add"), "Add");
            let all_rect = Rect::from_min_size(
                pos2(add_rect.right() + ADD_ALL_GAP, at.y),
                vec2(kit::button::width(ui, "Add All"), kit::button::HEIGHT),
            );
            let id = ui.id().with("font_add_all");
            let add_all = ui
                .add_enabled_ui(!missing.is_empty(), |ui| {
                    kit::button::button_at(ui, skin, all_rect, id, "Add All")
                })
                .inner;
            (field, add, add_all)
        }
        None => {
            ui.horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut state.add_char)
                        .desired_width(ADD_WIDTH)
                        .hint_text("Add char"),
                );
                let add = pixel_ui::text_button(ui, "Add");
                let add_all = ui
                    .add_enabled_ui(!missing.is_empty(), |ui| {
                        pixel_ui::text_button(ui, "Add All")
                    })
                    .inner;
                (field, add, add_all)
            })
            .inner
        }
    };
    if add_all.clicked() && !missing.is_empty() {
        editor.add_glyphs(&missing);
    }
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
