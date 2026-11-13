//! Formas responsivas: los paneles se arman con filas, columnas y grillas (el flexbox / CSS grid de
//! taffy, vía egui_taffy) en vez de coordenadas absolutas, así se acomodan al ancho que les toque.
//! Los tamaños van en puntos enteros: con la escala de la UI cada punto es un pixel del skin.
//! Solo este módulo conoce egui_taffy; los paneles usan lo que reexporta.

use eframe::egui::{self, Id, Ui, vec2};
use egui_taffy::taffy::prelude::*;

pub(crate) use egui_taffy::taffy::Style;
pub(crate) use egui_taffy::{Tui, TuiBuilderLogic};

/// Una forma del ancho disponible de `ui`: una columna con `gap` entre sus hijos.
///
/// `id` es global: no se combina con `ui.id()`. Dos formas con el mismo `id` en un frame comparten
/// el estado de taffy (una mide con lo de la otra) y, anidadas, truenan; las que se repiten (una
/// por fila de una lista) llevan en su `id` algo que no se repita, como su índice.
pub(crate) fn form(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    gap: f32,
    add: impl FnOnce(&mut Tui),
) {
    // En puntos enteros: con medio punto disponible (mientras entra la barra del scroll, por
    // ejemplo), taffy redondearía hacia arriba y la forma se saldría de `ui`.
    let width = ui.available_width().floor();
    egui_taffy::tui(ui, Id::new(id))
        .reserve_width(width)
        .style(Style {
            size: Size {
                width: percent(1.0_f32),
                height: auto(),
            },
            ..column(gap)
        })
        .show(add);
}

/// Una [`form`] en el flujo de `ui` (una fila entre títulos de egui), en puntos enteros aunque
/// el cursor no lo esté: desde el cursor redondeado, de los puntos enteros que quedan a lo ancho,
/// reserva en `ui` lo que ocupa la forma, como cualquier widget (lo que sigue va debajo).
/// Devuelve lo que reservó.
pub(crate) fn form_in_flow(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    gap: f32,
    add: impl FnOnce(&mut Tui),
) -> egui::Rect {
    let available = ui.available_rect_before_wrap();
    let min = available.min.round();
    let width = (available.max.x - min.x).floor();
    let rect = egui::Rect::from_min_size(min, vec2(width, available.max.y - min.y));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    form(&mut child, id, gap, add);
    let reserved = egui::Rect::from_min_size(min, vec2(width, child.min_rect().height()));
    ui.allocate_rect(reserved, egui::Sense::hover()).rect
}

/// El mismo `gap` entre filas y entre columnas.
fn gap_of(gap: f32) -> Size<LengthPercentage> {
    Size {
        width: length(gap),
        height: length(gap),
    }
}

/// Hijos uno debajo de otro, con `gap` entre ellos.
pub(crate) fn column(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Column,
        gap: gap_of(gap),
        ..Default::default()
    }
}

/// Hijos uno al lado del otro; si no caben, siguen en la fila de abajo.
pub(crate) fn row(gap: f32) -> Style {
    Style {
        flex_wrap: FlexWrap::Wrap,
        ..row_nowrap(gap)
    }
}

/// Como [`row`], con los hijos al final (a la derecha) de su fila: una fila de botones que
/// termina con el panel; si no caben, los que sobran siguen en la de abajo, también a la derecha.
pub(crate) fn row_end(gap: f32) -> Style {
    Style {
        justify_content: Some(JustifyContent::FlexEnd),
        ..row(gap)
    }
}

/// Como [`row`], pero sin espacio entre los hijos de una fila (cada uno pone el suyo con
/// [`air_right`]) y con `gap` entre las filas: para las filas con un hijo [`push_right`].
pub(crate) fn row_lines(gap: f32) -> Style {
    Style {
        gap: Size {
            width: length(0.0_f32),
            height: length(gap),
        },
        ..row(0.0)
    }
}

/// Hijos uno al lado del otro, sin partir la fila.
pub(crate) fn row_nowrap(gap: f32) -> Style {
    Style {
        flex_direction: FlexDirection::Row,
        gap: gap_of(gap),
        align_items: Some(AlignItems::Start),
        size: Size {
            width: percent(1.0_f32),
            height: auto(),
        },
        ..Default::default()
    }
}

/// Una hoja de `w` × `h` que no se encoge.
pub(crate) fn fixed(w: f32, h: f32) -> Style {
    Style {
        size: Size {
            width: length(w),
            height: length(h),
        },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

/// Una hoja de alto `h` que mide `basis` de ancho y crece (hasta `max`, si hay) o se achica
/// (hasta `min`) con lo que sobre o falte en su fila.
pub(crate) fn grow(min: f32, basis: f32, max: Option<f32>, h: f32) -> Style {
    Style {
        flex_grow: 1.0,
        flex_shrink: 1.0,
        flex_basis: length(basis),
        min_size: Size {
            width: length(min),
            height: length(h),
        },
        max_size: Size {
            width: max.map_or(auto(), length),
            height: length(h),
        },
        size: Size {
            width: auto(),
            height: length(h),
        },
        ..Default::default()
    }
}

/// Una hoja como [`grow`] (`basis` de ancho, entre `min` y lo que sobre en su fila) pero del alto
/// de lo que tiene adentro: para una columna de un panel, armada con el layout de egui.
pub(crate) fn grow_auto(min: f32, basis: f32) -> Style {
    Style {
        flex_grow: 1.0,
        flex_shrink: 1.0,
        flex_basis: length(basis),
        min_size: Size {
            width: length(min),
            height: auto(),
        },
        ..Default::default()
    }
}

/// `style` estirado al alto de su fila (aunque la fila ponga a sus hijos arriba).
pub(crate) fn full_height(style: Style) -> Style {
    Style {
        align_self: Some(AlignSelf::Stretch),
        ..style
    }
}

/// `style` que crece a lo alto con lo que sobre en su columna (desde 0), sin bajar de `min`: lo
/// que llena un panel hasta su pie.
pub(crate) fn fill_height(min: f32, style: Style) -> Style {
    Style {
        flex_grow: 1.0,
        flex_basis: length(0.0_f32),
        min_size: Size {
            height: length(min),
            ..style.min_size
        },
        ..style
    }
}

/// `style` que crece a lo alto con lo que sobre en su columna, sin bajar del alto de lo que tiene
/// adentro (una fila con una columna de controles y un lienzo que llega hasta el pie).
pub(crate) fn fill_height_auto(style: Style) -> Style {
    Style {
        flex_grow: 1.0,
        ..style
    }
}

/// `style` que crece `weight` veces lo que crecen sus hermanas de peso 1 (una columna que vale
/// por dos sigue del ancho de dos columnas de las de arriba).
pub(crate) fn weight(weight: f32, style: Style) -> Style {
    Style {
        flex_grow: weight,
        ..style
    }
}

/// `style` de por lo menos `h` de alto (un panel del alto de su columna, aunque tenga poco).
pub(crate) fn min_height(h: f32, style: Style) -> Style {
    Style {
        min_size: Size {
            height: length(h),
            ..style.min_size
        },
        ..style
    }
}

/// `style` con `right` de aire a la derecha (lo que sobra en la fila no lo usa).
pub(crate) fn air_right(right: f32, style: Style) -> Style {
    Style {
        margin: Rect {
            right: length(right),
            ..style.margin
        },
        ..style
    }
}

/// `style` con `bottom` de aire debajo (lo de abajo va más abajo; estirado, llega hasta ahí).
pub(crate) fn air_below(bottom: f32, style: Style) -> Style {
    Style {
        margin: Rect {
            bottom: length(bottom),
            ..style.margin
        },
        ..style
    }
}

/// `style` empujado a la derecha de su fila (lo de antes queda a la izquierda; si la fila se
/// parte, a la derecha de la suya). En una fila con espacio entre los hijos, taffy 0.9 no pone ese
/// espacio en la línea de este hijo y lo deja a ese espacio del borde: va en filas de
/// [`row_lines`] (o de espacio 0).
pub(crate) fn push_right(style: Style) -> Style {
    Style {
        margin: Rect {
            left: LengthPercentageAuto::auto(),
            ..style.margin
        },
        ..style
    }
}

/// Una hoja que reparte el ancho de su fila en partes iguales con sus hermanas `share` (desde
/// base 0), sin bajar de `min`.
pub(crate) fn share(min: f32, h: f32) -> Style {
    Style {
        flex_basis: length(0.0_f32),
        ..grow(min, 0.0, None, h)
    }
}

/// Hijos uno al lado del otro que van juntos dentro de una fila (una etiqueta y su campo): del
/// ancho de lo que tienen, sin partirse ni encogerse; si no caben, bajan juntos.
pub(crate) fn group(gap: f32) -> Style {
    Style {
        size: Size {
            width: auto(),
            height: auto(),
        },
        flex_shrink: 0.0,
        ..row_nowrap(gap)
    }
}

/// Una hoja del tamaño de lo que tiene adentro (lo que ocupó en el frame anterior), que no se
/// encoge: para los controles que se miden solos (casillas y botones con su texto).
pub(crate) fn natural() -> Style {
    Style {
        flex_shrink: 0.0,
        ..Default::default()
    }
}

/// `style` con `top` de aire de más arriba (negativo, sube): para las filas del mockup que no van
/// al paso de las demás.
pub(crate) fn air_above(top: f32, style: Style) -> Style {
    Style {
        margin: Rect {
            top: length(top),
            ..style.margin
        },
        ..style
    }
}

/// Una hoja de `style` en `tui` que no se mide: lo que `add` dibuja en ella (del tamaño que le dé
/// su estilo, `ui.max_rect()`) no la agranda. Para los lienzos libres que llenan lo que les toca
/// (una caja que llega hasta el pie): medidos, crecerían con lo que dibujaron la vez pasada.
pub(crate) fn canvas<R>(tui: &mut Tui, style: Style, add: impl FnOnce(&mut Ui) -> R) -> R {
    tui.style(style)
        .ui_manual(|ui, _| egui_taffy::TuiContainerResponse {
            inner: add(ui),
            min_size: egui::Vec2::ZERO,
            intrinsic_size: None,
            max_size: egui::Vec2::ZERO,
            infinite: egui::Vec2b::FALSE,
        })
}

/// El ancho que necesita `text` en la fuente de la UI (tinta + 1), en puntos enteros.
pub(crate) fn label_width(ui: &Ui, text: &str) -> f32 {
    crate::kit::button::ink_width(ui, text).ceil() + 1.0
}

/// Una etiqueta de `width` en la fila de 17 de los controles del kit (las mayúsculas a la altura
/// de las de los campos), con nombre para los lectores de pantalla.
pub(crate) fn label(ui: &mut Ui, text: &str, width: f32) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, crate::kit::input::HEIGHT), egui::Sense::hover());
    let info = crate::panel_kit::label_info(ui, text);
    response.widget_info(|| info.clone());
    crate::pixel_ui::small_text_at(ui, rect, text);
}
/// Para los tests de los paneles: dibuja en un `Ui` de `width` (con el skin y las fuentes de la
/// app, escala 1) y junta los widgets con nombre que quedaron.
#[cfg(test)]
pub(crate) mod testing {
    use eframe::egui::{self, Pos2, RawInput, Rect, Ui, Vec2, accesskit::Role};

    pub(crate) struct Probe {
        /// Los controles con nombre (botones, casillas, campos, selects, etiquetas), su rect y qué
        /// son (en la misma entrada: quien filtre la lista no los desalinea).
        pub widgets: Vec<(String, Rect, Role)>,
        /// Los textos pintados: lo que se ve (con su "…" si no cupo) y el rect de su galley.
        pub texts: Vec<(String, Rect)>,
        /// Las imágenes pintadas (las piezas del skin también): su textura y lo que se ve de
        /// ellas (con su recorte).
        pub images: Vec<(egui::TextureId, Rect)>,
        /// Los rects rellenos pintados (los cuadros de una tira de colores, por ejemplo): su color
        /// y su rect.
        pub fills: Vec<(egui::Color32, Rect)>,
        /// Los nombres de los controles de `widgets` que los lectores de pantalla ven
        /// deshabilitados.
        pub disabled: Vec<String>,
    }

    /// Lo que se mide: hojas que el usuario ve (no los grupos que las contienen).
    const LEAVES: &[Role] = &[
        Role::Button,
        Role::CheckBox,
        Role::TextInput,
        Role::ComboBox,
        Role::SpinButton,
        Role::Slider,
        Role::Label,
        Role::RadioButton,
    ];

    pub(crate) fn run_at(width: f32, mut draw: impl FnMut(&mut Ui)) -> Probe {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = crate::skin::Skin::embedded(&ctx);
        ctx.set_fonts(crate::appearance::font_definitions(true));
        crate::appearance::apply_style(&ctx, Some(skin.colors()));
        crate::pixel_ui::set_bubble(&ctx, Some(std::sync::Arc::new(skin)));
        // Tres frames: las fuentes llegan en el primero y taffy mide en el segundo.
        let mut probe = frame_at(&ctx, width, &mut draw);
        for _ in 0..2 {
            probe = frame_at(&ctx, width, &mut draw);
        }
        probe
    }

    /// Un frame de `ctx` (que ya tiene sus fuentes y su estilo: los de `run_at` o los de la app)
    /// con `draw` en un `Ui` de `width`, y lo que quedó. Así un barrido de anchos reusa el mismo
    /// contexto de un ancho al siguiente (como al arrastrar un divisor; ver `width_sweep_tests`).
    pub(crate) fn frame_at(
        ctx: &egui::Context,
        width: f32,
        mut draw: impl FnMut(&mut Ui),
    ) -> Probe {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(2000.0, 2000.0))),
            // Las texturas grandes que acepta la app (las muestras de una fuente, por ejemplo).
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(width, 1900.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.set_width(width);
                draw(ui);
            });
        });
        // Nadie sube texturas aquí: egui exige descartar lo pendiente.
        output.textures_delta.clear();
        let mut texts = Vec::new();
        let mut images = Vec::new();
        let mut fills = Vec::new();
        for clipped in &output.shapes {
            collect_texts(&clipped.shape, &mut texts);
            collect_images(&clipped.shape, clipped.clip_rect, &mut images);
            collect_fills(&clipped.shape, &mut fills);
        }
        let nodes = output
            .platform_output
            .accesskit_update
            .map(|u| u.nodes)
            .unwrap_or_default();
        let mut disabled = Vec::new();
        let widgets: Vec<(String, Rect, Role)> = nodes
            .iter()
            .filter(|(_, n)| LEAVES.contains(&n.role()))
            .filter_map(|(_, n)| {
                let b = n.bounds()?;
                let rect = Rect::from_min_max(
                    Pos2::new(b.x0 as f32 - 10.0, b.y0 as f32 - 10.0),
                    Pos2::new(b.x1 as f32 - 10.0, b.y1 as f32 - 10.0),
                );
                // Las etiquetas llevan su texto en el valor (así las nombra egui).
                let name = match n.label() {
                    Some(label) => label.to_string(),
                    None => n.value()?.to_string(),
                };
                if n.is_disabled() {
                    disabled.push(name.clone());
                }
                Some((name, rect, n.role()))
            })
            .collect();
        Probe {
            widgets,
            texts,
            images,
            fills,
            disabled,
        }
    }

    /// Los rects rellenos de `shape` (sin textura): su color y su rect, desde la esquina del
    /// panel.
    fn collect_fills(shape: &egui::Shape, out: &mut Vec<(egui::Color32, Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_fills(s, out)),
            egui::Shape::Rect(r) if r.fill.a() > 0 => {
                out.push((r.fill, r.rect.translate(Vec2::splat(-10.0))));
            }
            _ => {}
        }
    }

    /// Las mallas con textura de `shape` y lo que se ve de cada una con el recorte `clip`, desde
    /// la esquina del panel.
    fn collect_images(shape: &egui::Shape, clip: Rect, out: &mut Vec<(egui::TextureId, Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_images(s, clip, out)),
            egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                let shown = mesh.calc_bounds().intersect(clip);
                out.push((mesh.texture_id, shown.translate(Vec2::splat(-10.0))));
            }
            _ => {}
        }
    }

    /// Los textos de `shape` como se ven (las letras de sus filas) y su rect, desde la esquina del
    /// panel.
    fn collect_texts(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_texts(s, out)),
            egui::Shape::Text(text) => {
                let rows = text.galley.rows.iter();
                let shown = rows.flat_map(|r| r.glyphs.iter().map(|g| g.chr)).collect();
                let rect = Rect::from_min_size(text.pos, text.galley.size());
                out.push((shown, rect.translate(Vec2::splat(-10.0))));
            }
            _ => {}
        }
    }

    impl Probe {
        pub(crate) fn rect(&self, name: &str) -> Rect {
            self.widgets
                .iter()
                .find(|(n, _, _)| n == name)
                .map(|(_, r, _)| *r)
                .unwrap_or_else(|| panic!("no {name:?} in {:?}", self.widgets))
        }

        /// El primer texto pintado que empieza con `start`: lo que se ve y su rect.
        pub(crate) fn text(&self, start: &str) -> (&str, Rect) {
            self.texts
                .iter()
                .find(|(t, _)| t.starts_with(start))
                .map(|(t, r)| (t.as_str(), *r))
                .unwrap_or_else(|| panic!("no {start:?} in {:?}", self.texts))
        }

        /// Cada control empieza y termina en puntos enteros (nada de medio pixel del skin).
        pub(crate) fn assert_whole_px(&self) {
            for (name, r, _) in &self.widgets {
                for v in [r.min.x, r.min.y, r.max.x, r.max.y] {
                    assert_eq!(v, v.round(), "{name:?} at {r:?}");
                }
            }
        }

        /// Nada se sale a la derecha del ancho del panel.
        pub(crate) fn assert_within(&self, width: f32) {
            for (name, r, _) in &self.widgets {
                assert!(
                    r.min.x >= 0.0 && r.max.x <= width,
                    "{name:?} at {r:?} past {width}"
                );
            }
        }

        /// Ningún control tapa a otro.
        pub(crate) fn assert_no_overlap(&self) {
            for (i, (a, ra, _)) in self.widgets.iter().enumerate() {
                for (b, rb, _) in &self.widgets[i + 1..] {
                    let inter = ra.intersect(*rb);
                    assert!(
                        inter.width() <= 0.0 || inter.height() <= 0.0,
                        "{a:?} {ra:?} overlaps {b:?} {rb:?}"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::run_at;
    use super::*;

    /// Una forma chica como las de los paneles: etiqueta + campo que crece y tres botones parejos.
    fn sample(ui: &mut Ui) {
        let skin = crate::pixel_ui::frame_skin(ui.ctx()).expect("skin");
        form(ui, "sample", 4.0, |tui| {
            tui.style(row(4.0)).add(|tui| {
                let lw = 37.0;
                tui.style(fixed(lw, 17.0)).ui(|ui| label(ui, "Family", lw));
                tui.style(grow(60.0, 160.0, Some(160.0), 17.0)).ui(|ui| {
                    let w = ui.max_rect().width();
                    let mut text = String::from("Mine");
                    crate::kit::input::text_input(ui, &skin, &mut text, w, "Family name", "Font");
                });
            });
            tui.style(row(4.0)).add(|tui| {
                for name in ["Generate", "Build", "Export"] {
                    tui.style(share(50.0, 17.0)).ui(|ui| {
                        let w = ui.max_rect().width();
                        crate::kit::button::button_sized(ui, &skin, name, w);
                    });
                }
            });
        });
    }

    #[test]
    fn a_form_lands_on_whole_points_inside_its_width_at_any_width() {
        for width in [120.0, 220.0, 400.0, 600.0] {
            let probe = run_at(width, sample);
            probe.assert_whole_px();
            probe.assert_within(width);
            probe.assert_no_overlap();
        }
    }

    /// Con medio punto disponible (mientras entra la barra de un scroll, por ejemplo), la forma
    /// usa solo los puntos enteros: no se sale de su `Ui` (si se saliera, el panel crecería).
    #[test]
    fn a_form_keeps_to_whole_points_of_a_fractional_width() {
        let probe = run_at(311.5, sample);
        probe.assert_whole_px();
        probe.assert_within(311.5);
    }

    #[test]
    fn narrow_rows_wrap_and_wide_ones_grow() {
        let wide = run_at(600.0, sample);
        let (g, b) = (wide.rect("Generate"), wide.rect("Build"));
        assert_eq!(g.top(), b.top(), "one row at 600");
        assert_eq!(
            wide.rect("Family name").width(),
            160.0,
            "grows up to its max"
        );
        let narrow = run_at(120.0, sample);
        let (g, x) = (narrow.rect("Generate"), narrow.rect("Export"));
        assert!(x.top() > g.top(), "the buttons wrap at 120: {g:?} {x:?}");
    }

    /// Tres botones de `w` en una fila de `style`.
    fn three_buttons(tui: &mut Tui, style: Style, w: f32) {
        let skin = crate::pixel_ui::frame_skin(&tui.egui_ui().ctx().clone()).expect("skin");
        tui.style(style).add(|tui| {
            for name in ["One", "Two", "Six"] {
                tui.style(fixed(w, 17.0)).ui(|ui| {
                    crate::kit::button::button_sized(ui, &skin, name, w);
                });
            }
        });
    }

    /// Con el cursor en medio punto, `form_in_flow` reserva un rect en puntos enteros, de lo que
    /// ocupa la forma (nada de ella se sale), y lo que sigue va debajo de él.
    #[test]
    fn a_form_in_flow_reserves_whole_points_from_a_half_point_cursor() {
        let mut reserved = egui::Rect::NOTHING;
        let probe = run_at(300.0, |ui| {
            let at = ui.cursor().min + vec2(0.5, 0.5);
            let rect = egui::Rect::from_min_size(at, vec2(200.5, 300.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                reserved = form_in_flow(ui, "flow", 0.0, |tui| {
                    three_buttons(tui, row(4.0), 50.0);
                });
                label(ui, "After", 40.0);
            });
        });
        let reserved = reserved.translate(vec2(-10.0, -10.0));
        for v in [
            reserved.min.x,
            reserved.min.y,
            reserved.max.x,
            reserved.max.y,
        ] {
            assert_eq!(v, v.round(), "{reserved:?}");
        }
        assert_eq!(reserved.width(), 200.0, "{reserved:?}");
        // Lo de la forma (After va donde lo pone egui: a medio punto, como su `Ui`).
        for name in ["One", "Two", "Six"] {
            let r = probe.rect(name);
            assert_eq!((r.min, r.max), (r.min.round(), r.max.round()), "{name}");
            assert!(
                reserved.contains_rect(r),
                "{name} {r:?} out of {reserved:?}"
            );
        }
        assert!(
            probe.rect("After").top() >= reserved.bottom(),
            "{reserved:?}"
        );
    }

    /// `row_end` pone a sus hijos contra el borde derecho; los que no caben bajan, también a la
    /// derecha.
    #[test]
    fn row_end_keeps_its_children_at_the_right_edge() {
        let wide = run_at(300.0, |ui| {
            form(ui, "end", 0.0, |tui| three_buttons(tui, row_end(4.0), 50.0))
        });
        assert_eq!(wide.rect("Six").right(), 300.0);
        assert_eq!(wide.rect("Two").right(), 300.0 - 54.0);
        assert_eq!(wide.rect("One").right(), 300.0 - 108.0);
        assert_eq!(wide.rect("One").top(), wide.rect("Six").top());
        // A 120 caben dos: el tercero baja, a la derecha.
        let narrow = run_at(120.0, |ui| {
            form(ui, "end", 0.0, |tui| three_buttons(tui, row_end(4.0), 50.0))
        });
        let (one, two, six) = (narrow.rect("One"), narrow.rect("Two"), narrow.rect("Six"));
        assert_eq!(two.right(), 120.0);
        assert_eq!(one.right(), 120.0 - 54.0);
        assert_eq!(one.top(), two.top());
        assert!(six.top() > two.bottom(), "{six:?}");
        assert_eq!(six.right(), 120.0);
        narrow.assert_whole_px();
    }

    /// Las etiquetas (las de `label` y los textos de `text_wrapped`) salen deshabilitadas para los
    /// lectores de pantalla donde lo de alrededor lo está.
    #[test]
    fn labels_follow_the_enabled_state() {
        let probe = run_at(300.0, |ui| {
            for enabled in [true, false] {
                ui.add_enabled_ui(enabled, |ui| {
                    label(ui, &format!("Label {enabled}"), 60.0);
                    crate::panel_kit::text_wrapped(ui, &format!("Help {enabled}"));
                });
            }
        });
        let mut disabled = probe.disabled.clone();
        disabled.sort();
        assert_eq!(disabled, ["Help false", "Label false"]);
        probe.rect("Label true");
        probe.rect("Help true");
        // Lo que reciben (en los eventos y en el nodo), no solo lo que egui agrega por su cuenta.
        run_at(300.0, |ui| {
            assert!(crate::panel_kit::label_info(ui, "On").enabled);
            ui.add_enabled_ui(false, |ui| {
                assert!(!crate::panel_kit::label_info(ui, "Off").enabled);
            });
        });
    }

    #[test]
    fn shared_widths_differ_by_at_most_one_point() {
        let probe = run_at(400.0, sample);
        let ws: Vec<f32> = ["Generate", "Build", "Export"]
            .iter()
            .map(|n| probe.rect(n).width())
            .collect();
        let (min, max) = ws
            .iter()
            .fold((f32::MAX, 0.0f32), |(a, b), &w| (a.min(w), b.max(w)));
        assert!(max - min <= 1.0, "{ws:?}");
    }
}
