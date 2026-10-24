//! Con Pixel UI, la caja de los colores primario y secundario debajo de la barra de íconos, como
//! en el diseño (`Design.aseprite`, frame 4, capa Mockup: x 33..59, y 425..473). La caja de
//! colores de arriba de esa (y 393..418, con las flechas curvas) no se usa.
//!
//! - `fgbg.box` (27×49): la caja con la línea grabada; sus dos mitades son transparentes. Arriba
//!   el primario y abajo el secundario, cada uno en 19×19 a 4 px del borde (y la del secundario
//!   22 filas más abajo). Con alfa, el color va sobre el damero `fgbg.checker` (8×8, en mosaico
//!   desde la esquina de la mitad); el Mask (transparente) es el damero solo.
//! - `fgbg.swap` (9×9): el botón redondo al medio, sobre la línea que separa las mitades:
//!   intercambia los colores (como la X).
//! - Un clic en una mitad elige qué color edita el editor de color del panel derecho
//!   (`Editor::editing_secondary`). Si es el secundario, su mitad lleva un borde de 1 px en
//!   `accent` (el diseño no lo tiene: muestra el primario, que es lo normal).

use eframe::egui::{
    Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetType, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::editor::Editor;
use crate::menus::solid;
use crate::panels;
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};

/// Filas entre la caja de la barra de íconos y esta: en el diseño, de la caja de la barra (que
/// termina en la y 387) a la caja de debajo (y 392).
pub const GAP: f32 = 4.0;

/// Los nombres accesibles de la caja: las dos mitades y el botón redondo.
pub(crate) const LABELS: [&str; 3] = ["Primary color", "Secondary color", "Swap colors"];

const BOX: &str = "fgbg.box";
const SWAP: &str = "fgbg.swap";
const CHECKER: &str = "fgbg.checker";

/// La caja del diseño, la de respaldo sin la pieza.
const BOX_SIZE: Vec2 = Vec2::new(27.0, 49.0);
/// Cada mitad: el color en 19×19, a (4,4) y (4,26) de la caja.
const COLOR_SIDE: f32 = 19.0;
const PRIMARY_AT: Vec2 = Vec2::new(4.0, 4.0);
const SECONDARY_AT: Vec2 = Vec2::new(4.0, 26.0);
/// Lo que se puede apretar de cada mitad: lo de adentro de la línea (x 36..56), 21×21.
const HALF_SIDE: f32 = 21.0;
const HALF_INSET: f32 = 1.0;
/// El botón redondo (x 42..50, y 445..453).
const SWAP_AT: Vec2 = Vec2::new(9.0, 20.0);
const SWAP_SIZE: Vec2 = Vec2::new(9.0, 9.0);
/// El período del damero del diseño.
const CHECKER_TILE: f32 = 8.0;

/// La caja de los colores, alineada a la izquierda como la caja de la barra.
pub fn fg_bg(ui: &mut Ui, editor: &mut Editor, skin: &Skin) {
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let size = skin
        .piece(BOX, WidgetState::Normal)
        .map_or(BOX_SIZE, |p| vec2(p.size[0] as f32, p.size[1] as f32));
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), size);
    let id = ui.id().with("fg_bg");
    let enabled = ui.is_enabled();

    let primary = Rect::from_min_size(rect.min + PRIMARY_AT, Vec2::splat(COLOR_SIDE));
    let secondary = Rect::from_min_size(rect.min + SECONDARY_AT, Vec2::splat(COLOR_SIDE));
    for (half, value) in [(primary, editor.primary), (secondary, editor.secondary)] {
        let color = panels::to_color32(editor.resolve(value));
        let transparent = editor.is_mask(value) || color.a() == 0;
        // Con alfa, el damero detrás (las tiras de la orilla, recortadas a la mitad).
        if transparent || color.a() < 255 {
            ui.painter()
                .with_clip_rect(half.intersect(ui.clip_rect()))
                .extend(checker(skin, half, ppp));
        }
        if !transparent {
            ui.painter().add(solid(half, color, ppp));
        }
    }
    let mut shapes = Vec::new();
    shapes.extend(match skin.piece(BOX, WidgetState::Normal) {
        Some(p) => pixel_ui::piece_shapes(p, rect, ppp),
        None => vec![
            Shape::rect_stroke(
                primary.expand(1.0).round_to_pixels(ppp),
                0.0,
                Stroke::new(1.0, colors.outline),
                StrokeKind::Outside,
            ),
            Shape::rect_stroke(
                secondary.expand(1.0).round_to_pixels(ppp),
                0.0,
                Stroke::new(1.0, colors.outline),
                StrokeKind::Outside,
            ),
        ],
    });
    if editor.editing_secondary {
        let r = secondary;
        for line in [
            Rect::from_min_size(r.min, vec2(r.width(), 1.0)),
            Rect::from_min_size(pos2(r.left(), r.bottom() - 1.0), vec2(r.width(), 1.0)),
            Rect::from_min_size(pos2(r.left(), r.top() + 1.0), vec2(1.0, r.height() - 2.0)),
            Rect::from_min_size(
                pos2(r.right() - 1.0, r.top() + 1.0),
                vec2(1.0, r.height() - 2.0),
            ),
        ] {
            shapes.push(solid(line, colors.accent, ppp));
        }
    }
    let swap = Rect::from_min_size(rect.min + SWAP_AT, SWAP_SIZE);
    shapes.extend(match skin.piece(SWAP, WidgetState::Normal) {
        Some(p) => pixel_ui::piece_shapes(p, swap, ppp),
        None => vec![
            Shape::circle_filled(swap.center(), SWAP_SIZE.x / 2.0, colors.panel),
            Shape::circle_stroke(
                swap.center(),
                SWAP_SIZE.x / 2.0 - 0.5,
                Stroke::new(1.0, colors.outline),
            ),
        ],
    });
    ui.painter().extend(shapes);

    // Las mitades y, encima, el botón redondo.
    let half_at = |at: Vec2| {
        Rect::from_min_size(
            rect.min + at - Vec2::splat(HALF_INSET),
            Vec2::splat(HALF_SIDE),
        )
    };
    let name = |editor: &Editor, v| {
        if editor.is_mask(v) {
            " · Mask (transparent)"
        } else {
            ""
        }
    };
    for (k, at, label, tip) in [
        (0, PRIMARY_AT, LABELS[0], "Primary (left click)"),
        (1, SECONDARY_AT, LABELS[1], "Secondary (right click)"),
    ] {
        let response = ui.interact(half_at(at), id.with(k), Sense::click());
        let editing = editor.editing_secondary == (k == 1);
        response.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, editing, label));
        let value = if k == 0 {
            editor.primary
        } else {
            editor.secondary
        };
        let tip = format!("{tip}{}. Click to edit it", name(editor, value));
        if response.on_hover_text(tip).clicked() {
            editor.editing_secondary = k == 1;
        }
    }
    let response = ui.interact(swap, id.with("swap"), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, LABELS[2]));
    if response.on_hover_text("Swap (X)").clicked() {
        editor.swap_colors();
    }
}

/// El damero de `half`: la pieza en mosaico desde su esquina (quien lo pinta lo recorta a la
/// mitad), o el damero de los cuadritos sin la pieza.
fn checker(skin: &Skin, half: Rect, ppp: f32) -> Vec<Shape> {
    let Some(piece) = skin.piece(CHECKER, WidgetState::Normal) else {
        let light = crate::theme::CHECKER_LIGHT;
        let dark = crate::theme::CHECKER_DARK;
        let side = half.size() / 2.0;
        return vec![
            solid(half, light, ppp),
            solid(Rect::from_min_size(half.min, side), dark, ppp),
            solid(Rect::from_min_size(half.min + side, side), dark, ppp),
        ];
    };
    let mut tiles = Vec::new();
    let mut y = half.top();
    while y < half.bottom() {
        let mut x = half.left();
        while x < half.right() {
            let tile = Rect::from_min_size(pos2(x, y), Vec2::splat(CHECKER_TILE));
            tiles.extend(pixel_ui::piece_shapes(piece, tile, ppp));
            x += CHECKER_TILE;
        }
        y += CHECKER_TILE;
    }
    tiles
}
