//! El panel 3D (en el dock del lienzo; en la pestaña del modelo, fijo en su lugar): el modelo 3D
//! del artboard activo, partido en dos a lo alto por una línea grabada (a ≈ 48 %). Arriba, con su
//! scroll, lo general del modelo: los modos de selección y la cuadrícula (usables en modo Model), la
//! textura y su tamaño, la cámara, la luz, el contorno, Shading… (la cuadrícula, en su ventana)
//! y Sheet settings…. Abajo, el encabezado "Outliner" con Duplicate, New group y el + a la
//! derecha (su ventanita, en [`add_menu`]: el tipo, los lados de los redondos y Add) y, debajo,
//! con su scroll, la fila de botones del objeto (en modo Model, Extrude, Subdivide, Merge y
//! Delete; en modo Texture, los de las UVs) y el [`outliner`] con los objetos y los grupos. La
//! posición, la rotación y la escala de la fila activa van en la barrita del objeto, debajo del
//! encabezado de ChimpCAD (`canvas::object_bar`). Sin modelo, el botón New 3D Model….

use cs_core::{CameraPreset, PrimitiveKind, Projection, Rgba, ShadingTable};
use eframe::egui::{
    self, Color32, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};

use crate::dock::FLYOUT_MARGIN;
use crate::editor::{CadMode, Editor, Row, wrap_angle};
use crate::font_panel::lit_line;
use crate::forms::{self, TuiBuilderLogic as _};
use crate::kit;
use crate::menus;
use crate::panel_kit::label_with_hint;
use crate::pixel_ui;
use crate::skin::Skin;

mod add_menu;
pub(crate) mod outliner;

pub use outliner::OutlinerState;

/// Ancho de adentro del panel (sin su marco) del diseño: el contenido y la barra del scroll. Con
/// él se abre (adentro todo sale del ancho disponible).
pub const REFERENCE_WIDTH: f32 = 227.0;
/// Lo más angosto en que se ve bien (medido): "Texture Size" todavía cabe en su columna (los 3/8
/// de la derecha de la fila) y el select de la textura muestra su nombre. Más angosto que el de
/// referencia se parten las filas: por debajo de 217, la de la cámara (los presets bajan), la de
/// las ventanas y la de los botones del objeto; por debajo de 211, Pitch baja debajo de Yaw.
pub const MIN_WIDTH: f32 = 200.0;
/// El encabezado del outliner, 5 filas debajo de la línea que parte el panel (3 de aire después
/// de sus dos filas), sobre su propia línea grabada a [`kit::tabs::HEIGHT`] (donde estaba la
/// tira de pestañas).
const TABS_TOP: f32 = 5.0;
/// Las mayúsculas de "Outliner", centradas con los botones del encabezado.
const HEADER_CAPS: f32 = 5.0;
/// Sin Pixel UI: la fila del encabezado de egui (más alta) y la línea debajo, desde la que parte
/// el panel.
const PLAIN_TABS_TOP: f32 = 8.0;
const PLAIN_HEADER_LINE: f32 = 29.0;
/// Dónde parte la línea el panel, desde arriba de su caja: como en el boceto, 197 de sus 408
/// filas (≈ 48 %: lo general cabe entero sin scroll).
const SPLIT: f32 = 197.0 / 408.0;
/// Del borde de arriba de cada mitad a lo primero, y lo mismo abajo.
const MIDDLE_AIR: f32 = 4.0;
/// De la línea del encabezado del outliner a la fila de botones del objeto:
/// 9 filas (como el boceto), con [`MIDDLE_AIR`]. La fila de los botones es una fila más alta
/// que ellos ([`BUTTONS_DROP`] arriba de los botones).
const OBJECT_TOP_AIR: f32 = 9.0 - MIDDLE_AIR - BUTTONS_DROP;
const BUTTONS_DROP: f32 = 1.0;
/// Entre una cosa y otra del medio.
const ITEM_GAP: f32 = 4.0;
/// Entre las líneas de una fila que no cabe en una.
const ROW_GAP: f32 = ITEM_GAP;
/// Como el boceto: la fila de los modos, 12 filas debajo de lo alto de la caja (2 más que el
/// aire de arriba), y de ella a la fila de la textura, 42.
const MODE_ROW_AIR: f32 = 2.0;
const TEXTURE_AIR: f32 = 3.0;
/// Y de la fila de la luz a la de Shading… y Sheet settings…, 37 (como entre las otras filas).
const BUTTONS_AIR: f32 = 3.0;
/// Los botones del encabezado (Duplicate, New group y +): de 13×13, con 4 px entre uno y otro, una
/// fila debajo de lo alto del encabezado.
const PLUS_W: f32 = 13.0;
const PLUS_GAP: f32 = 4.0;
const PLUS_TOP: f32 = 1.0;
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
/// El aire de las filas antes de la barra del scroll.
const ROW_END_GAP: f32 = 4.0;
/// El título se acerca a su fila: egui deja debajo de la letra su aire y el de [`ITEM_GAP`], y el
/// boceto pone 4 filas de la letra a la fila (y 10 de la fila al título que sigue).
const TITLE_PULL: f32 = 7.0;
/// Los títulos de la textura van en la forma de su fila (cada uno arriba de su select): de lo alto
/// de los títulos a la fila, lo mismo que deja un título de egui con [`TITLE_PULL`] (la letra, 2
/// filas abajo de lo alto, y la fila 4 filas debajo de la letra, como el boceto).
const TITLE_ROW: f32 = 16.0;
/// Lo más angosto de un stepper de la luz: los botones y un valor de tres cifras.
const STEPPER_MIN: f32 = 48.0;

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
    /// Lo que el outliner recuerda (renombrar, arrastrar).
    pub outliner: OutlinerState,
}

impl Default for ModelPanelState {
    fn default() -> Self {
        ModelPanelState {
            sides: DEFAULT_SIDES,
            add_kind: PrimitiveKind::Cube,
            open_add_menu: false,
            shading: false,
            outliner: OutlinerState::default(),
        }
    }
}

/// Lo que el usuario pidió en el panel (lo resuelve la app).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelAction {
    NewModel,
    /// Bake…: abrir el diálogo de hornear el modelo.
    Bake,
    /// GIF settings…: abrir el diálogo de exportar el modelo girando.
    Gif,
    /// Shading…: abrir la ventana de la tabla de sombras.
    Shading,
    /// Pasar la textura a este lado, que deja UVs fuera: la app pide confirmación antes.
    TextureSize(u32),
}

/// El panel en `ui` (lo de adentro del marco, de la columna entera: ver
/// [`crate::dock::flyout_fixed`] y [`crate::dock::fixed_panel`]): arriba lo general, la línea a
/// [`SPLIT`] del alto y abajo el encabezado del outliner y, con su scroll, lo del objeto elegido
/// y el outliner.
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
        // Las filas de las dos mitades, del ancho del panel sin la barra del scroll y su aire
        // (haya barra o no: así no saltan cuando aparece).
        let rows = (inner.width() - pixel_ui::SCROLL_BAR - ROW_END_GAP)
            .floor()
            .max(0.0);
        let action = scrolled(ui, "model_panel_general", top, reopened, |ui| {
            ui.set_max_width(rows);
            ui.spacing_mut().item_spacing.y = ITEM_GAP;
            general(ui, editor, skin)
        });
        divider(ui, skin, outer, split);
        let line = header(ui, editor, skin, state, outer, inner, split);
        let bottom = Rect::from_x_y_ranges(
            inner.x_range(),
            line + pixel_ui::ETCHED_SIZE..=inner.bottom(),
        );
        scrolled(ui, "model_panel_object", bottom, reopened, |ui| {
            ui.set_max_width(rows);
            ui.spacing_mut().item_spacing.y = ITEM_GAP;
            // La fila de botones del objeto y, debajo, el outliner.
            object_buttons(ui, editor, skin);
            ui.add_space(ITEM_GAP);
            outliner::show(ui, editor, skin, &mut state.outliner, rows);
        });
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
pub(crate) fn kind_icon(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Cube => "icon.object.cube",
        PrimitiveKind::Plane => "icon.object.plane",
        PrimitiveKind::Cylinder => "icon.object.cylinder",
        PrimitiveKind::Cone => "icon.object.cone",
        PrimitiveKind::Sphere => "icon.object.sphere",
    }
}

/// Arriba de la mitad de abajo (desde la línea que parte el panel, en la fila `split`), fijo: el
/// encabezado del outliner, "Outliner" a la izquierda y, a la derecha, Duplicate, New group y
/// el `+` (botones del kit de 13×13), sobre una línea grabada de lado a lado; sin Pixel UI, una
/// fila de egui y una línea. Duplicate copia lo elegido (con un grupo activo, el grupo con lo de
/// adentro); New group mete lo elegido en un grupo nuevo; el + agrega lo elegido en su menú (y lo
/// elige). Devuelve la fila de arriba de la línea.
fn header(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut ModelPanelState,
    outer: Rect,
    inner: Rect,
    split: f32,
) -> f32 {
    let group = matches!(editor.active_row(), Some(Row::Group(_)));
    let can_duplicate = editor.can_duplicate_active();
    let mut added = None;
    let mut duplicate = false;
    let mut new_group = false;
    let line = match skin {
        Some(skin) => {
            let top = split + TABS_TOP;
            let line = top + kit::tabs::HEIGHT;
            let id = ui.id();
            let ids = (
                id.with("model_new_group"),
                id.with("model_duplicate_object"),
            );
            let colors = skin.colors();
            let font = egui::TextStyle::Body.resolve(ui.style());
            let galley = ui
                .painter()
                .layout_no_wrap("Outliner".to_owned(), font, colors.text);
            let ppp = ui.ctx().pixels_per_point();
            // "Outliner" al principio de la fila y, al final, Duplicate, New group y el + (del
            // mismo tamaño).
            let row = Rect::from_x_y_ranges(inner.x_range(), top..=line);
            let mut header = ui.new_child(UiBuilder::new().max_rect(row));
            forms::form(&mut header, "model_outliner_header", 0.0, |tui| {
                tui.style(forms::row_nowrap(0.0)).add(|tui| {
                    let title = forms::fixed(galley.size().x.ceil(), kit::tabs::HEIGHT);
                    forms::canvas(tui, title, |ui| {
                        // Con las mayúsculas centradas con los botones.
                        let left_top = ui.max_rect().min;
                        let caps = left_top.y + HEADER_CAPS;
                        let at = crate::tab_bar::text_pos(&galley, left_top.x, caps, ppp);
                        ui.painter().galley(at, galley, colors.text);
                    });
                    let buttons = forms::group(PLUS_GAP);
                    let buttons = forms::push_right(forms::air_above(PLUS_TOP, buttons));
                    tui.style(buttons).add(|tui| {
                        let button = || forms::fixed(PLUS_W, PLUS_W);
                        duplicate = tui.style(button()).ui(|ui| {
                            let rect = ui.max_rect();
                            duplicate_button(ui, skin, rect, ids.1, can_duplicate, group)
                        });
                        new_group = tui.style(button()).ui(|ui| {
                            let (rect, icon) = (ui.max_rect(), "icon.outliner.new_group");
                            let response = kit::button::icon_button_at(
                                ui,
                                skin,
                                rect,
                                ids.0,
                                icon,
                                "New group",
                            );
                            pixel_ui::bubble_tooltip(response, skin, "New group").clicked()
                        });
                        added = tui.style(button()).ui(|ui| {
                            let rect = ui.max_rect();
                            add_menu::plus_button(ui, skin, rect, state)
                        });
                    });
                });
            });
            let x = outer.x_range().shrink(1.0);
            ui.painter()
                .add(pixel_ui::etched_hline(x, line, colors, ppp));
            line
        }
        None => {
            let line = split + PLAIN_HEADER_LINE;
            let row = Rect::from_x_y_ranges(inner.x_range(), split + PLAIN_TABS_TOP..=line);
            ui.scope_builder(UiBuilder::new().max_rect(row), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Outliner");
                    duplicate = ui
                        .add_enabled(can_duplicate, egui::Button::new("Duplicate"))
                        .clicked();
                    new_group = ui.button("New group").clicked();
                    added = add_menu::plain_menu(ui, state);
                });
            });
            lit_line(ui, None, outer, line);
            line
        }
    };
    if let Some(p) = added {
        editor.add_primitive(p);
    }
    if duplicate {
        editor.duplicate_active();
    }
    if new_group {
        editor.new_group();
    }
    line
}

/// Duplicate en `rect` (con el id `id`), junto a New group: un botón del kit con el ícono de
/// copiar (`icon.copy`) y su tooltip (con el atajo; "group" con un grupo activo); deshabilitado
/// sin nada que duplicar. Devuelve si se apretó.
fn duplicate_button(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    id: egui::Id,
    enabled: bool,
    group: bool,
) -> bool {
    let shortcut = crate::keymap::label(ui.ctx(), crate::keymap::Action::DuplicateObject);
    let what = if group {
        "Duplicate group"
    } else {
        "Duplicate object"
    };
    let tip = if shortcut.is_empty() {
        what.to_owned()
    } else {
        format!("{what} ({shortcut})")
    };
    ui.add_enabled_ui(enabled, |ui| {
        let response = kit::button::icon_button_at(ui, skin, rect, id, "icon.copy", "Duplicate");
        crate::pixel_ui::bubble_tooltip(response, skin, &tip).clicked()
    })
    .inner
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
/// (sin caras, deshabilitados) y Pack UVs, de todo el modelo (ver [`Editor::can_pack_uvs`]; con
/// una capa de la textura bloqueada, el tooltip dice por qué no), en una fila sin título (como
/// el boceto).
fn uv_buttons(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let faces = !editor.selected_faces().is_empty();
    let packable = editor.can_pack_uvs();
    let locked = editor.texture_layers_locked();
    let unlock = locked.then_some("Unlock the texture layers to pack");
    let buttons = [
        ("Rotate 90°", faces, None),
        ("Flip H", faces, None),
        ("Flip V", faces, None),
        ("Pack UVs", packable, unlock),
    ];
    match object_row(ui, skin, "model_uv_buttons", &buttons) {
        Some(0) => {
            editor.rotate_selected_uvs();
        }
        Some(i @ (1 | 2)) => {
            editor.flip_selected_uvs(i == 1);
        }
        Some(3) => {
            editor.pack_uvs();
        }
        _ => {}
    }
}

/// Una fila de botones del objeto (cada uno con su texto, si está habilitado y lo que dice su
/// tooltip deshabilitado), de izquierda a derecha con el aire de entre botones de egui; los que
/// no caben siguen en la línea de abajo. Devuelve el que se apretó.
fn object_row(
    ui: &mut Ui,
    skin: Option<&Skin>,
    id: &str,
    buttons: &[(&str, bool, Option<&str>)],
) -> Option<usize> {
    let gap = ui.spacing().item_spacing.x;
    let sizes: Vec<forms::Style> = buttons
        .iter()
        .map(|&(label, ..)| match skin {
            Some(_) => forms::fixed(kit::button::width(ui, label), kit::button::HEIGHT),
            None => forms::natural(),
        })
        .collect();
    let mut clicked = None;
    forms::form_in_flow(ui, id, 0.0, |tui| {
        let row = forms::air_above(BUTTONS_DROP, forms::row_lines(ROW_GAP));
        tui.style(row).add(|tui| {
            for (i, (&(label, on, hint), size)) in buttons.iter().zip(sizes).enumerate() {
                let pressed = tui.style(forms::air_right(gap, size)).ui(|ui| {
                    ui.add_enabled_ui(on, |ui| {
                        let b = button(ui, skin, label);
                        match hint {
                            Some(hint) => b.on_disabled_hover_text(hint),
                            None => b,
                        }
                        .clicked()
                    })
                    .inner
                });
                if pressed {
                    clicked = Some(i);
                }
            }
        });
    });
    clicked
}

/// Lo del objeto elegido, en modo Model: Extrude, Subdivide, Merge y Delete (cada uno
/// deshabilitado cuando no aplica), en una fila sin título como la de las UVs. Los tres primeros
/// van sobre lo elegido en las vistas que se puede editar; Delete también, salvo con un grupo
/// como fila activa en modo Object, que borra el grupo entero (ver
/// [`Editor::delete_from_panel`]). Hacen lo mismo que Ctrl+E, Ctrl+D, M y Supr en las vistas.
fn topology_buttons(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let enabled = [
        editor.can_extrude(),
        editor.can_subdivide(),
        editor.can_merge(),
        editor.can_delete_from_panel(),
    ];
    let labels = ["Extrude", "Subdivide", "Merge", "Delete"];
    let buttons: Vec<_> = labels
        .into_iter()
        .zip(enabled)
        .map(|(l, on)| (l, on, None))
        .collect();
    match object_row(ui, skin, "model_topology_buttons", &buttons) {
        Some(0) => editor.request_extrude(),
        Some(1) => {
            editor.subdivide_model_selection();
        }
        Some(2) => {
            editor.merge_model_selection();
        }
        Some(3) => {
            editor.delete_from_panel();
        }
        _ => {}
    }
}

/// Una fila de lo general (la forma `id`, de lado a lado de las filas), en el lugar de un widget
/// de [`kit::select::HEIGHT`] sin el aire de entre filas arriba (egui centra las filas en una más
/// baja): queda a 4 filas de la letra de su título y a 10 del título que sigue, como el boceto.
/// Si lo de adentro no cabe en una línea, es más alta.
fn row_form(ui: &mut Ui, id: &str, add: impl FnOnce(&mut forms::Tui)) {
    ui.add_space(-ui.spacing().item_spacing.y);
    forms::form_in_flow(ui, id, 0.0, add);
}

/// Una etiqueta del alto de su hoja (la de `ui`), con la letra centrada a lo alto como la deja
/// `ui.label` en una fila de egui.
fn row_label(ui: &mut Ui, text: &str) {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, Sense::hover());
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, enabled, text));
    let color = ui.visuals().text_color();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, color);
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
            let id = ui.id();
            let label_w = label_width(ui, "Grid");
            let tile = forms::fixed(kit::tile::SIZE.x, kit::tile::SIZE.y);
            let tile = forms::air_right(MODE_PITCH - kit::tile::SIZE.x, tile);
            let tile = forms::air_above(MODE_TOP, tile);
            let h = kit::select::HEIGHT;
            ui.add_enabled_ui(enabled, |ui| {
                row_form(ui, "model_mode_row", |tui| {
                    tui.style(forms::row_lines(ROW_GAP)).add(|tui| {
                        for (k, mode) in cs_3d::SelectMode::ALL.into_iter().enumerate() {
                            let clicked = tui.style(tile.clone()).ui(|ui| {
                                let at = ui.max_rect().min;
                                let tile_id = id.with(("model_select_mode", k));
                                let (icon, on) = (mode_icon(mode), current == mode);
                                let label = mode.label();
                                kit::tile::tile_toggle_at(ui, skin, at, tile_id, icon, label, on)
                                    .clicked()
                            });
                            if clicked {
                                picked = Some(mode);
                            }
                        }
                        // "Grid" y su select, al final de la fila.
                        let field = forms::push_right(forms::group(LABEL_GAP));
                        tui.style(field).add(|tui| {
                            tui.style(forms::fixed(label_w, h))
                                .ui(|ui| row_label(ui, "Grid"));
                            tui.style(forms::fixed(GRID_W, h)).ui(|ui| {
                                let rect = ui.max_rect();
                                let shown = grid_label(grid);
                                kit::select::select_at(
                                    ui,
                                    skin,
                                    rect,
                                    id.with("model_grid"),
                                    shown,
                                    list,
                                );
                            });
                        });
                    });
                });
            });
            // Y el aire de entre filas otra vez: de la fila de los modos a la de la textura, lo
            // del boceto.
            ui.add_space(ui.spacing().item_spacing.y);
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

/// Un ángulo de la luz en `tui` (una hoja que crece con su fila, con `air` a la derecha): la
/// etiqueta (de `label_w`) y, a [`LABEL_GAP`], el stepper, que se lleva lo que sobre;
/// deshabilitados si no `enabled`. Un valor fuera de `range` se muestra en el borde pero solo se escribe si el usuario
/// lo cambia (el stepper no lo recorta solo).
fn angle_cell(
    tui: &mut forms::Tui,
    skin: &Skin,
    (label, label_w, air): (&str, f32, f32),
    enabled: bool,
    value: &mut i32,
    range: std::ops::RangeInclusive<i32>,
) {
    let h = kit::stepper::HEIGHT;
    let cell = forms::grow_auto(label_w + LABEL_GAP + STEPPER_MIN, label_w + LABEL_GAP);
    let cell = forms::air_right(air, cell);
    tui.style(cell).add(|tui| {
        tui.style(forms::air_right(LABEL_GAP, forms::fixed(label_w, h)))
            .ui(|ui| ui.add_enabled_ui(enabled, |ui| row_label(ui, label)));
        tui.style(forms::grow(STEPPER_MIN, 0.0, None, h)).ui(|ui| {
            let width = ui.max_rect().width();
            ui.add_enabled_ui(enabled, |ui| {
                let id = format!("light_{label}");
                edit_angle(value, range.clone(), |v| {
                    kit::stepper::stepper(ui, skin, width, &id, v, range, 1, None);
                });
            });
        });
    });
}

/// Un ángulo de `value` en `range` que se edita con `edit`: un valor fuera se muestra en el borde
/// pero solo se escribe si el usuario lo cambia.
fn edit_angle(value: &mut i32, range: std::ops::RangeInclusive<i32>, edit: impl FnOnce(&mut i32)) {
    let shown = (*value).clamp(*range.start(), *range.end());
    let mut v = shown;
    edit(&mut v);
    if v != shown {
        *value = v;
    }
}

/// La fila de botones del objeto, arriba del outliner: en modo Model los de topología y en modo
/// Texture los de las UVs (cada uno deshabilitado cuando no aplica).
fn object_buttons(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    ui.add_space(OBJECT_TOP_AIR);
    match editor.cad_mode() {
        CadMode::Texture => uv_buttons(ui, editor, skin),
        CadMode::Model => topology_buttons(ui, editor, skin),
        // Los bloques se agregan desde la barra del timeline.
        CadMode::Anim => {}
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
            let id = ui.id();
            // La columna de la derecha, a 5/8 de la fila.
            let column = (ui.available_width().floor() * RIGHT_COLUMN).round();
            let new_w = kit::button::width(ui, "New");
            let left = |h| forms::fixed(column - COLUMN_GAP, h);
            let right = |h| forms::grow(0.0, 0.0, None, h);
            let h = kit::select::HEIGHT;
            let mut new = false;
            forms::form_in_flow(ui, "model_texture_row", 0.0, |tui| {
                // Cada título arriba de su select.
                tui.style(forms::row_nowrap(COLUMN_GAP)).add(|tui| {
                    forms::canvas(tui, left(TITLE_ROW), |ui| {
                        label_with_hint(ui, "Texture", "")
                    });
                    let title = |ui: &mut Ui| label_with_hint(ui, "Texture Size", "");
                    forms::canvas(tui, right(TITLE_ROW), title);
                });
                tui.style(forms::row_nowrap(COLUMN_GAP)).add(|tui| {
                    new = forms::canvas(tui, left(h), |ui| {
                        // New, pegado al select: comparten la columna del borde. Va primero, así
                        // el borde del select queda encima (como en el boceto).
                        let row = ui.max_rect();
                        let new_rect =
                            Rect::from_min_max(pos2(row.right() - new_w, row.top()), row.max);
                        let new_id = id.with("model_texture_new");
                        let new = kit::button::button_at(ui, skin, new_rect, new_id, "New");
                        let field =
                            Rect::from_min_max(row.min, pos2(new_rect.left() + 1.0, row.bottom()));
                        kit::select::select_at(
                            ui,
                            skin,
                            field,
                            id.with("model_texture"),
                            &shown,
                            list,
                        );
                        new.clicked()
                    });
                    forms::canvas(tui, right(h), |ui| {
                        let rect = ui.max_rect();
                        // Deshabilitado sin textura.
                        ui.add_enabled_ui(size.is_some(), |ui| {
                            let size_id = id.with("model_texture_size");
                            kit::select::select_at(ui, skin, rect, size_id, &size_shown, sides);
                        });
                    });
                });
            });
            new
        }
        None => {
            label_with_hint(ui, "Texture", "");
            ui.add_space(-TITLE_PULL);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("model_texture")
                    .selected_text(&shown)
                    .show_ui(ui, list);
                let new = pixel_ui::text_button(ui, "New").clicked();
                ui.label("Texture Size");
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
    match skin {
        Some(skin) => {
            let items = PRESETS.map(|(label, _)| kit::group::GroupItem::text(label));
            let presets_w = kit::group::width(ui, &items);
            // El select, de lo que dice para arriba.
            let field_min = kit::select::width(ui, shown);
            let id = ui.id();
            let h = kit::select::HEIGHT;
            let mut picked = None;
            row_form(ui, "model_camera_row", |tui| {
                tui.style(forms::row_end(COLUMN_GAP)).add(|tui| {
                    tui.style(forms::grow(field_min, 0.0, None, h)).ui(|ui| {
                        let rect = ui.max_rect();
                        let field = id.with("model_projection");
                        kit::select::select_at(ui, skin, rect, field, shown, list);
                    });
                    picked = tui.style(forms::fixed(presets_w, h)).ui(|ui| {
                        let painter = ui.painter().clone();
                        let (presets, at) = (id.with("model_camera_presets"), ui.max_rect().min);
                        kit::group::button_group_at(
                            ui,
                            skin,
                            &painter,
                            presets,
                            at,
                            &items,
                            &|_| false,
                        )
                    });
                });
            });
            picked.map(|i| PRESETS[i].1)
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
    let (yaw, pitch) = (-YAW_MAX..=YAW_MAX, -PITCH_MAX..=PITCH_MAX);
    match skin {
        Some(skin) => {
            let mark = kit::check::Mark::Check(kit::check::Check::of(light.enabled));
            let light_w = kit::check::width(ui, skin, mark, "Light");
            let (yaw_w, pitch_w) = (label_width(ui, "Yaw"), label_width(ui, "Pitch"));
            // Light y los dos ángulos, con steppers del mismo ancho (±1) hasta el final de la
            // fila; si no caben, Pitch baja.
            row_form(ui, "model_light_row", |tui| {
                tui.style(forms::row_lines(ROW_GAP)).add(|tui| {
                    let light_cell = forms::fixed(light_w, kit::check::HEIGHT);
                    tui.style(forms::air_right(LIGHT_GAP, light_cell))
                        .ui(|ui| check(ui, Some(skin), &mut light.enabled, "Light"));
                    let on = light.enabled;
                    angle_cell(tui, skin, ("Yaw", yaw_w, PAIR_GAP), on, &mut y, yaw);
                    angle_cell(tui, skin, ("Pitch", pitch_w, 0.0), on, &mut p, pitch);
                });
            });
        }
        None => {
            ui.horizontal(|ui| {
                check(ui, None, &mut light.enabled, "Light");
                ui.add_enabled_ui(light.enabled, |ui| {
                    for (label, value, range) in [("Yaw", &mut y, yaw), ("Pitch", &mut p, pitch)] {
                        ui.label(label);
                        edit_angle(value, range.clone(), |v| {
                            ui.add(egui::DragValue::new(v).range(range));
                        });
                    }
                });
            });
        }
    }
    if y != was_yaw {
        light.yaw = y * 10;
    }
    if p != was_pitch {
        light.pitch = p * 10;
    }
}

/// Los FOV (en décimas de grado) que ofrece el select de la proyección; el de la vista Persp se
/// afina con Alt + rueda.
const FOV_PRESETS: [i32; 4] = [300, 450, 600, 900];

/// Lo que dice el select de la proyección: "Perspective 60°" u "Orthographic".
pub(crate) fn projection_label(p: Projection) -> String {
    match p {
        Projection::Perspective { fov } => format!("Perspective {}°", fov / 10),
        Projection::Orthographic => "Orthographic".to_owned(),
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
    // Como el boceto: de una fila al título que sigue, solo el aire de
    // entre filas, y del título a su fila, 4 filas de la letra.
    label_with_hint(ui, "Camera", "");
    ui.add_space(-TITLE_PULL);
    // Sin Yaw ni Pitch: la cámara la mueven la órbita, el gizmo de navegación y los presets.
    let cam = &mut next.camera;
    let mut projection = None;
    let current = cam.projection;
    let list = |ui: &mut Ui| {
        let choices = FOV_PRESETS
            .map(|fov| Projection::Perspective { fov })
            .into_iter()
            .chain([Projection::Orthographic]);
        for p in choices {
            if ui
                .add(menus::choice(current == p, &projection_label(p)))
                .clicked()
            {
                projection = Some(p);
                ui.close();
            }
        }
    };
    let shown = projection_label(current);
    let preset = camera_row(ui, skin, &shown, list);
    if let Some(p) = projection {
        cam.projection = p;
    }
    // Los presets miran al centro del modelo (con la distancia de antes).
    if let Some(preset) = preset {
        let target = crate::editor::model_center(&model).unwrap_or(cam.target);
        *cam = preset.camera(cam.distance, target);
    }

    label_with_hint(ui, "Light & Outline", "");
    ui.add_space(-TITLE_PULL);
    light_row(ui, skin, &mut next.light);
    let mut outline = next.outline.is_some();
    let lit = next.light.enabled;
    let dither = &mut next.light.dither;
    match skin {
        Some(skin) => outline_row(ui, skin, (&mut outline, &mut next.outline), dither, lit),
        None => {
            ui.horizontal(|ui| {
                check(ui, None, &mut outline, "Outline");
                if let Some(color) = &mut next.outline {
                    crate::widgets::color_edit(ui, color, false, "Outline color");
                }
                ui.add_space(LIGHT_GAP);
                ui.add_enabled_ui(lit, |ui| check(ui, None, dither, "Dither"));
            });
        }
    }
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

/// La fila del contorno, con Pixel UI: la casilla Outline (`on`) con su color (si hay contorno,
/// `color`) y, a [`LIGHT_GAP`] más del aire de entre botones, Dither, deshabilitada sin luz
/// (`lit`: sin luz no hay sombras que tramar). Si no caben, lo que no cabe baja.
fn outline_row(
    ui: &mut Ui,
    skin: &Skin,
    (on, color): (&mut bool, &mut Option<Rgba>),
    dither: &mut bool,
    lit: bool,
) {
    let gap = ui.spacing().item_spacing.x;
    let width = |ui: &Ui, label: &str| {
        let mark = kit::check::Mark::Check(kit::check::Check::of(false));
        kit::check::width(ui, skin, mark, label)
    };
    let (outline_w, dither_w) = (width(ui, "Outline"), width(ui, "Dither"));
    let swatch = kit::color_button::size(skin);
    let h = kit::check::HEIGHT;
    row_form(ui, "model_outline_row", |tui| {
        tui.style(forms::row_lines(ROW_GAP)).add(|tui| {
            let air = if color.is_some() {
                gap
            } else {
                gap + LIGHT_GAP
            };
            tui.style(forms::air_right(air, forms::fixed(outline_w, h)))
                .ui(|ui| check(ui, Some(skin), on, "Outline"));
            if let Some(color) = color {
                let cell = forms::fixed(swatch.x, swatch.y);
                tui.style(forms::air_right(gap + LIGHT_GAP, cell)).ui(|ui| {
                    crate::widgets::color_edit(ui, color, false, "Outline color");
                });
            }
            tui.style(forms::fixed(dither_w, h)).ui(|ui| {
                ui.add_enabled_ui(lit, |ui| check(ui, Some(skin), dither, "Dither"));
            });
        });
    });
}

/// Shading…, Sheet settings… y GIF settings…, al final de su fila (como el boceto, que tenía
/// los dos primeros). Devuelve lo pedido.
fn window_buttons(ui: &mut Ui, skin: Option<&Skin>) -> Option<ModelAction> {
    const BUTTONS: [(&str, ModelAction); 3] = [
        ("Shading…", ModelAction::Shading),
        ("Sheet settings…", ModelAction::Bake),
        ("GIF settings…", ModelAction::Gif),
    ];
    let mut picked = None;
    match skin {
        Some(skin) => {
            let id = ui.id();
            let widths = BUTTONS.map(|(label, _)| kit::button::width(ui, label));
            row_form(ui, "model_window_buttons", |tui| {
                tui.style(forms::row_end(COLUMN_GAP)).add(|tui| {
                    for ((label, action), w) in BUTTONS.into_iter().zip(widths) {
                        let cell = forms::fixed(w, kit::button::HEIGHT);
                        let clicked = tui.style(cell).ui(|ui| {
                            let (rect, id) =
                                (ui.max_rect(), id.with(("model_window_button", label)));
                            kit::button::button_at(ui, skin, rect, id, label).clicked()
                        });
                        if clicked {
                            picked = Some(action);
                        }
                    }
                });
            });
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
mod tests {
    use super::*;
    use crate::forms::testing::{Probe, run_at};

    /// Un modelo en su workspace, como en los tests de ChimpCAD: "Ship" (con el cubo elegido) y un
    /// artboard "Skin" para la textura.
    fn editing() -> Editor {
        let rgba = |w, h| cs_core::Sprite::new(w, h, cs_core::ColorMode::Rgba);
        let mut e = Editor::with_sprite(rgba(8, 8));
        assert!(e.add_artboard_with_sprite("Skin".into(), rgba(16, 16)));
        assert!(e.new_model("Ship", (32, 32)));
        e
    }

    /// El panel de `width` con el modelo de `setup`, y el ancho justo del select de la cámara
    /// (con lo que dice).
    fn panel_at(width: f32, setup: fn(&mut Editor)) -> (Probe, f32) {
        let mut editor = editing();
        setup(&mut editor);
        let mut state = ModelPanelState::default();
        let mut select = 0.0;
        let probe = run_at(width, |ui| {
            let skin = pixel_ui::frame_skin(ui.ctx());
            show(ui, &mut editor, skin.as_deref(), &mut state);
            let projection = editor.edited_model().map(|m| m.camera.projection);
            select = kit::select::width(ui, &projection_label(projection.unwrap()));
        });
        (probe, select)
    }

    /// El de más a la izquierda y el de más a la derecha de los controles `name` (los dos
    /// steppers de la luz tienen un − y un +).
    fn ends(probe: &Probe, name: &str) -> (Rect, Rect) {
        let mut all: Vec<Rect> = probe
            .widgets
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .collect();
        assert!(!all.is_empty(), "no {name:?}");
        all.sort_by(|a, b| {
            (a.top(), a.left())
                .partial_cmp(&(b.top(), b.left()))
                .unwrap()
        });
        (all[0], all[all.len() - 1])
    }

    /// Lo que se tapa a propósito, fuera de la cuenta: la fila del outliner es un botón de lado a
    /// lado con los suyos (ojo, candado) adentro, y el select de la textura y los presets de la
    /// cámara comparten la columna del borde con el que sigue.
    fn without_glued(mut probe: Probe) -> Probe {
        probe.widgets.retain(|(n, _)| n != "Cube");
        for (n, r) in &mut probe.widgets {
            if ["Ship Texture", "Iso", "Top", "Side"].contains(&n.as_str()) {
                r.max.x -= 1.0;
            }
        }
        probe
    }

    type Setup = (&'static str, fn(&mut Editor));

    /// De `MIN_WIDTH` para arriba (y en el ancho de referencia, como el boceto), en modo Model con
    /// el cubo elegido, en modo Texture, sin objeto y con la luz apagada y el contorno: todo entra
    /// en el ancho del panel, en puntos enteros y sin que nada tape a nada. Las filas llegan hasta
    /// antes de la barra del scroll y se parten si no caben: los presets de la cámara bajan
    /// debajo de su select (que entonces llena la fila), los botones de las ventanas y los del
    /// objeto siguen en la fila de abajo y Pitch baja debajo de Yaw; más ancho, crecen el select de
    /// la cámara, los steppers de la luz, los selects de la textura y las filas del outliner.
    #[test]
    fn the_model_panel_reflows_at_any_width() {
        let states: [Setup; 4] = [
            ("object", |_| {}),
            ("texture", |e| e.set_cad_mode(CadMode::Texture)),
            ("no object", |e| e.select_model_object(None)),
            ("light off", |e| {
                let off = cs_core::Light3D {
                    enabled: false,
                    ..Default::default()
                };
                assert!(e.set_light(off));
                assert!(e.set_outline(Some(cs_core::Rgba::rgb(0, 0, 0))));
            }),
        ];
        let gap = COLUMN_GAP;
        // Si se partió alguna vez la fila de la cámara, la de las ventanas, la de la luz y la del
        // objeto.
        let mut wrapped = [false; 4];
        let mut grown = [0.0; 3];
        for width in [MIN_WIDTH, 215.0, REFERENCE_WIDTH, 600.0] {
            for (state, setup) in states {
                let (probe, select) = panel_at(width, setup);
                let at = format!("{state} at {width}");
                probe.assert_whole_px();
                probe.assert_within(width);
                // Hasta antes de la barra del scroll y su aire.
                let end = width - pixel_ui::SCROLL_BAR - ROW_END_GAP;
                // El encabezado del outliner llega hasta el borde (está arriba del scroll).
                assert_eq!(probe.rect("Add object").right(), width, "{at}");
                // Los modos al principio y la cuadrícula al final.
                assert_eq!(probe.rect("Object").left(), 0.0, "{at}");
                assert_eq!(probe.rect("1/4").right(), end, "{at}");
                // La textura: el tamaño en la columna de la derecha (a 5/8 de la fila) y New
                // pegado al select.
                let size = probe.rect("64");
                assert_eq!(size.left(), (end * RIGHT_COLUMN).round(), "{at}");
                assert_eq!(size.right(), end, "{at}");
                let new = probe.rect("New");
                assert_eq!(new.right(), size.left() - gap, "{at}");
                let texture = probe.rect("Ship Texture");
                assert_eq!((texture.left(), texture.right()), (0.0, new.left() + 1.0));
                // La cámara: el select y, a su lado o debajo, los presets al final de la fila.
                let projection = probe.rect("Perspective 60°");
                let (iso, front) = (probe.rect("Iso"), probe.rect("Front"));
                assert_eq!(front.right(), end, "{at}");
                if select + gap + front.right() - iso.left() <= end {
                    assert_eq!(iso.top(), projection.top(), "beside {at}");
                    assert_eq!(projection.right(), iso.left() - gap, "{at}");
                } else {
                    wrapped[0] = true;
                    assert!(iso.top() > projection.bottom(), "below {at}");
                    assert_eq!(projection.right(), end, "{at}");
                }
                // Shading…, Sheet settings… y GIF settings…, al final de su fila (o de dos).
                let [shading, sheet, gif] =
                    ["Shading…", "Sheet settings…", "GIF settings…"].map(|l| probe.rect(l));
                assert_eq!(gif.right(), end, "{at}");
                if shading.width() + sheet.width() + gif.width() + 2.0 * gap <= end {
                    assert_eq!([shading.top(), sheet.top()], [gif.top(); 2], "{at}");
                    assert_eq!(sheet.right(), gif.left() - gap, "{at}");
                } else {
                    wrapped[1] = true;
                    assert!(gif.top() > shading.bottom(), "{at}");
                    assert_eq!(sheet.right(), end, "{at}");
                }
                // La luz: los steppers (casi) del mismo ancho y el de Pitch hasta el final de su
                // fila; si no caben al lado de Light, Pitch baja.
                let (yaw, pitch_minus) = ends(&probe, "Decrease");
                let (yaw_plus, pitch) = ends(&probe, "Increase");
                assert_eq!(pitch.right(), end, "{at}");
                let widths = [
                    yaw_plus.right() - yaw.left(),
                    pitch.right() - pitch_minus.left(),
                ];
                if pitch.top() > yaw.top() {
                    wrapped[2] = true;
                    assert!(pitch.top() >= yaw.bottom(), "{at}");
                } else {
                    assert!((widths[0] - widths[1]).abs() <= 1.0, "{widths:?} {at}");
                }
                // La fila del outliner, de lado a lado de las filas.
                let cube = probe.rect("Cube");
                assert_eq!((cube.left(), cube.right()), (0.0, end), "{at}");
                // Los botones del objeto, desde el principio de la fila.
                let buttons = match state {
                    "texture" => ["Rotate 90°", "Flip H", "Flip V", "Pack UVs"],
                    _ => ["Extrude", "Subdivide", "Merge", "Delete"],
                };
                let rects = buttons.map(|l| probe.rect(l));
                assert_eq!(rects[0].left(), 0.0, "{at}");
                if rects[3].top() > rects[0].top() {
                    wrapped[3] = true;
                    assert!(rects[3].top() >= rects[0].bottom(), "{at}");
                } else {
                    assert!(rects[3].right() <= end, "{at}");
                }
                if state == "object" {
                    if width == REFERENCE_WIDTH {
                        grown = [projection.width(), widths[1], size.width()];
                    } else if width > REFERENCE_WIDTH {
                        assert!(projection.width() > grown[0], "{at}");
                        assert!(widths[1] > grown[1], "{at}");
                        assert!(size.width() > grown[2], "{at}");
                    }
                }
                without_glued(probe).assert_no_overlap();
            }
        }
        assert_eq!(wrapped, [true; 4], "camera, windows, light, object");
    }

    /// Sin modelo, New 3D Model… y lo que dice, del ancho que haya.
    #[test]
    fn without_a_model_the_panel_fits_any_width() {
        for width in [MIN_WIDTH, REFERENCE_WIDTH, 600.0] {
            let rgba = cs_core::Sprite::new(8, 8, cs_core::ColorMode::Rgba);
            let mut editor = Editor::with_sprite(rgba);
            let mut state = ModelPanelState::default();
            let probe = run_at(width, |ui| {
                let skin = pixel_ui::frame_skin(ui.ctx());
                show(ui, &mut editor, skin.as_deref(), &mut state);
            });
            probe.assert_whole_px();
            probe.assert_within(width);
            probe.assert_no_overlap();
            assert_eq!(probe.rect("New 3D Model…").left(), 0.0);
        }
    }
}
