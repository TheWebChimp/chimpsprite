//! Con Pixel UI, la barra de estado del diseño (frame 4): 20 de alto con la línea grabada arriba
//! y secciones de ícono + texto (artboard, puntero, frame) separadas por líneas grabadas
//! verticales; después, sin ícono, lo que la barra avisa a veces (zoom, lo flotante…).

use eframe::egui::{Color32, Rect, Sense, Shape, TextStyle, Ui, pos2, vec2};

use crate::pixel_draw;
use crate::pixel_ui;
use crate::skin::Skin;
use crate::tab_bar;

/// Alto de la barra, con la línea grabada de arriba.
pub const HEIGHT: f32 = 20.0;
/// El aire de todo: del borde al ícono, del ícono al texto, del texto a la línea y de la línea
/// a lo siguiente.
const AIR: f32 = 3.0;
/// Fila de las mayúsculas (de 5, los textos chicos), desde arriba de la barra: centradas donde
/// iban las de 7 (filas 7..13).
const CAPS_TOP: f32 = 8.0;
/// Fila de abajo de los íconos (todos apoyados ahí), desde arriba de la barra.
const ICON_BOTTOM: f32 = 17.0;

/// Los íconos de las secciones: piezas del skin.
pub const ICON_ARTBOARD: &str = "icon.status.artboard";
pub const ICON_POINTER: &str = "icon.status.pointer";
pub const ICON_FRAME: &str = "icon.status.frame";

/// Una sección: ícono (opcional), texto y su color.
pub struct Section<'a> {
    /// La pieza del skin del ícono.
    pub icon: Option<&'a str>,
    pub text: String,
    pub color: Option<Color32>,
    /// Un texto de muestra para el ancho mínimo (la sección no cambia de ancho con su texto).
    pub min_text: Option<&'a str>,
}

/// La barra en `ui` (todo su ancho): la línea grabada arriba y las secciones de izquierda a
/// derecha. Cada texto queda en el árbol de accesibilidad como etiqueta.
pub fn show(ui: &mut Ui, skin: &Skin, sections: &[Section]) {
    let bar = ui.max_rect();
    let bar = Rect::from_min_size(bar.min.round(), vec2(bar.width(), HEIGHT));
    ui.allocate_rect(bar, Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let etched = pixel_ui::ETCHED_SIZE;
    let mut shapes = vec![pixel_ui::etched_hline(
        bar.x_range(),
        bar.top(),
        &colors,
        ppp,
    )];
    // Los textos chicos (chimpfont).
    let font = TextStyle::Small.resolve(ui.style());
    let mut x = bar.left() + AIR;
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            // La línea grabada vertical: oscura y clara, desde abajo de la oscura de arriba.
            let top = bar.top() + 1.0;
            let dark = Rect::from_min_max(pos2(x, top), pos2(x + 1.0, bar.bottom()));
            shapes.push(Shape::rect_filled(dark, 0.0, colors.canvas));
            shapes.push(Shape::rect_filled(
                dark.translate(vec2(1.0, 0.0)),
                0.0,
                colors.highlight,
            ));
            x += etched + AIR;
        }
        if let Some(name) = section.icon {
            let size = pixel_draw::piece_size(skin, name);
            let at = pos2(x, bar.top() + ICON_BOTTOM - size.y);
            shapes.extend(pixel_draw::piece_at(skin, name, at, ppp));
            x += size.x + AIR;
        }
        let color = section.color.unwrap_or(colors.text);
        let galley = ui
            .painter()
            .layout_no_wrap(section.text.clone(), font.clone(), color);
        // Con `min_text`, al menos su ancho: los números que cambian no mueven lo de la derecha.
        let min = section.min_text.map_or(0.0, |sample| {
            ui.painter()
                .layout_no_wrap(sample.to_owned(), font.clone(), color)
                .size()
                .x
        });
        let width = galley.size().x.max(min).ceil();
        let at = tab_bar::text_pos(&galley, x, bar.top() + CAPS_TOP, ppp);
        let label = Rect::from_min_size(pos2(x, bar.top() + etched), vec2(width, HEIGHT - etched));
        let response = ui.interact(label, ui.id().with(("status", i)), Sense::hover());
        let text = section.text.clone();
        response.widget_info(|| {
            eframe::egui::WidgetInfo::labeled(eframe::egui::WidgetType::Label, true, &text)
        });
        shapes.push(Shape::galley(at, galley, color));
        x += width + AIR;
    }
    ui.painter().with_clip_rect(bar).extend(shapes);
}
