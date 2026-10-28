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

/// El ancho del panel.
pub const WIDTH: f32 = 210.0;
/// La vista previa: el lado más largo.
const PREVIEW: f32 = 190.0;

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

#[derive(Default)]
pub struct GroomPanel {
    /// La imagen de partida y su nombre.
    source: Option<(String, Bitmap)>,
    settings: Option<Settings>,
    /// Los colores juntados a mano (se aplican al final, cada vez).
    merges: Merges,
    /// Los colores elegidos para juntar.
    picked: Vec<Rgba>,
    /// El resultado de `settings` (con el primario y los grupos de entonces).
    result: Option<(Settings, Rgba, Merges, Groomed)>,
    texture: Option<TextureHandle>,
}

impl GroomPanel {
    /// Empieza con `image`: detecta su cuadrícula.
    pub fn load(&mut self, name: String, image: Bitmap) {
        let grid = cs_raster::detect_grid(&image.pixels, image.width, image.height);
        self.settings = Some(Settings::new(grid));
        self.source = Some((name, image));
        self.result = None;
        self.merges.clear();
        self.picked.clear();
    }

    /// El resultado con los ajustes de ahora (se recalcula solo si cambiaron).
    fn groomed(&mut self, primary: Rgba) -> Option<&Groomed> {
        let (name, source) = self.source.as_ref()?;
        let settings = self.settings?;
        let stale = !matches!(&self.result,
            Some((s, p, m, _)) if *s == settings && *p == primary && *m == self.merges);
        if stale {
            let groomed = process(name, source, settings, primary, &self.merges);
            self.result = Some((settings, primary, self.merges.clone(), groomed));
            self.texture = None;
        }
        self.result.as_ref().map(|(_, _, _, g)| g)
    }

    pub fn ui(&mut self, ui: &mut Ui, primary: Rgba) -> Option<GroomAction> {
        let mut action = None;
        ui.set_width(WIDTH);
        ui.horizontal(|ui| {
            if pixel_ui::text_button(ui, "Import Image…").clicked() {
                action = Some(GroomAction::Import);
            }
            if pixel_ui::text_button(ui, "Use Layer").clicked() {
                action = Some(GroomAction::UseLayer);
            }
        });
        let Some((_, source)) = &self.source else {
            ui.label("Bring an image that looks like pixel art (blurry, scaled up) to make it real pixel art.");
            return action;
        };
        let (width, height) = (source.width, source.height);
        let Some(settings) = self.settings.as_mut() else {
            return action;
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
        if pixel_ui::text_button(ui, "Detect Grid").clicked()
            && let Some((_, image)) = &self.source
        {
            settings.grid = cs_raster::detect_grid(&image.pixels, image.width, image.height);
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
        ui.add_space(4.0);
        let Some(groomed) = self.groomed(primary).cloned() else {
            return action;
        };
        self.preview(ui, &groomed);
        ui.label(format!(
            "{}×{} · {} colors",
            groomed.image.width,
            groomed.image.height,
            groomed.colors.len()
        ));
        self.swatches(ui, &groomed);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if pixel_ui::text_button(ui, "New Sprite").clicked() {
                action = Some(GroomAction::NewSprite(groomed.clone()));
            }
            if pixel_ui::text_button(ui, "Add as Layer").clicked() {
                action = Some(GroomAction::AddLayer(groomed.clone()));
            }
        });
        if pixel_ui::text_button(ui, "Add as Artboard").clicked() {
            action = Some(GroomAction::AddArtboard(groomed.clone()));
        }
        action
    }

    /// Los colores del resultado (los más usados primero): un clic elige o suelta uno, y
    /// "Merge" junta los elegidos en el más usado de ellos.
    fn swatches(&mut self, ui: &mut Ui, groomed: &Groomed) {
        let shown = groomed.colors.len().min(MAX_SWATCHES);
        let per_row = ((WIDTH + SWATCH_GAP) / (SWATCH + SWATCH_GAP))
            .floor()
            .max(1.0) as usize;
        let rows = shown.div_ceil(per_row);
        let height = rows as f32 * (SWATCH + SWATCH_GAP);
        let (area, _) = ui.allocate_exact_size(vec2(WIDTH, height), Sense::hover());
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
                let mut group = std::mem::take(&mut self.picked);
                let count = |c: &Rgba| {
                    groomed
                        .colors
                        .iter()
                        .find(|(k, _)| k == c)
                        .map_or(0, |(_, n)| *n)
                };
                group.sort_by_key(|c| std::cmp::Reverse(count(c)));
                self.merges.push(group);
            }
            if !self.merges.is_empty() && pixel_ui::text_button(ui, "Reset Merges").clicked() {
                self.merges.clear();
                self.picked.clear();
            }
        });
    }

    /// El resultado al tamaño más grande que entra (en múltiplos enteros si se agranda), sobre
    /// el damero.
    fn preview(&mut self, ui: &mut Ui, groomed: &Groomed) {
        let image = &groomed.image;
        if image.width == 0 || image.height == 0 {
            return;
        }
        let texture = self.texture.get_or_insert_with(|| {
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|c| [c.r, c.g, c.b, c.a])
                .collect();
            let color = ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &bytes,
            );
            ui.ctx()
                .load_texture("groom_preview", color, TextureOptions::NEAREST)
        });
        let fit = PREVIEW / image.width.max(image.height) as f32;
        let k = if fit >= 1.0 { fit.floor() } else { fit };
        let size = vec2(image.width as f32, image.height as f32) * k;
        let (rect, _) = ui.allocate_exact_size(vec2(PREVIEW, size.y), Sense::hover());
        let rect = Rect::from_center_size(rect.center(), size);
        let painter = ui.painter_at(rect);
        crate::panels::checker(&painter, rect);
        let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), rect, full, Color32::WHITE);
    }
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
    // Los grupos juntados a mano: cada color del grupo pasa a ser el primero.
    if !merges.is_empty() {
        let target: std::collections::HashMap<Rgba, Rgba> = merges
            .iter()
            .flat_map(|group| group.iter().skip(1).map(move |&c| (c, group[0])))
            .collect();
        for p in pixels.iter_mut() {
            if let Some(&to) = target.get(p) {
                *p = to;
            }
        }
    }
    let image = trimmed(&pixels, w, h);
    let mut counts: std::collections::HashMap<Rgba, usize> = std::collections::HashMap::new();
    for &c in image.pixels.iter().filter(|c| c.a > 0) {
        *counts.entry(c).or_default() += 1;
    }
    let mut colors: Vec<(Rgba, usize)> = counts.into_iter().collect();
    colors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.to_hex().cmp(&b.0.to_hex())));
    Groomed {
        name: name.to_string(),
        image,
        colors,
    }
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
