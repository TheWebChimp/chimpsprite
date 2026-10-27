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
use crate::editor::{Editor, MAX_BRUSH_SIZE, SelectionMode, ToolKind};
use crate::skin::Skin;
use crate::{color_panel, fg_bg, menus, pixel_ui, theme, tile_ops, tool_group, widgets};

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

/// El radio que vuelve al prender el redondeo si no se había usado otro.
const DEFAULT_CORNER_RADIUS: u32 = 4;

/// Alto de la barra de opciones, con su margen: el mismo con todas las herramientas, así el lienzo
/// no se mueve al cambiar de herramienta.
pub const OPTIONS_BAR_HEIGHT: f32 = 32.0;
/// Alto de la barra de opciones con Pixel UI: 23 px de skin. Son 19 filas de controles, 2 de aire
/// y, abajo, el separador grabado (la línea grabada de arriba es la de las pestañas). Los
/// controles, de 16 y su luz, van 2 filas abajo del borde.
pub const PIXEL_OPTIONS_BAR_HEIGHT: f32 = 23.0;
/// El aire de abajo de la barra con Pixel UI, entre los controles y el separador grabado.
const PIXEL_BAR_AIR: i8 = 2;
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
                bottom: PIXEL_BAR_AIR,
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
    painter.set_opacity(1.0);
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
    // Con un diálogo abierto la barra queda deshabilitada (egui la dibuja medio transparente):
    // las líneas grabadas se ven igual.
    if let Some(etch) = etch {
        let mut painter = ui.painter().clone();
        painter.set_opacity(1.0);
        painter.add(pixel_ui::etched_hline(
            bar.x_range(),
            line,
            &etch.colors,
            ui.ctx().pixels_per_point(),
        ));
    }
    text_window(ui.ctx(), editor, skin);
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
    // Las ayudas van al final: con Pixel UI, juntas detrás del ícono de info.
    let mut hints: Vec<&'static str> = Vec::new();
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
        size.tip("Ctrl+wheel over the canvas changes it");
        match look {
            // Como en el diseño: un botón con la forma al tamaño del pincel; un clic la cambia.
            Some(look) => {
                gap(ui, CONTROL_GAP);
                bar_fields::brush_shape(
                    ui,
                    look,
                    &mut options.brush_shape,
                    options.brush_size,
                    options.brush_colors,
                )
                .tip("Brush shape (click to switch Round / Square / Custom)");
            }
            None => {
                brush_shape_buttons(ui, &mut options.brush_shape, 20.0);
            }
        }
    }
    // El pincel de Edit → New Brush: con sus colores o con el primario.
    if options.brush_shape == cs_raster::BrushShape::Custom
        && matches!(
            tool,
            ToolKind::Pencil
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Polygon
        )
    {
        if look.is_some() {
            gap(ui, CONTROL_GAP);
        } else {
            groups.next(ui);
        }
        check_option(ui, look, &mut options.brush_colors, "Brush Colors");
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
                crate::widgets::checkbox(ui, &mut options.pixel_perfect, "Pixel-perfect");
            }
        }
    }
    if matches!(
        tool,
        ToolKind::Rectangle | ToolKind::Ellipse | ToolKind::Polygon
    ) {
        groups.next(ui);
        check_option(ui, look, &mut options.filled, "Filled");
    }
    if tool == ToolKind::Rectangle {
        groups.next(ui);
        let radius = match look {
            Some(look) => {
                // El botón del redondeo (en vez del nombre): lo prende y lo apaga; al prenderlo
                // vuelve el último radio.
                let on = options.corner_radius > 0;
                let toggle = bar_fields::at_field_top(ui, look, |ui| {
                    crate::pixel_ui::icon_button(
                        ui,
                        Some(look.skin),
                        "icon.corner_radius",
                        "Corner radius",
                        Some(on),
                    )
                });
                let last_id = egui::Id::new("last_corner_radius");
                if on {
                    ui.data_mut(|d| d.insert_temp(last_id, options.corner_radius));
                }
                if toggle.clicked() {
                    options.corner_radius = if on {
                        0
                    } else {
                        ui.data(|d| d.get_temp(last_id))
                            .unwrap_or(DEFAULT_CORNER_RADIUS)
                    };
                }
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
        radius.tip("0 = square corners; large values make a pill or a circle");
    }
    if tool.is_selection() {
        groups.next(ui);
        match look {
            Some(look) => selection_mode_buttons(ui, look, &mut options.selection_mode),
            None => {
                ui.label("Mode");
                for mode in SelectionMode::ALL {
                    crate::widgets::radio_value(
                        ui,
                        &mut options.selection_mode,
                        mode,
                        mode.label(),
                    );
                }
            }
        }
    }
    match tool {
        ToolKind::Hand => {
            hints.push(
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
                    crate::widgets::checkbox(ui, &mut options.contiguous, "Contiguous");
                    ui.label("Sample:");
                    crate::widgets::radio_value(ui, &mut options.sample_all, false, "Layer");
                    crate::widgets::radio_value(ui, &mut options.sample_all, true, "All layers");
                }
            }
        }
        ToolKind::Lasso => {
            hints.push("Drag around the area; it closes when you release.");
        }
        ToolKind::PolygonLasso => {
            hints.push("Click to add points. Click the first point, double-click or press Enter to close; Esc cancels.",
            );
        }
        ToolKind::Polygon => {
            hints.push("Click to add corners. Click the first one, double-click or press Enter to paint it; Esc cancels.",
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
                    crate::widgets::radio_value(
                        ui,
                        &mut options.gradient_shape,
                        GradientShape::Linear,
                        "Linear",
                    );
                    crate::widgets::radio_value(
                        ui,
                        &mut options.gradient_shape,
                        GradientShape::Radial,
                        "Radial",
                    );
                    egui::ComboBox::from_id_salt("dither")
                        .selected_text(options.dither.label())
                        .show_ui(ui, |ui| {
                            for d in Dither::ALL {
                                ui.selectable_value(&mut options.dither, d, d.label());
                            }
                        });
                }
            }
            hints.push("Drag from the primary to the secondary color. Right-drag swaps them.");
        }
        ToolKind::Jumble => {
            hints.push("Scrambles pixels within the brush radius.");
        }
        ToolKind::Contour => {
            hints.push("Draw the outline: it closes and fills when you release.");
        }
        ToolKind::Blur => {
            hints
                .push("Softens the pixels under the brush. Go over an area again to blur it more.");
        }
        ToolKind::Text => {
            groups.next(ui);
            text_style_options(ui, editor, look);
            hints.push("Click on the canvas to place text.");
        }
        ToolKind::Slice => {
            hints.push("Drag to create a slice (hitboxes, pivots). Drag the selected one to move it or its edges to resize it: changes apply from the current frame on.",
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
                    crate::widgets::radio_value(
                        ui,
                        &mut editor.options.rotation,
                        algorithm,
                        algorithm.label(),
                    );
                }
            }
        }
        if editor.options.rotation != before {
            editor.refresh_floating(false);
        }
        hints.push("Drag the handles to scale, or just outside a corner to rotate. Shift keeps the proportions or snaps to 15°.",
        );
    }
    if matches!(
        tool,
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse | ToolKind::Gradient
    ) || matches!(tool, ToolKind::RectSelect | ToolKind::EllipseSelect)
    {
        hints.push("Shift: constrain");
    }
    if tool.paints() {
        hints.push("Alt+click: pick color");
    }
    show_hints(ui, look, &mut groups, &hints);
}

/// Las ayudas de la herramienta, al final de la barra: con Pixel UI, el ícono de info con todas
/// en su tooltip (una por línea); sin él, cada una con `ui.weak`, en su grupo.
fn show_hints(ui: &mut egui::Ui, look: Option<BarLook>, groups: &mut Groups, hints: &[&str]) {
    if hints.is_empty() {
        return;
    }
    match look {
        Some(look) => {
            groups.next(ui);
            bar_fields::info(
                ui,
                look,
                &hints.join(
                    "
",
                ),
            );
        }
        None => {
            for hint in hints {
                groups.next(ui);
                ui.weak(*hint);
            }
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
            crate::widgets::checkbox(ui, checked, label);
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
            .tip(value.label());
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
            response.tip(name)
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
    // Las fuentes pixel de dafont.com: abre su panel del dock.
    #[cfg(not(target_arch = "wasm32"))]
    if crate::pixel_ui::text_button(ui, "Dafont")
        .tip("Pixel fonts from dafont.com")
        .clicked()
    {
        crate::dafont::request_open(ui.ctx());
    }
    #[cfg(not(target_arch = "wasm32"))]
    if look.is_some() {
        gap(ui, CONTROL_GAP);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if crate::pixel_ui::text_button(ui, "Load font").clicked()
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
                crate::widgets::checkbox(ui, &mut style.antialias, "Antialias");
            }
        }
    }
}

/// Ventana flotante para escribir el texto que se está colocando.
fn text_window(ctx: &egui::Context, editor: &mut Editor, skin: Option<&Skin>) {
    if editor.text.is_none() {
        return;
    }
    let mut commit = false;
    let mut cancel = false;
    let (_, closed) = pixel_ui::app_window(
        ctx,
        skin,
        "text_window",
        "Text",
        true,
        (egui::Align2::CENTER_TOP, Vec2::new(0.0, 90.0)),
        |ui| {
            ui.set_width(260.0);
            text_style_options(ui, editor, None);
            let Some(draft) = editor.text.as_mut() else {
                return;
            };
            let edit = egui::TextEdit::multiline(&mut draft.text)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("Type here");
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
            ui.add_space(6.0);
            let clicked = pixel_ui::dialog_buttons(ui, skin, &["OK", "Cancel"]);
            commit = clicked == Some(0) || enter;
            cancel = clicked == Some(1) || esc;
        },
    );
    cancel |= closed;
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
/// pegado al borde): el botón de "Mirror Selection", "Center axes" si los
/// ejes se movieron, los cuatro ejes en orden inverso y el título. En pantalla se leen │ ─ ╲ ╱. Cada botón prende o apaga solo su eje. Con `look`
/// (Pixel UI), los botones son las piezas `sym.slot` con su ícono y el título va en la fila de
/// los textos del diseño.
fn symmetry_buttons(ui: &mut egui::Ui, editor: &mut Editor, look: Option<BarLook>) {
    let skin = look.map(|l| l.skin);
    let current = editor.symmetry();
    // La selección espejo (refleja lo seleccionado en los ejes y al moverlo cada lado va como
    // en un espejo), siempre a la vista. Es lo de más a la derecha.
    {
        let mirror = &mut editor.options.mirror_selection;
        let response = match look {
            // El botón del espejo del diseño, prendido o apagado.
            // El botón del espejo, como en el diseño: 25×16 con su ícono (prendido, hundido).
            Some(look) => crate::pixel_ui::icon_button(
                ui,
                Some(look.skin),
                "icon.mirror",
                "Mirror Selection",
                Some(*mirror),
            ),
            None => crate::widgets::checkbox(ui, mirror, "Mirror selection"),
        };
        if look.is_some() && response.clicked() {
            *mirror = !*mirror;
        }
        response.tip("Mirror Selection: select and move mirrored across the symmetry axes");
        if look.is_some() {
            gap(ui, CONTROL_GAP);
        }
    }
    if current.axes.any()
        && current.center2.is_some()
        && crate::pixel_ui::text_button(ui, "Center axes").clicked()
    {
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
                response.tip(format!("{tip}\n{AXES_HINT}"))
            } else {
                response.tip(tip)
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
    let on_instance = editor
        .sprite()
        .layer(editor.layer)
        .is_some_and(|l| l.kind == cs_core::LayerKind::Instance);
    if on_instance && editor.active_tool().paints() {
        out.push(("Instance layer: edit the source artboard".to_owned(), false));
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
                .tip("Delete artboard")
                .clicked()
            {
                editor.remove_active_artboard();
            }
            if ui.small_button("➕").tip("New 64×64 artboard").clicked() {
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
    if ui.button("Canvas Size").clicked() {
        resize = true;
    }
    resize
}

/// Con Pixel UI, la sección Artboards del diseño: el título con + y tacho, los artboards como
/// pestañas (sin marca) y, debajo, el nombre del activo y el botón de Canvas Size. Devuelve
/// `true` si se pidió abrir el diálogo de tamaño.
pub fn pixel_artboards(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    name_buffer: &mut (ArtboardId, String),
    skin: &Skin,
) -> bool {
    let can_remove = editor.document.artboards.len() > 1;
    match pixel_sections::header(
        ui,
        skin,
        "Artboards",
        "New 64×64 artboard",
        "Delete artboard",
        can_remove,
    ) {
        Some(HeaderAction::Add) => editor.add_artboard(64, 64),
        Some(HeaderAction::Delete) => editor.remove_active_artboard(),
        None => {}
    }
    let ids: Vec<ArtboardId> = editor.document.artboards.iter().map(|a| a.id).collect();
    let tabs: Vec<pixel_sections::Tab> = editor
        .document
        .artboards
        .iter()
        .map(|a| pixel_sections::Tab {
            name: a.name.clone(),
            color: None,
        })
        .collect();
    let active = ids.iter().position(|id| *id == editor.active);
    if let Some(i) = pixel_sections::tabs(ui, skin, &tabs, active) {
        editor.activate(ids[i]);
    }
    if name_buffer.0 != editor.active {
        *name_buffer = (editor.active, editor.artboard().name.clone());
    }
    let (response, resize) = pixel_sections::field_row(
        ui,
        |ui, width| {
            pixel_sections::text_field(ui, skin, &mut name_buffer.1, width, "Artboard name", "Name")
        },
        |ui| {
            pixel_sections::icon_button(
                ui,
                skin,
                pixel_sections::ICON_CANVAS_SIZE,
                "Canvas Size",
                true,
            )
            .clicked()
        },
    );
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
    resize
}

/// Un campo del diseño que edita `current`: mientras tiene el foco guarda lo escrito (en la
/// memoria de egui, con `id`); al perderlo devuelve el texto si cambió (Esc lo descarta).
/// Un campo de una línea (`widgets::text_line`) que edita una copia de `current`: lo escrito se
/// guarda mientras tiene el foco y se devuelve al terminar (Enter o al salir) si cambió; Escape
/// lo descarta.
fn buffered_line(ui: &mut egui::Ui, id: egui::Id, current: &str, label: &str) -> Option<String> {
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| current.to_owned());
    let response = crate::widgets::text_line_named(ui, &mut text, label);
    if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|d| d.remove::<String>(id));
    let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
    (response.lost_focus() && !cancelled && text != current).then_some(text)
}

fn pixel_buffered_field(
    ui: &mut egui::Ui,
    skin: &Skin,
    id: egui::Id,
    current: &str,
    width: f32,
    label: &str,
    hint: &str,
) -> Option<String> {
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| current.to_owned());
    let response = pixel_sections::text_field(ui, skin, &mut text, width, label, hint);
    if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|d| d.remove::<String>(id));
    let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
    (response.lost_focus() && !cancelled && text != current).then_some(text)
}

/// Con Pixel UI, la sección Slices como la de Artboards: el título con + (un slice del tamaño de
/// la selección o del lienzo) y tacho, los slices como pestañas con su color y, del elegido, el
/// nombre con el botón de color, los datos y la clave del frame actual.
pub fn pixel_slices(ui: &mut egui::Ui, editor: &mut Editor, skin: &Skin) {
    let selected = editor.slice;
    match pixel_sections::header(
        ui,
        skin,
        "Slices",
        "New slice",
        "Delete slice",
        selected.is_some(),
    ) {
        Some(HeaderAction::Add) => editor.add_slice(),
        Some(HeaderAction::Delete) => editor.delete_slice(),
        None => {}
    }
    let tabs: Vec<pixel_sections::Tab> = editor
        .sprite()
        .slices
        .iter()
        .map(|s| pixel_sections::Tab {
            name: s.name.clone(),
            color: Some(Color32::from_rgb(s.color.r, s.color.g, s.color.b)),
        })
        .collect();
    if tabs.is_empty() {
        pixel_sections::note(
            ui,
            skin,
            "No slices. Use the Slice tool (Shift+C) to mark hitboxes and pivots.",
        );
        return;
    }
    if let Some(i) = pixel_sections::tabs(ui, skin, &tabs, editor.slice) {
        editor.end_group();
        editor.slice = Some(i);
        editor.set_tool(ToolKind::Slice);
    }
    let Some(index) = editor.slice else {
        return;
    };
    let Some(slice) = editor.selected_slice().cloned() else {
        return;
    };
    let base = ui.id().with(("pixel_slice", index));
    let (name, color_button) = pixel_sections::field_row(
        ui,
        |ui, width| {
            pixel_buffered_field(
                ui,
                skin,
                base.with("name"),
                &slice.name,
                width,
                "Slice name",
                "Name",
            )
        },
        |ui| {
            let c = slice.color;
            pixel_sections::color_button(ui, skin, Color32::from_rgb(c.r, c.g, c.b), "Slice color")
        },
    );
    if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
        editor.update_slice(|s| s.name = name.trim().to_owned());
    }
    // El color: el selector de la columna derecha, en el panel de los menús, colgado del botón.
    let look = menus::MenuLook::new(skin, color_button.rect.top());
    let current = cs_core::Rgba::rgb(slice.color.r, slice.color.g, slice.color.b);
    let picked = menus::dropdown(&look, &color_button, color_button.rect, |ui| {
        ui.set_width(SLICE_PICKER_W);
        let id = ui.id().with("slice_color");
        crate::color_panel::picker(ui, skin, id, current)
    });
    if let Some(color) = picked.filter(|c| *c != current) {
        let rgb = cs_core::Rgba::rgb(color.r, color.g, color.b);
        editor.update_slice(|s| s.color = rgb);
    }
    let data = pixel_sections::field_alone(ui, |ui, width| {
        pixel_buffered_field(
            ui,
            skin,
            base.with("data"),
            &slice.text,
            width,
            "Slice data",
            "Data (e.g. damage=3)",
        )
    });
    if let Some(text) = data {
        editor.update_slice(|s| s.text = text);
    }
    slice_key_properties(ui, editor);
}

/// El ancho del selector de color de los slices.
const SLICE_PICKER_W: f32 = 180.0;

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
    ui.horizontal(|ui| {
        let mut source = key.source;
        widgets::combo(ui, "instance_source", &current, |ui| {
            for (id, name) in names {
                widgets::choice(ui, &mut source, Some(id), &name);
            }
        });
        ui.label("Source");
        if source != key.source {
            editor.update_instance_key(|k| k.source = source);
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
        if crate::widgets::checkbox(ui, &mut fh, "Flip H").changed() {
            editor.update_instance_key(|k| k.flip_h = fh);
        }
        if crate::widgets::checkbox(ui, &mut fv, "Flip V").changed() {
            editor.update_instance_key(|k| k.flip_v = fv);
        }
    });
    let mut opacity = key.opacity;
    let r = ui
        .horizontal(|ui| {
            ui.label("Opacity");
            widgets::number(ui, &mut opacity, 0..=255, "")
        })
        .inner;
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
    // El combo del diseño con los cuatro modos.
    let modes = [
        (Some(GraphicPlay::Loop), "Loop"),
        (Some(GraphicPlay::Once), "Once"),
        (Some(GraphicPlay::Single), "Single"),
        (None, "Movie clip"),
    ];
    let shown = modes
        .iter()
        .find(|(m, _)| *m == mode)
        .map_or("Loop", |(_, name)| *name);
    ui.horizontal(|ui| {
        ui.label("Play");
        widgets::combo(ui, "instance_play", shown, |ui| {
            for (value, name) in modes {
                widgets::choice(ui, &mut mode, value, name);
            }
        })
        .tip(
            "Loop, Once and Single follow this artboard's frames; Movie clip plays on its own              clock with its own frame durations",
        );
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
    let entered = crate::pixel_ui::text_button(ui, "Edit Symbol")
        .tip("Go to the source artboard (double-click with Move)")
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
                .tip("Delete slice")
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
    // El nombre y los datos se guardan mientras se escriben (si no, cada frame volverían a ser
    // los del slice) y se aplican al terminar.
    let base = ui.id().with(("slice_props", editor.slice));
    ui.horizontal(|ui| {
        ui.label("Name");
        if let Some(name) = buffered_line(ui, base.with("name"), &slice.name, "Slice name")
            .filter(|n| !n.trim().is_empty())
        {
            editor.update_slice(|s| s.name = name.trim().to_owned());
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
    ui.horizontal(|ui| {
        ui.label("Data");
        if let Some(text) = buffered_line(ui, base.with("data"), &slice.text, "Slice data") {
            editor.update_slice(|s| s.text = text);
        }
    });
    slice_key_properties(ui, editor);
}

/// La clave del frame actual del slice seleccionado: rect, pivote, 9-slice y ocultar.
fn slice_key_properties(ui: &mut egui::Ui, editor: &mut Editor) {
    let Some(slice) = editor.selected_slice().cloned() else {
        return;
    };
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
    if crate::widgets::checkbox(ui, &mut has_pivot, "Pivot").changed() {
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
    if crate::widgets::checkbox(ui, &mut nine, "9-Slice")
        .tip("Center area that stretches; corners keep their size")
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
        && crate::pixel_ui::text_button(ui, "Remove key on this frame").clicked()
    {
        editor.remove_slice_key();
    }
    let mut hide = false;
    if crate::pixel_ui::text_button(ui, "Hide from this frame")
        .tip("Adds an empty key: the slice disappears from here on")
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
mod tests;
