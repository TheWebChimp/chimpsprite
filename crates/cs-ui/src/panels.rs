use eframe::egui::{
    self, Color32, ColorImage, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use std::sync::Arc;

use cs_core::{ArtboardId, Palette, PixelValue, SymmetryAxes, Tile};

use crate::canvas::CanvasView;
use crate::pixel_sections::{self, HeaderAction};
use crate::pixel_ui::Tip as _;
use cs_raster::RotationAlgorithm;
use cs_tools::text::FontChoice;
use cs_tools::{BrushShape, Dither, GradientShape};

use crate::bar_fields::{self, BarLook};
use crate::editor::{Editor, Ink, MAX_BRUSH_SIZE, SelectionMode, ToolKind};
use crate::skin::Skin;
use crate::{color_panel, fg_bg, menus, pixel_ui, theme, tile_ops, tool_group, widgets};

mod options_bar;
pub use options_bar::*;

mod inspectors;
pub use inspectors::*;

/// Fila de un grupo en la barra con nombres: el botón con la herramienta que muestra el grupo y,
/// si hay más, un desplegable con las demás (también con clic derecho o manteniendo presionado),
/// como en Aseprite. Devuelve la respuesta del botón.
fn tool_group(ui: &mut egui::Ui, editor: &mut Editor, index: usize) -> egui::Response {
    let group = ToolKind::GROUPS[index];
    let more = group.len() > 1;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let width = ui.available_width() - if more { 22.0 } else { 0.0 };
        let response = tool_group::group_button(ui, editor, None, index, |ui, shown, state| {
            let label = format!("{} {}", tool_group::emoji(shown), shown.label());
            let button = egui::Button::selectable(state.selected, label)
                .shortcut_text("")
                .truncate();
            ui.add_sized([width, 24.0], button)
        });
        if more {
            let current = editor.tool;
            let mut pick = None;
            ui.menu_button("⏷", |ui| {
                pick = tool_group::variants_list(ui, None, current, group);
            })
            .response
            .tip("More tools (or right-click, or hold)");
            if let Some(kind) = pick {
                editor.set_tool(kind);
            }
        }
        response
    })
    .inner
}

/// Scroll del panel de herramientas: solo con la rueda y la barra. Arrastrar (también con el
/// dedo) no lo mueve, así mantener presionado un botón de grupo abre sus variantes.
pub fn tools_scroll_area() -> egui::ScrollArea {
    use egui::containers::scroll_area::ScrollSource;
    egui::ScrollArea::vertical()
        .id_salt("tools_scroll")
        .auto_shrink([false, false])
        .scroll_source(ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL)
}

/// La barra de íconos de Pixel UI: una columna con un botón por grupo dentro de la caja
/// redondeada (`toolbar.box`), alineada a la izquierda. Los marcos van pegados, uno debajo del
/// otro: el primero redondeado arriba, el último abajo y los del medio cuadrados. Cada botón
/// lleva un id propio de su grupo, así su lista de variantes (y la presión larga) no depende de
/// lo que haya antes en la barra.
///
/// Debajo, la caja de los colores primario y secundario (`fg_bg`).
fn tool_column(ui: &mut egui::Ui, editor: &mut Editor, skin: &Skin) {
    let base = ui.id();
    let len = ToolKind::GROUPS.len();
    pixel_ui::toolbar_box(ui, skin, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for index in 0..len {
            let id = base.with(("tool_group", index));
            tool_group::group_button(ui, editor, Some(skin), index, |ui, shown, state| {
                pixel_ui::tool_button(
                    ui,
                    id,
                    skin,
                    pixel_ui::SlotPos::of(index, len),
                    tool_group::icon_piece(shown),
                    shown.label(),
                    state.selected,
                    state.more,
                )
            });
        }
    });
    ui.add_space(fg_bg::GAP - ui.spacing().item_spacing.y);
    fg_bg::fg_bg(ui, editor, skin);
}

/// La barra de herramientas: solo los botones de grupo. Con `skin` (Pixel UI) es una columna de
/// íconos; sin él, filas con nombre bajo el título "Tools". Las opciones de la herramienta y
/// Symmetry van en la barra de opciones (`tool_options_bar`).
pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>) {
    match skin {
        Some(skin) => tool_column(ui, editor, skin),
        None => {
            ui.add_space(6.0);
            ui.heading("Tools");
            ui.separator();
            ui.spacing_mut().item_spacing.y = 3.0;
            for index in 0..ToolKind::GROUPS.len() {
                tool_group(ui, editor, index);
            }
        }
    }
}

/// Damero mínimo para que se note la transparencia.
pub(crate) fn checker(painter: &egui::Painter, rect: Rect) {
    let half = rect.size() / 2.0;
    painter.rect_filled(rect, 0.0, theme::CHECKER_LIGHT);
    painter.rect_filled(
        Rect::from_min_size(rect.min, half),
        0.0,
        theme::CHECKER_DARK,
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min + half, half),
        0.0,
        theme::CHECKER_DARK,
    );
}

fn swatch(ui: &mut egui::Ui, color: Color32, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    if color.a() < 255 {
        checker(painter, rect);
    }
    painter.rect_filled(rect, 0.0, color);
    response
}

/// Marco blanco si `value` es el color primario; del color de acento si es el secundario. Con
/// Pixel UI (`frame_skin`), de 1 px, como en el diseño; si no, de 2.
pub(crate) fn mark_selected(ui: &egui::Ui, editor: &Editor, value: PixelValue, rect: Rect) {
    let color = if value == editor.primary {
        Color32::WHITE
    } else if value == editor.secondary {
        theme::ACCENT
    } else {
        return;
    };
    let width = if crate::pixel_ui::frame_skin(ui.ctx()).is_some() {
        1.0
    } else {
        2.0
    };
    ui.painter()
        .rect_stroke(rect, 0.0, Stroke::new(width, color), StrokeKind::Inside);
}

/// Clic elige `value` como color primario; clic derecho, como secundario (`Editor::pick_color`).
/// `entry`: el cuadrito de la paleta, si `value` sale de uno.
pub(crate) fn pick_on_click(
    editor: &mut Editor,
    response: &egui::Response,
    value: PixelValue,
    entry: Option<u8>,
) {
    if response.clicked() {
        editor.pick_color(value, false, entry);
    }
    if response.secondary_clicked() {
        editor.pick_color(value, true, entry);
    }
}

/// Ayuda del swatch del color Mask.
pub(crate) const MASK_HINT: &str = "Mask (transparent): painting with it erases";

/// Swatch del color Mask (transparente): tablero con una diagonal roja.
fn mask_swatch(ui: &mut egui::Ui, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    paint_mask(ui.painter(), rect);
    response
}

pub(crate) fn paint_mask(painter: &egui::Painter, rect: Rect) {
    checker(painter, rect);
    painter.line_segment(
        [rect.left_bottom(), rect.right_top()],
        Stroke::new(2.0, Color32::from_rgb(220, 40, 40)),
    );
}

/// Swatch del color primario o secundario; el Mask se dibuja como tal.
fn color_swatch(
    ui: &mut egui::Ui,
    editor: &Editor,
    value: PixelValue,
    size: f32,
) -> egui::Response {
    if editor.is_mask(value) {
        mask_swatch(ui, size)
    } else {
        swatch(ui, to_color32(editor.resolve(value)), size)
    }
}

pub(crate) fn to_color32(c: cs_core::Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Lo que se pidió desde el encabezado del panel de paleta (lo resuelve `ChimpApp`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    OpenPresets,
    Load,
    Save,
    SaveAsPreset,
    SetAsDefault,
    LoadDefault,
    /// Una paleta nueva con el color primario (Pixel UI). En indexado se pide confirmación.
    NewPalette,
    /// Solo escritorio: en web no hay carpeta que abrir.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    OpenFolder,
}

/// Tira de colores de una paleta: como mucho `max` y termina en "…" si hay más.
pub fn palette_strip(ui: &mut egui::Ui, palette: &Palette, max: usize, size: f32) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for &c in palette.colors().iter().take(max) {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, to_color32(c));
        }
        if palette.len() > max {
            ui.weak("≡");
        }
    });
}

/// `menus`: con un diálogo abierto, Presets ⏷ y … quedan desactivados. Con Pixel UI (`skin`), la
/// grilla de la paleta y el editor de color son los del diseño (`color_panel`), con la caja de
/// `palette_height` de alto (lo que se elige con la división). Van fuera del scroll del panel: el
/// editor se ve siempre entero.
pub fn palette(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    menus: bool,
    skin: Option<&Skin>,
    palette_height: &mut f32,
) -> Option<PaletteAction> {
    if let Some(skin) = skin {
        // Con Pixel UI, los colores primario y secundario (y el intercambio) van en la caja de
        // debajo de la barra de íconos (`fg_bg`); los botones del diseño reemplazan al combo
        // Presets y al menú "…": todo lo de los dos queda en el menú del botón ≡.
        ui.add_space(6.0);
        let action = color_panel::palette_buttons(ui, editor, skin, menus);
        ui.add_space(color_panel::BUTTONS_GAP - ui.spacing().item_spacing.y);
        color_panel::palette_and_editor(ui, editor, skin, palette_height);
        return action;
    }
    ui.add_space(6.0);
    ui.heading("Color");
    ui.separator();
    ui.horizontal(|ui| {
        let name = |v| {
            if editor.is_mask(v) {
                " · Mask (transparent)"
            } else {
                ""
            }
        };
        color_swatch(ui, editor, editor.primary, 36.0)
            .tip(format!("Primary (left click){}", name(editor.primary)));
        color_swatch(ui, editor, editor.secondary, 24.0)
            .tip(format!("Secondary (right click){}", name(editor.secondary)));
        if ui.small_button("🔄").tip("Swap (X)").clicked() {
            editor.swap_colors();
        }
    });
    ui.add_space(8.0);
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label("Palette");
        ui.add_enabled_ui(menus, |ui| {
            if ui.small_button("Presets ⏷").clicked() {
                action = Some(PaletteAction::OpenPresets);
            }
            ui.menu_button("≡", |ui| {
                let mut item = |ui: &mut egui::Ui, label: &str, a: PaletteAction| {
                    if ui.button(label).clicked() {
                        action = Some(a);
                        ui.close();
                    }
                };
                item(ui, "Load Palette", PaletteAction::Load);
                item(ui, "Save Palette", PaletteAction::Save);
                item(ui, "Save as Preset", PaletteAction::SaveAsPreset);
                ui.separator();
                item(ui, "Set as Default", PaletteAction::SetAsDefault);
                item(ui, "Load Default", PaletteAction::LoadDefault);
                #[cfg(not(target_arch = "wasm32"))]
                {
                    ui.separator();
                    item(ui, "Open Folder", PaletteAction::OpenFolder);
                }
            });
        });
    });
    let size = 20.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        let mask = editor.mask_value();
        let response = mask_swatch(ui, size).tip(MASK_HINT);
        mark_selected(ui, editor, mask, response.rect);
        pick_on_click(editor, &response, mask, None);
        let count = editor.sprite().palette.len().min(256);
        for i in 0..count {
            let index = i as u8;
            let value = editor.palette_value(index);
            let color = to_color32(editor.sprite().palette.colors()[i]);
            let response = swatch(ui, color, size).tip(format!("#{i}"));
            mark_selected(ui, editor, value, response.rect);
            pick_on_click(editor, &response, value, Some(index));
        }
    });
    ui.add_space(10.0);
    color_picker(ui, editor, None);
    action
}

/// El selector de color con un sprite indexado (y la paleta bloqueada, con Pixel UI).
pub(crate) const INDEXED_HINT: &str = "Indexed sprite: colors come from the palette.";

/// Lado máximo del cuadro del selector de color, en puntos. Con Pixel UI, el ancho máximo del
/// editor (su cuadro S/V mide 64 de alto).
pub(crate) const COLOR_PICKER_MAX: f32 = 240.0;

/// Selector de color libre (RGBA): edita el color primario. Con Pixel UI (`skin`), el editor del
/// diseño (`color_panel`).
pub fn color_picker(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>) {
    if let Some(skin) = skin {
        color_panel::color_editor(ui, editor, skin);
        return;
    }
    let cs_core::PixelValue::Rgba(current) = editor.primary else {
        ui.weak(INDEXED_HINT);
        return;
    };
    let mut c32 = to_color32(current);
    // El cuadro de saturación/brillo ocupa el ancho del panel hasta `COLOR_PICKER_MAX` (es
    // cuadrado: con un sidebar ancho se haría enorme). Se restaura después: si no, los sliders de
    // los paneles siguientes heredan ese ancho y agrandan el panel en cada frame.
    let slider_width = ui.spacing().slider_width;
    ui.spacing_mut().slider_width = (ui.available_width() - 8.0).clamp(120.0, COLOR_PICKER_MAX);
    let picked = egui::color_picker::color_picker_color32(
        ui,
        &mut c32,
        egui::color_picker::Alpha::OnlyBlend,
    );
    ui.spacing_mut().slider_width = slider_width;
    if picked {
        let [r, g, b, a] = c32.to_srgba_unmultiplied();
        editor.primary = cs_core::PixelValue::Rgba(cs_core::Rgba::new(r, g, b, a));
    }
    ui.horizontal(|ui| {
        let id = ui.id().with("hex");
        let mut text = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| current.to_hex());
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .desired_width(90.0)
                .font(egui::TextStyle::Monospace),
        );
        if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
        if response.lost_focus()
            && let Some(c) = cs_core::Rgba::parse_hex(&text)
        {
            editor.primary = cs_core::PixelValue::Rgba(c);
        }
        let in_palette = editor.sprite().palette.colors().contains(&current);
        if ui
            .add_enabled(!in_palette, egui::Button::new("+").small())
            .tip("Add to palette")
            .clicked()
        {
            editor.add_to_palette(current);
        }
    });
}

/// Lado de las miniaturas del panel Tileset.
const THUMB: f32 = 32.0;

/// Identidad de las miniaturas: artboard, tileset, tamaño de tile, el `Arc` de cada tile (pintar
/// celdas no los cambia; reemplazar la imagen de un tile sí) y la paleta (los indexados se
/// resuelven con ella).
#[derive(PartialEq)]
struct ThumbKey {
    artboard: ArtboardId,
    index: u32,
    tile_size: (u32, u32),
    tiles: Vec<usize>,
    palette: cs_core::Palette,
}

/// Miniaturas de los tiles del tileset activo, cacheadas por la identidad del tileset (no por la
/// versión del documento: cada trazo de tiles la cambia).
#[derive(Default)]
pub struct TileThumbs {
    key: Option<ThumbKey>,
    /// Una por tile (la 0 es el vacío y no se dibuja).
    textures: Vec<TextureHandle>,
}

impl TileThumbs {
    fn refresh(&mut self, ctx: &egui::Context, editor: &Editor) {
        let Some((index, ts)) = editor.active_tileset() else {
            self.key = None;
            self.textures.clear();
            return;
        };
        let sprite = editor.sprite();
        let key = ThumbKey {
            artboard: editor.active,
            index,
            tile_size: ts.tile_size,
            tiles: ts.tiles.iter().map(|t| Arc::as_ptr(t) as usize).collect(),
            palette: sprite.palette.clone(),
        };
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.textures = ts
            .tiles
            .iter()
            .enumerate()
            .map(|(i, tile)| {
                let bytes: Vec<u8> = (0..tile.height())
                    .flat_map(|y| (0..tile.width()).map(move |x| (x, y)))
                    .flat_map(|(x, y)| {
                        let c = tile.get(x, y).map_or(cs_core::Rgba::TRANSPARENT, |v| {
                            cs_render::resolve(sprite, v)
                        });
                        [c.r, c.g, c.b, c.a]
                    })
                    .collect();
                let size = [tile.width() as usize, tile.height() as usize];
                let image = ColorImage::from_rgba_unmultiplied(size, &bytes);
                let name = format!("tile-{}-{index}-{i}", editor.active.0);
                ctx.load_texture(name, image, TextureOptions::NEAREST)
            })
            .collect();
        self.key = Some(key);
    }
}

/// Panel "Tileset" (reemplaza a la paleta con una capa tilemap activa): elegir el tile del pincel
/// y sus flips.
pub fn tileset(ui: &mut egui::Ui, editor: &mut Editor, thumbs: &mut TileThumbs) {
    thumbs.refresh(ui.ctx(), editor);
    let Some((_, ts)) = editor.active_tileset() else {
        return;
    };
    let (name, (tw, th), count, base) =
        (ts.name.clone(), ts.tile_size, ts.tiles.len(), ts.base_index);
    ui.add_space(6.0);
    ui.heading("Tileset");
    ui.separator();
    ui.label(format!(
        "{name}  {tw}×{th}  {} tiles",
        count.saturating_sub(1)
    ));
    let brush = editor.brush_tile();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        for i in 0..count {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(THUMB + 4.0), Sense::click());
            let inner = rect.shrink(2.0);
            let painter = ui.painter();
            checker(painter, inner);
            if i == 0 {
                // El vacío: el dibujo de "transparente".
                painter.line_segment(
                    [inner.left_bottom(), inner.right_top()],
                    Stroke::new(1.5, Color32::from_rgb(220, 60, 60)),
                );
            } else if let Some(tex) = thumbs.textures.get(i) {
                let scale = THUMB / tw.max(th) as f32;
                let size = Vec2::new(tw as f32 * scale, th as f32 * scale);
                let uv = Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0));
                painter.image(
                    tex.id(),
                    Rect::from_center_size(inner.center(), size),
                    uv,
                    Color32::WHITE,
                );
            }
            let number = tile_ops::tile_number(i as u32, base);
            widgets::outlined_text(
                painter,
                inner.left_top() + Vec2::new(1.0, 0.0),
                egui::Align2::LEFT_TOP,
                &number.to_string(),
                egui::FontId::monospace(9.0),
            );
            if brush.index() as usize == i {
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(2.0, Color32::WHITE),
                    StrokeKind::Inside,
                );
            }
            if response.tip(format!("Tile {number}")).clicked() {
                editor.set_brush_tile(Tile::new(
                    i as u32,
                    brush.flip_x(),
                    brush.flip_y(),
                    brush.flip_d(),
                ));
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Flip:");
        let brush = editor.brush_tile();
        if ui
            .selectable_label(brush.flip_x(), "X")
            .tip("Flip horizontally (X)")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_X);
        }
        if ui
            .selectable_label(brush.flip_y(), "Y")
            .tip("Flip vertically (Y)")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_Y);
        }
        if ui
            .add_enabled(tw == th, egui::Button::selectable(brush.flip_d(), "D"))
            .tip("Flip diagonally (swap X and Y)")
            .on_disabled_hover_text("Needs square tiles")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_D);
        }
    });
}

/// `notice`: aviso breve al final de la barra (p. ej. autosave desactivado).
pub fn status_bar(ui: &mut egui::Ui, editor: &Editor, canvas: &CanvasView, notice: Option<&str>) {
    ui.horizontal(|ui| {
        let s = editor.sprite();
        ui.label(format!(
            "{}  {}×{}",
            editor.artboard().name,
            s.width,
            s.height
        ));
        ui.separator();
        match canvas.hover {
            Some((x, y)) if x >= 0 && y >= 0 && x < s.width as i32 && y < s.height as i32 => {
                ui.label(format!("{x}, {y}"));
            }
            _ => {
                ui.label("—");
            }
        }
        ui.separator();
        ui.label(format!("{}%", (canvas.zoom * 100.0) as u32));
        ui.separator();
        ui.label(format!("Frame {}/{}", editor.frame + 1, s.frames.len()));
        for (text, guide) in status_notices(editor, canvas, notice) {
            ui.separator();
            let color = if guide {
                theme::GUIDE_LINE
            } else {
                theme::ACCENT
            };
            ui.colored_label(color, text);
        }
    });
}

/// Los avisos de la barra de estado en este momento (las dos barras muestran los mismos):
/// máscara, instancia, tiles, lo flotante, la guía bajo el puntero (`true`: se pinta en el color
/// de las guías) y `notice` (autosave).
fn status_notices(
    editor: &Editor,
    canvas: &CanvasView,
    notice: Option<&str>,
) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    if editor.editing_mask() {
        out.push((
            "Editing layer mask: white reveals, black hides".to_owned(),
            false,
        ));
    }
    if let Some(text) = editor.mask_status() {
        out.push((text.to_string(), false));
    }
    if editor.on_object_layer() && editor.active_tool().paints() {
        out.push((
            "Object layer: edit the symbol (double-click)".to_owned(),
            false,
        ));
    }
    if let Some(text) = editor.tiles_status() {
        out.push((text.to_string(), false));
    }
    if let Some(text) = editor.floating_status() {
        out.push((text, false));
    }
    if let Some(hint) = canvas.guide_hint {
        out.push((hint.to_string(), true));
    }
    if let Some(notice) = notice {
        out.push((notice.to_owned(), false));
    }
    out
}

/// Con Pixel UI, la barra de estado del diseño: el artboard con su tamaño, el píxel bajo el
/// puntero y el frame, cada uno con su ícono; después, sin ícono, el zoom y los avisos de ese
/// momento (máscara, instancia, tiles, lo flotante, guías, autosave).
pub fn pixel_status_bar(
    ui: &mut egui::Ui,
    editor: &Editor,
    canvas: &CanvasView,
    notice: Option<&str>,
    skin: &Skin,
) {
    use crate::pixel_status::{self, ICON_ARTBOARD, ICON_FRAME, ICON_POINTER, Section};
    let s = editor.sprite();
    let pointer = match canvas.hover {
        Some((x, y)) if x >= 0 && y >= 0 && x < s.width as i32 && y < s.height as i32 => {
            format!("{x} {y}")
        }
        // Fuera del lienzo, el guion de la fuente chica (el "—" no está en ella y sale de otra).
        _ => "-".to_owned(),
    };
    let mut sections = vec![
        Section {
            icon: Some(ICON_ARTBOARD),
            text: format!("{} {}\u{d7}{}", editor.artboard().name, s.width, s.height),
            color: None,
            min_text: None,
        },
        Section {
            icon: Some(ICON_POINTER),
            text: pointer,
            color: None,
            // De ancho fijo: las coordenadas cambian con cada movimiento.
            min_text: Some("9999 9999"),
        },
        Section {
            icon: Some(ICON_FRAME),
            text: format!("{}/{}", editor.frame + 1, s.frames.len()),
            color: None,
            min_text: None,
        },
        Section {
            icon: None,
            text: format!("{}%", (canvas.zoom * 100.0) as u32),
            color: None,
            min_text: None,
        },
    ];
    // Los avisos, sin ícono y en el acento del skin.
    let accent = skin.colors().accent;
    sections.extend(
        status_notices(editor, canvas, notice)
            .into_iter()
            .map(|(text, _)| Section {
                icon: None,
                text,
                color: Some(accent),
                min_text: None,
            }),
    );
    pixel_status::show(ui, skin, &sections);
}

#[cfg(test)]
mod tests;
