//! Groom, en el dock: de una imagen con pixel art falso (agrandada y borrosa, como las de una IA)
//! a pixel art de verdad. Se importa la imagen (o se toma la capa actual), se ajustan los pasos
//! (la cuadrícula, quitar el fondo, el contorno y los colores) mirando la vista previa, y el
//! resultado va a un sprite nuevo o a una capa.

use cs_core::Rgba;
use cs_io::export::Bitmap;
use cs_raster::GridGuess;
use eframe::egui::{
    self, Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, Ui, pos2, vec2,
};

use crate::{pixel_ui, widgets};

/// Las imágenes que se pueden importar (en la web, solo PNG).
pub const FILTER: &[crate::platform::FileFilter] = &[crate::platform::FileFilter {
    name: "Images",
    extensions: if cfg!(target_arch = "wasm32") {
        &["png"]
    } else {
        &["png", "jpg", "jpeg", "gif", "webp"]
    },
}];

/// El ancho de cada columna: a la izquierda los ajustes y la vista previa, a la derecha los
/// colores.
const COLUMN: f32 = 210.0;
const COLUMN_GAP: f32 = 14.0;
/// La vista previa: el lado más largo.
const PREVIEW: f32 = COLUMN;
/// Cuántas sugerencias se muestran.
const MAX_SUGGESTIONS: usize = 10;
/// La lupa sobre la vista previa: su lado y cuántos píxeles del resultado se ven en ella.
const LOUPE: f32 = 120.0;
const LOUPE_PIXELS: f32 = 15.0;

/// Lo que se ve en la vista previa.
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    /// Con los colores juntados.
    Result,
    /// Con "Before" presionado: sin el grupo que se está probando o, si no hay, sin juntar
    /// ninguno.
    Before,
    /// Con los juntados y además este grupo (el que se está por juntar).
    Trying(Vec<Rgba>),
}

/// Los ajustes de los pasos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub grid: GridGuess,
    pub background: bool,
    pub tolerance: u8,
    pub clean: bool,
    /// Un contorno de 1 px del color primario alrededor de todo.
    pub ring: bool,
    pub reduce: bool,
    pub colors: u16,
}

impl Settings {
    fn new(grid: GridGuess) -> Self {
        Self {
            grid,
            background: true,
            tolerance: 32,
            clean: true,
            ring: false,
            reduce: false,
            colors: 32,
        }
    }
}

/// El resultado: los píxeles (recortados a lo pintado) y sus colores, del más usado al menos.
#[derive(Clone, Debug, PartialEq)]
pub struct Groomed {
    pub name: String,
    pub image: Bitmap,
    pub colors: Vec<(Rgba, usize)>,
}

/// Colores juntados a mano: los de cada grupo pasan a ser el primero (el más usado).
pub type Merges = Vec<Vec<Rgba>>;

/// Cuántos colores se muestran para elegir (los más usados).
const MAX_SWATCHES: usize = 96;
/// Un cuadrito de color y su separación.
const SWATCH: f32 = 11.0;
const SWATCH_GAP: f32 = 2.0;

/// Lo que se pidió desde el panel (la app lo resuelve).
pub enum GroomAction {
    Import,
    UseLayer,
    NewSprite(Groomed),
    AddLayer(Groomed),
    AddArtboard(Groomed),
}

pub struct GroomPanel {
    /// La imagen de partida y su nombre.
    source: Option<(String, Bitmap)>,
    settings: Option<Settings>,
    /// Los colores juntados a mano (se aplican al final, cada vez).
    merges: Merges,
    /// Los colores elegidos para juntar.
    picked: Vec<Rgba>,
    /// Qué tan parecidos tienen que ser los colores para sugerir juntarlos.
    similar: u16,
    /// Los pasos con `settings` y el primario de entonces, sin juntar colores (lo pesado).
    base: Option<(Settings, Rgba, Groomed)>,
    /// `base` con los colores juntados de entonces.
    result: Option<(Merges, Groomed)>,
    /// La sugerencia bajo el mouse la vez pasada (la vista previa la prueba).
    hovered_group: Option<Vec<Rgba>>,
    /// Lo que muestra la vista previa y su textura.
    texture: Option<(Shown, TextureHandle)>,
}

impl Default for GroomPanel {
    fn default() -> Self {
        Self {
            source: None,
            settings: None,
            merges: Merges::new(),
            picked: Vec::new(),
            similar: 40,
            base: None,
            result: None,
            hovered_group: None,
            texture: None,
        }
    }
}

/// Importar una imagen o tomar la capa actual.
fn import_buttons(ui: &mut Ui, action: &mut Option<GroomAction>) {
    ui.horizontal(|ui| {
        if pixel_ui::text_button(ui, "Import Image…").clicked() {
            *action = Some(GroomAction::Import);
        }
        if pixel_ui::text_button(ui, "Use Layer").clicked() {
            *action = Some(GroomAction::UseLayer);
        }
    });
}

impl GroomPanel {
    /// El ancho del panel: una columna hasta que hay imagen, dos con ella.
    pub fn width(&self) -> f32 {
        if self.source.is_some() {
            2.0 * COLUMN + COLUMN_GAP
        } else {
            COLUMN
        }
    }

    /// Empieza con `image`: detecta su cuadrícula.
    pub fn load(&mut self, name: String, image: Bitmap) {
        let grid = cs_raster::detect_grid(&image.pixels, image.width, image.height);
        self.settings = Some(Settings::new(grid));
        self.source = Some((name, image));
        self.base = None;
        self.result = None;
        self.merges.clear();
        self.picked.clear();
    }

    /// El resultado con los ajustes de ahora: los pasos se rehacen solo si cambiaron los
    /// ajustes (o el primario) y los colores juntados, solo si cambiaron ellos.
    fn groomed(&mut self, primary: Rgba) -> Option<&Groomed> {
        let (name, source) = self.source.as_ref()?;
        let settings = self.settings?;
        if !matches!(&self.base, Some((s, p, _)) if *s == settings && *p == primary) {
            let base = process(name, source, settings, primary, &Merges::new());
            self.base = Some((settings, primary, base));
            self.result = None;
        }
        if !matches!(&self.result, Some((m, _)) if *m == self.merges) {
            let (_, _, base) = self.base.as_ref()?;
            self.result = Some((self.merges.clone(), merged(base, &self.merges)));
            self.texture = None;
        }
        self.result.as_ref().map(|(_, g)| g)
    }

    /// Desarrollo (capturas): elige los `n` colores más usados.
    pub(crate) fn pick_most_used(&mut self, primary: Rgba, n: usize) {
        if let Some(groomed) = self.groomed(primary) {
            self.picked = groomed.colors.iter().take(n).map(|(c, _)| *c).collect();
        }
    }

    /// El grupo que se está por juntar: el de la sugerencia bajo el mouse o, si no, los colores
    /// elegidos (el más usado primero, como lo deja Merge).
    fn trying(&self, groomed: &Groomed) -> Option<Vec<Rgba>> {
        if let Some(group) = &self.hovered_group {
            return Some(group.clone());
        }
        (self.picked.len() >= 2).then(|| merge_order(self.picked.clone(), groomed))
    }

    pub fn ui(&mut self, ui: &mut Ui, primary: Rgba) -> Option<GroomAction> {
        let mut action = None;
        ui.set_width(self.width());
        if self.source.is_none() {
            import_buttons(ui, &mut action);
            ui.label("Bring an image that looks like pixel art (blurry, scaled up) to make it real pixel art.");
            return action;
        }
        let mut groomed = None;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(COLUMN);
                import_buttons(ui, &mut action);
                self.settings_ui(ui);
                ui.add_space(4.0);
                groomed = self.groomed(primary).cloned();
                if let Some(groomed) = &groomed {
                    self.preview(ui, groomed);
                }
            });
            if let Some(groomed) = &groomed {
                ui.add_space(COLUMN_GAP - ui.spacing().item_spacing.x);
                ui.vertical(|ui| {
                    ui.set_width(COLUMN);
                    self.swatches(ui, groomed);
                    ui.add_space(6.0);
                    self.hovered_group = self.suggestions(ui, groomed);
                });
            }
        });
        let Some(groomed) = groomed else {
            return action;
        };
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if pixel_ui::text_button(ui, "New Sprite").clicked() {
                action = Some(GroomAction::NewSprite(groomed.clone()));
            }
            if pixel_ui::text_button(ui, "Add as Layer").clicked() {
                action = Some(GroomAction::AddLayer(groomed.clone()));
            }
            if pixel_ui::text_button(ui, "Add as Artboard").clicked() {
                action = Some(GroomAction::AddArtboard(groomed.clone()));
            }
        });
        action
    }

    /// Los pasos: la cuadrícula, el fondo, el contorno y los colores.
    fn settings_ui(&mut self, ui: &mut Ui) {
        let Some((_, source)) = &self.source else {
            return;
        };
        let (width, height) = (source.width, source.height);
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        ui.add_space(4.0);
        // La cuadrícula: cuánto mide cada píxel de verdad y dónde empieza.
        egui::Grid::new("groom_grid")
            .num_columns(3)
            .spacing(vec2(6.0, 4.0))
            .show(ui, |ui| {
                ui.label("Cell");
                widgets::number(ui, &mut settings.grid.cell.0, 1.0..=width as f32, "");
                widgets::number(ui, &mut settings.grid.cell.1, 1.0..=height as f32, "");
                ui.end_row();
                ui.label("Offset");
                let (cx, cy) = settings.grid.cell;
                widgets::number(ui, &mut settings.grid.offset.0, -cx..=cx, "");
                widgets::number(ui, &mut settings.grid.offset.1, -cy..=cy, "");
                ui.end_row();
            });
        if pixel_ui::text_button(ui, "Detect Grid").clicked() {
            settings.grid = cs_raster::detect_grid(&source.pixels, width, height);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            widgets::checkbox(ui, &mut settings.background, "Remove Background");
            ui.add_enabled_ui(settings.background, |ui| {
                widgets::number(ui, &mut settings.tolerance, 0..=255, "");
            });
        });
        widgets::checkbox(ui, &mut settings.clean, "Clean Outline");
        widgets::checkbox(ui, &mut settings.ring, "Add Outline (primary color)");
        ui.horizontal(|ui| {
            widgets::checkbox(ui, &mut settings.reduce, "Reduce Colors");
            ui.add_enabled_ui(settings.reduce, |ui| {
                widgets::number(ui, &mut settings.colors, 2..=256, "");
            });
        });
    }

    /// Junta `group` (el primero queda; los demás pasan a ser él) y suelta los elegidos.
    fn merge(&mut self, group: Vec<Rgba>) {
        if group.len() > 1 {
            self.merges.push(group);
        }
        self.picked.clear();
    }

    /// Grupos de colores parecidos para juntar: un clic elige los de uno (para revisarlos y
    /// juntarlos con Merge) y "Merge All" los junta todos. Devuelve el que está bajo el mouse.
    fn suggestions(&mut self, ui: &mut Ui, groomed: &Groomed) -> Option<Vec<Rgba>> {
        let groups = cs_raster::suggest_merges(&groomed.colors, self.similar as u32);
        ui.horizontal(|ui| {
            ui.label("Suggested");
            widgets::number(ui, &mut self.similar, 1..=255, "");
            ui.label("tolerance");
        });
        if groups.is_empty() {
            ui.label("No similar colors.");
            return None;
        }
        let row = SWATCH + SWATCH_GAP;
        let per_row = ((COLUMN + SWATCH_GAP) / row).floor() as usize;
        let shown = groups.len().min(MAX_SUGGESTIONS);
        let (area, _) =
            ui.allocate_exact_size(vec2(COLUMN, shown as f32 * (row + 2.0)), Sense::hover());
        let border = pixel_ui::control_border(ui.ctx());
        let accent = crate::theme::ACCENT;
        let mut chosen = None;
        let mut hovered = None;
        for (i, group) in groups.iter().take(shown).enumerate() {
            let line = Rect::from_min_size(
                area.min + vec2(0.0, i as f32 * (row + 2.0)),
                vec2(COLUMN, SWATCH + 2.0),
            );
            let response = ui
                .interact(line, ui.id().with(("groom_suggestion", i)), Sense::click())
                .on_hover_text("Pick these colors (the first one stays)");
            if response.clicked() {
                chosen = Some(group.clone());
            }
            if response.hovered() {
                hovered = Some(group.clone());
            }
            let painter = ui.painter();
            let picked = self.picked == *group;
            if picked || response.hovered() {
                let fill = if picked { accent } else { border };
                painter.rect_filled(line, 0.0, fill.gamma_multiply(0.35));
            }
            // El que queda, una flecha y los que pasan a ser él.
            let mut x = line.left() + 1.0;
            let y = line.top() + 1.0;
            let cells = per_row.saturating_sub(2);
            for (k, &color) in group.iter().take(cells).enumerate() {
                let rect = Rect::from_min_size(pos2(x, y), vec2(SWATCH, SWATCH));
                painter.rect_filled(rect, 0.0, crate::panels::to_color32(color));
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
                x += row;
                if k == 0 {
                    painter.text(
                        pos2(x + SWATCH / 2.0, y + SWATCH / 2.0),
                        egui::Align2::CENTER_CENTER,
                        "←",
                        egui::TextStyle::Body.resolve(ui.style()),
                        ui.visuals().text_color(),
                    );
                    x += row;
                }
            }
            if group.len() > cells {
                painter.text(
                    pos2(x, y + SWATCH / 2.0),
                    egui::Align2::LEFT_CENTER,
                    format!("+{}", group.len() - cells),
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
            }
        }
        if let Some(group) = chosen {
            self.picked = if self.picked == group {
                Vec::new()
            } else {
                group
            };
        }
        if groups.len() > shown {
            ui.label(format!("+{} more groups", groups.len() - shown));
        }
        if pixel_ui::text_button(ui, "Merge All Suggested").clicked() {
            self.picked.clear();
            self.merges.extend(groups);
        }
        hovered
    }

    /// Los colores del resultado (los más usados primero): un clic elige o suelta uno, y
    /// "Merge" junta los elegidos en el más usado de ellos.
    fn swatches(&mut self, ui: &mut Ui, groomed: &Groomed) {
        ui.label("Colors (click to pick)");
        let shown = groomed.colors.len().min(MAX_SWATCHES);
        let per_row = ((COLUMN + SWATCH_GAP) / (SWATCH + SWATCH_GAP))
            .floor()
            .max(1.0) as usize;
        let rows = shown.div_ceil(per_row);
        let height = rows as f32 * (SWATCH + SWATCH_GAP);
        let (area, _) = ui.allocate_exact_size(vec2(COLUMN, height), Sense::hover());
        let border = pixel_ui::control_border(ui.ctx());
        let accent = crate::theme::ACCENT;
        for (i, &(color, count)) in groomed.colors.iter().take(shown).enumerate() {
            let (col, row) = ((i % per_row) as f32, (i / per_row) as f32);
            let rect = Rect::from_min_size(
                area.min + vec2(col, row) * (SWATCH + SWATCH_GAP),
                vec2(SWATCH, SWATCH),
            );
            let response = ui
                .interact(rect, ui.id().with(("groom_swatch", i)), Sense::click())
                .on_hover_text(format!("{} · {count} px", color.to_hex()));
            if response.clicked() {
                match self.picked.iter().position(|&c| c == color) {
                    Some(at) => {
                        self.picked.remove(at);
                    }
                    None => self.picked.push(color),
                }
            }
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, crate::panels::to_color32(color));
            let picked = self.picked.contains(&color);
            let (stroke, width) = if picked { (accent, 2.0) } else { (border, 1.0) };
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(width, stroke),
                egui::StrokeKind::Inside,
            );
        }
        if groomed.colors.len() > shown {
            ui.label(format!(
                "+{} more (Reduce Colors first)",
                groomed.colors.len() - shown
            ));
        }
        ui.horizontal(|ui| {
            let can_merge = self.picked.len() >= 2;
            if ui
                .add_enabled_ui(can_merge, |ui| pixel_ui::text_button(ui, "Merge"))
                .inner
                .clicked()
                && can_merge
            {
                // El más usado de los elegidos queda; los demás pasan a ser él.
                let group = merge_order(self.picked.clone(), groomed);
                self.merge(group);
            }
            if !self.merges.is_empty() && pixel_ui::text_button(ui, "Reset Merges").clicked() {
                self.merges.clear();
                self.picked.clear();
            }
        });
    }

    /// El resultado al tamaño más grande que entra (en múltiplos enteros si se agranda), sobre
    /// el damero. Con un grupo por juntar (una sugerencia bajo el mouse o colores elegidos) se ve
    /// cómo quedaría, y con "Before" presionado, cómo era sin juntar ninguno. Encima, la lupa;
    /// un clic elige (o suelta) el color del píxel.
    fn preview(&mut self, ui: &mut Ui, groomed: &Groomed) {
        if groomed.image.width == 0 || groomed.image.height == 0 {
            return;
        }
        let before = ui
            .data(|d| d.get_temp::<bool>(before_id()))
            .unwrap_or(false);
        let trying = self.trying(groomed);
        let can_compare = trying.is_some() || !self.merges.is_empty();
        let shown = match trying {
            _ if before && can_compare => Shown::Before,
            Some(group) => Shown::Trying(group),
            None => Shown::Result,
        };
        let image = match &shown {
            Shown::Result => groomed.image.clone(),
            // Lo de antes: sin lo que se prueba o, si no se prueba nada, sin juntar ninguno.
            Shown::Before if self.trying(groomed).is_some() => groomed.image.clone(),
            Shown::Before => match &self.base {
                Some((_, _, base)) => base.image.clone(),
                None => groomed.image.clone(),
            },
            Shown::Trying(group) => merged(groomed, &vec![group.clone()]).image,
        };
        if !matches!(&self.texture, Some((s, _)) if *s == shown) {
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|c| [c.r, c.g, c.b, c.a])
                .collect();
            let color = ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &bytes,
            );
            let texture = ui
                .ctx()
                .load_texture("groom_preview", color, TextureOptions::NEAREST);
            self.texture = Some((shown.clone(), texture));
        }
        let Some((_, texture)) = &self.texture else {
            return;
        };
        let fit = PREVIEW / image.width.max(image.height) as f32;
        let k = if fit >= 1.0 { fit.floor() } else { fit };
        let size = vec2(image.width as f32, image.height as f32) * k;
        let (row, _) = ui.allocate_exact_size(vec2(PREVIEW, size.y), Sense::hover());
        let rect = Rect::from_center_size(row.center(), size);
        let response = ui.interact(rect, ui.id().with("groom_preview"), Sense::click());
        let painter = ui.painter_at(rect);
        crate::panels::checker(&painter, rect);
        let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), rect, full, Color32::WHITE);
        // El píxel bajo el mouse: la lupa y, con un clic, elegir su color. (Para capturas, con
        // CHIMPSPRITE_DEMO_GROOM_LOUPE, en el centro.)
        let demo = crate::demo::dev_var("CHIMPSPRITE_DEMO_GROOM_LOUPE").map(|_| rect.center());
        if let Some(pointer) = response.hover_pos().or(demo) {
            let at = ((pointer - rect.min) / k).floor();
            let (x, y) = (at.x as u32, at.y as u32);
            if x < image.width && y < image.height {
                let color = image.pixels[(y * image.width + x) as usize];
                loupe(ui, texture, &image, (x, y), color, pointer);
                if response.clicked() && color.a > 0 {
                    match self.picked.iter().position(|&c| c == color) {
                        Some(at) => {
                            self.picked.remove(at);
                        }
                        None => self.picked.push(color),
                    }
                }
            }
        }
        ui.horizontal(|ui| {
            ui.label(format!(
                "{}×{} · {} colors",
                groomed.image.width,
                groomed.image.height,
                groomed.colors.len()
            ));
            // "Before": mientras está presionado, lo de antes de juntar.
            if can_compare {
                let held = pixel_ui::text_button(ui, "Before")
                    .on_hover_text("Hold to see it before merging")
                    .is_pointer_button_down_on();
                ui.data_mut(|d| d.insert_temp(before_id(), held));
            }
        });
        let note = match shown {
            Shown::Trying(_) => "Previewing the merge (Merge keeps it).",
            Shown::Before => "Before merging.",
            Shown::Result => "Hover to zoom; click a pixel to pick its color.",
        };
        ui.label(note);
    }
}

/// Dónde se guarda si "Before" está presionado (se lee al pintar la vista previa, arriba del
/// botón: vale lo de la vez pasada).
fn before_id() -> egui::Id {
    egui::Id::new("groom_before")
}

/// La lupa junto al mouse: los píxeles alrededor de `(x, y)` en grande, el del centro marcado y
/// su color.
fn loupe(
    ui: &Ui,
    texture: &TextureHandle,
    image: &Bitmap,
    (x, y): (u32, u32),
    color: Rgba,
    pointer: egui::Pos2,
) {
    let layer = egui::LayerId::new(egui::Order::Tooltip, ui.id().with("groom_loupe"));
    let painter = ui.ctx().layer_painter(layer);
    let screen = ui.ctx().content_rect();
    // A la derecha y abajo del mouse; si no cabe, del otro lado.
    let mut min = pointer + vec2(18.0, 18.0);
    if min.x + LOUPE > screen.right() {
        min.x = pointer.x - 18.0 - LOUPE;
    }
    if min.y + LOUPE + 16.0 > screen.bottom() {
        min.y = pointer.y - 18.0 - LOUPE - 16.0;
    }
    let rect = Rect::from_min_size(min.round(), vec2(LOUPE, LOUPE));
    let cell = LOUPE / LOUPE_PIXELS;
    let half = (LOUPE_PIXELS / 2.0).floor();
    let (w, h) = (image.width as f32, image.height as f32);
    // Los píxeles de `(x - half, y - half)` a `(x + half, y + half)` (lo de afuera, vacío).
    let from = vec2(x as f32 - half, y as f32 - half);
    let uv = Rect::from_min_size(
        pos2(from.x / w, from.y / h),
        vec2(LOUPE_PIXELS / w, LOUPE_PIXELS / h),
    );
    let clipped = painter.with_clip_rect(rect);
    clipped.rect_filled(rect, 0.0, crate::theme::CANVAS_BG);
    crate::panels::checker(&clipped, rect);
    // La textura no se repite: fuera de ella queda el damero.
    let inside = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let visible = uv.intersect(inside);
    if visible.is_positive() {
        let to_screen = |p: egui::Pos2| {
            rect.min
                + vec2(
                    (p.x - uv.min.x) / uv.width(),
                    (p.y - uv.min.y) / uv.height(),
                ) * LOUPE
        };
        let target = Rect::from_min_max(to_screen(visible.min), to_screen(visible.max));
        clipped.image(texture.id(), target, visible, Color32::WHITE);
    }
    // El píxel del centro.
    let center = Rect::from_min_size(rect.min + vec2(half, half) * cell, vec2(cell, cell));
    let stroke = |c| egui::Stroke::new(1.0, c);
    clipped.rect_stroke(
        center,
        0.0,
        stroke(Color32::BLACK),
        egui::StrokeKind::Outside,
    );
    clipped.rect_stroke(
        center,
        0.0,
        stroke(Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect,
        0.0,
        stroke(pixel_ui::control_border(ui.ctx())),
        egui::StrokeKind::Outside,
    );
    let text = if color.a == 0 {
        format!("{x}, {y} · empty")
    } else {
        format!("{x}, {y} · {}", color.to_hex())
    };
    let label = Rect::from_min_size(rect.left_bottom(), vec2(LOUPE, 16.0));
    painter.rect_filled(label, 0.0, crate::theme::CANVAS_BG);
    painter.text(
        label.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::TextStyle::Body.resolve(ui.style()),
        Color32::WHITE,
    );
}

/// `group` ordenado para juntarlo: el más usado (en `groomed`) primero, que es el que queda.
fn merge_order(mut group: Vec<Rgba>, groomed: &Groomed) -> Vec<Rgba> {
    let count = |c: &Rgba| {
        groomed
            .colors
            .iter()
            .find(|(k, _)| k == c)
            .map_or(0, |(_, n)| *n)
    };
    group.sort_by_key(|c| std::cmp::Reverse(count(c)));
    group
}

/// `groomed` con los colores de cada grupo de `merges` cambiados por el primero del grupo.
fn merged(groomed: &Groomed, merges: &Merges) -> Groomed {
    let mut image = groomed.image.clone();
    if !merges.is_empty() {
        let target: std::collections::HashMap<Rgba, Rgba> = merges
            .iter()
            .flat_map(|group| group.iter().skip(1).map(move |&c| (c, group[0])))
            .collect();
        for p in image.pixels.iter_mut() {
            if let Some(&to) = target.get(p) {
                *p = to;
            }
        }
    }
    Groomed {
        name: groomed.name.clone(),
        colors: color_counts(&image),
        image,
    }
}

/// Los colores de `image` y cuántos píxeles tiene cada uno, del más usado al menos.
fn color_counts(image: &Bitmap) -> Vec<(Rgba, usize)> {
    let mut counts: std::collections::HashMap<Rgba, usize> = std::collections::HashMap::new();
    for &c in image.pixels.iter().filter(|c| c.a > 0) {
        *counts.entry(c).or_default() += 1;
    }
    let mut colors: Vec<(Rgba, usize)> = counts.into_iter().collect();
    colors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_hex().cmp(&b.0.to_hex())));
    colors
}

/// Los pasos sobre `source`: un color por celda, el fondo, el contorno, los colores y el recorte
/// a lo pintado.
pub fn process(
    name: &str,
    source: &Bitmap,
    settings: Settings,
    primary: Rgba,
    merges: &Merges,
) -> Groomed {
    let (mut pixels, w, h) =
        cs_raster::sample_grid(&source.pixels, source.width, source.height, settings.grid);
    if settings.background {
        pixels = cs_raster::remove_background(&pixels, w, h, settings.tolerance);
    }
    if settings.clean || settings.ring {
        let ring = settings.ring.then_some(primary);
        if settings.clean {
            pixels = cs_raster::clean_outline(&pixels, w, h, ring);
        } else if let Some(color) = ring {
            // Solo el contorno, sin tocar lo demás.
            let cleaned = cs_raster::clean_outline(&pixels, w, h, Some(color));
            pixels = pixels
                .iter()
                .zip(cleaned)
                .map(|(&p, c)| if p.a == 0 && c == color { c } else { p })
                .collect();
        }
    }
    if settings.reduce {
        pixels = cs_raster::reduce_colors(&pixels, settings.colors as usize).0;
    }
    let image = trimmed(&pixels, w, h);
    let groomed = Groomed {
        name: name.to_string(),
        colors: color_counts(&image),
        image,
    };
    // Al final, los grupos juntados a mano (el recorte no cambia: solo cambian colores).
    merged(&groomed, merges)
}

/// `pixels` recortado a lo pintado (entero si no hay nada).
fn trimmed(pixels: &[Rgba], w: u32, h: u32) -> Bitmap {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (i, c) in pixels.iter().enumerate() {
        if c.a > 0 {
            let (x, y) = (i as u32 % w, i as u32 / w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    if x0 > x1 {
        return Bitmap {
            width: w,
            height: h,
            pixels: pixels.to_vec(),
        };
    }
    let (tw, th) = (x1 - x0 + 1, y1 - y0 + 1);
    Bitmap {
        width: tw,
        height: th,
        pixels: (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .map(|(x, y)| pixels[(y * w + x) as usize])
            .collect(),
    }
}

/// Una imagen de cualquier formato común (PNG, JPEG, GIF, WebP en escritorio; PNG en la web).
pub fn decode_image(bytes: &[u8]) -> Result<Bitmap, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let decoded = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels = rgba
            .pixels()
            .map(|p| Rgba::new(p[0], p[1], p[2], p[3]))
            .collect();
        Ok(Bitmap {
            width,
            height,
            pixels,
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        cs_io::import::decode_png(bytes).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4×3 celdas de 5 px (rojo sobre blanco), agrandado.
    fn big() -> Bitmap {
        let (cw, ch, k) = (4u32, 3u32, 5u32);
        let white = Rgba::rgb(255, 255, 255);
        let red = Rgba::rgb(200, 30, 30);
        let pixels = (0..ch * k)
            .flat_map(|y| (0..cw * k).map(move |x| (x, y)))
            .map(|(x, y)| {
                let (cx, cy) = (x / k, y / k);
                if (1..3).contains(&cx) && cy == 1 {
                    red
                } else {
                    white
                }
            })
            .collect();
        Bitmap {
            width: cw * k,
            height: ch * k,
            pixels,
        }
    }

    #[test]
    fn the_steps_give_the_trimmed_pixels() {
        let source = big();
        let grid = GridGuess {
            cell: (5.0, 5.0),
            offset: (0.0, 0.0),
        };
        let groomed = process(
            "x",
            &source,
            Settings::new(grid),
            Rgba::rgb(0, 0, 0),
            &Vec::new(),
        );
        assert_eq!(
            (groomed.image.width, groomed.image.height),
            (2, 1),
            "only the red"
        );
        assert_eq!(groomed.colors.len(), 1);
        // Con contorno: 4×3.
        let mut settings = Settings::new(grid);
        settings.ring = true;
        let ringed = process("x", &source, settings, Rgba::rgb(0, 0, 0), &Vec::new());
        assert_eq!((ringed.image.width, ringed.image.height), (4, 3));
        assert_eq!(ringed.colors.len(), 2);
        // Juntar el contorno negro con el rojo: queda un color, el primero del grupo.
        let red = Rgba::rgb(200, 30, 30);
        let merged = process(
            "x",
            &source,
            settings,
            Rgba::rgb(0, 0, 0),
            &vec![vec![red, Rgba::rgb(0, 0, 0)]],
        );
        assert_eq!(merged.colors, vec![(red, 8)], "2 red + 6 of outline");
    }

    #[test]
    fn the_merge_being_tried_is_the_hovered_suggestion_or_the_picked_colors() {
        let mut panel = GroomPanel::default();
        panel.load("x".into(), big());
        panel.settings.as_mut().unwrap().ring = true;
        let groomed = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap().clone();
        let (red, black) = (Rgba::rgb(200, 30, 30), Rgba::rgb(0, 0, 0));
        assert_eq!(panel.trying(&groomed), None);
        // Elegidos: el más usado (el contorno negro, 6 contra 2) queda primero.
        panel.picked = vec![red, black];
        assert_eq!(panel.trying(&groomed), Some(vec![black, red]));
        // La sugerencia bajo el mouse manda.
        panel.hovered_group = Some(vec![red, black]);
        assert_eq!(panel.trying(&groomed), Some(vec![red, black]));
        // Juntar no rehace los pasos: solo cambia colores sobre lo de antes.
        panel.merges.push(vec![black, red]);
        let merged = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap();
        assert_eq!(merged.colors, vec![(black, 8)]);
        assert_eq!(
            panel.base.as_ref().unwrap().2,
            groomed,
            "the steps are kept"
        );
    }

    #[test]
    fn the_result_is_only_recomputed_when_something_changes() {
        let mut panel = GroomPanel::default();
        panel.load("x".into(), big());
        let first = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap().clone();
        let again = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap().clone();
        assert_eq!(first, again);
        panel.settings.as_mut().unwrap().ring = true;
        let ringed = panel.groomed(Rgba::rgb(0, 0, 0)).unwrap();
        assert_ne!(ringed.image.width, first.image.width);
    }
}
