//! El panel FontForge del dock: la fuente pixel del artboard activo. Arriba, fijo, el logo; abajo,
//! fijo, el botón Build; en medio, con scroll, lo demás. Sin fuente, el botón New Font…; con
//! fuente, el mapa de caracteres por páginas (ASCII, Latin-1, Latin Ext-A y los de la fuente):
//! cada celda con el glifo dibujado o, si está vacío o falta, su carácter en gris. Un clic va al
//! glifo, un doble clic en uno que falta lo agrega y va a él, y el menú de cada celda copia,
//! pega, borra o quita. Al final (debajo del Test Drive), la sección Glyph del glifo actual: sus
//! componentes (si es compuesto) con sus offsets, agregar uno, Unlink y Build. Generate ▾ arma
//! los acentos y trae glifos de un TTF o de una imagen; Build ▾ abre el menú de exportar (TTF,
//! TTF de color y BMFont).
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
use crate::font_metrics_preview::{self, Metric};
use crate::forms::{self, TuiBuilderLogic as _};
use crate::kit;
use crate::menus;
use crate::panel_kit::label_with_hint;
use crate::pixel_sections;
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use crate::widgets;

/// Ancho de adentro del panel (sin su marco) en el mockup: el contenido y la barra del scroll.
/// El dock abre el panel de este ancho; adentro, los ajustes se acomodan al que les toque.
pub const REFERENCE_WIDTH: f32 = CONTENT_W + pixel_ui::SCROLL_BAR;
/// Lo más angosto del panel: por ahora, su ancho de referencia. Baja cuando se migren el título,
/// la vista Edit (Test Drive mide 284) y la de Kerning; la de Config ya llega a
/// [`CONFIG_MIN_WIDTH`].
#[expect(dead_code, reason = "el dock lo usará cuando deje achicar los paneles")]
pub const MIN_WIDTH: f32 = REFERENCE_WIDTH;
/// El ancho más chico en que la vista Config cabe sin encimarse ni salirse (medido): lo más ancho
/// que no se parte es Weight con su select (32 + 5 + 112).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "el panel lo usará cuando se migren las otras vistas"
    )
)]
pub const CONFIG_MIN_WIDTH: f32 = 150.0;
/// Ancho del contenido que se desplaza (x 8 a 312 en el mockup; la barra, de 312 a 322).
const CONTENT_W: f32 = 304.0;
/// El logo del título (el chango herrero y "FontForge", 141×28), en (6, 2): sube por encima del
/// margen y su última fila pisa la línea del título (settings-mockup.png, x 469 e y 72 con el
/// panel en (463, 70)). Sin Pixel UI, "FontForge" en texto en (8, 8).
const LOGO: &str = "fontforge.logo";
const LOGO_AT: Vec2 = vec2(6.0, 2.0);
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
/// Las líneas del mapa, el gris de los caracteres sin dibujar y el marco del glifo actual.
const GRID: Color32 = Color32::from_rgb(136, 110, 106);
const MISSING_INK: Color32 = Color32::from_rgb(197, 199, 221);
const CURRENT: Color32 = Color32::from_rgb(226, 114, 133);
/// Ancho del campo "Add char" (el botón Add va pegado, sobre su borde) y el aire hasta Add All.
const ADD_WIDTH: f32 = 120.0;
const ADD_ALL_GAP: f32 = 4.0;
/// Ancho del campo del Test Drive y de su papel.
pub(crate) const TEST_W: f32 = 284.0;
/// La escala del texto del Test Drive y el aire alrededor de él.
pub(crate) const TEST_SCALE: u32 = 2;
pub(crate) const TEST_PAD: f32 = 4.0;
/// La tinta del Test Drive: oscura fija, porque el papel es blanco aunque el tema sea oscuro.
pub(crate) const TEST_INK: Color32 = Color32::from_rgb(5, 4, 3);
/// La vista de ajustes, como en settings-mockup.png (el panel en (463, 70) y el contenido desde
/// (471, 105), su esquina: todo lo de abajo es desde ahí, en [`REFERENCE_WIDTH`]). Se arma con
/// [`forms`]: con menos ancho las filas se parten (lo que va junto baja junto) y con más crece la
/// vista previa. Las etiquetas ([`forms::label`]) van en la fila de su control, con las
/// mayúsculas 6 más abajo, como en el mockup.
///
/// Las filas van a 4 una de otra (una cada 21) y las etiquetas a 4 de su campo: los campos en
/// x 37 (508), Family y Style de 160 en las filas 1 y 22 (106 y 127) y el select de Weight de 112
/// en la 43 (148). Con poco ancho, Family y Style se achican hasta [`NAME_MIN`].
const ROW_GAP: f32 = 4.0;
const FIELD_X: f32 = 37.0;
const LABEL_W: f32 = FIELD_X - ROW_GAP;
const NAME_W: f32 = 160.0;
const NAME_MIN: f32 = 60.0;
const FAMILY_Y: f32 = 1.0;
const WEIGHT_W: f32 = 112.0;
/// La fila de Weight va con 5 entre sus cosas: la casilla Ghost en x 154 (625; 1 más arriba que
/// el select: la casilla en la 150) y el select de la fantasma de 80 en x 200 (671).
const WEIGHT_GAP: f32 = 5.0;
const GHOST_X: f32 = 154.0;
const GHOST_RAISE: f32 = 1.0;
const GHOST_SELECT_X: f32 = 200.0;
const GHOST_W: f32 = 80.0;
/// La fila de Italic, Monospace y Auto width (las casillas en y 172 y x 508, 556 y 625).
const CHECKS_X: [f32; 3] = [FIELD_X, 85.0, GHOST_X];
/// Mode · 1 bit · Cell · 16×16: texto en cuatro columnas (la tinta en x 471, 545, 622 y 705),
/// las mayúsculas en la 196; de a dos, que bajan juntos.
const MODE_X: [f32; 4] = [0.0, NUMBER_X, 151.0, 234.0];
/// Los números en una columna: steppers de 62 en x 74 (545), uno cada 21 desde la 111 (216), 5
/// más abajo de lo que daría el paso de las filas.
const NUMBER_X: f32 = 74.0;
const NUMBER_W: f32 = 62.0;
const NUMBERS_AIR: f32 = 5.0;
/// El recuadro de la vista previa de métricas, con su marco: desde (146, 111) (617, 216), del
/// alto de los seis steppers menos 1 (121, hasta la 336) y hasta 2 antes del borde del contenido
/// (782). Con poco ancho, debajo de los números y de no menos de [`PREVIEW_MIN`].
const PREVIEW_X: f32 = 146.0;
const PREVIEW_H: f32 = 6.0 * kit::stepper::HEIGHT + 5.0 * ROW_GAP - 1.0;
const PREVIEW_MIN: f32 = 80.0;
const PREVIEW_RIGHT_AIR: f32 = 2.0;
/// La familia, 13 debajo de los números: el título con sus mayúsculas en la 250 (355; su caja
/// desde la 246), los radios de los estilos cada 21 desde la 268 (el radio en la 376) y New
/// Variant… 23 debajo del último (396).
const FAMILY_AIR: f32 = 13.0;
const FAMILY_TITLE_H: f32 = 18.0;
const NEW_VARIANT_AIR: f32 = 2.0;
/// El aire entre Generate ▾ y Build ▾.
const GENERATE_GAP: f32 = 4.0;
/// La sección Glyph: el aire de arriba y entre filas, entre controles, el ancho del nombre de
/// cada componente, de sus steppers y del campo Component.
const GLYPH_AIR: f32 = 6.0;
const GLYPH_ROW_GAP: f32 = 3.0;
const GLYPH_GAP: f32 = 4.0;
const COMPONENT_NAME_W: f32 = 70.0;
const OFFSET_W: f32 = 74.0;
const COMPONENT_FIELD_W: f32 = 80.0;
/// La marquita de los compuestos en el mapa.
const COMPOSITE_MARK: Color32 = Color32::from_rgb(60, 110, 235);

/// Los íconos del título (13×14 con su sombra clara): el kerning en (275, 9), los ajustes de la
/// fuente en (292, 9) y el mapa en (309, 9).
const KERN_ICON: &str = "icon.fontforge.kern";
const CONFIG_ICON: &str = "icon.fontforge.config";
const EDIT_ICON: &str = "icon.fontforge.edit";
const KERN_AT: Vec2 = vec2(275.0, 9.0);
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
    /// Los pares y las clases de kerning.
    Kerning,
}

/// El estado del panel entre frames.
pub struct FontPanelState {
    pub view: PanelView,
    pub page: Page,
    /// Lo escrito en el campo "Add char".
    pub add_char: String,
    /// El texto de prueba de la fuente (el del Test Drive).
    pub test_text: String,
    /// Lo escrito en el campo "Component" de la sección Glyph.
    pub component_char: String,
    /// El par elegido en la vista Kerning.
    pub kern_pair: Option<(char, char)>,
    /// Los campos First y Second de la fila para agregar un par.
    pub kern_first: String,
    pub kern_second: String,
    /// El arrastre de una letra en la vista Kerning: el valor al empezar y lo arrastrado.
    pub kern_drag: Option<(i32, f32)>,
    /// El par que la vista Kerning mostró elegido en el último frame (con el panel habilitado):
    /// las flechas lo corren. Los atajos lo toman al empezar cada frame.
    pub kern_arrows: Option<(char, char)>,
}

impl Default for FontPanelState {
    fn default() -> Self {
        FontPanelState {
            view: PanelView::default(),
            page: Page::default(),
            add_char: String::new(),
            test_text: "The quick brown fox".to_owned(),
            component_char: String::new(),
            kern_pair: None,
            kern_first: String::new(),
            kern_second: String::new(),
            kern_drag: None,
            kern_arrows: None,
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
    /// Build ▾ → un formato: abrir el diálogo Build Font con él.
    Build(crate::font_io::BuildFormat),
    /// Generate ▾ → Auto-build accents (la app dice qué pasó).
    AutoBuildAccents,
    /// Generate ▾ → Fill from TTF…
    FillFromTtf,
    /// Generate ▾ → Glyphs from image…
    GlyphsFromImage,
    /// La sección Family de los ajustes → abrir la pestaña de otro estilo.
    OpenFont(cs_core::ArtboardId),
    /// New Variant… (de los ajustes).
    NewVariant,
}

/// Lo del menú Generate ▾.
const GENERATE: [(&str, FontAction); 3] = [
    ("Auto-build accents", FontAction::AutoBuildAccents),
    ("Fill from TTF…", FontAction::FillFromTtf),
    ("Glyphs from image…", FontAction::GlyphsFromImage),
];

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
    let build = footer(ui, skin, outer, editor.font().map(|f| f.mode));
    let middle = Rect::from_x_y_ranges(
        outer.shrink(FLYOUT_MARGIN as f32).x_range(),
        outer.top() + HEADER_LINE + pixel_ui::ETCHED_SIZE..=outer.bottom() - FOOTER_LINE,
    );
    let area = egui::ScrollArea::vertical()
        .id_salt("font_panel_middle")
        .auto_shrink([false, false])
        .max_height(middle.height())
        .min_scrolled_height(middle.height());
    // Desarrollo: con CHIMPSPRITE_DEMO_FONT_SCROLL, el medio bajado hasta la sección Glyph, para
    // capturas.
    let area = match crate::demo::dev_var("CHIMPSPRITE_DEMO_FONT_SCROLL") {
        Some(_) => area.vertical_scroll_offset(330.0),
        None => area,
    };
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
    action.or(build)
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
    match state.view {
        PanelView::Config => return config_view(ui, editor, skin),
        PanelView::Kerning => {
            crate::font_kern_panel::show(ui, editor, skin, state);
            return None;
        }
        PanelView::Edit => {}
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
    // Debajo del Test Drive: así lo de arriba queda en las filas del mockup.
    glyph_section(ui, editor, skin, state);
    None
}

/// El título: con fuente los íconos de las vistas a la derecha, la línea grabada debajo y el
/// logo en [`LOGO_AT`], encima de la línea (sin Pixel UI, "FontForge" en texto).
fn header(ui: &mut Ui, skin: Option<&Skin>, state: &mut FontPanelState, outer: Rect, font: bool) {
    let line = outer.top() + HEADER_LINE;
    lit_line(ui, skin, outer, line);
    let logo = skin.and_then(|s| s.piece(LOGO, WidgetState::Normal));
    let ppp = ui.ctx().pixels_per_point();
    match logo {
        Some(piece) => {
            let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
            let rect = Rect::from_min_size(outer.min + LOGO_AT, size);
            ui.painter()
                .extend(pixel_ui::piece_shapes(piece, rect, ppp));
        }
        None => {
            ui.painter().text(
                outer.min + vec2(FLYOUT_MARGIN as f32, FLYOUT_MARGIN as f32),
                Align2::LEFT_TOP,
                "FontForge",
                TextStyle::Heading.resolve(ui.style()),
                ui.visuals().strong_text_color(),
            );
        }
    }
    let icons = [
        (KERN_ICON, KERN_AT, "Kerning", PanelView::Kerning),
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
pub(crate) fn lit_line(ui: &Ui, skin: Option<&Skin>, outer: Rect, top: f32) {
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

/// El pie: la línea grabada y el botón Build ▾ a la derecha, centrado en el alto del pie. Con
/// una fuente (de `mode`), abre el menú de los formatos; sin fuente, deshabilitado.
fn footer(
    ui: &mut Ui,
    skin: Option<&Skin>,
    outer: Rect,
    mode: Option<cs_core::FontMode>,
) -> Option<FontAction> {
    lit_line(ui, skin, outer, outer.bottom() - FOOTER_LINE);
    let label = "Build ▾";
    let rows = (outer.bottom() - FOOTER_TOP)..=(outer.bottom() - FOOTER_BOTTOM);
    let center = (rows.start() + rows.end()) / 2.0;
    let right = outer.right() - FLYOUT_MARGIN as f32;
    let id = ui.id().with("font_build");
    let response = ui
        .add_enabled_ui(mode.is_some(), |ui| match skin {
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
        .on_disabled_hover_text("Open a font to build it");
    let generate = generate_button(ui, skin, response.rect, mode.is_some());
    let mode = mode?;
    let content = |ui: &mut Ui| {
        let mut picked = None;
        for format in crate::font_io::BuildFormat::ALL {
            let item = format!("{}…", format.label());
            let enabled = format.available(mode);
            if ui
                .add_enabled(enabled, menus::plain(&item))
                .on_disabled_hover_text("Only for color fonts")
                .clicked()
            {
                picked = Some(FontAction::Build(format));
                ui.close();
            }
        }
        picked
    };
    let build = match skin {
        Some(skin) => {
            let look = menus::MenuLook::new(skin, response.rect.top());
            menus::dropdown(&look, &response, response.rect, content).flatten()
        }
        None => egui::Popup::menu(&response)
            .show(content)
            .and_then(|i| i.inner),
    };
    build.or(generate)
}

/// Generate ▾, a la izquierda de Build ▾ (`build` es su rect), con su menú.
fn generate_button(
    ui: &mut Ui,
    skin: Option<&Skin>,
    build: Rect,
    enabled: bool,
) -> Option<FontAction> {
    let label = "Generate ▾";
    let id = ui.id().with("font_generate");
    let response = ui
        .add_enabled_ui(enabled, |ui| match skin {
            Some(skin) => {
                let size = vec2(kit::button::width(ui, label), kit::button::HEIGHT);
                let min = pos2(build.left() - GENERATE_GAP - size.x, build.top());
                kit::button::button_at(ui, skin, Rect::from_min_size(min, size), id, label)
            }
            None => {
                let size = vec2(80.0, build.height());
                let min = pos2(build.left() - GENERATE_GAP - size.x, build.top());
                ui.put(Rect::from_min_size(min, size), egui::Button::new(label))
            }
        })
        .inner
        .on_disabled_hover_text("Open a font to generate glyphs");
    if !enabled {
        return None;
    }
    let content = |ui: &mut Ui| {
        let mut picked = None;
        for (item, action) in GENERATE {
            if ui.add(menus::plain(item)).clicked() {
                picked = Some(action);
                ui.close();
            }
        }
        picked
    };
    match skin {
        Some(skin) => {
            let look = menus::MenuLook::new(skin, response.rect.top());
            menus::dropdown(&look, &response, response.rect, content).flatten()
        }
        None => egui::Popup::menu(&response)
            .show(content)
            .and_then(|i| i.inner),
    }
}

/// La sección Glyph del glifo actual: si es compuesto, una fila por componente (elegirlo, sus
/// offsets X e Y y quitarlo); el campo Component con Add; Unlink (si es compuesto) y Build (si
/// el carácter es un acentuado de la tabla). Los offsets de un arrastre son un paso.
fn glyph_section(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut FontPanelState,
) {
    let Some(ch) = editor.current_glyph() else {
        return;
    };
    let components = editor.glyph_components();
    let composite = !components.is_empty();
    let hint = if composite {
        format!("({ch}, composite)")
    } else {
        format!("({ch})")
    };
    ui.add_space(GLYPH_AIR);
    label_with_hint(ui, "Glyph", &hint);
    ui.add_space(GLYPH_ROW_GAP);
    let (cell_w, cell_h) = (editor.sprite().width as i32, editor.sprite().height as i32);
    let selected = editor.selected_component();
    let group = ui.id().with("glyph_components").value();
    let font = editor.font().cloned();
    let mut select = None;
    let mut moved = None;
    let mut remove = None;
    for (i, c) in components.iter().enumerate() {
        let missing = font.as_ref().is_some_and(|f| f.index_of(c.ch).is_none());
        let name = if missing {
            format!("{} (missing)", c.ch)
        } else {
            c.ch.to_string()
        };
        let (mut x, mut y) = c.offset;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GLYPH_GAP;
            let on = selected == Some(i);
            let picked = match skin {
                Some(skin) => {
                    ui.allocate_ui_with_layout(
                        vec2(COMPONENT_NAME_W, kit::stepper::HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(COMPONENT_NAME_W);
                            kit::check::radio(ui, skin, on, &name)
                        },
                    )
                    .inner
                }
                None => ui.radio(on, &name),
            };
            if picked.clicked() {
                select = Some(i);
            }
            let changed = match skin {
                Some(skin) => {
                    let a = kit::stepper::axis_stepper(
                        ui,
                        skin,
                        OFFSET_W,
                        &format!("component_x{i}"),
                        "X",
                        &mut x,
                        -cell_w..=cell_w,
                        1,
                    );
                    let b = kit::stepper::axis_stepper(
                        ui,
                        skin,
                        OFFSET_W,
                        &format!("component_y{i}"),
                        "Y",
                        &mut y,
                        -cell_h..=cell_h,
                        1,
                    );
                    a.changed() || b.changed()
                }
                None => {
                    let a = ui.add(egui::DragValue::new(&mut x).range(-cell_w..=cell_w));
                    let b = ui.add(egui::DragValue::new(&mut y).range(-cell_h..=cell_h));
                    a.changed() || b.changed()
                }
            };
            if changed {
                moved = Some((i, (x, y)));
            }
            let gone = match skin {
                Some(skin) => kit::button::button(ui, skin, "Remove"),
                None => pixel_ui::text_button(ui, "Remove"),
            };
            if gone.clicked() {
                remove = Some(i);
            }
        });
        ui.add_space(GLYPH_ROW_GAP);
    }
    if let Some(i) = select {
        editor.select_component(Some(i));
    }
    if let Some((i, offset)) = moved {
        editor.select_component(Some(i));
        editor.move_component_grouped(i, offset, group);
    }
    if !ui.input(|i| i.pointer.any_down()) {
        editor.end_group_of(group);
    }
    if let Some(i) = remove {
        editor.remove_component(i);
    }
    // El campo Component con Add, Unlink y Build.
    let buildable = cs_core::decomposition(ch).is_some();
    let (add, unlink, build) = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GLYPH_GAP;
            let button = |ui: &mut Ui, label: &str, enabled: bool| {
                ui.add_enabled_ui(enabled, |ui| match skin {
                    Some(skin) => kit::button::button(ui, skin, label),
                    None => pixel_ui::text_button(ui, label),
                })
                .inner
            };
            let field = match skin {
                Some(skin) => kit::input::text_input(
                    ui,
                    skin,
                    &mut state.component_char,
                    COMPONENT_FIELD_W,
                    "Component",
                    "Component",
                ),
                None => ui.add(
                    egui::TextEdit::singleline(&mut state.component_char)
                        .desired_width(COMPONENT_FIELD_W)
                        .hint_text("Component"),
                ),
            };
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let add = button(ui, "Add", true).clicked() || entered;
            let unlink = button(ui, "Unlink", composite)
                .on_disabled_hover_text("Only for composite glyphs")
                .clicked();
            let build = button(ui, "Build", buildable)
                .on_disabled_hover_text("Not an accented letter")
                .on_hover_text("Build this accented letter from its base and mark")
                .clicked();
            (add, unlink, build)
        })
        .inner;
    if add {
        if let Some(c) = state.component_char.chars().find(|c| !c.is_whitespace()) {
            editor.add_component(c);
        }
        state.component_char.clear();
    }
    if unlink {
        editor.unlink_glyph(ch);
    }
    if build {
        // Un compuesto o un glifo dibujado se reemplaza: aquí lo pidió el usuario.
        editor.auto_build_accents(Some(&[ch]), true);
    }
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
    // Las letras: un rect por carácter, donde las pone `font_text` (con el kerning).
    let enabled = ui.is_enabled();
    let mut go = None;
    for laid in cs_tools::text::font_layout(&view, &state.test_text) {
        let letter = letter_rect(origin, line_h, &laid).intersect(paper);
        if laid.ch == ' ' || !letter.is_positive() {
            continue;
        }
        let id = ui.id().with(("test_drive", laid.row, laid.col));
        let response = ui.interact(letter, id, Sense::click());
        let label = format!("Test Drive {}", laid.ch);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        if response.clicked() {
            go = Some(laid.ch);
        }
    }
    if let Some(ch) = go {
        editor.go_to_glyph(ch);
    }
}

/// El rect de una letra del papel del texto de prueba (a `TEST_SCALE`, desde `origin`).
pub(crate) fn letter_rect(
    origin: egui::Pos2,
    line_h: f32,
    laid: &cs_tools::text::LaidChar,
) -> Rect {
    let scale = TEST_SCALE as f32;
    Rect::from_min_size(
        origin + vec2(laid.x as f32 * scale, laid.row as f32 * line_h),
        vec2(laid.advance.max(1) as f32 * scale, line_h),
    )
}

/// La vista de ajustes (el engrane): los ajustes de la fuente con la vista previa de métricas y,
/// debajo, la familia, del ancho de `ui` (en [`REFERENCE_WIDTH`], como el mockup: ver
/// [`FIELD_X`]).
pub(crate) fn config_view(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
) -> Option<FontAction> {
    editor.font()?;
    ui.scope(|ui| {
        // Sin el aire de egui entre una cosa y otra: el de cada lugar, el del mockup.
        ui.spacing_mut().item_spacing.y = 0.0;
        settings(ui, editor, skin);
        ui.add_space(FAMILY_AIR);
        family_section(ui, editor, skin)
    })
    .inner
}

/// Un número: el stepper del kit de [`NUMBER_W`] (sin Pixel UI, el `DragValue` de egui).
fn number(ui: &mut Ui, skin: Option<&Skin>, label: &str, value: &mut u32, range: RangeU32) {
    match skin {
        Some(skin) => {
            kit::stepper::stepper(ui, skin, NUMBER_W, label, value, range, 1, None);
        }
        None => {
            ui.add(egui::DragValue::new(value).range(range));
        }
    }
}

type RangeU32 = std::ops::RangeInclusive<u32>;

/// Una casilla: la del kit (sin Pixel UI, la de egui).
fn checkbox(ui: &mut Ui, skin: Option<&Skin>, checked: &mut bool, label: &str) -> egui::Response {
    match skin {
        Some(skin) => kit::check::checkbox(ui, skin, checked, label),
        None => ui.checkbox(checked, label),
    }
}

/// Un select del kit de `width` con `selected` (sin Pixel UI, el `ComboBox` de egui).
fn select(
    ui: &mut Ui,
    skin: Option<&Skin>,
    id: &str,
    selected: &str,
    width: f32,
    content: impl FnOnce(&mut Ui),
) {
    match skin {
        Some(skin) => {
            kit::select::select(ui, skin, id, selected, Some(width), content);
        }
        None => {
            egui::ComboBox::from_id_salt(id)
                .selected_text(selected)
                .width(width)
                .show_ui(ui, content);
        }
    }
}

/// Family y Style: la etiqueta, el nombre del campo y su texto de ayuda.
const NAMES: [(&str, &str, &str); 2] = [
    ("Family", "Font name", "Font name"),
    ("Style", "Style name", "Regular"),
];

/// Una fila de [`NAMES`] en `tui`, con `air` de más arriba: la etiqueta y el campo con
/// `current`, de [`NAME_W`] (con poco ancho se achica hasta [`NAME_MIN`]; no se separan). Lo
/// escrito, al terminar de escribir (ver [`kit::input::buffered`]).
fn name_row(
    tui: &mut forms::Tui,
    skin: Option<&Skin>,
    id: egui::Id,
    (caption, label, hint): (&str, &str, &str),
    current: &str,
    air: f32,
) -> Option<String> {
    let h = kit::input::HEIGHT;
    let mut edited = None;
    let row = forms::air_above(air, forms::row_nowrap(ROW_GAP));
    tui.style(row).add(|tui| {
        tui.style(forms::fixed(LABEL_W, h))
            .ui(|ui| forms::label(ui, caption, LABEL_W));
        tui.style(forms::grow(NAME_MIN, NAME_W, Some(NAME_W), h))
            .ui(|ui| {
                let w = ui.max_rect().width();
                edited = kit::input::buffered(ui, id, current, |ui, text| match skin {
                    Some(skin) => kit::input::text_input(ui, skin, text, w, label, hint),
                    None => ui.add(egui::TextEdit::singleline(text).desired_width(w)),
                });
            });
    });
    edited
}

/// Los ajustes de la fuente, del ancho de `ui`: Family, Style, Weight con Ghost y su fuente,
/// Italic, Monospace y el ancho automático del glifo actual, el modo y la celda, los seis números
/// en una columna y, a su derecha (o debajo, si no cabe), la vista previa de métricas (con la
/// línea del número que tiene el puntero encima resaltada). Todo lo cambiado en un frame (o en
/// un arrastre) es un paso de deshacer (`set_font_info_grouped`).
fn settings(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let Some(font) = editor.font().cloned() else {
        return;
    };
    let (cell_w, cell_h) = (editor.sprite().width, editor.sprite().height);
    let mut info = font.clone();
    // El glifo actual y su ancho (los ajustes no cambian los anchos de los glifos: es el mismo
    // antes y después de `set_font_info`).
    let current = editor
        .current_glyph()
        .and_then(|ch| Some((ch, font.glyphs[font.index_of(ch)?].advance)));
    let ids = [ui.id().with("font_name"), ui.id().with("font_style_name")];
    let h = kit::input::HEIGHT;
    let (mut name, mut style) = (None, None);
    let mut auto_width = None;
    let mut hovered = None;
    let mut preview = None;
    let cell = format!("{cell_w}×{cell_h}");
    let mode = [
        "Mode",
        font_dialog::mode_name(font.mode),
        "Cell",
        cell.as_str(),
    ];
    let cell_label_w = forms::label_width(ui, &cell);
    let baseline = font.metrics.baseline;
    forms::form(ui, "font_settings", ROW_GAP, |tui| {
        // Family y Style.
        name = name_row(tui, skin, ids[0], NAMES[0], &font.name, FAMILY_Y);
        style = name_row(tui, skin, ids[1], NAMES[1], &font.style.name, 0.0);
        // Weight y, en su fila, la letra fantasma (la casilla y su fuente, juntas).
        tui.style(forms::row(WEIGHT_GAP)).add(|tui| {
            tui.style(forms::group(WEIGHT_GAP)).add(|tui| {
                let label_w = FIELD_X - WEIGHT_GAP;
                tui.style(forms::fixed(label_w, h))
                    .ui(|ui| forms::label(ui, "Weight", label_w));
                tui.style(forms::fixed(WEIGHT_W, h)).ui(|ui| {
                    let weight = &mut info.style.weight;
                    let selected = format!("{weight} {}", cs_core::weight_name(*weight));
                    select(ui, skin, "font_weight", &selected, WEIGHT_W, |ui| {
                        for (w, name) in cs_core::WEIGHTS {
                            widgets::choice(ui, weight, w, &format!("{w} {name}"));
                        }
                    });
                });
            });
            tui.style(forms::group(WEIGHT_GAP)).add(|tui| {
                let ghost_w = GHOST_SELECT_X - GHOST_X - WEIGHT_GAP;
                let ghost = forms::air_above(-GHOST_RAISE, forms::fixed(ghost_w, h));
                tui.style(ghost).ui(|ui| {
                    checkbox(ui, skin, &mut info.ghost.enabled, "Ghost");
                });
                tui.style(forms::fixed(GHOST_W, h)).ui(|ui| {
                    let ghost = &mut info.ghost.monospace;
                    let face = if *ghost { "Hack" } else { "Ubuntu" };
                    select(ui, skin, "font_ghost", face, GHOST_W, |ui| {
                        widgets::choice(ui, ghost, false, "Ubuntu");
                        widgets::choice(ui, ghost, true, "Hack");
                    });
                });
            });
        });
        // Italic, Monospace y el ancho automático del glifo actual, en la columna de los campos.
        tui.style(forms::row(ROW_GAP)).add(|tui| {
            tui.style(forms::group(ROW_GAP)).add(|tui| {
                tui.style(forms::fixed(LABEL_W, h)).add_empty();
                let w = CHECKS_X[1] - CHECKS_X[0] - ROW_GAP;
                tui.style(forms::fixed(w, h)).ui(|ui| {
                    checkbox(ui, skin, &mut info.style.italic, "Italic");
                });
            });
            let w = CHECKS_X[2] - CHECKS_X[1] - ROW_GAP;
            tui.style(forms::fixed(w, h)).ui(|ui| {
                checkbox(ui, skin, &mut info.monospace, "Monospace");
            });
            if let Some((ch, advance)) = current {
                tui.style(forms::natural()).ui(|ui| {
                    let mut auto = advance.is_none();
                    let caption = format!("Auto width ({ch})");
                    if checkbox(ui, skin, &mut auto, &caption).changed() {
                        auto_width = Some((ch, auto));
                    }
                });
            }
        });
        // El modo y la celda.
        tui.style(forms::row(ROW_GAP)).add(|tui| {
            for pair in [0, 2] {
                tui.style(forms::group(ROW_GAP)).add(|tui| {
                    for i in [pair, pair + 1] {
                        let w = MODE_X
                            .get(i + 1)
                            .map_or(cell_label_w, |next| next - MODE_X[i] - ROW_GAP);
                        tui.style(forms::fixed(w, h))
                            .ui(|ui| forms::label(ui, mode[i], w));
                    }
                });
            }
        });
        // Los números: las mayúsculas y la x-height hasta la baseline; los descendentes, de ella
        // hasta abajo. Con el puntero sobre una fila (etiqueta o stepper), su línea se resalta en
        // la vista previa, que va a su derecha.
        let numbers = forms::row(PREVIEW_X - NUMBER_X - NUMBER_W);
        tui.style(forms::air_above(NUMBERS_AIR, numbers))
            .add(|tui| {
                let column = forms::Style {
                    flex_shrink: 0.0,
                    ..forms::column(ROW_GAP)
                };
                tui.style(column).add(|tui| {
                    for metric in Metric::ALL {
                        let m = &mut info.metrics;
                        let (value, range) = match metric {
                            Metric::Baseline => (&mut m.baseline, 0..=cell_h),
                            Metric::XHeight => (&mut m.x_height, 0..=baseline),
                            Metric::LetterSpacing => (&mut info.letter_spacing, 0..=8),
                            Metric::CapHeight => (&mut m.cap_height, 0..=baseline),
                            Metric::Descent => (&mut m.descent, 0..=cell_h - baseline.min(cell_h)),
                            Metric::SpaceWidth => (&mut info.space_width, 0..=cell_w),
                        };
                        tui.style(forms::group(0.0)).add(|tui| {
                            let label = tui.style(forms::fixed(NUMBER_X, h)).ui(|ui| {
                                forms::label(ui, metric.label(), NUMBER_X);
                                ui.max_rect()
                            });
                            tui.style(forms::fixed(NUMBER_W, h)).ui(|ui| {
                                number(ui, skin, metric.label(), value, range);
                                if ui.rect_contains_pointer(label.union(ui.max_rect())) {
                                    hovered = Some(metric);
                                }
                            });
                        });
                    }
                });
                let min = PREVIEW_MIN + PREVIEW_RIGHT_AIR;
                tui.style(forms::grow(min, min, None, PREVIEW_H)).ui(|ui| {
                    // Se pinta después, con los cambios de este frame (no en la pasada que mide).
                    preview = ui.is_visible().then(|| ui.max_rect());
                });
            });
    });
    if let Some(name) = name {
        info.name = name;
    }
    if let Some(name) = style.filter(|n: &String| !n.trim().is_empty()) {
        info.style.name = name.trim().to_owned();
    }
    // Con la baseline movida, lo demás se acomoda (solo si se cambió algo: abrir el panel no
    // deja pasos).
    if info.metrics != font.metrics {
        info.metrics = info.metrics.clamped(cell_h);
    }
    // Un arrastre (de un stepper) es un solo paso de undo: se juntan los cambios mientras el
    // botón siga apretado.
    let group = ui.id().with("font_settings").value();
    if info != font {
        editor.set_font_info_grouped(info, group);
    }
    if !ui.input(|i| i.pointer.any_down()) {
        editor.end_group_of(group);
    }
    // El ancho automático (después: `set_font_info` lleva los anchos de antes).
    if let Some((ch, auto)) = auto_width {
        let advance = if auto { None } else { editor.glyph_advance() };
        editor.set_glyph_advance(ch, advance);
    }
    if let Some(rect) = preview {
        metrics_preview(ui, editor, rect, hovered);
    }
}

/// La vista previa de métricas en `rect` (su recuadro, con el marco de las líneas del mapa, sin
/// el aire de la derecha), con lo que la fuente tiene ya con los cambios de este frame.
fn metrics_preview(ui: &Ui, editor: &Editor, rect: Rect, hovered: Option<Metric>) {
    let (Some(font), Some(view)) = (editor.font(), editor.artboard().font_view()) else {
        return;
    };
    let frame = Rect::from_min_max(
        rect.min,
        pos2(rect.right() - PREVIEW_RIGHT_AIR, rect.top() + PREVIEW_H),
    );
    if frame.width() < 3.0 {
        return;
    }
    ui.painter().rect_filled(frame, 0.0, GRID);
    let ch = font_metrics_preview::preview_char(&view, editor.current_glyph());
    let advance = ch
        .and_then(|c| view.glyphs.get(&c))
        .map_or(view.missing_width, |g| g.advance);
    let numbers = font_metrics_preview::Numbers {
        cell: view.cell,
        metrics: view.metrics,
        advance,
        letter_spacing: font.letter_spacing,
        space_width: view.space_width,
    };
    font_metrics_preview::paint(
        ui.painter(),
        frame.shrink(1.0),
        &view,
        ch,
        &numbers,
        hovered,
    );
}

/// La sección Family de los ajustes, del ancho de `ui`: el título con cuántos estilos tiene, un
/// radio por estilo de la familia (el actual marcado; un clic abre su pestaña) con Rebuild en
/// los generados (que baja si no cabe), y New Variant….
fn family_section(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) -> Option<FontAction> {
    let font = editor.font()?.clone();
    let members = editor.family_fonts();
    let hint = match members.len() {
        1 => "(1 style)".to_owned(),
        n => format!("({n} styles)"),
    };
    let title = format!("Family \"{}\"", font.name);
    let h = kit::input::HEIGHT;
    let mut action = None;
    let mut rebuild = None;
    let mut new = false;
    let editor_now: &Editor = editor;
    forms::form(ui, "font_family", ROW_GAP, |tui| {
        tui.style(forms::row(0.0)).add(|tui| {
            tui.style(forms::share(0.0, FAMILY_TITLE_H)).ui(|ui| {
                // Sin el alto mínimo de las filas de egui: el texto, arriba de su fila.
                ui.spacing_mut().interact_size.y = 0.0;
                label_with_hint(ui, &title, &hint);
                // Con nombre, para los lectores de pantalla (y los tests).
                let rect = ui.min_rect();
                let info = crate::panel_kit::label_info(ui, &title);
                ui.interact(rect, ui.id().with("title"), Sense::hover())
                    .widget_info(|| info.clone());
            });
        });
        for (id, style) in &members {
            let on = *id == editor_now.active;
            let generated = style.source.as_ref().map(|s| {
                let from = editor_now
                    .document
                    .artboard(s.artboard)
                    .and_then(|a| a.font.as_ref())
                    .map_or_else(|| "a removed font".to_owned(), |f| f.style.name.clone());
                format!("{} from {from}", s.recipe.kind.label())
            });
            tui.style(forms::row(GLYPH_GAP)).add(|tui| {
                let picked = tui
                    .style(forms::grow(NAME_MIN, NAME_W, Some(NAME_W), h))
                    .ui(|ui| match skin {
                        Some(skin) => kit::check::radio(ui, skin, on, &style.name),
                        None => ui.radio(on, &style.name),
                    });
                if picked.clicked() && !on {
                    action = Some(FontAction::OpenFont(*id));
                }
                if let Some(generated) = &generated {
                    let can = editor_now.can_rebuild(*id);
                    tui.style(forms::natural()).ui(|ui| {
                        let response = ui
                            .add_enabled_ui(can, |ui| match skin {
                                Some(skin) => kit::button::button(ui, skin, "Rebuild"),
                                None => pixel_ui::text_button(ui, "Rebuild"),
                            })
                            .inner
                            .on_hover_text(format!(
                                "{generated}: generate it again (your edits go)"
                            ))
                            .on_disabled_hover_text("Its source font is gone");
                        if response.clicked() {
                            rebuild = Some(*id);
                        }
                    });
                }
            });
        }
        let below = forms::air_above(NEW_VARIANT_AIR, forms::natural());
        tui.style(below).ui(|ui| {
            let response = match skin {
                Some(skin) => kit::button::button(ui, skin, "New Variant…"),
                None => pixel_ui::text_button(ui, "New Variant…"),
            };
            new = response.clicked();
        });
    });
    if let Some(id) = rebuild {
        editor.rebuild_variant(id);
    }
    if new {
        action = Some(FontAction::NewVariant);
    }
    action
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

/// La cuadrícula de la página, con tantas columnas como quepan a lo ancho y líneas de 1
/// compartidas: blanco lo que la fuente tiene, del color del panel lo que falta, con marco por
/// dentro el glifo actual. Cada celda muestra una sola cosa: el glifo dibujado o, si está vacío o
/// falta, su carácter en gris.
fn map(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &FontPanelState) {
    let Some(font) = editor.font().cloned() else {
        return;
    };
    let chars = state.page.chars(&font);
    let current = editor.current_glyph();
    let view = editor.artboard().font_view();
    let cell_size = (editor.sprite().width, editor.sprite().height);
    let scale = thumb_scale(cell_size);
    let fit = (((ui.available_width() - 1.0) / PITCH_X).floor() as usize).max(1);
    let rows = chars.len().div_ceil(fit);
    let columns = chars.len().min(fit);
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
            origin + vec2((i % fit) as f32 * PITCH_X, (i / fit) as f32 * PITCH_Y),
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
        let composite = font
            .index_of(ch)
            .is_some_and(|i| font.glyphs[i].is_composite());
        if composite {
            let mark = Rect::from_min_size(cell.min + vec2(1.0, 1.0), Vec2::splat(3.0));
            painter.rect_filled(mark, 0.0, COMPOSITE_MARK);
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
        menus::context_menu(&response, |ui| glyph_menu(ui, editor, ch, has, composite));
    }
}

/// El menú de una celda: Copy, Paste, Clear y Remove si la fuente tiene el carácter (y Unlink si
/// es compuesto); Add si no. Build composite en los acentuados de la tabla que no son compuestos
/// (solo arma los vacíos o los que faltan).
fn glyph_menu(ui: &mut Ui, editor: &mut Editor, ch: char, has: bool, composite: bool) {
    let buildable = cs_core::decomposition(ch).is_some();
    if !has {
        if ui.add(menus::plain("Add")).clicked() {
            editor.add_glyphs(&[ch]);
            editor.go_to_glyph(ch);
            ui.close();
        }
        if buildable && ui.add(menus::plain("Build composite")).clicked() {
            editor.auto_build_accents(Some(&[ch]), true);
            editor.go_to_glyph(ch);
            ui.close();
        }
        return;
    }
    if composite && ui.add(menus::plain("Unlink")).clicked() {
        editor.unlink_glyph(ch);
        ui.close();
    }
    if buildable && !composite && ui.add(menus::plain("Build composite")).clicked() {
        editor.go_to_glyph(ch);
        editor.auto_build_accents(Some(&[ch]), true);
        ui.close();
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
