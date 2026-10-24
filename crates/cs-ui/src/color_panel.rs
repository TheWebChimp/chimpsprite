//! Con Pixel UI, la paleta y el editor de color del panel derecho como en el diseño
//! (`Design.aseprite`, frame 4, capa Mockup: la columna de la derecha, x 784..972). Las medidas
//! son píxeles del diseño, que con la escala entera de Pixel UI son puntos:
//!
//! - **Paleta** (y 120..326): la caja `palette.box` (9-slice, transparente adentro) de todo el
//!   ancho y al menos 207 de alto. Los cuadritos empiezan 4 px adentro, uno cada 12: cada uno es
//!   `palette.swatch` (13×13, borde negro de 1 compartido con los vecinos) con el color en sus
//!   11×11. El primero es el color Mask. En la celda que sigue al último, a (3,3), la marca
//!   `palette.add` (Ⅲ): agrega el color primario a la paleta.
//! - **Editor** (5 filas debajo de la caja, y 332..473):
//!   - Arriba, `picker.copy` (25×16, 2 px desde la izquierda) copia el color en hex. 2 px
//!     después, la tira de canales (16 de alto, hasta 2 px antes del borde derecho):
//!     `channel.r` (la punta y el tramo rojo, 30), `channel.g` y `channel.b` (27),
//!     `channel.a` (27, el damero repetido cada 4) y `channel.hex` (el resto, con la punta
//!     derecha), con `channel.sep` (1) entre tramo y tramo. Cada canal lleva su punto
//!     (`channel.dot.*`, 6×6) 2 px después del comienzo del tramo y 4 filas abajo, y su número 9
//!     px después; el hex, 3 px después. Las mayúsculas en la fila 3 (los dígitos, una más
//!     abajo), en `field_text`.
//!   - 3 filas más abajo, la caja del cuadro S/V (`picker.sv`, 92 de alto con la línea que lo
//!     separa de las barras): el cuadro mide 86 de alto y va 3 px adentro. Debajo,
//!     `picker.bars` (31): el tono y el alfa, de 13 cada uno, 2 filas desde arriba y con el
//!     ancho del cuadro.
//!   - Los degradés se calculan acá, como en el selector de egui (HSV "gamma"). El alfa va
//!     sobre un damero gris (128/192) de celdas de 17×7.
//!   - Los cursores: `picker.cursor` (9×9) centrado en el S/V, recortado al cuadro;
//!     `picker.cursor.bar` (7×7) en las barras, de 1 px de la punta izquierda (0) a 1 px de la
//!     derecha (1), 3 filas abajo del borde de la barra. Son los "○ ○" sueltos del diseño
//!     (x 786..806, y 483..491), iguales a los dibujados sobre el cuadro y las barras.

use eframe::egui::ecolor::{Hsva, HsvaGamma};
use eframe::egui::{
    self, Align, Color32, Layout, Mesh, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2, WidgetInfo, WidgetType, emath::remap_clamp, lerp, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use cs_core::{PixelValue, Rgba};

use crate::bar_fields::{self, NumberLook};
use crate::editor::Editor;
use crate::menus::{galley, ink_width, solid};
use crate::panels::{self, COLOR_PICKER_MAX};
use crate::pixel_ui;
use crate::skin::{Piece, Skin, SkinColors, WidgetState};
use crate::tab_bar;

/// Filas entre la caja de la paleta y el editor de color (y 327..331 del diseño).
pub const EDITOR_GAP: f32 = 5.0;

const PALETTE_BOX: &str = "palette.box";
const SWATCH: &str = "palette.swatch";
const ADD: &str = "palette.add";
const COPY: &str = "picker.copy";
const SV_BOX: &str = "picker.sv";
const BARS_BOX: &str = "picker.bars";
const CURSOR: &str = "picker.cursor";
const BAR_CURSOR: &str = "picker.cursor.bar";
const SEPARATOR: &str = "channel.sep";
const HEX: &str = "channel.hex";

/// Alto mínimo de la caja de la paleta (la del diseño).
const PALETTE_MIN_HEIGHT: f32 = 207.0;
/// De la caja a la grilla, a cada lado.
const GRID_INSET: f32 = 4.0;
/// De cuadrito a cuadrito (comparten el borde).
const CELL: f32 = 12.0;
/// Un cuadrito con su borde.
const SWATCH_SIDE: f32 = 13.0;
/// La marca para agregar, desde la esquina de su celda.
const ADD_OFFSET: Vec2 = Vec2::new(3.0, 3.0);
const FALLBACK_ADD_SIZE: Vec2 = Vec2::new(6.0, 7.0);

/// El botón de copiar y la tira de canales.
const COPY_LEFT: f32 = 2.0;
const FALLBACK_COPY_SIZE: Vec2 = Vec2::new(25.0, 16.0);
const STRIP_GAP: f32 = 2.0;
const STRIP_RIGHT: f32 = 2.0;
const STRIP_HEIGHT: f32 = 16.0;
/// La punta izquierda de `channel.r` (borde y esquina), antes del tramo rojo.
const STRIP_END: f32 = 3.0;
/// El tramo de un canal sin su línea divisoria.
const SEGMENT: f32 = 27.0;
/// El tramo del hex como mínimo (con la punta derecha).
const HEX_MIN: f32 = 43.0;
/// Del comienzo del tramo al punto, al número y (en el hex) al texto.
const DOT_LEFT: f32 = 2.0;
const DOT_TOP: f32 = 4.0;
const DOT_SIZE: Vec2 = Vec2::new(6.0, 6.0);
const VALUE_LEFT: f32 = 9.0;
const HEX_TEXT_LEFT: f32 = 3.0;
/// La fila de las mayúsculas en la tira (los dígitos empiezan una más abajo, en la 4).
const STRIP_CAPS: f32 = 3.0;
/// Las 2 filas de abajo de cada tramo, del color de su canal (sin la pieza).
const UNDERLINE: f32 = 2.0;
/// El tramo del alfa repite un período del damero de su línea.
const ALPHA_TILE: f32 = 4.0;

/// La caja del cuadro S/V y las barras, desde arriba de la tira.
const PICKER_TOP: f32 = 19.0;
const SV_BOX_HEIGHT: f32 = 92.0;
const SV_INSET: f32 = 3.0;
const SV_HEIGHT: f32 = 86.0;
const BARS_BOX_HEIGHT: f32 = 31.0;
const BARS_TOP: f32 = 2.0;
const BAR_HEIGHT: f32 = 13.0;
/// Cursores: el del cuadro (9×9) y el de las barras (7×7), que va a 1 px de las puntas y 3 filas
/// abajo del borde de la barra.
const CURSOR_SIZE: f32 = 9.0;
const BAR_CURSOR_SIZE: f32 = 7.0;
const BAR_CURSOR_MARGIN: f32 = 1.0;
const BAR_CURSOR_TOP: f32 = 3.0;
/// El damero del alfa: celdas de 17×7 en grises 128 y 192.
const ALPHA_CELL: Vec2 = Vec2::new(17.0, 7.0);
const ALPHA_DARK: Color32 = Color32::from_gray(128);
const ALPHA_LIGHT: Color32 = Color32::from_gray(192);
/// Pasos de los degradés (como el selector de egui).
const STEPS: usize = 36;

fn piece<'a>(skin: &'a Skin, name: &str) -> Option<&'a Piece> {
    skin.piece(name, WidgetState::Normal)
}

fn size_of(piece: &Piece) -> Vec2 {
    vec2(piece.size[0] as f32, piece.size[1] as f32)
}

/// `name` estirada sobre `rect` o, sin la pieza, lo que dibuje `fallback`.
fn piece_or(
    skin: &Skin,
    name: &str,
    rect: Rect,
    ppp: f32,
    fallback: impl FnOnce() -> Vec<Shape>,
) -> Vec<Shape> {
    match piece(skin, name) {
        Some(p) => pixel_ui::piece_shapes(p, rect, ppp),
        None => fallback(),
    }
}

/// Un recuadro de 1 px (los respaldos sin pieza).
fn outline(rect: Rect, color: Color32, ppp: f32) -> Shape {
    Shape::rect_stroke(
        rect.round_to_pixels(ppp),
        0.0,
        Stroke::new(1.0, color),
        StrokeKind::Inside,
    )
}

/// La paleta en su caja: el color Mask, los de la paleta y la marca para agregar el primario. Clic
/// elige el primario; clic derecho, el secundario.
pub fn palette_grid(ui: &mut Ui, editor: &mut Editor, skin: &Skin) {
    let ppp = ui.ctx().pixels_per_point();
    let width = ui.available_width().floor();
    let columns = (((width - 2.0 * GRID_INSET - 1.0) / CELL).floor() as usize).max(1);
    let count = editor.sprite().palette.len().min(256);
    // El Mask, los colores y la marca.
    let cells = count + 2;
    let rows = cells.div_ceil(columns);
    let grid_height = rows as f32 * CELL + 1.0;
    let height = PALETTE_MIN_HEIGHT.max(grid_height + 2.0 * GRID_INSET);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), rect.size());
    let colors = *skin.colors();
    ui.painter()
        .extend(piece_or(skin, PALETTE_BOX, rect, ppp, || {
            vec![outline(rect, colors.outline, ppp)]
        }));
    let origin = rect.min + Vec2::splat(GRID_INSET);
    let cell_at =
        |i: usize| origin + vec2((i % columns) as f32 * CELL, (i / columns) as f32 * CELL);
    let id = ui.id().with("pixel_palette");
    let enabled = ui.is_enabled();
    for i in 0..count + 1 {
        let cell = Rect::from_min_size(cell_at(i), Vec2::splat(SWATCH_SIDE));
        let inside = cell.shrink(1.0);
        let response = ui.interact(cell, id.with(i), Sense::click());
        let painter = ui.painter();
        // La primera celda es el Mask; las demás, los colores de la paleta.
        let (value, label) = if i == 0 {
            panels::paint_mask(painter, inside);
            (editor.mask_value(), "Mask".to_owned())
        } else {
            let index = i - 1;
            let color = panels::to_color32(editor.sprite().palette.colors()[index]);
            if color.a() < 255 {
                panels::checker(painter, inside);
            }
            painter.rect_filled(inside, 0.0, color);
            (editor.palette_value(index as u8), format!("#{index}"))
        };
        painter.extend(piece_or(skin, SWATCH, cell, ppp, || {
            vec![outline(cell, Color32::BLACK, ppp)]
        }));
        panels::mark_selected(ui, editor, value, inside);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &label));
        let response = if i == 0 {
            response.on_hover_text(panels::MASK_HINT)
        } else {
            response.on_hover_text(label)
        };
        panels::pick_on_click(editor, &response, value);
    }
    // La marca para agregar el primario, en la celda siguiente.
    let add_piece = piece(skin, ADD);
    let add = Rect::from_min_size(
        cell_at(count + 1) + ADD_OFFSET,
        add_piece.map_or(FALLBACK_ADD_SIZE, size_of),
    );
    let response = ui.interact(add, id.with("add"), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, "Add to palette"));
    let shapes = match add_piece {
        Some(p) => pixel_ui::piece_shapes(p, add, ppp),
        // Sin la pieza, tres rayas como la Ⅲ.
        None => (0..3)
            .map(|k| {
                let bar = Rect::from_min_size(add.min + vec2(k as f32 * 2.0, 0.0), vec2(1.0, 6.0));
                solid(bar, colors.outline, ppp)
            })
            .collect(),
    };
    ui.painter().extend(shapes);
    if response.on_hover_text("Add to palette").clicked() && !editor.is_mask(editor.primary) {
        let color = editor.resolve(editor.primary);
        editor.add_to_palette(color);
    }
}

/// El estado del editor entre frames: el último color y su HSV, para no perder el tono de los
/// grises (como el caché del selector de egui).
#[derive(Clone, Copy)]
struct Remembered {
    color: Rgba,
    hsv: HsvaGamma,
}

fn hsv_of(color: Rgba) -> HsvaGamma {
    Hsva::from_srgba_unmultiplied([color.r, color.g, color.b, color.a]).into()
}

fn rgba_of(hsv: HsvaGamma) -> Rgba {
    let [r, g, b, a] = Hsva::from(hsv).to_srgba_unmultiplied();
    Rgba::new(r, g, b, a)
}

/// El editor del color primario (`current`): copiar, la tira de canales, el cuadro S/V y las
/// barras de tono y alfa.
pub fn color_editor(ui: &mut Ui, editor: &mut Editor, skin: &Skin, current: Rgba) {
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    // El ancho del panel, con el máximo de siempre del selector.
    let width = ui.available_width().min(COLOR_PICKER_MAX).floor();
    let height = PICKER_TOP + SV_BOX_HEIGHT + BARS_BOX_HEIGHT;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), rect.size());
    let id = ui.id().with("pixel_color_editor");

    // Copiar.
    let copy_piece = piece(skin, COPY);
    let copy = Rect::from_min_size(
        rect.min + vec2(COPY_LEFT, 0.0),
        copy_piece.map_or(FALLBACK_COPY_SIZE, size_of),
    );
    copy_button(ui, skin, id.with("copy"), copy, current);

    // La tira de canales.
    let strip = Rect::from_min_max(
        pos2(copy.right() + STRIP_GAP, rect.top()),
        pos2(rect.right() - STRIP_RIGHT, rect.top() + STRIP_HEIGHT),
    );
    let mut color = channel_strip(ui, skin, &colors, id, strip, current);

    // El cuadro S/V y las barras.
    let remembered = ui
        .data(|d| d.get_temp::<Remembered>(id))
        .filter(|r| r.color == color)
        .map_or_else(|| hsv_of(color), |r| r.hsv);
    let mut hsv = remembered;
    let sv_box = Rect::from_min_size(rect.min + vec2(0.0, PICKER_TOP), vec2(width, SV_BOX_HEIGHT));
    let bars_box = Rect::from_min_size(
        pos2(rect.left(), sv_box.bottom()),
        vec2(width, BARS_BOX_HEIGHT),
    );
    ui.painter().extend(piece_or(skin, SV_BOX, sv_box, ppp, || {
        vec![outline(sv_box, colors.outline, ppp)]
    }));
    ui.painter()
        .extend(piece_or(skin, BARS_BOX, bars_box, ppp, || {
            vec![outline(bars_box, colors.outline, ppp)]
        }));
    let sv = Rect::from_min_size(
        sv_box.min + Vec2::splat(SV_INSET),
        vec2(width - 2.0 * SV_INSET, SV_HEIGHT),
    );
    let hue = Rect::from_min_size(
        pos2(sv.left(), bars_box.top() + BARS_TOP),
        vec2(sv.width(), BAR_HEIGHT),
    );
    let alpha = Rect::from_min_size(hue.left_bottom(), hue.size());
    sv_square(ui, skin, id.with("sv"), sv, &mut hsv);
    hue_bar(ui, skin, id.with("hue"), hue, &mut hsv);
    alpha_bar(ui, skin, id.with("alpha"), alpha, &mut hsv);
    if hsv != remembered {
        color = rgba_of(hsv);
    }
    ui.data_mut(|d| d.insert_temp(id, Remembered { color, hsv }));
    if color != current {
        editor.primary = PixelValue::Rgba(color);
    }
}

/// El botón de copiar: copia el color en hex (`#rrggbb`, o `#rrggbbaa` con alfa).
fn copy_button(ui: &mut Ui, skin: &Skin, id: egui::Id, rect: Rect, current: Rgba) {
    let ppp = ui.ctx().pixels_per_point();
    let response = ui.interact(rect, id, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, "Copy color"));
    let state = if response.is_pointer_button_down_on() {
        WidgetState::Down
    } else if response.hovered() {
        WidgetState::Hover
    } else {
        WidgetState::Normal
    };
    let shapes = match skin.piece(COPY, state) {
        Some(p) => pixel_ui::piece_shapes(p, rect, ppp),
        None => {
            let colors = skin.colors();
            vec![
                solid(rect, colors.panel, ppp),
                outline(rect, colors.outline, ppp),
            ]
        }
    };
    ui.painter().extend(shapes);
    if response
        .on_hover_text(format!("Copy {}", current.to_hex()))
        .clicked()
    {
        ui.ctx().copy_text(current.to_hex());
    }
}

/// Un canal de la tira: su pieza, su punto y el color de su línea (sin la pieza).
struct Channel {
    piece: &'static str,
    dot: &'static str,
    label: &'static str,
    underline: Color32,
}

const CHANNELS: [Channel; 4] = [
    Channel {
        piece: "channel.r",
        dot: "channel.dot.r",
        label: "R",
        underline: Color32::from_rgb(241, 36, 36),
    },
    Channel {
        piece: "channel.g",
        dot: "channel.dot.g",
        label: "G",
        underline: Color32::from_rgb(97, 165, 63),
    },
    Channel {
        piece: "channel.b",
        dot: "channel.dot.b",
        label: "B",
        underline: Color32::from_rgb(39, 137, 205),
    },
    Channel {
        piece: "channel.a",
        dot: "channel.dot.a",
        label: "A",
        underline: Color32::from_rgb(154, 151, 185),
    },
];

/// Un tramo de respaldo (sin la pieza): blanco con borde arriba y abajo y su línea de color.
fn flat_segment(rect: Rect, underline: Color32, colors: &SkinColors, ppp: f32) -> Vec<Shape> {
    let line = Rect::from_min_max(
        pos2(rect.left(), rect.bottom() - 1.0 - UNDERLINE),
        pos2(rect.right(), rect.bottom() - 1.0),
    );
    vec![
        solid(rect, Color32::WHITE, ppp),
        solid(line, underline, ppp),
        solid(
            Rect::from_min_size(rect.min, vec2(rect.width(), 1.0)),
            colors.field_text,
            ppp,
        ),
        solid(
            Rect::from_min_size(rect.left_bottom() - vec2(0.0, 1.0), vec2(rect.width(), 1.0)),
            colors.field_text,
            ppp,
        ),
    ]
}

/// La tira de canales en `strip`: R, G, B y A (se arrastran o se escriben) y el hex (se escribe).
/// Devuelve el color con lo editado.
fn channel_strip(
    ui: &mut Ui,
    skin: &Skin,
    colors: &SkinColors,
    id: egui::Id,
    strip: Rect,
    current: Rgba,
) -> Rgba {
    let ppp = ui.ctx().pixels_per_point();
    // Los tramos miden lo del diseño; si el número más ancho no entra con la fuente, crecen.
    let widest = ink_width(&galley(ui, "255", colors.field_text));
    let segment = SEGMENT.max(VALUE_LEFT + widest + 2.0);
    let mut values = [current.r, current.g, current.b, current.a];
    let look = NumberLook {
        text: colors.field_text,
        outline: colors.field_text,
        align: Align::Min,
    };
    // Dónde empiezan las mayúsculas de un texto en la tira (para el `DragValue` y el hex).
    let probe = galley(ui, "0", colors.field_text);
    let caps = -tab_bar::text_pos(&probe, 0.0, 0.0, ppp).y;
    let text_rect = |left: f32, right: f32| {
        Rect::from_min_max(
            pos2(left, strip.top() + STRIP_CAPS - caps),
            pos2(right, strip.top() + STRIP_CAPS - caps + probe.size().y),
        )
    };
    let mut left = strip.left();
    for (i, channel) in CHANNELS.iter().enumerate() {
        // El tramo rojo trae la punta izquierda de la tira.
        let end = if i == 0 { STRIP_END } else { 0.0 };
        let rect = Rect::from_min_size(pos2(left, strip.top()), vec2(end + segment, STRIP_HEIGHT));
        let fill_left = left + end;
        let mut shapes = match piece(skin, channel.piece) {
            // El alfa repite su pieza (un período del damero), recortada al tramo.
            Some(p) if p.center.is_none() => {
                let tile = size_of(p);
                let mut tiles = Vec::new();
                let mut x = rect.left();
                while x < rect.right() {
                    let at = Rect::from_min_size(pos2(x, rect.top()), tile);
                    tiles.extend(pixel_ui::piece_shapes(p, at, ppp));
                    x += ALPHA_TILE.max(tile.x);
                }
                tiles
            }
            Some(p) => pixel_ui::piece_shapes(p, rect, ppp),
            None => flat_segment(rect, channel.underline, colors, ppp),
        };
        let dot = Rect::from_min_size(pos2(fill_left + DOT_LEFT, strip.top() + DOT_TOP), DOT_SIZE);
        shapes.extend(piece_or(skin, channel.dot, dot, ppp, || {
            vec![Shape::circle_filled(dot.center(), 3.0, channel.underline)]
        }));
        ui.painter()
            .with_clip_rect(rect.intersect(ui.clip_rect()))
            .extend(shapes);
        // El número: se arrastra en todo el tramo o se escribe con un clic.
        let inner = text_rect(fill_left + VALUE_LEFT, rect.right() - 1.0);
        let (response, zone) = bar_fields::drag_number(
            ui,
            rect,
            inner,
            ("pixel_channel", i),
            &mut values[i],
            0..=255,
            0.5,
            look,
        );
        let enabled = ui.is_enabled();
        // La zona del tramo lleva el nombre del canal.
        zone.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, enabled, channel.label));
        response.on_hover_text(channel.label);
        left = rect.right();
        // La línea entre tramos.
        let sep = Rect::from_min_size(pos2(left, strip.top()), vec2(1.0, STRIP_HEIGHT));
        ui.painter().extend(piece_or(skin, SEPARATOR, sep, ppp, || {
            vec![solid(sep, colors.field_text, ppp)]
        }));
        left += 1.0;
    }
    let edited = Rgba::new(values[0], values[1], values[2], values[3]);
    // El hex, en lo que queda (con la punta derecha).
    let hex = Rect::from_min_max(pos2(left, strip.top()), strip.right_bottom()).union(
        Rect::from_min_size(pos2(left, strip.top()), vec2(HEX_MIN, STRIP_HEIGHT)),
    );
    ui.painter().extend(piece_or(skin, HEX, hex, ppp, || {
        flat_segment(hex, colors.highlight, colors, ppp)
    }));
    let text_id = id.with("hex");
    let hex_now = edited.to_hex();
    // Lo guardado mientras el hex tiene el foco. Si no se escribió nada (el texto es el hex que
    // se mostraba) y el color cambió por otro lado (el S/V, el gotero), el texto lo sigue.
    let mut text = match ui.data_mut(|d| d.get_temp::<HexStash>(text_id)) {
        Some(stash) if stash.text != stash.shown => stash.text,
        _ => hex_now.clone(),
    };
    let inner = text_rect(hex.left() + HEX_TEXT_LEFT, hex.right() - 2.0);
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt("pixel_hex")
            .max_rect(inner)
            .layout(Layout::top_down(Align::Min)),
    );
    bar_fields::plain_field_style(
        child.style_mut(),
        inner.size(),
        colors.field_text,
        colors.field_text,
    );
    let response = child.add(
        egui::TextEdit::singleline(&mut text)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_width(inner.width()),
    );
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, enabled, "Hex"));
    if response.has_focus() {
        let stash = HexStash {
            text: text.clone(),
            shown: hex_now,
        };
        ui.data_mut(|d| d.insert_temp(text_id, stash));
    } else {
        ui.data_mut(|d| d.remove::<HexStash>(text_id));
    }
    // Esc cancela: no se aplica lo escrito (y el texto vuelve al hex del color).
    let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let typed = (response.lost_focus() && !cancelled).then_some(text.as_str());
    strip_result(current, edited, typed)
}

/// Lo escrito en el hex mientras tiene el foco y el hex del color que mostraba entonces.
#[derive(Clone)]
struct HexStash {
    text: String,
    shown: String,
}

/// El color que deja la tira: si un canal cambió en este frame, esa edición; si no, el hex
/// escrito (`typed`, al perder el foco) cuando es válido; si no, el color de antes.
fn strip_result(current: Rgba, edited: Rgba, typed: Option<&str>) -> Rgba {
    if edited != current {
        return edited;
    }
    typed.and_then(Rgba::parse_hex).unwrap_or(edited)
}

/// El cuadro de saturación (a lo ancho) y brillo (de abajo arriba), con el cursor.
fn sv_square(ui: &mut Ui, skin: &Skin, id: egui::Id, rect: Rect, hsv: &mut HsvaGamma) {
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| WidgetInfo::labeled(WidgetType::Slider, enabled, "Saturation and value"));
    if let Some(p) = response.interact_pointer_pos() {
        hsv.s = remap_clamp(p.x, rect.left()..=rect.right(), 0.0..=1.0);
        hsv.v = remap_clamp(p.y, rect.bottom()..=rect.top(), 0.0..=1.0);
    }
    let opaque = HsvaGamma { a: 1.0, ..*hsv };
    let mut mesh = Mesh::default();
    for yi in 0..=STEPS {
        for xi in 0..=STEPS {
            let (xt, yt) = (xi as f32 / STEPS as f32, yi as f32 / STEPS as f32);
            let color = HsvaGamma {
                s: xt,
                v: yt,
                ..opaque
            };
            let at = pos2(
                lerp(rect.left()..=rect.right(), xt),
                lerp(rect.bottom()..=rect.top(), yt),
            );
            mesh.colored_vertex(at, color.into());
            if xi < STEPS && yi < STEPS {
                let tl = (yi * (STEPS + 1) + xi) as u32;
                let row = (STEPS + 1) as u32;
                mesh.add_triangle(tl, tl + 1, tl + row);
                mesh.add_triangle(tl + 1, tl + row, tl + row + 1);
            }
        }
    }
    ui.painter().add(Shape::mesh(mesh));
    // El cursor centrado en el píxel del S/V, recortado al cuadro.
    let center = pos2(
        rect.left() + (hsv.s * (rect.width() - 1.0)).round() + 0.5,
        rect.top() + ((1.0 - hsv.v) * (rect.height() - 1.0)).round() + 0.5,
    );
    let cursor = Rect::from_center_size(center, Vec2::splat(CURSOR_SIZE));
    let ppp = ui.ctx().pixels_per_point();
    let shapes = piece_or(skin, CURSOR, cursor, ppp, || ring(center, CURSOR_SIZE));
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .extend(shapes);
}

/// Un cursor de respaldo (sin la pieza): anillo blanco con borde oscuro.
fn ring(center: Pos2, size: f32) -> Vec<Shape> {
    let r = size / 2.0 - 1.0;
    vec![
        Shape::circle_stroke(center, r, Stroke::new(1.0, Color32::BLACK)),
        Shape::circle_stroke(center, r - 1.0, Stroke::new(1.0, Color32::WHITE)),
    ]
}

/// Dónde va el cursor de una barra para `t` (0..1), y qué `t` da el puntero en `x`: de 1 px de la
/// punta izquierda a 1 px de la derecha.
fn bar_travel(rect: Rect) -> std::ops::RangeInclusive<f32> {
    let half = BAR_CURSOR_SIZE / 2.0;
    let start = rect.left() + BAR_CURSOR_MARGIN + half;
    start..=(rect.right() - BAR_CURSOR_MARGIN - half)
}

/// Una barra de `t` (0..1) con su degradé (`color_at`) y el cursor. Devuelve si se movió.
fn bar(
    ui: &mut Ui,
    skin: &Skin,
    id: egui::Id,
    rect: Rect,
    label: &str,
    t: &mut f32,
    color_at: impl Fn(f32) -> Color32,
) {
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, enabled, label));
    let travel = bar_travel(rect);
    if let Some(p) = response.interact_pointer_pos() {
        *t = remap_clamp(p.x, travel.clone(), 0.0..=1.0);
    }
    let mut mesh = Mesh::default();
    for i in 0..=STEPS {
        let s = i as f32 / STEPS as f32;
        let x = lerp(rect.left()..=rect.right(), s);
        let color = color_at(s);
        mesh.colored_vertex(pos2(x, rect.top()), color);
        mesh.colored_vertex(pos2(x, rect.bottom()), color);
        if i < STEPS {
            let k = (2 * i) as u32;
            mesh.add_triangle(k, k + 1, k + 2);
            mesh.add_triangle(k + 1, k + 2, k + 3);
        }
    }
    ui.painter().add(Shape::mesh(mesh));
    let x = (lerp(travel, *t) - BAR_CURSOR_SIZE / 2.0).round();
    let cursor = Rect::from_min_size(
        pos2(x, rect.top() + BAR_CURSOR_TOP),
        Vec2::splat(BAR_CURSOR_SIZE),
    );
    let ppp = ui.ctx().pixels_per_point();
    ui.painter()
        .extend(piece_or(skin, BAR_CURSOR, cursor, ppp, || {
            ring(cursor.center(), BAR_CURSOR_SIZE)
        }));
}

fn hue_bar(ui: &mut Ui, skin: &Skin, id: egui::Id, rect: Rect, hsv: &mut HsvaGamma) {
    bar(ui, skin, id, rect, "Hue", &mut hsv.h, |h| {
        HsvaGamma {
            h,
            s: 1.0,
            v: 1.0,
            a: 1.0,
        }
        .into()
    });
}

/// El alfa sobre el damero gris, del transparente al color opaco.
fn alpha_bar(ui: &mut Ui, skin: &Skin, id: egui::Id, rect: Rect, hsv: &mut HsvaGamma) {
    let ppp = ui.ctx().pixels_per_point();
    let mut cells = Vec::new();
    let mut y = rect.top();
    let mut row = 0;
    while y < rect.bottom() {
        let mut x = rect.left();
        let mut col = 0;
        while x < rect.right() {
            let cell = Rect::from_min_max(
                pos2(x, y),
                pos2(
                    (x + ALPHA_CELL.x).min(rect.right()),
                    (y + ALPHA_CELL.y).min(rect.bottom()),
                ),
            );
            let color = if (row + col) % 2 == 0 {
                ALPHA_DARK
            } else {
                ALPHA_LIGHT
            };
            cells.push(solid(cell, color, ppp));
            x += ALPHA_CELL.x;
            col += 1;
        }
        y += ALPHA_CELL.y;
        row += 1;
    }
    ui.painter().extend(cells);
    let opaque = HsvaGamma { a: 1.0, ..*hsv };
    bar(ui, skin, id, rect, "Alpha", &mut hsv.a, |a| {
        HsvaGamma { a, ..opaque }.into()
    });
}

#[cfg(test)]
mod tests;
