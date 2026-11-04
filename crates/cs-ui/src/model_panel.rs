//! El panel 3D (en el dock del lienzo; en la pestaña del modelo, fijo en su lugar): el modelo 3D
//! del artboard activo, partido en dos a lo alto por una línea grabada (a ≈ 48 %). Arriba, con su
//! scroll, lo general del modelo: los modos de selección y la cuadrícula (usables en modo Model), la
//! textura y su tamaño, la cámara, la luz, el contorno, Shading… (la cuadrícula, en su ventana)
//! y Sheet settings…. Abajo, la tira de pestañas de los objetos (una por objeto, con el ícono de su tipo:
//! un clic la elige, su marca quita el objeto y, si no caben, las flechas las recorren) con el +
//! a la derecha (su ventanita, en [`add_menu`]: el tipo, los lados de los redondos y Add) y,
//! debajo, con su scroll, lo del objeto elegido: en modo Model, Extrude, Subdivide, Merge y
//! Delete; en modo Texture, los botones de las UVs (girar y voltear las de las caras elegidas); su
//! posición, su rotación y su escala en filas de X, Y y Z, y Double sided. Sin objeto elegido,
//! abajo solo la tira. Sin modelo, el botón New 3D Model….

use cs_3d::Primitive;
use cs_core::{CameraPreset, Model3D, Object3D, PrimitiveKind, Projection, Rgba, ShadingTable};
use eframe::egui::{
    self, Color32, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::kit::tabs::{TabEvent, TabItem};

use crate::dock::FLYOUT_MARGIN;
use crate::editor::{CadMode, Editor, wrap_angle};
use crate::font_panel::lit_line;
use crate::kit;
use crate::menus;
use crate::panel_kit::label_with_hint;
use crate::pixel_ui;
use crate::skin::Skin;
use crate::tabs::TabIcon;

mod add_menu;

/// Ancho de adentro del panel (sin su marco): el contenido y la barra del scroll.
pub const WIDTH: f32 = 300.0;
/// Las pestañas de los objetos, 5 filas debajo de la línea que parte el panel (3 de aire después
/// de sus dos filas), sobre su propia línea grabada.
const TABS_TOP: f32 = 5.0;
/// Sin Pixel UI: la fila de pestañas de egui (más alta) y la línea debajo, desde la que parte el
/// panel.
const PLAIN_TABS_TOP: f32 = 8.0;
const PLAIN_HEADER_LINE: f32 = 29.0;
/// Dónde parte la línea el panel, desde arriba de su caja: como en el boceto, 197 de sus 408
/// filas (≈ 48 %: lo general cabe entero sin scroll).
const SPLIT: f32 = 197.0 / 408.0;
/// Del borde de arriba de cada mitad a lo primero, y lo mismo abajo.
const MIDDLE_AIR: f32 = 4.0;
/// De la línea de las pestañas a lo primero del objeto (en modo Texture, los botones de las UVs):
/// 9 filas (como el boceto), con [`MIDDLE_AIR`]. La fila de los botones es media fila más alta
/// que ellos y los centra: el botón cae (redondeado) una fila más abajo.
const OBJECT_TOP_AIR: f32 = 9.0 - MIDDLE_AIR - 1.0;
/// Entre una cosa y otra del medio.
const ITEM_GAP: f32 = 4.0;
/// Como el boceto: la fila de los modos, 12 filas debajo de lo alto de la caja (2 más que el
/// aire de arriba), y de ella a la fila de la textura, 42.
const MODE_ROW_AIR: f32 = 2.0;
const TEXTURE_AIR: f32 = 3.0;
/// Y de la fila de la luz a la de Shading… y Sheet settings…, 37 (como entre las otras filas).
const BUTTONS_AIR: f32 = 3.0;
/// El + de la tira: del alto de las flechas, 4 px después de ellas (como en el boceto).
const PLUS_W: f32 = 13.0;
const PLUS_GAP: f32 = 4.0;
/// La fila de los modos de selección (como el boceto): sus botones, uno cada 17 px y 2 filas
/// debajo de lo alto del select de la cuadrícula (al final de la fila, de 46 de ancho).
const MODE_PITCH: f32 = kit::tile::SIZE.x + 4.0;
const MODE_TOP: f32 = 2.0;
const GRID_W: f32 = 46.0;
/// Los lados del tamaño de la textura.
const TEXTURE_SIZES: [u32; 6] = [16, 32, 64, 128, 256, 512];
/// Los lados de cilindros, conos y esferas.
const MIN_SIDES: u32 = 3;
const MAX_SIDES: u32 = 32;
const DEFAULT_SIDES: u32 = 8;
/// El transform del objeto: los tres ejes en una fila de steppers con su etiqueta, con este aire
/// entre uno y otro (y antes de la barra del scroll), como en el boceto.
const AXIS_GAP: f32 = 4.0;
/// Lo menos de un stepper de eje: su etiqueta, sus botones y un campo donde quepa "-4096".
const AXIS_MIN_W: f32 = kit::stepper::TAG_W + 2.0 * kit::stepper::BUTTON_W + 30.0;
/// El título se acerca a su fila: egui deja debajo de la letra su aire y el de [`ITEM_GAP`], y el
/// boceto pone 4 filas de la letra al stepper (y 10 del stepper al título que sigue).
const AXIS_TITLE_PULL: f32 = 7.0;

/// Cuántos steppers de eje van por fila en `avail` (tres; si no caben, dos) y de qué ancho.
fn axis_grid(avail: f32) -> (usize, f32) {
    let per_row = if 3.0 * AXIS_MIN_W + 2.0 * AXIS_GAP <= avail {
        3
    } else {
        2
    };
    let gaps = (per_row - 1) as f32 * AXIS_GAP;
    (per_row, ((avail - gaps) / per_row as f32).floor())
}

/// La fila de la luz (como el boceto): de la casilla Light a "Yaw", y de cada etiqueta a su
/// stepper; entre el stepper de Yaw y "Pitch", [`PAIR_GAP`].
const LIGHT_GAP: f32 = 12.0;
const LABEL_GAP: f32 = 4.0;
/// Las filas de la textura y de la cámara (como el boceto): la columna de la derecha (Texture
/// Size) empieza a 5/8 de la fila (138 de sus 220 columnas), con este aire antes; el aire entre
/// el select de la cámara y sus presets es el mismo.
const RIGHT_COLUMN: f32 = 0.625;
const COLUMN_GAP: f32 = 4.0;
/// Lo que dice un select sin nada elegido.
const NO_OPTION: &str = "Select an option…";
/// Los presets de la cámara, en su grupo de botones.
const PRESETS: [(&str, CameraPreset); 4] = [
    ("Iso", CameraPreset::Iso),
    ("Top", CameraPreset::Top),
    ("Side", CameraPreset::Side),
    ("Front", CameraPreset::Front),
];
/// Hasta dónde llegan Yaw (en grados) y Pitch (en grados).
const YAW_MAX: i32 = 360;
const PITCH_MAX: i32 = 90;
const PAIR_GAP: f32 = 8.0;
/// Hasta dónde llegan la posición (en 1/16: ±256 unidades), la rotación (en grados) y la escala
/// (en %).
const POSITION_MAX: i32 = 256 * cs_core::FIXED;
const ROTATION_MAX: i32 = 360;
const SCALE_MAX: i32 = 1000;
/// La cuadrícula Shading (y las muestras de su popup): casillas de 12 con líneas de 1
/// compartidas, del color de las del mapa de FontForge, y hasta 16 columnas por bloque.
const SHADE_CELL: f32 = 12.0;
const SHADE_PITCH: f32 = SHADE_CELL + 1.0;
const SHADE_COLUMNS: usize = 16;
const SHADE_GRID: Color32 = Color32::from_rgb(136, 110, 106);
/// El punto de una casilla fijada sobre un color claro.
const PIN_DARK: Color32 = Color32::from_rgb(5, 4, 3);

/// El estado del panel entre frames.
pub struct ModelPanelState {
    /// Los lados de lo que se agregue redondo (cilindro, cono, esfera).
    pub sides: u32,
    /// El tipo elegido en el menú del + (de entrada, el cubo).
    pub add_kind: PrimitiveKind,
    /// Abrir el menú del + en el próximo frame (la demo, para capturas).
    pub open_add_menu: bool,
    /// La ventana Shading está abierta ([`shading_window`]).
    pub shading: bool,
}

impl Default for ModelPanelState {
    fn default() -> Self {
        ModelPanelState {
            sides: DEFAULT_SIDES,
            add_kind: PrimitiveKind::Cube,
            open_add_menu: false,
            shading: false,
        }
    }
}

/// Lo que el usuario pidió en el panel (lo resuelve la app).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelAction {
    NewModel,
    /// Bake…: abrir el diálogo de hornear el modelo.
    Bake,
    /// Shading…: abrir la ventana de la tabla de sombras.
    Shading,
    /// Pasar la textura a este lado, que deja UVs fuera: la app pide confirmación antes.
    TextureSize(u32),
}

/// El panel en `ui` (lo de adentro del marco, de la columna entera: ver
/// [`crate::dock::flyout_fixed`] y [`crate::dock::fixed_panel`]): arriba lo general, la línea a
/// [`SPLIT`] del alto y abajo la tira de pestañas y lo del objeto elegido, cada parte con su
/// scroll.
pub fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut ModelPanelState,
) -> Option<ModelAction> {
    let inner = ui.max_rect();
    // La caja del panel, como la pinta el dock: en puntos enteros.
    let outer = inner.expand(FLYOUT_MARGIN as f32);
    let outer = Rect::from_min_size(outer.min.round(), outer.size().round());
    // Recién abierto: arriba. egui guarda el desplazamiento del scroll (y lo persiste entre
    // sesiones), así que sin esto el panel reabre donde se dejó, con Edit y los modos ocultos.
    // Si el frame anterior no se dibujó el panel, es que estaba cerrado.
    let shown_id = egui::Id::new("model_panel_last_pass");
    let pass = ui.ctx().cumulative_pass_nr();
    let last: Option<u64> = ui.data(|d| d.get_temp(shown_id));
    let reopened = last.is_none_or(|last| last + 1 < pass);
    ui.data_mut(|d| d.insert_temp(shown_id, pass));
    let action = if editor.edited_model().is_none() {
        // Sin modelo, todo el panel para New 3D Model… (sin línea ni pestañas).
        scrolled(ui, "model_panel_middle", inner, reopened, |ui| {
            let new = button(ui, skin, "New 3D Model…");
            ui.add_space(ITEM_GAP);
            ui.add(egui::Label::new("The active artboard is not a 3D model.").wrap());
            new.clicked().then_some(ModelAction::NewModel)
        })
    } else {
        let split = split_row(outer);
        let top = Rect::from_x_y_ranges(inner.x_range(), inner.top()..=split);
        let action = scrolled(ui, "model_panel_general", top, reopened, |ui| {
            ui.spacing_mut().item_spacing.y = ITEM_GAP;
            general(ui, editor, skin)
        });
        divider(ui, skin, outer, split);
        let line = header(ui, editor, skin, state, outer, inner, split);
        if editor.selected_object().is_some() {
            let bottom = Rect::from_x_y_ranges(
                inner.x_range(),
                line + pixel_ui::ETCHED_SIZE..=inner.bottom(),
            );
            scrolled(ui, "model_panel_object", bottom, reopened, |ui| {
                ui.spacing_mut().item_spacing.y = ITEM_GAP;
                selected(ui, editor, skin);
            });
        }
        action
    };
    // Todo el panel es de la columna (las mitades no lo dicen: están en su propio `Ui`).
    ui.advance_cursor_after_rect(inner);
    action
}

/// La fila de la línea que parte el panel: a [`SPLIT`] del alto de su caja `outer`, en puntos
/// enteros.
fn split_row(outer: Rect) -> f32 {
    outer.top() + (outer.height() * SPLIT).floor()
}

/// La línea que parte el panel en la fila `top`: grabada, como la de la tira de pestañas, de lado
/// a lado de lo de adentro de la caja `outer` (sin Pixel UI, la de egui).
fn divider(ui: &Ui, skin: Option<&Skin>, outer: Rect, top: f32) {
    let x = outer.x_range().shrink(1.0);
    match skin {
        Some(skin) => {
            let ppp = ui.ctx().pixels_per_point();
            let line = pixel_ui::etched_hline(x, top, skin.colors(), ppp);
            ui.painter().add(line);
        }
        None => lit_line(ui, None, outer, top),
    }
}

/// Una mitad del panel en `rect`, con su propio scroll (`salt`) y [`MIDDLE_AIR`] arriba y abajo.
/// Con `reset`, desde arriba.
fn scrolled<R>(
    ui: &mut Ui,
    salt: &str,
    rect: Rect,
    reset: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let mut area = egui::ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .max_height(rect.height())
        .min_scrolled_height(rect.height());
    if reset {
        area = area.vertical_scroll_offset(0.0);
    }
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        pixel_ui::scroll(ui, area, |ui| {
            ui.add_space(MIDDLE_AIR);
            let inner = add_contents(ui);
            ui.add_space(MIDDLE_AIR);
            inner
        })
    })
    .inner
}

/// La pieza del ícono de un tipo de objeto.
fn kind_icon(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Cube => "icon.object.cube",
        PrimitiveKind::Plane => "icon.object.plane",
        PrimitiveKind::Cylinder => "icon.object.cylinder",
        PrimitiveKind::Cone => "icon.object.cone",
        PrimitiveKind::Sphere => "icon.object.sphere",
    }
}

/// Las pestañas de los objetos de `model`: el ícono de su tipo, su nombre y la marca ○ (que pasa
/// a × con el puntero encima) para quitarlo.
pub(crate) fn object_tabs(model: &Model3D) -> Vec<TabItem> {
    model
        .objects
        .iter()
        .map(|o| {
            TabItem::closable(o.name.clone(), true)
                .with_close_label("Delete")
                .with_icon(TabIcon::Piece(kind_icon(o.shown_kind())))
        })
        .collect()
}

/// Arriba de la mitad de abajo (desde la línea que parte el panel, en la fila `split`), fija: la
/// tira de pestañas de los objetos (la del kit, estilo documento, en lo de adentro de la caja; su
/// línea grabada, de lado a lado) con el + a la derecha o, sin Pixel UI, una fila de egui y una
/// línea. La pestaña `i` es la del objeto `i`: un clic la elige; su marca lo quita (un paso de
/// undo). Sin objeto elegido, ninguna activa. El + agrega lo elegido en su menú (y lo elige).
/// Devuelve la fila de arriba de la línea de la tira.
fn header(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut ModelPanelState,
    outer: Rect,
    inner: Rect,
    split: f32,
) -> f32 {
    let items = editor.edited_model().map(object_tabs).unwrap_or_default();
    let active = editor.selected_object();
    let id = ui.id().with("model_object_tabs");
    let mut added = None;
    let (line, event) = match skin {
        Some(skin) => {
            let top = split + TABS_TOP;
            let mut strip = kit::tabs::Strip::new(inner.left(), inner.right(), top);
            strip.line = outer.x_range().shrink(1.0);
            // El +, al final de la fila; las pestañas y sus flechas, antes.
            let plus = Rect::from_min_size(
                pos2(inner.right() - PLUS_W, top + kit::tabs::ARROWS_TOP),
                vec2(PLUS_W, kit::tabs::ARROW_SIZE),
            );
            strip.right = plus.left() - PLUS_GAP;
            added = add_menu::plus_button(ui, skin, plus, state);
            let event = kit::tabs::tab_strip(ui, skin, id, strip, &items, active);
            (strip.bottom(), event)
        }
        None => {
            let line = split + PLAIN_HEADER_LINE;
            let row = Rect::from_x_y_ranges(inner.x_range(), split + PLAIN_TABS_TOP..=line);
            let event = ui
                .scope_builder(UiBuilder::new().max_rect(row), |ui| {
                    plain_tabs(ui, &items, active, state, &mut added)
                })
                .inner;
            lit_line(ui, None, outer, line);
            (line, event)
        }
    };
    match event {
        Some(TabEvent::Select(i)) => editor.select_model_object(Some(i)),
        Some(TabEvent::Close(i)) => {
            editor.remove_object(i);
        }
        None => {}
    }
    if let Some(p) = added {
        editor.add_primitive(p);
    }
    line
}

/// Sin Pixel UI: una fila de egui (con scroll de lado) con el nombre de cada pestaña (un clic la
/// elige), el × de las que se cierran (quita su objeto) y, al final, el + con su menú (lo
/// agregado queda en `added`).
fn plain_tabs(
    ui: &mut Ui,
    items: &[TabItem],
    active: Option<usize>,
    state: &mut ModelPanelState,
    added: &mut Option<Primitive>,
) -> Option<TabEvent> {
    let mut event = None;
    egui::ScrollArea::horizontal()
        .id_salt("model_object_tabs_plain")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, item) in items.iter().enumerate() {
                    if ui
                        .selectable_label(active == Some(i), &item.label)
                        .clicked()
                    {
                        event = Some(TabEvent::Select(i));
                    }
                    if item.mark != kit::tabs::TabMark::None {
                        let close = ui.small_button("×").on_hover_text(item.close_label);
                        let label = format!("{} {}", item.close_label, item.label);
                        close.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label)
                        });
                        if close.clicked() {
                            event = Some(TabEvent::Close(i));
                        }
                    }
                    ui.add_space(4.0);
                }
                *added = add_menu::plain_menu(ui, state);
            });
        });
    event
}

/// Lo general del modelo (la mitad de arriba): en modo Model, los modos de selección y la
/// cuadrícula; la textura y su tamaño; la cámara, la luz y el contorno, y Shading… y Sheet
/// settings… (hornear).
fn general(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) -> Option<ModelAction> {
    edit_rows(ui, editor, skin);
    let action = texture(ui, editor, skin);
    settings(ui, editor, skin);
    ui.add_space(BUTTONS_AIR);
    let window = window_buttons(ui, skin);
    action.or(window)
}

/// Lo del objeto elegido, en modo Texture: Rotate 90°, Flip H y Flip V para las caras elegidas
/// (sin caras, deshabilitados), en una fila sin título (como el boceto).
fn uv_buttons(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let faces = !editor.selected_faces().is_empty();
    let (mut rotate, mut flip_h, mut flip_v) = (false, false, false);
    ui.add_enabled_ui(faces, |ui| {
        ui.horizontal(|ui| {
            rotate = button(ui, skin, "Rotate 90°").clicked();
            flip_h = button(ui, skin, "Flip H").clicked();
            flip_v = button(ui, skin, "Flip V").clicked();
        });
    });
    if rotate {
        editor.rotate_selected_uvs();
    }
    if flip_h || flip_v {
        editor.flip_selected_uvs(flip_h);
    }
}

/// Lo del objeto elegido, en modo Model: Extrude, Subdivide, Merge y Delete para lo elegido en
/// las vistas (cada uno deshabilitado cuando no aplica), en una fila sin título como la de las
/// UVs. Hacen lo mismo que Ctrl+E, Ctrl+D, M y Supr.
fn topology_buttons(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let enabled = [
        editor.can_extrude(),
        editor.can_subdivide(),
        editor.can_merge(),
        editor.can_delete_model_selection(),
    ];
    let labels = ["Extrude", "Subdivide", "Merge", "Delete"];
    let mut clicked = None;
    ui.horizontal(|ui| {
        for (i, (label, on)) in labels.into_iter().zip(enabled).enumerate() {
            if ui
                .add_enabled_ui(on, |ui| button(ui, skin, label))
                .inner
                .clicked()
            {
                clicked = Some(i);
            }
        }
    });
    match clicked {
        Some(0) => editor.request_extrude(),
        Some(1) => {
            editor.subdivide_model_selection();
        }
        Some(2) => {
            editor.merge_model_selection();
        }
        Some(3) => {
            editor.delete_model_selection();
        }
        _ => {}
    }
}

/// El ancho de las filas de lo general: lo de adentro sin la barra del scroll y el aire de antes
/// (como las filas de los ejes).
fn row_width(ui: &Ui) -> f32 {
    ((WIDTH - pixel_ui::SCROLL_BAR).min(ui.available_width()) - AXIS_GAP).floor()
}

/// El lugar de una fila de selects (y botones) de `width`, en puntos enteros. Sin el aire de entre
/// filas arriba: como las filas de los ejes (que egui centra en una fila más baja), queda a 4 filas
/// de la letra de su título y a 10 del título que sigue, como el boceto.
fn row_rect(ui: &mut Ui, width: f32) -> Rect {
    ui.add_space(-ui.spacing().item_spacing.y);
    let (slot, _) = ui.allocate_exact_size(vec2(width, kit::select::HEIGHT), Sense::hover());
    Rect::from_min_size(slot.min.round(), slot.size())
}

/// Lo que agregue `add_contents` de izquierda a derecha (centrado a lo alto) en una fila de
/// [`row_rect`]: con el mismo ritmo que las de los selects, aunque lo de adentro sea más bajo o
/// más alto.
fn in_row<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let row = row_rect(ui, row_width(ui));
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    let mut child = ui.new_child(UiBuilder::new().max_rect(row).layout(layout));
    add_contents(&mut child)
}

/// Dos títulos en una fila: `left` al principio (en `ui`, como cualquier título) y `right` a la
/// misma altura en la columna `column` (desde el principio de la fila), en su propio `Ui`.
fn pair_labels(ui: &mut Ui, (left, right): (&str, &str), column: f32) {
    let at = ui.cursor().min + vec2(column, 0.0);
    let rect = Rect::from_min_size(at, vec2((ui.available_width() - column).max(0.0), 0.0));
    let mut second = ui.new_child(UiBuilder::new().max_rect(rect));
    label_with_hint(&mut second, right, "");
    label_with_hint(ui, left, "");
    ui.add_space(-AXIS_TITLE_PULL);
}

/// Lo que pide el select de Texture Size: achicar la textura dejando UVs fuera lo confirma la app
/// (`ModelAction::TextureSize`).
fn resize_texture(editor: &mut Editor, side: u32) -> Option<ModelAction> {
    if editor.texture_size_cuts_uvs(side) {
        return Some(ModelAction::TextureSize(side));
    }
    editor.set_texture_size(side);
    None
}

/// El texto de una cuadrícula (en 1/16 de unidad).
fn grid_label(g: i32) -> &'static str {
    match g {
        1 => "1/16",
        2 => "1/8",
        4 => "1/4",
        8 => "1/2",
        _ => "1",
    }
}

/// La pieza del ícono de un modo de selección.
fn mode_icon(mode: cs_3d::SelectMode) -> &'static str {
    match mode {
        cs_3d::SelectMode::Object => "icon.mode.object",
        cs_3d::SelectMode::Vertex => "icon.mode.vertex",
        cs_3d::SelectMode::Edge => "icon.mode.edge",
        cs_3d::SelectMode::Face => "icon.mode.face",
    }
}

/// Los modos de selección y la cuadrícula del modo Edit (el del workspace del modelo), como el
/// boceto: los íconos de los modos (con su tooltip) al principio de la fila y "Grid" con su
/// select al final. En modo Texture se ven pero no se usan (la fila no se va, así lo demás no
/// salta al cambiar de modo).
fn edit_rows(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let current = editor.model_edit().selection.mode;
    let grid = editor.model_edit().grid;
    let mut picked = None;
    let mut chosen = None;
    let list = |ui: &mut Ui| {
        for g in crate::editor::GRIDS {
            if ui.add(menus::choice(g == grid, grid_label(g))).clicked() {
                chosen = Some(g);
                ui.close();
            }
        }
    };
    let enabled = editor.cad_mode() == CadMode::Model;
    match skin {
        Some(skin) => {
            ui.add_space(MODE_ROW_AIR);
            let row = row_rect(ui, row_width(ui));
            let id = ui.id();
            ui.add_enabled_ui(enabled, |ui| {
                for (k, mode) in cs_3d::SelectMode::ALL.into_iter().enumerate() {
                    let at = pos2(row.left() + k as f32 * MODE_PITCH, row.top() + MODE_TOP);
                    let tile = id.with(("model_select_mode", k));
                    let on = current == mode;
                    let icon = mode_icon(mode);
                    if kit::tile::tile_toggle_at(ui, skin, at, tile, icon, mode.label(), on)
                        .clicked()
                    {
                        picked = Some(mode);
                    }
                }
                let field = Rect::from_min_max(pos2(row.right() - GRID_W, row.top()), row.max);
                let w = label_width(ui, "Grid");
                let label = Rect::from_min_max(
                    pos2(field.left() - LABEL_GAP - w, row.top()),
                    pos2(field.left() - LABEL_GAP, row.bottom()),
                );
                let layout = egui::Layout::left_to_right(egui::Align::Center);
                ui.new_child(UiBuilder::new().max_rect(label).layout(layout))
                    .label("Grid");
                kit::select::select_at(
                    ui,
                    skin,
                    field,
                    id.with("model_grid"),
                    grid_label(grid),
                    list,
                );
            });
        }
        None => {
            ui.add_enabled_ui(enabled, |ui| {
                ui.horizontal(|ui| {
                    for mode in cs_3d::SelectMode::ALL {
                        if radio(ui, None, current == mode, mode.label()).clicked() {
                            picked = Some(mode);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Grid");
                    egui::ComboBox::from_id_salt("model_grid")
                        .selected_text(grid_label(grid))
                        .show_ui(ui, list);
                });
            });
        }
    }
    if let Some(mode) = picked {
        editor.set_select_mode(mode);
    }
    if let Some(g) = chosen {
        editor.set_model_grid(g);
    }
}

/// Un botón: el del kit (sin Pixel UI, el de texto).
fn button(ui: &mut Ui, skin: Option<&Skin>, label: &str) -> egui::Response {
    match skin {
        Some(skin) => kit::button::button(ui, skin, label),
        None => pixel_ui::text_button(ui, label),
    }
}

/// Un ángulo de la luz: la etiqueta y, a [`LABEL_GAP`], el stepper de `width`. Un valor fuera de
/// `range` se muestra en el borde pero solo se escribe si el usuario lo cambia (el stepper no lo
/// recorta solo).
fn angle_cell(
    ui: &mut Ui,
    skin: Option<&Skin>,
    width: f32,
    (label, value): (&str, &mut i32),
    range: std::ops::RangeInclusive<i32>,
) {
    ui.label(label);
    ui.add_space(LABEL_GAP);
    let id = format!("light_{label}");
    let shown = (*value).clamp(*range.start(), *range.end());
    let mut v = shown;
    match skin {
        Some(skin) => {
            kit::stepper::stepper(ui, skin, width, &id, &mut v, range, 1, None);
        }
        None => {
            ui.add(egui::DragValue::new(&mut v).range(range));
        }
    }
    if v != shown {
        *value = v;
    }
}

/// Un eje: el stepper de `width` con su etiqueta (`axis`) pegada a la izquierda. Un valor fuera
/// de `range` se muestra en el borde pero solo se escribe si el usuario lo cambia (el stepper no
/// lo recorta solo).
fn axis_cell(
    ui: &mut Ui,
    skin: Option<&Skin>,
    (title, width): (&str, f32),
    (axis, value): (&str, &mut i32),
    range: std::ops::RangeInclusive<i32>,
) {
    let id = format!("{title}_{axis}");
    let shown = (*value).clamp(*range.start(), *range.end());
    let mut v = shown;
    match skin {
        Some(skin) => {
            kit::stepper::axis_stepper(ui, skin, width, &id, axis, &mut v, range, 1);
        }
        None => {
            ui.label(axis);
            ui.add(egui::DragValue::new(&mut v).range(range));
        }
    }
    if v != shown {
        *value = v;
    }
}

/// Un título con su unidad y, debajo, los tres ejes en una fila de steppers con su etiqueta (X, Y,
/// Z) que llena lo de adentro sin la barra del scroll; si no caben, X e Y en una fila y Z debajo.
fn axes(
    ui: &mut Ui,
    skin: Option<&Skin>,
    (title, unit): (&str, &str),
    values: [&mut i32; 3],
    range: std::ops::RangeInclusive<i32>,
) {
    label_with_hint(ui, title, unit);
    ui.add_space(-AXIS_TITLE_PULL);
    let avail = (WIDTH - pixel_ui::SCROLL_BAR).min(ui.available_width()) - AXIS_GAP;
    let (per_row, width) = axis_grid(avail);
    let mut cells = ["X", "Y", "Z"].into_iter().zip(values);
    loop {
        let row: Vec<_> = cells.by_ref().take(per_row).collect();
        if row.is_empty() {
            break;
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = AXIS_GAP;
            for cell in row {
                axis_cell(ui, skin, (title, width), cell, range.clone());
            }
        });
    }
}

/// Los tres ejes de `v` mostrados en otra unidad: `per` de lo guardado (pasado antes por `show`)
/// por cada uno mostrado. Solo se escribe el eje que cambió (los otros no pierden lo que no se
/// ve).
fn scaled_axes(
    ui: &mut Ui,
    skin: Option<&Skin>,
    labels: (&str, &str),
    (v, show): (&mut cs_core::Vec3i, fn(i32) -> i32),
    per: i32,
    range: std::ops::RangeInclusive<i32>,
) {
    let before = [show(v.x) / per, show(v.y) / per, show(v.z) / per];
    let mut shown = before;
    let [a, b, c] = &mut shown;
    axes(ui, skin, labels, [a, b, c], range);
    for (axis, (now, was)) in [&mut v.x, &mut v.y, &mut v.z]
        .into_iter()
        .zip(shown.into_iter().zip(before))
    {
        if now != was {
            *axis = now * per;
        }
    }
}

/// Lo del objeto elegido (la mitad de abajo): primero los botones de topología (en modo Model) o
/// los de las UVs (en modo Texture);
/// posición, rotación y escala (una fila de X, Y, Z cada una) y Double sided. Quitarlo es la ○ de
/// su pestaña. Todo lo cambiado en un frame (o en un arrastre) es un paso de deshacer
/// (`set_model_grouped`).
fn selected(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let Some(i) = editor.selected_object() else {
        return;
    };
    let Some(model) = editor.edited_model().cloned() else {
        return;
    };
    // Los títulos de los ejes van con el aire de entre una fila y otra (como en el boceto).
    ui.add_space(OBJECT_TOP_AIR);
    match editor.cad_mode() {
        CadMode::Texture => uv_buttons(ui, editor, skin),
        CadMode::Model => topology_buttons(ui, editor, skin),
    }
    let obj: &Object3D = &model.objects[i];
    let mut next = obj.clone();
    let t = &mut next.transform;
    let p = &mut t.position;
    axes(
        ui,
        skin,
        ("Position", "(1/16)"),
        [&mut p.x, &mut p.y, &mut p.z],
        -POSITION_MAX..=POSITION_MAX,
    );
    scaled_axes(
        ui,
        skin,
        ("Rotation", "(°)"),
        (&mut t.rotation, wrap_angle),
        10,
        -ROTATION_MAX..=ROTATION_MAX,
    );
    let scale = (&mut t.scale, std::convert::identity as fn(i32) -> i32);
    scaled_axes(ui, skin, ("Scale", "(%)"), scale, 10, 1..=SCALE_MAX);
    ui.add_space(ITEM_GAP);
    match skin {
        Some(skin) => kit::check::checkbox(ui, skin, &mut next.double_sided, "Double sided"),
        None => ui.checkbox(&mut next.double_sided, "Double sided"),
    };
    // Un arrastre (de un stepper) es un solo paso de undo: se juntan los cambios mientras el
    // botón siga apretado.
    let group = ui.id().with("model_object");
    if next != *obj {
        let mut changed = model.clone();
        changed.objects[i] = next;
        editor.set_model_grouped(changed, group);
    }
    if !ui.input(|input| input.pointer.any_down()) {
        editor.end_group_of(group.value());
    }
}

/// Un radio: el del kit (sin Pixel UI, el de texto).
fn radio(ui: &mut Ui, skin: Option<&Skin>, on: bool, label: &str) -> egui::Response {
    match skin {
        Some(skin) => kit::check::radio(ui, skin, on, label),
        None => ui.radio(on, label),
    }
}

/// Una casilla: la del kit (sin Pixel UI, la de egui).
fn check(ui: &mut Ui, skin: Option<&Skin>, on: &mut bool, label: &str) {
    match skin {
        Some(skin) => kit::check::checkbox(ui, skin, on, label),
        None => ui.checkbox(on, label),
    };
}

/// La textura y su tamaño, como el boceto: arriba "Texture" y, en la columna de la derecha,
/// "Texture Size"; debajo, el select de la textura (los artboards del lienzo que no son modelos ni
/// fuentes, y None; sin textura dice "Select an option…" y una que ya no existe, "(missing)") con
/// New pegado a la derecha (agrega una plantilla y la asigna) y, en la otra columna, el del tamaño
/// (vacío y deshabilitado sin textura).
fn texture(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) -> Option<ModelAction> {
    let current = editor.edited_model().map(|m| m.texture)?;
    ui.add_space(TEXTURE_AIR);
    let row_w = row_width(ui);
    let column = (row_w * RIGHT_COLUMN).round();
    pair_labels(ui, ("Texture", "Texture Size"), column);
    let options: Vec<(cs_core::ArtboardId, String)> = editor
        .document
        .artboards
        .iter()
        .filter(|a| a.on_canvas())
        .map(|a| (a.id, a.name.clone()))
        .collect();
    // La textura puede no estar en la lista (en la biblioteca) o ya no existir.
    let shown = match current {
        None => NO_OPTION.to_owned(),
        Some(id) => editor
            .document
            .artboard(id)
            .map_or_else(|| "(missing)".to_owned(), |a| a.name.clone()),
    };
    // El tamaño de la textura del modelo que se edita (también fuera de su workspace).
    let size = editor
        .edited_texture()
        .and_then(|id| editor.document.artboard(id))
        .map(|a| (a.sprite.width, a.sprite.height));
    let size_shown = match size {
        Some((w, h)) if w == h => w.to_string(),
        Some((w, h)) => format!("{w}×{h}"),
        None => String::new(),
    };
    let mut picked = None;
    let list = |ui: &mut Ui| {
        if ui.add(menus::choice(current.is_none(), "None")).clicked() {
            picked = Some(None);
            ui.close();
        }
        for (id, name) in &options {
            if ui.add(menus::choice(current == Some(*id), name)).clicked() {
                picked = Some(Some(*id));
                ui.close();
            }
        }
    };
    let mut side = None;
    let sides = |ui: &mut Ui| {
        for s in TEXTURE_SIZES {
            let on = size == Some((s, s));
            if ui.add(menus::choice(on, &s.to_string())).clicked() {
                side = Some(s);
                ui.close();
            }
        }
    };
    let new = match skin {
        Some(skin) => {
            let row = row_rect(ui, row_w);
            let id = ui.id();
            // New, pegado al select: comparten la columna del borde. Va primero, así el borde del
            // select queda encima (como en el boceto).
            let end = row.left() + column - COLUMN_GAP;
            let new_w = kit::button::width(ui, "New");
            let new_rect =
                Rect::from_min_max(pos2(end - new_w, row.top()), pos2(end, row.bottom()));
            let new_id = id.with("model_texture_new");
            let new = kit::button::button_at(ui, skin, new_rect, new_id, "New").clicked();
            let field = Rect::from_min_max(row.min, pos2(new_rect.left() + 1.0, row.bottom()));
            kit::select::select_at(ui, skin, field, id.with("model_texture"), &shown, list);
            let size_rect = Rect::from_min_max(pos2(row.left() + column, row.top()), row.max);
            // En su propio `Ui` (deshabilitado sin textura), que no ocupa lugar en la columna.
            let mut column_ui = ui.new_child(UiBuilder::new().max_rect(size_rect));
            column_ui.add_enabled_ui(size.is_some(), |ui| {
                let size_id = id.with("model_texture_size");
                kit::select::select_at(ui, skin, size_rect, size_id, &size_shown, sides);
            });
            new
        }
        None => {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("model_texture")
                    .selected_text(&shown)
                    .show_ui(ui, list);
                let new = pixel_ui::text_button(ui, "New").clicked();
                ui.add_enabled_ui(size.is_some(), |ui| {
                    egui::ComboBox::from_id_salt("model_texture_size")
                        .selected_text(&size_shown)
                        .show_ui(ui, sides);
                });
                new
            })
            .inner
        }
    };
    if new {
        editor.new_texture();
    } else if let Some(texture) = picked {
        editor.set_texture(texture);
    }
    side.and_then(|side| resize_texture(editor, side))
}

/// La fila de la cámara, como el boceto: el select de la proyección (`shown`, con las opciones de
/// `list`) y, al final de la fila, el grupo de los presets. Devuelve el preset apretado.
fn camera_row(
    ui: &mut Ui,
    skin: Option<&Skin>,
    shown: &str,
    list: impl FnOnce(&mut Ui),
) -> Option<CameraPreset> {
    let row_w = row_width(ui);
    match skin {
        Some(skin) => {
            let items = PRESETS.map(|(label, _)| kit::group::GroupItem::text(label));
            let row = row_rect(ui, row_w);
            let at = pos2(
                (row.right() - kit::group::width(ui, &items)).round(),
                row.top(),
            );
            let field = Rect::from_min_max(row.min, pos2(at.x - COLUMN_GAP, row.bottom()));
            let id = ui.id();
            kit::select::select_at(ui, skin, field, id.with("model_projection"), shown, list);
            let painter = ui.painter().clone();
            let presets = id.with("model_camera_presets");
            kit::group::button_group_at(ui, skin, &painter, presets, at, &items, &|_| false)
                .map(|i| PRESETS[i].1)
        }
        None => {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("model_projection")
                    .selected_text(shown)
                    .show_ui(ui, list);
                let mut picked = None;
                for (label, preset) in PRESETS {
                    if pixel_ui::text_button(ui, label).clicked() {
                        picked = Some(preset);
                    }
                }
                picked
            })
            .inner
        }
    }
}

/// El ancho de `text` como lo deja `ui.label`.
fn label_width(ui: &Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::WHITE);
    galley.size().x
}

/// La fila de la luz, como el boceto: la casilla Light y, hasta el final de la fila, Yaw (dado la
/// vuelta a ±180°) y Pitch en grados (lo guardado, en décimas) con sus steppers del mismo ancho,
/// deshabilitados sin luz. Solo se escribe el ángulo que cambió.
fn light_row(ui: &mut Ui, skin: Option<&Skin>, light: &mut cs_core::Light3D) {
    let (was_yaw, was_pitch) = (wrap_angle(light.yaw) / 10, light.pitch / 10);
    let (mut y, mut p) = (was_yaw, was_pitch);
    let right = ui.cursor().left() + row_width(ui);
    in_row(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        check(ui, skin, &mut light.enabled, "Light");
        ui.add_space(LIGHT_GAP);
        ui.add_enabled_ui(light.enabled, |ui| {
            let (yaw_w, pitch_w) = (label_width(ui, "Yaw"), label_width(ui, "Pitch"));
            let free = right - ui.cursor().left() - yaw_w - pitch_w - 2.0 * LABEL_GAP - PAIR_GAP;
            let range = -YAW_MAX..=YAW_MAX;
            angle_cell(ui, skin, (free / 2.0).floor(), ("Yaw", &mut y), range);
            ui.add_space(PAIR_GAP);
            // El de Pitch llega justo al final de la fila.
            let width = right - ui.cursor().left() - pitch_w - LABEL_GAP;
            let range = -PITCH_MAX..=PITCH_MAX;
            angle_cell(ui, skin, width, ("Pitch", &mut p), range);
        });
    });
    if y != was_yaw {
        light.yaw = y * 10;
    }
    if p != was_pitch {
        light.pitch = p * 10;
    }
}

/// La cámara, la luz y el contorno. Todo lo cambiado en un frame (o en un arrastre) es un paso de
/// deshacer (`set_model_grouped`).
fn settings(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let Some(model) = editor.edited_model().cloned() else {
        return;
    };
    let primary = editor.resolve(editor.primary);
    let mut next = model.clone();
    // Como el boceto (y los ejes del objeto): de una fila al título que sigue, solo el aire de
    // entre filas, y del título a su fila, 4 filas de la letra.
    label_with_hint(ui, "Camera", "");
    ui.add_space(-AXIS_TITLE_PULL);
    // Sin Yaw ni Pitch: la cámara la mueven la órbita, el gizmo de navegación y los presets.
    let cam = &mut next.camera;
    let ortho = cam.projection == Projection::Orthographic;
    let mut projection = None;
    let list = |ui: &mut Ui| {
        for (label, o) in [("Perspective", false), ("Orthographic", true)] {
            if ui.add(menus::choice(ortho == o, label)).clicked() {
                projection = Some(o);
                ui.close();
            }
        }
    };
    let shown = if ortho { "Orthographic" } else { "Perspective" };
    let preset = camera_row(ui, skin, shown, list);
    match projection {
        Some(true) if !ortho => cam.projection = Projection::Orthographic,
        Some(false) if ortho => {
            cam.projection = Projection::Perspective {
                fov: match Model3D::default().camera.projection {
                    Projection::Perspective { fov } => fov,
                    Projection::Orthographic => 600,
                },
            };
        }
        _ => {}
    }
    // Los presets miran al centro del modelo (con la distancia de antes).
    if let Some(preset) = preset {
        let target = crate::editor::model_center(&model).unwrap_or(cam.target);
        *cam = preset.camera(cam.distance, target);
    }

    label_with_hint(ui, "Light & Outline", "");
    ui.add_space(-AXIS_TITLE_PULL);
    light_row(ui, skin, &mut next.light);
    let mut outline = next.outline.is_some();
    in_row(ui, |ui| {
        check(ui, skin, &mut outline, "Outline");
        if let Some(color) = &mut next.outline {
            crate::widgets::color_edit(ui, color, false, "Outline color");
        }
    });
    if outline != next.outline.is_some() {
        next.outline = outline.then_some(primary);
    }

    let group = ui.id().with("model_settings");
    if next != model {
        editor.set_model_grouped(next, group);
    }
    if !ui.input(|input| input.pointer.any_down()) {
        editor.end_group_of(group.value());
    }
}

/// Shading… y Sheet settings…, al final de su fila (como el boceto). Devuelve lo pedido.
fn window_buttons(ui: &mut Ui, skin: Option<&Skin>) -> Option<ModelAction> {
    const BUTTONS: [(&str, ModelAction); 2] = [
        ("Shading…", ModelAction::Shading),
        ("Sheet settings…", ModelAction::Bake),
    ];
    let mut picked = None;
    match skin {
        Some(skin) => {
            let row = row_rect(ui, row_width(ui));
            let mut right = row.right();
            for (label, action) in BUTTONS.into_iter().rev() {
                let w = kit::button::width(ui, label);
                let rect =
                    Rect::from_min_max(pos2(right - w, row.top()), pos2(right, row.bottom()));
                let id = ui.id().with(("model_window_button", label));
                if kit::button::button_at(ui, skin, rect, id, label).clicked() {
                    picked = Some(action);
                }
                right = rect.left() - COLUMN_GAP;
            }
        }
        None => {
            ui.horizontal(|ui| {
                for (label, action) in BUTTONS {
                    if pixel_ui::text_button(ui, label).clicked() {
                        picked = Some(action);
                    }
                }
            });
        }
    }
    picked
}

/// La ventana Shading (la abre Shading… de lo general): la cuadrícula de [`shading`] y Auto. Sin
/// luz (o con un diálogo abierto, `modal`) se ve pero no se edita. Cada cambio es su propio paso
/// de undo. Devuelve si se cerró con la ×.
pub fn shading_window(
    ctx: &egui::Context,
    editor: &mut Editor,
    skin: Option<&Skin>,
    modal: bool,
) -> bool {
    let anchor = (egui::Align2::CENTER_CENTER, Vec2::ZERO);
    let (table, closed) =
        pixel_ui::app_window(ctx, skin, "model_shading", "Shading", true, anchor, |ui| {
            let lit = editor.edited_model().is_some_and(|m| m.light.enabled);
            ui.add_enabled_ui(!modal && lit, |ui| shading(ui, editor, skin, lit))
                .inner
        });
    if let Some(table) = table.flatten() {
        editor.set_shading(table);
    }
    closed
}

/// El color de egui de un `Rgba`.
fn color32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// El punto de una casilla fijada, de `PIN_DARK` sobre lo claro y blanco sobre lo oscuro.
fn pin_color(c: Rgba) -> Color32 {
    let luminance = 299 * c.r as u32 + 587 * c.g as u32 + 114 * c.b as u32;
    if luminance > 128 * 1000 {
        PIN_DARK
    } else {
        Color32::WHITE
    }
}

/// Una casilla de la cuadrícula en la columna `col` y la fila `row` desde `origin`: sus líneas
/// (compartidas con las vecinas) y lo de adentro.
fn shade_cell(origin: egui::Pos2, col: usize, row: usize) -> (Rect, Rect) {
    let at = origin + vec2(col as f32 * SHADE_PITCH, row as f32 * SHADE_PITCH);
    let lines = Rect::from_min_size(at, Vec2::splat(SHADE_PITCH + 1.0));
    (lines, lines.shrink(1.0))
}

/// Una columna de la cuadrícula Shading: un color opaco de la paleta (con su índice) o una
/// entrada huérfana de la tabla (con su número entre las huérfanas), la de un color que ya no
/// está entre los opacos de la paleta.
#[derive(Clone, Copy)]
enum ShadeColumn {
    Palette(usize, Rgba),
    Orphan(usize, Rgba),
}

/// La cuadrícula Shading y, debajo, Auto (vaciar la tabla): por cada color opaco de la paleta del
/// modelo una columna (en bloques de [`SHADE_COLUMNS`]) con el color y, debajo, sus pasos 1 y 2
/// de luz. Un clic en un paso abre las muestras opacas de la paleta (elegir una lo fija); el
/// derecho lo vuelve automático. Después, las entradas huérfanas (de colores que ya no están en la
/// paleta, que igual cuentan en el render), tachadas en diagonal: el clic derecho en un paso lo
/// quita y en el color quita la entrada. Sin luz (`lit` apagado), al lado de Auto lo dice.
/// Devuelve la tabla nueva si el usuario la cambió.
fn shading(ui: &mut Ui, editor: &Editor, skin: Option<&Skin>, lit: bool) -> Option<ShadingTable> {
    let table = editor.edited_model()?.shading.clone();
    // La paleta del modelo (en modo Texture el activo es su textura).
    let palette = editor
        .document
        .artboard(editor.model_id())?
        .sprite
        .palette
        .colors()
        .to_vec();
    let opaque: Vec<(usize, Rgba)> = palette
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, c)| c.a == 255)
        .collect();
    let columns: Vec<ShadeColumn> = opaque
        .iter()
        .map(|&(i, c)| ShadeColumn::Palette(i, c))
        .chain(
            table
                .entries
                .iter()
                .filter(|e| !opaque.iter().any(|&(_, c)| c == e.color))
                .enumerate()
                .map(|(n, e)| ShadeColumn::Orphan(n, e.color)),
        )
        .collect();
    let mut next = None;
    let enabled = ui.is_enabled();
    for block in columns.chunks(SHADE_COLUMNS) {
        let size = vec2(
            SHADE_PITCH * block.len() as f32 + 1.0,
            SHADE_PITCH * 3.0 + 1.0,
        );
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let origin = rect.min.round();
        let painter = ui.painter().clone();
        for (col, &column) in block.iter().enumerate() {
            let (color, orphan) = match column {
                ShadeColumn::Palette(_, c) => (c, None),
                ShadeColumn::Orphan(n, c) => (c, Some(n)),
            };
            for row in 0..3 {
                let (lines, cell) = shade_cell(origin, col, row);
                let shown = cs_3d::shade(&palette, &table, color, row as u32);
                painter.rect_filled(lines, 0.0, SHADE_GRID);
                painter.rect_filled(cell, 0.0, color32(shown));
                if row == 0 {
                    if let Some(n) = orphan {
                        orphan_mark(&painter, cell, pin_color(shown));
                        let id = ui.id().with(("shade_orphan", n));
                        let response = ui.interact(cell, id, Sense::click());
                        let label = format!("Shade orphan {n} color");
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label)
                        });
                        if response.secondary_clicked() {
                            let mut t = table.clone();
                            t.set(color, 1, None);
                            t.set(color, 2, None);
                            next = Some(t);
                        }
                    }
                    continue;
                }
                let step = row;
                let pinned = table.get(color, step).is_some();
                if pinned {
                    let dot = Rect::from_min_size(
                        pos2(cell.right() - 2.0, cell.top() + 1.0),
                        Vec2::splat(1.0),
                    );
                    painter.rect_filled(dot, 0.0, pin_color(shown));
                }
                let (id, label) = match column {
                    ShadeColumn::Palette(i, _) => (
                        ui.id().with(("shade", i, step)),
                        format!("Shade {i} step {step}"),
                    ),
                    ShadeColumn::Orphan(n, _) => (
                        ui.id().with(("shade_orphan", n, step)),
                        format!("Shade orphan {n} step {step}"),
                    ),
                };
                let response = ui.interact(cell, id, Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label)
                });
                if response.secondary_clicked() && pinned {
                    let mut t = table.clone();
                    t.set(color, step, None);
                    next = Some(t);
                }
                // Las huérfanas solo se quitan: su color ya no está en la paleta.
                if orphan.is_some() {
                    continue;
                }
                if let Some(pick) = swatches(skin, &response, cell, &opaque, shown) {
                    let mut t = table.clone();
                    t.set(color, step, Some(pick));
                    next = Some(t);
                }
            }
        }
    }
    ui.horizontal(|ui| {
        let auto = ui.add_enabled_ui(!table.entries.is_empty(), |ui| button(ui, skin, "Auto"));
        if auto.inner.clicked() {
            next = Some(ShadingTable::default());
        }
        if !lit {
            ui.label("Turn the light on to edit.");
        }
    });
    next.filter(|t| *t != table)
}

/// La marca de una columna huérfana: una diagonal de 1 px sobre su color, de abajo a la izquierda
/// a arriba a la derecha.
fn orphan_mark(painter: &egui::Painter, cell: Rect, color: Color32) {
    let n = cell.width().round() as i32;
    for k in 0..n {
        let at = pos2(cell.left() + k as f32, cell.bottom() - 1.0 - k as f32);
        painter.rect_filled(Rect::from_min_size(at, Vec2::splat(1.0)), 0.0, color);
    }
}

/// Las muestras de `colors` (los opacos de la paleta, con su índice) en un popup colgado de la
/// casilla `anchor` (lo abre y lo cierra un clic en `response`), con `current` marcado: el color
/// elegido, si hubo.
fn swatches(
    skin: Option<&Skin>,
    response: &egui::Response,
    anchor: Rect,
    colors: &[(usize, Rgba)],
    current: Rgba,
) -> Option<Rgba> {
    let content = |ui: &mut Ui| {
        let columns = colors.len().clamp(1, SHADE_COLUMNS);
        let rows = colors.len().div_ceil(SHADE_COLUMNS);
        let size = vec2(
            SHADE_PITCH * columns as f32 + 1.0,
            SHADE_PITCH * rows as f32 + 1.0,
        );
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let origin = rect.min.round();
        let painter = ui.painter().clone();
        let enabled = ui.is_enabled();
        let mut picked = None;
        for (slot, &(j, c)) in colors.iter().enumerate() {
            let (lines, cell) = shade_cell(origin, slot % SHADE_COLUMNS, slot / SHADE_COLUMNS);
            painter.rect_filled(lines, 0.0, SHADE_GRID);
            painter.rect_filled(cell, 0.0, color32(c));
            if c == current {
                let stroke = Stroke::new(1.0, pin_color(c));
                painter.rect_stroke(cell, 0.0, stroke, StrokeKind::Inside);
            }
            let swatch = ui.interact(cell, ui.id().with(("shade_swatch", j)), Sense::click());
            let label = format!("Shade color {j}");
            swatch.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, c == current, &label)
            });
            if swatch.clicked() {
                picked = Some(c);
                ui.close();
            }
        }
        picked
    };
    match skin {
        Some(skin) => {
            let look = menus::MenuLook::new(skin, anchor.top());
            menus::dropdown(&look, response, anchor, content).flatten()
        }
        None => egui::Popup::menu(response)
            .show(content)
            .and_then(|i| i.inner),
    }
}

#[cfg(test)]
mod tests;
