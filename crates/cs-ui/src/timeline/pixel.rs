//! El timeline con Pixel UI, como en el diseño (`Design.aseprite`, frame 4, capa Mockup: la caja
//! va de (65,556) a (972,762)). Las medidas son en píxeles del skin; con la escala entera de Pixel
//! UI, un punto es un píxel del skin.
//!
//! - **Caja:** `workspace.box`, la misma del área de trabajo, con 5 de fondo a la derecha y abajo.
//! - **Barra:** filas de 22 (3 de aire, los grupos de 16, su brillo y 2 de aire), con los grupos
//!   a 3 del interior de la caja. Entre secciones, 3 de aire, la línea grabada y 3 de aire. Si no
//!   entra a lo ancho, las secciones pasan a la fila siguiente (`pixel_ui::wrap_rows`).
//!   - Grupos de botones: `timeline.group` alrededor de celdas `timeline.button` de 23×14
//!     separadas por `timeline.button.sep`, con el ícono `timeline.icon.*` de 21×11 adentro.
//!   - Onion skin: el botón (`timeline.icon.onion` apagado; prendido, la cebolla entera
//!     `timeline.icon.onion.on`) y, a 4, `timeline.onion_range`: frames antes ◂ | después ▸.
//!   - "Opacity" − [0..255] +, el combo del modo de mezcla y el reloj − [ms] + de la duración:
//!     `timeline.stepper.minus`, `.field` (se estira) y `.plus`; `timeline.combo`;
//!     `timeline.icon.clock`.
//! - **Línea de abajo de la barra:** `timeline.divider`, con sus uniones a la caja.
//! - **Tabla:** filas de 13 (11 de contenido y la línea grabada) y columnas con su línea a la
//!   derecha: ojo 17, candado 17, nombre 83 y un frame 13. Ojo `timeline.eye` (visible) o
//!   `timeline.eye.closed` (oculta); candado `palette.icon.lock` (bloqueada) o
//!   `palette.icon.lock.open`; cel `timeline.cel` o `timeline.cel.empty`.
//! - **Lo elegido** (frame 6 del diseño): la celda en `timeline_selection` con sus líneas grabadas
//!   en `timeline_selection_dark` y `timeline_selection_light` (las de arriba y la de la izquierda
//!   también, aunque sean de la celda vecina), y sus íconos en la versión `.selected`.

use eframe::egui::{
    self, Color32, Mesh, Pos2, Rect, Response, Sense, Shape, Ui, UiBuilder, Vec2, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use cs_core::LayerKind;

use super::{BLEND_MODES, Grid, TimelineState, blend_name};
use crate::bar_fields::{self, NumberLook};
use crate::editor::Editor;
use crate::menus::{self, MenuLook};
use crate::pixel_ui;
use crate::pixel_ui::Tip as _;
use crate::skin::{Skin, WidgetState};
use crate::tab_bar;

// ---- Tabla ----

/// Alto de una fila de la tabla: 11 de contenido y la línea grabada (oscura y brillo).
pub(super) const ROW_H: f32 = 13.0;
/// Ancho de la columna de un frame: 11 de contenido y su línea.
pub(super) const CELL_W: f32 = 13.0;
/// Columnas de la izquierda, cada una con su línea: ojo (15), candado (15) y nombre (81).
pub(super) const EYE_W: f32 = 17.0;
pub(super) const LOCK_W: f32 = 17.0;
pub(super) const NAME_W: f32 = 83.0;
/// De la línea del candado a la tinta del nombre.
pub(super) const NAME_PAD: f32 = 4.0;
/// La línea grabada de cada celda: abajo y a la derecha.
const LINE: f32 = 2.0;
/// Las mayúsculas del texto de una fila, desde arriba de la fila.
const TEXT_CAPS: f32 = 1.0;
/// Dónde van los íconos en su celda.
const EYE_AT: Vec2 = vec2(3.0, 2.0);
const LOCK_AT: Vec2 = vec2(4.0, 1.0);
const CEL_AT: Vec2 = vec2(2.0, 2.0);
/// La carpetita de los grupos (`timeline.folder`, `timeline.folder.open`): el ancho de la abierta
/// (la más ancha) y 3 de aire hasta el nombre.
pub(super) const FOLDER_W: f32 = 16.0;
const FOLDER: &str = "timeline.folder";
const FOLDER_OPEN: &str = "timeline.folder.open";
/// La flechita de un grupo (▾ abierto, ▸ cerrado): 2 después de la carpetita abierta, centrada
/// con ella, y 3 de aire hasta el nombre.
const FOLD_OPEN: &str = "timeline.fold.open";
const FOLD_CLOSED: &str = "timeline.fold.closed";
const FOLD_LEFT: f32 = 15.0;
pub(super) const FOLD_ARROW_W: f32 = 7.0;
const TILEMAP: &str = "timeline.tilemap";
const CLIP: &str = "timeline.clip";
const CLIP_TOP: f32 = 2.0;
/// Las carpetitas terminan 2 filas arriba del final del contenido de la fila (como en el diseño).
const FOLDER_BOTTOM: f32 = 10.0;
/// La versión de un ícono para lo elegido (sobre el rosa).
const SELECTED: &str = ".selected";

const EYE: &str = "timeline.eye";
const EYE_CLOSED: &str = "timeline.eye.closed";
const LOCK: &str = "palette.icon.lock";
const LOCK_OPEN: &str = "palette.icon.lock.open";
const CEL: &str = "timeline.cel";
const CEL_EMPTY: &str = "timeline.cel.empty";

// ---- Barra ----

/// Una fila de la barra: 3 de aire, el grupo de 16, su brillo y 2 de aire.
const BAR_ROW_H: f32 = 22.0;
/// Del interior de la caja al primer grupo (a la izquierda y arriba de cada fila).
const BAR_PAD: f32 = 3.0;
/// Entre dos secciones: 3 de aire, la línea grabada (2) y 3 de aire.
const SECTION_GAP: f32 = 8.0;
/// Entre dos cosas de la misma sección (el botón de onion y su rango; Opacity y el combo).
const ITEM_GAP: f32 = 4.0;
/// De la tinta de "Opacity" a su − y del reloj a su −.
const LABEL_GAP: f32 = 6.0;
const CLOCK_GAP: f32 = 2.0;
/// Alto de un grupo de botones o de un campo (sin el brillo de abajo).
const GROUP_H: f32 = 16.0;
/// Una celda de botón y el paso de una a la otra (con la línea entre las dos).
const BUTTON: Vec2 = vec2(23.0, 14.0);
const BUTTON_PITCH: f32 = 24.0;
/// Las mayúsculas de "Opacity" y del combo, y las cifras de los campos (una fila más arriba),
/// desde arriba del grupo.
const LABEL_CAPS: f32 = 5.0;
const FIELD_CAPS: f32 = 4.0;
/// − y + de los campos (de borde a borde) y el campo entre los dos.
const STEP_W: f32 = 15.0;
const OPACITY_FIELD_W: f32 = 28.0;
const DURATION_FIELD_W: f32 = 33.0;
/// Aire a cada lado del texto de la duración si no entra en el ancho del diseño.
const FIELD_TEXT_PAD: f32 = 3.0;
/// De la tinta del número de la duración a la de "ms": 2, una columna más que entre letras.
const UNIT_GAP: f32 = 2.0;
const UNIT: &str = "ms";
/// El combo: su ancho en el diseño, de su borde a la tinta del texto, de la tinta a la flecha y
/// la parte de la flecha.
const COMBO_W: f32 = 59.0;
const COMBO_TEXT_LEFT: f32 = 6.0;
const COMBO_TEXT_GAP: f32 = 5.0;
const COMBO_ARROW_W: f32 = 10.0;
/// El rango del onion skin: 59 de ancho. La mitad de la izquierda (frames antes) tiene la ◂ y la
/// línea del medio está en la columna 29; la de la derecha (después) tiene la ▸.
const RANGE_W: f32 = 59.0;
const RANGE_BEFORE: (f32, f32) = (8.0, 29.0);
const RANGE_AFTER: (f32, f32) = (30.0, 51.0);
const RANGE_MID: f32 = 29.0;
/// Hasta cuántos frames antes y después muestra el onion skin.
const ONION_MAX: usize = 8;
/// El reloj de la duración.
const CLOCK_W: f32 = 16.0;
/// La línea de abajo de la barra: empieza 1 fila antes del final de la barra y 1 columna antes
/// del interior de la caja (se une a sus lados).
const DIVIDER_H: f32 = 3.0;

const GROUP: &str = "timeline.group";
const BUTTON_PIECE: &str = "timeline.button";
const SEP: &str = "timeline.button.sep";
const ONION_ON: &str = "timeline.icon.onion.on";
const RANGE: &str = "timeline.onion_range";
const MINUS: &str = "timeline.stepper.minus";
const PLUS: &str = "timeline.stepper.plus";
const FIELD: &str = "timeline.stepper.field";
const COMBO: &str = "timeline.combo";
const CLOCK: &str = "timeline.icon.clock";
const DIVIDER: &str = "timeline.divider";

/// El marco del panel con Pixel UI: el fondo del skin, sin margen arriba (ya está el de abajo del
/// área de trabajo) ni a la izquierda (el de la barra de íconos).
pub fn panel_frame(skin: &Skin) -> egui::Frame {
    let margin = pixel_ui::WORKSPACE_MARGIN;
    egui::Frame::new()
        .fill(skin.colors().background)
        .inner_margin(egui::Margin {
            left: 0,
            // Termina junto al dock, como la caja del lienzo de arriba.
            right: crate::dock::LEFT_AIR,
            top: 0,
            bottom: margin,
        })
}

/// La caja, la barra, la línea de abajo y la tabla.
pub(super) fn show(ui: &mut Ui, editor: &mut Editor, state: &mut TimelineState, skin: &Skin) {
    let outer = ui.max_rect();
    // Todo el panel es de la caja: si solo ocupara su interior, el panel (que se ajusta a lo que
    // ocupa) se achicaría el borde de la caja en cada frame.
    ui.expand_to_include_rect(outer);
    let inner = pixel_ui::workspace_box(ui, skin, outer);
    ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
        ui.set_clip_rect(inner.intersect(ui.clip_rect()));
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let bar = toolbar(ui, editor, state, skin);
        divider(ui, skin, inner, bar.bottom());
        // La tabla empieza debajo de la línea; su scroll, una fila antes (las líneas de las
        // columnas del encabezado suben hasta el brillo de la línea).
        ui.add_space(DIVIDER_H - 1.0 - 1.0);
        super::grid(ui, editor, state, Grid::pixel(skin));
    });
}

/// Lo dibujado de la pieza `name` en `rect` (estirada si tiene centro). Si el skin no la trae,
/// nada, y devuelve `false`.
fn piece(ui: &Ui, skin: &Skin, name: &str, rect: Rect) -> bool {
    let Some(piece) = skin.piece(name, WidgetState::Normal) else {
        return false;
    };
    let ppp = ui.ctx().pixels_per_point();
    ui.painter()
        .extend(pixel_ui::piece_shapes(piece, rect, ppp));
    true
}

/// La pieza `name` a su tamaño, con la esquina en `at`. Devuelve si estaba.
fn piece_at(ui: &Ui, skin: &Skin, name: &str, at: Pos2) -> bool {
    let size = piece_size(skin, name).unwrap_or(Vec2::ZERO);
    piece(ui, skin, name, Rect::from_min_size(at, size))
}

/// Un ícono: con `selected`, su versión `.selected` (si el skin la tiene).
fn icon_at(ui: &Ui, skin: &Skin, name: &str, at: Pos2, selected: bool) -> bool {
    let chosen = format!("{name}{SELECTED}");
    if selected && skin.piece(&chosen, WidgetState::Normal).is_some() {
        return piece_at(ui, skin, &chosen, at);
    }
    piece_at(ui, skin, name, at)
}

fn piece_size(skin: &Skin, name: &str) -> Option<Vec2> {
    skin.piece(name, WidgetState::Normal)
        .map(|p| vec2(p.size[0] as f32, p.size[1] as f32))
}

/// Respaldo sin la pieza: un cuadro plano con el borde del skin.
fn flat(ui: &Ui, skin: &Skin, rect: Rect) {
    let ppp = ui.ctx().pixels_per_point();
    ui.painter()
        .extend(pixel_ui::flat_frame(skin.colors(), rect, 0.0, ppp));
}

/// Un rect liso, alineado a píxeles físicos.
fn solid(ui: &Ui, rect: Rect, color: Color32) {
    let ppp = ui.ctx().pixels_per_point();
    ui.painter().add(menus::solid(rect, color, ppp));
}

/// `text` con la tinta desde `left` y las mayúsculas en la fila `caps`.
fn text(ui: &Ui, painter: &egui::Painter, text: &str, left: f32, caps: f32, color: Color32) {
    let ppp = ui.ctx().pixels_per_point();
    let shown = menus::galley(ui, text, color);
    let at = tab_bar::text_pos(&shown, left, caps, ppp);
    painter.galley(at, shown, color);
}

fn ink(ui: &Ui, text: &str) -> f32 {
    menus::ink_width(&menus::galley(ui, text, Color32::PLACEHOLDER))
}

// ---- Barra ----

/// Lo que va en la barra, en orden. Cada una es una pieza que no se parte al pasar de fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Playback,
    Frames,
    Layers,
    Onion,
    OnionRange,
    Opacity,
    Blend,
    Duration,
}

const UNITS: [Unit; 8] = [
    Unit::Playback,
    Unit::Frames,
    Unit::Layers,
    Unit::Onion,
    Unit::OnionRange,
    Unit::Opacity,
    Unit::Blend,
    Unit::Duration,
];

/// Lo que hay entre `a` y la siguiente: la misma sección (aire) u otra (línea grabada).
fn join(a: Unit) -> (f32, bool) {
    match a {
        Unit::Onion | Unit::Opacity => (ITEM_GAP, false),
        _ => (SECTION_GAP, true),
    }
}

/// Ancho de un grupo de `n` botones: los bordes, las celdas y las líneas entre celdas.
fn group_width(n: usize) -> f32 {
    BUTTON_PITCH * n as f32 + 1.0
}

/// Lo que mide cada campo y el combo en este frame (depende de lo que muestran).
struct Widths {
    opacity_label: f32,
    duration_field: f32,
    combo: f32,
}

impl Widths {
    fn new(ui: &Ui, editor: &Editor, state: &TimelineState) -> Self {
        let ms = state
            .duration_edit
            .unwrap_or(editor.sprite().frames[editor.frame].duration_ms);
        let duration_text = duration_ink(ui, ms) + 2.0 * FIELD_TEXT_PAD;
        let blend = editor
            .sprite()
            .layer(editor.layer)
            .map_or(String::new(), |l| blend_name(l.blend));
        let combo = COMBO_TEXT_LEFT + ink(ui, &blend) + COMBO_TEXT_GAP + COMBO_ARROW_W;
        Self {
            opacity_label: ink(ui, "Opacity"),
            duration_field: DURATION_FIELD_W.max(duration_text.ceil()),
            combo: COMBO_W.max(combo.ceil()),
        }
    }

    fn of(&self, unit: Unit) -> f32 {
        match unit {
            Unit::Playback => group_width(5),
            Unit::Frames => group_width(2),
            Unit::Layers => group_width(4),
            Unit::Onion => group_width(1),
            Unit::OnionRange => RANGE_W,
            Unit::Opacity => self.opacity_label + LABEL_GAP + 2.0 * STEP_W + OPACITY_FIELD_W,
            Unit::Blend => self.combo,
            Unit::Duration => CLOCK_W + CLOCK_GAP + 2.0 * STEP_W + self.duration_field,
        }
    }
}

/// La barra: las unidades en filas que entran a lo ancho. Devuelve su rect.
fn toolbar(ui: &mut Ui, editor: &mut Editor, state: &mut TimelineState, skin: &Skin) -> Rect {
    let widths = Widths::new(ui, editor, state);
    let sizes: Vec<f32> = UNITS.iter().map(|&u| widths.of(u)).collect();
    let joins: Vec<f32> = UNITS[..UNITS.len() - 1]
        .iter()
        .map(|&u| join(u).0)
        .collect();
    let available = (ui.available_width() - 2.0 * BAR_PAD).max(0.0);
    let rows = pixel_ui::wrap_rows(available, &sizes, &joins);
    let height = BAR_ROW_H * rows.len().max(1) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    for (r, range) in rows.into_iter().enumerate() {
        let top = rect.top() + BAR_ROW_H * r as f32;
        let mut x = rect.left() + BAR_PAD;
        for i in range.clone() {
            if i > range.start {
                let (gap, etched) = join(UNITS[i - 1]);
                if etched {
                    let line = x + (gap - pixel_ui::ETCHED_SIZE) / 2.0;
                    let y = egui::Rangef::new(top, top + BAR_ROW_H);
                    ui.painter()
                        .add(pixel_ui::etched_vline(line, y, &colors, ppp));
                }
                x += gap;
            }
            let origin = pos2(x, top + BAR_PAD);
            unit(ui, editor, state, skin, UNITS[i], origin, &widths);
            x += sizes[i];
        }
    }
    rect
}

fn unit(
    ui: &mut Ui,
    editor: &mut Editor,
    state: &mut TimelineState,
    skin: &Skin,
    unit: Unit,
    origin: Pos2,
    widths: &Widths,
) {
    match unit {
        Unit::Playback => playback(ui, editor, skin, origin),
        Unit::Frames => {
            let can_remove = editor.frame_count() > 1;
            let buttons = [
                Btn::new(
                    "timeline.icon.frame_add",
                    "New frame",
                    "New frame: duplicates this one (Alt+N)",
                ),
                Btn::new("timeline.icon.frame_remove", "Delete frame", "Delete frame")
                    .enabled(can_remove),
            ];
            let r = group(ui, skin, origin, &buttons);
            if r[0].clicked() {
                editor.duplicate_frame();
            }
            if r[1].clicked() {
                editor.remove_frame();
            }
        }
        Unit::Layers => {
            let can_remove = editor
                .sprite()
                .subtree_range(editor.layer)
                .is_some_and(|r| r.len() < editor.sprite().layers.len());
            let buttons = [
                Btn::new(
                    "timeline.icon.layer_add",
                    "New layer",
                    "New layer (Shift+N)",
                ),
                Btn::new("timeline.icon.layer_remove", "Delete layer", "Delete layer")
                    .enabled(can_remove),
                Btn::new("timeline.icon.group_add", "New group", "New group"),
                Btn::new(
                    "timeline.icon.tilemap_add",
                    "New tilemap layer",
                    "New tilemap layer",
                ),
            ];
            let r = group(ui, skin, origin, &buttons);
            if r[0].clicked() {
                editor.add_layer(LayerKind::Image);
            }
            if r[1].clicked() {
                editor.remove_layer();
            }
            if r[2].clicked() {
                editor.add_layer(LayerKind::Group);
            }
            if r[3].clicked() {
                state.new_tilemap = true;
            }
        }
        Unit::Onion => {
            let on = editor.onion_enabled;
            let button = Btn::new("timeline.icon.onion", "Onion skin", "Onion skin (F3)").on(on);
            if group(ui, skin, origin, &[button])[0].clicked() {
                editor.onion_enabled = !on;
            }
        }
        Unit::OnionRange => onion_range(ui, editor, skin, origin),
        Unit::Opacity => opacity(ui, editor, state, skin, origin, widths),
        Unit::Blend => blend(ui, editor, skin, origin, widths.combo),
        Unit::Duration => duration(ui, editor, state, skin, origin, widths.duration_field),
    }
}

fn playback(ui: &mut Ui, editor: &mut Editor, skin: &Skin, origin: Pos2) {
    let last = editor.frame_count() - 1;
    let play = if editor.playing { "Pause" } else { "Play" };
    let mut play_button = Btn::new("timeline.icon.play", play, "Play / pause (Enter)");
    if editor.playing {
        // El diseño no tiene pausa: dos barras con la tinta de los íconos.
        play_button.icon = PAUSE;
    }
    let buttons = [
        Btn::new("timeline.icon.first", "First frame", "First frame"),
        Btn::new("timeline.icon.prev", "Previous frame", "Previous frame (,)"),
        play_button,
        Btn::new("timeline.icon.next", "Next frame", "Next frame (.)"),
        Btn::new("timeline.icon.last", "Last frame", "Last frame"),
    ];
    let r = group(ui, skin, origin, &buttons);
    if r[0].clicked() {
        editor.set_frame(0);
    }
    if r[1].clicked() {
        editor.prev_frame();
    }
    if r[2].clicked() {
        editor.toggle_play();
    }
    if r[3].clicked() {
        editor.next_frame();
    }
    if r[4].clicked() {
        editor.set_frame(last);
    }
}

/// El ícono de pausa, sin pieza en el skin: lo dibuja `pause_icon`.
const PAUSE: &str = "";

/// Un botón de un grupo.
#[derive(Clone, Copy)]
struct Btn {
    icon: &'static str,
    /// El nombre accesible.
    label: &'static str,
    tip: &'static str,
    enabled: bool,
    /// Los que se prenden y apagan: si está prendido.
    on: Option<bool>,
}

impl Btn {
    fn new(icon: &'static str, label: &'static str, tip: &'static str) -> Self {
        Self {
            icon,
            label,
            tip,
            enabled: true,
            on: None,
        }
    }

    fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn on(mut self, on: bool) -> Self {
        self.on = Some(on);
        self
    }
}

/// Un grupo de botones pegados con la esquina en `origin`: el marco, las celdas, las líneas entre
/// celdas y los íconos (presionado, el ícono baja 1). Devuelve una respuesta por botón; los
/// deshabilitados se ven igual (como en el diseño) pero no responden.
fn group(ui: &mut Ui, skin: &Skin, origin: Pos2, buttons: &[Btn]) -> Vec<Response> {
    let n = buttons.len();
    let frame = Rect::from_min_size(origin, vec2(group_width(n), GROUP_H));
    if !piece(ui, skin, GROUP, frame) {
        flat(ui, skin, frame);
    }
    let enabled_ui = ui.is_enabled();
    buttons
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let left = origin.x + 1.0 + BUTTON_PITCH * i as f32;
            let cell = Rect::from_min_size(pos2(left, origin.y + 1.0), BUTTON);
            let hit = Rect::from_min_size(pos2(left, origin.y), vec2(BUTTON.x, GROUP_H));
            let sense = if b.enabled {
                Sense::click()
            } else {
                Sense::hover()
            };
            let response = ui.interact(hit, ui.id().with(("timeline_button", b.label)), sense);
            let enabled = enabled_ui && b.enabled;
            response.widget_info(|| match b.on {
                Some(on) => {
                    egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, b.label)
                }
                None => egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, b.label),
            });
            piece(ui, skin, BUTTON_PIECE, cell);
            if i + 1 < n {
                let sep = Rect::from_min_size(cell.right_top(), vec2(1.0, BUTTON.y));
                piece(ui, skin, SEP, sep);
            }
            let down = if response.is_pointer_button_down_on() {
                1.0
            } else {
                0.0
            };
            let icon_at = cell.min + vec2(1.0, 1.0 + down);
            if b.on == Some(true) && piece_size(skin, ONION_ON).is_some() {
                // La cebolla entera: más alta que el botón, centrada en él.
                let size = piece_size(skin, ONION_ON).unwrap_or(Vec2::ZERO);
                let at = pos2(
                    cell.left() + ((BUTTON.x - size.x) / 2.0).floor(),
                    origin.y + ((GROUP_H - size.y) / 2.0).floor() + down,
                );
                piece_at(ui, skin, ONION_ON, at);
            } else if b.icon == PAUSE {
                pause_icon(ui, skin, icon_at);
            } else if !piece_at(ui, skin, b.icon, icon_at) {
                let initial: String = b.label.chars().take(1).collect();
                let caps = origin.y + LABEL_CAPS + down;
                text(
                    ui,
                    ui.painter(),
                    &initial,
                    cell.left() + 8.0,
                    caps,
                    skin.colors().text,
                );
            }
            response.tip(b.tip)
        })
        .collect()
}

/// Pausa: dos barras de 2×8 en las filas del triángulo de Play, con su tinta.
fn pause_icon(ui: &Ui, skin: &Skin, icon_at: Pos2) {
    let color = skin.colors().field_text;
    for x in [8.0, 12.0] {
        solid(
            ui,
            Rect::from_min_size(icon_at + vec2(x, 3.0), vec2(2.0, 8.0)),
            color,
        );
    }
}

/// El estilo de los números de los campos.
fn number_look(skin: &Skin) -> NumberLook {
    let colors = skin.colors();
    NumberLook {
        text: colors.text,
        outline: colors.outline,
        align: egui::Align::Center,
    }
}

/// Dónde va el `DragValue` de un campo de `left..right` para que sus cifras empiecen en la fila
/// `FIELD_CAPS` del grupo de arriba en `top`: centra su texto en un rect del alto del texto.
fn number_rect(ui: &Ui, left: f32, right: f32, top: f32) -> Rect {
    let ppp = ui.ctx().pixels_per_point();
    let probe = menus::galley(ui, "0", Color32::PLACEHOLDER);
    let caps = -tab_bar::text_pos(&probe, 0.0, 0.0, ppp).y;
    Rect::from_min_max(
        pos2(left, top + FIELD_CAPS - caps),
        pos2(right, top + FIELD_CAPS - caps + probe.size().y),
    )
}

/// Frames antes y después del onion skin: las dos mitades de `timeline.onion_range`, cada una un
/// número que se arrastra o se escribe.
fn onion_range(ui: &mut Ui, editor: &mut Editor, skin: &Skin, origin: Pos2) {
    let size = piece_size(skin, RANGE).unwrap_or(vec2(RANGE_W, GROUP_H + 1.0));
    let rect = Rect::from_min_size(origin, size);
    if !piece(ui, skin, RANGE, rect) {
        flat(
            ui,
            skin,
            Rect::from_min_size(origin, vec2(RANGE_W, GROUP_H)),
        );
    }
    let look = number_look(skin);
    let halves = [
        ("onion_before", 1.0, RANGE_MID, RANGE_BEFORE),
        ("onion_after", RANGE_MID + 1.0, RANGE_W - 1.0, RANGE_AFTER),
    ];
    for (i, (id, from, to, (text_from, text_to))) in halves.into_iter().enumerate() {
        let whole = Rect::from_min_max(
            pos2(origin.x + from, origin.y),
            pos2(origin.x + to, origin.y + GROUP_H),
        );
        let inner = number_rect(ui, origin.x + text_from, origin.x + text_to, origin.y);
        let value = if i == 0 {
            &mut editor.onion.before
        } else {
            &mut editor.onion.after
        };
        let (field, _) =
            bar_fields::drag_number(ui, whole, inner, id, value, 0..=ONION_MAX, 0.1, look);
        field.tip(if i == 0 {
            "Onion skin: frames before"
        } else {
            "Onion skin: frames after"
        });
    }
}

/// − [campo] +: las piezas con la esquina en `origin`. Devuelve el − (con su respuesta), el
/// rect del campo y el +.
fn stepper(
    ui: &mut Ui,
    skin: &Skin,
    origin: Pos2,
    field_w: f32,
    labels: [&'static str; 2],
) -> (Response, Rect, Response) {
    let step = vec2(STEP_W, GROUP_H + 1.0);
    let minus = Rect::from_min_size(origin, step);
    let field = Rect::from_min_size(pos2(minus.right(), origin.y), vec2(field_w, GROUP_H + 1.0));
    let plus = Rect::from_min_size(field.right_top(), step);
    for (name, rect) in [(MINUS, minus), (FIELD, field), (PLUS, plus)] {
        if !piece(ui, skin, name, rect) {
            flat(
                ui,
                skin,
                Rect::from_min_size(rect.min, vec2(rect.width(), GROUP_H)),
            );
        }
    }
    let enabled = ui.is_enabled();
    let button = |rect: Rect, label: &'static str| {
        let hit = Rect::from_min_size(rect.min, vec2(rect.width(), GROUP_H));
        let response = ui.interact(hit, ui.id().with(("timeline_step", label)), Sense::click());
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        response.tip(label)
    };
    let minus_response = button(minus, labels[0]);
    let plus_response = button(plus, labels[1]);
    let field = Rect::from_min_size(field.min, vec2(field_w, GROUP_H));
    (minus_response, field, plus_response)
}

/// Cuánto suma o resta un clic en − o +: `step`, o `big` con Shift.
fn step_of(ui: &Ui, step: i32, big: i32) -> i32 {
    if ui.input(|i| i.modifiers.shift) {
        big
    } else {
        step
    }
}

/// "Opacity" − [0..255] +: la opacidad de la capa. Lo que se arrastra o se escribe se confirma al
/// soltar (un paso de undo); − y + la cambian de a 1 (de a 10 con Shift).
fn opacity(
    ui: &mut Ui,
    editor: &mut Editor,
    state: &mut TimelineState,
    skin: &Skin,
    origin: Pos2,
    widths: &Widths,
) {
    let colors = *skin.colors();
    let painter = ui.painter().clone();
    let shown = menus::galley(ui, "Opacity", colors.field_text);
    let label = Rect::from_min_size(origin, vec2(widths.opacity_label, GROUP_H));
    let label_response = ui.interact(
        label,
        ui.id().with("timeline_opacity_label"),
        Sense::hover(),
    );
    label_response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Opacity"));
    let ppp = ui.ctx().pixels_per_point();
    let at = tab_bar::text_pos(&shown, origin.x, origin.y + LABEL_CAPS, ppp);
    painter.galley(at, shown, colors.field_text);

    let layer = editor.layer;
    let Some(current) = editor.sprite().layer(layer).map(|l| l.opacity) else {
        return;
    };
    let at = pos2(origin.x + widths.opacity_label + LABEL_GAP, origin.y);
    let (minus, field, plus) = stepper(
        ui,
        skin,
        at,
        OPACITY_FIELD_W,
        ["Less opacity", "More opacity"],
    );
    let mut value = state.opacity_edit.unwrap_or(current);
    let before = value;
    let inner = number_rect(ui, field.left(), field.right(), origin.y);
    let (response, zone) = bar_fields::drag_number(
        ui,
        field,
        inner,
        "timeline_opacity",
        &mut value,
        0..=255,
        1.0,
        number_look(skin),
    );
    let response = response.tip("Layer opacity");
    if value != before {
        state.opacity_edit = Some(value);
    }
    let busy = response.dragged() || response.has_focus() || zone.dragged();
    if !busy && let Some(v) = state.opacity_edit.take() {
        editor.set_opacity(layer, v);
    }
    let step = step_of(ui, 1, 10);
    let current = editor.sprite().layer(layer).map_or(current, |l| l.opacity);
    if minus.clicked() {
        editor.set_opacity(layer, (i32::from(current) - step).max(0) as u8);
    }
    if plus.clicked() {
        editor.set_opacity(layer, (i32::from(current) + step).min(255) as u8);
    }
}

/// El combo del modo de mezcla de la capa: `timeline.combo` con el nombre del modo y la lista en el
/// panel de los menús.
fn blend(ui: &mut Ui, editor: &mut Editor, skin: &Skin, origin: Pos2, width: f32) {
    let layer = editor.layer;
    let Some(mode) = editor.sprite().layer(layer).map(|l| l.blend) else {
        return;
    };
    let colors = *skin.colors();
    let name = blend_name(mode);
    let rect = Rect::from_min_size(origin, vec2(width, GROUP_H));
    let response = ui.interact(rect, ui.id().with("timeline_blend"), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, enabled, &name));
    let drawn = Rect::from_min_size(origin, vec2(width, GROUP_H + 1.0));
    if !piece(ui, skin, COMBO, drawn) {
        flat(ui, skin, rect);
    }
    let painter = ui.painter().clone();
    text(
        ui,
        &painter,
        &name,
        origin.x + COMBO_TEXT_LEFT,
        origin.y + LABEL_CAPS,
        colors.text,
    );
    let response = response.tip("Blend mode");
    let look = MenuLook::new(skin, origin.y);
    menus::dropdown(&look, &response, rect, |ui| {
        for m in BLEND_MODES {
            if ui.add(menus::choice(m == mode, &blend_name(m))).clicked() {
                editor.set_blend(layer, m);
            }
        }
    });
}

/// La tinta de la duración en el campo: el número, `UNIT_GAP` y "ms".
fn duration_ink(ui: &Ui, ms: u16) -> f32 {
    ink(ui, &ms.to_string()) + UNIT_GAP + ink(ui, UNIT)
}

/// El reloj y − [ms] +: la duración de los frames elegidos. Como la opacidad: lo que se arrastra o
/// se escribe se confirma al soltar; − y + la cambian de a 10 ms (de a 100 con Shift).
fn duration(
    ui: &mut Ui,
    editor: &mut Editor,
    state: &mut TimelineState,
    skin: &Skin,
    origin: Pos2,
    field_w: f32,
) {
    if !piece_at(ui, skin, CLOCK, origin) {
        let clock = Rect::from_min_size(origin, vec2(CLOCK_W, GROUP_H));
        text(
            ui,
            ui.painter(),
            "t",
            clock.left() + 4.0,
            origin.y + LABEL_CAPS,
            skin.colors().text,
        );
    }
    let at = pos2(origin.x + CLOCK_W + CLOCK_GAP, origin.y);
    let (minus, field, plus) = stepper(ui, skin, at, field_w, ["Shorter frame", "Longer frame"]);
    let current = editor.sprite().frames[editor.frame].duration_ms;
    let mut ms = state.duration_edit.unwrap_or(current);
    let before = ms;
    // El número y "ms" centrados juntos en el blanco del campo (entre sus bordes), con
    // `UNIT_GAP` entre los dos; "ms" no se edita.
    let number = ink(ui, &ms.to_string());
    let white = field.width() - 2.0;
    let left = field.left() + 1.0 + ((white - duration_ink(ui, ms)) / 2.0).floor();
    let inner = number_rect(ui, left, left + number + 1.0, origin.y);
    let look = NumberLook {
        align: egui::Align::Min,
        ..number_look(skin)
    };
    let (response, zone) = bar_fields::drag_number(
        ui,
        field,
        inner,
        "timeline_duration",
        &mut ms,
        1..=60000,
        1.0,
        look,
    );
    if !response.has_focus() {
        let painter = ui.painter().clone();
        let at = left + number + UNIT_GAP;
        text(
            ui,
            &painter,
            UNIT,
            at,
            origin.y + FIELD_CAPS,
            skin.colors().text,
        );
    }
    let response = response.tip("Duration of the selected frames");
    if ms != before {
        state.duration_edit = Some(ms);
    }
    let busy = response.dragged() || response.has_focus() || zone.dragged();
    if !busy && let Some(v) = state.duration_edit.take() {
        editor.set_duration(v);
    }
    let step = step_of(ui, 10, 100);
    let current = i32::from(editor.sprite().frames[editor.frame].duration_ms);
    if minus.clicked() {
        editor.set_duration((current - step).max(1) as u16);
    }
    if plus.clicked() {
        editor.set_duration((current + step).min(60000) as u16);
    }
}

/// La línea de abajo de la barra, de lado a lado de la caja (se une a sus lados), con la fila de
/// arriba en la última de la barra.
fn divider(ui: &Ui, skin: &Skin, inner: Rect, bar_bottom: f32) {
    let rect = Rect::from_min_max(
        pos2(inner.left() - 1.0, bar_bottom - 1.0),
        pos2(inner.right() + 1.0, bar_bottom - 1.0 + DIVIDER_H),
    );
    // Sale 1 columna de cada lado del interior (`with_clip_rect` solo achica el recorte).
    let mut painter = ui.painter().clone();
    painter.set_clip_rect(ui.clip_rect().expand(1.0));
    if let Some(p) = skin.piece(DIVIDER, WidgetState::Normal) {
        let ppp = ui.ctx().pixels_per_point();
        painter.extend(pixel_ui::piece_shapes(p, rect, ppp));
    } else {
        let ppp = ui.ctx().pixels_per_point();
        painter.add(pixel_ui::etched_hline(
            inner.x_range(),
            bar_bottom,
            skin.colors(),
            ppp,
        ));
    }
}

// ---- Tabla ----

/// El contenido de una celda de `rect`: sin su línea de abajo ni la de la derecha.
pub(super) fn content(rect: Rect) -> Rect {
    Rect::from_min_max(rect.min, rect.max - vec2(LINE, LINE))
}

/// Las líneas grabadas de una fila de la tabla que ocupa `row`, con `frames` columnas de frame.
/// Abajo, la línea a lo ancho, sin la última columna. Entre columnas, la
/// oscura y su brillo desde una fila antes de la fila (el brillo de la línea de arriba) hasta la
/// oscura de abajo; a la derecha de la tabla, solo la oscura. Se dibujan los brillos y después
/// las oscuras, como en el diseño.
fn row_lines(ui: &Ui, skin: &Skin, row: Rect, frames: usize) {
    let colors = skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let (top, dark_y) = (row.top() - 1.0, row.bottom() - LINE);
    let mut bounds = vec![
        row.left() + EYE_W,
        row.left() + EYE_W + LOCK_W,
        row.left() + EYE_W + LOCK_W + NAME_W,
    ];
    let first = row.left() + EYE_W + LOCK_W + NAME_W;
    bounds.extend((1..frames).map(|f| first + CELL_W * f as f32));
    let mut mesh = Mesh::default();
    let mut rect = |r: Rect, color: Color32| mesh.add_colored_rect(r.round_to_pixels(ppp), color);
    // Brillos: el de abajo y el de cada línea entre columnas.
    rect(
        Rect::from_min_max(
            pos2(row.left(), dark_y + 1.0),
            pos2(row.right() - 1.0, row.bottom()),
        ),
        colors.highlight,
    );
    for &b in &bounds {
        rect(
            Rect::from_min_max(pos2(b - 1.0, top), pos2(b, dark_y)),
            colors.highlight,
        );
    }
    // Oscuras: entre columnas, la de la derecha de la tabla y la de abajo.
    for b in bounds.iter().copied().chain([row.right()]) {
        rect(
            Rect::from_min_max(pos2(b - LINE, top), pos2(b - 1.0, dark_y)),
            colors.canvas,
        );
    }
    rect(
        Rect::from_min_max(
            pos2(row.left(), dark_y),
            pos2(row.right() - 1.0, dark_y + 1.0),
        ),
        colors.canvas,
    );
    ui.painter().add(Shape::mesh(mesh));
}

/// Las columnas de la izquierda de una fila: ojo, candado y nombre.
fn left_columns(row: Rect) -> [Rect; 3] {
    let eye = Rect::from_min_size(row.min, vec2(EYE_W, ROW_H));
    let lock = Rect::from_min_size(eye.right_top(), vec2(LOCK_W, ROW_H));
    let name = Rect::from_min_size(lock.right_top(), vec2(NAME_W, ROW_H));
    [eye, lock, name]
}

/// El encabezado: las líneas y los íconos de las columnas (el ojo abierto y el candado cerrado).
pub(super) fn header_left(ui: &Ui, skin: &Skin, row: Rect, frames: usize) {
    row_lines(ui, skin, row, frames);
    let [eye, lock, _] = left_columns(row);
    piece_at(ui, skin, EYE, eye.min + EYE_AT);
    piece_at(ui, skin, LOCK, lock.min + LOCK_AT);
}

/// Una fila de capa: las líneas y, si es la activa, el ojo, el candado y el nombre elegidos.
pub(super) fn layer_left(ui: &Ui, skin: &Skin, row: Rect, frames: usize, active: bool) {
    row_lines(ui, skin, row, frames);
    if active {
        for column in left_columns(row) {
            select(ui, skin, column, row.left(), false);
        }
    }
}

/// El ojo (abierto si se ve, cerrado si no) y el candado (cerrado si está bloqueada) de una capa.
/// En la capa activa (`selected`), en su versión para lo elegido.
pub(super) fn eye_and_lock(
    ui: &Ui,
    skin: &Skin,
    eye: Rect,
    lock: Rect,
    visible: bool,
    locked: bool,
    selected: bool,
) {
    let eye_name = if visible { EYE } else { EYE_CLOSED };
    icon_at(ui, skin, eye_name, eye.min + EYE_AT, selected);
    let lock_name = if locked { LOCK } else { LOCK_OPEN };
    icon_at(ui, skin, lock_name, lock.min + LOCK_AT, selected);
}

/// La carpetita de un grupo al principio de `rect` (su lugar en la columna del nombre): cerrada si
/// está `collapsed`; en la capa activa (`selected`), en su versión para lo elegido.
pub(super) fn folder(ui: &Ui, skin: &Skin, rect: Rect, collapsed: bool, selected: bool) {
    let name = if collapsed { FOLDER } else { FOLDER_OPEN };
    let size = piece_size(skin, name).unwrap_or(Vec2::ZERO);
    let at = pos2(rect.left(), rect.top() + FOLDER_BOTTOM - size.y);
    icon_at(ui, skin, name, at, selected);
    let arrow = if collapsed { FOLD_CLOSED } else { FOLD_OPEN };
    let size = piece_size(skin, arrow).unwrap_or(Vec2::ZERO);
    let middle = rect.top() + FOLDER_BOTTOM - 4.5;
    let at = pos2(rect.left() + FOLD_LEFT, (middle - size.y / 2.0).floor());
    icon_at(ui, skin, arrow, at, selected);
}

/// La flecha de una capa recortada (máscara de recorte) con su esquina de arriba en `at`, 2 filas
/// debajo de lo de arriba del contenido; en la capa activa, en su versión para lo elegido.
pub(super) fn clip_arrow(ui: &Ui, skin: &Skin, at: Pos2, selected: bool) {
    icon_at(ui, skin, CLIP, at + vec2(0.0, CLIP_TOP), selected);
}

/// Las máscaras de capa del diseño (frame 4): la cara; editándola, la del borde rosa; y el botón
/// para agregar una.
const MASK: &str = "timeline.mask";
const MASK_EDITING: &str = "timeline.mask.editing";
const MASK_ADD: &str = "timeline.mask.add";
/// La cara y el botón: su tamaño.
pub(super) const MASK_SIZE: Vec2 = vec2(9.0, 9.0);
pub(super) const MASK_ADD_SIZE: Vec2 = vec2(11.0, 9.0);

/// La máscara de una capa con su esquina en `at`; desactivada, a media luz.
pub(super) fn mask_chip(ui: &Ui, skin: &Skin, at: Pos2, editing: bool, enabled: bool) {
    let name = if editing { MASK_EDITING } else { MASK };
    let ppp = ui.ctx().pixels_per_point();
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.set_opacity(0.4);
    }
    painter.extend(crate::pixel_draw::piece_at(skin, name, at, ppp));
}

/// El botón para agregar una máscara, con su esquina en `at`.
pub(super) fn mask_add(ui: &Ui, skin: &Skin, at: Pos2) {
    let ppp = ui.ctx().pixels_per_point();
    ui.painter()
        .extend(crate::pixel_draw::piece_at(skin, MASK_ADD, at, ppp));
}

/// El ícono de una capa tilemap al principio de `rect` (donde va la carpetita de los grupos); en
/// la capa activa (`selected`), en su versión para lo elegido.
pub(super) fn tilemap_icon(ui: &Ui, skin: &Skin, rect: Rect, selected: bool) {
    let size = piece_size(skin, TILEMAP).unwrap_or(Vec2::ZERO);
    let at = pos2(rect.left(), rect.top() + FOLDER_BOTTOM - size.y);
    icon_at(ui, skin, TILEMAP, at, selected);
}

/// Lo elegido, como en el diseño: la celda `cell` (contenido y su línea de la derecha y de
/// abajo) en `timeline_selection`, enmarcada con las líneas grabadas en rosa: la oscura de arriba
/// y su brillo, la de la izquierda y su brillo (si la celda no es la primera, en `table_left`), la
/// oscura de la derecha y la de abajo. `header`: la fila de los números, donde la línea de arriba
/// es la de la barra (va solo sobre la celda).
pub(super) fn select(ui: &Ui, skin: &Skin, cell: Rect, table_left: f32, header: bool) {
    let colors = skin.colors();
    let (fill, dark, light) = (
        colors.timeline_selection,
        colors.timeline_selection_dark,
        colors.timeline_selection_light,
    );
    let inner = content(cell);
    let (x0, y0, x1, y1) = (inner.left(), inner.top(), inner.right(), inner.bottom());
    let first = x0 <= table_left;
    let px = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y), vec2(w, h));
    let ppp = ui.ctx().pixels_per_point();
    let mut mesh = Mesh::default();
    let mut rect = |r: Rect, color: Color32| mesh.add_colored_rect(r.round_to_pixels(ppp), color);
    rect(inner, fill);
    // Arriba: la línea oscura y su brillo (en la fila de los números, la de la barra y solo
    // sobre la celda).
    let (dark_from, dark_to) = if header {
        (x0 - 1.0, x1)
    } else if first {
        (x0, x1 + 1.0)
    } else {
        (x0 - 2.0, x1 + 1.0)
    };
    rect(px(dark_from, y0 - 2.0, dark_to - dark_from, 1.0), dark);
    let light_from = if first { x0 } else { x0 - 1.0 };
    rect(px(light_from, y0 - 1.0, x1 - light_from, 1.0), light);
    // La izquierda (si hay una celda antes): la oscura y su brillo.
    if !first {
        let top = if header { y0 - 1.0 } else { y0 - 2.0 };
        rect(px(x0 - 2.0, top, 1.0, y1 + 1.0 - top), dark);
        rect(px(x0 - 1.0, y0 - 1.0, 1.0, y1 - y0 + 1.0), light);
    }
    // La derecha y abajo: las oscuras.
    let top = if header { y0 - 1.0 } else { y0 - 2.0 };
    rect(px(x1, top, 1.0, y1 + 1.0 - top), dark);
    let left = if first { x0 } else { x0 - 2.0 };
    rect(px(left, y1, x1 + 1.0 - left, 1.0), dark);
    ui.painter().add(Shape::mesh(mesh));
}

/// El número del frame `f` en su columna del encabezado, centrado; el actual y los del rango, en
/// el color de selección.
pub(super) fn frame_number(
    ui: &Ui,
    skin: &Skin,
    cell: Rect,
    f: usize,
    current: bool,
    in_range: bool,
) {
    let colors = skin.colors();
    let inner = content(cell);
    if current || in_range {
        select(ui, skin, cell, f32::NEG_INFINITY, true);
    }
    let label = (f + 1).to_string();
    let left = inner.left() + ((inner.width() - ink(ui, &label)) / 2.0).floor();
    let painter = ui.painter().with_clip_rect(inner.intersect(ui.clip_rect()));
    text(
        ui,
        &painter,
        &label,
        left,
        inner.top() + TEXT_CAPS,
        colors.text,
    );
}

/// El nombre de una capa en `rect` (recortado a él), con las mayúsculas en la fila de los textos.
pub(super) fn name(ui: &Ui, rect: Rect, label: &str, color: Color32) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    text(
        ui,
        &painter,
        label,
        rect.left(),
        rect.top() + TEXT_CAPS,
        color,
    );
}

/// Un cel en el contenido `inner` de su celda: ● con imagen, ○ vacío. Si comparte la imagen con el
/// anterior, una línea lo une con el ● de la izquierda.
pub(super) fn cel(
    ui: &Ui,
    skin: &Skin,
    inner: Rect,
    full: bool,
    linked_prev: bool,
    selected: bool,
) {
    let at = inner.min + CEL_AT;
    let name = if full { CEL } else { CEL_EMPTY };
    if linked_prev {
        let size = piece_size(skin, CEL).unwrap_or(vec2(7.0, 7.0));
        let y = at.y + (size.y / 2.0).floor();
        let from = at.x - CELL_W + size.x;
        solid(
            ui,
            Rect::from_min_max(pos2(from, y), pos2(at.x, y + 1.0)),
            skin.colors().field_text,
        );
    }
    if !icon_at(ui, skin, name, at, selected) {
        let center = inner.center();
        let color = skin.colors().field_text;
        if full {
            ui.painter().circle_filled(center, 3.0, color);
        } else {
            ui.painter()
                .circle_stroke(center, 3.0, egui::Stroke::new(1.0, color));
        }
    }
}

/// El alto de cada piso de la fila de tags (la cajita del nombre y 2 de aire) y lo que bajan los
/// corchetes después del último piso, hasta los números de los frames.
pub(super) const TAG_LEVEL: f32 = 13.0;
pub(super) const TAG_FOOT: f32 = 4.0;
/// La cajita del nombre: su alto y el aire a los lados de la tinta.
const TAG_BOX_H: f32 = 11.0;
const TAG_BOX_PAD: f32 = 3.0;

/// Dónde se toca un tag del piso `level` cuyos frames ocupan `span`: la cajita y la línea de
/// arriba del corchete, de punta a punta.
pub(super) fn tag_hit(span: Rect, level: usize) -> Rect {
    let top = span.top() + level as f32 * TAG_LEVEL;
    Rect::from_min_max(pos2(span.left(), top), pos2(span.right(), top + TAG_BOX_H))
}

/// Un tag como en Aseprite: su nombre en una cajita del color del tag (arriba, en el piso
/// `level`) y un corchete de su color desde la cajita hasta `bottom` (los números), en los
/// bordes de sus frames (`span`).
pub(super) fn tag(ui: &Ui, span: Rect, bottom: f32, level: usize, color: Color32, name: &str) {
    let top = span.top() + level as f32 * TAG_LEVEL;
    let (left, right) = (span.left() + 1.0, span.right() - 2.0);
    let mid = top + (TAG_BOX_H / 2.0).floor();
    // El corchete: arriba de punta a punta y los lados hasta abajo.
    solid(
        ui,
        Rect::from_min_max(pos2(left, mid), pos2(right + 1.0, mid + 1.0)),
        color,
    );
    solid(
        ui,
        Rect::from_min_max(pos2(left, mid), pos2(left + 1.0, bottom)),
        color,
    );
    solid(
        ui,
        Rect::from_min_max(pos2(right, mid), pos2(right + 1.0, bottom)),
        color,
    );
    // La cajita con el nombre, en blanco o negro según lo claro del color.
    // Tan ancha como el nombre, aunque el tag sea más angosto (como en Aseprite).
    let width = ink(ui, name) + 2.0 * TAG_BOX_PAD;
    let label = Rect::from_min_size(pos2(left, top), vec2(width.max(4.0), TAG_BOX_H));
    solid(ui, label, color);
    let luma = 0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32;
    let ink_color = if luma > 150.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    let painter = ui.painter().with_clip_rect(label.intersect(ui.clip_rect()));
    text(
        ui,
        &painter,
        name,
        label.left() + TAG_BOX_PAD,
        label.top() + 2.0,
        ink_color,
    );
}
