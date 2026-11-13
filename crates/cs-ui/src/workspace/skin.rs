//! La piel pixel art de egui_dock (portada del prototipo `proto_workspace`, aprobado por el
//! usuario). egui_dock no deja dibujar sus pestañas ni sus divisores con otra cosa que colores, así
//! que se usan dos trucos:
//!
//! - **Colores marcador.** egui_dock pinta los divisores, la zona de destino y el marco de las
//!   ventanas flotantes con un color único; [`SkinRewriter`] (un plugin de egui) cambia esos rects
//!   por piezas del skin antes de teselar (la línea grabada, la franja de destino en el marco
//!   `menu.side` y el marco `menu.side`).
//! - **Pestañas encima.** Las de egui_dock van 100 % transparentes y la pestaña del kit se pinta
//!   encima, en su misma capa, con el rect de `TabViewer::on_tab_button` (ver [`paint_tabs`]).
//!
//! Además, [`snap`] y [`snap_windows`] dejan los divisores y las ventanas en puntos enteros
//! (egui_dock solo redondea a píxel físico: con la escala 2, medio píxel del skin), y el mismo
//! plugin pinta translúcida la ventana cuya única pestaña se arrastra (`Armed::faded`, con o sin
//! Pixel UI; ver `drop::see_through`).
//!
//! **Depende de detalles internos de egui_dock 0.21.1** (por eso la versión va fija en
//! `Cargo.toml`). Al subir de versión hay que revisarlos:
//!
//! 1. El id del área de cada ventana flotante, `"window {surface:?}"` ([`window_area_id`]). La
//!    ventana atenuada (`Armed::faded`) depende además de que todo lo de una ventana, incluidas
//!    las pestañas del kit, se pinte en la capa `(Order::Middle, window_area_id)`.
//! 2. La respuesta de la pestaña que se arrastra, `id.with("dragged")` ([`is_dragged_tab`]).
//! 3. El id de la franja de agarre de un grupo sin barra, `hidden_tab_bar_drag`
//!    ([`hidden_strip_id`]).
//! 4. Que con el estilo transparente siga pintando rects llenos, sin redondeo, con los colores de
//!    [`dock_style`] (divisores, zona de destino; el contorno de "ventana nueva" como trazo), que
//!    el marco de sus ventanas sea `Frame::window` del `Ui` que recibe el `DockArea` (ver
//!    [`window_visuals`]) y que los atenúe con `linear_multiply` (ver el test de los marcadores).
//!    La zona de destino y el contorno de "ventana nueva" van los dos de
//!    `overlay.selection_color`: transparente, no se ven ([`hide_drop_zone`]).
//! 5. Dónde los pinta: la zona de destino en la capa `Foreground` "overlay" recortada a la
//!    pantalla, los divisores recortados al `DockArea` o a su ventana (de eso depende
//!    `Armed::in_scope`).
//! 6. Que el cuerpo de cada pestaña se arme con `Ui::new` y el estilo del contexto (el `Viewer`
//!    de `workspace` le vuelve a poner el del anfitrión).
//! 7. Que registre los divisores después de los grupos: lo que alcanzan ([`SEPARATOR_EXTRA`]) le
//!    gana a la franja de agarre del grupo de abajo.
//! 8. Que la pestaña que se arrastra vaya en su propia capa `Tooltip` movida con
//!    `transform_layer_shapes` (la × del kit no va ahí; el fantasma se pinta aparte).
//! 9. Lo de sus ventanas y del soltar: `WindowState::set_size` es `Window::fixed_size` (el tamaño
//!    de fuera, con el marco) por un frame, `WindowState::rect()` viene vacío (se mide el área),
//!    una pestaña solo se suelta sobre un grupo (fuera de ellos `workspace` la pone a flotar) y el
//!    grupo por el que pasó el puntero se recuerda `max_preference_time` (0.3 s).
//! 10. El lienzo (fase D): el id de la pestaña de un grupo, `(tab, "tab")` después del de su
//!     nodo ([`canvas_drag_ids`]: con él y el de la franja se le corta el arrastre), y que la zona
//!     de destino "como pestaña" de un grupo sea su rect entero (así se reconoce la del lienzo para
//!     pintarla como la mitad que de verdad queda; ver [`Armed::canvas_drop`]).
//! 11. `separator.extra`: de fábrica, 175; `show_separator` lo impone cada frame como lo mínimo de
//!     cada lado de un split (la toolbar o una columna angosta no cabrían). Va en 0: los mínimos
//!     los pone `fit_splits`.
//! 12. La ventana que se arrastra no es destino (`drop::see_through`, en `show`): cada grupo que
//!     contiene el puntero sobrescribe el destino (`hover_data`, el último gana: la superficie
//!     principal primero, las ventanas después) en `tab_body` de `show/leaf.rs`, justo después
//!     de `TabViewer::ui` de su pestaña al frente, y solo si `TabViewer::allowed_in_windows` (o la
//!     superficie principal) lo deja. El `Viewer` de `show` marca su grupo en `ui` y
//!     `allowed_in_windows` gasta la marca (dice que no). El invariante: nada puede llamar
//!     `Viewer::allowed_in_windows` entre el `ui` de un grupo y esa comprobación, ni usarlo como
//!     predicado puro (fuera de egui_dock: `model::floats`). Por eso el menú de las pestañas de
//!     egui_dock va apagado (`tab_context_menus(false)`: el suyo llama `allowed_in_windows`) y el
//!     "Dock" de las pestañas de lo que flota es de `workspace` (`tabs::dock_menus`, después del
//!     `DockArea`). La llamada de `show_drag_drop_overlay` (al empezar el `DockArea`, antes de los
//!     grupos) no ve la marca. El frame en que egui_dock empieza
//!     el arrastre todavía no hay `dnd` (no pregunta `allowed_in_windows`) y el destino es la
//!     propia ventana: su zona (el grupo entero o su barra, `Armed::faded_leaf`) no se pinta.
//! 13. El grupo recordado (punto 9) se cambia en `State::set_drag_and_drop`, al empezar el
//!     `DockArea`, solo si pasó `overlay.feel.max_preference_time` del estilo de ese frame desde
//!     que se fijó. Con el puntero sobre la ventana que se arrastra va en 0: al empezar a
//!     arrastrar el grupo recordado es el suyo (antes de que haya destino, egui_dock no pregunta
//!     `allowed_in_windows`), y ahí lo de debajo cuenta en cuanto llega.
//! 14. Cuándo empieza egui_dock el arrastre de una pestaña o de la franja de agarre
//!     (`try_initiate_tab_drag` en `show/leaf.rs`): con el puntero a más de 30 pt a lo ancho o de
//!     6 a lo alto de donde estaba el primer frame en que egui lo dio por arrastre
//!     (`drop::DOCK_DRAG_START`). Antes (una pulsación larga, un deslizón corto) no suelta nada, y
//!     `workspace` tampoco (`drop::dragged_tab`). Lo vuelve a mirar cada frame (`drag_data` se
//!     borra al empezar `show_inside`): si el puntero regresa dentro del umbral, tampoco.
//! 15. egui_dock mueve lo soltado al empezar `show_inside` y dibuja ese frame con el split nuevo a
//!     la mitad (0.5), antes de que `workspace` acomode los lados (`pin_sides` y `fit_splits` van
//!     fuera del `DockArea`): partiendo un grupo angosto (la toolbar, 35 pt), un lado queda más
//!     angosto que su panel. Ese frame (el de soltar) el `Viewer` acomoda lo de adentro en su
//!     mínimo, recortado (`Viewer::squeeze`), y se vuelve a dibujar. La segunda pasada de egui
//!     no trae el soltar (su entrada va vacía).

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, Id, LayerId, Margin, Order, Rect, Shape, Stroke, vec2,
};
use egui_dock::{DockState, Node, NodePath, OverlayType, SurfaceIndex};

use super::fit::whole;
use crate::kit;
use crate::kit::tabs::{Strip, TabItem, TabLook};
use crate::panel_registry::PanelId;
use crate::pixel_ui;
use crate::skin::{Skin, SkinColors, WidgetState};

// Colores marcador: únicos, los cambia `SkinRewriter`. Opacos (la atenuación de egui_dock los
// multiplica por 1.0 y la vuelta por el espacio lineal no los cambia; ver el test).
const M_SEP_IDLE: Color32 = Color32::from_rgb(1, 0, 2);
const M_SEP_HOVER: Color32 = Color32::from_rgb(1, 0, 3);
const M_SEP_DRAG: Color32 = Color32::from_rgb(1, 0, 4);
const M_DROP: Color32 = Color32::from_rgb(1, 0, 5);
const M_WINDOW: Color32 = Color32::from_rgb(1, 0, 6);
/// Todos los marcadores (para los tests: no tiene que quedar ninguno en la salida).
#[cfg(test)]
pub(super) const MARKERS: [Color32; 5] = [M_SEP_IDLE, M_SEP_HOVER, M_SEP_DRAG, M_DROP, M_WINDOW];

/// El alto de la barra de pestañas: las del kit (15) más la línea grabada (2).
pub(super) const TAB_BAR_H: f32 = kit::tabs::ACTIVE_HEIGHT;
/// Lo que la barra de pestañas deja libre a la derecha: el botón « (`workspace::COLLAPSE`) y un
/// punto de aire. Las pestañas no pasan de ahí (ni la última ni su ×).
pub(super) const COLLAPSE_ROOM: f32 = kit::tabs::ARROW_SIZE + 1.0;
/// La franja de arriba de un grupo sin barra de pestañas: de ahí se agarra para arrastrarlo (con
/// el puntero encima se pinta la línea grabada resaltada, como un divisor).
pub(super) const GRAB_STRIP: f32 = 3.0;
/// Lo que la zona de un divisor crece más allá de su línea (la mitad de cada lado). egui_dock
/// registra los divisores después de los grupos: lo que alcanzan le gana a la franja de agarre del
/// grupo de abajo. Con 1 (medio punto por lado) le dejan 2.5 de los 3 puntos a la franja.
pub(super) const SEPARATOR_EXTRA: f32 = 1.0;
/// El marco de las ventanas flotantes y de la zona de destino (el de los paneles del dock).
const FLYOUT: &str = "menu.side";
/// El relleno de la zona de destino (la franja rosa de las listas, a media luz).
const DROP_FILL: &str = "select.item.hover";
/// La opacidad del relleno de la zona de destino y del contorno de "ventana nueva".
const DROP_ALPHA: f32 = 0.45;
const NEW_WINDOW_ALPHA: f32 = 0.6;
/// La opacidad de la ventana flotante cuya única pestaña se arrastra (ver `drop::see_through`):
/// lo de debajo se ve.
pub(super) const SEE_THROUGH: f32 = 0.4;

/// El estilo de egui_dock con la piel: barra y pestañas transparentes (las del kit van encima),
/// divisores y zona de destino con colores marcador, sin los botones de egui_dock (son líneas).
pub(super) fn dock_style(ui_style: &egui::Style, skin: &Skin) -> egui_dock::Style {
    let colors = *skin.colors();
    let mut style = plain_style(ui_style);
    style.tab.tab_body.hidden_tab_bar_drag_height = Some(GRAB_STRIP);
    style.tab_bar.height = TAB_BAR_H;
    style.tab_bar.bg_fill = colors.background;
    style.tab_bar.hline_color = Color32::TRANSPARENT;
    style.tab_bar.corner_radius = CornerRadius::ZERO;
    style.tab_bar.inner_margin = Margin {
        right: COLLAPSE_ROOM as i8,
        ..Margin::ZERO
    };
    style.tab_bar.show_scroll_bar_on_overflow = false;
    for t in [
        &mut style.tab.active,
        &mut style.tab.inactive,
        &mut style.tab.focused,
        &mut style.tab.hovered,
        &mut style.tab.active_with_kb_focus,
        &mut style.tab.inactive_with_kb_focus,
        &mut style.tab.focused_with_kb_focus,
    ] {
        t.bg_fill = Color32::TRANSPARENT;
        t.outline_color = Color32::TRANSPARENT;
        t.text_color = Color32::TRANSPARENT;
        t.corner_radius = CornerRadius::ZERO;
    }
    style.tab.spacing = kit::tabs::GAP;
    style.tab.hline_below_active_tab_name = false;
    style.tab.tab_body.bg_fill = colors.background;
    style.separator.width = pixel_ui::ETCHED_SIZE;
    style.separator.color_idle = M_SEP_IDLE;
    style.separator.color_hovered = M_SEP_HOVER;
    style.separator.color_dragged = M_SEP_DRAG;
    style.overlay.overlay_type = OverlayType::HighlightedAreas;
    style.overlay.selection_color = M_DROP;
    style.overlay.selection_stroke_width = 2.0;
    style
}

/// Sin la zona de destino de egui_dock ni su contorno de "ventana nueva" (los dos van del color
/// `overlay.selection_color`): mientras rige la de una orilla o una tira (ver `drop::TabDrop`), se
/// pinta solo esa.
pub(super) fn hide_drop_zone(style: &mut egui_dock::Style) {
    style.overlay.selection_color = Color32::TRANSPARENT;
}

/// Lo que va igual con y sin Pixel UI: sin borde ni aire alrededor, la franja para agarrar un
/// grupo sin barra, sin el triángulo de "mostrar la barra" (la barra la decide el workspace cada
/// frame) y sin atenuar (los marcadores tienen que llegar intactos).
pub(super) fn plain_style(ui_style: &egui::Style) -> egui_dock::Style {
    let mut style = egui_dock::Style::from_egui(ui_style);
    style.dock_area_padding = None;
    style.main_surface_border_stroke = Stroke::NONE;
    style.tab.tab_body.stroke = Stroke::NONE;
    style.tab.tab_body.corner_radius = CornerRadius::ZERO;
    // El fondo de los paneles de egui, como el panel derecho de antes (no el de las ventanas).
    style.tab.tab_body.bg_fill = ui_style.visuals.panel_fill;
    // Sin Pixel UI, sin franja de agarre (0: egui_dock no la registra para arrastrar y su rect
    // queda vacío): la del lienzo se tragaría los clics de arriba de sus reglas, que sin Pixel UI
    // llegan hasta arriba de su hoja. Con Pixel UI la pone `dock_style` (ahí el lienzo tiene 5
    // puntos de aire arriba).
    style.tab.tab_body.hidden_tab_bar_drag_height = Some(0.0);
    style.tab_bar.inner_margin.right += COLLAPSE_ROOM as i8;
    style.separator.extra_interact_width = SEPARATOR_EXTRA;
    // De 2 puntos (como el grabado del skin): el centro de un divisor cae en un punto entero y
    // cada lado empieza en uno.
    style.separator.width = pixel_ui::ETCHED_SIZE;
    // Sin el mínimo de 175 por lado de egui_dock (ver el comentario del módulo, punto 11).
    style.separator.extra = 0.0;
    style.buttons.show_tab_bar_size = 0.0;
    style.buttons.show_tab_bar_hover_expand = 0.0;
    style.buttons.show_tab_bar_color = Color32::TRANSPARENT;
    style.buttons.show_tab_bar_active_color = Color32::TRANSPARENT;
    style.overlay.hovered_leaf_highlight.color = Color32::TRANSPARENT;
    style.overlay.hovered_leaf_highlight.stroke = Stroke::NONE;
    style.overlay.surface_fade_opacity = 1.0;
    style
}

/// El estilo de egui para el `Ui` donde va el `DockArea`: el marco de sus ventanas flotantes
/// (`Frame::window`) con el color marcador, sin borde ni sombra y con el margen de `menu.side`.
/// Solo lo leen los marcos: el contenido de cada pestaña usa el estilo del contexto.
pub(super) fn window_visuals(style: &mut egui::Style, skin: &Skin) {
    style.visuals.window_fill = M_WINDOW;
    style.visuals.window_stroke = Stroke::NONE;
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.window_corner_radius = CornerRadius::ZERO;
    style.spacing.window_margin = flyout_margin(skin);
}

/// Los bordes de `menu.side` (lo de fuera del centro de su 9-slice): el margen de las ventanas
/// flotantes.
pub(super) fn flyout_margin(skin: &Skin) -> Margin {
    match skin
        .piece(FLYOUT, WidgetState::Normal)
        .and_then(|p| p.center.map(|c| (p, c)))
    {
        Some((p, c)) => Margin {
            left: c.x as i8,
            top: c.y as i8,
            right: (p.size[0] - c.x - c.w) as i8,
            bottom: (p.size[1] - c.y - c.h) as i8,
        },
        None => Margin::same(2),
    }
}

// ---------------------------------------------------------------------------------------------
// El plugin que cambia los marcadores.

/// Cambia los rects de color marcador por piezas del skin, en los frames en que se mostró el
/// workspace (ver [`arm`]).
#[derive(Default)]
pub(super) struct SkinRewriter;

/// Lo que el workspace le deja al plugin en un frame: el skin (sin Pixel UI, ninguno: solo se
/// atenúa la ventana de `faded`), dónde puede haber marcadores y qué ventana se ve a través.
#[derive(Clone, Default)]
pub(super) struct Armed {
    skin: Option<Arc<Skin>>,
    /// El `DockArea` y sus ventanas flotantes.
    pub areas: Vec<Rect>,
    /// El cuerpo de cada grupo: lo recortado adentro es de los paneles (un cuadro de la paleta
    /// del color justo de un marcador no se toca).
    pub bodies: Vec<Rect>,
    /// Mientras se arrastra una pestaña sobre el lienzo: el rect de su grupo y la mitad que de
    /// verdad ocuparía lo soltado. egui_dock pinta el grupo entero ("como pestaña"), pero el lienzo
    /// no recibe pestañas: `tidy_canvas` lo parte al lado, del lado del cuadrante.
    pub canvas_drop: Option<(Rect, Rect)>,
    /// Mientras se arrastra la única pestaña de una ventana flotante (ver `drop::see_through`):
    /// la capa de esa ventana ([`window_layer`]), que va con [`SEE_THROUGH`] de opacidad.
    pub faded: Option<LayerId>,
    /// Y el rect de su grupo: la zona de destino de egui_dock que es ese grupo o su barra no se
    /// pinta (ver [`own_zone`]).
    pub faded_leaf: Option<Rect>,
}

impl Armed {
    pub(super) fn new(skin: Option<Arc<Skin>>) -> Armed {
        Armed {
            skin,
            ..Default::default()
        }
    }

    /// Si lo recortado a `clip` puede ser de egui_dock: toca el `DockArea` o una ventana, y no
    /// está dentro del cuerpo de un grupo.
    fn in_scope(&self, clip: Rect) -> bool {
        let touches = self.areas.iter().any(|a| a.intersect(clip).area() > 0.0);
        touches
            && !self
                .bodies
                .iter()
                .any(|b| b.expand(0.5).contains_rect(clip))
    }
}

/// Donde [`arm`] deja lo del frame.
fn rewriter_id() -> Id {
    Id::new("workspace_skin_rewriter")
}

/// Registra el plugin (una vez por contexto) y le deja `armed` para este frame.
pub(super) fn arm(ctx: &egui::Context, armed: Armed) {
    if ctx.plugin_opt::<SkinRewriter>().is_none() {
        ctx.add_plugin(SkinRewriter);
    }
    ctx.data_mut(|d| d.insert_temp(rewriter_id(), armed));
}

impl egui::Plugin for SkinRewriter {
    fn debug_name(&self) -> &'static str {
        "workspace_skin_rewriter"
    }

    /// La ventana que se ve a través (`Armed::faded`), cuando ya se pintó todo lo de su capa en
    /// el frame (las pestañas del kit y su « van después del `DockArea`) y antes de que egui junte
    /// las capas: sus marcadores cambiados aquí (ya atenuados no se reconocerían) y todo a
    /// [`SEE_THROUGH`] de opacidad. La pastilla que sigue al puntero va en otra capa: opaca.
    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx();
        let Some(armed) = ctx.data(|d| d.get_temp::<Armed>(rewriter_id())) else {
            return;
        };
        let Some(layer) = armed.faded else {
            return;
        };
        let ppp = ctx.pixels_per_point();
        ctx.graphics_mut(|g| {
            let Some(list) = g.get_mut(layer) else {
                return;
            };
            for i in 0..list.next_idx().0 {
                list.mutate_shape(egui::layers::ShapeIdx(i), |c| {
                    if let Some(skin) = &armed.skin
                        && armed.in_scope(c.clip_rect)
                    {
                        rewrite(&mut c.shape, skin, ppp, &armed);
                    }
                    see_through(&mut c.shape, SEE_THROUGH);
                });
            }
        });
    }

    fn output_hook(&mut self, ctx: &egui::Context, output: &mut egui::FullOutput) {
        // Solo si el workspace se mostró en este frame (y con Pixel UI).
        let armed = ctx
            .data_mut(|d| d.remove_temp::<Armed>(rewriter_id()))
            .unwrap_or_default();
        let Some(skin) = armed.skin.clone() else {
            return;
        };
        let ppp = output.pixels_per_point;
        for clipped in &mut output.shapes {
            if armed.in_scope(clipped.clip_rect) {
                rewrite(&mut clipped.shape, &skin, ppp, &armed);
            }
        }
    }
}

/// Cambia los marcadores de `shape` (y de lo que lleva adentro). La zona de destino que cubre el
/// grupo del lienzo entero (`canvas_drop`) se pinta como la mitad que de verdad queda.
fn rewrite(shape: &mut Shape, skin: &Skin, ppp: f32, armed: &Armed) {
    match shape {
        Shape::Vec(shapes) => {
            for s in shapes {
                rewrite(s, skin, ppp, armed);
            }
        }
        Shape::Rect(r) => {
            let colors = *skin.colors();
            if [M_SEP_IDLE, M_SEP_HOVER, M_SEP_DRAG].contains(&r.fill) {
                // Con el puntero encima o arrastrándolo, la línea oscura en el color de selección.
                let mut c = colors;
                if r.fill != M_SEP_IDLE {
                    c.canvas = colors.text_selection;
                }
                *shape = etched_in(r.rect, &c, ppp);
            } else if r.fill == M_DROP {
                // La de la ventana que se ve a través: no es destino (el primer frame del
                // arrastre egui_dock todavía apunta a ella, ver el punto 12 del módulo).
                if armed.faded_leaf.is_some_and(|leaf| own_zone(r.rect, leaf)) {
                    *shape = Shape::Noop;
                    return;
                }
                // En puntos enteros, como las de la tira (`drop::drop_zone`): egui_dock parte un
                // grupo de alto impar a medio punto.
                let rect = match armed.canvas_drop {
                    Some((canvas, half)) if same_rect(r.rect, canvas) => half,
                    _ => whole(r.rect),
                };
                *shape = Shape::Vec(drop_shapes(skin, rect, ppp));
            } else if r.fill == M_WINDOW {
                *shape = Shape::Vec(frame_shapes(skin, r.rect, ppp));
            } else if r.stroke.color == M_DROP {
                // El contorno de "soltar aquí = ventana nueva" (sin el centro: lo de abajo se ve).
                let mut s = Shape::Vec(ring_shapes(skin, r.rect, ppp));
                fade(&mut s, NEW_WINDOW_ALPHA);
                *shape = s;
            }
        }
        _ => {}
    }
}

/// Si `a` y `b` son el mismo rect (a medio punto: egui_dock lo ajusta a píxel físico).
fn same_rect(a: Rect, b: Rect) -> bool {
    (a.min - b.min).length() < 1.5 && (a.max - b.max).length() < 1.5
}

/// Si la zona de destino `zone` de egui_dock es el grupo `leaf` entero (soltar como pestaña en su
/// cuerpo) o su barra de pestañas (o la franja de agarre, sin barra): lo de arriba del grupo, de
/// su orilla izquierda, a lo más tan ancho como él y del alto de una barra.
fn own_zone(zone: Rect, leaf: Rect) -> bool {
    let top = (zone.min - leaf.min).length() < 1.5;
    top && (same_rect(zone, leaf)
        || (zone.height() <= TAB_BAR_H + 1.5 && zone.right() <= leaf.right() + 1.5))
}

/// Multiplica la opacidad de los rects llenos y de las mallas de `shape` (las piezas del skin son
/// mallas con textura: su tinte va en los vértices).
fn fade(shape: &mut Shape, alpha: f32) {
    match shape {
        Shape::Vec(v) => v.iter_mut().for_each(|s| fade(s, alpha)),
        Shape::Rect(r) => r.fill = r.fill.gamma_multiply(alpha),
        Shape::Mesh(mesh) => Arc::make_mut(mesh)
            .vertices
            .iter_mut()
            .for_each(|v| v.color = v.color.gamma_multiply(alpha)),
        _ => {}
    }
}

/// Multiplica la opacidad de todo `shape` (como `Ui::multiply_opacity`: el texto y los trazos
/// también; el color "el del texto" de las letras, `Color32::PLACEHOLDER`, se queda: lo cambia
/// el teselador por el del texto, que sí se atenúa).
fn see_through(shape: &mut Shape, alpha: f32) {
    egui::epaint::shape_transform::adjust_colors(shape, move |c| {
        if *c != Color32::PLACEHOLDER {
            *c = c.gamma_multiply(alpha);
        }
    });
}

/// Una línea grabada a lo largo del rect: vertical si es alto, horizontal si es ancho.
fn etched_in(rect: Rect, colors: &SkinColors, ppp: f32) -> Shape {
    if rect.height() >= rect.width() {
        pixel_ui::etched_vline(rect.left(), rect.y_range(), colors, ppp)
    } else {
        pixel_ui::etched_hline(rect.x_range(), rect.top(), colors, ppp)
    }
}

/// La zona de destino: el marco `menu.side` sin su centro (el panel de abajo se ve) y, encima,
/// la franja de selección a media luz adentro.
pub(super) fn drop_shapes(skin: &Skin, rect: Rect, ppp: f32) -> Vec<Shape> {
    let mut out = ring_shapes(skin, rect, ppp);
    let inside = rect - flyout_margin(skin);
    let fill = match skin.piece(DROP_FILL, WidgetState::Normal) {
        Some(p) => pixel_ui::piece_shapes(p, inside, ppp),
        None => vec![Shape::rect_filled(
            inside,
            0.0,
            skin.colors().text_selection,
        )],
    };
    let mut fill = Shape::Vec(fill);
    fade(&mut fill, DROP_ALPHA);
    out.push(fill);
    out
}

/// Las zonas de destino de [`drop_shapes`] pintadas en `shapes`: el rect de cada una. Se
/// reconocen por su relleno (lo último de `drop_shapes`, siempre un `Shape::Vec`, se pinten todas
/// juntas, como en [`rewrite`], o una por una).
#[cfg(test)]
pub(crate) fn painted_drop_zones(
    shapes: &[egui::epaint::ClippedShape],
    skin: &Skin,
    ppp: f32,
) -> Vec<Rect> {
    fn walk(shape: &Shape, skin: &Skin, ppp: f32, out: &mut Vec<Rect>) {
        let Shape::Vec(v) = shape else {
            return;
        };
        let zone = shape.visual_bounding_rect() + flyout_margin(skin);
        if zone.is_positive() && drop_shapes(skin, zone, ppp).last() == Some(shape) {
            out.push(zone);
            return;
        }
        v.iter().for_each(|s| walk(s, skin, ppp, out));
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, skin, ppp, &mut out);
    }
    out
}

/// El marco `menu.side` en 9-slice sin su centro (sin el skin, un borde de 1 punto).
fn ring_shapes(skin: &Skin, rect: Rect, ppp: f32) -> Vec<Shape> {
    match skin.piece(FLYOUT, WidgetState::Normal) {
        Some(p) => pixel_ui::piece_ring_shapes(p, rect, ppp),
        None => vec![Shape::rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, skin.colors().outline),
            egui::StrokeKind::Inside,
        )],
    }
}

/// El marco `menu.side` en 9-slice.
fn frame_shapes(skin: &Skin, rect: Rect, ppp: f32) -> Vec<Shape> {
    match skin.piece(FLYOUT, WidgetState::Normal) {
        Some(p) => pixel_ui::piece_shapes(p, rect, ppp),
        None => vec![Shape::rect_filled(rect, 0.0, skin.colors().background)],
    }
}

// ---------------------------------------------------------------------------------------------
// Ids internos de egui_dock 0.21.1 (ver el comentario del módulo).

/// El área de egui de la ventana flotante `surface` (`show/window_surface.rs`). Su rect real:
/// `WindowState::rect()` viene vacío.
pub(super) fn window_area_id(surface: SurfaceIndex) -> Id {
    Id::new(format!("window {surface:?}"))
}

/// La capa de la ventana flotante `surface` (la de su área).
pub(super) fn window_layer(surface: SurfaceIndex) -> LayerId {
    LayerId::new(Order::Middle, window_area_id(surface))
}

/// Sin el marco de las ventanas del workspace (`menu.side`) en la ventana flotante `surface` (la
/// del Preview solo, que lleva el de su diálogo): su marcador, ya pintado en la capa de su área,
/// se borra. Por la capa y no por el rect: mientras se agranda de la agarradera, el área puede ir
/// un frame atrás del tamaño pedido.
pub(super) fn hide_window_frame(ctx: &egui::Context, surface: SurfaceIndex) {
    fn unmark(shape: &mut Shape) {
        match shape {
            Shape::Vec(shapes) => shapes.iter_mut().for_each(unmark),
            Shape::Rect(r) if r.fill == M_WINDOW => *shape = Shape::Noop,
            _ => {}
        }
    }
    let layer = egui::LayerId::new(egui::Order::Middle, window_area_id(surface));
    ctx.graphics_mut(|g| {
        if let Some(list) = g.get_mut(layer) {
            for i in 0..list.next_idx().0 {
                list.mutate_shape(egui::layers::ShapeIdx(i), |c| unmark(&mut c.shape));
            }
        }
    });
}

/// Si `response` (la que llega a `on_tab_button`) es la de una pestaña que se está arrastrando:
/// mientras se arrastra, egui_dock la pinta aparte y da la respuesta de `id.with("dragged")`
/// (`show/leaf.rs`).
pub(super) fn is_dragged_tab(response: &egui::Response) -> bool {
    response
        .ctx
        .dragged_id()
        .is_some_and(|d| d == response.id || d.with("dragged") == response.id)
        && response.ctx.input(|i| i.pointer.is_decidedly_dragging())
}

/// El id de la franja de agarre del grupo `path` sin barra de pestañas
/// (`hidden_tab_bar_controls` en `show/leaf.rs`).
pub(super) fn hidden_strip_id(dock_id: Id, path: NodePath) -> Id {
    dock_id
        .with((path.surface, "surface"))
        .with((path.node, "node"))
        .with("hidden_tab_bar_drag")
}

/// Los ids de lo que arrastra el lienzo en egui_dock: la franja de agarre de su grupo (sin barra)
/// y su pestaña (`tab_title` en `show/leaf.rs`: el id del nodo con `(tab, "tab")`), por si un día
/// lleva barra. Con ellos `fit::block_canvas_drag` le corta el arrastre.
pub(super) fn canvas_drag_ids(dock_id: Id, path: egui_dock::TabPath) -> [Id; 2] {
    let node = path.node_path();
    [
        hidden_strip_id(dock_id, node),
        dock_id
            .with((node.surface, "surface"))
            .with((node.node, "node"))
            .with((path.tab.0, "tab")),
    ]
}

/// Si se está arrastrando el grupo `path` (sin barra de pestañas) desde su franja de arriba.
pub(super) fn hidden_strip_dragged(ctx: &egui::Context, dock_id: Id, path: NodePath) -> bool {
    ctx.is_being_dragged(hidden_strip_id(dock_id, path))
        && ctx.input(|i| i.pointer.is_decidedly_dragging())
}

/// Si el puntero está sobre la franja de agarre del grupo `path` y es ella la que lo recibe (no
/// un divisor que la pisa).
pub(super) fn hidden_strip_hovered(ctx: &egui::Context, dock_id: Id, path: NodePath) -> bool {
    ctx.read_response(hidden_strip_id(dock_id, path))
        .is_some_and(|r| r.hovered())
}

// ---------------------------------------------------------------------------------------------
// Puntos enteros.

/// Los divisores a puntos enteros (para el frame siguiente): la fracción de cada split, de modo
/// que el punto medio caiga en un entero. Con un botón apretado no: un arrastre lento se pegaría
/// o se adelantaría.
pub(super) fn snap<T>(ctx: &egui::Context, state: &mut DockState<T>) {
    if ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    for surface in state.iter_surfaces_mut() {
        let Some(tree) = surface.node_tree_mut() else {
            continue;
        };
        for node in tree.iter_mut() {
            let (split, horizontal) = match node {
                Node::Horizontal(s) => (s, true),
                Node::Vertical(s) => (s, false),
                _ => continue,
            };
            let (min, len) = if horizontal {
                (split.rect.min.x, split.rect.width())
            } else {
                (split.rect.min.y, split.rect.height())
            };
            if len > 1.0 && len.is_finite() {
                let mid = (min + len * split.fraction).round();
                split.fraction = ((mid - min) / len).clamp(0.0, 1.0);
            }
        }
    }
}

/// Las ventanas flotantes a puntos enteros al soltarlas (arrastradas quedan en x.5), salvo
/// `keep` (la del Preview en su esquina: va donde iba su ventana de antes, que con una pantalla
/// que termina en medio punto queda en medio punto). Es una excepción deliberada a "rects en
/// puntos enteros": así se ve igual que la de antes (0 píxeles distintos).
pub(super) fn snap_windows<T>(
    ctx: &egui::Context,
    state: &mut DockState<T>,
    keep: Option<SurfaceIndex>,
) {
    if ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    let windows: Vec<SurfaceIndex> = state
        .iter_surfaces_indexed()
        .filter(|(i, s)| !i.is_main() && !s.is_empty() && Some(*i) != keep)
        .map(|(i, _)| i)
        .collect();
    for s in windows {
        let Some(rect) = ctx.memory(|m| m.area_rect(window_area_id(s))) else {
            continue;
        };
        let at = rect.min.round();
        if at != rect.min
            && let Some(window) = state.get_window_state_mut(s)
        {
            window.set_position(at);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Las pestañas del kit.

/// La pestaña de un panel que dibujó egui_dock (transparente) en este frame.
pub(super) struct TabHit {
    pub id: PanelId,
    pub response: egui::Response,
}

/// La pestaña del kit de `id`: con su × si se puede cerrar.
pub(super) fn tab_item(id: PanelId) -> TabItem {
    let title = id.spec().title;
    if id.spec().closable {
        TabItem::closable(title, false)
    } else {
        TabItem::new(title)
    }
}

// ---------------------------------------------------------------------------------------------
// Las barras con más pestañas de las que caben.

/// El aire entre las flechas del kit y el « del grupo.
const ARROWS_AIR: f32 = 1.0;

/// Cómo van las pestañas del kit en la barra de un grupo (con Pixel UI). Las acomoda
/// `kit::tabs::layout` en la barra sin el lugar del « ([`COLLAPSE_ROOM`]): si no caben, se achican
/// parejo y, si ni así, van las flechas del kit a la izquierda del « (con [`ARROWS_AIR`] de aire) y
/// se recorren con ellas (o con la rueda) de una en una; ni la última ni su × quedan bajo el «.
///
/// egui_dock (que les da el clic y las arrastra) las pone en el mismo lugar: cada una del ancho del
/// kit (`TabStyle::minimum_width`, ver [`TabBar::dock_width`]) y corridas a la izquierda lo que
/// ocupan las de antes de la primera que se ve (`LeafNode::scroll`). egui_dock no las deja correr
/// más que hasta que la última toca el final de su barra y el kit deja la última pegada a la
/// izquierda (con lugar libre después): la última lleva de más lo que falte (`tail`), que queda
/// bajo las flechas y el aire de antes de ellas (ver `tabs::tab_arrows`, que les ganan el puntero).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TabBar {
    pub path: NodePath,
    pub tabs: Vec<PanelId>,
    /// El ancho de cada pestaña (de todas, en el orden del grupo) y su nombre como entra.
    pub widths: Vec<f32>,
    pub names: Vec<String>,
    /// Desde cuál se ven y desde cuál se ve la última.
    pub offset: usize,
    pub last_offset: usize,
    /// Las que se ven: su índice en el grupo y su fila (sin lo que la activa baja).
    pub shown: Vec<(usize, Rect)>,
    /// Las flechas, si no caben.
    pub arrows: Option<Rect>,
    /// Lo que egui_dock las corre a la izquierda y lo que lleva de más la última.
    pub scroll: f32,
    pub tail: f32,
    /// De la orilla izquierda de la barra al «: hasta ahí llegan las pestañas de egui_dock.
    pub room: Rect,
}

impl TabBar {
    /// Lo que egui_dock tiene que saber para ponerlas igual: si cambia entre lo pedido antes del
    /// `DockArea` y lo de después (el grupo cambió de ancho), el frame se vuelve a dibujar.
    pub(super) fn dock_layout(&self) -> (NodePath, &[PanelId], &[f32], f32, f32) {
        (self.path, &self.tabs, &self.widths, self.scroll, self.tail)
    }

    /// El ancho de la pestaña `i` en egui_dock (la última, con lo de más).
    pub(super) fn dock_width(&self, i: usize) -> f32 {
        let tail = if i + 1 == self.tabs.len() {
            self.tail
        } else {
            0.0
        };
        self.widths[i] + tail
    }

    /// La fila de `id`, si se ve.
    pub(super) fn row(&self, id: PanelId) -> Option<Rect> {
        let i = self.tabs.iter().position(|&t| t == id)?;
        self.shown.iter().find(|(j, _)| *j == i).map(|(_, r)| *r)
    }

    /// Lo que se ve de las pestañas: de la orilla izquierda de la barra a las flechas (o al «).
    pub(super) fn visible(&self) -> Rect {
        match self.arrows {
            Some(a) => self.room.with_max_x(a.left()),
            None => self.room,
        }
    }

    pub(super) fn can_prev(&self) -> bool {
        self.offset > 0
    }

    pub(super) fn can_next(&self) -> bool {
        self.shown
            .last()
            .is_some_and(|(i, _)| i + 1 < self.tabs.len())
    }
}

/// Dónde se guarda desde cuál se ven las pestañas del grupo `path` (y cuál estaba al frente: la
/// que pasa al frente se trae a la vista, como en `kit::tabs::tab_strip`).
pub(super) fn tabs_offset_id(path: NodePath) -> Id {
    Id::new(("workspace_tabs_offset", path.surface, path.node))
}

/// La barra del grupo `path` (de rect `leaf`) con `tabs` y la de al frente `active`, desde la que
/// se ven según la memoria; `step` la corre antes (la rueda: −1 o 1). Guarda desde cuál se ven.
pub(super) fn plan_bar(
    ui: &egui::Ui,
    path: NodePath,
    leaf: Rect,
    (tabs, active): (&[PanelId], usize),
    step: i32,
) -> TabBar {
    use kit::tabs::{GAP, HEIGHT, layout};
    let memory = tabs_offset_id(path);
    let items: Vec<TabItem> = tabs.iter().map(|&t| tab_item(t)).collect();
    let (offset, seen): (usize, Option<PanelId>) =
        ui.data(|d| d.get_temp(memory)).unwrap_or((0, None));
    let offset = offset.saturating_add_signed(step as isize);
    let room = Rect::from_min_max(
        leaf.min,
        egui::pos2(
            (leaf.right() - COLLAPSE_ROOM).max(leaf.left()),
            leaf.top() + TAB_BAR_H,
        ),
    );
    let mut strip = Strip::new(room.left(), room.right(), room.top());
    let mut lay = layout(ui, &strip, &items, offset);
    if lay.arrows.is_some() {
        strip.right = (room.right() - ARROWS_AIR).max(room.left());
        lay = layout(ui, &strip, &items, offset);
    }
    // La que pasó al frente, a la vista.
    let front = tabs.get(active).copied();
    if lay.arrows.is_some() && seen != front && active < tabs.len() {
        if active < lay.offset {
            lay = layout(ui, &strip, &items, active);
        }
        while lay.placed.last().is_some_and(|p| p.index < active) && lay.offset < lay.last_offset {
            lay = layout(ui, &strip, &items, lay.offset + 1);
        }
    }
    ui.data_mut(|d| d.insert_temp(memory, (lay.offset, front)));
    let widths = kit::tabs::widths(ui, &strip, &items);
    let n = tabs.len();
    let (scroll, tail) = if lay.arrows.is_some() {
        let scroll: f32 = widths[..lay.offset].iter().map(|w| w + GAP).sum();
        let total = widths.iter().sum::<f32>() + GAP * n.saturating_sub(1) as f32;
        (scroll, (scroll - (total - room.width())).max(0.0))
    } else {
        (0.0, 0.0)
    };
    let mut names: Vec<String> = items.iter().map(|t| t.label.clone()).collect();
    for p in &lay.placed {
        names[p.index] = p.name.clone();
    }
    TabBar {
        path,
        tabs: tabs.to_vec(),
        widths,
        names,
        offset: lay.offset,
        last_offset: lay.last_offset,
        shown: lay
            .placed
            .iter()
            .map(|p| {
                (
                    p.index,
                    Rect::from_min_size(p.rect.min, vec2(p.rect.width(), HEIGHT)),
                )
            })
            .collect(),
        arrows: lay.arrows,
        scroll,
        tail,
        room,
    }
}

/// La rueda de este frame sobre las barras: dónde está el puntero, cuánto corrió (en puntos;
/// positivo, hacia atrás) y cuánto es un renglón de la rueda (una pestaña).
#[derive(Clone, Copy, Debug)]
pub(super) struct Wheel {
    pub at: egui::Pos2,
    pub delta: f32,
    pub notch: f32,
}

/// Dónde se junta lo que corrió la rueda sobre la barra del grupo `path` hasta un renglón.
fn wheel_sum_id(path: NodePath) -> Id {
    Id::new(("workspace_tabs_wheel", path.surface, path.node))
}

/// Lo que corre la barra de `path` la rueda (`wheel`): lo de este frame se suma a lo de antes y
/// cada renglón junto es una pestaña (−1: hacia atrás, 1: hacia adelante), de a una por frame; lo
/// que sobra se tira. Cambiar de sentido empieza de cero (un trackpad no corre una pestaña por
/// cada evento).
fn wheel_step(ui: &egui::Ui, path: NodePath, wheel: Wheel) -> i32 {
    let id = wheel_sum_id(path);
    let before = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let sum = if before * wheel.delta < 0.0 {
        wheel.delta
    } else {
        before + wheel.delta
    };
    let (step, left) = if sum >= wheel.notch {
        (-1, 0.0)
    } else if sum <= -wheel.notch {
        (1, 0.0)
    } else {
        (0, sum)
    };
    ui.data_mut(|d| d.insert_temp(id, left));
    step
}

/// Las barras de los grupos de `state` que llevan barra de pestañas (con su rect del último
/// frame que se dibujó). `wheel`: la rueda sobre la de alguno (la que tiene flechas) la corre, si
/// lo de más arriba bajo el puntero es su capa (`on_top`, con la superficie del grupo): no a
/// través de una ventana, un diálogo o un menú. Lo que la rueda dejó juntado en una barra (menos
/// de un renglón) se olvida cuando el puntero no está sobre ella.
pub(super) fn plan_bars(
    ui: &egui::Ui,
    state: &DockState<PanelId>,
    wheel: Option<Wheel>,
    on_top: impl Fn(SurfaceIndex, egui::Pos2) -> bool,
) -> Vec<TabBar> {
    state
        .iter_leaves()
        .filter(|(_, leaf)| !leaf.tab_bar_hidden && !leaf.tabs.is_empty())
        .filter(|(_, leaf)| leaf.rect.is_positive())
        .map(|(path, leaf)| {
            let row = Rect::from_min_size(leaf.rect.min, vec2(leaf.rect.width(), TAB_BAR_H));
            if !ui
                .ctx()
                .pointer_hover_pos()
                .is_some_and(|p| row.contains(p))
            {
                ui.data_mut(|d| d.remove::<f32>(wheel_sum_id(path)));
            }
            let wheel = wheel.filter(|w| row.contains(w.at) && on_top(path.surface, w.at));
            let bar = plan_bar(ui, path, leaf.rect, (&leaf.tabs, leaf.active.0), 0);
            let step = match wheel {
                Some(w) if bar.arrows.is_some() => wheel_step(ui, path, w),
                _ => 0,
            };
            if step != 0 {
                plan_bar(ui, path, leaf.rect, (&leaf.tabs, leaf.active.0), step)
            } else {
                bar
            }
        })
        .collect()
}

/// Las pestañas del kit sobre las de egui_dock, en su misma capa: por cada grupo con barra (ver
/// [`TabBar`]), la línea grabada cortada debajo de la activa y las pestañas que se ven, con su
/// nombre como entra (la activa al final, tapa a sus vecinas; las flechas las pone
/// `tabs::tab_arrows`); la que se arrastra, además, como fantasma (la pastilla del kit con su
/// nombre, `kit::tabs::drag_tag_shapes`). En los grupos sin barra (`hidden`), la franja de arriba
/// resaltada con el puntero encima, y su fantasma (la misma pastilla) al arrastrarla.
pub(super) fn paint_tabs(
    ui: &egui::Ui,
    skin: &Skin,
    state: &DockState<PanelId>,
    (hits, bars): (&[TabHit], &[TabBar]),
    dock_id: Id,
) {
    let ctx = ui.ctx();
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let press = ui.input(|i| i.pointer.press_origin());
    let hit_of = |p: PanelId| hits.iter().find(|h| h.id == p);
    // La pastilla del kit con el nombre del panel, con su esquina donde estaba la de lo agarrado
    // (`grabbed`), corrida lo arrastrado.
    let ghost = |grabbed: egui::Pos2, id: PanelId| {
        if let (Some(p), Some(o)) = (pointer, press) {
            let painter =
                ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("workspace_ghost")));
            let at = grabbed + (p - o).round();
            painter.extend(kit::tabs::drag_tag_shapes(ui, skin, at, id.spec().title));
        }
    };
    for (path, leaf) in state.iter_leaves() {
        let Some(&active) = leaf.tabs.get(leaf.active.0) else {
            continue;
        };
        if leaf.tab_bar_hidden {
            // El lienzo no se agarra (su arrastre se corta): su franja no se resalta.
            if active == PanelId::Canvas {
                continue;
            }
            let strip = Rect::from_min_size(leaf.rect.min, vec2(leaf.rect.width(), GRAB_STRIP));
            let dragged = hidden_strip_dragged(ctx, dock_id, path);
            if dragged || hidden_strip_hovered(ctx, dock_id, path) {
                let mut c = *skin.colors();
                c.canvas = c.text_selection;
                let line = pixel_ui::etched_hline(
                    strip.x_range(),
                    strip.top(),
                    &c,
                    ctx.pixels_per_point(),
                );
                ui.painter().with_clip_rect(strip).add(line);
            }
            if dragged {
                ghost(strip.min, active);
            }
            continue;
        }
        // Sin pestañas dibujadas (el grupo no se ve) o sin su barra acomodada, nada.
        let Some(first) = leaf.tabs.first().and_then(|&t| hit_of(t)) else {
            continue;
        };
        let Some(plan) = bars.iter().find(|b| b.path == path && b.tabs == leaf.tabs) else {
            continue;
        };
        let bar = Rect::from_min_size(leaf.rect.min, vec2(leaf.rect.width(), TAB_BAR_H));
        let screen = first.response.ctx.content_rect();
        let layer = ctx.layer_painter(first.response.layer_id);
        // La línea, a todo lo ancho; las pestañas, las que se ven (sin tapar las flechas ni el «).
        let painter = layer.with_clip_rect(plan.visible().intersect(screen));
        let active_rect = plan
            .row(active)
            .map(|r| r.with_max_y(r.top() + kit::tabs::ACTIVE_HEIGHT));
        let strip = Strip::new(bar.left(), bar.right(), bar.top());
        layer
            .with_clip_rect(bar.intersect(screen))
            .extend(kit::tabs::line_shapes(ui, skin, &strip, active_rect));
        let mut order: Vec<(usize, Rect)> = plan.shown.clone();
        order.sort_by_key(|&(i, _)| plan.tabs[i] == active);
        for (i, row) in order {
            let id = plan.tabs[i];
            let Some(h) = hit_of(id) else { continue };
            let item = tab_item(id);
            let mark = tab_mark(plan, i);
            let mark_hovered =
                pointer.is_some_and(|p| mark.is_some_and(|m| m.expand(2.0).contains(p)));
            let look = TabLook {
                active: id == active,
                hovered: id != active && h.response.hovered(),
                mark_hovered,
            };
            let shown = if mark.is_some() {
                item.clone()
            } else {
                TabItem {
                    mark: kit::tabs::TabMark::None,
                    ..item.clone()
                }
            };
            painter.extend(kit::tabs::tab_shapes(
                ui,
                skin,
                row,
                &shown,
                &plan.names[i],
                look,
            ));
        }
        // La que se arrastra (solo se agarra una que se ve), desde la esquina de su pestaña del
        // kit (la de egui_dock no cuenta el aire entre pestañas).
        for &id in &plan.tabs {
            if let Some(h) = hit_of(id)
                && is_dragged_tab(&h.response)
            {
                ghost(plan.row(id).map_or(h.response.rect.min, |r| r.min), id);
            }
        }
    }
}

/// La × de la pestaña `i` de `plan`, si se ve y le cabe (como en `kit::tabs::tab_strip`: en una
/// pestaña más angosta que su marco no hay ×).
pub(super) fn tab_mark(plan: &TabBar, i: usize) -> Option<Rect> {
    let item = tab_item(plan.tabs[i]);
    let (_, row) = plan.shown.iter().find(|(j, _)| *j == i)?;
    (item.mark != kit::tabs::TabMark::None && row.width() >= kit::tabs::chrome(&item))
        .then(|| kit::tabs::mark_rect(*row))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_survive_egui_docks_fade() {
        // egui_dock atenúa sus colores con `linear_multiply(surface_fade_opacity)` (1.0): la vuelta
        // por el espacio lineal no tiene que cambiar un marcador.
        for m in MARKERS {
            assert_eq!(m.linear_multiply(1.0), m, "{m:?}");
        }
    }

    /// Un split de Color y Library en 0.37 de un rect de 301 de alto (el medio cae en x.37).
    fn split_state() -> DockState<PanelId> {
        let mut state = DockState::new(vec![PanelId::Color]);
        let [_, _] = state.main_surface_mut().split_below(
            egui_dock::NodeIndex::root(),
            0.37,
            vec![PanelId::Library],
        );
        let root = &mut state.main_surface_mut()[egui_dock::NodeIndex::root()];
        root.set_rect(Rect::from_min_size(
            egui::pos2(0.0, 10.0),
            vec2(200.0, 301.0),
        ));
        state
    }

    fn fraction(state: &DockState<PanelId>) -> f32 {
        match &state.main_surface()[egui_dock::NodeIndex::root()] {
            Node::Vertical(split) => split.fraction,
            _ => panic!("a vertical split"),
        }
    }

    #[test]
    fn snapping_puts_a_split_on_a_whole_point() {
        let mut state = split_state();
        snap(&egui::Context::default(), &mut state);
        let mid = 10.0 + 301.0 * fraction(&state);
        assert!((mid - mid.round()).abs() < 1e-3, "{mid}");
    }

    #[test]
    fn splits_are_not_snapped_while_a_button_is_down() {
        let ctx = egui::Context::default();
        let at = egui::pos2(50.0, 50.0);
        let input = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |_| {});
        output.textures_delta.clear();
        let mut state = split_state();
        snap(&ctx, &mut state);
        assert_eq!(fraction(&state), 0.37, "an ongoing drag is left alone");
    }

    #[test]
    fn fading_reaches_the_meshes_of_skin_pieces() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(10.0, 10.0));
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let piece = Shape::image(egui::TextureId::default(), rect, uv, Color32::WHITE);
        let mut shape = Shape::Vec(vec![piece]);
        fade(&mut shape, 0.5);
        let Shape::Vec(v) = &shape else { panic!() };
        let Shape::Mesh(mesh) = &v[0] else {
            panic!("a piece is a mesh")
        };
        assert!(mesh.vertices.iter().all(|v| v.color.a() < 255), "{mesh:?}");
    }

    #[test]
    fn seeing_through_fades_every_color_but_the_text_placeholder() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(10.0, 10.0));
        let faded = Color32::RED.gamma_multiply(SEE_THROUGH);
        let mut fill = Shape::rect_filled(rect, 0.0, Color32::RED);
        see_through(&mut fill, SEE_THROUGH);
        let Shape::Rect(r) = &fill else { panic!() };
        assert_eq!(r.fill, faded);
        // Las letras de un texto van con `PLACEHOLDER` ("el color del texto"): lo cambia el
        // teselador, y el color del texto ya va atenuado.
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(rect.min, Color32::PLACEHOLDER);
        mesh.colored_vertex(rect.max, Color32::RED);
        let mut shape = Shape::Vec(vec![Shape::mesh(mesh)]);
        see_through(&mut shape, SEE_THROUGH);
        let Shape::Vec(v) = &shape else { panic!() };
        let Shape::Mesh(mesh) = &v[0] else { panic!() };
        let colors: Vec<Color32> = mesh.vertices.iter().map(|v| v.color).collect();
        assert_eq!(colors, [Color32::PLACEHOLDER, faded]);
    }

    #[test]
    fn the_zone_of_the_faded_window_is_its_group_or_its_tab_bar_only() {
        let leaf = Rect::from_min_max(egui::pos2(429.0, 95.0), egui::pos2(649.0, 393.0));
        let at = |x0, y0, x1, y1| Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1));
        // El grupo entero, su barra (sin lo del «) y la franja de agarre de un grupo sin barra, a
        // medio punto.
        assert!(own_zone(leaf, leaf));
        assert!(own_zone(leaf.translate(vec2(0.5, 0.5)), leaf));
        assert!(own_zone(at(429.0, 95.0, 632.0, 112.0), leaf));
        assert!(own_zone(at(429.0, 95.0, 649.0, 98.0), leaf));
        // Lo de otro grupo (debajo o encima de ella), no: la mitad de abajo, una barra más abajo,
        // uno más ancho o el lienzo entero.
        assert!(!own_zone(leaf.with_min_y(leaf.center().y), leaf));
        assert!(!own_zone(at(429.0, 120.0, 632.0, 137.0), leaf));
        assert!(!own_zone(at(429.0, 95.0, 900.0, 112.0), leaf));
        assert!(!own_zone(at(37.0, 63.0, 937.0, 780.0), leaf));
    }

    #[test]
    fn the_drop_zone_over_the_whole_canvas_is_painted_as_the_half_that_results() {
        let skin = crate::skin::Skin::embedded(&egui::Context::default());
        let canvas = Rect::from_min_max(egui::pos2(37.0, 63.0), egui::pos2(937.0, 780.0));
        let half = canvas.with_max_x(canvas.center().x);
        let bounds = |shape: &Shape| shape.visual_bounding_rect();
        // egui_dock pinta el lienzo entero ("como pestaña", a medio píxel): se pinta la mitad.
        let armed = Armed {
            canvas_drop: Some((canvas, half)),
            ..Default::default()
        };
        let mut zone = Shape::rect_filled(canvas.translate(vec2(0.5, 0.0)), 0.0, M_DROP);
        rewrite(&mut zone, &skin, 2.0, &armed);
        assert_eq!(bounds(&zone), half);
        // Otra zona (un borde del lienzo, un grupo) queda como la pinta egui_dock, en puntos
        // enteros (el lienzo es de alto impar: egui_dock lo parte en 421.5).
        let other = canvas.with_min_y(canvas.center().y);
        assert_eq!(other.top(), 421.5);
        let mut zone = Shape::rect_filled(other, 0.0, M_DROP);
        rewrite(&mut zone, &skin, 2.0, &armed);
        assert_eq!(bounds(&zone), other.with_min_y(422.0));
    }

    #[test]
    fn markers_are_only_rewritten_outside_the_panel_bodies() {
        let mut armed = Armed::default();
        armed.areas.push(Rect::from_min_max(
            egui::pos2(900.0, 0.0),
            egui::pos2(1280.0, 800.0),
        ));
        let body = Rect::from_min_max(egui::pos2(900.0, 0.0), egui::pos2(1280.0, 399.0));
        armed.bodies.push(body);
        // Lo recortado al DockArea (un divisor) o a la pantalla (la zona de destino), sí.
        assert!(armed.in_scope(armed.areas[0]));
        assert!(armed.in_scope(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))));
        // Lo de adentro de un panel (un cuadro de la paleta) o fuera del DockArea, no.
        assert!(!armed.in_scope(body.shrink(8.0)));
        assert!(!armed.in_scope(Rect::from_min_size(egui::Pos2::ZERO, vec2(300.0, 200.0))));
    }
}
