//! El panel 3D del dock: el modelo 3D del artboard activo. Arriba, fijo, el título; abajo, con
//! scroll, lo demás. Sin modelo, el botón New 3D Model…; con modelo, sus objetos: la lista (un
//! clic elige), Add con las primitivas (y los lados de las redondas) y, del elegido, su posición,
//! su rotación y su escala, Double sided y Delete.

use cs_3d::Primitive;
use cs_core::{CameraPreset, Model3D, Object3D, Projection};
use eframe::egui::{self, Align2, Rect, Sense, TextStyle, Ui, UiBuilder, pos2, vec2};

use crate::dock::FLYOUT_MARGIN;
use crate::editor::{Editor, wrap_angle};
use crate::font_panel::lit_line;
use crate::kit;
use crate::menus;
use crate::panel_kit::label_with_hint;
use crate::pixel_ui;
use crate::skin::Skin;

/// Ancho de adentro del panel (sin su marco): el contenido y la barra del scroll.
pub const WIDTH: f32 = 300.0;
/// La línea grabada bajo el título (como en FontForge).
const HEADER_LINE: f32 = 29.0;
/// Del borde de arriba del medio a lo primero, y lo mismo abajo.
const MIDDLE_AIR: f32 = 4.0;
/// Entre una cosa y otra del medio, y antes de cada sección.
const ITEM_GAP: f32 = 4.0;
const SECTION_GAP: f32 = 8.0;
/// Una fila de la lista de objetos y el aire del texto a su izquierda.
const ROW_HEIGHT: f32 = 16.0;
const ROW_PAD: f32 = 4.0;
/// El select Add y el stepper de los lados.
const ADD_W: f32 = 90.0;
const SIDES_W: f32 = 62.0;
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
    header(ui, skin, outer);
    let middle = Rect::from_x_y_ranges(
        inner.x_range(),
        outer.top() + HEADER_LINE + pixel_ui::ETCHED_SIZE..=inner.bottom(),
    );
    let area = egui::ScrollArea::vertical()
        .id_salt("model_panel_middle")
        .auto_shrink([false, false])
        .max_height(middle.height())
        .min_scrolled_height(middle.height());
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

/// El título "3D" arriba a la izquierda y la línea grabada debajo.
fn header(ui: &mut Ui, skin: Option<&Skin>, outer: Rect) {
    let at = outer.min + vec2(FLYOUT_MARGIN as f32, FLYOUT_MARGIN as f32);
    ui.painter().text(
        at,
        Align2::LEFT_TOP,
        "3D",
        TextStyle::Heading.resolve(ui.style()),
        ui.visuals().strong_text_color(),
    );
    lit_line(ui, skin, outer, outer.top() + HEADER_LINE);
}

/// Lo que se desplaza: New 3D Model… sin modelo; con modelo, sus objetos.
fn body(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut ModelPanelState,
) -> Option<ModelAction> {
    let Some(count) = editor.model().map(|m| m.objects.len()) else {
        let new = button(ui, skin, "New 3D Model…");
        ui.add_space(ITEM_GAP);
        ui.add(egui::Label::new("The active artboard is not a 3D model.").wrap());
        return new.clicked().then_some(ModelAction::NewModel);
    };
    ui.spacing_mut().item_spacing.y = ITEM_GAP;
    let note = match count {
        1 => "1 object".to_owned(),
        n => format!("{n} objects"),
    };
    label_with_hint(ui, "Objects", &note);
    objects(ui, editor, skin);
    add_row(ui, editor, skin, state);
    selected(ui, editor, skin);
    texture(ui, editor, skin);
    settings(ui, editor, skin);
    ui.add_space(SECTION_GAP);
    button(ui, skin, "Bake…")
        .clicked()
        .then_some(ModelAction::Bake)
}

/// Un botón: el del kit (sin Pixel UI, el de texto).
fn button(ui: &mut Ui, skin: Option<&Skin>, label: &str) -> egui::Response {
    match skin {
        Some(skin) => kit::button::button(ui, skin, label),
        None => pixel_ui::text_button(ui, label),
    }
}

/// La lista de los objetos: una fila por objeto con su nombre; la elegida, resaltada. Un clic
/// elige.
fn objects(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let names: Vec<String> = editor
        .model()
        .map(|m| m.objects.iter().map(|o| o.name.clone()).collect())
        .unwrap_or_default();
    let chosen = editor.selected_object();
    let (text, selection) = match skin {
        Some(skin) => {
            let c = skin.colors();
            (c.text, c.selection)
        }
        None => {
            let v = ui.visuals();
            (v.text_color(), v.selection.bg_fill)
        }
    };
    let enabled = ui.is_enabled();
    let font = TextStyle::Body.resolve(ui.style());
    let mut pick = None;
    // Las filas, pegadas.
    let spacing = ui.spacing().item_spacing.y;
    ui.spacing_mut().item_spacing.y = 0.0;
    for (i, name) in names.iter().enumerate() {
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
        let rect = Rect::from_min_size(rect.min.round(), rect.size());
        let on = chosen == Some(i);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, name)
        });
        if on {
            ui.painter().rect_filled(rect, 0.0, selection);
        } else if response.hovered() {
            let hover = ui.visuals().widgets.hovered.weak_bg_fill;
            ui.painter().rect_filled(rect, 0.0, hover);
        }
        let galley = ui
            .painter()
            .layout_no_wrap(name.clone(), font.clone(), text);
        let at = pos2(
            rect.left() + ROW_PAD,
            rect.center().y - galley.size().y / 2.0,
        );
        ui.painter_at(rect).galley(at.round(), galley, text);
        if response.clicked() {
            pick = Some(i);
        }
    }
    ui.spacing_mut().item_spacing.y = spacing;
    if let Some(i) = pick {
        editor.select_model_object(Some(i));
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
    let Some(model) = editor.model().cloned() else {
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

/// La textura: un select con los artboards que no son modelos ni fuentes (y None) y New Texture,
/// que agrega una plantilla y la asigna.
fn texture(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let Some(current) = editor.model().map(|m| m.texture) else {
        return;
    };
    ui.add_space(SECTION_GAP);
    label_with_hint(ui, "Texture", "");
    let options: Vec<(cs_core::ArtboardId, String)> = editor
        .document
        .artboards
        .iter()
        .filter(|a| a.model.is_none() && a.font.is_none())
        .map(|a| (a.id, a.name.clone()))
        .collect();
    let shown = options
        .iter()
        .find(|(id, _)| Some(*id) == current)
        .map_or("None", |(_, name)| name.as_str())
        .to_owned();
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
    let Some(model) = editor.model().cloned() else {
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
}
