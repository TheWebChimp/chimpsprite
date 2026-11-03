//! El panel 3D del dock: el modelo 3D del artboard activo. Arriba, fija, la tira de pestañas de sus
//! objetos (una por objeto, con el ícono de su tipo: un clic la elige, su marca quita el objeto y,
//! si no caben, las flechas las recorren); abajo, con scroll, lo demás. Sin modelo, el botón New
//! 3D Model…; con modelo, arriba los modos de selección y la cuadrícula (en modo Texture, la
//! sección UV: girar y voltear las UVs de las caras elegidas y el tamaño de la textura), Add con
//! las primitivas (y los lados de las redondas) y, del elegido, su posición, su rotación y su
//! escala, Double sided y Delete; luego la textura, la cámara, la luz (con la cuadrícula Shading)
//! y el contorno.

use cs_3d::Primitive;
use cs_core::{CameraPreset, Model3D, Object3D, PrimitiveKind, Projection, Rgba, ShadingTable};
use eframe::egui::{
    self, Color32, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::dock::FLYOUT_MARGIN;
use crate::editor::{CadMode, Editor, wrap_angle};
use crate::font_panel::lit_line;
use crate::kit;
use crate::menus;
use crate::panel_kit::label_with_hint;
use crate::pixel_ui;
use crate::skin::Skin;
use crate::tabs::TabIcon;

/// Ancho de adentro del panel (sin su marco): el contenido y la barra del scroll.
pub const WIDTH: f32 = 300.0;
/// Las pestañas de los objetos, 5 filas debajo del borde de arriba de la caja del panel, sobre su
/// línea grabada (como en el boceto).
const TABS_TOP: f32 = 5.0;
/// Sin Pixel UI: la fila de pestañas de egui (más alta) y la línea debajo.
const PLAIN_TABS_TOP: f32 = 8.0;
const PLAIN_HEADER_LINE: f32 = 29.0;
/// Del borde de arriba del medio a lo primero, y lo mismo abajo.
const MIDDLE_AIR: f32 = 4.0;
/// Entre una cosa y otra del medio, y antes de cada sección.
const ITEM_GAP: f32 = 4.0;
const SECTION_GAP: f32 = 8.0;
/// El select Add y el stepper de los lados.
const ADD_W: f32 = 90.0;
const SIDES_W: f32 = 62.0;
/// El select de la cuadrícula del modo Edit.
const GRID_W: f32 = 56.0;
/// El select del tamaño de la textura (modo Texture) y sus lados.
const TEXTURE_SIZE_W: f32 = 64.0;
const TEXTURE_SIZES: [u32; 6] = [16, 32, 64, 128, 256, 512];
/// Los lados de cilindros, conos y esferas.
const MIN_SIDES: u32 = 3;
const MAX_SIDES: u32 = 32;
const DEFAULT_SIDES: u32 = 8;
/// Los números van en dos columnas: cada una, el eje y su stepper, con aire entre las dos.
const AXIS_LABEL_W: f32 = 14.0;
/// El ancho de una columna (la etiqueta y su stepper): las dos columnas, con `PAIR_GAP` y el aire
/// entre los widgets de la fila (tres veces), llenan lo de adentro sin la barra del scroll.
fn cell_w(ui: &Ui) -> f32 {
    let gaps = PAIR_GAP + 3.0 * ui.spacing().item_spacing.x;
    ((WIDTH - pixel_ui::SCROLL_BAR - gaps) / 2.0).floor()
}
/// La etiqueta de Yaw y Pitch.
const ANGLE_LABEL_W: f32 = 34.0;
/// El select de la textura.
const TEXTURE_W: f32 = 150.0;
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
}

impl Default for ModelPanelState {
    fn default() -> Self {
        ModelPanelState {
            sides: DEFAULT_SIDES,
        }
    }
}

/// Lo que el usuario pidió en el panel (lo resuelve la app).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelAction {
    NewModel,
    /// Bake…: abrir el diálogo de hornear el modelo.
    Bake,
    /// Pasar la textura a este lado, que deja UVs fuera: la app pide confirmación antes.
    TextureSize(u32),
}

/// El panel en `ui` (lo de adentro del marco, de la columna entera: ver
/// [`crate::dock::flyout_fixed`]): el título fijo y el medio con scroll.
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
    let line = header(ui, editor, skin, outer, inner);
    let middle = Rect::from_x_y_ranges(
        inner.x_range(),
        line + pixel_ui::ETCHED_SIZE..=inner.bottom(),
    );
    let mut area = egui::ScrollArea::vertical()
        .id_salt("model_panel_middle")
        .auto_shrink([false, false])
        .max_height(middle.height())
        .min_scrolled_height(middle.height());
    // Recién abierto: arriba. egui guarda el desplazamiento del scroll (y lo persiste entre
    // sesiones), así que sin esto el panel reabre donde se dejó, con Edit y los modos ocultos.
    // Si el frame anterior no se dibujó el panel, es que estaba cerrado.
    let shown_id = egui::Id::new("model_panel_last_pass");
    let pass = ui.ctx().cumulative_pass_nr();
    let last: Option<u64> = ui.data(|d| d.get_temp(shown_id));
    if last.is_none_or(|last| last + 1 < pass) {
        area = area.vertical_scroll_offset(0.0);
    }
    ui.data_mut(|d| d.insert_temp(shown_id, pass));
    let action = ui
        .scope_builder(UiBuilder::new().max_rect(middle), |ui| {
            pixel_ui::scroll(ui, area, |ui| {
                ui.add_space(MIDDLE_AIR);
                let action = body(ui, editor, skin, state);
                ui.add_space(MIDDLE_AIR);
                action
            })
        })
        .inner;
    // Todo el panel es de la columna (el medio no lo dice: está en su propio `Ui`).
    ui.advance_cursor_after_rect(inner);
    action
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
pub(crate) fn object_tabs(model: &Model3D) -> Vec<kit::tabs::TabItem> {
    model
        .objects
        .iter()
        .map(|o| {
            kit::tabs::TabItem::closable(o.name.clone(), true)
                .with_icon(TabIcon::Piece(kind_icon(o.shown_kind())))
        })
        .collect()
}

/// Arriba del panel, fija: la tira de pestañas de los objetos (la del kit, estilo documento, en
/// lo de adentro de la caja; su línea grabada, de lado a lado) o, sin Pixel UI, una fila de egui
/// y una línea. Un clic elige el objeto; su marca lo quita (un paso de undo). Sin modelo, solo la
/// línea. Devuelve la fila de arriba de la línea.
fn header(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, outer: Rect, inner: Rect) -> f32 {
    let items = editor.edited_model().map(object_tabs).unwrap_or_default();
    let active = editor.selected_object();
    let id = ui.id().with("model_object_tabs");
    let (line, event) = match skin {
        Some(skin) => {
            let mut strip =
                kit::tabs::Strip::new(inner.left(), inner.right(), outer.top() + TABS_TOP);
            strip.line = outer.x_range().shrink(1.0);
            let event = kit::tabs::tab_strip(ui, skin, id, strip, &items, active);
            (strip.bottom(), event)
        }
        None => {
            let line = outer.top() + PLAIN_HEADER_LINE;
            let row = Rect::from_x_y_ranges(inner.x_range(), outer.top() + PLAIN_TABS_TOP..=line);
            let event = ui
                .scope_builder(UiBuilder::new().max_rect(row), |ui| {
                    plain_tabs(ui, &items, active)
                })
                .inner;
            lit_line(ui, None, outer, line);
            (line, event)
        }
    };
    match event {
        Some(kit::tabs::TabEvent::Select(i)) => editor.select_model_object(Some(i)),
        Some(kit::tabs::TabEvent::Close(i)) => {
            editor.remove_object(i);
        }
        None => {}
    }
    line
}

/// Sin Pixel UI: una fila de egui (con scroll de lado) con el nombre de cada objeto (un clic lo
/// elige) y su × (lo quita).
fn plain_tabs(
    ui: &mut Ui,
    items: &[kit::tabs::TabItem],
    active: Option<usize>,
) -> Option<kit::tabs::TabEvent> {
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
                        event = Some(kit::tabs::TabEvent::Select(i));
                    }
                    let close = ui.small_button("×").on_hover_text("Remove");
                    let label = format!("Close {}", item.label);
                    close.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label)
                    });
                    if close.clicked() {
                        event = Some(kit::tabs::TabEvent::Close(i));
                    }
                    ui.add_space(4.0);
                }
            });
        });
    event
}

/// Lo que se desplaza: New 3D Model… sin modelo; con modelo, sus objetos.
fn body(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut ModelPanelState,
) -> Option<ModelAction> {
    if editor.edited_model().is_none() {
        let new = button(ui, skin, "New 3D Model…");
        ui.add_space(ITEM_GAP);
        ui.add(egui::Label::new("The active artboard is not a 3D model.").wrap());
        return new.clicked().then_some(ModelAction::NewModel);
    }
    ui.spacing_mut().item_spacing.y = ITEM_GAP;
    let action = if editor.cad_mode() == CadMode::Texture {
        uv_section(ui, editor, skin)
    } else {
        edit_rows(ui, editor, skin);
        None
    };
    add_row(ui, editor, skin, state);
    selected(ui, editor, skin);
    texture(ui, editor, skin);
    settings(ui, editor, skin);
    ui.add_space(SECTION_GAP);
    let bake = button(ui, skin, "Bake…")
        .clicked()
        .then_some(ModelAction::Bake);
    action.or(bake)
}

/// En modo Texture, en lugar de los modos de selección (ahí es siempre de caras) y de la
/// cuadrícula (de las vistas orto): la sección UV, con Rotate 90°, Flip H y Flip V para las caras
/// elegidas (sin caras, deshabilitados) y Texture size. Achicar la textura dejando UVs fuera lo
/// confirma la app (`ModelAction::TextureSize`).
fn uv_section(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) -> Option<ModelAction> {
    label_with_hint(ui, "UV", "");
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
    let size = editor
        .workspace_texture()
        .and_then(|id| editor.document.artboard(id))
        .map(|a| (a.sprite.width, a.sprite.height));
    let mut picked = None;
    if let Some((w, h)) = size {
        let shown = if w == h {
            w.to_string()
        } else {
            format!("{w}×{h}")
        };
        let mut list = |ui: &mut Ui| {
            for side in TEXTURE_SIZES {
                let on = (w, h) == (side, side);
                if ui.add(menus::choice(on, &side.to_string())).clicked() {
                    picked = Some(side);
                    ui.close();
                }
            }
        };
        ui.horizontal(|ui| {
            ui.label("Texture size");
            let width = Some(TEXTURE_SIZE_W);
            match skin {
                Some(skin) => {
                    kit::select::select(ui, skin, "model_texture_size", &shown, width, list);
                }
                None => {
                    egui::ComboBox::from_id_salt("model_texture_size")
                        .selected_text(&shown)
                        .show_ui(ui, |ui| list(ui));
                }
            }
        });
    }
    ui.add_space(SECTION_GAP);
    match picked {
        Some(side) if editor.texture_size_cuts_uvs(side) => Some(ModelAction::TextureSize(side)),
        Some(side) => {
            editor.set_texture_size(side);
            None
        }
        None => None,
    }
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

/// Los modos de selección y la cuadrícula del modo Edit (el del workspace del modelo).
fn edit_rows(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let current = editor.model_edit().selection.mode;
    let mut picked = None;
    ui.horizontal(|ui| {
        for mode in cs_3d::SelectMode::ALL {
            if radio(ui, skin, current == mode, mode.label()).clicked() {
                picked = Some(mode);
            }
        }
    });
    if let Some(mode) = picked {
        editor.set_select_mode(mode);
    }
    let grid = editor.model_edit().grid;
    let mut chosen = None;
    let mut list = |ui: &mut Ui| {
        for g in crate::editor::GRIDS {
            if ui.add(menus::choice(g == grid, grid_label(g))).clicked() {
                chosen = Some(g);
                ui.close();
            }
        }
    };
    ui.horizontal(|ui| {
        ui.label("Grid");
        match skin {
            Some(skin) => {
                kit::select::select(ui, skin, "model_grid", grid_label(grid), Some(GRID_W), list);
            }
            None => {
                egui::ComboBox::from_id_salt("model_grid")
                    .selected_text(grid_label(grid))
                    .show_ui(ui, |ui| list(ui));
            }
        }
    });
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

/// Add con las primitivas (elegir una la agrega) y los lados de las redondas (las esferas, con
/// la mitad de anillos).
fn add_row(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &mut ModelPanelState) {
    let sides = state.sides.clamp(MIN_SIDES, MAX_SIDES);
    let options = [
        Primitive::Cube,
        Primitive::Plane,
        Primitive::Cylinder { sides },
        Primitive::Cone { sides },
        Primitive::Sphere {
            sides,
            rings: sides / 2,
        },
    ];
    let mut picked = None;
    let mut list = |ui: &mut Ui| {
        for p in options {
            if ui.add(menus::choice(false, p.name())).clicked() {
                picked = Some(p);
                ui.close();
            }
        }
    };
    ui.horizontal(|ui| {
        match skin {
            Some(skin) => {
                kit::select::select(ui, skin, "model_add", "Add", Some(ADD_W), list);
            }
            None => {
                egui::ComboBox::from_id_salt("model_add")
                    .selected_text("Add")
                    .show_ui(ui, |ui| list(ui));
            }
        }
        ui.add_space(PAIR_GAP);
        ui.label("Sides");
        let range = MIN_SIDES..=MAX_SIDES;
        match skin {
            Some(skin) => {
                let value = &mut state.sides;
                kit::stepper::stepper(ui, skin, SIDES_W, "model_sides", value, range, 1, None);
            }
            None => {
                ui.add(egui::DragValue::new(&mut state.sides).range(range));
            }
        }
    });
    if let Some(p) = picked {
        editor.add_primitive(p);
    }
}

/// Un eje en su columna: la etiqueta de `AXIS_LABEL_W` y el stepper.
fn axis_cell(
    ui: &mut Ui,
    skin: Option<&Skin>,
    salt: &str,
    cell: (&str, &mut i32),
    range: std::ops::RangeInclusive<i32>,
) {
    cell_with(ui, skin, salt, AXIS_LABEL_W, cell, range);
}

/// Una columna: la etiqueta de `label_w` y el stepper, que llena lo que queda de [`cell_w`]. Un
/// valor fuera de `range` se muestra en el borde pero solo se escribe si el usuario lo cambia
/// (el stepper no lo recorta solo).
fn cell_with(
    ui: &mut Ui,
    skin: Option<&Skin>,
    salt: &str,
    label_w: f32,
    (label, value): (&str, &mut i32),
    range: std::ops::RangeInclusive<i32>,
) {
    ui.allocate_ui_with_layout(
        vec2(label_w, kit::stepper::HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(label_w);
            ui.label(label);
        },
    );
    let id = format!("{salt}_{label}");
    let shown = (*value).clamp(*range.start(), *range.end());
    let mut v = shown;
    match skin {
        Some(skin) => {
            let width = cell_w(ui) - label_w;
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

/// Un título con su unidad y los tres ejes en dos columnas: X e Y en una fila y Z debajo.
fn axes(
    ui: &mut Ui,
    skin: Option<&Skin>,
    (title, unit): (&str, &str),
    [x, y, z]: [&mut i32; 3],
    range: std::ops::RangeInclusive<i32>,
) {
    label_with_hint(ui, title, unit);
    ui.horizontal(|ui| {
        axis_cell(ui, skin, title, ("X", x), range.clone());
        ui.add_space(PAIR_GAP);
        axis_cell(ui, skin, title, ("Y", y), range.clone());
    });
    ui.horizontal(|ui| axis_cell(ui, skin, title, ("Z", z), range));
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

/// Lo del objeto elegido: posición, rotación y escala, Double sided y Delete. Todo lo cambiado
/// en un frame (o en un arrastre) es un paso de deshacer (`set_model_grouped`).
fn selected(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let Some(i) = editor.selected_object() else {
        return;
    };
    let Some(model) = editor.edited_model().cloned() else {
        return;
    };
    let obj: &Object3D = &model.objects[i];
    let mut next = obj.clone();
    ui.add_space(SECTION_GAP);
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
    ui.add_space(ITEM_GAP);
    let delete = button(ui, skin, "Delete");
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
    if delete.clicked() {
        editor.remove_object(i);
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

/// La textura: un select con los artboards del lienzo que no son modelos ni fuentes (y None; una
/// que ya no existe se muestra "(missing)") y New Texture, que agrega una plantilla y la asigna.
fn texture(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let Some(current) = editor.edited_model().map(|m| m.texture) else {
        return;
    };
    ui.add_space(SECTION_GAP);
    label_with_hint(ui, "Texture", "");
    let options: Vec<(cs_core::ArtboardId, String)> = editor
        .document
        .artboards
        .iter()
        .filter(|a| a.on_canvas())
        .map(|a| (a.id, a.name.clone()))
        .collect();
    // La textura puede no estar en la lista (en la biblioteca) o ya no existir.
    let shown = match current {
        None => "None".to_owned(),
        Some(id) => editor
            .document
            .artboard(id)
            .map_or_else(|| "(missing)".to_owned(), |a| a.name.clone()),
    };
    let mut picked = None;
    let mut list = |ui: &mut Ui| {
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
    let mut new = false;
    ui.horizontal(|ui| {
        match skin {
            Some(skin) => {
                kit::select::select(ui, skin, "model_texture", &shown, Some(TEXTURE_W), list);
            }
            None => {
                egui::ComboBox::from_id_salt("model_texture")
                    .selected_text(&shown)
                    .show_ui(ui, |ui| list(ui));
            }
        }
        new = button(ui, skin, "New Texture").clicked();
    });
    if new {
        editor.new_texture();
    } else if let Some(texture) = picked {
        editor.set_texture(texture);
    }
}

/// Yaw (dado la vuelta a ±180°) y Pitch en grados (lo guardado, en décimas): solo se escribe el
/// que cambió.
fn angles(ui: &mut Ui, skin: Option<&Skin>, salt: &str, (yaw, pitch): (&mut i32, &mut i32)) {
    let (was_yaw, was_pitch) = (wrap_angle(*yaw) / 10, *pitch / 10);
    let (mut y, mut p) = (was_yaw, was_pitch);
    ui.horizontal(|ui| {
        let range = -YAW_MAX..=YAW_MAX;
        cell_with(ui, skin, salt, ANGLE_LABEL_W, ("Yaw", &mut y), range);
        ui.add_space(PAIR_GAP);
        let range = -PITCH_MAX..=PITCH_MAX;
        cell_with(ui, skin, salt, ANGLE_LABEL_W, ("Pitch", &mut p), range);
    });
    if y != was_yaw {
        *yaw = y * 10;
    }
    if p != was_pitch {
        *pitch = p * 10;
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
    ui.add_space(SECTION_GAP);
    label_with_hint(ui, "Camera", "");
    let cam = &mut next.camera;
    let ortho = cam.projection == Projection::Orthographic;
    ui.horizontal(|ui| {
        if radio(ui, skin, !ortho, "Perspective").clicked() && ortho {
            cam.projection = Projection::Perspective {
                fov: match Model3D::default().camera.projection {
                    Projection::Perspective { fov } => fov,
                    Projection::Orthographic => 600,
                },
            };
        }
        if radio(ui, skin, ortho, "Orthographic").clicked() {
            cam.projection = Projection::Orthographic;
        }
    });
    ui.horizontal(|ui| {
        for (label, preset) in [
            ("Iso", CameraPreset::Iso),
            ("Top", CameraPreset::Top),
            ("Side", CameraPreset::Side),
            ("Front", CameraPreset::Front),
        ] {
            if button(ui, skin, label).clicked() {
                *cam = preset.camera(cam.distance, cam.target);
            }
        }
        if button(ui, skin, "Reset").clicked() {
            *cam = Model3D::default().camera;
        }
    });
    angles(ui, skin, "camera", (&mut cam.yaw, &mut cam.pitch));

    ui.add_space(SECTION_GAP);
    label_with_hint(ui, "Light & Outline", "");
    let light = &mut next.light;
    check(ui, skin, &mut light.enabled, "Light");
    ui.add_enabled_ui(light.enabled, |ui| {
        angles(ui, skin, "light", (&mut light.yaw, &mut light.pitch));
    });
    // Sin luz no hay sombras: la cuadrícula se ve pero no se edita.
    let shading = ui
        .add_enabled_ui(light.enabled, |ui| shading(ui, editor, skin))
        .inner;
    let mut outline = next.outline.is_some();
    ui.horizontal(|ui| {
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
    // La tabla, aparte: cada clic en la cuadrícula es su propio paso de undo.
    if let Some(table) = shading {
        editor.set_shading(table);
    }
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

/// "Shading" con Auto (vaciar la tabla) y la cuadrícula: por cada color opaco de la paleta del
/// modelo una columna (en bloques de [`SHADE_COLUMNS`]) con el color y, debajo, sus pasos 1 y 2
/// de luz. Un clic en un paso abre las muestras opacas de la paleta (elegir una lo fija); el
/// derecho lo vuelve automático. Después, las entradas huérfanas (de colores que ya no están en la
/// paleta, que igual cuentan en el render), tachadas en diagonal: el clic derecho en un paso lo
/// quita y en el color quita la entrada. Devuelve la tabla nueva si el usuario la cambió.
fn shading(ui: &mut Ui, editor: &Editor, skin: Option<&Skin>) -> Option<ShadingTable> {
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
    ui.horizontal(|ui| {
        label_with_hint(ui, "Shading", "");
        let auto = ui.add_enabled_ui(!table.entries.is_empty(), |ui| button(ui, skin, "Auto"));
        if auto.inner.clicked() {
            next = Some(ShadingTable::default());
        }
    });
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
