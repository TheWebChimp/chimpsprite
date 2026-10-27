//! La barra de opciones de la herramienta (arriba del lienzo), con la de simetría a la derecha
//! y la ventana de texto.

use super::*;

/// El radio que vuelve al prender el redondeo si no se había usado otro.
pub(super) const DEFAULT_CORNER_RADIUS: u32 = 4;

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
pub(super) fn gap(ui: &mut egui::Ui, px: f32) {
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
    // El ink, como en Aseprite: cómo pinta encima de lo que hay.
    if tool.uses_ink() {
        groups.next(ui);
        let ink = &mut options.ink;
        match look {
            Some(look) => {
                bar_fields::label(ui, look, "Ink");
                gap(ui, LABEL_FIELD_GAP);
                bar_fields::combo(ui, look, "ink", ink.label(), |ui| {
                    bar_fields::choices(ui, ink, Ink::ALL, Ink::label)
                });
            }
            None => {
                egui::ComboBox::from_label("Ink")
                    .selected_text(ink.label())
                    .show_ui(ui, |ui| {
                        for i in Ink::ALL {
                            ui.selectable_value(ink, i, i.label());
                        }
                    });
            }
        }
        if *ink == Ink::Shading {
            hints.push("Shading: each pixel moves along the selected palette range. Right-click goes back.");
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
pub(super) fn brush_shape_buttons(
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
pub(super) enum SymAxis {
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
pub(super) const SYMMETRY_AXES: [(SymAxis, &str); 4] = [
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
