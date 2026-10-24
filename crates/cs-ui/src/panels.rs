use eframe::egui::{
    self, Color32, ColorImage, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use std::sync::Arc;

use cs_core::{ArtboardId, Palette, PixelValue, SymmetryAxes, Tile};

use crate::canvas::CanvasView;
use cs_raster::RotationAlgorithm;
use cs_tools::text::FontChoice;
use cs_tools::{BrushShape, Dither, GradientShape};

use crate::bar_fields::{self, BarLook};
use crate::editor::{Editor, MAX_BRUSH_SIZE, SelectionMode, ToolKind};
use crate::skin::Skin;
use crate::{menus, pixel_ui, theme, tile_ops, tool_group, widgets};

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
            .on_hover_text("More tools (or right-click, or hold)");
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

/// Alto de la barra de opciones, con su margen: el mismo con todas las herramientas, así el lienzo
/// no se mueve al cambiar de herramienta.
pub const OPTIONS_BAR_HEIGHT: f32 = 32.0;
/// Alto de la barra de opciones con Pixel UI: la del mockup (frame 4), de 21 px de skin. Son 19
/// filas de controles y, abajo, el separador grabado (la línea grabada de arriba es la de las
/// pestañas). Los botones de simetría, de 16, van 2 filas abajo del borde y 1 arriba del grabado.
pub const PIXEL_OPTIONS_BAR_HEIGHT: f32 = 21.0;
/// Aire entre un separador grabado de la barra y lo que tiene a cada lado: en el mockup, 5 px
/// entre el brillo y "Symmetry".
const ETCHED_GAP: f32 = 5.0;
/// Con Pixel UI, como en el mockup (frame 4): del texto "Symmetry" (que deja 1 px vacío después
/// de la tinta) al primer botón de simetría, y del último botón al borde derecho de la barra.
/// La tinta termina en la x 903 y el cuadro del botón empieza en la 909 (la pieza, con su
/// columna transparente, en la 908).
const SYMMETRY_LABEL_GAP: f32 = 3.0;
const PIXEL_BAR_RIGHT: i8 = 3;
/// Con Pixel UI, del borde izquierdo de la barra a la tinta del primer texto (en el diseño, el
/// área de la app empieza en la x 28 y "Mode" en la 35).
const PIXEL_BAR_LEFT: i8 = 7;
/// Espacios de la barra con Pixel UI, medidos en el diseño (frame 4), entre lo que tiene cada
/// lado (el texto con su píxel vacío del final; las piezas con sus columnas transparentes):
/// - entre grupos: del combo a "Brush Size" (x 213..226);
/// - entre controles de un grupo: del campo a la casilla (x 320..324);
/// - de una etiqueta a su campo: de "Brush Size" al campo (x 276..277);
/// - de una etiqueta a su primer botón: de "Mode" a la pieza del botón (x 56..59);
/// - entre botones, y de los botones al combo (las piezas, de 15, dejan 4 px entre cuadros).
const GROUP_GAP: f32 = 14.0;
const CONTROL_GAP: f32 = 5.0;
const LABEL_FIELD_GAP: f32 = 2.0;
const LABEL_TOGGLE_GAP: f32 = 4.0;
const TOGGLE_GAP: f32 = 2.0;
/// Alto de los controles de la barra con Pixel UI (sliders, campos, casillas y botones).
const PIXEL_CONTROL_HEIGHT: f32 = 13.0;

/// Alto de la barra de opciones: `PIXEL_OPTIONS_BAR_HEIGHT` con Pixel UI (`skin`) y
/// `OPTIONS_BAR_HEIGHT` sin él.
pub fn options_bar_height(skin: Option<&Skin>) -> f32 {
    if skin.is_some() {
        PIXEL_OPTIONS_BAR_HEIGHT
    } else {
        OPTIONS_BAR_HEIGHT
    }
}

/// El marco del panel de la barra: con Pixel UI, sin margen vertical (los controles de 13 y el
/// texto de la fuente pixel ocupan el alto justo), con `PIXEL_BAR_RIGHT` a la derecha (los
/// botones de simetría casi contra el borde, como en el mockup) y con lugar abajo para el
/// separador grabado; sin él, el de siempre.
pub fn options_bar_frame(style: &egui::Style, skin: Option<&Skin>) -> egui::Frame {
    let frame = egui::Frame::side_top_panel(style);
    match skin {
        Some(_) => frame
            .inner_margin(egui::Margin {
                left: PIXEL_BAR_LEFT,
                right: PIXEL_BAR_RIGHT,
                top: 0,
                bottom: 0,
            })
            .outer_margin(egui::Margin {
                bottom: pixel_ui::ETCHED_SIZE as i8,
                ..Default::default()
            }),
        None => frame,
    }
}

/// El panel de la barra de opciones, a todo el ancho: de `options_bar_height` y con
/// `options_bar_frame`. Con Pixel UI, sin la línea de egui: la reemplaza el separador grabado que
/// dibuja `tool_options_bar`.
pub fn options_bar_panel(style: &egui::Style, skin: Option<&Skin>) -> egui::Panel {
    egui::Panel::top("tool_options")
        .exact_size(options_bar_height(skin))
        .frame(options_bar_frame(style, skin))
        .show_separator_line(skin.is_none())
}

/// Los separadores grabados de la barra con Pixel UI: sus colores y las filas que cubren los
/// verticales (desde el brillo de la línea de las pestañas hasta la línea de abajo, sin tocarla).
#[derive(Clone, Copy)]
struct BarEtch {
    colors: crate::skin::SkinColors,
    rows: egui::Rangef,
}

/// Separador vertical de la barra, antes de Symmetry: con Pixel UI (`etch`), el grabado del
/// mockup con `ETCHED_GAP` de aire a cada lado; sin él, el de egui.
fn bar_separator(ui: &mut egui::Ui, etch: Option<BarEtch>) {
    let Some(etch) = etch else {
        ui.separator();
        return;
    };
    // El layout ya dejó `item_spacing` después del widget anterior y lo vuelve a dejar después de
    // este: se descuenta para que el aire sea justo `ETCHED_GAP` (puede ser negativo).
    let gap = ETCHED_GAP - ui.spacing().item_spacing.x;
    ui.add_space(gap);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(pixel_ui::ETCHED_SIZE, 0.0), Sense::hover());
    ui.add_space(gap);
    // Las filas del grabado pasan por arriba del alto de la fila (y del recorte del scroll): se
    // recorta solo a lo ancho.
    let mut painter = ui.painter().clone();
    painter.set_clip_rect(Rect::from_x_y_ranges(ui.clip_rect().x_range(), etch.rows));
    painter.add(pixel_ui::etched_vline(
        rect.left(),
        etch.rows,
        &etch.colors,
        ui.ctx().pixels_per_point(),
    ));
}

/// Barra de opciones, debajo de las pestañas: las opciones de la herramienta activa en una sola
/// fila y Symmetry a la derecha. Si las opciones no caben, hacen scroll horizontal con la rueda o
/// la barra (sin arrastre); no se parten en dos renglones. Con `skin` (Pixel UI), los controles
/// son de 13 de alto y los botones de simetría usan las piezas del skin.
pub fn tool_options_bar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>) {
    // Los textos no se parten: la fila se alarga y hace scroll.
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    // La rueda de siempre (vertical) mueve la fila de lado, sin Shift.
    ui.style_mut().always_scroll_the_only_direction = true;
    // Con Pixel UI: el panel entero (su recorte, con el margen de abajo que deja
    // `options_bar_frame`) y las filas de los separadores verticales.
    let bar = ui.clip_rect();
    let line = bar.bottom() - pixel_ui::ETCHED_SIZE;
    let etch = skin.map(|skin| BarEtch {
        colors: *skin.colors(),
        rows: egui::Rangef::new(bar.top() - 1.0, line),
    });
    if skin.is_some() {
        let spacing = ui.spacing_mut();
        spacing.interact_size.y = PIXEL_CONTROL_HEIGHT;
        spacing.button_padding.y = 0.0;
        spacing.item_spacing.y = 0.0;
        // La barra de scroll de la fila flota sobre los controles, sin sumar alto.
        spacing.scroll.floating = true;
    }
    // De derecha a izquierda: Symmetry queda pegado al borde y las opciones usan lo que sobra.
    let look = skin.map(|skin| BarLook {
        skin,
        top: bar.top(),
    });
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        symmetry_buttons(ui, editor, look);
        bar_separator(ui, etch);
        options_scroll_area().show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                tool_options(ui, editor, look)
            });
        });
    });
    // El separador grabado de abajo, a todo el ancho, después de los controles: queda encima si
    // alguno se pasa de las 19 filas.
    if let Some(etch) = etch {
        ui.painter().add(pixel_ui::etched_hline(
            bar.x_range(),
            line,
            &etch.colors,
            ui.ctx().pixels_per_point(),
        ));
    }
    text_window(ui.ctx(), editor);
}

/// Scroll de las opciones de la barra: horizontal, solo con la rueda y la barra.
fn options_scroll_area() -> egui::ScrollArea {
    row_scroll_area("tool_options_scroll")
}

/// Scroll horizontal de una fila de controles que no se parte: solo con la rueda y la barra (sin
/// arrastre). Cada fila lleva su propio `id_salt`.
pub fn row_scroll_area(id_salt: &'static str) -> egui::ScrollArea {
    use egui::containers::scroll_area::ScrollSource;
    egui::ScrollArea::horizontal()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .scroll_source(ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL)
}

/// Separadores entre los grupos de la barra de opciones: una línea antes de cada grupo, menos el
/// primero. Con Pixel UI (`pixel`), en vez de la línea, `GROUP_GAP` de aire: el diseño (frame 4)
/// no tiene líneas entre los grupos, solo antes de Symmetry.
struct Groups {
    started: bool,
    pixel: bool,
}

impl Groups {
    fn next(&mut self, ui: &mut egui::Ui) {
        if self.started {
            if self.pixel {
                gap(ui, GROUP_GAP);
            } else {
                ui.separator();
            }
        }
        self.started = true;
    }
}

/// `px` de aire después del último control: el layout ya deja `item_spacing`, así que se
/// descuenta (puede ser negativo).
fn gap(ui: &mut egui::Ui, px: f32) {
    ui.add_space(px - ui.spacing().item_spacing.x);
}

/// Las opciones de la herramienta activa, en fila: cada etiqueta al lado de su control y un
/// separador entre grupos. La ayuda de la herramienta va en el mismo orden que antes. Con `look`
/// (Pixel UI), los controles del diseño (`bar_fields`): textos en la fila del diseño, campos
/// numéricos en vez de sliders, botones de un clic en vez de grupos de radios, combos para las
/// listas (tramado, fuente), casillas del skin, los modos de
/// selección con íconos y aire en vez de separadores; los botones de forma, de 13×13.
fn tool_options(ui: &mut egui::Ui, editor: &mut Editor, look: Option<BarLook>) {
    let tool = editor.tool;
    let mut groups = Groups {
        started: false,
        pixel: look.is_some(),
    };
    let options = &mut editor.options;
    if tool.uses_brush() {
        groups.next(ui);
        let size = match look {
            Some(look) => {
                bar_fields::label(ui, look, "Brush Size");
                gap(ui, LABEL_FIELD_GAP);
                bar_fields::number(
                    ui,
                    look,
                    "brush_size",
                    &mut options.brush_size,
                    1..=MAX_BRUSH_SIZE,
                    true,
                )
            }
            None => {
                ui.label("Brush size");
                ui.add(egui::Slider::new(
                    &mut options.brush_size,
                    1..=MAX_BRUSH_SIZE,
                ))
            }
        };
        size.on_hover_text("Ctrl+wheel over the canvas changes it");
        let side = if look.is_some() {
            gap(ui, CONTROL_GAP);
            PIXEL_CONTROL_HEIGHT
        } else {
            20.0
        };
        brush_shape_buttons(ui, &mut options.brush_shape, side);
    }
    if matches!(tool, ToolKind::Pencil | ToolKind::Eraser) {
        match look {
            // Como en el diseño: en el grupo del pincel, al lado del campo.
            Some(look) => {
                gap(ui, CONTROL_GAP);
                bar_fields::checkbox(ui, look, &mut options.pixel_perfect, "Pixel Perfect");
            }
            None => {
                groups.next(ui);
                ui.checkbox(&mut options.pixel_perfect, "Pixel-perfect");
            }
        }
    }
    if matches!(tool, ToolKind::Rectangle | ToolKind::Ellipse) {
        groups.next(ui);
        check_option(ui, look, &mut options.filled, "Filled");
    }
    if tool == ToolKind::Rectangle {
        groups.next(ui);
        let radius = match look {
            Some(look) => {
                bar_fields::label(ui, look, "Corner radius");
                gap(ui, LABEL_FIELD_GAP);
                bar_fields::number(
                    ui,
                    look,
                    "corner_radius",
                    &mut options.corner_radius,
                    0..=32,
                    true,
                )
            }
            None => {
                ui.label("Corner radius");
                ui.add(egui::Slider::new(&mut options.corner_radius, 0..=32))
            }
        };
        radius.on_hover_text("0 = square corners; large values make a pill or a circle");
    }
    if tool.is_selection() {
        groups.next(ui);
        match look {
            Some(look) => selection_mode_buttons(ui, look, &mut options.selection_mode),
            None => {
                ui.label("Mode");
                for mode in SelectionMode::ALL {
                    ui.radio_value(&mut options.selection_mode, mode, mode.label());
                }
            }
        }
    }
    match tool {
        ToolKind::Hand => {
            groups.next(ui);
            hint(
                ui,
                look,
                "Drag to move the view. Space+drag or the middle button do it with any tool.",
            );
        }
        ToolKind::MagicWand => {
            groups.next(ui);
            match look {
                Some(look) => {
                    bar_fields::label(ui, look, "Tolerance");
                    gap(ui, LABEL_FIELD_GAP);
                    bar_fields::number(
                        ui,
                        look,
                        "tolerance",
                        &mut options.tolerance,
                        0..=255,
                        false,
                    );
                    gap(ui, CONTROL_GAP);
                    bar_fields::checkbox(ui, look, &mut options.contiguous, "Contiguous");
                    gap(ui, CONTROL_GAP);
                    bar_fields::label(ui, look, "Sample:");
                    gap(ui, LABEL_FIELD_GAP);
                    let name = |all| if all { "All layers" } else { "Layer" };
                    bar_fields::options(ui, look, &mut options.sample_all, [false, true], name);
                }
                None => {
                    ui.label("Tolerance");
                    ui.add(egui::Slider::new(&mut options.tolerance, 0..=255));
                    ui.checkbox(&mut options.contiguous, "Contiguous");
                    ui.label("Sample:");
                    ui.radio_value(&mut options.sample_all, false, "Layer");
                    ui.radio_value(&mut options.sample_all, true, "All layers");
                }
            }
        }
        ToolKind::Lasso => {
            groups.next(ui);
            hint(
                ui,
                look,
                "Drag around the area; it closes when you release.",
            );
        }
        ToolKind::PolygonLasso => {
            groups.next(ui);
            hint(
                ui,
                look,
                "Click to add points. Click the first point, double-click or press Enter to close; Esc cancels.",
            );
        }
        ToolKind::Bucket => {
            groups.next(ui);
            check_option(ui, look, &mut options.contiguous, "Contiguous");
        }
        ToolKind::Spray => {
            groups.next(ui);
            match look {
                Some(look) => {
                    bar_fields::label(ui, look, "Spray width");
                    gap(ui, LABEL_FIELD_GAP);
                    bar_fields::number(
                        ui,
                        look,
                        "spray_width",
                        &mut options.spray_width,
                        1..=64,
                        true,
                    );
                    gap(ui, CONTROL_GAP);
                    bar_fields::label(ui, look, "Spray speed");
                    gap(ui, LABEL_FIELD_GAP);
                    bar_fields::number(
                        ui,
                        look,
                        "spray_speed",
                        &mut options.spray_speed,
                        1..=100,
                        false,
                    );
                }
                None => {
                    ui.label("Spray width");
                    ui.add(egui::Slider::new(&mut options.spray_width, 1..=64));
                    ui.label("Spray speed");
                    ui.add(egui::Slider::new(&mut options.spray_speed, 1..=100));
                }
            }
        }
        ToolKind::Gradient => {
            groups.next(ui);
            match look {
                Some(look) => {
                    let name = |shape| match shape {
                        GradientShape::Linear => "Linear",
                        GradientShape::Radial => "Radial",
                    };
                    let shapes = [GradientShape::Linear, GradientShape::Radial];
                    bar_fields::options(ui, look, &mut options.gradient_shape, shapes, name);
                    gap(ui, CONTROL_GAP);
                    let dither = &mut options.dither;
                    bar_fields::combo(ui, look, "dither", dither.label(), |ui| {
                        bar_fields::choices(ui, dither, Dither::ALL, Dither::label)
                    });
                }
                None => {
                    ui.radio_value(&mut options.gradient_shape, GradientShape::Linear, "Linear");
                    ui.radio_value(&mut options.gradient_shape, GradientShape::Radial, "Radial");
                    egui::ComboBox::from_id_salt("dither")
                        .selected_text(options.dither.label())
                        .show_ui(ui, |ui| {
                            for d in Dither::ALL {
                                ui.selectable_value(&mut options.dither, d, d.label());
                            }
                        });
                }
            }
            groups.next(ui);
            hint(
                ui,
                look,
                "Drag from the primary to the secondary color. Right-drag swaps them.",
            );
        }
        ToolKind::Jumble => {
            groups.next(ui);
            hint(ui, look, "Scrambles pixels within the brush radius.");
        }
        ToolKind::Contour => {
            groups.next(ui);
            hint(
                ui,
                look,
                "Draw the outline: it closes and fills when you release.",
            );
        }
        ToolKind::Blur => {
            groups.next(ui);
            hint(
                ui,
                look,
                "Softens the pixels under the brush. Go over an area again to blur it more.",
            );
        }
        ToolKind::Text => {
            groups.next(ui);
            text_style_options(ui, editor, look);
            groups.next(ui);
            hint(ui, look, "Click on the canvas to place text.");
        }
        ToolKind::Slice => {
            groups.next(ui);
            hint(
                ui,
                look,
                "Drag to create a slice (hitbox, pivot…). Drag the selected one to move it or its edges to resize it: changes apply from the current frame on.",
            );
        }
        _ => {}
    }
    if tool.is_selection() || tool == ToolKind::Move {
        groups.next(ui);
        let before = editor.options.rotation;
        match look {
            Some(look) => {
                bar_fields::label(ui, look, "Rotation:");
                gap(ui, LABEL_FIELD_GAP);
                bar_fields::options(
                    ui,
                    look,
                    &mut editor.options.rotation,
                    RotationAlgorithm::ALL,
                    RotationAlgorithm::label,
                );
            }
            None => {
                ui.label("Rotation:");
                for algorithm in RotationAlgorithm::ALL {
                    ui.radio_value(&mut editor.options.rotation, algorithm, algorithm.label());
                }
            }
        }
        if editor.options.rotation != before {
            editor.refresh_floating(false);
        }
        groups.next(ui);
        hint(
            ui,
            look,
            "Drag the handles to scale, or just outside a corner to rotate. Shift keeps the proportions or snaps to 15°.",
        );
    }
    if matches!(
        tool,
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse | ToolKind::Gradient
    ) || matches!(tool, ToolKind::RectSelect | ToolKind::EllipseSelect)
    {
        groups.next(ui);
        hint(ui, look, "Shift: constrain");
    }
    if tool.paints() {
        groups.next(ui);
        hint(ui, look, "Alt+click: pick color");
    }
}

/// Una ayuda de la barra: con Pixel UI, en la fila de los textos del diseño; sin él, `ui.weak`.
fn hint(ui: &mut egui::Ui, look: Option<BarLook>, text: &str) {
    match look {
        Some(look) => {
            bar_fields::hint(ui, look, text);
        }
        None => {
            ui.weak(text);
        }
    }
}

/// Una casilla de la barra: con Pixel UI, la del diseño; sin él, la de egui.
fn check_option(ui: &mut egui::Ui, look: Option<BarLook>, checked: &mut bool, label: &str) {
    match look {
        Some(look) => {
            bar_fields::checkbox(ui, look, checked, label);
        }
        None => {
            ui.checkbox(checked, label);
        }
    }
}

/// El ícono del skin de cada modo de selección.
fn selection_mode_icon(mode: SelectionMode) -> &'static str {
    match mode {
        SelectionMode::Replace => "icon.sel_replace",
        SelectionMode::Add => "icon.sel_add",
        SelectionMode::Subtract => "icon.sel_subtract",
        SelectionMode::Intersect => "icon.sel_intersect",
    }
}

/// Con Pixel UI, "Mode" y los cuatro modos como botones de ícono, como en el diseño: los marcos
/// son los de simetría (`sym.slot`, y `sym.slot.active` el elegido; el diseño no muestra otro). Se
/// elige uno solo, como con radios. Cada botón tiene el nombre del modo de tooltip y de nombre
/// accesible.
fn selection_mode_buttons(ui: &mut egui::Ui, look: BarLook, mode: &mut SelectionMode) {
    bar_fields::label(ui, look, "Mode");
    gap(ui, LABEL_TOGGLE_GAP);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = TOGGLE_GAP;
        for value in SelectionMode::ALL {
            let on = *mode == value;
            let response = pixel_ui::toggle_button(
                ui,
                look.skin,
                selection_mode_icon(value),
                value.label(),
                on,
                |r, color| selection_mode_shapes(r, value, color),
            )
            .on_hover_text(value.label());
            if response.clicked() {
                *mode = value;
            }
        }
    });
}

/// Sin el ícono del skin: dos cuadrados que se superponen, con la parte del modo pintada.
fn selection_mode_shapes(r: Rect, mode: SelectionMode, color: Color32) -> Vec<egui::Shape> {
    let a = Rect::from_min_size(r.min, r.size() * 0.6);
    let b = Rect::from_min_max(r.max - r.size() * 0.6, r.max);
    let stroke = Stroke::new(1.0, color);
    let fill = match mode {
        SelectionMode::Replace => b,
        SelectionMode::Add => a.union(b),
        SelectionMode::Subtract => a,
        SelectionMode::Intersect => a.intersect(b),
    };
    vec![
        egui::Shape::rect_stroke(a, 0.0, stroke, StrokeKind::Inside),
        egui::Shape::rect_stroke(b, 0.0, stroke, StrokeKind::Inside),
        egui::Shape::rect_filled(fill.shrink(1.0), 0.0, color),
    ]
}

/// Botones Round y Square de la forma del pincel, cuadrados de `side`. El círculo y el cuadrado
/// se pintan en vez de usar glifos: `●` solo está en Hack, que no entra en la familia
/// proporcional de egui.
fn brush_shape_buttons(
    ui: &mut egui::Ui,
    shape: &mut BrushShape,
    side: f32,
) -> [egui::Response; 2] {
    ui.horizontal(|ui| {
        [(BrushShape::Round, "Round"), (BrushShape::Square, "Square")].map(|(value, name)| {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
            if response.clicked() {
                *shape = value;
            }
            let selected = *shape == value;
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name)
            });
            let visuals = ui.style().interact_selectable(&response, selected);
            let painter = ui.painter();
            painter.rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
                StrokeKind::Inside,
            );
            // El dibujo deja un cuarto del lado de margen: 5 de 20, 3 de 13.
            let icon = rect.shrink((side / 4.0).floor());
            let color = visuals.fg_stroke.color;
            if value == BrushShape::Round {
                painter.circle_filled(icon.center(), icon.width() / 2.0, color);
            } else {
                painter.rect_filled(icon, 0.0, color);
            }
            response.on_hover_text(name)
        })
    })
    .inner
}

/// Las opciones del texto: la fuente, cargar una y el tamaño. Con `look` (Pixel UI, en la barra),
/// los controles del diseño; sin él (o en la ventana del texto), los de egui.
fn text_style_options(ui: &mut egui::Ui, editor: &mut Editor, look: Option<BarLook>) {
    let style = &mut editor.text_style;
    let mut fonts = vec![FontChoice::Pixel, FontChoice::Hack, FontChoice::Ubuntu];
    if let FontChoice::Custom { .. } = style.font {
        fonts.push(style.font.clone());
    }
    let pick = |ui: &mut egui::Ui, style: &mut cs_tools::text::TextStyle| {
        for font in fonts {
            let name = font.name();
            let selected = style.font == font;
            let clicked = match look {
                Some(_) => ui.add(menus::choice(selected, &name)).clicked(),
                None => ui.selectable_label(selected, name).clicked(),
            };
            if clicked && !selected {
                style.size = if font == FontChoice::Pixel { 1 } else { 16 };
                style.font = font;
            }
        }
    };
    match look {
        Some(look) => {
            bar_fields::label(ui, look, "Font");
            gap(ui, LABEL_FIELD_GAP);
            let selected = style.font.name();
            bar_fields::combo(ui, look, "font", &selected, |ui| pick(ui, style));
        }
        None => {
            egui::ComboBox::from_label("Font")
                .selected_text(style.font.name())
                .show_ui(ui, |ui| pick(ui, style));
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    if look.is_some() {
        gap(ui, CONTROL_GAP);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Load font…").clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("Fonts", &["ttf", "otf"])
            .pick_file()
    {
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Font")
            .to_string();
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| FontChoice::load(name, &bytes))
        {
            Ok(font) => {
                style.font = font;
                style.size = 16;
            }
            Err(e) => log::warn!("could not load font {}: {e}", path.display()),
        }
    }
    match look {
        Some(look) => {
            gap(ui, CONTROL_GAP);
            if style.font == FontChoice::Pixel {
                bar_fields::label(ui, look, "Scale");
                gap(ui, LABEL_FIELD_GAP);
                bar_fields::number(ui, look, "text_scale", &mut style.size, 1..=8, false);
            } else {
                bar_fields::label(ui, look, "Size");
                gap(ui, LABEL_FIELD_GAP);
                bar_fields::number(ui, look, "text_size", &mut style.size, 6..=96, true);
                gap(ui, CONTROL_GAP);
                bar_fields::checkbox(ui, look, &mut style.antialias, "Antialias");
            }
        }
        None => {
            if style.font == FontChoice::Pixel {
                ui.add(egui::Slider::new(&mut style.size, 1..=8).text("Scale"));
            } else {
                ui.add(egui::Slider::new(&mut style.size, 6..=96).text("Size"));
                ui.checkbox(&mut style.antialias, "Antialias");
            }
        }
    }
}

/// Ventana flotante para escribir el texto que se está colocando.
fn text_window(ctx: &egui::Context, editor: &mut Editor) {
    if editor.text.is_none() {
        return;
    }
    let mut commit = false;
    let mut cancel = false;
    egui::Window::new("Text")
        .collapsible(false)
        .resizable(false)
        .default_width(260.0)
        .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 90.0))
        .show(ctx, |ui| {
            text_style_options(ui, editor, None);
            let Some(draft) = editor.text.as_mut() else {
                return;
            };
            let edit = egui::TextEdit::multiline(&mut draft.text)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("Type here…");
            let response = ui.add(edit);
            if draft.text.is_empty() && !response.has_focus() {
                response.request_focus();
            }
            if response.changed() {
                editor.revision += 1;
            }
            let (enter, esc) = ui.input(|i| {
                (
                    i.modifiers.command && i.key_pressed(egui::Key::Enter),
                    i.key_pressed(egui::Key::Escape),
                )
            });
            ui.weak("Click the canvas to move it. Ctrl+Enter applies.");
            ui.horizontal(|ui| {
                commit = ui.button("OK").clicked() || enter;
                cancel = ui.button("Cancel").clicked() || esc;
            });
        });
    if commit {
        editor.commit_text();
    } else if cancel {
        editor.cancel_gesture();
    }
}

/// Un eje de simetría de la barra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymAxis {
    Vertical,
    Horizontal,
    DiagonalDown,
    DiagonalUp,
}

impl SymAxis {
    /// Ícono del skin del eje.
    fn icon(self) -> &'static str {
        match self {
            SymAxis::Vertical => "icon.sym_vertical",
            SymAxis::Horizontal => "icon.sym_horizontal",
            SymAxis::DiagonalDown => "icon.sym_diag_down",
            SymAxis::DiagonalUp => "icon.sym_diag_up",
        }
    }

    /// Si el eje está prendido en `axes`.
    fn on(self, axes: SymmetryAxes) -> bool {
        match self {
            SymAxis::Vertical => axes.vertical,
            SymAxis::Horizontal => axes.horizontal,
            SymAxis::DiagonalDown => axes.diagonal_down,
            SymAxis::DiagonalUp => axes.diagonal_up,
        }
    }

    /// `axes` con este eje cambiado y los demás igual.
    fn toggled(self, mut axes: SymmetryAxes) -> SymmetryAxes {
        let flag = match self {
            SymAxis::Vertical => &mut axes.vertical,
            SymAxis::Horizontal => &mut axes.horizontal,
            SymAxis::DiagonalDown => &mut axes.diagonal_down,
            SymAxis::DiagonalUp => &mut axes.diagonal_up,
        };
        *flag = !*flag;
        axes
    }
}

/// Los ejes en el orden del mockup (│ ─ ╲ ╱), con su nombre, que es también el tooltip. Las
/// diagonales se nombran con `\` y `/`: ╲ y ╱ no están en las fuentes proporcionales.
const SYMMETRY_AXES: [(SymAxis, &str); 4] = [
    (SymAxis::Vertical, "Vertical axis (left ↔ right)"),
    (SymAxis::Horizontal, "Horizontal axis (top ↔ bottom)"),
    (SymAxis::DiagonalDown, "Diagonal axis \\"),
    (SymAxis::DiagonalUp, "Diagonal axis /"),
];

/// Ayuda de los ejes, en el tooltip de los botones de simetría cuando hay alguno prendido.
const AXES_HINT: &str = "Ctrl+drag the dashed axes to move them";

/// Symmetry, al final de la barra de opciones. Se arma de derecha a izquierda (la barra lo pone
/// pegado al borde): "Center axes" si los ejes se movieron, los cuatro ejes en orden inverso y el
/// título. En pantalla se leen │ ─ ╲ ╱. Cada botón prende o apaga solo su eje. Con `look`
/// (Pixel UI), los botones son las piezas `sym.slot` con su ícono y el título va en la fila de
/// los textos del diseño.
fn symmetry_buttons(ui: &mut egui::Ui, editor: &mut Editor, look: Option<BarLook>) {
    let skin = look.map(|l| l.skin);
    let current = editor.symmetry();
    if current.axes.any() && current.center2.is_some() && ui.small_button("Center axes").clicked() {
        editor.set_symmetry(cs_core::Symmetry {
            center2: None,
            ..current
        });
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (axis, tip) in SYMMETRY_AXES.into_iter().rev() {
            let on = axis.on(current.axes);
            let response = match skin {
                Some(skin) => {
                    pixel_ui::toggle_button(ui, skin, axis.icon(), tip, on, |r, color| {
                        axis_shapes(r, axis, color)
                    })
                }
                None => symmetry_toggle(ui, axis, tip, on),
            };
            let response = if current.axes.any() {
                response.on_hover_text(format!("{tip}\n{AXES_HINT}"))
            } else {
                response.on_hover_text(tip)
            };
            if response.clicked() {
                editor.set_symmetry(cs_core::Symmetry {
                    axes: axis.toggled(current.axes),
                    ..current
                });
            }
        }
    });
    match look {
        Some(look) => {
            gap(ui, SYMMETRY_LABEL_GAP);
            bar_fields::label(ui, look, "Symmetry");
        }
        None => {
            ui.label("Symmetry");
        }
    }
}

/// Un botón de prender y apagar un eje, dibujado con figuras.
fn symmetry_toggle(ui: &mut egui::Ui, axis: SymAxis, tip: &str, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, tip)
    });
    let visuals = ui.style().interact_selectable(&response, on);
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, visuals.weak_bg_fill);
    let color = if on {
        theme::ACCENT
    } else {
        visuals.fg_stroke.color
    };
    let r = rect.shrink(5.0);
    painter.rect_stroke(
        r,
        1.0,
        Stroke::new(1.0, color.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );
    painter.extend(axis_shapes(r, axis, color));
    response
}

/// El eje como una línea de lado a lado de `r`.
fn axis_shapes(r: Rect, axis: SymAxis, color: Color32) -> Vec<egui::Shape> {
    let stroke = Stroke::new(1.5, color);
    let c = r.center();
    let segment = match axis {
        SymAxis::Vertical => [egui::pos2(c.x, r.top()), egui::pos2(c.x, r.bottom())],
        SymAxis::Horizontal => [egui::pos2(r.left(), c.y), egui::pos2(r.right(), c.y)],
        SymAxis::DiagonalDown => [r.left_top(), r.right_bottom()],
        SymAxis::DiagonalUp => [r.left_bottom(), r.right_top()],
    };
    vec![egui::Shape::line_segment(segment, stroke)]
}

/// Damero mínimo para que se note la transparencia.
fn checker(painter: &egui::Painter, rect: Rect) {
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

/// Marco blanco si `value` es el color primario; del color de acento si es el secundario.
fn mark_selected(ui: &egui::Ui, editor: &Editor, value: PixelValue, rect: Rect) {
    let color = if value == editor.primary {
        Color32::WHITE
    } else if value == editor.secondary {
        theme::ACCENT
    } else {
        return;
    };
    ui.painter()
        .rect_stroke(rect, 0.0, Stroke::new(2.0, color), StrokeKind::Inside);
}

/// Swatch del color Mask (transparente): tablero con una diagonal roja.
fn mask_swatch(ui: &mut egui::Ui, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    paint_mask(ui.painter(), rect);
    response
}

fn paint_mask(painter: &egui::Painter, rect: Rect) {
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

fn to_color32(c: cs_core::Rgba) -> Color32 {
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
            ui.weak("…");
        }
    });
}

/// `menus`: con un diálogo abierto, Presets ⏷ y … quedan desactivados.
pub fn palette(ui: &mut egui::Ui, editor: &mut Editor, menus: bool) -> Option<PaletteAction> {
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
            .on_hover_text(format!("Primary (left click){}", name(editor.primary)));
        color_swatch(ui, editor, editor.secondary, 24.0)
            .on_hover_text(format!("Secondary (right click){}", name(editor.secondary)));
        if ui.small_button("🔄").on_hover_text("Swap (X)").clicked() {
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
            ui.menu_button("…", |ui| {
                let mut item = |ui: &mut egui::Ui, label: &str, a: PaletteAction| {
                    if ui.button(label).clicked() {
                        action = Some(a);
                        ui.close();
                    }
                };
                item(ui, "Load Palette…", PaletteAction::Load);
                item(ui, "Save Palette…", PaletteAction::Save);
                item(ui, "Save as Preset…", PaletteAction::SaveAsPreset);
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
        let response =
            mask_swatch(ui, size).on_hover_text("Mask (transparent): painting with it erases");
        mark_selected(ui, editor, mask, response.rect);
        if response.clicked() {
            editor.primary = mask;
        }
        if response.secondary_clicked() {
            editor.secondary = mask;
        }
        let count = editor.sprite().palette.len().min(256);
        for i in 0..count {
            let index = i as u8;
            let value = editor.palette_value(index);
            let color = to_color32(editor.sprite().palette.colors()[i]);
            let response = swatch(ui, color, size).on_hover_text(format!("#{i}"));
            mark_selected(ui, editor, value, response.rect);
            if response.clicked() {
                editor.primary = value;
            }
            if response.secondary_clicked() {
                editor.secondary = value;
            }
        }
    });
    ui.add_space(10.0);
    color_picker(ui, editor);
    action
}

/// Lado máximo del cuadro del selector de color, en puntos.
const COLOR_PICKER_MAX: f32 = 240.0;

/// Selector de color libre (RGBA): edita el color primario.
pub fn color_picker(ui: &mut egui::Ui, editor: &mut Editor) {
    let cs_core::PixelValue::Rgba(current) = editor.primary else {
        ui.weak("Indexed sprite: colors come from the palette.");
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
            .on_hover_text("Add to palette")
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
            if response.on_hover_text(format!("Tile {number}")).clicked() {
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
            .on_hover_text("Flip horizontally (X)")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_X);
        }
        if ui
            .selectable_label(brush.flip_y(), "Y")
            .on_hover_text("Flip vertically (Y)")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_Y);
        }
        if ui
            .add_enabled(tw == th, egui::Button::selectable(brush.flip_d(), "D"))
            .on_hover_text("Flip diagonally (swap X and Y)")
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
        if editor.editing_mask() {
            ui.separator();
            ui.colored_label(
                theme::ACCENT,
                "Editing layer mask: white reveals, black hides",
            );
        }
        if let Some(text) = editor.mask_status() {
            ui.separator();
            ui.colored_label(theme::ACCENT, text);
        }
        let on_instance = s
            .layer(editor.layer)
            .is_some_and(|l| l.kind == cs_core::LayerKind::Instance);
        if on_instance && editor.active_tool().paints() {
            ui.separator();
            ui.colored_label(theme::ACCENT, "Instance layer: edit the source artboard");
        }
        if let Some(text) = editor.tiles_status() {
            ui.separator();
            ui.colored_label(theme::ACCENT, text);
        }
        if let Some(text) = editor.floating_status() {
            ui.separator();
            ui.colored_label(theme::ACCENT, text);
        }
        if let Some(hint) = canvas.guide_hint {
            ui.separator();
            ui.colored_label(theme::GUIDE_LINE, hint);
        }
        if let Some(notice) = notice {
            ui.separator();
            ui.colored_label(theme::ACCENT, notice);
        }
    });
}

/// Lista de artboards: activar, crear, renombrar y eliminar.
/// Devuelve `true` si se pidió abrir el diálogo de tamaño.
pub fn artboards(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    name_buffer: &mut (ArtboardId, String),
) -> bool {
    let mut resize = false;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.heading("Artboards");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_remove = editor.document.artboards.len() > 1;
            if ui
                .add_enabled(can_remove, egui::Button::new("🗑").small())
                .on_hover_text("Delete artboard")
                .clicked()
            {
                editor.remove_active_artboard();
            }
            if ui
                .small_button("➕")
                .on_hover_text("New 64×64 artboard")
                .clicked()
            {
                editor.add_artboard(64, 64);
            }
        });
    });
    ui.separator();

    let ids: Vec<(ArtboardId, String)> = editor
        .document
        .artboards
        .iter()
        .rev()
        .map(|a| (a.id, a.name.clone()))
        .collect();
    for (id, name) in ids {
        if ui.selectable_label(id == editor.active, name).clicked() {
            editor.activate(id);
        }
    }

    ui.add_space(4.0);
    if name_buffer.0 != editor.active {
        *name_buffer = (editor.active, editor.artboard().name.clone());
    }
    let response = ui.add(egui::TextEdit::singleline(&mut name_buffer.1).hint_text("Name"));
    if response.lost_focus() {
        let name = name_buffer.1.trim().to_string();
        if name.is_empty() {
            name_buffer.1 = editor.artboard().name.clone();
        } else {
            editor.rename_active_artboard(name);
        }
    } else if !response.has_focus() {
        name_buffer.1 = editor.artboard().name.clone();
    }
    if ui.button("Canvas Size…").clicked() {
        resize = true;
    }
    resize
}

/// Gesto sobre campos numéricos o sliders: un solo paso de undo por arrastre o edición.
/// Llamar antes de aplicar el cambio; devuelve el grupo (id del widget que cambió) si cambió.
/// El gesto anterior del mismo widget ya terminó en `gesture_end` (al soltar o perder el foco),
/// así que acá solo se corta al tomar foco; cortar en `drag_started` partiría en dos el
/// "clic (el slider salta) + arrastre".
fn gesture_group(editor: &mut Editor, responses: &[&egui::Response]) -> Option<u64> {
    if responses.iter().any(|r| r.gained_focus()) {
        editor.end_group();
    }
    responses.iter().find(|r| r.changed()).map(|r| r.id.value())
}

/// Llamar después de aplicar: cierra el gesto si terminó (al soltar el botón, al perder el foco,
/// o si el cambio no vino de un botón apretado ni de un campo con foco).
fn gesture_end(editor: &mut Editor, responses: &[&egui::Response]) {
    let ended = responses.iter().any(|r| {
        r.drag_stopped()
            || r.clicked()
            || r.lost_focus()
            || (r.changed() && !r.is_pointer_button_down_on() && !r.has_focus())
    });
    if ended {
        editor.end_group();
    }
}

/// Rango de los campos de posición de la instancia.
const POSITION_RANGE: std::ops::RangeInclusive<i32> = -16384..=16384;

/// Propiedades de la instancia de la capa actual en el frame actual. Devuelve `true` si se
/// entró al símbolo (el lienzo debe centrarlo).
pub fn instance(ui: &mut egui::Ui, editor: &mut Editor) -> bool {
    let Some(key) = editor.instance_key() else {
        return false;
    };
    ui.heading("Instance");
    let is_key = editor.is_instance_keyframe(editor.layer, editor.frame);
    if !is_key {
        ui.weak("Changes create a keyframe on this frame.");
    }
    let names: Vec<(cs_core::ArtboardId, String)> = editor.instance_candidates();
    let current = key
        .source
        .and_then(|s| editor.document.artboard(s))
        .map_or("(missing)".to_string(), |a| a.name.clone());
    egui::ComboBox::from_label("Source")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (id, name) in names {
                if ui.selectable_label(key.source == Some(id), name).clicked() {
                    editor.update_instance_key(|k| k.source = Some(id));
                }
            }
        });
    let (mut x, mut y) = key.position;
    let (rx, ry) = ui
        .horizontal(|ui| {
            ui.label("Position");
            let rx = widgets::number(ui, &mut x, POSITION_RANGE, "");
            let ry = widgets::number(ui, &mut y, POSITION_RANGE, "");
            (rx, ry)
        })
        .inner;
    if let Some(group) = gesture_group(editor, &[&rx, &ry]) {
        editor.update_instance_key_grouped(Some(group), |k| k.position = (x, y));
    }
    gesture_end(editor, &[&rx, &ry]);
    let (mut fh, mut fv) = (key.flip_h, key.flip_v);
    ui.horizontal(|ui| {
        if ui.checkbox(&mut fh, "Flip H").changed() {
            editor.update_instance_key(|k| k.flip_h = fh);
        }
        if ui.checkbox(&mut fv, "Flip V").changed() {
            editor.update_instance_key(|k| k.flip_v = fv);
        }
    });
    let mut opacity = key.opacity;
    let r = ui.add(egui::Slider::new(&mut opacity, 0..=255).text("Opacity"));
    if let Some(group) = gesture_group(editor, &[&r]) {
        editor.update_instance_key_grouped(Some(group), |k| k.opacity = opacity);
    }
    gesture_end(editor, &[&r]);
    use cs_core::{GraphicPlay, Timing};
    let (mut first, mut mode) = match key.timing {
        Timing::Graphic { first_frame, play } => (first_frame, Some(play)),
        Timing::MovieClip { first_frame } => (first_frame, None),
    };
    let before = (first, mode);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut mode, Some(GraphicPlay::Loop), "Loop")
            .on_hover_text("Graphic: follows this artboard's frames, looping");
        ui.selectable_value(&mut mode, Some(GraphicPlay::Once), "Once")
            .on_hover_text("Graphic: plays once and holds the last frame");
        ui.selectable_value(&mut mode, Some(GraphicPlay::Single), "Single")
            .on_hover_text("Graphic: always the same frame");
        ui.selectable_value(&mut mode, None, "Movie clip")
            .on_hover_text("Plays on its own clock with its own frame durations");
    });
    let frames = key
        .source
        .and_then(|s| editor.document.artboard(s))
        .map_or(1, |a| a.sprite.frames.len());
    let first_response = ui
        .horizontal(|ui| {
            ui.label("First frame");
            let mut shown = first + 1;
            let r = widgets::number(ui, &mut shown, 1..=frames, "");
            if r.changed() {
                first = shown - 1;
            }
            r
        })
        .inner;
    // El modo es un clic (paso propio); el primer frame se arrastra (un paso por gesto).
    let group = gesture_group(editor, &[&first_response]).filter(|_| mode == before.1);
    if (first, mode) != before {
        editor.update_instance_key_grouped(group, |k| {
            k.timing = match mode {
                Some(play) => Timing::Graphic {
                    first_frame: first,
                    play,
                },
                None => Timing::MovieClip { first_frame: first },
            }
        });
    }
    gesture_end(editor, &[&first_response]);
    let entered = ui
        .button("Edit Symbol")
        .on_hover_text("Go to the source artboard (double-click with Move)")
        .clicked()
        && editor.enter_instance();
    ui.separator();
    entered
}

/// Panel de slices: lista, nombre, color, datos y la clave del frame actual (rect y pivote).
pub fn slices(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.horizontal(|ui| {
        ui.heading("Slices");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(editor.slice.is_some(), egui::Button::new("🗑").small())
                .on_hover_text("Delete slice")
                .clicked()
            {
                editor.delete_slice();
            }
        });
    });
    ui.separator();
    let names: Vec<(String, cs_core::Rgba)> = editor
        .sprite()
        .slices
        .iter()
        .map(|s| (s.name.clone(), s.color))
        .collect();
    if names.is_empty() {
        ui.weak("No slices. Use the Slice tool (Shift+C) to mark hitboxes and pivots.");
        return;
    }
    for (i, (name, color)) in names.iter().enumerate() {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            ui.painter()
                .rect_filled(r, 2.0, Color32::from_rgb(color.r, color.g, color.b));
            if ui.selectable_label(editor.slice == Some(i), name).clicked() {
                editor.end_group();
                editor.slice = Some(i);
                editor.set_tool(ToolKind::Slice);
            }
        });
    }
    if editor.slice.is_some() {
        ui.separator();
        slice_properties(ui, editor);
    }
}

/// Nombre, color, datos y clave del frame actual del slice seleccionado (panel y diálogo).
pub fn slice_properties(ui: &mut egui::Ui, editor: &mut Editor) {
    let Some(slice) = editor.selected_slice().cloned() else {
        return;
    };
    let mut name = slice.name.clone();
    ui.horizontal(|ui| {
        ui.label("Name");
        if ui.text_edit_singleline(&mut name).lost_focus() && name != slice.name {
            editor.update_slice(|s| s.name = name.clone());
        }
    });
    let mut c32 = Color32::from_rgb(slice.color.r, slice.color.g, slice.color.b);
    ui.horizontal(|ui| {
        ui.label("Color");
        if ui.color_edit_button_srgba(&mut c32).changed() {
            let [r, g, b, _] = c32.to_array();
            editor.update_slice(|s| s.color = cs_core::Rgba::rgb(r, g, b));
        }
    });
    let mut text = slice.text.clone();
    ui.horizontal(|ui| {
        ui.label("Data");
        if ui
            .text_edit_singleline(&mut text)
            .on_hover_text("Free text exported with the slice (e.g. damage=3)")
            .lost_focus()
            && text != slice.text
        {
            editor.update_slice(|s| s.text = text.clone());
        }
    });
    let frame = editor.frame;
    let Some(key) = slice.key_at(frame).copied() else {
        ui.weak(format!("Hidden on frame {}", frame + 1));
        return;
    };
    let own_key = key.frame == frame;
    ui.label(if own_key {
        format!("Frame {} (own key)", frame + 1)
    } else {
        format!("Frame {} (from frame {})", frame + 1, key.frame + 1)
    });
    let (mut x, mut y, mut w, mut h) = key.bounds;
    let (rx, ry) = ui
        .horizontal(|ui| {
            ui.label("X");
            let rx = widgets::number(ui, &mut x, -4096..=4096, "");
            ui.label("Y");
            let ry = widgets::number(ui, &mut y, -4096..=4096, "");
            (rx, ry)
        })
        .inner;
    let (rw, rh) = ui
        .horizontal(|ui| {
            ui.label("W");
            let rw = widgets::number(ui, &mut w, 1..=4096, "");
            ui.label("H");
            let rh = widgets::number(ui, &mut h, 1..=4096, "");
            (rw, rh)
        })
        .inner;
    let rect_fields = [&rx, &ry, &rw, &rh];
    if let Some(group) = gesture_group(editor, &rect_fields) {
        editor.update_slice_key_grouped(Some(group), |k| k.bounds = (x, y, w, h));
    }
    gesture_end(editor, &rect_fields);
    let mut has_pivot = key.pivot.is_some();
    if ui.checkbox(&mut has_pivot, "Pivot").changed() {
        let default = (w as i32 / 2, h as i32);
        editor.update_slice_key(|k| k.pivot = has_pivot.then_some(default));
    }
    if let Some((mut px, mut py)) = key.pivot {
        let (rx, ry) = ui
            .horizontal(|ui| {
                ui.label("X");
                let rx = widgets::number(ui, &mut px, -4096..=4096, "");
                ui.label("Y");
                let ry = widgets::number(ui, &mut py, -4096..=4096, "");
                (rx, ry)
            })
            .inner;
        if let Some(group) = gesture_group(editor, &[&rx, &ry]) {
            editor.update_slice_key_grouped(Some(group), |k| k.pivot = Some((px, py)));
        }
        gesture_end(editor, &[&rx, &ry]);
    }
    // 9-slice: el rect central (relativo al slice) se estira; las esquinas quedan fijas.
    let mut nine = key.center.is_some();
    if ui
        .checkbox(&mut nine, "9-Slice")
        .on_hover_text("Center area that stretches; corners keep their size")
        .changed()
    {
        editor
            .update_slice_key(|k| k.center = nine.then(|| crate::slice_ops::default_center(w, h)));
    }
    if let Some((mut cx, mut cy, mut cw, mut ch)) = key.center {
        let fields = ui
            .horizontal(|ui| {
                ui.label("X");
                let rx = widgets::number(ui, &mut cx, 0..=(w as i32 - 1).max(0), "");
                ui.label("Y");
                let ry = widgets::number(ui, &mut cy, 0..=(h as i32 - 1).max(0), "");
                [rx, ry]
            })
            .inner;
        let sizes = ui
            .horizontal(|ui| {
                ui.label("W");
                let rw = widgets::number(ui, &mut cw, 1..=w.max(1), "");
                ui.label("H");
                let rh = widgets::number(ui, &mut ch, 1..=h.max(1), "");
                [rw, rh]
            })
            .inner;
        let all = [&fields[0], &fields[1], &sizes[0], &sizes[1]];
        if let Some(group) = gesture_group(editor, &all) {
            // El centro no puede salirse del slice.
            let cw = cw.min(w.saturating_sub(cx as u32)).max(1);
            let ch = ch.min(h.saturating_sub(cy as u32)).max(1);
            editor.update_slice_key_grouped(Some(group), |k| k.center = Some((cx, cy, cw, ch)));
        }
        gesture_end(editor, &all);
    }
    if own_key
        && slice.keys.first().is_some_and(|k| k.frame != frame)
        && ui.button("Remove key on this frame").clicked()
    {
        editor.remove_slice_key();
    }
    let mut hide = false;
    if ui
        .small_button("Hide from this frame")
        .on_hover_text("Adds an empty key: the slice disappears from here on")
        .clicked()
    {
        hide = true;
    }
    if hide {
        editor.update_slice(|s| {
            s.set_key(cs_core::SliceKey {
                frame,
                bounds: (x, y, 0, 0),
                center: None,
                pivot: None,
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};
    use eframe::egui::{Event, PointerButton, Pos2, RawInput, pos2};

    use super::*;

    #[test]
    fn tile_thumbnails_are_cached_until_a_tile_image_changes() {
        let ctx = egui::Context::default();
        let mut e = crate::tile_ops::test_util::tile_editor();
        let mut thumbs = TileThumbs::default();
        thumbs.refresh(&ctx, &e);
        assert_eq!(thumbs.textures.len(), 3, "the empty tile and tiles 1 and 2");
        let first = thumbs.textures[1].id();
        thumbs.refresh(&ctx, &e);
        assert_eq!(thumbs.textures[1].id(), first, "cached");
        let cmd = cs_commands::ops::set_tiles(
            e.sprite(),
            e.layer,
            0,
            &[(0, 0, cs_core::Tile::new(1, false, false, false))],
        )
        .unwrap();
        e.execute(cmd);
        thumbs.refresh(&ctx, &e);
        assert_eq!(
            thumbs.textures[1].id(),
            first,
            "a set_tiles stroke keeps the thumbnails"
        );
        let mut img = cs_core::Image::new(4, 4, ColorMode::Rgba);
        img.set(1, 1, cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(9, 9, 9)))
            .unwrap();
        let cmd = cs_commands::ops::set_tile_image(e.sprite(), 0, 1, img).unwrap();
        e.execute(cmd);
        thumbs.refresh(&ctx, &e);
        assert_ne!(
            thumbs.textures[1].id(),
            first,
            "rebuilt after replacing a tile image"
        );
        e.select_layer(cs_core::LayerId(1));
        thumbs.refresh(&ctx, &e);
        assert!(thumbs.textures.is_empty(), "no tileset on an image layer");
    }

    /// Corre un frame con un slider de opacidad conectado como en el panel de instancia;
    /// devuelve el rect del slider.
    fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<Event>) -> Rect {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
            events,
            ..Default::default()
        };
        let mut rect = Rect::NOTHING;
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut opacity = editor.instance_key().unwrap().opacity;
                let r = ui.add(egui::Slider::new(&mut opacity, 0..=255));
                if let Some(group) = gesture_group(editor, &[&r]) {
                    editor.update_instance_key_grouped(Some(group), |k| k.opacity = opacity);
                }
                gesture_end(editor, &[&r]);
                rect = r.rect;
            });
        });
        output.textures_delta.clear();
        rect
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn slider_click_then_drag_is_one_undo_step() {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.add_artboard(4, 4);
        e.activate(cs_core::ArtboardId(1));
        e.add_instance_layer(cs_core::ArtboardId(2));
        let original = e.instance_key().unwrap();
        let ctx = egui::Context::default();
        let r = frame(&ctx, &mut e, vec![]);
        let y = r.center().y;
        let at = |t: f32| pos2(r.left() + r.width() * t, y);
        // Clic en el 20 % (el valor salta), arrastre hasta el 30 % y suelta.
        frame(&ctx, &mut e, vec![Event::PointerMoved(at(0.2))]);
        frame(&ctx, &mut e, vec![button(at(0.2), true)]);
        for t in [0.22, 0.25, 0.3] {
            frame(&ctx, &mut e, vec![Event::PointerMoved(at(t))]);
        }
        frame(&ctx, &mut e, vec![button(at(0.3), false)]);
        frame(&ctx, &mut e, vec![]);
        let dragged = e.instance_key().unwrap().opacity;
        assert_ne!(dragged, original.opacity, "the slider moved");
        // Un segundo clic es otro paso.
        frame(&ctx, &mut e, vec![Event::PointerMoved(at(0.6))]);
        frame(&ctx, &mut e, vec![button(at(0.6), true)]);
        frame(&ctx, &mut e, vec![button(at(0.6), false)]);
        frame(&ctx, &mut e, vec![]);
        assert_ne!(e.instance_key().unwrap().opacity, dragged);
        e.undo();
        assert_eq!(e.instance_key().unwrap().opacity, dragged);
        e.undo();
        assert_eq!(e.instance_key().unwrap(), original);
    }

    /// Corre un frame con el grupo `group` de la barra en el instante `time`; devuelve el rect
    /// de su botón.
    fn group_frame(
        ctx: &egui::Context,
        editor: &mut Editor,
        group: usize,
        time: f64,
        events: Vec<Event>,
    ) -> Rect {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut rect = Rect::NOTHING;
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                rect = tool_group(ui, editor, group).rect;
            });
        });
        output.textures_delta.clear();
        rect
    }

    fn mouse(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn escape() -> Event {
        Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn right_click_or_holding_a_group_button_opens_its_variants() {
        use PointerButton::{Primary, Secondary};
        use egui::Popup;
        let ctx = egui::Context::default();
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let bucket = ToolKind::GROUPS
            .iter()
            .position(|g| g.contains(&ToolKind::Bucket))
            .unwrap();
        let c = group_frame(&ctx, &mut e, bucket, 0.0, vec![]).center();
        group_frame(&ctx, &mut e, bucket, 0.05, vec![Event::PointerMoved(c)]);

        // Clic derecho: abre la lista y no cambia de herramienta.
        group_frame(&ctx, &mut e, bucket, 0.1, vec![mouse(c, Secondary, true)]);
        group_frame(&ctx, &mut e, bucket, 0.15, vec![mouse(c, Secondary, false)]);
        group_frame(&ctx, &mut e, bucket, 0.2, vec![]);
        assert!(Popup::is_any_open(&ctx), "right-click opens the list");
        assert_eq!(e.tool, ToolKind::Pencil);
        group_frame(&ctx, &mut e, bucket, 0.25, vec![escape()]);
        group_frame(&ctx, &mut e, bucket, 0.3, vec![]);
        assert!(!Popup::is_any_open(&ctx));

        // Mantener presionado: a los 400 ms se abre; al soltar sigue abierta y no elige.
        group_frame(&ctx, &mut e, bucket, 1.0, vec![mouse(c, Primary, true)]);
        group_frame(&ctx, &mut e, bucket, 1.2, vec![]);
        assert!(!Popup::is_any_open(&ctx), "not yet");
        group_frame(&ctx, &mut e, bucket, 1.45, vec![]);
        group_frame(&ctx, &mut e, bucket, 1.5, vec![]);
        assert!(Popup::is_any_open(&ctx), "held for 400 ms");
        group_frame(&ctx, &mut e, bucket, 1.55, vec![mouse(c, Primary, false)]);
        group_frame(&ctx, &mut e, bucket, 1.6, vec![]);
        assert!(Popup::is_any_open(&ctx), "releasing keeps it open");
        assert_eq!(e.tool, ToolKind::Pencil, "and does not pick the tool");
        group_frame(&ctx, &mut e, bucket, 1.65, vec![escape()]);
        group_frame(&ctx, &mut e, bucket, 1.7, vec![]);
        assert!(!Popup::is_any_open(&ctx));

        // Un clic corto elige la herramienta del grupo.
        group_frame(&ctx, &mut e, bucket, 2.0, vec![mouse(c, Primary, true)]);
        group_frame(&ctx, &mut e, bucket, 2.1, vec![mouse(c, Primary, false)]);
        group_frame(&ctx, &mut e, bucket, 2.15, vec![]);
        assert_eq!(e.tool, ToolKind::Bucket);
        assert!(!Popup::is_any_open(&ctx));
    }

    #[test]
    fn the_shape_buttons_pick_round_or_square() {
        use cs_tools::BrushShape;
        let ctx = egui::Context::default();
        let run = |shape: &mut BrushShape, events: Vec<Event>| -> [Rect; 2] {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
                events,
                ..Default::default()
            };
            let mut rects = [Rect::NOTHING; 2];
            let mut output = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    rects = brush_shape_buttons(ui, shape, 20.0).map(|r| r.rect);
                });
            });
            output.textures_delta.clear();
            rects
        };
        let click = |shape: &mut BrushShape, p: Pos2| {
            run(shape, vec![Event::PointerMoved(p)]);
            run(shape, vec![mouse(p, PointerButton::Primary, true)]);
            run(shape, vec![mouse(p, PointerButton::Primary, false)]);
        };
        let mut shape = BrushShape::Round;
        let [round, square] = run(&mut shape, vec![]);
        assert!(
            round.width() > 0.0 && square.left() >= round.right(),
            "side by side"
        );
        click(&mut shape, square.center());
        assert_eq!(shape, BrushShape::Square);
        click(&mut shape, round.center());
        assert_eq!(shape, BrushShape::Round);
    }

    /// La columna de íconos con el skin embebido dentro del scroll del panel, manejada con
    /// eventos crudos (el reloj avanza 50 ms por frame). Guarda los botones del árbol de
    /// accesibilidad (nombre y rect), las texturas dibujadas y el desplazamiento del scroll.
    struct ColumnHarness {
        ctx: egui::Context,
        editor: Editor,
        skin: Skin,
        time: f64,
        /// Alto de la pantalla: con 150 la columna (13 × 22) no entra y hay scroll.
        height: f32,
        buttons: Vec<(String, Rect)>,
        textures: Vec<egui::TextureId>,
        /// Lo dibujado con cada textura y su rect.
        meshes: Vec<(egui::TextureId, Rect)>,
        /// Los textos dibujados: el texto, su rect y su color.
        texts: Vec<(String, Rect, Color32)>,
        /// Los textos del árbol de accesibilidad (rol `Label`).
        label_nodes: Vec<String>,
        offset: f32,
        /// Sin Pixel UI: la barra con nombres (`toolbar` sin skin) en vez de la columna.
        named: bool,
    }

    impl ColumnHarness {
        fn new(height: f32) -> Self {
            let ctx = egui::Context::default();
            let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
            Self::with_skin(ctx, skin, height)
        }

        fn with_skin(ctx: egui::Context, skin: Skin, height: f32) -> Self {
            ctx.enable_accesskit();
            let mut h = Self {
                ctx,
                editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
                skin,
                time: 0.0,
                height,
                buttons: Vec::new(),
                textures: Vec::new(),
                meshes: Vec::new(),
                texts: Vec::new(),
                label_nodes: Vec::new(),
                offset: 0.0,
                named: false,
            };
            h.frame(vec![]);
            h
        }

        /// Con la fuente pixel y los colores del skin, como con Pixel UI encendido. La fuente rige
        /// desde el frame siguiente.
        fn pixel_style(mut self) -> Self {
            self.ctx
                .set_fonts(crate::appearance::font_definitions(true));
            crate::appearance::apply_style(&self.ctx, Some(self.skin.colors()));
            self.frame(vec![]);
            self
        }

        /// Pixel UI apagado: la barra con nombres, sin skin.
        fn named(mut self) -> Self {
            self.named = true;
            self.frame(vec![]);
            self
        }

        /// Deja el puntero quieto sobre el botón `label` hasta que pasa la demora del tooltip.
        /// Devuelve el rect del botón.
        fn hover(&mut self, label: &str) -> Rect {
            let rect = self.rect(label);
            self.frame(vec![Event::PointerMoved(rect.center())]);
            let delay = self.ctx.global_style().interaction.tooltip_delay as f64;
            self.frame_after(delay + 0.1, vec![]);
            self.frame(vec![]);
            // Que termine de aparecer (el área de egui entra con un fundido).
            self.frame_after(0.5, vec![]);
            rect
        }

        fn rect(&self, label: &str) -> Rect {
            self.buttons
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, r)| *r)
                .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.frame_after(0.05, events);
        }

        fn frame_after(&mut self, dt: f64, events: Vec<Event>) {
            self.time += dt;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(200.0, self.height),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let (editor, skin, named) = (&mut self.editor, &self.skin, self.named);
            let mut offset = 0.0;
            let mut output = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    offset = tools_scroll_area()
                        .show(ui, |ui| {
                            if named {
                                toolbar(ui, editor, None);
                            } else {
                                tool_column(ui, editor, skin);
                            }
                        })
                        .state
                        .offset
                        .y;
                });
            });
            output.textures_delta.clear();
            self.offset = offset;
            self.meshes.clear();
            self.texts.clear();
            for clipped in &output.shapes {
                collect_meshes(&clipped.shape, &mut self.meshes);
                collect_texts(&clipped.shape, &mut self.texts);
            }
            self.label_nodes = output
                .platform_output
                .accesskit_update
                .as_ref()
                .map(|update| {
                    update
                        .nodes
                        .iter()
                        .filter(|(_, n)| n.role() == egui::accesskit::Role::Label)
                        .filter_map(|(_, n)| n.label().or(n.value()).map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            self.textures = self.meshes.iter().map(|(t, _)| *t).collect();
            self.buttons = output
                .platform_output
                .accesskit_update
                .map(|update| {
                    update
                        .nodes
                        .iter()
                        .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
                        .filter_map(|(_, n)| {
                            let b = n.bounds()?;
                            let rect = Rect::from_min_max(
                                pos2(b.x0 as f32, b.y0 as f32),
                                pos2(b.x1 as f32, b.y1 as f32),
                            );
                            Some((n.label()?.to_string(), rect))
                        })
                        .collect()
                })
                .unwrap_or_default();
        }

        fn at(&self, label: &str) -> Pos2 {
            self.buttons
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, r)| r.center())
                .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
        }

        fn labels(&self) -> Vec<&str> {
            self.buttons.iter().map(|(l, _)| l.as_str()).collect()
        }

        fn press(&mut self, label: &str, button: PointerButton) -> Pos2 {
            let pos = self.at(label);
            self.frame(vec![Event::PointerMoved(pos)]);
            self.frame(vec![mouse(pos, button, true)]);
            pos
        }

        fn click(&mut self, label: &str, button: PointerButton) {
            let pos = self.press(label, button);
            self.frame(vec![mouse(pos, button, false)]);
            self.frame(vec![]);
        }

        fn texture(&self, piece: &str) -> egui::TextureId {
            self.skin
                .piece(piece, crate::skin::WidgetState::Normal)
                .unwrap()
                .texture
                .id()
        }
    }

    fn collect_meshes(shape: &egui::Shape, out: &mut Vec<(egui::TextureId, Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_meshes(s, out)),
            egui::Shape::Mesh(mesh) => out.push((mesh.texture_id, mesh.calc_bounds())),
            _ => {}
        }
    }

    fn collect_texts(shape: &egui::Shape, out: &mut Vec<(String, Rect, Color32)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_texts(s, out)),
            egui::Shape::Text(text) => out.push((
                text.galley.text().to_owned(),
                text.visual_bounding_rect(),
                text.fallback_color,
            )),
            _ => {}
        }
    }

    /// Lo dibujado con `texture`, unido en un rect.
    fn drawn_union(h: &ColumnHarness, texture: egui::TextureId) -> Option<Rect> {
        h.meshes
            .iter()
            .filter(|(t, _)| *t == texture)
            .map(|(_, r)| *r)
            .reduce(|a, b| a.union(b))
    }

    #[test]
    fn with_pixel_ui_the_tooltip_is_the_bubble_pointing_at_the_button() {
        let mut h = ColumnHarness::new(600.0).pixel_style();
        let bubble = h.texture("tooltip.bubble");
        assert_eq!(drawn_union(&h, bubble), None, "no bubble before hovering");
        let button = h.hover("Hand");
        let drawn = drawn_union(&h, bubble).expect("the bubble is drawn");
        // La punta de la cola, 6 px a la derecha del botón (el aire del mockup), y su fila de
        // arriba en el centro del botón: el cuerpo empieza 8 filas antes.
        assert_eq!(drawn.left(), button.right() + 6.0, "{drawn:?} {button:?}");
        assert_eq!(drawn.top() + 8.0, button.center().y, "{drawn:?} {button:?}");
        // Una línea: el alto del globo del mockup.
        assert_eq!(drawn.height(), 20.0, "{drawn:?}");
        let (text, rect, color) = h
            .texts
            .iter()
            .find(|(t, ..)| t == "Hand (H)")
            .cloned()
            .unwrap_or_else(|| panic!("the tooltip text in {:?}", h.texts));
        assert_eq!(color, h.skin.colors().text, "{text}");
        // El texto va en el cuerpo (sin la cola de 8), con 6 px de aire a la izquierda.
        let body = Rect::from_min_max(drawn.min + Vec2::new(8.0, 0.0), drawn.max);
        assert!(body.contains_rect(rect), "{rect:?} in {body:?}");
        assert_eq!(rect.left(), body.left() + 6.0, "{rect:?} {body:?}");
        // El tooltip sigue teniendo su texto en el árbol de accesibilidad.
        assert!(
            h.label_nodes.iter().any(|l| l == "Hand (H)"),
            "{:?}",
            h.label_nodes
        );
    }

    #[test]
    fn with_pixel_ui_the_bubble_grows_with_the_second_line_of_the_groups() {
        let mut h = ColumnHarness::new(600.0).pixel_style();
        let button = h.hover("Rectangular Marquee");
        let drawn = drawn_union(&h, h.texture("tooltip.bubble")).expect("the bubble is drawn");
        assert_eq!(drawn.left(), button.right() + 6.0, "{drawn:?} {button:?}");
        assert_eq!(drawn.top() + 8.0, button.center().y, "{drawn:?} {button:?}");
        assert!(drawn.height() > 20.0, "two lines: {drawn:?}");
        let text = tool_group::tooltip(ToolKind::RectSelect, true);
        let (_, rect, _) = h
            .texts
            .iter()
            .find(|(t, ..)| *t == text)
            .cloned()
            .unwrap_or_else(|| panic!("the tooltip text in {:?}", h.texts));
        let body = Rect::from_min_max(drawn.min + Vec2::new(8.0, 0.0), drawn.max);
        assert!(body.contains_rect(rect), "{rect:?} in {body:?}");
        assert_eq!(rect.left(), body.left() + 6.0, "{rect:?} {body:?}");
    }

    #[test]
    fn without_pixel_ui_the_tooltip_is_the_egui_one_and_there_is_no_bubble() {
        let mut h = ColumnHarness::new(600.0).named();
        let hand = h
            .labels()
            .into_iter()
            .find(|l| l.contains("Hand"))
            .expect("the Hand button")
            .to_owned();
        h.hover(&hand);
        assert!(
            h.texts.iter().any(|(t, ..)| t == "Hand (H)"),
            "the egui tooltip: {:?}",
            h.texts
        );
        assert_eq!(drawn_union(&h, h.texture("tooltip.bubble")), None);
    }

    /// Nombre accesible de una fila de la lista de variantes: el nombre y el atajo.
    fn item(kind: ToolKind) -> String {
        format!("{} {}", kind.label(), kind.shortcut())
    }

    #[test]
    fn the_icon_column_has_a_named_button_per_group_and_a_click_picks() {
        let mut h = ColumnHarness::new(600.0);
        let mut column = h.buttons.clone();
        column.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
        let names: Vec<&str> = column.iter().map(|(l, _)| l.as_str()).collect();
        let expected: Vec<&str> = ToolKind::GROUPS.iter().map(|g| g[0].label()).collect();
        assert_eq!(names, expected);
        // Los tramos del mockup: 23×22 y el último, con el borde de abajo, 23×23.
        let last = column.len() - 1;
        for (i, (label, r)) in column.iter().enumerate() {
            let height = if i == last { 23.0 } else { 22.0 };
            assert_eq!(r.size(), Vec2::new(23.0, height), "{label}");
        }
        h.click("Paint Bucket", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Bucket);
    }

    /// Botones de la columna de arriba abajo.
    fn column(h: &ColumnHarness) -> Vec<(String, Rect)> {
        let mut column = h.buttons.clone();
        column.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
        column
    }

    #[test]
    fn the_buttons_are_stacked_without_gaps_in_one_column() {
        let skins = |ctx: &egui::Context| {
            [
                Skin::load(ctx, crate::skin::EMBEDDED).unwrap(),
                Skin::fallback(),
            ]
        };
        for i in 0..2 {
            let ctx = egui::Context::default();
            let skin = skins(&ctx).into_iter().nth(i).unwrap();
            let h = ColumnHarness::with_skin(ctx, skin, 600.0);
            let column = column(&h);
            assert_eq!(column.len(), ToolKind::GROUPS.len());
            for pair in column.windows(2) {
                let ((_, above), (label, below)) = (&pair[0], &pair[1]);
                assert_eq!(below.top(), above.bottom(), "{label}: no gap ({i})");
                assert_eq!(below.left(), above.left(), "{label}: one column ({i})");
            }
        }
    }

    #[test]
    fn the_first_button_is_rounded_on_top_the_last_below_and_the_active_one_is_brown() {
        let mut h = ColumnHarness::new(600.0);
        let column = column(&h);
        // Los botones que se dibujaron enteros con `piece` (sus partes del 9-slice, unidas, cubren
        // el botón) y si quedan partes de `piece` fuera de los botones.
        let drawn = |h: &ColumnHarness, piece: &str| -> Vec<Rect> {
            let id = h.texture(piece);
            let parts: Vec<Rect> = h
                .meshes
                .iter()
                .filter(|(t, _)| *t == id)
                .map(|(_, r)| *r)
                .collect();
            assert!(
                parts
                    .iter()
                    .all(|p| column.iter().any(|(_, r)| r.contains_rect(*p))),
                "{piece}: {parts:?}"
            );
            column
                .iter()
                .map(|(_, r)| *r)
                .filter(|r| {
                    let inside = parts.iter().filter(|p| r.contains_rect(**p));
                    inside.copied().reduce(|a, b| a.union(b)) == Some(*r)
                })
                .collect()
        };
        let rect = |i: usize| column[i].1;
        let last = column.len() - 1;
        // Con el lápiz (un grupo del medio): el primer tramo arriba, el último abajo y los del
        // medio con `tool.slot`, salvo el del lápiz, que va activo.
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        let pencil = column.iter().position(|(l, _)| l == "Pencil").unwrap();
        assert_eq!(drawn(&h, "tool.slot.first"), [rect(0)]);
        assert_eq!(drawn(&h, "tool.slot.last"), [rect(last)]);
        assert_eq!(drawn(&h, "tool.slot.active"), [rect(pencil)]);
        let middle: Vec<Rect> = (1..last).filter(|&i| i != pencil).map(rect).collect();
        assert_eq!(drawn(&h, "tool.slot"), middle);
        assert!(drawn(&h, "tool.slot.first.active").is_empty());
        assert!(drawn(&h, "tool.slot.last.active").is_empty());
        // Activos en las puntas: `.first.active` y `.last.active`.
        h.click("Rectangular Marquee", PointerButton::Primary);
        assert_eq!(drawn(&h, "tool.slot.first.active"), [rect(0)]);
        assert!(drawn(&h, "tool.slot.first").is_empty());
        assert!(drawn(&h, "tool.slot.active").is_empty());
        h.click("Artboard", PointerButton::Primary);
        assert_eq!(drawn(&h, "tool.slot.last.active"), [rect(last)]);
        assert!(drawn(&h, "tool.slot.last").is_empty());
    }

    #[test]
    fn the_column_goes_in_the_toolbar_box_with_one_pixel_of_air() {
        let h = ColumnHarness::new(600.0);
        let column = column(&h);
        let id = h.texture("toolbar.box");
        let frame = h
            .meshes
            .iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, r)| *r)
            .reduce(|a, b| a.union(b))
            .expect("the box is drawn");
        let (first, last) = (column[0].1, column[column.len() - 1].1);
        assert_eq!(first.min - frame.min, Vec2::splat(2.0), "{frame:?}");
        assert_eq!(frame.max - last.max, Vec2::splat(2.0), "{frame:?}");
        // El mockup: 27 × 291.
        assert_eq!(frame.size(), Vec2::new(27.0, 291.0));
    }

    #[test]
    fn right_click_lists_the_five_marquee_tools_and_holding_opens_the_list_too() {
        use egui::Popup;
        let mut h = ColumnHarness::new(600.0);
        h.click("Rectangular Marquee", PointerButton::Secondary);
        assert!(Popup::is_any_open(&h.ctx));
        for &kind in ToolKind::GROUPS[0] {
            assert!(h.labels().contains(&item(kind).as_str()), "{kind:?}");
        }
        assert_eq!(h.editor.tool, ToolKind::Pencil, "right-click does not pick");
        h.frame(vec![escape()]);
        h.frame(vec![]);
        assert!(!Popup::is_any_open(&h.ctx));

        // Mantener presionado 400 ms abre la lista.
        h.press("Rectangular Marquee", PointerButton::Primary);
        h.frame_after(0.45, vec![]);
        h.frame(vec![]);
        assert!(Popup::is_any_open(&h.ctx), "held for 400 ms");
    }

    #[test]
    fn the_group_shows_and_picks_the_variant_chosen_in_the_list() {
        let mut h = ColumnHarness::new(600.0);
        h.click("Rectangular Marquee", PointerButton::Secondary);
        h.click(&item(ToolKind::Lasso), PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Lasso);
        h.click("Pencil", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        assert!(h.textures.contains(&h.texture("icon.lasso")));
        assert!(!h.textures.contains(&h.texture("icon.marquee")));
        // El botón del grupo ahora es Lasso: un clic vuelve a Lasso.
        h.click("Lasso", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Lasso);
    }

    /// Ids de los botones de la columna (por nombre), con o sin un widget antes de la columna.
    fn column_ids(prefix: bool) -> Vec<(String, egui::accesskit::NodeId)> {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                if prefix {
                    let _ = ui.button("Before");
                }
                tool_column(ui, &mut editor, &skin);
            });
        });
        output.textures_delta.clear();
        let mut ids: Vec<_> = output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
            .filter_map(|(id, n)| Some((n.label()?.to_string(), *id)))
            .filter(|(l, _)| l != "Before")
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn each_group_button_has_its_own_id_that_does_not_depend_on_what_comes_before() {
        let plain = column_ids(false);
        assert_eq!(plain.len(), ToolKind::GROUPS.len());
        let mut unique: Vec<_> = plain.iter().map(|(_, id)| *id).collect();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), plain.len());
        assert_eq!(plain, column_ids(true));
    }

    #[test]
    fn dragging_over_a_tool_button_does_not_scroll_the_panel() {
        let mut h = ColumnHarness::new(150.0);
        // Con una pantalla táctil, egui arrastra el contenido de un ScrollArea por defecto.
        let touch = |phase| Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos: pos2(199.0, 1.0),
            force: None,
        };
        h.frame(vec![
            touch(egui::TouchPhase::Start),
            touch(egui::TouchPhase::End),
        ]);
        assert!(h.ctx.input(|i| i.has_touch_screen()));
        let from = h.press("Pencil", PointerButton::Primary);
        for dy in [10.0, 25.0, 40.0] {
            h.frame(vec![Event::PointerMoved(from - Vec2::new(0.0, dy))]);
        }
        h.frame(vec![mouse(
            from - Vec2::new(0.0, 40.0),
            PointerButton::Primary,
            false,
        )]);
        h.frame(vec![]);
        assert_eq!(h.offset, 0.0, "the panel did not scroll");
    }

    #[test]
    fn holding_a_tool_button_in_the_scrolling_panel_opens_its_variants() {
        let mut h = ColumnHarness::new(150.0);
        h.press("Pencil", PointerButton::Primary);
        h.frame_after(0.45, vec![]);
        h.frame(vec![]);
        assert!(egui::Popup::is_any_open(&h.ctx));
        assert!(h.labels().contains(&item(ToolKind::Spray).as_str()));
        assert_eq!(h.offset, 0.0);
    }

    /// Nombre (o texto, en las etiquetas) y rect de cada nodo del árbol de accesibilidad.
    fn nodes(output: &egui::FullOutput) -> Vec<(String, Rect)> {
        let Some(update) = &output.platform_output.accesskit_update else {
            return Vec::new();
        };
        update
            .nodes
            .iter()
            .filter_map(|(_, n)| {
                let b = n.bounds()?;
                let text = n.label().or_else(|| n.value())?.to_string();
                let rect = Rect::from_min_max(
                    pos2(b.x0 as f32, b.y0 as f32),
                    pos2(b.x1 as f32, b.y1 as f32),
                );
                Some((text, rect))
            })
            .collect()
    }

    /// La barra de opciones en su panel de arriba, como en la app, manejada con eventos crudos (el
    /// reloj avanza 50 ms por frame). Guarda el rect del panel y los nodos de accesibilidad.
    struct BarHarness {
        ctx: egui::Context,
        editor: Editor,
        /// Con Pixel UI: el skin embebido (y el estilo de `apply_style` con sus colores).
        skin: Option<Skin>,
        time: f64,
        width: f32,
        bar: Rect,
        nodes: Vec<(String, Rect)>,
        /// Lo dibujado en el último frame.
        shapes: Vec<egui::epaint::ClippedShape>,
    }

    impl BarHarness {
        fn new(width: f32) -> Self {
            Self::with_skin(width, false)
        }

        /// Con `pixel`, la barra de Pixel UI: el estilo con los colores del skin y la fuente
        /// pixel, y el skin embebido.
        fn with_skin(width: f32, pixel: bool) -> Self {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let skin = pixel.then(|| Skin::embedded(&ctx));
            if let Some(skin) = &skin {
                ctx.set_fonts(crate::appearance::font_definitions(true));
                crate::appearance::apply_style(&ctx, Some(skin.colors()));
            }
            let mut h = Self {
                ctx,
                editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
                skin,
                time: 0.0,
                width,
                bar: Rect::NOTHING,
                nodes: Vec::new(),
                shapes: Vec::new(),
            };
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.time += 0.05;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(self.width, 300.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let editor = &mut self.editor;
            let skin = self.skin.as_ref();
            let mut bar = Rect::NOTHING;
            let mut output = self.ctx.run_ui(input, |ui| {
                bar = options_bar_panel(ui.style(), skin)
                    .show(ui, |ui| tool_options_bar(ui, editor, skin))
                    .response
                    .rect;
            });
            output.textures_delta.clear();
            self.bar = bar;
            self.nodes = nodes(&output);
            self.shapes = output.shapes;
        }

        /// Los rects de color liso dibujados (los separadores grabados).
        fn colored_rects(&self) -> Vec<(Color32, Rect)> {
            self.shapes
                .iter()
                .flat_map(|c| pixel_ui::colored_rects(&c.shape))
                .collect()
        }

        /// Las líneas de egui del color de sus separadores (`ui.separator()`, la del panel).
        fn separator_lines(&self) -> usize {
            let color = self
                .ctx
                .global_style()
                .visuals
                .widgets
                .noninteractive
                .bg_stroke
                .color;
            fn count(shape: &egui::Shape, color: Color32) -> usize {
                match shape {
                    egui::Shape::LineSegment { stroke, .. } => usize::from(stroke.color == color),
                    egui::Shape::Vec(shapes) => shapes.iter().map(|s| count(s, color)).sum(),
                    _ => 0,
                }
            }
            self.shapes.iter().map(|c| count(&c.shape, color)).sum()
        }

        fn has(&self, text: &str) -> bool {
            self.nodes.iter().any(|(t, _)| t == text)
        }

        /// Clic con el botón principal en el centro del nodo `text`, en tres frames.
        fn click(&mut self, text: &str) {
            let at = self.rect(text).center();
            let press = |pressed| Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![Event::PointerMoved(at)]);
            self.frame(vec![press(true)]);
            self.frame(vec![press(false)]);
        }

        fn rect(&self, text: &str) -> Rect {
            self.nodes
                .iter()
                .find(|(t, _)| t == text)
                .map(|(_, r)| *r)
                .unwrap_or_else(|| panic!("no {text:?} in {:?}", self.nodes))
        }

        /// Dónde se dibujó la pieza `name` del skin, de izquierda a derecha.
        fn drawn(&self, name: &str) -> Vec<Rect> {
            let skin = self.skin.as_ref().expect("with Pixel UI");
            let id = skin
                .piece(name, crate::skin::WidgetState::Normal)
                .unwrap()
                .texture
                .id();
            let mut meshes = Vec::new();
            for clipped in &self.shapes {
                collect_meshes(&clipped.shape, &mut meshes);
            }
            let mut rects: Vec<Rect> = meshes
                .into_iter()
                .filter(|(t, _)| *t == id)
                .map(|(_, r)| r)
                .collect();
            rects.sort_by(|a, b| a.left().total_cmp(&b.left()));
            rects
        }

        /// Como `drawn`, con las partes de un 9-slice (que se tocan) unidas en un rect.
        fn drawn_boxes(&self, name: &str) -> Vec<Rect> {
            let mut boxes: Vec<Rect> = Vec::new();
            for rect in self.drawn(name) {
                match boxes.iter_mut().find(|b| b.intersects(rect)) {
                    Some(b) => *b = b.union(rect),
                    None => boxes.push(rect),
                }
            }
            boxes
        }

        /// Los textos dibujados en el último frame: texto, tinta (para la fuente pixel, el rect
        /// visible) y color.
        fn texts(&self) -> Vec<(String, Rect, Color32)> {
            let mut texts = Vec::new();
            for clipped in &self.shapes {
                collect_texts(&clipped.shape, &mut texts);
            }
            texts
        }

        /// El color y la tinta del primer texto `text` dibujado.
        fn text(&self, text: &str) -> (Color32, Rect) {
            self.texts()
                .into_iter()
                .find(|(t, _, _)| t == text)
                .map(|(_, r, c)| (c, r))
                .unwrap_or_else(|| panic!("no text {text:?} in {:?}", self.texts()))
        }

        /// Clic en el combo `text` y dos frames más: la lista se mide en el primero (invisible) y
        /// se muestra en el segundo.
        fn open(&mut self, text: &str) {
            self.click(text);
            self.frame(vec![]);
            self.frame(vec![]);
        }

        /// Arrastre con el botón principal desde `from`, `dx` puntos a la derecha en 4 pasos.
        fn drag(&mut self, from: Pos2, dx: f32) {
            let press = |pos, pressed| Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![Event::PointerMoved(from)]);
            self.frame(vec![press(from, true)]);
            let mut at = from;
            for _ in 0..4 {
                at.x += dx / 4.0;
                self.frame(vec![Event::PointerMoved(at)]);
            }
            self.frame(vec![press(at, false)]);
            self.frame(vec![]);
        }

        /// Clic con el botón principal en `at`, en tres frames.
        fn click_at(&mut self, at: Pos2) {
            let press = |pressed| Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![Event::PointerMoved(at)]);
            self.frame(vec![press(true)]);
            self.frame(vec![press(false)]);
        }

        /// Escribe `text` y aprieta Enter.
        fn type_and_enter(&mut self, text: &str) {
            self.frame(vec![Event::Text(text.to_owned())]);
            let key = |pressed| Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            };
            self.frame(vec![key(true)]);
            self.frame(vec![key(false)]);
            self.frame(vec![]);
        }
    }

    #[test]
    fn the_options_bar_has_the_brush_with_the_pencil_but_not_with_the_hand() {
        let mut h = BarHarness::new(1200.0);
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        for text in ["Brush size", "Round", "Square", "Pixel-perfect"] {
            assert!(h.has(text), "{text}");
        }
        // Etiqueta y control uno al lado del otro: la forma va a la derecha del texto.
        assert!(h.rect("Round").left() > h.rect("Brush size").right());
        assert!(h.has("Vertical axis (left ↔ right)"), "Symmetry");
        h.editor.set_tool(ToolKind::Hand);
        h.frame(vec![]);
        for text in ["Brush size", "Round", "Square"] {
            assert!(!h.has(text), "{text}");
        }
        assert!(
            h.has("Vertical axis (left ↔ right)"),
            "Symmetry is always there"
        );
        assert!(h.has("Symmetry"));
    }

    #[test]
    fn symmetry_is_at_the_right_end_of_the_bar_in_the_usual_order() {
        let h = BarHarness::new(1200.0);
        let names: Vec<&str> = SYMMETRY_AXES.iter().map(|(_, tip)| *tip).collect();
        let rects: Vec<Rect> = names.iter().map(|n| h.rect(n)).collect();
        for pair in rects.windows(2) {
            assert!(pair[0].right() <= pair[1].left(), "{names:?}: {rects:?}");
        }
        let last = rects[rects.len() - 1];
        assert!(
            h.bar.right() - last.right() < 12.0,
            "{last:?} in {:?}",
            h.bar
        );
        assert!(h.rect("Symmetry").right() <= rects[0].left());
    }

    #[test]
    fn each_symmetry_button_turns_its_own_axis_on_and_off() {
        let mut h = BarHarness::new(1200.0);
        let [vertical, horizontal, down, up] = SYMMETRY_AXES.map(|(_, name)| name);
        h.click(vertical);
        h.click(horizontal);
        let axes = h.editor.symmetry().axes;
        assert!(axes.vertical && axes.horizontal, "{axes:?}");
        assert!(!axes.diagonal_down && !axes.diagonal_up, "{axes:?}");
        h.click(up);
        h.click(vertical);
        assert_eq!(
            h.editor.symmetry().axes,
            SymmetryAxes {
                horizontal: true,
                diagonal_up: true,
                ..SymmetryAxes::NONE
            }
        );
        assert!(!h.editor.symmetry().axes.diagonal_down, "{down} untouched");
        // Cada clic es un paso de undo.
        h.editor.undo();
        assert!(h.editor.symmetry().axes.vertical);
    }

    #[test]
    fn with_pixel_ui_the_symmetry_buttons_are_the_skin_pieces_and_toggle_their_axis() {
        let mut h = BarHarness::with_skin(1200.0, true);
        let [vertical, horizontal, down, up] = SYMMETRY_AXES.map(|(_, name)| name);
        // Los botones miden lo que la pieza `sym.slot` (15×16, la del mockup) y la forma del
        // pincel, 13×13.
        assert_eq!(h.rect(vertical).size(), Vec2::new(15.0, 16.0));
        assert_eq!(h.rect("Round").size(), Vec2::splat(13.0));
        // En orden y del mismo tamaño, con 2 de aire entre uno y otro.
        let buttons = [vertical, horizontal, down, up].map(|name| h.rect(name));
        for pair in buttons.windows(2) {
            assert_eq!(pair[1].left() - pair[0].right(), 2.0, "{pair:?}");
        }
        // Como en el mockup (frame 4): un botón cada 17 px, 2 filas debajo del borde de arriba
        // de la barra y 1 arriba del grabado de abajo, en puntos enteros (con la escala entera,
        // píxeles del skin enteros). Los íconos, de 11×11, 2 px adentro.
        let slots = h.drawn("sym.slot");
        assert_eq!(slots.len(), 4, "{slots:?}");
        for pair in slots.windows(2) {
            assert_eq!(pair[1].left() - pair[0].right(), 2.0, "{pair:?}");
        }
        let names = ["vertical", "horizontal", "diag_down", "diag_up"];
        for (slot, name) in slots.iter().zip(names) {
            assert_eq!(slot.top(), h.bar.top() + 2.0, "{slot:?} in {:?}", h.bar);
            assert_eq!(slot.bottom(), h.bar.bottom() - 2.0 - 1.0, "{slot:?}");
            assert_eq!(slot.left(), slot.left().round(), "{slot:?}");
            let icon = h.drawn(&format!("icon.sym_{name}"));
            let expected = Rect::from_min_size(slot.min + Vec2::splat(2.0), Vec2::splat(11.0));
            assert_eq!(icon, vec![expected], "{name}");
        }
        // De la tinta de "Symmetry" al cuadro del primer botón, 5 px: el texto deja 1 px vacío y
        // la pieza empieza con una columna transparente, así que entre los rects quedan 3. Del
        // último al borde derecho, 3 (en el mockup, la tinta termina en la x 903, los cuadros
        // van de la 909 a la 974 y el área de la app termina en la 977). Las mayúsculas, en la
        // fila 5 de la barra (y 76).
        assert_eq!(slots[0].left() - h.rect("Symmetry").right(), 3.0);
        let (_, ink) = h.text("Symmetry");
        assert_eq!(slots[0].left() + 1.0 - ink.right(), 5.0, "{ink:?}");
        assert_eq!(ink.top(), h.bar.top() + 5.0, "{ink:?}");
        assert_eq!(h.bar.right() - slots[3].right(), 3.0, "{:?}", h.bar);
        // A 2×, igual: sin medio punto (que sería medio píxel del skin).
        let mut double = BarHarness::with_skin(1200.0, true);
        double.ctx.set_pixels_per_point(2.0);
        double.frame(vec![]);
        double.frame(vec![]);
        for slot in double.drawn("sym.slot") {
            assert_eq!(slot.top(), double.bar.top() + 2.0, "{slot:?}");
        }
        h.click(vertical);
        h.click(horizontal);
        let axes = h.editor.symmetry().axes;
        assert!(axes.vertical && axes.horizontal, "{axes:?}");
    }

    #[test]
    fn the_options_bar_is_one_row_of_the_same_height_with_every_tool() {
        let mut h = BarHarness::new(800.0);
        for &kind in ToolKind::GROUPS.iter().flat_map(|g| g.iter()) {
            h.editor.set_tool(kind);
            h.frame(vec![]);
            assert_eq!(h.bar.height(), OPTIONS_BAR_HEIGHT, "{kind:?}");
            for (text, rect) in &h.nodes {
                assert!(
                    rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom(),
                    "{kind:?}: {text:?} {rect:?} out of {:?}",
                    h.bar
                );
            }
        }
    }

    #[test]
    fn with_pixel_ui_the_bar_is_21_and_every_node_fits_inside_it() {
        let mut h = BarHarness::with_skin(800.0, true);
        for kind in [
            ToolKind::Pencil,
            ToolKind::RectSelect,
            ToolKind::MagicWand,
            ToolKind::Text,
            ToolKind::Gradient,
        ] {
            h.editor.set_tool(kind);
            h.frame(vec![]);
            h.frame(vec![]);
            assert_eq!(h.bar.height(), 21.0, "{kind:?}");
            assert!(h.nodes.len() > 2, "{kind:?}: {:?}", h.nodes);
            for (text, rect) in &h.nodes {
                assert!(
                    rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom(),
                    "{kind:?}: {text:?} {rect:?} out of {:?}",
                    h.bar
                );
            }
            // Las piezas de los controles, en las 19 filas de arriba del grabado.
            for piece in [
                "field.combo",
                "field.number",
                "field.unit",
                "field.check.on",
                "sym.slot",
                "sym.slot.active",
                "button",
                "button.hover",
                "button.down",
            ] {
                for rect in h.drawn(piece) {
                    assert!(
                        rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom() - 2.0,
                        "{kind:?}: {piece} {rect:?} out of {:?}",
                        h.bar
                    );
                }
            }
            // Las herramientas con grupos de radios (Sample, Linear/Radial, Rotation) los
            // dibujan como botones: la prueba de arriba no es vacía para ellos.
            if kind != ToolKind::Pencil && kind != ToolKind::Text {
                assert!(!h.drawn("button.down").is_empty(), "{kind:?}: no buttons");
            }
        }
    }

    /// La barra con Pixel UI y `tool`, después de dos frames (los textos y las piezas se
    /// acomodan en el segundo).
    fn pixel_bar(tool: ToolKind) -> BarHarness {
        let mut h = BarHarness::with_skin(1200.0, true);
        h.editor.set_tool(tool);
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }

    /// Las piezas de los modos: `sym.slot` y `sym.slot.active` a la izquierda de Symmetry.
    fn mode_slots(h: &BarHarness) -> (Vec<Rect>, Vec<Rect>) {
        let left_of_symmetry = |r: &Rect| r.right() < h.rect("Symmetry").left();
        let normal = h.drawn("sym.slot").into_iter().filter(left_of_symmetry);
        let active = h
            .drawn("sym.slot.active")
            .into_iter()
            .filter(left_of_symmetry);
        (normal.collect(), active.collect())
    }

    const MODE_ICONS: [&str; 4] = [
        "icon.sel_replace",
        "icon.sel_add",
        "icon.sel_subtract",
        "icon.sel_intersect",
    ];

    #[test]
    fn with_pixel_ui_the_marquee_mode_is_four_icon_buttons_like_in_the_design() {
        let h = pixel_bar(ToolKind::RectSelect);
        let bar = h.bar;
        // Como en el diseño (frame 4, barra desde (28,71)): la tinta de "Mode" en x 35..54 y las
        // mayúsculas en la y 76, en el color del texto.
        let colors = *h.skin.as_ref().unwrap().colors();
        let (color, ink) = h.text("Mode");
        assert_eq!(color, colors.text);
        assert_eq!(ink.min, bar.min + Vec2::new(7.0, 5.0), "{ink:?}");
        assert_eq!(ink.width(), 20.0);
        // Los cuatro cuadritos: las piezas en x 60, 77, 94 y 111 y en la y 73 (la pieza tiene
        // una columna transparente antes del cuadro, que va en 61). Replace es el elegido.
        let (normal, active) = mode_slots(&h);
        let expected: Vec<Rect> = [32.0, 49.0, 66.0, 83.0]
            .map(|x| Rect::from_min_size(bar.min + Vec2::new(x, 2.0), Vec2::new(15.0, 16.0)))
            .into();
        assert_eq!(active, vec![expected[0]]);
        assert_eq!(normal, expected[1..].to_vec());
        // Cada ícono, de 11×11, en el interior de su cuadro (2 px adentro de la pieza).
        for (icon, slot) in MODE_ICONS.into_iter().zip(&expected) {
            let at = Rect::from_min_size(slot.min + Vec2::splat(2.0), Vec2::splat(11.0));
            assert_eq!(h.drawn(icon), vec![at], "{icon}");
        }
        // Un botón por modo, con su nombre; ya no están los radios.
        for mode in SelectionMode::ALL {
            assert_eq!(h.rect(mode.label()).size(), Vec2::new(15.0, 16.0));
            let radio_text = h.texts().into_iter().any(|(t, _, _)| t == mode.label());
            assert!(!radio_text, "{mode:?}: no radio text");
        }
        // Del último cuadro al grupo de Rotation, el aire de entre grupos.
        assert_eq!(h.rect("Rotation:").left() - expected[3].right(), 14.0);
    }

    #[test]
    fn with_pixel_ui_clicking_a_mode_icon_picks_that_mode_only() {
        let mut h = pixel_bar(ToolKind::RectSelect);
        assert_eq!(h.editor.options.selection_mode, SelectionMode::Replace);
        let subtract = SelectionMode::Subtract.label();
        let at = h.rect(subtract);
        h.click(subtract);
        h.frame(vec![]);
        assert_eq!(h.editor.options.selection_mode, SelectionMode::Subtract);
        // El marco activo pasa al botón elegido; los otros tres, normales.
        let (normal, active) = mode_slots(&h);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].left(), at.left().round());
        assert_eq!(normal.len(), 3);
        // Otro clic en el mismo no lo apaga: siempre hay un modo, como con radios.
        h.click(subtract);
        assert_eq!(h.editor.options.selection_mode, SelectionMode::Subtract);
        h.click(SelectionMode::Intersect.label());
        assert_eq!(h.editor.options.selection_mode, SelectionMode::Intersect);
    }

    #[test]
    fn with_pixel_ui_the_mode_buttons_have_the_mode_name_as_tooltip() {
        let mut h = pixel_bar(ToolKind::MagicWand);
        let add = SelectionMode::Add.label();
        // El botón no tiene texto: el nombre aparece solo en el tooltip.
        let drawn = |h: &BarHarness| h.texts().into_iter().any(|(t, _, _)| t == add);
        assert!(!drawn(&h), "no text before hovering");
        let at = h.rect(add).center();
        h.frame(vec![Event::PointerMoved(at)]);
        for _ in 0..20 {
            h.frame(vec![]);
        }
        assert!(drawn(&h), "tooltip {add:?} in {:?}", h.texts());
    }

    #[test]
    fn with_pixel_ui_brush_size_is_the_number_field_with_its_px_unit() {
        let mut h = pixel_bar(ToolKind::Pencil);
        let bar = h.bar;
        // "Brush Size" en x 227..274 del diseño (7 desde el borde) y, 3 px después de la tinta,
        // el campo de x 278 a 304 y la unidad "px" hasta la 319: 42 de ancho en total, de la y
        // 73 a la 88.
        let (_, ink) = h.text("Brush Size");
        assert_eq!(ink.min, bar.min + Vec2::new(7.0, 5.0), "{ink:?}");
        assert_eq!(ink.width(), 48.0);
        let field = h.drawn_boxes("field.number");
        let left = ink.right() + 3.0;
        assert_eq!(
            field,
            vec![Rect::from_min_size(
                pos2(left, bar.top() + 2.0),
                Vec2::new(27.0, 16.0)
            )]
        );
        let unit = h.drawn("field.unit");
        assert_eq!(
            unit,
            vec![Rect::from_min_size(
                pos2(left + 25.0, bar.top() + 2.0),
                Vec2::new(17.0, 16.0)
            )]
        );
        // El número, en la fila de los textos (los dígitos miden 6, una fila menos que las
        // mayúsculas, y apoyan en la misma línea: terminan en la fila 11) y adentro del blanco.
        let size = h.editor.options.brush_size;
        let (color, digits) = h.text(&size.to_string());
        assert_eq!(color, h.skin.as_ref().unwrap().colors().text);
        assert_eq!(digits.bottom(), bar.top() + 12.0, "{digits:?}");
        assert!(digits.left() > left + 1.0 && digits.right() < left + 26.0);
        // Arrastrar a la derecha lo agranda.
        h.drag(field[0].center(), 40.0);
        let dragged = h.editor.options.brush_size;
        assert!(dragged > size, "{size} -> {dragged}");
        // Un clic lo deja escribir: el número escrito reemplaza al de antes.
        h.click_at(field[0].center());
        h.type_and_enter("12");
        assert_eq!(h.editor.options.brush_size, 12);
        // Fuera del rango, el máximo.
        h.click_at(field[0].center());
        h.type_and_enter("500");
        assert_eq!(h.editor.options.brush_size, MAX_BRUSH_SIZE);
    }

    #[test]
    fn with_pixel_ui_fields_without_a_unit_have_no_px() {
        let h = pixel_bar(ToolKind::MagicWand);
        // Tolerance no tiene unidad: el campo solo.
        let (_, ink) = h.text("Tolerance");
        let field = h.drawn_boxes("field.number");
        assert_eq!(field.len(), 1, "{field:?}");
        assert_eq!(field[0].left(), ink.right() + 3.0);
        assert_eq!(field[0].size(), Vec2::new(27.0, 16.0));
        assert_eq!(h.drawn("field.unit"), vec![]);
        // Spray: el ancho en px y la velocidad sin unidad.
        let h = pixel_bar(ToolKind::Spray);
        assert_eq!(
            h.drawn_boxes("field.number").len(),
            3,
            "brush, width and speed"
        );
        assert_eq!(h.drawn("field.unit").len(), 2, "brush and width");
    }

    #[test]
    fn with_pixel_ui_a_combo_is_the_field_combo_and_picking_an_option_works() {
        let mut h = pixel_bar(ToolKind::Gradient);
        let bar = h.bar;
        // El tramado es una lista de verdad: el único combo del diseño, de la y 73 a la 88 (la
        // forma, Linear/Radial, son botones).
        let combos = h.drawn_boxes("field.combo");
        assert_eq!(combos.len(), 1, "{combos:?}");
        // El texto, 6 px adentro y en la fila de los textos; 5 px de su tinta a la parte de la
        // flecha, las últimas 10 columnas (en el diseño, x 128..212, tinta en 134..197).
        let current = h.editor.options.dither;
        let (_, ink) = h.text(current.label());
        let combo = combos[0];
        assert_eq!(combo.top(), bar.top() + 2.0);
        assert_eq!(combo.height(), 16.0);
        assert_eq!(ink.left() - combo.left(), 6.0);
        assert_eq!(ink.top(), bar.top() + 5.0);
        assert_eq!(combo.right() - ink.right(), 5.0 + 10.0);
        // Un clic abre la lista en el panel de los menús, colgada del combo.
        let other = Dither::ALL.into_iter().find(|d| *d != current).unwrap();
        assert!(!h.has(other.label()), "closed: only the chosen one");
        h.open(current.label());
        assert!(h.has(other.label()), "{:?}", h.nodes);
        let panel = h.drawn_boxes("menu.panel");
        assert_eq!(panel.len(), 1, "{panel:?}");
        assert_eq!(panel[0].left_top(), combo.left_bottom());
        assert!(panel[0].width() >= combo.width());
        h.click(other.label());
        assert_eq!(h.editor.options.dither, other);
        for _ in 0..10 {
            h.frame(vec![]);
        }
        assert_eq!(h.drawn_boxes("menu.panel"), vec![], "closed after picking");
        assert!(h.has(other.label()) && !h.has(current.label()));
    }

    /// Las cajas de los botones de opción dibujados: (normales, activos). Los normales pueden
    /// estar con el puntero encima (`button.hover`).
    fn option_buttons(h: &BarHarness) -> (Vec<Rect>, Vec<Rect>) {
        let mut normal = h.drawn_boxes("button");
        normal.extend(h.drawn_boxes("button.hover"));
        normal.sort_by(|a, b| a.left().total_cmp(&b.left()));
        (normal, h.drawn_boxes("button.down"))
    }

    /// Un grupo de radios de la barra con Pixel UI: la herramienta, sus opciones (el nombre de
    /// cada botón) y cuál está elegida.
    struct RadioGroup {
        tool: ToolKind,
        options: Vec<&'static str>,
        current: fn(&Editor) -> &'static str,
    }

    fn radio_groups() -> Vec<RadioGroup> {
        vec![
            RadioGroup {
                tool: ToolKind::MagicWand,
                options: vec!["Layer", "All layers"],
                current: |e| {
                    if e.options.sample_all {
                        "All layers"
                    } else {
                        "Layer"
                    }
                },
            },
            RadioGroup {
                tool: ToolKind::Gradient,
                options: vec!["Linear", "Radial"],
                current: |e| match e.options.gradient_shape {
                    GradientShape::Linear => "Linear",
                    GradientShape::Radial => "Radial",
                },
            },
            RadioGroup {
                tool: ToolKind::RectSelect,
                options: RotationAlgorithm::ALL.map(|r| r.label()).into(),
                current: |e| e.options.rotation.label(),
            },
        ]
    }

    /// La caja del botón `label`: la pieza dibujada debajo de su nodo.
    fn button_box(h: &BarHarness, label: &str) -> (Rect, bool) {
        let at = h.rect(label);
        let (normal, active) = option_buttons(h);
        let hit = |b: &Rect| b.x_range().contains(at.center().x);
        match active.iter().find(|b| hit(b)) {
            Some(b) => (*b, true),
            None => (
                *normal
                    .iter()
                    .find(|b| hit(b))
                    .unwrap_or_else(|| panic!("no button under {label:?}")),
                false,
            ),
        }
    }

    #[test]
    fn with_pixel_ui_each_radio_option_is_a_button_and_the_current_one_is_active() {
        for group in radio_groups() {
            let h = pixel_bar(group.tool);
            let bar = h.bar;
            let tool = group.tool;
            let current = (group.current)(&h.editor);
            let (normal, active) = option_buttons(&h);
            // Un botón por opción; solo el elegido con la pieza activa.
            let options: Vec<Rect> = group
                .options
                .iter()
                .map(|label| {
                    let (b, on) = button_box(&h, label);
                    assert_eq!(on, *label == current, "{tool:?}: {label}");
                    // En las filas de los campos (la y 73 a la 88 del diseño).
                    assert_eq!(b.top(), bar.top() + 2.0, "{tool:?}: {label} {b:?}");
                    assert_eq!(b.height(), 16.0, "{tool:?}: {label}");
                    // El texto, centrado en el botón y con las mayúsculas en la fila de los
                    // textos, en el color del texto.
                    let (color, ink) = h.text(label);
                    assert_eq!(color, h.skin.as_ref().unwrap().colors().text);
                    assert_eq!(ink.top(), bar.top() + 5.0, "{tool:?}: {label} {ink:?}");
                    assert_eq!(ink.center().x, b.center().x, "{tool:?}: {label}");
                    assert!(b.contains_rect(ink), "{tool:?}: {label}");
                    b
                })
                .collect();
            // En lo que ocupa el grupo (Magic Wand también tiene Rotation): un activo y los
            // demás normales, ni una pieza de más.
            let span = options.iter().fold(Rect::NOTHING, |a, b| a.union(*b));
            let inside = |boxes: &[Rect]| boxes.iter().filter(|b| span.intersects(**b)).count();
            assert_eq!(inside(&active), 1, "{tool:?}: {active:?}");
            assert_eq!(inside(&normal), options.len() - 1, "{tool:?}: {normal:?}");
            // En fila, sin solaparse.
            for pair in options.windows(2) {
                assert!(pair[1].left() > pair[0].right(), "{tool:?}: {pair:?}");
            }
            // Ya no hay combo para el grupo.
            for label in &group.options {
                let combo = h
                    .drawn_boxes("field.combo")
                    .into_iter()
                    .any(|c| c.contains(h.rect(label).center()));
                assert!(!combo, "{tool:?}: {label} in a combo");
            }
        }
    }

    #[test]
    fn with_pixel_ui_one_click_on_a_radio_button_picks_that_option() {
        for group in radio_groups() {
            let mut h = pixel_bar(group.tool);
            let tool = group.tool;
            for label in group.options.iter().rev().chain(&group.options) {
                h.click(label);
                assert_eq!((group.current)(&h.editor), *label, "{tool:?}");
                h.frame(vec![]);
                // La pieza activa pasa al elegido; los otros, normales.
                for other in &group.options {
                    assert_eq!(button_box(&h, other).1, other == label, "{tool:?}: {other}");
                }
            }
        }
    }

    #[test]
    fn with_pixel_ui_pixel_perfect_is_the_bar_checkbox_and_a_click_toggles_it() {
        let mut h = pixel_bar(ToolKind::Pencil);
        let bar = h.bar;
        // Sin marcar: la pieza no está dibujada y va el cuadro de respaldo.
        h.editor.options.pixel_perfect = false;
        h.frame(vec![]);
        assert_eq!(h.drawn("field.check.on"), vec![], "unchecked");
        let before = h.rect("Pixel Perfect");
        h.click("Pixel Perfect");
        h.frame(vec![]);
        assert!(h.editor.options.pixel_perfect);
        // Marcada: la casilla del diseño, 13×14 de la y 74 a la 87, 5 px después del control
        // anterior (en el diseño, del campo; acá, de los botones de forma), y la tinta del texto
        // 3 px después, en la fila de los textos.
        let check = h.drawn("field.check.on");
        let at = Rect::from_min_size(pos2(before.left(), bar.top() + 3.0), Vec2::new(13.0, 14.0));
        assert_eq!(check, vec![at]);
        assert_eq!(check[0].left() - h.rect("Square").right(), 5.0);
        let (_, ink) = h.text("Pixel Perfect");
        assert_eq!(ink.left() - check[0].right(), 3.0);
        assert_eq!(ink.top(), bar.top() + 5.0);
        h.click("Pixel Perfect");
        h.frame(vec![]);
        assert!(!h.editor.options.pixel_perfect);
        assert_eq!(h.drawn("field.check.on"), vec![]);
    }

    #[test]
    fn with_pixel_ui_every_text_of_the_bar_is_in_the_design_row() {
        for tool in [
            ToolKind::Pencil,
            ToolKind::RectSelect,
            ToolKind::MagicWand,
            ToolKind::Text,
            ToolKind::Gradient,
            ToolKind::Rectangle,
        ] {
            let h = pixel_bar(tool);
            // "Load font…" es un botón de egui: el diseño no tiene botones.
            for (text, ink, _) in h.texts().into_iter().filter(|(t, _, _)| t != "Load font…") {
                // Las mayúsculas en la fila 5 (los textos sin mayúsculas, como los números o
                // "px", no bajan de ahí).
                assert!(ink.top() >= h.bar.top() + 5.0, "{tool:?}: {text:?} {ink:?}");
                let caps = text.chars().any(|c| c.is_ascii_uppercase());
                if caps {
                    assert_eq!(ink.top(), h.bar.top() + 5.0, "{tool:?}: {text:?} {ink:?}");
                }
            }
        }
    }

    #[test]
    fn without_pixel_ui_the_bar_keeps_the_egui_controls() {
        let mut h = BarHarness::new(1200.0);
        assert!(h.has("Brush size") && h.has("Pixel-perfect"));
        assert!(!h.has("Brush Size") && !h.has("Pixel Perfect"));
        // El slider del pincel (su texto de valor) sigue ahí.
        h.editor.set_tool(ToolKind::RectSelect);
        h.frame(vec![]);
        // Los cuatro radios con su texto, y Rotation con los suyos.
        for mode in SelectionMode::ALL {
            assert!(
                h.texts().iter().any(|(t, _, _)| t == mode.label()),
                "{mode:?}"
            );
        }
        h.click(SelectionMode::Add.label());
        assert_eq!(h.editor.options.selection_mode, SelectionMode::Add);
        h.editor.set_tool(ToolKind::Gradient);
        h.frame(vec![]);
        assert!(h.has("Linear") && h.has("Radial"), "both radios");
        // Ninguna pieza del skin: no hay skin.
        assert!(h.skin.is_none());
    }

    #[test]
    fn with_pixel_ui_the_bar_ends_in_an_etched_line_and_symmetry_has_one_before_it() {
        let h = BarHarness::with_skin(1200.0, true);
        let colors = *h.skin.as_ref().unwrap().colors();
        let (dark, light) = (colors.canvas, colors.highlight);
        let rects = h.colored_rects();
        let bar = h.bar;
        assert_eq!(bar.height(), 21.0);
        // Abajo, a todo el ancho: la línea oscura en la fila 19 de la barra y el brillo en la 20
        // (en el mockup, frame 4, y 90 y 91 de la barra de y 71..91).
        let line = bar.bottom() - 2.0;
        let x = bar.x_range();
        for (color, y) in [(dark, line), (light, line + 1.0)] {
            let want = Rect::from_x_y_ranges(x, y..=y + 1.0);
            assert!(rects.contains(&(color, want)), "{want:?} in {rects:?}");
        }
        // Antes de "Symmetry", con 5 px entre el brillo y el texto (en el mockup, x 852/853 y
        // "Symmetry" desde x 859). Va desde el brillo de la línea de las pestañas (una fila
        // arriba de la barra) hasta la línea de abajo, sin tocarla: y 70..87 en el mockup.
        let symmetry = h.rect("Symmetry");
        let left = symmetry.left() - 5.0 - 2.0;
        let rows = egui::Rangef::new(bar.top() - 1.0, line);
        for (color, x) in [(dark, left), (light, left + 1.0)] {
            let want = Rect::from_x_y_ranges(x..=x + 1.0, rows);
            assert!(rects.contains(&(color, want)), "{want:?} in {rects:?}");
        }
    }

    #[test]
    fn with_pixel_ui_the_bar_has_one_etched_separator_and_no_egui_lines() {
        let mut h = BarHarness::with_skin(1200.0, true);
        let dark = h.skin.as_ref().unwrap().colors().canvas;
        for kind in [ToolKind::Pencil, ToolKind::MagicWand, ToolKind::Rectangle] {
            h.editor.set_tool(kind);
            h.frame(vec![]);
            h.frame(vec![]);
            assert_eq!(h.separator_lines(), 0, "{kind:?}");
            // Un solo vertical, el de Symmetry: el diseño (frame 4) no tiene líneas entre los
            // grupos de opciones, solo aire.
            let vertical = h
                .colored_rects()
                .into_iter()
                .filter(|(c, r)| *c == dark && r.width() == 1.0 && r.height() > 1.0)
                .count();
            assert_eq!(vertical, 1, "{kind:?}");
        }
        // Sin Pixel UI, los de siempre.
        let h = BarHarness::new(1200.0);
        assert!(h.separator_lines() > 0);
        let thin: Vec<_> = h
            .colored_rects()
            .into_iter()
            .filter(|(_, r)| r.width() == 1.0 || r.height() == 1.0)
            .collect();
        assert_eq!(thin, vec![], "no etched lines");
    }

    #[test]
    fn the_options_scroll_sideways_with_the_wheel_when_they_do_not_fit() {
        let mut h = BarHarness::new(320.0);
        let before = h.rect("Round");
        let over = pos2(40.0, h.bar.center().y);
        h.frame(vec![Event::PointerMoved(over)]);
        h.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -60.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        }]);
        for _ in 0..20 {
            h.frame(vec![]);
        }
        let after = h.rect("Round");
        assert!(after.left() < before.left(), "{before:?} -> {after:?}");
        assert_eq!(after.top(), before.top(), "still one row");
        assert!(
            h.rect("Vertical axis (left ↔ right)").right() <= 320.0,
            "Symmetry stays in view"
        );
    }

    #[test]
    fn the_toolbar_has_only_the_tool_buttons() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        for skin in [None, Some(&skin)] {
            let mut output = ctx.run_ui(RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| toolbar(ui, &mut editor, skin));
            });
            output.textures_delta.clear();
            let texts: Vec<String> = nodes(&output).into_iter().map(|(t, _)| t).collect();
            for gone in [
                "Options",
                "Symmetry",
                "Brush size",
                "Vertical axis (left ↔ right)",
            ] {
                assert!(!texts.iter().any(|t| t == gone), "{gone} in {texts:?}");
            }
            assert!(texts.iter().any(|t| t.contains("Pencil")), "{texts:?}");
        }
    }
}
